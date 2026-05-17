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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimerItem {
    pub id: String,
    pub name: String,
    pub duration_seconds: u64,
    pub hotkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerSettings {
    pub enabled: bool,
    pub display: TimerDisplaySettings,
    pub timers: Vec<TimerItem>,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            display: TimerDisplaySettings::default(),
            timers: vec![TimerItem {
                id: "timer-1".to_string(),
                name: "计时器 1".to_string(),
                duration_seconds: 30,
                hotkey: "F2".to_string(),
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
    pub remaining_seconds: u64,
    pub status: TimerRunStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerBootstrap {
    pub settings: TimerSettings,
    pub runs: Vec<TimerRunState>,
    pub hotkey_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerSelectionOutcome {
    pub kind: TimerSelectionKind,
    pub rect: TimerRect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TimerSelectionKind {
    Selected,
    Cancelled,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_settings_default_values_are_stable() {
        let settings = TimerSettings::default();

        assert!(!settings.enabled);
        assert_eq!(settings.display.rect.width, 320);
        assert_eq!(settings.timers.len(), 1);
        assert_eq!(settings.timers[0].duration_seconds, 30);
        assert_eq!(settings.timers[0].hotkey, "F2");
    }
}
