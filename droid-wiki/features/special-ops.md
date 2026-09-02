# 特勤处自动化（开发中）

`special_ops` 保存账号级制作台、子弹兑换和调度状态。每个账号包含 4 台制作台；同一账号的到期制作任务聚合处理。每日兑换时间按 `Asia/Shanghai` 的 `HH:mm` 解释。

`defaultBusinessConfig` 保存四制作台启用状态、时长、制作物品备注及有序子弹业务目标；子弹目标包含稳定 ID、备注、普通/赛季类型、指定点击点、A/D 重置后向下滚动次数和顺序。兼容字段 `scrollDirection` 在 normalize 时固定为 `down`，不参与 UI 或 runtime。账号默认继承；开启 `independentSettingsEnabled` 时复制当时默认配置并改用账号独立业务配置，关闭时二次确认并永久删除独立配置。`startedAtMs`、`finishesAtMs`、制作台状态、账号失败记录及当天子弹状态仍按账号保存，切换继承模式不得重算或删除这些运行态。旧 JSON 的制作台缺失备注时补空字符串，旧子弹 `name` 迁入 `note`、点击点补空；迁移幂等且保留运行态。

账号卡片继续提供”制作台与子弹人工校正”入口，支持**部分选中**：每个制作台和子弹目标各自有”不修改”选项，只有选中的项才会被提交，未选中项保持原状不变。至少选中一项才能点”核对制作台与子弹状态”进入二次确认；选中的制作台从”立即到期””正在制作””空闲”中选一，正在制作需填写 1 分钟至 168 小时的剩余时间。提交前显示第二次确认摘要（仅列出选中项），后端在 `SettingsCoordinator` revision 临界区内完成全量校验、账号初始化、一次磁盘写入和内存替换。初始化后的 `Ready` 账号允许主动覆盖实际制作与当天子弹状态；`NeedsManualLogin` / `LoginFailed` 不得通过业务状态校正恢复。提交期间按钮显示”正在保存”并禁止重复点击；浏览器预览、页面状态变化或后端拒绝均在 modal 内显示错误，禁止静默返回。

24 小时任务时间轴不再打开完整 modal。`AccountFailure.stationKind` 与 `AccountFailure.ammoTargetId` 互斥定位失败业务，`AmmoTarget.lastFailure` 保存目标级子弹人工失败，`TimelineTask.manualFailure` 把失败带到对应任务行。制作行提供“立即到期”“正在制作”“空闲中”，子弹行提供“已兑换”“未兑换”；每行独立保存和显示错误。`special_ops_confirm_station_state` 只校正失败制作台并恢复账号 `Ready`，`special_ops_confirm_ammo_state` 只清除对应目标失败；未处理子弹继续单独冻结。

单项判定入口不只看 `manualFailure`。`NavigationTimedOut` 只写 `ManualCheckRequired` 且 `stationKind` / `ammoTargetId` 均为空，因此制作台 `Uncertain` 或账号处于 `ManualCheckRequired` 时也必须给出任务行入口；只有 `NeedsManualLogin` / `LoginFailed` 才退回账号页处理，前端 `timelineTaskAllowsInlineCorrection` 与后端 `account_blocks_task_correction` 同步这条判定。选择“正在制作”时剩余时间预填异常前的存量计时，留空或填 0 表示继承 `finishesAtMs`；后端确实没有可继承值时才拒绝并要求填写 1 分钟至 168 小时。

全部非 `Ready` 账号显示“已人工检查”，`special_ops_confirm_account_manual_check` 恢复账号 `Ready` 并按存量 `finishesAtMs` 还原 `Uncertain` 制作台（未来时间→`Crafting`，已过→`Ready`，无计时→`Idle`），不改子弹成功日或 retry。只清账号状态会留下 `Uncertain` 制作台，它在调度与任务栏双重过滤下永久消失。

后端 gate `account_allows_manual_check` 必须与前端按钮的显示条件严格一致，判定式是 `!matches!(status, Ready)`：只列举 `NeedsManualLogin` / `LoginFailed` / `ManualCheckRequired` 会漏掉 `Uncertain` 与 `Isolated`（前者是紧急停止或未分类故障的 catch-all，后者由仓库空间不足升级而来），而前端对这两种状态同样渲染按钮 -> 点击拿到一句被顶部横幅吞掉的报错 -> 用户看到的是「任务栏说去账号页处理，账号页点已人工检查没反应，只有一键恢复能救」。同理，账号级动作（已人工检查 / 一键恢复）的失败原因必须落在按钮旁边的 inline alert 里：页头横幅在账号列表滚动位置上不可见，报错等于没报。

`Isolated` 由子弹目标失败升级而来，账号级 `last_failure.ammoTargetId` 指名出事的目标。清账号状态时必须同时清掉被指名目标的 `AmmoTarget.lastFailure`，其他目标不动：只恢复账号状态会让该目标继续被冻结过滤，账号回到 `Ready` 却永远少跑一种子弹。

账号卡片与账号区标题额外提供“一键恢复状态”。`special_ops_restore_account_state` 接收 `Option<String>`：传账号 ID 只恢复该账号，传 `null` 恢复全部异常账号。恢复内容为账号状态回 `Ready`、清 `lastFailure`、按存量计时还原 `Uncertain` 制作台、清子弹目标 `lastFailure` 与当天 retry 预算、清当天 `lastSuccessDay`、限时商品 `Failed` 回 `Pending`、当天交易行封锁状态回 `Pending`。当天成功标记一起清 -> 目标回未兑换、可再次调度；重复兑换由流程内资格与库存检查分支兜底。没有任何可恢复项时返回错误，不产生空转 revision。两个按钮常驻显示：无可恢复项时按钮 disabled 并在 title 说明原因，不再整块隐藏。

交易行只放回**当天**的 `Running` / `PriceRecognitionFailed` / `WindowClosed` 以及未到期的 `priceRetryAtMs`，`Completed` 与其他日期一律不动——购买次数已经花掉，放回会让同一天重复买满。必须放回的原因是任务栏对当天已买满次数的 `Completed | WindowClosed` 直接 `continue` 出栏，而 `build_round_plan_with_profit` 依赖这条任务栏任务，一次 `WindowClosed` 写入会让交易行当天永久消失 -> 一键恢复后点「继续」不再上线跑交易行。前端 `accountRestorable` 必须与后端 `changed` 判定完全一致（含这条交易行分支），否则按钮亮着点下去只拿到「没有需要恢复的异常状态」。

## 子弹兑换配置

业务配置保存有序子弹目标。默认配置供继承账号共用；开启独立设置后，账号可维护自己的目标列表。单项目包含备注、启用状态、普通/赛季限定、指定点击点、A/D 重置后向下滚动次数和顺序。UI 支持逐项新增、同组上移/下移、删除和复用校准 overlay 选择点击点；普通目标始终排在赛季目标前。每个目标定位先按 A、D，按键间隔 100ms，再向下滚配置次数；无论次数是否为 0 均等待 1000ms 后点击。每个 run 首个键鼠操作块发布 5→4→3→2→1，后续原本需要提示的块倒计时为 0 秒即不发布提示也不等待，原本不提示的固定等待和输入继续不提示。后续执行成功预检、补齐/购买/兑换、二次确认和完成确认。当天成功日期、`retryDay` 和重试次数始终属于账号运行态，复制默认配置时不得复制或重置。单账号真实兑换试运行和每日多账号兑换调度均已接入。

## 区域校准

校准结果全局共享，不随账号或 Profile 复制。UI 不要求用户填写环境名称、显示器、分辨率、DPI 或窗口模式，只维护一套当前校准结果。旧版本存在多套环境时，加载后保留当时选中的一套。

静态 UI 的 `recognitionRegion` 使用模板匹配，由用户选择一张本地参考图片，路径随校准目标保存到 `special_ops_settings.json`。登录试运行使用 8 项 WeGame 校准：`wegame.loginMode`、`wegame.loginFormReady`、`wegame.login`、`wegame.gameEntry`、`wegame.launch` 为 template 区域；`wegame.accountDropdown` 为账号列表展开点击点；`wegame.accountList` 为 Windows OCR 扫描区域；`wegame.selectedAccount` 为顶部已选账号双击复制区域。`runtime.mouseParking` 是 special_ops 独占的全局点击点，三类试运行都必须配置；每次业务点击、滚轮或按键完成后，执行器先把鼠标移至该点，下一步才截图，避免 hover 或鼠标遮挡污染识别区域。该点不影响 Timer、Rapidfire、Morse、Recognition 等其他工具。账号身份只取唯一纯数字 QQ。工具不保存或输入密码；用户需提前在 WeGame 登录账号并勾选“记住密码”。工具不读取或比对 WeGame/游戏 ID、UID，账号选择后只通过 Unicode 剪贴板精确复核 QQ。子弹目标不再使用共享 `ammo.target` 或已选名称 OCR；每个业务目标通过 `business.ammo.<targetId>` 与可选账号上下文写入自己的单点坐标。点击点不保存参考图。用户可替换或清除模板图片；游戏 UI 更新后应重新上传当前版本样本。区域坐标定义截图范围，参考图定义匹配目标，两者缺一时不得启动模板识别步骤。图片文件被移动或删除时路径失效，后续执行器必须报告缺失并暂停对应步骤。

