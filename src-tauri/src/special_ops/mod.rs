pub(crate) mod desktop_runtime;
#[allow(dead_code)]
pub(crate) mod login_flow;
mod login_runtime;
#[allow(dead_code)]
pub(crate) mod template_observer;

use chrono::{FixedOffset, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::overlay_utils::{destroy_window, encoded_query_value, safe_label_component};
use crate::{app_error::AppError, settings::SettingsCoordinator};

pub use login_runtime::{LoginRunSnapshot, LoginRunStatus};

const SETTINGS_FILE_NAME: &str = "special_ops_settings.json";
pub const STATE_CHANGED: &str = "special-ops://state-changed";
const LOGIN_HOTKEY_SCOPE: &str = "special-ops-emergency";
static LOGIN_RESOURCE_CLEANUP_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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
        "wegame.accountDropdown" | "wegame.selectedAccount" => &["wegame.loginFormReady"],
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
    use CalibrationTargetKind::{ClickPoint, RecognitionRegion};
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
        (
            "wegame.accountDropdown",
            "已记住账号列表展开按钮",
            ClickPoint,
        ),
        (
            "wegame.accountList",
            "已记住账号列表 OCR 区域",
            RecognitionRegion,
        ),
        (
            "wegame.selectedAccount",
            "已选账号双击复制区域",
            ClickPoint,
        ),
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
            (RecognitionRegion, "ammo.selectedTargetName" | "wegame.accountList") => {
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
    #[serde(default)]
    pub run_snapshot: Option<LoginRunSnapshot>,
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
    login_runtime: Arc<login_runtime::LoginRuntime>,
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

fn login_trial_signature(
    settings: &SpecialOpsSettings,
    account: &AccountPlan,
) -> Result<String, String> {
    let wegame = std::fs::canonicalize(&settings.wegame_executable_path)
        .map_err(|_| "WeGame.exe 路径无法规范化".to_string())?;
    let game = std::fs::canonicalize(&settings.game_executable_path)
        .map_err(|_| "游戏 .exe 路径无法规范化".to_string())?;
    let environment = settings
        .calibration_environments
        .first()
        .ok_or_else(|| "登录试运行校准未完成：缺少显示环境".to_string())?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"login-trial-v2\0");
    for value in [
        account.qq_account.as_bytes(),
        wegame.as_os_str().as_encoded_bytes(),
        game.as_os_str().as_encoded_bytes(),
    ] {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);
    }
    for key in [
        "wegame.loginMode",
        "wegame.loginFormReady",
        "wegame.accountDropdown",
        "wegame.accountList",
        "wegame.selectedAccount",
        "wegame.login",
        "wegame.gameEntry",
        "wegame.launch",
    ] {
        let target = environment
            .targets
            .iter()
            .find(|target| target.key == key)
            .ok_or_else(|| format!("登录试运行校准未完成：缺少步骤 {key}"))?;
        bytes.extend_from_slice(target.key.as_bytes());
        if let Some(rect) = &target.rect {
            for value in [rect.x, rect.y, rect.width, rect.height] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        if let Some(path) = target.reference_image_path.as_deref() {
            let path = std::fs::canonicalize(path)
                .map_err(|_| format!("{} 的参考图路径无法规范化", target.label))?;
            bytes.extend_from_slice(path.as_os_str().as_encoded_bytes());
        }
        bytes.extend_from_slice(&target.match_threshold.to_bits().to_le_bytes());
        if let Some(signature) = &target.verified_signature {
            bytes.extend_from_slice(signature.as_bytes());
        }
        for guard in &target.guard_any_of {
            bytes.extend_from_slice(guard.as_bytes());
            bytes.push(0);
        }
    }
    Ok(format!("login-v2-{:016x}", fnv1a_64(&bytes)))
}

fn apply_login_flow_result(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    result: &login_flow::LoginFlowResult,
    stop_reason: login_runtime::StopReason,
    frozen_signature: &str,
) -> Result<(), String> {
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "登录试运行账号已不存在".to_string())?;
    match result {
        login_flow::LoginFlowResult::GameReady { .. } => {
            account.status = AccountStatus::Ready;
            account.last_failure = None;
            account.login_trial_signature = Some(frozen_signature.to_string());
        }
        login_flow::LoginFlowResult::Paused {
            failed_step,
            last_observation,
            failed_at,
        } => {
            settings.paused = true;
            account.status = AccountStatus::LoginFailed;
            account.last_failure = Some(AccountFailure {
                step: format!("{failed_step:?}"),
                message: last_observation.clone(),
                at_ms: *failed_at,
            });
        }
        login_flow::LoginFlowResult::EmergencyStopped { stopped_at, .. } => match stop_reason {
            login_runtime::StopReason::Normal => {}
            login_runtime::StopReason::Emergency => {
                settings.paused = true;
                account.status = AccountStatus::Uncertain;
                account.last_failure = Some(AccountFailure {
                    step: "emergencyStop".to_string(),
                    message: "登录试运行已紧急停止，账号状态需人工确认".to_string(),
                    at_ms: *stopped_at,
                });
            }
            login_runtime::StopReason::Lifecycle { uncertain } => {
                settings.paused = true;
                if uncertain {
                    account.status = AccountStatus::Uncertain;
                    account.last_failure = Some(AccountFailure {
                        step: "lifecycleStop".to_string(),
                        message: "应用停止时登录操作尚未确认，账号状态需人工确认".to_string(),
                        at_ms: *stopped_at,
                    });
                }
            }
        },
    }
    Ok(())
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
        if !existing_targets.contains_key("wegame.selectedAccount") {
            if let Some(mut legacy_account) = existing_targets.remove("wegame.account") {
                legacy_account.key = "wegame.selectedAccount".to_string();
                existing_targets.insert(legacy_account.key.clone(), legacy_account);
            }
        }
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
        "wegame.accountDropdown",
        "wegame.accountList",
        "wegame.selectedAccount",
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
        if account.qq_account.is_empty()
            || !account.qq_account.chars().all(|ch| ch.is_ascii_digit())
        {
            return Err(format!("账号 {} 的 QQ 必须为非空纯数字", account.id));
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
    if account.qq_account.trim().is_empty()
        || !account.qq_account.chars().all(|ch| ch.is_ascii_digit())
    {
        return Err(format!("登录试运行账号 {account_id} 的 QQ 必须为非空纯数字"));
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
        "wegame.accountDropdown",
        "wegame.accountList",
        "wegame.selectedAccount",
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

fn freeze_login_run_config(
    settings: &SpecialOpsSettings,
    account_id: &str,
) -> Result<(login_flow::LoginRunConfig, String), String> {
    validate_login_trial_ready(settings, account_id)?;
    let account = settings
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "登录试运行账号不存在".to_string())?;
    let environment = settings
        .calibration_environments
        .first()
        .ok_or_else(|| "登录试运行校准未完成：缺少显示环境".to_string())?;
    let mut targets = std::collections::HashMap::new();
    for key in [
        "wegame.loginMode",
        "wegame.loginFormReady",
        "wegame.accountDropdown",
        "wegame.accountList",
        "wegame.selectedAccount",
        "wegame.login",
        "wegame.gameEntry",
        "wegame.launch",
    ] {
        let target = environment
            .targets
            .iter()
            .find(|target| target.key == key)
            .ok_or_else(|| format!("登录试运行校准目标 {key} 不存在"))?;
        let rect = target
            .rect
            .as_ref()
            .ok_or_else(|| format!("{} 尚未框选", target.label))?;
        let region = crate::morse::types::RegionRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let template = if target.recognition_method == Some(CalibrationRecognitionMethod::Template)
        {
            let reference_image_path = std::fs::canonicalize(
                target
                    .reference_image_path
                    .as_deref()
                    .ok_or_else(|| format!("{} 尚未上传参考图", target.label))?,
            )
            .map_err(|_| format!("{} 的参考图文件不存在", target.label))?;
            Some(template_observer::RuntimeTemplate {
                key: key.to_string(),
                region: region.clone(),
                reference_image_path,
                threshold: target.match_threshold,
            })
        } else {
            None
        };
        targets.insert(
            key.to_string(),
            template_observer::RuntimeTarget {
                key: key.to_string(),
                region,
                template,
                guard_any_of: target.guard_any_of.clone(),
            },
        );
    }
    Ok((
        login_flow::LoginRunConfig {
            account_id: account.id.clone(),
            qq_account: account.qq_account.clone(),
            password: String::new(),
            wegame_executable_path: std::fs::canonicalize(&settings.wegame_executable_path)
                .map_err(|_| "WeGame.exe 路径无法规范化".to_string())?,
            game_executable_path: std::fs::canonicalize(&settings.game_executable_path)
                .map_err(|_| "游戏 .exe 路径无法规范化".to_string())?,
            targets,
        },
        login_trial_signature(settings, account)?,
    ))
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
        run_snapshot: None,
    }
}

fn build_bootstrap_with_runtime(
    settings: SpecialOpsSettings,
    settings_revision: u64,
    current_ms: i64,
    runtime: &login_runtime::LoginRuntime,
) -> Result<SpecialOpsBootstrap, String> {
    let mut bootstrap = build_bootstrap(settings, settings_revision, current_ms);
    bootstrap.run_snapshot = runtime.snapshot()?;
    Ok(bootstrap)
}

fn emit_state(app: &AppHandle, bootstrap: &SpecialOpsBootstrap) {
    let _ = app.emit_to(
        "main",
        STATE_CHANGED,
        SpecialOpsStateChanged::from(bootstrap),
    );
}

fn emit_run(app: &AppHandle, snapshot: &LoginRunSnapshot) {
    login_runtime::emit_run_changed(app, snapshot);
}

fn emit_login_run_change(
    app: &AppHandle,
    runtime: &login_runtime::LoginRuntime,
    change: impl FnOnce() -> Result<Option<LoginRunSnapshot>, String>,
) -> Result<Option<LoginRunSnapshot>, String> {
    runtime.with_event_serialized(|| {
        let snapshot = change()?;
        if let Some(snapshot) = snapshot.as_ref() {
            emit_run(app, snapshot);
        }
        Ok(snapshot)
    })
}

fn emit_current_state(app: &AppHandle) -> Result<(), String> {
    let state = app
        .try_state::<SpecialOpsState>()
        .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
    let coordinator = app
        .try_state::<Arc<SettingsCoordinator>>()
        .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
    let settings = state
        .settings
        .lock()
        .map_err(|_| "特勤处状态已损坏".to_string())?
        .clone();
    emit_state(
        app,
        &build_bootstrap(settings, coordinator.current_revision()?, now_ms()),
    );
    Ok(())
}

fn create_operation_window(app: &AppHandle, emergency_hotkey: &str) -> Result<(), String> {
    destroy_operation_window(app)?;
    let hotkey = encoded_query_value(emergency_hotkey);
    let window = tauri::WebviewWindowBuilder::new(
        app,
        login_runtime::OPERATION_WINDOW_LABEL,
        tauri::WebviewUrl::App(
            format!("index.html?mode=special-ops-operation&emergencyHotkey={hotkey}").into(),
        ),
    )
    .title("特勤处登录试运行")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .resizable(false)
    .inner_size(420.0, 180.0)
    .build()
    .map_err(|error| format!("创建登录试运行窗口失败: {error}"))?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| format!("设置登录试运行窗口点击穿透失败: {error}"))
}

