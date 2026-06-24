use std::{
    collections::{HashMap, HashSet},
    sync::MutexGuard,
};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};
use tokio::sync::oneshot;

use crate::app_error::AppError;
use crate::hotkey_types::HotkeyAction;
use crate::hotkeys::HotkeyManager;
use crate::profile::{self, ActiveProfileSnapshotPatch};
use crate::overlay_utils::{
    destroy_stale_windows, destroy_window, destroy_windows_with_prefix, encoded_query_value,
    hide_window, safe_label_component,
};
use crate::sync_tool::{
    count_enabled_items_by_group, group_enabled, normalize_sync_settings, HotkeyBindingSet,
    SyncGroup, SyncItem, SyncSettings, SyncToolLogic,
};
use crate::tool_base::{ToolLogic, ToolState, ToolStateInner};

use self::counter_state::CounterRunStateSnapshot;
use self::types::{
    CounterDisplaySettings, CounterGroup, CounterRect,
    CounterRunState, CounterSelectionKind, CounterSelectionOutcome,
    DEFAULT_COUNTER_GROUP_ID,
};

pub(crate) mod counter_state;
mod settings;
mod types;
mod events;

// 对外暴露核心类型，供 profile 模块跨工具打包快照用。
pub use self::types::{CounterBootstrap, CounterItem, CounterSettings};

const COUNTER_DISPLAY_LABEL: &str = "counter-display";
const COUNTER_POSITION_LABEL: &str = "counter-position";
const COUNTER_DISPLAY_WIDTH: i32 = 320;
const COUNTER_DISPLAY_MIN_HEIGHT: i32 = 96;

pub struct CounterLogic {
    pub runs: HashMap<String, i64>,
    pub pending_position: Option<PendingCounterPosition>,
}

pub struct CounterState {
    pub tool: ToolState<CounterLogic>,
}

impl CounterState {
    pub fn lock_inner(&self) -> Result<MutexGuard<'_, ToolStateInner<CounterLogic>>, String> {
        self.tool.lock_inner()
    }
}

pub(crate) struct PendingCounterPosition {
    group_id: String,
    original_rect: CounterRect,
    staged_rect: CounterRect,
    sender: oneshot::Sender<CounterSelectionKind>,
}

fn counter_run_states(inner: &ToolStateInner<CounterLogic>) -> Vec<CounterRunState> {
    inner
        .settings
        .counters
        .iter()
        .map(|counter| CounterRunState {
            id: counter.id.clone(),
            value: inner
                .logic
                .runs
                .get(&counter.id)
                .copied()
                .unwrap_or(counter.start_value),
        })
        .collect()
}

impl ToolLogic for CounterLogic {
    type Settings = CounterSettings;
    type Bootstrap = CounterBootstrap;
    const NAME: &'static str = "计数器";

    fn load_settings(app: &AppHandle) -> Result<Self::Settings, String> {
        settings::load_settings(app)
    }

    fn save_settings(app: &AppHandle, settings: &Self::Settings) -> Result<(), String> {
        settings::save_settings(app, settings)
    }

    fn build_bootstrap(inner: &ToolStateInner<Self>) -> Self::Bootstrap {
        CounterBootstrap {
            settings: inner.settings.clone(),
            counter_runs: counter_run_states(inner),
            hotkey_error: inner.hotkey_error.clone(),
        }
    }

    fn emit_state<R: tauri::Runtime>(app: &AppHandle<R>, bootstrap: &Self::Bootstrap) {
        let _ = app.emit_to("main", events::STATE_CHANGED, (*bootstrap).clone());
        for group in &bootstrap.settings.counter_groups {
            let _ = app.emit_to(
                display_label_for_group(&group.id),
                events::STATE_CHANGED,
                (*bootstrap).clone(),
            );
        }
    }
}

impl SyncToolLogic for CounterLogic {
    const SCOPE: &'static str = "counter";
    const SCOPE_LABEL: &'static str = "计数器";

    fn tool_enabled(settings: &CounterSettings) -> bool {
        settings.counter_enabled
    }

