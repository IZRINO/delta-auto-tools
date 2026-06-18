//! 主题 token 合并纯函数。
//!
//! 把激活主题的 tokens 与临时 overrides 合并（overrides 优先），
//! 返回最终要写入 CSS 变量的 token 列表。可脱离 Tauri 在 `cargo test` 中单测。

use std::collections::HashMap;

use super::types::{ThemeDefinition, ThemeTokenOverride};

/// 合并主题 tokens 与 overrides。
///
/// 语义：
/// - 以 `theme.tokens` 为基底；
/// - `overrides` 中同 key 的项覆盖基底值；
/// - `overrides` 中独有的 key 追加到结果末尾；
/// - 结果保留基底顺序，追加项放在最后。
pub fn merge_theme_tokens(
    theme: &ThemeDefinition,
    overrides: &[ThemeTokenOverride],
) -> Vec<ThemeTokenOverride> {
    // overrides 转 map 便于查找；同 key 取最后一个（后写覆盖先写）
    let mut override_map: HashMap<&str, &ThemeTokenOverride> = HashMap::new();
    for ov in overrides {
        override_map.insert(ov.key.as_str(), ov);
    }

    let mut result: Vec<ThemeTokenOverride> = Vec::with_capacity(theme.tokens.len());
    let mut consumed_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    for tok in &theme.tokens {
        if let Some(ov) = override_map.get(tok.key.as_str()) {
            result.push((*ov).clone());
        } else {
            result.push(tok.clone());
        }
        consumed_keys.insert(tok.key.clone());
    }

    // 追加 overrides 中独有的 key
    for ov in overrides {
        if !consumed_keys.contains(&ov.key) {
            result.push(ov.clone());
            consumed_keys.insert(ov.key.clone());
        }
    }

    result
}

/// 在所有主题（内置 + 自定义）中按 id 查找主题定义。
pub fn find_theme<'a>(
    themes: &'a [ThemeDefinition],
    id: &str,
) -> Option<&'a ThemeDefinition> {
    themes.iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme(id: &str, tokens: Vec<(&str, &str)>) -> ThemeDefinition {
        ThemeDefinition {
            id: id.to_string(),
            name: id.to_string(),
            builtin: false,
            tokens: tokens
                .into_iter()
                .map(|(k, v)| ThemeTokenOverride {
                    key: k.to_string(),
                    value: v.to_string(),
                })
                .collect(),
        }
    }

    fn override_(key: &str, value: &str) -> ThemeTokenOverride {
        ThemeTokenOverride {
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn merge_without_overrides_returns_theme_tokens() {
        let t = theme("a", vec![("--x", "1"), ("--y", "2")]);
        let merged = merge_theme_tokens(&t, &[]);
        assert_eq!(merged, t.tokens);
    }

    #[test]
    fn merge_overrides_replace_existing_values() {
        let t = theme("a", vec![("--x", "1"), ("--y", "2")]);
        let merged = merge_theme_tokens(&t, &[override_("--x", "9")]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].key, "--x");
        assert_eq!(merged[0].value, "9");
        assert_eq!(merged[1].key, "--y");
        assert_eq!(merged[1].value, "2");
    }

    #[test]
    fn merge_overrides_append_new_keys() {
        let t = theme("a", vec![("--x", "1")]);
        let merged = merge_theme_tokens(&t, &[override_("--z", "3")]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].key, "--x");
        assert_eq!(merged[0].value, "1");
        assert_eq!(merged[1].key, "--z");
        assert_eq!(merged[1].value, "3");
    }

    #[test]
    fn merge_last_override_wins_for_duplicate_override_keys() {
        let t = theme("a", vec![("--x", "1")]);
        let merged = merge_theme_tokens(
            &t,
            &[override_("--x", "9"), override_("--x", "8")],
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].value, "8");
    }

    #[test]
    fn find_theme_returns_matching() {
        let themes = vec![theme("a", vec![]), theme("b", vec![])];
        assert!(find_theme(&themes, "a").is_some());
        assert!(find_theme(&themes, "missing").is_none());
    }

    #[test]
    fn merge_preserves_theme_order() {
        let t = theme("a", vec![("--a", "1"), ("--b", "2"), ("--c", "3")]);
        let merged = merge_theme_tokens(&t, &[override_("--c", "30"), override_("--a", "10")]);
        // 顺序应保持 a/b/c，值被覆盖
        assert_eq!(merged[0].key, "--a");
        assert_eq!(merged[0].value, "10");
        assert_eq!(merged[1].key, "--b");
        assert_eq!(merged[1].value, "2");
        assert_eq!(merged[2].key, "--c");
        assert_eq!(merged[2].value, "30");
    }
}
