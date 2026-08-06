use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    webview::WebviewBuilder, AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl,
};
use tokio::sync::oneshot;
use url::Url;
use uuid::Uuid;

const MOLIGOD_TITLE_PREFIX: &str = "DELTA_SPECIAL_OPS_PROFIT_RESULT:";
const MOLIGOD_ENDPOINT: &str = "https://moligod.com/ammo-exchange";
const MOLIGOD_QUERY_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MOLIGOD_CANCEL_POLL: Duration = Duration::from_millis(100);
const MOLIGOD_CLEANUP_RETRIES: usize = 30;
const MOLIGOD_CLEANUP_RETRY_DELAY: Duration = Duration::from_secs(1);
const MOLIGOD_CHILD_POSITION: f64 = -10_000.0;
const MOLIGOD_CHILD_WIDTH: f64 = 1_024.0;
const MOLIGOD_CHILD_HEIGHT: f64 = 720.0;
pub(crate) const MAX_MOLIGOD_TITLE_BYTES: usize = 64 * 1024;
type MoligodResultSender = Arc<Mutex<Option<oneshot::Sender<Result<MoligodSnapshot, String>>>>>;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MoligodRequestTarget {
    pub(crate) rule_id: String,
    pub(crate) exact_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MoligodExpected {
    pub(crate) generation: u64,
    pub(crate) nonce: String,
    pub(crate) targets: Vec<MoligodRequestTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MoligodRuleStatus {
    Matched,
    SourceFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MoligodRuleResult {
    pub(crate) rule_id: String,
    pub(crate) exact_name: String,
    pub(crate) profit: Option<i64>,
    pub(crate) status: MoligodRuleStatus,
    pub(crate) detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MoligodSnapshot {
    pub(crate) generation: u64,
    pub(crate) results: Vec<MoligodRuleResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMoligodPayload {
    generation: u64,
    nonce: String,
    results: Vec<RawMoligodRuleResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMoligodRuleResult {
    rule_id: String,
    exact_name: String,
    #[serde(default)]
    profit: Option<String>,
    status: MoligodRuleStatus,
    #[serde(default)]
    detail: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MoligodScriptConfig<'a> {
    generation: u64,
    nonce: &'a str,
    targets: &'a [MoligodRequestTarget],
}

#[derive(Clone)]
pub(crate) struct MoligodAdapter {
    app: AppHandle,
}

fn hidden_webview_geometry() -> (LogicalPosition<f64>, LogicalSize<f64>) {
    (
        LogicalPosition::new(MOLIGOD_CHILD_POSITION, MOLIGOD_CHILD_POSITION),
        LogicalSize::new(MOLIGOD_CHILD_WIDTH, MOLIGOD_CHILD_HEIGHT),
    )
}

impl MoligodAdapter {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub(crate) async fn fetch(
        &self,
        generation: u64,
        targets: Vec<MoligodRequestTarget>,
        cancelled: Arc<AtomicBool>,
    ) -> Result<MoligodSnapshot, String> {
        if targets.is_empty() {
            return Err("Moligod 查询目标为空".to_string());
        }
        if cancelled.load(Ordering::SeqCst) {
            return Err("Moligod 查询已取消".to_string());
        }

        let id = Uuid::new_v4().simple().to_string();
        let expected = Arc::new(MoligodExpected {
            generation,
            nonce: id.clone(),
            targets,
        });
        let host_window = self
            .app
            .get_webview_window("main")
            .ok_or_else(|| "Moligod 查询主窗口不可用".to_string())?;
        let script = build_initialization_script(&expected)?;
        let temp_dir = tempfile::Builder::new()
            .prefix("delta-special-ops-profit-")
            .tempdir()
            .map_err(|error| format!("创建 Moligod 临时目录失败：{error}"))?;
        let data_path = temp_dir.keep();
        let label = format!("special-ops-profit-{id}");
        let endpoint = MOLIGOD_ENDPOINT
            .parse()
            .map_err(|error| format!("解析 Moligod URL 失败：{error}"))?;
        let (sender, receiver) = oneshot::channel();
        let sender = Arc::new(Mutex::new(Some(sender)));
        let navigation_sender = Arc::clone(&sender);
        let title_sender = Arc::clone(&sender);
        let title_expected = Arc::clone(&expected);
        let webview_builder = WebviewBuilder::new(&label, WebviewUrl::External(endpoint))
            .data_directory(data_path.clone())
            .initialization_script(script)
            .on_navigation(move |url| {
                let allowed = is_allowed_moligod_navigation(url);
                if !allowed {
                    send_moligod_result(
                        &navigation_sender,
                        Err(format!("Moligod 拒绝跨站导航：{url}")),
                    );
                }
                allowed
            })
            .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
            .on_download(|_, _| false)
            .on_document_title_changed(move |_, title| {
                if let Some(result) = validated_title_event(&title, &title_expected) {
                    send_moligod_result(&title_sender, result);
                }
            });
        let (position, size) = hidden_webview_geometry();
        let webview = match host_window
            .as_ref()
            .window()
            .add_child(webview_builder, position, size)
        {
            Ok(webview) => webview,
            Err(error) => {
                let cleanup = cleanup_data_directory(&data_path).await;
                return Err(match cleanup {
                    Ok(()) => format!("创建 Moligod 后台页面失败：{error}"),
                    Err(cleanup_error) => format!(
                        "创建 Moligod 后台页面失败：{error}；临时目录清理失败：{cleanup_error}"
                    ),
                });
            }
        };

        let query_result = wait_for_moligod_result(receiver, cancelled).await;
        let destroy_result = webview
            .close()
            .map_err(|error| format!("关闭 Moligod 后台页面失败：{error}"));
        drop(webview);
        spawn_data_directory_cleanup(data_path);
        finish_query_result(query_result, destroy_result)
    }
}

pub(crate) fn build_initialization_script(expected: &MoligodExpected) -> Result<String, String> {
    let config = serde_json::to_string(&MoligodScriptConfig {
        generation: expected.generation,
        nonce: &expected.nonce,
        targets: &expected.targets,
    })
    .map_err(|error| format!("序列化 Moligod 只读配置失败：{error}"))?;
    Ok(format!(
        "if (window.location.origin === \"https://moligod.com\") {{\nwindow.__DELTA_SPECIAL_OPS_MOLIGOD_CONFIG__ = {config};\n{}\n}}",
        include_str!("moligod_scraper.js")
    ))
}

pub(crate) fn validated_title_event(
    title: &str,
    expected: &MoligodExpected,
) -> Option<Result<MoligodSnapshot, String>> {
    title
        .starts_with(MOLIGOD_TITLE_PREFIX)
        .then(|| parse_moligod_title(title, expected))
}

fn send_moligod_result(sender: &MoligodResultSender, result: Result<MoligodSnapshot, String>) {
    if let Ok(mut sender) = sender.lock() {
        if let Some(sender) = sender.take() {
            let _ = sender.send(result);
        }
    }
}

async fn wait_for_moligod_result(
    mut receiver: oneshot::Receiver<Result<MoligodSnapshot, String>>,
    cancelled: Arc<AtomicBool>,
) -> Result<MoligodSnapshot, String> {
    tokio::time::timeout(MOLIGOD_QUERY_TIMEOUT, async move {
        loop {
            if cancelled.load(Ordering::SeqCst) {
                return Err("Moligod 查询已取消".to_string());
            }
            tokio::select! {
                result = &mut receiver => {
                    return result
                        .map_err(|_| "Moligod title 结果通道已关闭".to_string())?;
                }
                _ = tokio::time::sleep(MOLIGOD_CANCEL_POLL) => {}
            }
        }
    })
    .await
    .map_err(|_| "Moligod 查询超时".to_string())?
}

fn finish_query_result(
    query_result: Result<MoligodSnapshot, String>,
    destroy_result: Result<(), String>,
) -> Result<MoligodSnapshot, String> {
    destroy_result?;
    query_result
}

fn spawn_data_directory_cleanup(path: PathBuf) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = cleanup_data_directory(&path).await {
            crate::log_warn!(
                "special_ops::profit",
                "清理 Moligod 临时目录失败",
                "error" => error
            );
        }
    });
}

async fn cleanup_data_directory(path: &Path) -> Result<(), String> {
    let path = PathBuf::from(path);
    let mut last_error = None;
    for retry in 0..=MOLIGOD_CLEANUP_RETRIES {
        match std::fs::remove_dir_all(&path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => last_error = Some(error),
        }
        if retry < MOLIGOD_CLEANUP_RETRIES {
            tokio::time::sleep(MOLIGOD_CLEANUP_RETRY_DELAY).await;
        }
    }
    Err(format!(
        "{}",
        last_error.expect("清理重试失败时必须保留最后错误")
    ))
}

pub(crate) fn parse_moligod_title(
    title: &str,
    expected: &MoligodExpected,
) -> Result<MoligodSnapshot, String> {
    if title.len() > MAX_MOLIGOD_TITLE_BYTES {
        return Err("Moligod title payload 超过 64 KiB".to_string());
    }
    let prefix = format!("{MOLIGOD_TITLE_PREFIX}{}:", expected.nonce);
    let encoded = title
        .strip_prefix(&prefix)
        .ok_or_else(|| "Moligod title nonce 或前缀不匹配".to_string())?;
    let decoded = decode_base64url(encoded)?;
    let payload: RawMoligodPayload = serde_json::from_slice(&decoded)
        .map_err(|error| format!("Moligod title JSON 无效：{error}"))?;
    if payload.generation != expected.generation {
        return Err("Moligod generation 已失效".to_string());
    }
    if payload.nonce != expected.nonce {
        return Err("Moligod payload nonce 不匹配".to_string());
    }

    let expected_by_id = expected
        .targets
        .iter()
        .map(|target| (target.rule_id.as_str(), target.exact_name.as_str()))
        .collect::<HashMap<_, _>>();
    if expected_by_id.len() != expected.targets.len() {
        return Err("Moligod 请求包含重复 ruleId".to_string());
    }
    let mut seen = HashSet::with_capacity(payload.results.len());
    let mut results = Vec::with_capacity(payload.results.len());
    for result in payload.results {
        if !seen.insert(result.rule_id.clone()) {
            return Err(format!("Moligod 返回重复 ruleId：{}", result.rule_id));
        }
        let expected_name = expected_by_id
            .get(result.rule_id.as_str())
            .ok_or_else(|| format!("Moligod 返回未知 ruleId：{}", result.rule_id))?;
        if result.exact_name != *expected_name {
            return Err(format!("Moligod 精确名称不匹配：{}", result.rule_id));
        }
        let (profit, detail) = match result.status {
            MoligodRuleStatus::Matched => {
                let profit = result
                    .profit
                    .as_deref()
                    .ok_or_else(|| format!("Moligod 命中结果缺少利润：{}", result.rule_id))?;
                (Some(parse_i64_decimal(profit)?), result.detail)
            }
            MoligodRuleStatus::SourceFailure => {
                if result.profit.is_some() {
                    return Err(format!(
                        "Moligod sourceFailure 不得携带利润：{}",
                        result.rule_id
                    ));
                }
                let detail = result
                    .detail
                    .filter(|detail| !detail.trim().is_empty())
                    .ok_or_else(|| format!("Moligod sourceFailure 缺少详情：{}", result.rule_id))?;
                (None, Some(detail))
            }
        };
        results.push(MoligodRuleResult {
            rule_id: result.rule_id,
            exact_name: result.exact_name,
            profit,
            status: result.status,
            detail,
        });
    }
    if seen.len() != expected_by_id.len() {
        return Err("Moligod 返回结果缺少请求 ruleId".to_string());
    }

    Ok(MoligodSnapshot {
        generation: payload.generation,
        results,
    })
}

pub(crate) fn is_allowed_moligod_navigation(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("moligod.com")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
}

fn parse_i64_decimal(value: &str) -> Result<i64, String> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Moligod 利润不是规范十进制整数".to_string());
    }
    value
        .parse::<i64>()
        .map_err(|_| "Moligod 利润超出 i64".to_string())
}

fn decode_base64url(encoded: &str) -> Result<Vec<u8>, String> {
    if encoded.len() % 4 == 1 {
        return Err("Moligod title base64url 长度无效".to_string());
    }
    let mut output = Vec::with_capacity(encoded.len() * 3 / 4);
    let mut buffer = 0_u32;
    let mut bits = 0_u8;
    for byte in encoded.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err("Moligod title base64url 包含非法字符".to_string()),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
            buffer &= (1_u32 << bits) - 1;
        }
    }
    if buffer != 0 {
        return Err("Moligod title base64url padding bits 无效".to_string());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn request_target(rule_id: &str, exact_name: &str) -> MoligodRequestTarget {
        MoligodRequestTarget {
            rule_id: rule_id.to_string(),
            exact_name: exact_name.to_string(),
        }
    }

    fn expected(targets: Vec<MoligodRequestTarget>) -> MoligodExpected {
        MoligodExpected {
            generation: 7,
            nonce: "nonce-7".to_string(),
            targets,
        }
    }

    fn title(payload: Value, prefix_nonce: &str) -> String {
        let bytes = serde_json::to_vec(&payload).unwrap();
        format!(
            "DELTA_SPECIAL_OPS_PROFIT_RESULT:{prefix_nonce}:{}",
            encode_base64url(&bytes)
        )
    }

    fn encode_base64url(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let value = chunk
                .iter()
                .fold(0_u32, |value, byte| (value << 8) | u32::from(*byte))
                << ((3 - chunk.len()) * 8);
            encoded.push(ALPHABET[((value >> 18) & 0x3f) as usize] as char);
            encoded.push(ALPHABET[((value >> 12) & 0x3f) as usize] as char);
            if chunk.len() > 1 {
                encoded.push(ALPHABET[((value >> 6) & 0x3f) as usize] as char);
            }
            if chunk.len() > 2 {
                encoded.push(ALPHABET[(value & 0x3f) as usize] as char);
            }
        }
        encoded
    }

    fn matched_payload(rule_id: &str, exact_name: &str, profit: Value) -> Value {
        json!({
            "generation": 7,
            "nonce": "nonce-7",
            "results": [{
                "ruleId": rule_id,
                "exactName": exact_name,
                "profit": profit,
                "status": "matched"
            }]
        })
    }

    #[test]
    fn validates_title_payload_and_converts_profit_to_i64() {
        let expected = expected(vec![request_target("rule-a", "目标 A")]);
        let parsed = parse_moligod_title(
            &title(
                matched_payload("rule-a", "目标 A", json!("270458")),
                "nonce-7",
            ),
            &expected,
        )
        .unwrap();

        assert_eq!(parsed.generation, 7);
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.results[0].profit, Some(270458));
        assert_eq!(parsed.results[0].status, MoligodRuleStatus::Matched);
    }

    #[test]
    fn rejects_nonce_generation_and_oversized_title() {
        let expected = expected(vec![request_target("rule-a", "目标 A")]);
        assert!(parse_moligod_title(
            &title(matched_payload("rule-a", "目标 A", json!("1")), "wrong"),
            &expected,
        )
        .is_err());

        let wrong_generation = json!({
            "generation": 8,
            "nonce": "nonce-7",
            "results": [{
                "ruleId": "rule-a",
                "exactName": "目标 A",
                "profit": "1",
                "status": "matched"
            }]
        });
        assert!(parse_moligod_title(&title(wrong_generation, "nonce-7"), &expected).is_err());
        assert!(parse_moligod_title(&"x".repeat(MAX_MOLIGOD_TITLE_BYTES + 1), &expected).is_err());
    }

    #[test]
    fn rejects_duplicate_unknown_missing_and_mismatched_results() {
        let expected = expected(vec![request_target("rule-a", "目标 A")]);
        let result = json!({
            "ruleId": "rule-a",
            "exactName": "目标 A",
            "profit": "1",
            "status": "matched"
        });
        for payload in [
            json!({"generation": 7, "nonce": "nonce-7", "results": [result.clone(), result.clone()]}),
            json!({"generation": 7, "nonce": "nonce-7", "results": [{
                "ruleId": "rule-b", "exactName": "目标 B", "profit": "1", "status": "matched"
            }]}),
            json!({"generation": 7, "nonce": "nonce-7", "results": []}),
            json!({"generation": 7, "nonce": "nonce-7", "results": [{
                "ruleId": "rule-a", "exactName": "目标 B", "profit": "1", "status": "matched"
            }]}),
        ] {
            assert!(parse_moligod_title(&title(payload, "nonce-7"), &expected).is_err());
        }
    }

    #[test]
    fn rejects_non_string_decimal_and_out_of_range_profit() {
        let expected = expected(vec![request_target("rule-a", "目标 A")]);
        for profit in [
            json!(1),
            json!("1.5"),
            json!("abc"),
            json!("9223372036854775808"),
            json!("-9223372036854775809"),
        ] {
            assert!(parse_moligod_title(
                &title(matched_payload("rule-a", "目标 A", profit), "nonce-7"),
                &expected,
            )
            .is_err());
        }
    }

    #[test]
    fn accepts_explicit_source_failure_without_profit() {
        let expected = expected(vec![request_target("rule-a", "目标 A")]);
        let payload = json!({
            "generation": 7,
            "nonce": "nonce-7",
            "results": [{
                "ruleId": "rule-a",
                "exactName": "目标 A",
                "status": "sourceFailure",
                "detail": "详情缺失"
            }]
        });

        let parsed = parse_moligod_title(&title(payload, "nonce-7"), &expected).unwrap();

        assert_eq!(parsed.results[0].status, MoligodRuleStatus::SourceFailure);
        assert_eq!(parsed.results[0].profit, None);
    }

    #[test]
    fn navigation_only_allows_exact_https_moligod_origin() {
        for allowed in [
            "https://moligod.com",
            "https://moligod.com/ammo-exchange",
            "https://moligod.com/path?next=1#result",
        ] {
            assert!(is_allowed_moligod_navigation(&allowed.parse().unwrap()));
        }
        for rejected in [
            "http://moligod.com/ammo-exchange",
            "https://sub.moligod.com/ammo-exchange",
            "https://moligod.com.evil.example/ammo-exchange",
            "https://moligod.com@evil.example/ammo-exchange",
            "https://user@moligod.com/ammo-exchange",
            "https://moligod.com:444/ammo-exchange",
        ] {
            assert!(!is_allowed_moligod_navigation(&rejected.parse().unwrap()));
        }
    }

    #[test]
    fn initialization_script_embeds_read_only_config_without_tauri_ipc() {
        let expected = expected(vec![request_target("rule-a", "目标 A")]);

        let script = build_initialization_script(&expected).unwrap();

        assert!(script.contains("window.location.origin === \"https://moligod.com\""));
        assert!(script.contains("\"generation\":7"));
        assert!(script.contains("\"nonce\":\"nonce-7\""));
        assert!(script.contains("\"ruleId\":\"rule-a\""));
        assert!(script.contains("DELTA_SPECIAL_OPS_PROFIT_RESULT:"));
        assert!(!script.contains("__TAURI__"));
    }

    #[test]
    fn title_event_ignores_normal_page_title_and_validates_result_title() {
        let expected = expected(vec![request_target("rule-a", "目标 A")]);
        assert!(validated_title_event("摩力数据", &expected).is_none());

        let result = validated_title_event(
            &title(matched_payload("rule-a", "目标 A", json!("1")), "nonce-7"),
            &expected,
        )
        .unwrap()
        .unwrap();

        assert_eq!(result.results[0].profit, Some(1));
    }

    #[test]
    fn cleanup_failure_does_not_replace_query_result() {
        let snapshot = MoligodSnapshot {
            generation: 7,
            results: vec![],
        };

        assert_eq!(
            finish_query_result(Ok(snapshot.clone()), Ok(())),
            Ok(snapshot)
        );
        assert_eq!(
            finish_query_result(
                Ok(MoligodSnapshot {
                    generation: 7,
                    results: vec![],
                }),
                Err("销毁失败".to_string())
            ),
            Err("销毁失败".to_string())
        );
    }

    #[test]
    fn hidden_webview_geometry_stays_outside_parent_without_mobile_viewport() {
        let (position, size) = hidden_webview_geometry();

        assert!(position.x < 0.0);
        assert!(position.y < 0.0);
        assert!(size.width >= 1024.0);
        assert!(size.height >= 720.0);
    }
}
