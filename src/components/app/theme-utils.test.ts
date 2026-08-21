import {describe, expect, it} from "vitest";

import {UI_SCHEME_STORAGE_KEY, UI_WORLD_STORAGE_KEY} from "@/components/app/theme-types";
import type {ThemeDefinition, ThemeTokenOverride} from "@/components/app/theme-types";
import {
    applyThemeTokens,
    applyPersistedThemeTokens,
    buildCustomOverrideSettings,
    colorToHex,
    findTheme,
    materializeCustomOverrides,
    mergeThemeTokens,
    normalizeColorInput,
    normalizeHex,
    parseImportedTheme,
    parseUiScheme,
    parseUiWorld,
    presentThemeSession,
    previewThemeTokens,
    readUiScheme,
    readUiWorld,
    restorePersistedThemeTokens,
    serializeThemeForExport,
    writeUiScheme,
    writeUiWorld,
} from "@/components/app/theme-utils";

function makeTheme(id: string, tokens: Array<[string, string]>): ThemeDefinition {
    return {
        id,
        name: id,
        builtin: false,
        tokens: tokens.map(([key, value]) => ({key, value})),
    };
}

describe("mergeThemeTokens", () => {
    it("无 overrides 时返回主题 tokens 副本", () => {
        const theme = makeTheme("a", [["--x", "1"], ["--y", "2"]]);
        const merged = mergeThemeTokens(theme, []);
        expect(merged).toEqual(theme.tokens);
        // 应是副本，修改不影响原主题
        merged[0].value = "9";
        expect(theme.tokens[0].value).toBe("1");
    });

    it("overrides 覆盖同 key 的基底值", () => {
        const theme = makeTheme("a", [["--x", "1"], ["--y", "2"]]);
        const merged = mergeThemeTokens(theme, [{key: "--x", value: "9"}]);
        expect(merged).toEqual([
            {key: "--x", value: "9"},
            {key: "--y", value: "2"},
        ]);
    });

    it("overrides 独有 key 追加到末尾", () => {
        const theme = makeTheme("a", [["--x", "1"]]);
        const merged = mergeThemeTokens(theme, [{key: "--z", value: "3"}]);
        expect(merged).toEqual([
            {key: "--x", value: "1"},
            {key: "--z", value: "3"},
        ]);
    });

    it("保留基底顺序", () => {
        const theme = makeTheme("a", [["--a", "1"], ["--b", "2"], ["--c", "3"]]);
        const merged = mergeThemeTokens(theme, [
            {key: "--c", value: "30"},
            {key: "--a", value: "10"},
        ]);
        expect(merged.map((t) => t.key)).toEqual(["--a", "--b", "--c"]);
        expect(merged.map((t) => t.value)).toEqual(["10", "2", "30"]);
    });
});

describe("findTheme", () => {
    it("返回匹配项", () => {
        const themes = [makeTheme("a", []), makeTheme("b", [])];
        expect(findTheme(themes, "a")?.id).toBe("a");
    });

    it("未找到返回 undefined", () => {
        const themes = [makeTheme("a", [])];
        expect(findTheme(themes, "missing")).toBeUndefined();
    });
});

