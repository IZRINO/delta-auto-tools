# 特勤处识图停放与停止可靠性 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让特勤处识图避开鼠标 hover，并保证全局关闭、紧急停止、普通取消和资源清理不会留下失控 run。

**Architecture:** 在 `special_ops` 校准中增加唯一鼠标停放点，把坐标冻结进三个 run config；各 production driver 在业务输入后、下一次截图前移动鼠标。热键层增加显式安全注册标志，只有运行期紧急停止 scope 可绕过全局 gate；取消状态与清理仍由单实例 worker 收尾，但输入释放改为有界并记录阶段日志。

**Tech Stack:** Rust、Tokio、Tauri 2、React 19、TypeScript、Vitest、Rust 单元测试、Bun

**Execution:** Inline Execution。禁止创建子代理或 worktree；保留当前普通目录与 `codex/special-ops-login` 分支中的既有未提交修改。

---

### Task 1: 特勤处鼠标停放点配置与预检

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:322-538,1142-1495`
- Modify: `src-tauri/src/special_ops/login_flow.rs:25-31`
- Modify: `src-tauri/src/special_ops/game_navigation.rs:53-57`
- Modify: `src-tauri/src/special_ops/craft_runtime.rs:53-64`
- Test: `src-tauri/src/special_ops/mod.rs:5610-5864`
- Test: `src/components/app/special-ops-page.test.tsx`

- [ ] **Step 1: 写停放点迁移与预检失败测试**

在 `special_ops::tests` 增加：

```rust
#[test]
fn default_calibration_contains_special_ops_mouse_parking_point() {
    let target = default_calibration_targets()
        .into_iter()
        .find(|target| target.key == "runtime.mouseParking")
        .expect("应包含特勤处鼠标停放点");
    assert_eq!(target.label, "特勤处鼠标停放点");
    assert_eq!(target.kind, CalibrationTargetKind::ClickPoint);
    assert!(target.reference_image_path.is_none());
}

#[test]
fn all_trial_configs_require_mouse_parking_point() {
    let mut fixture = LoginFixture::complete();
    calibration_target_mut(&mut fixture.settings, "runtime.mouseParking").rect = None;
    assert!(validate_login_trial_ready(&fixture.settings, "selected")
        .unwrap_err().contains("特勤处鼠标停放点尚未校准"));
}
```

在前端测试断言校准列表出现“特勤处鼠标停放点”。

- [ ] **Step 2: 运行测试并确认 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml mouse_parking -- --nocapture
bunx vitest run src/components/app/special-ops-page.test.tsx
```

Expected: Rust 测试因目标不存在失败；前端测试因文案不存在失败。

- [ ] **Step 3: 加入校准目标并冻结坐标**

在 `default_calibration_targets()` 首部加入：

```rust
("runtime.mouseParking", "特勤处鼠标停放点", ClickPoint),
```

增加统一读取函数：

```rust
fn mouse_parking_region(settings: &SpecialOpsSettings) -> Result<crate::morse::types::RegionRect, String> {
    let target = settings.calibration_environments.first()
        .and_then(|environment| environment.targets.iter()
            .find(|target| target.key == "runtime.mouseParking"))
        .ok_or_else(|| "特勤处鼠标停放点不存在".to_string())?;
    let rect = target.rect.as_ref()
        .ok_or_else(|| "特勤处鼠标停放点尚未校准".to_string())?;
    Ok(crate::morse::types::RegionRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    })
}
```

给 `LoginRunConfig`、`NavigationRunConfig`、`CraftRunConfig` 增加：

```rust
pub mouse_parking_region: crate::morse::types::RegionRect,
```

三个 freeze 函数都调用 `mouse_parking_region(settings)?`。点击点不要求参考图或模板验证。

