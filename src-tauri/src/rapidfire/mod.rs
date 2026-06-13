use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};
use tokio::sync::oneshot;

mod settings;
mod types;

pub use self::types::{
    RapidfireBootstrap, RapidfireCard, RapidfireGroup, RapidfireRect, RapidfireRunState,
    RapidfireRunStatus, RapidfireSelectionKind, RapidfireSelectionOutcome, RapidfireSettings,
    DEFAULT_RAPIDFIRE_GROUP_ID,
};

use crate::{
    app_error::AppError,
    hotkey_types,
    hotkeys::{HoldAction, HoldActionCallback, HotkeyManager},
    overlay_utils::{destroy_stale_windows, destroy_window, destroy_windows_with_prefix, encoded_query_value, hide_window, safe_label_component},
};

const RAPIDFIRE_DISPLAY_LABEL: &str = "rapidfire-display";
const RAPIDFIRE_POSITION_LABEL: &str = "rapidfire-position";
const RAPIDFIRE_DISPLAY_MIN_HEIGHT: i32 = 80;
const RAPIDFIRE_DISPLAY_MIN_WIDTH: i32 = 320;
const RAPIDFIRE_DISPLAY_MAX_WIDTH: i32 = 800;
const RAPIDFIRE_MIN_INTERVAL_MS: u64 = 1;
const RAPIDFIRE_TRIGGER_RELEASE_SETTLE_MS: u64 = 2;
const RAPIDFIRE_PRESS_JITTER_MIN_MS: u64 = 1;
const RAPIDFIRE_PRESS_JITTER_MAX_MS: u64 = 2000;
const RAPIDFIRE_GLOBAL_DELAY_MIN_MS: u64 = 0;
const RAPIDFIRE_GLOBAL_DELAY_MAX_MS: u64 = 10_000;
const RAPIDFIRE_TRIGGER_JITTER_MAX_MS: u64 = 99_999;

static NEXT_RAPIDFIRE_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static RAPIDFIRE_JITTER_COUNTER: AtomicU64 = AtomicU64::new(1);

// ---- State ----

pub struct RapidfireState {
    inner: Mutex<RapidfireStateInner>,
}

struct RapidfireStateInner {
    settings: RapidfireSettings,
    runs: HashMap<String, CardRuntime>,
    pending_position: Option<PendingRapidfirePosition>,
    hotkey_error: Option<String>,
}

struct CardRuntime {
    sessions: HashMap<String, RapidfireSessionRuntime>,
    active_session_ids: Vec<String>,
    last_press_at: Arc<Mutex<Instant>>,
}

impl Default for CardRuntime {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            active_session_ids: Vec::new(),
            last_press_at: Arc::new(Mutex::new(Instant::now())),
        }
    }
}

