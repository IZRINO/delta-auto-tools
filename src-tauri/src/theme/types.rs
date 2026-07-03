//! 主题引擎前端类型定义。
//!
//! 所有结构体均使用 `#[serde(rename_all = "camelCase")]`，与全仓 serde 约定保持一致，
//! 前端通过 camelCase JSON 反序列化。

use serde::{Deserialize, Serialize};

/// 单个 CSS 变量覆盖项。
///
/// `key` 必须以 `--` 开头（如 `--amber`），`value` 为合法 CSS 颜色或值（如 `#E8A000`）。
/// 前端在应用主题时遍历 `merged_tokens` 调用 `document.documentElement.style.setProperty(key, value)`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeTokenOverride {
    pub key: String,
    pub value: String,
}

/// 一套完整主题定义。
///
/// 内置主题（`builtin == true`）由 Rust 常量提供，不可删除，只能派生；
/// 自定义主题（`builtin == false`）由用户基于内置主题派生或完全自建，存到 `theme_settings.json`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeDefinition {
    /// 主题唯一 id，内置主题使用稳定短码（如 `valentine`），自定义主题用时间戳 + 随机串生成。
    pub id: String,
    /// 主题显示名，如「工业亮色」。
    pub name: String,
    /// 是否内置主题。内置主题不可删除、不可重命名。
    pub builtin: bool,
    /// 该主题包含的全部 token 覆盖项。
    pub tokens: Vec<ThemeTokenOverride>,
}

/// 主题持久化设置，存到 `theme_settings.json`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSettings {
    /// 当前激活主题 id。空串且 overrides 非空表示自定义配色模式；空串且 overrides 为空时使用默认内置主题。
    #[serde(default)]
    pub active_theme_id: String,
    /// 用户自定义主题列表。
    #[serde(default)]
    pub custom_themes: Vec<ThemeDefinition>,
    /// 主题 token 覆盖；自定义配色模式下保存完整 token 集。
    #[serde(default)]
    pub overrides: Vec<ThemeTokenOverride>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            active_theme_id: crate::theme::builtins::VALENTINE_ID.to_string(),
            custom_themes: Vec::new(),
            overrides: Vec::new(),
        }
    }
}

/// 主题 bootstrap：一次性返回前端所需的全部主题信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeBootstrap {
    /// 当前激活主题 id。空串且 overrides 非空表示自定义配色模式。
    pub active_theme_id: String,
    /// 内置主题列表（3 套）。
    pub builtin_themes: Vec<ThemeDefinition>,
    /// 自定义主题列表。
    pub custom_themes: Vec<ThemeDefinition>,
    /// 当前 token 覆盖。
    pub overrides: Vec<ThemeTokenOverride>,
    /// 合并后的最终 token 列表（内置/自定义主题 tokens + overrides），前端直接写入 CSS 变量。
    pub merged_tokens: Vec<ThemeTokenOverride>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_uses_valentine() {
        let settings = ThemeSettings::default();
        assert_eq!(settings.active_theme_id, "valentine");
        assert!(settings.custom_themes.is_empty());
        assert!(settings.overrides.is_empty());
    }

    #[test]
    fn theme_definition_serializes_camel_case() {
        let theme = ThemeDefinition {
            id: "test".to_string(),
            name: "测试".to_string(),
            builtin: false,
            tokens: vec![ThemeTokenOverride {
                key: "--amber".to_string(),
                value: "#E8A000".to_string(),
            }],
        };
        let json = serde_json::to_string(&theme).unwrap();
        // 字段名应为 camelCase
        assert!(json.contains("\"id\""));
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"builtin\""));
        assert!(json.contains("\"tokens\""));
        // 不应出现 snake_case 形式
        assert!(!json.contains("\"active_theme_id\""));
    }

    #[test]
    fn theme_settings_round_trip() {
        let settings = ThemeSettings {
            active_theme_id: "custom-1".to_string(),
            custom_themes: vec![ThemeDefinition {
                id: "custom-1".to_string(),
                name: "自定义1".to_string(),
                builtin: false,
                tokens: Vec::new(),
            }],
            overrides: vec![ThemeTokenOverride {
                key: "--amber".to_string(),
                value: "#FF0000".to_string(),
            }],
        };
        let json = serde_json::to_string(&settings).unwrap();
        let loaded: ThemeSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, settings);
    }

    #[test]
    fn theme_settings_missing_fields_use_default() {
        // 空 JSON 应能反序列化（所有字段带 #[serde(default)]）
        let json = "{}";
        let loaded: ThemeSettings = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.active_theme_id, "");
        assert!(loaded.custom_themes.is_empty());
        assert!(loaded.overrides.is_empty());
    }
}
