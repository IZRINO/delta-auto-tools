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
    Begin { group_id: String, rect: R },
    Moved { x: i32, y: i32 },
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
            let Some(_current) = pending else {
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

    pub fn registered_names(&self) -> Vec<&'static str> {
        self.handlers.iter().map(|(name, _)| *name).collect()
    }
}

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

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestGroup {
        id: String,
        enabled: bool,
    }

    impl SyncGroup for TestGroup {
        fn id(&self) -> &str {
            &self.id
        }
        fn enabled(&self) -> bool {
            self.enabled
        }
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

        fn items(&self) -> &[Self::Item] {
            &self.items
        }
        fn items_mut(&mut self) -> &mut Vec<Self::Item> {
            &mut self.items
        }
        fn replace_items(&mut self, items: Vec<Self::Item>) {
            self.items = items;
        }
        fn groups(&self) -> &[Self::Group] {
            &self.groups
        }
        fn normalize_groups(&self) -> Result<Vec<Self::Group>, String> {
            let mut groups = self.groups.clone();
            if groups.is_empty() {
                groups.push(TestGroup {
                    id: "default".to_string(),
                    enabled: true,
                });
            }
            Ok(groups)
        }
        fn replace_groups(&mut self, groups: Vec<Self::Group>) {
            self.groups = groups;
        }
        fn default_item(&self) -> Self::Item {
            TestItem {
                id: "item-1".to_string(),
                group_id: "default".to_string(),
                enabled: true,
            }
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
        let settings = TestSettings {
            enabled: false,
            groups: vec![],
            items: vec![],
        };

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
            groups: vec![TestGroup {
                id: "default".to_string(),
                enabled: true,
            }],
            items: vec![TestItem {
                id: "item-1".to_string(),
                group_id: "missing".to_string(),
                enabled: true,
            }],
        };

        let normalized = normalize_sync_settings(settings).expect("规范化应成功");

        assert_eq!(normalized.items[0].group_id, "default");
    }

    #[test]
    fn normalize_sync_settings_rejects_duplicate_item_ids() {
        let settings = TestSettings {
            enabled: false,
            groups: vec![TestGroup {
                id: "default".to_string(),
                enabled: true,
            }],
            items: vec![
                TestItem {
                    id: "same".to_string(),
                    group_id: "default".to_string(),
                    enabled: true,
                },
                TestItem {
                    id: "same".to_string(),
                    group_id: "default".to_string(),
                    enabled: true,
                },
            ],
        };

        let error = normalize_sync_settings(settings).expect_err("重复 ID 应报错");

        assert_eq!(error, "测试条目 ID 重复: same");
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    impl SyncRect for TestRect {
        fn with_position(&self, x: i32, y: i32) -> Self {
            Self {
                x,
                y,
                width: self.width,
                height: self.height,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TestKind {
        Selected,
        Cancelled,
        Closed,
    }

    impl PositionKinds for TestKind {
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

    #[test]
    fn apply_position_event_moves_pending_rect() {
        let pending = Some(PendingPosition {
            group_id: "g".to_string(),
            original_rect: TestRect {
                x: 1,
                y: 2,
                width: 320,
                height: 96,
            },
            staged_rect: TestRect {
                x: 1,
                y: 2,
                width: 320,
                height: 96,
            },
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
            original_rect: TestRect {
                x: 1,
                y: 2,
                width: 320,
                height: 96,
            },
            staged_rect: TestRect {
                x: 5,
                y: 6,
                width: 320,
                height: 96,
            },
        });

        let decision = apply_position_event::<TestRect, TestKind>(pending, PositionEvent::Commit)
            .expect("提交事件应成功");

        assert!(decision.pending.is_none());
        assert!(decision.save);
        assert_eq!(decision.send, Some(TestKind::Selected));
        assert!(decision.destroy_window);
    }

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

        let names = registry.registered_names();
        assert_eq!(names, vec!["ok", "bad"]);
    }
}
