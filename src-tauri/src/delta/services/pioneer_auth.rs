use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{cookie::Jar, header::LOCATION, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::delta::{
    client::http::{build_client, HttpOptions},
    constants::{
        PIONEER_3RD_AID, PIONEER_APP_ID, PIONEER_CALLBACK, PIONEER_DAID, PIONEER_FEEDBACK_LINK,
        PIONEER_OAUTH_APP_ID, PIONEER_REDIRECT_URI,
    },
    error::DeltaError,
    response::ApiResponse,
    utils::{
        cookies::{dump_cookie_json, insert_cookie, must_cookie, restore_cookie_json},
        hashes::get_qr_token,
        html::extract_query_param,
        jsonp::extract_jsonp_args,
        time::current_millis,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PioneerLoginQr {
    pub qr_sig: String,
    pub image: String,
    pub token: i64,
    pub login_sig: String,
    pub cookie: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PioneerStatusRequest {
    pub qr_token: String,
    pub qr_sig: String,
    pub login_sig: String,
    pub cookie: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PioneerAccessToken {
    pub key: String,
}

#[derive(Debug, Clone)]
pub struct PioneerAuthService {
    client: Client,
    jar: Arc<Jar>,
}

impl PioneerAuthService {
    pub fn new(options: HttpOptions) -> Result<Self, DeltaError> {
        let (client, jar) = build_client(options)?;
        Ok(Self { client, jar })
    }

    pub async fn get_login_qr(&self) -> Result<PioneerLoginQr, DeltaError> {
        self.client
            .get("https://xui.ptlogin2.qq.com/cgi-bin/xlogin")
            .query(&[
                ("appid", PIONEER_APP_ID),
                ("daid", PIONEER_DAID),
                ("style", "33"),
                ("login_text", "\u{767b}\u{5f55}"),
                ("hide_title_bar", "1"),
                ("hide_border", "1"),
                ("target", "self"),
                ("s_url", PIONEER_CALLBACK),
                ("pt_3rd_aid", PIONEER_3RD_AID),
                ("pt_feedback_link", PIONEER_FEEDBACK_LINK),
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
                ("appid", PIONEER_APP_ID),
                ("e", "2"),
                ("l", "M"),
                ("s", "3"),
                ("d", "72"),
                ("v", "4"),
                ("t", "0.6142752744667854"),
                ("daid", PIONEER_DAID),
                ("pt_3rd_aid", PIONEER_3RD_AID),
                ("u1", PIONEER_CALLBACK),
            ])
            .send()
            .await?
            .bytes()
            .await?;

        let qr_sig = must_cookie(&self.jar, "https://xui.ptlogin2.qq.com/", "qrsig")?;
        let login_sig = must_cookie(&self.jar, "https://xui.ptlogin2.qq.com/", "pt_login_sig")?;

        Ok(PioneerLoginQr {
            qr_sig: qr_sig.clone(),
            image: STANDARD.encode(image),
            token: get_qr_token(&qr_sig),
            login_sig,
            cookie: dump_cookie_json(&self.jar, "https://xui.ptlogin2.qq.com/")?,
        })
    }

    pub async fn poll_login_status(
        &self,
        req: PioneerStatusRequest,
    ) -> Result<ApiResponse<Value>, DeltaError> {
        restore_cookie_json(&self.jar, "https://xui.ptlogin2.qq.com/", &req.cookie)?;
        insert_cookie(&self.jar, "https://xui.ptlogin2.qq.com/", "qrsig", &req.qr_sig)?;

        let body = self
            .client
            .get("https://xui.ptlogin2.qq.com/ssl/ptqrlogin")
            .query(&[
                ("u1", PIONEER_CALLBACK),
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
                ("aid", PIONEER_APP_ID),
                ("daid", PIONEER_DAID),
                ("pt_3rd_aid", PIONEER_3RD_AID),
                ("o1vId", "378b06c889d9113b39e814ca627809e3"),
                ("pt_js_version", "530c3f68"),
            ])
            .send()
            .await?
            .text()
            .await?;

        Self::map_poll_body(&body, async |redirect_url| {
            let _ = self.client.get(&redirect_url).send().await?;
            let cookie = dump_cookie_json(&self.jar, "https://graph.qq.com/")?;
            Ok(json!({ "cookie": cookie, "uin": extract_query_param(&redirect_url, "uin").ok() }))
        })
        .await
    }

    pub async fn get_access_token(&self, cookie_json: &str) -> Result<PioneerAccessToken, DeltaError> {
        restore_cookie_json(&self.jar, "https://graph.qq.com/", cookie_json)?;
        let p_skey = must_cookie(&self.jar, "https://graph.qq.com/", "p_skey")?;
        let gtk = crate::delta::utils::hashes::get_gtk(&p_skey).to_string();

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
                ("client_id", PIONEER_OAUTH_APP_ID),
                ("redirect_uri", PIONEER_REDIRECT_URI),
                ("scope", "get_user_info"),
                ("state", "gamer.qq.com"),
                ("switch", ""),
                ("form_plogin", "1"),
                ("src", "1"),
                ("update_auth", "1"),
                ("openapi", "1010"),
                ("g_tk", gtk.as_str()),
                ("auth_time", auth_time.as_str()),
                ("ui", "4F384776-3605-4955-B015-DBA77968FC7C"),
            ])
            .send()
            .await?;

        let location = resp
            .headers()
            .get(LOCATION)
            .ok_or_else(|| DeltaError::Parse("missing location".to_string()))?
            .to_str()
            .map_err(|error| DeltaError::Parse(error.to_string()))?
            .to_string();
        let code = extract_query_param(&location, "code")?;

        let _ = self.client.get(&location).send().await?;

        let callback_resp = self
            .client
            .get("https://gamer.qq.com/v2/passport/qq/callback")
            .query(&[("code", code.as_str()), ("state", "gamer.qq.com")])
            .send()
            .await?;

        if callback_resp.status().as_u16() != 302 {
            let status = callback_resp.status().as_u16();
            return Err(DeltaError::Parse(format!("AccessToken获取失败: {status}")));
        }

        let redirect_location = callback_resp
            .headers()
            .get(LOCATION)
            .ok_or_else(|| DeltaError::Parse("missing callback redirect".to_string()))?
            .to_str()
            .map_err(|error| DeltaError::Parse(error.to_string()))?
            .to_string();

        let _ = self.client.get(&redirect_location).send().await?;

        let key = must_cookie(&self.jar, "https://gamer.qq.com/", "key")?;
        Ok(PioneerAccessToken { key })
    }

    pub async fn update_access_token(
        &self,
        openid: &str,
        access_token: &str,
        cookie_json: Option<&str>,
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
                ("acctype", "qc"),
                ("appid", PIONEER_APP_ID),
                ("access_token", access_token),
                ("openid", openid),
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

    pub async fn get_game_test_list(&self, key: &str, list_type: &str) -> Result<Value, DeltaError> {
        insert_cookie(&self.jar, "https://gamer.qq.com/", "key", key)?;

        let sub_type = match list_type {
            "mobile" => 22i64,
            _ => 12i64,
        };

        let value: Value = self
            .client
            .post("https://m.gamer.qq.com/graph/wxmini/GetCollList")
            .json(&json!({ "subType": sub_type }))
            .send()
            .await?
            .json()
            .await?;

        let err_code = value["errCode"].as_i64().unwrap_or(-1);
        if err_code != 0 {
            let msg = value["msg"].as_str().unwrap_or("获取失败");
            return Err(DeltaError::Parse(msg.to_string()));
        }

        let content_str = value["result"]["collList"][0]["content"]
            .as_str()
            .ok_or_else(|| DeltaError::Parse("missing content".to_string()))?;
        let list: Value = serde_json::from_str(content_str)?;

        if key.is_empty() {
            return Ok(list["list"].clone());
        }

        let items = list["list"].as_array().ok_or_else(|| DeltaError::Parse("invalid list".to_string()))?;
        let mut enriched = Vec::new();
        for item in items {
            let jump_url = item["szJumpUrl"].as_str().unwrap_or("");
            if let Some(id_str) = extract_test_detail_id(jump_url) {
                let detail = self.get_game_detail(key, &id_str).await.unwrap_or(json!(null));
                let mut enriched_item = item.clone();
                enriched_item["detail"] = detail;
                enriched.push(enriched_item);
            } else {
                enriched.push(item.clone());
            }
        }
        Ok(Value::Array(enriched))
    }

    async fn get_game_detail(&self, key: &str, id: &str) -> Result<Value, DeltaError> {
        insert_cookie(&self.jar, "https://gamer.qq.com/", "key", key)?;

        let value: Value = self
            .client
            .get("https://gamer.qq.com/task/misc/gettask2")
            .query(&[("iTaskID", id)])
            .send()
            .await?
            .json()
            .await?;

        let err_code = value["errCode"].as_i64().unwrap_or(-1);
        if err_code != 0 {
            return Ok(json!(null));
        }
        Ok(value["result"].clone())
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

fn extract_test_detail_id(jump_url: &str) -> Option<String> {
    let re = regex::Regex::new(r"/detail/\d+/(\d+)").ok()?;
    re.captures(jump_url).and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
}
