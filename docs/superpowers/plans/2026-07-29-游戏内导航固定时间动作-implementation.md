# 游戏内导航固定时间动作 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用三段全局毫秒等待依次触发 `Space`、`Tab` 和特勤处点击，删除中间模板识别及导航倒计时，并在校准列表中配置时间。

**Architecture:** `SpecialOpsSettings` 保存三个时间，`freeze_navigation_run_config` 冻结时间和四个校准目标。`game_navigation.rs` 保留首尾模板识别，中间执行可取消等待和无倒计时输入；前端在现有校准表按流程插入专用时间行，不建立通用动作编排器。

**Tech Stack:** Rust、Tauri 2、React 19、TypeScript、Vitest、Bun、Cargo test。

---

## 文件结构

- `src-tauri/src/special_ops/mod.rs`：设置、默认值、校验、目标迁移、preflight、冻结配置。
- `src-tauri/src/special_ops/game_navigation.rs`：固定等待状态机、取消语义、无倒计时输入。
- `src/components/app/special-ops-types.ts`：前端设置与 run step 类型。
- `src/components/app/special-ops-utils.ts`：毫秒输入解析。
- `src/components/app/special-ops-page.tsx`：校准表时间行。
- 对应 `*.test.*`：Rust/前端回归测试。
- `droid-wiki/features/special-ops.md`、`AGENTS.md`：行为文档。

### Task 1: 增加全局等待时间

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:186-230,892-1034`
- Modify: `src/components/app/special-ops-types.ts:17-22`
- Modify: `src/components/app/special-ops-page.tsx:57-80`
- Test: `src-tauri/src/special_ops/mod.rs` 内联 tests

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn navigation_delays_default_to_three_seconds() {
    let settings = SpecialOpsSettings::default();
    assert_eq!(settings.navigation_space_delay_ms, 3_000);
    assert_eq!(settings.navigation_tab_delay_ms, 3_000);
    assert_eq!(settings.navigation_special_ops_delay_ms, 3_000);
}

#[test]
fn navigation_delays_reject_values_above_sixty_seconds() {
    let mut settings = SpecialOpsSettings::default();
    settings.navigation_space_delay_ms = 0;
    settings.navigation_tab_delay_ms = 60_000;
    assert!(normalize_settings(settings.clone()).is_ok());
    settings.navigation_special_ops_delay_ms = 60_001;
    assert_eq!(normalize_settings(settings).unwrap_err(),
        "游戏内导航等待时间必须是 0–60000ms 的整数");
}
```

- [ ] **Step 2: 验证 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib navigation_delays_`

Expected: FAIL，三个字段尚不存在。

- [ ] **Step 3: 写最小实现**

```rust
fn default_navigation_delay_ms() -> u32 { 3_000 }

#[serde(default = "default_navigation_delay_ms")]
pub navigation_space_delay_ms: u32,
#[serde(default = "default_navigation_delay_ms")]
pub navigation_tab_delay_ms: u32,
#[serde(default = "default_navigation_delay_ms")]
pub navigation_special_ops_delay_ms: u32,
```

`Default` 写入三个 `3_000`；`normalize_settings` 拒绝任一字段大于 `60_000`。同步 TypeScript camelCase 字段和 `emptyBootstrap.settings` 默认值。

- [ ] **Step 4: 验证 GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib navigation_delays_`

Expected: PASS。

- [ ] **Step 5: 提交**

```powershell
git add src-tauri/src/special_ops/mod.rs src/components/app/special-ops-types.ts src/components/app/special-ops-page.tsx
git commit -m "feat(special_ops): 增加导航固定等待配置"
```

### Task 2: 迁移导航校准目标

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:322-410,892-977,1385-1478`
- Test: `src-tauri/src/special_ops/mod.rs` 内联 tests

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn navigation_targets_remove_intermediate_templates() {
    let targets = default_calibration_targets();
    assert!(!targets.iter().any(|target| target.key == "game.activityPopup"));
    assert!(!targets.iter().any(|target| target.key == "game.startGame"));
    let target = targets.iter().find(|target| target.key == "game.specialOps").unwrap();
    assert_eq!(target.kind, CalibrationTargetKind::ClickPoint);
    assert_eq!(target.recognition_method, None);
}
```

