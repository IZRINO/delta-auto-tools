# 全局规则

本文件为 Factory Droid 全局 AGENTS.md，适用于所有项目。项目级 `AGENTS.md` 可追加或收紧规则，但不得放宽本文件约束。

## 核心定位

顶级专家。准确 > 认同。直白、好辩。不客套不吹捧。先抛反论。无新证据不退让。

## 事实标注（TAG）

每条事实声明必须标注来源标签，无标注的疾病/法规/引用/命名实体禁止出现：

- `[KNOWN]` 训练事实
- `[COMPUTED]` 计算
- `[INFERRED]` 推理
- `[COMMON]` 领域常识
- `[FRAME]` 符号框架（内部自洽 ≠ 现实映射）
- `[GUESS]` 无依据

### FRAME→REALITY 禁令

禁止将符号框架（占星、类型学等）翻译为现实世界断言（医学/法律/金融）而不标注翻译；结论留在源框架内。

### 事后检验

框架若不知结果就无法预测 → 标注 `[INFERRED, post-hoc]`，容纳性而非预测性。

## 置信度（CONFIDENCE）

- `HIGH` ≥80%
- `MED` 50–80%
- `LOW` 20–50%
- `VERY LOW` <20%
- `UNKNOWN`

`[FRAME]` 现实映射和 `[GUESS]` 上限为 `LOW`。

## 不知原则

不确定时首行写「I don't know.」。不掩盖、不捏造、不埋藏。

## 反谄媚（ANTI-SYCOPHANCY）

警惕信号：异常优雅、单一模式解释一切、无证据就同意、未授权权威给细节。

应对：砍细节、加 `[GUESS]`、或「I don't know.」。

## 引用与修正

禁止捏造引用。持立场因一致性时公开修正。末尾附 `[RULES I BROKE]: which, where, why.`。

## 豁免

执行类任务（写/改/调试代码、跑命令、文件操作）豁免 TAG 与 CONFIDENCE 标注；仅在事实陈述、诊断结论、外部建议时使用。

代码改动后，若仓库存在测试/lint/编译命令，必须运行验证后再声明完成；失败照报，不掩盖。

## 语言风格（Language）

应尽量使用中文输出，使用地道计算机专业术语（内存泄漏、竞态条件、死锁、时间/空间复杂度、尾递归优化），禁止大白话口语。严谨、尖锐、直击痛点，删掉所有诸如「这段代码写得很好」的客套话。

### 英文输出风格

Drop: articles (a/an/the), filler (just/really/basically/actually/simply), pleasantries (sure/certainly/of course/happy to), hedging. Fragments OK. Short synonyms (big not extensive, fix not "implement a solution for"). Abbreviate common terms (DB/auth/config/req/res/fn/impl). Strip conjunctions. Use arrows for causality (X -> Y). One word when one word enough.

Technical terms stay exact. Code blocks unchanged. Errors quoted exact.

Pattern: `[thing] [action] [reason]. [next step].`

Not: "Sure! I'd be happy to help you with that. The issue you're experiencing is likely caused by..."
Yes: "Bug in auth middleware. Token expiry check use `<` not `<=`. Fix:"

#### 示例

**"Why React component re-render?"**

> Inline obj prop -> new ref -> re-render. `useMemo`.

**"Explain database connection pooling."**

> Pool = reuse DB conn. Skip handshake -> fast under load.

### 中文输出风格

砍虚词（的/了/着/过/其实/basically/just）、客套（当然/没问题/很高兴/不难看出）、冗余修饰（非常/十分/极其/大幅度）。短词优先（改→非重构，修→非修复，删→非移除）。缩写常见术语（DB/鉴权/配置/请求/响应/函数/实现）。箭头表因果（X → Y）。技术术语保留英文原名，不硬译（mutex 不写"互斥锁"，render 不写"渲染"，callback 不写"回调"——除非是中文已广泛接受的如死锁、竞态条件）。一句够用一句。代码块不变。错误原样引用。

模式：`[对象] [动作] [原因]。[下一步]。`

#### 示例

**"为什么 React 组件重渲染？"**

> 内联 obj prop → 新引用 → 重渲染。`useMemo`。

**"解释数据库连接池。"**

> 连接池 = 复用 DB 连接。跳过握手 → 高并发下快速响应。

## Auto-Clarity Exception

Drop caveman temporarily for: security warnings, irreversible action confirmations, multi-step sequences where fragment order risks misread, user asks to clarify or repeats question. Resume caveman after clear part done.

Example — destructive op:

> **Warning:** This will permanently delete all rows in the `users` table and cannot be undone.
>
> ```sql
> DROP TABLE users;
> ```
>
> Caveman resume. Verify backup exist first.

<!-- CODEGRAPH_START -->
## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely — indexing is the user's decision.

