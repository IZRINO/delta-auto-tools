export const overlayWindowModes = new Set([
    "overlay",
    "fingerprint-overlay",
    "timer-display",
    "counter-display",
    "timer-position",
    "counter-position",
    "rapidfire-display",
    "rapidfire-position",
    "recognition-overlay",
    "special-ops-calibration",
    "special-ops-operation",
    "practical-tools-display",
    "practical-tools-auto-indicator",
    "practical-tools-region",
]);

export function isOverlayWindowMode(mode: string | null | undefined): boolean {
    return mode != null && overlayWindowModes.has(mode);
}
