use tauri::Manager;

use super::types::AudioSettings;

const SETTINGS_FILE: &str = "audio_settings.json";

pub fn read_settings(app: &tauri::AppHandle) -> Result<AudioSettings, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {e}"))?;

    let path = config_dir.join(SETTINGS_FILE);

    if !path.exists() {
        return Ok(AudioSettings::default());
    }

    let content = std::fs::read_to_string(&path).map_err(|e| format!("读取音频设置失败: {e}"))?;

    let settings: AudioSettings =
        serde_json::from_str(&content).map_err(|e| format!("解析音频设置失败: {e}"))?;

    Ok(settings)
}

pub fn write_settings(app: &tauri::AppHandle, settings: &AudioSettings) -> Result<(), String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {e}"))?;

    std::fs::create_dir_all(&config_dir).map_err(|e| format!("创建配置目录失败: {e}"))?;

    let path = config_dir.join(SETTINGS_FILE);

    let content =
        serde_json::to_string_pretty(settings).map_err(|e| format!("序列化音频设置失败: {e}"))?;

    std::fs::write(&path, content).map_err(|e| format!("写入音频设置失败: {e}"))?;

    Ok(())
}
