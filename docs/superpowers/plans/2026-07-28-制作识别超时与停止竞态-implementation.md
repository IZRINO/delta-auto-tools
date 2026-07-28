# 制作识别超时与停止竞态 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. 本项目固定使用 Inline Execution，不调用子代理。编码前先调用项目级 `ponytail` skill。

**Goal:** 消除制作成功持久化自锁，为全部制作识别增加 30 秒有界等待，并让取消、紧急停止、校准测试和保存遵守同一 active-run 生命周期。

**Architecture:** 保留现有 `LoginRuntime`、`SettingsCoordinator` 和 Tauri command，不引入 actor queue。运行期写入采用 copy-on-write，并在 coordinator closure 返回后发送 revision 事件；模板 observer 增加制作专用 deadline API；runtime 通过 `stopping -> stopped -> bootstrap null` 表达停止收尾。

**Tech Stack:** Rust、Tauri 2、Tokio、React 19、TypeScript、Vitest、Bun、CodeGraph。

---

## 文件职责

- `src-tauri/src/special_ops/template_observer.rs`：模板双采样、取消和 deadline。
- `src-tauri/src/special_ops/craft_runtime.rs`：制作步骤编排、阶段文案、制作专用 30 秒等待。
- `src-tauri/src/special_ops/login_runtime.rs`：active run、`stopping` 状态和最终清理边界。
- `src-tauri/src/special_ops/mod.rs`：制作结果 copy-on-write、Tauri command 门禁、停止 owner、校准空间校验。
- `src/components/app/special-ops-types.ts`：前后端一致的 `LoginRunStatus`。
- `src/components/app/special-ops-utils.ts`：active-run 判断和 final bootstrap 清空规则。
- `src/components/app/special-ops-utils.test.ts`：run snapshot 合并和 active-run 单测。
- `src/components/app/special-ops-page.tsx`：表单、测试、框选和试运行按钮门禁。
- `src/components/app/special-ops-page.test.tsx`：页面门禁源代码回归检查。
- `droid-wiki/features/special-ops.md`：制作超时和停止生命周期文档。

## 执行约束

- 不清理仓库现有未提交修改。
- 每个 Task 只 stage 该 Task 列出的文件。
- 每个行为改动先写失败测试，再写最小实现。
- Task 1 开始前调用 `ponytail`，按其约束避免引入额外 abstraction。
- 大范围探索前使用 `codegraph explore`；完成全部代码改动后运行 `codegraph sync`。

### Task 1：消除制作结果持久化自锁

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:2590-2714`
- Test: `src-tauri/src/special_ops/mod.rs` 内联 tests

- [ ] **Step 1：写制作成功持久化失败测试**

在 `special_ops::tests` 增加两个测试，要求 coordinator 返回新 revision，磁盘写失败时不污染内存 settings：

```rust
#[test]
fn craft_success_persistence_returns_revision_without_reentrant_lock() {
    let coordinator = SettingsCoordinator::new();
    let initial_revision = coordinator.current_revision().unwrap();
    let fixture = LoginFixture::complete();
    let mut initial = fixture.settings;
    initial.accounts[0].stations = vec![station(StationKind::TechnicalCenter, 10_000)];
    let settings = Mutex::new(initial);

    let (_, revision) = persist_craft_success_with(
        &settings,
        &coordinator,
        "selected",
        &StationKind::TechnicalCenter,
        1_000,
        60,
        |_| Ok(()),
    )
    .unwrap();

    assert_eq!(revision, initial_revision + 1);
    let stored = settings.lock().unwrap();
    let station = stored.accounts[0]
        .stations
        .iter()
        .find(|station| station.kind == StationKind::TechnicalCenter)
        .unwrap();
    assert_eq!(station.started_at_ms, Some(1_000));
    assert_eq!(station.finishes_at_ms, Some(3_601_000));
}

