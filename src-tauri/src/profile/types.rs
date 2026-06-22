//! 多配置 Profile 前端类型定义。
//!
//! 一个 Profile = 5 份工具 settings 的快照（morse/timer/counter/rapidfire/audio），
//! 切换 Profile 时一次性写盘 5 份 `*_settings.json` 并让各工具 reload 内存状态。
//! 主题独立于 Profile，不打包进快照。

use serde::{Deserialize, Serialize};

use crate::audio::AudioSettings;
use crate::counter::CounterSettings;
use crate::morse::MorseSettings;
use crate::rapidfire::RapidfireSettings;
use crate::timer::TimerSettings;

/// 单个工具的配置快照。
///
/// `Option` 表示快照是否包含该工具的配置；当前实现中 5 个工具都会被打包，
/// 用 `Option` 是为了未来支持「仅切换部分工具」的灵活场景。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolSettingsSnapshot {
    #[serde(default)]
    pub morse: Option<MorseSettings>,
    #[serde(default)]
    pub timer: Option<TimerSettings>,
    #[serde(default)]
    pub counter: Option<CounterSettings>,
    #[serde(default)]
    pub rapidfire: Option<RapidfireSettings>,
    #[serde(default)]
    pub audio: Option<AudioSettings>,
}

impl ToolSettingsSnapshot {
    /// 创建空快照（5 个工具都为 None）。
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            morse: None,
            timer: None,
            counter: None,
            rapidfire: None,
            audio: None,
        }
    }
}

/// 一个完整 Profile。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// 唯一 id，由 `now_ms` + 短随机串生成。
    pub id: String,
    /// 显示名。
    pub name: String,
    /// 创建时间 unix 毫秒。
    pub created_at: u64,
    /// 最后更新时间 unix 毫秒。
    pub updated_at: u64,
    /// 5 份工具 settings 快照。
    pub snapshot: ToolSettingsSnapshot,
}

/// Profile 持久化设置，存到 `profile_settings.json`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSettings {
    /// 全部 Profile 列表。
    #[serde(default)]
    pub profiles: Vec<Profile>,
    /// 当前激活 Profile id。空串表示「默认」（未保存的现场）。
    #[serde(default)]
    pub active_profile_id: String,
    /// 下一次自动创建 `配置N` 时使用的编号。
    #[serde(default = "default_next_profile_number")]
    pub next_profile_number: u32,
}

fn default_next_profile_number() -> u32 {
    1
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            active_profile_id: String::new(),
            next_profile_number: default_next_profile_number(),
        }
    }
}

/// Profile bootstrap：一次性返回前端所需的全部信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileBootstrap {
    /// 全部 Profile 列表。
    pub profiles: Vec<Profile>,
    /// 当前激活 Profile id。
    pub active_profile_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_all_none() {
        let snap = ToolSettingsSnapshot::empty();
        assert!(snap.morse.is_none());
        assert!(snap.timer.is_none());
        assert!(snap.counter.is_none());
        assert!(snap.rapidfire.is_none());
        assert!(snap.audio.is_none());
    }

    #[test]
    fn profile_settings_default_empty() {
        let settings = ProfileSettings::default();
        assert!(settings.profiles.is_empty());
        assert_eq!(settings.active_profile_id, "");
    }

    #[test]
    fn profile_settings_default_next_profile_number_is_one() {
        let settings = ProfileSettings::default();
        assert_eq!(settings.next_profile_number, 1);
    }

    #[test]
    fn profile_settings_missing_next_profile_number_defaults_to_one() {
        let json = r#"{"profiles":[],"activeProfileId":""}"#;
        let loaded: ProfileSettings = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.next_profile_number, 1);
    }

    #[test]
    fn profile_settings_serializes_next_profile_number_camel_case() {
        let settings = ProfileSettings {
            profiles: Vec::new(),
            active_profile_id: String::new(),
            next_profile_number: 7,
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"nextProfileNumber\":7"));
        assert!(!json.contains("next_profile_number"));
    }

    #[test]
    fn profile_serializes_camel_case() {
        let profile = Profile {
            id: "p1".to_string(),
            name: "PVE".to_string(),
            created_at: 1000,
            updated_at: 2000,
            snapshot: ToolSettingsSnapshot::empty(),
        };
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("\"id\""));
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"createdAt\""));
        assert!(json.contains("\"updatedAt\""));
        assert!(json.contains("\"snapshot\""));
        // 不应出现 snake_case
        assert!(!json.contains("\"created_at\""));
        assert!(!json.contains("\"active_profile_id\""));
    }

    #[test]
    fn profile_settings_round_trip() {
        let settings = ProfileSettings {
            profiles: vec![Profile {
                id: "p1".to_string(),
                name: "PVE".to_string(),
                created_at: 1000,
                updated_at: 2000,
                snapshot: ToolSettingsSnapshot::empty(),
            }],
            active_profile_id: "p1".to_string(),
            next_profile_number: 1,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let loaded: ProfileSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, settings);
    }

    #[test]
    fn profile_settings_missing_fields_default() {
        let json = "{}";
        let loaded: ProfileSettings = serde_json::from_str(json).unwrap();
        assert!(loaded.profiles.is_empty());
        assert_eq!(loaded.active_profile_id, "");
        assert_eq!(loaded.next_profile_number, 1);
    }
}
