# Repository Guidelines

## Project Overview

**Delta Auto Tools** — Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust 桌面工具，面向《三角洲行动》玩家。原生能力模块：Morse 摩斯识别、计时器、计数器、连发器、音频触发器、Delta 账号与游戏数据接口、攻略网站工作台。

开发环境：Windows，仓库路径 `D:/code/ai/sjz/delta-auto-tools`，所有命令在 Windows + Bun 下测试通过。

## Wiki Documentation (droid-wiki/)

`droid-wiki/` 是项目自维护的结构化文档（36 个页面），覆盖架构、各功能模块、底层系统、开发流程、约定和发布。**当不确定某模块如何工作、某约定是什么、某流程怎么走时，优先查阅 `droid-wiki/` 下的对应文档，而不是凭记忆猜测。**

文档结构：

| 目录 | 内容 |
|------|------|
| `overview/` | 项目概览、系统架构、快速开始、术语表 |
| `features/` | 各功能模块详解（morse / timer / counter / rapidfire / audio / strategy / about） |
| `systems/` | 底层系统（tool-base / sync-tool / hotkeys / key-suppressor / overlay-windows / global-state / logging / theme-engine / profile-system） |
| `how-to-contribute/` | 开发流程、测试、调试、模式与约定、工具链 |
| `reference/` | 配置项与依赖参考 |
| `deployment.md` | 部署与发布流程 |

入口：`droid-wiki/overview/index.md`。`droid-wiki/.wiki-meta.json` 记录生成时间、commit、页面清单。

> Wiki 与 codegraph 互补：wiki 适合理解「为什么这样设计」和「整体流程」，codegraph 适合查「符号定义在哪、谁调用了谁」。

## Code Navigation: Use Codegraph MCP

本项目已建立 codegraph 索引（229 文件 / 4392 节点 / 9598 边）。**探索代码时优先使用 codegraph MCP，不要手动 grep 逐文件翻阅**：

| 场景 | 工具 | 示例 |
|------|------|------|
| 理解某模块如何工作 | `codegraph_explore` | `codegraph_explore("MorseState run_recognition_flow")` |
| 查找符号定义位置 | `codegraph_search` | `codegraph_search("MorseSettings")` |
| 查看某函数调用者 | `codegraph_callers` | `codegraph_callers("timer_save_settings")` |
| 查看某函数调用了什么 | `codegraph_callees` | `codegraph_callees("run_recognition_flow")` |
| 评估改动影响范围 | `codegraph_impact` | `codegraph_impact("ToolLogic")` |
| 获取单个符号完整源码 | `codegraph_node` | `codegraph_node("GameService", includeCode=true)` |
| 浏览文件树 | `codegraph_files` | `codegraph_files(format="tree")` |

典型探索流程：`codegraph_explore("keyword")` 获取相关源码 → `codegraph_callers`/`callees` 追踪调用链 → `codegraph_impact` 评估改动影响。

**以下信息不再在本文档中重复维护**，请通过 codegraph 查询：
- 前端与 Rust 后端完整文件树 → `codegraph_files`
- 各工具的 Tauri command 清单与签名 → `codegraph_search("morse_")` / `codegraph_search("delta_")` 等
- Tauri 事件名常量 → `codegraph_explore("events.rs")`
- 各模块数据结构定义 → `codegraph_search("Settings")` / `codegraph_search("Bootstrap")`
- 测试文件分布 → `codegraph_files(pattern="*.test.*")`

## AI Output 规范

- **所有 AI 输出必须使用中文**，包括代码注释、解释说明、错误提示和用户交互内容
- 技术术语（React、TypeScript、Tauri 等）保持英文原名
- 代码中的字符串、错误信息、UI 文案使用中文
- 文档、注释、commit message 使用中文

## Source of Truth

优先相信可执行配置与当前代码，而不是旧文档：

1. `src-tauri/tauri.conf.json`
2. `package.json`
3. `src/` 和 `src-tauri/src/`
4. `components.json`（shadcn/ui 配置）

文档与代码不一致时以当前实现为准。

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

PM2 开发编排（`ecosystem.config.cjs`）：将 Vite 和 Tauri 拆为两个独立 PM2 进程。

## Key Conventions

### 包管理

- 使用 **Bun**，不要切换到 npm / pnpm / yarn
- 不存在 `tailwind.config.js` — Tailwind v4 通过 CSS `@import "tailwindcss"` 配置，主题 token 在 `src/App.css` 的 `@theme inline`
- 路径别名：`@/components`、`@/components/ui`、`@/lib`、`@/hooks`

