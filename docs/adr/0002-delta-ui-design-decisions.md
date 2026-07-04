# ADR-0002: 三角洲行动 API 工具 UI 设计决策

## 状态

已接受

## 上下文

三角洲行动 API 工具（代码前缀 `delta_`）需要将现有 Rust 后端 43 个 Tauri API 命令暴露给用户。这些命令涵盖 5 种鉴权流程（QQ /
微信 / QQSafe / Wegame QQ / Wegame 微信）+ 1 种先遣服鉴权、11 个游戏数据查询 API、2 个 Wegame 运营操作、2 个 QQSafe 安全查询和
1 个先遣服查询。需确定页面拆分、交互流程、组件架构、状态管理和边界条件处理。

核心约束：

- 所有 API 通过 Tauri IPC（`invoke`）调用，非 HTTP 直连
- 返回值分两种模式：`ApiResponse<T>`（code=0 成功，code≠0 失败 + msg）和 `Result<T, String>`（DeltaError 序列化为错误字符串）
- 令牌有过期时间（`expiresAt`），刷新依赖 cookie 有效性
- 不同账号类型解锁完全不同的功能域，能力互不重叠
- 游戏数据 API 需要鉴权（`GameAuth`），但物品/枪械/物价等查询不需要
- IDE 网关可能有频率限制，需控制并发请求数

## 决策

### 1. 侧边栏入口拆分

**决策：** 3 个独立入口——账号管理（delta-accounts）/ 游戏数据（delta-game）/ 工具箱（delta-toolbox）。

**原因：**

- 账号管理是独立关注点（CRUD + 令牌生命周期），与数据展示逻辑正交
- 游戏数据体量最大（7 个无参数仪表盘 API + 6 个参数化查询），独占一页避免混合布局
- QQSafe（2 个操作）、Wegame（2 个操作）、先遣服（1 个操作）功能轻量，合入工具箱避免侧边栏膨胀

**拒绝方案：**

- 单入口：功能域差异大（鉴权 vs 查询 vs 运营），单页无法容纳，且需 Tab 切换增加导航深度
- 5 入口（账号/数据/Wegame/QQSafe/先遣服）：QQSafe 和先遣服各仅 1-2 个操作，不值得独立入口，侧边栏过度膨胀（从 6→8 项，6
  个现有 + 5 个新增 = 11 个，不可接受）
- 2 入口（账号+数据/Wegame+QQSafe+先遣服合并）：数据页和工具箱的账号依赖类型不同（QQ/微信 vs
  Wegame/QQSafe/Pioneer），合并后账号选择器逻辑混乱

**边界条件：**

- 侧边栏 `tools` 数组新增 3 项，`ToolId` 类型自动扩展
- 入口顺序：morse → timer → rapidfire → delta-accounts → delta-game → delta-toolbox（功能型在前，数据型在后）
- `App.tsx` 中的 ternary 链需扩展为 6 路分支（现有 3 路 + 新增 3 路），考虑重构为 map 查找表
- 本工具不需要 overlay 模式、透明窗口或热键——纯 API 调用层，不影响 `?mode=` 参数分支

### 2. 页面布局与动态渲染

**决策：** 选中账号后按账号能力动态渲染可用区域，不可用区域不渲染（非灰显/禁用）。

**原因：**

- 最简洁——用户只看到当前账号能做的事，不被无关内容干扰
- 账号是天然筛选器，切换账号自然看到不同面板
- 与现有工具的"bootstrap → 渲染"模式一致（Morse/Timer/Rapidfire 都是根据状态动态渲染）

**拒绝方案：**

- 固定布局 + 禁用灰显：浪费垂直空间，增加视觉噪音（"此功能需要 Wegame 账号"提示卡片占空间但无功能）
- Tab 域切换：多一层导航，且只有一个 Tab 是激活的，其余灰显——本质上等价于动态渲染 + 额外点击

**边界条件：**

- 无账号选中时：数据页/工具箱页渲染空态引导（Empty 组件），文案"请先在账号管理中添加对应类型的账号"，附"前往账号管理"链接
- 选中账号类型不匹配当前页：数据页选了 Wegame 账号 → 页面提示"当前账号为 Wegame 类型，无法查询游戏数据，请选择 QQ
  或微信账号"；工具箱页选了 QQ 账号 → 提示"当前账号为 QQ 类型，无法使用工具箱功能，请选择 Wegame/QQSafe/先遣服账号"
