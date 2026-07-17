//! 主题引擎模块。
//!
//! 提供 3 套 daisyUI 内置主题 + 用户自定义主题 + 临时 overrides 的持久化与切换能力。
//! 保存主题设置后向 `main` 窗口推送合并后的最终 token 列表，
//! 前端 listener 收到后遍历 token 调用 `document.documentElement.style.setProperty` 覆盖 CSS 变量。
//!
//! 命令面：
//! - `theme_get_bootstrap` — 获取主题初始状态
//! - `theme_save_settings` — 保存主题设置，保存后 emit `theme://changed`
//! - `theme_export` — 导出指定主题为 JSON 字符串
//! - `theme_import` — 解析导入的 JSON 为 ThemeDefinition（不直接保存）

pub mod apply;
pub mod builtins;
pub mod events;
pub mod settings;
pub mod types;

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};

use self::types::{ThemeBootstrap, ThemeDefinition, ThemeSettings, ThemeTokenOverride};

/// 主题模块运行时状态。
///
/// 仅持有一份 `ThemeSettings`，被 `Mutex` 保护。
/// 不走 `tool_base::ToolState` 泛型层，因为主题没有热键 / 透明窗口 / 运行态等工具特有概念。
pub struct ThemeState {
    settings: Mutex<ThemeSettings>,
}

impl ThemeState {
    pub fn new(settings: ThemeSettings) -> Self {
        Self {
            settings: Mutex::new(settings),
        }
    }
}

/// 初始化主题状态：加载持久化 `theme_settings.json`，缺失则用默认值。
pub fn initialize(app: &AppHandle) -> Result<ThemeState, String> {
    let settings = settings::load_settings(app).unwrap_or_default();
    Ok(ThemeState::new(settings))
}

/// 构建主题 bootstrap：合并内置 + 自定义主题列表，计算当前激活主题的最终 token 列表。
fn build_bootstrap(state: &ThemeState, builtin_themes: Vec<ThemeDefinition>) -> ThemeBootstrap {
    let settings = state.settings.lock().expect("主题状态锁被污染");

    let custom_themes = settings.custom_themes.clone();
    let overrides = settings.overrides.clone();
    let custom_mode = settings.active_theme_id.is_empty() && !overrides.is_empty();
    let themes = all_themes(&settings);
    let active_theme_id = if custom_mode {
        String::new()
    } else {
        resolve_active_theme_id(&settings.active_theme_id, &themes)
    };

    let merged_tokens = if custom_mode {
        overrides.clone()
    } else {
        apply::find_theme(&themes, &active_theme_id)
            .map(|theme| apply::merge_theme_tokens(theme, &overrides))
            .unwrap_or_default()
    };

    ThemeBootstrap {
        active_theme_id,
        builtin_themes,
        custom_themes,
        overrides,
        merged_tokens,
    }
}

fn resolve_active_theme_id(active_theme_id: &str, themes: &[ThemeDefinition]) -> String {
    // ponytail: 旧主题 id 或损坏配置统一回默认主题，避免空 token 白屏。
    if active_theme_id.is_empty() || apply::find_theme(themes, active_theme_id).is_none() {
        builtins::VALENTINE_ID.to_string()
    } else {
        active_theme_id.to_string()
    }
}

/// 合并内置主题与自定义主题列表（自定义在前，便于前端按用户优先级展示）。
fn all_themes(settings: &ThemeSettings) -> Vec<ThemeDefinition> {
    let mut all = settings.custom_themes.clone();
    all.extend(builtins::builtin_themes());
    all
}

/// 读取当前激活主题的最终 token 列表（供命令侧主动 emit）。
fn current_merged_tokens(state: &ThemeState) -> Vec<ThemeTokenOverride> {
    let settings = state.settings.lock().expect("主题状态锁被污染");
    let themes = all_themes(&settings);
    if settings.active_theme_id.is_empty() && !settings.overrides.is_empty() {
        return settings.overrides.clone();
    }

    let active_id = resolve_active_theme_id(&settings.active_theme_id, &themes);
    apply::find_theme(&themes, &active_id)
        .map(|theme| apply::merge_theme_tokens(theme, &settings.overrides))
        .unwrap_or_default()
}

#[tauri::command]
pub fn theme_get_bootstrap(state: State<'_, ThemeState>) -> Result<ThemeBootstrap, String> {
    let builtin = builtins::builtin_themes();
    Ok(build_bootstrap(&state, builtin))
}

#[tauri::command]
pub fn theme_save_settings(
    app: AppHandle,
    state: State<'_, ThemeState>,
    settings_value: ThemeSettings,
) -> Result<ThemeBootstrap, String> {
    {
        let mut current = state.settings.lock().map_err(|_| "主题状态锁已损坏")?;
        *current = settings_value;
    }

    // 持久化前再读一次（避免持有锁期间做 IO）
    let to_save = state
        .settings
        .lock()
        .map_err(|_| "主题状态锁已损坏")?
        .clone();
    settings::save_settings(&app, &to_save)?;

    // 推送合并后的最终 token 列表到 main 窗口
    let merged = current_merged_tokens(&state);
    let _ = app.emit_to("main", events::CHANGED, merged);

    let builtin = builtins::builtin_themes();
    Ok(build_bootstrap(&state, builtin))
}

#[tauri::command]
pub fn theme_export(state: State<'_, ThemeState>, id: String) -> Result<String, String> {
    let settings = state.settings.lock().map_err(|_| "主题状态锁已损坏")?;
    export_theme(&settings, &id)
}

/// 导出指定主题为 JSON 字符串的纯函数（供命令与测试复用）。
pub fn export_theme(settings: &ThemeSettings, id: &str) -> Result<String, String> {
    let themes = all_themes(settings);
    let theme = apply::find_theme(&themes, id).ok_or_else(|| format!("找不到主题: {id}"))?;
    serde_json::to_string_pretty(theme).map_err(|e| format!("导出失败: {e}"))
}

