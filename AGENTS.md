# AGENTS.md

## Project reality

- 当前仓库是 **Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust** 的桌面工具，当前产品界面已是 Morse 识别工作台。
- 当前真实产品是 **Morse 识别工作台**：主界面负责设置、识别结果、历史记录；overlay 负责连续区域框选。
- 前端已接入 Tailwind CSS v4 与 shadcn/ui；这些不是“仅安装未使用”的状态，而是当前界面基础设施的一部分。
- 原生能力通过 Tauri commands 暴露，核心逻辑位于 `src-tauri/src/morse/*`，不是 HTTP 服务。

## Source of truth

优先相信可执行配置与当前代码，而不是旧文档：

1. `src-tauri/tauri.conf.json`
2. `package.json`
3. `CLAUDE.md`
4. `docs/CODEMAPS/`
5. `src/` 和 `src-tauri/src/`

如果 `README.md`、旧注释或历史描述与代码不一致，以当前实现为准。

## Commands

- `bun run dev` -> 仅前端 Vite 开发服务器
- `bun run build` -> `tsc && vite build`
- `bun run preview` -> Vite preview
- `bun run tauri dev` -> 完整桌面开发流程
- `bun run tauri build` -> 桌面构建流程
- `cargo check --manifest-path src-tauri/Cargo.toml` -> 检查 Rust/Tauri 编译

## Current architecture

- 前端入口链路：`index.html` -> `src/main.tsx` -> `src/App.tsx`
- 原生入口链路：`src-tauri/src/main.rs` -> `src-tauri/src/lib.rs`
- 前端核心：`src/components/app/morse-page.tsx`
- 原生核心：`src-tauri/src/morse/mod.rs`
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

## UI and workflow constraints

- 保持白色桌面工具风格，不要改回模板首页或营销页。
- `?mode=overlay` 必须继续可用，不要引入路由来替代它。
- 区域选择应保持“一次进入 overlay，连续完成多个框选”。
- overlay 必须保持透明背景，避免重灰幕遮挡底层屏幕内容。
- 热键输入应保持录制式交互；真正的解绑/重绑由 Rust 保存逻辑负责。
- `TooltipProvider` 已在 `src/main.tsx` 根部提供，依赖 tooltip 的组件应沿用该入口结构。

## Frontend conventions

- 使用现有别名：`@/components`、`@/components/ui`、`@/lib`、`@/hooks`
- Tailwind v4 使用 CSS-first 方案，主题 token 在 `src/App.css`
- 优先复用 `src/components/ui/*` 中已有基础组件
- `src/App.css` 同时承载主题 token、桌面壳层样式与 overlay 相关样式；修改时要区分普通模式与 overlay 模式

## Native-side conventions

- `src-tauri/src/morse/mod.rs` 负责状态、命令注册、热键协调与识别流程调度
- `src-tauri/src/morse/overlay.rs` 负责多步骤框选会话；中途取消不应污染已保存配置
- `src-tauri/src/morse/settings.rs` 的持久化文件是 `morse_settings.json`
- 修改原生命令时，必要时同步更新 `src-tauri/capabilities/default.json`

## Repo-specific cautions

- 使用 **Bun**，不要切换到 npm / pnpm / yarn
- 不要虚构仓库中不存在的 lint/test/CI 命令
- `README.md`、`AGENTS.md`、`CLAUDE.md` 和 `docs/CODEMAPS/` 需要随重大功能变更一起更新
- 忽略本地或生成产物：`node_modules`、`dist`、`src-tauri/target`、`.claude/worktrees/`、`test-results/`

## If the project changes again

如果后续新增：
- 新的 Tauri commands
- 新的持久化结构
- 新的开发脚本
- 路由系统或新的应用壳层

请在同一轮改动里同步更新 `README.md`、`AGENTS.md`、`CLAUDE.md` 与相关 codemap。
