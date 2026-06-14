mod app_error;
mod global_state;
mod tool_base;
mod hotkey_types;
mod hotkeys;
mod morse;
mod overlay_utils;
mod rapidfire;
mod settings;
mod strategy;
mod timer;
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
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .setup(|app| {
            let hotkey_manager = hotkeys::HotkeyManager::start(app.handle().clone());
            let state = morse::initialize(app.handle(), &hotkey_manager)?;
            let timer_state = timer::initialize(app.handle(), &hotkey_manager)?;
            let rapidfire_state = rapidfire::initialize(app.handle(), &hotkey_manager)?;
            let global_state = global_state::GlobalState::new(true);
            app.manage(hotkey_manager);
            app.manage(state);
            app.manage(timer_state);
            app.manage(rapidfire_state);
            app.manage(global_state);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                if timer::is_main_window_close(window.label()) {
                    let app = window.app_handle();
                    let timer_state = app.state::<timer::TimerState>();
                    let hotkey_manager = app.state::<hotkeys::HotkeyManager>();
                    timer::shutdown(app, &timer_state, &hotkey_manager);
                    let rapidfire_state = app.state::<rapidfire::RapidfireState>();
                    rapidfire::shutdown(app, &rapidfire_state, &hotkey_manager);
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
            timer::timer_counter_trigger,
            timer::timer_counter_reset,
            timer::timer_counter_adjust,
            timer::timer_begin_position_selection,
            timer::timer_position_commit,
            timer::timer_position_cancel,
            timer::timer_position_moved,

            // ── rapidfire ──
            rapidfire::rapidfire_get_bootstrap,
            rapidfire::rapidfire_save_settings,
            rapidfire::rapidfire_stop,
            rapidfire::rapidfire_begin_position_selection,
            rapidfire::rapidfire_position_commit,
            rapidfire::rapidfire_position_cancel,
            rapidfire::rapidfire_position_moved,

            // ── strategy ──
            strategy::webview::strategy_open_window,
            strategy::fetch::strategy_fetch_page,

            // ── global state ──
            global_state::global_get_enabled,
            global_state::global_set_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