#[test]
fn craft_success_persistence_failure_keeps_memory_unchanged() {
    let coordinator = SettingsCoordinator::new();
    let fixture = LoginFixture::complete();
    let mut initial = fixture.settings;
    initial.accounts[0].stations = vec![station(StationKind::TechnicalCenter, 10_000)];
    let settings = Mutex::new(initial);
    let before = settings.lock().unwrap().clone();

    let error = persist_craft_success_with(
        &settings,
        &coordinator,
        "selected",
        &StationKind::TechnicalCenter,
        1_000,
        60,
        |_| Err("测试保存失败".to_string()),
    )
    .unwrap_err();

    assert_eq!(error, "测试保存失败");
    assert_eq!(*settings.lock().unwrap(), before);
}
```

- [ ] **Step 2：运行测试并确认失败**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml craft_success_persistence -- --nocapture
```

Expected: FAIL，`persist_craft_success_with` 或 `settings_with_enabled_station` 尚未定义。

- [ ] **Step 3：增加 copy-on-write helper**

在 `mod.rs` 增加可测试 helper。closure 内不允许调用 `coordinator.current_revision()`：

```rust
fn persist_craft_success_with<F>(
    settings: &Mutex<SpecialOpsSettings>,
    coordinator: &SettingsCoordinator,
    account_id: &str,
    station: &StationKind,
    started_at_ms: i64,
    duration_minutes: u32,
    persist: F,
) -> Result<(SpecialOpsSettings, u64), String>
where
    F: FnOnce(&SpecialOpsSettings) -> Result<(), String>,
{
    coordinator.with_runtime_change(|| {
        let current = settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        let mut next = current;
        let account = next
            .accounts
            .iter_mut()
            .find(|account| account.id == account_id)
            .ok_or_else(|| "制作账号不存在".to_string())?;
        let station_plan = account
            .stations
            .iter_mut()
            .find(|candidate| candidate.kind == *station)
            .ok_or_else(|| "制作台不存在".to_string())?;
        station_plan.started_at_ms = Some(started_at_ms);
        station_plan.finishes_at_ms = Some(
            started_at_ms.saturating_add(i64::from(duration_minutes) * 60_000),
        );
        station_plan.status = StationStatus::Crafting;
        persist(&next)?;
        *settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok(next)
    })
}
```

`run_craft_worker` 在调用 helper 前冻结 `account_id`。helper 返回后再发送事件：

```rust
persist_craft_success_with(
    &settings,
    &coordinator,
    &account_id,
    &station,
    started_at_ms,
    duration_minutes,
    |next| save_settings(&app, next),
)
.map(|(next, revision)| {
    emit_state(&app, &build_bootstrap(next, revision, now_ms()));
})
```

helper 返回错误时，`run_craft_worker` 的最终 message 固定包含：`制作已开始但完成时间保存失败，必须人工确认并修正完成时间`。不得把本轮报告为成功，也不得自动启动同一制作台的新试运行。

- [ ] **Step 4：运行定向 Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml craft_success_persistence -- --nocapture
```

Expected: 2 tests PASS；命令在 5 秒内结束。

- [ ] **Step 5：提交**

```powershell
git add -- src-tauri/src/special_ops/mod.rs
git commit -m "fix(special-ops): 消除制作结果持久化自锁"
```

### Task 2：为模板 observer 增加 deadline 和诊断样本

**Files:**
- Modify: `src-tauri/src/special_ops/template_observer.rs:10-216`
- Test: `src-tauri/src/special_ops/template_observer.rs` 内联 tests

- [ ] **Step 1：写单目标超时、任意目标超时、取消优先测试**

```rust
struct ConstantSampler(f32);

impl SimilaritySampler for ConstantSampler {
    async fn sample(&self, _: &RuntimeTemplate) -> Result<f32, String> {
        Ok(self.0)
    }
}

#[tokio::test(start_paused = true)]
async fn bounded_single_target_reports_last_samples() {
    let sampler = ConstantSampler(0.42);
    let error = wait_for_consistent_match_until(
        &sampler,
        &target(),
        Arc::new(AtomicBool::new(false)),
        Duration::from_millis(900),
    )
    .await
    .unwrap_err();

    assert!(error.contains("test-target"));
    assert!(error.contains("threshold=0.8000"));
    assert!(error.contains("samples="));
}

