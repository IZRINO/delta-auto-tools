mod about;
mod app_error;
mod audio;
mod counter;
mod global_state;
mod hotkey_types;
mod hotkeys;
mod key_suppressor;
mod logging;
mod morse;
mod overlay_utils;
mod profile;
mod rapidfire;
mod settings;
mod strategy;
mod sync_tool;
mod theme;
mod timer;
mod tool_base;
mod utils;

// 让 lib 单元测试也带上 Tauri 生成的 Windows manifest，避免旧版 comctl32 缺少 TaskDialogIndirect。
#[cfg(all(test, target_os = "windows"))]
#[link(name = "resource", kind = "static")]
extern "C" {}

use tauri::{Manager, WindowEvent};

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

            let hotkey_manager = hotkeys::HotkeyManager::start(app.handle().clone());
            let state = morse::initialize(app.handle(), &hotkey_manager)?;
            let timer_state = timer::initialize(app.handle(), &hotkey_manager)?;
            let counter_state = counter::initialize(app.handle(), &hotkey_manager)?;
            let rapidfire_state = rapidfire::initialize(app.handle(), &hotkey_manager)?;
            let audio_state = audio::initialize(app.handle(), &hotkey_manager)?;
            let theme_state = theme::initialize(app.handle())?;
            let profile_state = profile::initialize(app.handle())?;
            let global_state = global_state::GlobalState::new(true);
            let log_writer = logging::init_logger(app.handle())?;
            let mut sync_tool_registry = sync_tool::SyncToolRegistry::default();
            sync_tool_registry.register("counter", counter::stop_registered);
            sync_tool_registry.register("timer", timer::stop_registered);
            sync_tool_registry.register("rapidfire", rapidfire::stop_registered);
            app.manage(hotkey_manager);
            app.manage(state);
            app.manage(timer_state);
            app.manage(counter_state);
            app.manage(rapidfire_state);
            app.manage(audio_state);
            app.manage(theme_state);
            app.manage(profile_state);
            app.manage(global_state);
            app.manage(sync_tool_registry);
            app.manage(log_writer);
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
                    let Some(_audio_state) = app.try_state::<audio::AudioState>() else {
                        return;
                    };
                    let Some(hotkey_manager) = app.try_state::<hotkeys::HotkeyManager>() else {
                        return;
                    };
                    hotkey_manager.clear_all_suppressions();
                    audio::shutdown(app, &hotkey_manager);
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
            rapidfire::rapidfire_get_bootstrap,
            rapidfire::rapidfire_save_settings,
            rapidfire::rapidfire_stop,
            rapidfire::rapidfire_begin_position_selection,
            rapidfire::rapidfire_position_commit,
            rapidfire::rapidfire_position_cancel,
            rapidfire::rapidfire_position_moved,
            // ── audio ──
            audio::audio_get_bootstrap,
            audio::audio_save_settings,
            audio::audio_begin_region_selection,
            audio::audio_overlay_submit_selection,
            audio::audio_overlay_cancel_selection,
            audio::audio_test_play,
            audio::audio_test_match,
            audio::audio_read_reference_image,
            audio::audio_test_color_match,
            // ── strategy ──
            strategy::webview::strategy_open_window,
            strategy::fetch::strategy_fetch_page,
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
            profile::profile_rename,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
