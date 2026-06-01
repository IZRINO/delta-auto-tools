import { describe, expect, it } from "vitest";

import {
  BUILTIN_STRATEGY_SITES,
  DEFAULT_STRATEGY_REFRESH_SECONDS,
  STRATEGY_REFRESH_INTERVAL_SECONDS,
  createStrategySite,
  createUserStrategySiteId,
  formatStrategyRefreshLabel,
  injectBaseHrefIntoHtml,
  mergeStrategySites,
  nextRefreshDelayMs,
  normalizeStrategyRefreshSeconds,
  readStoredRefreshSeconds,
  readStoredUserSites,
  writeStoredRefreshSeconds,
  writeStoredUserSites,
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

  describe("injectBaseHrefIntoHtml", () => {
    it("prepends base href inside <head> when head exists", () => {
      const html = "<html><head><title>KK</title></head><body>x</body></html>";
      const out = injectBaseHrefIntoHtml(html, "https://www.kkrb.net/");
      expect(out.indexOf("<base href=\"https://www.kkrb.net/\">")).toBeGreaterThan(-1);
      expect(out.indexOf("<base")).toBeLessThan(out.indexOf("<title>"));
    });

    it("adds a head section when only <html> is present", () => {
      const html = "<html><body>hello</body></html>";
      const out = injectBaseHrefIntoHtml(html, "https://orzice.com/");
      expect(out).toContain("<head><base href=\"https://orzice.com/\"></head>");
    });

    it("escapes attribute special characters", () => {
      const html = "<html><head></head><body></body></html>";
      const out = injectBaseHrefIntoHtml(html, "https://example.com/?a=1&b=\"<x>");
      expect(out).toContain("&amp;");
      expect(out).toContain("&quot;");
      expect(out).not.toContain("=\"<x>");
    });
  });

  describe("user site CRUD", () => {
    function makeStub() {
      const data = new Map<string, string>();
      return {
        data,
        getItem: (k: string) => (data.has(k) ? data.get(k)! : null),
        setItem: (k: string, v: string) => {
          data.set(k, v);
        },
      };
    }

    it("createUserStrategySiteId yields a user_ prefix and non-empty suffix", () => {
      const id = createUserStrategySiteId();
      expect(id.startsWith("user_")).toBe(true);
      expect(id.length).toBeGreaterThan(5);
    });

    it("createStrategySite rejects incomplete input", () => {
      expect(createStrategySite({ shortLabel: "", label: "x", url: "https://x" })).toBeNull();
      expect(createStrategySite({ shortLabel: "x", label: "", url: "https://x" })).toBeNull();
      expect(createStrategySite({ shortLabel: "x", label: "x", url: "" })).toBeNull();
      expect(createStrategySite({ shortLabel: "x", label: "x", url: "ftp://x" })).toBeNull();
    });

    it("createStrategySite produces a non-builtin site with user_ id", () => {
      const site = createStrategySite({
        shortLabel: "测试",
        label: "测试站点",
        url: "https://example.com/path",
        description: "test",
      });
      expect(site).not.toBeNull();
      expect(site?.id.startsWith("user_")).toBe(true);
      expect(site?.builtin).toBe(false);
      expect(site?.favicon).toBe("https://example.com/favicon.ico");
    });

    it("createStrategySite uses provided favicon / externalUrl when given", () => {
      const site = createStrategySite({
        shortLabel: "x",
        label: "x",
        url: "https://example.com/",
        externalUrl: "https://m.example.com/",
        favicon: "https://cdn.example.com/icon.png",
        description: "",
      });
      expect(site?.externalUrl).toBe("https://m.example.com/");
      expect(site?.favicon).toBe("https://cdn.example.com/icon.png");
    });

    it("writeStoredUserSites + readStoredUserSites roundtrip", () => {
      const stub = makeStub();
      const created = createStrategySite({
        shortLabel: "x",
        label: "x",
        url: "https://example.com/",
        description: "d",
      });
      expect(created).not.toBeNull();
      writeStoredUserSites([created!], stub);
      const restored = readStoredUserSites(stub);
      expect(restored).toHaveLength(1);
      expect(restored[0]?.url).toBe("https://example.com/");
      expect(restored[0]?.builtin).toBe(false);
    });

    it("readStoredUserSites drops entries with non-user_ ids", () => {
      const stub = makeStub();
      stub.setItem("delta-auto-tools:strategy:user-sites", JSON.stringify([
        { id: "kkrb", shortLabel: "k", label: "k", url: "https://k", description: "" },
        { id: "user_abc", shortLabel: "u", label: "u", url: "https://u", description: "" },
      ]));
      const restored = readStoredUserSites(stub);
      expect(restored).toHaveLength(1);
      expect(restored[0]?.id).toBe("user_abc");
    });

    it("readStoredUserSites returns [] on corrupted storage", () => {
      const stub = makeStub();
      stub.setItem("delta-auto-tools:strategy:user-sites", "not json");
      expect(readStoredUserSites(stub)).toEqual([]);
    });

    it("mergeStrategySites appends user sites after builtin ones", () => {
      const user = createStrategySite({
        shortLabel: "u",
        label: "u",
        url: "https://u.example.com/",
        description: "",
      });
      const merged = mergeStrategySites(BUILTIN_STRATEGY_SITES, [user!]);
      expect(merged.length).toBe(BUILTIN_STRATEGY_SITES.length + 1);
      expect(merged[merged.length - 1]?.id).toBe(user?.id);
    });
  });
});