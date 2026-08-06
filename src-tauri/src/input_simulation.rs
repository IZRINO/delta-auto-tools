use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, MutexGuard, OnceLock,
    },
    thread,
    time::Duration,
};

use crate::hotkey_types::{HotkeyBinding, ModifierKey, NamedKey, PrimaryKey};
use enigo::{Axis, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

static INPUT_SIMULATION_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static INPUT_RELEASE_GENERATION: AtomicU64 = AtomicU64::new(0);
static INPUT_ACTION_STATE: OnceLock<Mutex<InputActionState>> = OnceLock::new();
const INPUT_POST_ACTION_GAP: Duration = Duration::from_millis(35);
const INPUT_CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(10);
const INPUT_RELEASE_RETRY_DELAY: Duration = Duration::from_millis(50);
const INPUT_RELEASE_MAX_ATTEMPTS: usize = 3;
const DOUBLE_CLICK_GAP_MS: u64 = 80;
const COPY_AFTER_DOUBLE_CLICK_DELAY_MS: u64 = 100;

#[derive(Default)]
struct InputActionState {
    tracked_keys: Vec<Key>,
    left_mouse_pressed: bool,
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
    fn press_left(&self) -> Result<(), String>;
    fn release_left(&self) -> Result<(), String>;
    fn scroll_vertical(&self, length: i32) -> Result<(), String> {
        let _ = length;
        Err("当前输入 emitter 不支持滚轮".to_string())
    }
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

    fn press_left(&self) -> Result<(), String> {
        self.enigo
            .lock()
            .map_err(|_| "自动输入状态已损坏".to_string())?
            .button(enigo::Button::Left, Direction::Press)
            .map_err(|error| format!("鼠标左键按下失败: {error}"))
    }

    fn release_left(&self) -> Result<(), String> {
        self.enigo
            .lock()
            .map_err(|_| "自动输入状态已损坏".to_string())?
            .button(enigo::Button::Left, Direction::Release)
            .map_err(|error| format!("鼠标左键抬起失败: {error}"))
    }

    fn scroll_vertical(&self, length: i32) -> Result<(), String> {
        self.enigo
            .lock()
            .map_err(|_| "自动输入状态已损坏".to_string())?
            .scroll(length, Axis::Vertical)
            .map_err(|error| format!("鼠标滚轮操作失败: {error}"))
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

fn press_left_with_emitter<E: InputEmitter>(
    emitter: &E,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    run_cancellable_input_action(cancelled, generation, |state| {
        emitter.press_left()?;
        state.left_mouse_pressed = true;
        Ok(())
    })
}

fn release_left_if_tracked_with_emitter<E: InputEmitter>(
    state: &mut InputActionState,
    emitter: &E,
) -> Result<(), String> {
    if !state.left_mouse_pressed {
        return Ok(());
    }
    emitter.release_left()?;
    state.left_mouse_pressed = false;
    Ok(())
}

fn click_left_held_with_emitter<E: InputEmitter>(
    emitter: &E,
    hold_ms: u64,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    press_left_with_emitter(emitter, cancelled, generation)?;
    let hold_result = wait_cancellable_input_delay(cancelled, generation, hold_ms);
    let release_result = {
        let mut state = lock_input_action_state();
        release_left_if_tracked_with_emitter(&mut state, emitter)
    };
    match (hold_result, release_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(hold_error), Ok(())) => Err(hold_error),
        (Ok(()), Err(release_error)) => Err(release_error),
        (Err(hold_error), Err(release_error)) => {
            Err(format!("{hold_error}; 鼠标左键抬起失败: {release_error}"))
        }
    }
}

fn click_region_center_held_with_emitter<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    hold_ms: u64,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    let (center_x, center_y) = region_center(region);
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.move_mouse(center_x, center_y)
    })?;
    click_left_held_with_emitter(emitter, hold_ms, cancelled, generation)
}

fn move_region_center_with_emitter<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    let (x, y) = region_center(region);
    run_cancellable_input_action(cancelled, generation, |_| emitter.move_mouse(x, y))
}

