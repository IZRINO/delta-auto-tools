//! Windows 低级键盘钩子实现，用于拦截（吞噬）指定按键的系统事件。
//!
//! 当用户按住触发键且启用 `ignore_trigger_key` 时，物理按键的自动重复
//! 会导致前台应用持续收到该键输入。仅通过 enigo 合成 Release 无法解决，
//! 因为物理按住时 Windows 会每 ~30ms 产生自动重复 KEYDOWN。
//!
//! 本模块通过 `WH_KEYBOARD_LL` 钩子在事件到达前台应用前吞噬（return 1）
//! 被抑制的按键事件，同时通过 crossbeam channel 将事件转发给热键监听线程，
//! 使热键回调仍能正常触发。

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
#[cfg(target_os = "windows")]
use std::sync::{OnceLock, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TrySendError};
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

pub struct VkBitset {
    words: [AtomicU64; 4],
}

impl Default for VkBitset {
    fn default() -> Self {
        Self {
            words: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

impl VkBitset {
    pub fn insert(&self, vk_code: u32) -> bool {
        let Some((word, mask)) = Self::word_and_mask(vk_code) else {
            return false;
        };
        self.words[word].fetch_or(mask, Ordering::Relaxed) & mask == 0
    }

    pub fn remove(&self, vk_code: u32) -> bool {
        let Some((word, mask)) = Self::word_and_mask(vk_code) else {
            return false;
        };
        self.words[word].fetch_and(!mask, Ordering::Relaxed) & mask != 0
    }

    pub fn contains(&self, vk_code: u32) -> bool {
        let Some((word, mask)) = Self::word_and_mask(vk_code) else {
            return false;
        };
        self.words[word].load(Ordering::Relaxed) & mask != 0
    }

    pub fn clear(&self) {
        for word in &self.words {
            word.store(0, Ordering::Relaxed);
        }
    }

    fn word_and_mask(vk_code: u32) -> Option<(usize, u64)> {
        (vk_code <= 255).then(|| ((vk_code / 64) as usize, 1_u64 << (vk_code % 64)))
    }
}

fn try_forward_suppressed_event(
    event_sender: &Sender<SuppressedKeyboardEvent>,
    event: SuppressedKeyboardEvent,
    dropped_events: &AtomicU64,
) -> Result<(), TrySendError<SuppressedKeyboardEvent>> {
    match event_sender.try_send(event) {
        Err(TrySendError::Full(event)) => {
            dropped_events.fetch_add(1, Ordering::Relaxed);
            Err(TrySendError::Full(event))
        }
        result => result,
    }
}

fn prepare_worker_thread(
    stopped: &AtomicBool,
    worker_thread_id: &AtomicU32,
    thread_id: u32,
) -> bool {
    worker_thread_id.store(thread_id, Ordering::SeqCst);
    !stopped.load(Ordering::SeqCst)
}

fn request_worker_stop(stopped: &AtomicBool, worker_thread_id: &AtomicU32) -> Option<u32> {
    stopped.store(true, Ordering::SeqCst);
    let thread_id = worker_thread_id.load(Ordering::SeqCst);
    (thread_id != 0).then_some(thread_id)
}

#[cfg(target_os = "windows")]
fn wake_worker_thread(thread_id: u32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW;
    const WM_USER_SHUTDOWN: u32 = 0x0400 + 1;
    unsafe {
        PostThreadMessageW(thread_id, WM_USER_SHUTDOWN, 0, 0);
    }
}

fn stop_worker(stopped: &AtomicBool, worker_thread_id: &AtomicU32) {
    if let Some(thread_id) = request_worker_stop(stopped, worker_thread_id) {
        #[cfg(target_os = "windows")]
        wake_worker_thread(thread_id);
        #[cfg(not(target_os = "windows"))]
        let _ = thread_id;
    }
}

fn stop_and_join_worker(
    stopped: &AtomicBool,
    worker_thread_id: &AtomicU32,
    worker: JoinHandle<()>,
) {
    stop_worker(stopped, worker_thread_id);
    let _ = worker.join();
}

static ACTIVE_HOOK_WORKER: AtomicBool = AtomicBool::new(false);

struct ActiveHookGuard<'a> {
    active: &'a AtomicBool,
}

impl<'a> ActiveHookGuard<'a> {
    fn acquire(active: &'a AtomicBool) -> Result<Self, ()> {
        active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self { active })
            .map_err(|_| ())
    }
}

impl Drop for ActiveHookGuard<'_> {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallWaitAction {
    Retain,
    Join,
    Detach,
}

