# 背景与设计决策

Delta Auto Tools 的设计决策、陷阱与迁移背景。

## 设计决策

### 为什么用 Tauri 而非 Electron

应用需要底层 Windows 键盘钩子（通过 `willhook` 的 `WH_KEYBOARD_LL`）、截屏（`xcap`）和模拟键盘输入（`enigo`）。这些需要 Rust 直接提供的原生访问。Tauri 2 提供小体积二进制、原生性能和真正的 WebView2 渲染器，没有 Electron 的开销。

### 为什么用单一共享键盘钩子

多个键盘钩子会竞争，可能导致 Windows 上安装失败。`HotkeyManager` 在启动时安装一个 `willhook::keyboard_hook()`，将事件分发给所有工具 scope。这避免了「第二个钩子安装失败」问题，并集中了冲突检测。

### 为什么用 `?mode=` 而非路由

透明窗口（透明、点击穿透、置顶）是独立的 Tauri 窗口，加载相同的前端 bundle 但使用不同的查询参数。使用 `?mode=overlay` / `?mode=timer-display` 等让每个窗口渲染不同内容而无需路由器。这是无法被客户端路由替代的硬约束。

### 为什么用 bootstrap/form 双状态模式

前端需要显示 Rust 的规范态（用于展示）同时允许本地编辑（用于表单）。将它们保持为独立对象，用 `JSON.stringify` 脏检测，比逐字段 diff 更简单可靠。400ms autosave 防抖加版本守卫防止用户快速输入时的陈旧保存。

### 为什么计数器运行态独立持久化

计数器值随时间累积，应在应用重启后存活，但它们不是用户配置。存储在 `counter_state.json`（独立于 `counter_settings.json`）意味着修改 `start_value` 或热键不会重置累积计数，profile 切换可以重置计数而不触碰配置。

### 为什么策略网站用内嵌 WebView 而非 iframe/代理

iframe 被大多数攻略站点阻止（X-Frame-Options）。代理 HTML 会丢失 cookie、JavaScript 和 CAPTCHA 处理。主窗口内的真实 WebView2 子窗口提供完整浏览器能力（cookie、JS、localStorage、同源 API），同时留在应用壳层内。

### 为什么引入同步工具基座

计时器、计数器、连发器三个工具共享相同的生命周期模式：分组/条目规范化、热键重启、位置状态机、全局停止。v0.17.5 将这些重复实现提取到 `sync_tool.rs` 的 `SyncToolLogic` trait 中，减少了代码重复并确保行为一致。

## 陷阱

### AGENTS.md 已过时

AGENTS.md 和 CLAUDE.md 大量记录了一个已不存在的 `delta/` 模块。阅读这些文件的人会被不存在的命令、类型和前端页面困惑。文档与代码不一致时以代码和 `lib.rs` 为准。

### capabilities 中的 glob 模式

`src-tauri/capabilities/default.json` 列出前端允许调用的 Tauri 命令。忘记在此添加新命令会导致 `invoke()` 静默失败或抛出难以追踪的权限错误。

### 热键冲突边界情况

`AllowHold` 策略仅在计时器/计数器普通 scope 和连发器 hold scope 之间有效。Morse 使用 `Strict` 会拒绝任何其他 scope 使用的按键。新增工具 scope 时需仔细决定冲突策略并在 `hotkeys.rs` 中添加测试。

### 透明窗口渲染

透明叠加窗口不能继承主窗口的深色纸面 CSS。`document.body` 上的 `data-overlay-mode` 属性用于切换样式。将主窗口背景应用到 overlay 会使它们不透明并阻挡游戏视图。

### 序列化的旧字段

多个结构体有 `legacy_*` 字段（`#[serde(skip_serializing)]`），仅用于向后兼容反序列化。`normalize_settings` 将它们迁移到新字段。新增替换旧字段的字段时，遵循此模式否则旧 JSON 文件会加载失败。

## 迁移背景

Delta 模块移除是项目历史上最大的迁移。它移除了整个后端子系统（鉴权、游戏数据、存储、加密）和对应的前端页面。文档尚未跟上。在此代码库中工作时，始终验证 AGENTS.md 中提到的命令或页面是否实际存在于 `lib.rs` 或 `App.tsx` 中。