#[allow(dead_code)]
fn click_screen_point_with_emitter<E: InputEmitter>(
    emitter: &E,
    x: i32,
    y: i32,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    run_cancellable_input_action(cancelled, generation, |_| emitter.move_mouse(x, y))?;
    run_cancellable_input_action(cancelled, generation, |_| emitter.click_left())
}

fn click_screen_point_held_with_emitter<E: InputEmitter>(
    emitter: &E,
    x: i32,
    y: i32,
    hold_ms: u64,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    run_cancellable_input_action(cancelled, generation, |_| emitter.move_mouse(x, y))?;
    click_left_held_with_emitter(emitter, hold_ms, cancelled, generation)
}

#[allow(dead_code)]
fn scroll_region_down_with_emitter<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    steps: i32,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    let (center_x, center_y) = region_center(region);
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.move_mouse(center_x, center_y)
    })?;
    run_cancellable_input_action(cancelled, generation, |_| emitter.scroll_vertical(steps))
}

#[allow(dead_code)]
fn scroll_region_segments_with_emitter<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    segments: &[(i32, u32)],
    step_interval_ms: u64,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    let (center_x, center_y) = region_center(region);
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.move_mouse(center_x, center_y)
    })?;
    let total = segments.iter().try_fold(0_u32, |total, (_, count)| {
        total
            .checked_add(*count)
            .ok_or_else(|| "滚轮次数超出范围".to_string())
    })?;
    let mut emitted = 0_u32;
    for (delta, count) in segments {
        for _ in 0..*count {
            run_cancellable_input_action(cancelled, generation, |_| {
                emitter.scroll_vertical(*delta)
            })?;
            emitted += 1;
            if emitted < total {
                wait_cancellable_input_delay(cancelled, generation, step_interval_ms)?;
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn double_click_region_and_copy_with_emitter<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    let (center_x, center_y) = region_center(region);
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.move_mouse(center_x, center_y)
    })?;
    run_cancellable_input_action(cancelled, generation, |_| emitter.click_left())?;
    wait_cancellable_input_delay(cancelled, generation, DOUBLE_CLICK_GAP_MS)?;
    run_cancellable_input_action(cancelled, generation, |_| emitter.click_left())?;
    wait_cancellable_input_delay(cancelled, generation, COPY_AFTER_DOUBLE_CLICK_DELAY_MS)?;

    let mut ctrl = TrackedModifierGuard::new(emitter, cancelled, generation);
    ctrl.press(Key::Control)?;
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.key(Key::Unicode('c'), Direction::Click)
    })?;
    ctrl.release(&Key::Control)
}

fn double_click_region_and_copy_held_with_emitter<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    hold_ms: u64,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    let (center_x, center_y) = region_center(region);
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.move_mouse(center_x, center_y)
    })?;
    click_left_held_with_emitter(emitter, hold_ms, cancelled, generation)?;
    wait_cancellable_input_delay(cancelled, generation, DOUBLE_CLICK_GAP_MS)?;
    click_left_held_with_emitter(emitter, hold_ms, cancelled, generation)?;
    wait_cancellable_input_delay(cancelled, generation, COPY_AFTER_DOUBLE_CLICK_DELAY_MS)?;

    let mut ctrl = TrackedModifierGuard::new(emitter, cancelled, generation);
    ctrl.press(Key::Control)?;
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.key(Key::Unicode('c'), Direction::Click)
    })?;
    ctrl.release(&Key::Control)
}

/// 发送一次可追踪单键输入，确保按下后始终配对释放。
fn press_key_with_emitter<E: InputEmitter>(
    emitter: &E,
    key: Key,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    let mut pressed = TrackedModifierGuard::new(emitter, cancelled, generation);
    pressed.press(key)?;
    pressed.release(&key)
}

