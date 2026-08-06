# 制作台点击点复用 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 删除四个重复的“进入制作列表”点击点，让收取和进入列表复用对应制作台点击点，同时保留四台独立的制作列表就绪判定。

**Architecture:** 仅调整 `special_ops` 校准目标目录、guard 与执行预检键集合。`normalize_settings` 已按默认目标白名单重建校准数组，因此从默认目录删键即可自动清理旧配置，无需新增迁移函数。

**Tech Stack:** Rust、Tauri 2、内联 Rust 单元测试、CodeGraph、Bun 质量门禁。

---

### Task 1: 用测试锁定点击点复用与旧配置迁移

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Test: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1: 写默认目标失败测试**

在现有 `#[cfg(test)]` 模块加入测试，要求默认目录不再包含 `craft.openRecipeList.*`，四台入口与四台就绪判定仍存在：

```rust
#[test]
fn craft_recipe_list_reuses_station_click_targets() {
    let keys = default_calibration_targets()
        .into_iter()
        .map(|target| target.key)
        .collect::<std::collections::HashSet<_>>();

    for kind in StationKind::all() {
        let suffix = kind.calibration_suffix();
        assert!(keys.contains(&format!("craft.station.{suffix}")));
        assert!(keys.contains(&format!("craft.recipeListReady.{suffix}")));
        assert!(!keys.contains(&format!("craft.openRecipeList.{suffix}")));
    }
}
```

- [ ] **Step 2: 写旧配置迁移失败测试**

向默认环境插入一个旧 `craft.openRecipeList.technicalCenter` 目标，规范化后断言旧键消失且 `craft.station.technicalCenter` 保留。

- [ ] **Step 3: 验证 RED**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml craft_recipe_list_reuses_station_click_targets
cargo test --manifest-path src-tauri/Cargo.toml normalize_removes_legacy_open_recipe_list_targets
```

预期：第一条因默认目录仍包含 `craft.openRecipeList.*` 失败；第二条因白名单尚包含旧键失败。

### Task 2: 删除重复目标并复用制作台入口

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Verify: `docs/superpowers/specs/2026-07-22-特勤处多账号自动化-设计草案.md`
- Verify: `docs/superpowers/specs/2026-07-23-特勤处多账号自动化-正式设计.md`
- Verify: `droid-wiki/features/special-ops.md`

- [ ] **Step 1: 写最小实现**

从 `default_guard_any_of` 删除四个 `craft.openRecipeList.*` 分支；从 `default_calibration_targets` 删除四个重复点击点；从 `required_execution_target_keys` 的制作前缀数组删除 `craft.openRecipeList`。保留四个 `craft.station.*`、`craft.idle.*`、`craft.recipeListReady.*` 与 `craft.recipe.*`。

- [ ] **Step 2: 验证 GREEN**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml craft_recipe_list_reuses_station_click_targets
cargo test --manifest-path src-tauri/Cargo.toml normalize_removes_legacy_open_recipe_list_targets
```

预期：两条测试通过。

- [ ] **Step 3: 检查所有引用**

运行：

```powershell
rg -n "craft\.openRecipeList|进入制作列表点击区域" src src-tauri docs droid-wiki
```

预期：源码、设计文档与 wiki 均无旧校准键或旧 UI 文案。

- [ ] **Step 4: 刷新索引并执行全量门禁**

运行：

```powershell
codegraph sync
bun run check
git diff --check
```

预期：CodeGraph 同步成功；TypeScript、Vitest、coverage、Rust fmt、Clippy 与 Rust tests 全部通过；diff 无空白错误。

- [ ] **Step 5: 等待现场验证**

不创建 commit。用户重启开发版后确认校准列表只显示四个制作台点击点，且仍显示四个制作列表就绪区域；现场通过后再提交。
