//! 日志系统公共接口：LogLevel、LogSettings、FrontendLogRequest、
//! TraceContext、session_id、Tauri Commands、初始化与关闭

pub mod format;
pub mod macros;
pub mod writer;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::Manager;
pub use writer::LogWriter;

// ── LogLevel ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    /// 数值越大越详细，用于级别过滤
    pub fn value(&self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warn => 1,
            Self::Info => 2,
            Self::Debug => 3,
            Self::Trace => 4,
        }
    }
}

// ── LogSettings ──

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogSettings {
    pub global_level: LogLevel,
    #[serde(default)]
    pub module_levels: HashMap<String, LogLevel>,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            global_level: LogLevel::Info,
            module_levels: HashMap::new(),
        }
    }
}

impl LogSettings {
    /// 判断是否应该记录此级别日志
    pub fn should_log(&self, level: LogLevel, origin: &str) -> bool {
        let module = writer::extract_module(origin);
        let threshold = self
            .module_levels
            .get(module)
            .copied()
            .unwrap_or(self.global_level);
        level.value() <= threshold.value()
    }
}

// ── FrontendLogRequest ──

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendLogRequest {
    pub level: LogLevel,
    pub source: String,
    pub location: String,
    pub trace_id: String,
    pub message: String,
    pub payload: Option<Value>,
}

// ── TraceContext ──

thread_local! {
    static CURRENT_TRACE_ID: RefCell<String> = RefCell::new("--".to_string());
}

/// 当前线程的 trace_id 上下文
#[allow(dead_code)]
pub struct TraceContext;

#[allow(dead_code)]
impl TraceContext {
    /// 设置当前线程的 trace_id
    pub fn set(trace_id: &str) {
        CURRENT_TRACE_ID.with(|v| {
            if trace_id.is_empty() {
                *v.borrow_mut() = "--".to_string();
            } else {
                *v.borrow_mut() = trace_id.to_string();
            }
        });
    }

    /// 获取当前线程的 trace_id
    pub fn current() -> String {
        CURRENT_TRACE_ID.with(|v| v.borrow().clone())
    }

    /// 清除当前线程的 trace_id（恢复为 "--"）
    pub fn clear() {
        CURRENT_TRACE_ID.with(|v| *v.borrow_mut() = "--".to_string());
    }
}

// ── session_id ──

static SESSION_ID: LazyLock<String> = LazyLock::new(|| {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let chars: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut id = String::with_capacity(6);
    let mut remaining = seed;
    for _ in 0..6 {
        let idx = (remaining % 36) as usize;
        id.push(chars[idx] as char);
        remaining /= 36;
    }
    id
});

/// 获取本次运行实例的 session_id（6 位字母数字）
pub fn session_id() -> &'static str {
    &SESSION_ID
}

/// 全局日志写入入口（供宏调用）
///
/// 由宏自动注入 origin/location/trace_id/thread_id，
/// 业务代码不应直接调用此函数，应使用 log_error! 等宏。
#[allow(dead_code)]
pub fn log_write(
    level: LogLevel,
    source: &str,
    location: &str,
    message: &str,
    payload: Option<Value>,
) {
    let timestamp = chrono::Local::now();
    let origin = format!("[RUST]·{}", source);
    let trace_id = TraceContext::current();

    // 构建 payload，自动注入 thread_id
    let mut enriched_payload = payload.unwrap_or_default();
    if let Some(obj) = enriched_payload.as_object_mut() {
        // 注入 msg
        obj.insert("msg".to_string(), Value::String(message.to_string()));
        // 注入 thread_id
        let thread_id = format!("{:?}", std::thread::current().id());
        obj.insert("thread_id".to_string(), Value::String(thread_id));
        // DEBUG 级别注入 memory_kb
        if level == LogLevel::Debug {
            if let Ok(mem) = get_process_memory_kb() {
                obj.insert("memory_kb".to_string(), Value::Number(mem.into()));
            }
        }
    } else {
        enriched_payload = serde_json::json!({
            "msg": message,
            "thread_id": format!("{:?}", std::thread::current().id()),
        });
    }

    let line = format::format_log_line(format::LogLine {
        timestamp: &timestamp,
        level,
        origin: &origin,
        location,
        trace_id: &trace_id,
        session_id: session_id(),
        message,
        payload: Some(&enriched_payload),
    });

    if let Some(app_handle) = GLOBAL_APP_HANDLE.get() {
        if let Some(log_writer) = app_handle.try_state::<LogWriter>() {
            log_writer.write(level, &origin, &line);
        }
    }
}

