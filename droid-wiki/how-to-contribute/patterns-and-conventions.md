# 模式与约定

## Rust serde

所有前端反序列化的 Rust 结构体必须使用 `#[serde(rename_all = "camelCase")]`。适用于 settings、bootstrap、运行态、事件和 DTO。Rust 与 TypeScript 间的大小写不匹配是最常见的 IPC bug。前端期望 camelCase key（如 `startValue`，而非 `start_value`）。

## 错误处理

所有命令返回 `Result<T, String>`，其中 String 是中文错误消息（如 `"摩斯状态已损坏"`）。统一的 `AppError` 类型（`src-tauri/src/app_error.rs`）序列化为字符串，前端行为与 String 错误一致。

Mutex 中毒时，工具返回中文「已损坏」错误。`ToolState::lock_inner` helper 集中处理此逻辑。

## Bootstrap/form 双状态

容器页（`morse-page.tsx`、`timer-page.tsx` 等）同时持有 `bootstrap` 状态（来自 Rust，不可变）和 `form` 状态（本地草稿）。两者通过 `JSON.stringify` 比较检测脏状态。`useBootstrapForm` hook（`src/hooks/use-bootstrap-form.ts`）封装此逻辑。form 分歧时，通过 `useAutosave`（`src/hooks/use-autosave.ts`）触发 400ms 防抖 autosave，调用工具的 `xxx_save_settings` 命令。

`LatestSaveQueue` 保证每个工具最多只有一个 in-flight save；保存期间继续编辑时，等待区只保留最新 snapshot。当前 save 失败不会阻断等待中的最新 snapshot，各 caller 只接收自己所属保存批次的结果。`autosaveVersionRef` 仅阻止旧响应回写较新的本地 form，不承担后端并发控制。

所有会持久化 5 类工具 settings 的命令都必须携带 Profile `settingsRevision`，包括主 `xxx_save_settings` 与 position/region overlay commit。Rust `SettingsCoordinator::with_revision` 在同一 guard 内完成磁盘写入、runtime 更新和 active Profile snapshot 更新；Profile 切换通过 `with_profile_change` 串行执行并递增 revision，旧页面写入在产生副作用前返回陈旧错误。

## Settings/form 转换层

因 form 输入使用 string 而 Rust 使用 int，每个工具在 `*-utils.ts` 中有转换层：

- `settingsToForm()`：int -> string 供输入框
- `parseSettingsForm()`：验证并 string -> int 供 Rust

这使验证逻辑脱离渲染层，utils 可独立单元测试。

## 事件命名

事件名为字符串常量。后端在 `events.rs` 文件（`src-tauri/src/<tool>/events.rs`）中定义。前端在 `src/lib/tauri-events.ts` 中以字符串常量镜像（`MORSE_EVENTS`、`TIMER_EVENTS` 等），调用方使用显式泛型 `subscribeTauriEvent<PayloadType>(EVENTS.xxx, callback)`。两层均不硬编码事件名字符串。

生产代码统一通过 `src/lib/tauri-listener.ts` 的 `subscribeTauriEvent` 订阅 Tauri 事件，不直接调用 `@tauri-apps/api/event` 的 `listen`。helper 同步返回幂等 cleanup；即使 React cleanup 早于 `listen()` Promise resolve，listener 就绪后也会立即 unlisten。helper 还会阻止 disposed 后的业务 callback，并消费 `listen()` reject；调用方需要展示订阅错误时传入 `onError`。

## 原生 shell 检测

`useNativeShell()`（`src/hooks/use-native-shell.ts`）检查 `__TAURI_INTERNALS__`。浏览器预览模式禁用所有 `invoke()` 调用。预览提示由应用壳 `PagePreviewBanner` 说一次，工具页不要再各自挂一条。每个调用 Tauri command 的工具页应使用此 hook 守卫，使页面可在普通浏览器中渲染用于 UI 开发。

## 透明窗口

计时器、计数器、连发器的透明窗口必须无边框、透明、置顶、点击穿透。位置设置窗口（`?mode=*-position`）可使用校准靶风格。overlay 背景必须保持透明以保持游戏可见。不要将主窗口的深色纸面风格应用到 overlay。

## 热键冲突策略

- Morse 使用 `ConflictPolicy::Strict`：禁止跨任何 scope 复用按键
- 计时器和计数器使用 `ConflictPolicy::AllowHold`：可与连发器的 hold scope 共享按键
- 连发器使用 `ConflictPolicy::AllowHold`（hold scope）

运行时，hold Down/Up 事件先分发，然后普通热键事件，因此同一按键可同时触发连发器会话和计时器/计数器。

## 样式规则

- 保留 Radix headless 组件的交互能力，视觉层优先使用 daisyUI class、Tailwind 工具类和 `src/App.css` daisyUI token。禁止自定义旧桌面/战术风格 CSS 类
- 主题 token 以 daisyUI 语义 token 为主：`--color-base-*`、`--color-primary`、`--color-error`、`--radius-*`、`--border` 等。禁止新增旧组件生成器/工业桥接 token
- `--border` 在 daisyUI 中表示边框宽度；边框颜色使用 `base-300`、`primary`、`error` 等 daisyUI 语义色
- 图标使用 `@remixicon/react`。Button 内图标必须设置 `data-icon="inline-start"` 或 `"inline-end"`

## 文档中的文件引用

提及源文件时，始终使用从仓库根开始的完整路径（如 `src-tauri/src/morse/mod.rs`，而非 `mod.rs`）。短文件名在渲染文档中会产生断链。
