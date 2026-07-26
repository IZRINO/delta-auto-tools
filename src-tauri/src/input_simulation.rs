use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, MutexGuard, OnceLock,
    },
    thread,
    time::Duration,
};

use crate::hotkey_types::{HotkeyBinding, ModifierKey, NamedKey, PrimaryKey};
use enigo::{Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

static INPUT_SIMULATION_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static INPUT_RELEASE_GENERATION: AtomicU64 = AtomicU64::new(0);
static INPUT_ACTION_STATE: OnceLock<Mutex<InputActionState>> = OnceLock::new();
const INPUT_POST_ACTION_GAP: Duration = Duration::from_millis(35);
const INPUT_CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextInputTiming {
    pub focus_delay_ms: u64,
    pub char_delay_ms: u64,
    pub settle_delay_ms: u64,
}

#[derive(Default)]
struct InputActionState {
    tracked_keys: Vec<Key>,
}

#[allow(dead_code)]
fn input_action_state() -> &'static Mutex<InputActionState> {
    INPUT_ACTION_STATE.get_or_init(|| Mutex::new(InputActionState::default()))
}

fn lock_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_input_action_state() -> MutexGuard<'static, InputActionState> {
    lock_recover(input_action_state())
}

#[allow(dead_code)]
fn input_release_generation() -> u64 {
    INPUT_RELEASE_GENERATION.load(Ordering::SeqCst)
}

#[allow(dead_code)]
fn invalidate_cancellable_inputs() {
    INPUT_RELEASE_GENERATION.fetch_add(1, Ordering::SeqCst);
}

#[allow(dead_code)]
fn ensure_not_cancelled(cancelled: &AtomicBool, generation: u64) -> Result<(), String> {
    if cancelled.load(Ordering::SeqCst) || generation != input_release_generation() {
        return Err("输入操作已取消".to_string());
    }
    Ok(())
}

#[allow(dead_code)]
fn run_cancellable_input_action<T>(
    cancelled: &AtomicBool,
    generation: u64,
    action: impl FnOnce(&mut InputActionState) -> Result<T, String>,
) -> Result<T, String> {
    let mut state = lock_input_action_state();
    ensure_not_cancelled(cancelled, generation)?;
    action(&mut state)
}

#[allow(dead_code)]
trait InputEmitter {
    fn move_mouse(&self, x: i32, y: i32) -> Result<(), String>;
    fn click_left(&self) -> Result<(), String>;
    fn key(&self, key: Key, direction: Direction) -> Result<(), String>;
}

#[allow(dead_code)]
struct EnigoInputEmitter {
    enigo: Mutex<Enigo>,
}

#[allow(dead_code)]
impl EnigoInputEmitter {
    fn new() -> Result<Self, String> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化自动输入失败: {error}"))?;
        Ok(Self {
            enigo: Mutex::new(enigo),
        })
    }
}

#[allow(dead_code)]
impl InputEmitter for EnigoInputEmitter {
    fn move_mouse(&self, x: i32, y: i32) -> Result<(), String> {
        self.enigo
            .lock()
            .map_err(|_| "自动输入状态已损坏".to_string())?
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|error| format!("移动鼠标失败: {error}"))
    }

    fn click_left(&self) -> Result<(), String> {
        self.enigo
            .lock()
            .map_err(|_| "自动输入状态已损坏".to_string())?
            .button(enigo::Button::Left, Direction::Click)
            .map_err(|error| format!("鼠标左键点击失败: {error}"))
    }

    fn key(&self, key: Key, direction: Direction) -> Result<(), String> {
        self.enigo
            .lock()
            .map_err(|_| "自动输入状态已损坏".to_string())?
            .key(key, direction)
            .map_err(|error| format!("自动输入按键失败: {error}"))
    }
}

#[allow(dead_code)]
struct TrackedModifierGuard<'a, E: InputEmitter> {
    emitter: &'a E,
    cancelled: &'a AtomicBool,
    generation: u64,
    pressed: Vec<Key>,
}

#[allow(dead_code)]
impl<'a, E: InputEmitter> TrackedModifierGuard<'a, E> {
    fn new(emitter: &'a E, cancelled: &'a AtomicBool, generation: u64) -> Self {
        Self {
            emitter,
            cancelled,
            generation,
            pressed: Vec::new(),
        }
    }

    fn press(&mut self, key: Key) -> Result<(), String> {
        run_cancellable_input_action(self.cancelled, self.generation, |state| {
            self.emitter.key(key, Direction::Press)?;
            track_injected_key(state, key);
            self.pressed.push(key);
            Ok(())
        })
    }

