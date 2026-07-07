use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::app_error::AppError;
use crate::hotkey_types::ConflictPolicy;
use crate::hotkeys::HotkeyManager;
use crate::profile::{self, ActiveProfileSnapshotPatch};
use crate::tool_base::{ToolLogic, ToolState, ToolStateInner};

mod effects;
mod events;
mod player;
mod settings;
mod types;
pub(crate) mod watcher;

pub use self::types::{RecognitionBootstrap, RecognitionSettings, RecognitionTriggerMode};
pub use events::*;

use crate::morse::types::RegionRect;
use crate::overlay_utils::{
    destroy_stale_windows, destroy_window, encoded_query_value, safe_label_component,
};

const RECOGNITION_OVERLAY_LABEL: &str = "recognition-overlay";

// ---- TestMatchResult ----

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchPosition {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestMatchResult {
    pub similarity: f32,
    pub triggered: bool,
    pub match_position: Option<MatchPosition>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorProbeTestResult {
    /// 该 probe 是否命中（按 probe_match_mode 聚合后）
    pub matched: bool,
    /// 截取区域采样色（average=区域平均色；anyPixel=最近像素）
    pub sampled_color: [u8; 3],
    /// 与最近目标的距离（聚合摘要）
    pub distance: f32,
    /// 最近目标颜色（聚合摘要，取距离最小的目标）
    pub target_color: [u8; 3],
    /// 最近目标容差（聚合摘要）
    pub tolerance: u8,
    /// anyPixel 命中像素数（聚合摘要）；average 恒 0
    pub matching_pixel_count: usize,
    /// Issue #65：每个目标颜色的详细命中结果
    pub targets: Vec<ColorTargetTestResult>,
}

/// 单个目标颜色的测试命中结果（Issue #65）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorTargetTestResult {
    /// 该目标是否命中
    pub matched: bool,
    /// 目标颜色
    pub target_color: [u8; 3],
    /// 容差
    pub tolerance: u8,
    /// 采样色（average=区域平均色；anyPixel=最近像素）
    pub sampled_color: [u8; 3],
    /// 采样色与目标颜色的距离
    pub distance: f32,
    /// anyPixel 命中像素数；average 恒 0
    pub matching_pixel_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorTestResult {
    /// 是否触发（按 mode 聚合后）
    pub triggered: bool,
    /// 命中 probe 数量
    pub hit_count: usize,
    /// probe 总数
    pub total_count: usize,
    /// 每个 probe 的详细结果
    pub probes: Vec<ColorProbeTestResult>,
}

// ---- State ----

pub struct RecognitionLogic {
    /// 音频播放线程的命令发送端
    pub playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
    /// 连杀/随机播放的 per-card 运行时状态（纯内存，重启归零，不持久化）。
    pub play_states: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, PlayState>>>,
}

pub type RecognitionState = ToolState<RecognitionLogic>;

/// 连杀/随机播放的卡片级运行时状态。
/// - current_index：连杀当前播放到第几个文件
/// - last_trigger_at：上一次触发时刻（连杀窗口起算点）
/// - last_random_index：随机上一次选中的索引（避免连续重复）
#[derive(Debug, Clone, Default)]
pub struct PlayState {
    pub current_index: usize,
    pub last_trigger_at: Option<std::time::Instant>,
    pub last_random_index: Option<usize>,
}

/// 随机数抖动计数器（无依赖的轻量随机源，仿 rapidfire::press_jitter_duration_ms）。
static RECOGNITION_RANDOM_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

/// 在 [0, len) 范围内取一个伪随机索引，可排除 `exclude` 指定的索引。
/// len==1 时直接返回 0（无法排除）。
fn random_index(len: usize, exclude: Option<usize>) -> usize {
    if len <= 1 {
        return 0;
    }
    let counter = RECOGNITION_RANDOM_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()))
        .unwrap_or(0);
    let mut pick = ((nanos ^ counter.rotate_left(13)) as usize) % len;
    if Some(pick) == exclude {
        // 与上一次重复：顺移到下一个索引
        pick = (pick + 1) % len;
    }
    pick
}

/// 按播放方式选择要播放的音频文件路径，并就地更新 PlayState。
///
/// - Single：直接返回 files[0]，不动 state。
/// - Combo：距上次触发 < 当前 index 的连杀窗口 → current_index+1（封顶末首）；否则复位 0。
///   per-segment 窗口取自 `combo_windows`：用 `combo_windows[current_index]` 判断
///   「播完第 i 段后用第 i 段窗口决定是否进 i+1 段」（Issue #62）。
///   `combo_windows` 长度不足或缺省 index 时回落到 `combo_window_ms`（向后兼容旧数据）。
/// - Random：随机选一个，避免与上一次重复（记录 last_random_index）。
///
/// `now` 由调用方注入，便于单测控制时间。调用前应保证 files 非空。
pub(crate) fn pick_audio_file(
    files: &[String],
    mode: types::PlayMode,
    combo_windows: &[u32],
    combo_window_ms: u32,
    state: &mut PlayState,
    now: std::time::Instant,
) -> String {
    if files.is_empty() {
        return String::new();
    }
    if files.len() == 1 || mode == types::PlayMode::Single {
        return files[0].clone();
    }
    match mode {
        types::PlayMode::Single => files[0].clone(),
        types::PlayMode::Combo => {
            let last_index = files.len() - 1;
            // 取当前 index 的连杀窗口；缺省则回落到卡片级 combo_window_ms
            let window_ms = combo_windows
                .get(state.current_index)
                .copied()
                .unwrap_or(combo_window_ms)
                .max(100);
            let in_window = match state.last_trigger_at {
                Some(last) => {
                    now.duration_since(last) < std::time::Duration::from_millis(window_ms as u64)
                }
                None => false, // 首次触发不在窗口内 → 走复位分支播第一首
            };
            if in_window {
                state.current_index = (state.current_index + 1).min(last_index);
            } else {
                state.current_index = 0;
            }
            state.last_trigger_at = Some(now);
            files[state.current_index].clone()
        }
        types::PlayMode::Random => {
            let pick = random_index(files.len(), state.last_random_index);
            state.last_random_index = Some(pick);
            state.last_trigger_at = Some(now);
            files[pick].clone()
        }
    }
}

impl ToolLogic for RecognitionLogic {
    type Settings = RecognitionSettings;
    type Bootstrap = RecognitionBootstrap;

    const NAME: &'static str = "识别触发";

