# Debugging

## Log files

The app writes logs to `logs/delta-{yyyyMMdd}.log` (or `%LocalAppData%\org.izrino.delta-auto-tools\logs\` as fallback). Both Rust and frontend logs go to the same files. Each log line includes timestamp, level, origin (`[RUST]·{source}` or `[FE]`), file:line, trace_id, session_id, and message.

Use `log_get_session_id` to get the 6-char session ID for the current run, then grep the log file for that ID to see all activity from this session.

### Adjusting log level

Use `log_set_level` to raise verbosity. The `LogSettings` supports a global level plus per-module overrides (e.g. `"morse": "debug"`). Changes persist to `log_settings.json`.

### Frontend console hijack

In production builds, `console.log/warn/error` are wrapped to also write to the log file. In dev mode (`bun run dev`), console works normally without file logging.

## Common issues

### Hotkey not firing

1. Check the global switch is on (Top Manifest Bar, green = enabled). See [global state](../systems/global-state.md).
2. Check `hotkey_error` in the bootstrap response - if the willhook install failed, all hotkeys are disabled.
3. Check for conflicts - saving settings returns a Chinese error string if a key conflicts with another scope. See [hotkeys](../systems/hotkeys.md).
4. On Windows, ensure no antivirus or system permission is blocking the `WH_KEYBOARD_LL` hook.

### Overlay window not visible

1. Check the tool's enabled flag is on (e.g. `timer_enabled`).
2. Check the display window was created (look for the `timer-display` / `counter-display` / `rapidfire-display` label in logs).
3. Check the window position is on-screen (not dragged off-screen in position setup).
4. Transparent windows may be hard to see on dark backgrounds; the content uses chalk-colored text.

### Autosave overwrites

If settings seem to revert, the `autosaveVersionRef` may be out of sync. The autosave hook (`src/hooks/use-autosave.ts`) discards saves whose version is older than the current form version. If bootstrap is re-fetched (e.g. after a hotkey error event), the form is reset from the new bootstrap, which resets the version.

### Serde casing mismatch

If the frontend receives `undefined` for a field, check that the Rust struct uses `#[serde(rename_all = "camelCase")]` and the frontend type expects the camelCase key. This is the most common IPC bug.

### Mutex poisoning

If a command returns a "已损坏" (corrupted) error, a Mutex was poisoned by a panic during a previous lock hold. The app needs to be restarted. This should not happen in normal operation; if it does, check logs for the panic that caused it.

## Tauri dev tools

In dev mode (`bun run tauri dev`), the WebView2 devtools are available (right-click -> Inspect). The frontend state can be inspected through React DevTools if installed.