fn press_primary_key_sequence_with_emitter<E: InputEmitter>(
    emitter: &E,
    keys: &[PrimaryKey],
    delay_after_each_ms: u64,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    for primary in keys {
        press_key_with_emitter(emitter, primary_to_key(*primary)?, cancelled, generation)?;
        wait_cancellable_input_delay(cancelled, generation, delay_after_each_ms)?;
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

pub async fn click_region_center_held_cancellable(
    region: crate::morse::types::RegionRect,
    hold_ms: u64,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        click_region_center_held_with_emitter(&emitter, &region, hold_ms, &cancelled, generation)
    })
    .await
}

#[allow(dead_code)]
pub async fn click_screen_point_cancellable(
    x: i32,
    y: i32,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        click_screen_point_with_emitter(&emitter, x, y, &cancelled, generation)
    })
    .await
}

pub async fn click_screen_point_held_cancellable(
    x: i32,
    y: i32,
    hold_ms: u64,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        click_screen_point_held_with_emitter(&emitter, x, y, hold_ms, &cancelled, generation)
    })
    .await
}

#[allow(dead_code)]
pub async fn scroll_region_down_cancellable(
    region: crate::morse::types::RegionRect,
    steps: i32,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        scroll_region_down_with_emitter(&emitter, &region, steps, &cancelled, generation)
    })
    .await
}

#[allow(dead_code)]
pub async fn scroll_region_segments_cancellable(
    region: crate::morse::types::RegionRect,
    segments: Vec<(i32, u32)>,
    step_interval_ms: u64,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        scroll_region_segments_with_emitter(
            &emitter,
            &region,
            &segments,
            step_interval_ms,
            &cancelled,
            generation,
        )
    })
    .await
}

#[allow(dead_code)]
pub async fn double_click_region_and_copy_cancellable(
    region: crate::morse::types::RegionRect,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        double_click_region_and_copy_with_emitter(&emitter, &region, &cancelled, generation)
    })
    .await
}

pub async fn double_click_region_and_copy_held_cancellable(
    region: crate::morse::types::RegionRect,
    hold_ms: u64,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        double_click_region_and_copy_held_with_emitter(
            &emitter, &region, hold_ms, &cancelled, generation,
        )
    })
    .await
}

/// 按下一次命名按键；紧急停止会阻断输入并释放已经按下的按键。
pub async fn press_named_key_cancellable(
    key: crate::hotkey_types::NamedKey,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let key = primary_to_key(crate::hotkey_types::PrimaryKey::Named(key))?;
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        press_key_with_emitter(&emitter, key, &cancelled, generation)
    })
    .await
}

pub async fn press_primary_key_sequence_cancellable(
    keys: Vec<PrimaryKey>,
    delay_after_each_ms: u64,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        press_primary_key_sequence_with_emitter(
            &emitter,
            &keys,
            delay_after_each_ms,
            &cancelled,
            generation,
        )
    })
    .await
}

pub async fn move_region_center_cancellable(
    region: crate::morse::types::RegionRect,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        move_region_center_with_emitter(&emitter, &region, &cancelled, generation)
    })
    .await
}

#[allow(dead_code)]
pub fn release_tracked_injected_inputs() -> Result<(), String> {
    invalidate_cancellable_inputs();
    release_tracked_injected_inputs_with_factory(EnigoInputEmitter::new)
}

fn release_tracked_injected_inputs_with_factory<E, F>(emitter_factory: F) -> Result<(), String>
where
    E: InputEmitter,
    F: FnMut() -> Result<E, String>,
{
    let mut state = lock_input_action_state();
    release_tracked_injected_inputs_bounded(
        &mut state,
        emitter_factory,
        INPUT_RELEASE_MAX_ATTEMPTS,
        INPUT_RELEASE_RETRY_DELAY,
    )
}

