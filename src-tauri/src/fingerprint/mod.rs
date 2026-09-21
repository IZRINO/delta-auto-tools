mod events;
mod library;
mod matching;
pub(crate) mod overlay;
mod pipeline;
mod settings;
pub mod types;

use std::{collections::VecDeque, fs, path::PathBuf, sync::Arc};

use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::app_error::AppError;
use crate::hotkey_types;
use crate::hotkeys::{HotkeyAction, HotkeyManager};
use crate::morse::types::RegionRect;
use crate::recognition::watcher::{capture_region, read_reference_image_as_data_url};
use crate::settings::SettingsCoordinator;

use self::{
    overlay::PendingSelection,
    pipeline::PipelineOutput,
    types::{
        FingerprintBootstrap, FingerprintPerson, FingerprintRunResult, FingerprintSettings,
        HistoryEntry, RegionSelectionOutcome, RegionSelectionProgress,
    },
};

pub(crate) fn cancel_active_overlay(app: &AppHandle) {
    overlay::cancel_active_overlay(app);
}

pub struct FingerprintLogic {
    pub history: VecDeque<HistoryEntry>,
    pub latest_run: Option<FingerprintRunResult>,
    pub next_history_id: u64,
    pub pending_selection: Option<PendingSelection>,
    pub run_in_progress: bool,
}

impl FingerprintLogic {
    fn push_history(&mut self, entry: HistoryEntry) {
        self.history.push_front(entry);
        while self.history.len() > 100 {
            self.history.pop_back();
        }
    }
}

impl crate::tool_base::ToolLogic for FingerprintLogic {
    type Settings = FingerprintSettings;
    type Bootstrap = FingerprintBootstrap;
    const NAME: &'static str = "指纹";

    fn build_bootstrap(inner: &crate::tool_base::ToolStateInner<Self>) -> Self::Bootstrap {
        FingerprintBootstrap {
            settings: inner.settings.clone(),
            history: inner.logic.history.iter().cloned().collect(),
            latest_run: inner.logic.latest_run.clone(),
            hotkey_error: inner.hotkey_error.clone(),
        }
    }

    fn emit_state<R: Runtime>(_app: &AppHandle<R>, _bootstrap: &Self::Bootstrap) {}
}

pub type FingerprintState = crate::tool_base::ToolState<FingerprintLogic>;

pub(crate) fn restart_hotkey_listener(
    state: &FingerprintState,
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
        "fingerprint",
        vec![(hotkey.to_string(), action)],
        "指纹密码".to_string(),
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
    hotkey_manager.set_scope_enabled("fingerprint", !paused)
}

pub(crate) fn normalize_settings(
    mut settings_value: FingerprintSettings,
) -> Result<FingerprintSettings, String> {
    settings_value.hotkey = settings_value.hotkey.trim().to_string();
    if settings_value.hotkey.is_empty() {
        return Err("热键不能为空".to_string());
    }
    if !(0.0..=65025.0).contains(&settings_value.occupancy_threshold) {
        return Err("占用阈值必须在 0 到 65025 之间".to_string());
    }
    if !(0.0..=1.0).contains(&settings_value.match_threshold) {
        return Err("匹配阈值必须在 0 到 1 之间".to_string());
    }
    Ok(settings_value)
}

fn begin_run(app: &AppHandle) -> Result<FingerprintSettings, String> {
    let state = app.state::<FingerprintState>();
    let mut inner = state.lock_inner()?;
    if inner.logic.pending_selection.is_some() {
        return Err("当前正在执行区域选择，请完成后再试".to_string());
    }
    if inner.logic.run_in_progress {
        return Err("当前已有识别任务在运行中".to_string());
    }
    inner.logic.run_in_progress = true;
    Ok(inner.settings.clone())
}

fn finish_run(app: &AppHandle) {
    let state = app.state::<FingerprintState>();
    if let Ok(mut inner) = state.lock_inner() {
        inner.logic.run_in_progress = false;
    } else {
        crate::log_error!("fingerprint", "指纹状态已损坏，无法清除运行标志");
    };
}

