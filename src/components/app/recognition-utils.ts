import type {
    ColorProbe,
    ColorProbeForm,
    RecognitionAudioEffect,
    RecognitionCard,
    RecognitionCardForm,
    RecognitionClickEffect,
    RecognitionGroup,
    RecognitionHotkeyEffect,
    RecognitionSettings,
    RecognitionSettingsForm,
} from "@/components/app/recognition-types";
import {DEFAULT_RECOGNITION_CARD} from "@/components/app/recognition-types";

export const DEFAULT_RECOGNITION_GROUP_ID = "default-recognition-group";

function defaultRecognitionGroup(): RecognitionGroup {
    return {
        id: DEFAULT_RECOGNITION_GROUP_ID,
        name: "默认分组",
        order: 0,
        collapsed: false,
        enabled: true,
    };
}

function normalizeRecognitionGroups(settings: RecognitionSettings): RecognitionGroup[] {
    const groups = (settings.cardGroups ?? [])
        .map((group) => ({
            id: group.id.trim() || DEFAULT_RECOGNITION_GROUP_ID,
            name: group.name.trim() || "未命名分组",
            order: Number.isFinite(group.order) ? group.order : 0,
            collapsed: group.collapsed ?? false,
            enabled: group.enabled ?? true,
        }))
        .filter((group, index, all) => all.findIndex((item) => item.id === group.id) === index);
    if (!groups.some((group) => group.id === DEFAULT_RECOGNITION_GROUP_ID)) {
        groups.unshift(defaultRecognitionGroup());
    }
    return groups.sort((a, b) => a.order - b.order);
}

function normalizeRecognitionCardGroupId(groupId: string | null | undefined, groupIds: Set<string>): string {
    const trimmed = groupId?.trim();
    return trimmed && groupIds.has(trimmed) ? trimmed : DEFAULT_RECOGNITION_GROUP_ID;
}

function recognitionCardSortKey(
    card: RecognitionCard,
    index: number,
    groupOrderById: Map<string, number>,
    groupIds: Set<string>,
): [number, number, number] {
    const groupId = normalizeRecognitionCardGroupId(card.groupId, groupIds);
    const groupOrder = groupOrderById.get(groupId) ?? Number.MAX_SAFE_INTEGER;
    const cardOrder = Number.isFinite(card.order ?? NaN) ? card.order ?? 0 : index;
    return [groupOrder, cardOrder, index];
}

export function rgbToHex(rgb: [number, number, number]): string {
    const [r, g, b] = rgb;
    return "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");
}

function hexToRgb(hex: string): [number, number, number] {
    const clean = hex.startsWith("#") ? hex.slice(1) : hex;
    if (clean.length !== 6) {
        throw new Error("颜色格式必须为 #RRGGBB。");
    }
    const r = parseInt(clean.slice(0, 2), 16);
    const g = parseInt(clean.slice(2, 4), 16);
    const b = parseInt(clean.slice(4, 6), 16);
    if ([r, g, b].some((v) => Number.isNaN(v) || v < 0 || v > 255)) {
        throw new Error("颜色值必须在 00-FF 之间。");
    }
    return [r, g, b];
}

function probeToForm(probe: ColorProbe): ColorProbeForm {
    return {
        region: probe.region,
        targets: (probe.targets ?? []).map((t) => ({
            color: rgbToHex(t.color),
            tolerance: String(t.tolerance),
        })),
        probeMatchMode: probe.probeMatchMode ?? "any",
    };
}

function parseProbeForm(form: ColorProbeForm): ColorProbe {
    if (form.targets.length === 0) {
        throw new Error("探针至少需要配置一个目标颜色。");
    }
    const targets = form.targets.map((t) => {
        const tolerance = parseInt(t.tolerance, 10);
        if (Number.isNaN(tolerance) || tolerance < 0 || tolerance > 255) {
            throw new Error("颜色容差必须在 0 到 255 之间。");
        }
        return {
            color: hexToRgb(t.color),
            tolerance,
        };
    });
    return {
        region: form.region,
        targets,
        probeMatchMode: form.probeMatchMode ?? "any",
    };
}

