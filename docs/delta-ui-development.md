# 三角洲行动 API 工具 UI 开发文档

## 1. 功能概述

三角洲行动 API 工具（代码前缀 `delta_`）是将现有 Rust 后端 43 个 Tauri API 命令暴露给用户的交互层，按功能域拆为 3 个侧边栏入口：

| 侧边栏入口 | 职责 | 依赖账号类型 |
|------------|------|------------|
| **账号管理** | 账号 CRUD + 令牌全生命周期管理 | 所有类型 |
| **游戏数据** | 11 个游戏数据 API + 查询工作台 | QQ / 微信 |
| **工具箱** | Wegame 操作 + QQSafe 封禁/举报 + 先遣服列表 | Wegame / QQSafe / Pioneer |

## 2. 全局架构

### 2.1 侧边栏结构

```
现有侧边栏：
├── 摩斯密码解析（morse）
├── 计时器（timer）
├── 连发器（rapidfire）
├── 账号管理（delta-accounts）      ← 新增
├── 游戏数据（delta-game）          ← 新增
└── 工具箱（delta-toolbox）         ← 新增
```

每个入口对应独立页面组件。三页共享全局账号选中态。

### 2.2 全局账号状态

```
DeltaAccountsContext（React Context）
├── accounts: DeltaAccountRecord[]          // 来自 delta_list_accounts
├── selectedAccountId: i64 | null           // 全局选中账号 ID
├── selectedAccount: DeltaAccountRecord | null
├── selectAccount(id: i64): void
├── refreshAccounts(): void                 // 重新拉取账号列表
└── refreshToken(accountId: i64): Promise   // 刷新指定账号令牌
```

- 账号选中是**应用级状态**，三页共享
- 切换页面后选中态保持
- 游戏数据页只展示 QQ/微信账号的选择，工具箱页只展示对应类型账号
- 无账号选中时，数据页/工具箱页显示空态引导"请先添加账号"

### 2.3 账号能力矩阵

| 账号类型 | 游戏数据 | Wegame 操作 | QQSafe 封禁 | QQSafe 举报 | 先遣服 |
|---------|---------|-----------|-----------|-----------|-------|
| QQ | ✅ acctype=qc | ❌ | ❌ | ❌ | ❌ |
| WeChat | ✅ acctype=wx | ❌ | ❌ | ❌ | ❌ |
| QQSafe | ❌ | ❌ | ✅ | ✅ | ❌ |
| WegameQQ | ❌ | ✅ | ❌ | ❌ | ❌ |
| WegameWeChat | ❌ | ✅ | ❌ | ❌ | ❌ |
| Pioneer | ❌ | ❌ | ❌ | ❌ | ✅ |

页面根据选中账号类型**动态渲染**可用区域，不可用区域不渲染。

## 3. 账号管理页

### 3.1 页面结构

```
AppPage
├── PageHero
│   ├── eyebrow: "三角洲行动"
│   ├── title: "账号管理"
│   ├── description: "管理游戏账号登录状态与访问令牌"
│   └── stats: [总账号数, 有效令牌数, 即将过期数]  // SignalTile
│
├── TacticalCard "添加账号"
│   ├── 账号类型选择（QQ / 微信 / QQSafe / Wegame QQ / Wegame 微信 / 先遣服）
│   └── "扫码登录" 按钮 → 打开登录 Dialog
│
└── 账号列表（小卡片布局）
    └── 每个账号卡片：
        ├── 上行：类型 Badge + UIN/OpenID
        ├── 下行：令牌状态 + 能力标签（"游戏数据" / "Wegame" / "QQSafe" / "先遣服"）
        └── 点击选中 → 全局高亮
```

### 3.2 登录 Dialog 流程

```
Dialog（模态）
├── Step 1: 选择账号类型（6 选 1）
├── Step 2: 显示二维码 + 状态轮询
│   ├── 二维码图片（base64）
│   ├── 轮询状态指示（等待扫描 / 已扫描 / 登录成功 / 已过期 / 已拒绝）
│   └── "刷新二维码" 按钮（过期时显示）
├── Step 3: 获取访问令牌（自动，显示进度）
│   ├── 成功 → 关闭 Dialog，账号出现在列表
│   └── 失败 → 显示错误，"重试" / "重新扫码"
└── Esc / X 关闭（轮询自动取消）
```

