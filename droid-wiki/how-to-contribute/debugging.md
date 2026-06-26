# 调试

## 日志文件

应用将日志写入 `logs/delta-{yyyyMMdd}.log`（回退到 `%LocalAppData%\org.izrino.delta-auto-tools\logs\`）。Rust 和前端日志写入同一批文件。每行包含时间戳、级别、来源（`[RUST]·{source}` 或 `[FE]`）、file:line、trace_id、session_id、消息。

使用 `log_get_session_id` 获取当前运行的 6 字符 session ID，然后在日志文件中搜索该 ID 查看本次会话的全部活动。

### 调整日志级别

使用 `log_set_level` 提高详细度。`LogSettings` 支持全局级别加按模块覆盖（如 `"morse": "debug"`）。变更持久化到 `log_settings.json`。

### 前端 console 劫持

生产构建中 `console.log/warn/error` 被包装，同时写入日志文件。开发模式（`bun run dev`）下 console 正常工作，不写文件。

## 常见问题

### 热键不触发

1. 检查全局开关是否开启（顶栏，绿色 = 开启）。见 [全局总开关](../systems/global-state.md)。
2. 检查 bootstrap 响应中的 `hotkey_error`：如 willhook 安装失败，所有热键禁用。
3. 检查冲突：保存 settings 时如按键与其他 scope 冲突会返回中文错误字符串。见 [热键系统](../systems/hotkeys.md)。
4. Windows 上确认杀毒软件或系统权限未阻止 `WH_KEYBOARD_LL` 钩子。

### 透明窗口不可见

1. 检查工具的 enabled 标志是否开启（如 `timer_enabled`）
2. 检查显示窗口是否已创建（日志中查找 `timer-display` / `counter-display` / `rapidfire-display` label）
3. 检查窗口位置是否在屏幕内（位置设置时未被拖出屏幕）
4. 透明窗口在深色背景上可能难以看到；内容使用 chalk 色文字

### Autosave 覆盖

如 settings 似乎回退，`autosaveVersionRef` 可能不同步。autosave hook（`src/hooks/use-autosave.ts`）丢弃版本号旧于当前 form 版本的保存。如 bootstrap 被重新获取（如热键错误事件后），form 从新 bootstrap 重置，版本也重置。

### Serde 大小写不匹配

如前端收到某字段为 `undefined`，检查 Rust 结构体是否使用 `#[serde(rename_all = "camelCase")]`，前端类型是否期望 camelCase key。这是最常见的 IPC bug。

### Mutex 中毒

如命令返回「已损坏」错误，说明 Mutex 因前次锁定期间 panic 而中毒。需要重启应用。正常操作不应发生；如发生，查看日志找出导致 panic 的原因。

## Tauri 开发工具

开发模式（`bun run tauri dev`）下，WebView2 devtools 可用（右键 -> Inspect）。如安装了 React DevTools，可检查前端状态。