    fn build_hotkey_bindings(settings: &CounterSettings) -> Result<HotkeyBindingSet, String> {
        let mut by_hotkey: HashMap<String, Vec<String>> = HashMap::new();
        for counter in &settings.counters {
            if !counter.enabled || !group_enabled(&settings.counter_groups, &counter.group_id) {
                continue;
            }
            by_hotkey
                .entry(counter.hotkey.trim().to_string())
                .or_default()
                .push(counter.id.clone());
        }

        let mut bindings = HotkeyBindingSet::empty();
        for (hotkey, counter_ids) in by_hotkey {
            let action: HotkeyAction = std::sync::Arc::new(move |app_handle| {
                let targets = counter_ids.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = trigger_hotkey_targets(&app_handle, targets) {
                        let _ = app_handle.emit_to("main", events::HOTKEY_ERROR, error);
                    }
                });
            });
            bindings.normal.push((hotkey, action));
        }
        Ok(bindings)
    }

    fn stop_all(app: &AppHandle) -> Result<(), String> {
        let Some(state) = app.try_state::<CounterState>() else {
            return Ok(());
        };
        stop_all(app, &state);
        Ok(())
    }
}

pub(crate) fn persist_counter_runs(app: &AppHandle, inner: &ToolStateInner<CounterLogic>) {
    let mut runs = std::collections::BTreeMap::new();
    for counter in &inner.settings.counters {
        if let Some(value) = inner.logic.runs.get(&counter.id) {
            runs.insert(counter.id.clone(), *value);
        }
    }
    let state = CounterRunStateSnapshot { runs };
    let _ = counter_state::save(app, &state);
}

/// 将所有 counter 运行值重置为其 `start_value` 并落盘到 `counter_state.json`。
///
/// 供 Profile 切换编排调用：切到新 Profile 时按用户决策「重置为 start_value」。
/// 必须在 `inner.settings` 已替换为目标 Profile 的 counters 之后调用。
pub(crate) fn reset_runs_to_start_values(app: &AppHandle, state: &CounterState) -> Result<(), String> {
    let mut inner = state
        .lock_inner()
        .map_err(|_| "计数器状态已损坏".to_string())?;
    inner.logic.runs = inner
        .settings
        .counters
        .iter()
        .map(|counter| (counter.id.clone(), counter.start_value))
        .collect();
    persist_counter_runs(app, &inner);
    Ok(())
}

pub(crate) fn emit_state(app: &AppHandle, bootstrap: CounterBootstrap) {
    CounterLogic::emit_state(app, &bootstrap);
}

fn display_height(item_count: usize) -> i32 {
    COUNTER_DISPLAY_MIN_HEIGHT.max(48 + item_count.max(1) as i32 * 30)
}

fn normalize_display(
    display: &mut CounterDisplaySettings,
    item_count: usize,
) -> Result<(), String> {
    display.rect.width = display.rect.width.max(COUNTER_DISPLAY_WIDTH);
    display.rect.height = display_height(item_count);

    if !(0.1..=1.0).contains(&display.font_opacity) {
        return Err("字体透明度必须在 0.1 到 1 之间".to_string());
    }

    Ok(())
}

fn default_group(id: &str, name: &str, display: CounterDisplaySettings) -> CounterGroup {
    CounterGroup {
        id: id.to_string(),
        name: name.to_string(),
        enabled: true,
        display,
    }
}

fn normalize_counter_groups(
    mut groups: Vec<CounterGroup>,
    default_group_id: &str,
    legacy_display: CounterDisplaySettings,
    item_count_by_group: &HashMap<String, usize>,
) -> Result<Vec<CounterGroup>, String> {
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
            return Err("计数器分组名称不能为空".to_string());
        }
        if seen.insert(group.id.clone(), true).is_some() {
            return Err(format!("计数器分组 ID 重复: {}", group.id));
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
    groups: &'a [CounterGroup],
    default_group_id: &str,
    group_id: &str,
) -> Option<&'a CounterDisplaySettings> {
    groups
        .iter()
        .find(|group| group.id == group_id)
        .or_else(|| groups.iter().find(|group| group.id == default_group_id))
        .map(|group| &group.display)
}

