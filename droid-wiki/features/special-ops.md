# 特勤处自动化（开发中）

`special_ops` 保存账号级制作台、子弹兑换和调度状态。每个账号包含 4 台制作台；同一账号的到期制作任务聚合处理。每日兑换时间按 `Asia/Shanghai` 的 `HH:mm` 解释。

## 子弹兑换配置

每个账号保存独立的有序子弹目标。单项目包含名称、启用状态、普通/赛季限定、相对上一目标的滚轮步数、当天成功日期和当天重试次数。UI 支持逐项新增、上移、下移和删除；调度按 `order` 依次执行。模板只能复制目标选择、类型、顺序和滚轮步数，不能复制当天成功或重试状态。

## 区域校准

校准结果全局共享，不随账号或 Profile 复制。UI 不要求用户填写环境名称、显示器、分辨率、DPI 或窗口模式，只维护一套当前校准结果。旧版本存在多套环境时，加载后保留当时选中的一套。

静态 UI 的 `recognitionRegion` 使用模板匹配，由用户选择一张本地参考图片，路径随校准目标保存到 `special_ops_settings.json`。动态 WeGame ID 与已选子弹名称使用 OCR，不上传静态参考图；OCR 结果分别与账号 `wegameId`、当前 `AmmoTarget.name` 比对。点击点与输入区域不保存参考图。用户可替换或清除图片；游戏 UI 更新后应重新上传当前版本样本。区域坐标定义截图范围，参考图定义匹配目标，两者缺一时不得启动模板识别步骤。图片文件被移动或删除时路径失效，后续执行器必须报告缺失并暂停对应步骤。

框选行为沿用摩斯区域框选交互：在单个显示器打开全屏透明 overlay，主窗口保持存在；按住左键拖拽，松开后立即提交并关闭。区域过小时要求重新框选，Esc、右键或 Alt+F4 取消。overlay 30 秒未关闭时由 native 侧自动销毁，避免前端异常时持续占用键鼠。提交、取消、超时或窗口异常关闭后恢复主窗口焦点。点击动作执行时使用所选矩形中心。

创建入口必须使用 async Tauri command，避免在当前 WebView IPC callback 内同步创建第二个 WebView2 导致重入阻塞。校准窗口先按默认尺寸加载页面，页面完成后再切换为单显示器全屏；前端使用与摩斯框选一致的 Mouse Events 处理拖拽。

工作台通过 `special_ops_begin_calibration_selection` 打开框选窗口。提交调用 `special_ops_submit_calibration_selection`，取消调用 `special_ops_cancel_calibration_selection`。窗口 label 使用 `special-ops-calibration-*`，由 `overlays.json` 授权。

制作台入口、进入制作列表点击点、制作列表就绪状态、置顶配方点击点、空闲中文字区域和可收取感叹号均按技术中心、工作台、制药台和防具台保存 4 个独立区域，不使用通用位置。旧 `craft.station`、`craft.recipe`、`craft.idle` 与 `craft.claimReady` 区域加载时删除，禁止把单个旧坐标错误复制到四台。点击烽火地带后增加 `game.activityPopup` 识别区域；命中时执行一次空格，未命中则继续原流程。

`game.modeReady` 使用用户上传的模板图判定模式选择已可操作。没有识别样本前，该步骤不能进入真实执行器；不得仅以固定延时判定成功。

## 判定与动作守卫

执行器必须将模板匹配或 OCR 连续两次一致作为成功，采样间隔约 300–500ms；结果不一致时重新采样，不点击、不输入、不更新持久化状态。可见按钮自身使用 `recognitionRegion`，只有自身模板命中后才点击。不能依靠按钮自身判断的固定动作使用以下守卫：

