use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};
use tokio::{
    sync::oneshot,
    time::{self, Duration},
};

mod settings;
mod types;

pub use self::types::{
    RapidfireBootstrap, RapidfireCard, RapidfireRect, RapidfireRunState, RapidfireRunStatus,
    RapidfireSelectionKind, RapidfireSelectionOutcome, RapidfireSettings,
};

use crate::hotkeys::{HoldAction, HoldActionCallback, HotkeyManager};

const RAPIDFIRE_DISPLAY_LABEL: &str = "rapidfire-display";
const RAPIDFIRE_POSITION_LABEL: &str = "rapidfire-position";
const RAPIDFIRE_DISPLAY_MIN_HEIGHT: i32 = 80;
const RAPIDFIRE_DISPLAY_MIN_WIDTH: i32 = 320;
const RAPIDFIRE_DISPLAY_MAX_WIDTH: i32 = 800;
const RAPIDFIRE_MIN_INTERVAL_MS: u64 = 10;

// ---- State ----

pub struct RapidfireState {
    inner: Mutex<RapidfireStateInner>,
}

struct RapidfireStateInner {
    settings: RapidfireSettings,
    runs: HashMap<String, CardRuntime>,
    pending_position: Option<PendingRapidfirePosition>,
    hotkey_error: Option<String>,
}

struct CardRuntime {
    count: u64,
    status: RapidfireRunStatus,
    abort_tx: Option<oneshot::Sender<()>>,
}

struct PendingRapidfirePosition {
    original_position: RapidfireRect,
    staged_position: RapidfireRect,
    sender: oneshot::Sender<RapidfireSelectionKind>,
}

impl RapidfireStateInner {
    fn bootstrap(&self) -> RapidfireBootstrap {
        RapidfireBootstrap {
            settings: self.settings.clone(),
            runs: self.run_states(),
            hotkey_error: self.hotkey_error.clone(),
        }
    }

    fn run_states(&self) -> Vec<RapidfireRunState> {
        self.settings
            .cards
            .iter()
            .map(|card| {
                let run = self.runs.get(&card.id);
                RapidfireRunState {
                    card_id: card.id.clone(),
                    status: run.map(|r| r.status.clone()).unwrap_or(RapidfireRunStatus::Idle),
                    count: run.map(|r| r.count).unwrap_or(0),
                }
            })
            .collect()
    }
}

// ---- Key mapping ----

/// 将目标键字符串映射为 enigo Key 并触发一次点击
async fn fire_target_key(target_key: &str) -> Result<(), String> {
    use enigo::{Direction, Enigo, Keyboard, Settings};

    let enigo_key = match parse_target_key(target_key) {
        Some(k) => k,
        None => return Err(format!("不支持的目标键: {target_key}")),
    };

    let key_str = target_key.to_string();
    tokio::task::spawn_blocking(move || {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化连发输入失败: {error}"))?;
        enigo
            .key(enigo_key, Direction::Click)
            .map_err(|error| format!("连发触发键 {key_str} 失败: {error}"))
    })
    .await
    .map_err(|error| format!("连发任务执行失败: {error}"))?
}

