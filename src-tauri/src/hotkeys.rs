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

    fn validate_scope_conflicts(
        &self,
        scope: &str,
        new_bindings: &[HotkeyBinding],
    ) -> Result<(), String> {
        let registrations = self
            .registrations
            .lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        for new_binding in new_bindings {
            if let Some(existing) = registrations.iter().find(|registration| {
                registration.enabled
                    && registration.scope != scope
                    && registration.binding == *new_binding
            }) {
                return Err(format!(
                    "快捷键 {} 与{}的快捷键冲突",
                    types::binding_to_string(new_binding),
                    scope_name(existing.scope.as_str())
                ));
            }
        }
        drop(registrations);

        let hold_registrations = self
            .hold_registrations
            .lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        for new_binding in new_bindings {
            for registrations in hold_registrations.values() {
                if let Some(existing) = registrations.iter().find(|registration| {
                    registration.enabled
                        && registration.scope != scope
                        && registration.binding == *new_binding
                        && !normal_hold_conflict_allowed(scope, registration.scope.as_str())
                }) {
                    return Err(format!(
                        "快捷键 {} 与{}的触发键冲突",
                        types::binding_to_string(new_binding),
                        scope_name(existing.scope.as_str())
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_hold_scope_conflicts(
        &self,
        scope: &str,
        new_bindings: &[HotkeyBinding],
    ) -> Result<(), String> {
        let registrations = self
            .registrations
            .lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        for new_binding in new_bindings {
            if let Some(existing) = registrations.iter().find(|registration| {
                registration.enabled
                    && registration.scope != scope
                    && registration.binding == *new_binding
                    && !normal_hold_conflict_allowed(registration.scope.as_str(), scope)
            }) {
                return Err(format!(
                    "触发键 {} 与{}的快捷键冲突",
                    types::binding_to_string(new_binding),
                    scope_name(existing.scope.as_str())
                ));
            }
        }
        Ok(())
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
        let parsed_bindings = next_registrations
            .iter()
            .map(|registration| registration.binding.clone())
            .collect::<Vec<_>>();
        self.validate_scope_conflicts(scope, parsed_bindings.as_slice())?;

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

        let mut regs = Vec::with_capacity(bindings.len());
        for (key, action) in bindings {
            regs.push(HoldRegistration {
                scope: scope.to_string(),
                binding: HotkeyBinding::parse(&key)?,
                enabled: true,
                action,
            });
        }
        let parsed_bindings = regs
            .iter()
            .map(|registration| registration.binding.clone())
            .collect::<Vec<_>>();
        self.validate_hold_scope_conflicts(scope, parsed_bindings.as_slice())?;

        let mut hold_regs = self
            .hold_registrations
            .lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;

        hold_regs.remove(scope);

        if !regs.is_empty() {
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
}

fn normal_hold_conflict_allowed(normal_scope: &str, hold_scope: &str) -> bool {
    normal_scope == "timer" && hold_scope == "rapidfire"
}

fn scope_name(scope: &str) -> &'static str {
    match scope {
        "morse" => "摩斯密码解析",
        "timer" => "计时器",
        "rapidfire" => "连发器",
        _ => "其他工具",
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
    let mut active_hold_keys: HashMap<PrimaryKey, Vec<HotkeyBinding>> = HashMap::new();
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
    active_hold_keys: &mut HashMap<PrimaryKey, Vec<HotkeyBinding>>,
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
                let Some(modifier) = modifier else {
                    return Vec::new();
                };
                if active_hold_modifiers.insert(modifier) {
                    return transition_pressed_hold_keys(
                        hold_registrations,
                        active_hold_keys,
                        active_hold_modifiers,
                        None,
                    );
                }
                return Vec::new();
            };

            if active_hold_keys.contains_key(&primary) {
                if let Some(modifier) = modifier {
                    if active_hold_modifiers.insert(modifier) {
                        return transition_pressed_hold_keys(
                            hold_registrations,
                            active_hold_keys,
                            active_hold_modifiers,
                            Some(primary),
                        );
                    }
                }
                return Vec::new();
            }

            let key_state = key_state_for_primary(primary, active_hold_modifiers);
            let (active_bindings, mut actions) =
                hold_matches_for_key_state(hold_registrations, &key_state, HoldAction::Down);
            active_hold_keys.insert(primary, active_bindings);

            if let Some(modifier) = modifier {
                if active_hold_modifiers.insert(modifier) {
                    actions.extend(transition_pressed_hold_keys(
                        hold_registrations,
                        active_hold_keys,
                        active_hold_modifiers,
                        Some(primary),
                    ));
                }
            }

            actions
        }
        KeyPress::Up(_) => {
            let modifier_changed = modifier
                .map(|modifier| active_hold_modifiers.remove(&modifier))
                .unwrap_or(false);

            let mut actions = Vec::new();
            if let Some(primary) = primary {
                if let Some(active_bindings) = active_hold_keys.remove(&primary) {
                    actions.extend(hold_actions_for_bindings(
                        hold_registrations,
                        active_bindings.as_slice(),
                        HoldAction::Up,
                    ));
                }
            }

            if modifier_changed {
                actions.extend(transition_pressed_hold_keys(
                    hold_registrations,
                    active_hold_keys,
                    active_hold_modifiers,
                    primary,
                ));
            }

            actions
        }
        KeyPress::Other(_) => Vec::new(),
    }
}

#[cfg(target_os = "windows")]
fn transition_pressed_hold_keys(
    hold_registrations: &Arc<Mutex<HashMap<String, Vec<HoldRegistration>>>>,
    active_hold_keys: &mut HashMap<PrimaryKey, Vec<HotkeyBinding>>,
    active_hold_modifiers: &HashSet<ModifierKey>,
    ignored_primary: Option<PrimaryKey>,
) -> Vec<(HoldActionCallback, HoldAction)> {
    let primaries = active_hold_keys
        .keys()
        .copied()
        .filter(|primary| Some(*primary) != ignored_primary)
        .collect::<Vec<_>>();
    let mut actions = Vec::new();

    for primary in primaries {
        let next_state = key_state_for_primary(primary, active_hold_modifiers);
        let next_bindings = hold_bindings_for_key_state(hold_registrations, &next_state);
        let current_bindings = active_hold_keys.remove(&primary).unwrap_or_default();

        if same_hold_bindings(current_bindings.as_slice(), next_bindings.as_slice()) {
            active_hold_keys.insert(primary, current_bindings);
            continue;
        }

        let removed_bindings = current_bindings
            .iter()
            .filter(|binding| !next_bindings.contains(binding))
            .cloned()
            .collect::<Vec<_>>();
        actions.extend(hold_actions_for_bindings(
            hold_registrations,
            removed_bindings.as_slice(),
            HoldAction::Up,
        ));

        let added_bindings = next_bindings
            .iter()
            .filter(|binding| !current_bindings.contains(binding))
            .cloned()
            .collect::<Vec<_>>();
        actions.extend(hold_actions_for_bindings(
            hold_registrations,
            added_bindings.as_slice(),
            HoldAction::Down,
        ));

        active_hold_keys.insert(primary, next_bindings);
    }

    actions
}

#[cfg(target_os = "windows")]
fn key_state_for_primary(
    primary: PrimaryKey,
    active_hold_modifiers: &HashSet<ModifierKey>,
) -> KeyState {
    KeyState {
        modifiers: effective_hold_modifiers(primary, active_hold_modifiers),
        primary,
    }
}

#[cfg(target_os = "windows")]
fn effective_hold_modifiers(
    primary: PrimaryKey,
    active_hold_modifiers: &HashSet<ModifierKey>,
) -> HashSet<ModifierKey> {
    let mut modifiers = active_hold_modifiers.clone();
    if matches!(primary, PrimaryKey::Named(types::NamedKey::Alt)) {
        modifiers.remove(&ModifierKey::Alt);
    }
    modifiers
}

#[cfg(target_os = "windows")]
fn same_hold_bindings(left: &[HotkeyBinding], right: &[HotkeyBinding]) -> bool {
    left.len() == right.len() && left.iter().all(|binding| right.contains(binding))
}

#[cfg(target_os = "windows")]
fn matches_hold_binding(binding: &HotkeyBinding, key_state: &KeyState) -> bool {
    binding.primary == key_state.primary && binding.modifiers.is_subset(&key_state.modifiers)
}

#[cfg(target_os = "windows")]
fn hold_matches_for_key_state(
    hold_registrations: &Arc<Mutex<HashMap<String, Vec<HoldRegistration>>>>,
    key_state: &KeyState,
    hold_action: HoldAction,
) -> (Vec<HotkeyBinding>, Vec<(HoldActionCallback, HoldAction)>) {
    hold_registrations
        .lock()
        .ok()
        .map(|regs| {
            let mut bindings = Vec::new();
            let mut actions = Vec::new();
            for scope_regs in regs.values() {
                for reg in scope_regs {
                    if reg.enabled && matches_hold_binding(&reg.binding, key_state) {
                        if !bindings.contains(&reg.binding) {
                            bindings.push(reg.binding.clone());
                        }
                        actions.push((Arc::clone(&reg.action), hold_action.clone()));
                    }
                }
            }
            (bindings, actions)
        })
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn hold_bindings_for_key_state(
    hold_registrations: &Arc<Mutex<HashMap<String, Vec<HoldRegistration>>>>,
    key_state: &KeyState,
) -> Vec<HotkeyBinding> {
    hold_matches_for_key_state(hold_registrations, key_state, HoldAction::Down).0
}

#[cfg(target_os = "windows")]
fn hold_actions_for_bindings(
    hold_registrations: &Arc<Mutex<HashMap<String, Vec<HoldRegistration>>>>,
    bindings: &[HotkeyBinding],
    hold_action: HoldAction,
) -> Vec<(HoldActionCallback, HoldAction)> {
    if bindings.is_empty() {
        return Vec::new();
    }

    hold_registrations
        .lock()
        .ok()
        .map(|regs| {
            let mut actions = Vec::new();
            for binding in bindings {
                for scope_regs in regs.values() {
                    for reg in scope_regs {
                        if reg.enabled && reg.binding == *binding {
                            actions.push((Arc::clone(&reg.action), hold_action.clone()));
                        }
                    }
                }
            }
            actions
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

    fn test_manager() -> HotkeyManager {
        HotkeyManager {
            registrations: Arc::new(Mutex::new(Vec::new())),
            hold_registrations: Arc::new(Mutex::new(HashMap::new())),
            stopped: Arc::new(AtomicBool::new(false)),
            install_error: None,
            #[cfg(target_os = "windows")]
            worker: None,
            #[cfg(not(target_os = "windows"))]
            _worker: (),
        }
    }

    #[test]
    fn replace_scope_rejects_morse_binding_when_existing_hold_binding_matches() {
        let manager = test_manager();
        let callback: HoldActionCallback = Arc::new(|_, _| {});
        manager
            .replace_hold_scope("rapidfire", vec![("Shift+-".to_string(), callback)])
            .expect("应注册连发器组合触发键");

        let action: HotkeyAction = Arc::new(|_| {});
        let error = manager
            .replace_scope("morse", vec![("Shift+-".to_string(), action)])
            .expect_err("摩斯快捷键不能复用连发器触发键");

        assert!(error.contains("与连发器的触发键冲突"));
    }

    #[test]
    fn replace_hold_scope_rejects_existing_normal_binding_from_other_scope() {
        let manager = test_manager();
        let action: HotkeyAction = Arc::new(|_| {});
        manager
            .replace_scope("morse", vec![("Ctrl+F2".to_string(), action)])
            .expect("应注册摩斯快捷键");

        let callback: HoldActionCallback = Arc::new(|_, _| {});
        let error = manager
            .replace_hold_scope("rapidfire", vec![("Ctrl+F2".to_string(), callback)])
            .expect_err("连发器触发键不能复用其他工具快捷键");

        assert!(error.contains("与摩斯密码解析的快捷键冲突"));
    }

    #[test]
    fn timer_scope_allows_existing_rapidfire_hold_binding() {
        let manager = test_manager();
        let callback: HoldActionCallback = Arc::new(|_, _| {});
        manager
            .replace_hold_scope("rapidfire", vec![("F2".to_string(), callback)])
            .expect("应注册连发器触发键");

        let action: HotkeyAction = Arc::new(|_| {});
        manager
            .replace_scope("timer", vec![("F2".to_string(), action)])
            .expect("计时器快捷键允许复用连发器触发键");
    }

    #[test]
    fn rapidfire_hold_scope_allows_existing_timer_binding() {
        let manager = test_manager();
        let action: HotkeyAction = Arc::new(|_| {});
        manager
            .replace_scope("timer", vec![("F3".to_string(), action)])
            .expect("应注册计时器快捷键");

        let callback: HoldActionCallback = Arc::new(|_, _| {});
        manager
            .replace_hold_scope("rapidfire", vec![("F3".to_string(), callback)])
            .expect("连发器触发键允许复用计时器快捷键");
    }

    #[test]
    fn replace_scope_allows_same_scope_replacement() {
        let manager = test_manager();
        let action: HotkeyAction = Arc::new(|_| {});
        manager
            .replace_scope("timer", vec![("F2".to_string(), Arc::clone(&action))])
            .expect("首次注册应成功");

        manager
            .replace_scope("timer", vec![("F2".to_string(), action)])
            .expect("同一工具覆盖自身快捷键应成功");
    }

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
    fn timer_and_rapidfire_same_key_dispatch_together() {
        use willhook::event::{IsSystemKeyPress, KeyPress, KeyboardKey};

        let timer_action: HotkeyAction = Arc::new(|_| {});
        let rapidfire_action: HoldActionCallback = Arc::new(|_, _| {});
        let registrations = Arc::new(Mutex::new(vec![HotkeyRegistration {
            scope: "timer".to_string(),
            binding: HotkeyBinding::parse("F2").unwrap(),
            enabled: true,
            action: Arc::clone(&timer_action),
        }]));
        let hold_registrations = Arc::new(Mutex::new(HashMap::from([(
            "rapidfire".to_string(),
            vec![HoldRegistration {
                scope: "rapidfire".to_string(),
                binding: HotkeyBinding::parse("F2").unwrap(),
                enabled: true,
                action: Arc::clone(&rapidfire_action),
            }],
        )])));
        let mut active_hold_keys = HashMap::new();
        let mut active_hold_modifiers = HashSet::new();
        let event = keyboard_event(KeyboardKey::F2, KeyPress::Down(IsSystemKeyPress::Normal));

        let hold_actions = hold_actions_for_event(
            &hold_registrations,
            event,
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        let mut matcher = HotkeyMatcher::new();
        let key_state = matcher.handle_event(event).expect("计时器普通快捷键应触发");
        let normal_actions = actions_for_key_state(&registrations, &key_state);

        assert_eq!(hold_actions.len(), 1);
        assert_eq!(hold_actions[0].1, HoldAction::Down);
        assert!(Arc::ptr_eq(&hold_actions[0].0, &rapidfire_action));
        assert_eq!(normal_actions.len(), 1);
        assert!(Arc::ptr_eq(&normal_actions[0], &timer_action));
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

        let modifier_up_actions = hold_actions_for_event(
            &hold_registrations,
            keyboard_event(
                KeyboardKey::LeftShift,
                KeyPress::Up(IsSystemKeyPress::Normal),
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert_eq!(modifier_up_actions.len(), 1);
        assert_eq!(modifier_up_actions[0].1, HoldAction::Up);

        let primary_up_actions = hold_actions_for_event(
            &hold_registrations,
            keyboard_event(
                KeyboardKey::Other(0xBD),
                KeyPress::Up(IsSystemKeyPress::Normal),
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert!(primary_up_actions.is_empty());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn hold_keeps_bare_binding_active_when_modified_binding_releases() {
        use willhook::event::{IsSystemKeyPress, KeyPress, KeyboardKey};

        let modified_callback: HoldActionCallback = Arc::new(|_, _| {});
        let bare_callback: HoldActionCallback = Arc::new(|_, _| {});
        let hold_registrations = Arc::new(Mutex::new(HashMap::from([(
            "rapidfire".to_string(),
            vec![
                HoldRegistration {
                    scope: "rapidfire".to_string(),
                    binding: HotkeyBinding::parse("Shift+1").expect("should parse"),
                    enabled: true,
                    action: Arc::clone(&modified_callback),
                },
                HoldRegistration {
                    scope: "rapidfire".to_string(),
                    binding: HotkeyBinding::parse("1").expect("should parse"),
                    enabled: true,
                    action: Arc::clone(&bare_callback),
                },
            ],
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
                KeyboardKey::Number1,
                KeyPress::Down(IsSystemKeyPress::Normal),
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert_eq!(down_actions.len(), 2);
        assert!(Arc::ptr_eq(&down_actions[0].0, &modified_callback));
        assert_eq!(down_actions[0].1, HoldAction::Down);
        assert!(Arc::ptr_eq(&down_actions[1].0, &bare_callback));
        assert_eq!(down_actions[1].1, HoldAction::Down);

        let modifier_up_actions = hold_actions_for_event(
            &hold_registrations,
            keyboard_event(
                KeyboardKey::LeftShift,
                KeyPress::Up(IsSystemKeyPress::Normal),
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert_eq!(modifier_up_actions.len(), 1);
        assert!(Arc::ptr_eq(&modifier_up_actions[0].0, &modified_callback));
        assert_eq!(modifier_up_actions[0].1, HoldAction::Up);

        let up_actions = hold_actions_for_event(
            &hold_registrations,
            keyboard_event(KeyboardKey::Number1, KeyPress::Up(IsSystemKeyPress::Normal)),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert_eq!(up_actions.len(), 1);
        assert!(Arc::ptr_eq(&up_actions[0].0, &bare_callback));
        assert_eq!(up_actions[0].1, HoldAction::Up);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn hold_keeps_bare_binding_active_when_modifier_presses_late() {
        use willhook::event::{IsSystemKeyPress, KeyPress, KeyboardKey};

        let bare_callback: HoldActionCallback = Arc::new(|_, _| {});
        let modified_callback: HoldActionCallback = Arc::new(|_, _| {});
        let hold_registrations = Arc::new(Mutex::new(HashMap::from([(
            "rapidfire".to_string(),
            vec![
                HoldRegistration {
                    scope: "rapidfire".to_string(),
                    binding: HotkeyBinding::parse("1").expect("should parse"),
                    enabled: true,
                    action: Arc::clone(&bare_callback),
                },
                HoldRegistration {
                    scope: "rapidfire".to_string(),
                    binding: HotkeyBinding::parse("Shift+1").expect("should parse"),
                    enabled: true,
                    action: Arc::clone(&modified_callback),
                },
            ],
        )])));
        let mut active_hold_keys = HashMap::new();
        let mut active_hold_modifiers = HashSet::new();

        let down_actions = hold_actions_for_event(
            &hold_registrations,
            keyboard_event(
                KeyboardKey::Number1,
                KeyPress::Down(IsSystemKeyPress::Normal),
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert_eq!(down_actions.len(), 1);
        assert!(Arc::ptr_eq(&down_actions[0].0, &bare_callback));
        assert_eq!(down_actions[0].1, HoldAction::Down);

        let modifier_down_actions = hold_actions_for_event(
            &hold_registrations,
            keyboard_event(
                KeyboardKey::LeftShift,
                KeyPress::Down(IsSystemKeyPress::Normal),
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert_eq!(modifier_down_actions.len(), 1);
        assert!(Arc::ptr_eq(&modifier_down_actions[0].0, &modified_callback));
        assert_eq!(modifier_down_actions[0].1, HoldAction::Down);

        let modifier_up_actions = hold_actions_for_event(
            &hold_registrations,
            keyboard_event(
                KeyboardKey::LeftShift,
                KeyPress::Up(IsSystemKeyPress::Normal),
            ),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert_eq!(modifier_up_actions.len(), 1);
        assert!(Arc::ptr_eq(&modifier_up_actions[0].0, &modified_callback));
        assert_eq!(modifier_up_actions[0].1, HoldAction::Up);

        let up_actions = hold_actions_for_event(
            &hold_registrations,
            keyboard_event(KeyboardKey::Number1, KeyPress::Up(IsSystemKeyPress::Normal)),
            &mut active_hold_keys,
            &mut active_hold_modifiers,
        );
        assert_eq!(up_actions.len(), 1);
        assert!(Arc::ptr_eq(&up_actions[0].0, &bare_callback));
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
