//! `profile_settings.json` 持久化，复用公共 `crate::settings` 工具函数。

use tauri::AppHandle;

use super::types::ProfileSettings;
use crate::settings;

const SETTINGS_FILE_NAME: &str = "profile_settings.json";

pub fn load_settings(app: &AppHandle) -> Result<ProfileSettings, String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    Ok(settings::load_settings::<ProfileSettings>(&path)?)
}

pub fn save_settings(app: &AppHandle, settings_value: &ProfileSettings) -> Result<(), String> {
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
        let loaded = settings::load_settings::<ProfileSettings>(&path).unwrap();
        assert!(loaded.profiles.is_empty());
        assert_eq!(loaded.active_profile_id, "");
        assert_eq!(loaded.next_profile_number, 1);
    }

    #[test]
    fn round_trip_preserves_data() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        let s = ProfileSettings {
            profiles: vec![super::super::types::Profile {
                id: "p1".to_string(),
                name: "PVE".to_string(),
                created_at: 1,
                updated_at: 2,
                snapshot: super::super::types::ToolSettingsSnapshot::empty(),
            }],
            active_profile_id: "p1".to_string(),
            next_profile_number: 1,
        };
        settings::save_settings(&path, &s).unwrap();
        let loaded = settings::load_settings::<ProfileSettings>(&path).unwrap();
        assert_eq!(loaded, s);
    }
}