每个识别区域提供“测试”按钮。模板测试对当前区域执行两次真实截图与 NCC 模板匹配，间隔 400ms，返回两次原始相似度；两次都达到默认阈值 `0.75` 才通过。OCR 测试对当前区域执行两次真实 Windows OCR，间隔 400ms，显示两次识别到的纯数字文本；两次均非空才显示通过。账号列表截图在送入 Windows OCR 前内部放大 3 倍，结果坐标再映射回原校准区域；OCR 数字词内部空白会被删除，减少小字号数字漏行或被分段后整体丢弃。只有双采样都稳定识别完整三行时才使用 OCR 行中心；仅识别一行或两行时改用三行固定中心，防止漏识物理首行后从第二行开始点击。OCR 测试仅用于诊断框选范围，不写入验证签名或配置。`game.*`、`craft.*`、`ammo.*` 测试先等待 3 秒、恢复并聚焦游戏、停放鼠标，再截图；`wegame.*` 维持当前窗口采样，避免错误聚焦游戏。模板验证签名绑定目标 key、区域、参考图 canonical 路径、文件长度与修改时间、阈值；重新框选、换图、清图、图片文件变化或阈值变化都会使验证失效。点击点和输入区域不显示识别测试。

框选行为沿用摩斯区域框选交互：在单个显示器打开全屏透明 overlay，主窗口保持存在；按住左键拖拽，松开后立即提交并关闭。区域过小时要求重新框选，Esc、右键或 Alt+F4 取消。overlay 30 秒未关闭时由 native 侧自动销毁，避免前端异常时持续占用键鼠。提交、取消、超时或窗口异常关闭后恢复主窗口焦点。点击动作执行时使用所选矩形中心。

创建入口必须使用 async Tauri command，避免在当前 WebView IPC callback 内同步创建第二个 WebView2 导致重入阻塞。校准窗口先按默认尺寸加载页面，页面完成后再切换为单显示器全屏；前端使用与摩斯框选一致的 Mouse Events 处理拖拽。

工作台通过 `special_ops_begin_calibration_selection` 打开框选窗口。提交调用 `special_ops_submit_calibration_selection`，取消调用 `special_ops_cancel_calibration_selection`。窗口 label 使用 `special-ops-calibration-*`，由 `overlays.json` 授权。

## 多账号制作台更改

独立登录流程，入口在默认账号配置。开关默认关；关着时下一账号热键不登记，轮换和试运行不受影响。打开条件：已暂停轮换、已录下一账号热键且不与紧急停止撞键、总开关开、全局自动化开、没有进行中的试运行、至少一名启用且 QQ 为纯数字的账号。

打开后按账号顺序登录到四制作台即结束本次运行，游戏留下等人改配方。按下一账号热键后按正常登录流程关游戏和 WeGame，再登下一启用账号。未启用或 QQ 非法的账号跳过。最后一号再按热键只关进程，开关保持打开。中途关开关不关游戏；再开从第一号重新登录。登录失败不自动跳号。紧急停止留游戏现场并关掉本功能。打开期间禁止继续轮换、禁止任何试运行。`special_ops_set_station_walkthrough` 是独立 command，`save_settings` 不得用草稿回滚该开关。`LoginRunKind` 增加 `stationWalkthrough`。

## 登录试运行 runtime

`special_ops_start_login_trial` 校验 settings revision、账号、两条 exe 路径及 8 个登录校准目标后冻结本次输入。单实例 `LoginRuntime` 在后台执行流程，IPC 立即返回 `LoginRunSnapshot`；active run 完成资源清理前拒绝下一次启动。登录试运行与多账号轮换均先发送完整 5→4→3→2→1 倒计时；倒计时期间不得关闭游戏、关闭 WeGame、启动 WeGame、截图、识图或模拟键鼠。倒计时结束后才结束旧进程并进入登录流程。后续原本需要提示的登录动作倒计时为 0 秒即不提示不等待直接执行，账号选择、复制、滚动和登录提交等原本不提示动作不新增提示。接管段结束后，游戏条目和启动按钮沿用同一 run 级规则。每次动作仍重新查找并聚焦 WeGame 窗口，再对目标自身模板或 `guardAnyOf` 执行双采样校验；纯 OCR 采样不触发倒计时。等待多个模板时每轮采样全部候选，避免首个目标长期未命中时饿死后续目标。

倒计时结束后，试运行与轮换按 canonical exe 路径结束旧游戏和 WeGame，再启动 WeGame，确保记住账号列表从顶部开始。登录表单出现后直接展开账号列表；`wegame.accountList` 必须紧密覆盖同时可见的 3 行账号。执行器以 400ms 间隔执行两次 Windows OCR；截图内部放大 3 倍后识别，并把行中心坐标映射回原区域。只要求两次都检测到非空账号内容，用于确认列表仍处于展开状态；OCR 文本不再承担目标 QQ 识别，两次文本允许因漏字、错字或分段而不同。连续 3 轮双采样均未检测到数字内容时，列表视为当前账号不可用，立即标记该账号 `NeedsManualLogin` 并继续下一账号，不重启 WeGame、不重复扫描。执行器将列表区域纵向分为 3 个槽位，按从上到下顺序点击；每次选中后清空剪贴板，双击顶部账号并发送 `Ctrl+C`，精确比较 Unicode 文本与目标 QQ。复制值不是目标时，将完整 QQ 加入已见集合并重新展开列表，继续下一槽位；处理完 3 行后把鼠标移到列表中心并向下滚动 3 格。新页面仍按相同方式逐行复制；整页 3 个完整 QQ 均已出现过时判定到底。列表未找到或未复制到有效 QQ 时立即标记账号 `NeedsManualLogin`，不重启 WeGame、不重复扫描；未来多账号 round 跳到下一账号。截图失败、OCR 引擎错误、剪贴板占用或窗口等系统能力异常仍全面暂停。复核成功后才点击登录，每次 run 最多提交一次。

运行期间使用固定 label `special-ops-operation` window，并仅在本次 run 注册 `special-ops-emergency` Strict 热键。operation window 透明、无边框、置顶、固定 480×220 且点击穿透；前端不提供按钮。界面是满幅黑纱 HUD：无倒计时显示当前步骤，占用键鼠倒计时放大等宽秒数并走 1 秒导火索，数字下保留当前步骤，底部粉笔白紧急停止热键。无账号时第一屏是「开始值班」（添加账号、补 exe、去校准），不渲染空的二十四小时任务井。任务行「请在账号页处理」滚到对应账号卡。首次运行先以隐藏状态创建，等待 `PageLoadEvent::Finished` 后显式显示并确认可见，才允许 worker handoff；3 秒未就绪则回滚本次试运行，禁止后台无提示操作。后续运行发现同 label window 已存在时直接显示并复用，运行清理只隐藏而不销毁，避免 Tauri WebView 注册表尚未释放时重建同 label 失败。全局总开关关闭时，登录、导航、制作、子弹四类 start command 均拒绝启动；已运行时紧急热键通过显式 safety scope 绕过全局 gate。`LoginRunSnapshot.runKind` 区分 `login`、`navigation`、`craft`、`ammo`、`round`、`stationWalkthrough`；operation window 以运行事件中的该值更新显示，URL 参数仅作为首次创建前的兜底文案。子弹停止与制作停止相同，durable 写入成功后必须完成 runtime persistence claim，`cleanup_ready` 才允许清理。后台结果通过 `SettingsCoordinator::with_runtime_change` 串行保存并递增 revision，旧 UI save 随后被拒绝。

多账号自动轮次复用相同 operation window 与紧急停止资源。此时 `LoginRunSnapshot.runKind` 为 `round`，`roundProgress` 追加账号序号、QQ、当前制作台及制作台进度；主页面不显示普通取消按钮，只允许“当前账号结束后暂停”或紧急停止。

