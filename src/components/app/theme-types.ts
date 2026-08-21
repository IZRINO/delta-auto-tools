/**
 * 主题引擎前端类型定义。
 *
 * 与 Rust 端 `src-tauri/src/theme/types.rs` 保持结构一致，
 * 所有字段名使用 camelCase（Rust 端 `#[serde(rename_all = "camelCase")]`）。
 */

/** 单个 CSS 变量覆盖项。`key` 必须以 `--` 开头。 */
export interface ThemeTokenOverride {
    key: string;
    value: string;
}

/** 一套完整主题定义。 */
export interface ThemeDefinition {
    /** 主题唯一 id，内置主题用稳定短码，自定义主题用时间戳生成。 */
    id: string;
    /** 主题显示名。 */
    name: string;
    /** 是否内置主题。内置主题不可删除、不可重命名。 */
    builtin: boolean;
    /** 该主题包含的全部 token 覆盖项。 */
    tokens: ThemeTokenOverride[];
}

/** 主题持久化设置，对应后端 `theme_settings.json`。 */
export interface ThemeSettings {
    /** 当前激活主题 id。空串且 overrides 非空时表示自定义配色模式。 */
    activeThemeId: string;
    customThemes: ThemeDefinition[];
    /** 主题 token 覆盖；自定义配色模式下保存完整 token 集。 */
    overrides: ThemeTokenOverride[];
}

/** 主题 bootstrap：一次性返回前端所需的全部主题信息。 */
export interface ThemeBootstrap {
    /** 当前激活主题 id。空串且 overrides 非空时表示自定义配色模式。 */
    activeThemeId: string;
    builtinThemes: ThemeDefinition[];
    customThemes: ThemeDefinition[];
    overrides: ThemeTokenOverride[];
    /** 合并后的最终 token 列表（内置/自定义主题 tokens + overrides），前端直接写入 CSS 变量。 */
    mergedTokens: ThemeTokenOverride[];
}

/** localStorage 持久化 key（浏览器预览模式 fallback 用）。 */
export const THEME_STORAGE_KEY = "delta-auto-tools:theme:v1";

/** 界面世界：战地控制台（现行壳）或黑标。与配色主题正交。 */
export type UiWorld = "console" | "blackmark";

export const UI_WORLD_STORAGE_KEY = "delta-auto-tools:ui-world";

/** 黑标色相。只在黑标世界生效，不进 theme_settings。 */
export type UiScheme = "night" | "day";

export const UI_SCHEME_STORAGE_KEY = "delta-auto-tools:ui-scheme";

/** 3 套内置主题 id 常量，与 Rust `builtins.rs` 保持同步。 */
export const BUILTIN_THEME_IDS = {
    oliveAmber: "olive-amber",
    valentine: "valentine",
    arcticBlue: "arctic-blue",
} as const;

/**
 * 可编辑的 daisyUI 语义 token 白名单（20 个，architecture.md §1.6）。
 *
 * 主题面板的 TOKENS 编辑区只展示这些 key：
 * 4 个 base 相关 + 8 组品牌/状态色的主色+content 配对。
 * Rust 端 builtin 主题仍包含完整 28 个 token 集合（含 radius/size/border/depth/noise），
 * 这里只是前端颜色编辑入口的子集，非颜色 token 不暴露给用户。
 */
export const EDITABLE_TOKEN_KEYS = [
    "--color-base-100",
    "--color-base-200",
    "--color-base-300",
    "--color-base-content",
    "--color-primary",
    "--color-primary-content",
    "--color-secondary",
    "--color-secondary-content",
    "--color-accent",
    "--color-accent-content",
    "--color-neutral",
    "--color-neutral-content",
    "--color-info",
    "--color-info-content",
    "--color-success",
    "--color-success-content",
    "--color-warning",
    "--color-warning-content",
    "--color-error",
    "--color-error-content",
] as const;

/** token 的中文显示名映射，供主题面板展示。 */
export const TOKEN_LABELS: Record<string, string> = {
    "--color-base-100": "基底 / Base-100",
    "--color-base-200": "二级底 / Base-200",
    "--color-base-300": "三级底 / Base-300",
    "--color-base-content": "基底文字 / Base-Content",
    "--color-primary": "主色 / Primary",
    "--color-primary-content": "主色文字 / Primary-Content",
    "--color-secondary": "次色 / Secondary",
    "--color-secondary-content": "次色文字 / Secondary-Content",
    "--color-accent": "强调色 / Accent",
    "--color-accent-content": "强调文字 / Accent-Content",
    "--color-neutral": "中性色 / Neutral",
    "--color-neutral-content": "中性文字 / Neutral-Content",
    "--color-info": "信息色 / Info",
    "--color-info-content": "信息文字 / Info-Content",
    "--color-success": "成功色 / Success",
    "--color-success-content": "成功文字 / Success-Content",
    "--color-warning": "警告色 / Warning",
    "--color-warning-content": "警告文字 / Warning-Content",
    "--color-error": "错误色 / Error",
    "--color-error-content": "错误文字 / Error-Content",
};
