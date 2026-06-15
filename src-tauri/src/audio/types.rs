use serde::{Deserialize, Serialize};

use crate::morse::types::RegionRect;

fn default_audio_enabled() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_volume() -> f32 {
    0.8
}

fn default_cooldown_ms() -> u32 {
    1000
}

fn default_watch_match_threshold() -> f32 {
    0.9
}

fn default_watch_poll_interval_ms() -> u32 {
    500
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioSettings {
    #[serde(default = "default_audio_enabled")]
    pub audio_enabled: bool,
    #[serde(default)]
    pub cards: Vec<AudioCard>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            audio_enabled: true,
            cards: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioCard {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub trigger_mode: AudioTriggerMode,
    // 快捷键模式
    #[serde(default)]
    pub hotkey: Option<String>,
    // 区域监听模式
    #[serde(default)]
    pub watch_region: Option<RegionRect>,
    #[serde(default)]
    pub watch_reference_image_path: Option<String>,
    #[serde(default = "default_watch_match_threshold")]
    pub watch_match_threshold: f32,
    #[serde(default = "default_watch_poll_interval_ms")]
    pub watch_poll_interval_ms: u32,
    // 通用
    #[serde(default)]
    pub audio_file_path: String,
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default = "default_cooldown_ms")]
    pub cooldown_ms: u32,
    /// 允许此卡片的音频与其他卡片同时播放（默认互斥）
    #[serde(default)]
    pub allow_simultaneous: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioTriggerMode {
    Hotkey,
    RegionWatch,
}

impl Default for AudioTriggerMode {
    fn default() -> Self {
        Self::Hotkey
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioBootstrap {
    pub settings: AudioSettings,
    pub hotkey_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_settings_default_values_are_stable() {
        let settings = AudioSettings::default();
        assert!(settings.audio_enabled);
        assert!(settings.cards.is_empty());
    }

    #[test]
    fn audio_card_deserialize_with_defaults() {
        let json = r#"{"id":"c1","name":"测试"}"#;
        let card: AudioCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.id, "c1");
        assert!(card.enabled);
        assert_eq!(card.trigger_mode, AudioTriggerMode::Hotkey);
        assert_eq!(card.volume, 0.8);
        assert_eq!(card.cooldown_ms, 1000);
        assert!(!card.allow_simultaneous);
    }

    #[test]
    fn audio_card_allow_simultaneous_roundtrip() {
        let json = r#"{"id":"c2","name":"并发播放","allowSimultaneous":true}"#;
        let card: AudioCard = serde_json::from_str(json).unwrap();
        assert!(card.allow_simultaneous);
        let reserialized = serde_json::to_string(&card).unwrap();
        assert!(reserialized.contains("\"allowSimultaneous\":true"));
    }
}
