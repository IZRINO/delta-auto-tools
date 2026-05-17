# AGENTS.md

## Project reality

- **开发环境**：Windows（当前仓库路径 `D:/code/ai/sjz/delta-auto-tools`），所有命令在 Windows + Bun 下测试通过
- 当前仓库是 **Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust** 的桌面工具，产品名为"三角洲行动工具"（Delta Auto Tools），为游戏《三角洲行动》提供辅助功能。
- 当前产品由三部分原生能力组成：
  1. **Morse 识别工作台**：主界面负责设置、识别结果、历史记录；overlay 负责连续区域框选。核心流程：截取屏幕区域 → 二值化 → 轮廓检测 → 摩斯密码解码 → 自动输入结果。
  2. **计时器工作台**：主界面负责多个计时器卡片、总开关、透明窗口位置与字体透明度设置；透明窗口负责置顶点击穿透显示倒计时。核心流程：自定义快捷键 → 触发一个或多个计时器 → 透明窗口按卡片顺序逐行显示倒计时。
  3. **Delta 工具接口层**：通过 Tauri commands 暴露 Wegame 认证、QQ/微信/QQSafe 鉴权和游戏数据查询能力，当前阶段以原生命令与存储为主，尚未接入前端页面（前端仅有 `ToolPlaceholderPage` 占位组件）。
- 前端已接入 Tailwind CSS v4 与 shadcn/ui（`radix-mira` 风格，remixicon 图标库）。这些是当前界面基础设施的一部分。
- 原生能力通过 Tauri commands 暴露，核心逻辑位于 `src-tauri/src/morse/*`、`src-tauri/src/timer/*` 与 `src-tauri/src/delta/*`，不是 HTTP 服务。

## AI 输出规范

- **所有 AI 输出必须使用中文**，包括代码注释、解释说明、错误提示和用户交互内容
- 技术术语（如 React、TypeScript、Tauri 等）保持英文原名，其余描述使用中文
- 代码中的字符串、错误信息、UI 文案使用中文
- 生成的文档、注释、commit message 使用中文

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
- `bun run tauri` -> Tauri CLI（如 `bun run tauri dev` / `bun run tauri build`）
- `bun run tauri dev` -> 完整桌面开发流程（先启动 Vite dev server，再启动 Tauri）
- `bun run tauri build` -> 桌面构建流程
- `bun run test` -> Vitest 单元测试（前端）
- `bun run test:coverage` -> 前端覆盖率输出（仅覆盖 `src/components/app/morse-utils.ts`）
- `cargo check --manifest-path src-tauri/Cargo.toml` -> 检查 Rust/Tauri 编译
- `cargo test --manifest-path src-tauri/Cargo.toml` -> Rust 单元测试（含 game.rs、repo.rs 等后端测试）

PM2 开发编排（`ecosystem.config.cjs`）：将 Vite 和 Tauri 拆为两个独立 PM2 进程，`delta-auto-tools-tauri` 启动前等待端口 1420。

## 前端代码结构

```
src/
├── App.tsx                     # 应用根组件：Sidebar 壳层 + MorsePage/TimerPage
├── App.css                     # 主题 token（@theme）与 overlay 样式
├── main.tsx                    # React 入口，mount <App />，含 TooltipProvider
├── main.tsx                    # 提供 TooltipProvider
├── hooks/
│   └── use-mobile.ts           # 响应式断点 hook
├── lib/
│   └── utils.ts                # tailwind-merge + clsx 工具函数
├── components/
│   ├── ui/                     # shadcn/ui 基础组件（~60 个）
│   └── app/
│       ├── morse-page.tsx      # Morse 页面容器：状态编排、三步向导流
│       ├── morse-panels.tsx    # 面板子组件（Selection/Workbench/Result/History）
│       ├── morse-overlay.tsx   # overlay 窗口框选 UI
│       ├── morse-types.ts      # 前端 TypeScript 类型定义与常量
│       ├── morse-utils.ts      # 纯逻辑工具函数（序列化、格式化、热键解析）
│       ├── morse-utils.test.ts # Morse 前端测试文件
│       ├── timer-page.tsx      # 计时器页面、透明窗口与位置设置 UI
│       ├── timer-types.ts      # 计时器前端 TypeScript 类型定义与常量
│       ├── timer-utils.ts      # 计时器纯逻辑工具函数（序列化、格式化、热键复用）
│       ├── timer-utils.test.ts # 计时器前端测试文件
│       └── tool-placeholder-page.tsx  # Delta 工具模块占位组件（未开放状态）
```

