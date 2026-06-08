# AGENTS.md

## Project reality

- **开发环境**：Windows（当前仓库路径 `D:/code/ai/sjz/delta-auto-tools`），所有命令在 Windows + Bun 下测试通过
- 当前仓库是 **Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust** 的桌面工具，产品名为"三角洲行动工具"（Delta Auto Tools），为游戏《三角洲行动》提供辅助功能。
- 当前产品由四部分原生能力组成：
  1. **Morse 识别工作台**：主界面负责设置、识别结果、历史记录；overlay 负责连续区域框选。核心流程：截取屏幕区域 → 二值化 → 轮廓检测 → 摩斯密码解码 → 自动输入结果。
  2. **计时\计数器工作台**：主界面负责多个计时器/计数器卡片、计时器与计数器独立总开关、两个透明窗口位置与字体透明度设置；计时器透明窗口负责按卡片顺序逐行显示正/反计时和进度背景，计数器透明窗口负责逐行显示当前计数。核心流程：自定义快捷键 → 计时器触发后运行到结束且运行中不重复触发 / 计数器触发后累加 → 独立透明窗口置顶点击穿透显示结果。
  3. **连发器工作台**：主界面负责多张连发器卡片配置、卡片级不追加补齐、卡片级按键最小间距、卡片级启动抖动延迟/松手策略、全局补齐延迟、总开关、透明窗口显示/隐藏和位置设置；透明窗口负责按卡片顺序逐行显示触发键→目标键映射和运行状态。核心流程：按住触发键 → 按卡片配置的启动抖动和最小间距持续触发目标键 → 松开时未开启不追加的卡片按全局补齐延迟等待并自动补齐触发次数为偶数 / 开启不追加的卡片保持原始次数 → 独立透明窗口置顶点击穿透显示结果。
- 原生能力通过 Tauri commands 暴露，核心逻辑位于 `src-tauri/src/morse/*`、`src-tauri/src/timer/*`、`src-tauri/src/rapidfire/*`、`src-tauri/src/strategy/*` 与 `src-tauri/src/delta/*`，不是 HTTP 服务。
  4. **Delta 工具接口层**：通过 Tauri commands 暴露 Wegame 认证、QQ/微信/QQ安全中心/先遣服鉴权和游戏数据查询能力，前端已接入账号管理、游戏数据与工具箱页面。
