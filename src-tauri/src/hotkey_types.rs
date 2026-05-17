use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierKey {
    Ctrl,
    Alt,
    Shift,
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryKey {
    Letter(char),
    Digit(char),
    Function(u8),
    Named(NamedKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedKey {
    Space,
    Enter,
    Tab,
    Esc,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Backspace,
}

#[derive(Debug, Clone)]
pub struct HotkeyBinding {
    pub modifiers: HashSet<ModifierKey>,
    pub primary: PrimaryKey,
}

impl HotkeyBinding {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("热键不能为空".to_string());
        }

        let segments = trimmed
            .split('+')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();

        if segments.is_empty() {
            return Err("热键不能为空".to_string());
        }

        let mut modifiers = HashSet::new();
        let mut primary = None;

        for segment in segments {
            if let Some(modifier) = parse_modifier(segment) {
                modifiers.insert(modifier);
                continue;
            }

            if primary.is_some() {
                return Err(format!("热键格式无效，存在多个主键: {trimmed}"));
            }

            primary = Some(parse_primary(segment)?);
        }

        let Some(primary) = primary else {
            return Err(format!("热键格式无效，缺少主键: {trimmed}"));
        };

        Ok(Self { modifiers, primary })
    }
}

pub fn parse_modifier(segment: &str) -> Option<ModifierKey> {
    if segment.eq_ignore_ascii_case("ctrl") || segment.eq_ignore_ascii_case("control") {
        return Some(ModifierKey::Ctrl);
    }

    if segment.eq_ignore_ascii_case("alt") {
        return Some(ModifierKey::Alt);
    }

    if segment.eq_ignore_ascii_case("shift") {
        return Some(ModifierKey::Shift);
    }

    if segment.eq_ignore_ascii_case("super")
        || segment.eq_ignore_ascii_case("meta")
        || segment.eq_ignore_ascii_case("win")
        || segment.eq_ignore_ascii_case("windows")
    {
        return Some(ModifierKey::Super);
    }

    None
}

pub fn parse_primary(segment: &str) -> Result<PrimaryKey, String> {
    if segment.len() == 1 {
        let char_value = segment.chars().next().unwrap_or_default();
        if char_value.is_ascii_alphabetic() {
            return Ok(PrimaryKey::Letter(char_value.to_ascii_uppercase()));
        }
        if char_value.is_ascii_digit() {
            return Ok(PrimaryKey::Digit(char_value));
        }
    }

    if let Some(function_number) = segment
        .strip_prefix(['F', 'f'])
        .and_then(|value| value.parse::<u8>().ok())
        .filter(|value| (1..=24).contains(value))
    {
        return Ok(PrimaryKey::Function(function_number));
    }

    let named = if segment.eq_ignore_ascii_case("space") {
        Some(NamedKey::Space)
    } else if segment.eq_ignore_ascii_case("enter") {
        Some(NamedKey::Enter)
    } else if segment.eq_ignore_ascii_case("tab") {
        Some(NamedKey::Tab)
    } else if segment.eq_ignore_ascii_case("esc") || segment.eq_ignore_ascii_case("escape") {
        Some(NamedKey::Esc)
    } else if segment.eq_ignore_ascii_case("up") {
        Some(NamedKey::Up)
    } else if segment.eq_ignore_ascii_case("down") {
        Some(NamedKey::Down)
    } else if segment.eq_ignore_ascii_case("left") {
        Some(NamedKey::Left)
    } else if segment.eq_ignore_ascii_case("right") {
        Some(NamedKey::Right)
    } else if segment.eq_ignore_ascii_case("home") {
        Some(NamedKey::Home)
    } else if segment.eq_ignore_ascii_case("end") {
        Some(NamedKey::End)
    } else if segment.eq_ignore_ascii_case("pageup") {
        Some(NamedKey::PageUp)
    } else if segment.eq_ignore_ascii_case("pagedown") {
        Some(NamedKey::PageDown)
    } else if segment.eq_ignore_ascii_case("insert") {
        Some(NamedKey::Insert)
    } else if segment.eq_ignore_ascii_case("delete") {
        Some(NamedKey::Delete)
    } else if segment.eq_ignore_ascii_case("backspace") {
        Some(NamedKey::Backspace)
    } else {
        None
    };

    named
        .map(PrimaryKey::Named)
        .ok_or_else(|| format!("暂不支持的热键主键: {segment}"))
}

#[cfg(target_os = "windows")]
pub fn to_modifier_key(key: willhook::event::KeyboardKey) -> Option<ModifierKey> {
    use willhook::event::KeyboardKey;
    match key {
        KeyboardKey::LeftControl | KeyboardKey::RightControl => Some(ModifierKey::Ctrl),
        KeyboardKey::LeftAlt | KeyboardKey::RightAlt => Some(ModifierKey::Alt),
        KeyboardKey::LeftShift | KeyboardKey::RightShift => Some(ModifierKey::Shift),
        KeyboardKey::LeftWindows | KeyboardKey::RightWindows => Some(ModifierKey::Super),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
pub fn to_primary_key(key: willhook::event::KeyboardKey) -> Option<PrimaryKey> {
    use willhook::event::KeyboardKey;
    match key {
        KeyboardKey::A => Some(PrimaryKey::Letter('A')),
        KeyboardKey::B => Some(PrimaryKey::Letter('B')),
        KeyboardKey::C => Some(PrimaryKey::Letter('C')),
        KeyboardKey::D => Some(PrimaryKey::Letter('D')),
        KeyboardKey::E => Some(PrimaryKey::Letter('E')),
        KeyboardKey::F => Some(PrimaryKey::Letter('F')),
        KeyboardKey::G => Some(PrimaryKey::Letter('G')),
        KeyboardKey::H => Some(PrimaryKey::Letter('H')),
        KeyboardKey::I => Some(PrimaryKey::Letter('I')),
        KeyboardKey::J => Some(PrimaryKey::Letter('J')),
        KeyboardKey::K => Some(PrimaryKey::Letter('K')),
        KeyboardKey::L => Some(PrimaryKey::Letter('L')),
        KeyboardKey::M => Some(PrimaryKey::Letter('M')),
        KeyboardKey::N => Some(PrimaryKey::Letter('N')),
        KeyboardKey::O => Some(PrimaryKey::Letter('O')),
        KeyboardKey::P => Some(PrimaryKey::Letter('P')),
        KeyboardKey::Q => Some(PrimaryKey::Letter('Q')),
        KeyboardKey::R => Some(PrimaryKey::Letter('R')),
        KeyboardKey::S => Some(PrimaryKey::Letter('S')),
        KeyboardKey::T => Some(PrimaryKey::Letter('T')),
        KeyboardKey::U => Some(PrimaryKey::Letter('U')),
        KeyboardKey::V => Some(PrimaryKey::Letter('V')),
        KeyboardKey::W => Some(PrimaryKey::Letter('W')),
        KeyboardKey::X => Some(PrimaryKey::Letter('X')),
        KeyboardKey::Y => Some(PrimaryKey::Letter('Y')),
        KeyboardKey::Z => Some(PrimaryKey::Letter('Z')),
        KeyboardKey::Number0 => Some(PrimaryKey::Digit('0')),
        KeyboardKey::Number1 => Some(PrimaryKey::Digit('1')),
        KeyboardKey::Number2 => Some(PrimaryKey::Digit('2')),
        KeyboardKey::Number3 => Some(PrimaryKey::Digit('3')),
        KeyboardKey::Number4 => Some(PrimaryKey::Digit('4')),
        KeyboardKey::Number5 => Some(PrimaryKey::Digit('5')),
        KeyboardKey::Number6 => Some(PrimaryKey::Digit('6')),
        KeyboardKey::Number7 => Some(PrimaryKey::Digit('7')),
        KeyboardKey::Number8 => Some(PrimaryKey::Digit('8')),
        KeyboardKey::Number9 => Some(PrimaryKey::Digit('9')),
        KeyboardKey::F1 => Some(PrimaryKey::Function(1)),
        KeyboardKey::F2 => Some(PrimaryKey::Function(2)),
        KeyboardKey::F3 => Some(PrimaryKey::Function(3)),
        KeyboardKey::F4 => Some(PrimaryKey::Function(4)),
        KeyboardKey::F5 => Some(PrimaryKey::Function(5)),
        KeyboardKey::F6 => Some(PrimaryKey::Function(6)),
        KeyboardKey::F7 => Some(PrimaryKey::Function(7)),
        KeyboardKey::F8 => Some(PrimaryKey::Function(8)),
        KeyboardKey::F9 => Some(PrimaryKey::Function(9)),
        KeyboardKey::F10 => Some(PrimaryKey::Function(10)),
        KeyboardKey::F11 => Some(PrimaryKey::Function(11)),
        KeyboardKey::F12 => Some(PrimaryKey::Function(12)),
        KeyboardKey::F13 => Some(PrimaryKey::Function(13)),
        KeyboardKey::F14 => Some(PrimaryKey::Function(14)),
        KeyboardKey::F15 => Some(PrimaryKey::Function(15)),
        KeyboardKey::F16 => Some(PrimaryKey::Function(16)),
        KeyboardKey::F17 => Some(PrimaryKey::Function(17)),
        KeyboardKey::F18 => Some(PrimaryKey::Function(18)),
        KeyboardKey::F19 => Some(PrimaryKey::Function(19)),
        KeyboardKey::F20 => Some(PrimaryKey::Function(20)),
        KeyboardKey::F21 => Some(PrimaryKey::Function(21)),
        KeyboardKey::F22 => Some(PrimaryKey::Function(22)),
        KeyboardKey::F23 => Some(PrimaryKey::Function(23)),
        KeyboardKey::F24 => Some(PrimaryKey::Function(24)),
        KeyboardKey::Space => Some(PrimaryKey::Named(NamedKey::Space)),
        KeyboardKey::Enter => Some(PrimaryKey::Named(NamedKey::Enter)),
        KeyboardKey::Tab => Some(PrimaryKey::Named(NamedKey::Tab)),
        KeyboardKey::Escape => Some(PrimaryKey::Named(NamedKey::Esc)),
        KeyboardKey::ArrowUp => Some(PrimaryKey::Named(NamedKey::Up)),
        KeyboardKey::ArrowDown => Some(PrimaryKey::Named(NamedKey::Down)),
        KeyboardKey::ArrowLeft => Some(PrimaryKey::Named(NamedKey::Left)),
        KeyboardKey::ArrowRight => Some(PrimaryKey::Named(NamedKey::Right)),
        KeyboardKey::Home => Some(PrimaryKey::Named(NamedKey::Home)),
        KeyboardKey::Other(0x23) => Some(PrimaryKey::Named(NamedKey::End)),
        KeyboardKey::PageUp => Some(PrimaryKey::Named(NamedKey::PageUp)),
        KeyboardKey::PageDown => Some(PrimaryKey::Named(NamedKey::PageDown)),
        KeyboardKey::Insert => Some(PrimaryKey::Named(NamedKey::Insert)),
        KeyboardKey::Delete => Some(PrimaryKey::Named(NamedKey::Delete)),
        KeyboardKey::BackSpace => Some(PrimaryKey::Named(NamedKey::Backspace)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_function_hotkey() {
        let binding = HotkeyBinding::parse("F2").expect("should parse");
        assert!(binding.modifiers.is_empty());
        assert_eq!(binding.primary, PrimaryKey::Function(2));
    }

    #[test]
    fn parses_modifier_hotkey() {
        let binding = HotkeyBinding::parse("Ctrl+Shift+F2").expect("should parse");
        assert!(binding.modifiers.contains(&ModifierKey::Ctrl));
        assert!(binding.modifiers.contains(&ModifierKey::Shift));
        assert_eq!(binding.primary, PrimaryKey::Function(2));
    }

    #[test]
    fn rejects_multiple_primary_keys() {
        let error = HotkeyBinding::parse("Ctrl+F+G").expect_err("should reject");
        assert!(error.contains("多个主键"));
    }

    #[test]
    fn parses_bare_letter_hotkey() {
        let binding = HotkeyBinding::parse("F").expect("should parse");
        assert!(binding.modifiers.is_empty());
        assert_eq!(binding.primary, PrimaryKey::Letter('F'));
    }
}
