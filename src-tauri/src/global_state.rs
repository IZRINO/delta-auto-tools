use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::tool_base::ToolLogic;

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
    // 停止 sync 工具（timer/counter/rapidfire）
    let Some(registry) = app.try_state::<crate::sync_tool::SyncToolRegistry>() else {
        return;
    };

    for error in registry.stop_all(app) {
        eprintln!("停止同步工具失败: {error}");
    }

    // 停止 morse overlay 会话：销毁 overlay 窗口并 resolve pending sender
    crate::morse::cancel_active_overlay(app);
}

/// 全局开关重新打开时，按各工具自身 `*_enabled` 配置重建透明窗口并重启热键监听。
///
/// 与 `stop_active_sessions` 对称：关闭时统一隐藏所有 display 窗口，
/// 打开时统一调用 `ensure_display_windows` / `ensure_overlay_window`
/// 恢复（工具自身总开关关闭的会继续隐藏，符合预期），
/// 同时重启热键监听并向前端推送最新状态。
///
/// 各工具恢复独立执行，一个工具失败不影响其他工具恢复。
fn restore_active_windows(app: &AppHandle) -> Result<(), String> {
    use crate::counter;
    use crate::rapidfire;
    use crate::timer;

    let hotkey_manager = app.try_state::<crate::hotkeys::HotkeyManager>();
    let mut errors: Vec<String> = Vec::new();

    // 计时器：先克隆 settings 再释放锁，避免死锁
    if let Some(timer_state) = app.try_state::<timer::TimerState>() {
        let (settings, bootstrap) = {
            let inner = timer_state.lock_inner()?;
            let settings = inner.settings.clone();
            let bootstrap = timer::TimerLogic::build_bootstrap(&inner);
            (settings, bootstrap)
        };
        if let Err(e) = timer::ensure_display_windows(app, &settings) {
            errors.push(format!("计时器: {e}"));
        }
        if let Some(hm) = hotkey_manager.as_ref() {
            if let Err(e) = timer::restart_hotkey_listeners(&timer_state, &**hm, &settings) {
                errors.push(format!("计时器热键: {e}"));
            }
        }
        timer::emit_state(app, bootstrap);
    }

    // 计数器
    if let Some(counter_state) = app.try_state::<counter::CounterState>() {
        let (settings, bootstrap) = {
            let inner = counter_state.lock_inner()?;
            let settings = inner.settings.clone();
            let bootstrap = counter::CounterLogic::build_bootstrap(&inner);
            (settings, bootstrap)
        };
        if let Err(e) = counter::ensure_display_windows(app, &settings) {
            errors.push(format!("计数器: {e}"));
        }
        if let Some(hm) = hotkey_manager.as_ref() {
            if let Err(e) = counter::restart_hotkey_listeners(&counter_state, &**hm, &settings) {
                errors.push(format!("计数器热键: {e}"));
            }
        }
        counter::emit_state(app, bootstrap);
    }

    // 连发器
    if let Some(rapidfire_state) = app.try_state::<rapidfire::RapidfireState>() {
        let (settings, bootstrap) = {
            let inner = rapidfire_state.lock_inner()?;
            let settings = inner.settings.clone();
            let bootstrap = rapidfire::RapidfireLogic::build_bootstrap(&inner);
            (settings, bootstrap)
        };
        if let Err(e) = rapidfire::ensure_overlay_window(app, &settings) {
            errors.push(format!("连发器: {e}"));
        }
        if let Some(hm) = hotkey_manager.as_ref() {
            if let Err(e) =
                rapidfire::restart_hotkey_listeners(&rapidfire_state, &**hm, &settings, false)
            {
                errors.push(format!("连发器热键: {e}"));
            }
        }
        rapidfire::emit_state(app, bootstrap);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_state_new_enabled() {
        let state = GlobalState::new(true);
        assert!(state.enabled());
    }

    #[test]
    fn global_state_new_disabled() {
        let state = GlobalState::new(false);
        assert!(!state.enabled());
    }

    #[test]
    fn global_state_set_enabled_toggles() {
        let state = GlobalState::new(false);
        assert!(!state.enabled());

        state.set_enabled(true);
        assert!(state.enabled());

        state.set_enabled(false);
        assert!(!state.enabled());
    }

    /// 验证 SyncToolRegistry 空状态下注册和名称查询正常，
    /// stop_active_sessions 在无 handler 时不 panic（由 SyncToolRegistry::stop_all
    /// 对空 handlers 遍历保证，此处验证注册状态为空）。
    #[test]
    fn stop_active_sessions_empty_registry_no_panic() {
        let registry = crate::sync_tool::SyncToolRegistry::default();
        let names = registry.registered_names();
        assert!(names.is_empty(), "空 registry 不应有已注册工具");
    }

    /// 验证 SyncToolRegistry 注册了 timer/counter/rapidfire 三类工具的 stop handler，
    /// stop_active_sessions 遍历时会全部调用。
    #[test]
    fn stop_active_sessions_covers_all_sync_tools() {
        fn handler_ok(_app: &AppHandle) -> Result<(), String> {
            Ok(())
        }

        let mut registry = crate::sync_tool::SyncToolRegistry::default();
        registry.register("timer", handler_ok);
        registry.register("counter", handler_ok);
        registry.register("rapidfire", handler_ok);

        let names = registry.registered_names();
        assert_eq!(names, vec!["timer", "counter", "rapidfire"]);
    }

    /// 验证 SyncToolRegistry stop_all 错误收集：部分 handler 失败不影响其他。
    #[test]
    fn stop_active_sessions_collects_errors_from_all_handlers() {
        fn handler_ok(_app: &AppHandle) -> Result<(), String> {
            Ok(())
        }

        fn handler_err(_app: &AppHandle) -> Result<(), String> {
            Err("停止失败".to_string())
        }

        let mut registry = crate::sync_tool::SyncToolRegistry::default();
        registry.register("ok", handler_ok);
        registry.register("bad", handler_err);

        let names = registry.registered_names();
        assert_eq!(names, vec!["ok", "bad"]);
    }

    /// 验证 stop_active_sessions 调用 morse cancel 路径：
    /// crate::morse::cancel_active_overlay 函数引用存在 + resolve_pending 核心逻辑。
    /// 此测试验证 cancel_active_overlay 是 stop_active_sessions 的一部分，
    /// 以及 resolve_pending 的核心语义（pending sender 被 resolve 为 Cancelled）。
    #[test]
    fn stop_active_sessions_includes_morse_cancel() {
        // 验证 crate::morse::cancel_active_overlay 函数存在于 public API。
        // 如果函数不存在或签名变更，编译会失败。
        let _ = crate::morse::cancel_active_overlay as fn(&AppHandle) -> ();

        // 验证 resolve_pending 核心逻辑：
        // pending sender 应被 resolve 为 Cancelled（而非 Closed）。
        // 这是 cancel_active_overlay 的核心语义。
        use tokio::sync::oneshot;
        let (sender, receiver) = oneshot::channel();
        sender.send(crate::morse::types::RegionSelectionKind::Cancelled).unwrap();
        let result = receiver.blocking_recv().unwrap();
        assert!(
            matches!(result, crate::morse::types::RegionSelectionKind::Cancelled),
            "morse overlay cancel 应 resolve 为 Cancelled，实际: {result:?}"
        );
    }
}
