use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegionRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// 点击区域配置，包含区域坐标和独立延迟
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClickRegion {
    pub rect: RegionRect,
    /// 点击此区域前的延迟（毫秒）
    #[serde(default = "default_click_delay")]
    pub delay_ms: u64,
}

fn default_click_delay() -> u64 {
    500
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MorseSettings {
    pub hotkey: String,
    pub regions: [Option<RegionRect>; 3],
    pub binary_threshold: u8,
    pub auto_input_delay: u64,
    /// 自动点击整组成功完成后按一次；None 表示不执行
    #[serde(default)]
    pub after_click_hotkey: Option<String>,
    /// 识别成功后自动点击已配置区域
    #[serde(default)]
    pub auto_click_enabled: bool,
    /// 点击区域（1~7 个），每个有独立延迟
    #[serde(default)]
    pub click_regions: Vec<ClickRegion>,
}

impl Default for MorseSettings {
    fn default() -> Self {
        Self {
            hotkey: "F1".to_string(),
            regions: [None, None, None],
            binary_threshold: 127,
            auto_input_delay: 50,
            after_click_hotkey: None,
            auto_click_enabled: false,
            click_regions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MorseRegionDetail {
    pub slot: usize,
    pub threshold_mode: String,
    pub contour_count: usize,
    pub morse: Option<String>,
    pub digit: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MorseRunResult {
    pub value: Option<String>,
    pub details: Vec<MorseRegionDetail>,
    pub triggered_by: String,
    pub auto_typed: bool,
    pub occurred_at_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: u64,
    pub result: Option<String>,
    pub success: bool,
    pub triggered_by: String,
    pub auto_typed: bool,
    pub occurred_at_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MorseBootstrap {
    pub settings: MorseSettings,
    pub history: Vec<HistoryEntry>,
    pub latest_run: Option<MorseRunResult>,
    pub hotkey_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionSelectionProgress {
    pub current_slot: Option<usize>,
    pub regions: [Option<RegionRect>; 3],
    pub completed_slots: Vec<usize>,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_regions: Option<Vec<ClickRegion>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionSelectionOutcome {
    pub kind: RegionSelectionKind,
    pub regions: [Option<RegionRect>; 3],
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_regions: Option<Vec<ClickRegion>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RegionSelectionKind {
    Selected,
    Cancelled,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::MorseSettings;

    #[test]
    fn morse_settings_default_values_are_stable() {
        let settings = MorseSettings::default();
        assert_eq!(settings.hotkey, "F1");
        assert_eq!(settings.binary_threshold, 127);
        assert_eq!(settings.auto_input_delay, 50);
        assert_eq!(settings.regions, [None, None, None]);
        assert_eq!(settings.after_click_hotkey, None);
        assert!(settings.click_regions.is_empty());
    }
}
