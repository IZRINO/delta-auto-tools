use std::collections::HashMap;
use std::sync::{mpsc, Arc};

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::oneshot;

use crate::hotkey_types;
use crate::hotkeys::HotkeyManager;
use crate::sync_tool::{
    group_enabled, normalize_sync_settings, HotkeyBindingSet, SyncGroup, SyncItem, SyncSettings,
    SyncToolLogic,
};
use crate::tool_base::{ToolLogic, ToolState, ToolStateInner};

pub mod commands;
mod events;
pub mod keys;
pub mod overlay;
mod settings;
pub mod types;
pub mod worker;
pub use self::overlay::ensure_overlay_window;
pub use self::types::{
    RapidfireBootstrap, RapidfireCard, RapidfireGroup, RapidfireRect, RapidfireRunState,
    RapidfireRunStatus, RapidfireSelectionKind, RapidfireSettings, DEFAULT_RAPIDFIRE_GROUP_ID,
};
pub use self::worker::{
    CardRuntime, RapidfireSessionRuntime, RapidfireSessionStatus, RapidfireSessionWorker,
    SessionControl,
};

// 内部引用：mod.rs 自身代码使用
use self::keys::{
    parse_target_key, RAPIDFIRE_DISPLAY_MAX_WIDTH, RAPIDFIRE_DISPLAY_MIN_WIDTH,
    RAPIDFIRE_GLOBAL_DELAY_MAX_MS, RAPIDFIRE_GLOBAL_DELAY_MIN_MS, RAPIDFIRE_MIN_INTERVAL_MS,
    RAPIDFIRE_PRESS_JITTER_MAX_MS, RAPIDFIRE_PRESS_JITTER_MIN_MS, RAPIDFIRE_TRIGGER_JITTER_MAX_MS,
};
use self::overlay::{
    destroy_display_windows, destroy_position_windows, display_label_for_group,
    hide_display_windows,
};
use self::worker::{
    next_session_id, spawn_session_worker, stop_all_sessions, stop_latest_active_session,
};

// ---- State ----

pub struct RapidfireLogic {
    pub runs: HashMap<String, CardRuntime>,
    pub pending_position: Option<PendingRapidfirePosition>,
}

pub type RapidfireState = ToolState<RapidfireLogic>;

pub(crate) struct PendingRapidfirePosition {
    pub group_id: String,
    pub original_position: RapidfireRect,
    pub staged_position: RapidfireRect,
    pub sender: oneshot::Sender<RapidfireSelectionKind>,
}

// ---- ToolLogic impl ----

fn run_states(inner: &ToolStateInner<RapidfireLogic>) -> Vec<RapidfireRunState> {
    inner
        .settings
        .cards
        .iter()
        .map(|card| {
            let run = inner.logic.runs.get(&card.id);
            RapidfireRunState {
                card_id: card.id.clone(),
                status: run
                    .map(CardRuntime::aggregate_status)
                    .unwrap_or(RapidfireRunStatus::Idle),
                count: run.map(CardRuntime::aggregate_count).unwrap_or(0),
            }
        })
        .collect()
}

impl ToolLogic for RapidfireLogic {
    type Settings = RapidfireSettings;
    type Bootstrap = RapidfireBootstrap;
    const NAME: &'static str = "连发器";

    fn load_settings(app: &AppHandle) -> Result<Self::Settings, String> {
        settings::load_settings(app)
    }

    fn save_settings(app: &AppHandle, settings: &Self::Settings) -> Result<(), String> {
        settings::save_settings(app, settings)
    }

    fn build_bootstrap(inner: &ToolStateInner<Self>) -> Self::Bootstrap {
        RapidfireBootstrap {
            settings: inner.settings.clone(),
            runs: run_states(inner),
            hotkey_error: inner.hotkey_error.clone(),
        }
    }

