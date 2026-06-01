//! 攻略网站代理抓取实现。
//!
//! 通过 reqwest 拉取目标页面，附带完整的 Chrome 浏览器请求头，
//! 返回 HTML 文本 + 最终 URL，让前端用 `srcDoc` 渲染（避开 X-Frame-Options 限制）。
//!
//! 失败时返回中文错误字符串，便于前端在 Tauri IPC 错误链路中直接展示。
//!
//! 部分站点（典型：kkrb.net）首次返回的 HTML 只含一段 JS，用于
//! 1) 通过 `document.cookie = ...` 写入令牌 cookie
//! 2) 通过 `window.location.href = ...` 跳转到同源另一条 URL
//! 直接交给 `srcDoc` 渲染会让 iframe 重新以 WebView UA 抓取那条 URL，
//! 丢失刚刚写入的 cookie，又会卡回同一段 JS 死循环。
//!
//! 因此本模块在解析响应体后做一次"JS 重定向嗅探"：识别出 `document.cookie` +
//! `window.location.href / location.replace` 模式时，把 cookie 注入到 reqwest 的 cookie jar
//! 并继续向跳转目标再发起一次请求，最多跟随 `MAX_JS_REDIRECTS` 次。
use std::time::Duration;
use regex::Regex;
use reqwest::cookie::Jar;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CACHE_CONTROL,
    REFERER, UPGRADE_INSECURE_REQUESTS, USER_AGENT,
};
use url::Url;

use super::types::{StrategyFetchRequest, StrategyFetchResponse};

/// 单次 HTTP 请求的最长等待时间。
const FETCH_TIMEOUT_SECS: u64 = 15;
/// 单次响应体最大读取字节数（10 MB）。
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;
/// JS 重定向嗅探最多允许的跟随次数。
const MAX_JS_REDIRECTS: usize = 3;
/// 嗅探出的 JS 重定向目标，最长允许的相对路径长度（防御性上限）。
const JS_REDIRECT_TARGET_MAX_LEN: usize = 1024;
/// Chrome 135 在 Windows 10 上的 User-Agent 字符串。
const CHROME_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

/// 拼装一次性自定义头：reqwest 未提供 `Sec-Ch-Ua` / `Sec-Fetch-*` 常量，
/// 必须通过 `HeaderName::from_static` 构造。
fn set_sec_header(headers: &mut HeaderMap, name: &'static str, value: &'static str) {
    if let (Ok(header_name), Ok(header_value)) = (
        HeaderName::from_bytes(name.as_bytes()),
        HeaderValue::from_str(value),
    ) {
        headers.insert(header_name, header_value);
    }
}

/// 拼装完整的 Chrome 浏览器请求头。
///
/// - `Sec-Ch-Ua-*` 告诉服务器客户端是 Chrome 135 / Windows
/// - `Sec-Fetch-*` 模拟 top-level navigation
/// - `Accept` / `Accept-Language` 收敛到真实浏览器值
/// - `Referer` 缺省时回落到目标 URL 自身，模拟"在新标签页输入 URL"的场景
fn build_headers(request: &StrategyFetchRequest) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();

    headers.insert(USER_AGENT, HeaderValue::from_static(CHROME_USER_AGENT));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        ),
    );
    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6"),
    );
    headers.insert(
        ACCEPT_ENCODING,
        HeaderValue::from_static("gzip, deflate, br, zstd"),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(UPGRADE_INSECURE_REQUESTS, HeaderValue::from_static("1"));
    set_sec_header(
        &mut headers,
        "sec-ch-ua",
        "\"Chromium\";v=\"135\", \"Not-A.Brand\";v=\"8\", \"Google Chrome\";v=\"135\"",
    );
    set_sec_header(&mut headers, "sec-ch-ua-mobile", "?0");
    set_sec_header(&mut headers, "sec-ch-ua-platform", "\"Windows\"");
    set_sec_header(&mut headers, "sec-fetch-dest", "document");
    set_sec_header(&mut headers, "sec-fetch-mode", "navigate");
    set_sec_header(&mut headers, "sec-fetch-site", "none");
    set_sec_header(&mut headers, "sec-fetch-user", "?1");

    let referer_value = request
        .referer
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(request.url.as_str());
    let referer_header = HeaderValue::from_str(referer_value)
        .map_err(|error| format!("referer 头无效：{error}"))?;
    headers.insert(REFERER, referer_header);

    Ok(headers)
}

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

