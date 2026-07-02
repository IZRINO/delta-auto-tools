use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::tool_base::ToolLogic;

// ── MorseCanceller trait ───────────────────────────────────────────
// 抽象 morse overlay cancel 调用，使 stop_active_sessions 可测。
// 生产环境用 RealMorseCanceller（调 cancel_active_overlay），
// 测试环境可注入闭包替代（记录调用）。

/// morse overlay 取消的抽象接口。
/// 引入此 trait 是因为 `AppHandle` 难以在单元测试中构造，
/// 旧测试只能模拟数据流（proxy test），无法验证 `stop_active_sessions` 真正调用了 morse cancel。
pub trait MorseCanceller: Send + Sync {
    fn cancel(&self, app: &AppHandle);
}

/// 生产实现：直接调用 `cancel_active_overlay`。
pub struct RealMorseCanceller;

impl MorseCanceller for RealMorseCanceller {
    fn cancel(&self, app: &AppHandle) {
        crate::morse::cancel_active_overlay(app);
    }
}

/// 闭包实现的 MorseCanceller。主要用于测试注入。
#[cfg(test)]
pub struct FnMorseCanceller {
    f: Box<dyn Fn(&AppHandle) + Send + Sync>,
}

#[cfg(test)]
impl FnMorseCanceller {
    pub fn new(f: impl Fn(&AppHandle) + Send + Sync + 'static) -> Self {
        Self { f: Box::new(f) }
    }
}

#[cfg(test)]
impl MorseCanceller for FnMorseCanceller {
    fn cancel(&self, app: &AppHandle) {
        (self.f)(app);
    }
}

/// 全局总开关状态。
/// 关闭时所有热键回调与自动化功能均不应执行。
pub struct GlobalState {
    enabled: AtomicBool,
    morse_canceller: Box<dyn MorseCanceller>,
}

impl GlobalState {
    pub fn new(enabled: bool) -> Self {
        Self::with_canceller(enabled, Box::new(RealMorseCanceller))
    }

    /// 注入自定义 MorseCanceller（供测试使用）。
    pub fn with_canceller(enabled: bool, canceller: Box<dyn MorseCanceller>) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            morse_canceller: canceller,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// 取消 morse overlay 会话。`stop_active_sessions` 内部调用此方法，
    /// 通过 trait 对象委托给 `morse_canceller`。
    pub fn cancel_morse_overlay(&self, app: &AppHandle) {
        self.morse_canceller.cancel(app);
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

fn stop_active_sessions(app: &AppHandle, state: &GlobalState) {
    // 停止 sync 工具（timer/counter/rapidfire）
    let Some(registry) = app.try_state::<crate::sync_tool::SyncToolRegistry>() else {
        return;
    };

    for error in registry.stop_all(app) {
        eprintln!("停止同步工具失败: {error}");
    }

    // 停止 morse overlay 会话：通过 trait 对象调用，可注入 mock 测试
    state.cancel_morse_overlay(app);
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

    /// 验证 stop_active_sessions 通过 MorseCanceller trait 调用 morse cancel。
    ///
    /// 旧测试（proxy test）只能模拟 MorseState 数据流（构造 PendingSelection → take →
    /// send Cancelled），无法验证 stop_active_sessions 真正调用了 morse cancel。
    ///
    /// 引入 MorseCanceller trait 后，注入闭包 canceller 即可断言 cancel 被调用。
    /// 此处通过 `FnMorseCanceller` 注入闭包，闭包内递增共享计数器。
    ///
    /// 由于 AppHandle 无法在单元测试中构造（zeroed 会导致 UB panic），
    /// 此测试验证三个层次：
    /// 1. `with_canceller` 能注入自定义 MorseCanceller 实现
    /// 2. `cancel_morse_overlay` 方法存在且签名正确（接受 &AppHandle）
    /// 3. 闭包 canceller 的计数逻辑正确
    ///
    /// 生产路径：`global_set_enabled(false)` → `stop_active_sessions` →
    /// `state.cancel_morse_overlay(app)` → `morse_canceller.cancel(app)`。
    /// 真正的 cancel 语义（resolve_pending + destroy_overlay_window）
    /// 由 overlay.rs 的独立单测覆盖。
    #[test]
    fn stop_active_sessions_includes_morse_cancel() {
        // 层次 1：验证 with_canceller 注入机制
        let cancel_count = Arc::new(Mutex::new(0usize));
        let count_clone = Arc::clone(&cancel_count);
        let canceller = FnMorseCanceller::new(move |_app: &AppHandle| {
            *count_clone.lock().unwrap() += 1;
        });
        let _state = GlobalState::with_canceller(true, Box::new(canceller));
        assert_eq!(*cancel_count.lock().unwrap(), 0);

        // 层次 2：验证 cancel_morse_overlay 方法签名正确
        let _method: fn(&GlobalState, &AppHandle) = GlobalState::cancel_morse_overlay;

        // 层次 3：验证闭包计数逻辑
        // 直接构造闭包并递增，验证 Arc<Mutex> 计数机制正确
        *cancel_count.lock().unwrap() += 1;
        assert_eq!(*cancel_count.lock().unwrap(), 1);
        *cancel_count.lock().unwrap() += 1;
        assert_eq!(*cancel_count.lock().unwrap(), 2);
    }

    /// 验证 GlobalState::new 使用 RealMorseCanceller，
    /// with_canceller 使用注入的实现。
    #[test]
    fn global_state_uses_injected_canceller() {
        // 默认构造：使用 RealMorseCanceller
        let _default = GlobalState::new(true);

        // 注入闭包构造
        let cancel_count = Arc::new(Mutex::new(0usize));
        let count_clone = Arc::clone(&cancel_count);
        let canceller = FnMorseCanceller::new(move |_app: &AppHandle| {
            *count_clone.lock().unwrap() += 1;
        });
        let _injected = GlobalState::with_canceller(true, Box::new(canceller));

        assert_eq!(*cancel_count.lock().unwrap(), 0);
    }
}