fn parse_target_key(key: &str) -> Option<enigo::Key> {
    use enigo::Key;
    let upper = key.trim().to_uppercase();
    match upper.as_str() {
        "A" => Some(Key::Unicode('a')),
        "B" => Some(Key::Unicode('b')),
        "C" => Some(Key::Unicode('c')),
        "D" => Some(Key::Unicode('d')),
        "E" => Some(Key::Unicode('e')),
        "F" => Some(Key::Unicode('f')),
        "G" => Some(Key::Unicode('g')),
        "H" => Some(Key::Unicode('h')),
        "I" => Some(Key::Unicode('i')),
        "J" => Some(Key::Unicode('j')),
        "K" => Some(Key::Unicode('k')),
        "L" => Some(Key::Unicode('l')),
        "M" => Some(Key::Unicode('m')),
        "N" => Some(Key::Unicode('n')),
        "O" => Some(Key::Unicode('o')),
        "P" => Some(Key::Unicode('p')),
        "Q" => Some(Key::Unicode('q')),
        "R" => Some(Key::Unicode('r')),
        "S" => Some(Key::Unicode('s')),
        "T" => Some(Key::Unicode('t')),
        "U" => Some(Key::Unicode('u')),
        "V" => Some(Key::Unicode('v')),
        "W" => Some(Key::Unicode('w')),
        "X" => Some(Key::Unicode('x')),
        "Y" => Some(Key::Unicode('y')),
        "Z" => Some(Key::Unicode('z')),
        "0" => Some(Key::Unicode('0')),
        "1" => Some(Key::Unicode('1')),
        "2" => Some(Key::Unicode('2')),
        "3" => Some(Key::Unicode('3')),
        "4" => Some(Key::Unicode('4')),
        "5" => Some(Key::Unicode('5')),
        "6" => Some(Key::Unicode('6')),
        "7" => Some(Key::Unicode('7')),
        "8" => Some(Key::Unicode('8')),
        "9" => Some(Key::Unicode('9')),
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
        "SPACE" => Some(Key::Space),
        "ENTER" | "RETURN" => Some(Key::Return),
        "TAB" => Some(Key::Tab),
        "ESC" | "ESCAPE" => Some(Key::Escape),
        "BACKSPACE" => Some(Key::Backspace),
        "UP" => Some(Key::UpArrow),
        "DOWN" => Some(Key::DownArrow),
        "LEFT" => Some(Key::LeftArrow),
        "RIGHT" => Some(Key::RightArrow),
        "HOME" => Some(Key::Home),
        "END" => Some(Key::End),
        "PAGEUP" => Some(Key::PageUp),
        "PAGEDOWN" => Some(Key::PageDown),
        "INSERT" => Some(Key::Insert),
        "DELETE" => Some(Key::Delete),
        _ => None,
    }
}

// ---- Normalization ----

fn normalize_card(card: &RapidfireCard) -> Result<RapidfireCard, String> {
    let name = card.name.trim();
    if name.is_empty() {
        return Err("连发器卡片名称不能为空".to_string());
    }

    let trigger_key = card.trigger_key.trim();
    if trigger_key.is_empty() {
        return Err(format!("{} 的触发键不能为空", name));
    }

    let target_key = card.target_key.trim();
    if target_key.is_empty() {
        return Err(format!("{} 的目标键不能为空", name));
    }

    if parse_target_key(target_key).is_none() {
        return Err(format!("{} 的目标键不支持: {}", name, target_key));
    }

    let interval_ms = card.interval_ms.max(RAPIDFIRE_MIN_INTERVAL_MS);

    Ok(RapidfireCard {
        id: card.id.trim().to_string(),
        name: name.to_string(),
        trigger_key: trigger_key.to_string(),
        target_key: target_key.to_string(),
        interval_ms,
        enabled: card.enabled,
    })
}

fn normalize_settings(mut settings_value: RapidfireSettings) -> Result<RapidfireSettings, String> {
    if settings_value.cards.is_empty() {
        settings_value.cards.push(
            RapidfireSettings::default()
                .cards
                .into_iter()
                .next()
                .unwrap(),
        );
    }

    settings_value.overlay_width = settings_value
        .overlay_width
        .max(RAPIDFIRE_DISPLAY_MIN_WIDTH)
        .min(RAPIDFIRE_DISPLAY_MAX_WIDTH);

    let mut seen_ids = HashMap::new();
    let mut cards = Vec::with_capacity(settings_value.cards.len());
    for card in &settings_value.cards {
        let normalized = normalize_card(card)?;
        if seen_ids.insert(normalized.id.clone(), true).is_some() {
            return Err(format!("连发器卡片 ID 重复: {}", normalized.id));
        }
        cards.push(normalized);
    }
    settings_value.cards = cards;
    Ok(settings_value)
}

// ---- Hotkey registration ----

