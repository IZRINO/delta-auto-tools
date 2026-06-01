import { describe, expect, it } from "vitest";

import {
  DEFAULT_STRATEGY_REFRESH_SECONDS,
  STRATEGY_REFRESH_INTERVAL_SECONDS,
  formatStrategyRefreshLabel,
  nextRefreshDelayMs,
  normalizeStrategyRefreshSeconds,
  readStoredRefreshSeconds,
  writeStoredRefreshSeconds,
} from "@/components/app/strategy-utils";

describe("strategy-utils", () => {
  describe("normalizeStrategyRefreshSeconds", () => {
    it("returns null for nullish inputs", () => {
      expect(normalizeStrategyRefreshSeconds(null)).toBeNull();
      expect(normalizeStrategyRefreshSeconds(undefined)).toBeNull();
    });

    it("returns null for non-positive or non-finite numbers", () => {
      expect(normalizeStrategyRefreshSeconds(0)).toBeNull();
      expect(normalizeStrategyRefreshSeconds(-1)).toBeNull();
      expect(normalizeStrategyRefreshSeconds(Number.NaN)).toBeNull();
      expect(normalizeStrategyRefreshSeconds(Number.POSITIVE_INFINITY)).toBeNull();
    });

    it("keeps known bucket values unchanged", () => {
      for (const bucket of STRATEGY_REFRESH_INTERVAL_SECONDS) {
        expect(normalizeStrategyRefreshSeconds(bucket)).toBe(bucket);
      }
    });

    it("rounds up to the nearest known bucket", () => {
      expect(normalizeStrategyRefreshSeconds(45)).toBe(60);
      expect(normalizeStrategyRefreshSeconds(90)).toBe(120);
      expect(normalizeStrategyRefreshSeconds(200)).toBe(300);
    });

    it("clamps to the largest bucket for values above the ceiling", () => {
      const max = STRATEGY_REFRESH_INTERVAL_SECONDS[STRATEGY_REFRESH_INTERVAL_SECONDS.length - 1];
      expect(normalizeStrategyRefreshSeconds(9999)).toBe(max);
    });
  });

  describe("formatStrategyRefreshLabel", () => {
    it("uses 关闭 for disabled state", () => {
      expect(formatStrategyRefreshLabel(null)).toBe("关闭");
    });

    it("uses 秒 suffix for sub-minute values", () => {
      expect(formatStrategyRefreshLabel(30)).toBe("30 秒");
    });

    it("uses 分钟 suffix for sub-hour values", () => {
      expect(formatStrategyRefreshLabel(60)).toBe("1 分钟");
      expect(formatStrategyRefreshLabel(300)).toBe("5 分钟");
    });
  });

  describe("nextRefreshDelayMs", () => {
    it("returns null when disabled", () => {
      expect(nextRefreshDelayMs(null)).toBeNull();
    });

    it("converts seconds to milliseconds", () => {
      expect(nextRefreshDelayMs(30)).toBe(30_000);
      expect(nextRefreshDelayMs(300)).toBe(300_000);
    });
  });

  describe("storage round-trip", () => {
    function makeStub(): { data: Map<string, string>; getItem: (k: string) => string | null; setItem: (k: string, v: string) => void } {
      const data = new Map<string, string>();
      return {
        data,
        getItem: (k: string) => (data.has(k) ? data.get(k)! : null),
        setItem: (k: string, v: string) => {
          data.set(k, v);
        },
      };
    }

    it("returns the default bucket when no value is stored", () => {
      const stub = makeStub();
      expect(readStoredRefreshSeconds("kkrb", stub)).toBe(DEFAULT_STRATEGY_REFRESH_SECONDS);
    });

    it("persists numeric buckets as integers and reads them back", () => {
      const stub = makeStub();
      writeStoredRefreshSeconds("kkrb", 120, stub);
      expect(stub.data.get("delta-auto-tools:strategy:kkrb")).toBe("120");
      expect(readStoredRefreshSeconds("kkrb", stub)).toBe(120);
    });

    it("persists the disabled state as the literal off marker", () => {
      const stub = makeStub();
      writeStoredRefreshSeconds("kkrb", null, stub);
      expect(stub.data.get("delta-auto-tools:strategy:kkrb")).toBe("off");
      expect(readStoredRefreshSeconds("kkrb", stub)).toBeNull();
    });

    it("falls back to disabled when stored payload is malformed", () => {
      const stub = makeStub();
      stub.data.set("delta-auto-tools:strategy:kkrb", "garbage");
      expect(readStoredRefreshSeconds("kkrb", stub)).toBeNull();
    });

    it("returns the default when storage is null", () => {
      expect(readStoredRefreshSeconds("kkrb", null)).toBe(DEFAULT_STRATEGY_REFRESH_SECONDS);
    });
  });
});
