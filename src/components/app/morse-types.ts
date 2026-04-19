export type MorsePageProps = {
  overlayMode?: boolean;
};

export type RegionRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type RegionTuple = [RegionRect | null, RegionRect | null, RegionRect | null];

export type MorseSettings = {
  hotkey: string;
  regions: RegionTuple;
  binaryThreshold: number;
  autoInputDelay: number;
};

export type MorseSettingsForm = {
  hotkey: string;
  regions: RegionTuple;
  binaryThreshold: string;
  autoInputDelay: string;
};

export type VerificationStatus = "idle" | "running" | "success" | "empty" | "error";

export type MorseRegionDetail = {
  slot: number;
  thresholdMode: string;
  contourCount: number;
  morse: string | null;
  digit: string | null;
  error: string | null;
};

export type MorseRunResult = {
  value: string | null;
  details: MorseRegionDetail[];
  triggeredBy: string;
  autoTyped: boolean;
  occurredAtMs: number;
  error: string | null;
};

export type HistoryEntry = {
  id: number;
  result: string | null;
  success: boolean;
  triggeredBy: string;
  autoTyped: boolean;
  occurredAtMs: number;
  error: string | null;
};

export type MorseBootstrap = {
  settings: MorseSettings;
  history: HistoryEntry[];
  latestRun: MorseRunResult | null;
  hotkeyError: string | null;
};

export type RegionSelectionProgress = {
  currentSlot: number | null;
  regions: RegionTuple;
  completedSlots: number[];
};

export type RegionSelectionOutcome = {
  kind: "selected" | "cancelled" | "closed";
  regions: RegionTuple;
};

export type Point = {
  x: number;
  y: number;
};

export const REGION_LABELS = ["位置 1", "位置 2", "位置 3"] as const;
export const MIN_SELECTION_WIDTH = 10;
export const MIN_SELECTION_HEIGHT = 5;
export const EMPTY_REGIONS: RegionTuple = [null, null, null];
export const HOTKEY_MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta"]);
export const AUTOSAVE_DELAY_MS = 400;
