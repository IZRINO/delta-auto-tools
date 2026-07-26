# WeGame 登录输入节流 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为特勤处 WeGame 账号与密码输入增加可取消的聚焦等待、逐字符节流和输入结束等待。

**Architecture:** 在共享输入模拟模块用 `TextInputTiming` 承载三段延时，避免三个同类型参数错位。登录 runtime 提供唯一固定常量 `800ms / 100ms / 500ms`，其他输入流程不调用该时序。

**Tech Stack:** Rust、Tokio、Enigo、Cargo test、CodeGraph

---

## 文件结构

- Modify: `src-tauri/src/input_simulation.rs`：实现三段可取消输入延时及底层行为测试。
- Modify: `src-tauri/src/special_ops/login_runtime.rs`：定义并传入 WeGame 登录专用固定时序。
- Modify: `droid-wiki/features/special-ops.md`：记录真实登录输入时序。

### Task 1: 共享输入模拟支持三段可取消延时

**Files:**
- Modify: `src-tauri/src/input_simulation.rs:13-365`
- Test: `src-tauri/src/input_simulation.rs:740-1175`

- [ ] **Step 1: 写失败测试**

在 `RecordingEmitter` 中记录点击时刻与首字符时刻：

```rust
clicked_at: StdMutex<Option<Instant>>,
first_character_at: StdMutex<Option<Instant>>,
```

`click_left` 保存 `Instant::now()`；非 Ctrl 组合的首个 `Key::Unicode` 保存首字符时刻。新增测试：

```rust
#[tokio::test]
async fn replace_text_waits_for_focus_and_settle() {
    let _test_guard = lock_input_simulation_tests().await;
    let emitter = RecordingEmitter::default();
    let started_at = Instant::now();
    replace_text_with_emitter(
        &emitter,
        &rect(),
        "1",
        TextInputTiming {
            focus_delay_ms: 40,
            char_delay_ms: 0,
            settle_delay_ms: 30,
        },
        Arc::new(AtomicBool::new(false)),
    )
    .await
    .unwrap();
    let clicked_at = emitter.clicked_at().expect("未记录输入框点击");
    let first_character_at = emitter.first_character_at().expect("未记录首字符输入");
    assert!(first_character_at.duration_since(clicked_at) >= Duration::from_millis(30));
    assert!(started_at.elapsed() >= Duration::from_millis(60));
}
```

新增结束等待取消测试：输入一个字符后触发 emergency release，断言任务在 `100ms` 内返回 `输入操作已取消`，而非等待完整 `500ms`。

- [ ] **Step 2: 运行测试并确认 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml input_simulation::tests::replace_text_waits_for_focus_and_settle -- --exact
```

Expected: FAIL，`TextInputTiming` 或新 helper 参数尚不存在。

- [ ] **Step 3: 写最小实现**

在 `input_simulation.rs` 增加：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextInputTiming {
    pub focus_delay_ms: u64,
    pub char_delay_ms: u64,
    pub settle_delay_ms: u64,
}
```

将 `replace_text_with_emitter_locked` 的 `char_delay_ms` 攟为 `timing: TextInputTiming`。点击后、逐字符后、全部输入后分别调用已有 `wait_cancellable_input_delay`：

```rust
click_region_center_with_emitter(emitter, region, cancelled, generation)?;
wait_cancellable_input_delay(cancelled, generation, timing.focus_delay_ms)?;

// 保留现有 Ctrl+A、Backspace 和 TrackedModifierGuard 逻辑。

for ch in value.chars() {
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.key(Key::Unicode(ch), Direction::Click)
    })?;
    wait_cancellable_input_delay(cancelled, generation, timing.char_delay_ms)?;
}
wait_cancellable_input_delay(cancelled, generation, timing.settle_delay_ms)
```

`replace_text_at_region_cancellable` 改为接收 `TextInputTiming`。测试 helper 和既有测试使用显式结构体；需要原行为的测试将聚焦与结束延时设为 `0`。

- [ ] **Step 4: 运行输入模拟测试并确认 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml input_simulation::tests
```

Expected: PASS，包含新增聚焦/结束等待及紧急停止测试。

- [ ] **Step 5: 提交共享输入改动**

```powershell
git add src-tauri/src/input_simulation.rs
git commit -m "feat(special-ops): 支持登录输入节流"
```

### Task 2: WeGame 登录启用固定时序

**Files:**
- Modify: `src-tauri/src/special_ops/login_runtime.rs:588-869`
- Test: `src-tauri/src/special_ops/login_runtime.rs`
- Modify: `droid-wiki/features/special-ops.md:23-29`

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn wegame_login_input_timing_is_human_paced() {
    assert_eq!(
        WEGAME_LOGIN_INPUT_TIMING,
        crate::input_simulation::TextInputTiming {
            focus_delay_ms: 800,
            char_delay_ms: 100,
            settle_delay_ms: 500,
        }
    );
}
```

- [ ] **Step 2: 运行测试并确认 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops::login_runtime::tests::wegame_login_input_timing_is_human_paced -- --exact
```

Expected: FAIL，`WEGAME_LOGIN_INPUT_TIMING` 尚不存在。

- [ ] **Step 3: 写最小实现**

```rust
const WEGAME_LOGIN_INPUT_TIMING: crate::input_simulation::TextInputTiming =
    crate::input_simulation::TextInputTiming {
        focus_delay_ms: 800,
        char_delay_ms: 100,
        settle_delay_ms: 500,
    };
```

将 `ProductionLoginDriver::replace_text` 调用改为：

```rust
crate::input_simulation::replace_text_at_region_cancellable(
    region,
    value,
    WEGAME_LOGIN_INPUT_TIMING,
    action_cancelled,
)
.await
```

- [ ] **Step 4: 更新 wiki**

在 `droid-wiki/features/special-ops.md` 登录 runtime 段写明：输入框点击后等待 `800ms`，每字符间隔 `100ms`，输入完成等待 `500ms`；三段等待均响应紧急停止。明确这些固定延时不是成功判定。

- [ ] **Step 5: 运行登录 runtime 测试并确认 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops::login_runtime::tests
```

Expected: PASS。

- [ ] **Step 6: 提交登录与文档改动**

```powershell
git add src-tauri/src/special_ops/login_runtime.rs droid-wiki/features/special-ops.md
git commit -m "fix(special-ops): 放慢 WeGame 登录输入"
```

### Task 3: 全量验证

**Files:**
- Verify: `src-tauri/src/input_simulation.rs`
- Verify: `src-tauri/src/special_ops/login_runtime.rs`
- Verify: `droid-wiki/features/special-ops.md`

- [ ] **Step 1: 刷新 CodeGraph**

```powershell
codegraph sync
```

Expected: 索引刷新成功。

- [ ] **Step 2: 运行统一质量门禁**

```powershell
bun run check
```

Expected: TypeScript、Vitest、coverage、Rust fmt、Clippy、Rust tests 全部通过。

- [ ] **Step 3: 检查最终工作区**

```powershell
git status --short --branch
git log -4 --oneline
```

Expected: 仅保留任务开始前已存在的 `src-tauri/Cargo.toml` 行尾状态；本任务文件无未提交差异。
