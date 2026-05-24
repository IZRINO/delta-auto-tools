use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::delta::{
    client::http::{build_client, HttpOptions},
    constants::{DF_REFERER, WECHAT_APP_ID, WECHAT_CODE_DOMAIN, WECHAT_ORIGINAL_URL, WECHAT_REDIRECT_URI},
    error::DeltaError,
    response::ApiResponse,
    services::qq_auth::UpdateTokenOnlyRequest,
    utils::{html::{extract_wx_code, extract_wx_errcode, extract_wx_qrcode_uuid}, time::current_millis},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WechatQr {
    pub qr_code: String,
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WechatAccessToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub openid: String,
    pub unionid: Option<String>,
    pub expires_in: i64,
}

#[derive(Debug, Clone)]
pub struct WechatAuthService {
    client: reqwest::Client,
}

impl WechatAuthService {
    pub fn new(options: HttpOptions) -> Result<Self, DeltaError> {
        let (client, _) = build_client(options)?;
        Ok(Self { client })
    }

    pub async fn get_login_qr(&self) -> Result<WechatQr, DeltaError> {
        let ts = current_millis().to_string();
        let html = self
            .client
            .get("https://open.weixin.qq.com/connect/qrconnect")
            .query(&[
                ("appid", WECHAT_APP_ID),
                ("scope", "snsapi_login"),
                ("redirect_uri", WECHAT_REDIRECT_URI),
                ("state", "1"),
                ("login_type", "jssdk"),
                ("self_redirect", "true"),
                ("ts", &ts),
                ("style", "black"),
            ])
            .header(reqwest::header::REFERER, DF_REFERER)
            .send()
            .await?
            .text()
            .await?;
        let uuid = extract_wx_qrcode_uuid(&html)?;
        Ok(WechatQr {
            qr_code: format!("https://open.weixin.qq.com/connect/qrcode/{uuid}"),
            uuid,
        })
    }

    pub async fn poll_status(&self, uuid: &str) -> Result<ApiResponse<Value>, DeltaError> {
        let body = self
            .client
            .get("https://lp.open.weixin.qq.com/connect/l/qrconnect")
            .query(&[("uuid", uuid)])
            .send()
            .await?
            .text()
            .await?;
        Self::map_status_body(&body)
    }

    pub fn map_status_body(body: &str) -> Result<ApiResponse<Value>, DeltaError> {
        let errcode = extract_wx_errcode(body)?;
        let wx_code = extract_wx_code(body).unwrap_or_default();
        Ok(match errcode {
            405 => ApiResponse { code: 3, msg: "扫码成功".to_string(), data: json!({ "wxErrcode": 405, "wxCode": wx_code }) },
            404 => ApiResponse { code: 2, msg: "已扫码".to_string(), data: json!([]) },
            408 => ApiResponse { code: 1, msg: "等待扫码".to_string(), data: json!([]) },
            402 => ApiResponse { code: -2, msg: "二维码超时".to_string(), data: json!([]) },
            403 => ApiResponse { code: -3, msg: "扫码被拒绝".to_string(), data: json!([]) },
            _ => ApiResponse { code: -4, msg: "其他错误代码".to_string(), data: json!({ "wxErrcode": errcode, "wxCode": wx_code }) },
        })
    }

    pub async fn get_access_token(&self, code: &str) -> Result<WechatAccessToken, DeltaError> {
        let now = current_millis().to_string();
        let raw: Value = self
            .client
            .get("https://apps.game.qq.com/ams/ame/codeToOpenId.php")
            .query(&[
                ("callback", ""),
                ("appid", WECHAT_APP_ID),
                ("wxcode", code),
                ("originalUrl", WECHAT_ORIGINAL_URL),
                ("wxcodedomain", WECHAT_CODE_DOMAIN),
                ("acctype", "wx"),
                ("sServiceType", "undefined"),
                ("_", now.as_str()),
            ])
            .header(reqwest::header::REFERER, DF_REFERER)
            .send()
            .await?
            .json()
            .await?;

        let nested = raw["sMsg"]
            .as_str()
            .ok_or_else(|| DeltaError::Parse("missing sMsg".to_string()))?;
        serde_json::from_str(nested).map_err(Into::into)
    }

    pub async fn update_access_token(&self, req: &UpdateTokenOnlyRequest) -> Result<bool, DeltaError> {
        let now = current_millis().to_string();
        let body = self
            .client
            .post("https://ams.game.qq.com/ams/userLoginSvr")
            .query(&[
                ("callback", "coolxitech"),
                ("acctype", "wx"),
                ("appid", WECHAT_APP_ID),
                ("access_token", req.access_token.as_str()),
                ("openid", req.openid.as_str()),
                ("refresh_token", ""),
                ("ieg_ams_sign", "null"),
                ("expires_time", "null"),
                ("_", now.as_str()),
            ])
            .send()
            .await?
            .text()
            .await?;
        Ok(body.contains("\"isLogin\":1"))
    }
}

#[cfg(test)]
mod tests {
    use super::WechatAuthService;

    #[test]
    fn maps_wechat_success_status() {
        let response = WechatAuthService::map_status_body("window.wx_errcode=405;window.wx_code='abc';").unwrap();
        assert_eq!(response.code, 3);
        assert_eq!(response.data["wxCode"], "abc");
    }

    #[test]
    fn maps_wechat_waiting_status() {
        let response = WechatAuthService::map_status_body("window.wx_errcode=408;").unwrap();
        assert_eq!(response.code, 1);
    }

    #[test]
    fn maps_wechat_scanned_status() {
        let response = WechatAuthService::map_status_body("window.wx_errcode=404;").unwrap();
        assert_eq!(response.code, 2);
        assert_eq!(response.msg, "已扫码");
    }

    #[test]
    fn maps_wechat_timeout_status() {
        let response = WechatAuthService::map_status_body("window.wx_errcode=402;").unwrap();
        assert_eq!(response.code, -2);
        assert_eq!(response.msg, "二维码超时");
    }

    #[test]
    fn maps_wechat_rejected_status() {
        let response = WechatAuthService::map_status_body("window.wx_errcode=403;").unwrap();
        assert_eq!(response.code, -3);
        assert_eq!(response.msg, "扫码被拒绝");
    }

    #[test]
    fn maps_wechat_unknown_status() {
        let response = WechatAuthService::map_status_body("window.wx_errcode=500;").unwrap();
        assert_eq!(response.code, -4);
        assert_eq!(response.msg, "其他错误代码");
        assert_eq!(response.data["wxErrcode"], 500);
    }

    #[test]
    fn maps_wechat_success_with_empty_code() {
        let response = WechatAuthService::map_status_body("window.wx_errcode=405;").unwrap();
        assert_eq!(response.code, 3);
        assert_eq!(response.data["wxCode"], "");
    }
}
