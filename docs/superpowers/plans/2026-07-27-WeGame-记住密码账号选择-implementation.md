# WeGame 记住密码账号选择 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. 本计划只允许 Inline Execution，不调用子代理。

**Goal:** 删除密码保存与模拟输入，改为 OCR 扫描 WeGame 已记住账号列表、精确选择目标 QQ、剪贴板复核后提交登录。

**Architecture:** `login_flow.rs` 只编排可测试状态机；Windows OCR、剪贴板和键鼠动作分别放入小型 adapter。生产 driver 组合现有截图、模板守卫、倒计时和输入串行化；前端只维护 QQ 与新增校准目标。

**Tech Stack:** Rust 2021、Tokio、Windows Runtime OCR (`windows 0.61.3`)、Win32 clipboard (`windows-sys 0.61`)、Enigo、React 19、TypeScript、Vitest。

**状态：** Task 1–8 已完成代码实现、文档同步与自动化验证；剩余 WeGame 实机校准和验收。

---

## 文件结构

- `src-tauri/src/special_ops/remembered_account.rs`：OCR 结果标准化、双采样重合、已见集合和到底判定。
- `src-tauri/src/special_ops/windows_ocr.rs`：`DynamicImage` 转 Windows `SoftwareBitmap` 并输出文字 bounding box。
- `src-tauri/src/special_ops/windows_clipboard.rs`：清空、重试读取 `CF_UNICODETEXT`。
- `src-tauri/src/special_ops/login_flow.rs`：记住账号登录状态机和 driver contract。
- `src-tauri/src/special_ops/login_runtime.rs`：生产 driver，连接截图/OCR/剪贴板/键鼠。
- `src-tauri/src/special_ops/mod.rs`：配置迁移、校准、preflight、冻结配置、状态映射。
- `src-tauri/src/input_simulation.rs`：可取消坐标点击、区域滚轮、区域双击与 `Ctrl+C`。
- `src/components/app/special-ops-{types,utils,page}.tsx?`：删密码 UI，增加账号列表校准文案和新步骤。
- `droid-wiki/features/special-ops.md`、`README.md`、`AGENTS.md`：同步持久化结构、登录行为和原生依赖。

### Task 1: 删除密码配置并迁移校准键

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src/components/app/special-ops-types.ts`
- Modify: `src/components/app/special-ops-utils.ts`
- Modify: `src/components/app/special-ops-utils.test.ts`
- Modify: `src/components/app/special-ops-page.tsx`

- [ ] **Step 1: 写失败测试**

Rust 测试覆盖旧 JSON `password` 可读取但重新序列化消失、QQ 仅允许纯数字、`wegame.account` 迁移为 `wegame.selectedAccount`、`wegame.password` 删除。Vitest 改为账号只凭 `enabled + qqAccount` 入选。

```rust
assert!(!serde_json::to_string(&settings).unwrap().contains("password"));
assert!(validate_login_trial_ready(&settings, "letters").unwrap_err().contains("纯数字"));
```

```ts
expect(eligibleLoginTrialAccounts([account({ qqAccount: "123", enabled: true })])).toHaveLength(1)
```

- [ ] **Step 2: 运行 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::tests::legacy_password_is_dropped_after_load`
Expected: FAIL，`AccountPlan` 仍序列化 `password`。

Run: `bunx vitest run src/components/app/special-ops-utils.test.ts`
Expected: FAIL，无密码账号仍被过滤。

- [ ] **Step 3: 最小实现**

删除 Rust/TS `password` 字段、默认值、UI、签名和 preflight 依赖。校准目标改为：

```rust
("wegame.accountDropdown", "已记住账号列表展开按钮", ClickPoint),
("wegame.accountList", "已记住账号列表 OCR 区域", RecognitionRegion),
("wegame.selectedAccount", "已选账号双击复制区域", ClickPoint),
```

normalize 时将旧 `wegame.account` rect 克隆到 `wegame.selectedAccount`，删除旧 account/password target。

