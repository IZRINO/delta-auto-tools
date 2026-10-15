mod types;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::overlay_utils::destroy_window;
use crate::settings::{self, SettingsCoordinator};
pub use types::PracticalToolsSettings;

const SETTINGS_FILE: &str = "practical_tools_settings.json";
const AUTO_INDICATOR_WINDOW: &str = "practical-tools-auto-indicator";

pub struct PracticalToolsState {
    settings: Mutex<PracticalToolsSettings>,
    progress_started_at: Mutex<Option<ProgressRuntime>>,
    selection_target: Mutex<Option<String>>,
    needs_mock_fire: AtomicBool,
    detection_known: AtomicBool,
    force_mock_fire: AtomicBool,
    fire_held: AtomicBool,
    detection_generation: std::sync::atomic::AtomicU64,
    fire_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

#[derive(Clone, Copy)]
struct ProgressRuntime {
    started_at: Instant,
    duration: Duration,
    warning: Duration,
    opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticalToolsBootstrap {
    pub settings: PracticalToolsSettings,
}

impl PracticalToolsState {
    pub fn initialize(app: &AppHandle) -> Result<Self, String> {
        let path = settings::settings_path(app, SETTINGS_FILE)?;
        let stored: PracticalToolsSettings = settings::load_settings(&path)?;
        let settings = stored.normalize()?;
        settings::save_settings(&path, &settings)?;
        Ok(Self {
            settings: Mutex::new(settings),
            progress_started_at: Mutex::new(None),
            selection_target: Mutex::new(None),
            needs_mock_fire: AtomicBool::new(false),
            detection_known: AtomicBool::new(false),
            force_mock_fire: AtomicBool::new(false),
            fire_held: AtomicBool::new(false),
            detection_generation: std::sync::atomic::AtomicU64::new(0),
            fire_cancel: Mutex::new(None),
        })
    }

    pub fn settings_snapshot(&self) -> Result<PracticalToolsSettings, String> {
        self.settings
            .lock()
            .map(|value| value.clone())
            .map_err(|_| "实用小工具状态已损坏".to_string())
    }

    pub fn apply_profile_settings(
        &self,
        app: &AppHandle,
        next: PracticalToolsSettings,
    ) -> Result<(), String> {
        let next = next.normalize()?;
        let path = settings::settings_path(app, SETTINGS_FILE)?;
        settings::save_settings(&path, &next)?;
        *self
            .settings
            .lock()
            .map_err(|_| "实用小工具状态已损坏".to_string())? = next.clone();
        self.force_mock_fire.store(false, Ordering::SeqCst);
        self.detection_known.store(false, Ordering::SeqCst);
        reconcile_progress_window(app, self, &next);
        restart_hotkeys(app, &next)?;
        position_auto_indicator_window(app, &next);
        sync_auto_indicator_window(app);
        Ok(())
    }

