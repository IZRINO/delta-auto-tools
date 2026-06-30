//! 主题引擎模块。
//!
//! 提供 5 套内置主题 + 用户自定义主题 + 临时 overrides 的持久化与切换能力。
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
    let active_theme_id = if custom_mode {
        String::new()
    } else if settings.active_theme_id.is_empty() {
        builtins::INDUSTRIAL_LIGHT_ID.to_string()
    } else {
        settings.active_theme_id.clone()
    };

    // 找到激活主题定义：先自定义，后内置
    let active_theme = custom_themes
        .iter()
        .chain(builtin_themes.iter())
        .find(|t| t.id == active_theme_id)
        .cloned()
        .or_else(|| builtin_themes.first().cloned());

    let merged_tokens = if custom_mode {
        overrides.clone()
    } else {
        match active_theme {
            Some(ref theme) => apply::merge_theme_tokens(theme, &overrides),
            None => Vec::new(),
        }
    };

    ThemeBootstrap {
        active_theme_id: active_theme_id,
        builtin_themes,
        custom_themes,
        overrides,
        merged_tokens,
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

    let active_id = if settings.active_theme_id.is_empty() {
        builtins::INDUSTRIAL_LIGHT_ID.to_string()
    } else {
        settings.active_theme_id.clone()
    };
    let active_theme = apply::find_theme(&themes, &active_id);
    match active_theme {
        Some(theme) => apply::merge_theme_tokens(theme, &settings.overrides),
        None => Vec::new(),
    }
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
    fn build_bootstrap_defaults_to_industrial_light() {
        let state = sample_state();
        let builtin = builtins::builtin_themes();
        let boot = build_bootstrap(&state, builtin.clone());
        assert_eq!(boot.active_theme_id, "industrial-light");
        assert_eq!(boot.builtin_themes.len(), 5);
        assert!(boot.custom_themes.is_empty());
        // 默认无 overrides 时 merged_tokens 应等于 industrial-light 的 tokens
        assert_eq!(boot.merged_tokens.len(), builtin[0].tokens.len());
    }

    #[test]
    fn build_bootstrap_applies_overrides() {
        let mut settings = ThemeSettings::default();
        settings.overrides = vec![ThemeTokenOverride {
            key: "--amber".to_string(),
            value: "#FF0000".to_string(),
        }];
        let state = ThemeState::new(settings);
        let builtin = builtins::builtin_themes();
        let boot = build_bootstrap(&state, builtin);
        // amber 应被覆盖为红色
        let amber = boot
            .merged_tokens
            .iter()
            .find(|t| t.key == "--amber")
            .unwrap();
        assert_eq!(amber.value, "#FF0000");
    }


    #[test]
    fn build_bootstrap_uses_overrides_when_no_theme_selected() {
        let settings = ThemeSettings {
            active_theme_id: String::new(),
            custom_themes: Vec::new(),
            overrides: vec![
                ThemeTokenOverride {
                    key: "--carbon".to_string(),
                    value: "#111111".to_string(),
                },
                ThemeTokenOverride {
                    key: "--amber".to_string(),
                    value: "#FF0000".to_string(),
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
                key: "--chalk".to_string(),
                value: "#EEEEEE".to_string(),
            }],
        };
        let expected = settings.overrides.clone();
        let state = ThemeState::new(settings);
        assert_eq!(current_merged_tokens(&state), expected);
    }
    #[test]
    fn build_bootstrap_falls_back_to_first_builtin_when_active_id_missing() {
        let mut settings = ThemeSettings::default();
        settings.active_theme_id = "nonexistent".to_string();
        let state = ThemeState::new(settings);
        let builtin = builtins::builtin_themes();
        let boot = build_bootstrap(&state, builtin.clone());
        // active_id 无效时回退到第一个内置主题（industrial-light）的 tokens，
        // 但 active_theme_id 字段仍保留原值（前端可据此提示用户当前生效主题与配置不一致）
        assert_eq!(boot.active_theme_id, "nonexistent");
        // merged_tokens 应等于第一个内置主题的 tokens（无 overrides 时）
        let expected = apply::merge_theme_tokens(&builtin[0], &[]);
        assert_eq!(boot.merged_tokens, expected);
    }

    #[test]
    fn theme_import_parses_valid_json() {
        let json = r##"{"id":"custom","name":"自定义","builtin":false,"tokens":[{"key":"--amber","value":"#FF0000"}]}"##;
        let theme = theme_import(json.to_string()).unwrap();
        assert_eq!(theme.id, "custom");
        assert_eq!(theme.name, "自定义");
        assert!(!theme.builtin);
        assert_eq!(theme.tokens.len(), 1);
    }

    #[test]
    fn theme_import_rejects_invalid_key() {
        let json = r##"{"id":"custom","name":"自定义","builtin":false,"tokens":[{"key":"amber","value":"#FF0000"}]}"##;
        let err = theme_import(json.to_string()).unwrap_err();
        assert!(err.contains("必须以 -- 开头"));
    }

    #[test]
    fn theme_export_returns_pretty_json() {
        let state = sample_state();
        let json = export_theme(&state.settings.lock().unwrap(), "industrial-light").unwrap();
        assert!(json.contains("\"id\": \"industrial-light\""));
        assert!(json.contains("\"name\": \"工业亮色\""));
    }
}
