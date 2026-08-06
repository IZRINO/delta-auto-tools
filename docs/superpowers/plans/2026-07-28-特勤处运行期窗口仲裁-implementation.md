# 特勤处运行期窗口仲裁 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 特勤处登录与导航试运行键鼠/识图阶段隐藏其他功能窗口，run 结束后恢复启动前可见窗口。

**Architecture:** 在 `special_ops/mod.rs` 增加轻量窗口快照与恢复函数，复用 `start_login_run_with_resources` 统一启动/清理入口。只操作 `WebviewWindow` 显隐，不调用其他工具模块 stop API。

**Tech Stack:** Rust、Tauri 2、内联 Rust 单元测试、CodeGraph、Bun 质量门禁。

---

### Task 1: 先锁定窗口筛选规则

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Test: `src-tauri/src/special_ops/mod.rs`

- [x] **Step 1: 写失败测试**

```rust
#[test]
fn special_ops_window_guard_filters_operation_and_detects_selection_overlays() {
    assert!(!should_hide_for_special_ops("special-ops-operation", true));
    assert!(!should_hide_for_special_ops("timer-display-main", false));
    assert!(should_hide_for_special_ops("timer-display-main", true));
    assert!(is_active_selection_overlay("morse-overlay"));
    assert!(is_active_selection_overlay("timer-position-group"));
    assert!(!is_active_selection_overlay("timer-display-group"));
}
```

- [x] **Step 2: 验证 RED**

运行 `cargo test --manifest-path src-tauri/Cargo.toml special_ops_window_guard_filters_operation_and_detects_selection_overlays`。

预期：编译失败，函数尚未定义。

### Task 2: 接入统一启动与清理路径

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Test: `src-tauri/src/special_ops/mod.rs`

- [x] **Step 1: 写最小实现**

资源锁内枚举 `app.webview_windows()`：若存在活动框选或定位 overlay，返回中文错误；否则记录可见且非 `special-ops-operation` 的 label，依次 `hide()`。任何隐藏失败时恢复前面已隐藏窗口并返回错误。

把窗口快照捕获到 cleanup closure：先执行 `release_login_resources_unlocked`，再对仍存在的快照窗口逐个 `show()`。恢复失败记录日志并继续，缺失窗口跳过。

- [x] **Step 2: 加回归测试**

对启动回滚与统一 cleanup 注入闭包计数器，断言创建资源失败、普通结束与紧急结束均调用恢复闭包；不创建真实 Tauri 窗口。

- [x] **Step 3: 验证 GREEN**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops_window_guard_filters_operation_and_detects_selection_overlays
cargo test --manifest-path src-tauri/Cargo.toml start_login_run_with_resources
```

预期：新增与既有启动/清理测试通过。

### Task 3: 同步文档与验证

**Files:**
- Modify: `droid-wiki/features/special-ops.md`
- Verify: `docs/superpowers/specs/2026-07-28-特勤处运行期窗口仲裁-design.md`

- [x] **Step 1: 更新 wiki**

记录试运行启动前隐藏其他窗口、保留操作提示窗口、活动框选/定位 overlay 阻止启动、退出后恢复启动前可见窗口。

- [x] **Step 2: 刷新索引与全量验证**

运行 `codegraph sync`、`bun run check` 与 `git diff --check`。

- [ ] **Step 3: 等待现场验证**

不创建 commit。打开主窗口、计时器/计数器/连发器显示窗口后启动导航试运行，确认它们隐藏；run 结束后仅恢复启动前可见窗口。
