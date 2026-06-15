import type React from "react";
import {useEffect} from "react";
import {invoke} from "@tauri-apps/api/core";
import {COUNTER_EVENTS, listenEvent} from "@/lib/tauri-events";

import type {
    CounterBootstrap,
    CounterGroup,
    CounterItem,
    CounterItemForm,
    CounterRunState,
    CounterSettings,
    CounterSettingsForm,
    TimerDisplaySettings,
    TimerGroupForm
} from "@/components/app/timer-types";
import {DEFAULT_COUNTER_GROUP_ID, TIMER_DISPLAY_MIN_HEIGHT, TIMER_DISPLAY_WIDTH} from "@/components/app/timer-types";
import {formatRecordedHotkey} from "@/components/app/morse-utils";

export function formatTimerHotkey(event: Pick<React.KeyboardEvent<HTMLButtonElement>, "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): string | null {
    return formatRecordedHotkey(event);
}

function displaySettingsToForm(display: TimerDisplaySettings) {
    return {
        rect: display.rect,
        fontOpacity: String(display.fontOpacity),
    };
}

function defaultCounterGroup(display: TimerDisplaySettings): CounterGroup {
    return {
        id: DEFAULT_COUNTER_GROUP_ID,
        name: "默认分组",
        enabled: true,
        display,
    };
}

function groupsToForm(groups: CounterGroup[]): TimerGroupForm[] {
    return groups.map((group) => ({
        id: group.id,
        name: group.name,
        enabled: group.enabled,
        display: displaySettingsToForm(group.display),
    }));
}

function normalizeGroups(
    groups: CounterGroup[] | undefined,
    legacyDisplay: TimerDisplaySettings,
    defaultGroupId: string,
): CounterGroup[] {
    const normalized = (groups && groups.length > 0 ? groups : [defaultCounterGroup(legacyDisplay)])
        .map((group) => ({
            id: group.id.trim() || defaultGroupId,
            name: group.name.trim() || "未命名分组",
            enabled: group.enabled ?? true,
            display: group.display ?? legacyDisplay,
        }));

    if (!normalized.some((group) => group.id === defaultGroupId)) {
        normalized.unshift(defaultCounterGroup(legacyDisplay));
    }

    return normalized;
}

function normalizeGroupId(groupId: string | undefined, groupIds: Set<string>, defaultGroupId: string): string {
    return groupId && groupIds.has(groupId) ? groupId : defaultGroupId;
}

function parseFontOpacity(value: string): number {
    const fontOpacity = Number.parseFloat(value);
    if (!Number.isFinite(fontOpacity) || fontOpacity < 0.1 || fontOpacity > 1) {
        throw new Error("字体透明度必须是 0.1 到 1 之间的数字。");
    }
    return fontOpacity;
}

export function counterSettingsToForm(settings: CounterSettings): CounterSettingsForm {
    const legacyEnabled = Boolean(settings.enabled);
    const counterGroups = normalizeGroups(settings.counterGroups, settings.display, DEFAULT_COUNTER_GROUP_ID);
    const counterGroupIds = new Set(counterGroups.map((group) => group.id));

    return {
        counterEnabled: settings.counterEnabled ?? legacyEnabled,
        display: displaySettingsToForm(settings.display),
        counterGroups: groupsToForm(counterGroups),
        counters: settings.counters.map((counter) => ({
            id: counter.id,
            groupId: normalizeGroupId(counter.groupId, counterGroupIds, DEFAULT_COUNTER_GROUP_ID),
            name: counter.name,
            startValue: String(counter.startValue),
            hotkey: counter.hotkey,
            enabled: counter.enabled ?? true,
        })),
    };
}

function parseDisplaySettings(display: CounterSettingsForm["display"], itemCount: number): TimerDisplaySettings {
    const displayWidth = Math.max(TIMER_DISPLAY_WIDTH, Math.round(display.rect.width));

    return {
        rect: {
            ...display.rect,
            width: displayWidth,
            height: displayHeight(itemCount),
        },
        fontOpacity: parseFontOpacity(display.fontOpacity),
    };
}

export function parseCounterSettingsForm(form: CounterSettingsForm): CounterSettings {
    if (form.counters.length === 0) {
        throw new Error("至少需要保留一个计数器。");
    }

    const counterGroups = parseGroups(
        mirrorDefaultGroupDisplay(form.counterGroups, DEFAULT_COUNTER_GROUP_ID, form.display),
        DEFAULT_COUNTER_GROUP_ID,
        "计数器分组",
    );
    const counterGroupIds = new Set(counterGroups.map((group) => group.id));

    const counters = form.counters.map((counter): CounterItem => {
        const name = counter.name.trim();
        if (!name) {
            throw new Error("计数器名称不能为空。");
        }

        const hotkey = counter.hotkey.trim();
        if (!hotkey) {
            throw new Error(`${name} 的快捷键不能为空。`);
        }

        const startValue = Number.parseInt(counter.startValue, 10);
        if (!Number.isInteger(startValue)) {
            throw new Error(`${name} 的起始数必须是整数。`);
        }

        return {
            id: counter.id,
            groupId: normalizeGroupId(counter.groupId, counterGroupIds, DEFAULT_COUNTER_GROUP_ID),
            name,
            startValue,
            hotkey,
            enabled: counter.enabled,
        };
    });

    const counterCountsByGroup = enabledCountByGroup(counters);
    const normalizedCounterGroups = counterGroups.map((group) => ({
        ...group,
        display: parseDisplaySettings(displaySettingsToForm(group.display), counterCountsByGroup.get(group.id) ?? 0),
    }));
    const legacyCounterDisplay = normalizedCounterGroups.find((group) => group.id === DEFAULT_COUNTER_GROUP_ID)?.display ?? normalizedCounterGroups[0].display;

    return {
        enabled: form.counterEnabled,
        counterEnabled: form.counterEnabled,
        display: legacyCounterDisplay,
        counterGroups: normalizedCounterGroups,
        counters,
    };
}

