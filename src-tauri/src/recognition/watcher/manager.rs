//! Watcher 生命周期管理（restart / stop / run 循环）

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;
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

struct WatcherTask {
    generation: u64,
    cancel: Arc<AtomicBool>,
    handle: tauri::async_runtime::JoinHandle<()>,
}

/// 全局 watcher 状态：卡片 ID -> 当前 generation、取消标记和任务 handle。
static WATCHER_TASK_MAP: OnceLock<Mutex<HashMap<String, WatcherTask>>> = OnceLock::new();
static ACTIVATION_CANCEL_MAP: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
static BLOCKING_PERMITS: OnceLock<Arc<Semaphore>> = OnceLock::new();
static WATCHER_GENERATIONS: WatcherGenerations = WatcherGenerations::new();

struct WatcherGenerations(AtomicU64);

impl WatcherGenerations {
    const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn is_current(&self, generation: u64) -> bool {
        self.0.load(Ordering::SeqCst) == generation
    }
}

impl Default for WatcherGenerations {
    fn default() -> Self {
        Self::new()
    }
}

async fn run_blocking_limited<T, F>(job: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let permit = BLOCKING_PERMITS
        .get_or_init(|| Arc::new(Semaphore::new(2)))
        .clone()
        .try_acquire_owned()
        .ok()?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        job()
    })
    .await
    .ok()
}

fn watcher_is_current(generation: u64, cancel: &AtomicBool) -> bool {
    !cancel.load(Ordering::SeqCst) && WATCHER_GENERATIONS.is_current(generation)
}

fn cancel_watcher_tasks(task_map: &mut HashMap<String, WatcherTask>, generation: u64) {
    let mut handles = Vec::with_capacity(task_map.len());
    for (_, task) in task_map.drain() {
        debug_assert!(task.generation < generation);
        task.cancel.store(true, Ordering::SeqCst);
        task.handle.abort();
        handles.push(task.handle);
    }
    if !handles.is_empty() {
        tauri::async_runtime::spawn(async move {
            for handle in handles {
                let _ = handle.await;
            }
        });
    }
}

