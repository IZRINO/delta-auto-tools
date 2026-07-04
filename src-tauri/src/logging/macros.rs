//! 日志宏：log_error!

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
