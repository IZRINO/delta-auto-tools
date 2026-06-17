use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::app_error::AppError;
use crate::hotkey_types::ConflictPolicy;
use crate::hotkeys::HotkeyManager;
use crate::tool_base::{ToolLogic, ToolState, ToolStateInner};

mod types;
mod events;
mod settings;
mod player;
mod watcher;

pub use self::types::{
    AudioBootstrap, AudioSettings, AudioTriggerMode,
};
pub use events::*;

use crate::morse::types::RegionRect;
use crate::overlay_utils::{
    destroy_stale_windows, destroy_window, encoded_query_value, safe_label_component,
};

const AUDIO_OVERLAY_LABEL: &str = "audio-overlay";

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
    /// 该 probe 是否命中
    pub matched: bool,
    /// 截取区域平均 RGB
    pub sampled_color: [u8; 3],
    /// 与目标颜色的距离
    pub distance: f32,
    /// 目标颜色
    pub target_color: [u8; 3],
    /// 容差
    pub tolerance: u8,
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

pub struct AudioLogic {
    /// 音频播放线程的命令发送端
    pub playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
}

pub type AudioState = ToolState<AudioLogic>;

impl ToolLogic for AudioLogic {
    type Settings = AudioSettings;
    type Bootstrap = AudioBootstrap;

    const NAME: &'static str = "音频";

    fn load_settings(app: &tauri::AppHandle) -> Result<Self::Settings, String> {
        settings::read_settings(app)
    }

    fn save_settings(app: &tauri::AppHandle, settings: &Self::Settings) -> Result<(), String> {
        settings::write_settings(app, settings)
    }

    fn build_bootstrap(inner: &ToolStateInner<Self>) -> Self::Bootstrap {
        AudioBootstrap {
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
pub fn audio_get_bootstrap(
    state: tauri::State<'_, AudioState>,
) -> Result<AudioBootstrap, AppError> {
    crate::tool_base::get_bootstrap(state).map_err(AppError::from)
}

#[tauri::command]
pub fn audio_save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    settings_value: AudioSettings,
) -> Result<AudioBootstrap, AppError> {
    let normalized = normalize_settings(settings_value);
    let previous_settings = {
        let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        inner.settings.clone()
    };

    // 先保存到磁盘，失败时直接返回错误，不重启 listeners
    settings::write_settings(&app, &normalized).map_err(|e| AppError::from(e))?;

    // 再更新内存状态
    let mut inner = state.lock_inner().map_err(|e| AppError::from(e))?;
    inner.settings = normalized.clone();

    // 然后重启热键和 watcher
    let hotkey_manager = app.state::<HotkeyManager>();
    if let Err(e) = restart_hotkey_listeners(&hotkey_manager, &normalized) {
        // 热键注册失败：回滚到之前的设置
        let _ = settings::write_settings(&app, &previous_settings);
        inner.settings = previous_settings;
        inner.hotkey_error = Some(e.clone());
        return Err(AppError::from(e));
    }
    let _ = watcher::restart_watchers(&app, &normalized, inner.logic.playback_tx.clone());

    // 总开关关闭时停止所有 watcher
    if !inner.settings.audio_enabled {
        let _ = watcher::stop_all_watchers(&app);
    }

    inner.hotkey_error = None;
    let bootstrap = AudioLogic::build_bootstrap(&inner);
    AudioLogic::emit_state(&app, &bootstrap);
    Ok(bootstrap)
}

#[tauri::command]
pub async fn audio_begin_region_selection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    card_id: String,
) -> Result<(), AppError> {
    let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
    if !inner.settings.audio_enabled {
        return Err(AppError::from("音频功能未启用".to_string()));
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
    drop(inner);

    // 创建 overlay 窗口
    let label = format!("{}-{}", AUDIO_OVERLAY_LABEL, safe_label_component(&card_id));
    let mut active_labels = std::collections::HashSet::new();
    active_labels.insert(label.clone());
    destroy_stale_windows(&app, AUDIO_OVERLAY_LABEL, &active_labels);
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

    let window = tauri::WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::App(
            format!(
                "index.html?mode=audio-overlay&audio_card={}",
                encoded_query_value(&card_id)
            )
                .into(),
        ),
    )
        .title("音频区域选择")
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
        .map_err(|error| AppError::from(format!("创建音频区域选择窗口失败: {error}")))?;

    // 窗口创建后强制最大化，防止 DPI 缩放导致尺寸不足
    let _ = window.maximize();

    Ok(())
}

#[tauri::command]
pub async fn audio_overlay_submit_selection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    card_id: String,
    region: RegionRect,
) -> Result<(), AppError> {
    // 关闭 overlay 窗口
    let overlay_label = format!("{}-{}", AUDIO_OVERLAY_LABEL, safe_label_component(&card_id));
    destroy_window(&app, &overlay_label);

    // 更新卡片区域
    let (settings_snapshot, bootstrap, playback_tx) = {
        let mut inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        let Some(card) = inner.settings.cards.iter_mut().find(|c| c.id == card_id) else {
            return Err(AppError::from("卡片不存在".to_string()));
        };
        card.watch_region = Some(region);
        settings::write_settings(&app, &inner.settings).map_err(|e| AppError::from(e))?;
        (inner.settings.clone(), AudioLogic::build_bootstrap(&inner), inner.logic.playback_tx.clone())
    };

    watcher::restart_watchers(&app, &settings_snapshot, playback_tx).map_err(AppError::from)?;
    AudioLogic::emit_state(&app, &bootstrap);
    Ok(())
}

