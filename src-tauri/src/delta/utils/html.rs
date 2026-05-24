use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use regex::Regex;
use serde_json::Value;

use crate::delta::error::DeltaError;

pub fn extract_query_param(url: &str, key: &str) -> Result<String, DeltaError> {
    let parsed = url::Url::parse(url)?;
    parsed
        .query_pairs()
        .find_map(|(name, value)| (name == key).then(|| value.to_string()))
        .ok_or_else(|| DeltaError::Parse(format!("query param {key} not found")))
}

pub fn extract_raw_query_param(url: &str, key: &str) -> Result<String, DeltaError> {
    let query = url
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or("");
    for pair in query.split('&') {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        if name == key {
            return Ok(value.to_string());
        }
    }
    Err(DeltaError::Parse(format!("query param {key} not found")))
}

pub fn extract_wx_qrcode_uuid(html: &str) -> Result<String, DeltaError> {
    let re = Regex::new(r#"/connect/qrcode/(?P<uuid>[A-Za-z0-9]+)"#)
        .map_err(|error| DeltaError::Parse(error.to_string()))?;
    let captures = re
        .captures(html)
        .ok_or_else(|| DeltaError::Parse("wechat uuid not found".to_string()))?;
    Ok(captures["uuid"].to_string())
}

pub fn extract_wx_errcode(body: &str) -> Result<i32, DeltaError> {
    let re = Regex::new(r"wx_errcode=(?P<code>-?\d+)")
        .map_err(|error| DeltaError::Parse(error.to_string()))?;
    let captures = re
        .captures(body)
        .ok_or_else(|| DeltaError::Parse("wx_errcode not found".to_string()))?;
    captures["code"]
        .parse::<i32>()
        .map_err(|error| DeltaError::Parse(error.to_string()))
}

pub fn extract_wx_code(body: &str) -> Option<String> {
    let re = Regex::new(r#"wx_code='(?P<code>[^']+)'"#).ok()?;
    re.captures(body)
        .map(|captures| captures["code"].to_string())
}

pub fn decode_jwt_middle(value: &str) -> Result<Value, DeltaError> {
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() < 3 {
        return Err(DeltaError::Parse("invalid gs_code format".to_string()));
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|error| DeltaError::Parse(error.to_string()))?;
    serde_json::from_slice(&decoded).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{
        decode_jwt_middle, extract_query_param, extract_raw_query_param, extract_wx_code,
        extract_wx_errcode, extract_wx_qrcode_uuid,
    };

    #[test]
    fn extracts_query_param() {
        let code =
            extract_query_param("https://qq.com/callback?code=abc123&state=1", "code").unwrap();
        assert_eq!(code, "abc123");
    }

    #[test]
    fn extracts_raw_query_param_without_decoding() {
        let code =
            extract_raw_query_param("https://qq.com/callback?code=a%2Bb%252F&state=1", "code")
                .unwrap();
        assert_eq!(code, "a%2Bb%252F");
    }

    #[test]
    fn extracts_wechat_uuid() {
        let uuid = extract_wx_qrcode_uuid("<img src=\"/connect/qrcode/XYZ123\" />").unwrap();
        assert_eq!(uuid, "XYZ123");
    }

    #[test]
    fn extracts_wechat_status_fields() {
        let body = "window.wx_errcode=405; window.wx_code='code-123';";
        assert_eq!(extract_wx_errcode(body).unwrap(), 405);
        assert_eq!(extract_wx_code(body).unwrap(), "code-123");
    }

    #[test]
    fn decodes_jwt_middle_payload() {
        let payload = decode_jwt_middle("head.eyJ0b2tlbiI6ImFiYyIsImV4cCI6MTIzfQ.tail").unwrap();
        assert_eq!(payload["token"], "abc");
    }
}
