//! 多配置 Profile 模块。
//!
//! 一个 Profile = 5 份工具 settings 快照（morse/timer/counter/rapidfire/audio）。
//! 切换 Profile 时：
//! 1. 先停止所有运行态会话（rapidfire/timer/counter）
//! 2. 把目标 Profile 的 5 份 settings 写盘（统一走 `settings::save_settings`）
//! 3. 逐工具 reload 内存状态：normalize → swap inner.settings → restart 热键 → 刷新透明窗口 → emit_state
//! 4. counter 运行值重置为目标 Profile 的 start_value 并落盘 counter_state.json
//! 5. 更新 active_profile_id 并持久化 profile_settings.json
//!
//! 各工具的 reload 编排复用其已有的 `pub(crate)` 函数，不重写热键/窗口逻辑。
//! 主题独立于 Profile，不打包进快照。

pub mod settings;
pub mod types;

use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use crate::audio;
use crate::counter;
use crate::morse;
use crate::rapidfire;
use crate::settings as common_settings;
use crate::timer;
use crate::utils::now_ms;

use self::types::{Profile, ProfileBootstrap, ProfileSettings};

/// 持久化文件名常量（供 apply_profile 写盘 5 份 settings 用）。
const MORSE_FILE: &str = "morse_settings.json";
const TIMER_FILE: &str = "timer_settings.json";
const COUNTER_FILE: &str = "counter_settings.json";
const RAPIDFIRE_FILE: &str = "rapidfire_settings.json";
const AUDIO_FILE: &str = "audio_settings.json";

/// Profile 模块运行时状态。
pub struct ProfileState {
    settings: Mutex<ProfileSettings>,
}

impl ProfileState {
    pub fn new(settings: ProfileSettings) -> Self {
        Self {
            settings: Mutex::new(settings),
        }
    }
}

/// 初始化 Profile 状态：加载持久化 `profile_settings.json`，缺失则用默认值。
pub fn initialize(app: &AppHandle) -> Result<ProfileState, String> {
    let settings = settings::load_settings(app).unwrap_or_default();
    Ok(ProfileState::new(settings))
}

/// 生成新 Profile id：`p` + 毫秒时间戳 + 2 位递增计数。
///
/// 不引入 uuid 依赖；时间戳 + 原子计数足以避免单机短时碰撞。
fn generate_profile_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed) % 100;
    format!("p{}{:02}", now_ms(), seq)
}

