use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CounterRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Default for CounterRect {
    fn default() -> Self {
        Self {
            x: 420,
            y: 80,
            width: 320,
            height: 96,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CounterDisplaySettings {
    pub rect: CounterRect,
    pub font_opacity: f64,
}

impl Default for CounterDisplaySettings {
    fn default() -> Self {
        Self {
            rect: CounterRect::default(),
            font_opacity: 0.92,
        }
    }
}

fn default_true() -> bool {
    true
}

pub const DEFAULT_COUNTER_GROUP_ID: &str = "default-counter-group";

fn default_counter_group_id() -> String {
    DEFAULT_COUNTER_GROUP_ID.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CounterGroup {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub display: CounterDisplaySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CounterItem {
    pub id: String,
    #[serde(default = "default_counter_group_id")]
    pub group_id: String,
    pub name: String,
    pub start_value: i64,
    pub hotkey: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CounterSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub counter_enabled: bool,
    pub display: CounterDisplaySettings,
    #[serde(default)]
    pub counter_groups: Vec<CounterGroup>,
    #[serde(default)]
    pub counters: Vec<CounterItem>,
}

impl Default for CounterSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            counter_enabled: false,
            display: CounterDisplaySettings::default(),
            counter_groups: vec![CounterGroup {
                id: DEFAULT_COUNTER_GROUP_ID.to_string(),
                name: "默认分组".to_string(),
                enabled: true,
                display: CounterDisplaySettings::default(),
            }],
            counters: vec![CounterItem {
                id: "counter-1".to_string(),
                group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
                name: "计数器 1".to_string(),
                start_value: 0,
                hotkey: "F3".to_string(),
                enabled: true,
            }],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CounterRunState {
    pub id: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CounterBootstrap {
    pub settings: CounterSettings,
    pub counter_runs: Vec<CounterRunState>,
    pub hotkey_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CounterSelectionOutcome {
    pub kind: CounterSelectionKind,
    pub rect: CounterRect,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CounterSelectionKind {
    Selected,
    Cancelled,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_settings_default_values_are_stable() {
        let settings = CounterSettings::default();

        assert!(!settings.enabled);
        assert!(!settings.counter_enabled);
        assert_eq!(settings.display.rect.width, 320);
        assert_eq!(settings.display.rect.x, 420);
        assert_eq!(settings.counter_groups.len(), 1);
        assert_eq!(settings.counter_groups[0].id, DEFAULT_COUNTER_GROUP_ID);
        assert_eq!(settings.counters.len(), 1);
        assert_eq!(settings.counters[0].start_value, 0);
        assert_eq!(settings.counters[0].hotkey, "F3");
        assert_eq!(settings.counters[0].group_id, DEFAULT_COUNTER_GROUP_ID);
        assert!(settings.counters[0].enabled);
    }

    #[test]
    fn counter_settings_deserializes_legacy_without_groups() {
        let settings: CounterSettings = serde_json::from_value(serde_json::json!({
            "enabled": true,
            "counterEnabled": true,
            "display": { "rect": { "x": 3, "y": 4, "width": 520, "height": 96 }, "fontOpacity": 0.7 },
            "counters": [{
                "id": "counter-a",
                "name": "旧计数器",
                "startValue": 0,
                "hotkey": "F3"
            }]
        }))
        .expect("旧计数器配置应反序列化");

        assert!(settings.counter_groups.is_empty());
        assert_eq!(settings.counters[0].group_id, DEFAULT_COUNTER_GROUP_ID);
    }
}
