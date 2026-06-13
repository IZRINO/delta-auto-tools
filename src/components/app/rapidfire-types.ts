export type RapidfireRect = {
  x: number;
  y: number;
};

export type RapidfireGroup = {
  id: string;
  name: string;
  enabled: boolean;
  showOverlay: boolean;
  overlayPosition: RapidfireRect | null;
  overlayWidth: number;
};

export type RapidfireCard = {
  id: string;
  groupId?: string;
  name: string;
  triggerKey: string;
  targetKey: string;
  intervalMs: number;
  pressJitterMinMs: number;
  pressJitterMaxMs: number;
  minPressSpacingMs: number;
  triggerJitterMaxMs: number;
  cancelJitterOnRelease: boolean;
  enabled: boolean;
  skipCompensation: boolean;
  ignoreTriggerKey: boolean;
};

export type RapidfireSettings = {
  version: number;
  rapidfireEnabled: boolean;
  showOverlay: boolean;
  overlayPosition: RapidfireRect | null;
  overlayWidth: number;
  compensationDelayMinMs: number;
  compensationDelayMaxMs: number;
  /** 旧配置兼容：旧全局按键间距；新 UI 按卡片保存 */
  minPressSpacingMs?: number;
  /** 旧配置兼容：旧全局启动抖动延迟；新 UI 按卡片保存 */
  triggerJitterMaxMs?: number;
  /** 旧配置兼容：旧全局抖动期间松手策略；新 UI 按卡片保存 */
  cancelJitterOnRelease?: boolean;
  groups?: RapidfireGroup[];
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
  groupId?: string | null;
};

// ---- 表单类型 ----

export type RapidfireCardForm = {
  id: string;
  groupId: string;
  name: string;
  triggerKey: string;
  targetKey: string;
  intervalMs: string; // 字符串用于输入框
  pressJitterMinMs: string;
  pressJitterMaxMs: string;
  minPressSpacingMs: string;
  triggerJitterMaxMs: string;
  cancelJitterOnRelease: boolean;
  enabled: boolean;
  skipCompensation: boolean;
  ignoreTriggerKey: boolean;
};

export type RapidfireGroupForm = {
  id: string;
  name: string;
  enabled: boolean;
  showOverlay: boolean;
  overlayPosition: RapidfireRect | null;
  overlayWidth: string;
};

export type RapidfireSettingsForm = {
  rapidfireEnabled: boolean;
  showOverlay: boolean;
  overlayWidth: string;
  compensationDelayMinMs: string;
  compensationDelayMaxMs: string;
  /** 旧配置兼容字段不在表单中展示；解析时写回默认值给 Rust 兼容层 */
  overlayPosition: RapidfireRect | null;
  groups: RapidfireGroupForm[];
  cards: RapidfireCardForm[];
};

// ---- 常量 ----

export const RAPIDFIRE_AUTOSAVE_DELAY_MS = 400;
export const RAPIDFIRE_MIN_INTERVAL_MS = 1;
export const RAPIDFIRE_DISPLAY_MIN_WIDTH = 320;
export const RAPIDFIRE_DISPLAY_MAX_WIDTH = 800;
export const RAPIDFIRE_DEFAULT_INTERVAL_MS = 100;
export const RAPIDFIRE_PRESS_JITTER_MIN_MS = 1;
export const RAPIDFIRE_PRESS_JITTER_MAX_MS = 2000;
export const RAPIDFIRE_DEFAULT_PRESS_JITTER_MIN_MS = 8;
export const RAPIDFIRE_DEFAULT_PRESS_JITTER_MAX_MS = 12;
export const RAPIDFIRE_GLOBAL_DELAY_MIN_MS = 0;
export const RAPIDFIRE_GLOBAL_DELAY_MAX_MS = 10000;
export const RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MIN_MS = 100;
export const RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MAX_MS = 150;
export const RAPIDFIRE_DEFAULT_MIN_PRESS_SPACING_MS = 80;
export const RAPIDFIRE_TRIGGER_JITTER_MAX_MS = 99999;
export const DEFAULT_RAPIDFIRE_GROUP_ID = "default-rapidfire-group";

// ---- 转换函数 ----

function defaultRapidfireGroup(settings: Pick<RapidfireSettings, "showOverlay" | "overlayPosition" | "overlayWidth">): RapidfireGroup {
  return {
    id: DEFAULT_RAPIDFIRE_GROUP_ID,
    name: "默认分组",
    enabled: true,
    showOverlay: settings.showOverlay,
    overlayPosition: settings.overlayPosition,
    overlayWidth: settings.overlayWidth,
  };
}

