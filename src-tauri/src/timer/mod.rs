use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};
use tokio::{
    sync::oneshot,
    time::{self, Duration},
};

mod counter_state;
mod settings;
mod types;
use crate::hotkeys::{HoldAction, HoldActionCallback, HotkeyAction, HotkeyManager};
use crate::utils::now_ms;

use self::counter_state::CounterState;
use self::types::{
    CounterItem, CounterRunState, TimerBootstrap, TimerDirection, TimerDisplaySettings,
    TimerDisplayTarget, TimerItem, TimerRect, TimerRunState, TimerRunStatus, TimerSelectionKind,
    TimerSelectionOutcome, TimerSettings, TimerTriggerMode,
};

const TIMER_DISPLAY_LABEL: &str = "timer-display";
const TIMER_POSITION_LABEL: &str = "timer-position";
const COUNTER_DISPLAY_LABEL: &str = "counter-display";
const COUNTER_POSITION_LABEL: &str = "counter-position";
const TIMER_DISPLAY_WIDTH: i32 = 320;
const TIMER_DISPLAY_MIN_HEIGHT: i32 = 96;

pub struct TimerState {
    inner: Mutex<TimerStateInner>,
    tick_task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

struct TimerStateInner {
    settings: TimerSettings,
    runs: HashMap<String, TimerRuntime>,
    counter_runs: HashMap<String, i64>,
    pending_position: Option<PendingTimerPosition>,
    hotkey_error: Option<String>,
}

struct TimerRuntime {
    started_at_ms: u64,
    ends_at_ms: Option<u64>,
    current_seconds: u64,
    remaining_seconds: u64,
    duration_seconds: u64,
    direction: TimerDirection,
    status: TimerRunStatus,
    segment_count: u32,
    segment_duration: u64,
    recovery_start_pool: u64,
}

struct PendingTimerPosition {
    target: TimerDisplayTarget,
    original_rect: TimerRect,
    staged_rect: TimerRect,
    sender: oneshot::Sender<TimerSelectionKind>,
}

#[derive(Clone)]
struct HotkeyTriggerTargets {
    timer_ids: Vec<String>,
    counter_ids: Vec<String>,
}

impl TimerStateInner {
    fn bootstrap(&self) -> TimerBootstrap {
        TimerBootstrap {
            settings: self.settings.clone(),
            runs: self.run_states(),
            counter_runs: self.counter_run_states(),
            hotkey_error: self.hotkey_error.clone(),
        }
    }

    fn run_states(&self) -> Vec<TimerRunState> {
        self.settings
            .timers
            .iter()
            .filter_map(|timer| {
                self.runs.get(&timer.id).map(|runtime| {
                    let is_multi = runtime.segment_count > 1;
                    TimerRunState {
                        id: timer.id.clone(),
                        current_seconds: runtime.current_seconds,
                        remaining_seconds: if is_multi {
                            runtime.current_seconds
                        } else {
                            runtime.remaining_seconds
                        },
                        duration_seconds: runtime.duration_seconds,
                        direction: runtime.direction.clone(),
                        status: runtime.status.clone(),
                        segment_count: if is_multi {
                            Some(runtime.segment_count)
                        } else {
                            None
                        },
                        segment_duration: runtime.segment_duration,
                        recovering: runtime.status == TimerRunStatus::Running,
                        recovering_count: 0,
                        active_segment_index: 0,
                        started_at_ms: runtime.started_at_ms,
                        recovery_start_pool: runtime.recovery_start_pool,
                    }
                })
            })
            .collect()
    }

