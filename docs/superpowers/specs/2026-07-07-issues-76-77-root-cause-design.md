# Issues 76-77 Root Cause Fix Design

## Context

Issue #76 and #77 already have first-round fixes on `master`, but user feedback shows both still fail in real use.

- #76 latest symptom: after moving a recognition card to a new group, moving it up/down sometimes does not persist; UI later shows original order.
- #77 latest symptom: symbol hotkeys still do not trigger. New scope: Chinese punctuation input such as `，` and `。` must also record as usable hotkeys.

This design replaces patch-by-guess work with fixes tied to confirmed code paths.

## Goals

1. Recognition card order must survive `frontend reorder -> autosave -> backend normalize -> stateChanged/settingsToForm` round trip.
2. Card order must be deterministic per group, not globally across all groups.
3. Symbol hotkeys must work through recorder formatting, saved config parsing, global hook event mapping, suppression mapping, and output simulation.
4. Chinese/full-width punctuation entered during recording or imported config must normalize to the same physical ASCII hotkey used at runtime.
5. Tests must reproduce the old failure paths, not only isolated helper behavior.

## Non-goals

- Do not change Tauri command names, query parameters, window labels, profile file locations, or issue state.
- Do not redesign recognition UI beyond any required hotkey helper text.
- Do not store Chinese punctuation as canonical config. Hotkeys represent physical keys, not text input.
- Do not close GitHub issues automatically.

## Root Causes

### #76 order persistence

The backend `normalize_settings` currently treats `card.order == 0 && index > 0` as legacy missing order and rewrites it to the global array index. That is invalid after groups were added because every group can legitimately contain a first card with `order = 0`.

The same model mismatch exists in frontend conversion: `settingsToForm` sorts all cards globally by `order`, then `cardsForGroup` filters by group and sorts again. When multiple groups contain `order = 0`, array order can drift after save/load even if each group is internally valid.

### #77 symbol hotkeys

The parser accepts several ASCII punctuation literals, but the real Windows hook path does not map through `KeyboardKey::Other(0xBC)` for comma/period. `willhook 0.6.3` exposes dedicated variants such as `KeyboardKey::Comma`, `KeyboardKey::Period`, `KeyboardKey::Slash`, `KeyboardKey::SemiColon`, `KeyboardKey::Apostrophe`, `KeyboardKey::LeftBrace`, `KeyboardKey::BackwardSlash`, `KeyboardKey::RightBrace`, and `KeyboardKey::Grave`.

Current tests use synthetic `Other(0xBC)`/`Other(0xBE)`, so they pass while real keyboard events miss the mapping.

Frontend recording also only accepts ASCII punctuation values from `event.key`. Under Chinese IME, users may produce `，` or `。`; those should normalize to `,` and `.` rather than being rejected.

## Design

### Canonical hotkey representation

Persist and compare only canonical physical-key strings:

- `，` -> `,`
- `。` -> `.`
- `；` -> `;`
- `、` and `？` -> `/`
- `【` and `「` -> `[`
- `】` and `」` -> `]`
- `￥` and `｜` -> `\`
- `－` -> `-`
- `＝` -> `=`
- `＋` -> `+`
- `｀` -> `` ` ``
- `‘` and `’` -> `'`

This mapping applies in both frontend recorder normalization and Rust hotkey parsing. Stored settings stay ASCII, so runtime matching remains independent of IME state.

### Runtime hook mapping

Extend `hotkey_types::to_primary_key` to handle the real willhook variants:

- `Comma` -> `NamedKey::Comma`
- `Period` -> `NamedKey::Period`
- `Slash` -> `NamedKey::Slash`
- `SemiColon` -> `NamedKey::Semicolon`
- `Apostrophe` -> `NamedKey::Quote`
- `LeftBrace` -> `NamedKey::BracketLeft`
- `BackwardSlash` -> `NamedKey::Backslash`
- `RightBrace` -> `NamedKey::BracketRight`
- `Grave` -> `NamedKey::Backquote`

Keep `Other(0xBA..0xDE)` as fallback for suppressed/manual event conversion and compatibility.

Extend `key_suppressor::keyboard_key_to_vk` with the same dedicated willhook variants so suppressed keys are detected consistently and do not double-dispatch or fail to dispatch.

### Group-aware order normalization

Backend `normalize_settings` must stop rewriting every non-first `order = 0`. Instead:

1. Normalize card IDs and group IDs as today.
2. Group cards by normalized `group_id`.
3. For each group, sort by `(order, original_index)` to keep deterministic order.
4. Renumber orders inside that group from `0..n`.
5. Rebuild card list in group order, then card order inside each group.

Frontend conversion mirrors this model:

1. `settingsToForm` normalizes groups first.
2. Cards get valid `groupId` and numeric order.
3. Cards are sorted by `(group order, card order, original index)`, not by card order alone.
4. `cardsForGroup`, `reorderCardsWithinGroup`, and `moveCardToGroup` keep per-group orders contiguous.

### Event merge behavior

`mergeRecognitionWatchRegionsIntoForm` should keep local drafts and only merge backend-derived watch/click/probe regions, as it does now. It must not overwrite local order/group edits from a concurrent `stateChanged` event unless the save response itself updates form through `saveSettings`.

## Tests

### Frontend

Add or update Vitest coverage:

- `normalizeHotkeyPrimaryKey` maps Chinese punctuation to canonical ASCII.
- `formatRecordedHotkey` returns canonical ASCII for Chinese punctuation input.
- `settingsToForm` preserves per-group order across duplicate `order` values.
- `parseSettingsForm(settingsToForm(settings))` round-trips group/card order.
- `moveCardToGroup` followed by `reorderCardsWithinGroup` keeps source and target group orders contiguous.

### Rust

Add or update Cargo tests:

- `HotkeyBinding::parse` accepts Chinese punctuation aliases and canonicalizes through `hotkey_to_string`.
- `to_primary_key` maps real willhook variants for supported punctuation.
- `keyboard_key_to_vk` maps real willhook variants to Windows VK codes.
- `normalize_settings` preserves legitimate per-group `order = 0` cards and renumbers per group.
- Existing `Other(0xBC)`/`Other(0xBE)` fallback tests remain.

## Verification

Focused checks:

```powershell
bunx vitest run src/components/app/morse-utils.test.ts src/components/app/recognition-utils.test.ts src/components/app/recognition-page.test.ts
cargo test --manifest-path src-tauri/Cargo.toml hotkey_types key_suppressor recognition::
```

Full checks before reporting completion:

```powershell
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
codegraph sync
git status --short
```

## Documentation Updates

Update `droid-wiki/features/recognition.md` if behavior wording needs to mention Chinese punctuation normalization or per-group order persistence. No `AGENTS.md`/`README.md` change is needed because no command, route, persistent file path, dev script, or top-level project convention changes.
