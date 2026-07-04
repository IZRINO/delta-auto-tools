//! Watcher 生命周期管理（restart / stop / run 循环）

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tokio::time::{interval, MissedTickBehavior};

use crate::global_state::GlobalState;
use crate::recognition::effects::{self, ColorProbeMatch, TriggerContext};
use crate::recognition::events::{HOTKEY_ERROR, REGION_MATCHED};
use crate::recognition::player;
use crate::recognition::types::{
    RecognitionActivationMode, RecognitionCard, RecognitionSettings, RecognitionTriggerMode,
};
use tauri::{AppHandle, Emitter, Manager};

use super::capture;
use super::matching;

/// 全局 watcher 状态：卡片 ID -> 取消标记
static WATCHER_CANCEL_MAP: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
static ACTIVATION_CANCEL_MAP: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

/// 启动/重启所有区域监听 watcher
pub fn restart_watchers(
    app: &AppHandle,
    settings: &RecognitionSettings,
    playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
) -> Result<(), String> {
    if !settings.recognition_enabled {
        return stop_all_watchers(app);
    }

    let mut cancel_map = WATCHER_CANCEL_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "识别监听状态已损坏".to_string())?;

    // 先取消所有现有 watcher
    for (_, cancel) in cancel_map.drain() {
        cancel.store(true, Ordering::SeqCst);
    }

    // A-M1 修复：不在启动时快照 recognition_enabled，循环内实时读取 RecognitionState
    // 这样当用户在 watcher 运行期间关闭识别触发模块开关时，watcher 能立即感知并停止

    // 为每张区域监听 / 识色卡片启动 watcher
    for card in &settings.cards {
        if !card.enabled {
            continue;
        }
        if card.activation.mode != RecognitionActivationMode::Always || !card.effects.has_any() {
            continue;
        }
        // 按触发模式校验必要字段，缺字段则跳过
        match card.trigger_mode {
            RecognitionTriggerMode::RegionWatch => {
                let Some(ref_path) = &card.watch_reference_image_path else {
                    continue;
                };
                if ref_path.is_empty() {
                    continue;
                }
                if card.watch_region.is_none() {
                    continue;
                }
            }
            RecognitionTriggerMode::ColorWatch => {
                if card.color_probes.is_empty() {
                    continue;
                }
                // 含未框选（region=None）探针的卡片视为未就绪草稿，不启动 watcher
                if card.color_probes.iter().any(|p| p.region.is_none()) {
                    continue;
                }
            }
            RecognitionTriggerMode::Hotkey => continue,
        }

        let cancel = Arc::new(AtomicBool::new(false));
        let app_clone = app.clone();
        let card_id = card.id.clone();
        let cooldown_ms = card.cooldown_ms;
        let poll_interval_ms = card.watch_poll_interval_ms;
        let playback_tx_clone = playback_tx.clone();
        let cancel_clone = Arc::clone(&cancel);

        cancel_map.insert(card_id.clone(), cancel);

        match card.trigger_mode {
            RecognitionTriggerMode::RegionWatch => {
                let Some(region) = &card.watch_region else {
                    continue;
                };
                let Some(ref_path) = &card.watch_reference_image_path else {
                    continue;
                };
                let threshold = card.watch_match_threshold;
                let region_clone = region.clone();
                let ref_path_clone = ref_path.clone();
                tauri::async_runtime::spawn(async move {
                    run_region_watcher(
                        app_clone,
                        card_id,
                        region_clone,
                        ref_path_clone,
                        playback_tx_clone,
                        cooldown_ms,
                        threshold,
                        poll_interval_ms,
                        cancel_clone,
                    )
                    .await;
                });
            }
            RecognitionTriggerMode::ColorWatch => {
                let probes = card.color_probes.clone();
                let match_mode = card.color_match_mode.clone();
                let match_method = card.color_match_method.clone();
                tauri::async_runtime::spawn(async move {
                    run_color_watcher(
                        app_clone,
                        card_id,
                        probes,
                        match_mode,
                        match_method,
                        playback_tx_clone,
                        cooldown_ms,
                        poll_interval_ms,
                        cancel_clone,
                    )
                    .await;
                });
            }
            RecognitionTriggerMode::Hotkey => {}
        }
    }

    Ok(())
}