fn release_tracked_injected_inputs_bounded<E, F>(
    state: &mut InputActionState,
    mut emitter_factory: F,
    max_attempts: usize,
    retry_delay: Duration,
) -> Result<(), String>
where
    E: InputEmitter,
    F: FnMut() -> Result<E, String>,
{
    if state.tracked_keys.is_empty() && !state.left_mouse_pressed {
        return Ok(());
    }
    let mut last_error = "没有执行输入释放".to_string();
    for attempt in 1..=max_attempts {
        match emitter_factory() {
            Ok(emitter) => {
                let errors = release_tracked_injected_inputs_with(state, &emitter);
                if state.tracked_keys.is_empty() && !state.left_mouse_pressed {
                    return Ok(());
                }
                last_error = errors.join(", ");
            }
            Err(error) => last_error = error,
        }
        if attempt < max_attempts {
            thread::sleep(retry_delay);
        }
    }
    Err(format!(
        "输入释放失败，已尝试 {max_attempts} 次: {last_error}"
    ))
}

fn release_tracked_injected_inputs_with<E: InputEmitter>(
    state: &mut InputActionState,
    emitter: &E,
) -> Vec<String> {
    let mut errors = Vec::new();
    if let Err(error) = release_left_if_tracked_with_emitter(state, emitter) {
        errors.push(error);
    }
    errors.extend(release_tracked_injected_keys_with(state, emitter));
    errors
}

