# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

主要用户是作者本人式的《三角洲行动》重度玩家：管理多个 Delta 账号，靠特勤处自动轮次跑日常制作与子弹兑换，工具需长时间挂机无人值守。

同一份产物同时分发给朋友和小圈子，其中包含只用单个账号、只需要局内实时辅助（Morse 解码、计时器、计数器、连发器、识别触发）的轻量用户。因此新手上手成本是真实约束，不能假设用户读过文档或懂 OCR/模板匹配原理。

用户在 Windows 桌面上以管理员权限运行本工具，与游戏客户端同时在场。

## Product Purpose

把《三角洲行动》玩家反复手工执行的两类劳动收进一个本地桌面应用：

1. **无人值守的账号日常**——特勤处四制作台的收取重做、当天启用子弹的兑换，跨多账号按轮次自动完成。
2. **局内实时辅助**——摩斯密码解码并自动输入、倒计时、计数、按住连发、屏幕识别触发音频/按键/点击。

成功定义为：用户点一次「继续」后，逾期任务立即执行、未来任务到点执行，失败任务能被准确定位到具体制作台或具体子弹目标而不误伤其他任务；局内工具则以识别精度和不干扰游戏操作为准。

## Positioning

两条别人抄不走的机制同时成立：

**其一，特勤处全自动闭环。** 从登录、游戏内导航、四制作台批处理，到子弹目标的入口点击、A/D 重置后滚动、补齐、购买、兑换、双采样命中 `ammo.confirm` 参考图并以 `ammo.success` 模板差异确认完成——整链无人值守。链条上叠了联网利润筛选（KKRB 主源，仅主源整体失败才用 Moligod 隐藏 WebView 备用），回答的不是「怎么做」而是「这一轮什么值得做」。失败语义分层：账号级失败（制作、登录）记录后关游戏换下一账号，可定位的子弹失败只冻结当前目标、不阻断该账号后续任务。

**其二，一体化纯本地工具箱。** 7 个工具页 + 攻略站内嵌 WebView 共处一个原生应用，无云端依赖、无账号体系、无服务端。竞品要么是单点脚本，要么把凭据交给服务器。

## Operating Context

- **运行方式**：Windows 桌面应用，管理员权限启动，启动时一次 UAC；后续 WeGame 切号不重复提权。
- **主窗口**：`1280×800` 起，无路由库，`useState<ToolId>` 切页；左侧 Index Rail（≥1024px）与顶部 Tab Bar（<1024px）互斥呈现。
- **overlay 窗口**：透明显示窗与位置校准窗通过 `?mode=` 查询参数进入（`overlay`、`timer-display`、`timer-position`、`counter-display`、`counter-position`、`rapidfire-display`、`rapidfire-position`、`recognition-overlay`、`special-ops-calibration`、`special-ops-operation`），与游戏画面同屏共存。
- **默认暂停**：应用启动后特勤处保持暂停态。用户点「继续」并通过 preflight 才启用 scheduler。
- **多配置**：Profile 快照可切换、复制、删除、单配置导入导出；写入带 `settingsRevision` 防陈旧覆盖。
- **全局总开关**：关闭时暂停所有自动化与热键，页面显示 alert 横幅（攻略页除外）。
- **分发**：GitHub Releases。正式版带 minisign 签名 + `latest.json` 供 Tauri updater 拉取；beta 走 prerelease、不签名、不建独立通道。

## Capabilities and Constraints

**已实现工具页**（`src/App.tsx`）：计时器、计数器、连发器、攻略网站、识别触发、息屏（通用工具）；特勤处、摩斯密码解析（三角洲工具）；外加收藏夹与统一设置 Dialog（主题 / 配置 / 关于）。

**原生能力栈**：截图 `xcap`、输入模拟 `enigo`、全局键盘钩子 `willhook`、音频 `rodio`、OCR `Windows.Media.Ocr`（`windows` crate）、HTTP `reqwest`（rustls）。

**存储**：JSON 配置文件 + Profile 快照。**不使用 SQLite，不使用 DPAPI/keyring**（README 中相关描述已过期）。

**账号处理**：不保存、不输入 QQ 密码。账号识别依赖 WeGame「已记住账号」列表的 OCR + 剪贴板读取 QQ 号（`special_ops/remembered_account.rs`），双采样确认列表可见性。

