# 模式与约定

## Rust serde

所有前端反序列化的 Rust 结构体必须使用 `#[serde(rename_all = "camelCase")]`。适用于 settings、bootstrap、运行态、事件和 DTO。Rust 与 TypeScript 间的大小写不匹配是最常见的 IPC bug。前端期望 camelCase key（如 `startValue`，而非 `start_value`）。

## 错误处理

所有命令返回 `Result<T, String>`，其中 String 是中文错误消息（如 `"摩斯状态已损坏"`）。统一的 `AppError` 类型（`src-tauri/src/app_error.rs`）序列化为字符串，前端行为与 String 错误一致。

Mutex 中毒时，工具返回中文「已损坏」错误。`ToolState::lock_inner` helper 集中处理此逻辑。

## Bootstrap/form 双状态

容器页（`morse-page.tsx`、`timer-page.tsx` 等）同时持有 `bootstrap` 状态（来自 Rust，不可变）和 `form` 状态（本地草稿）。两者通过 `JSON.stringify` 比较检测脏状态。`useBootstrapForm` hook（`src/hooks/use-bootstrap-form.ts`）封装此逻辑。form 分歧时，通过 `useAutosave`（`src/hooks/use-autosave.ts`）触发 400ms 防抖 autosave，调用工具的 `xxx_save_settings` 命令。

`autosaveVersionRef` 计数器防止陈旧保存覆盖新 form：每个保存请求携带排队时的版本号，如当前版本更高则丢弃保存。

## Settings/form 转换层

因 form 输入使用 string 而 Rust 使用 int，每个工具在 `*-utils.ts` 中有转换层：

- `settingsToForm()`：int -> string 供输入框
- `parseSettingsForm()`：验证并 string -> int 供 Rust

这使验证逻辑脱离渲染层，utils 可独立单元测试。

## 事件命名

事件名为字符串常量。后端在 `events.rs` 文件（`src-tauri/src/<tool>/events.rs`）中定义。前端在 `src/lib/tauri-events.ts` 中以字符串常量镜像（`MORSE_EVENTS`、`TIMER_EVENTS` 等），调用方使用显式泛型 `listen<PayloadType>(EVENTS.xxx, callback)`。两层均不硬编码事件名字符串。

## 原生 shell 检测

`useNativeShell()`（`src/hooks/use-native-shell.ts`）检查 `__TAURI_INTERNALS__`。浏览器预览模式禁用所有 `invoke()` 调用并显示提示。每个调用 Tauri command 的工具页应使用此 hook 守卫，使页面可在普通浏览器中渲染用于 UI 开发。

## 透明窗口

计时器、计数器、连发器的透明窗口必须无边框、透明、置顶、点击穿透。位置设置窗口（`?mode=*-position`）可使用校准靶风格。overlay 背景必须保持透明以保持游戏可见。不要将主窗口的深色纸面风格应用到 overlay。

## 热键冲突策略

- Morse 使用 `ConflictPolicy::Strict`：禁止跨任何 scope 复用按键
- 计时器和计数器使用 `ConflictPolicy::AllowHold`：可与连发器的 hold scope 共享按键
- 连发器使用 `ConflictPolicy::AllowHold`（hold scope）

运行时，hold Down/Up 事件先分发，然后普通热键事件，因此同一按键可同时触发连发器会话和计时器/计数器。

## 样式规则

- 仅使用 shadcn/ui 组件、Tailwind 工具类和 `src/App.css` 主题 token。禁止自定义 `.desktop-*` 或 `.tactical-*` CSS 类
- 全局 `--radius: 0`（90 度直角）。主窗口禁止圆角卡片、柔和阴影、玻璃态、渐变
- Amber（`#E8A000`）是唯一强调色，应占画面 3-8%。状态色（Rust、Moss）仅表达语义
- 图标使用 `@remixicon/react`。Button 内图标必须设置 `data-icon="inline-start"` 或 `"inline-end"`

## 文档中的文件引用

提及源文件时，始终使用从仓库根开始的完整路径（如 `src-tauri/src/morse/mod.rs`，而非 `mod.rs`）。短文件名在渲染文档中会产生断链。
