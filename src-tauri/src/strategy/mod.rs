//! 攻略网站 Tauri 模块。
//!
//! 暴露一个 Tauri command：`strategy_open_window`，
//! 在 Tauri 主进程下新建一个 WebviewWindow 加载外部 URL（top-level navigation），
//! 由真正的 WebView2 Chromium 直接渲染目标站点本身，不走任何代理。
pub mod fetch;
pub mod types;
pub mod webview;
