//! 攻略网站 HTTP 抓取 + CC check 检测 + JS 重定向跟随。
//!
//! 提供一个 Tauri command：`strategy_fetch_page`，
//! 使用 reqwest 以 Chrome 135 身份请求目标 URL，
//! 检测 CC check 安全验证页面并跟随 JS 重定向（最多 3 层）。

use std::sync::Arc;

use regex::Regex;
use reqwest::header;

use super::types::{ChallengeInfo, StrategyFetchResponse};

/// JS 重定向最大跟随层数。
const MAX_REDIRECT_DEPTH: u32 = 3;

// ---------------------------------------------------------------------------
// 默认请求头：Chrome 135 on Windows
// ---------------------------------------------------------------------------

fn default_headers() -> header::HeaderMap {
    let mut h = header::HeaderMap::new();

    h.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36",
        ),
    );
    h.insert(
        header::ACCEPT,
        header::HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,\
             image/avif,image/webp,image/apng,*/*;q=0.8",
        ),
    );
    h.insert(
        header::ACCEPT_LANGUAGE,
        header::HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
    );
    h.insert(
        header::HeaderName::from_static("sec-ch-ua"),
        header::HeaderValue::from_static(
            r#""Google Chrome";v="135", "Not-A.Brand";v="8", "Chromium";v="135""#,
        ),
    );
    h.insert(
        header::HeaderName::from_static("sec-ch-ua-mobile"),
        header::HeaderValue::from_static("?0"),
    );
    h.insert(
        header::HeaderName::from_static("sec-ch-ua-platform"),
        header::HeaderValue::from_static(r#""Windows""#),
    );
    h.insert(
        header::HeaderName::from_static("sec-fetch-dest"),
        header::HeaderValue::from_static("document"),
    );
    h.insert(
        header::HeaderName::from_static("sec-fetch-mode"),
        header::HeaderValue::from_static("navigate"),
    );
    h.insert(
        header::HeaderName::from_static("sec-fetch-site"),
        header::HeaderValue::from_static("none"),
    );
    h.insert(
        header::HeaderName::from_static("sec-fetch-user"),
        header::HeaderValue::from_static("?1"),
    );
    h.insert(
        header::UPGRADE_INSECURE_REQUESTS,
        header::HeaderValue::from_static("1"),
    );

    h
}

// ---------------------------------------------------------------------------
// CC check 检测
// ---------------------------------------------------------------------------

/// 检测响应 HTML 是否命中 CC check 安全验证页面。
fn detect_cc_challenge(html: &str) -> Option<ChallengeInfo> {
    // 精确标题匹配
    if html.contains("<title>CC check</title>") {
        return Some(ChallengeInfo {
            kind: "ccCheck".into(),
            message: "检测到 CC check 安全验证页面".into(),
        });
    }

    // CDN 盾路径
    if html.contains("/cdn-shield/") {
        return Some(ChallengeInfo {
            kind: "ccCheck".into(),
            message: "检测到 CDN Shield 安全验证".into(),
        });
    }

    // 中文安全验证组合特征
    if html.contains("安全验证") && html.contains("点击确认您是真人") {
        return Some(ChallengeInfo {
            kind: "ccCheck".into(),
            message: "检测到安全验证页面（安全验证 + 点击确认您是真人）".into(),
        });
    }

    // 验证卡片 class
    if html.contains("verification-card") {
        return Some(ChallengeInfo {
            kind: "ccCheck".into(),
            message: "检测到验证卡片（verification-card）".into(),
        });
    }

    None
}

// ---------------------------------------------------------------------------
// JS 重定向提取
// ---------------------------------------------------------------------------

/// 从 HTML 中提取 JS 重定向信息：`document.cookie = '...'` +
/// `location.href = '...'` / `window.location.href = '...'`（或 replace 调用）。
///
/// 返回 `(cookie_str, target_url)`。
fn extract_js_redirect(html: &str) -> Option<(String, String)> {
    let cookie_re = Regex::new(r#"document\.cookie\s*=\s*['"]([^'"]+)['"]"#).ok()?;
    let location_re = Regex::new(
        r#"(?:window\.)?location\.(?:href\s*=|replace\s*\()\s*['\"]([^'\"]+)['\"]"#,
    )
    .ok()?;

    let cookie = cookie_re.captures(html)?.get(1)?.as_str().to_string();
    let target = location_re.captures(html)?.get(1)?.as_str().to_string();

    Some((cookie, target))
}

// ---------------------------------------------------------------------------
// 递归抓取
// ---------------------------------------------------------------------------

/// 递归抓取 HTML，跟随 JS 重定向最多 `MAX_REDIRECT_DEPTH` 层。
async fn fetch_with_redirect(
    client: &reqwest::Client,
    jar: &Arc<reqwest::cookie::Jar>,
    url: &str,
    depth: u32,
) -> Result<StrategyFetchResponse, String> {
    if depth >= MAX_REDIRECT_DEPTH {
        return Err("JS 重定向层数超过上限".into());
    }

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败：{e}"))?;

    let final_url = response.url().to_string();
    let html = response
        .text()
        .await
        .map_err(|e| format!("读取响应体失败：{e}"))?;

    // 1) 优先检测 CC challenge
    if let Some(challenge) = detect_cc_challenge(&html) {
        return Ok(StrategyFetchResponse {
            html,
            final_url,
            challenge: Some(challenge),
        });
    }

    // 2) 检测 JS 重定向
    if let Some((cookie_str, target_url)) = extract_js_redirect(&html) {
        // 将 cookie 关联到重定向来源域（document.cookie 设置的是当前页域）
        if let Ok(parsed) = url::Url::parse(url) {
            jar.add_cookie_str(&cookie_str, &parsed);
        }

        // 解析目标 URL（支持相对路径）
        let resolved = resolve_url(url, &target_url)?;
        return Box::pin(fetch_with_redirect(client, jar, &resolved, depth + 1)).await;
    }

    Ok(StrategyFetchResponse {
        html,
        final_url,
        challenge: None,
    })
}

/// 解析目标 URL，若为相对路径则以当前 URL 为基准解析。
fn resolve_url(current: &str, target: &str) -> Result<String, String> {
    // 先尝试直接解析
    if let Ok(parsed) = url::Url::parse(target) {
        if parsed.scheme() == "http" || parsed.scheme() == "https" {
            return Ok(parsed.to_string());
        }
    }

    // 相对路径：以当前 URL 为基准
    let base = url::Url::parse(current).map_err(|e| format!("当前 URL 解析失败：{e}"))?;
    let resolved = base
        .join(target)
        .map_err(|e| format!("目标 URL 解析失败：{e}"))?;
    Ok(resolved.to_string())
}

// ---------------------------------------------------------------------------
// Tauri command
// ---------------------------------------------------------------------------

/// 抓取攻略网站页面。
///
/// 使用 Chrome 135 身份请求目标 URL，自动检测 CC check 安全验证并跟随 JS 重定向。
/// 如果命中 CC check，`challenge` 字段会包含验证信息，
/// 前端可回退到 `strategy_open_window` 让用户手动完成验证。
#[tauri::command]
pub async fn strategy_fetch_page(url: String) -> Result<StrategyFetchResponse, String> {
    let headers = default_headers();
    let jar = Arc::new(reqwest::cookie::Jar::default());

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .cookie_provider(Arc::clone(&jar))
        .build()
        .map_err(|e| format!("构建 HTTP 客户端失败：{e}"))?;

    fetch_with_redirect(&client, &jar, &url, 0).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_js_redirect_supports_window_location_href() {
        let html = r#"<script>document.cookie = 'shield=ok; path=/'; window.location.href = '/next';</script>"#;
        let (cookie, target) = extract_js_redirect(html).unwrap();
        assert_eq!(cookie, "shield=ok; path=/");
        assert_eq!(target, "/next");
    }

    #[test]
    fn extract_js_redirect_supports_location_replace_call() {
        let html = r#"<script>document.cookie = "shield=ok"; location.replace("/next");</script>"#;
        let (cookie, target) = extract_js_redirect(html).unwrap();
        assert_eq!(cookie, "shield=ok");
        assert_eq!(target, "/next");
    }
}
