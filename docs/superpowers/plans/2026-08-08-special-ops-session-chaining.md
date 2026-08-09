# 特勤处同账号会话链 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让特勤处轮次按全局到期时间执行；当前任务与下一任务同账号且间隔不超过10分钟时保持游戏会话等待，否则关闭游戏切换账号。

**Architecture:** 将轮次任务从“按账号聚合的当前到期快照”扩展为全局排序任务队列。`round_runner` 负责会话链决策和等待，`round_account` 负责登录一次后的单任务执行，`ProductionRoundDriver` 负责游戏窗口、账号校验和持久化。未来任务只进入内存冻结队列，不写入新的业务状态；重启、休眠跳变和紧急停止仍从持久化完成时间重新规划。

**Tech Stack:** Rust、Tokio、Tauri 2、Vitest、Cargo test、现有 `round_planner`/`round_runner`/`round_account` 抽象。

---

### Task 1: 建立全局原子任务队列

**Files:**
- Modify: `src-tauri/src/special_ops/round_planner.rs`
- Modify: `src-tauri/src/special_ops/mod.rs:8727-8885`
- Test: `src-tauri/src/special_ops/round_planner.rs` 内联测试

- [ ] **Step 1: 写失败测试，锁定排序与后继规则**

新增测试覆盖：

```rust
#[test]
fn ordered_tasks_keep_intervening_account_before_same_account_follow_up() {
    // account-1: 10:00, 10:04, 10:08; account-2: 10:02
    // 预期顺序：account-1@10:00, account-2@10:02,
    // account-1@10:04, account-1@10:08
}

#[test]
fn same_account_follow_up_at_exactly_ten_minutes_is_chainable() {
    // 10:00 -> 10:10 可保持会话
}

#[test]
fn same_account_follow_up_at_eleven_minutes_requires_new_session() {
    // 10:00 -> 10:11 不可保持会话
}
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml round_planner
```

预期：新测试因不存在全局原子任务/后继判断而失败。

- [ ] **Step 3: 增加冻结任务类型和排序函数**

在 `round_planner.rs` 增加一个只读运行时任务类型，至少包含：

```rust
pub(crate) struct ScheduledRoundTask {
    pub account_id: String,
    pub qq_account: String,
    pub account_order: u32,
    pub scheduled_at_ms: i64,
    pub station_kind: Option<StationKind>,
    pub ammo_target_id: Option<String>,
}

pub(crate) fn can_chain_follow_up(
    current: &ScheduledRoundTask,
    next: &ScheduledRoundTask,
) -> bool {
    current.account_id == next.account_id
        && next.scheduled_at_ms.saturating_sub(current.scheduled_at_ms) <= 10 * 60_000
}
```

按 `(scheduled_at_ms, account_order, task_order)` 排序；同一账号同一时间的制作台和子弹任务保持现有业务顺序。

- [ ] **Step 4: 让轮次计划同时保留当前到期任务和后继快照**

在 `RoundPlan` 增加全局排序的冻结任务列表；现有 `AccountRoundTask` 继续作为兼容的账号工作集合，直到 `round_runner` 完成迁移。未来任务不得在未到期前执行，只用于会话链后继判断。

- [ ] **Step 5: 运行测试确认通过**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml round_planner
```

预期：排序、10分钟边界和跨账号插入测试通过；已有 scheduler/利润测试不回归。

### Task 2: 拆分账号会话与任务执行

**Files:**
- Modify: `src-tauri/src/special_ops/round_account.rs`
- Modify: `src-tauri/src/special_ops/round_runner.rs`
- Test: `src-tauri/src/special_ops/round_account.rs` 内联测试
- Test: `src-tauri/src/special_ops/round_runner.rs` 内联测试

- [ ] **Step 1: 写失败测试，验证一次登录执行多个任务**

Fake driver 记录动作并断言：

```rust
assert_eq!(actions, [
    "login", "navigation", "task:technicalCenter",
    "wait-until:...", "task:workbench", "close-game"
]);
```

同时覆盖：下一任务为其他账号时先关闭、账号级失败跳过后继、任务成功后逐项持久化。

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml round_account round_runner
```

预期：现有接口无法表达“登录一次后继续任务”，测试失败。

- [ ] **Step 3: 将 `AccountSessionDriver` 拆成会话生命周期接口**

保留现有制作/子弹动作实现，新增明确边界：

```rust
async fn login_and_navigate(...);
async fn run_task_in_session(...);
async fn wait_until(...);
async fn close_game(...);
```

`run_task_in_session` 只处理当前任务，不重复登录；制作仍先于同一账号的子弹兑换。

- [ ] **Step 4: 改造 `run_round` 会话链循环**

维护当前登录账号和全局任务游标：

