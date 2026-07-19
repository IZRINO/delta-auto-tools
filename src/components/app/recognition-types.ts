import type {RegionRect} from "@/components/app/morse-types";

export type ColorMatchMode = "all" | "any";

export type ColorMatchMethod = "average" | "anyPixel";

export type ColorTarget = {
    color: [number, number, number];
    tolerance: number;
};

export type ColorTargetForm = {
    color: string;
    tolerance: string;
};

export type ColorProbe = {
    region: RegionRect | null;
    targets: ColorTarget[];
    probeMatchMode: ColorMatchMode;
};

export type ColorProbeForm = {
    region: RegionRect | null;
    targets: ColorTargetForm[];
    probeMatchMode: ColorMatchMode;
};

export type RecognitionTriggerMode = "hotkey" | "regionWatch" | "colorWatch";

export type RecognitionHotkeyRepeatMode = "once" | "whileHeld";

export type RecognitionPlayMode = "single" | "combo" | "random";

export type RecognitionActivationMode = "always" | "onceHotkey" | "timedHotkey";

export type RecognitionActivation = {
    mode: RecognitionActivationMode;
    hotkey: string | null;
    durationMs: number;
    triggerCount: number;
};

export type RecognitionAudioEffect = {
    audioFiles: string[];
    playMode: RecognitionPlayMode;
    comboWindowMs: number;
    comboWindows: number[];
    volume: number;
    allowSimultaneous: boolean;
};

export type RecognitionHotkeyEffectStep = {
    hotkey: string;
    delayMs: number;
};

export type RecognitionHotkeyEffect = {
    hotkey?: string;
    steps?: RecognitionHotkeyEffectStep[];
};

export type RecognitionClickMode = "customRegion" | "recognitionRegion";

export type RecognitionClickEffect = {
    mode: RecognitionClickMode;
    customRegion: RegionRect | null;
    colorProbeIndex: number | null;
};

export type RecognitionGroup = {
    id: string;
    name: string;
    order: number;
    collapsed: boolean;
    enabled: boolean;
};

export type RecognitionEffects = {
    audio?: RecognitionAudioEffect | null;
    hotkey?: RecognitionHotkeyEffect | null;
    click?: RecognitionClickEffect | null;
};

export type RecognitionCard = {
    id: string;
    groupId?: string | null;
    order?: number | null;
    name: string;
    enabled: boolean;
    triggerMode: RecognitionTriggerMode;
    hotkey: string | null;
    hotkeyRepeatMode?: RecognitionHotkeyRepeatMode;
    watchRegion: RegionRect | null;
    watchReferenceImagePaths?: string[];
    /** 旧配置兼容；加载时迁移到 watchReferenceImagePaths。 */
    watchReferenceImagePath?: string | null;
    watchMatchThreshold: number;
    watchPollIntervalMs: number;
    retriggerAfterDisappear?: boolean;
    activation?: RecognitionActivation | null;
    effects?: RecognitionEffects | null;
    cooldownMs: number;
    colorProbes: ColorProbe[];
    colorMatchMode: ColorMatchMode;
    colorMatchMethod: ColorMatchMethod;

    // 旧 settings 输入兼容：settingsToForm 迁移到 effects.audio；parseSettingsForm 不输出这些字段。
    audioFiles?: string[];
    playMode?: RecognitionPlayMode;
    comboWindowMs?: number;
    comboWindows?: number[];
    volume?: number;
    allowSimultaneous?: boolean;
};

export type RecognitionSettings = {
    recognitionEnabled?: boolean;
    audioEnabled?: boolean;
    cardGroups?: RecognitionGroup[];
    cards: RecognitionCard[];
};

export type RecognitionBootstrap = {
    settings: RecognitionSettings;
    hotkeyError: string | null;
};

export type RecognitionSettingsForm = {
    recognitionEnabled?: boolean;
    audioEnabled: boolean;
    cardGroups?: RecognitionGroup[];
    cards: RecognitionCardForm[];
};

export type RecognitionCardForm = {
    id: string;
    groupId?: string | null;
    order?: number | null;
    collapsed?: boolean;
    name: string;
    enabled: boolean;
    triggerMode: RecognitionTriggerMode;
    hotkey: string;
    hotkeyRepeatMode?: RecognitionHotkeyRepeatMode;
    watchRegion: RegionRect | null;
    watchReferenceImagePaths?: string[];
    /** 旧表单兼容；保存时迁移到 watchReferenceImagePaths。 */
    watchReferenceImagePath?: string;
    watchMatchThreshold: string;
    watchPollIntervalMs: string;
    retriggerAfterDisappear?: boolean;
    activationMode?: RecognitionActivationMode;
    activationHotkey?: string;
    activationDurationMs?: string;
    activationTriggerCount?: string;
    audioEffectEnabled?: boolean;
    hotkeyEffectEnabled?: boolean;
    clickEffectEnabled?: boolean;
    effectHotkey?: string;
    hotkeyEffectSteps?: { hotkey: string; delayMs: string }[];
    clickMode?: RecognitionClickMode;
    clickCustomRegion?: RegionRect | null;
    clickColorProbeIndex?: string;
    audioFiles: string[];
    playMode: RecognitionPlayMode;
    comboWindowMs: string;
    comboWindows?: string[];
    volume: string;
    cooldownMs: string;
    allowSimultaneous: boolean;
    colorProbes: ColorProbeForm[];
    colorMatchMode: ColorMatchMode;
    colorMatchMethod: ColorMatchMethod;
};

export const DEFAULT_RECOGNITION_CARD: RecognitionCard = {
    id: "",
    groupId: null,
    order: 0,
    name: "",
    enabled: true,
    triggerMode: "hotkey",
    hotkey: null,
    hotkeyRepeatMode: "once",
    watchRegion: null,
    watchReferenceImagePaths: [],
    watchMatchThreshold: 0.75,
    watchPollIntervalMs: 500,
    retriggerAfterDisappear: false,
    activation: {mode: "always", hotkey: null, durationMs: 10000, triggerCount: 1},
    effects: {
        audio: {
            audioFiles: [],
            playMode: "single",
            comboWindowMs: 60000,
            comboWindows: [],
            volume: 0.8,
            allowSimultaneous: false,
        },
    },
    cooldownMs: 1000,
    colorProbes: [],
    colorMatchMode: "all",
    colorMatchMethod: "average",
};

export const DEFAULT_RECOGNITION_SETTINGS: RecognitionSettings = {
    recognitionEnabled: true,
    cardGroups: [],
    cards: [],
};

export const RECOGNITION_AUTOSAVE_DELAY_MS = 400;
