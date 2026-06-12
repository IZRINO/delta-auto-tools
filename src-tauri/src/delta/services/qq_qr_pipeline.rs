use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{cookie::Jar, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

use crate::delta::{
    client::http::{build_client, HttpOptions},
    error::DeltaError,
    response::ApiResponse,
    utils::{
        cookies::{dump_cookie_json, insert_cookie, must_cookie, restore_cookie_json},
        hashes::get_qr_token,
        jsonp::extract_jsonp_args,
        time::current_millis,
    },
};

/// 通用 QQ 扫码登录 QR 结果（三个服务的字段完全一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginResult {
    pub qr_sig: String,
    pub image: String,
    pub token: i64,
    pub login_sig: String,
    pub cookie: String,
}

/// 通用 QQ 扫码状态轮询请求（三个服务的字段完全一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrStatusRequest {
    pub qr_token: String,
    pub qr_sig: String,
    pub login_sig: String,
    pub cookie: String,
}

/// QQ QR 鉴权管线配置
#[derive(Debug, Clone)]
pub struct QrAuthConfig {
    /// xlogin / ptqrshow / ptqrlogin 使用的 appid
    pub appid: &'static str,
    /// xlogin / ptqrshow / ptqrlogin 使用的 daid
    pub daid: &'static str,
    /// xlogin 的 s_url 和 ptqrshow / ptqrlogin 的 u1
    pub jump_url: &'static str,
    /// xlogin / ptqrshow 的 pt_3rd_aid（None 表示不发送该参数）
    pub third_aid: Option<&'static str>,
    /// xlogin 的 style
    pub style: &'static str,
    /// ptqrlogin 的 URL
    pub poll_url: &'static str,
    /// cookie 恢复域名
    pub cookie_restore_domain: &'static str,
    /// ptqrlogin 的 js_ver
    pub js_ver: &'static str,
    /// ptqrlogin 的 o1vId
    pub o1v_id: &'static str,
    /// ptqrlogin 的 pt_js_version
    pub pt_js_version: &'static str,
    /// xlogin 额外参数
    pub xlogin_extra: Vec<(&'static str, &'static str)>,
    /// ptqrlogin Referer（None 表示不设）
    pub poll_referer: Option<&'static str>,
    /// map_poll_body 中的等待扫码消息
    pub waiting_msg: &'static str,
    /// map_poll_body 中的已扫码待确认消息
    pub scanned_msg: &'static str,
    /// map_poll_body 中的未知错误消息
    pub unknown_msg: &'static str,
}

/// 统一的 QQ 扫码鉴权管线。
///
/// 提取 QqAuthService / PioneerAuthService / WegameAuthService 三重复制的
/// ptlogin2 扫码流程：xlogin → ptqrshow → ptqrlogin → map_poll_body。
///
/// 各服务保留自己独有的 token 交换逻辑，扫码阶段委托给 pipeline。
#[derive(Clone)]
pub struct QqQrAuthPipeline {
    pub(crate) client: Client,
    pub(crate) jar: Arc<Jar>,
    pub(crate) config: QrAuthConfig,
}