- 前端已接入 Tailwind CSS v4 与 shadcn/ui（`radix-vega` 风格，remixicon 图标库）。这些是当前界面基础设施的一部分。
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
├── App.tsx                     # 应用根组件：Top Manifest Bar + Left Index Rail + Main Work Grid；overlay/display/position 模式 early return
├── App.css                     # 工业粗粝 token（@theme）、纸面网格、噪声与 overlay 透明例外
├── main.tsx                    # React 入口，mount <App />，含 TooltipProvider
├── main.tsx                    # 提供 TooltipProvider
├── hooks/
│   ├── use-mobile.ts           # 响应式断点 hook
│   └── use-delta-accounts.tsx  # Delta 账号全局 Context + Provider + hook
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
│       ├── timer-page.tsx      # 计时\计数器页面、透明窗口与位置设置 UI
│       ├── timer-types.ts      # 计时\计数器前端 TypeScript 类型定义与常量
│       ├── timer-utils.ts      # 计时\计数器纯逻辑工具函数（序列化、格式化、热键复用）
│       ├── timer-utils.test.ts # 计时\计数器前端测试文件
│       ├── rapidfire-page.tsx  # 连发器页面、透明窗口与位置设置 UI
│       ├── rapidfire-types.ts  # 连发器前端 TypeScript 类型定义与常量
│       ├── rapidfire-types.test.ts # 连发器前端测试文件
│       ├── strategy-page.tsx  # 攻略网站工作台：贴顶浏览器工具条 + 主窗口内嵌 WebView2 + 站点 Tab + 刷新档位
│       ├── strategy-utils.ts  # 攻略网站纯逻辑工具（站点常量、刷新档位、localStorage 读写）
│       ├── app-ui.tsx         # 桌面工作台共享视觉组件（PageHero/TacticalCard/SignalTile 等）
│       ├── tool-placeholder-page.tsx  # 未开放工具占位组件
│       ├── delta-accounts-page.tsx  # 账号管理页：账号 CRUD + 令牌生命周期 + 登录 Dialog
│       ├── delta-game-data-loader.ts # 游戏数据分批加载 Module（主数据批次 + 详情批次 + 版本号防陈旧）
│       ├── delta-game-data-loader.test.ts # 游戏数据分批加载测试
│       ├── delta-game-page.tsx      # 游戏数据页：仪表盘分批加载 + 查询工作台
│       ├── delta-toolbox-page.tsx   # 工具箱页：Wegame/QQ安全中心/先遣服按账号动态渲染
│       ├── delta-types.ts          # Delta 前端 TypeScript 类型定义与常量
│       ├── delta-types.test.ts     # Delta 类型常量测试（AccountKind camelCase 一致性等）
│       ├── delta-utils.ts          # Delta 工具函数（令牌状态、账号能力、显示名等）
│       ├── delta-utils.test.ts     # Delta 工具函数测试
│       ├── delta-account-card.tsx   # 账号小卡片组件（类型 Badge + UIN + 令牌状态 + 能力标签）
│       ├── delta-token-badge.tsx    # 令牌状态徽章组件
│       ├── delta-login-dialog.tsx   # 扫码登录 Dialog（6 种鉴权流程 × 3 种模式）
│       ├── delta-account-selector.tsx # 账号选择器横条组件（按类型过滤）
│       ├── delta-data-card.tsx      # 数据展示卡片组件（loading/error/retry 通用）
│       └── delta-query-workbench.tsx # 查询工作台（6 种参数化 API 动态表单）
```

### 前端核心模式

- **入口链路**：`index.html` → `src/main.tsx` → `src/App.tsx`
- `App.tsx` 判断 `?mode=overlay` / `?mode=timer-display` / `?mode=timer-position` / `?mode=counter-display` / `?mode=counter-position` / `?mode=rapidfire-display` / `?mode=rapidfire-position` 参数：overlay / display / position 模式直接渲染对应独立窗口；桌面模式渲染自定义三段式工业壳层（48px Top Manifest Bar、240px Left Index Rail、Main Work Grid）。Delta 工具不使用 overlay 模式，攻略网站不再使用 `?mode=strategy-browser` 独立窗口入口
- 当前有四个真实工具页面（Morse、计时器、连发器、攻略网站）和 Delta 三页，Left Index Rail 在“当前工具 / 三角洲行动 API / PINNED”下切换；当前项黑底反白并使用 Alert Red 标识。
- `ToolPlaceholderPage` 接收 `title` / `shortLabel` / `description` 参数，展示"未开放"状态——Delta 命令的 UI 尚未接入
- **攻略网站工作台（strategy-page）**：主窗口负责内置站点与用户自定义站点的集中管理（`localStorage` 前缀 `delta-auto-tools:strategy:user-sites`），页面顶部使用紧凑浏览器工具条承载站点横向 Tab、新增 / 删除自定义站点、自动刷新档位、手动刷新和系统浏览器打开；当前 URL 只在工具条中紧凑展示 / tooltip 展示，不再保留 PageHero 或大块说明卡。工具条下方定位宿主区域创建 label `strategy-content` 的 Tauri 子 WebView 真实导航当前外部 URL，并使用 `min-h-0 flex-1 overflow-hidden` 吃满主应用剩余高度；站点切换、手动刷新、自动刷新到期时会销毁并重建该子 WebView，主窗口 resize / 布局变化 / 滚动时同步 `setPosition` / `setSize`，组件卸载时关闭 `strategy-content`，避免切换工具页后遮挡主界面。自动刷新档位按站点持久化到 `delta-auto-tools:strategy:<site>:refresh-seconds`，允许值为关闭 / 30 秒 / 1 分钟 / 2 分钟 / 5 分钟 / 10 分钟；损坏值回落到关闭态。cookie、JS redirect、localStorage、同源 API 和人机验证由 WebView2 站点自身处理，不再默认使用 iframe/srcDoc，也不再打开 `strategy-browser` 独立窗口。`strategy_fetch_page` 保留为后端实验 / 兼容入口：Rust 端使用 Chrome 135 头抓取 HTML，共享 cookie jar，嗅探 `document.cookie = '...'; location.href = '...'` / `window.location.href = '...'` / `location.replace(...)` JS 重定向并最多跟随 3 次；命中 CC check 时返回 `challenge`。
- **Morse 状态编排**：`morse-page.tsx` 负责所有状态管理，子组件只接收 props
- **计时\计数器状态编排**：`timer-page.tsx` 负责计时器/计数器表单、两个透明窗口状态订阅、位置设置与自动保存
- **autosave 模式**：表单变更后 debounce 400ms（`AUTOSAVE_DELAY_MS`）自动调用 `morse_save_settings`。使用 `autosaveVersionRef` 防止陈旧保存覆盖
- **热键录制**：录制时调用 `morse_set_hotkey_recording(true)` 暂停被动热键监听，录制后恢复。按 Escape 取消恢复旧值
- 浏览器预览模式（非 Tauri shell）会禁用所有原生命令操作，显示提示信息
- **Delta AccountKind 序列化一致性**：Rust 端 `#[serde(rename_all = "camelCase")]` 将 `QqSafe`→`"qqSafe"`、`WegameQq`→`"wegameQq"`、`WegameWechat`→`"wegameWechat"`、`Pioneer`→`"pioneer"`；前端 `AccountKind` 必须使用这些 camelCase 字符串（不是 snake_case 的 `"qqsafe"`/`"wegame_qq"`/`"wegame_wechat"`）。`delta-types.test.ts` 中的 `AccountKind camelCase consistency` 测试守卫此约束
- **Delta 全局账号状态**：`DeltaAccountsProvider` 包裹整个应用，三页共享 `selectedAccountId`；切换页面后选中态保持
- **Delta 登录流程**：`LOGIN_FLOW_MODE_MAP` 将 6 种 `LoginFlowKind` 映射到 QQ 模式或微信模式；登录二维码和轮询只在前端传递一次性 `sessionKey`，cookie、access token、Wegame ticket、QQ安全中心 code 等凭据只保存在 Rust 状态/SQLite 中。
- **Delta 游戏数据分批加载**：`delta-game-data-loader.ts` 是游戏数据页的加载 Module；先请求 `player + record`，至少一个主请求成功后再请求 `assets + recent + achievement + password + bind`，并用版本号丢弃账号切换后的陈旧响应。
- **Delta 数据展示**：当前所有游戏 API 返回使用 `JSON.stringify` 原始展示；待 API 响应结构确认后替换为结构化渲染
- **Delta 账号选择器**：`DeltaAccountSelector` 按 `filterKinds` 过滤账号；当前实现会在选中账号不在过滤范围且存在匹配账号时自动切换到第一个匹配账号

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
├── rapidfire/
│   ├── mod.rs                  # RapidfireState、状态机、命令注册、透明窗口、位置设置
│   ├── types.rs                # RapidfireSettings/RapidfireCard/RapidfireBootstrap 等 DTO
│   └── settings.rs             # rapidfire_settings.json 持久化
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
    │   ├── qq_safe.rs          # QQ安全中心扫码登录 + 封禁查询
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
- `lib.rs` 中的 `run()` 在 `setup` 回调中依次初始化 `morse::initialize()`、`delta::initialize()`、`timer::initialize()` 和 `rapidfire::initialize()`，然后通过 `app.manage()` 注册状态
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
| `timer_get_bootstrap` | 获取计时\计数器初始状态（settings + runs + counterRuns + hotkeyError） |
| `timer_save_settings` | 保存计时\计数器设置，计时器/计数器各自总开关关闭时隐藏对应透明窗口并解绑对应快捷键 |
| `timer_trigger` | 手动触发一个或多个计时器 |
| `timer_counter_trigger` | 手动触发一个或多个计数器 |
| `timer_counter_reset` | 将指定计数器重置为设置的起始数 |
| `timer_begin_position_selection` | 打开固定大小的位置设置窗口（支持计时器/计数器目标） |
| `timer_position_commit` | Enter 保存透明窗口位置 |
| `timer_position_cancel` | Esc 取消位置设置 |
| `timer_position_moved` | 位置设置窗口拖动时暂存坐标 |

