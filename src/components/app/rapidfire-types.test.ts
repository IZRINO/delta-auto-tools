import { describe, expect, it } from "vitest";

import type { RapidfireSettings } from "@/components/app/rapidfire-types";
import {
  DEFAULT_RAPIDFIRE_GROUP_ID,
  formatTriggerHotkey,
  formatTriggerKey,
  isRapidfireDirty,
  moveRapidfireCard,
  parseRapidfireSettingsForm,
  rapidfireEffectiveCardsByGroup,
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
    groups: [
      {
        id: DEFAULT_RAPIDFIRE_GROUP_ID,
        name: "默认分组",
        enabled: true,
        showOverlay: true,
        overlayPosition: { x: 100, y: 200 },
        overlayWidth: 420,
      },
    ],
    compensationDelayMinMs: 100,
    compensationDelayMaxMs: 150,
    minPressSpacingMs: 80,
    triggerJitterMaxMs: 0,
    cancelJitterOnRelease: true,
    cards: [
      {
        id: "rf-a",
        groupId: DEFAULT_RAPIDFIRE_GROUP_ID,
        name: "测试连发器",
        triggerKey: "F6",
        targetKey: "Space",
        intervalMs: 80,
        pressJitterMinMs: 8,
        pressJitterMaxMs: 12,
        minPressSpacingMs: 80,
        triggerJitterMaxMs: 0,
        cancelJitterOnRelease: true,
        enabled: true,
        skipCompensation: false,
        ignoreTriggerKey: false,
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

  it("migrates legacy settings into the default rapidfire group", () => {
    const legacy = sampleSettings();
    delete legacy.groups;
    legacy.cards = legacy.cards.map(({ groupId: _groupId, ...card }) => card);

    const form = rapidfireSettingsToForm(legacy);
    const parsed = parseRapidfireSettingsForm(form);

    expect(form.groups.map((group) => group.id)).toEqual([DEFAULT_RAPIDFIRE_GROUP_ID]);
    expect(parsed.cards[0].groupId).toBe(DEFAULT_RAPIDFIRE_GROUP_ID);
    expect(parsed.groups?.[0].overlayWidth).toBe(420);
  });

  it("filters effective rapidfire cards by master, group, and card switches", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.groups.push({
      id: "rapidfire-group-b",
      name: "B",
      enabled: false,
      showOverlay: true,
      overlayPosition: null,
      overlayWidth: "420",
    });
    form.cards.push({
      ...form.cards[0],
      id: "rf-b",
      groupId: "rapidfire-group-b",
      triggerKey: "F7",
    });

    expect(rapidfireEffectiveCardsByGroup(form, DEFAULT_RAPIDFIRE_GROUP_ID).map((card) => card.id)).toEqual(["rf-a"]);
    expect(rapidfireEffectiveCardsByGroup(form, "rapidfire-group-b")).toEqual([]);
    form.rapidfireEnabled = false;
    expect(rapidfireEffectiveCardsByGroup(form, DEFAULT_RAPIDFIRE_GROUP_ID)).toEqual([]);
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

  it("round trips per-card ignore trigger key switch", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].ignoreTriggerKey = true;

    const parsed = parseRapidfireSettingsForm(form);

    expect(parsed.cards[0].ignoreTriggerKey).toBe(true);
    expect(rapidfireSettingsToForm(parsed).cards[0].ignoreTriggerKey).toBe(true);
  });

  it("defaults legacy cards to not ignore trigger key", () => {
    const legacy = sampleSettings();
    const legacyCard = legacy.cards[0] as Partial<RapidfireSettings["cards"][number]>;
    delete legacyCard.ignoreTriggerKey;

    expect(rapidfireSettingsToForm(legacy).cards[0].ignoreTriggerKey).toBe(false);
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
    expect(formatTriggerHotkey({ key: "Escape", code: "Escape", ctrlKey: false, altKey: false, shiftKey: false, metaKey: false })).toBe("Esc");
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
      groupId: DEFAULT_RAPIDFIRE_GROUP_ID,
      name: "备用连发器",
      triggerKey: "f6",
      targetKey: "1",
      intervalMs: "100",
      pressJitterMinMs: "10",
      pressJitterMaxMs: "18",
      minPressSpacingMs: "90",
      triggerJitterMaxMs: "20",
      cancelJitterOnRelease: true,
      enabled: true,
      skipCompensation: false,
      ignoreTriggerKey: false,
    });

    const parsed = parseRapidfireSettingsForm(form);
    expect(parsed.cards).toHaveLength(2);
    expect(parsed.cards[0].triggerKey).toBe("F6");
    expect(parsed.cards[1].triggerKey).toBe("F6");
  });

  it("round trips custom press jitter through form state", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].pressJitterMinMs = "1990";
    form.cards[0].pressJitterMaxMs = "2000";

    const parsed = parseRapidfireSettingsForm(form);

    expect(parsed.cards[0].pressJitterMinMs).toBe(1990);
    expect(parsed.cards[0].pressJitterMaxMs).toBe(2000);
  });

  it("rejects press jitter above 2000ms", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.cards[0].pressJitterMaxMs = "2001";

    expect(() => parseRapidfireSettingsForm(form)).toThrow("触发抖动必须在 1-2000ms 之间");
  });

  it("round trips global compensation and per-card rapidfire parameters", () => {
    const form = rapidfireSettingsToForm(sampleSettings());
    form.compensationDelayMinMs = "120";
    form.compensationDelayMaxMs = "180";
    form.cards[0].minPressSpacingMs = "0";
    form.cards[0].triggerJitterMaxMs = "90";
    form.cards[0].cancelJitterOnRelease = false;

    const parsed = parseRapidfireSettingsForm(form);

    expect(parsed.compensationDelayMinMs).toBe(120);
    expect(parsed.compensationDelayMaxMs).toBe(180);
    expect(parsed.minPressSpacingMs).toBe(80);
    expect(parsed.cards[0].minPressSpacingMs).toBe(0);
    expect(parsed.cards[0].triggerJitterMaxMs).toBe(90);
    expect(parsed.cards[0].cancelJitterOnRelease).toBe(false);
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