特勤处的鼠标左键输入固定按住 `100ms` 后抬起，覆盖登录、账号选择与复核、游戏内导航、制作和子弹兑换；账号复核双击中的两次左键分别按住。共享输入状态同时追踪已按下的左键，按住等待检测到取消时先抬起左键再返回错误；紧急停止最多重试 3 次释放已按下的键盘按键和左键。Morse、Rapidfire、Recognition、Timer、Counter 等其他工具仍使用原即时点击节奏。

特勤处试运行开始前建立应用窗口快照，隐藏计时器/计数器/连发器显示窗口及其他可见工具窗口；不隐藏、不最小化、不临时置顶主窗口，也不停止对应后台计时、watcher 或热键。`special-ops-operation` 保留显示。存在摩斯、计时器/计数器/连发器定位或特勤处校准框选窗口时拒绝启动，避免 pending selection session 与游戏输入竞争。成功、失败、普通取消、紧急停止、生命周期停止和启动回滚均恢复快照中仍存在的其他功能窗口；运行期间被关闭的窗口不重建。

`SpecialOpsBootstrap.runSnapshot` 返回当前 run；`special-ops://run-changed` payload 仅含带 `runKind` 的 `LoginRunSnapshot`，不含 settings 或密码，并同时发送到主窗口与 operation window。主窗口提供 WeGame 与游戏 exe 选择、紧急停止热键录制、符合条件账号选择、单次启动和普通取消；启动前先 flush 最新 settings，并使用保存回包的 revision 启动。“继续”或“暂停”IPC 尚未返回时，页面显示对应处理中状态，锁定新试运行、校准、账号与制作配置，避免 scheduler 启动窗口与手动 start 发生竞态。主窗口以 `settingsRevision` 为主序、单调请求序号为同 revision 次序合并 reload/save 回包，并按 `runId`、`updatedAtMs` 合并 run snapshot，旧回包不得回退 runtime 结果。terminal snapshot 只有在同 revision 权威 bootstrap 返回 `runSnapshot: null` 后才清空；清空前设置保存、暂停切换、校准框选、参考图操作、模板测试和新试运行均由前后端共同拒绝，错误固定为“特勤处试运行尚未完成清理”。主窗口显示步骤、消息、倒计时和最近失败时间。

## 游戏内导航试运行

`special_ops_start_navigation_trial` 复用当前已打开游戏，不结束或启动游戏，也不操作 WeGame。预检冻结所选账号、游戏 canonical exe 路径、已测试模板 `game.modeReady` 与 `game.stationGrid`、点击点 `game.beaconMode` 与 `game.specialOps`，以及三段全局固定等待时间。状态机按“等待模式可用 → 点击烽火地带 → 等待后按 Space → 等待后按 Tab → 等待后点击特勤处 → 等待四制作台页面”执行；每步独立 3 分钟超时。三段等待使用整数毫秒，范围 `0–60000`，默认均为 `3000`，在点击区域校准列表对应步骤配置。导航首个键鼠动作前显示 5→4→3→2→1，后续需要提示的动作倒计时为 0 秒即不提示不等待直接执行，固定等待本身不倒计时；中间固定动作不执行模板守卫。每次输入前仍重新查找、恢复并聚焦 canonical 游戏窗口。`game.modeReady` 与 `game.stationGrid` 继续执行 400ms 间隔双采样。成功不改业务状态；独立试运行的步骤超时或步骤执行错误均持久化全局暂停；普通取消与紧急停止均可中断固定等待，停止后不得发送下一输入。

登录与导航共用单实例 runtime、`special-ops-operation` window、`special-ops-emergency` 热键及取消 command，禁止并发运行。运行期收起其他已存在辅助窗口，主工具窗口保持原状态；启动路径禁止同步读取窗口 `is_visible`，避免“继续”命令与 Tauri UI 线程互相等待。Timer、Counter、Rapidfire 和 Recognition 的透明窗口在特勤处结束后不得直接 `show()`，必须按各工具当前总开关重新 reconcile，关闭状态保持隐藏。账号下四制作台不再显示制作物品名称输入框，只保留启用开关、小时、分钟和状态；兼容字段 `itemName` 暂留配置结构。

制作台入口点击点按技术中心、工作台、制药台和防具台保存 4 个独立位置。固定探测另保存共享 `craft.confirmPinned` 确认置顶点击点、共享 `craft.returnToStationGrid` 制作中返回点击点，以及四个全局 `craft.recipe.<station>` 制作物品选择点击点。账号开启独立设置后，UI 额外提供四个账号级制作物品选择点击点；其结果写入 `independentBusinessConfig.recipePoints`，runtime 优先使用账号级点，未配置时回退全局点，不覆盖校准环境。奖励页、共享制作中、四台制作中和制作列表就绪校准项均已删除；旧 `craft.reward`、`craft.inProgress.*`、`craft.recipeListReady.*`、`craft.claimReady.*`、`craft.idle.*` 加载时按默认 target 白名单清理。

`game.modeReady` 使用用户上传的模板图判定模式选择已可操作。没有识别样本前，该步骤不能进入真实执行器；不得仅以固定延时判定成功。

## 判定与动作守卫

执行器必须将模板匹配连续两次一致作为成功，采样间隔约 300–500ms；结果不一致时重新采样，不点击、不输入、不更新持久化状态。账号列表 OCR 仅用于确认两次采样均非空，不比较文本内容；账号身份只由选中后的剪贴板完整 QQ 判定。可见按钮自身使用 `recognitionRegion`，只有自身模板命中后才点击。不能依靠按钮自身判断的固定动作使用以下守卫：

