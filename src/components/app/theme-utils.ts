/**
 * 主题引擎纯逻辑工具函数。
 *
 * 所有函数均不依赖 React / Tauri，可在 Node 测试环境直接调用。
 */

import {formatHex, oklch, parse} from "culori";

import type {
    ThemeBootstrap,
    ThemeDefinition,
    ThemeSettings,
    ThemeTokenOverride,
    UiScheme,
    UiWorld,
} from "@/components/app/theme-types";
import {UI_SCHEME_STORAGE_KEY, UI_WORLD_STORAGE_KEY} from "@/components/app/theme-types";

/**
 * 把合并后的 token 列表写入目标元素的 inline style。
 *
 * inline style 优先级高于 `:root`，因此能覆盖 `App.css` 的默认值。
 * 调用方传入 `document.documentElement` 即可全局换肤。
 *
 * 注意：切换主题时不会自动清除旧主题的 inline style。调用方应在写入新 token 前
 * 先调用 `clearThemeTokens`，或使用 `applyThemeTokens` 帮你做这件事。
 */
export function setThemeTokens(
    target: HTMLElement | SVGElement | null,
    tokens: readonly ThemeTokenOverride[],
): void {
    if (!target) return;
    for (const token of tokens) {
        if (!token.key.startsWith("--")) continue;
        target.style.setProperty(token.key, token.value);
    }
}

/**
 * 清除目标元素上由主题注入的 inline style。
 *
 * 传入之前应用过的 token 列表，逐个 `removeProperty` 回落到 `:root` 默认值。
 */
export function clearThemeTokens(
    target: HTMLElement | SVGElement | null,
    tokens: readonly ThemeTokenOverride[],
): void {
    if (!target) return;
    for (const token of tokens) {
        if (!token.key.startsWith("--")) continue;
        target.style.removeProperty(token.key);
    }
}

/**
 * 原子化切换主题：先清旧 token，再写新 token。
 *
 * 返回新应用的 token 列表，调用方可保存下来供下次切换时清除用。
 */
export function applyThemeTokens(
    target: HTMLElement | SVGElement | null,
    newTokens: readonly ThemeTokenOverride[],
    previousTokens: readonly ThemeTokenOverride[] = [],
): readonly ThemeTokenOverride[] {
    clearThemeTokens(target, previousTokens);
    setThemeTokens(target, newTokens);
    return newTokens;
}

export type ThemeTokenSession = {
    appliedTokens: readonly ThemeTokenOverride[];
    persistedTokens: readonly ThemeTokenOverride[];
};

export function applyPersistedThemeTokens(
    target: HTMLElement | SVGElement | null,
    tokens: readonly ThemeTokenOverride[],
    session: ThemeTokenSession,
): ThemeTokenSession {
    const appliedTokens = applyThemeTokens(target, tokens, session.appliedTokens);
    return {
        appliedTokens,
        persistedTokens: appliedTokens,
    };
}

export function previewThemeTokens(
    target: HTMLElement | SVGElement | null,
    tokens: readonly ThemeTokenOverride[],
    session: ThemeTokenSession,
    options?: {persistOnClose?: boolean},
): ThemeTokenSession {
    const appliedTokens = applyThemeTokens(target, tokens, session.appliedTokens);
    return {
        appliedTokens,
        persistedTokens: options?.persistOnClose ? appliedTokens : session.persistedTokens,
    };
}

export function restorePersistedThemeTokens(
    target: HTMLElement | SVGElement | null,
    session: ThemeTokenSession,
): ThemeTokenSession {
    const appliedTokens = applyThemeTokens(
        target,
        session.persistedTokens,
        session.appliedTokens,
    );
    return {
        appliedTokens,
        persistedTokens: session.persistedTokens,
    };
}

export function parseUiWorld(value: string | null | undefined): UiWorld {
    return value === "blackmark" ? "blackmark" : "console";
}

export function readUiWorld(storage: Pick<Storage, "getItem"> | null | undefined): UiWorld {
    if (!storage) return "console";
    try {
        return parseUiWorld(storage.getItem(UI_WORLD_STORAGE_KEY));
    } catch {
        return "console";
    }
}