function normalizeRapidfireGroups(settings: RapidfireSettings): RapidfireGroup[] {
  const groups = settings.groups && settings.groups.length > 0 ? settings.groups : [defaultRapidfireGroup(settings)];
  const normalized = groups.map((group) => ({
    id: group.id.trim() || DEFAULT_RAPIDFIRE_GROUP_ID,
    name: group.name.trim() || "未命名分组",
    enabled: group.enabled ?? true,
    showOverlay: group.showOverlay ?? settings.showOverlay,
    overlayPosition: group.overlayPosition ?? null,
    overlayWidth: Math.max(RAPIDFIRE_DISPLAY_MIN_WIDTH, Math.min(RAPIDFIRE_DISPLAY_MAX_WIDTH, group.overlayWidth ?? settings.overlayWidth)),
  }));
  if (!normalized.some((group) => group.id === DEFAULT_RAPIDFIRE_GROUP_ID)) {
    normalized.unshift(defaultRapidfireGroup(settings));
  }
  return normalized;
}

function normalizeRapidfireGroupId(groupId: string | undefined, groupIds: Set<string>): string {
  return groupId && groupIds.has(groupId) ? groupId : DEFAULT_RAPIDFIRE_GROUP_ID;
}

export function rapidfireSettingsToForm(settings: RapidfireSettings): RapidfireSettingsForm {
  const groups = normalizeRapidfireGroups(settings);
  const groupIds = new Set(groups.map((group) => group.id));
  const defaultGroup = groups.find((group) => group.id === DEFAULT_RAPIDFIRE_GROUP_ID) ?? groups[0];
  return {
    rapidfireEnabled: settings.rapidfireEnabled,
    showOverlay: defaultGroup.showOverlay,
    overlayWidth: String(defaultGroup.overlayWidth),
    compensationDelayMinMs: String(settings.compensationDelayMinMs ?? RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MIN_MS),
    compensationDelayMaxMs: String(settings.compensationDelayMaxMs ?? RAPIDFIRE_DEFAULT_COMPENSATION_DELAY_MAX_MS),
    overlayPosition: defaultGroup.overlayPosition,
    groups: groups.map((group) => ({
      id: group.id,
      name: group.name,
      enabled: group.enabled,
      showOverlay: group.showOverlay,
      overlayPosition: group.overlayPosition,
      overlayWidth: String(group.overlayWidth),
    })),
    cards: settings.cards.map((card) => ({
      id: card.id,
      groupId: normalizeRapidfireGroupId(card.groupId, groupIds),
      name: card.name,
      triggerKey: card.triggerKey,
      targetKey: card.targetKey,
      intervalMs: String(card.intervalMs),
      pressJitterMinMs: String(card.pressJitterMinMs),
      pressJitterMaxMs: String(card.pressJitterMaxMs),
      minPressSpacingMs: String(card.minPressSpacingMs ?? settings.minPressSpacingMs ?? RAPIDFIRE_DEFAULT_MIN_PRESS_SPACING_MS),
      triggerJitterMaxMs: String(card.triggerJitterMaxMs ?? settings.triggerJitterMaxMs ?? 0),
      cancelJitterOnRelease: card.cancelJitterOnRelease ?? settings.cancelJitterOnRelease ?? true,
      enabled: card.enabled,
      skipCompensation: card.skipCompensation ?? false,
      ignoreTriggerKey: card.ignoreTriggerKey ?? false,
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
  "+",
]);

const SUPPORTED_MODIFIER_KEY_LABELS = ["Ctrl", "Alt", "Shift", "Super"] as const;

type SupportedModifierKeyLabel = (typeof SUPPORTED_MODIFIER_KEY_LABELS)[number];

function normalizePositiveInteger(value: string, fallback: number): number {
  const parsed = Number.parseInt(value, 10);
  return Number.isInteger(parsed) ? parsed : fallback;
}

function normalizeNonNegativeInteger(value: string, fallback: number): number {
  const parsed = Number.parseInt(value, 10);
  return Number.isInteger(parsed) && parsed >= 0 ? parsed : fallback;
}

function clampOverlayWidth(value: string): number {
  const parsed = normalizePositiveInteger(value, RAPIDFIRE_DISPLAY_MIN_WIDTH);
  return Math.max(RAPIDFIRE_DISPLAY_MIN_WIDTH, Math.min(RAPIDFIRE_DISPLAY_MAX_WIDTH, parsed));
}

function normalizeRapidfirePrimary(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "";

  const modifierLabel = normalizeRapidfireModifier(trimmed);
  if (modifierLabel) return SUPPORTED_KEY_LABELS.has(modifierLabel) ? modifierLabel : "";
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
    Plus: "+",
    plus: "+",
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

function normalizeRapidfireKey(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "";

  return trimmed.includes("+") ? normalizeRapidfireHotkey(trimmed) : normalizeRapidfirePrimary(trimmed);
}

function normalizeRapidfireCode(code: string): string {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-2])$/.test(code)) return code;

  const codeMap: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    NumpadEnter: "Enter",
    Tab: "Tab",
    Escape: "Esc",
    Backspace: "Backspace",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    Insert: "Insert",
    Delete: "Delete",
    Semicolon: ";",
    Comma: ",",
    Period: ".",
    Slash: "/",
    Backslash: "\\",
    BracketLeft: "[",
    BracketRight: "]",
    Minus: "-",
    Plus: "+",
    Backquote: "`",
    Quote: "'",
  };

  return codeMap[code] ?? "";
}