    fn counter_run_states(&self) -> Vec<CounterRunState> {
        self.settings
            .counters
            .iter()
            .map(|counter| CounterRunState {
                id: counter.id.clone(),
                value: self
                    .counter_runs
                    .get(&counter.id)
                    .copied()
                    .unwrap_or(counter.start_value),
            })
            .collect()
    }
 }

/// 把当前 `counter_runs` 落盘。仅写入 `settings.counters` 里仍存在的 ID，
/// 孤儿 ID（counter 已删但 state 文件残留）自动清理。写盘失败不阻塞主流程。
fn persist_counter_runs(app: &AppHandle, inner: &TimerStateInner) {
    let mut runs = std::collections::BTreeMap::new();
    for counter in &inner.settings.counters {
        if let Some(value) = inner.counter_runs.get(&counter.id) {
            runs.insert(counter.id.clone(), *value);
        }
    }
    let state = CounterState { runs };
    let _ = counter_state::save(app, &state);
}

fn display_height(item_count: usize) -> i32 {
    TIMER_DISPLAY_MIN_HEIGHT.max(48 + item_count.max(1) as i32 * 30)
}

fn normalize_display(display: &mut TimerDisplaySettings, item_count: usize) -> Result<(), String> {
    display.rect.width = display.rect.width.max(TIMER_DISPLAY_WIDTH);
    display.rect.height = display_height(item_count);

    if !(0.1..=1.0).contains(&display.font_opacity) {
        return Err("字体透明度必须在 0.1 到 1 之间".to_string());
    }

    Ok(())
}

fn normalize_timer(timer: &TimerItem) -> Result<TimerItem, String> {
    let name = timer.name.trim();
    if name.is_empty() {
        return Err("计时器名称不能为空".to_string());
    }

    let hotkey = timer.hotkey.trim();
    if hotkey.is_empty() {
        return Err(format!("{} 的快捷键不能为空", name));
    }

    if timer.duration_seconds == 0 {
        return Err(format!("{} 的计时秒数必须大于 0", name));
    }

    if let Some(count) = timer.segment_count {
        if count < 2 {
            return Err(format!("{} 的多段数必须大于 1", name));
        }
    }

    Ok(TimerItem {
        id: timer.id.trim().to_string(),
        name: name.to_string(),
        duration_seconds: timer.duration_seconds,
        hotkey: hotkey.to_string(),
        direction: timer.direction.clone(),
        trigger_mode: timer.trigger_mode.clone(),
        enabled: timer.enabled,
        ignore_running: timer.ignore_running,
        segment_count: timer.segment_count,
    })
}

fn normalize_counter(counter: &CounterItem) -> Result<CounterItem, String> {
    let name = counter.name.trim();
    if name.is_empty() {
        return Err("计数器名称不能为空".to_string());
    }

    let hotkey = counter.hotkey.trim();
    if hotkey.is_empty() {
        return Err(format!("{} 的快捷键不能为空", name));
    }

    Ok(CounterItem {
        id: counter.id.trim().to_string(),
        name: name.to_string(),
        start_value: counter.start_value,
        hotkey: hotkey.to_string(),
        enabled: counter.enabled,
    })
}

fn normalize_settings(mut settings_value: TimerSettings) -> Result<TimerSettings, String> {
    if settings_value.enabled && !settings_value.timer_enabled && !settings_value.counter_enabled {
        settings_value.timer_enabled = true;
        settings_value.counter_enabled = true;
    }

    settings_value.enabled = settings_value.timer_enabled || settings_value.counter_enabled;

    normalize_display(
        &mut settings_value.display,
        settings_value.timers.iter().filter(|t| t.enabled).count(),
    )?;
    normalize_display(
        &mut settings_value.counter_display,
        settings_value.counters.iter().filter(|c| c.enabled).count(),
    )?;

    if settings_value.timers.is_empty() {
        settings_value.timers.push(TimerItem {
            id: format!("timer-{}", crate::utils::now_ms()),
            name: "计时器 1".to_string(),
            duration_seconds: 30,
            hotkey: "F2".to_string(),
            direction: TimerDirection::Countdown,
            trigger_mode: TimerTriggerMode::Press,
            enabled: true,
            ignore_running: true,
            segment_count: None,
        });
    }

    if settings_value.counters.is_empty() {
        settings_value.counters.push(CounterItem {
            id: format!("counter-{}", crate::utils::now_ms()),
            name: "计数器 1".to_string(),
            start_value: 0,
            hotkey: "F3".to_string(),
            enabled: true,
        });
    }

    let mut seen_ids = HashMap::new();
    let mut timers = Vec::with_capacity(settings_value.timers.len());
    for timer in &settings_value.timers {
        let normalized = normalize_timer(timer)?;
        if seen_ids.insert(normalized.id.clone(), true).is_some() {
            return Err(format!("计时器 ID 重复: {}", normalized.id));
        }
        timers.push(normalized);
    }

    let mut counters = Vec::with_capacity(settings_value.counters.len());
    for counter in &settings_value.counters {
        let normalized = normalize_counter(counter)?;
        if seen_ids.insert(normalized.id.clone(), true).is_some() {
            return Err(format!("计时/计数器 ID 重复: {}", normalized.id));
        }
        counters.push(normalized);
    }

    settings_value.timers = timers;
    settings_value.counters = counters;
    Ok(settings_value)
}

fn restart_hotkey_listeners(
    state: &TimerState,
    hotkey_manager: &HotkeyManager,
    settings_value: &TimerSettings,
) -> Result<(), String> {
    if !settings_value.timer_enabled && !settings_value.counter_enabled {
        hotkey_manager.clear_scope("timer")?;
        return hotkey_manager.clear_hold_scope("timer");
    }

    // 收集所有热键绑定的目标
    let mut by_hotkey: HashMap<String, (Vec<String>, Vec<String>, Vec<String>)> = HashMap::new();
    // (press_timer_ids, release_timer_ids, counter_ids)

    if settings_value.timer_enabled {
        for timer in &settings_value.timers {
            if !timer.enabled {
                continue;
            }
            let entry = by_hotkey
                .entry(timer.hotkey.trim().to_string())
                .or_insert_with(|| (Vec::new(), Vec::new(), Vec::new()));
            match timer.trigger_mode {
                TimerTriggerMode::Press => entry.0.push(timer.id.clone()),
                TimerTriggerMode::Release => entry.1.push(timer.id.clone()),
            }
        }
    }
    if settings_value.counter_enabled {
        for counter in &settings_value.counters {
            if !counter.enabled {
                continue;
            }
            by_hotkey
                .entry(counter.hotkey.trim().to_string())
                .or_insert_with(|| (Vec::new(), Vec::new(), Vec::new()))
                .2
                .push(counter.id.clone());
        }
    }

    let mut normal_bindings: Vec<(String, HotkeyAction)> = Vec::new();
    let mut hold_bindings: Vec<(String, HoldActionCallback)> = Vec::new();

    for (hotkey, (press_timer_ids, release_timer_ids, counter_ids)) in by_hotkey {
        if !release_timer_ids.is_empty() {
            // 有释放模式计时器：使用 hold 绑定，Down 触发按压模式，Up 触发释放模式
            let press_targets = HotkeyTriggerTargets {
                timer_ids: press_timer_ids,
                counter_ids,
            };
            let release_targets = HotkeyTriggerTargets {
                timer_ids: release_timer_ids,
                counter_ids: Vec::new(),
            };
            let hold_callback: HoldActionCallback = Arc::new(move |app_handle, action| {
                let targets = match action {
                    HoldAction::Down => press_targets.clone(),
                    HoldAction::Up => release_targets.clone(),
                };
                if targets.timer_ids.is_empty() && targets.counter_ids.is_empty() {
                    return;
                }
                let app = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = trigger_hotkey_targets(&app, targets) {
                        let _ = app.emit_to("main", "timer://hotkey-error", error);
                    }
                });
            });
            hold_bindings.push((hotkey, hold_callback));
        } else {
            // 纯按压模式：使用普通绑定（当前行为）
            let targets = HotkeyTriggerTargets {
                timer_ids: press_timer_ids,
                counter_ids,
            };
            let action: HotkeyAction = Arc::new(move |app_handle| {
                let targets = targets.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = trigger_hotkey_targets(&app_handle, targets) {
                        let _ = app_handle.emit_to("main", "timer://hotkey-error", error);
                    }
                });
            });
            normal_bindings.push((hotkey, action));
        }
    }

    // 先清空 scope，再分别注册普通和 hold 绑定
    hotkey_manager.clear_scope("timer")?;
    // 始终调用 clear_hold_scope，避免 release 模式计时器全部移除后
    // 旧的 hold 绑定残留导致 Up 事件仍触发空回调
    hotkey_manager.clear_hold_scope("timer")?;

    if !normal_bindings.is_empty() {
        hotkey_manager.replace_scope("timer", normal_bindings)?;
    }

    if !hold_bindings.is_empty() {
        hotkey_manager.replace_hold_scope("timer", hold_bindings)?;
    }
    if let Ok(mut inner) = state.inner.lock() {
        inner.hotkey_error = None;
    }
    Ok(())
}

