mod decoder;
mod input;
mod overlay;
pub mod recognition;
mod settings;
pub mod types;
mod events;

// 对外暴露核心类型，供 profile 模块跨工具打包快照用。
pub use self::types::{MorseBootstrap, MorseSettings};

use std::{
    collections::VecDeque,
    sync::Arc,
};

use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::app_error::AppError;
use crate::hotkey_types;
use crate::hotkeys::{HotkeyAction, HotkeyManager};
use crate::profile::{self, ActiveProfileSnapshotPatch};
use crate::utils::now_ms;

use self::{
    overlay::PendingSelection,
    types::{
        HistoryEntry, MorseRunResult, RegionRect,
        RegionSelectionOutcome, RegionSelectionProgress,
    },
};

pub struct MorseLogic {
    pub history: VecDeque<HistoryEntry>,
    pub latest_run: Option<MorseRunResult>,
    pub next_history_id: u64,
    pub pending_selection: Option<PendingSelection>,
    pub run_in_progress: bool,
}

impl MorseLogic {
    pub fn push_history(&mut self, entry: HistoryEntry) {
        push_history_with_limit(&mut self.history, entry, 1000);
    }
}

impl crate::tool_base::ToolLogic for MorseLogic {
    type Settings = MorseSettings;
    type Bootstrap = MorseBootstrap;
    const NAME: &'static str = "摩斯";

    fn load_settings(app: &AppHandle) -> Result<Self::Settings, String> {
        settings::load_settings(app)
    }

    fn save_settings(app: &AppHandle, settings: &Self::Settings) -> Result<(), String> {
        settings::save_settings(app, settings)
    }

    fn build_bootstrap(inner: &crate::tool_base::ToolStateInner<Self>) -> Self::Bootstrap {
        MorseBootstrap {
            settings: inner.settings.clone(),
            history: inner.logic.history.iter().cloned().collect(),
            latest_run: inner.logic.latest_run.clone(),
            hotkey_error: inner.hotkey_error.clone(),
        }
    }

    fn emit_state<R: Runtime>(_app: &AppHandle<R>, _bootstrap: &Self::Bootstrap) {
        // Morse 不通过 emit_state 推送完整 bootstrap，仅在识别完成和区域选择时推送事件
    }
}

pub type MorseState = crate::tool_base::ToolState<MorseLogic>;

fn push_history_with_limit(
    history: &mut VecDeque<HistoryEntry>,
    entry: HistoryEntry,
    limit: usize,
) {
    history.push_front(entry);
    while history.len() > limit {
        history.pop_back();
    }
}

pub(crate) fn restart_hotkey_listener(
    state: &MorseState,
    app: &AppHandle,
    hotkey_manager: &HotkeyManager,
    hotkey: &str,
) -> Result<(), String> {
    let action: HotkeyAction = Arc::new(|app_handle| {
        tauri::async_runtime::spawn(async move {
            if let Err(error) = run_recognition_flow(&app_handle, "hotkey", true).await {
                let _ = app_handle.emit_to("main", events::HOTKEY_ERROR, error);
            }
        });
    });

    match hotkey_manager.replace_scope(
        "morse",
        vec![(hotkey.to_string(), action)],
        "摩斯密码解析".to_string(),
        hotkey_types::ConflictPolicy::Strict,
    ) {
        Ok(()) => {
            if let Ok(mut inner) = state.lock_inner() {
                inner.hotkey_error = None;
            }
            let _ = app;
            Ok(())
        }
        Err(error) => {
            if let Ok(mut inner) = state.lock_inner() {
                inner.hotkey_error = Some(error.clone());
            }
            Err(error)
        }
    }
}

fn set_hotkey_listener_paused(hotkey_manager: &HotkeyManager, paused: bool) -> Result<(), String> {
    hotkey_manager.set_scope_enabled("morse", !paused)
}

fn begin_run(app: &AppHandle) -> Result<MorseSettings, String> {
    let state = app.state::<MorseState>();
    let mut inner = state
        .lock_inner()?;

    if inner.logic.pending_selection.is_some() {
        return Err("当前正在执行区域选择，请完成后再试".to_string());
    }

    if inner.logic.run_in_progress {
        return Err("当前已有识别任务在运行中".to_string());
    }

    inner.logic.run_in_progress = true;
    Ok(inner.settings.clone())
}