### 连发器命令面

| 命令 | 说明 |
|------|------|
| `rapidfire_get_bootstrap` | 获取连发器初始状态（settings + runs + hotkeyError） |
| `rapidfire_save_settings` | 保存连发器设置，总开关关闭时解绑快捷键并隐藏透明窗口 |
| `rapidfire_stop` | 停止所有运行中的连发 |
| `rapidfire_begin_position_selection` | 打开固定大小的位置设置窗口 |
| `rapidfire_position_commit` | Enter 保存透明窗口位置 |
| `rapidfire_position_cancel` | Esc 取消位置设置 |
| `rapidfire_position_moved` | 位置设置窗口拖动时暂存坐标 |

### 攻略网站命令面

| 命令 | 说明 |
|------|------|
| `strategy_fetch_page` | 兼容/实验拉取目标攻略页面：带完整 Chrome 135 头 + JS 重定向跟随（含 `window.location.href`）+ CC check 嗅探；命中人机验证时 `challenge` 字段非空 |
| `strategy_open_window` | 兼容入口：按 host 新建 / 复用 WebView2 子窗口直接打开外部 URL；主 UI 不依赖该命令 |

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
- `delta_game_get_record(accountId)` — 战绩记录（含 gun + operator）
- `delta_game_get_player(accountId)` — 玩家信息
- `delta_game_get_assets(accountId)` — 资产查询
- `delta_game_get_logs(accountId, logType, page)` — 操作日志
- `delta_game_get_recent(accountId)` — 近期对局
- `delta_game_get_achievement(accountId)` — 成就
- `delta_game_get_password(accountId)` — 地图保险密码（返回 Map<地图名, 密码>）
- `delta_game_get_manufacture(accountId)` — 制造列表
- `delta_game_get_guns(gunId)` — 枪械详情（含弹药/配件 enrich）
- `delta_game_get_bind(accountId)` — 角色绑定

## UI and workflow constraints

- 保持白色桌面工具风格，不要改回模板首页或营销页。
- `?mode=overlay` 必须继续可用，不要引入路由来替代它。
- `?mode=timer-display`、`?mode=timer-position`、`?mode=counter-display`、`?mode=counter-position`、`?mode=rapidfire-display` 和 `?mode=rapidfire-position` 必须继续由 `App.tsx` 查询参数分支进入，不要引入路由替代。
- 区域选择应保持"一次进入 overlay，连续完成多个框选"。
- overlay 必须保持透明背景，避免重灰幕遮挡底层屏幕内容。
- 计时器和计数器透明窗口必须保持无边框、透明、置顶、点击穿透，避免挡游戏。
- 计时器总开关关闭后必须隐藏计时器透明窗口、停止计时器快捷键监听并保留本地配置；计数器总开关关闭后必须隐藏计数器透明窗口、停止计数器快捷键监听并保留本地配置。
- 连发器总开关关闭后必须隐藏连发器透明窗口、停止连发器快捷键监听并保留本地配置。
- 热键输入应保持录制式交互；真正的解绑/重绑由 Rust 保存逻辑负责。
- `TooltipProvider` 已在 `src/main.tsx` 根部提供，依赖 tooltip 的组件应沿用该入口结构。

## UI and Styling Rules

- **整体视觉方向**：当前 UI 是 `DESIGN.md` 定义的 **Swiss Industrial Print × Declassified Tactical Control Board**，不是营销页、模板首页、普通后台卡片堆叠、旧 Sidebar + 圆角 Card + Hero，也不是全黑 CRT。主基底为工业纸面 Paper `#F1EFE8` / Bone `#DDD8CC`，Ink `#080808` 粗黑结构线，单一航空红 Alert Red `#E11919` 只用于当前选择、危险动作、运行态和关键焦点；Warning Amber `#A36A00` 与 Valid Green `#3F6B2A` 只用于语义状态。
- **仅使用 shadcn/ui 组件和 Tailwind CSS 进行样式设计**。禁止新增 `.desktop-*`、`.tactical-*` 等自定义 CSS 类；桌面页面样式通过 shadcn/ui、Tailwind 工具类和 `src/App.css` 主题 token 实现。详细设计规范见仓库根目录 `DESIGN.md`，重构边界见 `docs/ui-industrial-brutalist-refactor.md`。
- `src/App.css` 中的 `:root` 与 `@theme inline` 是全局视觉 token 来源。允许维护颜色、字体、半径、工程纸网格、纸面噪声和 overlay 基础样式；全局 `--radius: 0`，主窗口默认 90 度直角，不要堆叠 `rounded-*`、柔和阴影、玻璃态或大面积 raw color（例如 `bg-blue-500`）替代 token。
- 桌面主界面复用 `src/components/app/app-ui.tsx` 的共享工业语义层：`AppPage`（12 列 Work Grid）、`PageHero`（Macro Module Header）、`SignalTile`（Status Matrix Cell）、`TacticalCard`（FIELD UNIT）、`SectionHeader`（黑色机器标签条）、`ControlTile` / `InlineControl`（配置格）、`CardToolbar`、`SurfaceToggleGroup`、`SaveStateBadge`、`CardBody`。不要在每个页面重复手写一套 hero、统计卡、保存状态和 section header。
- 工作台页面采用“Macro Module Header + Status Matrix + FIELD UNIT / Command Unit / Data Well”的结构：顶部说明模块目的与状态，中部放开关/透明窗口/校准等关键控制，下部放可编辑配置行或日志历史；页面必须有巨大结构元素、高密度数据区和红色操作焦点。
- 攻略网站页是例外：为最大化网页占比，该页使用贴顶工业浏览器工具条 + `strategy-content` 内容宿主，不使用 PageHero / 大说明卡；不得隐藏主应用 Left Index Rail，也不得新增独立浏览器窗口替代主窗口内嵌 WebView。
- 表单仍使用 `FieldGroup` + `Field` + `FieldLabel` + `FieldContent`；开关设置放在 `ControlTile` 中；提示与异常优先用 `Alert` / `FieldError` / `Badge`，不要手写自定义 callout。
- 图标继续使用 `@remixicon/react`，Button 内图标必须设置 `data-icon="inline-start"` / `data-icon="inline-end"`；不要引入 lucide 或混用其他图标库。
- 透明窗口和位置设置窗口属于游戏叠加层，可以保留深色半透明 overlay 风格；不要套用主界面的白色卡片背景，也不要破坏无边框、透明、置顶、点击穿透约束。
- 设计改动必须保持功能不变：不要为了重构 UI 改 Tauri command 名称、查询参数 mode、状态机、保存逻辑或原生窗口 label。
- 新增复杂 UI 前先检查 `src/components/ui/*` 已安装组件和 `components.json` 配置；已有 shadcn/ui 组件能组合解决时，不新增第三方 UI 依赖。