#[tauri::command]
pub async fn audio_overlay_cancel_selection(
    app: tauri::AppHandle,
    card_id: String,
) -> Result<(), AppError> {
    let overlay_label = format!("{}-{}", AUDIO_OVERLAY_LABEL, safe_label_component(&card_id));
    destroy_window(&app, &overlay_label);
    Ok(())
}

#[tauri::command]
pub async fn audio_test_play(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    card_id: String,
) -> Result<(), AppError> {
    let (path, volume, allow_simultaneous, playback_tx) = {
        let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        let card = inner
            .settings
            .cards
            .iter()
            .find(|c| c.id == card_id)
            .ok_or_else(|| AppError::from("卡片不存在".to_string()))?;

        if card.audio_file_path.is_empty() {
            return Err(AppError::from("未设置音频文件路径".to_string()));
        }

        (
            card.audio_file_path.clone(),
            card.volume,
            card.allow_simultaneous,
            inner.logic.playback_tx.clone(),
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
pub async fn audio_test_match(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
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
            types::AudioTriggerMode::RegionWatch => {}
            types::AudioTriggerMode::ColorWatch => {
                return Err(AppError::from("识色模式请使用 audio_test_color_match 命令".to_string()));
            }
            types::AudioTriggerMode::Hotkey => {
                return Err(AppError::from("快捷键模式不支持匹配测试".to_string()));
            }
        }
        let region = card.watch_region.clone().ok_or_else(|| AppError::from("未设置监听区域".to_string()))?;
        let ref_path = card.watch_reference_image_path.clone().ok_or_else(|| AppError::from("未设置参考图像".to_string()))?;
        if ref_path.is_empty() {
            return Err(AppError::from("参考图像路径为空".to_string()));
        }
        let threshold = card.watch_match_threshold;
        (region, ref_path, threshold)
    };

    // 截图
    let captured = watcher::capture_region(&region)
        .ok_or_else(|| AppError::from("截图失败".to_string()))?;

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
pub async fn audio_test_color_match(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    card_id: String,
) -> Result<ColorTestResult, AppError> {
    let (probes, match_mode) = {
        let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        let card = inner
            .settings
            .cards
            .iter()
            .find(|c| c.id == card_id)
            .ok_or_else(|| AppError::from("卡片不存在".to_string()))?;

        if card.trigger_mode != types::AudioTriggerMode::ColorWatch {
            return Err(AppError::from("只有识色模式卡片支持识色测试".to_string()));
        }
        if card.color_probes.is_empty() {
            return Err(AppError::from("未配置识色探针".to_string()));
        }
        (card.color_probes.clone(), card.color_match_mode.clone())
    };

    let mut probe_results: Vec<ColorProbeTestResult> = Vec::with_capacity(probes.len());
    let mut hit_count = 0usize;

    for probe in &probes {
        let captured = match watcher::capture_region(&probe.region) {
            Some(img) => img,
            None => return Err(AppError::from("截图失败".to_string())),
        };
        let sampled = watcher::average_region_rgb(&captured);
        let dist = watcher::color_distance(sampled, probe.target_color);
        let matched = dist <= probe.tolerance as f32;
        if matched {
            hit_count += 1;
        }
        probe_results.push(ColorProbeTestResult {
            matched,
            sampled_color: sampled,
            distance: dist,
            target_color: probe.target_color,
            tolerance: probe.tolerance,
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
pub fn audio_read_reference_image(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    card_id: String,
) -> Result<String, AppError> {
    let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
    let card = inner
        .settings
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| AppError::from("卡片不存在".to_string()))?;

    let ref_path = card.watch_reference_image_path.clone()
        .ok_or_else(|| AppError::from("未设置参考图像".to_string()))?;
    if ref_path.is_empty() {
        return Err(AppError::from("参考图像路径为空".to_string()));
    }

    watcher::read_reference_image_as_data_url(&ref_path)
        .ok_or_else(|| AppError::from("无法读取参考图像".to_string()))
}

// ---- 热键 ----

fn restart_hotkey_listeners(hotkey_manager: &HotkeyManager, settings: &AudioSettings) -> Result<(), String> {
    let _ = hotkey_manager.clear_scope("audio");

    if !settings.audio_enabled {
        return Ok(());
    }

    let bindings: Vec<(String, crate::hotkey_types::HotkeyAction)> = settings
        .cards
        .iter()
        .filter(|c| c.enabled && c.trigger_mode == AudioTriggerMode::Hotkey)
        .filter_map(|c| {
            c.hotkey.as_ref().map(|key| {
                let card_id = c.id.clone();
                let action: crate::hotkey_types::HotkeyAction =
                    Arc::new(move |app: tauri::AppHandle| {
                        let card_id = card_id.clone();
                        if let Err(error) = trigger_audio_play(&app, &card_id) {
                            let _ = app.emit_to("main", HOTKEY_ERROR, error);
                        }
                    });
                (key.clone(), action)
            })
        })
        .collect();

    if !bindings.is_empty() {
        hotkey_manager.replace_scope(
            "audio",
            bindings,
            "音频".to_string(),
            ConflictPolicy::AllowHold,
        )?;
    }

    Ok(())
}

fn trigger_audio_play(app: &tauri::AppHandle, card_id: &str) -> Result<(), String> {
    let state = app.state::<AudioState>();
    let inner = state.lock_inner()?;
    if !inner.settings.audio_enabled {
        eprintln!("[音频] 触发播放跳过：音频功能未启用");
        return Ok(());
    }

    let card = inner
        .settings
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| "卡片不存在".to_string())?;

    if !card.enabled || card.audio_file_path.is_empty() {
        eprintln!("[音频] 触发播放跳过：卡片未启用或音频路径为空 (card_id={card_id})");
        return Ok(());
    }

    let path = card.audio_file_path.clone();
    let volume = card.volume;
    let allow_simultaneous = card.allow_simultaneous;
    let playback_tx = inner.logic.playback_tx.clone();
    drop(inner);

    eprintln!("[音频] 快捷键触发播放: card_id={card_id}, path={path}, volume={volume}, simultaneous={allow_simultaneous}");

    // 通过协调器播放音频
    let _ = playback_tx.send(player::AudioCommand::Play {
        path,
        volume,
        exclusive: !allow_simultaneous,
    });

    let _ = app.emit(HOTKEY_TRIGGERED, card_id);
    Ok(())
}

// ---- 初始化与关闭 ----

pub fn initialize(
    app: &tauri::AppHandle,
    hotkey_manager: &HotkeyManager,
) -> Result<AudioState, String> {
    let settings = settings::read_settings(app)?;
    let _ = restart_hotkey_listeners(hotkey_manager, &settings);

    // 启动音频播放线程
    let (playback_tx, _worker) = player::start_audio_thread();

    // 启动区域监听 watcher
    let _ = watcher::restart_watchers(app, &settings, playback_tx.clone());

    let logic = AudioLogic { playback_tx };

    Ok(AudioState::new(logic, settings))
}

pub fn shutdown(app: &tauri::AppHandle, hotkey_manager: &HotkeyManager) {
    let _ = hotkey_manager.clear_scope("audio");
    hotkey_manager.clear_all_suppressions();
    let _ = watcher::stop_all_watchers(app);

    // 通知音频线程关闭
    if let Ok(inner) = app.state::<AudioState>().lock_inner() {
        let _ = inner.logic.playback_tx.send(player::AudioCommand::Shutdown);
    };
}

// ---- 设置规范化 ----

fn normalize_settings(settings: AudioSettings) -> AudioSettings {
    let mut cards = settings.cards;

    // 确保每张卡片有唯一 ID
    for card in &mut cards {
        if card.id.is_empty() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            card.id = format!("audio-{now}");
        }
    }

    AudioSettings {
        audio_enabled: settings.audio_enabled,
        cards,
    }
}
