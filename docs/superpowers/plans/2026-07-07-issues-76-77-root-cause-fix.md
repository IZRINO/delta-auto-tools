# Issues 76-77 Root Cause Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix recognition card per-group order persistence and symbol hotkeys, including Chinese punctuation normalization.

**Architecture:** Keep hotkey config canonical as ASCII physical keys while accepting Chinese/full-width punctuation at input boundaries. Make recognition card ordering group-aware in frontend conversion and backend normalization, so each group owns its own contiguous `order` range.

**Tech Stack:** Tauri 2, Rust, React 19, TypeScript, Vite, Bun, Vitest, Cargo tests, CodeGraph.

---

## File Structure

- Modify: `src/components/app/morse-utils.ts`
  - Add canonical punctuation normalization for frontend hotkey recording.
- Modify: `src/components/app/morse-utils.test.ts`
  - Cover Chinese/full-width punctuation recording.
- Modify: `src-tauri/src/hotkey_types.rs`
  - Add Rust punctuation alias normalization and real `willhook::KeyboardKey` symbol variants.
- Modify: `src-tauri/src/key_suppressor.rs`
  - Map real `willhook::KeyboardKey` symbol variants back to Windows VK codes.
- Modify: `src-tauri/src/recognition/mod.rs`
  - Replace global order migration with group-aware card order normalization.
- Modify: `src/components/app/recognition-utils.ts`
  - Sort cards by group order, card order, and original index.
- Modify: `src/components/app/recognition-utils.test.ts`
  - Cover settings round-trip with duplicate per-group `order` values.
- Modify: `src/components/app/recognition-page.test.ts`
  - Cover move-to-group then reorder behavior.
- Modify: `droid-wiki/features/recognition.md`
  - Document per-group order persistence and Chinese punctuation normalization if current wording is stale.

---

## Task 1: Frontend Failure Tests

**Files:**
- Modify: `src/components/app/morse-utils.test.ts`
- Modify: `src/components/app/recognition-utils.test.ts`
- Modify: `src/components/app/recognition-page.test.ts`

- [x] **Step 1: Add hotkey normalization tests**

Add to `src/components/app/morse-utils.test.ts` near existing hotkey tests:

```ts
it("normalizes Chinese punctuation hotkeys to canonical physical keys", () => {
    expect(normalizeHotkeyPrimaryKey("，")).toBe(",");
    expect(normalizeHotkeyPrimaryKey("。")).toBe(".");
    expect(normalizeHotkeyPrimaryKey("；")).toBe(";");
    expect(normalizeHotkeyPrimaryKey("？")).toBe("/");
    expect(normalizeHotkeyPrimaryKey("、")).toBe("/");
    expect(normalizeHotkeyPrimaryKey("【")).toBe("[");
    expect(normalizeHotkeyPrimaryKey("】")).toBe("]");
    expect(normalizeHotkeyPrimaryKey("￥")).toBe("\\");
});

it("formats Chinese punctuation hotkeys as canonical ASCII", () => {
    expect(formatRecordedHotkey({
        key: "，",
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        metaKey: false,
    } as React.KeyboardEvent<HTMLButtonElement>)).toBe(",");
    expect(formatRecordedHotkey({
        key: "。",
        ctrlKey: true,
        altKey: false,
        shiftKey: false,
        metaKey: false,
    } as React.KeyboardEvent<HTMLButtonElement>)).toBe("Ctrl+.");
});
```

- [x] **Step 2: Add frontend order round-trip tests**

Add to `src/components/app/recognition-utils.test.ts`:

```ts
it("settingsToForm keeps duplicate per-group order values stable by group", () => {
    const settings = {
        recognitionEnabled: true,
        cardGroups: [
            {id: "g2", name: "二组", order: 1, collapsed: false, enabled: true},
            {id: "g1", name: "一组", order: 0, collapsed: false, enabled: true},
        ],
        cards: [
            makeRecognitionCard({id: "g2-a", groupId: "g2", order: 0, name: "G2 A"}),
            makeRecognitionCard({id: "g1-a", groupId: "g1", order: 0, name: "G1 A"}),
            makeRecognitionCard({id: "g1-b", groupId: "g1", order: 1, name: "G1 B"}),
        ],
    };

    const form = settingsToForm(settings);

    expect(form.cards.map((card) => card.id)).toEqual(["g1-a", "g1-b", "g2-a"]);
    expect(parseSettingsForm(form).cards.map((card) => [card.id, card.groupId, card.order])).toEqual([
        ["g1-a", "g1", 0],
        ["g1-b", "g1", 1],
        ["g2-a", "g2", 0],
    ]);
});
```

