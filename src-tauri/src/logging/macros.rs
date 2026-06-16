//! 日志宏：log_error! / log_warn! / log_info! / log_debug! / log_trace!

/// 写入 ERROR 级别日志
///
/// 用法：
/// - `log_error!("morse::mod", "识别失败");`
/// - `log_error!("delta::commands", "请求超时", "endpoint" => "get_player", "duration_ms" => 5000);`
#[macro_export]
macro_rules! log_error {
    ($source:expr, $msg:expr $(, $key:expr => $val:expr)*) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Error,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            Some(serde_json::json!({ $($key: $val),* }))
        )
    };
    ($source:expr, $msg:expr) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Error,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            None
        )
    };
}

/// 写入 WARN 级别日志
#[macro_export]
macro_rules! log_warn {
    ($source:expr, $msg:expr $(, $key:expr => $val:expr)*) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Warn,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            Some(serde_json::json!({ $($key: $val),* }))
        )
    };
    ($source:expr, $msg:expr) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Warn,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            None
        )
    };
}

/// 写入 INFO 级别日志
#[macro_export]
macro_rules! log_info {
    ($source:expr, $msg:expr $(, $key:expr => $val:expr)*) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Info,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            Some(serde_json::json!({ $($key: $val),* }))
        )
    };
    ($source:expr, $msg:expr) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Info,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            None
        )
    };
}

/// 写入 DEBUG 级别日志
#[macro_export]
macro_rules! log_debug {
    ($source:expr, $msg:expr $(, $key:expr => $val:expr)*) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Debug,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            Some(serde_json::json!({ $($key: $val),* }))
        )
    };
    ($source:expr, $msg:expr) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Debug,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            None
        )
    };
}

/// 写入 TRACE 级别日志
#[macro_export]
macro_rules! log_trace {
    ($source:expr, $msg:expr $(, $key:expr => $val:expr)*) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Trace,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            Some(serde_json::json!({ $($key: $val),* }))
        )
    };
    ($source:expr, $msg:expr) => {
        $crate::logging::log_write(
            $crate::logging::LogLevel::Trace,
            $source,
            concat!(file!(), ":", line!()),
            $msg,
            None
        )
    };
}
