mod delta;
mod morse;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = morse::initialize(app.handle())?;
            let delta_state = delta::initialize(app.handle())?;
            app.manage(state);
            app.manage(delta_state);
            Ok(())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
