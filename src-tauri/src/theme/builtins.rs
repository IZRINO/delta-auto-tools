//! 3 套 daisyUI 内置主题常量。
//!
//! 每套主题完整覆盖 architecture.md §1.2 列出的 26 个 daisyUI token
//! （18 色 + 8 结构/效果），切换主题时由前端写入 `document.documentElement.style`
//! 覆盖 `App.css` 的 `:root` 默认值。
//!
//! 3 套主题：
//! - `olive-amber`：暗橄榄琥珀（暗橄榄绿底 + 琥珀主色）
//! - `valentine`：黑红（深灰底 + 红色主色，**默认主题**）
//! - `arctic-blue`：浅蓝红调（浅蓝底 + 红色主色）
//!
//! 主题值使用迁移规格给出的 daisyUI token 原值，不再派生 base-200/300 或状态色。

use super::types::{ThemeDefinition, ThemeTokenOverride};

pub const OLIVE_AMBER_ID: &str = "olive-amber";
pub const VALENTINE_ID: &str = "valentine";
pub const ARCTIC_BLUE_ID: &str = "arctic-blue";

/// 构造一个 token 覆盖项。仅在本模块内使用，减少重复字面量。
fn t(key: &str, value: &str) -> ThemeTokenOverride {
    ThemeTokenOverride {
        key: key.to_string(),
        value: value.to_string(),
    }
}

/// 暗橄榄琥珀（暗橄榄绿底 + 琥珀主色）。
fn olive_amber() -> Vec<ThemeTokenOverride> {
    vec![
        t("--color-base-100", "oklch(27% 0.072 132.109)"),
        t("--color-base-200", "oklch(40% 0.101 131.063)"),
        t("--color-base-300", "oklch(45% 0.124 130.933)"),
        t("--color-base-content", "oklch(96% 0.067 122.328)"),
        t("--color-primary", "oklch(82% 0.189 84.429)"),
        t("--color-primary-content", "oklch(27% 0.077 45.635)"),
        t("--color-secondary", "oklch(85% 0.199 91.936)"),
        t("--color-secondary-content", "oklch(28% 0.066 53.813)"),
        t("--color-accent", "oklch(82% 0.189 84.429)"),
        t("--color-accent-content", "oklch(27% 0.077 45.635)"),
        t("--color-neutral", "oklch(27% 0.072 132.109)"),
        t("--color-neutral-content", "oklch(98% 0.031 120.757)"),
        t("--color-info", "oklch(54% 0.245 262.881)"),
        t("--color-info-content", "oklch(97% 0.014 254.604)"),
        t("--color-success", "oklch(62% 0.194 149.214)"),
        t("--color-success-content", "oklch(98% 0.018 155.826)"),
        t("--color-warning", "oklch(64% 0.222 41.116)"),
        t("--color-warning-content", "oklch(98% 0.016 73.684)"),
        t("--color-error", "oklch(58% 0.253 17.585)"),
        t("--color-error-content", "oklch(96% 0.015 12.422)"),
        t("--radius-selector", "1rem"),
        t("--radius-field", "0rem"),
        t("--radius-box", "2rem"),
        t("--size-selector", "0.25rem"),
        t("--size-field", "0.25rem"),
        t("--border", "1px"),
        t("--depth", "1"),
        t("--noise", "0"),
    ]
}

/// 黑红（深灰底 + 红色主色，默认主题）。
fn valentine() -> Vec<ThemeTokenOverride> {
    vec![
        t("--color-base-100", "oklch(21.5% 0 261.692)"),
        t("--color-base-200", "oklch(18.8% 0 264.665)"),
        t("--color-base-300", "oklch(42% 0.06 48)"),
        t("--color-base-content", "oklch(96% 0.003 264.542)"),
        t("--color-primary", "oklch(54% 0.21 25)"),
        t("--color-primary-content", "oklch(100% 0 281.288)"),
        t("--color-secondary", "oklch(82% 0.189 84.429)"),
        t("--color-secondary-content", "oklch(27% 0.077 45.635)"),
        t("--color-accent", "oklch(26% 0 0)"),
        t("--color-accent-content", "oklch(100% 0 0)"),
        t("--color-neutral", "oklch(44% 0.017 285.786)"),
        t("--color-neutral-content", "oklch(98% 0.002 247.839)"),
        t("--color-info", "oklch(60% 0.126 221.723)"),
        t("--color-info-content", "oklch(97% 0.014 254.604)"),
        t("--color-success", "oklch(64% 0.2 131.684)"),
        t("--color-success-content", "oklch(98% 0.031 120.757)"),
        t("--color-warning", "oklch(66% 0.179 58.318)"),
        t("--color-warning-content", "oklch(98% 0.022 95.277)"),
        t("--color-error", "oklch(58% 0.253 17.585)"),
        t("--color-error-content", "oklch(97% 0.014 343.198)"),
        t("--radius-selector", "2rem"),
        t("--radius-field", "0.5rem"),
        t("--radius-box", "0.5rem"),
        t("--size-selector", "0.25rem"),
        t("--size-field", "0.25rem"),
        t("--border", "1px"),
        t("--depth", "0"),
        t("--noise", "0"),
    ]
}

