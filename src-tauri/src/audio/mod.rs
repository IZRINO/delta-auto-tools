use std::sync::Arc;

use tauri::{Emitter, Manager};
use tokio::sync::oneshot;

use crate::app_error::AppError;
use crate::hotkey_types::ConflictPolicy;
use crate::hotkeys::HotkeyManager;
use crate::tool_base::{ToolLogic, ToolState, ToolStateInner};

mod types;
mod events;
mod settings;
mod player;
mod watcher;

pub use self::types::{
    AudioBootstrap, AudioCard, AudioSettings, AudioTriggerMode,
};
pub use events::*;

use crate::morse::types::RegionRect;
use crate::overlay_utils::{
    destroy_stale_windows, destroy_window, encoded_query_value, safe_label_component,
};

const AUDIO_OVERLAY_LABEL: &str = "audio-overlay";

// ---- State ----

pub struct AudioLogic {
    #[allow(dead_code)]
    pub pending_selection: Option<PendingAudioSelection>,
}

pub type AudioState = ToolState<AudioLogic>;

impl ToolLogic for AudioLogic {
    type Settings = AudioSettings;
    type Bootstrap = AudioBootstrap;

    const NAME: &'static str = "音频";

    fn load_settings(app: &tauri::AppHandle) -> Result<Self::Settings, String> {
        settings::read_settings(app)
    }

    fn save_settings(app: &tauri::AppHandle, settings: &Self::Settings) -> Result<(), String> {
        settings::write_settings(app, settings)
    }

    fn build_bootstrap(inner: &ToolStateInner<Self>) -> Self::Bootstrap {
        AudioBootstrap {
            settings: inner.settings.clone(),
            hotkey_error: inner.hotkey_error.clone(),
        }
    }

    fn emit_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, bootstrap: &Self::Bootstrap) {
        let _ = app.emit(STATE_CHANGED, bootstrap);
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct PendingAudioSelection {
    pub card_id: String,
    pub sender: oneshot::Sender<RegionRect>,
}

// ---- Tauri commands ----

#[tauri::command]
pub fn audio_get_bootstrap(
    state: tauri::State<'_, AudioState>,
) -> Result<AudioBootstrap, AppError> {
    crate::tool_base::get_bootstrap(state).map_err(AppError::from)
}

#[tauri::command]
pub fn audio_save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    settings: AudioSettings,
) -> Result<AudioBootstrap, AppError> {
    let normalized = normalize_settings(settings);

    // 热键变化时重新注册
    let hotkey_manager = app.state::<HotkeyManager>();
    restart_hotkey_listeners(&hotkey_manager, &normalized);

    // 区域监听变化时重启 watcher
    let _ = watcher::restart_watchers(&app, &normalized);

    // 保存设置
    settings::write_settings(&app, &normalized).map_err(|e| AppError::from(e))?;

    // 更新内存状态
    let mut inner = state.lock_inner().map_err(|e| AppError::from(e))?;
    inner.settings = normalized;

    // 总开关关闭时停止所有
    if !inner.settings.audio_enabled {
        let _ = watcher::stop_all_watchers(&app);
    }

    let bootstrap = AudioLogic::build_bootstrap(&inner);
    AudioLogic::emit_state(&app, &bootstrap);
    Ok(bootstrap)
}

#[tauri::command]
pub async fn audio_begin_region_selection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    card_id: String,
) -> Result<(), AppError> {
    let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
    if !inner.settings.audio_enabled {
        return Err(AppError::from("音频功能未启用".to_string()));
    }

    // 检查卡片存在
    let card = inner
        .settings
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| AppError::from("卡片不存在".to_string()))?;

    if !card.enabled {
        return Err(AppError::from("卡片未启用".to_string()));
    }

    // 创建 overlay 窗口
    let overlay_url = format!(
        "/?mode=audio-overlay&audio_card={}",
        encoded_query_value(&card_id)
    );

    let label = format!("{}-{}", AUDIO_OVERLAY_LABEL, safe_label_component(&card_id));
    let mut active_labels = std::collections::HashSet::new();
    active_labels.insert(label.clone());
    destroy_stale_windows(&app, AUDIO_OVERLAY_LABEL, &active_labels);

    let _window = tauri::WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::App(overlay_url.parse().unwrap()),
    )
    .title("音频区域选择")
    .inner_size(800.0, 600.0)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .build();

    Ok(())
}

#[tauri::command]
pub async fn audio_overlay_submit_selection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    card_id: String,
    region: RegionRect,
) -> Result<(), AppError> {
    // 关闭 overlay 窗口
    let overlay_label = format!("{}-{}", AUDIO_OVERLAY_LABEL, safe_label_component(&card_id));
    destroy_window(&app, &overlay_label);

    // 更新卡片区域
    let mut inner = state.lock_inner().map_err(|e| AppError::from(e))?;
    if let Some(card) = inner.settings.cards.iter_mut().find(|c| c.id == card_id) {
        card.watch_region = Some(region);
    }
    settings::write_settings(&app, &inner.settings).map_err(|e| AppError::from(e))?;

    let bootstrap = AudioLogic::build_bootstrap(&inner);
    AudioLogic::emit_state(&app, &bootstrap);
    Ok(())
}

