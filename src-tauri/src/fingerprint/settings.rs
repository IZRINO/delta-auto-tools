use tauri::{AppHandle, Manager};

use super::types::FingerprintSettings;
use crate::settings;

const SETTINGS_FILE_NAME: &str = "fingerprint_settings.json";

pub fn load_settings(app: &AppHandle) -> Result<FingerprintSettings, String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::load_settings(&path)
}

pub fn save_settings(app: &AppHandle, settings_value: &FingerprintSettings) -> Result<(), String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::save_settings(&path, settings_value)
}

pub fn images_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法解析配置目录: {error}"))?;
    let dir = config_dir.join("fingerprint-images");
    settings::ensure_config_dir(&dir)?;
    Ok(dir)
}
