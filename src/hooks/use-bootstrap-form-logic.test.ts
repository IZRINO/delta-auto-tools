import { describe, expect, it } from "vitest";
import {
  computeIsDirty,
  isStaleSave,
  shouldSyncFormFully,
} from "@/hooks/use-bootstrap-form-logic";

describe("useBootstrapForm core logic", () => {
  describe("computeIsDirty", () => {
    // 模拟真实的 settingsToForm/parseSettingsForm 往返转换：
    // settingsToForm 将 number 字段转为 string，parseSettingsForm 将 string 解析回 number
    const settingsToForm = (s: Record<string, unknown>) => ({
      count: String(s.count),
      name: s.name,
    });
    const parseSettingsForm = (f: Record<string, unknown>) => ({
      count: Number(f.count),
      name: f.name,
    });

    it("returns false when bootstrap is null", () => {
      expect(computeIsDirty(null, null, settingsToForm, parseSettingsForm)).toBe(false);
    });

    it("returns false when form is null", () => {
      expect(computeIsDirty(null, { count: 1, name: "a" }, settingsToForm, parseSettingsForm)).toBe(false);
    });

    it("returns false when form matches bootstrap (round-trip)", () => {
      const form = settingsToForm({ count: 5, name: "test" });
      expect(computeIsDirty(form, { count: 5, name: "test" }, settingsToForm, parseSettingsForm)).toBe(false);
    });

    it("returns true when form differs from bootstrap", () => {
      const form = settingsToForm({ count: 10, name: "test" });
      expect(computeIsDirty(form, { count: 5, name: "test" }, settingsToForm, parseSettingsForm)).toBe(true);
    });

    it("returns true on parse error", () => {
      const badParse = () => { throw new Error("bad"); };
      const form = settingsToForm({ count: 5, name: "test" });
      expect(computeIsDirty(form, { count: 5, name: "test" }, settingsToForm, badParse as unknown as typeof parseSettingsForm)).toBe(true);
    });
  });

  describe("isStaleSave", () => {
    it("returns false when no pending version", () => {
      expect(isStaleSave(undefined, { current: 5 })).toBe(false);
    });

    it("returns false when version matches", () => {
      expect(isStaleSave(5, { current: 5 })).toBe(false);
    });

    it("returns true when version mismatch", () => {
      expect(isStaleSave(3, { current: 5 })).toBe(true);
    });
  });

  describe("shouldSyncFormFully", () => {
    it("returns true for syncMode full", () => {
      expect(shouldSyncFormFully("full", undefined, false)).toBe(true);
    });

    it("returns true for syncForm true", () => {
      expect(shouldSyncFormFully(undefined, true, false)).toBe(true);
    });

    it("returns true when form is null", () => {
      expect(shouldSyncFormFully(undefined, false, true)).toBe(true);
    });

    it("returns false for syncMode none with syncForm false and form not null", () => {
      expect(shouldSyncFormFully("none", false, false)).toBe(false);
    });

    it("returns false for no syncMode with syncForm false", () => {
      expect(shouldSyncFormFully(undefined, false, false)).toBe(false);
    });
  });
});