    fn load_settings(app: &tauri::AppHandle) -> Result<Self::Settings, String> {
        settings::read_settings(app)
    }

    fn save_settings(app: &tauri::AppHandle, settings: &Self::Settings) -> Result<(), String> {
        settings::write_settings(app, settings)
    }

    fn build_bootstrap(inner: &ToolStateInner<Self>) -> Self::Bootstrap {
        RecognitionBootstrap {
            settings: inner.settings.clone(),
            hotkey_error: inner.hotkey_error.clone(),
        }
    }

    fn emit_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, bootstrap: &Self::Bootstrap) {
        let _ = app.emit(STATE_CHANGED, bootstrap);
    }
}

// ---- Tauri commands ----

#[tauri::command]
pub fn recognition_get_bootstrap(
    state: tauri::State<'_, RecognitionState>,
) -> Result<RecognitionBootstrap, AppError> {
    crate::tool_base::get_bootstrap(state).map_err(AppError::from)
}

#[tauri::command]
pub async fn recognition_save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecognitionState>,
    settings_value: RecognitionSettings,
) -> Result<RecognitionBootstrap, AppError> {
    let normalized = normalize_settings(settings_value);
    validate_settings(&normalized).map_err(AppError::from)?;
    let previous_settings = {
        let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        inner.settings.clone()
    };

    // 先保存到磁盘，失败时直接返回错误，不重启 listeners
    settings::write_settings(&app, &normalized).map_err(|e| AppError::from(e))?;

    let playback_tx = {
        let mut inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        inner.settings = normalized.clone();
        inner.logic.playback_tx.clone()
    };

    // 然后重启热键和 watcher。不要持有 RecognitionState 锁做 watcher IPC。
    let hotkey_manager = app.state::<HotkeyManager>();
    if let Err(e) = restart_hotkey_listeners(&hotkey_manager, &normalized) {
        let _ = settings::write_settings(&app, &previous_settings);
        let mut inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        inner.settings = previous_settings;
        inner.hotkey_error = Some(e.clone());
        return Err(AppError::from(e));
    }
    let _ = watcher::restart_watchers(&app, &normalized, playback_tx);

    let bootstrap = {
        let mut inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        inner.hotkey_error = None;
        RecognitionLogic::build_bootstrap(&inner)
    };
    RecognitionLogic::emit_state(&app, &bootstrap);
    profile::update_active_profile_snapshot(
        &app,
        ActiveProfileSnapshotPatch::Recognition(bootstrap.settings.clone()),
    )?;
    Ok(bootstrap)
}

#[tauri::command]
pub fn recognition_set_hotkey_recording(
    recording: bool,
    hotkey_manager: tauri::State<'_, HotkeyManager>,
) -> Result<(), AppError> {
    hotkey_manager
        .set_scope_enabled("recognition", !recording)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn recognition_begin_region_selection(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecognitionState>,
    card_id: String,
    selection_target: Option<String>,
    probe_index: Option<usize>,
) -> Result<(), AppError> {
    let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
    if !inner.settings.recognition_enabled {
        return Err(AppError::from("识别触发功能未启用".to_string()));
    }

    // 检查卡片存在
    let card = inner
        .settings
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| AppError::from("卡片不存在".to_string()))?;

    if !card.enabled {
        return Err(AppError::from("卡片未启用".to_string()));
    }

    // 识色模式传了 probe_index 时，校验探针索引有效
    if selection_target.as_deref() != Some("customClick") {
        if let Some(idx) = probe_index {
            if matches!(card.trigger_mode, types::RecognitionTriggerMode::ColorWatch)
                && idx >= card.color_probes.len()
            {
                return Err(AppError::from("探针索引越界".to_string()));
            }
        }
    }
    drop(inner);

    // 创建 overlay 窗口
    let label = format!(
        "{}-{}",
        RECOGNITION_OVERLAY_LABEL,
        safe_label_component(&card_id)
    );
    let mut active_labels = std::collections::HashSet::new();
    active_labels.insert(label.clone());
    destroy_stale_windows(&app, RECOGNITION_OVERLAY_LABEL, &active_labels);
    destroy_window(&app, &label);

    // 用 xcap 获取主显示器物理尺寸，显式设置窗口 inner_size + position 覆盖全屏，
    // 替代 fullscreen(true)——后者在 WebView2 透明窗口上可能只覆盖部分区域。
    let (screen_x, screen_y, screen_w, screen_h) = xcap::Monitor::all()
        .ok()
        .and_then(|monitors| monitors.into_iter().next())
        .map(|monitor| {
            let x = monitor.x().unwrap_or(0);
            let y = monitor.y().unwrap_or(0);
            let w = monitor.width().unwrap_or(1920);
            let h = monitor.height().unwrap_or(1080);
            (x, y, w, h)
        })
        .unwrap_or((0, 0, 1920, 1080));

    // 识色探针框选时把 probe_index 透传给 overlay，提交时回传定位探针
    let mut url = format!(
        "index.html?mode=recognition-overlay&recognition_card={}",
        encoded_query_value(&card_id)
    );
    if let Some(idx) = probe_index {
        url.push_str(&format!("&probe_index={idx}"));
    }
    if let Some(target) = selection_target.as_ref() {
        url.push_str(&format!(
            "&selection_target={}",
            encoded_query_value(target)
        ));
    }

    let window = tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App(url.into()))
        .title("识别区域选择")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .visible(true)
        .resizable(false)
        .inner_size(screen_w as f64, screen_h as f64)
        .position(screen_x as f64, screen_y as f64)
        .build()
        .map_err(|error| AppError::from(format!("创建识别区域选择窗口失败: {error}")))?;

    // 窗口创建后强制最大化，防止 DPI 缩放导致尺寸不足
    let _ = window.maximize();

    Ok(())
}