fn stop_tick_task(state: &TimerState) -> Result<(), String> {
    let mut tick_task = state
        .tick_task
        .lock()
        .map_err(|_| "计时器刷新状态已损坏".to_string())?;

    if let Some(task) = tick_task.take() {
        task.abort();
    }

    Ok(())
}

fn start_tick_task(state: &TimerState, app: &AppHandle) -> Result<(), String> {
    stop_tick_task(state)?;

    let app_handle = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;
            let _ = tick(&app_handle);
        }
    });

    let mut tick_task = state
        .tick_task
        .lock()
        .map_err(|_| "计时器刷新状态已损坏".to_string())?;
    *tick_task = Some(task);
    Ok(())
}

fn emit_state(app: &AppHandle, bootstrap: TimerBootstrap) {
    let _ = app.emit_to("main", "timer://state-changed", bootstrap.clone());
    let _ = app.emit_to(
        TIMER_DISPLAY_LABEL,
        "timer://state-changed",
        bootstrap.clone(),
    );
    let _ = app.emit_to(COUNTER_DISPLAY_LABEL, "timer://state-changed", bootstrap);
}

fn ensure_overlay_window(
    app: &AppHandle,
    label: &str,
    query_mode: &str,
    title: &str,
    display: &TimerDisplaySettings,
    enabled: bool,
) -> Result<(), String> {
    if !enabled {
        hide_window(app, label);
        return Ok(());
    }

    let rect = &display.rect;
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_size(PhysicalSize::new(rect.width as u32, rect.height as u32));
        let _ = window.set_position(PhysicalPosition::new(rect.x, rect.y));
        let _ = window.set_always_on_top(true);
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.show();
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(format!("index.html?mode={query_mode}").into()),
    )
    .title(title)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(true)
    .resizable(false)
    .inner_size(rect.width as f64, rect.height as f64)
    .position(rect.x as f64, rect.y as f64)
    .build()
    .map_err(|error| format!("创建{title}失败: {error}"))?;

    let _ = window.set_ignore_cursor_events(true);
    Ok(())
}

