use std::{collections::HashSet, sync::Arc};

use tauri::{AppHandle, Manager, PhysicalPosition, State};
use tokio::sync::oneshot;

use crate::app_error::AppError;
use crate::hotkeys::HotkeyManager;
use crate::profile::{self, ActiveProfileSnapshotPatch};
use crate::settings::SettingsCoordinator;
use crate::sync_tool::group_enabled;
use crate::tool_base::ToolLogic;

use super::overlay::{display_height, ensure_overlay_window, position_label_for_group};
use super::settings;
use super::types::{
    RapidfireRect, RapidfireSelectionKind, RapidfireSelectionOutcome, RapidfireSettings,
    DEFAULT_RAPIDFIRE_GROUP_ID,
};
use super::worker::{self, SessionControl};
use super::PendingRapidfirePosition;
use super::RapidfireLogic;
use super::RapidfireState;

#[tauri::command]
pub fn rapidfire_get_bootstrap(
    state: State<'_, RapidfireState>,
) -> Result<super::types::RapidfireBootstrap, AppError> {
    let inner = state
        .lock_inner()
        .map_err(|_| "连发器状态已损坏".to_string())?;
    Ok(RapidfireLogic::build_bootstrap(&inner))
}

#[tauri::command]
pub async fn rapidfire_save_settings(
    settings_value: RapidfireSettings,
    settings_revision: u64,
    app: AppHandle,
    state: State<'_, RapidfireState>,
    hotkey_manager: State<'_, HotkeyManager>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
) -> Result<super::types::RapidfireBootstrap, AppError> {
    let settings_value = super::normalize_settings(settings_value)?;
    settings_coordinator.with_revision(settings_revision, || {
        let previous_settings = {
            let inner = state
                .lock_inner()
                .map_err(|_| "连发器状态已损坏".to_string())?;
            inner.settings.clone()
        };

        settings::save_settings(&app, &settings_value)?;

        if let Err(error) =
            super::restart_hotkey_listeners(&state, &hotkey_manager, &settings_value, false)
        {
            let _ = settings::save_settings(&app, &previous_settings);
            let _ =
                super::restart_hotkey_listeners(&state, &hotkey_manager, &previous_settings, true);
            let mut inner = state
                .lock_inner()
                .map_err(|_| "连发器状态已损坏".to_string())?;
            inner.hotkey_error = Some(error.clone());
            return Err(AppError::from(error));
        }

        let (bootstrap, suppressions_to_clear, should_stop_suppressor) = {
            let mut inner = state
                .lock_inner()
                .map_err(|_| "连发器状态已损坏".to_string())?;
            inner.settings = settings_value.clone();
            inner.hotkey_error = None;

            let active_card_ids: Vec<String> = settings_value
                .cards
                .iter()
                .filter(|c| c.enabled && group_enabled(&settings_value.groups, &c.group_id))
                .map(|c| c.id.clone())
                .collect();
            worker::stop_removed_or_disabled_sessions(&mut inner.logic.runs, &active_card_ids);

            let should_suppress: HashSet<String> = settings_value
                .cards
                .iter()
                .filter(|c| {
                    c.enabled
                        && c.ignore_trigger_key
                        && group_enabled(&settings_value.groups, &c.group_id)
                })
                .map(|c| c.trigger_key.clone())
                .collect();
            let previous_should_suppress: HashSet<String> = previous_settings
                .cards
                .iter()
                .filter(|c| {
                    c.enabled
                        && c.ignore_trigger_key
                        && group_enabled(&previous_settings.groups, &c.group_id)
                })
                .map(|c| c.trigger_key.clone())
                .collect();

            if !settings_value.rapidfire_enabled {
                worker::stop_all_sessions(&mut inner.logic.runs, SessionControl::Cancel);
                inner.logic.runs.clear();
            }

            (
                RapidfireLogic::build_bootstrap(&inner),
                previous_should_suppress
                    .difference(&should_suppress)
                    .cloned()
                    .collect::<Vec<_>>(),
                (should_suppress.is_empty() && !previous_should_suppress.is_empty())
                    || !settings_value.rapidfire_enabled,
            )
        };

        for trigger_key in suppressions_to_clear {
            let _ = hotkey_manager.unsuppress_key(&trigger_key);
        }
        if !settings_value.rapidfire_enabled {
            hotkey_manager.clear_all_suppressions();
        }
        if should_stop_suppressor {
            let _ = hotkey_manager.stop_suppressor();
        }

        ensure_overlay_window(&app, &bootstrap.settings)?;
        super::emit_state(&app, bootstrap.clone());
        profile::update_active_profile_snapshot(
            &app,
            ActiveProfileSnapshotPatch::Rapidfire(bootstrap.settings.clone()),
        )?;
        Ok(bootstrap)
    })
}