/// 浅蓝红调（浅蓝底 + 红色主色）。
fn arctic_blue() -> Vec<ThemeTokenOverride> {
    vec![
        t("--color-base-100", "oklch(97% 0.013 236.62)"),
        t("--color-base-200", "oklch(95% 0.026 236.824)"),
        t("--color-base-300", "oklch(90% 0.058 230.902)"),
        t("--color-base-content", "oklch(39% 0.09 240.876)"),
        t("--color-primary", "oklch(63% 0.237 25.331)"),
        t("--color-primary-content", "oklch(97% 0.013 17.38)"),
        t("--color-secondary", "oklch(79% 0.184 86.047)"),
        t("--color-secondary-content", "oklch(98% 0.026 102.212)"),
        t("--color-accent", "oklch(70% 0.213 47.604)"),
        t("--color-accent-content", "oklch(98% 0.016 73.684)"),
        t("--color-neutral", "oklch(50% 0.134 242.749)"),
        t("--color-neutral-content", "oklch(97% 0.013 236.62)"),
        t("--color-info", "oklch(70% 0.165 254.624)"),
        t("--color-info-content", "oklch(28% 0.091 267.935)"),
        t("--color-success", "oklch(79% 0.209 151.711)"),
        t("--color-success-content", "oklch(26% 0.065 152.934)"),
        t("--color-warning", "oklch(85% 0.199 91.936)"),
        t("--color-warning-content", "oklch(28% 0.066 53.813)"),
        t("--color-error", "oklch(71% 0.202 349.761)"),
        t("--color-error-content", "oklch(28% 0.109 3.907)"),
        t("--radius-selector", "2rem"),
        t("--radius-field", "0rem"),
        t("--radius-box", "0.5rem"),
        t("--size-selector", "0.25rem"),
        t("--size-field", "0.25rem"),
        t("--border", "1px"),
        t("--depth", "0"),
        t("--noise", "0"),
    ]
}

/// 返回全部内置主题定义（3 套）。顺序：olive-amber, valentine, arctic-blue。
pub fn builtin_themes() -> Vec<ThemeDefinition> {
    vec![
        ThemeDefinition {
            id: OLIVE_AMBER_ID.to_string(),
            name: "暗橄榄琥珀".to_string(),
            builtin: true,
            tokens: olive_amber(),
        },
        ThemeDefinition {
            id: VALENTINE_ID.to_string(),
            name: "黑红".to_string(),
            builtin: true,
            tokens: valentine(),
        },
        ThemeDefinition {
            id: ARCTIC_BLUE_ID.to_string(),
            name: "浅蓝红调".to_string(),
            builtin: true,
            tokens: arctic_blue(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// daisyUI token 的完整 key 集合（architecture.md §1.2 列举）。
    /// 共 28 个：20 色（4 base + 8 组 color/content 对）+ 8 结构/效果。
    const EXPECTED_TOKEN_KEYS: &[&str] = &[
        // 18 色
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
        // 8 结构/效果
        "--radius-selector",
        "--radius-field",
        "--radius-box",
        "--size-selector",
        "--size-field",
        "--border",
        "--depth",
        "--noise",
    ];

    #[test]
    fn builtin_themes_count_is_three() {
        let themes = builtin_themes();
        assert_eq!(themes.len(), 3);
        assert!(themes.iter().all(|t| t.builtin));
    }

    #[test]
    fn builtin_theme_ids_are_unique() {
        let themes = builtin_themes();
        let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "内置主题 id 必须唯一");
    }

    #[test]
    fn builtin_theme_ids_match_expected() {
        let themes = builtin_themes();
        let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec![OLIVE_AMBER_ID, VALENTINE_ID, ARCTIC_BLUE_ID]);
    }

    #[test]
    fn valentine_defines_expected_key_colors() {
        // 默认主题的关键色必须与 architecture.md §1.5 一致
        let valentine = builtin_themes()
            .into_iter()
            .find(|t| t.id == VALENTINE_ID)
            .unwrap();
        let find = |key: &str| -> String {
            valentine
                .tokens
                .iter()
                .find(|tok| tok.key == key)
                .unwrap_or_else(|| panic!("缺少 token {key}"))
                .value
                .clone()
        };
        assert_eq!(find("--color-base-100"), "oklch(21.5% 0 261.692)");
        assert_eq!(find("--color-base-200"), "oklch(18.8% 0 264.665)");
        assert_eq!(find("--color-base-300"), "oklch(42% 0.06 48)");
        assert_eq!(find("--color-base-content"), "oklch(96% 0.003 264.542)");
        assert_eq!(find("--color-primary"), "oklch(54% 0.21 25)");
        assert_eq!(find("--color-secondary"), "oklch(82% 0.189 84.429)");
        assert_eq!(find("--noise"), "0");
    }

    #[test]
    fn every_builtin_theme_defines_same_token_keys() {
        // 3 套主题必须定义相同的 key 集合，且恰好覆盖 EXPECTED_TOKEN_KEYS 列举的全部 daisyUI token
        let themes = builtin_themes();
        let baseline: std::collections::HashSet<String> =
            themes[0].tokens.iter().map(|t| t.key.clone()).collect();
        assert_eq!(
            baseline.len(),
            EXPECTED_TOKEN_KEYS.len(),
            "每套主题应恰好定义 {} 个 token，实际 {}",
            EXPECTED_TOKEN_KEYS.len(),
            baseline.len()
        );
        for theme in &themes {
            let keys: std::collections::HashSet<String> =
                theme.tokens.iter().map(|t| t.key.clone()).collect();
            assert_eq!(
                keys, baseline,
                "主题 {} 的 token key 集合与基线不一致",
                theme.id
            );
        }
        // 与 EXPECTED_TOKEN_KEYS 对照
        for key in EXPECTED_TOKEN_KEYS {
            assert!(baseline.contains(*key), "缺少预期 token key: {key}");
        }
    }

    #[test]
    fn every_builtin_theme_token_starts_with_double_dash() {
        // CSS 变量必须以 -- 开头，否则 setProperty 无效
        for theme in builtin_themes() {
            for tok in &theme.tokens {
                assert!(
                    tok.key.starts_with("--"),
                    "主题 {} 的 token key {} 必须以 -- 开头",
                    theme.id,
                    tok.key
                );
            }
        }
    }
}
