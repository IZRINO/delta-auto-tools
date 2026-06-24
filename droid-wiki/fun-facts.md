# Fun facts

## The empty resource files

`src-tauri/src/delta/resources/ammo.json` and `accessory.json` are both empty arrays `[]`. AGENTS.md explicitly warns: "these are NOT used; the actual ammo/accessory data comes from `game_config.rs` Rust constants." These files are ghosts of the delta module that was removed, yet they persist in the repo as empty placeholders. If you try to read game config from them, you will get nothing.

## The pubkey that was censored

In `src-tauri/tauri.conf.json`, the `plugins.updater.pubkey` field appears partially censored with asterisks. This is a public key (safe to share) used to verify update signatures. It must be present for the auto-updater to work; if it is empty or missing, the about panel falls back to "open GitHub Release page" mode.

## The comment that outlived its module

AGENTS.md is over 80,000 characters long and dedicates enormous space to the delta module: 6 authentication flows, SQLite storage, DPAPI encryption, IDE gateway game data, capability enums, and command tables. None of this exists in the current codebase. The documentation is a fossil record of a feature that was built, used, and then removed, while the docs were never pruned.

## The counter that outlived the timer

`src-tauri/src/timer/counter_state.rs` is marked "已废弃" (deprecated). Counter logic was moved to its own `counter/` module, but the file still sits inside `timer/` as a tombstone. The real counter run-state persistence lives at `src-tauri/src/counter/counter_state.rs`.

## Windows-only, no fallbacks

The hotkey system does not degrade on non-Windows platforms. The `#[cfg(not(target_os = "windows"))]` branches simply return error strings like "当前仅 Windows 桌面环境支持被动热键监听". There is no macOS or Linux path. The app is unapologetically Windows-first because the game (Delta Force) and its keyboard hooks (`willhook`'s `WH_KEYBOARD_LL`) are Windows-specific.

## The three Chinese words in Rust errors

Every poisoned-mutex error across all tools uses the same pattern: `format!("{}状态已损坏", T::NAME)` where `NAME` is the tool's Chinese name ("摩斯", "计时器", etc.). The result is "摩斯状态已损坏" (Morse state corrupted). This is one of the few places where Chinese appears in Rust source code rather than UI strings.