    fn release(&mut self, key: &Key) -> Result<(), String> {
        run_cancellable_input_action(self.cancelled, self.generation, |state| {
            let Some(index) = self.pressed.iter().position(|pressed| pressed == key) else {
                return Ok(());
            };
            let key = self.pressed[index];
            self.emitter.key(key, Direction::Release)?;
            self.pressed.remove(index);
            untrack_injected_key(state, &key);
            Ok(())
        })
    }
}

#[allow(dead_code)]
impl<E: InputEmitter> Drop for TrackedModifierGuard<'_, E> {
    fn drop(&mut self) {
        let mut state = lock_input_action_state();
        for key in self.pressed.drain(..).rev() {
            if !take_tracked_injected_key(&mut state, &key) {
                continue;
            }
            let _ = self.emitter.key(key, Direction::Release);
        }
    }
}

#[allow(dead_code)]
fn track_injected_key(state: &mut InputActionState, key: Key) {
    if !state.tracked_keys.contains(&key) {
        state.tracked_keys.push(key);
    }
}

#[allow(dead_code)]
fn untrack_injected_key(state: &mut InputActionState, key: &Key) {
    state.tracked_keys.retain(|tracked| tracked != key);
}

fn take_tracked_injected_key(state: &mut InputActionState, key: &Key) -> bool {
    let Some(index) = state.tracked_keys.iter().position(|tracked| tracked == key) else {
        return false;
    };
    state.tracked_keys.remove(index);
    true
}

fn wait_cancellable_input_delay(
    cancelled: &AtomicBool,
    generation: u64,
    delay_ms: u64,
) -> Result<(), String> {
    let mut remaining = Duration::from_millis(delay_ms);
    while !remaining.is_zero() {
        ensure_not_cancelled(cancelled, generation)?;
        let slice = remaining.min(INPUT_CANCELLATION_POLL_INTERVAL);
        thread::sleep(slice);
        remaining = remaining.saturating_sub(slice);
    }
    ensure_not_cancelled(cancelled, generation)
}

#[allow(dead_code)]
fn region_center(region: &crate::morse::types::RegionRect) -> (i32, i32) {
    (
        region.x.saturating_add(region.width / 2),
        region.y.saturating_add(region.height / 2),
    )
}

#[allow(dead_code)]
fn click_region_center_with_emitter<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    let (center_x, center_y) = region_center(region);
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.move_mouse(center_x, center_y)
    })?;
    run_cancellable_input_action(cancelled, generation, |_| emitter.click_left())
}

#[allow(dead_code)]
fn replace_text_with_emitter_locked<E: InputEmitter, F: FnOnce()>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    value: &str,
    timing: TextInputTiming,
    cancelled: &AtomicBool,
    generation: u64,
    after_ctrl_pressed: F,
) -> Result<(), String> {
    click_region_center_with_emitter(emitter, region, cancelled, generation)?;
    wait_cancellable_input_delay(cancelled, generation, timing.focus_delay_ms)?;

    let mut ctrl = TrackedModifierGuard::new(emitter, cancelled, generation);
    ctrl.press(Key::Control)?;
    after_ctrl_pressed();
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.key(Key::Unicode('a'), Direction::Click)
    })?;
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.key(Key::Backspace, Direction::Click)
    })?;
    ctrl.release(&Key::Control)?;

    for ch in value.chars() {
        run_cancellable_input_action(cancelled, generation, |_| {
            emitter.key(Key::Unicode(ch), Direction::Click)
        })?;
        wait_cancellable_input_delay(cancelled, generation, timing.char_delay_ms)?;
    }

    wait_cancellable_input_delay(cancelled, generation, timing.settle_delay_ms)
}

async fn run_serialized_input<F, T>(operation: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let lock = INPUT_SIMULATION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;
    let result = tokio::task::spawn_blocking(operation)
        .await
        .map_err(|err| format!("输入模拟任务失败: {err}"))?;
    tokio::time::sleep(INPUT_POST_ACTION_GAP).await;
    result
}

#[cfg(test)]
async fn run_serialized_input_for_test<F, Fut>(operation: F) -> Result<(), String>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    let lock = INPUT_SIMULATION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;
    operation().await
}

#[cfg(test)]
async fn replace_text_with_emitter<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    value: &str,
    timing: TextInputTiming,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let lock = INPUT_SIMULATION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;
    replace_text_with_emitter_locked(
        emitter,
        region,
        value,
        timing,
        &cancelled,
        input_release_generation(),
        || {},
    )
}

#[cfg(test)]
async fn replace_text_with_emitter_after_ctrl_press<E: InputEmitter, F: FnOnce()>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    value: &str,
    cancelled: Arc<AtomicBool>,
    after_ctrl_pressed: F,
) -> Result<(), String> {
    let lock = INPUT_SIMULATION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;
    replace_text_with_emitter_locked(
        emitter,
        region,
        value,
        TextInputTiming {
            focus_delay_ms: 0,
            char_delay_ms: 0,
            settle_delay_ms: 0,
        },
        &cancelled,
        input_release_generation(),
        after_ctrl_pressed,
    )
}

