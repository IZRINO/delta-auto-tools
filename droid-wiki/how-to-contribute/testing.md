# Testing

## Frontend tests (Vitest)

Frontend tests live alongside the source in `src/` and use Vitest. Run all tests with `bun run test`, or with coverage via `bun run test:coverage` (coverage is currently scoped to `morse-utils.ts` only).

Run a single file:

```bash
bunx vitest run src/components/app/morse-utils.test.ts
```

### What is tested

| Test file | Scope |
|-----------|-------|
| `src/components/app/morse-utils.test.ts` | Morse serialization, formatting, hotkey parsing |
| `src/components/app/timer-utils.test.ts` | Timer settings conversion, progress calc, countdown formatting |
| `src/components/app/counter-utils.test.ts` | Counter settings conversion |
| `src/components/app/favorites-utils.test.ts` | Favorites ID read/write, card filtering |
| `src/components/app/rapidfire-types.test.ts` | Rapidfire type constants |
| `src/components/app/audio-utils.test.ts` | Audio color conversion, probe form parsing |
| `src/components/app/strategy-utils.test.ts` | Strategy site constants, refresh tiers |
| `src/components/app/theme-utils.test.ts` | Theme token merge, apply, import, hex normalization |
| `src/components/app/profile-utils.test.ts` | Profile timestamp format, name validation, snapshot helpers |
| `src/components/app/about-deps.test.ts` | Dependency list basics |
| `src/hooks/use-autosave.test.ts` | Autosave debounce logic |
| `src/hooks/use-bootstrap-form-logic.test.ts` | Bootstrap/form dual-state dirty detection |
| `src/hooks/use-hotkey-recorder.test.ts` | Hotkey recording interaction logic |
| `src/lib/logging.test.ts` | TraceId generation, setTraceId/clearTraceId, logFronted serialization |

### Pattern

Frontend tests focus on pure logic functions in `*-utils.ts` files and hook behavior. They do not test Tauri command invocation directly; instead, `useNativeShell()` returns false in the test environment, and tests assert that `invoke` is not called or that the hook degrades gracefully.

## Rust tests (cargo test)

Rust tests are inline `#[cfg(test)] mod tests` blocks within each module. Run with:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Run a single test:

```bash
cargo test --manifest-path src-tauri/Cargo.toml <test_name>
```

### What is tested

- `src-tauri/src/hotkey_types.rs` / `hotkeys.rs` - Hotkey parsing, conflict detection (Strict vs AllowHold), hold matching, combined modifier keys, timer+rapidfire co-firing.
- `src-tauri/src/morse/decoder.rs` - Morse decode for digits 0-9, unknown pattern errors.
- `src-tauri/src/morse/recognition.rs` - DPI/multi-monitor region coordinate conversion.
- `src-tauri/src/morse/mod.rs` - History push limit.
- `src-tauri/src/morse/types.rs` - Settings defaults.
- `src-tauri/src/timer/types.rs` / `settings.rs` - Timer defaults, settings read/write round-trip.
- `src-tauri/src/timer/mod.rs` - Transparent window size calculation, settings validation.
- `src-tauri/src/audio/types.rs` - Audio card deserialization with defaults, legacy field migration, color probe round-trip.
- `src-tauri/src/theme/apply.rs` - `merge_theme_tokens` (override, append, order, dedup), `find_theme`.
- `src-tauri/src/theme/builtins.rs` - 5 built-in themes, unique IDs, token key consistency.
- `src-tauri/src/theme/mod.rs` - `build_bootstrap`, `theme_import` (valid/invalid key rejection), export.
- `src-tauri/src/theme/settings.rs` - Load missing returns default, round-trip.
- `src-tauri/src/theme/types.rs` - Defaults, camelCase serialization, missing-field fallback.
- `src-tauri/src/profile/types.rs` / `settings.rs` / `mod.rs` - Snapshot empty, defaults, camelCase, round-trip, profile_id uniqueness.
- `src-tauri/src/logging/format.rs` / `writer.rs` / `mod.rs` - Format field order, rotation, cleanup (tempdir), level filtering, session_id, TraceContext.

### GameService test note

AGENTS.md references `src-tauri/src/delta/services/game.rs` tests using mockito. The delta module has been removed from the current codebase. Any references to those tests are historical.

## Adding tests

- New pure logic functions should have corresponding `*.test.ts` files.
- New Rust types should have round-trip serde tests and default-value tests.
- Hotkey conflict rules are guarded by tests in `hotkeys.rs`; if you change conflict policy, update those tests.