function normalizeRapidfireModifier(raw: string): SupportedModifierKeyLabel | null {
  const normalized = raw.trim().toLowerCase();
  if (normalized === "control" || normalized === "ctrl") return "Ctrl";
  if (normalized === "alt") return "Alt";
  if (normalized === "shift") return "Shift";
  if (["meta", "win", "windows", "super", "os"].includes(normalized)) return "Super";
  return null;
}

function normalizeRapidfireHotkey(raw: string): string {
  const trimmed = raw.trim();
  const segments = trimmed.split("+").map((segment) => segment.trim());
  if (segments.length === 0) return "";

  const modifiers = new Set<SupportedModifierKeyLabel>();
  let primary = "";
  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index] === "" && index === segments.length - 1 ? "+" : segments[index];
    if (segment === "") continue;

    const modifier = normalizeRapidfireModifier(segment);
    if (modifier) {
      modifiers.add(modifier);
      continue;
    }
    if (primary) return trimmed;
    primary = normalizeRapidfirePrimary(segment);
  }

  if (!primary) return trimmed;

  return [...SUPPORTED_MODIFIER_KEY_LABELS.filter((modifier) => modifiers.has(modifier)), primary].join("+");
}

function rapidfirePrimaryKeyLabel(key: string): string {
  const segments = key.split("+").map((segment) => segment.trim());
  for (let index = segments.length - 1; index >= 0; index -= 1) {
    if (segments[index] !== "") return segments[index];
    if (index === segments.length - 1) return "+";
  }
  return key;
}

function validateRapidfireHotkeyPrimary(key: string, label: string, allowModifiers: boolean): string {
  if (!allowModifiers || !key.includes("+")) return key;

  const segments = key.split("+").map((segment) => segment.trim());
  let primaryKey = "";

  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index] === "" && index === segments.length - 1 ? "+" : segments[index];
    if (segment === "") continue;
    if (normalizeRapidfireModifier(segment)) continue;
    if (primaryKey) {
      throw new Error(`${label}格式无效，组合键只能包含一个主键。`);
    }
    primaryKey = segment;
  }

  if (!primaryKey) {
    throw new Error(`${label}格式无效，缺少主键。`);
  }

  return primaryKey;
}

function validateRapidfireKey(key: string, label: string, options: { allowModifiers: boolean }): void {
  if (!key) {
    throw new Error(`${label}不能为空。`);
  }

  if (key.includes("+") && !options.allowModifiers) {
    throw new Error(`${label}必须是单键，不能包含组合键。`);
  }

  const primaryKey = validateRapidfireHotkeyPrimary(key, label, options.allowModifiers);
  if (!SUPPORTED_KEY_LABELS.has(primaryKey)) {
    throw new Error(`${label}不支持：${primaryKey}。`);
  }
}