### 前端核心模式

- **入口链路**：`index.html` → `src/main.tsx` → `src/App.tsx`
- `App.tsx` 判断 `?mode=overlay` / `?mode=timer-display` / `?mode=timer-position` 参数：overlay 模式直接渲染对应透明窗口；桌面模式渲染 `SidebarProvider` + 侧边栏 + 当前工具壳层
- 当前有两个真实工具页面（Morse、计时器），侧边栏在“当前工具”下切换
- `ToolPlaceholderPage` 接收 `title` / `shortLabel` / `description` 参数，展示"未开放"状态——Delta 命令的 UI 尚未接入
- **Morse 状态编排**：`morse-page.tsx` 负责所有状态管理，子组件只接收 props
- **计时器状态编排**：`timer-page.tsx` 负责计时器表单、透明窗口状态订阅、位置设置与自动保存
- **autosave 模式**：表单变更后 debounce 400ms（`AUTOSAVE_DELAY_MS`）自动调用 `morse_save_settings`。使用 `autosaveVersionRef` 防止陈旧保存覆盖
- **热键录制**：录制时调用 `morse_set_hotkey_recording(true)` 暂停被动热键监听，录制后恢复。按 Escape 取消恢复旧值
- 浏览器预览模式（非 Tauri shell）会禁用所有原生命令操作，显示提示信息

## 原生代码结构

```
src-tauri/src/
├── lib.rs                      # Tauri Builder 入口，注册所有 commands
├── main.rs                     # Windows 入口
├── morse/
│   ├── mod.rs                  # MorseState、命令注册、识别流程编排、持久化
│   ├── types.rs                # Morse 数据结构（MorseSettings/MorseRunResult/HistoryEntry 等）
│   ├── decoder.rs              # 摩斯密码 → 数字解码（仅 0-9）
│   ├── input.rs                # enigo 自动键盘输入
│   ├── input_listener.rs       # willhook 底层键盘钩子（Windows only）
│   ├── overlay.rs              # overlay 多步骤框选会话
│   ├── recognition.rs          # 截屏 + 二值化 + 轮廓检测 + 解码链路
│   └── settings.rs             # morse_settings.json 持久化
├── timer/
│   ├── mod.rs                  # TimerState、命令注册、透明窗口、位置设置、运行态编排
│   ├── types.rs                # TimerSettings/TimerItem/TimerBootstrap 等 DTO
│   ├── hotkey.rs               # willhook 底层键盘钩子（Windows only）
│   └── settings.rs             # timer_settings.json 持久化
└── delta/
    ├── mod.rs                  # 模块声明 + initialize()
    ├── commands.rs             # 所有 Delta Tauri commands + DTO 定义
    ├── constants.rs            # 常量（appid、URL、referer 等）
    ├── error.rs                # DeltaError 枚举 + 各类型转换
    ├── response.rs             # ApiResponse<T> 泛型响应结构
    ├── state.rs                # DeltaState（repo + buckets + pending）
    ├── client/
    │   ├── mod.rs
    │   ├── headers.rs          # 浏览器模拟请求头
    │   ├── http.rs             # build_client()：reqwest Client 构建
    │   └── ide.rs              # IdeCall 封装（IDE 网关表单请求）
    ├── services/
    │   ├── mod.rs
    │   ├── game.rs             # GameService：游戏数据查询（IDE gateway）
    │   ├── qq_auth.rs          # QQ 扫码登录 + 鉴权
    │   ├── qq_safe.rs          # QQSafe 扫码登录 + 封禁查询
    │   ├── wechat_auth.rs      # 微信扫码登录 + 鉴权
    │   └── wegame_auth.rs      # Wegame QQ/微信登录 + 宝箱/抽卡
    ├── storage/
    │   ├── mod.rs
    │   └── repo.rs             # DeltaRepo（SQLite，单表 delta_accounts）
    ├── resources/
    │   ├── ammo.json           # 空数组（未使用，配置在 game_config.rs）
    │   └── accessory.json      # 空数组（未使用，配置在 game_config.rs）
    └── utils/
        ├── mod.rs
        ├── cookies.rs          # cookie 解析/序列化
        ├── encoding.rs         # GBK/URL 解码
        ├── game.rs             # 枪械/弹药/配件映射 + bind-role JS 解析
        ├── game_config.rs      # 弹药和配件内置配置（编译期常量）
        ├── hashes.rs           # 哈希计算
        ├── html.rs             # HTML 解析工具
        ├── jsonp.rs            # JSONP 解析
        └── time.rs             # 时间戳工具
```

