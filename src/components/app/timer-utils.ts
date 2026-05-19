import type React from "react";

import type { CounterItem, CounterRunState, TimerBootstrap, TimerDisplaySettings, TimerItem, TimerRunState, TimerSettings, TimerSettingsForm } from "@/components/app/timer-types";
import { TIMER_DISPLAY_MIN_HEIGHT, TIMER_DISPLAY_WIDTH } from "@/components/app/timer-types";
import { formatRecordedHotkey } from "@/components/app/morse-utils";

function displaySettingsToForm(display: TimerDisplaySettings) {
  return {
    rect: display.rect,
    fontOpacity: String(display.fontOpacity),
  };
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

  return {
    timerEnabled: settings.timerEnabled ?? legacyEnabled,
    counterEnabled: settings.counterEnabled ?? legacyEnabled,
    display: displaySettingsToForm(settings.display),
    counterDisplay: displaySettingsToForm(settings.counterDisplay),
    timers: settings.timers.map((timer) => ({
      id: timer.id,
      name: timer.name,
      durationSeconds: String(timer.durationSeconds),
      hotkey: timer.hotkey,
      direction: timer.direction,
    })),
    counters: settings.counters.map((counter) => ({
      id: counter.id,
      name: counter.name,
      startValue: String(counter.startValue),
      hotkey: counter.hotkey,
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
      name,
      durationSeconds,
      hotkey,
      direction: timer.direction,
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
      name,
      startValue,
      hotkey,
    };
  });

  return {
    enabled: form.timerEnabled || form.counterEnabled,
    timerEnabled: form.timerEnabled,
    counterEnabled: form.counterEnabled,
    display: parseDisplaySettings(form.display, timers.length),
    counterDisplay: parseDisplaySettings(form.counterDisplay, counters.length),
    timers,
    counters,
  };
}

export function createTimerItem(existingCount: number): TimerItem {
  const nextIndex = existingCount + 1;
  const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

  return {
    id: `timer-${suffix}`,
    name: `计时器 ${nextIndex}`,
    durationSeconds: 30,
    hotkey: "F2",
    direction: "countdown",
  };
}

export function createCounterItem(existingCount: number): CounterItem {
  const nextIndex = existingCount + 1;
  const suffix = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

  return {
    id: `counter-${suffix}`,
    name: `计数器 ${nextIndex}`,
    startValue: 0,
    hotkey: "F3",
  };
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
  return Math.max(0, Math.min(100, (run.remainingSeconds / total) * 100));
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

export function formatTimerHotkey(event: Pick<React.KeyboardEvent<HTMLButtonElement>, "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): string | null {
  return formatRecordedHotkey(event);
}