- 多个同类型账号：账号选择器列出所有同类型账号供切换，选中态高亮
- Pioneer 不在 `AccountKind` 枚举中（当前 5 种：qq/wechat/qqsafe/wegame_qq/wegame_wechat），Pioneer 的登录走 QQ 流程但产出
  `key` 而非 `openid+accessToken`——需在前端判定 Pioneer 账号（通过 `extraJson` 中存储 `"pioneer"` 标记或扩展
  `AccountKind`）。**此为待决项**，需在 Phase 1 实现 `delta-types.ts` 时确定方案
- 账号能力查询使用纯前端函数 `getAccountCapabilities(kind: AccountKind): Capability[]`，不依赖后端接口

### 3. 账号选中态

**决策：** 应用级全局状态（React Context），三页共享，切换页面保持选中。

**原因：**

- 账号是跨页面概念——数据页和工具箱页都依赖它
- 用户从账号管理页添加账号后，切到数据页应立即看到新账号可选
- 每页独立选择会导致：用户在账号页添加 QQ 账号 → 切到数据页 → 数据页不知道新账号存在 → 需手动刷新

**拒绝方案：**

- 每页独立选中：切换页面需重新选择账号，增加操作步骤；页面间账号列表不同步问题
- URL 参数传递选中态：本工具不用路由（沿用 `?mode=` 参数模式），无 URL 路径可编码

**边界条件：**

- `selectedAccountId` 类型为 `number | null`，`null` 表示无选中
- 选中账号被删除时：`selectedAccountId` 重置为 `null`，当前页面显示空态
- 选中账号令牌过期时：不自动取消选中，令牌状态通过 `TokenBadge` 展示，自动刷新逻辑在 API 调用层处理
- 首次进入应用无账号：`selectedAccountId = null`，账号管理页正常显示（无选中态不影响账号管理页功能）
- Context 刷新时机：`delta_list_accounts` 在应用启动时调用一次 + 添加/删除账号后手动调用 `refreshAccounts()`
- Context 不缓存 API 返回数据——游戏数据查询结果由各页面组件自行管理，Context 只管账号列表和选中态
- Tauri WebView 重载时 Context 丢失：需在 `App.tsx` 的 `useEffect` 中重新 `invoke("delta_list_accounts")` 恢复

### 4. 登录交互

**决策：** Dialog 模态流程——选类型 → 二维码 → 轮询 → 令牌。

**原因：**

- 登录是偶发操作（添加一次长期使用），Dialog 不打断主面板布局
- 步骤少（1-3 步），关闭即回归数据视图
- 与现有工具的"设置保存后自动刷新"模式不同——登录是异步多步骤流程，需要隔离的交互空间

**拒绝方案：**

- 右侧面板流：占据数据展示空间，登录完成后需清空面板再渲染数据，过渡生硬
- 独立页面：步骤太少（3步）不值得整页切换；且需回退导航，增加复杂度

**详细流程（6 种账号类型，3 种模式）：**

**模式 A — QQ 扫码（QQ / QQSafe / WegameQQ / Pioneer 共享结构）**：

```
Step 1: 选择类型（6选1）→ 记录 kind
Step 2: invoke("delta_{kind}_get_login_qr")
  → 成功: { qrSig, image(base64), token, loginSig, cookie }
  → 失败: 显示错误 + "重试"按钮
Step 3: 显示二维码 + 开始轮询
  invoke("delta_{kind}_poll_login_status", { qrToken, qrSig, loginSig, cookie })
  轮询间隔: 2秒
  状态映射:
    code=0  → 登录成功 → 进入 Step 4
    code=1  → 等待扫描 → 继续轮询
    code=2  → 已扫描等确认 → 继续轮询（UI 切换为"已扫描，请在手机上确认"）
    code=-2 → 二维码过期 → 停止轮询，显示"刷新二维码"按钮
    code=-3 → 用户拒绝 → 停止轮询，显示"登录被拒绝"+ "重新扫码"按钮
    code=-4 → 错误 → 停止轮询，显示错误消息
Step 4: 获取访问令牌
  QQ:       invoke("delta_qq_get_access_token", { cookie })
  QQSafe:   invoke("delta_qqsafe_get_access_token", { cookie })
  WegameQQ: invoke("delta_wegame_qq_get_access_token", { cookie })
  Pioneer:  invoke("delta_pioneer_get_access_token", { cookie })
  → 成功: AccountBoundAccess<T> → 关闭 Dialog, 刷新账号列表
  → 失败: 显示错误 + "重试" / "重新扫码" 按钮
```

