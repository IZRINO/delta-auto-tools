//! 日志写入器：BufWriter + Mutex + 按天轮转 + 清理 + 级别过滤

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use chrono;
use tauri::Manager;

use super::{LogLevel, LogSettings};

const MAX_LOG_AGE_DAYS: u64 = 30;
const MAX_LOG_DIR_SIZE_MB: u64 = 100;

struct LogWriterInner {
    writer: BufWriter<File>,
    current_date: String, // "20250616"
    log_dir: PathBuf,
}

pub struct LogWriter {
    inner: Mutex<Option<LogWriterInner>>,
    settings: Mutex<LogSettings>,
}

impl LogWriter {
    /// 创建 LogWriter，初始化日志目录并清理过期文件
    pub fn new(log_dir: PathBuf, settings: LogSettings) -> Self {
        // 清理过期文件
        cleanup_old_logs(&log_dir);

        // 确保目录存在
        let _ = fs::create_dir_all(&log_dir);

        let today = chrono::Local::now().format("%Y%m%d").to_string();
        let inner = Self::create_inner(&log_dir, &today);

        Self {
            inner: Mutex::new(inner.ok()),
            settings: Mutex::new(settings),
        }
    }

    /// 创建内部 writer
    fn create_inner(log_dir: &PathBuf, date_str: &str) -> Result<LogWriterInner, String> {
        let filename = format!("delta-{}.log", date_str);
        let path = log_dir.join(&filename);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("无法打开日志文件 {:?}: {}", path, e))?;
        Ok(LogWriterInner {
            writer: BufWriter::new(file),
            current_date: date_str.to_string(),
            log_dir: log_dir.clone(),
        })
    }

    /// 写入一行日志
    pub fn write(&self, level: LogLevel, origin: &str, formatted_line: &str) {
        if !self.should_log(level, origin) {
            return;
        }

        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return, // 锁已损坏，静默丢弃
        };

        let Some(inner) = guard.as_mut() else {
            return;
        };

        // 检查是否需要轮转
        let today = chrono::Local::now().format("%Y%m%d").to_string();
        if inner.current_date != today {
            // 先 flush 旧文件
            let _ = inner.writer.flush();
            // 创建新文件
            match Self::create_inner(&inner.log_dir, &today) {
                Ok(new_inner) => {
                    *inner = new_inner;
                }
                Err(_) => return,
            }
        }

        // 写入并 flush（每行立即落盘，确保崩溃安全）
        let _ = inner.writer.write_all(formatted_line.as_bytes());
        let _ = inner.writer.write_all(b"\n");
        let _ = inner.writer.flush();
    }

    /// 判断是否应该记录此级别日志
    pub fn should_log(&self, level: LogLevel, origin: &str) -> bool {
        let settings = match self.settings.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        settings.should_log(level, origin)
    }

    /// 更新日志设置
    pub fn set_settings(&self, new_settings: LogSettings) {
        if let Ok(mut guard) = self.settings.lock() {
            *guard = new_settings;
        }
    }

    /// 获取当前日志设置
    pub fn get_settings(&self) -> LogSettings {
        match self.settings.lock() {
            Ok(g) => g.clone(),
            Err(e) => e.into_inner().clone(),
        }
    }

    /// 关闭时 flush
    pub fn shutdown(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(inner) = guard.as_mut() {
                let _ = inner.writer.flush();
            }
        }
    }
}

/// 从 origin 提取模块名用于级别过滤
/// "[RUST]·morse::mod" → "morse"
/// "[FE]·timer-page" → "timer"
pub fn extract_module(origin: &str) -> &str {
    // 去掉 [RUST]· 或 [FE]· 前缀
    let rest = if let Some(idx) = origin.find('·') {
        &origin[idx + '·'.len_utf8()..]
    } else {
        origin
    };
    // 取第一个 :: 或 - 之前的部分
    if let Some(idx) = rest.find("::") {
        &rest[..idx]
    } else if let Some(idx) = rest.find('-') {
        &rest[..idx]
    } else {
        rest
    }
}

/// 清理过期日志文件
fn cleanup_old_logs(log_dir: &PathBuf) {
    let Ok(entries) = fs::read_dir(log_dir) else {
        return;
    };

    let now = SystemTime::now();
    let max_age = std::time::Duration::from_secs(MAX_LOG_AGE_DAYS * 24 * 3600);

    let mut files: Vec<(PathBuf, SystemTime, u64)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "log") {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let size = metadata.len();

        // 删除超过 30 天的文件
        if let Ok(age) = now.duration_since(modified) {
            if age > max_age {
                let _ = fs::remove_file(&path);
                continue;
            }
        }

        files.push((path, modified, size));
    }

    // 检查总大小
    let total_size: u64 = files.iter().map(|(_, _, s)| *s).sum();
    let max_size = MAX_LOG_DIR_SIZE_MB * 1024 * 1024;

    if total_size > max_size {
        // 按修改时间从旧到新排序
        files.sort_by_key(|(_, mtime, _)| *mtime);

        let mut current_size = total_size;
        for (path, _, size) in files {
            if current_size <= max_size {
                break;
            }
            let _ = fs::remove_file(&path);
            current_size -= size;
        }
    }
}

