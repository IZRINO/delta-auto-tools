use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, MutexGuard},
};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};
use tokio::{
    sync::oneshot,
    time::{self, Duration},
};

use crate::app_error::AppError;
use crate::hotkey_types::{HoldAction, HoldActionCallback, HotkeyAction};
use crate::hotkeys::HotkeyManager;
use crate::overlay_utils::{
    destroy_stale_windows, destroy_window, destroy_windows_with_prefix, encoded_query_value,
    hide_window, safe_label_component,
};
use crate::profile::{self, ActiveProfileSnapshotPatch};
use crate::sync_tool::{
    count_enabled_items_by_group, group_enabled, normalize_sync_settings, HotkeyBindingSet,
    RunsSync, SyncGroup, SyncItem, SyncSettings, SyncToolLogic,
};
use crate::tool_base::{ToolLogic, ToolState, ToolStateInner};
use crate::utils::now_ms;

use self::types::{
    TimerDirection, TimerDisplaySettings, TimerGroup, TimerRect, TimerRunState, TimerRunStatus,
    TimerSelectionKind, TimerSelectionOutcome, TimerTriggerMode, DEFAULT_TIMER_GROUP_ID,
};

mod events;
mod settings;
mod types;

// 对外暴露核心类型，供 profile 模块跨工具打包快照用。
pub use self::types::{TimerBootstrap, TimerItem, TimerSettings};

const TIMER_DISPLAY_LABEL: &str = "timer-display";
const TIMER_POSITION_LABEL: &str = "timer-position";
const TIMER_DISPLAY_WIDTH: i32 = 320;
const TIMER_DISPLAY_MIN_HEIGHT: i32 = 96;

pub struct TimerLogic {
    pub runs: HashMap<String, TimerRuntime>,
    pub pending_position: Option<PendingTimerPosition>,
}

pub struct TimerState {
    pub tool: ToolState<TimerLogic>,
    tick_task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl TimerState {
    pub fn lock_inner(&self) -> Result<MutexGuard<'_, ToolStateInner<TimerLogic>>, String> {
        self.tool.lock_inner()
    }
}

