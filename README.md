# Delta Auto Tools

**Delta Auto Tools** — Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust 桌面工具，面向《三角洲行动》玩家。

6 个工具模块 + 攻略网站工作台，前端负责交互与攻略站 WebView2 子视图，Rust/Tauri 负责快捷键、透明窗口、屏幕识别和本地存储。

## 功能模块

| 模块 | 能力 |
|------|------|
| **Morse** | 区域框选 → 二值化 → 轮廓检测 → 摩斯解码 → 自动输入；快捷键触发、历史记录 |
| **Timer** | 多计时器，250ms tick 循环，独立总开关，置顶透明显示窗口，位置校准 |
| **Counter** | 多计数器，运行态独立持久化，独立总开关，置顶透明显示窗口，位置校准 |
| **Rapidfire** | 按住触发键连发，每 session 独立 OS worker 线程，卡片级不追加/抖动/间距策略 |
| **Recognition** | 快捷键、区域图像匹配、多区域识色三种识别来源；可执行音频、按键、点击三类效果 |
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
| `profile-system` | 多配置快照切换、复制、删除、单配置导入/导出 |
| `logging` | 混合格式日志 + 按天轮转 + 链路追踪 |

### 架构改进

- **ToolLifecycleRegistry**：统一 stop 入口，所有工具模块注册停止逻辑，替代各模块独立 shutdown
- **RunsSync trait**：`runs` narrowing 逻辑下放到 Logic 层，Rust 侧通过 trait 约束统一调用
- **Rapidfire 模块拆分**：`keys` / `worker` / `overlay` / `commands` 四个子模块，职责清晰
- **Recognition watcher 拆分**：`manager` / `matching` / `capture` 三层，匹配逻辑与捕获逻辑分离
- **事件对齐**：前端 `listen<PayloadType>(EVENTS.xxx, handler)` 模式，事件名通过 `src/lib/tauri-events.ts` 常量订阅，杜绝硬编码

### 事件模式

事件名格式 `{tool}://{event}`，后端在 `*/events.rs` 定义常量，前端通过 `EVENTS` 常量与显式泛型订阅。

### Overlay 约束

计时器/计数器/连发器透明窗口必须无边框、透明、置顶、点击穿透。overlay 保持透明背景。`?mode=` 查询参数分支进入 overlay/display/position 模式，不可用路由替代。

## 技术栈

- **桌面框架**：Tauri 2
- **原生能力**：Rust
- **前端**：React 19、TypeScript、Vite
- **包管理**：Bun
- **UI**：daisyUI、Radix headless 组件、Tailwind CSS v4、Remix Icon
- **本地存储**：SQLite、JSON 配置文件、Windows DPAPI 凭据加密

## 测试覆盖

| 层 | 数量 |
|----|------|
| Rust 单元测试 | 386 |
| 前端单元测试 | 346 |
| 编译检查 | `cargo check` clean |
| 前端构建 | `bun run build` clean |

## 本地开发

```bash
bun install                       # 安装前端依赖
bun run dev                       # Vite 前端开发服务器（端口 1420）
bun run tauri dev                 # 完整桌面开发（Vite + Tauri）
bun run build                     # tsc && vite build
bun run test                      # Vitest 前端单元测试
cargo check --manifest-path src-tauri/Cargo.toml   # Rust 编译检查
cargo test --manifest-path src-tauri/Cargo.toml    # Rust 单元测试
```

## 项目结构

```text
src/                        # React 前端应用
src/components/app/         # 业务页面（morse-page、timer-page、counter-page、rapidfire-page、recognition-page、strategy-page 等）
src/components/ui/          # Radix headless 行为 + daisyUI 风格基础组件
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
