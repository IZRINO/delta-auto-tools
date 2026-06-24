# Overlay windows

Timer, counter, and rapidfire each use a transparent overlay window that sits on top of the game, showing real-time data without blocking clicks. These windows are created via Tauri's WebviewWindow API with specific flags for transparency, always-on-top, and click-through.

## Window labels

| Label | Tool | Purpose |
|-------|------|---------|
| `timer-display` | Timer | Shows timer cards with countdown/countup and progress |
| `timer-position` | Timer | Calibration window for dragging the display position |
| `counter-display` | Counter | Shows counter current values |
| `counter-position` | Counter | Calibration window for position |
| `rapidfire-display` | Rapidfire | Shows trigger-to-target mappings and firing status |
| `rapidfire-position` | Rapidfire | Calibration window for position |
| `morse-overlay` | Morse | Full-screen transparent region selection overlay |
| `audio-overlay` | Audio | Region/probe selection overlay (shared with Morse flow) |

## Display windows

Display windows (`*-display`) are created with these properties:

- Transparent background (no dark paper style from the main window)
- No decorations (borderless)
- Always on top
- Skip taskbar
- Click-through (mouse events pass to the window below)
- Resizable width (timer/counter min 320px, rapidfire 320-800px)
- Height calculated from enabled card count

The frontend renders these via `?mode=*-display` query parameters in `src/App.tsx`, which early-return the overlay content instead of the main shell.

## Position windows

Position windows (`*-position`) are small calibration-style windows that the user drags to position the display window. They may use a target/crosshair visual style. Entered via `?mode=*-position`.

The drag flow:
1. Frontend calls `xxx_begin_position_selection` to open the position window.
2. User drags the window to the desired screen location.
3. `xxx_position_moved` fires during drag, storing temporary coordinates.
4. `xxx_position_commit` (Enter key) saves the coordinates to settings and closes the window.
5. `xxx_position_cancel` (Escape key) discards and closes.

## Region selection overlay

Morse and audio use a full-screen transparent overlay (`morse-overlay` / `audio-overlay`) for drag-selecting screen regions. The overlay is entered via `?mode=overlay` (Morse) or `?mode=audio-overlay` (audio). It uses `oneshot` channels to communicate completion back to the caller.

## Shared components

The frontend reuses overlay infrastructure across tools:

| Component | File | Purpose |
|-----------|------|---------|
| `SyncOverlayWindow` | `src/components/app/sync-overlay-window.tsx` | Shared display/position window wrapper for timer/counter/rapidfire |
| `MorseOverlay` / `RegionSelectionOverlay` | `src/components/app/morse-overlay.tsx` | Morse region selection full-screen overlay |
| `AudioRegionOverlay` | `src/components/app/audio-page.tsx` | Audio region/probe selection overlay |

## Integration points

- [Timer](../features/timer.md), [Counter](../features/counter.md), [Rapidfire](../features/rapidfire.md) - Each owns display and position windows.
- [Morse](../features/morse.md) - Owns the region selection overlay.
- [Audio](../features/audio.md) - Shares the overlay flow for region/probe selection.
- `src-tauri/src/overlay_utils.rs` - Shared helpers for overlay window creation and sizing.
- State changes are emitted to both `main` and the display window label so overlays update live.

## Key source files

| File | Purpose |
|------|---------|
| `src-tauri/src/overlay_utils.rs` | Shared overlay/position window creation helpers |
| `src/components/app/sync-overlay-window.tsx` | Shared frontend display/position window component |
| `src/App.tsx` | `?mode=` query parameter branching into overlay windows |