1. 登录并导航当前任务账号。
2. 执行当前任务。
3. 查看全局队列下一条未完成任务。
4. 同账号且计划时间差 `<=10` 分钟：等待到期后在原会话执行。
5. 其他账号或差值 `>10` 分钟：关闭游戏，回到账号切换流程。
6. 失败、暂停、紧急停止立即走现有错误分支。

- [ ] **Step 5: 运行测试确认通过**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml round_account round_runner
```

预期：登录次数、等待行为、跨账号切换和错误跳过测试通过。

### Task 3: 接入生产驱动和窗口校验

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:5570-5905`
- Modify: `src-tauri/src/special_ops/game_navigation.rs`（仅复用现有窗口/账号校验接口）
- Test: `src-tauri/src/special_ops/mod.rs` 内联 round/session 测试

- [ ] **Step 1: 写生产驱动行为测试**

验证：

- 同账号链中只调用一次登录、导航；
- 等待结束重新确认游戏窗口和当前账号；
- 不同账号切换前关闭游戏；
- WeGame 崩溃时重新读取账号，游戏单独崩溃时沿用现有恢复规则。

- [ ] **Step 2: 实现 `ProductionRoundDriver` 会话保持**

将 `run_account` 的登录/导航与制作/子弹执行拆开；同账号后继调用只进入四制作台页面，不重复启动 WeGame 或提交账号信息。

- [ ] **Step 3: 实现可取消等待**

等待同时监听：

- 下一任务到期时间；
- `pauseRequested`；
- 紧急停止取消标志；
- scheduler shutdown；
- 时间跳变/休眠检测。

等待期间禁止键鼠输入。等待结束必须重新验证当前游戏窗口和账号会话状态。

- [ ] **Step 4: 保持逐项持久化和现有失败映射**

每个制作台/子弹目标成功后立即保存；`AccountRunError` 的账号级失败跳过当前账号；系统级失败继续全面暂停；关闭游戏失败继续使用 `round.closeGame` 暂停规则。

- [ ] **Step 5: 运行 Rust 特勤处测试**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops
```

预期：特勤处全部单元测试通过。

### Task 4: 调整 scheduler 与暂停边界

**Files:**
- Modify: `src-tauri/src/special_ops/round_scheduler.rs`
- Modify: `src-tauri/src/special_ops/mod.rs:6811-6935`
- Test: `src-tauri/src/special_ops/round_scheduler.rs` 内联测试

- [ ] **Step 1: 写 scheduler 回归测试**

覆盖：活动会话等待时不重复启动新轮次；等待结束后只唤醒一次；暂停/休眠跳变不会继续使用旧会话。

- [ ] **Step 2: 实现活动会话优先级**

`active_run` 期间 scheduler 不启动第二个 `LaunchRound`。会话链自身负责等待和后继任务；会话结束后由 scheduler 重新读取持久化状态生成下一轮。

- [ ] **Step 3: 验证暂停、紧急停止和系统级失败**

确认：

- 用户暂停不执行新键鼠；
- 紧急停止立即取消等待并释放输入；
- 休眠跳变全面暂停且不复用旧游戏；
- 普通账号失败不升级为全局暂停。

- [ ] **Step 4: 运行 scheduler 测试**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml round_scheduler
```

### Task 5: 更新时间轴状态与文档

**Files:**
- Modify: `src/components/app/special-ops-types.ts`
- Modify: `src/components/app/special-ops-page.tsx`
- Modify: `src/components/app/special-ops-utils.ts`
- Test: `src/components/app/special-ops-utils.test.ts`
- Test: `src/components/app/special-ops-page.test.tsx`
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `README.md`

- [ ] **Step 1: 增加会话链展示状态**

仅展示当前账号“等待下一任务/准备切换账号”等状态；不改变现有时间轴视觉分组和任务时间。

- [ ] **Step 2: 增加前端测试**

验证时间轴仍按原规则视觉合并，且运行状态能区分“会话等待”和“账号切换”。

- [ ] **Step 3: 同步 wiki、README 和使用教程**

补充：同账号10分钟内等待、跨账号按时间抢占、暂停/休眠后的重新规划、不会提前执行未来任务。

- [ ] **Step 4: 运行前端测试**

运行：

```powershell
bun run test
bun run build
```

### Task 6: 全量质量验证

**Files:**
- No new source files.

- [ ] **Step 1: 运行格式化和静态检查**

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

- [ ] **Step 2: 运行 Rust 全量测试**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 3: 运行项目质量门禁**

```powershell
bun run check
```

- [ ] **Step 4: 汇总验证结果**

记录每条命令的通过/失败结果。失败项不得宣称完成，需保留错误输出和对应任务状态。

## Git 约束

当前工作区存在用户既有未提交修改。本计划执行期间不执行 `git reset`、`git checkout` 或自动提交；如需提交，由用户单独确认。