After modifying code, run `codegraph sync` to refresh the index — no need to sync after every small change, just before larger explorations.
<!-- CODEGRAPH_END -->

## 源码优先

涉及具体代码的问题，先用工具读实际源码再下结论。不凭训练记忆断言当前仓库内代码的行为、签名或实现。

# Repository Guidelines

## Project Overview

`special_ops` 新 schema 以 `defaultBusinessConfig` 保存默认制作台与子弹业务配置；制作台业务项包含 `recipeNote`，子弹业务目标包含稳定 ID、备注、普通/赛季类型、指定点击点、A/D 重置后向下滚动次数和顺序。兼容字段 `scrollDirection` 在 normalize 时固定归一化为 `down`，不参与 UI 或 runtime。账号通过 `independentSettingsEnabled` 选择继承或独立配置，独立配置可覆盖四台 `recipePoints` 与子弹目标，制作计时与当天子弹状态始终按账号保存。业务点复用现有 calibration overlay，以 `business.ammo.<targetId>` 和可选账号上下文区分默认/独立目标；账号级配方点不覆盖全局点击点，制作 runtime 优先读取账号级点。`AccountFailure` 以互斥的 `stationKind` / `ammoTargetId` 定位失败业务，`AmmoTarget.lastFailure` 保存目标级人工失败。`special_ops_confirm_station_state` 与 `special_ops_confirm_ammo_state` 只校正当前任务；`special_ops_confirm_account_manual_check` 的 gate `account_allows_manual_check` 是 `!matches!(status, Ready)`，必须覆盖全部非 `Ready` 状态（含紧急停止后的 `Uncertain` 与仓库不足升级的 `Isolated`）：只列举 `NeedsManualLogin` / `LoginFailed` / `ManualCheckRequired` 会与前端按钮的显示条件失配 -> 点击只拿到一句被顶部横幅吞掉的报错 -> 表现为「账号页点已人工检查没反应，只有一键恢复能救」。该命令不改子弹成功日与 retry，但必须按存量 `finishesAtMs` 还原 `Uncertain` 制作台，并清掉账号级 `lastFailure.ammoTargetId` 指名的那个子弹目标的 `lastFailure`（其他目标不动），否则账号回 `Ready` 却永远少跑一种子弹。账号级动作的失败原因必须渲染在按钮旁的 inline alert，页头横幅在账号列表滚动位置上不可见；`special_ops_confirm_account_station_states` 在 revision 临界区内原子校正**选中**的制作台与子弹状态（支持部分选中：至少一项，未选中项保持原状），`special_ops_restore_account_state` 按 `Option<String>` 一键恢复单账号或全部账号异常（账号回 `Ready`、`Uncertain` 制作台按存量计时还原、失败子弹解冻、清当天 `lastSuccessDay` 让目标回未兑换、限时商品 `Failed` 回 `Pending`），重复兑换由流程内资格与库存检查分支兜底；无可恢复项时报错，不产生空转 revision。初始化后的 `Ready` 可主动重新校正；制作失败单项校正后恢复 `Ready`，未校正子弹失败继续只冻结对应目标。任务行单项判定入口不能只看 `manualFailure`：`ManualCheckRequired` 与 `Uncertain` 制作台同样要出现入口，`NeedsManualLogin` / `LoginFailed` 才退回账号页；“正在制作”剩余时间留空或 0 表示继承存量计时。提交失败必须在对应行或完整人工校正 modal 内显示，禁止静默返回。