    fn emit_state<R: Runtime>(app: &AppHandle<R>, bootstrap: &Self::Bootstrap) {
        let _ = app.emit_to("main", events::STATE_CHANGED, (*bootstrap).clone());
        for group in &bootstrap.settings.groups {
            let _ = app.emit_to(
                display_label_for_group(&group.id),
                events::STATE_CHANGED,
                (*bootstrap).clone(),
            );
        }
    }
}

// ---- SyncItem / SyncGroup / SyncSettings impl ----

impl SyncItem for RapidfireCard {
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

impl SyncGroup for RapidfireGroup {
    fn id(&self) -> &str {
        &self.id
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl SyncSettings for RapidfireSettings {
    type Item = RapidfireCard;
    type Group = RapidfireGroup;
    const DEFAULT_GROUP_ID: &'static str = DEFAULT_RAPIDFIRE_GROUP_ID;
    const DUPLICATE_ITEM_MESSAGE_PREFIX: &'static str = "连发器卡片 ID 重复";

    fn sync_legacy_enabled(&mut self) {}
    fn items(&self) -> &[Self::Item] {
        &self.cards
    }
    fn items_mut(&mut self) -> &mut Vec<Self::Item> {
        &mut self.cards
    }
    fn replace_items(&mut self, items: Vec<Self::Item>) {
        self.cards = items;
    }
    fn normalize_groups(&self) -> Result<Vec<Self::Group>, String> {
        normalize_groups(self)
    }
    fn replace_groups(&mut self, groups: Vec<Self::Group>) {
        self.groups = groups;
    }

    fn default_item(&self) -> Self::Item {
        RapidfireSettings::default()
            .cards
            .into_iter()
            .next()
            .expect("默认连发器配置必须包含一张卡片")
    }
    fn normalize_item(&self, item: &Self::Item) -> Result<Self::Item, String> {
        normalize_card(item)
    }

    fn after_groups_normalized(&mut self) {
        if let Some(default_group) = self
            .groups
            .iter()
            .find(|group| group.id == DEFAULT_RAPIDFIRE_GROUP_ID)
        {
            self.show_overlay = default_group.show_overlay;
            self.overlay_position = default_group.overlay_position.clone();
            self.overlay_width = default_group.overlay_width;
        }
    }
}

// ---- SyncToolLogic impl ----

impl SyncToolLogic for RapidfireLogic {
    const SCOPE: &'static str = "rapidfire";
    const SCOPE_LABEL: &'static str = "连发器";

    fn tool_enabled(settings: &RapidfireSettings) -> bool {
        settings.rapidfire_enabled
    }

    fn build_hotkey_bindings(settings: &RapidfireSettings) -> Result<HotkeyBindingSet, String> {
        let mut by_key: HashMap<String, Vec<String>> = HashMap::new();
        for card in &settings.cards {
            if !card.enabled || !group_enabled(&settings.groups, &card.group_id) {
                continue;
            }
            by_key
                .entry(card.trigger_key.clone())
                .or_default()
                .push(card.id.clone());
        }

        let mut bindings = HotkeyBindingSet::empty();
        for (trigger_key, card_ids) in by_key {
            let key_for_tuple = trigger_key.clone();
            let callback: hotkey_types::HoldActionCallback = Arc::new(move |app_handle, action| {
                let card_ids = card_ids.clone();
                let app = app_handle.clone();
                let hold_action = action.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = handle_hold_event(&app, card_ids, hold_action).await {
                        let _ = app.emit_to("main", events::HOTKEY_ERROR, error);
                    }
                });
            });
            bindings.hold.push((key_for_tuple, callback));
        }
        Ok(bindings)
    }

