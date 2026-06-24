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
    } else {
        // 重新打开全局开关时重建各工具透明窗口，保持「全部关/全部开」的统一表现形式（Issue #64）。
        restore_active_windows(&app)?;
    }

    Ok(())
}

fn stop_active_sessions(app: &AppHandle) {
    let Some(registry) = app.try_state::<crate::sync_tool::SyncToolRegistry>() else {
        return;
    };

    for error in registry.stop_all(app) {
        eprintln!("停止同步工具失败: {error}");
    }
}

/// 全局开关重新打开时，按各工具自身 `*_enabled` 配置重建透明窗口。
///
/// 与 `stop_active_sessions` 对称：关闭时统一销毁所有 display 窗口，
/// 打开时统一调用 `ensure_display_windows` / `ensure_overlay_window`
/// 重建（工具自身总开关关闭的会继续隐藏，符合预期）。
fn restore_active_windows(app: &AppHandle) -> Result<(), String> {
    use crate::counter;
    use crate::rapidfire;
    use crate::timer;

    if let Some(counter_state) = app.try_state::<counter::CounterState>() {
        let inner = counter_state.lock_inner()?;
        counter::ensure_display_windows(app, &inner.settings)?;
    }
    if let Some(timer_state) = app.try_state::<timer::TimerState>() {
        let inner = timer_state.lock_inner()?;
        timer::ensure_display_windows(app, &inner.settings)?;
    }
    if let Some(rapidfire_state) = app.try_state::<rapidfire::RapidfireState>() {
        let inner = rapidfire_state.lock_inner()?;
        rapidfire::ensure_overlay_window(app, &inner.settings)?;
    }
    Ok(())
}
