# Delta Auto Tools

> 特勤处默认业务配置统一管理四制作台时长、制作物品备注及有序子弹目标；子弹目标保存备注、普通/赛季类型、指定点击点和 A/D 重置后向下滚动次数。账号默认继承，也可显式开启独立设置，分别覆盖四台制作物品选择点击点和子弹目标。`Ready` 账号可主动重新校正实际状态；账号页“人工校正制作与子弹状态”继续一次确认四制作台和当天全部启用子弹。24 小时任务行按 `AccountFailure.stationKind` 或 `AmmoTarget.lastFailure` 只处理实际失败制作台或子弹；账号处于“需人工验证”或制作台不确定时，任务行同样提供单项判定，选“正在制作”会预填异常前的剩余时间，留空即继承。登录、账号列表扫描及二次导航超时等账号级失败用“已人工检查”恢复，同时按存量计时还原不确定制作台，不改子弹状态。账号卡片与账号区标题另有“一键恢复状态”，可单账号或全部清除异常：账号回正常、制作台恢复异常前剩余时间、失败子弹回未兑换、限时商品失败回待检查；当天已兑换成功的子弹保持已兑换，不会重复兑换。当前计时不会因默认时长变化而重算。

> 特勤处自动化开发中：单账号登录、游戏内导航、单制作台“收取并重做”、当前账号四制作台批处理、单账号真实子弹兑换试运行及多账号自动轮次已形成闭环。军需处流程识别并点击部门后，按两段可配置等待依次点击军需处与“进入军需处”；随后分别通过 `ammo.tacticalDepartment` 或 `ammo.researchDepartment` 用户参考图进入子弹兑换或限时商品分支。同账号两类任务同时到期时共享一次入口。子弹先执行全部普通目标，存在赛季目标时只点击一次赛季入口，再执行全部赛季目标。每个 run 首个键鼠操作块显示 5→4→3→2→1，后续原本需要提示的操作块只显示 1；原本不提示的固定等待和输入仍不提示。每个目标先执行 A、D（各间隔 100ms），再向下滚配置次数；无论次数是否为 0，均等待 1000ms 后点击目标。入口、补齐、购买、兑换和二次确认沿用上述 run 级倒计时规则。点击兑换后必须双采样命中全局 `ammo.confirm` 用户参考图并点击区域中心，再以 `ammo.success` 模板差异确认完成。成功逐项立即保存；制作购买连续 3 次仍无法继续时阻断账号，子弹购买或人工确认异常只冻结当前目标并结束当前账号本轮。应用启动保持暂停；用户点击“继续”并通过 preflight 后立即启用 scheduler。轮次启动时，全部已到期业务先按账号配置顺序分桶，同账号制作、子弹、限时商品与交易行连续执行；同一账号多台已到期制作台一次性收取，不再拆成一台一轮。未来制作任务仍按计划时间追加，只用于后继判断，不得提前执行。未来下一任务同账号且已逾期或与当前任务相差不超过 10 分钟时保持游戏在线，到期后直接继续；不同账号正常关闭旧游戏并切号；下一任务尚未到期且间隔超过 10 分钟时关闭游戏，交回 scheduler 到点重启。制作、登录等账号级失败记录后关闭旧游戏并继续下一账号；可定位子弹失败不阻断该账号后续制作和其他子弹，系统失败、休眠跳变或紧急停止保留游戏现场。页面提供后端权威投影驱动的滚动未来 24 小时时间轴；制作与子弹任务按原计划时间展示，10 分钟内任务只做视觉合并，暂停期间逾期任务显示“0 分钟后”，失败任务可用 `special_ops_confirm_station_state` 或 `special_ops_confirm_ammo_state` 单项判定。限时商品页面就绪超时会补偿重试一次，重试排在队首，第二次仍超时则标记为失败，不再出现打开页面等待后直接关游戏且不留标记的情况。工具不保存或输入 QQ 密码。

> 联网利润筛选默认关闭。启用后，只有绑定稳定利润规则的已配置子弹才会在每日兑换时间至利润截止时间之间进入轮次；KKRB 为主数据源，仅主源整体失败时使用 Moligod 隐藏 WebView 备用。截止时冻结当日剩余账号与子弹目标，并按固定最低总利润 10,000 执行最终查询；低于阈值当天轮空，目标缺失、来源失败或利润无效只在 5 分钟后补查一次，仍失败则当天轮空。截止结果按账号与子弹目标持久化，截止后新增目标不进入当日范围，不再绕过利润条件。

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
| `profile-system` | 多配置快照切换（含特勤处 `specialOps`）、revision 防陈旧写入、复制、删除、单配置导入/导出 |
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
bun run tauri dev                 # 完整桌面开发（需管理员 PowerShell）
bun run build                     # tsc && vite build
bun run test                      # Vitest 前端单元测试
bun run test:coverage             # 全量前端覆盖率与阈值检查
bun run check                     # TypeScript、测试、coverage、fmt、Clippy、Rust 测试
cargo check --manifest-path src-tauri/Cargo.toml   # Rust 编译检查
cargo test --manifest-path src-tauri/Cargo.toml    # Rust 单元测试
```

Windows 桌面版以管理员权限运行，软件启动时显示一次 UAC，后续 WeGame 切号不重复提权。`bun run tauri dev` 必须从管理员 PowerShell 执行；仅运行 `bun run dev` 做浏览器 UI 开发时不要求管理员权限。

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

## 特勤处新增市场任务

- 限时商品检查固定在 Asia/Shanghai 每日 12:00、20:00 执行；命中 9 个识色区域任一配置颜色时只提示人工检查，不自动购买。
- 交易行购买窗口为每日 02:00–04:00；OCR 价格不高于设定值才点击购买，随后点击独立配置的最终确认点，按配置次数完成后切换账号，04:00 后本日任务关闭。
- 限时商品配置为全局；交易行配置位于默认业务配置与账号独立业务配置，账号关闭独立设置时继承默认值，可分别设置启用状态、购买次数、商品备注、最高价与商品入口点击点。
- 试运行提供限时商品检查、交易行安全识别、交易行单次试买，均不写正式周期结果或购买次数；限时商品与交易行试运行正常结束后将鼠标停放到 `runtime.mouseParking`，其他试运行不增加该动作。`ammo.researchDepartment` 使用模板识别与点击区域，限时商品仅保留页面就绪超时配置；`market.entry` 使用模板识别与点击区域，`market.price` 只做 OCR，`market.confirm` 为独立最终购买点击点。识色测试显示 9 个区域双采样详情，颜色 1/2 使用原生颜色面板、系统吸管或 Hex 输入设置。
