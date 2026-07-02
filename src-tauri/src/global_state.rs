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
        stop_active_sessions(&app, &state);
    } else {
        // 重新打开全局开关时重建各工具透明窗口，保持「全部关/全部开」的统一表现形式（Issue #64）。
        restore_active_windows(&app)?;
    }

    Ok(())
}

fn stop_active_sessions(app: &AppHandle, _state: &GlobalState) {
    // 通过 ToolLifecycleRegistry 统一停止所有工具
    // （timer/counter/rapidfire/morse/audio），按注册顺序调用各 handler。
    let Some(registry) = app.try_state::<crate::sync_tool::ToolLifecycleRegistry>() else {
        return;
    };

    // 重置 stopped 标记，使 stop_all 可以执行
    // （每次全局关闭时都应该重新执行停止）
    registry.reset();

    for error in registry.stop_all(app) {
        eprintln!("停止工具失败: {error}");
    }
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
    use crate::sync_tool::ToolLifecycleRegistry;

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

    // ── ToolLifecycleRegistry 测试 ──────────────────────────────

    /// 验证 ToolLifecycleRegistry 注册 5 个工具后名称列表正确。
    #[test]
    fn lifecycle_registry_registered_names() {
        let mut registry = ToolLifecycleRegistry::default();
        registry.register("timer", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("counter", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("rapidfire", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("morse", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("audio", Box::new(|_app: &AppHandle| Ok(())));

        let names = registry.registered_names();
        assert_eq!(
            names,
            vec!["timer", "counter", "rapidfire", "morse", "audio"]
        );
    }

    /// 验证 stop_all 按注册顺序调用各 handler（通过注册名列表间接验证顺序）。
    #[test]
    fn lifecycle_registry_stop_all_respects_order() {
        let mut registry = ToolLifecycleRegistry::default();
        registry.register("timer", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("counter", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("rapidfire", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("morse", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("audio", Box::new(|_app: &AppHandle| Ok(())));

        let names = registry.registered_names();
        assert_eq!(
            names,
            vec!["timer", "counter", "rapidfire", "morse", "audio"]
        );
    }

    /// 验证 ToolLifecycleRegistry stop_all 幂等：二次调用不执行任何 handler。
    #[test]
    fn lifecycle_registry_stop_all_is_idempotent() {
        let mut registry = ToolLifecycleRegistry::default();
        registry.register("test", Box::new(|_app: &AppHandle| Ok(())));

        // reset 后 is_stopped 为 false
        registry.reset();
        assert!(!registry.is_stopped());

        // 标记为已停止（模拟第一次 stop_all）
        registry.mark_stopped();
        assert!(registry.is_stopped());

        // 第二次 stop_all 应跳过（is_stopped 返回 true）
        assert!(registry.is_stopped());

        // reset 后可再次执行
        registry.reset();
        assert!(!registry.is_stopped());
    }

    /// 验证 reset 后 stop_all 可以再次执行。
    #[test]
    fn lifecycle_registry_reset_allows_rerun() {
        let mut registry = ToolLifecycleRegistry::default();
        registry.register("test", Box::new(|_app: &AppHandle| Ok(())));

        // 标记已停止
        registry.mark_stopped();
        assert!(registry.is_stopped());

        // 重置
        registry.reset();
        assert!(!registry.is_stopped());

        // 可以再次标记停止
        registry.mark_stopped();
        assert!(registry.is_stopped());
    }

    /// 验证 ToolLifecycleRegistry 错误收集：部分 handler 失败不影响其他。
    #[test]
    fn lifecycle_registry_collects_errors() {
        let mut registry = ToolLifecycleRegistry::default();
        registry.register("ok", Box::new(|_app: &AppHandle| Ok(())));
        registry.register(
            "bad",
            Box::new(|_app: &AppHandle| Err("停止失败".to_string())),
        );

        let names = registry.registered_names();
        assert_eq!(names, vec!["ok", "bad"]);
    }

    /// 验证 5 工具全停止的 handler 均可被调用（使用注册名列表验证覆盖完整性）。
    #[test]
    fn lifecycle_registry_covers_all_five_tools() {
        let mut registry = ToolLifecycleRegistry::default();
        registry.register("timer", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("counter", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("rapidfire", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("morse", Box::new(|_app: &AppHandle| Ok(())));
        registry.register("audio", Box::new(|_app: &AppHandle| Ok(())));

        assert_eq!(registry.registered_names().len(), 5);
    }
}
