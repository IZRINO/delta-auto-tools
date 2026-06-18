//! 5 套内置主题常量。
//!
//! 每套主题包含完整的语义 token 覆盖（shadcn 基础变量 + 工业语义族 + surface 族），
//! 切换主题时由前端写入 `document.documentElement.style` 覆盖 `App.css` 的 `:root` 默认值。
//!
//! 结构/效果 token（`--radius*`、`--shadow-*`、`--scanline`、`--stripe-warning`、`--misprint-offset`）
//! **不进入主题切换**，保持工业直角硬边风格恒定。
//!
//! 5 套主题：
//! - `industrial-light`：工业亮色（基线，等同 `App.css :root` 当前值）
//! - `industrial-dark`：工业暗色（carbon/chalk 翻转）
//! - `tactical-red`：战术红（alert-red 提升为主色，深灰底）
//! - `phosphor-green`：磷光绿（terminal-green 主色，CRT 黑底）
//! - `paper-amber`：纸面琥珀（暖白纸底 + 深琥珀强调）

use super::types::{ThemeDefinition, ThemeTokenOverride};

pub const INDUSTRIAL_LIGHT_ID: &str = "industrial-light";
pub const INDUSTRIAL_DARK_ID: &str = "industrial-dark";
pub const TACTICAL_RED_ID: &str = "tactical-red";
pub const PHOSPHOR_GREEN_ID: &str = "phosphor-green";
pub const PAPER_AMBER_ID: &str = "paper-amber";

/// 构造一个 token 覆盖项。仅在本模块内使用，减少重复字面量。
fn t(key: &str, value: &str) -> ThemeTokenOverride {
    ThemeTokenOverride {
        key: key.to_string(),
        value: value.to_string(),
    }
}

/// 工业亮色（基线主题，等同 `App.css :root` 当前值）。
fn industrial_light() -> Vec<ThemeTokenOverride> {
    vec![
        // shadcn 基础变量
        t("--background", "#FFFFFF"),
        t("--foreground", "#0A0A0A"),
        t("--card", "#FFFFFF"),
        t("--card-foreground", "#0A0A0A"),
        t("--popover", "#FFFFFF"),
        t("--popover-foreground", "#0A0A0A"),
        t("--primary", "#E8A000"),
        t("--primary-foreground", "#0A0A0A"),
        t("--secondary", "#F5F5F5"),
        t("--secondary-foreground", "#0A0A0A"),
        t("--muted", "#F5F5F5"),
        t("--muted-foreground", "#6B6B6B"),
        t("--accent", "#E8A000"),
        t("--accent-foreground", "#0A0A0A"),
        t("--destructive", "#E11919"),
        t("--border", "#0A0A0A"),
        t("--input", "#0A0A0A"),
        t("--ring", "#E8A000"),
        t("--chart-1", "#E8A000"),
        t("--chart-2", "#3F8A30"),
        t("--chart-3", "#E11919"),
        t("--chart-4", "#6B6B6B"),
        t("--chart-5", "#9A9A9A"),
        t("--sidebar", "#F5F5F5"),
        t("--sidebar-foreground", "#0A0A0A"),
        t("--sidebar-primary", "#E8A000"),
        t("--sidebar-primary-foreground", "#0A0A0A"),
        t("--sidebar-accent", "#FFFFFF"),
        t("--sidebar-accent-foreground", "#0A0A0A"),
        t("--sidebar-border", "#0A0A0A"),
        t("--sidebar-ring", "#E8A000"),
        // 工业语义族
        t("--carbon", "#FFFFFF"),
        t("--slate", "#F5F5F5"),
        t("--iron", "#E5E5E5"),
        t("--chalk", "#0A0A0A"),
        t("--zinc", "#6B6B6B"),
        t("--dust", "#9A9A9A"),
        t("--seam", "#E5E5E5"),
        t("--amber", "#E8A000"),
        t("--rust", "#C85400"),
        t("--moss", "#3F8A30"),
        t("--void", "#2A2A2A"),
        t("--alert-red", "#E11919"),
        t("--warning-amber", "#A36A00"),
        t("--valid-green", "#3F8A30"),
        t("--terminal-green", "#4AF626"),
        t("--phosphor", "#0A0A0A"),
        // surface 表层
        t("--surface-shell", "#FFFFFF"),
        t("--surface-panel", "#F5F5F5"),
        t("--surface-card", "#FFFFFF"),
        t("--surface-card-strong", "#F5F5F5"),
        t("--surface-tile", "#F5F5F5"),
        t("--surface-border", "#0A0A0A"),
        t("--surface-border-strong", "#0A0A0A"),
        t("--surface-hover", "#E5E5E5"),
        t("--surface-highlight", "#E8A000"),
        t("--surface-dot", "#E5E5E5"),
    ]
}

