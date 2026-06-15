use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use willhook::{
    event::{InputEvent, IsEventInjected, KeyPress, KeyboardEvent},
    hook::Hook,
};

#[cfg(target_os = "windows")]
use crossbeam_channel::Receiver;

use crate::global_state::GlobalState;
use crate::hotkey_types::{
    self as types, HotkeyBinding, ModifierKey, PrimaryKey,
};

pub use crate::hotkey_types::{
    ConflictPolicy, HoldAction, HoldActionCallback, HoldRegistration, HotkeyAction, HotkeyRegistration,
};

pub struct HotkeyManager {
    registrations: Arc<Mutex<Vec<HotkeyRegistration>>>,
    hold_registrations: Arc<Mutex<HashMap<String, Vec<HoldRegistration>>>>,
    stopped: Arc<AtomicBool>,
    install_error: Option<String>,
    /// KeySuppressor：通过 WH_KEYBOARD_LL 钩子吞噬被抑制的按键事件（懒加载）
    #[cfg(target_os = "windows")]
    key_suppressor: Arc<Mutex<Option<crate::key_suppressor::KeySuppressor>>>,
    /// 接收 KeySuppressor 转发的被抑制事件（懒加载，通过 Arc<Mutex> 共享给 run_listener）
    #[cfg(target_os = "windows")]
    suppressed_rx: Arc<Mutex<Option<Receiver<crate::key_suppressor::SuppressedKeyboardEvent>>>>,
    /// 被抑制按键 VK 集合的共享引用，用于 run_listener 过滤 willhook 重复事件
    #[cfg(target_os = "windows")]
    suppressed_vk_set: Arc<Mutex<Option<Arc<Mutex<std::collections::HashSet<u32>>>>>>,
    #[cfg(target_os = "windows")]
    worker: Option<JoinHandle<()>>,
    #[cfg(not(target_os = "windows"))]
    _worker: (),
}