/// 嗅探 HTML 中的 JS 重定向模式。
///
/// 返回值：
/// - `Some((cookie, target))` 当且仅当 HTML 同时包含 `document.cookie = '...'` 写入
///   与 `window.location.href = '...'`（或 `location.replace(...)` / `location.href = ...`）跳转
/// - `None` 表示该 HTML 不需要 JS 嗅探（正常页面或无跳转）
fn detect_js_redirect(html: &str) -> Option<(String, String)> {
    if html.len() > 64 * 1024 {
        // 过大的 HTML 不太可能是单脚本重定向；避免误判。
        return None;
    }
    let cookie_re = Regex::new(r#"(?is)document\.cookie\s*=\s*['"]([^'"]+)['"]"#).ok()?;
    let target_re = Regex::new(
        r#"(?is)(?:window\.location\.href|location\.href|location\.replace)\s*=\s*['"]([^'"]+)['"]"#,
    )
    .ok()?;

    let cookie_match = cookie_re.captures(html)?;
    let target_match = target_re.captures(html)?;
    let cookie = cookie_match.get(1)?.as_str().trim().to_string();
    let target = target_match.get(1)?.as_str().trim().to_string();
    if cookie.is_empty()
        || target.is_empty()
        || target.len() > JS_REDIRECT_TARGET_MAX_LEN
    {
        return None;
    }
    if !cookie.contains('=') || cookie.contains(';') {
        return None;
    }
    Some((cookie, target))
}

/// 在 reqwest 的共享 cookie jar 上写入一段 cookie 字符串，作用域为 `base_url`。
fn push_cookie(jar: &Jar, base_url: &Url, raw: &str) -> Result<(), String> {
    let (pair, attrs) = match raw.split_once(';') {
        Some((pair, attrs)) => (pair.trim(), attrs.trim()),
        None => (raw.trim(), ""),
    };
    if pair.is_empty() {
        return Err(format!("cookie 格式无效：{raw}"));
    }
    let host = base_url
        .host_str()
        .ok_or_else(|| "无法从目标 URL 解析 host".to_string())?;
    let mut normalized = format!("{pair}; Domain={host}; Path=/");
    if !attrs.is_empty() {
        normalized.push_str("; ");
        normalized.push_str(attrs);
    }
    jar.add_cookie_str(&normalized, base_url);
    Ok(())
}
#[derive(Debug, Clone)]
struct FetchedPage {
    status: u16,
    final_url: String,
    content_type: String,
    html: String,
    byte_length: usize,
}

/// 单次拉取（包含 JS 重定向嗅探跟随）。
async fn fetch_with_js_redirect_following(
    request: &StrategyFetchRequest,
) -> Result<FetchedPage, String> {
    let initial_url = normalize_url(&request.url)?;
    let headers = build_headers(request)?;

    let jar = std::sync::Arc::new(Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .default_headers(headers)
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(8))
        .user_agent(CHROME_USER_AGENT)
        .build()
        .map_err(|error| format!("构建 HTTP 客户端失败：{error}"))?;

    let mut current_url = initial_url;
    let mut last: Option<FetchedPage> = None;

    for hop in 0..=MAX_JS_REDIRECTS {
        let response = client
            .get(current_url.as_str())
            .send()
            .await
            .map_err(|error| format!("请求 {current_url} 失败：{error}"))?;

        let status = response.status();
        let final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();

        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("读取响应体失败：{error}"))?;
        if bytes.len() > MAX_BODY_BYTES {
            return Err(format!(
                "响应体超过 {} MB 上限（实际 {} 字节）",
                MAX_BODY_BYTES / 1024 / 1024,
                bytes.len()
            ));
        }
        let html = String::from_utf8_lossy(&bytes).into_owned();
        let page = FetchedPage {
            status: status.as_u16(),
            final_url: final_url.clone(),
            content_type,
            html: html.clone(),
            byte_length: bytes.len(),
        };

        if hop == MAX_JS_REDIRECTS {
            return Ok(page);
        }

        if let Some((cookie, raw_target)) = detect_js_redirect(&html) {
            let target_url = match Url::parse(&raw_target) {
                Ok(parsed) if parsed.has_host() => parsed,
                _ => match current_url.join(&raw_target) {
                    Ok(joined) => joined,
                    Err(_) => return Ok(page),
                },
            };
            if target_url.origin() != current_url.origin() {
                return Ok(page);
            }
            if push_cookie(&jar, &current_url, &cookie).is_err() {
                return Ok(page);
            }
            last = Some(page);
            current_url = target_url;
            continue;
        }

        return Ok(page);
    }

    Ok(last.expect("JS redirect loop should populate last before returning"))
}

