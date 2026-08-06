# WeGame 账号列表扫描行中心与诊断 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用已确认的账号列表 OCR 行中心代替机械三等分点击，并在未找到账号时持久化可读、脱敏的扫描轨迹。

**Architecture:** `remembered_account.rs` 负责从两份 OCR 样本导出稳定行槽位、驱动账号选择循环和生成脱敏轨迹；`login_runtime.rs` 负责真实双采样、相对坐标到屏幕坐标转换与点击；`login_flow.rs` 和 `mod.rs` 仅传递、持久化已脱敏的失败说明。账号身份仍只由“点击行后复制顶部 QQ”精确比较决定。

**Tech Stack:** Rust、Tokio、Windows OCR、Tauri 2、Cargo test。

---

## 文件结构

- `src-tauri/src/special_ops/remembered_account.rs`：行槽位模型、双样本稳定性判断、选择循环、脱敏轨迹及单元测试。
- `src-tauri/src/special_ops/login_runtime.rs`：真实 OCR 双采样、稳定行槽位转屏幕点击坐标。
- `src-tauri/src/special_ops/login_flow.rs`：把未命中轨迹转成不含完整 QQ 的 `NeedsManualLogin` 说明。
- `src-tauri/src/special_ops/mod.rs`：试运行和多账号 round 持久化、显示相同脱敏失败说明。
- `droid-wiki/features/special-ops.md`：更新已记住账号扫描的真实行为和失败诊断说明。

### Task 1: 锁定账号行槽位与脱敏诊断行为

**Files:**
- Modify: `src-tauri/src/special_ops/remembered_account.rs`
- Test: `src-tauri/src/special_ops/remembered_account.rs`

- [ ] **Step 1: 写失败测试，稳定 OCR 行中心优先于三等分**

在测试模块加入三个相对 `y` 中心为 `18`、`54`、`90` 的样本对，断言 `derive_visible_row_slots(&first, &second, 108)` 返回三个 `AccountRowSlot::Ocr` 槽位；第二槽位中心必须为 `54`，而非三等分的 `50`。

```rust
assert_eq!(
    derive_visible_row_slots(&first, &second, 108),
    vec![
        AccountRowSlot::Ocr { index: 0, center_y: 18 },
        AccountRowSlot::Ocr { index: 1, center_y: 54 },
        AccountRowSlot::Ocr { index: 2, center_y: 90 },
    ],
);
```

- [ ] **Step 2: 运行定向测试，确认当前代码尚无行中心模型**

Run: `cargo test --manifest-path src-tauri/Cargo.toml remembered_account::tests::stable_ocr_rows_use_detected_centers`

Expected: FAIL，缺少 `derive_visible_row_slots` 或 `AccountRowSlot`。

- [ ] **Step 3: 写失败测试，少于三行只扫描实际槽位、不稳定时回退**

为两个稳定行样本断言只返回两个 OCR 槽位；为样本行数不同或同序中心相差超过 8 像素的样本断言返回三个 `AccountRowSlot::Fallback { index: 0..=2 }`。空样本不在此函数中回退，保留给列表可见性校验。

```rust
assert_eq!(stable_two_rows.len(), 2);
assert_eq!(unstable_rows, fallback_slots());
```

- [ ] **Step 4: 写失败测试，未命中轨迹只保存 QQ 尾四位**

将 `FakeDriver::select_row` 改为返回精确点击坐标；连续复制三个非目标 QQ 后断言：

```rust
let AccountSelectionError::NotFound { attempts } = error else { panic!() };
assert_eq!(attempts[1].page, 1);
assert_eq!(attempts[1].slot, 2);
assert_eq!(attempts[1].click_y, 754);
assert_eq!(attempts[1].copied_qq, "***3589");
assert!(!format!("{attempts:?}").contains("3079643589"));
```

- [ ] **Step 5: 实现最小行槽位和轨迹模型**

在 `remembered_account.rs` 定义：

```rust
const OCR_ROW_STABILITY_TOLERANCE_PX: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountRowSlot {
    Ocr { index: u8, center_y: i32 },
    Fallback { index: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AccountRowClick {
    pub index: u8,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountScanAttempt {
    pub page: u32,
    pub slot: u8,
    pub click_x: i32,
    pub click_y: i32,
    pub copied_qq: String,
}
```

`derive_visible_row_slots(first, second, list_height)` 必须先按纵向中心排序，把相差不超过 `8.0` 像素的 OCR 文字合并为同一行，再排除非有限、宽高非正或中心越出 `0..=list_height` 的边界。两样本都得到相同的 1–3 行数量且每个同序中心差不超过 `8.0` 时返回 OCR 槽位；其余非空组合返回 `fallback_slots()`。`redact_qq` 仅返回 `***` 加最后四位；空或非纯数字值返回“未复制到 QQ”。

- [ ] **Step 6: 改选择循环，保留复制后的精确身份校验**

将 trait 改为：

```rust
async fn visible_account_rows(
    &self,
    cancelled: Arc<AtomicBool>,
) -> Result<Vec<AccountRowSlot>, String>;

async fn select_row(
    &self,
    slot: AccountRowSlot,
    cancelled: Arc<AtomicBool>,
) -> Result<AccountRowClick, String>;
```

每页先读取已确认的槽位；对每槽位点击、复制顶部 QQ、立即向轨迹追加脱敏结果，再与 `target_qq` 精确比较。非目标时重新打开列表，页面扫描仍按上到下顺序；页内无新 QQ 时返回 `AccountSelectionError::NotFound { attempts }`。复制错误保持现有 `Driver` 语义，不伪造命中。