    fn stop_all(app: &AppHandle) -> Result<(), String> {
        let Some(state) = app.try_state::<RapidfireState>() else {
            return Ok(());
        };
        let hotkey_manager = app.try_state::<HotkeyManager>();
        stop_all(app, &state, hotkey_manager.as_ref().map(|v| &**v));
        Ok(())
    }
}

// ---- Event emission ----

pub(crate) fn emit_state(app: &AppHandle, bootstrap: RapidfireBootstrap) {
    RapidfireLogic::emit_state(app, &bootstrap);
}

// ---- Hotkey handling ----

async fn handle_hold_event(
    app: &AppHandle,
    card_ids: Vec<String>,
    hold_action: hotkey_types::HoldAction,
) -> Result<(), String> {
    match hold_action {
        hotkey_types::HoldAction::Down => handle_key_down(app, card_ids).await,
        hotkey_types::HoldAction::Up => handle_key_up(app, card_ids).await,
    }
}

async fn handle_key_down(app: &AppHandle, card_ids: Vec<String>) -> Result<(), String> {
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicBool, Ordering};

    let state = app.state::<RapidfireState>();
    let sessions_to_spawn = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "连发器状态已损坏".to_string())?;

        if !inner.settings.rapidfire_enabled {
            return Ok(());
        }

        let compensation_delay_min_ms = inner.settings.compensation_delay_min_ms;
        let compensation_delay_max_ms = inner.settings.compensation_delay_max_ms;

        let mut card_infos: Vec<(
            String,
            String,
            String,
            u64,
            u64,
            u64,
            u64,
            u64,
            bool,
            bool,
            bool,
        )> = Vec::new();
        for card_id in &card_ids {
            let cid = card_id.clone();
            if let Some(info) = inner
                .settings
                .cards
                .iter()
                .find(|c| {
                    c.id == cid && c.enabled && group_enabled(&inner.settings.groups, &c.group_id)
                })
                .map(|c| {
                    (
                        c.id.clone(),
                        c.trigger_key.clone(),
                        c.target_key.clone(),
                        c.interval_ms,
                        c.press_jitter_min_ms,
                        c.press_jitter_max_ms,
                        c.min_press_spacing_ms,
                        c.trigger_jitter_max_ms,
                        c.cancel_jitter_on_release,
                        c.skip_compensation,
                        c.ignore_trigger_key,
                    )
                })
            {
                card_infos.push(info);
            }
        }
        let ignore_trigger_key_for_batch = card_infos.iter().any(|info| info.10);

        let mut sessions_to_spawn = Vec::new();
        for (
            cid,
            trigger,
            target,
            interval,
            jitter_min,
            jitter_max,
            min_press_spacing_ms,
            trigger_jitter_max_ms,
            cancel_jitter_on_release,
            skip_compensation,
            _,
        ) in card_infos
        {
            let session_id = next_session_id();
            let (control_tx, control_rx) = mpsc::channel();
            let compensate_now = Arc::new(AtomicBool::new(false));
            let run = inner.logic.runs.entry(cid.clone()).or_default();
            let last_press_at = run.last_press_at.clone();

            for session in run.sessions.values_mut() {
                session.compensate_now.store(true, Ordering::Relaxed);
            }

            run.active_session_ids.push(session_id.clone());
            run.sessions.insert(
                session_id.clone(),
                RapidfireSessionRuntime {
                    count: 0,
                    status: RapidfireSessionStatus::Firing,
                    control_tx: Some(control_tx),
                    compensate_now: compensate_now.clone(),
                },
            );

            sessions_to_spawn.push(RapidfireSessionWorker {
                card_id: cid,
                session_id,
                trigger_key: trigger,
                target_key: target,
                interval_ms: interval,
                press_jitter_min_ms: jitter_min,
                press_jitter_max_ms: jitter_max,
                skip_compensation,
                compensation_delay_min_ms,
                compensation_delay_max_ms,
                min_press_spacing_ms,
                trigger_jitter_max_ms,
                cancel_jitter_on_release,
                control_rx,
                compensate_now,
                last_press_at,
            });
        }

        if ignore_trigger_key_for_batch {
            let trigger_keys: HashSet<String> = sessions_to_spawn
                .iter()
                .map(|w| w.trigger_key.clone())
                .collect();
            let hm = app.state::<HotkeyManager>();
            for trigger_key in &trigger_keys {
                match hm.suppress_key(trigger_key) {
                    Ok(was_new) => crate::log_debug!(
                        "rapidfire",
                        "触发键抑制已更新",
                        "trigger_key" => trigger_key.clone(),
                        "was_new" => was_new
                    ),
                    Err(e) => crate::log_error!(
                        "rapidfire",
                        "触发键抑制失败",
                        "trigger_key" => trigger_key.clone(),
                        "error" => e.to_string()
                    ),
                }
            }
        }

        sessions_to_spawn
    };

    let spawned_count = sessions_to_spawn.len();
    for worker in sessions_to_spawn {
        spawn_session_worker(app.clone(), worker);
    }

    if spawned_count > 0 {
        let bootstrap = {
            let inner = state
                .lock_inner()
                .map_err(|_| "连发器状态已损坏".to_string())?;
            RapidfireLogic::build_bootstrap(&inner)
        };
        emit_state(app, bootstrap);
    }
    Ok(())
}