- [ ] **Step 4: 运行定向测试并确认 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml mouse_parking -- --nocapture
bunx vitest run src/components/app/special-ops-page.test.tsx
```

Expected: 两条命令 exit 0。

### Task 2: 每次特勤处输入后停放鼠标

**Files:**
- Modify: `src-tauri/src/input_simulation.rs:220-405`
- Modify: `src-tauri/src/special_ops/login_runtime.rs:625-1045`
- Modify: `src-tauri/src/special_ops/game_navigation.rs:233-410`
- Modify: `src-tauri/src/special_ops/craft_runtime.rs:53-318`
- Test: `src-tauri/src/input_simulation.rs:780-977`
- Test: `src-tauri/src/special_ops/login_runtime.rs`
- Test: `src-tauri/src/special_ops/game_navigation.rs:420-520`
- Test: `src-tauri/src/special_ops/craft_runtime.rs:485-650`

- [ ] **Step 1: 写输入后停放顺序测试**

给 `input_simulation` 增加：

```rust
#[test]
fn move_region_center_only_moves_without_clicking() {
    let emitter = RecordingEmitter::default();
    let cancelled = AtomicBool::new(false);
    move_region_center_with_emitter(
        &emitter,
        &RegionRect { x: 100, y: 200, width: 20, height: 10 },
        &cancelled,
        input_release_generation(),
    ).unwrap();
    assert_eq!(emitter.events(), ["move:110:205"]);
}
```

给三个 production driver 的可注入输入测试分别断言：

```text
click:<业务目标>
move:runtime.mouseParking
sample:<后置模板>
```

- [ ] **Step 2: 运行测试并确认 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml move_region_center_only_moves_without_clicking -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml parks_mouse -- --nocapture
```

Expected: 第一条因函数不存在失败；第二条因 driver 未停放鼠标失败。

- [ ] **Step 3: 实现只移动鼠标的公共输入函数**

```rust
fn move_region_center_with_emitter<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    let (x, y) = region_center(region);
    run_cancellable_input_action(cancelled, generation, |_| emitter.move_mouse(x, y))
}

pub async fn move_region_center_cancellable(
    region: crate::morse::types::RegionRect,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        move_region_center_with_emitter(&emitter, &region, &cancelled, generation)
    }).await
}
```

- [ ] **Step 4: 在三个 driver 的每个输入出口后停放鼠标**

统一使用以下顺序，不增加第二次倒计时：

```rust
input_simulation::click_region_center_cancellable(region, Arc::clone(&cancelled)).await?;
input_simulation::move_region_center_cancellable(
    self.mouse_parking_region.clone(),
    cancelled,
).await
```

同样覆盖 `press_named_key_cancellable`、`scroll_region_down_cancellable`、`click_screen_point_cancellable`、`double_click_region_and_copy_cancellable`。停放失败直接返回错误，禁止继续截图。

- [ ] **Step 5: 运行定向测试并确认 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml input_simulation::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml special_ops::login_runtime::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml special_ops::game_navigation::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml special_ops::craft_runtime::tests -- --nocapture
```

Expected: 四组测试全部 PASS。

### Task 3: 全局开关预检与紧急热键安全例外

**Files:**
- Modify: `src-tauri/src/hotkey_types.rs:445-452`
- Modify: `src-tauri/src/hotkeys.rs:366-409,625-739`
- Modify: `src-tauri/src/special_ops/mod.rs:1747-1766,2597-2765`
- Test: `src-tauri/src/hotkeys.rs:1052-1250`
- Test: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1: 写全局 gate 与启动预检测试**

```rust
#[test]
fn global_gate_only_keeps_explicit_safety_actions() {
    let normal = registration_for_test("normal", false);
    let emergency = registration_for_test("special-ops-emergency", true);
    assert_eq!(dispatchable_registrations(&[normal, emergency], false), ["special-ops-emergency"]);
}

#[test]
fn special_ops_trials_reject_disabled_global_state() {
    assert_eq!(ensure_global_automation_enabled(false).unwrap_err(), "全局总开关已关闭");
}
```

- [ ] **Step 2: 运行测试并确认 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml global_gate_only_keeps_explicit_safety_actions -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml special_ops_trials_reject_disabled_global_state -- --nocapture
```