#[tauri::command]
pub async fn recognition_overlay_submit_selection(
    app: tauri::AppHandle,
    state: tauri::State<'_, RecognitionState>,
    card_id: String,
    region: RegionRect,
    selection_target: Option<String>,
    probe_index: Option<usize>,
) -> Result<(), AppError> {
    // 关闭 overlay 窗口
    let overlay_label = format!(
        "{}-{}",
        RECOGNITION_OVERLAY_LABEL,
        safe_label_component(&card_id)
    );
    destroy_window(&app, &overlay_label);

    // 更新卡片区域
    let (settings_snapshot, bootstrap, playback_tx) = {
        let mut inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        let Some(card) = inner.settings.cards.iter_mut().find(|c| c.id == card_id) else {
            return Err(AppError::from("卡片不存在".to_string()));
        };

        if selection_target.as_deref() == Some("customClick") {
            let click = card
                .effects
                .click
                .get_or_insert(types::RecognitionClickEffect {
                    mode: types::RecognitionClickMode::CustomRegion,
                    custom_region: None,
                    color_probe_index: None,
                });
            click.mode = types::RecognitionClickMode::CustomRegion;
            click.custom_region = Some(region);
        } else if let Some(idx) = probe_index {
            if matches!(card.trigger_mode, types::RecognitionTriggerMode::ColorWatch) {
                let probe = card
                    .color_probes
                    .get_mut(idx)
                    .ok_or_else(|| AppError::from("探针已变更，请重新框选".to_string()))?;
                probe.region = Some(region);
            } else {
                card.watch_region = Some(region);
            }
        } else {
            card.watch_region = Some(region);
        }
        settings::write_settings(&app, &inner.settings).map_err(|e| AppError::from(e))?;
        (
            inner.settings.clone(),
            RecognitionLogic::build_bootstrap(&inner),
            inner.logic.playback_tx.clone(),
        )
    };

    watcher::restart_watchers(&app, &settings_snapshot, playback_tx).map_err(AppError::from)?;
    RecognitionLogic::emit_state(&app, &bootstrap);
    profile::update_active_profile_snapshot(
        &app,
        ActiveProfileSnapshotPatch::Recognition(settings_snapshot),
    )?;
    Ok(())
}

#[tauri::command]
pub async fn recognition_overlay_cancel_selection(
    app: tauri::AppHandle,
    card_id: String,
) -> Result<(), AppError> {
    let overlay_label = format!(
        "{}-{}",
        RECOGNITION_OVERLAY_LABEL,
        safe_label_component(&card_id)
    );
    destroy_window(&app, &overlay_label);
    Ok(())
}

#[tauri::command]
pub async fn recognition_test_play(
    _app: tauri::AppHandle,
    state: tauri::State<'_, RecognitionState>,
    card_id: String,
) -> Result<(), AppError> {
    let (path, volume, allow_simultaneous, playback_tx) = {
        let inner = state.lock_inner().map_err(AppError::from)?;
        let playback_tx = inner.logic.playback_tx.clone();
        let resolved = resolve_audio_path(&inner, &card_id, std::time::Instant::now())
            .map_err(AppError::from)?;
        (
            resolved.path,
            resolved.volume,
            resolved.allow_simultaneous,
            playback_tx,
        )
    };

    // 通过协调器播放音频
    let _ = playback_tx.send(player::AudioCommand::Play {
        path,
        volume,
        exclusive: !allow_simultaneous,
    });

    Ok(())
}

#[tauri::command]
pub async fn recognition_test_match(
    _app: tauri::AppHandle,
    state: tauri::State<'_, RecognitionState>,
    card_id: String,
) -> Result<TestMatchResult, AppError> {
    let (region, ref_path, threshold) = {
        let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        let card = inner
            .settings
            .cards
            .iter()
            .find(|c| c.id == card_id)
            .ok_or_else(|| AppError::from("卡片不存在".to_string()))?;

        match card.trigger_mode {
            types::RecognitionTriggerMode::RegionWatch => {}
            types::RecognitionTriggerMode::ColorWatch => {
                return Err(AppError::from(
                    "识色模式请使用 recognition_test_color_match 命令".to_string(),
                ));
            }
            types::RecognitionTriggerMode::Hotkey => {
                return Err(AppError::from("快捷键模式不支持匹配测试".to_string()));
            }
        }
        let region = card
            .watch_region
            .clone()
            .ok_or_else(|| AppError::from("未设置监听区域".to_string()))?;
        let ref_path = card
            .watch_reference_image_path
            .clone()
            .ok_or_else(|| AppError::from("未设置参考图像".to_string()))?;
        if ref_path.is_empty() {
            return Err(AppError::from("参考图像路径为空".to_string()));
        }
        let threshold = card.watch_match_threshold;
        (region, ref_path, threshold)
    };

    // 截图
    let captured =
        watcher::capture_region(&region).ok_or_else(|| AppError::from("截图失败".to_string()))?;

    // 加载参考图像
    let reference_image = watcher::load_reference_image(&ref_path)
        .ok_or_else(|| AppError::from("无法加载参考图像".to_string()))?;

    let similarity = watcher::compare_images(&captured, &reference_image);
    let triggered = similarity.similarity >= threshold;

    Ok(TestMatchResult {
        similarity: similarity.similarity,
        triggered,
        match_position: Some(MatchPosition {
            x: similarity.best_x,
            y: similarity.best_y,
        }),
    })
}

#[tauri::command]
pub async fn recognition_test_color_match(
    _app: tauri::AppHandle,
    state: tauri::State<'_, RecognitionState>,
    card_id: String,
) -> Result<ColorTestResult, AppError> {
    let (probes, match_mode, match_method) = {
        let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        let card = inner
            .settings
            .cards
            .iter()
            .find(|c| c.id == card_id)
            .ok_or_else(|| AppError::from("卡片不存在".to_string()))?;

        if card.trigger_mode != types::RecognitionTriggerMode::ColorWatch {
            return Err(AppError::from("只有识色模式卡片支持识色测试".to_string()));
        }
        if card.color_probes.is_empty() {
            return Err(AppError::from("未配置识色探针".to_string()));
        }
        (
            card.color_probes.clone(),
            card.color_match_mode.clone(),
            card.color_match_method.clone(),
        )
    };

    let mut probe_results: Vec<ColorProbeTestResult> = Vec::with_capacity(probes.len());
    let mut hit_count = 0usize;

    for probe in &probes {
        let region = probe
            .region
            .as_ref()
            .ok_or_else(|| AppError::from("存在未框选区域的探针，请先框选再测试".to_string()))?;
        let captured = match watcher::capture_region(region) {
            Some(img) => img,
            None => return Err(AppError::from("截图失败".to_string()))?,
        };
        // Issue #65：对每个目标分别判定，返回每目标详情
        let target_hits = watcher::probe_hit_targets(&captured, probe, match_method.clone(), false);
        let probe_hit =
            watcher::aggregate_probe_hits_pub(&target_hits, probe.probe_match_mode.clone());
        if probe_hit.matched {
            hit_count += 1;
        }
        let targets: Vec<ColorTargetTestResult> = target_hits
            .iter()
            .map(|h| ColorTargetTestResult {
                matched: h.matched,
                target_color: h.target_color,
                tolerance: h.tolerance,
                sampled_color: h.sampled_color,
                distance: h.distance,
                matching_pixel_count: h.matching_pixel_count,
            })
            .collect();
        probe_results.push(ColorProbeTestResult {
            matched: probe_hit.matched,
            sampled_color: probe_hit.sampled_color,
            distance: probe_hit.distance,
            target_color: probe_hit.target_color,
            tolerance: probe_hit.tolerance,
            matching_pixel_count: probe_hit.matching_pixel_count,
            targets,
        });
    }

    let triggered = match match_mode {
        types::ColorMatchMode::All => hit_count == probes.len(),
        types::ColorMatchMode::Any => hit_count > 0,
    };

    Ok(ColorTestResult {
        triggered,
        hit_count,
        total_count: probes.len(),
        probes: probe_results,
    })
}

