# 系统

跨工具模块共享的 Rust 基础设施。这些不是面向用户的功能，而是每个工具都依赖的架构构建块。

- [工具基座](tool-base.md) - 通用 `ToolState<T>` 层，统一处理 settings、bootstrap 与错误
- [同步工具基座](sync-tool.md) - 扩展 ToolBase，为计时器/计数器/连发器提供共享生命周期管理（v0.17.5 新增）
- [热键系统](hotkeys.md) - 共享 `HotkeyManager`，基于 willhook 键盘钩子，支持 scope 注册与冲突检测
- [按键抑制器](key-suppressor.md) - `WH_KEYBOARD_LL` 钩子，吞掉物理按键事件同时保留热键回调
- [透明叠加窗](overlay-windows.md) - 透明、点击穿透、置顶的窗口基础设施
- [全局总开关](global-state.md) - 单一开关，关闭时挂起所有自动化
- [日志系统](logging.md) - 文件日志，支持按天轮转、session ID 与链路追踪
- [主题引擎](theme-engine.md) - 3 套 daisyUI 内置主题，外加自定义主题与 CSS 变量覆盖
- [配置系统](profile-system.md) - 所有工具 settings 的多配置快照切换
