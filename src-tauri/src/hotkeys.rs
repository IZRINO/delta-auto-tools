use std::{
    collections::{HashMap, HashSet},
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoldAction {
    Down,
    Up,
}

pub type HoldActionCallback = Arc<dyn Fn(AppHandle, HoldAction) + Send + Sync + 'static>;

pub struct HotkeyManager {
    registrations: Arc<Mutex<Vec<HotkeyRegistration>>>,
    hold_registrations: Arc<Mutex<HashMap<String, Vec<HoldRegistration>>>>,
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

struct HoldRegistration {
    scope: String,
    binding: HotkeyBinding,
    enabled: bool,
    action: HoldActionCallback,
}

impl HotkeyManager {
    pub fn start(app: AppHandle) -> Self {
        let registrations = Arc::new(Mutex::new(Vec::new()));
        let hold_registrations = Arc::new(Mutex::new(HashMap::new()));
        let stopped = Arc::new(AtomicBool::new(false));

        #[cfg(target_os = "windows")]
        {
            let Some(hook) = willhook::keyboard_hook() else {
                return Self {
                    registrations,
                    hold_registrations,
                    stopped,
                    install_error: Some(
                        "键盘钩子安装失败，请检查杀毒软件或系统权限设置".to_string(),
                    ),
                    worker: None,
                };
            };

            let worker_registrations = Arc::clone(&registrations);
            let worker_hold_registrations = Arc::clone(&hold_registrations);
            let worker_stopped = Arc::clone(&stopped);
            let worker = thread::Builder::new()
                .name("shared-hotkey-listener".to_string())
                .spawn(move || {
                    run_listener(
                        app,
                        hook,
                        worker_registrations,
                        worker_hold_registrations,
                        worker_stopped,
                    )
                })
                .map_err(|error| format!("启动热键监听线程失败: {error}"));

            match worker {
                Ok(worker) => Self {
                    registrations,
                    hold_registrations,
                    stopped,
                    install_error: None,
                    worker: Some(worker),
                },
                Err(error) => Self {
                    registrations,
                    hold_registrations,
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
                hold_registrations,
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
        for registration in registrations
            .iter_mut()
            .filter(|registration| registration.scope == scope)
        {
            registration.enabled = enabled;
        }
        Ok(())
    }

    pub fn replace_hold_scope(
        &self,
        scope: &str,
        bindings: Vec<(String, HoldActionCallback)>,
    ) -> Result<(), String> {
        if let Some(error) = &self.install_error {
            return Err(error.clone());
        }

        let mut hold_regs = self
            .hold_registrations
            .lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;

        hold_regs.remove(scope);

        if !bindings.is_empty() {
            let regs: Vec<HoldRegistration> = bindings
                .into_iter()
                .map(|(key, action)| {
                    HotkeyBinding::parse(&key).map(|binding| HoldRegistration {
                        scope: scope.to_string(),
                        binding,
                        enabled: true,
                        action,
                    })
                })
                .collect::<Result<_, _>>()?;
            hold_regs.insert(scope.to_string(), regs);
        }

        Ok(())
    }

    pub fn clear_hold_scope(&self, scope: &str) -> Result<(), String> {
        let mut hold_regs = self
            .hold_registrations
            .lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        hold_regs.remove(scope);
        Ok(())
    }

    pub fn active_binding_labels_except(
        &self,
        excluded_scope: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let mut labels = Vec::new();

        {
            let registrations = self
                .registrations
                .lock()
                .map_err(|_| "热键监听状态已损坏".to_string())?;
            labels.extend(
                registrations
                    .iter()
                    .filter(|registration| {
                        registration.enabled && registration.scope != excluded_scope
                    })
                    .map(|registration| {
                        (
                            registration.scope.clone(),
                            types::primary_to_string(registration.binding.primary),
                        )
                    }),
            );
        }

        {
            let hold_registrations = self
                .hold_registrations
                .lock()
                .map_err(|_| "热键监听状态已损坏".to_string())?;
            for (scope, registrations) in hold_registrations.iter() {
                if scope == excluded_scope {
                    continue;
                }
                labels.extend(
                    registrations
                        .iter()
                        .filter(|registration| registration.enabled)
                        .map(|registration| {
                            (
                                registration.scope.clone(),
                                types::binding_to_string(&registration.binding),
                            )
                        }),
                );
            }
        }

        Ok(labels)
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
    hold_registrations: Arc<Mutex<HashMap<String, Vec<HoldRegistration>>>>,
    stopped: Arc<AtomicBool>,
) {
    let mut matcher = HotkeyMatcher::new();
    let mut active_hold_keys: HashMap<PrimaryKey, KeyState> = HashMap::new();
    let mut active_hold_modifiers = HashSet::new();

    while !stopped.load(Ordering::SeqCst) {
        match hook.try_recv() {
            Ok(InputEvent::Keyboard(event)) => {
                let hold_actions = hold_actions_for_event(
                    &hold_registrations,
                    event,
                    &mut active_hold_keys,
                    &mut active_hold_modifiers,
                );
                for (action, hold_action) in hold_actions {
                    action(app.clone(), hold_action);
                }

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
                .filter(|registration| {
                    registration.enabled && matches_binding(&registration.binding, key_state)
                })
                .map(|registration| Arc::clone(&registration.action))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn hold_actions_for_event(
    hold_registrations: &Arc<Mutex<HashMap<String, Vec<HoldRegistration>>>>,
    event: KeyboardEvent,
    active_hold_keys: &mut HashMap<PrimaryKey, KeyState>,
    active_hold_modifiers: &mut HashSet<ModifierKey>,
) -> Vec<(HoldActionCallback, HoldAction)> {
    if matches!(event.is_injected, Some(IsEventInjected::Injected)) {
        return Vec::new();
    }

    let Some(key) = event.key else {
        return Vec::new();
    };
    let modifier = types::to_modifier_key(key);
    let primary = types::to_primary_key(key);

    match event.pressed {
        KeyPress::Down(_) => {
            let Some(primary) = primary else {
                if let Some(modifier) = modifier {
                    active_hold_modifiers.insert(modifier);
                }
                return Vec::new();
            };

            if active_hold_keys.contains_key(&primary) {
                if let Some(modifier) = modifier {
                    active_hold_modifiers.insert(modifier);
                }
                return Vec::new();
            }

            let key_state = KeyState {
                modifiers: active_hold_modifiers.clone(),
                primary,
            };
            let actions =
                hold_actions_for_key_state(hold_registrations, &key_state, HoldAction::Down);
            if !actions.is_empty() {
                active_hold_keys.insert(primary, key_state);
            }
            if let Some(modifier) = modifier {
                active_hold_modifiers.insert(modifier);
            }
            actions
        }
        KeyPress::Up(_) => {
            if let Some(modifier) = modifier {
                active_hold_modifiers.remove(&modifier);
            }
            let Some(primary) = primary else {
                return Vec::new();
            };
            let Some(key_state) = active_hold_keys.remove(&primary) else {
                return Vec::new();
            };
            hold_actions_for_key_state(hold_registrations, &key_state, HoldAction::Up)
        }
        KeyPress::Other(_) => Vec::new(),
    }
}

#[cfg(target_os = "windows")]
fn hold_actions_for_key_state(
    hold_registrations: &Arc<Mutex<HashMap<String, Vec<HoldRegistration>>>>,
    key_state: &KeyState,
    hold_action: HoldAction,
) -> Vec<(HoldActionCallback, HoldAction)> {
    hold_registrations
        .lock()
        .ok()
        .map(|regs| {
            let mut actions = Vec::new();
            for scope_regs in regs.values() {
                for reg in scope_regs {
                    if reg.enabled && matches_binding(&reg.binding, key_state) {
                        actions.push((Arc::clone(&reg.action), hold_action.clone()));
                    }
                }
            }
            actions
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

    #[cfg(target_os = "windows")]
    fn keyboard_event(
        key: willhook::event::KeyboardKey,
        pressed: willhook::event::KeyPress,
    ) -> KeyboardEvent {
        KeyboardEvent {
            pressed,
            key: Some(key),
            is_injected: Some(IsEventInjected::NotInjected),
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn ordinary_hotkey_dispatches_on_key_down_not_key_up() {
        use willhook::event::{IsSystemKeyPress, KeyPress, KeyboardKey};

        let mut matcher = HotkeyMatcher::new();

        let key_state = matcher
            .handle_event(keyboard_event(
                KeyboardKey::F2,
                KeyPress::Down(IsSystemKeyPress::Normal),
            ))
            .expect("按下主键时应立即触发");

        assert!(key_state.modifiers.is_empty());
        assert_eq!(key_state.primary, PrimaryKey::Function(2));
        assert!(matcher
            .handle_event(keyboard_event(
                KeyboardKey::F2,
                KeyPress::Up(IsSystemKeyPress::Normal),
            ))
            .is_none());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn modified_hotkey_dispatches_when_primary_key_goes_down() {
        use willhook::event::{IsSystemKeyPress, KeyPress, KeyboardKey};

        let mut matcher = HotkeyMatcher::new();

        assert!(matcher
            .handle_event(keyboard_event(
                KeyboardKey::LeftControl,
                KeyPress::Down(IsSystemKeyPress::Normal),
            ))
            .is_none());

        let key_state = matcher
            .handle_event(keyboard_event(
                KeyboardKey::F2,
                KeyPress::Down(IsSystemKeyPress::Normal),
            ))
            .expect("组合键应在主键按下时触发");

        assert!(key_state.modifiers.contains(&ModifierKey::Ctrl));
        assert_eq!(key_state.primary, PrimaryKey::Function(2));
        assert!(matcher
            .handle_event(keyboard_event(
                KeyboardKey::F2,
                KeyPress::Up(IsSystemKeyPress::Normal),
            ))
            .is_none());
        assert!(matcher
            .handle_event(keyboard_event(
                KeyboardKey::LeftControl,
                KeyPress::Up(IsSystemKeyPress::Normal),
            ))
            .is_none());

        let key_state_after_modifier_release = matcher
            .handle_event(keyboard_event(
                KeyboardKey::F2,
                KeyPress::Down(IsSystemKeyPress::Normal),
            ))
            .expect("修饰键释放后主键应恢复为无组合键触发");
        assert!(key_state_after_modifier_release.modifiers.is_empty());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn modified_hold_dispatches_down_and_up_with_original_modifiers() {
        use willhook::event::{IsSystemKeyPress, KeyPress, KeyboardKey};

        let callback: HoldActionCallback = Arc::new(|_, _| {});
        let hold_registrations = Arc::new(Mutex::new(HashMap::from([(
            "rapidfire".to_string(),
            vec![HoldRegistration {
                scope: "rapidfire".to_string(),
                binding: HotkeyBinding::parse("Shift+-").expect("should parse"),
                enabled: true,
                action: callback,
            }],
        )])));
        let mut active_hold_keys = HashMap::new();
        let mut active_hold_modifiers = HashSet::new();

        assert!(hold_actions_for_event(
            &hold_registrations,
            keyboard_event(
                KeyboardKey::LeftShift,
                KeyPress::Down(IsSystemKeyPress::Normal)
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        )
        .is_empty());

        let down_actions = hold_actions_for_event(
            &hold_registrations,
            keyboard_event(
                KeyboardKey::Other(0xBD),
                KeyPress::Down(IsSystemKeyPress::Normal),
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert_eq!(down_actions.len(), 1);
        assert_eq!(down_actions[0].1, HoldAction::Down);

        assert!(hold_actions_for_event(
            &hold_registrations,
            keyboard_event(
                KeyboardKey::LeftShift,
                KeyPress::Up(IsSystemKeyPress::Normal)
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        )
        .is_empty());

        let up_actions = hold_actions_for_event(
            &hold_registrations,
            keyboard_event(
                KeyboardKey::Other(0xBD),
                KeyPress::Up(IsSystemKeyPress::Normal),
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert_eq!(up_actions.len(), 1);
        assert_eq!(up_actions[0].1, HoldAction::Up);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn held_primary_key_does_not_repeat_before_release() {
        use willhook::event::{IsSystemKeyPress, KeyPress, KeyboardKey};

        let mut matcher = HotkeyMatcher::new();

        assert!(matcher
            .handle_event(keyboard_event(
                KeyboardKey::F2,
                KeyPress::Down(IsSystemKeyPress::Normal),
            ))
            .is_some());
        assert!(matcher
            .handle_event(keyboard_event(
                KeyboardKey::F2,
                KeyPress::Down(IsSystemKeyPress::Normal),
            ))
            .is_none());
        assert!(matcher
            .handle_event(keyboard_event(
                KeyboardKey::F2,
                KeyPress::Up(IsSystemKeyPress::Normal),
            ))
            .is_none());
        assert!(matcher
            .handle_event(keyboard_event(
                KeyboardKey::F2,
                KeyPress::Down(IsSystemKeyPress::Normal),
            ))
            .is_some());
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
