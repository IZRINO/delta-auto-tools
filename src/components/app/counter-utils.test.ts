import {describe, expect, it} from "vitest";

import type {CounterSettings} from "@/components/app/timer-types";
import {DEFAULT_COUNTER_GROUP_ID} from "@/components/app/timer-types";
import {
    counterEffectiveByGroup,
    counterRunsById,
    counterSettingsToForm,
    formatTimerHotkey,
    isCounterDirty,
    moveCounterItem,
    parseCounterSettingsForm
} from "@/components/app/counter-utils";

function sampleSettings(): CounterSettings {
    return {
        enabled: true,
        counterEnabled: true,
        display: {
            rect: {
                x: 340,
                y: 20,
                width: 320,
                height: 96,
            },
            fontOpacity: 0.8,
        },
        counterGroups: [
            {
                id: DEFAULT_COUNTER_GROUP_ID,
                name: "默认分组",
                enabled: true,
                display: {
                    rect: {
                        x: 340,
                        y: 20,
                        width: 320,
                        height: 96,
                    },
                    fontOpacity: 0.8,
                },
            },
        ],
        counters: [
            {
                id: "counter-alpha",
                groupId: DEFAULT_COUNTER_GROUP_ID,
                name: "测试计数器",
                startValue: 5,
                hotkey: "Ctrl+F3",
                enabled: true,
            },
        ],
    };
}

describe("counter-utils", () => {
    it("round trips counter settings through form state", () => {
        const settings = sampleSettings();
        const parsed = parseCounterSettingsForm(counterSettingsToForm(settings));

        expect(parsed).toEqual(settings);
    });

    it("formats Esc as a valid counter hotkey", () => {
        expect(formatTimerHotkey({
            key: "Escape",
            ctrlKey: false,
            altKey: false,
            shiftKey: false,
            metaKey: false
        } as React.KeyboardEvent<HTMLButtonElement>)).toBe("Esc");
    });

    it("migrates legacy settings into default groups", () => {
        const legacy = sampleSettings();
        delete legacy.counterGroups;
        legacy.counters = legacy.counters.map(({groupId: _groupId, ...counter}) => counter);

        const form = counterSettingsToForm(legacy);
        const parsed = parseCounterSettingsForm(form);

        expect(form.counterGroups.map((group) => group.id)).toEqual([DEFAULT_COUNTER_GROUP_ID]);
        expect(parsed.counters[0].groupId).toBe(DEFAULT_COUNTER_GROUP_ID);
    });

    it("filters effective counters by master, group, and card switches", () => {
        const form = counterSettingsToForm(sampleSettings());
        form.counterGroups.push({
            id: "counter-group-b",
            name: "分组B",
            enabled: true,
            display: {
                rect: {x: 0, y: 0, width: 320, height: 96},
                fontOpacity: "0.9",
            },
        });
        form.counters.push({
            id: "counter-b",
            groupId: "counter-group-b",
            name: "计数器B",
            startValue: "10",
            hotkey: "F2",
            enabled: true,
        });

        form.counterEnabled = false;
        expect(counterEffectiveByGroup(form, DEFAULT_COUNTER_GROUP_ID)).toEqual([]);

        form.counterEnabled = true;
        form.counterGroups[1].enabled = false;
        expect(counterEffectiveByGroup(form, DEFAULT_COUNTER_GROUP_ID).map((c) => c.id)).toEqual(["counter-alpha"]);

        form.counterGroups[1].enabled = true;
        form.counters[1].enabled = false;
        expect(counterEffectiveByGroup(form, "counter-group-b")).toEqual([]);

        form.counters[1].enabled = true;
        expect(counterEffectiveByGroup(form, DEFAULT_COUNTER_GROUP_ID).map((c) => c.id)).toEqual(["counter-alpha"]);
        expect(counterEffectiveByGroup(form, "counter-group-b").map((c) => c.id)).toEqual(["counter-b"]);
    });

    it("preserves custom display width through form parsing", () => {
        const form = counterSettingsToForm(sampleSettings());
        form.display.rect.width = 520;

        const parsed = parseCounterSettingsForm(form);

        expect(parsed.display.rect.width).toBe(520);
    });

    it("moves counter items within the same group", () => {
        const form = counterSettingsToForm(sampleSettings());
        form.counters.push({
            id: "beta",
            groupId: DEFAULT_COUNTER_GROUP_ID,
            name: "B",
            startValue: "1",
            hotkey: "F3",
            enabled: true
        });

        const moved = moveCounterItem(form.counters, "beta", "counter-alpha");
        expect(moved.map((c) => c.id)).toEqual(["beta", "counter-alpha"]);
    });

    it("maps counter runs by id", () => {
        const runs = [
            {id: "a", value: 3},
            {id: "b", value: 7},
        ];
        const map = counterRunsById(runs);
        expect(map.get("a")?.value).toBe(3);
        expect(map.get("b")?.value).toBe(7);
        expect(map.get("c")).toBeUndefined();
    });

    it("detects dirty counter form", () => {
        const settings = sampleSettings();
        const bootstrap = {settings, counterRuns: [], hotkeyError: null};
        const form = counterSettingsToForm(settings);

        expect(isCounterDirty(bootstrap, form)).toBe(false);

        const dirtyForm = {...form, counters: form.counters.map((c) => ({...c, startValue: "99"}))};
        expect(isCounterDirty(bootstrap, dirtyForm)).toBe(true);
    });
});