**调用链路（以 QQ 为例）**：
1. `delta_qq_get_login_qr()` → `{ qrImage, qrToken, qrSig, loginSig, cookie }`
2. 轮询 `delta_qq_poll_login_status({ qrToken, qrSig, loginSig, cookie })` → `{ code, msg }`
   - code=0: 登录成功
   - code=1: 等待扫描
   - code=2: 已扫描等待确认
   - code=-2: 二维码过期 → 重新获取
   - code=-3: 用户拒绝
   - code=-4: 错误
3. 登录成功后 `delta_qq_get_access_token({ cookie })` → `{ accountId, account, auth: { openid, accessToken, expiresIn } }`
4. 账号自动持久化到 SQLite

**微信/Wegame 微信**流程略有不同：
1. `delta_wechat_get_login_qr()` → `{ qrCode(微信扫码URL), uuid }`
2. 轮询 `delta_wechat_poll_status({ uuid })` → `{ code, wxErrcode?, wxCode? }`
3. `delta_wechat_get_access_token({ code: wxCode })` → 令牌

**Wegame QQ** 使用子流程：
1. `delta_wegame_qq_get_login_qr()` → QR 码
2. 轮询 `delta_wegame_qq_poll_status(...)` → ticket
3. `delta_wegame_qq_get_access_token(...)` → tgpId + tgpTicket

### 3.3 令牌生命周期管理

```
令牌状态判定：
├── expires_at > now + 3天     → "令牌有效"（绿点）
├── expires_at > now           → "即将过期"（黄点 + 剩余天数）
└── expires_at ≤ now           → "已过期"（红点）
```

**自动刷新策略**（问题 8 决策 C）：
1. 其他页面调用 API 失败时检测令牌过期
2. 自动调用 `delta_qq_update_access_token` / `delta_wechat_update_access_token` 对应命令
3. 刷新成功 → 重试原请求
4. 刷新失败（cookie 也过期）→ 标记"需重新登录"，在账号卡片显示红点+提示

**账号管理页职责**：
- 展示所有账号令牌状态
- 提供"刷新令牌"手动操作
- 提供"删除账号"操作（调用 `delta_delete_account`）
- 令牌过期时自动提示刷新或重新登录

### 3.4 令牌状态摘要

账号管理页 PageHero 的 SignalTile：
- **总账号数**：`accounts.length`
- **有效令牌**：`accounts.filter(a => a.expiresAt > Date.now()).length`
- **即将过期**：`accounts.filter(a => a.expiresAt > Date.now() && a.expiresAt < Date.now() + 3*86400000).length`

## 4. 游戏数据页

### 4.1 页面结构

```
AppPage
├── PageHero
│   ├── eyebrow: "三角洲行动"
│   ├── title: "游戏数据"
│   ├── description: "查看游戏内角色数据与资产信息"
│   └── stats: [等级, 烽火地带场次, 全面战场场次]  // 来自 player + record
│
├── 账号选择器（顶部横条）
│   ├── 下拉选择当前账号（仅 QQ/微信）
│   └── 无账号时显示"请先在账号管理中添加 QQ 或微信账号"
│
├── 数据仪表盘（无参数 API，选中即加载）
│   ├── TacticalCard "角色信息"     ← player
│   ├── TacticalCard "战绩记录"     ← record（烽火地带 + 全面战场）
│   ├── TacticalCard "近期对局"     ← recent
│   ├── TacticalCard "资产概览"     ← assets
│   ├── TacticalCard "成就进度"     ← achievement
│   ├── TacticalCard "地图密码"     ← password
│   └── TacticalCard "角色绑定"     ← bind
│
└── TacticalCard "查询工作台"（有参数 API）
    ├── API 类型下拉（物品查询 / 物价查询 / 枪械详情 / 操作日志 / 改装方案 / 地图推荐）
    ├── 动态参数表单
    └── 结果展示区
```

### 4.2 数据加载策略（问题 15 决策 B）

