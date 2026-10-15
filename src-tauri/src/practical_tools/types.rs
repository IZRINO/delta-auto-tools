use serde::{Deserialize, Serialize};

use crate::morse::types::RegionRect;

fn default_weapon_keys() -> [String; 5] {
    ["1", "2", "3", "4", "5"].map(str::to_string)
}
fn default_fire_key() -> String {
    "MouseLeft".to_string()
}
fn default_auto_full_auto_toggle_key() -> String {
    "F6".to_string()
}
fn default_start_delay() -> u64 {
    80
}
fn default_weapon_detection_delay() -> u64 {
    100
}
fn default_press_ms() -> u64 {
    30
}
fn default_interval_ms() -> u64 {
    30
}
fn default_threshold() -> f32 {
    0.75
}
fn default_auto_indicator_opacity() -> f32 {
    0.9
}
fn default_auto_indicator_right() -> u32 {
    28
}
fn default_auto_indicator_bottom() -> u32 {
    28
}
fn default_bomb_target_color() -> String {
    "#F5C400".to_string()
}
fn default_bomb_color_tolerance() -> u8 {
    55
}
fn default_duration() -> u32 {
    30
}
fn default_warning() -> u32 {
    5
}
fn default_width() -> u32 {
    420
}
fn default_height() -> u32 {
    28
}
fn default_opacity() -> f32 {
    0.85
}
fn default_false() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PracticalToolsSettings {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default)]
    pub auto_full_auto_enabled: bool,
    #[serde(default = "default_auto_full_auto_toggle_key")]
    pub auto_full_auto_toggle_key: String,
    #[serde(default = "default_fire_key")]
    pub fire_key: String,
    #[serde(default = "default_weapon_keys")]
    pub weapon_keys: [String; 5],
    #[serde(default = "default_start_delay")]
    pub start_delay_ms: u64,
    #[serde(default = "default_weapon_detection_delay")]
    pub weapon_detection_delay_ms: u64,
    #[serde(default = "default_press_ms")]
    pub press_ms: u64,
    #[serde(default = "default_interval_ms")]
    pub interval_ms: u64,
    #[serde(default)]
    pub auto_region: Option<RegionRect>,
    #[serde(default = "default_threshold")]
    pub match_threshold: f32,
    #[serde(default = "default_auto_indicator_opacity")]
    pub auto_indicator_opacity: f32,
    #[serde(default = "default_auto_indicator_right")]
    pub auto_indicator_right: u32,
    #[serde(default = "default_auto_indicator_bottom")]
    pub auto_indicator_bottom: u32,
    #[serde(default)]
    pub bomb_progress_enabled: bool,
    #[serde(default)]
    pub install_region: Option<RegionRect>,
    #[serde(default)]
    pub bomb_icon_region: Option<RegionRect>,
    #[serde(default = "default_bomb_target_color")]
    pub bomb_target_color: String,
    #[serde(default = "default_bomb_color_tolerance")]
    pub bomb_color_tolerance: u8,
    #[serde(default = "default_duration")]
    pub bomb_duration_seconds: u32,
    #[serde(default = "default_warning")]
    pub warning_seconds: u32,
    #[serde(default)]
    pub overlay_x: i32,
    #[serde(default)]
    pub overlay_y: i32,
    #[serde(default = "default_width")]
    pub overlay_width: u32,
    #[serde(default = "default_height")]
    pub overlay_height: u32,
    #[serde(default = "default_opacity")]
    pub overlay_opacity: f32,
}

impl Default for PracticalToolsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_full_auto_enabled: false,
            auto_full_auto_toggle_key: default_auto_full_auto_toggle_key(),
            fire_key: default_fire_key(),
            weapon_keys: default_weapon_keys(),
            start_delay_ms: default_start_delay(),
            weapon_detection_delay_ms: default_weapon_detection_delay(),
            press_ms: default_press_ms(),
            interval_ms: default_interval_ms(),
            auto_region: None,
            match_threshold: default_threshold(),
            auto_indicator_opacity: default_auto_indicator_opacity(),
            auto_indicator_right: default_auto_indicator_right(),
            auto_indicator_bottom: default_auto_indicator_bottom(),
            bomb_progress_enabled: false,
            install_region: None,
            bomb_icon_region: None,
            bomb_target_color: default_bomb_target_color(),
            bomb_color_tolerance: default_bomb_color_tolerance(),
            bomb_duration_seconds: default_duration(),
            warning_seconds: default_warning(),
            overlay_x: 0,
            overlay_y: 0,
            overlay_width: default_width(),
            overlay_height: default_height(),
            overlay_opacity: default_opacity(),
        }
    }
}

