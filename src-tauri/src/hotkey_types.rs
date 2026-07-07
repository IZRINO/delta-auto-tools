use std::collections::HashSet;
use std::sync::Arc;

use tauri::AppHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierKey {
    Ctrl,
    Alt,
    Shift,
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimaryKey {
    Letter(char),
    Digit(char),
    Function(u8),
    Named(NamedKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    Alt,
    Semicolon,
    Comma,
    Period,
    Slash,
    Backslash,
    BracketLeft,
    BracketRight,
    Minus,
    Equal,
    Plus,
    Backquote,
    Quote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

        let has_separator = trimmed.contains('+');
        let mut segments = trimmed.split('+').map(str::trim).peekable();

        if segments.peek().is_none() {
            return Err("热键不能为空".to_string());
        }

        let mut modifiers = HashSet::new();
        let mut primary = None;

        while let Some(segment) = segments.next() {
            let primary_segment = if segment.is_empty()
                && primary.is_none()
                && segments.peek().is_none()
                && trimmed.ends_with('+')
            {
                "+"
            } else if segment.is_empty() {
                continue;
            } else {
                segment
            };

            if has_separator {
                if let Some(modifier) = parse_modifier(primary_segment) {
                    modifiers.insert(modifier);
                    continue;
                }
            }

            if primary.is_some() {
                return Err(format!("热键格式无效，存在多个主键: {trimmed}"));
            }

            primary = Some(parse_primary(primary_segment)?);
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
    } else if segment.eq_ignore_ascii_case("alt") {
        Some(NamedKey::Alt)
    } else if segment == ";" || segment.eq_ignore_ascii_case("semicolon") {
        Some(NamedKey::Semicolon)
    } else if segment == "," || segment.eq_ignore_ascii_case("comma") {
        Some(NamedKey::Comma)
    } else if segment == "." || segment.eq_ignore_ascii_case("period") {
        Some(NamedKey::Period)
    } else if segment == "/" || segment.eq_ignore_ascii_case("slash") {
        Some(NamedKey::Slash)
    } else if segment == "\\" || segment.eq_ignore_ascii_case("backslash") {
        Some(NamedKey::Backslash)
    } else if segment == "[" || segment.eq_ignore_ascii_case("bracketleft") {
        Some(NamedKey::BracketLeft)
    } else if segment == "]" || segment.eq_ignore_ascii_case("bracketright") {
        Some(NamedKey::BracketRight)
    } else if segment == "-" || segment.eq_ignore_ascii_case("minus") {
        Some(NamedKey::Minus)
    } else if segment == "=" || segment.eq_ignore_ascii_case("equal") {
        Some(NamedKey::Equal)
    } else if segment == "+" || segment.eq_ignore_ascii_case("plus") {
        Some(NamedKey::Plus)
    } else if segment == "`" || segment.eq_ignore_ascii_case("backquote") {
        Some(NamedKey::Backquote)
    } else if segment == "'" || segment.eq_ignore_ascii_case("quote") {
        Some(NamedKey::Quote)
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

pub fn modifier_to_string(modifier: ModifierKey) -> &'static str {
    match modifier {
        ModifierKey::Ctrl => "Ctrl",
        ModifierKey::Alt => "Alt",
        ModifierKey::Shift => "Shift",
        ModifierKey::Super => "Super",
    }
}

pub fn primary_to_string(primary: PrimaryKey) -> String {
    match primary {
        PrimaryKey::Letter(value) => value.to_string(),
        PrimaryKey::Digit(value) => value.to_string(),
        PrimaryKey::Function(value) => format!("F{value}"),
        PrimaryKey::Named(NamedKey::Space) => "Space".to_string(),
        PrimaryKey::Named(NamedKey::Enter) => "Enter".to_string(),
        PrimaryKey::Named(NamedKey::Tab) => "Tab".to_string(),
        PrimaryKey::Named(NamedKey::Esc) => "Esc".to_string(),
        PrimaryKey::Named(NamedKey::Up) => "Up".to_string(),
        PrimaryKey::Named(NamedKey::Down) => "Down".to_string(),
        PrimaryKey::Named(NamedKey::Left) => "Left".to_string(),
        PrimaryKey::Named(NamedKey::Right) => "Right".to_string(),
        PrimaryKey::Named(NamedKey::Home) => "Home".to_string(),
        PrimaryKey::Named(NamedKey::End) => "End".to_string(),
        PrimaryKey::Named(NamedKey::PageUp) => "PageUp".to_string(),
        PrimaryKey::Named(NamedKey::PageDown) => "PageDown".to_string(),
        PrimaryKey::Named(NamedKey::Insert) => "Insert".to_string(),
        PrimaryKey::Named(NamedKey::Delete) => "Delete".to_string(),
        PrimaryKey::Named(NamedKey::Backspace) => "Backspace".to_string(),
        PrimaryKey::Named(NamedKey::Alt) => "Alt".to_string(),
        PrimaryKey::Named(NamedKey::Semicolon) => ";".to_string(),
        PrimaryKey::Named(NamedKey::Comma) => ",".to_string(),
        PrimaryKey::Named(NamedKey::Period) => ".".to_string(),
        PrimaryKey::Named(NamedKey::Slash) => "/".to_string(),
        PrimaryKey::Named(NamedKey::Backslash) => "\\".to_string(),
        PrimaryKey::Named(NamedKey::BracketLeft) => "[".to_string(),
        PrimaryKey::Named(NamedKey::BracketRight) => "]".to_string(),
        PrimaryKey::Named(NamedKey::Minus) => "-".to_string(),
        PrimaryKey::Named(NamedKey::Equal) => "=".to_string(),
        PrimaryKey::Named(NamedKey::Plus) => "+".to_string(),
        PrimaryKey::Named(NamedKey::Backquote) => "`".to_string(),
        PrimaryKey::Named(NamedKey::Quote) => "'".to_string(),
    }
}

pub fn binding_to_string(binding: &HotkeyBinding) -> String {
    let primary = primary_to_string(binding.primary);
    let mut segments = Vec::with_capacity(binding.modifiers.len() + 1);
    for modifier in [
        ModifierKey::Ctrl,
        ModifierKey::Alt,
        ModifierKey::Shift,
        ModifierKey::Super,
    ] {
        if binding.modifiers.contains(&modifier) {
            segments.push(modifier_to_string(modifier).to_string());
        }
    }
    segments.push(primary);
    segments.join("+")
}

pub fn hotkey_to_string(raw: &str) -> Result<String, String> {
    HotkeyBinding::parse(raw).map(|binding| binding_to_string(&binding))
}

pub fn hotkey_primary_label(raw: &str) -> Result<String, String> {
    HotkeyBinding::parse(raw).map(|binding| primary_to_string(binding.primary))
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
        KeyboardKey::LeftAlt | KeyboardKey::RightAlt => Some(PrimaryKey::Named(NamedKey::Alt)),
        KeyboardKey::Other(0xBA) => Some(PrimaryKey::Named(NamedKey::Semicolon)),
        KeyboardKey::Other(0xBB) => Some(PrimaryKey::Named(NamedKey::Equal)),
        KeyboardKey::Other(0x6B) => Some(PrimaryKey::Named(NamedKey::Plus)),
        KeyboardKey::Other(0xBC) => Some(PrimaryKey::Named(NamedKey::Comma)),
        KeyboardKey::Other(0xBD) => Some(PrimaryKey::Named(NamedKey::Minus)),
        KeyboardKey::Other(0xBE) => Some(PrimaryKey::Named(NamedKey::Period)),
        KeyboardKey::Other(0xBF) => Some(PrimaryKey::Named(NamedKey::Slash)),
        KeyboardKey::Other(0xC0) => Some(PrimaryKey::Named(NamedKey::Backquote)),
        KeyboardKey::Other(0xDB) => Some(PrimaryKey::Named(NamedKey::BracketLeft)),
        KeyboardKey::Other(0xDC) => Some(PrimaryKey::Named(NamedKey::Backslash)),
        KeyboardKey::Other(0xDD) => Some(PrimaryKey::Named(NamedKey::BracketRight)),
        KeyboardKey::Other(0xDE) => Some(PrimaryKey::Named(NamedKey::Quote)),
        _ => None,
    }
}

pub type HotkeyAction = Arc<dyn Fn(AppHandle) + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoldAction {
    Down,
    Up,
}

pub type HoldActionCallback = Arc<dyn Fn(AppHandle, HoldAction) + Send + Sync + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictPolicy {
    Strict,
    AllowHold,
}

pub struct HotkeyRegistration {
    pub scope: String,
    pub binding: HotkeyBinding,
    pub enabled: bool,
    pub display_name: String,
    pub conflict_policy: ConflictPolicy,
    pub action: HotkeyAction,
}

pub struct HoldRegistration {
    pub scope: String,
    pub binding: HotkeyBinding,
    pub enabled: bool,
    pub display_name: String,
    pub conflict_policy: ConflictPolicy,
    pub action: HoldActionCallback,
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

    #[test]
    fn parses_comma_and_period_hotkeys() {
        assert_eq!(
            HotkeyBinding::parse(",").unwrap().primary,
            PrimaryKey::Named(NamedKey::Comma)
        );
        assert_eq!(
            HotkeyBinding::parse("Ctrl+.").unwrap().primary,
            PrimaryKey::Named(NamedKey::Period)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn maps_comma_and_period_vk_events_to_primary_keys() {
        use willhook::event::KeyboardKey;

        assert_eq!(
            to_primary_key(KeyboardKey::Other(0xBC)),
            Some(PrimaryKey::Named(NamedKey::Comma))
        );
        assert_eq!(
            to_primary_key(KeyboardKey::Other(0xBE)),
            Some(PrimaryKey::Named(NamedKey::Period))
        );
    }

    #[test]
    fn extracts_primary_label_from_modified_hotkey() {
        assert_eq!(hotkey_primary_label("Ctrl+Shift+F6").unwrap(), "F6");
        assert_eq!(hotkey_primary_label("Alt+Space").unwrap(), "Space");
    }

    #[test]
    fn normalizes_modified_hotkey_label_order() {
        assert_eq!(hotkey_to_string("shift+ctrl+-").unwrap(), "Ctrl+Shift+-");
        assert_eq!(
            hotkey_to_string("win+alt+space").unwrap(),
            "Alt+Super+Space"
        );

        assert_eq!(hotkey_to_string("shift++").unwrap(), "Shift++");
    }

    #[test]
    fn parses_standalone_alt_as_primary_key() {
        let binding = HotkeyBinding::parse("alt").expect("should parse");

        assert!(binding.modifiers.is_empty());
        assert_eq!(binding.primary, PrimaryKey::Named(NamedKey::Alt));
        assert_eq!(hotkey_to_string("alt").unwrap(), "Alt");
    }
}
