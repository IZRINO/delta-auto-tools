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


export type ClickRegion = {
  rect: RegionRect;
  /** 该点击区域的独立延迟（毫秒） */
  delayMs: number;
};

export type MorseSettings = {
  hotkey: string;
  regions: RegionTuple;
  binaryThreshold: number;
  autoInputDelay: number;
  /** 自动点击整组成功完成后按一次；空值表示不执行 */
  afterClickHotkey?: string | null;
  /** 识别成功后自动点击已配置区域 */
  autoClickEnabled: boolean;
  /** 点击区域（最多 7 个），每个区域独立延迟 */
  clickRegions: ClickRegion[];
};

export type MorseSettingsForm = {
  hotkey: string;
  regions: RegionTuple;
  binaryThreshold: string;
  autoInputDelay: string;
  /** 自动点击整组成功完成后按一次；空字符串表示不执行 */
  afterClickHotkey: string;
  /** 识别成功后自动点击已配置区域 */
  autoClickEnabled: boolean;
  /** 点击区域（最多 7 个），每个区域独立延迟字符串 */
  clickRegions: { rect: RegionRect | null; delayMs: string }[];
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
  target: "sampling" | "click" | string;
  clickRegions?: ClickRegion[] | null;
};

export type RegionSelectionKind = "selected" | "cancelled" | "closed";

export type RegionSelectionOutcome = {
  kind: RegionSelectionKind;
  regions: RegionTuple;
  target: string;
  /** 点击区域的完整选择结果，仅当 target === "click" 时有值 */
  clickRegions?: ClickRegion[] | null;
};

export type Point = {
  x: number;
  y: number;
};

export const REGION_LABELS = ["位置 1", "位置 2", "位置 3"] as const;
export const CLICK_REGION_LABELS = ["点击区域 1", "点击区域 2", "点击区域 3", "点击区域 4", "点击区域 5", "点击区域 6", "点击区域 7"] as const;
export const MIN_SELECTION_WIDTH = 10;
export const MIN_SELECTION_HEIGHT = 5;
export const EMPTY_REGIONS: RegionTuple = [null, null, null];
export const HOTKEY_MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta"]);
export const AUTOSAVE_DELAY_MS = 400;