### Rust serde

所有对外序列化的 Rust 结构体**必须**使用 `#[serde(rename_all = "camelCase")]`。前端 TypeScript 类型必须匹配 camelCase 字段名。

### Delta 凭据边界

前端只收到 `DeltaAccountView`（id / kind / uinOrOpenid / hasAccessToken / expiresAt / capabilities）。**不得向前端返回** cookie、access_token、openid、ticket 或 code。游戏数据命令从 `accountId` 解析后端凭据。

- Delta 命令返回 `Result<ApiResponse<T>, DeltaError>`（`code=0` 为成功）
- Morse 等其他命令返回 `Result<T, String>`（中文错误字符串）
- `AccountKind` 序列化为 camelCase：`QqSafe`→`"qqSafe"`、`WegameQq`→`"wegameQq"`、`WegameWechat`→`"wegameWechat"`、`Pioneer`→`"pioneer"`

### 热键冲突规则

`ConflictPolicy` 枚举：`Strict`（禁止跨 scope 复用）和 `AllowHold`（允许 hold scope 与普通 scope 共存）。

- Timer / Counter 普通 scope 与 Rapidfire hold scope 允许同键共存（双方均用 `AllowHold`）
- Morse 与任何其他 scope 冲突必须拒绝（Morse 用 `Strict`）
- 录制热键时暂停对应 scope

### Overlay 透明窗口约束

计时器/计数器/连发器透明窗口必须无边框、透明、置顶、点击穿透。位置设置窗口可保留校准靶风格。overlay 必须保持透明背景。`?mode=` 查询参数分支进入 overlay/display/position 模式，不可用路由替代。

### Tauri command 注册

新增 Tauri command 必须同时注册到：
1. `src-tauri/src/lib.rs` 的 `generate_handler![]`
2. `src-tauri/capabilities/default.json`

### 版本号同步

版本号必须同步更新 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。如 `Cargo.lock` 中本包版本随解析更新，也应一并提交。

### UI 约束

- 视觉方向见 `DESIGN.md`（Swiss Industrial Print × Declassified Tactical Control Board）
- 仅使用 shadcn/ui + Tailwind CSS + `src/App.css` 主题 token；禁止新增 `.desktop-*`、`.tactical-*` 等自定义 CSS 类
- 图标使用 `@remixicon/react`，Button 内图标必须设置 `data-icon="inline-start"` / `"inline-end"`
- 攻略网站页使用主窗口内嵌 `strategy-content` 子 WebView，不创建独立浏览器窗口，不使用 iframe/srcDoc，不得隐藏 Left Index Rail
- `TooltipProvider` 已在 `src/main.tsx` 根部提供
- 设计改动必须保持功能不变：不改 Tauri command 名、查询参数 mode、状态机、保存逻辑或原生窗口 label

## Architecture Quick Reference

详细实现请通过 codegraph 探索，以下为核心架构锚点：

**前端**：`index.html` → `src/main.tsx` → `src/App.tsx`。App.tsx 通过 `useState<ToolId>` 切换工具页；overlay/display/position 模式通过 `?mode=` 查询参数分支。每个工具页遵循 Bootstrap/Form 双状态 + autosave debounce 400ms 模式。

**后端**：`src-tauri/src/lib.rs` 的 `run()` 在 `setup` 中依次初始化各工具模块并 `app.manage()` 注册状态。工具模块共享 `ToolBase` 泛型基座（`ToolLogic` trait、`ToolState<T>`）。

**工具模块**（详见 codegraph）：
- `morse/` — 截屏→二值化→轮廓检测→摩斯解码→自动输入
- `timer/` — 多计时器，250ms tick，透明窗口
- `counter/` — 多计数器，运行态独立持久化（counter_state.json）
- `rapidfire/` — 按住触发键连发，每 session 独立 OS worker 线程
- `audio/` — 快捷键/区域监听/识色三种触发模式
- `delta/` — 6 种账号鉴权 + SQLite 存储 + DPAPI 加密 + IDE 网关游戏数据
- `strategy/` — 攻略网站 WebView2 嵌入
- `theme/` — 5 套内置主题 + 自定义 + token override
- `profile/` — 多配置快照切换
- `logging/` — 混合格式日志 + 按天轮转 + 链路追踪

**事件模式**：事件名格式 `{tool}://{event}`，后端在 `*/events.rs` 定义常量，前端通过 `src/lib/tauri-events.ts` 的 `listenEvent<T>` helper 订阅。

