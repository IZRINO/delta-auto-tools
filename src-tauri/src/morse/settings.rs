use std::{fs, path::PathBuf};

use tauri::{AppHandle, Manager};

use super::types::MorseSettings;

const SETTINGS_FILE_NAME: &str = "morse_settings.json";

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法解析配置目录: {error}"))?;

    fs::create_dir_all(&config_dir).map_err(|error| format!("无法创建配置目录: {error}"))?;

    Ok(config_dir.join(SETTINGS_FILE_NAME))
}

pub fn load_settings(app: &AppHandle) -> Result<MorseSettings, String> {
    let path = settings_path(app)?;

    if !path.exists() {
        return Ok(MorseSettings::default());
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| format!("无法读取配置文件 {}: {error}", path.display()))?;

    serde_json::from_str::<MorseSettings>(&content)
        .map_err(|error| format!("无法解析配置文件 {}: {error}", path.display()))
}

pub fn save_settings(app: &AppHandle, settings: &MorseSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let content = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("无法序列化设置: {error}"))?;

    fs::write(&path, content)
        .map_err(|error| format!("无法写入配置文件 {}: {error}", path.display()))
}
