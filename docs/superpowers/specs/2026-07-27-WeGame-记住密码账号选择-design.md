# WeGame 记住密码账号选择设计

## 状态

本设计取代登录流程中的“工具输入 QQ 与密码”方案，以及 `2026-07-26-WeGame-登录输入节流-design.md`。WeGame 密码框只接受物理键盘输入，工具改为使用 WeGame 已记住密码的账号列表。

## 目标

- 用户预先逐个登录目标 QQ，并在 WeGame 勾选“记住密码”。
- 工具每次强制关闭并重启 WeGame，从账号列表顶部查找目标 QQ。
- 账号列表动态排序时仍能按 QQ 精确选择目标账号。
- 选择后用剪贴板精确复核顶部账号，再提交一次登录。
- 工具不再保存、显示或输入 QQ 密码。

## 非目标

- 不绕过 WeGame 受保护密码输入。
- 不使用驱动级虚拟键盘、剪贴板粘贴密码或 UI 私有接口。
- 不按固定行号或固定滚轮次数绑定账号。
- 不恢复用户原剪贴板；复核后剪贴板保留当前 QQ。

## 前置条件

用户必须先在 WeGame 中成功登录每个目标 QQ，并勾选“记住密码”。账号均为纯数字且互不重复。执行前仍按现有规则强制结束游戏与 WeGame，再启动新的 WeGame 会话；完整重启后账号列表回到顶部。

## 配置模型与迁移

`AccountPlan` 删除 `password`。前端删除密码输入项，Rust/TypeScript 类型、默认值、校验、登录冻结配置和试运行签名同步删除密码依赖。

旧 `special_ops_settings.json` 中的 `password` 作为未知字段读取；用户下次保存配置时，序列化结果不再包含该字段，从而清除已有明文密码。导入、导出、模板复制及测试 fixture 不得重新生成密码字段。

QQ 账号必须去除首尾空白后为非空纯数字。目标账号未在 WeGame 记住密码列表中时，不尝试回退密码输入。

## 校准目标

保留：

- `wegame.loginMode`：切换到账号密码登录入口。
- `wegame.loginFormReady`：记住密码登录页已就绪。
- `wegame.login`：登录按钮。
- `wegame.gameEntry`：置顶游戏入口。
- `wegame.launch`：启动游戏按钮。

新增：

- `wegame.accountDropdown`：账号下拉按钮点击区域，守卫为 `wegame.loginFormReady`。
- `wegame.accountList`：展开后账号列表 OCR 区域。
- `wegame.selectedAccount`：顶部已选账号双击复制区域，守卫为 `wegame.loginFormReady`。

删除：

- `wegame.account`：旧账号输入区域。
- `wegame.password`：旧密码输入区域。

旧 `wegame.account` 的矩形可迁移为 `wegame.selectedAccount`，减少重复框选；旧 `wegame.password` 直接删除。`wegame.accountDropdown` 与 `wegame.accountList` 必须由用户重新校准。

## Windows OCR

Windows 内置 OCR adapter 只用于确认账号列表仍有可见内容，不承担目标 QQ 身份识别。OCR 输入来自现有屏幕截图能力，不依赖内置参考图或第三方模型。

账号列表每次识别连续采样两次，间隔 `400ms`。两次都返回至少一个数字片段即可确认列表仍处于展开状态；两次 OCR 文本允许不同，避免漏字、错字或分段变化阻塞行点击。账号身份只信选中后从顶部账号输入框复制的完整 QQ。

OCR 初始化失败、系统 OCR 不可用或截图失败时不点击，并全面暂停报告失败步骤。单轮双采样为空不是系统能力错误；执行器以 400ms 间隔重试，连续 3 轮为空时立即标记当前账号 `NeedsManualLogin`，跳过后继续下一账号，不重启 WeGame、不重复扫描。

## 动态列表扫描