pub(crate) fn normalize_settings(mut settings_value: MorseSettings) -> Result<MorseSettings, String> {
    settings_value.hotkey = settings_value.hotkey.trim().to_string();
    if settings_value.hotkey.is_empty() {
        return Err("热键不能为空".to_string());
    }

    settings_value.after_click_hotkey = settings_value
        .after_click_hotkey
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(hotkey_types::hotkey_to_string)
        .transpose()?;

    if settings_value.click_regions.len() > 7 {
        settings_value.click_regions.truncate(7);
    }

    Ok(settings_value)
}

fn finish_run(app: &AppHandle) {
    let state = app.state::<MorseState>();
    if let Ok(mut inner) = state.lock_inner() {
        inner.logic.run_in_progress = false;
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
                details: recognition::missing_regions_details(&settings_snapshot.regions),
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

        // 自动点击：识别成功 + auto_click_enabled 开启 + 有配置点击区域
        if settings_snapshot.auto_click_enabled
            && result.value.is_some()
            && result.error.is_none()
            && !settings_snapshot.click_regions.is_empty()
        {
            if let Err(error) = input::click_regions(&settings_snapshot.click_regions).await {
                result.error = Some(error);
            } else if let Some(after_click_hotkey) = settings_snapshot.after_click_hotkey.as_deref()
            {
                if let Err(error) = input::press_hotkey_once(after_click_hotkey).await {
                    result.error = Some(error);
                }
            }
        }
        Ok::<MorseRunResult, String>(result)
    }
        .await;

    finish_run(app);
    let result = run_result?;
    persist_run_result(app, result.clone());
    let _ = app.emit_to("main", events::RUN_FINISHED, result.clone());

    Ok(result)
}

pub fn initialize(app: &AppHandle, hotkey_manager: &HotkeyManager) -> Result<MorseState, String> {
    let settings = normalize_settings(settings::load_settings(app)?)?;
    let state = MorseState::new(
        MorseLogic {
            history: VecDeque::new(),
            latest_run: None,
            next_history_id: 1,
            pending_selection: None,
            run_in_progress: false,
        },
        settings.clone(),
    );

    if let Err(error) = restart_hotkey_listener(&state, app, hotkey_manager, &settings.hotkey) {
        if let Ok(mut inner) = state.lock_inner() {
            inner.hotkey_error = Some(error);
        }
    }

    Ok(state)
}

#[tauri::command]
pub fn morse_get_bootstrap(state: State<'_, MorseState>) -> Result<MorseBootstrap, AppError> {
    crate::tool_base::get_bootstrap(state).map_err(AppError::from)
}

#[tauri::command]
pub fn morse_save_settings(
    settings_value: MorseSettings,
    app: AppHandle,
    state: State<'_, MorseState>,
    hotkey_manager: State<'_, HotkeyManager>,
) -> Result<MorseBootstrap, AppError> {
    let settings_value = normalize_settings(settings_value)?;
    let previous_settings = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "摩斯状态已损坏".to_string())?;
        inner.settings.clone()
    };

    let hotkey_changed = previous_settings.hotkey.trim() != settings_value.hotkey.trim();

    if let Err(error) = settings::save_settings(&app, &settings_value) {
        return Err(AppError::from(error));
    }

    if hotkey_changed {
        if let Err(error) =
            restart_hotkey_listener(&state, &app, &hotkey_manager, &settings_value.hotkey)
        {
            let _ = settings::save_settings(&app, &previous_settings);
            return Err(AppError::from(error));
        }
    }

    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "摩斯状态已损坏".to_string())?;
    inner.settings = settings_value.clone();

    let bootstrap = crate::tool_base::ToolLogic::build_bootstrap(&inner);
    drop(inner);
    profile::update_active_profile_snapshot(
        &app,
        ActiveProfileSnapshotPatch::Morse(settings_value),
    )?;
    Ok(bootstrap)
}

#[tauri::command]
pub fn morse_set_hotkey_recording(
    recording: bool,
    hotkey_manager: State<'_, HotkeyManager>,
) -> Result<(), AppError> {
    set_hotkey_listener_paused(&hotkey_manager, recording).map_err(AppError::from)
}

#[tauri::command]
pub async fn morse_begin_region_selection(
    slots: Vec<usize>,
    target: String,
    app: AppHandle,
    state: State<'_, MorseState>,
) -> Result<RegionSelectionOutcome, AppError> {
    overlay::begin_region_selection(&app, slots, target, state).await.map_err(AppError::from)
}