fn register_emergency_hotkey(app: &AppHandle, hotkey: String) -> Result<(), String> {
    let manager = app
        .try_state::<crate::hotkeys::HotkeyManager>()
        .ok_or_else(|| "热键管理器尚未初始化".to_string())?;
    let action: crate::hotkey_types::HotkeyAction = Arc::new(|app| {
        if let Err(error) = emergency_stop_core(&app) {
            crate::log_error!(
                "special_ops::login",
                "紧急停止失败",
                "error" => error
            );
        }
    });
    manager.replace_scope(
        LOGIN_HOTKEY_SCOPE,
        vec![(hotkey, action)],
        "特勤处紧急停止".to_string(),
        crate::hotkey_types::ConflictPolicy::Strict,
    )
}

fn clear_login_hotkey(app: &AppHandle) -> Result<(), String> {
    if let Some(manager) = app.try_state::<crate::hotkeys::HotkeyManager>() {
        manager.clear_scope(LOGIN_HOTKEY_SCOPE)?;
    }
    Ok(())
}

fn destroy_operation_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(login_runtime::OPERATION_WINDOW_LABEL) {
        window
            .destroy()
            .map_err(|error| format!("销毁登录试运行窗口失败: {error}"))?;
    }
    Ok(())
}

fn release_login_resources_with(
    release_inputs: impl FnOnce(),
    clear_hotkey: impl FnOnce() -> Result<(), String>,
    destroy_window: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    release_inputs();
    let mut errors = Vec::new();
    if let Err(error) = clear_hotkey() {
        errors.push(error);
    }
    if let Err(error) = destroy_window() {
        errors.push(error);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn release_login_resources_unlocked(app: &AppHandle) -> Result<(), String> {
    release_login_resources_with(
        crate::input_simulation::release_tracked_injected_inputs,
        || clear_login_hotkey(app),
        || destroy_operation_window(app),
    )
}

fn rollback_login_start_unlocked(
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
    error: String,
    cleanup: impl FnOnce() -> Result<(), String>,
) -> String {
    if let Err(cleanup_error) = cleanup() {
        let _ = runtime.mark_cleanup_failed(run_id);
        return format!("{error}; {cleanup_error}");
    }
    match runtime.stop_reason(run_id) {
        Ok(Some(
            login_runtime::StopReason::Emergency | login_runtime::StopReason::Lifecycle { .. },
        )) => {}
        Ok(stop_reason) => {
            let status = if stop_reason.is_some() {
                LoginRunStatus::Stopped
            } else {
                LoginRunStatus::Failed
            };
            let _ = runtime.finish(run_id, status, "登录试运行启动失败");
        }
        Err(runtime_error) => return format!("{error}; {runtime_error}"),
    }
    error
}

fn start_login_run_with_resources<R, C, A, L, S>(
    runtime: &login_runtime::LoginRuntime,
    account_id: String,
    register_hotkey: R,
    create_window: C,
    announce_start: A,
    cleanup: L,
    spawn_worker: S,
) -> Result<(login_runtime::StartedLoginRun, LoginRunSnapshot), String>
where
    R: FnOnce() -> Result<(), String>,
    C: FnOnce() -> Result<(), String>,
    A: FnOnce(&LoginRunSnapshot),
    L: FnOnce() -> Result<(), String>,
    S: FnOnce(&login_runtime::StartedLoginRun) -> Result<(), String>,
{
    let _resources = LOGIN_RESOURCE_CLEANUP_LOCK
        .lock()
        .map_err(|_| "登录试运行资源清理锁已损坏".to_string())?;
    let started = runtime.try_start(account_id)?;
    let snapshot = runtime
        .snapshot()?
        .ok_or_else(|| "登录试运行启动状态丢失".to_string())?;

    if !runtime.can_continue_start(started.run_id)? {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            "登录试运行启动期间已停止".to_string(),
            cleanup,
        ));
    }
    if let Err(error) = register_hotkey() {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            error,
            cleanup,
        ));
    }
    if !runtime.can_continue_start(started.run_id)? {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            "登录试运行启动期间已停止".to_string(),
            cleanup,
        ));
    }
    if let Err(error) = create_window() {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            error,
            cleanup,
        ));
    }
    let handed_off = runtime.with_event_serialized(|| {
        if !runtime.claim_worker_handoff(started.run_id)? {
            return Ok(false);
        }
        announce_start(&snapshot);
        Ok(true)
    });
    if !matches!(handed_off, Ok(true)) {
        let error = handed_off
            .err()
            .unwrap_or_else(|| "登录试运行启动期间已停止".to_string());
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            error,
            cleanup,
        ));
    }
    if let Err(error) = spawn_worker(&started) {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            error,
            cleanup,
        ));
    }
    let snapshot = runtime
        .snapshot()?
        .filter(|snapshot| snapshot.run_id == started.run_id)
        .ok_or_else(|| "登录试运行启动状态已变化".to_string())?;
    Ok((started, snapshot))
}

fn release_login_resources_for_run(
    app: &AppHandle,
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
) -> Result<(), String> {
    let _cleanup = LOGIN_RESOURCE_CLEANUP_LOCK
        .lock()
        .map_err(|_| "登录试运行资源清理锁已损坏".to_string())?;
    match runtime.snapshot()? {
        Some(snapshot) if snapshot.run_id == run_id => release_login_resources_unlocked(app),
        _ => Ok(()),
    }
}

