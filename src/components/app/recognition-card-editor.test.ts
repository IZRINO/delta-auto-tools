import {describe, expect, it} from "vitest";

import {RecognitionCardEditor} from "@/components/app/recognition-card-editor";
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
        watchReferenceImagePaths: [],
        watchMatchThreshold: "0.75",
        watchPollIntervalMs: "500",
        audioFiles: [],
        playMode: "single",
        comboWindowMs: "60000",
        volume: "0.8",
        cooldownMs: "1000",
        allowSimultaneous: false,
        colorProbes: [],
        colorMatchMode: "all",
        colorMatchMethod: "average",
    };
}

const compare = (RecognitionCardEditor as unknown as {
    compare: (previous: Record<string, unknown>, next: Record<string, unknown>) => boolean;
}).compare;

const cardGroups: never[] = [];
const dispatch = () => undefined;

function props(cardValue: RecognitionCardForm, adapter: object) {
    return {
        card: cardValue,
        index: 0,
        position: 0,
        groupSize: 1,
        cardGroups,
        collapsed: false,
        isNativeShell: true,
        dispatch,
        adapter,
        recordingTarget: null,
    };
}

describe("RecognitionCardEditor memo", () => {
    it("编辑卡片 A 时跳过引用未变的卡片 B", () => {
        const first = card("a");
        const second = card("b");
        const next = recognitionCardReducer([first, second], {
            type: "patch",
            cardId: "a",
            patch: {name: "updated"},
        });
        const adapter = {};

        expect(compare(props(first, adapter), props(next[0], adapter))).toBe(false);
        expect(compare(props(second, adapter), props(next[1], adapter))).toBe(true);
    });

    it("副作用 adapter 变化时必须刷新 callback", () => {
        const value = card("a");
        expect(compare(props(value, {}), props(value, {}))).toBe(false);
    });
});
