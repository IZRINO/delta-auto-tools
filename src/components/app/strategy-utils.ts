/**
 * 攻略网站（strategy）模块的纯逻辑工具函数。
 *
 * 该模块只承载：站点常量、自动刷新间隔档位、序列化与本地存储读写。
 * 不包含 React 组件、副作用与 Tauri 命令调用，方便被单元测试覆盖。
 */

export type StrategySiteId = "kkrb" | "orzice";

/**
 * Tauri 端 `strategy_fetch_page` 命令的响应。
 *
 * - `status`：HTTP 状态码
 * - `finalUrl`：跳转后的最终 URL
 * - `contentType`：原始响应 Content-Type
 * - `html`：HTML 文本（UTF-8 解码后的）
 * - `byteLength`：原始响应字节数
 */
export type StrategyFetchResponse = {
  status: number;
  finalUrl: string;
  contentType: string;
  html: string;
  byteLength: number;
};
export type StrategySite = {
  /** 站点内部 ID，用于本地存储、aria-label 等 */
  id: StrategySiteId;
  /** UI 上展示的简称，2-4 个汉字 */
  shortLabel: string;
  /** UI 上展示的完整中文标签 */
  label: string;
  /** 默认 iframe 入口 URL */
  url: string;
  /** 通过系统浏览器打开的 URL（与 url 一致；保留扩展点） */
  externalUrl: string;
  /** 站点 favicon，回退到 RSSHub-like 默认 favicon */
  favicon: string;
  /** 站点简介 */
  description: string;
};

/**
 * 内置两个攻略站点。
 *
 * - kkrb → https://www.kkrb.net/?viewpage=view%2Foverview
 * - orzice → https://orzice.com/v/rb
 *
 * 添加新站点时直接 push 一个新对象即可，UI 会按数组顺序逐张卡片渲染。
 */
export const STRATEGY_SITES: ReadonlyArray<StrategySite> = [
  {
    id: "kkrb",
    shortLabel: "KK 日报",
    label: "KK 日报攻略总览",
    url: "https://www.kkrb.net/?viewpage=view%2Foverview",
    externalUrl: "https://www.kkrb.net/?viewpage=view%2Foverview",
    favicon: "https://www.kkrb.net/favicon.ico",
    description: "覆盖地图任务、藏宝、跑刀路线的高频更新攻略总览。",
  },
  {
    id: "orzice",
    shortLabel: "Orzice",
    label: "Orzice RB 攻略",
    url: "https://orzice.com/v/rb",
    externalUrl: "https://orzice.com/v/rb",
    favicon: "https://orzice.com/favicon.ico",
    description: "跑刀与战备推荐专题，适合赛季初对照参考。",
  },
];

/**
 * 自动刷新间隔档位（秒）。
 *
 * `null` 表示关闭自动刷新；其余档位按真实场景挑选：覆盖短时滚动监控到低频巡检。
 */
export const STRATEGY_REFRESH_INTERVAL_SECONDS = [30, 60, 120, 300, 600] as const;
export type StrategyRefreshInterval = (typeof STRATEGY_REFRESH_INTERVAL_SECONDS)[number] | null;

/**
 * 将 `seconds` 归一化到合法档位上。
 *
 * 规则：
 * - `null` / `undefined` / 非数字 → `null`（关闭）
 * - `<= 0` → `null`（关闭）
 * - 命中已知档位 → 原样返回
 * - 其他正数 → 向上取最近档位；超过 600 仍封顶 600
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

/**
 * 从给定的 storage 后端反序列化自动刷新档位。
 * 优先使用 `storage` 参数；未提供且当前为浏览器环境时，回落到 `window.localStorage`。
 * 解析失败、类型不符、key 缺失时统一回落到 `DEFAULT_STRATEGY_REFRESH_SECONDS`。
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
 * `null` 序列化为字面量 `"off"`，数值原样写回。
 * 任何 IO 异常都静默吞掉，避免破坏主流程（隐私模式、磁盘满）。
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
 * `null` 表示未启用自动刷新。
 */
export function nextRefreshDelayMs(seconds: StrategyRefreshInterval): number | null {
  if (seconds === null) {
    return null;
  }
  return seconds * 1000;
}

/**
 * 给 HTML 注入 `<base href>` 并返回适合 `iframe.srcDoc` 的字符串。
 *
 * 把站点资源（CSS / JS / 图片）的相对路径解析为绝对 URL，避免 `srcDoc` 上下文（`about:srcdoc`）
 * 找不到 `/assets/...`、`/static/...` 这类相对资源。
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
