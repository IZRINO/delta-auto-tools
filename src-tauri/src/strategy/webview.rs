//! 攻略网站 Tauri WebView2 窗口实现。
//!
//! `strategy_open_window` 保留旧的单站 top-level WebviewWindow 入口，供未来实验或兼容调用。
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use url::Url;

use super::types::{StrategyOpenWindowRequest, StrategyOpenWindowResponse};
use crate::app_error::AppError;

/// 默认单站窗口尺寸（足够阅读攻略页内容）。
const DEFAULT_INNER_WIDTH: f64 = 1024.0;
const DEFAULT_INNER_HEIGHT: f64 = 720.0;
/// 最小单站窗口尺寸（防止用户把窗口拖得过小）。
const MIN_INNER_WIDTH: f64 = 480.0;
const MIN_INNER_HEIGHT: f64 = 360.0;

/// 校验并规范化目标 URL。
fn normalize_url(raw: &str) -> Result<Url, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("目标 URL 不能为空".to_string());
    }
    let parsed = Url::parse(trimmed).map_err(|error| format!("目标 URL 解析失败：{error}"))?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        other => Err(format!("目标 URL 协议必须是 http / https，当前是 {other}")),
    }
}

/// 从 host 派生出稳定的窗口 label。
///
/// `kkrb.net` -> `strategy-view-kkrb-net`
/// 字符仅保留 `[a-z0-9-]`，避免 Tauri label 字符限制。
fn derive_view_label(host: &str) -> String {
    let mut sanitized = String::with_capacity(host.len());
    for ch in host.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            sanitized.push(lower);
        } else {
            sanitized.push('-');
        }
    }
    format!("strategy-view-{sanitized}")
}

/// 应用内打开攻略网站：在 Tauri 主进程下新建一个 WebviewWindow 加载外部 URL。
///
/// 同一 host 派生的 label 只维护一个窗口；再次调用会关闭旧窗口并
/// 重新加载（避免堆叠多个同站子窗口）。
#[tauri::command]
pub async fn strategy_open_window(
    app: AppHandle,
    request: StrategyOpenWindowRequest,
) -> Result<StrategyOpenWindowResponse, AppError> {
    let parsed = normalize_url(&request.url)?;

    let host = parsed
        .host_str()
        .ok_or_else(|| "目标 URL 缺少 host".to_string())?
        .to_string();
    let derived_label = derive_view_label(&host);
    let label = request
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or(derived_label);

    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| host.clone());

    // 同一 host 复用窗口：若已存在则关闭并重建，避免堆叠。
    let reused = if let Some(existing) = app.get_webview_window(&label) {
        if let Err(error) = existing.close() {
            return Err(AppError::from(format!("关闭旧窗口失败：{error}")));
        }
        true
    } else {
        false
    };

    let url = WebviewUrl::External(parsed);
    let builder = WebviewWindowBuilder::new(&app, &label, url)
        .title(title)
        .inner_size(DEFAULT_INNER_WIDTH, DEFAULT_INNER_HEIGHT)
        .min_inner_size(MIN_INNER_WIDTH, MIN_INNER_HEIGHT)
        .resizable(true)
        .decorations(true)
        .focused(true)
        .visible(true);

    builder
        .build()
        .map_err(|error| AppError::from(format!("创建应用内浏览器窗口失败：{error}")))?;

    Ok(StrategyOpenWindowResponse { label, reused })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_url_rejects_empty() {
        let result = normalize_url("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("URL 不能为空"));
    }

    #[test]
    fn normalize_url_rejects_non_http() {
        let result = normalize_url("ftp://example.com");
        assert!(result.is_err());
        let message = result.unwrap_err();
        assert!(message.contains("http / https"));
    }

    #[test]
    fn normalize_url_accepts_https() {
        let result = normalize_url("https://example.com/path?q=1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().scheme(), "https");
    }

    #[test]
    fn derive_view_label_normalizes_host() {
        assert_eq!(derive_view_label("kkrb.net"), "strategy-view-kkrb-net");
        assert_eq!(
            derive_view_label("www.kkrb.net"),
            "strategy-view-www-kkrb-net"
        );
        assert_eq!(derive_view_label("orzice.com"), "strategy-view-orzice-com");
    }
}