export function parseRapidfireSettingsForm(form: RapidfireSettingsForm): RapidfireSettings {
  if (form.cards.length === 0) {
    throw new Error("至少需要保留一个连发器卡片。");
  }

  const groups = parseRapidfireGroups(mirrorDefaultRapidfireGroup(form.groups, form));
  const groupIds = new Set(groups.map((group) => group.id));

  const cards = form.cards.map((card) => {
    const name = card.name.trim();
    if (!name) {
      throw new Error("连发器卡片名称不能为空。");
    }

    const triggerKey = normalizeRapidfireKey(card.triggerKey);
    const targetKey = normalizeRapidfireKey(card.targetKey);
    validateRapidfireKey(triggerKey, `${name} 的触发键`, { allowModifiers: true });
    validateRapidfireKey(targetKey, `${name} 的目标键`, { allowModifiers: false });

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

    const minPressSpacingMs = normalizeNonNegativeInteger(card.minPressSpacingMs, RAPIDFIRE_DEFAULT_MIN_PRESS_SPACING_MS);
    if (minPressSpacingMs < RAPIDFIRE_GLOBAL_DELAY_MIN_MS || minPressSpacingMs > RAPIDFIRE_GLOBAL_DELAY_MAX_MS) {
      throw new Error(`${name} 的按键最小间距必须在 ${RAPIDFIRE_GLOBAL_DELAY_MIN_MS}-${RAPIDFIRE_GLOBAL_DELAY_MAX_MS}ms 之间。`);
    }
    const triggerJitterMaxMs = normalizeNonNegativeInteger(card.triggerJitterMaxMs, 0);
    if (triggerJitterMaxMs > RAPIDFIRE_TRIGGER_JITTER_MAX_MS) {
      throw new Error(`${name} 的触发抖动延迟上限不能大于 ${RAPIDFIRE_TRIGGER_JITTER_MAX_MS}ms。`);
    }

    return {
      id: card.id,
      groupId: normalizeRapidfireGroupId(card.groupId, groupIds),
      name,
      triggerKey,
      targetKey,
      intervalMs,
      pressJitterMinMs,
      pressJitterMaxMs,
      minPressSpacingMs,
      triggerJitterMaxMs,
      cancelJitterOnRelease: card.cancelJitterOnRelease ?? true,
      enabled: card.enabled,
      skipCompensation: card.skipCompensation,
      ignoreTriggerKey: card.ignoreTriggerKey ?? false,
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
  const legacyMinPressSpacingMs = RAPIDFIRE_DEFAULT_MIN_PRESS_SPACING_MS;
  const defaultGroup = groups.find((group) => group.id === DEFAULT_RAPIDFIRE_GROUP_ID) ?? groups[0];

  if (
    compensationDelayMinMs < RAPIDFIRE_GLOBAL_DELAY_MIN_MS ||
    compensationDelayMaxMs > RAPIDFIRE_GLOBAL_DELAY_MAX_MS
  ) {
    throw new Error(`补齐延迟必须在 ${RAPIDFIRE_GLOBAL_DELAY_MIN_MS}-${RAPIDFIRE_GLOBAL_DELAY_MAX_MS}ms 之间。`);
  }
  if (compensationDelayMinMs > compensationDelayMaxMs) {
    throw new Error("补齐延迟最小值不能大于最大值。");
  }
  

  return {
    version: 1,
    rapidfireEnabled: form.rapidfireEnabled,
    showOverlay: defaultGroup.showOverlay,
    overlayPosition: defaultGroup.overlayPosition,
    overlayWidth: defaultGroup.overlayWidth,
    compensationDelayMinMs,
    compensationDelayMaxMs,
    minPressSpacingMs: legacyMinPressSpacingMs,
    triggerJitterMaxMs: 0,
    cancelJitterOnRelease: true,
    groups,
    cards,
  };
}

function parseRapidfireGroups(groups: RapidfireGroupForm[]): RapidfireGroup[] {
  if (groups.length === 0) {
    throw new Error("至少需要保留一个连发器分组。");
  }

  const seen = new Set<string>();
  return groups.map((group) => {
    const id = group.id.trim() || DEFAULT_RAPIDFIRE_GROUP_ID;
    if (seen.has(id)) {
      throw new Error(`连发器分组 ID 重复：${id}`);
    }
    seen.add(id);

    const name = group.name.trim();
    if (!name) {
      throw new Error("连发器分组名称不能为空。");
    }

    return {
      id,
      name,
      enabled: group.enabled,
      showOverlay: group.showOverlay,
      overlayPosition: group.overlayPosition,
      overlayWidth: clampOverlayWidth(group.overlayWidth),
    };
  });
}

function mirrorDefaultRapidfireGroup(
  groups: RapidfireGroupForm[],
  form: Pick<RapidfireSettingsForm, "showOverlay" | "overlayPosition" | "overlayWidth">,
): RapidfireGroupForm[] {
  return groups.map((group) =>
    group.id === DEFAULT_RAPIDFIRE_GROUP_ID
      ? {
          ...group,
          showOverlay: form.showOverlay,
          overlayPosition: form.overlayPosition,
          overlayWidth: form.overlayWidth,
        }
      : group,
  );
}

export function createRapidfireGroup(existingCount: number): RapidfireGroupForm {
  const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  return {
    id: `rapidfire-group-${suffix}`,
    name: `连发分组 ${existingCount + 1}`,
    enabled: true,
    showOverlay: true,
    overlayPosition: null,
    overlayWidth: String(RAPIDFIRE_DISPLAY_MIN_WIDTH),
  };
}

export function createRapidfireCard(id: string, existingCount = 0, groupId = DEFAULT_RAPIDFIRE_GROUP_ID): RapidfireCardForm {
  const triggerKey = `F${Math.min(12, 6 + existingCount)}`;

  return {
    id,
    groupId,
    name: `连发器 ${existingCount + 1}`,
    triggerKey,
    targetKey: "Space",
    intervalMs: String(RAPIDFIRE_DEFAULT_INTERVAL_MS),
    pressJitterMinMs: String(RAPIDFIRE_DEFAULT_PRESS_JITTER_MIN_MS),
    pressJitterMaxMs: String(RAPIDFIRE_DEFAULT_PRESS_JITTER_MAX_MS),
    minPressSpacingMs: String(RAPIDFIRE_DEFAULT_MIN_PRESS_SPACING_MS),
    triggerJitterMaxMs: "0",
    cancelJitterOnRelease: true,
    enabled: false,
    skipCompensation: false,
    ignoreTriggerKey: false,
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
  const triggerPrimaryKey = rapidfirePrimaryKeyLabel(triggerKey);
  const targetKey = rapidfirePrimaryKeyLabel(normalizeRapidfireKey(card.targetKey));

  if (name && pageError.includes(name)) {
    return pageError;
  }

  if (!name && pageError.includes("名称不能为空")) {
    return pageError;
  }

  const triggerPatterns = [
    `触发键 ${triggerKey}`,
    `触发键${triggerKey}`,
    `触发键 ${triggerPrimaryKey}`,
    `触发键${triggerPrimaryKey}`,
    `触发键不支持：${triggerKey}`,
    `触发键不支持: ${triggerKey}`,
    `触发键不支持：${triggerPrimaryKey}`,
    `触发键不支持: ${triggerPrimaryKey}`,
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
  if (!settings?.rapidfireEnabled) {
    return 0;
  }
  const groups = "groups" in settings && settings.groups ? settings.groups : [];
  if (groups.length === 0) {
    return settings.cards.filter((card) => card.enabled).length;
  }
  const enabledGroupIds = new Set(groups.filter((group) => group.enabled).map((group) => group.id));
  return settings.cards.filter((card) => card.enabled && enabledGroupIds.has(card.groupId ?? DEFAULT_RAPIDFIRE_GROUP_ID)).length;
}

export function rapidfireEffectiveCardsByGroup(
  settings: RapidfireSettingsForm | null,
  groupId: string,
): RapidfireCardForm[] {
  if (!settings?.rapidfireEnabled) {
    return [];
  }
  const group = settings.groups.find((item) => item.id === groupId);
  if (!group?.enabled) {
    return [];
  }
  return settings.cards.filter((card) => card.groupId === groupId && card.enabled);
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

// ---- 热键格式化 ----

export function formatTriggerKey(raw: string): string {
  return normalizeRapidfireKey(raw);
}

export function formatTriggerHotkey(
  event: Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">,
): string {
  if (normalizeRapidfireModifier(event.key)) return "";

  const primary = normalizeRapidfireCode(event.code) || normalizeRapidfireKey(event.key);
  if (!primary || primary.includes("+")) return primary;

  const modifiers: SupportedModifierKeyLabel[] = [];
  if (event.ctrlKey) modifiers.push("Ctrl");
  if (event.altKey) modifiers.push("Alt");
  if (event.shiftKey) modifiers.push("Shift");
  if (event.metaKey) modifiers.push("Super");

  return [...modifiers, primary].join("+");
}
