use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use tauri::{AppHandle, Manager};

pub fn settings_path(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法解析配置目录: {error}"))?;

    ensure_config_dir(&config_dir)?;

    Ok(config_dir.join(file_name))
}

pub fn ensure_config_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("无法创建配置目录: {error}"))
}

pub fn load_settings<T: DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    if !path.exists() {
        return Ok(T::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|error| format!("无法读取配置文件 {}: {error}", path.display()))?;

    serde_json::from_str::<T>(&content)
        .map_err(|error| format!("无法解析配置文件 {}: {error}", path.display()))
}

pub fn save_settings<T: serde::Serialize>(path: &Path, settings: &T) -> Result<(), String> {
    let content = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("无法序列化设置: {error}"))?;

    fs::write(path, content)
        .map_err(|error| format!("无法写入配置文件 {}: {error}", path.display()))
}