| 动作 | 前置守卫 | 后置判定 |
|---|---|---|
| 结束旧游戏与 WeGame | 用户选择的两个 exe canonical 完整路径 | 对应路径的目标进程实例全部消失；不按 basename 误杀，不递归结束进程树 |
| 启动 WeGame | `wegameExecutablePath` 为有效绝对 `.exe` 文件 | native 进程/窗口检查可继续，随后等待登录入口、表单或游戏入口 |
| 等待登录入口 | WeGame 已启动 | 同时观察 `wegame.loginFormReady` / `wegame.loginMode` / `wegame.gameEntry`。先命中游戏入口（自动登录或「账号已切换」）则当前账号失败，不卡死等待表单 |
| 切换到账号密码登录 | `wegame.loginFormReady` 未命中且登录入口自身模板命中 | `wegame.loginFormReady`；已命中时跳过该点击 |
| 展开记住账号列表 | `wegame.loginFormReady` | `wegame.accountList` 连续两次 OCR 均为非空；连续 3 轮为空则标记当前账号 `NeedsManualLogin` |
| 选择并复核 QQ | 账号列表连续两次 OCR 均为非空 | 稳定识别到 1–3 行时按 OCR 行中心点击；行数、位置或边界不稳定时回退区域三等分；复制顶部账号后与目标 QQ 完全一致才结束扫描 |
| 提交 WeGame 登录 | `wegame.login` 自身模板连续两次命中 | 每次 run 只点击一次；之后只等待 `wegame.gameEntry`，失败时不返回输入步骤、不重复提交密码 |
| 选择置顶游戏 | `wegame.gameEntry` 自身模板连续两次命中 | `wegame.launch` 连续两次命中；运行时不搜索、不滚动游戏列表 |
| 点击启动游戏 | `wegame.launch` 自身模板连续两次命中 | native 检查指定游戏 PID/HWND 出现；登录试运行到此结束 |
| 点击烽火地带 | `game.modeReady` 双采样已命中 | 等待 `navigationSpaceDelayMs` |
| 关闭活动弹窗 | 固定等待结束并重新聚焦游戏窗口 | 按一次 Space；不执行中间模板识别 |
| 切换大厅视角 | 等待 `navigationTabDelayMs` 并重新聚焦游戏窗口 | 按一次 Tab；不执行中间模板识别 |
| 进入特勤处 | 等待 `navigationSpecialOpsDelayMs` 并重新聚焦游戏窗口 | 点击 `game.specialOps` 点击点，再等待 `game.stationGrid` 双采样命中 |
| 制作台固定探测 | 当前游戏窗口可聚焦 | run 首个键鼠块显示 5→4→3→2→1；点击 `craft.station.<station>` → 等待 `craftSpaceDelayMs` → 按 Space → 等待 `craftReopenDelayMs` → 再次点击制作台 → 等待 `craftConfirmPinnedDelayMs` → 点击 `craft.confirmPinned`；后续原本不提示的固定输入继续不提示 |
| 判断正在制作 | 固定探测输入已完成 | `craft.abort` 单次双采样；连续命中按新制作落盘（`startedAtMs = now`，`finishesAtMs = now + 配置时长`），与生产后命中中止相同；批次再点 `craft.returnToStationGrid` 并等待 `game.stationGrid` |
| 进入制作列表 | `craft.abort` 两个有效低分样本 | 按 run 级规则显示倒计时，点击当前台 `craft.recipe.<station>` 制作物品选择点 |
| 判断生产路径 | 制作物品选择点已点击 | 等待 `craft.fill` 或 `craft.produce` |
| 点击制作一键补齐 | `craft.fill` 自身模板 | `craft.purchase` |
| 购买制作材料 | 由前一步 `wait_ready` / `wait_button` 确认 `craft.purchase`，点击本身不再重复复核 | 识别命中后点击独立点击点 `craft.purchaseClick`，每次点击后等待 1 秒，双采样 `craft.produce`、`craft.purchase` 或购买 UI 消失后重新出现的 `craft.fill`；命中 `craft.fill` 时再次点击补齐并购买，连续 3 次仍回到补齐则隔离账号 |
| 开始制作 | `craft.produce` 按钮自身模板 | `craft.abort` |
| 返回部门页 | `ammo.department` 已命中时跳过；否则仅在 `game.stationGrid` 或 `craft.abort` 命中时按一次 Tab | `ammo.department` |
| 点击部门 | `ammo.department` 自身模板 | 识别命中后倒计时一次，再点击模板中心 |
| 点击军需处 | 独立固定等待 `ammoSupplyDelayMs` | 等待结束后倒计时一次，直接点击 `ammo.supply` 点击点，不识别该入口 |
| 进入军需处 | 独立固定等待 `ammoTacticalDelayMs` | 等待结束后倒计时一次，直接点击 `ammo.enterSupply` 点击点；同账号子弹与限时商品共享该入口 |
| 点击战术部门 | `ammo.tacticalDepartment` 自身模板 | 子弹分支识别命中后点击模板中心 |
| 点击研发部门 | `ammo.researchDepartment` 自身模板 | 限时商品分支识别命中后点击模板中心 |
| 点击普通目标子弹 | 普通目标配置顺序 | run 级倒计时；先按 A、D（各间隔 100ms），再向下滚配置次数，事件间隔 100ms，结束等待 1000ms 后点击；随后等待 `ammo.success`、`ammo.fill` 或 `ammo.exchange` |
| 切换赛季限定 | 全部普通目标结束且存在赛季目标 | 按 run 级规则显示倒计时，直接点击 `ammo.seasonal` 点击点；多个赛季目标之间不重复点击 |
| 点击赛季目标子弹 | 赛季目标配置顺序 | run 级倒计时；先按 A、D（各间隔 100ms），再向下滚配置次数，事件间隔 100ms，结束等待 1000ms 后点击；随后等待 `ammo.success`、`ammo.fill` 或 `ammo.exchange` |
| 点击子弹一键补齐 | `ammo.fill` 自身模板 | 按 run 级规则显示倒计时后点击，随后识别 `ammo.purchase` |
| 购买子弹材料 | `ammo.purchase` 按钮自身模板 | 识别命中后点击独立点击点 `ammo.purchaseClick`，每次购买点击前倒计时一次，点击后等待 1 秒，双采样 `ammo.exchange`、`ammo.purchase` 或购买 UI 消失后重新出现的 `ammo.fill`；命中 `ammo.fill` 时再次点击补齐并购买，连续 3 次仍回到补齐则隔离账号 |
| 兑换子弹 | `ammo.exchange` 可兑换状态模板 | 按 run 级规则显示倒计时后点击兑换；双采样命中全局 `ammo.confirm` 用户参考图后再次按规则提示并点击区域中心，再以 `ammo.success` 用户参考图确认完成；均不读取颜色 |

`ammo.success` 在 30 秒内未连续命中时，runtime 重新截取已配置区域，保存到 `<应用数据目录>/special_ops_diagnostics/<时间戳>-<账号QQ>-ammo.success.png`。失败消息保留目标、阈值与最后双采样，并附截图绝对路径；截图保存失败只能附加说明，不得覆盖原识别失败。诊断图不截全屏，也不包含 WeGame 登录区域。

每个普通裸动作的允许前置状态保存于校准目标 `guardAnyOf`，其语义为 OR。执行器必须先确认其中至少一个守卫连续两次命中，才能使用该动作坐标。固定探测中的 `craft.station.*`、`craft.confirmPinned`、`craft.returnToStationGrid` 与 `craft.recipe.*` 属于显式状态机动作：制作台与确认置顶点不执行通用倒计时或正向模板守卫；返回点只在 `craft.abort` 连续命中后无倒计时点击；物品选择点恢复倒计时但仍不做正向守卫。安全边界由固定顺序、`craft.abort` 一次性双采样、`game.stationGrid` 返回确认和失败后 `Uncertain` 保证。窗口恢复、进程启动、进程结束和窗口存在使用 native 状态，不以截图模板替代。

制作或子弹补齐购买最多点击 3 次，每次点击后等待 1 秒。购买结果双采样判定：制作对应 `craft.produce` / `craft.purchase` / `craft.fill`，子弹对应 `ammo.exchange` / `ammo.purchase` / `ammo.fill`。购买材料按钮的识别区域与点击点分离：`craft.purchase` / `ammo.purchase` 保持 RecognitionRegion 只做识别与复核，实际点击落在 `craft.purchaseClick` / `ammo.purchaseClick` 两个 ClickPoint 上，映射集中在 `click_target_key()`。两个点击点以对应识别区域作 `guardAnyOf` 守卫，识别命中购买按钮才允许点击，不存在盲点；冻结配置缺少点击点时回落到识别区域，避免旧配置在轮次中途硬失败，由 preflight 要求补校准。重试次数与仓库空间不足判定流程不变。购买 UI 消失并回到补齐按钮时，重新点击补齐后再购买；第三次仍回到补齐或停留购买按钮时，不依赖短暂仓库公告文本，账号标记为 `Isolated`。两者都结束当前账号剩余流程并切换下一账号；切换前关闭游戏失败只记 warn 并继续下一账号，不再转为全局暂停。双采样不一致、无稳定状态或截图/窗口/输入错误不得误判仓库空间不足，仍按目标级人工失败或系统级失败处理。

单账号登录试运行 preflight 只检查所选账号为非空纯数字 QQ、两个有效绝对 `.exe` 文件，以及上述 5 个 template 的有效双采样签名、账号列表展开点击点、账号列表 OCR 区域和顶部账号双击区域。完整自动化的业务 preflight 仍按启用制作台、普通子弹和赛季限定子弹计算必需校准项；账号缺 QQ、启用制作台缺有效时长、启用子弹缺备注或指定点击点，或任一必需目标缺配置、参考图、有效双采样签名时均拒绝启动并报告首个失败步骤。暂停状态允许保存不完整草稿，避免配置期间反复报错。

## 未来 24 小时任务时间轴

`ScheduleSnapshot` 除现有 `dueAccounts` 和 `nextWakeAtMs` 外，返回 `timelineStartMs`、`timelineEndMs` 与排序后的 `timelineTasks`。制作任务来自启用制作台的实际 `finishesAtMs`；子弹任务来自当天尚未成功的启用目标，并在次日兑换时间落入窗口时投影次日目标。可定位失败任务即使不具备 runtime eligibility 也投影为当前到期，并携带 `manualFailure`；非 `Ready`、登录失败、状态不确定或隔离账号仍展示任务并携带 `accountStatus`，但这不代表 runtime 允许执行。

前端按小时分组显示未来 24 小时内有任务的时段，空小时不占位；每分钟刷新当前时间和权威 bootstrap。逾期任务保留原计划时间，延迟固定显示“0 分钟后”。任务按执行顺序排序，对齐 `build_round_plan_with_profit`：已到期任务排在前面并按账号配置顺序分桶（同账号内按时间），未到期任务整体排在其后并按时间优先、账号顺序次之，同键再按制作台顺序与任务 ID 定序。未到期桶保留账号顺序作次键，避免同毫秒未来制作台被拆进多个 `AccountRoundTask`。视觉分组以每组第一项为锚，仅把严格小于 10 分钟的后续任务合并到同一视觉块，不链式扩展、不改执行时间、不提供拖动或改期入口。结构化制作失败行显示三个单项按钮；结构化子弹失败行显示两个单项按钮；账号级人工状态显示“已人工检查”。制作 scheduler 使用制作台实际完成时间触发，不使用视觉分组结果；启用联网利润筛选时，子弹任务额外显示等待查询、截止查询中、等待 5 分钟补查、已达标、当前轮次；当天已轮空目标不投影到任务时间轴，调度仍保留当天不兑换结果。

## 联网利润筛选