export function settingsToForm(settings: RecognitionSettings): RecognitionSettingsForm {
    const recognitionEnabled = settings.recognitionEnabled ?? settings.audioEnabled ?? true;
    const cardGroups = normalizeRecognitionGroups(settings);
    const groupIds = new Set(cardGroups.map((group) => group.id));
    const groupOrderById = new Map(cardGroups.map((group, index) => [group.id, group.order ?? index]));
    return {
        recognitionEnabled,
        audioEnabled: recognitionEnabled,
        cardGroups,
        cards: settings.cards
            .map((card, index) => ({card, index, key: recognitionCardSortKey(card, index, groupOrderById, groupIds)}))
            .sort((a, b) =>
                a.key[0] - b.key[0]
                || a.key[1] - b.key[1]
                || a.key[2] - b.key[2]
            )
            .map(({card, index}) => cardToForm({
                ...card,
                groupId: normalizeRecognitionCardGroupId(card.groupId, groupIds),
                order: Number.isFinite(card.order ?? NaN) ? card.order : index,
            })),
    };
}

function legacyAudioEffect(card: RecognitionCard): RecognitionAudioEffect | null {
    const hasLegacyAudio = card.audioFiles !== undefined
        || card.playMode !== undefined
        || card.comboWindowMs !== undefined
        || card.comboWindows !== undefined
        || card.volume !== undefined
        || card.allowSimultaneous !== undefined;
    if (!hasLegacyAudio) {
        return card.effects?.audio ?? null;
    }
    const audioFiles = card.audioFiles ?? [];
    return {
        audioFiles,
        playMode: card.playMode ?? "single",
        comboWindowMs: card.comboWindowMs ?? 60000,
        comboWindows: card.comboWindows ?? [],
        volume: card.volume ?? 0.8,
        allowSimultaneous: card.allowSimultaneous ?? false,
    };
}

function hotkeyEffectSteps(effect: RecognitionHotkeyEffect | null | undefined): { hotkey: string; delayMs: string }[] {
    if (!effect) {
        return [];
    }
    const steps = (effect.steps ?? [])
        .map((step) => ({
            hotkey: step.hotkey ?? "",
            delayMs: String(step.delayMs ?? 0),
        }))
        .filter((step) => step.hotkey.trim().length > 0);
    if (steps.length > 0) {
        return steps;
    }
    const legacyHotkey = effect.hotkey?.trim() ?? "";
    return legacyHotkey ? [{hotkey: legacyHotkey, delayMs: "0"}] : [];
}

function cardToForm(card: RecognitionCard): RecognitionCardForm {
    const audio = legacyAudioEffect(card);
    const activation = card.activation ?? {mode: "always", hotkey: null, durationMs: 10000, triggerCount: 1};
    const click = card.effects?.click ?? null;
    const hotkeySteps = hotkeyEffectSteps(card.effects?.hotkey);
    return {
        id: card.id,
        groupId: card.groupId ?? DEFAULT_RECOGNITION_GROUP_ID,
        order: card.order ?? 0,
        collapsed: false,
        name: card.name,
        enabled: card.enabled,
        triggerMode: card.triggerMode,
        hotkey: card.hotkey ?? "",
        watchRegion: card.watchRegion,
        watchReferenceImagePath: card.watchReferenceImagePath ?? "",
        watchMatchThreshold: String(card.watchMatchThreshold),
        watchPollIntervalMs: String(card.watchPollIntervalMs),
        activationMode: activation.mode ?? "always",
        activationHotkey: activation.hotkey ?? "",
        activationDurationMs: String(activation.durationMs ?? 10000),
        activationTriggerCount: String(activation.triggerCount ?? 1),
        audioEffectEnabled: Boolean(audio),
        hotkeyEffectEnabled: Boolean(card.effects?.hotkey),
        clickEffectEnabled: Boolean(click),
        effectHotkey: hotkeySteps[0]?.hotkey ?? card.effects?.hotkey?.hotkey ?? "",
        hotkeyEffectSteps: hotkeySteps,
        clickMode: click?.mode ?? "customRegion",
        clickCustomRegion: click?.customRegion ?? null,
        clickColorProbeIndex: click?.colorProbeIndex == null ? "" : String(click.colorProbeIndex),
        audioFiles: audio?.audioFiles ?? [],
        playMode: audio?.playMode ?? "single",
        comboWindowMs: String(audio?.comboWindowMs ?? 60000),
        comboWindows: (audio?.audioFiles ?? []).map((_, i) =>
            String((audio?.comboWindows ?? [])[i] ?? audio?.comboWindowMs ?? 60000),
        ),
        volume: String(audio?.volume ?? 0.8),
        cooldownMs: String(card.cooldownMs),
        allowSimultaneous: audio?.allowSimultaneous ?? false,
        colorProbes: (card.colorProbes ?? []).map(probeToForm),
        colorMatchMode: card.colorMatchMode ?? "all",
        colorMatchMethod: card.colorMatchMethod ?? "average",
    };
}

