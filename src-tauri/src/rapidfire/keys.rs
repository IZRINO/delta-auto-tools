use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use enigo::{Direction, Key, Keyboard};

use crate::hotkey_types;

// ---- 常量 ----

pub const RAPIDFIRE_TRIGGER_RELEASE_SETTLE_MS: u64 = 2;
/// 首次开火前等待物理触发键事件到达前台应用的稳定延迟。
/// 解决 enigo SendInput 目标键先于物理触发键到达前台应用的竞态。
pub const RAPIDFIRE_INITIAL_SETTLE_MS: u64 = 8;
pub const RAPIDFIRE_PRESS_JITTER_MIN_MS: u64 = 1;
pub const RAPIDFIRE_PRESS_JITTER_MAX_MS: u64 = 2000;
pub const RAPIDFIRE_GLOBAL_DELAY_MAX_MS: u64 = 10_000;
pub const RAPIDFIRE_TRIGGER_JITTER_MAX_MS: u64 = 99_999;
pub const RAPIDFIRE_MIN_INTERVAL_MS: u64 = 1;
pub const RAPIDFIRE_DISPLAY_MIN_HEIGHT: i32 = 80;
pub const RAPIDFIRE_DISPLAY_MIN_WIDTH: i32 = 320;
pub const RAPIDFIRE_DISPLAY_MAX_WIDTH: i32 = 800;

static RAPIDFIRE_JITTER_COUNTER: AtomicU64 = AtomicU64::new(1);

// ---- KeyEmitter trait ----

/// 抽象按键输出接口，让 worker 线程可测。
/// 生产实现使用 enigo 真实合成键盘事件；测试实现记录调用。
pub trait KeyEmitter: Send {
    /// 按下并释放目标键。
    fn press_release_target_key(
        &mut self,
        target_key: &str,
        held_trigger_key: Option<&str>,
        press_jitter_min_ms: u64,
        press_jitter_max_ms: u64,
    ) -> Result<(), String>;
}

// ---- EnigoKeyEmitter ----

/// 使用 enigo 库合成真实键盘事件的 KeyEmitter 实现。
pub struct EnigoKeyEmitter {
    enigo: enigo::Enigo,
}

impl EnigoKeyEmitter {
    pub fn new() -> Result<Self, String> {
        let enigo = enigo::Enigo::new(&enigo::Settings::default())
            .map_err(|error| format!("初始化连发输入失败: {error}"))?;
        Ok(Self { enigo })
    }
}

impl KeyEmitter for EnigoKeyEmitter {
    fn press_release_target_key(
        &mut self,
        target_key: &str,
        held_trigger_key: Option<&str>,
        press_jitter_min_ms: u64,
        press_jitter_max_ms: u64,
    ) -> Result<(), String> {
        let plan = target_fire_plan(target_key, held_trigger_key)?;
        let key_str = target_key.to_string();

        if let Some(trigger_key) = plan.trigger_key_to_release {
            self.enigo
                .key(trigger_key, Direction::Release)
                .map_err(|error| format!("释放连发触发键失败: {error}"))?;
            std::thread::sleep(Duration::from_millis(RAPIDFIRE_TRIGGER_RELEASE_SETTLE_MS));
        }

        self.enigo
            .key(plan.target_key, Direction::Press)
            .map_err(|error| format!("按下连发目标键 {key_str} 失败: {error}"))?;
        std::thread::sleep(Duration::from_millis(press_jitter_duration_ms(
            press_jitter_min_ms,
            press_jitter_max_ms,
        )));
        self.enigo
            .key(plan.target_key, Direction::Release)
            .map_err(|error| format!("抬起连发目标键 {key_str} 失败: {error}"))?;

        Ok(())
    }
}

// ---- MockKeyEmitter ----

/// 记录调用历史的 mock KeyEmitter，用于测试。
#[cfg(test)]
pub struct MockKeyEmitter {
    pub calls: Vec<MockKeyEmitCall>,
}

#[cfg(test)]
impl MockKeyEmitter {
    pub fn new() -> Self {
        Self { calls: Vec::new() }
    }
}

