# Issues #71 / #75 最终修复设计

## 背景

Issue #71 的现象是：卸载重装后初始配置正常；添加各工具卡片并重启后，配置异常，主题不可用。旧修复只处理了 JSON 语法损坏，并把前端误报“浏览器预览模式”改成“主题状态未注册”。用户复测后仍报主题不可用，说明根因不是主题面板本身，而是 Tauri setup 在 theme state 注册前被前置工具初始化错误打断。

Issue #75 的现象是：Recognition 按键效果序列第一步使用录制按钮，后续步骤使用输入框，交互风格不统一。用户选择方案 A：所有步骤统一使用录制。

## 目标

1. 启动阶段任一工具配置异常都不能阻断 theme/profile/global 等 state 注册。
2. 配置 JSON 语法损坏和语义非法都要能恢复：备份异常文件，回退默认配置，继续启动。
3. 恢复动作必须有可追踪日志，包含工具名、配置文件、错误原因、备份路径。
4. Theme 不再只表现为“state 未注册”；如果后端确实无法启动，要给出更接近根因的错误。
5. Recognition 按键效果序列每一步都使用录制交互。

## 非目标

- 不做全局 health center 页面。
- 不改配置文件格式。
- 不改变各工具 save_settings 的严格校验语义；用户手动保存非法配置仍应报错。
- 不关闭 GitHub issue，等待用户复测确认。

## 方案

### 1. 配置恢复能力

在 Rust 公共 settings 层增加语义异常备份能力：

- 保留现有 JSON 解析失败恢复：`<file>.corrupt-<timestamp>`。
- 新增语义异常恢复：`<file>.invalid-<timestamp>`。
- 备份失败仍返回错误，因为权限/磁盘问题不应被吞掉。

实现形式保持小而明确：

- `backup_invalid_settings(path) -> Result<PathBuf, String>` 或通用 `backup_settings(path, suffix)`。
- 日志级别使用 warn，字段包括 `tool`、`path`、`backup`、`error`。

### 2. 启动初始化恢复

当前 `lib.rs` setup 顺序是 Morse、Timer、Counter、Rapidfire、Recognition、Theme、Profile。前五个工具任一 `initialize()?` 失败，ThemeState 就不会注册，前端只能看到 theme state not managed。

修复策略：

- Theme/Profile/Global 不能被工具配置异常拖死。
- 对每个工具初始化使用专用恢复 wrapper：
  1. 正常调用 `tool::initialize`。
  2. 失败时备份该工具配置文件为 `.invalid-<timestamp>`。
  3. 再用默认配置重试初始化。
  4. 重试仍失败则返回错误，让真正不可恢复问题暴露。

每个工具需要提供配置文件名或恢复入口：

- Morse：`morse_settings.json`
- Timer：`timer_settings.json`
- Counter：`counter_settings.json`
- Rapidfire：`rapidfire_settings.json`
- Recognition：`recognition_settings.json`；旧 `audio_settings.json` 迁移失败也按 recognition 恢复处理。

恢复只作用于启动期。保存期仍保留严格校验，避免把非法用户输入静默改成默认。

### 3. 初始化错误可见性

启动恢复成功后，应用应能打开，Theme 应可用。具体工具回退默认后，用户能通过日志看到恢复记录。现阶段不新增 UI health center。

Theme 前端调整：

- `theme_get_bootstrap` 遇到 `state not managed` 时，不再暗示“主题配置坏”。
- 文案改为“后端启动未完成或初始化失败，请查看启动日志”。如果恢复 wrapper 生效，此路径理论上只剩不可恢复启动失败。

### 4. Recognition 按键效果序列统一录制

修改 `RecognitionRecordingTarget`：

```ts
type RecognitionRecordingTarget = {
  cardId: string;
  field: "triggerHotkey" | "activationHotkey" | "effectHotkey";
  stepIndex?: number;
} | null;
```

修改录制提交逻辑：

- 触发热键和激活热键保持现状。
- 按键效果根据 `stepIndex` 更新对应 `hotkeyEffectSteps[index].hotkey`。
- `effectHotkey` 始终同步为第 0 步 hotkey，保持旧字段兼容。

UI：

- 每个按键 step 都使用 `HotkeyField`。
- `id` 使用 `${card.id}-effect-hotkey-${stepIndex}`。
- delay 输入框、删除按钮、新增按键逻辑保持现状。
- 录制期间继续调用 `recognition_set_hotkey_recording(true)` 暂停 recognition scope。

## 数据流

启动：

```text
Tauri setup
  -> init_with_recovery("timer", "timer_settings.json", timer::initialize)
      -> normalize/settings/window/hotkey success: manage state
      -> failure: backup timer_settings.json.invalid-<ts>, retry default
  -> theme::initialize
  -> profile::initialize
  -> app.manage(...)
```

Recognition 按键序列：

```text
用户点击第 N 步录制
  -> recordingTarget = { cardId, field: "effectHotkey", stepIndex: N }
  -> useHotkeyRecorder 捕获 hotkey
  -> updateEffectHotkeyById(cardId, hotkey, N)
  -> parseSettingsForm 序列化 steps
  -> Rust validate_hotkey_duplicates 校验监听热键冲突和防递归
```

## 错误处理

- JSON 语法错误：备份 `.corrupt-*`，默认配置继续启动。
- 语义非法：备份 `.invalid-*`，默认配置重试启动。
- 备份失败：中断启动并返回错误，避免掩盖权限/磁盘问题。
- 默认配置重试失败：中断启动。这代表代码级缺陷或环境级问题，不能继续伪装可用。
- Hotkey 注册失败：沿用现有 hotkey_error，不阻断工具 state 注册。

## 测试计划

Rust：

- `settings` 新增 `.invalid-*` 备份测试。
- 启动恢复 helper：配置语义非法时备份并用默认配置重试。
- Timer/Counter/Rapidfire/Morse/Recognition 至少覆盖一个代表性非法配置恢复路径。
- 确认 save_settings 非法配置仍返回错误，不走启动恢复。

Frontend：

- Recognition 页面：第 2 个按键 step 录制后只更新第 2 步，不污染第 1 步。
- 删除第 1 步后 `effectHotkey` 同步为新的第 1 步。
- 所有 step 渲染为录制按钮，不再混用输入框。

集成验证：

- `bun run test`
- `bun run build`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- 手工构造非法 `timer_settings.json` 后启动：应用能打开，Theme 可用，原文件被备份为 `.invalid-*`。

## 发布与 issue 回复

修复完成后发布新 beta。回复 #71 和 #75：

- #71：说明配置语义非法也已恢复，不再让前置工具初始化拖死 theme。
- #75：说明按键效果序列已统一为每步录制。

两个 issue 都保持 open，等用户复测确认。