fn install_wait_action(
    result: &Result<Result<(), String>, mpsc::RecvTimeoutError>,
) -> InstallWaitAction {
    match result {
        Ok(Ok(())) => InstallWaitAction::Retain,
        Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => InstallWaitAction::Join,
        Err(mpsc::RecvTimeoutError::Timeout) => InstallWaitAction::Detach,
    }
}

#[cfg(target_os = "windows")]
struct CallbackContext {
    suppressed_keys: Arc<VkBitset>,
    event_sender: Sender<SuppressedKeyboardEvent>,
    dropped_events: Arc<AtomicU64>,
}

#[cfg(target_os = "windows")]
type CallbackContextSlot = RwLock<Option<Arc<CallbackContext>>>;

#[cfg(target_os = "windows")]
static CALLBACK_CONTEXT: OnceLock<CallbackContextSlot> = OnceLock::new();

#[cfg(target_os = "windows")]
fn callback_context_slot() -> &'static CallbackContextSlot {
    CALLBACK_CONTEXT.get_or_init(|| RwLock::new(None))
}

#[cfg(target_os = "windows")]
fn replace_callback_context(slot: &CallbackContextSlot, context: Arc<CallbackContext>) {
    let mut current = slot.write().unwrap_or_else(|error| error.into_inner());
    *current = Some(context);
}

#[cfg(target_os = "windows")]
fn clear_callback_context(slot: &CallbackContextSlot, expected: &Arc<CallbackContext>) {
    let mut current = slot.write().unwrap_or_else(|error| error.into_inner());
    if current
        .as_ref()
        .is_some_and(|context| Arc::ptr_eq(context, expected))
    {
        *current = None;
    }
}

#[cfg(target_os = "windows")]
fn try_forward_from_callback_context(
    slot: &CallbackContextSlot,
    event: SuppressedKeyboardEvent,
) -> bool {
    let Ok(current) = slot.try_read() else {
        return false;
    };
    let Some(context) = current.as_ref() else {
        return false;
    };
    if !context.suppressed_keys.contains(event.vk_code) {
        return false;
    }

    let _ = try_forward_suppressed_event(
        &context.event_sender,
        event,
        context.dropped_events.as_ref(),
    );
    true
}

pub struct KeySuppressor {
    suppressed_keys: Arc<VkBitset>,
    #[allow(dead_code)]
    event_sender: Sender<SuppressedKeyboardEvent>,
    #[allow(dead_code)]
    dropped_events: Arc<AtomicU64>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    /// worker 线程 ID，用于 PostThreadMessage 唤醒
    worker_thread_id: Arc<AtomicU32>,
}

impl KeySuppressor {
    /// 创建并启动 KeySuppressor。
    ///
    /// 返回 (KeySuppressor, Receiver<SuppressedKeyboardEvent>)，
    /// Receiver 用于接收被抑制的事件并转发给热键监听线程。
    pub fn start() -> Result<(Self, Receiver<SuppressedKeyboardEvent>), String> {
        let active_hook = ActiveHookGuard::acquire(&ACTIVE_HOOK_WORKER)
            .map_err(|_| "按键抑制钩子仍在清理中，请稍后重试".to_string())?;
        let (tx, rx) = crossbeam_channel::bounded(256);
        let suppressed_keys = Arc::new(VkBitset::default());
        let dropped_events = Arc::new(AtomicU64::new(0));
        let stopped = Arc::new(AtomicBool::new(false));

        let worker_suppressed = Arc::clone(&suppressed_keys);
        let worker_stopped = Arc::clone(&stopped);
        let worker_tx = tx.clone();
        let worker_dropped_events = Arc::clone(&dropped_events);
        let worker_thread_id = Arc::new(AtomicU32::new(0));
        let worker_id = Arc::clone(&worker_thread_id);
        let (install_tx, install_rx) = mpsc::channel();

        let worker = thread::Builder::new()
            .name("key-suppressor".to_string())
            .spawn(move || {
                let _active_hook = active_hook;
                #[cfg(target_os = "windows")]
                {
                    use std::ptr;
                    use windows_sys::Win32::UI::WindowsAndMessaging::{
                        PeekMessageW, MSG, PM_NOREMOVE,
                    };

                    // 先创建消息队列，再发布 thread ID，确保后续 PostThreadMessage 可唤醒。
                    let mut msg: MSG = unsafe { std::mem::zeroed() };
                    unsafe {
                        PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_NOREMOVE);
                    }
                    let thread_id =
                        unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
                    if !prepare_worker_thread(&worker_stopped, &worker_id, thread_id) {
                        return;
                    }
                }
                let _ = (); // suppress unused variable warning on non-windows
                run_suppressor_hook(
                    worker_suppressed,
                    worker_stopped,
                    worker_tx,
                    worker_dropped_events,
                    install_tx,
                );
            })
            .map_err(|e| format!("启动按键抑制线程失败: {e}"))?;