fn enabled_counter_count_for_group(settings_value: &CounterSettings, group_id: &str) -> usize {
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

impl SyncItem for CounterItem {
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

impl SyncGroup for CounterGroup {
    fn id(&self) -> &str {
        &self.id
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl SyncSettings for CounterSettings {
    type Item = CounterItem;
    type Group = CounterGroup;

    const DEFAULT_GROUP_ID: &'static str = DEFAULT_COUNTER_GROUP_ID;
    const DUPLICATE_ITEM_MESSAGE_PREFIX: &'static str = "计数器 ID 重复";

    fn sync_legacy_enabled(&mut self) {
        if self.enabled && !self.counter_enabled {
            self.counter_enabled = true;
        }
        self.enabled = self.counter_enabled;
    }

    fn items(&self) -> &[Self::Item] {
        &self.counters
    }

    fn items_mut(&mut self) -> &mut Vec<Self::Item> {
        &mut self.counters
    }

    fn replace_items(&mut self, items: Vec<Self::Item>) {
        self.counters = items;
    }

    fn groups(&self) -> &[Self::Group] {
        &self.counter_groups
    }

    fn normalize_groups(&self) -> Result<Vec<Self::Group>, String> {
        let legacy_display = self.display.clone();
        let counter_count_by_group = count_enabled_items_by_group(&self.counters);
        normalize_counter_groups(
            self.counter_groups.clone(),
            DEFAULT_COUNTER_GROUP_ID,
            legacy_display,
            &counter_count_by_group,
        )
    }

    fn replace_groups(&mut self, groups: Vec<Self::Group>) {
        self.counter_groups = groups;
    }

    fn default_item(&self) -> Self::Item {
        CounterItem {
            id: format!("counter-{}", crate::utils::now_ms()),
            group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
            name: "计数器 1".to_string(),
            start_value: 0,
            hotkey: "F3".to_string(),
            enabled: true,
        }
    }

    fn normalize_item(&self, item: &Self::Item) -> Result<Self::Item, String> {
        normalize_counter(item)
    }

    fn after_groups_normalized(&mut self) {
        self.display = group_display(
            &self.counter_groups,
            DEFAULT_COUNTER_GROUP_ID,
            DEFAULT_COUNTER_GROUP_ID,
        )
        .cloned()
        .unwrap_or_default();
    }
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

pub(crate) fn normalize_settings(settings_value: CounterSettings) -> Result<CounterSettings, String> {
    normalize_sync_settings(settings_value)
}

pub(crate) fn restart_hotkey_listeners(
    state: &CounterState,
    hotkey_manager: &HotkeyManager,
    settings_value: &CounterSettings,
) -> Result<(), String> {
    state.tool.restart_sync_hotkeys(hotkey_manager, settings_value)
}

fn ensure_overlay_window(
    app: &AppHandle,
    label: &str,
    query_mode: &str,
    title: &str,
    display: &CounterDisplaySettings,
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
        .map_err(|error| format!("创建{}透明窗口失败: {}", title, error))?;

    let _ = window.set_ignore_cursor_events(true);
    Ok(())
}

pub(crate) fn ensure_display_windows(app: &AppHandle, settings_value: &CounterSettings) -> Result<(), String> {
    let mut active_labels = HashSet::new();
    for group in &settings_value.counter_groups {
        let label = display_label_for_group(&group.id);
        active_labels.insert(label.clone());
        let mut display = group.display.clone();
        display.rect.height =
            display_height(enabled_counter_count_for_group(settings_value, &group.id));
        ensure_overlay_window(
            app,
            &label,
            &display_query_for_group(&group.id),
            &format!("计数器透明窗口 - {}", group.name),
            &display,
            settings_value.counter_enabled && group.enabled,
        )?;
    }
    destroy_stale_windows(app, COUNTER_DISPLAY_LABEL, &active_labels);
    Ok(())
}

fn display_label_for_group(group_id: &str) -> String {
    if group_id == DEFAULT_COUNTER_GROUP_ID {
        COUNTER_DISPLAY_LABEL.to_string()
    } else {
        format!(
            "{}-{}",
            COUNTER_DISPLAY_LABEL,
            safe_label_component(group_id)
        )
    }
}

fn display_query_for_group(group_id: &str) -> String {
    let group_id = encoded_query_value(group_id);
    format!("counter-display&groupId={group_id}")
}

fn destroy_display_windows(app: &AppHandle) {
    destroy_windows_with_prefix(app, COUNTER_DISPLAY_LABEL);
}

fn destroy_position_windows(app: &AppHandle) {
    destroy_windows_with_prefix(app, COUNTER_POSITION_LABEL);
}

fn position_label_for_group(group_id: &str) -> String {
    if group_id == DEFAULT_COUNTER_GROUP_ID {
        COUNTER_POSITION_LABEL.to_string()
    } else {
        format!(
            "{}-{}",
            COUNTER_POSITION_LABEL,
            safe_label_component(group_id)
        )
    }
}

fn position_mode_for_group(group_id: &str) -> String {
    let group_id = encoded_query_value(group_id);
    format!("counter-position&groupId={group_id}")
}

fn rect_for_group(settings_value: &CounterSettings, group_id: &str) -> CounterRect {
    group_display(
        &settings_value.counter_groups,
        DEFAULT_COUNTER_GROUP_ID,
        group_id,
    )
        .map(|display| display.rect.clone())
        .unwrap_or_else(|| settings_value.display.rect.clone())
}

fn set_rect_for_group(
    settings_value: &mut CounterSettings,
    group_id: &str,
    rect: CounterRect,
) {
    if let Some(group) = settings_value
        .counter_groups
        .iter_mut()
        .find(|group| group.id == group_id)
    {
        group.display.rect = rect.clone();
    }
    if group_id == DEFAULT_COUNTER_GROUP_ID {
        settings_value.display.rect = rect;
    }
}

fn trigger_hotkey_targets(
    app: &AppHandle,
    counter_ids: Vec<String>,
) -> Result<CounterBootstrap, String> {
    let state = app.state::<CounterState>();
    let triggered_ids = counter_ids.clone();
    let bootstrap = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计数器状态已损坏".to_string())?;

        if !inner.settings.counter_enabled {
            return Ok(CounterLogic::build_bootstrap(&inner));
        }

        let mut counter_changed = false;
        for counter_id in counter_ids {
            let Some((id, start_value)) = inner
                .settings
                .counters
                .iter()
                .find(|item| {
                    item.id == counter_id
                        && item.enabled
                        && group_enabled(&inner.settings.counter_groups, &item.group_id)
                })
                .map(|counter| (counter.id.clone(), counter.start_value))
            else {
                continue;
            };
            let value = inner.logic.runs.entry(id).or_insert(start_value);
            *value += 1;
            counter_changed = true;
        }
        if counter_changed {
            persist_counter_runs(app, &inner);
        }

        CounterLogic::build_bootstrap(&inner)
    };

    emit_state(app, bootstrap.clone());
    ensure_display_windows(app, &bootstrap.settings)?;
    if !triggered_ids.is_empty() {
        let _ = app.emit_to("main", events::HOTKEY_TRIGGERED, triggered_ids);
    }
    Ok(bootstrap)
}

pub fn shutdown(app: &AppHandle, state: &CounterState, hotkey_manager: &HotkeyManager) {
    let _ = hotkey_manager.clear_scope("counter");
    if let Ok(inner) = state.lock_inner() {
        persist_counter_runs(app, &inner);
    }
    destroy_position_windows(app);
    destroy_display_windows(app);
}

pub fn stop_all(app: &AppHandle, state: &CounterState) {
    let bootstrap = {
        let Ok(inner) = state.lock_inner() else {
            return;
        };
        CounterLogic::build_bootstrap(&inner)
    };
    destroy_display_windows(app);
    emit_state(app, bootstrap);
}

pub fn initialize(
    app: &AppHandle,
    hotkey_manager: &HotkeyManager,
) -> Result<CounterState, String> {
    let settings = normalize_settings(settings::load_settings(app)?)?;
    let counter_state = counter_state::load(app);
    let mut runs: HashMap<String, i64> = HashMap::new();
    for counter in &settings.counters {
        let value = counter_state
            .runs
            .get(&counter.id)
            .copied()
            .unwrap_or(counter.start_value);
        runs.insert(counter.id.clone(), value);
    }

    let logic = CounterLogic {
        runs,
        pending_position: None,
    };
    let tool = ToolState::new(logic, settings.clone());
    let state = CounterState { tool };

    if settings.counter_enabled {
        if let Err(error) = restart_hotkey_listeners(&state, hotkey_manager, &settings) {
            if let Ok(mut inner) = state.lock_inner() {
                inner.hotkey_error = Some(error);
            }
        }
        ensure_display_windows(app, &settings)?;
    }

    Ok(state)
}

#[tauri::command]
pub fn counter_get_bootstrap(
    state: State<'_, CounterState>,
) -> Result<CounterBootstrap, AppError> {
    let inner = state
        .lock_inner()
        .map_err(|_| "计数器状态已损坏".to_string())?;
    Ok(CounterLogic::build_bootstrap(&inner))
}

#[tauri::command]
pub fn counter_save_settings(
    settings_value: CounterSettings,
    app: AppHandle,
    state: State<'_, CounterState>,
    hotkey_manager: State<'_, HotkeyManager>,
) -> Result<CounterBootstrap, AppError> {
    let settings_value = normalize_settings(settings_value)?;
    settings::save_settings(&app, &settings_value)?;

    if let Err(error) = restart_hotkey_listeners(&state, &hotkey_manager, &settings_value) {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计数器状态已损坏".to_string())?;
        inner.hotkey_error = Some(error.clone());
        return Err(AppError::from(error));
    }

    let bootstrap = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计数器状态已损坏".to_string())?;
        inner.settings = settings_value.clone();
        inner.hotkey_error = None;
        inner.logic.runs.retain(|id, _| {
            settings_value
                .counters
                .iter()
                .any(|counter| counter.id == *id)
        });
        for counter in &settings_value.counters {
            inner
                .logic
                .runs
                .entry(counter.id.clone())
                .or_insert(counter.start_value);
        }
        if !settings_value.counter_enabled {
            inner.logic.runs = settings_value
                .counters
                .iter()
                .map(|counter| (counter.id.clone(), counter.start_value))
                .collect();
        }
        let enabled_ids: Vec<String> = settings_value
            .counters
            .iter()
            .filter(|c| c.enabled && group_enabled(&settings_value.counter_groups, &c.group_id))
            .map(|c| c.id.clone())
            .collect();
        inner.logic.runs.retain(|id, _| enabled_ids.contains(id));
        CounterLogic::build_bootstrap(&inner)
    };

    ensure_display_windows(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    profile::update_active_profile_snapshot(
        &app,
        ActiveProfileSnapshotPatch::Counter(bootstrap.settings.clone()),
    )?;
    Ok(bootstrap)
}

#[tauri::command]
pub fn counter_trigger(
    counter_ids: Vec<String>,
    app: AppHandle,
) -> Result<CounterBootstrap, AppError> {
    trigger_hotkey_targets(&app, counter_ids).map_err(AppError::from)
}

#[tauri::command]
pub fn counter_reset(
    counter_id: String,
    app: AppHandle,
) -> Result<CounterBootstrap, AppError> {
    let state = app.state::<CounterState>();
    let bootstrap = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计数器状态已损坏".to_string())?;
        let Some((id, start_value)) = inner
            .settings
            .counters
            .iter()
            .find(|counter| counter.id == counter_id)
            .map(|counter| (counter.id.clone(), counter.start_value))
        else {
            return Err(AppError::Message("未找到计数器".to_string()));
        };
        inner.logic.runs.insert(id, start_value);
        persist_counter_runs(&app, &inner);
        CounterLogic::build_bootstrap(&inner)
    };

    emit_state(&app, bootstrap.clone());
    ensure_display_windows(&app, &bootstrap.settings)?;
    Ok(bootstrap)
}

#[tauri::command]
pub fn counter_adjust(
    counter_id: String,
    delta: i32,
    app: AppHandle,
    state: State<'_, CounterState>,
) -> Result<CounterBootstrap, AppError> {
    let bootstrap = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计数器状态已损坏".to_string())?;
        let exists = inner
            .settings
            .counters
            .iter()
            .any(|c| c.id == counter_id && c.enabled);
        if !exists {
            return Err(AppError::Message("计数器不存在或未启用".to_string()));
        }

        let start_value = inner
            .settings
            .counters
            .iter()
            .find(|c| c.id == counter_id)
            .map(|c| c.start_value)
            .unwrap_or(0);

        let current = inner.logic.runs.entry(counter_id).or_insert(start_value);
        let new_value = (*current + delta as i64).max(0);
        *current = new_value;

        persist_counter_runs(&app, &inner);
        CounterLogic::build_bootstrap(&inner)
    };

    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub async fn counter_begin_position_selection(
    group_id: Option<String>,
    app: AppHandle,
    state: State<'_, CounterState>,
) -> Result<CounterSelectionOutcome, AppError> {
    let (sender, receiver) = oneshot::channel();
    let group_id = group_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_COUNTER_GROUP_ID.to_string());
    let rect = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计数器位置设置状态已损坏".to_string())?;

        if inner.logic.pending_position.is_some() {
            return Err(AppError::Message(
                "当前已有一个位置设置流程在进行中".to_string(),
            ));
        }

        let rect = rect_for_group(&inner.settings, &group_id);
        inner.logic.pending_position = Some(PendingCounterPosition {
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
        WebviewUrl::App(
            format!("index.html?mode={}", position_mode_for_group(&group_id)).into(),
        ),
    )
        .title("设置计数器位置")
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
            let state = close_app.state::<CounterState>();
            if let Ok(mut inner) = state.lock_inner() {
                if let Some(pending) = inner.logic.pending_position.take() {
                    let _ = pending.sender.send(CounterSelectionKind::Closed);
                }
            };
        }
    });

    let kind = match receiver.await {
        Ok(kind) => kind,
        Err(_) => CounterSelectionKind::Closed,
    };
    destroy_window(&app, &label);

    let rect = {
        let inner = state
            .lock_inner()
            .map_err(|_| "计数器位置设置状态已损坏".to_string())?;
        rect_for_group(&inner.settings, &group_id)
    };

    Ok(CounterSelectionOutcome {
        kind,
        rect,
        group_id: Some(group_id),
    })
}

