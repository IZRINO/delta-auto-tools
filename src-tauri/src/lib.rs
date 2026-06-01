mod delta;
mod hotkey_types;
mod hotkeys;
mod morse;
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
        .setup(|app| {
            let hotkey_manager = hotkeys::HotkeyManager::start(app.handle().clone());
            let state = morse::initialize(app.handle(), &hotkey_manager)?;
            let delta_state = delta::initialize(app.handle())?;
            let timer_state = timer::initialize(app.handle(), &hotkey_manager)?;
            let rapidfire_state = rapidfire::initialize(app.handle(), &hotkey_manager)?;
            app.manage(hotkey_manager);
            app.manage(state);
            app.manage(delta_state);
            app.manage(timer_state);
            app.manage(rapidfire_state);
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
            delta::commands::delta_list_accounts,
            delta::commands::delta_delete_account,
            delta::commands::delta_qq_get_login_qr,
            delta::commands::delta_qq_poll_login_status,
            delta::commands::delta_qq_get_access_token,
            delta::commands::delta_qq_update_access_token,
            delta::commands::delta_wechat_get_login_qr,
            delta::commands::delta_wechat_poll_status,
            delta::commands::delta_wechat_get_access_token,
            delta::commands::delta_wechat_update_access_token,
            delta::commands::delta_qqsafe_get_login_qr,
            delta::commands::delta_qqsafe_poll_status,
            delta::commands::delta_qqsafe_get_access_token,
            delta::commands::delta_qqsafe_get_banned_list,
            delta::commands::delta_qqsafe_report,
            delta::commands::delta_pioneer_get_login_qr,
            delta::commands::delta_pioneer_poll_status,
            delta::commands::delta_pioneer_get_access_token,
            delta::commands::delta_pioneer_update_access_token,
            delta::commands::delta_pioneer_get_game_test_list,
            delta::commands::delta_wegame_qq_get_login_qr,
            delta::commands::delta_wegame_qq_poll_status,
            delta::commands::delta_wegame_qq_get_access_token,
            delta::commands::delta_wegame_wechat_get_login_qr,
            delta::commands::delta_wegame_wechat_poll_status,
            delta::commands::delta_wegame_wechat_get_access_token,
            delta::commands::delta_wegame_open_treasure_gift,
            delta::commands::delta_wegame_draw_daily_card,
            delta::commands::delta_game_get_items,
            delta::commands::delta_game_get_config,
            delta::commands::delta_game_get_price,
            delta::commands::delta_game_get_firearm_mod_list,
            delta::commands::delta_game_get_recommendation,
            delta::commands::delta_game_get_record,
            delta::commands::delta_game_get_player,
            delta::commands::delta_game_get_assets,
            delta::commands::delta_game_get_logs,
            delta::commands::delta_game_get_recent,
            delta::commands::delta_game_get_achievement,
            delta::commands::delta_game_get_password,
            delta::commands::delta_game_get_manufacture,
            delta::commands::delta_game_get_guns,
            delta::commands::delta_game_get_bind,
            morse::morse_get_bootstrap,
            morse::morse_save_settings,
            morse::morse_set_hotkey_recording,
            morse::morse_begin_region_selection,
            morse::morse_overlay_submit_selection,
            morse::morse_overlay_cancel_selection,
            morse::morse_run_recognition,
            timer::timer_get_bootstrap,
            timer::timer_save_settings,
            timer::timer_trigger,
            timer::timer_counter_trigger,
            timer::timer_counter_reset,
            timer::timer_begin_position_selection,
            timer::timer_position_commit,
            timer::timer_position_cancel,
            timer::timer_position_moved,
            rapidfire::rapidfire_get_bootstrap,
            rapidfire::rapidfire_save_settings,
            rapidfire::rapidfire_stop,
            rapidfire::rapidfire_begin_position_selection,
            rapidfire::rapidfire_position_commit,
            rapidfire::rapidfire_position_cancel,
            rapidfire::rapidfire_position_moved,
            strategy::fetch::fetch_strategy_page,
            strategy::fetch::strategy_open_in_view,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
