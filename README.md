# Delta Auto Tools

> 特勤处自动化开发中：已提供账号、制作台、有序子弹目标、每日兑换调度、全屏区域校准、用户参考图及模板/OCR 判定契约；真实 WeGame/图片匹配执行/OCR/键鼠执行尚未接入。

**Delta Auto Tools** — Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust 桌面工具，面向《三角洲行动》玩家。

6 个工具模块 + 攻略网站工作台，前端负责交互与 daisyUI 视觉层，Rust/Tauri 负责快捷键、透明窗口、屏幕识别、本地存储和 Delta 接口调用。

## 功能模块

| 模块 | 能力 |
|------|------|
| **Morse** | 区域框选 → 二值化 → 轮廓检测 → 摩斯解码 → 自动输入；快捷键触发、历史记录 |
| **Timer** | 多计时器，250ms tick 循环，独立总开关，置顶透明显示窗口，位置校准 |
| **Counter** | 多计数器，运行态独立持久化与 latest-wins 合并写入，独立总开关，置顶透明显示窗口，位置校准 |
| **Rapidfire** | 按住触发键连发，每 session 独立 OS worker 线程，运行态事件限制 60Hz，卡片级不追加/抖动/间距策略 |
| **Recognition** | 快捷键、多参考图区域匹配、多区域识色三种识别来源；快捷键支持按下单次或按住持续触发，可执行音频、按键、点击三类效果 |
| **Strategy** | 主窗口内嵌 `strategy-content` 子 WebView2，站点切换、自定义站点、刷新档位 |

其他能力：Delta 账号管理与游戏数据查询（本地凭据 DPAPI 加密）、关于面板与 Tauri 官方更新器。

## 架构

### 核心系统

| 系统 | 职责 |
|------|------|
| `tool_base` | 共享泛型基座：`ToolLogic` trait、`ToolState<T>`、`ToolStateInner<T>`、`get_bootstrap<T>` |
| `global_state` | 全局总开关（GlobalState）+ `ToolLifecycleRegistry`（统一 stop 入口，所有工具模块注册停止逻辑） |
| `hotkeys` | 共享 willhook 键盘钩子，scope 注册，`ConflictPolicy`（Strict / AllowHold），跨 scope 冲突检测 |
| `overlay-windows` | 透明窗口：无边框、置顶、点击穿透；位置设置窗口保留校准靶风格 |
| `theme-engine` | 3 套 daisyUI 内置主题 + 自定义 + token override |
| `profile-system` | 多配置快照切换、revision 防陈旧写入、复制、删除、单配置导入/导出 |
| `logging` | 混合格式日志 + 按天轮转 + 链路追踪 |
| `security` | main / overlay / Strategy remote WebView 独立 capability；生产 CSP 仅允许本地资源、Tauri IPC 与 data/blob 图片 |

### 架构改进

- **ToolLifecycleRegistry**：统一 stop 入口，所有工具模块注册停止逻辑，替代各模块独立 shutdown
- **RunsSync trait**：`runs` narrowing 逻辑下放到 Logic 层，Rust 侧通过 trait 约束统一调用
- **Rapidfire 模块拆分**：`keys` / `worker` / `overlay` / `commands` 四个子模块，职责清晰
- **Recognition watcher 拆分**：`manager` / `matching` / `capture` 三层，匹配逻辑与捕获逻辑分离
- **Recognition 调度隔离**：截图与 NCC 在 `spawn_blocking` 中执行，全局最多 2 个并发任务；restart/stop 通过 generation + abort 阻止旧 watcher 继续触发效果
- **Recognition 持续触发**：每张卡片维护独立 hold session，Down 立即执行，之后按冷却串行执行，Up、保存、禁用、Profile 切换或全局关闭时取消
- **事件对齐**：前端 `subscribeTauriEvent<PayloadType>(EVENTS.xxx, handler)` 模式；`state-changed` 只传 settings/结构，`runs-changed` 传轻量运行态
- **运行态热路径**：Rapidfire count emit 最多 60Hz；Counter 单 writer 线程合并磁盘写入；位置拖动按 rAF 合并且最多一个 invoke in-flight
- **权限分区**：`default.json` 只覆盖 main，`overlays.json` 只给本地叠加窗事件/窗口权限，`strategy.json` 对 remote `strategy-content` 授权为空

### 事件模式

事件名格式 `{tool}://{event}`，后端在 `*/events.rs` 定义常量，前端通过 `EVENTS` 常量和 `src/lib/tauri-listener.ts` 的显式泛型 helper 订阅。Timer/Counter/Rapidfire 将 settings/结构事件与运行态事件拆分，避免高频序列化完整 Bootstrap。

### Overlay 约束