#[cfg(test)]
impl KeyEmitter for MockKeyEmitter {
    fn press_release_target_key(
        &mut self,
        target_key: &str,
        held_trigger_key: Option<&str>,
        press_jitter_min_ms: u64,
        press_jitter_max_ms: u64,
    ) -> Result<(), String> {
        self.calls.push(MockKeyEmitCall {
            target_key: target_key.to_string(),
            held_trigger_key: held_trigger_key.map(|s| s.to_string()),
            press_jitter_min_ms,
            press_jitter_max_ms,
        });
        Ok(())
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockKeyEmitCall {
    pub target_key: String,
    pub held_trigger_key: Option<String>,
    pub press_jitter_min_ms: u64,
    pub press_jitter_max_ms: u64,
}

// ---- 发射计划 ----

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFirePlan {
    pub target_key: Key,
    pub trigger_key_to_release: Option<Key>,
}

pub fn target_fire_plan(
    target_key: &str,
    held_trigger_key: Option<&str>,
) -> Result<TargetFirePlan, String> {
    let target_key =
        parse_target_key(target_key).ok_or_else(|| format!("不支持的目标键: {target_key}"))?;
    let held_trigger_key = held_trigger_key
        .map(trigger_primary_label)
        .transpose()?
        .map(|key| parse_target_key(&key).ok_or_else(|| format!("不支持的触发键: {key}")))
        .transpose()?;
    let trigger_key_to_release = held_trigger_key.filter(|trigger_key| trigger_key == &target_key);

    Ok(TargetFirePlan {
        target_key,
        trigger_key_to_release,
    })
}

pub fn press_jitter_duration_ms(min_ms: u64, max_ms: u64) -> u64 {
    let span = max_ms - min_ms + 1;
    let counter = RAPIDFIRE_JITTER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::from(duration.subsec_nanos()))
        .unwrap_or(0);

    min_ms + ((nanos ^ counter.rotate_left(13)) % span)
}

pub fn parse_target_key(key: &str) -> Option<Key> {
    let upper = key.trim().to_uppercase();
    match upper.as_str() {
        "A" => Some(Key::A),
        "B" => Some(Key::B),
        "C" => Some(Key::C),
        "D" => Some(Key::D),
        "E" => Some(Key::E),
        "F" => Some(Key::F),
        "G" => Some(Key::G),
        "H" => Some(Key::H),
        "I" => Some(Key::I),
        "J" => Some(Key::J),
        "K" => Some(Key::K),
        "L" => Some(Key::L),
        "M" => Some(Key::M),
        "N" => Some(Key::N),
        "O" => Some(Key::O),
        "P" => Some(Key::P),
        "Q" => Some(Key::Q),
        "R" => Some(Key::R),
        "S" => Some(Key::S),
        "T" => Some(Key::T),
        "U" => Some(Key::U),
        "V" => Some(Key::V),
        "W" => Some(Key::W),
        "X" => Some(Key::X),
        "Y" => Some(Key::Y),
        "Z" => Some(Key::Z),
        "0" => Some(Key::Num0),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
        "SPACE" => Some(Key::Space),
        "ENTER" | "RETURN" => Some(Key::Return),
        "TAB" => Some(Key::Tab),
        "ESC" | "ESCAPE" => Some(Key::Escape),
        "BACKSPACE" => Some(Key::Backspace),
        "UP" => Some(Key::UpArrow),
        "DOWN" => Some(Key::DownArrow),
        "LEFT" => Some(Key::LeftArrow),
        "RIGHT" => Some(Key::RightArrow),
        "HOME" => Some(Key::Home),
        "END" => Some(Key::End),
        "PAGEUP" => Some(Key::PageUp),
        "PAGEDOWN" => Some(Key::PageDown),
        "INSERT" => Some(Key::Insert),
        "DELETE" => Some(Key::Delete),
        "ALT" => Some(Key::Alt),
        ";" | "SEMICOLON" => Some(Key::OEM1),
        "," | "COMMA" => Some(Key::OEMComma),
        "." | "PERIOD" => Some(Key::OEMPeriod),
        "/" | "SLASH" => Some(Key::OEM2),
        "\\" | "BACKSLASH" => Some(Key::OEM5),
        "[" | "BRACKETLEFT" => Some(Key::OEM4),
        "]" | "BRACKETRIGHT" => Some(Key::OEM6),
        "-" | "MINUS" => Some(Key::OEMMinus),
        "=" | "EQUAL" => Some(Key::OEMPlus),
        "+" | "PLUS" => Some(Key::Add),
        "`" | "BACKQUOTE" => Some(Key::OEM3),
        "'" | "QUOTE" => Some(Key::OEM7),
        _ => None,
    }
}

pub fn trigger_primary_label(trigger_key: &str) -> Result<String, String> {
    hotkey_types::hotkey_primary_label(trigger_key)
        .map_err(|_| format!("不支持的触发键: {trigger_key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_target_key_supports_all_valid_keys() {
        assert_eq!(parse_target_key("A"), Some(Key::A));
        assert_eq!(parse_target_key("Z"), Some(Key::Z));
        assert_eq!(parse_target_key("0"), Some(Key::Num0));
        assert_eq!(parse_target_key("9"), Some(Key::Num9));
        assert!(parse_target_key("F1").is_some());
        assert!(parse_target_key("F12").is_some());
        assert!(parse_target_key("Space").is_some());
        assert!(parse_target_key("Enter").is_some());
        assert!(parse_target_key("Tab").is_some());
        assert!(parse_target_key("Esc").is_some());
        assert!(parse_target_key("Backspace").is_some());
        assert!(parse_target_key("Up").is_some());
        assert!(parse_target_key("Down").is_some());
        assert!(parse_target_key("Left").is_some());
        assert!(parse_target_key("Right").is_some());
        assert!(parse_target_key("Home").is_some());
        assert!(parse_target_key("End").is_some());
        assert!(parse_target_key("PageUp").is_some());
        assert!(parse_target_key("PageDown").is_some());
        assert!(parse_target_key("Insert").is_some());
        assert!(parse_target_key("Delete").is_some());
        assert!(parse_target_key("Alt").is_some());
        assert!(parse_target_key("Unknown").is_none());
    }

    #[test]
    fn target_fire_plan_uses_press_and_release_actions() {
        let plan = target_fire_plan("T", Some("T")).unwrap();
        assert_eq!(plan.target_key, parse_target_key("T").unwrap());
        assert_eq!(plan.trigger_key_to_release, parse_target_key("T"));
    }

    #[test]
    fn target_fire_plan_releases_same_primary_trigger_for_modified_hotkey() {
        let plan = target_fire_plan("-", Some("Shift+-")).unwrap();
        assert_eq!(plan.target_key, parse_target_key("-").unwrap());
        assert_eq!(plan.trigger_key_to_release, parse_target_key("-"));
    }

    #[test]
    fn target_fire_plan_keeps_different_trigger_key_held() {
        let plan = target_fire_plan("Space", Some("W")).unwrap();
        assert_eq!(plan.target_key, parse_target_key("Space").unwrap());
        assert_eq!(plan.trigger_key_to_release, None);
    }

    #[test]
    fn target_fire_plan_allows_compensation_without_held_trigger() {
        let plan = target_fire_plan("T", None).unwrap();
        assert_eq!(plan.target_key, parse_target_key("T").unwrap());
        assert_eq!(plan.trigger_key_to_release, None);
    }

    #[test]
    fn press_jitter_stays_within_custom_range() {
        for _ in 0..100 {
            let jitter = press_jitter_duration_ms(15, 25);
            assert!(
                (15..=25).contains(&jitter),
                "按下抖动应落在 15-25ms，实际为 {jitter}ms"
            );
        }
    }

    #[test]
    fn mock_key_emitter_records_calls() {
        let mut emitter = MockKeyEmitter::new();
        emitter
            .press_release_target_key("A", Some("W"), 8, 12)
            .unwrap();
        emitter.press_release_target_key("B", None, 10, 15).unwrap();

        assert_eq!(emitter.calls.len(), 2);
        assert_eq!(emitter.calls[0].target_key, "A");
        assert_eq!(emitter.calls[0].held_trigger_key, Some("W".to_string()));
        assert_eq!(emitter.calls[1].target_key, "B");
        assert_eq!(emitter.calls[1].held_trigger_key, None);
    }
}
