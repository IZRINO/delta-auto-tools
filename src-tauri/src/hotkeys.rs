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
    event::{InputEvent, IsEventInjected, KeyPress, KeyboardEvent},
    hook::Hook,
};

use crate::hotkey_types::{self as types, HotkeyBinding, ModifierKey, PrimaryKey};

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

        if let Some(modifier) = types::to_modifier_key(key) {
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

        let primary_key = types::to_primary_key(key)?;

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

#[cfg(test)]
mod tests {
    use super::*;

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
