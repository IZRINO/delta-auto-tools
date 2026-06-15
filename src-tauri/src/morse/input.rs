use std::{thread, time::Duration};

use crate::{
    hotkey_types::{HotkeyBinding, ModifierKey, NamedKey, PrimaryKey},
    morse::types::ClickRegion,
};
use enigo::{Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

pub async fn type_result(value: &str, delay_ms: u64) -> Result<(), String> {
    let value = value.to_string();

    tokio::task::spawn_blocking(move || -> Result<(), String> {
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
        .await
        .map_err(|error| format!("自动输入任务执行失败: {error}"))?
}

pub async fn press_hotkey_once(hotkey: &str) -> Result<(), String> {
    let hotkey = hotkey.trim().to_string();
    if hotkey.is_empty() {
        return Ok(());
    }

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let binding = HotkeyBinding::parse(&hotkey)
            .map_err(|error| format!("点击完成后按键配置无效: {error}"))?;
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化点击完成后按键失败: {error}"))?;

        let modifiers = ordered_modifiers(&binding);
        for modifier in &modifiers {
            enigo
                .key(modifier_to_key(*modifier), Direction::Press)
                .map_err(|error| format!("按下修饰键失败: {error}"))?;
        }

        let primary = primary_to_key(binding.primary)?;
        let click_result = enigo
            .key(primary, Direction::Click)
            .map_err(|error| format!("执行点击完成后按键 {hotkey} 失败: {error}"));

        for modifier in modifiers.iter().rev() {
            let _ = enigo.key(modifier_to_key(*modifier), Direction::Release);
        }

        click_result
    })
        .await
        .map_err(|error| format!("点击完成后按键任务执行失败: {error}"))?
}

/// 按顺序点击已配置的点击区域，每个区域使用独立的延迟
pub async fn click_regions(regions: &[ClickRegion]) -> Result<(), String> {
    if regions.is_empty() {
        return Ok(());
    }

    let regions: Vec<(i32, i32, u64)> = regions
        .iter()
        .map(|c| {
            let center_x = c.rect.x + c.rect.width / 2;
            let center_y = c.rect.y + c.rect.height / 2;
            (center_x, center_y, c.delay_ms)
        })
        .collect();

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化鼠标点击失败: {error}"))?;

        for (center_x, center_y, delay_ms) in &regions {
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
        .await
        .map_err(|error| format!("自动点击任务执行失败: {error}"))?
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