fn fail_closed_login_error_for_run(
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
    runtime_error: String,
    cleanup: impl FnOnce() -> Result<(), String>,
) -> String {
    let Ok(_resources) = LOGIN_RESOURCE_CLEANUP_LOCK.lock() else {
        return runtime_error;
    };
    match runtime.snapshot() {
        Ok(Some(snapshot)) if snapshot.run_id == run_id => match cleanup() {
            Ok(()) => runtime_error,
            Err(cleanup_error) => format!("{runtime_error}; {cleanup_error}"),
        },
        Ok(_) | Err(_) => runtime_error,
    }
}

fn fail_closed_login_error(
    app: &AppHandle,
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
    runtime_error: String,
) -> String {
    fail_closed_login_error_for_run(runtime, run_id, runtime_error, || {
        release_login_resources_unlocked(app)
    })
}

fn cleanup_login_run(
    app: &AppHandle,
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
    status: LoginRunStatus,
    message: &str,
) -> Result<(), String> {
    match runtime.snapshot() {
        Ok(Some(snapshot)) if snapshot.run_id == run_id => {}
        Ok(_) => return Ok(()),
        Err(runtime_error) => {
            return Err(fail_closed_login_error(app, runtime, run_id, runtime_error));
        }
    }
    let finished = finish_login_run_after_cleanup(runtime, run_id, status, message, || {
        release_login_resources_unlocked(app)
    });
    let finished = match finished {
        Ok(finished) => finished,
        Err(error) => {
            let _ = runtime.update(
                run_id,
                LoginRunStatus::Failed,
                None,
                "登录试运行资源清理失败",
                None,
            );
            return Err(error);
        }
    };
    if let Some(snapshot) = finished {
        emit_run(app, &snapshot);
    }
    emit_current_state(app)
}

fn finish_login_run_after_cleanup(
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
    status: LoginRunStatus,
    message: &str,
    cleanup: impl FnOnce() -> Result<(), String>,
) -> Result<Option<LoginRunSnapshot>, String> {
    let _cleanup = LOGIN_RESOURCE_CLEANUP_LOCK
        .lock()
        .map_err(|_| "登录试运行资源清理锁已损坏".to_string())?;
    match runtime.snapshot()? {
        Some(snapshot) if snapshot.run_id == run_id => {}
        _ => return Ok(None),
    }
    if let Err(error) = cleanup() {
        if let Err(state_error) = runtime.mark_cleanup_failed(run_id) {
            return Err(format!("{error}; {state_error}"));
        }
        return Err(error);
    }
    runtime.finish(run_id, status, message)
}

fn persist_login_result(
    app: &AppHandle,
    account_id: &str,
    result: &login_flow::LoginFlowResult,
    stop_reason: login_runtime::StopReason,
    frozen_signature: &str,
) -> Result<u64, String> {
    let state = app
        .try_state::<SpecialOpsState>()
        .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
    let coordinator = app
        .try_state::<Arc<SettingsCoordinator>>()
        .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
    let (settings, revision) = coordinator.with_runtime_change(|| {
        let current = state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        let mut next = current;
        apply_login_flow_result(&mut next, account_id, result, stop_reason, frozen_signature)?;
        save_settings(app, &next)?;
        *state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok::<_, String>(next)
    })?;
    emit_state(app, &build_bootstrap(settings, revision, now_ms()));
    Ok(revision)
}

fn persist_login_outcome_with<F>(
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
    account_id: &str,
    flow_result: &login_flow::LoginFlowResult,
    frozen_signature: &str,
    persist: F,
) -> Result<Option<login_runtime::PersistenceKind>, String>
where
    F: FnMut(&login_flow::LoginFlowResult, login_runtime::StopReason, &str) -> Result<(), String>,
{
    persist_login_outcome_with_deadline(
        runtime,
        run_id,
        account_id,
        flow_result,
        frozen_signature,
        std::time::Duration::from_secs(5),
        persist,
    )
}

fn persist_login_outcome_with_deadline<F>(
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
    account_id: &str,
    flow_result: &login_flow::LoginFlowResult,
    frozen_signature: &str,
    wait_deadline: std::time::Duration,
    mut persist: F,
) -> Result<Option<login_runtime::PersistenceKind>, String>
where
    F: FnMut(&login_flow::LoginFlowResult, login_runtime::StopReason, &str) -> Result<(), String>,
{
    let deadline = std::time::Instant::now()
        .checked_add(wait_deadline)
        .ok_or_else(|| "登录结果持久化等待期限无效".to_string())?;
    loop {
        match runtime.claim_persistence(run_id)? {
            login_runtime::PersistenceClaim::Pending => {
                let remaining = deadline
                    .checked_duration_since(std::time::Instant::now())
                    .ok_or_else(|| "等待持久化权限超时，保留当前登录试运行".to_string())?;
                runtime.wait_for_persistence_change(
                    run_id,
                    remaining.min(std::time::Duration::from_millis(50)),
                )?;
            }
            login_runtime::PersistenceClaim::Persisted
            | login_runtime::PersistenceClaim::NoPersistence => return Ok(None),
            login_runtime::PersistenceClaim::Stale => {
                return Err("登录试运行持久化任务已过期".to_string());
            }
            login_runtime::PersistenceClaim::NoActive => {
                return Err("登录试运行已不存在，拒绝持久化".to_string());
            }
            login_runtime::PersistenceClaim::Acquired(guard) => {
                let kind = guard.kind();
                let stop_result;
                let (result, reason, signature) = match kind {
                    login_runtime::PersistenceKind::Flow => (
                        flow_result,
                        login_runtime::StopReason::Normal,
                        frozen_signature,
                    ),
                    login_runtime::PersistenceKind::Stop(reason) => {
                        stop_result = login_flow::LoginFlowResult::EmergencyStopped {
                            account_id: account_id.to_string(),
                            stopped_at: now_ms(),
                        };
                        (&stop_result, reason, "")
                    }
                };
                let result = persist(result, reason, signature);
                if let Err(error) = result {
                    guard.fail("登录结果保存失败")?;
                    return Err(error);
                }
                if guard.complete()? {
                    return Ok(Some(kind));
                }
            }
        }
    }
}

fn persist_login_outcome(
    app: &AppHandle,
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
    account_id: &str,
    flow_result: &login_flow::LoginFlowResult,
    frozen_signature: &str,
) -> Result<Option<login_runtime::PersistenceKind>, String> {
    persist_login_outcome_with(
        runtime,
        run_id,
        account_id,
        flow_result,
        frozen_signature,
        |result, reason, signature| {
            persist_login_result(app, account_id, result, reason, signature).map(|_| ())
        },
    )
}

fn login_step_message(step: &login_flow::LoginStep) -> &'static str {
    use login_flow::LoginStep::*;
    match step {
        StopGame => "正在结束旧游戏进程",
        StopWeGame => "正在结束旧 WeGame 进程",
        StartWeGame => "正在启动 WeGame",
        WaitLoginChoice => "正在识别登录入口",
        OpenLoginForm => "正在打开账号密码登录",
        InputAccount => "正在准备输入 QQ 账号",
        InputPassword => "正在准备输入密码",
        SubmitLogin => "正在提交登录",
        WaitGameEntry => "正在等待游戏入口",
        OpenGameEntry => "正在打开游戏入口",
        WaitLaunchButton => "正在等待启动按钮",
        LaunchGame => "正在启动游戏",
        WaitGameWindow => "正在等待游戏窗口",
    }
}

fn cleanup_login_worker_after_persistence<T>(
    persist_result: &Result<T, String>,
    cleanup: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match persist_result {
        Ok(_) => cleanup(),
        Err(error) => Err(error.clone()),
    }
}