describe("materializeCustomOverrides", () => {
    it("保存自定义颜色时把当前主题合并成完整 token 集", () => {
        const theme = makeTheme("valentine", [
            ["--color-base-100", "oklch(21.5% 0 261.692)"],
            ["--color-primary", "oklch(70% 0.234 24.700)"],
            ["--color-base-content", "oklch(89% 0.02 261.692)"],
        ]);
        const overrides: ThemeTokenOverride[] = [{key: "--color-primary", value: "oklch(50% 0.2 20)"}];

        const tokens = materializeCustomOverrides(
            {
                activeThemeId: "valentine",
                builtinThemes: [theme],
                customThemes: [],
                overrides: [],
                mergedTokens: theme.tokens,
            },
            overrides,
        );

        expect(tokens).toEqual([
            {key: "--color-base-100", value: "oklch(21.5% 0 261.692)"},
            {key: "--color-primary", value: "oklch(50% 0.2 20)"},
            {key: "--color-base-content", value: "oklch(89% 0.02 261.692)"},
        ]);
    });

    it("自定义模式下继续保存时保留完整自定义 token 集", () => {
        const customTokens: ThemeTokenOverride[] = [
            {key: "--color-base-100", value: "oklch(20% 0 0)"},
            {key: "--color-primary", value: "oklch(50% 0.2 20)"},
        ];

        const tokens = materializeCustomOverrides(
            {
                activeThemeId: "",
                builtinThemes: [makeTheme("olive-amber", [["--color-base-100", "oklch(27% 0.072 132.109)"]])],
                customThemes: [],
                overrides: customTokens,
                mergedTokens: customTokens,
            },
            customTokens,
        );

        expect(tokens).toEqual(customTokens);
        expect(tokens).not.toBe(customTokens);
    });
});

describe("buildCustomOverrideSettings", () => {
    it("保存自定义颜色时取消选中主题并保留自定义主题列表", () => {
        const builtin = makeTheme("valentine", [
            ["--color-base-100", "oklch(21.5% 0 261.692)"],
            ["--color-primary", "oklch(70% 0.234 24.700)"],
        ]);
        const customTheme = makeTheme("custom-1", [["--color-primary", "oklch(40% 0.1 200)"]]);

        const settings = buildCustomOverrideSettings(
            {
                activeThemeId: "valentine",
                builtinThemes: [builtin],
                customThemes: [customTheme],
                overrides: [],
                mergedTokens: builtin.tokens,
            },
            [{key: "--color-primary", value: "oklch(50% 0.2 20)"}],
        );

        expect(settings).toEqual({
            activeThemeId: "",
            customThemes: [customTheme],
            overrides: [
                {key: "--color-base-100", value: "oklch(21.5% 0 261.692)"},
                {key: "--color-primary", value: "oklch(50% 0.2 20)"},
            ],
        });
    });
});

describe("applyThemeTokens", () => {
    function fakeElement(): HTMLElement {
        const el = {style: new Map<string, string>()} as unknown as HTMLElement;
        // 模拟 CSSStyleDeclaration 的 setProperty / removeProperty / getProperty
        (el.style as unknown as {
            setProperty: (k: string, v: string) => void;
            removeProperty: (k: string) => void;
        }).setProperty = (k: string, v: string) => {
            (el.style as unknown as Map<string, string>).set(k, v);
        };
        (el.style as unknown as {
            removeProperty: (k: string) => void;
        }).removeProperty = (k: string) => {
            (el.style as unknown as Map<string, string>).delete(k);
        };
        return el;
    }

    it("写入新 token", () => {
        const el = fakeElement();
        const tokens: ThemeTokenOverride[] = [{key: "--amber", value: "#E8A000"}];
        applyThemeTokens(el, tokens, []);
        expect((el.style as unknown as Map<string, string>).get("--amber")).toBe("#E8A000");
    });

    it("切换主题时清除旧 token", () => {
        const el = fakeElement();
        const old: ThemeTokenOverride[] = [{key: "--amber", value: "#E8A000"}];
        const next: ThemeTokenOverride[] = [{key: "--carbon", value: "#000000"}];
        applyThemeTokens(el, old, []);
        applyThemeTokens(el, next, old);
        const style = el.style as unknown as Map<string, string>;
        expect(style.get("--amber")).toBeUndefined();
        expect(style.get("--carbon")).toBe("#000000");
    });

    it("忽略非 -- 开头的 key", () => {
        const el = fakeElement();
        applyThemeTokens(el, [{key: "amber", value: "#E8A000"}], []);
        expect((el.style as unknown as Map<string, string>).size).toBe(0);
    });

    it("target 为 null 时安全返回", () => {
        expect(() => applyThemeTokens(null, [], [])).not.toThrow();
    });
});

