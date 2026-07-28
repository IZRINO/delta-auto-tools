# 制作台“制作中”负向守卫 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用共享“制作中”模板替代不可靠的空闲/感叹号前置分类，未命中后统一点击制作台，并按奖励页或制作列表结果分流。

**Architecture:** `template_observer.rs` 提供可区分“连续命中、连续未命中、采样不一致、系统错误”的一次性采样，以及保留最后样本的有界多目标等待。`mod.rs` 在 active `CalibrationEnvironment` 保存一次共享参考图与 threshold，四个 `craft.inProgress.*` target 只保存独立 rect。`craft_runtime.rs` 编排负向守卫、最多三次点击和奖励/列表分流；worker 通过明确 outcome 决定不写状态、保存新完成时间或持久化 `Uncertain`。

**Tech Stack:** Rust、Tokio、Tauri 2、serde、React 19、TypeScript、Vitest、Bun。

**Execution constraint:** 使用 Inline Execution，不调用子代理，不创建 worktree。当前目标文件含历史未提交修改；每个 Task 完成后做定向验证，但不得整文件 `git add` 形成混合提交。仅计划文档独立提交，功能代码在现有分支连续开发并最终统一复核。

---

### Task 1: 增加可安全用于负向守卫的模板观察原语

**Files:**
- Modify: `src-tauri/src/special_ops/template_observer.rs:30-275`
- Test: `src-tauri/src/special_ops/template_observer.rs` 内联 tests

- [ ] **Step 1: 调用编码前必需 skill**

调用项目要求的 `ponytail`，选择最小实现；随后调用 `superpowers:test-driven-development`。不得先改生产代码。

- [ ] **Step 2: 写一次性单目标分类失败测试**

```rust
#[tokio::test]
async fn single_consistency_distinguishes_match_absence_and_mismatch() {
    let template = target();
    let matched = sample_single_consistent_once(
        &SequenceSampler::new([0.91, 0.92]),
        &template,
        Arc::new(AtomicBool::new(false)),
    ).await.unwrap();
    assert!(matches!(matched, SingleConsistency::Matched { .. }));

    let absent = sample_single_consistent_once(
        &SequenceSampler::new([0.21, 0.20]),
        &template,
        Arc::new(AtomicBool::new(false)),
    ).await.unwrap();
    assert!(matches!(absent, SingleConsistency::NotMatched { .. }));

    let error = sample_single_consistent_once(
        &SequenceSampler::new([0.91, 0.20]),
        &template,
        Arc::new(AtomicBool::new(false)),
    ).await.unwrap_err();
    assert!(error.contains("两次采样不一致"));
}
```

再加一个 sampler 返回 `Err("截取识别区域失败")` 的测试，断言错误原样传播，不能返回 `NotMatched`。

- [ ] **Step 3: 验证 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib single_consistency_distinguishes_match_absence_and_mismatch`

Expected: FAIL，`sample_single_consistent_once` 与 `SingleConsistency` 尚不存在。

- [ ] **Step 4: 写最小单目标分类实现**

```rust
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SingleConsistency {
    Matched { samples: [f32; 2] },
    NotMatched { samples: [f32; 2] },
}