export function parseSettingsForm(form: RecognitionSettingsForm): RecognitionSettings {
    const cards = form.cards.map((card) => parseCardForm(card));
    return {
        recognitionEnabled: form.recognitionEnabled ?? form.audioEnabled ?? true,
        cardGroups: form.cardGroups ?? [defaultRecognitionGroup()],
        cards,
    };
}

function parseCardForm(form: RecognitionCardForm): RecognitionCard {
    const name = form.name.trim();
    if (!name) {
        throw new Error("卡片名称不能为空。");
    }

    const cooldownMs = parseInt(form.cooldownMs, 10);
    if (Number.isNaN(cooldownMs) || cooldownMs < 0 || cooldownMs > 60000) {
        throw new Error("冷却时间必须在 0 到 60000 毫秒之间。");
    }

    const watchMatchThreshold = parseFloat(form.watchMatchThreshold);
    if (Number.isNaN(watchMatchThreshold) || watchMatchThreshold < 0 || watchMatchThreshold > 1) {
        throw new Error("匹配阈值必须在 0 到 1 之间。");
    }

    const watchPollIntervalMs = parseInt(form.watchPollIntervalMs, 10);
    if (Number.isNaN(watchPollIntervalMs) || watchPollIntervalMs < 100 || watchPollIntervalMs > 10000) {
        throw new Error("轮询间隔必须在 100 到 10000 毫秒之间。");
    }

    const hotkey = form.triggerMode === "hotkey" ? form.hotkey.trim() || null : null;
    if (form.triggerMode === "hotkey" && !hotkey) {
        throw new Error("快捷键模式下必须设置触发快捷键。");
    }

    const activationMode = form.triggerMode === "hotkey" ? "always" : form.activationMode ?? "always";
    const activationHotkey = activationMode === "always" ? null : form.activationHotkey?.trim() || null;
    if (form.triggerMode !== "hotkey" && activationMode !== "always" && !activationHotkey) {
        throw new Error("当前识别激活方式必须设置激活快捷键。");
    }
    let activationDurationMs = 10000;
    let activationTriggerCount = 1;
    if (activationMode === "timedHotkey") {
        const parsed = parseInt(form.activationDurationMs ?? "10000", 10);
        if (Number.isNaN(parsed) || parsed < 100 || parsed > 600000) {
            throw new Error("限时识别时长必须在 100 到 600000 毫秒之间。");
        }
        activationDurationMs = parsed;
        const parsedTriggerCount = parseInt(form.activationTriggerCount ?? "1", 10);
        if (Number.isNaN(parsedTriggerCount) || parsedTriggerCount < 1 || parsedTriggerCount > 1000) {
            throw new Error("限时识别触发次数必须在 1 到 1000 之间。");
        }
        activationTriggerCount = parsedTriggerCount;
    }

    const colorProbes = form.triggerMode === "colorWatch"
        ? form.colorProbes.map(parseProbeForm)
        : [];
    if (form.triggerMode === "colorWatch" && colorProbes.length === 0) {
        throw new Error("识色模式下至少需要配置一个探针。");
    }

    const effects: NonNullable<RecognitionCard["effects"]> = {};
    const hasExplicitEffects = form.audioEffectEnabled !== undefined
        || form.hotkeyEffectEnabled !== undefined
        || form.clickEffectEnabled !== undefined;
    const audioEffectEnabled = form.audioEffectEnabled ?? (!hasExplicitEffects && form.audioFiles !== undefined);
    if (audioEffectEnabled) {
        effects.audio = parseAudioEffect(form);
    }
    if (form.hotkeyEffectEnabled) {
        const configuredSteps = form.hotkeyEffectSteps?.length
            ? form.hotkeyEffectSteps
            : [{hotkey: form.effectHotkey ?? "", delayMs: "0"}];
        const steps = configuredSteps
            .map((step) => ({
                hotkey: step.hotkey.trim(),
                delayMs: parseInt(step.delayMs || "0", 10),
            }))
            .filter((step) => step.hotkey.length > 0);
        const effectHotkey = steps[0]?.hotkey ?? "";
        if (!effectHotkey) {
            throw new Error("按键效果必须设置快捷键。");
        }
        if (steps.some((step) => Number.isNaN(step.delayMs) || step.delayMs < 0 || step.delayMs > 600000)) {
            throw new Error("按键效果延迟必须在 0 到 600000 毫秒之间。");
        }
        effects.hotkey = {hotkey: effectHotkey, steps};
    }
    if (form.clickEffectEnabled) {
        effects.click = parseClickEffect(form, colorProbes.length);
    }
    if (!effects.audio && !effects.hotkey && !effects.click) {
        throw new Error("每张卡片至少需要启用一个触发效果。");
    }

    return {
        id: form.id || generateCardId(),
        groupId: form.groupId?.trim() || DEFAULT_RECOGNITION_GROUP_ID,
        order: form.order ?? 0,
        name,
        enabled: form.enabled,
        triggerMode: form.triggerMode,
        hotkey,
        watchRegion: form.triggerMode === "regionWatch" ? form.watchRegion : null,
        watchReferenceImagePath: form.triggerMode === "regionWatch" ? form.watchReferenceImagePath.trim() || null : null,
        watchMatchThreshold,
        watchPollIntervalMs,
        activation: {
            mode: activationMode,
            hotkey: activationHotkey,
            durationMs: activationDurationMs,
            triggerCount: activationTriggerCount,
        },
        effects,
        cooldownMs,
        colorProbes,
        colorMatchMode: form.colorMatchMode ?? "all",
        colorMatchMethod: form.colorMatchMethod ?? "average",
    };
}