        let install_result = install_rx.recv_timeout(Duration::from_secs(2));
        match install_wait_action(&install_result) {
            InstallWaitAction::Retain => {}
            InstallWaitAction::Join => {
                stop_and_join_worker(stopped.as_ref(), worker_thread_id.as_ref(), worker);
                return Err(match install_result {
                    Ok(Err(error)) => error,
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        "按键抑制钩子安装线程异常退出".to_string()
                    }
                    _ => unreachable!(),
                });
            }
            InstallWaitAction::Detach => {
                stop_worker(stopped.as_ref(), worker_thread_id.as_ref());
                drop(worker);
                return Err("按键抑制钩子安装超时".to_string());
            }
        }

        Ok((
            Self {
                suppressed_keys,
                event_sender: tx,
                dropped_events,
                stopped,
                worker: Some(worker),
                worker_thread_id,
            },
            rx,
        ))
    }

    /// 添加一个按键到抑制列表。返回该键之前是否未被抑制。
    pub fn suppress(&self, vk_code: u32) -> bool {
        self.suppressed_keys.insert(vk_code)
    }

    /// 从抑制列表移除一个按键。返回该键之前是否被抑制。
    pub fn unsuppress(&self, vk_code: u32) -> bool {
        self.suppressed_keys.remove(vk_code)
    }

    /// 查询指定按键当前是否在抑制列表中
    #[allow(dead_code)]
    pub fn is_suppressing(&self, vk_code: u32) -> bool {
        self.suppressed_keys.contains(vk_code)
    }

    /// 返回抑制键集合的共享引用，供热键监听线程过滤 willhook 重复事件
    pub fn suppressed_keys_ref(&self) -> Arc<VkBitset> {
        Arc::clone(&self.suppressed_keys)
    }

    /// 取消所有抑制
    pub fn clear_all(&self) {
        self.suppressed_keys.clear();
    }
}

impl Drop for KeySuppressor {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            stop_and_join_worker(
                self.stopped.as_ref(),
                self.worker_thread_id.as_ref(),
                worker,
            );
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
        KeyboardKey::SemiColon => Some(0xBA),
        KeyboardKey::Comma => Some(0xBC),
        KeyboardKey::Period => Some(0xBE),
        KeyboardKey::Slash => Some(0xBF),
        KeyboardKey::Grave => Some(0xC0),
        KeyboardKey::LeftBrace => Some(0xDB),
        KeyboardKey::BackwardSlash => Some(0xDC),
        KeyboardKey::RightBrace => Some(0xDD),
        KeyboardKey::Apostrophe => Some(0xDE),
        KeyboardKey::Add => Some(0x6B),
        KeyboardKey::Subtract => Some(0x6D),
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
    suppressed_keys: Arc<VkBitset>,
    stopped: Arc<AtomicBool>,
    event_sender: Sender<SuppressedKeyboardEvent>,
    dropped_events: Arc<AtomicU64>,
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

    // extern callback 通过可替换 slot 读取当前 hook context。
    // callback 仅 try_read；生命周期写入可阻塞，但不会发生在 callback 中。
    let callback_context = Arc::new(CallbackContext {
        suppressed_keys,
        event_sender,
        dropped_events,
    });
    replace_callback_context(callback_context_slot(), Arc::clone(&callback_context));

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