**模式 B — 微信扫码（WeChat / WegameWeChat）**：

```
Step 1: 选择类型
Step 2: invoke("delta_{kind}_get_login_qr")
  → 成功: { qrCode(微信扫码URL), uuid }
  → 失败: 同上
Step 3: 显示二维码（微信扫码 URL 渲染为 QR 图片）+ 轮询
  invoke("delta_wechat_poll_status", { uuid })  或
  invoke("delta_wegame_wechat_poll_status", { uuid })
  状态映射:
    code=1  → 等待扫描
    code=2  → 已扫描
    code=3  → 登录成功 → wxErrcode + wxCode
    code=-2 → 超时
    code=-3 → 拒绝
    code=-4 → 错误
Step 4: 获取访问令牌
  WeChat:       invoke("delta_wechat_get_access_token", { code: wxCode })
  WegameWeChat: invoke("delta_wegame_wechat_get_access_token", { code: wxCode })
  → 成功: AccountBoundAccess<WechatAccessToken> 或 AccountBoundAccess<WegameTicket>
```

**边界条件：**

- Dialog 关闭（Esc / X / 点击遮罩）：立即停止轮询定时器，不发送取消请求给后端（Rust 侧轮询是无状态的——每次调用是独立 HTTP
  请求，停止前端轮询即可）
- 二维码过期后点击"刷新二维码"：重新调用 `get_login_qr`，覆盖旧 QR 数据，重启轮询
- 轮询超时上限：120秒（60次 × 2秒），超时后提示"二维码已过期"+ 刷新按钮
- 网络错误：轮询请求失败（非状态码错误而是 `invoke` 抛异常）→ 重试最多 3 次后停止，显示"网络异常"
- 同一类型重复添加：不阻止，SQLite 允许同类型多账号（`uin_or_openid` 区分）
- QQ/QQSafe/Pioneer 登录二维码 30 秒有效期（服务端限制），前端倒计时显示
- 微信二维码 URL 有效期约 5 分钟，前端轮询 2 秒间隔 × 150 次 = 300 秒上限
- Pioneer 的 `get_access_token` 返回 `ApiResponse<Value>` 而非 `AccountBoundAccess`——数据结构为
  `{ code: 0, data: { key: "..." } }`，前端需特殊处理
- WegameWeChat 的 `get_access_token` 返回 `AccountBoundAccess<WegameTicket>`——`auth` 字段为 `{ id, ticket }` 而非
  `{ openid, accessToken, expiresIn }`
- `CommandOptions.insecureSkipTlsVerify` 始终传 `undefined`（默认 false），除非用户有特殊网络环境需求——Phase 1 不暴露此选项

### 5. 数据加载策略

**决策：** 分批加载——首批 player+record 核心摘要，次批异步拉取详情。

**原因：**

- 即时反馈——用户切换账号后立即看到关键信息（等级、战绩摘要）
- 避免瞬间 7+ 个请求冲击 IDE 网关（`comm.ams.game.qq.com/ide/`）
- player+record 数据量小，是用户最高频关注的数据

**详细策略：**

```
选中账号 →
  ├─ 首批（并行，2 个请求）:
  │   invoke("delta_game_get_player", { auth })   → SignalTile + 角色卡片
  │   invoke("delta_game_get_record", { auth })    → SignalTile + 战绩卡片
  │   首批完成后 → 渲染 MacroHeader stats + 两个卡片
  │
  └─ 次批（并行，5 个请求）:
      invoke("delta_game_get_assets", { auth })
      invoke("delta_game_get_recent", { auth })
      invoke("delta_game_get_achievement", { auth })
      invoke("delta_game_get_password", { auth })
      invoke("delta_game_get_bind", { auth })
      次批逐个完成后 → 逐个渲染对应卡片（skeleton → 数据）
```

