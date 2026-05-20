export type RapidfireRect = {
  x: number;
  y: number;
};

export type RapidfireCard = {
  id: string;
  name: string;
  triggerKey: string;
  targetKey: string;
  intervalMs: number;
  enabled: boolean;
};

export type RapidfireSettings = {
  version: number;
  rapidfireEnabled: boolean;
  showOverlay: boolean;
  overlayPosition: RapidfireRect | null;
  overlayWidth: number;
  cards: RapidfireCard[];
};

export type RapidfireRunStatus = "idle" | "firing" | "pendingCompensation";

export type RapidfireRunState = {
  cardId: string;
  status: RapidfireRunStatus;
  count: number;
};

export type RapidfireBootstrap = {
  settings: RapidfireSettings;
  runs: RapidfireRunState[];
  hotkeyError: string | null;
};

export type RapidfireSelectionOutcome = {
  kind: "selected" | "cancelled" | "closed";
  position: RapidfireRect;
};

// ---- 表单类型 ----

export type RapidfireCardForm = {
  id: string;
  name: string;
  triggerKey: string;
  targetKey: string;
  intervalMs: string; // 字符串用于输入框
  enabled: boolean;
};

export type RapidfireSettingsForm = {
  rapidfireEnabled: boolean;
  showOverlay: boolean;
  overlayWidth: string;
  overlayPosition: RapidfireRect | null;
  cards: RapidfireCardForm[];
};

// ---- 常量 ----

export const RAPIDFIRE_AUTOSAVE_DELAY_MS = 400;
export const RAPIDFIRE_MIN_INTERVAL_MS = 10;
export const RAPIDFIRE_DISPLAY_MIN_WIDTH = 320;
export const RAPIDFIRE_DISPLAY_MAX_WIDTH = 800;

// ---- 转换函数 ----

export function rapidfireSettingsToForm(settings: RapidfireSettings): RapidfireSettingsForm {
  return {
    rapidfireEnabled: settings.rapidfireEnabled,
    showOverlay: settings.showOverlay,
    overlayWidth: String(settings.overlayWidth),
    overlayPosition: settings.overlayPosition,
    cards: settings.cards.map((card) => ({
      id: card.id,
      name: card.name,
      triggerKey: card.triggerKey,
      targetKey: card.targetKey,
      intervalMs: String(card.intervalMs),
      enabled: card.enabled,
    })),
  };
}

export function parseRapidfireSettingsForm(form: RapidfireSettingsForm): RapidfireSettings {
  return {
    version: 1,
    rapidfireEnabled: form.rapidfireEnabled,
    showOverlay: form.showOverlay,
    overlayWidth: parseInt(form.overlayWidth, 10) || RAPIDFIRE_DISPLAY_MIN_WIDTH,
    overlayPosition: form.overlayPosition,
    cards: form.cards.map((card) => ({
      id: card.id,
      name: card.name.trim(),
      triggerKey: card.triggerKey.trim(),
      targetKey: card.targetKey.trim(),
      intervalMs: parseInt(card.intervalMs, 10) || RAPIDFIRE_MIN_INTERVAL_MS,
      enabled: card.enabled,
    })),
  };
}

export function createRapidfireCard(id: string): RapidfireCardForm {
  return {
    id,
    name: "",
    triggerKey: "",
    targetKey: "",
    intervalMs: "100",
    enabled: false,
  };
}

export function isRapidfireDirty(
  bootstrap: RapidfireBootstrap | null,
  form: RapidfireSettingsForm | null,
): boolean {
  if (bootstrap === null || form === null) return false;
  const current = parseRapidfireSettingsForm(form);
  return JSON.stringify(current) !== JSON.stringify(bootstrap.settings);
}

export function rapidfireRunsById(
  runs: RapidfireRunState[],
): Map<string, RapidfireRunState> {
  const map = new Map<string, RapidfireRunState>();
  for (const run of runs) {
    map.set(run.cardId, run);
  }
  return map;
}

// ---- 状态徽章文案 ----

export function rapidfireStatusLabel(status: RapidfireRunStatus): string {
  switch (status) {
    case "idle":
      return "空闲";
    case "firing":
      return "连发中";
    case "pendingCompensation":
      return "补齐等待";
  }
}

export function rapidfireStatusVariant(
  status: RapidfireRunStatus,
): "outline" | "default" | "secondary" {
  switch (status) {
    case "idle":
      return "outline";
    case "firing":
      return "default";
    case "pendingCompensation":
      return "secondary";
  }
}

// ---- 支持的目标键列表 ----

export const SUPPORTED_TARGET_KEYS = [
  { group: "字母键", keys: "A-Z".split("").map((k) => k) },
  { group: "数字键", keys: "0-9".split("").map((k) => k) },
  { group: "功能键", keys: Array.from({ length: 12 }, (_, i) => `F${i + 1}`) },
  {
    group: "特殊键",
    keys: ["Space", "Enter", "Tab", "Esc", "Backspace"],
  },
  {
    group: "方向键",
    keys: ["Up", "Down", "Left", "Right"],
  },
  {
    group: "其他",
    keys: ["Home", "End", "PageUp", "PageDown", "Insert", "Delete"],
  },
];

// ---- 热键格式化（单键，不含修饰键） ----

export function formatTriggerKey(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  return trimmed;
}