async fn run_login_worker(
    app: AppHandle,
    runtime: Arc<login_runtime::LoginRuntime>,
    run_id: u64,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    config: Arc<login_flow::LoginRunConfig>,
    frozen_signature: String,
) {
    let driver = login_runtime::ProductionLoginDriver::new(
        app.clone(),
        Arc::clone(&runtime),
        run_id,
        Arc::clone(&config),
    );
    let update_app = app.clone();
    let update_runtime = Arc::clone(&runtime);
    let update_cancelled = Arc::clone(&cancelled);
    let result =
        login_flow::run_login_flow(
            &driver,
            &config,
            cancelled,
            move |step| match update_runtime.update(
                run_id,
                LoginRunStatus::Waiting,
                Some(step.clone()),
                login_step_message(&step),
                None,
            ) {
                Ok(Some(snapshot)) => emit_run(&update_app, &snapshot),
                Ok(None) | Err(_) => {
                    update_cancelled.store(true, std::sync::atomic::Ordering::SeqCst)
                }
            },
        )
        .await;

    let persist_result = persist_login_outcome(
        &app,
        &runtime,
        run_id,
        &config.account_id,
        &result,
        &frozen_signature,
    );
    if persist_result.is_err() {
        if let Ok(Some(snapshot)) = runtime.snapshot() {
            if snapshot.run_id == run_id {
                emit_run(&app, &snapshot);
            }
        }
    }
    let stopped = runtime.stop_reason(run_id).ok().flatten().is_some();
    let (status, message) = if stopped {
        (LoginRunStatus::Stopped, "登录试运行已停止")
    } else {
        match result {
            login_flow::LoginFlowResult::GameReady { .. } => {
                (LoginRunStatus::Succeeded, "登录试运行成功")
            }
            login_flow::LoginFlowResult::Paused { .. } => {
                (LoginRunStatus::Failed, "登录试运行失败，已暂停自动化")
            }
            login_flow::LoginFlowResult::EmergencyStopped { .. } => {
                (LoginRunStatus::Stopped, "登录试运行已停止")
            }
        }
    };
    if let Err(error) = cleanup_login_worker_after_persistence(&persist_result, || {
        cleanup_login_run(&app, &runtime, run_id, status, message)
    }) {
        crate::log_error!(
            "special_ops::login",
            "登录试运行持久化或清理失败",
            "error" => error
        );
    }
}

pub fn initialize(app: &AppHandle) -> Result<SpecialOpsState, String> {
    let settings = load_settings(app)?;
    Ok(SpecialOpsState {
        settings: Arc::new(Mutex::new(settings)),
        login_runtime: Arc::new(login_runtime::LoginRuntime::default()),
    })
}

pub(crate) fn stop_registered(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<SpecialOpsState>() else {
        return Ok(());
    };
    let active = state.login_runtime.snapshot()?;
    if let Some(snapshot) = active {
        let Some(stopped) = emit_login_run_change(app, &state.login_runtime, || {
            state.login_runtime.request_lifecycle_stop(snapshot.run_id)
        })
        .map_err(|error| {
            fail_closed_login_error(app, &state.login_runtime, snapshot.run_id, error)
        })?
        else {
            return Ok(());
        };
        let resource_result =
            release_login_resources_for_run(app, &state.login_runtime, stopped.run_id);
        let result = login_flow::LoginFlowResult::EmergencyStopped {
            account_id: stopped.account_id.clone(),
            stopped_at: now_ms(),
        };
        let persist_result = persist_login_outcome(
            app,
            &state.login_runtime,
            stopped.run_id,
            &stopped.account_id,
            &result,
            "",
        );
        let mut errors = Vec::new();
        if let Err(error) = resource_result {
            errors.push(error);
        }
        if let Err(error) = persist_result {
            errors.push(error);
        }
        if !errors.is_empty() {
            return Err(errors.join("; "));
        }
        if let Some(finished) = state.login_runtime.finish(
            stopped.run_id,
            LoginRunStatus::Stopped,
            "登录试运行已停止",
        )? {
            emit_run(app, &finished);
        }
        return Ok(());
    }
    let coordinator = app
        .try_state::<Arc<SettingsCoordinator>>()
        .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
    let (settings, revision) = coordinator.with_runtime_change(|| {
        let current = state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        let mut next = current;
        next.paused = true;
        save_settings(app, &next)?;
        *state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok::<_, String>(next)
    })?;
    emit_state(app, &build_bootstrap(settings, revision, now_ms()));
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
    let bootstrap = build_bootstrap_with_runtime(
        settings,
        settings_coordinator.current_revision()?,
        now_ms(),
        &state.login_runtime,
    )?;
    Ok(bootstrap)
}

#[tauri::command]
pub async fn special_ops_start_login_trial(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    settings_revision: u64,
) -> Result<LoginRunSnapshot, AppError> {
    let runtime = Arc::clone(&state.login_runtime);
    let snapshot =
        settings_coordinator.with_revision(settings_revision, || -> Result<_, String> {
            let settings = state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            let (config, frozen_signature) = freeze_login_run_config(&settings, &account_id)?;
            let config = Arc::new(config);
            let worker_app = app.clone();
            let worker_runtime = Arc::clone(&runtime);
            let worker_config = Arc::clone(&config);
            let worker_signature = frozen_signature.clone();
            let registered_hotkey = settings.emergency_hotkey.clone();
            let operation_hotkey = settings.emergency_hotkey.clone();
            let (_, snapshot) = start_login_run_with_resources(
                &runtime,
                account_id.clone(),
                || register_emergency_hotkey(&app, registered_hotkey),
                || create_operation_window(&app, &operation_hotkey),
                |snapshot| emit_run(&app, snapshot),
                || release_login_resources_unlocked(&app),
                |started| {
                    let run_id = started.run_id;
                    let cancelled = Arc::clone(&started.cancelled);
                    tauri::async_runtime::spawn(async move {
                        run_login_worker(
                            worker_app,
                            worker_runtime,
                            run_id,
                            cancelled,
                            worker_config,
                            worker_signature,
                        )
                        .await;
                    });
                    Ok(())
                },
            )?;
            Ok(snapshot)
        })?;
    Ok(snapshot)
}

#[tauri::command]
pub fn special_ops_cancel_login_trial(
    state: State<'_, SpecialOpsState>,
    app: AppHandle,
) -> Result<LoginRunSnapshot, AppError> {
    let cleanup_retry = state
        .login_runtime
        .snapshot()?
        .map(|snapshot| {
            state
                .login_runtime
                .cleanup_failed(snapshot.run_id)
                .map(|failed| (snapshot, failed))
        })
        .transpose()?;
    let snapshot = match cleanup_retry {
        Some((snapshot, true)) => {
            let finished = finish_login_run_after_cleanup(
                &state.login_runtime,
                snapshot.run_id,
                LoginRunStatus::Failed,
                "登录试运行清理完成",
                || release_login_resources_unlocked(&app),
            )?
            .ok_or_else(|| "登录试运行状态已变化".to_string())?;
            emit_run(&app, &finished);
            return Ok(finished);
        }
        Some((snapshot, false)) => snapshot,
        None => return Err("当前没有运行中的登录试运行".into()),
    };
    let stopped = emit_login_run_change(&app, &state.login_runtime, || {
        state
            .login_runtime
            .request_stop(snapshot.run_id, login_runtime::StopReason::Normal)
    })?
    .ok_or_else(|| "登录试运行状态已变化".to_string())?;
    Ok(stopped)
}

