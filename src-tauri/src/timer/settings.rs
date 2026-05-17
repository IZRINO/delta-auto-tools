use std::{
    fs,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Manager};

use super::types::TimerSettings;

const SETTINGS_FILE_NAME: &str = "timer_settings.json";

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

fn deserialize_settings(content: &str, path: &Path) -> Result<TimerSettings, String> {
    serde_json::from_str::<TimerSettings>(content)
        .map_err(|error| format!("无法解析计时器配置文件 {}: {error}", path.display()))
}

fn read_settings_from_path(path: &Path) -> Result<TimerSettings, String> {
    if !path.exists() {
        return Ok(TimerSettings::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|error| format!("无法读取计时器配置文件 {}: {error}", path.display()))?;

    deserialize_settings(&content, path)
}

fn write_settings_to_path(path: &Path, settings: &TimerSettings) -> Result<(), String> {
    let content = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("无法序列化计时器设置: {error}"))?;

    fs::write(path, content)
        .map_err(|error| format!("无法写入计时器配置文件 {}: {error}", path.display()))
}

pub fn load_settings(app: &AppHandle) -> Result<TimerSettings, String> {
    let path = settings_path(app)?;
    read_settings_from_path(&path)
}

pub fn save_settings(app: &AppHandle, settings: &TimerSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    write_settings_to_path(&path, settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::types::{TimerDisplaySettings, TimerItem, TimerRect};

    fn sample_settings() -> TimerSettings {
        TimerSettings {
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
            timers: vec![TimerItem {
                id: "alpha".to_string(),
                name: "测试计时器".to_string(),
                duration_seconds: 300,
                hotkey: "Ctrl+F2".to_string(),
            }],
        }
    }

    #[test]
    fn read_settings_returns_default_when_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);

        let settings = read_settings_from_path(&path).unwrap();
        assert_eq!(settings.timers[0].duration_seconds, TimerSettings::default().timers[0].duration_seconds);
    }

    #[test]
    fn write_and_read_settings_round_trip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(SETTINGS_FILE_NAME);
        let settings = sample_settings();

        write_settings_to_path(&path, &settings).unwrap();
        let loaded = read_settings_from_path(&path).unwrap();

        assert_eq!(loaded.enabled, settings.enabled);
        assert_eq!(loaded.display.rect, settings.display.rect);
        assert_eq!(loaded.timers, settings.timers);
    }

    #[test]
    fn deserialize_settings_reports_invalid_json() {
        let path = PathBuf::from(SETTINGS_FILE_NAME);
        let error = deserialize_settings("{not-json}", &path).unwrap_err();
        assert!(error.contains("无法解析计时器配置文件"));
    }
}