impl fmt::Debug for QqQrAuthPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QqQrAuthPipeline")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl QqQrAuthPipeline {
    pub fn new(options: HttpOptions, config: QrAuthConfig) -> Result<Self, DeltaError> {
        let (client, jar) = build_client(options)?;
        Ok(Self {
            client,
            jar,
            config,
        })
    }

    /// 获取扫码登录 QR 码
    pub async fn get_login_qr(&self) -> Result<QrLoginResult, DeltaError> {
        let cfg = &self.config;

        // 构建 xlogin 查询参数
        let mut xlogin_params: Vec<(&str, &str)> = vec![
            ("appid", cfg.appid),
            ("daid", cfg.daid),
            ("style", cfg.style),
            ("target", "self"),
            ("hide_border", "1"),
            ("s_url", cfg.jump_url),
        ];
        if let Some(third_aid) = cfg.third_aid {
            xlogin_params.push(("pt_3rd_aid", third_aid));
        }
        for &(k, v) in &cfg.xlogin_extra {
            xlogin_params.push((k, v));
        }

        self.client
            .get("https://xui.ptlogin2.qq.com/cgi-bin/xlogin")
            .query(&xlogin_params)
            .send()
            .await?
            .error_for_status()?;

        // 构建 ptqrshow 查询参数
        let mut ptqrshow_params: Vec<(&str, &str)> = vec![
            ("appid", cfg.appid),
            ("e", "2"),
            ("l", "M"),
            ("s", "3"),
            ("d", "72"),
            ("v", "4"),
            ("t", "0.6142752744667854"),
            ("daid", cfg.daid),
            ("u1", cfg.jump_url),
        ];
        if let Some(third_aid) = cfg.third_aid {
            ptqrshow_params.push(("pt_3rd_aid", third_aid));
        }

        let image = self
            .client
            .get("https://xui.ptlogin2.qq.com/ssl/ptqrshow")
            .query(&ptqrshow_params)
            .send()
            .await?
            .bytes()
            .await?;

        let qr_sig = must_cookie(&self.jar, "https://xui.ptlogin2.qq.com/", "qrsig")?;
        let login_sig = must_cookie(&self.jar, "https://xui.ptlogin2.qq.com/", "pt_login_sig")?;

        Ok(QrLoginResult {
            qr_sig: qr_sig.clone(),
            image: STANDARD.encode(image),
            token: get_qr_token(&qr_sig),
            login_sig,
            cookie: dump_cookie_json(&self.jar, "https://xui.ptlogin2.qq.com/")?,
        })
    }

    /// 轮询扫码登录状态
    pub async fn poll_login_status<F, Fut>(
        &self,
        req: QrStatusRequest,
        on_success: F,
    ) -> Result<ApiResponse<Value>, DeltaError>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<Value, DeltaError>>,
    {
        let cfg = &self.config;

        restore_cookie_json(&self.jar, cfg.cookie_restore_domain, &req.cookie)?;
        insert_cookie(&self.jar, cfg.cookie_restore_domain, "qrsig", &req.qr_sig)?;

        let action = format!("0-0-{}", current_millis());

        let mut poll_params: Vec<(&str, &str)> = vec![
            ("u1", cfg.jump_url),
            ("ptqrtoken", req.qr_token.as_str()),
            ("ptredirect", "0"),
            ("h", "1"),
            ("t", "1"),
            ("g", "1"),
            ("from_ui", "1"),
            ("ptlang", "2052"),
            ("action", action.as_str()),
            ("js_ver", cfg.js_ver),
            ("js_type", "1"),
            ("login_sig", req.login_sig.as_str()),
            ("pt_uistyle", "40"),
            ("aid", cfg.appid),
            ("daid", cfg.daid),
            ("o1vId", cfg.o1v_id),
            ("pt_js_version", cfg.pt_js_version),
        ];
        if let Some(third_aid) = cfg.third_aid {
            poll_params.push(("pt_3rd_aid", third_aid));
        }

        let mut request = self.client.get(cfg.poll_url).query(&poll_params);
        if let Some(referer) = cfg.poll_referer {
            request = request.header("Referer", referer);
        }

        let body = request.send().await?.text().await?;

        Self::map_poll_body(cfg, &body, on_success).await
    }

    /// 解析 ptuiCB JSONP 回调并映射状态码
    pub async fn map_poll_body<F, Fut>(
        config: &QrAuthConfig,
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
                msg: config.waiting_msg.to_string(),
                data: json!([]),
            }),
            Some("67") => Ok(ApiResponse {
                code: 2,
                msg: config.scanned_msg.to_string(),
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
                msg: config.unknown_msg.to_string(),
                data: json!([]),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{QrAuthConfig, QqQrAuthPipeline};

    fn test_config() -> QrAuthConfig {
        QrAuthConfig {
            appid: "716027609",
            daid: "383",
            jump_url: "https://graph.qq.com/oauth2.0/login_jump",
            third_aid: Some("101491592"),
            style: "33",
            poll_url: "https://ssl.ptlogin2.qq.com/ptqrlogin",
            cookie_restore_domain: "https://ssl.ptlogin2.qq.com/",
            js_ver: "25040111",
            o1v_id: "378b06c889d9113b39e814ca627809e3",
            pt_js_version: "530c3f68",
            xlogin_extra: vec![],
            poll_referer: None,
            waiting_msg: "等待扫码",
            scanned_msg: "已扫码,待确认",
            unknown_msg: "未知错误信息",
        }
    }

    #[tokio::test]
    async fn maps_waiting_status() {
        let config = test_config();
        let response = QqQrAuthPipeline::map_poll_body(
            &config,
            "ptuiCB('66','0','','0','msg','')",
            |_| async { Ok(json!({})) },
        )
        .await
        .unwrap();
        assert_eq!(response.code, 1);
        assert_eq!(response.msg, "等待扫码");
    }

    #[tokio::test]
    async fn maps_success_status() {
        let config = test_config();
        let response = QqQrAuthPipeline::map_poll_body(
            &config,
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
        let config = test_config();
        let response = QqQrAuthPipeline::map_poll_body(
            &config,
            "ptuiCB('67','0','','0','msg','')",
            |_| async { Ok(json!({})) },
        )
        .await
        .unwrap();
        assert_eq!(response.code, 2);
        assert_eq!(response.msg, "已扫码,待确认");
    }

    #[tokio::test]
    async fn maps_qr_expired_status() {
        let config = test_config();
        let response = QqQrAuthPipeline::map_poll_body(
            &config,
            "ptuiCB('65','0','','0','msg','')",
            |_| async { Ok(json!({})) },
        )
        .await
        .unwrap();
        assert_eq!(response.code, -2);
        assert_eq!(response.msg, "二维码失效");
    }

    #[tokio::test]
    async fn maps_login_rejected_status() {
        let config = test_config();
        let response = QqQrAuthPipeline::map_poll_body(
            &config,
            "ptuiCB('86','0','','0','msg','')",
            |_| async { Ok(json!({})) },
        )
        .await
        .unwrap();
        assert_eq!(response.code, -3);
        assert_eq!(response.msg, "登录被拒绝");
    }

    #[tokio::test]
    async fn maps_unknown_status() {
        let config = test_config();
        let response = QqQrAuthPipeline::map_poll_body(
            &config,
            "ptuiCB('99','0','','0','msg','')",
            |_| async { Ok(json!({})) },
        )
        .await
        .unwrap();
        assert_eq!(response.code, -4);
        assert_eq!(response.msg, "未知错误信息");
    }

    #[tokio::test]
    async fn maps_empty_callback() {
        let config = test_config();
        let result = QqQrAuthPipeline::map_poll_body(
            &config,
            "ptuiCB()",
            |_| async { Ok(json!({})) },
        )
        .await;
        assert!(result.is_err() || result.unwrap().code == -4);
    }

    #[tokio::test]
    async fn wegame_custom_messages() {
        let config = QrAuthConfig {
            waiting_msg: "二维码未失效",
            scanned_msg: "已扫码待确认",
            unknown_msg: "未知错误",
            ..test_config()
        };
        let response = QqQrAuthPipeline::map_poll_body(
            &config,
            "ptuiCB('66','0','','0','msg','')",
            |_| async { Ok(json!({})) },
        )
        .await
        .unwrap();
        assert_eq!(response.msg, "二维码未失效");

        let response = QqQrAuthPipeline::map_poll_body(
            &config,
            "ptuiCB('67','0','','0','msg','')",
            |_| async { Ok(json!({})) },
        )
        .await
        .unwrap();
        assert_eq!(response.msg, "已扫码待确认");
    }
}