fn emergency_stop_core(app: &AppHandle) -> Result<LoginRunSnapshot, String> {
    let state = app
        .try_state::<SpecialOpsState>()
        .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
    let active = state
        .login_runtime
        .snapshot()?
        .ok_or_else(|| "当前没有运行中的登录试运行".to_string())?;
    let snapshot = emit_login_run_change(app, &state.login_runtime, || {
        state
            .login_runtime
            .request_stop(active.run_id, login_runtime::StopReason::Emergency)
    })
    .map_err(|error| fail_closed_login_error(app, &state.login_runtime, active.run_id, error))?
    .ok_or_else(|| "登录试运行状态已变化".to_string())?;
    let resource_result =
        release_login_resources_for_run(app, &state.login_runtime, snapshot.run_id);
    let result = login_flow::LoginFlowResult::EmergencyStopped {
        account_id: snapshot.account_id.clone(),
        stopped_at: now_ms(),
    };
    persist_login_outcome(
        app,
        &state.login_runtime,
        snapshot.run_id,
        &snapshot.account_id,
        &result,
        "",
    )?;
    resource_result?;
    if let Some(finished) =
        state
            .login_runtime
            .finish(snapshot.run_id, LoginRunStatus::Stopped, "登录试运行已停止")?
    {
        emit_run(app, &finished);
        return Ok(finished);
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn special_ops_emergency_stop(app: AppHandle) -> Result<LoginRunSnapshot, AppError> {
    emergency_stop_core(&app).map_err(AppError::from)
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
                let bootstrap = build_bootstrap_with_runtime(
                    settings_value,
                    settings_revision,
                    now_ms(),
                    &state.login_runtime,
                )?;
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
                let bootstrap = build_bootstrap_with_runtime(
                    settings,
                    settings_revision,
                    now_ms(),
                    &state.login_runtime,
                )?;
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

    fn persist_test_login_result(
        coordinator: &SettingsCoordinator,
        settings: &Mutex<SpecialOpsSettings>,
        account_id: &str,
        result: &login_flow::LoginFlowResult,
        reason: login_runtime::StopReason,
        frozen_signature: &str,
        fail: bool,
    ) -> Result<(), String> {
        coordinator
            .with_runtime_change(|| {
                let mut next = settings.lock().unwrap().clone();
                apply_login_flow_result(&mut next, account_id, result, reason, frozen_signature)?;
                if fail {
                    return Err("测试持久化失败".to_string());
                }
                *settings.lock().unwrap() = next;
                Ok::<_, String>(())
            })
            .map(|_| ())
    }

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
            settings.accounts[0].qq_account = "10001".to_string();
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
            for key in [
                "wegame.accountDropdown",
                "wegame.accountList",
                "wegame.selectedAccount",
            ] {
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
            qq_account: format!(
                "10{:05}",
                id.bytes().map(u32::from).sum::<u32>()
            ),
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
    fn trial_success_signature_changes_when_account_or_calibration_changes() {
        let fixture = LoginFixture::complete();
        let settings = fixture.settings;
        let account = settings.accounts.first().unwrap();
        let original = login_trial_signature(&settings, account).unwrap();

        let mut account_changed = settings.clone();
        account_changed.accounts[0].qq_account.push('9');
        assert_ne!(
            original,
            login_trial_signature(&account_changed, &account_changed.accounts[0]).unwrap()
        );

        let mut calibration_changed = settings.clone();
        calibration_changed.calibration_environments[0].targets[0]
            .rect
            .as_mut()
            .unwrap()
            .x += 1;
        assert_ne!(
            original,
            login_trial_signature(&calibration_changed, &calibration_changed.accounts[0]).unwrap()
        );
    }

    #[test]
    fn emergency_stop_marks_current_account_uncertain() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        apply_login_flow_result(
            &mut settings,
            "selected",
            &login_flow::LoginFlowResult::EmergencyStopped {
                account_id: "selected".to_string(),
                stopped_at: 42,
            },
            login_runtime::StopReason::Emergency,
            "v1-deadbeef",
        )
        .unwrap();

        assert!(settings.paused);
        assert_eq!(settings.accounts[0].status, AccountStatus::Uncertain);
    }

    #[test]
    fn emergency_claim_prevents_concurrent_worker_from_persisting_stale_ready() {
        let runtime = Arc::new(login_runtime::LoginRuntime::default());
        let started = runtime.try_start("selected".to_string()).unwrap();
        let run_id = started.run_id;
        runtime
            .request_stop(run_id, login_runtime::StopReason::Emergency)
            .unwrap()
            .unwrap();
        let settings = Arc::new(Mutex::new(LoginFixture::complete().settings));
        let coordinator = Arc::new(SettingsCoordinator::new());
        let initial_revision = coordinator.current_revision().unwrap();
        let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let claimed = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));

        let emergency_runtime = Arc::clone(&runtime);
        let emergency_settings = Arc::clone(&settings);
        let emergency_coordinator = Arc::clone(&coordinator);
        let emergency_writes = Arc::clone(&writes);
        let emergency_claimed = Arc::clone(&claimed);
        let emergency_release = Arc::clone(&release);
        let emergency = std::thread::spawn(move || {
            persist_login_outcome_with(
                &emergency_runtime,
                run_id,
                "selected",
                &login_flow::LoginFlowResult::GameReady {
                    account_id: "selected".to_string(),
                    qq_account: "10001".to_string(),
                    game_process_id: 1,
                    game_window_handle: 2,
                },
                "stale-flow-signature",
                |result, reason, signature| {
                    emergency_claimed.wait();
                    emergency_release.wait();
                    emergency_writes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    persist_test_login_result(
                        &emergency_coordinator,
                        &emergency_settings,
                        "selected",
                        result,
                        reason,
                        signature,
                        false,
                    )
                },
            )
        });
        claimed.wait();

        let worker_runtime = Arc::clone(&runtime);
        let worker_settings = Arc::clone(&settings);
        let worker_coordinator = Arc::clone(&coordinator);
        let worker_writes = Arc::clone(&writes);
        let worker = std::thread::spawn(move || {
            persist_login_outcome_with(
                &worker_runtime,
                run_id,
                "selected",
                &login_flow::LoginFlowResult::GameReady {
                    account_id: "selected".to_string(),
                    qq_account: "10001".to_string(),
                    game_process_id: 3,
                    game_window_handle: 4,
                },
                "stale-worker-signature",
                |result, reason, signature| {
                    worker_writes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    persist_test_login_result(
                        &worker_coordinator,
                        &worker_settings,
                        "selected",
                        result,
                        reason,
                        signature,
                        false,
                    )
                },
            )
        });
        release.wait();

        assert!(emergency.join().unwrap().unwrap().is_some());
        assert!(worker.join().unwrap().unwrap().is_none());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(
            coordinator.current_revision().unwrap(),
            initial_revision + 1
        );
        let settings = settings.lock().unwrap();
        assert!(settings.paused);
        assert_eq!(settings.accounts[0].status, AccountStatus::Uncertain);
    }

    #[test]
    fn concurrent_emergency_requests_persist_once() {
        let runtime = Arc::new(login_runtime::LoginRuntime::default());
        let started = runtime.try_start("selected".to_string()).unwrap();
        let run_id = started.run_id;
        runtime
            .request_stop(run_id, login_runtime::StopReason::Emergency)
            .unwrap()
            .unwrap();
        let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let coordinator = Arc::new(SettingsCoordinator::new());
        let initial_revision = coordinator.current_revision().unwrap();
        let claimed = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));

        let first_runtime = Arc::clone(&runtime);
        let first_writes = Arc::clone(&writes);
        let first_claimed = Arc::clone(&claimed);
        let first_release = Arc::clone(&release);
        let first_coordinator = Arc::clone(&coordinator);
        let first = std::thread::spawn(move || {
            persist_login_outcome_with(
                &first_runtime,
                run_id,
                "selected",
                &login_flow::LoginFlowResult::EmergencyStopped {
                    account_id: "selected".to_string(),
                    stopped_at: 1,
                },
                "",
                |_, _, _| {
                    first_claimed.wait();
                    first_release.wait();
                    first_writes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    first_coordinator
                        .with_runtime_change(|| Ok::<_, String>(()))
                        .map(|_| ())
                },
            )
        });
        claimed.wait();

        let second_runtime = Arc::clone(&runtime);
        let second_writes = Arc::clone(&writes);
        let second = std::thread::spawn(move || {
            second_runtime
                .request_stop(run_id, login_runtime::StopReason::Emergency)
                .unwrap()
                .unwrap();
            persist_login_outcome_with(
                &second_runtime,
                run_id,
                "selected",
                &login_flow::LoginFlowResult::EmergencyStopped {
                    account_id: "selected".to_string(),
                    stopped_at: 2,
                },
                "",
                |_, _, _| {
                    second_writes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
            )
        });
        release.wait();

        assert!(first.join().unwrap().unwrap().is_some());
        assert!(second.join().unwrap().unwrap().is_none());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(
            coordinator.current_revision().unwrap(),
            initial_revision + 1
        );
    }

    #[test]
    fn worker_retries_authoritative_emergency_after_command_persistence_failure() {
        let runtime = login_runtime::LoginRuntime::default();
        let started = runtime.try_start("selected".to_string()).unwrap();
        runtime
            .request_stop(started.run_id, login_runtime::StopReason::Emergency)
            .unwrap()
            .unwrap();
        let settings = Mutex::new(LoginFixture::complete().settings);
        let coordinator = SettingsCoordinator::new();
        let initial_revision = coordinator.current_revision().unwrap();
        let stale_ready = login_flow::LoginFlowResult::GameReady {
            account_id: "selected".to_string(),
            qq_account: "10001".to_string(),
            game_process_id: 1,
            game_window_handle: 2,
        };

        let failed = persist_login_outcome_with(
            &runtime,
            started.run_id,
            "selected",
            &stale_ready,
            "stale-signature",
            |result, reason, signature| {
                persist_test_login_result(
                    &coordinator,
                    &settings,
                    "selected",
                    result,
                    reason,
                    signature,
                    true,
                )
            },
        );
        assert_eq!(failed.unwrap_err(), "测试持久化失败");

        let retried = persist_login_outcome_with(
            &runtime,
            started.run_id,
            "selected",
            &stale_ready,
            "stale-signature",
            |result, reason, signature| {
                persist_test_login_result(
                    &coordinator,
                    &settings,
                    "selected",
                    result,
                    reason,
                    signature,
                    false,
                )
            },
        )
        .unwrap();

        assert_eq!(
            retried,
            Some(login_runtime::PersistenceKind::Stop(
                login_runtime::StopReason::Emergency
            ))
        );
        assert_eq!(
            coordinator.current_revision().unwrap(),
            initial_revision + 1
        );
        assert_eq!(
            settings.lock().unwrap().accounts[0].status,
            AccountStatus::Uncertain
        );
    }

    #[test]
    fn persistence_wait_has_total_deadline_and_keeps_active_run() {
        let runtime = login_runtime::LoginRuntime::default();
        let started = runtime.try_start("selected".to_string()).unwrap();
        let login_runtime::PersistenceClaim::Acquired(_held) =
            runtime.claim_persistence(started.run_id).unwrap()
        else {
            panic!("应先占用持久化权限");
        };
        let result = login_flow::LoginFlowResult::GameReady {
            account_id: "selected".to_string(),
            qq_account: "10001".to_string(),
            game_process_id: 1,
            game_window_handle: 2,
        };

        let error = persist_login_outcome_with_deadline(
            &runtime,
            started.run_id,
            "selected",
            &result,
            "signature",
            std::time::Duration::from_millis(10),
            |_, _, _| panic!("等待超时前不得进入持久化 closure"),
        )
        .unwrap_err();

        assert!(error.contains("等待持久化权限超时"));
        assert!(runtime.snapshot().unwrap().is_some());
        assert!(runtime.try_start("next".to_string()).is_err());
    }

    #[test]
    fn failed_worker_persistence_skips_cleanup_and_keeps_active_run() {
        let runtime = login_runtime::LoginRuntime::default();
        let started = runtime.try_start("selected".to_string()).unwrap();
        let cleanup_calls = std::sync::atomic::AtomicUsize::new(0);
        let persist_result: Result<Option<login_runtime::PersistenceKind>, String> =
            Err("测试持久化失败".to_string());

        let error = cleanup_login_worker_after_persistence(&persist_result, || {
            cleanup_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            runtime.finish(started.run_id, LoginRunStatus::Failed, "不应执行 cleanup")?;
            Ok(())
        })
        .unwrap_err();

        assert_eq!(error, "测试持久化失败");
        assert_eq!(cleanup_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(runtime.snapshot().unwrap().unwrap().run_id, started.run_id);
        assert!(runtime.try_start("next".to_string()).is_err());
    }

    #[test]
    fn pending_emergency_takes_over_after_worker_flow_persistence_failure() {
        let runtime = Arc::new(login_runtime::LoginRuntime::default());
        let started = runtime.try_start("selected".to_string()).unwrap();
        let run_id = started.run_id;
        let settings = Arc::new(Mutex::new(LoginFixture::complete().settings));
        let coordinator = Arc::new(SettingsCoordinator::new());
        let initial_revision = coordinator.current_revision().unwrap();
        let stale_ready = login_flow::LoginFlowResult::GameReady {
            account_id: "selected".to_string(),
            qq_account: "10001".to_string(),
            game_process_id: 1,
            game_window_handle: 2,
        };
        let (claimed_tx, claimed_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();

        let worker_runtime = Arc::clone(&runtime);
        let worker_result = stale_ready.clone();
        let worker = std::thread::spawn(move || {
            persist_login_outcome_with(
                &worker_runtime,
                run_id,
                "selected",
                &worker_result,
                "stale-signature",
                |_, _, _| {
                    claimed_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Err("worker 写入失败".to_string())
                },
            )
        });
        claimed_rx.recv().unwrap();
        runtime
            .request_stop(run_id, login_runtime::StopReason::Emergency)
            .unwrap()
            .unwrap();

        let emergency_runtime = Arc::clone(&runtime);
        let emergency_settings = Arc::clone(&settings);
        let emergency_coordinator = Arc::clone(&coordinator);
        let emergency = std::thread::spawn(move || {
            persist_login_outcome_with(
                &emergency_runtime,
                run_id,
                "selected",
                &login_flow::LoginFlowResult::EmergencyStopped {
                    account_id: "selected".to_string(),
                    stopped_at: 42,
                },
                "",
                |result, reason, signature| {
                    persist_test_login_result(
                        &emergency_coordinator,
                        &emergency_settings,
                        "selected",
                        result,
                        reason,
                        signature,
                        false,
                    )
                },
            )
        });
        release_tx.send(()).unwrap();

        assert_eq!(worker.join().unwrap().unwrap_err(), "worker 写入失败");
        assert_eq!(
            emergency.join().unwrap().unwrap(),
            Some(login_runtime::PersistenceKind::Stop(
                login_runtime::StopReason::Emergency
            ))
        );
        assert_eq!(
            coordinator.current_revision().unwrap(),
            initial_revision + 1
        );
        let settings = settings.lock().unwrap();
        assert!(settings.paused);
        assert_eq!(settings.accounts[0].status, AccountStatus::Uncertain);
        drop(settings);
        assert_eq!(
            runtime.snapshot().unwrap().unwrap().status,
            LoginRunStatus::Failed
        );
        assert!(runtime.try_start("next".to_string()).is_err());
    }

    #[test]
    fn start_announces_starting_before_synchronous_worker_update() {
        let runtime = login_runtime::LoginRuntime::default();
        let events = std::sync::Mutex::new(Vec::new());

        let (_, snapshot) = start_login_run_with_resources(
            &runtime,
            "selected".to_string(),
            || Ok(()),
            || Ok(()),
            |snapshot| events.lock().unwrap().push(snapshot.status),
            || Ok(()),
            |started| {
                let waiting = runtime
                    .update(
                        started.run_id,
                        LoginRunStatus::Waiting,
                        None,
                        "worker 已启动",
                        None,
                    )?
                    .ok_or_else(|| "登录试运行状态已变化".to_string())?;
                events.lock().unwrap().push(waiting.status);
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(
            events.into_inner().unwrap(),
            vec![LoginRunStatus::Starting, LoginRunStatus::Waiting]
        );
        assert_eq!(snapshot.status, LoginRunStatus::Waiting);
    }

    #[test]
    fn cancel_while_window_creation_is_blocked_never_announces_starting_after_stopped() {
        let runtime = Arc::new(login_runtime::LoginRuntime::default());
        let events = Arc::new(Mutex::new(Vec::new()));
        let spawn_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (create_entered_tx, create_entered_rx) = std::sync::mpsc::channel();
        let (create_release_tx, create_release_rx) = std::sync::mpsc::channel();

        let start_runtime = Arc::clone(&runtime);
        let start_events = Arc::clone(&events);
        let start_spawn_calls = Arc::clone(&spawn_calls);
        let start = std::thread::spawn(move || {
            start_login_run_with_resources(
                &start_runtime,
                "selected".to_string(),
                || Ok(()),
                || {
                    create_entered_tx.send(()).unwrap();
                    create_release_rx.recv().unwrap();
                    Ok(())
                },
                |snapshot| start_events.lock().unwrap().push(snapshot.status),
                || Ok(()),
                |_| {
                    start_spawn_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
            )
        });

        create_entered_rx.recv().unwrap();
        let run_id = runtime.snapshot().unwrap().unwrap().run_id;
        let stopped = runtime
            .with_event_serialized(|| {
                let stopped = runtime
                    .request_stop(run_id, login_runtime::StopReason::Normal)?
                    .ok_or_else(|| "登录试运行状态已变化".to_string())?;
                events.lock().unwrap().push(stopped.status);
                Ok(stopped)
            })
            .unwrap();
        create_release_tx.send(()).unwrap();

        assert_eq!(stopped.status, LoginRunStatus::Stopped);
        assert!(start.join().unwrap().is_err());
        assert_eq!(
            events.lock().unwrap().as_slice(),
            &[LoginRunStatus::Stopped]
        );
        assert_eq!(spawn_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(runtime.snapshot().unwrap().is_none());
    }

    #[test]
    fn lifecycle_stop_during_start_prevents_window_and_worker_handoff() {
        let runtime = Arc::new(login_runtime::LoginRuntime::default());
        let create_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let spawn_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cleanup_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (register_entered_tx, register_entered_rx) = std::sync::mpsc::channel();
        let (register_release_tx, register_release_rx) = std::sync::mpsc::channel();

        let start_runtime = Arc::clone(&runtime);
        let start_create_calls = Arc::clone(&create_calls);
        let start_spawn_calls = Arc::clone(&spawn_calls);
        let start_cleanup_calls = Arc::clone(&cleanup_calls);
        let start = std::thread::spawn(move || {
            start_login_run_with_resources(
                &start_runtime,
                "selected".to_string(),
                || {
                    register_entered_tx.send(()).unwrap();
                    register_release_rx.recv().unwrap();
                    Ok(())
                },
                || {
                    start_create_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
                |_| {},
                || {
                    start_cleanup_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
                |_| {
                    start_spawn_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
            )
        });

        register_entered_rx.recv().unwrap();
        let stopped = runtime
            .request_stop(
                runtime.snapshot().unwrap().unwrap().run_id,
                login_runtime::StopReason::Lifecycle { uncertain: false },
            )
            .unwrap()
            .unwrap();
        register_release_tx.send(()).unwrap();

        assert!(start
            .join()
            .unwrap()
            .unwrap_err()
            .contains("启动期间已停止"));
        assert_eq!(create_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(spawn_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(cleanup_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(runtime.snapshot().unwrap().unwrap().run_id, stopped.run_id);
        runtime
            .finish(stopped.run_id, LoginRunStatus::Stopped, "生命周期停止完成")
            .unwrap();
        assert!(runtime.snapshot().unwrap().is_none());
    }

    #[test]
    fn stale_fail_closed_error_does_not_cleanup_new_run_resources() {
        let runtime = login_runtime::LoginRuntime::default();
        let old = runtime.try_start("old".to_string()).unwrap();
        runtime
            .finish(old.run_id, LoginRunStatus::Failed, "旧 run 结束")
            .unwrap();
        let current = runtime.try_start("current".to_string()).unwrap();
        let cleanup_calls = std::sync::atomic::AtomicUsize::new(0);

        let error = fail_closed_login_error_for_run(
            &runtime,
            old.run_id,
            "旧 run 状态错误".to_string(),
            || {
                cleanup_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            },
        );

        assert_eq!(error, "旧 run 状态错误");
        assert_eq!(cleanup_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(runtime.snapshot().unwrap().unwrap().run_id, current.run_id);
    }

    #[test]
    fn worker_finishes_authoritative_stop_persistence_before_cleanup_when_stop_arrives_mid_write() {
        let runtime = login_runtime::LoginRuntime::default();
        let started = runtime.try_start("selected".to_string()).unwrap();
        let settings = Mutex::new(LoginFixture::complete().settings);
        let coordinator = SettingsCoordinator::new();
        let initial_revision = coordinator.current_revision().unwrap();
        let writes = std::sync::atomic::AtomicUsize::new(0);
        let stale_ready = login_flow::LoginFlowResult::GameReady {
            account_id: "selected".to_string(),
            qq_account: "10001".to_string(),
            game_process_id: 1,
            game_window_handle: 2,
        };

        let persisted = persist_login_outcome_with(
            &runtime,
            started.run_id,
            "selected",
            &stale_ready,
            "stale-signature",
            |result, reason, signature| {
                persist_test_login_result(
                    &coordinator,
                    &settings,
                    "selected",
                    result,
                    reason,
                    signature,
                    false,
                )?;
                if writes.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    runtime
                        .request_stop(started.run_id, login_runtime::StopReason::Emergency)?
                        .ok_or_else(|| "登录试运行状态已变化".to_string())?;
                }
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(
            persisted,
            Some(login_runtime::PersistenceKind::Stop(
                login_runtime::StopReason::Emergency
            ))
        );
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(
            coordinator.current_revision().unwrap(),
            initial_revision + 2
        );
        assert_eq!(
            settings.lock().unwrap().accounts[0].status,
            AccountStatus::Uncertain
        );
    }

    #[test]
    fn resource_cleanup_attempts_hotkey_and_window_and_aggregates_errors() {
        for (hotkey_fails, window_fails) in [(true, false), (false, true), (true, true)] {
            let calls = Mutex::new(Vec::new());
            let error = release_login_resources_with(
                || calls.lock().unwrap().push("inputs"),
                || {
                    calls.lock().unwrap().push("hotkey");
                    if hotkey_fails {
                        Err("热键清理失败".to_string())
                    } else {
                        Ok(())
                    }
                },
                || {
                    calls.lock().unwrap().push("window");
                    if window_fails {
                        Err("窗口销毁失败".to_string())
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();

            assert_eq!(*calls.lock().unwrap(), ["inputs", "hotkey", "window"]);
            assert_eq!(error.contains("热键清理失败"), hotkey_fails);
            assert_eq!(error.contains("窗口销毁失败"), window_fails);
        }
    }

    #[test]
    fn cleanup_failure_keeps_singleton_until_retry_succeeds() {
        let runtime = login_runtime::LoginRuntime::default();
        let started = runtime.try_start("selected".to_string()).unwrap();

        let error = finish_login_run_after_cleanup(
            &runtime,
            started.run_id,
            LoginRunStatus::Stopped,
            "已停止",
            || Err("窗口销毁失败".to_string()),
        )
        .unwrap_err();

        assert_eq!(error, "窗口销毁失败");
        assert!(runtime.cleanup_failed(started.run_id).unwrap());
        assert!(runtime.try_start("next".to_string()).is_err());
        assert!(finish_login_run_after_cleanup(
            &runtime,
            started.run_id,
            LoginRunStatus::Stopped,
            "已停止",
            || Ok(()),
        )
        .unwrap()
        .is_some());
        assert!(runtime.try_start("next".to_string()).is_ok());
    }

    #[test]
    fn stale_cleanup_does_not_touch_current_run_resources() {
        let runtime = login_runtime::LoginRuntime::default();
        let old = runtime.try_start("old".to_string()).unwrap();
        runtime
            .finish(old.run_id, LoginRunStatus::Stopped, "旧任务完成")
            .unwrap();
        let current = runtime.try_start("current".to_string()).unwrap();
        let cleanup_calls = std::sync::atomic::AtomicUsize::new(0);

        let cleaned = finish_login_run_after_cleanup(
            &runtime,
            old.run_id,
            LoginRunStatus::Stopped,
            "旧任务完成",
            || {
                cleanup_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap();

        assert!(cleaned.is_none());
        assert_eq!(cleanup_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(runtime.snapshot().unwrap().unwrap().run_id, current.run_id);
    }

    #[test]
    fn concurrent_old_cleanup_cannot_touch_resources_after_new_run_starts() {
        let runtime = Arc::new(login_runtime::LoginRuntime::default());
        let old = runtime.try_start("old".to_string()).unwrap();
        let (first_started_tx, first_started_rx) = std::sync::mpsc::channel();
        let (release_first_tx, release_first_rx) = std::sync::mpsc::channel();
        let first_runtime = Arc::clone(&runtime);
        let first = std::thread::spawn(move || {
            finish_login_run_after_cleanup(
                &first_runtime,
                old.run_id,
                LoginRunStatus::Stopped,
                "旧任务完成",
                || {
                    first_started_tx.send(()).unwrap();
                    release_first_rx.recv().unwrap();
                    Ok(())
                },
            )
        });
        first_started_rx.recv().unwrap();

        let touched = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let second_touched = Arc::clone(&touched);
        let (second_started_tx, second_started_rx) = std::sync::mpsc::channel();
        let (release_second_tx, release_second_rx) = std::sync::mpsc::channel();
        let second_runtime = Arc::clone(&runtime);
        let second = std::thread::spawn(move || {
            finish_login_run_after_cleanup(
                &second_runtime,
                old.run_id,
                LoginRunStatus::Stopped,
                "旧任务完成",
                || {
                    second_started_tx.send(()).unwrap();
                    release_second_rx.recv().unwrap();
                    second_touched.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
            )
        });
        let second_entered_before_finish = second_started_rx
            .recv_timeout(std::time::Duration::from_millis(500))
            .is_ok();

        release_first_tx.send(()).unwrap();
        assert!(first.join().unwrap().unwrap().is_some());
        let current = runtime.try_start("current".to_string()).unwrap();
        let _ = release_second_tx.send(());
        assert!(second.join().unwrap().unwrap().is_none());

        assert!(!second_entered_before_finish);
        assert_eq!(touched.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(runtime.snapshot().unwrap().unwrap().run_id, current.run_id);
    }

    #[test]
    fn normal_cancel_does_not_change_account_or_pause_state() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        settings.paused = false;

        apply_login_flow_result(
            &mut settings,
            "selected",
            &login_flow::LoginFlowResult::EmergencyStopped {
                account_id: "selected".to_string(),
                stopped_at: 42,
            },
            login_runtime::StopReason::Normal,
            "",
        )
        .unwrap();

        assert!(!settings.paused);
        assert_eq!(settings.accounts[0].status, AccountStatus::Ready);
        assert_eq!(settings.accounts[0].last_failure, None);
    }

    #[test]
    fn lifecycle_stop_pauses_and_only_marks_uncertain_after_input_started() {
        let fixture = LoginFixture::complete();
        let mut safe = fixture.settings.clone();
        safe.paused = false;
        apply_login_flow_result(
            &mut safe,
            "selected",
            &login_flow::LoginFlowResult::EmergencyStopped {
                account_id: "selected".to_string(),
                stopped_at: 42,
            },
            login_runtime::StopReason::Lifecycle { uncertain: false },
            "",
        )
        .unwrap();
        assert!(safe.paused);
        assert_eq!(safe.accounts[0].status, AccountStatus::Ready);

        let mut unsafe_settings = fixture.settings;
        apply_login_flow_result(
            &mut unsafe_settings,
            "selected",
            &login_flow::LoginFlowResult::EmergencyStopped {
                account_id: "selected".to_string(),
                stopped_at: 42,
            },
            login_runtime::StopReason::Lifecycle { uncertain: true },
            "",
        )
        .unwrap();
        assert!(unsafe_settings.paused);
        assert_eq!(unsafe_settings.accounts[0].status, AccountStatus::Uncertain);
    }

    #[test]
    fn paused_result_records_sanitized_failure_and_pauses_only_current_account() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        settings.paused = false;
        settings
            .accounts
            .push(account("other", AccountStatus::Ready, Vec::new()));

        apply_login_flow_result(
            &mut settings,
            "selected",
            &login_flow::LoginFlowResult::Paused {
                failed_step: login_flow::LoginStep::WaitGameEntry,
                last_observation: "最后识别结果：截图失败".to_string(),
                failed_at: 99,
            },
            login_runtime::StopReason::Emergency,
            "",
        )
        .unwrap();

        assert!(settings.paused);
        assert_eq!(settings.accounts[0].status, AccountStatus::LoginFailed);
        assert_eq!(settings.accounts[1].status, AccountStatus::Ready);
        let failure = settings.accounts[0].last_failure.as_ref().unwrap();
        assert_eq!(failure.step, "WaitGameEntry");
        assert_eq!(failure.message, "最后识别结果：截图失败");
    }

    #[test]
    fn game_ready_writes_frozen_signature_and_clears_failure() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        settings.accounts[0].status = AccountStatus::LoginFailed;
        settings.accounts[0].last_failure = Some(AccountFailure {
            step: "old".to_string(),
            message: "old".to_string(),
            at_ms: 1,
        });

        apply_login_flow_result(
            &mut settings,
            "selected",
            &login_flow::LoginFlowResult::GameReady {
                account_id: "selected".to_string(),
                qq_account: "selected".to_string(),
                game_process_id: 1,
                game_window_handle: 2,
            },
            login_runtime::StopReason::Emergency,
            "login-v1-frozen",
        )
        .unwrap();

        assert_eq!(settings.accounts[0].status, AccountStatus::Ready);
        assert_eq!(settings.accounts[0].last_failure, None);
        assert_eq!(
            settings.accounts[0].login_trial_signature.as_deref(),
            Some("login-v1-frozen")
        );
    }

    #[test]
    fn trial_signature_ignores_schedule_and_ammo_runtime_fields() {
        let fixture = LoginFixture::complete();
        let settings = fixture.settings;
        let original = login_trial_signature(&settings, &settings.accounts[0]).unwrap();
        let mut changed = settings.clone();
        changed.daily_exchange_time = "21:30".to_string();
        changed.accounts[0].ammo_targets.push(AmmoTarget {
            id: "ammo".to_string(),
            name: "测试子弹".to_string(),
            enabled: true,
            seasonal: true,
            scroll_steps: 7,
            order: 0,
            last_success_day: Some("2026-07-25".to_string()),
            retry_count: 3,
        });

        assert_eq!(
            original,
            login_trial_signature(&changed, &changed.accounts[0]).unwrap()
        );
    }

    #[test]
    fn bootstrap_contains_current_run_snapshot() {
        let runtime = login_runtime::LoginRuntime::default();
        let started = runtime.try_start("selected".to_string()).unwrap();

        let bootstrap =
            build_bootstrap_with_runtime(SpecialOpsSettings::default(), 7, 8, &runtime).unwrap();

        assert_eq!(bootstrap.run_snapshot.unwrap().run_id, started.run_id);
    }

    #[test]
    fn legacy_bootstrap_defaults_missing_run_snapshot() {
        let mut value =
            serde_json::to_value(build_bootstrap(SpecialOpsSettings::default(), 7, 8)).unwrap();
        value.as_object_mut().unwrap().remove("runSnapshot");

        let bootstrap: SpecialOpsBootstrap = serde_json::from_value(value).unwrap();

        assert_eq!(bootstrap.run_snapshot, None);
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
        assert!(serialized["accounts"][0].get("password").is_none());
        assert!(!target_keys.contains(&"wegame.launchPage"));
        assert!(target_keys.contains(&"wegame.gameEntry"));
    }

    #[test]
    fn login_targets_use_remembered_account_selection() {
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
                "wegame.accountDropdown",
                "wegame.accountList",
                "wegame.selectedAccount",
                "wegame.login",
                "wegame.gameEntry",
                "wegame.launch",
            ]
        );
    }

    #[test]
    fn enabled_qq_accounts_must_be_unique() {
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
        invalid_account.qq_account = "abc".to_string();
        let settings = SpecialOpsSettings {
            accounts: vec![invalid_account],
            ..SpecialOpsSettings::default()
        };
        assert_eq!(
            validate_execution_ready(&settings).unwrap_err(),
            "账号 active 的 QQ 必须为非空纯数字"
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
    fn required_execution_wegame_targets_use_remembered_account_selection() {
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
            "wegame.accountDropdown",
            "wegame.accountList",
            "wegame.selectedAccount",
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
    fn state_changed_payload_excludes_settings_and_accounts() {
        let mut settings = SpecialOpsSettings::default();
        settings
            .accounts
            .push(account("selected", AccountStatus::Ready, Vec::new()));
        let bootstrap = build_bootstrap(settings, 17, 23);

        let payload = SpecialOpsStateChanged::from(&bootstrap);
        let json = serde_json::to_string(&payload).unwrap();
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
    fn login_trial_preflight_requires_selected_numeric_qq() {
        let fixture = LoginFixture::complete();
        assert!(validate_login_trial_ready(&fixture.settings, "selected").is_ok());

        let mut empty_qq = fixture.settings.clone();
        empty_qq.accounts[0].qq_account = "   ".to_string();
        let error = validate_login_trial_ready(&empty_qq, "selected").unwrap_err();
        assert!(error.contains("selected"));
        assert!(error.contains("QQ"));

        let mut non_numeric = fixture.settings.clone();
        non_numeric.accounts[0].qq_account = "abc123".to_string();
        let error = validate_login_trial_ready(&non_numeric, "selected").unwrap_err();
        assert!(error.contains("selected"));
        assert!(error.contains("纯数字"));
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
    fn login_trial_preflight_requires_five_templates_and_three_account_targets() {
        let fixture = LoginFixture::complete();
        let ordered_keys = [
            "wegame.loginMode",
            "wegame.loginFormReady",
            "wegame.accountDropdown",
            "wegame.accountList",
            "wegame.selectedAccount",
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