struct RapidfireSessionRuntime {
    count: u64,
    status: RapidfireSessionStatus,
    control_tx: Option<mpsc::Sender<SessionControl>>,
    compensate_now: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RapidfireSessionStatus {
    Firing,
    Stopping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionControl {
    StopWithCompensation,
    Cancel,
}

struct PendingRapidfirePosition {
    group_id: String,
    original_position: RapidfireRect,
    staged_position: RapidfireRect,
    sender: oneshot::Sender<RapidfireSelectionKind>,
}

impl RapidfireStateInner {
    fn bootstrap(&self) -> RapidfireBootstrap {
        RapidfireBootstrap {
            settings: self.settings.clone(),
            runs: self.run_states(),
            hotkey_error: self.hotkey_error.clone(),
        }
    }

    fn run_states(&self) -> Vec<RapidfireRunState> {
        self.settings
            .cards
            .iter()
            .map(|card| {
                let run = self.runs.get(&card.id);
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
}

impl CardRuntime {
    fn aggregate_status(&self) -> RapidfireRunStatus {
        if self.sessions.is_empty() {
            RapidfireRunStatus::Idle
        } else {
            RapidfireRunStatus::Firing
        }
    }

    fn aggregate_count(&self) -> u64 {
        self.sessions.values().map(|session| session.count).sum()
    }
}

// ---- Key mapping ----

#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetFirePlan {
    target_key: enigo::Key,
    trigger_key_to_release: Option<enigo::Key>,
    actions: Vec<TargetKeyAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKeyAction {
    ReleaseHeldTrigger,
    PressTarget,
    ReleaseTarget,
}

fn target_fire_plan(
    target_key: &str,
    held_trigger_key: Option<&str>,
    force_release_trigger: bool,
) -> Result<TargetFirePlan, String> {
    let target_key =
        parse_target_key(target_key).ok_or_else(|| format!("不支持的目标键: {target_key}"))?;
    let held_trigger_key = held_trigger_key
        .map(trigger_primary_label)
        .transpose()?
        .map(|key| parse_target_key(&key).ok_or_else(|| format!("不支持的触发键: {key}")))
        .transpose()?;
    let trigger_key_to_release = held_trigger_key.filter(|trigger_key| {
        trigger_key == &target_key || force_release_trigger
    });
    let should_release_trigger_key = trigger_key_to_release.is_some();

    Ok(TargetFirePlan {
        target_key,
        trigger_key_to_release,
        actions: target_fire_actions(should_release_trigger_key),
    })
}

fn target_fire_actions(has_held_trigger_key: bool) -> Vec<TargetKeyAction> {
    let mut actions = Vec::new();
    if has_held_trigger_key {
        actions.push(TargetKeyAction::ReleaseHeldTrigger);
    }
    actions.push(TargetKeyAction::PressTarget);
    actions.push(TargetKeyAction::ReleaseTarget);
    actions
}

fn press_jitter_duration_ms(min_ms: u64, max_ms: u64) -> u64 {
    let span = max_ms - min_ms + 1;
    let counter = RAPIDFIRE_JITTER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::from(duration.subsec_nanos()))
        .unwrap_or(0);

    min_ms + ((nanos ^ counter.rotate_left(13)) % span)
}

/// 将目标键字符串映射为 enigo Key，并执行真实按下/抬起
fn press_release_target_key(
    target_key: &str,
    held_trigger_key: Option<&str>,
    press_jitter_min_ms: u64,
    press_jitter_max_ms: u64,
    force_release_trigger: bool,
) -> Result<(), String> {
    use enigo::{Direction, Enigo, Keyboard, Settings};

    let plan = target_fire_plan(target_key, held_trigger_key, force_release_trigger)?;
    let key_str = target_key.to_string();
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|error| format!("初始化连发输入失败: {error}"))?;

    if let Some(trigger_key) = plan.trigger_key_to_release {
        enigo
            .key(trigger_key, Direction::Release)
            .map_err(|error| format!("释放连发触发键失败: {error}"))?;
        thread::sleep(Duration::from_millis(RAPIDFIRE_TRIGGER_RELEASE_SETTLE_MS));
    }

    enigo
        .key(plan.target_key.clone(), Direction::Press)
        .map_err(|error| format!("按下连发目标键 {key_str} 失败: {error}"))?;
    thread::sleep(Duration::from_millis(press_jitter_duration_ms(
        press_jitter_min_ms,
        press_jitter_max_ms,
    )));
    enigo
        .key(plan.target_key, Direction::Release)
        .map_err(|error| format!("抬起连发目标键 {key_str} 失败: {error}"))
}

fn parse_target_key(key: &str) -> Option<enigo::Key> {
    use enigo::Key;
    let upper = key.trim().to_uppercase();
    match upper.as_str() {
        "A" => Some(Key::A),
        "B" => Some(Key::B),
        "C" => Some(Key::C),
        "D" => Some(Key::D),
        "E" => Some(Key::E),
        "F" => Some(Key::F),
        "G" => Some(Key::G),
        "H" => Some(Key::H),
        "I" => Some(Key::I),
        "J" => Some(Key::J),
        "K" => Some(Key::K),
        "L" => Some(Key::L),
        "M" => Some(Key::M),
        "N" => Some(Key::N),
        "O" => Some(Key::O),
        "P" => Some(Key::P),
        "Q" => Some(Key::Q),
        "R" => Some(Key::R),
        "S" => Some(Key::S),
        "T" => Some(Key::T),
        "U" => Some(Key::U),
        "V" => Some(Key::V),
        "W" => Some(Key::W),
        "X" => Some(Key::X),
        "Y" => Some(Key::Y),
        "Z" => Some(Key::Z),
        "0" => Some(Key::Num0),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
        "SPACE" => Some(Key::Space),
        "ENTER" | "RETURN" => Some(Key::Return),
        "TAB" => Some(Key::Tab),
        "ESC" | "ESCAPE" => Some(Key::Escape),
        "BACKSPACE" => Some(Key::Backspace),
        "UP" => Some(Key::UpArrow),
        "DOWN" => Some(Key::DownArrow),
        "LEFT" => Some(Key::LeftArrow),
        "RIGHT" => Some(Key::RightArrow),
        "HOME" => Some(Key::Home),
        "END" => Some(Key::End),
        "PAGEUP" => Some(Key::PageUp),
        "PAGEDOWN" => Some(Key::PageDown),
        "INSERT" => Some(Key::Insert),
        "DELETE" => Some(Key::Delete),
        "ALT" => Some(Key::Alt),
        ";" | "SEMICOLON" => Some(Key::OEM1),
        "," | "COMMA" => Some(Key::OEMComma),
        "." | "PERIOD" => Some(Key::OEMPeriod),
        "/" | "SLASH" => Some(Key::OEM2),
        "\\" | "BACKSLASH" => Some(Key::OEM5),
        "[" | "BRACKETLEFT" => Some(Key::OEM4),
        "]" | "BRACKETRIGHT" => Some(Key::OEM6),
        "-" | "MINUS" => Some(Key::OEMMinus),
        "=" | "EQUAL" => Some(Key::OEMPlus),
        "+" | "PLUS" => Some(Key::Add),
        "`" | "BACKQUOTE" => Some(Key::OEM3),
        "'" | "QUOTE" => Some(Key::OEM7),
        _ => None,
    }
}

fn trigger_primary_label(trigger_key: &str) -> Result<String, String> {
    hotkey_types::hotkey_primary_label(trigger_key)
        .map_err(|_| format!("不支持的触发键: {trigger_key}"))
}

// ---- Normalization ----

fn normalize_card(card: &RapidfireCard) -> Result<RapidfireCard, String> {
    let name = card.name.trim();
    if name.is_empty() {
        return Err("连发器卡片名称不能为空".to_string());
    }

    let trigger_key = normalize_trigger_key(&card.trigger_key)
        .map_err(|error| format!("{name} 的触发键{error}"))?;
    if trigger_key.is_empty() {
        return Err(format!("{} 的触发键不能为空", name));
    }
    let trigger_primary = trigger_primary_label(&trigger_key)?;
    if parse_target_key(&trigger_primary).is_none() {
        return Err(format!("{} 的触发键不支持: {}", name, trigger_primary));
    }

    let target_key = normalize_single_key(&card.target_key)
        .map_err(|error| format!("{name} 的目标键{error}"))?;
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
    let min_press_spacing_ms = card.min_press_spacing_ms;
    let trigger_jitter_max_ms = card.trigger_jitter_max_ms;

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
        min_press_spacing_ms,
        trigger_jitter_max_ms,
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

    if !groups
        .iter()
        .any(|group| group.id == DEFAULT_RAPIDFIRE_GROUP_ID)
    {
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

fn group_enabled(groups: &[RapidfireGroup], group_id: &str) -> bool {
    groups
        .iter()
        .find(|group| group.id == group_id)
        .map(|group| group.enabled)
        .unwrap_or(false)
}

fn group_id_set(groups: &[RapidfireGroup]) -> HashMap<String, bool> {
    let mut map = HashMap::new();
    map.insert(DEFAULT_RAPIDFIRE_GROUP_ID.to_string(), true);
    for group in groups {
        let id = group.id.trim();
        if !id.is_empty() {
            map.insert(id.to_string(), true);
        }
    }
    map
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

fn normalize_settings(mut settings_value: RapidfireSettings) -> Result<RapidfireSettings, String> {
    if settings_value.cards.is_empty() {
        settings_value.cards.push(
            RapidfireSettings::default()
                .cards
                .into_iter()
                .next()
                .unwrap(),
        );
    }

    settings_value.overlay_width = settings_value
        .overlay_width
        .max(RAPIDFIRE_DISPLAY_MIN_WIDTH)
        .min(RAPIDFIRE_DISPLAY_MAX_WIDTH);
    let groups = normalize_groups(&settings_value)?;
    let group_ids = group_id_set(&groups);
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

    let mut seen_ids = HashMap::new();
    let mut cards = Vec::with_capacity(settings_value.cards.len());
    for card in &settings_value.cards {
        let mut normalized = normalize_card(card)?;
        if !group_ids.contains_key(&normalized.group_id) {
            normalized.group_id = DEFAULT_RAPIDFIRE_GROUP_ID.to_string();
        }
        if seen_ids.insert(normalized.id.clone(), true).is_some() {
            return Err(format!("连发器卡片 ID 重复: {}", normalized.id));
        }
        cards.push(normalized);
    }
    settings_value.groups = groups;
    if let Some(default_group) = settings_value
        .groups
        .iter()
        .find(|group| group.id == DEFAULT_RAPIDFIRE_GROUP_ID)
    {
        settings_value.show_overlay = default_group.show_overlay;
        settings_value.overlay_position = default_group.overlay_position.clone();
        settings_value.overlay_width = default_group.overlay_width;
    }
    settings_value.cards = cards;
    Ok(settings_value)
}

// ---- Hotkey registration ----

fn restart_hotkey_listeners(
    state: &RapidfireState,
    hotkey_manager: &HotkeyManager,
    settings_value: &RapidfireSettings,
) -> Result<(), String> {
    if !settings_value.rapidfire_enabled {
        return hotkey_manager.clear_hold_scope("rapidfire");
    }

    let mut by_key: HashMap<String, Vec<String>> = HashMap::new();
    for card in &settings_value.cards {
        if !card.enabled || !group_enabled(&settings_value.groups, &card.group_id) {
            continue;
        }
        by_key
            .entry(card.trigger_key.clone())
            .or_default()
            .push(card.id.clone());
    }

    let bindings = by_key
        .into_iter()
        .map(|(key, card_ids)| {
            let action: HoldActionCallback = Arc::new(move |app_handle, hold_action| {
                let card_ids = card_ids.clone();
                let hold_action = hold_action.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = handle_hold_event(&app_handle, card_ids, hold_action).await
                    {
                        let _ = app_handle.emit_to("main", "rapidfire://hotkey-error", error);
                    }
                });
            });
            (key, action)
        })
        .collect::<Vec<_>>();

    let result = hotkey_manager.replace_hold_scope("rapidfire", bindings);
    if result.is_ok() {
        if let Ok(mut inner) = state.inner.lock() {
            inner.hotkey_error = None;
        }
    }
    result
}

// ---- Core state machine ----

async fn handle_hold_event(
    app: &AppHandle,
    card_ids: Vec<String>,
    hold_action: HoldAction,
) -> Result<(), String> {
    match hold_action {
        HoldAction::Down => handle_key_down(app, card_ids).await,
        HoldAction::Up => handle_key_up(app, card_ids).await,
    }
}

async fn handle_key_down(app: &AppHandle, card_ids: Vec<String>) -> Result<(), String> {
    let state = app.state::<RapidfireState>();
    let sessions_to_spawn = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;

        if !inner.settings.rapidfire_enabled {
            return Ok(());
        }

        let compensation_delay_min_ms = inner.settings.compensation_delay_min_ms;
        let compensation_delay_max_ms = inner.settings.compensation_delay_max_ms;

        // 同一次按键按下可能触发多张卡片；只要其中任意一张要求忽略触发键，
        // 就对该触发键下的所有卡片统一释放触发键，避免其他卡片把触发键同步输入。
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
            let cid_owned = card_id.clone();
            if let Some(info) = inner
                .settings
                .cards
                .iter()
                .find(|c| {
                    c.id == cid_owned
                        && c.enabled
                        && group_enabled(&inner.settings.groups, &c.group_id)
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
            _ignore_trigger_key,
        ) in card_infos
        {
            let session_id = next_session_id();
            let (control_tx, control_rx) = mpsc::channel();
            let compensate_now = Arc::new(AtomicBool::new(false));
            let run = inner.runs.entry(cid.clone()).or_default();
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
                ignore_trigger_key: ignore_trigger_key_for_batch,
                control_rx,
                compensate_now,
                last_press_at,
            });
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
                .inner
                .lock()
                .map_err(|_| "连发器状态已损坏".to_string())?;
            inner.bootstrap()
        };
        emit_state(app, bootstrap);
    }
    Ok(())
}

async fn handle_key_up(app: &AppHandle, card_ids: Vec<String>) -> Result<(), String> {
    let state = app.state::<RapidfireState>();
    let stopped_count = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;

        if !inner.settings.rapidfire_enabled {
            return Ok(());
        }

        let mut stopped_count = 0usize;

        for card_id in &card_ids {
            let Some(run) = inner.runs.get_mut(card_id) else {
                continue;
            };

            if stop_latest_active_session(run, SessionControl::StopWithCompensation) {
                stopped_count += 1;
            }
        }

        stopped_count
    };

    if stopped_count > 0 {
        emit_state(app, {
            let inner = state
                .inner
                .lock()
                .map_err(|_| "连发器状态已损坏".to_string())?;
            inner.bootstrap()
        });
    }
    Ok(())
}

struct RapidfireSessionWorker {
    card_id: String,
    session_id: String,
    trigger_key: String,
    target_key: String,
    interval_ms: u64,
    press_jitter_min_ms: u64,
    press_jitter_max_ms: u64,
    skip_compensation: bool,
    compensation_delay_min_ms: u64,
    compensation_delay_max_ms: u64,
    min_press_spacing_ms: u64,
    /// 触发按键抖动延迟上限（毫秒，0=关闭）
    trigger_jitter_max_ms: u64,
    /// 抖动期间释放按键是否立即触发并追加
    cancel_jitter_on_release: bool,
    /// 触发过程中是否忽略触发键本身
    ignore_trigger_key: bool,
    control_rx: mpsc::Receiver<SessionControl>,
    compensate_now: Arc<AtomicBool>,
    last_press_at: Arc<Mutex<Instant>>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerDecision {
    Fire { stop_after_fire: bool },
    Stop,
    Cancel,
}

fn next_session_id() -> String {
    let id = NEXT_RAPIDFIRE_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    format!("rapidfire-session-{id}")
}

fn ensure_press_spacing(last_press_at: &Mutex<Instant>, min_press_spacing_ms: u64) {
    if min_press_spacing_ms == 0 {
        return;
    }

    let min_spacing = Duration::from_millis(min_press_spacing_ms);
    let wait = {
        let Ok(mut last) = last_press_at.lock() else {
            return;
        };
        let now = Instant::now();
        if *last > now {
            let wait = last.duration_since(now);
            *last = last
                .checked_add(min_spacing)
                .unwrap_or_else(|| now + min_spacing);
            wait
        } else {
            *last = now
                .checked_add(min_spacing)
                .unwrap_or_else(|| now + min_spacing);
            Duration::ZERO
        }
    };
    if !wait.is_zero() {
        thread::sleep(wait);
    }
}

fn spawn_session_worker(app: AppHandle, worker: RapidfireSessionWorker) {
    let name = format!("rapidfire-{}", worker.session_id);
    let error_app = app.clone();
    let error_card_id = worker.card_id.clone();
    let error_session_id = worker.session_id.clone();
    let spawn_result = thread::Builder::new()
        .name(name)
        .spawn(move || run_session_worker(app, worker));

    if let Err(error) = spawn_result {
        finish_session(&error_app, &error_card_id, &error_session_id);
        emit_hotkey_error(&error_app, format!("启动连发器线程失败: {error}"));
    }
}

fn should_compensate_count(count: u64, skip_compensation: bool) -> bool {
    count % 2 == 1 && !skip_compensation
}

fn run_session_worker(app: AppHandle, worker: RapidfireSessionWorker) {
    let interval = Duration::from_millis(worker.interval_ms.max(RAPIDFIRE_MIN_INTERVAL_MS));
    let mut count = 0u64;
    let mut next_fire_at = Instant::now();

    // 触发抖动延迟：按键按下后等待 jitter 时长再开始连发
    if worker.trigger_jitter_max_ms > 0 {
        let jitter_duration = Duration::from_millis(worker.trigger_jitter_max_ms);
        let jitter_deadline = Instant::now() + jitter_duration;
        let mut early_release = false;
        while Instant::now() < jitter_deadline {
            let remaining = jitter_deadline.saturating_duration_since(Instant::now());
            match worker.control_rx.recv_timeout(remaining) {
                Ok(SessionControl::StopWithCompensation) => {
                    if worker.cancel_jitter_on_release {
                        // 抖动期间释放：立即触发一次并执行追加判定
                        early_release = true;
                        break;
                    }
                    // 不取消抖动：继续等待
                }
                Ok(SessionControl::Cancel) => {
                    finish_session(&app, &worker.card_id, &worker.session_id);
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    break; // jitter 到期
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    finish_session(&app, &worker.card_id, &worker.session_id);
                    return;
                }
            }
        }

        if early_release {
            // 抖动期间释放：触发一次按压
            ensure_press_spacing(&worker.last_press_at, worker.min_press_spacing_ms);
            match press_release_target_key(
                &worker.target_key,
                Some(&worker.trigger_key),
                worker.press_jitter_min_ms,
                worker.press_jitter_max_ms,
                worker.ignore_trigger_key,
            ) {
                Ok(()) => {
                    count = 1;
                    let _ = update_session_count(&app, &worker.card_id, &worker.session_id, count);
                }
                Err(error) => emit_hotkey_error(&app, error),
            }
            // 进入补偿判断（main loop 后的 should_compensate_count 逻辑）
        }
    }

    // 如果是早期释放且已触发，跳转到补偿阶段
    if worker.trigger_jitter_max_ms > 0 && count > 0 {
        // 直接进入补偿判断，跳过主循环
    } else {
        loop {
            match wait_for_next_fire(&worker.control_rx, next_fire_at, count) {
                WorkerDecision::Fire { stop_after_fire } => {
                    ensure_press_spacing(&worker.last_press_at, worker.min_press_spacing_ms);
                    match press_release_target_key(
                        &worker.target_key,
                        Some(&worker.trigger_key),
                        worker.press_jitter_min_ms,
                        worker.press_jitter_max_ms,
                        worker.ignore_trigger_key,
                    ) {
                        Ok(()) => {
                            count += 1;
                            if !update_session_count(
                                &app,
                                &worker.card_id,
                                &worker.session_id,
                                count,
                            ) {
                                return;
                            }
                        }
                        Err(error) => {
                            emit_hotkey_error(&app, error);
                            break;
                        }
                    }

                    if stop_after_fire {
                        break;
                    }
                    next_fire_at = Instant::now()
                        .checked_add(interval)
                        .unwrap_or_else(Instant::now);
                }
                WorkerDecision::Stop => {
                    break;
                }
                WorkerDecision::Cancel => {
                    finish_session(&app, &worker.card_id, &worker.session_id);
                    return;
                }
            }
        }
    }

    if should_compensate_count(count, worker.skip_compensation) {
        let compensation_delay = press_jitter_duration_ms(
            worker.compensation_delay_min_ms,
            worker.compensation_delay_max_ms,
        );
        let compensation_deadline = Instant::now() + Duration::from_millis(compensation_delay);
        while Instant::now() < compensation_deadline {
            if worker.compensate_now.load(Ordering::Relaxed) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        ensure_press_spacing(&worker.last_press_at, worker.min_press_spacing_ms);
        match press_release_target_key(
            &worker.target_key,
            None,
            worker.press_jitter_min_ms,
            worker.press_jitter_max_ms,
            false,
        ) {
            Ok(()) => {
                count += 1;
                let _ = update_session_count(&app, &worker.card_id, &worker.session_id, count);
            }
            Err(error) => emit_hotkey_error(&app, error),
        }
    }

    finish_session(&app, &worker.card_id, &worker.session_id);
}

fn wait_for_next_fire(
    control_rx: &mpsc::Receiver<SessionControl>,
    fire_at: Instant,
    count: u64,
) -> WorkerDecision {
    loop {
        let now = Instant::now();
        if now >= fire_at {
            return match control_rx.try_recv() {
                Ok(SessionControl::StopWithCompensation) if count == 0 => WorkerDecision::Fire {
                    stop_after_fire: true,
                },
                Ok(SessionControl::StopWithCompensation) => WorkerDecision::Stop,
                Ok(SessionControl::Cancel) => WorkerDecision::Cancel,
                Err(mpsc::TryRecvError::Empty) => WorkerDecision::Fire {
                    stop_after_fire: false,
                },
                Err(mpsc::TryRecvError::Disconnected) => WorkerDecision::Cancel,
            };
        }

        let wait_for = fire_at.saturating_duration_since(now);
        match control_rx.recv_timeout(wait_for) {
            Ok(SessionControl::StopWithCompensation) if count == 0 => {
                return WorkerDecision::Fire {
                    stop_after_fire: true,
                };
            }
            Ok(SessionControl::StopWithCompensation) => return WorkerDecision::Stop,
            Ok(SessionControl::Cancel) => return WorkerDecision::Cancel,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return WorkerDecision::Cancel,
        }
    }
}

fn update_session_count(app: &AppHandle, card_id: &str, session_id: &str, count: u64) -> bool {
    let state = app.state::<RapidfireState>();
    let bootstrap = {
        let Ok(mut inner) = state.inner.lock() else {
            emit_hotkey_error(app, "连发器状态已损坏".to_string());
            return false;
        };

        let Some(run) = inner.runs.get_mut(card_id) else {
            return false;
        };
        let Some(session) = run.sessions.get_mut(session_id) else {
            return false;
        };

        session.count = count;
        inner.bootstrap()
    };

    emit_state(app, bootstrap);
    true
}

fn finish_session(app: &AppHandle, card_id: &str, session_id: &str) {
    let state = app.state::<RapidfireState>();
    let bootstrap = {
        let Ok(mut inner) = state.inner.lock() else {
            emit_hotkey_error(app, "连发器状态已损坏".to_string());
            return;
        };

        let should_remove_run = if let Some(run) = inner.runs.get_mut(card_id) {
            run.sessions.remove(session_id);
            run.active_session_ids.retain(|id| id != session_id);
            run.sessions.is_empty()
        } else {
            false
        };

        if should_remove_run {
            inner.runs.remove(card_id);
        }

        inner.bootstrap()
    };

    emit_state(app, bootstrap);
}

fn stop_latest_active_session(run: &mut CardRuntime, control: SessionControl) -> bool {
    while let Some(session_id) = run.active_session_ids.pop() {
        let Some(session) = run.sessions.get_mut(&session_id) else {
            continue;
        };

        session.status = RapidfireSessionStatus::Stopping;
        if let Some(control_tx) = session.control_tx.take() {
            let _ = control_tx.send(control);
            return true;
        }
    }

    false
}

fn stop_all_sessions(runs: &mut HashMap<String, CardRuntime>, control: SessionControl) {
    for run in runs.values_mut() {
        run.active_session_ids.clear();
        for session in run.sessions.values_mut() {
            session.status = RapidfireSessionStatus::Stopping;
            if let Some(control_tx) = session.control_tx.take() {
                let _ = control_tx.send(control);
            }
        }
    }
}

fn stop_removed_or_disabled_sessions(
    runs: &mut HashMap<String, CardRuntime>,
    active_card_ids: &[String],
) {
    let removed_ids = runs
        .keys()
        .filter(|id| !active_card_ids.contains(id))
        .cloned()
        .collect::<Vec<_>>();

    for id in &removed_ids {
        if let Some(run) = runs.get_mut(id) {
            run.active_session_ids.clear();
            for session in run.sessions.values_mut() {
                session.status = RapidfireSessionStatus::Stopping;
                if let Some(control_tx) = session.control_tx.take() {
                    let _ = control_tx.send(SessionControl::Cancel);
                }
            }
        }
    }

    for id in removed_ids {
        runs.remove(&id);
    }
}

fn emit_hotkey_error(app: &AppHandle, error: String) {
    let _ = app.emit_to("main", "rapidfire://hotkey-error", error);
}

// ---- Event emission ----

fn emit_state(app: &AppHandle, bootstrap: RapidfireBootstrap) {
    let _ = app.emit_to("main", "rapidfire://state-changed", bootstrap.clone());
    for group in &bootstrap.settings.groups {
        let _ = app.emit_to(
            display_label_for_group(&group.id),
            "rapidfire://state-changed",
            bootstrap.clone(),
        );
    }
}

// ---- Window management ----

fn display_height(item_count: usize) -> i32 {
    RAPIDFIRE_DISPLAY_MIN_HEIGHT.max(32 + item_count.max(1) as i32 * 28)
}

fn ensure_overlay_window(
    app: &AppHandle,
    settings_value: &RapidfireSettings,
) -> Result<(), String> {
    let mut active_labels = std::collections::HashSet::new();
    for group in &settings_value.groups {
        let label = display_label_for_group(&group.id);
        active_labels.insert(label.clone());
        ensure_overlay_window_for_group(app, settings_value, group, &label)?;
    }
    destroy_stale_windows(app, RAPIDFIRE_DISPLAY_LABEL, &active_labels);
    Ok(())
}

fn ensure_overlay_window_for_group(
    app: &AppHandle,
    settings_value: &RapidfireSettings,
    group: &RapidfireGroup,
    label: &str,
) -> Result<(), String> {
    if !group.show_overlay || !group.enabled || !settings_value.rapidfire_enabled {
        hide_window(app, label);
        return Ok(());
    }

    let enabled_count = settings_value
        .cards
        .iter()
        .filter(|c| c.enabled && c.group_id == group.id)
        .count();
    let height = display_height(enabled_count);
    let width = group.overlay_width;
    let pos = group
        .overlay_position
        .as_ref()
        .map(|p| (p.x, p.y))
        .unwrap_or((100, 100));

    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_size(PhysicalSize::new(width as u32, height as u32));
        let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
        let _ = window.set_always_on_top(true);
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.show();
        return Ok(());
    }

    let group_query_id = encoded_query_value(&group.id);
    let window = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(
            format!("index.html?mode=rapidfire-display&groupId={group_query_id}").into(),
        ),
    )
    .title(format!("连发器透明窗口 - {}", group.name))
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(true)
    .resizable(false)
    .inner_size(width as f64, height as f64)
    .position(pos.0 as f64, pos.1 as f64)
    .build()
    .map_err(|error| format!("创建连发器透明窗口失败: {error}"))?;

