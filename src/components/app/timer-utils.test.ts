import { describe, expect, it } from "vitest";

import type { TimerSettings } from "@/components/app/timer-types";
import { formatTimerRemaining, parseTimerSettingsForm, timerSettingsToForm } from "@/components/app/timer-utils";

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