#[tokio::test(start_paused = true)]
async fn bounded_any_target_reports_each_last_pair() {
    let sampler = ConstantSampler(0.2);
    let mut first = target();
    first.key = "produce".to_string();
    let mut second = target();
    second.key = "fill".to_string();
    let error = wait_for_any_consistent_match_until(
        &sampler,
        &[&first, &second],
        Arc::new(AtomicBool::new(false)),
        Duration::from_millis(900),
    )
    .await
    .unwrap_err();

    assert!(error.contains("produce"));
    assert!(error.contains("fill"));
}

#[tokio::test(start_paused = true)]
async fn cancellation_wins_over_deadline() {
    let cancelled = Arc::new(AtomicBool::new(true));
    let error = wait_for_consistent_match_until(
        &ScriptedSampler::new([]),
        &target(),
        cancelled,
        Duration::from_secs(30),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "模板识别已取消");
}
```

- [ ] **Step 2：运行测试并确认失败**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml bounded_ -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml cancellation_wins_over_deadline -- --nocapture
```

Expected: FAIL，两个 `_until` API 尚未定义。

- [ ] **Step 3：实现 deadline API**

保留现有无界 API供登录流程使用；新增制作专用 API：

```rust
pub(crate) async fn wait_for_consistent_match_until<S: SimilaritySampler>(
    sampler: &S,
    target: &RuntimeTemplate,
    cancelled: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<TemplateObservation, String>;

pub(crate) async fn wait_for_target_match_until<S: SimilaritySampler>(
    sampler: &S,
    target: &RuntimeTarget,
    cancelled: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<TemplateObservation, String>;

pub(crate) async fn wait_for_any_consistent_match_until<S: SimilaritySampler>(
    sampler: &S,
    targets: &[&RuntimeTemplate],
    cancelled: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<(String, TemplateObservation), String>;
```

实现使用 `tokio::time::Instant` deadline。每次采样和 400ms 间隔前依次检查：取消、deadline、采样。deadline 错误格式固定：

```rust
fn timeout_message(targets: &[(&RuntimeTemplate, [f32; 2])]) -> String {
    let details = targets
        .iter()
        .map(|(target, samples)| format!(
            "{} threshold={:.4} samples=[{:.4}, {:.4}]",
            target.key, target.threshold, samples[0], samples[1]
        ))
        .collect::<Vec<_>>()
        .join("; ");
    format!("模板识别超时：{details}")
}
```

采样本身必须放进 `tokio::select!`，deadline 到达时不能继续等待 `spawn_blocking` join handle。

- [ ] **Step 4：运行 observer tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops::template_observer::tests -- --nocapture
```

Expected: PASS，旧无界 observer tests 同时保持通过。

- [ ] **Step 5：提交**

```powershell
git add -- src-tauri/src/special_ops/template_observer.rs
git commit -m "feat(special-ops): 制作识别支持有界等待"
```

### Task 3：制作流程使用有界等待并发布准确阶段

**Files:**
- Modify: `src-tauri/src/special_ops/craft_runtime.rs:10-490`
- Modify: `src-tauri/src/special_ops/craft_trial.rs:13-167`
- Test: `src-tauri/src/special_ops/craft_runtime.rs` 内联 tests
- Test: `src-tauri/src/special_ops/craft_trial.rs` 内联 tests

- [ ] **Step 1：写 30 秒 deadline、单组中止双采样、阶段文案测试**

扩展 `FakeDriver` 记录 `update_stage`，增加断言：

```rust
#[tokio::test]
async fn idle_flow_publishes_recipe_list_stage_before_waiting() {
    let driver = FakeDriver::new(Some(CraftStationState::Idle));
    let result = run_craft_station(
        &driver,
        StationKind::Pharmacy,
        Arc::new(AtomicBool::new(false)),
    )
    .await;

    assert!(result.is_ok());
    let actions = driver.actions.lock().unwrap();
    let stage = actions
        .iter()
        .position(|value| value == "stage:正在确认制药台制作列表已打开")
        .unwrap();
    let wait = actions
        .iter()
        .position(|value| value == "wait:craft.recipeListReady.pharmacy")
        .unwrap();
    assert!(stage < wait);
}

