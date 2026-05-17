use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use tauri::AppHandle;

#[cfg(target_os = "windows")]
use willhook::{
    event::{InputEvent, IsEventInjected, KeyPress, KeyboardEvent, KeyboardKey},
    hook::Hook,
};

pub type HotkeyAction = Arc<dyn Fn(AppHandle) + Send + Sync + 'static>;

pub struct HotkeyManager {
    registrations: Arc<Mutex<Vec<HotkeyRegistration>>>,
    stopped: Arc<AtomicBool>,
    install_error: Option<String>,
    #[cfg(target_os = "windows")]
    worker: Option<JoinHandle<()>>,
    #[cfg(not(target_os = "windows"))]
    _worker: (),
}

struct HotkeyRegistration {
    scope: String,
    binding: HotkeyBinding,
    enabled: bool,
    action: HotkeyAction,
}

impl HotkeyManager {
    pub fn start(app: AppHandle) -> Self {
        let registrations = Arc::new(Mutex::new(Vec::new()));
        let stopped = Arc::new(AtomicBool::new(false));

        #[cfg(target_os = "windows")]
        {
            let Some(hook) = willhook::keyboard_hook() else {
                return Self {
                    registrations,
                    stopped,
                    install_error: Some("键盘钩子安装失败，请检查杀毒软件或系统权限设置".to_string()),
                    worker: None,
                };
            };

            let worker_registrations = Arc::clone(&registrations);
            let worker_stopped = Arc::clone(&stopped);
            let worker = thread::Builder::new()
                .name("shared-hotkey-listener".to_string())
                .spawn(move || run_listener(app, hook, worker_registrations, worker_stopped))
                .map_err(|error| format!("启动热键监听线程失败: {error}"));

            match worker {
                Ok(worker) => Self {
                    registrations,
                    stopped,
                    install_error: None,
                    worker: Some(worker),
                },
                Err(error) => Self {
                    registrations,
                    stopped,
                    install_error: Some(error),
                    worker: None,
                },
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = app;
            Self {
                registrations,
                stopped,
                install_error: Some("当前仅 Windows 桌面环境支持被动热键监听".to_string()),
                _worker: (),
            }
        }
    }

    pub fn replace_scope(
        &self,
        scope: &str,
        bindings: Vec<(String, HotkeyAction)>,
    ) -> Result<(), String> {
        if let Some(error) = &self.install_error {
            return Err(error.clone());
        }

        let mut next_registrations = Vec::with_capacity(bindings.len());
        for (hotkey, action) in bindings {
            next_registrations.push(HotkeyRegistration {
                scope: scope.to_string(),
                binding: HotkeyBinding::parse(&hotkey)?,
                enabled: true,
                action,
            });
        }

        let mut registrations = self
            .registrations
            .lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        registrations.retain(|registration| registration.scope != scope);
        registrations.extend(next_registrations);

        Ok(())
    }

    pub fn clear_scope(&self, scope: &str) -> Result<(), String> {
        let mut registrations = self
            .registrations
            .lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        registrations.retain(|registration| registration.scope != scope);
        Ok(())
    }

    pub fn set_scope_enabled(&self, scope: &str, enabled: bool) -> Result<(), String> {
        let mut registrations = self
            .registrations
            .lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        for registration in registrations.iter_mut().filter(|registration| registration.scope == scope) {
            registration.enabled = enabled;
        }
        Ok(())
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);

        #[cfg(target_os = "windows")]
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ModifierKey {
    Ctrl,
    Alt,
    Shift,
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrimaryKey {
    Letter(char),
    Digit(char),
    Function(u8),
    Named(NamedKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamedKey {
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
struct HotkeyBinding {
    modifiers: HashSet<ModifierKey>,
    primary: PrimaryKey,
}

impl HotkeyBinding {
    fn parse(raw: &str) -> Result<Self, String> {
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

fn parse_modifier(segment: &str) -> Option<ModifierKey> {
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

fn parse_primary(segment: &str) -> Result<PrimaryKey, String> {
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
fn matches_binding(binding: &HotkeyBinding, key_state: &KeyState) -> bool {
    binding.primary == key_state.primary && binding.modifiers == key_state.modifiers
}

#[cfg(target_os = "windows")]
fn run_listener(
    app: AppHandle,
    hook: Hook,
    registrations: Arc<Mutex<Vec<HotkeyRegistration>>>,
    stopped: Arc<AtomicBool>,
) {
    let mut matcher = HotkeyMatcher::new();

    while !stopped.load(Ordering::SeqCst) {
        match hook.try_recv() {
            Ok(InputEvent::Keyboard(event)) => {
                if let Some(key_state) = matcher.handle_event(event) {
                    let actions = actions_for_key_state(&registrations, &key_state);

                    for action in actions {
                        action(app.clone());
                    }
                }
            }
            Ok(_) => {}
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(8));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }
    }
}

#[cfg(target_os = "windows")]
fn actions_for_key_state(
    registrations: &Arc<Mutex<Vec<HotkeyRegistration>>>,
    key_state: &KeyState,
) -> Vec<HotkeyAction> {
    registrations
        .lock()
        .ok()
        .map(|registrations| {
            registrations
                .iter()
                .filter(|registration| registration.enabled && matches_binding(&registration.binding, key_state))
                .map(|registration| Arc::clone(&registration.action))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
struct KeyState {
    modifiers: HashSet<ModifierKey>,
    primary: PrimaryKey,
}

#[cfg(target_os = "windows")]
struct HotkeyMatcher {
    pressed_modifiers: HashSet<ModifierKey>,
    active_primary: Option<PrimaryKey>,
}

#[cfg(target_os = "windows")]
impl HotkeyMatcher {
    fn new() -> Self {
        Self {
            pressed_modifiers: HashSet::new(),
            active_primary: None,
        }
    }

    fn handle_event(&mut self, event: KeyboardEvent) -> Option<KeyState> {
        if matches!(event.is_injected, Some(IsEventInjected::Injected)) {
            return None;
        }

        let key = event.key?;

        if let Some(modifier) = to_modifier_key(key) {
            match event.pressed {
                KeyPress::Down(_) => {
                    self.pressed_modifiers.insert(modifier);
                }
                KeyPress::Up(_) => {
                    self.pressed_modifiers.remove(&modifier);
                }
                KeyPress::Other(_) => {}
            }
            return None;
        }

        let primary_key = to_primary_key(key)?;

        match event.pressed {
            KeyPress::Down(_) => {
                if self.active_primary == Some(primary_key) {
                    return None;
                }
                self.active_primary = Some(primary_key);
                Some(KeyState {
                    modifiers: self.pressed_modifiers.clone(),
                    primary: primary_key,
                })
            }
            KeyPress::Up(_) => {
                if self.active_primary == Some(primary_key) {
                    self.active_primary = None;
                }
                None
            }
            KeyPress::Other(_) => None,
        }
    }
}

#[cfg(target_os = "windows")]
fn to_modifier_key(key: KeyboardKey) -> Option<ModifierKey> {
    match key {
        KeyboardKey::LeftControl | KeyboardKey::RightControl => Some(ModifierKey::Ctrl),
        KeyboardKey::LeftAlt | KeyboardKey::RightAlt => Some(ModifierKey::Alt),
        KeyboardKey::LeftShift | KeyboardKey::RightShift => Some(ModifierKey::Shift),
        KeyboardKey::LeftWindows | KeyboardKey::RightWindows => Some(ModifierKey::Super),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn to_primary_key(key: KeyboardKey) -> Option<PrimaryKey> {
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
    #[cfg(target_os = "windows")]
    fn dispatches_one_key_state_to_all_matching_registrations() {
        let registrations = Arc::new(Mutex::new(vec![
            HotkeyRegistration {
                scope: "timer".to_string(),
                binding: HotkeyBinding::parse("F2").expect("should parse"),
                enabled: true,
                action: Arc::new(|_| {}),
            },
            HotkeyRegistration {
                scope: "timer".to_string(),
                binding: HotkeyBinding::parse("F2").expect("should parse"),
                enabled: true,
                action: Arc::new(|_| {}),
            },
            HotkeyRegistration {
                scope: "morse".to_string(),
                binding: HotkeyBinding::parse("F3").expect("should parse"),
                enabled: true,
                action: Arc::new(|_| {}),
            },
        ]));
        let key_state = KeyState {
            modifiers: HashSet::new(),
            primary: PrimaryKey::Function(2),
        };

        let actions = actions_for_key_state(&registrations, &key_state);

        assert_eq!(actions.len(), 2);
    }
}