        if try_forward_from_callback_context(
            callback_context_slot(),
            SuppressedKeyboardEvent {
                vk_code,
                scan_code: kb.scan_code,
                is_key_up,
                is_injected: false,
            },
        ) {
            // 吞噬事件：return 1 阻止事件传递到前台应用
            return 1;
        }

        CallNextHookEx(ptr::null_mut(), code, w_param, l_param)
    }

    // 安装钩子
    let hook_handle =
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_callback), ptr::null_mut(), 0) };

    if hook_handle.is_null() {
        let error_code = unsafe { GetLastError() };
        clear_callback_context(callback_context_slot(), &callback_context);
        let _ = install_sender.send(Err(format!(
            "安装按键抑制钩子失败，系统错误码: {error_code}"
        )));
        return;
    }

    if stopped.load(Ordering::SeqCst) {
        unsafe {
            UnhookWindowsHookEx(hook_handle);
        }
        clear_callback_context(callback_context_slot(), &callback_context);
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
    clear_callback_context(callback_context_slot(), &callback_context);
}

#[cfg(not(target_os = "windows"))]
fn run_suppressor_hook(
    _suppressed_keys: Arc<VkBitset>,
    _stopped: Arc<AtomicBool>,
    _event_sender: Sender<SuppressedKeyboardEvent>,
    _dropped_events: Arc<AtomicU64>,
    install_sender: mpsc::Sender<Result<(), String>>,
) {
    let _ = install_sender.send(Err("当前仅 Windows 支持按键抑制".to_string()));
    // 非 Windows 平台不做任何操作
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crossbeam_channel::TrySendError;
    use willhook::event::KeyboardKey;

    fn suppressed_event(vk_code: u32) -> SuppressedKeyboardEvent {
        SuppressedKeyboardEvent {
            vk_code,
            scan_code: 0,
            is_key_up: false,
            is_injected: false,
        }
    }

    #[test]
    fn worker_stopped_before_install_does_not_start_hook() {
        let stopped = AtomicBool::new(true);
        let worker_thread_id = std::sync::atomic::AtomicU32::new(0);

        assert!(!prepare_worker_thread(&stopped, &worker_thread_id, 42));
        assert_eq!(worker_thread_id.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn stopping_worker_with_known_thread_id_requests_wake() {
        let stopped = AtomicBool::new(false);
        let worker_thread_id = std::sync::atomic::AtomicU32::new(42);

        assert_eq!(request_worker_stop(&stopped, &worker_thread_id), Some(42));
        assert!(stopped.load(Ordering::SeqCst));
    }

    #[test]
    fn worker_starting_after_unknown_id_stop_exits_before_install() {
        let stopped = AtomicBool::new(false);
        let worker_thread_id = std::sync::atomic::AtomicU32::new(0);

        assert_eq!(request_worker_stop(&stopped, &worker_thread_id), None);
        assert!(!prepare_worker_thread(&stopped, &worker_thread_id, 42));
        assert_eq!(worker_thread_id.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn active_hook_slot_rejects_retry_until_worker_exits() {
        let active = AtomicBool::new(false);
        let first = ActiveHookGuard::acquire(&active).expect("首个 worker 应占用 hook slot");

        assert!(ActiveHookGuard::acquire(&active).is_err());

        drop(first);
        assert!(ActiveHookGuard::acquire(&active).is_ok());
    }

    #[test]
    fn install_timeout_detaches_worker_without_joining() {
        assert_eq!(
            install_wait_action(&Err(mpsc::RecvTimeoutError::Timeout)),
            InstallWaitAction::Detach,
        );
    }

    #[test]
    fn disconnected_install_channel_joins_finished_worker() {
        assert_eq!(
            install_wait_action(&Err(mpsc::RecvTimeoutError::Disconnected)),
            InstallWaitAction::Join,
        );
    }

    #[test]
    fn successful_install_retains_worker() {
        assert_eq!(install_wait_action(&Ok(Ok(()))), InstallWaitAction::Retain,);
    }

    #[test]
    fn vk_bitset_handles_boundaries_and_rejects_out_of_range_codes() {
        let keys = VkBitset::default();

        for vk in [0, 63, 64, 255] {
            assert!(keys.insert(vk));
            assert!(keys.contains(vk));
        }
        assert!(!keys.insert(256));
        assert!(!keys.contains(256));
    }

    #[test]
    fn vk_bitset_insert_remove_are_idempotent_and_clear_resets_all_words() {
        let keys = VkBitset::default();

        assert!(keys.insert(63));
        assert!(!keys.insert(63));
        assert!(keys.remove(63));
        assert!(!keys.remove(63));

        for vk in [0, 64, 255] {
            assert!(keys.insert(vk));
        }
        keys.clear();
        for vk in [0, 64, 255] {
            assert!(!keys.contains(vk));
        }
    }

    #[test]
    fn forwarding_full_channel_increments_dropped_counter_without_blocking() {
        let (tx, _rx) = crossbeam_channel::bounded(1);
        tx.send(suppressed_event(0x70)).expect("应先填满有界队列");
        let dropped_events = AtomicU64::new(0);

        let result = try_forward_suppressed_event(&tx, suppressed_event(0x71), &dropped_events);

        assert!(matches!(result, Err(TrySendError::Full(_))));
        assert_eq!(dropped_events.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn callback_context_replacement_and_cleanup_never_reuses_previous_context() {
        let slot = std::sync::RwLock::new(None);
        let (sender_a, receiver_a) = crossbeam_channel::bounded(4);
        let keys_a = Arc::new(VkBitset::default());
        keys_a.insert(0x70);
        let context_a = Arc::new(CallbackContext {
            suppressed_keys: keys_a,
            event_sender: sender_a,
            dropped_events: Arc::new(AtomicU64::new(0)),
        });
        let (sender_b, receiver_b) = crossbeam_channel::bounded(4);
        let keys_b = Arc::new(VkBitset::default());
        keys_b.insert(0x71);
        let context_b = Arc::new(CallbackContext {
            suppressed_keys: keys_b,
            event_sender: sender_b,
            dropped_events: Arc::new(AtomicU64::new(0)),
        });

        replace_callback_context(&slot, Arc::clone(&context_a));
        assert!(try_forward_from_callback_context(
            &slot,
            suppressed_event(0x70)
        ));
        assert_eq!(
            receiver_a
                .try_recv()
                .expect("context A 应接收首个事件")
                .vk_code,
            0x70
        );

        replace_callback_context(&slot, Arc::clone(&context_b));
        assert!(!try_forward_from_callback_context(
            &slot,
            suppressed_event(0x70)
        ));
        assert!(receiver_a.try_recv().is_err());
        assert!(try_forward_from_callback_context(
            &slot,
            suppressed_event(0x71)
        ));
        assert_eq!(
            receiver_b
                .try_recv()
                .expect("context B 应接收替换后的事件")
                .vk_code,
            0x71
        );

        clear_callback_context(&slot, &context_a);
        assert!(try_forward_from_callback_context(
            &slot,
            suppressed_event(0x71)
        ));
        assert_eq!(
            receiver_b
                .try_recv()
                .expect("旧 context 清理不得移除当前 context")
                .vk_code,
            0x71
        );

        clear_callback_context(&slot, &context_b);
        assert!(!try_forward_from_callback_context(
            &slot,
            suppressed_event(0x70)
        ));
        assert!(!try_forward_from_callback_context(
            &slot,
            suppressed_event(0x71)
        ));
        assert!(receiver_a.try_recv().is_err());
        assert!(receiver_b.try_recv().is_err());
    }

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

    #[test]
    fn maps_real_willhook_symbol_variants_to_vk_codes() {
        assert_eq!(keyboard_key_to_vk(&KeyboardKey::Comma), Some(0xBC));
        assert_eq!(keyboard_key_to_vk(&KeyboardKey::Period), Some(0xBE));
        assert_eq!(keyboard_key_to_vk(&KeyboardKey::Slash), Some(0xBF));
        assert_eq!(keyboard_key_to_vk(&KeyboardKey::SemiColon), Some(0xBA));
        assert_eq!(keyboard_key_to_vk(&KeyboardKey::Apostrophe), Some(0xDE));
        assert_eq!(keyboard_key_to_vk(&KeyboardKey::LeftBrace), Some(0xDB));
        assert_eq!(keyboard_key_to_vk(&KeyboardKey::BackwardSlash), Some(0xDC));
        assert_eq!(keyboard_key_to_vk(&KeyboardKey::RightBrace), Some(0xDD));
        assert_eq!(keyboard_key_to_vk(&KeyboardKey::Grave), Some(0xC0));
    }
}
