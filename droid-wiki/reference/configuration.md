# Configuration

## Settings files

All settings are JSON files stored in the Tauri app config directory. Each tool has its own file.

| File | Tool | Contents |
|------|------|----------|
| `morse_settings.json` | Morse | Hotkey, regions, auto-type, auto-click chain settings |
| `timer_settings.json` | Timer | `timer_enabled`, timers array (duration, direction, hotkey), display settings |
| `counter_settings.json` | Counter | `counter_enabled`, counters array (start_value, hotkey), display settings |
| `rapidfire_settings.json` | Rapidfire | `rapidfire_enabled`, cards array (trigger, target, interval, jitter, spacing, no-append), compensation delay |
| `audio_settings.json` | Audio | `audio_enabled`, cards array (trigger mode, files, volume, cooldown, probes) |
| `theme_settings.json` | Theme | `activeThemeId`, custom themes, token overrides |
| `profile_settings.json` | Profile | `profiles` array, `activeProfileId` |
| `counter_state.json` | Counter (runtime) | Accumulated counter values (separate from config) |
| `log_settings.json` | Logging | Global log level, per-module overrides |

## Tauri configuration

`src-tauri/tauri.conf.json` contains:

- `productName`: `delta-auto-tools`
- `identifier`: `org.izrino.delta-auto-tools`
- Window: 1280x800, min 1280x800
- Bundle target: `nsis`
- Updater: GitHub Releases endpoint, `installMode: "passive"`, `pubkey` (public signing key)
- CSP: null (no content security policy restriction)
- `createUpdaterArtifacts: true` (generates .sig files on build)

## Capabilities

`src-tauri/capabilities/default.json` defines which Tauri commands the frontend may invoke. Every new command must be added here or `invoke()` will fail.

## Environment variables

| Variable | Purpose |
|----------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | Private key content for signing stable builds (required for .sig generation) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Optional password for the signing key |
| `TAURI_SIGNING_PRIVATE_KEY_PATH` | Alternative: path to key file instead of content |
| `HTTP_PROXY` / `HTTPS_PROXY` | Local proxy for git push / gh release when GitHub is slow |

## Theme CSS tokens

Defined in `src/App.css` under `@theme inline` and `:root`:

| Token | Color | Role |
|-------|-------|------|
| `--carbon` | `#0C0C0B` | Main background |
| `--slate` | `#171715` | Secondary panel |
| `--iron` | `#232320` | Card surface |
| `--chalk` | `#D8D4CC` | Primary text, borders |
| `--zinc` | `#807C74` | Secondary text |
| `--dust` | `#545250` | Meta info |
| `--seam` | `#2A2926` | Grid lines |
| `--amber` | `#E8A000` | Single accent color |
| `--rust` | `#C85400` | Warning/danger |
| `--moss` | `#3F8A30` | Success/valid |
| `--void` | `#050504` | Data wells, JSON display |
| `--alert-red` | `#E11919` | Current selection, danger (light theme variant) |

Global `--radius: 0` enforces 90-degree corners.

## PM2

`ecosystem.config.cjs` defines two processes: `delta-auto-tools-vite` (Vite dev server) and `delta-auto-tools-tauri` (Tauri dev). The Tauri process waits for port 1420 via `scripts/wait-for-port.cjs`.
