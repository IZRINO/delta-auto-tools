# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Delta Auto Tools — Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust 桌面工具，面向《三角洲行动》玩家。四个原生能力模块：Morse 摩斯识别、计时/计数器、连发器、Delta 账号与游戏数据接口。外加攻略网站工作台。

## Commands

```bash
bun install                    # 安装前端依赖
bun run dev                    # Vite 前端开发服务器（端口 1420，strictPort）
bun run tauri dev              # 完整桌面开发（Vite + Tauri）
bun run build                  # tsc && vite build
bun run test                   # Vitest 前端单元测试
bun run test:coverage          # 前端覆盖率（仅 morse-utils.ts）
cargo check --manifest-path src-tauri/Cargo.toml   # Rust 编译检查
cargo test --manifest-path src-tauri/Cargo.toml    # Rust 单元测试
```

运行单个前端测试：`bunx vitest run src/components/app/morse-utils.test.ts`
运行单个 Rust 测试：`cargo test --manifest-path src-tauri/Cargo.toml <test_name>`

## Source of Truth 优先级

1. `src-tauri/tauri.conf.json`
2. `package.json`
3. `src/` 和 `src-tauri/src/`
4. `components.json`（shadcn/ui 配置）

文档与代码不一致时以当前实现为准。

## AI 输出规范

所有 AI 输出必须使用中文，技术术语保持英文原名。代码中的字符串、错误信息、UI 文案使用中文。Commit message 使用中文。

## Architecture

### 前端入口链路

`index.html` → `src/main.tsx` → `src/App.tsx`

App.tsx 无路由库，通过 `useState<ToolId>` 切换工具页。Overlay/display/position 模式通过 `?mode=` 查询参数分支进入（`overlay`、`timer-display`、`timer-position`、`counter-display`、`counter-position`、`rapidfire-display`、`rapidfire-position`），不可用路由替代。

### 页面组件模式

每个工具页遵循同一模式：
- **Bootstrap/FormData 双状态**：`bootstrap`（Rust 返回的不可变规范态）+ `form`（本地可编辑草稿），脏检测通过 `JSON.stringify` 比较
- **Autosave**：表单变更后 debounce 400ms（`AUTOSAVE_DELAY_MS`）调用 `xxx_save_settings`，`autosaveVersionRef` 防止陈旧保存覆盖
- **Form↔Settings 转层**：`settingsToForm()`（int→string 供 Input）和 `parseSettingsForm()`（验证+string→int 供 Rust）
- 容器页（`morse-page.tsx` 等）负责状态编排，子组件只接收 props

### Tauri IPC 模式

- **Command 调用**：`invoke<XxxBootstrap>("tool_action", { params })`
- **事件监听**：`listen<XxxPayload>("tool://event-name", callback)`，事件名格式 `{tool}://{event}`
- **原生 shell 检测**：`useNativeShell()` 检查 `__TAURI_INTERNALS__`，浏览器预览模式下禁用所有原生命令

### Overlay 窗口系统

两类机制：
1. **同窗口 overlay**（Morse 区域框选）：`?mode=overlay` → `RegionSelectionOverlay`，全屏透明拖拽框选，坐标通过 `morse_overlay_submit_selection` 提交
2. **独立窗口 overlay**（Timer/Rapidfire 透明显示/位置设置）：各自有 display 和 position 两种模式，position 模式拖拽定位坐标提交

### Rust 后端模块

| 模块 | 路径 | 职责 |
|------|------|------|
| morse | `src-tauri/src/morse/` | 屏幕截取→二值化→轮廓检测→摩斯解码→自动输入；overlay 多步骤框选会话 |
| timer | `src-tauri/src/timer/` | 多计时器/计数器，250ms tick 循环，透明窗口，计数器运行态独立持久化 |
| rapidfire | `src-tauri/src/rapidfire/` | 按住触发键连发，每 session 独立 OS worker 线程，卡片级不追加/抖动/间距 |
| delta | `src-tauri/src/delta/` | 6 种账号鉴权流程（QQ/微信/QQ安全中心/Wegame/先遣服），SQLite 账号存储，DPAPI 加密，IDE 网关游戏数据查询 |
| hotkeys | `src-tauri/src/hotkeys.rs` | 全局共享 willhook 键盘钩子，scope 注册，普通/hold 两种绑定，跨 scope 冲突检测 |
| strategy | `src-tauri/src/strategy/` | 兼容入口：`strategy_open_window` 创建子 WebView，`strategy_fetch_page` Chrome 头抓取+JS 重定向跟随 |