/// 读取参考图像并返回 base64 PNG 数据 URL（供前端预览）
#[tauri::command]
pub fn recognition_read_reference_image(
    _app: tauri::AppHandle,
    state: tauri::State<'_, RecognitionState>,
    card_id: String,
) -> Result<String, AppError> {
    let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
    let card = inner
        .settings
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| AppError::from("卡片不存在".to_string()))?;

    let ref_path = card
        .watch_reference_image_path
        .clone()
        .ok_or_else(|| AppError::from("未设置参考图像".to_string()))?;
    if ref_path.is_empty() {
        return Err(AppError::from("参考图像路径为空".to_string()));
    }

    watcher::read_reference_image_as_data_url(&ref_path)
        .ok_or_else(|| AppError::from("无法读取参考图像".to_string()))
}

// ---- 热键 ----

pub(crate) fn restart_hotkey_listeners(
    hotkey_manager: &HotkeyManager,
    settings: &RecognitionSettings,
) -> Result<(), String> {
    let _ = hotkey_manager.clear_scope("recognition");

    if !settings.recognition_enabled {
        return Ok(());
    }

    validate_hotkey_duplicates(settings)?;

    let mut bindings: Vec<(String, crate::hotkey_types::HotkeyAction)> = Vec::new();
    for card in settings.cards.iter().filter(|c| c.enabled) {
        if card.trigger_mode == RecognitionTriggerMode::Hotkey {
            if let Some(key) = card
                .hotkey
                .as_ref()
                .filter(|value| !value.trim().is_empty())
            {
                let card_id = card.id.clone();
                let action: crate::hotkey_types::HotkeyAction =
                    Arc::new(move |app: tauri::AppHandle| {
                        effects::spawn_execute(
                            app,
                            card_id.clone(),
                            effects::TriggerContext::Hotkey,
                        );
                    });
                bindings.push((key.clone(), action));
            }
            continue;
        }

        if card.activation.mode != types::RecognitionActivationMode::Always {
            if let Some(key) = card
                .activation
                .hotkey
                .as_ref()
                .filter(|value| !value.trim().is_empty())
            {
                let card_id = card.id.clone();
                let action: crate::hotkey_types::HotkeyAction =
                    Arc::new(move |app: tauri::AppHandle| {
                        watcher::start_activation_session(app, card_id.clone());
                    });
                bindings.push((key.clone(), action));
            }
        }
    }

    if !bindings.is_empty() {
        hotkey_manager.replace_scope(
            "recognition",
            bindings,
            "识别触发".to_string(),
            ConflictPolicy::AllowHold,
        )?;
    }

    Ok(())
}

fn validate_hotkey_duplicates(settings: &RecognitionSettings) -> Result<(), String> {
    let mut listener_set = std::collections::HashSet::<String>::new();
    for card in settings.cards.iter().filter(|c| c.enabled) {
        let mut keys: Vec<(String, &str)> = Vec::new();
        if card.trigger_mode == RecognitionTriggerMode::Hotkey {
            if let Some(key) = card.hotkey.as_ref().filter(|v| !v.trim().is_empty()) {
                keys.push((key.clone(), "触发快捷键"));
            }
        } else if card.activation.mode != types::RecognitionActivationMode::Always {
            if let Some(key) = card
                .activation
                .hotkey
                .as_ref()
                .filter(|v| !v.trim().is_empty())
            {
                keys.push((key.clone(), "识别激活快捷键"));
            }
        }

        for (key, _label) in keys {
            let normalized = crate::hotkey_types::hotkey_to_string(&key)?;
            listener_set.insert(normalized);
        }
    }

    for card in settings.cards.iter().filter(|c| c.enabled) {
        if let Some(effect) = card.effects.hotkey.as_ref() {
            for step in effect.normalized_steps() {
                if step.hotkey.trim().is_empty() {
                    continue;
                }
                let normalized = crate::hotkey_types::hotkey_to_string(&step.hotkey)?;
                if listener_set.contains(&normalized) {
                    return Err(format!(
                        "卡片 {} 的按键效果不能和监听热键相同：{normalized}",
                        card.name
                    ));
                }
            }
        }
    }
    Ok(())
}

/// 一次解析播放的必要字段：选定的文件路径 + 卡片音量与并发策略。
pub(crate) struct ResolvedPlay {
    pub path: String,
    pub volume: f32,
    pub allow_simultaneous: bool,
}

fn resolve_audio_path(
    inner: &ToolStateInner<RecognitionLogic>,
    card_id: &str,
    now: std::time::Instant,
) -> Result<ResolvedPlay, String> {
    let card = inner
        .settings
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| "卡片不存在".to_string())?
        .clone();
    if !card.enabled {
        return Err("卡片未启用".to_string());
    }
    let effect = card
        .effects
        .audio
        .as_ref()
        .ok_or_else(|| "未启用播放音频效果".to_string())?;
    resolve_audio_effect_path(inner, card_id, effect, now)
}

pub(crate) fn resolve_audio_effect_path(
    inner: &ToolStateInner<RecognitionLogic>,
    card_id: &str,
    effect: &types::RecognitionAudioEffect,
    now: std::time::Instant,
) -> Result<ResolvedPlay, String> {
    if effect.audio_files.is_empty() {
        return Err("未设置音频文件路径".to_string());
    }
    let path = {
        let mut states = inner.logic.play_states.lock().map_err(|e| e.to_string())?;
        let state = states.entry(card_id.to_string()).or_default();
        pick_audio_file(
            &effect.audio_files,
            effect.play_mode,
            &effect.combo_windows,
            effect.combo_window_ms,
            state,
            now,
        )
    };
    Ok(ResolvedPlay {
        path,
        volume: effect.volume,
        allow_simultaneous: effect.allow_simultaneous,
    })
}