// ── 进程内存（Windows）──

#[cfg(target_os = "windows")]
fn get_process_memory_kb() -> Result<u64, String> {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };

    let process: HANDLE = unsafe { windows_sys::Win32::System::Threading::GetCurrentProcess() };
    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

    if unsafe { GetProcessMemoryInfo(process, &mut counters, size) } != 0 {
        Ok((counters.WorkingSetSize / 1024) as u64)
    } else {
        Err("GetProcessMemoryInfo 失败".to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn get_process_memory_kb() -> Result<u64, String> {
    Err("非 Windows 平台不支持内存查询".to_string())
}

// ── 全局 AppHandle 存储 ──

use std::sync::OnceLock;
static GLOBAL_APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

// ── 初始化 ──

/// 初始化日志系统
///
/// 返回 LogWriter 实例，由 app.manage() 注册
pub fn init_logger(app_handle: &tauri::AppHandle) -> Result<LogWriter, String> {
    let _ = GLOBAL_APP_HANDLE.set(app_handle.clone());

    let log_dir = writer::resolve_log_dir(app_handle);
    let settings = load_log_settings(app_handle);

    let writer = LogWriter::new(log_dir.clone(), settings);

    // 记录实际日志目录
    let timestamp = chrono::Local::now();
    let message = format!("日志目录: {}", log_dir.display());
    let payload = serde_json::json!({"msg": message, "log_dir": log_dir.display().to_string()});
    let line = format::format_log_line(format::LogLine {
        timestamp: &timestamp,
        level: LogLevel::Info,
        origin: "[RUST]·logging",
        location: "mod.rs:init",
        trace_id: "--",
        session_id: session_id(),
        message: &message,
        payload: Some(&payload),
    });
    writer.write(LogLevel::Info, "[RUST]·logging", &line);

    Ok(writer)
}

/// 关闭日志系统（flush）
pub fn shutdown(log_writer: &LogWriter) {
    log_writer.shutdown();
}

/// 加载日志配置
fn load_log_settings(app_handle: &tauri::AppHandle) -> LogSettings {
    let config_dir = match app_handle.path().app_config_dir() {
        Ok(dir) => dir,
        Err(_) => return LogSettings::default(),
    };

    let settings_path = config_dir.join("log_settings.json");
    if settings_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&settings_path) {
            if let Ok(settings) = serde_json::from_str::<LogSettings>(&content) {
                return settings;
            }
        }
    }

    LogSettings::default()
}

/// 保存日志配置
fn save_log_settings(app_handle: &tauri::AppHandle, settings: &LogSettings) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {}", e))?;
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("无法创建配置目录: {}", e))?;
    let settings_path = config_dir.join("log_settings.json");
    let json =
        serde_json::to_string_pretty(settings).map_err(|e| format!("无法序列化日志配置: {}", e))?;
    std::fs::write(&settings_path, json).map_err(|e| format!("无法写入日志配置: {}", e))
}

// ── Tauri Commands ──

/// 前端日志写入命令
#[tauri::command]
pub fn log_write_frontend(request: FrontendLogRequest) -> Result<(), String> {
    let timestamp = chrono::Local::now();
    let origin = format!("[FE]·{}", request.source);
    let trace_tag = if request.trace_id.is_empty() {
        "--".to_string()
    } else {
        request.trace_id
    };
    let session = session_id().to_string();

    // 构建 payload：确保 msg 字段存在
    let mut payload = request.payload.unwrap_or_default();
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("msg".to_string(), Value::String(request.message.clone()));
    } else {
        payload = serde_json::json!({"msg": request.message});
    }

    let line = format::format_log_line(format::LogLine {
        timestamp: &timestamp,
        level: request.level,
        origin: &origin,
        location: &request.location,
        trace_id: &trace_tag,
        session_id: &session,
        message: &request.message,
        payload: Some(&payload),
    });

    if let Some(app_handle) = GLOBAL_APP_HANDLE.get() {
        if let Some(log_writer) = app_handle.try_state::<LogWriter>() {
            log_writer.write(request.level, &origin, &line);
        }
    }

    Ok(())
}

