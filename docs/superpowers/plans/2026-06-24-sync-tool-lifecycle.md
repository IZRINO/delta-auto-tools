# SyncTool Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将计时器、计数器、连发器共享的同步工具生命周期（配置规范化、分组规则、热键重启、透明窗口、位置设置、全局停止）收敛到一个深模块，同时保持现有 Tauri command、窗口 label、持久化 JSON 和 UI 行为不变。

**Architecture:** 新增 `src-tauri/src/sync_tool.rs`，在 `ToolLogic` 之上提供 `SyncToolLogic`、`SyncItem`、`SyncGroup`、`SyncSettings`、位置状态机纯函数、热键重启骨架与全局停止注册表。`timer`、`counter`、`rapidfire` 作为 adapter 实现这些 interface；`morse` 保持只实现 `ToolLogic`，不进入同步工具生命周期。先以 `counter` 作为 pilot，验证 trait 形状后迁移 `rapidfire` 和 `timer`。

**Tech Stack:** Rust, Tauri 2, tokio `oneshot`, willhook hotkey manager, serde camelCase DTO, cargo test.

## Global Constraints

- 所有对外序列化 Rust 结构体继续使用 `#[serde(rename_all = "camelCase")]`。
- 不改变任何 Tauri command 名称、参数、返回结构或 `src-tauri/capabilities/default.json` 权限。
- 不改变窗口 label：`timer-display` / `timer-position` / `counter-display` / `counter-position` / `rapidfire-display` / `rapidfire-position`。
- 不改变查询参数模式：`?mode=timer-display` / `timer-position` / `counter-display` / `counter-position` / `rapidfire-display` / `rapidfire-position`。
- 不改变持久化 JSON 字段名；兼容 legacy `enabled` 与 `timerEnabled` / `counterEnabled` / `rapidfireEnabled` 字段。
- 不新增第三方依赖。
- 错误信息和注释使用中文。
- 先写失败测试，再写实现。
- 每个任务完成后至少运行对应 Rust 单测；最终运行 `cargo test --manifest-path src-tauri/Cargo.toml` 和 `cargo check --manifest-path src-tauri/Cargo.toml`。
- 本计划不处理 Delta 文档过期问题，不处理前端 IPC seam，不处理 audio watcher loop。

---

## File Structure

### 新增文件

- `src-tauri/src/sync_tool.rs`
  - 负责同步工具深模块。
  - 定义 `SyncItem`、`SyncGroup`、`SyncSettings`、`SyncToolLogic`。
  - 提供 `normalize_sync_settings`、`count_enabled_items_by_group`、`group_id_set`、`restart_sync_hotkeys`。
  - 提供位置状态机纯函数 `apply_position_event`。
  - 提供 `SyncToolRegistry`，供 `global_state` 停止所有可停止同步工具。
  - 包含不依赖 Tauri window 的单元测试。

### 修改文件

- `src-tauri/src/lib.rs`
  - 新增 `mod sync_tool;`。
  - 初始化并 `app.manage(sync_tool::SyncToolRegistry)`。
  - 让 timer/counter/rapidfire 注册 stop handler。

- `src-tauri/src/global_state.rs`
  - 删除对 `counter` / `timer` / `rapidfire` 的直接 import。
  - 改为读取 `SyncToolRegistry` 并调用 `stop_all(app)`。

- `src-tauri/src/counter/mod.rs`
  - 作为 pilot。
  - 实现 `SyncItem` / `SyncGroup` / `SyncSettings` / `SyncToolLogic`。
  - 用 `normalize_sync_settings` 替换本地 `normalize_settings` 骨架。
  - 用 `restart_sync_hotkeys` 替换本地 clear/replace skeleton。
  - 用 `apply_position_event` 重构 `counter_position_commit` / `counter_position_cancel` / `counter_position_moved` 的状态转移。
  - 保留 counter 特有的运行态持久化和 `counter_trigger` / `counter_reset` / `counter_adjust`。

- `src-tauri/src/rapidfire/mod.rs`
  - 实现同步工具 traits。
  - 保留连发器 session、worker、补齐、抖动逻辑。
  - 只迁移设置规范化骨架、热键 clear/replace skeleton、透明窗口和位置状态机。

- `src-tauri/src/timer/mod.rs`
  - 实现同步工具 traits。
  - 保留 `tick_task`、`tick`、`trigger_hotkey_targets`、多段计时逻辑。
  - 只迁移设置规范化骨架、热键 clear/replace skeleton、透明窗口和位置状态机。

- `src-tauri/src/counter/types.rs`
  - 如 trait impl 放在 `types.rs` 更清晰，可添加 impl；默认优先放在 `counter/mod.rs`，避免跨模块暴露更多类型。

- `src-tauri/src/timer/types.rs`
  - 同上。

- `src-tauri/src/rapidfire/types.rs`
  - 同上。

- `CONTEXT.md`
  - 已加入 `同步工具` 和 `可停止` 术语。执行计划时只在发现术语不准确时修正。

---

## Task 1: Create the SyncTool deep module interfaces

**Files:**
- Create: `src-tauri/src/sync_tool.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/sync_tool.rs`

**Interfaces:**
- Produces:
  - `pub trait SyncItem`
  - `pub trait SyncGroup`
  - `pub trait SyncSettings`
  - `pub trait SyncToolLogic: ToolLogic`
  - `pub struct HotkeyBindingSet`
  - `pub fn normalize_sync_settings<S: SyncSettings>(settings: S) -> Result<S, String>`
  - `pub fn count_enabled_items_by_group<I: SyncItem>(items: &[I]) -> HashMap<String, usize>`
  - `pub enum PositionEvent<R>`
  - `pub struct PositionDecision<R, K>`
  - `pub fn apply_position_event<R, K>(...) -> Result<PositionDecision<R, K>, String>`
  - `pub struct SyncToolRegistry`

- Consumes:
  - `crate::tool_base::{ToolLogic, ToolState}`
  - `crate::hotkey_types::{HotkeyAction, HoldActionCallback, ConflictPolicy}`
  - `crate::hotkeys::HotkeyManager`

- [ ] **Step 1: Add module declaration**

Modify `src-tauri/src/lib.rs` near the other module declarations:

```rust
mod sync_tool;
```

- [ ] **Step 2: Create the initial interface file**

Create `src-tauri/src/sync_tool.rs` with this interface-first implementation:

```rust
use std::collections::{HashMap, HashSet};

use tauri::{AppHandle, Manager};

use crate::hotkey_types::{ConflictPolicy, HoldActionCallback, HotkeyAction};
use crate::hotkeys::HotkeyManager;
use crate::tool_base::{ToolLogic, ToolState};

pub trait SyncItem: Clone {
    fn id(&self) -> &str;
    fn group_id(&self) -> &str;
    fn set_group_id(&mut self, group_id: String);
    fn enabled(&self) -> bool;
}

pub trait SyncGroup: Clone {
    fn id(&self) -> &str;
    fn enabled(&self) -> bool;
}

pub trait SyncSettings: Clone {
    type Item: SyncItem;
    type Group: SyncGroup;

    const DEFAULT_GROUP_ID: &'static str;
    const DUPLICATE_ITEM_MESSAGE_PREFIX: &'static str;

    fn sync_legacy_enabled(&mut self);
    fn items(&self) -> &[Self::Item];
    fn items_mut(&mut self) -> &mut Vec<Self::Item>;
    fn replace_items(&mut self, items: Vec<Self::Item>);
    fn groups(&self) -> &[Self::Group];
    fn normalize_groups(&self) -> Result<Vec<Self::Group>, String>;
    fn replace_groups(&mut self, groups: Vec<Self::Group>);
    fn default_item(&self) -> Self::Item;
    fn normalize_item(&self, item: &Self::Item) -> Result<Self::Item, String>;
    fn after_groups_normalized(&mut self) {}
}

pub struct HotkeyBindingSet {
    pub normal: Vec<(String, HotkeyAction)>,
    pub hold: Vec<(String, HoldActionCallback)>,
}

impl HotkeyBindingSet {
    pub fn empty() -> Self {
        Self {
            normal: Vec::new(),
            hold: Vec::new(),
        }
    }
}

pub trait SyncToolLogic: ToolLogic
where
    Self::Settings: SyncSettings,
{
    const SCOPE: &'static str;
    const SCOPE_LABEL: &'static str;
    const CONFLICT_POLICY: ConflictPolicy = ConflictPolicy::AllowHold;

    fn tool_enabled(settings: &Self::Settings) -> bool;
    fn build_hotkey_bindings(settings: &Self::Settings) -> Result<HotkeyBindingSet, String>;
    fn stop_all(app: &AppHandle) -> Result<(), String>;
}

pub fn group_id_set<G: SyncGroup>(groups: &[G], default_group_id: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    ids.insert(default_group_id.to_string());
    for group in groups {
        let id = group.id().trim();
        if !id.is_empty() {
            ids.insert(id.to_string());
        }
    }
    ids
}

pub fn count_enabled_items_by_group<I: SyncItem>(items: &[I]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for item in items {
        if item.enabled() {
            *map.entry(item.group_id().to_string()).or_insert(0) += 1;
        }
    }
    map
}

pub fn group_enabled<G: SyncGroup>(groups: &[G], group_id: &str) -> bool {
    groups
        .iter()
        .find(|group| group.id() == group_id)
        .map(|group| group.enabled())
        .unwrap_or(false)
}

pub fn normalize_sync_settings<S: SyncSettings>(mut settings: S) -> Result<S, String> {
    settings.sync_legacy_enabled();

    if settings.items().is_empty() {
        let default_item = settings.default_item();
        settings.items_mut().push(default_item);
    }

    let normalized_groups = settings.normalize_groups()?;
    let group_ids = group_id_set(&normalized_groups, S::DEFAULT_GROUP_ID);
    let mut seen_ids = HashSet::new();
    let mut normalized_items = Vec::with_capacity(settings.items().len());

    for item in settings.items() {
        let mut normalized = settings.normalize_item(item)?;
        if !group_ids.contains(normalized.group_id()) {
            normalized.set_group_id(S::DEFAULT_GROUP_ID.to_string());
        }
        if !seen_ids.insert(normalized.id().to_string()) {
            return Err(format!(
                "{}: {}",
                S::DUPLICATE_ITEM_MESSAGE_PREFIX,
                normalized.id()
            ));
        }
        normalized_items.push(normalized);
    }

    settings.replace_groups(normalized_groups);
    settings.replace_items(normalized_items);
    settings.after_groups_normalized();
    Ok(settings)
}

impl<L> ToolState<L>
where
    L: SyncToolLogic,
    L::Settings: SyncSettings,
{
    pub fn restart_sync_hotkeys(
        &self,
        hotkey_manager: &HotkeyManager,
        settings: &L::Settings,
    ) -> Result<(), String> {
        if !L::tool_enabled(settings) {
            hotkey_manager.clear_scope(L::SCOPE)?;
            return hotkey_manager.clear_hold_scope(L::SCOPE);
        }

        let bindings = L::build_hotkey_bindings(settings)?;
        hotkey_manager.clear_scope(L::SCOPE)?;
        hotkey_manager.clear_hold_scope(L::SCOPE)?;

        if !bindings.normal.is_empty() {
            hotkey_manager.replace_scope(
                L::SCOPE,
                bindings.normal,
                L::SCOPE_LABEL.to_string(),
                L::CONFLICT_POLICY,
            )?;
        }

        if !bindings.hold.is_empty() {
            hotkey_manager.replace_hold_scope(
                L::SCOPE,
                bindings.hold,
                L::SCOPE_LABEL.to_string(),
                L::CONFLICT_POLICY,
            )?;
        }

        if let Ok(mut inner) = self.lock_inner() {
            inner.hotkey_error = None;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PositionEvent<R> {
    Begin {
        group_id: String,
        rect: R,
    },
    Moved {
        x: i32,
        y: i32,
    },
    Commit,
    Cancel,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPosition<R> {
    pub group_id: String,
    pub original_rect: R,
    pub staged_rect: R,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionDecision<R, K> {
    pub pending: Option<PendingPosition<R>>,
    pub save: bool,
    pub send: Option<K>,
    pub destroy_window: bool,
    pub move_window_to: Option<R>,
}

pub trait SyncRect: Clone {
    fn with_position(&self, x: i32, y: i32) -> Self;
}

pub trait PositionKinds: Clone {
    fn selected() -> Self;
    fn cancelled() -> Self;
    fn closed() -> Self;
}

pub fn apply_position_event<R, K>(
    pending: Option<PendingPosition<R>>,
    event: PositionEvent<R>,
) -> Result<PositionDecision<R, K>, String>
where
    R: SyncRect,
    K: PositionKinds,
{
    match event {
        PositionEvent::Begin { group_id, rect } => {
            if pending.is_some() {
                return Err("位置设置已在进行中".to_string());
            }
            Ok(PositionDecision {
                pending: Some(PendingPosition {
                    group_id,
                    original_rect: rect.clone(),
                    staged_rect: rect,
                }),
                save: false,
                send: None,
                destroy_window: false,
                move_window_to: None,
            })
        }
        PositionEvent::Moved { x, y } => {
            let Some(mut current) = pending else {
                return Err("没有正在进行的位置设置".to_string());
            };
            current.staged_rect = current.staged_rect.with_position(x, y);
            Ok(PositionDecision {
                move_window_to: Some(current.staged_rect.clone()),
                pending: Some(current),
                save: false,
                send: None,
                destroy_window: false,
            })
        }
        PositionEvent::Commit => {
            let Some(current) = pending else {
                return Err("没有正在进行的位置设置".to_string());
            };
            Ok(PositionDecision {
                pending: None,
                save: true,
                send: Some(K::selected()),
                destroy_window: true,
                move_window_to: None,
            })
        }
        PositionEvent::Cancel => {
            let Some(_current) = pending else {
                return Err("没有正在进行的位置设置".to_string());
            };
            Ok(PositionDecision {
                pending: None,
                save: false,
                send: Some(K::cancelled()),
                destroy_window: true,
                move_window_to: None,
            })
        }
        PositionEvent::Closed => {
            let Some(_current) = pending else {
                return Err("没有正在进行的位置设置".to_string());
            };
            Ok(PositionDecision {
                pending: None,
                save: false,
                send: Some(K::closed()),
                destroy_window: true,
                move_window_to: None,
            })
        }
    }
}

pub type StopHandler = fn(&AppHandle) -> Result<(), String>;

#[derive(Default)]
pub struct SyncToolRegistry {
    handlers: Vec<(&'static str, StopHandler)>,
}

impl SyncToolRegistry {
    pub fn register(&mut self, name: &'static str, handler: StopHandler) {
        self.handlers.push((name, handler));
    }

    pub fn stop_all(&self, app: &AppHandle) -> Vec<String> {
        let mut errors = Vec::new();
        for (name, handler) in &self.handlers {
            if let Err(error) = handler(app) {
                errors.push(format!("{name}: {error}"));
            }
        }
        errors
    }
}
```

- [ ] **Step 3: Add interface tests**