async fn run_recognition_flow(
    app: &AppHandle,
    triggered_by: &str,
    auto_click: bool,
) -> Result<FingerprintRunResult, String> {
    let settings_snapshot = begin_run(app)?;
    let triggered = triggered_by.to_string();
    let run_result = async {
        let output = tokio::task::spawn_blocking(move || {
            pipeline::run_pipeline_with_points(&settings_snapshot, &triggered, auto_click)
        })
        .await
        .map_err(|error| format!("识别线程失败: {error}"))?;

        let mut output = match output {
            Ok(output) => output,
            Err(error) => PipelineOutput {
                result: FingerprintRunResult {
                    person_id: None,
                    person_name: None,
                    mode: None,
                    occupied_count: None,
                    matches: Vec::new(),
                    clicked: false,
                    triggered_by: triggered_by.to_string(),
                    occurred_at_ms: chrono::Utc::now().timestamp_millis() as u64,
                    error: Some(error),
                },
                points: Vec::new(),
            },
        };

        if output.result.error.is_none() && !output.points.is_empty() {
            if let Err(error) = crate::input_simulation::click_points(&output.points).await {
                output.result.error = Some(error);
            } else {
                output.result.clicked = true;
            }
        }
        Ok::<FingerprintRunResult, String>(output.result)
    }
    .await;

    finish_run(app);
    let result = run_result?;
    persist_run_result(app, result.clone());
    let _ = app.emit_to("main", events::RUN_FINISHED, result.clone());
    Ok(result)
}

fn persist_run_result(app: &AppHandle, result: FingerprintRunResult) {
    let state = app.state::<FingerprintState>();
    if let Ok(mut inner) = state.inner.lock() {
        let entry = HistoryEntry {
            id: inner.logic.next_history_id,
            person_name: result.person_name.clone(),
            mode: result.mode.clone(),
            success: result.error.is_none(),
            triggered_by: result.triggered_by.clone(),
            occurred_at_ms: result.occurred_at_ms,
            error: result.error.clone(),
        };
        inner.logic.next_history_id += 1;
        inner.logic.latest_run = Some(result);
        inner.logic.push_history(entry);
    } else {
        crate::log_error!("fingerprint", "指纹状态已损坏，无法写入运行结果");
    };
}

pub fn initialize(
    app: &AppHandle,
    hotkey_manager: &HotkeyManager,
) -> Result<FingerprintState, String> {
    let mut settings = normalize_settings(settings::load_settings(app)?)?;
    match seed_bundled_library(app, &mut settings) {
        Ok(0) => {}
        Ok(count) => {
            crate::log_info!(
                "fingerprint",
                "已灌入内置档案指纹",
                "count" => count
            );
            if let Err(error) = settings::save_settings(app, &settings) {
                crate::log_warn!(
                    "fingerprint",
                    "内置指纹落盘失败",
                    "error" => error
                );
            }
        }
        Err(error) => {
            crate::log_warn!(
                "fingerprint",
                "内置指纹灌入失败",
                "error" => error
            );
        }
    }
    let state = FingerprintState::new(
        FingerprintLogic {
            history: VecDeque::new(),
            latest_run: None,
            next_history_id: 1,
            pending_selection: None,
            run_in_progress: false,
        },
        settings.clone(),
    );

    if let Err(error) = restart_hotkey_listener(&state, app, hotkey_manager, &settings.hotkey) {
        crate::log_warn!(
            "fingerprint",
            "初始化热键监听失败",
            "hotkey" => settings.hotkey.clone(),
            "error" => error.clone()
        );
        if let Ok(mut inner) = state.lock_inner() {
            inner.hotkey_error = Some(error);
        }
    }

    Ok(state)
}

fn bundled_library_candidates(app: &AppHandle) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(resource) = app.path().resource_dir() {
        candidates.push(resource.join("fingerprint"));
        candidates.push(resource.join("resources").join("fingerprint"));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("fingerprint"),
    );
    candidates
}