fn ensure_display_windows(app: &AppHandle, settings_value: &TimerSettings) -> Result<(), String> {
    let enabled_timer_count = settings_value.timers.iter().filter(|t| t.enabled).count();
    let enabled_counter_count = settings_value.counters.iter().filter(|c| c.enabled).count();

    let mut timer_display = settings_value.display.clone();
    timer_display.rect.height = display_height(enabled_timer_count);
    let mut counter_display = settings_value.counter_display.clone();
    counter_display.rect.height = display_height(enabled_counter_count);

    ensure_overlay_window(
        app,
        TIMER_DISPLAY_LABEL,
        "timer-display",
        "计时器透明窗口",
        &timer_display,
        settings_value.timer_enabled,
    )?;
    ensure_overlay_window(
        app,
        COUNTER_DISPLAY_LABEL,
        "counter-display",
        "计数器透明窗口",
        &counter_display,
        settings_value.counter_enabled,
    )?;
    Ok(())
}

fn hide_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.hide();
    }
}

fn destroy_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.destroy();
    }
}

fn destroy_display_windows(app: &AppHandle) {
    destroy_window(app, TIMER_DISPLAY_LABEL);
    destroy_window(app, COUNTER_DISPLAY_LABEL);
}

fn destroy_position_windows(app: &AppHandle) {
    destroy_window(app, TIMER_POSITION_LABEL);
    destroy_window(app, COUNTER_POSITION_LABEL);
}

pub fn is_main_window_close(label: &str) -> bool {
    label == "main"
}

pub fn shutdown(app: &AppHandle, state: &TimerState, hotkey_manager: &HotkeyManager) {
    let _ = hotkey_manager.clear_scope("timer");
    let _ = hotkey_manager.clear_hold_scope("timer");
    let _ = stop_tick_task(state);
    // 这里在进程异常退出 / 用户直接关窗没走 reset 流程时仍有最后一份）。
    if let Ok(inner) = state.inner.lock() {
        persist_counter_runs(app, &inner);
    }
    destroy_position_windows(app);
    destroy_display_windows(app);
}

fn update_timer_runtime(runtime: &mut TimerRuntime, now: u64) -> bool {
    // Multi-segment timer: pool model, recovers 1 second per real second
    if runtime.segment_count > 1 {
        if runtime.status != TimerRunStatus::Running {
            return false;
        }
        let elapsed_seconds = now.saturating_sub(runtime.started_at_ms) / 1000;
        let recovered = runtime
            .recovery_start_pool
            .saturating_add(elapsed_seconds)
            .min(runtime.duration_seconds);
        if recovered != runtime.current_seconds {
            runtime.current_seconds = recovered;
            runtime.remaining_seconds = recovered;
            if recovered >= runtime.duration_seconds {
                runtime.status = TimerRunStatus::Finished;
                // 池恢复满时同步 recovery_start_pool，避免旧值导致下次按键错误计算
                runtime.recovery_start_pool = runtime.duration_seconds;
            }
            return true;
        }
    }

    if runtime.status != TimerRunStatus::Running {
        return false;
    }

    let Some(ends_at_ms) = runtime.ends_at_ms else {
        return false;
    };

    // Original single-segment timer logic
    let elapsed_seconds = now
        .saturating_sub(runtime.started_at_ms)
        .div_ceil(1000)
        .min(runtime.duration_seconds);
    let remaining_seconds = ends_at_ms.saturating_sub(now).div_ceil(1000);
    let current_seconds = match runtime.direction {
        TimerDirection::Countdown => remaining_seconds,
        TimerDirection::Countup => elapsed_seconds,
    };

    if remaining_seconds == 0 {
        runtime.current_seconds = match runtime.direction {
            TimerDirection::Countdown => 0,
            TimerDirection::Countup => runtime.duration_seconds,
        };
        runtime.remaining_seconds = 0;
        runtime.status = TimerRunStatus::Finished;
        runtime.ends_at_ms = None;
        return true;
    }

    let changed = runtime.current_seconds != current_seconds
        || runtime.remaining_seconds != remaining_seconds;
    runtime.current_seconds = current_seconds;
    runtime.remaining_seconds = remaining_seconds;
    changed
}

