import {describe, expect, it} from "vitest";

import {recognitionCardReducer} from "@/components/app/recognition-card-reducer";
import type {RecognitionCardForm} from "@/components/app/recognition-types";

function card(id: string): RecognitionCardForm {
    return {
        id,
        name: id,
        enabled: true,
        triggerMode: "hotkey",
        hotkey: "F1",
        watchRegion: null,
        watchReferenceImagePaths: ["old.png"],
        watchMatchThreshold: "0.75",
        watchPollIntervalMs: "500",
        audioFiles: ["old.wav"],
        playMode: "single",
        comboWindowMs: "60000",
        volume: "0.8",
        cooldownMs: "1000",
        allowSimultaneous: false,
        colorProbes: [{region: null, targets: [{color: "#ff0000", tolerance: "30"}], probeMatchMode: "any"}],
        colorMatchMode: "all",
        colorMatchMethod: "average",
    };
}

describe("recognitionCardReducer", () => {
    it("只替换目标卡片并保持其他卡片引用", () => {
        const first = card("a");
        const second = card("b");
        const next = recognitionCardReducer([first, second], {
            type: "patch",
            cardId: "a",
            patch: {name: "updated", enabled: false},
        });

        expect(next[0]).not.toBe(first);
        expect(next[0].name).toBe("updated");
        expect(next[1]).toBe(second);
    });

    it("通过卡片级 update 收口嵌套编辑并保持其他卡片引用", () => {
        const second = card("b");
        let cards = [card("a"), second];
        cards = recognitionCardReducer(cards, {
            type: "patchProbe",
            cardId: "a",
            probeIndex: 0,
            patch: {probeMatchMode: "all"},
        });
        cards = recognitionCardReducer(cards, {
            type: "update",
            cardId: "a",
            update: (current) => ({
                ...current,
                colorProbes: [...current.colorProbes, {
                    region: null,
                    targets: [{color: "#00ff00", tolerance: "20"}],
                    probeMatchMode: "any",
                }],
                watchReferenceImagePaths: ["new.png"],
                audioFiles: ["new.wav"],
            }),
        });

        expect(cards[0].colorProbes[0].probeMatchMode).toBe("all");
        expect(cards[0].colorProbes).toHaveLength(2);
        expect(cards[0].watchReferenceImagePaths).toEqual(["new.png"]);
        expect(cards[0].audioFiles).toEqual(["new.wav"]);
        expect(cards[1]).toBe(second);
    });
});
