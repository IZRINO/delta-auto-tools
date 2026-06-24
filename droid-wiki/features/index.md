# Features

The native desktop capabilities of Delta Auto Tools, each backed by a Rust module under `src-tauri/src/` and a React page under `src/components/app/`. All features share the `ToolBase` generic state layer and the `HotkeyManager` keyboard hook.

- **[Morse code recognition](./morse.md)** — Screen-capture Morse decoder: captures 3 regions, binarizes, detects contours, decodes Morse to digits 0-9, and auto-types the 3-digit result with an optional click chain.
- **Timer** — Multi-timer board with a 250ms tick loop, countdown/countup direction, transparent always-on-top overlay window, and per-card hotkey triggering.
- **Counter** — Multi-counter board with independent run-state persistence (`counter_state.json`), transparent overlay window, and per-card increment/reset hotkeys.
- **Rapidfire** — Hold-trigger key automation: per-card jitter/spacing/no-append policies, OS worker threads per session, and a transparent overlay showing ARMED/FIRING state.
- **Audio** — Audio cards with three trigger modes (Hotkey, RegionWatch image template NCC, ColorWatch RGB distance), rodio playback worker, and overlay region selection.
- **Strategy** — Embedded WebView2 guide-website workbench inside the main window: site tabs, custom sites, auto-refresh tiers, and a compat HTTP fetcher with JS-redirect following.
- **About** — About panel (version/license/dependency credits) plus the Tauri official updater with check/download/install and progress events.
