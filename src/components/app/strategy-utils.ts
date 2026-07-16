/**
 * 攻略网站（strategy）模块的纯逻辑工具函数。
 *
 * 该模块只承载：站点常量、用户站点 CRUD、本地存储读写。
 * 不包含 React 组件、副作用与 Tauri 命令调用，方便被单元测试覆盖。
 */

/**
 * 内置站点 ID：kkrb / orzice 不允许用户删除。
 *
 * 用户新增的站点会得到形如 `user_<random>` 的 ID，避免与内置 ID 冲突。
 */
export type BuiltinStrategySiteId = "kkrb" | "orzice";
export type StrategySiteId = BuiltinStrategySiteId | `user_${string}`;

export type StrategySite = {
    /** 站点内部 ID，用于本地存储、aria-label 等 */
    id: StrategySiteId;
    /** UI 上展示的简称，2-6 个汉字 / 字符 */
    shortLabel: string;
    /** UI 上展示的完整中文标签 */
    label: string;
    /** 目标 URL（同时是应用内 webview 与外部浏览器打开的入口） */
    url: string;
    /** 站点 favicon；缺省时回退到默认 favicon */
    favicon: string;
    /** 站点简介 */
    description: string;
    /** 是否为内置站点（不允许删除） */
    builtin: boolean;
};

export type StrategyContentBounds = {
    x: number;
    y: number;
    width: number;
    height: number;
};

export type StrategyContentRectLike = Pick<DOMRectReadOnly, "left" | "top" | "width" | "height"> | null | undefined;

export type StrategyContentViewportLike = Pick<DOMRectReadOnly, "width" | "height"> | null | undefined;

export const DEFAULT_STRATEGY_REFRESH_SECONDS = 0;
export const STRATEGY_CONTENT_MIN_WIDTH = 320;
export const STRATEGY_CONTENT_MIN_HEIGHT = 360;

export const STRATEGY_REFRESH_OPTIONS = [
    {seconds: DEFAULT_STRATEGY_REFRESH_SECONDS, label: "关闭"},
    {seconds: 30, label: "30 秒"},
    {seconds: 60, label: "1 分钟"},
    {seconds: 120, label: "2 分钟"},
    {seconds: 300, label: "5 分钟"},
    {seconds: 600, label: "10 分钟"},
] as const;

export type StrategyRefreshSeconds = (typeof STRATEGY_REFRESH_OPTIONS)[number]["seconds"];

const STRATEGY_REFRESH_SECONDS_ALLOWED: Record<StrategyRefreshSeconds, true> = {
    0: true,
    30: true,
    60: true,
    120: true,
    300: true,
    600: true,
};

function isStrategyRefreshSeconds(value: number): value is StrategyRefreshSeconds {
    return Number.isInteger(value) && STRATEGY_REFRESH_SECONDS_ALLOWED[value as StrategyRefreshSeconds] === true;
}

/**
 * 内置两个攻略站点（只读、不可删除）。
 *
 * - kkrb → https://www.kkrb.net/?viewpage=view%2Foverview
 * - orzice → https://orzice.com/v/rb
 */
export const BUILTIN_STRATEGY_SITES: ReadonlyArray<StrategySite> = [
    {
        id: "kkrb",
        shortLabel: "KK 日报",
        label: "KK 日报攻略总览",
        url: "https://www.kkrb.net/?viewpage=view%2Foverview",
        favicon: "https://www.kkrb.net/favicon.ico",
        description: "覆盖地图任务、藏宝、跑刀路线的高频更新攻略总览。",
        builtin: false,
    },
    {
        id: "orzice",
        shortLabel: "Orzice",
        label: "Orzice RB 攻略",
        url: "https://orzice.com/v/rb",
        favicon: "https://orzice.com/favicon.ico",
        description: "跑刀与战备推荐专题，适合赛季初对照参考。",
        builtin: false,
    },
];

/** 默认列表：只读内置站点。 */
export function defaultStrategySites(): ReadonlyArray<StrategySite> {
    return BUILTIN_STRATEGY_SITES;
}

/**
 * 用户新增的站点条目（不含内置站点）。本地存储只保存这一段。
 *
 * `description` 缺省时回落到空字符串；`favicon` 缺省时回落
 * 到 `${origin}/favicon.ico`。
 */
export type UserStrategySite = Omit<StrategySite, "id" | "builtin" | "favicon" | "description"> &
    Partial<Pick<StrategySite, "favicon" | "description">>;