export function writeUiWorld(
    storage: Pick<Storage, "setItem"> | null | undefined,
    world: UiWorld,
): void {
    try {
        storage?.setItem(UI_WORLD_STORAGE_KEY, world);
    } catch {
        // 隐私模式 / 配额：静默
    }
}

export function parseUiScheme(value: string | null | undefined): UiScheme {
    return value === "day" ? "day" : "night";
}

export function readUiScheme(storage: Pick<Storage, "getItem"> | null | undefined): UiScheme {
    if (!storage) return "night";
    try {
        return parseUiScheme(storage.getItem(UI_SCHEME_STORAGE_KEY));
    } catch {
        return "night";
    }
}

export function writeUiScheme(
    storage: Pick<Storage, "setItem"> | null | undefined,
    scheme: UiScheme,
): void {
    try {
        storage?.setItem(UI_SCHEME_STORAGE_KEY, scheme);
    } catch {
        // 隐私模式 / 配额：静默
    }
}

/**
 * 战地：把落盘 daisyUI token 打到根节点。
 * 黑标：清掉根节点 inline token，壳用 --bm-*，不劫持 28 key。
 */
export function presentThemeSession(
    target: HTMLElement | SVGElement | null,
    session: ThemeTokenSession,
    world: UiWorld,
    nextPersisted: readonly ThemeTokenOverride[] = session.persistedTokens,
): ThemeTokenSession {
    if (world === "blackmark") {
        clearThemeTokens(target, session.appliedTokens);
        return {appliedTokens: [], persistedTokens: nextPersisted};
    }
    return {
        appliedTokens: applyThemeTokens(target, nextPersisted, session.appliedTokens),
        persistedTokens: nextPersisted,
    };
}

/** 在所有主题（内置 + 自定义）中按 id 查找主题定义。 */
export function findTheme(
    themes: readonly ThemeDefinition[],
    id: string,
): ThemeDefinition | undefined {
    return themes.find((t) => t.id === id);
}

/**
 * 合并主题 tokens 与 overrides（overrides 优先）。
 *
 * 与 Rust `apply::merge_theme_tokens` 语义一致：
 * - 以主题 tokens 为基底；
 * - overrides 中同 key 的项覆盖基底值；
 * - overrides 中独有的 key 追加到末尾；
 * - 保留基底顺序。
 */
export function mergeThemeTokens(
    theme: ThemeDefinition,
    overrides: readonly ThemeTokenOverride[],
): ThemeTokenOverride[] {
    const result: ThemeTokenOverride[] = [];
    const consumedKeys = new Set<string>();

    for (const tok of theme.tokens) {
        const override = overrides.find((o) => o.key === tok.key);
        if (override) {
            result.push({...override});
        } else {
            result.push({...tok});
        }
        consumedKeys.add(tok.key);
    }

    for (const ov of overrides) {
        if (!consumedKeys.has(ov.key)) {
            result.push({...ov});
            consumedKeys.add(ov.key);
        }
    }

    return result;
}

/**
 * 把用户编辑中的颜色转成完整自定义 token 集。
 *
 * 保存自定义颜色后 `activeThemeId` 会置空，因此必须把当前主题基底一并固化，
 * 避免关闭设置或重启后只剩少量覆盖项而回落到默认 CSS 变量。
 */
export function materializeCustomOverrides(
    bootstrap: ThemeBootstrap,
    overrides: readonly ThemeTokenOverride[],
): ThemeTokenOverride[] {
    if (bootstrap.activeThemeId === "") {
        return overrides.map((token) => ({...token}));
    }

    const activeTheme = findTheme(
        [...bootstrap.customThemes, ...bootstrap.builtinThemes],
        bootstrap.activeThemeId,
    );

    return mergeThemeTokens(
        activeTheme ?? {
            id: "__current__",
            name: "当前配色",
            builtin: false,
            tokens: bootstrap.mergedTokens,
        },
        overrides,
    );
}

/** 构造保存自定义颜色所需的设置：取消主题选中，并保存完整 token 集。 */
export function buildCustomOverrideSettings(
    bootstrap: ThemeBootstrap,
    overrides: readonly ThemeTokenOverride[],
): ThemeSettings {
    return {
        activeThemeId: "",
        customThemes: bootstrap.customThemes,
        overrides: materializeCustomOverrides(bootstrap, overrides),
    };
}

