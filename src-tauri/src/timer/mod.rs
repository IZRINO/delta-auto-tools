use std::{collections::HashMap, sync::{Arc, Mutex}};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};
use tokio::{sync::oneshot, time::{self, Duration}};

mod settings;
mod types;

use crate::hotkeys::{HotkeyAction, HotkeyManager};

use self::{
    types::{
        TimerBootstrap, TimerItem, TimerRect, TimerRunState, TimerRunStatus, TimerSelectionKind,
        TimerSelectionOutcome, TimerSettings,
    },
};

const TIMER_DISPLAY_LABEL: &str = "timer-display";
const TIMER_POSITION_LABEL: &str = "timer-position";
const TIMER_DISPLAY_WIDTH: i32 = 320;
const TIMER_DISPLAY_MIN_HEIGHT: i32 = 96;

pub struct TimerState {
    inner: Mutex<TimerStateInner>,
    tick_task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

struct TimerStateInner {
    settings: TimerSettings,
    runs: HashMap<String, TimerRuntime>,
    pending_position: Option<PendingTimerPosition>,
    hotkey_error: Option<String>,
}

struct TimerRuntime {
    ends_at_ms: Option<u64>,
    remaining_seconds: u64,
    status: TimerRunStatus,
}

struct PendingTimerPosition {
    original_rect: TimerRect,
    staged_rect: TimerRect,
    sender: oneshot::Sender<TimerSelectionKind>,
}

impl TimerStateInner {
    fn bootstrap(&self) -> TimerBootstrap {
        TimerBootstrap {
            settings: self.settings.clone(),
            runs: self.run_states(),
            hotkey_error: self.hotkey_error.clone(),
        }
    }

    fn run_states(&self) -> Vec<TimerRunState> {
        self.settings
            .timers
            .iter()
            .filter_map(|timer| {
                self.runs.get(&timer.id).map(|runtime| TimerRunState {
                    id: timer.id.clone(),
                    remaining_seconds: runtime.remaining_seconds,
                    status: runtime.status.clone(),
                })
            })
            .collect()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn display_height(timer_count: usize) -> i32 {
    TIMER_DISPLAY_MIN_HEIGHT.max(56 + timer_count.max(1) as i32 * 34)
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
        return Err(format!("{} 的倒计时秒数必须大于 0", name));
    }

    Ok(TimerItem {
        id: timer.id.trim().to_string(),
        name: name.to_string(),
        duration_seconds: timer.duration_seconds,
        hotkey: hotkey.to_string(),
    })
}

fn normalize_settings(mut settings_value: TimerSettings) -> Result<TimerSettings, String> {
    settings_value.display.rect.width = TIMER_DISPLAY_WIDTH;
    settings_value.display.rect.height = display_height(settings_value.timers.len());

    if !(0.1..=1.0).contains(&settings_value.display.font_opacity) {
        return Err("字体透明度必须在 0.1 到 1 之间".to_string());
    }

    if settings_value.timers.is_empty() {
        return Err("至少需要保留一个计时器".to_string());
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

    settings_value.timers = timers;
    Ok(settings_value)
}

fn restart_hotkey_listeners(state: &TimerState, app: &AppHandle, hotkey_manager: &HotkeyManager, settings_value: &TimerSettings) -> Result<(), String> {
    if !settings_value.enabled {
        return hotkey_manager.clear_scope("timer");
    }

    let mut by_hotkey: HashMap<String, Vec<String>> = HashMap::new();
    for timer in &settings_value.timers {
        by_hotkey
            .entry(timer.hotkey.trim().to_string())
            .or_default()
            .push(timer.id.clone());
    }

    let bindings = by_hotkey
        .into_iter()
        .map(|(hotkey, timer_ids)| {
            let action: HotkeyAction = Arc::new(move |app_handle| {
                let timer_ids = timer_ids.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = trigger_timers(&app_handle, timer_ids) {
                        let _ = app_handle.emit_to("main", "timer://hotkey-error", error);
                    }
                });
            });
            (hotkey, action)
        })
        .collect::<Vec<_>>();

