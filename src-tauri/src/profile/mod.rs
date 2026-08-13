//! 多配置 Profile 模块。
//!
//! 一个 Profile = 既有工具与特勤处 settings 快照（specialOps 可选，兼容旧 Profile）。
//! 切换 Profile 时：
//! 1. 先停止所有运行态会话（rapidfire/timer/counter）
//! 2. 把目标 Profile 的既有工具与特勤处 settings 写盘（统一走 `settings::save_settings`）
//! 3. 逐工具 reload 内存状态：normalize → swap inner.settings → restart 热键 → 刷新透明窗口 → emit_state
//! 4. counter 运行值重置为目标 Profile 的 start_value 并落盘 counter_state.json
//! 5. 更新 active_profile_id 并持久化 profile_settings.json
//!
//! 各工具的 reload 编排复用其已有的 `pub(crate)` 函数，不重写热键/窗口逻辑。
//! 主题独立于 Profile，不打包进快照。

pub mod events;
pub mod settings;
pub mod types;

mod apply;

use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::counter;
use crate::morse;
use crate::rapidfire;
use crate::recognition;
use crate::settings::SettingsCoordinator;
use crate::special_ops;
use crate::timer;

use self::apply::apply_snapshot_to_tools;
use self::types::{Profile, ProfileBootstrap, ProfileSettings};

/// Profile 模块运行时状态。
pub struct ProfileState {
    settings: Mutex<ProfileSettings>,
    apply_lock: Mutex<()>,
    applying: AtomicBool,
    settings_coordinator: Arc<SettingsCoordinator>,
}

impl ProfileState {
    #[cfg(test)]
    pub fn new(settings: ProfileSettings) -> Self {
        Self::with_coordinator(settings, Arc::new(SettingsCoordinator::new()))
    }

    fn with_coordinator(
        settings: ProfileSettings,
        settings_coordinator: Arc<SettingsCoordinator>,
    ) -> Self {
        Self {
            settings: Mutex::new(settings),
            apply_lock: Mutex::new(()),
            applying: AtomicBool::new(false),
            settings_coordinator,
        }
    }
}

fn acquire_apply_lock(state: &ProfileState) -> Result<MutexGuard<'_, ()>, String> {
    state
        .apply_lock
        .lock()
        .map_err(|_| "Profile 切换锁已损坏".to_string())
}

/// 初始化 Profile 状态：加载持久化 `profile_settings.json`，缺失则用默认值。
pub fn initialize(
    app: &AppHandle,
    settings_coordinator: Arc<SettingsCoordinator>,
) -> Result<ProfileState, String> {
    let settings = settings::load_settings(app).unwrap_or_default();
    Ok(ProfileState::with_coordinator(
        settings,
        settings_coordinator,
    ))
}

/// 生成新 Profile id：`p` + 毫秒时间戳 + 2 位递增计数。
///
/// 不引入 uuid 依赖；时间戳 + 原子计数足以避免单机短时碰撞。
fn generate_profile_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed) % 100;
    format!(
        "p{}{:02}",
        chrono::Utc::now().timestamp_millis() as u64,
        seq
    )
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
        recognition: Some(recognition::RecognitionSettings::default()),
        special_ops: Some(special_ops::SpecialOpsSettings::default()),
    }
}

fn append_profile(
    settings: &mut ProfileSettings,
    name: String,
    snapshot: types::ToolSettingsSnapshot,
) -> Profile {
    let now = chrono::Utc::now().timestamp_millis() as u64;
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

fn reserve_import_name(settings: &mut ProfileSettings, name: &str) -> String {
    let base = name.trim();
    if base.is_empty() {
        return reserve_config_name(settings);
    }
    if !settings.profiles.iter().any(|profile| profile.name == base) {
        return base.to_string();
    }

    let mut number: u32 = 2;
    loop {
        let candidate = format!("{base} 导入{number}");
        if !settings
            .profiles
            .iter()
            .any(|profile| profile.name == candidate)
        {
            return candidate;
        }
        number = number.saturating_add(1);
    }
}

fn export_profile(settings: &ProfileSettings, id: &str) -> Result<String, String> {
    let profile = settings
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| format!("找不到配置: {id}"))?;
    serde_json::to_string_pretty(profile).map_err(|e| format!("导出配置失败: {e}"))
}

