mod about;
mod app_error;
mod counter;
mod global_state;
mod hotkey_types;
mod hotkeys;
mod input_simulation;
mod key_suppressor;
mod logging;
mod morse;
mod overlay_utils;
mod profile;
mod rapidfire;
mod recognition;
mod settings;
mod special_ops;
mod sync_tool;
mod theme;
mod timer;
mod tool_base;

// 让 lib 单元测试也带上 Tauri 生成的 Windows manifest，避免旧版 comctl32 缺少 TaskDialogIndirect。
#[cfg(all(test, target_os = "windows"))]
#[link(name = "resource", kind = "static")]
extern "C" {}

use std::{path::PathBuf, sync::Arc};

use tauri::{Manager, WindowEvent};

fn initialize_with_settings_recovery<T, F>(
    app: &tauri::AppHandle,
    tool: &str,
    file_names: &[&str],
    initialize: F,
) -> Result<T, String>
where
    F: Fn() -> Result<T, String>,
{
    let paths = file_names
        .iter()
        .map(|file_name| settings::settings_path(app, file_name))
        .collect::<Result<Vec<_>, _>>()?;
    initialize_with_settings_recovery_paths(tool, paths, initialize)
}

fn initialize_with_settings_recovery_paths<T, F>(
    tool: &str,
    paths: Vec<PathBuf>,
    initialize: F,
) -> Result<T, String>
where
    F: Fn() -> Result<T, String>,
{
    match initialize() {
        Ok(state) => Ok(state),
        Err(first_error) => {
            let mut recovered = Vec::new();
            for path in paths {
                if path.exists() {
                    let backup = settings::backup_invalid_settings(&path)?;
                    recovered.push(format!("{}=>{}", path.display(), backup.display()));
                }
            }
            crate::log_warn!(
                "settings",
                "工具启动配置语义异常，已回退默认配置重试",
                "tool" => tool.to_string(),
                "backup" => recovered.join(";"),
                "error" => first_error.clone()
            );
            initialize().map_err(|retry_error| {
                format!(
                    "{tool} 初始化失败，配置恢复后仍失败: {retry_error}; 原始错误: {first_error}"
                )
            })
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            let _ = app
                .handle()
                .plugin(tauri_plugin_updater::Builder::new().build());

            let log_writer = logging::init_logger(app.handle())?;
            app.manage(log_writer);
            let theme_state = theme::initialize(app.handle())?;
            let settings_coordinator = Arc::new(settings::SettingsCoordinator::new());
            let profile_state =
                profile::initialize(app.handle(), Arc::clone(&settings_coordinator))?;
            let global_state = global_state::GlobalState::new(true);
            app.manage(theme_state);
            app.manage(settings_coordinator);
            app.manage(profile_state);
            app.manage(global_state);

            let hotkey_manager = hotkeys::HotkeyManager::start(app.handle().clone());
            let state = initialize_with_settings_recovery(
                app.handle(),
                "morse",
                &["morse_settings.json"],
                || morse::initialize(app.handle(), &hotkey_manager),
            )?;
            let timer_state = initialize_with_settings_recovery(
                app.handle(),
                "timer",
                &["timer_settings.json"],
                || timer::initialize(app.handle(), &hotkey_manager),
            )?;
            let counter_state = initialize_with_settings_recovery(
                app.handle(),
                "counter",
                &["counter_settings.json"],
                || counter::initialize(app.handle(), &hotkey_manager),
            )?;
            let rapidfire_state = initialize_with_settings_recovery(
                app.handle(),
                "rapidfire",
                &["rapidfire_settings.json"],
                || rapidfire::initialize(app.handle(), &hotkey_manager),
            )?;
            let recognition_state = initialize_with_settings_recovery(
                app.handle(),
                "recognition",
                &["recognition_settings.json", "audio_settings.json"],
                || recognition::initialize(app.handle()),
            )?;
            let special_ops_state = initialize_with_settings_recovery(
                app.handle(),
                "special_ops",
                &["special_ops_settings.json"],
                || special_ops::initialize(app.handle()),
            )?;
            let mut lifecycle_registry = sync_tool::ToolLifecycleRegistry::default();
            lifecycle_registry.register(
                "timer",
                Box::new(|app: &tauri::AppHandle| timer::stop_registered(app)),
            );
            lifecycle_registry.register(
                "counter",
                Box::new(|app: &tauri::AppHandle| counter::stop_registered(app)),
            );
            lifecycle_registry.register(
                "rapidfire",
                Box::new(|app: &tauri::AppHandle| rapidfire::stop_registered(app)),
            );
            lifecycle_registry.register(
                "morse",
                Box::new(|app: &tauri::AppHandle| {
                    crate::morse::cancel_active_overlay(app);
                    Ok(())
                }),
            );
            lifecycle_registry.register(
                "recognition",
                Box::new(|app: &tauri::AppHandle| crate::recognition::stop_registered(app)),
            );
            lifecycle_registry.register(
                "special_ops",
                Box::new(|app: &tauri::AppHandle| crate::special_ops::stop_registered(app)),
            );
            app.manage(hotkey_manager);
            app.manage(state);
            app.manage(timer_state);
            app.manage(counter_state);
            app.manage(rapidfire_state);
            app.manage(recognition_state);
            app.manage(special_ops_state);
            app.manage(lifecycle_registry);
            recognition::start_runtime(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                if timer::is_main_window_close(window.label()) {
                    let app = window.app_handle();
                    let Some(timer_state) = app.try_state::<timer::TimerState>() else {
                        return;
                    };
                    let Some(hotkey_manager) = app.try_state::<hotkeys::HotkeyManager>() else {
                        return;
                    };
                    timer::shutdown(app, &timer_state, &hotkey_manager);
                    let Some(counter_state) = app.try_state::<counter::CounterState>() else {
                        return;
                    };
                    counter::shutdown(app, &counter_state, &hotkey_manager);
                    let Some(rapidfire_state) = app.try_state::<rapidfire::RapidfireState>() else {
                        return;
                    };
                    rapidfire::shutdown(app, &rapidfire_state, &hotkey_manager);
                    let Some(_recognition_state) = app.try_state::<recognition::RecognitionState>()
                    else {
                        return;
                    };
                    let Some(hotkey_manager) = app.try_state::<hotkeys::HotkeyManager>() else {
                        return;
                    };
                    hotkey_manager.clear_all_suppressions();
                    recognition::shutdown(app, &hotkey_manager);
                    if let Some(log_writer) = app.try_state::<logging::LogWriter>() {
                        logging::shutdown(&log_writer);
                    }
                    app.exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // ── morse ──
            morse::morse_get_bootstrap,
            morse::morse_save_settings,
            morse::morse_set_hotkey_recording,
            morse::morse_begin_region_selection,
            morse::morse_overlay_submit_selection,
            morse::morse_overlay_cancel_selection,
            morse::morse_overlay_finish_early,
            morse::morse_run_recognition,
            // ── timer ──
            timer::timer_get_bootstrap,
            timer::timer_save_settings,
            timer::timer_trigger,
            timer::timer_begin_position_selection,
            timer::timer_position_commit,
            timer::timer_position_cancel,
            timer::timer_position_moved,
            // ── counter ──
            counter::counter_get_bootstrap,
            counter::counter_save_settings,
            counter::counter_trigger,
            counter::counter_reset,
            counter::counter_adjust,
            counter::counter_begin_position_selection,
            counter::counter_position_commit,
            counter::counter_position_cancel,
            counter::counter_position_moved,
            // ── rapidfire ──
            rapidfire::commands::rapidfire_get_bootstrap,
            rapidfire::commands::rapidfire_save_settings,
            rapidfire::commands::rapidfire_stop,
            rapidfire::commands::rapidfire_begin_position_selection,
            rapidfire::commands::rapidfire_position_commit,
            rapidfire::commands::rapidfire_position_cancel,
            rapidfire::commands::rapidfire_position_moved,
            // ── recognition ──
            recognition::recognition_get_bootstrap,
            recognition::recognition_save_settings,
            recognition::recognition_set_hotkey_recording,
            recognition::recognition_begin_region_selection,
            recognition::recognition_overlay_submit_selection,
            recognition::recognition_overlay_cancel_selection,
            recognition::recognition_test_play,
            recognition::recognition_test_match,
            recognition::recognition_read_reference_image,
            recognition::recognition_test_color_match,
            // ── 特勤处自动化 ──
            special_ops::special_ops_get_bootstrap,
            special_ops::special_ops_save_settings,
            special_ops::special_ops_set_paused,
            // ── global state ──
            global_state::global_get_enabled,
            global_state::global_set_enabled,
            // ── logging ──
            logging::log_write_frontend,
            logging::log_get_session_id,
            logging::log_get_level,
            logging::log_set_level,
            // ── about ──
            about::about_get_bootstrap,
            about::about_check_for_update,
            about::about_download_and_install,
            // ── theme ──
            theme::theme_get_bootstrap,
            theme::theme_save_settings,
            theme::theme_export,
            theme::theme_import,
            // ── profile ──
            profile::profile_get_bootstrap,
            profile::profile_save_current,
            profile::profile_create_default,
            profile::profile_apply,
            profile::profile_delete,
            profile::profile_export,
            profile::profile_export_to_path,
            profile::profile_import,
            profile::profile_import_from_path,
            profile::profile_rename,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;

    use super::*;

    #[test]
    fn initialize_with_settings_recovery_backs_up_invalid_file_and_retries() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("timer_settings.json");
        fs::write(&path, r#"{"timerEnabled":true,"timers":[{"name":""}]}"#).unwrap();
        let attempts = Cell::new(0);

        let result = initialize_with_settings_recovery_paths("timer", vec![path.clone()], || {
            let next = attempts.get() + 1;
            attempts.set(next);
            if next == 1 {
                Err("计时器名称不能为空".to_string())
            } else {
                Ok("default-state")
            }
        })
        .unwrap();

        assert_eq!(result, "default-state");
        assert_eq!(attempts.get(), 2);
        assert!(!path.exists());
        let backups = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("timer_settings.json.invalid-")
            })
            .count();
        assert_eq!(backups, 1);
    }
}