新增开发中模块 `special_ops`：配置保存到 `special_ops_settings.json`，并纳入 Profile 快照 `specialOps`；Profile 只保存校准参考图片路径，不复制图片本体。账号身份使用唯一纯数字 QQ；工具不保存或输入 QQ 密码，用户需提前在 WeGame 登录目标账号并勾选“记住密码”。账号内子弹运行态按当天成功日期、`retryDay` 和重试次数保存，不得随模板复制。账号下四制作台 UI 保存启用开关、小时/分钟及制作物品备注，兼容字段 `itemName` 不再由 UI 编辑。`ScheduleSnapshot.timelineTasks` 提供未来 24 小时制作、子弹、限时商品和交易行权威投影并携带 `manualFailure`；逾期任务保留原 `scheduledAtMs`，前端显示“0 分钟后”，以首任务为锚将严格小于 10 分钟的任务视觉合并且不修改执行时间。`timelineTasks` 按执行顺序返回，对齐 `build_round_plan_with_profit`：已到期任务在前并按 `account.order` 分桶（桶内按时间），未到期任务在后并按时间优先、账号顺序次之，同键再按制作台顺序与任务 ID 定序；未到期桶保留账号顺序作次键，避免同毫秒未来制作台被拆进多个 `AccountRoundTask`。限时商品任务携带 `limitedCycleId`；当前周期任务**检查完即出栏**（`build_timeline_tasks` 只接受 `pending`，任意终态立刻出栏），重跑只由人工触发。两个入口均在账号人工校正面板 `CorrectionLimitedSupply`：未确认 `highValue` 时显示”已查看高价值商品”按钮（调用 `special_ops_acknowledge_limited_supply`，只接受 `highValue` 状态），四种终态全显示”重新检查”按钮（调用 `special_ops_recheck_limited_supply`，复位 `LimitedSupplyAccountState` 到 `pending`，保留 `cycleId`）；任务栏不再渲染 `limitedOutcome` 结果行或确认按钮。两侧 gate 同源：任务栏出栏（`Pending` gate）与 planner 的 `limited_supply_due`（同样只认换周期或 `Pending`）保持一致，`recheck` 同时重开两侧。交易行任务携带 `marketCompletedCount` / `marketTargetCount` / `marketStatus`，任务栏必须渲染 `已购买 N/M · <状态>`；当天 `completed` / `windowClosed` 只在买满配置次数时出栏，上调购买次数即刻回栏，planner 的 `market_purchase_due` 只看 `completedCount < purchaseCount` 与之互补，但当天 `PriceRecognitionFailed` 任务栏保留、planner 跳过直到一键恢复。可定位失败在自身任务行显示单项判定；账号 `ManualCheckRequired` 或制作台 `Uncertain` 同样显示单项判定，只有登录环节失败退回账号页完整处理。`Uncertain` 制作台必须继续投影到时间轴，否则账号看着已恢复而任务永久消失。非 `Ready` 账号任务仍投影并携带状态；页面每分钟刷新时间和 bootstrap。

校准全局保存，通过 `special-ops-calibration-*` overlay 框选点击点与识别区域；静态 UI 使用用户参考图模板匹配并以 400ms 间隔双采样测试。制作试运行不再保存奖励页、共享制作中或四台制作列表就绪模板；先按三段全局 `0–60000ms` 固定等待执行 `craft.station.<station>`、Space、再次点击制作台和共享 `craft.confirmPinned`，再双采样 `craft.abort`。连续命中按新制作落盘（`startedAtMs = now`，`finishesAtMs = now + 配置时长`），与生产后命中中止同一条 `Started` 路径；批次再点共享 `craft.returnToStationGrid` 并等待 `game.stationGrid`，不发送 Esc；两个有效低分样本进入当前台独立 `craft.recipe.<station>` 物品选择点；不一致、截图错误、返回点击或确认失败保存实际步骤并标记账号与当前台 `Uncertain`。run 首个键鼠操作块显示 5→4→3→2→1；后续原本需要提示的块倒计时为 0 秒即不提示不等待直接执行，固定探测中原本不提示的后续输入继续不提示；物品选择及后续生产动作按同一规则提示。`runtime.mouseParking` 是 special_ops 独占全局点击点，各输入后先把鼠标移至该点再截图，不影响其他工具。

登录使用 5 个 template 目标 `wegame.loginMode/loginFormReady/login/gameEntry/launch`、`wegame.accountDropdown` 点击点、`wegame.accountList` OCR 区域与 `wegame.selectedAccount` 双击复制区域。runtime 强制重启 WeGame，从列表顶部逐行选择并通过 Unicode 剪贴板精确复核 QQ；扫描失败直接标记 `NeedsManualLogin`，不重启 WeGame。WeGame 与游戏 exe 由用户选择；runtime 按 canonical 完整路径结束目标实例，不递归结束进程树。单实例 runtime 支持登录、游戏内导航、单制作台试运行、当前账号四制作台批处理、`special_ops_start_ammo_trial` 单账号真实子弹兑换及 `special_ops_start_due_round` 多账号自动轮次。

军需处共享入口先识别点击 `ammo.department`，随后按独立 `0–60000ms` 等待直接点击 `ammo.supply` 与 `ammo.enterSupply`；子弹分支再识别点击 `ammo.tacticalDepartment`，限时商品分支识别点击 `ammo.researchDepartment`，同账号两类任务共享一次入口。兼容字段 `limitedSupply.researchDelayMs` 继续反序列化，但不参与 UI、preflight 或 runtime。普通目标全部完成后才在存在赛季目标时点击一次 `ammo.seasonal`，不再保存或校验 `ammo.list` / `ammo.seasonalList`。每个目标定位先在同一全局输入锁内按 `A`、等待 100ms、按 `D`、等待 100ms；再向下滚配置次数，滚轮事件间隔 100ms，滚动结束无论次数是否为 0 均等待 1000ms 后点击。run 首个键鼠操作块显示 5→4→3→2→1，后续原本需要提示的块倒计时为 0 秒即不提示不等待直接执行；识别、固定等待和持久化不倒计时。点击 `ammo.exchange` 后必须双采样命中全局用户参考图模板 `ammo.confirm` 并点击区域中心，再双采样 `ammo.success`；`ammo.success` 通过模板差异判断兑换完成，不读取颜色。确认模板超时、确认后完成状态未命中或购买重试耗尽 → 结束当前账号本轮并把失败写入当前 `AmmoTarget.lastFailure`，账号保持 `Ready`；窗口、截图、输入和持久化故障保持系统级失败。普通兑换 retry 只增加当天次数，不产生人工失败；成功逐项立即保存。