fn tick(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<TimerState>();
    let bootstrap = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器状态已损坏".to_string())?;
        let now = now_ms();
        let mut changed = false;
        for runtime in inner.runs.values_mut() {
            changed |= update_timer_runtime(runtime, now);
        }

        if !changed {
            return Ok(());
        }

        inner.bootstrap()
    };

    emit_state(app, bootstrap);
    Ok(())
}

fn trigger_hotkey_targets(
    app: &AppHandle,
    targets: HotkeyTriggerTargets,
) -> Result<TimerBootstrap, String> {
    let state = app.state::<TimerState>();
    let triggered_timer_ids = targets.timer_ids.clone();
    let triggered_counter_ids = targets.counter_ids.clone();
    let bootstrap = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器状态已损坏".to_string())?;

        if !inner.settings.timer_enabled && !inner.settings.counter_enabled {
            return Ok(inner.bootstrap());
        }

        let now = now_ms();
        if inner.settings.timer_enabled {
            for timer_id in &targets.timer_ids {
                let Some(item) = inner
                    .settings
                    .timers
                    .iter()
                    .find(|item| item.id == *timer_id && item.enabled)
                    .map(|item| {
                        (
                            item.id.clone(),
                            item.duration_seconds,
                            item.direction.clone(),
                            item.ignore_running,
                            item.segment_count,
                        )
                    })
                else {
                    continue;
                };

                let (timer_id, duration_seconds, direction, ignore_running, segment_count) = item;

                // Multi-segment timer: pool model, deduct segment_duration, auto-recover
                if let Some(seg_count) = segment_count {
                    if seg_count < 2 {
                        continue;
                    }
                    let total_duration = seg_count as u64 * duration_seconds;

                    // 读取当前 pool（首次触发时为 total_duration）
                    let pool = inner
                        .runs
                        .get(&timer_id)
                        .map(|r| r.remaining_seconds)
                        .unwrap_or(total_duration);
                    if pool < duration_seconds {
                        continue; // not enough pool to deduct
                    }

                    let new_pool = pool - duration_seconds;
                    inner.runs.insert(
                        timer_id,
                        TimerRuntime {
                            started_at_ms: now,
                            ends_at_ms: None,
                            current_seconds: new_pool,
                            remaining_seconds: new_pool,
                            duration_seconds: total_duration,
                            direction: direction.clone(),
                            status: TimerRunStatus::Running,
                            segment_count: seg_count,
                            segment_duration: duration_seconds,
                            recovery_start_pool: new_pool,
                        },
                    );
                    continue;
                }

                // Normal timer: check running state
                let is_running = matches!(
                    inner.runs.get(&timer_id).map(|runtime| &runtime.status),
                    Some(TimerRunStatus::Running)
                );

                if is_running {
                    if ignore_running {
                        continue;
                    }
                    inner.runs.remove(&timer_id);
                }

                let cur = match direction {
                    TimerDirection::Countdown => duration_seconds,
                    TimerDirection::Countup => 0,
                };

                inner.runs.insert(
                    timer_id,
                    TimerRuntime {
                        started_at_ms: now,
                        ends_at_ms: Some(now + duration_seconds * 1000),
                        current_seconds: cur,
                        remaining_seconds: duration_seconds,
                        duration_seconds,
                        direction,
                        status: TimerRunStatus::Running,
                        segment_count: 1,
                        segment_duration: duration_seconds,
                        recovery_start_pool: 0,
                    },
                );
            }
        }

        if inner.settings.counter_enabled {
            let mut counter_changed = false;
            for counter_id in &targets.counter_ids {
                let Some((id, start_value)) = inner
                    .settings
                    .counters
                    .iter()
                    .find(|item| item.id == *counter_id && item.enabled)
                    .map(|counter| (counter.id.clone(), counter.start_value))
                else {
                    continue;
                };
                let value = inner.counter_runs.entry(id).or_insert(start_value);
                *value += 1;
                counter_changed = true;
            }
            if counter_changed {
                persist_counter_runs(app, &inner);
            }
        }

        inner.bootstrap()
    };

    emit_state(app, bootstrap.clone());
    ensure_display_windows(app, &bootstrap.settings)?;
    if !triggered_timer_ids.is_empty() {
        let _ = app.emit_to("main", "timer://hotkey-triggered", triggered_timer_ids);
    }
    if !triggered_counter_ids.is_empty() {
        let _ = app.emit_to("main", "timer://counter-triggered", triggered_counter_ids);
    }
    Ok(bootstrap)
}