#[tauri::command]
pub fn rapidfire_stop(
    app: AppHandle,
    state: State<'_, RapidfireState>,
    hotkey_manager: State<'_, HotkeyManager>,
) -> Result<super::types::RapidfireBootstrap, AppError> {
    let (bootstrap, runs_changed) = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        worker::stop_all_sessions(&mut inner.logic.runs, SessionControl::Cancel);
        inner.logic.runs.clear();
        hotkey_manager.clear_all_suppressions();
        let _ = hotkey_manager.stop_suppressor();
        let bootstrap = RapidfireLogic::build_bootstrap(&inner);
        let runs_changed = super::force_runs_changed(&mut inner);
        (bootstrap, runs_changed)
    };
    super::emit_runs(&app, runs_changed);
    Ok(bootstrap)
}

#[tauri::command]
pub async fn rapidfire_begin_position_selection(
    group_id: Option<String>,
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<RapidfireSelectionOutcome, AppError> {
    use tauri::WebviewUrl;
    use tauri::WebviewWindowBuilder;
    use tauri::WindowEvent;

    let (sender, receiver) = oneshot::channel();
    let group_id = group_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_RAPIDFIRE_GROUP_ID.to_string());
    let position = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;

        if inner.logic.pending_position.is_some() {
            return Err(AppError::Message(
                "当前已有一个位置设置流程在进行中".to_string(),
            ));
        }

        let pos = inner
            .settings
            .groups
            .iter()
            .find(|group| group.id == group_id)
            .and_then(|group| group.overlay_position.clone())
            .or_else(|| inner.settings.overlay_position.clone())
            .unwrap_or(RapidfireRect { x: 100, y: 100 });

        inner.logic.pending_position = Some(PendingRapidfirePosition {
            group_id: group_id.clone(),
            original_position: pos.clone(),
            staged_position: pos.clone(),
            sender,
        });
        pos
    };

    let position_label = position_label_for_group(&group_id);
    crate::overlay_utils::destroy_window(&app, &position_label);

    let display_width = {
        let inner = state
            .lock_inner()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        inner
            .settings
            .groups
            .iter()
            .find(|group| group.id == group_id)
            .map(|group| group.overlay_width)
            .unwrap_or(inner.settings.overlay_width)
    };
    let display_height_val = {
        let inner = state
            .lock_inner()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        display_height(
            inner
                .settings
                .cards
                .iter()
                .filter(|c| c.enabled && c.group_id == group_id)
                .count(),
        )
    };

    let group_query_id = crate::overlay_utils::encoded_query_value(&group_id);
    let window = WebviewWindowBuilder::new(
        &app,
        &position_label,
        WebviewUrl::App(
            format!("index.html?mode=rapidfire-position&groupId={group_query_id}").into(),
        ),
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
    .inner_size(display_width as f64, display_height_val as f64)
    .position(position.x as f64, position.y as f64)
    .build()
    .map_err(|error| format!("创建连发器位置设置窗口失败: {error}"))?;

    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Destroyed | WindowEvent::CloseRequested { .. }
        ) {
            let state = close_app.state::<RapidfireState>();
            let mut inner = match state.lock_inner() {
                Ok(inner) => inner,
                Err(_) => return,
            };
            if let Some(pending) = inner.logic.pending_position.take() {
                let _ = pending.sender.send(RapidfireSelectionKind::Closed);
            }
        }
    });

    let kind = match receiver.await {
        Ok(kind) => kind,
        Err(_) => RapidfireSelectionKind::Closed,
    };
    crate::overlay_utils::destroy_window(&app, &position_label);

    let position = {
        let inner = state
            .lock_inner()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        inner
            .settings
            .groups
            .iter()
            .find(|group| group.id == group_id)
            .and_then(|group| group.overlay_position.clone())
            .or_else(|| inner.settings.overlay_position.clone())
            .unwrap_or(RapidfireRect { x: 100, y: 100 })
    };

    Ok(RapidfireSelectionOutcome {
        kind,
        position,
        group_id: Some(group_id),
    })
}

