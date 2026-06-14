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
- **事件监听**：`listen<XxxPayload>("tool://event-name", callback)`，事件名格式 `{tool}://{event}`。后端在 `morse/events.rs`、`timer/events.rs`、`rapidfire/events.rs` 定义常量，前端通过 `src/lib/tauri-events.ts` 的 `MORSE_EVENTS` / `TIMER_EVENTS` / `RAPIDFIRE_EVENTS` / `GLOBAL_EVENTS` 和 `listenEvent<T>` helper 订阅，避免硬编码事件名。
- **原生 shell 检测**：`useNativeShell()` 检查 `__TAURI_INTERNALS__`，浏览器预览模式下禁用所有原生命令

### Overlay 窗口系统

两类机制：
1. **同窗口 overlay**（Morse 区域框选）：`?mode=overlay` → `RegionSelectionOverlay`，全屏透明拖拽框选，坐标通过 `morse_overlay_submit_selection` 提交
2. **独立窗口 overlay**（Timer/Rapidfire 透明显示/位置设置）：各自有 display 和 position 两种模式，position 模式拖拽定位坐标提交

### Rust 后端模块

| 模块 | 路径 | 职责 |
|------|------|------|
| tool_base | `src-tauri/src/tool_base.rs` | 工具模块共享泛型基座：ToolLogic trait、ToolState<T>、ToolStateInner<T>、get_bootstrap<T> |
| global_state | `src-tauri/src/global_state.rs` | 全局总开关（GlobalState）与 enabled-changed 事件 |
| morse | `src-tauri/src/morse/` | 屏幕截取→二值化→轮廓检测→摩斯解码→自动输入；overlay 多步骤框选会话；MorseState = ToolState<MorseLogic> |
| timer | `src-tauri/src/timer/` | 多计时器/计数器，250ms tick 循环，透明窗口，计数器运行态独立持久化；TimerState 包装 ToolState<TimerLogic> |
| rapidfire | `src-tauri/src/rapidfire/` | 按住触发键连发，每 session 独立 OS worker 线程，卡片级不追加/抖动/间距；RapidfireState = ToolState<RapidfireLogic> |
| delta | `src-tauri/src/delta/` | 6 种账号鉴权流程（QQ/微信/QQ安全中心/Wegame/先遣服），SQLite 账号存储，DPAPI 加密，IDE 网关游戏数据查询；GameService 缓存于 DeltaState |
| hotkeys | `src-tauri/src/hotkeys.rs` | 全局共享 willhook 键盘钩子，scope 注册，普通/hold 两种绑定，跨 scope 冲突检测（ConflictPolicy） |
| strategy | `src-tauri/src/strategy/` | 兼容入口：`strategy_open_window` 创建子 WebView，`strategy_fetch_page` Chrome 头抓取+JS 重定向跟随 |

新增 Tauri command 必须同时注册到 `src-tauri/src/lib.rs` 的 `generate_handler![]` 和 `src-tauri/capabilities/default.json`。

## UI & Styling

### 视觉方向

**Swiss Industrial Print × Declassified Tactical Control Board**（详见 `DESIGN.md`）。主基底 Carbon `#0C0C0B` / Slate `#171715`，Chalk `#D8D4CC` 粗粉笔结构线，Amber `#E8A000` 仅占 3–8% 画面。90 度直角，禁止圆角卡片、柔和阴影、玻璃态、渐变。暗色唯一模式，无 light/dark 切换。

### 组件层

- **共享工业组件**（`src/components/app/app-ui.tsx`）：`AppPage`（12 列 Work Grid）、`PageHero`、`SignalTile`、`TacticalCard`、`SectionHeader`、`ControlTile`、`InlineControl`、`CardBody` 等。三个以上页面需要同一种结构时应先扩展共享组件。
- **shadcn/ui 基础组件**（`src/components/ui/`）：radix-vega 风格，remixicon 图标库。Button 内图标必须 `data-icon="inline-start"` / `"inline-end"`。
- **禁止新增** `.desktop-*`、`.tactical-*` 等自定义 CSS 类。仅 shadcn/ui + Tailwind 工具类 + `src/App.css` 主题 token。

### Tailwind v4

CSS-first 方案，**不存在** `tailwind.config.js`。主题 token 在 `src/App.css` 的 `@theme inline` 中。全局 `--radius: 0`。

### CSS 变量（设计 token）

Carbon `#0C0C0B`、Slate `#171715`、Iron `#232320`、Chalk `#D8D4CC`、Zinc `#9A968E`、Dust `#6E6B65`、Seam `#2A2926`、Amber `#E8A000`、Rust `#C85400`、Moss `#3F8A30`、Void `#050504`。

## Key Conventions

### Rust serde

所有对外序列化的 Rust 结构体**必须**使用 `#[serde(rename_all = "camelCase")]`。Delta 端 `AccountKind` 序列化为 camelCase（`QqSafe`→`"qqSafe"`、`WegameQq`→`"wegameQq"`），前端必须匹配。

### Delta 凭据边界

