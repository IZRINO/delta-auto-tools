use std::{sync::OnceLock, thread, time::Duration};

use crate::hotkey_types::{HotkeyBinding, ModifierKey, NamedKey, PrimaryKey};
use enigo::{Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

static INPUT_SIMULATION_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
const INPUT_POST_ACTION_GAP: Duration = Duration::from_millis(35);

async fn run_serialized_input<F, T>(operation: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let lock = INPUT_SIMULATION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;
    let result = tokio::task::spawn_blocking(operation)
        .await
        .map_err(|err| format!("输入模拟任务失败: {err}"))?;
    tokio::time::sleep(INPUT_POST_ACTION_GAP).await;
    result
}

#[cfg(test)]
async fn run_serialized_input_for_test<F, Fut>(operation: F) -> Result<(), String>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    let lock = INPUT_SIMULATION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;
    operation().await
}

pub async fn type_text(value: &str, delay_ms: u64) -> Result<(), String> {
    let value = value.to_string();
    let char_count = value.chars().count();

    crate::log_debug!(
        "input_simulation",
        "输入模拟开始",
        "kind" => "text",
        "primary" => "text",
        "char_count" => char_count,
        "card_id" => Option::<String>::None
    );
    let result = run_serialized_input(move || -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化自动输入失败: {error}"))?;

        for ch in value.chars() {
            enigo
                .key(Key::Unicode(ch), Direction::Click)
                .map_err(|error| format!("自动输入字符 {ch} 失败: {error}"))?;

            if delay_ms > 0 {
                thread::sleep(Duration::from_millis(delay_ms));
            }
        }

        Ok(())
    })
    .await;
    crate::log_debug!(
        "input_simulation",
        "输入模拟结束",
        "kind" => "text",
        "primary" => "text",
        "char_count" => char_count,
        "success" => result.is_ok(),
        "error" => result.as_ref().err(),
        "card_id" => Option::<String>::None
    );
    result
}

pub async fn press_hotkey_once(hotkey: &str, label: &str) -> Result<(), String> {
    press_hotkey_once_with_card(hotkey, label, None).await
}

pub async fn press_hotkey_once_for_card(
    hotkey: &str,
    label: &str,
    card_id: &str,
) -> Result<(), String> {
    press_hotkey_once_with_card(hotkey, label, Some(card_id)).await
}

async fn press_hotkey_once_with_card(
    hotkey: &str,
    label: &str,
    card_id: Option<&str>,
) -> Result<(), String> {
    let hotkey = hotkey.trim().to_string();
    let label = label.to_string();
    let card_id = card_id.map(str::to_string);
    if hotkey.is_empty() {
        return Ok(());
    }
    let task_label = label.clone();
    let binding =
        HotkeyBinding::parse(&hotkey).map_err(|error| format!("{task_label}配置无效: {error}"))?;
    let primary_label = crate::hotkey_types::primary_to_string(binding.primary);

    crate::log_debug!(
        "input_simulation",
        "输入模拟开始",
        "kind" => "hotkey",
        "primary" => primary_label.clone(),
        "hotkey" => hotkey.clone(),
        "label" => label.clone(),
        "card_id" => card_id.as_deref()
    );
    let hotkey_for_task = hotkey.clone();
    let result = run_serialized_input(move || -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化{task_label}失败: {error}"))?;

        let modifiers = ordered_modifiers(&binding);
        for modifier in &modifiers {
            enigo
                .key(modifier_to_key(*modifier), Direction::Press)
                .map_err(|error| format!("按下修饰键失败: {error}"))?;
        }

        let primary = primary_to_key(binding.primary)?;
        let click_result = enigo
            .key(primary, Direction::Click)
            .map_err(|error| format!("执行{task_label} {hotkey_for_task} 失败: {error}"));

        for modifier in modifiers.iter().rev() {
            let _ = enigo.key(modifier_to_key(*modifier), Direction::Release);
        }

        click_result
    })
    .await;
    crate::log_debug!(
        "input_simulation",
        "输入模拟结束",
        "kind" => "hotkey",
        "primary" => primary_label,
        "hotkey" => hotkey,
        "label" => label,
        "success" => result.is_ok(),
        "error" => result.as_ref().err(),
        "card_id" => card_id.as_deref()
    );
    result
}