async fn handle_key_up(app: &AppHandle, card_ids: Vec<String>) -> Result<(), String> {
    let state = app.state::<RapidfireState>();
    let stopped_count = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        if !inner.settings.rapidfire_enabled {
            return Ok(());
        }

        let ignore_trigger_keys: Vec<String> = inner
            .settings
            .cards
            .iter()
            .filter(|c| card_ids.contains(&c.id) && c.ignore_trigger_key)
            .map(|c| c.trigger_key.clone())
            .collect();

        let mut stopped_count = 0usize;
        for card_id in &card_ids {
            if let Some(run) = inner.logic.runs.get_mut(card_id) {
                if stop_latest_active_session(run, SessionControl::StopWithCompensation) {
                    stopped_count += 1;
                }
            }
        }

        for trigger_key in &ignore_trigger_keys {
            let has_active = inner
                .settings
                .cards
                .iter()
                .filter(|c| c.trigger_key == *trigger_key && c.ignore_trigger_key && c.enabled)
                .any(|c| {
                    inner
                        .logic
                        .runs
                        .get(&c.id)
                        .map(|run| !run.sessions.is_empty())
                        .unwrap_or(false)
                });
            if !has_active {
                let hm = app.state::<HotkeyManager>();
                match hm.unsuppress_key(trigger_key) {
                    Ok(was) => crate::log_debug!(
                        "rapidfire",
                        "触发键抑制已取消",
                        "trigger_key" => trigger_key.clone(),
                        "was_suppressed" => was
                    ),
                    Err(e) => crate::log_error!(
                        "rapidfire",
                        "取消触发键抑制失败",
                        "trigger_key" => trigger_key.clone(),
                        "error" => e.to_string()
                    ),
                }
            }
        }
        stopped_count
    };

    if stopped_count > 0 {
        emit_state(app, {
            let inner = state
                .lock_inner()
                .map_err(|_| "连发器状态已损坏".to_string())?;
            RapidfireLogic::build_bootstrap(&inner)
        });
    }
    Ok(())
}

// ---- Normalization ----

