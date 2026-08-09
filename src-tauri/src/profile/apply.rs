//! 配置应用编排。
//!
//! 这里只处理「把 Profile 快照应用到当前运行现场」：停止运行态、写工具 settings、
//! reload 工具内存状态、重启热键/监听、重置计数器运行值，并调度透明窗口刷新。
//! Profile 的命名、导入导出、active_profile_id 管理仍留在 `profile::mod`。

use tauri::{AppHandle, Manager};

use crate::settings as common_settings;
use crate::{counter, morse, rapidfire, recognition, special_ops, timer};

use super::types;

const MORSE_FILE: &str = "morse_settings.json";
const TIMER_FILE: &str = "timer_settings.json";
const COUNTER_FILE: &str = "counter_settings.json";
const RAPIDFIRE_FILE: &str = "rapidfire_settings.json";
const RECOGNITION_FILE: &str = "recognition_settings.json";

/// 把快照中的 5 份 settings 写盘 + 应用到各工具内存状态 + 重置计数器运行值。
///
/// 复用各工具已有的 `pub(crate)` reload 函数，不重写热键/窗口逻辑。
pub(crate) fn apply_snapshot_to_tools(
    app: &AppHandle,
    snapshot: &types::ToolSettingsSnapshot,
) -> Result<(), String> {
    let special_ops_snapshot = snapshot
        .special_ops
        .clone()
        .map(special_ops::normalize_settings)
        .transpose()?;
    let special_ops_state = app.try_state::<special_ops::SpecialOpsState>();
    if let Some(state) = special_ops_state.as_ref() {
        state.ensure_profile_apply_allowed()?;
    }

    apply_snapshot_to_tool_state(app, snapshot)?;
    if let (Some(state), Some(settings)) = (special_ops_state, special_ops_snapshot) {
        state.apply_profile_settings(app, settings)?;
    }
    schedule_profile_window_reconcile(app, snapshot);
    Ok(())
}

fn apply_snapshot_to_tool_state(
    app: &AppHandle,
    snapshot: &types::ToolSettingsSnapshot,
) -> Result<(), String> {
    use crate::hotkeys::HotkeyManager;

    let hotkey_manager = app.try_state::<HotkeyManager>();

    // 1. 先停止所有运行态会话，避免旧 session 残留。
    if let Some(rapidfire_state) = app.try_state::<rapidfire::RapidfireState>() {
        rapidfire::stop_all(app, &rapidfire_state, hotkey_manager.as_deref());
    }
    if let Some(timer_state) = app.try_state::<timer::TimerState>() {
        timer::stop_all(app, &timer_state);
    }
    if let Some(counter_state) = app.try_state::<counter::CounterState>() {
        counter::stop_all(app, &counter_state);
    }

    // 2. 写盘 5 份 settings。保持原有文件名与 normalize 顺序。
    if let Some(m) = &snapshot.morse {
        let path = common_settings::settings_path(app, MORSE_FILE)?;
        common_settings::save_settings(&path, m)?;
    }
    if let Some(t) = &snapshot.timer {
        let path = common_settings::settings_path(app, TIMER_FILE)?;
        common_settings::save_settings(&path, t)?;
    }
    if let Some(c) = &snapshot.counter {
        let path = common_settings::settings_path(app, COUNTER_FILE)?;
        common_settings::save_settings(&path, c)?;
    }
    if let Some(r) = &snapshot.rapidfire {
        let path = common_settings::settings_path(app, RAPIDFIRE_FILE)?;
        common_settings::save_settings(&path, r)?;
    }
    if let Some(a) = &snapshot.recognition {
        let path = common_settings::settings_path(app, RECOGNITION_FILE)?;
        let normalized = recognition::normalize_settings(a.clone());
        common_settings::save_settings(&path, &normalized)?;
    }

    // 3. 逐工具 reload 内存状态。
    apply_morse_settings(app, &snapshot.morse, hotkey_manager.as_deref())?;
    apply_timer_settings(app, &snapshot.timer, hotkey_manager.as_deref())?;
    apply_counter_settings(app, &snapshot.counter, hotkey_manager.as_deref())?;
    apply_rapidfire_settings(app, &snapshot.rapidfire, hotkey_manager.as_deref())?;
    apply_recognition_settings(app, &snapshot.recognition, hotkey_manager.as_deref())?;

    Ok(())
}