#[tokio::test]
async fn abort_confirmation_uses_one_driver_call() {
    let driver = FakeDriver::new(Some(CraftStationState::Idle));
    run_craft_station(
        &driver,
        StationKind::TechnicalCenter,
        Arc::new(AtomicBool::new(false)),
    )
    .await
    .unwrap();
    assert_eq!(
        driver.actions.lock().unwrap()
            .iter()
            .filter(|value| value.as_str() == "wait:craft.abort")
            .count(),
        1
    );
}
```

- [ ] **Step 2：运行测试并确认失败**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml idle_flow_publishes_recipe_list_stage -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml abort_confirmation_uses_one_driver_call -- --nocapture
```

Expected: FAIL，当前没有等待前阶段文案，trait 仍使用 `wait_abort_pair`。

- [ ] **Step 3：接入制作专用 deadline**

增加：

```rust
const CRAFT_RECOGNITION_TIMEOUT: Duration = Duration::from_secs(30);
```

`verify`、`wait_ready` 使用 `wait_for_target_match_until`；`wait_button` 使用 `wait_for_any_consistent_match_until`。trait 将：

```rust
async fn wait_abort_pair(&self, cancelled: Arc<AtomicBool>) -> Result<i64, String>;
```

改为：

```rust
async fn wait_abort(&self, cancelled: Arc<AtomicBool>) -> Result<i64, String>;
```

实现只调用一次 bounded 双采样：

```rust
template_observer::wait_for_target_match_until(
    &RuntimeSimilaritySampler,
    &runtime_target,
    cancelled,
    CRAFT_RECOGNITION_TIMEOUT,
)
.await?;
Ok(crate::special_ops::now_ms())
```

同步修改 `FakeDriver`：`wait_abort` 只记录一次 `wait:craft.abort`。同时修改 `craft_trial.rs` 的纯状态模型：`Observation::Abort` 表示已经完成标准双采样的确认，因此成功路径只消费一个 `Abort { at_ms }`；现有直接生产和补齐测试各删除第二个 Abort observation。

- [ ] **Step 4：在每次等待前发布真实文案**

在 `run_craft_station` 使用固定映射：

```rust
fn station_label(station: &StationKind) -> &'static str {
    match station {
        StationKind::TechnicalCenter => "技术中心",
        StationKind::Workbench => "工作台",
        StationKind::Pharmacy => "制药台",
        StationKind::ArmorBench => "防具台",
    }
}
```

每次 `wait_ready`、`wait_button`、`wait_abort` 前调用 `update_stage(LoginRunStatus::Waiting, ...)`。初始状态一轮未命中仍直接返回 `当前可能正在制作或状态识别失败`。

- [ ] **Step 5：运行制作 runtime tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops::craft_runtime::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml special_ops::craft_trial::tests -- --nocapture
```

Expected: PASS。

- [ ] **Step 6：提交**

```powershell
git add -- src-tauri/src/special_ops/craft_runtime.rs src-tauri/src/special_ops/craft_trial.rs
git commit -m "fix(special-ops): 限制制作识别并校正阶段提示"
```

### Task 4：引入 stopping 状态并统一停止 owner

**Files:**
- Modify: `src-tauri/src/special_ops/login_runtime.rs:80-400`
- Modify: `src-tauri/src/special_ops/mod.rs:2093-2173, 2986-3081`
- Modify: `src/components/app/special-ops-types.ts:18`
- Test: `src-tauri/src/special_ops/login_runtime.rs` 内联 tests
- Test: `src-tauri/src/special_ops/mod.rs` 内联 tests

- [ ] **Step 1：写 stopping 状态机测试**

```rust
#[test]
fn stop_request_stays_stopping_until_worker_finishes() {
    let runtime = LoginRuntime::default();
    let run = runtime.try_start("account-a".to_string()).unwrap();

    let stopping = runtime
        .request_stop(run.run_id, StopReason::Normal)
        .unwrap()
        .unwrap();
    assert_eq!(stopping.status, LoginRunStatus::Stopping);
    assert!(runtime.snapshot().unwrap().is_some());

    let stopped = runtime
        .finish(run.run_id, LoginRunStatus::Failed, "制作试运行已停止")
        .unwrap()
        .unwrap();
    assert_eq!(stopped.status, LoginRunStatus::Stopped);
    assert!(runtime.snapshot().unwrap().is_none());
}
```

增加 emergency test：停止 command 只请求停止并调用输入释放，不执行 `persist_login_outcome` 或 `release_login_resources_for_run`。

- [ ] **Step 2：运行测试并确认失败**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml stop_request_stays_stopping -- --nocapture
```