    fn is_progress_running(&self) -> bool {
        self.progress_started_at
            .lock()
            .map(|value| value.is_some())
            .unwrap_or(false)
    }
}

#[tauri::command]
pub fn practical_tools_get_bootstrap(
    state: State<'_, PracticalToolsState>,
) -> Result<PracticalToolsBootstrap, String> {
    Ok(PracticalToolsBootstrap {
        settings: state.settings_snapshot()?,
    })
}

#[tauri::command]
pub fn practical_tools_save_settings(
    app: AppHandle,
    state: State<'_, PracticalToolsState>,
    settings_value: PracticalToolsSettings,
    settings_revision: u64,
    coordinator: State<'_, Arc<SettingsCoordinator>>,
) -> Result<PracticalToolsBootstrap, String> {
    let settings_value = settings_value.normalize()?;
    coordinator.with_revision(settings_revision, || {
        let path = settings::settings_path(&app, SETTINGS_FILE)?;
        settings::save_settings(&path, &settings_value)?;
        *state
            .settings
            .lock()
            .map_err(|_| "实用小工具状态已损坏".to_string())? = settings_value.clone();
        state.detection_known.store(false, Ordering::SeqCst);
        state.force_mock_fire.store(false, Ordering::SeqCst);
        restart_hotkeys(&app, &settings_value)?;
        reconcile_progress_window(&app, &state, &settings_value);
        position_auto_indicator_window(&app, &settings_value);
        sync_auto_indicator_window(&app);
        crate::profile::update_active_profile_snapshot(
            &app,
            crate::profile::ActiveProfileSnapshotPatch::PracticalTools(settings_value.clone()),
        )?;
        Ok(PracticalToolsBootstrap {
            settings: settings_value,
        })
    })
}

pub fn stop_registered(app: &AppHandle) -> Result<(), String> {
    if let Some(state) = app.try_state::<PracticalToolsState>() {
        *state
            .progress_started_at
            .lock()
            .map_err(|_| "实用小工具状态已损坏".to_string())? = None;
        state.needs_mock_fire.store(false, Ordering::SeqCst);
        state.detection_known.store(false, Ordering::SeqCst);
        state.force_mock_fire.store(false, Ordering::SeqCst);
        state.fire_held.store(false, Ordering::SeqCst);
        stop_fire_session(&state);
    }
    sync_auto_indicator_window(app);
    hide_progress_window(app);
    if let Some(hotkeys) = app.try_state::<crate::hotkeys::HotkeyManager>() {
        let _ = hotkeys.clear_hold_scope("practical_tools");
    }
    Ok(())
}

fn stop_fire_session(state: &PracticalToolsState) {
    if let Ok(mut session) = state.fire_cancel.lock() {
        if let Some(cancel) = session.take() {
            cancel.store(true, Ordering::SeqCst);
        }
    }
}

fn start_fire_session(
    app: &AppHandle,
    state: &PracticalToolsState,
    fire_key: String,
    start_delay_ms: u64,
    press_ms: u64,
    interval_ms: u64,
) {
    let needs_mock_fire = state.needs_mock_fire.load(Ordering::SeqCst);
    let fire_held = state.fire_held.load(Ordering::SeqCst);
    let globally_enabled = global_enabled(app);
    let force_mock_fire = state.force_mock_fire.load(Ordering::SeqCst);
    if !should_run_fire_session(
        needs_mock_fire,
        force_mock_fire,
        fire_held,
        globally_enabled,
    ) {
        crate::log_info!(
            "practical_tools",
            "跳过模拟连发",
            "needs_mock_fire" => needs_mock_fire,
            "force_mock_fire" => force_mock_fire,
            "fire_held" => fire_held,
            "global_enabled" => globally_enabled
        );
        return;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let Ok(mut current) = state.fire_cancel.lock() else {
        return;
    };
    if !should_run_fire_session(
        state.needs_mock_fire.load(Ordering::SeqCst),
        state.force_mock_fire.load(Ordering::SeqCst),
        state.fire_held.load(Ordering::SeqCst),
        global_enabled(app),
    ) {
        return;
    }
    if current.is_some() {
        crate::log_info!("practical_tools", "模拟连发已在运行");
        return;
    }
    *current = Some(Arc::clone(&cancel));
    drop(current);
    crate::log_info!(
        "practical_tools",
        "模拟连发已启动",
        "fire_key" => fire_key,
        "start_delay_ms" => start_delay_ms,
        "press_ms" => press_ms,
        "interval_ms" => interval_ms
    );
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(start_delay_ms)).await;
        while !cancel.load(Ordering::SeqCst) && global_enabled(&app) {
            let result = if fire_key.starts_with("Mouse") {
                crate::input_simulation::tap_mouse_button_for_cancellable(
                    fire_key.clone(),
                    press_ms,
                    Arc::clone(&cancel),
                )
                .await
            } else {
                match crate::hotkey_types::HotkeyBinding::parse(&fire_key) {
                    Ok(binding) => {
                        crate::input_simulation::tap_primary_key_for_cancellable(
                            binding.primary,
                            press_ms,
                            Arc::clone(&cancel),
                        )
                        .await
                    }
                    Err(error) => Err(error),
                }
            };
            if let Err(error) = result {
                crate::log_warn!("practical_tools", "自动全自动输入失败", "error" => error);
                break;
            }
            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        }
        if let Some(state) = app.try_state::<PracticalToolsState>() {
            if let Ok(mut current) = state.fire_cancel.lock() {
                if current
                    .as_ref()
                    .is_some_and(|active| Arc::ptr_eq(active, &cancel))
                {
                    *current = None;
                }
            }
        }
    });
}

