# 特勤处账号分桶与军需处入口实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. 当前按 inline 执行，不派发子代理。

**Goal:** 让同一轮已到期任务按账号分桶连续执行，并让子弹兑换与限时商品共享一次军需处入口后在同一登录会话串行完成。

**Architecture:** 保留 `RoundPlan.accounts` 与现有 round scheduler/runner 契约。planner 先聚合已到期账号桶，再追加未来制作 lookahead；session driver 增加一次 `military_supply` 编排，统一完成部门、军需处和进入军需处动作，业务模块只处理战术/研发分支。旧校准 key 在标准化时丢弃，新识别 key 进入统一 preflight。

**Tech Stack:** Rust、Tokio、Serde、Tauri 2、React 19、TypeScript、Vitest、Cargo test、Bun。

---

### Task 1: 计划按账号分桶

**Files:**
- Modify: `src-tauri/src/special_ops/round_planner.rs`
- Test: `src-tauri/src/special_ops/round_planner.rs` 内联测试

- [ ] **Step 1: 替换旧行为测试**

把 `global_tasks_keep_intervening_account_before_same_account_follow_ups` 改名为 `due_tasks_group_by_account_order`，输入两个账号在当前时间同时到期的 `10:00`、`10:04`、`10:08` 与 `10:02` 任务，断言计划仅产生两个已到期桶，账号顺序为 `account-1`、`account-2`，账号 1 的 stations 按 `TechnicalCenter`、`Workbench`、`Pharmacy` 排列。

把 `plans_due_work_then_near_future_work_in_global_time_order` 改为断言：已到期任务先按账号顺序聚合；未来任务仍按时间追加，且不会并入已到期桶。

新增失败测试：同一账号同时拥有制作台、子弹、限时商品和交易行任务时，单个 `AccountRoundTask` 同时携带 `stations`、`ammo_target_ids`、`limited_supply_cycle_id`、`market_purchase_day`。

- [ ] **Step 2: 运行 planner 测试确认失败**

运行：

```text
cargo test --manifest-path src-tauri/Cargo.toml special_ops::round_planner
```

预期：旧全局交错断言失败，证明测试锁定新需求。

- [ ] **Step 3: 实现两阶段构建**

在 `build_round_plan_with_profit` 中将 `schedule.timeline_tasks` 分为 `due_tasks` 与 `future_craft_tasks`：

```rust
fn is_due_for_account_task(
    task: &TimelineTask,
    due_accounts: &HashMap<&str, &DueAccount>,
    now_ms: i64,
) -> bool;

fn merge_timeline_task(entry: &mut AccountRoundTask, task: TimelineTask);

fn group_future_craft_tasks(tasks: Vec<TimelineTask>) -> Vec<AccountRoundTask>;

let mut due_by_account = HashMap::<String, AccountRoundTask>::new();
let mut future_tasks = Vec::new();
for task in schedule.timeline_tasks {
    if is_due_for_account_task(&task, &due_accounts, created_at_ms) {
        let account_order = account_order_by_id[task.account_id.as_str()];
        let entry = due_by_account.entry(task.account_id.clone()).or_insert_with(|| {
            AccountRoundTask {
                account_id: task.account_id.clone(),
                qq_account: task.qq_account.clone(),
                account_order,
                scheduled_at_ms: task.scheduled_at_ms,
                stations: Vec::new(),
                ammo_target_ids: Vec::new(),
                limited_supply_cycle_id: None,
                market_purchase_day: None,
            }
        });
        merge_timeline_task(entry, task);
    } else if is_future_craft_task(&task, created_at_ms) {
        future_tasks.push(task);
    }
}
let mut accounts = due_by_account.into_values().collect::<Vec<_>>();
accounts.sort_by_key(|task| (task.account_order, task.scheduled_at_ms));
accounts.extend(group_future_craft_tasks(future_tasks));
```

`merge_timeline_task` 只追加业务 ID，不修改业务配置和持久化状态；stations 使用 `StationKind::all()` 的固定顺序重排，ammo 使用业务 `order`，限时商品与交易行仍各自只保留一个周期字段。保留现有账号资格、profit gate、未来 lookahead 和空桶剔除逻辑。

- [ ] **Step 4: 运行 planner 测试确认通过**

运行同一命令，预期全部 planner 测试通过；特别确认 `should_continue_round`、`can_chain_follow_up` 未被改写。

- [ ] **Step 5: 提交计划分桶实现**