#[allow(dead_code)]
pub async fn click_region_center_cancellable(
    region: crate::morse::types::RegionRect,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        click_region_center_with_emitter(&emitter, &region, &cancelled, generation)
    })
    .await
}

#[allow(dead_code)]
pub async fn replace_text_at_region_cancellable(
    region: crate::morse::types::RegionRect,
    value: String,
    char_delay_ms: u64,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    replace_text_at_region_with_timing_cancellable(
        region,
        value,
        TextInputTiming {
            focus_delay_ms: 0,
            char_delay_ms,
            settle_delay_ms: 0,
        },
        cancelled,
    )
    .await
}

#[allow(dead_code)]
pub async fn replace_text_at_region_with_timing_cancellable(
    region: crate::morse::types::RegionRect,
    value: String,
    timing: TextInputTiming,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        replace_text_with_emitter_locked(
            &emitter,
            &region,
            &value,
            timing,
            &cancelled,
            generation,
            || {},
        )
    })
    .await
}

#[allow(dead_code)]
pub fn release_tracked_injected_inputs() {
    release_tracked_injected_inputs_with_factory(EnigoInputEmitter::new);
}

fn release_tracked_injected_inputs_with_factory<E, F>(mut emitter_factory: F)
where
    E: InputEmitter,
    F: FnMut() -> Result<E, String>,
{
    invalidate_cancellable_inputs();
    let mut emitter = None;

    loop {
        let mut state = lock_input_action_state();
        if state.tracked_keys.is_empty() {
            return;
        }

        if emitter.is_none() {
            emitter = emitter_factory().ok();
        }
        if let Some(emitter) = emitter.as_ref() {
            release_tracked_injected_keys_with(&mut state, emitter);
        }
        if state.tracked_keys.is_empty() {
            return;
        }

        drop(state);
        thread::sleep(INPUT_CANCELLATION_POLL_INTERVAL);
    }
}

fn release_tracked_injected_keys_with<E: InputEmitter>(state: &mut InputActionState, emitter: &E) {
    let keys = std::mem::take(&mut state.tracked_keys);
    for key in keys.into_iter().rev() {
        if emitter.key(key, Direction::Release).is_err() {
            track_injected_key(state, key);
        }
    }
}

pub async fn type_text(value: &str, delay_ms: u64) -> Result<(), String> {
    let value = value.to_string();
    let char_count = value.chars().count();

    crate::log_debug!(
        "input_simulation",
        "输入模拟开始",
        "kind" => "text",
        "primary" => "text",
        "char_count" => char_count,
        "card_id" => Option::<String>::None
    );
    let result = run_serialized_input(move || -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化自动输入失败: {error}"))?;

        for ch in value.chars() {
            enigo
                .key(Key::Unicode(ch), Direction::Click)
                .map_err(|error| format!("自动输入字符 {ch} 失败: {error}"))?;

            if delay_ms > 0 {
                thread::sleep(Duration::from_millis(delay_ms));
            }
        }

        Ok(())
    })
    .await;
    crate::log_debug!(
        "input_simulation",
        "输入模拟结束",
        "kind" => "text",
        "primary" => "text",
        "char_count" => char_count,
        "success" => result.is_ok(),
        "error" => result.as_ref().err(),
        "card_id" => Option::<String>::None
    );
    result
}

pub async fn press_hotkey_once(hotkey: &str, label: &str) -> Result<(), String> {
    press_hotkey_once_with_card(hotkey, label, None).await
}

pub async fn press_hotkey_once_for_card(
    hotkey: &str,
    label: &str,
    card_id: &str,
) -> Result<(), String> {
    press_hotkey_once_with_card(hotkey, label, Some(card_id)).await
}

async fn press_hotkey_once_with_card(
    hotkey: &str,
    label: &str,
    card_id: Option<&str>,
) -> Result<(), String> {
    let hotkey = hotkey.trim().to_string();
    let label = label.to_string();
    let card_id = card_id.map(str::to_string);
    if hotkey.is_empty() {
        return Ok(());
    }
    let task_label = label.clone();
    let binding =
        HotkeyBinding::parse(&hotkey).map_err(|error| format!("{task_label}配置无效: {error}"))?;
    let primary_label = crate::hotkey_types::primary_to_string(binding.primary);

    crate::log_debug!(
        "input_simulation",
        "输入模拟开始",
        "kind" => "hotkey",
        "primary" => primary_label.clone(),
        "hotkey" => hotkey.clone(),
        "label" => label.clone(),
        "card_id" => card_id.as_deref()
    );
    let hotkey_for_task = hotkey.clone();
    let result = run_serialized_input(move || -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化{task_label}失败: {error}"))?;

        let modifiers = ordered_modifiers(&binding);
        for modifier in &modifiers {
            enigo
                .key(modifier_to_key(*modifier), Direction::Press)
                .map_err(|error| format!("按下修饰键失败: {error}"))?;
        }

        let primary = primary_to_key(binding.primary)?;
        let click_result = enigo
            .key(primary, Direction::Click)
            .map_err(|error| format!("执行{task_label} {hotkey_for_task} 失败: {error}"));

        for modifier in modifiers.iter().rev() {
            let _ = enigo.key(modifier_to_key(*modifier), Direction::Release);
        }

        click_result
    })
    .await;
    crate::log_debug!(
        "input_simulation",
        "输入模拟结束",
        "kind" => "hotkey",
        "primary" => primary_label,
        "hotkey" => hotkey,
        "label" => label,
        "success" => result.is_ok(),
        "error" => result.as_ref().err(),
        "card_id" => card_id.as_deref()
    );
    result
}