    let result = hotkey_manager.replace_scope("timer", bindings);
    let _ = app;
    if result.is_ok() {
        if let Ok(mut inner) = state.inner.lock() {
            inner.hotkey_error = None;
        }
    }
    result
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
    let _ = app.emit_to(TIMER_DISPLAY_LABEL, "timer://state-changed", bootstrap);
}

fn ensure_display_window(app: &AppHandle, settings_value: &TimerSettings) -> Result<(), String> {
    if !settings_value.enabled {
        destroy_display_window(app);
        return Ok(());
    }

    let rect = &settings_value.display.rect;
    if let Some(window) = app.get_webview_window(TIMER_DISPLAY_LABEL) {
        let _ = window.set_size(PhysicalSize::new(rect.width as u32, rect.height as u32));
        let _ = window.set_position(PhysicalPosition::new(rect.x, rect.y));
        let _ = window.set_always_on_top(true);
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.show();
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        app,
        TIMER_DISPLAY_LABEL,
        WebviewUrl::App("index.html?mode=timer-display".into()),
    )
    .title("计时器透明窗口")
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
    .map_err(|error| format!("创建计时器透明窗口失败: {error}"))?;

    let _ = window.set_ignore_cursor_events(true);
    Ok(())
}

fn destroy_display_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(TIMER_DISPLAY_LABEL) {
        let _ = window.destroy();
    }
}

fn destroy_position_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(TIMER_POSITION_LABEL) {
        let _ = window.destroy();
    }
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
            if runtime.status != TimerRunStatus::Running {
                continue;
            }

            let Some(ends_at_ms) = runtime.ends_at_ms else {
                continue;
            };

            let next_remaining = ends_at_ms.saturating_sub(now).div_ceil(1000);
            if next_remaining == 0 {
                runtime.remaining_seconds = 0;
                runtime.status = TimerRunStatus::Finished;
                runtime.ends_at_ms = None;
                changed = true;
            } else if runtime.remaining_seconds != next_remaining {
                runtime.remaining_seconds = next_remaining;
                changed = true;
            }
        }

        if !changed {
            return Ok(());
        }

        inner.bootstrap()
    };

    emit_state(app, bootstrap);
    Ok(())
}

fn trigger_timers(app: &AppHandle, timer_ids: Vec<String>) -> Result<TimerBootstrap, String> {
    let state = app.state::<TimerState>();
    let triggered_timer_ids = timer_ids.clone();
    let bootstrap = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器状态已损坏".to_string())?;

        if !inner.settings.enabled {
            return Ok(inner.bootstrap());
        }

        let now = now_ms();
        for timer_id in timer_ids {
            let Some((id, duration_seconds)) = inner
                .settings
                .timers
                .iter()
                .find(|item| item.id == timer_id)
                .map(|timer| (timer.id.clone(), timer.duration_seconds))
            else {
                continue;
            };

            inner.runs.insert(id, TimerRuntime {
                ends_at_ms: Some(now + duration_seconds * 1000),
                remaining_seconds: duration_seconds,
                status: TimerRunStatus::Running,
            });
        }

        inner.bootstrap()
    };

    emit_state(app, bootstrap.clone());
    ensure_display_window(app, &bootstrap.settings)?;
    let _ = app.emit_to("main", "timer://hotkey-triggered", triggered_timer_ids);
    Ok(bootstrap)
}

