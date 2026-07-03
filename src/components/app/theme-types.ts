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

/** 3 套内置主题 id 常量，与 Rust `builtins.rs` 保持同步。 */
export const BUILTIN_THEME_IDS = {
    oliveAmber: "olive-amber",
    valentine: "valentine",
    arcticBlue: "arctic-blue",
} as const;

/**
 * 可编辑的语义 token 白名单。
 *
 * 主题面板的 TOKENS 编辑区只展示这些 key（避免把 chart/sidebar 等派生变量
 * 暴露给用户，降低认知负担）。Rust 端 builtin 主题仍包含完整 token 集合，
 * 这里只是前端编辑入口的子集。
 */
export const EDITABLE_TOKEN_KEYS = [
    "--carbon",
    "--slate",
    "--iron",
    "--chalk",
    "--zinc",
    "--dust",
    "--seam",
    "--amber",
    "--rust",
    "--moss",
    "--void",
    "--alert-red",
    "--warning-amber",
    "--valid-green",
    "--terminal-green",
    "--phosphor",
] as const;

/** token 的中文显示名映射，供主题面板展示。 */
export const TOKEN_LABELS: Record<string, string> = {
    "--carbon": "基底 / Carbon",
    "--slate": "二级底 / Slate",
    "--iron": "分隔灰 / Iron",
    "--chalk": "主前景 / Chalk",
    "--zinc": "次文字 / Zinc",
    "--dust": "弱文字 / Dust",
    "--seam": "缝合线 / Seam",
    "--amber": "强调主色 / Amber",
    "--rust": "警告橙 / Rust",
    "--moss": "成功绿 / Moss",
    "--void": "深空灰 / Void",
    "--alert-red": "告警红 / Alert Red",
    "--warning-amber": "警示琥珀 / Warning Amber",
    "--valid-green": "有效绿 / Valid Green",
    "--terminal-green": "终端绿 / Terminal Green",
    "--phosphor": "磷光 / Phosphor",
};
