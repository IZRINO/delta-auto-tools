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
/// - `challenge`：若响应体被检测为人机验证页（典型：kkrb cdn-shield），
///   此字段给出 `kind` + 文案，前端据此把"应用内打开"按钮升到主操作位
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyFetchResponse {
    pub status: u16,
    pub final_url: String,
    pub content_type: String,
    pub html: String,
    pub byte_length: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub challenge: Option<StrategyChallenge>,
}

/// 代理层检测到的人机验证挑战描述。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyChallenge {
    /// 挑战类型，目前固定为 `ccCheck`（kkrb / cdn-shield 风格）。
    pub kind: String,
    /// 给前端 Alert 用的中文提示。
    pub message: String,
}

/// 在 Tauri 窗口内打开外部 URL 的请求。
///
/// - `url`：完整目标 URL（含 scheme / host / path）
/// - `title`：窗口标题；缺省时使用 host
/// - `label`：可选手动指定窗口 label，缺省时按 host 自动生成（同一站点复用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyOpenInViewRequest {
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

/// 打开应用内浏览器视图的响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyOpenInViewResponse {
    /// 新建 / 复用窗口的 label，前端可用来聚焦或定位。
    pub label: String,
    /// 是否复用了已存在的窗口。
    pub reused: bool,
}