impl PracticalToolsSettings {
    pub fn normalize(mut self) -> Result<Self, String> {
        self.fire_key = self.fire_key.trim().to_string();
        if self.fire_key.is_empty() {
            return Err("开火键不能为空".into());
        }
        if !matches!(
            self.fire_key.as_str(),
            "MouseLeft" | "MouseRight" | "MouseMiddle" | "MouseBack" | "MouseForward"
        ) {
            let binding = crate::hotkey_types::HotkeyBinding::parse(&self.fire_key)?;
            if !binding.modifiers.is_empty() || binding.extra_primary.is_some() {
                return Err("开火键必须是单键".into());
            }
            self.fire_key = crate::hotkey_types::binding_to_string(&binding);
        }
        let toggle_binding =
            crate::hotkey_types::HotkeyBinding::parse(&self.auto_full_auto_toggle_key)?;
        if toggle_binding.extra_primary.is_some() {
            return Err("自动全自动切换键必须是单个主键或修饰键组合".into());
        }
        self.auto_full_auto_toggle_key = crate::hotkey_types::binding_to_string(&toggle_binding);
        for key in &mut self.weapon_keys {
            *key = key.trim().to_string();
            if key.is_empty() {
                return Err("切枪快捷键不能为空".into());
            }
            let binding = crate::hotkey_types::HotkeyBinding::parse(key)?;
            if !binding.modifiers.is_empty() || binding.extra_primary.is_some() {
                return Err("切枪快捷键必须是单键".into());
            }
            *key = crate::hotkey_types::binding_to_string(&binding);
        }
        self.start_delay_ms = self.start_delay_ms.min(10_000);
        self.weapon_detection_delay_ms = self.weapon_detection_delay_ms.min(10_000);
        self.press_ms = self.press_ms.clamp(1, 2_000);
        self.interval_ms = self.interval_ms.clamp(1, 2_000);
        self.match_threshold = self.match_threshold.clamp(0.0, 1.0);
        self.auto_indicator_opacity = self.auto_indicator_opacity.clamp(0.05, 1.0);
        self.auto_indicator_right = self.auto_indicator_right.min(10000);
        self.auto_indicator_bottom = self.auto_indicator_bottom.min(10000);
        self.bomb_target_color = self.bomb_target_color.trim().to_ascii_uppercase();
        if !is_hex_color(&self.bomb_target_color) {
            return Err("炸弹识色目标必须是 #RRGGBB 格式".into());
        }
        self.bomb_duration_seconds = self.bomb_duration_seconds.clamp(1, 3600);
        self.warning_seconds = self.warning_seconds.min(self.bomb_duration_seconds);
        self.overlay_width = self.overlay_width.clamp(160, 1600);
        self.overlay_height = self.overlay_height.clamp(12, 240);
        self.overlay_opacity = self.overlay_opacity.clamp(0.05, 1.0);
        Ok(self)
    }
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_compatible_and_normalizable() {
        let settings = PracticalToolsSettings::default().normalize().unwrap();
        assert!(!settings.enabled);
        assert_eq!(settings.auto_full_auto_toggle_key, "F6");
        assert_eq!(settings.weapon_keys, ["1", "2", "3", "4", "5"]);
        assert_eq!(settings.start_delay_ms, 80);
        assert_eq!(settings.weapon_detection_delay_ms, 100);
        assert_eq!(settings.overlay_opacity, 0.85);
        assert_eq!(settings.auto_indicator_opacity, 0.9);
        assert_eq!(settings.auto_indicator_right, 28);
        assert_eq!(settings.auto_indicator_bottom, 28);
        assert_eq!(settings.bomb_target_color, "#F5C400");
        assert_eq!(settings.bomb_color_tolerance, 55);
    }

    #[test]
    fn missing_weapon_detection_delay_uses_default() {
        let settings: PracticalToolsSettings = serde_json::from_str(
            r#"{"enabled":false,"autoFullAutoEnabled":false,"fireKey":"MouseLeft","weaponKeys":["1","2","3","4","5"],"startDelayMs":80,"pressMs":30,"intervalMs":30,"autoRegion":null,"matchThreshold":0.75,"bombProgressEnabled":false,"installRegion":null,"bombIconRegion":null,"bombDurationSeconds":30,"warningSeconds":5,"overlayX":0,"overlayY":0,"overlayWidth":420,"overlayHeight":28,"overlayOpacity":0.85}"#,
        )
        .unwrap();
        assert_eq!(settings.weapon_detection_delay_ms, 100);
        assert_eq!(settings.auto_indicator_opacity, 0.9);
        assert_eq!(settings.bomb_target_color, "#F5C400");
        assert_eq!(settings.bomb_color_tolerance, 55);
    }

    #[test]
    fn malformed_bomb_target_color_is_rejected() {
        let settings = PracticalToolsSettings {
            bomb_target_color: "yellow".into(),
            ..Default::default()
        };
        assert!(settings.normalize().is_err());
    }
    #[test]
    fn malformed_empty_fire_key_is_rejected() {
        let settings = PracticalToolsSettings {
            fire_key: String::new(),
            ..Default::default()
        };
        assert!(settings.normalize().is_err());
    }

    #[test]
    fn mouse_buttons_and_symbol_keys_are_supported() {
        let settings = PracticalToolsSettings {
            fire_key: "MouseBack".into(),
            weapon_keys: ["1".into(), "2".into(), "3".into(), "-".into(), "=".into()],
            ..Default::default()
        }
        .normalize()
        .unwrap();
        assert_eq!(settings.fire_key, "MouseBack");
        assert_eq!(settings.weapon_keys[3], "-");
        assert_eq!(settings.weapon_keys[4], "=");
    }

    #[test]
    fn auto_full_auto_toggle_key_accepts_modifier_hotkeys() {
        let settings = PracticalToolsSettings {
            auto_full_auto_toggle_key: "Ctrl+F6".into(),
            ..Default::default()
        }
        .normalize()
        .unwrap();

        assert_eq!(settings.auto_full_auto_toggle_key, "Ctrl+F6");
    }
}