round 启动时通过 `build_schedule()` 冻结所有启用且账号状态为 `Ready` 的到期制作台、当天可执行子弹、限时商品、交易行和未来 24 小时制作 lookahead。已到期业务分两组分桶：非交易行业务（制作台、子弹、限时商品）按账号 `order` 合并成每账号一个桶，交易行单独成桶且**全局排在所有非交易行桶之后**；桶内制作台按固定台序，子弹按业务配置顺序。账号 1 有特勤处 + 交易行、账号 2 只有特勤处 -> 账号 1 特勤处 → 账号 2 特勤处 → 账号 1 交易行。禁止在分桶后再按 `(account_order, scheduled_at_ms)` 整体重排：那会把交易行桶塞回它自己账号后面，交易行就不是最后了。没有其他账号的非交易行任务时两桶相邻且同账号，`can_chain_follow_up` 保持会话 -> 特勤处跑完直接进交易行，不重新登录。冻结缓存键必须是 `FrozenRoundAccountKey = (String, i64, bool)`，第三位为「是否交易行桶」：两桶的 `scheduledAtMs` 都分钟对齐（子弹取每日兑换时间、交易行取窗口起点），撞同一分钟时二元组相同 -> `collect()` 只留后插入的交易行桶 -> 非交易行桶拿到 `craft: []` / `ammo: None`，制作与子弹被静默跳过。未来制作任务继续按计划时间追加，不并入已到期桶，不得提前执行。未来下一任务已逾期或与当前任务计划时间差 `<=10` 分钟时继续本轮：同账号保持游戏在线并只执行会话内任务，其他账号关闭旧游戏后正常切号；下一任务尚未到期且间隔 `>10` 分钟时关闭游戏并结束本轮，交回 scheduler。`ammo.department` / `ammo.tacticalDepartment` / `ammo.researchDepartment` / `market.entry` 模板超时按账号级可重试问题处理：首次落 `lastFailure` 保持 `Ready` 并把该账号剩余任务插到剩余到期任务队尾（远期 lookahead 之前，禁止 append 到整队最后：last due 账号会去 `wait_until` 未来制作，游戏已关掉，WeGame 不关、重试永不启动）；第二次同一 `step`/`stationKind`/`ammoTargetId` 仍未进入正常流程标记 `ManualCheckRequired`。跨轮次计数靠落盘的 `lastFailure`，单账号不会无限重试。到期台探测先命中中止、未进入购买/生产时按新制作落盘：`startedAtMs = now`，`finishesAtMs = now + 配置时长`，与生产后命中中止同一条 `Started` 路径。窗口、截图、输入与持久化故障保持系统级失败。逐台制作与逐种子弹成功立即持久化；账号异常清除该账号本轮剩余任务并继续其他账号；系统级失败全面暂停。交易行窗口内制作台优先：进 `market.entry` 之前若已有到期制作立即让位；循环内只让位给入口后新到期的制作（`latest_due_craft_at_ms()` 取 `max` 与基线比较），避免把仍排在交易行后面的队列任务当成让位理由导致每买一件就换号。让位时点击 `game.specialOps` 从交易行/大厅进四制作台；同账号保持会话，跨账号才关游戏切号。价格 OCR/`market.price` 截图失败按未识别页累计。连续三页失败把该账号交易行插到剩余到期任务队尾重试一次（插在远期 lookahead 之前）；队尾补偿仍失败则写入 `priceRetryAtMs = now + 1h` 并标记 `PriceRecognitionFailed` 供展示，planner 只在冷却结束且窗口仍开时再次到期，新一轮仍走「三页失败→队尾重试」。循环直到交易行窗口结束；窗口关闭走既有 `WindowClosed`。禁止升级成系统暂停。一键恢复清冷却并放回 `Pending`。`PauseRequested` 持久化后关闭游戏；scheduler 系统暂停、`SystemFailure` 和 `EmergencyStopped` 保留游戏现场。关闭游戏预算 `ROUND_CLOSE_GAME_TIMEOUT = 45s`，不再沿用比登录 `StopGame` 还紧的 10 秒。导航超时后、账号失败后与会话结束这三处切换关闭失败只记 warn 继续本轮，不得全局暂停：登录头两步 `StopGame` / `StopWeGame` 无条件关掉游戏与 WeGame；关进程失败再试一次，两次都失败继续登录，不得整轮暂停（游戏卡在「正在退出」时 ACE 概率杀不掉，暂停代价高于残留）。按 canonical 路径直接 `TerminateProcess`，最多两轮后看进程还在不在；仍在则等 1 分钟再两轮，循环到杀掉或紧急停止/暂停，WeGame 两轮还在则聚焦已有窗口继续登录，不开第二份。上一个号结束后等 15 秒再跑下一个号。查询被拒仍按文件名杀。强杀前启用 `SeDebugPrivilege`，失败忽略。只有 `PauseRequested` 路径关闭失败仍报告 `round.closeGame`，且暂停原因已先落盘，进程错误文本不进 `pausedReason`。

