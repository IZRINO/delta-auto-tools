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
//! 关键色（base-100 / primary / secondary）取自 architecture.md §1.5；
//! base-200/300 与 base-content 按 daisyUI 惯例从 base-100 派生（明度递减 / 反相）；
//! info/success/warning/error 取 daisyUI 内置主题通用值（light/dark 一致）；
//! 结构/效果 token 取 daisyUI 通用默认值（圆角 1rem/0.25rem、边框 1px、depth/noise 关闭）。

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

/// 追加 daisyUI 通用状态色（info/success/warning/error 及其 content）。
/// 这些值在 daisyUI light/dark 等内置主题中保持一致，跨主题无需差异化。
fn push_status_colors(v: &mut Vec<ThemeTokenOverride>) {
    v.push(t("--color-info", "oklch(74% 0.16 232.661)"));
    v.push(t("--color-info-content", "oklch(29% 0.066 243.157)"));
    v.push(t("--color-success", "oklch(76% 0.177 163.223)"));
    v.push(t("--color-success-content", "oklch(37% 0.077 168.94)"));
    v.push(t("--color-warning", "oklch(82% 0.189 84.429)"));
    v.push(t("--color-warning-content", "oklch(41% 0.112 45.904)"));
    v.push(t("--color-error", "oklch(71% 0.194 13.428)"));
    v.push(t("--color-error-content", "oklch(27% 0.105 12.094)"));
}

/// 追加 daisyUI 通用结构/效果 token（圆角/尺寸/边框宽度/depth/noise）。
fn push_structural_tokens(v: &mut Vec<ThemeTokenOverride>) {
    v.push(t("--radius-selector", "1rem"));
    v.push(t("--radius-field", "0.25rem"));
    v.push(t("--radius-box", "1rem"));
    v.push(t("--size-selector", "0.25rem"));
    v.push(t("--size-field", "0.25rem"));
    v.push(t("--border", "1px"));
    v.push(t("--depth", "0"));
    v.push(t("--noise", "0"));
}

/// 暗橄榄琥珀（暗橄榄绿底 + 琥珀主色）。
fn olive_amber() -> Vec<ThemeTokenOverride> {
    let mut v = vec![
        // base 系：暗橄榄绿，明度递减
        t("--color-base-100", "oklch(27% 0.072 132.109)"),
        t("--color-base-200", "oklch(24% 0.072 132.109)"),
        t("--color-base-300", "oklch(21% 0.072 132.109)"),
        t("--color-base-content", "oklch(89% 0.05 132.109)"),
        // 品牌/中性色
        t("--color-primary", "oklch(82% 0.189 84.429)"),
        t("--color-primary-content", "oklch(27% 0.072 132.109)"),
        t("--color-secondary", "oklch(85% 0.199 91.936)"),
        t("--color-secondary-content", "oklch(27% 0.072 132.109)"),
        t("--color-accent", "oklch(82% 0.189 84.429)"),
        t("--color-accent-content", "oklch(27% 0.072 132.109)"),
        t("--color-neutral", "oklch(27% 0.03 132.109)"),
        t("--color-neutral-content", "oklch(89% 0.05 132.109)"),
    ];
    push_status_colors(&mut v);
    push_structural_tokens(&mut v);
    v
}

/// 黑红（深灰底 + 红色主色，默认主题）。
fn valentine() -> Vec<ThemeTokenOverride> {
    let mut v = vec![
        // base 系：近黑蓝灰，明度递减
        t("--color-base-100", "oklch(21.5% 0 261.692)"),
        t("--color-base-200", "oklch(18.5% 0 261.692)"),
        t("--color-base-300", "oklch(15.5% 0 261.692)"),
        t("--color-base-content", "oklch(89% 0.02 261.692)"),
        // 品牌/中性色
        t("--color-primary", "oklch(70% 0.234 24.700)"),
        t("--color-primary-content", "oklch(98% 0 0)"),
        t("--color-secondary", "oklch(82% 0.189 84.429)"),
        t("--color-secondary-content", "oklch(21.5% 0 261.692)"),
        t("--color-accent", "oklch(70% 0.234 24.700)"),
        t("--color-accent-content", "oklch(98% 0 0)"),
        t("--color-neutral", "oklch(27% 0.03 261.692)"),
        t("--color-neutral-content", "oklch(89% 0.02 261.692)"),
    ];
    push_status_colors(&mut v);
    push_structural_tokens(&mut v);
    v
}

/// 浅蓝红调（浅蓝底 + 红色主色）。
fn arctic_blue() -> Vec<ThemeTokenOverride> {
    let mut v = vec![
        // base 系：浅蓝白，明度递减
        t("--color-base-100", "oklch(97% 0.013 236.62)"),
        t("--color-base-200", "oklch(94% 0.013 236.62)"),
        t("--color-base-300", "oklch(91% 0.013 236.62)"),
        t("--color-base-content", "oklch(43% 0.05 236.62)"),
        // 品牌/中性色
        t("--color-primary", "oklch(63% 0.237 25.331)"),
        t("--color-primary-content", "oklch(98% 0 0)"),
        t("--color-secondary", "oklch(79% 0.184 86.047)"),
        t("--color-secondary-content", "oklch(43% 0.05 236.62)"),
        t("--color-accent", "oklch(63% 0.237 25.331)"),
        t("--color-accent-content", "oklch(98% 0 0)"),
        t("--color-neutral", "oklch(43% 0.05 236.62)"),
        t("--color-neutral-content", "oklch(89% 0.05 236.62)"),
    ];
    push_status_colors(&mut v);
    push_structural_tokens(&mut v);
    v
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
        assert_eq!(find("--color-primary"), "oklch(70% 0.234 24.700)");
        assert_eq!(find("--color-secondary"), "oklch(82% 0.189 84.429)");
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
            assert!(
                baseline.contains(*key),
                "缺少预期 token key: {key}"
            );
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