#[tauri::command]
pub fn theme_import(json: String) -> Result<ThemeDefinition, String> {
    let theme: ThemeDefinition =
        serde_json::from_str(&json).map_err(|e| format!("主题 JSON 解析失败: {e}"))?;
    // 校验 token key 必须以 -- 开头
    for tok in &theme.tokens {
        if !tok.key.starts_with("--") {
            return Err(format!("token key {} 必须以 -- 开头", tok.key));
        }
    }
    Ok(theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> ThemeState {
        ThemeState::new(ThemeSettings::default())
    }

    #[test]
    fn build_bootstrap_defaults_to_valentine() {
        let state = sample_state();
        let builtin = builtins::builtin_themes();
        let boot = build_bootstrap(&state, builtin.clone());
        assert_eq!(boot.active_theme_id, "valentine");
        assert_eq!(boot.builtin_themes.len(), 3);
        assert!(boot.custom_themes.is_empty());
        // 默认无 overrides 时 merged_tokens 应等于 valentine 的 tokens
        let valentine = builtin.iter().find(|t| t.id == "valentine").unwrap();
        assert_eq!(boot.merged_tokens.len(), valentine.tokens.len());
    }

    #[test]
    fn build_bootstrap_applies_overrides() {
        let settings = ThemeSettings {
            overrides: vec![ThemeTokenOverride {
                key: "--color-primary".to_string(),
                value: "oklch(50% 0.2 20)".to_string(),
            }],
            ..Default::default()
        };
        let state = ThemeState::new(settings);
        let builtin = builtins::builtin_themes();
        let boot = build_bootstrap(&state, builtin);
        // color-primary 应被覆盖
        let primary = boot
            .merged_tokens
            .iter()
            .find(|t| t.key == "--color-primary")
            .unwrap();
        assert_eq!(primary.value, "oklch(50% 0.2 20)");
    }

    #[test]
    fn build_bootstrap_uses_overrides_when_no_theme_selected() {
        let settings = ThemeSettings {
            active_theme_id: String::new(),
            custom_themes: Vec::new(),
            overrides: vec![
                ThemeTokenOverride {
                    key: "--color-base-100".to_string(),
                    value: "oklch(20% 0 0)".to_string(),
                },
                ThemeTokenOverride {
                    key: "--color-primary".to_string(),
                    value: "oklch(60% 0.2 20)".to_string(),
                },
            ],
        };
        let expected = settings.overrides.clone();
        let state = ThemeState::new(settings);
        let boot = build_bootstrap(&state, builtins::builtin_themes());
        assert_eq!(boot.active_theme_id, "");
        assert_eq!(boot.merged_tokens, expected);
    }

    #[test]
    fn current_merged_tokens_uses_overrides_when_no_theme_selected() {
        let settings = ThemeSettings {
            active_theme_id: String::new(),
            custom_themes: Vec::new(),
            overrides: vec![ThemeTokenOverride {
                key: "--color-base-content".to_string(),
                value: "oklch(90% 0 0)".to_string(),
            }],
        };
        let expected = settings.overrides.clone();
        let state = ThemeState::new(settings);
        assert_eq!(current_merged_tokens(&state), expected);
    }

    #[test]
    fn current_merged_tokens_falls_back_to_valentine_when_active_id_missing() {
        let settings = ThemeSettings {
            active_theme_id: "industrial-light".to_string(),
            ..Default::default()
        };
        let state = ThemeState::new(settings);
        let builtin = builtins::builtin_themes();
        let valentine = builtin
            .iter()
            .find(|t| t.id == builtins::VALENTINE_ID)
            .unwrap();
        assert_eq!(
            current_merged_tokens(&state),
            apply::merge_theme_tokens(valentine, &[])
        );
    }

    #[test]
    fn build_bootstrap_falls_back_to_valentine_when_active_id_missing() {
        let settings = ThemeSettings {
            active_theme_id: "nonexistent".to_string(),
            ..Default::default()
        };
        let state = ThemeState::new(settings);
        let builtin = builtins::builtin_themes();
        let boot = build_bootstrap(&state, builtin.clone());
        assert_eq!(boot.active_theme_id, builtins::VALENTINE_ID);
        let valentine = builtin
            .iter()
            .find(|t| t.id == builtins::VALENTINE_ID)
            .unwrap();
        let expected = apply::merge_theme_tokens(valentine, &[]);
        assert_eq!(boot.merged_tokens, expected);
    }

    #[test]
    fn theme_import_parses_valid_json() {
        let json = r##"{"id":"custom","name":"自定义","builtin":false,"tokens":[{"key":"--color-primary","value":"oklch(50% 0.2 20)"}]}"##;
        let theme = theme_import(json.to_string()).unwrap();
        assert_eq!(theme.id, "custom");
        assert_eq!(theme.name, "自定义");
        assert!(!theme.builtin);
        assert_eq!(theme.tokens.len(), 1);
    }

    #[test]
    fn theme_import_rejects_invalid_key() {
        let json = r##"{"id":"custom","name":"自定义","builtin":false,"tokens":[{"key":"color-primary","value":"oklch(50% 0.2 20)"}]}"##;
        let err = theme_import(json.to_string()).unwrap_err();
        assert!(err.contains("必须以 -- 开头"));
    }

    #[test]
    fn theme_export_returns_pretty_json() {
        let state = sample_state();
        let json = export_theme(&state.settings.lock().unwrap(), "valentine").unwrap();
        assert!(json.contains("\"id\": \"valentine\""));
        assert!(json.contains("\"name\": \"黑红\""));
    }
}