fn rect_for_target(settings_value: &TimerSettings, target: &TimerDisplayTarget) -> TimerRect {
    match target {
        TimerDisplayTarget::Timer => settings_value.display.rect.clone(),
        TimerDisplayTarget::Counter => settings_value.counter_display.rect.clone(),
    }
}

fn set_rect_for_target(
    settings_value: &mut TimerSettings,
    target: &TimerDisplayTarget,
    rect: TimerRect,
) {
    match target {
        TimerDisplayTarget::Timer => settings_value.display.rect = rect,
        TimerDisplayTarget::Counter => settings_value.counter_display.rect = rect,
    }
}

fn position_label_for_target(target: &TimerDisplayTarget) -> &'static str {
    match target {
        TimerDisplayTarget::Timer => TIMER_POSITION_LABEL,
        TimerDisplayTarget::Counter => COUNTER_POSITION_LABEL,
    }
}

fn position_mode_for_target(target: &TimerDisplayTarget) -> &'static str {
    match target {
        TimerDisplayTarget::Timer => "timer-position",
        TimerDisplayTarget::Counter => "counter-position",
    }
}

fn position_title_for_target(target: &TimerDisplayTarget) -> &'static str {
    match target {
        TimerDisplayTarget::Timer => "设置计时器位置",
        TimerDisplayTarget::Counter => "设置计数器位置",
    }
}
pub fn initialize(app: &AppHandle, hotkey_manager: &HotkeyManager) -> Result<TimerState, String> {
    let settings = normalize_settings(settings::load_settings(app)?)?;
    let counter_state = counter_state::load(app);
    let mut counter_runs: HashMap<String, i64> = HashMap::new();
    for counter in &settings.counters {
        let value = counter_state
            .runs
            .get(&counter.id)
            .copied()
            .unwrap_or(counter.start_value);
        counter_runs.insert(counter.id.clone(), value);
    }
    // 孤儿 ID（settings.counters 已删、counter_state 还残留）直接丢弃，不污染运行态。

    let state = TimerState {
        inner: Mutex::new(TimerStateInner {
            settings: settings.clone(),
            runs: HashMap::new(),
            counter_runs,
            pending_position: None,
            hotkey_error: None,
        }),
        tick_task: Mutex::new(None),
    };

    if settings.timer_enabled || settings.counter_enabled {
        if let Err(error) = restart_hotkey_listeners(&state, hotkey_manager, &settings) {
            if let Ok(mut inner) = state.inner.lock() {
                inner.hotkey_error = Some(error);
            }
        }
        ensure_display_windows(app, &settings)?;
    }

    start_tick_task(&state, app)?;
    Ok(state)
}

#[tauri::command]
pub fn timer_get_bootstrap(state: State<'_, TimerState>) -> Result<TimerBootstrap, String> {
    let inner = state
        .inner
        .lock()
        .map_err(|_| "计时器状态已损坏".to_string())?;

    Ok(inner.bootstrap())
}

#[tauri::command]
pub fn timer_save_settings(
    settings_value: TimerSettings,
    app: AppHandle,
    state: State<'_, TimerState>,
    hotkey_manager: State<'_, HotkeyManager>,
) -> Result<TimerBootstrap, String> {
    let settings_value = normalize_settings(settings_value)?;
    settings::save_settings(&app, &settings_value)?;

    if let Err(error) = restart_hotkey_listeners(&state, &hotkey_manager, &settings_value) {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器状态已损坏".to_string())?;
        inner.hotkey_error = Some(error.clone());
        return Err(error);
    }

    let bootstrap = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器状态已损坏".to_string())?;
        inner.settings = settings_value.clone();
        inner.hotkey_error = None;
        inner
            .runs
            .retain(|id, _| settings_value.timers.iter().any(|timer| timer.id == *id));
        inner.counter_runs.retain(|id, _| {
            settings_value
                .counters
                .iter()
                .any(|counter| counter.id == *id)
        });
        for counter in &settings_value.counters {
            inner
                .counter_runs
                .entry(counter.id.clone())
                .or_insert(counter.start_value);
        }
        if !settings_value.timer_enabled {
            inner.runs.clear();
        }
        if !settings_value.counter_enabled {
            inner.counter_runs = settings_value
                .counters
                .iter()
                .map(|counter| (counter.id.clone(), counter.start_value))
                .collect();
        }
        // Clear runs for disabled timers
        let enabled_timer_ids: Vec<String> = settings_value
            .timers
            .iter()
            .filter(|t| t.enabled)
            .map(|t| t.id.clone())
            .collect();
        inner.runs.retain(|id, _| enabled_timer_ids.contains(id));
        inner.bootstrap()
    };

    ensure_display_windows(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub fn timer_trigger(timer_ids: Vec<String>, app: AppHandle) -> Result<TimerBootstrap, String> {
    trigger_hotkey_targets(
        &app,
        HotkeyTriggerTargets {
            timer_ids,
            counter_ids: Vec::new(),
        },
    )
}

#[tauri::command]
pub fn timer_counter_trigger(
    counter_ids: Vec<String>,
    app: AppHandle,
) -> Result<TimerBootstrap, String> {
    trigger_hotkey_targets(
        &app,
        HotkeyTriggerTargets {
            timer_ids: Vec::new(),
            counter_ids,
        },
    )
}

#[tauri::command]
pub fn timer_counter_reset(counter_id: String, app: AppHandle) -> Result<TimerBootstrap, String> {
    let state = app.state::<TimerState>();
    let bootstrap = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计数器状态已损坏".to_string())?;
        let Some((id, start_value)) = inner
            .settings
            .counters
            .iter()
            .find(|counter| counter.id == counter_id)
            .map(|counter| (counter.id.clone(), counter.start_value))
        else {
            return Err("未找到计数器".to_string());
        };
        inner.counter_runs.insert(id, start_value);
        persist_counter_runs(&app, &inner);
        inner.bootstrap()
    };

    emit_state(&app, bootstrap.clone());
    ensure_display_windows(&app, &bootstrap.settings)?;
    Ok(bootstrap)
}

