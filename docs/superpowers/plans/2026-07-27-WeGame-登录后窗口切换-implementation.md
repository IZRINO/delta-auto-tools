# WeGame 登录后窗口切换 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** WeGame 登录后主窗口转交给 `browser.exe` 子进程时，试运行仍能安全聚焦窗口并继续选择游戏。

**Architecture:** 保留按完整 exe 路径识别根进程的边界；只为 WeGame 聚焦新增进程树窗口查找与复核。进程树由 ToolHelp 快照的 PID/PPID 构建，窗口仍按可见、无 owner、最大面积选择；终止流程和游戏窗口查找不变。

**Tech Stack:** Rust、windows-sys、Tokio、Cargo test、Bun 质量门禁

---

### Task 1: 进程树 PID 计算

**Files:**
- Modify: `src-tauri/src/special_ops/desktop_runtime.rs:43-59,214-308,569-1006`

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn process_tree_includes_direct_and_nested_descendants_only() {
    let entries = vec![(10, 0), (11, 10), (12, 11), (20, 0), (21, 20)];
    assert_eq!(process_tree_ids(&[10], &entries), vec![10, 11, 12]);
}

#[test]
fn process_tree_supports_multiple_matching_roots() {
    let entries = vec![(10, 0), (11, 10), (20, 0), (21, 20), (30, 0)];
    assert_eq!(process_tree_ids(&[10, 20], &entries), vec![10, 11, 20, 21]);
}
```

- [ ] **Step 2: 验证 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml process_tree_ -- --nocapture
```

Expected: 编译失败，提示找不到 `process_tree_ids`。

- [ ] **Step 3: 写最小实现**

```rust
fn process_tree_ids(root_ids: &[u32], entries: &[(u32, u32)]) -> Vec<u32> {
    let mut ids = root_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    loop {
        let before = ids.len();
        for &(pid, parent_pid) in entries {
            if ids.binary_search(&parent_pid).is_ok() && ids.binary_search(&pid).is_err() {
                ids.push(pid);
                ids.sort_unstable();
            }
        }
        if ids.len() == before {
            return ids;
        }
    }
}
```

将 ToolHelp 循环抽为 `scan_process_entries() -> Result<Vec<ProcessEntry>, String>`；`ProcessEntry` 保存 `pid`、`parent_pid`、`executable_name: [u16; 260]`。现有 `scan_process_entries_by_name` 改为过滤该完整快照，错误文案与精确终止行为不变。

- [ ] **Step 4: 验证 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml process_tree_ -- --nocapture
```

Expected: 2 个测试通过。

- [ ] **Step 5: 提交**

```powershell
git add src-tauri/src/special_ops/desktop_runtime.rs
git commit -m "test(special-ops): 覆盖 WeGame 进程树计算"
```

### Task 2: 进程树窗口发现与竞态复核

**Files:**
- Modify: `src-tauri/src/special_ops/desktop_runtime.rs:32-177,214-308,569-1006`

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn process_tree_window_selects_descendant_and_rejects_foreign_browser() {
    let entries = vec![(10, 0), (11, 10), (12, 11), (99, 0)];
    let windows = vec![
        WindowCandidate { hwnd: 1, pid: 99, visible: true, owned: false, area: 2_000 },
        WindowCandidate { hwnd: 2, pid: 12, visible: true, owned: false, area: 1_000 },
    ];
    let ids = process_tree_ids(&[10], &entries);
    assert_eq!(select_primary_window(&ids, &windows), Some(WindowIdentity {
        process_id: 12,
        handle: 2,
    }));
}

#[test]
fn selected_pid_must_still_belong_to_current_process_tree() {
    assert!(process_tree_contains(&[10], &[(10, 0), (11, 10)], 11));
    assert!(!process_tree_contains(&[10], &[(10, 0), (11, 99)], 11));
}
```

