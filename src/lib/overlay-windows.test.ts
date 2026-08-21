import {describe, expect, it} from "vitest";

import {isOverlayWindowMode, overlayWindowModes} from "@/lib/overlay-windows";

describe("overlay window modes", () => {
    it("锁住生产 overlay 名单，避免与 ThemeProvider 漂移", () => {
        expect([...overlayWindowModes].sort()).toEqual([
            "counter-display",
            "counter-position",
            "overlay",
            "rapidfire-display",
            "rapidfire-position",
            "recognition-overlay",
            "special-ops-calibration",
            "special-ops-operation",
            "timer-display",
            "timer-position",
        ]);
    });

    it("空与未知 mode 不是 overlay", () => {
        expect(isOverlayWindowMode(null)).toBe(false);
        expect(isOverlayWindowMode(undefined)).toBe(false);
        expect(isOverlayWindowMode("")).toBe(false);
        expect(isOverlayWindowMode("timer")).toBe(false);
        expect(isOverlayWindowMode("timer-display")).toBe(true);
    });
});