#[tauri::command]
pub fn timer_counter_adjust(
    counter_id: String,
    delta: i32,
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerBootstrap, String> {
    let bootstrap = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计数器状态已损坏".to_string())?;

        // 验证 counter 存在且启用
        let exists = inner
            .settings
            .counters
            .iter()
            .any(|c| c.id == counter_id && c.enabled);
        if !exists {
            return Err("计数器不存在或未启用".to_string());
        }

        let start_value = inner
            .settings
            .counters
            .iter()
            .find(|c| c.id == counter_id)
            .map(|c| c.start_value)
            .unwrap_or(0);

        let current = inner.counter_runs.entry(counter_id).or_insert(start_value as i64);
        let new_value = (*current + delta as i64).max(0);
        *current = new_value;

        persist_counter_runs(&app, &inner);
        inner.bootstrap()
    };

    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub async fn timer_begin_position_selection(
    target: TimerDisplayTarget,
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerSelectionOutcome, String> {
    let (sender, receiver) = oneshot::channel();
    let rect = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;

        if inner.pending_position.is_some() {
            return Err("当前已有一个位置设置流程在进行中".to_string());
        }

        let rect = rect_for_target(&inner.settings, &target);
        inner.pending_position = Some(PendingTimerPosition {
            target: target.clone(),
            original_rect: rect.clone(),
            staged_rect: rect.clone(),
            sender,
        });
        rect
    };

    let label = position_label_for_target(&target);
    destroy_window(&app, label);

    let window = WebviewWindowBuilder::new(
        &app,
        label,
        WebviewUrl::App(format!("index.html?mode={}", position_mode_for_target(&target)).into()),
    )
    .title(position_title_for_target(&target))
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(true)
    .visible(true)
    .resizable(false)
    .inner_size(rect.width as f64, rect.height as f64)
    .position(rect.x as f64, rect.y as f64)
    .build()
    .map_err(|error| format!("创建位置设置窗口失败: {error}"))?;

    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Destroyed | WindowEvent::CloseRequested { .. }
        ) {
            let state = close_app.state::<TimerState>();
            if let Ok(mut inner) = state.inner.lock() {
                if let Some(pending) = inner.pending_position.take() {
                    let _ = pending.sender.send(TimerSelectionKind::Closed);
                }
            };
        }
    });

    let kind = match receiver.await {
        Ok(kind) => kind,
        Err(_) => TimerSelectionKind::Closed,
    };
    destroy_window(&app, label);

    let rect = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        rect_for_target(&inner.settings, &target)
    };

    Ok(TimerSelectionOutcome { kind, rect, target })
}