/// 启动/重启所有区域监听 watcher
pub fn restart_watchers(
    app: &AppHandle,
    settings: &RecognitionSettings,
    playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
) -> Result<(), String> {
    if !settings.recognition_enabled {
        return stop_all_watchers(app);
    }

    let generation = WATCHER_GENERATIONS.next();
    let mut task_map = WATCHER_TASK_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "识别监听状态已损坏".to_string())?;

    // 先取消所有现有 watcher
    cancel_watcher_tasks(&mut task_map, generation);
    let mut activations = ACTIVATION_CANCEL_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "识别激活会话状态已损坏".to_string())?;
    for (_, cancel) in activations.drain() {
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
                if card.watch_reference_image_paths.is_empty() {
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
        let task_card_id = card_id.clone();

        let handle = match card.trigger_mode {
            RecognitionTriggerMode::RegionWatch => {
                let Some(region) = &card.watch_region else {
                    continue;
                };
                let threshold = card.watch_match_threshold;
                let region_clone = region.clone();
                let ref_paths_clone = card.watch_reference_image_paths.clone();
                tauri::async_runtime::spawn(async move {
                    run_region_watcher(
                        app_clone,
                        task_card_id,
                        region_clone,
                        ref_paths_clone,
                        playback_tx_clone,
                        cooldown_ms,
                        retrigger_after_disappear,
                        threshold,
                        poll_interval_ms,
                        cancel_clone,
                        generation,
                    )
                    .await;
                })
            }
            RecognitionTriggerMode::ColorWatch => {
                let probes = card.color_probes.clone();
                let match_mode = card.color_match_mode.clone();
                let match_method = card.color_match_method.clone();
                tauri::async_runtime::spawn(async move {
                    run_color_watcher(
                        app_clone,
                        task_card_id,
                        probes,
                        match_mode,
                        match_method,
                        playback_tx_clone,
                        cooldown_ms,
                        retrigger_after_disappear,
                        poll_interval_ms,
                        cancel_clone,
                        generation,
                    )
                    .await;
                })
            }
            RecognitionTriggerMode::Hotkey => continue,
        };
        task_map.insert(
            card_id,
            WatcherTask {
                generation,
                cancel,
                handle,
            },
        );
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
    let generation = WATCHER_GENERATIONS.next();
    let mut task_map = WATCHER_TASK_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "识别监听状态已损坏".to_string())?;
    cancel_watcher_tasks(&mut task_map, generation);
    let mut activations = ACTIVATION_CANCEL_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "识别激活会话状态已损坏".to_string())?;
    for (_, cancel) in activations.drain() {
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
                RecognitionTriggerMode::RegionWatch => run_region_once(&app, &card, &cancel).await,
                RecognitionTriggerMode::ColorWatch => run_color_once(&app, &card, &cancel).await,
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

async fn run_region_once(app: &AppHandle, card: &RecognitionCard, cancel: &AtomicBool) -> bool {
    let Some(region) = card.watch_region.as_ref() else {
        return false;
    };
    if card.watch_reference_image_paths.is_empty() {
        return false;
    }
    let region_for_job = region.clone();
    let paths = card.watch_reference_image_paths.clone();
    let Some(Some((result, reference_width, reference_height))) = run_blocking_limited(move || {
        let captured = capture::capture_region(&region_for_job)?;
        let references: Vec<_> = paths
            .iter()
            .filter_map(|path| capture::load_reference_image(path))
            .collect();
        let (reference_index, result) = matching::best_reference_match(&captured, &references)?;
        let reference = &references[reference_index];
        Some((result, reference.width(), reference.height()))
    })
    .await
    else {
        return false;
    };
    if result.similarity < card.watch_match_threshold {
        return false;
    }

    if cancel.load(Ordering::SeqCst) {
        return false;
    }
    let center_x = region.x + result.best_x as i32 + reference_width as i32 / 2;
    let center_y = region.y + result.best_y as i32 + reference_height as i32 / 2;
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

async fn run_color_once(app: &AppHandle, card: &RecognitionCard, cancel: &AtomicBool) -> bool {
    let probes = card.color_probes.clone();
    let probes_for_job = probes.clone();
    let match_mode = card.color_match_mode.clone();
    let match_method = card.color_match_method.clone();
    let Some(Some(result)) = run_blocking_limited(move || {
        let mut screenshots = Vec::with_capacity(probes_for_job.len());
        for probe in &probes_for_job {
            screenshots.push(capture::capture_region(probe.region.as_ref()?)?);
        }
        Some(matching::match_color_probes(
            &screenshots,
            &probes_for_job,
            match_mode,
            match_method,
        ))
    })
    .await
    else {
        return false;
    };
    if !result.matched {
        return false;
    }
    if cancel.load(Ordering::SeqCst) {
        return false;
    }

    let _ = app.emit(REGION_MATCHED, &card.id);
    if let Err(error) = effects::execute(
        app.clone(),
        card.id.clone(),
        color_trigger_context(&result, &probes, &card.color_match_method),
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
    reference_image_paths: Vec<String>,
    _playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
    cooldown_ms: u32,
    retrigger_after_disappear: bool,
    threshold: f32,
    poll_interval_ms: u32,
    cancel: Arc<AtomicBool>,
    generation: u64,
) {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms.max(100) as u64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut match_gate = MatchGate::new(retrigger_after_disappear);

    let reference_images: Vec<_> = reference_image_paths
        .iter()
        .filter_map(|path| match capture::load_reference_image(path) {
            Some(image) => {
                crate::log_debug!(
                    "recognition::watcher",
                    "参考图像加载成功",
                    "card_id" => card_id.clone(),
                    "path" => path.clone(),
                    "width" => image.width(),
                    "height" => image.height()
                );
                Some(image)
            }
            None => {
                crate::log_error!(
                    "recognition::watcher",
                    "无法加载参考图像",
                    "card_id" => card_id.clone(),
                    "path" => path.clone()
                );
                None
            }
        })
        .collect();
    if reference_images.is_empty() {
        return;
    }
    let reference_images = Arc::new(reference_images);

    while watcher_is_current(generation, &cancel) {
        ticker.tick().await;

        if !watcher_is_current(generation, &cancel) {
            break;
        }

        // A-M1 修复：循环内实时重读全局开关与识别触发模块开关（不再用启动快照）
        if !watcher_should_run(global_enabled(&app), recognition_module_enabled(&app)) {
            continue;
        }

        let region_for_job = region.clone();
        let references_for_job = Arc::clone(&reference_images);
        let Some(frame) = run_blocking_limited(move || {
            let captured = capture::capture_region(&region_for_job)?;
            let (reference_index, result) =
                matching::best_reference_match(&captured, references_for_job.iter())?;
            let reference = &references_for_job[reference_index];
            Some((result, reference.width(), reference.height()))
        })
        .await
        else {
            continue;
        };
        match frame {
            Some((result, reference_width, reference_height)) => {
                if !watcher_is_current(generation, &cancel) {
                    break;
                }
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
                    if !watcher_is_current(generation, &cancel) {
                        break;
                    }
                    let _ = app.emit(REGION_MATCHED, &card_id);
                    let center_x = region.x + result.best_x as i32 + reference_width as i32 / 2;
                    let center_y = region.y + result.best_y as i32 + reference_height as i32 / 2;
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
    generation: u64,
) {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms.max(100) as u64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut match_gate = MatchGate::new(retrigger_after_disappear);
    let probes = Arc::new(probes);

    while watcher_is_current(generation, &cancel) {
        ticker.tick().await;

        if !watcher_is_current(generation, &cancel) {
            break;
        }

        // A-M1 修复：循环内实时重读全局开关与识别触发模块开关（不再用启动快照）
        if !watcher_should_run(global_enabled(&app), recognition_module_enabled(&app)) {
            continue;
        }

        let probes_for_job = Arc::clone(&probes);
        let match_mode_for_job = match_mode.clone();
        let match_method_for_job = match_method.clone();
        let Some(frame) = run_blocking_limited(move || {
            let mut screenshots = Vec::with_capacity(probes_for_job.len());
            for probe in probes_for_job.iter() {
                let region = probe.region.as_ref()?;
                screenshots.push(capture::capture_region(region)?);
            }
            Some(matching::match_color_probes(
                &screenshots,
                &probes_for_job,
                match_mode_for_job,
                match_method_for_job,
            ))
        })
        .await
        else {
            continue;
        };

        let Some(result) = frame else {
            match_gate.observe(MatchObservation::CaptureFailed, cooldown_ms, Instant::now());
            continue;
        };
        if !watcher_is_current(generation, &cancel) {
            break;
        }
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
            if !watcher_is_current(generation, &cancel) {
                break;
            }
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

    #[test]
    fn newer_watcher_generation_invalidates_old_work() {
        let generations = WatcherGenerations::default();
        let first = generations.next();
        let second = generations.next();

        assert!(!generations.is_current(first));
        assert!(generations.is_current(second));
    }

    #[test]
    fn watcher_validity_rejects_cancelled_or_stale_results() {
        let cancel = AtomicBool::new(false);
        let generation = WATCHER_GENERATIONS.next();
        assert!(watcher_is_current(generation, &cancel));

        WATCHER_GENERATIONS.next();
        assert!(!watcher_is_current(generation, &cancel));

        let current = WATCHER_GENERATIONS.next();
        cancel.store(true, Ordering::SeqCst);
        assert!(!watcher_is_current(current, &cancel));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blocking_scheduler_skips_third_job_while_two_are_running() {
        let started = Arc::new(std::sync::Barrier::new(3));
        let release = Arc::new(std::sync::Barrier::new(3));
        let mut jobs = Vec::new();

        for _ in 0..2 {
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            jobs.push(tokio::spawn(async move {
                run_blocking_limited(move || {
                    started.wait();
                    release.wait();
                    1
                })
                .await
            }));
        }

        started.wait();
        assert_eq!(run_blocking_limited(|| 3).await, None);
        release.wait();
        for job in jobs {
            assert_eq!(job.await.unwrap(), Some(1));
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blocking_result_becomes_stale_when_generation_changes() {
        let started = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let generation = WATCHER_GENERATIONS.next();
        let cancel = Arc::new(AtomicBool::new(false));
        let job = {
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            tokio::spawn(async move {
                run_blocking_limited(move || {
                    started.wait();
                    release.wait();
                    "旧匹配结果"
                })
                .await
            })
        };

        started.wait();
        WATCHER_GENERATIONS.next();
        release.wait();

        assert_eq!(job.await.unwrap(), Some("旧匹配结果"));
        assert!(!watcher_is_current(generation, &cancel));
    }

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
            watch_reference_image_paths: vec!["ref.png".into()],
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
