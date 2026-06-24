# Tooling

## Build tools

### Vite

The frontend uses Vite 7 with `@vitejs/plugin-react`. The dev server runs on port 1420 (strictPort - if taken, it fails rather than incrementing). The config is in `vite.config.ts` (not shown in the root listing but present). Tailwind v4 is integrated via `@tailwindcss/vite`.

### TypeScript

TypeScript 5.8 with strict settings. `bun run build` runs `tsc && vite build`. Path aliases: `@/components`, `@/components/ui`, `@/lib`, `@/hooks` (configured in tsconfig and vite).

### Tailwind CSS v4

CSS-first configuration - there is no `tailwind.config.js`. Theme tokens are defined in `src/App.css` under `@theme inline`. Global `--radius: 0` for 90-degree corners.

### Bun

Bun is the package manager and script runner. Do not use npm/pnpm/yarn. `bun install` reads `bun.lock`.

### Tauri 2

Tauri CLI is available via `bun run tauri`. The config is in `src-tauri/tauri.conf.json`. Capabilities (permissions) are in `src-tauri/capabilities/default.json`.

## Rust tooling

### Cargo

`src-tauri/Cargo.toml` is the manifest. The crate is named `delta-auto-tools` with library name `delta_auto_tools_lib`. Build with `cargo check --manifest-path src-tauri/Cargo.toml`. Test with `cargo test --manifest-path src-tauri/Cargo.toml`.

### Key Rust dependencies

- `willhook` - Global keyboard hook (WH_KEYBOARD_LL)
- `xcap` - Screen capture
- `enigo` - Keyboard input simulation
- `rodio` - Audio playback
- `image` - Image processing (template matching, color sampling)
- `tauri` 2 with plugins: dialog, opener, window-state, updater, process

## PM2

`ecosystem.config.cjs` splits Vite and Tauri into two PM2 processes. The Tauri process waits for port 1420 before starting (via `scripts/wait-for-port.cjs`). Useful for keeping both running in the background during development.

## Release scripts

| Script | Purpose |
|--------|---------|
| `scripts/build-release.ps1` | One-click signed build: sets TAURI_SIGNING_PRIVATE_KEY, runs tauri build, generates .sig |
| `scripts/generate-latest-json.ps1` | Generates `latest.json` from the .sig file for the Tauri updater |
| `scripts/setup-update-key.ps1` | Generates the Tauri signing key pair (first-time setup) |
| `scripts/wait-for-port.cjs` | Waits for port 1420 to be available (used by PM2) |

## shadcn/ui

The project uses shadcn/ui (v4.11.0) with `components.json` configuration. Base components are in `src/components/ui/` (~60 components). The style is "radix-vega" with remixicon icons. To add a component, use `bunx shadcn add <component>`.

## Codegraph

The repo has a `.codegraph/` directory, indicating the codegraph MCP server can be used for symbol search and dependency exploration during development.