**边界条件：**

- 首批失败：如果 player 和 record 都失败，整个仪表盘显示错误态，不触发次批
- 首批部分失败：player 成功但 record 失败 → 已成功的正常渲染，失败的卡片显示错误
- 次批失败：单个 API 失败不影响其他卡片，失败卡片显示错误 + "重试"按钮
- 切换账号：取消正在进行的请求（通过 AbortController 或请求标记），重新触发加载
    - 注意：Tauri `invoke` 不支持 AbortController——需使用"版本号"模式：每次切换账号递增 `loadVersion`，请求回调中检查
      `loadVersion` 是否仍为当前值，过期的回调丢弃
- 同一账号二次加载：用户从工具箱切回数据页，如果数据仍在内存中（组件未卸载），不重新加载。通过 `useRef` 缓存上次加载的
  `accountId`，相同则跳过
- 首次进入数据页无选中账号：不发起任何请求，显示空态引导
- 选中 Wegame/QQSafe/Pioneer 账号：数据页不发起任何请求，提示切换到 QQ/微信账号
- `GameAuth.acctype` 构造规则：`kind === "wechat" ? "wx" : "qc"`——WegameWeChat 不会出现在数据页（被能力矩阵过滤），所以无需处理
  `"wx"` 以外的 acctype
- 网络超时：单个请求 10 秒超时（Tauri IPC 默认无超时，需前端 `Promise.race` + `setTimeout` 实现）
- API 返回 `code !== 0`：视为请求失败，卡片显示 `{ msg }` 错误信息

### 6. 参数化查询

**决策：** 统一查询工作台——下拉选 API 类型 → 动态表单 → 结果展示。

**原因：**

- 6 个参数化 API 不需同时展示，下拉切换最省空间
- 线性流程（选 API → 填参 → 查询 → 看结果）清晰
- 与仪表盘的无参数卡片视觉区分明确——工作台是"主动查询"，仪表盘是"自动展示"

**详细设计：**

| API                   | 参数                                     | 验证规则                        | 结果展示   | 分页 |
|-----------------------|----------------------------------------|-----------------------------|--------|----|
| 物品查询 items            | typeId (必填), subType (必填), itemId (选填) | typeId/subType 正整数          | 物品卡片列表 | 无  |
| 物价查询 price            | args (必填, i64 数组), withRecent (开关)     | args 非空数组                   | 价格表格   | 无  |
| 枪械详情 guns             | gunId (必填)                             | gunId 非空字符串                 | 枪械属性面板 | 无  |
| 操作日志 logs             | auth (自动填充), logType (必填), page (默认 1) | logType 正整数, page ≥ 1       | 日志列表   | 有  |
| 改装方案 firearm_mod_list | page (默认 1), pageSize (默认 20)          | page ≥ 1, 1 ≤ pageSize ≤ 50 | 方案列表   | 有  |
| 地图推荐 recommendation   | place (必填)                             | place 非空                    | 推荐装备列表 | 无  |

**边界条件：**

- 切换 API 类型：清空当前表单和结果，重置为默认值
- 表单验证失败：查询按钮禁用，字段下方显示 `FieldError` 提示
- 未选中账号时查询 logs：logs 需要 auth，如果当前无 QQ/微信账号选中，logs 选项在下拉中不可选（灰显 + tooltip "需要选择账号"）
- items/guns/price/config/firearm_mod_list/recommendation 不需要 auth——即使无账号也可查询
- 分页请求：点击"下一页"时只更新 `page` 参数，不清空已展示的其他卡片数据
- gunId 输入：用户不一定知道 gunId 的值——需在 Phase 4+ 提供枪械 ID 搜索/选择器（暂用纯文本输入）
- args 输入（物价查询）：多个 itemId 用逗号分隔输入 → 前端 `split(",").map(Number)` → 验证每个值为正整数
- 查询结果为空：显示"未找到相关数据"空态
- 查询结果 `code !== 0`：显示 `{ msg }` 错误信息
- 并发查询：同一时间只允许一个查询请求（按钮 loading 状态），防止重复提交