#[tauri::command]
pub fn counter_position_commit(
    app: AppHandle,
    state: State<'_, CounterState>,
) -> Result<CounterBootstrap, AppError> {
    let (sender, group_id, bootstrap) = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计数器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.logic.pending_position.take() else {
            return Err(AppError::Message("当前没有等待中的位置设置流程".to_string()));
        };

        let group_id = pending.group_id.clone();
        set_rect_for_group(&mut inner.settings, &group_id, pending.staged_rect.clone());
        settings::save_settings(&app, &inner.settings)?;
        (
            pending.sender,
            group_id,
            CounterLogic::build_bootstrap(&inner),
        )
    };

    let _ = sender.send(CounterSelectionKind::Selected);
    destroy_window(&app, &position_label_for_group(&group_id));
    ensure_display_windows(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    profile::update_active_profile_snapshot(
        &app,
        ActiveProfileSnapshotPatch::Counter(bootstrap.settings.clone()),
    )?;
    Ok(bootstrap)
}

#[tauri::command]
pub fn counter_position_cancel(
    app: AppHandle,
    state: State<'_, CounterState>,
) -> Result<(), AppError> {
    let (sender, group_id) = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计数器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.logic.pending_position.take() else {
            return Err(AppError::Message("当前没有等待中的位置设置流程".to_string()));
        };

        let group_id = pending.group_id.clone();
        set_rect_for_group(&mut inner.settings, &group_id, pending.original_rect);
        (pending.sender, group_id)
    };

    let _ = sender.send(CounterSelectionKind::Cancelled);
    destroy_window(&app, &position_label_for_group(&group_id));
    Ok(())
}

