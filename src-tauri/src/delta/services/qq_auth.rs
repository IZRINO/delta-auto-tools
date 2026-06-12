use std::sync::Arc;

use reqwest::{cookie::Jar, header::LOCATION, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::delta::{
    client::http::HttpOptions,
    constants::{
        DF_REFERER, QQ_LOGIN_APP_ID, QQ_LOGIN_DAID, QQ_LOGIN_JUMP_URL, QQ_OAUTH_APP_ID,
        QQ_REDIRECT_URI,
    },
    error::DeltaError,
    response::ApiResponse,
    services::qq_qr_pipeline::{QqQrAuthPipeline, QrAuthConfig, QrLoginResult, QrStatusRequest},
    utils::{
        cookies::{
            dump_cookie_json_for_urls, must_cookie,
            restore_cookie_json, restore_cookie_json_for_domain,
        },
        hashes::get_gtk,
        html::{extract_query_param, extract_raw_query_param},
        jsonp::extract_jsonp_args,
        time::current_millis,
    },
};

const QQ_COOKIE_DOMAIN: &str = ".qq.com";
const QQ_LOGIN_COOKIE_URLS: &[&str] = &[
    "https://xui.ptlogin2.qq.com/",
    "https://ssl.ptlogin2.qq.com/",
    "https://graph.qq.com/",
    "https://ptlogin2.qq.com/",
    "https://qq.com/",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqLoginQr {
    pub qr_sig: String,
    pub image: String,
    pub token: i64,
    pub login_sig: String,
    pub cookie: String,
}

impl From<QrLoginResult> for QqLoginQr {
    fn from(r: QrLoginResult) -> Self {
        Self {
            qr_sig: r.qr_sig,
            image: r.image,
            token: r.token,
            login_sig: r.login_sig,
            cookie: r.cookie,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqStatusRequest {
    pub qr_token: String,
    pub qr_sig: String,
    pub login_sig: String,
    pub cookie: String,
}

impl From<QqStatusRequest> for QrStatusRequest {
    fn from(r: QqStatusRequest) -> Self {
        Self {
            qr_token: r.qr_token,
            qr_sig: r.qr_sig,
            login_sig: r.login_sig,
            cookie: r.cookie,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqAccessToken {
    pub access_token: String,
    pub openid: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTokenOnlyRequest {
    pub openid: String,
    pub access_token: String,
}

pub(crate) fn qq_auth_config(third_aid: &'static str, jump_url: &'static str) -> QrAuthConfig {
    QrAuthConfig {
        appid: QQ_LOGIN_APP_ID,
        daid: QQ_LOGIN_DAID,
        jump_url,
        third_aid: Some(third_aid),
        style: "33",
        poll_url: "https://ssl.ptlogin2.qq.com/ptqrlogin",
        cookie_restore_domain: "https://ssl.ptlogin2.qq.com/",
        js_ver: "25040111",
        o1v_id: "378b06c889d9113b39e814ca627809e3",
        pt_js_version: "530c3f68",
        xlogin_extra: vec![
            ("login_text", "\u{767b}\u{5f55}"),
            ("hide_title_bar", "1"),
            (
                "pt_feedback_link",
                "https://support.qq.com/products/77942?customInfo=milo.qq.com.appid101491592",
            ),
            ("theme", "2"),
            ("verify_theme", ""),
        ],
        poll_referer: None,
        waiting_msg: "等待扫码",
        scanned_msg: "已扫码,待确认",
        unknown_msg: "未知错误信息",
    }
}

#[derive(Debug, Clone)]
pub struct QqAuthService {
    pipeline: QqQrAuthPipeline,
    oauth_client_id: &'static str,
    oauth_redirect_uri: &'static str,
}

impl QqAuthService {
    pub fn new(options: HttpOptions) -> Result<Self, DeltaError> {
        Self::with_config(
            options,
            QQ_LOGIN_JUMP_URL.to_string(),
            QQ_OAUTH_APP_ID,
            QQ_REDIRECT_URI,
            QQ_OAUTH_APP_ID,
        )
    }

    pub fn with_config(
        options: HttpOptions,
        poll_jump_url: String,
        login_third_party_aid: &'static str,
        oauth_redirect_uri: &'static str,
        oauth_client_id: &'static str,
    ) -> Result<Self, DeltaError> {
        // 使用 leak 将 String 转为 &'static str 供 QrAuthConfig
        let jump_url: &'static str = Box::leak(poll_jump_url.into_boxed_str());
        let config = qq_auth_config(login_third_party_aid, jump_url);
        let pipeline = QqQrAuthPipeline::new(options, config)?;
        Ok(Self {
            pipeline,
            oauth_client_id,
            oauth_redirect_uri,
        })
    }

    pub fn client(&self) -> &Client {
        &self.pipeline.client
    }

    pub fn jar(&self) -> &Arc<Jar> {
        &self.pipeline.jar
    }

    pub async fn get_login_qr(&self) -> Result<QqLoginQr, DeltaError> {
        let result = self.pipeline.get_login_qr().await?;
        Ok(result.into())
    }

    pub async fn poll_login_status(
        &self,
        req: QqStatusRequest,
    ) -> Result<ApiResponse<Value>, DeltaError> {
        let jar = self.pipeline.jar.clone();
        let client = self.pipeline.client.clone();
        let req_clone = req.clone();
        self.pipeline
            .poll_login_status(req.into(), async |redirect_url| {
                let _ = client.get(&redirect_url).send().await?;
                let cookie = dump_cookie_json_for_urls(&jar, QQ_LOGIN_COOKIE_URLS)?;
                Ok(json!({ "cookie": cookie, "sessionKey": extract_query_param(&redirect_url, "uin").unwrap_or_else(|_| req_clone.qr_sig.clone()), "uin": extract_query_param(&redirect_url, "uin").ok() }))
            })
            .await
    }

    pub async fn get_access_token(&self, cookie_json: &str) -> Result<QqAccessToken, DeltaError> {
        restore_cookie_json_for_domain(
            &self.pipeline.jar,
            "https://graph.qq.com/",
            QQ_COOKIE_DOMAIN,
            cookie_json,
        )?;
        let p_skey = must_cookie(&self.pipeline.jar, "https://graph.qq.com/", "p_skey")?;
        let gtk = get_gtk(&p_skey).to_string();

        let auth_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();

        let resp = self
            .pipeline
            .client
            .post("https://graph.qq.com/oauth2.0/authorize")
            .form(&[
                ("response_type", "code"),
                ("client_id", self.oauth_client_id),
                ("redirect_uri", self.oauth_redirect_uri),
                ("scope", ""),
                ("state", "STATE"),
                ("switch", ""),
                ("form_plogin", "1"),
                ("src", "1"),
                ("update_auth", "1"),
                ("openapi", "1010"),
                ("g_tk", gtk.as_str()),
                ("auth_time", auth_time.as_str()),
                ("ui", "979D48F3-6CE2-4E95-A789-3BD3187648B6"),
            ])
            .header(reqwest::header::REFERER, "https://xui.ptlogin2.qq.com/")
            .send()
            .await?;

        let location = resp
            .headers()
            .get(LOCATION)
            .ok_or_else(|| DeltaError::Parse("missing location".to_string()))?
            .to_str()
            .map_err(|error| DeltaError::Parse(error.to_string()))?
            .to_string();
        let code = extract_raw_query_param(&location, "code")?;
        let _ = self.pipeline.client.get(&location).send().await?;

        let now = current_millis().to_string();
        let body = self
            .pipeline
            .client
            .get("https://ams.game.qq.com/ams/userLoginSvr")
            .query(&[
                ("a", "qcCodeToOpenId"),
                ("qc_code", code.as_str()),
                ("appid", QQ_OAUTH_APP_ID),
                (
                    "redirect_uri",
                    "https://milo.qq.com/comm-htdocs/login/qc_redirect.html",
                ),
                ("callback", "coolxitech"),
                ("_", now.as_str()),
            ])
            .header(reqwest::header::REFERER, DF_REFERER)
            .send()
            .await?
            .text()
            .await?;
        let args = extract_jsonp_args(&body, "coolxitech")?;
        let payload: Value =
            serde_json::from_str(args.first().map(String::as_str).unwrap_or("{}"))?;
        if payload["iRet"].as_i64().unwrap_or(-1) != 0 {
            return Err(DeltaError::Parse(qq_access_token_error_message(&payload)));
        }

        let access_token = payload["access_token"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| DeltaError::Parse("AccessToken获取失败: 缺少 access_token".to_string()))?
            .to_string();
        let openid = payload["openid"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| DeltaError::Parse("AccessToken获取失败: 缺少 openid".to_string()))?
            .to_string();
        let expires_in = payload["expires_in"]
            .as_i64()
            .filter(|value| *value > 0)
            .ok_or_else(|| DeltaError::Parse("AccessToken获取失败: 缺少有效期".to_string()))?;

        Ok(QqAccessToken {
            access_token,
            openid,
            expires_in,
        })
    }

    pub async fn update_access_token(
        &self,
        req: &UpdateTokenOnlyRequest,
        cookie_json: Option<&str>,
        acctype: &str,
        appid: &str,
    ) -> Result<bool, DeltaError> {
        if let Some(cookie_json) = cookie_json {
            restore_cookie_json(
                &self.pipeline.jar,
                "https://ssl.ptlogin2.qq.com/",
                cookie_json,
            )?;
        }

        let now = current_millis().to_string();
        let body = self
            .pipeline
            .client
            .post("https://ams.game.qq.com/ams/userLoginSvr")
            .query(&[
                ("callback", "coolxitech"),
                ("acctype", acctype),
                ("appid", appid),
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
        parse_login_valid(&body)
    }
}

pub(crate) fn parse_login_valid(body: &str) -> Result<bool, DeltaError> {
    let args = extract_jsonp_args(body, "coolxitech")?;
    let payload: Value = serde_json::from_str(args.first().map(String::as_str).unwrap_or("{}"))?;
    Ok(payload["isLogin"].as_i64() == Some(1))
}

fn qq_access_token_error_message(payload: &Value) -> String {
    let ret = payload["iRet"].as_i64().unwrap_or(-1);
    let message = payload["sMsg"]
        .as_str()
        .or_else(|| payload["msg"].as_str())
        .or_else(|| payload["message"].as_str())
        .unwrap_or("AccessToken获取失败");
    format!("AccessToken获取失败: iRet={ret}, {message}")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse_login_valid, qq_access_token_error_message, qq_auth_config};
    use crate::delta::services::qq_qr_pipeline::QqQrAuthPipeline;

    fn test_config() -> crate::delta::services::qq_qr_pipeline::QrAuthConfig {
        qq_auth_config("101491592", "https://graph.qq.com/oauth2.0/login_jump")
    }

    #[tokio::test]
    async fn maps_waiting_status() {
        let response =
            QqQrAuthPipeline::map_poll_body(&test_config(), "ptuiCB('66','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();
        assert_eq!(response.code, 1);
    }

    #[tokio::test]
    async fn maps_success_status() {
        let response = QqQrAuthPipeline::map_poll_body(
            &test_config(),
            "ptuiCB('0','0','https://qq.com/callback?uin=10001','0','ok','')",
            |redirect| async move { Ok(json!({ "redirect": redirect })) },
        )
        .await
        .unwrap();
        assert_eq!(response.code, 0);
        assert_eq!(
            response.data["redirect"],
            "https://qq.com/callback?uin=10001"
        );
    }

    #[tokio::test]
    async fn maps_scanned_awaiting_confirm_status() {
        let response =
            QqQrAuthPipeline::map_poll_body(&test_config(), "ptuiCB('67','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();
        assert_eq!(response.code, 2);
        assert_eq!(response.msg, "已扫码,待确认");
    }

    #[tokio::test]
    async fn maps_qr_expired_status() {
        let response =
            QqQrAuthPipeline::map_poll_body(&test_config(), "ptuiCB('65','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();
        assert_eq!(response.code, -2);
        assert_eq!(response.msg, "二维码失效");
    }

    #[tokio::test]
    async fn maps_login_rejected_status() {
        let response =
            QqQrAuthPipeline::map_poll_body(&test_config(), "ptuiCB('86','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();
        assert_eq!(response.code, -3);
        assert_eq!(response.msg, "登录被拒绝");
    }

    #[tokio::test]
    async fn maps_unknown_status() {
        let response =
            QqQrAuthPipeline::map_poll_body(&test_config(), "ptuiCB('99','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();
        assert_eq!(response.code, -4);
        assert_eq!(response.msg, "未知错误信息");
    }

    #[tokio::test]
    async fn maps_empty_callback() {
        let result = QqQrAuthPipeline::map_poll_body(&test_config(), "ptuiCB()", |_| async { Ok(json!({})) }).await;
        assert!(result.is_err() || result.unwrap().code == -4);
    }

    #[test]
    fn formats_access_token_failure_detail() {
        let message = qq_access_token_error_message(&json!({
            "iRet": -1,
            "sMsg": "登录态无效"
        }));

        assert_eq!(message, "AccessToken获取失败: iRet=-1, 登录态无效");
    }

    #[test]
    fn parses_refresh_login_state_structurally() {
        assert!(parse_login_valid(r#"coolxitech({"isLogin":1})"#).unwrap());
        assert!(!parse_login_valid(r#"coolxitech({"isLogin":0,"message":"失效"})"#).unwrap());
    }
}