| 动作 | 前置守卫 | 后置判定 |
|---|---|---|
| 切换到账号密码登录 | `wegame.loginFormReady` 未命中且登录入口自身模板命中 | `wegame.loginFormReady`；已命中时跳过该点击 |
| 输入 QQ 账号、密码 | `wegame.loginFormReady` | 登录按钮仍可识别；输入完成后才允许提交 |
| 提交 WeGame 登录 | 登录按钮自身模板 | 提交后先 native 最大化 WeGame，再由 `wegame.loggedIn`、`wegame.humanVerification`、`wegame.loginFailed` 三选一 |
| 点击头像 | 头像自身模板 | `wegame.avatarMenuReady` |
| 切换账号 | 切换账号入口自身模板 | `wegame.loginFormReady` |
| 打开游戏启动前置页 | 入口自身模板 | `wegame.launchPageReady` |
| 点击启动游戏 | 启动按钮自身模板 | native 游戏窗口/进程出现，之后进入 `game.modeReady` |
| 点击烽火地带 | 入口自身模板 | 可选 `game.activityPopup` 或 `game.startGame` |
| 关闭活动弹窗 | `game.activityPopup` | 按一次空格后等待 `game.startGame` |
| 切换大厅视角 | `game.startGame` | 按一次 Tab；制作分支等待 `game.specialOps`，仅兑换分支等待 `ammo.department` |
| 进入特勤处 | `game.specialOps` 自身模板 | `game.stationGrid` |
| 点击制作台 | 对应 `craft.claimReady.*` | `craft.reward`；奖励页识别因网络延迟漏采后允许以对应 `craft.recipeListReady.*` 确认已收取；连续 3 次仍稳定命中感叹号则隔离账号 |
| 关闭制作奖励页 | `craft.reward` | 按一次空格后等待对应 `craft.idle.*` |
| 进入制作列表 | 对应 `craft.idle.*` | 对应 `craft.recipeListReady.*` |
| 点击置顶配方 | 对应 `craft.recipeListReady.*` | `craft.fill` 或 `craft.produce` |
| 点击制作一键补齐 | `craft.fill` 自身模板 | `craft.purchase` |
| 购买制作材料 | `craft.purchase` 按钮自身模板 | `craft.produce`；仍为补齐状态则按价格波动规则重试 |
| 开始制作 | `craft.produce` 按钮自身模板 | `craft.abort` |
| 返回部门页 | `ammo.department` 已命中时跳过；否则仅在 `game.stationGrid` 或 `craft.abort` 命中时按一次 Tab | `ammo.department` |
| 点击部门 | `ammo.department` 自身模板 | `ammo.supply` |
| 点击军需处 | `ammo.supply` 自身模板 | `ammo.tactical` |
| 点击战术部门 | `ammo.tactical` 自身模板 | `ammo.list` |
| 切换赛季限定列表 | `ammo.seasonal` 自身模板 | `ammo.seasonalList` |
| 滚动到目标子弹 | `ammo.list` 或 `ammo.seasonalList` | 每次滚轮动作前列表仍需连续两次命中；滚动完成后再次确认列表，才允许点击 |
| 点击目标子弹 | `ammo.list` 或 `ammo.seasonalList` | OCR `ammo.selectedTargetName` 必须等于当前目标名称，并出现 `ammo.fill` 或 `ammo.exchange` |
| 点击子弹一键补齐 | `ammo.fill` 自身模板 | `ammo.purchase` |
| 购买子弹材料 | `ammo.purchase` 按钮自身模板 | `ammo.exchange`；失败按既定重试/隔离规则处理 |
| 兑换子弹 | `ammo.exchange` 按钮自身模板 | `ammo.success` 灰色状态 |

每个裸动作的允许前置状态保存于校准目标 `guardAnyOf`，其语义为 OR。执行器必须先确认其中至少一个守卫连续两次命中，才能使用该动作坐标。`default_click_and_input_targets_have_recognition_guards` 测试枚举所有 `clickPoint` 与 `inputRegion`；新增裸动作却未登记有效守卫时测试必须失败。窗口最大化、最小化恢复、进程启动、窗口崩溃使用 native 窗口/进程状态，不以截图模板替代。

制作或子弹补齐购买最多重试 3 次，每次间隔 1 秒并重新识别。重试后仍未进入 `craft.produce` 或 `ammo.exchange`，且补齐/购买状态仍存在时，不依赖短暂的仓库公告文本；直接将账号标记为需人工处理，结束该账号剩余流程并切换下一账号。

用户点击“继续”或在未暂停状态启用功能时执行 preflight。preflight 只检查存在正常启用账号且至少配置一项制作台或子弹目标的流程；按实际启用制作台、普通子弹、赛季限定子弹计算必需校准项。账号缺 QQ、密码或 WeGame ID，启用制作台缺物品/有效时长，启用子弹缺名称，或任一必需校准项缺少矩形、模板图路径、模板图文件时，均拒绝启动并报告首个缺失步骤。暂停状态允许保存不完整草稿，避免配置过程中反复报错。

## WeGame 窗口归一化

提交 QQ 账号密码后，执行器必须立即获取 WeGame 窗口句柄并将窗口最大化，再判定登录成功、人工验证或登录失败。登录成功后直接 OCR WeGame 主界面 ID，不再进入个人主页；只有切换账号时才打开头像菜单。所有登录后的 WeGame 固定坐标与识别区域均以最大化窗口为前置条件，禁止在普通窗口尺寸下继续复用坐标。窗口已最大化时该操作应幂等；最大化失败时标记当前账号为“窗口异常”并跳过，不继续点击后续区域。

## 当前边界

已实现配置持久化、调度、暂停、账号/制作台/有序子弹配置和区域框选。WeGame 登录、参考图/OCR/识色判定、键鼠执行、游戏崩溃恢复尚未实现，不能视为自动化完成。