Expected: FAIL，`Stopping` 尚不存在，当前请求直接返回 `Stopped`。

- [ ] **Step 3：增加枚举并改变 request_stop**

Rust：

```rust
pub enum LoginRunStatus {
    Starting,
    Waiting,
    Countdown,
    Inputting,
    Stopping,
    Succeeded,
    Failed,
    Stopped,
}
```

`request_stop_locked` 设置 `Stopping`；`finish` 发现 `stop_reason` 时才设置 `Stopped`。TypeScript 同步：

```typescript
export type LoginRunStatus = "starting" | "waiting" | "countdown" | "inputting" | "stopping" | "succeeded" | "failed" | "stopped";
```

同步更新 `login_runtime.rs` 和 `mod.rs` 中所有“request 后立即等于 `Stopped`”的既有测试：停止请求断言改为 `Stopping`，只有 `finish` 返回值断言 `Stopped`。

- [ ] **Step 4：停止 command 只登记请求**

普通取消保持 request-only。紧急停止改为：

```rust
pub fn special_ops_emergency_stop(app: AppHandle) -> Result<LoginRunSnapshot, AppError> {
    let snapshot = request_emergency_stop_core(&app)?;
    crate::input_simulation::release_tracked_injected_inputs()?;
    Ok(snapshot)
}
```

删除 command 线程中的 `complete_emergency_stop_core` 持久化和资源清理。`register_emergency_hotkey` 不再调用 `request_then_schedule_emergency`；热键 callback 与 Tauri command 都只调用 `request_emergency_stop_core`。该 core 设置取消 flag 后立即调用 `release_tracked_injected_inputs`。最终 settings 持久化、热键清理、operation window 销毁只由 worker 的既有收尾路径执行一次。

生命周期停止是例外：`stop_registered` 继续作为同步 fail-closed owner，使用 persistence claim 完成一次保存和资源释放，然后移除 active run；被取消 worker 后续看到 stale run 后不得再次写入或释放资源。

制作 worker 不能沿用当前 `Some(_) => Ok(())` 分支。新增测试并统一安全判断：

```rust
fn should_mark_craft_stop_uncertain(
    stop_reason: Option<login_runtime::StopReason>,
    entered_input: bool,
) -> bool {
    match stop_reason {
        Some(login_runtime::StopReason::Normal) => entered_input,
        Some(login_runtime::StopReason::Emergency) => true,
        Some(login_runtime::StopReason::Lifecycle { uncertain }) => uncertain,
        None => false,
    }
}
```

满足条件时由 craft worker 调用 `mark_craft_cancel_uncertain`，以 copy-on-write 方式保存账号和当前制作台 `Uncertain`。增加 `craft_emergency_stop_marks_account_and_station_uncertain` 测试，断言 `settings.paused == true`、账号和制作台均为 `Uncertain`。

