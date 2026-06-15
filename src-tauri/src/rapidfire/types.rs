use serde::{Deserialize, Deserializer, Serialize};

fn default_true() -> bool {
    true
}

fn default_press_jitter_min_ms() -> u64 {
    8
}

fn default_press_jitter_max_ms() -> u64 {
    12
}

fn default_trigger_jitter_max_ms() -> u64 {
    0
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

pub const DEFAULT_RAPIDFIRE_GROUP_ID: &str = "default-rapidfire-group";

fn default_rapidfire_group_id() -> String {
    DEFAULT_RAPIDFIRE_GROUP_ID.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RapidfireGroup {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub show_overlay: bool,
    #[serde(default)]
    pub overlay_position: Option<RapidfireRect>,
    #[serde(default = "default_overlay_width")]
    pub overlay_width: i32,
}

/// 连发器卡片
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RapidfireCard {
    pub id: String,
    pub group_id: String,
    pub name: String,
    /// 触发键（单键或组合键，如 F1、Shift+-）
    pub trigger_key: String,
    /// 目标键（连发时触发）
    pub target_key: String,
    /// 连发间隔（毫秒，最小 1）
    pub interval_ms: u64,
    /// 目标键按下保持时间抖动下限（毫秒）
    pub press_jitter_min_ms: u64,
    /// 目标键按下保持时间抖动上限（毫秒）
    pub press_jitter_max_ms: u64,
    /// 当前卡片目标键最小触发间距（毫秒）
    pub min_press_spacing_ms: u64,
    /// 当前卡片按下触发键后的启动抖动延迟上限（毫秒）
    pub trigger_jitter_max_ms: u64,
    /// 当前卡片抖动期间松手是否立即触发一次并进入补齐判断
    pub cancel_jitter_on_release: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 开启后松开触发键时不执行奇数补齐，单数次数保持单数
    #[serde(default)]
    pub skip_compensation: bool,
    /// 触发过程中是否忽略触发键本身（阻止触发键同步输入）
    #[serde(default)]
    pub ignore_trigger_key: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RapidfireCardInput {
    id: String,
    #[serde(default)]
    group_id: Option<String>,
    name: String,
    trigger_key: String,
    target_key: String,
    interval_ms: u64,
    #[serde(default = "default_press_jitter_min_ms")]
    press_jitter_min_ms: u64,
    #[serde(default = "default_press_jitter_max_ms")]
    press_jitter_max_ms: u64,
    #[serde(default)]
    min_press_spacing_ms: Option<u64>,
    #[serde(default)]
    trigger_jitter_max_ms: Option<u64>,
    #[serde(default)]
    cancel_jitter_on_release: Option<bool>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    skip_compensation: bool,
    #[serde(default)]
    ignore_trigger_key: bool,
}

impl RapidfireCardInput {
    fn into_card_with_defaults(
        self,
        min_press_spacing_ms: u64,
        trigger_jitter_max_ms: u64,
        cancel_jitter_on_release: bool,
    ) -> RapidfireCard {
        RapidfireCard {
            id: self.id,
            group_id: self.group_id.unwrap_or_else(default_rapidfire_group_id),
            name: self.name,
            trigger_key: self.trigger_key,
            target_key: self.target_key,
            interval_ms: self.interval_ms,
            press_jitter_min_ms: self.press_jitter_min_ms,
            press_jitter_max_ms: self.press_jitter_max_ms,
            min_press_spacing_ms: self.min_press_spacing_ms.unwrap_or(min_press_spacing_ms),
            trigger_jitter_max_ms: self.trigger_jitter_max_ms.unwrap_or(trigger_jitter_max_ms),
            cancel_jitter_on_release: self
                .cancel_jitter_on_release
                .unwrap_or(cancel_jitter_on_release),
            enabled: self.enabled,
            skip_compensation: self.skip_compensation,
            ignore_trigger_key: self.ignore_trigger_key,
        }
    }
}

impl<'de> Deserialize<'de> for RapidfireCard {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        RapidfireCardInput::deserialize(deserializer).map(|input| {
            input.into_card_with_defaults(
                default_min_press_spacing_ms(),
                default_trigger_jitter_max_ms(),
                true,
            )
        })
    }
}

/// 连发器设置
#[derive(Debug, Clone, Serialize, PartialEq)]
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
    /// 旧配置兼容：旧全局目标键最小触发间距；新 UI 按卡片保存
    #[serde(default = "default_min_press_spacing_ms")]
    pub min_press_spacing_ms: u64,
    /// 旧配置兼容：旧全局启动抖动延迟；新 UI 按卡片保存
    #[serde(default = "default_trigger_jitter_max_ms")]
    pub trigger_jitter_max_ms: u64,
    /// 旧配置兼容：旧全局抖动松手策略；新 UI 按卡片保存
    #[serde(default = "default_true")]
    pub cancel_jitter_on_release: bool,
    #[serde(default)]
    pub groups: Vec<RapidfireGroup>,
    #[serde(default)]
    pub cards: Vec<RapidfireCard>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RapidfireSettingsInput {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    rapidfire_enabled: bool,
    #[serde(default)]
    show_overlay: bool,
    #[serde(default)]
    overlay_position: Option<RapidfireRect>,
    #[serde(default = "default_overlay_width")]
    overlay_width: i32,
    #[serde(default = "default_compensation_delay_min_ms")]
    compensation_delay_min_ms: u64,
    #[serde(default = "default_compensation_delay_max_ms")]
    compensation_delay_max_ms: u64,
    #[serde(default = "default_min_press_spacing_ms")]
    min_press_spacing_ms: u64,
    #[serde(default = "default_trigger_jitter_max_ms")]
    trigger_jitter_max_ms: u64,
    #[serde(default = "default_true")]
    cancel_jitter_on_release: bool,
    #[serde(default)]
    groups: Vec<RapidfireGroup>,
    #[serde(default)]
    cards: Vec<RapidfireCardInput>,
}

impl<'de> Deserialize<'de> for RapidfireSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = RapidfireSettingsInput::deserialize(deserializer)?;
        Ok(Self {
            version: input.version,
            rapidfire_enabled: input.rapidfire_enabled,
            show_overlay: input.show_overlay,
            overlay_position: input.overlay_position,
            overlay_width: input.overlay_width,
            compensation_delay_min_ms: input.compensation_delay_min_ms,
            compensation_delay_max_ms: input.compensation_delay_max_ms,
            min_press_spacing_ms: input.min_press_spacing_ms,
            trigger_jitter_max_ms: input.trigger_jitter_max_ms,
            cancel_jitter_on_release: input.cancel_jitter_on_release,
            groups: input.groups,
            cards: input
                .cards
                .into_iter()
                .map(|card| {
                    card.into_card_with_defaults(
                        input.min_press_spacing_ms,
                        input.trigger_jitter_max_ms,
                        input.cancel_jitter_on_release,
                    )
                })
                .collect(),
        })
    }
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
            trigger_jitter_max_ms: default_trigger_jitter_max_ms(),
            cancel_jitter_on_release: true,
            groups: vec![RapidfireGroup {
                id: DEFAULT_RAPIDFIRE_GROUP_ID.to_string(),
                name: "默认分组".to_string(),
                enabled: true,
                show_overlay: false,
                overlay_position: None,
                overlay_width: 400,
            }],
            cards: vec![RapidfireCard {
                id: format!("rapidfire-{}", crate::utils::now_ms()),
                group_id: DEFAULT_RAPIDFIRE_GROUP_ID.to_string(),
                name: "连发器 1".to_string(),
                trigger_key: "F6".to_string(),
                target_key: "Space".to_string(),
                interval_ms: 100,
                press_jitter_min_ms: default_press_jitter_min_ms(),
                press_jitter_max_ms: default_press_jitter_max_ms(),
                min_press_spacing_ms: default_min_press_spacing_ms(),
                trigger_jitter_max_ms: default_trigger_jitter_max_ms(),
                cancel_jitter_on_release: true,
                enabled: false,
                skip_compensation: false,
                ignore_trigger_key: false,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
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
        assert_eq!(settings.groups.len(), 1);
        assert_eq!(settings.groups[0].id, DEFAULT_RAPIDFIRE_GROUP_ID);
        assert_eq!(settings.cards.len(), 1);
        assert_eq!(settings.cards[0].group_id, DEFAULT_RAPIDFIRE_GROUP_ID);
        assert_eq!(settings.cards[0].trigger_key, "F6");
        assert_eq!(settings.cards[0].target_key, "Space");
        assert_eq!(settings.cards[0].interval_ms, 100);
        assert_eq!(settings.cards[0].press_jitter_min_ms, 8);
        assert_eq!(settings.cards[0].press_jitter_max_ms, 12);
        assert_eq!(settings.cards[0].min_press_spacing_ms, 80);
        assert_eq!(settings.cards[0].trigger_jitter_max_ms, 0);
        assert!(settings.cards[0].cancel_jitter_on_release);
        assert!(!settings.cards[0].enabled);
        assert!(!settings.cards[0].skip_compensation);
        assert!(!settings.cards[0].ignore_trigger_key);
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
        assert_eq!(card.group_id, DEFAULT_RAPIDFIRE_GROUP_ID);
        assert_eq!(card.min_press_spacing_ms, 80);
        assert_eq!(card.trigger_jitter_max_ms, 0);
        assert!(card.cancel_jitter_on_release);
        assert!(!card.skip_compensation);
        assert!(!card.ignore_trigger_key);
    }

    #[test]
    fn rapidfire_settings_deserializes_legacy_global_values_into_cards() {
        let settings: RapidfireSettings = serde_json::from_value(serde_json::json!({
            "version": 1,
            "rapidfireEnabled": true,
            "showOverlay": true,
            "overlayPosition": null,
            "overlayWidth": 420,
            "minPressSpacingMs": 120,
            "triggerJitterMaxMs": 30,
            "cancelJitterOnRelease": false,
            "cards": [{
                "id": "legacy",
                "name": "旧配置",
                "triggerKey": "F6",
                "targetKey": "Space",
                "intervalMs": 100,
                "enabled": true
            }]
        }))
            .expect("旧连发器设置应把全局值迁移到卡片");

        assert_eq!(settings.compensation_delay_min_ms, 100);
        assert_eq!(settings.compensation_delay_max_ms, 150);
        assert_eq!(settings.min_press_spacing_ms, 120);
        assert!(settings.groups.is_empty());
        assert_eq!(settings.cards[0].min_press_spacing_ms, 120);
        assert_eq!(settings.cards[0].group_id, DEFAULT_RAPIDFIRE_GROUP_ID);
        assert_eq!(settings.cards[0].trigger_jitter_max_ms, 30);
        assert!(!settings.cards[0].cancel_jitter_on_release);
    }

    #[test]
    fn rapidfire_settings_preserves_card_level_values_over_legacy_globals() {
        let settings: RapidfireSettings = serde_json::from_value(serde_json::json!({
            "version": 1,
            "minPressSpacingMs": 120,
            "triggerJitterMaxMs": 30,
            "cancelJitterOnRelease": false,
            "cards": [{
                "id": "card",
                "name": "新配置",
                "triggerKey": "F6",
                "targetKey": "Space",
                "intervalMs": 100,
                "minPressSpacingMs": 0,
                "triggerJitterMaxMs": 5,
                "cancelJitterOnRelease": true,
                "enabled": true
            }]
        }))
            .expect("卡片级值优先于旧全局值");

        assert_eq!(settings.cards[0].min_press_spacing_ms, 0);
        assert_eq!(settings.cards[0].trigger_jitter_max_ms, 5);
        assert!(settings.cards[0].cancel_jitter_on_release);
    }
}
