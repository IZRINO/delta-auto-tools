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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TimerDirection {
    Countdown,
    Countup,
}

fn default_timer_direction() -> TimerDirection {
    TimerDirection::Countdown
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimerItem {
    pub id: String,
    pub name: String,
    pub duration_seconds: u64,
    pub hotkey: String,
    #[serde(default = "default_timer_direction")]
    pub direction: TimerDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CounterItem {
    pub id: String,
    pub name: String,
    pub start_value: i64,
    pub hotkey: String,
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
            timers: vec![TimerItem {
                id: "timer-1".to_string(),
                name: "计时器 1".to_string(),
                duration_seconds: 30,
                hotkey: "F2".to_string(),
                direction: TimerDirection::Countdown,
            }],
            counters: vec![CounterItem {
                id: "counter-1".to_string(),
                name: "计数器 1".to_string(),
                start_value: 0,
                hotkey: "F3".to_string(),
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
        assert_eq!(settings.timers.len(), 1);
        assert_eq!(settings.counters.len(), 1);
        assert_eq!(settings.timers[0].duration_seconds, 30);
        assert_eq!(settings.timers[0].hotkey, "F2");
        assert_eq!(settings.timers[0].direction, TimerDirection::Countdown);
        assert_eq!(settings.counters[0].start_value, 0);
        assert_eq!(settings.counters[0].hotkey, "F3");
    }
}
