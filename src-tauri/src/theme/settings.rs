//! `theme_settings.json` 持久化，复用公共 `crate::settings` 工具函数。

use tauri::AppHandle;

use super::types::ThemeSettings;
use crate::settings;

const SETTINGS_FILE_NAME: &str = "theme_settings.json";

pub fn load_settings(app: &AppHandle) -> Result<ThemeSettings, String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::load_settings::<ThemeSettings>(&path)
}

pub fn save_settings(app: &AppHandle, settings_value: &ThemeSettings) -> Result<(), String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::save_settings(&path, settings_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_returns_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        let loaded = settings::load_settings::<ThemeSettings>(&path).unwrap();
        assert_eq!(
            loaded.active_theme_id,
            ThemeSettings::default().active_theme_id
        );
        assert!(loaded.custom_themes.is_empty());
    }

    #[test]
    fn round_trip_preserves_data() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        let s = ThemeSettings {
            active_theme_id: "custom-1".to_string(),
            custom_themes: vec![super::super::types::ThemeDefinition {
                id: "custom-1".to_string(),
                name: "自定义".to_string(),
                builtin: false,
                tokens: Vec::new(),
            }],
            overrides: Vec::new(),
        };
        settings::save_settings(&path, &s).unwrap();
        let loaded = settings::load_settings::<ThemeSettings>(&path).unwrap();
        assert_eq!(loaded, s);
    }
}
