//! Profile 模块事件名字符串常量。

/// Profile 变更事件：写命令执行成功后推送最新 bootstrap 到 `main` 窗口。
/// 前端 listener 收到后刷新 ProfileProvider 状态。
pub const CHANGED: &str = "profile://changed";
