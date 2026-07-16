use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        LazyLock, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::de::DeserializeOwned;
use tauri::{AppHandle, Manager};

pub fn settings_path(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法解析配置目录: {error}"))?;

    ensure_config_dir(&config_dir)?;

    Ok(config_dir.join(file_name))
}

pub fn ensure_config_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("无法创建配置目录: {error}"))
}

pub fn load_settings<T: DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    if !path.exists() {
        crate::log_info!(
            "settings",
            "配置文件不存在，使用默认配置",
            "path" => path.display().to_string()
        );
        return Ok(T::default());
    }

    let content = fs::read_to_string(path).map_err(|error| {
        crate::log_warn!(
            "settings",
            "读取配置文件失败",
            "path" => path.display().to_string(),
            "error" => error.to_string()
        );
        format!("无法读取配置文件 {}: {error}", path.display())
    })?;

    match serde_json::from_str::<T>(&content) {
        Ok(settings) => Ok(settings),
        Err(error) => {
            let backup_path = backup_settings(path, "corrupt");
            crate::log_warn!(
                "settings",
                "配置 JSON 损坏，已回退默认配置",
                "path" => path.display().to_string(),
                "backup" => backup_path.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|error| error.clone()),
                "error" => error.to_string()
            );
            Ok(T::default())
        }
    }
}

pub fn backup_invalid_settings(path: &Path) -> Result<PathBuf, String> {
    backup_settings(path, "invalid")
}

fn backup_settings(path: &Path, kind: &str) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("无法解析配置文件名 {}", path.display()))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let backup_path = path.with_file_name(format!("{file_name}.{kind}-{timestamp}"));
    fs::rename(path, &backup_path).map_err(|error| {
        crate::log_warn!(
            "settings",
            "备份异常配置文件失败",
            "path" => path.display().to_string(),
            "backup" => backup_path.display().to_string(),
            "error" => error.to_string()
        );
        format!("无法备份损坏配置文件 {}: {error}", path.display())
    })?;
    Ok(backup_path)
}

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
// ponytail: replace 很短且配置写入低频；实测需要串行化 Windows MoveFileExW。
static SETTINGS_REPLACE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub struct SettingsCoordinator {
    revision: Mutex<u64>,
}

impl Default for SettingsCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsCoordinator {
    pub fn new() -> Self {
        Self {
            revision: Mutex::new(1),
        }
    }

    pub fn current_revision(&self) -> Result<u64, String> {
        self.revision
            .lock()
            .map(|revision| *revision)
            .map_err(|_| "配置写入协调器已损坏".to_string())
    }

    pub fn with_revision<T, E>(
        &self,
        expected_revision: u64,
        operation: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<String>,
    {
        let revision = self
            .revision
            .lock()
            .map_err(|_| E::from("配置写入协调器已损坏".to_string()))?;
        if *revision != expected_revision {
            return Err(E::from(format!(
                "配置保存已陈旧：页面 revision {expected_revision}，当前 revision {}",
                *revision
            )));
        }
        operation()
    }

    pub fn with_profile_change<T, E>(
        &self,
        operation: impl FnOnce(&mut bool) -> Result<T, E>,
    ) -> Result<(T, u64), E>
    where
        E: From<String>,
    {
        let mut revision = self
            .revision
            .lock()
            .map_err(|_| E::from("配置写入协调器已损坏".to_string()))?;
        let mut side_effect_started = false;
        let result = operation(&mut side_effect_started);
        if result.is_ok() || side_effect_started {
            *revision = revision.saturating_add(1);
        }
        result.map(|value| (value, *revision))
    }
}

fn temp_settings_path(path: &Path) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析配置文件目录 {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("配置文件名无效 {}", path.display()))?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        sequence
    )))
}

pub fn save_settings<T: serde::Serialize>(path: &Path, settings: &T) -> Result<(), String> {
    let content = serde_json::to_string_pretty(settings).map_err(|error| {
        crate::log_error!(
            "settings",
            "序列化配置失败",
            "path" => path.display().to_string(),
            "error" => error.to_string()
        );
        format!("无法序列化设置: {error}")
    })?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析配置文件目录 {}", path.display()))?;
    ensure_config_dir(parent)?;

    let temp_path = temp_settings_path(path)?;

    if let Err(error) = fs::write(&temp_path, content) {
        let _ = fs::remove_file(&temp_path);
        crate::log_error!(
            "settings",
            "写入临时配置文件失败",
            "path" => temp_path.display().to_string(),
            "target_path" => path.display().to_string(),
            "error" => error.to_string()
        );
        return Err(format!(
            "无法写入临时配置文件 {}: {error}",
            temp_path.display()
        ));
    }

    let _replace_guard = match SETTINGS_REPLACE_LOCK.lock() {
        Ok(guard) => guard,
        Err(_) => {
            let _ = fs::remove_file(&temp_path);
            return Err("配置文件替换锁已损坏".to_string());
        }
    };
    replace_settings_file(&temp_path, path).map_err(|error| {
        crate::log_error!(
            "settings",
            "替换配置文件失败",
            "path" => path.display().to_string(),
            "temp_path" => temp_path.display().to_string(),
            "error" => error.to_string()
        );
        format!("无法写入配置文件 {}: {error}", path.display())
    })
}

