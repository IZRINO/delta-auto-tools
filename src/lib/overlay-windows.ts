export const overlayWindowModes = new Set([
    "overlay",
    "timer-display",
    "counter-display",
    "timer-position",
    "counter-position",
    "rapidfire-display",
    "rapidfire-position",
    "recognition-overlay",
    "special-ops-calibration",
    "special-ops-operation",
]);

export function isOverlayWindowMode(mode: string | null | undefined): boolean {
    return mode != null && overlayWindowModes.has(mode);
}