### 7. 令牌过期处理

**决策：** 自动刷新 + 回退提示。

**原因：**

- 最优体验——成功时用户无感知
- 刷新失败时明确告知原因和下一步（重新扫码）
- 与"账号管理页管令牌生命周期"职责一致

**详细流程：**

```
API 调用 → 失败
  ├─ 检测到令牌过期（Rust 返回特定错误码或前端判断 expiresAt < now）
  ├─ 自动调用 update_access_token
  │   ├─ 刷新成功 → 更新 Context 中账号的 accessToken + expiresAt → 重试原请求
  │   └─ 刷新失败（cookie 也过期）→ 标记账号 tokenStatus = "need_relogin"
  │       ├─ 当前页面 Toast 提示"令牌刷新失败，请重新登录"
  │       └─ 账号卡片显示红点 + "需重新登录" 标签
  └─ 非令牌过期错误 → 正常展示错误信息，不触发刷新
```

**边界条件：**

- 如何检测令牌过期：Rust 后端返回的 `ApiResponse { code: -1, msg: "..." }` 中 msg 包含过期信息——前端通过
  `msg.includes("过期") || msg.includes("expired") || msg.includes("token")` 检测。如果无法从 msg 可靠判断，退回到前端
  `expiresAt < Date.now()` 预检
- 刷新竞态：两个 API 同时失败触发两次刷新 → 第二次刷新应等待第一次完成（通过 `Promise` 去重：同一账号的刷新请求只发一次）
- 刷新成功后 Context 更新：`refreshToken()` 调用 `delta_qq_update_access_token` → 成功后 `refreshAccounts()` → Context 重新
  `invoke("delta_list_accounts")` → 所有订阅组件重新渲染
- Wegame 账号无 `update_access_token` 命令——Wegame 的 `tgpTicket` 过期后只能重新扫码登录
- Pioneer 有 `delta_pioneer_update_access_token`——但依赖 cookie 有效性，同 QQ
- QQSafe 无 `update_access_token` 命令——只能重新扫码
- 微信有 `delta_wechat_update_access_token`——依赖 refreshToken（存储在 `extraJson` 中），如果 refreshToken 也过期则无法刷新
- `expiresAt` 为 `null` 的账号：Wegame/Pioneer 账号可能没有过期时间字段，无法预判过期，只能在 API 调用失败时后置检测
- 自动刷新不应在账号管理页的令牌状态展示中触发——账号管理页只做展示和手动刷新按钮

### 8. Wegame 操作

**决策：** 单卡片双操作——一个 TacticalCard 内含两个按钮（开箱/抽卡）。

**原因：**

- 操作简单、结果轻量，一个卡片足够
- 两个独立卡片浪费垂直空间

**详细交互：**

```
TacticalCard "Wegame 运营"
├── 领取保险箱礼包
│   ├── Button "领取" → invoke("delta_wegame_open_treasure_gift", { ticket })
│   ├── 成功: 卡片内展示奖励物品列表（名称 + 图标 + 稀有度 Badge）
│   ├── 失败(code≠0): 展示错误 msg
│   └── loading: 按钮禁用 + Spinner
│
└── 每日抽卡
    ├── Button "抽卡" → invoke("delta_wegame_draw_daily_card", { ticket })
    ├── 成功: 展示抽卡结果（角色 + 阵营 + 颜色）
    ├── 失败: 同上
    └── loading: 同上
```

**边界条件：**

- `WegameTicket` 构造：`{ id: account.uinOrOpenid, ticket: account.accessToken }`——Wegame 账号的 `accessToken` 字段实际存储的是
  `tgpTicket`
- 每日限制：开箱/抽卡每日一次，重复调用后端会返回错误（如"今日已领取"）→ 前端直接展示后端返回的 msg
- 按钮状态追踪：不本地记录"今日已操作"——状态由后端判断，前端每次点击都调用 API
- 两个按钮并发：允许同时点击开箱和抽卡（两个独立 API），各自有独立 loading 状态
- Wegame 账号的 `tgpTicket` 可能过期：操作失败时触发令牌过期检测流程（同决策 7），但 Wegame 无刷新命令 → 直接提示"请重新登录
  Wegame 账号"