/// 停止所有区域监听 watcher
pub fn stop_all_watchers(_app: &AppHandle) -> Result<(), String> {
    let mut cancel_map = WATCHER_CANCEL_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "识别监听状态已损坏".to_string())?;
    for (_, cancel) in cancel_map.drain() {
        cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

pub fn start_activation_session(app: AppHandle, card_id: String) {
    let cancel = Arc::new(AtomicBool::new(false));
    if let Ok(mut map) = ACTIVATION_CANCEL_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        if let Some(previous) = map.insert(card_id.clone(), Arc::clone(&cancel)) {
            previous.store(true, Ordering::SeqCst);
        }
    }

    tauri::async_runtime::spawn(async move {
        run_activation_session(app.clone(), card_id.clone(), Arc::clone(&cancel)).await;
        if let Ok(mut map) = ACTIVATION_CANCEL_MAP
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
        {
            if map
                .get(&card_id)
                .map(|current| Arc::ptr_eq(current, &cancel))
                .unwrap_or(false)
            {
                map.remove(&card_id);
            }
        }
    });
}

/// 读取全局总开关当前状态。`GlobalState` 缺失时视为已启用，不阻断 watcher。
fn global_enabled(app: &AppHandle) -> bool {
    app.try_state::<GlobalState>()
        .map(|state| state.enabled())
        .unwrap_or(true)
}

/// 读取 RecognitionState 中识别触发模块开关状态。`RecognitionState` 缺失时视为已启用。
fn recognition_module_enabled(app: &AppHandle) -> bool {
    use crate::recognition::RecognitionState;
    app.try_state::<RecognitionState>()
        .and_then(|state| {
            state
                .lock_inner()
                .ok()
                .map(|inner| inner.settings.recognition_enabled)
        })
        .unwrap_or(true)
}

/// watcher 每轮 tick 的执行门：全局总开关与识别触发模块开关均开启时才执行。
///
/// - `global_enabled`：实时读取 `GlobalState`。全局总开关切换不会触发 `restart_watchers`，
///   故必须在 watcher 循环内实时检查。
/// - `recognition_module_enabled`：实时读取 `RecognitionState.settings.recognition_enabled`。
///   A-M1 修复：之前用启动快照，关开关后 watcher 仍可能继续。
///   现在改为循环内重读，保证关闭后 watcher 立即停止截图与匹配。
pub(crate) fn watcher_should_run(global_on: bool, recognition_on: bool) -> bool {
    global_on && recognition_on
}

async fn run_activation_session(app: AppHandle, card_id: String, cancel: Arc<AtomicBool>) {
    let Some(card) = card_snapshot(&app, &card_id) else {
        return;
    };
    let duration = Duration::from_millis(card.activation.duration_ms.max(100) as u64);
    let interval_ms = card.watch_poll_interval_ms.max(100);
    let deadline = Instant::now() + duration;

    loop {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        if watcher_should_run(global_enabled(&app), recognition_module_enabled(&app)) {
            let matched = match card.trigger_mode {
                RecognitionTriggerMode::RegionWatch => run_region_once(&app, &card).await,
                RecognitionTriggerMode::ColorWatch => run_color_once(&app, &card).await,
                RecognitionTriggerMode::Hotkey => false,
            };
            if matched || card.activation.mode == RecognitionActivationMode::OnceHotkey {
                break;
            }
        }
        if card.activation.mode != RecognitionActivationMode::TimedHotkey
            || Instant::now() >= deadline
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(interval_ms as u64)).await;
    }
}