#[tauri::command]
pub fn timer_position_commit(
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerBootstrap, String> {
    let (sender, target, bootstrap) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.take() else {
            return Err("当前没有等待中的位置设置流程".to_string());
        };

        let target = pending.target.clone();
        set_rect_for_target(&mut inner.settings, &target, pending.staged_rect.clone());
        settings::save_settings(&app, &inner.settings)?;
        (pending.sender, target, inner.bootstrap())
    };

    let _ = sender.send(TimerSelectionKind::Selected);
    destroy_window(&app, position_label_for_target(&target));
    ensure_display_windows(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub fn timer_position_cancel(app: AppHandle, state: State<'_, TimerState>) -> Result<(), String> {
    let (sender, target) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.take() else {
            return Err("当前没有等待中的位置设置流程".to_string());
        };

        let target = pending.target.clone();
        set_rect_for_target(&mut inner.settings, &target, pending.original_rect);
        (pending.sender, target)
    };

    let _ = sender.send(TimerSelectionKind::Cancelled);
    destroy_window(&app, position_label_for_target(&target));
    Ok(())
}

#[tauri::command]
pub fn timer_position_moved(
    x: i32,
    y: i32,
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerRect, String> {
    let (rect, target) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.as_mut() else {
            return Err("当前没有等待中的位置设置流程".to_string());
        };

        pending.staged_rect.x = x;
        pending.staged_rect.y = y;
        (pending.staged_rect.clone(), pending.target.clone())
    };

    if let Some(window) = app.get_webview_window(position_label_for_target(&target)) {
        let _ = window.set_position(PhysicalPosition::new(rect.x, rect.y));
    }

    Ok(rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_timer(id: &str, hotkey: &str) -> TimerItem {
        TimerItem {
            id: id.to_string(),
            name: id.to_string(),
            duration_seconds: 30,
            hotkey: hotkey.to_string(),
            direction: TimerDirection::Countdown,
            trigger_mode: TimerTriggerMode::Press,
            enabled: true,
            ignore_running: true,
            segment_count: None,
        }
    }

    fn sample_counter(id: &str, hotkey: &str) -> CounterItem {
        CounterItem {
            id: id.to_string(),
            name: id.to_string(),
            start_value: 0,
            hotkey: hotkey.to_string(),
            enabled: true,
        }
    }

    #[test]
    fn display_height_has_minimum() {
        assert_eq!(display_height(0), TIMER_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(1), TIMER_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(4), 168);
    }

    #[test]
    fn main_window_close_is_app_shutdown_request() {
        assert!(is_main_window_close("main"));
        assert!(!is_main_window_close(TIMER_DISPLAY_LABEL));
        assert!(!is_main_window_close(TIMER_POSITION_LABEL));
        assert!(!is_main_window_close(COUNTER_DISPLAY_LABEL));
        assert!(!is_main_window_close(COUNTER_POSITION_LABEL));
    }

    #[test]
    fn normalize_settings_preserves_custom_width() {
        let mut settings = TimerSettings::default();
        settings.display.rect.width = 480;
        settings.timers = vec![sample_timer("a", "F2"), sample_timer("b", "F3")];
        settings.counters = vec![sample_counter("c", "F4")];

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.display.rect.width, 480);
        assert_eq!(normalized.display.rect.height, 108);
        assert_eq!(
            normalized.counter_display.rect.height,
            TIMER_DISPLAY_MIN_HEIGHT
        );
    }

    #[test]
    fn normalize_settings_rejects_invalid_duration() {
        let mut settings = TimerSettings::default();
        settings.timers[0].duration_seconds = 0;

        let error = normalize_settings(settings).unwrap_err();
        assert!(error.contains("计时秒数"));
    }

    #[test]
    fn update_timer_runtime_finishes_elapsed_countdown_timer() {
        let mut runtime = TimerRuntime {
            started_at_ms: 0,
            ends_at_ms: Some(1_000),
            current_seconds: 1,
            remaining_seconds: 1,
            duration_seconds: 1,
            direction: TimerDirection::Countdown,
            status: TimerRunStatus::Running,
            segment_count: 1,
            segment_duration: 1,
            recovery_start_pool: 0,
        };

        let changed = update_timer_runtime(&mut runtime, 1_000);
        assert!(changed);
        assert_eq!(runtime.current_seconds, 0);
        assert_eq!(runtime.remaining_seconds, 0);
        assert_eq!(runtime.status, TimerRunStatus::Finished);
        assert_eq!(runtime.ends_at_ms, None);
    }

    #[test]
    fn update_timer_runtime_updates_countup_seconds() {
        let mut runtime = TimerRuntime {
            started_at_ms: 0,
            ends_at_ms: Some(5_000),
            current_seconds: 0,
            remaining_seconds: 5,
            duration_seconds: 5,
            direction: TimerDirection::Countup,
            status: TimerRunStatus::Running,
            segment_count: 1,
            segment_duration: 5,
            recovery_start_pool: 0,
        };

        let changed = update_timer_runtime(&mut runtime, 2_001);
        assert!(changed);
        assert_eq!(runtime.current_seconds, 3);
        assert_eq!(runtime.remaining_seconds, 3);
        assert_eq!(runtime.status, TimerRunStatus::Running);
        assert_eq!(runtime.ends_at_ms, Some(5_000));
    }
}
