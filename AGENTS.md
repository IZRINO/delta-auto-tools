# AGENTS.md

## Project reality

- 当前仓库是 **Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust** 的桌面工具，当前产品界面主线仍是 Morse 识别工作台。
- 当前真实产品由两部分原生能力组成：
  1. **Morse 识别工作台**：主界面负责设置、识别结果、历史记录；overlay 负责连续区域框选。
  2. **Delta 工具接口层**：通过 Tauri commands 暴露 Wegame 认证、QQ/微信/QQSafe 鉴权和游戏数据查询能力，当前阶段以原生命令与存储为主，尚未接入前端页面。
- 前端已接入 Tailwind CSS v4 与 shadcn/ui；这些不是“仅安装未使用”的状态，而是当前界面基础设施的一部分。
- 原生能力通过 Tauri commands 暴露，核心逻辑位于 `src-tauri/src/morse/*` 与 `src-tauri/src/delta/*`，不是 HTTP 服务。

## AI 输出规范

- **所有 AI 输出必须使用中文**，包括代码注释、解释说明、错误提示和用户交互内容
- 技术术语（如 React、TypeScript、Tauri 等）保持英文原名，其余描述使用中文
- 代码中的字符串、错误信息、UI 文案使用中文
- 生成的文档、注释、commit message 使用中文

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
- `bun run test` -> Vitest 单元测试
- `bun run test:coverage` -> 前端覆盖率输出
- `cargo check --manifest-path src-tauri/Cargo.toml` -> 检查 Rust/Tauri 编译
- `cargo test --manifest-path src-tauri/Cargo.toml` -> Rust 单元测试

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

## UI and Styling Rules

- **仅使用 shadcn/ui 组件和 Tailwind CSS 进行样式设计**
- **禁止自定义 CSS 类** - 不得创建 `.desktop-*` 或其他自定义 CSS 类
- 所有样式必须通过以下方式实现：
  - shadcn/ui 组件（Button、Card、Badge 等）
  - Tailwind 工具类（`bg-primary`、`text-foreground`、`rounded-lg` 等）
  - 仅在绝对必要时使用内联样式（例如动态定位）
- `src/App.css` 中的 @theme 块定义的主题令牌是颜色的唯一来源
- 当现有 shadcn/ui 组件无法满足需求时，应组合使用它们而不是编写自定义 CSS

## Frontend conventions

- 使用现有别名：`@/components`、`@/components/ui`、`@/lib`、`@/hooks`
- Tailwind v4 使用 CSS-first 方案，主题 token 在 `src/App.css`
- 优先复用 `src/components/ui/*` 中已有基础组件
- `src/components/app/morse-page.tsx` 负责容器与状态编排；展示块拆在 app 子组件中，纯逻辑放 `morse-utils.ts`
- `src/App.css` 仅承载主题 token 与 overlay 相关样式；所有桌面壳层样式改用 shadcn/ui + Tailwind

## Native-side conventions

- `src-tauri/src/morse/mod.rs` 负责状态、命令注册、热键协调与识别流程调度
- `src-tauri/src/morse/overlay.rs` 负责多步骤框选会话；中途取消不应污染已保存配置
- `src-tauri/src/morse/settings.rs` 的持久化文件是 `morse_settings.json`
- 修改原生命令时，必要时同步更新 `src-tauri/capabilities/default.json`
- `src-tauri/src/delta/commands.rs` 负责 Delta DTO、Tauri commands、账号解析与持久化编排
- `src-tauri/src/delta/services/` 下按领域拆分 QQ / WeChat / QQSafe / Wegame / Game 逻辑，不要额外引入与仓库现状不一致的 `models/handlers` 架构
- `src-tauri/src/delta/storage/repo.rs` 使用单表 `delta_accounts` 承载不同账号类型；新增账号类型应优先扩展 `AccountKind`
- `src-tauri/src/delta/client/ide.rs` 负责 IDE 网关表单请求，`src-tauri/src/delta/utils/game.rs` 负责枪械/弹药/配件映射与 bind-role 解析

## Repo-specific cautions

- 使用 **Bun**，不要切换到 npm / pnpm / yarn
- 不要虚构仓库中不存在的 lint/test/CI 命令
- `README.md`、`AGENTS.md`、`CLAUDE.md` 和 `docs/CODEMAPS/` 需要随重大功能变更一起更新
- 仓库当前允许提交项目级 skills 目录：`.agents/skills/` 与 `.claude/skills/`；不要把它们误当成本地垃圾直接删除
- 忽略本地或生成产物：`node_modules`、`dist`、`src-tauri/target`、`.claude/worktrees/`、`.claude/settings.local.json`、`temp/`、`test-results/`

## If the project changes again

如果后续新增：
- 新的 Tauri commands
- 新的持久化结构
- 新的开发脚本
- 路由系统或新的应用壳层
- 新的项目级 skills / agents 目录约定

请在同一轮改动里同步更新 `README.md`、`AGENTS.md`、`CLAUDE.md` 与相关 codemap。

## graphify

This project has a graphify knowledge graph at graphify-out/.

Rules:
- Before answering architecture or codebase questions, read graphify-out/GRAPH_REPORT.md for god nodes and community structure
- If graphify-out/wiki/index.md exists, navigate it instead of reading raw files
- For cross-module "how does X relate to Y" questions, prefer `graphify query "<question>"`, `graphify path "<A>" "<B>"`, or `graphify explain "<concept>"` over grep — these traverse the graph's EXTRACTED + INFERRED edges instead of scanning files
- After modifying code files in this session, run `graphify update .` to keep the graph current (AST-only, no API cost)
