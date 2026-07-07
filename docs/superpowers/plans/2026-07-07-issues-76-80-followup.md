# Issues 76-80 Follow-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish GitHub issues #76, #77, #79, and #80 follow-up fixes after the first-round patch missed user-reported behavior.

**Architecture:** Add recognition group runtime gates, prove punctuation hotkey support end-to-end, serialize simulated input, and make profile apply schedule overlay/window reconciliation instead of doing synchronous WebView work.

**Tech Stack:** Tauri 2, Rust, React 19, TypeScript, Vite, Bun, Vitest, Cargo tests.

---

## File Structure

Target files:

- `src-tauri/src/recognition/` for group model, validation, hotkey/listener filtering, watcher filtering, and effects.
- `src/components/app/` for recognition group UI, card group movement, hotkey formatter tests, and recognition page tests.
- `src-tauri/src/input_simulation.rs` for global input serialization.
- `src-tauri/src/profile/mod.rs`, `src-tauri/src/timer/mod.rs`, `src-tauri/src/counter/mod.rs`, and `src-tauri/src/rapidfire/` for non-blocking profile apply/window reconcile.
- `droid-wiki/features/recognition.md` and `droid-wiki/systems/profile-system.md` for behavior docs.

Do not modify command names, window labels, query `mode` handling, profile file paths, or issue state.

---

## Task 1 - #76 Recognition Group Switch And Cross-group Move

- [ ] Add failing frontend tests before implementation.

  Cover:

  - missing `enabled` on old group data normalizes to `true`;
  - disabled group state persists in normalized settings;
  - moving a card to another group updates `groupId`;
  - card orders are contiguous inside source and destination groups;
  - group switch UI sends a settings patch.

  Command:

  ```powershell
  bunx vitest run src/components/app/recognition-utils.test.ts src/components/app/recognition-page.test.ts
  ```

- [ ] Add failing Rust tests before implementation.

  Cover:

  - deserializing a group without `enabled` yields `enabled = true`;
  - runtime helper treats missing/unknown group as enabled for backward compatibility;
  - disabled group cards are omitted from runtime listener/watcher plans.

  Command:

  ```powershell
  cargo test --manifest-path src-tauri/Cargo.toml recognition::
  ```

- [ ] Extend recognition group data model.

  TypeScript shape:

  ```ts
  export interface RecognitionGroup {
    id: string
    name: string
    order: number
    collapsed: boolean
    enabled: boolean
  }
  ```

  Rust shape:

  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
  #[serde(rename_all = "camelCase")]
  pub struct RecognitionGroup {
      pub id: String,
      pub name: String,
      pub order: i32,
      pub collapsed: bool,
      #[serde(default = "default_true")]
      pub enabled: bool,
  }

  fn default_true() -> bool {
      true
  }
  ```

  Ensure all new group constructors set `enabled: true`.

- [ ] Add backend group runtime helper and use it in listener/watcher paths.

  Required helper behavior:

  ```rust
  fn card_group_enabled(settings: &RecognitionSettings, card: &RecognitionCard) -> bool {
      card.group_id
          .as_deref()
          .and_then(|group_id| settings.groups.iter().find(|group| group.id == group_id))
          .map(|group| group.enabled)
          .unwrap_or(true)
  }
  ```

  Apply this helper before registering listener hotkeys, activation hotkeys, region watchers, color watchers, and before activation-session effect execution.

- [ ] Add card move helper on the frontend.

  Required behavior:

  - selecting target group changes only that card's `groupId`;
  - card receives last order in target group;
  - source and target group orders are normalized after the move;
  - ungrouped/default group remains valid.

  Use one helper rather than mutating arrays inline in JSX.

- [ ] Add UI controls.

  - Group header: add `Switch`/daisyUI toggle for group `enabled`.
  - Card editor: add group select next to card identity/name controls.
  - Disabled group remains visible and editable; runtime disabled state must not delete card settings.

- [ ] Update docs.

  - `droid-wiki/features/recognition.md`: group switch, cross-group movement, disabled group runtime effect.

- [ ] Verify Task 1.

  ```powershell
  bunx vitest run src/components/app/recognition-utils.test.ts src/components/app/recognition-page.test.ts
  cargo test --manifest-path src-tauri/Cargo.toml recognition::
  ```

- [ ] Commit Task 1.

  ```powershell
  git status --short
  git add src/components/app src-tauri/src/recognition droid-wiki/features/recognition.md
  git commit -m "feat(recognition): 支持卡片跨分组与分组开关"
  ```

---

## Task 2 - #77 Symbol Hotkey Proof And Fix

- [ ] Add failing tests for comma and period through each supported layer.

  Frontend cases:

  ```ts
  expect(normalizeHotkeyPrimaryKey(",")).toBe(",")
  expect(normalizeHotkeyPrimaryKey(".")).toBe(".")
  expect(formatRecordedHotkey({ primary: ",", modifiers: [] })).toBe(",")
  expect(formatRecordedHotkey({ primary: ".", modifiers: ["Ctrl"] })).toBe("Ctrl+.")
  ```

  Rust parser/event cases:

  ```rust
  assert_eq!(parse_hotkey(",").unwrap().primary, HotkeyPrimary::Named(NamedKey::Comma));
  assert_eq!(parse_hotkey(".").unwrap().primary, HotkeyPrimary::Named(NamedKey::Period));
  assert_eq!(to_primary_key(&KeyboardKey::Other(0xBC)), Some(HotkeyPrimary::Named(NamedKey::Comma)));
  assert_eq!(to_primary_key(&KeyboardKey::Other(0xBE)), Some(HotkeyPrimary::Named(NamedKey::Period)));
  ```

  Output simulation mapping cases must verify comma/period map to their Enigo key variants without sending real input.

- [ ] Run the failing tests.

  ```powershell
  bunx vitest run src/components/app/morse-utils.test.ts src/components/app/timer-utils.test.ts src/components/app/counter-utils.test.ts src/components/app/recognition-page.test.ts
  cargo test --manifest-path src-tauri/Cargo.toml hotkey_types input_simulation
  ```

- [ ] Fix the failing layer only.

  Acceptable fixes:

  - frontend recorder normalization missing a punctuation literal;
  - frontend display/validation rejects punctuation despite recorder support;
  - Rust `willhook` event mapping misses `Other(0xBC)` or `Other(0xBE)`;
  - output simulation lacks comma/period mapping.

  Do not duplicate independent hotkey vocabularies unless the existing module has no shared helper.

- [ ] Make supported punctuation discoverable in UI.

  Recognition hotkey recorder helper text must explicitly include:

  ```text
  支持字母、数字、F1-F24、方向键及 , . ; / \ [ ] - = + ` ' 等符号
  ```

