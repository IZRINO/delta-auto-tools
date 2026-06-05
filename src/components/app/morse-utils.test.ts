import { describe, expect, it } from "vitest";

import type { MorseRunResult, MorseSettingsForm } from "@/components/app/morse-types";
import {
  clickRegionRows,
  formatRecordedHotkey,
  formatRegion,
  formatTimestamp,
  getSelectionRect,
  normalizeHotkeyPrimaryKey,
  normalizeRunDetails,
  parseOverlaySlots,
  parseOverlayTarget,
  parseSettingsForm,
  settingsToForm,
} from "@/components/app/morse-utils";

describe("morse-utils", () => {
  it("converts settings to form strings", () => {
    const form = settingsToForm({
      hotkey: "Ctrl+F1",
      regions: [null, null, null],
      binaryThreshold: 120,
      autoInputDelay: 80,
      afterClickHotkey: "Ctrl+F4",
      autoClickEnabled: false,
      clickRegions: [],
    });

    expect(form).toEqual({
      hotkey: "Ctrl+F1",
      regions: [null, null, null],
      binaryThreshold: "120",
      autoInputDelay: "80",
      afterClickHotkey: "Ctrl+F4",
      autoClickEnabled: false,
      clickRegions: new Array(7).fill(null).map(() => ({ rect: null, delayMs: "500" })),
    });
  });

  it("parses a valid settings form", () => {
    const form: MorseSettingsForm = {
      hotkey: " Ctrl+F2 ",
      regions: [null, null, null],
      binaryThreshold: "127",
      autoInputDelay: "50",
      afterClickHotkey: "  ",
      autoClickEnabled: false,
      clickRegions: [],
    };

    expect(parseSettingsForm(form)).toEqual({
      hotkey: "Ctrl+F2",
      regions: [null, null, null],
      binaryThreshold: 127,
      autoInputDelay: 50,
      afterClickHotkey: null,
      autoClickEnabled: false,
      clickRegions: [],
    });
  });

  it("rejects an empty hotkey", () => {
    expect(() =>
      parseSettingsForm({
        hotkey: "   ",
        regions: [null, null, null],
        binaryThreshold: "127",
        autoInputDelay: "50",
        afterClickHotkey: "",
        autoClickEnabled: false,
        clickRegions: [],
      } as MorseSettingsForm),
    ).toThrow("热键不能为空");
  });

  it("rejects an invalid binary threshold", () => {
    expect(() =>
      parseSettingsForm({
        hotkey: "F1",
        regions: [null, null, null],
        binaryThreshold: "300",
        autoInputDelay: "50",
        afterClickHotkey: "",
        autoClickEnabled: false,
        clickRegions: [],
      } as MorseSettingsForm),
    ).toThrow("二值化阈值必须是 0 到 255 之间的整数");
  });

  it("rejects an invalid auto input delay", () => {
    expect(() =>
      parseSettingsForm({
        hotkey: "F1",
        regions: [null, null, null],
        binaryThreshold: "127",
        autoInputDelay: "-1",
        afterClickHotkey: "",
        autoClickEnabled: false,
        clickRegions: [],
      } as MorseSettingsForm),
    ).toThrow("输入延迟必须是大于等于 0 的整数毫秒值");
  });

  it("normalizes hotkey primary keys", () => {
    expect(normalizeHotkeyPrimaryKey("a")).toBe("A");
    expect(normalizeHotkeyPrimaryKey("5")).toBe("5");
    expect(normalizeHotkeyPrimaryKey("F12")).toBe("F12");
    expect(normalizeHotkeyPrimaryKey("ArrowLeft")).toBe("Left");
    expect(normalizeHotkeyPrimaryKey("Control")).toBeNull();
  });

  it("formats recorded hotkeys", () => {
    expect(
      formatRecordedHotkey({
        key: "a",
        ctrlKey: true,
        altKey: false,
        shiftKey: true,
        metaKey: false,
      } as React.KeyboardEvent<HTMLButtonElement>),
    ).toBe("Ctrl+Shift+A");

    expect(
      formatRecordedHotkey({
        key: "Meta",
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        metaKey: true,
      } as React.KeyboardEvent<HTMLButtonElement>),
    ).toBeNull();
  });

  it("parses overlay slots from slots query", () => {
    expect(parseOverlaySlots("?mode=overlay&slots=0,2,2,5")).toEqual([0, 2]);
  });

  it("parses click overlay slots up to seven regions", () => {
    expect(parseOverlayTarget("?mode=overlay&target=click&slots=3")).toBe("click");
    expect(parseOverlaySlots("?mode=overlay&target=click&slots=3")).toEqual([3]);
    expect(parseOverlaySlots("?mode=overlay&target=click&slots=6,7")).toEqual([6]);
  });

  it("parses overlay slots from slot query and falls back to defaults", () => {
    expect(parseOverlaySlots("?mode=overlay&slot=1")).toEqual([1]);
    expect(parseOverlaySlots("?mode=overlay&slot=9")).toEqual([0, 1, 2]);
    expect(parseOverlaySlots("?mode=overlay&target=click&slot=9")).toEqual([0]);
  });

  it("keeps real click region slot indexes after filtering empty slots", () => {
    const rows = clickRegionRows([
      { rect: null, delayMs: "500" },
      { rect: { x: 1, y: 2, width: 3, height: 4 }, delayMs: "600" },
      { rect: null, delayMs: "500" },
      { rect: { x: 5, y: 6, width: 7, height: 8 }, delayMs: "700" },
    ]);

    expect(rows.map((row) => row.slotIndex)).toEqual([1, 3]);
    expect(rows.map((row) => row.delayMs)).toEqual(["600", "700"]);
  });

  it("normalizes dragged rectangles", () => {
    expect(getSelectionRect({ x: 50, y: 80 }, { x: 10, y: 20 })).toEqual({
      x: 10,
      y: 20,
      width: 40,
      height: 60,
    });
  });

  it("normalizes run details with defaults", () => {
    const run: MorseRunResult = {
      value: "123",
      triggeredBy: "manual",
      autoTyped: false,
      occurredAtMs: 1,
      error: null,
      details: [
        {
          slot: 1,
          thresholdMode: "manual",
          contourCount: 3,
          morse: ".-",
          digit: "2",
          error: null,
        },
      ],
    };

    const details = normalizeRunDetails(run);
    expect(details).toHaveLength(3);
    expect(details[0].digit).toBeNull();
    expect(details[1].digit).toBe("2");
    expect(details[2].thresholdMode).toBe("--");
  });

  it("formats timestamps and regions", () => {
    expect(formatTimestamp(null)).toBe("--:--:--");
    expect(formatTimestamp(0)).toBe("--:--:--");
    expect(formatRegion(null)).toBe("未设置");
    expect(formatRegion({ x: 1, y: 2, width: 3, height: 4 })).toBe("X 1 · Y 2 · W 3 · H 4");
  });
});
