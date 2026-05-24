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
  pressJitterMinMs: number;
  pressJitterMaxMs: number;
  enabled: boolean;
};

export type RapidfireSettings = {
  version: number;
  rapidfireEnabled: boolean;
  showOverlay: boolean;
  overlayPosition: RapidfireRect | null;
  overlayWidth: number;
  compensationDelayMinMs: number;
  compensationDelayMaxMs: number;
  minPressSpacingMs: number;
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
  pressJitterMinMs: string;
  pressJitterMaxMs: string;
  enabled: boolean;
};

export type RapidfireSettingsForm = {
  rapidfireEnabled: boolean;
  showOverlay: boolean;
  overlayWidth: string;
  compensationDelayMinMs: string;
  compensationDelayMaxMs: string;
  minPressSpacingMs: string;
  overlayPosition: RapidfireRect | null;
  cards: RapidfireCardForm[];
};

// ---- 常量 ----

export const RAPIDFIRE_AUTOSAVE_DELAY_MS = 400;
export const RAPIDFIRE_MIN_INTERVAL_MS = 10;
export const RAPIDFIRE_DISPLAY_MIN_WIDTH = 320;
export const RAPIDFIRE_DISPLAY_MAX_WIDTH = 800;
export const RAPIDFIRE_DEFAULT_INTERVAL_MS = 100;
export const RAPIDFIRE_PRESS_JITTER_MIN_MS = 1;
export const RAPIDFIRE_PRESS_JITTER_MAX_MS = 200;
export const RAPIDFIRE_DEFAULT_PRESS_JITTER_MIN_MS = 8;
export const RAPIDFIRE_DEFAULT_PRESS_JITTER_MAX_MS = 12;
export const RAPIDFIRE_GLOBAL_DELAY_MIN_MS = 0;
export const RAPIDFIRE_GLOBAL_DELAY_MAX_MS = 10000;
export const RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MIN_MS = 100;
export const RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MAX_MS = 150;
export const RAPIDFIRE_DEFAULT_MIN_PRESS_SPACING_MS = 80;

// ---- 转换函数 ----

export function rapidfireSettingsToForm(settings: RapidfireSettings): RapidfireSettingsForm {
  return {
    rapidfireEnabled: settings.rapidfireEnabled,
    showOverlay: settings.showOverlay,
    overlayWidth: String(settings.overlayWidth),
    compensationDelayMinMs: String(settings.compensationDelayMinMs ?? RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MIN_MS),
    compensationDelayMaxMs: String(settings.compensationDelayMaxMs ?? RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MAX_MS),
    minPressSpacingMs: String(settings.minPressSpacingMs ?? RAPIDFIRE_DEFAULT_MIN_PRESS_SPACING_MS),
    overlayPosition: settings.overlayPosition,
    cards: settings.cards.map((card) => ({
      id: card.id,
      name: card.name,
      triggerKey: card.triggerKey,
      targetKey: card.targetKey,
      intervalMs: String(card.intervalMs),
      pressJitterMinMs: String(card.pressJitterMinMs),
      pressJitterMaxMs: String(card.pressJitterMaxMs),
      enabled: card.enabled,
    })),
  };
}

const SUPPORTED_KEY_LABELS = new Set([
  ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ".split(""),
  ..."0123456789".split(""),
  ...Array.from({ length: 12 }, (_, index) => `F${index + 1}`),
  "Space",
  "Enter",
  "Tab",
  "Esc",
  "Backspace",
  "Up",
  "Down",
  "Left",
  "Right",
  "Home",
  "End",
  "PageUp",
  "PageDown",
  "Insert",
  "Delete",
  "Alt",
  ";",
  ",",
  ".",
  "/",
  "\\",
  "[",
  "]",
  "-",
  "=",
  "`",
  "'",
]);

function normalizePositiveInteger(value: string, fallback: number): number {
  const parsed = Number.parseInt(value, 10);
  return Number.isInteger(parsed) ? parsed : fallback;
}

function clampOverlayWidth(value: string): number {
  const parsed = normalizePositiveInteger(value, RAPIDFIRE_DISPLAY_MIN_WIDTH);
  return Math.max(RAPIDFIRE_DISPLAY_MIN_WIDTH, Math.min(RAPIDFIRE_DISPLAY_MAX_WIDTH, parsed));
}