- 选中非 Wegame 账号时：整个卡片不渲染

### 9. QQSafe 操作

**决策：** 封禁记录为主 + 举报折叠。

**原因：**

- 封禁查询是主要功能，自动展示
- 举报虽已弃用但可能仍有需求，折叠不占空间不丢失功能

**详细交互：**

```
TacticalCard "QQSafe 安全查询"
├── 封禁记录（默认展开）
│   ├── 选中账号后自动查询
│   ├── invoke("delta_qqsafe_get_banned_list", { openid, accessToken, code })
│   ├── code 来源: account.extraJson 中 JWT 解码后的 code 字段
│   ├── 成功: 显示封禁记录列表
│   │   ├── 无记录: "暂无封禁记录"
│   │   └── 有记录: 列表展示（游戏名 + 封禁类型 + 时长 + 原因）
│   └── 失败: 错误信息 + "重试"按钮
│
└── 游戏报告（折叠区，默认收起）
    ├── 折叠标题带 Badge "已弃用"（灰色）
    ├── 展开后: userId 输入框 + "查询"按钮
    ├── invoke("delta_qqsafe_report", { openid, accessToken, userId })
    ├── 成功: 展示信用分 + 处罚信息 + 游戏列表 + 设备信息
    └── 失败: 错误信息
```

**边界条件：**

- `code` 参数构造：QQSafe 登录后 JWT 存储在 `account.extraJson`，前端需 base64 解码 JWT payload 提取 `code` 字段。JWT 格式：
  `header.payload.signature`，取 payload 部分 `atob(base64UrlDecode(payload))` → JSON.parse → `.code`
- `extraJson` 为 null：无法构造 code 参数 → 封禁记录查询不可用，卡片显示"令牌信息不完整，请重新登录"
- userId 验证：QQ 号格式（5-11 位数字），前端输入时限制只能输入数字
- 举报 API 返回 `ApiResponse<Value>`（非结构化）→ 前端需动态渲染 JSON 树或使用代码块展示原始数据
- 封禁记录 API 也返回 `ApiResponse<Value>` → 同上，Phase 1 先用原始 JSON 展示，Phase 2+ 可做结构化
- QQSafe 无 `update_access_token` → 令牌过期只能重新登录
- 选中非 QQSafe 账号时：整个卡片不渲染

### 10. 先遣服操作

**决策：** 测试游戏列表卡片。

**详细交互：**

```
TacticalCard "先遣服测试"
├── 选中 Pioneer 账号后自动查询
├── invoke("delta_pioneer_get_game_test_list", { key: account.accessToken, listType: "pc" })
├── listType 切换: PC / 手机 两个按钮
├── 成功: 游戏列表
│   ├── 无测试: "当前无先遣服测试"
│   └── 有测试: 列表展示（游戏名 + 简介 + 封面 + 测试日期 + 参与人数）
└── 失败: 错误信息 + "重试"
```

**边界条件：**

- Pioneer 账号标识：Pioneer 的 `kind` 在 SQLite 中仍是 `"qq"`（因为走 QQ 登录流程），需通过 `extraJson` 中存储 `"pioneer"`
  标记或新增 `AccountKind::Pioneer` 变体区分。**此为待决项**
- `key` 来源：Pioneer `get_access_token` 返回 `{ key: "..." }`，前端需在登录成功后将 `key` 存入 `account.accessToken` 或
  `account.extraJson`
- `listType` 参数：Rust 侧接受 `Option<String>`，默认 `"pc"`；前端提供 `"pc"` / `"mobile"` 切换按钮
- 先遣服有 `update_access_token` → 令牌过期可尝试刷新
- 选中非 Pioneer 账号时：整个卡片不渲染
- Pioneer 账号同时是 QQ 账号：如果 `kind = "qq"` 且 `extraJson` 包含 pioneer 标记，则同时拥有"游戏数据"能力和"先遣服"
  能力——需在前端能力判定中特殊处理

### 11. 账号列表信息密度

