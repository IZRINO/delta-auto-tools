//! 攻略网站代理抓取实现。
//!
//! 通过 reqwest 拉取目标页面，附带完整的 Chrome 浏览器请求头，
//! 返回 HTML 文本 + 最终 URL，让前端用 `srcDoc` 渲染（避开 X-Frame-Options 限制）。
//!
//! 失败时返回中文错误字符串，便于前端在 Tauri IPC 错误链路中直接展示。

use std::time::Duration;

use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CACHE_CONTROL,
    REFERER, UPGRADE_INSECURE_REQUESTS, USER_AGENT,
};
use url::Url;

use super::types::{StrategyFetchRequest, StrategyFetchResponse};

/// 拉取单次响应的最长等待时间。
const FETCH_TIMEOUT_SECS: u64 = 15;
/// 单次响应体最大读取字节数（10 MB）。
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;
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

/// 抓取攻略页面。
///
/// 该函数会构造一次临时 reqwest 客户端并发起请求，不复用任何状态（无 cookie 共享）；
/// 每次刷新即重新拉取，便于绕过部分站点按 IP 频控的人机验证。
#[tauri::command]
pub async fn fetch_strategy_page(
    request: StrategyFetchRequest,
) -> Result<StrategyFetchResponse, String> {
    let target_url = normalize_url(&request.url)?;
    let headers = build_headers(&request)?;

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(8))
        .user_agent(CHROME_USER_AGENT)
        .build()
        .map_err(|error| format!("构建 HTTP 客户端失败：{error}"))?;

    let response = client
        .get(target_url.as_str())
        .send()
        .await
        .map_err(|error| format!("请求 {target_url} 失败：{error}"))?;

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

    Ok(StrategyFetchResponse {
        status: status.as_u16(),
        final_url,
        content_type,
        html,
        byte_length: bytes.len(),
    })
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