## Frontend conventions

- 使用现有别名：`@/components`、`@/components/ui`、`@/lib`、`@/hooks`
- Tailwind v4 使用 CSS-first 方案（`@import "tailwindcss"`），主题 token 在 `src/App.css`；**不存在** `tailwind.config.js`
- 优先复用 `src/components/ui/*` 中已有基础组件（基于 shadcn/ui 的 radix-vega 风格，remixicon 图标库）
- 优先复用 `src/components/app/app-ui.tsx` 中的桌面工具共享展示组件；如果发现三个以上页面需要同一种视觉结构，应先扩展共享组件，而不是复制粘贴 Tailwind 片段。
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

- `src-tauri/src/timer/mod.rs` 负责状态、命令注册、计时器/计数器透明窗口创建/销毁、位置设置窗口和倒计时 tick 编排。
- `src-tauri/src/timer/types.rs` 定义所有计时\计数器数据结构（`TimerSettings`、`TimerItem`、`CounterItem`、`TimerDisplaySettings`、`TimerBootstrap`、`TimerRunState`、`CounterRunState` 等）。
- `src-tauri/src/timer/settings.rs` 的持久化文件是 `timer_settings.json`。
- `src-tauri/src/hotkeys.rs` 使用 `willhook` crate 注册全局共享底层键盘钩子；Morse 与计时器都通过同一个 `HotkeyManager` 注册 scope，避免多个 keyboard hook 互相抢占导致安装失败。
- 相同快捷键的计时器会分组到同一个 action 并同时触发。
- 计时器透明窗口 label 是 `"timer-display"`，位置设置窗口 label 是 `"timer-position"`；计数器透明窗口 label 是 `"counter-display"`，位置设置窗口 label 是 `"counter-position"`。
- `TimerSettings.timer_enabled` 控制计时器快捷键注册、计时器透明窗口显示和计时器运行态；`TimerSettings.counter_enabled` 控制计数器快捷键注册、计数器透明窗口显示和计数器运行态。旧 `enabled` 字段仅用于兼容旧配置，归一化后等于两个独立开关的并集。
- 计时器和计数器透明窗口宽度可由用户调整，最小宽度 320px；高度按卡片数量计算，避免多于 3 个项目时出现滚动条。
- 计时器卡片顺序由 `settings.timers` 数组顺序决定，计数器卡片顺序由 `settings.counters` 数组顺序决定；设置页拖动排序后，透明窗口按相同顺序逐行显示。
- 计时器支持 `Countdown`（10→0）和 `Countup`（0→10）两种方向；运行中重复快捷键触发会被忽略，结束后才能再次触发。
- 计时结束后运行态保持 `remainingSeconds=0` 与 `status=Finished`，前端按方向显示终值并高亮斜体。
- 计数器运行态保存在 `counter_runs`，快捷键触发时累加 1，`timer_counter_reset` 会恢复到 `start_value`。
- 计数器运行态独立持久化到 `timer_counter_state.json`（`src-tauri/src/timer/counter_state.rs`），与 `timer_settings.json` 平行：用户配置（`start_value` / hotkey / enabled）和运行态（实际累加值）分离。`initialize()` 加载时合并 `settings.counters` 与已保存的 runs（缺则用 `start_value`，孤儿 ID 丢弃）；每次累加 / reset / 应用关闭时通过 `persist_counter_runs` 落盘，孤儿 ID（counter 已删）自动清理，写盘失败不阻塞主流程。
- 修改计时器命令或窗口 label 时，同步更新 `src-tauri/src/lib.rs` 和 `src-tauri/capabilities/default.json`。

### 连发器端

