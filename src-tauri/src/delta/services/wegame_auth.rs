use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{cookie::Jar, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::delta::{
    client::http::{build_client, HttpOptions},
    constants::{
        WECHAT_REDIRECT_URI, WEGAME_BASE, WEGAME_QQ_APPID, WEGAME_QQ_CALLBACK, WEGAME_QQ_DAID,
        WEGAME_WECHAT_APPID, WEGAME_WECHAT_CALLBACK, XLOGIN_REFERER,
    },
    error::DeltaError,
    response::ApiResponse,
    services::wechat_auth::{WechatAuthService, WechatQr},
    utils::{
        cookies::{dump_cookie_json, insert_cookie, must_cookie, restore_cookie_json},
        hashes::get_qr_token,
        html::{extract_query_param, extract_wx_qrcode_uuid},
        jsonp::extract_jsonp_args,
        time::current_millis,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameTicket {
    pub id: String,
    pub ticket: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameQqLoginQr {
    pub qr_sig: String,
    pub image: String,
    pub token: i64,
    pub login_sig: String,
    pub cookie: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameQqStatusRequest {
    pub qr_token: String,
    pub qr_sig: String,
    pub login_sig: String,
    pub cookie: String,
}

#[derive(Debug, Clone)]
pub struct WegameAuthService {
    client: Client,
    jar: Arc<Jar>,
}

impl WegameAuthService {
    pub fn new(options: HttpOptions) -> Result<Self, DeltaError> {
        let (client, jar) = build_client(options)?;
        Ok(Self { client, jar })
    }

    // -------- Wegame QQ --------

    pub async fn get_qq_login_qr(&self) -> Result<WegameQqLoginQr, DeltaError> {
        self.client
            .get("https://xui.ptlogin2.qq.com/cgi-bin/xlogin")
            .query(&[
                ("appid", WEGAME_QQ_APPID),
                ("daid", WEGAME_QQ_DAID),
                ("pt_3rd_aid", "0"),
                ("s_url", WEGAME_QQ_CALLBACK),
            ])
            .send()
            .await?
            .error_for_status()?;

        let image = self
            .client
            .get("https://xui.ptlogin2.qq.com/ssl/ptqrshow")
            .query(&[("appid", WEGAME_QQ_APPID), ("daid", WEGAME_QQ_DAID)])
            .send()
            .await?
            .bytes()
            .await?;

        let qr_sig = must_cookie(&self.jar, XLOGIN_REFERER, "qrsig")?;
        let login_sig = must_cookie(&self.jar, XLOGIN_REFERER, "pt_login_sig")?;

        Ok(WegameQqLoginQr {
            qr_sig: qr_sig.clone(),
            image: STANDARD.encode(image),
            token: get_qr_token(&qr_sig),
            login_sig,
            cookie: dump_cookie_json(&self.jar, XLOGIN_REFERER)?,
        })
    }

    pub async fn poll_qq_login_status(
        &self,
        req: WegameQqStatusRequest,
    ) -> Result<ApiResponse<Value>, DeltaError> {
        restore_cookie_json(&self.jar, "https://ssl.ptlogin2.qq.com/", &req.cookie)?;
        insert_cookie(&self.jar, "https://ssl.ptlogin2.qq.com/", "qrsig", &req.qr_sig)?;

        let action = format!("0-0-{}", current_millis());
        let body = self
            .client
            .get("https://ssl.ptlogin2.qq.com/ptqrlogin")
            .header("Referer", XLOGIN_REFERER)
            .query(&[
                ("u1", WEGAME_QQ_CALLBACK),
                ("ptqrtoken", req.qr_token.as_str()),
                ("pt_login_sig", req.login_sig.as_str()),
                ("qrsig", req.qr_sig.as_str()),
                ("action", action.as_str()),
                ("js_ver", "10143"),
                ("js_type", "1"),
                ("login_sig", ""),
                ("pt_vcode_v1", "0"),
                ("pt_verifysession_v1", "0"),
                ("h", "1"),
                ("t", "1"),
                ("g", "1"),
                ("from_ui", "1"),
                ("ptredirect", "0"),
                ("aid", WEGAME_QQ_APPID),
                ("daid", WEGAME_QQ_DAID),
                ("has_onekey", "1"),
            ])
            .send()
            .await?
            .text()
            .await?;

        Self::map_qq_poll_body(&body, async |redirect_url| {
            let _ = self.client.get(&redirect_url).send().await?;
            let cookie = dump_cookie_json(&self.jar, "https://graph.qq.com/")?;
            Ok(json!({ "cookie": cookie, "uin": extract_query_param(&redirect_url, "uin").ok() }))
        })
        .await
    }

    pub async fn get_qq_access_token(&self, cookie_json: &str) -> Result<WegameTicket, DeltaError> {
        restore_cookie_json(&self.jar, "https://graph.qq.com/", cookie_json)?;
        let uin_raw = must_cookie(&self.jar, "https://graph.qq.com/", "uin")?;
        let uin = uin_raw.trim_start_matches('o').to_string();
        let sig = must_cookie(&self.jar, "https://graph.qq.com/", "p_skey")?;

        let body = self
            .client
            .post("https://www.wegame.com.cn/api/middle/clientapi/auth/login_by_qq")
            .header("Referer", WEGAME_BASE)
            .header("Origin", "https://www.wegame.com.cn")
            .json(&json!({
                "clienttype": 1000005,
                "mappid": 10001,
                "config_params": { "lang_type": 0 },
                "login_info": {
                    "qq_info_type": 6,
                    "uin": uin,
                    "sig": sig,
                }
            }))
            .send()
            .await?
            .json::<Value>()
            .await?;

        Self::parse_ticket(&body)
    }

    // -------- Wegame WeChat --------

    pub async fn get_wechat_login_qr(&self) -> Result<WechatQr, DeltaError> {
        let body = self
            .client
            .get("https://open.weixin.qq.com/connect/qrconnect")
            .header("Referer", WECHAT_REDIRECT_URI)
            .query(&[
                ("appid", WEGAME_WECHAT_APPID),
                ("scope", "snsapi_login"),
                ("redirect_uri", WEGAME_WECHAT_CALLBACK),
                ("state", "1"),
                ("self_redirect", "true"),
            ])
            .send()
            .await?
            .text()
            .await?;
        let uuid = extract_wx_qrcode_uuid(&body)?;
        Ok(WechatQr {
            qr_code: format!("https://open.weixin.qq.com/connect/qrcode/{uuid}"),
            uuid,
        })
    }

    pub async fn poll_wechat_status(
        &self,
        uuid: &str,
        options: HttpOptions,
    ) -> Result<ApiResponse<Value>, DeltaError> {
        let service = WechatAuthService::new(options)?;
        service.poll_status(uuid).await
    }

    pub async fn get_wechat_access_token(&self, code: &str) -> Result<WegameTicket, DeltaError> {
        let body = self
            .client
            .post("https://www.wegame.com.cn/api/middle/clientapi/auth/login_by_wechat")
            .header("Referer", WEGAME_BASE)
            .header("Origin", "https://www.wegame.com.cn")
            .json(&json!({
                "clienttype": 1000005,
                "mappid": 10001,
                "login_info": {
                    "wx_info_type": 1,
                    "appid": WEGAME_WECHAT_APPID,
                    "code": code,
                }
            }))
            .send()
            .await?
            .json::<Value>()
            .await?;

        let outer_code = body["code"].as_i64().unwrap_or(-1);
        let inner_code = body["data"]["error_code"].as_i64().unwrap_or(-1);
        if outer_code != 0 || inner_code != 0 {
            return Err(DeltaError::Parse(format!(
                "wegame wechat login failed: outer={outer_code} inner={inner_code}"
            )));
        }

        let id = must_cookie(&self.jar, WEGAME_BASE, "tgp_id")?;
        let ticket = must_cookie(&self.jar, WEGAME_BASE, "tgp_ticket")?;
        Ok(WegameTicket { id, ticket })
    }

    // -------- Wegame actions --------

    fn inject_ticket(&self, ticket: &WegameTicket) -> Result<(), DeltaError> {
        insert_cookie(&self.jar, WEGAME_BASE, "tgp_id", &ticket.id)?;
        insert_cookie(&self.jar, WEGAME_BASE, "tgp_ticket", &ticket.ticket)?;
        Ok(())
    }

    pub async fn open_treasure_gift(&self, ticket: &WegameTicket) -> Result<Value, DeltaError> {
        self.inject_ticket(ticket)?;
        let preview: Value = self
            .client
            .post("https://www.wegame.com.cn/api/act/delta_force/OpenTreasureChest")
            .header("Referer", WEGAME_BASE)
            .json(&json!({}))
            .send()
            .await?
            .json()
            .await?;

        if preview["data"]["is_obtain"].as_bool().unwrap_or(false) {
            return Ok(preview["data"].clone());
        }

        let obtain: Value = self
            .client
            .post("https://www.wegame.com.cn/api/act/delta_force/ObtainTreasureChest")
            .header("Referer", WEGAME_BASE)
            .json(&json!({}))
            .send()
            .await?
            .json()
            .await?;
        Ok(obtain["data"].clone())
    }

    pub async fn draw_daily_card(&self, ticket: &WegameTicket) -> Result<Value, DeltaError> {
        self.inject_ticket(ticket)?;
        let current: Value = self
            .client
            .post("https://www.wegame.com.cn/api/act/delta_force/GetUserCards")
            .header("Referer", WEGAME_BASE)
            .json(&json!({}))
            .send()
            .await?
            .json()
            .await?;

        if current["data"]["has_drawn_today"].as_bool().unwrap_or(false) {
            return Ok(current["data"].clone());
        }

        let _ = self
            .client
            .post("https://www.wegame.com.cn/api/act/delta_force/DrawCard")
            .header("Referer", WEGAME_BASE)
            .json(&json!({}))
            .send()
            .await?
            .text()
            .await?;
        let combo: Value = self
            .client
            .post("https://www.wegame.com.cn/api/act/delta_force/GetCardsBestCombination")
            .header("Referer", WEGAME_BASE)
            .json(&json!({}))
            .send()
            .await?
            .json()
            .await?;
        Ok(combo["data"].clone())
    }

    // -------- Pure helpers (kept stable for tests) --------

    pub async fn map_qq_poll_body<F, Fut>(
        body: &str,
        on_success: F,
    ) -> Result<ApiResponse<Value>, DeltaError>
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
            Some("66") => Ok(ApiResponse {
                code: 1,
                msg: "二维码未失效".to_string(),
                data: json!([]),
            }),
            Some("67") => Ok(ApiResponse {
                code: 2,
                msg: "已扫码待确认".to_string(),
                data: json!([]),
            }),
            Some("65") => Ok(ApiResponse {
                code: -2,
                msg: "二维码失效".to_string(),
                data: json!([]),
            }),
            Some("86") => Ok(ApiResponse {
                code: -3,
                msg: "登录被拒绝".to_string(),
                data: json!([]),
            }),
            _ => Ok(ApiResponse {
                code: -4,
                msg: "未知错误".to_string(),
                data: json!([]),
            }),
        }
    }

    pub fn parse_ticket(value: &Value) -> Result<WegameTicket, DeltaError> {
        Ok(WegameTicket {
            id: value["data"]["user_id"].as_str().unwrap_or_default().to_string(),
            ticket: value["data"]["wt"].as_str().unwrap_or_default().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{WegameAuthService, WegameTicket};

    #[tokio::test]
    async fn maps_wegame_qq_waiting_status() {
        let response =
            WegameAuthService::map_qq_poll_body("ptuiCB('66','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();

        assert_eq!(response.code, 1);
        assert_eq!(response.msg, "二维码未失效");
    }

    #[tokio::test]
    async fn maps_wegame_qq_success_status() {
        let response = WegameAuthService::map_qq_poll_body(
            "ptuiCB('0','0','https://www.wegame.com.cn/login/callback.html?t=qq','0','ok','')",
            |redirect| async move {
                Ok(json!({ "redirect": redirect, "cookie": { "p_skey": "abc" } }))
            },
        )
        .await
        .unwrap();

        assert_eq!(response.code, 0);
        assert_eq!(response.msg, "登录成功");
        assert_eq!(
            response.data["redirect"],
            "https://www.wegame.com.cn/login/callback.html?t=qq"
        );
        assert_eq!(response.data["cookie"]["p_skey"], "abc");
    }

    #[test]
    fn parses_wegame_ticket_payload() {
        let ticket = WegameAuthService::parse_ticket(&json!({
            "data": {
                "user_id": "user-1",
                "wt": "ticket-1"
            }
        }))
        .unwrap();

        assert_eq!(
            ticket,
            WegameTicket {
                id: "user-1".to_string(),
                ticket: "ticket-1".to_string()
            }
        );
    }
}