- [ ] **Step 4: 运行 GREEN 并提交**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::tests`
Run: `bunx vitest run src/components/app/special-ops-utils.test.ts`
Expected: PASS。

Commit: `feat(special-ops): 删除密码配置并迁移账号校准`

### Task 2: Windows OCR adapter

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Create: `src-tauri/src/special_ops/windows_ocr.rs`

- [ ] **Step 1: 写失败测试**

纯逻辑测试先定义 `OcrWord { text, rect }`，验证只保留完整纯数字、相对坐标转屏幕坐标、相同 QQ 的 box 可查询。

```rust
assert_eq!(numeric_words(words), vec![OcrWord::new("123456", rect)]);
assert_eq!(to_screen_rect(rect, region), ScreenRect { x: region.x + 10, y: region.y + 20, .. });
```

- [ ] **Step 2: 运行 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml windows_ocr`
Expected: FAIL，module/API 不存在。

- [ ] **Step 3: 最小实现**

添加：

```toml
windows = { version = "0.61.3", features = ["Foundation", "Graphics_Imaging", "Media_Ocr", "Storage_Streams"] }
```

Windows 实现：RGBA bytes 写入 `DataWriter`，`DetachBuffer` 后创建 `SoftwareBitmap`，`OcrEngine::TryCreateFromUserProfileLanguages()`，await `RecognizeAsync`，遍历 `Lines()/Words()`。非 Windows 返回“仅支持 Windows”。

- [ ] **Step 4: 运行 GREEN 并提交**

Run: `cargo test --manifest-path src-tauri/Cargo.toml windows_ocr`
Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: PASS。

Commit: `feat(special-ops): 接入 Windows 账号列表 OCR`

### Task 3: 剪贴板复核 adapter

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/special_ops/windows_clipboard.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1: 写失败测试**

提取可测纯函数 `normalize_copied_qq`：trim 后必须纯数字；空值、非数字、NUL 尾部拒绝或清理后拒绝。

- [ ] **Step 2: 运行 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml windows_clipboard`
Expected: FAIL。

- [ ] **Step 3: 最小实现**

为 `windows-sys` 增加 `Win32_System_DataExchange`、`Win32_System_Memory`。用 RAII 保证 `CloseClipboard`/`GlobalUnlock`；`EmptyClipboard` 清空；读取 `CF_UNICODETEXT`，按首个 NUL 截断并 UTF-16 解码。占用时每 50ms 重试，最长 1 秒。

- [ ] **Step 4: 运行 GREEN 并提交**

Run: `cargo test --manifest-path src-tauri/Cargo.toml windows_clipboard`
Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: PASS。

Commit: `feat(special-ops): 增加 QQ 剪贴板精确复核`

### Task 4: 可取消滚轮、双击、复制动作

**Files:**
- Modify: `src-tauri/src/input_simulation.rs`

- [ ] **Step 1: 写失败测试**

扩展 fake emitter action 记录，分别断言：滚轮先移动到区域中心再向下 scroll；双击产生两次 click 且间隔可取消；复制按住 Ctrl 时发送 `c`，取消/异常后释放 Ctrl。

- [ ] **Step 2: 运行 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml input_simulation`
Expected: FAIL，新 API 不存在。

- [ ] **Step 3: 最小实现**

`InputEmitter` 增加 `scroll_vertical`。导出 `click_point_cancellable`、`scroll_region_down_cancellable`、`double_click_region_and_copy_cancellable`，全部走现有 `run_serialized_input`、generation 和 `TrackedModifierGuard`。

- [ ] **Step 4: 运行 GREEN 并提交**

Run: `cargo test --manifest-path src-tauri/Cargo.toml input_simulation`
Expected: PASS。

Commit: `feat(special-ops): 增加可取消账号选择输入动作`

### Task 5: 记住账号扫描状态机

**Files:**
- Create: `src-tauri/src/special_ops/remembered_account.rs`
- Modify: `src-tauri/src/special_ops/login_flow.rs`

- [ ] **Step 1: 写失败测试**

覆盖首屏命中、滚动后命中、两次 OCR 不一致不点击、动态排序、连续两屏无新增判到底、剪贴板不匹配不提交、补偿重启一次、登录按钮每 run 一次、各阶段取消。

```rust
assert_eq!(stable_match(&first, &second, "123"), Some(second_box));
assert!(stable_match(&first, &moved, "123").is_none());
assert!(scan.note_screen(set!["1", "2"]));
assert!(!scan.note_screen(set!["1", "2"]));
assert!(scan.reached_bottom_after_second_unchanged());
```

