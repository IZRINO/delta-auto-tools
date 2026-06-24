# Getting started

## Prerequisites

- Windows 10 or 11 (the app uses Windows-only keyboard hooks via `willhook` and screen capture via `xcap`; other platforms are not supported for the native features).
- [Bun](https://bun.sh/) for the frontend package manager and scripts.
- [Rust](https://rustup.rs/) toolchain (stable).
- [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) - on Windows this means the WebView2 runtime and the MSVC build tools.

## Install

```bash
bun install
```

This installs all frontend dependencies from `package.json`.

## Run

### Frontend-only dev server

```bash
bun run dev
```

Starts Vite on `http://localhost:1420` (strictPort). Useful for UI work in a browser, but all Tauri commands will be disabled (the app detects the missing `__TAURI_INTERNALS__` and shows placeholders).

### Full desktop dev

```bash
bun run tauri dev
```

Starts Vite first, then launches the Tauri window. This is the mode where native features (hotkeys, screen capture, overlays) actually work.

### PM2 orchestration

The repo has an `ecosystem.config.cjs` that splits Vite and Tauri into two PM2 processes. The Tauri process waits for port 1420 before starting.

## Build

### Frontend build

```bash
bun run build
```

Runs `tsc && vite build`, producing `dist/`.

### Desktop build (NSIS installer)

```bash
bun run tauri build
```

Produces `src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe`.

For signed releases (required for the auto-updater), set the `TAURI_SIGNING_PRIVATE_KEY` environment variable to the private key content before building, or use `scripts/build-release.ps1` which wraps the whole flow. After a signed build, run `scripts/generate-latest-json.ps1` to produce `latest.json` (the Tauri updater manifest).

## Test

### Frontend tests

```bash
bun run test              # all Vitest tests
bun run test:coverage     # with coverage (currently scoped to morse-utils.ts)
```

Run a single file:

```bash
bunx vitest run src/components/app/morse-utils.test.ts
```

### Rust tests

```bash
cargo check --manifest-path src-tauri/Cargo.toml    # compile check
cargo test --manifest-path src-tauri/Cargo.toml     # unit tests
```

Run a single test:

```bash
cargo test --manifest-path src-tauri/Cargo.toml <test_name>
```

## Version sync

When bumping the version, update all three files in lockstep: `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`. If `Cargo.lock` updates the local crate version, commit that too.
