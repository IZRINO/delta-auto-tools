use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::delta::{
    client::http::HttpOptions,
    constants::{QQ_OAUTH_APP_ID, WECHAT_APP_ID},
    error::DeltaError,
    response::ApiResponse,
    services::{
        game::GameService,
        pioneer_auth::{PioneerAuthService, PioneerLoginQr, PioneerStatusRequest},
        qq_auth::{QqAuthService, QqLoginQr, QqStatusRequest, UpdateTokenOnlyRequest},
        qq_safe::{QqSafeAccess, QqSafeService},
        wechat_auth::{WechatAuthService, WechatQr},
        wegame_auth::{WegameAuthService, WegameQqLoginQr},
    },
    state::DeltaState,
    storage::{AccountKind, DeltaAccountRecord, DeltaAccountView},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountIdRequest {
    pub account_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSessionRequest {
    pub session_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqLikePollRequest {
    pub qr_token: String,
    pub qr_sig: String,
    pub login_sig: String,
    pub session_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqSafeReportRequest {
    pub account_id: i64,
    pub user_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PioneerGameTestListRequest {
    pub account_id: i64,
    #[serde(default)]
    pub list_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginResult {
    pub account_id: i64,
    pub account: DeltaAccountView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QqLoginQrView {
    pub qr_sig: String,
    pub image: String,
    pub token: i64,
    pub login_sig: String,
    pub session_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameQqLoginQrView {
    pub qr_sig: String,
    pub image: String,
    pub token: i64,
    pub login_sig: String,
    pub session_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PioneerLoginQrView {
    pub qr_sig: String,
    pub image: String,
    pub token: i64,
    pub login_sig: String,
    pub session_key: String,
}

#[tauri::command]
pub async fn delta_qq_get_login_qr(
    state: State<'_, DeltaState>,
) -> Result<ApiResponse<QqLoginQrView>, DeltaError> {
    let service = QqAuthService::new(http_options())?;
    let qr = service.get_login_qr().await?;
    state.remember_pending("qq", qr.qr_sig.clone(), qr.cookie.clone())?;
    Ok(ApiResponse::ok("获取成功", qq_login_qr_view(qr)))
}

#[tauri::command]
pub async fn delta_qq_poll_login_status(
    state: State<'_, DeltaState>,
    req: QqLikePollRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let session_key = req.session_key.clone();
    let cookie = state.pending_cookie("qq", &session_key)?;
    let service = QqAuthService::new(http_options())?;
    let mut response = service
        .poll_login_status(qq_status_request(req, cookie))
        .await?;
    sanitize_qq_like_poll_success(&state, "qq", "qq_access", &session_key, &mut response, "QQ")?;
    Ok(response)
}

#[tauri::command]
pub async fn delta_qq_get_access_token(
    state: State<'_, DeltaState>,
    req: AccountSessionRequest,
) -> Result<ApiResponse<AccountLoginResult>, DeltaError> {
    let cookie = state.consume_pending_cookie("qq_access", &req.session_key)?;
    let service = QqAuthService::new(http_options())?;
    let auth = service.get_access_token(&cookie).await?;
    let account = state.persist_account(
        AccountKind::Qq,
        auth.openid.clone(),
        cookie,
        Some(auth.openid.clone()),
        Some(auth.access_token.clone()),
        Some(auth.expires_in),
        Some(json!({ "appid": QQ_OAUTH_APP_ID }).to_string()),
    )?;
    Ok(ApiResponse::ok("获取成功", account_login_result(&account)))
}

#[tauri::command]
pub async fn delta_qq_update_access_token(
    state: State<'_, DeltaState>,
    req: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(req.account_id)?;
    let cookie = state.get_account_cookie(req.account_id)?;
    let service = QqAuthService::new(http_options())?;
    let valid = service
        .update_access_token(
            &UpdateTokenOnlyRequest {
                openid: auth.openid,
                access_token: auth.access_token,
            },
            Some(&cookie),
            "qc",
            QQ_OAUTH_APP_ID,
        )
        .await?;
    token_refresh_response(valid)
}

#[tauri::command]
pub async fn delta_wechat_get_login_qr() -> Result<ApiResponse<WechatQr>, DeltaError> {
    let service = WechatAuthService::new(http_options())?;
    Ok(ApiResponse::ok("获取成功", service.get_login_qr().await?))
}

#[tauri::command]
pub async fn delta_wechat_poll_status(
    state: State<'_, DeltaState>,
    uuid: String,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = WechatAuthService::new(http_options())?;
    let mut response = service.poll_status(&uuid).await?;
    sanitize_wechat_poll_success(&state, "wechat_access", &uuid, &mut response)?;
    Ok(response)
}

#[tauri::command]
pub async fn delta_wechat_get_access_token(
    state: State<'_, DeltaState>,
    req: AccountSessionRequest,
) -> Result<ApiResponse<AccountLoginResult>, DeltaError> {
    let code = state.consume_pending_cookie("wechat_access", &req.session_key)?;
    let service = WechatAuthService::new(http_options())?;
    let auth = service.get_access_token(&code).await?;
    let account = state.persist_account(
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
    Ok(ApiResponse::ok("获取成功", account_login_result(&account)))
}

#[tauri::command]
pub async fn delta_wechat_update_access_token(
    state: State<'_, DeltaState>,
    req: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(req.account_id)?;
    let service = WechatAuthService::new(http_options())?;
    let valid = service
        .update_access_token(&UpdateTokenOnlyRequest {
            openid: auth.openid,
            access_token: auth.access_token,
        })
        .await?;
    token_refresh_response(valid)
}

#[tauri::command]
pub async fn delta_qqsafe_get_login_qr(
    state: State<'_, DeltaState>,
) -> Result<ApiResponse<QqLoginQrView>, DeltaError> {
    let service = QqSafeService::new(http_options())?;
    let qr = service.get_login_qr().await?;
    state.remember_pending("qqsafe", qr.qr_sig.clone(), qr.cookie.clone())?;
    Ok(ApiResponse::ok("获取成功", qq_login_qr_view(qr)))
}

#[tauri::command]
pub async fn delta_qqsafe_poll_status(
    state: State<'_, DeltaState>,
    req: QqLikePollRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let session_key = req.session_key.clone();
    let cookie = state.pending_cookie("qqsafe", &session_key)?;
    let service = QqSafeService::new(http_options())?;
    let mut response = service.poll_status(qq_status_request(req, cookie)).await?;
    sanitize_qq_like_poll_success(
        &state,
        "qqsafe",
        "qqsafe_access",
        &session_key,
        &mut response,
        "QQ安全中心",
    )?;
    Ok(response)
}

#[tauri::command]
pub async fn delta_qqsafe_get_access_token(
    state: State<'_, DeltaState>,
    req: AccountSessionRequest,
) -> Result<ApiResponse<AccountLoginResult>, DeltaError> {
    let cookie = state.consume_pending_cookie("qqsafe_access", &req.session_key)?;
    let service = QqSafeService::new(http_options())?;
    let auth = service.get_access_token(&cookie).await?;
    let account = state.persist_account(
        AccountKind::QqSafe,
        auth.openid.clone(),
        cookie,
        Some(auth.openid.clone()),
        Some(auth.access_token.clone()),
        None,
        Some(json!({ "code": auth.code }).to_string()),
    )?;
    Ok(ApiResponse::ok("获取成功", account_login_result(&account)))
}

#[tauri::command]
pub async fn delta_qqsafe_get_banned_list(
    state: State<'_, DeltaState>,
    req: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let (openid, access_token, code) = state.qq_safe_access(req.account_id)?;
    let service = QqSafeService::new(http_options())?;
    let data = service
        .get_banned_list(&QqSafeAccess {
            openid,
            access_token,
            code,
        })
        .await?;
    Ok(ApiResponse::ok("获取成功", data))
}

#[tauri::command]
pub async fn delta_qqsafe_report(
    state: State<'_, DeltaState>,
    req: QqSafeReportRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let (openid, access_token, _) = state.qq_safe_access(req.account_id)?;
    let service = QqSafeService::new(http_options())?;
    let data = service.report(&openid, &access_token, &req.user_id).await?;
    Ok(ApiResponse::ok("获取成功", data))
}

#[tauri::command]
pub async fn delta_pioneer_get_login_qr(
    state: State<'_, DeltaState>,
) -> Result<ApiResponse<PioneerLoginQrView>, DeltaError> {
    let service = PioneerAuthService::new(http_options())?;
    let qr = service.get_login_qr().await?;
    state.remember_pending("pioneer", qr.qr_sig.clone(), qr.cookie.clone())?;
    Ok(ApiResponse::ok("获取成功", pioneer_login_qr_view(qr)))
}

#[tauri::command]
pub async fn delta_pioneer_poll_status(
    state: State<'_, DeltaState>,
    req: QqLikePollRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let session_key = req.session_key.clone();
    let cookie = state.pending_cookie("pioneer", &session_key)?;
    let service = PioneerAuthService::new(http_options())?;
    let mut response = service
        .poll_login_status(PioneerStatusRequest {
            qr_token: req.qr_token,
            qr_sig: req.qr_sig,
            login_sig: req.login_sig,
            cookie,
        })
        .await?;
    sanitize_qq_like_poll_success(
        &state,
        "pioneer",
        "pioneer_access",
        &session_key,
        &mut response,
        "先遣服",
    )?;
    Ok(response)
}

#[tauri::command]
pub async fn delta_pioneer_get_access_token(
    state: State<'_, DeltaState>,
    req: AccountSessionRequest,
) -> Result<ApiResponse<AccountLoginResult>, DeltaError> {
    let cookie = state.consume_pending_cookie("pioneer_access", &req.session_key)?;
    let service = PioneerAuthService::new(http_options())?;
    let data = service.get_access_token(&cookie).await?;
    let uin = cookie_identity(&cookie).unwrap_or_else(|| "pioneer".to_string());
    let account = state.persist_account(
        AccountKind::Pioneer,
        uin,
        cookie,
        None,
        Some(data.key),
        None,
        Some(json!({ "source": "pioneer" }).to_string()),
    )?;
    Ok(ApiResponse::ok("获取成功", account_login_result(&account)))
}

#[tauri::command]
pub async fn delta_pioneer_update_access_token(
    _state: State<'_, DeltaState>,
    _req: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    Ok(ApiResponse {
        code: -1,
        msg: "先遣服账号不支持自动刷新，请重新扫码登录".to_string(),
        data: json!([]),
    })
}

#[tauri::command]
pub async fn delta_pioneer_get_game_test_list(
    state: State<'_, DeltaState>,
    req: PioneerGameTestListRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let key = state.pioneer_key(req.account_id)?;
    let service = PioneerAuthService::new(http_options())?;
    let list_type = req.list_type.as_deref().unwrap_or("pc");
    let data = service.get_game_test_list(&key, list_type).await?;
    Ok(ApiResponse::ok("获取成功", data))
}

#[tauri::command]
pub fn delta_list_accounts(
    state: State<'_, DeltaState>,
) -> Result<ApiResponse<Vec<DeltaAccountView>>, DeltaError> {
    Ok(ApiResponse::ok("获取成功", state.list_accounts()?))
}

#[tauri::command]
pub fn delta_delete_account(
    account_id: i64,
    state: State<'_, DeltaState>,
) -> Result<ApiResponse<Value>, DeltaError> {
    let deleted = state.delete_account(account_id)?;
    if deleted {
        Ok(ApiResponse::ok("删除成功", json!([])))
    } else {
        Ok(ApiResponse {
            code: -1,
            msg: "账户不存在".to_string(),
            data: json!([]),
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameCodeRequest {
    pub session_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameQqPollRequest {
    pub request: QqLikePollRequest,
}

#[tauri::command]
pub async fn delta_wegame_qq_get_login_qr(
    state: State<'_, DeltaState>,
) -> Result<ApiResponse<WegameQqLoginQrView>, DeltaError> {
    let service = WegameAuthService::new(http_options())?;
    let qr = service.get_qq_login_qr().await?;
    state.remember_pending("wegame_qq", qr.qr_sig.clone(), qr.cookie.clone())?;
    Ok(ApiResponse::ok(
        "获取二维码成功",
        wegame_qq_login_qr_view(qr),
    ))
}

#[tauri::command]
pub async fn delta_wegame_qq_poll_status(
    state: State<'_, DeltaState>,
    request: WegameQqPollRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let session_key = request.request.session_key.clone();
    let cookie = state.pending_cookie("wegame_qq", &session_key)?;
    let service = WegameAuthService::new(http_options())?;
    let mut response = service
        .poll_qq_login_status(wegame_qq_status_request(request.request, cookie))
        .await?;
    sanitize_qq_like_poll_success(
        &state,
        "wegame_qq",
        "wegame_qq_access",
        &session_key,
        &mut response,
        "Wegame QQ",
    )?;
    Ok(response)
}

#[tauri::command]
pub async fn delta_wegame_qq_get_access_token(
    state: State<'_, DeltaState>,
    request: AccountSessionRequest,
) -> Result<ApiResponse<AccountLoginResult>, DeltaError> {
    let cookie = state.consume_pending_cookie("wegame_qq_access", &request.session_key)?;
    let service = WegameAuthService::new(http_options())?;
    let ticket = service.get_qq_access_token(&cookie).await?;
    let record = state.persist_account(
        AccountKind::WegameQq,
        ticket.id.clone(),
        cookie,
        None,
        Some(ticket.ticket),
        None,
        None,
    )?;
    Ok(ApiResponse::ok(
        "获取Wegame票据成功",
        account_login_result(&record),
    ))
}

#[tauri::command]
pub async fn delta_wegame_wechat_get_login_qr() -> Result<ApiResponse<WechatQr>, DeltaError> {
    let service = WegameAuthService::new(http_options())?;
    let qr = service.get_wechat_login_qr().await?;
    Ok(ApiResponse::ok("获取二维码成功", qr))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WegameWechatPollRequest {
    pub uuid: String,
}

#[tauri::command]
pub async fn delta_wegame_wechat_poll_status(
    state: State<'_, DeltaState>,
    request: WegameWechatPollRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = WegameAuthService::new(http_options())?;
    let mut response = service
        .poll_wechat_status(&request.uuid, http_options())
        .await?;
    sanitize_wechat_poll_success(&state, "wegame_wechat_access", &request.uuid, &mut response)?;
    Ok(response)
}

#[tauri::command]
pub async fn delta_wegame_wechat_get_access_token(
    state: State<'_, DeltaState>,
    request: WegameCodeRequest,
) -> Result<ApiResponse<AccountLoginResult>, DeltaError> {
    let code = state.consume_pending_cookie("wegame_wechat_access", &request.session_key)?;
    let service = WegameAuthService::new(http_options())?;
    let ticket = service.get_wechat_access_token(&code).await?;
    let record = state.persist_account(
        AccountKind::WegameWechat,
        ticket.id.clone(),
        "{}".to_string(),
        None,
        Some(ticket.ticket),
        None,
        None,
    )?;
    Ok(ApiResponse::ok(
        "获取Wegame票据成功",
        account_login_result(&record),
    ))
}

#[tauri::command]
pub async fn delta_wegame_open_treasure_gift(
    state: State<'_, DeltaState>,
    request: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let ticket = state.wegame_ticket(request.account_id)?;
    let service = WegameAuthService::new(http_options())?;
    let data = service.open_treasure_gift(&ticket).await?;
    Ok(ApiResponse::ok("开启宝箱成功", data))
}

#[tauri::command]
pub async fn delta_wegame_draw_daily_card(
    state: State<'_, DeltaState>,
    request: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let ticket = state.wegame_ticket(request.account_id)?;
    let service = WegameAuthService::new(http_options())?;
    let data = service.draw_daily_card(&ticket).await?;
    Ok(ApiResponse::ok("抽卡成功", data))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameItemsRequest {
    pub type_id: i64,
    pub sub_type: i64,
    pub item_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePriceRequest {
    pub args: Vec<i64>,
    #[serde(default)]
    pub with_recent: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameLogsRequest {
    pub account_id: i64,
    pub log_type: i64,
    pub page: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameFirearmModListRequest {
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRecommendationRequest {
    pub place: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameGunsRequest {
    pub gun_id: String,
}

#[tauri::command]
pub async fn delta_game_get_items(
    request: GameItemsRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options())?;
    let data = service
        .get_items(request.type_id, request.sub_type, request.item_id)
        .await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_config() -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options())?;
    let data = service.get_config().await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_price(
    request: GamePriceRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options())?;
    let data = service.get_price(request.args, request.with_recent).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_firearm_mod_list(
    request: GameFirearmModListRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options())?;
    let data = service
        .get_firearm_mod_list(request.page, request.page_size)
        .await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_recommendation(
    request: GameRecommendationRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options())?;
    let data = service.get_recommendation(&request.place).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_record(
    state: State<'_, DeltaState>,
    request: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(request.account_id)?;
    let service = GameService::new(http_options())?;
    let data = service.get_record(&auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_player(
    state: State<'_, DeltaState>,
    request: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(request.account_id)?;
    let service = GameService::new(http_options())?;
    let data = service.get_player(&auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_assets(
    state: State<'_, DeltaState>,
    request: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(request.account_id)?;
    let service = GameService::new(http_options())?;
    service.get_assets(&auth).await
}

#[tauri::command]
pub async fn delta_game_get_logs(
    state: State<'_, DeltaState>,
    request: GameLogsRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(request.account_id)?;
    let service = GameService::new(http_options())?;
    let data = service
        .get_logs(&auth, request.log_type, request.page)
        .await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_recent(
    state: State<'_, DeltaState>,
    request: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(request.account_id)?;
    let service = GameService::new(http_options())?;
    let data = service.get_recent(&auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_achievement(
    state: State<'_, DeltaState>,
    request: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(request.account_id)?;
    let service = GameService::new(http_options())?;
    let data = service.get_achievement(&auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_password(
    state: State<'_, DeltaState>,
    request: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(request.account_id)?;
    let service = GameService::new(http_options())?;
    let data = service.get_password(&auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_manufacture(
    state: State<'_, DeltaState>,
    request: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(request.account_id)?;
    let service = GameService::new(http_options())?;
    let data = service.get_manufacture(&auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_guns(
    request: GameGunsRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let service = GameService::new(http_options())?;
    let data = service.get_guns(&request.gun_id).await?;
    Ok(ApiResponse::ok("ok", data))
}

#[tauri::command]
pub async fn delta_game_get_bind(
    state: State<'_, DeltaState>,
    request: AccountIdRequest,
) -> Result<ApiResponse<Value>, DeltaError> {
    let auth = state.game_auth(request.account_id)?;
    let service = GameService::new(http_options())?;
    let data = service.get_bind(&auth).await?;
    Ok(ApiResponse::ok("ok", data))
}

fn http_options() -> HttpOptions {
    HttpOptions
}

fn account_login_result(record: &DeltaAccountRecord) -> AccountLoginResult {
    AccountLoginResult {
        account_id: record.id,
        account: DeltaAccountView::from(record),
    }
}

fn qq_login_qr_view(qr: QqLoginQr) -> QqLoginQrView {
    QqLoginQrView {
        session_key: qr.qr_sig.clone(),
        qr_sig: qr.qr_sig,
        image: qr.image,
        token: qr.token,
        login_sig: qr.login_sig,
    }
}

fn wegame_qq_login_qr_view(qr: WegameQqLoginQr) -> WegameQqLoginQrView {
    WegameQqLoginQrView {
        session_key: qr.qr_sig.clone(),
        qr_sig: qr.qr_sig,
        image: qr.image,
        token: qr.token,
        login_sig: qr.login_sig,
    }
}

fn pioneer_login_qr_view(qr: PioneerLoginQr) -> PioneerLoginQrView {
    PioneerLoginQrView {
        session_key: qr.qr_sig.clone(),
        qr_sig: qr.qr_sig,
        image: qr.image,
        token: qr.token,
        login_sig: qr.login_sig,
    }
}

fn qq_status_request(req: QqLikePollRequest, cookie: String) -> QqStatusRequest {
    QqStatusRequest {
        qr_token: req.qr_token,
        qr_sig: req.qr_sig,
        login_sig: req.login_sig,
        cookie,
    }
}

fn wegame_qq_status_request(
    req: QqLikePollRequest,
    cookie: String,
) -> crate::delta::services::wegame_auth::WegameQqStatusRequest {
    crate::delta::services::wegame_auth::WegameQqStatusRequest {
        qr_token: req.qr_token,
        qr_sig: req.qr_sig,
        login_sig: req.login_sig,
        cookie,
    }
}

fn sanitize_qq_like_poll_success(
    state: &DeltaState,
    source_kind: &str,
    access_kind: &str,
    session_key: &str,
    response: &mut ApiResponse<Value>,
    label: &str,
) -> Result<(), DeltaError> {
    if response.code == 0 {
        let cookie = response
            .data
            .get("cookie")
            .and_then(Value::as_str)
            .filter(|cookie| !cookie.is_empty())
            .ok_or_else(|| DeltaError::Parse(format!("{label} 登录成功但缺少 Cookie")))?
            .to_string();
        let uin = response.data.get("uin").cloned().unwrap_or(Value::Null);
        state.forget_pending(source_kind, session_key)?;
        state.remember_pending(access_kind, session_key.to_string(), cookie)?;
        response.data = json!({ "sessionKey": session_key, "uin": uin });
    } else if response.code < 0 {
        state.forget_pending(source_kind, session_key)?;
    }
    Ok(())
}

fn sanitize_wechat_poll_success(
    state: &DeltaState,
    access_kind: &str,
    session_key: &str,
    response: &mut ApiResponse<Value>,
) -> Result<(), DeltaError> {
    if response.code == 3 {
        let code = response
            .data
            .get("wxCode")
            .and_then(Value::as_str)
            .filter(|code| !code.is_empty())
            .ok_or_else(|| DeltaError::Parse("微信登录成功但缺少授权码".to_string()))?
            .to_string();
        state.remember_pending(access_kind, session_key.to_string(), code)?;
        response.data = json!({ "sessionKey": session_key });
    }
    Ok(())
}

fn token_refresh_response(valid: bool) -> Result<ApiResponse<Value>, DeltaError> {
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

fn cookie_identity(cookie_json: &str) -> Option<String> {
    let cookies: serde_json::Map<String, Value> = serde_json::from_str(cookie_json).ok()?;
    let raw = cookies.get("uin")?.as_str()?;
    let normalized = raw.trim_start_matches('o');
    (!normalized.is_empty()).then(|| normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::{cookie_identity, PioneerGameTestListRequest};
    use super::{AccountIdRequest, GameGunsRequest, GameItemsRequest, QqSafeReportRequest};

    #[test]
    fn extracts_cookie_identity_from_uin_cookie() {
        assert_eq!(
            cookie_identity(r#"{"uin":"o10001"}"#).as_deref(),
            Some("10001")
        );
    }

    #[test]
    fn ignores_empty_cookie_identity() {
        assert_eq!(cookie_identity(r#"{"uin":"o"}"#), None);
    }

    #[test]
    fn deserializes_game_guns_request_camel_case() {
        let request: GameGunsRequest = serde_json::from_value(serde_json::json!({
            "gunId": "weapon-1"
        }))
        .unwrap();
        assert_eq!(request.gun_id, "weapon-1");
    }

    #[test]
    fn deserializes_account_id_request_camel_case() {
        let request: AccountIdRequest = serde_json::from_value(serde_json::json!({
            "accountId": 42
        }))
        .unwrap();
        assert_eq!(request.account_id, 42);
    }

    #[test]
    fn deserializes_game_items_request_camel_case() {
        let request: GameItemsRequest = serde_json::from_value(serde_json::json!({
            "typeId": 3,
            "subType": 4,
            "itemId": "item-1"
        }))
        .unwrap();
        assert_eq!(request.type_id, 3);
        assert_eq!(request.sub_type, 4);
        assert_eq!(request.item_id.as_deref(), Some("item-1"));
    }

    #[test]
    fn deserializes_tool_account_requests_camel_case() {
        let report: QqSafeReportRequest = serde_json::from_value(serde_json::json!({
            "accountId": 7,
            "userId": "target-user"
        }))
        .unwrap();
        assert_eq!(report.account_id, 7);
        assert_eq!(report.user_id, "target-user");

        let pioneer: PioneerGameTestListRequest = serde_json::from_value(serde_json::json!({
            "accountId": 8,
            "listType": "pc"
        }))
        .unwrap();
        assert_eq!(pioneer.account_id, 8);
        assert_eq!(pioneer.list_type.as_deref(), Some("pc"));
    }
}
