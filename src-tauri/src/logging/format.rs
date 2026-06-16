//! 日志行格式化：混合格式（人类可读 | JSON 结构化）

use chrono::{DateTime, Local};
use serde_json::Value;

use super::LogLevel;

/// 格式化时间戳为 `yyyy-MM-dd HH:mm:ss.SSS +ZZZZ`
fn format_timestamp(dt: &DateTime<Local>) -> String {
    let ms = dt.timestamp_subsec_millis();
    format!(
        "{}.{:03} {}",
        dt.format("%Y-%m-%d %H:%M:%S"),
        ms,
        dt.format("%:z")
    )
}

/// 级别标签：左对齐 5 字符
fn format_level(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN ",
        LogLevel::Info => "INFO ",
        LogLevel::Debug => "DEBUG",
        LogLevel::Trace => "TRACE",
    }
}

/// 截断字符串到指定宽度（字符数），超出部分截断尾部
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let mut truncated: String = s.chars().take(max_len).collect();
        truncated.truncate(truncated.len()); // 确保 UTF-8 安全
        truncated
    }
}

/// 格式化完整日志行
///
/// 格式：`{timestamp} | {level} | {origin} | {location} | {trace} | {session} | {message} | {json_payload}`
pub fn format_log_line(
    timestamp: &DateTime<Local>,
    level: LogLevel,
    origin: &str,
    location: &str,
    trace_id: &str,
    session_id: &str,
    message: &str,
    payload: Option<&Value>,
) -> String {
    let ts = format_timestamp(timestamp);
    let lvl = format_level(level);
    let origin_trunc = truncate(origin, 24);
    let loc_trunc = truncate(location, 20);
    let trace_tag = if trace_id.is_empty() {
        "--".to_string()
    } else {
        trace_id.to_string()
    };

    // 构建 payload JSON
    let json_part = match payload {
        Some(v) => {
            let mut obj = v.clone();
            if let Some(map) = obj.as_object_mut() {
                // 确保 msg 字段与 message 一致
                map.insert("msg".to_string(), Value::String(message.to_string()));
            } else {
                // payload 不是 object，用 message 替代
                obj = serde_json::json!({"msg": message});
            }
            format!("| {}", obj)
        }
        None => format!("| {{\"msg\":\"{}\"}}", message.replace('"', "\\\"")),
    };

    format!(
        "{} | {} | {} | {} | trace:{} | sess:{} | {} {}",
        ts, lvl, origin_trunc, loc_trunc, trace_tag, session_id, message, json_part
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp_has_millis() {
        let dt = Local::now();
        let formatted = format_timestamp(&dt);
        // 应包含毫秒部分 `.XXX`
        assert!(formatted.contains('.'));
        // 时区偏移
        assert!(formatted.contains('+') || formatted.contains('-'));
    }

    #[test]
    fn test_format_level_alignment() {
        assert_eq!(format_level(LogLevel::Error), "ERROR");
        assert_eq!(format_level(LogLevel::Warn), "WARN ");
        assert_eq!(format_level(LogLevel::Info), "INFO ");
        assert_eq!(format_level(LogLevel::Debug), "DEBUG");
        assert_eq!(format_level(LogLevel::Trace), "TRACE");
        // 所有级别 5 字符宽度
        for level in [
            LogLevel::Error,
            LogLevel::Warn,
            LogLevel::Info,
            LogLevel::Debug,
            LogLevel::Trace,
        ] {
            assert_eq!(format_level(level).chars().count(), 5);
        }
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("abc", 10), "abc");
    }

    #[test]
    fn test_truncate_exact() {
        assert_eq!(truncate("abcdefghij", 10), "abcdefghij");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("abcdefghijklm", 10), "abcdefghij");
    }

    #[test]
    fn test_format_log_line_basic() {
        let dt = Local::now();
        let line = format_log_line(
            &dt,
            LogLevel::Info,
            "[RUST]·morse::mod",
            "mod.rs:142",
            "a7f3",
            "8k2m9p",
            "识别完成，结果: 1234",
            Some(&serde_json::json!({
                "ctx": {"result": "1234", "regions": 3},
                "duration_ms": 234
            })),
        );

        // 检查各段存在
        assert!(line.contains("INFO"));
        assert!(line.contains("[RUST]·morse::mod"));
        assert!(line.contains("mod.rs:142"));
        assert!(line.contains("trace:a7f3"));
        assert!(line.contains("sess:8k2m9p"));
        assert!(line.contains("识别完成，结果: 1234"));
        assert!(line.contains("\"msg\":\"识别完成，结果: 1234\""));
        assert!(line.contains("\"duration_ms\":234"));
    }

    #[test]
    fn test_format_log_line_no_trace() {
        let dt = Local::now();
        let line = format_log_line(
            &dt,
            LogLevel::Error,
            "[RUST]·delta::commands",
            "commands.rs:88",
            "",
            "8k2m9p",
            "查询失败",
            None,
        );

        assert!(line.contains("trace:--"));
        assert!(line.contains("查询失败"));
    }

    #[test]
    fn test_format_log_line_origin_truncation() {
        let dt = Local::now();
        let long_origin = "[RUST]·delta::services::game::very_long_module_name";
        let line = format_log_line(
            &dt,
            LogLevel::Info,
            long_origin,
            "mod.rs:1",
            "abcd",
            "123456",
            "测试截断",
            None,
        );

        // origin 不应超过 24 字符
        let parts: Vec<&str> = line.split(" | ").collect();
        assert!(parts[2].chars().count() <= 24);
    }

    #[test]
    fn test_format_log_line_location_truncation() {
        let dt = Local::now();
        let long_loc = "very_long_file_name_with_many_chars.rs:999";
        let line = format_log_line(
            &dt,
            LogLevel::Info,
            "[RUST]·test",
            long_loc,
            "abcd",
            "123456",
            "测试截断",
            None,
        );

        // location 不应超过 20 字符
        let parts: Vec<&str> = line.split(" | ").collect();
        assert!(parts[3].chars().count() <= 20);
    }

    #[test]
    fn test_format_log_line_payload_msg_matches() {
        let dt = Local::now();
        let payload = serde_json::json!({"ctx": {"key": "val"}});
        let line = format_log_line(
            &dt,
            LogLevel::Info,
            "[RUST]·test",
            "test.rs:1",
            "abcd",
            "123456",
            "我的消息",
            Some(&payload),
        );

        // payload 中的 msg 应与 message 一致
        assert!(line.contains("\"msg\":\"我的消息\""));
        assert!(line.contains("\"key\":\"val\""));
    }
}