```rust
#[test]
fn normalize_clears_legacy_special_ops_template() {
    let mut settings = SpecialOpsSettings::default();
    let target = settings.calibration_environments[0].targets.iter_mut()
        .find(|target| target.key == "game.specialOps").unwrap();
    target.kind = CalibrationTargetKind::RecognitionRegion;
    target.rect = Some(CalibrationRect { x: 1, y: 2, width: 20, height: 20 });
    target.reference_image_path = Some("legacy.png".to_string());
    target.verified_signature = Some("legacy".to_string());
    let normalized = normalize_settings(settings).unwrap();
    let migrated = normalized.calibration_environments[0].targets.iter()
        .find(|target| target.key == "game.specialOps").unwrap();
    assert_eq!(migrated.kind, CalibrationTargetKind::ClickPoint);
    assert_eq!(migrated.rect, None);
    assert_eq!(migrated.reference_image_path, None);
    assert_eq!(migrated.verified_signature, None);
}
```

- [ ] **Step 2: 验证 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib navigation_targets_remove_intermediate_templates`

Expected: FAIL，当前仍保留两个模板，特勤处仍是识别区域。

- [ ] **Step 3: 写最小实现**

```rust
("game.specialOps", "特勤处入口点击点", ClickPoint),
```

从 `default_calibration_targets` 删除 `game.activityPopup`、`game.startGame`。将 `freeze_navigation_run_config` 的 key 数组精确替换为：

```rust
[
    "game.modeReady",
    "game.beaconMode",
    "game.specialOps",
    "game.stationGrid",
]
```

沿用 `kind_changed` 分支清除旧模板数据，并把三个时间冻结进 `NavigationRunConfig`。

- [ ] **Step 4: 验证 GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib navigation_targets_`

Expected: 目标、迁移和 preflight 测试 PASS。

- [ ] **Step 5: 提交**

```powershell
git add src-tauri/src/special_ops/mod.rs
git commit -m "refactor(special_ops): 精简导航校准目标"
```

### Task 3: 重写固定等待状态机

**Files:**
- Modify: `src-tauri/src/special_ops/game_navigation.rs:24-445`
- Modify: `src-tauri/src/special_ops/mod.rs` worker 调用处
- Modify: `src/components/app/special-ops-types.ts:18-21`
- Test: `src-tauri/src/special_ops/game_navigation.rs` 内联 tests

- [ ] **Step 1: 写状态机失败测试**

扩展 `FakeDriver`，让 `wait_delay` 记录 `delay:{ms}`。测试只提供 `game.modeReady`、`game.stationGrid` 两个观察，并断言：

```rust
assert_eq!(driver.actions(), [
    "click:game.beaconMode", "delay:100", "key:Space",
    "delay:200", "key:Tab", "delay:300", "click:game.specialOps",
]);
```

- [ ] **Step 2: 验证 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib navigation_uses_three_fixed_delays`

Expected: FAIL，现有状态机仍请求中间模板。

- [ ] **Step 3: 写最小实现**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NavigationDelays {
    pub space_ms: u32,
    pub tab_ms: u32,
    pub special_ops_ms: u32,
}
```

状态机固定为：等待 `modeReady` → 点击 `beaconMode` → 等待/Space → 等待/Tab → 等待/点击 `specialOps` → 等待 `stationGrid`。删除 `WaitStartGame`、`DismissActivityPopup`、`WaitSpecialOps` 及 TypeScript 对应字面量。

生产 `wait_delay` 调用：

```rust
wait_cancellable(Duration::from_millis(u64::from(delay_ms)), cancelled.as_ref()).await
```

`click`、`press` 删除 `countdown()` 与 `verify_guard()`；保留取消检查、窗口聚焦、输入态消息、真实输入和鼠标停放。

- [ ] **Step 4: 写边界失败测试再实现**

```rust
#[tokio::test]
async fn zero_delays_keep_full_action_order() {
    let driver = FakeDriver::with_waits(["game.modeReady", "game.stationGrid"]);
    let delays = NavigationDelays { space_ms: 0, tab_ms: 0, special_ops_ms: 0 };
    assert_eq!(run_game_navigation(&driver, delays, Arc::new(AtomicBool::new(false)), |_| {}).await,
        GameNavigationResult::Ready);
    assert_eq!(driver.actions(), [
        "click:game.beaconMode", "delay:0", "key:Space",
        "delay:0", "key:Tab", "delay:0", "click:game.specialOps",
    ]);
}

#[tokio::test]
async fn cancellation_during_delay_stops_before_space() {
    let driver = FakeDriver::with_wait_failure("游戏内导航试运行已取消");
    let result = run_game_navigation(
        &driver,
        NavigationDelays { space_ms: 100, tab_ms: 200, special_ops_ms: 300 },
        Arc::new(AtomicBool::new(false)),
        |_| {},
    ).await;
    assert!(matches!(result, GameNavigationResult::Paused { .. }));
    assert_eq!(driver.actions(), ["click:game.beaconMode", "delay:100"]);
}
```

先运行见 RED，再让生产 `wait_delay` 使用 `wait_cancellable`。

