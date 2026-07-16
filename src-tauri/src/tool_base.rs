use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Runtime};

/// 工具模块共享逻辑 trait。
///
/// Morse、Timer、Counter、Rapidfire 与 Recognition 共享以下模式：
/// - Settings 运行态
/// - Bootstrap 构建（get_bootstrap 返回的初始状态）
/// - 事件 emit（emit_state）
/// - 运行时锁污染检查（"已损坏"错误）
///
/// 每个模块实现此 trait，将工具特有字段放在 `logic` 结构体中，
/// 共享字段（settings、hotkey_error）放在 `ToolStateInner` 中。
pub trait ToolLogic: Send + 'static {
    type Settings: serde::Serialize
        + for<'de> serde::Deserialize<'de>
        + Default
        + Clone
        + Send
        + 'static;
    type Bootstrap: serde::Serialize + Send + 'static;

    const NAME: &'static str;

    fn build_bootstrap(inner: &ToolStateInner<Self>) -> Self::Bootstrap
    where
        Self: Sized;
    fn emit_state<R: Runtime>(app: &AppHandle<R>, bootstrap: &Self::Bootstrap);
}

/// 工具模块共享状态壳。
///
/// 替代每个模块手写的 `Mutex<InnerState>` 结构，
/// 将 `settings` 和 `hotkey_error` 提取到共享层，
/// 工具特有字段放在 `logic` 中。
pub struct ToolState<T: ToolLogic> {
    pub inner: Arc<Mutex<ToolStateInner<T>>>,
}

/// 工具模块共享内层状态。
///
/// `settings` 和 `hotkey_error` 是所有工具共享的字段；
/// `logic` 包含工具特有字段（如 history、runs、pending_position 等）。
pub struct ToolStateInner<T: ToolLogic> {
    pub logic: T,
    pub settings: T::Settings,
    pub hotkey_error: Option<String>,
}

impl<T: ToolLogic> ToolState<T> {
    pub fn new(logic: T, settings: T::Settings) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ToolStateInner {
                logic,
                settings,
                hotkey_error: None,
            })),
        }
    }

    pub fn lock_inner(&self) -> Result<MutexGuard<'_, ToolStateInner<T>>, String> {
        self.inner
            .lock()
            .map_err(|_| format!("{}状态已损坏", T::NAME))
    }
}

/// 通用 get_bootstrap 命令实现。
///
/// 各模块提供 thin wrapper：
/// ```ignore
/// #[tauri::command]
/// pub fn xxx_get_bootstrap(state: State<'_, ToolState<XxxLogic>>) -> Result<XxxBootstrap, AppError> {
///     tool_base::get_bootstrap(state).map_err(AppError::from)
/// }
/// ```
pub fn get_bootstrap<T: ToolLogic>(
    state: tauri::State<'_, ToolState<T>>,
) -> Result<T::Bootstrap, String> {
    let inner = state.lock_inner()?;
    Ok(T::build_bootstrap(&inner))
}