pub(crate) fn restart_hotkeys(
    app: &AppHandle,
    settings: &PracticalToolsSettings,
) -> Result<(), String> {
    let Some(manager) = app.try_state::<crate::hotkeys::HotkeyManager>() else {
        return Ok(());
    };
    if let Some(state) = app.try_state::<PracticalToolsState>() {
        state.fire_held.store(false, Ordering::SeqCst);
        state.needs_mock_fire.store(false, Ordering::SeqCst);
        state.detection_known.store(false, Ordering::SeqCst);
        state.detection_generation.fetch_add(1, Ordering::SeqCst);
        stop_fire_session(&state);
    }
    manager.clear_hold_scope("practical_tools")?;
    if !settings.enabled || !settings.auto_full_auto_enabled {
        if let Some(state) = app.try_state::<PracticalToolsState>() {
            state.force_mock_fire.store(false, Ordering::SeqCst);
            stop_fire_session(&state);
            hide_progress_window_if_idle(app, &state);
        }
        sync_auto_indicator_window(app);
        return Ok(());
    }
    let mut keys = HashMap::<String, (bool, bool, bool)>::new();
    for key in &settings.weapon_keys {
        if !key.is_empty() {
            keys.entry(key.clone()).or_default().0 = true;
        }
    }
    if !settings.fire_key.starts_with("Mouse") {
        keys.entry(settings.fire_key.clone()).or_default().1 = true;
    }
    keys.entry(settings.auto_full_auto_toggle_key.clone())
        .or_default()
        .2 = true;
    let mut bindings = Vec::new();
    for (key, (weapon, fire, toggle)) in keys {
        let registered_key = key.clone();
        let fire_key = settings.fire_key.clone();
        let start_delay_ms = settings.start_delay_ms;
        let weapon_detection_delay_ms = settings.weapon_detection_delay_ms;
        let press_ms = settings.press_ms;
        let interval_ms = settings.interval_ms;
        let area = settings.auto_region.clone();
        let threshold = settings.match_threshold;
        bindings.push((
            key,
            Arc::new(move |app: AppHandle, action| {
                crate::log_info!(
                    "practical_tools",
                    "收到键盘热键事件",
                    "key" => registered_key,
                    "action" => format!("{action:?}"),
                    "weapon_key" => weapon,
                    "fire_key" => fire
                );
                if weapon && action == crate::hotkeys::HoldAction::Up {
                    let detection_app = app.clone();
                    let area = area.clone();
                    let detection_fire_key = fire_key.clone();
                    let generation =
                        detection_app
                            .try_state::<PracticalToolsState>()
                            .map(|state| {
                                state.detection_known.store(false, Ordering::SeqCst);
                                state
                                    .detection_generation
                                    .fetch_add(1, Ordering::SeqCst)
                                    .wrapping_add(1)
                            });
                    sync_auto_indicator_window(&detection_app);
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(weapon_detection_delay_ms)).await;
                        let reference = crate::practical_tools::load_templates()
                            .ok()
                            .map(|images| images[0].clone());
                        let mut similarities = Vec::with_capacity(3);
                        if let (Some(area), Some(reference)) = (area, reference) {
                            for sample in 0..3 {
                                if sample > 0 {
                                    tokio::time::sleep(Duration::from_millis(60)).await;
                                }
                                let area = area.clone();
                                let reference = reference.clone();
                                if let Ok(Some(similarity)) = tokio::task::spawn_blocking(move || {
                                    crate::recognition::watcher::capture_region(&area)
                                        .map(|frame| best_auto_icon_similarity(&frame, &reference))
                                })
                                .await
                                {
                                    similarities.push(similarity);
                                }
                            }
                        }
                        let detected = full_auto_icon_detected(&similarities, threshold);
                        crate::log_info!(
                            "practical_tools",
                            "切枪识图完成",
                            "similarities" => format!("{similarities:?}"),
                            "threshold" => threshold,
                            "detected_full_auto" => detected,
                            "needs_mock_fire" => !detected
                        );
                        if let Some(state) = detection_app.try_state::<PracticalToolsState>() {
                            if generation.is_some_and(|generation| {
                                state.detection_generation.load(Ordering::SeqCst) != generation
                            }) {
                                return;
                            }
                            state.needs_mock_fire.store(!detected, Ordering::SeqCst);
                            state.detection_known.store(true, Ordering::SeqCst);
                            sync_auto_indicator_window(&detection_app);
                            let fire_held = state.fire_held.load(Ordering::SeqCst);
                            let globally_enabled = global_enabled(&detection_app);
                            crate::log_info!(
                                "practical_tools",
                                "应用射击模式判定",
                                "fire_held" => fire_held,
                                "global_enabled" => globally_enabled
                            );
                            let force_mock_fire =
                                state.force_mock_fire.load(Ordering::SeqCst);
                            if should_run_fire_session(
                                !detected,
                                force_mock_fire,
                                fire_held,
                                globally_enabled,
                            ) {
                                start_fire_session(
                                    &detection_app,
                                    &state,
                                    detection_fire_key,
                                    start_delay_ms,
                                    press_ms,
                                    interval_ms,
                                );
                            } else if detected {
                                stop_fire_session(&state);
                            }
                        }
                    });
                }
                if toggle && action == crate::hotkeys::HoldAction::Down {
                    if let Some(state) = app.try_state::<PracticalToolsState>() {
                        let enabled = !state.force_mock_fire.fetch_xor(true, Ordering::SeqCst);
                        crate::log_info!(
                            "practical_tools",
                            "手动切换自动全自动",
                            "force_mock_fire" => enabled,
                            "key" => registered_key
                        );
                        sync_auto_indicator_window(&app);
                        let fire_held = state.fire_held.load(Ordering::SeqCst);
                        if should_run_fire_session(
                            state.needs_mock_fire.load(Ordering::SeqCst),
                            enabled,
                            fire_held,
                            global_enabled(&app),
                        ) {
                            start_fire_session(
                                &app,
                                &state,
                                fire_key.clone(),
                                start_delay_ms,
                                press_ms,
                                interval_ms,
                            );
                        } else {
                            stop_fire_session(&state);
                        }
                    }
                }
                if fire {
                    if let Some(state) = app.try_state::<PracticalToolsState>() {
                        if action == crate::hotkeys::HoldAction::Down {
                            state.fire_held.store(true, Ordering::SeqCst);
                            crate::log_info!("practical_tools", "开火键按下", "fire_key" => fire_key);
                            start_fire_session(
                                &app,
                                &state,
                                fire_key.clone(),
                                start_delay_ms,
                                press_ms,
                                interval_ms,
                            );
                        } else {
                            state.fire_held.store(false, Ordering::SeqCst);
                            crate::log_info!("practical_tools", "开火键松开", "fire_key" => fire_key);
                            stop_fire_session(&state);
                        }
                    }
                }
            }) as crate::hotkeys::HoldActionCallback,
        ));
    }
    manager.replace_hold_scope(
        "practical_tools",
        bindings,
        "实用小工具".to_string(),
        crate::hotkeys::ConflictPolicy::AllowHold,
    )?;
    Ok(())
}

