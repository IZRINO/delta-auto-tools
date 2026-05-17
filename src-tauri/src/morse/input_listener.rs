use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use tauri::{AppHandle, Emitter};

use super::run_recognition_flow;
use crate::hotkey_types::{self as types, HotkeyBinding, ModifierKey, PrimaryKey};

#[cfg(target_os = "windows")]
use willhook::{
    event::{InputEvent, IsEventInjected, KeyPress, KeyboardEvent},
    hook::Hook,
};

pub struct PassiveHotkeyListener {
    paused: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    #[cfg(target_os = "windows")]
    worker: Option<JoinHandle<()>>,
    #[cfg(not(target_os = "windows"))]
    _worker: (),
}

impl PassiveHotkeyListener {
    pub fn start(app: AppHandle, hotkey: &str) -> Result<Self, String> {
        let binding = HotkeyBinding::parse(hotkey)?;
        let paused = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));

        #[cfg(target_os = "windows")]
        {
            let hook = willhook::keyboard_hook()
                .ok_or_else(|| "键盘钩子安装失败，请检查杀毒软件或系统权限设置".to_string())?;

            let paused_flag = Arc::clone(&paused);
            let stopped_flag = Arc::clone(&stopped);
            let worker = thread::Builder::new()
                .name("morse-hotkey-listener".to_string())
                .spawn(move || run_listener(app, binding, hook, paused_flag, stopped_flag))
                .map_err(|error| format!("启动热键监听线程失败: {error}"))?;

            return Ok(Self {
                paused,
                stopped,
                worker: Some(worker),
            });
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = app;
            let _ = binding;
            Err("当前仅 Windows 桌面环境支持被动热键监听".to_string())
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
    }

    pub fn stop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);

        #[cfg(target_os = "windows")]
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for PassiveHotkeyListener {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(target_os = "windows")]
fn run_listener(
    app: AppHandle,
    binding: HotkeyBinding,
    hook: Hook,
    paused: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
) {
    let mut matcher = HotkeyMatcher::new(binding);

    while !stopped.load(Ordering::SeqCst) {
        if paused.load(Ordering::SeqCst) {
            drain_pending_events(&hook, &mut matcher);
            thread::sleep(Duration::from_millis(25));
            continue;
        }

        match hook.try_recv() {
            Ok(InputEvent::Keyboard(event)) => {
                if matcher.handle_event(event) {
                    let app_handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = run_recognition_flow(&app_handle, "hotkey", true).await {
                            let _ = app_handle.emit_to("main", "morse://hotkey-error", error);
                        }
                    });
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
fn drain_pending_events(hook: &Hook, matcher: &mut HotkeyMatcher) {
    while let Ok(InputEvent::Keyboard(event)) = hook.try_recv() {
        matcher.handle_event(event);
    }
}

#[cfg(target_os = "windows")]
struct HotkeyMatcher {
    binding: HotkeyBinding,
    pressed_modifiers: HashSet<ModifierKey>,
    active_primary: Option<PrimaryKey>,
}

#[cfg(target_os = "windows")]
impl HotkeyMatcher {
    fn new(binding: HotkeyBinding) -> Self {
        Self {
            binding,
            pressed_modifiers: HashSet::new(),
            active_primary: None,
        }
    }

    fn handle_event(&mut self, event: KeyboardEvent) -> bool {
        if matches!(event.is_injected, Some(IsEventInjected::Injected)) {
            return false;
        }

        let Some(key) = event.key else {
            return false;
        };

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
            return false;
        }

        let Some(primary_key) = types::to_primary_key(key) else {
            return false;
        };

        match event.pressed {
            KeyPress::Down(_) => {
                if self.active_primary == Some(primary_key) {
                    return false;
                }
                self.active_primary = Some(primary_key);
                self.binding.primary == primary_key
                    && self.binding.modifiers == self.pressed_modifiers
            }
            KeyPress::Up(_) => {
                if self.active_primary == Some(primary_key) {
                    self.active_primary = None;
                }
                false
            }
            KeyPress::Other(_) => false,
        }
    }
}