**分批加载**：
1. **首批**（核心摘要）：`player` + `record` → 渲染 PageHero SignalTile + 两个卡片
2. **次批**（详情数据）：`assets` + `recent` + `achievement` + `password` + `bind` → 并行请求，逐步渲染

切换账号时重新触发加载。加载期间卡片显示 skeleton 状态。

### 4.3 查询工作台

| API | 参数表单字段 | 结果展示 |
|-----|------------|---------|
| 物品查询 | type 下拉 + subType 下拉 + itemId 输入（可选） | 物品列表（名称/图片/属性） |
| 物价查询 | itemId 多选输入 + withRecent 开关 | 价格表格 |
| 枪械详情 | gunId 输入 | 枪械属性面板（伤害/射速/弹容/配件槽） |
| 操作日志 | logType 下拉 + page 输入 | 日志列表（分页） |
| 改装方案 | page + pageSize | 方案列表（分页） |
| 地图推荐 | place 下拉（地图名） | 推荐装备列表 |

**交互**：选择 API 类型 → 表单字段动态切换 → 填写参数 → 点击"查询" → 结果渲染在卡片下方。支持切换 API 类型清空结果。

### 4.4 无参数 API 调用方式

所有游戏数据 API 通过 `GameAuth { openid, accessToken, acctype }` 鉴权：

```typescript
// 从选中账号构造 GameAuth
const auth = {
  openid: selectedAccount.openid,
  accessToken: selectedAccount.accessToken,
  acctype: selectedAccount.kind === "wechat" ? "wx" : "qc"
}

// 调用示例
const record = await invoke("delta_game_get_record", { auth })
const player = await invoke("delta_game_get_player", { auth })
```

## 5. 工具箱页

### 5.1 页面结构

```
AppPage
├── PageHero
│   ├── eyebrow: "三角洲行动"
│   ├── title: "工具箱"
│   ├── description: "Wegame 运营、安全查询与先遣服测试"
│   └── stats: [可用功能数, 今日操作次数]  // SignalTile
│
├── 账号选择器（顶部横条，可选类型随内容变化）
│
├── Wegame 卡片（选中 Wegame 账号时显示）
│   ├── 领取保险箱礼包按钮 + 结果展示
│   └── 每日抽卡按钮 + 结果展示
│
├── QQSafe 卡片（选中 QQSafe 账号时显示）
│   ├── 封禁记录列表（自动查询）
│   └── 举报折叠区（默认收起，标记"已弃用"）
│       ├── user_id 输入框
│       └── 查询按钮 + 结果
│
└── 先遣服卡片（选中 Pioneer 账号时显示）
    └── 测试游戏列表
```

### 5.2 Wegame 操作

**调用链路**：
```typescript
// 需要从 Wegame 账号构造 ticket
const ticket = {
  id: selectedAccount.uinOrOpenid,   // Wegame uin
  ticket: selectedAccount.accessToken // tgpTicket
}

// 领取保险箱礼包
const gift = await invoke("delta_wegame_open_treasure_gift", { ticket })

// 每日抽卡
const card = await invoke("delta_wegame_draw_daily_card", { ticket })
```

结果展示：Toast 通知或卡片内联展开。

### 5.3 QQSafe 操作

**封禁记录**：
```typescript
const auth = {
  openid: selectedAccount.openid,
  accessToken: selectedAccount.accessToken,
  code: selectedAccount.extraJson // JWT 解码后的 code
}
const bannedList = await invoke("delta_qqsafe_get_banned_list", auth)
```

**举报/游戏报告**（已弃用）：
```typescript
const report = await invoke("delta_qqsafe_report", {
  openid: selectedAccount.openid,
  accessToken: selectedAccount.accessToken,
  userId: inputUserId
})
```

### 5.4 先遣服

```typescript
// Pioneer key 来自账号的 accessToken
const testList = await invoke("delta_pioneer_get_game_test_list", {
  key: selectedAccount.accessToken,
  listType: "pc"  // 或 "mobile"
})
```

## 6. 前端类型定义

### 6.1 核心类型（delta-types.ts）

