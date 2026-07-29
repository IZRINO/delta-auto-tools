# 制作中负向守卫取消正向验证门槛 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 允许未通过正向模板测试的 `craft.inProgress.<station>` 进入制作 runtime，由双采样负向守卫决定是否点击。

**Architecture:** 删除 `freeze_craft_run_config()` 对制作中目标验证签名的 preflight gate，保留共享图片、threshold、rect 与文件存在校验。模板测试和诊断签名继续保留，不改 runtime 的双命中、双低分、不一致和系统错误语义。

**Tech Stack:** Rust、Tauri 2、Bun、Vitest

**Execution constraint:** Inline Execution；不调用子代理，不创建 worktree。目标生产文件含历史修改，不整文件 stage，不创建混入历史改动的 commit。

---

### Task 1: 用失败测试固定未验证负向守卫可启动

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:7051-7131`

- [ ] **Step 1: 修改现有测试，使制作中目标不写验证签名**

把测试重命名为：

```rust
#[test]
fn craft_trial_preflight_allows_unverified_shared_in_progress_template() {
```

删除以下正向验证准备：

```rust
let environment = &fixture.settings.calibration_environments[0];
let target = environment
    .targets
    .iter()
    .find(|target| target.key == "craft.inProgress.technicalCenter")
    .unwrap();
let signature = resolved_calibration_signature(environment, target).unwrap();
let target =
    calibration_target_mut(&mut fixture.settings, "craft.inProgress.technicalCenter");
target.verified_signature = Some(signature);
target.verified_at_ms = Some(1);
```

在 `freeze_craft_run_config()` 前断言诊断签名为空：

```rust
let target = fixture.settings.calibration_environments[0]
    .targets
    .iter()
    .find(|target| target.key == "craft.inProgress.technicalCenter")
    .unwrap();
assert!(target.verified_signature.is_none());
assert!(target.verified_at_ms.is_none());
```

- [ ] **Step 2: 验证 RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib craft_trial_preflight_allows_unverified_shared_in_progress_template
```

Expected: FAIL，错误包含 `制作校准目标 craft.inProgress.technicalCenter 尚未测试或验证失效`。

### Task 2: 删除正向验证 gate，保留配置错误 gate

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs:1615-1665`
- Test: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1: 删除唯一的制作中验证签名检查**

从 `freeze_craft_run_config()` 删除：

```rust
if is_craft_in_progress_target(&key)
    && !resolved_verification_is_current(environment, target)
{
    return Err(format!("制作校准目标 {key} 尚未测试或验证失效"));
}
```

保留 `resolved_template_config()`、`std::fs::canonicalize(reference)` 与 rect 校验。

- [ ] **Step 2: 增加缺失配置回归断言**

在 Task 1 测试成功路径后增加三份 settings clone：

```rust
let mut no_shared_reference = fixture.settings.clone();
no_shared_reference.calibration_environments[0]
    .craft_in_progress_reference_image_path = None;
assert!(freeze_craft_run_config(
    &no_shared_reference,
    "selected",
    StationKind::TechnicalCenter,
)
.unwrap_err()
.contains("制作中状态尚未上传共享参考图"));

let mut missing_region = fixture.settings.clone();
calibration_target_mut(
    &mut missing_region,
    "craft.inProgress.technicalCenter",
)
.rect = None;
assert!(freeze_craft_run_config(
    &missing_region,
    "selected",
    StationKind::TechnicalCenter,
)
.unwrap_err()
.contains("未框选"));

let mut missing_reference_file = fixture.settings.clone();
missing_reference_file.calibration_environments[0]
    .craft_in_progress_reference_image_path = Some(
        tempfile::tempdir()
            .unwrap()
            .path()
            .join("missing.png")
            .display()
            .to_string(),
    );
assert!(freeze_craft_run_config(
    &missing_reference_file,
    "selected",
    StationKind::TechnicalCenter,
)
.unwrap_err()
.contains("制作参考图不存在"));
```

- [ ] **Step 3: 验证 GREEN 与 runtime fail-closed 测试**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib craft_trial_preflight_allows_unverified_shared_in_progress_template
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::craft_runtime::negative_guard_tests
cargo test --manifest-path src-tauri/Cargo.toml --lib special_ops::tests
```

Expected: 全部 PASS；制作中双命中零点击、双低分授权点击、采样不一致与采样错误零点击。

### Task 3: 同步行为文档并跑全量门禁

**Files:**
- Modify: `AGENTS.md`
- Modify: `droid-wiki/features/special-ops.md:93-95`

- [ ] **Step 1: 更新仓库约束摘要**

在 `AGENTS.md` 的 `special_ops` 项目概览中明确：

```text
制作试运行 preflight 只校验共享图、threshold、当前台 rect 和文件；四台测试结果仅用于诊断，不作为启动 gate。
```

- [ ] **Step 2: 更新 Wiki**

把 Wiki 中“共享参考图和当前台 rect 必须配置并通过当前签名测试”改为：

```text
共享“制作中”参考图和当前台 rect 必须配置；四台模板测试仅用于诊断当前画面能否命中，不作为制作试运行启动 gate。
```

保留首次点击由负向守卫授权、后续模板正向双采样的说明。

- [ ] **Step 3: 同步 CodeGraph 并运行统一门禁**

Run:

```powershell
codegraph sync
git diff --check
bun run check
```

Expected: CodeGraph 同步成功；diff 无空白错误；TypeScript、Vitest、coverage、Rust fmt、Clippy 与 Rust tests 全部 PASS。

- [ ] **Step 4: 交付实机复测**

用户选择未显示制作中状态的技术中心，直接启动制作试运行。Expected：不再报“尚未测试或验证失效”；双低分后点击技术中心，再按奖励页或制作列表分流。