/**
 * 为用户新增的站点生成 ID：基于 crypto-safe 随机串，避免与内置 ID 冲突。
 *
 * 浏览器环境：使用 `crypto.randomUUID()`，剔除 `-` 取前 12 位
 * 测试 / SSR 环境：回落到 `Math.random` + 时间戳，确保非空
 */
export function createUserStrategySiteId(): StrategySiteId {
    if (typeof globalThis !== "undefined" && globalThis.crypto?.randomUUID) {
        const raw = globalThis.crypto.randomUUID().replace(/-/g, "");
        return `user_${raw.slice(0, 12)}` as StrategySiteId;
    }
    const fallback = `${Date.now().toString(36)}${Math.floor(Math.random() * 1e6).toString(36)}`;
    return `user_${fallback.slice(0, 12)}` as StrategySiteId;
}

/**
 * 把 `UserStrategySite` 草稿转换为 `StrategySite`。
 *
 * 规则：
 * - 必填字段：shortLabel / label / url；trim 后非空
 * - 缺省的 `favicon` 回落到 `${origin}/favicon.ico`
 * - 生成新的 `user_xxx` ID 并标记 `builtin: false`
 */
export function createStrategySite(input: UserStrategySite): StrategySite | null {
    const shortLabel = input.shortLabel.trim();
    const label = input.label.trim();
    const url = input.url.trim();
    if (!shortLabel || !label || !url) {
        return null;
    }
    if (!/^https?:\/\//i.test(url)) {
        return null;
    }
    return {
        id: createUserStrategySiteId(),
        shortLabel,
        label,
        url,
        favicon: input.favicon?.trim() || faviconForUrl(url),
        description: (input.description ?? "").trim(),
        builtin: false,
    };
}

/** 根据 URL 推导默认 favicon。 */
function faviconForUrl(url: string): string {
    try {
        const parsed = new URL(url);
        return `${parsed.origin}/favicon.ico`;
    } catch {
        return "";
    }
}

const STORAGE_PREFIX = "delta-auto-tools:strategy:";

function storageKey(suffix: string): string {
    return `${STORAGE_PREFIX}${suffix}`;
}

const SITES_STORAGE_KEY = "user-sites";

function isStrategySiteStorageId(siteId: string): siteId is StrategySiteId {
    return siteId === "kkrb" || siteId === "orzice" || siteId.startsWith("user_") || siteId.startsWith("preset_");
}

function refreshStorageKey(siteId: StrategySiteId): string {
    return storageKey(`${siteId}:refresh-seconds`);
}

export function readStrategyRefreshSeconds(
    siteId: string,
    storage: Pick<Storage, "getItem"> | null = getDefaultStorage(),
): StrategyRefreshSeconds {
    if (storage === null || !isStrategySiteStorageId(siteId)) {
        return DEFAULT_STRATEGY_REFRESH_SECONDS;
    }
    let raw: string | null;
    try {
        raw = storage.getItem(refreshStorageKey(siteId));
    } catch {
        return DEFAULT_STRATEGY_REFRESH_SECONDS;
    }
    if (raw === null || raw.length === 0) {
        return DEFAULT_STRATEGY_REFRESH_SECONDS;
    }
    const parsed = Number(raw);
    return isStrategyRefreshSeconds(parsed) ? parsed : DEFAULT_STRATEGY_REFRESH_SECONDS;
}

export function writeStrategyRefreshSeconds(
    siteId: string,
    seconds: number,
    storage: Pick<Storage, "setItem"> | null = getDefaultStorage(),
): void {
    if (storage === null || !isStrategySiteStorageId(siteId)) {
        return;
    }
    const normalized = isStrategyRefreshSeconds(seconds) ? seconds : DEFAULT_STRATEGY_REFRESH_SECONDS;
    try {
        storage.setItem(refreshStorageKey(siteId), String(normalized));
    } catch {
        // 隐私模式 / 配额限制下会抛错；保持主流程不被破坏。
    }
}

/**
 * 反序列化用户新增的站点列表。损坏 / 解析失败时回落到空数组。
 *
 * 校验：
 * - 必须为对象数组
 * - `id` 必须以 `user_` 开头（防御性：避免有人篡改 localStorage 注入内置站点 ID）
 * - `url` / `shortLabel` / `label` / `description` 必须为字符串
 * - 任何字段缺失或类型不符的条目会被丢弃
 */
