import {describe, expect, it} from "vitest";

import {
    BUILTIN_STRATEGY_SITES,
    createStrategySite,
    createUserStrategySiteId,
    DEFAULT_STRATEGY_REFRESH_SECONDS,
    mergeStrategySites,
    normalizeStrategyContentBounds,
    normalizeVisibleStrategyContentBounds,
    readStoredUserSites,
    readStrategyRefreshSeconds,
    writeStoredUserSites,
    writeStrategyRefreshSeconds,
} from "@/components/app/strategy-utils";

describe("strategy-utils", () => {
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

    describe("user site CRUD", () => {
        it("createUserStrategySiteId yields a user_ prefix and non-empty suffix", () => {
            const id = createUserStrategySiteId();
            expect(id.startsWith("user_")).toBe(true);
            expect(id.length).toBeGreaterThan(5);
        });

        it("createStrategySite rejects incomplete input", () => {
            expect(createStrategySite({shortLabel: "", label: "x", url: "https://x"})).toBeNull();
            expect(createStrategySite({shortLabel: "x", label: "", url: "https://x"})).toBeNull();
            expect(createStrategySite({shortLabel: "x", label: "x", url: ""})).toBeNull();
            expect(createStrategySite({shortLabel: "x", label: "x", url: "ftp://x"})).toBeNull();
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

        it("readStoredUserSites accepts preset kkrb/orzice and user_ ids", () => {
            const stub = makeStub();
            stub.setItem("delta-auto-tools:strategy:user-sites", JSON.stringify([
                {id: "kkrb", shortLabel: "k", label: "k", url: "https://k", description: ""},
                {id: "user_abc", shortLabel: "u", label: "u", url: "https://u", description: ""},
                {id: "invalid_id", shortLabel: "i", label: "i", url: "https://i", description: ""},
            ]));
            const restored = readStoredUserSites(stub);
            expect(restored).toHaveLength(2);
            expect(restored[0]?.id).toBe("kkrb");
            expect(restored[1]?.id).toBe("user_abc");
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

    describe("refresh persistence", () => {
        it("writeStrategyRefreshSeconds + readStrategyRefreshSeconds roundtrip", () => {
            const stub = makeStub();
            writeStrategyRefreshSeconds("kkrb", 60, stub);
            expect(readStrategyRefreshSeconds("kkrb", stub)).toBe(60);
            expect(stub.data.get("delta-auto-tools:strategy:kkrb:refresh-seconds")).toBe("60");
        });

        it("readStrategyRefreshSeconds falls back on corrupted storage", () => {
            const stub = makeStub();
            stub.setItem("delta-auto-tools:strategy:kkrb:refresh-seconds", "not-number");
            expect(readStrategyRefreshSeconds("kkrb", stub)).toBe(DEFAULT_STRATEGY_REFRESH_SECONDS);
        });

        it("writeStrategyRefreshSeconds normalizes illegal seconds", () => {
            const stub = makeStub();
            writeStrategyRefreshSeconds("kkrb", 45, stub);
            expect(readStrategyRefreshSeconds("kkrb", stub)).toBe(DEFAULT_STRATEGY_REFRESH_SECONDS);
            expect(stub.data.get("delta-auto-tools:strategy:kkrb:refresh-seconds")).toBe("0");
        });

        it("readStrategyRefreshSeconds isolates different site keys", () => {
            const stub = makeStub();
            writeStrategyRefreshSeconds("kkrb", 30, stub);
            writeStrategyRefreshSeconds("orzice", 300, stub);
            expect(readStrategyRefreshSeconds("kkrb", stub)).toBe(30);
            expect(readStrategyRefreshSeconds("orzice", stub)).toBe(300);
        });

        it("readStrategyRefreshSeconds ignores illegal site id", () => {
            const stub = makeStub();
            stub.setItem("delta-auto-tools:strategy:bad:refresh-seconds", "60");
            expect(readStrategyRefreshSeconds("bad", stub)).toBe(DEFAULT_STRATEGY_REFRESH_SECONDS);
        });
    });

    describe("content bounds", () => {
        it("normalizes non-zero host rects for WebView placement", () => {
            expect(normalizeStrategyContentBounds({left: 10.4, top: 20.6, width: 801.2, height: 560.8})).toEqual({
                x: 10,
                y: 21,
                width: 801,
                height: 561,
            });
        });

        it("keeps a usable minimum while the host is still laying out", () => {
            expect(normalizeStrategyContentBounds({left: 0, top: 0, width: 0, height: 0})).toEqual({
                x: 0,
                y: 0,
                width: 320,
                height: 360,
            });
            expect(normalizeStrategyContentBounds(null)).toEqual({
                x: 0,
                y: 0,
                width: 320,
                height: 360,
            });
        });

        it("clips host rects to the visible viewport for detached WebView windows", () => {
            expect(
                normalizeVisibleStrategyContentBounds(
                    {left: 240, top: 520, width: 860, height: 560},
                    {width: 1280, height: 800},
                ),
            ).toEqual({
                x: 240,
                y: 520,
                width: 860,
                height: 280,
            });
            expect(
                normalizeVisibleStrategyContentBounds(
                    {left: 240, top: -120, width: 860, height: 560},
                    {width: 1280, height: 800},
                ),
            ).toEqual({
                x: 240,
                y: 0,
                width: 860,
                height: 440,
            });
        });

        it("returns null when the content host is outside the visible viewport", () => {
            expect(
                normalizeVisibleStrategyContentBounds(
                    {left: 240, top: 820, width: 860, height: 560},
                    {width: 1280, height: 800},
                ),
            ).toBeNull();
            expect(normalizeVisibleStrategyContentBounds(null, {width: 1280, height: 800})).toBeNull();
        });
    });
});