`profitFilter` 保存独立开关、利润截止时间、规则、当天最近审计及 `cutoffState`。规则包含工具生成的稳定 ID、显示名、KKRB 精确名称、可选 Moligod 精确名称和非负最低总利润；默认业务配置与账号独立业务配置中的子弹目标只保存 `profitRuleId` 引用。规则、绑定、审计和当天截止决策写入 `special_ops_settings.json`；常规 qualified rule IDs、查询 generation、组内 cadence、取消令牌和 active round targets 只保存在进程内，重启后审计不得复用为常规兑换资格。

特勤处配置同时纳入顶栏 Profile 快照 `specialOps`。快照保存完整 `SpecialOpsSettings`，校准参考图片只保存本地路径，不复制图片二进制；运行中的轮次或试运行存在时拒绝切换 Profile。应用含特勤处快照的 Profile 后，特勤处 scheduler 强制 disarm、配置强制暂停、利润查询 runtime 失效，必须由用户点击“继续”恢复。

筛选默认关闭，关闭时现有多账号子弹流程不读取规则或审计。启用后，从每日兑换时间到利润截止时间按“立即 → 5 分钟 → 5 分钟 → 50 分钟”串行查询；KKRB 结构化数据是主源，只有主源 HTTP、根 schema、业务错误或“系统繁忙”等整体失败时才调用 Moligod。KKRB 正常但目标缺失、重复或利润无效时不使用 Moligod 覆盖该结果。Moligod 使用只允许 `https://moligod.com/*` 且无 IPC permission 的主窗口外侧 child Remote WebView，不创建顶级查询窗口；通过 DOM 精确名称读取网页已显示的“预估净利润”，不使用截图、OCR、网页排序、键鼠或配方重算。页面仍显示“加载军需处兑换价格中...”时不得把稳定的空 DOM 当成结果，必须等加载提示消失后再开始精确名称扫描。child WebView 关闭后，独立数据目录在后台最多重试 30 秒清理；仍被 WebView2 锁定时只写 warning，不覆盖已取得的利润结果或向 UI 返回 `os error 32`。配置页手动“刷新 KKRB 名称”遇到业务码 `-101` 时最多重试 3 次、每次间隔 1 秒；3 次仍繁忙则保留已加载名称并提示“KKRB 暂时繁忙，名称列表未更新。可直接手工填写并保存‘KKRB 精确名称’”。该规则不影响后台查询节奏或 Moligod fallback。

截止前达到各自最低利润的规则可冻结进本轮；同一规则被多个 `Ready` 账号引用时，所有符合当天状态的目标一起进入轮次。未达标、规则未绑定、目标缺失或两站失败保持待执行，不增加游戏内 retry，不标记账号失败。到达截止时间时冻结当日剩余账号与子弹目标，执行一次固定最低总利润 10,000 的最终查询：低于阈值直接标记当天轮空；目标缺失、来源失败或利润无效在 5 分钟后补查一次，第二次仍失败则当天轮空；未绑定有效规则的目标直接轮空。截止达标结果按 `(accountId, targetId)` 放行，截止后新增目标不加入当日冻结范围。round 启动时消费同一 generation 的资格并记录 active targets；窗口、热键或 worker handoff 启动失败时立即撤销该 generation，禁止遗留 `ActiveRound`。撤销只解除 `ActiveRound` 占用：`consume_for_round` 把 `qualifiedRuleIds` 取到 `consumedRuleIds` 暂存，`rollback_failed_round_start` 将其原样放回并把 phase 转为 `WaitingNextQuery`，不清空当天资格。启动失败常见于操作提示窗加载超时，1 秒后就会重试同一到期动作；若把资格一并烧掉，重试时 gate 变空、全部子弹被滤出计划，日志表现为每个任务 `ammoTargetCount` 为 0，当天只执行制作后关闭游戏且再也不兑换。资格清空只发生在 `end_active_round`，即轮次真正结束之后，避免凭旧资格重复兑换。

取消与清空是两套语义，不能混用：

| 入口 | 内部 | 效果 |
|------|------|------|
| `ProfitQueryControl::cancel_in_flight` | `cancel_runtime` | bump generation、`notify_waiters`、清节奏与 `activeQuery`，**保留** `qualifiedRuleIds` / `consumedRuleIds` / `currentSessionRuleIds` / `activeRoundTargets`；`ActiveRound` 相位不降级 |
| `ProfitQueryControl::invalidate` | `reset_runtime` | 同上再连当天资格一起清空 |

绝大多数场景只是让 in-flight 查询作废，“当天哪些规则达标”这个结论并没有失效，必须走 `cancel_in_flight`：导航失败、账号/制作台/子弹人工判定、一键恢复、settings autosave、`set_paused` 双向、scheduler 暂停、资源释放、审计保存失败。只有身份真的没了才 `invalidate`：切 Profile、应用关闭、利润查询配置本身被改。

`sync_window` 按同一套语义分流——换天走 `reset_runtime(Disabled)`，仅 `settingsRevision` 前进走 `cancel_runtime`；`paused` 分支也只取消，且用 `!matches!(phase, Paused | ActiveRound)` 去重。`begin_query` / `begin_cutoff_query` 内部同构。`SettingsCoordinator::with_expected_revision_change` 会在每次 settings 写入时 bump revision，把 revision 变化当身份变化会让任何一次 autosave 或人工判定清掉当天资格 -> `profit_gate_for_round` 拿到 `Qualified(∅)` -> 利润 gate 下的子弹目标全被滤掉，达标了也不提前兑换，且没有任何报错。

`special_ops_save_settings` 用 `profit_query_identity_changed` 判断该走哪条：只比较 `profitFilter.enabled`、`profitFilter.cutoffTime`、`profitFilter.rules`、`dailyExchangeTime` 四项查询输入。审计历史与 `cutoffState` 是查询的**产物**，延迟、点击点、账号顺序与查询无关，改它们不清资格。

达标必须**立刻**兑换，不等截止时间。这条依赖 `ProfitRuntimeSnapshot` 一路传到 planner：`build_round_plan_with_profit` 必须调用带快照的 `build_schedule_with_profit_runtime(settings, createdAtMs, gate, profitSnapshot)`，`profit_gate_for_round` 取到的快照要经 `freeze_round_run` 透传进去。任务栏投影只在拿到 `qualifiedRuleIds` 时才把达标子弹的计划时间提到「现在」并标 `Qualified`；没有快照会退到 `WaitingQuery` 分支、把子弹排到 `cutoffAtMs` -> planner 的 `is_due`（`scheduledAtMs <= createdAtMs` AND `DueAccount` 标记）恒为 false -> 计划为空 -> 抛 `EMPTY_ROUND_PLAN_ERROR`。该错误在 `is_transient_round_launch_error` 列表里，只 warn 并 `RetryAfter(30s)`，所以表现不是报错而是静默：scheduler poll 自己带快照、判定「该启动了」，freeze 不带快照、算出空计划，两边每 30 秒对拆一次，一直重试到截止时间才真正兑换。

## WeGame 进程与窗口约束

用户通过文件选择器指定 WeGame 与游戏 `.exe`。执行器 canonicalize 两个完整路径，只结束路径精确匹配的进程实例；同名不同目录进程不匹配，辅助子进程不会因父子关系被递归结束。WeGame 登录前后可能把主窗口转交给子进程，因此每次 WeGame 键鼠输入前重新查找配置 exe 及其完整进程树中的顶层窗口，并在恢复、聚焦前再次复核 HWND→PID→当前进程树；不按窗口标题或 `browser.exe` 名称兜底。游戏窗口仍按游戏 exe 自身的完整路径确认，不扩大到游戏进程树。请求前台切换后最长等待 1.5 秒，每 50ms 复核一次前台 HWND；首轮仍未成为前台时，执行器发送一次成对 Alt 按下/释放并重试一次。重试后仍无法恢复、聚焦或确认归属时不发送后续输入，当前步骤失败并全面暂停。流程不要求用户在点击试运行前手动把游戏置前，不要求最大化 WeGame，也不按比例缩放旧校准坐标。

## 单账号制作试运行

