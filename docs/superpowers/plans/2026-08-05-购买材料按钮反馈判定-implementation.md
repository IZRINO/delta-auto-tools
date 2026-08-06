# 购买材料按钮反馈判定 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` for inline execution. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 购买材料后仅依据“继续动作按钮”或“购买按钮仍出现”判定结果；第三次购买按钮仍出现时隔离账号。

**Architecture:** `ammo_runtime` 与 `craft_runtime` 各保留独立购买校准目标，但共用相同状态机语义。制作运行时以显式 `craft.isolated` 失败向单台试运行、四台批处理和多账号轮次传递账号隔离，不把当前制作台写为不确定。

**Tech Stack:** Rust、Tokio、现有模板双采样驱动、Cargo unit tests。

---

### Task 1: 子弹购买反馈 RED 测试

**Files:**
- Modify: `src-tauri/src/special_ops/ammo_runtime.rs`

- [x] **Step 1: 写入失败测试**

验证三次 `ammo.purchase` 稳定出现时返回 `AmmoRunStop::Isolated`；前两次出现、第三次转为 `ammo.exchange` 时继续确认并成功；购买后无稳定模板时返回 `AmmoRunStop::Uncertain`。

- [x] **Step 2: 运行失败测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::ammo_runtime::tests::purchase_ -- --nocapture`

Expected: 当前实现将购买按钮视为未知状态，至少一个断言失败。

### Task 2: 制作购买反馈 RED 测试

**Files:**
- Modify: `src-tauri/src/special_ops/craft_runtime.rs`

- [x] **Step 1: 写入失败测试**

验证三次 `CraftButton::Purchase` 返回 `craft.isolated`；两次购买后出现 `CraftButton::Produce` 时继续生产；购买后采样失败时保留不确定状态。

- [x] **Step 2: 运行失败测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::craft_runtime::fixed_probe_tests::purchase_ -- --nocapture`

Expected: 当前实现只在第一次购买后等待生产按钮，测试失败。

### Task 3: 最小状态机实现

**Files:**
- Modify: `src-tauri/src/special_ops/ammo_runtime.rs`
- Modify: `src-tauri/src/special_ops/craft_runtime.rs`
- Modify: `src-tauri/src/special_ops/craft_trial.rs`

- [x] **Step 1: 子弹状态机**

购买后固定等待 1 秒，双采样仅观察 `ammo.exchange` 或 `ammo.purchase`。购买仍出现时最多重试两次；第三次出现返回隔离；观察错误映射为不确定。

- [x] **Step 2: 制作状态机**

向 `CraftButton` 加入 `Purchase`，购买后固定等待 1 秒，仅观察 `craft.produce` 或 `craft.purchase`。第三次购买仍出现返回显式 `craft.isolated` 失败。

- [x] **Step 3: 运行失败测试并转绿**

Run: `cargo test --manifest-path src-tauri/Cargo.toml purchase_ -- --nocapture`

Expected: PASS。

### Task 4: 隔离持久化映射与回归验证

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `droid-wiki/features/special-ops.md`

- [x] **Step 1: 映射 `craft.isolated`**

单制作台试运行、四台批处理和多账号 round 均保存账号 `Isolated`。round 不把触发隔离的制作台写为 `Uncertain`。

- [x] **Step 2: 写入并运行映射测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::tests::craft_isolation -- --nocapture`

Expected: PASS。

- [x] **Step 3: 更新文档并运行验证**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
