# Lore

The story of how Delta Auto Tools evolved, derived from git history and code structure.

## Eras

### Founding era (early 2026)

The project began as a Tauri 2 + React desktop tool for the game Delta Force. The initial architecture established the three-segment industrial shell (Top Manifest Bar, Left Index Rail, Main Work Grid), the `?mode=` overlay window system, and the ToolBase generic layer. The first tools were Morse code recognition, timer, and counter - the three originally documented in CLAUDE.md.

### Tool expansion (mid 2026)

Rapidfire was added as the fourth native tool, introducing the hold-hotkey mechanism and per-session OS worker threads. This required the `ConflictPolicy::AllowHold` exception so rapidfire's hold scope could coexist with timer and counter normal scopes on the same key. The audio module followed, initially with hotkey triggering only, then expanded to RegionWatch (image template matching) and ColorWatch (RGB distance).

### Strategy browser integration

The strategy browser was originally designed with a separate `strategy-browser` window and HTML proxy rendering via `strategy_fetch_page`. This was later replaced by an embedded WebView2 sub-window (`strategy-content`) inside the main window, which provides real browser rendering without proxying. The `strategy_fetch_page` and `strategy_open_window` commands remain as compat/experimental entries.

### Delta module era and removal

The project once included a large `delta/` Rust module (visible throughout AGENTS.md and CLAUDE.md) providing QQ/WeChat/Wegame/Pioneer authentication, game data via the Tencent IDE gateway, SQLite account storage with DPAPI encryption, and corresponding frontend pages (accounts, game data, toolbox). This module was removed from the codebase. `lib.rs` no longer imports or initializes `delta`, the `generate_handler![]` has no delta commands, and `App.tsx` has no delta tool entries. The `deltaTools` array in App.tsx now only contains `morse`. AGENTS.md and CLAUDE.md have not been fully updated to reflect this removal, which is why they still describe delta commands and frontend pages in detail.

### Theme and profile systems (June 2026)

Two cross-cutting systems were added: a theme engine (`src-tauri/src/theme/`) with 5 built-in themes, custom themes, and CSS variable overrides, and a multi-config profile system (`src-tauri/src/profile/`) that snapshots all tool settings. The settings dialog was unified into a three-tab Dialog (theme / config / about).

### Audio ColorWatch expansion (June 17, 2026)

Based on the plan in `docs/superpowers/plans/2026-06-17-audio-color-watch.md`, the audio module gained the ColorWatch trigger mode with multi-probe color matching, the `AnyPixel` match method, and per-probe multi-target color support (Issue #65). This is documented in the specs under `docs/superpowers/`.

### Rapidfire redesign (June 20, 2026)

A significant rapidfire redesign (`docs/superpowers/plans/2026-06-20-连发器卡片级配置-redesign.md`) moved interval, jitter, and no-append settings from global to per-card fields, with old global fields serving only as deserialization defaults. The key suppressor was added to support `ignore_trigger_key`.

### Profile switcher (June 22, 2026)

The profile system was finalized per `docs/superpowers/plans/2026-06-22-profile-switcher.md`, including the cross-tool apply orchestration and counter run-value reset.

## Longest-standing features

The Morse recognition pipeline (`src-tauri/src/morse/recognition.rs`) and the hotkey system (`src-tauri/src/hotkeys.rs`) are the oldest and most central components. The binary `HotkeyManager` with its single willhook hook and scope-based registration has survived every refactor and is still the backbone of all native automation.

## Deprecated features

- **Delta module** (accounts, game data, toolbox, QQ/WeChat/Wegame/Pioneer auth, IDE gateway, SQLite storage, DPAPI encryption) - Removed. The `src-tauri/src/delta/` directory no longer exists in the active codebase. AGENTS.md still documents it extensively.
- **`strategy-browser` window** - Replaced by the embedded `strategy-content` sub-WebView.
- **`timer/counter_state.rs` in `timer/`** - Marked as deprecated; counter logic migrated to `counter/counter_state.rs`.
- **Global rapidfire settings** (`min_press_spacing_ms`, `trigger_jitter_max_ms`, `cancel_jitter_on_release` at the settings level) - Superseded by per-card fields; old settings-level fields are deserialization defaults only.
- **Single-value `audioFilePath`** on AudioCard - Migrated to `audio_files` array by `normalize_settings`.
- **Single-value `targetColor`/`tolerance`** on ColorProbe - Migrated to `targets` array (Issue #65).

## Growth trajectory

The project grew from 3 tools (morse, timer, counter) to 7 (adding rapidfire, audio, strategy) plus cross-cutting systems (theme, profile, logging, about/updater). The frontend shadcn/ui component library grew to ~60 base components. The codebase is approximately 37,000 lines across ~184 files, developed over 219 commits by a small team with AI assistance.