pub(crate) fn normalize_card(card: &RapidfireCard) -> Result<RapidfireCard, String> {
    let name = card.name.trim();
    if name.is_empty() {
        return Err("连发器卡片名称不能为空".to_string());
    }

    let trigger_key =
        normalize_trigger_key(&card.trigger_key).map_err(|e| format!("{name} 的触发键{e}"))?;
    if trigger_key.is_empty() {
        return Err(format!("{} 的触发键不能为空", name));
    }
    let trigger_primary = keys::trigger_primary_label(&trigger_key)?;
    if parse_target_key(&trigger_primary).is_none() {
        return Err(format!("{} 的触发键不支持: {}", name, trigger_primary));
    }

    let target_key =
        normalize_single_key(&card.target_key).map_err(|e| format!("{name} 的目标键{e}"))?;
    if target_key.is_empty() {
        return Err(format!("{} 的目标键不能为空", name));
    }
    if parse_target_key(&target_key).is_none() {
        return Err(format!("{} 的目标键不支持: {}", name, target_key));
    }

    let interval_ms = card.interval_ms.max(RAPIDFIRE_MIN_INTERVAL_MS);
    let press_jitter_min_ms = card
        .press_jitter_min_ms
        .max(RAPIDFIRE_PRESS_JITTER_MIN_MS)
        .min(RAPIDFIRE_PRESS_JITTER_MAX_MS);
    let press_jitter_max_ms = card
        .press_jitter_max_ms
        .max(RAPIDFIRE_PRESS_JITTER_MIN_MS)
        .min(RAPIDFIRE_PRESS_JITTER_MAX_MS);
    if card.min_press_spacing_ms > RAPIDFIRE_GLOBAL_DELAY_MAX_MS {
        return Err(format!(
            "{} 的按键最小间距不能大于 {}ms",
            name, RAPIDFIRE_GLOBAL_DELAY_MAX_MS
        ));
    }
    if card.trigger_jitter_max_ms > RAPIDFIRE_TRIGGER_JITTER_MAX_MS {
        return Err(format!(
            "{} 的触发抖动延迟上限不能大于 {}ms",
            name, RAPIDFIRE_TRIGGER_JITTER_MAX_MS
        ));
    }
    if press_jitter_min_ms > press_jitter_max_ms {
        return Err(format!("{} 的目标键触发抖动最小值不能大于最大值", name));
    }

    Ok(RapidfireCard {
        id: card.id.trim().to_string(),
        group_id: card.group_id.trim().to_string(),
        name: name.to_string(),
        trigger_key,
        target_key,
        interval_ms,
        press_jitter_min_ms,
        press_jitter_max_ms,
        min_press_spacing_ms: card.min_press_spacing_ms,
        trigger_jitter_max_ms: card.trigger_jitter_max_ms,
        cancel_jitter_on_release: card.cancel_jitter_on_release,
        enabled: card.enabled,
        skip_compensation: card.skip_compensation,
        ignore_trigger_key: card.ignore_trigger_key,
    })
}

fn normalize_trigger_key(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    hotkey_types::hotkey_to_string(trimmed).map_err(|_| format!("不支持: {trimmed}"))
}

fn default_rapidfire_group(settings_value: &RapidfireSettings) -> RapidfireGroup {
    RapidfireGroup {
        id: DEFAULT_RAPIDFIRE_GROUP_ID.to_string(),
        name: "默认分组".to_string(),
        enabled: true,
        show_overlay: settings_value.show_overlay,
        overlay_position: settings_value.overlay_position.clone(),
        overlay_width: settings_value.overlay_width,
    }
}

fn normalize_groups(settings_value: &RapidfireSettings) -> Result<Vec<RapidfireGroup>, String> {
    let mut groups = if settings_value.groups.is_empty() {
        vec![default_rapidfire_group(settings_value)]
    } else {
        settings_value.groups.clone()
    };

    if !groups.iter().any(|g| g.id == DEFAULT_RAPIDFIRE_GROUP_ID) {
        groups.insert(0, default_rapidfire_group(settings_value));
    }

    let mut seen = HashMap::new();
    let mut normalized = Vec::with_capacity(groups.len());
    for mut group in groups {
        group.id = group.id.trim().to_string();
        if group.id.is_empty() {
            group.id = DEFAULT_RAPIDFIRE_GROUP_ID.to_string();
        }
        group.name = group.name.trim().to_string();
        if group.name.is_empty() {
            return Err("连发器分组名称不能为空".to_string());
        }
        if seen.insert(group.id.clone(), true).is_some() {
            return Err(format!("连发器分组 ID 重复: {}", group.id));
        }
        if group.id == DEFAULT_RAPIDFIRE_GROUP_ID {
            group.show_overlay = settings_value.show_overlay;
            group.overlay_position = settings_value.overlay_position.clone();
            group.overlay_width = settings_value.overlay_width;
        }
        group.overlay_width = group
            .overlay_width
            .max(RAPIDFIRE_DISPLAY_MIN_WIDTH)
            .min(RAPIDFIRE_DISPLAY_MAX_WIDTH);
        normalized.push(group);
    }
    Ok(normalized)
}