## Testing

- **前端**：Vitest，测试文件 `*.test.ts` 紧邻源文件。运行 `bun run test`
- **Rust**：`cargo test`，测试内联在模块中。GameService 测试使用 `mockito` mock HTTP。运行 `cargo test --manifest-path src-tauri/Cargo.toml`
- Vitest coverage 配置只包含 `morse-utils.ts`

## Commit Guidelines

- Commit message 使用中文
- 发布 commit：subject `发布 v<version>`，正文必须包含 `变更：` 段，变更项从实际 diff 提炼，禁止泛泛"更新版本"
- 常规 commit 示例：`feat(audio): 识色探针支持多目标颜色`、`fix(counter): 全局开关关闭时保留计数器运行值`

## Release Workflow

### 正式版

1. 同步版本号（`package.json` / `Cargo.toml` / `tauri.conf.json`）
2. 签名构建：`scripts/build-release.ps1`（需设置 `TAURI_SIGNING_PRIVATE_KEY`）
3. 生成 `latest.json`：`scripts/generate-latest-json.ps1`
4. 检查产物：`.exe` + `.exe.sig` + `latest.json`（三者缺一不可）
5. Commit + Tag：`git tag -a v<version> -m "发布 v<version>"` → `git push origin master v<version>`
6. 创建 GitHub Release 上传 3 个资产（`.exe` / `.exe.sig` / `latest.json`）
7. 验证：`gh release view v<version> --json tagName,isDraft,isPrerelease,assets`

### Beta 版

1. 版本号格式：`<major>.<minor>.<patch>-beta.<N>`
2. 无签名构建：`bun run tauri build`（不需要 `TAURI_SIGNING_PRIVATE_KEY`）
3. 产物仅 `.exe`，无 `.sig` 和 `latest.json`
4. 创建 Release 加 `--prerelease` 标记，只上传 1 个资产（`.exe`）
5. Beta 应用内「检查更新」走 stable 端点；同数值正式版 > beta，更高数值正式版触发更新

### 网络与代理

`git push` 或 `gh release` 访问 GitHub 遇连接重置时，设置代理（注意 `&&` 前不要有空格）：
```bash
set HTTP_PROXY=http://127.0.0.1:7897&& set HTTPS_PROXY=http://127.0.0.1:7897&& git push origin master v<version>
```

### Windows cmd 多行字符串

`gh issue comment` / `gh release create` 的 `--body` / `--notes` 参数在 cmd.exe 中传多行内容会被截断。**必须使用 `--body-file` / `--notes-file` 从文件读取**。

## Repo-Specific Cautions

- `README.md`、`AGENTS.md` 和 `CLAUDE.md` 需随重大功能变更一起更新
- `.agents/skills/` 和 `.omp/extensions/` 是项目级扩展目录，不要误删
- 忽略：`node_modules`、`dist`、`src-tauri/target`、`.claude/worktrees/`、`temp/`、`test-results/`
- `src-tauri/src/delta/resources/ammo.json` 和 `accessory.json` 为空数组，未使用；实际配置在 `game_config.rs` 内联常量
- localStorage 偏好 key 统一前缀 `delta-auto-tools:`
- GitHub 远端：`https://github.com/IZRINO/delta-auto-tools`
- Issue 处理：先回复处理结论，不要在回复后直接关闭 Issue，等待确认后再关

## If the Project Changes

新增以下内容时，请在同一轮改动里同步更新 `README.md` 与 `AGENTS.md`：
- 新的 Tauri commands
- 新的持久化结构
- 新的开发脚本
- 路由系统或新的应用壳层
- 新的项目级 skills / agents / OMP 扩展目录约定

### Wiki 文档同步

修改代码时，如果改动涉及 `droid-wiki/` 已记录的内容，必须在同一轮改动里更新对应的 wiki 页面，避免文档与代码漂移：

- 改动工具模块行为 → 更新 `droid-wiki/features/<tool>.md` 或 `droid-wiki/systems/<system>.md`
- 新增/移除 Tauri command 或事件 → 更新对应 feature/system 页面
- 改动架构、基座、约定 → 更新 `droid-wiki/overview/architecture.md` 或 `droid-wiki/how-to-contribute/patterns-and-conventions.md`
- 改动配置项或依赖 → 更新 `droid-wiki/reference/configuration.md` 或 `droid-wiki/reference/dependencies.md`
- 改动发布流程 → 更新 `droid-wiki/deployment.md`

纯文案或纯重构（不改变行为和接口）无需更新 wiki。