- **原生入口链路**：`src-tauri/src/main.rs` → `src-tauri/src/lib.rs`
- `lib.rs` 中的 `run()` 在 `setup` 回调中依次初始化 `morse::initialize()`、`delta::initialize()` 和 `timer::initialize()`，然后通过 `app.manage()` 注册状态
- `tauri::generate_handler![]` 中列出所有命令，新增命令必须同步添加到这里和 `src-tauri/capabilities/default.json`

## Tauri commands

### Morse 命令面

| 命令 | 说明 |
|------|------|
| `morse_get_bootstrap` | 获取初始状态（settings + history + latestRun + hotkeyError） |
| `morse_save_settings` | 保存设置，热键变化时重启监听器，失败回滚 |
| `morse_set_hotkey_recording` | 暂停/恢复热键监听（录制时调用） |
| `morse_begin_region_selection` | 进入 overlay 框选模式 |
| `morse_overlay_submit_selection` | overlay 提交框选结果 |
| `morse_overlay_cancel_selection` | overlay 取消框选 |
| `morse_run_recognition` | 执行识别流程（autoType 可选，默认 true） |

### 计时器命令面

| 命令 | 说明 |
|------|------|
| `timer_get_bootstrap` | 获取计时器初始状态（settings + runs + hotkeyError） |
| `timer_save_settings` | 保存计时器设置，总开关关闭时停止监听并隐藏透明窗口 |
| `timer_trigger` | 手动触发一个或多个计时器 |
| `timer_begin_position_selection` | 打开固定大小的位置设置窗口 |
| `timer_position_commit` | Enter 保存透明窗口位置 |
| `timer_position_cancel` | Esc 取消位置设置 |
| `timer_position_moved` | 位置设置窗口拖动时暂存坐标 |

### Delta 命令面

**账号与鉴权**：
- `delta_list_accounts` / `delta_delete_account`
- `delta_qq_get_login_qr` / `delta_qq_poll_login_status` / `delta_qq_get_access_token` / `delta_qq_update_access_token`
- `delta_wechat_get_login_qr` / `delta_wechat_poll_status` / `delta_wechat_get_access_token` / `delta_wechat_update_access_token`
- `delta_qqsafe_get_login_qr` / `delta_qqsafe_poll_status` / `delta_qqsafe_get_access_token` / `delta_qqsafe_get_banned_list`

**Wegame**：
- `delta_wegame_qq_get_login_qr` / `delta_wegame_qq_poll_status` / `delta_wegame_qq_get_access_token`
- `delta_wegame_wechat_get_login_qr` / `delta_wegame_wechat_poll_status` / `delta_wegame_wechat_get_access_token`
- `delta_wegame_open_treasure_gift` / `delta_wegame_draw_daily_card`

