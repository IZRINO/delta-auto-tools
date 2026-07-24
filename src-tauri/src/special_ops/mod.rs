use chrono::{FixedOffset, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::overlay_utils::{destroy_window, encoded_query_value, safe_label_component};
use crate::{app_error::AppError, settings::SettingsCoordinator};

const SETTINGS_FILE_NAME: &str = "special_ops_settings.json";
pub const STATE_CHANGED: &str = "special-ops://state-changed";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StationKind {
    TechnicalCenter,
    Workbench,
    Pharmacy,
    ArmorBench,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StationStatus {
    Idle,
    Crafting,
    Ready,
    Uncertain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StationPlan {
    pub kind: StationKind,
    pub enabled: bool,
    pub item_name: String,
    pub duration_minutes: u32,
    pub started_at_ms: Option<i64>,
    pub finishes_at_ms: Option<i64>,
    pub status: StationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AccountStatus {
    Ready,
    NeedsManualLogin,
    LoginFailed,
    Uncertain,
    Isolated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountFailure {
    pub step: String,
    pub message: String,
    pub at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AmmoTarget {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub seasonal: bool,
    #[serde(default)]
    pub scroll_steps: u32,
    pub order: u32,
    pub last_success_day: Option<String>,
    pub retry_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountPlan {
    pub id: String,
    pub qq_account: String,
    pub password: String,
    pub enabled: bool,
    pub initialized: bool,
    pub order: u32,
    pub status: AccountStatus,
    pub stations: Vec<StationPlan>,
    pub ammo_targets: Vec<AmmoTarget>,
    #[serde(default)]
    pub last_failure: Option<AccountFailure>,
    #[serde(default)]
    pub login_trial_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CalibrationTargetKind {
    ClickPoint,
    InputRegion,
    RecognitionRegion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CalibrationRecognitionMethod {
    Template,
    Ocr,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationTarget {
    pub key: String,
    pub label: String,
    pub kind: CalibrationTargetKind,
    pub rect: Option<CalibrationRect>,
    #[serde(default)]
    pub reference_image_path: Option<String>,
    #[serde(default)]
    pub recognition_method: Option<CalibrationRecognitionMethod>,
    #[serde(default)]
    pub guard_any_of: Vec<String>,
    #[serde(default = "default_match_threshold")]
    pub match_threshold: f32,
    #[serde(default)]
    pub verified_signature: Option<String>,
    #[serde(default)]
    pub verified_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationTemplateTestResult {
    pub sample_similarities: [f32; 2],
    pub passed: bool,
    pub verified_at_ms: Option<i64>,
}

fn default_match_threshold() -> f32 {
    0.75
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationEnvironment {
    pub id: String,
    pub name: String,
    pub monitor: String,
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub dpi_scale: f64,
    pub window_mode: String,
    pub targets: Vec<CalibrationTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpecialOpsSettings {
    pub enabled: bool,
    pub paused: bool,
    pub daily_exchange_time: String,
    pub emergency_hotkey: String,
    #[serde(default)]
    pub wegame_executable_path: String,
    #[serde(default)]
    pub game_executable_path: String,
    pub accounts: Vec<AccountPlan>,
    #[serde(default)]
    pub active_calibration_id: Option<String>,
    #[serde(default)]
    pub calibration_environments: Vec<CalibrationEnvironment>,
}

impl Default for SpecialOpsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            paused: true,
            daily_exchange_time: "08:00".to_string(),
            emergency_hotkey: "Ctrl+Shift+F12".to_string(),
            wegame_executable_path: String::new(),
            game_executable_path: String::new(),
            accounts: Vec::new(),
            active_calibration_id: Some("default".to_string()),
            calibration_environments: vec![default_calibration_environment()],
        }
    }
}

fn default_calibration_environment() -> CalibrationEnvironment {
    CalibrationEnvironment {
        id: "default".to_string(),
        name: "默认显示环境".to_string(),
        monitor: "主显示器".to_string(),
        resolution_width: 1920,
        resolution_height: 1080,
        dpi_scale: 1.0,
        window_mode: "无边框窗口".to_string(),
        targets: default_calibration_targets(),
    }
}

fn default_guard_any_of(key: &str) -> &'static [&'static str] {
    match key {
        "wegame.account" | "wegame.password" => &["wegame.loginFormReady"],
        "craft.station.technicalCenter" => &["craft.claimReady.technicalCenter"],
        "craft.station.workbench" => &["craft.claimReady.workbench"],
        "craft.station.pharmacy" => &["craft.claimReady.pharmacy"],
        "craft.station.armorBench" => &["craft.claimReady.armorBench"],
        "craft.openRecipeList.technicalCenter" => &["craft.idle.technicalCenter"],
        "craft.openRecipeList.workbench" => &["craft.idle.workbench"],
        "craft.openRecipeList.pharmacy" => &["craft.idle.pharmacy"],
        "craft.openRecipeList.armorBench" => &["craft.idle.armorBench"],
        "craft.recipe.technicalCenter" => &["craft.recipeListReady.technicalCenter"],
        "craft.recipe.workbench" => &["craft.recipeListReady.workbench"],
        "craft.recipe.pharmacy" => &["craft.recipeListReady.pharmacy"],
        "craft.recipe.armorBench" => &["craft.recipeListReady.armorBench"],
        "ammo.target" => &["ammo.list", "ammo.seasonalList"],
        _ => &[],
    }
}

fn default_calibration_targets() -> Vec<CalibrationTarget> {
    use CalibrationTargetKind::{ClickPoint, InputRegion, RecognitionRegion};
    [
        (
            "wegame.loginMode",
            "WeGame QQ 账号密码登录入口识别与点击区域",
            RecognitionRegion,
        ),
        (
            "wegame.loginFormReady",
            "QQ 账号密码登录表单就绪区域",
            RecognitionRegion,
        ),
        ("wegame.account", "QQ 账号输入区域", InputRegion),
        ("wegame.password", "QQ 密码输入区域", InputRegion),
        (
            "wegame.login",
            "WeGame 登录按钮识别与点击区域",
            RecognitionRegion,
        ),
        (
            "wegame.gameEntry",
            "游戏启动前置界面入口识别与点击区域",
            RecognitionRegion,
        ),
        (
            "wegame.launch",
            "启动游戏按钮识别与点击区域",
            RecognitionRegion,
        ),
        (
            "game.modeReady",
            "模式选择可用状态参考区域",
            RecognitionRegion,
        ),
        (
            "game.beaconMode",
            "烽火地带入口识别与点击区域",
            RecognitionRegion,
        ),
        (
            "game.activityPopup",
            "烽火地带活动弹窗识别区域（命中按空格）",
            RecognitionRegion,
        ),
        ("game.startGame", "开始游戏识别区域", RecognitionRegion),
        ("game.specialOps", "特勤处识别与点击区域", RecognitionRegion),
        (
            "game.stationGrid",
            "四制作台页面识别区域",
            RecognitionRegion,
        ),
        (
            "craft.station.technicalCenter",
            "技术中心点击区域",
            ClickPoint,
        ),
        ("craft.station.workbench", "工作台点击区域", ClickPoint),
        ("craft.station.pharmacy", "制药台点击区域", ClickPoint),
        ("craft.station.armorBench", "防具台点击区域", ClickPoint),
        (
            "craft.claimReady.technicalCenter",
            "技术中心可收取感叹号",
            RecognitionRegion,
        ),
        (
            "craft.claimReady.workbench",
            "工作台可收取感叹号",
            RecognitionRegion,
        ),
        (
            "craft.claimReady.pharmacy",
            "制药台可收取感叹号",
            RecognitionRegion,
        ),
        (
            "craft.claimReady.armorBench",
            "防具台可收取感叹号",
            RecognitionRegion,
        ),
        ("craft.reward", "获得奖励页面识别区域", RecognitionRegion),
        (
            "craft.idle.technicalCenter",
            "技术中心空闲中文字识别区域",
            RecognitionRegion,
        ),
        (
            "craft.idle.workbench",
            "工作台空闲中文字识别区域",
            RecognitionRegion,
        ),
        (
            "craft.idle.pharmacy",
            "制药台空闲中文字识别区域",
            RecognitionRegion,
        ),
        (
            "craft.idle.armorBench",
            "防具台空闲中文字识别区域",
            RecognitionRegion,
        ),
        (
            "craft.openRecipeList.technicalCenter",
            "技术中心进入制作列表点击区域",
            ClickPoint,
        ),
        (
            "craft.openRecipeList.workbench",
            "工作台进入制作列表点击区域",
            ClickPoint,
        ),
        (
            "craft.openRecipeList.pharmacy",
            "制药台进入制作列表点击区域",
            ClickPoint,
        ),
        (
            "craft.openRecipeList.armorBench",
            "防具台进入制作列表点击区域",
            ClickPoint,
        ),
        (
            "craft.recipeListReady.technicalCenter",
            "技术中心制作列表就绪区域",
            RecognitionRegion,
        ),
        (
            "craft.recipeListReady.workbench",
            "工作台制作列表就绪区域",
            RecognitionRegion,
        ),
        (
            "craft.recipeListReady.pharmacy",
            "制药台制作列表就绪区域",
            RecognitionRegion,
        ),
        (
            "craft.recipeListReady.armorBench",
            "防具台制作列表就绪区域",
            RecognitionRegion,
        ),
        (
            "craft.recipe.technicalCenter",
            "技术中心置顶配方点击区域",
            ClickPoint,
        ),
        (
            "craft.recipe.workbench",
            "工作台置顶配方点击区域",
            ClickPoint,
        ),
        (
            "craft.recipe.pharmacy",
            "制药台置顶配方点击区域",
            ClickPoint,
        ),
        (
            "craft.recipe.armorBench",
            "防具台置顶配方点击区域",
            ClickPoint,
        ),
        ("craft.fill", "一键补齐识别与点击区域", RecognitionRegion),
        (
            "craft.purchase",
            "购买材料按钮识别与点击区域",
            RecognitionRegion,
        ),
        ("craft.produce", "生产按钮识别与点击区域", RecognitionRegion),
        ("craft.abort", "中止按钮识别区域", RecognitionRegion),
        (
            "ammo.department",
            "部门入口识别与点击区域",
            RecognitionRegion,
        ),
        ("ammo.supply", "军需处入口识别与点击区域", RecognitionRegion),
        (
            "ammo.tactical",
            "战术部门入口识别与点击区域",
            RecognitionRegion,
        ),
        ("ammo.list", "子弹兑换列表区域", RecognitionRegion),
        (
            "ammo.seasonal",
            "赛季限定入口识别与点击区域",
            RecognitionRegion,
        ),
        (
            "ammo.seasonalList",
            "赛季限定子弹列表就绪区域",
            RecognitionRegion,
        ),
        ("ammo.target", "目标子弹点击区域", ClickPoint),
        (
            "ammo.selectedTargetName",
            "已选目标子弹名称 OCR 区域",
            RecognitionRegion,
        ),
        ("ammo.fill", "子弹一键补齐区域", RecognitionRegion),
        (
            "ammo.purchase",
            "子弹材料购买按钮识别与点击区域",
            RecognitionRegion,
        ),
        ("ammo.exchange", "兑换按钮区域", RecognitionRegion),
        ("ammo.success", "兑换成功灰色按钮区域", RecognitionRegion),
    ]
    .into_iter()
    .map(|(key, label, kind)| {
        let recognition_method = match (&kind, key) {
            (RecognitionRegion, "ammo.selectedTargetName") => {
                Some(CalibrationRecognitionMethod::Ocr)
            }
            (RecognitionRegion, _) => Some(CalibrationRecognitionMethod::Template),
            _ => None,
        };
        CalibrationTarget {
            key: key.to_string(),
            label: label.to_string(),
            kind,
            rect: None,
            reference_image_path: None,
            recognition_method,
            guard_any_of: default_guard_any_of(key)
                .iter()
                .map(|guard| (*guard).to_string())
                .collect(),
            match_threshold: default_match_threshold(),
            verified_signature: None,
            verified_at_ms: None,
        }
    })
    .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DueAccount {
    pub account_id: String,
    pub station_kinds: Vec<StationKind>,
    pub ammo_target_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleSnapshot {
    pub due_accounts: Vec<DueAccount>,
    pub next_wake_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpecialOpsBootstrap {
    pub settings: SpecialOpsSettings,
    pub schedule: ScheduleSnapshot,
    pub settings_revision: u64,
    pub now_ms: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpecialOpsStateChanged {
    pub settings_revision: u64,
    pub now_ms: i64,
}

impl From<&SpecialOpsBootstrap> for SpecialOpsStateChanged {
    fn from(bootstrap: &SpecialOpsBootstrap) -> Self {
        Self {
            settings_revision: bootstrap.settings_revision,
            now_ms: bootstrap.now_ms,
        }
    }
}

pub struct SpecialOpsState {
    settings: Arc<Mutex<SpecialOpsSettings>>,
}

impl StationKind {
    fn all() -> [Self; 4] {
        [
            Self::TechnicalCenter,
            Self::Workbench,
            Self::Pharmacy,
            Self::ArmorBench,
        ]
    }

    fn calibration_suffix(&self) -> &'static str {
        match self {
            Self::TechnicalCenter => "technicalCenter",
            Self::Workbench => "workbench",
            Self::Pharmacy => "pharmacy",
            Self::ArmorBench => "armorBench",
        }
    }
}

impl StationStatus {
    fn default_for_new() -> Self {
        Self::Idle
    }
}

impl StationPlan {
    fn default_for(kind: StationKind) -> Self {
        Self {
            kind,
            enabled: false,
            item_name: String::new(),
            duration_minutes: 240,
            started_at_ms: None,
            finishes_at_ms: None,
            status: StationStatus::default_for_new(),
        }
    }
}

fn template_test_passed(samples: [f32; 2], threshold: f32) -> bool {
    threshold.is_finite()
        && (0.0..=1.0).contains(&threshold)
        && samples
            .into_iter()
            .all(|sample| sample.is_finite() && sample >= threshold)
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn calibration_signature(target: &CalibrationTarget) -> Result<String, String> {
    let rect = target
        .rect
        .as_ref()
        .ok_or_else(|| format!("{} 尚未框选", target.label))?;
    let reference_path = target
        .reference_image_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| format!("{} 尚未上传参考图", target.label))?;
    let canonical_path = std::fs::canonicalize(reference_path)
        .map_err(|_| format!("{} 的参考图文件不存在", target.label))?;
    let metadata = std::fs::metadata(&canonical_path)
        .map_err(|_| format!("{} 的参考图文件不存在", target.label))?;
    if !metadata.is_file() {
        return Err(format!("{} 的参考图文件不存在", target.label));
    }
    let reference_bytes = std::fs::read(&canonical_path)
        .map_err(|error| format!("读取 {} 的参考图失败: {error}", target.label))?;
    let content_hash = fnv1a_64(&reference_bytes);
    let modified_ms = metadata
        .modified()
        .map_err(|error| format!("读取 {} 的参考图修改时间失败: {error}", target.label))?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("{} 的参考图修改时间无效: {error}", target.label))?
        .as_millis();
    Ok(format!(
        "v2|{}|{},{},{},{}|{}|{}|{}|{}|{content_hash:016x}",
        target.key,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        canonical_path.display(),
        metadata.len(),
        modified_ms,
        target.match_threshold
    ))
}

fn verification_is_current(target: &CalibrationTarget) -> bool {
    let (Some(stored_signature), Some(verified_at_ms)) =
        (&target.verified_signature, target.verified_at_ms)
    else {
        return false;
    };
    verified_at_ms >= 0
        && calibration_signature(target)
            .is_ok_and(|current_signature| current_signature == *stored_signature)
}

fn normalize_settings(mut settings: SpecialOpsSettings) -> Result<SpecialOpsSettings, String> {
    if daily_exchange_minutes(&settings.daily_exchange_time).is_none() {
        return Err("每日兑换时间必须是 HH:mm，范围 00:00-23:59".to_string());
    }
    if settings.emergency_hotkey.trim().is_empty() {
        return Err("紧急停止快捷键不能为空".to_string());
    }

    if settings.calibration_environments.is_empty() {
        settings
            .calibration_environments
            .push(default_calibration_environment());
    }
    if let Some(active_id) = settings.active_calibration_id.as_deref() {
        if let Some(index) = settings
            .calibration_environments
            .iter()
            .position(|item| item.id == active_id)
        {
            settings.calibration_environments.swap(0, index);
        }
    }
    settings.calibration_environments.truncate(1);
    let required_targets = default_calibration_targets();
    let mut environment_ids = std::collections::HashSet::new();
    for environment in &mut settings.calibration_environments {
        environment.id = environment.id.trim().to_string();
        environment.name = environment.name.trim().to_string();
        if environment.id.is_empty() || !environment_ids.insert(environment.id.clone()) {
            return Err("显示环境 ID 必须非空且唯一".to_string());
        }
        if environment.name.is_empty()
            || environment.resolution_width == 0
            || environment.resolution_height == 0
            || !environment.dpi_scale.is_finite()
            || environment.dpi_scale <= 0.0
        {
            return Err(format!("显示环境 {} 配置无效", environment.id));
        }
        let mut existing_targets = environment
            .targets
            .drain(..)
            .map(|target| (target.key.clone(), target))
            .collect::<std::collections::HashMap<_, _>>();
        environment.targets = required_targets
            .iter()
            .map(|required| {
                let mut target = existing_targets
                    .remove(&required.key)
                    .unwrap_or_else(|| required.clone());
                target.label = required.label.clone();
                target.kind = required.kind.clone();
                target.recognition_method = required.recognition_method.clone();
                target.guard_any_of = required.guard_any_of.clone();
                if target.recognition_method == Some(CalibrationRecognitionMethod::Template) {
                    if !target.match_threshold.is_finite()
                        || !(0.0..=1.0).contains(&target.match_threshold)
                    {
                        return Err(format!("{} 的模板匹配阈值必须在 0 到 1 之间", target.label));
                    }
                    if !verification_is_current(&target) {
                        target.verified_signature = None;
                        target.verified_at_ms = None;
                    }
                } else {
                    target.reference_image_path = None;
                    target.verified_signature = None;
                    target.verified_at_ms = None;
                }
                Ok(target)
            })
            .collect::<Result<Vec<_>, String>>()?;
    }
    if settings
        .active_calibration_id
        .as_ref()
        .is_none_or(|id| !environment_ids.contains(id))
    {
        settings.active_calibration_id = settings
            .calibration_environments
            .first()
            .map(|item| item.id.clone());
    }

    let mut ids = std::collections::HashSet::new();
    let mut enabled_qq_accounts = std::collections::HashSet::new();
    for (index, account) in settings.accounts.iter_mut().enumerate() {
        account.id = account.id.trim().to_string();
        account.qq_account = account.qq_account.trim().to_string();
        if account.id.is_empty() || !ids.insert(account.id.clone()) {
            return Err("账号 ID 必须非空且唯一".to_string());
        }
        if account.enabled
            && !account.qq_account.is_empty()
            && !enabled_qq_accounts.insert(account.qq_account.clone())
        {
            return Err("启用账号的 QQ 账号必须唯一".to_string());
        }
        account.order = index as u32;
        if account.stations.len() > 4 {
            return Err(format!("账号 {} 的制作台数量不能超过 4 个", account.id));
        }
        // 允许先启用制作台再填写物品/时长，配置页需要保存未完成草稿。
        // 调度只处理存在完成时间的制作任务，真正执行前再做完整配置校验。
        let mut stations = StationKind::all()
            .into_iter()
            .map(StationPlan::default_for)
            .collect::<Vec<_>>();
        for configured in account.stations.drain(..) {
            if let Some(target) = stations
                .iter_mut()
                .find(|station| station.kind == configured.kind)
            {
                *target = configured;
            }
        }
        account.stations = stations;
        account.ammo_targets.sort_by_key(|target| target.order);
        let mut ammo_ids = std::collections::HashSet::new();
        for (order, target) in account.ammo_targets.iter_mut().enumerate() {
            target.id = target.id.trim().to_string();
            target.name = target.name.trim().to_string();
            if target.id.is_empty() || !ammo_ids.insert(target.id.clone()) {
                return Err(format!("账号 {} 的子弹目标 ID 必须非空且唯一", account.id));
            }
            target.order = order as u32;
        }
    }
    Ok(settings)
}

fn required_execution_target_keys(
    settings: &SpecialOpsSettings,
) -> std::collections::HashSet<String> {
    let active_accounts = settings
        .accounts
        .iter()
        .filter(|account| account.enabled && account.status == AccountStatus::Ready)
        .filter(|account| {
            account.stations.iter().any(|station| station.enabled)
                || account.ammo_targets.iter().any(|target| target.enabled)
        })
        .collect::<Vec<_>>();
    if active_accounts.is_empty() {
        return std::collections::HashSet::new();
    }

    let mut keys = [
        "wegame.loginMode",
        "wegame.loginFormReady",
        "wegame.account",
        "wegame.password",
        "wegame.login",
        "wegame.gameEntry",
        "wegame.launch",
        "game.modeReady",
        "game.beaconMode",
        "game.activityPopup",
        "game.startGame",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<std::collections::HashSet<_>>();

    let has_crafting = active_accounts
        .iter()
        .any(|account| account.stations.iter().any(|station| station.enabled));
    if has_crafting {
        keys.extend(
            [
                "game.specialOps",
                "game.stationGrid",
                "craft.reward",
                "craft.fill",
                "craft.purchase",
                "craft.produce",
                "craft.abort",
            ]
            .into_iter()
            .map(str::to_string),
        );
        for kind in StationKind::all() {
            if active_accounts.iter().any(|account| {
                account
                    .stations
                    .iter()
                    .any(|station| station.enabled && station.kind == kind)
            }) {
                let suffix = kind.calibration_suffix();
                for prefix in [
                    "craft.station",
                    "craft.claimReady",
                    "craft.idle",
                    "craft.openRecipeList",
                    "craft.recipeListReady",
                    "craft.recipe",
                ] {
                    keys.insert(format!("{prefix}.{suffix}"));
                }
            }
        }
    }

    let has_ammo = active_accounts
        .iter()
        .any(|account| account.ammo_targets.iter().any(|target| target.enabled));
    if has_ammo {
        keys.extend(
            [
                "ammo.department",
                "ammo.supply",
                "ammo.tactical",
                "ammo.list",
                "ammo.target",
                "ammo.selectedTargetName",
                "ammo.fill",
                "ammo.purchase",
                "ammo.exchange",
                "ammo.success",
            ]
            .into_iter()
            .map(str::to_string),
        );
        if active_accounts.iter().any(|account| {
            account
                .ammo_targets
                .iter()
                .any(|target| target.enabled && target.seasonal)
        }) {
            keys.insert("ammo.seasonal".to_string());
            keys.insert("ammo.seasonalList".to_string());
        }
    }
    keys
}

fn validate_execution_ready(settings: &SpecialOpsSettings) -> Result<(), String> {
    let required_keys = required_execution_target_keys(settings);
    if required_keys.is_empty() {
        return Ok(());
    }
    for account in settings
        .accounts
        .iter()
        .filter(|account| account.enabled && account.status == AccountStatus::Ready)
        .filter(|account| {
            account.stations.iter().any(|station| station.enabled)
                || account.ammo_targets.iter().any(|target| target.enabled)
        })
    {
        if account.qq_account.trim().is_empty() || account.password.is_empty() {
            return Err(format!("账号 {} 的 QQ 与密码必须完整", account.id));
        }
        for station in account.stations.iter().filter(|station| station.enabled) {
            if station.item_name.trim().is_empty()
                || !(1..=168 * 60).contains(&station.duration_minutes)
            {
                return Err(format!("账号 {} 的制作台配置不完整", account.id));
            }
        }
        if account
            .ammo_targets
            .iter()
            .any(|target| target.enabled && target.name.trim().is_empty())
        {
            return Err(format!("账号 {} 存在未命名的子弹目标", account.id));
        }
    }
    let environment = settings
        .calibration_environments
        .first()
        .ok_or_else(|| "校准未完成：缺少显示环境".to_string())?;
    for required in default_calibration_targets()
        .into_iter()
        .filter(|target| required_keys.contains(&target.key))
    {
        let target = environment
            .targets
            .iter()
            .find(|target| target.key == required.key)
            .ok_or_else(|| format!("校准未完成：缺少步骤 {}", required.label))?;
        if target.rect.is_none() {
            return Err(format!("校准未完成：{} 尚未框选", target.label));
        }
        if target.recognition_method == Some(CalibrationRecognitionMethod::Template) {
            let path = target
                .reference_image_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| format!("校准未完成：{} 尚未上传参考图", target.label))?;
            if !std::path::Path::new(path).is_file() {
                return Err(format!("校准未完成：{} 的参考图文件不存在", target.label));
            }
        }
    }
    Ok(())
}

// 登录试运行 command 在后续任务接入。
#[allow(dead_code)]
fn validate_login_trial_ready(
    settings: &SpecialOpsSettings,
    account_id: &str,
) -> Result<(), String> {
    let validate_executable_path = |path: &str, label: &str| {
        let path = path.trim();
        if path.is_empty() {
            return Err(format!("{label} 路径不能为空"));
        }
        let path = std::path::Path::new(path);
        if !path.is_absolute() {
            return Err(format!("{label} 路径必须是绝对路径"));
        }
        if !path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            return Err(format!("{label} 路径必须指向 .exe 文件"));
        }
        if !path.is_file() {
            return Err(format!("{label} 文件不存在"));
        }
        Ok(())
    };
    let account = settings
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .ok_or_else(|| format!("登录试运行账号 {account_id} 不存在"))?;
    if !account.enabled {
        return Err(format!("登录试运行账号 {account_id} 未启用"));
    }
    if account.qq_account.trim().is_empty() {
        return Err(format!("登录试运行账号 {account_id} 的 QQ 不能为空"));
    }
    if account.password.is_empty() {
        return Err(format!("登录试运行账号 {account_id} 的密码不能为空"));
    }
    validate_executable_path(&settings.wegame_executable_path, "WeGame.exe")?;
    validate_executable_path(&settings.game_executable_path, "游戏 .exe")?;

    let environment = settings
        .calibration_environments
        .first()
        .ok_or_else(|| "登录试运行校准未完成：缺少显示环境".to_string())?;
    let required_targets = default_calibration_targets();
    for key in [
        "wegame.loginMode",
        "wegame.loginFormReady",
        "wegame.account",
        "wegame.password",
        "wegame.login",
        "wegame.gameEntry",
        "wegame.launch",
    ] {
        let required = required_targets
            .iter()
            .find(|target| target.key == key)
            .expect("默认登录校准目标必须存在");
        let target = environment
            .targets
            .iter()
            .find(|target| target.key == key)
            .ok_or_else(|| format!("登录试运行校准未完成：缺少步骤 {}", required.label))?;
        if target.rect.is_none() {
            return Err(format!("登录试运行校准未完成：{} 尚未框选", target.label));
        }
        if required.recognition_method == Some(CalibrationRecognitionMethod::Template) {
            let reference_path = target
                .reference_image_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| format!("登录试运行校准未完成：{} 尚未上传参考图", target.label))?;
            if !std::path::Path::new(reference_path).is_file() {
                return Err(format!(
                    "登录试运行校准未完成：{} 的参考图文件不存在",
                    target.label
                ));
            }
            if !verification_is_current(target) {
                return Err(format!(
                    "登录试运行校准未完成：{} 尚未测试或验证失效",
                    target.label
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct CalibrationTemplateTestInput {
    region: crate::morse::types::RegionRect,
    reference_image_path: String,
    match_threshold: f32,
    calibration_signature: String,
}

fn commit_calibration_test_verification(
    settings: &mut SpecialOpsSettings,
    environment_id: &str,
    target_key: &str,
    tested_signature: &str,
    passed: bool,
    verified_at_ms: Option<i64>,
    persist: impl FnOnce(&SpecialOpsSettings) -> Result<(), String>,
) -> Result<(), String> {
    let mut next = settings.clone();
    let target = next
        .calibration_environments
        .iter_mut()
        .find(|environment| environment.id == environment_id)
        .and_then(|environment| {
            environment
                .targets
                .iter_mut()
                .find(|target| target.key == target_key)
        })
        .ok_or_else(|| "校准配置已变化，请重新测试".to_string())?;
    let current_signature =
        calibration_signature(target).map_err(|_| "校准配置已变化，请重新测试".to_string())?;
    if current_signature != tested_signature {
        return Err("校准配置已变化，请重新测试".to_string());
    }
    if passed {
        let verified_at_ms = verified_at_ms
            .filter(|value| *value >= 0)
            .ok_or_else(|| "校准验证时间无效".to_string())?;
        target.verified_signature = Some(current_signature);
        target.verified_at_ms = Some(verified_at_ms);
    } else {
        target.verified_signature = None;
        target.verified_at_ms = None;
    }
    persist(&next)?;
    *settings = next;
    Ok(())
}

fn calibration_template_test_input(
    settings: &SpecialOpsSettings,
    environment_id: &str,
    target_key: &str,
) -> Result<CalibrationTemplateTestInput, String> {
    let target = settings
        .calibration_environments
        .iter()
        .find(|environment| environment.id == environment_id)
        .and_then(|environment| {
            environment
                .targets
                .iter()
                .find(|target| target.key == target_key)
        })
        .ok_or_else(|| "校准目标不存在".to_string())?;
    if target.recognition_method != Some(CalibrationRecognitionMethod::Template) {
        return Err(
            if target.recognition_method == Some(CalibrationRecognitionMethod::Ocr) {
                "OCR 测试尚未接入，不能伪造识别结果".to_string()
            } else {
                "点击点和输入区域不支持模板测试".to_string()
            },
        );
    }
    let rect = target
        .rect
        .as_ref()
        .ok_or_else(|| format!("{} 尚未框选", target.label))?;
    let reference_image_path = target
        .reference_image_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| format!("{} 尚未上传参考图", target.label))?;
    Ok(CalibrationTemplateTestInput {
        region: crate::morse::types::RegionRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        },
        reference_image_path: reference_image_path.to_string(),
        match_threshold: target.match_threshold,
        calibration_signature: calibration_signature(target)?,
    })
}

async fn sample_template_similarity(
    region: crate::morse::types::RegionRect,
    reference_image_path: String,
) -> Result<f32, String> {
    tokio::task::spawn_blocking(move || {
        let captured = crate::recognition::watcher::capture_region(&region)
            .ok_or_else(|| "截取校准区域失败".to_string())?;
        let reference = crate::recognition::watcher::load_reference_image(&reference_image_path)
            .ok_or_else(|| "无法读取参考图".to_string())?;
        let (_, result) =
            crate::recognition::watcher::best_reference_match(&captured, [&reference])
                .ok_or_else(|| "模板匹配失败".to_string())?;
        Ok(result.similarity)
    })
    .await
    .map_err(|error| format!("模板测试任务失败: {error}"))?
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn settings_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    crate::settings::settings_path(app, SETTINGS_FILE_NAME)
}

fn load_settings(app: &AppHandle) -> Result<SpecialOpsSettings, String> {
    let path = settings_path(app)?;
    let settings = crate::settings::load_settings(&path)?;
    normalize_settings(settings)
}

fn save_settings(app: &AppHandle, settings: &SpecialOpsSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    crate::settings::save_settings(&path, settings)
}

fn build_bootstrap(
    settings: SpecialOpsSettings,
    settings_revision: u64,
    current_ms: i64,
) -> SpecialOpsBootstrap {
    SpecialOpsBootstrap {
        schedule: build_schedule(&settings, current_ms),
        settings,
        settings_revision,
        now_ms: current_ms,
    }
}

fn emit_state(app: &AppHandle, bootstrap: &SpecialOpsBootstrap) {
    let _ = app.emit_to(
        "main",
        STATE_CHANGED,
        SpecialOpsStateChanged::from(bootstrap),
    );
}

pub fn initialize(app: &AppHandle) -> Result<SpecialOpsState, String> {
    let settings = load_settings(app)?;
    Ok(SpecialOpsState {
        settings: Arc::new(Mutex::new(settings)),
    })
}

pub(crate) fn stop_registered(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<SpecialOpsState>() else {
        return Ok(());
    };
    let settings = {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?;
        settings.paused = true;
        settings.clone()
    };
    save_settings(app, &settings)?;
    if let Some(coordinator) = app.try_state::<Arc<SettingsCoordinator>>() {
        let revision = coordinator.current_revision()?;
        emit_state(app, &build_bootstrap(settings, revision, now_ms()));
    }
    Ok(())
}

#[tauri::command]
pub fn special_ops_get_bootstrap(
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
) -> Result<SpecialOpsBootstrap, AppError> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "特勤处状态已损坏".to_string())?
        .clone();
    Ok(build_bootstrap(
        settings,
        settings_coordinator.current_revision()?,
        now_ms(),
    ))
}

#[tauri::command]
pub fn special_ops_save_settings(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    settings_value: SpecialOpsSettings,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    let settings_value = normalize_settings(settings_value)?;
    if settings_value.enabled && !settings_value.paused {
        validate_execution_ready(&settings_value)?;
    }
    settings_coordinator
        .with_revision(
            settings_revision,
            || -> Result<SpecialOpsBootstrap, String> {
                save_settings(&app, &settings_value)?;
                {
                    let mut settings = state
                        .settings
                        .lock()
                        .map_err(|_| "特勤处状态已损坏".to_string())?;
                    *settings = settings_value.clone();
                }
                let bootstrap = build_bootstrap(settings_value, settings_revision, now_ms());
                emit_state(&app, &bootstrap);
                Ok(bootstrap)
            },
        )
        .map_err(AppError::from)
}

#[tauri::command]
pub fn special_ops_set_paused(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    paused: bool,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    settings_coordinator
        .with_revision(
            settings_revision,
            || -> Result<SpecialOpsBootstrap, String> {
                if !paused {
                    let settings = state
                        .settings
                        .lock()
                        .map_err(|_| "特勤处状态已损坏".to_string())?;
                    if settings.enabled {
                        validate_execution_ready(&settings)?;
                    }
                }
                let settings = {
                    let mut settings = state
                        .settings
                        .lock()
                        .map_err(|_| "特勤处状态已损坏".to_string())?;
                    settings.paused = paused;
                    settings.clone()
                };
                save_settings(&app, &settings)?;
                let bootstrap = build_bootstrap(settings, settings_revision, now_ms());
                emit_state(&app, &bootstrap);
                Ok(bootstrap)
            },
        )
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn special_ops_test_calibration_target(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    environment_id: String,
    target_key: String,
    settings_revision: u64,
) -> Result<CalibrationTemplateTestResult, AppError> {
    let input = {
        let settings = state
            .settings
            .lock()
            .map_err(|_| AppError::from("特勤处状态已损坏"))?;
        calibration_template_test_input(&settings, &environment_id, &target_key)?
    };
    let first =
        sample_template_similarity(input.region.clone(), input.reference_image_path.clone())
            .await?;
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    let second =
        sample_template_similarity(input.region.clone(), input.reference_image_path.clone())
            .await?;
    let sample_similarities = [first, second];
    let passed = template_test_passed(sample_similarities, input.match_threshold);
    let verified_at_ms = passed.then(now_ms);
    settings_coordinator
        .with_revision(settings_revision, || -> Result<(), String> {
            let mut settings = state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?;
            commit_calibration_test_verification(
                &mut settings,
                &environment_id,
                &target_key,
                &input.calibration_signature,
                passed,
                verified_at_ms,
                |next| save_settings(&app, next),
            )?;
            let bootstrap = build_bootstrap(settings.clone(), settings_revision, now_ms());
            drop(settings);
            emit_state(&app, &bootstrap);
            Ok(())
        })
        .map_err(AppError::from)?;
    Ok(CalibrationTemplateTestResult {
        sample_similarities,
        passed,
        verified_at_ms,
    })
}

#[tauri::command]
pub async fn special_ops_begin_calibration_selection(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    environment_id: String,
    target_key: String,
    settings_revision: u64,
) -> Result<(), AppError> {
    {
        let settings = state
            .settings
            .lock()
            .map_err(|_| AppError::from("特勤处状态已损坏"))?;
        settings
            .calibration_environments
            .iter()
            .find(|item| item.id == environment_id)
            .and_then(|environment| {
                environment
                    .targets
                    .iter()
                    .find(|item| item.key == target_key)
            })
            .ok_or_else(|| AppError::from("校准目标不存在"))?;
    }
    let label = format!(
        "special-ops-calibration-{}-{}",
        safe_label_component(&environment_id),
        safe_label_component(&target_key)
    );
    destroy_window(&app, &label);
    let url = format!(
        "index.html?mode=special-ops-calibration&environment_id={}&target_key={}&settings_revision={}",
        encoded_query_value(&environment_id),
        encoded_query_value(&target_key),
        settings_revision
    );
    let load_app = app.clone();
    let load_label = label.clone();
    let window = tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App(url.into()))
        .title("特勤处校准")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(true)
        .resizable(false)
        .on_page_load(move |window, payload| {
            if !matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                return;
            }
            let ready_app = load_app.clone();
            let ready_label = load_label.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Err(error) = window.set_fullscreen(true) {
                    crate::log_error!(
                        "special_ops::calibration",
                        "校准窗口进入全屏失败",
                        "error" => error.to_string()
                    );
                    destroy_window(&ready_app, &ready_label);
                    restore_main_window(&ready_app);
                    return;
                }
                let _ = window.show();
                let _ = window.set_focus();
            });
        })
        .build()
        .map_err(|error| {
            restore_main_window(&app);
            AppError::from(format!("创建特勤处校准窗口失败: {error}"))
        })?;
    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::Destroyed | tauri::WindowEvent::CloseRequested { .. }
        ) {
            restore_main_window(&close_app);
        }
    });
    let timeout_app = app.clone();
    let timeout_label = label.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(30));
        if timeout_app.get_webview_window(&timeout_label).is_some() {
            destroy_window(&timeout_app, &timeout_label);
            restore_main_window(&timeout_app);
        }
    });
    Ok(())
}

#[tauri::command]
pub fn special_ops_submit_calibration_selection(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    environment_id: String,
    target_key: String,
    region: CalibrationRect,
    settings_revision: u64,
) -> Result<(), AppError> {
    settings_coordinator
        .with_revision(settings_revision, || -> Result<(), String> {
            if region.width <= 2 || region.height <= 2 {
                return Err("校准区域太小".to_string());
            }
            let settings = {
                let settings = state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?;
                let mut next = settings.clone();
                let environment = next
                    .calibration_environments
                    .iter_mut()
                    .find(|item| item.id == environment_id)
                    .ok_or_else(|| "显示环境不存在".to_string())?;
                let target = environment
                    .targets
                    .iter_mut()
                    .find(|item| item.key == target_key)
                    .ok_or_else(|| "校准目标不存在".to_string())?;
                target.rect = Some(region);
                target.verified_signature = None;
                target.verified_at_ms = None;
                next
            };
            save_settings(&app, &settings)?;
            *state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())? = settings.clone();
            emit_state(
                &app,
                &build_bootstrap(settings, settings_revision, now_ms()),
            );
            let label = format!(
                "special-ops-calibration-{}-{}",
                safe_label_component(&environment_id),
                safe_label_component(&target_key)
            );
            destroy_window(&app, &label);
            restore_main_window(&app);
            Ok(())
        })
        .map_err(AppError::from)
}

#[tauri::command]
pub fn special_ops_cancel_calibration_selection(
    app: AppHandle,
    environment_id: String,
    target_key: String,
) -> Result<(), AppError> {
    let label = format!(
        "special-ops-calibration-{}-{}",
        safe_label_component(&environment_id),
        safe_label_component(&target_key)
    );
    destroy_window(&app, &label);
    restore_main_window(&app);
    Ok(())
}

fn restore_main_window(app: &AppHandle) {
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.show();
        let _ = main_window.set_focus();
    }
}