Use the existing local test card factory if present. If the file only has inline settings helpers, create a local helper in the test file:

```ts
function makeRecognitionCard(patch: Partial<RecognitionCard>): RecognitionCard {
    return {
        id: "card",
        groupId: "default-recognition-group",
        order: 0,
        name: "卡片",
        enabled: true,
        triggerMode: "hotkey",
        hotkey: "F1",
        watchRegion: null,
        watchReferenceImagePath: "",
        watchMatchThreshold: 0.75,
        watchPollIntervalMs: 500,
        activation: {mode: "always", hotkey: null, durationMs: 10000, triggerCount: 1},
        effects: {},
        cooldownMs: 1000,
        colorProbes: [],
        colorMatchMode: "all",
        colorMatchMethod: "average",
        ...patch,
    };
}
```

- [x] **Step 3: Add move-then-reorder regression test**

Add to `src/components/app/recognition-page.test.ts` in `recognition-page 分组排序 helper`:

```ts
it("moveCardToGroup 后可把新组内卡片上移到 order 0", async () => {
    const {moveCardToGroup, reorderCardsWithinGroup} = await import("@/components/app/recognition-page");
    const cards = [
        makeCard("a", "g1", 0),
        makeCard("b", "g1", 1),
        makeCard("x", "g2", 0),
    ];

    const moved = moveCardToGroup(cards, "b", "g2");
    const reordered = reorderCardsWithinGroup(moved, "g2", "b", -1);

    expect(
        reordered
            .filter((card) => card.groupId === "g2")
            .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
            .map((card) => [card.id, card.order]),
    ).toEqual([
        ["b", 0],
        ["x", 1],
    ]);
});
```

- [x] **Step 4: Run frontend tests and confirm failures**

Run:

```powershell
bunx vitest run src/components/app/morse-utils.test.ts src/components/app/recognition-utils.test.ts src/components/app/recognition-page.test.ts
```

Expected before implementation:

- Chinese punctuation tests fail because `normalizeHotkeyPrimaryKey` returns `null`.
- Round-trip/order test fails if global order sorting or backend-like assumptions reorder incorrectly.

---

## Task 2: Rust Failure Tests

**Files:**
- Modify: `src-tauri/src/hotkey_types.rs`
- Modify: `src-tauri/src/key_suppressor.rs`
- Modify: `src-tauri/src/recognition/mod.rs`

- [x] **Step 1: Add Rust hotkey parser/event tests**

In `src-tauri/src/hotkey_types.rs` tests, add:

```rust
#[test]
fn parses_chinese_punctuation_hotkey_aliases() {
    assert_eq!(
        hotkey_to_string("，").unwrap(),
        ","
    );
    assert_eq!(
        hotkey_to_string("Ctrl+。").unwrap(),
        "Ctrl+."
    );
    assert_eq!(
        hotkey_to_string("；").unwrap(),
        ";"
    );
    assert_eq!(
        hotkey_to_string("？").unwrap(),
        "/"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn maps_real_willhook_symbol_variants_to_primary_keys() {
    use willhook::event::KeyboardKey;

    assert_eq!(to_primary_key(KeyboardKey::Comma), Some(PrimaryKey::Named(NamedKey::Comma)));
    assert_eq!(to_primary_key(KeyboardKey::Period), Some(PrimaryKey::Named(NamedKey::Period)));
    assert_eq!(to_primary_key(KeyboardKey::Slash), Some(PrimaryKey::Named(NamedKey::Slash)));
    assert_eq!(to_primary_key(KeyboardKey::SemiColon), Some(PrimaryKey::Named(NamedKey::Semicolon)));
    assert_eq!(to_primary_key(KeyboardKey::Apostrophe), Some(PrimaryKey::Named(NamedKey::Quote)));
    assert_eq!(to_primary_key(KeyboardKey::LeftBrace), Some(PrimaryKey::Named(NamedKey::BracketLeft)));
    assert_eq!(to_primary_key(KeyboardKey::BackwardSlash), Some(PrimaryKey::Named(NamedKey::Backslash)));
    assert_eq!(to_primary_key(KeyboardKey::RightBrace), Some(PrimaryKey::Named(NamedKey::BracketRight)));
    assert_eq!(to_primary_key(KeyboardKey::Grave), Some(PrimaryKey::Named(NamedKey::Backquote)));
}
```