- `src-tauri/src/rapidfire/mod.rs` 负责状态、命令注册、会话状态机编排、透明窗口创建/销毁、位置设置窗口、hotkey hold 回调协调与连发 worker 线程编排。
- `src-tauri/src/rapidfire/types.rs` 定义所有连发器数据结构（`RapidfireSettings`、`RapidfireCard`、`RapidfireBootstrap`、`RapidfireRunState`、`RapidfireRunStatus`、`RapidfireRect` 等）。
- `src-tauri/src/rapidfire/settings.rs` 的持久化文件是 `rapidfire_settings.json`。
- `RapidfireState` 使用单个 `Mutex<RapidfireStateInner>` 包裹所有可变字段；每张卡片的 `CardRuntime` 自带 `last_press_at: Arc<Mutex<Instant>>`，同一卡多 session 共享按键间距，不同卡片互不拖慢。
- `RapidfireStateInner` 包含：`settings`、`runs`（HashMap<cardId, CardRuntime>，每张卡可包含多个独立 session 与卡片级 last_press_at）、`pending_position`、`hotkey_error`。
- 连发器使用 `hotkeys::HotkeyManager` 的 hold 机制（`replace_hold_scope`/`clear_hold_scope`），注册范围为 `"rapidfire"`。
- 触发键可为单键或包含 Ctrl/Alt/Shift/Win 的组合键（例如 `Shift+-`），通过 `HoldAction::Down` 启动连发、`HoldAction::Up` 停止连发；组合键触发键按下时也会同时触发同主键的无修饰键绑定（例如 `Shift+1` 同时触发 `Shift+1` 和 `1`），松开修饰键只停止组合键 session 并保留无修饰键 session；先按主键再按修饰键只新增组合键 session，不重启已运行的无修饰键 session。
- 触发键主键支持范围：字母 A-Z、数字 0-9、F1-F12、Space、Enter、Tab、Esc、Backspace、方向键、Home/End/PageUp/PageDown/Insert/Delete、Alt、符号键（`;` `,` `.` `/` `\` `[` `]` `-` `=` `` ` `` `'`）。
- 同一快捷键可绑定多个连发器卡片，按下时同时为所有绑定卡片创建独立连发 session 和独立 OS worker 线程。
- 每次触发键 Down 都创建新的 session；同一卡片快速再次触发不会覆盖、取消或 abort 旧 session，旧 session 会在收到 Up 后按卡片补齐策略自行退出。
- 状态机以 session 为单位：`Firing → Stopping → Finished`；对外 `RapidfireRunState` 仍按 card 聚合，任一 session 存在时显示 `Firing`。
- 触发键 Up 只停止本次对应 session；count 为偶数则线程退出；count 为奇数时，未开启卡片级不追加的卡片在线程内额外触发一次目标键补齐为偶数，开启不追加的卡片直接以单数退出。
- 连发器透明窗口 label 是 `"rapidfire-display"`，位置设置窗口 label 是 `"rapidfire-position"`。
- `RapidfireSettings.rapidfire_enabled` 控制 hold 热键注册、透明窗口显示和运行态。
- 透明窗口宽度可由用户调整，范围 320-800px；高度按启用卡片数量计算。
- 连发间隔最小 1ms（`RAPIDFIRE_MIN_INTERVAL_MS`）。
- `RapidfireSettings.compensation_delay_min_ms` / `compensation_delay_max_ms` 控制奇数次数补齐前的随机等待范围；默认 100-150ms，可由 UI 全局设置。
- `RapidfireCard.min_press_spacing_ms` 控制当前卡片目标键最小触发间距；默认 80ms，范围 0-10000ms。旧 `RapidfireSettings.min_press_spacing_ms` 仅作为反序列化兼容默认值来源。
- `RapidfireCard.trigger_jitter_max_ms` / `cancel_jitter_on_release` 控制当前卡片按下触发键后的启动抖动延迟和抖动期间松手策略；旧 `RapidfireSettings.trigger_jitter_max_ms` / `cancel_jitter_on_release` 仅作为反序列化兼容默认值来源。
- 目标键通过 `enigo::Key` 模拟真实 `Press → 8-12ms 抖动等待 → Release`，不要使用 `Direction::Click` 作为连发主路径。
- 修改连发器命令或窗口 label 时，同步更新 `src-tauri/src/lib.rs` 和 `src-tauri/capabilities/default.json`。
- 连发器卡片支持按 `moveRapidfireCard` 顺序拖拽排序（与计时器拖拽实现一致：pointerdown 启动 / pointerup 收尾 / pointerenter 即时重排），卡片头部新增 `↕` DragButton；保留上移/下移按钮作为可访问性兜底。

### 攻略网站端

- `src-tauri/src/strategy/mod.rs` 暴露 `strategy_open_window` 与 `strategy_fetch_page`。默认 UI 路径不再用 iframe/srcDoc，也不再创建 `strategy-browser` 独立窗口：前端 `StrategyPage` 直接在主应用窗口内创建 label `strategy-content` 的 Tauri 子 WebView 真实导航外部 URL；切换站点 / 手动刷新 / 自动刷新时销毁并重建内容 WebView，窗口 resize、主页面布局变化和滚动时同步调整 bounds，组件卸载时关闭该子 WebView。`strategy_open_window` 保留旧 per-host top-level WebView2 外部 URL 窗口入口。`strategy_fetch_page` 保留兼容/实验用途：Rust 端用完整 Chrome 135 浏览器头拉取目标页面，`fetch_with_redirect` 在 reqwest 的 `Jar` 上共享 cookie 状态，嗅探 `document.cookie = '...'; location.href = '...'` / `window.location.href = '...'` / `location.replace(...)` 后写入 cookie 并继续向同源跳转目标再发起一次请求，最多跟随 `MAX_REDIRECT_DEPTH = 3` 次；命中 CC check 时返回 `challenge`，但主 UI 不再以代理 HTML 渲染作为默认路径。

### Delta 端
- `src-tauri/src/delta/services/` 下按领域拆分 QQ / WeChat / QQ安全中心 / Wegame / Pioneer / Game 逻辑，不要额外引入与仓库现状不一致的 `models/handlers` 架构
- `src-tauri/src/delta/storage/repo.rs` 使用 SQLite 单表 `delta_accounts` 承载不同账号类型

**AccountKind 枚举（6 种变体）**：
- `Qq` — QQ 账号
- `Wechat` — 微信账号
- `QqSafe` — QQ安全中心
- `WegameQq` — Wegame QQ 登录
- `WegameWechat` — Wegame 微信登录
- `Pioneer` — 先遣服登录

新增账号类型应优先扩展此枚举，对应的 DB 存储使用 `kind.as_str()` / `AccountKind::from_str()` 序列化。

