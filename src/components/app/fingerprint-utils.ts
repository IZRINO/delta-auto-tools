import {
    ARCHIVE_LABELS,
    CANDIDATE_LABELS,
    type FingerprintSettings,
    type FingerprintSettingsForm,
    type LayoutTarget,
} from "@/components/app/fingerprint-types";

export {getErrorMessage} from "@/lib/error-utils";
export {formatRecordedHotkey, formatRegion, formatTimestamp} from "@/components/app/morse-utils";

export function settingsToForm(settings: FingerprintSettings): FingerprintSettingsForm {
    return {
        hotkey: settings.hotkey,
        occupancyThreshold: String(settings.occupancyThreshold),
        matchThreshold: String(settings.matchThreshold),
        autoClickEnabled: settings.autoClickEnabled,
        clickDelayMs: String(settings.clickDelayMs),
        nameRegion: settings.nameRegion,
        candidateBoxes: settings.candidateBoxes,
        archiveSlots: settings.archiveSlots,
        people: settings.people,
    };
}

export function parseSettingsForm(form: FingerprintSettingsForm): FingerprintSettings {
    const hotkey = form.hotkey.trim();
    if (!hotkey) {
        throw new Error("热键不能为空。");
    }
    const occupancyThreshold = Number.parseFloat(form.occupancyThreshold);
    if (!Number.isFinite(occupancyThreshold) || occupancyThreshold < 0) {
        throw new Error("占用阈值必须是大于等于 0 的数字。");
    }
    const matchThreshold = Number.parseFloat(form.matchThreshold);
    if (!Number.isFinite(matchThreshold) || matchThreshold < 0 || matchThreshold > 1) {
        throw new Error("匹配阈值必须是 0 到 1 之间的数字。");
    }
    const clickDelayMs = Number.parseInt(form.clickDelayMs, 10);
    if (!Number.isInteger(clickDelayMs) || clickDelayMs < 0) {
        throw new Error("点击延迟必须是大于等于 0 的整数毫秒值。");
    }
    return {
        hotkey,
        occupancyThreshold,
        matchThreshold,
        autoClickEnabled: form.autoClickEnabled,
        clickDelayMs,
        nameRegion: form.nameRegion,
        candidateBoxes: form.candidateBoxes,
        archiveSlots: form.archiveSlots,
        people: form.people,
    };
}

export function layoutSlotCount(target: LayoutTarget): number {
    if (target === "name") return 1;
    if (target === "candidates") return 9;
    return 8;
}

export function layoutLabels(target: LayoutTarget): readonly string[] {
    if (target === "name") return ["名条"];
    if (target === "candidates") return CANDIDATE_LABELS;
    return ARCHIVE_LABELS;
}

export function parseOverlayTarget(search = window.location.search): LayoutTarget {
    const target = new URLSearchParams(search).get("target");
    if (target === "candidates" || target === "archive" || target === "name") {
        return target;
    }
    return "name";
}

export function parseOverlaySlots(search = window.location.search): number[] {
    const params = new URLSearchParams(search);
    const target = parseOverlayTarget(search);
    const max = layoutSlotCount(target);
    const raw = params.get("slots");
    if (!raw) {
        const single = Number.parseInt(params.get("slot") ?? "", 10);
        if (Number.isInteger(single) && single >= 0 && single < max) {
            return [single];
        }
        return Array.from({length: max}, (_, index) => index);
    }
    const seen = new Set<number>();
    const slots: number[] = [];
    for (const part of raw.split(",")) {
        const slot = Number.parseInt(part, 10);
        if (!Number.isInteger(slot) || slot < 0 || slot >= max || seen.has(slot)) {
            continue;
        }
        seen.add(slot);
        slots.push(slot);
    }
    return slots.length > 0 ? slots : [0];
}

export function personFingerprintCount(paths: Array<string | null> | undefined): number {
    return (paths ?? []).filter(Boolean).length;
}
