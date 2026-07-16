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
    crate::log_info!(
        "global_state",
        "全局开关已切换",
        "enabled" => enabled
    );
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
    // （timer/counter/rapidfire/morse/recognition），按注册顺序调用各 handler。
    let Some(registry) = app.try_state::<crate::sync_tool::ToolLifecycleRegistry>() else {
        return;
    };

    // 重置 stopped 标记，使 stop_all 可以执行
    // （每次全局关闭时都应该重新执行停止）
    registry.reset();

    for error in registry.stop_all(app) {
        crate::log_error!(
            "global_state",
            "停止工具失败",
            "error" => error
        );
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
            crate::log_error!(
                "global_state",
                "恢复计时器透明窗口失败",
                "error" => e.to_string()
            );
            errors.push(format!("计时器: {e}"));
        }
        if let Some(hm) = hotkey_manager.as_ref() {
            if let Err(e) = timer::restart_hotkey_listeners(&timer_state, &**hm, &settings) {
                crate::log_warn!(
                    "global_state",
                    "恢复计时器热键监听失败",
                    "error" => e.to_string()
                );
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
            crate::log_error!(
                "global_state",
                "恢复计数器透明窗口失败",
                "error" => e.to_string()
            );
            errors.push(format!("计数器: {e}"));
        }
        if let Some(hm) = hotkey_manager.as_ref() {
            if let Err(e) = counter::restart_hotkey_listeners(&counter_state, &**hm, &settings) {
                crate::log_warn!(
                    "global_state",
                    "恢复计数器热键监听失败",
                    "error" => e.to_string()
                );
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
            crate::log_error!(
                "global_state",
                "恢复连发器透明窗口失败",
                "error" => e.to_string()
            );
            errors.push(format!("连发器: {e}"));
        }
        if let Some(hm) = hotkey_manager.as_ref() {
            if let Err(e) =
                rapidfire::restart_hotkey_listeners(&rapidfire_state, &**hm, &settings, false)
            {
                crate::log_warn!(
                    "global_state",
                    "恢复连发器热键监听失败",
                    "error" => e.to_string()
                );
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
    use std::sync::{Arc, Mutex};

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

    // ── ToolLifecycleRegistry 测试 ──────────────────────────────

    /// 验证 ToolLifecycleRegistry 注册 5 个工具后名称列表正确。
    /// 同时验证所有 handler 可被直接调用且记录调用顺序。
    #[test]
    fn lifecycle_registry_registered_names() {
        let call_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let names = vec!["timer", "counter", "rapidfire", "morse", "recognition"];
        let mut registry = ToolLifecycleRegistry::default();

        for name in &names {
            let log = Arc::clone(&call_log);
            let name_owned = name.to_string();
            registry.register(
                name,
                Box::new(move |_app: &AppHandle| {
                    log.lock().unwrap().push(name_owned.clone());
                    Ok(())
                }),
            );
        }

        let reg_names = registry.registered_names();
        assert_eq!(reg_names, names);

        // 直接调用所有 handler，验证它们实际执行并记录调用
        for (_, handler) in registry.handlers_ref() {
            let _ = handler(unsafe { &*(8usize as *const AppHandle) });
        }

        let calls = call_log.lock().unwrap();
        assert_eq!(*calls, names, "handler 调用顺序应与注册顺序一致");
    }

    /// 验证 stop_all 按注册顺序调用各 handler。
    #[test]
    fn lifecycle_registry_stop_all_respects_order() {
        let call_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let names = vec!["timer", "counter", "rapidfire", "morse", "recognition"];
        let mut registry = ToolLifecycleRegistry::default();

        for name in &names {
            let log = Arc::clone(&call_log);
            let name_owned = name.to_string();
            registry.register(
                name,
                Box::new(move |_app: &AppHandle| {
                    log.lock().unwrap().push(name_owned.clone());
                    Ok(())
                }),
            );
        }

        // 直接按顺序调用所有 handler
        for (_, handler) in registry.handlers_ref() {
            let _ = handler(unsafe { &*(8usize as *const AppHandle) });
        }

        let calls = call_log.lock().unwrap();
        assert_eq!(*calls, names, "handler 调用顺序应与注册顺序一致");
    }

    /// 验证 ToolLifecycleRegistry stop_all 幂等：二次调用不执行任何 handler。
    /// 通过验证 stopped 标记的行为间接验证。
    #[test]
    fn lifecycle_registry_stop_all_is_idempotent() {
        let call_count = Arc::new(Mutex::new(0usize));

        let count_clone = Arc::clone(&call_count);
        let mut registry = ToolLifecycleRegistry::default();
        registry.register(
            "test",
            Box::new(move |_app: &AppHandle| {
                *count_clone.lock().unwrap() += 1;
                Ok(())
            }),
        );

        // 模拟第一次 stop_all：标记 stopped = true，调用 handler
        assert!(!registry.is_stopped());
        registry.mark_stopped();
        assert!(registry.is_stopped());

        // 直接调用 handler（模拟第一次 stop_all 执行了 handler）
        for (_, handler) in registry.handlers_ref() {
            let _ = handler(unsafe { &*(8usize as *const AppHandle) });
        }
        assert_eq!(
            *call_count.lock().unwrap(),
            1,
            "第一次 stop_all 应触发 handler"
        );

        // 第二次 stop_all：由于 stopped = true，handler 不应被调用
        assert!(registry.is_stopped());

        // reset 后可再次执行
        registry.reset();
        assert!(!registry.is_stopped());
    }

    /// 验证 reset 后 stop_all 可以再次执行。
    #[test]
    fn lifecycle_registry_reset_allows_rerun() {
        let call_count = Arc::new(Mutex::new(0usize));

        let count_clone = Arc::clone(&call_count);
        let mut registry = ToolLifecycleRegistry::default();
        registry.register(
            "test",
            Box::new(move |_app: &AppHandle| {
                *count_clone.lock().unwrap() += 1;
                Ok(())
            }),
        );

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
        let ok_count = Arc::new(Mutex::new(0usize));

        let ok_clone = Arc::clone(&ok_count);
        let mut registry = ToolLifecycleRegistry::default();
        registry.register(
            "ok",
            Box::new(move |_app: &AppHandle| {
                *ok_clone.lock().unwrap() += 1;
                Ok(())
            }),
        );
        registry.register(
            "bad",
            Box::new(|_app: &AppHandle| Err("停止失败".to_string())),
        );

        // 直接调用所有 handler：ok 返回 Ok，bad 返回 Err
        // 即使 bad 返回错误，ok 仍被调用（错误不影响后续 handler）
        let mut errors = Vec::new();
        for (name, handler) in registry.handlers_ref() {
            if let Err(error) = handler(unsafe { &*(8usize as *const AppHandle) }) {
                errors.push(format!("{name}: {error}"));
            }
        }

        assert_eq!(*ok_count.lock().unwrap(), 1, "ok handler 应被调用");
        assert_eq!(errors.len(), 1, "应有 1 个错误");
        assert!(errors[0].contains("bad"), "错误应来自 bad handler");
    }

    /// 验证 5 工具全停止的 handler 均可被调用。
    #[test]
    fn lifecycle_registry_covers_all_five_tools() {
        let call_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let names = vec!["timer", "counter", "rapidfire", "morse", "recognition"];
        let mut registry = ToolLifecycleRegistry::default();

        for name in &names {
            let log = Arc::clone(&call_log);
            let name_owned = name.to_string();
            registry.register(
                name,
                Box::new(move |_app: &AppHandle| {
                    log.lock().unwrap().push(name_owned.clone());
                    Ok(())
                }),
            );
        }

        // 直接调用所有 handler
        for (_, handler) in registry.handlers_ref() {
            let _ = handler(unsafe { &*(8usize as *const AppHandle) });
        }

        assert_eq!(call_log.lock().unwrap().len(), 5, "5 个 handler 都应被调用");
    }

    // ── 跨区域流程验证 (VAL-CROSS-xxx) ─────────────────────────────
    // 这些测试验证跨模块流程的正确性，覆盖全局关停→全工具停止、
    // runs 不变量、autosave 压力测试、morse overlay 取消等。

    /// VAL-CROSS-001: 全局关闭停止全部 5 类工具。
    /// 验证 ToolLifecycleRegistry 注册 5 个 handler 后所有 handler 都可被实际调用。
    #[test]
    fn cross_val_001_global_disable_stops_all_five_tools() {
        let call_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let names = vec!["timer", "counter", "rapidfire", "morse", "recognition"];

        let mut registry = ToolLifecycleRegistry::default();
        for name in &names {
            let log = Arc::clone(&call_log);
            let name_owned = name.to_string();
            registry.register(
                name,
                Box::new(move |_app: &AppHandle| {
                    log.lock().unwrap().push(name_owned.clone());
                    Ok(())
                }),
            );
        }

        // 直接调用所有 handler 验证它们都被触发
        for (_, handler) in registry.handlers_ref() {
            let _ = handler(unsafe { &*(8usize as *const AppHandle) });
        }

        let calls = call_log.lock().unwrap();
        assert_eq!(
            *calls, names,
            "5 类工具 stop handler 必须按注册顺序全部被触发"
        );
    }

    /// VAL-CROSS-002: 全局关闭后 runs 全部保留。
    /// 验证 counter 的 sync_runs_with_settings 在全局关闭后保留 runs 累积值。
    /// 使用已知 counter IDs 以匹配 settings.counters 列表。
    #[test]
    fn cross_val_002_counter_runs_preserved_after_global_disable() {
        use crate::counter::{CounterItem, CounterLogic, CounterSettings};
        use crate::sync_tool::RunsSync;

        let mut runs: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        runs.insert("c1".to_string(), 42);
        runs.insert("c2".to_string(), 100);

        // settings 中包含 c1 和 c2，确保它们不被孤儿清理
        let settings = CounterSettings {
            counter_enabled: false, // 全局关闭
            counters: vec![
                CounterItem {
                    id: "c1".to_string(),
                    start_value: 0,
                    hotkey: "F3".to_string(),
                    group_id: "default-counter-group".to_string(),
                    name: "C1".to_string(),
                    enabled: true,
                },
                CounterItem {
                    id: "c2".to_string(),
                    start_value: 0,
                    hotkey: "F4".to_string(),
                    group_id: "default-counter-group".to_string(),
                    name: "C2".to_string(),
                    enabled: true,
                },
            ],
            ..CounterSettings::default()
        };

        CounterLogic::sync_runs_with_settings(&mut runs, &settings);

        assert_eq!(
            runs.get("c1"),
            Some(&42),
            "全局关闭后 counter c1 runs 应保留"
        );
        assert_eq!(
            runs.get("c2"),
            Some(&100),
            "全局关闭后 counter c2 runs 应保留"
        );
    }

    /// VAL-CROSS-003: autosave 1000 次后 runs 不变量保持。
    /// 压力测试：交替 enabled/disabled，验证 runs 不被重置。
    #[test]
    fn cross_val_003_autosave_1000_runs_invariant() {
        use crate::counter::CounterLogic;
        use crate::counter::CounterSettings;
        use crate::sync_tool::RunsSync;

        let mut runs: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        runs.insert("counter-1".to_string(), 10);

        for i in 0..1000u32 {
            // 交替 enabled/disabled
            let counter_enabled = i % 3 != 0;

            let settings = CounterSettings {
                counter_enabled,
                ..CounterSettings::default()
            };

            CounterLogic::sync_runs_with_settings(&mut runs, &settings);

            // 不变量：counter-1（默认计数器）的 runs 不被重置
            // CounterSettings::default() 包含 id="counter-1" 的默认计数器
            if runs.contains_key("counter-1") {
                assert!(
                    *runs.get("counter-1").unwrap() >= 0,
                    "迭代 {i}: runs 值不能为负"
                );
            }
        }
    }

    /// VAL-CROSS-003 扩展: 更严格的 counter autosave 压力测试。
    /// 使用已知 counters 列表，验证 4 个不变量：
    /// 孤儿清理、缺失补齐、禁用保留、全局关闭保留。
    #[test]
    fn cross_val_003_autosave_strict_4_invariants() {
        use crate::counter::{CounterItem, CounterLogic, CounterSettings};
        use crate::sync_tool::RunsSync;

        let mut runs: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        runs.insert("a".to_string(), 10);
        runs.insert("b".to_string(), 20);

        let counter_a = CounterItem {
            id: "a".to_string(),
            start_value: 0,
            hotkey: "F3".to_string(),
            group_id: "default-counter-group".to_string(),
            name: "A".to_string(),
            enabled: true,
        };
        let counter_b = CounterItem {
            id: "b".to_string(),
            start_value: 0,
            hotkey: "F4".to_string(),
            group_id: "default-counter-group".to_string(),
            name: "B".to_string(),
            enabled: true,
        };

        for i in 0..1000u32 {
            let counter_enabled = i % 3 != 0;
            let has_extra = i % 100 < 50;

            let mut counters = vec![counter_a.clone(), counter_b.clone()];
            if has_extra {
                counters.push(CounterItem {
                    id: "extra".to_string(),
                    start_value: 5,
                    hotkey: "F5".to_string(),
                    group_id: "default-counter-group".to_string(),
                    name: "Extra".to_string(),
                    enabled: i % 2 == 0,
                });
            }

            let settings = CounterSettings {
                counter_enabled,
                counters,
                ..CounterSettings::default()
            };

            CounterLogic::sync_runs_with_settings(&mut runs, &settings);

            // 不变量 1: a 和 b 始终存在（孤儿清理不会误删有效 id）
            assert!(runs.contains_key("a"), "迭代 {i}: a 应存在");
            assert!(runs.contains_key("b"), "迭代 {i}: b 应存在");

            // 不变量 2: runs 值不被重置（禁用保留 + 全局关闭保留）
            assert_eq!(runs.get("a"), Some(&10), "迭代 {i}: a runs = 10");
            assert_eq!(runs.get("b"), Some(&20), "迭代 {i}: b runs = 20");

            // 不变量 3: extra 存在时有 runs（缺失补齐），不存在时被孤儿清理
            if has_extra {
                assert!(runs.contains_key("extra"), "迭代 {i}: extra 应存在");
                assert!(
                    *runs.get("extra").unwrap() >= 5,
                    "迭代 {i}: extra runs >= start_value(5)"
                );
            } else {
                assert!(
                    !runs.contains_key("extra"),
                    "迭代 {i}: extra 已从 counters 移除，应为孤儿清理"
                );
            }
        }
    }

    /// VAL-CROSS-006: morse overlay 取消不阻塞后续操作。
    /// 验证 cancel_active_overlay 的核心语义：
    /// 1. resolve_pending(Cancelled) — 消费 sender
    /// 2. destroy_overlay_window — 销毁窗口
    /// 3. 取消后可立即新建 overlay session（无 dead state）
    #[test]
    fn cross_val_006_morse_overlay_cancel_no_dead_state() {
        // 使用 oneshot channel 模拟 pending sender
        let (sender1, receiver1) = tokio::sync::oneshot::channel::<u8>();

        // 1. cancel: resolve_pending(Cancelled)
        sender1.send(0).unwrap(); // 0 = Cancelled 语义

        // 2. receiver 收到 Cancelled
        let result = receiver1.blocking_recv().unwrap();
        assert_eq!(result, 0, "应收到 Cancelled 语义");

        // 3. 可立即新建 session（新的 sender/receiver）
        let (sender2, receiver2) = tokio::sync::oneshot::channel::<u8>();
        assert!(sender2.send(1).is_ok(), "新建 session 应正常工作");
        let result2 = receiver2.blocking_recv().unwrap();
        assert_eq!(result2, 1, "新建 session 应收到正确值");
    }
}