    let _ = window.set_ignore_cursor_events(true);
    Ok(())
}

fn display_label_for_group(group_id: &str) -> String {
    if group_id == DEFAULT_RAPIDFIRE_GROUP_ID {
        RAPIDFIRE_DISPLAY_LABEL.to_string()
    } else {
        format!(
            "{}-{}",
            RAPIDFIRE_DISPLAY_LABEL,
            safe_label_component(group_id)
        )
    }
}

fn position_label_for_group(group_id: &str) -> String {
    if group_id == DEFAULT_RAPIDFIRE_GROUP_ID {
        RAPIDFIRE_POSITION_LABEL.to_string()
    } else {
        format!(
            "{}-{}",
            RAPIDFIRE_POSITION_LABEL,
            safe_label_component(group_id)
        )
    }
}

fn destroy_display_windows(app: &AppHandle) {
    destroy_windows_with_prefix(app, RAPIDFIRE_DISPLAY_LABEL);
}

fn destroy_position_windows(app: &AppHandle) {
    destroy_windows_with_prefix(app, RAPIDFIRE_POSITION_LABEL);
}

// ---- Initialize / Shutdown ----

pub fn shutdown(app: &AppHandle, state: &RapidfireState, hotkey_manager: &HotkeyManager) {
    let _ = hotkey_manager.clear_hold_scope("rapidfire");
    if let Ok(mut inner) = state.inner.lock() {
        stop_all_sessions(&mut inner.runs, SessionControl::Cancel);
        inner.runs.clear();
    }
    destroy_position_windows(app);
    destroy_display_windows(app);
}

pub fn initialize(
    app: &AppHandle,
    hotkey_manager: &HotkeyManager,
) -> Result<RapidfireState, String> {
    let settings = normalize_settings(settings::load_settings(app)?)?;
    let state = RapidfireState {
        inner: Mutex::new(RapidfireStateInner {
            settings: settings.clone(),
            runs: HashMap::new(),
            pending_position: None,
            hotkey_error: None,
        }),
    };

    if settings.rapidfire_enabled {
        if let Err(error) = restart_hotkey_listeners(&state, hotkey_manager, &settings) {
            if let Ok(mut inner) = state.inner.lock() {
                inner.hotkey_error = Some(error);
            }
        }
        ensure_overlay_window(app, &settings)?;
    }

    Ok(state)
}

// ---- Tauri commands ----

#[tauri::command]
pub fn rapidfire_get_bootstrap(
    state: State<'_, RapidfireState>,
) -> Result<RapidfireBootstrap, AppError> {
    let inner = state
        .inner
        .lock()
        .map_err(|_| "连发器状态已损坏".to_string())?;
    Ok(inner.bootstrap())
}

#[tauri::command]
pub fn rapidfire_save_settings(
    settings_value: RapidfireSettings,
    app: AppHandle,
    state: State<'_, RapidfireState>,
    hotkey_manager: State<'_, HotkeyManager>,
) -> Result<RapidfireBootstrap, AppError> {
    let settings_value = normalize_settings(settings_value)?;
    let previous_settings = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        inner.settings.clone()
    };

