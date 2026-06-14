import type React from "react";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listenEvent, TIMER_EVENTS } from "@/lib/tauri-events";

import type { CounterItem, CounterItemForm, CounterRunState, TimerBootstrap, TimerDisplaySettings, TimerGroup, TimerGroupForm, TimerItem, TimerItemForm, TimerRunState, TimerSettings, TimerSettingsForm } from "@/components/app/timer-types";
import { DEFAULT_COUNTER_GROUP_ID, DEFAULT_TIMER_GROUP_ID, TIMER_DISPLAY_MIN_HEIGHT, TIMER_DISPLAY_WIDTH } from "@/components/app/timer-types";
import { formatRecordedHotkey } from "@/components/app/morse-utils";

export function formatTimerHotkey(event: Pick<React.KeyboardEvent<HTMLButtonElement>, "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): string | null {
  return formatRecordedHotkey(event);
}

function displaySettingsToForm(display: TimerDisplaySettings) {
  return {
    rect: display.rect,
    fontOpacity: String(display.fontOpacity),
  };
}

function defaultTimerGroup(display: TimerDisplaySettings): TimerGroup {
  return {
    id: DEFAULT_TIMER_GROUP_ID,
    name: "默认分组",
    enabled: true,
    display,
  };
}

function defaultCounterGroup(display: TimerDisplaySettings): TimerGroup {
  return {
    id: DEFAULT_COUNTER_GROUP_ID,
    name: "默认分组",
    enabled: true,
    display,
  };
}

function groupsToForm(groups: TimerGroup[]): TimerGroupForm[] {
  return groups.map((group) => ({
    id: group.id,
    name: group.name,
    enabled: group.enabled,
    display: displaySettingsToForm(group.display),
  }));
}

function normalizeGroups(
  groups: TimerGroup[] | undefined,
  legacyDisplay: TimerDisplaySettings,
  defaultGroupId: string,
): TimerGroup[] {
  const normalized = (groups && groups.length > 0 ? groups : [defaultGroupId === DEFAULT_TIMER_GROUP_ID ? defaultTimerGroup(legacyDisplay) : defaultCounterGroup(legacyDisplay)])
    .map((group) => ({
      id: group.id.trim() || defaultGroupId,
      name: group.name.trim() || "未命名分组",
      enabled: group.enabled ?? true,
      display: group.display ?? legacyDisplay,
    }));

  if (!normalized.some((group) => group.id === defaultGroupId)) {
    normalized.unshift(defaultGroupId === DEFAULT_TIMER_GROUP_ID ? defaultTimerGroup(legacyDisplay) : defaultCounterGroup(legacyDisplay));
  }

  return normalized;
}

function normalizeGroupId(groupId: string | undefined, groupIds: Set<string>, defaultGroupId: string): string {
  return groupId && groupIds.has(groupId) ? groupId : defaultGroupId;
}

function parseFontOpacity(value: string): number {
  const fontOpacity = Number.parseFloat(value);
  if (!Number.isFinite(fontOpacity) || fontOpacity < 0.1 || fontOpacity > 1) {
    throw new Error("字体透明度必须是 0.1 到 1 之间的数字。");
  }
  return fontOpacity;
}

export function timerSettingsToForm(settings: TimerSettings): TimerSettingsForm {
  const legacyEnabled = Boolean(settings.enabled);
  const timerGroups = normalizeGroups(settings.timerGroups, settings.display, DEFAULT_TIMER_GROUP_ID);
  const counterGroups = normalizeGroups(settings.counterGroups, settings.counterDisplay, DEFAULT_COUNTER_GROUP_ID);
  const timerGroupIds = new Set(timerGroups.map((group) => group.id));
  const counterGroupIds = new Set(counterGroups.map((group) => group.id));

  return {
    timerEnabled: settings.timerEnabled ?? legacyEnabled,
    counterEnabled: settings.counterEnabled ?? legacyEnabled,
    display: displaySettingsToForm(settings.display),
    counterDisplay: displaySettingsToForm(settings.counterDisplay),
    timerGroups: groupsToForm(timerGroups),
    counterGroups: groupsToForm(counterGroups),
    timers: settings.timers.map((timer) => ({
      id: timer.id,
      groupId: normalizeGroupId(timer.groupId, timerGroupIds, DEFAULT_TIMER_GROUP_ID),
      name: timer.name,
      durationSeconds: String(timer.durationSeconds),
      hotkey: timer.hotkey,
      direction: timer.direction,
      triggerMode: timer.triggerMode ?? "press",
      enabled: timer.enabled ?? true,
      ignoreRunning: timer.ignoreRunning ?? true,
      segmentCount: timer.segmentCount != null ? String(timer.segmentCount) : "",
    })),
    counters: settings.counters.map((counter) => ({
      id: counter.id,
      groupId: normalizeGroupId(counter.groupId, counterGroupIds, DEFAULT_COUNTER_GROUP_ID),
      name: counter.name,
      startValue: String(counter.startValue),
      hotkey: counter.hotkey,
      enabled: counter.enabled ?? true,
    })),
  };
}