pub fn initialize(app: &AppHandle, hotkey_manager: &HotkeyManager) -> Result<TimerState, String> {
    let settings = normalize_settings(settings::load_settings(app)?)?;
    let state = TimerState {
        inner: Mutex::new(TimerStateInner {
            settings: settings.clone(),
            runs: HashMap::new(),
            pending_position: None,
            hotkey_error: None,
        }),
        tick_task: Mutex::new(None),
    };

    if settings.enabled {
        if let Err(error) = restart_hotkey_listeners(&state, app, hotkey_manager, &settings) {
            if let Ok(mut inner) = state.inner.lock() {
                inner.hotkey_error = Some(error);
            }
        }
        ensure_display_window(app, &settings)?;
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

    if let Err(error) = restart_hotkey_listeners(&state, &app, &hotkey_manager, &settings_value) {
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
        inner.runs.retain(|id, _| settings_value.timers.iter().any(|timer| timer.id == *id));
        if !settings_value.enabled {
            inner.runs.clear();
        }
        inner.bootstrap()
    };

    ensure_display_window(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub fn timer_trigger(timer_ids: Vec<String>, app: AppHandle) -> Result<TimerBootstrap, String> {
    trigger_timers(&app, timer_ids)
}

#[tauri::command]
pub async fn timer_begin_position_selection(
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
            return Err("当前已有一个计时器位置设置流程在进行中".to_string());
        }

        let rect = inner.settings.display.rect.clone();
        inner.pending_position = Some(PendingTimerPosition {
            original_rect: rect.clone(),
            staged_rect: rect.clone(),
            sender,
        });
        rect
    };

    destroy_position_window(&app);

    let window = WebviewWindowBuilder::new(
        &app,
        TIMER_POSITION_LABEL,
        WebviewUrl::App("index.html?mode=timer-position".into()),
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
    .map_err(|error| format!("创建计时器位置设置窗口失败: {error}"))?;

    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed | WindowEvent::CloseRequested { .. }) {
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
    destroy_position_window(&app);

    let rect = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        inner.settings.display.rect.clone()
    };

    Ok(TimerSelectionOutcome { kind, rect })
}

#[tauri::command]
pub fn timer_position_commit(app: AppHandle, state: State<'_, TimerState>) -> Result<TimerBootstrap, String> {
    let (sender, bootstrap) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.take() else {
            return Err("当前没有等待中的计时器位置设置流程".to_string());
        };

        inner.settings.display.rect = pending.staged_rect.clone();
        settings::save_settings(&app, &inner.settings)?;
        (pending.sender, inner.bootstrap())
    };

    let _ = sender.send(TimerSelectionKind::Selected);
    ensure_display_window(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub fn timer_position_cancel(app: AppHandle, state: State<'_, TimerState>) -> Result<(), String> {
    let sender = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.take() else {
            return Err("当前没有等待中的计时器位置设置流程".to_string());
        };

        inner.settings.display.rect = pending.original_rect;
        pending.sender
    };

    let _ = sender.send(TimerSelectionKind::Cancelled);
    destroy_position_window(&app);
    Ok(())
}

#[tauri::command]
pub fn timer_position_moved(
    x: i32,
    y: i32,
    app: AppHandle,
    state: State<'_, TimerState>,
) -> Result<TimerRect, String> {
    let rect = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "计时器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.as_mut() else {
            return Err("当前没有等待中的计时器位置设置流程".to_string());
        };

        pending.staged_rect.x = x;
        pending.staged_rect.y = y;
        pending.staged_rect.clone()
    };

    if let Some(window) = app.get_webview_window(TIMER_POSITION_LABEL) {
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
        }
    }

    #[test]
    fn display_height_has_minimum() {
        assert_eq!(display_height(0), TIMER_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(1), TIMER_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(4), 192);
    }

    #[test]
    fn normalize_settings_forces_fixed_width() {
        let mut settings = TimerSettings::default();
        settings.display.rect.width = 100;
        settings.timers = vec![sample_timer("a", "F2"), sample_timer("b", "F3")];

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.display.rect.width, TIMER_DISPLAY_WIDTH);
        assert_eq!(normalized.display.rect.height, 124);
    }

    #[test]
    fn normalize_settings_rejects_invalid_duration() {
        let mut settings = TimerSettings::default();
        settings.timers[0].duration_seconds = 0;

        let error = normalize_settings(settings).unwrap_err();
        assert!(error.contains("倒计时秒数"));
    }
}
