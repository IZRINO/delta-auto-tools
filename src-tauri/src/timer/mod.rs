use std::{
    collections::{HashMap, HashSet},
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
use crate::overlay_utils::{destroy_stale_windows, destroy_window, destroy_windows_with_prefix, encoded_query_value, hide_window, safe_label_component};
use crate::utils::now_ms;

use self::counter_state::CounterState;
use self::types::{
    CounterItem, CounterRunState, TimerBootstrap, TimerDirection, TimerDisplaySettings,
    TimerDisplayTarget, TimerGroup, TimerItem, TimerRect, TimerRunState, TimerRunStatus,
    TimerSelectionKind, TimerSelectionOutcome, TimerSettings, TimerTriggerMode,
    DEFAULT_COUNTER_GROUP_ID, DEFAULT_TIMER_GROUP_ID,
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
    group_id: String,
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

fn default_group(id: &str, name: &str, display: TimerDisplaySettings) -> TimerGroup {
    TimerGroup {
        id: id.to_string(),
        name: name.to_string(),
        enabled: true,
        display,
    }
}

fn normalize_groups(
    mut groups: Vec<TimerGroup>,
    default_group_id: &str,
    legacy_display: TimerDisplaySettings,
    item_count_by_group: &HashMap<String, usize>,
    label: &str,
) -> Result<Vec<TimerGroup>, String> {
    if groups.is_empty() {
        groups.push(default_group(
            default_group_id,
            "默认分组",
            legacy_display.clone(),
        ));
    }

    if !groups.iter().any(|group| group.id == default_group_id) {
        groups.insert(
            0,
            default_group(default_group_id, "默认分组", legacy_display.clone()),
        );
    }

    let mut seen = HashMap::new();
    let mut normalized = Vec::with_capacity(groups.len());
    for mut group in groups {
        group.id = group.id.trim().to_string();
        if group.id.is_empty() {
            group.id = default_group_id.to_string();
        }
        group.name = group.name.trim().to_string();
        if group.name.is_empty() {
            return Err(format!("{label}分组名称不能为空"));
        }
        if seen.insert(group.id.clone(), true).is_some() {
            return Err(format!("{label}分组 ID 重复: {}", group.id));
        }

        if group.id == default_group_id {
            group.display = legacy_display.clone();
        }

        normalize_display(
            &mut group.display,
            item_count_by_group.get(&group.id).copied().unwrap_or(0),
        )?;
        normalized.push(group);
    }

    Ok(normalized)
}

fn group_enabled(groups: &[TimerGroup], group_id: &str) -> bool {
    groups
        .iter()
        .find(|group| group.id == group_id)
        .map(|group| group.enabled)
        .unwrap_or(false)
}

fn group_display<'a>(
    groups: &'a [TimerGroup],
    default_group_id: &str,
    group_id: &str,
) -> Option<&'a TimerDisplaySettings> {
    groups
        .iter()
        .find(|group| group.id == group_id)
        .or_else(|| groups.iter().find(|group| group.id == default_group_id))
        .map(|group| &group.display)
}

fn enabled_timer_count_for_group(settings_value: &TimerSettings, group_id: &str) -> usize {
    settings_value
        .timers
        .iter()
        .filter(|timer| {
            timer.enabled
                && timer.group_id == group_id
                && group_enabled(&settings_value.timer_groups, group_id)
        })
        .count()
}