/// 获取当前运行实例的 session_id
#[tauri::command]
pub fn log_get_session_id() -> String {
    session_id().to_string()
}

/// 更新日志级别设置
#[tauri::command]
pub fn log_set_level(app_handle: tauri::AppHandle, settings: LogSettings) -> Result<(), String> {
    // 保存到文件
    save_log_settings(&app_handle, &settings)?;

    // 更新内存中的过滤阈值
    if let Some(log_writer) = app_handle.try_state::<LogWriter>() {
        log_writer.set_settings(settings);
    }

    Ok(())
}

/// 获取当前日志级别设置
#[tauri::command]
pub fn log_get_level(app_handle: tauri::AppHandle) -> LogSettings {
    let log_writer = app_handle.state::<LogWriter>();
    log_writer.get_settings()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_value_ordering() {
        assert!(LogLevel::Error.value() < LogLevel::Warn.value());
        assert!(LogLevel::Warn.value() < LogLevel::Info.value());
        assert!(LogLevel::Info.value() < LogLevel::Debug.value());
        assert!(LogLevel::Debug.value() < LogLevel::Trace.value());
    }

    #[test]
    fn test_log_level_serde() {
        let json = serde_json::to_string(&LogLevel::Info).unwrap();
        assert_eq!(json, "\"info\"");
        let parsed: LogLevel = serde_json::from_str("\"warn\"").unwrap();
        assert_eq!(parsed, LogLevel::Warn);
    }

    #[test]
    fn test_log_settings_default() {
        let settings = LogSettings::default();
        assert_eq!(settings.global_level, LogLevel::Info);
        assert!(settings.module_levels.is_empty());
    }

    #[test]
    fn test_log_settings_should_log_global() {
        let settings = LogSettings::default(); // global = Info
        assert!(settings.should_log(LogLevel::Error, "[RUST]·test"));
        assert!(settings.should_log(LogLevel::Info, "[RUST]·test"));
        assert!(!settings.should_log(LogLevel::Debug, "[RUST]·test"));
    }

    #[test]
    fn test_log_settings_should_log_module_override() {
        let mut settings = LogSettings::default();
        settings
            .module_levels
            .insert("morse".to_string(), LogLevel::Debug);
        // morse 模块允许 Debug
        assert!(settings.should_log(LogLevel::Debug, "[RUST]·morse::mod"));
        // 其他模块走全局 Info
        assert!(!settings.should_log(LogLevel::Debug, "[RUST]·delta::commands"));
    }

    #[test]
    fn test_frontend_log_request_deserialize() {
        let json = r#"{
            "level": "info",
            "source": "timer-page",
            "location": "TimerPage:autosave",
            "traceId": "a7f3",
            "message": "热键解析失败",
            "payload": {"hotkey": "Ctrl+invalid", "error": "Unrecognized key"}
        }"#;
        let req: FrontendLogRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.level, LogLevel::Info);
        assert_eq!(req.source, "timer-page");
        assert_eq!(req.trace_id, "a7f3");
        assert!(req.payload.is_some());
    }

    #[test]
    fn test_trace_context_set_get_clear() {
        TraceContext::set("abcd");
        assert_eq!(TraceContext::current(), "abcd");
        TraceContext::clear();
        assert_eq!(TraceContext::current(), "--");
    }

    #[test]
    fn test_trace_context_empty_set() {
        TraceContext::set("");
        assert_eq!(TraceContext::current(), "--");
        TraceContext::clear();
    }

    #[test]
    fn test_session_id_format() {
        let id = session_id();
        assert_eq!(id.len(), 6);
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_log_settings_serde_roundtrip() {
        let mut settings = LogSettings::default();
        settings
            .module_levels
            .insert("morse".to_string(), LogLevel::Debug);
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: LogSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.global_level, LogLevel::Info);
        assert_eq!(parsed.module_levels.get("morse"), Some(&LogLevel::Debug));
    }
}