fn schedule_profile_window_reconcile(app: &AppHandle, snapshot: &types::ToolSettingsSnapshot) {
    if let Some(settings) = &snapshot.timer {
        timer::schedule_display_windows_reconcile_from_profile(app, settings);
    }
    if let Some(settings) = &snapshot.counter {
        counter::schedule_counter_windows_reconcile_from_profile(app, settings);
    }
    if let Some(settings) = &snapshot.rapidfire {
        rapidfire::schedule_overlay_window_reconcile_from_profile(app, settings);
    }
}

/// 应用 morse settings：normalize → swap inner.settings → 重启热键监听。
fn apply_morse_settings(
    app: &AppHandle,
    snapshot: &Option<morse::MorseSettings>,
    hotkey_manager: Option<&crate::hotkeys::HotkeyManager>,
) -> Result<(), String> {
    let Some(new_settings) = snapshot.as_ref() else {
        return Ok(());
    };
    let Some(state) = app.try_state::<morse::MorseState>() else {
        return Ok(());
    };
    let Some(hm) = hotkey_manager else {
        return Err("热键管理器未注册".to_string());
    };

    let normalized = morse::normalize_settings(new_settings.clone())?;
    morse::restart_hotkey_listener(&state, app, hm, &normalized.hotkey)?;
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "摩斯状态已损坏".to_string())?;
    inner.settings = normalized;
    Ok(())
}

/// 应用 timer settings：normalize → swap inner.settings → 重启热键 → emit_state。
fn apply_timer_settings(
    app: &AppHandle,
    snapshot: &Option<timer::TimerSettings>,
    hotkey_manager: Option<&crate::hotkeys::HotkeyManager>,
) -> Result<(), String> {
    let Some(new_settings) = snapshot.as_ref() else {
        return Ok(());
    };
    let Some(state) = app.try_state::<timer::TimerState>() else {
        return Ok(());
    };
    let Some(hm) = hotkey_manager else {
        return Err("热键管理器未注册".to_string());
    };

    let normalized = timer::normalize_settings(new_settings.clone())?;
    timer::restart_hotkey_listeners(&state, hm, &normalized)?;
    let bootstrap = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计时器状态已损坏".to_string())?;
        inner.settings = normalized.clone();
        inner.hotkey_error = None;
        inner
            .logic
            .runs
            .retain(|id, _| normalized.timers.iter().any(|t| t.id == *id));
        if !normalized.timer_enabled {
            inner.logic.runs.clear();
        }
        crate::tool_base::ToolLogic::build_bootstrap(&inner)
    };
    timer::emit_state(app, bootstrap);
    Ok(())
}

/// 应用 counter settings：normalize → swap inner.settings → 重启热键 → emit_state。
fn apply_counter_settings(
    app: &AppHandle,
    snapshot: &Option<counter::CounterSettings>,
    hotkey_manager: Option<&crate::hotkeys::HotkeyManager>,
) -> Result<(), String> {
    let Some(new_settings) = snapshot.as_ref() else {
        return Ok(());
    };
    let Some(state) = app.try_state::<counter::CounterState>() else {
        return Ok(());
    };
    let Some(hm) = hotkey_manager else {
        return Err("热键管理器未注册".to_string());
    };

    let normalized = counter::normalize_settings(new_settings.clone())?;
    counter::restart_hotkey_listeners(&state, hm, &normalized)?;
    let bootstrap = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "计数器状态已损坏".to_string())?;
        inner.settings = normalized.clone();
        inner.hotkey_error = None;
        counter::reset_runs_for_settings(&mut inner.logic.runs, &normalized);
        counter::persist_counter_runs(&state, &inner);
        crate::tool_base::ToolLogic::build_bootstrap(&inner)
    };
    counter::emit_runs(app, bootstrap.counter_runs.clone());
    counter::emit_state(app, bootstrap);
    Ok(())
}