#[cfg(target_os = "windows")]
fn replace_settings_file(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let temp_wide = temp_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target_wide = target_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    let ok = unsafe {
        MoveFileExW(
            temp_wide.as_ptr(),
            target_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        let error = std::io::Error::last_os_error();
        let _ = fs::remove_file(temp_path);
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_settings_file(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    fs::rename(temp_path, target_path).map_err(|error| {
        let _ = fs::remove_file(temp_path);
        error.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct TestSettings {
        value: String,
    }

    #[test]
    fn load_settings_backs_up_corrupt_json_and_returns_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("broken_settings.json");
        fs::write(&path, "{ broken json").unwrap();

        let loaded = load_settings::<TestSettings>(&path).unwrap();

        assert_eq!(loaded, TestSettings::default());
        let backups = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("broken_settings.json.corrupt-")
            })
            .count();
        assert_eq!(backups, 1);
    }

    #[test]
    fn backup_invalid_settings_renames_file_with_invalid_suffix() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("bad_settings.json");
        fs::write(&path, "{\"valid_json\":\"bad_semantics\"}").unwrap();

        let backup = backup_invalid_settings(&path).unwrap();

        assert!(!path.exists());
        assert!(backup
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("bad_settings.json.invalid-"));
        assert_eq!(
            fs::read_to_string(backup).unwrap(),
            "{\"valid_json\":\"bad_semantics\"}"
        );
    }

    #[test]
    fn concurrent_writes_use_distinct_temp_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target = temp_dir.path().join("settings.json");

        let first = temp_settings_path(&target).unwrap();
        let second = temp_settings_path(&target).unwrap();

        assert_ne!(first, second);
        assert_eq!(first.parent(), target.parent());
        assert!(first
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".settings.json."));
        assert_eq!(
            first.extension().and_then(|value| value.to_str()),
            Some("tmp")
        );
    }

    #[test]
    fn thirty_two_concurrent_writers_leave_complete_json_without_temp_files() {
        use std::sync::{Arc, Barrier};

        let temp_dir = tempfile::tempdir().unwrap();
        let target = Arc::new(temp_dir.path().join("concurrent.json"));
        let barrier = Arc::new(Barrier::new(32));
        let writers = (0..32)
            .map(|index| {
                let target = Arc::clone(&target);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    save_settings(
                        target.as_ref(),
                        &TestSettings {
                            value: format!("writer-{index}"),
                        },
                    )
                })
            })
            .collect::<Vec<_>>();

        for writer in writers {
            writer.join().unwrap().unwrap();
        }

        let saved = load_settings::<TestSettings>(target.as_ref()).unwrap();
        assert!(saved.value.starts_with("writer-"));
        let leftovers = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "tmp")
            })
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn profile_revision_rejects_old_save_after_switch() {
        let coordinator = SettingsCoordinator::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let disk_path = temp_dir.path().join("profile-barrier.json");
        let runtime = Mutex::new(TestSettings {
            value: "initial".to_string(),
        });
        let profile_snapshot = Mutex::new(TestSettings {
            value: "initial".to_string(),
        });
        let initial_revision = coordinator.current_revision().unwrap();

        coordinator
            .with_profile_change(|side_effect_started| {
                *side_effect_started = true;
                runtime.lock().unwrap().value = "new-profile".to_string();
                save_settings(
                    &disk_path,
                    &TestSettings {
                        value: "new-profile".to_string(),
                    },
                )?;
                profile_snapshot.lock().unwrap().value = "new-profile".to_string();
                Ok::<(), String>(())
            })
            .unwrap();

        let stale = coordinator.with_revision(initial_revision, || {
            runtime.lock().unwrap().value = "stale-save".to_string();
            save_settings(
                &disk_path,
                &TestSettings {
                    value: "stale-save".to_string(),
                },
            )?;
            profile_snapshot.lock().unwrap().value = "stale-save".to_string();
            Ok::<(), String>(())
        });

        assert!(stale.unwrap_err().contains("陈旧"));
        assert_eq!(runtime.lock().unwrap().value, "new-profile");
        assert_eq!(
            load_settings::<TestSettings>(&disk_path).unwrap().value,
            "new-profile"
        );
        assert_eq!(profile_snapshot.lock().unwrap().value, "new-profile");
        assert_eq!(
            coordinator.current_revision().unwrap(),
            initial_revision + 1
        );
    }

    #[test]
    fn profile_revision_advances_when_change_fails_after_side_effect() {
        let coordinator = SettingsCoordinator::new();
        let initial_revision = coordinator.current_revision().unwrap();
        let value = Mutex::new("initial");

        let result = coordinator.with_profile_change(|side_effect_started| {
            *side_effect_started = true;
            *value.lock().unwrap() = "partial-profile";
            Err::<(), String>("应用 Profile 失败".to_string())
        });

        assert_eq!(result.unwrap_err(), "应用 Profile 失败");
        assert_eq!(
            coordinator.current_revision().unwrap(),
            initial_revision + 1
        );
        let stale = coordinator.with_revision(initial_revision, || {
            *value.lock().unwrap() = "stale-save";
            Ok::<(), String>(())
        });
        assert!(stale.unwrap_err().contains("陈旧"));
        assert_eq!(*value.lock().unwrap(), "partial-profile");
    }

    #[test]
    fn profile_revision_stays_when_change_fails_before_side_effect() {
        let coordinator = SettingsCoordinator::new();
        let initial_revision = coordinator.current_revision().unwrap();

        let result = coordinator.with_profile_change(|_side_effect_started| {
            Err::<(), String>("读取 Profile 失败".to_string())
        });

        assert_eq!(result.unwrap_err(), "读取 Profile 失败");
        assert_eq!(coordinator.current_revision().unwrap(), initial_revision);
    }
}
