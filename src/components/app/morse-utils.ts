import type React from "react";

import type {
    MorseRegionDetail,
    MorseRunResult,
    MorseSettings,
    MorseSettingsForm,
    Point,
    RegionRect,
} from "@/components/app/morse-types";
import {CLICK_REGION_LABELS, HOTKEY_MODIFIER_KEYS, REGION_LABELS} from "@/components/app/morse-types";

export {getErrorMessage} from "@/lib/error-utils";

export function settingsToForm(settings: MorseSettings): MorseSettingsForm {
    return {
        hotkey: settings.hotkey,
        regions: settings.regions,
        binaryThreshold: String(settings.binaryThreshold),
        autoInputDelay: String(settings.autoInputDelay),
        afterClickHotkey: settings.afterClickHotkey ?? "",
        autoClickEnabled: settings.autoClickEnabled ?? false,
        clickRegions: (() => {
            const regions: { rect: RegionRect | null; delayMs: string }[] = (settings.clickRegions ?? []).map((r) => ({
                rect: r.rect,
                delayMs: String(r.delayMs ?? 500),
            }));
            while (regions.length < 7) {
                regions.push({rect: null, delayMs: "500"});
            }
            return regions;
        })(),
    };
}

export function parseSettingsForm(form: MorseSettingsForm): MorseSettings {
    const hotkey = form.hotkey.trim();
    if (!hotkey) {
        throw new Error("热键不能为空。");
    }

    const binaryThreshold = Number.parseInt(form.binaryThreshold, 10);
    if (!Number.isInteger(binaryThreshold) || binaryThreshold < 0 || binaryThreshold > 255) {
        throw new Error("二值化阈值必须是 0 到 255 之间的整数。");
    }


    const autoInputDelay = Number.parseInt(form.autoInputDelay, 10);
    if (!Number.isInteger(autoInputDelay) || autoInputDelay < 0) {
        throw new Error("输入延迟必须是大于等于 0 的整数毫秒值。");
    }

    const afterClickHotkey = form.afterClickHotkey.trim();

    return {
        hotkey,
        regions: form.regions,
        binaryThreshold,
        autoInputDelay,
        afterClickHotkey: afterClickHotkey ? afterClickHotkey : null,
        autoClickEnabled: form.autoClickEnabled,
        clickRegions: (form.clickRegions ?? [])
            .filter((r) => r.rect !== null)
            .map((r) => ({
                rect: r.rect!,
                delayMs: Number.parseInt(r.delayMs, 10) || 500,
            })),
    };
}

export function formatTimestamp(timestamp: number | null | undefined): string {
    if (!timestamp) {
        return "--:--:--";
    }

    return new Intl.DateTimeFormat("zh-CN", {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
        hour12: false,
    }).format(timestamp);
}

export function formatRegion(rect: RegionRect | null): string {
    if (!rect) {
        return "未设置";
    }

    return `X ${rect.x} · Y ${rect.y} · W ${rect.width} · H ${rect.height}`;
}

export function getSelectionRect(start: Point, end: Point): RegionRect {
    const left = Math.min(start.x, end.x);
    const top = Math.min(start.y, end.y);
    const width = Math.abs(end.x - start.x);
    const height = Math.abs(end.y - start.y);

    return {
        x: Math.round(left),
        y: Math.round(top),
        width: Math.round(width),
        height: Math.round(height),
    };
}

export function normalizeRunDetails(latestRun: MorseRunResult | null): MorseRegionDetail[] {
    return REGION_LABELS.map((_, slot) => {
        const detail = latestRun?.details.find((item) => item.slot === slot);

        return (
            detail ?? {
                slot,
                thresholdMode: "--",
                contourCount: 0,
                morse: null,
                digit: null,
                error: null,
            }
        );
    });
}