fn restart_hotkey_listeners(
    state: &RapidfireState,
    hotkey_manager: &HotkeyManager,
    settings_value: &RapidfireSettings,
) -> Result<(), String> {
    if !settings_value.rapidfire_enabled {
        return hotkey_manager.clear_hold_scope("rapidfire");
    }

    let mut by_key: HashMap<String, Vec<String>> = HashMap::new();
    for card in &settings_value.cards {
        if !card.enabled {
            continue;
        }
        by_key
            .entry(card.trigger_key.trim().to_string())
            .or_default()
            .push(card.id.clone());
    }

    let bindings = by_key
        .into_iter()
        .map(|(key, card_ids)| {
            let action: HoldActionCallback = Arc::new(move |app_handle, hold_action| {
                let card_ids = card_ids.clone();
                let hold_action = hold_action.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = handle_hold_event(&app_handle, card_ids, hold_action).await {
                        let _ = app_handle.emit_to("main", "rapidfire://hotkey-error", error);
                    }
                });
            });
            (key, action)
        })
        .collect::<Vec<_>>();

    let result = hotkey_manager.replace_hold_scope("rapidfire", bindings);
    if result.is_ok() {
        if let Ok(mut inner) = state.inner.lock() {
            inner.hotkey_error = None;
        }
    }
    result
}

// ---- Core state machine ----

async fn handle_hold_event(
    app: &AppHandle,
    card_ids: Vec<String>,
    hold_action: HoldAction,
) -> Result<(), String> {
    match hold_action {
        HoldAction::Down => handle_key_down(app, card_ids).await,
        HoldAction::Up => handle_key_up(app, card_ids).await,
    }
}

async fn handle_key_down(app: &AppHandle, card_ids: Vec<String>) -> Result<(), String> {
    let state = app.state::<RapidfireState>();
    let tasks_to_spawn = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;

        if !inner.settings.rapidfire_enabled {
            return Ok(());
        }

        let mut tasks_to_spawn: Vec<(String, String, u64)> = Vec::new();

        for card_id in &card_ids {
            let cid_owned = card_id.clone();
            // 收集卡片信息（脱离不可变借用）
            let card_info = inner
                .settings
                .cards
                .iter()
                .find(|c| c.id == cid_owned && c.enabled)
                .map(|c| (c.id.clone(), c.target_key.clone(), c.interval_ms));

            let Some((cid, target, interval)) = card_info else {
                continue;
            };

            // 如果正在补齐，取消补齐
            if let Some(run) = inner.runs.get_mut(&cid) {
                if let Some(tx) = run.abort_tx.take() {
                    let _ = tx.send(());
                }
            }

            let (abort_tx, abort_rx) = oneshot::channel();
            inner.runs.insert(
                cid.clone(),
                CardRuntime {
                    count: 0,
                    status: RapidfireRunStatus::Firing,
                    abort_tx: Some(abort_tx),
                },
            );

            tasks_to_spawn.push((cid, target.clone(), interval));

            // 启动 tick 任务
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                run_tick_task(app_handle, cid_owned, target, interval, abort_rx).await;
            });
        }

        tasks_to_spawn
    };

    if !tasks_to_spawn.is_empty() {
        let bootstrap = {
            let inner = state
                .inner
                .lock()
                .map_err(|_| "连发器状态已损坏".to_string())?;
            inner.bootstrap()
        };
        emit_state(app, bootstrap);
    }
    Ok(())
}