fn local_day_and_minute(now_ms: i64) -> (String, u32) {
    let offset = FixedOffset::east_opt(8 * 60 * 60).expect("固定东八区偏移有效");
    let local = chrono::DateTime::<Utc>::from_timestamp_millis(now_ms)
        .unwrap_or_else(|| {
            Utc.timestamp_millis_opt(0)
                .single()
                .expect("Unix epoch 有效")
        })
        .with_timezone(&offset);
    (
        local.format("%Y-%m-%d").to_string(),
        local.hour() * 60 + local.minute(),
    )
}

fn daily_exchange_minutes(value: &str) -> Option<u32> {
    let bytes = value.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || !bytes[..2].iter().all(u8::is_ascii_digit)
        || !bytes[3..].iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let (hours, minutes) = value.split_once(':')?;
    let hours = hours.parse::<u32>().ok()?;
    let minutes = minutes.parse::<u32>().ok()?;
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

fn daily_exchange_at_ms(now_ms: i64, exchange_minute: u32) -> Option<i64> {
    let offset = FixedOffset::east_opt(8 * 60 * 60)?;
    let local = chrono::DateTime::<Utc>::from_timestamp_millis(now_ms)?.with_timezone(&offset);
    let date = local.date_naive();
    let candidate = date
        .and_hms_opt(exchange_minute / 60, exchange_minute % 60, 0)?
        .and_local_timezone(offset)
        .single()?
        .timestamp_millis();
    Some(candidate)
}

pub fn build_schedule(settings: &SpecialOpsSettings, now_ms: i64) -> ScheduleSnapshot {
    if !settings.enabled || settings.paused {
        return ScheduleSnapshot {
            due_accounts: Vec::new(),
            next_wake_at_ms: None,
        };
    }

    let mut accounts = settings.accounts.iter().collect::<Vec<_>>();
    accounts.sort_by_key(|account| account.order);

    let exchange_minute = daily_exchange_minutes(&settings.daily_exchange_time);
    let today = local_day_and_minute(now_ms).0;
    let exchange_at_ms = exchange_minute.and_then(|minute| daily_exchange_at_ms(now_ms, minute));
    let mut due_accounts = Vec::new();
    let mut next_wake_at_ms = None;
    for account in accounts {
        if !account.enabled || !account.initialized || account.status != AccountStatus::Ready {
            continue;
        }

        let mut station_kinds = Vec::new();
        for station in &account.stations {
            if !station.enabled || station.status != StationStatus::Crafting {
                continue;
            }
            let Some(finishes_at_ms) = station.finishes_at_ms else {
                continue;
            };
            if finishes_at_ms <= now_ms {
                let defer_to_exchange = exchange_at_ms
                    .filter(|exchange_at| *exchange_at > now_ms)
                    .is_some_and(|exchange_at| exchange_at - now_ms <= 5 * 60 * 1000);
                if defer_to_exchange {
                    next_wake_at_ms = exchange_at_ms;
                } else {
                    station_kinds.push(station.kind.clone());
                }
            } else {
                next_wake_at_ms = Some(
                    next_wake_at_ms
                        .map_or(finishes_at_ms, |current: i64| current.min(finishes_at_ms)),
                );
            }
        }

        let mut ammo_target_ids = account
            .ammo_targets
            .iter()
            .filter(|target| target.enabled)
            .filter(|target| target.last_success_day.as_deref() != Some(today.as_str()))
            .filter(|_| {
                exchange_minute.is_some_and(|minute| local_day_and_minute(now_ms).1 >= minute)
            })
            .map(|target| (target.order, target.id.clone()))
            .collect::<Vec<_>>();
        ammo_target_ids.sort_by_key(|(order, _)| *order);
        let ammo_target_ids = ammo_target_ids
            .into_iter()
            .map(|(_, id)| id)
            .collect::<Vec<_>>();

        if station_kinds.is_empty() && ammo_target_ids.is_empty() {
            if let Some(exchange_at) = exchange_at_ms.filter(|exchange_at| *exchange_at > now_ms) {
                let has_pending_ammo = account.ammo_targets.iter().any(|target| {
                    target.enabled && target.last_success_day.as_deref() != Some(today.as_str())
                });
                if has_pending_ammo {
                    next_wake_at_ms = Some(
                        next_wake_at_ms.map_or(exchange_at, |current| current.min(exchange_at)),
                    );
                }
            }
        }
        if !station_kinds.is_empty() || !ammo_target_ids.is_empty() {
            due_accounts.push(DueAccount {
                account_id: account.id.clone(),
                station_kinds,
                ammo_target_ids,
            });
        }
    }

    ScheduleSnapshot {
        due_accounts,
        next_wake_at_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct LoginFixture {
        settings: SpecialOpsSettings,
        _exe_files: Vec<tempfile::NamedTempFile>,
        _reference_files: Vec<tempfile::NamedTempFile>,
    }

    impl LoginFixture {
        fn complete() -> Self {
            let wegame_exe = tempfile::Builder::new().suffix(".exe").tempfile().unwrap();
            let game_exe = tempfile::Builder::new().suffix(".exe").tempfile().unwrap();
            let mut settings = SpecialOpsSettings {
                wegame_executable_path: wegame_exe.path().display().to_string(),
                game_executable_path: game_exe.path().display().to_string(),
                accounts: vec![account("selected", AccountStatus::Ready, Vec::new())],
                ..SpecialOpsSettings::default()
            };
            let mut reference_files = Vec::new();
            for key in [
                "wegame.loginMode",
                "wegame.loginFormReady",
                "wegame.login",
                "wegame.gameEntry",
                "wegame.launch",
            ] {
                let reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
                std::fs::write(reference.path(), key).unwrap();
                let target = settings.calibration_environments[0]
                    .targets
                    .iter_mut()
                    .find(|target| target.key == key)
                    .unwrap();
                target.rect = Some(CalibrationRect {
                    x: 10,
                    y: 20,
                    width: 30,
                    height: 40,
                });
                target.reference_image_path = Some(reference.path().display().to_string());
                target.verified_signature = Some(calibration_signature(target).unwrap());
                target.verified_at_ms = Some(1);
                reference_files.push(reference);
            }
            for key in ["wegame.account", "wegame.password"] {
                settings.calibration_environments[0]
                    .targets
                    .iter_mut()
                    .find(|target| target.key == key)
                    .unwrap()
                    .rect = Some(CalibrationRect {
                    x: 10,
                    y: 20,
                    width: 30,
                    height: 40,
                });
            }
            Self {
                settings,
                _exe_files: vec![wegame_exe, game_exe],
                _reference_files: reference_files,
            }
        }
    }

    fn calibration_target_mut<'a>(
        settings: &'a mut SpecialOpsSettings,
        key: &str,
    ) -> &'a mut CalibrationTarget {
        settings.calibration_environments[0]
            .targets
            .iter_mut()
            .find(|target| target.key == key)
            .unwrap()
    }

    fn station(kind: StationKind, finishes_at_ms: i64) -> StationPlan {
        StationPlan {
            kind,
            enabled: true,
            item_name: "测试物品".to_string(),
            duration_minutes: 240,
            started_at_ms: Some(finishes_at_ms - 14_400_000),
            finishes_at_ms: Some(finishes_at_ms),
            status: StationStatus::Crafting,
        }
    }

    fn account(id: &str, status: AccountStatus, stations: Vec<StationPlan>) -> AccountPlan {
        AccountPlan {
            id: id.to_string(),
            qq_account: id.to_string(),
            password: "password".to_string(),
            enabled: true,
            initialized: true,
            order: 0,
            status,
            stations,
            ammo_targets: Vec::new(),
            last_failure: None,
            login_trial_signature: None,
        }
    }

    #[test]
    fn legacy_wegame_identity_fields_are_dropped_after_roundtrip() {
        let settings: SpecialOpsSettings = serde_json::from_str(
            r#"{
                "enabled": true,
                "paused": true,
                "dailyExchangeTime": "08:00",
                "emergencyHotkey": "Ctrl+Shift+F12",
                "accounts": [{
                    "id": "legacy",
                    "qqAccount": "10001",
                    "password": "password",
                    "wegameId": "legacy-wegame-id",
                    "enabled": true,
                    "initialized": false,
                    "order": 0,
                    "status": "ready",
                    "stations": [],
                    "ammoTargets": []
                }],
                "activeCalibrationId": "legacy",
                "calibrationEnvironments": [{
                    "id": "legacy",
                    "name": "旧显示环境",
                    "monitor": "主显示器",
                    "resolutionWidth": 1920,
                    "resolutionHeight": 1080,
                    "dpiScale": 1.0,
                    "windowMode": "无边框窗口",
                    "targets": [{
                        "key": "wegame.launchPage",
                        "label": "旧游戏入口",
                        "kind": "recognitionRegion",
                        "rect": null
                    }]
                }]
            }"#,
        )
        .unwrap();

        assert!(settings.wegame_executable_path.is_empty());
        assert!(settings.game_executable_path.is_empty());
        assert_eq!(settings.accounts[0].last_failure, None);
        assert_eq!(settings.accounts[0].login_trial_signature, None);

        let normalized = normalize_settings(settings).unwrap();
        let serialized = serde_json::to_value(normalized).unwrap();
        let target_keys = serialized["calibrationEnvironments"][0]["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|target| target["key"].as_str().unwrap())
            .collect::<Vec<_>>();

        assert!(serialized["accounts"][0].get("wegameId").is_none());
        assert!(!target_keys.contains(&"wegame.launchPage"));
        assert!(target_keys.contains(&"wegame.gameEntry"));
    }

    #[test]
    fn login_targets_are_exactly_the_approved_seven() {
        let keys = default_calibration_targets()
            .into_iter()
            .filter(|target| target.key.starts_with("wegame."))
            .map(|target| target.key)
            .collect::<Vec<_>>();

        assert_eq!(
            keys,
            [
                "wegame.loginMode",
                "wegame.loginFormReady",
                "wegame.account",
                "wegame.password",
                "wegame.login",
                "wegame.gameEntry",
                "wegame.launch",
            ]
        );
    }

    #[test]
    fn enabled_qq_accounts_must_be_unique_but_passwords_may_match() {
        let mut settings = SpecialOpsSettings {
            accounts: vec![
                account("first", AccountStatus::Ready, Vec::new()),
                account("second", AccountStatus::Ready, Vec::new()),
            ],
            ..SpecialOpsSettings::default()
        };
        settings.accounts[0].qq_account = "10001".to_string();
        settings.accounts[1].qq_account = "10002".to_string();

        assert!(normalize_settings(settings.clone()).is_ok());

        settings.accounts[1].qq_account = " 10001 ".to_string();
        settings.accounts[1].enabled = false;
        assert!(normalize_settings(settings.clone()).is_ok());

        settings.accounts[1].enabled = true;
        assert_eq!(
            normalize_settings(settings.clone()).unwrap_err(),
            "启用账号的 QQ 账号必须唯一"
        );

        settings.accounts[0].qq_account.clear();
        settings.accounts[1].qq_account.clear();
        assert!(normalize_settings(settings).is_ok());
    }

    #[test]
    fn schedule_groups_all_due_stations_and_skips_isolated_accounts() {
        let now = 10_000_000;
        let settings = SpecialOpsSettings {
            enabled: true,
            paused: false,
            daily_exchange_time: "08:00".to_string(),
            emergency_hotkey: "Ctrl+Shift+F12".to_string(),
            accounts: vec![
                account(
                    "active",
                    AccountStatus::Ready,
                    vec![
                        station(StationKind::TechnicalCenter, now - 1),
                        station(StationKind::Workbench, now - 2),
                    ],
                ),
                account(
                    "isolated",
                    AccountStatus::Isolated,
                    vec![station(StationKind::Pharmacy, now - 1)],
                ),
            ],
            ..SpecialOpsSettings::default()
        };

        let snapshot = build_schedule(&settings, now);

        assert_eq!(snapshot.due_accounts.len(), 1);
        assert_eq!(snapshot.due_accounts[0].account_id, "active");
        assert_eq!(
            snapshot.due_accounts[0].station_kinds,
            vec![StationKind::TechnicalCenter, StationKind::Workbench]
        );
    }

    #[test]
    fn schedule_includes_only_unredeemed_ammo_after_daily_time() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-23T09:00:00+08:00")
            .unwrap()
            .timestamp_millis();
        let settings = SpecialOpsSettings {
            enabled: true,
            paused: false,
            daily_exchange_time: "08:00".to_string(),
            emergency_hotkey: "Ctrl+Shift+F12".to_string(),
            accounts: vec![AccountPlan {
                ammo_targets: vec![
                    AmmoTarget {
                        id: "alpha".to_string(),
                        name: "目标 A".to_string(),
                        enabled: true,
                        seasonal: false,
                        scroll_steps: 0,
                        order: 1,
                        last_success_day: None,
                        retry_count: 0,
                    },
                    AmmoTarget {
                        id: "beta".to_string(),
                        name: "目标 B".to_string(),
                        enabled: true,
                        seasonal: false,
                        scroll_steps: 2,
                        order: 2,
                        last_success_day: Some("2026-07-23".to_string()),
                        retry_count: 0,
                    },
                ],
                ..account("active", AccountStatus::Ready, Vec::new())
            }],
            ..SpecialOpsSettings::default()
        };

        let snapshot = build_schedule(&settings, now);

        assert_eq!(snapshot.due_accounts[0].ammo_target_ids, vec!["alpha"]);
    }

    #[test]
    fn schedule_wakes_at_daily_exchange_time_before_exchange() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-23T07:00:00+08:00")
            .unwrap()
            .timestamp_millis();
        let settings = SpecialOpsSettings {
            enabled: true,
            paused: false,
            daily_exchange_time: "08:00".to_string(),
            emergency_hotkey: "Ctrl+Shift+F12".to_string(),
            accounts: vec![AccountPlan {
                ammo_targets: vec![AmmoTarget {
                    id: "alpha".to_string(),
                    name: "鐩爣 A".to_string(),
                    enabled: true,
                    seasonal: false,
                    scroll_steps: 0,
                    order: 0,
                    last_success_day: None,
                    retry_count: 0,
                }],
                ..account("active", AccountStatus::Ready, Vec::new())
            }],
            ..SpecialOpsSettings::default()
        };

        let snapshot = build_schedule(&settings, now);

        assert!(snapshot.due_accounts.is_empty());
        assert_eq!(snapshot.next_wake_at_ms, Some(now + 60 * 60 * 1000));
    }

    #[test]
    fn due_crafting_within_five_minutes_of_exchange_is_deferred() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-23T07:57:00+08:00")
            .unwrap()
            .timestamp_millis();
        let settings = SpecialOpsSettings {
            enabled: true,
            paused: false,
            daily_exchange_time: "08:00".to_string(),
            emergency_hotkey: "Ctrl+Shift+F12".to_string(),
            accounts: vec![account(
                "active",
                AccountStatus::Ready,
                vec![station(StationKind::TechnicalCenter, now - 1)],
            )],
            ..SpecialOpsSettings::default()
        };

        let snapshot = build_schedule(&settings, now);

        assert!(snapshot.due_accounts.is_empty());
        assert_eq!(snapshot.next_wake_at_ms, Some(now + 3 * 60 * 1000));
    }

    #[test]
    fn daily_exchange_time_requires_hh_mm() {
        assert_eq!(daily_exchange_minutes("08:00"), Some(480));
        assert_eq!(daily_exchange_minutes("8:00"), None);
        assert_eq!(daily_exchange_minutes("08:0"), None);
        assert_eq!(daily_exchange_minutes("24:00"), None);
    }

    #[test]
    fn execution_preflight_allows_no_active_work_and_rejects_missing_calibration() {
        assert!(validate_execution_ready(&SpecialOpsSettings::default()).is_ok());

        let active = account(
            "active",
            AccountStatus::Ready,
            vec![station(StationKind::TechnicalCenter, 1)],
        );
        let settings = SpecialOpsSettings {
            accounts: vec![active.clone()],
            ..SpecialOpsSettings::default()
        };

        assert_eq!(
            validate_execution_ready(&settings).unwrap_err(),
            "校准未完成：WeGame QQ 账号密码登录入口识别与点击区域 尚未框选"
        );

        let mut invalid_account = active;
        invalid_account.password.clear();
        let settings = SpecialOpsSettings {
            accounts: vec![invalid_account],
            ..SpecialOpsSettings::default()
        };
        assert_eq!(
            validate_execution_ready(&settings).unwrap_err(),
            "账号 active 的 QQ 与密码必须完整"
        );
    }

    #[test]
    fn execution_preflight_reports_deleted_target_in_execution_order() {
        let mut settings = SpecialOpsSettings {
            accounts: vec![account(
                "active",
                AccountStatus::Ready,
                vec![station(StationKind::TechnicalCenter, 1)],
            )],
            ..SpecialOpsSettings::default()
        };
        let environment = &mut settings.calibration_environments[0];
        for target in &mut environment.targets {
            target.rect = Some(CalibrationRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            });
            if target.recognition_method == Some(CalibrationRecognitionMethod::Template) {
                target.reference_image_path =
                    Some(std::env::current_exe().unwrap().display().to_string());
            }
        }
        environment
            .targets
            .retain(|target| target.key != "wegame.loginFormReady");

        assert_eq!(
            validate_execution_ready(&settings).unwrap_err(),
            "校准未完成：缺少步骤 QQ 账号密码登录表单就绪区域"
        );
    }

    #[test]
    fn required_execution_targets_follow_enabled_features() {
        let mut active = account(
            "active",
            AccountStatus::Ready,
            vec![station(StationKind::TechnicalCenter, 1)],
        );
        active.ammo_targets.push(AmmoTarget {
            id: "ammo".to_string(),
            name: "测试子弹".to_string(),
            enabled: true,
            seasonal: false,
            scroll_steps: 0,
            order: 0,
            last_success_day: None,
            retry_count: 0,
        });
        let settings = SpecialOpsSettings {
            accounts: vec![active],
            ..SpecialOpsSettings::default()
        };

        let keys = required_execution_target_keys(&settings);
        assert!(keys.contains("craft.station.technicalCenter"));
        assert!(!keys.contains("craft.station.workbench"));
        assert!(keys.contains("ammo.target"));
        assert!(!keys.contains("ammo.seasonal"));
    }

    #[test]
    fn required_execution_wegame_targets_are_exactly_the_approved_seven() {
        let settings = SpecialOpsSettings {
            accounts: vec![account(
                "active",
                AccountStatus::Ready,
                vec![station(StationKind::TechnicalCenter, 1)],
            )],
            ..SpecialOpsSettings::default()
        };

        let actual = required_execution_target_keys(&settings)
            .into_iter()
            .filter(|key| key.starts_with("wegame."))
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            "wegame.loginMode",
            "wegame.loginFormReady",
            "wegame.account",
            "wegame.password",
            "wegame.login",
            "wegame.gameEntry",
            "wegame.launch",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(actual, expected);
    }

    #[test]
    fn ammo_target_defaults_scroll_steps_for_legacy_settings() {
        let target: AmmoTarget = serde_json::from_str(
            r#"{"id":"ammo-1","name":"测试子弹","enabled":true,"seasonal":false,"order":0,"lastSuccessDay":null,"retryCount":0}"#,
        )
        .expect("旧配置应兼容新增滚轮步数字段");

        assert_eq!(target.scroll_steps, 0);
    }

    #[test]
    fn calibration_target_defaults_reference_image_for_legacy_settings() {
        let target: CalibrationTarget = serde_json::from_str(
            r#"{"key":"game.modeReady","label":"模式选择","kind":"recognitionRegion","rect":null}"#,
        )
        .expect("旧校准配置应兼容新增参考图字段");

        assert_eq!(target.reference_image_path, None);
        assert_eq!(target.recognition_method, None);
        assert!(target.guard_any_of.is_empty());
    }

    #[test]
    fn calibration_target_defaults_verification_fields_for_legacy_settings() {
        let target: CalibrationTarget = serde_json::from_str(
            r#"{"key":"game.modeReady","label":"模式选择","kind":"recognitionRegion","rect":null}"#,
        )
        .expect("旧校准配置应兼容验证字段");

        assert_eq!(target.match_threshold, 0.75);
        assert_eq!(target.verified_signature, None);
        assert_eq!(target.verified_at_ms, None);
    }

    #[test]
    fn state_changed_payload_excludes_settings_accounts_and_passwords() {
        let mut settings = SpecialOpsSettings::default();
        let mut selected = account("selected", AccountStatus::Ready, Vec::new());
        selected.password = "test-secret-placeholder".to_string();
        settings.accounts.push(selected);
        let bootstrap = build_bootstrap(settings, 17, 23);

        let payload = SpecialOpsStateChanged::from(&bootstrap);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(!json.contains("test-secret-placeholder"));
        assert!(!json.contains("\"password\""));
        assert!(!json.contains("\"settings\""));
        assert!(!json.contains("\"accounts\""));
        let value = serde_json::to_value(payload).unwrap();
        assert_eq!(value["settingsRevision"], 17);
        assert_eq!(value["nowMs"], 23);
    }

    #[test]
    fn two_samples_must_both_reach_default_threshold_to_verify() {
        assert!(template_test_passed([0.75, 0.91], 0.75));
        assert!(!template_test_passed([0.74, 0.99], 0.75));
        assert!(!template_test_passed([f32::NAN, 0.99], 0.75));
        assert!(!template_test_passed([0.99, 0.99], f32::NAN));
        assert!(!template_test_passed([0.99, 0.99], -0.01));
        assert!(!template_test_passed([0.99, 0.99], 1.01));
    }

    #[test]
    fn changed_rect_reference_or_file_content_invalidates_verification() {
        let first_reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        let second_reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        std::fs::write(first_reference.path(), b"first").unwrap();
        std::fs::write(second_reference.path(), b"first").unwrap();
        let mut target = default_calibration_targets()
            .into_iter()
            .find(|target| target.key == "wegame.loginMode")
            .unwrap();
        target.rect = Some(CalibrationRect {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        });
        target.reference_image_path = Some(first_reference.path().display().to_string());
        let verified = calibration_signature(&target).unwrap();
        assert!(verified.starts_with("v2|"));

        target.rect.as_mut().unwrap().x += 1;
        assert_ne!(calibration_signature(&target).unwrap(), verified);

        target.rect.as_mut().unwrap().x -= 1;
        target.reference_image_path = Some(second_reference.path().display().to_string());
        assert_ne!(calibration_signature(&target).unwrap(), verified);

        target.reference_image_path = Some(first_reference.path().display().to_string());
        let original_metadata = std::fs::metadata(first_reference.path()).unwrap();
        let original_modified = original_metadata.modified().unwrap();
        let original_accessed = original_metadata.accessed().unwrap();
        std::fs::write(first_reference.path(), b"other").unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(first_reference.path())
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(original_modified)
                    .set_accessed(original_accessed),
            )
            .unwrap();
        let changed_metadata = std::fs::metadata(first_reference.path()).unwrap();
        assert_eq!(changed_metadata.len(), original_metadata.len());
        assert_eq!(changed_metadata.modified().unwrap(), original_modified);
        assert_ne!(calibration_signature(&target).unwrap(), verified);
    }

    #[test]
    fn normalize_clears_stale_template_verification() {
        let reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        std::fs::write(reference.path(), b"reference").unwrap();
        let mut settings = SpecialOpsSettings::default();
        let target = settings.calibration_environments[0]
            .targets
            .iter_mut()
            .find(|target| target.key == "wegame.loginMode")
            .unwrap();
        target.rect = Some(CalibrationRect {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        });
        target.reference_image_path = Some(reference.path().display().to_string());
        let valid_signature = calibration_signature(target).unwrap();
        target.verified_signature = Some(valid_signature.clone());
        target.verified_at_ms = Some(123);
        assert!(verification_is_current(target));

        let preserved = normalize_settings(settings.clone()).unwrap();
        let preserved_target = preserved.calibration_environments[0]
            .targets
            .iter()
            .find(|target| target.key == "wegame.loginMode")
            .unwrap();
        assert!(preserved_target.verified_signature.is_some());
        assert_eq!(preserved_target.verified_at_ms, Some(123));

        for (signature, verified_at_ms) in [
            (Some(valid_signature.clone()), None),
            (None, Some(123)),
            (Some(valid_signature.clone()), Some(-1)),
            (Some("旧签名".to_string()), Some(123)),
        ] {
            let mut invalid = settings.clone();
            let target = calibration_target_mut(&mut invalid, "wegame.loginMode");
            target.verified_signature = signature;
            target.verified_at_ms = verified_at_ms;
            assert!(!verification_is_current(target));

            let normalized = normalize_settings(invalid).unwrap();
            let target = normalized.calibration_environments[0]
                .targets
                .iter()
                .find(|target| target.key == "wegame.loginMode")
                .unwrap();
            assert_eq!(target.verified_signature, None);
            assert_eq!(target.verified_at_ms, None);
        }
    }

    #[test]
    fn calibration_test_commit_persists_success_before_updating_memory() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        let target = calibration_target_mut(&mut settings, "wegame.loginMode");
        let tested_signature = calibration_signature(target).unwrap();
        target.verified_signature = None;
        target.verified_at_ms = None;
        let persisted = std::cell::Cell::new(false);

        commit_calibration_test_verification(
            &mut settings,
            "default",
            "wegame.loginMode",
            &tested_signature,
            true,
            Some(456),
            |next| {
                let target = next.calibration_environments[0]
                    .targets
                    .iter()
                    .find(|target| target.key == "wegame.loginMode")
                    .unwrap();
                assert_eq!(
                    target.verified_signature.as_deref(),
                    Some(tested_signature.as_str())
                );
                assert_eq!(target.verified_at_ms, Some(456));
                persisted.set(true);
                Ok(())
            },
        )
        .unwrap();

        assert!(persisted.get());
        let target = calibration_target_mut(&mut settings, "wegame.loginMode");
        assert_eq!(
            target.verified_signature.as_deref(),
            Some(tested_signature.as_str())
        );
        assert_eq!(target.verified_at_ms, Some(456));
    }

    #[test]
    fn calibration_test_commit_clears_verification_after_low_score() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        let tested_signature =
            calibration_signature(calibration_target_mut(&mut settings, "wegame.loginMode"))
                .unwrap();

        commit_calibration_test_verification(
            &mut settings,
            "default",
            "wegame.loginMode",
            &tested_signature,
            false,
            None,
            |next| {
                let target = next.calibration_environments[0]
                    .targets
                    .iter()
                    .find(|target| target.key == "wegame.loginMode")
                    .unwrap();
                assert_eq!(target.verified_signature, None);
                assert_eq!(target.verified_at_ms, None);
                Ok(())
            },
        )
        .unwrap();

        let target = calibration_target_mut(&mut settings, "wegame.loginMode");
        assert_eq!(target.verified_signature, None);
        assert_eq!(target.verified_at_ms, None);
    }

    #[test]
    fn calibration_test_commit_rejects_stale_signature_without_saving() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        let original = settings.clone();
        let saved = std::cell::Cell::new(false);

        let error = commit_calibration_test_verification(
            &mut settings,
            "default",
            "wegame.loginMode",
            "stale",
            true,
            Some(456),
            |_| {
                saved.set(true);
                Ok(())
            },
        )
        .unwrap_err();

        assert_eq!(error, "校准配置已变化，请重新测试");
        assert!(!saved.get());
        assert_eq!(settings, original);
    }

    #[test]
    fn calibration_test_commit_save_failure_does_not_pollute_memory() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        let original = settings.clone();
        let tested_signature =
            calibration_signature(calibration_target_mut(&mut settings, "wegame.loginMode"))
                .unwrap();

        let error = commit_calibration_test_verification(
            &mut settings,
            "default",
            "wegame.loginMode",
            &tested_signature,
            true,
            Some(456),
            |_| Err("保存失败".to_string()),
        )
        .unwrap_err();

        assert_eq!(error, "保存失败");
        assert_eq!(settings, original);
    }

    #[test]
    fn stale_revision_rejects_calibration_commit_before_state_transition() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        let original = settings.clone();
        let coordinator = SettingsCoordinator::new();
        coordinator
            .with_profile_change(|_| Ok::<_, String>(()))
            .unwrap();
        let called = std::cell::Cell::new(false);

        let error = coordinator
            .with_revision(1, || -> Result<(), String> {
                called.set(true);
                commit_calibration_test_verification(
                    &mut settings,
                    "default",
                    "wegame.loginMode",
                    "unused",
                    true,
                    Some(456),
                    |_| Ok(()),
                )
            })
            .unwrap_err();

        assert!(error.contains("配置保存已陈旧"));
        assert!(!called.get());
        assert_eq!(settings, original);
    }

    #[test]
    fn default_click_and_input_targets_have_recognition_guards() {
        let targets = default_calibration_targets();
        let actions = targets
            .iter()
            .filter(|target| {
                matches!(
                    target.kind,
                    CalibrationTargetKind::ClickPoint | CalibrationTargetKind::InputRegion
                )
            })
            .collect::<Vec<_>>();

        assert!(!actions.is_empty());
        for action in actions {
            assert!(
                !action.guard_any_of.is_empty(),
                "动作 {} 缺少识别守卫",
                action.key
            );
            assert!(action
                .guard_any_of
                .iter()
                .all(|guard_key| targets.iter().any(|target| {
                    target.key == *guard_key
                        && target.kind == CalibrationTargetKind::RecognitionRegion
                })));
        }
    }

    #[test]
    fn default_dynamic_text_targets_use_ocr() {
        let targets = default_calibration_targets();

        let target = targets
            .iter()
            .find(|target| target.key == "ammo.selectedTargetName")
            .unwrap();
        assert_eq!(
            target.recognition_method,
            Some(CalibrationRecognitionMethod::Ocr)
        );
        assert_eq!(target.reference_image_path, None);
        assert_eq!(
            targets
                .iter()
                .find(|target| target.key == "wegame.loginFormReady")
                .unwrap()
                .recognition_method,
            Some(CalibrationRecognitionMethod::Template)
        );
    }

    #[test]
    fn calibration_template_test_requires_template_region_and_reference() {
        let mut settings = SpecialOpsSettings::default();
        assert_eq!(
            calibration_template_test_input(&settings, "default", "wegame.loginMode").unwrap_err(),
            "WeGame QQ 账号密码登录入口识别与点击区域 尚未框选"
        );

        let target = settings.calibration_environments[0]
            .targets
            .iter_mut()
            .find(|target| target.key == "wegame.loginMode")
            .unwrap();
        target.rect = Some(CalibrationRect {
            x: 10,
            y: 20,
            width: 30,
            height: 40,
        });
        let reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        std::fs::write(reference.path(), b"reference").unwrap();
        target.reference_image_path = Some(reference.path().display().to_string());
        let expected_signature = calibration_signature(target).unwrap();
        let input =
            calibration_template_test_input(&settings, "default", "wegame.loginMode").unwrap();
        assert_eq!(
            (
                input.region.x,
                input.region.y,
                input.region.width,
                input.region.height
            ),
            (10, 20, 30, 40)
        );
        assert_eq!(
            input.reference_image_path,
            reference.path().display().to_string()
        );
        assert_eq!(input.match_threshold, 0.75);
        assert_eq!(input.calibration_signature, expected_signature);
        assert_eq!(
            calibration_template_test_input(&settings, "default", "ammo.selectedTargetName")
                .unwrap_err(),
            "OCR 测试尚未接入，不能伪造识别结果"
        );
    }

    #[test]
    fn normalize_restores_required_calibration_targets() {
        let mut settings = SpecialOpsSettings::default();
        settings.calibration_environments[0].targets.clear();

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(
            normalized.calibration_environments[0].targets.len(),
            default_calibration_targets().len()
        );
        assert_eq!(normalized.active_calibration_id.as_deref(), Some("default"));
    }

    #[test]
    fn normalize_replaces_shared_claim_ready_with_station_targets() {
        let mut settings = SpecialOpsSettings::default();
        settings.calibration_environments[0]
            .targets
            .push(CalibrationTarget {
                key: "craft.claimReady".to_string(),
                label: "旧通用感叹号".to_string(),
                kind: CalibrationTargetKind::RecognitionRegion,
                rect: Some(CalibrationRect {
                    x: 1,
                    y: 2,
                    width: 3,
                    height: 4,
                }),
                reference_image_path: None,
                recognition_method: None,
                guard_any_of: Vec::new(),
                match_threshold: default_match_threshold(),
                verified_signature: None,
                verified_at_ms: None,
            });

        let normalized = normalize_settings(settings).unwrap();
        let targets = &normalized.calibration_environments[0].targets;

        assert!(!targets
            .iter()
            .any(|target| target.key == "craft.claimReady"));
        assert_eq!(
            targets
                .iter()
                .filter(|target| target.key.starts_with("craft.claimReady."))
                .count(),
            4
        );
    }

    #[test]
    fn normalize_replaces_shared_station_click_with_station_targets() {
        let mut settings = SpecialOpsSettings::default();
        settings.calibration_environments[0]
            .targets
            .push(CalibrationTarget {
                key: "craft.station".to_string(),
                label: "旧通用制作台点击区域".to_string(),
                kind: CalibrationTargetKind::ClickPoint,
                rect: Some(CalibrationRect {
                    x: 1,
                    y: 2,
                    width: 3,
                    height: 4,
                }),
                reference_image_path: None,
                recognition_method: None,
                guard_any_of: Vec::new(),
                match_threshold: default_match_threshold(),
                verified_signature: None,
                verified_at_ms: None,
            });

        let normalized = normalize_settings(settings).unwrap();
        let targets = &normalized.calibration_environments[0].targets;

        assert!(!targets.iter().any(|target| target.key == "craft.station"));
        assert_eq!(
            targets
                .iter()
                .filter(|target| target.key.starts_with("craft.station."))
                .count(),
            4
        );
    }

    #[test]
    fn normalize_replaces_shared_recipe_and_idle_targets() {
        let mut settings = SpecialOpsSettings::default();
        settings.calibration_environments[0].targets.extend([
            CalibrationTarget {
                key: "craft.recipe".to_string(),
                label: "旧通用配方点击区域".to_string(),
                kind: CalibrationTargetKind::ClickPoint,
                rect: None,
                reference_image_path: None,
                recognition_method: None,
                guard_any_of: Vec::new(),
                match_threshold: default_match_threshold(),
                verified_signature: None,
                verified_at_ms: None,
            },
            CalibrationTarget {
                key: "craft.idle".to_string(),
                label: "旧通用空闲文字区域".to_string(),
                kind: CalibrationTargetKind::RecognitionRegion,
                rect: None,
                reference_image_path: None,
                recognition_method: None,
                guard_any_of: Vec::new(),
                match_threshold: default_match_threshold(),
                verified_signature: None,
                verified_at_ms: None,
            },
        ]);

        let normalized = normalize_settings(settings).unwrap();
        let targets = &normalized.calibration_environments[0].targets;

        assert!(!targets
            .iter()
            .any(|target| { target.key == "craft.recipe" || target.key == "craft.idle" }));
        assert_eq!(
            targets
                .iter()
                .filter(|target| target.key.starts_with("craft.recipe."))
                .count(),
            4
        );
        assert_eq!(
            targets
                .iter()
                .filter(|target| target.key.starts_with("craft.idle."))
                .count(),
            4
        );
    }

    #[test]
    fn normalize_allows_incomplete_enabled_station_draft() {
        let mut settings = SpecialOpsSettings::default();
        settings.accounts.push(account(
            "draft",
            AccountStatus::Ready,
            vec![StationPlan {
                kind: StationKind::TechnicalCenter,
                enabled: true,
                item_name: String::new(),
                duration_minutes: 0,
                started_at_ms: None,
                finishes_at_ms: None,
                status: StationStatus::Idle,
            }],
        ));

        assert!(normalize_settings(settings).is_ok());
    }

    #[test]
    fn normalize_keeps_only_active_calibration_environment() {
        let mut settings = SpecialOpsSettings::default();
        let mut second = default_calibration_environment();
        second.id = "second".to_string();
        settings.calibration_environments.push(second);
        settings.active_calibration_id = Some("second".to_string());

        let normalized = normalize_settings(settings).unwrap();

        assert_eq!(normalized.calibration_environments.len(), 1);
        assert_eq!(normalized.calibration_environments[0].id, "second");
    }

    #[test]
    fn login_trial_preflight_requires_existing_absolute_exe_paths() {
        let fixture = LoginFixture::complete();
        assert!(validate_login_trial_ready(&fixture.settings, "selected").is_ok());

        let mut settings = fixture.settings.clone();
        settings.wegame_executable_path.clear();
        let error = validate_login_trial_ready(&settings, "selected").unwrap_err();
        assert!(error.contains("WeGame.exe"));
        assert!(error.contains("不能为空"));

        settings = fixture.settings.clone();
        settings.wegame_executable_path = "WeGame.exe".to_string();
        let error = validate_login_trial_ready(&settings, "selected").unwrap_err();
        assert!(error.contains("WeGame.exe"));
        assert!(error.contains("绝对路径"));

        settings = fixture.settings.clone();
        settings.wegame_executable_path = fixture._reference_files[0].path().display().to_string();
        let error = validate_login_trial_ready(&settings, "selected").unwrap_err();
        assert!(error.contains("WeGame.exe"));
        assert!(error.contains(".exe"));

        let missing = tempfile::tempdir().unwrap().path().join("missing.exe");
        settings = fixture.settings.clone();
        settings.wegame_executable_path = missing.display().to_string();
        let error = validate_login_trial_ready(&settings, "selected").unwrap_err();
        assert!(error.contains("WeGame.exe"));
        assert!(error.contains("不存在"));

        settings = fixture.settings.clone();
        settings.game_executable_path.clear();
        let error = validate_login_trial_ready(&settings, "selected").unwrap_err();
        assert!(error.contains("游戏 .exe"));
    }

    #[test]
    fn login_trial_preflight_allows_duplicate_passwords_and_requires_selected_account_credentials()
    {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings.clone();
        let mut other = account("other", AccountStatus::Ready, Vec::new());
        other.password = settings.accounts[0].password.clone();
        settings.accounts.push(other);
        let settings = normalize_settings(settings).unwrap();
        assert!(validate_login_trial_ready(&settings, "selected").is_ok());

        let mut empty_qq = fixture.settings.clone();
        empty_qq.accounts[0].qq_account = "   ".to_string();
        let error = validate_login_trial_ready(&empty_qq, "selected").unwrap_err();
        assert!(error.contains("selected"));
        assert!(error.contains("QQ"));

        let mut empty_password = fixture.settings.clone();
        empty_password.accounts[0].password.clear();
        let error = validate_login_trial_ready(&empty_password, "selected").unwrap_err();
        assert!(error.contains("selected"));
        assert!(error.contains("密码"));
    }

    #[test]
    fn login_trial_preflight_requires_existing_enabled_account() {
        let fixture = LoginFixture::complete();
        let error = validate_login_trial_ready(&fixture.settings, "missing").unwrap_err();
        assert!(error.contains("missing"));
        assert!(error.contains("不存在"));

        let mut disabled = fixture.settings.clone();
        disabled.accounts[0].enabled = false;
        let error = validate_login_trial_ready(&disabled, "selected").unwrap_err();
        assert!(error.contains("selected"));
        assert!(error.contains("未启用"));
    }

    #[test]
    fn login_trial_preflight_requires_five_verified_templates_and_two_input_regions() {
        let fixture = LoginFixture::complete();
        let ordered_keys = [
            "wegame.loginMode",
            "wegame.loginFormReady",
            "wegame.account",
            "wegame.password",
            "wegame.login",
            "wegame.gameEntry",
            "wegame.launch",
        ];
        assert!(validate_login_trial_ready(&fixture.settings, "selected").is_ok());

        for (index, key) in ordered_keys.iter().enumerate() {
            let mut settings = fixture.settings.clone();
            for missing_key in &ordered_keys[index..] {
                settings.calibration_environments[0]
                    .targets
                    .iter_mut()
                    .find(|target| target.key == *missing_key)
                    .unwrap()
                    .rect = None;
            }
            let expected_label = fixture.settings.calibration_environments[0]
                .targets
                .iter()
                .find(|target| target.key == *key)
                .unwrap()
                .label
                .clone();
            let error = validate_login_trial_ready(&settings, "selected").unwrap_err();
            assert!(error.contains(&expected_label), "{key}: {error}");
            assert!(error.contains("尚未框选"), "{key}: {error}");
        }

        let mut no_reference = fixture.settings.clone();
        no_reference.calibration_environments[0]
            .targets
            .iter_mut()
            .find(|target| target.key == "wegame.loginMode")
            .unwrap()
            .reference_image_path = None;
        assert!(validate_login_trial_ready(&no_reference, "selected")
            .unwrap_err()
            .contains("尚未上传参考图"));

        let mut missing_reference = fixture.settings.clone();
        missing_reference.calibration_environments[0]
            .targets
            .iter_mut()
            .find(|target| target.key == "wegame.loginMode")
            .unwrap()
            .reference_image_path = Some(
            tempfile::tempdir()
                .unwrap()
                .path()
                .join("missing.png")
                .display()
                .to_string(),
        );
        assert!(validate_login_trial_ready(&missing_reference, "selected")
            .unwrap_err()
            .contains("参考图文件不存在"));

        for key in [
            "wegame.loginMode",
            "wegame.loginFormReady",
            "wegame.login",
            "wegame.gameEntry",
            "wegame.launch",
        ] {
            let mut settings = fixture.settings.clone();
            settings.calibration_environments[0]
                .targets
                .iter_mut()
                .find(|target| target.key == key)
                .unwrap()
                .verified_signature = None;
            let error = validate_login_trial_ready(&settings, "selected").unwrap_err();
            assert!(error.contains("尚未测试或验证失效"), "{key}: {error}");
        }

        for verified_at_ms in [None, Some(-1)] {
            let mut settings = fixture.settings.clone();
            calibration_target_mut(&mut settings, "wegame.loginMode").verified_at_ms =
                verified_at_ms;
            let error = validate_login_trial_ready(&settings, "selected").unwrap_err();
            assert!(error.contains("尚未测试或验证失效"), "{error}");
        }
    }
}