const PROGRESS_WINDOW: &str = "practical-tools-bomb-progress";
const REGION_WINDOW: &str = "practical-tools-region-selection";
const PROGRESS_EVENT: &str = "practical-tools://bomb-progress";
const AUTO_INDICATOR_URL: &str = "index.html?mode=practical-tools-auto-indicator";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BombProgress {
    remaining_ms: u64,
    duration_ms: u64,
    warning_ms: u64,
    opacity: f32,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticalToolsDisplayState {
    progress: Option<BombProgress>,
    auto_full_auto_forced: bool,
    auto_full_auto_active: bool,
    auto_full_auto_simulating: bool,
    auto_indicator_opacity: f32,
}

fn hide_progress_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PROGRESS_WINDOW) {
        let _ = window.hide();
    }
}

fn auto_full_auto_active(state: &PracticalToolsState) -> bool {
    auto_full_auto_mode_active(
        state.detection_known.load(Ordering::SeqCst),
        state.needs_mock_fire.load(Ordering::SeqCst),
        state.force_mock_fire.load(Ordering::SeqCst),
    )
}

fn auto_full_auto_mode_active(detection_known: bool, needs_mock_fire: bool, forced: bool) -> bool {
    forced || (detection_known && needs_mock_fire)
}

fn sync_auto_indicator_window(app: &AppHandle) {
    let Some(state) = app.try_state::<PracticalToolsState>() else {
        return;
    };
    let settings = state.settings_snapshot().ok();
    let active = settings.as_ref().is_some_and(|settings| {
        settings.enabled
            && settings.auto_full_auto_enabled
            && global_enabled(app)
            && auto_full_auto_active(&state)
    });
    let Some(window) = app.get_webview_window(AUTO_INDICATOR_WINDOW) else {
        if active {
            if let Some(settings) = settings.as_ref() {
                if let Err(error) = create_auto_indicator_window(app, settings) {
                    crate::log_warn!(
                        "practical_tools",
                        "创建自动全自动提示失败",
                        "error" => error
                    );
                }
            }
        }
        return;
    };
    if active {
        if !window.is_visible().unwrap_or(false) {
            let _ = window.show();
        }
    } else if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    }
}

fn auto_indicator_position(settings: &PracticalToolsSettings) -> (i32, i32) {
    let (screen_x, screen_y, screen_width, screen_height) = xcap::Monitor::all()
        .ok()
        .and_then(|monitors| monitors.into_iter().next())
        .map(|monitor| {
            (
                monitor.x().unwrap_or(0),
                monitor.y().unwrap_or(0),
                monitor.width().unwrap_or(1920),
                monitor.height().unwrap_or(1080),
            )
        })
        .unwrap_or((0, 0, 1920, 1080));
    let width = 360_u32;
    let height = 64_u32;
    (
        screen_x + screen_width as i32 - width as i32 - settings.auto_indicator_right as i32,
        screen_y + screen_height as i32 - height as i32 - settings.auto_indicator_bottom as i32,
    )
}

