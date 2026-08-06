# 单账号制作试运行 Implementation Plan

> **For agentic workers:** Inline Execution。本计划按 TDD 分批执行，不创建子代理或 worktree。

**Goal:** 在现有特勤处 runtime 中加入单账号、单制作台收取并重做试运行闭环。

**Architecture:** 新增独立 `craft_trial` 纯状态机，输入抽象为等待/点击/按键/记录时间动作；生产 runtime 通过适配器把状态机动作映射到现有 `RuntimeTarget`、双采样、倒计时和 Windows 输入。`mod.rs` 只负责配置快照、run 生命周期、持久化与 Tauri command，前端只选择账号/制作台并显示 run snapshot。

**Tech Stack:** Rust、Tokio、Serde、Tauri 2、React 19、TypeScript、Vitest。

---

### Task 1: 建立制作状态机失败测试

**Files:**
- Create: `src-tauri/src/special_ops/craft_trial.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`（仅增加 `mod craft_trial;`）

- [ ] **Step 1: 写失败测试**

在 `craft_trial.rs` 中先定义测试所需的最小类型和测试动作记录，测试以下行为：

```rust
#[test]
fn 直接生产时按收取到中止顺序推进并计算完成时间() { /* 断言 actions 与 finishes_at_ms */ }

#[test]
fn 缺材料时补齐购买再生产() { /* 断言 fill -> purchase -> produce */ }

#[test]
fn 收取后必须先看到空闲才能进入配方列表() { /* 未 idle 时返回失败 */ }

#[test]
fn 中止双采样第二次时间作为开始时间() { /* started_at_ms 等于第二次 abort 采样时间 */ }

#[test]
fn 识别不一致时不点击不记录状态() { /* action 为空且结果为失败 */ }
```

- [ ] **Step 2: 运行测试确认按预期失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml craft_trial -- --nocapture`

Expected: 编译失败，原因是制作状态机类型和运行函数尚未实现。

### Task 2: 实现最小纯状态机

**Files:**
- Modify: `src-tauri/src/special_ops/craft_trial.rs`

- [ ] **Step 1: 定义纯接口**

实现 `CraftStation`、`CraftAction`、`CraftObservation`、`CraftTrialResult`，动作只包含 `Click(target_key)`、`PressSpace`、`RecordStartedAt(i64)`；观察输入包含 `Claimed`、`Reward`、`Idle`、`RecipeListReady`、`RecipeSelected`、`FillVisible`、`PurchaseVisible`、`ProduceVisible`、`Abort`。

- [ ] **Step 2: 实现状态推进**

实现 `run_craft_trial(observations, duration_minutes)`：按设计顺序消费观察，直接生产路径跳过补齐；补齐路径必须经历购买；任何顺序错误或观察缺失返回 `CraftTrialFailure { step, message }`；第二次 `Abort` 采样时间写入 `started_at_ms`。

- [ ] **Step 3: 运行状态机测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml craft_trial -- --nocapture`

Expected: Task 1 测试全部通过。

### Task 3: 接入 RuntimeTarget 驱动

**Files:**
- Create: `src-tauri/src/special_ops/craft_runtime.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1: 写驱动测试**

使用 fake target sampler 和 fake input driver，验证每个点击前调用现有双采样守卫；`craft.fill` 命中时执行购买分支；取消标记在倒计时、等待和动作期间均能返回 `EmergencyStopped`。

- [ ] **Step 2: 实现驱动适配器**

复用 `game_navigation.rs` 的 `RuntimeSimilaritySampler`、`WindowsDesktopRuntime`、`click_region_center_cancellable`、`press_named_key_cancellable` 和 3 秒倒计时模式。制作台点击目标使用 `craft.station.<suffix>`，配方使用 `craft.recipe.<suffix>`，状态模板使用对应 `craft.*.<suffix>`。

- [ ] **Step 3: 运行驱动测试与现有导航测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::craft_runtime special_ops::game_navigation -- --nocapture`

Expected: 新测试与既有导航测试通过。

### Task 4: 接入单实例 Tauri command 与持久化

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src/components/app/special-ops-types.ts`

- [ ] **Step 1: 写 command 契约测试**

在现有 Rust/前端静态契约测试中断言存在 `special_ops_start_craft_trial`，参数包含 `accountId`、`stationKind`、`settingsRevision`，且 command 不接收密码。

- [ ] **Step 2: 实现 command**

新增 `special_ops_start_craft_trial`：校验 revision、账号和启用制作台，冻结校准与时长配置；保持主工具窗口可见，复用 operation window、紧急热键和 `LoginRuntime` 单实例；worker 完成后通过 `special-ops://run-changed` 返回 snapshot。动态制作模板无需预先测试签名，运行期仍执行双采样。

- [ ] **Step 3: 成功持久化制作计时**

仅在 `craft.abort` 双采样成功后，通过 `SettingsCoordinator::with_runtime_change` 更新对应 `StationPlan.startedAtMs`、`finishesAtMs`、`status = Crafting`。失败、取消、紧急停止不覆盖既有计时；紧急停止沿用账号 `Uncertain` 语义。

- [ ] **Step 4: 注册并验证 command**

在 `src-tauri/src/lib.rs` 加入 handler；在 `default.json` 增加对应 command permission；运行 Rust command 契约测试。

### Task 5: 前端试运行入口

**Files:**
- Modify: `src/components/app/special-ops-page.tsx`
- Modify: `src/components/app/special-ops-page.test.tsx`

- [ ] **Step 1: 写前端失败测试**

断言页面包含单独的“制作试运行”入口、制作台选择器、调用 `special_ops_start_craft_trial`，运行中禁用重复启动，不新增制作物品名称输入。

- [ ] **Step 2: 实现最小 UI**

复用现有账号选择、`stationKinds`、`runSnapshot`、错误提示和刷新逻辑；新增选定制作台状态及启动函数，启动前 `flushSettings()` 并传入保存后的 revision。

- [ ] **Step 3: 运行前端测试**

Run: `bunx vitest run src/components/app/special-ops-page.test.tsx`

Expected: 前端契约测试通过。

### Task 6: 文档与质量门禁

**Files:**
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `README.md`（仅在功能入口说明确实存在时补充）
- Modify: `AGENTS.md`（仅在 command/持久化结构变化需要同步时补充）

- [ ] **Step 1: 更新 Wiki**

记录单账号单制作台试运行范围、状态机流程、成功计时和当前未覆盖的多账号/兑换边界。

- [ ] **Step 2: 同步 CodeGraph**

Run: `codegraph sync`

Expected: index 更新成功。

- [ ] **Step 3: 运行完整检查**

Run: `bun run check`

Expected: TypeScript、Vitest coverage、Rust fmt、Clippy、Rust tests 全部通过。

- [ ] **Step 4: 检查 diff**

Run: `git diff --check`

Expected: 无空白错误。

## 自检

- 设计中的收取、空闲、配方、补齐、购买、生产、中止步骤均有对应状态机测试和 runtime 任务。
- `startedAtMs` 只在 `abort` 双采样成功后写入。
- 未引入密码、自动换配方、多账号或子弹兑换。
- command 注册、capability、前端入口、Wiki 同步均在任务中覆盖。