- [x] **Step 2: Add key suppressor mapping tests**

In `src-tauri/src/key_suppressor.rs` tests, add:

```rust
#[cfg(target_os = "windows")]
#[test]
fn maps_real_willhook_symbol_variants_to_vk_codes() {
    use willhook::event::KeyboardKey;

    assert_eq!(keyboard_key_to_vk(&KeyboardKey::Comma), Some(0xBC));
    assert_eq!(keyboard_key_to_vk(&KeyboardKey::Period), Some(0xBE));
    assert_eq!(keyboard_key_to_vk(&KeyboardKey::Slash), Some(0xBF));
    assert_eq!(keyboard_key_to_vk(&KeyboardKey::SemiColon), Some(0xBA));
    assert_eq!(keyboard_key_to_vk(&KeyboardKey::Apostrophe), Some(0xDE));
    assert_eq!(keyboard_key_to_vk(&KeyboardKey::LeftBrace), Some(0xDB));
    assert_eq!(keyboard_key_to_vk(&KeyboardKey::BackwardSlash), Some(0xDC));
    assert_eq!(keyboard_key_to_vk(&KeyboardKey::RightBrace), Some(0xDD));
    assert_eq!(keyboard_key_to_vk(&KeyboardKey::Grave), Some(0xC0));
}
```

- [x] **Step 3: Add backend order normalization test**

In `src-tauri/src/recognition/mod.rs` tests, add:

```rust
#[test]
fn normalize_settings_preserves_per_group_zero_orders() {
    let mut g1_a = base_card();
    g1_a.id = "g1-a".into();
    g1_a.group_id = Some("g1".into());
    g1_a.order = 0;

    let mut g1_b = base_card();
    g1_b.id = "g1-b".into();
    g1_b.group_id = Some("g1".into());
    g1_b.order = 1;

    let mut g2_a = base_card();
    g2_a.id = "g2-a".into();
    g2_a.group_id = Some("g2".into());
    g2_a.order = 0;

    let normalized = normalize_settings(types::RecognitionSettings {
        recognition_enabled: true,
        card_groups: vec![
            types::RecognitionGroup {
                id: "g1".into(),
                name: "一组".into(),
                order: 0,
                collapsed: false,
                enabled: true,
            },
            types::RecognitionGroup {
                id: "g2".into(),
                name: "二组".into(),
                order: 1,
                collapsed: false,
                enabled: true,
            },
        ],
        cards: vec![g2_a, g1_b, g1_a],
    });

    let grouped = normalized
        .cards
        .iter()
        .map(|card| (card.id.as_str(), card.group_id.as_deref(), card.order))
        .collect::<Vec<_>>();

    assert_eq!(
        grouped,
        vec![
            ("g1-a", Some("g1"), 0),
            ("g1-b", Some("g1"), 1),
            ("g2-a", Some("g2"), 0),
        ]
    );
}
```

- [x] **Step 4: Run Rust tests and confirm failures**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml hotkey_types
cargo test --manifest-path src-tauri/Cargo.toml key_suppressor
cargo test --manifest-path src-tauri/Cargo.toml recognition::
```

Expected before implementation:

- `to_primary_key(KeyboardKey::Comma/Period/...)` fails.
- `keyboard_key_to_vk(&KeyboardKey::Comma/Period/...)` fails.
- Chinese punctuation parser aliases fail.
- Order preservation test fails because `order == 0 && index > 0` rewrites valid group-first cards.

---

## Task 3: Implement Symbol Hotkey Canonicalization

**Files:**
- Modify: `src/components/app/morse-utils.ts`
- Modify: `src-tauri/src/hotkey_types.rs`
- Modify: `src-tauri/src/key_suppressor.rs`

- [x] **Step 1: Add frontend punctuation alias map**

In `src/components/app/morse-utils.ts`, inside `normalizeHotkeyPrimaryKey`, extend `specialKeyMap` with Chinese/full-width aliases:

```ts
        "，": ",",
        "。": ".",
        "；": ";",
        "？": "/",
        "、": "/",
        "【": "[",
        "「": "[",
        "】": "]",
        "」": "]",
        "￥": "\\",
        "｜": "\\",
        "－": "-",
        "＝": "=",
        "＋": "+",
        "｀": "`",
        "‘": "'",
        "’": "'",