fn position_auto_indicator_window(app: &AppHandle, settings: &PracticalToolsSettings) {
    if let Some(window) = app.get_webview_window(AUTO_INDICATOR_WINDOW) {
        let (x, y) = auto_indicator_position(settings);
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

fn create_auto_indicator_window(
    app: &AppHandle,
    settings: &PracticalToolsSettings,
) -> Result<(), String> {
    let (x, y) = auto_indicator_position(settings);
    let width = 360_u32;
    let height = 64_u32;
    WebviewWindowBuilder::new(
        app,
        AUTO_INDICATOR_WINDOW,
        WebviewUrl::App(AUTO_INDICATOR_URL.into()),
    )
    .title("自动全自动模式")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(true)
    .resizable(false)
    .inner_size(width as f64, height as f64)
    .position(x as f64, y as f64)
    .build()
    .map_err(|error| format!("创建自动全自动提示窗口失败: {error}"))?;
    if let Some(window) = app.get_webview_window(AUTO_INDICATOR_WINDOW) {
        let _ = window.set_ignore_cursor_events(true);
    }
    Ok(())
}

fn hide_progress_window_if_idle(app: &AppHandle, state: &PracticalToolsState) {
    if !state.is_progress_running() {
        hide_progress_window(app);
    }
}

fn reconcile_progress_window(
    app: &AppHandle,
    state: &PracticalToolsState,
    settings: &PracticalToolsSettings,
) {
    if !settings.enabled {
        state.force_mock_fire.store(false, Ordering::SeqCst);
        if let Ok(mut progress) = state.progress_started_at.lock() {
            *progress = None;
        }
        hide_progress_window(app);
        return;
    }
    if !settings.bomb_progress_enabled {
        if let Ok(mut progress) = state.progress_started_at.lock() {
            *progress = None;
        }
    }
    if state.is_progress_running() {
        if let Some(window) = app.get_webview_window(PROGRESS_WINDOW) {
            let _ = window.set_size(PhysicalSize::new(
                settings.overlay_width,
                settings.overlay_height,
            ));
            let _ = window.set_position(PhysicalPosition::new(
                settings.overlay_x,
                settings.overlay_y,
            ));
            let _ = window.show();
        }
    } else {
        hide_progress_window(app);
    }
}

fn ensure_progress_window(
    app: &AppHandle,
    settings: &PracticalToolsSettings,
) -> Result<(), String> {
    let window = if let Some(window) = app.get_webview_window(PROGRESS_WINDOW) {
        window
    } else {
        WebviewWindowBuilder::new(
            app,
            PROGRESS_WINDOW,
            WebviewUrl::App("index.html?mode=practical-tools-display".into()),
        )
        .title("实用小工具提示")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .resizable(false)
        .inner_size(
            settings.overlay_width as f64,
            settings.overlay_height as f64,
        )
        .position(settings.overlay_x as f64, settings.overlay_y as f64)
        .build()
        .map_err(|error| format!("创建爆破进度窗口失败: {error}"))?
    };
    let _ = window.set_size(PhysicalSize::new(
        settings.overlay_width,
        settings.overlay_height,
    ));
    let _ = window.set_position(PhysicalPosition::new(
        settings.overlay_x,
        settings.overlay_y,
    ));
    let _ = window.set_ignore_cursor_events(true);
    let _ = window.show();
    Ok(())
}

fn start_progress(
    app: &AppHandle,
    state: &PracticalToolsState,
    preview: bool,
) -> Result<bool, String> {
    let settings = state.settings_snapshot()?;
    if !preview && (!settings.enabled || !settings.bomb_progress_enabled || !global_enabled(app)) {
        return Ok(false);
    }
    let mut started = state
        .progress_started_at
        .lock()
        .map_err(|_| "实用小工具状态已损坏".to_string())?;
    if started.is_some() {
        return Ok(false);
    }
    ensure_progress_window(app, &settings)?;
    let runtime = ProgressRuntime {
        started_at: Instant::now(),
        duration: Duration::from_secs(settings.bomb_duration_seconds as u64),
        warning: Duration::from_secs(settings.warning_seconds as u64),
        opacity: settings.overlay_opacity,
    };
    *started = Some(runtime);
    drop(started);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let elapsed = runtime.started_at.elapsed();
            if elapsed >= runtime.duration {
                break;
            }
            let _ = app.emit_to(
                PROGRESS_WINDOW,
                PROGRESS_EVENT,
                BombProgress {
                    remaining_ms: runtime.duration.saturating_sub(elapsed).as_millis() as u64,
                    duration_ms: runtime.duration.as_millis() as u64,
                    warning_ms: runtime.warning.as_millis() as u64,
                    opacity: runtime.opacity,
                },
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if let Some(state) = app.try_state::<PracticalToolsState>() {
            if let Ok(mut active) = state.progress_started_at.lock() {
                *active = None;
            }
            hide_progress_window_if_idle(&app, &state);
        } else {
            hide_progress_window(&app);
        }
    });
    Ok(true)
}

#[tauri::command]
pub async fn practical_tools_start_bomb_progress(
    app: AppHandle,
    state: State<'_, PracticalToolsState>,
) -> Result<bool, String> {
    start_progress(&app, &state, false)
}

#[tauri::command]
pub async fn practical_tools_preview_bomb_progress(
    app: AppHandle,
    state: State<'_, PracticalToolsState>,
) -> Result<bool, String> {
    start_progress(&app, &state, true)
}

#[tauri::command]
pub fn practical_tools_get_progress(
    state: State<'_, PracticalToolsState>,
) -> Result<PracticalToolsDisplayState, String> {
    let started = state
        .progress_started_at
        .lock()
        .map_err(|_| "实用小工具状态已损坏".to_string())?;
    let progress = started.and_then(|runtime| {
        let elapsed = runtime.started_at.elapsed();
        (elapsed < runtime.duration).then(|| BombProgress {
            remaining_ms: runtime.duration.saturating_sub(elapsed).as_millis() as u64,
            duration_ms: runtime.duration.as_millis() as u64,
            warning_ms: runtime.warning.as_millis() as u64,
            opacity: runtime.opacity,
        })
    });
    Ok(PracticalToolsDisplayState {
        progress,
        auto_full_auto_forced: state.force_mock_fire.load(Ordering::SeqCst),
        auto_full_auto_active: auto_full_auto_active(&state),
        auto_full_auto_simulating: state.needs_mock_fire.load(Ordering::SeqCst),
        auto_indicator_opacity: state.settings_snapshot()?.auto_indicator_opacity,
    })
}

#[tauri::command]
pub async fn practical_tools_begin_region_selection(
    app: AppHandle,
    state: State<'_, PracticalToolsState>,
    target: String,
) -> Result<(), String> {
    if !matches!(target.as_str(), "auto" | "install" | "bomb") {
        return Err("识别区域类型无效".into());
    }
    *state
        .selection_target
        .lock()
        .map_err(|_| "实用小工具状态已损坏".to_string())? = Some(target.clone());
    if app.get_webview_window(REGION_WINDOW).is_some() {
        destroy_window(&app, REGION_WINDOW);
    }
    let url = format!(
        "index.html?mode=practical-tools-region&target={}",
        crate::overlay_utils::encoded_query_value(&target)
    );
    let (screen_x, screen_y, screen_w, screen_h) = xcap::Monitor::all()
        .ok()
        .and_then(|monitors| monitors.into_iter().next())
        .map(|monitor| {
            let x = monitor.x().unwrap_or(0);
            let y = monitor.y().unwrap_or(0);
            let w = monitor.width().unwrap_or(1920);
            let h = monitor.height().unwrap_or(1080);
            (x, y, w, h)
        })
        .unwrap_or((0, 0, 1920, 1080));
    let window = WebviewWindowBuilder::new(&app, REGION_WINDOW, WebviewUrl::App(url.into()))
        .title("实用小工具识别区域")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .visible(true)
        .resizable(false)
        .inner_size(screen_w as f64, screen_h as f64)
        .position(screen_x as f64, screen_y as f64)
        .build()
        .map_err(|error| format!("创建识别区域窗口失败: {error}"))?;
    let _ = window.maximize();
    let _ = window.set_ignore_cursor_events(false);
    let _ = window.show();
    let _ = window.set_focus();
    Ok(())
}

#[tauri::command]
pub async fn practical_tools_submit_region_selection(
    app: AppHandle,
    state: State<'_, PracticalToolsState>,
    target: String,
    region: crate::morse::types::RegionRect,
    settings_revision: u64,
    coordinator: State<'_, Arc<SettingsCoordinator>>,
) -> Result<(), String> {
    if region.width <= 2 || region.height <= 2 {
        return Err("识别区域过小".into());
    }
    coordinator.with_revision(settings_revision, || {
        let pending = state
            .selection_target
            .lock()
            .map_err(|_| "实用小工具状态已损坏".to_string())?
            .clone();
        if pending.as_deref() != Some(target.as_str()) {
            return Err("当前识别区域选择已失效".into());
        }
        let mut settings = state.settings_snapshot()?;
        match target.as_str() {
            "auto" => settings.auto_region = Some(region),
            "install" => settings.install_region = Some(region),
            "bomb" => settings.bomb_icon_region = Some(region),
            _ => return Err("识别区域类型无效".into()),
        }
        settings = settings.normalize()?;
        let path = settings::settings_path(&app, SETTINGS_FILE)?;
        settings::save_settings(&path, &settings)?;
        *state
            .settings
            .lock()
            .map_err(|_| "实用小工具状态已损坏".to_string())? = settings.clone();
        crate::profile::update_active_profile_snapshot(
            &app,
            crate::profile::ActiveProfileSnapshotPatch::PracticalTools(settings.clone()),
        )?;
        app.emit_to(
            "main",
            "practical-tools://region-selected",
            PracticalToolsBootstrap { settings },
        )
        .map_err(|error| format!("通知识别区域更新失败: {error}"))?;
        if app.get_webview_window(REGION_WINDOW).is_some() {
            destroy_window(&app, REGION_WINDOW);
        }
        *state
            .selection_target
            .lock()
            .map_err(|_| "实用小工具状态已损坏".to_string())? = None;
        Ok(())
    })
}

#[tauri::command]
pub async fn practical_tools_cancel_region_selection(
    app: AppHandle,
    state: State<'_, PracticalToolsState>,
) -> Result<(), String> {
    if app.get_webview_window(REGION_WINDOW).is_some() {
        destroy_window(&app, REGION_WINDOW);
    }
    *state
        .selection_target
        .lock()
        .map_err(|_| "实用小工具状态已损坏".to_string())? = None;
    Ok(())
}

pub fn start_runtime(app: &AppHandle) {
    if let Some(state) = app.try_state::<PracticalToolsState>() {
        if let Ok(settings) = state.settings_snapshot() {
            let _ = restart_hotkeys(app, &settings);
        }
    }
    let indicator_app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            sync_auto_indicator_window(&indicator_app);
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
    #[cfg(target_os = "windows")]
    {
        let app = app.clone();
        std::thread::spawn(move || {
            use willhook::event::{
                InputEvent, IsEventInjected, MouseButton, MouseButtonPress, MouseEventType,
            };
            let Some(hook) = willhook::mouse_hook() else {
                return;
            };
            loop {
                let Ok(event) = hook.try_recv() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };
                let InputEvent::Mouse(event) = event else {
                    continue;
                };
                if event.is_injected != Some(IsEventInjected::NotInjected) {
                    continue;
                }
                let MouseEventType::Press(press) = event.event else {
                    continue;
                };
                let button = match press.button {
                    MouseButton::Left(_) => "MouseLeft",
                    MouseButton::Right(_) => "MouseRight",
                    MouseButton::Middle(_) => "MouseMiddle",
                    MouseButton::X1(_) => "MouseBack",
                    MouseButton::X2(_) => "MouseForward",
                    _ => continue,
                };
                let Some(state) = app.try_state::<PracticalToolsState>() else {
                    continue;
                };
                let Ok(settings) = state.settings_snapshot() else {
                    continue;
                };
                if !settings.enabled
                    || !settings.auto_full_auto_enabled
                    || settings.fire_key != button
                {
                    continue;
                }
                match press.pressed {
                    MouseButtonPress::Down => {
                        state.fire_held.store(true, Ordering::SeqCst);
                        crate::log_info!("practical_tools", "鼠标开火键按下", "fire_key" => button);
                        start_fire_session(
                            &app,
                            &state,
                            settings.fire_key,
                            settings.start_delay_ms,
                            settings.press_ms,
                            settings.interval_ms,
                        );
                    }
                    MouseButtonPress::Up => {
                        state.fire_held.store(false, Ordering::SeqCst);
                        crate::log_info!("practical_tools", "鼠标开火键松开", "fire_key" => button);
                        stop_fire_session(&state);
                    }
                    _ => {}
                }
            }
        });
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let templates = match load_templates() {
            Ok(templates) => templates,
            Err(error) => {
                crate::log_warn!("practical_tools", "加载识别模板失败", "error" => error);
                return;
            }
        };
        let mut armed = true;
        let mut absent_since: Option<Instant> = None;
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let Some(state) = app.try_state::<PracticalToolsState>() else {
                continue;
            };
            let Ok(settings) = state.settings_snapshot() else {
                continue;
            };
            if !settings.enabled
                || !settings.bomb_progress_enabled
                || !global_enabled(&app)
                || state.is_progress_running()
            {
                continue;
            }
            let mut matched_any = false;
            for (index, region) in [
                settings.install_region.as_ref(),
                settings.bomb_icon_region.as_ref(),
            ]
            .into_iter()
            .enumerate()
            {
                let Some(region) = region else {
                    continue;
                };
                let Some(frame) = crate::recognition::watcher::capture_region(region) else {
                    continue;
                };
                let matched = if index == 0 {
                    crate::recognition::watcher::matching::compare_images(&frame, &templates[1])
                        .similarity
                        >= settings.match_threshold
                } else {
                    bomb_color_detected(
                        &frame,
                        &settings.bomb_target_color,
                        settings.bomb_color_tolerance,
                    )
                };
                if matched {
                    matched_any = true;
                    if armed {
                        if let Err(error) = start_progress(&app, &state, false) {
                            crate::log_warn!("practical_tools", "启动爆破进度失败", "error" => error);
                        }
                        armed = false;
                    }
                    break;
                }
            }
            if !armed {
                if matched_any {
                    absent_since = None;
                } else {
                    let since = absent_since.get_or_insert_with(Instant::now);
                    if since.elapsed() >= Duration::from_secs(4) {
                        armed = true;
                        absent_since = None;
                    }
                }
            }
        }
    });
}