- [ ] Update docs.

  - `droid-wiki/features/recognition.md`: supported symbol hotkeys.

- [ ] Verify Task 2.

  ```powershell
  bunx vitest run src/components/app/morse-utils.test.ts src/components/app/timer-utils.test.ts src/components/app/counter-utils.test.ts src/components/app/recognition-page.test.ts
  cargo test --manifest-path src-tauri/Cargo.toml hotkey_types input_simulation
  ```

- [ ] Commit Task 2.

  ```powershell
  git status --short
  git add src/components/app src-tauri/src/hotkey_types.rs src-tauri/src/input_simulation.rs droid-wiki/features/recognition.md
  git commit -m "fix(hotkeys): 支持符号快捷键录制与触发"
  ```

---

## Task 3 - #79 Serialize Recognition Output Input

- [ ] Add failing Rust tests for input serialization before implementation.

  Test design:

  - create two async simulated input jobs;
  - first job blocks on a channel while holding the input queue;
  - second job must not start until first job releases;
  - event order must be `first-start`, `first-end`, `second-start`, `second-end`.

  Test helper signature:

  ```rust
  #[cfg(test)]
  async fn run_serialized_input_for_test<F, Fut>(operation: F) -> Result<(), String>
  where
      F: FnOnce() -> Fut + Send + 'static,
      Fut: Future<Output = Result<(), String>> + Send + 'static,
  ```

- [ ] Run the failing test.

  ```powershell
  cargo test --manifest-path src-tauri/Cargo.toml input_simulation
  ```

- [ ] Implement global input serialization in `src-tauri/src/input_simulation.rs`.

  Required structure:

  ```rust
  static INPUT_SIMULATION_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
  const INPUT_POST_ACTION_GAP: Duration = Duration::from_millis(35);

  async fn run_serialized_input<F, T>(operation: F) -> Result<T, String>
  where
      F: FnOnce() -> Result<T, String> + Send + 'static,
      T: Send + 'static,
  {
      let lock = INPUT_SIMULATION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
      let _guard = lock.lock().await;
      let result = tokio::task::spawn_blocking(operation)
          .await
          .map_err(|err| format!("输入模拟任务失败: {err}"))?;
      tokio::time::sleep(INPUT_POST_ACTION_GAP).await;
      result
  }
  ```

  Wrap every physical Enigo action path with the helper:

  - `press_hotkey_once`
  - click effect helper
  - text typing helper if present

- [ ] Ensure recognition effects call the serialized paths.

  `recognition/effects.rs` must not create its own Enigo instance or bypass `input_simulation`.

- [ ] Add focused logging.

  Log at debug level:

  - input action kind;
  - primary key/button;
  - start/end;
  - card id when available from recognition effects.

- [ ] Verify Task 3.

  ```powershell
  cargo test --manifest-path src-tauri/Cargo.toml input_simulation recognition::effects recognition::
  ```

- [ ] Commit Task 3.

  ```powershell
  git status --short
  git add src-tauri/src/input_simulation.rs src-tauri/src/recognition
  git commit -m "fix(recognition): 串行化触发效果按键输出"
  ```

---

## Task 4 - #80 Non-blocking Profile Apply

