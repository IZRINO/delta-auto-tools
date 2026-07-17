use serde::{Deserialize, Deserializer, Serialize};

use crate::morse::types::RegionRect;

fn default_recognition_enabled() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_volume() -> f32 {
    0.8
}

fn default_cooldown_ms() -> u32 {
    1000
}

fn default_activation_duration_ms() -> u32 {
    10000
}

fn default_activation_trigger_count() -> u32 {
    1
}

fn default_watch_match_threshold() -> f32 {
    0.75
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ReferenceImagePaths {
    One(String),
    Many(Vec<String>),
}

fn deserialize_reference_image_paths<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(
        match Option::<ReferenceImagePaths>::deserialize(deserializer)? {
            Some(ReferenceImagePaths::One(path)) => vec![path],
            Some(ReferenceImagePaths::Many(paths)) => paths,
            None => Vec::new(),
        },
    )
}

fn default_watch_poll_interval_ms() -> u32 {
    500
}

fn default_color_tolerance() -> u8 {
    30
}

pub(crate) const DEFAULT_COLOR_TOLERANCE: u8 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionSettings {
    #[serde(default = "default_recognition_enabled", alias = "audioEnabled")]
    pub recognition_enabled: bool,
    #[serde(default)]
    pub card_groups: Vec<RecognitionGroup>,
    #[serde(default)]
    pub cards: Vec<RecognitionCard>,
}

