use serde::{Deserialize, Serialize};

use crate::morse::types::{ClickRegion, RegionRect};

fn default_hotkey() -> String {
    "F6".to_string()
}

fn default_occupancy_threshold() -> f32 {
    80.0
}

fn default_match_threshold() -> f32 {
    0.55
}

fn default_auto_click_enabled() -> bool {
    true
}

fn default_click_delay_ms() -> u64 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintPerson {
    pub id: String,
    pub name: String,
    pub name_image_path: String,
    #[serde(default)]
    pub fingerprint_paths: [Option<String>; 8],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintSettings {
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default)]
    pub name_region: Option<RegionRect>,
    #[serde(default)]
    pub candidate_boxes: [Option<RegionRect>; 9],
    #[serde(default)]
    pub archive_slots: [Option<RegionRect>; 8],
    #[serde(default = "default_occupancy_threshold")]
    pub occupancy_threshold: f32,
    #[serde(default = "default_match_threshold")]
    pub match_threshold: f32,
    #[serde(default = "default_auto_click_enabled")]
    pub auto_click_enabled: bool,
    #[serde(default = "default_click_delay_ms")]
    pub click_delay_ms: u64,
    /// 自动点击整组成功完成后按一次；None 表示不执行
    #[serde(default)]
    pub after_click_hotkey: Option<String>,
    /// 九宫格点击后再点的指定区域（1~7 个），每个有独立延迟
    #[serde(default)]
    pub click_regions: Vec<ClickRegion>,
    #[serde(default)]
    pub people: Vec<FingerprintPerson>,
}

impl Default for FingerprintSettings {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey(),
            name_region: None,
            candidate_boxes: Default::default(),
            archive_slots: Default::default(),
            occupancy_threshold: default_occupancy_threshold(),
            match_threshold: default_match_threshold(),
            auto_click_enabled: default_auto_click_enabled(),
            click_delay_ms: default_click_delay_ms(),
            after_click_hotkey: None,
            click_regions: Vec::new(),
            people: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintMatch {
    pub template_index: usize,
    pub candidate_index: usize,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintRunResult {
    pub person_id: Option<String>,
    pub person_name: Option<String>,
    pub mode: Option<String>,
    pub occupied_count: Option<usize>,
    pub matches: Vec<FingerprintMatch>,
    pub clicked: bool,
    pub triggered_by: String,
    pub occurred_at_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: u64,
    pub person_name: Option<String>,
    pub mode: Option<String>,
    pub success: bool,
    pub triggered_by: String,
    pub occurred_at_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintBootstrap {
    pub settings: FingerprintSettings,
    pub history: Vec<HistoryEntry>,
    pub latest_run: Option<FingerprintRunResult>,
    pub hotkey_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegionSelectionProgress {
    pub current_slot: Option<usize>,
    pub completed_slots: Vec<usize>,
    pub target: String,
    pub rects: Vec<Option<RegionRect>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegionSelectionOutcome {
    pub kind: RegionSelectionKind,
    pub target: String,
    pub rects: Vec<Option<RegionRect>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RegionSelectionKind {
    Selected,
    Cancelled,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_stable() {
        let settings = FingerprintSettings::default();
        assert_eq!(settings.hotkey, "F6");
        assert!(settings.auto_click_enabled);
        assert_eq!(settings.click_delay_ms, 50);
        assert_eq!(settings.after_click_hotkey, None);
        assert!(settings.click_regions.is_empty());
        assert_eq!(settings.candidate_boxes.len(), 9);
        assert_eq!(settings.archive_slots.len(), 8);
        assert!(settings.people.is_empty());
    }

    #[test]
    fn settings_round_trip_camel_case() {
        let mut settings = FingerprintSettings::default();
        settings.people.push(FingerprintPerson {
            id: "p1".into(),
            name: "克莱尔".into(),
            name_image_path: "n.png".into(),
            fingerprint_paths: Default::default(),
        });
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("nameRegion"));
        assert!(json.contains("candidateBoxes"));
        assert!(json.contains("fingerprintPaths"));
        let loaded: FingerprintSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.people[0].name, "克莱尔");
    }
}