#[tauri::command]
pub fn morse_overlay_submit_selection(
    slot: usize,
    rect: RegionRect,
    app: AppHandle,
    state: State<'_, MorseState>,
) -> Result<RegionSelectionProgress, AppError> {
    let prepared = overlay::prepare_selection(slot, rect, &state)?;
    let progress = prepared.progress.clone();
    let is_complete = prepared.is_complete;

    overlay::commit_selection(&app, prepared, &state)?;

    if is_complete {
        let settings_snapshot = {
            let inner = state
                .inner
                .lock()
                .map_err(|_| "摩斯状态已损坏".to_string())?;
            inner.settings.clone()
        };
        settings::save_settings(&app, &settings_snapshot)?;
        profile::update_active_profile_snapshot(
            &app,
            ActiveProfileSnapshotPatch::Morse(settings_snapshot),
        )?;
    }

    let _ = app.emit_to("main", events::SELECTION_PROGRESS, progress.clone());

    Ok(progress)
}

#[tauri::command]
pub fn morse_overlay_cancel_selection(
    slot: usize,
    app: AppHandle,
    state: State<'_, MorseState>,
) -> Result<(), AppError> {
    overlay::cancel_selection(&app, slot, &state).map_err(AppError::from)
}

/// 提前结束点击区域选择（Enter 键触发），保存当前已选区域。
#[tauri::command]
pub fn morse_overlay_finish_early(
    app: AppHandle,
    state: State<'_, MorseState>,
) -> Result<(), AppError> {
    overlay::finish_early(&app, &state)?;
    let settings_snapshot = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "摩斯状态已损坏".to_string())?;
        inner.settings.clone()
    };
    settings::save_settings(&app, &settings_snapshot)?;
    profile::update_active_profile_snapshot(
        &app,
        ActiveProfileSnapshotPatch::Morse(settings_snapshot),
    )?;
    Ok(())
}

#[tauri::command]
pub async fn morse_run_recognition(
    auto_type: Option<bool>,
    app: AppHandle,
) -> Result<MorseRunResult, AppError> {
    run_recognition_flow(&app, "manual", auto_type.unwrap_or(true)).await.map_err(AppError::from)
}

fn persist_run_result(app: &AppHandle, result: MorseRunResult) {
    let state = app.state::<MorseState>();
    if let Ok(mut inner) = state.inner.lock() {
        let entry = HistoryEntry {
            id: inner.logic.next_history_id,
            result: result.value.clone(),
            success: result.error.is_none(),
            triggered_by: result.triggered_by.clone(),
            auto_typed: result.auto_typed,
            occurred_at_ms: result.occurred_at_ms,
            error: result.error.clone(),
        };

        inner.logic.next_history_id += 1;
        inner.logic.latest_run = Some(result);
        inner.logic.push_history(entry);
    } else {
        eprintln!("摩斯状态已损坏，无法写入运行结果");
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(id: u64) -> HistoryEntry {
        HistoryEntry {
            id,
            result: Some(id.to_string()),
            success: true,
            triggered_by: "manual".to_string(),
            auto_typed: false,
            occurred_at_ms: id,
            error: None,
        }
    }

    #[test]
    fn push_history_with_limit_trims_old_entries() {
        let mut history = VecDeque::new();
        for id in 0..1005 {
            push_history_with_limit(&mut history, sample_entry(id), 1000);
        }

        assert_eq!(history.len(), 1000);
        assert_eq!(history.front().unwrap().id, 1004);
        assert_eq!(history.back().unwrap().id, 5);
    }

    #[test]
    fn normalize_settings_truncates_click_regions_to_seven() {
        let mut settings = MorseSettings::default();
        settings.click_regions = (0..8)
            .map(|index| super::types::ClickRegion {
                rect: RegionRect {
                    x: index,
                    y: 0,
                    width: 10,
                    height: 10,
                },
                delay_ms: 500,
            })
            .collect();

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.click_regions.len(), 7);
    }

    #[test]
    fn normalize_settings_normalizes_after_click_hotkey() {
        let mut settings = MorseSettings::default();
        settings.after_click_hotkey = Some(" shift+ctrl+- ".to_string());

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(
            normalized.after_click_hotkey.as_deref(),
            Some("Ctrl+Shift+-")
        );
    }
}