describe("theme token session", () => {
    function fakeElement(): HTMLElement {
        const el = {style: new Map<string, string>()} as unknown as HTMLElement;
        (el.style as unknown as {
            setProperty: (k: string, v: string) => void;
            removeProperty: (k: string) => void;
        }).setProperty = (k: string, v: string) => {
            (el.style as unknown as Map<string, string>).set(k, v);
        };
        (el.style as unknown as {
            removeProperty: (k: string) => void;
        }).removeProperty = (k: string) => {
            (el.style as unknown as Map<string, string>).delete(k);
        };
        return el;
    }

    it("普通预览关闭后恢复到已持久化 token", () => {
        const el = fakeElement();
        let session = applyPersistedThemeTokens(el, [{key: "--amber", value: "#e8a000"}], {
            appliedTokens: [],
            persistedTokens: [],
        });

        session = previewThemeTokens(el, [{key: "--amber", value: "#ff0000"}], session);
        expect((el.style as unknown as Map<string, string>).get("--amber")).toBe("#ff0000");

        session = restorePersistedThemeTokens(el, session);
        expect((el.style as unknown as Map<string, string>).get("--amber")).toBe("#e8a000");
    });

    it("标记为关闭后保留的预览不会被恢复成旧主题", () => {
        const el = fakeElement();
        let session = applyPersistedThemeTokens(el, [{key: "--amber", value: "#e8a000"}], {
            appliedTokens: [],
            persistedTokens: [],
        });

        session = previewThemeTokens(
            el,
            [{key: "--amber", value: "#ff0000"}],
            session,
            {persistOnClose: true},
        );
        session = restorePersistedThemeTokens(el, session);

        expect((el.style as unknown as Map<string, string>).get("--amber")).toBe("#ff0000");
    });
});

describe("parseImportedTheme", () => {
    it("解析合法 JSON", () => {
        const json = JSON.stringify({
            id: "custom",
            name: "自定义",
            builtin: true,
            tokens: [{key: "--amber", value: "#FF0000"}],
        });
        const theme = parseImportedTheme(json);
        expect(theme.id).toBe("custom");
        expect(theme.name).toBe("自定义");
        // 导入后 builtin 强制为 false
        expect(theme.builtin).toBe(false);
        expect(theme.tokens).toEqual([{key: "--amber", value: "#FF0000"}]);
    });

    it("拒绝缺少 id 的 JSON", () => {
        const json = JSON.stringify({name: "x", builtin: false, tokens: []});
        expect(() => parseImportedTheme(json)).toThrow("id");
    });

    it("拒绝非 -- 开头的 token key", () => {
        const json = JSON.stringify({
            id: "x",
            name: "x",
            builtin: false,
            tokens: [{key: "amber", value: "#FF0000"}],
        });
        expect(() => parseImportedTheme(json)).toThrow("--");
    });

    it("拒绝非对象输入", () => {
        expect(() => parseImportedTheme("[]")).toThrow();
        expect(() => parseImportedTheme('"string"')).toThrow();
    });
});

describe("serializeThemeForExport", () => {
    it("输出 pretty JSON 含全部字段", () => {
        const theme = makeTheme("a", [["--amber", "#E8A000"]]);
        const json = serializeThemeForExport(theme);
        expect(JSON.parse(json)).toEqual(theme);
        expect(json).toContain("\n");
    });
});

describe("normalizeHex", () => {
    it("规范 6 位 hex", () => {
        expect(normalizeHex("#E8A000")).toBe("#e8a000");
        expect(normalizeHex("E8A000")).toBe("#e8a000");
    });

    it("展开 3 位 hex", () => {
        expect(normalizeHex("#fff")).toBe("#ffffff");
        expect(normalizeHex("f00")).toBe("#ff0000");
    });

    it("非法输入原样返回", () => {
        expect(normalizeHex("not-a-color")).toBe("not-a-color");
        expect(normalizeHex("#12")).toBe("#12");
    });
});

