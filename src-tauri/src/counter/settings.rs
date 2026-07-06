use tauri::AppHandle;

use super::types::CounterSettings;
use crate::settings;

const SETTINGS_FILE_NAME: &str = "counter_settings.json";

pub fn load_settings(app: &AppHandle) -> Result<CounterSettings, String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::load_settings(&path)
}

pub fn save_settings(app: &AppHandle, settings_value: &CounterSettings) -> Result<(), String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::save_settings(&path, settings_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::counter::types::{
        CounterDisplaySettings, CounterGroup, CounterItem, CounterRect, DEFAULT_COUNTER_GROUP_ID,
    };

    fn sample_settings() -> CounterSettings {
        CounterSettings {
            enabled: true,
            counter_enabled: true,
            display: CounterDisplaySettings {
                rect: CounterRect {
                    x: 330,
                    y: 20,
                    width: 320,
                    height: 120,
                },
                font_opacity: 0.8,
            },
            counter_groups: vec![CounterGroup {
                id: DEFAULT_COUNTER_GROUP_ID.to_string(),
                name: "默认分组".to_string(),
                enabled: true,
                display: CounterDisplaySettings {
                    rect: CounterRect {
                        x: 330,
                        y: 20,
                        width: 320,
                        height: 120,
                    },
                    font_opacity: 0.8,
                },
            }],
            counters: vec![CounterItem {
                id: "counter-alpha".to_string(),
                group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
                name: "测试计数器".to_string(),
                start_value: 5,
                hotkey: "Ctrl+F3".to_string(),
                enabled: true,
            }],
        }
    }

    #[test]
    fn read_settings_returns_default_when_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);

        let loaded = settings::load_settings::<CounterSettings>(&path).unwrap();
        assert_eq!(
            loaded.counters[0].start_value,
            CounterSettings::default().counters[0].start_value
        );
    }

    #[test]
    fn write_and_read_settings_round_trip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        let s = sample_settings();

        settings::save_settings(&path, &s).unwrap();
        let loaded = settings::load_settings::<CounterSettings>(&path).unwrap();

        assert_eq!(loaded.enabled, s.enabled);
        assert_eq!(loaded.display.rect, s.display.rect);
        assert_eq!(loaded.counters, s.counters);
    }

    #[test]
    fn deserialize_settings_recovers_invalid_json() {
        use std::fs;

        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        fs::write(&path, "{not-json}").unwrap();

        let loaded = settings::load_settings::<CounterSettings>(&path).unwrap();
        assert_eq!(
            loaded.counter_enabled,
            CounterSettings::default().counter_enabled
        );
        assert!(!path.exists());
        assert_eq!(temp_dir.path().read_dir().unwrap().count(), 1);
    }
}