pub async fn click_points(points: &[(i32, i32, u64)]) -> Result<(), String> {
    click_points_with_card(points, None).await
}

pub async fn click_points_for_card(
    points: &[(i32, i32, u64)],
    card_id: &str,
) -> Result<(), String> {
    click_points_with_card(points, Some(card_id)).await
}

async fn click_points_with_card(
    points: &[(i32, i32, u64)],
    card_id: Option<&str>,
) -> Result<(), String> {
    if points.is_empty() {
        return Ok(());
    }

    let points = points.to_vec();
    let point_count = points.len();
    let card_id = card_id.map(str::to_string);
    crate::log_debug!(
        "input_simulation",
        "输入模拟开始",
        "kind" => "click",
        "primary" => "left",
        "button" => "left",
        "point_count" => point_count,
        "card_id" => card_id.as_deref()
    );
    let result = run_serialized_input(move || -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化鼠标点击失败: {error}"))?;

        for (center_x, center_y, delay_ms) in &points {
            if *delay_ms > 0 {
                thread::sleep(Duration::from_millis(*delay_ms));
            }
            enigo
                .move_mouse(*center_x, *center_y, Coordinate::Abs)
                .map_err(|error| format!("移动鼠标到 ({center_x}, {center_y}) 失败: {error}"))?;
            enigo
                .button(enigo::Button::Left, Direction::Click)
                .map_err(|error| format!("鼠标左键点击失败: {error}"))?;
        }

        Ok(())
    })
    .await;
    crate::log_debug!(
        "input_simulation",
        "输入模拟结束",
        "kind" => "click",
        "primary" => "left",
        "button" => "left",
        "point_count" => point_count,
        "success" => result.is_ok(),
        "error" => result.as_ref().err(),
        "card_id" => card_id.as_deref()
    );
    result
}

fn ordered_modifiers(binding: &HotkeyBinding) -> Vec<ModifierKey> {
    [
        ModifierKey::Ctrl,
        ModifierKey::Alt,
        ModifierKey::Shift,
        ModifierKey::Super,
    ]
    .into_iter()
    .filter(|modifier| binding.modifiers.contains(modifier))
    .collect()
}

fn modifier_to_key(modifier: ModifierKey) -> Key {
    match modifier {
        ModifierKey::Ctrl => Key::Control,
        ModifierKey::Alt => Key::Alt,
        ModifierKey::Shift => Key::Shift,
        ModifierKey::Super => Key::Meta,
    }
}

fn primary_to_key(primary: PrimaryKey) -> Result<Key, String> {
    Ok(match primary {
        PrimaryKey::Letter(value) => letter_to_key(value)?,
        PrimaryKey::Digit(value) => digit_to_key(value)?,
        PrimaryKey::Function(value) => function_to_key(value)?,
        PrimaryKey::Named(named) => named_to_key(named),
    })
}

fn letter_to_key(value: char) -> Result<Key, String> {
    Ok(match value {
        'A' => Key::A,
        'B' => Key::B,
        'C' => Key::C,
        'D' => Key::D,
        'E' => Key::E,
        'F' => Key::F,
        'G' => Key::G,
        'H' => Key::H,
        'I' => Key::I,
        'J' => Key::J,
        'K' => Key::K,
        'L' => Key::L,
        'M' => Key::M,
        'N' => Key::N,
        'O' => Key::O,
        'P' => Key::P,
        'Q' => Key::Q,
        'R' => Key::R,
        'S' => Key::S,
        'T' => Key::T,
        'U' => Key::U,
        'V' => Key::V,
        'W' => Key::W,
        'X' => Key::X,
        'Y' => Key::Y,
        'Z' => Key::Z,
        _ => return Err(format!("暂不支持的字母按键: {value}")),
    })
}