fn max_config_number(profiles: &[Profile]) -> u32 {
    profiles
        .iter()
        .filter_map(|profile| profile.name.strip_prefix("配置"))
        .filter_map(|suffix| suffix.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
}

fn reserve_config_name(settings: &mut ProfileSettings) -> String {
    let mut number = settings
        .next_profile_number
        .max(max_config_number(&settings.profiles).saturating_add(1))
        .max(1);

    loop {
        let name = format!("配置{number}");
        settings.next_profile_number = number.saturating_add(1);
        if !settings.profiles.iter().any(|profile| profile.name == name) {
            return name;
        }
        number = number.saturating_add(1);
    }
}

fn build_default_snapshot() -> types::ToolSettingsSnapshot {
    types::ToolSettingsSnapshot {
        morse: Some(morse::MorseSettings::default()),
        timer: Some(timer::TimerSettings::default()),
        counter: Some(counter::CounterSettings::default()),
        rapidfire: Some(rapidfire::RapidfireSettings::default()),
        audio: Some(audio::AudioSettings::default()),
    }
}

fn append_profile(
    settings: &mut ProfileSettings,
    name: String,
    snapshot: types::ToolSettingsSnapshot,
) -> Profile {
    let now = now_ms();
    let profile = Profile {
        id: generate_profile_id(),
        name,
        created_at: now,
        updated_at: now,
        snapshot,
    };
    settings.active_profile_id = profile.id.clone();
    settings.profiles.push(profile.clone());
    profile
}

/// 构建 Profile bootstrap。
fn build_bootstrap(state: &ProfileState) -> ProfileBootstrap {
    let settings = state
        .settings
        .lock()
        .expect("Profile 状态锁被污染");
    ProfileBootstrap {
        profiles: settings.profiles.clone(),
        active_profile_id: settings.active_profile_id.clone(),
    }
}

#[tauri::command]
pub fn profile_get_bootstrap(
    app: AppHandle,
    state: State<'_, ProfileState>,
) -> Result<ProfileBootstrap, String> {
    let needs_default = {
        let settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        settings.profiles.is_empty()
    };

    if needs_default {
        let snapshot = snapshot_current_settings(&app)?;
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        let name = reserve_config_name(&mut settings);
        append_profile(&mut settings, name, snapshot);
        settings::save_settings(&app, &settings)?;
    }

    Ok(build_bootstrap(&state))
}

#[tauri::command]
pub fn profile_save_current(
    app: AppHandle,
    state: State<'_, ProfileState>,
    name: String,
) -> Result<Profile, String> {
    let snapshot = snapshot_current_settings(&app)?;

    let profile = {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        let profile = append_profile(&mut settings, name.trim().to_string(), snapshot);
        settings::save_settings(&app, &settings)?;
        profile
    };

    Ok(profile)
}

#[tauri::command]
pub fn profile_create_default(
    app: AppHandle,
    state: State<'_, ProfileState>,
) -> Result<ProfileBootstrap, String> {
    let snapshot = build_default_snapshot();
    let profile_id = {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        let name = reserve_config_name(&mut settings);
        let profile = append_profile(&mut settings, name, snapshot.clone());
        settings::save_settings(&app, &settings)?;
        profile.id
    };

    apply_snapshot_to_tools(&app, &snapshot)?;

    {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        settings.active_profile_id = profile_id;
        settings::save_settings(&app, &settings)?;
    }

    Ok(build_bootstrap(&state))
}

#[tauri::command]
pub fn profile_apply(
    app: AppHandle,
    state: State<'_, ProfileState>,
    id: String,
) -> Result<(), String> {
    // 先取出目标 Profile 快照（不持有锁做 IO）
    let snapshot = {
        let settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        let profile = settings
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("找不到配置: {id}"))?;
        profile.snapshot.clone()
    };

    apply_snapshot_to_tools(&app, &snapshot)?;

    // 更新 active_profile_id 并持久化
    {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        settings.active_profile_id = id;
        settings::save_settings(&app, &settings)?;
    }

    Ok(())
}

#[tauri::command]
pub fn profile_delete(
    app: AppHandle,
    state: State<'_, ProfileState>,
    id: String,
) -> Result<ProfileBootstrap, String> {
    {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        if settings.active_profile_id == id {
            return Err("不能删除当前激活的配置".to_string());
        }
        settings.profiles.retain(|p| p.id != id);
        settings::save_settings(&app, &settings)?;
    }
    Ok(build_bootstrap(&state))
}

#[tauri::command]
pub fn profile_rename(
    app: AppHandle,
    state: State<'_, ProfileState>,
    id: String,
    name: String,
) -> Result<ProfileBootstrap, String> {
    {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        let profile = settings
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("找不到配置: {id}"))?;
        profile.name = name.trim().to_string();
        profile.updated_at = now_ms();
        settings::save_settings(&app, &settings)?;
    }
    Ok(build_bootstrap(&state))
}

#[allow(dead_code)]
pub(crate) enum ActiveProfileSnapshotPatch {
    Morse(morse::MorseSettings),
    Timer(timer::TimerSettings),
    Counter(counter::CounterSettings),
    Rapidfire(rapidfire::RapidfireSettings),
    Audio(audio::AudioSettings),
}