function parseDisplaySettings(display: TimerSettingsForm["display"], itemCount: number): TimerDisplaySettings {
  const displayWidth = Math.max(TIMER_DISPLAY_WIDTH, Math.round(display.rect.width));

  return {
    rect: {
      ...display.rect,
      width: displayWidth,
      height: displayHeight(itemCount),
    },
    fontOpacity: parseFontOpacity(display.fontOpacity),
  };
}

export function parseTimerSettingsForm(form: TimerSettingsForm): TimerSettings {
  if (form.timers.length === 0) {
    throw new Error("至少需要保留一个计时器。");
  }

  if (form.counters.length === 0) {
    throw new Error("至少需要保留一个计数器。");
  }

  const timerGroups = parseGroups(
    mirrorDefaultGroupDisplay(form.timerGroups, DEFAULT_TIMER_GROUP_ID, form.display),
    DEFAULT_TIMER_GROUP_ID,
    "计时器分组",
  );
  const counterGroups = parseGroups(
    mirrorDefaultGroupDisplay(form.counterGroups, DEFAULT_COUNTER_GROUP_ID, form.counterDisplay),
    DEFAULT_COUNTER_GROUP_ID,
    "计数器分组",
  );
  const timerGroupIds = new Set(timerGroups.map((group) => group.id));
  const counterGroupIds = new Set(counterGroups.map((group) => group.id));

  const timers = form.timers.map((timer): TimerItem => {
    const name = timer.name.trim();
    if (!name) {
      throw new Error("计时器名称不能为空。");
    }

    const hotkey = timer.hotkey.trim();
    if (!hotkey) {
      throw new Error(`${name} 的快捷键不能为空。`);
    }

    const durationSeconds = Number.parseInt(timer.durationSeconds, 10);
    if (!Number.isInteger(durationSeconds) || durationSeconds <= 0) {
      throw new Error(`${name} 的计时秒数必须是大于 0 的整数。`);
    }

    return {
      id: timer.id,
      groupId: normalizeGroupId(timer.groupId, timerGroupIds, DEFAULT_TIMER_GROUP_ID),
      name,
      durationSeconds,
      hotkey,
      direction: timer.direction,
      triggerMode: timer.triggerMode,
      enabled: timer.enabled,
      ignoreRunning: timer.ignoreRunning,
      segmentCount: timer.segmentCount ? Number.parseInt(timer.segmentCount, 10) : null,
    };
  });

  const counters = form.counters.map((counter): CounterItem => {
    const name = counter.name.trim();
    if (!name) {
      throw new Error("计数器名称不能为空。");
    }

    const hotkey = counter.hotkey.trim();
    if (!hotkey) {
      throw new Error(`${name} 的快捷键不能为空。`);
    }

    const startValue = Number.parseInt(counter.startValue, 10);
    if (!Number.isInteger(startValue)) {
      throw new Error(`${name} 的起始数必须是整数。`);
    }

    return {
      id: counter.id,
      groupId: normalizeGroupId(counter.groupId, counterGroupIds, DEFAULT_COUNTER_GROUP_ID),
      name,
      startValue,
      hotkey,
      enabled: counter.enabled,
    };
  });

  const timerCountsByGroup = enabledCountByGroup(timers);
  const counterCountsByGroup = enabledCountByGroup(counters);
  const normalizedTimerGroups = timerGroups.map((group) => ({
    ...group,
    display: parseDisplaySettings(displaySettingsToForm(group.display), timerCountsByGroup.get(group.id) ?? 0),
  }));
  const normalizedCounterGroups = counterGroups.map((group) => ({
    ...group,
    display: parseDisplaySettings(displaySettingsToForm(group.display), counterCountsByGroup.get(group.id) ?? 0),
  }));
  const legacyTimerDisplay = normalizedTimerGroups.find((group) => group.id === DEFAULT_TIMER_GROUP_ID)?.display ?? normalizedTimerGroups[0].display;
  const legacyCounterDisplay = normalizedCounterGroups.find((group) => group.id === DEFAULT_COUNTER_GROUP_ID)?.display ?? normalizedCounterGroups[0].display;

  return {
    enabled: form.timerEnabled || form.counterEnabled,
    timerEnabled: form.timerEnabled,
    counterEnabled: form.counterEnabled,
    display: legacyTimerDisplay,
    counterDisplay: legacyCounterDisplay,
    timerGroups: normalizedTimerGroups,
    counterGroups: normalizedCounterGroups,
    timers,
    counters,
  };
}

