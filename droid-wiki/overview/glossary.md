# Glossary

Terms used throughout Delta Auto Tools that may not be obvious to a new reader.

| Term | Meaning |
|------|---------|
| Bootstrap | The initial state Rust returns to the frontend via `xxx_get_bootstrap`. Contains settings plus runtime state (runs, history, errors). The frontend treats it as immutable canonical state. |
| Form | The local editable draft state in the frontend, derived from bootstrap. Compared via `JSON.stringify` for dirty detection. |
| Autosave | A 400ms debounced save triggered when the form diverges from bootstrap. Guarded by `autosaveVersionRef` to prevent stale overwrites. |
| Overlay window | A transparent, borderless, always-on-top, click-through Tauri window used for in-game display. Timer, counter, and rapidfire each have one. |
| Position window | A calibration-style window for dragging the overlay to a screen position. Entered via `?mode=*-position`. |
| Display window | The actual transparent overlay that shows timer/counter/rapidfire data. Entered via `?mode=*-display`. |
| Scope | A named hotkey registration group (e.g. `"morse"`, `"timer"`, `"counter"`, `"rapidfire"`). The `HotkeyManager` detects cross-scope conflicts. |
| Hold action | A hotkey that fires on key-down and key-up (used by rapidfire), as opposed to a normal hotkey that fires once on key-down. |
| ConflictPolicy | `Strict` (no cross-scope key reuse) or `AllowHold` (allows a hold scope to coexist with a normal scope on the same key). Timer/counter and rapidfire use AllowHold; Morse uses Strict. |
| KeySuppressor | A second `WH_KEYBOARD_LL` hook that swallows physical key events so they do not reach the foreground app, while still triggering hotkey callbacks. Lazily started. |
| ToolBase | The generic layer in `src-tauri/src/tool_base.rs` that gives every tool module shared settings/bootstrap/error handling via `ToolState<T: ToolLogic>`. |
| ToolLogic | The trait each tool implements to plug into ToolBase: `load_settings`, `save_settings`, `build_bootstrap`, `emit_state`. |
| GlobalState | A single `AtomicBool` on/off switch. When off, all hotkey callbacks and automation are suspended and running sessions are stopped. |
| Region selection | The overlay flow where the user drag-selects screen regions (Morse) or color probe areas (audio). Multi-step; uses `oneshot` channels. |
| Session | A single rapidfire activation lifecycle: key-down creates a session, key-up stops it. Each session runs on its own OS worker thread. |
| Compensation | When a rapidfire card fires an odd number of times, the "compensation" logic fires one extra key to make it even (unless the card has no-append enabled). |
| ColorWatch | An audio trigger mode that samples screen regions, takes the average RGB, and compares to a target color via Euclidean distance. |
| RegionWatch | An audio trigger mode that does normalized cross-correlation template matching against a reference image region. |
| Theme | A set of CSS variable overrides. 5 built-in themes plus user custom themes, persisted to `theme_settings.json`. Applied via inline styles on `document.documentElement`. |
| Profile | A snapshot of all 5 tool settings files. Switching profiles writes the 5 settings to disk, reloads in-memory state, and resets counter run values. |
| IDE gateway | A legacy concept from the removed Delta module. References to it in old docs do not apply to the current codebase. |
