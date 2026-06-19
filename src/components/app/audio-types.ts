import type {RegionRect} from "@/components/app/morse-types";

export type ColorMatchMode = "all" | "any";

export type ColorProbe = {
    region: RegionRect;
    targetColor: [number, number, number];
    tolerance: number;
};

export type ColorProbeForm = {
    region: RegionRect | null;
    targetColor: string; // "#RRGGBB" 格式
    tolerance: string;   // 数字字符串，0-255
};

export type AudioTriggerMode = "hotkey" | "regionWatch" | "colorWatch";

/// 音频播放方式：叠加在触发模式之上的文件选择策略。
/// - single：单文件
/// - combo：连杀（窗口内按序递增，末首后保持，超时复位第一首）
/// - random：随机（不重复上一次）
export type AudioPlayMode = "single" | "combo" | "random";

export type AudioCard = {
    id: string;
    name: string;
    enabled: boolean;
    triggerMode: AudioTriggerMode;
    hotkey: string | null;
    watchRegion: RegionRect | null;
    watchReferenceImagePath: string | null;
    watchMatchThreshold: number;
    watchPollIntervalMs: number;
    audioFiles: string[];
    playMode: AudioPlayMode;
    comboWindowMs: number;
    volume: number;
    cooldownMs: number;
    allowSimultaneous: boolean;
    colorProbes: ColorProbe[];
    colorMatchMode: ColorMatchMode;
};

export type AudioSettings = {
    audioEnabled: boolean;
    cards: AudioCard[];
};

export type AudioBootstrap = {
    settings: AudioSettings;
    hotkeyError: string | null;
};

export type AudioSettingsForm = {
    audioEnabled: boolean;
    cards: AudioCardForm[];
};

export type AudioCardForm = {
    id: string;
    name: string;
    enabled: boolean;
    triggerMode: AudioTriggerMode;
    hotkey: string;
    watchRegion: RegionRect | null;
    watchReferenceImagePath: string;
    watchMatchThreshold: string;
    watchPollIntervalMs: string;
    audioFiles: string[];
    playMode: AudioPlayMode;
    comboWindowMs: string;
    volume: string;
    cooldownMs: string;
    allowSimultaneous: boolean;
    colorProbes: ColorProbeForm[];
    colorMatchMode: ColorMatchMode;
};

export const DEFAULT_AUDIO_CARD: AudioCard = {
    id: "",
    name: "",
    enabled: true,
    triggerMode: "hotkey",
    hotkey: null,
    watchRegion: null,
    watchReferenceImagePath: null,
    watchMatchThreshold: 0.75,
    watchPollIntervalMs: 500,
    audioFiles: [],
    playMode: "single",
    comboWindowMs: 60000,
    volume: 0.8,
    cooldownMs: 1000,
    allowSimultaneous: false,
    colorProbes: [],
    colorMatchMode: "all",
};

export const DEFAULT_AUDIO_SETTINGS: AudioSettings = {
    audioEnabled: true,
    cards: [],
};

export const AUDIO_AUTOSAVE_DELAY_MS = 400;
