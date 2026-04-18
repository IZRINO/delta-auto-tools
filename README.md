# delta-auto-tools

Desktop app scaffold built with **Tauri 2 + React 19 + TypeScript + Vite + Bun**.

The repo now has **Tailwind CSS v4** and **shadcn/ui** wired in, but the current runtime screen in `src/App.tsx` is still the simple Tauri greet demo. The design system is installed; the main app has not been fully migrated to it yet.

## Stack and verified repo markers

- Tauri 2 desktop shell in `src-tauri/`
- React 19 + TypeScript + Vite frontend
- Bun-driven dev/build flow
- Tailwind v4 via `@tailwindcss/vite`
- shadcn/ui via root `components.json`
- `@/*` alias wired in both `vite.config.ts` and `tsconfig.json`
- shadcn-style primitives under `src/components/ui/*`
- `cn(...)` helper in `src/lib/utils.ts`
- theme tokens and Tailwind imports in `src/App.css`

## Commands

```bash
bun run dev
bun run build
bun run preview
bun run tauri dev
bun run tauri build
```

## Current project shape

- Frontend entry: `index.html` -> `src/main.tsx` -> `src/App.tsx`
- Native entry: `src-tauri/src/main.rs` -> `src-tauri/src/lib.rs`
- Current Tauri command surface is still minimal (`greet`)
- UI primitives are available under `src/components/ui/*`
- Repo-local skills are mirrored in both `.claude/skills/` and `.agents/skills/`:
  - `expo-tailwind-setup`
  - `shadcn`
  - `tailwind-design-system`
  - `tauri-v2`

## Notes

- Vite dev server is fixed at `http://localhost:1420`
- When `TAURI_DEV_HOST` is set, HMR uses port `1421`
- `src/App.css` currently contains both the new Tailwind/shadcn token layer and leftover template demo styles
- There is no `docs/` directory or GitHub Actions workflow in the repo right now
- See `AGENTS.md` for repo-specific editing and maintenance rules

## Recommended editor setup

- VS Code
- `tauri-apps.tauri-vscode`
- `rust-lang.rust-analyzer`