**游戏数据**（返回 `ApiResponse<Value>`，`code=0` 为成功）：
- `delta_game_get_items(typeId, subType, itemId?)` — 游戏物品查询
- `delta_game_get_config`
- `delta_game_get_price(args, withRecent?)` — 物价查询
- `delta_game_get_firearm_mod_list(page, pageSize)` — 枪械改装方案
- `delta_game_get_recommendation(place)` — 地图推荐装备
- `delta_game_get_record(auth)` — 战绩记录（含 gun + operator）
- `delta_game_get_player(auth)` — 玩家信息
- `delta_game_get_assets(auth)` — 资产查询
- `delta_game_get_logs(auth, logType, page)` — 操作日志
- `delta_game_get_recent(auth)` — 近期对局
- `delta_game_get_achievement(auth)` — 成就
- `delta_game_get_password(auth)` — 地图保险密码（返回 Map<地图名, 密码>）
- `delta_game_get_manufacture(auth)` — 制造列表
- `delta_game_get_guns(gunId)` — 枪械详情（含弹药/配件 enrich）
- `delta_game_get_bind(auth)` — 角色绑定

## UI and workflow constraints

- 保持白色桌面工具风格，不要改回模板首页或营销页。
- `?mode=overlay` 必须继续可用，不要引入路由来替代它。
- `?mode=timer-display` 和 `?mode=timer-position` 必须继续由 `App.tsx` 查询参数分支进入，不要引入路由替代。
- 区域选择应保持"一次进入 overlay，连续完成多个框选"。
- overlay 必须保持透明背景，避免重灰幕遮挡底层屏幕内容。
- 计时器透明窗口必须保持无边框、透明、置顶、点击穿透，避免挡游戏。
- 计时器总开关关闭后必须隐藏透明窗口、停止热键监听并保留本地配置。
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
- Tailwind v4 使用 CSS-first 方案（`@import "tailwindcss"`），主题 token 在 `src/App.css`；**不存在** `tailwind.config.js`
- 优先复用 `src/components/ui/*` 中已有基础组件（基于 shadcn/ui 的 radix-mira 风格，remixicon 图标库）
- `src/components/app/morse-page.tsx` 负责容器与状态编排；展示块拆在 app 子组件中，纯逻辑放 `morse-utils.ts`
- `src/App.css` 仅承载主题 token 与 overlay 相关样式；所有桌面壳层样式改用 shadcn/ui + Tailwind
- form 状态使用转层模式：`MorseSettings`（原始类型，int 字段）↔ `MorseSettingsForm`（表单类型，string 字段），通过 `settingsToForm()` / `parseSettingsForm()` 转换

## Native-side conventions

### Morse 端

- `src-tauri/src/morse/mod.rs` 负责状态、命令注册、热键协调与识别流程调度。包含 `MorseState` （单 `Mutex<MorseStateInner>` + 独立 `Mutex<Option<PassiveHotkeyListener>>`）和 `run_recognition_flow()` 编排函数
- `src-tauri/src/morse/types.rs` 定义了所有 Morse 数据结构（`MorseSettings`、`MorseRunResult`、`MorseRegionDetail`、`HistoryEntry`、`MorseBootstrap`、`RegionSelectionProgress`、`RegionSelectionOutcome`、`RegionSelectionKind`）
- `src-tauri/src/morse/overlay.rs` 负责多步骤框选会话；中途取消不应污染已保存配置
- `src-tauri/src/morse/settings.rs` 的持久化文件是 `morse_settings.json`
- `src-tauri/src/morse/decoder.rs` 解码器仅支持 0-9 数字（10 种模式），不包含字母
- `src-tauri/src/morse/input.rs` 使用 `enigo` crate 模拟键盘输入，通过 `spawn_blocking` 在阻塞线程中逐字符输入
- `src-tauri/src/morse/recognition.rs` 识别链路：截屏（xcap）→ 二值化 → 轮廓检测 → 匹配每个区域的轮廓数 → 解码
- 锁被污染时统一返回中文错误"已损坏"
- 修改原生命令时，必要时同步更新 `src-tauri/capabilities/default.json` 和 `src-tauri/src/lib.rs` 中的 `generate_handler![]`