async fn handle_key_up(app: &AppHandle, card_ids: Vec<String>) -> Result<(), String> {
    let state = app.state::<RapidfireState>();
    let tasks_to_spawn = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;

        if !inner.settings.rapidfire_enabled {
            return Ok(());
        }

        let mut tasks_to_spawn: Vec<(String, String, u64)> = Vec::new();

        for card_id in &card_ids {
            // 先收集卡片信息
            let card_info = inner
                .settings
                .cards
                .iter()
                .find(|c| c.id == *card_id)
                .map(|c| (c.target_key.clone(), c.interval_ms));

            let Some((target_key, interval_ms)) = card_info else {
                continue;
            };

            let Some(run) = inner.runs.get_mut(card_id) else {
                continue;
            };

            // 停止 tick 任务
            if let Some(abort_tx) = run.abort_tx.take() {
                let _ = abort_tx.send(());
            }

            let count = run.count;

            if count % 2 == 0 {
                // 偶数：直接结束
                run.status = RapidfireRunStatus::Idle;
            } else {
                // 奇数：进入补齐等待
                run.status = RapidfireRunStatus::PendingCompensation;
                tasks_to_spawn.push((card_id.clone(), target_key, interval_ms));
            }
        }

        tasks_to_spawn
    };

    // 在锁外执行补齐任务
    for (card_id, target_key, interval_ms) in tasks_to_spawn {
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            // 等待一个间隔
            time::sleep(Duration::from_millis(interval_ms)).await;

            let state = app_handle.state::<RapidfireState>();
            let should_fire = {
                let mut inner = match state.inner.lock() {
                    Ok(inner) => inner,
                    Err(_) => return,
                };

                let Some(run) = inner.runs.get_mut(&card_id) else {
                    return;
                };

                // 如果状态已改变（例如被新的按下列表取消），则不再补齐
                if run.status != RapidfireRunStatus::PendingCompensation {
                    return;
                }

                true
            };

            if should_fire {
                // 触发一次目标键
                if let Err(error) = fire_target_key(&target_key).await {
                    let _ = app_handle.emit_to("main", "rapidfire://hotkey-error", error);
                }

                // 更新状态
                let bootstrap = {
                    let mut inner = match state.inner.lock() {
                        Ok(inner) => inner,
                        Err(_) => return,
                    };

                    if let Some(run) = inner.runs.get_mut(&card_id) {
                        if run.status == RapidfireRunStatus::PendingCompensation {
                            run.count += 1;
                            run.status = RapidfireRunStatus::Idle;
                        }
                    }

                    inner.bootstrap()
                };

                emit_state(&app_handle, bootstrap);
            }
        });
    }

    emit_state(app, {
        let inner = state.inner.lock().map_err(|_| "连发器状态已损坏".to_string())?;
        inner.bootstrap()
    });
    Ok(())
}

async fn run_tick_task(
    app: AppHandle,
    card_id: String,
    target_key: String,
    interval_ms: u64,
    mut abort_rx: oneshot::Receiver<()>,
) {
    let mut interval = time::interval(Duration::from_millis(interval_ms));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // 检查状态是否仍为 Firing
                let should_fire = {
                    let state = app.state::<RapidfireState>();
                    let inner = match state.inner.lock() {
                        Ok(inner) => inner,
                        Err(_) => break,
                    };
                    inner.runs.get(&card_id).map(|r| r.status == RapidfireRunStatus::Firing).unwrap_or(false)
                };

                if !should_fire {
                    break;
                }

                // 触发目标键
                if let Err(error) = fire_target_key(&target_key).await {
                    let _ = app.emit_to("main", "rapidfire://hotkey-error", error);
                    break;
                }

                // 更新计数
                let state = app.state::<RapidfireState>();
                let bootstrap = {
                    let mut inner = match state.inner.lock() {
                        Ok(inner) => inner,
                        Err(_) => break,
                    };
                    if let Some(run) = inner.runs.get_mut(&card_id) {
                        if run.status == RapidfireRunStatus::Firing {
                            run.count += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                    inner.bootstrap()
                };

                emit_state(&app, bootstrap);
            }
            _ = &mut abort_rx => {
                break;
            }
        }
    }
}

// ---- Event emission ----

fn emit_state(app: &AppHandle, bootstrap: RapidfireBootstrap) {
    let _ = app.emit_to("main", "rapidfire://state-changed", bootstrap.clone());
    let _ = app.emit_to(RAPIDFIRE_DISPLAY_LABEL, "rapidfire://state-changed", bootstrap);
}

// ---- Window management ----

fn display_height(item_count: usize) -> i32 {
    RAPIDFIRE_DISPLAY_MIN_HEIGHT.max(32 + item_count.max(1) as i32 * 28)
}

fn ensure_overlay_window(app: &AppHandle, settings_value: &RapidfireSettings) -> Result<(), String> {
    if !settings_value.show_overlay || !settings_value.rapidfire_enabled {
        hide_window(app, RAPIDFIRE_DISPLAY_LABEL);
        return Ok(());
    }

    let enabled_count = settings_value.cards.iter().filter(|c| c.enabled).count();
    let height = display_height(enabled_count);
    let width = settings_value.overlay_width;
    let pos = settings_value
        .overlay_position
        .as_ref()
        .map(|p| (p.x, p.y))
        .unwrap_or((100, 100));

    if let Some(window) = app.get_webview_window(RAPIDFIRE_DISPLAY_LABEL) {
        let _ = window.set_size(PhysicalSize::new(width as u32, height as u32));
        let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
        let _ = window.set_always_on_top(true);
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.show();
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        app,
        RAPIDFIRE_DISPLAY_LABEL,
        WebviewUrl::App("index.html?mode=rapidfire-display".into()),
    )
    .title("连发器透明窗口")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(true)
    .resizable(false)
    .inner_size(width as f64, height as f64)
    .position(pos.0 as f64, pos.1 as f64)
    .build()
    .map_err(|error| format!("创建连发器透明窗口失败: {error}"))?;

    let _ = window.set_ignore_cursor_events(true);
    Ok(())
}

