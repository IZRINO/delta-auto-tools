# AGENTS.md

## Project reality

- Verified stack: **Tauri 2 + React + TypeScript + Vite + Bun**.
- Repo-local evidence does **not** currently support shadcn/ui, Tailwind, React Router, test tooling, lint/format tooling, or CI workflows.
- `README.md` is still template-level prose. Treat executable config and live code as the source of truth.

## Source of truth order

1. `src-tauri/tauri.conf.json`
2. `package.json`
3. `vite.config.ts`
4. `tsconfig.json`
5. `src-tauri/Cargo.toml`
6. `src/` and `src-tauri/src/`

If prose disagrees with config or code, follow config/code.

## Commands

- `bun run dev` -> frontend-only Vite dev server
- `bun run build` -> `tsc && vite build`
- `bun run preview` -> Vite preview
- `bun run tauri dev` -> preferred full desktop dev flow; Tauri auto-runs `bun run dev`
- `bun run tauri build` -> preferred desktop build flow; Tauri auto-runs `bun run build`

## Runtime and build constraints

- Bun is the actual package manager here (`bun.lock` + Tauri build hooks), even though `package.json` has no `packageManager` field.
- Vite dev server is pinned to **`http://localhost:1420`** with `strictPort: true`.
- When `TAURI_DEV_HOST` is set, HMR uses port **1421**.
- Tauri production frontend output is **`../dist`**.
- `security.csp` is currently **`null`** in `src-tauri/tauri.conf.json`; do not assume CSP protection is configured.

## Current architecture

- Frontend entry chain: `index.html` -> `src/main.tsx` -> `src/App.tsx`
- Native entry chain: `src-tauri/src/main.rs` -> `src-tauri/src/lib.rs`
- Current Tauri command surface is minimal: `greet` in `src-tauri/src/lib.rs`
- Current native plugin surface is minimal: `tauri-plugin-opener`
- Current capability surface is `src-tauri/capabilities/default.json` with `core:default` and `opener:default`

The frontend is still template-small. Under `src/`, the real app surface is only:

- `src/main.tsx`
- `src/App.tsx`
- `src/App.css`
- `src/assets/react.svg`
- `src/vite-env.d.ts`

There is no repo-local evidence of routes, providers, feature folders, shared UI primitives, or a broader app shell.

## Change rules

- Use **Bun**, not npm/pnpm/yarn.
- Keep changes aligned with the current small-template structure unless the task explicitly expands architecture.
- Do not assume router setup, provider layers, `@/` aliases, or `src/components/ui` exist.
- Styling is currently plain CSS in `src/App.css`; do not write Tailwind/shadcn-specific code unless that tooling is added first.
- TypeScript is strict: `strict`, `noUnusedLocals`, `noUnusedParameters`, and `noFallthroughCasesInSwitch` are enabled.
- There is an existing `@ts-expect-error` comment in `vite.config.ts`; treat it as legacy config debt, not a pattern to copy.
- When adding native features, update Tauri config/capabilities intentionally rather than only changing frontend code.

## Repo-specific cautions

- `README.md`, `index.html` title, and some metadata still look like template defaults. Verify against config/code before reusing them in docs or product copy.
- Do not invent commands for linting, testing, formatting, or CI. No repo-local config for those workflows exists yet.
- Ignore generated or dependency artifacts: `node_modules`, `dist`, `dist-ssr`, `src-tauri/target`, and `src-tauri/gen/schemas`.
- Recommended editor extensions in `.vscode/extensions.json`: `tauri-apps.tauri-vscode`, `rust-lang.rust-analyzer`.

## If the project grows

- If you add routing, shared UI layers, tests, linting, Tailwind, shadcn/ui, or more Rust commands, update this file in the same change.
- If shadcn/ui is introduced later, add real repo markers first (for example `components.json`, alias wiring, and styling setup) before documenting shadcn-specific conventions.