#[allow(dead_code)]
pub(crate) fn update_active_profile_snapshot(
    app: &AppHandle,
    patch: ActiveProfileSnapshotPatch,
) -> Result<(), String> {
    let Some(state) = app.try_state::<ProfileState>() else {
        return Ok(());
    };

    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "Profile 状态锁已损坏")?;

    if settings.profiles.is_empty() || settings.active_profile_id.is_empty() {
        return Ok(());
    }

    let active_id = settings.active_profile_id.clone();
    let Some(profile) = settings
        .profiles
        .iter_mut()
        .find(|profile| profile.id == active_id)
    else {
        return Ok(());
    };

    match patch {
        ActiveProfileSnapshotPatch::Morse(value) => profile.snapshot.morse = Some(value),
        ActiveProfileSnapshotPatch::Timer(value) => profile.snapshot.timer = Some(value),
        ActiveProfileSnapshotPatch::Counter(value) => profile.snapshot.counter = Some(value),
        ActiveProfileSnapshotPatch::Rapidfire(value) => profile.snapshot.rapidfire = Some(value),
        ActiveProfileSnapshotPatch::Audio(value) => profile.snapshot.audio = Some(value),
    }
    profile.updated_at = now_ms();
    settings::save_settings(app, &settings)
}

/// 读取当前 5 份 settings 作为快照。
fn snapshot_current_settings(app: &AppHandle) -> Result<types::ToolSettingsSnapshot, String> {
    use crate::hotkeys::HotkeyManager;
    // 直接从内存状态读，避免重复 normalize
    let morse_settings = app
        .try_state::<morse::MorseState>()
        .map(|s| {
            s.inner
                .lock()
                .map(|inner| inner.settings.clone())
                .map_err(|_| "摩斯状态已损坏".to_string())
        })
        .transpose()?;

    let timer_settings = app
        .try_state::<timer::TimerState>()
        .map(|s| {
            s.lock_inner()
                .map(|inner| inner.settings.clone())
                .map_err(|_| "计时器状态已损坏".to_string())
        })
        .transpose()?;

    let counter_settings = app
        .try_state::<counter::CounterState>()
        .map(|s| {
            s.lock_inner()
                .map(|inner| inner.settings.clone())
                .map_err(|_| "计数器状态已损坏".to_string())
        })
        .transpose()?;

    let rapidfire_settings = app
        .try_state::<rapidfire::RapidfireState>()
        .map(|s| {
            s.lock_inner()
                .map(|inner| inner.settings.clone())
                .map_err(|_| "连发器状态已损坏".to_string())
        })
        .transpose()?;

    let audio_settings = app
        .try_state::<audio::AudioState>()
        .map(|s| {
            s.lock_inner()
                .map(|inner| inner.settings.clone())
                .map_err(|_| "音频状态已损坏".to_string())
        })
        .transpose()?;

    // 避免 unused import 警告（HotkeyManager 在 apply 路径用，snapshot 路径不用）
    let _ = std::marker::PhantomData::<HotkeyManager>;

    Ok(types::ToolSettingsSnapshot {
        morse: morse_settings,
        timer: timer_settings,
        counter: counter_settings,
        rapidfire: rapidfire_settings,
        audio: audio_settings,
    })
}

/// 把快照中的 5 份 settings 写盘 + 应用到各工具内存状态 + 重置计数器运行值。
///
/// 复用各工具已有的 `pub(crate)` reload 函数，不重写热键/窗口逻辑。
fn apply_snapshot_to_tools(
    app: &AppHandle,
    snapshot: &types::ToolSettingsSnapshot,
) -> Result<(), String> {
    use crate::hotkeys::HotkeyManager;

    let hotkey_manager = app.try_state::<HotkeyManager>();

    // 1. 先停止所有运行态会话（rapidfire/timer/counter），避免旧 session 残留
    if let Some(rapidfire_state) = app.try_state::<rapidfire::RapidfireState>() {
        rapidfire::stop_all(app, &rapidfire_state, hotkey_manager.as_ref().map(|v| &**v));
    }
    if let Some(timer_state) = app.try_state::<timer::TimerState>() {
        timer::stop_all(app, &timer_state);
    }
    if let Some(counter_state) = app.try_state::<counter::CounterState>() {
        counter::stop_all(app, &counter_state);
    }

    // 2. 写盘 5 份 settings（统一走公共 helper；audio 也用同一套，绕开其私有 write_settings）
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
    if let Some(a) = &snapshot.audio {
        let path = common_settings::settings_path(app, AUDIO_FILE)?;
        common_settings::save_settings(&path, a)?;
    }

    // 3. 逐工具 reload 内存状态
    apply_morse_settings(app, &snapshot.morse, hotkey_manager.as_ref().map(|v| &**v))?;
    apply_timer_settings(app, &snapshot.timer, hotkey_manager.as_ref().map(|v| &**v))?;
    apply_counter_settings(app, &snapshot.counter, hotkey_manager.as_ref().map(|v| &**v))?;
    apply_rapidfire_settings(app, &snapshot.rapidfire, hotkey_manager.as_ref().map(|v| &**v))?;
    apply_audio_settings(app, &snapshot.audio, hotkey_manager.as_ref().map(|v| &**v))?;

    // 4. counter 运行值重置为目标 Profile 的 start_value 并落盘
    if let Some(counter_state) = app.try_state::<counter::CounterState>() {
        counter::reset_runs_to_start_values(app, &counter_state)?;
    }

    Ok(())
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
    // 重启热键（无条件，不复用 save_settings 的「仅热键变才重启」优化）
    morse::restart_hotkey_listener(&state, app, hm, &normalized.hotkey)?;
    // swap 内存状态
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "摩斯状态已损坏".to_string())?;
    inner.settings = normalized;
    Ok(())
}

