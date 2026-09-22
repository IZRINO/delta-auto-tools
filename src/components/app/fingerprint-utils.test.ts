import {describe, expect, it} from "vitest";

import type {FingerprintSettings} from "@/components/app/fingerprint-types";
import {
    archiveSlotsReady,
    layoutReadyForRun,
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
    afterClickHotkey: null,
    clickRegions: [],
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
        expect(parsed.afterClickHotkey).toBeNull();
        expect(parsed.clickRegions).toEqual([]);
    });

    it("keeps after-click hotkey and click regions", () => {
        const current = {
            ...sample,
            afterClickHotkey: " F4 ",
            clickRegions: [{rect: {x: 1, y: 2, width: 10, height: 12}, delayMs: 400}],
        };
        const form = settingsToForm(current);
        expect(form.clickRegions).toHaveLength(7);
        expect(form.afterClickHotkey).toBe(" F4 ");
        const parsed = parseSettingsForm({
            ...form,
            afterClickHotkey: "  F4  ",
        });
        expect(parsed.afterClickHotkey).toBe("F4");
        expect(parsed.clickRegions).toEqual([{rect: {x: 1, y: 2, width: 10, height: 12}, delayMs: 400}]);
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
        expect(parseOverlayTarget("?mode=fingerprint-overlay&target=click&slots=0")).toBe("click");
        expect(parseOverlaySlots("?target=click&slots=0,6")).toEqual([0, 6]);
        expect(layoutSlotCount("click")).toBe(7);
    });

    it("counts captured fingerprints", () => {
        expect(personFingerprintCount([null, "a.png", null, "b.png"])).toBe(2);
        expect(personFingerprintCount(undefined)).toBe(0);
    });

    it("run layout ignores archive slots", () => {
        const boxes = Array.from({length: 9}, () => ({x: 1, y: 1, width: 20, height: 20}));
        expect(layoutReadyForRun({candidateBoxes: boxes})).toBe(true);
        expect(layoutReadyForRun({candidateBoxes: boxes.slice(0, 8)})).toBe(false);
        expect(archiveSlotsReady({archiveSlots: Array.from({length: 8}, () => null)})).toBe(false);
        expect(archiveSlotsReady({
            archiveSlots: Array.from({length: 8}, () => ({x: 0, y: 0, width: 12, height: 12})),
        })).toBe(true);
    });
});