function parseAudioEffect(form: RecognitionCardForm): RecognitionAudioEffect {
    const volume = parseFloat(form.volume);
    if (Number.isNaN(volume) || volume < 0 || volume > 1) {
        throw new Error("音量必须在 0 到 1 之间。");
    }

    const playMode = form.playMode ?? "single";
    const audioFiles = (form.audioFiles ?? [])
        .map((f) => f.trim())
        .filter((f) => f.length > 0);
    if (audioFiles.length === 0) {
        throw new Error("请至少添加一个音频文件。");
    }
    if (audioFiles.length > 0 && playMode !== "single" && audioFiles.length < 2) {
        throw new Error("连杀或随机播放至少需要添加 2 个音频文件。");
    }

    let comboWindowMs = 60000;
    if (playMode === "combo") {
        const parsed = parseInt(form.comboWindowMs, 10);
        if (Number.isNaN(parsed) || parsed < 100 || parsed > 600000) {
            throw new Error("连杀窗口时间必须在 100 到 600000 毫秒之间。");
        }
        comboWindowMs = parsed;
    }

    let comboWindows: number[] = [];
    if (playMode === "combo") {
        const formWindows = form.comboWindows ?? [];
        comboWindows = audioFiles.map((_, i) => {
            const raw = formWindows[i] ?? "";
            const trimmed = raw.trim();
            if (trimmed === "") {
                return comboWindowMs;
            }
            const w = parseInt(trimmed, 10);
            if (Number.isNaN(w) || w < 100 || w > 600000) {
                throw new Error("连杀窗口时间必须在 100 到 600000 毫秒之间。");
            }
            return w;
        });
    }

    return {
        audioFiles,
        playMode,
        comboWindowMs,
        comboWindows,
        volume,
        allowSimultaneous: form.allowSimultaneous ?? false,
    };
}

function parseClickEffect(form: RecognitionCardForm, colorProbeCount: number): RecognitionClickEffect {
    const clickMode = form.clickMode ?? "customRegion";
    if (clickMode === "customRegion") {
        return {
            mode: "customRegion",
            customRegion: form.clickCustomRegion ?? null,
            colorProbeIndex: null,
        };
    }
    if (form.triggerMode === "hotkey") {
        throw new Error("快捷键触发的点击效果必须使用自定义区域。");
    }
    if (form.triggerMode === "colorWatch") {
        const index = parseInt(form.clickColorProbeIndex ?? "", 10);
        if (Number.isNaN(index) || index < 0 || index >= colorProbeCount) {
            throw new Error("识色点击效果必须选择有效探针。");
        }
        return {
            mode: "recognitionRegion",
            customRegion: null,
            colorProbeIndex: index,
        };
    }
    return {
        mode: "recognitionRegion",
        customRegion: null,
        colorProbeIndex: null,
    };
}