/// 工业暗色（carbon/chalk 翻转，amber 提亮）。
fn industrial_dark() -> Vec<ThemeTokenOverride> {
    vec![
        t("--background", "#0A0A0A"),
        t("--foreground", "#F5F5F5"),
        t("--card", "#141414"),
        t("--card-foreground", "#F5F5F5"),
        t("--popover", "#141414"),
        t("--popover-foreground", "#F5F5F5"),
        t("--primary", "#F0B820"),
        t("--primary-foreground", "#0A0A0A"),
        t("--secondary", "#1F1F1F"),
        t("--secondary-foreground", "#F5F5F5"),
        t("--muted", "#1F1F1F"),
        t("--muted-foreground", "#9A9A9A"),
        t("--accent", "#F0B820"),
        t("--accent-foreground", "#0A0A0A"),
        t("--destructive", "#FF3B3B"),
        t("--border", "#2A2A2A"),
        t("--input", "#2A2A2A"),
        t("--ring", "#F0B820"),
        t("--chart-1", "#F0B820"),
        t("--chart-2", "#5BC24A"),
        t("--chart-3", "#FF3B3B"),
        t("--chart-4", "#9A9A9A"),
        t("--chart-5", "#6B6B6B"),
        t("--sidebar", "#141414"),
        t("--sidebar-foreground", "#F5F5F5"),
        t("--sidebar-primary", "#F0B820"),
        t("--sidebar-primary-foreground", "#0A0A0A"),
        t("--sidebar-accent", "#1F1F1F"),
        t("--sidebar-accent-foreground", "#F5F5F5"),
        t("--sidebar-border", "#2A2A2A"),
        t("--sidebar-ring", "#F0B820"),
        t("--carbon", "#0A0A0A"),
        t("--slate", "#141414"),
        t("--iron", "#1F1F1F"),
        t("--chalk", "#F5F5F5"),
        t("--zinc", "#9A9A9A"),
        t("--dust", "#6B6B6B"),
        t("--seam", "#2A2A2A"),
        t("--amber", "#F0B820"),
        t("--rust", "#D86A20"),
        t("--moss", "#5BC24A"),
        t("--void", "#1F1F1F"),
        t("--alert-red", "#FF3B3B"),
        t("--warning-amber", "#D49500"),
        t("--valid-green", "#5BC24A"),
        t("--terminal-green", "#4AF626"),
        t("--phosphor", "#F5F5F5"),
        t("--surface-shell", "#0A0A0A"),
        t("--surface-panel", "#141414"),
        t("--surface-card", "#141414"),
        t("--surface-card-strong", "#1F1F1F"),
        t("--surface-tile", "#1F1F1F"),
        t("--surface-border", "#F5F5F5"),
        t("--surface-border-strong", "#F5F5F5"),
        t("--surface-hover", "#2A2A2A"),
        t("--surface-highlight", "#F0B820"),
        t("--surface-dot", "#2A2A2A"),
    ]
}

/// 战术红（深灰底，alert-red 提升为主色）。
fn tactical_red() -> Vec<ThemeTokenOverride> {
    vec![
        t("--background", "#1A1A1A"),
        t("--foreground", "#F0F0F0"),
        t("--card", "#242424"),
        t("--card-foreground", "#F0F0F0"),
        t("--popover", "#242424"),
        t("--popover-foreground", "#F0F0F0"),
        t("--primary", "#E11919"),
        t("--primary-foreground", "#FFFFFF"),
        t("--secondary", "#2E2E2E"),
        t("--secondary-foreground", "#F0F0F0"),
        t("--muted", "#2E2E2E"),
        t("--muted-foreground", "#9A9A9A"),
        t("--accent", "#E11919"),
        t("--accent-foreground", "#FFFFFF"),
        t("--destructive", "#FF3B3B"),
        t("--border", "#3A3A3A"),
        t("--input", "#3A3A3A"),
        t("--ring", "#E11919"),
        t("--chart-1", "#E11919"),
        t("--chart-2", "#C85400"),
        t("--chart-3", "#FF3B3B"),
        t("--chart-4", "#9A9A9A"),
        t("--chart-5", "#6B6B6B"),
        t("--sidebar", "#1A1A1A"),
        t("--sidebar-foreground", "#F0F0F0"),
        t("--sidebar-primary", "#E11919"),
        t("--sidebar-primary-foreground", "#FFFFFF"),
        t("--sidebar-accent", "#2E2E2E"),
        t("--sidebar-accent-foreground", "#F0F0F0"),
        t("--sidebar-border", "#3A3A3A"),
        t("--sidebar-ring", "#E11919"),
        t("--carbon", "#1A1A1A"),
        t("--slate", "#242424"),
        t("--iron", "#2E2E2E"),
        t("--chalk", "#F0F0F0"),
        t("--zinc", "#9A9A9A"),
        t("--dust", "#6B6B6B"),
        t("--seam", "#3A3A3A"),
        t("--amber", "#E11919"),
        t("--rust", "#C85400"),
        t("--moss", "#3F8A30"),
        t("--void", "#2E2E2E"),
        t("--alert-red", "#FF3B3B"),
        t("--warning-amber", "#D49500"),
        t("--valid-green", "#3F8A30"),
        t("--terminal-green", "#4AF626"),
        t("--phosphor", "#F0F0F0"),
        t("--surface-shell", "#1A1A1A"),
        t("--surface-panel", "#242424"),
        t("--surface-card", "#242424"),
        t("--surface-card-strong", "#2E2E2E"),
        t("--surface-tile", "#2E2E2E"),
        t("--surface-border", "#F0F0F0"),
        t("--surface-border-strong", "#F0F0F0"),
        t("--surface-hover", "#3A3A3A"),
        t("--surface-highlight", "#E11919"),
        t("--surface-dot", "#3A3A3A"),
    ]
}

