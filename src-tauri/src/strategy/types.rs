//! 攻略网站（strategy）模块的请求 / 响应 DTO。
//!
//! 该模块只承载 serde 数据结构与最小校验；网络 / 窗口管理
//! 均位于 `super::webview`。
use serde::{Deserialize, Serialize};

/// 在 Tauri 窗口内打开外部 URL 的请求。
///
/// - `url`：完整目标 URL（含 scheme / host / path）
/// - `title`：窗口标题；缺省时使用 host
/// - `label`：可选手动指定窗口 label，缺省时按 host 自动生成（同一站点复用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyOpenWindowRequest {
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

/// 打开应用内 Webview2 窗口的响应。
///
/// - `label`：新建 / 复用窗口的 label，前端可用来聚焦或定位
/// - `reused`：是否复用了已存在的窗口
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyOpenWindowResponse {
    pub label: String,
    pub reused: bool,
}