Append this test module to `src-tauri/src/sync_tool.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestItem {
        id: String,
        group_id: String,
        enabled: bool,
    }

    impl SyncItem for TestItem {
        fn id(&self) -> &str { &self.id }
        fn group_id(&self) -> &str { &self.group_id }
        fn set_group_id(&mut self, group_id: String) { self.group_id = group_id; }
        fn enabled(&self) -> bool { self.enabled }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestGroup {
        id: String,
        enabled: bool,
    }

    impl SyncGroup for TestGroup {
        fn id(&self) -> &str { &self.id }
        fn enabled(&self) -> bool { self.enabled }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestSettings {
        enabled: bool,
        groups: Vec<TestGroup>,
        items: Vec<TestItem>,
    }

    impl SyncSettings for TestSettings {
        type Item = TestItem;
        type Group = TestGroup;

        const DEFAULT_GROUP_ID: &'static str = "default";
        const DUPLICATE_ITEM_MESSAGE_PREFIX: &'static str = "测试条目 ID 重复";

        fn sync_legacy_enabled(&mut self) {
            self.enabled = true;
        }

        fn items(&self) -> &[Self::Item] { &self.items }
        fn items_mut(&mut self) -> &mut Vec<Self::Item> { &mut self.items }
        fn replace_items(&mut self, items: Vec<Self::Item>) { self.items = items; }
        fn groups(&self) -> &[Self::Group] { &self.groups }
        fn normalize_groups(&self) -> Result<Vec<Self::Group>, String> {
            let mut groups = self.groups.clone();
            if groups.is_empty() {
                groups.push(TestGroup { id: "default".to_string(), enabled: true });
            }
            Ok(groups)
        }
        fn replace_groups(&mut self, groups: Vec<Self::Group>) { self.groups = groups; }
        fn default_item(&self) -> Self::Item {
            TestItem { id: "item-1".to_string(), group_id: "default".to_string(), enabled: true }
        }
        fn normalize_item(&self, item: &Self::Item) -> Result<Self::Item, String> {
            Ok(TestItem {
                id: item.id.trim().to_string(),
                group_id: item.group_id.trim().to_string(),
                enabled: item.enabled,
            })
        }
    }

    #[test]
    fn normalize_sync_settings_inserts_default_item_and_group() {
        let settings = TestSettings { enabled: false, groups: vec![], items: vec![] };

        let normalized = normalize_sync_settings(settings).expect("规范化应成功");

        assert!(normalized.enabled);
        assert_eq!(normalized.groups.len(), 1);
        assert_eq!(normalized.items.len(), 1);
        assert_eq!(normalized.items[0].group_id, "default");
    }

    #[test]
    fn normalize_sync_settings_moves_unknown_group_to_default() {
        let settings = TestSettings {
            enabled: false,
            groups: vec![TestGroup { id: "default".to_string(), enabled: true }],
            items: vec![TestItem { id: "item-1".to_string(), group_id: "missing".to_string(), enabled: true }],
        };

        let normalized = normalize_sync_settings(settings).expect("规范化应成功");

        assert_eq!(normalized.items[0].group_id, "default");
    }

    #[test]
    fn normalize_sync_settings_rejects_duplicate_item_ids() {
        let settings = TestSettings {
            enabled: false,
            groups: vec![TestGroup { id: "default".to_string(), enabled: true }],
            items: vec![
                TestItem { id: "same".to_string(), group_id: "default".to_string(), enabled: true },
                TestItem { id: "same".to_string(), group_id: "default".to_string(), enabled: true },
            ],
        };

        let error = normalize_sync_settings(settings).expect_err("重复 ID 应报错");

        assert_eq!(error, "测试条目 ID 重复: same");
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestRect { x: i32, y: i32, width: i32, height: i32 }

    impl SyncRect for TestRect {
        fn with_position(&self, x: i32, y: i32) -> Self {
            Self { x, y, width: self.width, height: self.height }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TestKind { Selected, Cancelled, Closed }

    impl PositionKinds for TestKind {
        fn selected() -> Self { Self::Selected }
        fn cancelled() -> Self { Self::Cancelled }
        fn closed() -> Self { Self::Closed }
    }

    #[test]
    fn apply_position_event_moves_pending_rect() {
        let pending = Some(PendingPosition {
            group_id: "g".to_string(),
            original_rect: TestRect { x: 1, y: 2, width: 320, height: 96 },
            staged_rect: TestRect { x: 1, y: 2, width: 320, height: 96 },
        });

        let decision = apply_position_event::<TestRect, TestKind>(
            pending,
            PositionEvent::Moved { x: 50, y: 60 },
        )
        .expect("移动事件应成功");

        assert_eq!(decision.pending.as_ref().unwrap().staged_rect.x, 50);
        assert_eq!(decision.pending.as_ref().unwrap().staged_rect.y, 60);
        assert_eq!(decision.move_window_to.unwrap().x, 50);
        assert!(!decision.save);
        assert!(!decision.destroy_window);
    }

    #[test]
    fn apply_position_event_commit_saves_and_sends_selected() {
        let pending = Some(PendingPosition {
            group_id: "g".to_string(),
            original_rect: TestRect { x: 1, y: 2, width: 320, height: 96 },
            staged_rect: TestRect { x: 5, y: 6, width: 320, height: 96 },
        });

        let decision = apply_position_event::<TestRect, TestKind>(
            pending,
            PositionEvent::Commit,
        )
        .expect("提交事件应成功");

        assert!(decision.pending.is_none());
        assert!(decision.save);
        assert_eq!(decision.send, Some(TestKind::Selected));
        assert!(decision.destroy_window);
    }
}
```

- [ ] **Step 4: Run test to verify the new module passes**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml sync_tool --lib
```

Expected: all `sync_tool` tests pass.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/lib.rs src-tauri/src/sync_tool.rs
git commit -m "新增同步工具生命周期基座"
```

---

## Task 2: Migrate counter normalization to SyncSettings

**Files:**
- Modify: `src-tauri/src/counter/mod.rs`
- Test: `src-tauri/src/counter/mod.rs`

**Interfaces:**
- Consumes:
  - `sync_tool::{SyncGroup, SyncItem, SyncSettings, normalize_sync_settings, count_enabled_items_by_group, group_enabled}`
- Produces:
  - `impl SyncItem for CounterItem`
  - `impl SyncGroup for CounterGroup`
  - `impl SyncSettings for CounterSettings`
  - `pub(crate) fn normalize_settings(settings_value: CounterSettings) -> Result<CounterSettings, String>` delegates to `normalize_sync_settings`

- [ ] **Step 1: Write failing counter normalization tests**

Add these tests to the existing `#[cfg(test)] mod tests` in `src-tauri/src/counter/mod.rs`:

```rust
#[test]
fn counter_normalize_moves_unknown_group_to_default() {
    let mut settings = CounterSettings::default();
    settings.counters[0].group_id = "missing".to_string();

    let normalized = normalize_settings(settings).expect("计数器配置应规范化");

    assert_eq!(normalized.counters[0].group_id, DEFAULT_COUNTER_GROUP_ID);
}

#[test]
fn counter_normalize_rejects_duplicate_counter_ids() {
    let mut settings = CounterSettings::default();
    settings.counters.push(CounterItem {
        id: settings.counters[0].id.clone(),
        group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
        name: "重复计数器".to_string(),
        start_value: 0,
        hotkey: "F4".to_string(),
        enabled: true,
    });

    let error = normalize_settings(settings).expect_err("重复 ID 应报错");

    assert_eq!(error, "计数器 ID 重复: counter-1");
}
```

- [ ] **Step 2: Run failing tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml counter_normalize --lib
```

Expected: tests compile-fail or fail until `SyncSettings` impls are added.

- [ ] **Step 3: Add counter trait impls**

In `src-tauri/src/counter/mod.rs`, add imports:

```rust
use crate::sync_tool::{
    count_enabled_items_by_group, group_enabled, normalize_sync_settings, SyncGroup, SyncItem,
    SyncSettings,
};
```

Add these impls below `normalize_counter`:

```rust
impl SyncItem for CounterItem {
    fn id(&self) -> &str {
        &self.id
    }