function parseGroups(groups: TimerGroupForm[], defaultGroupId: string, label: string): TimerGroup[] {
  if (groups.length === 0) {
    throw new Error(`至少需要保留一个${label}。`);
  }

  const seen = new Set<string>();
  return groups.map((group) => {
    const id = group.id.trim() || defaultGroupId;
    if (seen.has(id)) {
      throw new Error(`${label} ID 重复：${id}`);
    }
    seen.add(id);

    const name = group.name.trim();
    if (!name) {
      throw new Error(`${label}名称不能为空。`);
    }

    return {
      id,
      name,
      enabled: group.enabled,
      display: parseDisplaySettings(group.display, 0),
    };
  });
}

function mirrorDefaultGroupDisplay(
  groups: TimerGroupForm[],
  defaultGroupId: string,
  display: TimerGroupForm["display"],
): TimerGroupForm[] {
  return groups.map((group) => (group.id === defaultGroupId ? { ...group, display } : group));
}

function enabledCountByGroup(items: Array<{ groupId?: string; enabled: boolean }>): Map<string, number> {
  const map = new Map<string, number>();
  for (const item of items) {
    if (!item.enabled || !item.groupId) {
      continue;
    }
    map.set(item.groupId, (map.get(item.groupId) ?? 0) + 1);
  }
  return map;
}

export function createTimerGroup(existingCount: number): TimerGroupForm {
  const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  return {
    id: `timer-group-${suffix}`,
    name: `计时分组 ${existingCount + 1}`,
    enabled: true,
    display: displaySettingsToForm(timerDefaultDisplay()),
  };
}

export function createCounterGroup(existingCount: number): TimerGroupForm {
  const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  return {
    id: `counter-group-${suffix}`,
    name: `计数分组 ${existingCount + 1}`,
    enabled: true,
    display: displaySettingsToForm(counterDefaultDisplay()),
  };
}

function timerDefaultDisplay(): TimerDisplaySettings {
  return {
    rect: { x: 80, y: 80, width: TIMER_DISPLAY_WIDTH, height: TIMER_DISPLAY_MIN_HEIGHT },
    fontOpacity: 0.92,
  };
}

function counterDefaultDisplay(): TimerDisplaySettings {
  return {
    rect: { x: 420, y: 80, width: TIMER_DISPLAY_WIDTH, height: TIMER_DISPLAY_MIN_HEIGHT },
    fontOpacity: 0.92,
  };
}

export function createTimerItem(existingCount: number, groupId = DEFAULT_TIMER_GROUP_ID): TimerItem {
  const nextIndex = existingCount + 1;
  const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

  return {
    id: `timer-${suffix}`,
    groupId,
    name: `计时器 ${nextIndex}`,
    durationSeconds: 30,
    hotkey: "F2",
    direction: "countdown",
    enabled: true,
    ignoreRunning: true,
    segmentCount: null,
    triggerMode: "press",
  };
}