前端只收到 `DeltaAccountView`（id/kind/uinOrOpenid/hasAccessToken/expiresAt/capabilities）。不得向前端返回 cookie、access_token、openid、ticket 或 code。游戏数据命令从 `accountId` 解析后端凭据。

### Morse/Delta 返回差异

- Morse 命令返回 `Result<T, String>`（中文错误字符串）
- Delta 命令返回 `Result<ApiResponse<T>, DeltaError>`（`code=0` 为成功）

### 热键冲突规则

- `ConflictPolicy` 枚举声明冲突策略：`Strict`（禁止跨 scope 复用）和 `AllowHold`（允许 hold scope 与普通 scope 共存）。
- `HotkeyRegistration` 和 `HoldRegistration` 均包含 `conflict_policy` 字段；`replace_scope` / `replace_hold_scope` 接收该参数。
- Timer 普通 scope 与 Rapidfire hold scope 允许同键共存（双方均使用 `ConflictPolicy::AllowHold`）；运行时会先分发连发器 hold Down/Up，再分发计时器普通快捷键。Morse 与 Timer 普通快捷键冲突、Morse 与 Rapidfire hold 冲突仍必须拒绝（Morse 使用 `ConflictPolicy::Strict`）。
- 其他跨 scope 冲突必须拒绝。录制热键时暂停对应 scope。

### Overlay 透明窗口约束

计时器/计数器/连发器透明窗口必须无边框、透明、置顶、点击穿透。位置设置窗口可保留校准靶风格。overlay 必须保持透明背景。

### 攻略网站约束

主窗口内嵌 `strategy-content` 子 WebView，不创建独立浏览器窗口，不使用 iframe/srcDoc。新增站点和刷新档位使用内联面板，不使用 Dialog/SelectContent 等浮层。不得隐藏 Left Index Rail。

## Version Release

### 版本号同步

版本号必须同步更新 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。如 `src-tauri/Cargo.lock` 中的本包版本随 Cargo 解析更新，也应一并提交。

### 构建

每次更新版本号后必须运行 `bun run tauri build` 完成桌面打包。打包成功后检查以下两个产物存在：
- `src-tauri/target/release/bundle/msi/delta-auto-tools_<version>_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe`

### 发布 Commit

发布 commit 不能只写 `发布 v<version>`。Subject 使用 `发布 v<version>`，正文必须包含变更摘要与验证结果，至少包含 `变更：` 和 `验证：` 两段。变更项从本次实际 diff / Release notes 提炼，禁止泛泛"更新版本"。推荐格式：

```bash
git commit -m "发布 v<version>" -m "变更：
- ...

验证：
- bun run test
- bun run tauri build"
```

### Tag

每次版本发布必须创建并推送对应 `v<version>` Tag：

```bash
git tag -a v<version> -m "发布 v<version>"
git push origin v<version>
```

### GitHub Release + 安装包上传

每次版本发布必须创建 GitHub Release 并上传 MSI 与 NSIS 安装包：

```bash
# 新建 Release 并上传
gh release create v<version> \
  src-tauri/target/release/bundle/msi/delta-auto-tools_<version>_x64_en-US.msi \
  src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe \
  --repo IZRINO/delta-auto-tools --target master \
  --title "delta-auto-tools <version>" --notes "<发布说明>"

# Release 已存在时覆盖上传安装包
gh release upload v<version> \
  src-tauri/target/release/bundle/msi/delta-auto-tools_<version>_x64_en-US.msi \
  src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe \
  --repo IZRINO/delta-auto-tools --clobber
```

### 验证

Release 发布后必须验证：

```bash
gh release view v<version> --repo IZRINO/delta-auto-tools \
  --json tagName,url,isDraft,isPrerelease,assets
```

确认非 draft、非 prerelease，且两个安装包状态均为 `uploaded`。

## Agent skills

### Issue tracker

Issues 使用 GitHub Issues，通过 `gh` CLI 读写。详见 `docs/agents/issue-tracker.md`。

### Triage labels

使用五级分流标签：needs-triage / needs-info / ready-for-agent / ready-for-human / wontfix。详见 `docs/agents/triage-labels.md`。

### Domain docs

Single-context 布局：根目录 `CONTEXT.md` + `docs/adr/`。详见 `docs/agents/domain.md`。

## Repo-Specific Notes

- 使用 **Bun**，不要切换到 npm/pnpm/yarn
- 不存在 `tailwind.config.js`
- `src-tauri/src/delta/resources/ammo.json` 和 `accessory.json` 为空数组，未使用；实际配置在 `game_config.rs` 内联常量
- 前端测试覆盖已扩展至 `morse-utils.ts` + `timer-utils.ts` + `favorites-utils.ts` + `delta-utils.ts` + `delta-types.ts` + `delta-login-utils.ts` + `delta-game-data-loader.ts` + `use-bootstrap-form-logic.ts` + `use-hotkey-recorder.ts` + `use-autosave.ts`（Vitest coverage 配置仍只包含 `morse-utils.ts`）
- `.agents/skills/` 和 `.omp/extensions/` 是项目级扩展目录，不要误删
- `README.md`、`AGENTS.md` 和 `CLAUDE.md` 需随重大功能变更一起更新
