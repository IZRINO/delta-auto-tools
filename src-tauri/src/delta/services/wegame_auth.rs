use std::sync::Arc;

use reqwest::{cookie::Jar, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::delta::{
    client::http::HttpOptions,
    constants::{
        WECHAT_REDIRECT_URI, WEGAME_BASE, WEGAME_QQ_APPID, WEGAME_QQ_CALLBACK, WEGAME_QQ_DAID,
        WEGAME_WECHAT_APPID, WEGAME_WECHAT_CALLBACK, XLOGIN_REFERER,
    },
    error::DeltaError,
    response::ApiResponse,
    services::qq_qr_pipeline::{QrAuthConfig, QrLoginResult, QrStatusRequest, QqQrAuthPipeline},
    services::wechat_auth::{WechatAuthService, WechatQr},
    utils::{
        cookies::{dump_cookie_json, insert_cookie, must_cookie, restore_cookie_json},
        html::{extract_query_param, extract_wx_qrcode_uuid},
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

impl From<QrLoginResult> for WegameQqLoginQr {
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
pub struct WegameQqStatusRequest {
    pub qr_token: String,
    pub qr_sig: String,
    pub login_sig: String,
    pub cookie: String,
}

impl From<WegameQqStatusRequest> for QrStatusRequest {
    fn from(r: WegameQqStatusRequest) -> Self {
        Self {
            qr_token: r.qr_token,
            qr_sig: r.qr_sig,
            login_sig: r.login_sig,
            cookie: r.cookie,
        }
    }
}

fn wegame_qq_auth_config() -> QrAuthConfig {
    QrAuthConfig {
        appid: WEGAME_QQ_APPID,
        daid: WEGAME_QQ_DAID,
        jump_url: WEGAME_QQ_CALLBACK,
        third_aid: None,
        style: "20",
        poll_url: "https://xui.ptlogin2.qq.com/ssl/ptqrlogin",
        cookie_restore_domain: "https://xui.ptlogin2.qq.com/",
        js_ver: "25051315",
        o1v_id: "3f7262f28e2853a1549dbdd4f0008b0f",
        pt_js_version: "9fce2a54",
        xlogin_extra: vec![
            ("pt_no_auth", "0"),
            ("hide_close_icon", "1"),
        ],
        poll_referer: Some(XLOGIN_REFERER),
        waiting_msg: "二维码未失效",
        scanned_msg: "已扫码待确认",
        unknown_msg: "未知错误",
    }
}

#[derive(Debug, Clone)]
pub struct WegameAuthService {
    pipeline: QqQrAuthPipeline,
}

impl WegameAuthService {
    pub fn new(options: HttpOptions) -> Result<Self, DeltaError> {
        let pipeline = QqQrAuthPipeline::new(options, wegame_qq_auth_config())?;
        Ok(Self { pipeline })
    }

    #[allow(dead_code)]
    pub fn client(&self) -> &Client {
        &self.pipeline.client
    }

    #[allow(dead_code)]
    pub fn jar(&self) -> &Arc<Jar> {
        &self.pipeline.jar
    }

    // -------- Wegame QQ --------

    pub async fn get_qq_login_qr(&self) -> Result<WegameQqLoginQr, DeltaError> {
        let result = self.pipeline.get_login_qr().await?;
        Ok(result.into())
    }

    pub async fn poll_qq_login_status(
        &self,
        req: WegameQqStatusRequest,
    ) -> Result<ApiResponse<Value>, DeltaError> {
        let jar = self.pipeline.jar.clone();
        let client = self.pipeline.client.clone();
        self.pipeline
            .poll_login_status(req.into(), async |redirect_url| {
                let _ = client.get(&redirect_url).send().await?;
                let cookie = dump_cookie_json(&jar, "https://graph.qq.com/")?;
                Ok(json!({ "cookie": cookie, "uin": extract_query_param(&redirect_url, "uin").ok() }))
            })
            .await
    }

    pub async fn get_qq_access_token(&self, cookie_json: &str) -> Result<WegameTicket, DeltaError> {
        restore_cookie_json(&self.pipeline.jar, "https://graph.qq.com/", cookie_json)?;
        let uin_raw = must_cookie(&self.pipeline.jar, "https://graph.qq.com/", "uin")?;
        let uin = uin_raw.trim_start_matches('o').to_string();
        let sig = must_cookie(&self.pipeline.jar, "https://graph.qq.com/", "p_skey")?;

        let body = self
            .pipeline
            .client
            .post("https://www.wegame.com.cn/api/middle/clientapi/auth/login_by_qq")
            .header("Referer", WEGAME_QQ_CALLBACK)
            .header("Origin", "https://www.wegame.com.cn")
            .json(&json!({
                "clienttype": 1000005,
                "mappid": 10001,
                "mcode": null,
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
        let ts = current_millis().to_string();
        let body = self
            .pipeline
            .client
            .get("https://open.weixin.qq.com/connect/qrconnect")
            .header("Referer", WECHAT_REDIRECT_URI)
            .query(&[
                ("appid", WEGAME_WECHAT_APPID),
                ("scope", "snsapi_login"),
                ("redirect_uri", WEGAME_WECHAT_CALLBACK),
                ("state", "1"),
                ("login_type", "jssdk"),
                ("self_redirect", "true"),
                ("ts", ts.as_str()),
                ("style", "black"),
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
            .pipeline
            .client
            .post("https://www.wegame.com.cn/api/middle/clientapi/auth/login_by_wechat")
            .header("Referer", "https://www.wegame.com.cn/login/callback.html")
            .header("Origin", "https://www.wegame.com.cn")
            .json(&json!({
                "clienttype": 1000005,
                "mappid": 10001,
                "mcode": "",
                "config_params": { "lang_type": 0 },
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

        let id = must_cookie(&self.pipeline.jar, WEGAME_BASE, "tgp_id")?;
        let ticket = must_cookie(&self.pipeline.jar, WEGAME_BASE, "tgp_ticket")?;
        Ok(WegameTicket { id, ticket })
    }

    // -------- Wegame actions --------

    fn inject_ticket(&self, ticket: &WegameTicket) -> Result<(), DeltaError> {
        insert_cookie(&self.pipeline.jar, WEGAME_BASE, "tgp_id", &ticket.id)?;
        insert_cookie(&self.pipeline.jar, WEGAME_BASE, "tgp_ticket", &ticket.ticket)?;
        Ok(())
    }

    pub async fn open_treasure_gift(&self, ticket: &WegameTicket) -> Result<Value, DeltaError> {
        self.inject_ticket(ticket)?;
        let preview: Value = self
            .pipeline
            .client
            .post("https://www.wegame.com.cn/api/v1/wegame.pallas.dfm.DfmSocial/OpenTreasureChest")
            .header("Referer", "https://www.wegame.com.cn/helper/df/")
            .json(&json!({
                "account_type": 1,
                "from_src": "df_web",
            }))
            .send()
            .await?
            .json()
            .await?;

        if preview["data"]["is_obtain"].as_bool().unwrap_or(false) {
            return Ok(preview["data"].clone());
        }

        let obtain: Value = self
            .pipeline
            .client
            .post(
                "https://www.wegame.com.cn/api/v1/wegame.pallas.dfm.DfmSocial/ObtainTreasureChest",
            )
            .header("Referer", "https://www.wegame.com.cn/helper/df/")
            .json(&json!({
                "account_type": 1,
                "from_src": "df_web",
            }))
            .send()
            .await?
            .json()
            .await?;
        Ok(obtain["data"].clone())
    }

    pub async fn draw_daily_card(&self, ticket: &WegameTicket) -> Result<Value, DeltaError> {
        self.inject_ticket(ticket)?;
        let current: Value = self
            .pipeline
            .client
            .post("https://www.wegame.com.cn/api/v1/wegame.pallas.dfm.DfmSocial/GetUserCards")
            .header("Referer", "https://www.wegame.com.cn/helper/df/")
            .json(&json!({
                "from_src": "\u{4e09}\u{89d2}\u{6d32}\u{884c}\u{52a8}",
            }))
            .send()
            .await?
            .json()
            .await?;

        if current["data"]["has_drawn_today"]
            .as_bool()
            .unwrap_or(false)
        {
            return Ok(current["data"].clone());
        }

        let _ = self
            .pipeline
            .client
            .post("https://www.wegame.com.cn/api/v1/wegame.pallas.dfm.DfmSocial/DrawCard")
            .header("Referer", "https://www.wegame.com.cn/helper/df/")
            .json(&json!({
                "from_src": "\u{4e09}\u{89d2}\u{6d32}\u{884c}\u{52a8}",
            }))
            .send()
            .await?
            .text()
            .await?;
        let combo: Value = self
            .pipeline
            .client
            .post("https://www.wegame.com.cn/api/v1/wegame.pallas.dfm.DfmSocial/GetCardsBestCombination")
            .header("Referer", "https://www.wegame.com.cn/helper/df/")
            .json(&json!({
                "from_src": "\u{4e09}\u{89d2}\u{6d32}\u{884c}\u{52a8}",
            }))
            .send()
            .await?
            .json()
            .await?;
        Ok(combo["data"].clone())
    }

    // -------- Pure helpers (kept stable for tests) --------

    pub fn parse_ticket(value: &Value) -> Result<WegameTicket, DeltaError> {
        let id = value["data"]["user_id"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| DeltaError::Parse("Wegame 登录失败: 缺少 user_id".to_string()))?
            .to_string();
        let ticket = value["data"]["wt"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| DeltaError::Parse("Wegame 登录失败: 缺少票据".to_string()))?
            .to_string();
        Ok(WegameTicket { id, ticket })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{WegameAuthService, WegameTicket};
    use crate::delta::services::qq_qr_pipeline::{QqQrAuthPipeline, QrAuthConfig};

    fn wegame_test_config() -> QrAuthConfig {
        super::wegame_qq_auth_config()
    }

    #[tokio::test]
    async fn maps_wegame_qq_waiting_status() {
        let response =
            QqQrAuthPipeline::map_poll_body(&wegame_test_config(), "ptuiCB('66','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();

        assert_eq!(response.code, 1);
        assert_eq!(response.msg, "二维码未失效");
    }

    #[tokio::test]
    async fn maps_wegame_qq_success_status() {
        let response =
            QqQrAuthPipeline::map_poll_body(
                &wegame_test_config(),
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

    #[tokio::test]
    async fn maps_wegame_qq_scanned_awaiting_confirm() {
        let response =
            QqQrAuthPipeline::map_poll_body(&wegame_test_config(), "ptuiCB('67','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();
        assert_eq!(response.code, 2);
        assert_eq!(response.msg, "已扫码待确认");
    }

    #[tokio::test]
    async fn maps_wegame_qq_expired() {
        let response =
            QqQrAuthPipeline::map_poll_body(&wegame_test_config(), "ptuiCB('65','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();
        assert_eq!(response.code, -2);
        assert_eq!(response.msg, "二维码失效");
    }

    #[tokio::test]
    async fn maps_wegame_qq_rejected() {
        let response =
            QqQrAuthPipeline::map_poll_body(&wegame_test_config(), "ptuiCB('86','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();
        assert_eq!(response.code, -3);
        assert_eq!(response.msg, "登录被拒绝");
    }

    #[tokio::test]
    async fn maps_wegame_qq_unknown_status() {
        let response =
            QqQrAuthPipeline::map_poll_body(&wegame_test_config(), "ptuiCB('99','0','','0','msg','')", |_| async {
                Ok(json!({}))
            })
            .await
            .unwrap();
        assert_eq!(response.code, -4);
        assert_eq!(response.msg, "未知错误");
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

    #[test]
    fn parse_ticket_rejects_missing_fields() {
        assert!(WegameAuthService::parse_ticket(&json!({ "data": {} })).is_err());
        assert!(WegameAuthService::parse_ticket(&json!({})).is_err());
    }
}