    settings::save_settings(&app, &settings_value)?;

    if let Err(error) = restart_hotkey_listeners(&state, &hotkey_manager, &settings_value) {
        let _ = settings::save_settings(&app, &previous_settings);
        let _ = restart_hotkey_listeners(&state, &hotkey_manager, &previous_settings);
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        inner.hotkey_error = Some(error.clone());
        return Err(AppError::from(error));
    }

    let bootstrap = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        inner.settings = settings_value.clone();
        inner.hotkey_error = None;

        let active_card_ids: Vec<String> = settings_value
            .cards
            .iter()
            .filter(|c| c.enabled && group_enabled(&settings_value.groups, &c.group_id))
            .map(|c| c.id.clone())
            .collect();
        stop_removed_or_disabled_sessions(&mut inner.runs, &active_card_ids);

        if !settings_value.rapidfire_enabled {
            stop_all_sessions(&mut inner.runs, SessionControl::Cancel);
            inner.runs.clear();
        }

        inner.bootstrap()
    };

    ensure_overlay_window(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub fn rapidfire_stop(
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<RapidfireBootstrap, AppError> {
    let bootstrap = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        stop_all_sessions(&mut inner.runs, SessionControl::Cancel);
        inner.runs.clear();
        inner.bootstrap()
    };
    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub async fn rapidfire_begin_position_selection(
    group_id: Option<String>,
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<RapidfireSelectionOutcome, AppError> {
    let (sender, receiver) = oneshot::channel();
    let group_id = group_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_RAPIDFIRE_GROUP_ID.to_string());
    let position = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;

        if inner.pending_position.is_some() {
            return Err(AppError::Message("当前已有一个位置设置流程在进行中".to_string()));
        }

        let pos = inner
            .settings
            .groups
            .iter()
            .find(|group| group.id == group_id)
            .and_then(|group| group.overlay_position.clone())
            .or_else(|| inner.settings.overlay_position.clone())
            .unwrap_or(RapidfireRect { x: 100, y: 100 });

        inner.pending_position = Some(PendingRapidfirePosition {
            group_id: group_id.clone(),
            original_position: pos.clone(),
            staged_position: pos.clone(),
            sender,
        });
        pos
    };

    let position_label = position_label_for_group(&group_id);
    destroy_window(&app, &position_label);

    let display_width = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        inner
            .settings
            .groups
            .iter()
            .find(|group| group.id == group_id)
            .map(|group| group.overlay_width)
            .unwrap_or(inner.settings.overlay_width)
    };
    let display_height = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        display_height(
            inner
                .settings
                .cards
                .iter()
                .filter(|c| c.enabled && c.group_id == group_id)
                .count(),
        )
    };

    let group_query_id = encoded_query_value(&group_id);
    let window = WebviewWindowBuilder::new(
        &app,
        &position_label,
        WebviewUrl::App(
            format!("index.html?mode=rapidfire-position&groupId={group_query_id}").into(),
        ),
    )
    .title("设置连发器位置")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(true)
    .visible(true)
    .resizable(false)
    .inner_size(display_width as f64, display_height as f64)
    .position(position.x as f64, position.y as f64)
    .build()
    .map_err(|error| format!("创建连发器位置设置窗口失败: {error}"))?;

    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Destroyed | WindowEvent::CloseRequested { .. }
        ) {
            let state = close_app.state::<RapidfireState>();
            let mut inner = match state.inner.lock() {
                Ok(inner) => inner,
                Err(_) => return,
            };
            if let Some(pending) = inner.pending_position.take() {
                let _ = pending.sender.send(RapidfireSelectionKind::Closed);
            }
        }
    });

    let kind = match receiver.await {
        Ok(kind) => kind,
        Err(_) => RapidfireSelectionKind::Closed,
    };
    destroy_window(&app, &position_label);

    let position = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        inner
            .settings
            .groups
            .iter()
            .find(|group| group.id == group_id)
            .and_then(|group| group.overlay_position.clone())
            .or_else(|| inner.settings.overlay_position.clone())
            .unwrap_or(RapidfireRect { x: 100, y: 100 })
    };

    Ok(RapidfireSelectionOutcome {
        kind,
        position,
        group_id: Some(group_id),
    })
}