pub(crate) async fn sample_single_consistent_once<S: SimilaritySampler>(
    sampler: &S,
    target: &RuntimeTemplate,
    cancelled: Arc<AtomicBool>,
) -> Result<SingleConsistency, String> {
    ensure_not_cancelled(&cancelled)?;
    let first = sample_cancellable(sampler, target, &cancelled).await?;
    wait_for_sample_interval(&cancelled).await?;
    let second = sample_cancellable(sampler, target, &cancelled).await?;
    ensure_not_cancelled(&cancelled)?;
    match (first >= target.threshold, second >= target.threshold) {
        (true, true) => Ok(SingleConsistency::Matched { samples: [first, second] }),
        (false, false) => Ok(SingleConsistency::NotMatched { samples: [first, second] }),
        _ => Err(format!(
            "模板 {} 两次采样不一致：{first:.4} / {second:.4}",
            target.key
        )),
    }
}
```

- [ ] **Step 5: 写有界多目标等待的结构化 timeout 失败测试**

```rust
#[tokio::test(start_paused = true)]
async fn bounded_any_match_returns_timeout_with_last_samples() {
    let template = target();
    let result = try_wait_for_any_consistent_match_until(
        &ConstantSampler(0.2),
        &[&template],
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(1),
    ).await.unwrap();
    match result {
        BoundedAnyMatch::TimedOut { last_samples } => {
            assert_eq!(last_samples[0].0, "test-target");
            assert_eq!(last_samples[0].1, [0.2, 0.2]);
        }
        other => panic!("应返回结构化 timeout，实际为 {other:?}"),
    }
}
```

- [ ] **Step 6: 实现结构化有界等待并保持旧 API**

```rust
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BoundedAnyMatch {
    Matched { key: String, observation: TemplateObservation },
    TimedOut { last_samples: Vec<(String, [f32; 2])> },
}
```

把现有 `wait_for_any_consistent_match_until` 循环主体移入 `try_wait_for_any_consistent_match_until`。到 deadline 时返回 `TimedOut`；采样错误仍返回 `Err`。旧函数调用新函数，`Matched` 转旧 tuple，`TimedOut` 转现有 timeout 文本，避免影响其他调用方。

- [ ] **Step 7: 验证 Task 1**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::template_observer::tests`

Expected: 全部 PASS；负向结果只来自两个有效低分样本。

---

### Task 2: 迁移为一张共享模板和四个制作中区域

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:140-545`
- Modify: `src-tauri/src/special_ops/mod.rs:899-980`
- Modify: `src-tauri/src/special_ops/mod.rs:1592-1704`
- Modify: `src-tauri/src/special_ops/mod.rs:3590-3675`
- Modify: `src/components/app/special-ops-types.ts:13-17`
- Test: `src-tauri/src/special_ops/mod.rs` 内联 tests

- [ ] **Step 1: 写 serde 默认值和迁移失败测试**

```rust
#[test]
fn legacy_calibration_replaces_claim_and_idle_with_in_progress_regions() {
    let normalized = normalize_settings(SpecialOpsSettings::default()).unwrap();
    let environment = &normalized.calibration_environments[0];
    assert!(environment.craft_in_progress_reference_image_path.is_none());
    assert_eq!(environment.craft_in_progress_match_threshold, default_match_threshold());
    for suffix in ["technicalCenter", "workbench", "pharmacy", "armorBench"] {
        assert!(environment.targets.iter().any(|target| {
            target.key == format!("craft.inProgress.{suffix}")
                && target.kind == CalibrationTargetKind::RecognitionRegion
        }));
    }
    assert!(!environment.targets.iter().any(|target| {
        target.key.starts_with("craft.claimReady.") || target.key.starts_with("craft.idle.")
    }));
}
```

增加 JSON 缺少共享字段的反序列化测试，断言路径为 `None`、threshold 为 `0.75`。

- [ ] **Step 2: 验证 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib legacy_calibration_replaces_claim_and_idle_with_in_progress_regions`

Expected: FAIL，共享字段和 `craft.inProgress.*` 尚不存在。

