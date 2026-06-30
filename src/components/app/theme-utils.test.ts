import {describe, expect, it} from "vitest";

import type {ThemeDefinition, ThemeTokenOverride} from "@/components/app/theme-types";
import {
    applyThemeTokens,
    applyPersistedThemeTokens,
    buildCustomOverrideSettings,
    findTheme,
    materializeCustomOverrides,
    mergeThemeTokens,
    normalizeHex,
    parseImportedTheme,
    previewThemeTokens,
    restorePersistedThemeTokens,
    serializeThemeForExport,
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
        const theme = makeTheme("industrial-dark", [
            ["--carbon", "#0c0c0b"],
            ["--amber", "#e8a000"],
            ["--chalk", "#d8d4cc"],
        ]);
        const overrides: ThemeTokenOverride[] = [{key: "--amber", value: "#ff0000"}];

        const tokens = materializeCustomOverrides(
            {
                activeThemeId: "industrial-dark",
                builtinThemes: [theme],
                customThemes: [],
                overrides: [],
                mergedTokens: theme.tokens,
            },
            overrides,
        );

        expect(tokens).toEqual([
            {key: "--carbon", value: "#0c0c0b"},
            {key: "--amber", value: "#ff0000"},
            {key: "--chalk", value: "#d8d4cc"},
        ]);
    });

    it("自定义模式下继续保存时保留完整自定义 token 集", () => {
        const customTokens: ThemeTokenOverride[] = [
            {key: "--carbon", value: "#111111"},
            {key: "--amber", value: "#ff0000"},
        ];

        const tokens = materializeCustomOverrides(
            {
                activeThemeId: "",
                builtinThemes: [makeTheme("industrial-light", [["--carbon", "#ffffff"]])],
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
        const builtin = makeTheme("industrial-dark", [
            ["--carbon", "#0c0c0b"],
            ["--amber", "#e8a000"],
        ]);
        const customTheme = makeTheme("custom-1", [["--amber", "#00ff00"]]);

        const settings = buildCustomOverrideSettings(
            {
                activeThemeId: "industrial-dark",
                builtinThemes: [builtin],
                customThemes: [customTheme],
                overrides: [],
                mergedTokens: builtin.tokens,
            },
            [{key: "--amber", value: "#ff0000"}],
        );

        expect(settings).toEqual({
            activeThemeId: "",
            customThemes: [customTheme],
            overrides: [
                {key: "--carbon", value: "#0c0c0b"},
                {key: "--amber", value: "#ff0000"},
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