fn normalize_single_key(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.contains('+') {
        return Err("必须是单键，不能包含组合键".to_string());
    }
    hotkey_types::hotkey_primary_label(trimmed).map_err(|_| format!("不支持: {trimmed}"))
}

pub(crate) fn normalize_settings(
    mut settings_value: RapidfireSettings,
) -> Result<RapidfireSettings, String> {
    settings_value.overlay_width = settings_value
        .overlay_width
        .max(RAPIDFIRE_DISPLAY_MIN_WIDTH)
        .min(RAPIDFIRE_DISPLAY_MAX_WIDTH);
    if settings_value.compensation_delay_min_ms > settings_value.compensation_delay_max_ms {
        return Err("补齐延迟最小值不能大于最大值".to_string());
    }
    if settings_value.min_press_spacing_ms > RAPIDFIRE_GLOBAL_DELAY_MAX_MS {
        return Err(format!(
            "按键最小间距不能大于 {}ms",
            RAPIDFIRE_GLOBAL_DELAY_MAX_MS
        ));
    }
    if settings_value.compensation_delay_max_ms > RAPIDFIRE_GLOBAL_DELAY_MAX_MS {
        return Err(format!(
            "补齐延迟不能大于 {}ms",
            RAPIDFIRE_GLOBAL_DELAY_MAX_MS
        ));
    }
    settings_value.min_press_spacing_ms = settings_value
        .min_press_spacing_ms
        .max(RAPIDFIRE_GLOBAL_DELAY_MIN_MS)
        .min(RAPIDFIRE_GLOBAL_DELAY_MAX_MS);
    settings_value.trigger_jitter_max_ms = settings_value
        .trigger_jitter_max_ms
        .min(RAPIDFIRE_TRIGGER_JITTER_MAX_MS);
    normalize_sync_settings(settings_value)
}

// ---- Hotkey registration ----

pub(crate) fn restart_hotkey_listeners(
    state: &RapidfireState,
    hotkey_manager: &HotkeyManager,
    settings_value: &RapidfireSettings,
    force: bool,
) -> Result<(), String> {
    if !settings_value.rapidfire_enabled {
        return hotkey_manager.clear_hold_scope("rapidfire");
    }

    let mut new_by_key: HashMap<String, Vec<String>> = HashMap::new();
    for card in &settings_value.cards {
        if !card.enabled || !group_enabled(&settings_value.groups, &card.group_id) {
            continue;
        }
        new_by_key
            .entry(card.trigger_key.clone())
            .or_default()
            .push(card.id.clone());
    }

    let previous_by_key = {
        let inner = state
            .lock_inner()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        let mut by_key: HashMap<String, Vec<String>> = HashMap::new();
        for card in &inner.settings.cards {
            if !card.enabled || !group_enabled(&inner.settings.groups, &card.group_id) {
                continue;
            }
            by_key
                .entry(card.trigger_key.clone())
                .or_default()
                .push(card.id.clone());
        }
        by_key
    };

    if !force && new_by_key == previous_by_key {
        return Ok(());
    }
    state.restart_sync_hotkeys(hotkey_manager, settings_value)
}

// ---- Initialize / Shutdown ----

pub fn shutdown(app: &AppHandle, state: &RapidfireState, hotkey_manager: &HotkeyManager) {
    let _ = hotkey_manager.clear_hold_scope("rapidfire");
    hotkey_manager.clear_all_suppressions();
    if let Ok(mut inner) = state.lock_inner() {
        stop_all_sessions(&mut inner.logic.runs, SessionControl::Cancel);
        inner.logic.runs.clear();
    }
    destroy_position_windows(app);
    destroy_display_windows(app);
}

