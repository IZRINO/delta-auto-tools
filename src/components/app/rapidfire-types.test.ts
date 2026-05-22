import { describe, expect, it } from "vitest";

import type { RapidfireSettings } from "@/components/app/rapidfire-types";
import {
  formatTriggerKey,
  isRapidfireDirty,
  moveRapidfireCard,
  parseRapidfireSettingsForm,
  rapidfireSettingsToForm,
} from "@/components/app/rapidfire-types";

function sampleSettings(): RapidfireSettings {
  return {
    version: 1,
    rapidfireEnabled: true,
    showOverlay: true,
    overlayPosition: { x: 100, y: 200 },
    overlayWidth: 420,
    cards: [
      {
        id: "rf-a",
        name: "测试连发器",
        triggerKey: "F6",
        targetKey: "Space",
        intervalMs: 80,
        enabled: true,
      },
    ],
  };
}

describe("rapidfire-types", () => {
  it("round trips settings through form state", () => {
    const settings = sampleSettings();
    const parsed = parseRapidfireSettingsForm(rapidfireSettingsToForm(settings));

    expect(parsed).toEqual(settings);
  });

  it("does not mark saved settings dirty because of object key order", () => {
    const settings = sampleSettings();
    const form = rapidfireSettingsToForm(settings);

    expect(isRapidfireDirty({ settings, runs: [], hotkeyError: null }, form)).toBe(false);
  });

  it("normalizes browser key labels into supported single keys", () => {
    expect(formatTriggerKey("escape")).toBe("Esc");
    expect(formatTriggerKey("ArrowUp")).toBe("Up");
    expect(formatTriggerKey("f6")).toBe("F6");
  });

  it("rejects modifier combinations for trigger keys", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].triggerKey = "Ctrl+F6";

    expect(() => parseRapidfireSettingsForm(form)).toThrow("单键");
  });

  it("allows duplicate enabled trigger keys across cards", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards.push({
      id: "rf-b",
      name: "备用连发器",
      triggerKey: "f6",
      targetKey: "1",
      intervalMs: "100",
      enabled: true,
    });

    const parsed = parseRapidfireSettingsForm(form);
    expect(parsed.cards).toHaveLength(2);
    expect(parsed.cards[0].triggerKey).toBe("F6");
    expect(parsed.cards[1].triggerKey).toBe("F6");
  });

  it("moves cards by id while preserving all cards", () => {
    const moved = moveRapidfireCard(
      [
        { id: "a", name: "A" },
        { id: "b", name: "B" },
        { id: "c", name: "C" },
      ],
      "c",
      "a",
    );

    expect(moved.map((card) => card.id)).toEqual(["c", "a", "b"]);
  });
});
