use tauri::Manager;

use super::types::RecognitionSettings;

const SETTINGS_FILE: &str = "recognition_settings.json";
const LEGACY_SETTINGS_FILE: &str = "audio_settings.json";

pub fn read_settings(app: &tauri::AppHandle) -> Result<RecognitionSettings, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {e}"))?;

    let path = config_dir.join(SETTINGS_FILE);

    let (content, should_write_migration) = if path.exists() {
        (
            std::fs::read_to_string(&path).map_err(|e| format!("读取识别触发设置失败: {e}"))?,
            false,
        )
    } else {
        let legacy_path = config_dir.join(LEGACY_SETTINGS_FILE);
        if !legacy_path.exists() {
            return Ok(RecognitionSettings::default());
        }
        (
            std::fs::read_to_string(&legacy_path)
                .map_err(|e| format!("读取旧音频设置失败: {e}"))?,
            true,
        )
    };

    let settings: RecognitionSettings =
        serde_json::from_str(&content).map_err(|e| format!("解析识别触发设置失败: {e}"))?;
    let normalized = super::normalize_settings(settings.clone());
    if should_write_migration || normalized != settings {
        write_settings(app, &normalized)?;
    }

    Ok(normalized)
}

pub fn write_settings(
    app: &tauri::AppHandle,
    settings: &RecognitionSettings,
) -> Result<(), String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {e}"))?;

    std::fs::create_dir_all(&config_dir).map_err(|e| format!("创建配置目录失败: {e}"))?;

    let path = config_dir.join(SETTINGS_FILE);

    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化识别触发设置失败: {e}"))?;

    std::fs::write(&path, content).map_err(|e| format!("写入识别触发设置失败: {e}"))?;

    Ok(())
}