fn hide_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.hide();
    }
}

fn destroy_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.destroy();
    }
}

fn destroy_display_windows(app: &AppHandle) {
    destroy_window(app, RAPIDFIRE_DISPLAY_LABEL);
}

fn destroy_position_windows(app: &AppHandle) {
    destroy_window(app, RAPIDFIRE_POSITION_LABEL);
}

// ---- Initialize / Shutdown ----

pub fn is_main_window_close(label: &str) -> bool {
    label == "main"
}

pub fn shutdown(app: &AppHandle, state: &RapidfireState, hotkey_manager: &HotkeyManager) {
    let _ = hotkey_manager.clear_hold_scope("rapidfire");
    // Abort all running tasks
    if let Ok(mut inner) = state.inner.lock() {
        for run in inner.runs.values_mut() {
            if let Some(abort_tx) = run.abort_tx.take() {
                let _ = abort_tx.send(());
            }
        }
        inner.runs.clear();
    }
    destroy_position_windows(app);
    destroy_display_windows(app);
}

pub fn initialize(app: &AppHandle, hotkey_manager: &HotkeyManager) -> Result<RapidfireState, String> {
    let settings = normalize_settings(settings::load_settings(app)?)?;
    let state = RapidfireState {
        inner: Mutex::new(RapidfireStateInner {
            settings: settings.clone(),
            runs: HashMap::new(),
            pending_position: None,
            hotkey_error: None,
        }),
    };

    if settings.rapidfire_enabled {
        if let Err(error) = restart_hotkey_listeners(&state, hotkey_manager, &settings) {
            if let Ok(mut inner) = state.inner.lock() {
                inner.hotkey_error = Some(error);
            }
        }
        ensure_overlay_window(app, &settings)?;
    }

    Ok(state)
}

// ---- Tauri commands ----

#[tauri::command]
pub fn rapidfire_get_bootstrap(state: State<'_, RapidfireState>) -> Result<RapidfireBootstrap, String> {
    let inner = state
        .inner
        .lock()
        .map_err(|_| "连发器状态已损坏".to_string())?;
    Ok(inner.bootstrap())
}

#[tauri::command]
pub fn rapidfire_save_settings(
    settings_value: RapidfireSettings,
    app: AppHandle,
    state: State<'_, RapidfireState>,
    hotkey_manager: State<'_, HotkeyManager>,
) -> Result<RapidfireBootstrap, String> {
    let settings_value = normalize_settings(settings_value)?;
    settings::save_settings(&app, &settings_value)?;

    if let Err(error) = restart_hotkey_listeners(&state, &hotkey_manager, &settings_value) {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        inner.hotkey_error = Some(error.clone());
        return Err(error);
    }

    let bootstrap = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        inner.settings = settings_value.clone();
        inner.hotkey_error = None;

        // Abort runs for removed/disabled cards
        let active_card_ids: Vec<String> = settings_value
            .cards
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.id.clone())
            .collect();
        inner.runs.retain(|id, run| {
            if !active_card_ids.contains(id) {
                if let Some(abort_tx) = run.abort_tx.take() {
                    let _ = abort_tx.send(());
                }
                false
            } else {
                true
            }
        });

        if !settings_value.rapidfire_enabled {
            // Abort all
            for run in inner.runs.values_mut() {
                if let Some(abort_tx) = run.abort_tx.take() {
                    let _ = abort_tx.send(());
                }
            }
            inner.runs.clear();
        }

        inner.bootstrap()
    };

    ensure_overlay_window(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub fn rapidfire_stop(state: State<'_, RapidfireState>) -> Result<RapidfireBootstrap, String> {
    let bootstrap = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        for run in inner.runs.values_mut() {
            if let Some(abort_tx) = run.abort_tx.take() {
                let _ = abort_tx.send(());
            }
            run.status = RapidfireRunStatus::Idle;
            run.count = 0;
        }
        inner.runs.clear();
        inner.bootstrap()
    };
    Ok(bootstrap)
}