新增 `special_ops_start_craft_trial`，支持选择一个账号和一个制作台执行收取并重做。流程按三段全局等待执行固定动作：点击制作台、等待后按 Space、等待后再次点击制作台、等待后点击共享确认置顶点；三段范围 `0–60000ms`，默认均为 `3000ms`。随后只对 `craft.abort` 执行一次双采样：连续命中按新制作落盘（`startedAtMs = now`，`finishesAtMs = now + 配置时长`），与生产后命中中止相同，不发送 Esc；两个有效低分样本视为已进入制作列表，恢复普通倒计时并点击当前台 `craft.recipe.<station>` 制作物品选择点，再按 `craft.fill` / `craft.produce` 进入原生产流程。一高一低、截图或参考图错误、返回点击失败、返回页面确认失败均不得降级为低分分支；输入开始后的失败会暂停自动化，将账号和当前台标记为 `Uncertain` 并保存实际失败步骤。生产后 `craft.abort` 命中时以确认时间记录 `startedAtMs`，按配置时长计算 `finishesAtMs`。购买按钮连续三次稳定出现时返回 `craft.isolated`，账号保存为 `Isolated`，不把当前制作台覆盖为 `Uncertain`。

制作试运行与登录、导航共用窗口仲裁：隐藏其他功能窗口，主工具窗口保持原状态。仍沿用 operation window 与紧急停止。run 首个键鼠块显示 5→4→3→2→1，后续原本需要提示的动作倒计时为 0 秒即不提示不等待直接执行，固定探测中原本不提示的后续输入继续不提示；每次输入仍检查取消、重新聚焦游戏窗口并在输入后停放鼠标；制作物品选择、补齐、购买和生产继续使用同一 run 级倒计时，其中补齐、购买和生产执行正向双采样。preflight 冻结当前台制作台点击点、共享确认置顶点、当前台制作物品选择点、`game.stationGrid`、`craft.fill/purchase/produce/abort` 及三段等待。

## 当前账号四制作台批处理试运行

`special_ops_start_craft_batch_trial` 在创建 runtime、operation window 或发送键鼠输入前，以一次 `nowMs` 冻结当前账号任务集合。账号必须为 `Ready`；只选择启用、非 `Uncertain` 且 `finishesAtMs <= frozenNowMs` 的制作台，按技术中心 → 工作台 → 制药台 → 防具台执行。空任务直接返回“当前账号没有到期制作台”，不创建运行态或占用键鼠。

批次开始先双采样确认 `game.stationGrid`，每台复用单制作台状态机。到期台探测先命中中止、未进入购买/生产时按新制作处理：`startedAtMs = now`，`finishesAtMs = now + 配置时长`（例如 8 小时物品即使游戏还剩 4 小时也记满 8 小时），与生产后命中中止同一条 `Started` 落盘路径，再返回四制作台页进入下一台。`Started` 先通过 `SettingsCoordinator::with_runtime_change` 保存实际开始时间和新完成时间，再点击 `craft.returnToStationGrid` 并确认 `game.stationGrid`，随后进入下一台。最后一台 `Started` 也必须返回四制作台页面。运行中新增到期台不插入当前批次。任一单台失败、持久化失败、返回失败或取消立即截断后续台；此前已成功写入的台不回滚，当前失败台按实际输入状态和失败类型决定是否标记 `Uncertain`。

## 多账号自动轮次

`special_ops_start_due_round` 与后台 scheduler 共用 `build_schedule()` 和同一 worker。轮次启动时冻结启用、已初始化、`Ready` 账号中已到期且业务配置启用的制作台、当天可执行子弹、限时商品、交易行，以及未来 24 小时制作任务的只读 lookahead。已到期业务分成两组分桶：非交易行业务（制作台、子弹、限时商品）按账号配置顺序合并成每账号一个 `AccountRoundTask`，交易行单独成桶；桶内制作台按技术中心 → 工作台 → 制药台 → 防具台排序，子弹按业务配置顺序执行。未来制作任务仍按时间追加，不并入已到期账号桶，只用于后继判断，不得提前执行。

交易行全局排最后，不参与账号顺序混排：全部账号的非交易行桶按账号顺序跑完，才轮到交易行桶（交易行桶之间同样按账号顺序）。账号 1 有特勤处 + 交易行、账号 2 只有特勤处时，执行序列是账号 1 特勤处 → 账号 2 特勤处 → 账号 1 交易行。禁止在分桶后再按 `(account_order, scheduled_at_ms)` 整体重排：那会把交易行桶塞回它自己账号后面 -> 账号 1 交易行插到账号 2 之前 -> 交易行不再是最后。没有其他账号的非交易行任务时，两个桶相邻且同账号，`can_chain_follow_up` 保持会话，账号 1 特勤处跑完直接进交易行，不重新登录。

已到期账号桶按上述顺序依次执行，不再让其他账号已到期任务插入同账号桶。同账号非交易行桶内先制作，再通过一次共享军需处入口串行执行子弹和限时商品。当前任务成功后检查未来下一任务：下一任务已逾期，或与当前任务计划时间差 `<=10` 分钟时，本轮继续；同账号保留游戏并等待到期，不重新登录，其他账号先关闭旧游戏再走正常登录流程。下一任务尚未到期且时间差 `>10` 分钟时关闭游戏并结束本轮，由 scheduler 到点重新规划。未来任务仍按计划时间排序，禁止提前并入已到期账号桶。

到期桶的 `scheduledAtMs` 取桶内最早完成时间，`freeze_round_run` 冻结制作配置时必须用 `scheduledAtMs.max(frozenNowMs)`：`select_due_craft_tasks` 按 `finishesAtMs <= frozenNowMs` 过滤，直接把桶的最早时间当过滤基准会丢掉同桶里完成更晚但同样已到期的制作台，桶被冻结阶段重新拆成一台一轮。未来桶必须保留自身计划时间才能通过过滤，因此只能取 `max` 而非固定用当前时间。

冻结缓存键 `FrozenRoundAccountKey = (String, i64, bool)`，第三位是「是否交易行桶」。交易行独立成桶后，同账号可以同时存在非交易行桶与交易行桶，而两者的 `scheduledAtMs` 都是分钟对齐的（子弹取每日兑换时间、交易行取窗口起点）；配成同一分钟时 `(accountId, scheduledAtMs)` 二元组完全相同 -> `collect()` 只留下后插入的交易行桶 -> 非交易行桶拿到 `craft: []` / `ammo: None` 的冻结配置，制作与子弹被静默跳过。

每台生产成功后立即通过 `SettingsCoordinator::with_runtime_change` 保存实际开始时间和下一完成时间；每种子弹命中 `ammo.success` 后立即保存当天成功，后续失败不得回滚既有结果。登录与制作异常保存账号失败并阻断该账号后续调度；`craft.isolated` 保存账号 `Isolated` 且不覆盖当前制作台状态。子弹补齐、购买、确认或完成识别失败时保存当前 `ammoTargetId` 的 `AmmoTarget.lastFailure`；仓库空间不足对应的 `ammo.isolated` 同时把账号标记为 `Isolated`，其他目标和制作台随账号跳过。普通目标级兑换失败仍保持账号 `Ready`，当前账号本轮结束后切号；该目标后续被冻结，同账号制作和其他子弹仍可调度。普通兑换失败只增加当天 retry，不进入人工判定。账号选择成功后的游戏入口、启动按钮和游戏窗口等待也归入导航启动阶段；军需处 `ammo.department` / `ammo.tacticalDepartment` / `ammo.researchDepartment` 与交易行 `market.entry` 模板超时同样走该路径：首次 `TimedOut` 落 `lastFailure` 保持 `Ready` 并把账号插到剩余到期任务队尾（远期 lookahead 之前；append 到整队最后会让 last due 账号去 `wait_until` 未来制作，游戏已关掉，WeGame 不关、重试永不启动。同时记录 `log_info!` 日志「导航超时，账号任务已挪到队尾重试」，可在运行日志中区分「已挪队尾待重试」与「二次超时已持久化失败」），重试从登录流程重新开始；第二次同一问题仍超时才保存 `ManualCheckRequired`，制作台与子弹状态保持不变。跨轮次靠 `lastFailure` 识别同一问题，单账号不会无限重试。成功进入正常流程后清掉这次 `lastFailure`。账号列表扫描、账号复核、登录提交失败仍是登录失败，不重复提交密码。导航结果通过 `TimedOut` 与 `Paused` 类型分流，不依据错误消息文本猜测；`Paused` 及导航窗口、截图、输入、持久化和 runtime 资源故障保持系统级，持久化全局暂停并停止本轮。运行中点击暂停只登记请求，当前账号结束后保存暂停并停止切号。紧急停止立即释放输入；当前账号或制作台已发生输入时标记 `Uncertain`。

