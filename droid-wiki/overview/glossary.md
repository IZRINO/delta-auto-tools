# 术语表

Delta Auto Tools 中可能对新读者不直观的术语。

| 术语 | 含义 |
|------|------|
| Bootstrap | Rust 通过 `xxx_get_bootstrap` 返回给前端的初始状态。包含 settings 和运行态数据（runs、history、errors）。前端将其视为不可变规范态。 |
| Form | 前端本地可编辑草稿状态，从 bootstrap 派生。通过 `JSON.stringify` 比较做脏检测。 |
| Autosave | 表单与 bootstrap 分歧时触发的 400ms 防抖保存。通过 `autosaveVersionRef` 防止陈旧覆盖。 |
| 透明叠加窗（Overlay window） | 透明、无边框、置顶、点击穿透的 Tauri 窗口，用于游戏内显示。计时器、计数器、连发器各有自己的叠加窗。 |
| 位置窗口（Position window） | 拖拽校准叠加窗位置的窗口。通过 `?mode=*-position` 进入。 |
| 显示窗口（Display window） | 实际显示计时器/计数器/连发器数据的透明叠加窗。通过 `?mode=*-display` 进入。 |
| Scope | 命名的热键注册组（如 `"morse"`、`"timer"`、`"counter"`、`"rapidfire"`）。`HotkeyManager` 检测跨 scope 冲突。 |
| Hold action | 在按下和松开时都触发的热键（连发器使用），与仅在按下时触发一次的普通热键相对。 |
| ConflictPolicy | `Strict`（禁止跨 scope 复用按键）或 `AllowHold`（允许 hold scope 与普通 scope 在同键共存）。计时器/计数器和连发器使用 AllowHold；Morse 使用 Strict。 |
| KeySuppressor | 第二个 `WH_KEYBOARD_LL` 钩子，吞噬物理按键事件使其不到达前台应用，同时仍触发热键回调。懒加载启动。 |
| ToolBase | `src-tauri/src/tool_base.rs` 中的泛型层，通过 `ToolState<T: ToolLogic>` 为每个工具模块提供共享的 settings/bootstrap/error 处理。 |
| ToolLogic | 每个工具实现的 trait，接入 ToolBase：`load_settings`、`save_settings`、`build_bootstrap`、`emit_state`。 |
| SyncTool | `src-tauri/src/sync_tool.rs` 中的同步工具基座，扩展 ToolBase，为计时器/计数器/连发器提供分组规范化、热键重启、位置状态机、全局停止注册表。 |
| GlobalState | 单个 `AtomicBool` 开关。关闭时所有热键回调和自动化暂停，运行态会话停止。 |
| 区域选择（Region selection） | 用户拖拽选择屏幕区域（Morse）或识色探针区域（音频）的 overlay 流程。多步骤，使用 `oneshot` channel。 |
| Session | 单次连发器激活生命周期：按下创建 session，松开停止。每个 session 在独立 OS worker 线程运行。 |
| 补齐（Compensation） | 连发器卡片触发奇数次时，补齐逻辑额外触发一次使总数为偶数（除非卡片启用了不追加补齐）。 |
| ColorWatch | 音频触发模式，采样屏幕区域取平均 RGB，通过欧氏距离与目标色比较。支持 Average 和 AnyPixel 两种匹配方式。 |
| RegionWatch | 音频触发模式，对参考图像区域做归一化互相关（NCC）模板匹配。 |
| Theme | 一组 CSS 变量覆盖。3 套 daisyUI 内置主题加用户自定义主题，持久化到 `theme_settings.json`。通过 `document.documentElement` 的内联样式应用。 |
| Profile | 全部 5 个工具 settings 文件的快照。切换 profile 时写入 5 份 settings 到磁盘，重载内存状态，重置计数器运行值。 |
| IDE gateway | 已移除的 Delta 模块遗留概念。旧文档中的引用不适用于当前代码库。 |
