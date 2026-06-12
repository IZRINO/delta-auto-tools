use serde::Serialize;
use thiserror::Error;

use crate::delta::error::DeltaError;

/// 统一应用错误类型接缝。
///
/// Morse / Timer / Rapidfire / Strategy 命令返回 `Result<T, AppError>`，
/// 内部仍返回 `Result<T, String>`，通过 `From<String>` 在 `?` 处自动转换。
/// Delta 命令返回 `Result<ApiResponse<T>, AppError>`，`DeltaError` 通过 `#[from]` 自动转换。
///
/// 序列化为字符串，前端行为与当前 String 错误一致。
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Delta(#[from] DeltaError),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Message(s)
    }
}

/// 允许 `&str` 字面量直接通过 `.into()` 转换（主要用于测试）
impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Message(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::AppError;
    use crate::delta::error::DeltaError;

    #[test]
    fn message_serializes_as_string() {
        let err = AppError::Message("摩斯状态已损坏".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"摩斯状态已损坏\"");
    }

    #[test]
    fn delta_error_wraps_transparently() {
        let err = AppError::Delta(DeltaError::Parse("解析失败".to_string()));
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("解析失败"));
    }

    #[test]
    fn from_string_converts_to_message() {
        let err: AppError = "测试错误".to_string().into();
        assert!(matches!(err, AppError::Message(s) if s == "测试错误"));
    }

    #[test]
    fn from_str_literal_converts() {
        let err: AppError = "字面量".into();
        assert!(matches!(err, AppError::Message(s) if s == "字面量"));
    }
}
