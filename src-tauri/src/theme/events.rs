//! 主题模块事件名字符串常量。

/// 主题变更事件：保存主题设置后推送合并后的最终 token 列表到 `main` 窗口。
/// 前端 listener 收到后遍历 token 调用 `document.documentElement.style.setProperty`。
pub const CHANGED: &str = "theme://changed";