- [ ] **Step 3: 扩展 CalibrationEnvironment 与前端类型**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationEnvironment {
    pub id: String,
    pub name: String,
    pub monitor: String,
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub dpi_scale: f64,
    pub window_mode: String,
    #[serde(default)]
    pub craft_in_progress_reference_image_path: Option<String>,
    #[serde(default = "default_match_threshold")]
    pub craft_in_progress_match_threshold: f32,
    pub targets: Vec<CalibrationTarget>,
}
```

`default_calibration_environment()` 写入 `None` 与 `default_match_threshold()`。TypeScript 的 `CalibrationEnvironment` 同步增加 `craftInProgressReferenceImagePath: string | null` 与 `craftInProgressMatchThreshold: number`。

- [ ] **Step 4: 替换默认目标和点击守卫**

删除八个 `craft.claimReady.*` / `craft.idle.*`，加入：

```rust
("craft.inProgress.technicalCenter", "技术中心制作中状态区域", RecognitionRegion),
("craft.inProgress.workbench", "工作台制作中状态区域", RecognitionRegion),
("craft.inProgress.pharmacy", "制药台制作中状态区域", RecognitionRegion),
("craft.inProgress.armorBench", "防具台制作中状态区域", RecognitionRegion),
```

四个目标保持 `recognition_method = Template`，但 target 自身路径不作为 runtime 来源。`default_guard_any_of()` 删除 `craft.station.*` 正向守卫；制作台点击只能由新状态机的负向守卫路径调用。

- [ ] **Step 5: 校验共享字段并更新空间包络**

`normalize_settings` 校验共享 threshold 为有限 `0..=1`，把空白共享路径归一化为 `None`。包络改为当前台单个 `craft.inProgress.*` rect：

```rust
fn station_state_envelope(
    environment: &CalibrationEnvironment,
    suffix: &str,
) -> Option<CalibrationRect> {
    environment.targets.iter()
        .find(|target| target.key == format!("craft.inProgress.{suffix}"))
        .and_then(|target| target.rect.clone())
}
```

保留“点击点落入其他台但不落入本台则拒绝”语义。

- [ ] **Step 6: 让模板测试使用共享路径和 threshold**

```rust
fn is_craft_in_progress_target(key: &str) -> bool {
    key.starts_with("craft.inProgress.")
}

fn resolved_template_config<'a>(
    environment: &'a CalibrationEnvironment,
    target: &'a CalibrationTarget,
) -> Result<(&'a str, f32), String> {
    if is_craft_in_progress_target(&target.key) {
        let path = environment.craft_in_progress_reference_image_path.as_deref()
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| "制作中状态尚未上传共享参考图".to_string())?;
        Ok((path, environment.craft_in_progress_match_threshold))
    } else {
        let path = target.reference_image_path.as_deref()
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| format!("{} 尚未上传参考图", target.label))?;
        Ok((path, target.match_threshold))
    }
}
```

`calibration_template_test_input`、校准签名与 `commit_calibration_test_verification` 都通过 environment + target 计算签名；共享图片或 threshold 改变后，四个制作中区域旧验证签名失效。

- [ ] **Step 7: 验证 Task 2**

先补 preflight 测试：共享路径为空、文件不存在、当前台 `craft.inProgress.*` rect 为空、当前台未通过共享签名验证时分别拒绝；其他三台 rect 缺失不阻止当前台试运行。断言旧 `claimReady/idle` 图片路径不会被复制到共享字段。

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib legacy_calibration_
cargo test --manifest-path src-tauri/Cargo.toml --lib station_click_point_
cargo test --manifest-path src-tauri/Cargo.toml --lib calibration_test_
```

Expected: 迁移、空间校验、共享模板测试签名全部 PASS。

---

### Task 3: 重写单制作台 runtime 状态机

**Files:**
- Modify: `src-tauri/src/special_ops/craft_trial.rs`
- Modify: `src-tauri/src/special_ops/craft_runtime.rs:21-535`
- Modify: `src-tauri/src/special_ops/mod.rs:1500-1585`
- Test: `src-tauri/src/special_ops/craft_runtime.rs` 内联 tests

- [ ] **Step 1: 写“制作中零点击”失败测试**

```rust
#[tokio::test]
async fn in_progress_station_finishes_without_input() {
    let driver = FakeDriver::in_progress();
    let result = run_craft_station(
        &driver,
        StationKind::TechnicalCenter,
        Arc::new(AtomicBool::new(false)),
    ).await.unwrap();
    assert_eq!(result, CraftStationOutcome::StillInProgress);
    assert!(driver.actions().is_empty());
}
```

