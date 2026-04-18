# AGENTS.md

## Project reality

- Verified stack: **Tauri 2 + React 19 + TypeScript + Vite + Bun**.
- Tailwind CSS v4 and shadcn/ui are now wired in. Repo markers include root `components.json`, `@tailwindcss/vite` in `vite.config.ts`, `@/*` alias wiring in Vite and TypeScript, `src/components/ui/*`, `src/lib/utils.ts`, and Tailwind/shadcn imports plus theme tokens in `src/App.css`.
- The runtime app is still the simple Tauri greet demo in `src/App.tsx`. Do not confuse “UI system installed” with “app already migrated to shadcn/ui”.
- Repo-local skills exist in both `.claude/skills/` and `.agents/skills/`: `expo-tailwind-setup`, `shadcn`, `tailwind-design-system`, and `tauri-v2`.

## Source of truth

Prefer executable config and live code over prose:

1. `src-tauri/tauri.conf.json`
2. `package.json`
3. `components.json`
4. `vite.config.ts`
5. `tsconfig.json`
6. `src/` and `src-tauri/src/`

If `README.md` or older notes disagree, follow config/code.

## Commands

- `bun run dev` -> frontend-only Vite dev server
- `bun run build` -> `tsc && vite build`
- `bun run preview` -> Vite preview
- `bun run tauri dev` -> full desktop dev flow; Tauri auto-runs `bun run dev`
- `bun run tauri build` -> desktop build flow; Tauri auto-runs `bun run build`

## Current architecture

- Frontend entry chain: `index.html` -> `src/main.tsx` -> `src/App.tsx`
- Native entry chain: `src-tauri/src/main.rs` -> `src-tauri/src/lib.rs`
- Current Tauri command surface is still minimal: `greet`
- Current native plugin/capability surface is still minimal: `tauri-plugin-opener` plus `src-tauri/capabilities/default.json`
- Frontend source now includes:
  - `src/components/ui/*` for installed shadcn-style primitives
  - `src/lib/utils.ts` for `cn(...)`
  - `src/hooks/use-mobile.ts`
  - `src/App.tsx` and `src/App.css` for the still-template demo surface
- There is still no repo-local evidence of routing, a provider tree, feature modules, CI workflows, or a `docs/` directory.

## Tailwind and shadcn/ui conventions

- Tailwind is configured through `@tailwindcss/vite` plus CSS-first v4 setup in `src/App.css`; no `tailwind.config.*` is required here.
- shadcn project config lives in `components.json`.
- Use the configured aliases: `@/components`, `@/components/ui`, `@/lib`, `@/hooks`.
- Reuse existing UI primitives before adding custom markup.
- Keep `cn(...)` in `src/lib/utils.ts` as the class merge helper.
- If you migrate screens from the greet demo to shadcn/ui, do it intentionally instead of layering more template CSS on top of the new token system.

## Change rules

- Use **Bun**, not npm/pnpm/yarn.
- Keep Vite/Tauri dev-port assumptions intact: frontend on `http://localhost:1420`, HMR on `1421` when `TAURI_DEV_HOST` is set.
- Do not reintroduce old assumptions that Tailwind or shadcn/ui are absent; that guidance is outdated for this repo.
- Do not assume routing or app-wide providers exist just because the UI system is installed.
- When adding native features, update Tauri config and capabilities intentionally, not just frontend code.
- `src/App.css` now carries both the old demo styling and the new design-token layer. Be deliberate when removing or migrating legacy styles.

## Repo-specific cautions

- `README.md` and `AGENTS.md` should stay aligned with the current setup; both were previously stale.
- `README.md` and `src/App.tsx` still reflect a template-era app experience even though the UI/tooling surface has grown.
- Do not invent lint, test, formatter, or CI commands. No repo-local config for those workflows has been found yet.
- Ignore generated or dependency artifacts such as `node_modules`, `dist`, `dist-ssr`, `src-tauri/target`, and `src-tauri/gen`.
- Recommended editor extensions in `.vscode/extensions.json`: `tauri-apps.tauri-vscode`, `rust-lang.rust-analyzer`.

## If the project grows

- If you add routing, a real app shell, tests, linting, CI, or broader native command surfaces, update this file in the same change.
- If `src/App.tsx` is replaced with a real shadcn-based application shell, update both `AGENTS.md` and `README.md` so they stop describing the greet demo as the current runtime surface.