/// 磷光绿（CRT 黑底，terminal-green 主色）。
fn phosphor_green() -> Vec<ThemeTokenOverride> {
    vec![
        t("--background", "#000000"),
        t("--foreground", "#4AF626"),
        t("--card", "#0A1A0A"),
        t("--card-foreground", "#4AF626"),
        t("--popover", "#0A1A0A"),
        t("--popover-foreground", "#4AF626"),
        t("--primary", "#4AF626"),
        t("--primary-foreground", "#000000"),
        t("--secondary", "#0A1A0A"),
        t("--secondary-foreground", "#4AF626"),
        t("--muted", "#0A1A0A"),
        t("--muted-foreground", "#2A8A1A"),
        t("--accent", "#4AF626"),
        t("--accent-foreground", "#000000"),
        t("--destructive", "#FF3B3B"),
        t("--border", "#2A8A1A"),
        t("--input", "#2A8A1A"),
        t("--ring", "#4AF626"),
        t("--chart-1", "#4AF626"),
        t("--chart-2", "#2A8A1A"),
        t("--chart-3", "#FF3B3B"),
        t("--chart-4", "#2A8A1A"),
        t("--chart-5", "#1A5A0A"),
        t("--sidebar", "#000000"),
        t("--sidebar-foreground", "#4AF626"),
        t("--sidebar-primary", "#4AF626"),
        t("--sidebar-primary-foreground", "#000000"),
        t("--sidebar-accent", "#0A1A0A"),
        t("--sidebar-accent-foreground", "#4AF626"),
        t("--sidebar-border", "#2A8A1A"),
        t("--sidebar-ring", "#4AF626"),
        t("--carbon", "#000000"),
        t("--slate", "#0A1A0A"),
        t("--iron", "#1A2A1A"),
        t("--chalk", "#4AF626"),
        t("--zinc", "#2A8A1A"),
        t("--dust", "#1A5A0A"),
        t("--seam", "#2A8A1A"),
        t("--amber", "#4AF626"),
        t("--rust", "#C85400"),
        t("--moss", "#4AF626"),
        t("--void", "#0A1A0A"),
        t("--alert-red", "#FF3B3B"),
        t("--warning-amber", "#D49500"),
        t("--valid-green", "#4AF626"),
        t("--terminal-green", "#4AF626"),
        t("--phosphor", "#4AF626"),
        t("--surface-shell", "#000000"),
        t("--surface-panel", "#0A1A0A"),
        t("--surface-card", "#0A1A0A"),
        t("--surface-card-strong", "#1A2A1A"),
        t("--surface-tile", "#1A2A1A"),
        t("--surface-border", "#2A8A1A"),
        t("--surface-border-strong", "#4AF626"),
        t("--surface-hover", "#1A2A1A"),
        t("--surface-highlight", "#4AF626"),
        t("--surface-dot", "#2A8A1A"),
    ]
}