Expected: 因安全标志和预检函数不存在失败。

- [ ] **Step 3: 增加显式安全注册，不硬编码热键字符串**

给 `HotkeyRegistration` 增加：

```rust
pub allow_when_global_disabled: bool,
```

`replace_scope()` 默认传 `false`；新增：

```rust
pub fn replace_safety_scope(
    &self,
    scope: &str,
    bindings: Vec<(String, HotkeyAction)>,
    display_name: String,
    conflict_policy: ConflictPolicy,
) -> Result<(), String> {
    self.replace_scope_with_gate(scope, bindings, display_name, conflict_policy, true)
}
```

listener 仍更新 matcher，但普通 action 过滤为：

```rust
registration.enabled
    && (global_enabled || registration.allow_when_global_disabled)
    && matches_binding(&registration.binding, key_state)
```

hold scope 不增加安全例外。

- [ ] **Step 4: 特勤处启动读取 `GlobalState`，热键 callback 异步收尾**

三个 start command 在 `with_revision` 内、freeze 前执行：

```rust
let global_enabled = app.try_state::<crate::global_state::GlobalState>()
    .map(|state| state.enabled())
    .unwrap_or(true);
ensure_global_automation_enabled(global_enabled)?;
```

`register_emergency_hotkey` 改用 `replace_safety_scope`。callback 只派发：

```rust
let action: HotkeyAction = Arc::new(|app| {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = emergency_stop_core(&app) {
            crate::log_error!("special_ops::runtime", "紧急停止失败", "error" => error);
        }
    });
});
```

- [ ] **Step 5: 运行定向测试并确认 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml hotkeys::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml special_ops_trials_reject_disabled_global_state -- --nocapture
```

Expected: 全部 PASS。

### Task 4: 普通取消文案与制作状态不确定

**Files:**
- Modify: `src-tauri/src/special_ops/login_runtime.rs:46-70,313-400`
- Modify: `src-tauri/src/special_ops/mod.rs:2416-2498`
- Test: `src-tauri/src/special_ops/login_runtime.rs`
- Test: `src-tauri/src/special_ops/mod.rs:3492-3900`

- [ ] **Step 1: 写三类取消文案和制作风险测试**

```rust
#[test]
fn normal_cancel_message_matches_run_kind() {
    assert_eq!(LoginRunKind::Login.normal_cancel_message(), "正在取消登录试运行");
    assert_eq!(LoginRunKind::Navigation.normal_cancel_message(), "正在取消游戏内导航试运行");
    assert_eq!(LoginRunKind::Craft.normal_cancel_message(), "正在取消制作试运行");
}

#[test]
fn craft_cancel_after_input_marks_account_and_station_uncertain() {
    let mut settings = LoginFixture::complete().settings;
    mark_craft_cancel_uncertain(&mut settings, "selected", StationKind::Workbench, 42).unwrap();
    assert!(settings.paused);
    assert_eq!(settings.accounts[0].status, AccountStatus::Uncertain);
    assert_eq!(settings.accounts[0].stations[0].status, StationStatus::Uncertain);
}
```

- [ ] **Step 2: 运行测试并确认 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml normal_cancel_message_matches_run_kind -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml craft_cancel_after_input_marks_account_and_station_uncertain -- --nocapture
```

Expected: 文案方法与状态 helper 不存在。

- [ ] **Step 3: 实现 run-kind 文案与输入状态读取**

```rust
impl LoginRunKind {
    fn normal_cancel_message(self) -> &'static str {
        match self {
            Self::Login => "正在取消登录试运行",
            Self::Navigation => "正在取消游戏内导航试运行",
            Self::Craft => "正在取消制作试运行",
        }
    }
}
```

