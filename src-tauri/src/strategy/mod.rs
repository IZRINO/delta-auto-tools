//! 攻略网站代理模块。
//!
//! 暴露一个 Tauri command：`strategy_fetch_page`，
//! 由 Rust 端使用完整 Chrome 浏览器头拉取目标页面并返回 HTML 文本，
//! 前端再用 `<iframe srcDoc>` 渲染（避开 X-Frame-Options / CSP frame-ancestors）。

pub mod fetch;
pub mod types;