```typescript
// 账号记录（镜像 Rust DeltaAccountRecord）
export interface DeltaAccountRecord {
  id: number
  kind: AccountKind
  uinOrOpenid: string
  cookieJson: string
  openid: string | null
  accessToken: string | null
  extraJson: string | null
  expiresAt: number | null
  createdAt: number
  updatedAt: number
}

// 账号类型
export type AccountKind = "qq" | "wechat" | "qqsafe" | "wegame_qq" | "wegame_wechat"

// 游戏鉴权
export interface GameAuth {
  openid: string
  accessToken: string
  acctype: "qc" | "wx"
}

// 令牌状态
export type TokenStatus = "valid" | "expiring_soon" | "expired" | "none"

// 登录流程状态
export type LoginStep = "select_type" | "qr_code" | "polling" | "fetching_token" | "success" | "error"
export type PollStatus = "waiting" | "scanned" | "confirmed" | "expired" | "rejected" | "error"
```

### 6.2 登录结果类型

```typescript
// QQ 登录二维码响应
export interface QqLoginQrResult {
  qrImage: string       // base64
  qrToken: string
  qrSig: string
  loginSig: string
  cookie: string        // JSON 字符串
}

// QQ 轮询状态响应
export interface QqPollResult {
  code: number          // 0=成功 1=等待 2=已扫描 -2=过期 -3=拒绝 -4=错误
  msg: string
}

// QQ 访问令牌响应
export interface QqAccessTokenResult {
  accountId: number
  account: DeltaAccountRecord
  auth: { openid: string; accessToken: string; expiresIn: number }
}

// 微信登录二维码响应
export interface WechatLoginQrResult {
  qrCode: string        // 微信扫码 URL
  uuid: string
}

// 微信轮询状态响应
export interface WechatPollResult {
  code: number          // 1=等待 2=已扫描 3=成功 -2=超时 -3=拒绝 -4=错误
  wxErrcode?: string
  wxCode?: string
}

// Wegame QQ ticket 响应
export interface WegameQqAccessResult {
  accountId: number
  account: DeltaAccountRecord
  auth: { tgpId: string; tgpTicket: string }
}

// Wegame ticket（用于开箱/抽卡）
export interface WegameTicket {
  id: string
  ticket: string
}
```

### 6.3 游戏数据响应类型

```typescript
// 所有游戏 API 响应统一包装
export interface ApiResponse<T> {
  code: number   // 0=成功
  msg: string
  data: T
}

// 角色
export interface PlayerData { /* 待 Rust 返回结构确认 */ }

// 战绩
export interface RecordData {
  touchGold: BattleRecord[]    // 烽火地带
  battlefield: BattleRecord[]  // 全面战场
}

// 近期对局
export interface RecentData { /* 待确认 */ }

// 资产
export interface AssetsData { /* 待确认 */ }

// 成就
export interface AchievementData { /* 待确认 */ }

// 地图密码
export interface PasswordData { /* Map<地图名, 密码> */ }

// 角色绑定
export interface BindData { /* 待确认 */ }
```

### 6.4 查询工作台请求类型

```typescript
export interface ItemsQuery {
  typeId: number
  subType: number
  itemId?: number
}

export interface PriceQuery {
  args: number[]
  withRecent?: boolean
}

export interface GunsQuery {
  gunId: string
}

export interface LogsQuery {
  logType: number
  page: number
}

export interface FirearmModQuery {
  page: number
  pageSize: number
}

export interface RecommendationQuery {
  place: string
}
```

## 7. Rust 后端 API 映射

### 7.1 账号管理命令