- [ ] **Step 5：运行停止与持久化 tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops::login_runtime::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml persistence -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml emergency_stop -- --nocapture
```

Expected: PASS；普通取消和紧急停止各只有一次持久化 owner、一次最终 cleanup。

- [ ] **Step 6：提交**

```powershell
git add -- src-tauri/src/special_ops/login_runtime.rs src-tauri/src/special_ops/mod.rs src/components/app/special-ops-types.ts
git commit -m "fix(special-ops): 停止请求等待 worker 完成收尾"
```

### Task 5：active run 前后端门禁与 snapshot 清空

**Files:**
- Modify: `src/components/app/special-ops-utils.ts:23-69`
- Modify: `src/components/app/special-ops-utils.test.ts`
- Modify: `src/components/app/special-ops-page.tsx:123-675`
- Modify: `src/components/app/special-ops-page.test.tsx`
- Modify: `src-tauri/src/special_ops/mod.rs:2794-3418`
- Test: above files

- [ ] **Step 1：写前端 active-run 与 authoritative null 测试**

```typescript
it("stopping 与 final snapshot 在 bootstrap 清空前均保持 active", () => {
    expect(hasActiveSpecialOpsRun({...runSnapshot(10), status: "stopping"})).toBe(true);
    expect(hasActiveSpecialOpsRun({...runSnapshot(11), status: "stopped"})).toBe(true);
    expect(hasActiveSpecialOpsRun(null)).toBe(false);
});

it("已结束 run 可被同 revision authoritative bootstrap null 清空", () => {
    const current = bootstrap(8, {
        runSnapshot: {...runSnapshot(50), status: "stopped"},
    });
    const incoming = bootstrap(8, {runSnapshot: null});

    expect(mergeSpecialOpsBootstrap(current, incoming).runSnapshot).toBeNull();
});

it("运行中的 run 不被旧 null response 清空", () => {
    const current = bootstrap(8, {runSnapshot: runSnapshot(50)});
    const incoming = bootstrap(8, {runSnapshot: null});

    expect(mergeSpecialOpsBootstrap(current, incoming).runSnapshot).toBe(current.runSnapshot);
});
```

- [ ] **Step 2：运行前端 tests 并确认失败**

Run:

```powershell
bunx vitest run src/components/app/special-ops-utils.test.ts
```

Expected: FAIL，`hasActiveSpecialOpsRun` 尚未定义，terminal null 仍被旧 snapshot 覆盖。

- [ ] **Step 3：实现 active helper 和 null 清理规则**

```typescript
const terminalRunStatuses = new Set<LoginRunSnapshot["status"]>(["succeeded", "failed", "stopped"]);

export function hasActiveSpecialOpsRun(snapshot: LoginRunSnapshot | null): boolean {
    return snapshot !== null;
}

function latestRunSnapshot(current: LoginRunSnapshot | null, incoming: LoginRunSnapshot | null) {
    if (incoming === null) {
        return current && terminalRunStatuses.has(current.status) ? null : current;
    }
    if (current === null) return incoming;
    if (current.runId > incoming.runId) return current;
    if (current.runId === incoming.runId && current.updatedAtMs > incoming.updatedAtMs) return current;
    return incoming;
}
```

- [ ] **Step 4：前端所有入口统一使用 `hasActiveRun`**

`SpecialOpsPage`：

```typescript
const runSnapshot = bootstrap.runSnapshot;
const hasActiveRun = hasActiveSpecialOpsRun(runSnapshot);
```

试运行启动、设置输入、总开关、暂停、热键录制、文件选择、模板上传/删除/测试、框选按钮全部使用 `disabled={hasActiveRun || ...}`。`save`、`beginCalibration`、`pickReferenceImage`、`testCalibrationTarget` 函数入口也快速 return，防止只靠按钮属性。

取消按钮使用：

```tsx
<Button
    disabled={!hasActiveRun || runSnapshot?.status === "stopping"}
    variant="outline"
    onClick={() => void cancelLoginTrial()}