后台单 worker scheduler 默认未 armed，应用启动强制暂停；用户点击继续并成功持久化后立即 armed，逾期任务立即执行，未来任务到点执行。前端不提供“开始当前到期轮次”手动入口；scheduler 与 round 仍共用 `build_schedule()`，统一调度到期制作与当天子弹。每日兑换时间前 5 分钟内到期的制作延迟至兑换时间合并执行，当天成功或重试耗尽的子弹不再加入。设置保存、人工校正和 round 完成后通过 `Notify` 唤醒。30 秒健康检查发现定时器晚醒超过 60 秒时持久化暂停、请求 active round 停止并聚焦主窗口；晚醒判定只用 poll 成功返回的 `nowMs`，poll 失败不算时间跳变，交给下一轮循环写真实原因。系统暂停不关闭游戏，继续后重新规划且不复用旧会话。scheduler 启动到期轮次失败经 `is_transient_round_launch_error` 分流：poll 与 `freeze_round_run` 过滤条件不一致导致的空计划、暂停中、总开关关闭、试运行未清理、revision 陈旧只记 warn 并 `RetryAfter(30s)`，其余错误才全局暂停。所有自动暂停把原因写入 `SpecialOpsSettings.pausedReason`（可选字段，缺失按 `null`），页头以 warning alert 展示；`special_ops_set_paused` 手动切换两个方向都清空该字段。`special_ops_save_settings` 强制沿用进程内 `paused` / `pausedReason`，前端草稿不得回滚运行态。用户主动暂停仍在当前任务完成后关闭游戏并停止切号。`LoginRunSnapshot.runKind = round` 时携带 `roundProgress`，等待期间显示保持会话或切号状态。全局关闭只 disarm scheduler，应用关闭才 shutdown worker。

联网利润筛选以 `profitFilter` 独立保存全局开关、截止时间、稳定规则 ID、KKRB/Moligod 精确名称、最低总利润、当天最近审计及 `cutoffState`；`AmmoBusinessTarget.profitRuleId` 只引用规则 ID。常规运行期资格、query generation、cadence、active round targets 不持久化，重启不得复用历史审计。每天每日兑换时间至利润截止时间内按立即、5 分钟、5 分钟、50 分钟节奏查询；KKRB 正常响应时只使用 KKRB，只有整体失败才使用无 IPC 权限的 Moligod 隐藏 WebView。截止时冻结当日剩余账号+子弹目标，以固定最低利润 10,000 查询；低利润直接轮空，目标缺失、来源失败或利润无效在 5 分钟后只补查一次，仍失败则轮空。截止达标 gate 按账号+目标 ID，不按规则 ID；截止后新增目标不加入当日 `cutoffState`。round 启动先消费同 generation 的达标目标；运行资源启动失败必须仅回滚该 generation，防止遗留 `ActiveRound`，且必须把 `consume_for_round` 暂存的达标资格原样放回、phase 转 `WaitingNextQuery`，不得清空——瞬时启动失败 1 秒后即重试，资格被烧掉会让重试 gate 变空并滤掉全部子弹。只有 `end_active_round` 才清空资格。所有返回 `SpecialOpsBootstrap` 的保存、人工校正和暂停路径均必须包含当前 `ProfitRuntimeSnapshot`，并在成功写盘后递增 revision。

达标必须立刻兑换，不等截止时间：`build_round_plan_with_profit` 必须调用带快照的 `build_schedule_with_profit_runtime(settings, createdAtMs, gate, profitSnapshot)`，`profit_gate_for_round` 取到的 `ProfitRuntimeSnapshot` 要经 `freeze_round_run` 透传进去。任务栏投影只在拿到 `qualifiedRuleIds` 时才把达标子弹的计划时间提到「现在」；无快照会退到 `WaitingQuery` 分支排到 `cutoffAtMs` -> planner 的 `is_due` 恒 false -> 空计划 -> `EMPTY_ROUND_PLAN_ERROR`，而该错误在 `is_transient_round_launch_error` 里只 warn 并 `RetryAfter(30s)` -> 表现为静默：poll 带快照说「该启动」，freeze 不带快照算出空计划，每 30 秒对拆一次直到截止时间才兑换。

