# AGENTS.md

## Project reality

- 当前仓库是 **Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust** 的桌面工具，当前产品界面主线仍是 Morse 识别工作台。
- 当前真实产品由两部分原生能力组成：
  1. **Morse 识别工作台**：主界面负责设置、识别结果、历史记录；overlay 负责连续区域框选。
  2. **Delta 工具接口层**：通过 Tauri commands 暴露 Wegame 认证、QQ/微信/QQSafe 鉴权和游戏数据查询能力，当前阶段以原生命令与存储为主，尚未接入前端页面。
- 前端已接入 Tailwind CSS v4 与 shadcn/ui；这些不是“仅安装未使用”的状态，而是当前界面基础设施的一部分。
- 原生能力通过 Tauri commands 暴露，核心逻辑位于 `src-tauri/src/morse/*` 与 `src-tauri/src/delta/*`，不是 HTTP 服务。

## Source of truth

优先相信可执行配置与当前代码，而不是旧文档：

1. `src-tauri/tauri.conf.json`
2. `package.json`
3. `src/` 和 `src-tauri/src/`
4. `components.json`（shadcn/ui 配置）

如果 `README.md`、旧注释或历史描述与代码不一致，以当前实现为准。

## Commands

- `bun run dev` -> 仅前端 Vite 开发服务器（端口 1420，strictPort）
- `bun run build` -> `tsc && vite build`
- `bun run preview` -> Vite preview
- `bun run tauri dev` -> 完整桌面开发流程（先启动 Vite dev server，再启动 Tauri）
- `bun run tauri build` -> 桌面构建流程
- `bun run test` -> Vitest 单元测试
- `bun run test:coverage` -> 前端覆盖率输出（仅覆盖 `src/components/app/morse-utils.ts`）
- `cargo check --manifest-path src-tauri/Cargo.toml` -> 检查 Rust/Tauri 编译
- `cargo test --manifest-path src-tauri/Cargo.toml` -> Rust 单元测试

PM2 开发编排（`ecosystem.config.cjs`）：将 Vite 和 Tauri 拆为两个独立 PM2 进程，`delta-auto-tools-tauri` 启动前等待端口 1420。

## Current architecture

- 前端入口链路：`index.html` -> `src/main.tsx` -> `src/App.tsx`
- 原生入口链路：`src-tauri/src/main.rs` -> `src-tauri/src/lib.rs`
- 前端核心容器：`src/components/app/morse-page.tsx`
- 前端纯逻辑：`src/components/app/morse-utils.ts`
- 原生核心：`src-tauri/src/morse/mod.rs`
- Delta 原生入口：`src-tauri/src/delta/mod.rs`
- Delta Tauri 命令边界：`src-tauri/src/delta/commands.rs`
- Delta 服务层：`src-tauri/src/delta/services/*`
- Delta 存储层：`src-tauri/src/delta/storage/repo.rs`
- Delta 公共客户端：`src-tauri/src/delta/client/*`
- Overlay 状态机：`src-tauri/src/morse/overlay.rs`
- 识别链路：`src-tauri/src/morse/recognition.rs`
- 设置持久化：`src-tauri/src/morse/settings.rs`

当前命令面不是 `greet`，而是：
- `morse_get_bootstrap`
- `morse_save_settings`
- `morse_begin_region_selection`
- `morse_overlay_submit_selection`
- `morse_overlay_cancel_selection`
- `morse_run_recognition`

Delta 命令面当前包括：
- 账号与鉴权：`delta_list_accounts`、`delta_delete_account`、`delta_qq_*`、`delta_wechat_*`、`delta_qqsafe_*`
- Wegame：`delta_wegame_qq_*`、`delta_wegame_wechat_*`、`delta_wegame_open_treasure_gift`、`delta_wegame_draw_daily_card`
- 游戏数据：`delta_game_get_items`、`delta_game_get_config`、`delta_game_get_price`、`delta_game_get_firearm_mod_list`、`delta_game_get_recommendation`、`delta_game_get_record`、`delta_game_get_player`、`delta_game_get_assets`、`delta_game_get_logs`、`delta_game_get_recent`、`delta_game_get_achievement`、`delta_game_get_password`、`delta_game_get_manufacture`、`delta_game_get_guns`、`delta_game_get_bind`

## UI and workflow constraints

