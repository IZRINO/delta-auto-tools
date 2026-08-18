use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

pub(crate) const LIMITED_SUPPLY_TIMES: [u16; 2] = [12 * 60, 20 * 60];

fn default_research_delay_ms() -> u32 {
    3_000
}

fn default_ready_timeout_ms() -> u32 {
    10_000
}

fn default_colors() -> [[u8; 3]; 2] {
    [[0, 0, 0], [255, 255, 255]]
}

fn default_color_tolerances() -> [u8; 2] {
    [30, 30]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LimitedSupplySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_research_delay_ms")]
    pub research_delay_ms: u32,
    #[serde(default = "default_ready_timeout_ms")]
    pub ready_timeout_ms: u32,
    #[serde(default = "default_colors")]
    pub colors: [[u8; 3]; 2],
    #[serde(default = "default_color_tolerances")]
    pub color_tolerances: [u8; 2],
}

impl Default for LimitedSupplySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            research_delay_ms: default_research_delay_ms(),
            ready_timeout_ms: default_ready_timeout_ms(),
            colors: default_colors(),
            color_tolerances: default_color_tolerances(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LimitedSupplyCycle {
    pub(crate) id: String,
    start_day: NaiveDate,
    start_minute: u16,
}

impl LimitedSupplyCycle {
    pub(crate) fn for_day_and_minute(day: &str, minute: u16) -> Option<Self> {
        if minute >= 24 * 60 {
            return None;
        }
        let day = NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()?;
        let (start_day, start_minute) = if minute < LIMITED_SUPPLY_TIMES[0] {
            (
                day.checked_sub_signed(Duration::days(1))?,
                LIMITED_SUPPLY_TIMES[1],
            )
        } else if minute < LIMITED_SUPPLY_TIMES[1] {
            (day, LIMITED_SUPPLY_TIMES[0])
        } else {
            (day, LIMITED_SUPPLY_TIMES[1])
        };
        let hour = start_minute / 60;
        let minute = start_minute % 60;
        Some(Self {
            id: format!("{}T{hour:02}:{minute:02}", start_day.format("%Y-%m-%d")),
            start_day,
            start_minute,
        })
    }

    #[cfg(test)]
    pub(crate) fn contains_day_and_minute(&self, day: &str, minute: u16) -> bool {
        let Some(day) = NaiveDate::parse_from_str(day, "%Y-%m-%d").ok() else {
            return false;
        };
        if minute >= 24 * 60 {
            return false;
        }
        let start = (self.start_day, self.start_minute);
        let end = if self.start_minute == LIMITED_SUPPLY_TIMES[0] {
            (self.start_day, LIMITED_SUPPLY_TIMES[1])
        } else {
            let Some(next_day) = self.start_day.checked_add_signed(Duration::days(1)) else {
                return false;
            };
            (next_day, LIMITED_SUPPLY_TIMES[0])
        };
        (day, minute) >= start && (day, minute) < end
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum LimitedSupplyOutcome {
    #[default]
    Pending,
    NoHighValue,
    HighValue,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LimitedSupplyAccountState {
    pub cycle_id: Option<String>,
    pub outcome: LimitedSupplyOutcome,
    pub checked_at_ms: Option<i64>,
    pub matched_region: Option<u8>,
    pub matched_color: Option<[u8; 3]>,
    /// 命中的配置颜色编号，1 / 2，可同时命中。
    #[serde(default)]
    pub matched_color_indexes: Vec<u8>,
    pub acknowledged: bool,
    pub last_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::LimitedSupplyCycle;

    #[test]
    fn noon_cycle_expires_when_evening_cycle_starts() {
        let cycle =
            LimitedSupplyCycle::for_day_and_minute("2026-08-08", 12 * 60).expect("中午周期应有效");

        assert_eq!(cycle.id, "2026-08-08T12:00");
        assert!(cycle.contains_day_and_minute("2026-08-08", 19 * 60 + 59));
        assert!(!cycle.contains_day_and_minute("2026-08-08", 20 * 60));
    }

    #[test]
    fn evening_cycle_survives_midnight_until_noon() {
        let cycle = LimitedSupplyCycle::for_day_and_minute("2026-08-09", 60)
            .expect("凌晨应归入前一日晚间周期");

        assert_eq!(cycle.id, "2026-08-08T20:00");
        assert!(cycle.contains_day_and_minute("2026-08-09", 11 * 60 + 59));
        assert!(!cycle.contains_day_and_minute("2026-08-09", 12 * 60));
    }

    #[test]
    fn invalid_day_or_minute_is_rejected() {
        assert!(LimitedSupplyCycle::for_day_and_minute("2026-02-30", 12 * 60).is_none());
        assert!(LimitedSupplyCycle::for_day_and_minute("2026-08-08", 24 * 60).is_none());
    }

    #[test]
    fn legacy_color_sample_regions_are_ignored_and_not_reserialized() {
        let mut value = serde_json::to_value(super::LimitedSupplySettings::default()).unwrap();
        value["colorSampleRegions"] = serde_json::json!([1, 9]);

        let settings: super::LimitedSupplySettings = serde_json::from_value(value).unwrap();
        let serialized = serde_json::to_value(settings).unwrap();

        assert!(serialized.get("colorSampleRegions").is_none());
    }
}