fn enabled_counter_count_for_group(settings_value: &TimerSettings, group_id: &str) -> usize {
    settings_value
        .counters
        .iter()
        .filter(|counter| {
            counter.enabled
                && counter.group_id == group_id
                && group_enabled(&settings_value.counter_groups, group_id)
        })
        .count()
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
        group_id: timer.group_id.trim().to_string(),
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
        group_id: counter.group_id.trim().to_string(),
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
    let legacy_timer_display = settings_value.display.clone();
    let legacy_counter_display = settings_value.counter_display.clone();

    if settings_value.timers.is_empty() {
        settings_value.timers.push(TimerItem {
            id: format!("timer-{}", crate::utils::now_ms()),
            group_id: DEFAULT_TIMER_GROUP_ID.to_string(),
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
            group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
            name: "计数器 1".to_string(),
            start_value: 0,
            hotkey: "F3".to_string(),
            enabled: true,
        });
    }

    let raw_timer_group_ids = group_id_set(&settings_value.timer_groups, DEFAULT_TIMER_GROUP_ID);
    let raw_counter_group_ids =
        group_id_set(&settings_value.counter_groups, DEFAULT_COUNTER_GROUP_ID);

    let mut seen_ids = HashMap::new();
    let mut timers = Vec::with_capacity(settings_value.timers.len());
    for timer in &settings_value.timers {
        let mut normalized = normalize_timer(timer)?;
        if !raw_timer_group_ids.contains_key(&normalized.group_id) {
            normalized.group_id = DEFAULT_TIMER_GROUP_ID.to_string();
        }
        if seen_ids.insert(normalized.id.clone(), true).is_some() {
            return Err(format!("计时器 ID 重复: {}", normalized.id));
        }
        timers.push(normalized);
    }

    let mut counters = Vec::with_capacity(settings_value.counters.len());
    for counter in &settings_value.counters {
        let mut normalized = normalize_counter(counter)?;
        if !raw_counter_group_ids.contains_key(&normalized.group_id) {
            normalized.group_id = DEFAULT_COUNTER_GROUP_ID.to_string();
        }
        if seen_ids.insert(normalized.id.clone(), true).is_some() {
            return Err(format!("计时/计数器 ID 重复: {}", normalized.id));
        }
        counters.push(normalized);
    }

    settings_value.timers = timers;
    settings_value.counters = counters;
    let timer_count_by_group = count_enabled_timers_by_group(&settings_value.timers);
    let counter_count_by_group = count_enabled_counters_by_group(&settings_value.counters);
    settings_value.timer_groups = normalize_groups(
        settings_value.timer_groups,
        DEFAULT_TIMER_GROUP_ID,
        legacy_timer_display,
        &timer_count_by_group,
        "计时器",
    )?;
    settings_value.counter_groups = normalize_groups(
        settings_value.counter_groups,
        DEFAULT_COUNTER_GROUP_ID,
        legacy_counter_display,
        &counter_count_by_group,
        "计数器",
    )?;
    settings_value.display = group_display(
        &settings_value.timer_groups,
        DEFAULT_TIMER_GROUP_ID,
        DEFAULT_TIMER_GROUP_ID,
    )
    .cloned()
    .unwrap_or_default();
    settings_value.counter_display = group_display(
        &settings_value.counter_groups,
        DEFAULT_COUNTER_GROUP_ID,
        DEFAULT_COUNTER_GROUP_ID,
    )
    .cloned()
    .unwrap_or_else(|| TimerDisplaySettings {
        rect: TimerRect {
            x: 420,
            y: 80,
            width: TIMER_DISPLAY_WIDTH,
            height: TIMER_DISPLAY_MIN_HEIGHT,
        },
        font_opacity: 0.92,
    });
    Ok(settings_value)
}

fn group_id_set(groups: &[TimerGroup], default_group_id: &str) -> HashMap<String, bool> {
    let mut map = HashMap::new();
    map.insert(default_group_id.to_string(), true);
    for group in groups {
        let id = group.id.trim();
        if !id.is_empty() {
            map.insert(id.to_string(), true);
        }
    }
    map
}

fn count_enabled_timers_by_group(timers: &[TimerItem]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for timer in timers {
        if timer.enabled {
            *map.entry(timer.group_id.clone()).or_insert(0) += 1;
        }
    }
    map
}