`request_stop_locked` 对 `StopReason::Normal` 使用 `active.snapshot.run_kind.normal_cancel_message()`。给 runtime 增加只读方法：

```rust
pub(crate) fn entered_input(&self, run_id: u64) -> Result<bool, String> {
    self.inner.lock()
        .map_err(|_| "登录试运行状态已损坏".to_string())
        .map(|inner| inner.active.as_ref()
            .filter(|active| active.snapshot.run_id == run_id)
            .is_some_and(|active| active.entered_input))
}
```

- [ ] **Step 4: 普通制作取消在已输入后持久化不确定状态**

```rust
fn mark_craft_cancel_uncertain(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    station: StationKind,
    at_ms: i64,
) -> Result<(), String> {
    settings.paused = true;
    let account = settings.accounts.iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "制作试运行账号已不存在".to_string())?;
    account.status = AccountStatus::Uncertain;
    account.last_failure = Some(AccountFailure {
        step: "craftCancel".to_string(),
        message: "制作试运行取消时已执行键鼠输入，请人工确认制作状态并修正完成时间".to_string(),
        at_ms,
    });
    account.stations.iter_mut()
        .find(|candidate| candidate.kind == station)
        .ok_or_else(|| "制作台不存在".to_string())?
        .status = StationStatus::Uncertain;
    Ok(())
}
```

`run_craft_worker` 仅在 `StopReason::Normal` 且 `entered_input(run_id)?` 时，通过 `SettingsCoordinator::with_runtime_change` 保存该状态。首次输入前取消不写业务配置；紧急停止继续走既有权威持久化，禁止重复写。

- [ ] **Step 5: 运行定向测试并确认 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml normal_cancel_message_matches_run_kind -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml craft_cancel_after_input_marks_account_and_station_uncertain -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml emergency -- --nocapture
```

Expected: 三条命令 exit 0。

### Task 5: 有界输入释放与清理边界日志

**Files:**
- Modify: `src-tauri/src/input_simulation.rs:400-441`
- Modify: `src-tauri/src/special_ops/mod.rs:1784-1821,2223-2498`
- Test: `src-tauri/src/input_simulation.rs:900-977`
- Test: `src-tauri/src/special_ops/mod.rs:5378-5400`

- [ ] **Step 1: 写 emitter 永久失败的有界退出测试**

```rust
#[test]
fn release_tracked_inputs_stops_after_bounded_attempts() {
    let mut state = lock_input_action_state();
    state.tracked_keys = vec![Key::Control];
    drop(state);
    let attempts = AtomicUsize::new(0);
    let error = release_tracked_injected_inputs_with_factory(|| {
        attempts.fetch_add(1, Ordering::SeqCst);
        Err::<RecordingEmitter, _>("emitter failed".to_string())
    }).unwrap_err();
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert!(error.contains("释放已注入按键失败"));
}
```

资源清理测试增加输入释放失败后仍依次调用 `hotkey`、`window`、`restore` 的断言。

- [ ] **Step 2: 运行测试并确认 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml release_tracked_inputs_stops_after_bounded_attempts -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml release_login_resources -- --nocapture
```

Expected: 第一条卡在现有无限循环或因返回类型不匹配失败；第二条缺少错误后继续清理覆盖。

- [ ] **Step 3: 把输入释放改为三次有界重试**

```rust
const INPUT_RELEASE_MAX_ATTEMPTS: usize = 3;

pub fn release_tracked_injected_inputs() -> Result<(), String> {
    release_tracked_injected_inputs_with_factory(EnigoInputEmitter::new)
}

fn release_tracked_injected_inputs_with_factory<E, F>(mut emitter_factory: F) -> Result<(), String>
where
    E: InputEmitter,
    F: FnMut() -> Result<E, String>,
{
    invalidate_cancellable_inputs();
    for attempt in 1..=INPUT_RELEASE_MAX_ATTEMPTS {
        let mut state = lock_input_action_state();
        if state.tracked_keys.is_empty() { return Ok(()); }
        if let Ok(emitter) = emitter_factory() {
            release_tracked_injected_keys_with(&mut state, &emitter);
        }
        if state.tracked_keys.is_empty() { return Ok(()); }
        drop(state);
        if attempt < INPUT_RELEASE_MAX_ATTEMPTS {
            thread::sleep(INPUT_CANCELLATION_POLL_INTERVAL);
        }
    }
    Err("释放已注入按键失败，已达到 3 次重试上限".to_string())
}
```

