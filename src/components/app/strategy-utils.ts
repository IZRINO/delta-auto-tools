/**
 * 攻略网站（strategy）模块的纯逻辑工具函数。
 *
 * 该模块只承载：站点常量、自动刷新间隔档位、序列化与本地存储读写。
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
  /** 默认 iframe 入口 URL */
  url: string;
  /** 通过系统浏览器打开的 URL（与 url 一致；保留扩展点） */
  externalUrl: string;
  /** 站点 favicon；缺省时回退到默认 favicon */
  favicon: string;
  /** 站点简介 */
  description: string;
  /** 是否为内置站点（不允许删除） */
  builtin: boolean;
};

/**
 * Tauri 端 `strategy_fetch_page` 命令的响应。
 */
export type StrategyFetchResponse = {
  status: number;
  finalUrl: string;
  contentType: string;
  html: string;
  byteLength: number;
  /** 命中客户端人机验证时由 Rust 端填充，前端应引导用户改用应用内打开。 */
  challenge?: StrategyChallenge;
};

/**
 * 代理层嗅探到的人机验证挑战。
 */
export type StrategyChallenge = {
  /** 挑战类型，固定为 `ccCheck`（kkrb cdn-shield 风格）。 */
  kind: "ccCheck";
  /** 提示用户的中文文案。 */
  message: string;
};

/**
 * Tauri 端 `strategy_open_in_view` 命令的请求 / 响应。
 */
export type StrategyOpenInViewRequest = {
  url: string;
  title?: string;
  label?: string;
};

export type StrategyOpenInViewResponse = {
  label: string;
  reused: boolean;
};

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
    externalUrl: "https://www.kkrb.net/?viewpage=view%2Foverview",
    favicon: "https://www.kkrb.net/favicon.ico",
    description: "覆盖地图任务、藏宝、跑刀路线的高频更新攻略总览。",
    builtin: true,
  },
  {
    id: "orzice",
    shortLabel: "Orzice",
    label: "Orzice RB 攻略",
    url: "https://orzice.com/v/rb",
    externalUrl: "https://orzice.com/v/rb",
    favicon: "https://orzice.com/favicon.ico",
    description: "跑刀与战备推荐专题，适合赛季初对照参考。",
    builtin: true,
  },
];

/** 默认列表：先内置再用户新增。 */
export function defaultStrategySites(): ReadonlyArray<StrategySite> {
  return BUILTIN_STRATEGY_SITES;
}

/**
 * 用户新增的站点条目（不含内置站点）。本地存储只保存这一段。
 *
 * `description` 缺省时回落到空字符串；`externalUrl` / `favicon` 缺省时分别回落
 * 到 `url` 自身与 `${origin}/favicon.ico`。
 */
export type UserStrategySite = Omit<StrategySite, "id" | "builtin" | "externalUrl" | "favicon" | "description"> &
  Partial<Pick<StrategySite, "externalUrl" | "favicon" | "description">>;

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
 * - 缺省的 `externalUrl` / `favicon` 回落到 `url` 自身
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
    externalUrl: input.externalUrl?.trim() || url,
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

/**
 * 自动刷新间隔档位（秒）。
 */
export const STRATEGY_REFRESH_INTERVAL_SECONDS = [30, 60, 120, 300, 600] as const;
export type StrategyRefreshInterval = (typeof STRATEGY_REFRESH_INTERVAL_SECONDS)[number] | null;

/**
 * 将 `seconds` 归一化到合法档位上。
 */
export function normalizeStrategyRefreshSeconds(
  value: number | null | undefined,
): StrategyRefreshInterval {
  if (value === null || value === undefined) {
    return null;
  }
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) {
    return null;
  }
  const rounded = Math.round(value);
  for (const candidate of STRATEGY_REFRESH_INTERVAL_SECONDS) {
    if (rounded <= candidate) {
      return candidate;
    }
  }
  return STRATEGY_REFRESH_INTERVAL_SECONDS[STRATEGY_REFRESH_INTERVAL_SECONDS.length - 1];
}

/**
 * UI 上展示的"自动刷新"档位文案。
 */
export function formatStrategyRefreshLabel(seconds: StrategyRefreshInterval): string {
  if (seconds === null) {
    return "关闭";
  }
  if (seconds < 60) {
    return `${seconds} 秒`;
  }
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) {
    return `${minutes} 分钟`;
  }
  const hours = Math.round(minutes / 60);
  return `${hours} 小时`;
}

