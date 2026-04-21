pub mod client;
pub mod commands;
pub mod constants;
pub mod error;
pub mod response;
pub mod services;
pub mod state;
pub mod storage;
pub mod utils;

pub use commands::{
    delta_delete_account, delta_list_accounts, delta_qq_get_access_token,
    delta_qq_get_login_qr, delta_qq_poll_login_status, delta_qq_update_access_token,
    delta_qqsafe_get_access_token, delta_qqsafe_get_banned_list, delta_qqsafe_get_login_qr,
    delta_qqsafe_poll_status, delta_wechat_get_access_token, delta_wechat_get_login_qr,
    delta_wechat_poll_status, delta_wechat_update_access_token,
};
pub use state::DeltaState;

use tauri::AppHandle;

pub fn initialize(app: &AppHandle) -> Result<DeltaState, String> {
    state::DeltaState::initialize(app).map_err(|error| error.to_string())
}