- [ ] **Step 2: 验证 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml process_tree_window_ -- --nocapture
```

Expected: 编译失败，提示找不到 `process_tree_contains`。

- [ ] **Step 3: 写最小实现**

扩展 `DesktopRuntime`：

```rust
fn find_primary_window_in_tree(&self, exe: &Path) -> Result<Option<WindowIdentity>, String>;
fn restore_and_focus_in_tree(&self, exe: &Path, window: WindowIdentity) -> Result<(), String>;
```

加入：

```rust
fn process_tree_contains(root_ids: &[u32], entries: &[(u32, u32)], pid: u32) -> bool {
    process_tree_ids(root_ids, entries).binary_search(&pid).is_ok()
}
```

`find_primary_window_in_tree`：canonicalize exe → 单次进程快照 → 按文件名缩小根候选 → `query_process_path` 精确确认根 PID → `process_tree_ids` → 枚举窗口 → `select_primary_window` → 再取一次快照确认所选 PID 仍属于当前精确根进程树。

`restore_and_focus_in_tree`：`IsWindow` → HWND/PID 一致性 → 再取快照复核进程树成员 → 恢复窗口 → `SetForegroundWindow` → 验证前台窗口。抽取私有 `restore_and_focus_verified(window)` 复用 Win32 操作；现有 `restore_and_focus` 仍先校验所选 PID 的完整路径。

- [ ] **Step 4: 验证 GREEN 与回归**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops::desktop_runtime::tests -- --nocapture
```

Expected: desktop runtime 全部测试通过，受控 helper 测试保持 ignored。

- [ ] **Step 5: 提交**

```powershell
git add src-tauri/src/special_ops/desktop_runtime.rs
git commit -m "fix(special-ops): 支持 WeGame 子进程窗口"
```

### Task 3: 接入登录流程并同步文档

**Files:**
- Modify: `src-tauri/src/special_ops/login_runtime.rs:651-662`
- Modify: `droid-wiki/features/special-ops.md`

- [ ] **Step 1: 接入 WeGame 专用窗口能力**

```rust
let window = runtime
    .find_primary_window_in_tree(&executable)?
    .ok_or_else(|| "未找到 WeGame 窗口".to_string())?;
runtime.restore_and_focus_in_tree(&executable, window)
```

`find_process_window` 保持调用 `find_primary_window`，避免改变游戏 PID/HWND 规则。

- [ ] **Step 2: 同步 Wiki**

在 `droid-wiki/features/special-ops.md` 的 WeGame 窗口约束中写明：聚焦允许 canonical 配置 exe 的进程树顶层窗口；终止仍只精确匹配配置 exe，不递归结束后代；游戏窗口仍按游戏 exe 自身识别。

- [ ] **Step 3: 运行定向检查**

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml special_ops::desktop_runtime::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml special_ops::login_flow::tests -- --nocapture
```

Expected: 全部通过；helper 测试仅显示受控 ignored。

- [ ] **Step 4: 提交接线与文档**

```powershell
git add src-tauri/src/special_ops/login_runtime.rs droid-wiki/features/special-ops.md
git commit -m "fix(special-ops): 登录后聚焦 WeGame 主窗口"
```

### Task 4: 全量门禁与索引同步

**Files:**
- Modify: `.codegraph/`（由索引工具管理，不手工编辑）

- [ ] **Step 1: 同步 CodeGraph**

```powershell
codegraph sync
```

Expected: 索引同步成功。

- [ ] **Step 2: 运行统一质量门禁**

```powershell
bun run check
```

Expected: TypeScript、Vitest、coverage、Rust fmt、Clippy `-D warnings`、Rust tests 全部通过。

- [ ] **Step 3: 检查工作区**

```powershell
git status --short
git log -5 --oneline
```

Expected: 无生产代码或 Wiki 遗漏。

完成后实机试运行：登录后倒计时结束应点击置顶游戏入口，随后等待并点击启动游戏；不得在 `OpenGameEntry` 结束。
