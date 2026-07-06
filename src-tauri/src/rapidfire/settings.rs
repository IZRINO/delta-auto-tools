use tauri::AppHandle;

use super::types::RapidfireSettings;
use crate::settings;

const SETTINGS_FILE_NAME: &str = "rapidfire_settings.json";

pub fn load_settings(app: &AppHandle) -> Result<RapidfireSettings, String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    let mut settings_value: RapidfireSettings = settings::load_settings(&path)?;

    // 迁移旧版本：确保默认卡片存在
    if settings_value.cards.is_empty() {
        settings_value.cards.push(
            RapidfireSettings::default()
                .cards
                .into_iter()
                .next()
                .unwrap(),
        );
    }

    Ok(settings_value)
}

pub fn save_settings(app: &AppHandle, settings_value: &RapidfireSettings) -> Result<(), String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::save_settings(&path, settings_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rapidfire::types::{
        RapidfireCard, RapidfireGroup, RapidfireRect, DEFAULT_RAPIDFIRE_GROUP_ID,
    };

    fn sample_settings() -> RapidfireSettings {
        RapidfireSettings {
            version: 1,
            rapidfire_enabled: true,
            show_overlay: true,
            overlay_position: Some(RapidfireRect { x: 100, y: 200 }),
            overlay_width: 500,
            compensation_delay_min_ms: 120,
            compensation_delay_max_ms: 180,
            min_press_spacing_ms: 90,
            trigger_jitter_max_ms: 0,
            cancel_jitter_on_release: true,
            groups: vec![RapidfireGroup {
                id: DEFAULT_RAPIDFIRE_GROUP_ID.to_string(),
                name: "默认分组".to_string(),
                enabled: true,
                show_overlay: true,
                overlay_position: Some(RapidfireRect { x: 100, y: 200 }),
                overlay_width: 500,
            }],
            cards: vec![RapidfireCard {
                id: "rf-test".to_string(),
                group_id: DEFAULT_RAPIDFIRE_GROUP_ID.to_string(),
                name: "测试连发器".to_string(),
                trigger_key: "F1".to_string(),
                target_key: "1".to_string(),
                interval_ms: 50,
                press_jitter_min_ms: 10,
                press_jitter_max_ms: 18,
                min_press_spacing_ms: 90,
                trigger_jitter_max_ms: 0,
                cancel_jitter_on_release: true,
                enabled: true,
                skip_compensation: false,
                ignore_trigger_key: false,
            }],
        }
    }

    #[test]
    fn read_settings_returns_default_when_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);

        let loaded = settings::load_settings::<RapidfireSettings>(&path).unwrap();
        assert_eq!(loaded.cards.len(), 1);
        assert_eq!(loaded.cards[0].trigger_key, "F6");
    }

    #[test]
    fn write_and_read_settings_round_trip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        let s = sample_settings();

        settings::save_settings(&path, &s).unwrap();
        let loaded = settings::load_settings::<RapidfireSettings>(&path).unwrap();

        assert_eq!(loaded.rapidfire_enabled, s.rapidfire_enabled);
        assert_eq!(loaded.show_overlay, s.show_overlay);
        assert_eq!(loaded.overlay_position, s.overlay_position);
        assert_eq!(loaded.overlay_width, s.overlay_width);
        assert_eq!(
            loaded.compensation_delay_min_ms,
            s.compensation_delay_min_ms
        );
        assert_eq!(
            loaded.compensation_delay_max_ms,
            s.compensation_delay_max_ms
        );
        assert_eq!(loaded.min_press_spacing_ms, s.min_press_spacing_ms);
        assert_eq!(loaded.cards, s.cards);
    }

    #[test]
    fn deserialize_settings_recovers_invalid_json() {
        use std::fs;

        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        fs::write(&path, "{not-json}").unwrap();

        let loaded = settings::load_settings::<RapidfireSettings>(&path).unwrap();
        assert_eq!(
            loaded.rapidfire_enabled,
            RapidfireSettings::default().rapidfire_enabled
        );
        assert!(!path.exists());
        assert_eq!(temp_dir.path().read_dir().unwrap().count(), 1);
    }
}