1. 强制重启 WeGame，等待 `wegame.loginMode` 或 `wegame.loginFormReady`。
2. 必要时点击 `wegame.loginMode`。
3. 等待 `wegame.loginFormReady`，点击 `wegame.accountDropdown`。
4. 对 `wegame.accountList` 连续执行两次 OCR；两次均非空后确认列表已展开。
5. 将紧密框选的列表区域纵向分为 3 个可见槽位，从上到下依次点击。槽位只代表当前位置，不把具体账号绑定到固定行号。
6. 每次选中后复制顶部完整 QQ；命中目标立即结束扫描。
7. 非目标 QQ 加入已见集合，重新展开列表并继续下一槽位。列表重新展开后保持关闭前滚动位置。
8. 处理完本页 3 行后，将鼠标移入列表区域并向下滚动 3 格。
9. 新页继续逐行复制；整页 3 个完整 QQ 均已出现过时判定到达底部。
10. 到底仍未找到目标 → 标记账号“需要人工登录”，跳过并继续其他账号。

滚轮仅负责推动列表变化，不能作为成功条件。滚动和点击账号不得继续使用会被展开列表遮挡的 `wegame.loginFormReady` 模板作为动作守卫；账号列表打开状态只要求两次 OCR 均非空，不比较 OCR 文本。账号列表到底未找到目标或复制失败时不重启 WeGame，直接返回 `NeedsManualLogin`；未来多账号 round 据此标记并跳到下一账号。

## 账号选择与精确复核

点击账号项后列表自动关闭。工具随后：

1. 清空系统剪贴板，防止读取旧账号造成假成功。
2. 双击 `wegame.selectedAccount`。
3. 发送 `Ctrl+C`。
4. 在短期限内重试读取 Unicode 文本剪贴板。
5. 去除首尾空白，要求内容为纯数字且与目标 QQ 完全一致。

复核成功后才允许点击 `wegame.login`。剪贴板保留当前 QQ，不恢复原内容，UI 和操作提示需明确这一行为。

复制值与目标不同属于正常扫描。列表到底未找到目标或无法复制有效 QQ 时禁止点击登录，立即标记“需要人工登录”并记录目标 QQ 和失败步骤；不强制重启 WeGame，不重复扫描该账号。

## 登录提交与失败状态

每个 run 最多点击一次 `wegame.login`。点击后只等待 `wegame.gameEntry`，不返回账号选择步骤，不重复提交登录。

- 目标账号不在记住密码列表、复制复核连续失败 → `NeedsManualLogin`。
- 登录按钮点击后未进入游戏入口 → `LoginFailed`。
- OCR、截图、窗口或系统能力异常导致步骤超时 → 全面暂停。
- 紧急停止规则不变：立即释放输入，当前账号标记 `Uncertain`。

人工处理后，账号按保存数据重新加入后续轮次；不增加额外身份验证流程。

## 登录步骤调整

删除 `InputAccount`、`InputPassword`。新增可观测步骤：

- `OpenAccountList`
- `ScanRememberedAccounts`
- `SelectRememberedAccount`
- `VerifySelectedAccount`

后续 `SubmitLogin`、`WaitGameEntry`、`OpenGameEntry`、`WaitLaunchButton`、`LaunchGame` 和 `WaitGameWindow` 保持现有语义。

operation window 在扫描、滚动、点击、复制期间继续显示当前步骤。每次账号登录只显示一次 `3/2/1` 接管提醒：若需切换到账号密码入口，则该入口点击使用本次提醒；若登录表单已显示，则展开账号列表使用本次提醒。此后账号选择、复制、重新展开、滚动和登录提交均处于同一登录接管段，不重复倒计时。`wegame.gameEntry` 识别成功后结束接管段；后续点击游戏条目和启动按钮恢复逐动作倒计时。新账号开始时重新提醒一次。OCR 等待期间不新增接管提醒。

## 测试

- 配置迁移：旧密码字段可读取，保存后消失；前端类型和 UI 不再暴露密码。
- preflight：QQ 必须为纯数字；不再要求密码和两个输入区域。
- OCR adapter：纯数字片段提取、双采样均非空、失败与不可用错误。
- 列表扫描：首屏第二槽位命中、滚动后命中、动态排序、整页无新账号后判底。
- 选择复核：剪贴板清空、双击复制、精确匹配、非目标继续下一槽位、到底后直接返回 `NeedsManualLogin`。
- 接管提醒：登录入口到 `wegame.gameEntry` 只倒计时一次；新账号重新倒计时；游戏条目与启动按钮恢复逐动作倒计时。
- 登录提交：复核前禁止点击；成功复核后只提交一次；超时不返回选择步骤。
- 紧急停止：扫描、滚动、复制各阶段均可取消并释放输入。
- 运行 `codegraph sync` 与 `bun run check` 全量质量门禁。