export function createCounterItem(existingCount: number, groupId = DEFAULT_COUNTER_GROUP_ID): CounterItem {
  const nextIndex = existingCount + 1;
  const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

  return {
    id: `counter-${suffix}`,
    groupId,
    name: `计数器 ${nextIndex}`,
    startValue: 0,
    hotkey: "F3",
    enabled: true,
  };
}

export function timerEffectiveTimersByGroup(form: TimerSettingsForm | null, groupId: string): TimerItemForm[] {
  if (!form?.timerEnabled) {
    return [];
  }
  const group = form.timerGroups.find((item) => item.id === groupId);
  if (!group?.enabled) {
    return [];
  }
  return form.timers.filter((timer) => timer.groupId === groupId && timer.enabled);
}

export function timerEffectiveCountersByGroup(form: TimerSettingsForm | null, groupId: string): CounterItemForm[] {
  if (!form?.counterEnabled) {
    return [];
  }
  const group = form.counterGroups.find((item) => item.id === groupId);
  if (!group?.enabled) {
    return [];
  }
  return form.counters.filter((counter) => counter.groupId === groupId && counter.enabled);
}

export function moveTimerItem<T extends { id: string }>(items: T[], activeId: string, overId: string): T[] {
  if (activeId === overId) {
    return items;
  }

  const activeIndex = items.findIndex((item) => item.id === activeId);
  const overIndex = items.findIndex((item) => item.id === overId);
  if (activeIndex === -1 || overIndex === -1) {
    return items;
  }

  const next = [...items];
  const [moved] = next.splice(activeIndex, 1);
  next.splice(overIndex, 0, moved);
  return next;
}

export function displayHeight(itemCount: number): number {
  return Math.max(TIMER_DISPLAY_MIN_HEIGHT, 48 + Math.max(1, itemCount) * 30);
}

export function formatTimerRemaining(seconds: number): string {
  return String(Math.max(0, Math.floor(seconds)));
}

export function timerProgressPercent(run: TimerRunState | undefined, durationSeconds: number): number {
  if (!run) {
    return 100;
  }

  if (run.durationSeconds <= 0 && durationSeconds <= 0) {
    return 0;
  }

  const total = run.durationSeconds || durationSeconds;
  // 多段倒计时：进度 = 已消耗 / 总时长
  if (run.segmentCount != null && run.segmentCount >= 2 && run.direction === "countdown") {
    return Math.max(0, Math.min(100, ((total - run.remainingSeconds) / total) * 100));
  }
  return Math.max(0, Math.min(100, (run.remainingSeconds / total) * 100));
}

export function isTimerRunActive(run: TimerRunState | undefined): boolean {
  return run?.status === "running";
}

export function timerRunsById(runs: TimerRunState[]): Map<string, TimerRunState> {
  return new Map(runs.map((run) => [run.id, run]));
}

export function counterRunsById(runs: CounterRunState[]): Map<string, CounterRunState> {
  return new Map(runs.map((run) => [run.id, run]));
}

export function isTimerDirty(bootstrap: TimerBootstrap | null, form: TimerSettingsForm | null): boolean {
  if (!bootstrap || !form) {
    return false;
  }

  try {
    return JSON.stringify(timerSettingsToForm(bootstrap.settings)) !== JSON.stringify(timerSettingsToForm(parseTimerSettingsForm(form)));
  } catch {
    return true;
  }
}

export function useTimerOverlayBootstrap(isNativeShell: boolean, setBootstrap: (value: TimerBootstrap) => void) {
  useEffect(() => {
    document.body.dataset.overlayMode = "true";
    return () => {
      delete document.body.dataset.overlayMode;
    };
  }, []);

  useEffect(() => {
    if (!isNativeShell) {
      return;
    }

    let disposed = false;
    let unlistenStateChanged: (() => void) | undefined;

    void invoke<TimerBootstrap>("timer_get_bootstrap").then((next) => {
      if (!disposed) {
        setBootstrap(next);
      }
    });

    void listenEvent(TIMER_EVENTS.stateChanged, (event) => {
      if (!disposed) {
        setBootstrap(event.payload);
      }
    }).then((dispose) => {
      unlistenStateChanged = dispose;
    });

    return () => {
      disposed = true;
      unlistenStateChanged?.();
    };
  }, [isNativeShell, setBootstrap]);
}
