use tauri::AppHandle;

use super::types::MorseSettings;
use crate::settings;

const SETTINGS_FILE_NAME: &str = "morse_settings.json";

pub fn load_settings(app: &AppHandle) -> Result<MorseSettings, String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::load_settings(&path)
}

pub fn save_settings(app: &AppHandle, settings_value: &MorseSettings) -> Result<(), String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::save_settings(&path, settings_value)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn sample_settings() -> MorseSettings {
        MorseSettings {
            hotkey: "Ctrl+F1".to_string(),
            regions: [None, None, None],
            binary_threshold: 100,
            auto_input_delay: 25,
            auto_click_enabled: false,
            auto_click_delay_ms: 500,
            click_regions: Default::default(),
        }
    }

    #[test]
    fn read_settings_returns_default_when_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);

        let loaded = settings::load_settings::<MorseSettings>(&path).unwrap();
        assert_eq!(loaded.hotkey, MorseSettings::default().hotkey);
        assert_eq!(
            loaded.binary_threshold,
            MorseSettings::default().binary_threshold
        );
    }

    #[test]
    fn write_and_read_settings_round_trip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        let s = sample_settings();

        settings::save_settings(&path, &s).unwrap();
        let loaded = settings::load_settings::<MorseSettings>(&path).unwrap();

        assert_eq!(loaded.hotkey, s.hotkey);
        assert_eq!(loaded.binary_threshold, s.binary_threshold);
        assert_eq!(loaded.auto_input_delay, s.auto_input_delay);
    }

    #[test]
    fn deserialize_settings_reports_invalid_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        fs::write(&path, "{not-json}").unwrap();

        let error = settings::load_settings::<MorseSettings>(&path).unwrap_err();
        assert!(error.contains("无法解析配置文件"));
    }

    #[test]
    fn ensure_config_dir_creates_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target = temp_dir.path().join("nested").join("config");
        settings::ensure_config_dir(&target).unwrap();
        assert!(target.exists());
    }
}