fn should_run_fire_session(
    needs_mock_fire: bool,
    force_mock_fire: bool,
    fire_held: bool,
    global_enabled: bool,
) -> bool {
    (needs_mock_fire || force_mock_fire) && fire_held && global_enabled
}

fn best_auto_icon_similarity(frame: &image::DynamicImage, reference: &image::DynamicImage) -> f32 {
    [0.9_f32, 0.95, 1.0, 1.05, 1.1]
        .into_iter()
        .map(|scale| {
            if scale == 1.0 {
                crate::recognition::watcher::matching::compare_images(frame, reference).similarity
            } else {
                let width = (reference.width() as f32 * scale).round().max(1.0) as u32;
                let height = (reference.height() as f32 * scale).round().max(1.0) as u32;
                let scaled =
                    reference.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
                crate::recognition::watcher::matching::compare_images(frame, &scaled).similarity
            }
        })
        .fold(0.0_f32, f32::max)
}

fn parse_hex_color(value: &str) -> [u8; 3] {
    [1, 3, 5].map(|start| u8::from_str_radix(&value[start..start + 2], 16).unwrap_or(0))
}

fn bomb_color_detected(frame: &image::DynamicImage, target: &str, tolerance: u8) -> bool {
    crate::recognition::watcher::matching::scan_region_for_color(
        frame,
        parse_hex_color(target),
        tolerance as f32,
        true,
    )
    .matching_count
        > 0
}