/** 把 ThemeDefinition 序列化为可导入导出的 JSON 字符串（pretty）。 */
export function serializeThemeForExport(theme: ThemeDefinition): string {
    return JSON.stringify(theme, null, 2);
}

/** 解析导入的 JSON 为 ThemeDefinition，校验 token key 必须 `--` 开头。 */
export function parseImportedTheme(json: string): ThemeDefinition {
    const parsed = JSON.parse(json) as unknown;
    if (typeof parsed !== "object" || parsed === null) {
        throw new Error("主题 JSON 必须是对象");
    }
    const obj = parsed as Record<string, unknown>;
    if (typeof obj.id !== "string") throw new Error("主题 id 必须是字符串");
    if (typeof obj.name !== "string") throw new Error("主题 name 必须是字符串");
    if (typeof obj.builtin !== "boolean") throw new Error("主题 builtin 必须是布尔值");
    if (!Array.isArray(obj.tokens)) throw new Error("主题 tokens 必须是数组");

    const tokens: ThemeTokenOverride[] = [];
    for (const raw of obj.tokens) {
        if (typeof raw !== "object" || raw === null) throw new Error("token 项必须是对象");
        const tok = raw as Record<string, unknown>;
        if (typeof tok.key !== "string" || typeof tok.value !== "string") {
            throw new Error("token 项必须含 key 和 value 字符串");
        }
        if (!tok.key.startsWith("--")) {
            throw new Error(`token key "${tok.key}" 必须以 -- 开头`);
        }
        tokens.push({key: tok.key, value: tok.value});
    }

    return {
        id: obj.id,
        name: obj.name,
        builtin: false, // 导入的主题一律标记为非内置，避免被当成内置主题处理
        tokens,
    };
}

/**
 * 把 hex 颜色（#RRGGBB / #RGB）规范化为 `#RRGGBB` 小写形式。
 *
 * 用于颜色选择器的 hex 输入框回填。非法输入原样返回。
 */
export function normalizeHex(value: string): string {
    const trimmed = value.trim();
    if (/^#?[0-9a-fA-F]{6}$/.test(trimmed)) {
        const hex = trimmed.startsWith("#") ? trimmed.slice(1) : trimmed;
        return `#${hex.toLowerCase()}`;
    }
    if (/^#?[0-9a-fA-F]{3}$/.test(trimmed)) {
        const hex = trimmed.startsWith("#") ? trimmed.slice(1) : trimmed;
        const expanded = hex
            .split("")
            .map((c) => c + c)
            .join("");
        return `#${expanded.toLowerCase()}`;
    }
    return trimmed;
}

/**
 * 把任意合法 CSS 颜色字符串（hex / oklch() / rgb() 等）规范化为 `oklch(L C H)` 字符串。
 *
 * 用 culori 做解析与 oklch 转换。新主题 token 统一存 oklch 字符串；
 * 旧主题遗留的 hex 值经此函数转为 oklch。无法解析的输入原样返回。
 */
export function normalizeColorInput(value: string): string {
    const trimmed = value.trim();
    const parsed = parse(trimmed);
    if (!parsed) return trimmed;
    const c = oklch(parsed);
    if (!c || c.l === undefined || c.c === undefined) return trimmed;
    // ponytail: hue 对无彩色（c=0）可能为 NaN/undefined，回退 0
    const h = c.h === undefined || Number.isNaN(c.h) ? 0 : c.h;
    return `oklch(${round(c.l)} ${round(c.c)} ${round(h)})`;
}

/**
 * 把任意合法 CSS 颜色字符串转为 `#rrggbb`，供原生 `<input type="color">` 使用。
 *
 * 无法解析时回退 `#000000`。
 */
export function colorToHex(value: string): string {
    const parsed = parse(value.trim());
    if (!parsed) return "#000000";
    const hex = formatHex(parsed);
    return hex ?? "#000000";
}

function round(n: number): string {
    // ponytail: 6 位小数足够 hex 往返精度，避免浮点尾噪
    return n.toFixed(6);
}