另写测试覆盖：一次高分一次低分时返回普通失败且零点击；采样错误时返回普通失败且零点击。

- [ ] **Step 2: 写点击后双分支与三次重试失败测试**

```rust
#[tokio::test]
async fn idle_station_opens_recipe_list_and_starts_craft() {
    let driver = FakeDriver::not_in_progress()
        .with_open_results([Some(StationOpenResult::RecipeList)])
        .with_button(CraftButton::Produce)
        .with_abort_time(123_456);
    let result = run_craft_station(
        &driver,
        StationKind::Workbench,
        Arc::new(AtomicBool::new(false)),
    ).await.unwrap();
    assert_eq!(result, CraftStationOutcome::Started { started_at_ms: 123_456 });
    assert_eq!(driver.actions()[0], "click-station:craft.station.workbench");
}

#[tokio::test]
async fn reward_branch_returns_to_grid_then_reopens_same_station() {
    let driver = FakeDriver::not_in_progress()
        .with_open_results([
            Some(StationOpenResult::Reward),
            Some(StationOpenResult::RecipeList),
        ])
        .with_button(CraftButton::Produce)
        .with_abort_time(123_456);
    let result = run_craft_station(
        &driver,
        StationKind::Pharmacy,
        Arc::new(AtomicBool::new(false)),
    ).await.unwrap();
    assert!(matches!(result, CraftStationOutcome::Started { .. }));
    assert_eq!(driver.action_count("press-space"), 1);
    assert_eq!(driver.action_count("wait:game.stationGrid"), 1);
    assert_eq!(driver.action_count("click-station:craft.station.pharmacy"), 2);
}

#[tokio::test(start_paused = true)]
async fn three_open_failures_require_uncertain() {
    let driver = FakeDriver::not_in_progress().with_open_results([None, None, None]);
    let error = run_craft_station(
        &driver,
        StationKind::ArmorBench,
        Arc::new(AtomicBool::new(false)),
    ).await.unwrap_err();
    assert!(error.requires_uncertain);
    assert_eq!(driver.action_count("click-station:craft.station.armorBench"), 3);
}
```

再补“前两次无结果、第三次列表成功”测试，断言只延迟两次、点击三次、最终成功。

- [ ] **Step 3: 验证 RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib in_progress_station_finishes_without_input
cargo test --manifest-path src-tauri/Cargo.toml --lib three_open_failures_require_uncertain
```

Expected: FAIL，`CraftStationOutcome`、`StationOpenResult` 与新 driver 方法尚不存在。

- [ ] **Step 4: 定义明确 outcome 与失败语义**

在 `craft_trial.rs` 或现有职责最接近的 runtime 类型区定义，并由 `craft_runtime.rs` 使用：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CraftStationOutcome {
    StillInProgress,
    Started { started_at_ms: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StationOpenResult {
    Reward,
    RecipeList,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StationOpenObservation {
    pub result: Option<StationOpenResult>,
    pub last_samples: Vec<(String, [f32; 2])>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CraftTrialFailure {
    pub step: String,
    pub message: String,
    pub requires_uncertain: bool,
}

impl CraftTrialFailure {
    fn ordinary(step: impl Into<String>, message: impl Into<String>) -> Self {
        Self { step: step.into(), message: message.into(), requires_uncertain: false }
    }

    fn after_input(step: impl Into<String>, message: impl Into<String>) -> Self {
        Self { step: step.into(), message: message.into(), requires_uncertain: true }
    }
}
```

删除 `CraftStationState::{ClaimReady, Idle}`。首次制作台点击前的识别失败设置 `requires_uncertain = false`；首次点击后发生的点击错误、观察错误、取消、奖励页退出失败、台面未就绪、二次打开失败以及三次无结果全部设置 `requires_uncertain = true`。

- [ ] **Step 5: 收紧 CraftTrialDriver 输入边界**