fn count_enabled_counters_by_group(counters: &[CounterItem]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for counter in counters {
        if counter.enabled {
            *map.entry(counter.group_id.clone()).or_insert(0) += 1;
        }
    }
    map
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
            if !timer.enabled || !group_enabled(&settings_value.timer_groups, &timer.group_id) {
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
            if !counter.enabled || !group_enabled(&settings_value.counter_groups, &counter.group_id)
            {
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
    for group in &bootstrap.settings.timer_groups {
        let _ = app.emit_to(
            display_label_for_group(&TimerDisplayTarget::Timer, &group.id),
            "timer://state-changed",
            bootstrap.clone(),
        );
    }
    for group in &bootstrap.settings.counter_groups {
        let _ = app.emit_to(
            display_label_for_group(&TimerDisplayTarget::Counter, &group.id),
            "timer://state-changed",
            bootstrap.clone(),
        );
    }
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
    let mut active_labels = HashSet::new();

    for group in &settings_value.timer_groups {
        let label = display_label_for_group(&TimerDisplayTarget::Timer, &group.id);
        active_labels.insert(label.clone());
        let mut display = group.display.clone();
        display.rect.height =
            display_height(enabled_timer_count_for_group(settings_value, &group.id));
        ensure_overlay_window(
            app,
            &label,
            &display_query_for_group(&TimerDisplayTarget::Timer, &group.id),
            &format!("计时器透明窗口 - {}", group.name),
            &display,
            settings_value.timer_enabled && group.enabled,
        )?;
    }
    destroy_stale_windows(app, TIMER_DISPLAY_LABEL, &active_labels);

    let mut active_labels = HashSet::new();
    for group in &settings_value.counter_groups {
        let label = display_label_for_group(&TimerDisplayTarget::Counter, &group.id);
        active_labels.insert(label.clone());
        let mut display = group.display.clone();
        display.rect.height =
            display_height(enabled_counter_count_for_group(settings_value, &group.id));
        ensure_overlay_window(
            app,
            &label,
            &display_query_for_group(&TimerDisplayTarget::Counter, &group.id),
            &format!("计数器透明窗口 - {}", group.name),
            &display,
            settings_value.counter_enabled && group.enabled,
        )?;
    }
    destroy_stale_windows(app, COUNTER_DISPLAY_LABEL, &active_labels);
    Ok(())
}

fn display_label_for_group(target: &TimerDisplayTarget, group_id: &str) -> String {
    match target {
        TimerDisplayTarget::Timer if group_id == DEFAULT_TIMER_GROUP_ID => {
            TIMER_DISPLAY_LABEL.to_string()
        }
        TimerDisplayTarget::Timer => {
            format!("{}-{}", TIMER_DISPLAY_LABEL, safe_label_component(group_id))
        }
        TimerDisplayTarget::Counter if group_id == DEFAULT_COUNTER_GROUP_ID => {
            COUNTER_DISPLAY_LABEL.to_string()
        }
        TimerDisplayTarget::Counter => {
            format!(
                "{}-{}",
                COUNTER_DISPLAY_LABEL,
                safe_label_component(group_id)
            )
        }
    }
}

fn display_query_for_group(target: &TimerDisplayTarget, group_id: &str) -> String {
    let group_id = encoded_query_value(group_id);
    match target {
        TimerDisplayTarget::Timer => {
            format!("timer-display&groupId={group_id}")
        }
        TimerDisplayTarget::Counter => {
            format!("counter-display&groupId={group_id}")
        }
    }
}

fn destroy_display_windows(app: &AppHandle) {
    destroy_windows_with_prefix(app, TIMER_DISPLAY_LABEL);
    destroy_windows_with_prefix(app, COUNTER_DISPLAY_LABEL);
}

fn destroy_position_windows(app: &AppHandle) {
    destroy_windows_with_prefix(app, TIMER_POSITION_LABEL);
    destroy_windows_with_prefix(app, COUNTER_POSITION_LABEL);
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

fn multisegment_pool_ms(runtime: Option<&TimerRuntime>, now: u64, total_duration: u64) -> u64 {
    let total_ms = total_duration.saturating_mul(1000);
    runtime
        .and_then(|r| {
            if r.segment_count <= 1 {
                None
            } else if r.status == TimerRunStatus::Finished {
                Some(total_ms)
            } else {
                let elapsed_ms = now.saturating_sub(r.started_at_ms);
                Some(
                    r.recovery_start_pool
                        .saturating_mul(1000)
                        .saturating_add(elapsed_ms)
                        .min(total_ms),
                )
            }
        })
        .unwrap_or(total_ms)
}

fn deduct_multisegment_pool(
    runtime: Option<&TimerRuntime>,
    now: u64,
    total_duration: u64,
    segment_duration: u64,
) -> Option<(u64, u64)> {
    let exact_pool_ms = multisegment_pool_ms(runtime, now, total_duration);
    let duration_ms = segment_duration.saturating_mul(1000);
    if exact_pool_ms < duration_ms {
        return None;
    }

    let new_exact_pool_ms = exact_pool_ms - duration_ms;
    let new_pool = new_exact_pool_ms / 1000;
    let new_started_at_ms = now.saturating_sub(new_exact_pool_ms % 1000);
    Some((new_pool, new_started_at_ms))
}

fn trigger_multisegment_runtime(
    runtime: Option<&mut TimerRuntime>,
    now: u64,
    total_duration: u64,
    segment_duration: u64,
    direction: TimerDirection,
    segment_count: u32,
) -> Option<TimerRuntime> {
    let runtime = runtime.map(|runtime| {
        update_timer_runtime(runtime, now);
        &*runtime
    });
    let (new_pool, new_started_at_ms) =
        deduct_multisegment_pool(runtime, now, total_duration, segment_duration)?;

    Some(TimerRuntime {
        started_at_ms: new_started_at_ms,
        ends_at_ms: None,
        current_seconds: new_pool,
        remaining_seconds: new_pool,
        duration_seconds: total_duration,
        direction,
        status: TimerRunStatus::Running,
        segment_count,
        segment_duration,
        recovery_start_pool: new_pool,
    })
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
                    .find(|item| {
                        item.id == *timer_id
                            && item.enabled
                            && group_enabled(&inner.settings.timer_groups, &item.group_id)
                    })
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
                    let total_duration = (seg_count as u64).saturating_mul(duration_seconds);

                    // 触发前先按 tick 同源逻辑归一化 runtime，再用毫秒级 pool 扣段。
                    // 这样按键落在 250ms tick 间隙、Finished 满池或 999ms 余量时，
                    // 后续显示状态和扣除起点保持一致。
                    let Some(next_runtime) = trigger_multisegment_runtime(
                        inner.runs.get_mut(&timer_id),
                        now,
                        total_duration,
                        duration_seconds,
                        direction,
                        seg_count,
                    ) else {
                        continue;
                    };

                    inner.runs.insert(timer_id, next_runtime);
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
                    .find(|item| {
                        item.id == *counter_id
                            && item.enabled
                            && group_enabled(&inner.settings.counter_groups, &item.group_id)
                    })
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

fn default_group_id_for_target(target: &TimerDisplayTarget) -> &'static str {
    match target {
        TimerDisplayTarget::Timer => DEFAULT_TIMER_GROUP_ID,
        TimerDisplayTarget::Counter => DEFAULT_COUNTER_GROUP_ID,
    }
}

fn rect_for_target(
    settings_value: &TimerSettings,
    target: &TimerDisplayTarget,
    group_id: &str,
) -> TimerRect {
    match target {
        TimerDisplayTarget::Timer => group_display(
            &settings_value.timer_groups,
            DEFAULT_TIMER_GROUP_ID,
            group_id,
        )
        .map(|display| display.rect.clone())
        .unwrap_or_else(|| settings_value.display.rect.clone()),
        TimerDisplayTarget::Counter => group_display(
            &settings_value.counter_groups,
            DEFAULT_COUNTER_GROUP_ID,
            group_id,
        )
        .map(|display| display.rect.clone())
        .unwrap_or_else(|| settings_value.counter_display.rect.clone()),
    }
}

fn set_rect_for_target(
    settings_value: &mut TimerSettings,
    target: &TimerDisplayTarget,
    group_id: &str,
    rect: TimerRect,
) {
    match target {
        TimerDisplayTarget::Timer => {
            if let Some(group) = settings_value
                .timer_groups
                .iter_mut()
                .find(|group| group.id == group_id)
            {
                group.display.rect = rect.clone();
            }
            if group_id == DEFAULT_TIMER_GROUP_ID {
                settings_value.display.rect = rect;
            }
        }
        TimerDisplayTarget::Counter => {
            if let Some(group) = settings_value
                .counter_groups
                .iter_mut()
                .find(|group| group.id == group_id)
            {
                group.display.rect = rect.clone();
            }
            if group_id == DEFAULT_COUNTER_GROUP_ID {
                settings_value.counter_display.rect = rect;
            }
        }
    }
}

fn position_label_for_target(target: &TimerDisplayTarget, group_id: &str) -> String {
    match target {
        TimerDisplayTarget::Timer if group_id == DEFAULT_TIMER_GROUP_ID => {
            TIMER_POSITION_LABEL.to_string()
        }
        TimerDisplayTarget::Timer => {
            format!(
                "{}-{}",
                TIMER_POSITION_LABEL,
                safe_label_component(group_id)
            )
        }
        TimerDisplayTarget::Counter if group_id == DEFAULT_COUNTER_GROUP_ID => {
            COUNTER_POSITION_LABEL.to_string()
        }
        TimerDisplayTarget::Counter => {
            format!(
                "{}-{}",
                COUNTER_POSITION_LABEL,
                safe_label_component(group_id)
            )
        }
    }
}

fn position_mode_for_target(target: &TimerDisplayTarget, group_id: &str) -> String {
    let group_id = encoded_query_value(group_id);
    match target {
        TimerDisplayTarget::Timer => format!("timer-position&groupId={group_id}"),
        TimerDisplayTarget::Counter => format!("counter-position&groupId={group_id}"),
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
            .filter(|t| t.enabled && group_enabled(&settings_value.timer_groups, &t.group_id))
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

        let current = inner
            .counter_runs
            .entry(counter_id)
            .or_insert(start_value as i64);
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
    group_id: Option<String>,
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerSelectionOutcome, String> {
    let (sender, receiver) = oneshot::channel();
    let group_id = group_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_group_id_for_target(&target).to_string());
    let rect = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;

        if inner.pending_position.is_some() {
            return Err("当前已有一个位置设置流程在进行中".to_string());
        }

        let rect = rect_for_target(&inner.settings, &target, &group_id);
        inner.pending_position = Some(PendingTimerPosition {
            target: target.clone(),
            group_id: group_id.clone(),
            original_rect: rect.clone(),
            staged_rect: rect.clone(),
            sender,
        });
        rect
    };

    let label = position_label_for_target(&target, &group_id);
    destroy_window(&app, &label);

    let window = WebviewWindowBuilder::new(
        &app,
        &label,
        WebviewUrl::App(
            format!(
                "index.html?mode={}",
                position_mode_for_target(&target, &group_id)
            )
            .into(),
        ),
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
    destroy_window(&app, &label);

    let rect = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        rect_for_target(&inner.settings, &target, &group_id)
    };

    Ok(TimerSelectionOutcome {
        kind,
        rect,
        target,
        group_id: Some(group_id),
    })
}

#[tauri::command]
pub fn timer_position_commit(
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerBootstrap, String> {
    let (sender, target, group_id, bootstrap) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.take() else {
            return Err("当前没有等待中的位置设置流程".to_string());
        };

        let target = pending.target.clone();
        let group_id = pending.group_id.clone();
        set_rect_for_target(
            &mut inner.settings,
            &target,
            &group_id,
            pending.staged_rect.clone(),
        );
        settings::save_settings(&app, &inner.settings)?;
        (pending.sender, target, group_id, inner.bootstrap())
    };

    let _ = sender.send(TimerSelectionKind::Selected);
    destroy_window(&app, &position_label_for_target(&target, &group_id));
    ensure_display_windows(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub fn timer_position_cancel(app: AppHandle, state: State<'_, TimerState>) -> Result<(), String> {
    let (sender, target, group_id) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.take() else {
            return Err("当前没有等待中的位置设置流程".to_string());
        };

        let target = pending.target.clone();
        let group_id = pending.group_id.clone();
        set_rect_for_target(
            &mut inner.settings,
            &target,
            &group_id,
            pending.original_rect,
        );
        (pending.sender, target, group_id)
    };

    let _ = sender.send(TimerSelectionKind::Cancelled);
    destroy_window(&app, &position_label_for_target(&target, &group_id));
    Ok(())
}

#[tauri::command]
pub fn timer_position_moved(
    x: i32,
    y: i32,
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerRect, String> {
    let (rect, target, group_id) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.as_mut() else {
            return Err("当前没有等待中的位置设置流程".to_string());
        };

        pending.staged_rect.x = x;
        pending.staged_rect.y = y;
        (
            pending.staged_rect.clone(),
            pending.target.clone(),
            pending.group_id.clone(),
        )
    };

    if let Some(window) = app.get_webview_window(&position_label_for_target(&target, &group_id)) {
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
            group_id: DEFAULT_TIMER_GROUP_ID.to_string(),
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
            group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
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
    fn normalize_settings_migrates_legacy_groups() {
        let mut settings = TimerSettings::default();
        settings.display.rect.width = 480;
        settings.counter_display.rect.width = 520;
        settings.timer_groups.clear();
        settings.counter_groups.clear();
        settings.timers = vec![sample_timer("a", "F2")];
        settings.counters = vec![sample_counter("c", "F3")];

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.timer_groups.len(), 1);
        assert_eq!(normalized.counter_groups.len(), 1);
        assert_eq!(normalized.timer_groups[0].id, DEFAULT_TIMER_GROUP_ID);
        assert_eq!(normalized.counter_groups[0].id, DEFAULT_COUNTER_GROUP_ID);
        assert_eq!(normalized.timer_groups[0].display.rect.width, 480);
        assert_eq!(normalized.counter_groups[0].display.rect.width, 520);
        assert_eq!(normalized.timers[0].group_id, DEFAULT_TIMER_GROUP_ID);
        assert_eq!(normalized.counters[0].group_id, DEFAULT_COUNTER_GROUP_ID);
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

    #[test]
    fn deduct_multisegment_pool_accepts_trigger_between_backend_ticks() {
        let runtime = TimerRuntime {
            started_at_ms: 0,
            ends_at_ms: None,
            current_seconds: 59,
            remaining_seconds: 59,
            duration_seconds: 300,
            direction: TimerDirection::Countup,
            status: TimerRunStatus::Running,
            segment_count: 5,
            segment_duration: 60,
            recovery_start_pool: 0,
        };

        let deduction = deduct_multisegment_pool(Some(&runtime), 60_000, 300, 60);

        assert_eq!(deduction, Some((0, 60_000)));
    }

    #[test]
    fn deduct_multisegment_pool_preserves_fractional_recovery() {
        let runtime = TimerRuntime {
            started_at_ms: 0,
            ends_at_ms: None,
            current_seconds: 120,
            remaining_seconds: 120,
            duration_seconds: 300,
            direction: TimerDirection::Countup,
            status: TimerRunStatus::Running,
            segment_count: 5,
            segment_duration: 60,
            recovery_start_pool: 60,
        };

        let deduction = deduct_multisegment_pool(Some(&runtime), 60_999, 300, 60);

        assert_eq!(deduction, Some((60, 60_000)));
        let (pool, started_at_ms) = deduction.unwrap();
        let next_runtime = TimerRuntime {
            recovery_start_pool: pool,
            started_at_ms,
            ..runtime
        };
        assert_eq!(
            multisegment_pool_ms(Some(&next_runtime), 60_999, 300),
            60_999
        );
    }

    #[test]
    fn deduct_multisegment_pool_treats_finished_runtime_as_full_pool() {
        let runtime = TimerRuntime {
            started_at_ms: 1_000,
            ends_at_ms: None,
            current_seconds: 120,
            remaining_seconds: 120,
            duration_seconds: 300,
            direction: TimerDirection::Countdown,
            status: TimerRunStatus::Finished,
            segment_count: 5,
            segment_duration: 60,
            recovery_start_pool: 120,
        };

        let deduction = deduct_multisegment_pool(Some(&runtime), 123_456, 300, 60);

        assert_eq!(deduction, Some((240, 123_456)));
    }

    fn trigger_multisegment_at(
        runtime: Option<TimerRuntime>,
        now: u64,
        direction: TimerDirection,
    ) -> Option<TimerRuntime> {
        let mut runtime = runtime;
        trigger_multisegment_runtime(runtime.as_mut(), now, 300, 60, direction, 5)
    }

    fn running_multisegment(
        pool: u64,
        started_at_ms: u64,
        direction: TimerDirection,
    ) -> TimerRuntime {
        TimerRuntime {
            started_at_ms,
            ends_at_ms: None,
            current_seconds: pool,
            remaining_seconds: pool,
            duration_seconds: 300,
            direction,
            status: TimerRunStatus::Running,
            segment_count: 5,
            segment_duration: 60,
            recovery_start_pool: pool,
        }
    }

    #[test]
    fn multisegment_trigger_normalizes_recovered_pool_before_deducting_countup() {
        let direction = TimerDirection::Countup;
        let first = trigger_multisegment_at(None, 0, direction.clone()).unwrap();
        assert_eq!(first.current_seconds, 240);
        assert_eq!(first.recovery_start_pool, 240);

        let second = trigger_multisegment_at(Some(first), 0, direction.clone()).unwrap();
        assert_eq!(second.current_seconds, 180);

        let third = trigger_multisegment_at(Some(second), 0, direction.clone()).unwrap();
        assert_eq!(third.current_seconds, 120);

        let fourth = trigger_multisegment_at(Some(third), 120_000, direction).unwrap();
        assert_eq!(fourth.current_seconds, 180);
        assert_eq!(fourth.remaining_seconds, 180);
        assert_eq!(fourth.recovery_start_pool, 180);
        assert_eq!(fourth.started_at_ms, 120_000);
        assert_eq!(fourth.status, TimerRunStatus::Running);
    }

    #[test]
    fn multisegment_trigger_normalizes_recovered_pool_before_deducting_countdown() {
        let direction = TimerDirection::Countdown;
        let first = trigger_multisegment_at(None, 0, direction.clone()).unwrap();
        let second = trigger_multisegment_at(Some(first), 0, direction.clone()).unwrap();
        let third = trigger_multisegment_at(Some(second), 0, direction.clone()).unwrap();

        let fourth = trigger_multisegment_at(Some(third), 120_000, direction).unwrap();

        assert_eq!(fourth.current_seconds, 180);
        assert_eq!(fourth.remaining_seconds, 180);
        assert_eq!(fourth.recovery_start_pool, 180);
        assert_eq!(fourth.started_at_ms, 120_000);
        assert_eq!(fourth.status, TimerRunStatus::Running);
    }

    #[test]
    fn multisegment_trigger_treats_finished_runtime_as_full_pool() {
        let runtime = TimerRuntime {
            status: TimerRunStatus::Finished,
            current_seconds: 300,
            remaining_seconds: 300,
            recovery_start_pool: 300,
            ..running_multisegment(120, 1_000, TimerDirection::Countdown)
        };

        let next =
            trigger_multisegment_at(Some(runtime), 123_456, TimerDirection::Countdown).unwrap();

        assert_eq!(next.current_seconds, 240);
        assert_eq!(next.remaining_seconds, 240);
        assert_eq!(next.recovery_start_pool, 240);
        assert_eq!(next.started_at_ms, 123_456);
    }

    #[test]
    fn multisegment_trigger_preserves_999ms_recovery_remainder() {
        let runtime = running_multisegment(120, 0, TimerDirection::Countup);

        let next = trigger_multisegment_at(Some(runtime), 60_999, TimerDirection::Countup).unwrap();

        assert_eq!(next.current_seconds, 120);
        assert_eq!(next.remaining_seconds, 120);
        assert_eq!(next.recovery_start_pool, 120);
        assert_eq!(next.started_at_ms, 60_000);
        assert_eq!(multisegment_pool_ms(Some(&next), 60_999, 300), 120_999);
    }
}