/// 纸面琥珀（暖白纸底 + 深琥珀强调，柔光）。
fn paper_amber() -> Vec<ThemeTokenOverride> {
    vec![
        t("--background", "#F5EFE0"),
        t("--foreground", "#3A2A0A"),
        t("--card", "#FBF6E8"),
        t("--card-foreground", "#3A2A0A"),
        t("--popover", "#FBF6E8"),
        t("--popover-foreground", "#3A2A0A"),
        t("--primary", "#B87000"),
        t("--primary-foreground", "#FBF6E8"),
        t("--secondary", "#EDE3CC"),
        t("--secondary-foreground", "#3A2A0A"),
        t("--muted", "#EDE3CC"),
        t("--muted-foreground", "#8A7A5A"),
        t("--accent", "#B87000"),
        t("--accent-foreground", "#FBF6E8"),
        t("--destructive", "#B53030"),
        t("--border", "#3A2A0A"),
        t("--input", "#3A2A0A"),
        t("--ring", "#B87000"),
        t("--chart-1", "#B87000"),
        t("--chart-2", "#5A7A2A"),
        t("--chart-3", "#B53030"),
        t("--chart-4", "#8A7A5A"),
        t("--chart-5", "#BAA88A"),
        t("--sidebar", "#EDE3CC"),
        t("--sidebar-foreground", "#3A2A0A"),
        t("--sidebar-primary", "#B87000"),
        t("--sidebar-primary-foreground", "#FBF6E8"),
        t("--sidebar-accent", "#FBF6E8"),
        t("--sidebar-accent-foreground", "#3A2A0A"),
        t("--sidebar-border", "#3A2A0A"),
        t("--sidebar-ring", "#B87000"),
        t("--carbon", "#F5EFE0"),
        t("--slate", "#EDE3CC"),
        t("--iron", "#E0D4B8"),
        t("--chalk", "#3A2A0A"),
        t("--zinc", "#8A7A5A"),
        t("--dust", "#BAA88A"),
        t("--seam", "#D4C5A0"),
        t("--amber", "#B87000"),
        t("--rust", "#A05000"),
        t("--moss", "#5A7A2A"),
        t("--void", "#5A4A2A"),
        t("--alert-red", "#B53030"),
        t("--warning-amber", "#8A5A00"),
        t("--valid-green", "#5A7A2A"),
        t("--terminal-green", "#4AF626"),
        t("--phosphor", "#3A2A0A"),
        t("--surface-shell", "#F5EFE0"),
        t("--surface-panel", "#EDE3CC"),
        t("--surface-card", "#FBF6E8"),
        t("--surface-card-strong", "#EDE3CC"),
        t("--surface-tile", "#EDE3CC"),
        t("--surface-border", "#3A2A0A"),
        t("--surface-border-strong", "#3A2A0A"),
        t("--surface-hover", "#E0D4B8"),
        t("--surface-highlight", "#B87000"),
        t("--surface-dot", "#D4C5A0"),
    ]
}

/// 返回全部内置主题定义（5 套）。
pub fn builtin_themes() -> Vec<ThemeDefinition> {
    vec![
        ThemeDefinition {
            id: INDUSTRIAL_LIGHT_ID.to_string(),
            name: "工业亮色".to_string(),
            builtin: true,
            tokens: industrial_light(),
        },
        ThemeDefinition {
            id: INDUSTRIAL_DARK_ID.to_string(),
            name: "工业暗色".to_string(),
            builtin: true,
            tokens: industrial_dark(),
        },
        ThemeDefinition {
            id: TACTICAL_RED_ID.to_string(),
            name: "战术红".to_string(),
            builtin: true,
            tokens: tactical_red(),
        },
        ThemeDefinition {
            id: PHOSPHOR_GREEN_ID.to_string(),
            name: "磷光绿".to_string(),
            builtin: true,
            tokens: phosphor_green(),
        },
        ThemeDefinition {
            id: PAPER_AMBER_ID.to_string(),
            name: "纸面琥珀".to_string(),
            builtin: true,
            tokens: paper_amber(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_themes_count_is_five() {
        let themes = builtin_themes();
        assert_eq!(themes.len(), 5);
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
    fn industrial_light_matches_app_css_root() {
        // 基线主题的几个关键 token 必须与 App.css :root 当前值一致
        let light = builtin_themes()
            .into_iter()
            .find(|t| t.id == INDUSTRIAL_LIGHT_ID)
            .unwrap();
        let find = |key: &str| -> String {
            light
                .tokens
                .iter()
                .find(|tok| tok.key == key)
                .unwrap_or_else(|| panic!("缺少 token {key}"))
                .value
                .clone()
        };
        assert_eq!(find("--carbon"), "#FFFFFF");
        assert_eq!(find("--chalk"), "#0A0A0A");
        assert_eq!(find("--amber"), "#E8A000");
        assert_eq!(find("--alert-red"), "#E11919");
        assert_eq!(find("--moss"), "#3F8A30");
    }

    #[test]
    fn every_builtin_theme_defines_same_token_keys() {
        // 5 套主题必须定义相同的 key 集合，否则切换主题时会出现 token 残留
        let themes = builtin_themes();
        let baseline: std::collections::HashSet<String> = themes[0]
            .tokens
            .iter()
            .map(|t| t.key.clone())
            .collect();
        for theme in &themes {
            let keys: std::collections::HashSet<String> =
                theme.tokens.iter().map(|t| t.key.clone()).collect();
            assert_eq!(
                keys, baseline,
                "主题 {} 的 token key 集合与基线不一致",
                theme.id
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