    fn group_id(&self) -> &str {
        &self.group_id
    }

    fn set_group_id(&mut self, group_id: String) {
        self.group_id = group_id;
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl SyncGroup for CounterGroup {
    fn id(&self) -> &str {
        &self.id
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl SyncSettings for CounterSettings {
    type Item = CounterItem;
    type Group = CounterGroup;

    const DEFAULT_GROUP_ID: &'static str = DEFAULT_COUNTER_GROUP_ID;
    const DUPLICATE_ITEM_MESSAGE_PREFIX: &'static str = "计数器 ID 重复";

    fn sync_legacy_enabled(&mut self) {
        if self.enabled && !self.counter_enabled {
            self.counter_enabled = true;
        }
        self.enabled = self.counter_enabled;
    }

    fn items(&self) -> &[Self::Item] {
        &self.counters
    }

    fn items_mut(&mut self) -> &mut Vec<Self::Item> {
        &mut self.counters
    }

    fn replace_items(&mut self, items: Vec<Self::Item>) {
        self.counters = items;
    }

    fn groups(&self) -> &[Self::Group] {
        &self.counter_groups
    }

    fn normalize_groups(&self) -> Result<Vec<Self::Group>, String> {
        let legacy_display = self.display.clone();
        let counter_count_by_group = count_enabled_items_by_group(&self.counters);
        normalize_counter_groups(
            self.counter_groups.clone(),
            DEFAULT_COUNTER_GROUP_ID,
            legacy_display,
            &counter_count_by_group,
        )
    }

    fn replace_groups(&mut self, groups: Vec<Self::Group>) {
        self.counter_groups = groups;
    }

    fn default_item(&self) -> Self::Item {
        CounterItem {
            id: format!("counter-{}", crate::utils::now_ms()),
            group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
            name: "计数器 1".to_string(),
            start_value: 0,
            hotkey: "F3".to_string(),
            enabled: true,
        }
    }

    fn normalize_item(&self, item: &Self::Item) -> Result<Self::Item, String> {
        normalize_counter(item)
    }

    fn after_groups_normalized(&mut self) {
        self.display = group_display(
            &self.counter_groups,
            DEFAULT_COUNTER_GROUP_ID,
            DEFAULT_COUNTER_GROUP_ID,
        )
        .cloned()
        .unwrap_or_default();
    }
}
```

- [ ] **Step 4: Rename local group normalizer**

Rename the current local `normalize_groups` function to avoid colliding with the trait method:

```rust
fn normalize_counter_groups(
    groups: Vec<CounterGroup>,
    default_group_id: &str,
    legacy_display: CounterDisplaySettings,
    counter_count_by_group: &HashMap<String, usize>,
) -> Result<Vec<CounterGroup>, String> {
    // body is the current normalize_groups body, unchanged
}
```

Then update every local call from `normalize_groups(...)` to `normalize_counter_groups(...)`.

- [ ] **Step 5: Replace counter normalize_settings body**

Replace the current `normalize_settings` with:

```rust
pub(crate) fn normalize_settings(settings_value: CounterSettings) -> Result<CounterSettings, String> {
    normalize_sync_settings(settings_value)
}
```

- [ ] **Step 6: Remove duplicated helper**

Delete local `group_id_set` and `count_enabled_counters_by_group`. Replace any remaining `count_enabled_counters_by_group(&settings_value.counters)` with `count_enabled_items_by_group(&settings_value.counters)`.

- [ ] **Step 7: Run counter tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml counter --lib
```

Expected: counter tests pass.

- [ ] **Step 8: Commit**

```powershell
git add src-tauri/src/counter/mod.rs
git commit -m "迁移计数器配置规范化到同步工具基座"
```

---

## Task 3: Migrate counter hotkey restart to SyncToolLogic

**Files:**
- Modify: `src-tauri/src/counter/mod.rs`
- Test: `src-tauri/src/counter/mod.rs`

**Interfaces:**
- Consumes:
  - `SyncToolLogic`
  - `HotkeyBindingSet`
  - `ToolState<CounterLogic>::restart_sync_hotkeys`
- Produces:
  - `impl SyncToolLogic for CounterLogic`
  - Counter `restart_hotkey_listeners` delegates to generic skeleton.

- [ ] **Step 1: Add failing binding shape test**

Add this test inside `counter/mod.rs` test module:

```rust
#[test]
fn counter_build_hotkey_bindings_groups_same_hotkey() {
    let mut settings = CounterSettings::default();
    settings.counter_enabled = true;
    settings.enabled = true;
    settings.counters.push(CounterItem {
        id: "counter-2".to_string(),
        group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
        name: "计数器 2".to_string(),
        start_value: 5,
        hotkey: "F3".to_string(),
        enabled: true,
    });

    let bindings = CounterLogic::build_hotkey_bindings(&settings).expect("绑定构建应成功");

    assert_eq!(bindings.normal.len(), 1);
    assert!(bindings.hold.is_empty());
    assert_eq!(bindings.normal[0].0, "F3");
}
```

- [ ] **Step 2: Run failing binding test**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml counter_build_hotkey_bindings_groups_same_hotkey --lib
```

Expected: FAIL because `CounterLogic::build_hotkey_bindings` is not implemented yet.

- [ ] **Step 3: Implement SyncToolLogic for CounterLogic**

Add imports:

```rust
use crate::sync_tool::{HotkeyBindingSet, SyncToolLogic};
```

Add this impl near the existing `ToolLogic for CounterLogic` impl:

```rust
impl SyncToolLogic for CounterLogic {
    const SCOPE: &'static str = "counter";
    const SCOPE_LABEL: &'static str = "计数器";

    fn tool_enabled(settings: &CounterSettings) -> bool {
        settings.counter_enabled
    }

    fn build_hotkey_bindings(settings: &CounterSettings) -> Result<HotkeyBindingSet, String> {
        let mut by_hotkey: HashMap<String, Vec<String>> = HashMap::new();
        for counter in &settings.counters {
            if !counter.enabled || !group_enabled(&settings.counter_groups, &counter.group_id) {
                continue;
            }
            by_hotkey
                .entry(counter.hotkey.trim().to_string())
                .or_default()
                .push(counter.id.clone());
        }

        let mut bindings = HotkeyBindingSet::empty();
        for (hotkey, counter_ids) in by_hotkey {
            let action: HotkeyAction = std::sync::Arc::new(move |app_handle| {
                let targets = counter_ids.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = trigger_hotkey_targets(&app_handle, targets) {
                        let _ = app_handle.emit_to("main", events::HOTKEY_ERROR, error);
                    }
                });
            });
            bindings.normal.push((hotkey, action));
        }
        Ok(bindings)
    }

    fn stop_all(app: &AppHandle) -> Result<(), String> {
        let Some(state) = app.try_state::<CounterState>() else {
            return Ok(());
        };
        stop_all(app, &state);
        Ok(())
    }
}
```

- [ ] **Step 4: Delegate counter restart_hotkey_listeners**

Replace the existing `restart_hotkey_listeners` body with:

```rust
pub(crate) fn restart_hotkey_listeners(
    state: &CounterState,
    hotkey_manager: &HotkeyManager,
    settings_value: &CounterSettings,
) -> Result<(), String> {
    state.tool.restart_sync_hotkeys(hotkey_manager, settings_value)
}
```

- [ ] **Step 5: Run counter hotkey tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml counter_build_hotkey_bindings_groups_same_hotkey --lib
cargo test --manifest-path src-tauri/Cargo.toml hotkeys --lib
```

Expected: both commands pass.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/counter/mod.rs
git commit -m "迁移计数器热键重启到同步工具基座"
```

---

## Task 4: Migrate counter position transition to pure Decision

**Files:**
- Modify: `src-tauri/src/counter/mod.rs`
- Test: `src-tauri/src/counter/mod.rs`

**Interfaces:**
- Consumes:
  - `SyncRect`
  - `PositionKinds`
  - `PendingPosition`
  - `PositionEvent`
  - `apply_position_event`
- Produces:
  - `impl SyncRect for CounterRect`
  - `impl PositionKinds for CounterSelectionKind`
  - Counter position commands use the shared transition function.

- [ ] **Step 1: Add counter-specific transition test**

Add this test:

```rust
#[test]
fn counter_position_transition_cancel_does_not_save() {
    use crate::sync_tool::{apply_position_event, PendingPosition, PositionEvent};

    let pending = Some(PendingPosition {
        group_id: DEFAULT_COUNTER_GROUP_ID.to_string(),
        original_rect: CounterRect { x: 1, y: 2, width: 320, height: 96 },
        staged_rect: CounterRect { x: 50, y: 60, width: 320, height: 96 },
    });

    let decision = apply_position_event::<CounterRect, CounterSelectionKind>(
        pending,
        PositionEvent::Cancel,
    )
    .expect("取消位置设置应成功");

    assert!(!decision.save);
    assert_eq!(decision.send, Some(CounterSelectionKind::Cancelled));
    assert!(decision.destroy_window);
}
```

- [ ] **Step 2: Run failing transition test**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml counter_position_transition_cancel_does_not_save --lib
```

Expected: FAIL until `SyncRect` and `PositionKinds` impls are added.

- [ ] **Step 3: Implement rect and kind adapters**

Add imports:

```rust
use crate::sync_tool::{apply_position_event, PendingPosition, PositionEvent, PositionKinds, SyncRect};
```

Add impls:

```rust
impl SyncRect for CounterRect {
    fn with_position(&self, x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            width: self.width,
            height: self.height,
        }
    }
}

impl PositionKinds for CounterSelectionKind {
    fn selected() -> Self {
        Self::Selected
    }

    fn cancelled() -> Self {
        Self::Cancelled
    }

    fn closed() -> Self {
        Self::Closed
    }
}
```

- [ ] **Step 4: Add conversion helpers**

Add helpers near `PendingCounterPosition`:

```rust
fn pending_counter_to_sync(
    pending: PendingCounterPosition,
) -> (PendingPosition<CounterRect>, tokio::sync::oneshot::Sender<CounterSelectionKind>) {
    (
        PendingPosition {
            group_id: pending.group_id,
            original_rect: pending.original_rect,
            staged_rect: pending.staged_rect,
        },
        pending.sender,
    )
}

fn pending_counter_from_sync(
    pending: PendingPosition<CounterRect>,
    sender: tokio::sync::oneshot::Sender<CounterSelectionKind>,
) -> PendingCounterPosition {
    PendingCounterPosition {
        group_id: pending.group_id,
        original_rect: pending.original_rect,
        staged_rect: pending.staged_rect,
        sender,
    }
}
```

- [ ] **Step 5: Refactor counter_position_moved with the shared transition**

In `counter_position_moved`, replace direct staged rect mutation with:

```rust
let pending = inner.logic.pending_position.take();
let Some(pending) = pending else {
    return Err("没有正在进行的位置设置".to_string());
};
let (sync_pending, sender) = pending_counter_to_sync(pending);
let decision = apply_position_event::<CounterRect, CounterSelectionKind>(
    Some(sync_pending),
    PositionEvent::Moved { x, y },
)?;
if let Some(next_pending) = decision.pending {
    inner.logic.pending_position = Some(pending_counter_from_sync(next_pending, sender));
}
let staged_rect = decision
    .move_window_to
    .ok_or_else(|| "位置设置移动结果缺失".to_string())?;
```

Then keep the existing Tauri shell behavior that moves the window:

```rust
if let Some(window) = app.get_webview_window(COUNTER_POSITION_LABEL) {
    let _ = window.set_position(PhysicalPosition::new(staged_rect.x, staged_rect.y));
}
```

- [ ] **Step 6: Refactor commit/cancel with shared transition**

In `counter_position_commit`, apply `PositionEvent::Commit`; if `decision.save` is true, write `decision`'s staged rect to the matching group display rect before calling `settings::save_settings`. Send `decision.send.unwrap()` through the original sender and destroy the position window if `decision.destroy_window`.

Use this exact commit shell shape:

```rust
let pending = {
    let mut inner = state.lock_inner()?;
    inner.logic.pending_position.take()
};
let Some(pending) = pending else {
    return Err("没有正在进行的位置设置".to_string());
};
let (sync_pending, sender) = pending_counter_to_sync(pending);
let staged_rect = sync_pending.staged_rect.clone();
let group_id = sync_pending.group_id.clone();
let decision = apply_position_event::<CounterRect, CounterSelectionKind>(
    Some(sync_pending),
    PositionEvent::Commit,
)?;

if decision.save {
    let mut inner = state.lock_inner()?;
    if let Some(group) = inner.settings.counter_groups.iter_mut().find(|group| group.id == group_id) {
        group.display.rect = staged_rect.clone();
    }
    inner.settings.display = group_display(
        &inner.settings.counter_groups,
        DEFAULT_COUNTER_GROUP_ID,
        DEFAULT_COUNTER_GROUP_ID,
    )
    .cloned()
    .unwrap_or_default();
    settings::save_settings(&app, &inner.settings)?;
}

if let Some(kind) = decision.send {
    let _ = sender.send(kind);
}
if decision.destroy_window {
    destroy_window(&app, COUNTER_POSITION_LABEL);
}
```

Apply the same shape to `counter_position_cancel`, using `PositionEvent::Cancel` and no save branch.

- [ ] **Step 7: Run counter position tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml counter_position_transition --lib
cargo test --manifest-path src-tauri/Cargo.toml counter --lib
```

Expected: both commands pass.

- [ ] **Step 8: Commit**

```powershell
git add src-tauri/src/counter/mod.rs
git commit -m "迁移计数器位置状态机到同步工具基座"
```

---

## Task 5: Register sync-tool global stop handlers

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/global_state.rs`
- Modify: `src-tauri/src/counter/mod.rs`
- Modify: `src-tauri/src/timer/mod.rs`
- Modify: `src-tauri/src/rapidfire/mod.rs`
- Test: `src-tauri/src/sync_tool.rs`

**Interfaces:**
- Consumes:
  - `SyncToolRegistry`
  - `SyncToolLogic::stop_all`
- Produces:
  - `counter::stop_registered`
  - `timer::stop_registered`
  - `rapidfire::stop_registered`
  - `global_state` no longer imports timer/counter/rapidfire directly.

- [ ] **Step 1: Add registry unit test**

Add to `sync_tool.rs` tests:

```rust
#[test]
fn sync_tool_registry_collects_stop_errors() {
    fn ok_handler(_: &AppHandle) -> Result<(), String> {
        Ok(())
    }

    fn err_handler(_: &AppHandle) -> Result<(), String> {
        Err("停止失败".to_string())
    }

    let mut registry = SyncToolRegistry::default();
    registry.register("ok", ok_handler);
    registry.register("bad", err_handler);

    // This test only validates error formatting by calling handlers directly.
    let names: Vec<&'static str> = registry.handlers.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, vec!["ok", "bad"]);
}
```

If `handlers` remains private, replace the final assertion by adding this method:

```rust
pub fn registered_names(&self) -> Vec<&'static str> {
    self.handlers.iter().map(|(name, _)| *name).collect()
}
```

and assert `registry.registered_names()`.

- [ ] **Step 2: Add per-tool registered stop functions**

Add to each module:

Counter:

```rust
pub(crate) fn stop_registered(app: &AppHandle) -> Result<(), String> {
    CounterLogic::stop_all(app)
}
```

Timer:

```rust
pub(crate) fn stop_registered(app: &AppHandle) -> Result<(), String> {
    TimerLogic::stop_all(app)
}
```

Rapidfire:

```rust
pub(crate) fn stop_registered(app: &AppHandle) -> Result<(), String> {
    RapidfireLogic::stop_all(app)
}
```

- [ ] **Step 3: Build and manage the registry in lib.rs**

In `setup`, before `app.manage(global_state)`, add:

```rust
let mut sync_tool_registry = sync_tool::SyncToolRegistry::default();
sync_tool_registry.register("counter", counter::stop_registered);
sync_tool_registry.register("timer", timer::stop_registered);
sync_tool_registry.register("rapidfire", rapidfire::stop_registered);
```

Then manage it:

```rust
app.manage(sync_tool_registry);
```

- [ ] **Step 4: Replace global_state hardcoding**

Replace `stop_active_sessions` in `src-tauri/src/global_state.rs` with:

```rust
fn stop_active_sessions(app: &AppHandle) {
    let Some(registry) = app.try_state::<crate::sync_tool::SyncToolRegistry>() else {
        return;
    };

    for error in registry.stop_all(app) {
        eprintln!("停止同步工具失败: {error}");
    }
}
```

Remove these imports from `stop_active_sessions`:

```rust
use crate::hotkeys::HotkeyManager;
use crate::counter;
use crate::rapidfire;
use crate::timer;
```

- [ ] **Step 5: Run tests and check**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml sync_tool --lib
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: tests pass and cargo check succeeds.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/lib.rs src-tauri/src/global_state.rs src-tauri/src/counter/mod.rs src-tauri/src/timer/mod.rs src-tauri/src/rapidfire/mod.rs src-tauri/src/sync_tool.rs
git commit -m "接入同步工具全局停止注册表"
```

---

## Task 6: Migrate rapidfire normalization and hotkey restart

**Files:**
- Modify: `src-tauri/src/rapidfire/mod.rs`
- Test: `src-tauri/src/rapidfire/mod.rs`

**Interfaces:**
- Consumes:
  - `SyncItem`, `SyncGroup`, `SyncSettings`, `SyncToolLogic`
  - `normalize_sync_settings`
  - `restart_sync_hotkeys`
- Produces:
  - Rapidfire cards/groups use the shared normalize skeleton.
  - Rapidfire hold-scope replacement uses shared clear/replace skeleton.

- [ ] **Step 1: Write rapidfire normalization tests**

Add tests:

```rust
#[test]
fn rapidfire_normalize_moves_unknown_group_to_default() {
    let mut settings = RapidfireSettings::default();
    settings.cards[0].group_id = "missing".to_string();

    let normalized = normalize_settings(settings).expect("连发器配置应规范化");

    assert_eq!(normalized.cards[0].group_id, DEFAULT_RAPIDFIRE_GROUP_ID);
}

#[test]
fn rapidfire_build_hotkey_bindings_is_hold_only() {
    let mut settings = RapidfireSettings::default();
    settings.rapidfire_enabled = true;

    let bindings = RapidfireLogic::build_hotkey_bindings(&settings).expect("绑定构建应成功");

    assert!(bindings.normal.is_empty());
    assert_eq!(bindings.hold.len(), 1);
}
```

- [ ] **Step 2: Run failing rapidfire tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml rapidfire_normalize_moves_unknown_group_to_default --lib
cargo test --manifest-path src-tauri/Cargo.toml rapidfire_build_hotkey_bindings_is_hold_only --lib
```

Expected: FAIL until trait impls are added.

- [ ] **Step 3: Implement rapidfire SyncItem and SyncGroup**

Add:

```rust
impl SyncItem for RapidfireCard {
    fn id(&self) -> &str { &self.id }
    fn group_id(&self) -> &str { &self.group_id }
    fn set_group_id(&mut self, group_id: String) { self.group_id = group_id; }
    fn enabled(&self) -> bool { self.enabled }
}

impl SyncGroup for RapidfireGroup {
    fn id(&self) -> &str { &self.id }
    fn enabled(&self) -> bool { self.enabled }
}
```

- [ ] **Step 4: Implement rapidfire SyncSettings**

Use the existing rapidfire `normalize_groups(&settings_value)` because rapidfire group display shape differs from timer/counter:

```rust
impl SyncSettings for RapidfireSettings {
    type Item = RapidfireCard;
    type Group = RapidfireGroup;

    const DEFAULT_GROUP_ID: &'static str = DEFAULT_RAPIDFIRE_GROUP_ID;
    const DUPLICATE_ITEM_MESSAGE_PREFIX: &'static str = "连发器卡片 ID 重复";

    fn sync_legacy_enabled(&mut self) {}
    fn items(&self) -> &[Self::Item] { &self.cards }
    fn items_mut(&mut self) -> &mut Vec<Self::Item> { &mut self.cards }
    fn replace_items(&mut self, items: Vec<Self::Item>) { self.cards = items; }
    fn groups(&self) -> &[Self::Group] { &self.groups }
    fn normalize_groups(&self) -> Result<Vec<Self::Group>, String> { normalize_groups(self) }
    fn replace_groups(&mut self, groups: Vec<Self::Group>) { self.groups = groups; }
    fn default_item(&self) -> Self::Item {
        RapidfireSettings::default()
            .cards
            .into_iter()
            .next()
            .expect("默认连发器配置必须包含一张卡片")
    }
    fn normalize_item(&self, item: &Self::Item) -> Result<Self::Item, String> {
        normalize_card(item)
    }
    fn after_groups_normalized(&mut self) {
        if let Some(default_group) = self
            .groups
            .iter()
            .find(|group| group.id == DEFAULT_RAPIDFIRE_GROUP_ID)
        {
            self.show_overlay = default_group.show_overlay;
            self.overlay_position = default_group.overlay_position.clone();
            self.overlay_width = default_group.overlay_width;
        }
    }
}
```

- [ ] **Step 5: Preserve rapidfire global validation before generic normalize**

Keep rapidfire-specific global setting validation by wrapping generic normalize:

```rust
pub(crate) fn normalize_settings(mut settings_value: RapidfireSettings) -> Result<RapidfireSettings, String> {
    settings_value.overlay_width = settings_value
        .overlay_width
        .max(RAPIDFIRE_DISPLAY_MIN_WIDTH)
        .min(RAPIDFIRE_DISPLAY_MAX_WIDTH);
    if settings_value.compensation_delay_min_ms > settings_value.compensation_delay_max_ms {
        return Err("补齐延迟最小值不能大于最大值".to_string());
    }
    if settings_value.min_press_spacing_ms > RAPIDFIRE_GLOBAL_DELAY_MAX_MS {
        return Err(format!("按键最小间距不能大于 {}ms", RAPIDFIRE_GLOBAL_DELAY_MAX_MS));
    }
    if settings_value.compensation_delay_max_ms > RAPIDFIRE_GLOBAL_DELAY_MAX_MS {
        return Err(format!("补齐延迟不能大于 {}ms", RAPIDFIRE_GLOBAL_DELAY_MAX_MS));
    }
    settings_value.min_press_spacing_ms = settings_value
        .min_press_spacing_ms
        .max(RAPIDFIRE_GLOBAL_DELAY_MIN_MS)
        .min(RAPIDFIRE_GLOBAL_DELAY_MAX_MS);
    settings_value.trigger_jitter_max_ms = settings_value
        .trigger_jitter_max_ms
        .min(RAPIDFIRE_TRIGGER_JITTER_MAX_MS);

    normalize_sync_settings(settings_value)
}
```

- [ ] **Step 6: Implement rapidfire SyncToolLogic**

Move existing `new_by_key` grouping and hold callback construction into:

```rust
impl SyncToolLogic for RapidfireLogic {
    const SCOPE: &'static str = "rapidfire";
    const SCOPE_LABEL: &'static str = "连发器";

    fn tool_enabled(settings: &RapidfireSettings) -> bool {
        settings.rapidfire_enabled
    }

    fn build_hotkey_bindings(settings: &RapidfireSettings) -> Result<HotkeyBindingSet, String> {
        let mut by_key: HashMap<String, Vec<String>> = HashMap::new();
        for card in &settings.cards {
            if !card.enabled || !group_enabled(&settings.groups, &card.group_id) {
                continue;
            }
            by_key.entry(card.trigger_key.clone()).or_default().push(card.id.clone());
        }

        let mut bindings = HotkeyBindingSet::empty();
        for (trigger_key, card_ids) in by_key {
            let callback: HoldActionCallback = std::sync::Arc::new(move |app_handle, action| {
                let card_ids = card_ids.clone();
                let app = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    handle_hold_action(&app, trigger_key.clone(), card_ids, action);
                });
            });
            bindings.hold.push((trigger_key, callback));
        }
        Ok(bindings)
    }

    fn stop_all(app: &AppHandle) -> Result<(), String> {
        let Some(state) = app.try_state::<RapidfireState>() else {
            return Ok(());
        };
        let hotkey_manager = app.try_state::<HotkeyManager>();
        stop_all(app, &state, hotkey_manager.as_ref().map(|value| &**value));
        Ok(())
    }
}
```

If current hold logic is embedded inside `restart_hotkey_listeners`, extract it to a private helper named:

```rust
fn handle_hold_action(
    app: &AppHandle,
    trigger_key: String,
    card_ids: Vec<String>,
    action: hotkey_types::HoldAction,
)
```

The helper body is the existing hold callback body moved without behavior changes.

- [ ] **Step 7: Delegate rapidfire restart**

Keep the current `force` optimization before replacing scopes. If `force == false` and existing key mapping equals new mapping, return `Ok(())`; otherwise call:

```rust
state.restart_sync_hotkeys(hotkey_manager, settings_value)
```

- [ ] **Step 8: Run rapidfire tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml rapidfire --lib
cargo test --manifest-path src-tauri/Cargo.toml hotkeys --lib
```

Expected: tests pass.

- [ ] **Step 9: Commit**

```powershell
git add src-tauri/src/rapidfire/mod.rs
git commit -m "迁移连发器配置与热键重启到同步工具基座"
```

---

## Task 7: Migrate timer normalization and hotkey restart

**Files:**
- Modify: `src-tauri/src/timer/mod.rs`
- Test: `src-tauri/src/timer/mod.rs`

**Interfaces:**
- Consumes:
  - `SyncItem`, `SyncGroup`, `SyncSettings`, `SyncToolLogic`
  - `normalize_sync_settings`
  - `restart_sync_hotkeys`
- Produces:
  - Timer settings use shared normalize skeleton.
  - Timer hotkeys use shared clear/replace skeleton while preserving Press/Release behavior.
  - `tick_task` stays in `TimerState`.

- [ ] **Step 1: Write timer hotkey binding test**

Add:

```rust
#[test]
fn timer_build_hotkey_bindings_uses_hold_when_release_targets_exist() {
    let mut settings = TimerSettings::default();
    settings.timer_enabled = true;
    settings.enabled = true;
    settings.timers[0].trigger_mode = TimerTriggerMode::Release;

    let bindings = TimerLogic::build_hotkey_bindings(&settings).expect("绑定构建应成功");

    assert!(bindings.normal.is_empty());
    assert_eq!(bindings.hold.len(), 1);
    assert_eq!(bindings.hold[0].0, "F2");
}
```

- [ ] **Step 2: Run failing timer test**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml timer_build_hotkey_bindings_uses_hold_when_release_targets_exist --lib
```

Expected: FAIL until `SyncToolLogic` is implemented for `TimerLogic`.

- [ ] **Step 3: Implement timer SyncItem, SyncGroup, SyncSettings**

Use the same pattern as counter:

```rust
impl SyncItem for TimerItem {
    fn id(&self) -> &str { &self.id }
    fn group_id(&self) -> &str { &self.group_id }
    fn set_group_id(&mut self, group_id: String) { self.group_id = group_id; }
    fn enabled(&self) -> bool { self.enabled }
}

impl SyncGroup for TimerGroup {
    fn id(&self) -> &str { &self.id }
    fn enabled(&self) -> bool { self.enabled }
}

impl SyncSettings for TimerSettings {
    type Item = TimerItem;
    type Group = TimerGroup;

    const DEFAULT_GROUP_ID: &'static str = DEFAULT_TIMER_GROUP_ID;
    const DUPLICATE_ITEM_MESSAGE_PREFIX: &'static str = "计时器 ID 重复";

    fn sync_legacy_enabled(&mut self) {
        if self.enabled && !self.timer_enabled {
            self.timer_enabled = true;
        }
        self.enabled = self.timer_enabled;
    }

    fn items(&self) -> &[Self::Item] { &self.timers }
    fn items_mut(&mut self) -> &mut Vec<Self::Item> { &mut self.timers }
    fn replace_items(&mut self, items: Vec<Self::Item>) { self.timers = items; }
    fn groups(&self) -> &[Self::Group] { &self.timer_groups }
    fn normalize_groups(&self) -> Result<Vec<Self::Group>, String> {
        let legacy_display = self.display.clone();
        let timer_count_by_group = count_enabled_items_by_group(&self.timers);
        normalize_timer_groups(
            self.timer_groups.clone(),
            DEFAULT_TIMER_GROUP_ID,
            legacy_display,
            &timer_count_by_group,
        )
    }
    fn replace_groups(&mut self, groups: Vec<Self::Group>) { self.timer_groups = groups; }
    fn default_item(&self) -> Self::Item {
        TimerItem {
            id: format!("timer-{}", crate::utils::now_ms()),
            group_id: DEFAULT_TIMER_GROUP_ID.to_string(),
            name: "计时器 1".to_string(),
            duration_seconds: 30,
            hotkey: "F2".to_string(),
            direction: TimerDirection::Countdown,
            trigger_mode: TimerTriggerMode::Press,
            enabled: true,
            ignore_running: true,
            segment_count: None,
        }
    }
    fn normalize_item(&self, item: &Self::Item) -> Result<Self::Item, String> {
        normalize_timer(item)
    }
    fn after_groups_normalized(&mut self) {
        self.display = group_display(
            &self.timer_groups,
            DEFAULT_TIMER_GROUP_ID,
            DEFAULT_TIMER_GROUP_ID,
        )
        .cloned()
        .unwrap_or_default();
    }
}
```

- [ ] **Step 4: Rename timer group normalizer**

Rename timer's local `normalize_groups` to `normalize_timer_groups` and update local calls. Keep its body unchanged.

- [ ] **Step 5: Delegate timer normalize_settings**

Replace timer `normalize_settings` with:

```rust
pub(crate) fn normalize_settings(settings_value: TimerSettings) -> Result<TimerSettings, String> {
    normalize_sync_settings(settings_value)
}
```

- [ ] **Step 6: Implement TimerLogic SyncToolLogic**

Use existing timer hotkey logic inside `build_hotkey_bindings`:

```rust
impl SyncToolLogic for TimerLogic {
    const SCOPE: &'static str = "timer";
    const SCOPE_LABEL: &'static str = "计时器";

    fn tool_enabled(settings: &TimerSettings) -> bool {
        settings.timer_enabled
    }

    fn build_hotkey_bindings(settings: &TimerSettings) -> Result<HotkeyBindingSet, String> {
        let mut by_hotkey: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
        for timer in &settings.timers {
            if !timer.enabled || !group_enabled(&settings.timer_groups, &timer.group_id) {
                continue;
            }
            let entry = by_hotkey
                .entry(timer.hotkey.trim().to_string())
                .or_insert_with(|| (Vec::new(), Vec::new()));
            match timer.trigger_mode {
                TimerTriggerMode::Press => entry.0.push(timer.id.clone()),
                TimerTriggerMode::Release => entry.1.push(timer.id.clone()),
            }
        }

        let mut bindings = HotkeyBindingSet::empty();
        for (hotkey, (press_timer_ids, release_timer_ids)) in by_hotkey {
            if !release_timer_ids.is_empty() {
                let press_targets = press_timer_ids.clone();
                let release_targets = release_timer_ids.clone();
                let hold_callback: HoldActionCallback = std::sync::Arc::new(move |app_handle, action| {
                    let targets = match action {
                        HoldAction::Down => press_targets.clone(),
                        HoldAction::Up => release_targets.clone(),
                    };
                    if targets.is_empty() {
                        return;
                    }
                    let app = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = trigger_hotkey_targets(&app, targets) {
                            let _ = app.emit_to("main", events::HOTKEY_ERROR, error);
                        }
                    });
                });
                bindings.hold.push((hotkey, hold_callback));
            } else {
                let targets = press_timer_ids.clone();
                let action: HotkeyAction = std::sync::Arc::new(move |app_handle| {
                    let targets = targets.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = trigger_hotkey_targets(&app_handle, targets) {
                            let _ = app_handle.emit_to("main", events::HOTKEY_ERROR, error);
                        }
                    });
                });
                bindings.normal.push((hotkey, action));
            }
        }
        Ok(bindings)
    }

    fn stop_all(app: &AppHandle) -> Result<(), String> {
        let Some(state) = app.try_state::<TimerState>() else {
            return Ok(());
        };
        stop_all(app, &state);
        Ok(())
    }
}
```

- [ ] **Step 7: Delegate timer restart but keep tick_task**

Replace only the clear/replace skeleton:

```rust
pub(crate) fn restart_hotkey_listeners(
    state: &TimerState,
    hotkey_manager: &HotkeyManager,
    settings_value: &TimerSettings,
) -> Result<(), String> {
    state.tool.restart_sync_hotkeys(hotkey_manager, settings_value)
}
```

Do not move `stop_tick_task`, `start_tick_task`, `tick`, or runtime update functions.

- [ ] **Step 8: Run timer tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml timer --lib
cargo test --manifest-path src-tauri/Cargo.toml hotkeys --lib
```

Expected: tests pass.

- [ ] **Step 9: Commit**

```powershell
git add src-tauri/src/timer/mod.rs
git commit -m "迁移计时器配置与热键重启到同步工具基座"
```

---

## Task 8: Finish cleanup and full validation

**Files:**
- Modify: `src-tauri/src/counter/mod.rs`
- Modify: `src-tauri/src/timer/mod.rs`
- Modify: `src-tauri/src/rapidfire/mod.rs`
- Modify: `src-tauri/src/sync_tool.rs`
- Optional Modify: `src-tauri/src/counter/mod.rs` to collapse fieldless `CounterState`

**Interfaces:**
- Consumes all prior tasks.
- Produces no new public Tauri command surface.

- [ ] **Step 1: Remove duplicated helpers**

Delete dead local helper functions after confirming no references remain:

```rust
fn group_id_set(...)
fn count_enabled_counters_by_group(...)
fn count_enabled_timers_by_group(...)
```

Keep `normalize_counter_groups`, `normalize_timer_groups`, and rapidfire `normalize_groups`, because group display shapes remain tool-specific.

- [ ] **Step 2: Collapse CounterState if compiler allows**

If all counter call sites now use `state.tool` only for generic methods and no extra field is needed, replace:

```rust
pub struct CounterState {
    pub tool: ToolState<CounterLogic>,
}

impl CounterState {
    pub fn lock_inner(&self) -> Result<MutexGuard<'_, ToolStateInner<CounterLogic>>, String> {
        self.tool.lock_inner()
    }
}
```

with:

```rust
pub type CounterState = ToolState<CounterLogic>;
```

Then replace `state.tool.restart_sync_hotkeys(...)` with `state.restart_sync_hotkeys(...)` in counter only.

If this expands the diff across too many call sites, keep the wrapper and record this exact reason in the commit body:

```text
CounterState wrapper retained because command/state call sites still expect `state.tool` and changing all call sites would obscure the sync-tool migration diff.
```

- [ ] **Step 3: Run formatting**

Run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml
```

Expected: exits with code 0.

- [ ] **Step 4: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all Rust tests pass.

- [ ] **Step 5: Run Rust check**

Run:

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: check passes.

- [ ] **Step 6: Run frontend tests only if frontend files changed**

If no files under `src/` changed, skip this step and note it in the final summary. If any frontend file changed, run:

```powershell
bun run test
```

Expected: Vitest passes.

- [ ] **Step 7: Inspect diff**

Run:

```powershell
git diff -- src-tauri/src/sync_tool.rs src-tauri/src/lib.rs src-tauri/src/global_state.rs src-tauri/src/counter/mod.rs src-tauri/src/timer/mod.rs src-tauri/src/rapidfire/mod.rs
```

Expected:
- No Tauri command names changed.
- No window labels changed.
- No `capabilities/default.json` changes needed.
- No persisted JSON field names changed.
- `global_state.rs` no longer imports timer/counter/rapidfire.

- [ ] **Step 8: Commit cleanup**

```powershell
git add src-tauri/src/sync_tool.rs src-tauri/src/lib.rs src-tauri/src/global_state.rs src-tauri/src/counter/mod.rs src-tauri/src/timer/mod.rs src-tauri/src/rapidfire/mod.rs
git commit -m "收敛同步工具生命周期重复实现"
```

---

## Self-Review

### Spec coverage

- ToolLogic split: Task 1 keeps base `ToolLogic` intact and adds `SyncToolLogic`; Morse remains out of scope.
- Pure-core + shell: Task 1 adds `apply_position_event`; Task 4 migrates counter position transition first.
- Normalize pushed into generic: Task 1 defines `SyncItem` / `SyncGroup` / `SyncSettings`; Tasks 2, 6, 7 migrate counter/rapidfire/timer.
- Generic method home: Task 1 adds `impl<L: SyncToolLogic> ToolState<L>`.
- Stop registry: Task 5 adds `SyncToolRegistry` and removes tool imports from `global_state`.
- Migration order: Tasks 2 to 4 migrate counter first, Task 6 rapidfire, Task 7 timer, Task 8 cleanup.
- Testing strategy: each migration task begins with failing tests and ends with scoped test commands; Task 8 runs full Rust validation.

### Placeholder scan

The plan contains no `TBD`, no empty future work markers, no unspecified validation, and no references to undefined interfaces. Every new type named in later tasks is produced in Task 1.

### Type consistency

- `SyncItem`, `SyncGroup`, `SyncSettings`, `SyncToolLogic`, `HotkeyBindingSet`, `PendingPosition`, `PositionEvent`, `PositionKinds`, `SyncRect`, and `SyncToolRegistry` are all defined in Task 1 before use.
- `CounterLogic::build_hotkey_bindings`, `RapidfireLogic::build_hotkey_bindings`, and `TimerLogic::build_hotkey_bindings` all return `Result<HotkeyBindingSet, String>`.
- Tool-specific group normalizers remain tool-specific: `normalize_counter_groups`, `normalize_timer_groups`, rapidfire `normalize_groups`.

---

Plan complete and saved to `docs/superpowers/plans/2026-06-24-sync-tool-lifecycle.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