```

- [x] **Step 2: Add Rust alias helper**

In `src-tauri/src/hotkey_types.rs`, add a small helper near `parse_primary`:

```rust
fn canonical_symbol_alias(segment: &str) -> Option<&'static str> {
    match segment {
        "，" => Some(","),
        "。" => Some("."),
        "；" => Some(";"),
        "？" | "、" => Some("/"),
        "【" | "「" => Some("["),
        "】" | "」" => Some("]"),
        "￥" | "｜" => Some("\\"),
        "－" => Some("-"),
        "＝" => Some("="),
        "＋" => Some("+"),
        "｀" => Some("`"),
        "‘" | "’" => Some("'"),
        _ => None,
    }
}
```

At the top of `parse_primary`, normalize:

```rust
    let segment = canonical_symbol_alias(segment).unwrap_or(segment);
```

Then leave the existing ASCII matching unchanged.

- [x] **Step 3: Map real willhook symbol variants**

In `src-tauri/src/hotkey_types.rs` `to_primary_key`, add these match arms before `Other(...)` fallback arms:

```rust
        KeyboardKey::Comma => Some(PrimaryKey::Named(NamedKey::Comma)),
        KeyboardKey::Period => Some(PrimaryKey::Named(NamedKey::Period)),
        KeyboardKey::Slash => Some(PrimaryKey::Named(NamedKey::Slash)),
        KeyboardKey::SemiColon => Some(PrimaryKey::Named(NamedKey::Semicolon)),
        KeyboardKey::Apostrophe => Some(PrimaryKey::Named(NamedKey::Quote)),
        KeyboardKey::LeftBrace => Some(PrimaryKey::Named(NamedKey::BracketLeft)),
        KeyboardKey::BackwardSlash => Some(PrimaryKey::Named(NamedKey::Backslash)),
        KeyboardKey::RightBrace => Some(PrimaryKey::Named(NamedKey::BracketRight)),
        KeyboardKey::Grave => Some(PrimaryKey::Named(NamedKey::Backquote)),
```

- [x] **Step 4: Map real willhook variants in suppressor**

In `src-tauri/src/key_suppressor.rs` `keyboard_key_to_vk`, add:

```rust
        KeyboardKey::SemiColon => Some(0xBA),
        KeyboardKey::Comma => Some(0xBC),
        KeyboardKey::Period => Some(0xBE),
        KeyboardKey::Slash => Some(0xBF),
        KeyboardKey::Grave => Some(0xC0),
        KeyboardKey::LeftBrace => Some(0xDB),
        KeyboardKey::BackwardSlash => Some(0xDC),
        KeyboardKey::RightBrace => Some(0xDD),
        KeyboardKey::Apostrophe => Some(0xDE),
```

- [x] **Step 5: Verify symbol tests**

Run:

```powershell
bunx vitest run src/components/app/morse-utils.test.ts
cargo test --manifest-path src-tauri/Cargo.toml hotkey_types key_suppressor
```

Expected: all pass.

---

## Task 4: Implement Group-aware Order Normalization

**Files:**
- Modify: `src/components/app/recognition-utils.ts`
- Modify: `src-tauri/src/recognition/mod.rs`

- [x] **Step 1: Add frontend group sort helper**

In `src/components/app/recognition-utils.ts`, add helper functions near `settingsToForm`:

```ts
function normalizeRecognitionCardGroupId(groupId: string | null | undefined, groupIds: Set<string>): string {
    const trimmed = groupId?.trim();
    return trimmed && groupIds.has(trimmed) ? trimmed : DEFAULT_RECOGNITION_GROUP_ID;
}

function recognitionCardSortKey(
    card: RecognitionCard,
    index: number,
    groupOrderById: Map<string, number>,
    groupIds: Set<string>,
): [number, number, number] {
    const groupId = normalizeRecognitionCardGroupId(card.groupId, groupIds);
    const groupOrder = groupOrderById.get(groupId) ?? Number.MAX_SAFE_INTEGER;
    const cardOrder = Number.isFinite(card.order ?? NaN) ? card.order ?? 0 : index;
    return [groupOrder, cardOrder, index];
}
```

- [x] **Step 2: Replace global card order sort in settingsToForm**

Change `settingsToForm` card mapping from global `.sort((a, b) => (a.order ?? 0) - (b.order ?? 0))` to sorting mapped items with their original index:

```ts
    const groupOrderById = new Map(cardGroups.map((group, index) => [group.id, group.order ?? index]));
    return {
        recognitionEnabled,
        audioEnabled: recognitionEnabled,
        cardGroups,
        cards: settings.cards
            .map((card, index) => ({card, index, key: recognitionCardSortKey(card, index, groupOrderById, groupIds)}))
            .sort((a, b) =>
                a.key[0] - b.key[0]
                || a.key[1] - b.key[1]
                || a.key[2] - b.key[2]
            )
            .map(({card, index}) => cardToForm({
                ...card,
                groupId: normalizeRecognitionCardGroupId(card.groupId, groupIds),
                order: Number.isFinite(card.order ?? NaN) ? card.order : index,
            })),
    };