### 计时器端

- `src-tauri/src/timer/mod.rs` 负责状态、命令注册、透明窗口创建/销毁、位置设置窗口和倒计时 tick 编排。
- `src-tauri/src/timer/types.rs` 定义所有计时器数据结构（`TimerSettings`、`TimerItem`、`TimerDisplaySettings`、`TimerBootstrap`、`TimerRunState` 等）。
- `src-tauri/src/timer/settings.rs` 的持久化文件是 `timer_settings.json`。
- `src-tauri/src/hotkeys.rs` 使用 `willhook` crate 注册全局共享底层键盘钩子；Morse 与计时器都通过同一个 `HotkeyManager` 注册 scope，避免多个 keyboard hook 互相抢占导致安装失败。
- 相同快捷键的计时器会分组到同一个 action 并同时触发。
- 计时器透明窗口 label 是 `"timer-display"`，位置设置窗口 label 是 `"timer-position"`。
- 计时器透明窗口宽度可由用户调整，最小宽度 320px；高度按计时器数量计算，避免多于 3 个计时器时出现滚动条。
- 计时器卡片顺序由 `settings.timers` 数组顺序决定；设置页拖动排序后，透明窗口按相同顺序逐行显示。
- 计时结束后运行态保持 `remainingSeconds=0` 与 `status=Finished`，前端显示为高亮斜体 `0`。
- 修改计时器命令或窗口 label 时，同步更新 `src-tauri/src/lib.rs` 和 `src-tauri/capabilities/default.json`。

### Delta 端

- `src-tauri/src/delta/commands.rs` 负责 Delta DTO、Tauri commands、账号解析与持久化编排
- `src-tauri/src/delta/services/` 下按领域拆分 QQ / WeChat / QQSafe / Wegame / Game 逻辑，不要额外引入与仓库现状不一致的 `models/handlers` 架构
- `src-tauri/src/delta/storage/repo.rs` 使用 SQLite 单表 `delta_accounts` 承载不同账号类型

**AccountKind 枚举（5 种变体）**：
- `Qq` — QQ 账号
- `Wechat` — 微信账号
- `QqSafe` — QQSafe 安全中心
- `WegameQq` — Wegame QQ 登录
- `WegameWechat` — Wegame 微信登录

新增账号类型应优先扩展此枚举，对应的 DB 存储使用 `kind.as_str()` / `AccountKind::from_str()` 序列化。

### Rust serde conventions（全仓通用）

所有对外序列化的 Rust 结构体 **必须** 使用 `#[serde(rename_all = "camelCase")]`：
- Delta 端：`ApiResponse<T>`、`DeltaAccountRecord`、`AccountKind`、服务 DTO（`QqLoginQr`、`WechatQr`、`WegameTicket`、`GameAuth` 等）、请求 struct（`CommandOptions`、`AccountCookieRequest` 等）
- Morse 端：`MorseSettings`、`MorseBootstrap`、`RegionRect`、`MorseRunResult`、`HistoryEntry`、`RegionSelectionProgress`、`RegionSelectionOutcome` 等
- 计时器端：`TimerSettings`、`TimerItem`、`TimerDisplaySettings`、`TimerBootstrap`、`TimerRunState`、`TimerSelectionOutcome` 等

### Delta 命令返回模式（与 Morse 不同）

