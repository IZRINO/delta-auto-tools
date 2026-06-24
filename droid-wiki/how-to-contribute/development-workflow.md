# Development workflow

## Branch and code cycle

1. `git checkout master && git pull` to start from the latest.
2. Create a feature branch: `git checkout -b <topic>`.
3. Code your change. Follow the [patterns and conventions](patterns-and-conventions.md).
4. Test locally:
   - `bun run test` for frontend Vitest tests.
   - `cargo check --manifest-path src-tauri/Cargo.toml` for Rust compile check.
   - `cargo test --manifest-path src-tauri/Cargo.toml` for Rust unit tests.
5. For UI work, `bun run dev` gives a browser-only preview (native commands disabled). For full integration testing, `bun run tauri dev`.

## Tauri command checklist

When adding or changing a Tauri command, update all three:

1. Define the `#[tauri::command]` function in the module.
2. Register it in `src-tauri/src/lib.rs` under `generate_handler![]` (grouped by module comment).
3. Add the permission in `src-tauri/capabilities/default.json`.

Missing any of these causes the frontend `invoke()` to fail at runtime.

## Settings changes

When modifying a tool's settings struct:

1. Add the field with `#[serde(default = "fn")]` for backward-compatible deserialization.
2. Use `#[serde(rename_all = "camelCase")]` on the struct (required for all frontend-facing structs).
3. Update the frontend types in the corresponding `*-types.ts`.
4. Update the `settingsToForm()` / `parseSettingsForm()` conversion in `*-utils.ts`.
5. If the field needs to trigger hotkey re-registration or watcher restart, handle it in the module's `save_settings` handler.

## Commit conventions

- Commit messages in Chinese (per AGENTS.md).
- For version releases, the subject is `发布 v<version>` and the body must include a `变更：` section with actual change items. See the [deployment](../deployment.md) page for the full release process.

## Version sync

When bumping the version, update all three in lockstep: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`. If `Cargo.lock` updates the local crate version, commit that too.