- [ ] **Step 2: 运行 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::login_flow`
Expected: FAIL，旧流程仍调用 `replace_text`。

- [ ] **Step 3: 最小实现**

driver contract 改为 `ocr_account_list`、`click_screen_point`、`scroll_account_list`、`copy_selected_account`。流程新增 `OpenAccountList/ScanRememberedAccounts/SelectRememberedAccount/VerifySelectedAccount`，删除 `InputAccount/InputPassword`。双采样间隔 400ms；扫描 deadline 3 分钟；未找到或复核失败返回可区分 failure kind，外层只对选择/复核补偿重启一轮。

- [ ] **Step 4: 运行 GREEN 并提交**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::login_flow`
Expected: PASS。

Commit: `feat(special-ops): 编排记住密码账号扫描登录`

### Task 6: 生产 runtime 接线

**Files:**
- Modify: `src-tauri/src/special_ops/login_runtime.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`

- [ ] **Step 1: 写失败测试**

为 runtime adapter 注入截图/OCR/clipboard/input seam，测试错误转 `LoginObservation`，键鼠动作仍执行 `3/2/1 → focus → guard → action`，纯 OCR 等待不触发倒计时。

- [ ] **Step 2: 运行 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::login_runtime`
Expected: FAIL。

- [ ] **Step 3: 最小实现**

账号列表截图调用现有 `capture_region`；OCR 结果携带 region 偏移；坐标点击、区域滚轮、双击复制连接 Task 4；复制前清空剪贴板并在动作后读取。移除 `WEGAME_LOGIN_INPUT_TIMING` 与 `replace_text`。

- [ ] **Step 4: 运行 GREEN 并提交**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops::login_runtime`
Expected: PASS。

Commit: `feat(special-ops): 接通 WeGame 记住账号生产运行时`

### Task 7: UI、preflight、状态与文档

**Files:**
- Modify: `src/components/app/special-ops-types.ts`
- Modify: `src/components/app/special-ops-page.tsx`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `README.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: 写失败测试**

更新序列化、step message、preflight target 集合测试；断言试运行不再要求密码/输入区域，要求三个新校准目标；失败映射区分 `NeedsManualLogin`、`LoginFailed`、全面暂停。

- [ ] **Step 2: 运行 RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops`
Run: `bun run test`
Expected: FAIL，类型和旧期望未同步。

- [ ] **Step 3: 最小实现**

UI 删除密码输入；账号提示改为“纯数字 QQ，需提前登录并记住密码”；校准列表加入下拉、列表 OCR、顶部账号；operation window 显示新步骤；文档同步新持久化和登录流程。

- [ ] **Step 4: 运行 GREEN 并提交**

Run: `cargo test --manifest-path src-tauri/Cargo.toml special_ops`
Run: `bun run test`
Expected: PASS。

Commit: `docs(special-ops): 同步记住账号登录配置与流程`

### Task 8: 清理旧输入扩展并执行全量门禁

**Files:**
- Modify: `src-tauri/src/input_simulation.rs`
- Modify: `docs/superpowers/plans/2026-07-27-WeGame-记住密码账号选择-implementation.md`

- [ ] **Step 1: 删除仅由旧密码输入使用的 API**

确认无 caller 后删除 `TextInputTiming`、`replace_text_at_region_with_timing_cancellable` 及对应节流测试；保留其他工具仍调用的通用输入 API。

- [ ] **Step 2: 同步 CodeGraph**

Run: `codegraph sync`
Expected: 索引成功。

- [ ] **Step 3: 全量验证**

Run: `bun run check`
Expected: TypeScript、Vitest、coverage、fmt、Clippy、Rust tests 全部 PASS；失败原样报告并修正。

- [ ] **Step 4: 检查 diff 与提交**

Run: `git diff --check`
Run: `git status --short`
Expected: 无 whitespace error，仅包含本功能文件及任务开始前既有 `Cargo.toml` 行尾状态。

Commit: `test(special-ops): 完成记住账号登录质量门禁`

## 自查

- Spec coverage：密码清除、动态 OCR、双采样、滚动到底、剪贴板精确复核、单次提交、失败分类、取消、校准、UI、文档均有对应 Task。
- Placeholder scan：无 TBD/TODO；每个 Task 给出测试、命令、最小实现和提交。
- Type consistency：统一使用 `OcrWord`、`ScreenRect`、四个新 `LoginStep`、三个新校准 key。