把旧 `detect_station_state` 替换为：

```rust
async fn inspect_in_progress(
    &self,
    target_key: &str,
    cancelled: Arc<AtomicBool>,
) -> Result<SingleConsistency, String>;

async fn click_station(
    &self,
    target_key: &str,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String>;

async fn observe_station_open(
    &self,
    reward_key: &str,
    recipe_list_key: &str,
    timeout: Duration,
    cancelled: Arc<AtomicBool>,
) -> Result<StationOpenObservation, String>;

async fn retry_delay(&self, cancelled: Arc<AtomicBool>) -> Result<(), String>;
```

`click_station` 不调用旧正向 guard；它只从 `run_craft_station` 的 `NotMatched` 分支进入。原 `click` 继续负责置顶配方、补齐、购买、生产等正向模板守卫点击。

- [ ] **Step 6: 实现制作台打开重试状态机**

核心流程写成单入口，禁止把错误转成未命中：

```rust
match driver.inspect_in_progress(&in_progress_key, Arc::clone(&cancelled)).await
    .map_err(|message| CraftTrialFailure::ordinary("craft.inProgress", message))?
{
    SingleConsistency::Matched { .. } => {
        driver.update_stage(
            LoginRunStatus::Succeeded,
            "当前制作尚未完成，本次未执行点击",
        ).map_err(|message| CraftTrialFailure::ordinary("craft.inProgress", message))?;
        return Ok(CraftStationOutcome::StillInProgress);
    }
    SingleConsistency::NotMatched { .. } => {}
}

let mut opened = None;
for attempt in 0..3 {
    driver.click_station(&station_key, Arc::clone(&cancelled)).await
        .map_err(|message| CraftTrialFailure::after_input("craft.station", message))?;
    let observation = driver.observe_station_open(
        "craft.reward",
        &recipe_list_key,
        Duration::from_secs(2),
        Arc::clone(&cancelled),
    ).await.map_err(|message| CraftTrialFailure::after_input("craft.stationOpen", message))?;
    if observation.result.is_some() {
        opened = observation.result;
        break;
    }
    if attempt < 2 {
        driver.retry_delay(Arc::clone(&cancelled)).await
            .map_err(|message| CraftTrialFailure::after_input("craft.stationRetry", message))?;
    }
}
```

`Reward` → `Space` → 等 `game.stationGrid` → 再执行同一套“最多三次点击并等当前台列表”函数，但第二段只接受 `RecipeList`；`RecipeList` → 直接进入置顶配方。首次点击后所有错误通过 `CraftTrialFailure::after_input` 构造，`requires_uncertain = true`；任一三次耗尽同样返回不确定失败。

- [ ] **Step 7: 冻结新 runtime key，并让共享模板进入 runtime**

`freeze_craft_run_config` 只冻结：

```text
craft.station.<station>
craft.inProgress.<station>
craft.recipeListReady.<station>
craft.recipe.<station>
game.stationGrid
craft.reward
craft.fill
craft.purchase
craft.produce
craft.abort
```

构造 `craft.inProgress.<station>` 的 `RuntimeTarget` 时，rect 取当前 target，模板路径和 threshold 取 active `CalibrationEnvironment` 的共享字段。缺图、缺 rect 或未通过测试时拒绝启动试运行。

- [ ] **Step 8: 验证 Task 3**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::craft_runtime::tests`

Expected: 制作中零点击、奖励分支、列表分支、第三次奖励成功、第三次列表成功、三次失败、首次点击后错误/取消进入不确定状态、补齐/购买/生产旧流程全部 PASS。

---

### Task 4: 按 outcome 持久化或保持原状态

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:2663-2927`
- Test: `src-tauri/src/special_ops/mod.rs` 内联 tests

- [ ] **Step 1: 写 worker decision 失败测试**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
enum CraftPersistenceDecision {
    NoChange,
    SaveStarted { started_at_ms: i64 },
    MarkUncertain { step: String, message: String },
    FailWithoutChange { step: String, message: String },
}

