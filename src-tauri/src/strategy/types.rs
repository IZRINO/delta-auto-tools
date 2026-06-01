//! 攻略网站代理请求 / 响应的 DTO。
//!
//! 该模块只承载 serde 数据结构与最小校验，不做网络调用。

use serde::{Deserialize, Serialize};

/// 攻略网站代理请求。
///
/// - `url`：完整目标 URL（含 scheme / host / path）
/// - `referer`：可选，模拟浏览器同源 referer 头；缺省时使用 `url` 自身
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyFetchRequest {
    pub url: String,
    #[serde(default)]
    pub referer: Option<String>,
}

/// 攻略网站代理响应。
///
/// - `status`：HTTP 状态码
/// - `final_url`：跳转后的最终 URL（用于 `srcDoc` 注入 `<base href>`）
/// - `content_type`：原始响应 Content-Type
/// - `html`：响应正文（HTML 文本；其他类型以 UTF-8 强转）
/// - `byte_length`：原始字节数（用于 UI 展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyFetchResponse {
    pub status: u16,
    pub final_url: String,
    pub content_type: String,
    pub html: String,
    pub byte_length: usize,
}
