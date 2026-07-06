use std::path::Path;

use tauri::Manager;

use super::types::RecognitionSettings;
use crate::settings as common_settings;

const SETTINGS_FILE: &str = "recognition_settings.json";
const LEGACY_SETTINGS_FILE: &str = "audio_settings.json";

pub fn read_settings(app: &tauri::AppHandle) -> Result<RecognitionSettings, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {e}"))?;
    read_settings_from_dir(&config_dir)
}

fn read_settings_from_dir(config_dir: &Path) -> Result<RecognitionSettings, String> {
    common_settings::ensure_config_dir(config_dir)?;

    let path = config_dir.join(SETTINGS_FILE);
    if path.exists() {
        let settings: RecognitionSettings = common_settings::load_settings(&path)?;
        let normalized = super::normalize_settings(settings.clone());
        if normalized != settings {
            common_settings::save_settings(&path, &normalized)?;
        }
        return Ok(normalized);
    }

    let legacy_path = config_dir.join(LEGACY_SETTINGS_FILE);
    if !legacy_path.exists() {
        return Ok(RecognitionSettings::default());
    }

    let settings: RecognitionSettings = common_settings::load_settings(&legacy_path)?;
    let normalized = super::normalize_settings(settings);
    common_settings::save_settings(&path, &normalized)?;
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
    let path = config_dir.join(SETTINGS_FILE);
    common_settings::save_settings(&path, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_settings_recovers_corrupt_current_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE);
        std::fs::write(&path, "{ broken json").unwrap();

        let loaded = read_settings_from_dir(temp_dir.path()).unwrap();

        assert_eq!(loaded, RecognitionSettings::default());
        let backups = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("recognition_settings.json.corrupt-")
            })
            .count();
        assert_eq!(backups, 1);
    }
}