impl HotkeyManager {
    pub fn start(app: AppHandle) -> Self {
        let registrations = Arc::new(Mutex::new(Vec::new()));
        let hold_registrations = Arc::new(Mutex::new(HashMap::new()));
        let stopped = Arc::new(AtomicBool::new(false));

        #[cfg(target_os = "windows")]
        {
            // KeySuppressor 不再无条件启动，改为懒加载
            // 只在有 ignore_trigger_key 卡片启用时才安装第二个 WH_KEYBOARD_LL 钩子
            let key_suppressor = Arc::new(Mutex::new(None));
            let suppressed_rx = Arc::new(Mutex::new(None));
            let suppressed_vk_set = Arc::new(Mutex::new(None));

            let Some(hook) = willhook::keyboard_hook() else {
                return Self {
                    registrations,
                    hold_registrations,
                    stopped,
                    install_error: Some(
                        "键盘钩子安装失败，请检查杀毒软件或系统权限设置".to_string(),
                    ),
                    key_suppressor,
                    suppressed_rx,
                    suppressed_vk_set,
                    worker: None,
                };
            };

            let worker_registrations = Arc::clone(&registrations);
            let worker_hold_registrations = Arc::clone(&hold_registrations);
            let worker_stopped = Arc::clone(&stopped);
            let worker_suppressed_rx = Arc::clone(&suppressed_rx);
            let worker_suppressed_vk_set = Arc::clone(&suppressed_vk_set);
            let worker = thread::Builder::new()
                .name("shared-hotkey-listener".to_string())
                .spawn(move || {
                    run_listener(
                        app,
                        hook,
                        worker_registrations,
                        worker_hold_registrations,
                        worker_stopped,
                        worker_suppressed_rx,
                        worker_suppressed_vk_set,
                    )
                })
                .map_err(|error| format!("启动热键监听线程失败: {error}"));

            match worker {
                Ok(worker) => Self {
                    registrations,
                    hold_registrations,
                    stopped,
                    install_error: None,
                    key_suppressor,
                    suppressed_rx,
                    suppressed_vk_set,
                    worker: Some(worker),
                },
                Err(error) => Self {
                    registrations,
                    hold_registrations,
                    stopped,
                    install_error: Some(error),
                    key_suppressor,
                    suppressed_rx,
                    suppressed_vk_set,
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

    /// 懒加载启动 KeySuppressor（仅在有 ignore_trigger_key 卡片启用时调用）
    #[cfg(target_os = "windows")]
    pub fn start_suppressor(&self) -> Result<(), String> {
        let mut suppressor_guard = self.key_suppressor.lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        if suppressor_guard.is_some() {
            return Ok(()); // 已经启动
        }

        let (suppressor, rx) = crate::key_suppressor::KeySuppressor::start()?;
        let vk_set = suppressor.suppressed_keys_ref();

        *suppressor_guard = Some(suppressor);

        let mut rx_guard = self.suppressed_rx.lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        *rx_guard = Some(rx);

        let mut vk_guard = self.suppressed_vk_set.lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        *vk_guard = Some(vk_set);

        Ok(())
    }

    /// 停止 KeySuppressor（当所有抑制需求消失时调用）
    #[cfg(target_os = "windows")]
    pub fn stop_suppressor(&self) -> Result<(), String> {
        let mut suppressor_guard = self.key_suppressor.lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        if suppressor_guard.is_none() {
            return Ok(()); // 已经停止
        }

        // 先清理抑制列表，再 Drop suppressor（触发钩子卸载）
        if let Some(suppressor) = suppressor_guard.take() {
            suppressor.clear_all();
        }

        let mut rx_guard = self.suppressed_rx.lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        *rx_guard = None;

        let mut vk_guard = self.suppressed_vk_set.lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        *vk_guard = None;

        Ok(())
    }

    /// 抑制指定按键：物理按键事件不会到达前台应用，但热键回调仍正常触发
    #[cfg(target_os = "windows")]
    pub fn suppress_key(&self, key: &str) -> Result<bool, String> {
        // 确保 suppressor 已启动
        self.start_suppressor()?;

        let guard = self.key_suppressor.lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        if let Some(ref suppressor) = *guard {
            let vk = crate::key_suppressor::hotkey_primary_to_vk(key)
                .ok_or_else(|| format!("无法解析按键: {key}"))?;
            Ok(suppressor.suppress(vk))
        } else {
            Err("按键抑制钩子未安装".to_string())
        }
    }

    /// 取消抑制指定按键
    #[cfg(target_os = "windows")]
    pub fn unsuppress_key(&self, key: &str) -> Result<bool, String> {
        let guard = self.key_suppressor.lock()
            .map_err(|_| "热键监听状态已损坏".to_string())?;
        if let Some(ref suppressor) = *guard {
            let vk = crate::key_suppressor::hotkey_primary_to_vk(key)
                .ok_or_else(|| format!("无法解析按键: {key}"))?;
            Ok(suppressor.unsuppress(vk))
        } else {
            Err("按键抑制钩子未安装".to_string())
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn suppress_key(&self, _key: &str) -> Result<bool, String> {
        Err("当前仅 Windows 支持按键抑制".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    pub fn unsuppress_key(&self, _key: &str) -> Result<bool, String> {
        Err("当前仅 Windows 支持按键抑制".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    pub fn start_suppressor(&self) -> Result<(), String> {
        Err("当前仅 Windows 支持按键抑制".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    pub fn stop_suppressor(&self) -> Result<(), String> {
        Err("当前仅 Windows 支持按键抑制".to_string())
    }

    /// 取消所有被抑制的按键（应用关闭或全局关闭时调用）
    #[cfg(target_os = "windows")]
    pub fn clear_all_suppressions(&self) {
        if let Ok(guard) = self.key_suppressor.lock() {
            if let Some(ref suppressor) = *guard {
                suppressor.clear_all();
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn clear_all_suppressions(&self) {}

    fn validate_scope_conflicts(
        &self,
        scope: &str,
        new_bindings: &[HotkeyBinding],
        conflict_policy: ConflictPolicy,
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
                    existing.display_name
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
                        && !(conflict_policy == ConflictPolicy::AllowHold
                        && registration.conflict_policy == ConflictPolicy::AllowHold)
                }) {
                    return Err(format!(
                        "快捷键 {} 与{}的触发键冲突",
                        types::binding_to_string(new_binding),
                        existing.display_name
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
        conflict_policy: ConflictPolicy,
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
                    && !(registration.conflict_policy == ConflictPolicy::AllowHold
                    && conflict_policy == ConflictPolicy::AllowHold)
            }) {
                return Err(format!(
                    "触发键 {} 与{}的快捷键冲突",
                    types::binding_to_string(new_binding),
                    existing.display_name
                ));
            }
        }
        Ok(())
    }

    pub fn replace_scope(
        &self,
        scope: &str,
        bindings: Vec<(String, HotkeyAction)>,
        display_name: String,
        conflict_policy: ConflictPolicy,
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
                display_name: display_name.clone(),
                conflict_policy,
                action,
            });
        }
        let parsed_bindings = next_registrations
            .iter()
            .map(|registration| registration.binding.clone())
            .collect::<Vec<_>>();
        self.validate_scope_conflicts(scope, parsed_bindings.as_slice(), conflict_policy)?;

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
        display_name: String,
        conflict_policy: ConflictPolicy,
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
                display_name: display_name.clone(),
                conflict_policy,
                action,
            });
        }
        let parsed_bindings = regs
            .iter()
            .map(|registration| registration.binding.clone())
            .collect::<Vec<_>>();
        self.validate_hold_scope_conflicts(scope, parsed_bindings.as_slice(), conflict_policy)?;

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

/// 从 willhook KeyboardEvent 提取主键的 Windows VK code
#[cfg(target_os = "windows")]
fn keyboard_event_to_vk(event: &KeyboardEvent) -> Option<u32> {
    use crate::key_suppressor::keyboard_key_to_vk;
    event.key.as_ref().and_then(|k| keyboard_key_to_vk(k))
}

#[cfg(target_os = "windows")]
fn is_event_suppressed(
    event: &KeyboardEvent,
    suppressed_vk_set: &Arc<Mutex<Option<Arc<Mutex<std::collections::HashSet<u32>>>>>>,
) -> bool {
    suppressed_vk_set
        .lock()
        .ok()
        .and_then(|guard| {
            guard.as_ref().and_then(|vk_set| {
                keyboard_event_to_vk(event).map(|vk| {
                    vk_set
                        .lock()
                        .map(|set| set.contains(&vk))
                        .unwrap_or(false)
                })
            })
        })
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn run_listener(
    app: AppHandle,
    hook: Hook,
    registrations: Arc<Mutex<Vec<HotkeyRegistration>>>,
    hold_registrations: Arc<Mutex<HashMap<String, Vec<HoldRegistration>>>>,
    stopped: Arc<AtomicBool>,
    suppressed_rx: Arc<Mutex<Option<Receiver<crate::key_suppressor::SuppressedKeyboardEvent>>>>,
    suppressed_vk_set: Arc<Mutex<Option<Arc<Mutex<std::collections::HashSet<u32>>>>>>,
) {
    let mut matcher = HotkeyMatcher::new();
    let mut active_hold_keys: HashMap<PrimaryKey, Vec<HotkeyBinding>> = HashMap::new();
    let mut active_hold_modifiers = HashSet::new();

    while !stopped.load(Ordering::SeqCst) {
        // 1. 处理 willhook 正常事件
        match hook.try_recv() {
            Ok(InputEvent::Keyboard(event)) => {
                // 全局总开关关闭时，忽略所有热键事件（不触发任何回调）。
                let global_enabled = app
                    .try_state::<GlobalState>()
                    .map(|state| state.enabled())
                    .unwrap_or(true);

                // 消除双重事件分发：如果该键正在被 KeySuppressor 抑制，
                // willhook 收到的是 KeySuppressor 钩子链传递过来的原始事件，
                // 但 KeySuppressor 已吞噬该事件并转发到 suppressed_rx。
                // 为避免同一事件被处理两次，跳过 willhook 对被抑制键的事件。
                let is_suppressed = is_event_suppressed(&event, &suppressed_vk_set);

                if global_enabled && !is_suppressed {
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
            }
            Ok(_) => {}
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }

        // 2. 处理 KeySuppressor 转发的被抑制事件
        //    这些事件已被 WH_KEYBOARD_LL 钩子吞噬，不会到达前台应用，
        //    但热键回调仍需正常触发
        if let Some(rx) = suppressed_rx.lock().ok().and_then(|mut g| g.take() ) {
            while let Ok(suppressed_event) = rx.try_recv() {
                let global_enabled = app
                    .try_state::<GlobalState>()
                    .map(|state| state.enabled())
                    .unwrap_or(true);
                if !global_enabled {
                    continue;
                }

                // 将被抑制事件转换为 willhook KeyboardEvent
                let event =
                    crate::key_suppressor::suppressed_event_to_willhook_event(&suppressed_event);

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
            // 把 rx 放回去
            if let Ok(mut guard) = suppressed_rx.lock() {
                *guard = Some(rx);
            }
        }

        thread::sleep(Duration::from_millis(1));
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
            key_suppressor: Arc::new(Mutex::new(None)),
            #[cfg(target_os = "windows")]
            suppressed_rx: Arc::new(Mutex::new(None)),
            #[cfg(target_os = "windows")]
            suppressed_vk_set: Arc::new(Mutex::new(None)),
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
            .replace_hold_scope(
                "rapidfire",
                vec![("Shift+-".to_string(), callback)],
                "连发器".to_string(),
                ConflictPolicy::AllowHold,
            )
            .expect("应注册连发器组合触发键");

        let action: HotkeyAction = Arc::new(|_| {});
        let error = manager
            .replace_scope(
                "morse",
                vec![("Shift+-".to_string(), action)],
                "摩斯密码解析".to_string(),
                ConflictPolicy::Strict,
            )
            .expect_err("摩斯快捷键不能复用连发器触发键");

        assert!(error.contains("与连发器的触发键冲突"));
    }

    #[test]
    fn replace_hold_scope_rejects_existing_normal_binding_from_other_scope() {
        let manager = test_manager();
        let action: HotkeyAction = Arc::new(|_| {});
        manager
            .replace_scope(
                "morse",
                vec![("Ctrl+F2".to_string(), action)],
                "摩斯密码解析".to_string(),
                ConflictPolicy::Strict,
            )
            .expect("应注册摩斯快捷键");

        let callback: HoldActionCallback = Arc::new(|_, _| {});
        let error = manager
            .replace_hold_scope(
                "rapidfire",
                vec![("Ctrl+F2".to_string(), callback)],
                "连发器".to_string(),
                ConflictPolicy::AllowHold,
            )
            .expect_err("连发器触发键不能复用其他工具快捷键");

        assert!(error.contains("与摩斯密码解析的快捷键冲突"));
    }

    #[test]
    fn timer_scope_allows_existing_rapidfire_hold_binding() {
        let manager = test_manager();
        let callback: HoldActionCallback = Arc::new(|_, _| {});
        manager
            .replace_hold_scope(
                "rapidfire",
                vec![("F2".to_string(), callback)],
                "连发器".to_string(),
                ConflictPolicy::AllowHold,
            )
            .expect("应注册连发器触发键");

        let action: HotkeyAction = Arc::new(|_| {});
        manager
            .replace_scope(
                "timer",
                vec![("F2".to_string(), action)],
                "计时器".to_string(),
                ConflictPolicy::AllowHold,
            )
            .expect("计时器快捷键允许复用连发器触发键");
    }

    #[test]
    fn rapidfire_hold_scope_allows_existing_timer_binding() {
        let manager = test_manager();
        let action: HotkeyAction = Arc::new(|_| {});
        manager
            .replace_scope(
                "timer",
                vec![("F3".to_string(), action)],
                "计时器".to_string(),
                ConflictPolicy::AllowHold,
            )
            .expect("应注册计时器快捷键");

        let callback: HoldActionCallback = Arc::new(|_, _| {});
        manager
            .replace_hold_scope(
                "rapidfire",
                vec![("F3".to_string(), callback)],
                "连发器".to_string(),
                ConflictPolicy::AllowHold,
            )
            .expect("连发器触发键允许复用计时器快捷键");
    }

    #[test]
    fn replace_scope_allows_same_scope_replacement() {
        let manager = test_manager();
        let action: HotkeyAction = Arc::new(|_| {});
        manager
            .replace_scope(
                "timer",
                vec![("F2".to_string(), Arc::clone(&action))],
                "计时器".to_string(),
                ConflictPolicy::AllowHold,
            )
            .expect("首次注册应成功");

        manager
            .replace_scope(
                "timer",
                vec![("F2".to_string(), action)],
                "计时器".to_string(),
                ConflictPolicy::AllowHold,
            )
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
            display_name: "计时器".to_string(),
            conflict_policy: ConflictPolicy::AllowHold,
            action: Arc::clone(&timer_action),
        }]));
        let hold_registrations = Arc::new(Mutex::new(HashMap::from([(
            "rapidfire".to_string(),
            vec![HoldRegistration {
                scope: "rapidfire".to_string(),
                binding: HotkeyBinding::parse("F2").unwrap(),
                enabled: true,
                display_name: "连发器".to_string(),
                conflict_policy: ConflictPolicy::AllowHold,
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
    fn suppressed_willhook_event_is_detected_without_consuming_loop() {
        use willhook::event::{IsSystemKeyPress, KeyPress, KeyboardKey};

        let suppressed = Arc::new(Mutex::new(Some(Arc::new(Mutex::new(HashSet::from([0x70]))))));
        let event = keyboard_event(KeyboardKey::F1, KeyPress::Down(IsSystemKeyPress::Normal));
        let other_event = keyboard_event(KeyboardKey::F2, KeyPress::Down(IsSystemKeyPress::Normal));

        assert!(is_event_suppressed(&event, &suppressed));
        assert!(!is_event_suppressed(&other_event, &suppressed));
        let empty: Arc<Mutex<Option<Arc<Mutex<HashSet<u32>>>>>> = Arc::new(Mutex::new(None));
        assert!(!is_event_suppressed(&event, &empty));
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
                display_name: "连发器".to_string(),
                conflict_policy: ConflictPolicy::AllowHold,
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
                    display_name: "连发器".to_string(),
                    conflict_policy: ConflictPolicy::AllowHold,
                    action: Arc::clone(&modified_callback),
                },
                HoldRegistration {
                    scope: "rapidfire".to_string(),
                    binding: HotkeyBinding::parse("1").expect("should parse"),
                    enabled: true,
                    display_name: "连发器".to_string(),
                    conflict_policy: ConflictPolicy::AllowHold,
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
                    display_name: "连发器".to_string(),
                    conflict_policy: ConflictPolicy::AllowHold,
                    action: Arc::clone(&bare_callback),
                },
                HoldRegistration {
                    scope: "rapidfire".to_string(),
                    binding: HotkeyBinding::parse("Shift+1").expect("should parse"),
                    enabled: true,
                    display_name: "连发器".to_string(),
                    conflict_policy: ConflictPolicy::AllowHold,
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
                display_name: "计时器".to_string(),
                conflict_policy: ConflictPolicy::AllowHold,
                action: Arc::new(|_| {}),
            },
            HotkeyRegistration {
                scope: "timer".to_string(),
                binding: HotkeyBinding::parse("F2").expect("should parse"),
                enabled: true,
                display_name: "计时器".to_string(),
                conflict_policy: ConflictPolicy::AllowHold,
                action: Arc::new(|_| {}),
            },
            HotkeyRegistration {
                scope: "morse".to_string(),
                binding: HotkeyBinding::parse("F3").expect("should parse"),
                enabled: true,
                display_name: "摩斯密码解析".to_string(),
                conflict_policy: ConflictPolicy::Strict,
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