**Delta Auto Tools** — Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust 桌面工具，面向《三角洲行动》玩家。原生能力模块：Morse 摩斯识别、计时器、计数器、连发器、识别触发、息屏、攻略网站工作台。

开发环境：Windows，仓库路径 `D:/code/ai/sjz/delta-auto-tools`，所有命令在 Windows + Bun 下测试通过。

## Wiki Documentation (droid-wiki/)

`droid-wiki/` 是项目自维护的结构化文档（36 个页面），覆盖架构、各功能模块、底层系统、开发流程、约定和发布。**当不确定某模块如何工作、某约定是什么、某流程怎么走时，优先查阅 `droid-wiki/` 下的对应文档，而不是凭记忆猜测。**

文档结构：

| 目录 | 内容 |
|------|------|
| `overview/` | 项目概览、系统架构、快速开始、术语表 |
| `features/` | 各功能模块详解（morse / timer / counter / rapidfire / recognition / strategy / about） |
| `systems/` | 底层系统（tool-base / sync-tool / hotkeys / key-suppressor / overlay-windows / global-state / logging / theme-engine / profile-system） |
| `how-to-contribute/` | 开发流程、测试、调试、模式与约定、工具链 |
| `reference/` | 配置项与依赖参考 |
| `deployment.md` | 部署与发布流程 |

入口：`droid-wiki/overview/index.md`。`droid-wiki/.wiki-meta.json` 记录生成时间、commit、页面清单。

> Wiki 与 codegraph 互补：wiki 适合理解「为什么这样设计」和「整体流程」，codegraph 适合查「符号定义在哪、谁调用了谁」。

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

文档与代码不一致时以当前实现为准。

## Commands

```bash
bun install                    # 安装前端依赖
bun run dev                    # Vite 前端开发服务器（端口 1420，strictPort）
bun run tauri dev              # 完整桌面开发（需管理员 PowerShell）
bun run build                  # tsc && vite build
bun run test                   # Vitest 前端单元测试
bun run test:coverage          # 全量前端覆盖率与阈值检查
bun run check                  # Windows 全量质量门禁
cargo check --manifest-path src-tauri/Cargo.toml   # Rust 编译检查
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml    # Rust 单元测试
```

Windows 桌面版以管理员权限运行，启动时显示一次 UAC。`bun run tauri dev` 必须从管理员 PowerShell 执行；`bun run dev` 仅启动浏览器 UI，不要求管理员权限。

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

### 热键冲突规则

`ConflictPolicy` 枚举：`Strict`（禁止跨 scope 复用）和 `AllowHold`（允许 hold scope 与普通 scope 共存）。

- Timer / Counter 普通 scope 与 Rapidfire / Recognition hold scope 允许同键共存（双方均用 `AllowHold`）
- Recognition 使用混合 scope，普通与 hold 注册必须通过 `replace_mixed_scope` 原子替换；热键录制同时暂停两类注册
- Morse 与任何其他 scope 冲突必须拒绝（Morse 用 `Strict`）
- 录制热键时暂停对应 scope

### Overlay 透明窗口约束

计时器/计数器/连发器透明窗口必须无边框、透明、置顶、点击穿透。位置设置窗口可保留校准靶风格。overlay 必须保持透明背景。`?mode=` 查询参数分支进入 overlay/display/position 模式，不可用路由替代。

### Tauri command 注册

新增应用自定义 `#[tauri::command]` 必须注册到 `src-tauri/src/lib.rs` 的 `generate_handler![]`。当前仓库尚未把全部 app commands 迁移到 Tauri app ACL；禁止只为单个新 command 创建 `src-tauri/permissions/*.toml` 或向 capability 添加无 namespace permission，否则 Tauri 会启用局部 `__app-acl__` 并拒绝未列入 allow 的既有 commands。若要启用 app ACL，必须在独立任务中一次迁移并验证全部 commands。

新增 Tauri core/plugin 前端 API 调用时，必须在实际调用窗口对应 capability 增加精确 `allow-*` permission：main 用 `default.json`，overlay 用 `overlays.json`，remote Strategy WebView 用 `strategy.json`。Capability 不得重新使用 `core:default`、`opener:default`、`updater:default` 或 `process:default`。生产 CSP 不得恢复为 `null` 或加入远程通配源。

### 版本号同步

版本号必须同步更新 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。如 `Cargo.lock` 中本包版本随解析更新，也应一并提交。

### UI 约束