```

- [x] **Step 3: Replace backend global order rewrite**

In `src-tauri/src/recognition/mod.rs`, remove:

```rust
        if card.order == 0 && index > 0 {
            card.order = index as i32;
        }
```

After the card field normalization loop, replace `cards.sort_by_key(|card| card.order);` with group-aware rebuild:

```rust
    let group_order_by_id = groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.id.clone(), (group.order, index)))
        .collect::<std::collections::HashMap<_, _>>();

    let mut indexed_cards = cards.into_iter().enumerate().collect::<Vec<_>>();
    indexed_cards.sort_by(|(left_index, left), (right_index, right)| {
        let left_group = left.group_id.as_deref().unwrap_or(DEFAULT_RECOGNITION_GROUP_ID);
        let right_group = right.group_id.as_deref().unwrap_or(DEFAULT_RECOGNITION_GROUP_ID);
        let left_group_order = group_order_by_id
            .get(left_group)
            .copied()
            .unwrap_or((i32::MAX, usize::MAX));
        let right_group_order = group_order_by_id
            .get(right_group)
            .copied()
            .unwrap_or((i32::MAX, usize::MAX));

        left_group_order
            .cmp(&right_group_order)
            .then_with(|| left.order.cmp(&right.order))
            .then_with(|| left_index.cmp(right_index))
    });

    let mut next_order_by_group = std::collections::HashMap::<String, i32>::new();
    let cards = indexed_cards
        .into_iter()
        .map(|(_, mut card)| {
            let group_id = card
                .group_id
                .clone()
                .unwrap_or_else(|| DEFAULT_RECOGNITION_GROUP_ID.to_string());
            let next_order = next_order_by_group.entry(group_id).or_insert(0);
            card.order = *next_order;
            *next_order += 1;
            card
        })
        .collect::<Vec<_>>();
```

- [x] **Step 4: Verify order tests**

Run:

```powershell
bunx vitest run src/components/app/recognition-utils.test.ts src/components/app/recognition-page.test.ts
cargo test --manifest-path src-tauri/Cargo.toml recognition::
```

Expected: all pass.

---

## Task 5: Docs, Full Verification, Commit

**Files:**
- Modify if stale: `droid-wiki/features/recognition.md`
- Read: `package.json`
- Read: `src-tauri/Cargo.toml`

- [x] **Step 1: Update wiki if needed**

In `droid-wiki/features/recognition.md`, ensure current behavior is documented:

```md
- 卡片排序按分组独立持久化；跨分组移动后源分组和目标分组会分别归一 `order`。
- 符号快捷键以 ASCII 物理键持久化；中文/全角标点录制时会归一到对应物理键，例如 `，` -> `,`、`。` -> `.`。
```

- [x] **Step 2: Run focused verification**

Run:

```powershell
bunx vitest run src/components/app/morse-utils.test.ts src/components/app/recognition-utils.test.ts src/components/app/recognition-page.test.ts
cargo test --manifest-path src-tauri/Cargo.toml hotkey_types
cargo test --manifest-path src-tauri/Cargo.toml key_suppressor
cargo test --manifest-path src-tauri/Cargo.toml recognition::
```

Expected: pass.

- [x] **Step 3: Run full verification**

Run:

```powershell
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
codegraph sync
git status --short
```

Expected: frontend tests pass, build passes, Rust tests pass, CodeGraph sync passes, only intended files remain modified.

- [x] **Step 4: Commit implementation**

Run:

```powershell
git status --short
git add src/components/app/morse-utils.ts src/components/app/morse-utils.test.ts src/components/app/recognition-utils.ts src/components/app/recognition-utils.test.ts src/components/app/recognition-page.test.ts src-tauri/src/hotkey_types.rs src-tauri/src/key_suppressor.rs src-tauri/src/recognition/mod.rs droid-wiki/features/recognition.md docs/superpowers/plans/2026-07-07-issues-76-77-root-cause-fix.md
git commit -m "fix(recognition): 修复分组排序与符号快捷键"
```

Expected: commit succeeds with only #76/#77 root-cause fix files.
