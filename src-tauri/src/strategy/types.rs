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

/// 打开固定攻略浏览器窗口的请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyOpenBrowserRequest {
    pub site_id: String,
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
}

/// 打开固定攻略浏览器窗口的响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyOpenBrowserResponse {
    pub label: String,
    pub reused: bool,
}

/// `strategy_fetch_page` 命令的响应。
///
/// - `html`：最终页面的 HTML 内容
/// - `final_url`：最终请求的 URL（经过 HTTP / JS 重定向后）
/// - `challenge`：如果命中 CC check 验证页面，返回 challenge 信息；否则为 None
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyFetchResponse {
    pub html: String,
    pub final_url: String,
    pub challenge: Option<ChallengeInfo>,
}

/// CC check 验证信息。
///
/// - `kind`：验证类型，当前固定为 `"ccCheck"`
/// - `message`：人类可读的验证提示
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeInfo {
    pub kind: String,
    pub message: String,
}