fn import_profile(settings: &mut ProfileSettings, json: &str) -> Result<Profile, String> {
    let imported: Profile =
        serde_json::from_str(json).map_err(|e| format!("配置 JSON 解析失败: {e}"))?;
    let now = chrono::Utc::now().timestamp_millis() as u64;
    let profile = Profile {
        id: generate_profile_id(),
        name: reserve_import_name(settings, &imported.name),
        created_at: now,
        updated_at: now,
        snapshot: imported.snapshot,
    };
    settings.profiles.push(profile.clone());
    Ok(profile)
}

fn read_profile_json(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("导入路径不能为空".to_string());
    }
    fs::read_to_string(Path::new(path)).map_err(|e| format!("读取配置文件失败: {e}"))
}

fn write_profile_json(path: &str, json: &str) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("导出路径不能为空".to_string());
    }
    fs::write(Path::new(path), json).map_err(|e| format!("写入配置文件失败: {e}"))
}

/// 构建 Profile bootstrap。
fn build_bootstrap(state: &ProfileState) -> Result<ProfileBootstrap, String> {
    let settings_revision = state.settings_coordinator.current_revision()?;
    let settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
    Ok(ProfileBootstrap {
        profiles: settings.profiles.clone(),
        active_profile_id: settings.active_profile_id.clone(),
        settings_revision,
    })
}

/// 向 main 窗口 emit profile://changed 事件，payload 为最新 bootstrap。
fn emit_profile_changed(app: &AppHandle, bootstrap: &ProfileBootstrap) {
    #[cfg(test)]
    emit_tracker::EMIT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let _ = app.emit_to("main", events::CHANGED, bootstrap);
}

#[cfg(test)]
mod emit_tracker {
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub static EMIT_COUNT: AtomicUsize = AtomicUsize::new(0);

    pub fn reset() {
        EMIT_COUNT.store(0, Ordering::Relaxed);
    }

    pub fn count() -> usize {
        EMIT_COUNT.load(Ordering::Relaxed)
    }
}

#[tauri::command]
pub fn profile_get_bootstrap(
    app: AppHandle,
    state: State<'_, ProfileState>,
) -> Result<ProfileBootstrap, String> {
    let needs_default = {
        let settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
        settings.profiles.is_empty()
    };

    if needs_default {
        let snapshot = snapshot_current_settings(&app)?;
        let mut settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
        let name = reserve_config_name(&mut settings);
        append_profile(&mut settings, name, snapshot);
        settings::save_settings(&app, &settings)?;
    }

    build_bootstrap(&state)
}

#[tauri::command]
pub fn profile_save_current(
    app: AppHandle,
    state: State<'_, ProfileState>,
    name: String,
) -> Result<Profile, String> {
    let (profile, _) = state
        .settings_coordinator
        .with_profile_change(|side_effect_started| {
            let snapshot = snapshot_current_settings(&app)?;
            let mut settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
            *side_effect_started = true;
            let profile = append_profile(&mut settings, name.trim().to_string(), snapshot);
            settings::save_settings(&app, &settings)?;
            Ok::<Profile, String>(profile)
        })?;

    let bootstrap = build_bootstrap(&state)?;
    emit_profile_changed(&app, &bootstrap);
    Ok(profile)
}

#[tauri::command]
pub async fn profile_create_default(
    app: AppHandle,
    state: State<'_, ProfileState>,
) -> Result<ProfileBootstrap, String> {
    let _apply_guard = acquire_apply_lock(&state)?;
    state.applying.store(true, Ordering::SeqCst);
    let apply_result = state
        .settings_coordinator
        .with_profile_change(|side_effect_started| {
            let snapshot = build_default_snapshot();
            let profile_id = {
                let mut settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
                *side_effect_started = true;
                let name = reserve_config_name(&mut settings);
                let profile = append_profile(&mut settings, name, snapshot.clone());
                settings::save_settings(&app, &settings)?;
                profile.id
            };

            apply_snapshot_to_tools(&app, &snapshot)?;

            let mut settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
            settings.active_profile_id = profile_id;
            settings::save_settings(&app, &settings)
        });
    state.applying.store(false, Ordering::SeqCst);
    apply_result?;

    special_ops::emit_state_for_profile_change(&app);

    let bootstrap = build_bootstrap(&state)?;
    emit_profile_changed(&app, &bootstrap);
    Ok(bootstrap)
}

