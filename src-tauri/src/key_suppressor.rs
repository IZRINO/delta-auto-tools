//! Windows 低级键盘钩子实现，用于拦截（吞噬）指定按键的系统事件。
//!
//! 当用户按住触发键且启用 `ignore_trigger_key` 时，物理按键的自动重复
//! 会导致前台应用持续收到该键输入。仅通过 enigo 合成 Release 无法解决，
//! 因为物理按住时 Windows 会每 ~30ms 产生自动重复 KEYDOWN。
//!
//! 本模块通过 `WH_KEYBOARD_LL` 钩子在事件到达前台应用前吞噬（return 1）
//! 被抑制的按键事件，同时通过 crossbeam channel 将事件转发给热键监听线程，
//! 使热键回调仍能正常触发。

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use std::sync::mpsc;

/// 被抑制的键盘事件，从 KeySuppressor 转发给热键监听线程
#[derive(Debug, Clone)]
pub struct SuppressedKeyboardEvent {
    pub vk_code: u32,
    #[allow(dead_code)]
    pub scan_code: u32,
    pub is_key_up: bool,
    pub is_injected: bool,
}

pub struct KeySuppressor {
    suppressed_keys: Arc<Mutex<HashSet<u32>>>,
    #[allow(dead_code)]
    event_sender: Sender<SuppressedKeyboardEvent>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    /// worker 线程 ID，用于 PostThreadMessage 唤醒
    #[cfg(target_os = "windows")]
    worker_thread_id: Option<u32>,
}

impl KeySuppressor {
    /// 创建并启动 KeySuppressor。
    ///
    /// 返回 (KeySuppressor, Receiver<SuppressedKeyboardEvent>)，
    /// Receiver 用于接收被抑制的事件并转发给热键监听线程。
    pub fn start() -> Result<(Self, Receiver<SuppressedKeyboardEvent>), String> {
        let (tx, rx) = crossbeam_channel::bounded(256);
        let suppressed_keys = Arc::new(Mutex::new(HashSet::new()));
        let stopped = Arc::new(AtomicBool::new(false));

        let worker_suppressed = Arc::clone(&suppressed_keys);
        let worker_stopped = Arc::clone(&stopped);
        let worker_tx = tx.clone();
        let (install_tx, install_rx) = mpsc::channel();

        #[cfg(target_os = "windows")]
        let (tid_tx, tid_rx) = mpsc::channel();

        let worker = thread::Builder::new()
            .name("key-suppressor".to_string())
            .spawn(move || {
                #[cfg(target_os = "windows")]
                {
                    // Windows 线程 ID 来自 GetCurrentThreadId
                    let win_tid =
                        unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
                    let _ = tid_tx.send(win_tid);
                }
                let _ = (); // suppress unused variable warning on non-windows
                run_suppressor_hook(worker_suppressed, worker_stopped, worker_tx, install_tx);
            })
            .map_err(|e| format!("启动按键抑制线程失败: {e}"))?;

        #[cfg(target_os = "windows")]
        let worker_thread_id = tid_rx.recv_timeout(Duration::from_secs(2)).ok();
        #[cfg(not(target_os = "windows"))]
        let worker_thread_id: Option<u32> = None;

        match install_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                stopped.store(true, Ordering::SeqCst);
                let _ = worker.join();
                return Err(error);
            }
            Err(_) => {
                stopped.store(true, Ordering::SeqCst);
                let _ = worker.join();
                return Err("按键抑制钩子安装超时".to_string());
            }
        }

        Ok((
            Self {
                suppressed_keys,
                event_sender: tx,
                stopped,
                worker: Some(worker),
                worker_thread_id,
            },
            rx,
        ))
    }

    /// 添加一个按键到抑制列表。返回该键之前是否未被抑制。
    pub fn suppress(&self, vk_code: u32) -> bool {
        if let Ok(mut keys) = self.suppressed_keys.lock() {
            keys.insert(vk_code)
        } else {
            false
        }
    }

    /// 从抑制列表移除一个按键。返回该键之前是否被抑制。
    pub fn unsuppress(&self, vk_code: u32) -> bool {
        if let Ok(mut keys) = self.suppressed_keys.lock() {
            keys.remove(&vk_code)
        } else {
            false
        }
    }

    /// 查询指定按键当前是否在抑制列表中
    #[allow(dead_code)]
    pub fn is_suppressing(&self, vk_code: u32) -> bool {
        self.suppressed_keys
            .lock()
            .map(|keys| keys.contains(&vk_code))
            .unwrap_or(false)
    }

    /// 返回抑制键集合的共享引用，供热键监听线程过滤 willhook 重复事件
    pub fn suppressed_keys_ref(&self) -> Arc<Mutex<HashSet<u32>>> {
        Arc::clone(&self.suppressed_keys)
    }

    /// 取消所有抑制
    pub fn clear_all(&self) {
        if let Ok(mut keys) = self.suppressed_keys.lock() {
            keys.clear();
        }
    }
}

