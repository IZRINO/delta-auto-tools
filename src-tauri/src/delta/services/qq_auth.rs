use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{cookie::Jar, header::LOCATION, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::delta::{
    client::http::{build_client, HttpOptions},
    constants::{DF_REFERER, QQ_LOGIN_APP_ID, QQ_LOGIN_DAID, QQ_LOGIN_JUMP_URL, QQ_OAUTH_APP_ID, QQ_REDIRECT_URI},
    error::DeltaError,
    response::ApiResponse,
    utils::{
        cookies::{
            dump_cookie_json, dump_cookie_json_for_urls, insert_cookie, must_cookie,
            restore_cookie_json, restore_cookie_json_for_domain,
        },
        hashes::{get_gtk, get_qr_token},
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqStatusRequest {
    pub qr_token: String,
    pub qr_sig: String,
    pub login_sig: String,
    pub cookie: String,
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

#[derive(Debug, Clone)]
pub struct QqAuthService {
    pub(crate) client: Client,
    pub(crate) jar: Arc<Jar>,
    poll_jump_url: String,
    login_third_party_aid: &'static str,
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
        let (client, jar) = build_client(options)?;
        Ok(Self {
            client,
            jar,
            poll_jump_url,
            login_third_party_aid,
            oauth_client_id,
            oauth_redirect_uri,
        })
    }

    pub async fn get_login_qr(&self) -> Result<QqLoginQr, DeltaError> {
        self.client
            .get("https://xui.ptlogin2.qq.com/cgi-bin/xlogin")
            .query(&[
                ("appid", QQ_LOGIN_APP_ID),
                ("daid", QQ_LOGIN_DAID),
                ("style", "33"),
                ("login_text", "\u{767b}\u{5f55}"),
                ("hide_title_bar", "1"),
                ("hide_border", "1"),
                ("target", "self"),
                ("s_url", QQ_LOGIN_JUMP_URL),
                ("pt_3rd_aid", self.login_third_party_aid),
                ("pt_feedback_link", "https://support.qq.com/products/77942?customInfo=milo.qq.com.appid101491592"),
                ("theme", "2"),
                ("verify_theme", ""),
            ])
            .send()
            .await?
            .error_for_status()?;

        let image = self
            .client
            .get("https://xui.ptlogin2.qq.com/ssl/ptqrshow")
            .query(&[
                ("appid", QQ_LOGIN_APP_ID),
                ("e", "2"),
                ("l", "M"),
                ("s", "3"),
                ("d", "72"),
                ("v", "4"),
                ("t", "0.6142752744667854"),
                ("daid", QQ_LOGIN_DAID),
                ("pt_3rd_aid", self.login_third_party_aid),
                ("u1", self.poll_jump_url.as_str()),
            ])
            .send()
            .await?
            .bytes()
            .await?;

        let qr_sig = must_cookie(&self.jar, "https://xui.ptlogin2.qq.com/", "qrsig")?;
        let login_sig = must_cookie(&self.jar, "https://xui.ptlogin2.qq.com/", "pt_login_sig")?;

        Ok(QqLoginQr {
            qr_sig: qr_sig.clone(),
            image: STANDARD.encode(image),
            token: get_qr_token(&qr_sig),
            login_sig,
            cookie: dump_cookie_json(&self.jar, "https://xui.ptlogin2.qq.com/")?,
        })
    }

    pub async fn poll_login_status(&self, req: QqStatusRequest) -> Result<ApiResponse<Value>, DeltaError> {
        restore_cookie_json(&self.jar, "https://ssl.ptlogin2.qq.com/", &req.cookie)?;
        insert_cookie(&self.jar, "https://ssl.ptlogin2.qq.com/", "qrsig", &req.qr_sig)?;

        let body = self
            .client
            .get("https://ssl.ptlogin2.qq.com/ptqrlogin")
            .query(&[
                ("u1", self.poll_jump_url.as_str()),
                ("ptqrtoken", req.qr_token.as_str()),
                ("ptredirect", "0"),
                ("h", "1"),
                ("t", "1"),
                ("g", "1"),
                ("from_ui", "1"),
                ("ptlang", "2052"),
                ("action", &format!("0-0-{}", current_millis())),
                ("js_ver", "25040111"),
                ("js_type", "1"),
                ("login_sig", req.login_sig.as_str()),
                ("pt_uistyle", "40"),
                ("aid", QQ_LOGIN_APP_ID),
                ("daid", QQ_LOGIN_DAID),
                ("pt_3rd_aid", self.login_third_party_aid),
                ("o1vId", "378b06c889d9113b39e814ca627809e3"),
                ("pt_js_version", "530c3f68"),
            ])
            .send()
            .await?
            .text()
            .await?;

        Self::map_poll_body(&body, async |redirect_url| {
            let _ = self.client.get(&redirect_url).send().await?;
            let cookie = dump_cookie_json_for_urls(&self.jar, QQ_LOGIN_COOKIE_URLS)?;
            Ok(json!({ "cookie": cookie, "uin": extract_query_param(&redirect_url, "uin").ok() }))
        })
        .await
    }

    pub async fn get_access_token(&self, cookie_json: &str) -> Result<QqAccessToken, DeltaError> {
        restore_cookie_json_for_domain(&self.jar, "https://graph.qq.com/", QQ_COOKIE_DOMAIN, cookie_json)?;
        let p_skey = must_cookie(&self.jar, "https://graph.qq.com/", "p_skey")?;
        let gtk = get_gtk(&p_skey).to_string();

        let auth_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();

        let resp = self
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
        let _ = self.client.get(&location).send().await?;

        let now = current_millis().to_string();
        let body = self
            .client
            .get("https://ams.game.qq.com/ams/userLoginSvr")
            .query(&[
                ("a", "qcCodeToOpenId"),
                ("qc_code", code.as_str()),
                ("appid", QQ_OAUTH_APP_ID),
                ("redirect_uri", "https://milo.qq.com/comm-htdocs/login/qc_redirect.html"),
                ("callback", "coolxitech"),
                ("_", now.as_str()),
            ])
            .header(reqwest::header::REFERER, DF_REFERER)
            .send()
            .await?
            .text()
            .await?;
        let args = extract_jsonp_args(&body, "coolxitech")?;
        let payload: Value = serde_json::from_str(args.first().map(String::as_str).unwrap_or("{}"))?;
        if payload["iRet"].as_i64().unwrap_or(-1) != 0 {
            return Err(DeltaError::Parse(qq_access_token_error_message(&payload)));
        }

        Ok(QqAccessToken {
            access_token: payload["access_token"].as_str().unwrap_or_default().to_string(),
            openid: payload["openid"].as_str().unwrap_or_default().to_string(),
            expires_in: payload["expires_in"].as_i64().unwrap_or_default(),
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
            restore_cookie_json(&self.jar, "https://ssl.ptlogin2.qq.com/", cookie_json)?;
        }

        let now = current_millis().to_string();
        let body = self
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
        Ok(body.contains("\"isLogin\":1"))
    }

    pub async fn map_poll_body<F, Fut>(body: &str, on_success: F) -> Result<ApiResponse<Value>, DeltaError>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<Value, DeltaError>>,
    {
        let args = extract_jsonp_args(body, "ptuiCB")?;
        match args.first().map(String::as_str) {
            Some("0") => {
                let redirect_url = args.get(2).cloned().unwrap_or_default();
                Ok(ApiResponse::ok("登录成功", on_success(redirect_url).await?))
            }
            Some("66") => Ok(ApiResponse { code: 1, msg: "等待扫码".to_string(), data: json!([]) }),
            Some("67") => Ok(ApiResponse { code: 2, msg: "已扫码,待确认".to_string(), data: json!([]) }),
            Some("65") => Ok(ApiResponse { code: -2, msg: "二维码失效".to_string(), data: json!([]) }),
            Some("86") => Ok(ApiResponse { code: -3, msg: "登录被拒绝".to_string(), data: json!([]) }),
            _ => Ok(ApiResponse { code: -4, msg: "未知错误信息".to_string(), data: json!([]) }),
        }
    }
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

    use super::{qq_access_token_error_message, QqAuthService};

    #[tokio::test]
    async fn maps_waiting_status() {
        let response = QqAuthService::map_poll_body("ptuiCB('66','0','','0','msg','')", |_| async {
            Ok(json!({}))
        })
        .await
        .unwrap();
        assert_eq!(response.code, 1);
    }

    #[tokio::test]
    async fn maps_success_status() {
        let response = QqAuthService::map_poll_body(
            "ptuiCB('0','0','https://qq.com/callback?uin=10001','0','ok','')",
            |redirect| async move { Ok(json!({ "redirect": redirect })) },
        )
        .await
        .unwrap();
        assert_eq!(response.code, 0);
        assert_eq!(response.data["redirect"], "https://qq.com/callback?uin=10001");
    }

    #[tokio::test]
    async fn maps_scanned_awaiting_confirm_status() {
        let response = QqAuthService::map_poll_body("ptuiCB('67','0','','0','msg','')", |_| async {
            Ok(json!({}))
        })
        .await
        .unwrap();
        assert_eq!(response.code, 2);
        assert_eq!(response.msg, "已扫码,待确认");
    }

    #[tokio::test]
    async fn maps_qr_expired_status() {
        let response = QqAuthService::map_poll_body("ptuiCB('65','0','','0','msg','')", |_| async {
            Ok(json!({}))
        })
        .await
        .unwrap();
        assert_eq!(response.code, -2);
        assert_eq!(response.msg, "二维码失效");
    }

    #[tokio::test]
    async fn maps_login_rejected_status() {
        let response = QqAuthService::map_poll_body("ptuiCB('86','0','','0','msg','')", |_| async {
            Ok(json!({}))
        })
        .await
        .unwrap();
        assert_eq!(response.code, -3);
        assert_eq!(response.msg, "登录被拒绝");
    }

    #[tokio::test]
    async fn maps_unknown_status() {
        let response = QqAuthService::map_poll_body("ptuiCB('99','0','','0','msg','')", |_| async {
            Ok(json!({}))
        })
        .await
        .unwrap();
        assert_eq!(response.code, -4);
        assert_eq!(response.msg, "未知错误信息");
    }

    #[tokio::test]
    async fn maps_empty_callback() {
        let result = QqAuthService::map_poll_body("ptuiCB()", |_| async {
            Ok(json!({}))
        })
        .await;
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
}