- Delta 命令返回 `Result<ApiResponse<T>, DeltaError>`，其中 `ApiResponse` 携带 `code`/`msg`/`data`，成功时 `code=0`，msg 为中文描述
- Morse 命令返回 `Result<T, String>`，直接返回数据或中文错误字符串
- `DeltaError` 显式实现了 `Serialize`（序列化为错误信息字符串），用于 Tauri IPC 传输。包含变体：`Request`、`Storage`、`Parse`、`AccountNotFound`、`InvalidInput`

### Delta 命令 DTO 模式

每个 Delta command 都有对应的请求 struct，前端通过 camelCase JSON 反序列化：
- `AccountCookieRequest`：支持显式 `cookie` 或 `accountId` 两种指定方式
- 所有请求 DTO 都带有可选 `options: Option<CommandOptions>` 字段（目前仅 `insecureSkipTlsVerify`）
- 游戏数据命令的鉴权统一使用 `GameAuth { openid, access_token, acctype }` 结构

### IDE 网关模式（GameService 核心）

所有游戏数据查询通过腾讯 IDE 网关进行：`https://comm.ams.game.qq.com/ide/`。

`IdeCall` 结构（`src-tauri/src/delta/client/ide.rs`）封装了：
- `iChartId` / `sIdeToken` — 接口凭证对（如 `352143`/`YWRywA` 为物品查询，`319386`/`zMemOt` 为战绩日志）
- `param` — 业务参数的 JSON 字符串
- 可选的 `method` 和 `source`

请求以表单格式 POST 到 IDE 网关，每次携带统一 referer。每个游戏端点在 `GameService` 中用固定的 chartId/token 对。

### reqwest HTTP 客户端模式

`delta::client::http::build_client()` 创建 reqwest Client 时固定使用：
- `cookie_provider`（Arc\<Jar\>，由调用方持有复用）
- `default_headers(browser_headers())`（模拟浏览器）
- `redirect(Policy::none())`（不自动跟随重定向）
- `danger_accept_invalid_certs` 可选（通过 `HttpOptions` 控制）

### Delta 状态管理

`DeltaState` 使用多个独立锁分别保护不同数据：
- `repo: DeltaRepo` — 直接持有（SQLite 连接内部自带 Mutex）
- `buckets: Mutex<HashMap<i64, DeltaAccountRecord>>` — 内存账号缓存，与 DB 同步
- `pending: Mutex<HashMap<String, PendingSession>>` — 扫码登录中会话（QQ/Wegame QQ）

账号持久化使用 `persist_account()` 辅助函数（`commands.rs`）：DB upsert + 内存 buckets 同步更新。
扫码登录 pending 会话使用 `remember_pending()` 函数存储。

### Morse 状态管理（与 Delta 不同）

`MorseState` 使用 **单个** `Mutex<MorseStateInner>` 包裹所有可变字段，外加独立的 `Mutex<Option<PassiveHotkeyListener>>`。锁被污染时返回中文错误"已损坏"。

`MorseStateInner` 包含：`settings`、`history`（VecDeque，上限 1000）、`latest_run`、`next_history_id`、`pending_selection`、`run_in_progress`、`hotkey_error`。

### GameService 编译期配置

`src-tauri/src/delta/utils/game_config.rs` 在编译期以内联常量形式定义了弹药和配件配置：
- `AMMO_CONFIG`：18 种口径的弹药列表（名称 + 等级）
- `ACCESSORY_CONFIG`：54 个配件槽位 ID 到中文名称的映射

通过 `built_in_ammo_config()` / `built_in_accessory_config()` 函数在运行时构造 `HashMap`。这些配置用于 `get_guns()` 接口的 `enrich_gun_detail()` 丰富化处理。

**注意**：`src-tauri/src/delta/resources/ammo.json` 和 `accessory.json` 是空数组 `[]`，**并未被使用**。实际的弹药/配件数据来自 `game_config.rs` 的 Rust 常量。

### 被动热键监听（Windows only）

