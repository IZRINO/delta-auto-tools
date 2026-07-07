# Issues 76-80 Follow-up Design

## Context

GitHub issues #76-80 were re-read after authentication. The current code already contains the first-round fixes, but new user feedback changes the target:

- #76: recognition cards still cannot move to other groups; each group needs a master enable switch.
- #77: punctuation hotkeys such as `,` and `.` are still reported as missing.
- #78: closed, no new user feedback in the re-read issue content.
- #79: same output hotkey can still be skipped when multiple recognition cards finish in close succession.
- #80: switching profiles can still freeze, with symptoms similar to the earlier #69/#73 main-thread WebView deadlock class.

## Goals

1. Recognition groups become first-class runtime gates: enabled groups run as today, disabled groups suppress their cards' listeners/watchers/effects without deleting settings.
2. Recognition cards can be moved between groups from the card editor; per-group order remains deterministic after every move.
3. Symbol hotkeys are verified end-to-end across frontend formatting, Rust parsing, keyboard event mapping, and output simulation.
4. Recognition output input is serialized globally so concurrent activation sessions cannot interleave Enigo key/mouse operations.
5. Profile apply/default-profile creation becomes non-blocking with respect to overlay/WebView reconciliation, so profile switching cannot self-deadlock on window creation.
6. Each functional fix is independently verified and committed before the next fix starts.

## Non-goals

- Do not redesign the recognition page beyond the controls required by #76/#77.
- Do not change Tauri command names, query parameter modes, window labels, or profile file locations.
- Do not close GitHub issues automatically; report results and wait for user confirmation.
- Do not treat #78 as active unless a new regression is found while touching shared click-effect code.

## Architecture Decisions

### Recognition groups

`RecognitionGroup` gets an `enabled: boolean` field. Existing persisted profiles/settings migrate by defaulting missing `enabled` to `true`.

Group enabled state gates runtime behavior in one place per backend path:

- hotkey listener registration ignores listener/activation hotkeys for cards in disabled groups;
- region/color watchers skip cards in disabled groups;
- activation sessions re-check group state before each recognition attempt and before executing effects;
- validation still validates saved card structure, but duplicate runtime hotkeys only consider enabled cards where the check is runtime-specific.

Frontend UI changes stay compact:

- group header gets a switch next to collapse/sort controls;
- card editor gets a group select/dropdown;
- changing group renumbers card `order` values in source and destination groups.

### Symbol hotkeys

The source already has several punctuation paths. The fix must prove the full chain instead of only adding another mapping:

- frontend recorder/formatter accepts literal `,`, `.`, `;`, `/`, `\`, `[`, `]`, `-`, `=`, `+`, `` ` ``, and `'`;
- Rust `parse_hotkey` accepts the same literals;
- `willhook::KeyboardKey::Other(0xBC)` and `Other(0xBE)` map to comma/period;
- output simulation can press comma/period through Enigo.

If a layer already passes, keep the test as regression coverage and only adjust UI copy where users discover supported keys.

### Recognition output serialization

All simulated input operations use one global async queue/lock:

- `press_hotkey_once`
- mouse click/click-points helper
- text typing helper if present in the module

The lock wraps the full physical input sequence, including modifier press/release and a short post-action gap. That prevents overlapping activation sessions from pressing the same output key at nearly the same time.

### Profile apply

Profile apply must not synchronously create or mutate overlay WebViews inside the command call. The command may update files, swap state, restart listeners, and emit state events; overlay/window reconciliation is scheduled after state application.

Required shape:

- `profile_apply` becomes async.
- `profile_create_default` uses the same apply lock as `profile_apply`.
- `apply_snapshot_to_tools` splits into state/hotkey application plus scheduled window reconciliation.
- timer uses its existing generation-based reconcile path.
- counter and rapidfire get small schedule wrappers mirroring timer behavior where needed.

## Verification Strategy

Each feature has targeted tests before its commit. After all feature commits:

```powershell
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
codegraph sync
git status --short
```

Manual validation targets:

- #76: move a recognition card from group A to group B; disable group B; verify its card no longer triggers.
- #77: record `,` and `.` as recognition hotkeys and as output keys.
- #79: configure A/B/C activation cards with the same output D; trigger A, B, C one second apart; verify every successful recognition emits D in order.
- #80: switch profiles repeatedly, including profiles with timer/counter/rapidfire overlays enabled; verify UI remains responsive.
