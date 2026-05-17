import { describe, expect, it } from "vitest";

import type { TimerSettings } from "@/components/app/timer-types";
import { formatTimerRemaining, moveTimerItem, parseTimerSettingsForm, timerSettingsToForm } from "@/components/app/timer-utils";

function sampleSettings(): TimerSettings {
  return {
    enabled: true,
    display: {
      rect: {
        x: 10,
        y: 20,
        width: 320,
        height: 96,
      },
      fontOpacity: 0.75,
    },
    timers: [
      {
        id: "alpha",
        name: "测试计时器",
        durationSeconds: 300,
        hotkey: "Ctrl+F2",
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

    const parsed = parseTimerSettingsForm(form);

    expect(parsed.display.rect.width).toBe(480);
  });

  it("moves timer items by id while preserving all items", () => {
    const settings = sampleSettings();
    settings.timers = [
      { id: "a", name: "A", durationSeconds: 1, hotkey: "F1" },
      { id: "b", name: "B", durationSeconds: 1, hotkey: "F2" },
      { id: "c", name: "C", durationSeconds: 1, hotkey: "F3" },
    ];
    const form = timerSettingsToForm(settings);

    const moved = moveTimerItem(form.timers, "c", "a");

    expect(moved.map((timer) => timer.id)).toEqual(["c", "a", "b"]);
  });

  it("sizes display height for four timers without overflow", () => {
    const settings = sampleSettings();
    settings.timers = [
      { id: "a", name: "A", durationSeconds: 1, hotkey: "F1" },
      { id: "b", name: "B", durationSeconds: 1, hotkey: "F2" },
      { id: "c", name: "C", durationSeconds: 1, hotkey: "F3" },
      { id: "d", name: "D", durationSeconds: 1, hotkey: "F4" },
    ];

    const parsed = parseTimerSettingsForm(timerSettingsToForm(settings));

    expect(parsed.display.rect.height).toBe(168);
  });

  it("formats remaining seconds as seconds only", () => {
    expect(formatTimerRemaining(30)).toBe("30");
    expect(formatTimerRemaining(300)).toBe("300");
    expect(formatTimerRemaining(900)).toBe("900");
  });

  it("rejects zero second timers", () => {
    const form = timerSettingsToForm(sampleSettings());
    form.timers[0].durationSeconds = "0";

    expect(() => parseTimerSettingsForm(form)).toThrow("倒计时秒数");
  });
});