impl Drop for KeySuppressor {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        // 通过 PostThreadMessage 唤醒 GetMessageW 阻塞的 worker 线程
        #[cfg(target_os = "windows")]
        if let Some(tid) = self.worker_thread_id {
            use windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW;
            const WM_USER_SHUTDOWN: u32 = 0x0400 + 1; // WM_USER + 1
            unsafe {
                PostThreadMessageW(tid, WM_USER_SHUTDOWN, 0, 0);
            }
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// 将热键字符串（如 "E"、"F1"、"Shift+-"）中的主键解析为 Windows VK code。
///
/// 注意：只解析主键，忽略修饰键。因为抑制是针对物理主键的。
pub fn hotkey_primary_to_vk(raw: &str) -> Option<u32> {
    use crate::hotkey_types::HotkeyBinding;
    let binding = HotkeyBinding::parse(raw).ok()?;
    primary_key_to_vk(binding.primary)
}

/// 将 PrimaryKey 映射为 Windows VK code
pub fn primary_key_to_vk(primary: crate::hotkey_types::PrimaryKey) -> Option<u32> {
    use crate::hotkey_types::{NamedKey, PrimaryKey};
    match primary {
        PrimaryKey::Letter(c) => Some(c as u32),
        PrimaryKey::Digit(c) => Some(c as u32),
        PrimaryKey::Function(n) => Some(0x70 + (n - 1) as u32), // VK_F1=0x70
        PrimaryKey::Named(NamedKey::Space) => Some(0x20),
        PrimaryKey::Named(NamedKey::Enter) => Some(0x0D),
        PrimaryKey::Named(NamedKey::Tab) => Some(0x09),
        PrimaryKey::Named(NamedKey::Esc) => Some(0x1B),
        PrimaryKey::Named(NamedKey::Up) => Some(0x26),
        PrimaryKey::Named(NamedKey::Down) => Some(0x28),
        PrimaryKey::Named(NamedKey::Left) => Some(0x25),
        PrimaryKey::Named(NamedKey::Right) => Some(0x27),
        PrimaryKey::Named(NamedKey::Home) => Some(0x24),
        PrimaryKey::Named(NamedKey::End) => Some(0x23),
        PrimaryKey::Named(NamedKey::PageUp) => Some(0x21),
        PrimaryKey::Named(NamedKey::PageDown) => Some(0x22),
        PrimaryKey::Named(NamedKey::Insert) => Some(0x2D),
        PrimaryKey::Named(NamedKey::Delete) => Some(0x2E),
        PrimaryKey::Named(NamedKey::Backspace) => Some(0x08),
        PrimaryKey::Named(NamedKey::Alt) => Some(0x12),
        PrimaryKey::Named(NamedKey::Semicolon) => Some(0xBA),
        PrimaryKey::Named(NamedKey::Equal) => Some(0xBB),
        PrimaryKey::Named(NamedKey::Plus) => Some(0x6B),
        PrimaryKey::Named(NamedKey::Comma) => Some(0xBC),
        PrimaryKey::Named(NamedKey::Minus) => Some(0xBD),
        PrimaryKey::Named(NamedKey::Period) => Some(0xBE),
        PrimaryKey::Named(NamedKey::Slash) => Some(0xBF),
        PrimaryKey::Named(NamedKey::Backquote) => Some(0xC0),
        PrimaryKey::Named(NamedKey::BracketLeft) => Some(0xDB),
        PrimaryKey::Named(NamedKey::Backslash) => Some(0xDC),
        PrimaryKey::Named(NamedKey::BracketRight) => Some(0xDD),
        PrimaryKey::Named(NamedKey::Quote) => Some(0xDE),
    }
}

/// 将 willhook 的 KeyboardKey 映射回 Windows VK code（vk_to_keyboard_key 的逆向映射）
pub fn keyboard_key_to_vk(key: &willhook::event::KeyboardKey) -> Option<u32> {
    use willhook::event::KeyboardKey;
    match key {
        KeyboardKey::BackSpace => Some(0x08),
        KeyboardKey::Tab => Some(0x09),
        KeyboardKey::Enter => Some(0x0D),
        KeyboardKey::Escape => Some(0x1B),
        KeyboardKey::Space => Some(0x20),
        KeyboardKey::PageUp => Some(0x21),
        KeyboardKey::PageDown => Some(0x22),
        KeyboardKey::Home => Some(0x24),
        KeyboardKey::Insert => Some(0x2D),
        KeyboardKey::Delete => Some(0x2E),
        KeyboardKey::ArrowLeft => Some(0x25),
        KeyboardKey::ArrowUp => Some(0x26),
        KeyboardKey::ArrowRight => Some(0x27),
        KeyboardKey::ArrowDown => Some(0x28),
        KeyboardKey::Number0 => Some(0x30),
        KeyboardKey::Number1 => Some(0x31),
        KeyboardKey::Number2 => Some(0x32),
        KeyboardKey::Number3 => Some(0x33),
        KeyboardKey::Number4 => Some(0x34),
        KeyboardKey::Number5 => Some(0x35),
        KeyboardKey::Number6 => Some(0x36),
        KeyboardKey::Number7 => Some(0x37),
        KeyboardKey::Number8 => Some(0x38),
        KeyboardKey::Number9 => Some(0x39),
        KeyboardKey::A => Some(0x41),
        KeyboardKey::B => Some(0x42),
        KeyboardKey::C => Some(0x43),
        KeyboardKey::D => Some(0x44),
        KeyboardKey::E => Some(0x45),
        KeyboardKey::F => Some(0x46),
        KeyboardKey::G => Some(0x47),
        KeyboardKey::H => Some(0x48),
        KeyboardKey::I => Some(0x49),
        KeyboardKey::J => Some(0x4A),
        KeyboardKey::K => Some(0x4B),
        KeyboardKey::L => Some(0x4C),
        KeyboardKey::M => Some(0x4D),
        KeyboardKey::N => Some(0x4E),
        KeyboardKey::O => Some(0x4F),
        KeyboardKey::P => Some(0x50),
        KeyboardKey::Q => Some(0x51),
        KeyboardKey::R => Some(0x52),
        KeyboardKey::S => Some(0x53),
        KeyboardKey::T => Some(0x54),
        KeyboardKey::U => Some(0x55),
        KeyboardKey::V => Some(0x56),
        KeyboardKey::W => Some(0x57),
        KeyboardKey::X => Some(0x58),
        KeyboardKey::Y => Some(0x59),
        KeyboardKey::Z => Some(0x5A),
        KeyboardKey::F1 => Some(0x70),
        KeyboardKey::F2 => Some(0x71),
        KeyboardKey::F3 => Some(0x72),
        KeyboardKey::F4 => Some(0x73),
        KeyboardKey::F5 => Some(0x74),
        KeyboardKey::F6 => Some(0x75),
        KeyboardKey::F7 => Some(0x76),
        KeyboardKey::F8 => Some(0x77),
        KeyboardKey::F9 => Some(0x78),
        KeyboardKey::F10 => Some(0x79),
        KeyboardKey::F11 => Some(0x7A),
        KeyboardKey::F12 => Some(0x7B),
        KeyboardKey::LeftAlt => Some(0x12),
        KeyboardKey::LeftShift => Some(0xA0),
        KeyboardKey::RightShift => Some(0xA1),
        KeyboardKey::LeftControl => Some(0xA2),
        KeyboardKey::RightControl => Some(0xA3),
        KeyboardKey::LeftWindows => Some(0x5B),
        KeyboardKey::RightWindows => Some(0x5C),
        KeyboardKey::Other(vk) => Some(*vk),
        _ => None,
    }
}

/// 将 VK code 映射回 willhook 的 KeyboardKey
pub fn vk_to_keyboard_key(vk_code: u32) -> willhook::event::KeyboardKey {
    use willhook::event::KeyboardKey;
    match vk_code {
        0x08 => KeyboardKey::BackSpace,
        0x09 => KeyboardKey::Tab,
        0x0D => KeyboardKey::Enter,
        0x1B => KeyboardKey::Escape,
        0x20 => KeyboardKey::Space,
        0x21 => KeyboardKey::PageUp,
        0x22 => KeyboardKey::PageDown,
        0x23 => KeyboardKey::Other(0x23), // End: willhook 无 End 变体
        0x24 => KeyboardKey::Home,
        0x25 => KeyboardKey::ArrowLeft,
        0x26 => KeyboardKey::ArrowUp,
        0x27 => KeyboardKey::ArrowRight,
        0x28 => KeyboardKey::ArrowDown,
        0x2D => KeyboardKey::Insert,
        0x2E => KeyboardKey::Delete,
        0x30 => KeyboardKey::Number0,
        0x31 => KeyboardKey::Number1,
        0x32 => KeyboardKey::Number2,
        0x33 => KeyboardKey::Number3,
        0x34 => KeyboardKey::Number4,
        0x35 => KeyboardKey::Number5,
        0x36 => KeyboardKey::Number6,
        0x37 => KeyboardKey::Number7,
        0x38 => KeyboardKey::Number8,
        0x39 => KeyboardKey::Number9,
        0x41 => KeyboardKey::A,
        0x42 => KeyboardKey::B,
        0x43 => KeyboardKey::C,
        0x44 => KeyboardKey::D,
        0x45 => KeyboardKey::E,
        0x46 => KeyboardKey::F,
        0x47 => KeyboardKey::G,
        0x48 => KeyboardKey::H,
        0x49 => KeyboardKey::I,
        0x4A => KeyboardKey::J,
        0x4B => KeyboardKey::K,
        0x4C => KeyboardKey::L,
        0x4D => KeyboardKey::M,
        0x4E => KeyboardKey::N,
        0x4F => KeyboardKey::O,
        0x50 => KeyboardKey::P,
        0x51 => KeyboardKey::Q,
        0x52 => KeyboardKey::R,
        0x53 => KeyboardKey::S,
        0x54 => KeyboardKey::T,
        0x55 => KeyboardKey::U,
        0x56 => KeyboardKey::V,
        0x57 => KeyboardKey::W,
        0x58 => KeyboardKey::X,
        0x59 => KeyboardKey::Y,
        0x5A => KeyboardKey::Z,
        0x70 => KeyboardKey::F1,
        0x71 => KeyboardKey::F2,
        0x72 => KeyboardKey::F3,
        0x73 => KeyboardKey::F4,
        0x74 => KeyboardKey::F5,
        0x75 => KeyboardKey::F6,
        0x76 => KeyboardKey::F7,
        0x77 => KeyboardKey::F8,
        0x78 => KeyboardKey::F9,
        0x79 => KeyboardKey::F10,
        0x7A => KeyboardKey::F11,
        0x7B => KeyboardKey::F12,
        0x12 => KeyboardKey::LeftAlt,
        0xA0 => KeyboardKey::LeftShift,
        0xA1 => KeyboardKey::RightShift,
        0xA2 => KeyboardKey::LeftControl,
        0xA3 => KeyboardKey::RightControl,
        0x5B => KeyboardKey::LeftWindows,
        0x5C => KeyboardKey::RightWindows,
        // OEM keys - use Other variant
        0xBA..=0xDE => KeyboardKey::Other(vk_code),
        _ => KeyboardKey::Other(vk_code),
    }
}

/// 将 SuppressedKeyboardEvent 转换为 willhook 的 KeyboardEvent
pub fn suppressed_event_to_willhook_event(
    event: &SuppressedKeyboardEvent,
) -> willhook::event::KeyboardEvent {
    use willhook::event::{IsEventInjected, IsSystemKeyPress, KeyPress};

    let pressed = if event.is_key_up {
        KeyPress::Up(IsSystemKeyPress::Normal)
    } else {
        KeyPress::Down(IsSystemKeyPress::Normal)
    };

    let key = Some(vk_to_keyboard_key(event.vk_code));

    let is_injected = if event.is_injected {
        Some(IsEventInjected::Injected)
    } else {
        Some(IsEventInjected::NotInjected)
    };

    willhook::event::KeyboardEvent {
        pressed,
        key,
        is_injected,
    }
}

// ── Windows WH_KEYBOARD_LL 钩子实现 ──

#[cfg(target_os = "windows")]
fn run_suppressor_hook(
    suppressed_keys: Arc<Mutex<HashSet<u32>>>,
    stopped: Arc<AtomicBool>,
    event_sender: Sender<SuppressedKeyboardEvent>,
    install_sender: mpsc::Sender<Result<(), String>>,
) {
    use std::ptr;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, MSG, WH_KEYBOARD_LL,
        WM_KEYUP, WM_SYSKEYUP,
    };

    // KBDLLHOOKSTRUCT 不在 windows-sys 的默认 feature 中，手动定义
    #[repr(C)]
    struct Kbdllhookstruct {
        vk_code: u32,
        scan_code: u32,
        flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    // LLKHF_INJECTED = 0x10
    const LLKHF_INJECTED: u32 = 0x10;

    // 自定义退出消息，对应 Drop 中的 PostThreadMessage
    const WM_USER_SHUTDOWN: u32 = 0x0400 + 1;

    // 全局共享状态：使用全局静态变量让钩子回调能访问
    // 由于 WH_KEYBOARD_LL 钩子回调必须是 extern "system" fn，不能直接捕获环境
    static SUPPRESSED_KEYS: std::sync::OnceLock<Arc<Mutex<HashSet<u32>>>> =
        std::sync::OnceLock::new();
    static EVENT_SENDER: std::sync::OnceLock<Sender<SuppressedKeyboardEvent>> =
        std::sync::OnceLock::new();

    SUPPRESSED_KEYS.get_or_init(|| suppressed_keys);
    EVENT_SENDER.get_or_init(|| event_sender);

    unsafe extern "system" fn hook_callback(code: i32, w_param: usize, l_param: isize) -> isize {
        if code < 0 {
            return CallNextHookEx(ptr::null_mut(), code, w_param, l_param);
        }

        let kb = &*(l_param as *const Kbdllhookstruct);
        let vk_code = kb.vk_code;
        let is_key_up = (w_param == WM_KEYUP as usize) || (w_param == WM_SYSKEYUP as usize);
        let is_injected = (kb.flags & LLKHF_INJECTED) != 0;

        // enigo 合成事件不抑制（放行给系统）
        if is_injected {
            return CallNextHookEx(ptr::null_mut(), code, w_param, l_param);
        }

        // 检查该键是否在抑制列表中
        let should_suppress = SUPPRESSED_KEYS
            .get()
            .and_then(|keys| keys.lock().map(|k| k.contains(&vk_code)).ok())
            .unwrap_or(false);

        if should_suppress {
            // 吞噬事件：return 1 阻止事件传递到前台应用
            // 同时转发给热键监听线程，使热键回调仍能触发
            let _ = EVENT_SENDER.get().map(|tx| {
                tx.send(SuppressedKeyboardEvent {
                    vk_code,
                    scan_code: kb.scan_code,
                    is_key_up,
                    is_injected: false,
                })
            });
            return 1;
        }

        CallNextHookEx(ptr::null_mut(), code, w_param, l_param)
    }

    // 安装钩子
    let hook_handle = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL as i32,
            Some(hook_callback),
            ptr::null_mut(),
            0,
        )
    };