export function parseOverlaySlots(search?: string): number[] {
    const params = new URLSearchParams(
        search ?? (typeof window === "undefined" ? "" : window.location.search),
    );
    const slotsParam = params.get("slots");
    const singleSlotParam = params.get("slot");
    const target = parseOverlayTarget(search);
    const maxSlots = target === "click" ? CLICK_REGION_LABELS.length : REGION_LABELS.length;

    const rawValues = slotsParam
        ? slotsParam.split(",")
        : singleSlotParam
            ? [singleSlotParam]
            : [];

    const parsed = rawValues
        .map((value) => Number.parseInt(value, 10))
        .filter((value, index, values) => Number.isInteger(value) && value >= 0 && value < maxSlots && values.indexOf(value) === index);

    return parsed.length > 0 ? parsed : target === "click" ? [0] : [0, 1, 2];
}

export function parseOverlayTarget(search?: string): "sampling" | "click" {
    const params = new URLSearchParams(
        search ?? (typeof window === "undefined" ? "" : window.location.search),
    );
    return params.get("target") === "click" ? "click" : "sampling";
}

export function createRegionSelectionRequest(
    slots: number[],
    target?: "sampling" | "click",
): { target: "sampling" | "click"; slots: number[] } {
    const resolvedTarget = target ?? (slots.some((slot) => slot >= REGION_LABELS.length) ? "click" : "sampling");
    const maxSlots = resolvedTarget === "click" ? CLICK_REGION_LABELS.length : REGION_LABELS.length;
    return {
        target: resolvedTarget,
        slots: slots.filter((slot, index, values) =>
            Number.isInteger(slot) && slot >= 0 && slot < maxSlots && values.indexOf(slot) === index
        ),
    };
}

export function clickRegionRows(clickRegions: MorseSettingsForm["clickRegions"]) {
    return clickRegions
        .map((region, slotIndex) => ({...region, slotIndex}))
        .filter((region) => region.rect !== null)
        .slice(0, CLICK_REGION_LABELS.length);
}

export function normalizeHotkeyPrimaryKey(key: string): string | null {
    if (HOTKEY_MODIFIER_KEYS.has(key)) {
        return null;
    }

    if (/^F\d{1,2}$/i.test(key)) {
        return key.toUpperCase();
    }

    if (/^[a-z]$/i.test(key)) {
        return key.toUpperCase();
    }

    if (/^[0-9]$/.test(key)) {
        return key;
    }

    const specialKeyMap: Record<string, string> = {
        " ": "Space",
        Enter: "Enter",
        Tab: "Tab",
        Escape: "Esc",
        ArrowUp: "Up",
        ArrowDown: "Down",
        ArrowLeft: "Left",
        ArrowRight: "Right",
        Home: "Home",
        End: "End",
        PageUp: "PageUp",
        PageDown: "PageDown",
        Insert: "Insert",
        Delete: "Delete",
        Backspace: "Backspace",
        ";": ";",
        "；": ";",
        ",": ",",
        "，": ",",
        ".": ".",
        "。": ".",
        "/": "/",
        "？": "/",
        "、": "/",
        "\\": "\\",
        "￥": "\\",
        "｜": "\\",
        "[": "[",
        "【": "[",
        "「": "[",
        "]": "]",
        "】": "]",
        "」": "]",
        "-": "-",
        "－": "-",
        "=": "=",
        "＝": "=",
        "+": "+",
        "＋": "+",
        "`": "`",
        "｀": "`",
        "'": "'",
        "‘": "'",
        "’": "'",
    };

    return specialKeyMap[key] ?? null;
}

export function formatRecordedHotkey(event: Pick<React.KeyboardEvent<HTMLButtonElement>, "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): string | null {
    const primaryKey = normalizeHotkeyPrimaryKey(event.key);
    if (!primaryKey) {
        return null;
    }

    const segments: string[] = [];
    if (event.ctrlKey) {
        segments.push("Ctrl");
    }
    if (event.altKey) {
        segments.push("Alt");
    }
    if (event.shiftKey) {
        segments.push("Shift");
    }
    if (event.metaKey) {
        segments.push("Super");
    }

    segments.push(primaryKey);
    return segments.join("+");
}