fn card_snapshot(app: &AppHandle, card_id: &str) -> Option<RecognitionCard> {
    use crate::recognition::RecognitionState;
    let state = app.try_state::<RecognitionState>()?;
    let inner = state.lock_inner().ok()?;
    inner
        .settings
        .cards
        .iter()
        .find(|card| card.id == card_id && card.enabled)
        .cloned()
}

async fn run_region_once(app: &AppHandle, card: &RecognitionCard) -> bool {
    let Some(region) = card.watch_region.as_ref() else {
        return false;
    };
    let Some(reference_image_path) = card.watch_reference_image_path.as_ref() else {
        return false;
    };
    let Some(captured) = capture::capture_region(region) else {
        return false;
    };
    let Some(reference_image) = capture::load_reference_image(reference_image_path) else {
        return false;
    };

    let result = matching::compare_images(&captured, &reference_image);
    if result.similarity < card.watch_match_threshold {
        return false;
    }

    let center_x = region.x + result.best_x as i32 + reference_image.width() as i32 / 2;
    let center_y = region.y + result.best_y as i32 + reference_image.height() as i32 / 2;
    let _ = app.emit(REGION_MATCHED, &card.id);
    if let Err(error) = effects::execute(
        app.clone(),
        card.id.clone(),
        TriggerContext::Region { center_x, center_y },
    )
    .await
    {
        let _ = app.emit_to("main", HOTKEY_ERROR, error);
    }
    true
}

async fn run_color_once(app: &AppHandle, card: &RecognitionCard) -> bool {
    let mut screenshots: Vec<image::DynamicImage> = Vec::with_capacity(card.color_probes.len());
    for probe in &card.color_probes {
        let Some(region) = probe.region.as_ref() else {
            return false;
        };
        let Some(img) = capture::capture_region(region) else {
            return false;
        };
        screenshots.push(img);
    }

    let result = matching::match_color_probes(
        &screenshots,
        &card.color_probes,
        card.color_match_mode.clone(),
        card.color_match_method.clone(),
    );
    if !result.matched {
        return false;
    }

    let matched_probes = result
        .matched_indices
        .iter()
        .filter_map(|index| {
            card.color_probes
                .get(*index)
                .and_then(|probe| probe.region.as_ref())
                .map(|region| ColorProbeMatch {
                    index: *index,
                    center_x: region.x + region.width / 2,
                    center_y: region.y + region.height / 2,
                })
        })
        .collect::<Vec<_>>();
    let _ = app.emit(REGION_MATCHED, &card.id);
    if let Err(error) = effects::execute(
        app.clone(),
        card.id.clone(),
        TriggerContext::Color { matched_probes },
    )
    .await
    {
        let _ = app.emit_to("main", HOTKEY_ERROR, error);
    }
    true
}

// ── 可注入的 watcher 循环步进 ──────────────────────────────────
// 从 run_region_watcher / run_color_watcher 循环体提取的核心逻辑，
// 接受可替换的依赖（截图/匹配/回放），便于测试。

/// Watcher 循环每轮的可替换依赖。
/// 生产代码使用真实实现，测试代码注入 mock。
#[cfg(test)]
pub trait WatcherDeps {
    /// 截取指定区域。
    fn capture(&self, region: &crate::morse::types::RegionRect) -> Option<image::DynamicImage>;

    /// 比较截图与参考图像，返回相似度。
    fn compare(&self, screenshot: &image::DynamicImage, reference: &image::DynamicImage) -> f32;

    /// 分派回放命令。
    fn dispatch_playback(&self, command: player::AudioCommand);
}