export function generateCardId(): string {
    return `recognition-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
}

export function createEmptyRecognitionCard(): RecognitionCard {
    return {
        ...DEFAULT_RECOGNITION_CARD,
        id: generateCardId(),
    };
}

export function mergeRecognitionWatchRegionsIntoForm(
    current: RecognitionSettingsForm | null,
    settings: RecognitionSettings,
): RecognitionSettingsForm {
    const nextForm = settingsToForm(settings);
    if (!current) {
        return nextForm;
    }
    const byId = new Map(nextForm.cards.map((card) => [card.id, card]));
    return {
        ...current,
        recognitionEnabled: nextForm.recognitionEnabled,
        audioEnabled: nextForm.audioEnabled,
        cards: current.cards.map((card) => {
            const remote = byId.get(card.id);
            if (!remote) {
                return card;
            }
            const mergedProbes = card.colorProbes.map((probe, i) => {
                const remoteProbe = remote.colorProbes[i];
                if (remoteProbe && remoteProbe.region) {
                    return {...probe, region: remoteProbe.region};
                }
                return probe;
            });
            return {
                ...card,
                watchRegion: remote.watchRegion,
                clickCustomRegion: remote.clickCustomRegion ?? card.clickCustomRegion,
                colorProbes: mergedProbes,
            };
        }),
    };
}

export function getRecognitionCardFormErrors(form: RecognitionCardForm): Record<string, string | null> {
    const errors: Record<string, string | null> = {};

    if (!form.name.trim()) {
        errors.name = "卡片名称不能为空";
    }

    const cooldownMs = parseInt(form.cooldownMs, 10);
    if (Number.isNaN(cooldownMs) || cooldownMs < 0 || cooldownMs > 60000) {
        errors.cooldownMs = "冷却时间必须在 0 到 60000 毫秒之间";
    }

    if (form.triggerMode === "hotkey" && !form.hotkey.trim()) {
        errors.hotkey = "必须设置触发快捷键";
    }

    if (form.triggerMode === "regionWatch" && !form.watchRegion) {
        errors.watchRegion = "必须设置监听区域";
    }

    if (form.triggerMode !== "hotkey" && form.activationMode !== "always" && !form.activationHotkey?.trim()) {
        errors.activationHotkey = "必须设置激活快捷键";
    }

    const hasExplicitEffects = form.audioEffectEnabled !== undefined
        || form.hotkeyEffectEnabled !== undefined
        || form.clickEffectEnabled !== undefined;
    const audioEffectEnabled = form.audioEffectEnabled ?? (!hasExplicitEffects && form.audioFiles !== undefined);
    if (!audioEffectEnabled && !form.hotkeyEffectEnabled && !form.clickEffectEnabled) {
        errors.effects = "至少启用一个触发效果";
    }

    if (audioEffectEnabled) {
        const audioFiles = (form.audioFiles ?? []).map((f) => f.trim()).filter((f) => f.length > 0);
        if (audioFiles.length === 0) {
            errors.audioFiles = "请至少添加一个音频文件";
        } else if ((form.playMode === "combo" || form.playMode === "random") && audioFiles.length < 2) {
            errors.audioFiles = "连杀或随机播放至少需要 2 个音频文件";
        }
        const volume = parseFloat(form.volume);
        if (Number.isNaN(volume) || volume < 0 || volume > 1) {
            errors.volume = "音量必须在 0 到 1 之间";
        }
    }

    if (form.hotkeyEffectEnabled && !form.effectHotkey?.trim()) {
        errors.effectHotkey = "必须设置按键效果快捷键";
    }

    if (form.clickEffectEnabled) {
        const clickMode = form.clickMode ?? "customRegion";
        if (clickMode === "customRegion" && !form.clickCustomRegion) {
            errors.clickCustomRegion = "必须框选自定义点击区域";
        }
        if (clickMode === "recognitionRegion" && form.triggerMode === "colorWatch" && (form.clickColorProbeIndex ?? "").trim() === "") {
            errors.clickColorProbeIndex = "必须选择识色探针";
        }
    }

    if (form.playMode === "combo") {
        const comboWindowMs = parseInt(form.comboWindowMs, 10);
        if (Number.isNaN(comboWindowMs) || comboWindowMs < 100 || comboWindowMs > 600000) {
            errors.comboWindowMs = "连杀窗口必须在 100 到 600000 毫秒之间";
        }
    }

    return errors;
}
