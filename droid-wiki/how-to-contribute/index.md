# How to contribute

## Work pickup

Issues are tracked on GitHub Issues using the `gh` CLI. The project uses a five-level triage label system: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md` for label definitions.

When picking up work, look for issues labeled `ready-for-agent` or `ready-for-human`. Read the issue fully before starting, and comment that you are picking it up.

## PR process

1. Branch from `master` (the default branch).
2. Make focused commits with Chinese commit messages (per project convention).
3. Run `bun run test`, `cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` before requesting review.
4. Open a PR against `master`. Summarize what changed and why.
5. Address review feedback. Do not squash until merge is approved.

## Definition of done

- All tests pass (Vitest + cargo test).
- `cargo check` and `bun run build` (tsc + vite build) succeed with no errors.
- If you added a Tauri command, it is registered in both `src-tauri/src/lib.rs` `generate_handler![]` and `src-tauri/capabilities/default.json`.
- If you changed settings structures, serde still uses `#[serde(rename_all = "camelCase")]` and the frontend types match.
- UI changes follow the industrial-brutalist design system (no rounded cards, no soft shadows, amber accent only).

See also:
- [Development workflow](development-workflow.md)
- [Testing](testing.md)
- [Debugging](debugging.md)
- [Patterns and conventions](patterns-and-conventions.md)
- [Tooling](tooling.md)
