import { describe, expect, it } from "vitest";

import {
  BUILTIN_STRATEGY_SITES,
  createStrategySite,
  createUserStrategySiteId,
  mergeStrategySites,
  readStoredUserSites,
  writeStoredUserSites,
} from "@/components/app/strategy-utils";

describe("strategy-utils", () => {
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
      expect(site?.url).toBe("https://example.com/path");
    });

    it("createStrategySite uses provided favicon when given", () => {
      const site = createStrategySite({
        shortLabel: "x",
        label: "x",
        url: "https://example.com/",
        favicon: "https://cdn.example.com/icon.png",
        description: "",
      });
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

    it("readStoredUserSites returns [] when storage is null", () => {
      expect(readStoredUserSites(null)).toEqual([]);
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