#[tauri::command]
pub fn rapidfire_position_commit(
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<RapidfireBootstrap, AppError> {
    let (sender, group_id, bootstrap) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.take() else {
            return Err(AppError::Message("当前没有等待中的位置设置流程".to_string()));
        };

        let group_id = pending.group_id.clone();
        if let Some(group) = inner
            .settings
            .groups
            .iter_mut()
            .find(|group| group.id == group_id)
        {
            group.overlay_position = Some(pending.staged_position.clone());
        }
        if group_id == DEFAULT_RAPIDFIRE_GROUP_ID {
            inner.settings.overlay_position = Some(pending.staged_position.clone());
        }
        settings::save_settings(&app, &inner.settings)?;
        (pending.sender, group_id, inner.bootstrap())
    };

    let _ = sender.send(RapidfireSelectionKind::Selected);
    destroy_window(&app, &position_label_for_group(&group_id));
    ensure_overlay_window(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub fn rapidfire_position_cancel(
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<(), AppError> {
    let (sender, group_id, _original_position) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.take() else {
            return Err(AppError::Message("当前没有等待中的位置设置流程".to_string()));
        };

        let original = pending.original_position.clone();
        let group_id = pending.group_id.clone();
        if let Some(group) = inner
            .settings
            .groups
            .iter_mut()
            .find(|group| group.id == group_id)
        {
            group.overlay_position = Some(original.clone());
        }
        if group_id == DEFAULT_RAPIDFIRE_GROUP_ID {
            inner.settings.overlay_position = Some(original.clone());
        }
        (pending.sender, group_id, original)
    };

    let _ = sender.send(RapidfireSelectionKind::Cancelled);
    destroy_window(&app, &position_label_for_group(&group_id));
    Ok(())
}

#[tauri::command]
pub fn rapidfire_position_moved(
    x: i32,
    y: i32,
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<RapidfireRect, AppError> {
    let (rect, group_id) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.as_mut() else {
            return Err(AppError::Message("当前没有等待中的位置设置流程".to_string()));
        };

        pending.staged_position.x = x;
        pending.staged_position.y = y;
        (pending.staged_position.clone(), pending.group_id.clone())
    };

    if let Some(window) = app.get_webview_window(&position_label_for_group(&group_id)) {
        let _ = window.set_position(PhysicalPosition::new(rect.x, rect.y));
    }

    Ok(rect)
}

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
        let error = normalize_card(&card).unwrap_err();
        assert!(error.contains("名称不能为空"));
    }

    #[test]
    fn normalize_card_clamps_interval_to_minimum() {
        let mut card = sample_card("a", "F1");
        card.interval_ms = 0;
        let normalized = normalize_card(&card).unwrap();
        assert_eq!(normalized.interval_ms, RAPIDFIRE_MIN_INTERVAL_MS);
    }

    #[test]
    fn normalize_card_clamps_press_jitter_to_supported_range() {
        let mut card = sample_card("a", "F1");
        card.press_jitter_min_ms = 0;
        card.press_jitter_max_ms = 2500;

        let normalized = normalize_card(&card).unwrap();

        assert_eq!(
            normalized.press_jitter_min_ms,
            RAPIDFIRE_PRESS_JITTER_MIN_MS
        );
        assert_eq!(
            normalized.press_jitter_max_ms,
            RAPIDFIRE_PRESS_JITTER_MAX_MS
        );
    }

    #[test]
    fn normalize_card_preserves_press_jitter_at_new_upper_bound() {
        let mut card = sample_card("a", "F1");
        card.press_jitter_min_ms = 1990;
        card.press_jitter_max_ms = 2000;

        let normalized = normalize_card(&card).unwrap();

        assert_eq!(normalized.press_jitter_min_ms, 1990);
        assert_eq!(normalized.press_jitter_max_ms, 2000);
    }

    #[test]
    fn normalize_card_rejects_inverted_press_jitter_range() {
        let mut card = sample_card("a", "F1");
        card.press_jitter_min_ms = 30;
        card.press_jitter_max_ms = 20;

        let error = normalize_card(&card).unwrap_err();

        assert!(error.contains("触发抖动"));
    }

    #[test]
    fn normalize_card_preserves_skip_compensation() {
        let mut card = sample_card("a", "F1");
        card.skip_compensation = true;

        let normalized = normalize_card(&card).unwrap();

        assert!(normalized.skip_compensation);
    }

    #[test]
    fn normalize_card_rejects_unsupported_target_key() {
        let mut card = sample_card("a", "F1");
        card.target_key = "UnknownKey".to_string();
        let error = normalize_card(&card).unwrap_err();
        assert!(error.contains("目标键不支持"));
    }

    #[test]
    fn normalize_card_allows_modified_trigger_key() {
        let card = sample_card("a", "shift+-");
        let normalized = normalize_card(&card).unwrap();
        assert_eq!(normalized.trigger_key, "Shift+-");
    }

    #[test]
    fn normalize_card_keeps_alt_as_trigger_primary_key() {
        let card = sample_card("a", "alt");
        let normalized = normalize_card(&card).unwrap();

        assert_eq!(normalized.trigger_key, "Alt");
    }

    #[test]
    fn normalize_card_allows_plus_with_modifier_as_trigger_key() {
        let card = sample_card("a", "shift++");
        let normalized = normalize_card(&card).unwrap();

        assert_eq!(normalized.trigger_key, "Shift++");
    }

    #[test]
    fn normalize_card_rejects_unsupported_trigger_key() {
        let card = sample_card("a", "F25");
        let error = normalize_card(&card).unwrap_err();
        assert!(error.contains("触发键不支持"));
    }

    #[test]
    fn normalize_card_normalizes_key_labels() {
        let mut card = sample_card("a", "space");
        card.target_key = "escape".to_string();
        let normalized = normalize_card(&card).unwrap();

        assert_eq!(normalized.trigger_key, "Space");
        assert_eq!(normalized.target_key, "Esc");
    }

    #[test]
    fn normalize_card_normalizes_alt_key() {
        let mut card = sample_card("a", "alt");
        card.target_key = "alt".to_string();
        let normalized = normalize_card(&card).unwrap();

        assert_eq!(normalized.trigger_key, "Alt");
        assert_eq!(normalized.target_key, "Alt");
    }

    #[test]
    fn normalize_card_normalizes_symbol_keys() {
        let mut card = sample_card("a", ";");
        card.target_key = ",".to_string();
        let normalized = normalize_card(&card).unwrap();

        assert_eq!(normalized.trigger_key, ";");
        assert_eq!(normalized.target_key, ",");
    }

    #[test]
    fn parse_target_key_supports_all_valid_keys() {
        assert_eq!(parse_target_key("A"), Some(enigo::Key::A));
        assert_eq!(parse_target_key("Z"), Some(enigo::Key::Z));
        assert_eq!(parse_target_key("0"), Some(enigo::Key::Num0));
        assert_eq!(parse_target_key("9"), Some(enigo::Key::Num9));
        assert_eq!(parse_target_key(";"), Some(enigo::Key::OEM1));
        assert_eq!(parse_target_key(","), Some(enigo::Key::OEMComma));
        assert_eq!(parse_target_key("."), Some(enigo::Key::OEMPeriod));
        assert_eq!(parse_target_key("/"), Some(enigo::Key::OEM2));
        assert_eq!(parse_target_key("\\"), Some(enigo::Key::OEM5));
        assert_eq!(parse_target_key("["), Some(enigo::Key::OEM4));
        assert_eq!(parse_target_key("]"), Some(enigo::Key::OEM6));
        assert_eq!(parse_target_key("-"), Some(enigo::Key::OEMMinus));
        assert_eq!(parse_target_key("="), Some(enigo::Key::OEMPlus));
        assert_eq!(parse_target_key("+"), Some(enigo::Key::Add));
        assert_eq!(parse_target_key("`"), Some(enigo::Key::OEM3));
        assert_eq!(parse_target_key("'"), Some(enigo::Key::OEM7));
        assert!(parse_target_key("F1").is_some());
        assert!(parse_target_key("F12").is_some());
        assert!(parse_target_key("Space").is_some());
        assert!(parse_target_key("Enter").is_some());
        assert!(parse_target_key("Tab").is_some());
        assert!(parse_target_key("Esc").is_some());
        assert!(parse_target_key("Backspace").is_some());
        assert!(parse_target_key("Up").is_some());
        assert!(parse_target_key("Down").is_some());
        assert!(parse_target_key("Left").is_some());
        assert!(parse_target_key("Right").is_some());
        assert!(parse_target_key("Home").is_some());
        assert!(parse_target_key("End").is_some());
        assert!(parse_target_key("PageUp").is_some());
        assert!(parse_target_key("PageDown").is_some());
        assert!(parse_target_key("Insert").is_some());
        assert!(parse_target_key("Delete").is_some());
        assert!(parse_target_key("Alt").is_some());
        assert!(parse_target_key("Unknown").is_none());
    }

    #[test]
    fn target_fire_plan_uses_press_and_release_actions() {
        let plan = target_fire_plan("T", Some("T"), false).unwrap();

        assert_eq!(plan.target_key, parse_target_key("T").unwrap());
        assert_eq!(plan.trigger_key_to_release, parse_target_key("T"));
        assert_eq!(
            plan.actions,
            vec![
                TargetKeyAction::ReleaseHeldTrigger,
                TargetKeyAction::PressTarget,
                TargetKeyAction::ReleaseTarget
            ]
        );
    }

    #[test]
    fn target_fire_plan_releases_same_primary_trigger_for_modified_hotkey() {
        let plan = target_fire_plan("-", Some("Shift+-"), false).unwrap();

        assert_eq!(plan.target_key, parse_target_key("-").unwrap());
        assert_eq!(plan.trigger_key_to_release, parse_target_key("-"));
        assert_eq!(
            plan.actions,
            vec![
                TargetKeyAction::ReleaseHeldTrigger,
                TargetKeyAction::PressTarget,
                TargetKeyAction::ReleaseTarget
            ]
        );
    }
    #[test]
    fn target_fire_plan_keeps_different_trigger_key_held() {
        let plan = target_fire_plan("Space", Some("W"), false).unwrap();

        assert_eq!(plan.target_key, parse_target_key("Space").unwrap());
        assert_eq!(plan.trigger_key_to_release, None);
        assert_eq!(
            plan.actions,
            vec![TargetKeyAction::PressTarget, TargetKeyAction::ReleaseTarget]
        );
    }

    #[test]
    fn target_fire_plan_allows_compensation_without_held_trigger() {
        let plan = target_fire_plan("T", None, false).unwrap();

        assert_eq!(plan.target_key, parse_target_key("T").unwrap());
        assert_eq!(plan.trigger_key_to_release, None);
        assert_eq!(
            plan.actions,
            vec![TargetKeyAction::PressTarget, TargetKeyAction::ReleaseTarget]
        );
    }

    #[test]
    fn target_fire_plan_releases_different_trigger_when_forced() {
        let plan = target_fire_plan("Space", Some("W"), true).unwrap();

        assert_eq!(plan.target_key, parse_target_key("Space").unwrap());
        assert_eq!(plan.trigger_key_to_release, parse_target_key("W"));
        assert_eq!(
            plan.actions,
            vec![
                TargetKeyAction::ReleaseHeldTrigger,
                TargetKeyAction::PressTarget,
                TargetKeyAction::ReleaseTarget
            ]
        );
    }

    #[test]
    fn press_jitter_stays_within_custom_range() {
        for _ in 0..100 {
            let jitter = press_jitter_duration_ms(15, 25);

            assert!(
                (15..=25).contains(&jitter),
                "按下抖动应落在 15-25ms，实际为 {jitter}ms"
            );
        }
    }

    #[test]
    fn stop_before_first_tick_still_allows_one_fire_for_compensation() {
        let (tx, rx) = mpsc::channel();
        tx.send(SessionControl::StopWithCompensation).unwrap();

        let decision = wait_for_next_fire(&rx, Instant::now() + Duration::from_secs(1), 0);

        assert_eq!(
            decision,
            WorkerDecision::Fire {
                stop_after_fire: true
            }
        );
    }

    #[test]
    fn stop_after_existing_count_exits_worker_loop_for_compensation_stage() {
        let (tx, rx) = mpsc::channel();
        tx.send(SessionControl::StopWithCompensation).unwrap();

        let decision = wait_for_next_fire(&rx, Instant::now() + Duration::from_secs(1), 3);

        assert_eq!(decision, WorkerDecision::Stop);
    }

    #[test]
    fn should_compensate_count_respects_no_append_switch() {
        assert!(should_compensate_count(1, false));
        assert!(!should_compensate_count(2, false));
        assert!(!should_compensate_count(1, true));
    }

    #[test]
    fn same_card_can_hold_multiple_sessions_without_overwriting() {
        let mut runtime = CardRuntime::default();
        let (tx1, _rx1) = mpsc::channel();
        let (tx2, _rx2) = mpsc::channel();

        runtime.active_session_ids.push("session-1".to_string());
        runtime.sessions.insert(
            "session-1".to_string(),
            RapidfireSessionRuntime {
                count: 1,
                status: RapidfireSessionStatus::Firing,
                control_tx: Some(tx1),
                compensate_now: Arc::new(AtomicBool::new(false)),
            },
        );
        runtime.active_session_ids.push("session-2".to_string());
        runtime.sessions.insert(
            "session-2".to_string(),
            RapidfireSessionRuntime {
                count: 2,
                status: RapidfireSessionStatus::Firing,
                control_tx: Some(tx2),
                compensate_now: Arc::new(AtomicBool::new(false)),
            },
        );

        assert_eq!(runtime.aggregate_status(), RapidfireRunStatus::Firing);
        assert_eq!(runtime.aggregate_count(), 3);
        assert_eq!(runtime.sessions.len(), 2);
    }

    #[test]
    fn stop_latest_active_session_does_not_cancel_older_session() {
        let mut runtime = CardRuntime::default();
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();

        runtime.active_session_ids.push("session-1".to_string());
        runtime.sessions.insert(
            "session-1".to_string(),
            RapidfireSessionRuntime {
                count: 1,
                status: RapidfireSessionStatus::Firing,
                control_tx: Some(tx1),
                compensate_now: Arc::new(AtomicBool::new(false)),
            },
        );
        runtime.active_session_ids.push("session-2".to_string());
        runtime.sessions.insert(
            "session-2".to_string(),
            RapidfireSessionRuntime {
                count: 1,
                status: RapidfireSessionStatus::Firing,
                control_tx: Some(tx2),
                compensate_now: Arc::new(AtomicBool::new(false)),
            },
        );

        assert!(stop_latest_active_session(
            &mut runtime,
            SessionControl::StopWithCompensation
        ));

        assert!(rx1.try_recv().is_err());
        assert_eq!(
            rx2.try_recv().unwrap(),
            SessionControl::StopWithCompensation
        );
        assert_eq!(
            runtime.sessions["session-1"].status,
            RapidfireSessionStatus::Firing
        );
        assert_eq!(
            runtime.sessions["session-2"].status,
            RapidfireSessionStatus::Stopping
        );
    }

    #[test]
    fn normalize_settings_auto_adds_default_card_when_empty() {
        let mut settings = RapidfireSettings::default();
        settings.cards.clear();
        let normalized = normalize_settings(settings).unwrap();
        assert_eq!(normalized.cards.len(), 1);
    }

    #[test]
    fn normalize_settings_migrates_legacy_group() {
        let mut settings = RapidfireSettings::default();
        settings.groups.clear();
        settings.show_overlay = true;
        settings.overlay_width = 420;
        settings.overlay_position = Some(RapidfireRect { x: 11, y: 22 });
        settings.cards = vec![sample_card("a", "F1")];

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.groups.len(), 1);
        assert_eq!(normalized.groups[0].id, DEFAULT_RAPIDFIRE_GROUP_ID);
        assert!(normalized.groups[0].show_overlay);
        assert_eq!(normalized.groups[0].overlay_width, 420);
        assert_eq!(
            normalized.groups[0].overlay_position,
            Some(RapidfireRect { x: 11, y: 22 })
        );
        assert_eq!(normalized.cards[0].group_id, DEFAULT_RAPIDFIRE_GROUP_ID);
    }

    #[test]
    fn normalize_settings_rejects_duplicate_card_ids() {
        let mut settings = RapidfireSettings::default();
        settings.cards = vec![sample_card("same", "F1"), sample_card("same", "F2")];
        let error = normalize_settings(settings).unwrap_err();
        assert!(error.contains("ID 重复"));
    }

    #[test]
    fn normalize_settings_allows_same_enabled_trigger_keys() {
        let mut settings = RapidfireSettings::default();
        settings.cards = vec![sample_card("a", "F1"), sample_card("b", "f1")];

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.cards.len(), 2);
        assert_eq!(normalized.cards[0].trigger_key, "F1");
        assert_eq!(normalized.cards[1].trigger_key, "F1");
    }

    #[test]
    fn normalize_settings_allows_duplicate_disabled_trigger_keys() {
        let mut settings = RapidfireSettings::default();
        let mut disabled = sample_card("b", "F1");
        disabled.enabled = false;
        settings.cards = vec![sample_card("a", "F1"), disabled];

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.cards.len(), 2);
    }

    #[test]
    fn normalize_settings_clamps_overlay_width() {
        let mut settings = RapidfireSettings::default();
        settings.overlay_width = 100;
        let normalized = normalize_settings(settings).unwrap();
        assert_eq!(normalized.overlay_width, RAPIDFIRE_DISPLAY_MIN_WIDTH);

        let mut settings2 = RapidfireSettings::default();
        settings2.overlay_width = 1000;
        let normalized2 = normalize_settings(settings2).unwrap();
        assert_eq!(normalized2.overlay_width, RAPIDFIRE_DISPLAY_MAX_WIDTH);
    }

    #[test]
    fn normalize_settings_keeps_global_delay_parameters() {
        let mut settings = RapidfireSettings::default();
        settings.compensation_delay_min_ms = 120;
        settings.compensation_delay_max_ms = 180;
        settings.min_press_spacing_ms = 90;

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.compensation_delay_min_ms, 120);
        assert_eq!(normalized.compensation_delay_max_ms, 180);
        assert_eq!(normalized.min_press_spacing_ms, 90);
    }

    #[test]
    fn normalize_settings_rejects_inverted_global_compensation_delay() {
        let mut settings = RapidfireSettings::default();
        settings.compensation_delay_min_ms = 180;
        settings.compensation_delay_max_ms = 120;

        let error = normalize_settings(settings).unwrap_err();

        assert!(error.contains("补齐延迟"));
    }

    #[test]
    fn normalize_settings_rejects_too_large_global_delays() {
        let mut settings = RapidfireSettings::default();
        settings.compensation_delay_max_ms = RAPIDFIRE_GLOBAL_DELAY_MAX_MS + 1;
        let compensation_error = normalize_settings(settings).unwrap_err();
        assert!(compensation_error.contains("补齐延迟"));

        let mut settings2 = RapidfireSettings::default();
        settings2.min_press_spacing_ms = RAPIDFIRE_GLOBAL_DELAY_MAX_MS + 1;
        let spacing_error = normalize_settings(settings2).unwrap_err();
        assert!(spacing_error.contains("按键最小间距"));
    }

    #[test]
    fn display_height_has_minimum() {
        assert_eq!(display_height(0), RAPIDFIRE_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(1), RAPIDFIRE_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(5), 172);
    }
}