`src-tauri/src/morse/input_listener.rs` 使用 `willhook` crate 注册底层键盘钩子：
- 独立线程轮询键盘事件，匹配热键组合（modifier + primary key）
- 录制热键时通过 `morse_set_hotkey_recording` 暂停监听（`set_paused(true)`），并 drain 积压事件
- 非 Windows 平台直接返回错误，不做降级处理
- 热键绑定 parser 支持：`Ctrl+Shift+F2`、`F1`、`Ctrl+Alt+K` 等格式

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

overlay 窗口通过 `?mode=overlay&slots=0,1,2` 或 `?mode=overlay&slot=0` 查询参数进入 overlay 模式。

## 测试模式

### 前端测试（Vitest）
- `src/components/app/morse-utils.test.ts` — Morse 前端测试，测试工具函数
- `src/components/app/timer-utils.test.ts` — 计时器前端测试，测试设置转层与倒计时格式化
- Vitest coverage 配置只包含 `morse-utils.ts`

### Rust 测试（cargo test）
- `src-tauri/src/morse/mod.rs` — 测试 history push limit
- `src-tauri/src/morse/types.rs` — 测试 settings 默认值
- `src-tauri/src/morse/decoder.rs` — 测试解码器和未知 pattern 错误
- `src-tauri/src/timer/types.rs` — 测试计时器默认值
- `src-tauri/src/timer/settings.rs` — 测试计时器设置读写与反序列化错误
- `src-tauri/src/timer/hotkey.rs` — 测试计时器快捷键解析
- `src-tauri/src/timer/mod.rs` — 测试透明窗口尺寸计算与设置校验
- `src-tauri/src/delta/commands.rs` — 测试 DTO 反序列化
- `src-tauri/src/delta/storage/repo.rs` — 测试 upsert、list、delete（使用 tempfile）
- `src-tauri/src/delta/utils/game.rs` — 测试 caliber 标准化、bind-role 解析、enrich_gun_detail
- `src-tauri/src/delta/services/game.rs` — 核心测试文件，使用 `mockito` mock HTTP 服务端，覆盖所有 game API 端点

### GameService 测试模式
`src-tauri/src/delta/services/game.rs` 中的测试使用：
- `mockito::Server` 启动 mock HTTP 服务器
- `make_service()` 辅助函数构造带有 mock URL 的 `GameService`
- `ide_form()` 辅助函数按 IDE 网关格式构造 expected request body
- 每个测试验证：请求参数匹配 + 响应解析正确 + mock assertion

## Repo-specific cautions

- 使用 **Bun**，不要切换到 npm / pnpm / yarn
- 不要虚构仓库中不存在的 lint/test/CI 命令
- `README.md`、`AGENTS.md` 和 `CLAUDE.md` 需要随重大功能变更一起更新
- 仓库当前允许提交项目级 skills 目录：`.agents/skills/` 与 `.claude/skills/`；不要把它们误当成本地垃圾直接删除
- 忽略本地或生成产物：`node_modules`、`dist`、`src-tauri/target`、`.claude/worktrees/`、`.claude/settings.local.json`、`temp/`、`test-results/`
- 不存在 `tailwind.config.js` — Tailwind v4 通过 CSS `@import "tailwindcss"` 配置
- `GameService` 的弹药/配件配置已内联在 `game_config.rs`，**不需要**仓库根目录下的 `ammo.php` / `accessory.php`
- 前端仅对 `src/components/app/morse-utils.ts` 有测试覆盖
- 新增 Tauri command 必须同时注册到 `src-tauri/src/lib.rs` 的 `generate_handler![]` 和 `src-tauri/capabilities/default.json`
- 仓库根目录的 `ammo.json` 和 `accessory.json`（`resources/` 下）为空数组，未被实际使用

## If the project changes again

如果后续新增：
- 新的 Tauri commands
- 新的持久化结构
- 新的开发脚本
- 路由系统或新的应用壳层
- 新的项目级 skills / agents 目录约定

请在同一轮改动里同步更新 `README.md` 与 `AGENTS.md`。