function normalizeRapidfireKey(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  if (trimmed.includes("+")) return trimmed;

  const aliasMap: Record<string, string> = {
    " ": "Space",
    Spacebar: "Space",
    Escape: "Esc",
    escape: "Esc",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    arrowup: "Up",
    arrowdown: "Down",
    arrowleft: "Left",
    arrowright: "Right",
    Pageup: "PageUp",
    Pagedown: "PageDown",
    pageup: "PageUp",
    pagedown: "PageDown",
    Del: "Delete",
    del: "Delete",
    Semicolon: ";",
    semicolon: ";",
    Comma: ",",
    comma: ",",
    Period: ".",
    period: ".",
    Slash: "/",
    slash: "/",
    Backslash: "\\",
    backslash: "\\",
    BracketLeft: "[",
    bracketleft: "[",
    BracketRight: "]",
    bracketright: "]",
    Minus: "-",
    minus: "-",
    Equal: "=",
    equal: "=",
    Backquote: "`",
    backquote: "`",
    Quote: "'",
    quote: "'",
  };
  const aliased = aliasMap[raw] ?? aliasMap[trimmed] ?? trimmed;

  if (aliased.length === 1) {
    return aliased.toUpperCase();
  }

  const functionMatch = /^f([1-9]|1[0-2])$/i.exec(aliased);
  if (functionMatch) {
    return `F${functionMatch[1]}`;
  }

  const supported = Array.from(SUPPORTED_KEY_LABELS).find((key) => key.toLowerCase() === aliased.toLowerCase());
  return supported ?? aliased;
}

function validateRapidfireKey(key: string, label: string): void {
  if (!key) {
    throw new Error(`${label}不能为空。`);
  }

  if (key.includes("+")) {
    throw new Error(`${label}必须是单键，不能包含组合键。`);
  }

  if (!SUPPORTED_KEY_LABELS.has(key)) {
    throw new Error(`${label}不支持：${key}。`);
  }
}

export function parseRapidfireSettingsForm(form: RapidfireSettingsForm): RapidfireSettings {
  if (form.cards.length === 0) {
    throw new Error("至少需要保留一个连发器卡片。");
  }

  const cards = form.cards.map((card) => {
    const name = card.name.trim();
    if (!name) {
      throw new Error("连发器卡片名称不能为空。");
    }

    const triggerKey = normalizeRapidfireKey(card.triggerKey);
    const targetKey = normalizeRapidfireKey(card.targetKey);
    validateRapidfireKey(triggerKey, `${name} 的触发键`);
    validateRapidfireKey(targetKey, `${name} 的目标键`);

    const intervalMs = normalizePositiveInteger(card.intervalMs, RAPIDFIRE_DEFAULT_INTERVAL_MS);
    if (intervalMs < RAPIDFIRE_MIN_INTERVAL_MS) {
      throw new Error(`${name} 的连发间隔不能小于 ${RAPIDFIRE_MIN_INTERVAL_MS}ms。`);
    }
    const pressJitterMinMs = normalizePositiveInteger(card.pressJitterMinMs, RAPIDFIRE_DEFAULT_PRESS_JITTER_MIN_MS);
    const pressJitterMaxMs = normalizePositiveInteger(card.pressJitterMaxMs, RAPIDFIRE_DEFAULT_PRESS_JITTER_MAX_MS);
    if (pressJitterMinMs < RAPIDFIRE_PRESS_JITTER_MIN_MS || pressJitterMaxMs > RAPIDFIRE_PRESS_JITTER_MAX_MS) {
      throw new Error(`${name} 的触发抖动必须在 ${RAPIDFIRE_PRESS_JITTER_MIN_MS}-${RAPIDFIRE_PRESS_JITTER_MAX_MS}ms 之间。`);
    }
    if (pressJitterMinMs > pressJitterMaxMs) {
      throw new Error(`${name} 的触发抖动最小值不能大于最大值。`);
    }

    return {
      id: card.id,
      name,
      triggerKey,
      targetKey,
      intervalMs,
      pressJitterMinMs,
      pressJitterMaxMs,
      enabled: card.enabled,
    };
  });

  const compensationDelayMinMs = normalizePositiveInteger(
    form.compensationDelayMinMs,
    RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MIN_MS,
  );
  const compensationDelayMaxMs = normalizePositiveInteger(
    form.compensationDelayMaxMs,
    RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MAX_MS,
  );
  const minPressSpacingMs = normalizePositiveInteger(
    form.minPressSpacingMs,
    RAPIDFIRE_DEFAULT_MIN_PRESS_SPACING_MS,
  );

  if (
    compensationDelayMinMs < RAPIDFIRE_GLOBAL_DELAY_MIN_MS ||
    compensationDelayMaxMs > RAPIDFIRE_GLOBAL_DELAY_MAX_MS
  ) {
    throw new Error(`补齐延迟必须在 ${RAPIDFIRE_GLOBAL_DELAY_MIN_MS}-${RAPIDFIRE_GLOBAL_DELAY_MAX_MS}ms 之间。`);
  }
  if (compensationDelayMinMs > compensationDelayMaxMs) {
    throw new Error("补齐延迟最小值不能大于最大值。");
  }
  if (
    minPressSpacingMs < RAPIDFIRE_GLOBAL_DELAY_MIN_MS ||
    minPressSpacingMs > RAPIDFIRE_GLOBAL_DELAY_MAX_MS
  ) {
    throw new Error(`按键最小间距必须在 ${RAPIDFIRE_GLOBAL_DELAY_MIN_MS}-${RAPIDFIRE_GLOBAL_DELAY_MAX_MS}ms 之间。`);
  }

  return {
    version: 1,
    rapidfireEnabled: form.rapidfireEnabled,
    showOverlay: form.showOverlay,
    overlayPosition: form.overlayPosition,
    overlayWidth: clampOverlayWidth(form.overlayWidth),
    compensationDelayMinMs,
    compensationDelayMaxMs,
    minPressSpacingMs,
    cards,
  };
}

