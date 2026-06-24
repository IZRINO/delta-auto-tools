# Delta Auto Tools

Delta Auto Tools is a Tauri 2 desktop application for players of the game Delta Force. It bundles several native Windows automation tools into a single industrial-brutalist interface: Morse code recognition from screen captures, a multi-timer board, a multi-counter board, a rapid-fire key automation module, an audio playback tool triggered by hotkeys or screen region matching, and an embedded guide-website workbench.

The app is built with React 19 and TypeScript on the frontend and Rust on the backend, glued together by Tauri 2. The UI follows a "Swiss Industrial Print x Declassified Tactical Control Board" dark theme with 90-degree corners, chalk structural lines, and a single amber accent color. It targets Windows as its primary platform.

## Who uses it

Players of Delta Force who want in-game assists: timed ability cooldowns, kill counters, auto-fire macros, screen-based Morse puzzle solving, and quick access to community strategy sites. All overlays are transparent, always-on-top, and click-through so they do not block gameplay.

## Quick links

- [Architecture](architecture.md) - how the Tauri + React layers fit together
- [Getting started](getting-started.md) - prerequisites, build, test, run
- [Glossary](glossary.md) - project-specific terms
- [Features](../features/index.md) - the seven tool modules
- [Systems](../systems/index.md) - cross-cutting Rust infrastructure