**凭据边界**：
- 前端账号视图使用 `DeltaAccountView`，只暴露 `id`、`kind`、`uinOrOpenid`、`hasAccessToken`、`expiresAt` 和时间戳；不得向前端返回 `cookie_json`、`openid`、`access_token`、`extra_json`、Wegame ticket 或 QQ安全中心 code。
- QQ/QQ安全中心/Wegame QQ/先遣服扫码流程返回 `sessionKey`，轮询成功后 Rust 将 cookie 转存到一次性 pending 会话；获取令牌命令消费该 `sessionKey`，不能让前端传 cookie。
- 微信/Wegame 微信轮询成功后只返回 `sessionKey`，Rust pending 会话保存授权 code；获取令牌命令消费该 `sessionKey`。
- 游戏数据、QQ安全中心、Wegame、先遣服工具命令从 `accountId` 解析后端持有凭据，不接受前端传入的 openid/access token/ticket/code。
- `DeltaRepo` 写入 `cookie_json`、`access_token`、`extra_json` 前必须通过 `storage::secrets` 本地加密；启动时 `migrate_plaintext_secrets()` 迁移旧明文记录。

### Rust serde conventions（全仓通用）

所有对外序列化的 Rust 结构体 **必须** 使用 `#[serde(rename_all = "camelCase")]`：
- Delta 端：`ApiResponse<T>`、`DeltaAccountView`、`AccountKind`、服务 DTO（`QqLoginQr`、`WechatQr`、`WegameTicket`、`GameAuth` 等）、请求 struct（`AccountIdRequest`、`AccountSessionRequest`、游戏数据请求等）
- Morse 端：`MorseSettings`、`MorseBootstrap`、`RegionRect`、`MorseRunResult`、`HistoryEntry`、`RegionSelectionProgress`、`RegionSelectionOutcome` 等
- 计时器端：`TimerSettings`、`TimerItem`、`TimerDisplaySettings`、`TimerBootstrap`、`TimerRunState`、`TimerSelectionOutcome` 等

### Delta 命令返回模式（与 Morse 不同）

- Delta 命令返回 `Result<ApiResponse<T>, DeltaError>`，其中 `ApiResponse` 携带 `code`/`msg`/`data`，成功时 `code=0`，msg 为中文描述
- Morse 命令返回 `Result<T, String>`，直接返回数据或中文错误字符串
- `DeltaError` 显式实现了 `Serialize`（序列化为错误信息字符串），用于 Tauri IPC 传输。包含变体：`Request`、`Storage`、`Parse`、`AccountNotFound`、`InvalidInput`

### Delta 命令 DTO 模式

每个 Delta command 都有对应的请求 struct，前端通过 camelCase JSON 反序列化：
- `AccountIdRequest` 是账号型命令的标准输入，字段为 `accountId`
- `AccountSessionRequest` 是登录取令牌命令的标准输入，字段为一次性 `sessionKey`
- 游戏数据命令的鉴权由 Rust 根据 `accountId` 构造 `GameAuth { openid, access_token, acctype }`，前端不得构造或传入 `GameAuth`

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
- 不允许前端控制 TLS 校验；`HttpOptions` 不再包含 `insecureSkipTlsVerify`，客户端必须拒绝无效证书

### Delta 状态管理

`DeltaState` 使用多个独立锁分别保护不同数据：
- `repo: DeltaRepo` — 直接持有（SQLite 连接内部自带 Mutex）
- `buckets: Mutex<HashMap<i64, DeltaAccountRecord>>` — 内存账号缓存，与 DB 同步
- `pending: Mutex<HashMap<String, PendingSession>>` — 扫码登录中会话（QQ/Wegame QQ）

账号持久化使用 `persist_account()` 辅助函数（`commands.rs`）：DB upsert + 内存 buckets 同步更新，返回前必须转成 `DeltaAccountView` / `AccountLoginResult`。
扫码登录 pending 会话使用 `remember_pending()` / `pending_cookie()` / `consume_pending_cookie()`，sessionKey 有 5 分钟 TTL，取令牌流程必须一次性消费。

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

`src-tauri/src/hotkeys.rs` 使用 `willhook` crate 注册全局共享底层键盘钩子：
- Morse、计时器和连发器都必须通过同一个 `HotkeyManager` 注册 scope，避免多个 keyboard hook 互相抢占导致安装失败。
- 普通快捷键使用 `replace_scope`；连发器按住触发键使用 `replace_hold_scope` / `clear_hold_scope`，通过 `HoldAction::Down` / `HoldAction::Up` 回调通知。
- `HotkeyManager` 在注册时基于解析后的 `HotkeyBinding` 做跨 scope 冲突检测；不要用显示字符串或主键 label 自行比较。
- 冲突策略有一个显式例外：普通快捷键 scope `timer` 与 hold scope `rapidfire` 允许同键共存；运行时会先分发连发器 hold Down/Up，再分发计时器普通快捷键。Morse 与 Timer 普通快捷键冲突、Morse 与 Rapidfire hold 冲突仍必须拒绝。
- 热键绑定 parser 支持：`Ctrl+Shift+F2`、`F1`、`Ctrl+Alt+K`、`Shift+-`、单独 `Alt` 等格式，组合触发键能力属于连发器已完成特性，不得回退。
- 录制 Morse 热键时通过 `morse_set_hotkey_recording` 暂停 Morse scope（`set_scope_enabled("morse", false)`），录制后恢复。
- 非 Windows 平台直接返回错误，不做降级处理。

## Tauri 事件模式

Morse 通过 Tauri events 通知前端（emit_to "main"）：
- `"morse://run-finished"` — 识别完成后推送 `MorseRunResult`
- `"morse://selection-progress"` — 区域选择完成后推送 `RegionSelectionProgress`
- `"morse://hotkey-error"` — 热键执行出错时推送错误字符串

计时器通过 Tauri events 通知前端：
- `"timer://state-changed"` — 状态变更时推送 `TimerBootstrap`（同时推送到 timer-display / counter-display 窗口）
- `"timer://hotkey-triggered"` — 计时器快捷键触发后推送计时器 ID 列表
- `"timer://counter-triggered"` — 计数器快捷键触发后推送计数器 ID 列表
- `"timer://hotkey-error"` — 热键执行出错时推送错误字符串

连发器通过 Tauri events 通知前端：
- `"rapidfire://state-changed"` — 状态变更时推送 `RapidfireBootstrap`（同时推送到 rapidfire-display 窗口）
- `"rapidfire://hotkey-error"` — 热键执行出错时推送错误字符串