>
```

- [ ] **Step 5：后端 command 增加快速门禁**

增加共用 helper：

```rust
fn ensure_no_active_special_ops_run(runtime: &login_runtime::LoginRuntime) -> Result<(), String> {
    if runtime.snapshot()?.is_some() {
        return Err("特勤处试运行尚未完成清理".to_string());
    }
    Ok(())
}
```

在 settings save、模板 test、calibration begin/submit/cancel 和三类 start command 执行业务前调用。start command 仍依赖 `try_start_kind` 作为最终单实例保护。

- [ ] **Step 6：运行前后端门禁 tests**

Run:

```powershell
bunx vitest run src/components/app/special-ops-utils.test.ts src/components/app/special-ops-page.test.tsx
cargo test --manifest-path src-tauri/Cargo.toml active_special_ops_run -- --nocapture
```

Expected: PASS。

- [ ] **Step 7：提交**

```powershell
git add -- src/components/app/special-ops-utils.ts src/components/app/special-ops-utils.test.ts src/components/app/special-ops-page.tsx src/components/app/special-ops-page.test.tsx src-tauri/src/special_ops/mod.rs
git commit -m "fix(special-ops): 运行收尾前锁定配置与校准"
```

### Task 6：拒绝明显跨制作台点击点

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:3337-3418`
- Test: `src-tauri/src/special_ops/mod.rs` 内联 tests

- [ ] **Step 1：写跨台拒绝和正确区域允许测试**

```rust
#[test]
fn station_click_point_inside_other_station_envelope_is_rejected() {
    let mut settings = SpecialOpsSettings::default();
    set_station_state_rects(&mut settings, StationKind::TechnicalCenter, 100, 100);
    set_station_state_rects(&mut settings, StationKind::Pharmacy, 100, 500);

    let error = validate_station_click_point(
        &settings.calibration_environments[0],
        "craft.station.pharmacy",
        &CalibrationRect {x: 120, y: 140, width: 1, height: 1},
    )
    .unwrap_err();

    assert!(error.contains("疑似误点到技术中心"));
}

#[test]
fn station_click_point_inside_own_envelope_is_allowed() {
    let mut settings = SpecialOpsSettings::default();
    set_station_state_rects(&mut settings, StationKind::TechnicalCenter, 100, 100);
    set_station_state_rects(&mut settings, StationKind::Pharmacy, 100, 500);

    validate_station_click_point(
        &settings.calibration_environments[0],
        "craft.station.pharmacy",
        &CalibrationRect {x: 120, y: 540, width: 1, height: 1},
    )
    .unwrap();
}
```

测试模块增加明确 helper，不依赖隐藏 fixture：

```rust
fn set_station_state_rects(
    settings: &mut SpecialOpsSettings,
    station: StationKind,
    x: i32,
    y: i32,
) {
    let suffix = match station {
        StationKind::TechnicalCenter => "technicalCenter",
        StationKind::Workbench => "workbench",
        StationKind::Pharmacy => "pharmacy",
        StationKind::ArmorBench => "armorBench",
    };
    calibration_target_mut(settings, &format!("craft.claimReady.{suffix}")).rect = Some(
        CalibrationRect {x, y, width: 200, height: 40},
    );
    calibration_target_mut(settings, &format!("craft.idle.{suffix}")).rect = Some(
        CalibrationRect {x, y: y + 80, width: 200, height: 40},
    );
}
```

- [ ] **Step 2：运行测试并确认失败**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml station_click_point_inside_ -- --nocapture
```

Expected: FAIL，空间校验 helper 尚未定义。

- [ ] **Step 3：实现状态区域包络**

```rust
fn rect_union(left: &CalibrationRect, right: &CalibrationRect) -> CalibrationRect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = left.x.saturating_add(left.width)
        .max(right.x.saturating_add(right.width));
    let bottom_edge = left.y.saturating_add(left.height)
        .max(right.y.saturating_add(right.height));
    CalibrationRect {
        x,
        y,
        width: right_edge.saturating_sub(x),
        height: bottom_edge.saturating_sub(y),
    }
}