#[tauri::command]
pub async fn rapidfire_begin_position_selection(
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<RapidfireSelectionOutcome, String> {
    let (sender, receiver) = oneshot::channel();
    let position = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;

        if inner.pending_position.is_some() {
            return Err("当前已有一个位置设置流程在进行中".to_string());
        }

        let pos = inner
            .settings
            .overlay_position
            .clone()
            .unwrap_or(RapidfireRect { x: 100, y: 100 });

        inner.pending_position = Some(PendingRapidfirePosition {
            original_position: pos.clone(),
            staged_position: pos.clone(),
            sender,
        });
        pos
    };

    destroy_window(&app, RAPIDFIRE_POSITION_LABEL);

    let display_width = {
        let inner = state.inner.lock().map_err(|_| "连发器状态已损坏".to_string())?;
        inner.settings.overlay_width
    };
    let display_height = {
        let inner = state.inner.lock().map_err(|_| "连发器状态已损坏".to_string())?;
        display_height(inner.settings.cards.iter().filter(|c| c.enabled).count())
    };

    let window = WebviewWindowBuilder::new(
        &app,
        RAPIDFIRE_POSITION_LABEL,
        WebviewUrl::App("index.html?mode=rapidfire-position".into()),
    )
    .title("设置连发器位置")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(true)
    .visible(true)
    .resizable(false)
    .inner_size(display_width as f64, display_height as f64)
    .position(position.x as f64, position.y as f64)
    .build()
    .map_err(|error| format!("创建连发器位置设置窗口失败: {error}"))?;

    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed | WindowEvent::CloseRequested { .. }) {
            let state = close_app.state::<RapidfireState>();
            let mut inner = match state.inner.lock() {
                Ok(inner) => inner,
                Err(_) => return,
            };
            if let Some(pending) = inner.pending_position.take() {
                let _ = pending.sender.send(RapidfireSelectionKind::Closed);
            }
        }
    });

    let kind = match receiver.await {
        Ok(kind) => kind,
        Err(_) => RapidfireSelectionKind::Closed,
    };
    destroy_window(&app, RAPIDFIRE_POSITION_LABEL);

    let position = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        inner
            .settings
            .overlay_position
            .clone()
            .unwrap_or(RapidfireRect { x: 100, y: 100 })
    };

    Ok(RapidfireSelectionOutcome { kind, position })
}

#[tauri::command]
pub fn rapidfire_position_commit(
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<RapidfireBootstrap, String> {
    let (sender, bootstrap) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.take() else {
            return Err("当前没有等待中的位置设置流程".to_string());
        };

        inner.settings.overlay_position = Some(pending.staged_position.clone());
        settings::save_settings(&app, &inner.settings)?;
        (pending.sender, inner.bootstrap())
    };

    let _ = sender.send(RapidfireSelectionKind::Selected);
    destroy_window(&app, RAPIDFIRE_POSITION_LABEL);
    ensure_overlay_window(&app, &bootstrap.settings)?;
    emit_state(&app, bootstrap.clone());
    Ok(bootstrap)
}

#[tauri::command]
pub fn rapidfire_position_cancel(
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<(), String> {
    let (sender, _original_position) = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.take() else {
            return Err("当前没有等待中的位置设置流程".to_string());
        };

        let original = pending.original_position.clone();
        inner.settings.overlay_position = Some(original.clone());
        (pending.sender, original)
    };

    let _ = sender.send(RapidfireSelectionKind::Cancelled);
    destroy_window(&app, RAPIDFIRE_POSITION_LABEL);
    Ok(())
}