- [ ] **Step 4: 清理阶段独立执行并记录边界日志**

`release_login_resources_with` 接收返回 `Result` 的输入释放 closure。每个阶段使用相同模式：

```rust
crate::log_info!("special_ops::cleanup", "资源清理开始", "stage" => "inputs");
match release_inputs() {
    Ok(()) => crate::log_info!("special_ops::cleanup", "资源清理完成", "stage" => "inputs"),
    Err(error) => {
        crate::log_error!("special_ops::cleanup", "资源清理失败", "stage" => "inputs", "error" => error.clone());
        errors.push(error);
    }
}
```

同样覆盖 `hotkey`、`operationWindow`、`restoreWindows`。worker 在流程返回、持久化开始/结束、runtime finish 前后记录 `run_id`、`run_kind`；禁止记录 QQ 密码。

- [ ] **Step 5: 运行定向测试并确认 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml input_simulation::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml release_login_resources -- --nocapture
```

Expected: 两组测试 PASS，失败 emitter 测试在有限时间内结束。

### Task 6: 文档同步与全量门禁

**Files:**
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Verify: all modified source and tests

- [ ] **Step 1: 同步行为文档**

记录以下事实：

```text
- special_ops 独占 runtime.mouseParking，不影响其他工具；
- 全局关闭时禁止启动试运行；
- 运行期紧急热键可绕过全局 gate；
- 制作输入后普通取消标记账号与制作台状态不确定；
- 输入释放最多重试三次，之后继续其余清理。
```

`README.md` 与 `AGENTS.md` 的 special_ops 摘要同步新增校准目标和停止语义，不新增 Tauri command 或持久化顶层文件。

- [ ] **Step 2: 运行格式化与定向测试**

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml
bunx vitest run src/components/app/special-ops-page.test.tsx src/components/app/special-ops-operation-overlay.test.ts
cargo test --manifest-path src-tauri/Cargo.toml special_ops -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml hotkeys::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml input_simulation::tests -- --nocapture
```

Expected: 全部 exit 0。

- [ ] **Step 3: 运行全量质量门禁**

```powershell
bun run check
```

Expected: TypeScript、Vitest、coverage、Rust fmt、Clippy `-D warnings`、Rust tests 全部 exit 0。

- [ ] **Step 4: 刷新 CodeGraph 并检查 diff**

```powershell
codegraph sync
git diff --check
git status --short --branch
```

Expected: `codegraph sync` 与 `git diff --check` exit 0；status 只包含本任务和进入任务前已存在的用户修改。

- [ ] **Step 5: 提交本轮实现**

只暂存本计划实际修改且已核对归属的文件：

```powershell
git add -- src-tauri/src/input_simulation.rs src-tauri/src/hotkey_types.rs src-tauri/src/hotkeys.rs src-tauri/src/special_ops/mod.rs src-tauri/src/special_ops/login_flow.rs src-tauri/src/special_ops/login_runtime.rs src-tauri/src/special_ops/game_navigation.rs src-tauri/src/special_ops/craft_runtime.rs src/components/app/special-ops-page.test.tsx droid-wiki/features/special-ops.md README.md AGENTS.md docs/superpowers/plans/2026-07-28-特勤处识图停放与停止可靠性-implementation.md
git commit -m "fix(special-ops): 修正识图停放与停止可靠性"
```

Expected: commit 成功，不纳入无关修改。
