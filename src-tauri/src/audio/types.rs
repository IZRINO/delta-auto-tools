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
    0.75
}

fn default_watch_poll_interval_ms() -> u32 {
    500
}

fn default_color_tolerance() -> u8 {
    30
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
    /// 音频文件列表（按序）。连杀顺序=数组顺序；单文件时为单元素数组。
    #[serde(default)]
    pub audio_files: Vec<String>,
    /// 仅用于反序列化旧 JSON 的单值 audioFilePath 字段；normalize_settings 迁移进 audio_files 后清空。
    /// 序列化时跳过，不输出到新 JSON。
    #[serde(default, rename = "audioFilePath", skip_serializing)]
    pub legacy_audio_file_path: Option<String>,
    /// 播放方式：Single/Combo/Random
    #[serde(default)]
    pub play_mode: PlayMode,
    /// 连杀窗口（毫秒），从上一次触发起算；超时复位第一首。默认 60000。
    #[serde(default = "default_combo_window_ms")]
    pub combo_window_ms: u32,
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default = "default_cooldown_ms")]
    pub cooldown_ms: u32,
    /// 允许此卡片的音频与其他卡片同时播放（默认互斥）
    #[serde(default)]
    pub allow_simultaneous: bool,
    // 识色模式探针列表
    #[serde(default)]
    pub color_probes: Vec<ColorProbe>,
    // 识色聚合模式
    #[serde(default)]
    pub color_match_mode: ColorMatchMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioTriggerMode {
    Hotkey,
    RegionWatch,
    ColorWatch,
}

impl Default for AudioTriggerMode {
    fn default() -> Self {
        Self::Hotkey
    }
}

/// 音频播放方式：叠加在触发模式之上的文件选择策略。
/// Single=单文件；Combo=连杀（窗口内按序递增，末首后保持，超时复位）；Random=随机（不重复上一次）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum PlayMode {
    #[default]
    Single,
    Combo,
    Random,
}

fn default_combo_window_ms() -> u32 {
    60000
}

/// 识色探针：一个矩形区域 + 目标颜色 + 容差
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ColorProbe {
    pub region: RegionRect,
    /// 目标 RGB 颜色 [R, G, B]，每通道 0-255
    pub target_color: [u8; 3],
    /// 颜色容差（RGB 欧氏距离阈值，0-255）
    #[serde(default = "default_color_tolerance")]
    pub tolerance: u8,
}

/// 多探针聚合模式：All = 全部命中才触发；Any = 任一命中即触发
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ColorMatchMode {
    #[default]
    All,
    Any,
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
        assert_eq!(card.watch_match_threshold, 0.75);
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

    #[test]
    fn color_probe_roundtrip() {
        let json = r#"{"region":{"x":10,"y":20,"width":5,"height":5},"targetColor":[200,100,50],"tolerance":40}"#;
        let probe: ColorProbe = serde_json::from_str(json).unwrap();
        assert_eq!(probe.region.x, 10);
        assert_eq!(probe.target_color, [200, 100, 50]);
        assert_eq!(probe.tolerance, 40);
        let reserialized = serde_json::to_string(&probe).unwrap();
        assert!(reserialized.contains("\"targetColor\":[200,100,50]"));
        assert!(reserialized.contains("\"tolerance\":40"));
    }

    #[test]
    fn color_probe_default_tolerance_is_30() {
        // 缺省 tolerance 应默认 30
        let json = r#"{"region":{"x":0,"y":0,"width":3,"height":3},"targetColor":[0,0,0]}"#;
        let probe: ColorProbe = serde_json::from_str(json).unwrap();
        assert_eq!(probe.tolerance, 30);
    }

    #[test]
    fn color_match_mode_default_is_all() {
        // AudioCard 缺省 color_match_mode 应为 All
        let json = r#"{"id":"c1","name":"测试","triggerMode":"colorWatch"}"#;
        let card: AudioCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.color_match_mode, ColorMatchMode::All);
        assert!(card.color_probes.is_empty());
    }

    #[test]
    fn audio_card_with_color_watch_roundtrip() {
        let card = AudioCard {
            id: "c1".into(),
            name: "识色卡".into(),
            enabled: true,
            trigger_mode: AudioTriggerMode::ColorWatch,
            hotkey: None,
            watch_region: None,
            watch_reference_image_path: None,
            watch_match_threshold: 0.75,
            watch_poll_interval_ms: 500,
            audio_files: vec!["a.mp3".into()],
            legacy_audio_file_path: None,
            play_mode: PlayMode::Combo,
            combo_window_ms: 60000,
            volume: 0.8,
            cooldown_ms: 1000,
            allow_simultaneous: false,
            color_probes: vec![ColorProbe {
                region: RegionRect { x: 1, y: 2, width: 3, height: 4 },
                target_color: [10, 20, 30],
                tolerance: 25,
            }],
            color_match_mode: ColorMatchMode::Any,
        };
        let json = serde_json::to_string(&card).unwrap();
        // 序列化不应输出兼容字段 audioFilePath
        assert!(!json.contains("audioFilePath"));
        assert!(json.contains("\"audioFiles\""));
        let back: AudioCard = serde_json::from_str(&json).unwrap();
        assert_eq!(back.trigger_mode, AudioTriggerMode::ColorWatch);
        assert_eq!(back.color_match_mode, ColorMatchMode::Any);
        assert_eq!(back.color_probes.len(), 1);
        assert_eq!(back.color_probes[0].target_color, [10, 20, 30]);
        assert_eq!(back.audio_files, vec!["a.mp3".to_string()]);
        assert_eq!(back.play_mode, PlayMode::Combo);
        assert_eq!(back.combo_window_ms, 60000);
    }

    #[test]
    fn audio_card_legacy_audio_file_path_deserialized() {
        // 旧 JSON 的 audioFilePath 单值字段应能被反序列化到 legacy_audio_file_path
        let json = r#"{"id":"c1","name":"旧卡","audioFilePath":"old.mp3"}"#;
        let card: AudioCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.legacy_audio_file_path, Some("old.mp3".to_string()));
        assert!(card.audio_files.is_empty());
        assert_eq!(card.play_mode, PlayMode::Single);
        assert_eq!(card.combo_window_ms, 60000);
    }
}