fn seed_bundled_library(
    app: &AppHandle,
    settings: &mut FingerprintSettings,
) -> Result<usize, String> {
    let root = bundled_library_candidates(app).into_iter().find(|path| {
        library::scan_library_dir(path)
            .map(|plans| !plans.is_empty())
            .unwrap_or(false)
    });
    let Some(root) = root else {
        return Ok(0);
    };
    let plans = library::scan_library_dir(&root)?;
    let dest = settings::images_dir(app)?;
    library::apply_import(&mut settings.people, &plans, &dest, false)
}

fn persist_settings(
    app: &AppHandle,
    state: &FingerprintState,
) -> Result<FingerprintBootstrap, String> {
    let inner = state.lock_inner()?;
    settings::save_settings(app, &inner.settings)?;
    Ok(crate::tool_base::ToolLogic::build_bootstrap(&inner))
}

fn capture_to_path(region: &RegionRect, path: &PathBuf) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建图片目录: {error}"))?;
    }
    let image = capture_region(region).ok_or_else(|| "截图失败".to_string())?;
    image
        .save(path)
        .map_err(|error| format!("保存参考图失败: {error}"))
}

fn person_dir(app: &AppHandle, person_id: &str) -> Result<PathBuf, String> {
    Ok(settings::images_dir(app)?.join(person_id))
}

#[tauri::command]
pub fn fingerprint_get_bootstrap(
    state: State<'_, FingerprintState>,
) -> Result<FingerprintBootstrap, AppError> {
    crate::tool_base::get_bootstrap(state).map_err(AppError::from)
}

