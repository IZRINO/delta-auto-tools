//! Watcher 生命周期管理（restart / stop / run 循环）

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tokio::time::{interval, MissedTickBehavior};

use crate::audio::events::REGION_MATCHED;
use crate::audio::player;
use crate::audio::resolve_play_for_card;
use crate::audio::types::AudioSettings;
use crate::global_state::GlobalState;
use tauri::{AppHandle, Emitter, Manager};

use super::capture;
use super::matching;

/// 全局 watcher 状态：卡片 ID -> 取消标记
static WATCHER_CANCEL_MAP: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

/// 启动/重启所有区域监听 watcher
pub fn restart_watchers(
    app: &AppHandle,
    settings: &AudioSettings,
    playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
) -> Result<(), String> {
    if !settings.audio_enabled {
        return stop_all_watchers(app);
    }

    let mut cancel_map = WATCHER_CANCEL_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "音频监听状态已损坏".to_string())?;

    // 先取消所有现有 watcher
    for (_, cancel) in cancel_map.drain() {
        cancel.store(true, Ordering::SeqCst);
    }

    // A-M1 修复：不在启动时快照 audio_enabled，循环内实时读取 AudioState
    // 这样当用户在 watcher 运行期间关闭音频模块开关时，watcher 能立即感知并停止

    // 为每张区域监听 / 识色卡片启动 watcher
    for card in &settings.cards {
        if !card.enabled {
            continue;
        }
        // 按触发模式校验必要字段，缺字段则跳过
        match card.trigger_mode {
            super::super::types::AudioTriggerMode::RegionWatch => {
                let Some(ref_path) = &card.watch_reference_image_path else {
                    continue;
                };
                if ref_path.is_empty() || card.audio_files.is_empty() {
                    continue;
                }
                if card.watch_region.is_none() {
                    continue;
                }
            }
            super::super::types::AudioTriggerMode::ColorWatch => {
                if card.color_probes.is_empty() || card.audio_files.is_empty() {
                    continue;
                }
                // 含未框选（region=None）探针的卡片视为未就绪草稿，不启动 watcher
                if card.color_probes.iter().any(|p| p.region.is_none()) {
                    continue;
                }
            }
            super::super::types::AudioTriggerMode::Hotkey => continue,
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
            super::super::types::AudioTriggerMode::RegionWatch => {
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
            super::super::types::AudioTriggerMode::ColorWatch => {
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
            super::super::types::AudioTriggerMode::Hotkey => {}
        }
    }

    Ok(())
}

/// 停止所有区域监听 watcher
pub fn stop_all_watchers(_app: &AppHandle) -> Result<(), String> {
    let mut cancel_map = WATCHER_CANCEL_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "音频监听状态已损坏".to_string())?;
    for (_, cancel) in cancel_map.drain() {
        cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// 读取全局总开关当前状态。`GlobalState` 缺失时视为已启用，不阻断 watcher。
fn global_enabled(app: &AppHandle) -> bool {
    app.try_state::<GlobalState>()
        .map(|state| state.enabled())
        .unwrap_or(true)
}

/// 读取 AudioState 中音频模块开关状态。`AudioState` 缺失时视为已启用。
fn audio_module_enabled(app: &AppHandle) -> bool {
    use crate::audio::AudioState;
    app.try_state::<AudioState>()
        .and_then(|state| {
            state.lock_inner().ok().map(|inner| inner.settings.audio_enabled)
        })
        .unwrap_or(true)
}

/// watcher 每轮 tick 的执行门：全局总开关与音频模块开关均开启时才执行。
///
/// - `global_enabled`：实时读取 `GlobalState`。全局总开关切换不会触发 `restart_watchers`，
///   故必须在 watcher 循环内实时检查。
/// - `audio_module_enabled`：实时读取 `AudioState.settings.audio_enabled`。
///   A-M1 修复：之前用启动快照，关开关后 watcher 仍可能继续。
///   现在改为循环内重读，保证关闭后 watcher 立即停止截图与匹配。
pub(crate) fn watcher_should_run(global_on: bool, audio_on: bool) -> bool {
    global_on && audio_on
}

async fn run_region_watcher(
    app: AppHandle,
    card_id: String,
    region: crate::morse::types::RegionRect,
    reference_image_path: String,
    playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
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
                "[音频 watcher] 卡片 {card_id}: 参考图像加载成功 ({reference_image_path}), {}x{}",
                img.width(),
                img.height()
            );
            img
        }
        None => {
            eprintln!("[音频 watcher] 卡片 {card_id}: 无法加载参考图像: {reference_image_path}");
            return;
        }
    };

    while !cancel.load(Ordering::SeqCst) {
        ticker.tick().await;

        if cancel.load(Ordering::SeqCst) {
            break;
        }

        // A-M1 修复：循环内实时重读全局开关与音频模块开关（不再用启动快照）
        if !watcher_should_run(global_enabled(&app), audio_module_enabled(&app)) {
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
                    // 触发音频播放
                    eprintln!("[音频 watcher] 卡片 {card_id}: 匹配成功 similarity={:.4} >= threshold={threshold} (位置: {},{})", result.similarity, result.best_x, result.best_y);
                    let _ = app.emit(REGION_MATCHED, &card_id);
                    let resolved = resolve_play_for_card(&app, &card_id);
                    if let Some(resolved) = resolved {
                        let tx = playback_tx.clone();
                        let exclusive = !resolved.allow_simultaneous;
                        let _ = tx.send(player::AudioCommand::Play {
                            path: resolved.path,
                            volume: resolved.volume,
                            exclusive,
                        });
                        last_triggered = Some(Instant::now());
                    }
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
    probes: Vec<crate::audio::types::ColorProbe>,
    match_mode: crate::audio::types::ColorMatchMode,
    match_method: crate::audio::types::ColorMatchMethod,
    playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
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

        // A-M1 修复：循环内实时重读全局开关与音频模块开关（不再用启动快照）
        if !watcher_should_run(global_enabled(&app), audio_module_enabled(&app)) {
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
                "[音频 color watcher] 卡片 {card_id}: 识色命中 {}/{} probes",
                result.hit_count,
                probes.len()
            );
            let _ = app.emit(REGION_MATCHED, &card_id);
            if let Some(resolved) = resolve_play_for_card(&app, &card_id) {
                let tx = playback_tx.clone();
                let exclusive = !resolved.allow_simultaneous;
                let _ = tx.send(player::AudioCommand::Play {
                    path: resolved.path,
                    volume: resolved.volume,
                    exclusive,
                });
                last_triggered = Some(Instant::now());
            }
        }
    }
}