```text
git add src-tauri/src/special_ops/round_planner.rs
git commit -m "fix(special-ops): 已到期任务按账号分桶"
```

### Task 2: 共享军需处入口 runtime

**Files:**
- Create: `src-tauri/src/special_ops/military_supply_runtime.rs`
- Modify: `src-tauri/src/special_ops/ammo_runtime.rs`
- Modify: `src-tauri/src/special_ops/limited_supply_runtime.rs`
- Test: 上述三个 Rust 文件内联测试

- [ ] **Step 1: 写共享入口失败测试**

在新模块加入 `ScriptedSupplyDriver`，测试组合分支调用顺序：

```rust
assert_eq!(driver.actions(), [
    "wait:ammo.department",
    "delay:3000",
    "click:ammo.supply",
    "delay:3000",
    "click:ammo.enterSupply",
    "wait:ammo.tacticalDepartment",
    "ammo-targets",
    "wait:ammo.researchDepartment",
    "limited-check",
]);
```

另写两个测试：只有子弹时不存在 `ammo.researchDepartment`，只有限时商品时不存在 `ammo.tacticalDepartment`。

- [ ] **Step 2: 运行新测试确认失败**

运行：

```text
cargo test --manifest-path src-tauri/Cargo.toml military_supply_runtime
```

预期：模块或入口函数尚不存在，测试失败。

- [ ] **Step 3: 实现共享入口接口**

在 `military_supply_runtime.rs` 定义不持久化的运行配置和 driver：

```rust
pub(crate) struct MilitarySupplyEntryConfig {
    pub supply_delay: Duration,
    pub enter_supply_delay: Duration,
}

pub(crate) trait MilitarySupplyEntryDriver: Send + Sync {
    async fn wait_and_click(&self, key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn delay(&self, duration: Duration, cancelled: Arc<AtomicBool>) -> Result<(), String>;
}

pub(crate) async fn enter_military_supply<D: MilitarySupplyEntryDriver + ?Sized>(
    driver: &D,
    config: MilitarySupplyEntryConfig,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String>;
```

该函数只执行 `ammo.department` 识别点击、`ammo.supply` 固定等待点击、`ammo.enterSupply` 固定等待点击；不执行战术/研发分支。

- [ ] **Step 4: 拆出两个业务模块的入口后逻辑**

将 `ammo_runtime::run_ammo_trial` 拆为可复用的 `run_ammo_targets` 与保留入口的试运行包装器；将 `limited_supply_runtime::run_limited_supply` 拆为 `run_limited_supply_branch` 与保留入口的试运行包装器。旧单功能测试继续通过，组合编排测试调用无入口分支。

- [ ] **Step 5: 运行 runtime 单测**

```text
cargo test --manifest-path src-tauri/Cargo.toml ammo_runtime
cargo test --manifest-path src-tauri/Cargo.toml limited_supply_runtime
cargo test --manifest-path src-tauri/Cargo.toml military_supply_runtime
```

- [ ] **Step 6: 提交共享入口实现**

```text
git add src-tauri/src/special_ops/military_supply_runtime.rs src-tauri/src/special_ops/ammo_runtime.rs src-tauri/src/special_ops/limited_supply_runtime.rs
git commit -m "feat(special-ops): 共享军需处入口编排"
```

### Task 3: 接入账号会话与冻结配置