/// 应用 rapidfire settings：normalize → swap inner.settings → 重启热键(force=true) → emit_state。
///
/// 因为切换前已调用 `rapidfire::stop_all` 清掉了所有 session 与抑制，
/// 这里不需要复制 `stop_removed_or_disabled_sessions` 的复杂 diff 逻辑。
fn apply_rapidfire_settings(
    app: &AppHandle,
    snapshot: &Option<rapidfire::RapidfireSettings>,
    hotkey_manager: Option<&crate::hotkeys::HotkeyManager>,
) -> Result<(), String> {
    let Some(new_settings) = snapshot.as_ref() else {
        return Ok(());
    };
    let Some(state) = app.try_state::<rapidfire::RapidfireState>() else {
        return Ok(());
    };
    let Some(hm) = hotkey_manager else {
        return Err("热键管理器未注册".to_string());
    };

    let normalized = rapidfire::normalize_settings(new_settings.clone())?;
    rapidfire::restart_hotkey_listeners(&state, hm, &normalized, true)?;
    let bootstrap = {
        let mut inner = state
            .lock_inner()
            .map_err(|_| "连发器状态已损坏".to_string())?;
        inner.settings = normalized.clone();
        inner.hotkey_error = None;
        crate::tool_base::ToolLogic::build_bootstrap(&inner)
    };
    rapidfire::emit_state(app, bootstrap);
    Ok(())
}

/// 应用 recognition settings：normalize → swap inner.settings → 重启热键 → 重启 watcher → emit_state。
fn apply_recognition_settings(
    app: &AppHandle,
    snapshot: &Option<recognition::RecognitionSettings>,
    hotkey_manager: Option<&crate::hotkeys::HotkeyManager>,
) -> Result<(), String> {
    let Some(new_settings) = snapshot.as_ref() else {
        return Ok(());
    };
    let Some(state) = app.try_state::<recognition::RecognitionState>() else {
        return Ok(());
    };
    let Some(hm) = hotkey_manager else {
        return Err("热键管理器未注册".to_string());
    };

    let normalized = recognition::normalize_settings(new_settings.clone());
    recognition::stop_all_hold_sessions(app)?;
    let mut inner = state
        .lock_inner()
        .map_err(|_| "识别触发状态已损坏".to_string())?;
    inner.settings = normalized.clone();
    drop(inner);

    recognition::restart_hotkey_listeners(hm, &normalized)?;
    crate::recognition::watcher::restart_watchers(app, &normalized)?;

    let mut inner = state
        .lock_inner()
        .map_err(|_| "识别触发状态已损坏".to_string())?;
    if !inner.settings.recognition_enabled {
        let _ = crate::recognition::watcher::stop_all_watchers(app);
    }
    inner.hotkey_error = None;
    let bootstrap =
        <recognition::RecognitionLogic as crate::tool_base::ToolLogic>::build_bootstrap(&inner);
    <recognition::RecognitionLogic as crate::tool_base::ToolLogic>::emit_state(app, &bootstrap);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_apply_splits_state_phase_from_window_reconcile() {
        let _state_phase = apply_snapshot_to_tool_state
            as fn(&AppHandle, &types::ToolSettingsSnapshot) -> Result<(), String>;
        let _window_phase =
            schedule_profile_window_reconcile as fn(&AppHandle, &types::ToolSettingsSnapshot);
    }

    #[test]
    fn profile_window_reconcile_schedulers_are_exposed() {
        let _timer = timer::schedule_display_windows_reconcile_from_profile
            as fn(&AppHandle, &timer::TimerSettings);
        let _counter = counter::schedule_counter_windows_reconcile_from_profile
            as fn(&AppHandle, &counter::CounterSettings);
        let _rapidfire = rapidfire::schedule_overlay_window_reconcile_from_profile
            as fn(&AppHandle, &rapidfire::RapidfireSettings);
    }
}
