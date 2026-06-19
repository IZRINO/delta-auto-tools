import type {AudioCard, AudioCardForm, AudioSettings, AudioSettingsForm, ColorProbe, ColorProbeForm,} from "@/components/app/audio-types";
import {DEFAULT_AUDIO_CARD} from "@/components/app/audio-types";

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
        targetColor: rgbToHex(probe.targetColor),
        tolerance: String(probe.tolerance),
    };
}

function parseProbeForm(form: ColorProbeForm): ColorProbe {
    if (!form.region) {
        throw new Error("识色探针必须设置区域。");
    }
    const tolerance = parseInt(form.tolerance, 10);
    if (Number.isNaN(tolerance) || tolerance < 0 || tolerance > 255) {
        throw new Error("颜色容差必须在 0 到 255 之间。");
    }
    return {
        region: form.region,
        targetColor: hexToRgb(form.targetColor),
        tolerance,
    };
}

export function settingsToForm(settings: AudioSettings): AudioSettingsForm {
    return {
        audioEnabled: settings.audioEnabled,
        cards: settings.cards.map((card) => cardToForm(card)),
    };
}

function cardToForm(card: AudioCard): AudioCardForm {
    return {
        id: card.id,
        name: card.name,
        enabled: card.enabled,
        triggerMode: card.triggerMode,
        hotkey: card.hotkey ?? "",
        watchRegion: card.watchRegion,
        watchReferenceImagePath: card.watchReferenceImagePath ?? "",
        watchMatchThreshold: String(card.watchMatchThreshold),
        watchPollIntervalMs: String(card.watchPollIntervalMs),
        audioFiles: card.audioFiles ?? [],
        playMode: card.playMode ?? "single",
        comboWindowMs: String(card.comboWindowMs ?? 60000),
        volume: String(card.volume),
        cooldownMs: String(card.cooldownMs),
        allowSimultaneous: card.allowSimultaneous ?? false,
        colorProbes: (card.colorProbes ?? []).map(probeToForm),
        colorMatchMode: card.colorMatchMode ?? "all",
    };
}

export function parseSettingsForm(form: AudioSettingsForm): AudioSettings {
    const cards = form.cards.map((card) => parseCardForm(card));
    return {
        audioEnabled: form.audioEnabled,
        cards,
    };
}

function parseCardForm(form: AudioCardForm): AudioCard {
    const name = form.name.trim();
    if (!name) {
        throw new Error("卡片名称不能为空。");
    }

    const volume = parseFloat(form.volume);
    if (Number.isNaN(volume) || volume < 0 || volume > 1) {
        throw new Error("音量必须在 0 到 1 之间。");
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
        throw new Error("快捷键模式下必须设置热键。");
    }

    const colorProbes = form.triggerMode === "colorWatch"
        ? form.colorProbes.map(parseProbeForm)
        : [];
    if (form.triggerMode === "colorWatch" && colorProbes.length === 0) {
        throw new Error("识色模式下至少需要配置一个探针。");
    }
    const colorMatchMode = form.colorMatchMode ?? "all";

    // 播放方式校验
    const playMode = form.playMode ?? "single";
    const audioFiles = (form.audioFiles ?? [])
        .map((f) => f.trim())
        .filter((f) => f.length > 0);
    if (audioFiles.length === 0) {
        throw new Error("音频文件不能为空，请至少添加一个音频文件。");
    }
    if (playMode !== "single" && audioFiles.length < 2) {
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

    return {
        id: form.id || generateCardId(),
        name,
        enabled: form.enabled,
        triggerMode: form.triggerMode,
        hotkey,
        watchRegion: form.triggerMode === "regionWatch" ? form.watchRegion : null,
        watchReferenceImagePath: form.triggerMode === "regionWatch" ? form.watchReferenceImagePath.trim() || null : null,
        watchMatchThreshold,
        watchPollIntervalMs,
        audioFiles,
        playMode,
        comboWindowMs,
        volume,
        cooldownMs,
        allowSimultaneous: form.allowSimultaneous ?? false,
        colorProbes,
        colorMatchMode,
    };
}

export function generateCardId(): string {
    return `audio-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
}

export function createEmptyAudioCard(): AudioCard {
    return {
        ...DEFAULT_AUDIO_CARD,
        id: generateCardId(),
    };
}

export function mergeAudioWatchRegionsIntoForm(
    current: AudioSettingsForm | null,
    settings: AudioSettings,
): AudioSettingsForm {
    const nextForm = settingsToForm(settings);
    if (!current) {
        return nextForm;
    }
    const byId = new Map(nextForm.cards.map((card) => [card.id, card]));
    return {
        ...current,
        cards: current.cards.map((card) => {
            const remote = byId.get(card.id);
            if (!remote) {
                return card;
            }
            // 识色探针区域回写：仅同步后端有值的探针 region 坐标，
            // 本地 targetColor/tolerance 草稿保留不被覆盖。
            const mergedProbes = card.colorProbes.map((probe, i) => {
                const remoteProbe = remote.colorProbes[i];
                if (remoteProbe && remoteProbe.region) {
                    return {...probe, region: remoteProbe.region};
                }
                return probe;
            });
            return {...card, watchRegion: remote.watchRegion, colorProbes: mergedProbes};
        }),
    };
}

export function getAudioCardFormErrors(form: AudioCardForm): Record<string, string | null> {
    const errors: Record<string, string | null> = {};

    if (!form.name.trim()) {
        errors.name = "卡片名称不能为空";
    }

    const volume = parseFloat(form.volume);
    if (Number.isNaN(volume) || volume < 0 || volume > 1) {
        errors.volume = "音量必须在 0 到 1 之间";
    }

    const cooldownMs = parseInt(form.cooldownMs, 10);
    if (Number.isNaN(cooldownMs) || cooldownMs < 0 || cooldownMs > 60000) {
        errors.cooldownMs = "冷却时间必须在 0 到 60000 毫秒之间";
    }

    if (form.triggerMode === "hotkey" && !form.hotkey.trim()) {
        errors.hotkey = "必须设置快捷键";
    }

    if (form.triggerMode === "regionWatch" && !form.watchRegion) {
        errors.watchRegion = "必须设置监听区域";
    }

    const audioFiles = (form.audioFiles ?? []).map((f) => f.trim()).filter((f) => f.length > 0);
    if (audioFiles.length === 0) {
        errors.audioFiles = "请至少添加一个音频文件";
    } else if ((form.playMode === "combo" || form.playMode === "random") && audioFiles.length < 2) {
        errors.audioFiles = "连杀或随机播放至少需要 2 个音频文件";
    }

    if (form.playMode === "combo") {
        const comboWindowMs = parseInt(form.comboWindowMs, 10);
        if (Number.isNaN(comboWindowMs) || comboWindowMs < 100 || comboWindowMs > 600000) {
            errors.comboWindowMs = "连杀窗口必须在 100 到 600000 毫秒之间";
        }
    }

    return errors;
}