- 保持白色桌面工具风格，不要改回模板首页或营销页。
- `?mode=overlay` 必须继续可用，不要引入路由来替代它。
- 区域选择应保持“一次进入 overlay，连续完成多个框选”。
- overlay 必须保持透明背景，避免重灰幕遮挡底层屏幕内容。
- 热键输入应保持录制式交互；真正的解绑/重绑由 Rust 保存逻辑负责。
- `TooltipProvider` 已在 `src/main.tsx` 根部提供，依赖 tooltip 的组件应沿用该入口结构。

## Frontend conventions

- 使用现有别名：`@/components`、`@/components/ui`、`@/lib`、`@/hooks`
- Tailwind v4 使用 CSS-first 方案（`@import "tailwindcss"`），主题 token 在 `src/App.css`；**不存在** `tailwind.config.js`
- 优先复用 `src/components/ui/*` 中已有基础组件（基于 shadcn/ui，`radix-mira` 风格，remixicon 图标库）
- `src/components/app/morse-page.tsx` 负责容器与状态编排；展示块拆在 app 子组件中，纯逻辑放 `morse-utils.ts`
- `src/App.css` 同时承载主题 token、桌面壳层样式与 overlay 相关样式；修改时要区分普通模式与 overlay 模式
- 类型定义集中在 `src/components/app/morse-types.ts`，前端类型使用 camelCase（与 Rust `#[serde(rename_all = "camelCase")]` 对齐）
- `src/lib/utils.ts` 导出 `cn()` 函数（`clsx` + `tailwind-merge`），所有 shadcn/ui 组件通过它合并 className
- TypeScript strict mode 开启，含 `noUnusedLocals` / `noUnusedParameters` — 未使用的变量/参数会导致编译失败
- 通过 `window.__TAURI_INTERNALS__` 检测是否在 Tauri 原生壳中运行（`isNativeShell`），非原生环境跳过 Tauri invoke
- 表单使用 `MorseSettingsForm`（字符串字段）与 `MorseSettings`（数字字段）互转，parseSettingsForm 做校验

## Native-side conventions

- `src-tauri/src/morse/mod.rs` 负责状态、命令注册、热键协调与识别流程调度
- `src-tauri/src/morse/overlay.rs` 负责多步骤框选会话；中途取消不应污染已保存配置
- `src-tauri/src/morse/settings.rs` 的持久化文件是 `morse_settings.json`
- 修改原生命令时，必要时同步更新 `src-tauri/capabilities/default.json`
- `src-tauri/src/delta/commands.rs` 负责 Delta DTO、Tauri commands、账号解析与持久化编排
- `src-tauri/src/delta/services/` 下按领域拆分 QQ / WeChat / QQSafe / Wegame / Game 逻辑，不要额外引入与仓库现状不一致的 `models/handlers` 架构
- `src-tauri/src/delta/storage/repo.rs` 使用单表 `delta_accounts` 承载不同账号类型；新增账号类型应优先扩展 `AccountKind`
- `src-tauri/src/delta/client/ide.rs` 负责 IDE 网关表单请求，`src-tauri/src/delta/utils/game.rs` 负责枪械/弹药/配件映射与 bind-role 解析

### Rust serde conventions（全仓通用）

所有对外序列化的 Rust 结构体 **必须** 使用 `#[serde(rename_all = "camelCase")]`：
- Delta 端：`ApiResponse<T>`、`DeltaAccountRecord`、`AccountKind`、服务 DTO（`QqLoginQr`、`WechatQr` 等）、请求 struct（`CommandOptions`、`AccountCookieRequest` 等）
- Morse 端：`MorseSettings`、`MorseBootstrap`、`RegionRect`、`MorseRunResult`、`HistoryEntry`、`RegionSelectionProgress`、`RegionSelectionOutcome`

### Delta 命令返回模式（与 Morse 不同）

- Delta 命令返回 `Result<ApiResponse<T>, DeltaError>`，其中 `ApiResponse` 携带 `code`/`msg`/`data`，成功时 `code=0`，msg 为中文描述
- Morse 命令返回 `Result<T, String>`，直接返回数据或中文错误字符串
- `DeltaError` 显式实现了 `Serialize`（序列化为错误信息字符串），用于 Tauri IPC 传输

### Delta 命令 DTO 模式

每个 Delta command 都有对应的请求 struct，前端通过 camelCase JSON 反序列化：
- `AccountCookieRequest`：支持显式 `cookie` 或 `accountId` 两种指定方式
- 所有请求 DTO 都带有可选 `options: Option<CommandOptions>` 字段（目前仅 `insecureSkipTlsVerify`）

### reqwest HTTP 客户端模式

