pub mod client;
pub mod commands;
pub mod constants;
pub mod error;
pub mod response;
pub mod services;
pub mod state;
pub mod storage;
pub mod utils;

use crate::app_error::AppError;
use tauri::AppHandle;

pub fn initialize(app: &AppHandle) -> Result<state::DeltaState, AppError> {
    state::DeltaState::initialize(app).map_err(AppError::from)
}