pub(crate) struct TimerRuntime {
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

pub(crate) struct PendingTimerPosition {
    group_id: String,
    original_rect: TimerRect,
    staged_rect: TimerRect,
    sender: oneshot::Sender<TimerSelectionKind>,
}

fn run_states(inner: &ToolStateInner<TimerLogic>) -> Vec<TimerRunState> {
    inner
        .settings
        .timers
        .iter()
        .filter_map(|timer| {
            inner.logic.runs.get(&timer.id).map(|runtime| {
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

impl ToolLogic for TimerLogic {
    type Settings = TimerSettings;
    type Bootstrap = TimerBootstrap;
    const NAME: &'static str = "计时器";

    fn load_settings(app: &AppHandle) -> Result<Self::Settings, String> {
        settings::load_settings(app)
    }

    fn save_settings(app: &AppHandle, settings: &Self::Settings) -> Result<(), String> {
        settings::save_settings(app, settings)
    }

    fn build_bootstrap(inner: &ToolStateInner<Self>) -> Self::Bootstrap {
        TimerBootstrap {
            settings: inner.settings.clone(),
            runs: run_states(inner),
            hotkey_error: inner.hotkey_error.clone(),
        }
    }

    fn emit_state<R: tauri::Runtime>(app: &AppHandle<R>, bootstrap: &Self::Bootstrap) {
        let _ = app.emit_to("main", events::STATE_CHANGED, (*bootstrap).clone());
        for group in &bootstrap.settings.timer_groups {
            let _ = app.emit_to(
                display_label_for_group(&group.id),
                events::STATE_CHANGED,
                (*bootstrap).clone(),
            );
        }
    }
}

impl SyncItem for TimerItem {
    fn id(&self) -> &str {
        &self.id
    }
    fn group_id(&self) -> &str {
        &self.group_id
    }
    fn set_group_id(&mut self, group_id: String) {
        self.group_id = group_id;
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl SyncGroup for TimerGroup {
    fn id(&self) -> &str {
        &self.id
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl SyncSettings for TimerSettings {
    type Item = TimerItem;
    type Group = TimerGroup;

    const DEFAULT_GROUP_ID: &'static str = DEFAULT_TIMER_GROUP_ID;
    const DUPLICATE_ITEM_MESSAGE_PREFIX: &'static str = "计时器 ID 重复";

    fn sync_legacy_enabled(&mut self) {
        if self.enabled && !self.timer_enabled {
            self.timer_enabled = true;
        }
        self.enabled = self.timer_enabled;
    }

    fn items(&self) -> &[Self::Item] {
        &self.timers
    }
    fn items_mut(&mut self) -> &mut Vec<Self::Item> {
        &mut self.timers
    }
    fn replace_items(&mut self, items: Vec<Self::Item>) {
        self.timers = items;
    }
    fn normalize_groups(&self) -> Result<Vec<Self::Group>, String> {
        let legacy_display = self.display.clone();
        let timer_count_by_group = count_enabled_items_by_group(&self.timers);
        normalize_timer_groups(
            self.timer_groups.clone(),
            DEFAULT_TIMER_GROUP_ID,
            legacy_display,
            &timer_count_by_group,
        )
    }
    fn replace_groups(&mut self, groups: Vec<Self::Group>) {
        self.timer_groups = groups;
    }
    fn default_item(&self) -> Self::Item {
        TimerItem {
            id: format!("timer-{}", now_ms()),
            group_id: DEFAULT_TIMER_GROUP_ID.to_string(),
            name: "计时器 1".to_string(),
            duration_seconds: 30,
            hotkey: "F2".to_string(),
            direction: TimerDirection::Countdown,
            trigger_mode: TimerTriggerMode::Press,
            enabled: true,
            ignore_running: true,
            segment_count: None,
        }
    }
    fn normalize_item(&self, item: &Self::Item) -> Result<Self::Item, String> {
        normalize_timer(item)
    }
    fn after_groups_normalized(&mut self) {
        self.display = group_display(
            &self.timer_groups,
            DEFAULT_TIMER_GROUP_ID,
            DEFAULT_TIMER_GROUP_ID,
        )
        .cloned()
        .unwrap_or_default();
    }
}

impl SyncToolLogic for TimerLogic {
    const SCOPE: &'static str = "timer";
    const SCOPE_LABEL: &'static str = "计时器";

    fn tool_enabled(settings: &TimerSettings) -> bool {
        settings.timer_enabled
    }

    fn build_hotkey_bindings(settings: &TimerSettings) -> Result<HotkeyBindingSet, String> {
        let mut by_hotkey: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
        for timer in &settings.timers {
            if !timer.enabled || !group_enabled(&settings.timer_groups, &timer.group_id) {
                continue;
            }
            let entry = by_hotkey
                .entry(timer.hotkey.trim().to_string())
                .or_insert_with(|| (Vec::new(), Vec::new()));
            match timer.trigger_mode {
                TimerTriggerMode::Press => entry.0.push(timer.id.clone()),
                TimerTriggerMode::Release => entry.1.push(timer.id.clone()),
            }
        }

        let mut bindings = HotkeyBindingSet::empty();
        for (hotkey, (press_timer_ids, release_timer_ids)) in by_hotkey {
            if !release_timer_ids.is_empty() {
                let press_targets = press_timer_ids.clone();
                let release_targets = release_timer_ids.clone();
                let hold_callback: HoldActionCallback =
                    std::sync::Arc::new(move |app_handle, action| {
                        let targets = match action {
                            HoldAction::Down => press_targets.clone(),
                            HoldAction::Up => release_targets.clone(),
                        };
                        if targets.is_empty() {
                            return;
                        }
                        let app = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(error) = trigger_hotkey_targets(&app, targets) {
                                let _ = app.emit_to("main", events::HOTKEY_ERROR, error);
                            }
                        });
                    });
                bindings.hold.push((hotkey, hold_callback));
            } else {
                let targets = press_timer_ids.clone();
                let action: HotkeyAction = std::sync::Arc::new(move |app_handle| {
                    let targets = targets.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = trigger_hotkey_targets(&app_handle, targets) {
                            let _ = app_handle.emit_to("main", events::HOTKEY_ERROR, error);
                        }
                    });
                });
                bindings.normal.push((hotkey, action));
            }
        }
        Ok(bindings)
    }

    fn stop_all(app: &AppHandle) -> Result<(), String> {
        let Some(state) = app.try_state::<TimerState>() else {
            return Ok(());
        };
        stop_all(app, &state);
        Ok(())
    }
}

impl RunsSync for TimerLogic {
    type Runs = HashMap<String, TimerRuntime>;

    fn sync_runs_with_settings(runs: &mut Self::Runs, settings: &Self::Settings) {
        // 1. retain(id ∈ settings.timers) — 孤儿清理
        runs.retain(|id, _| settings.timers.iter().any(|t| t.id == *id));
        // 2. 缺失补齐 — 新计时器插入 Finished 状态的 idle runtime
        // 不重置、不按 enabled 清理、不按 timer_enabled 清空
        for timer in &settings.timers {
            if !runs.contains_key(&timer.id) {
                let cur = match timer.direction {
                    TimerDirection::Countdown => timer.duration_seconds,
                    TimerDirection::Countup => 0,
                };
                runs.insert(
                    timer.id.clone(),
                    TimerRuntime {
                        started_at_ms: 0,
                        ends_at_ms: None,
                        current_seconds: cur,
                        remaining_seconds: timer.duration_seconds,
                        duration_seconds: timer.duration_seconds,
                        direction: timer.direction.clone(),
                        status: TimerRunStatus::Finished,
                        segment_count: 1,
                        segment_duration: timer.duration_seconds,
                        recovery_start_pool: 0,
                    },
                );
            }
        }
    }
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

fn normalize_timer_groups(
    mut groups: Vec<TimerGroup>,
    default_group_id: &str,
    legacy_display: TimerDisplaySettings,
    item_count_by_group: &HashMap<String, usize>,
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
            return Err("计时器分组名称不能为空".to_string());
        }
        if seen.insert(group.id.clone(), true).is_some() {
            return Err(format!("计时器分组 ID 重复: {}", group.id));
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

pub(crate) fn normalize_settings(settings_value: TimerSettings) -> Result<TimerSettings, String> {
    normalize_sync_settings(settings_value)
}

pub(crate) fn restart_hotkey_listeners(
    state: &TimerState,
    hotkey_manager: &HotkeyManager,
    settings_value: &TimerSettings,
) -> Result<(), String> {
    state
        .tool
        .restart_sync_hotkeys(hotkey_manager, settings_value)
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

pub(crate) fn emit_state(app: &AppHandle, bootstrap: TimerBootstrap) {
    TimerLogic::emit_state(app, &bootstrap);
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
    .map_err(|error| format!("创建{title}透明窗口失败: {error}"))?;

    let _ = window.set_ignore_cursor_events(true);
    Ok(())
}

pub(crate) fn ensure_display_windows(
    app: &AppHandle,
    settings_value: &TimerSettings,
) -> Result<(), String> {
    let mut active_labels = HashSet::new();

    for group in &settings_value.timer_groups {
        let label = display_label_for_group(&group.id);
        active_labels.insert(label.clone());
        let mut display = group.display.clone();
        display.rect.height =
            display_height(enabled_timer_count_for_group(settings_value, &group.id));
        ensure_overlay_window(
            app,
            &label,
            &display_query_for_group(&group.id),
            &format!("计时器透明窗口 - {}", group.name),
            &display,
            settings_value.timer_enabled && group.enabled,
        )?;
    }
    destroy_stale_windows(app, TIMER_DISPLAY_LABEL, &active_labels);
    Ok(())
}

fn display_label_for_group(group_id: &str) -> String {
    if group_id == DEFAULT_TIMER_GROUP_ID {
        TIMER_DISPLAY_LABEL.to_string()
    } else {
        format!("{}-{}", TIMER_DISPLAY_LABEL, safe_label_component(group_id))
    }
}

fn display_query_for_group(group_id: &str) -> String {
    let group_id = encoded_query_value(group_id);
    format!("timer-display&groupId={group_id}")
}

fn destroy_display_windows(app: &AppHandle) {
    destroy_windows_with_prefix(app, TIMER_DISPLAY_LABEL);
}

fn destroy_position_windows(app: &AppHandle) {
    destroy_windows_with_prefix(app, TIMER_POSITION_LABEL);
}

fn position_label_for_group(group_id: &str) -> String {
    if group_id == DEFAULT_TIMER_GROUP_ID {
        TIMER_POSITION_LABEL.to_string()
    } else {
        format!(
            "{}-{}",
            TIMER_POSITION_LABEL,
            safe_label_component(group_id)
        )
    }
}

fn position_mode_for_group(group_id: &str) -> String {
    let group_id = encoded_query_value(group_id);
    format!("timer-position&groupId={group_id}")
}

fn update_timer_runtime(runtime: &mut TimerRuntime, now: u64) -> bool {
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
    let Some(state) = app.try_state::<TimerState>() else {
        // 状态尚未注册（setup 期间），跳过本次 tick
        return Ok(());
    };
    let bootstrap = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计时器状态已损坏".to_string())?;
        let now = now_ms();
        let mut changed = false;
        for runtime in inner.logic.runs.values_mut() {
            changed |= update_timer_runtime(runtime, now);
        }

        if !changed {
            return Ok(());
        }

        TimerLogic::build_bootstrap(&inner)
    };

    emit_state(app, bootstrap);
    Ok(())
}

fn trigger_hotkey_targets(
    app: &AppHandle,
    timer_ids: Vec<String>,
) -> Result<TimerBootstrap, String> {
    let state = app.state::<TimerState>();
    let triggered_timer_ids = timer_ids.clone();
    let bootstrap = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计时器状态已损坏".to_string())?;

        if !inner.settings.timer_enabled {
            return Ok(TimerLogic::build_bootstrap(&inner));
        }

        let now = now_ms();
        for timer_id in timer_ids {
            let Some(item) = inner
                .settings
                .timers
                .iter()
                .find(|item| {
                    item.id == timer_id
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

            if let Some(seg_count) = segment_count {
                if seg_count < 2 {
                    continue;
                }
                let total_duration = (seg_count as u64).saturating_mul(duration_seconds);
                let Some(next_runtime) = trigger_multisegment_runtime(
                    inner.logic.runs.get_mut(&timer_id),
                    now,
                    total_duration,
                    duration_seconds,
                    direction,
                    seg_count,
                ) else {
                    continue;
                };
                inner.logic.runs.insert(timer_id, next_runtime);
                continue;
            }

            let is_running = matches!(
                inner
                    .logic
                    .runs
                    .get(&timer_id)
                    .map(|runtime| &runtime.status),
                Some(TimerRunStatus::Running)
            );

            if is_running {
                if ignore_running {
                    continue;
                }
                inner.logic.runs.remove(&timer_id);
            }

            let cur = match direction {
                TimerDirection::Countdown => duration_seconds,
                TimerDirection::Countup => 0,
            };

            inner.logic.runs.insert(
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

        TimerLogic::build_bootstrap(&inner)
    };

    emit_state(app, bootstrap.clone());
    ensure_display_windows(app, &bootstrap.settings)?;
    if !triggered_timer_ids.is_empty() {
        let _ = app.emit_to("main", events::HOTKEY_TRIGGERED, triggered_timer_ids);
    }
    Ok(bootstrap)
}

fn rect_for_group(settings_value: &TimerSettings, group_id: &str) -> TimerRect {
    group_display(
        &settings_value.timer_groups,
        DEFAULT_TIMER_GROUP_ID,
        group_id,
    )
    .map(|display| display.rect.clone())
    .unwrap_or_else(|| settings_value.display.rect.clone())
}

fn set_rect_for_group(settings_value: &mut TimerSettings, group_id: &str, rect: TimerRect) {
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

pub fn is_main_window_close(label: &str) -> bool {
    label == "main"
}

pub fn shutdown(app: &AppHandle, state: &TimerState, hotkey_manager: &HotkeyManager) {
    let _ = hotkey_manager.clear_scope("timer");
    let _ = hotkey_manager.clear_hold_scope("timer");
    let _ = stop_tick_task(state);
    destroy_position_windows(app);
    destroy_display_windows(app);
}

pub fn stop_all(app: &AppHandle, state: &TimerState) {
    let bootstrap = {
        let Ok(mut inner) = state.lock_inner() else {
            return;
        };
        inner.logic.runs.clear();
        TimerLogic::build_bootstrap(&inner)
    };
    // 全局开关关闭时只隐藏透明窗口（不销毁），重新打开时 ensure_display_windows 直接 show 恢复，
    // 避免窗口重建导致的 label 冲突与加载空白。
    crate::overlay_utils::hide_windows_with_prefix(app, TIMER_DISPLAY_LABEL);
    emit_state(app, bootstrap);
}

pub(crate) fn stop_registered(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<TimerState>() else {
        return Ok(());
    };
    stop_all(app, &state);
    Ok(())
}

pub fn initialize(app: &AppHandle, hotkey_manager: &HotkeyManager) -> Result<TimerState, String> {
    let settings = normalize_settings(settings::load_settings(app)?)?;
    let logic = TimerLogic {
        runs: HashMap::new(),
        pending_position: None,
    };
    let tool = ToolState::new(logic, settings.clone());
    let state = TimerState {
        tool,
        tick_task: Mutex::new(None),
    };

    if settings.timer_enabled {
        if let Err(error) = restart_hotkey_listeners(&state, hotkey_manager, &settings) {
            if let Ok(mut inner) = state.lock_inner() {
                inner.hotkey_error = Some(error);
            }
        }
        ensure_display_windows(app, &settings)?;
    }

    start_tick_task(&state, app)?;
    Ok(state)
}

#[tauri::command]
pub fn timer_get_bootstrap(state: State<'_, TimerState>) -> Result<TimerBootstrap, AppError> {
    let inner = state
        .lock_inner()
        .map_err(|_| "计时器状态已损坏".to_string())?;

    Ok(TimerLogic::build_bootstrap(&inner))
}

#[tauri::command]
pub fn timer_save_settings(
    settings_value: TimerSettings,
    app: AppHandle,
    state: State<'_, TimerState>,
    hotkey_manager: State<'_, HotkeyManager>,
) -> Result<TimerBootstrap, AppError> {
    let settings_value = normalize_settings(settings_value)?;
    settings::save_settings(&app, &settings_value)?;

    if let Err(error) = restart_hotkey_listeners(&state, &hotkey_manager, &settings_value) {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计时器状态已损坏".to_string())?;
        inner.hotkey_error = Some(error.clone());
        return Err(AppError::from(error));
    }

    let bootstrap = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计时器状态已损坏".to_string())?;
        inner.settings = settings_value.clone();
        inner.hotkey_error = None;
        TimerLogic::sync_runs_with_settings(&mut inner.logic.runs, &settings_value);
        TimerLogic::build_bootstrap(&inner)
    };

    ensure_display_windows(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    profile::update_active_profile_snapshot(
        &app,
        ActiveProfileSnapshotPatch::Timer(bootstrap.settings.clone()),
    )?;
    Ok(bootstrap)
}

#[tauri::command]
pub fn timer_trigger(timer_ids: Vec<String>, app: AppHandle) -> Result<TimerBootstrap, AppError> {
    trigger_hotkey_targets(&app, timer_ids).map_err(AppError::from)
}

#[tauri::command]
pub async fn timer_begin_position_selection(
    group_id: Option<String>,
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerSelectionOutcome, AppError> {
    let (sender, receiver) = oneshot::channel();
    let group_id = group_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_TIMER_GROUP_ID.to_string());
    let rect = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;

        if inner.logic.pending_position.is_some() {
            return Err(AppError::Message(
                "当前已有一个位置设置流程在进行中".to_string(),
            ));
        }

        let rect = rect_for_group(&inner.settings, &group_id);
        inner.logic.pending_position = Some(PendingTimerPosition {
            group_id: group_id.clone(),
            original_rect: rect.clone(),
            staged_rect: rect.clone(),
            sender,
        });
        rect
    };

    let label = position_label_for_group(&group_id);
    destroy_window(&app, &label);

    let window = WebviewWindowBuilder::new(
        &app,
        &label,
        WebviewUrl::App(format!("index.html?mode={}", position_mode_for_group(&group_id)).into()),
    )
    .title("设置计时器位置")
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
    .map_err(|error| format!("创建位置设置窗口失败: {}", error))?;

    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Destroyed | WindowEvent::CloseRequested { .. }
        ) {
            let state = close_app.state::<TimerState>();
            if let Ok(mut inner) = state.lock_inner() {
                if let Some(pending) = inner.logic.pending_position.take() {
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
            .lock_inner()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        rect_for_group(&inner.settings, &group_id)
    };

    Ok(TimerSelectionOutcome {
        kind,
        rect,
        group_id: Some(group_id),
    })
}

#[tauri::command]
pub fn timer_position_commit(
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerBootstrap, AppError> {
    let (sender, group_id, bootstrap) = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.logic.pending_position.take() else {
            return Err(AppError::Message(
                "当前没有等待中的位置设置流程".to_string(),
            ));
        };

        let group_id = pending.group_id.clone();
        set_rect_for_group(&mut inner.settings, &group_id, pending.staged_rect.clone());
        settings::save_settings(&app, &inner.settings)?;
        (
            pending.sender,
            group_id,
            TimerLogic::build_bootstrap(&inner),
        )
    };

    let _ = sender.send(TimerSelectionKind::Selected);
    destroy_window(&app, &position_label_for_group(&group_id));
    ensure_display_windows(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    profile::update_active_profile_snapshot(
        &app,
        ActiveProfileSnapshotPatch::Timer(bootstrap.settings.clone()),
    )?;
    Ok(bootstrap)
}

#[tauri::command]
pub fn timer_position_cancel(app: AppHandle, state: State<'_, TimerState>) -> Result<(), AppError> {
    let (sender, group_id) = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.logic.pending_position.take() else {
            return Err(AppError::Message(
                "当前没有等待中的位置设置流程".to_string(),
            ));
        };

        let group_id = pending.group_id.clone();
        set_rect_for_group(&mut inner.settings, &group_id, pending.original_rect);
        (pending.sender, group_id)
    };

    let _ = sender.send(TimerSelectionKind::Cancelled);
    destroy_window(&app, &position_label_for_group(&group_id));
    Ok(())
}

#[tauri::command]
pub fn timer_position_moved(
    x: i32,
    y: i32,
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerRect, AppError> {
    let (rect, group_id) = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.logic.pending_position.as_mut() else {
            return Err(AppError::Message(
                "当前没有等待中的位置设置流程".to_string(),
            ));
        };

        pending.staged_rect.x = x;
        pending.staged_rect.y = y;
        (pending.staged_rect.clone(), pending.group_id.clone())
    };

    if let Some(window) = app.get_webview_window(&position_label_for_group(&group_id)) {
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
    }

    #[test]
    fn normalize_settings_preserves_custom_width() {
        let mut settings = TimerSettings::default();
        settings.display.rect.width = 480;
        settings.timers = vec![sample_timer("a", "F2"), sample_timer("b", "F3")];

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.display.rect.width, 480);
        assert_eq!(normalized.display.rect.height, 108);
    }

    #[test]
    fn normalize_settings_migrates_legacy_groups() {
        let mut settings = TimerSettings::default();
        settings.display.rect.width = 480;
        settings.timer_groups.clear();
        settings.timers = vec![sample_timer("a", "F2")];

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.timer_groups.len(), 1);
        assert_eq!(normalized.timer_groups[0].id, DEFAULT_TIMER_GROUP_ID);
        assert_eq!(normalized.timer_groups[0].display.rect.width, 480);
        assert_eq!(normalized.timers[0].group_id, DEFAULT_TIMER_GROUP_ID);
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

    // ── sync_runs_with_settings 4 场景单测 ───────────────────────
    // 对齐 counter 的 4 场景：孤儿清理、缺失补齐、禁用保留、全局关闭保留。

    #[test]
    fn test_timer_save_removes_orphan_runs() {
        // 孤儿 runs（settings.timers 中不存在的 id）被清理
        let mut runs = HashMap::new();
        runs.insert(
            "orphan-99".to_string(),
            TimerRuntime {
                started_at_ms: 0,
                ends_at_ms: None,
                current_seconds: 30,
                remaining_seconds: 30,
                duration_seconds: 30,
                direction: TimerDirection::Countdown,
                status: TimerRunStatus::Finished,
                segment_count: 1,
                segment_duration: 30,
                recovery_start_pool: 0,
            },
        );
        runs.insert(
            "t".to_string(),
            TimerRuntime {
                started_at_ms: 0,
                ends_at_ms: None,
                current_seconds: 30,
                remaining_seconds: 30,
                duration_seconds: 30,
                direction: TimerDirection::Countdown,
                status: TimerRunStatus::Running,
                segment_count: 1,
                segment_duration: 30,
                recovery_start_pool: 0,
            },
        );

        let timer = sample_timer("t", "F2");
        let settings = TimerSettings {
            timers: vec![timer],
            ..TimerSettings::default()
        };

        TimerLogic::sync_runs_with_settings(&mut runs, &settings);

        assert!(!runs.contains_key("orphan-99"), "孤儿 runs 应被清理");
        assert!(runs.contains_key("t"), "有效计时器 runs 应保留");
    }

    #[test]
    fn test_timer_save_inserts_missing_runs() {
        // settings.timers 中存在但 runs 中缺失的 id 被补齐
        let mut runs = HashMap::new();

        let mut timer = sample_timer("t", "F2");
        timer.duration_seconds = 60;
        let settings = TimerSettings {
            timers: vec![timer],
            ..TimerSettings::default()
        };

        TimerLogic::sync_runs_with_settings(&mut runs, &settings);

        assert!(runs.contains_key("t"), "缺失计时器应补齐");
        let runtime = runs.get("t").unwrap();
        assert_eq!(runtime.duration_seconds, 60);
        assert_eq!(runtime.status, TimerRunStatus::Finished, "补齐的运行应为 Finished 状态");
    }

    #[test]
    fn test_timer_save_retains_disabled_timer_runs() {
        // 禁用计时器（enabled=false）的 runs 保留累积值，不被清除
        let mut runs = HashMap::new();
        runs.insert(
            "a".to_string(),
            TimerRuntime {
                started_at_ms: 1000,
                ends_at_ms: None,
                current_seconds: 10,
                remaining_seconds: 20,
                duration_seconds: 30,
                direction: TimerDirection::Countdown,
                status: TimerRunStatus::Running,
                segment_count: 1,
                segment_duration: 30,
                recovery_start_pool: 0,
            },
        );
        runs.insert(
            "b".to_string(),
            TimerRuntime {
                started_at_ms: 2000,
                ends_at_ms: None,
                current_seconds: 5,
                remaining_seconds: 25,
                duration_seconds: 30,
                direction: TimerDirection::Countdown,
                status: TimerRunStatus::Running,
                segment_count: 1,
                segment_duration: 30,
                recovery_start_pool: 0,
            },
        );

        let timer_a = sample_timer("a", "F2");
        let mut timer_b = sample_timer("b", "F3");
        timer_b.enabled = false;

        let settings = TimerSettings {
            timers: vec![timer_a, timer_b],
            ..TimerSettings::default()
        };

        TimerLogic::sync_runs_with_settings(&mut runs, &settings);

        assert_eq!(runs.get("a").unwrap().remaining_seconds, 20, "启用计时器 runs 应保留");
        assert_eq!(runs.get("b").unwrap().remaining_seconds, 25, "禁用计时器 runs 应保留");
    }

    #[test]
    fn test_timer_save_disabled_keeps_runs() {
        // 全局关闭（timer_enabled=false）时 runs 保留累积值，不重置/不清理
        let mut runs = HashMap::new();
        runs.insert(
            "t".to_string(),
            TimerRuntime {
                started_at_ms: 5000,
                ends_at_ms: None,
                current_seconds: 42,
                remaining_seconds: 18,
                duration_seconds: 60,
                direction: TimerDirection::Countdown,
                status: TimerRunStatus::Running,
                segment_count: 1,
                segment_duration: 60,
                recovery_start_pool: 0,
            },
        );

        let timer = sample_timer("t", "F2");
        let settings = TimerSettings {
            timer_enabled: false,
            timers: vec![timer],
            ..TimerSettings::default()
        };

        TimerLogic::sync_runs_with_settings(&mut runs, &settings);

        assert_eq!(
            runs.get("t").unwrap().current_seconds, 42,
            "全局关闭时 runs 应保留累积值，不重置"
        );
        assert_eq!(
            runs.get("t").unwrap().remaining_seconds, 18,
            "全局关闭时 runs 应保留剩余秒数"
        );
    }
}