新增 Tauri command 必须同时注册到 `src-tauri/src/lib.rs` 的 `generate_handler![]` 和 `src-tauri/capabilities/default.json`。

## UI & Styling

### 视觉方向

**Swiss Industrial Print × Declassified Tactical Control Board**（详见 `DESIGN.md`）。主基底 Paper `#F1EFE8` / Bone `#DDD8CC`，Ink `#080808` 粗黑结构线，Alert Red `#E11919` 仅占 3–8% 画面。90 度直角，禁止圆角卡片、柔和阴影、玻璃态、渐变。

### 组件层

- **共享工业组件**（`src/components/app/app-ui.tsx`）：`AppPage`（12 列 Work Grid）、`PageHero`、`SignalTile`、`TacticalCard`、`SectionHeader`、`ControlTile`、`InlineControl`、`CardBody` 等。三个以上页面需要同一种结构时应先扩展共享组件。
- **shadcn/ui 基础组件**（`src/components/ui/`）：radix-vega 风格，remixicon 图标库。Button 内图标必须 `data-icon="inline-start"` / `"inline-end"`。
- **禁止新增** `.desktop-*`、`.tactical-*` 等自定义 CSS 类。仅 shadcn/ui + Tailwind 工具类 + `src/App.css` 主题 token。

### Tailwind v4

CSS-first 方案，**不存在** `tailwind.config.js`。主题 token 在 `src/App.css` 的 `@theme inline` 中。全局 `--radius: 0`。

### CSS 变量（设计 token）

Paper `#F1EFE8`、Bone `#DDD8CC`、Ink `#080808`、Steel `#3B3B36`、Ash `#8A867B`、Line `#B9B2A4`、Alert Red `#E11919`、Warning Amber `#A36A00`、Valid Green `#3F6B2A`、Data Well `#141414`。

## Key Conventions

### Rust serde

所有对外序列化的 Rust 结构体**必须**使用 `#[serde(rename_all = "camelCase")]`。Delta 端 `AccountKind` 序列化为 camelCase（`QqSafe`→`"qqSafe"`、`WegameQq`→`"wegameQq"`），前端必须匹配。

### Delta 凭据边界

前端只收到 `DeltaAccountView`（id/kind/uinOrOpenid/hasAccessToken/expiresAt）。不得向前端返回 cookie、access_token、openid、ticket 或 code。游戏数据命令从 `accountId` 解析后端凭据。

### Morse/Delta 返回差异

- Morse 命令返回 `Result<T, String>`（中文错误字符串）
- Delta 命令返回 `Result<ApiResponse<T>, DeltaError>`（`code=0` 为成功）

### 热键冲突规则

Timer 普通 scope 与 Rapidfire hold scope 允许同键共存。其他跨 scope 冲突必须拒绝。录制热键时暂停对应 scope。

### Overlay 透明窗口约束

计时器/计数器/连发器透明窗口必须无边框、透明、置顶、点击穿透。位置设置窗口可保留校准靶风格。overlay 必须保持透明背景。

### 攻略网站约束

主窗口内嵌 `strategy-content` 子 WebView，不创建独立浏览器窗口，不使用 iframe/srcDoc。新增站点和刷新档位使用内联面板，不使用 Dialog/SelectContent 等浮层。不得隐藏 Left Index Rail。

## Version Release

版本号必须同步更新 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`（及 `Cargo.lock`）。发布 commit 须包含变更摘要和验证结果。必须创建 `v<version>` Tag 并推送。必须创建 GitHub Release 上传 MSI + NSIS 安装包。

## Repo-Specific Notes

- 使用 **Bun**，不要切换到 npm/pnpm/yarn
- 不存在 `tailwind.config.js`
- `src-tauri/src/delta/resources/ammo.json` 和 `accessory.json` 为空数组，未使用；实际配置在 `game_config.rs` 内联常量
- 前端测试覆盖仅 `morse-utils.ts`
- `.agents/skills/` 和 `.omp/extensions/` 是项目级扩展目录，不要误删
- `README.md`、`AGENTS.md` 和 `CLAUDE.md` 需随重大功能变更一起更新