// ---- 初始化与关闭 ----

pub fn initialize(
    app: &tauri::AppHandle,
    hotkey_manager: &HotkeyManager,
) -> Result<RecognitionState, String> {
    let settings = settings::read_settings(app)?;
    let _ = restart_hotkey_listeners(hotkey_manager, &settings);

    // 启动音频播放线程
    let (playback_tx, _worker) = player::start_audio_thread();

    // 启动区域监听 watcher
    let _ = watcher::restart_watchers(app, &settings, playback_tx.clone());

    let logic = RecognitionLogic {
        playback_tx,
        play_states: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    };

    Ok(RecognitionState::new(logic, settings))
}

pub fn shutdown(app: &tauri::AppHandle, hotkey_manager: &HotkeyManager) {
    let _ = hotkey_manager.clear_scope("recognition");
    hotkey_manager.clear_all_suppressions();
    let _ = watcher::stop_all_watchers(app);

    // 通知音频线程关闭
    if let Ok(inner) = app.state::<RecognitionState>().lock_inner() {
        let _ = inner.logic.playback_tx.send(player::AudioCommand::Shutdown);
    };
}

// ---- 设置规范化 ----

pub(crate) fn normalize_settings(settings: RecognitionSettings) -> RecognitionSettings {
    let mut cards = settings.cards;

    for card in &mut cards {
        // 确保每张卡片有唯一 ID
        if card.id.is_empty() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            card.id = format!("recognition-{now}");
        }

        // 迁移旧单值 audioFilePath → audio_files 单元素数组
        if card.audio_files.is_empty() {
            if let Some(legacy) = card.legacy_audio_file_path.take() {
                if !legacy.is_empty() {
                    card.audio_files = vec![legacy];
                }
            }
        } else {
            // 已有 audio_files 时清掉兼容字段（避免后续误用）
            card.legacy_audio_file_path = None;
        }

        if card.effects.audio.is_none() && !card.audio_files.is_empty() {
            card.effects.audio = Some(types::RecognitionAudioEffect {
                audio_files: std::mem::take(&mut card.audio_files),
                play_mode: card.play_mode,
                combo_window_ms: card.combo_window_ms,
                combo_windows: std::mem::take(&mut card.combo_windows),
                volume: card.volume,
                allow_simultaneous: card.allow_simultaneous,
            });
        }

        // Issue #65：迁移旧 ColorProbe 单值 targetColor/tolerance → targets 单元素
        for probe in &mut card.color_probes {
            if probe.targets.is_empty() {
                if let Some(tc) = probe.legacy_target_color.take() {
                    let tol = probe
                        .legacy_tolerance
                        .take()
                        .unwrap_or(types::DEFAULT_COLOR_TOLERANCE);
                    probe.targets.push(types::ColorTarget {
                        color: tc,
                        tolerance: tol,
                    });
                }
            } else {
                // 已有 targets 时清掉兼容字段（避免后续误用）
                probe.legacy_target_color = None;
                probe.legacy_tolerance = None;
            }
            // 探针内聚合模式缺省回退为 Any
            if probe.targets.is_empty() {
                // 空探针视为草稿态，保留原样（watcher 启动会跳过）
                // 但 probe_match_mode 仍需有值，serde default 已保证 Any
            }
        }
    }

    RecognitionSettings {
        recognition_enabled: settings.recognition_enabled,
        cards,
    }
}

