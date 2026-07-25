use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

use crate::hotkey_types::{HotkeyBinding, ModifierKey, NamedKey, PrimaryKey};
use enigo::{Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

static INPUT_SIMULATION_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
#[allow(dead_code)]
static TRACKED_INJECTED_KEYS: OnceLock<Mutex<Vec<Key>>> = OnceLock::new();
const INPUT_POST_ACTION_GAP: Duration = Duration::from_millis(35);

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
    pressed: Vec<Key>,
}

#[allow(dead_code)]
impl<'a, E: InputEmitter> TrackedModifierGuard<'a, E> {
    fn new(emitter: &'a E) -> Self {
        Self {
            emitter,
            pressed: Vec::new(),
        }
    }

    fn press(&mut self, key: Key, cancelled: &AtomicBool) -> Result<(), String> {
        ensure_not_cancelled(cancelled)?;
        self.emitter.key(key, Direction::Press)?;
        track_injected_key(key);
        self.pressed.push(key);
        Ok(())
    }

    fn release(&mut self, key: &Key) -> Result<(), String> {
        let Some(index) = self.pressed.iter().position(|pressed| pressed == key) else {
            return Ok(());
        };
        let key = self.pressed[index];
        self.emitter.key(key, Direction::Release)?;
        self.pressed.remove(index);
        untrack_injected_key(&key);
        Ok(())
    }
}

#[allow(dead_code)]
impl<E: InputEmitter> Drop for TrackedModifierGuard<'_, E> {
    fn drop(&mut self) {
        for key in self.pressed.drain(..).rev() {
            if self.emitter.key(key, Direction::Release).is_ok() {
                untrack_injected_key(&key);
            }
        }
    }
}

#[allow(dead_code)]
fn track_injected_key(key: Key) {
    let keys = TRACKED_INJECTED_KEYS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut keys) = keys.lock() {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
}

#[allow(dead_code)]
fn untrack_injected_key(key: &Key) {
    let Some(keys) = TRACKED_INJECTED_KEYS.get() else {
        return;
    };
    if let Ok(mut keys) = keys.lock() {
        keys.retain(|tracked| tracked != key);
    }
}

#[allow(dead_code)]
fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::SeqCst) {
        return Err("输入操作已取消".to_string());
    }
    Ok(())
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
) -> Result<(), String> {
    let (center_x, center_y) = region_center(region);
    ensure_not_cancelled(cancelled)?;
    emitter.move_mouse(center_x, center_y)?;
    ensure_not_cancelled(cancelled)?;
    emitter.click_left()
}

#[allow(dead_code)]
fn replace_text_with_emitter_locked<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    value: &str,
    char_delay_ms: u64,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    click_region_center_with_emitter(emitter, region, cancelled)?;

    let mut ctrl = TrackedModifierGuard::new(emitter);
    ctrl.press(Key::Control, cancelled)?;
    ensure_not_cancelled(cancelled)?;
    emitter.key(Key::Unicode('a'), Direction::Click)?;
    ensure_not_cancelled(cancelled)?;
    emitter.key(Key::Backspace, Direction::Click)?;
    ensure_not_cancelled(cancelled)?;
    ctrl.release(&Key::Control)?;

    for ch in value.chars() {
        ensure_not_cancelled(cancelled)?;
        emitter.key(Key::Unicode(ch), Direction::Click)?;
        if char_delay_ms > 0 {
            thread::sleep(Duration::from_millis(char_delay_ms));
        }
    }

    Ok(())
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
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let lock = INPUT_SIMULATION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;
    replace_text_with_emitter_locked(emitter, region, value, 0, &cancelled)
}

#[allow(dead_code)]
pub async fn click_region_center_cancellable(
    region: crate::morse::types::RegionRect,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    run_serialized_input(move || {
        ensure_not_cancelled(&cancelled)?;
        let emitter = EnigoInputEmitter::new()?;
        click_region_center_with_emitter(&emitter, &region, &cancelled)
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
    run_serialized_input(move || {
        ensure_not_cancelled(&cancelled)?;
        let emitter = EnigoInputEmitter::new()?;
        replace_text_with_emitter_locked(&emitter, &region, &value, char_delay_ms, &cancelled)
    })
    .await
}

#[allow(dead_code)]
pub fn release_tracked_injected_inputs() {
    let Ok(mut enigo) = Enigo::new(&Settings::default()) else {
        return;
    };
    let Some(tracked_keys) = TRACKED_INJECTED_KEYS.get() else {
        return;
    };
    let Ok(mut tracked_keys) = tracked_keys.lock() else {
        return;
    };
    let keys = std::mem::take(&mut *tracked_keys);
    drop(tracked_keys);

    for key in keys.into_iter().rev() {
        if enigo.key(key, Direction::Release).is_err() {
            track_injected_key(key);
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
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as StdMutex,
    };
    use tokio::sync::{oneshot, Mutex};

    #[derive(Default)]
    struct RecordingEmitter {
        characters: StdMutex<Vec<char>>,
        pressed_keys: StdMutex<Vec<Key>>,
        cancel_after_character: StdMutex<Option<(usize, Arc<AtomicBool>)>>,
    }

    impl RecordingEmitter {
        fn cancel_after_character(&self, count: usize, cancelled: Arc<AtomicBool>) {
            *self.cancel_after_character.lock().unwrap() = Some((count, cancelled));
        }

        fn characters(&self) -> Vec<char> {
            self.characters.lock().unwrap().clone()
        }

        fn pressed_keys(&self) -> Vec<Key> {
            self.pressed_keys.lock().unwrap().clone()
        }
    }

    impl InputEmitter for RecordingEmitter {
        fn move_mouse(&self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }

        fn click_left(&self) -> Result<(), String> {
            Ok(())
        }

        fn key(&self, key: Key, direction: Direction) -> Result<(), String> {
            match (key, direction) {
                (Key::Unicode(ch), Direction::Click) => {
                    if self.pressed_keys.lock().unwrap().contains(&Key::Control) {
                        return Ok(());
                    }
                    let character_count = {
                        let mut characters = self.characters.lock().unwrap();
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
                }
                (Key::Control, Direction::Press) => {
                    self.pressed_keys.lock().unwrap().push(Key::Control);
                }
                (Key::Control, Direction::Release) => {
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
        let emitter = RecordingEmitter::default();
        let cancel = Arc::new(AtomicBool::new(false));
        emitter.cancel_after_character(1, cancel.clone());

        replace_text_with_emitter(&emitter, &rect(), "12345", cancel)
            .await
            .unwrap_err();

        assert_eq!(emitter.characters(), vec!['1']);
        assert_eq!(emitter.pressed_keys(), Vec::<Key>::new());
    }
}