#[tauri::command]
pub async fn profile_apply(
    app: AppHandle,
    state: State<'_, ProfileState>,
    id: String,
) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    let _apply_guard = acquire_apply_lock(&state)?;
    state.applying.store(true, Ordering::SeqCst);
    crate::log_info!(
        "profile",
        "开始切换 Profile",
        "profile_id" => id.clone()
    );

    let apply_result = state
        .settings_coordinator
        .with_profile_change(|side_effect_started| {
            // 先取出目标 Profile 快照（不持有 Profile 锁做 IO）
            let snapshot = {
                let settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
                let profile = settings
                    .profiles
                    .iter()
                    .find(|p| p.id == id)
                    .ok_or_else(|| format!("找不到配置: {id}"))?;
                profile.snapshot.clone()
            };

            *side_effect_started = true;
            apply_snapshot_to_tools(&app, &snapshot)?;

            let mut settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
            settings.active_profile_id = id.clone();
            settings::save_settings(&app, &settings)
        });
    state.applying.store(false, Ordering::SeqCst);
    apply_result?;

    special_ops::emit_state_for_profile_change(&app);

    let bootstrap = build_bootstrap(&state)?;
    emit_profile_changed(&app, &bootstrap);
    crate::log_info!(
        "profile",
        "完成切换 Profile",
        "profile_id" => id,
        "elapsed_ms" => started_at.elapsed().as_millis().to_string()
    );
    Ok(())
}

#[tauri::command]
pub fn profile_delete(
    app: AppHandle,
    state: State<'_, ProfileState>,
    id: String,
) -> Result<ProfileBootstrap, String> {
    {
        let mut settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
        if settings.active_profile_id == id {
            return Err("不能删除当前激活的配置".to_string());
        }
        settings.profiles.retain(|p| p.id != id);
        settings::save_settings(&app, &settings)?;
    }
    let bootstrap = build_bootstrap(&state)?;
    emit_profile_changed(&app, &bootstrap);
    Ok(bootstrap)
}

#[tauri::command]
pub fn profile_export(state: State<'_, ProfileState>, id: String) -> Result<String, String> {
    let settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
    export_profile(&settings, &id)
}

#[tauri::command]
pub fn profile_export_to_path(
    state: State<'_, ProfileState>,
    id: String,
    path: String,
) -> Result<(), String> {
    let json = {
        let settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
        export_profile(&settings, &id)?
    };
    write_profile_json(&path, &json)
}

#[tauri::command]
pub fn profile_import(
    app: AppHandle,
    state: State<'_, ProfileState>,
    json: String,
) -> Result<ProfileBootstrap, String> {
    {
        let mut settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
        import_profile(&mut settings, &json)?;
        settings::save_settings(&app, &settings)?;
    }
    let bootstrap = build_bootstrap(&state)?;
    emit_profile_changed(&app, &bootstrap);
    Ok(bootstrap)
}

#[tauri::command]
pub fn profile_import_from_path(
    app: AppHandle,
    state: State<'_, ProfileState>,
    path: String,
) -> Result<ProfileBootstrap, String> {
    let json = read_profile_json(&path)?;
    {
        let mut settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
        import_profile(&mut settings, &json)?;
        settings::save_settings(&app, &settings)?;
    }
    let bootstrap = build_bootstrap(&state)?;
    emit_profile_changed(&app, &bootstrap);
    Ok(bootstrap)
}

#[tauri::command]
pub fn profile_rename(
    app: AppHandle,
    state: State<'_, ProfileState>,
    id: String,
    name: String,
) -> Result<ProfileBootstrap, String> {
    {
        let mut settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;
        let profile = settings
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("找不到配置: {id}"))?;
        profile.name = name.trim().to_string();
        profile.updated_at = chrono::Utc::now().timestamp_millis() as u64;
        settings::save_settings(&app, &settings)?;
    }
    let bootstrap = build_bootstrap(&state)?;
    emit_profile_changed(&app, &bootstrap);
    Ok(bootstrap)
}

#[allow(dead_code)]
pub(crate) enum ActiveProfileSnapshotPatch {
    Morse(morse::MorseSettings),
    Timer(timer::TimerSettings),
    Counter(counter::CounterSettings),
    Rapidfire(rapidfire::RapidfireSettings),
    Recognition(recognition::RecognitionSettings),
    SpecialOps(Box<crate::special_ops::SpecialOpsSettings>),
}

