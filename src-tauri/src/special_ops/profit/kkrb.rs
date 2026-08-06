use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

const KKRB_ENDPOINT: &str = "https://www.kkrb.net/getTradeAmmoData";
pub(crate) const MAX_KKRB_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KkrbFailureKind {
    WholeSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KkrbSourceError {
    pub(crate) kind: KkrbFailureKind,
    pub(crate) message: String,
    business_code: Option<i64>,
}

impl KkrbSourceError {
    fn whole(message: impl Into<String>) -> Self {
        Self {
            kind: KkrbFailureKind::WholeSource,
            message: message.into(),
            business_code: None,
        }
    }

    fn business(code: i64, message: impl Into<String>) -> Self {
        Self {
            kind: KkrbFailureKind::WholeSource,
            message: message.into(),
            business_code: Some(code),
        }
    }

    pub(crate) fn is_catalog_busy(&self) -> bool {
        self.business_code == Some(-101)
    }
}

impl std::fmt::Display for KkrbSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for KkrbSourceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KkrbProfit {
    Value(i64),
    RuleError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KkrbSnapshot {
    by_name: HashMap<String, KkrbProfit>,
    pub(crate) source_version: Option<String>,
    pub(crate) source_data_at: Option<String>,
}

impl KkrbSnapshot {
    pub(crate) fn exact_profit(&self, exact_name: &str) -> Result<Option<i64>, String> {
        match self.by_name.get(exact_name) {
            Some(KkrbProfit::Value(profit)) => Ok(Some(*profit)),
            Some(KkrbProfit::RuleError(message)) => Err(message.clone()),
            None => Ok(None),
        }
    }

    pub(crate) fn catalog(&self) -> ProfitCatalogSnapshot {
        ProfitCatalogSnapshot {
            names: self
                .by_name
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            source_version: self.source_version.clone(),
            source_data_at: self.source_data_at.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfitCatalogSnapshot {
    pub names: Vec<String>,
    pub source_version: Option<String>,
    pub source_data_at: Option<String>,
}

#[derive(Clone)]
pub(crate) struct KkrbAdapter {
    client: reqwest::Client,
    endpoint: String,
}

impl KkrbAdapter {
    pub(crate) fn new() -> Result<Self, String> {
        Self::for_endpoint(KKRB_ENDPOINT.to_string())
    }

    fn for_endpoint(endpoint: String) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .user_agent("DeltaAutoTools/SpecialOpsProfit")
            .build()
            .map_err(|error| format!("创建 KKRB 客户端失败：{error}"))?;
        Ok(Self { client, endpoint })
    }

    pub(crate) async fn fetch(&self) -> Result<KkrbSnapshot, KkrbSourceError> {
        let mut response = self
            .client
            .post(&self.endpoint)
            .send()
            .await
            .map_err(|error| KkrbSourceError::whole(format!("KKRB 请求失败：{error}")))?;
        let status = response.status().as_u16();
        if response
            .content_length()
            .is_some_and(|length| length > MAX_KKRB_BODY_BYTES as u64)
        {
            return Err(KkrbSourceError::whole("KKRB 响应体超过大小限制"));
        }

        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| KkrbSourceError::whole(format!("读取 KKRB 响应失败：{error}")))?
        {
            if body.len().saturating_add(chunk.len()) > MAX_KKRB_BODY_BYTES {
                return Err(KkrbSourceError::whole("KKRB 响应体超过大小限制"));
            }
            body.extend_from_slice(&chunk);
        }
        parse_kkrb_http_response(status, &body)
    }

    pub(crate) async fn fetch_catalog_with_busy_retry(
        &self,
    ) -> Result<ProfitCatalogSnapshot, KkrbSourceError> {
        for attempt in 0..3 {
            match self.fetch().await {
                Ok(snapshot) => return Ok(snapshot.catalog()),
                Err(error) if error.is_catalog_busy() && attempt < 2 => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Err(error) if error.is_catalog_busy() => {
                    return Err(KkrbSourceError::whole("KKRB 暂时繁忙，名称列表未更新"));
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("固定次数重试必须在循环内返回")
    }
}

pub(crate) fn parse_kkrb_http_response(
    status: u16,
    body: &[u8],
) -> Result<KkrbSnapshot, KkrbSourceError> {
    if !(200..300).contains(&status) {
        return Err(KkrbSourceError::whole(format!(
            "KKRB HTTP 状态异常：{status}"
        )));
    }
    if body.len() > MAX_KKRB_BODY_BYTES {
        return Err(KkrbSourceError::whole("KKRB 响应体超过大小限制"));
    }
    parse_kkrb_response(body)
}

pub(crate) fn parse_kkrb_response(body: &[u8]) -> Result<KkrbSnapshot, KkrbSourceError> {
    let root: Value = serde_json::from_slice(body)
        .map_err(|error| KkrbSourceError::whole(format!("KKRB JSON 无效：{error}")))?;
    let root = root
        .as_object()
        .ok_or_else(|| KkrbSourceError::whole("KKRB 根数据不是对象"))?;
    let code = root
        .get("code")
        .and_then(Value::as_i64)
        .ok_or_else(|| KkrbSourceError::whole("KKRB 响应缺少有效 code"))?;
    if code != 0 {
        let message = root
            .get("msg")
            .and_then(Value::as_str)
            .unwrap_or("未提供错误信息");
        return Err(KkrbSourceError::business(
            code,
            format!("KKRB 返回失败（code {code}）：{message}"),
        ));
    }

    let rows = root
        .get("data")
        .and_then(Value::as_object)
        .and_then(|data| data.get("cn"))
        .and_then(Value::as_array)
        .ok_or_else(|| KkrbSourceError::whole("KKRB 响应缺少 data.cn 数组"))?;
    let source_version = optional_string(root.get("version"), "version")?;
    let mut by_name = HashMap::with_capacity(rows.len());

    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| KkrbSourceError::whole("KKRB data.cn 包含非对象条目"))?;
        let name = row
            .get("itemName")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| KkrbSourceError::whole("KKRB 条目缺少有效 itemName"))?
            .to_string();
        let profit = row.get("profit").and_then(Value::as_i64).map_or_else(
            || KkrbProfit::RuleError(format!("KKRB 目标“{name}”利润不是有效整数")),
            KkrbProfit::Value,
        );
        match by_name.entry(name.clone()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(profit);
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.insert(KkrbProfit::RuleError(format!("KKRB 精确名称重复：{name}")));
            }
        }
    }

    Ok(KkrbSnapshot {
        by_name,
        source_version,
        source_data_at: None,
    })
}

fn optional_string(value: Option<&Value>, field: &str) -> Result<Option<String>, KkrbSourceError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(KkrbSourceError::whole(format!("KKRB {field} 字段类型无效"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn parses_unique_exact_name_and_profit() {
        let snapshot = parse_kkrb_response(
            r#"{"code":0,"version":"v1","data":{"cn":[
                {"itemName":"5.45x39mm BT","profit":270458},
                {"itemName":"其他子弹","profit":10}
            ]}}"#
                .as_bytes(),
        )
        .unwrap();

        assert_eq!(snapshot.exact_profit("5.45x39mm BT").unwrap(), Some(270458));
        assert_eq!(snapshot.source_version.as_deref(), Some("v1"));
    }

    #[test]
    fn busy_response_is_whole_source_failure() {
        let error = parse_kkrb_response(r#"{"code":-101,"msg":"系统繁忙，请稍后再试"}"#.as_bytes())
            .unwrap_err();

        assert_eq!(error.kind, KkrbFailureKind::WholeSource);
        assert!(error.is_catalog_busy());
        assert!(error.message.contains("系统繁忙"));
    }

    #[test]
    fn non_busy_business_response_is_not_retryable_for_catalog() {
        let error =
            parse_kkrb_response(r#"{"code":-102,"msg":"请求参数错误"}"#.as_bytes()).unwrap_err();

        assert!(!error.is_catalog_busy());
    }

    #[test]
    fn invalid_root_http_status_and_oversized_body_are_whole_failures() {
        assert!(parse_kkrb_response(br#"{"code":0,"data":{}}"#).is_err());
        assert_eq!(
            parse_kkrb_http_response(503, br#"{"code":0}"#)
                .unwrap_err()
                .kind,
            KkrbFailureKind::WholeSource
        );
        let oversized = vec![b' '; MAX_KKRB_BODY_BYTES + 1];
        assert_eq!(
            parse_kkrb_http_response(200, &oversized).unwrap_err().kind,
            KkrbFailureKind::WholeSource
        );
    }

    #[test]
    fn target_missing_is_not_a_whole_source_failure() {
        let snapshot = parse_kkrb_response(
            r#"{"code":0,"data":{"cn":[{"itemName":"目标 A","profit":1}]}}"#.as_bytes(),
        )
        .unwrap();

        assert_eq!(snapshot.exact_profit("目标 B").unwrap(), None);
    }

    #[test]
    fn duplicate_exact_name_is_a_rule_error_not_a_whole_failure() {
        let snapshot = parse_kkrb_response(
            r#"{"code":0,"data":{"cn":[
                {"itemName":"目标 A","profit":1},
                {"itemName":"目标 A","profit":2}
            ]}}"#
                .as_bytes(),
        )
        .unwrap();

        assert!(snapshot
            .exact_profit("目标 A")
            .unwrap_err()
            .contains("重复"));
    }

    #[test]
    fn invalid_profit_is_a_rule_error_and_does_not_hide_other_targets() {
        for invalid in [
            serde_json::json!("1"),
            serde_json::json!(1.5),
            serde_json::json!(18_446_744_073_709_551_615_u64),
        ] {
            let body = serde_json::json!({
                "code": 0,
                "data": {"cn": [
                    {"itemName": "坏目标", "profit": invalid},
                    {"itemName": "好目标", "profit": 7}
                ]}
            });
            let snapshot = parse_kkrb_response(&serde_json::to_vec(&body).unwrap()).unwrap();
            assert!(snapshot.exact_profit("坏目标").is_err());
            assert_eq!(snapshot.exact_profit("好目标").unwrap(), Some(7));
        }
    }

    #[test]
    fn catalog_sorts_and_deduplicates_names() {
        let snapshot = parse_kkrb_response(
            r#"{"code":0,"version":"v2","data":{"cn":[
                {"itemName":"目标 B","profit":2},
                {"itemName":"目标 A","profit":1},
                {"itemName":"目标 B","profit":3}
            ]}}"#
                .as_bytes(),
        )
        .unwrap();

        assert_eq!(
            snapshot.catalog().names,
            ["目标 A".to_string(), "目标 B".to_string()]
        );
    }

    #[tokio::test]
    async fn adapter_posts_once_and_parses_response() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let request_size = stream.read(&mut request).unwrap();
            assert!(std::str::from_utf8(&request[..request_size])
                .unwrap()
                .starts_with("POST /ammo HTTP/1.1"));

            let body = r#"{"code":0,"version":"test","data":{"cn":[
                {"itemName":"测试子弹","profit":42}
            ]}}"#
                .as_bytes();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });

        let adapter = KkrbAdapter::for_endpoint(format!("http://{address}/ammo")).unwrap();
        let snapshot = adapter.fetch().await.unwrap();

        server.join().unwrap();
        assert_eq!(snapshot.exact_profit("测试子弹").unwrap(), Some(42));
    }

    #[tokio::test]
    async fn catalog_refresh_retries_busy_response_until_success() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for body in [
                r#"{"code":-101,"msg":"系统繁忙，请稍后再试"}"#.as_bytes(),
                r#"{"code":-101,"msg":"系统繁忙，请稍后再试"}"#.as_bytes(),
                r#"{"code":0,"data":{"cn":[{"itemName":"重试后子弹","profit":42}]}}"#.as_bytes(),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 2048];
                let _bytes_read = stream.read(&mut request).unwrap();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(body).unwrap();
            }
        });

        let adapter = KkrbAdapter::for_endpoint(format!("http://{address}/ammo")).unwrap();
        let catalog = adapter.fetch_catalog_with_busy_retry().await.unwrap();

        server.join().unwrap();
        assert_eq!(catalog.names, ["重试后子弹".to_string()]);
    }

    #[tokio::test]
    async fn catalog_refresh_reports_stable_error_after_three_busy_responses() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 2048];
                let _bytes_read = stream.read(&mut request).unwrap();
                let body = r#"{"code":-101,"msg":"系统繁忙，请稍后再试"}"#.as_bytes();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(body).unwrap();
            }
        });

        let adapter = KkrbAdapter::for_endpoint(format!("http://{address}/ammo")).unwrap();
        let error = adapter.fetch_catalog_with_busy_retry().await.unwrap_err();

        server.join().unwrap();
        assert_eq!(error.to_string(), "KKRB 暂时繁忙，名称列表未更新");
    }
}
