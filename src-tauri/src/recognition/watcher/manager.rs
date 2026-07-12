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
    for card in watcher_runtime_cards(settings) {
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
        let retrigger_after_disappear = card.retrigger_after_disappear;
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
                        retrigger_after_disappear,
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
                        retrigger_after_disappear,
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

pub(crate) fn watcher_runtime_cards<'a>(
    settings: &'a RecognitionSettings,
) -> impl Iterator<Item = &'a RecognitionCard> + 'a {
    crate::recognition::runtime_cards(settings)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchObservation {
    CaptureFailed,
    Matched,
    NotMatched,
}

const REARM_MISS_COUNT: u8 = 2;

#[derive(Debug)]
struct MatchGate {
    retrigger_after_disappear: bool,
    was_matched: bool,
    consecutive_misses: u8,
    last_triggered: Option<Instant>,
}

impl MatchGate {
    fn new(retrigger_after_disappear: bool) -> Self {
        Self {
            retrigger_after_disappear,
            was_matched: false,
            consecutive_misses: 0,
            last_triggered: None,
        }
    }

    fn observe(&mut self, observation: MatchObservation, cooldown_ms: u32, now: Instant) -> bool {
        match observation {
            MatchObservation::CaptureFailed => false,
            MatchObservation::NotMatched => {
                if self.retrigger_after_disappear && self.was_matched {
                    self.consecutive_misses = self.consecutive_misses.saturating_add(1);
                    if self.consecutive_misses >= REARM_MISS_COUNT {
                        self.was_matched = false;
                        self.consecutive_misses = 0;
                    }
                }
                false
            }
            MatchObservation::Matched => {
                self.consecutive_misses = 0;
                if self.retrigger_after_disappear && self.was_matched {
                    return false;
                }
                if self.retrigger_after_disappear {
                    self.was_matched = true;
                }
                let ready = self
                    .last_triggered
                    .map(|last| {
                        now.duration_since(last) >= Duration::from_millis(cooldown_ms as u64)
                    })
                    .unwrap_or(true);
                if ready {
                    self.last_triggered = Some(now);
                }
                ready
            }
        }
    }
}

fn activation_session_should_continue(
    mode: RecognitionActivationMode,
    matched_count: u32,
    trigger_count: u32,
    timed_out: bool,
) -> bool {
    match mode {
        RecognitionActivationMode::TimedHotkey => {
            !timed_out && matched_count < trigger_count.max(1)
        }
        RecognitionActivationMode::OnceHotkey | RecognitionActivationMode::Always => false,
    }
}

async fn run_activation_session(app: AppHandle, card_id: String, cancel: Arc<AtomicBool>) {
    let Some(card) = card_snapshot(&app, &card_id) else {
        return;
    };
    let duration = Duration::from_millis(card.activation.duration_ms.max(100) as u64);
    let interval_ms = card.watch_poll_interval_ms.max(100);
    let trigger_count = card.activation.trigger_count.max(1);
    let mut matched_count = 0_u32;
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
            if matched {
                matched_count = matched_count.saturating_add(1);
            }
            if card.activation.mode == RecognitionActivationMode::OnceHotkey && matched {
                break;
            }
        }
        if !activation_session_should_continue(
            card.activation.mode.clone(),
            matched_count,
            trigger_count,
            Instant::now() >= deadline,
        ) {
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
        .find(|card| {
            card.id == card_id
                && card.enabled
                && crate::recognition::card_group_enabled(&inner.settings, card)
        })
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

fn color_trigger_context(
    result: &matching::ColorMatchResult,
    probes: &[crate::recognition::types::ColorProbe],
    method: &crate::recognition::types::ColorMatchMethod,
) -> TriggerContext {
    let matched_probes = result
        .matched_probes
        .iter()
        .filter_map(|matched| {
            let region = probes.get(matched.index)?.region.as_ref()?;
            let (point_x, point_y) = match (method, matched.match_position) {
                (crate::recognition::types::ColorMatchMethod::AnyPixel, Some((x, y))) => {
                    (region.x + x as i32, region.y + y as i32)
                }
                _ => (region.x + region.width / 2, region.y + region.height / 2),
            };
            Some(ColorProbeMatch {
                index: matched.index,
                point_x,
                point_y,
            })
        })
        .collect();
    TriggerContext::Color { matched_probes }
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

    let _ = app.emit(REGION_MATCHED, &card.id);
    if let Err(error) = effects::execute(
        app.clone(),
        card.id.clone(),
        color_trigger_context(&result, &card.color_probes, &card.color_match_method),
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
    retrigger_after_disappear: bool,
    threshold: f32,
    poll_interval_ms: u32,
    cancel: Arc<AtomicBool>,
) {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms.max(100) as u64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut match_gate = MatchGate::new(retrigger_after_disappear);

    // 加载参考图像
    let reference_image = match capture::load_reference_image(&reference_image_path) {
        Some(img) => {
            crate::log_debug!(
                "recognition::watcher",
                "参考图像加载成功",
                "card_id" => card_id.clone(),
                "path" => reference_image_path.clone(),
                "width" => img.width(),
                "height" => img.height()
            );
            img
        }
        None => {
            crate::log_error!(
                "recognition::watcher",
                "无法加载参考图像",
                "card_id" => card_id.clone(),
                "path" => reference_image_path.clone()
            );
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

        // 截取屏幕区域
        match capture::capture_region(&region) {
            Some(captured) => {
                let result = matching::compare_images(&captured, &reference_image);
                if result.similarity >= threshold {
                    if !match_gate.observe(MatchObservation::Matched, cooldown_ms, Instant::now()) {
                        continue;
                    }
                    crate::log_debug!(
                        "recognition::watcher",
                        "区域识别命中",
                        "card_id" => card_id.clone(),
                        "similarity" => result.similarity,
                        "threshold" => threshold,
                        "best_x" => result.best_x,
                        "best_y" => result.best_y
                    );
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
                        crate::log_error!(
                            "recognition::watcher",
                            "区域识别效果执行失败",
                            "card_id" => card_id.clone(),
                            "error" => error.clone()
                        );
                        let _ = app.emit_to("main", HOTKEY_ERROR, error);
                    }
                } else {
                    match_gate.observe(MatchObservation::NotMatched, cooldown_ms, Instant::now());
                }
            }
            None => {
                match_gate.observe(MatchObservation::CaptureFailed, cooldown_ms, Instant::now());
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
    retrigger_after_disappear: bool,
    poll_interval_ms: u32,
    cancel: Arc<AtomicBool>,
) {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms.max(100) as u64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut match_gate = MatchGate::new(retrigger_after_disappear);

    while !cancel.load(Ordering::SeqCst) {
        ticker.tick().await;

        if cancel.load(Ordering::SeqCst) {
            break;
        }

        // A-M1 修复：循环内实时重读全局开关与识别触发模块开关（不再用启动快照）
        if !watcher_should_run(global_enabled(&app), recognition_module_enabled(&app)) {
            continue;
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
            match_gate.observe(MatchObservation::CaptureFailed, cooldown_ms, Instant::now());
            continue;
        }

        let result = matching::match_color_probes(
            &screenshots,
            &probes,
            match_mode.clone(),
            match_method.clone(),
        );
        if result.matched {
            if !match_gate.observe(MatchObservation::Matched, cooldown_ms, Instant::now()) {
                continue;
            }
            crate::log_debug!(
                "recognition::watcher",
                "识色命中",
                "card_id" => card_id.clone(),
                "hit_count" => result.hit_count,
                "probe_count" => probes.len()
            );
            let _ = app.emit(REGION_MATCHED, &card_id);
            if let Err(error) = effects::execute(
                app.clone(),
                card_id.clone(),
                color_trigger_context(&result, &probes, &match_method),
            )
            .await
            {
                crate::log_error!(
                    "recognition::watcher",
                    "识色效果执行失败",
                    "card_id" => card_id.clone(),
                    "error" => error.clone()
                );
                let _ = app.emit_to("main", HOTKEY_ERROR, error);
            }
        } else {
            match_gate.observe(MatchObservation::NotMatched, cooldown_ms, Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color_context_probe() -> crate::recognition::types::ColorProbe {
        crate::recognition::types::ColorProbe {
            region: Some(crate::morse::types::RegionRect {
                x: 100,
                y: 200,
                width: 20,
                height: 10,
            }),
            targets: vec![],
            probe_match_mode: crate::recognition::types::ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        }
    }

    #[test]
    fn any_pixel_context_adds_probe_origin() {
        let result = matching::ColorMatchResult {
            matched: true,
            hit_count: 1,
            matched_probes: vec![matching::MatchedColorProbe {
                index: 0,
                match_position: Some((3, 4)),
            }],
        };
        let TriggerContext::Color { matched_probes } = color_trigger_context(
            &result,
            &[color_context_probe()],
            &crate::recognition::types::ColorMatchMethod::AnyPixel,
        ) else {
            panic!("应生成识色上下文")
        };

        assert_eq!(
            (matched_probes[0].point_x, matched_probes[0].point_y),
            (103, 204)
        );
    }

    #[test]
    fn average_context_uses_probe_center() {
        let result = matching::ColorMatchResult {
            matched: true,
            hit_count: 1,
            matched_probes: vec![matching::MatchedColorProbe {
                index: 0,
                match_position: None,
            }],
        };
        let TriggerContext::Color { matched_probes } = color_trigger_context(
            &result,
            &[color_context_probe()],
            &crate::recognition::types::ColorMatchMethod::Average,
        ) else {
            panic!("应生成识色上下文")
        };

        assert_eq!(
            (matched_probes[0].point_x, matched_probes[0].point_y),
            (110, 205)
        );
    }

    #[test]
    fn cooldown_repeat_triggers_again_after_cooldown_while_still_matched() {
        let start = Instant::now();
        let mut gate = MatchGate::new(false);

        assert!(gate.observe(MatchObservation::Matched, 1000, start));
        assert!(!gate.observe(
            MatchObservation::Matched,
            1000,
            start + Duration::from_millis(999),
        ));
        assert!(gate.observe(
            MatchObservation::Matched,
            1000,
            start + Duration::from_millis(1000),
        ));
    }

    #[test]
    fn after_disappear_requires_two_consecutive_misses() {
        let start = Instant::now();
        let mut gate = MatchGate::new(true);

        assert!(gate.observe(MatchObservation::Matched, 0, start));
        gate.observe(
            MatchObservation::NotMatched,
            0,
            start + Duration::from_secs(1),
        );
        assert!(!gate.observe(MatchObservation::Matched, 0, start + Duration::from_secs(2)));
        gate.observe(
            MatchObservation::NotMatched,
            0,
            start + Duration::from_secs(3),
        );
        gate.observe(
            MatchObservation::NotMatched,
            0,
            start + Duration::from_secs(4),
        );
        assert!(gate.observe(MatchObservation::Matched, 0, start + Duration::from_secs(5)));
    }

    #[test]
    fn after_disappear_capture_failure_does_not_count_as_miss() {
        let start = Instant::now();
        let mut gate = MatchGate::new(true);

        assert!(gate.observe(MatchObservation::Matched, 0, start));
        gate.observe(
            MatchObservation::NotMatched,
            0,
            start + Duration::from_secs(1),
        );
        gate.observe(
            MatchObservation::CaptureFailed,
            0,
            start + Duration::from_secs(2),
        );
        assert!(!gate.observe(MatchObservation::Matched, 0, start + Duration::from_secs(3)));
    }

    #[test]
    fn after_disappear_consumes_rising_edge_during_cooldown() {
        let start = Instant::now();
        let mut gate = MatchGate::new(true);

        assert!(gate.observe(MatchObservation::Matched, 5000, start));
        gate.observe(
            MatchObservation::NotMatched,
            5000,
            start + Duration::from_secs(1),
        );
        gate.observe(
            MatchObservation::NotMatched,
            5000,
            start + Duration::from_secs(2),
        );
        assert!(!gate.observe(
            MatchObservation::Matched,
            5000,
            start + Duration::from_secs(3)
        ));
        assert!(!gate.observe(
            MatchObservation::Matched,
            5000,
            start + Duration::from_secs(6)
        ));
    }

    #[test]
    fn timed_activation_stops_after_target_trigger_count() {
        assert!(activation_session_should_continue(
            RecognitionActivationMode::TimedHotkey,
            2,
            3,
            false
        ));
        assert!(!activation_session_should_continue(
            RecognitionActivationMode::TimedHotkey,
            3,
            3,
            false
        ));
        assert!(!activation_session_should_continue(
            RecognitionActivationMode::TimedHotkey,
            2,
            3,
            true
        ));
    }

    #[test]
    fn watcher_runtime_cards_skip_disabled_groups() {
        let card = RecognitionCard {
            id: "disabled-group-card".into(),
            group_id: Some("g1".into()),
            order: 0,
            name: "禁用组卡片".into(),
            enabled: true,
            trigger_mode: RecognitionTriggerMode::RegionWatch,
            hotkey: None,
            watch_region: Some(crate::morse::types::RegionRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            }),
            watch_reference_image_path: Some("ref.png".into()),
            watch_match_threshold: 0.75,
            watch_poll_interval_ms: 500,
            retrigger_after_disappear: false,
            activation: crate::recognition::types::RecognitionActivation::default(),
            effects: crate::recognition::types::RecognitionEffects::default(),
            audio_files: Vec::new(),
            legacy_audio_file_path: None,
            play_mode: crate::recognition::types::PlayMode::Single,
            combo_window_ms: 60000,
            combo_windows: Vec::new(),
            volume: 0.8,
            cooldown_ms: 1000,
            allow_simultaneous: false,
            color_probes: Vec::new(),
            color_match_mode: crate::recognition::types::ColorMatchMode::All,
            color_match_method: crate::recognition::types::ColorMatchMethod::Average,
        };
        let settings = RecognitionSettings {
            recognition_enabled: true,
            card_groups: vec![crate::recognition::types::RecognitionGroup {
                id: "g1".into(),
                name: "禁用组".into(),
                order: 0,
                collapsed: false,
                enabled: false,
            }],
            cards: vec![card],
        };

        assert!(watcher_runtime_cards(&settings).next().is_none());
    }
}
