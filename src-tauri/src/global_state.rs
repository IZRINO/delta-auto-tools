use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, State};

/// 全局总开关状态。
/// 关闭时所有热键回调与自动化功能均不应执行。
pub struct GlobalState {
    enabled: AtomicBool,
}

impl GlobalState {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

const ENABLED_CHANGED: &str = "global://enabled-changed";

#[tauri::command]
pub fn global_get_enabled(state: State<'_, GlobalState>) -> bool {
    state.enabled()
}

#[tauri::command]
pub fn global_set_enabled(
    app: AppHandle,
    state: State<'_, GlobalState>,
    enabled: bool,
) -> Result<(), String> {
    state.set_enabled(enabled);
    let _ = app.emit_to("main", ENABLED_CHANGED, enabled);

    if !enabled {
        // 关闭全局开关时立即停止连发器与计时器的运行态会话
        stop_active_sessions(&app);
    }

    Ok(())
}

fn stop_active_sessions(app: &AppHandle) {
    use crate::hotkeys::HotkeyManager;
    use crate::rapidfire;
    use crate::timer;

    let hotkey_manager = app.try_state::<HotkeyManager>();
    if let Some(rapidfire_state) = app.try_state::<rapidfire::RapidfireState>() {
        rapidfire::stop_all(app, &rapidfire_state, hotkey_manager.as_ref().map(|v| &**v));
    }
    if let Some(timer_state) = app.try_state::<timer::TimerState>() {
        timer::stop_all(app, &timer_state);
    }
}
