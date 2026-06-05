use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimerRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Default for TimerRect {
    fn default() -> Self {
        Self {
            x: 80,
            y: 80,
            width: 320,
            height: 96,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerDisplaySettings {
    pub rect: TimerRect,
    pub font_opacity: f64,
}

impl Default for TimerDisplaySettings {
    fn default() -> Self {
        Self {
            rect: TimerRect::default(),
            font_opacity: 0.92,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_counter_display() -> TimerDisplaySettings {
    TimerDisplaySettings {
        rect: TimerRect {
            x: 420,
            y: 80,
            width: 320,
            height: 96,
        },
        font_opacity: 0.92,
    }
}

pub const DEFAULT_TIMER_GROUP_ID: &str = "default-timer-group";
pub const DEFAULT_COUNTER_GROUP_ID: &str = "default-counter-group";

fn default_timer_group_id() -> String {
    DEFAULT_TIMER_GROUP_ID.to_string()
}

fn default_counter_group_id() -> String {
    DEFAULT_COUNTER_GROUP_ID.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerGroup {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub display: TimerDisplaySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TimerDirection {
    Countdown,
    Countup,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TimerTriggerMode {
    /// 快捷键按下时触发计时（当前默认行为）
    Press,
    /// 快捷键释放时触发计时
    Release,
}

fn default_timer_direction() -> TimerDirection {
    TimerDirection::Countdown
}

fn default_trigger_mode() -> TimerTriggerMode {
    TimerTriggerMode::Press
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimerItem {
    pub id: String,
    #[serde(default = "default_timer_group_id")]
    pub group_id: String,
    pub name: String,
    pub duration_seconds: u64,
    pub hotkey: String,
    #[serde(default = "default_timer_direction")]
    pub direction: TimerDirection,
    #[serde(default = "default_trigger_mode")]
    pub trigger_mode: TimerTriggerMode,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub ignore_running: bool,
    #[serde(default)]
    pub segment_count: Option<u32>,
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
pub struct TimerSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub timer_enabled: bool,
    #[serde(default)]
    pub counter_enabled: bool,
    pub display: TimerDisplaySettings,
    #[serde(default = "default_counter_display")]
    pub counter_display: TimerDisplaySettings,
    #[serde(default)]
    pub timer_groups: Vec<TimerGroup>,
    #[serde(default)]
    pub counter_groups: Vec<TimerGroup>,
    pub timers: Vec<TimerItem>,
    #[serde(default)]
    pub counters: Vec<CounterItem>,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            timer_enabled: false,
            counter_enabled: false,
            display: TimerDisplaySettings::default(),
            counter_display: default_counter_display(),
            timer_groups: vec![TimerGroup {
                id: DEFAULT_TIMER_GROUP_ID.to_string(),
                name: "默认分组".to_string(),
                enabled: true,
                display: TimerDisplaySettings::default(),
            }],
            counter_groups: vec![TimerGroup {
                id: DEFAULT_COUNTER_GROUP_ID.to_string(),
                name: "默认分组".to_string(),
                enabled: true,
                display: default_counter_display(),
            }],
            timers: vec![TimerItem {
                id: "timer-1".to_string(),
                group_id: DEFAULT_TIMER_GROUP_ID.to_string(),
                name: "计时器 1".to_string(),
                duration_seconds: 30,
                hotkey: "F2".to_string(),
                direction: TimerDirection::Countdown,
                trigger_mode: TimerTriggerMode::Press,
                enabled: true,
                ignore_running: true,
                segment_count: None,
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
pub enum TimerRunStatus {
    Running,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimerRunState {
    pub id: String,
    pub current_seconds: u64,
    pub remaining_seconds: u64,
    pub duration_seconds: u64,
    pub direction: TimerDirection,
    pub status: TimerRunStatus,
    #[serde(default)]
    pub segment_count: Option<u32>,
    #[serde(default)]
    pub segment_duration: u64,
    #[serde(default)]
    pub recovering: bool,
    #[serde(default)]
    pub recovering_count: u32,
    #[serde(default)]
    pub active_segment_index: u32,
    #[serde(default)]
    pub started_at_ms: u64,
    #[serde(default)]
    pub recovery_start_pool: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CounterRunState {
    pub id: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerBootstrap {
    pub settings: TimerSettings,
    pub runs: Vec<TimerRunState>,
    pub counter_runs: Vec<CounterRunState>,
    pub hotkey_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerSelectionOutcome {
    pub kind: TimerSelectionKind,
    pub rect: TimerRect,
    pub target: TimerDisplayTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TimerSelectionKind {
    Selected,
    Cancelled,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TimerDisplayTarget {
    Timer,
    Counter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_settings_default_values_are_stable() {
        let settings = TimerSettings::default();

        assert!(!settings.enabled);
        assert!(!settings.timer_enabled);
        assert!(!settings.counter_enabled);
        assert_eq!(settings.display.rect.width, 320);
        assert_eq!(settings.counter_display.rect.x, 420);
        assert_eq!(settings.timer_groups.len(), 1);
        assert_eq!(settings.counter_groups.len(), 1);
        assert_eq!(settings.timer_groups[0].id, DEFAULT_TIMER_GROUP_ID);
        assert_eq!(settings.counter_groups[0].id, DEFAULT_COUNTER_GROUP_ID);
        assert_eq!(settings.timers.len(), 1);
        assert_eq!(settings.counters.len(), 1);
        assert_eq!(settings.timers[0].duration_seconds, 30);
        assert_eq!(settings.timers[0].hotkey, "F2");
        assert_eq!(settings.timers[0].group_id, DEFAULT_TIMER_GROUP_ID);
        assert_eq!(settings.timers[0].direction, TimerDirection::Countdown);
        assert!(settings.timers[0].enabled);
        assert!(settings.timers[0].ignore_running);
        assert_eq!(settings.timers[0].segment_count, None);
        assert_eq!(settings.counters[0].start_value, 0);
        assert_eq!(settings.counters[0].hotkey, "F3");
        assert_eq!(settings.counters[0].group_id, DEFAULT_COUNTER_GROUP_ID);
        assert!(settings.counters[0].enabled);
    }

    #[test]
    fn timer_settings_deserializes_legacy_without_groups() {
        let settings: TimerSettings = serde_json::from_value(serde_json::json!({
            "enabled": true,
            "timerEnabled": true,
            "counterEnabled": true,
            "display": { "rect": { "x": 1, "y": 2, "width": 480, "height": 96 }, "fontOpacity": 0.8 },
            "counterDisplay": { "rect": { "x": 3, "y": 4, "width": 520, "height": 96 }, "fontOpacity": 0.7 },
            "timers": [{
                "id": "timer-a",
                "name": "旧计时器",
                "durationSeconds": 30,
                "hotkey": "F2"
            }],
            "counters": [{
                "id": "counter-a",
                "name": "旧计数器",
                "startValue": 0,
                "hotkey": "F3"
            }]
        }))
        .expect("旧计时器配置应反序列化");

        assert!(settings.timer_groups.is_empty());
        assert!(settings.counter_groups.is_empty());
        assert_eq!(settings.timers[0].group_id, DEFAULT_TIMER_GROUP_ID);
        assert_eq!(settings.counters[0].group_id, DEFAULT_COUNTER_GROUP_ID);
    }
}