前端通过 `listen()` from `@tauri-apps/api/event` 订阅这些事件。

## Overlay 状态机

overlay 框选流程使用 `oneshot::Sender<RegionSelectionKind>` 实现完成通知：
1. 前端调用 `morse_begin_region_selection`，Rust 创建 overlay 窗口（label: `"morse-overlay"`），存储 `PendingSelection`（含 oneshot sender）
2. 前端在 overlay 窗口中完成框选，调用 `morse_overlay_submit_selection`
3. Rust 更新 staged_regions，全部完成后保存 settings 并发送 sender
4. 主窗口通过 await 在 oneshot receiver 上等待完成，拿到最终的 `RegionSelectionOutcome`

overlay 窗口通过 `?mode=overlay&slots=0,1,2` 或 `?mode=overlay&slot=0` 查询参数进入 overlay 模式。

识别截图时，overlay 上报的逻辑坐标会在 Rust 侧按显示器 scale factor 转换为 `xcap::Monitor::capture_region` 需要的物理坐标；高 DPI/多显示器下不要绕过 `region_to_capture_bounds()`。

## 测试模式

### 前端测试（Vitest）
- `src/components/app/morse-utils.test.ts` — Morse 前端测试，测试工具函数
- `src/components/app/timer-utils.test.ts` — 计时\计数器前端测试，测试设置转层、进度计算与倒计时格式化
- `src/components/app/delta-login-utils.test.ts` — Delta 登录工具测试（Tauri invoke 参数包装、sessionKey 提取等）
- `src/components/app/delta-game-data-loader.test.ts` — 游戏数据分批加载测试（主批次、详情批次、陈旧响应丢弃、重试入口）
- `src/components/app/delta-utils.test.ts` — Delta 工具函数测试（令牌状态判定、账号能力、显示名截断等）
- `src/components/app/delta-types.test.ts` — Delta 类型常量测试（AccountKind camelCase 一致性守卫、能力映射完备性、登录流程映射等）
- Vitest coverage 配置只包含 `morse-utils.ts`

### Rust 测试（cargo test）
- `src-tauri/src/morse/mod.rs` — 测试 history push limit
- `src-tauri/src/morse/types.rs` — 测试 settings 默认值
- `src-tauri/src/morse/decoder.rs` — 测试解码器和未知 pattern 错误
- `src-tauri/src/morse/recognition.rs` — 测试高 DPI/多显示器区域坐标转换
- `src-tauri/src/timer/types.rs` — 测试计时器默认值
- `src-tauri/src/timer/settings.rs` — 测试计时器设置读写与反序列化错误
- `src-tauri/src/hotkey_types.rs` / `src-tauri/src/hotkeys.rs` — 测试热键解析、共享监听分发、hold 切换和跨 scope 冲突检测
- `src-tauri/src/timer/mod.rs` — 测试透明窗口尺寸计算与设置校验
- `src-tauri/src/delta/commands.rs` — 测试 DTO 反序列化
- `src-tauri/src/delta/storage/repo.rs` — 测试 upsert、list、delete、明文凭据迁移与加密读写（使用 tempfile）
- `src-tauri/src/delta/utils/game.rs` — 测试 caliber 标准化、bind-role 解析、enrich_gun_detail
- `src-tauri/src/delta/services/game.rs` — 核心测试文件，使用 `mockito` mock HTTP 服务端，覆盖所有 game API 端点

### GameService 测试模式
`src-tauri/src/delta/services/game.rs` 中的测试使用：
- `mockito::Server` 启动 mock HTTP 服务器
- `make_service()` 辅助函数构造带有 mock URL 的 `GameService`
- `ide_form()` 辅助函数按 IDE 网关格式构造 expected request body
- 每个测试验证：请求参数匹配 + 响应解析正确 + mock assertion

## GitHub workflow