pub(crate) fn validate_settings(settings: &RecognitionSettings) -> Result<(), String> {
    validate_hotkey_duplicates(settings)?;
    for card in &settings.cards {
        if card.trigger_mode == RecognitionTriggerMode::Hotkey
            && card.hotkey.as_deref().unwrap_or("").trim().is_empty()
        {
            return Err(format!("卡片 {} 必须设置触发快捷键", card.name));
        }
        if card.trigger_mode != RecognitionTriggerMode::Hotkey
            && card.activation.mode != types::RecognitionActivationMode::Always
            && card
                .activation
                .hotkey
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err(format!("卡片 {} 必须设置识别激活快捷键", card.name));
        }

        let mut executable_effect_count = 0;
        if let Some(effect) = &card.effects.audio {
            if effect.audio_files.is_empty() {
                return Err(format!(
                    "卡片 {} 的音频效果至少需要 1 个音频文件",
                    card.name
                ));
            }
            if effect.play_mode != types::PlayMode::Single && effect.audio_files.len() < 2 {
                return Err(format!(
                    "卡片 {} 的连杀/随机播放至少需要 2 个音频文件",
                    card.name
                ));
            }
            executable_effect_count += 1;
        }
        if let Some(effect) = &card.effects.hotkey {
            if effect.normalized_steps().is_empty() {
                return Err(format!("卡片 {} 的按键效果必须设置快捷键", card.name));
            }
            executable_effect_count += 1;
        }
        if let Some(effect) = &card.effects.click {
            match effect.mode {
                types::RecognitionClickMode::CustomRegion => {}
                types::RecognitionClickMode::RecognitionRegion => {
                    if card.trigger_mode == RecognitionTriggerMode::Hotkey {
                        return Err(format!(
                            "卡片 {} 的快捷键触发点击必须使用自定义区域",
                            card.name
                        ));
                    }
                    if card.trigger_mode == RecognitionTriggerMode::ColorWatch {
                        let Some(index) = effect.color_probe_index else {
                            return Err(format!("卡片 {} 的识色点击效果必须选择探针", card.name));
                        };
                        if index >= card.color_probes.len() {
                            return Err(format!("卡片 {} 的识色点击探针不存在", card.name));
                        }
                    }
                }
            }
            executable_effect_count += 1;
        }
        if executable_effect_count == 0 {
            return Err(format!("卡片 {} 至少需要启用一个触发效果", card.name));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn files(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn base_card() -> types::RecognitionCard {
        types::RecognitionCard {
            id: "c1".into(),
            name: "测试卡".into(),
            enabled: true,
            trigger_mode: types::RecognitionTriggerMode::Hotkey,
            hotkey: Some("Ctrl+F1".into()),
            watch_region: None,
            watch_reference_image_path: None,
            watch_match_threshold: 0.75,
            watch_poll_interval_ms: 500,
            activation: types::RecognitionActivation::default(),
            effects: types::RecognitionEffects::default(),
            audio_files: vec![],
            legacy_audio_file_path: None,
            play_mode: types::PlayMode::Single,
            combo_window_ms: 60000,
            combo_windows: vec![],
            volume: 0.8,
            cooldown_ms: 1000,
            allow_simultaneous: false,
            color_probes: vec![],
            color_match_mode: types::ColorMatchMode::All,
            color_match_method: types::ColorMatchMethod::Average,
        }
    }

    #[test]
    fn pick_single_returns_first_file_unchanged() {
        let f = files(&["a.mp3", "b.mp3"]);
        let mut state = PlayState::default();
        let now = Instant::now();
        let path = pick_audio_file(
            &f,
            types::PlayMode::Single,
            &[60000],
            60000,
            &mut state,
            now,
        );
        assert_eq!(path, "a.mp3");
        // Single 不更新连杀状态
        assert_eq!(state.current_index, 0);
        assert!(state.last_trigger_at.is_none());
    }

    #[test]
    fn pick_single_with_single_file_returns_first() {
        let f = files(&["only.mp3"]);
        let mut state = PlayState::default();
        let path = pick_audio_file(
            &f,
            types::PlayMode::Single,
            &[60000],
            60000,
            &mut state,
            Instant::now(),
        );
        assert_eq!(path, "only.mp3");
    }

    #[test]
    fn pick_combo_first_trigger_plays_first_file() {
        let f = files(&["a.mp3", "b.mp3", "c.mp3"]);
        let mut state = PlayState::default();
        let now = Instant::now();
        let path = pick_audio_file(&f, types::PlayMode::Combo, &[60000], 60000, &mut state, now);
        assert_eq!(path, "a.mp3");
        assert_eq!(state.current_index, 0);
        assert_eq!(state.last_trigger_at, Some(now));
    }

    #[test]
    fn pick_combo_within_window_advances_index() {
        let f = files(&["a.mp3", "b.mp3", "c.mp3"]);
        let t0 = Instant::now();
        let mut state = PlayState::default();
        // 第一次触发 → 第 0 首
        pick_audio_file(&f, types::PlayMode::Combo, &[60000], 60000, &mut state, t0);
        // 30s 后（窗口内）第二次 → 第 1 首
        let path = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &[60000],
            60000,
            &mut state,
            t0 + Duration::from_secs(30),
        );
        assert_eq!(path, "b.mp3");
        assert_eq!(state.current_index, 1);
    }

    #[test]
    fn pick_combo_at_last_index_holds_within_window() {
        let f = files(&["a.mp3", "b.mp3", "c.mp3"]);
        let t0 = Instant::now();
        let mut state = PlayState::default();
        state.current_index = 2; // 已在末首
        state.last_trigger_at = Some(t0);
        // 窗口内再触发 → 保持末首（不越界）
        let path = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &[60000],
            60000,
            &mut state,
            t0 + Duration::from_secs(10),
        );
        assert_eq!(path, "c.mp3");
        assert_eq!(state.current_index, 2);
    }

    #[test]
    fn pick_combo_after_window_resets_to_first() {
        let f = files(&["a.mp3", "b.mp3", "c.mp3"]);
        let t0 = Instant::now();
        let mut state = PlayState::default();
        state.current_index = 2; // 已在末首
        state.last_trigger_at = Some(t0);
        // 61s 后（超时）→ 复位第 0 首
        let path = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &[60000],
            60000,
            &mut state,
            t0 + Duration::from_millis(61000),
        );
        assert_eq!(path, "a.mp3");
        assert_eq!(state.current_index, 0);
    }

    #[test]
    fn pick_combo_full_sequence_advances_then_resets() {
        let f = files(&["a.mp3", "b.mp3", "c.mp3"]);
        let mut state = PlayState::default();
        let t0 = Instant::now();
        let p1 = pick_audio_file(&f, types::PlayMode::Combo, &[60000], 60000, &mut state, t0);
        let p2 = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &[60000],
            60000,
            &mut state,
            t0 + Duration::from_secs(10),
        );
        let p3 = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &[60000],
            60000,
            &mut state,
            t0 + Duration::from_secs(20),
        );
        let p4 = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &[60000],
            60000,
            &mut state,
            t0 + Duration::from_secs(30),
        );
        assert_eq!(p1, "a.mp3");
        assert_eq!(p2, "b.mp3");
        assert_eq!(p3, "c.mp3");
        assert_eq!(p4, "c.mp3"); // 末首后窗口内保持
                                 // 超时复位
        let p5 = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &[60000],
            60000,
            &mut state,
            t0 + Duration::from_secs(100),
        );
        assert_eq!(p5, "a.mp3");
    }

    // Issue #62: per-segment 连杀窗口。播完第 i 段后用第 i 段窗口判断是否进 i+1 段。
    #[test]
    fn pick_combo_per_segment_window_advances_by_current_index_window() {
        // 三段文件，每段自己的窗口：A=500ms, B=800ms, C=1000ms
        let f = files(&["a.mp3", "b.mp3", "c.mp3"]);
        let windows = [500, 800, 1000];
        let mut state = PlayState::default();
        let t0 = Instant::now();
        // 触发 A（index 0），last_trigger_at = t0
        let p1 = pick_audio_file(&f, types::PlayMode::Combo, &windows, 60000, &mut state, t0);
        assert_eq!(p1, "a.mp3");
        // 400ms 后（< window[0]=500）→ 进 B
        let p2 = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &windows,
            60000,
            &mut state,
            t0 + Duration::from_millis(400),
        );
        assert_eq!(p2, "b.mp3");
        // 700ms 后（< window[1]=800）→ 进 C
        let p3 = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &windows,
            60000,
            &mut state,
            t0 + Duration::from_millis(400 + 700),
        );
        assert_eq!(p3, "c.mp3");
        // 900ms 后（< window[2]=1000）→ 保持末首 C
        let p4 = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &windows,
            60000,
            &mut state,
            t0 + Duration::from_millis(400 + 700 + 900),
        );
        assert_eq!(p4, "c.mp3");
    }

    #[test]
    fn pick_combo_per_segment_window_resets_when_current_window_exceeded() {
        // 在 B（index 1，window[1]=800ms），超 800ms → 复位 A
        let f = files(&["a.mp3", "b.mp3", "c.mp3"]);
        let windows = [500, 800, 1000];
        let t0 = Instant::now();
        let mut state = PlayState::default();
        pick_audio_file(&f, types::PlayMode::Combo, &windows, 60000, &mut state, t0); // → A
        pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &windows,
            60000,
            &mut state,
            t0 + Duration::from_millis(400),
        ); // → B
        assert_eq!(state.current_index, 1);
        // 距上次触发 900ms（> window[1]=800）→ 复位 A
        let p = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &windows,
            60000,
            &mut state,
            t0 + Duration::from_millis(400 + 900),
        );
        assert_eq!(p, "a.mp3");
        assert_eq!(state.current_index, 0);
    }

    #[test]
    fn pick_combo_per_segment_window_falls_back_to_default_for_missing_index() {
        // windows 长度 < 文件数：缺省 index 回落到 combo_window_ms（向后兼容旧数据）
        let f = files(&["a.mp3", "b.mp3", "c.mp3"]);
        let windows = [500]; // 只配了 A 的窗口，B/C 缺省 → 用 60000
        let t0 = Instant::now();
        let mut state = PlayState::default();
        pick_audio_file(&f, types::PlayMode::Combo, &windows, 60000, &mut state, t0); // → A
                                                                                      // 400ms（< 500）→ 进 B
        let p2 = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &windows,
            60000,
            &mut state,
            t0 + Duration::from_millis(400),
        );
        assert_eq!(p2, "b.mp3");
        // B 的窗口缺省为 60000，10s 后仍在窗口内 → 进 C
        let p3 = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &windows,
            60000,
            &mut state,
            t0 + Duration::from_millis(400 + 10000),
        );
        assert_eq!(p3, "c.mp3");
    }

    #[test]
    fn pick_random_single_file_returns_only_file() {
        let f = files(&["only.mp3"]);
        let mut state = PlayState::default();
        let path = pick_audio_file(
            &f,
            types::PlayMode::Random,
            &[60000],
            60000,
            &mut state,
            Instant::now(),
        );
        assert_eq!(path, "only.mp3");
    }

    #[test]
    fn pick_random_does_not_repeat_last_index() {
        let f = files(&["a.mp3", "b.mp3"]);
        let mut state = PlayState::default();
        // 上一次选了 0，下次必须不选 0 → 只能是 1
        state.last_random_index = Some(0);
        let path = pick_audio_file(
            &f,
            types::PlayMode::Random,
            &[60000],
            60000,
            &mut state,
            Instant::now(),
        );
        assert_eq!(path, "b.mp3");
        assert_eq!(state.last_random_index, Some(1));
    }

    #[test]
    fn pick_random_first_call_without_last_picks_any() {
        let f = files(&["a.mp3", "b.mp3"]);
        let mut state = PlayState::default();
        let path = pick_audio_file(
            &f,
            types::PlayMode::Random,
            &[60000],
            60000,
            &mut state,
            Instant::now(),
        );
        assert!(path == "a.mp3" || path == "b.mp3");
        assert!(state.last_random_index.is_some());
        assert!(state.last_trigger_at.is_some());
    }

    #[test]
    fn pick_random_three_files_never_consecutively_same() {
        let f = files(&["a.mp3", "b.mp3", "c.mp3"]);
        let mut state = PlayState::default();
        let mut last = None;
        for _ in 0..20 {
            let path = pick_audio_file(
                &f,
                types::PlayMode::Random,
                &[60000],
                60000,
                &mut state,
                Instant::now(),
            );
            if let Some(prev) = last {
                assert_ne!(path, prev, "随机不应连续两次相同");
            }
            last = Some(path);
        }
    }

    #[test]
    fn pick_empty_files_returns_empty_string() {
        let f: Vec<String> = vec![];
        let mut state = PlayState::default();
        let path = pick_audio_file(
            &f,
            types::PlayMode::Combo,
            &[60000],
            60000,
            &mut state,
            Instant::now(),
        );
        assert_eq!(path, "");
    }

    #[test]
    fn normalize_migrates_legacy_audio_file_path_to_audio_files() {
        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![types::RecognitionCard {
                id: "c1".into(),
                name: "旧卡".into(),
                enabled: true,
                trigger_mode: types::RecognitionTriggerMode::Hotkey,
                hotkey: Some("Ctrl+F1".into()),
                watch_region: None,
                watch_reference_image_path: None,
                watch_match_threshold: 0.75,
                watch_poll_interval_ms: 500,
                activation: types::RecognitionActivation::default(),
                effects: types::RecognitionEffects::default(),
                audio_files: vec![],
                legacy_audio_file_path: Some("old.mp3".into()),
                play_mode: types::PlayMode::Single,
                combo_window_ms: 60000,
                combo_windows: vec![],
                volume: 0.8,
                cooldown_ms: 1000,
                allow_simultaneous: false,
                color_probes: vec![],
                color_match_mode: types::ColorMatchMode::All,
                color_match_method: types::ColorMatchMethod::Average,
            }],
        };
        let normalized = normalize_settings(settings);
        assert_eq!(
            normalized.cards[0]
                .effects
                .audio
                .as_ref()
                .unwrap()
                .audio_files,
            vec!["old.mp3".to_string()]
        );
        assert!(normalized.cards[0].audio_files.is_empty());
        assert!(normalized.cards[0].legacy_audio_file_path.is_none());
    }

    #[test]
    fn normalize_keeps_existing_audio_files_and_clears_legacy() {
        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![types::RecognitionCard {
                id: "c1".into(),
                name: "新卡".into(),
                enabled: true,
                trigger_mode: types::RecognitionTriggerMode::Hotkey,
                hotkey: None,
                watch_region: None,
                watch_reference_image_path: None,
                watch_match_threshold: 0.75,
                watch_poll_interval_ms: 500,
                activation: types::RecognitionActivation::default(),
                effects: types::RecognitionEffects::default(),
                audio_files: vec!["a.mp3".into(), "b.mp3".into()],
                legacy_audio_file_path: Some("ignored.mp3".into()),
                play_mode: types::PlayMode::Combo,
                combo_window_ms: 60000,
                combo_windows: vec![],
                volume: 0.8,
                cooldown_ms: 1000,
                allow_simultaneous: false,
                color_probes: vec![],
                color_match_mode: types::ColorMatchMode::All,
                color_match_method: types::ColorMatchMethod::Average,
            }],
        };
        let normalized = normalize_settings(settings);
        assert_eq!(
            normalized.cards[0]
                .effects
                .audio
                .as_ref()
                .unwrap()
                .audio_files,
            vec!["a.mp3".to_string(), "b.mp3".to_string()]
        );
        assert!(normalized.cards[0].audio_files.is_empty());
        assert!(normalized.cards[0].legacy_audio_file_path.is_none());
    }

    #[test]
    fn validate_rejects_empty_audio_effect() {
        let mut card = base_card();
        card.effects.audio = Some(types::RecognitionAudioEffect {
            audio_files: vec![],
            play_mode: types::PlayMode::Single,
            combo_window_ms: 60000,
            combo_windows: vec![],
            volume: 0.8,
            allow_simultaneous: false,
        });
        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![card],
        };
        assert!(validate_settings(&settings)
            .unwrap_err()
            .contains("音频效果至少需要 1 个音频文件"));
    }

    #[test]
    fn validate_rejects_empty_hotkey_effect() {
        let mut card = base_card();
        card.effects.hotkey = Some(types::RecognitionHotkeyEffect {
            hotkey: " ".into(),
            steps: Vec::new(),
        });
        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![card],
        };
        assert!(validate_settings(&settings)
            .unwrap_err()
            .contains("按键效果必须设置快捷键"));
    }

    #[test]
    fn validate_accepts_hotkey_effect_steps_without_legacy_hotkey() {
        let mut card = base_card();
        card.effects.hotkey = Some(types::RecognitionHotkeyEffect {
            hotkey: String::new(),
            steps: vec![types::RecognitionHotkeyEffectStep {
                hotkey: "F2".into(),
                delay_ms: 25,
            }],
        });
        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![card],
        };

        validate_settings(&settings).unwrap();
    }

    #[test]
    fn validate_accepts_repeated_output_steps_in_same_card() {
        let mut card = base_card();
        card.effects.hotkey = Some(types::RecognitionHotkeyEffect {
            hotkey: String::new(),
            steps: vec![
                types::RecognitionHotkeyEffectStep {
                    hotkey: "Q".into(),
                    delay_ms: 0,
                },
                types::RecognitionHotkeyEffectStep {
                    hotkey: "Q".into(),
                    delay_ms: 25,
                },
                types::RecognitionHotkeyEffectStep {
                    hotkey: "W".into(),
                    delay_ms: 50,
                },
            ],
        });
        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![card],
        };

        validate_settings(&settings).unwrap();
    }

    #[test]
    fn validate_accepts_repeated_output_steps_across_cards() {
        let mut card_a = base_card();
        card_a.id = "c1".into();
        card_a.name = "卡片 A".into();
        card_a.hotkey = Some("Ctrl+F1".into());
        card_a.effects.hotkey = Some(types::RecognitionHotkeyEffect {
            hotkey: String::new(),
            steps: vec![types::RecognitionHotkeyEffectStep {
                hotkey: "Q".into(),
                delay_ms: 0,
            }],
        });

        let mut card_b = base_card();
        card_b.id = "c2".into();
        card_b.name = "卡片 B".into();
        card_b.hotkey = Some("Ctrl+F2".into());
        card_b.effects.hotkey = Some(types::RecognitionHotkeyEffect {
            hotkey: String::new(),
            steps: vec![types::RecognitionHotkeyEffectStep {
                hotkey: "Q".into(),
                delay_ms: 0,
            }],
        });

        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![card_a, card_b],
        };

        validate_settings(&settings).unwrap();
    }

    #[test]
    fn validate_rejects_output_step_equal_to_listener_hotkey() {
        let mut card = base_card();
        card.effects.hotkey = Some(types::RecognitionHotkeyEffect {
            hotkey: String::new(),
            steps: vec![types::RecognitionHotkeyEffectStep {
                hotkey: "Ctrl+F1".into(),
                delay_ms: 0,
            }],
        });
        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![card],
        };

        assert!(validate_settings(&settings)
            .unwrap_err()
            .contains("按键效果不能和监听热键相同"));
    }

    #[test]
    fn validate_rejects_output_step_equal_to_activation_hotkey() {
        let mut card = base_card();
        card.trigger_mode = types::RecognitionTriggerMode::RegionWatch;
        card.hotkey = None;
        card.activation = types::RecognitionActivation {
            mode: types::RecognitionActivationMode::TimedHotkey,
            hotkey: Some("Alt+F1".into()),
            duration_ms: 3000,
            trigger_count: 3,
        };
        card.effects.hotkey = Some(types::RecognitionHotkeyEffect {
            hotkey: String::new(),
            steps: vec![types::RecognitionHotkeyEffectStep {
                hotkey: "Alt+F1".into(),
                delay_ms: 0,
            }],
        });
        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![card],
        };

        assert!(validate_settings(&settings)
            .unwrap_err()
            .contains("按键效果不能和监听热键相同"));
    }

    #[test]
    fn validate_accepts_duplicate_listener_hotkeys_across_cards() {
        let mut card_a = base_card();
        card_a.id = "c1".into();
        card_a.name = "卡片 A".into();
        card_a.hotkey = Some("Ctrl+F1".into());
        card_a.effects.hotkey = Some(types::RecognitionHotkeyEffect {
            hotkey: String::new(),
            steps: vec![types::RecognitionHotkeyEffectStep {
                hotkey: "F2".into(),
                delay_ms: 0,
            }],
        });

        let mut card_b = base_card();
        card_b.id = "c2".into();
        card_b.name = "卡片 B".into();
        card_b.hotkey = Some("Ctrl+F1".into());
        card_b.effects.hotkey = Some(types::RecognitionHotkeyEffect {
            hotkey: String::new(),
            steps: vec![types::RecognitionHotkeyEffectStep {
                hotkey: "F3".into(),
                delay_ms: 0,
            }],
        });

        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![card_a, card_b],
        };

        validate_settings(&settings).unwrap();
    }

    #[test]
    fn validate_accepts_missing_custom_click_region_as_draft() {
        let mut card = base_card();
        card.effects.click = Some(types::RecognitionClickEffect {
            mode: types::RecognitionClickMode::CustomRegion,
            custom_region: None,
            color_probe_index: None,
        });
        let settings = RecognitionSettings {
            recognition_enabled: true,
            cards: vec![card],
        };
        validate_settings(&settings).unwrap();
    }
}
