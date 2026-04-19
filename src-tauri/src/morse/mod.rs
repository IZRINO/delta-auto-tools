mod decoder;
mod input;
mod input_listener;
mod overlay;
mod recognition;
mod settings;
mod types;

use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Emitter, Manager, State};

use self::{
    input_listener::PassiveHotkeyListener,
    overlay::PendingSelection,
    types::{
        HistoryEntry, MorseBootstrap, MorseRunResult, MorseSettings, RegionRect,
        RegionSelectionOutcome, RegionSelectionProgress,
    },
};

pub struct MorseState {
    pub(crate) inner: Mutex<MorseStateInner>,
    hotkey_listener: Mutex<Option<PassiveHotkeyListener>>,
}

pub(crate) struct MorseStateInner {
    settings: MorseSettings,
    history: VecDeque<HistoryEntry>,
    latest_run: Option<MorseRunResult>,
    next_history_id: u64,
    pending_selection: Option<PendingSelection>,
    run_in_progress: bool,
    hotkey_error: Option<String>,
}

impl MorseStateInner {
    fn bootstrap(&self) -> MorseBootstrap {
        MorseBootstrap {
            settings: self.settings.clone(),
            history: self.history.iter().cloned().collect(),
            latest_run: self.latest_run.clone(),
            hotkey_error: self.hotkey_error.clone(),
        }
    }

    fn push_history(&mut self, entry: HistoryEntry) {
        self.history.push_front(entry);
        while self.history.len() > 1000 {
            self.history.pop_back();
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn restart_hotkey_listener(
    state: &MorseState,
    app: &AppHandle,
    hotkey: &str,
) -> Result<(), String> {
    let mut listener = state
        .hotkey_listener
        .lock()
        .map_err(|_| "热键监听状态已损坏".to_string())?;

    if let Some(existing) = listener.as_mut() {
        existing.stop();
    }

    match PassiveHotkeyListener::start(app.clone(), hotkey) {
        Ok(new_listener) => {
            *listener = Some(new_listener);
            if let Ok(mut inner) = state.inner.lock() {
                inner.hotkey_error = None;
            }
            Ok(())
        }
        Err(error) => {
            *listener = None;
            if let Ok(mut inner) = state.inner.lock() {
                inner.hotkey_error = Some(error.clone());
            }
            Err(error)
        }
    }
}

fn set_hotkey_listener_paused(state: &MorseState, paused: bool) -> Result<(), String> {
    let listener = state
        .hotkey_listener
        .lock()
        .map_err(|_| "热键监听状态已损坏".to_string())?;

    if let Some(listener) = listener.as_ref() {
        listener.set_paused(paused);
    }

    Ok(())
}

fn begin_run(app: &AppHandle) -> Result<MorseSettings, String> {
    let state = app.state::<MorseState>();
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "摩斯状态已损坏".to_string())?;

    if inner.pending_selection.is_some() {
        return Err("当前正在执行区域选择，请完成后再试".to_string());
    }

    if inner.run_in_progress {
        return Err("当前已有识别任务在运行中".to_string());
    }

    inner.run_in_progress = true;
    Ok(inner.settings.clone())
}

fn finish_run(app: &AppHandle) {
    let state = app.state::<MorseState>();
    if let Ok(mut inner) = state.inner.lock() {
        inner.run_in_progress = false;
    } else {
        eprintln!("摩斯状态已损坏，无法清除运行标志");
    };
}

async fn run_recognition_flow(
    app: &AppHandle,
    triggered_by: &str,
    auto_type: bool,
) -> Result<MorseRunResult, String> {
    let settings_snapshot = begin_run(app)?;
    let run_result = async {
        let configured_regions = settings_snapshot
            .regions
            .iter()
            .filter(|region| region.is_some())
            .count();

        let mut result = if configured_regions != 3 {
            MorseRunResult {
                value: None,
                details: recognition::missing_regions_details(),
                triggered_by: triggered_by.to_string(),
                auto_typed: false,
                occurred_at_ms: now_ms(),
                error: Some("请先完成 3 个区域选择".to_string()),
            }
        } else {
            recognition::run_recognition(&settings_snapshot, triggered_by).await?
        };

        if auto_type {
            if let Some(value) = &result.value {
                let input_result =
                    input::type_result(value, settings_snapshot.auto_input_delay).await;
                if let Err(error) = input_result {
                    result.error = Some(error);
                } else {
                    result.auto_typed = true;
                }
            }
        }

        Ok::<MorseRunResult, String>(result)
    }
    .await;

    finish_run(app);
    let result = run_result?;
    persist_run_result(app, result.clone());
    let _ = app.emit_to("main", "morse://run-finished", result.clone());

    Ok(result)
}

pub fn initialize(app: &AppHandle) -> Result<MorseState, String> {
    let settings = settings::load_settings(app)?;
    let (listener, hotkey_error) = match PassiveHotkeyListener::start(app.clone(), &settings.hotkey) {
        Ok(listener) => (Some(listener), None),
        Err(error) => (None, Some(error)),
    };

    Ok(MorseState {
        inner: Mutex::new(MorseStateInner {
            settings,
            history: VecDeque::new(),
            latest_run: None,
            next_history_id: 1,
            pending_selection: None,
            run_in_progress: false,
            hotkey_error,
        }),
        hotkey_listener: Mutex::new(listener),
    })
}

#[tauri::command]
pub fn morse_get_bootstrap(state: State<'_, MorseState>) -> Result<MorseBootstrap, String> {
    let inner = state
        .inner
        .lock()
        .map_err(|_| "摩斯状态已损坏".to_string())?;

    Ok(inner.bootstrap())
}

#[tauri::command]
pub fn morse_save_settings(
    settings_value: MorseSettings,
    app: AppHandle,
    state: State<'_, MorseState>,
) -> Result<MorseBootstrap, String> {
    let previous_settings = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "摩斯状态已损坏".to_string())?;
        inner.settings.clone()
    };