#[test]
fn craft_outcome_maps_to_persistence_decision() {
    assert_eq!(
        decide_craft_persistence(Ok(CraftStationOutcome::StillInProgress)),
        CraftPersistenceDecision::NoChange,
    );
    assert_eq!(
        decide_craft_persistence(Ok(CraftStationOutcome::Started { started_at_ms: 42 })),
        CraftPersistenceDecision::SaveStarted { started_at_ms: 42 },
    );
    let uncertain = CraftTrialFailure {
        step: "craft.stationOpen".into(),
        message: "三次点击后均未识别到奖励页或制作列表".into(),
        requires_uncertain: true,
    };
    assert!(matches!(
        decide_craft_persistence(Err(uncertain)),
        CraftPersistenceDecision::MarkUncertain { .. }
    ));
}
```

- [ ] **Step 2: 验证 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib craft_outcome_maps_to_persistence_decision`

Expected: FAIL，decision 类型与映射函数尚不存在。

- [ ] **Step 3: 实现纯 decision 映射**

```rust
fn decide_craft_persistence(
    result: Result<craft_trial::CraftStationOutcome, craft_trial::CraftTrialFailure>,
) -> CraftPersistenceDecision {
    match result {
        Ok(craft_trial::CraftStationOutcome::StillInProgress) => CraftPersistenceDecision::NoChange,
        Ok(craft_trial::CraftStationOutcome::Started { started_at_ms }) => {
            CraftPersistenceDecision::SaveStarted { started_at_ms }
        }
        Err(error) if error.requires_uncertain => CraftPersistenceDecision::MarkUncertain {
            step: error.step,
            message: error.message,
        },
        Err(error) => CraftPersistenceDecision::FailWithoutChange {
            step: error.step,
            message: error.message,
        },
    }
}
```

- [ ] **Step 4: 让 worker 严格执行 decision**

在 `stop_reason == None` 时：

- `NoChange`：不调用 `SettingsCoordinator::with_runtime_change`、不保存文件、不增加 revision，运行结果为 `Succeeded`，文案固定为“当前制作尚未完成，本次未执行点击”。
- `SaveStarted`：沿用 `persist_craft_success_with`，保存实际确认 `craft.abort` 的时间与下一次完成时间。
- `MarkUncertain`：调用 `persist_craft_uncertain_with` 并 emit 新 bootstrap，最终运行状态为 `Failed`，错误保留失败步骤。
- `FailWithoutChange`：不写配置，运行状态为 `Failed`。
- 有 stop reason：继续走现有 `persist_craft_stop_with`；输入已经发生时普通取消仍标记 `Uncertain`，紧急停止仍先完成 persistence claim 再 cleanup。

- [ ] **Step 5: 写配置不变回归测试**

复制一份 `SpecialOpsSettings`，记录 `SettingsCoordinator` revision，执行 `NoChange` 与 `FailWithoutChange` 路径，断言：

```rust
assert_eq!(after.accounts, before.accounts);
assert_eq!(revision_after, revision_before);
```

执行 `MarkUncertain`，断言只修改当前账号与当前制作台，其他三台和其他账号不变。

