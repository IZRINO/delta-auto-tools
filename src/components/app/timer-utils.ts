import type React from "react";

import type { TimerBootstrap, TimerItem, TimerRunState, TimerSettings, TimerSettingsForm } from "@/components/app/timer-types";
import { TIMER_DISPLAY_MIN_HEIGHT } from "@/components/app/timer-types";
import { formatRecordedHotkey } from "@/components/app/morse-utils";

export function timerSettingsToForm(settings: TimerSettings): TimerSettingsForm {
  return {
    enabled: settings.enabled,
    display: {
      rect: settings.display.rect,
      fontOpacity: String(settings.display.fontOpacity),
    },
    timers: settings.timers.map((timer) => ({
      id: timer.id,
      name: timer.name,
      durationSeconds: String(timer.durationSeconds),
      hotkey: timer.hotkey,
    })),
  };
}

export function parseTimerSettingsForm(form: TimerSettingsForm): TimerSettings {
  const fontOpacity = Number.parseFloat(form.display.fontOpacity);
  if (!Number.isFinite(fontOpacity) || fontOpacity < 0.1 || fontOpacity > 1) {
    throw new Error("字体透明度必须是 0.1 到 1 之间的数字。");
  }

  if (form.timers.length === 0) {
    throw new Error("至少需要保留一个计时器。");
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
      throw new Error(`${name} 的倒计时秒数必须是大于 0 的整数。`);
    }

    return {
      id: timer.id,
      name,
      durationSeconds,
      hotkey,
    };
  });

  return {
    enabled: form.enabled,
    display: {
      rect: {
        ...form.display.rect,
        width: 320,
        height: Math.max(TIMER_DISPLAY_MIN_HEIGHT, 56 + Math.max(1, timers.length) * 34),
      },
      fontOpacity,
    },
    timers,
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
  };
}

export function formatTimerRemaining(seconds: number): string {
  return String(Math.max(0, Math.floor(seconds)));
}

export function timerRunsById(runs: TimerRunState[]): Map<string, TimerRunState> {
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