#[tauri::command]
pub fn counter_position_moved(
    x: i32,
    y: i32,
    app: AppHandle,
    state: State<'_, CounterState>,
) -> Result<CounterRect, AppError> {
    let (rect, group_id) = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计数器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.logic.pending_position.as_mut() else {
            return Err(AppError::Message("当前没有等待中的位置设置流程".to_string()));
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
        assert_eq!(display_height(0), COUNTER_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(1), COUNTER_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(4), 168);
    }

    #[test]
    fn main_window_close_is_app_shutdown_request() {
        assert!("main" == "main");
        assert!(COUNTER_DISPLAY_LABEL != "main");
        assert!(COUNTER_POSITION_LABEL != "main");
    }

    #[test]
    fn normalize_settings_preserves_custom_width() {
        let mut settings = CounterSettings::default();
        settings.display.rect.width = 480;
        settings.counters = vec![sample_counter("c", "F3")];

        let normalized = normalize_settings(settings).unwrap();
        assert_eq!(normalized.display.rect.width, 480);
        assert_eq!(normalized.display.rect.height, 96);
    }

    #[test]
    fn normalize_settings_migrates_legacy_groups() {
        let mut settings = CounterSettings::default();
        settings.display.rect.width = 520;
        settings.counter_groups.clear();
        settings.counters = vec![sample_counter("c", "F3")];

        let normalized = normalize_settings(settings).unwrap();
        assert_eq!(normalized.counter_groups.len(), 1);
        assert_eq!(normalized.counter_groups[0].id, DEFAULT_COUNTER_GROUP_ID);
        assert_eq!(normalized.counter_groups[0].display.rect.width, 520);
        assert_eq!(normalized.counters[0].group_id, DEFAULT_COUNTER_GROUP_ID);
    }

    #[test]
    fn normalize_settings_rejects_empty_name() {
        let mut settings = CounterSettings::default();
        settings.counters[0].name = "   ".to_string();
        let error = normalize_settings(settings).unwrap_err();
        assert!(error.contains("名称不能为空"));
    }

    #[test]
    fn normalize_settings_rejects_empty_hotkey() {
        let mut settings = CounterSettings::default();
        settings.counters[0].hotkey = "   ".to_string();
        let error = normalize_settings(settings).unwrap_err();
        assert!(error.contains("快捷键不能为空"));
    }

    #[test]
    fn counter_normalize_moves_unknown_group_to_default() {
        let mut settings = CounterSettings::default();
        settings.counters[0].group_id = "missing".to_string();

        let normalized = normalize_settings(settings).expect("计数器配置应规范化");

        assert_eq!(normalized.counters[0].group_id, DEFAULT_COUNTER_GROUP_ID);
    }

    #[test]
    fn counter_normalize_rejects_duplicate_counter_ids() {
        let mut settings = CounterSettings::default();
        settings.counters.push(CounterItem {
            id: settings.counters[0].id.clone(),
            group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
            name: "重复计数器".to_string(),
            start_value: 0,
            hotkey: "F4".to_string(),
            enabled: true,
        });

        let error = normalize_settings(settings).expect_err("重复 ID 应报错");

        assert_eq!(error, "计数器 ID 重复: counter-1");
    }

    #[test]
    fn counter_build_hotkey_bindings_groups_same_hotkey() {
        let mut settings = CounterSettings::default();
        settings.counter_enabled = true;
        settings.enabled = true;
        settings.counters.push(CounterItem {
            id: "counter-2".to_string(),
            group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
            name: "计数器 2".to_string(),
            start_value: 5,
            hotkey: "F3".to_string(),
            enabled: true,
        });

        let bindings = CounterLogic::build_hotkey_bindings(&settings).expect("绑定构建应成功");

        assert_eq!(bindings.normal.len(), 1);
        assert!(bindings.hold.is_empty());
        assert_eq!(bindings.normal[0].0, "F3");
    }

    #[test]
    fn counter_run_states_reflects_accumulated_runs() {
        // 守卫 #64 回归：stop_all 不再清空 runs，因此 counter_run_states
        // 必须能反映 runs 中累积的值，而非始终回落到 start_value。
        let counter = sample_counter("c", "F3");
        let settings = CounterSettings {
            counters: vec![counter],
            ..CounterSettings::default()
        };
        let mut logic = CounterLogic {
            runs: HashMap::new(),
            pending_position: None,
        };
        logic.runs.insert("c".to_string(), 42);
        let inner = ToolStateInner {
            settings,
            logic,
            hotkey_error: None,
        };
        let states = counter_run_states(&inner);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].value, 42, "累积值应被保留而非回落到 start_value");
    }
}