| 命令 | 前端调用 | 说明 |
|------|---------|------|
| `delta_list_accounts` | `invoke("delta_list_accounts")` | 返回 `DeltaAccountRecord[]` |
| `delta_delete_account` | `invoke("delta_delete_account", { accountId })` | 删除账号 |
| `delta_qq_get_login_qr` | `invoke("delta_qq_get_login_qr", { options? })` | 获取 QQ 登录二维码 |
| `delta_qq_poll_login_status` | `invoke("delta_qq_poll_login_status", { qrToken, qrSig, loginSig, cookie, options? })` | 轮询 QQ 登录状态 |
| `delta_qq_get_access_token` | `invoke("delta_qq_get_access_token", { accountId?, cookie?, options? })` | 获取 QQ 访问令牌 |
| `delta_qq_update_access_token` | `invoke("delta_qq_update_access_token", { accountId?, cookie?, openid, accessToken, options? })` | 刷新 QQ 令牌 |
| `delta_wechat_get_login_qr` | `invoke("delta_wechat_get_login_qr", { options? })` | 获取微信登录二维码 |
| `delta_wechat_poll_status` | `invoke("delta_wechat_poll_status", { uuid, options? })` | 轮询微信登录状态 |
| `delta_wechat_get_access_token` | `invoke("delta_wechat_get_access_token", { code, options? })` | 获取微信访问令牌 |
| `delta_wechat_update_access_token` | `invoke("delta_wechat_update_access_token", { openid, accessToken, options? })` | 刷新微信令牌 |
| `delta_qqsafe_get_login_qr` | `invoke("delta_qqsafe_get_login_qr", { options? })` | 获取 QQSafe 登录二维码 |
| `delta_qqsafe_poll_status` | `invoke("delta_qqsafe_poll_status", { qrToken, qrSig, loginSig, cookie, options? })` | 轮询 QQSafe 登录状态 |
| `delta_qqsafe_get_access_token` | `invoke("delta_qqsafe_get_access_token", { accountId?, cookie?, qq?, options? })` | 获取 QQSafe 访问令牌 |
| `delta_wegame_qq_get_login_qr` | `invoke("delta_wegame_qq_get_login_qr", { options? })` | 获取 Wegame QQ 登录二维码 |
| `delta_wegame_qq_poll_status` | `invoke("delta_wegame_qq_poll_status", { request: { qrToken, qrSig, loginSig, cookie }, options? })` | 轮询 Wegame QQ 状态 |
| `delta_wegame_qq_get_access_token` | `invoke("delta_wegame_qq_get_access_token", { accountId?, cookie?, options? })` | 获取 Wegame QQ 令牌 |
| `delta_wegame_wechat_get_login_qr` | `invoke("delta_wegame_wechat_get_login_qr", { options? })` | 获取 Wegame 微信登录二维码 |
| `delta_wegame_wechat_poll_status` | `invoke("delta_wegame_wechat_poll_status", { uuid, options? })` | 轮询 Wegame 微信状态 |
| `delta_wegame_wechat_get_access_token` | `invoke("delta_wegame_wechat_get_access_token", { code, options? })` | 获取 Wegame 微信令牌 |
| `delta_pioneer_get_login_qr` | `invoke("delta_pioneer_get_login_qr", { options? })` | 获取先遣服登录二维码 |
| `delta_pioneer_poll_status` | `invoke("delta_pioneer_poll_status", { qrToken, qrSig, loginSig, cookie, options? })` | 轮询先遣服登录状态 |
| `delta_pioneer_get_access_token` | `invoke("delta_pioneer_get_access_token", { cookie, options? })` | 获取先遣服令牌 |
| `delta_pioneer_update_access_token` | `invoke("delta_pioneer_update_access_token", { openid, accessToken, cookie?, options? })` | 刷新先遣服令牌 |

### 7.2 游戏数据命令

| 命令 | 参数 | 说明 |
|------|------|------|
| `delta_game_get_record` | `{ auth: GameAuth }` | 战绩记录 |
| `delta_game_get_player` | `{ auth: GameAuth }` | 角色信息 |
| `delta_game_get_assets` | `{ auth: GameAuth }` | 资产查询 |
| `delta_game_get_logs` | `{ auth: GameAuth, logType, page }` | 操作日志 |
| `delta_game_get_recent` | `{ auth: GameAuth }` | 近期对局 |
| `delta_game_get_achievement` | `{ auth: GameAuth }` | 成就 |
| `delta_game_get_password` | `{ auth: GameAuth }` | 地图密码 |
| `delta_game_get_manufacture` | `{ auth: GameAuth }` | 制造列表 |
| `delta_game_get_guns` | `{ gunId }` | 枪械详情（无需 auth） |
| `delta_game_get_items` | `{ typeId, subType, itemId? }` | 物品查询（无需 auth） |
| `delta_game_get_config` | `{ }` | 配置查询（无需 auth） |
| `delta_game_get_price` | `{ args, withRecent? }` | 物价查询（无需 auth） |
| `delta_game_get_firearm_mod_list` | `{ page, pageSize }` | 改装方案（无需 auth） |
| `delta_game_get_recommendation` | `{ place }` | 地图推荐（无需 auth） |
| `delta_game_get_bind` | `{ auth: GameAuth }` | 角色绑定 |