export function readStoredUserSites(
    storage: Pick<Storage, "getItem"> | null = getDefaultStorage(),
): StrategySite[] {
    if (storage === null) {
        return [];
    }
    let raw: string | null;
    try {
        raw = storage.getItem(storageKey(SITES_STORAGE_KEY));
    } catch {
        return [];
    }
    if (raw === null || raw.length === 0) {
        return [];
    }
    let parsed: unknown;
    try {
        parsed = JSON.parse(raw);
    } catch {
        return [];
    }
    if (!Array.isArray(parsed)) {
        return [];
    }
    const result: StrategySite[] = [];
    const seen = new Set<string>();
    for (const item of parsed) {
        if (!item || typeof item !== "object") {
            continue;
        }
        const candidate = item as Record<string, unknown>;
        const id = typeof candidate.id === "string" ? candidate.id : null;
        const shortLabel = typeof candidate.shortLabel === "string" ? candidate.shortLabel : null;
        const label = typeof candidate.label === "string" ? candidate.label : null;
        const url = typeof candidate.url === "string" ? candidate.url : null;
        const favicon = typeof candidate.favicon === "string" ? candidate.favicon : null;
        const description = typeof candidate.description === "string" ? candidate.description : null;
        if (!id || !shortLabel || !label || !url) {
            continue;
        }
        if ((!id.startsWith("user_") && id !== "kkrb" && id !== "orzice") || seen.has(id)) {
            continue;
        }
        seen.add(id);
        result.push({
            id: id as StrategySiteId,
            shortLabel,
            label,
            url,
            favicon: favicon || faviconForUrl(url),
            description: description || "",
            builtin: false,
        });
    }
    return result;
}

/**
 * 序列化用户新增的站点列表。损坏数据通过 `try/catch` 静默吞掉。
 */
export function writeStoredUserSites(
    sites: ReadonlyArray<StrategySite>,
    storage: Pick<Storage, "setItem"> | null = getDefaultStorage(),
): void {
    if (storage === null) {
        return;
    }
    try {
        const userOnly = sites
            .filter((site) => !site.builtin)
            .map(({id, shortLabel, label, url, favicon, description}) => ({
                id,
                shortLabel,
                label,
                url,
                favicon,
                description,
            }));
        storage.setItem(storageKey(SITES_STORAGE_KEY), JSON.stringify(userOnly));
    } catch {
        // 隐私模式 / 配额限制下会抛错；保持主流程不被破坏。
    }
}

/**
 * 合并内置站点 + 用户新增站点。
 * 内置站点 builtin=false 时可被用户删除，已删除的不会重新出现。
 */
export function mergeStrategySites(
    builtin: ReadonlyArray<StrategySite>,
    user: ReadonlyArray<StrategySite>,
): ReadonlyArray<StrategySite> {
    // 已存在于用户站点中的不再重复追加
    const userIds = new Set(user.map((site) => site.id));
    const builtinNotInUser = builtin.filter((site) => !userIds.has(site.id));
    return [...builtinNotInUser, ...user];
}

export function normalizeStrategyContentBounds(
    rect: StrategyContentRectLike,
    minimum: { width: number; height: number } = {
        width: STRATEGY_CONTENT_MIN_WIDTH,
        height: STRATEGY_CONTENT_MIN_HEIGHT,
    },
): StrategyContentBounds {
    return {
        x: Math.max(0, Math.round(rect?.left ?? 0)),
        y: Math.max(0, Math.round(rect?.top ?? 0)),
        width: Math.max(minimum.width, Math.round(rect?.width ?? minimum.width)),
        height: Math.max(minimum.height, Math.round(rect?.height ?? minimum.height)),
    };
}

export function normalizeVisibleStrategyContentBounds(
    rect: StrategyContentRectLike,
    viewport: StrategyContentViewportLike,
): StrategyContentBounds | null {
    if (!rect || !viewport) {
        return null;
    }
    const viewportWidth = Math.max(0, Math.round(viewport.width));
    const viewportHeight = Math.max(0, Math.round(viewport.height));
    const left = rect.left;
    const top = rect.top;
    const right = left + rect.width;
    const bottom = top + rect.height;
    const clippedLeft = Math.max(0, left);
    const clippedTop = Math.max(0, top);
    const clippedRight = Math.min(viewportWidth, right);
    const clippedBottom = Math.min(viewportHeight, bottom);
    const width = Math.round(clippedRight - clippedLeft);
    const height = Math.round(clippedBottom - clippedTop);
    if (width <= 0 || height <= 0) {
        return null;
    }
    return {
        x: Math.round(clippedLeft),
        y: Math.round(clippedTop),
        width,
        height,
    };
}

function getDefaultStorage(): Pick<Storage, "getItem" | "setItem"> | null {
    if (typeof window === "undefined" || !window.localStorage) {
        return null;
    }
    return window.localStorage;
}