/// 区域监听 watcher 每轮 tick 的纯逻辑。
///
/// 返回 `true` 表示本轮匹配成功且已分派回放；`false` 表示未匹配或未分派。
/// 调用方负责冷却检查和更新 last_triggered。
#[cfg(test)]
pub fn region_watcher_step(
    deps: &dyn WatcherDeps,
    global_on: bool,
    recognition_on: bool,
    region: &crate::morse::types::RegionRect,
    reference_image: &image::DynamicImage,
    threshold: f32,
    _card_id: &str,
    playback_tx: &std::sync::mpsc::Sender<player::AudioCommand>,
    resolved_play: Option<&crate::recognition::ResolvedPlay>,
) -> bool {
    // 门控检查
    if !watcher_should_run(global_on, recognition_on) {
        return false;
    }

    // 截图
    let Some(captured) = deps.capture(region) else {
        return false;
    };

    // 比较
    let similarity = deps.compare(&captured, reference_image);
    if similarity < threshold {
        return false;
    }

    // 匹配成功 → 分派回放
    if let Some(resolved) = resolved_play {
        let exclusive = !resolved.allow_simultaneous;
        let _ = playback_tx.send(player::AudioCommand::Play {
            path: resolved.path.clone(),
            volume: resolved.volume,
            exclusive,
        });
        deps.dispatch_playback(player::AudioCommand::Play {
            path: resolved.path.clone(),
            volume: resolved.volume,
            exclusive,
        });
    }

    true
}

/// 识色 watcher 每轮 tick 的纯逻辑。
///
/// 返回 `true` 表示本轮匹配成功且已分派回放；`false` 表示未匹配或未分派。
#[cfg(test)]
pub fn color_watcher_step(
    deps: &dyn WatcherDeps,
    global_on: bool,
    recognition_on: bool,
    screenshots: &[image::DynamicImage],
    probes: &[crate::recognition::types::ColorProbe],
    match_mode: &crate::recognition::types::ColorMatchMode,
    match_method: &crate::recognition::types::ColorMatchMethod,
    playback_tx: &std::sync::mpsc::Sender<player::AudioCommand>,
    resolved_play: Option<&crate::recognition::ResolvedPlay>,
) -> bool {
    // 门控检查
    if !watcher_should_run(global_on, recognition_on) {
        return false;
    }

    let result = matching::match_color_probes(
        screenshots,
        probes,
        match_mode.clone(),
        match_method.clone(),
    );
    if !result.matched {
        return false;
    }

    // 匹配成功 → 分派回放
    if let Some(resolved) = resolved_play {
        let exclusive = !resolved.allow_simultaneous;
        let _ = playback_tx.send(player::AudioCommand::Play {
            path: resolved.path.clone(),
            volume: resolved.volume,
            exclusive,
        });
        deps.dispatch_playback(player::AudioCommand::Play {
            path: resolved.path.clone(),
            volume: resolved.volume,
            exclusive,
        });
    }

    true
}