#[tauri::command]
pub fn rapidfire_position_commit(
    settings_revision: u64,
    app: AppHandle,
    state: State<'_, RapidfireState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
) -> Result<super::types::RapidfireBootstrap, AppError> {
    settings_coordinator.with_revision(settings_revision, || {
        let (sender, group_id, bootstrap) = {
            let mut inner = state
                .lock_inner()
                .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
            let Some(pending) = inner.logic.pending_position.take() else {
                return Err(AppError::Message(
                    "当前没有等待中的位置设置流程".to_string(),
                ));
            };

            let group_id = pending.group_id.clone();
            if let Some(group) = inner
                .settings
                .groups
                .iter_mut()
                .find(|group| group.id == group_id)
            {
                group.overlay_position = Some(pending.staged_position.clone());
            }
            if group_id == DEFAULT_RAPIDFIRE_GROUP_ID {
                inner.settings.overlay_position = Some(pending.staged_position.clone());
            }
            settings::save_settings(&app, &inner.settings)?;
            (
                pending.sender,
                group_id,
                RapidfireLogic::build_bootstrap(&inner),
            )
        };

        let _ = sender.send(RapidfireSelectionKind::Selected);
        crate::overlay_utils::destroy_window(&app, &position_label_for_group(&group_id));
        ensure_overlay_window(&app, &bootstrap.settings)?;
        super::emit_state(&app, bootstrap.clone());
        profile::update_active_profile_snapshot(
            &app,
            ActiveProfileSnapshotPatch::Rapidfire(bootstrap.settings.clone()),
        )?;
        Ok(bootstrap)
    })
}

#[tauri::command]
pub fn rapidfire_position_cancel(
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<(), AppError> {
    let (sender, group_id, _original_position) = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.logic.pending_position.take() else {
            return Err(AppError::Message(
                "当前没有等待中的位置设置流程".to_string(),
            ));
        };

        let original = pending.original_position.clone();
        let group_id = pending.group_id.clone();
        if let Some(group) = inner
            .settings
            .groups
            .iter_mut()
            .find(|group| group.id == group_id)
        {
            group.overlay_position = Some(original.clone());
        }
        if group_id == DEFAULT_RAPIDFIRE_GROUP_ID {
            inner.settings.overlay_position = Some(original.clone());
        }
        (pending.sender, group_id, original)
    };

    let _ = sender.send(RapidfireSelectionKind::Cancelled);
    crate::overlay_utils::destroy_window(&app, &position_label_for_group(&group_id));
    Ok(())
}

#[tauri::command]
pub fn rapidfire_position_moved(
    x: i32,
    y: i32,
    app: AppHandle,
    state: State<'_, RapidfireState>,
) -> Result<RapidfireRect, AppError> {
    let (rect, group_id) = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "连发器位置设置状态已损坏".to_string())?;
        let Some(pending) = inner.logic.pending_position.as_mut() else {
            return Err(AppError::Message(
                "当前没有等待中的位置设置流程".to_string(),
            ));
        };

        pending.staged_position.x = x;
        pending.staged_position.y = y;
        (pending.staged_position.clone(), pending.group_id.clone())
    };

    if let Some(window) = app.get_webview_window(&position_label_for_group(&group_id)) {
        let _ = window.set_position(PhysicalPosition::new(rect.x, rect.y));
    }

    Ok(rect)
}
