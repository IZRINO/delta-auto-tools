use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::delta::{
    client::http::HttpOptions,
    constants::{QQSAFE_OAUTH_APP_ID, QQSAFE_REDIRECT_URI, QQ_LOGIN_JUMP_URL},
    error::DeltaError,
    services::qq_auth::{QqAuthService, QqLoginQr, QqStatusRequest},
    utils::{
        cookies::{insert_cookie, must_cookie, restore_cookie_json},
        hashes::get_gtk,
        html::{decode_jwt_middle, extract_query_param},
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqSafeAccess {
    pub access_token: String,
    pub openid: String,
    pub code: String,
}

#[derive(Debug, Clone)]
pub struct QqSafeService {
    qq_service: QqAuthService,
}

impl QqSafeService {
    pub fn new(options: HttpOptions) -> Result<Self, DeltaError> {
        Ok(Self {
            qq_service: QqAuthService::with_config(
                options,
                QQ_LOGIN_JUMP_URL.to_string(),
                QQSAFE_OAUTH_APP_ID,
                QQSAFE_REDIRECT_URI,
                QQSAFE_OAUTH_APP_ID,
            )?,
        })
    }

    pub async fn get_login_qr(&self) -> Result<QqLoginQr, DeltaError> {
        self.qq_service.get_login_qr().await
    }

    pub async fn poll_status(
        &self,
        req: QqStatusRequest,
    ) -> Result<crate::delta::response::ApiResponse<Value>, DeltaError> {
        self.qq_service.poll_login_status(req).await
    }

    pub async fn get_access_token(&self, cookie_json: &str) -> Result<QqSafeAccess, DeltaError> {
        restore_cookie_json(&self.qq_service.jar, "https://graph.qq.com/", cookie_json)?;
        let p_skey = must_cookie(&self.qq_service.jar, "https://graph.qq.com/", "p_skey")?;
        let gtk = get_gtk(&p_skey).to_string();

        let auth_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();

        let resp = self
            .qq_service
            .client
            .post("https://graph.qq.com/oauth2.0/authorize")
            .form(&[
                ("response_type", "code"),
                ("client_id", QQSAFE_OAUTH_APP_ID),
                ("redirect_uri", QQSAFE_REDIRECT_URI),
                ("scope", "all"),
                ("state", "qqconnect_2"),
                ("switch", ""),
                ("form_plogin", "1"),
                ("src", "1"),
                ("update_auth", "1"),
                ("openapi", "1010"),
                ("g_tk", gtk.as_str()),
                ("auth_time", auth_time.as_str()),
                ("ui", "8414A4DC-B157-42EE-84AE-84477CD7832A"),
            ])
            .send()
            .await?;

        let location = resp
            .headers()
            .get(reqwest::header::LOCATION)
            .ok_or_else(|| DeltaError::Parse("missing location".to_string()))?
            .to_str()
            .map_err(|error| DeltaError::Parse(error.to_string()))?
            .to_string();
        let code = extract_query_param(&location, "code")?;
        let _ = self.qq_service.client.get(&location).send().await?;
        let _ = self
            .qq_service
            .client
            .get("https://gamesafe.qq.com/connect")
            .query(&[
                ("code", code.as_str()),
                ("appId", QQSAFE_OAUTH_APP_ID),
                ("atype", "QQ"),
            ])
            .header(
                reqwest::header::REFERER,
                "https://gamesafe.qq.com/login-ui/index.html",
            )
            .send()
            .await?;

        let gs_code = must_cookie(&self.qq_service.jar, "https://gamesafe.qq.com/", "gs_code")?;
        let openid = must_cookie(&self.qq_service.jar, "https://gamesafe.qq.com/", "gs_id")?;
        let payload = decode_jwt_middle(&gs_code)?;

        let access_token = payload["token"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| DeltaError::Parse("QQ安全中心鉴权失败: 缺少 token".to_string()))?
            .to_string();

        Ok(QqSafeAccess {
            access_token,
            openid,
            code: gs_code,
        })
    }

    pub async fn get_banned_list(&self, req: &QqSafeAccess) -> Result<Value, DeltaError> {
        insert_cookie(
            &self.qq_service.jar,
            "https://gamesafe.qq.com/",
            "openid",
            &req.openid,
        )?;
        insert_cookie(
            &self.qq_service.jar,
            "https://gamesafe.qq.com/",
            "access_token",
            &req.access_token,
        )?;
        insert_cookie(
            &self.qq_service.jar,
            "https://gamesafe.qq.com/",
            "gs_id",
            &req.openid,
        )?;
        insert_cookie(
            &self.qq_service.jar,
            "https://gamesafe.qq.com/",
            "gs_code",
            &req.code,
        )?;

        let value: Value = self
            .qq_service
            .client
            .get("https://gamesafe.qq.com/api/proxy/punish_query")
            .query(&[("query_type", "4"), ("limit", "10")])
            .send()
            .await?
            .json()
            .await?;
        Ok(value["data"].clone())
    }

    pub async fn report(
        &self,
        openid: &str,
        access_token: &str,
        user_id: &str,
    ) -> Result<Value, DeltaError> {
        insert_cookie(
            &self.qq_service.jar,
            "https://wx.gamesafe.qq.com/",
            "openid",
            openid,
        )?;
        insert_cookie(
            &self.qq_service.jar,
            "https://wx.gamesafe.qq.com/",
            "access_token",
            access_token,
        )?;

        let value: Value = self
            .qq_service
            .client
            .get("https://wx.gamesafe.qq.com/api/plat/user_report")
            .query(&[("user_id", user_id)])
            .header("User-Agent", "MicroMessenger")
            .send()
            .await?
            .json()
            .await?;

        let ret = value["ret"].as_i64().unwrap_or(-1);
        if ret != 0 {
            let msg = value["message"].as_str().unwrap_or("获取失败");
            return Err(DeltaError::Parse(msg.to_string()));
        }
        Ok(value["data"].clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::delta::utils::html::decode_jwt_middle;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    #[test]
    fn decodes_qqsafe_token_payload() {
        let payload = decode_jwt_middle("head.eyJ0b2tlbiI6InRva2VuIn0.tail").unwrap();
        assert_eq!(payload["token"], "token");
    }

    #[test]
    fn decodes_qqsafe_token_with_multiple_fields() {
        let json_str = r#"{"token":"abc123","uin":"10001"}"#;
        let json_b64 = URL_SAFE_NO_PAD.encode(json_str);
        let payload = decode_jwt_middle(&format!("head.{json_b64}.tail")).unwrap();
        assert_eq!(payload["token"], "abc123");
        assert_eq!(payload["uin"], "10001");
    }

    #[test]
    fn decodes_qqsafe_token_returns_error_on_invalid_base64() {
        let result = decode_jwt_middle("head.!!!invalid!!!.tail");
        assert!(result.is_err());
    }
}
