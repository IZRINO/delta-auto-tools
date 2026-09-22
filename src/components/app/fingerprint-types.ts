import type {ClickRegion, RegionRect} from "@/components/app/morse-types";

export type {ClickRegion, RegionRect};

export const AUTOSAVE_DELAY_MS = 400;

export const CANDIDATE_LABELS = ["候选1", "候选2", "候选3", "候选4", "候选5", "候选6", "候选7", "候选8", "候选9"] as const;
export const ARCHIVE_LABELS = ["档案1", "档案2", "档案3", "档案4", "档案5", "档案6", "档案7", "档案8"] as const;

export type LayoutTarget = "name" | "candidates" | "archive" | "click";

export type FingerprintPerson = {
    id: string;
    name: string;
    nameImagePath: string;
    fingerprintPaths: Array<string | null>;
};

export type FingerprintSettings = {
    hotkey: string;
    nameRegion: RegionRect | null;
    candidateBoxes: Array<RegionRect | null>;
    archiveSlots: Array<RegionRect | null>;
    occupancyThreshold: number;
    matchThreshold: number;
    autoClickEnabled: boolean;
    clickDelayMs: number;
    afterClickHotkey?: string | null;
    clickRegions: ClickRegion[];
    people: FingerprintPerson[];
};

export type FingerprintSettingsForm = {
    hotkey: string;
    occupancyThreshold: string;
    matchThreshold: string;
    autoClickEnabled: boolean;
    clickDelayMs: string;
    afterClickHotkey: string;
    clickRegions: {rect: RegionRect | null; delayMs: string}[];
    nameRegion: RegionRect | null;
    candidateBoxes: Array<RegionRect | null>;
    archiveSlots: Array<RegionRect | null>;
    people: FingerprintPerson[];
};

export type FingerprintMatch = {
    templateIndex: number;
    candidateIndex: number;
    score: number;
};

export type FingerprintRunResult = {
    personId: string | null;
    personName: string | null;
    mode: string | null;
    occupiedCount: number | null;
    matches: FingerprintMatch[];
    clicked: boolean;
    triggeredBy: string;
    occurredAtMs: number;
    error: string | null;
};

export type HistoryEntry = {
    id: number;
    personName: string | null;
    mode: string | null;
    success: boolean;
    triggeredBy: string;
    occurredAtMs: number;
    error: string | null;
};

export type FingerprintBootstrap = {
    settings: FingerprintSettings;
    history: HistoryEntry[];
    latestRun: FingerprintRunResult | null;
    hotkeyError: string | null;
};

export type RegionSelectionProgress = {
    currentSlot: number | null;
    completedSlots: number[];
    target: string;
    rects: Array<RegionRect | null>;
};

export type RegionSelectionOutcome = {
    kind: "selected" | "cancelled" | "closed";
    target: string;
    rects: Array<RegionRect | null>;
};

export type FingerprintPageProps = {
    overlayMode?: boolean;
};