- [ ] **Step 7: 运行定向测试，确认选择循环与既有滚动停止规则仍成立**

Run: `cargo test --manifest-path src-tauri/Cargo.toml remembered_account`

Expected: PASS，覆盖首屏命中、稳定行中心、两行列表、不稳定回退、脱敏未命中、重复页到底。

### Task 2: 接入真实 Windows OCR 与屏幕点击

**Files:**
- Modify: `src-tauri/src/special_ops/login_runtime.rs`
- Test: `src-tauri/src/special_ops/remembered_account.rs`

- [ ] **Step 1: 写失败测试，运行时接收的 OCR 槽位携带真实点击坐标**

在 `remembered_account.rs` 测试中让 fake driver 收到 `AccountRowSlot::Ocr { index: 1, center_y: 54 }`，返回 `AccountRowClick { index: 1, x: 1719, y: 827 }`。断言选择循环记录该坐标而不从槽位索引重算。

- [ ] **Step 2: 运行定向测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml remembered_account::tests::not_found_trace_uses_driver_reported_click_coordinates`

Expected: FAIL，旧 trait 只接收 `u8` 行号且不返回点击坐标。

- [ ] **Step 3: 实现真实双采样与坐标转换**

将 `ProductionLoginDriver::verify_account_list_open` 改为返回 `Vec<AccountRowSlot>`：每次尝试截取 `wegame.accountList`、等待 400ms、再截取；两个样本都非空时调用 `derive_visible_row_slots`。三个尝试仍失败时返回 `ACCOUNT_LIST_UNAVAILABLE`。

`visible_account_rows` 调用该方法。`select_row` 对 `Ocr` 槽位使用 `region.y + center_y`，对 `Fallback` 槽位沿用原三等分公式；两者 X 均为区域水平中心。保留聚焦、鼠标停放、取消检查、按住点击时间和运行状态更新，但删除重复 OCR 守卫，因同一调用刚完成双采样确认。

- [ ] **Step 4: 运行 Rust 定向测试**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml remembered_account
cargo test --manifest-path src-tauri/Cargo.toml login_flow
```

Expected: PASS。

### Task 3: 失败结果端到端脱敏持久化

**Files:**
- Modify: `src-tauri/src/special_ops/login_flow.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Test: `src-tauri/src/special_ops/login_flow.rs`
- Test: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1: 写失败测试，`NeedsManualLogin` 不暴露完整复制 QQ**

让 fake driver 复制非数字或非目标 QQ，断言 `LoginFlowResult::NeedsManualLogin` 只携带 `failure_message`；格式化结果不得包含完整 QQ，仅允许 `***` 加尾四位。

```rust
assert!(failure_message.contains("***3589"));
assert!(!failure_message.contains("3079643589"));
```

- [ ] **Step 2: 写失败测试，试运行与 round 持久化复用相同脱敏说明**

在 `mod.rs` 的登录 fixture 中构造 `NeedsManualLogin { failure_message }`，断言 `AccountFailure.message` 和 `AccountRunError.message` 与该说明一致，且不含完整 QQ。

- [ ] **Step 3: 运行定向测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml "needs_manual_login"`

Expected: FAIL，旧结果仍携带 `target_qq` 与 `actual_qq`，持久化直接格式化完整实际 QQ。

- [ ] **Step 4: 最小改动登录结果和失败映射**

将 `LoginFlowResult::NeedsManualLogin` 改为：

```rust
NeedsManualLogin {
    account_id: String,
    failed_step: LoginStep,
    failure_message: String,
    failed_at: i64,
}
```

`NotFound { attempts }` 使用 `format_scan_attempts` 构造“未找到目标 QQ；扫描轨迹：页 1 槽位 2 (1719,827) -> ***3589”格式；`ListUnavailable` 保持“已记住账号列表未确认”；剪贴板无有效 QQ 的人工处理路径输出固定脱敏说明，绝不拼接原剪贴板文本。`apply_login_flow_result` 与 `ProductionRoundDriver::login` 只传递 `failure_message`。

- [ ] **Step 5: 运行定向测试**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml login_flow
cargo test --manifest-path src-tauri/Cargo.toml special_ops
```

Expected: PASS。

### Task 4: 文档与全量验证

**Files:**
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `docs/superpowers/specs/2026-08-04-WeGame-账号列表扫描行中心与诊断-design.md`

- [ ] **Step 1: 更新 Wiki**

在 WeGame 登录流程段落说明：账号列表 OCR 仅用于纵向行中心和列表展开确认，账号身份仍由复制顶部 QQ 精确比较；稳定一至三行按中心点击，异常回退三等分；最终未命中持久化页号、槽位、坐标和尾四位，不持久化完整 QQ。

- [ ] **Step 2: 更新设计文档验收记录**

在设计文档末尾加入“实现状态”段，列出已完成的稳定行中心、三等分回退、两行列表、脱敏轨迹、试运行和 round 持久化测试。

- [ ] **Step 3: 运行验证**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml remembered_account
cargo test --manifest-path src-tauri/Cargo.toml login_flow
cargo check --manifest-path src-tauri/Cargo.toml
bun run build
```

Expected: 全部 exit code 0。

- [ ] **Step 4: 运行扩展质量检查并记录真实结果**

Run:

```powershell
bun run test
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
codegraph sync
```

Expected: 通过；若环境或既有脏改动导致任一命令失败，保留错误输出并在交付中明确报告，不回滚用户改动。

- [ ] **Step 5: 不提交 Git**

用户已明确要求不提交。保留 `codex/special-ops-login` 上的工作区改动；不得暂存、提交、合并、创建 worktree 或清理既有脏文件。