fn full_auto_icon_detected(similarities: &[f32], threshold: f32) -> bool {
    similarities
        .iter()
        .filter(|similarity| **similarity >= threshold)
        .count()
        >= 2
}

fn global_enabled(app: &AppHandle) -> bool {
    app.try_state::<crate::global_state::GlobalState>()
        .is_some_and(|state| state.enabled())
}

pub fn load_templates() -> Result<[image::DynamicImage; 2], String> {
    let auto = image::load_from_memory(include_bytes!("auto_icon.png"))
        .map_err(|error| format!("自动开火图模板无效: {error}"))?;
    let install = image::load_from_memory(include_bytes!("install_notice.png"))
        .map_err(|error| format!("安装提示模板无效: {error}"))?;
    Ok([auto, install])
}

#[cfg(test)]
mod tests {
    use super::{
        auto_full_auto_mode_active, best_auto_icon_similarity, bomb_color_detected,
        full_auto_icon_detected, should_run_fire_session,
    };

    #[test]
    fn indicator_is_active_only_for_simulated_or_forced_mode() {
        assert!(auto_full_auto_mode_active(true, true, false));
        assert!(auto_full_auto_mode_active(false, false, true));
        assert!(!auto_full_auto_mode_active(false, true, false));
        assert!(!auto_full_auto_mode_active(true, false, false));
    }