    let hotkey_changed = previous_settings.hotkey.trim() != settings_value.hotkey.trim();

    if let Err(error) = settings::save_settings(&app, &settings_value) {
        return Err(error);
    }

    if hotkey_changed {
        if let Err(error) = restart_hotkey_listener(&state, &app, &settings_value.hotkey) {
            let _ = settings::save_settings(&app, &previous_settings);
            return Err(error);
        }
    }

    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "摩斯状态已损坏".to_string())?;
    inner.settings = settings_value;

    Ok(inner.bootstrap())
}

#[tauri::command]
pub fn morse_set_hotkey_recording(
    recording: bool,
    state: State<'_, MorseState>,
) -> Result<(), String> {
    set_hotkey_listener_paused(&state, recording)
}

#[tauri::command]
pub async fn morse_begin_region_selection(
    slots: Vec<usize>,
    app: AppHandle,
    state: State<'_, MorseState>,
) -> Result<RegionSelectionOutcome, String> {
    overlay::begin_region_selection(&app, slots, state).await
}

#[tauri::command]
pub fn morse_overlay_submit_selection(
    slot: usize,
    rect: RegionRect,
    app: AppHandle,
    state: State<'_, MorseState>,
) -> Result<RegionSelectionProgress, String> {
    let prepared = overlay::prepare_selection(slot, rect, &state)?;
    let progress = prepared.progress.clone();

    if prepared.is_complete {
        let mut settings_snapshot = {
            let inner = state
                .inner
                .lock()
                .map_err(|_| "摩斯状态已损坏".to_string())?;
            inner.settings.clone()
        };
        settings_snapshot.regions = progress.regions.clone();
        settings::save_settings(&app, &settings_snapshot)?;
    }

    let _ = app.emit_to("main", "morse://selection-progress", progress.clone());
    overlay::commit_selection(&app, prepared, &state)?;

    Ok(progress)
}

#[tauri::command]
pub fn morse_overlay_cancel_selection(
    slot: usize,
    app: AppHandle,
    state: State<'_, MorseState>,
) -> Result<(), String> {
    overlay::cancel_selection(&app, slot, &state)
}

#[tauri::command]
pub async fn morse_run_recognition(
    auto_type: Option<bool>,
    app: AppHandle,
) -> Result<MorseRunResult, String> {
    run_recognition_flow(&app, "manual", auto_type.unwrap_or(true)).await
}

fn persist_run_result(app: &AppHandle, result: MorseRunResult) {
    let state = app.state::<MorseState>();
    if let Ok(mut inner) = state.inner.lock() {
        let entry = HistoryEntry {
            id: inner.next_history_id,
            result: result.value.clone(),
            success: result.error.is_none(),
            triggered_by: result.triggered_by.clone(),
            auto_typed: result.auto_typed,
            occurred_at_ms: result.occurred_at_ms,
            error: result.error.clone(),
        };

        inner.next_history_id += 1;
        inner.latest_run = Some(result);
        inner.push_history(entry);
    } else {
        eprintln!("摩斯状态已损坏，无法写入运行结果");
    };
}
