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
        crate::log_info!(
            "settings",
            "配置文件不存在，使用默认配置",
            "path" => path.display().to_string()
        );
        return Ok(T::default());
    }

    let content = fs::read_to_string(path).map_err(|error| {
        crate::log_warn!(
            "settings",
            "读取配置文件失败",
            "path" => path.display().to_string(),
            "error" => error.to_string()
        );
        format!("无法读取配置文件 {}: {error}", path.display())
    })?;

    serde_json::from_str::<T>(&content).map_err(|error| {
        crate::log_warn!(
            "settings",
            "解析配置文件失败",
            "path" => path.display().to_string(),
            "error" => error.to_string()
        );
        format!("无法解析配置文件 {}: {error}", path.display())
    })
}

pub fn save_settings<T: serde::Serialize>(path: &Path, settings: &T) -> Result<(), String> {
    let content = serde_json::to_string_pretty(settings).map_err(|error| {
        crate::log_error!(
            "settings",
            "序列化配置失败",
            "path" => path.display().to_string(),
            "error" => error.to_string()
        );
        format!("无法序列化设置: {error}")
    })?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析配置文件目录 {}", path.display()))?;
    ensure_config_dir(parent)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("配置文件名无效 {}", path.display()))?;
    let temp_path = parent.join(format!(".{file_name}.tmp"));

    fs::write(&temp_path, content).map_err(|error| {
        crate::log_error!(
            "settings",
            "写入临时配置文件失败",
            "path" => temp_path.display().to_string(),
            "target_path" => path.display().to_string(),
            "error" => error.to_string()
        );
        format!("无法写入临时配置文件 {}: {error}", temp_path.display())
    })?;

    replace_settings_file(&temp_path, path).map_err(|error| {
        crate::log_error!(
            "settings",
            "替换配置文件失败",
            "path" => path.display().to_string(),
            "temp_path" => temp_path.display().to_string(),
            "error" => error.to_string()
        );
        format!("无法写入配置文件 {}: {error}", path.display())
    })
}

#[cfg(target_os = "windows")]
fn replace_settings_file(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let temp_wide = temp_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target_wide = target_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    let ok = unsafe {
        MoveFileExW(
            temp_wide.as_ptr(),
            target_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        let error = std::io::Error::last_os_error();
        let _ = fs::remove_file(temp_path);
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_settings_file(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    fs::rename(temp_path, target_path).map_err(|error| {
        let _ = fs::remove_file(temp_path);
        error.to_string()
    })
}
