import { describe, expect, it } from "vitest";

import type { TimerSettings } from "@/components/app/timer-types";
import { formatTimerRemaining, isTimerRunActive, moveTimerItem, parseTimerSettingsForm, timerProgressPercent, timerSettingsToForm } from "@/components/app/timer-utils";

function sampleSettings(): TimerSettings {
  return {
    enabled: true,
    timerEnabled: true,
    counterEnabled: true,
    display: {
      rect: {
        x: 10,
        y: 20,
        width: 320,
        height: 96,
      },
      fontOpacity: 0.75,
    },
    counterDisplay: {
      rect: {
        x: 340,
        y: 20,
        width: 320,
        height: 96,
      },
      fontOpacity: 0.8,
    },
    timers: [
      {
        id: "alpha",
        name: "测试计时器",
        durationSeconds: 300,
        hotkey: "Ctrl+F2",
        direction: "countdown",
        triggerMode: "press" as const,
        enabled: true,
        ignoreRunning: true,
        segmentCount: null,
      },
    ],
    counters: [
      {
        id: "counter-alpha",
        name: "测试计数器",
        startValue: 5,
        hotkey: "Ctrl+F3",
        enabled: true,
      },
    ],
  };
}

describe("timer-utils", () => {
  it("round trips timer settings through form state", () => {
    const settings = sampleSettings();
    const parsed = parseTimerSettingsForm(timerSettingsToForm(settings));

    expect(parsed).toEqual(settings);
  });

  it("preserves custom display width through form parsing", () => {
    const form = timerSettingsToForm(sampleSettings());
    form.display.rect.width = 480;
    form.counterDisplay.rect.width = 520;

    const parsed = parseTimerSettingsForm(form);

    expect(parsed.display.rect.width).toBe(480);
    expect(parsed.counterDisplay.rect.width).toBe(520);
  });

  it("moves timer items by id while preserving all items", () => {
    const settings = sampleSettings();
    settings.timers = [
      { id: "a", name: "A", durationSeconds: 1, hotkey: "F1", direction: "countdown", triggerMode: "press" as const, enabled: true, ignoreRunning: true, segmentCount: null },
      { id: "b", name: "B", durationSeconds: 1, hotkey: "F2", direction: "countup", triggerMode: "press" as const, enabled: true, ignoreRunning: true, segmentCount: null },
      { id: "c", name: "C", durationSeconds: 1, hotkey: "F3", direction: "countdown", triggerMode: "press" as const, enabled: true, ignoreRunning: true, segmentCount: null },
    ];
    const form = timerSettingsToForm(settings);

    const moved = moveTimerItem(form.timers, "c", "a");

    expect(moved.map((timer) => timer.id)).toEqual(["c", "a", "b"]);
  });

  it("sizes display height for four timers without overflow", () => {
    const settings = sampleSettings();
    settings.timers = [
      { id: "a", name: "A", durationSeconds: 1, hotkey: "F1", direction: "countdown", triggerMode: "press" as const, enabled: true, ignoreRunning: true, segmentCount: null },
      { id: "b", name: "B", durationSeconds: 1, hotkey: "F2", direction: "countdown", triggerMode: "press" as const, enabled: true, ignoreRunning: true, segmentCount: null },
      { id: "c", name: "C", durationSeconds: 1, hotkey: "F3", direction: "countdown", triggerMode: "press" as const, enabled: true, ignoreRunning: true, segmentCount: null },
      { id: "d", name: "D", durationSeconds: 1, hotkey: "F4", direction: "countdown", triggerMode: "press" as const, enabled: true, ignoreRunning: true, segmentCount: null },
    ];

    const parsed = parseTimerSettingsForm(timerSettingsToForm(settings));

    expect(parsed.display.rect.height).toBe(168);
  });

  it("formats remaining seconds as seconds only", () => {
    expect(formatTimerRemaining(30)).toBe("30");
    expect(formatTimerRemaining(300)).toBe("300");
    expect(formatTimerRemaining(900)).toBe("900");
  });

  it("computes progress from remaining seconds", () => {
    expect(timerProgressPercent({ id: "a", currentSeconds: 5, remainingSeconds: 5, durationSeconds: 10, direction: "countdown", status: "running", segmentCount: null, segmentDuration: 10, recovering: false, recoveringCount: 0, activeSegmentIndex: 0, startedAtMs: 0, recoveryStartPool: 0 }, 10)).toBe(50);
    expect(timerProgressPercent({ id: "a", currentSeconds: 10, remainingSeconds: 0, durationSeconds: 10, direction: "countup", status: "finished", segmentCount: null, segmentDuration: 10, recovering: false, recoveringCount: 0, activeSegmentIndex: 0, startedAtMs: 0, recoveryStartPool: 0 }, 10)).toBe(0);
  });

  it("刚触发且秒数未变化时仍识别为运行中", () => {
    const runningRun = {
      id: "a",
      currentSeconds: 30,
      remainingSeconds: 30,
      durationSeconds: 30,
      direction: "countdown" as const,
      status: "running" as const,
      segmentCount: null,
      segmentDuration: 30,
      recovering: false,
      recoveringCount: 0,
      activeSegmentIndex: 0,
      startedAtMs: 1000,
      recoveryStartPool: 0,
    };
    const finishedRun = { ...runningRun, currentSeconds: 0, remainingSeconds: 0, status: "finished" as const };

    expect(isTimerRunActive(runningRun)).toBe(true);
    expect(isTimerRunActive(finishedRun)).toBe(false);
    expect(isTimerRunActive(undefined)).toBe(false);
  });

  it("rejects zero second timers", () => {
    const form = timerSettingsToForm(sampleSettings());
    form.timers[0].durationSeconds = "0";

    expect(() => parseTimerSettingsForm(form)).toThrow("计时秒数");
  });
});