/// 应用 timer settings：normalize → swap inner.settings → 重启热键 → 刷新透明窗口 → emit_state。
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
    timer::ensure_display_windows(app, &bootstrap.settings)?;
    timer::emit_state(app, bootstrap);
    Ok(())
}

/// 应用 counter settings：normalize → swap inner.settings → 重启热键 → 刷新透明窗口 → emit_state。
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
        inner
            .logic
            .runs
            .retain(|id, _| normalized.counters.iter().any(|c| c.id == *id));
        for counter in &normalized.counters {
            inner
                .logic
                .runs
                .entry(counter.id.clone())
                .or_insert(counter.start_value);
        }
        if !normalized.counter_enabled {
            inner.logic.runs = normalized
                .counters
                .iter()
                .map(|c| (c.id.clone(), c.start_value))
                .collect();
        }
        crate::tool_base::ToolLogic::build_bootstrap(&inner)
    };
    counter::ensure_display_windows(app, &bootstrap.settings)?;
    counter::emit_state(app, bootstrap);
    Ok(())
}

/// 应用 rapidfire settings：normalize → swap inner.settings → 重启热键(force=true) → 刷新透明窗口 → emit_state。
///
/// 因为切换前已调用 `rapidfire::stop_all` 清掉了所有 session 与抑制，
/// 这里不需要复制 `stop_removed_or_disabled_sessions` 的复杂 diff 逻辑——
/// 直接换 settings + force 重启热键即可。
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
        // stop_all 已清空 runs；这里保持空（新 Profile 没有运行态）
        crate::tool_base::ToolLogic::build_bootstrap(&inner)
    };
    rapidfire::ensure_overlay_window(app, &bootstrap.settings)?;
    rapidfire::emit_state(app, bootstrap);
    Ok(())
}