- [ ] **Step 6: 验证 Task 4**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib craft_outcome_maps_to_persistence_decision
cargo test --manifest-path src-tauri/Cargo.toml --lib persist_craft_
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::login_runtime::tests
```

Expected: 所有 PASS；`StillInProgress` 与普通识别错误不产生持久化写入。

---

### Task 5: 前端改为一张共享图片、四个独立区域

**Files:**
- Modify: `src/components/app/special-ops-page.tsx:431-710`
- Modify: `src/components/app/special-ops-types.ts:13-25`
- Test: `src/components/app/special-ops-page.test.tsx`

- [ ] **Step 1: 写共享 uploader 与四区域失败测试**

```tsx
it("制作中只上传一张共享参考图并保留四个独立框选区域", async () => {
  renderSpecialOpsPage({
    calibrationEnvironment: {
      craftInProgressReferenceImagePath: "C:\\templates\\crafting.png",
      craftInProgressMatchThreshold: 0.75,
    },
  });

  expect(screen.getAllByText("crafting.png")).toHaveLength(5);
  expect(screen.getByRole("button", { name: "替换制作中参考图" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "清除制作中参考图" })).toBeInTheDocument();
  expect(screen.getAllByRole("button", { name: /框选.*制作中状态区域/ })).toHaveLength(4);
  expect(screen.queryByText(/craft\.claimReady\./)).not.toBeInTheDocument();
  expect(screen.queryByText(/craft\.idle\./)).not.toBeInTheDocument();
});
```

`5` = uploader 当前文件名一次 + 四个区域行各显示一次共享文件名。另写 active run 时上传、清除、threshold、框选、测试全部 disabled 的测试。

- [ ] **Step 2: 验证 RED**

Run: `bunx vitest run src/components/app/special-ops-page.test.tsx -t "制作中只上传一张共享参考图"`

Expected: FAIL，共享 uploader 与新字段尚未渲染。

- [ ] **Step 3: 增加共享参考图编辑区**

按当前页面的 `open()` 与 `save()` 模式新增 handler：

```tsx
const updateCalibrationEnvironment = (
  environment: CalibrationEnvironment,
  patch: Partial<CalibrationEnvironment>,
) => save({
  ...settingsDraftRef.current,
  calibrationEnvironments: settingsDraftRef.current.calibrationEnvironments.map((item) =>
    item.id === environment.id ? {...item, ...patch} : item,
  ),
});

const pickCraftInProgressReferenceImage = async (environment: CalibrationEnvironment) => {
  if (!isNativeShell || hasActiveRun) return;
  const picked = await open({
    multiple: false,
    directory: false,
    filters: [{name: "图片文件", extensions: ["png", "jpg", "jpeg", "webp", "bmp"]}],
  });
  if (typeof picked === "string") {
    updateCalibrationEnvironment(environment, {
      craftInProgressReferenceImagePath: picked,
    });
  }
};
```

在校准表上方直接复用现有 `Button`、`DraftInput` 与 `RiFolderOpenLine`/`RiDeleteBinLine`，渲染“上传/替换制作中参考图”“清除制作中参考图”和 threshold 输入。不新增抽象组件。共享 threshold 输入范围 `0..=1`；保存仍走原 latest-wins 队列与 active run 前后端 gate。

- [ ] **Step 4: 重排校准列表**

删除 `craft.claimReady.*` 与 `craft.idle.*` 行。渲染四个 `craft.inProgress.*` 行，每行：

- 显示当前台名称。
- 显示共享文件名，只读。
- 保留“框选”与“测试”按钮。
- 不显示独立上传/清除按钮。
- 测试 command 仍传 target key，后端按 active environment 解析共享图片。

- [ ] **Step 5: 更新前端 source contract 测试**

断言 `special-ops-types.ts` 只有 camelCase 字段：

```ts
craftInProgressReferenceImagePath: string | null;
craftInProgressMatchThreshold: number;
```

断言页面不存在对 `craft.claimReady.`、`craft.idle.` 的运行期依赖。

- [ ] **Step 6: 验证 Task 5**

Run:

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx src/components/app/special-ops-utils.test.ts
bun run build
```

Expected: Vitest 与 TypeScript/Vite build 全部 PASS。

---

### Task 6: 同步设计、Wiki 与仓库约束文档