    #[test]
    fn full_auto_detection_requires_two_matching_samples() {
        assert!(full_auto_icon_detected(&[0.81, 0.78, 0.2], 0.75));
        assert!(!full_auto_icon_detected(&[0.81, 0.2, 0.2], 0.75));
        assert!(!full_auto_icon_detected(&[0.81], 0.75));
    }

    #[test]
    fn bomb_color_detection_matches_yellow_with_tolerance() {
        let frame = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            8,
            8,
            image::Rgb([246, 194, 8]),
        ));
        assert!(bomb_color_detected(&frame, "#F5C400", 55));
        assert!(!bomb_color_detected(&frame, "#00AAFF", 55));
    }

    #[test]
    fn full_auto_icon_matching_tolerates_small_scale_changes() {
        let mut icon = image::RgbaImage::from_pixel(20, 16, image::Rgba([0, 0, 0, 0]));
        for y in 3..13 {
            for x in 2..18 {
                if (x + y) % 3 != 0 {
                    icon.put_pixel(x, y, image::Rgba([230, 230, 230, 255]));
                }
            }
        }
        let reference = image::DynamicImage::ImageRgba8(icon);
        let scaled = reference.resize_exact(19, 15, image::imageops::FilterType::Lanczos3);
        let frame = image::DynamicImage::ImageRgba8({
            let mut frame = image::RgbaImage::from_pixel(32, 24, image::Rgba([15, 20, 25, 255]));
            image::imageops::overlay(&mut frame, &scaled.to_rgba8(), 6, 4);
            frame
        });

        assert!(best_auto_icon_similarity(&frame, &reference) >= 0.75);
    }

    #[test]
    fn detection_result_starts_mock_fire_if_fire_key_is_already_held() {
        assert!(should_run_fire_session(true, false, true, true));
        assert!(should_run_fire_session(false, true, true, true));
        assert!(!should_run_fire_session(false, false, true, true));
        assert!(!should_run_fire_session(true, false, false, true));
        assert!(!should_run_fire_session(true, true, true, false));
    }
}