/// 停止所有正在运行的连发器会话（用于全局总开关关闭）。
pub fn stop_all(app: &AppHandle, state: &RapidfireState, hotkey_manager: Option<&HotkeyManager>) {
    let bootstrap = {
        let Ok(mut inner) = state.lock_inner() else {
            return;
        };
        stop_all_sessions(&mut inner.logic.runs, SessionControl::Cancel);
        inner.logic.runs.clear();
        RapidfireLogic::build_bootstrap(&inner)
    };
    if let Some(hm) = hotkey_manager {
        hm.clear_all_suppressions();
        let _ = hm.stop_suppressor();
    }
    hide_display_windows(app);
    emit_state(app, bootstrap);
}

pub(crate) fn stop_registered(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<RapidfireState>() else {
        return Ok(());
    };
    let hotkey_manager = app.try_state::<HotkeyManager>();
    stop_all(app, &state, hotkey_manager.as_ref().map(|v| &**v));
    Ok(())
}

pub fn initialize(
    app: &AppHandle,
    hotkey_manager: &HotkeyManager,
) -> Result<RapidfireState, String> {
    let settings = normalize_settings(settings::load_settings(app)?)?;
    let logic = RapidfireLogic {
        runs: HashMap::new(),
        pending_position: None,
    };
    let state = RapidfireState::new(logic, settings.clone());

    if settings.rapidfire_enabled {
        if let Err(error) = restart_hotkey_listeners(&state, hotkey_manager, &settings, true) {
            crate::log_warn!(
                "rapidfire",
                "初始化热键监听失败",
                "error" => error.clone()
            );
            if let Ok(mut inner) = state.lock_inner() {
                inner.hotkey_error = Some(error);
            }
        }
        ensure_overlay_window(app, &settings)?;
    }

    Ok(state)
}