- [ ] **Step 5: 验证 GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::game_navigation::tests`

Expected: 全部 PASS；不再存在活动弹窗分支测试。

- [ ] **Step 6: 提交**

```powershell
git add src-tauri/src/special_ops/game_navigation.rs src-tauri/src/special_ops/mod.rs src/components/app/special-ops-types.ts
git commit -m "feat(special_ops): 导航使用可取消固定等待"
```

### Task 4: 在校准列表配置时间

**Files:**
- Modify: `src/components/app/special-ops-utils.ts`
- Test: `src/components/app/special-ops-utils.test.ts`
- Modify: `src/components/app/special-ops-page.tsx:650-710`
- Test: `src/components/app/special-ops-page.test.tsx`

- [ ] **Step 1: 写输入解析失败测试**

```ts
expect(parseNavigationDelayMs("0")).toEqual({ok: true, value: 0});
expect(parseNavigationDelayMs("60000")).toEqual({ok: true, value: 60000});
for (const raw of ["", "-1", "1.5", "60001", "abc"]) {
    expect(parseNavigationDelayMs(raw)).toEqual({
        ok: false, message: "等待时间必须是 0–60000ms 的整数",
    });
}
```

- [ ] **Step 2: 验证 RED**

Run: `bunx vitest run src/components/app/special-ops-utils.test.ts`

Expected: FAIL，解析函数尚不存在。

- [ ] **Step 3: 写最小解析实现**

```ts
export function parseNavigationDelayMs(raw: string) {
    if (!/^\d+$/.test(raw)) return {ok: false as const, message: "等待时间必须是 0–60000ms 的整数"};
    const value = Number(raw);
    return value <= 60000
        ? {ok: true as const, value}
        : {ok: false as const, message: "等待时间必须是 0–60000ms 的整数"};
}
```

- [ ] **Step 4: 写页面失败测试**

```ts
expect(pageSource).toContain("跳过活动弹窗等待时间");
expect(pageSource).toContain("切换大厅视角等待时间");
expect(pageSource).toContain("点击特勤处前等待时间");
expect(pageSource).toContain("navigationSpaceDelayMs");
expect(pageSource).toContain("navigationTabDelayMs");
expect(pageSource).toContain("navigationSpecialOpsDelayMs");
```

- [ ] **Step 5: 实现流程列表 UI**

不要把时间伪造成 `CalibrationTarget`。`game.beaconMode` 行后插入 Space、Tab 两条专用时间行；`game.specialOps` 原点击点行增加第三个时间输入。输入使用本地字符串 draft、`inputMode="numeric"`、单位 `ms`；失焦解析成功才 `save`，失败设置页面错误。父 `fieldset disabled={hasActiveRun}` 继续锁定运行期编辑。

- [ ] **Step 6: 验证 GREEN**

```powershell
bunx vitest run src/components/app/special-ops-utils.test.ts src/components/app/special-ops-page.test.tsx
bun run build
```

Expected: tests 与 build PASS。

- [ ] **Step 7: 提交**

```powershell
git add src/components/app/special-ops-utils.ts src/components/app/special-ops-utils.test.ts src/components/app/special-ops-page.tsx src/components/app/special-ops-page.test.tsx
git commit -m "feat(special_ops): 校准列表配置导航等待时间"
```

### Task 5: 文档和完整门禁

**Files:**
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: 同步文档**

记录首尾模板识别、三段固定等待、无倒计时、两个废弃模板删除及 `game.specialOps` 点击点迁移。

- [ ] **Step 2: 定向验证**

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml
git diff --check
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::game_navigation::tests
bunx vitest run src/components/app/special-ops-utils.test.ts src/components/app/special-ops-page.test.tsx
```

- [ ] **Step 3: 统一门禁**

Run: `bun run check`

Expected: TypeScript、Vitest、coverage、Rustfmt、Clippy、Rust tests 全部通过。

- [ ] **Step 4: 索引与状态复核**

```powershell
codegraph sync
git diff --check
git status --short --branch
```

- [ ] **Step 5: 提交文档**

```powershell
git add AGENTS.md droid-wiki/features/special-ops.md
git commit -m "docs(special_ops): 更新导航固定等待流程"
```

## 真实桌面验收

1. 重新设置 `game.specialOps` 点击点。
2. 分别设置三个毫秒时间并启动导航试运行。
3. 确认 operation window 不出现 3、2、1 倒计时。
4. 确认顺序：点击烽火地带 → `Space` → `Tab` → 点击特勤处。
5. 固定等待期间测试普通取消与紧急停止，确认后续输入不再发生。
6. 确认最终仍需 `game.stationGrid` 双采样命中才成功。
