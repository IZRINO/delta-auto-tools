# Dependencies

## Rust dependencies (`src-tauri/Cargo.toml`)

### Core framework

| Crate | Version | Purpose |
|-------|---------|---------|
| `tauri` | 2.11.2 | Desktop application framework (`unstable` feature) |
| `tauri-plugin-dialog` | 2 | File/message dialogs |
| `tauri-plugin-opener` | 2.5.4 | Open URLs/files in system apps |
| `tauri-plugin-window-state` | 2.4.1 | Persist window state |
| `tauri-plugin-updater` | 2 | Auto-update via GitHub Releases |
| `tauri-plugin-process` | 2 | Process control (relaunch after update) |

### Native automation

| Crate | Version | Purpose |
|-------|---------|---------|
| `willhook` | 0.6.3 | Global keyboard hook (WH_KEYBOARD_LL) |
| `xcap` | 0.9.6 | Screen capture |
| `enigo` | 0.6.1 | Simulated keyboard input |
| `rodio` | 0.20 | Audio playback |
| `image` | 0.25.10 | Image processing (template matching, color sampling) |
| `crossbeam-channel` | 0.5 | Channel for key suppressor event forwarding |

### Async and serialization

| Crate | Version | Purpose |
|-------|---------|---------|
| `tokio` | 1.52.3 | Async runtime (macros, rt-multi-thread, sync, time) |
| `serde` | 1.0.228 | Serialization (derive) |
| `serde_json` | 1.0.150 | JSON handling |
| `thiserror` | 2.0.18 | Error derive macros |
| `chrono` | 0.4 | Date/time with serde |
| `regex` | 1.12.4 | Regex matching |
| `url` | 2.5.8 | URL parsing |

### System

| Crate | Version | Purpose |
|-------|---------|---------|
| `windows-sys` | 0.61 | Windows API bindings (Foundation, UI, System) |

### Dev

| Crate | Version | Purpose |
|-------|---------|---------|
| `tempfile` | 3.27.0 | Temp directories for tests |

## Frontend dependencies (`package.json`)

### Core

| Package | Version | Purpose |
|---------|---------|---------|
| `react` | 19.2.7 | UI library |
| `react-dom` | 19.2.7 | React DOM renderer |
| `@tauri-apps/api` | 2.11.0 | Tauri frontend API (invoke, events) |
| `@tauri-apps/plugin-updater` | ^2.10.1 | Updater frontend |
| `@tauri-apps/plugin-process` | ^2.3.1 | Process control (relaunch) |
| `@tauri-apps/plugin-dialog` | ^2.0.0 | Dialogs |
| `@tauri-apps/plugin-opener` | 2.5.4 | Open external URLs |

### UI

| Package | Version | Purpose |
|---------|---------|---------|
| `radix-ui` | 1.5.0 | Primitives for shadcn/ui |
| `@base-ui/react` | 1.5.0 | Additional primitives |
| `shadcn` | 4.11.0 | Component system |
| `@remixicon/react` | ^4.9.0 | Icon library |
| `class-variance-authority` | ^0.7.1 | Variant styling |
| `clsx` | ^2.1.1 | Class merging |
| `tailwind-merge` | 3.6.0 | Tailwind class dedup |
| `react-colorful` | ^5.7.0 | Color picker (approved exception) |
| `sonner` | ^2.0.7 | Toast notifications |
| `vaul` | ^1.1.2 | Drawer component |

### Build

| Package | Version | Purpose |
|---------|---------|---------|
| `vite` | 7.3.5 | Build tool |
| `@vitejs/plugin-react` | 4.7.0 | React plugin |
| `tailwindcss` | 4.3.1 | CSS framework (v4, CSS-first) |
| `@tailwindcss/vite` | 4.3.1 | Tailwind Vite plugin |
| `typescript` | ~5.8.3 | Type checking |
| `vitest` | 3.2.6 | Test runner |
| `@vitest/coverage-v8` | 3.2.6 | Test coverage |

### Other

| Package | Version | Purpose |
|---------|---------|---------|
| `cmdk` | ^1.1.1 | Command palette |
| `date-fns` | 4.4.0 | Date utilities |
| `recharts` | 3.8.1 | Charts |
| `next-themes` | ^0.4.6 | Theme provider |
| `react-resizable-panels` | 4.11.2 | Resizable panels |
| `input-otp` | ^1.4.2 | OTP input |
| `react-day-picker` | ^9.14.0 | Date picker |
| `embla-carousel-react` | ^8.6.0 | Carousel |

## Dependency notes

- The project uses Bun as package manager (`bun.lock`), not npm/pnpm/yarn.
- `react-colorful` (~3KB) is the only approved third-party color picker; shadcn's official color-picker is not used.
- Tailwind v4 is CSS-first: there is no `tailwind.config.js`. Theme tokens live in `src/App.css`.
- The `devDependencies` do not include ESLint or Prettier; code style is enforced by convention and review.