#[tauri::command]
pub async fn audio_overlay_cancel_selection(
    app: tauri::AppHandle,
    card_id: String,
) -> Result<(), AppError> {
    let overlay_label = format!("{}-{}", AUDIO_OVERLAY_LABEL, safe_label_component(&card_id));
    destroy_window(&app, &overlay_label);
    Ok(())
}

#[tauri::command]
pub async fn audio_test_play(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    card_id: String,
) -> Result<(), AppError> {
    let (path, volume) = {
        let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        let card = inner
            .settings
            .cards
            .iter()
            .find(|c| c.id == card_id)
            .ok_or_else(|| AppError::from("卡片不存在".to_string()))?;

        if card.audio_file_path.is_empty() {
            return Err(AppError::from("未设置音频文件路径".to_string()));
        }

        (card.audio_file_path.clone(), card.volume)
    };

    // 在阻塞线程中播放音频
    let _ = tokio::task::spawn_blocking(move || player::play_audio_file(&path, volume))
        .await
        .map_err(|e| AppError::from(format!("播放失败: {e}")))?;

    Ok(())
}

// ---- 热键 ----

fn restart_hotkey_listeners(hotkey_manager: &HotkeyManager, settings: &AudioSettings) {
    let _ = hotkey_manager.clear_scope("audio");

    if !settings.audio_enabled {
        return;
    }

    let bindings: Vec<(String, crate::hotkey_types::HotkeyAction)> = settings
        .cards
        .iter()
        .filter(|c| c.enabled && c.trigger_mode == AudioTriggerMode::Hotkey)
        .filter_map(|c| {
            c.hotkey.as_ref().map(|key| {
                let card_id = c.id.clone();
                let action: crate::hotkey_types::HotkeyAction =
                    Arc::new(move |app: tauri::AppHandle| {
                        let card_id = card_id.clone();
                        tokio::spawn(async move {
                            let _ = trigger_audio_play(&app, &card_id);
                        });
                    });
                (key.clone(), action)
            })
        })
        .collect();

    if !bindings.is_empty() {
        let _ = hotkey_manager.replace_scope(
            "audio",
            bindings,
            "音频".to_string(),
            ConflictPolicy::AllowHold,
        );
    }
}

fn trigger_audio_play(app: &tauri::AppHandle, card_id: &str) -> Result<(), String> {
    let state = app.state::<AudioState>();
    let inner = state.lock_inner()?;
    if !inner.settings.audio_enabled {
        return Ok(());
    }

    let card = inner
        .settings
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| "卡片不存在".to_string())?;

    if !card.enabled || card.audio_file_path.is_empty() {
        return Ok(());
    }

    let path = card.audio_file_path.clone();
    let volume = card.volume;
    drop(inner);

    // spawn_blocking 中播放音频
    let _ = std::thread::spawn(move || {
        let _ = player::play_audio_file(&path, volume);
    });

    let _ = app.emit(HOTKEY_TRIGGERED, card_id);
    Ok(())
}

// ---- 初始化与关闭 ----

pub fn initialize(
    app: &tauri::AppHandle,
    hotkey_manager: &HotkeyManager,
) -> Result<AudioState, String> {
    let settings = settings::read_settings(app)?;
    restart_hotkey_listeners(hotkey_manager, &settings);

    // 启动区域监听 watcher
    let _ = watcher::restart_watchers(app, &settings);

    let logic = AudioLogic {
        pending_selection: None,
    };

    Ok(AudioState::new(logic, settings))
}

pub fn shutdown(app: &tauri::AppHandle, hotkey_manager: &HotkeyManager) {
    let _ = hotkey_manager.clear_scope("audio");
    let _ = watcher::stop_all_watchers(app);
}

// ---- 设置规范化 ----

fn normalize_settings(settings: AudioSettings) -> AudioSettings {
    let mut cards = settings.cards;

    // 确保每张卡片有唯一 ID
    for card in &mut cards {
        if card.id.is_empty() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            card.id = format!("audio-{now}");
        }
    }

    // 确保至少有一张默认卡片
    if cards.is_empty() {
        cards.push(AudioCard {
            id: "audio-default".to_string(),
            name: "音频卡片 1".to_string(),
            enabled: true,
            trigger_mode: AudioTriggerMode::Hotkey,
            hotkey: None,
            watch_region: None,
            watch_reference_image_path: None,
            watch_match_threshold: 0.9,
            watch_poll_interval_ms: 500,
            audio_file_path: String::new(),
            volume: 0.8,
            cooldown_ms: 1000,
        });
    }

    AudioSettings {
        audio_enabled: settings.audio_enabled,
        cards,
    }
}