impl Default for RecognitionSettings {
    fn default() -> Self {
        Self {
            recognition_enabled: true,
            card_groups: Vec::new(),
            cards: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionCard {
    pub id: String,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub order: i32,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub trigger_mode: RecognitionTriggerMode,
    // 快捷键模式
    #[serde(default)]
    pub hotkey: Option<String>,
    // 区域监听模式
    #[serde(default)]
    pub watch_region: Option<RegionRect>,
    #[serde(
        default,
        alias = "watchReferenceImagePath",
        deserialize_with = "deserialize_reference_image_paths"
    )]
    pub watch_reference_image_paths: Vec<String>,
    #[serde(default = "default_watch_match_threshold")]
    pub watch_match_threshold: f32,
    #[serde(default = "default_watch_poll_interval_ms")]
    pub watch_poll_interval_ms: u32,
    /// 常驻识别是否等待目标消失后再触发。
    #[serde(default)]
    pub retrigger_after_disappear: bool,
    #[serde(default)]
    pub activation: RecognitionActivation,
    #[serde(default)]
    pub effects: RecognitionEffects,
    // 旧播放字段：仅用于迁移到 effects.audio。
    /// 音频文件列表（按序）。连杀顺序=数组顺序；单文件时为单元素数组。
    #[serde(default, skip_serializing)]
    pub audio_files: Vec<String>,
    /// 仅用于反序列化旧 JSON 的单值 audioFilePath 字段；normalize_settings 迁移进 audio_files 后清空。
    /// 序列化时跳过，不输出到新 JSON。
    #[serde(default, rename = "audioFilePath", skip_serializing)]
    pub legacy_audio_file_path: Option<String>,
    /// 播放方式：Single/Combo/Random
    #[serde(default, skip_serializing)]
    pub play_mode: PlayMode,
    /// 连杀窗口（毫秒），从上一次触发起算；超时复位第一首。默认 60000。
    /// 作为卡片级默认窗口，被 combo_windows 缺省 index 时回落使用。
    #[serde(default = "default_combo_window_ms", skip_serializing)]
    pub combo_window_ms: u32,
    /// 每段音频各自的连杀窗口（毫秒）。播完第 i 段后用 combo_windows[i] 判断是否进 i+1 段（Issue #62）。
    /// 长度可小于 audio_files，缺省 index 回落到 combo_window_ms。空数组 = 全用卡片级默认窗口（向后兼容）。
    #[serde(default, skip_serializing)]
    pub combo_windows: Vec<u32>,
    #[serde(default = "default_volume", skip_serializing)]
    pub volume: f32,
    #[serde(default = "default_cooldown_ms")]
    pub cooldown_ms: u32,
    /// 允许此卡片的音频与其他卡片同时播放（默认互斥）
    #[serde(default, skip_serializing)]
    pub allow_simultaneous: bool,
    // 识色模式探针列表
    #[serde(default)]
    pub color_probes: Vec<ColorProbe>,
    // 识色聚合模式
    #[serde(default)]
    pub color_match_mode: ColorMatchMode,
    // 识色匹配方式：Average=平均色判定；AnyPixel=单像素命中。缺省 Average（识色1，向后兼容）
    #[serde(default)]
    pub color_match_method: ColorMatchMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionActivation {
    #[serde(default)]
    pub mode: RecognitionActivationMode,
    #[serde(default)]
    pub hotkey: Option<String>,
    #[serde(default = "default_activation_duration_ms")]
    pub duration_ms: u32,
    #[serde(default = "default_activation_trigger_count")]
    pub trigger_count: u32,
}

impl Default for RecognitionActivation {
    fn default() -> Self {
        Self {
            mode: RecognitionActivationMode::Always,
            hotkey: None,
            duration_ms: default_activation_duration_ms(),
            trigger_count: default_activation_trigger_count(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum RecognitionActivationMode {
    #[default]
    Always,
    OnceHotkey,
    TimedHotkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionEffects {
    #[serde(default)]
    pub audio: Option<RecognitionAudioEffect>,
    #[serde(default)]
    pub hotkey: Option<RecognitionHotkeyEffect>,
    #[serde(default)]
    pub click: Option<RecognitionClickEffect>,
}

impl RecognitionEffects {
    pub fn has_any(&self) -> bool {
        self.audio.is_some() || self.hotkey.is_some() || self.click.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionAudioEffect {
    #[serde(default)]
    pub audio_files: Vec<String>,
    #[serde(default)]
    pub play_mode: PlayMode,
    #[serde(default = "default_combo_window_ms")]
    pub combo_window_ms: u32,
    #[serde(default)]
    pub combo_windows: Vec<u32>,
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default)]
    pub allow_simultaneous: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionHotkeyEffectStep {
    pub hotkey: String,
    #[serde(default)]
    pub delay_ms: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionHotkeyEffect {
    #[serde(default)]
    pub hotkey: String,
    #[serde(default)]
    pub steps: Vec<RecognitionHotkeyEffectStep>,
}

impl RecognitionHotkeyEffect {
    pub fn normalized_steps(&self) -> Vec<RecognitionHotkeyEffectStep> {
        let steps: Vec<_> = self
            .steps
            .iter()
            .filter(|step| !step.hotkey.trim().is_empty())
            .cloned()
            .collect();
        if !steps.is_empty() {
            return steps;
        }
        if self.hotkey.trim().is_empty() {
            Vec::new()
        } else {
            vec![RecognitionHotkeyEffectStep {
                hotkey: self.hotkey.clone(),
                delay_ms: 0,
            }]
        }
    }
}

impl<'de> Deserialize<'de> for RecognitionHotkeyEffect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Helper {
            #[serde(default)]
            hotkey: String,
            #[serde(default)]
            steps: Vec<RecognitionHotkeyEffectStep>,
        }

        let helper = Helper::deserialize(deserializer)?;
        let mut effect = Self {
            hotkey: helper.hotkey,
            steps: helper.steps,
        };
        if effect.steps.is_empty() && !effect.hotkey.trim().is_empty() {
            effect.steps.push(RecognitionHotkeyEffectStep {
                hotkey: effect.hotkey.clone(),
                delay_ms: 0,
            });
        }
        Ok(effect)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionClickEffect {
    #[serde(default)]
    pub mode: RecognitionClickMode,
    #[serde(default)]
    pub custom_region: Option<RegionRect>,
    #[serde(default)]
    pub color_probe_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum RecognitionClickMode {
    #[default]
    CustomRegion,
    RecognitionRegion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum RecognitionTriggerMode {
    #[default]
    Hotkey,
    RegionWatch,
    ColorWatch,
}

/// 音频播放方式：叠加在触发模式之上的文件选择策略。
/// Single=单文件；Combo=连杀（窗口内按序递增，末首后保持，超时复位）；Random=随机（不重复上一次）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum PlayMode {
    #[default]
    Single,
    Combo,
    Random,
}

fn default_combo_window_ms() -> u32 {
    60000
}

/// 识色探针内的单个目标颜色（含独立容差）。
///
/// 每个探针可配置多个目标颜色，探针内按 `probe_match_mode`（默认 Any）聚合：
/// 任一目标命中即视为该探针命中（Any）；全部命中才视为命中（All）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ColorTarget {
    /// 目标 RGB 颜色 [R, G, B]，每通道 0-255
    pub color: [u8; 3],
    /// 颜色容差（RGB 欧氏距离阈值，0-255）
    #[serde(default = "default_color_tolerance")]
    pub tolerance: u8,
}

fn default_probe_match_mode_any() -> ColorMatchMode {
    ColorMatchMode::Any
}

/// 识色探针：一个矩形区域 + 多个目标颜色（每个含独立容差）+ 探针内聚合模式
///
/// `region` 可为 None：用户刚新增探针、尚未框选区域的草稿态。
/// watcher 启动时会跳过含 None 探针的卡片，使其能作为中间态被保存
/// （Issue #61/#60：避免 autosave / flushSettings 因 region 缺失而整体失败）。
///
/// Issue #65：探针内支持多目标颜色。旧 JSON 的单值 `targetColor`/`tolerance`
/// 在 `normalize_settings` 中迁移为单元素 `targets`；序列化只输出 `targets`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ColorProbe {
    #[serde(default)]
    pub region: Option<RegionRect>,
    /// 多目标颜色列表（Issue #65）。空列表时 watcher 视为未就绪。
    #[serde(default)]
    pub targets: Vec<ColorTarget>,
    /// 探针内聚合模式：All = 所有目标都命中才视为探针命中；Any = 任一命中即视为命中。缺省 Any。
    #[serde(default = "default_probe_match_mode_any")]
    pub probe_match_mode: ColorMatchMode,
    // ---- 旧字段：仅反序列化兼容，序列化时 skip ----
    /// 旧单值目标颜色，反序列化旧 JSON 用；`normalize_settings` 迁移进 `targets` 后清空。
    #[serde(default, rename = "targetColor", skip_serializing)]
    pub legacy_target_color: Option<[u8; 3]>,
    /// 旧单值容差，反序列化旧 JSON 用；`normalize_settings` 迁移进 `targets` 后清空。
    #[serde(default, rename = "tolerance", skip_serializing)]
    pub legacy_tolerance: Option<u8>,
}

/// 多探针聚合模式：All = 全部命中才触发；Any = 任一命中即触发
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ColorMatchMode {
    #[default]
    All,
    Any,
}

/// 单个探针的匹配方式
/// - Average（识色1）：取框选区域平均 RGB，与目标色距离 ≤ 容差即命中
/// - AnyPixel（识色2）：框选区域内有任意一个像素与目标色距离 ≤ 容差即命中
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ColorMatchMethod {
    #[default]
    Average,
    AnyPixel,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionBootstrap {
    pub settings: RecognitionSettings,
    pub hotkey_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognition_settings_default_values_are_stable() {
        let settings = RecognitionSettings::default();
        assert!(settings.recognition_enabled);
        assert!(settings.card_groups.is_empty());
        assert!(settings.cards.is_empty());
    }

    #[test]
    fn recognition_card_deserialize_with_defaults() {
        let json = r#"{"id":"c1","name":"测试"}"#;
        let card: RecognitionCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.id, "c1");
        assert!(card.enabled);
        assert_eq!(card.trigger_mode, RecognitionTriggerMode::Hotkey);
        assert_eq!(card.volume, 0.8);
        assert_eq!(card.cooldown_ms, 1000);
        assert_eq!(card.watch_match_threshold, 0.75);
        assert!(card.group_id.is_none());
        assert_eq!(card.order, 0);
        assert!(!card.allow_simultaneous);
    }

    #[test]
    fn recognition_card_migrates_legacy_reference_image_path() {
        let card: RecognitionCard = serde_json::from_str(
            r#"{"id":"legacy","name":"旧参考图","watchReferenceImagePath":"legacy.png"}"#,
        )
        .unwrap();

        let json = serde_json::to_string(&card).unwrap();
        assert!(json.contains("\"watchReferenceImagePaths\":[\"legacy.png\"]"));
        assert!(!json.contains("\"watchReferenceImagePath\":"));
    }

    #[test]
    fn recognition_group_roundtrip_uses_camel_case() {
        let settings: RecognitionSettings = serde_json::from_str(
            r#"{"recognitionEnabled":true,"cardGroups":[{"id":"g1","name":"战斗","order":2,"collapsed":true}],"cards":[]}"#,
        )
        .unwrap();

        assert_eq!(settings.card_groups.len(), 1);
        assert_eq!(settings.card_groups[0].id, "g1");
        assert_eq!(settings.card_groups[0].order, 2);
        assert!(settings.card_groups[0].collapsed);
        assert!(settings.card_groups[0].enabled);

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"cardGroups\""));
        assert!(json.contains("\"collapsed\":true"));
        assert!(json.contains("\"enabled\":true"));
    }

    #[test]
    fn recognition_group_enabled_roundtrip() {
        let settings: RecognitionSettings = serde_json::from_str(
            r#"{"recognitionEnabled":true,"cardGroups":[{"id":"g1","name":"战斗","enabled":false}],"cards":[]}"#,
        )
        .unwrap();

        assert!(!settings.card_groups[0].enabled);
    }

    #[test]
    fn recognition_card_allow_simultaneous_roundtrip() {
        let json = r#"{"id":"c2","name":"并发播放","allowSimultaneous":true}"#;
        let card: RecognitionCard = serde_json::from_str(json).unwrap();
        assert!(card.allow_simultaneous);
        let reserialized = serde_json::to_string(&card).unwrap();
        assert!(!reserialized.contains("\"allowSimultaneous\""));
    }

    #[test]
    fn activation_trigger_count_defaults_and_roundtrips() {
        let activation: RecognitionActivation =
            serde_json::from_str(r#"{"mode":"timedHotkey","hotkey":"Alt+F1","durationMs":3000}"#)
                .unwrap();
        assert_eq!(activation.trigger_count, 1);

        let activation: RecognitionActivation = serde_json::from_str(
            r#"{"mode":"timedHotkey","hotkey":"Alt+F1","durationMs":3000,"triggerCount":10}"#,
        )
        .unwrap();
        assert_eq!(activation.trigger_count, 10);
        let json = serde_json::to_string(&activation).unwrap();
        assert!(json.contains("\"triggerCount\":10"));
    }

    #[test]
    fn hotkey_effect_migrates_legacy_hotkey_to_steps() {
        let effect: RecognitionHotkeyEffect = serde_json::from_str(r#"{"hotkey":"F2"}"#).unwrap();
        assert_eq!(effect.hotkey, "F2");
        assert_eq!(effect.steps.len(), 1);
        assert_eq!(effect.steps[0].hotkey, "F2");
        assert_eq!(effect.steps[0].delay_ms, 0);
    }

    #[test]
    fn color_probe_roundtrip() {
        // Issue #65：旧 JSON 的 targetColor/tolerance 反序列化到 legacy_* 字段；
        // 迁移到 targets 由 normalize_settings 完成（不在 serde 层）。
        let json = r#"{"region":{"x":10,"y":20,"width":5,"height":5},"targetColor":[200,100,50],"tolerance":40}"#;
        let probe: ColorProbe = serde_json::from_str(json).unwrap();
        assert_eq!(probe.region.as_ref().unwrap().x, 10);
        assert_eq!(probe.legacy_target_color, Some([200, 100, 50]));
        assert_eq!(probe.legacy_tolerance, Some(40));
        assert!(probe.targets.is_empty(), "serde 层不做迁移，targets 应为空");
        // 序列化只输出 targets（空数组），不输出 legacy 字段
        let reserialized = serde_json::to_string(&probe).unwrap();
        assert!(
            !reserialized.contains("targetColor"),
            "旧字段应 skip_serializing，实际 {reserialized}"
        );
        assert!(
            !reserialized.contains("tolerance"),
            "旧字段应 skip_serializing，实际 {reserialized}"
        );
        assert!(
            reserialized.contains("\"targets\":[]"),
            "应输出空 targets，实际 {reserialized}"
        );
    }

    #[test]
    fn color_probe_region_null_roundtrip() {
        // Issue #61: 未框选探针 region=None 应能序列化往返（前端草稿态可保存）
        let json = r#"{"region":null,"targetColor":[200,100,50],"tolerance":40}"#;
        let probe: ColorProbe = serde_json::from_str(json).unwrap();
        assert!(probe.region.is_none());
        assert_eq!(probe.legacy_target_color, Some([200, 100, 50]));
        let reserialized = serde_json::to_string(&probe).unwrap();
        assert!(
            reserialized.contains("\"region\":null"),
            "region=None 应序列化为 null，实际 {reserialized}"
        );
        let back: ColorProbe = serde_json::from_str(&reserialized).unwrap();
        assert!(back.region.is_none());
    }

    #[test]
    fn color_probe_region_omitted_defaults_to_none() {
        // 缺省 region 字段（旧/部分前端）应反序列化为 None，不报错
        let json = r#"{"targetColor":[0,0,0],"tolerance":30}"#;
        let probe: ColorProbe = serde_json::from_str(json).unwrap();
        assert!(probe.region.is_none());
        assert_eq!(probe.legacy_tolerance, Some(30));
    }

    #[test]
    fn color_probe_default_tolerance_is_30() {
        // 缺省 tolerance 应默认 30（legacy_tolerance=None，serde 不迁移，normalize 迁移）
        let json = r#"{"region":{"x":0,"y":0,"width":3,"height":3},"targetColor":[0,0,0]}"#;
        let probe: ColorProbe = serde_json::from_str(json).unwrap();
        assert_eq!(probe.legacy_tolerance, None);
        assert_eq!(probe.legacy_target_color, Some([0, 0, 0]));
    }

    #[test]
    fn color_match_mode_default_is_all() {
        // RecognitionCard 缺省 color_match_mode 应为 All
        let json = r#"{"id":"c1","name":"测试","triggerMode":"colorWatch"}"#;
        let card: RecognitionCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.color_match_mode, ColorMatchMode::All);
        assert!(card.color_probes.is_empty());
    }

    #[test]
    fn recognition_card_with_color_watch_roundtrip() {
        let card = RecognitionCard {
            id: "c1".into(),
            group_id: None,
            order: 0,
            name: "识色卡".into(),
            enabled: true,
            trigger_mode: RecognitionTriggerMode::ColorWatch,
            hotkey: None,
            watch_region: None,
            watch_reference_image_paths: Vec::new(),
            watch_match_threshold: 0.75,
            watch_poll_interval_ms: 500,
            retrigger_after_disappear: false,
            activation: RecognitionActivation::default(),
            effects: RecognitionEffects {
                audio: Some(RecognitionAudioEffect {
                    audio_files: vec!["a.mp3".into()],
                    play_mode: PlayMode::Combo,
                    combo_window_ms: 60000,
                    combo_windows: vec![],
                    volume: 0.8,
                    allow_simultaneous: false,
                }),
                hotkey: None,
                click: None,
            },
            audio_files: vec!["a.mp3".into()],
            legacy_audio_file_path: None,
            play_mode: PlayMode::Combo,
            combo_window_ms: 60000,
            combo_windows: vec![],
            volume: 0.8,
            cooldown_ms: 1000,
            allow_simultaneous: false,
            color_probes: vec![ColorProbe {
                region: Some(RegionRect {
                    x: 1,
                    y: 2,
                    width: 3,
                    height: 4,
                }),
                targets: vec![ColorTarget {
                    color: [10, 20, 30],
                    tolerance: 25,
                }],
                probe_match_mode: ColorMatchMode::Any,
                legacy_target_color: None,
                legacy_tolerance: None,
            }],
            color_match_mode: ColorMatchMode::Any,
            color_match_method: ColorMatchMethod::AnyPixel,
        };
        let json = serde_json::to_string(&card).unwrap();
        // 序列化不应输出兼容字段 audioFilePath
        assert!(!json.contains("audioFilePath"));
        assert!(json.contains("\"audioFiles\""));
        let back: RecognitionCard = serde_json::from_str(&json).unwrap();
        assert_eq!(back.trigger_mode, RecognitionTriggerMode::ColorWatch);
        assert_eq!(back.color_match_mode, ColorMatchMode::Any);
        assert_eq!(back.color_match_method, ColorMatchMethod::AnyPixel);
        assert_eq!(back.color_probes.len(), 1);
        assert_eq!(back.color_probes[0].targets.len(), 1);
        assert_eq!(back.color_probes[0].targets[0].color, [10, 20, 30]);
        assert_eq!(back.color_probes[0].targets[0].tolerance, 25);
        let effect = back.effects.audio.unwrap();
        assert_eq!(effect.audio_files, vec!["a.mp3".to_string()]);
        assert_eq!(effect.play_mode, PlayMode::Combo);
        assert_eq!(effect.combo_window_ms, 60000);
    }

    #[test]
    fn recognition_card_legacy_audio_file_path_deserialized() {
        // 旧 JSON 的 audioFilePath 单值字段应能被反序列化到 legacy_audio_file_path
        let json = r#"{"id":"c1","name":"旧卡","audioFilePath":"old.mp3"}"#;
        let card: RecognitionCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.legacy_audio_file_path, Some("old.mp3".to_string()));
        assert!(card.audio_files.is_empty());
        assert_eq!(card.play_mode, PlayMode::Single);
        assert_eq!(card.combo_window_ms, 60000);
    }

    #[test]
    fn color_match_method_default_is_average() {
        // RecognitionCard 缺省 color_match_method 应为 Average（向后兼容旧 JSON）
        let json = r#"{"id":"c1","name":"测试","triggerMode":"colorWatch"}"#;
        let card: RecognitionCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.color_match_method, ColorMatchMethod::Average);
    }

    #[test]
    fn color_match_method_anypixel_roundtrip() {
        let json = r#"{"id":"c1","name":"单像素","triggerMode":"colorWatch","colorMatchMethod":"anyPixel"}"#;
        let card: RecognitionCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.color_match_method, ColorMatchMethod::AnyPixel);
        let reserialized = serde_json::to_string(&card).unwrap();
        assert!(
            reserialized.contains("\"colorMatchMethod\":\"anyPixel\""),
            "应序列化为 camelCase anyPixel，实际 {reserialized}"
        );
    }

    #[test]
    fn color_match_method_default_value_is_average_enum() {
        assert_eq!(ColorMatchMethod::default(), ColorMatchMethod::Average);
    }

    #[test]
    fn recognition_card_retrigger_defaults_false_and_serializes_camel_case() {
        let mut card: RecognitionCard = serde_json::from_value(serde_json::json!({
            "id": "legacy",
            "name": "旧卡片"
        }))
        .expect("旧卡片应可反序列化");

        assert!(!card.retrigger_after_disappear);

        card.retrigger_after_disappear = true;
        let value = serde_json::to_value(card).expect("卡片应可序列化");
        assert_eq!(value["retriggerAfterDisappear"], true);
    }
}
