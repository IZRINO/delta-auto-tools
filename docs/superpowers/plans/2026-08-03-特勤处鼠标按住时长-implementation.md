# 特勤处鼠标按住时长实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans 逐项 Inline 实施。用户已禁止子代理、worktree 与提交；保留现有未提交改动。

**Goal:** 让特勤处自动化每次鼠标左键点击按住 100ms 后抬起，并在紧急停止期间可靠释放左键。

**Architecture:** 共享输入层新增显式按住点击 API，继续使用全局输入串行锁与 generation 取消机制。该 API 跟踪鼠标左键按下状态，使紧急释放与当前点击都能处理按住期间取消；特勤处调用点切换到该 API，既有即时点击 API 不变。

**Tech Stack:** Rust、Tauri 2、Enigo、Tokio、Cargo test。

---

### Task 1: 为按住点击写失败测试

**Files:**
- Modify: src-tauri/src/input_simulation.rs:13-20,73-132,233-266,892-1200

- [ ] **Step 1: 扩展测试 emitter**

在测试模块的 RecordingEmitter 增加左键事件记录，并补齐 InputEmitter 的新方法：

~~~rust
fn press_left(&self) -> Result<(), String> {
    self.events.lock().unwrap().push("left:press".to_string());
    Ok(())
}

fn release_left(&self) -> Result<(), String> {
    self.events.lock().unwrap().push("left:release".to_string());
    Ok(())
}
~~~

增加 CancelOnLeftPressEmitter：其 move_mouse 记录 move:{x}:{y}；其 press_left 先记录 left:press，再将共享 AtomicBool 设为 true；其 release_left 记录 left:release。其余 trait 方法返回成功。

- [ ] **Step 2: 写入顺序与取消测试**

在测试模块加入：

~~~rust
#[test]
fn held_region_click_moves_then_presses_and_releases_left_button() {
    let emitter = RecordingEmitter::default();
    let cancelled = AtomicBool::new(false);

    click_region_center_held_with_emitter(
        &emitter,
        &rect(),
        0,
        &cancelled,
        input_release_generation(),
    )
    .unwrap();

    assert_eq!(
        emitter.events(),
        vec!["move:140:215", "left:press", "left:release"],
    );
}

#[test]
fn held_click_releases_left_button_when_cancelled_during_hold() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let emitter = CancelOnLeftPressEmitter::new(Arc::clone(&cancelled));

    let result = click_region_center_held_with_emitter(
        &emitter,
        &rect(),
        100,
        &cancelled,
        input_release_generation(),
    );

    assert_eq!(result.unwrap_err(), "输入操作已取消");
    assert_eq!(
        emitter.events(),
        vec!["move:140:215", "left:press", "left:release"],
    );
}
~~~

- [ ] **Step 3: 写入紧急释放测试**

在测试模块加入：

~~~rust
#[test]
fn emergency_release_releases_tracked_left_button() {
    let mut state = InputActionState {
        tracked_keys: Vec::new(),
        left_mouse_pressed: true,
    };
    let emitter = RecordingEmitter::default();

    let errors = release_tracked_injected_inputs_with(&mut state, &emitter);

    assert!(errors.is_empty());
    assert!(!state.left_mouse_pressed);
    assert_eq!(emitter.events(), vec!["left:release"]);
}
~~~

- [ ] **Step 4: 运行失败测试**

Run:

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml input_simulation::tests::held_region_click_moves_then_presses_and_releases_left_button
~~~

Expected: FAIL，缺少 held click helper、鼠标按下/抬起 trait 方法与状态字段。

### Task 2: 实现可取消按住点击与鼠标释放

**Files:**
- Modify: src-tauri/src/input_simulation.rs:23-132,209-266,389-416,513-570

- [ ] **Step 1: 扩展状态与 emitter**

将状态定义改为：

~~~rust
#[derive(Default)]
struct InputActionState {
    tracked_keys: Vec<Key>,
    left_mouse_pressed: bool,
}
~~~

在 InputEmitter 中保留 click_left，新增：

~~~rust
fn press_left(&self) -> Result<(), String>;
fn release_left(&self) -> Result<(), String>;
~~~

EnigoInputEmitter 分别调用 Button::Left + Direction::Press 和 Button::Left + Direction::Release。即时 click_left 保持 Direction::Click，不得变更。

- [ ] **Step 2: 实现追踪按下与释放**

新增：

~~~rust
fn press_left_with_emitter<E: InputEmitter>(
    emitter: &E,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    run_cancellable_input_action(cancelled, generation, |state| {
        emitter.press_left()?;
        state.left_mouse_pressed = true;
        Ok(())
    })
}

fn release_left_if_tracked_with_emitter<E: InputEmitter>(
    state: &mut InputActionState,
    emitter: &E,
) -> Result<(), String> {
    if !state.left_mouse_pressed {
        return Ok(());
    }
    emitter.release_left()?;
    state.left_mouse_pressed = false;
    Ok(())
}
~~~

将键盘释放逻辑与鼠标释放汇总为 release_tracked_injected_inputs_with：先释放鼠标，再释放反向遍历的键盘按键；任一释放失败都保留对应追踪状态，使既有最多 3 次有界重试能够继续尝试。

- [ ] **Step 3: 实现内部按住点击**