#[tauri::command]
pub async fn fingerprint_save_settings(
    settings_value: FingerprintSettings,
    settings_revision: u64,
    app: AppHandle,
    state: State<'_, FingerprintState>,
    hotkey_manager: State<'_, HotkeyManager>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
) -> Result<FingerprintBootstrap, AppError> {
    let settings_value = normalize_settings(settings_value)?;
    settings_coordinator.with_revision(settings_revision, || {
        let previous_settings = {
            let inner = state
                .inner
                .lock()
                .map_err(|_| "指纹状态已损坏".to_string())?;
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
            .map_err(|_| "指纹状态已损坏".to_string())?;
        inner.settings = settings_value;
        Ok(crate::tool_base::ToolLogic::build_bootstrap(&inner))
    })
}

#[tauri::command]
pub fn fingerprint_set_hotkey_recording(
    recording: bool,
    hotkey_manager: State<'_, HotkeyManager>,
) -> Result<(), AppError> {
    set_hotkey_listener_paused(&hotkey_manager, recording).map_err(AppError::from)
}

#[tauri::command]
pub async fn fingerprint_begin_region_selection(
    slots: Vec<usize>,
    target: String,
    app: AppHandle,
    state: State<'_, FingerprintState>,
) -> Result<RegionSelectionOutcome, AppError> {
    overlay::begin_region_selection(&app, slots, target, state)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub fn fingerprint_overlay_submit_selection(
    slot: usize,
    rect: RegionRect,
    settings_revision: u64,
    app: AppHandle,
    state: State<'_, FingerprintState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
) -> Result<RegionSelectionProgress, AppError> {
    settings_coordinator.with_revision(settings_revision, || {
        let prepared = overlay::prepare_selection(slot, rect, &state)?;
        let progress = prepared.progress.clone();
        let is_complete = prepared.is_complete;
        overlay::commit_selection(&app, prepared, &state)?;
        if is_complete {
            persist_settings(&app, &state)?;
        }
        let _ = app.emit_to("main", events::SELECTION_PROGRESS, progress.clone());
        Ok(progress)
    })
}

#[tauri::command]
pub fn fingerprint_overlay_cancel_selection(
    slot: usize,
    app: AppHandle,
    state: State<'_, FingerprintState>,
) -> Result<(), AppError> {
    overlay::cancel_selection(&app, slot, &state).map_err(AppError::from)
}

#[tauri::command]
pub async fn fingerprint_run(
    auto_click: Option<bool>,
    app: AppHandle,
) -> Result<FingerprintRunResult, AppError> {
    run_recognition_flow(&app, "manual", auto_click.unwrap_or(true))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub fn fingerprint_capture_name(
    name: String,
    settings_revision: u64,
    app: AppHandle,
    state: State<'_, FingerprintState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
) -> Result<FingerprintBootstrap, AppError> {
    settings_coordinator.with_revision(settings_revision, || {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::from("人名不能为空".to_string()));
        }
        let name_region = {
            let inner = state.lock_inner()?;
            inner
                .settings
                .name_region
                .clone()
                .ok_or_else(|| "请先框选名条区域".to_string())?
        };
        let inner = state.lock_inner()?;
        let existing = inner
            .settings
            .people
            .iter()
            .find(|person| person.name == name)
            .map(|person| person.id.clone());
        let person_id = existing.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let path = person_dir(&app, &person_id)?.join("name.png");
        drop(inner);
        capture_to_path(&name_region, &path)?;
        let mut inner = state.lock_inner()?;
        if let Some(person) = inner
            .settings
            .people
            .iter_mut()
            .find(|person| person.id == person_id)
        {
            person.name = name;
            person.name_image_path = path.to_string_lossy().into_owned();
        } else {
            inner.settings.people.push(FingerprintPerson {
                id: person_id,
                name,
                name_image_path: path.to_string_lossy().into_owned(),
                fingerprint_paths: Default::default(),
            });
        }
        drop(inner);
        persist_settings(&app, &state).map_err(AppError::from)
    })
}

#[tauri::command]
pub fn fingerprint_capture_archive(
    person_id: String,
    slots: Vec<usize>,
    settings_revision: u64,
    app: AppHandle,
    state: State<'_, FingerprintState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
) -> Result<FingerprintBootstrap, AppError> {
    settings_coordinator.with_revision(settings_revision, || {
        if slots.is_empty() {
            return Err(AppError::from("至少采集一枚档案指纹".to_string()));
        }
        let archive_slots = {
            let inner = state.lock_inner()?;
            inner.settings.archive_slots.clone()
        };
        for &slot in &slots {
            if slot >= 8 {
                return Err(AppError::from(format!("无效档案槽位: {slot}")));
            }
            let region = archive_slots[slot]
                .as_ref()
                .ok_or_else(|| format!("请先框选档案第 {} 槽", slot + 1))?;
            let path = person_dir(&app, &person_id)?.join(format!("{}.png", slot + 1));
            capture_to_path(region, &path)?;
            let mut inner = state.lock_inner()?;
            let person = inner
                .settings
                .people
                .iter_mut()
                .find(|person| person.id == person_id)
                .ok_or_else(|| "找不到这个角色".to_string())?;
            person.fingerprint_paths[slot] = Some(path.to_string_lossy().into_owned());
        }
        persist_settings(&app, &state).map_err(AppError::from)
    })
}

#[tauri::command]
pub fn fingerprint_delete_person(
    person_id: String,
    settings_revision: u64,
    app: AppHandle,
    state: State<'_, FingerprintState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
) -> Result<FingerprintBootstrap, AppError> {
    settings_coordinator.with_revision(settings_revision, || {
        {
            let mut inner = state.lock_inner()?;
            inner
                .settings
                .people
                .retain(|person| person.id != person_id);
        }
        let dir = person_dir(&app, &person_id)?;
        let _ = fs::remove_dir_all(dir);
        persist_settings(&app, &state).map_err(AppError::from)
    })
}

#[tauri::command]
pub fn fingerprint_read_image(path: String) -> Result<Option<String>, AppError> {
    Ok(read_reference_image_as_data_url(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rejects_empty_hotkey() {
        let settings = FingerprintSettings {
            hotkey: "  ".into(),
            ..FingerprintSettings::default()
        };
        assert!(normalize_settings(settings).is_err());
    }

    #[test]
    fn normalize_accepts_defaults() {
        let settings = normalize_settings(FingerprintSettings::default()).unwrap();
        assert_eq!(settings.hotkey, "F6");
    }
}