/// 应用 audio settings：normalize → swap inner.settings → 重启热键 → 重启 watcher → emit_state。
fn apply_audio_settings(
    app: &AppHandle,
    snapshot: &Option<audio::AudioSettings>,
    hotkey_manager: Option<&crate::hotkeys::HotkeyManager>,
) -> Result<(), String> {
    let Some(new_settings) = snapshot.as_ref() else {
        return Ok(());
    };
    let Some(state) = app.try_state::<audio::AudioState>() else {
        return Ok(());
    };
    let Some(hm) = hotkey_manager else {
        return Err("热键管理器未注册".to_string());
    };

    let normalized = audio::normalize_settings(new_settings.clone());
    let mut inner = state
        .lock_inner()
        .map_err(|_| "音频状态已损坏".to_string())?;
    inner.settings = normalized.clone();
    let playback_tx = inner.logic.playback_tx.clone();
    // 在锁外重启热键和 watcher，避免持有锁期间做 IPC
    drop(inner);

    audio::restart_hotkey_listeners(hm, &normalized)?;
    crate::audio::watcher::restart_watchers(app, &normalized, playback_tx)?;

    let mut inner = state
        .lock_inner()
        .map_err(|_| "音频状态已损坏".to_string())?;
    if !inner.settings.audio_enabled {
        let _ = crate::audio::watcher::stop_all_watchers(app);
    }
    inner.hotkey_error = None;
    let bootstrap = <audio::AudioLogic as crate::tool_base::ToolLogic>::build_bootstrap(&inner);
    <audio::AudioLogic as crate::tool_base::ToolLogic>::emit_state(app, &bootstrap);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_profile_id_unique() {
        let a = generate_profile_id();
        let b = generate_profile_id();
        assert!(a.starts_with('p'));
        assert!(b.starts_with('p'));
        // 计数器保证短时间内不重复
        assert_ne!(a, b);
    }

    #[test]
    fn default_profile_settings_empty() {
        let state = ProfileState::new(ProfileSettings::default());
        let boot = build_bootstrap(&state);
        assert!(boot.profiles.is_empty());
        assert_eq!(boot.active_profile_id, "");
    }

    #[test]
    fn build_bootstrap_returns_profiles() {
        let settings = ProfileSettings {
            profiles: vec![Profile {
                id: "p1".to_string(),
                name: "PVE".to_string(),
                created_at: 1,
                updated_at: 2,
                snapshot: types::ToolSettingsSnapshot::empty(),
            }],
            active_profile_id: "p1".to_string(),
            next_profile_number: 1,
        };
        let state = ProfileState::new(settings);
        let boot = build_bootstrap(&state);
        assert_eq!(boot.profiles.len(), 1);
        assert_eq!(boot.profiles[0].id, "p1");
        assert_eq!(boot.active_profile_id, "p1");
    }

    #[test]
    fn reserve_config_name_starts_at_config_one() {
        let mut settings = ProfileSettings::default();
        let name = reserve_config_name(&mut settings);
        assert_eq!(name, "配置1");
        assert_eq!(settings.next_profile_number, 2);
    }

    #[test]
    fn reserve_config_name_uses_existing_max_number() {
        let mut settings = ProfileSettings {
            profiles: vec![
                Profile {
                    id: "p1".to_string(),
                    name: "配置1".to_string(),
                    created_at: 1,
                    updated_at: 1,
                    snapshot: types::ToolSettingsSnapshot::empty(),
                },
                Profile {
                    id: "p9".to_string(),
                    name: "配置9".to_string(),
                    created_at: 1,
                    updated_at: 1,
                    snapshot: types::ToolSettingsSnapshot::empty(),
                },
            ],
            active_profile_id: "p1".to_string(),
            next_profile_number: 2,
        };

        let name = reserve_config_name(&mut settings);

        assert_eq!(name, "配置10");
        assert_eq!(settings.next_profile_number, 11);
    }

    #[test]
    fn reserve_config_name_skips_existing_manual_name() {
        let mut settings = ProfileSettings {
            profiles: vec![Profile {
                id: "manual".to_string(),
                name: "配置2".to_string(),
                created_at: 1,
                updated_at: 1,
                snapshot: types::ToolSettingsSnapshot::empty(),
            }],
            active_profile_id: "manual".to_string(),
            next_profile_number: 2,
        };

        let name = reserve_config_name(&mut settings);

        assert_eq!(name, "配置3");
        assert_eq!(settings.next_profile_number, 4);
    }

    #[test]
    fn build_default_snapshot_includes_all_tools() {
        let snapshot = build_default_snapshot();
        assert!(snapshot.morse.is_some());
        assert!(snapshot.timer.is_some());
        assert!(snapshot.counter.is_some());
        assert!(snapshot.rapidfire.is_some());
        assert!(snapshot.audio.is_some());
    }

    #[test]
    fn empty_snapshot_stays_empty() {
        let snap = types::ToolSettingsSnapshot::empty();
        assert!(snap.morse.is_none());
        assert!(snap.timer.is_none());
        assert!(snap.counter.is_none());
        assert!(snap.rapidfire.is_none());
        assert!(snap.audio.is_none());
    }
}
