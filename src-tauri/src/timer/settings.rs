use tauri::AppHandle;

use super::types::TimerSettings;
use crate::settings;

const SETTINGS_FILE_NAME: &str = "timer_settings.json";

pub fn load_settings(app: &AppHandle) -> Result<TimerSettings, String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::load_settings(&path)
}

pub fn save_settings(app: &AppHandle, settings_value: &TimerSettings) -> Result<(), String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    settings::save_settings(&path, settings_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::types::{
        TimerDirection, TimerDisplaySettings, TimerGroup, TimerItem, TimerRect,
        TimerTriggerMode, DEFAULT_TIMER_GROUP_ID,
    };

    fn sample_settings() -> TimerSettings {
        TimerSettings {
            enabled: true,
            timer_enabled: true,
            display: TimerDisplaySettings {
                rect: TimerRect {
                    x: 10,
                    y: 20,
                    width: 320,
                    height: 120,
                },
                font_opacity: 0.75,
            },
            timer_groups: vec![TimerGroup {
                id: DEFAULT_TIMER_GROUP_ID.to_string(),
                name: "默认分组".to_string(),
                enabled: true,
                display: TimerDisplaySettings {
                    rect: TimerRect {
                        x: 10,
                        y: 20,
                        width: 320,
                        height: 120,
                    },
                    font_opacity: 0.75,
                },
            }],
            timers: vec![TimerItem {
                id: "alpha".to_string(),
                group_id: DEFAULT_TIMER_GROUP_ID.to_string(),
                name: "测试计时器".to_string(),
                duration_seconds: 300,
                hotkey: "Ctrl+F2".to_string(),
                direction: TimerDirection::Countdown,
                trigger_mode: TimerTriggerMode::Press,
                enabled: true,
                ignore_running: true,
                segment_count: None,
            }],
        }
    }

    #[test]
    fn read_settings_returns_default_when_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);

        let loaded = settings::load_settings::<TimerSettings>(&path).unwrap();
        assert_eq!(
            loaded.timers[0].duration_seconds,
            TimerSettings::default().timers[0].duration_seconds
        );
    }

    #[test]
    fn write_and_read_settings_round_trip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        let s = sample_settings();

        settings::save_settings(&path, &s).unwrap();
        let loaded = settings::load_settings::<TimerSettings>(&path).unwrap();

        assert_eq!(loaded.enabled, s.enabled);
        assert_eq!(loaded.display.rect, s.display.rect);
        assert_eq!(loaded.timers, s.timers);
    }

    #[test]
    fn deserialize_settings_reports_invalid_json() {
        use std::fs;

        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        fs::write(&path, "{not-json}").unwrap();

        let error = settings::load_settings::<TimerSettings>(&path).unwrap_err();
        assert!(error.contains("无法解析配置文件"));
    }
}