async fn run_region_watcher(
    app: AppHandle,
    card_id: String,
    region: crate::morse::types::RegionRect,
    reference_image_path: String,
    _playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
    cooldown_ms: u32,
    threshold: f32,
    poll_interval_ms: u32,
    cancel: Arc<AtomicBool>,
) {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms.max(100) as u64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut last_triggered: Option<Instant> = None;

    // 加载参考图像
    let reference_image = match capture::load_reference_image(&reference_image_path) {
        Some(img) => {
            eprintln!(
                "[识别 watcher] 卡片 {card_id}: 参考图像加载成功 ({reference_image_path}), {}x{}",
                img.width(),
                img.height()
            );
            img
        }
        None => {
            eprintln!("[识别 watcher] 卡片 {card_id}: 无法加载参考图像: {reference_image_path}");
            return;
        }
    };

    while !cancel.load(Ordering::SeqCst) {
        ticker.tick().await;

        if cancel.load(Ordering::SeqCst) {
            break;
        }

        // A-M1 修复：循环内实时重读全局开关与识别触发模块开关（不再用启动快照）
        if !watcher_should_run(global_enabled(&app), recognition_module_enabled(&app)) {
            continue;
        }

        // 检查冷却
        if let Some(last) = last_triggered {
            if last.elapsed() < Duration::from_millis(cooldown_ms as u64) {
                continue;
            }
        }

        // 截取屏幕区域
        match capture::capture_region(&region) {
            Some(captured) => {
                let result = matching::compare_images(&captured, &reference_image);
                if result.similarity >= threshold {
                    eprintln!("[识别 watcher] 卡片 {card_id}: 匹配成功 similarity={:.4} >= threshold={threshold} (位置: {},{})", result.similarity, result.best_x, result.best_y);
                    let _ = app.emit(REGION_MATCHED, &card_id);
                    let center_x =
                        region.x + result.best_x as i32 + reference_image.width() as i32 / 2;
                    let center_y =
                        region.y + result.best_y as i32 + reference_image.height() as i32 / 2;
                    if let Err(error) = effects::execute(
                        app.clone(),
                        card_id.clone(),
                        TriggerContext::Region { center_x, center_y },
                    )
                    .await
                    {
                        let _ = app.emit_to("main", HOTKEY_ERROR, error);
                    }
                    last_triggered = Some(Instant::now());
                }
            }
            None => {
                // 截图失败，静默跳过
            }
        }
    }
}

async fn run_color_watcher(
    app: AppHandle,
    card_id: String,
    probes: Vec<crate::recognition::types::ColorProbe>,
    match_mode: crate::recognition::types::ColorMatchMode,
    match_method: crate::recognition::types::ColorMatchMethod,
    _playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
    cooldown_ms: u32,
    poll_interval_ms: u32,
    cancel: Arc<AtomicBool>,
) {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms.max(100) as u64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut last_triggered: Option<Instant> = None;

    while !cancel.load(Ordering::SeqCst) {
        ticker.tick().await;

        if cancel.load(Ordering::SeqCst) {
            break;
        }

        // A-M1 修复：循环内实时重读全局开关与识别触发模块开关（不再用启动快照）
        if !watcher_should_run(global_enabled(&app), recognition_module_enabled(&app)) {
            continue;
        }

        // 检查冷却
        if let Some(last) = last_triggered {
            if last.elapsed() < Duration::from_millis(cooldown_ms as u64) {
                continue;
            }
        }

        // 逐个截取 probe 区域；region 缺失（None）的探针视为未就绪，跳过本轮
        let mut screenshots: Vec<image::DynamicImage> = Vec::with_capacity(probes.len());
        let mut all_captured = true;
        for probe in &probes {
            let Some(region) = probe.region.as_ref() else {
                all_captured = false;
                break;
            };
            match capture::capture_region(region) {
                Some(img) => screenshots.push(img),
                None => {
                    all_captured = false;
                    break;
                }
            }
        }

        if !all_captured {
            continue;
        }

        let result = matching::match_color_probes(
            &screenshots,
            &probes,
            match_mode.clone(),
            match_method.clone(),
        );
        if result.matched {
            eprintln!(
                "[识别 color watcher] 卡片 {card_id}: 识色命中 {}/{} probes",
                result.hit_count,
                probes.len()
            );
            let _ = app.emit(REGION_MATCHED, &card_id);
            let matched_probes = result
                .matched_indices
                .iter()
                .filter_map(|index| {
                    probes
                        .get(*index)
                        .and_then(|probe| probe.region.as_ref())
                        .map(|region| ColorProbeMatch {
                            index: *index,
                            center_x: region.x + region.width / 2,
                            center_y: region.y + region.height / 2,
                        })
                })
                .collect::<Vec<_>>();
            if let Err(error) = effects::execute(
                app.clone(),
                card_id.clone(),
                TriggerContext::Color { matched_probes },
            )
            .await
            {
                let _ = app.emit_to("main", HOTKEY_ERROR, error);
            }
            last_triggered = Some(Instant::now());
        }
    }
}