- 本项目代码托管、Issue 跟踪、Tag 与 Release 发布以 GitHub 为准，当前远端应为 `https://github.com/IZRINO/delta-auto-tools`；不得再恢复或使用旧远程地址。
- GitHub 初次迁移或远程异常时，按顺序处理：确认 `gh auth status` 已登录 → 必要时用 `gh repo create IZRINO/delta-auto-tools --public --description "三角洲行动工具：Tauri 2 + React + Rust 桌面工具"` 创建仓库 → 用 `git remote set-url origin https://github.com/IZRINO/delta-auto-tools` 切换远程 → `git push -u origin master` 推送主分支。
- 更新版本号时必须同步更新 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json`；如 `src-tauri/Cargo.lock` 中的本包版本随 Cargo 解析更新，也应一并提交。
- 每次更新版本号后必须运行 `bun run tauri build` 完成桌面打包；打包成功后检查以下两个产物存在：`src-tauri/target/release/bundle/msi/delta-auto-tools_<version>_x64_en-US.msi` 与 `src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe`。
- 每次版本发布提交不能只写 `发布 v<version>`。发布 commit subject 使用 `发布 v<version>`，正文必须跟上本次变更摘要与验证结果，至少包含 `变更：` 和 `验证：` 两段；变更项从本次实际 diff / Release notes 提炼，禁止写成泛泛的“更新版本”。推荐格式：`git commit -m "发布 v<version>" -m "变更：\n- ...\n- ...\n\n验证：\n- bun run test\n- bun run tauri build"`。
- 每次版本发布必须创建并推送对应 `v<version>` Tag：`git tag -a v<version> -m "发布 v<version>"`，然后 `git push origin v<version>`。
- 每次版本发布必须创建 GitHub Release，并通过 `gh release create v<version> <msi路径> <exe路径> --repo IZRINO/delta-auto-tools --target master --title "delta-auto-tools <version>" --notes <发布说明>` 上传 MSI 与 NSIS 安装包；Release 已存在时使用 `gh release upload v<version> <msi路径> <exe路径> --repo IZRINO/delta-auto-tools --clobber` 覆盖上传。
- Release 发布后必须用 `gh release view v<version> --repo IZRINO/delta-auto-tools --json tagName,url,isDraft,isPrerelease,assets` 验证 Release 非 draft、非 prerelease，且两个安装包状态均为 `uploaded`。
- 处理 GitHub Issues 时，先回复处理结论、变更范围、验证方式和需要用户确认的功能点；**不要在回复后直接关闭 Issue**。
- Issue 回复后应保持开放状态，等待提报者或维护者确认功能行为符合预期；只有收到明确确认、重复问题已被合并追踪，或维护者明确判定无需继续处理时，才关闭 Issue。
- 如果已提交修复但仍未确认，应在 Issue 中说明对应提交/版本与验证入口，并标记为待确认，而不是关闭。

## Repo-specific cautions

- 使用 **Bun**，不要切换到 npm / pnpm / yarn
- 不要虚构仓库中不存在的 lint/test/CI 命令
- `README.md`、`AGENTS.md` 和 `CLAUDE.md` 需要随重大功能变更一起更新
- 仓库当前允许提交项目级 skills 目录：`.agents/skills/` 与 `.claude/skills/`；不要把它们误当成本地垃圾直接删除
- 项目级 OMP 扩展位于 `.omp/extensions/<name>/`；扩展子包自带 `.gitignore` 排除 `node_modules` 与 `bun.lock`，不要把这些误当成本地垃圾直接删除；扩展自身的 devDep 仅装在子包内，不要污染根 `package.json`
- 忽略本地或生成产物：`node_modules`、`dist`、`src-tauri/target`、`.claude/worktrees/`、`.claude/settings.local.json`、`temp/`、`test-results/`
- 不存在 `tailwind.config.js` — Tailwind v4 通过 CSS `@import "tailwindcss"` 配置
- `GameService` 的弹药/配件配置已内联在 `game_config.rs`，**不需要**仓库根目录下的 `ammo.php` / `accessory.php`
- 前端仅对 `src/components/app/morse-utils.ts` 有测试覆盖
- 新增 Tauri command 必须同时注册到 `src-tauri/src/lib.rs` 的 `generate_handler![]` 和 `src-tauri/capabilities/default.json`
- 仓库根目录的 `ammo.json` 和 `accessory.json`（`resources/` 下）为空数组，未被实际使用

## OMP 扩展

- OMP 扩展放在项目级 `.omp/extensions/<name>/`，包形式（`package.json#omp.extensions` 入口声明 + `index.ts`），启动时由 OMP native provider 自动发现
- 每个扩展子包独立 `node_modules`，只装自己需要的 devDep（例如 `@oh-my-pi/pi-coding-agent`、`@oh-my-pi/pi-tui`、`@types/bun` 用于类型校验与扩展子包测试），不要把扩展依赖写到根 `package.json`
- 加载期禁止调用运行时方法（`pi.sendMessage` / `pi.sendUserMessage` 等会抛 `ExtensionRuntimeNotInitializedError`）
- `pi.sendUserMessage` 不支持 `triggerTurn`；需要自动触发后续 Agent turn 时，使用 `pi.sendMessage({ customType, content, display, attribution }, { deliverAs: "nextTurn", triggerTurn: true })` 注入 custom message，不要用 `sendUserMessage(..., { deliverAs: "followUp" })` 假装会自动执行
- 新增 OMP 扩展属于"新项目级 skills / agents 目录约定"，需在本节登记扩展名、入口、命令/工具清单与默认行为

### `gh-issues`（`.omp/extensions/gh-issues/`）

- 依赖：`gh` CLI（需已 `gh auth login`），`@oh-my-pi/pi-coding-agent`、`@oh-my-pi/pi-tui` 和 `@types/bun` devDep 用于类型校验与扩展子包测试
- 命令：
  - `/gh-issues [repo] [interval-min] [prompt]` — 启动长期轮询器；仓库默认 `IZRINO/delta-auto-tools`、间隔默认 60 分钟、prompt 空时仅通知；再次执行会先停止并 abort 旧轮询器，再启动新配置
  - `/gh-issues-stop` — 停止当前轮询器
- 行为：通过 `pi.exec` 调 `gh issue list --json ...`，按 issue `number` 去重；无 prompt 时发现新 issue 用 `ctx.ui.notify` 通知前 5 条，新轮询无新增时也通知“本轮检查完成”以证明周期执行；每次启动、输出和状态栏刷新都会显示“上次输出”与“下次运行”时间；有 prompt 时把新 issue 摘要 + 用户提示词作为 `gh-issues-prompt` custom message 通过 `deliverAs: "nextTurn"` + `triggerTurn: true` 注入并自动触发 Agent 执行，不再要求用户二次按 Enter；命令 handler 会保持未完成以维持主 OMP working 状态，`ctx.ui.setWorkingMessage` / `setStatus` 显示长期运行提示；按 `app.interrupt` 绑定键（默认 Esc，兼容 Kitty/modifyOtherKeys 终端序列与用户重映射）或执行 `/gh-issues-stop` 会停止 timer、abort 运行中的 `gh` 命令并 resolve handler；后续轮询使用 `setTimeout` 链式调度，上一轮完整结束后才开始下一轮间隔计时，不使用 `setInterval` 重叠执行；`session_shutdown` 时自动清理定时器并 abort 运行中的 `gh` 命令
- 状态：本会话内 in-memory，不跨会话持久化
- 类型：`Issue`、`IssueAuthor`、`IssueLabel`、`ParsedArgs` 为导出或核心接口；内部 `GhIssuesWatcher` 持有轮询状态、定时器、AbortController 与 UI 清理逻辑
- 验证：扩展子包提供 `bun test` 与 `bunx tsc --noEmit`

## If the project changes again

如果后续新增：
- 新的 Tauri commands
- 新的持久化结构
- 新的开发脚本
- 路由系统或新的应用壳层
- 新的项目级 skills / agents / OMP 扩展目录约定

请在同一轮改动里同步更新 `README.md` 与 `AGENTS.md`。