**决策：** 两行小卡片——上行类型 badge + UIN，下行令牌状态 + 能力标签。

**原因：**

- 信息足够判断状态和能力
- 账号数量少（1-5 个），紧凑不浪费空间
- 快捷操作放在右键菜单或 ContextMenu，不挤占列表

**边界条件：**

- 令牌状态判定逻辑：
    - `expiresAt === null` → "无过期时间"（灰色点）——Wegame/Pioneer 账号常见
    - `expiresAt > now + 3天` → "有效"（绿色点）
    - `now < expiresAt ≤ now + 3天` → "即将过期 N天"（黄色点）
    - `expiresAt ≤ now` → "已过期"（红色点）
    - `accessToken === null` → "无令牌"（灰色点 + 虚线边框）
- 能力标签映射：
    - `"qq"` → ["游戏数据"]
    - `"wechat"` → ["游戏数据"]
    - `"qqsafe"` → ["QQSafe"]
    - `"wegame_qq"` / `"wegame_wechat"` → ["Wegame"]
    - `"qq"` + `extraJson.includes("pioneer")` → ["游戏数据", "先遣服"]
- 点击选中：高亮边框（TacticalCard `active` 属性）
- 删除操作：ContextMenu 弹出"删除账号"选项 → 确认 Dialog → `invoke("delta_delete_account", { accountId: id })`
- 刷新令牌操作：ContextMenu 弹出"刷新令牌"选项 → 仅在有 `update_access_token` 的账号类型上显示（QQ / 微信 /
  Pioneer）；QQSafe / Wegame 不显示此选项
- UIN/OpenID 显示：`account.uinOrOpenid`——QQ 系为 QQ 号（数字），微信系为 openid（长字符串截断显示前 8 位 + "..."）
- 令牌状态自动更新：令牌状态不设定时器轮询，仅在以下时机刷新：
    1. 进入账号管理页时
    2. 添加/删除/刷新账号后
    3. 其他页面自动刷新成功后触发 `refreshAccounts()`

## 后果

### 正面

- 与现有工具入口风格一致，用户学习成本低
- 全局账号选中态避免重复选择，跨页面体验流畅
- 动态渲染保持页面简洁，无无关内容噪音
- 分批加载平衡即时反馈与网关压力
- Dialog 登录不中断主面板数据展示

### 负面

- 全局账号 Context 需跨页面同步——令牌刷新后需通知其他页面的进行中请求
- 三页共享状态——任一页面的令牌刷新需触发 Context 更新，所有订阅组件重渲染
- 登录 Dialog 需支持 6 种流程（3 种模式 × 2+ 变体），组件复杂度较高
- Pioneer 账号类型不在 Rust `AccountKind` 枚举中，需前端特殊判定——可能导致与后端数据模型不一致
- 游戏 API 返回 `ApiResponse<Value>`（非结构化），前端无法做类型安全的数据展示——需逐步补充 DTO 或接受动态 JSON 渲染
- Wegame 账号无令牌刷新能力——过期后只能重新扫码，用户体验不如 QQ/微信

## 待决项

1. **Pioneer 账号标识方案**：扩展 `AccountKind` 枚举添加 `Pioneer` 变体 vs 在 `extraJson` 中存标记 vs 前端通过
   `uin_or_openid` 前缀判定
2. **游戏 API 返回值结构化**：当前返回 `Value`（动态 JSON），是否需要 Rust 侧定义 typed DTO？建议 Phase 1 用 `Value` +
   前端动态渲染，Phase 2+ 根据实际数据结构补充类型
3. **令牌过期检测可靠性**：依赖 msg 文本匹配不可靠——需确认 Rust 后端是否能在 `ApiResponse.code` 中使用特定错误码标识令牌过期（如
   `code = -403`），而非通用的 `-1`

## 相关

- CONTEXT.md: 三角洲行动 API 工具相关术语
- docs/delta-ui-development.md: 完整开发文档
- src-tauri/src/delta/commands.rs: 43 个 Tauri 命令定义
- src-tauri/src/delta/storage/repo.rs: DeltaAccountRecord + AccountKind 定义
- src-tauri/src/delta/error.rs: DeltaError 枚举