pub async fn click_points(points: &[(i32, i32, u64)]) -> Result<(), String> {
    click_points_with_card(points, None).await
}

pub async fn click_points_for_card(
    points: &[(i32, i32, u64)],
    card_id: &str,
) -> Result<(), String> {
    click_points_with_card(points, Some(card_id)).await
}

async fn click_points_with_card(
    points: &[(i32, i32, u64)],
    card_id: Option<&str>,
) -> Result<(), String> {
    if points.is_empty() {
        return Ok(());
    }

    let points = points.to_vec();
    let point_count = points.len();
    let card_id = card_id.map(str::to_string);
    crate::log_debug!(
        "input_simulation",
        "输入模拟开始",
        "kind" => "click",
        "primary" => "left",
        "button" => "left",
        "point_count" => point_count,
        "card_id" => card_id.as_deref()
    );
    let result = run_serialized_input(move || -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化鼠标点击失败: {error}"))?;

        for (center_x, center_y, delay_ms) in &points {
            if *delay_ms > 0 {
                thread::sleep(Duration::from_millis(*delay_ms));
            }
            enigo
                .move_mouse(*center_x, *center_y, Coordinate::Abs)
                .map_err(|error| format!("移动鼠标到 ({center_x}, {center_y}) 失败: {error}"))?;
            enigo
                .button(enigo::Button::Left, Direction::Click)
                .map_err(|error| format!("鼠标左键点击失败: {error}"))?;
        }

        Ok(())
    })
    .await;
    crate::log_debug!(
        "input_simulation",
        "输入模拟结束",
        "kind" => "click",
        "primary" => "left",
        "button" => "left",
        "point_count" => point_count,
        "success" => result.is_ok(),
        "error" => result.as_ref().err(),
        "card_id" => card_id.as_deref()
    );
    result
}

fn ordered_modifiers(binding: &HotkeyBinding) -> Vec<ModifierKey> {
    [
        ModifierKey::Ctrl,
        ModifierKey::Alt,
        ModifierKey::Shift,
        ModifierKey::Super,
    ]
    .into_iter()
    .filter(|modifier| binding.modifiers.contains(modifier))
    .collect()
}

fn modifier_to_key(modifier: ModifierKey) -> Key {
    match modifier {
        ModifierKey::Ctrl => Key::Control,
        ModifierKey::Alt => Key::Alt,
        ModifierKey::Shift => Key::Shift,
        ModifierKey::Super => Key::Meta,
    }
}

fn primary_to_key(primary: PrimaryKey) -> Result<Key, String> {
    Ok(match primary {
        PrimaryKey::Letter(value) => letter_to_key(value)?,
        PrimaryKey::Digit(value) => digit_to_key(value)?,
        PrimaryKey::Function(value) => function_to_key(value)?,
        PrimaryKey::Named(named) => named_to_key(named),
    })
}

fn letter_to_key(value: char) -> Result<Key, String> {
    Ok(match value {
        'A' => Key::A,
        'B' => Key::B,
        'C' => Key::C,
        'D' => Key::D,
        'E' => Key::E,
        'F' => Key::F,
        'G' => Key::G,
        'H' => Key::H,
        'I' => Key::I,
        'J' => Key::J,
        'K' => Key::K,
        'L' => Key::L,
        'M' => Key::M,
        'N' => Key::N,
        'O' => Key::O,
        'P' => Key::P,
        'Q' => Key::Q,
        'R' => Key::R,
        'S' => Key::S,
        'T' => Key::T,
        'U' => Key::U,
        'V' => Key::V,
        'W' => Key::W,
        'X' => Key::X,
        'Y' => Key::Y,
        'Z' => Key::Z,
        _ => return Err(format!("暂不支持的字母按键: {value}")),
    })
}