function parseGroups(groups: TimerGroupForm[], defaultGroupId: string, label: string): CounterGroup[] {
    if (groups.length === 0) {
        throw new Error(`至少需要保留一个${label}。`);
    }

    const seen = new Set<string>();
    return groups.map((group) => {
        const id = group.id.trim() || defaultGroupId;
        if (seen.has(id)) {
            throw new Error(`${label} ID 重复：${id}`);
        }
        seen.add(id);

        const name = group.name.trim();
        if (!name) {
            throw new Error(`${label}名称不能为空。`);
        }

        return {
            id,
            name,
            enabled: group.enabled,
            display: parseDisplaySettings(group.display, 0),
        };
    });
}

function mirrorDefaultGroupDisplay(
    groups: TimerGroupForm[],
    defaultGroupId: string,
    display: TimerGroupForm["display"],
): TimerGroupForm[] {
    return groups.map((group) => (group.id === defaultGroupId ? {...group, display} : group));
}

function enabledCountByGroup(items: Array<{ groupId?: string; enabled: boolean }>): Map<string, number> {
    const map = new Map<string, number>();
    for (const item of items) {
        if (!item.enabled || !item.groupId) {
            continue;
        }
        map.set(item.groupId, (map.get(item.groupId) ?? 0) + 1);
    }
    return map;
}

export function createCounterGroup(existingCount: number): TimerGroupForm {
    const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    return {
        id: `counter-group-${suffix}`,
        name: `计数分组 ${existingCount + 1}`,
        enabled: true,
        display: displaySettingsToForm(counterDefaultDisplay()),
    };
}

function counterDefaultDisplay(): TimerDisplaySettings {
    return {
        rect: {x: 420, y: 80, width: TIMER_DISPLAY_WIDTH, height: TIMER_DISPLAY_MIN_HEIGHT},
        fontOpacity: 0.92,
    };
}

export function createCounterItem(existingCount: number, groupId = DEFAULT_COUNTER_GROUP_ID): CounterItem {
    const nextIndex = existingCount + 1;
    const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

    return {
        id: `counter-${suffix}`,
        groupId,
        name: `计数器 ${nextIndex}`,
        startValue: 0,
        hotkey: "F3",
        enabled: true,
    };
}

export function counterEffectiveByGroup(form: CounterSettingsForm | null, groupId: string): CounterItemForm[] {
    if (!form?.counterEnabled) {
        return [];
    }
    const group = form.counterGroups.find((item) => item.id === groupId);
    if (!group?.enabled) {
        return [];
    }
    return form.counters.filter((counter) => counter.groupId === groupId && counter.enabled);
}

export function moveCounterItem<T extends { id: string }>(items: T[], activeId: string, overId: string): T[] {
    if (activeId === overId) {
        return items;
    }

    const activeIndex = items.findIndex((item) => item.id === activeId);
    const overIndex = items.findIndex((item) => item.id === overId);
    if (activeIndex === -1 || overIndex === -1) {
        return items;
    }

    const next = [...items];
    const [moved] = next.splice(activeIndex, 1);
    next.splice(overIndex, 0, moved);
    return next;
}

export function displayHeight(itemCount: number): number {
    return Math.max(TIMER_DISPLAY_MIN_HEIGHT, 48 + Math.max(1, itemCount) * 30);
}

export function counterRunsById(runs: CounterRunState[]): Map<string, CounterRunState> {
    return new Map(runs.map((run) => [run.id, run]));
}

export function isCounterDirty(bootstrap: CounterBootstrap | null, form: CounterSettingsForm | null): boolean {
    if (!bootstrap || !form) {
        return false;
    }

    try {
        return JSON.stringify(counterSettingsToForm(bootstrap.settings)) !== JSON.stringify(counterSettingsToForm(parseCounterSettingsForm(form)));
    } catch {
        return true;
    }
}

export function useCounterOverlayBootstrap(isNativeShell: boolean, setBootstrap: (value: CounterBootstrap) => void) {
    useEffect(() => {
        document.body.dataset.overlayMode = "true";
        return () => {
            delete document.body.dataset.overlayMode;
        };
    }, []);

    useEffect(() => {
        if (!isNativeShell) {
            return;
        }

        let disposed = false;
        let unlistenStateChanged: (() => void) | undefined;

        void invoke<CounterBootstrap>("counter_get_bootstrap").then((next) => {
            if (!disposed) {
                setBootstrap(next);
            }
        });

        void listenEvent(COUNTER_EVENTS.stateChanged, (event) => {
            if (!disposed) {
                setBootstrap(event.payload);
            }
        }).then((dispose) => {
            unlistenStateChanged = dispose;
        });

        return () => {
            disposed = true;
            unlistenStateChanged?.();
        };
    }, [isNativeShell, setBootstrap]);
}
