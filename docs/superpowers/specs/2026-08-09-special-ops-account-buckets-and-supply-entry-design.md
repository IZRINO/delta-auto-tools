# 特勤处到期账号分桶与军需处入口设计

## 状态

设计已按用户选择 A 方案，待用户评审后进入实现计划。

## 目标

修正两个运行问题：

1. 同一轮有多个已到期任务时，先按账号顺序处理；同账号已到期制作台、子弹兑换、限时商品、交易行任务在一次登录会话内串行完成，避免账号任务交错。
2. 统一子弹兑换与限时商品的军需处入口。两类业务共享一次入口流程，再分别识别并点击“战术部门”或“研发部门”，从而可在同一次登录会话内连续执行。

## 不在范围内

- 不改 24 小时任务时间轴的时间排序、逾期时间、10 分钟视觉合并规则。
- 不并发发送键鼠输入。子弹、限时商品仍在同一输入锁内串行执行。
- 不改利润筛选、仓库满、价格波动、人工校正、账号失败分级规则。
- 不保存新的参考图片。新识别目标继续使用用户上传参考图路径与现有双采样模板验证。

## 执行计划模型

### 计划构建

`RoundPlan.accounts` 继续使用 `AccountRoundTask`，不新增持久化字段。

构建计划时分两段处理：

1. 收集 `scheduledAt <= createdAt` 且通过账号状态、业务启用和利润 gate 的任务，按 `accountId` 聚合为一个桶。
2. 对每个账号桶按账号 `order` 升序排列；桶内固定顺序为四制作台顺序、子弹业务配置顺序、限时商品、交易行。`scheduledAt` 取桶内最早任务时间，仅用于运行快照和等待判断。
3. 再追加未来制作任务，未来任务仍按 `(scheduledAt, accountOrder, taskOrder, id)` 排序；同账号同一 `scheduledAt` 的未来制作台仍可合并。未来任务不得因为账号分桶提前执行。

因此，若账号 1 有三个同时到期制作台，账号 2 有一个同时到期制作台，计划顺序固定为：

```text
账号 1（三台连续） -> 账号 2 -> 后续账号
```

若当前任务完成后队首下一任务仍是同账号，且计划时间差不超过 10 分钟，沿用现有会话保持规则；若队首是其他账号或间隔超过 10 分钟，关闭游戏后切号。该判断仍由 `round_runner` 负责，计划构建不把未来任务错误并入当前账号桶。

### 失败与重试

- 账号级失败继续移除该账号本轮剩余队列，转下一个账号。
- 系统级失败、输入取消、关闭游戏失败继续全局暂停。
- 导航重试、限时商品补偿重试、交易行窗口规则保持现有实现。
- 同账号桶中某个业务失败时，不自动盲目执行该账号桶后续业务；沿用现有账号级失败处理。

## 军需处入口

### 新流程

```text
1. 识别并点击 ammo.department
2. 等待 `ammoSupplyDelayMs`，点击 `ammo.supply`
3. 等待 `ammoTacticalDelayMs`，点击 `ammo.enterSupply`
4. 子弹业务：识别并点击 ammo.tacticalDepartment
5. 限时商品业务：识别并点击 ammo.researchDepartment
6. 执行各自后续业务
```

同一账号同时包含子弹和限时商品时，步骤 1–3 只执行一次；步骤 4、5 按业务顺序串行执行。只有子弹时不识别研发部门，只有限时商品时不识别战术部门。

### 校准目标

保留：

- `ammo.department`：模板识别区域并点击
- `ammo.supply`：共享“军需处”固定点击点
- `limited.ready`：研发部门页面就绪模板
- `limited.color.1` 至 `limited.color.9`

新增：

- `ammo.enterSupply`：共享“进入军需处”固定点击点
- `ammo.tacticalDepartment`：战术部门模板识别与点击区域
- `ammo.researchDepartment`：研发部门模板识别与点击区域

删除运行依赖并从标准校准列表移除：

- `ammo.tactical`
- `limited.research`

旧环境中这两个 key 由 `normalize_settings` 丢弃；新识别目标没有参考图或未验证时，执行前置校验失败，禁止盲操作。用户需要为新目标重新框选并上传参考图。

### 等待配置

- `ammoSupplyDelayMs` 继续表示点击 `ammo.supply` 前等待。
- 现有序列化字段 `ammoTacticalDelayMs` 兼容保留，语义改为点击 `ammo.enterSupply` 前等待；前端标签改为“进入军需处前等待”。不新增第二份重复时间配置。
- `limitedSupply.researchDelayMs` 保留旧 JSON 读取兼容，但不再参与 runtime，也不在 UI 显示。
- 新流程中的识别等待、模板双采样和页面就绪超时继续使用现有超时策略，不增加固定延时成功条件。

## Runtime 接口边界

新增一个共享军需处 runtime 编排层，负责步骤 1–3；子弹与限时商品模块只负责各自分支：

- `ammo_runtime`：拆出“不含入口”的目标兑换函数，入口由共享编排层调用。
- `limited_supply_runtime`：拆出“不含入口”的页面就绪与识色函数，入口由共享编排层调用。
- `round_account::AccountSessionDriver` 增加一次 `military_supply` 会话动作，替代同一任务中先后调用 `ammo`、`limited_supply` 造成重复入口。
- 单独子弹试运行、单独限时商品试运行复用共享入口，但仍只执行请求的分支。

共享编排结果必须区分：子弹完成、限时商品完成、限时商品可重试超时；错误 step 保留具体目标 key，现有持久化和人工标记逻辑不变。

## 前端变更

- 点击区域校准列表显示 `ammo.enterSupply`、`ammo.tacticalDepartment`、`ammo.researchDepartment`。
- 删除 `ammo.tactical`、`limited.research` 的校准行及各自等待行。
- `ammoTacticalDelayMs` 输入标签改为“进入军需处前等待”。
- 限时商品配置不再显示研发部门等待输入。
- 其他业务配置、试运行按钮、测试反馈和折叠结构不变。

## 兼容与持久化

- 不新增持久化账号字段；计划分桶只存在于本轮内存快照。
- 旧配置的旧校准 key 在标准化时移除，避免错误地把固定点当作识别点击点。
- 旧 `ammoTacticalDelayMs` 数值直接作为共享入口等待值继续使用。
- 旧 `limitedSupply.researchDelayMs` 读取后不报错，重新保存时可保留字段或按现有 serde 行为丢弃，但 runtime 不读取。
- Profile、`special_ops_settings.json`、版本号和其他工具配置边界不改变。

## 验证

Rust：

- 新增计划测试：同一时间多个账号任务按账号 order 分桶；同账号桶内制作台、子弹、限时商品、交易行字段全部合并。
- 保留并改写旧的全局交错测试，断言不再出现 `账号1 -> 账号2 -> 账号1` 的已到期拆桶。
- 新增共享入口 runtime 测试：组合任务只调用一次 `ammo.department`、`ammo.supply`、`ammo.enterSupply`，再按顺序调用战术/研发识别分支。
- 新增单分支测试：只有子弹不调用研发目标，只有限时商品不调用战术目标。
- 新增标准化测试：旧 `ammo.tactical`、`limited.research` 被移除，新 key 被补齐且类型正确。

执行：

```text
cargo fmt --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
bun run build
bun run test
```