轮次正常完成、切换账号或遇到超过 10 分钟的空档时，按启动时冻结的 canonical 游戏 exe 路径关闭游戏，不关闭 WeGame。转场发 `TerminateProcess` 两轮：游戏窗口没了就算成功，不因残留进程默等 1 分钟。扫描/OpenProcess 卡内核时 5 秒放弃。下一号登录再补杀；登录先强杀游戏直到进程没了，立刻强杀 WeGame 两轮：杀掉则启动 WeGame，还在则聚焦已有窗口继续登录，不开第二份。上一个号结束后 overlay 以与开场 5 秒相同的倒计时提示「N 秒后切换下一账号」，从 15 数到 1 再跑下一个号。查询被拒仍按文件名杀。强杀前启用 `SeDebugPrivilege`，失败忽略。overlay 更新为「正在关闭游戏，准备切换下一账号」；紧急停止和用户暂停都必须立刻打断这次等待。导航超时后、账号失败后和会话结束这三处切换关闭失败只记 warn 并继续本轮，不再全局暂停：登录流程头两步 `StopGame` / `StopWeGame` 会用各自预算无条件重杀游戏与 WeGame -> 残留进程下轮自愈，为一次慢退出停摆到人工点继续代价远高于收益。`PauseRequested` 先持久化暂停（原因固定为“用户请求暂停”），再关闭游戏；该路径关闭失败仍返回 `SystemFailure { step: "round.closeGame" }`，但暂停原因已落盘，进程错误文本不会写进 `pausedReason`。scheduler 健康检查触发的系统暂停使用 `PauseRequestedPreservingGame`：停止轮换但不关闭游戏，保留现场；用户继续后重新规划并走完整登录流程，不复用旧会话。`SystemFailure` 与 `EmergencyStopped` 同样不关闭游戏。

应用启动强制保持暂停，scheduler 默认未 armed。用户点击“继续”时先执行完整业务 preflight；任一必需 template 未测试或验证失效时保持暂停并返回具体目标，不创建窗口、不启动键鼠流程。校验和暂停状态持久化成功后立即 armed：逾期制作与当天子弹立即进入 round，未来任务按上述 10 分钟规则等待或留给下一轮；配置页不提供手动启动到期轮次入口。本轮结束后 scheduler 继续等待下一制作完成时间或每日兑换时间。每日兑换时间前 5 分钟内到期的制作任务延迟至兑换时间合并；已成功或当天重试耗尽的子弹不再加入。操作提示窗页面加载超时时，scheduler 保持 armed、不创建 worker、不发送键鼠，1 秒后重试同一到期动作；提示窗挂载后主动读取权威 bootstrap 的 `runSnapshot`，避免错过窗口创建早期的运行事件。scheduler 使用单 worker、`Notify` 唤醒和最长 30 秒健康检查；设置保存、人工校正和 round 完成会立即唤醒。定时器晚醒超过 60 秒视为休眠或系统时间跳变，保存暂停、请求 active round 停止、刷新逾期任务并聚焦主窗口；用户继续后不复用休眠前游戏会话。判定晚醒必须用 poll 成功返回的 `nowMs`；poll 本身失败不算时间跳变，交给下一轮循环写真实错误原因。全局总开关关闭时 disarm，应用退出时 shutdown。

scheduler 启动到期轮次失败分两类。poll 与 `freeze_round_run` 的过滤条件不完全一致（利润 gate、business config、运行态在两次读取之间可能变化），因此“当前没有到期特勤处任务”“当前没有到期制作或子弹任务”“处于暂停状态”“总开关已关闭”“试运行尚未完成清理”“配置保存已陈旧”属于正常竞态，只记 warn 并 `RetryAfter(30s)`，不暂停自动化；其余错误才全局暂停。空计划文案由常量 `EMPTY_ROUND_PLAN_ERROR` 同时供 `freeze_round_run` 抛出与分流列表匹配，禁止两处各写字面量：一旦漂移，利润达标当天的空计划竞态会被判成故障全局暂停，而暂停会停掉当天所有查询与轮次。暂停本身不再清 `qualifiedRuleIds`（`paused` 分支已改为只取消 in-flight 查询），但被误判暂停仍会让当天卡在原地，且 `pausedReason` 会写进一个用户看不懂的原因。所有自动暂停把原因写入 `SpecialOpsSettings.pausedReason`，页头以 warning alert 展示“自动化已暂停：{原因}”。用户手动切换暂停或继续都会清空该字段 -> UI 只在“不是我点的”时给出解释。`special_ops_save_settings` 强制沿用当前进程内的 `paused` 与 `pausedReason`，前端草稿不得回滚运行态；只有 `special_ops_set_paused` 与自动暂停路径能改这两个字段。

`SettingsCoordinator::with_runtime_change` 在每次 runtime 写入（制作开始、子弹成功、账号失败等）时递增全局 revision，但不 emit `profile://changed`。其他工具（如 Rapidfire）的 autosave 携带的 `settingsRevision` 在特勤处运行期间可能因此变陈旧，提交时后端拒绝并返回「配置保存已陈旧」。前端 `useBootstrapForm` 捕获该错误后自动调用 `profile_get_bootstrap` 刷新最新 revision 并重试一次，失败计数不超过 1 次，整个过程对用户无感知。

## 当前边界

已实现配置持久化、24 小时任务投影、暂停配置、账号/制作台/有序子弹点击与滚轮配置、区域框选、模板双采样测试、原生单账号登录、游戏内导航、单账号单制作台制作试运行、当前账号四制作台批处理、单账号真实子弹兑换试运行、多账号制作与当天子弹合并 round、后台 scheduler，以及无按钮 operation window。默认子弹兑换顺序、账号独立设置与利润业务目标表默认折叠，不保存展开状态。`ammo.confirm` 和 `ammo.success` 均使用用户参考图模板，不做识色。完整游戏/WeGame 崩溃恢复尚未实现，自动轮次仍需桌面开发版实机验收。

## Tauri Commands

| 命令 | 作用 |
|---|---|
| `special_ops_test_calibration_target` | 对模板区域执行两次真实截图与 NCC，或对 OCR 区域执行两次真实 Windows OCR；点击点和输入区域拒绝测试 |
| `special_ops_start_login_trial` | 校验 revision 与登录 preflight，启动单实例后台试运行并立即返回 run snapshot |
| `special_ops_start_navigation_trial` | 校验导航 preflight，从当前游戏进入四制作台页面 |
| `special_ops_start_craft_trial` | 运行当前账号指定单制作台试运行 |
| `special_ops_start_craft_batch_trial` | 冻结当前账号到期制作台并按固定顺序批处理 |
| `special_ops_start_ammo_trial` | 冻结当前账号全部启用子弹，按普通组、赛季组执行真实兑换并即时保存结果 |
| `special_ops_start_due_round` | 冻结全部到期制作与当天子弹任务，按账号顺序执行多账号自动轮次 |
| `special_ops_confirm_account_station_states` | 原子保存四制作台及当天全部启用子弹状态；`Uncertain` / `Isolated` 可恢复 `Ready` |
| `special_ops_confirm_account_manual_check` | 确认账号级人工问题（登录、账号列表扫描、二次导航超时、紧急停止后的 `Uncertain`、仓库不足的 `Isolated`）；gate 为 `!matches!(status, Ready)`，恢复 `Ready`、按存量计时还原 `Uncertain` 制作台、清被 `ammoTargetId` 指名的子弹目标失败，不改子弹成功日与 retry |
| `special_ops_restore_account_state` | 一键恢复单账号或全部账号异常：账号回 `Ready`、`Uncertain` 制作台按存量计时还原、失败子弹解冻、清当天 `lastSuccessDay`、限时商品 `Failed` 回 `Pending` |
| `special_ops_confirm_station_state` | 只校正时间轴中结构化定位的失败制作台，保留其他子弹失败 |
| `special_ops_confirm_ammo_state` | 只校正时间轴中结构化定位的失败子弹，写入或清除当天成功状态 |
| `special_ops_cancel_login_trial` | 请求普通取消；等待 worker 完成统一清理 |
| `special_ops_emergency_stop` | 立即释放输入并将当前账号标记为不确定 |

### 限时商品与交易行

限时商品任务固定在 Asia/Shanghai 每日 12:00、20:00 创建。运行步骤为 Tab、识别点击部门、固定等待点击军需处、固定等待点击进入军需处、识别点击研发部门，随后等待 `limitedSupply.researchDelayMs` 再识别 `limited.ready` 与 9 个 `limited.color.1`–`limited.color.9` 区域。与子弹同时到期时复用前三个军需处入口动作。任意区域命中配置颜色即记录高价值提醒；不执行购买。高价值结果保存 `matchedColorIndexes`（配置颜色 1 / 2，可同时命中），账号校正面板和任务栏展示「命中颜色 1」「命中颜色 2」或「命中颜色 1 和 2」。