- [ ] Add focused Rust tests or compile-time guards before implementation.

  Required coverage:

  - `profile_create_default` and `profile_apply` both use the same apply lock helper;
  - profile apply state phase does not call direct `ensure_display_windows` or `ensure_overlay_window`;
  - timer/counter/rapidfire expose schedule wrappers for profile reconciliation.

  Use unit tests for pure helpers where practical, and rely on Cargo tests/build for Tauri command signature safety.

- [ ] Convert profile commands to async.

  Required command signatures:

  ```rust
  #[tauri::command]
  pub async fn profile_apply(app: AppHandle, state: State<'_, ProfileState>, id: String) -> Result<(), String>
  ```

  ```rust
  #[tauri::command]
  pub async fn profile_create_default(
      app: AppHandle,
      state: State<'_, ProfileState>,
      name: String,
  ) -> Result<ProfileEntry, String>
  ```

- [ ] Share apply lock between profile apply and default-profile creation.

  Required helper:

  ```rust
  fn acquire_apply_lock(state: &ProfileState) -> Result<std::sync::MutexGuard<'_, ()>, String> {
      state
          .apply_lock
          .lock()
          .map_err(|_| "配置文件切换锁已损坏".to_string())
  }
  ```

  Both commands must acquire this lock before reading/writing profile state and applying tool snapshots.

- [ ] Split profile apply into state phase and window reconcile phase.

  Required function shape:

  ```rust
  fn apply_snapshot_to_tools(app: &AppHandle, snapshot: &ProfileSnapshot) -> Result<(), String> {
      apply_snapshot_to_tool_state(app, snapshot)?;
      schedule_profile_window_reconcile(app, snapshot);
      Ok(())
  }
  ```

  `apply_snapshot_to_tool_state` may save settings, swap managed state, restart hotkeys/watchers, reset counter runtime state, and emit tool state events. It must not call direct WebView creation helpers.

- [ ] Add/reuse window reconcile schedulers.

  Required public module helpers:

  ```rust
  pub(crate) fn schedule_display_windows_reconcile_from_profile(app: &AppHandle, settings: &TimerSettings);
  pub(crate) fn schedule_counter_windows_reconcile_from_profile(app: &AppHandle, settings: &CounterSettings);
  pub(crate) fn schedule_overlay_window_reconcile_from_profile(app: &AppHandle, settings: &RapidfireSettings);
  ```

  Timer should reuse existing generation-based reconcile. Counter and rapidfire should use the same behavior pattern: clone `AppHandle`, clone settings, spawn async task, call existing `ensure_*` inside the task, log error instead of failing profile apply after state already switched.

- [ ] Update profile-system docs.

  - `droid-wiki/systems/profile-system.md`: profile apply is serialized; overlay/window reconcile is scheduled after state apply.

- [ ] Verify Task 4.

  ```powershell
  cargo test --manifest-path src-tauri/Cargo.toml profile:: timer:: counter:: rapidfire::
  bunx vitest run src/hooks/use-profile.test.ts src/components/app/profile-switcher.test.ts
  ```

- [ ] Commit Task 4.

  ```powershell
  git status --short
  git add src-tauri/src/profile src-tauri/src/timer src-tauri/src/counter src-tauri/src/rapidfire droid-wiki/systems/profile-system.md
  git commit -m "fix(profile): 异步应用配置并延后窗口刷新"
  ```

---

## Task 5 - Final Verification And Issue Reply Draft

- [ ] Run full verification.

  ```powershell
  bun run test
  bun run build
  cargo test --manifest-path src-tauri/Cargo.toml
  codegraph sync
  git status --short
  git log --oneline --max-count=8
  ```

- [ ] Manually validate issue scenarios.

  - #76: move card between groups; disable target group; verify no trigger from disabled group.
  - #77: record `,` and `.` as listener/activation/output hotkeys.
  - #79: trigger cards A/B/C one second apart with same output D; verify every successful recognition emits D in order.
  - #80: switch profiles repeatedly while timer/counter/rapidfire overlays are enabled; verify no UI freeze.

- [ ] Prepare GitHub issue reply draft without closing issues.

  Required content:

  - #76: cross-group move and group master switch fixed; mention disabled groups do not delete cards.
  - #77: punctuation hotkeys verified across recorder/parser/output path; list tested symbols.
  - #79: recognition output simulation serialized; same output key no longer races between near-simultaneous card completions.
  - #80: profile apply is async/serialized and overlay reconciliation is delayed outside command path.
  - #78: unchanged unless regression found.

- [ ] Do not create a final extra commit unless Task 5 changes tracked files.

---

## Acceptance Criteria

- #76 cards can move across groups and group master switch disables runtime triggers.
- #77 `,` and `.` work as recognition hotkeys and output hotkeys, with regression tests.
- #79 same output hotkey from staggered activation sessions is serialized and not dropped by overlapping input simulation.
- #80 profile switching no longer performs synchronous overlay/WebView reconciliation inside the command path.
- Each functional task has its own commit.
- Full frontend tests, frontend build, and Rust tests complete before reporting development done.