新增区域中心版本：

~~~rust
fn click_region_center_held_with_emitter<E: InputEmitter>(
    emitter: &E,
    region: &crate::morse::types::RegionRect,
    hold_ms: u64,
    cancelled: &AtomicBool,
    generation: u64,
) -> Result<(), String> {
    let (center_x, center_y) = region_center(region);
    run_cancellable_input_action(cancelled, generation, |_| {
        emitter.move_mouse(center_x, center_y)
    })?;
    press_left_with_emitter(emitter, cancelled, generation)?;
    let hold_result = wait_cancellable_input_delay(cancelled, generation, hold_ms);
    let release_result = {
        let mut state = lock_input_action_state();
        release_left_if_tracked_with_emitter(&mut state, emitter)
    };
    hold_result?;
    release_result
}
~~~

新增 click_screen_point_held_with_emitter，逻辑完全相同，首步改为 move_mouse(x, y)。保存 hold_result 后才处理结果，保证取消或等待失败时已按下左键仍会被抬起。

- [ ] **Step 4: 暴露异步 API**

在即时 API 旁新增：

~~~rust
pub async fn click_region_center_held_cancellable(
    region: crate::morse::types::RegionRect,
    hold_ms: u64,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        click_region_center_held_with_emitter(&emitter, &region, hold_ms, &cancelled, generation)
    })
    .await
}

pub async fn click_screen_point_held_cancellable(
    x: i32,
    y: i32,
    hold_ms: u64,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let generation = input_release_generation();
    run_serialized_input(move || {
        let emitter = EnigoInputEmitter::new()?;
        click_screen_point_held_with_emitter(&emitter, x, y, hold_ms, &cancelled, generation)
    })
    .await
}
~~~

- [ ] **Step 5: 运行输入层测试**

Run:

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml input_simulation::tests
~~~

Expected: PASS。

### Task 3: 切换特勤处全部鼠标点击调用点

**Files:**
- Modify: src-tauri/src/special_ops/mod.rs:4540-4600,4660-4700
- Modify: src-tauri/src/special_ops/login_runtime.rs:1001-1045,1140-1190
- Modify: src-tauri/src/special_ops/game_navigation.rs:340-380
- Modify: src-tauri/src/special_ops/craft_runtime.rs:200-240

- [ ] **Step 1: 定义唯一时长常量**

在 src-tauri/src/special_ops/mod.rs 模块根部定义：

~~~rust
pub(crate) const MOUSE_CLICK_HOLD_MS: u64 = 100;
~~~

子模块使用 super::MOUSE_CLICK_HOLD_MS。不新增前端设置、serde 字段或持久化迁移。

- [ ] **Step 2: 替换区域点击**

将特勤处每一个：

~~~rust
crate::input_simulation::click_region_center_cancellable(region, Arc::clone(&cancelled)).await?
~~~

替换为：

~~~rust
crate::input_simulation::click_region_center_held_cancellable(
    region,
    super::MOUSE_CLICK_HOLD_MS,
    Arc::clone(&cancelled),
)
.await?
~~~

mod.rs 内使用 MOUSE_CLICK_HOLD_MS，不加 super::。替换范围限定为登录、游戏导航、制作和子弹兑换调用点。

- [ ] **Step 3: 替换账号列表行坐标点击**

在 login_runtime.rs 将：

~~~rust
crate::input_simulation::click_screen_point_cancellable(x, y, Arc::clone(&action_cancelled)).await?
~~~

替换为：

~~~rust
crate::input_simulation::click_screen_point_held_cancellable(
    x,
    y,
    super::MOUSE_CLICK_HOLD_MS,
    Arc::clone(&action_cancelled),
)
.await?
~~~

保留聚焦、动作守卫、运行状态更新和鼠标停车顺序。

- [ ] **Step 4: 运行特勤处测试与调用面检查**

Run:

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml special_ops
rg -n "click_(region_center|screen_point)_cancellable" src-tauri/src/special_ops
~~~

Expected: 测试 PASS；第二条命令无输出。

### Task 4: 同步文档与全量验证

**Files:**
- Modify: droid-wiki/features/special-ops.md
- Modify: docs/superpowers/specs/2026-08-03-特勤处鼠标按住时长-design.md

- [ ] **Step 1: 更新 wiki**

在特勤处运行时输入行为段落写明：特勤处每次鼠标左键点击固定按住 100ms 后抬起；紧急停止同时释放已按下键盘按键和鼠标左键；其他工具即时点击节奏不变。

- [ ] **Step 2: 记录验证结果**

在设计文档末尾追加“实施验证”段，写入实际执行的定向测试、bun run check、git diff --check 和 codegraph sync 结果。失败命令必须保留失败摘要。

- [ ] **Step 3: 执行质量门禁**

Run:

~~~powershell
bun run check
git diff --check
codegraph sync
~~~

Expected: 全部 PASS；若环境阻断，报告命令和原始原因，不能声称完成。

- [ ] **Step 4: 手工验收**

1. 启动开发版，执行任一特勤处试运行。
2. 确认每次点击有可见按住间隔，不再漏触发。
3. 在按住期间触发紧急停止，确认鼠标左键未残留按住状态。
4. 运行一个非特勤处点击功能，确认其节奏未变化。

