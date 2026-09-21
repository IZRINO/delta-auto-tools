import {describe, expect, it} from "vitest";

import type {FingerprintSettings} from "@/components/app/fingerprint-types";
import {
    layoutSlotCount,
    parseOverlaySlots,
    parseOverlayTarget,
    parseSettingsForm,
    personFingerprintCount,
    settingsToForm,
} from "@/components/app/fingerprint-utils";

const sample: FingerprintSettings = {
    hotkey: "F6",
    nameRegion: null,
    candidateBoxes: Array.from({length: 9}, () => null),
    archiveSlots: Array.from({length: 8}, () => null),
    occupancyThreshold: 80,
    matchThreshold: 0.55,
    autoClickEnabled: true,
    clickDelayMs: 50,
    people: [],
};

describe("fingerprint utils", () => {
    it("form round trip keeps layout and people", () => {
        const current = {
            ...sample,
            people: [{id: "p1", name: "克莱尔", nameImagePath: "n.png", fingerprintPaths: Array(8).fill(null)}],
        };
        const parsed = parseSettingsForm(settingsToForm(current));
        expect(parsed.hotkey).toBe("F6");
        expect(parsed.people).toEqual(current.people);
        expect(parsed.autoClickEnabled).toBe(true);
    });

    it("rejects bad match threshold", () => {
        expect(() => parseSettingsForm({
            ...settingsToForm(sample),
            matchThreshold: "1.2",
        })).toThrow(/匹配阈值/);
    });

    it("parses overlay target and slots", () => {
        expect(parseOverlayTarget("?mode=fingerprint-overlay&target=candidates&slots=0,8")).toBe("candidates");
        expect(parseOverlaySlots("?target=candidates&slots=0,8")).toEqual([0, 8]);
        expect(parseOverlaySlots("?target=archive&slots=0,1,1,9")).toEqual([0, 1]);
        expect(layoutSlotCount("archive")).toBe(8);
    });

    it("counts captured fingerprints", () => {
        expect(personFingerprintCount([null, "a.png", null, "b.png"])).toBe(2);
        expect(personFingerprintCount(undefined)).toBe(0);
    });
});
