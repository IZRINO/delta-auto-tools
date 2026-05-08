use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T>
where
    T: Serialize,
{
    pub code: i32,
    pub msg: String,
    pub data: T,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    pub fn ok(msg: impl Into<String>, data: T) -> Self {
        Self {
            code: 0,
            msg: msg.into(),
            data,
        }
    }

    pub fn of(code: i32, msg: impl Into<String>, data: T) -> Self {
        Self {
            code,
            msg: msg.into(),
            data,
        }
    }
}