研发部门点击后必须先等 `researchDelayMs` 才识别页面（`LimitedRunConfig.enter_delay`）。点完立刻采样时 `limited.ready` 有机会在**上一页**连续命中两次直接放行，后续识色跑在错误页面上、两次采样同样“稳定”，约 800ms 内就写下高价值 -> 表现为只点到研发部门就关游戏并标记发现高价值，实际没有检查。

当前周期的任务**检查完即出栏**：`build_timeline_tasks` 中只有 `outcome == Pending` 的限时商品任务才进入任务栏，`noHighValue` / `highValue` / `failed` 任意终态都立即出栏。任务消失后不会自动重跑——「每周期只跑一次」由 `limited_supply_due`（只认换周期或 `pending`）单独保证，重跑只由人工触发。任务栏不再渲染 `limitedOutcome` 结果行或”已查看高价值商品”按钮；确认与重新检查两个入口均移至账号人工校正面板（`CorrectionLimitedSupply`）。未确认 `highValue` 时”已查看高价值商品”按钮才显示，调用 `special_ops_acknowledge_limited_supply`；重新检查按钮四种终态全可点，调用 `special_ops_recheck_limited_supply` 把状态复位到 `pending`，同时重开任务栏的出栏 gate 和 `limited_supply_due` gate。失败原因仍只写入 `limitedSupply.lastError`。

`special_ops_recheck_limited_supply` 把账号的 `LimitedSupplyAccountState` 复位到 `pending`，但**保留** `cycleId`——任务栏用 `state_matches`（`account.limited_supply.cycle_id == cycle.id`）把结果认领到当前周期任务上，清掉 `cycleId` 会断开这个关联。命令拒绝周期已变化和状态仍为 `pending` 两种请求。

已检查过的周期出栏后不可再被 planner 调度——两个门的 AND 关系：任务必须同时出现在 `schedule.timeline_tasks`（`build_timeline_tasks` 产出，只接受 `pending`）且 `DueAccount.limited_supply_due` 为真（`build_schedule_with_profit_runtime` 产出，同样只认换周期或 `pending`），`build_round_plan_with_profit` 的 `is_due` 拿时间轴任务去匹配 `due_accounts`。出栏后 `is_due` 永远匹配不到 -> 本周期彻底不可重跑。放宽任一侧 gate 接受终态都会复现无限重查的 bug，因此严格同源。重跑只由 `special_ops_recheck_limited_supply` 人工触发（复位到 `pending` 同时重开两侧 gate），常驻状态不会导致自动重跑。

`compare_samples` 判定 `highValue` 的条件是两次采样的命中区域集合**完全相等**（哪怕只有 1 个区域命中），且该区域两次命中的目标颜色相同——9 个区域里任意一个稳定命中即可判高价值。取交集非空就判定会把命中抖动（第一次命中 1、3，第二次只命中 3）算成一致；只读第二次采样的颜色会把跨色命中（第一次颜色 1、第二次颜色 2）算成稳定结果。两者都会误报高价值，因此都必须继续采样，直到超时写入 `failed`。两次均无命中仍直接判 `noHighValue`。

交易行任务固定在每日 02:00–04:00。同账号桶里有子弹或限时商品任务时，先点一次 `market.backToEntry`（校准名「返回大厅列表点击点」）把界面从部门页退回大厅列表，再识别点击 `market.entry`；没有部门任务时界面本来就在大厅，跳过这一步——提前点会把界面带进别的面板。判定条件是 `!task.ammo_target_ids.is_empty() || task.limited_supply_cycle_id.is_some()`，不是「进入交易行流程就点」。随后按账号或全局配置点击商品入口，OCR `market.price`；价格小于等于设定值点击 `market.buy`，再点击独立的 `market.confirm` 最终确认购买点，否则点击 `market.return` 返回并继续，按购买次数计数，不判定购买成功。04:00 后任务标记关闭且不补做次日。交易行配置包含启用状态、购买次数、商品备注、最高价与商品入口点击点；账号关闭独立设置时继承默认配置。

任务栏渲染 `已购买 N/M · <状态>`（`marketCompletedCount` / `marketTargetCount` / `marketStatus`）。后端一直在下发这三个字段，前端不渲染的话上调购买次数后任务栏毫无变化 -> 用户以为配置没生效。

当天的两个终态（`Completed` / `WindowClosed`）都只在 `completedCount >= purchaseCount` 时出栏，出栏条件对购买次数敏感：把次数从 1 调到 3，任务立刻回到任务栏继续买剩下 2 次，不需要一键恢复。planner 的 `market_purchase_due` 因此不再筛状态白名单，只看 `completedCount < purchaseCount`，与任务栏严格互补——planner 比任务栏严会出现「任务栏有任务、点继续却不执行」。例外：当天 `PriceRecognitionFailed` 若带未来 `priceRetryAtMs`，任务栏保留并把计划时间提到冷却点，planner 冷却期内跳过；冷却结束且窗口仍开则再到期。无冷却时间戳的旧存档视为可立刻重试。一键恢复清冷却并放回 `Pending`。`WindowClosed` 只在 `minute >= windowEndMinute` 时写入，而 `is_current` 蕴含窗口开着（`market_start_projections` 在 `minute >= end` 时只投影明天），所以能看见当天 `WindowClosed` 只有一种情形：窗口结束时间被人为延后，那正是应该继续买的场景。

当前账号必须跑满配置购买次数才切换下一账号。交易行窗口内制作台优先：点 `market.entry` 之前若已有到期制作立即 `YieldedForCraft`。循环内每个原子流程结束后只让位给入口后**新**到期的制作：`MarketDriver::latest_due_craft_at_ms()` 在已到期制作里取最晚计划时间，与入口点击、固定等待之后取的基线比较。取 `max` 而非 `min` 是因为循环内要回答「有没有新制作到期」，`min` 会被陈旧任务永久钉住。循环内拿全量到期集合让位会把仍排在交易行后面的队列任务当成理由，每买一件就退出换号，购买次数永远停在 1。让位时点击 `game.specialOps`（交易行与大厅共用特勤处入口）进入四制作台；同账号保持会话，跨账号才关游戏切号。让位后仍按 `market_retry_task` 排在最后一个已到期制作之后，按已保存次数续跑。

`market.price` OCR 或截图失败按本页未识别处理。连续三个商品页后本轮把该账号交易行插到剩余到期任务队尾重试一次（插在远期 lookahead 之前）。队尾补偿仍失败则写入 `priceRetryAtMs = now + 1h`，任务栏保留、`scheduledAtMs` 提到该时刻、planner 冷却期内跳过；冷却结束且窗口仍开则再开一轮，仍走「三页失败→队尾重试」，直到窗口结束。禁止升级成系统暂停。一键恢复清冷却并放回 `Pending`。旧存档只有 `PriceRecognitionFailed`、没有冷却时间戳的，视为冷却已过，允许立刻再跑。

试运行 command：`special_ops_start_limited_supply_trial`、`special_ops_start_market_trial`；两者都在运行中被 `ensure_no_active_special_ops_run` 拦截，`special_ops_recheck_limited_supply` 同样如此。交易行试运行支持 `inspectOnly` 和 `realSingleAttempt`，不写正式购买次数；限时商品颜色测试只双采样，不写正式周期结果。

`ammo.researchDepartment` 与 `limited.ready` 使用用户参考图模板；页面暴露 `limited.ready` 超时与「研发部门页面等待（ms）」两项配置，后者即 `limitedSupply.researchDelayMs`（0–60000，默认 3000），preflight 校验上限并在 runtime 作为识别页面前的固定等待。9 个识色区域测试显示双采样命中颜色与距离。颜色 1/2 使用原生 `input[type=color]`，可打开系统颜色面板使用吸管，也可输入 `#RRGGBB`；不保存截图。9 个 `limited.color.1`–`limited.color.9` 区域只用于正式 `AnyPixel` 识别与测试。`market.entry` 为模板识别与点击区域，命中后才点击入口；`market.price` 仅为 OCR 区域，`market.confirm` 为独立最终确认购买点击点。限时商品与交易行试运行正常结束后停放鼠标到 `runtime.mouseParking`，其他试运行不增加停放动作。

五类运行 command 的运行态通过 `special-ops://run-changed` 同时发送到主窗口与 `special-ops-operation` window；payload 只有带 `runKind` 的 `LoginRunSnapshot`，round 可附带 `roundProgress`，不得包含 QQ 密码。

当前应用自定义 commands 只通过 `generate_handler![]` 注册，尚未整体迁移到 Tauri app ACL。不得只为单个 command 创建局部 permission；这会生成 `default_permission: null` 的 `__app-acl__`，导致 `special_ops_get_bootstrap` 等未列入 allow 的既有 commands 被拒绝。
