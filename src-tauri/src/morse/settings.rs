use std::{fs, path::{Path, PathBuf}};

use tauri::{AppHandle, Manager};

use super::types::MorseSettings;

const SETTINGS_FILE_NAME: &str = "morse_settings.json";

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法解析配置目录: {error}"))?;

    ensure_config_dir(&config_dir)?;

    Ok(config_dir.join(SETTINGS_FILE_NAME))
}

fn ensure_config_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("无法创建配置目录: {error}"))
}

fn deserialize_settings(content: &str, path: &Path) -> Result<MorseSettings, String> {
    serde_json::from_str::<MorseSettings>(content)
        .map_err(|error| format!("无法解析配置文件 {}: {error}", path.display()))
}

fn read_settings_from_path(path: &Path) -> Result<MorseSettings, String> {
    if !path.exists() {
        return Ok(MorseSettings::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|error| format!("无法读取配置文件 {}: {error}", path.display()))?;

    deserialize_settings(&content, path)
}

fn write_settings_to_path(path: &Path, settings: &MorseSettings) -> Result<(), String> {
    let content = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("无法序列化设置: {error}"))?;

    fs::write(path, content)
        .map_err(|error| format!("无法写入配置文件 {}: {error}", path.display()))
}

pub fn load_settings(app: &AppHandle) -> Result<MorseSettings, String> {
    let path = settings_path(app)?;
    read_settings_from_path(&path)
}

pub fn save_settings(app: &AppHandle, settings: &MorseSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    write_settings_to_path(&path, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_settings() -> MorseSettings {
        MorseSettings {
            hotkey: "Ctrl+F1".to_string(),
            regions: [None, None, None],
            binary_threshold: 100,
            auto_input_delay: 25,
        }
    }

    #[test]
    fn read_settings_returns_default_when_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);

        let settings = read_settings_from_path(&path).unwrap();
        assert_eq!(settings.hotkey, MorseSettings::default().hotkey);
        assert_eq!(settings.binary_threshold, MorseSettings::default().binary_threshold);
    }

    #[test]
    fn write_and_read_settings_round_trip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        let settings = sample_settings();

        write_settings_to_path(&path, &settings).unwrap();
        let loaded = read_settings_from_path(&path).unwrap();

        assert_eq!(loaded.hotkey, settings.hotkey);
        assert_eq!(loaded.binary_threshold, settings.binary_threshold);
        assert_eq!(loaded.auto_input_delay, settings.auto_input_delay);
    }

    #[test]
    fn deserialize_settings_reports_invalid_json() {
        let path = PathBuf::from(SETTINGS_FILE_NAME);
        let error = deserialize_settings("{not-json}", &path).unwrap_err();
        assert!(error.contains("无法解析配置文件"));
    }

    #[test]
    fn ensure_config_dir_creates_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target = temp_dir.path().join("nested").join("config");
        ensure_config_dir(&target).unwrap();
        assert!(target.exists());
    }
}