- 主窗口两条视觉线路并行，权威在 `DESIGN.md`：夜航黑标（默认生产壳）与战地控制台。设置切换，禁止同一屏混语法。配色主题三套只服务战地，不得用换色冒充黑标。游戏 overlay 不跟随黑标。
- UI 迁移方向：保留 Radix headless 交互能力。战地视觉层使用 daisyUI + Tailwind CSS + `src/App.css` token；禁止新增旧桌面/战术风格自定义 CSS 类。黑标以演示页 `bm-*` 语法为准，接到生产时对照演示。
- 基础组件位于 `src/components/ui/`，保留 Radix headless 行为能力，战地 class 必须优先映射到 daisyUI 组件语义
- 图标：战地与按钮内图标使用 `@remixicon/react`，Button 内必须设置 `data-icon="inline-start"` / `"inline-end"`。黑标 dock 图标必须自制 SVG，禁止用 remixicon 替换
- 本 mission 的 worker 编码前必须调用 `ponytail`
- 攻略网站页使用主窗口内嵌 `strategy-content` 子 WebView，不创建独立浏览器窗口，不使用 iframe/srcDoc，不得隐藏 Left Index Rail
- `TooltipProvider` 已在 `src/main.tsx` 根部提供
- 设计改动必须保持功能不变：不改 Tauri command 名、查询参数 mode、状态机、保存逻辑或原生窗口 label

## Architecture Quick Reference

详细实现请通过 codegraph 探索，以下为核心架构锚点：

**前端**：`index.html` → `src/main.tsx` → `src/App.tsx`。App.tsx 通过 `useState<ToolId>` 切换工具页；overlay/display/position 模式通过 `?mode=` 查询参数分支。每个工具页遵循 Bootstrap/Form 双状态 + autosave debounce 400ms + `LatestSaveQueue` latest-wins 模式；所有持久化工具配置的命令（含 position/region overlay commit）必须携带 Profile `settingsRevision`。

**后端**：`src-tauri/src/lib.rs` 的 `run()` 在 `setup` 中依次初始化各工具模块并 `app.manage()` 注册状态。工具模块共享 `ToolBase` 泛型基座（`ToolLogic` trait、`ToolState<T>`）；5 类工具保存与 Profile 切换必须通过全局 `SettingsCoordinator` 串行化并校验 revision。

**工具模块**（详见 codegraph）：
- `morse/` — 截屏→二值化→轮廓检测→摩斯解码→自动输入
- `timer/` — 多计时器，250ms tick，透明窗口
- `counter/` — 多计数器，运行态通过单 writer 线程 50ms latest-wins 合并持久化（counter_state.json）
- `rapidfire/` — 按住触发键连发，每 session 独立 OS worker 线程，count 事件共享 60Hz budget
- `recognition/` — 快捷键/多参考图区域监听/识色三种识别来源 + 音频/按键/点击效果；Hotkey 卡片支持 `once` / `whileHeld`，持续模式使用 per-card session 串行执行；截图/NCC 使用全局 `Semaphore(2)` 的 `spawn_blocking` 调度，watcher restart/stop 必须使旧 generation 失效；前端卡片更新、编辑器、框选分别位于 `recognition-card-reducer.ts`、`recognition-card-editor.tsx`、`recognition-overlay.tsx`
- `strategy/` — 前端管理主窗口 `strategy-content` WebView2 嵌入，无专用 Rust command
- `theme/` — 3 套 daisyUI 内置配色（默认 `valentine`）+ 自定义 + token override。只服务战地控制台，与黑标界面世界正交
- `profile/` — 多配置快照切换、复制、删除、单配置导入/导出
- `logging/` — 混合格式日志 + 按天轮转 + 链路追踪

**事件模式**：事件名格式 `{tool}://{event}`，后端在 `*/events.rs` 定义常量，前端通过 `src/lib/tauri-events.ts` 的 `EVENTS` 常量与显式泛型 `subscribeTauriEvent<PayloadType>(EVENTS.xxx, handler)` 订阅。Timer/Counter/Rapidfire 的 `state-changed` 只用于 settings/结构变化，`runs-changed` 只携带运行态；禁止高频路径发送完整 Bootstrap。

## Testing

- **前端**：Vitest，测试文件 `*.test.ts` 紧邻源文件。运行 `bun run test`
- **Rust**：`cargo test`，测试内联在模块中。运行 `cargo test --manifest-path src-tauri/Cargo.toml`
- **统一门禁**：`bun run check` 依次执行 TypeScript、Vitest、全量 coverage、Rust fmt、Clippy `-D warnings`、Rust tests；Windows CI 复用同一命令
- Vitest coverage 统计 `src/**/*.{ts,tsx}`；全局阈值为 lines 25.49%、statements 25.67%、functions 22.31%、branches 25.76%，新 queue/listener/reducer module 的 lines 阈值为 90%

## Commit Guidelines