    if hook_handle.is_null() {
        let error_code = unsafe { GetLastError() };
        let _ = install_sender.send(Err(format!(
            "安装按键抑制钩子失败，系统错误码: {error_code}"
        )));
        return;
    }
    let _ = install_sender.send(Ok(()));

    // 消息循环：使用 GetMessageW 阻塞等待，零延迟响应键盘事件。
    // 退出时通过 PostThreadMessage(WM_USER_SHUTDOWN) 唤醒线程。
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        let ret = unsafe { GetMessageW(&mut msg, ptr::null_mut(), 0, 0) };
        if ret == 0 || msg.message == WM_USER_SHUTDOWN {
            // WM_QUIT 或自定义退出消息
            break;
        }
        // TranslateMessage/DispatchMessage 对 WH_KEYBOARD_LL 钩子不是必须的，
        // 钩子由系统在消息处理前直接调用。但消息循环需要 GetMessageW 来维持钩子链。
        // stopped 标志作为额外安全网：如果 PostThreadMessage 丢失，线程仍能退出
        if stopped.load(Ordering::SeqCst) {
            break;
        }
    }

    // 卸载钩子
    unsafe {
        UnhookWindowsHookEx(hook_handle);
    }
}

#[cfg(not(target_os = "windows"))]
fn run_suppressor_hook(
    _suppressed_keys: Arc<Mutex<HashSet<u32>>>,
    _stopped: Arc<AtomicBool>,
    _event_sender: Sender<SuppressedKeyboardEvent>,
    install_sender: mpsc::Sender<Result<(), String>>,
) {
    let _ = install_sender.send(Err("当前仅 Windows 支持按键抑制".to_string()));
    // 非 Windows 平台不做任何操作
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use willhook::event::KeyboardKey;

    #[test]
    fn keyboard_key_vk_roundtrip_supports_common_keys() {
        let cases = [
            (KeyboardKey::A, 0x41),
            (KeyboardKey::Number1, 0x31),
            (KeyboardKey::F1, 0x70),
            (KeyboardKey::Space, 0x20),
            (KeyboardKey::Other(0xBD), 0xBD),
        ];

        for (key, vk) in cases {
            assert_eq!(keyboard_key_to_vk(&key), Some(vk));
            assert_eq!(keyboard_key_to_vk(&vk_to_keyboard_key(vk)), Some(vk));
        }
    }
}