### 7.3 工具箱命令

| 命令 | 参数 | 说明 |
|------|------|------|
| `delta_wegame_open_treasure_gift` | `{ ticket: WegameTicket }` | 领取保险箱礼包 |
| `delta_wegame_draw_daily_card` | `{ ticket: WegameTicket }` | 每日抽卡 |
| `delta_qqsafe_get_banned_list` | `{ openid, accessToken, code }` | 封禁列表 |
| `delta_qqsafe_report` | `{ openid, accessToken, userId }` | 游戏报告（已弃用） |
| `delta_pioneer_get_game_test_list` | `{ key, listType? }` | 先遣服测试列表 |

## 8. 新增前端文件规划

```
src/
├── components/
│   ├── app/
│   │   ├── delta-accounts-page.tsx      # 账号管理页
│   │   ├── delta-game-page.tsx          # 游戏数据页
│   │   ├── delta-toolbox-page.tsx       # 工具箱页
│   │   ├── delta-types.ts              # 类型定义
│   │   ├── delta-utils.ts              # 工具函数（令牌状态判定、账号能力查询等）
│   │   ├── delta-login-dialog.tsx       # 登录 Dialog 组件
│   │   ├── delta-account-card.tsx       # 账号小卡片组件
│   │   ├── delta-account-selector.tsx   # 账号选择器横条组件
│   │   ├── delta-query-workbench.tsx    # 查询工作台卡片组件
│   │   ├── delta-data-card.tsx          # 数据展示卡片组件（通用）
│   │   └── delta-token-badge.tsx        # 令牌状态徽章组件
│   └── ui/                              # 无需新增，复用现有 shadcn/ui 组件
├── hooks/
│   └── use-delta-accounts.tsx           # DeltaAccountsContext + Provider + useDeltaAccounts hook
└── App.tsx                              # 修改：侧边栏增加 3 个入口 + 路由分支
```

## 9. 开发优先级

### Phase 1：基础设施
1. `delta-types.ts` — 类型定义
2. `delta-utils.ts` — 工具函数
3. `use-delta-accounts.tsx` — 全局账号 Context
4. `App.tsx` — 侧边栏 3 个入口 + 占位页面

### Phase 2：账号管理页
5. `delta-account-card.tsx` — 账号卡片
6. `delta-token-badge.tsx` — 令牌徽章
7. `delta-login-dialog.tsx` — 登录 Dialog
8. `delta-accounts-page.tsx` — 完整页面

### Phase 3：游戏数据页
9. `delta-account-selector.tsx` — 账号选择器
10. `delta-data-card.tsx` — 通用数据卡片
11. `delta-game-page.tsx` — 仪表盘页面（含分批加载）

### Phase 4：查询工作台
12. `delta-query-workbench.tsx` — 参数化查询

### Phase 5：工具箱页
13. `delta-toolbox-page.tsx` — Wegame + QQSafe + 先遣服

## 10. 设计约束

- 延续现有"战术白色操作台"视觉风格
- 复用 `app-ui.tsx` 共享组件：`AppPage` / `PageHero` / `SignalTile` / `TacticalCard` / `SectionHeader` / `ControlTile` / `SaveStateBadge` / `CardBody`
- 图标使用 `@remixicon/react`，Button 内图标加 `data-icon` 属性
- 表单使用 `FieldGroup` / `Field` / `FieldLabel` / `FieldContent`
- 异常提示使用 `Alert` / `FieldError` / `Badge`
- 无需新增透明窗口、热键、overlay——本工具是纯 API 调用层
- 无需新增 Tauri 命令——所有 43 个命令已存在
- 无需新增 Tauri 事件——API 工具当前无推送事件