**Files:**
- Modify: `docs/superpowers/specs/2026-07-22-特勤处多账号自动化-设计草案.md`
- Modify: `docs/superpowers/specs/2026-07-28-单账号制作试运行-design.md`
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `README.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: 更新主设计草案**

删除“感叹号/空闲中用于前置分类”的陈述，写入：共享制作中参考图、四台独立区域、两个有效低分才允许点击、奖励页/制作列表后置分流、2 秒观察与最多 3 次点击、三次失败标记 `Uncertain`。

- [ ] **Step 2: 更新单账号制作试运行设计**

把 `craft.claimReady.*` / `craft.idle.*` 输入清单替换为 `craft.inProgress.*`，补齐 `StillInProgress` 零写入语义和 `CraftStationOutcome` 持久化矩阵。

- [ ] **Step 3: 更新 Wiki**

在 `droid-wiki/features/special-ops.md` 记录：

```text
共享资产：craftInProgressReferenceImagePath + craftInProgressMatchThreshold
独立区域：craft.inProgress.<station>
负向守卫：双低分才点击；不一致与系统错误均停止
点击结果：craft.reward 或 craft.recipeListReady.<station>
```

同步校准、试运行、停止与 `Uncertain` 行为，禁止继续描述旧前置分类。

- [ ] **Step 4: 更新 README 与 AGENTS**

`README.md` 的特勤处进度项改为新状态机。`AGENTS.md` 项目概览删旧 `claimReady/idle` 依赖，写入共享模板与负向守卫。不得改无关模块说明。

- [ ] **Step 5: 做文档一致性扫描**

Run:

```powershell
rg -n "craft\.claimReady|craft\.idle|感叹号|空闲中" README.md AGENTS.md droid-wiki/features/special-ops.md docs/superpowers/specs
```

Expected: 仅保留明确标注为“旧行为/已废弃”的迁移说明；运行逻辑与校准清单不再引用旧 key。

---

### Task 7: 全量门禁与实机验收交接

**Files:**
- Verify only; no new production file

- [ ] **Step 1: 跑 Rust 定向测试**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::template_observer::tests
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::craft_runtime::tests
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::tests
```

Expected: 全部 PASS。

- [ ] **Step 2: 跑前端定向测试和 build**

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx src/components/app/special-ops-utils.test.ts
bun run build
```

Expected: 全部 PASS。

- [ ] **Step 3: 跑统一质量门禁**

Run: `bun run check`

Expected: TypeScript、Vitest、coverage、Rust fmt、Clippy `-D warnings`、Rust tests 全部 PASS。若运行中的 `delta-auto-tools.exe` 锁定 Rust binary 并出现 `os error 5`，不得结束用户现有进程；原样记录阻塞，并补跑 `cargo test --manifest-path src-tauri/Cargo.toml --lib` 证明 library tests 状态。

- [ ] **Step 4: 更新 CodeGraph 并检查 diff**

```powershell
codegraph sync
git diff --check
git status --short --branch
```

Expected: CodeGraph 更新成功；`git diff --check` 无空白错误；状态只包含本功能与用户既有修改，不清理、不 reset、不覆盖历史改动。

- [ ] **Step 5: 交给用户做五组实机验收**

按顺序验收并逐项记录 UI 文案、点击次数、配置变化：

1. 制作中：双采样命中 → 零点击、提示尚未完成、计时不变。
2. 空闲：双采样双低分 → 第一次点击进入制作列表 → 生产 → `craft.abort` → 保存新完成时间。
3. 可收取：双低分 → 奖励页 → `Space` → 台面 → 再点同一台 → 制作列表 → 重做。
4. 前两次点击无结果、第三次成功：只点击三次，成功后不再重试。
5. 三次均无结果：当前账号与当前制作台标记 `Uncertain`，停止剩余流程，不覆盖完成时间。

另验普通取消与紧急停止：发生输入后账号/当前台进入 `Uncertain`；停止完成后新试运行与设置编辑才恢复。

- [ ] **Step 6: 最终复核，不提交混合历史改动**

对照设计文档逐条核验。当前目标生产文件包含历史未提交修改时，不执行整文件 `git add`，不创建混入用户历史改动的 commit。只报告已修改文件、验证结果、实机待验项与任何已知阻塞。
