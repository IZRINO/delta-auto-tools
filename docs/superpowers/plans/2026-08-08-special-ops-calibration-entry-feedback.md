# 特勤处校准反馈与交易行入口识别 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. User requires Inline Execution; do not dispatch subagents. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复限时商品校准/识色反馈，加入颜色吸取与研发等待行配置，并将交易行入口改为模板识别后点击。

**Architecture:** 保留现有 calibration overlay、revision 保存和 recognition watcher。颜色吸取复用 `capture_region`、`average_region_rgb`；测试反馈扩展现有结果文本。`market.entry` 从 `ClickPoint` 改为模板 `RecognitionRegion`，runtime 使用既有模板 observer 命中后点击，不新增独立流程状态。

**Tech Stack:** React 19、TypeScript、Tauri 2、Rust、Tokio、Vitest、Cargo。

**Execution constraint:** 在当前工作区 Inline 修改；不创建 worktree、不调用子代理、不提交 Git。

---

### Task 1: 校准入口与 target 类型

**Files:**
- Modify: `src/components/app/special-ops-page.tsx`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Test: `src/components/app/special-ops-page.test.tsx`
- Test: `src-tauri/tests/special_ops_async_command_contract.rs`

- [ ] **Step 1: 为不可执行校准增加可见反馈测试**

断言 `beginCalibration` 在非 native shell 或 controlsLocked 时设置错误，而不是静默返回；断言校准 target 列表包含 `limited.ready` 与 `market.entry`。

- [ ] **Step 2: 修正 `beginCalibration` 守卫**

将静默返回改为设置中文错误；Tauri 调用失败继续显示原错误。保持运行中禁止创建 overlay 的行为。

- [ ] **Step 3: 将 `market.entry` 改为模板识别区域**

`default_calibration_targets()` 使用 `RecognitionRegion` 和 `Template`；runtime 的入口动作先等待模板双采样命中，再点击命中区域中心。更新 target contract 测试。

- [ ] **Step 4: 运行定向测试**

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx
cargo test --manifest-path src-tauri/Cargo.toml special_ops_async_command_contract -- --nocapture
git diff --check
```

### Task 2: 限时商品等待配置与颜色反馈

**Files:**
- Modify: `src/components/app/special-ops-page.tsx`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tests/special_ops_async_command_contract.rs`

- [ ] **Step 1: 将等待字段移到校准 target 行**

`limited.research` 行编辑 `researchDelayMs`；`limited.ready` 行编辑 `readyTimeoutMs`。删除折叠区重复输入，底层字段不变。

- [ ] **Step 2: 为颜色测试扩展 UI 结果**

保存两次命中颜色、最近距离、通过状态和错误；测试按钮执行期间显示进行中，完成后在对应区域行显示结果。

- [ ] **Step 3: 新增颜色吸取 command**

复用已校准 `limited.color.N` 区域截图与 `average_region_rgb`，参数含 environment、region、color index、revision；校验 1–9/0–1，成功通过 coordinator 持久化对应颜色并返回 RGB。

- [ ] **Step 4: 增加吸取按钮与调用**

颜色输入旁放置两个吸取按钮，调用 command，保存结果；截图/窗口/输入失败显示错误且不修改配置。

- [ ] **Step 5: 运行限时商品定向测试**

```powershell
bunx vitest run src/components/app/special-ops-page.test.tsx
cargo test --manifest-path src-tauri/Cargo.toml limited_supply -- --nocapture
cargo check --manifest-path src-tauri/Cargo.toml
git diff --check
```

### Task 3: 文档与完整验证

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `droid-wiki/features/special-ops.md`

- [ ] **Step 1: 同步校准、颜色吸取、交易行入口识别说明**

说明 `market.entry` 为模板识别与点击区域；限时等待配置位于对应校准行；测试反馈显示采样详情；颜色吸取不保存图片。

- [ ] **Step 2: 运行质量门禁**

```powershell
bun run build
bun run test
cargo test --manifest-path src-tauri/Cargo.toml
bun run check
git diff --check
```