describe("normalizeColorInput", () => {
    it("hex 转 oklch 字符串", () => {
        const result = normalizeColorInput("#e8a000");
        expect(result).toMatch(/^oklch\(0\.\d+ 0\.\d+ \d+\.\d+\)$/);
    });

    it("oklch 输入仍规范化为 oklch 字符串", () => {
        const result = normalizeColorInput("oklch(0.7 0.2 25)");
        expect(result).toMatch(/^oklch\(0\.700000 0\.200000 25\.000000\)$/);
    });

    it("hex→oklch→hex 往返零误差（RGB 整数复原）", () => {
        const oklchStr = normalizeColorInput("#e8a000");
        expect(colorToHex(oklchStr)).toBe("#e8a000");
    });

    it("非法输入原样返回", () => {
        expect(normalizeColorInput("not-a-color")).toBe("not-a-color");
    });
});

describe("colorToHex", () => {
    it("oklch 字符串转 hex", () => {
        expect(colorToHex("oklch(0.7 0.2 25)")).toMatch(/^#[0-9a-f]{6}$/);
    });

    it("hex 原样归一", () => {
        expect(colorToHex("#ff0000")).toBe("#ff0000");
    });

    it("非法输入回退 #000000", () => {
        expect(colorToHex("not-a-color")).toBe("#000000");
    });
});

describe("ui world", () => {
    it("只认 console，其余回 blackmark", () => {
        expect(parseUiWorld("console")).toBe("console");
        expect(parseUiWorld("blackmark")).toBe("blackmark");
        expect(parseUiWorld("valentine")).toBe("blackmark");
        expect(parseUiWorld(null)).toBe("blackmark");
    });

    it("读写 localStorage 键", () => {
        const store = new Map<string, string>();
        const storage = {
            getItem: (key: string) => store.get(key) ?? null,
            setItem: (key: string, value: string) => {
                store.set(key, value);
            },
        };
        expect(readUiWorld(storage)).toBe("blackmark");
        writeUiWorld(storage, "console");
        expect(store.get(UI_WORLD_STORAGE_KEY)).toBe("console");
        expect(readUiWorld(storage)).toBe("console");
        writeUiWorld(storage, "blackmark");
        expect(readUiWorld(storage)).toBe("blackmark");
    });

    it("黑标清掉根节点 token，不改落盘 session", () => {
        const el = {
            style: new Map<string, string>(),
        } as unknown as HTMLElement;
        (el.style as unknown as {
            setProperty: (k: string, v: string) => void;
            removeProperty: (k: string) => void;
        }).setProperty = (k, v) => {
            (el.style as unknown as Map<string, string>).set(k, v);
        };
        (el.style as unknown as {removeProperty: (k: string) => void}).removeProperty = (k) => {
            (el.style as unknown as Map<string, string>).delete(k);
        };

        const persisted: ThemeTokenOverride[] = [{key: "--color-primary", value: "red"}];
        let session = presentThemeSession(
            el,
            {appliedTokens: [], persistedTokens: []},
            "console",
            persisted,
        );
        expect((el.style as unknown as Map<string, string>).get("--color-primary")).toBe("red");
        expect(session.appliedTokens).toEqual(persisted);

        session = presentThemeSession(el, session, "blackmark");
        expect((el.style as unknown as Map<string, string>).has("--color-primary")).toBe(false);
        expect(session.appliedTokens).toEqual([]);
        expect(session.persistedTokens).toEqual(persisted);
    });
});

describe("ui scheme", () => {
    it("只认 day，其余回 night", () => {
        expect(parseUiScheme("day")).toBe("day");
        expect(parseUiScheme("night")).toBe("night");
        expect(parseUiScheme("console")).toBe("night");
        expect(parseUiScheme(null)).toBe("night");
    });

    it("读写 localStorage 键", () => {
        const store = new Map<string, string>();
        const storage = {
            getItem: (key: string) => store.get(key) ?? null,
            setItem: (key: string, value: string) => {
                store.set(key, value);
            },
        };
        expect(readUiScheme(storage)).toBe("night");
        writeUiScheme(storage, "day");
        expect(store.get(UI_SCHEME_STORAGE_KEY)).toBe("day");
        expect(readUiScheme(storage)).toBe("day");
    });
});
