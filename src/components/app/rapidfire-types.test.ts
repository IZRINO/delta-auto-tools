import { describe, expect, it } from "vitest";

import type { RapidfireSettings } from "@/components/app/rapidfire-types";
import {
  formatTriggerHotkey,
  formatTriggerKey,
  isRapidfireDirty,
  moveRapidfireCard,
  parseRapidfireSettingsForm,
  rapidfireCardError,
  rapidfireCardStatus,
  rapidfireSettingsToForm,
} from "@/components/app/rapidfire-types";

function sampleSettings(): RapidfireSettings {
  return {
    version: 1,
    rapidfireEnabled: true,
    showOverlay: true,
    overlayPosition: { x: 100, y: 200 },
    overlayWidth: 420,
    compensationDelayMinMs: 100,
    compensationDelayMaxMs: 150,
    minPressSpacingMs: 80,
    triggerJitterMaxMs: 0,
    cancelJitterOnRelease: true,
    cards: [
      {
        id: "rf-a",
        name: "测试连发器",
        triggerKey: "F6",
        targetKey: "Space",
        intervalMs: 80,
        pressJitterMinMs: 8,
        pressJitterMaxMs: 12,
        enabled: true,
        skipCompensation: false,
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

  it("round trips per-card no-append compensation switch", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].skipCompensation = true;

    const parsed = parseRapidfireSettingsForm(form);

    expect(parsed.cards[0].skipCompensation).toBe(true);
    expect(rapidfireSettingsToForm(parsed).cards[0].skipCompensation).toBe(true);
  });

  it("defaults legacy cards to automatic compensation", () => {
    const legacy = sampleSettings();
    const legacyCard = legacy.cards[0] as Partial<RapidfireSettings["cards"][number]>;
    delete legacyCard.skipCompensation;

    expect(rapidfireSettingsToForm(legacy).cards[0].skipCompensation).toBe(false);
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

  it("allows modifier combinations for trigger keys", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].triggerKey = "shift+ctrl+-";

    const parsed = parseRapidfireSettingsForm(form);
    expect(parsed.cards[0].triggerKey).toBe("Ctrl+Shift+-");
  });

  it("keeps standalone Alt as a trigger primary key", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].triggerKey = "alt";

    const parsed = parseRapidfireSettingsForm(form);
    expect(parsed.cards[0].triggerKey).toBe("Alt");
  });

  it("allows plus as a trigger hotkey primary key", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].triggerKey = "shift++";

    const parsed = parseRapidfireSettingsForm(form);
    expect(parsed.cards[0].triggerKey).toBe("Shift++");
  });

  it("rejects modifier combinations for target keys", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].targetKey = "Ctrl+F6";

    expect(() => parseRapidfireSettingsForm(form)).toThrow("单键");
  });
  it("formats trigger hotkeys from physical keyboard codes", () => {
    expect(
      formatTriggerHotkey({ key: "_", code: "Minus", ctrlKey: false, altKey: false, shiftKey: true, metaKey: false }),
    ).toBe("Shift+-");
    expect(
      formatTriggerHotkey({ key: "A", code: "KeyA", ctrlKey: true, altKey: true, shiftKey: false, metaKey: false }),
    ).toBe("Ctrl+Alt+A");
    expect(
      formatTriggerHotkey({ key: "Shift", code: "ShiftLeft", ctrlKey: false, altKey: false, shiftKey: true, metaKey: false }),
    ).toBe("");
  });

  it("allows duplicate enabled trigger keys across cards", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards.push({
      id: "rf-b",
      name: "备用连发器",
      triggerKey: "f6",
      targetKey: "1",
      intervalMs: "100",
      pressJitterMinMs: "10",
      pressJitterMaxMs: "18",
      enabled: true,
      skipCompensation: false,
    });

    const parsed = parseRapidfireSettingsForm(form);
    expect(parsed.cards).toHaveLength(2);
    expect(parsed.cards[0].triggerKey).toBe("F6");
    expect(parsed.cards[1].triggerKey).toBe("F6");
  });

  it("round trips custom press jitter through form state", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].pressJitterMinMs = "15";
    form.cards[0].pressJitterMaxMs = "25";

    const parsed = parseRapidfireSettingsForm(form);

    expect(parsed.cards[0].pressJitterMinMs).toBe(15);
    expect(parsed.cards[0].pressJitterMaxMs).toBe(25);
  });

  it("round trips global rapidfire delay parameters through form state", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.compensationDelayMinMs = "120";
    form.compensationDelayMaxMs = "180";
    form.minPressSpacingMs = "90";

    const parsed = parseRapidfireSettingsForm(form);

    expect(parsed.compensationDelayMinMs).toBe(120);
    expect(parsed.compensationDelayMaxMs).toBe(180);
    expect(parsed.minPressSpacingMs).toBe(90);
  });

  it("rejects an inverted global compensation delay range", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.compensationDelayMinMs = "180";
    form.compensationDelayMaxMs = "120";

    expect(() => parseRapidfireSettingsForm(form)).toThrow("补齐延迟");
  });

  it("rejects an inverted press jitter range", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].pressJitterMinMs = "30";
    form.cards[0].pressJitterMaxMs = "20";

    expect(() => parseRapidfireSettingsForm(form)).toThrow("触发抖动");
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

  it("shows disabled cards as not enabled instead of idle", () => {
    const card = rapidfireSettingsToForm(sampleSettings()).cards[0];
    card.enabled = false;

    expect(rapidfireCardStatus(card, undefined, null)).toMatchObject({
      label: "未启用",
      variant: "outline",
    });
  });

  it("shows enabled cards without runs as idle", () => {
    const card = rapidfireSettingsToForm(sampleSettings()).cards[0];

    expect(rapidfireCardStatus(card, undefined, null)).toMatchObject({
      label: "空闲",
      variant: "outline",
    });
  });

  it("keeps running count in the card status badge", () => {
    const card = rapidfireSettingsToForm(sampleSettings()).cards[0];

    expect(rapidfireCardStatus(card, { cardId: card.id, status: "firing", count: 7 }, null)).toMatchObject({
      label: "连发中 · 7",
      active: true,
    });
  });

  it("prioritizes config errors over running state", () => {
    const card = rapidfireSettingsToForm(sampleSettings()).cards[0];

    expect(
      rapidfireCardStatus(card, { cardId: card.id, status: "firing", count: 7 }, "测试连发器 的触发键 F6 与计时器的快捷键 F6 冲突"),
    ).toMatchObject({
      label: "配置未生效",
      variant: "destructive",
      error: true,
    });
  });

  it("matches card errors by card name or key text", () => {
    const card = rapidfireSettingsToForm(sampleSettings()).cards[0];

    expect(rapidfireCardError(card, "测试连发器 的触发键 F6 与计时器的快捷键 F6 冲突")).toBeTruthy();
    expect(rapidfireCardError(card, "连发器2 的触发键 F6 与计时器的快捷键 F6 冲突")).toBeTruthy();
    expect(rapidfireCardError(card, "其他模块保存失败")).toBeNull();
  });
});
