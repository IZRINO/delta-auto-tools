pub mod client;
pub mod commands;
pub mod constants;
pub mod error;
pub mod response;
pub mod services;
pub mod state;
pub mod storage;
pub mod utils;

use tauri::AppHandle;

pub fn initialize(app: &AppHandle) -> Result<state::DeltaState, String> {
    state::DeltaState::initialize(app).map_err(|error| error.to_string())
}