/// 抓取攻略页面。
#[tauri::command]
pub async fn fetch_strategy_page(
    request: StrategyFetchRequest,
) -> Result<StrategyFetchResponse, String> {
    let page = fetch_with_js_redirect_following(&request).await?;
    Ok(StrategyFetchResponse {
        status: page.status,
        final_url: page.final_url,
        content_type: page.content_type,
        html: page.html,
        byte_length: page.byte_length,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::cookie::CookieStore;

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
    fn build_headers_includes_chrome_ua() {
        let request = StrategyFetchRequest {
            url: "https://www.kkrb.net/".to_string(),
            referer: None,
        };
        let headers = build_headers(&request).expect("headers");
        let ua = headers
            .get(USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .expect("ua");
        assert!(ua.contains("Chrome/135"));
        assert!(headers.contains_key(REFERER));
    }

    #[test]
    fn build_headers_prefers_explicit_referer() {
        let request = StrategyFetchRequest {
            url: "https://www.kkrb.net/path".to_string(),
            referer: Some("https://www.google.com/".to_string()),
        };
        let headers = build_headers(&request).expect("headers");
        let referer = headers
            .get(REFERER)
            .and_then(|value| value.to_str().ok())
            .expect("referer");
        assert_eq!(referer, "https://www.google.com/");
    }

    #[test]
    fn detect_js_redirect_matches_kkrb_pattern() {
        let html = r#"<script>document.cookie = 'yxd_token=abc123'
window.location.href='/?viewpage=view%2Foverview'</script>"#;
        let result = detect_js_redirect(html);
        assert_eq!(
            result,
            Some((
                "yxd_token=abc123".to_string(),
                "/?viewpage=view%2Foverview".to_string()
            ))
        );
    }

    #[test]
    fn detect_js_redirect_returns_none_for_real_html() {
        let html = r#"<html><head><title>KK 日报</title></head><body>...</body></html>"#;
        assert!(detect_js_redirect(html).is_none());
    }

    #[test]
    fn detect_js_redirect_rejects_invalid_cookie() {
        let html = r#"<script>document.cookie='broken; value'
window.location.href='/'</script>"#;
        assert!(detect_js_redirect(html).is_none());
    }

    #[test]
    fn push_cookie_writes_into_jar() {
        let jar = Jar::default();
        let url = Url::parse("https://www.kkrb.net/path").unwrap();
        push_cookie(&jar, &url, "yxd_token=abc123").expect("push");
        let value = jar.cookies(&url).expect("cookie was just pushed");
        let as_str = value.to_str().unwrap();
        assert!(as_str.contains("yxd_token=abc123"));
    }

    #[tokio::test]
    async fn fetch_strategy_page_follows_js_redirect_via_mockito() {
        let mut server = mockito::Server::new_async().await;
        let redirect_mock = server
            .mock("GET", "/?viewpage=view%2Foverview")
            .match_header(
                "user-agent",
                mockito::Matcher::Regex("Chrome/135.*".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "text/html; charset=utf-8")
            .with_body(
                "<script>document.cookie = 'yxd_token=abc123'\nwindow.location.href='/?viewpage=view%2Foverview&token=abc123'</script>",
            )
            .expect_at_least(1)
            .create_async()
            .await;

        let final_mock = server
            .mock("GET", "/?viewpage=view%2Foverview&token=abc123")
            .match_header(
                "user-agent",
                mockito::Matcher::Regex("Chrome/135.*".to_string()),
            )
            .match_header(
                "cookie",
                mockito::Matcher::Regex("yxd_token=abc123.*".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "text/html; charset=utf-8")
            .with_body("<html><body>KK 日报</body></html>")
            .expect_at_least(1)
            .create_async()
            .await;

        let request = StrategyFetchRequest {
            url: format!("{}/?viewpage=view%2Foverview", server.url()),
            referer: None,
        };
        let response = fetch_strategy_page(request).await.expect("response");
        redirect_mock.assert_async().await;
        final_mock.assert_async().await;

        assert_eq!(response.status, 200);
        assert!(response.html.contains("KK 日报"));
    }

    #[tokio::test]
    async fn fetch_strategy_page_returns_html_via_mockito() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/viewpage/view%2Foverview")
            .match_header(
                "user-agent",
                mockito::Matcher::Regex("Chrome/135.*".to_string()),
            )
            .match_header("sec-ch-ua", mockito::Matcher::Any)
            .match_header("accept-language", mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "text/html; charset=utf-8")
            .with_body("<html><body>KK 日报</body></html>")
            .create_async()
            .await;

        let request = StrategyFetchRequest {
            url: format!("{}/viewpage/view%2Foverview", server.url()),
            referer: None,
        };
        let response = fetch_strategy_page(request).await.expect("response");
        mock.assert_async().await;

        assert_eq!(response.status, 200);
        assert!(response.content_type.starts_with("text/html"));
        assert!(response.html.contains("KK 日报"));
        assert!(response.byte_length > 0);
    }
}