**Files:**
- Modify: `src-tauri/src/special_ops/round_account.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Test: `src-tauri/src/special_ops/round_account.rs`、`src-tauri/src/special_ops/mod.rs` 内联测试

- [ ] **Step 1: 扩展会话 driver 测试替身**

给 `AccountSessionDriver` 测试替身增加 `military_supply` action，组合任务断言 `login -> navigation -> craft -> militarySupply -> market`；删除旧的组合 `craft -> ammo -> limited` 断言，保留单功能任务行为测试。

- [ ] **Step 2: 运行失败测试**

```text
cargo test --manifest-path src-tauri/Cargo.toml round_account
```

预期：trait 未提供新动作时编译或断言失败。

- [ ] **Step 3: 修改会话接口与结果映射**

新增：

```rust
async fn military_supply(
    &self,
    task: &AccountRoundTask,
    cancelled: Arc<AtomicBool>,
) -> Result<MilitarySupplySessionResult, AccountRunError>;
```

`MilitarySupplySessionResult` 至少包含 `limited_retry_requested: bool`；子弹失败继续直接返回 `AccountRunError::account_ammo`，限时商品 ready 超时只返回可重试结果。`run_task_in_session` 在 craft 后最多调用一次 `military_supply`，再进入 market。

- [ ] **Step 4: 修改 ProductionRoundDriver 冻结与运行**

`FrozenRoundAccount` 保存组合军需处配置，不再为同一 task 分别生成独立入口配置。`freeze_ammo_run`、`freeze_limited_supply_run` 分别只校验各自分支 key；共享入口校验：

```rust
[("ammo.department", true), ("ammo.supply", false), ("ammo.enterSupply", false)]
```

子弹分支追加 `ammo.tacticalDepartment`，限时商品分支追加 `ammo.researchDepartment` 与 `limited.ready`、九个识色区域。`ProductionRoundDriver::military_supply` 先调用共享入口，再按 `craft -> ammo -> limited` 既定业务顺序执行分支。

- [ ] **Step 5: 运行会话与 round runner 测试**

```text
cargo test --manifest-path src-tauri/Cargo.toml round_account
cargo test --manifest-path src-tauri/Cargo.toml round_runner
```

预期：同账号分桶只登录一次；不同账号仍关闭游戏并重新登录；导航重试和账号级失败队列语义不变。

- [ ] **Step 6: 提交会话接入**

```text
git add src-tauri/src/special_ops/round_account.rs src-tauri/src/special_ops/mod.rs
git commit -m "feat(special-ops): 同账号会话合并军需处业务"
```

### Task 4: 校准目标、标准化与前置校验

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src/components/app/special-ops-page.tsx`
- Modify: `src/components/app/special-ops-types.ts`
- Test: Rust `mod.rs` 内联测试、`src/components/app/special-ops-page.test.tsx`

- [ ] **Step 1: 写校准迁移失败测试**

扩展 `normalize_removes_legacy_*` 测试：标准目标包含 `ammo.enterSupply`、`ammo.tacticalDepartment`、`ammo.researchDepartment`，不包含 `ammo.tactical`、`limited.research`；旧 key 加入输入后 normalize 后消失。

- [ ] **Step 2: 实现目标列表和 required keys**

更新 `default_calibration_targets` 标签、kind、recognition method 和 guards；更新 `required_execution_target_keys`、`freeze_ammo_run`、`freeze_limited_supply_run`、校准测试 step 映射。删除旧固定点的 runtime 引用。

- [ ] **Step 3: 更新等待字段校验与 TS 类型**

Rust 保留 `ammo_tactical_delay_ms` 字段和 JSON 名称；取消 `limited_supply.research_delay_ms` 的运行校验。TypeScript 保留 `ammoTacticalDelayMs` 类型，更新页面标签和自动保存路径，不再渲染研发部门等待输入。

- [ ] **Step 4: 更新校准页面**

新增三行目标对应的参考图/框选/测试操作；移除两个旧目标及其等待行。识别目标仍由现有 calibration overlay 处理，点击点继续点中心，禁止新增遮罩窗口或账号级坐标复制。

- [ ] **Step 5: 运行前端与 Rust 相关测试**

```text
cargo test --manifest-path src-tauri/Cargo.toml normalize
bunx vitest run src/components/app/special-ops-page.test.tsx
```

- [ ] **Step 6: 提交校准与 UI**

```text
git add src-tauri/src/special_ops/mod.rs src/components/app/special-ops-page.tsx src/components/app/special-ops-types.ts src/components/app/special-ops-page.test.tsx
git commit -m "feat(special-ops): 更新军需处识别校准目标"
```

### Task 5: 文档同步与全量验证

**Files:**
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `droid-wiki/reference/configuration.md`
- Modify: `README.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: 同步 wiki**

更新特勤处执行顺序、共享军需处入口、校准 key、兼容字段和账号分桶规则。删除文档中“`limited.research` 固定点击点”和“`ammo.tactical` 固定点击点”作为当前运行目标的描述。

- [ ] **Step 2: 运行代码图同步**

```text
codegraph sync
```

- [ ] **Step 3: 运行全量质量门禁**

```text
cargo fmt --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
bun run build
bun run test
```

- [ ] **Step 4: 检查工作区边界**

```text
git status --short
git diff --check
```

只确认本计划文件、代码文件和 wiki 文件发生变化；不处理用户已有未提交改动。

- [ ] **Step 5: 提交文档与最终结果**

```text
git add droid-wiki/features/special-ops.md droid-wiki/reference/configuration.md README.md AGENTS.md
git commit -m "docs(special-ops): 同步账号分桶与军需处入口规则"
```