#[tauri::command]
pub fn rapidfire_position_moved(
    x: i32,
    y: i32,
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<RapidfireRect, String> {
    let rect = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.pending_position.as_mut() else {
            return Err("当前没有等待中的位置设置流程".to_string());
        };

        pending.staged_position.x = x;
        pending.staged_position.y = y;
        pending.staged_position.clone()
    };

    if let Some(window) = app.get_webview_window(RAPIDFIRE_POSITION_LABEL) {
        let _ = window.set_position(PhysicalPosition::new(rect.x, rect.y));
    }

    Ok(rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_card(id: &str, trigger: &str) -> RapidfireCard {
        RapidfireCard {
            id: id.to_string(),
            name: id.to_string(),
            trigger_key: trigger.to_string(),
            target_key: "1".to_string(),
            interval_ms: 100,
            enabled: true,
        }
    }

    #[test]
    fn normalize_card_rejects_empty_name() {
        let mut card = sample_card("a", "F1");
        card.name = "  ".to_string();
        let error = normalize_card(&card).unwrap_err();
        assert!(error.contains("名称不能为空"));
    }

    #[test]
    fn normalize_card_clamps_interval_to_minimum() {
        let mut card = sample_card("a", "F1");
        card.interval_ms = 5;
        let normalized = normalize_card(&card).unwrap();
        assert_eq!(normalized.interval_ms, RAPIDFIRE_MIN_INTERVAL_MS);
    }

    #[test]
    fn normalize_card_rejects_unsupported_target_key() {
        let mut card = sample_card("a", "F1");
        card.target_key = "UnknownKey".to_string();
        let error = normalize_card(&card).unwrap_err();
        assert!(error.contains("目标键不支持"));
    }

    #[test]
    fn parse_target_key_supports_all_valid_keys() {
        assert!(parse_target_key("A").is_some());
        assert!(parse_target_key("F1").is_some());
        assert!(parse_target_key("F12").is_some());
        assert!(parse_target_key("Space").is_some());
        assert!(parse_target_key("Enter").is_some());
        assert!(parse_target_key("Tab").is_some());
        assert!(parse_target_key("Esc").is_some());
        assert!(parse_target_key("Backspace").is_some());
        assert!(parse_target_key("Up").is_some());
        assert!(parse_target_key("Down").is_some());
        assert!(parse_target_key("Left").is_some());
        assert!(parse_target_key("Right").is_some());
        assert!(parse_target_key("Home").is_some());
        assert!(parse_target_key("End").is_some());
        assert!(parse_target_key("PageUp").is_some());
        assert!(parse_target_key("PageDown").is_some());
        assert!(parse_target_key("Insert").is_some());
        assert!(parse_target_key("Delete").is_some());
        assert!(parse_target_key("0").is_some());
        assert!(parse_target_key("9").is_some());
        assert!(parse_target_key("Unknown").is_none());
    }

    #[test]
    fn normalize_settings_auto_adds_default_card_when_empty() {
        let mut settings = RapidfireSettings::default();
        settings.cards.clear();
        let normalized = normalize_settings(settings).unwrap();
        assert_eq!(normalized.cards.len(), 1);
    }

    #[test]
    fn normalize_settings_rejects_duplicate_card_ids() {
        let mut settings = RapidfireSettings::default();
        settings.cards = vec![sample_card("same", "F1"), sample_card("same", "F2")];
        let error = normalize_settings(settings).unwrap_err();
        assert!(error.contains("ID 重复"));
    }

    #[test]
    fn normalize_settings_clamps_overlay_width() {
        let mut settings = RapidfireSettings::default();
        settings.overlay_width = 100;
        let normalized = normalize_settings(settings).unwrap();
        assert_eq!(normalized.overlay_width, RAPIDFIRE_DISPLAY_MIN_WIDTH);

        let mut settings2 = RapidfireSettings::default();
        settings2.overlay_width = 1000;
        let normalized2 = normalize_settings(settings2).unwrap();
        assert_eq!(normalized2.overlay_width, RAPIDFIRE_DISPLAY_MAX_WIDTH);
    }

    #[test]
    fn display_height_has_minimum() {
        assert_eq!(display_height(0), RAPIDFIRE_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(1), RAPIDFIRE_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(5), 172);
    }

    #[test]
    fn main_window_close_is_app_shutdown_request() {
        assert!(is_main_window_close("main"));
        assert!(!is_main_window_close(RAPIDFIRE_DISPLAY_LABEL));
        assert!(!is_main_window_close(RAPIDFIRE_POSITION_LABEL));
    }
}
