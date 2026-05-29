use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_press_jitter_min_ms() -> u64 {
    8
}

fn default_press_jitter_max_ms() -> u64 {
    12
}

pub(crate) fn default_compensation_delay_min_ms() -> u64 {
    100
}

pub(crate) fn default_compensation_delay_max_ms() -> u64 {
    150
}

pub(crate) fn default_min_press_spacing_ms() -> u64 {
    80
}

/// 连发器卡片
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RapidfireCard {
    pub id: String,
    pub name: String,
    /// 触发键（单键或组合键，如 F1、Shift+-）
    pub trigger_key: String,
    /// 目标键（连发时触发）
    pub target_key: String,
    /// 连发间隔（毫秒，最小 10）
    pub interval_ms: u64,
    /// 目标键按下保持时间抖动下限（毫秒）
    #[serde(default = "default_press_jitter_min_ms")]
    pub press_jitter_min_ms: u64,
    /// 目标键按下保持时间抖动上限（毫秒）
    #[serde(default = "default_press_jitter_max_ms")]
    pub press_jitter_max_ms: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 连发器设置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RapidfireSettings {
    #[serde(default)]
    pub version: u32,
    /// 连发器总开关
    #[serde(default)]
    pub rapidfire_enabled: bool,
    /// 是否显示透明窗口
    #[serde(default)]
    pub show_overlay: bool,
    /// 透明窗口位置
    #[serde(default)]
    pub overlay_position: Option<RapidfireRect>,
    /// 透明窗口宽度 (320-800)
    #[serde(default = "default_overlay_width")]
    pub overlay_width: i32,
    /// 松开触发键后补齐奇数次数前的随机等待下限（毫秒）
    #[serde(default = "default_compensation_delay_min_ms")]
    pub compensation_delay_min_ms: u64,
    /// 松开触发键后补齐奇数次数前的随机等待上限（毫秒）
    #[serde(default = "default_compensation_delay_max_ms")]
    pub compensation_delay_max_ms: u64,
    /// 所有连发会话共享的目标键最小触发间距（毫秒）
    #[serde(default = "default_min_press_spacing_ms")]
    pub min_press_spacing_ms: u64,
    #[serde(default)]
    pub cards: Vec<RapidfireCard>,
}

fn default_overlay_width() -> i32 {
    400
}

impl Default for RapidfireSettings {
    fn default() -> Self {
        Self {
            version: 1,
            rapidfire_enabled: false,
            show_overlay: false,
            overlay_position: None,
            overlay_width: 400,
            compensation_delay_min_ms: default_compensation_delay_min_ms(),
            compensation_delay_max_ms: default_compensation_delay_max_ms(),
            min_press_spacing_ms: default_min_press_spacing_ms(),
            cards: vec![RapidfireCard {
                id: format!("rapidfire-{}", crate::utils::now_ms()),
                name: "连发器 1".to_string(),
                trigger_key: "F6".to_string(),
                target_key: "Space".to_string(),
                interval_ms: 100,
                press_jitter_min_ms: default_press_jitter_min_ms(),
                press_jitter_max_ms: default_press_jitter_max_ms(),
                enabled: false,
            }],
        }
    }
}

/// 透明窗口位置/尺寸矩形
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RapidfireRect {
    pub x: i32,
    pub y: i32,
}

/// 连发器运行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RapidfireRunStatus {
    Idle,
    Firing,
    PendingCompensation,
}

/// 单张卡片的运行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RapidfireRunState {
    /// 关联的卡片 ID
    pub card_id: String,
    pub status: RapidfireRunStatus,
    pub count: u64,
}

/// 初始状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RapidfireBootstrap {
    pub settings: RapidfireSettings,
    pub runs: Vec<RapidfireRunState>,
    pub hotkey_error: Option<String>,
}

/// 位置选择结果
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RapidfireSelectionOutcome {
    pub kind: RapidfireSelectionKind,
    pub position: RapidfireRect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RapidfireSelectionKind {
    Selected,
    Cancelled,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapidfire_settings_default_values_are_stable() {
        let settings = RapidfireSettings::default();

        assert_eq!(settings.version, 1);
        assert!(!settings.rapidfire_enabled);
        assert!(!settings.show_overlay);
        assert!(settings.overlay_position.is_none());
        assert_eq!(settings.overlay_width, 400);
        assert_eq!(settings.compensation_delay_min_ms, 100);
        assert_eq!(settings.compensation_delay_max_ms, 150);
        assert_eq!(settings.min_press_spacing_ms, 80);
        assert_eq!(settings.cards.len(), 1);
        assert_eq!(settings.cards[0].trigger_key, "F6");
        assert_eq!(settings.cards[0].target_key, "Space");
        assert_eq!(settings.cards[0].interval_ms, 100);
        assert_eq!(settings.cards[0].press_jitter_min_ms, 8);
        assert_eq!(settings.cards[0].press_jitter_max_ms, 12);
        assert!(!settings.cards[0].enabled);
    }

    #[test]
    fn rapidfire_card_deserializes_legacy_jitter_defaults() {
        let card: RapidfireCard = serde_json::from_value(serde_json::json!({
            "id": "legacy",
            "name": "旧配置",
            "triggerKey": "F6",
            "targetKey": "Space",
            "intervalMs": 100,
            "enabled": true
        }))
        .expect("旧卡片配置应补齐默认抖动");

        assert_eq!(card.press_jitter_min_ms, 8);
        assert_eq!(card.press_jitter_max_ms, 12);
    }

    #[test]
    fn rapidfire_settings_deserializes_legacy_global_delay_defaults() {
        let settings: RapidfireSettings = serde_json::from_value(serde_json::json!({
            "version": 1,
            "rapidfireEnabled": true,
            "showOverlay": true,
            "overlayPosition": null,
            "overlayWidth": 420,
            "cards": []
        }))
        .expect("旧连发器设置应补齐全局延迟默认值");

        assert_eq!(settings.compensation_delay_min_ms, 100);
        assert_eq!(settings.compensation_delay_max_ms, 150);
        assert_eq!(settings.min_press_spacing_ms, 80);
    }
}
