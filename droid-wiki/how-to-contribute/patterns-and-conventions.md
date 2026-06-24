# Patterns and conventions

## Rust serde

Every Rust struct that the frontend deserializes must use `#[serde(rename_all = "camelCase")]`. This applies to settings, bootstrap, run-state, events, and DTOs. Mismatched casing between Rust and TypeScript is the most common IPC bug. The frontend expects camelCase keys (e.g. `startValue`, not `start_value`).

## Error handling

Two distinct error conventions:

- Morse, timer, counter, rapidfire, audio, strategy commands return `Result<T, String>` where the string is a Chinese error message (e.g. `"摩斯状态已损坏"`).
- The removed Delta module returned `Result<ApiResponse<T>, DeltaError>`. This pattern is no longer in the active codebase.

When a Mutex is poisoned, tools return a Chinese "已损坏" (corrupted) error. The `ToolState::lock_inner` helper centralizes this.

## Bootstrap/form dual-state

Container pages (`morse-page.tsx`, `timer-page.tsx`, etc.) hold both a `bootstrap` state (from Rust, immutable) and a `form` state (local draft). The two are compared with `JSON.stringify` to detect dirty state. The `useBootstrapFormLogic` hook (`src/hooks/use-bootstrap-form-logic.ts`) encapsulates this. When the form diverges, a 400ms debounced autosave fires via `useAutosave` (`src/hooks/use-autosave.ts`), which calls the tool's `xxx_save_settings` command.

An `autosaveVersionRef` counter prevents a stale save from overwriting a newer form: each save request carries the version it was queued with, and the save is discarded if the current version is higher.

## Settings/form conversion layer

Because form inputs use strings and Rust uses integers, each tool has a conversion layer in its `*-utils.ts`:

- `settingsToForm()` - int -> string for input fields.
- `parseSettingsForm()` - validates and converts string -> int for Rust.

This keeps validation out of the rendering layer and lets the utils be unit-tested in isolation.

## Event naming

Event names are string constants. Backend defines them in `events.rs` files (`src-tauri/src/<tool>/events.rs`). Frontend mirrors them in `src/lib/tauri-events.ts` as typed objects (`MORSE_EVENTS`, `TIMER_EVENTS`, etc.) and a `listenEvent<T>` helper. Never hardcode event name strings in either layer.

## Native shell detection

`useNativeShell()` (`src/hooks/use-native-shell.ts`) checks `__TAURI_INTERNALS__`. Browser preview mode disables all `invoke()` calls and shows a notice. Every tool page that calls a Tauri command should guard with this hook so the page can render in a plain browser for UI work.

## Overlay windows

Timer, counter, and rapidfire overlays must be borderless, transparent, always-on-top, and click-through. The position-setup windows (`?mode=*-position`) may look like calibration targets. Overlay backgrounds must stay transparent so the game is visible. Do not apply the main window's dark paper style to overlays.

## Hotkey conflict policy

- Morse uses `ConflictPolicy::Strict` - no key reuse across any scope.
- Timer and counter use `ConflictPolicy::AllowHold` - they can share keys with rapidfire's hold scope.
- Rapidfire uses `ConflictPolicy::AllowHold` for its hold scope.

At runtime, hold Down/Up events are dispatched first, then normal hotkey events, so the same key can trigger both a rapidfire session and a timer/counter.

## Styling rules

- Only shadcn/ui components, Tailwind utility classes, and the `src/App.css` theme tokens. No custom `.desktop-*` or `.tactical-*` CSS classes.
- Global `--radius: 0` (90-degree corners). No rounded cards, soft shadows, glassmorphism, or gradients in the main window.
- Amber (`#E8A000`) is the single accent color and should occupy 3-8% of the screen. Status colors (Rust, Moss) are semantic only.
- Icons use `@remixicon/react`. Button icons must set `data-icon="inline-start"` or `"inline-end"`.

## File references in docs

When mentioning a source file, always use the full path from the repo root (e.g. `src-tauri/src/morse/mod.rs`, not `mod.rs`). Short filenames produce broken links in rendered documentation.