fn digit_to_key(value: char) -> Result<Key, String> {
    Ok(match value {
        '0' => Key::Num0,
        '1' => Key::Num1,
        '2' => Key::Num2,
        '3' => Key::Num3,
        '4' => Key::Num4,
        '5' => Key::Num5,
        '6' => Key::Num6,
        '7' => Key::Num7,
        '8' => Key::Num8,
        '9' => Key::Num9,
        _ => return Err(format!("暂不支持的数字按键: {value}")),
    })
}

fn function_to_key(value: u8) -> Result<Key, String> {
    Ok(match value {
        1 => Key::F1,
        2 => Key::F2,
        3 => Key::F3,
        4 => Key::F4,
        5 => Key::F5,
        6 => Key::F6,
        7 => Key::F7,
        8 => Key::F8,
        9 => Key::F9,
        10 => Key::F10,
        11 => Key::F11,
        12 => Key::F12,
        _ => return Err(format!("暂不支持的功能键: F{value}")),
    })
}

fn named_to_key(named: NamedKey) -> Key {
    match named {
        NamedKey::Space => Key::Space,
        NamedKey::Enter => Key::Return,
        NamedKey::Tab => Key::Tab,
        NamedKey::Esc => Key::Escape,
        NamedKey::Up => Key::UpArrow,
        NamedKey::Down => Key::DownArrow,
        NamedKey::Left => Key::LeftArrow,
        NamedKey::Right => Key::RightArrow,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::Insert => Key::Insert,
        NamedKey::Delete => Key::Delete,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Alt => Key::Alt,
        NamedKey::Semicolon => Key::OEM1,
        NamedKey::Comma => Key::OEMComma,
        NamedKey::Period => Key::OEMPeriod,
        NamedKey::Slash => Key::OEM2,
        NamedKey::Backslash => Key::OEM5,
        NamedKey::BracketLeft => Key::OEM4,
        NamedKey::BracketRight => Key::OEM6,
        NamedKey::Minus => Key::OEMMinus,
        NamedKey::Equal => Key::OEMPlus,
        NamedKey::Plus => Key::Add,
        NamedKey::Backquote => Key::OEM3,
        NamedKey::Quote => Key::OEM7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex as StdMutex,
    };
    use std::time::Instant;
    use tokio::sync::{oneshot, Mutex};

    static INPUT_SIMULATION_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    async fn lock_input_simulation_tests() -> tokio::sync::MutexGuard<'static, ()> {
        INPUT_SIMULATION_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .await
    }

    #[derive(Default)]
    struct RecordingEmitter {
        action_count: StdMutex<usize>,
        characters: StdMutex<Vec<char>>,
        pressed_keys: StdMutex<Vec<Key>>,
        ctrl_release_count: StdMutex<usize>,
        clicked_at: StdMutex<Option<Instant>>,
        first_character_at: StdMutex<Option<Instant>>,
        fail_ctrl_release: AtomicBool,
        cancel_after_character: StdMutex<Option<(usize, Arc<AtomicBool>)>>,
        notify_after_character: StdMutex<Option<(usize, mpsc::Sender<()>)>>,
    }

    impl RecordingEmitter {
        fn cancel_after_character(&self, count: usize, cancelled: Arc<AtomicBool>) {
            *self.cancel_after_character.lock().unwrap() = Some((count, cancelled));
        }

        fn characters(&self) -> Vec<char> {
            self.characters.lock().unwrap().clone()
        }

        fn notify_after_character(&self, count: usize, sender: mpsc::Sender<()>) {
            *self.notify_after_character.lock().unwrap() = Some((count, sender));
        }

        fn action_count(&self) -> usize {
            *self.action_count.lock().unwrap()
        }

        fn set_fail_ctrl_release(&self, fail: bool) {
            self.fail_ctrl_release.store(fail, Ordering::SeqCst);
        }

        fn pressed_keys(&self) -> Vec<Key> {
            self.pressed_keys.lock().unwrap().clone()
        }

        fn ctrl_release_count(&self) -> usize {
            *self.ctrl_release_count.lock().unwrap()
        }

        fn clicked_at(&self) -> Option<Instant> {
            *self.clicked_at.lock().unwrap()
        }

        fn first_character_at(&self) -> Option<Instant> {
            *self.first_character_at.lock().unwrap()
        }
    }

    impl InputEmitter for RecordingEmitter {
        fn move_mouse(&self, _: i32, _: i32) -> Result<(), String> {
            *self.action_count.lock().unwrap() += 1;
            Ok(())
        }

        fn click_left(&self) -> Result<(), String> {
            *self.action_count.lock().unwrap() += 1;
            *self.clicked_at.lock().unwrap() = Some(Instant::now());
            Ok(())
        }

        fn key(&self, key: Key, direction: Direction) -> Result<(), String> {
            *self.action_count.lock().unwrap() += 1;
            if key == Key::Control
                && direction == Direction::Release
                && self.fail_ctrl_release.load(Ordering::SeqCst)
            {
                return Err("测试 Ctrl release 失败".to_string());
            }
            match (key, direction) {
                (Key::Unicode(ch), Direction::Click) => {
                    if self.pressed_keys.lock().unwrap().contains(&Key::Control) {
                        return Ok(());
                    }
                    let character_count = {
                        let mut characters = self.characters.lock().unwrap();
                        let mut first_character_at = self.first_character_at.lock().unwrap();
                        first_character_at.get_or_insert_with(Instant::now);
                        characters.push(ch);
                        characters.len()
                    };
                    if let Some((count, cancelled)) =
                        self.cancel_after_character.lock().unwrap().clone()
                    {
                        if character_count >= count {
                            cancelled.store(true, Ordering::SeqCst);
                        }
                    }
                    if let Some((count, notifier)) =
                        self.notify_after_character.lock().unwrap().clone()
                    {
                        if character_count >= count {
                            let _ = notifier.send(());
                        }
                    }
                }
                (Key::Control, Direction::Press) => {
                    self.pressed_keys.lock().unwrap().push(Key::Control);
                }
                (Key::Control, Direction::Release) => {
                    *self.ctrl_release_count.lock().unwrap() += 1;
                    self.pressed_keys
                        .lock()
                        .unwrap()
                        .retain(|key| *key != Key::Control);
                }
                _ => {}
            }
            Ok(())
        }
    }

    struct SharedRecordingEmitter(Arc<RecordingEmitter>);

    impl InputEmitter for SharedRecordingEmitter {
        fn move_mouse(&self, x: i32, y: i32) -> Result<(), String> {
            self.0.move_mouse(x, y)
        }

        fn click_left(&self) -> Result<(), String> {
            self.0.click_left()
        }

        fn key(&self, key: Key, direction: Direction) -> Result<(), String> {
            self.0.key(key, direction)
        }
    }

    struct FailingReleaseEmitter {
        action_count: Arc<AtomicUsize>,
    }

    impl InputEmitter for FailingReleaseEmitter {
        fn move_mouse(&self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }

        fn click_left(&self) -> Result<(), String> {
            Ok(())
        }

        fn key(&self, _: Key, _: Direction) -> Result<(), String> {
            self.action_count.fetch_add(1, Ordering::SeqCst);
            Err("测试 emergency release 失败".to_string())
        }
    }

    fn assert_failed_emergency_release_waits_for_guard_cleanup<E, F>(
        mut factory: F,
        emergency_action_count: Arc<AtomicUsize>,
        guard_release_fails: bool,
    ) where
        E: InputEmitter + Send + 'static,
        F: FnMut() -> Result<E, String> + Send + 'static,
    {
        let emitter = Arc::new(RecordingEmitter::default());
        emitter.set_fail_ctrl_release(guard_release_fails);
        let typing_emitter = Arc::clone(&emitter);
        let typing_cancelled = Arc::new(AtomicBool::new(false));
        let typing_region = rect();
        let (ctrl_pressed_tx, ctrl_pressed_rx) = mpsc::channel();
        let (release_barrier_tx, release_barrier_rx) = mpsc::channel();

        let typing = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            runtime.block_on(replace_text_with_emitter_after_ctrl_press(
                typing_emitter.as_ref(),
                &typing_region,
                "12",
                typing_cancelled,
                move || {
                    ctrl_pressed_tx.send(()).unwrap();
                    release_barrier_rx
                        .recv_timeout(Duration::from_secs(1))
                        .expect("emergency release 未建立 action-state barrier");
                },
            ))
        });

        ctrl_pressed_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("Ctrl 未按下");

        let release_emitter = Arc::clone(&emitter);
        let release_action_count = Arc::clone(&emergency_action_count);
        let (release_done_tx, release_done_rx) = mpsc::channel();
        let release = std::thread::spawn(move || {
            release_tracked_injected_inputs_with_factory(move || {
                let _ = release_barrier_tx.send(());
                factory()
            });
            release_done_tx
                .send((
                    release_emitter.action_count(),
                    release_action_count.load(Ordering::SeqCst),
                ))
                .unwrap();
        });

        let action_counts_at_return = release_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("emergency release 未等待 guard cleanup 后返回");
        let typing_result = typing.join().expect("旧输入任务 panic");
        release.join().expect("emergency release 线程 panic");

        assert_eq!(typing_result, Err("输入操作已取消".to_string()));
        assert_eq!(action_counts_at_return.0, emitter.action_count());
        assert_eq!(
            action_counts_at_return.1,
            emergency_action_count.load(Ordering::SeqCst)
        );
        assert_eq!(
            emitter.ctrl_release_count(),
            usize::from(!guard_release_fails)
        );
        assert!(lock_input_action_state().tracked_keys.is_empty());
    }

    fn rect() -> crate::morse::types::RegionRect {
        crate::morse::types::RegionRect {
            x: 100,
            y: 200,
            width: 80,
            height: 30,
        }
    }

    #[test]
    fn maps_comma_and_period_to_enigo_keys() {
        assert!(matches!(
            primary_to_key(PrimaryKey::Named(NamedKey::Comma)).unwrap(),
            Key::OEMComma
        ));
        assert!(matches!(
            primary_to_key(PrimaryKey::Named(NamedKey::Period)).unwrap(),
            Key::OEMPeriod
        ));
    }

    #[tokio::test]
    async fn serialized_input_jobs_do_not_overlap() {
        let _test_guard = lock_input_simulation_tests().await;
        let events = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let (first_started_tx, first_started_rx) = oneshot::channel::<()>();
        let (release_first_tx, release_first_rx) = oneshot::channel::<()>();

        let first_events = Arc::clone(&events);
        let first = tokio::spawn(run_serialized_input_for_test(move || async move {
            first_events.lock().await.push("first-start");
            let _ = first_started_tx.send(());
            release_first_rx.await.map_err(|error| error.to_string())?;
            first_events.lock().await.push("first-end");
            Ok(())
        }));

        first_started_rx.await.unwrap();

        let second_events = Arc::clone(&events);
        let second = tokio::spawn(run_serialized_input_for_test(move || async move {
            second_events.lock().await.push("second-start");
            second_events.lock().await.push("second-end");
            Ok(())
        }));

        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(events.lock().await.as_slice(), ["first-start"]);

        release_first_tx.send(()).unwrap();
        first.await.unwrap().unwrap();
        second.await.unwrap().unwrap();

        assert_eq!(
            events.lock().await.as_slice(),
            ["first-start", "first-end", "second-start", "second-end"]
        );
    }

    #[tokio::test]
    async fn cancelled_replace_text_stops_before_next_character_and_releases_ctrl() {
        let _test_guard = lock_input_simulation_tests().await;
        let emitter = RecordingEmitter::default();
        let cancel = Arc::new(AtomicBool::new(false));
        emitter.cancel_after_character(1, cancel.clone());

        replace_text_with_emitter(
            &emitter,
            &rect(),
            "12345",
            TextInputTiming {
                focus_delay_ms: 0,
                char_delay_ms: 0,
                settle_delay_ms: 0,
            },
            cancel,
        )
            .await
            .unwrap_err();

        assert_eq!(emitter.characters(), vec!['1']);
        assert_eq!(emitter.pressed_keys(), Vec::<Key>::new());
    }

    #[tokio::test]
    async fn replace_text_waits_for_focus_and_settle() {
        let _test_guard = lock_input_simulation_tests().await;
        let emitter = RecordingEmitter::default();
        let started_at = Instant::now();

        replace_text_with_emitter(
            &emitter,
            &rect(),
            "1",
            TextInputTiming {
                focus_delay_ms: 40,
                char_delay_ms: 0,
                settle_delay_ms: 30,
            },
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();

        let clicked_at = emitter.clicked_at().expect("未记录输入框点击");
        let first_character_at = emitter.first_character_at().expect("未记录首字符输入");
        assert!(first_character_at.duration_since(clicked_at) >= Duration::from_millis(30));
        assert!(started_at.elapsed() >= Duration::from_millis(60));
    }

    #[tokio::test]
    async fn emergency_release_interrupts_settle_delay() {
        let _test_guard = lock_input_simulation_tests().await;
        let emitter = Arc::new(RecordingEmitter::default());
        let (first_character_tx, first_character_rx) = mpsc::channel();
        emitter.notify_after_character(1, first_character_tx);

        let typing_emitter = Arc::clone(&emitter);
        let typing_cancelled = Arc::new(AtomicBool::new(false));
        let typing_region = rect();
        let typing = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            runtime.block_on(replace_text_with_emitter(
                typing_emitter.as_ref(),
                &typing_region,
                "1",
                TextInputTiming {
                    focus_delay_ms: 0,
                    char_delay_ms: 0,
                    settle_delay_ms: 500,
                },
                typing_cancelled,
            ))
        });

        first_character_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("首字符未输入");
        let released_at = Instant::now();
        release_tracked_injected_inputs_with_factory(|| {
            Ok(SharedRecordingEmitter(Arc::clone(&emitter)))
        });

        assert_eq!(typing.join().unwrap(), Err("输入操作已取消".to_string()));
        assert!(released_at.elapsed() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn emergency_release_after_ctrl_press_stops_before_selection_and_releases_ctrl() {
        let _test_guard = lock_input_simulation_tests().await;
        let emitter = Arc::new(RecordingEmitter::default());
        let release_emitter = Arc::clone(&emitter);
        let cancel = Arc::new(AtomicBool::new(false));
        let action_count_after_release = Arc::new(StdMutex::new(None));
        let release_action_count = Arc::clone(&action_count_after_release);

        let error = replace_text_with_emitter_after_ctrl_press(
            emitter.as_ref(),
            &rect(),
            "12345",
            cancel,
            move || {
                release_tracked_injected_inputs_with_factory(|| {
                    Ok(SharedRecordingEmitter(Arc::clone(&release_emitter)))
                });
                *release_action_count.lock().unwrap() = Some(release_emitter.action_count());
            },
        )
        .await
        .unwrap_err();

        assert_eq!(error, "输入操作已取消");
        assert!(emitter.characters().is_empty());
        assert_eq!(emitter.pressed_keys(), Vec::<Key>::new());
        assert_eq!(emitter.ctrl_release_count(), 1);
        assert_eq!(
            *action_count_after_release.lock().unwrap(),
            Some(emitter.action_count())
        );
    }

    #[tokio::test]
    async fn emergency_emitter_init_failure_waits_for_guard_cleanup() {
        let _test_guard = lock_input_simulation_tests().await;
        assert_failed_emergency_release_waits_for_guard_cleanup::<FailingReleaseEmitter, _>(
            || Err("测试 emitter 初始化失败".to_string()),
            Arc::new(AtomicUsize::new(0)),
            false,
        );
    }

    #[tokio::test]
    async fn emergency_key_release_failure_waits_for_guard_cleanup() {
        let _test_guard = lock_input_simulation_tests().await;
        let emergency_action_count = Arc::new(AtomicUsize::new(0));
        let factory_action_count = Arc::clone(&emergency_action_count);
        assert_failed_emergency_release_waits_for_guard_cleanup(
            move || {
                Ok(FailingReleaseEmitter {
                    action_count: Arc::clone(&factory_action_count),
                })
            },
            emergency_action_count,
            false,
        );
    }

    #[tokio::test]
    async fn repeated_emergency_and_guard_release_failures_do_not_wait_forever() {
        let _test_guard = lock_input_simulation_tests().await;
        let emergency_action_count = Arc::new(AtomicUsize::new(0));
        let factory_action_count = Arc::clone(&emergency_action_count);
        assert_failed_emergency_release_waits_for_guard_cleanup(
            move || {
                Ok(FailingReleaseEmitter {
                    action_count: Arc::clone(&factory_action_count),
                })
            },
            emergency_action_count,
            true,
        );
    }

    #[test]
    fn reclaims_poisoned_input_action_state_mutex() {
        let state = Arc::new(StdMutex::new(InputActionState::default()));
        let poisoned_state = Arc::clone(&state);
        let panic_result = std::thread::spawn(move || {
            let _guard = poisoned_state.lock().unwrap();
            panic!("测试 input action state poison");
        })
        .join();

        assert!(panic_result.is_err());
        let mut recovered = lock_recover(state.as_ref());
        track_injected_key(&mut recovered, Key::Control);
        assert_eq!(recovered.tracked_keys, vec![Key::Control]);
    }

    #[tokio::test]
    async fn emergency_release_interrupts_character_delay_and_unblocks_next_input_job() {
        let _test_guard = lock_input_simulation_tests().await;
        let emitter = Arc::new(RecordingEmitter::default());
        let (first_character_tx, first_character_rx) = mpsc::channel();
        emitter.notify_after_character(1, first_character_tx);

        let typing_emitter = Arc::clone(&emitter);
        let typing_cancelled = Arc::new(AtomicBool::new(false));
        let typing_region = rect();
        let typing = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            runtime.block_on(replace_text_with_emitter(
                typing_emitter.as_ref(),
                &typing_region,
                "12",
                TextInputTiming {
                    focus_delay_ms: 0,
                    char_delay_ms: 500,
                    settle_delay_ms: 0,
                },
                typing_cancelled,
            ))
        });

        first_character_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("首字符未输入");

        let next_job_acquired_at = Arc::new(StdMutex::new(None));
        let next_job_acquired_at_for_task = Arc::clone(&next_job_acquired_at);
        let next_job = tokio::spawn(run_serialized_input_for_test(move || async move {
            *next_job_acquired_at_for_task.lock().unwrap() = Some(Instant::now());
            Ok(())
        }));

        let released_at = Instant::now();
        release_tracked_injected_inputs_with_factory(|| {
            Ok(SharedRecordingEmitter(Arc::clone(&emitter)))
        });

        assert_eq!(typing.join().unwrap(), Err("输入操作已取消".to_string()));
        next_job.await.unwrap().unwrap();
        let next_job_delay = next_job_acquired_at
            .lock()
            .unwrap()
            .expect("下一输入任务未取得串行锁")
            .duration_since(released_at);
        assert!(
            next_job_delay < Duration::from_millis(100),
            "紧急释放后串行锁延迟 {next_job_delay:?}"
        );
    }
}