/// 确定日志目录路径
///
/// 优先使用软件安装目录，失败则回退到 app_local_data_dir
pub fn resolve_log_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    // 主路径：软件安装目录
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let log_dir = parent.join("logs");
            if fs::create_dir_all(&log_dir).is_ok() {
                // 测试写入
                let test_file = log_dir.join(".write_test");
                if let Ok(mut f) = File::create(&test_file) {
                    if f.write_all(b"test").is_ok() {
                        let _ = fs::remove_file(&test_file);
                        return log_dir;
                    }
                }
                let _ = fs::remove_file(&test_file);
            }
        }
    }

    // 回退路径：app_local_data_dir
    if let Some(data_dir) = app_handle.path().app_local_data_dir().ok() {
        let log_dir = data_dir.join("logs");
        let _ = fs::create_dir_all(&log_dir);
        return log_dir;
    }

    // 最终回退
    std::env::current_dir().unwrap_or_default().join("logs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_module_rust() {
        assert_eq!(extract_module("[RUST]·morse::mod"), "morse");
        assert_eq!(extract_module("[RUST]·delta::commands"), "delta");
        assert_eq!(extract_module("[RUST]·delta::services::game"), "delta");
    }

    #[test]
    fn test_extract_module_frontend() {
        assert_eq!(extract_module("[FE]·timer-page"), "timer");
        assert_eq!(extract_module("[FE]·morse-page"), "morse");
        assert_eq!(extract_module("[FE]·counter-utils"), "counter");
    }

    #[test]
    fn test_extract_module_no_prefix() {
        assert_eq!(extract_module("morse::mod"), "morse");
    }

    #[test]
    fn test_cleanup_old_logs_removes_expired() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        // 创建正常文件（不超期）
        let recent_file = dir_path.join("delta-20990101.log");
        fs::write(&recent_file, "recent").unwrap();

        // cleanup 不应删除当天附近的文件
        cleanup_old_logs(&dir_path);
        assert!(recent_file.exists());
    }

    #[test]
    fn test_log_writer_basic_write() {
        let dir = tempfile::tempdir().unwrap();
        let settings = LogSettings::default();
        let writer = LogWriter::new(dir.path().to_path_buf(), settings);

        writer.write(
            LogLevel::Info,
            "[RUST]·test",
            "2025-06-16 14:32:01.234 +0800 | INFO  | [RUST]·test | test.rs:1 | trace:abcd | sess:123456 | 测试日志 | {\"msg\":\"测试日志\"}",
        );
        writer.shutdown();

        // 验证文件存在且有内容
        let today = chrono::Local::now().format("%Y%m%d").to_string();
        let log_file = dir.path().join(format!("delta-{}.log", today));
        assert!(log_file.exists());
        let content = fs::read_to_string(&log_file).unwrap();
        assert!(content.contains("测试日志"));
    }

    #[test]
    fn test_log_writer_level_filter() {
        let dir = tempfile::tempdir().unwrap();
        let settings = LogSettings {
            global_level: LogLevel::Warn,
            module_levels: std::collections::HashMap::new(),
        };
        let writer = LogWriter::new(dir.path().to_path_buf(), settings);

        assert!(writer.should_log(LogLevel::Error, "[RUST]·test"));
        assert!(writer.should_log(LogLevel::Warn, "[RUST]·test"));
        assert!(!writer.should_log(LogLevel::Info, "[RUST]·test"));
        assert!(!writer.should_log(LogLevel::Debug, "[RUST]·test"));
        assert!(!writer.should_log(LogLevel::Trace, "[RUST]·test"));
    }

    #[test]
    fn test_log_writer_module_level_override() {
        let dir = tempfile::tempdir().unwrap();
        let mut module_levels = std::collections::HashMap::new();
        module_levels.insert("morse".to_string(), LogLevel::Debug);
        let settings = LogSettings {
            global_level: LogLevel::Warn,
            module_levels,
        };
        let writer = LogWriter::new(dir.path().to_path_buf(), settings);

        // morse 模块允许 Debug
        assert!(writer.should_log(LogLevel::Debug, "[RUST]·morse::mod"));
        // 其他模块走全局级别 Warn
        assert!(!writer.should_log(LogLevel::Info, "[RUST]·delta::commands"));
    }
}