fn release_tracked_injected_keys_with<E: InputEmitter>(
    state: &mut InputActionState,
    emitter: &E,
) -> Vec<String> {
    let keys = std::mem::take(&mut state.tracked_keys);
    let mut errors = Vec::new();
    for key in keys.into_iter().rev() {
        if let Err(error) = emitter.key(key, Direction::Release) {
            errors.push(error);
            track_injected_key(state, key)
        }
    }
    errors
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
    use std::sync::{atomic::AtomicBool, Arc, Mutex as StdMutex};
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
        events: StdMutex<Vec<String>>,
        scrolls: StdMutex<Vec<i32>>,
        pressed_keys: StdMutex<Vec<Key>>,
    }

    impl RecordingEmitter {
        fn events(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }

        fn scrolls(&self) -> Vec<i32> {
            self.scrolls.lock().unwrap().clone()
        }

        fn pressed_keys(&self) -> Vec<Key> {
            self.pressed_keys.lock().unwrap().clone()
        }
    }

    impl InputEmitter for RecordingEmitter {
        fn move_mouse(&self, x: i32, y: i32) -> Result<(), String> {
            *self.action_count.lock().unwrap() += 1;
            self.events.lock().unwrap().push(format!("move:{x}:{y}"));
            Ok(())
        }

        fn click_left(&self) -> Result<(), String> {
            *self.action_count.lock().unwrap() += 1;
            self.events.lock().unwrap().push("click".to_string());
            Ok(())
        }

        fn press_left(&self) -> Result<(), String> {
            *self.action_count.lock().unwrap() += 1;
            self.events.lock().unwrap().push("left:press".to_string());
            Ok(())
        }

        fn release_left(&self) -> Result<(), String> {
            *self.action_count.lock().unwrap() += 1;
            self.events.lock().unwrap().push("left:release".to_string());
            Ok(())
        }

        fn scroll_vertical(&self, length: i32) -> Result<(), String> {
            *self.action_count.lock().unwrap() += 1;
            self.scrolls.lock().unwrap().push(length);
            self.events.lock().unwrap().push(format!("scroll:{length}"));
            Ok(())
        }

        fn key(&self, key: Key, direction: Direction) -> Result<(), String> {
            *self.action_count.lock().unwrap() += 1;
            self.events
                .lock()
                .unwrap()
                .push(format!("key:{key:?}:{direction:?}"));
            match (key, direction) {
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

    struct CancelOnLeftPressEmitter {
        cancelled: Arc<AtomicBool>,
        events: StdMutex<Vec<String>>,
    }

    impl CancelOnLeftPressEmitter {
        fn new(cancelled: Arc<AtomicBool>) -> Self {
            Self {
                cancelled,
                events: StdMutex::new(Vec::new()),
            }
        }

        fn events(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }
    }

    impl InputEmitter for CancelOnLeftPressEmitter {
        fn move_mouse(&self, x: i32, y: i32) -> Result<(), String> {
            self.events.lock().unwrap().push(format!("move:{x}:{y}"));
            Ok(())
        }

        fn click_left(&self) -> Result<(), String> {
            Ok(())
        }

        fn press_left(&self) -> Result<(), String> {
            self.events.lock().unwrap().push("left:press".to_string());
            self.cancelled.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn release_left(&self) -> Result<(), String> {
            self.events.lock().unwrap().push("left:release".to_string());
            Ok(())
        }

        fn key(&self, _key: Key, _direction: Direction) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn move_region_center_only_moves_without_clicking() {
        let emitter = RecordingEmitter::default();
        let cancelled = AtomicBool::new(false);

        move_region_center_with_emitter(
            &emitter,
            &crate::morse::types::RegionRect {
                x: 100,
                y: 200,
                width: 20,
                height: 10,
            },
            &cancelled,
            input_release_generation(),
        )
        .unwrap();

        assert_eq!(emitter.events(), ["move:110:205"]);
    }

    fn rect() -> crate::morse::types::RegionRect {
        crate::morse::types::RegionRect {
            x: 100,
            y: 200,
            width: 80,
            height: 30,
        }
    }

    #[tokio::test]
    async fn held_region_click_moves_then_presses_and_releases_left_button() {
        let _test_guard = lock_input_simulation_tests().await;
        let emitter = RecordingEmitter::default();
        let cancelled = AtomicBool::new(false);

        click_region_center_held_with_emitter(
            &emitter,
            &rect(),
            0,
            &cancelled,
            input_release_generation(),
        )
        .unwrap();

        assert_eq!(
            emitter.events(),
            ["move:140:215", "left:press", "left:release"]
        );
    }

    #[tokio::test]
    async fn held_click_releases_left_button_when_cancelled_during_hold() {
        let _test_guard = lock_input_simulation_tests().await;
        let cancelled = Arc::new(AtomicBool::new(false));
        let emitter = CancelOnLeftPressEmitter::new(Arc::clone(&cancelled));

        let error = click_region_center_held_with_emitter(
            &emitter,
            &rect(),
            100,
            &cancelled,
            input_release_generation(),
        )
        .unwrap_err();

        assert_eq!(error, "输入操作已取消");
        assert_eq!(
            emitter.events(),
            ["move:140:215", "left:press", "left:release"]
        );
    }

    #[test]
    fn emergency_release_releases_tracked_left_button() {
        let mut state = InputActionState {
            tracked_keys: Vec::new(),
            left_mouse_pressed: true,
        };
        let emitter = RecordingEmitter::default();

        let errors = release_tracked_injected_inputs_with(&mut state, &emitter);

        assert!(errors.is_empty());
        assert!(!state.left_mouse_pressed);
        assert_eq!(emitter.events(), ["left:release"]);
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
    async fn account_list_scroll_moves_to_region_before_scrolling_down() {
        let _test_guard = lock_input_simulation_tests().await;
        let emitter = RecordingEmitter::default();
        let cancelled = AtomicBool::new(false);

        scroll_region_down_with_emitter(
            &emitter,
            &rect(),
            3,
            &cancelled,
            input_release_generation(),
        )
        .unwrap();

        assert_eq!(
            emitter.events().first().map(String::as_str),
            Some("move:140:215")
        );
        assert_eq!(emitter.scrolls(), vec![3]);
    }

    #[test]
    fn segmented_scroll_emits_one_wheel_event_per_step() {
        let emitter = RecordingEmitter::default();
        let cancelled = AtomicBool::new(false);

        scroll_region_segments_with_emitter(
            &emitter,
            &rect(),
            &[(-1, 3), (1, 2)],
            0,
            &cancelled,
            input_release_generation(),
        )
        .unwrap();

        assert_eq!(
            emitter.events().first().map(String::as_str),
            Some("move:140:215")
        );
        assert_eq!(emitter.scrolls(), vec![-1, -1, -1, 1, 1]);
    }

    #[test]
    fn primary_key_sequence_clicks_a_then_d() {
        let emitter = RecordingEmitter::default();
        let cancelled = AtomicBool::new(false);

        press_primary_key_sequence_with_emitter(
            &emitter,
            &[PrimaryKey::Letter('A'), PrimaryKey::Letter('D')],
            0,
            &cancelled,
            input_release_generation(),
        )
        .unwrap();

        assert_eq!(
            emitter.events(),
            [
                "key:A:Press",
                "key:A:Release",
                "key:D:Press",
                "key:D:Release"
            ]
        );
    }

    #[tokio::test]
    async fn account_copy_double_clicks_then_sends_ctrl_c_and_releases_ctrl() {
        let _test_guard = lock_input_simulation_tests().await;
        let emitter = RecordingEmitter::default();
        let cancelled = AtomicBool::new(false);

        double_click_region_and_copy_with_emitter(
            &emitter,
            &rect(),
            &cancelled,
            input_release_generation(),
        )
        .unwrap();

        assert_eq!(
            emitter.events(),
            [
                "move:140:215",
                "click",
                "click",
                "key:Control:Press",
                "key:Unicode('c'):Click",
                "key:Control:Release",
            ]
        );
        assert!(emitter.pressed_keys().is_empty());
    }

    #[tokio::test]
    async fn held_account_copy_holds_both_clicks_then_sends_ctrl_c() {
        let _test_guard = lock_input_simulation_tests().await;
        let emitter = RecordingEmitter::default();
        let cancelled = AtomicBool::new(false);

        double_click_region_and_copy_held_with_emitter(
            &emitter,
            &rect(),
            0,
            &cancelled,
            input_release_generation(),
        )
        .unwrap();

        assert_eq!(
            emitter.events(),
            [
                "move:140:215",
                "left:press",
                "left:release",
                "left:press",
                "left:release",
                "key:Control:Press",
                "key:Unicode('c'):Click",
                "key:Control:Release",
            ]
        );
        assert!(emitter.pressed_keys().is_empty());
    }

    #[tokio::test]
    async fn cancellable_single_key_press_releases_key_and_honors_cancellation() {
        let _test_guard = lock_input_simulation_tests().await;
        let emitter = RecordingEmitter::default();
        let active = AtomicBool::new(false);

        press_key_with_emitter(&emitter, Key::Tab, &active, input_release_generation()).unwrap();
        assert_eq!(emitter.events(), ["key:Tab:Press", "key:Tab:Release"]);

        let cancelled_emitter = RecordingEmitter::default();
        let cancelled = AtomicBool::new(true);
        assert!(press_key_with_emitter(
            &cancelled_emitter,
            Key::Space,
            &cancelled,
            input_release_generation(),
        )
        .is_err());
        assert!(cancelled_emitter.events().is_empty());
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

    #[derive(Default)]
    struct FailingReleaseEmitter;

    impl InputEmitter for FailingReleaseEmitter {
        fn move_mouse(&self, _x: i32, _y: i32) -> Result<(), String> {
            Ok(())
        }

        fn click_left(&self) -> Result<(), String> {
            Ok(())
        }

        fn press_left(&self) -> Result<(), String> {
            Ok(())
        }

        fn release_left(&self) -> Result<(), String> {
            Ok(())
        }

        fn scroll_vertical(&self, _length: i32) -> Result<(), String> {
            Ok(())
        }

        fn key(&self, _key: Key, _direction: Direction) -> Result<(), String> {
            Err("模拟释放失败".to_string())
        }
    }

    #[test]
    fn tracked_input_release_stops_after_three_failed_attempts() {
        let mut state = InputActionState {
            tracked_keys: vec![Key::Control],
            left_mouse_pressed: false,
        };
        let attempts = std::sync::atomic::AtomicUsize::new(0);

        let error = release_tracked_injected_inputs_bounded(
            &mut state,
            || {
                attempts.fetch_add(1, Ordering::SeqCst);
                Ok(FailingReleaseEmitter)
            },
            3,
            Duration::ZERO,
        )
        .unwrap_err();

        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(state.tracked_keys, vec![Key::Control]);
        assert!(error.contains("3 次"));
    }
}