`delta::client::http::build_client()` 创建 reqwest Client 时固定使用：
- `cookie_provider`（Arc\<Jar\>，由调用方持有复用）
- `default_headers(browser_headers())`（模拟浏览器）
- `redirect(Policy::none())`（不自动跟随重定向）
- `danger_accept_invalid_certs` 可选（通过 `HttpOptions` 控制）

### Delta 状态管理

`DeltaState` 使用多个独立 `Mutex` 分别保护不同数据：
- `buckets: Mutex<HashMap<i64, DeltaAccountRecord>>` — 内存账号缓存，与 DB 同步
- `pending: Mutex<HashMap<String, PendingSession>>` — 扫码登录中会话（QQ/Wegame QQ）
- `http_options: Mutex<HttpOptions>` — 全局 HTTP 选项（预留）

账号持久化使用 `persist_account()` 辅助函数（`commands.rs:339-364`）：DB upsert + 内存 buckets 同步更新。

### Morse 状态管理（与 Delta 不同）

`MorseState` 使用 **单个** `Mutex<MorseStateInner>` 包裹所有可变字段，外加独立的 `Mutex<Option<PassiveHotkeyListener>>`。锁被污染时返回中文错误 "已损坏"。

### 被动热键监听（Windows only）

`src-tauri/src/morse/input_listener.rs` 使用 `willhook` crate 注册底层键盘钩子：
- 独立线程轮询键盘事件，匹配热键组合（modifier + primary key）
- 录制热键时通过 `morse_set_hotkey_recording` 暂停监听（`set_paused(true)`），并 drain 积压事件
- 非 Windows 平台直接返回错误，不做降级处理
- 热键绑定 parser 支持：`Ctrl+Shift+F2`、`F1`、`Ctrl+Alt+K` 等格式

### GameService 编译期配置

`GameService` 构造时通过 `include_str!()` 嵌入本地 PHP 配置文件（ammo.php、accessory.php），在编译期解析为 `HashMap`。运行时不再读取文件。这些文件位于仓库根目录。

## Repo-specific cautions

- 使用 **Bun**，不要切换到 npm / pnpm / yarn
- 不要虚构仓库中不存在的 lint/test/CI 命令
- `README.md`、`AGENTS.md`、`CLAUDE.md` 和 `docs/CODEMAPS/` 需要随重大功能变更一起更新（注：`docs/CODEMAPS/` 当前不存在）
- 仓库当前允许提交项目级 skills 目录：`.agents/skills/` 与 `.claude/skills/`；不要把它们误当成本地垃圾直接删除
- 忽略本地或生成产物：`node_modules`、`dist`、`src-tauri/target`、`.claude/worktrees/`、`.claude/settings.local.json`、`temp/`、`test-results/`
- 不存在 `tailwind.config.js` — Tailwind v4 通过 CSS `@import "tailwindcss"` 配置
- `GameService` 需要仓库根目录下的 `ammo.php` 和 `accessory.php` 才能在编译期嵌入；若缺少这些文件会导致 Rust 编译失败（LSP 报错 `os error 2`）
- 前端仅对 `src/components/app/morse-utils.ts` 有测试覆盖（`morse-utils.test.ts`）；Vitest coverage 配置也只包含该文件

## Tauri 事件模式

Morse 通过 Tauri events 通知前端（emit_to "main"）：
- `"morse://run-finished"` — 识别完成后推送 `MorseRunResult`
- `"morse://selection-progress"` — 区域选择完成后推送 `RegionSelectionProgress`
- `"morse://hotkey-error"` — 热键执行出错时推送错误字符串

前端通过 `listen()` from `@tauri-apps/api/event` 订阅这些事件。

## Overlay 状态机

overlay 框选流程使用 `oneshot::Sender<RegionSelectionKind>` 实现完成通知：
1. 前端调用 `morse_begin_region_selection`，Rust 创建 overlay 窗口（label: `"morse-overlay"`），存储 `PendingSelection`（含 oneshot sender）
2. 前端在 overlay 窗口中完成框选，调用 `morse_overlay_submit_selection`
3. Rust 更新 staged_regions，全部完成后保存 settings 并发送 sender
4. 主窗口通过 await 在 oneshot receiver 上等待完成，拿到最终的 `RegionSelectionOutcome`

overlay 窗口通过 `?mode=overlay&slots=0,1,2` 查询参数进入 overlay 模式。

## If the project changes again

如果后续新增：
- 新的 Tauri commands
- 新的持久化结构
- 新的开发脚本
- 路由系统或新的应用壳层
- 新的项目级 skills / agents 目录约定

请在同一轮改动里同步更新 `README.md`、`AGENTS.md` 与相关 codemap。
