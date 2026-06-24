# Background

Design decisions, pitfalls, and migration context for Delta Auto Tools.

## Design decisions

### Why Tauri instead of Electron

The app needs low-level Windows keyboard hooks (`WH_KEYBOARD_LL` via `willhook`), screen capture (`xcap`), and simulated keyboard input (`enigo`). These require native access that Rust provides directly. Tauri 2 gives a small binary, native performance, and a real WebView2 renderer without the Electron overhead.

### Why a single shared keyboard hook

Multiple keyboard hooks compete and can cause installation failures on Windows. The `HotkeyManager` installs one `willhook::keyboard_hook()` at startup and distributes events to all tool scopes. This avoids the "second hook fails to install" problem and centralizes conflict detection.

### Why `?mode=` instead of routing

Overlay windows (transparent, click-through, always-on-top) are separate Tauri windows that load the same frontend bundle with different query parameters. Using `?mode=overlay` / `?mode=timer-display` etc. lets each window render different content without a router. This is a hard constraint that cannot be replaced by client-side routing.

### Why the bootstrap/form dual-state pattern

The frontend needs to show Rust's canonical state (for display) while allowing local edits (for the form). Keeping them as separate objects with `JSON.stringify` dirty detection is simpler and more reliable than diffing individual fields. The 400ms autosave debounce with version guards prevents stale saves when the user types quickly.

### Why counter run-state is persisted separately

Counter values accumulate over time and should survive app restarts, but they are not user configuration. Storing them in `counter_state.json` (separate from `counter_settings.json`) means changing `start_value` or hotkey does not reset accumulated counts, and profile switching can reset counts without touching config.

### Why the design is dark-only

The industrial-brutalist aesthetic ("Swiss Industrial Print x Declassified Tactical Control Board") uses a dark carbon base with chalk structural lines and a single amber accent. A light mode would undermine the contrast and the "declassified tactical" feel. There is a `light` theme in the theme engine, but the default and primary experience is dark.

### Why strategy uses an embedded WebView, not iframe/proxy

iframes are blocked by most guide sites (X-Frame-Options). Proxying HTML loses cookies, JavaScript, and CAPTCHA handling. A real WebView2 sub-window inside the main window gives full browser capability (cookies, JS, localStorage, same-origin APIs) while staying within the app shell.

## Pitfalls

### AGENTS.md is stale

AGENTS.md and CLAUDE.md extensively document a `delta/` module that no longer exists in the codebase. Anyone reading these files will be confused by commands, types, and frontend pages that are not present. Trust the code and `lib.rs` over the docs when they disagree.

### Glob patterns in capabilities

The `src-tauri/capabilities/default.json` file lists which Tauri commands the frontend is allowed to invoke. Forgetting to add a new command here causes `invoke()` to silently fail or throw a permission error that is hard to trace.

### Hotkey conflict edge cases

The `AllowHold` policy only works between timer/counter normal scopes and rapidfire hold scope. Morse with `Strict` will reject any key that any other scope uses. When adding a new tool scope, decide its conflict policy carefully and add tests in `hotkeys.rs`.

### Transparent window rendering

Transparent overlay windows must not inherit the main window's dark paper CSS. The `data-overlay-mode` attribute on `document.body` is used to switch styles. Applying main-window backgrounds to overlays makes them opaque and blocks the game view.

### Serialized legacy fields

Several structs have `legacy_*` fields with `#[serde(skip_serializing)]` that exist only for backward-compatible deserialization. `normalize_settings` migrates these into modern fields. If you add a new field that replaces an old one, follow this pattern or old JSON files will fail to load.

## Migration context

The delta module removal is the largest migration in the project's history. It removed an entire backend subsystem (auth, game data, storage, encryption) and corresponding frontend pages. The documentation has not caught up. When working in this codebase, always verify that a command or page mentioned in AGENTS.md actually exists in `lib.rs` or `App.tsx` before relying on it.