**热键冲突策略**：`ConflictPolicy::Strict`（摩斯）与 `AllowHold`（计时器/计数器普通 scope 可与连发器/识别触发 hold scope 同键共存）。跨 scope 冲突默认拒绝。

**并发约束**：识别截图与 NCC 走 `spawn_blocking`，全局 `Semaphore(2)`；旧 watcher generation 不得继续触发效果。连发器 count 事件 ≤60Hz，计数器运行态经单 writer 线程 50ms latest-wins 合并落盘。

**权限分区**：`default.json`（main）、`overlays.json`（本地叠加窗）、`strategy.json`（remote WebView，授权为空）、`special-ops-profit.json`。禁止 `*:default` 宽权限或 `csp: null`。

**未决**：无。

## Brand Commitments

- 产品名：**三角洲行动工具**（窗口标题），英文标识 **Delta Auto Tools**（`tauri.conf.json` productName、README）。
- 标识符 `org.izrino.delta-auto-tools`，仓库 `IZRINO/delta-auto-tools`。
- 界面文案、错误信息、UI 文本一律中文，技术术语保留英文原名。
- 图标语义为准星（`RiCrosshair2Line`）；图标库统一 `@remixicon/react`。
- 内置主题 `olive-amber`、`valentine`、`arctic-blue`，默认 `valentine`；等宽字体 JetBrains Mono Variable。
- 视觉层已迁移至 daisyUI 5 + Tailwind v4 token，全局 `--radius: 0`；Radix/Base UI 仅保留 headless 行为层。

## Evidence on Hand

- `README.md`、`CLAUDE.md`、`AGENTS.md`、`CONTEXT.md`（领域词汇表，其中账号管理/游戏数据/工具箱三页描述已过期）。
- `droid-wiki/` 36 页自维护结构化文档：`overview/`、`features/`（含 special-ops.md）、`systems/`、`how-to-contribute/`、`reference/`、`deployment.md`。
- `docs/adr/`、`docs/agents/`（issue-tracker、triage-labels、domain）。
- `logo.png`、`src-tauri/icons/`。
- 质量门禁实测存在：`bun run check`（TypeScript → Vitest → coverage → cargo fmt → clippy `-D warnings` → cargo test）。前端 coverage 阈值 lines 25.49% / statements 25.67% / functions 22.31% / branches 25.76%，`autosave-queue`、`tauri-listener`、`recognition-card-reducer` 单文件 lines 阈值 90%。
- 当前版本 `0.19.0`，三处版本号需同步（`package.json`、`Cargo.toml`、`tauri.conf.json`）。
- **不存在**：用户评价、案例、装机量、压测数据、竞品对比、商业化定价。未来工作不得虚构。

## Product Principles

1. **本地优先、不上云。** 无服务端、无账号体系，凭据与配置只留在本机，绝不上传账号数据，绝不保存或输入 QQ 密码。
2. **overlay 不干扰游戏。** 透明窗口必须无边框、置顶、点击穿透、背景透明，不遮挡视野、不抢输入焦点。
3. **自动化默认停在安全侧。** 启动即暂停，preflight 通过才跑；失败精确定位到任务而非粗暴阻断账号；系统失败与紧急停止保留游戏现场供人工检查。
4. **无人值守要能被复盘。** 24 小时时间轴、逐项即时保存、单项人工判定命令，让用户离开数小时后仍能判断发生了什么。
5. **重度能力不能压垮新手。** 同一界面要同时服务多账号轮次编排和只想用一个连发器的用户。

## Accessibility & Inclusion

- 保留 Radix/Base UI headless 组件的焦点管理、键盘导航、Portal 与无障碍行为，视觉层替换不得牺牲这些能力。
- 主窗口已有「跳到主内容」skip link（`src/App.tsx:446`）与 `#app-content` 可聚焦主区域，需保持。
- 装饰性图标一律 `aria-hidden="true"`，控件需 `aria-label`（如全局总开关）。
- 用户在游戏中分心状态下操作，状态必须同时由文字与颜色表达，不可仅靠颜色区分（令牌状态、失败态、开关态）。