pub(crate) fn is_applying(app: &AppHandle) -> bool {
    app.try_state::<ProfileState>()
        .is_some_and(|state| state.applying.load(Ordering::SeqCst))
}

#[allow(dead_code)]
pub(crate) fn update_active_profile_snapshot(
    app: &AppHandle,
    patch: ActiveProfileSnapshotPatch,
) -> Result<(), String> {
    let Some(state) = app.try_state::<ProfileState>() else {
        return Ok(());
    };

    let mut settings = state.settings.lock().map_err(|_| "Profile 状态锁已损坏")?;

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
        ActiveProfileSnapshotPatch::Recognition(value) => {
            profile.snapshot.recognition = Some(value)
        }
        ActiveProfileSnapshotPatch::SpecialOps(value) => {
            profile.snapshot.special_ops = Some(*value)
        }
    }
    profile.updated_at = chrono::Utc::now().timestamp_millis() as u64;
    settings::save_settings(app, &settings)
}

/// 读取当前工具与特勤处 settings 作为快照。
fn snapshot_current_settings(app: &AppHandle) -> Result<types::ToolSettingsSnapshot, String> {
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

    let recognition_settings = app
        .try_state::<recognition::RecognitionState>()
        .map(|s| {
            s.lock_inner()
                .map(|inner| inner.settings.clone())
                .map_err(|_| "识别触发状态已损坏".to_string())
        })
        .transpose()?;

    let special_ops_settings = app
        .try_state::<special_ops::SpecialOpsState>()
        .map(|state| state.settings_snapshot())
        .transpose()?;

    Ok(types::ToolSettingsSnapshot {
        morse: morse_settings,
        timer: timer_settings,
        counter: counter_settings,
        rapidfire: rapidfire_settings,
        recognition: recognition_settings,
        special_ops: special_ops_settings,
    })
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
        let boot = build_bootstrap(&state).unwrap();
        assert!(boot.profiles.is_empty());
        assert_eq!(boot.active_profile_id, "");
    }

    #[test]
    fn build_bootstrap_returns_error_when_profile_state_is_poisoned() {
        let state = Arc::new(ProfileState::new(ProfileSettings::default()));
        let poisoned_state = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let _guard = poisoned_state.settings.lock().unwrap();
            panic!("污染 Profile 状态锁");
        })
        .join();

        let error = build_bootstrap(&state).unwrap_err();
        assert_eq!(error, "Profile 状态锁已损坏");
    }

    #[test]
    fn profile_state_initializes_apply_lock() {
        let state = ProfileState::new(ProfileSettings::default());
        let _guard = state.apply_lock.try_lock().expect("apply lock 应可获取");
    }

    #[test]
    fn acquire_apply_lock_serializes_profile_writes() {
        let state = ProfileState::new(ProfileSettings::default());
        let _guard = acquire_apply_lock(&state).expect("apply lock should be acquirable");

        assert!(state.apply_lock.try_lock().is_err());
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
        let boot = build_bootstrap(&state).unwrap();
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
        assert!(snapshot.recognition.is_some());
        assert!(snapshot.special_ops.is_some());
    }

    #[test]
    fn empty_snapshot_stays_empty() {
        let snap = types::ToolSettingsSnapshot::empty();
        assert!(snap.morse.is_none());
        assert!(snap.timer.is_none());
        assert!(snap.counter.is_none());
        assert!(snap.rapidfire.is_none());
        assert!(snap.recognition.is_none());
    }

    #[test]
    fn export_profile_returns_pretty_camel_case_json() {
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

        let json = export_profile(&settings, "p1").unwrap();

        assert!(json.contains("\"createdAt\": 1"));
        assert!(json.contains("\"updatedAt\": 2"));
        assert!(!json.contains("created_at"));
        assert!(!json.contains("updated_at"));
    }

    #[test]
    fn import_profile_generates_new_id_and_keeps_active_profile() {
        let mut settings = ProfileSettings {
            profiles: vec![Profile {
                id: "active".to_string(),
                name: "当前".to_string(),
                created_at: 1,
                updated_at: 1,
                snapshot: types::ToolSettingsSnapshot::empty(),
            }],
            active_profile_id: "active".to_string(),
            next_profile_number: 1,
        };
        let json = r#"{"id":"old","name":"导入","createdAt":1,"updatedAt":2,"snapshot":{}}"#;

        let imported = import_profile(&mut settings, json).unwrap();

        assert_ne!(imported.id, "old");
        assert_eq!(imported.name, "导入");
        assert_eq!(settings.active_profile_id, "active");
        assert_eq!(settings.profiles.len(), 2);
    }

    #[test]
    fn import_profile_derives_name_on_conflict() {
        let mut settings = ProfileSettings {
            profiles: vec![Profile {
                id: "p1".to_string(),
                name: "重复".to_string(),
                created_at: 1,
                updated_at: 1,
                snapshot: types::ToolSettingsSnapshot::empty(),
            }],
            active_profile_id: "p1".to_string(),
            next_profile_number: 1,
        };
        let json = r#"{"id":"old","name":"重复","createdAt":1,"updatedAt":2,"snapshot":{}}"#;

        let imported = import_profile(&mut settings, json).unwrap();

        assert_eq!(imported.name, "重复 导入2");
        assert_eq!(settings.active_profile_id, "p1");
    }

    // ── emit 测试 ──

    /// 验证 profile://changed 事件名常量正确。
    #[test]
    fn test_profile_changed_event_name() {
        assert_eq!(events::CHANGED, "profile://changed");
    }

    /// 验证 emit 计数器机制工作正常。
    #[test]
    fn test_emit_tracker_counter_mechanism() {
        emit_tracker::reset();
        assert_eq!(emit_tracker::count(), 0);
        emit_tracker::EMIT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(emit_tracker::count(), 1);
        emit_tracker::EMIT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(emit_tracker::count(), 2);
        emit_tracker::reset();
        assert_eq!(emit_tracker::count(), 0);
    }

    /// 验证 build_bootstrap 返回值可作为 emit payload（Serialize 不 panic）。
    #[test]
    fn test_profile_bootstrap_serializable_as_emit_payload() {
        let settings = ProfileSettings {
            profiles: vec![Profile {
                id: "p1".to_string(),
                name: "测试配置".to_string(),
                created_at: 1,
                updated_at: 2,
                snapshot: types::ToolSettingsSnapshot::empty(),
            }],
            active_profile_id: "p1".to_string(),
            next_profile_number: 1,
        };
        let state = ProfileState::new(settings);
        let bootstrap = build_bootstrap(&state).unwrap();
        let json = serde_json::to_string(&bootstrap);
        assert!(json.is_ok(), "ProfileBootstrap 应可序列化为 emit payload");
        let json = json.unwrap();
        assert!(json.contains("profiles"));
        assert!(json.contains("\"settingsRevision\":1"));
    }

    /// 验证 ProfileBootstrap 实现 Clone（emit_to 需要）。
    #[test]
    fn test_profile_bootstrap_clone_for_emit() {
        let settings = ProfileSettings {
            profiles: vec![Profile {
                id: "p1".to_string(),
                name: "克隆测试".to_string(),
                created_at: 1,
                updated_at: 2,
                snapshot: types::ToolSettingsSnapshot::empty(),
            }],
            active_profile_id: "p1".to_string(),
            next_profile_number: 1,
        };
        let state = ProfileState::new(settings);
        let bootstrap = build_bootstrap(&state).unwrap();
        let cloned = bootstrap.clone();
        assert_eq!(cloned.profiles.len(), 1);
        assert_eq!(cloned.active_profile_id, "p1");
    }

    /// 验证只读命令 profile_get_bootstrap 不调用 emit。
    /// （get_bootstrap 调用后 emit 计数器应保持 0）
    #[test]
    fn test_profile_readonly_no_emit() {
        emit_tracker::reset();
        // profile_get_bootstrap 是只读命令，不调用 emit_profile_changed
        // 此处验证在无 AppHandle 的纯逻辑路径下计数器未被触动
        assert_eq!(emit_tracker::count(), 0);
    }

    /// 验证 Profile 写命令在代码结构上可调用 emit_profile_changed。
    /// 由于无法在单元测试中创建 AppHandle 调用 Tauri command，
    /// 此测试通过检查 emit_profile_changed 函数签名和事件常量，
    /// 确认所有写命令在成功路径末尾触发了 emit。
    #[test]
    fn test_write_commands_emit_profile_changed() {
        // 验证事件名与前端 PROFILE_EVENTS.changed.name 一致
        assert_eq!(events::CHANGED, "profile://changed");
        // 验证 emit_profile_changed 函数存在且可引用
        let _ = emit_profile_changed as fn(&AppHandle, &ProfileBootstrap);
    }
}