pub(crate) fn schedule_overlay_window_reconcile_from_profile(
    app: &AppHandle,
    settings: &RapidfireSettings,
) {
    let settings = match normalize_settings(settings.clone()) {
        Ok(settings) => settings,
        Err(error) => {
            crate::log_warn!(
                "rapidfire",
                "Profile 连发器透明窗口配置无效",
                "error" => error
            );
            return;
        }
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = ensure_overlay_window(&app, &settings) {
            crate::log_warn!(
                "rapidfire",
                "同步连发器透明窗口失败",
                "error" => error
            );
        }
    });
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;

    const RAPIDFIRE_DEFAULT_PRESS_JITTER_MIN_MS: u64 = 8;
    const RAPIDFIRE_DEFAULT_PRESS_JITTER_MAX_MS: u64 = 12;

    fn sample_card(id: &str, trigger: &str) -> RapidfireCard {
        RapidfireCard {
            id: id.to_string(),
            group_id: DEFAULT_RAPIDFIRE_GROUP_ID.to_string(),
            name: id.to_string(),
            trigger_key: trigger.to_string(),
            target_key: "1".to_string(),
            interval_ms: 100,
            press_jitter_min_ms: RAPIDFIRE_DEFAULT_PRESS_JITTER_MIN_MS,
            press_jitter_max_ms: RAPIDFIRE_DEFAULT_PRESS_JITTER_MAX_MS,
            min_press_spacing_ms: 80,
            trigger_jitter_max_ms: 0,
            cancel_jitter_on_release: true,
            enabled: true,
            skip_compensation: false,
            ignore_trigger_key: false,
        }
    }

    #[test]
    fn normalize_card_rejects_empty_name() {
        let mut card = sample_card("a", "F1");
        card.name = "  ".to_string();
        assert!(normalize_card(&card).unwrap_err().contains("名称不能为空"));
    }

    #[test]
    fn normalize_card_clamps_interval_to_minimum() {
        let mut card = sample_card("a", "F1");
        card.interval_ms = 0;
        assert_eq!(
            normalize_card(&card).unwrap().interval_ms,
            RAPIDFIRE_MIN_INTERVAL_MS
        );
    }

    #[test]
    fn normalize_card_clamps_press_jitter_to_supported_range() {
        let mut card = sample_card("a", "F1");
        card.press_jitter_min_ms = 0;
        card.press_jitter_max_ms = 2500;
        let n = normalize_card(&card).unwrap();
        assert_eq!(n.press_jitter_min_ms, RAPIDFIRE_PRESS_JITTER_MIN_MS);
        assert_eq!(n.press_jitter_max_ms, RAPIDFIRE_PRESS_JITTER_MAX_MS);
    }

    #[test]
    fn normalize_card_rejects_inverted_press_jitter_range() {
        let mut card = sample_card("a", "F1");
        card.press_jitter_min_ms = 30;
        card.press_jitter_max_ms = 20;
        assert!(normalize_card(&card).unwrap_err().contains("触发抖动"));
    }

    #[test]
    fn normalize_card_preserves_skip_compensation() {
        let mut card = sample_card("a", "F1");
        card.skip_compensation = true;
        assert!(normalize_card(&card).unwrap().skip_compensation);
    }

    #[test]
    fn normalize_card_rejects_unsupported_target_key() {
        let mut card = sample_card("a", "F1");
        card.target_key = "UnknownKey".to_string();
        assert!(normalize_card(&card).unwrap_err().contains("目标键不支持"));
    }

    #[test]
    fn normalize_card_allows_modified_trigger_key() {
        let n = normalize_card(&sample_card("a", "shift+-")).unwrap();
        assert_eq!(n.trigger_key, "Shift+-");
    }

    #[test]
    fn normalize_card_keeps_alt_as_trigger_primary_key() {
        assert_eq!(
            normalize_card(&sample_card("a", "alt"))
                .unwrap()
                .trigger_key,
            "Alt"
        );
    }

    #[test]
    fn normalize_card_rejects_unsupported_trigger_key() {
        assert!(normalize_card(&sample_card("a", "F25"))
            .unwrap_err()
            .contains("触发键不支持"));
    }

    #[test]
    fn normalize_card_normalizes_key_labels() {
        let mut card = sample_card("a", "space");
        card.target_key = "escape".to_string();
        let n = normalize_card(&card).unwrap();
        assert_eq!(n.trigger_key, "Space");
        assert_eq!(n.target_key, "Esc");
    }

    #[test]
    fn normalize_settings_auto_adds_default_card_when_empty() {
        let mut s = RapidfireSettings::default();
        s.cards.clear();
        assert_eq!(normalize_settings(s).unwrap().cards.len(), 1);
    }

    #[test]
    fn normalize_settings_migrates_legacy_group() {
        let mut s = RapidfireSettings::default();
        s.groups.clear();
        s.show_overlay = true;
        s.overlay_width = 420;
        s.overlay_position = Some(RapidfireRect { x: 11, y: 22 });
        s.cards = vec![sample_card("a", "F1")];
        let n = normalize_settings(s).unwrap();
        assert_eq!(n.groups.len(), 1);
        assert!(n.groups[0].show_overlay);
        assert_eq!(n.groups[0].overlay_width, 420);
    }

    #[test]
    fn normalize_settings_rejects_duplicate_card_ids() {
        let mut s = RapidfireSettings::default();
        s.cards = vec![sample_card("same", "F1"), sample_card("same", "F2")];
        assert!(normalize_settings(s).unwrap_err().contains("ID 重复"));
    }

    #[test]
    fn normalize_settings_rejects_inverted_global_compensation_delay() {
        let mut s = RapidfireSettings::default();
        s.compensation_delay_min_ms = 180;
        s.compensation_delay_max_ms = 120;
        assert!(normalize_settings(s).unwrap_err().contains("补齐延迟"));
    }

    #[test]
    fn normalize_settings_rejects_too_large_global_delays() {
        let mut s = RapidfireSettings::default();
        s.compensation_delay_max_ms = RAPIDFIRE_GLOBAL_DELAY_MAX_MS + 1;
        assert!(normalize_settings(s).unwrap_err().contains("补齐延迟"));
    }

    #[test]
    fn display_height_has_minimum() {
        use super::keys::RAPIDFIRE_DISPLAY_MIN_HEIGHT;
        use super::overlay::display_height;
        assert_eq!(display_height(0), RAPIDFIRE_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(1), RAPIDFIRE_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(5), 172);
    }
}