/**
 * 默认自动刷新档位：5 分钟。
 */
export const DEFAULT_STRATEGY_REFRESH_SECONDS: StrategyRefreshInterval = 300;

const STORAGE_PREFIX = "delta-auto-tools:strategy:";

function storageKey(suffix: string): string {
  return `${STORAGE_PREFIX}${suffix}`;
}

const SITES_STORAGE_KEY = "user-sites";

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
    const externalUrl = typeof candidate.externalUrl === "string" ? candidate.externalUrl : null;
    const favicon = typeof candidate.favicon === "string" ? candidate.favicon : null;
    const description = typeof candidate.description === "string" ? candidate.description : null;
    if (!id || !shortLabel || !label || !url) {
      continue;
    }
    if (!id.startsWith("user_") || seen.has(id)) {
      continue;
    }
    seen.add(id);
    result.push({
      id: id as StrategySiteId,
      shortLabel,
      label,
      url,
      externalUrl: externalUrl || url,
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
      .map(({ id, shortLabel, label, url, externalUrl, favicon, description }) => ({
        id,
        shortLabel,
        label,
        url,
        externalUrl,
        favicon,
        description,
      }));
    storage.setItem(storageKey(SITES_STORAGE_KEY), JSON.stringify(userOnly));
  } catch {
    // 隐私模式 / 配额限制下会抛错；保持主流程不被破坏。
  }
}

/**
 * 合并内置站点 + 用户新增站点；用户站点始终追加在末尾。
 */
export function mergeStrategySites(
  builtin: ReadonlyArray<StrategySite>,
  user: ReadonlyArray<StrategySite>,
): ReadonlyArray<StrategySite> {
  return [...builtin, ...user];
}

/**
 * 从给定的 storage 后端反序列化自动刷新档位。
 */
export function readStoredRefreshSeconds(
  key: string,
  storage: Pick<Storage, "getItem"> | null = getDefaultStorage(),
): StrategyRefreshInterval {
  if (storage === null) {
    return DEFAULT_STRATEGY_REFRESH_SECONDS;
  }
  let raw: string | null;
  try {
    raw = storage.getItem(storageKey(key));
  } catch {
    return DEFAULT_STRATEGY_REFRESH_SECONDS;
  }
  if (raw === null) {
    return DEFAULT_STRATEGY_REFRESH_SECONDS;
  }
  if (raw === "off") {
    return null;
  }
  const parsed = Number.parseInt(raw, 10);
  return normalizeStrategyRefreshSeconds(parsed);
}

/**
 * 将自动刷新档位写入给定的 storage 后端。
 */
export function writeStoredRefreshSeconds(
  key: string,
  value: StrategyRefreshInterval,
  storage: Pick<Storage, "setItem"> | null = getDefaultStorage(),
): void {
  if (storage === null) {
    return;
  }
  try {
    const payload = value === null ? "off" : String(value);
    storage.setItem(storageKey(key), payload);
  } catch {
    // 隐私模式 / 配额限制下会抛错；保持主流程不被破坏。
  }
}

function getDefaultStorage(): Pick<Storage, "getItem" | "setItem"> | null {
  if (typeof window === "undefined" || !window.localStorage) {
    return null;
  }
  return window.localStorage;
}

/**
 * 单卡片的下次刷新倒计时（毫秒）。
 */
export function nextRefreshDelayMs(seconds: StrategyRefreshInterval): number | null {
  if (seconds === null) {
    return null;
  }
  return seconds * 1000;
}

/**
 * 给 HTML 注入 `<base href>` 并返回适合 `iframe.srcDoc` 的字符串。
 */
export function injectBaseHrefIntoHtml(html: string, baseUrl: string): string {
  const safeBase = escapeHtmlAttribute(baseUrl);
  const baseTag = `<base href="${safeBase}">`;
  const headMatch = html.match(/<head[^>]*>/i);
  if (headMatch && typeof headMatch.index === "number") {
    const insertAt = headMatch.index + headMatch[0].length;
    return `${html.slice(0, insertAt)}${baseTag}${html.slice(insertAt)}`;
  }
  const htmlMatch = html.match(/<html[^>]*>/i);
  if (htmlMatch && typeof htmlMatch.index === "number") {
    const insertAt = htmlMatch.index + htmlMatch[0].length;
    return `${html.slice(0, insertAt)}<head>${baseTag}</head>${html.slice(insertAt)}`;
  }
  return `${baseTag}${html}`;
}

function escapeHtmlAttribute(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