export function createRapidfireCard(id: string, existingCount = 0): RapidfireCardForm {
  const triggerKey = `F${Math.min(12, 6 + existingCount)}`;

  return {
    id,
    name: `连发器 ${existingCount + 1}`,
    triggerKey,
    targetKey: "Space",
    intervalMs: String(RAPIDFIRE_DEFAULT_INTERVAL_MS),
    pressJitterMinMs: String(RAPIDFIRE_DEFAULT_PRESS_JITTER_MIN_MS),
    pressJitterMaxMs: String(RAPIDFIRE_DEFAULT_PRESS_JITTER_MAX_MS),
    enabled: false,
  };
}

export function isRapidfireDirty(
  bootstrap: RapidfireBootstrap | null,
  form: RapidfireSettingsForm | null,
): boolean {
  if (bootstrap === null || form === null) return false;
  try {
    const current = parseRapidfireSettingsForm(form);
    return JSON.stringify(rapidfireSettingsToForm(current)) !== JSON.stringify(rapidfireSettingsToForm(bootstrap.settings));
  } catch {
    return true;
  }
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
      return "补齐中";
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

export type RapidfireCardStatusView = {
  label: string;
  variant: "outline" | "default" | "secondary" | "destructive";
  active: boolean;
  error: boolean;
};

function errorIncludesAny(error: string, values: string[]): boolean {
  return values.some((value) => value !== "" && error.includes(value));
}

export function rapidfireCardError(
  card: Pick<RapidfireCardForm, "name" | "triggerKey" | "targetKey">,
  pageError: string | null,
): string | null {
  if (!pageError) return null;

  const name = card.name.trim();
  const triggerKey = normalizeRapidfireKey(card.triggerKey);
  const targetKey = normalizeRapidfireKey(card.targetKey);

  if (name && pageError.includes(name)) {
    return pageError;
  }

  if (!name && pageError.includes("名称不能为空")) {
    return pageError;
  }

  const triggerPatterns = [
    `触发键 ${triggerKey}`,
    `触发键${triggerKey}`,
    `触发键不支持：${triggerKey}`,
    `触发键不支持: ${triggerKey}`,
  ];
  if (errorIncludesAny(pageError, triggerPatterns)) {
    return pageError;
  }

  const targetPatterns = [
    `目标键 ${targetKey}`,
    `目标键${targetKey}`,
    `目标键不支持：${targetKey}`,
    `目标键不支持: ${targetKey}`,
  ];
  if (errorIncludesAny(pageError, targetPatterns)) {
    return pageError;
  }

  return null;
}

export function rapidfireCardStatus(
  card: Pick<RapidfireCardForm, "enabled">,
  run: RapidfireRunState | undefined,
  cardError: string | null,
): RapidfireCardStatusView {
  if (cardError) {
    return {
      label: "配置未生效",
      variant: "destructive",
      active: false,
      error: true,
    };
  }

  if (run?.status === "firing") {
    return {
      label: `连发中 · ${run.count}`,
      variant: "default",
      active: true,
      error: false,
    };
  }

  if (run?.status === "pendingCompensation") {
    return {
      label: `补齐中 · ${run.count}`,
      variant: "secondary",
      active: true,
      error: false,
    };
  }

  if (!card.enabled) {
    return {
      label: "未启用",
      variant: "outline",
      active: false,
      error: false,
    };
  }

  return {
    label: "空闲",
    variant: "outline",
    active: false,
    error: false,
  };
}

export function moveRapidfireCard<T extends { id: string }>(items: T[], activeId: string, overId: string): T[] {
  if (activeId === overId) return items;

  const activeIndex = items.findIndex((item) => item.id === activeId);
  const overIndex = items.findIndex((item) => item.id === overId);
  if (activeIndex === -1 || overIndex === -1) return items;

  const next = [...items];
  const [moved] = next.splice(activeIndex, 1);
  next.splice(overIndex, 0, moved);
  return next;
}

export function rapidfireEnabledCards(settings: RapidfireSettingsForm | RapidfireSettings | null): number {
  return settings?.cards.filter((card) => card.enabled).length ?? 0;
}

// ---- 支持的目标键列表 ----

export const SUPPORTED_TARGET_KEYS = [
  { group: "字母键", keys: "A-Z".split("").map((k) => k) },
  { group: "数字键", keys: "0-9".split("").map((k) => k) },
  { group: "功能键", keys: Array.from({ length: 12 }, (_, i) => `F${i + 1}`) },
  {
    group: "修饰键",
    keys: ["Alt"],
  },
  {
    group: "符号键",
    keys: [";", ",", ".", "/", "\\", "[", "]", "-", "=", "`", "'"],
  },
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
  return normalizeRapidfireKey(raw);
}