fn contains_center(envelope: &CalibrationRect, point: &CalibrationRect) -> bool {
    let x = point.x + point.width / 2;
    let y = point.y + point.height / 2;
    x >= envelope.x
        && x <= envelope.x + envelope.width
        && y >= envelope.y
        && y <= envelope.y + envelope.height
}
```

`validate_station_click_point` 解析 `craft.station.<suffix>`，收集四台各自 claim/idle union。点击点只命中其他台、未命中自己的包络时返回错误；目标状态区域不完整时跳过该台比较。

- [ ] **Step 4：在 submit command 保存前调用校验**

`special_ops_submit_calibration_selection` 在 `validate_calibration_selection` 之后、写入 target rect 之前调用：

```rust
validate_station_click_point(environment, &target_key, &region)?;
```

错误时不修改旧 rect、不递增 revision。

- [ ] **Step 5：运行校准 tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml station_click_point_inside_ -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml calibration_selection -- --nocapture
```

Expected: PASS。

- [ ] **Step 6：提交**

```powershell
git add -- src-tauri/src/special_ops/mod.rs
git commit -m "fix(special-ops): 阻止制作台点击点跨台误校准"
```

### Task 7：同步文档并执行全量门禁

**Files:**
- Modify: `droid-wiki/features/special-ops.md`
- Modify only if implementation changes documented quick reference: `AGENTS.md`

- [ ] **Step 1：更新 special-ops wiki**

补充以下确定行为：

```markdown
- 制作模板等待单步骤最长 30 秒；超时报告目标 key、threshold 与最后双采样 similarity。
- `craft.abort` 使用一次 400ms 间隔双采样确认制作开始。
- 取消先进入 `stopping`；worker 保存安全状态并释放资源后才进入 `stopped`。
- active run 清空前，设置保存、校准框选和模板测试均快速拒绝。
- 制作台点击点若明显落入其他制作台状态区域，校准提交失败并保留旧坐标。
```

- [ ] **Step 2：运行格式与定向测试**

Run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
bunx vitest run src/components/app/special-ops-utils.test.ts src/components/app/special-ops-page.test.tsx
cargo test --manifest-path src-tauri/Cargo.toml special_ops -- --nocapture
```

Expected: 全部 PASS。

- [ ] **Step 3：运行全量质量门禁**

Run:

```powershell
bun run check
```

Expected: TypeScript、Vitest、coverage、Rust fmt、Clippy `-D warnings`、Rust tests 全部 PASS。失败时保留完整错误，不声明完成。

- [ ] **Step 4：刷新 CodeGraph 并复核调用链**

Run:

```powershell
codegraph sync
codegraph explore "run_craft_worker persist_craft_success_with wait_for_target_match_until special_ops_emergency_stop hasActiveSpecialOpsRun"
```

Expected: `emit_state` 位于 `with_runtime_change` 返回之后；制作 driver 只调用 bounded observer；停止 command 不再执行结果持久化。

- [ ] **Step 5：检查 diff 范围**

Run:

```powershell
git status --short
git diff --check
git diff --stat
```

Expected: 无 whitespace error；不包含用户无关文件。

- [ ] **Step 6：提交文档和最终修正**

```powershell
git add -- droid-wiki/features/special-ops.md
git commit -m "docs(special-ops): 记录制作超时与停止边界"
```

若 `AGENTS.md` 因最终行为变化需要同步，必须与 wiki 一并 stage；否则保留用户当前 `AGENTS.md` 修改，不纳入本任务提交。

## 手工验收顺序

全量门禁通过后才执行以下操作：

1. 重启开发版，清除旧进程中不可恢复的 revision 自锁。
2. 用“空闲中”的制作台试运行：进入列表、选配方、生产、识别一次双采样“中止”、保存完成时间、自动结束。
3. 用参考图不匹配的测试环境验证：30 秒内失败并显示 key、threshold、两次 similarity。
4. 在识别等待期间点击普通取消：立即显示“正在取消”，框选和测试保持禁用；worker 清理后恢复。
5. 在键鼠阶段触发紧急停止：输入立即释放，状态进入 `stopping`，清理完成后恢复操作。
6. 把制药台点击点误选到技术中心状态包络：提交被拒绝；重新选到制药台后允许保存。