计时器/计数器/连发器透明窗口必须无边框、透明、置顶、点击穿透。overlay 保持透明背景。`?mode=` 查询参数分支进入 overlay/display/position 模式，不可用路由替代。

## 技术栈

- **桌面框架**：Tauri 2
- **原生能力**：Rust
- **前端**：React 19、TypeScript、Vite
- **包管理**：Bun
- **UI**：daisyUI 5、Tailwind CSS v4（CSS-first）、Radix/Base UI headless 交互、本地 `src/components/ui/` 包装组件、Remix Icon
- **本地存储**：SQLite、JSON 配置文件、Windows DPAPI 凭据加密

## UI 与样式

项目已移除 shadcn 组件生成器与默认视觉体系。当前 UI 约定：

- **视觉层**：优先使用 daisyUI 组件 class、Tailwind CSS 工具类和 `src/App.css` 中的 daisyUI token
- **行为层**：保留 Radix/Base UI headless 能力，用于 Dialog、Dropdown、Tooltip、Select 等焦点管理、键盘导航、Portal 和无障碍交互
- **基础组件**：`src/components/ui/` 是项目本地包装层，不再作为 shadcn CLI 生成目录维护
- **图标**：统一使用 `@remixicon/react`，按钮内图标保留 `data-icon="inline-start"` / `"inline-end"` 语义标记
- **主题**：通过 daisyUI token（如 `--color-base-*`、`--color-primary`、`--radius-*`）和运行时 override 管理，不再新增旧 shadcn/战术风格桥接 token

## 测试覆盖

| 层 | 门禁 |
|----|------|
| 前端单元测试 | `bun run test` |
| 前端全量覆盖率 | lines 25.49% / statements 25.67% / functions 22.31% / branches 25.76% |
| Rust | `cargo fmt` + `cargo clippy --all-targets -D warnings` + `cargo test` |
| Windows 统一门禁 | `bun run check` |

## 本地开发

```bash
bun install                       # 安装前端依赖
bun run dev                       # Vite 前端开发服务器（端口 1420）
bun run tauri dev                 # 完整桌面开发（Vite + Tauri）
bun run build                     # tsc && vite build
bun run test                      # Vitest 前端单元测试
bun run test:coverage             # 全量前端覆盖率与阈值检查
bun run check                     # TypeScript、测试、coverage、fmt、Clippy、Rust 测试
cargo check --manifest-path src-tauri/Cargo.toml   # Rust 编译检查
cargo test --manifest-path src-tauri/Cargo.toml    # Rust 单元测试
```

## 项目结构

```text
src/                        # React 前端应用
src/components/app/         # 业务页面（morse-page、timer-page、counter-page、rapidfire-page、recognition-page、strategy-page 等）
src/components/ui/          # 本地 UI 包装层：Radix headless 行为 + daisyUI class，非 shadcn 生成目录
src/lib/                    # 共享工具函数与 tauri-events 常量
src-tauri/src/              # Rust/Tauri 原生能力
src-tauri/src/tool_base.rs  # 共享泛型基座（ToolLogic / ToolState<T>）
src-tauri/src/global_state.rs  # 全局总开关 + ToolLifecycleRegistry
src-tauri/src/hotkeys.rs    # 共享键盘钩子与冲突策略
src-tauri/src/morse/        # 摩斯识别
src-tauri/src/timer/        # 计时器
src-tauri/src/counter/      # 计数器
src-tauri/src/rapidfire/    # 连发器（keys / worker / overlay / commands）
src-tauri/src/recognition/  # 识别触发（effects / manager / matching / capture / player）
src/components/app/recognition-{page,card-editor,overlay}.tsx  # 识别编排、卡片编辑与框选模块
src/components/app/recognition-card-reducer.ts                # 识别卡片不可变更新 seam
src/components/app/strategy-page.tsx          # 攻略网站 WebView2 子视图
src/components/app/strategy-utils.ts          # 攻略站点、刷新档位与 bounds 工具
src-tauri/src/about/        # 关于面板 + Tauri 官方更新器
src-tauri/src/delta/        # Delta 登录、账号与游戏数据
scripts/                    # 发布流水线脚本
droid-wiki/                 # 项目自维护结构化文档（36 页）
```

## 辅助脚本

| 脚本 | 用途 |
|------|------|
| `scripts/setup-update-key.ps1` | 生成 Tauri 更新签名密钥对 |
| `scripts/build-release.ps1` | 带签名的 NSIS 安装包构建 |
| `scripts/generate-latest-json.ps1` | 从 `.sig` 生成 `latest.json`（更新器端点） |
| `scripts/wait-for-port.cjs` | PM2 启动前端口等待 |

完整发布流程见 `AGENTS.md` / `CLAUDE.md`。
