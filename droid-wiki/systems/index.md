# Systems

Cross-cutting Rust infrastructure shared across all tool modules. These are not user-facing features but the architectural building blocks that every tool depends on.

- [Tool base](tool-base.md) - Generic `ToolState<T>` layer for settings, bootstrap, and error handling
- [Hotkeys](hotkeys.md) - Shared `HotkeyManager` with willhook keyboard hook, scope registration, and conflict detection
- [Key suppressor](key-suppressor.md) - WH_KEYBOARD_LL hook to swallow physical key events while preserving hotkey callbacks
- [Overlay windows](overlay-windows.md) - Transparent, click-through, always-on-top window infrastructure
- [Global state](global-state.md) - Single on/off switch that suspends all automation
- [Logging](logging.md) - File-based logger with rotation, session IDs, and trace context
- [Theme engine](theme-engine.md) - 5 built-in themes plus custom themes and CSS variable overrides
- [Profile system](profile-system.md) - Multi-config snapshots of all tool settings
