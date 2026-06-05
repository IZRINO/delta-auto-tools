//! 攻略网站 Tauri WebView2 窗口实现。
//!
//! `strategy_open_browser` 打开固定攻略浏览器 shell，shell 内由前端创建外部 URL 子 WebView；
//! `strategy_open_window` 保留旧的单站 top-level WebviewWindow 入口，供未来实验或兼容调用。
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use url::Url;

use super::types::{
    StrategyOpenBrowserRequest, StrategyOpenBrowserResponse, StrategyOpenWindowRequest,
    StrategyOpenWindowResponse,
};

/// 默认单站窗口尺寸（足够阅读攻略页内容）。
const DEFAULT_INNER_WIDTH: f64 = 1024.0;
const DEFAULT_INNER_HEIGHT: f64 = 720.0;
/// 最小单站窗口尺寸（防止用户把窗口拖得过小）。
const MIN_INNER_WIDTH: f64 = 480.0;
const MIN_INNER_HEIGHT: f64 = 360.0;

/// 固定攻略浏览器窗口 label；窗口内再由前端创建真实站点子 WebView。
const STRATEGY_BROWSER_LABEL: &str = "strategy-browser";
const BROWSER_INNER_WIDTH: f64 = 1180.0;
const BROWSER_INNER_HEIGHT: f64 = 780.0;
const BROWSER_MIN_INNER_WIDTH: f64 = 640.0;
const BROWSER_MIN_INNER_HEIGHT: f64 = 480.0;

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

fn encode_query_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// 打开固定攻略浏览器窗口。
///
/// 该窗口加载本应用的 `?mode=strategy-browser` shell；真实攻略站点由 shell 内部的
/// `@tauri-apps/api/webview::Webview` 子 WebView 直接导航加载。
#[tauri::command]
pub async fn strategy_open_browser(
    app: AppHandle,
    request: StrategyOpenBrowserRequest,
) -> Result<StrategyOpenBrowserResponse, String> {
    let parsed = normalize_url(&request.url)?;
    let site_id = request.site_id.trim();
    if site_id.is_empty() {
        return Err("攻略站点 ID 不能为空".to_string());
    }

    let reused = if let Some(existing) = app.get_webview_window(STRATEGY_BROWSER_LABEL) {
        existing
            .close()
            .map_err(|error| format!("关闭旧攻略浏览器窗口失败：{error}"))?;
        true
    } else {
        false
    };

    let url = format!(
        "index.html?mode=strategy-browser&site={}&url={}",
        encode_query_component(site_id),
        encode_query_component(parsed.as_str())
    );
    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("攻略浏览器 - {value}"))
        .unwrap_or_else(|| "攻略浏览器".to_string());

    WebviewWindowBuilder::new(&app, STRATEGY_BROWSER_LABEL, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(BROWSER_INNER_WIDTH, BROWSER_INNER_HEIGHT)
        .min_inner_size(BROWSER_MIN_INNER_WIDTH, BROWSER_MIN_INNER_HEIGHT)
        .resizable(true)
        .decorations(true)
        .focused(true)
        .visible(true)
        .build()
        .map_err(|error| format!("创建攻略浏览器窗口失败：{error}"))?;

    Ok(StrategyOpenBrowserResponse {
        label: STRATEGY_BROWSER_LABEL.to_string(),
        reused,
    })
}

/// 应用内打开攻略网站：在 Tauri 主进程下新建一个 WebviewWindow 加载外部 URL。
///
/// 同一 host 派生的 label 只维护一个窗口；再次调用会关闭旧窗口并
/// 重新加载（避免堆叠多个同站子窗口）。
#[tauri::command]
pub async fn strategy_open_window(
    app: AppHandle,
    request: StrategyOpenWindowRequest,
) -> Result<StrategyOpenWindowResponse, String> {
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
            return Err(format!("关闭旧窗口失败：{error}"));
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
        .map_err(|error| format!("创建应用内浏览器窗口失败：{error}"))?;

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
        assert_eq!(derive_view_label("www.kkrb.net"), "strategy-view-www-kkrb-net");
        assert_eq!(derive_view_label("orzice.com"), "strategy-view-orzice-com");
    }

    #[test]
    fn encode_query_component_escapes_url() {
        assert_eq!(
            encode_query_component("https://www.kkrb.net/?viewpage=view%2Foverview"),
            "https%3A%2F%2Fwww.kkrb.net%2F%3Fviewpage%3Dview%252Foverview"
        );
    }
}
