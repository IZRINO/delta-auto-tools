use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::delta::{
    client::http::HttpOptions,
    constants::{QQ_OAUTH_APP_ID, WECHAT_APP_ID},
    error::DeltaError,
    response::ApiResponse,
    services::{
        game::{GameAuth, GameService},
        qq_auth::{QqAccessToken, QqAuthService, QqLoginQr, QqStatusRequest, UpdateTokenOnlyRequest},
        qq_safe::{QqSafeAccess, QqSafeService},
        wechat_auth::{WechatAccessToken, WechatAuthService, WechatQr},
        wegame_auth::{WegameAuthService, WegameQqLoginQr, WegameQqStatusRequest, WegameTicket},
    },
    state::{DeltaState, PendingSession},
    storage::{AccountKind, DeltaAccountRecord, DeltaAccountUpsert},
    utils::time::current_millis,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandOptions {
    pub insecure_skip_tls_verify: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountCookieRequest {
    pub account_id: Option<i64>,
    pub cookie: Option<String>,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqUpdateRequest {
    pub account_id: Option<i64>,
    pub cookie: Option<String>,
    pub openid: String,
    pub access_token: String,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WechatAccessRequest {
    pub code: String,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WechatUpdateRequest {
    pub openid: String,
    pub access_token: String,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqSafeBannedListRequest {
    pub openid: String,
    pub access_token: String,
    pub code: String,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBoundAccess<T>
where
    T: Serialize,
{
    pub account_id: i64,
    pub account: DeltaAccountRecord,
    pub auth: T,
}

#[tauri::command]
pub async fn delta_qq_get_login_qr(
    state: State<'_, DeltaState>,
    options: Option<CommandOptions>,
) -> Result<ApiResponse<QqLoginQr>, DeltaError> {
    let service = QqAuthService::new(http_options(options))?;
    let qr = service.get_login_qr().await?;
    remember_pending(&state, "qq", qr.qr_sig.clone(), qr.cookie.clone())?;
    Ok(ApiResponse::ok("获取成功", qr))
}

#[tauri::command]
pub async fn delta_qq_poll_login_status(
    req: QqStatusRequest,
    options: Option<CommandOptions>,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = QqAuthService::new(http_options(options))?;
    service.poll_login_status(req).await
}

#[tauri::command]
pub async fn delta_qq_get_access_token(
    state: State<'_, DeltaState>,
    req: AccountCookieRequest,
) -> Result<ApiResponse<AccountBoundAccess<QqAccessToken>>, DeltaError> {
    let cookie = resolve_cookie(&state, req.account_id, req.cookie.clone())?;
    let service = QqAuthService::new(http_options(req.options))?;
    let auth = service.get_access_token(&cookie).await?;
    let account = persist_account(
        &state,
        AccountKind::Qq,
        auth.openid.clone(),
        cookie,
        Some(auth.openid.clone()),
        Some(auth.access_token.clone()),
        Some(auth.expires_in),
        Some(json!({ "appid": QQ_OAUTH_APP_ID }).to_string()),
    )?;
    Ok(ApiResponse::ok(
        "获取成功",
        AccountBoundAccess {
            account_id: account.id,
            account,
            auth,
        },
    ))
}

#[tauri::command]
pub async fn delta_qq_update_access_token(
    state: State<'_, DeltaState>,
    req: QqUpdateRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let cookie = resolve_cookie(&state, req.account_id, req.cookie.clone())?;
    let service = QqAuthService::new(http_options(req.options))?;
    let valid = service
        .update_access_token(
            &UpdateTokenOnlyRequest {
                openid: req.openid,
                access_token: req.access_token,
            },
            Some(&cookie),
            "qc",
            QQ_OAUTH_APP_ID,
        )
        .await?;
    Ok(if valid {
        ApiResponse::ok("鉴权仍然有效", json!([]))
    } else {
        ApiResponse {
            code: -1,
            msg: "鉴权已失效".to_string(),
            data: json!([]),
        }
    })
}

#[tauri::command]
pub async fn delta_wechat_get_login_qr(
    options: Option<CommandOptions>,
) -> Result<ApiResponse<WechatQr>, DeltaError> {
    let service = WechatAuthService::new(http_options(options))?;
    Ok(ApiResponse::ok("获取成功", service.get_login_qr().await?))
}

#[tauri::command]
pub async fn delta_wechat_poll_status(
    uuid: String,
    options: Option<CommandOptions>,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = WechatAuthService::new(http_options(options))?;
    service.poll_status(&uuid).await
}

#[tauri::command]
pub async fn delta_wechat_get_access_token(
    state: State<'_, DeltaState>,
    req: WechatAccessRequest,
) -> Result<ApiResponse<AccountBoundAccess<WechatAccessToken>>, DeltaError> {
    let service = WechatAuthService::new(http_options(req.options))?;
    let auth = service.get_access_token(&req.code).await?;
    let account = persist_account(
        &state,
        AccountKind::Wechat,
        auth.openid.clone(),
        "{}".to_string(),
        Some(auth.openid.clone()),
        Some(auth.access_token.clone()),
        Some(auth.expires_in),
        Some(
            json!({ "unionid": auth.unionid, "refreshToken": auth.refresh_token, "appid": WECHAT_APP_ID })
                .to_string(),
        ),
    )?;
    Ok(ApiResponse::ok(
        "获取成功",
        AccountBoundAccess {
            account_id: account.id,
            account,
            auth,
        },
    ))
}

#[tauri::command]
pub async fn delta_wechat_update_access_token(
    req: WechatUpdateRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = WechatAuthService::new(http_options(req.options))?;
    let valid = service
        .update_access_token(&UpdateTokenOnlyRequest {
            openid: req.openid,
            access_token: req.access_token,
        })
        .await?;
    Ok(if valid {
        ApiResponse::ok("鉴权仍然有效", json!([]))
    } else {
        ApiResponse {
            code: -1,
            msg: "鉴权已失效".to_string(),
            data: json!([]),
        }
    })
}

#[tauri::command]
pub async fn delta_qqsafe_get_login_qr(
    options: Option<CommandOptions>,
) -> Result<ApiResponse<QqLoginQr>, DeltaError> {
    let service = QqSafeService::new(http_options(options))?;
    Ok(ApiResponse::ok("获取成功", service.get_login_qr().await?))
}

#[tauri::command]
pub async fn delta_qqsafe_poll_status(
    req: QqStatusRequest,
    options: Option<CommandOptions>,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = QqSafeService::new(http_options(options))?;
    service.poll_status(req).await
}

#[tauri::command]
pub async fn delta_qqsafe_get_access_token(
    state: State<'_, DeltaState>,
    req: AccountCookieRequest,
) -> Result<ApiResponse<AccountBoundAccess<QqSafeAccess>>, DeltaError> {
    let cookie = resolve_cookie(&state, req.account_id, req.cookie.clone())?;
    let service = QqSafeService::new(http_options(req.options))?;
    let auth = service.get_access_token(&cookie).await?;
    let account = persist_account(
        &state,
        AccountKind::QqSafe,
        auth.openid.clone(),
        cookie,
        Some(auth.openid.clone()),
        Some(auth.access_token.clone()),
        None,
        Some(json!({ "code": auth.code }).to_string()),
    )?;
    Ok(ApiResponse::ok(
        "获取成功",
        AccountBoundAccess {
            account_id: account.id,
            account,
            auth,
        },
    ))
}

#[tauri::command]
pub async fn delta_qqsafe_get_banned_list(
    req: QqSafeBannedListRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = QqSafeService::new(http_options(req.options))?;
    let data = service
        .get_banned_list(&QqSafeAccess {
            openid: req.openid,
            access_token: req.access_token,
            code: req.code,
        })
        .await?;
    Ok(ApiResponse::ok("获取成功", data))
}

#[tauri::command]
pub fn delta_list_accounts(
    state: State<'_, DeltaState>,
) -> Result<ApiResponse<Vec<DeltaAccountRecord>>, DeltaError> {
    Ok(ApiResponse::ok("获取成功", state.repo.list_accounts()?))
}

#[tauri::command]
pub fn delta_delete_account(
    account_id: i64,
    state: State<'_, DeltaState>,
) -> Result<ApiResponse<Value>, DeltaError> {
    let deleted = state.repo.delete_account(account_id)?;
    if deleted {
        if let Ok(mut buckets) = state.buckets.lock() {
            buckets.remove(&account_id);
        }
        Ok(ApiResponse::ok("删除成功", json!([])))
    } else {
        Ok(ApiResponse {
            code: -1,
            msg: "账户不存在".to_string(),
            data: json!([]),
        })
    }
}

fn http_options(options: Option<CommandOptions>) -> HttpOptions {
    HttpOptions {
        insecure_skip_tls_verify: options
            .and_then(|options| options.insecure_skip_tls_verify)
            .unwrap_or(false),
    }
}

fn resolve_cookie(
    state: &State<'_, DeltaState>,
    account_id: Option<i64>,
    cookie: Option<String>,
) -> Result<String, DeltaError> {
    if let Some(cookie) = cookie {
        return Ok(cookie);
    }
    let account_id = account_id.ok_or_else(|| DeltaError::InvalidInput("cookie 或 accountId 必填".to_string()))?;
    state
        .repo
        .get_account(account_id)?
        .map(|record| record.cookie_json)
        .ok_or(DeltaError::AccountNotFound)
}

fn persist_account(
    state: &State<'_, DeltaState>,
    kind: AccountKind,
    uin_or_openid: String,
    cookie_json: String,
    openid: Option<String>,
    access_token: Option<String>,
    expires_in: Option<i64>,
    extra_json: Option<String>,
) -> Result<DeltaAccountRecord, DeltaError> {
    let now = current_millis();
    let record = state.repo.upsert_account(DeltaAccountUpsert {
        kind,
        uin_or_openid,
        cookie_json,
        openid,
        access_token,
        extra_json,
        expires_at: expires_in.map(|expires_in| now + expires_in * 1000),
        now,
    })?;
    if let Ok(mut buckets) = state.buckets.lock() {
        buckets.insert(record.id, record.clone());
    }
    Ok(record)
}

fn remember_pending(
    state: &State<'_, DeltaState>,
    kind: &str,
    session_key: String,
    cookie_json: String,
) -> Result<(), DeltaError> {
    let mut pending = state
        .pending
        .lock()
        .map_err(|_| DeltaError::Storage("pending lock poisoned".to_string()))?;
    pending.insert(
        session_key.clone(),
        PendingSession {
            kind: kind.to_string(),
            session_key,
            cookie_json,
        },
    );
    Ok(())
}

// ============================================================================
// Wegame commands
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameCodeRequest {
    pub code: String,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameTicketRequest {
    pub ticket: WegameTicket,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameQqPollRequest {
    pub request: WegameQqStatusRequest,
    pub options: Option<CommandOptions>,
}

#[tauri::command]
pub async fn delta_wegame_qq_get_login_qr(
    state: State<'_, DeltaState>,
    options: Option<CommandOptions>,
) -> Result<ApiResponse<WegameQqLoginQr>, DeltaError> {
    let service = WegameAuthService::new(http_options(options))?;
    let qr = service.get_qq_login_qr().await?;
    remember_pending(&state, "wegame_qq", qr.qr_sig.clone(), qr.cookie.clone())?;
    Ok(ApiResponse::ok("获取二维码成功", qr))
}

#[tauri::command]
pub async fn delta_wegame_qq_poll_status(
    state: State<'_, DeltaState>,
    request: WegameQqPollRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = WegameAuthService::new(http_options(request.options.clone()))?;
    let resp = service.poll_qq_login_status(request.request.clone()).await?;
    if resp.code == 0 {
        // Persist cookie under wegame_qq once redirect completes
        let cookie_json = resp
            .data
            .get("cookie")
            .and_then(Value::as_str)
            .unwrap_or("{}")
            .to_string();
        let uin = resp
            .data
            .get("uin")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !uin.is_empty() {
            let _ = persist_account(
                &state,
                AccountKind::WegameQq,
                uin,
                cookie_json,
                None,
                None,
                None,
                None,
            )?;
        }
    }
    Ok(resp)
}

#[tauri::command]
pub async fn delta_wegame_qq_get_access_token(
    state: State<'_, DeltaState>,
    request: AccountCookieRequest,
) -> Result<ApiResponse<AccountBoundAccess<WegameTicket>>, DeltaError> {
    let cookie = resolve_cookie(&state, request.account_id, request.cookie)?;
    let service = WegameAuthService::new(http_options(request.options))?;
    let ticket = service.get_qq_access_token(&cookie).await?;
    let record = persist_account(
        &state,
        AccountKind::WegameQq,
        ticket.id.clone(),
        cookie,
        None,
        Some(ticket.ticket.clone()),
        None,
        None,
    )?;
    Ok(ApiResponse::ok(
        "获取Wegame票据成功",
        AccountBoundAccess {
            account_id: record.id,
            account: record,
            auth: ticket,
        },
    ))
}

#[tauri::command]
pub async fn delta_wegame_wechat_get_login_qr(
    options: Option<CommandOptions>,
) -> Result<ApiResponse<WechatQr>, DeltaError> {
    let service = WegameAuthService::new(http_options(options))?;
    let qr = service.get_wechat_login_qr().await?;
    Ok(ApiResponse::ok("获取二维码成功", qr))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameWechatPollRequest {
    pub uuid: String,
    pub options: Option<CommandOptions>,
}

#[tauri::command]
pub async fn delta_wegame_wechat_poll_status(
    request: WegameWechatPollRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = WegameAuthService::new(http_options(request.options.clone()))?;
    service
        .poll_wechat_status(&request.uuid, http_options(request.options))
        .await
}

#[tauri::command]
pub async fn delta_wegame_wechat_get_access_token(
    state: State<'_, DeltaState>,
    request: WegameCodeRequest,
) -> Result<ApiResponse<AccountBoundAccess<WegameTicket>>, DeltaError> {
    let service = WegameAuthService::new(http_options(request.options))?;
    let ticket = service.get_wechat_access_token(&request.code).await?;
    let record = persist_account(
        &state,
        AccountKind::WegameWechat,
        ticket.id.clone(),
        "{}".to_string(),
        None,
        Some(ticket.ticket.clone()),
        None,
        None,
    )?;
    Ok(ApiResponse::ok(
        "获取Wegame票据成功",
        AccountBoundAccess {
            account_id: record.id,
            account: record,
            auth: ticket,
        },
    ))
}

#[tauri::command]
pub async fn delta_wegame_open_treasure_gift(
    request: WegameTicketRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = WegameAuthService::new(http_options(request.options))?;
    let data = service.open_treasure_gift(&request.ticket).await?;
    Ok(ApiResponse::ok("开启宝箱成功", data))
}

#[tauri::command]
pub async fn delta_wegame_draw_daily_card(
    request: WegameTicketRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = WegameAuthService::new(http_options(request.options))?;
    let data = service.draw_daily_card(&request.ticket).await?;
    Ok(ApiResponse::ok("抽卡成功", data))
}

// ============================================================================
// Game commands
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAuthRequest {
    pub auth: GameAuth,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameItemsRequest {
    #[serde(rename = "type")]
    pub type_id: i64,
    pub sub_type: i64,
    pub item_id: Option<String>,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePriceRequest {
    pub args: Vec<i64>,
    #[serde(default)]
    pub with_recent: bool,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameLogsRequest {
    pub auth: GameAuth,
    pub log_type: i64,
    pub page: i64,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameFirearmModListRequest {
    pub page: i64,
    pub page_size: i64,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRecommendationRequest {
    pub place: String,
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameNoAuthOptions {
    pub options: Option<CommandOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameGunsRequest {
    pub gun_id: String,
    pub options: Option<CommandOptions>,
}

#[tauri::command]
pub async fn delta_game_get_items(
    request: GameItemsRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service
        .get_items(request.type_id, request.sub_type, request.item_id)
        .await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_config(
    request: GameNoAuthOptions,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_config().await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_price(
    request: GamePriceRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_price(request.args, request.with_recent).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_firearm_mod_list(
    request: GameFirearmModListRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service
        .get_firearm_mod_list(request.page, request.page_size)
        .await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_recommendation(
    request: GameRecommendationRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_recommendation(&request.place).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_record(
    request: GameAuthRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_record(&request.auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_player(
    request: GameAuthRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_player(&request.auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_assets(
    request: GameAuthRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    service.get_assets(&request.auth).await
}

#[tauri::command]
pub async fn delta_game_get_logs(
    request: GameLogsRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service
        .get_logs(&request.auth, request.log_type, request.page)
        .await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_recent(
    request: GameAuthRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_recent(&request.auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_achievement(
    request: GameAuthRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_achievement(&request.auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_password(
    request: GameAuthRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_password(&request.auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_manufacture(
    request: GameAuthRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_manufacture(&request.auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_guns(
    request: GameGunsRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_guns(&request.gun_id).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_bind(
    request: GameAuthRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options(request.options))?;
    let data = service.get_bind(&request.auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::GameGunsRequest;

    #[test]
    fn deserializes_game_guns_request_from_gun_id() {
        let request: GameGunsRequest =
            serde_json::from_value(json!({ "gunId": "gun-akm" })).unwrap();

        assert_eq!(request.gun_id, "gun-akm");
        assert!(request.options.is_none());
    }
}