- Issue 修复分支必须合并回 `master` 后再作为最终提交结果；不要把只存在于临时 `codex/*` 分支的提交当作完成。
- 本地合并完成并验证通过后，删除已合并的临时开发分支，保持分支列表干净。
- Commit message 使用中文
- 发布 commit：subject `发布 v<version>`，正文必须包含 `变更：` 段，变更项从实际 diff 提炼，禁止泛泛"更新版本"
- 常规 commit 示例：`feat(recognition): 识色探针支持多目标颜色`、`fix(counter): 全局开关关闭时保留计数器运行值`

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
2. 无签名构建：`bun run tauri build --config src-tauri/tauri.beta.conf.json`（关闭 updater artifact，不需要 `TAURI_SIGNING_PRIVATE_KEY`）
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
- `.agents/skills/`、`.claude/skills/`、`.factory/skills/` 三个目录镜像存储技能，内容逐字节一致，`skills-lock.json` 记录来源
- 忽略：`node_modules`、`dist`、`src-tauri/target`、`.claude/worktrees/`、`temp/`、`test-results/`
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
### 特勤处 Profile 持久化补充

`SpecialOpsSettings` 同时保存于 `special_ops_settings.json` 和 Profile 快照 `specialOps`；Profile 只保存校准参考图片路径，不复制图片本体。

特勤处限时商品颜色使用原生 `input[type=color]` 控件，可通过系统颜色面板吸管或 Hex 输入设置；不保存截图，9 个 `limited.color.1`–`limited.color.9` 校准区域只用于正式 `AnyPixel` 识别与双采样测试。交易行入口 `market.entry` 为模板识别与点击区域。购买材料按钮识别与点击分离：`craft.purchase` / `ammo.purchase` 只做识别，点击用 `craft.purchaseClick` / `ammo.purchaseClick` 两个 ClickPoint，并以对应识别区域作 `guardAnyOf` 守卫；映射集中在 `click_target_key()`，`wait_and_click` 与 `click_unverified` 两条路径都必须走该映射，冻结配置缺点击点时回落识别区域。制作购买点击改用 `click_unverified` 不再重复复核：到达该点击的三条路径都刚确认过同一张模板，内层 `verify` 只是白付一次 400ms 双采样。补齐/购买重试 3 次判定仓库空间不足的流程不变。

### 特勤处限时商品与交易行

`limitedSupply` 为全局配置，固定投影 12:00、20:00 限时商品检查；`LimitedSupplyAccountState` 保存当前周期结果与人工已检查标记。`marketPurchase` 为全局周期配置，固定投影 02:00–04:00 交易行购买；账号 `BusinessConfig.market` 保存启用状态、购买次数、商品备注、最高价与商品入口点击点，关闭独立设置时继承默认配置。交易行 `market.price` 只作为 OCR 区域，`market.confirm` 保存独立最终确认购买点击点。限时商品和交易行试运行正常结束后停放鼠标到 `runtime.mouseParking`，紧急停止不追加鼠标动作。新增 command：`special_ops_start_limited_supply_trial`、`special_ops_start_market_trial`、`special_ops_acknowledge_limited_supply`、`special_ops_recheck_limited_supply`、`special_ops_test_limited_supply_colors`、`special_ops_set_station_walkthrough`，均通过 `generate_handler![]` 注册，不单独创建 ACL。高价值结果额外保存 `matchedColorIndexes`（1 / 2 / 两者），人工校正与任务栏展示命中颜色 1、颜色 2 或都有。

息屏是通用工具独立模块，配置写 `privacy_screen_settings.json`（关闭快捷键、可选图片路径），不进 Profile。command：`privacy_screen_get_bootstrap`、`privacy_screen_save_settings`、`privacy_screen_show`、`privacy_screen_hide`。按钮打开、快捷键只在打开后关闭。原生 Win32 视觉遮罩（仿 UU 私密屏保，不是 WebView）必须跑在独立线程自带 `GetMessage` 循环：禁止在 WebView2/winit GUI 线程 `CreateWindow`/`RegisterHotKey`，否则主界面乱码崩溃。`WS_EX_TRANSPARENT`+`WS_EX_NOACTIVATE` 让键鼠/Alt+Tab 落到下面窗口且不抢焦点；`WS_EX_TOOLWINDOW`+定时钉 `HWND_TOPMOST` 只挡画面。识别默认走 xcap GDI；息屏打开时改走 WGC，遮罩必须 `WDA_EXCLUDEFROMCAPTURE` 才能从 WGC 构图里抠掉并透视到下方画面。现代 DWM 下 GDI BitBlt 会拍到遮罩（分层窗口也一样），NCC 对纯色图映射成约 50%。关闭快捷键用 `RegisterHotKey`（本程序聚焦时 willhook 会被 WebView2 吞掉）。默认全黑，可换本地图片。