fn digit_to_key(value: char) -> Result<Key, String> {
    Ok(match value {
        '0' => Key::Num0,
        '1' => Key::Num1,
        '2' => Key::Num2,
        '3' => Key::Num3,
        '4' => Key::Num4,
        '5' => Key::Num5,
        '6' => Key::Num6,
        '7' => Key::Num7,
        '8' => Key::Num8,
        '9' => Key::Num9,
        _ => return Err(format!("暂不支持的数字按键: {value}")),
    })
}

fn function_to_key(value: u8) -> Result<Key, String> {
    Ok(match value {
        1 => Key::F1,
        2 => Key::F2,
        3 => Key::F3,
        4 => Key::F4,
        5 => Key::F5,
        6 => Key::F6,
        7 => Key::F7,
        8 => Key::F8,
        9 => Key::F9,
        10 => Key::F10,
        11 => Key::F11,
        12 => Key::F12,
        _ => return Err(format!("暂不支持的功能键: F{value}")),
    })
}

fn named_to_key(named: NamedKey) -> Key {
    match named {
        NamedKey::Space => Key::Space,
        NamedKey::Enter => Key::Return,
        NamedKey::Tab => Key::Tab,
        NamedKey::Esc => Key::Escape,
        NamedKey::Up => Key::UpArrow,
        NamedKey::Down => Key::DownArrow,
        NamedKey::Left => Key::LeftArrow,
        NamedKey::Right => Key::RightArrow,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::Insert => Key::Insert,
        NamedKey::Delete => Key::Delete,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Alt => Key::Alt,
        NamedKey::Semicolon => Key::OEM1,
        NamedKey::Comma => Key::OEMComma,
        NamedKey::Period => Key::OEMPeriod,
        NamedKey::Slash => Key::OEM2,
        NamedKey::Backslash => Key::OEM5,
        NamedKey::BracketLeft => Key::OEM4,
        NamedKey::BracketRight => Key::OEM6,
        NamedKey::Minus => Key::OEMMinus,
        NamedKey::Equal => Key::OEMPlus,
        NamedKey::Plus => Key::Add,
        NamedKey::Backquote => Key::OEM3,
        NamedKey::Quote => Key::OEM7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::{oneshot, Mutex};

    #[test]
    fn maps_comma_and_period_to_enigo_keys() {
        assert!(matches!(
            primary_to_key(PrimaryKey::Named(NamedKey::Comma)).unwrap(),
            Key::OEMComma
        ));
        assert!(matches!(
            primary_to_key(PrimaryKey::Named(NamedKey::Period)).unwrap(),
            Key::OEMPeriod
        ));
    }

    #[tokio::test]
    async fn serialized_input_jobs_do_not_overlap() {
        let events = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let (first_started_tx, first_started_rx) = oneshot::channel::<()>();
        let (release_first_tx, release_first_rx) = oneshot::channel::<()>();

        let first_events = Arc::clone(&events);
        let first = tokio::spawn(run_serialized_input_for_test(move || async move {
            first_events.lock().await.push("first-start");
            let _ = first_started_tx.send(());
            release_first_rx.await.map_err(|error| error.to_string())?;
            first_events.lock().await.push("first-end");
            Ok(())
        }));

        first_started_rx.await.unwrap();

        let second_events = Arc::clone(&events);
        let second = tokio::spawn(run_serialized_input_for_test(move || async move {
            second_events.lock().await.push("second-start");
            second_events.lock().await.push("second-end");
            Ok(())
        }));

        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(events.lock().await.as_slice(), ["first-start"]);

        release_first_tx.send(()).unwrap();
        first.await.unwrap().unwrap();
        second.await.unwrap().unwrap();

        assert_eq!(
            events.lock().await.as_slice(),
            ["first-start", "first-end", "second-start", "second-end"]
        );
    }
}
