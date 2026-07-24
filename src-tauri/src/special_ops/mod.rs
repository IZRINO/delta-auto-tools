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
    pub wegame_id: String,
    pub enabled: bool,
    pub initialized: bool,
    pub order: u32,
    pub status: AccountStatus,
    pub stations: Vec<StationPlan>,
    pub ammo_targets: Vec<AmmoTarget>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationTemplateTestResult {
    pub sample_similarities: [f32; 2],
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
        "wegame.profileId" => &["wegame.profileReady"],
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
            "wegame.humanVerification",
            "WeGame 人工验证提示区域",
            RecognitionRegion,
        ),
        (
            "wegame.loginFailed",
            "WeGame 登录失败提示区域",
            RecognitionRegion,
        ),
        (
            "wegame.loggedIn",
            "WeGame 最大化后登录成功状态区域",
            RecognitionRegion,
        ),
        (
            "wegame.launchPage",
            "游戏启动前置界面入口识别与点击区域",
            RecognitionRegion,
        ),
        (
            "wegame.launchPageReady",
            "游戏启动前置界面就绪区域",
            RecognitionRegion,
        ),
        (
            "wegame.launchId",
            "游戏启动前置界面 ID OCR 区域",
            RecognitionRegion,
        ),
        (
            "wegame.avatar",
            "WeGame 头像识别与点击区域",
            RecognitionRegion,
        ),
        (
            "wegame.avatarMenuReady",
            "WeGame 头像菜单展开状态区域",
            RecognitionRegion,
        ),
        (
            "wegame.profile",
            "个人主页入口识别与点击区域",
            RecognitionRegion,
        ),
        ("wegame.profileReady", "个人主页就绪区域", RecognitionRegion),
        ("wegame.profileId", "个人主页 ID 双击复制区域", ClickPoint),
        (
            "wegame.switchAccount",
            "切换账号入口识别与点击区域",
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
            (RecognitionRegion, "wegame.launchId" | "ammo.selectedTargetName") => {
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
                if target.recognition_method != Some(CalibrationRecognitionMethod::Template) {
                    target.reference_image_path = None;
                }
                target
            })
            .collect();
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
    for (index, account) in settings.accounts.iter_mut().enumerate() {
        account.id = account.id.trim().to_string();
        account.qq_account = account.qq_account.trim().to_string();
        account.wegame_id = account.wegame_id.trim().to_string();
        if account.id.is_empty() || !ids.insert(account.id.clone()) {
            return Err("账号 ID 必须非空且唯一".to_string());
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
        "wegame.humanVerification",
        "wegame.loginFailed",
        "wegame.loggedIn",
        "wegame.launchPage",
        "wegame.launchPageReady",
        "wegame.launchId",
        "wegame.avatar",
        "wegame.avatarMenuReady",
        "wegame.profile",
        "wegame.profileReady",
        "wegame.profileId",
        "wegame.switchAccount",
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
        if account.qq_account.trim().is_empty()
            || account.password.is_empty()
            || account.wegame_id.trim().is_empty()
        {
            return Err(format!(
                "账号 {} 的 QQ、密码与 WeGame ID 必须完整",
                account.id
            ));
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

fn calibration_template_test_input(
    settings: &SpecialOpsSettings,
    environment_id: &str,
    target_key: &str,
) -> Result<(crate::morse::types::RegionRect, String), String> {
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
    Ok((
        crate::morse::types::RegionRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        },
        reference_image_path.to_string(),
    ))
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
    let _ = app.emit_to("main", STATE_CHANGED, bootstrap);
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
    state: State<'_, SpecialOpsState>,
    environment_id: String,
    target_key: String,
) -> Result<CalibrationTemplateTestResult, AppError> {
    let (region, reference_image_path) = {
        let settings = state
            .settings
            .lock()
            .map_err(|_| AppError::from("特勤处状态已损坏"))?;
        calibration_template_test_input(&settings, &environment_id, &target_key)?
    };
    let first = sample_template_similarity(region.clone(), reference_image_path.clone()).await?;
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    let second = sample_template_similarity(region, reference_image_path).await?;
    Ok(CalibrationTemplateTestResult {
        sample_similarities: [first, second],
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
            wegame_id: id.to_string(),
            enabled: true,
            initialized: true,
            order: 0,
            status,
            stations,
            ammo_targets: Vec::new(),
        }
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
            "账号 active 的 QQ、密码与 WeGame ID 必须完整"
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
    fn default_click_and_input_targets_have_recognition_guards() {
        let targets = default_calibration_targets();
        let actions = targets.iter().filter(|target| {
            matches!(
                target.kind,
                CalibrationTargetKind::ClickPoint | CalibrationTargetKind::InputRegion
            )
        });
        let mut action_count = 0;
        for action in actions {
            action_count += 1;
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
        assert_eq!(action_count, 16);
    }

    #[test]
    fn default_dynamic_text_targets_use_ocr() {
        let targets = default_calibration_targets();

        for key in ["wegame.launchId", "ammo.selectedTargetName"] {
            let target = targets.iter().find(|target| target.key == key).unwrap();
            assert_eq!(
                target.recognition_method,
                Some(CalibrationRecognitionMethod::Ocr)
            );
            assert_eq!(target.reference_image_path, None);
        }
        assert_eq!(
            targets
                .iter()
                .find(|target| target.key == "wegame.loginFormReady")
                .unwrap()
                .recognition_method,
            Some(CalibrationRecognitionMethod::Template)
        );
        assert!(targets.iter().any(|target| target.key == "wegame.profile"));
        assert!(targets
            .iter()
            .any(|target| target.key == "wegame.profileReady"));
    }

    #[test]
    fn wegame_id_validation_targets_follow_ocr_then_copy_fallback_order() {
        let targets = default_calibration_targets();
        let position = |key: &str| {
            targets
                .iter()
                .position(|target| target.key == key)
                .unwrap_or_else(|| panic!("缺少 WeGame ID 校准目标 {key}"))
        };
        let ordered_keys = [
            "wegame.launchPage",
            "wegame.launchPageReady",
            "wegame.launchId",
            "wegame.avatar",
            "wegame.avatarMenuReady",
            "wegame.profile",
            "wegame.profileReady",
            "wegame.profileId",
            "wegame.launch",
        ];

        assert!(ordered_keys
            .windows(2)
            .all(|pair| position(pair[0]) < position(pair[1])));
        assert_eq!(
            targets[position("wegame.launchId")].recognition_method,
            Some(CalibrationRecognitionMethod::Ocr)
        );
        assert_eq!(
            targets[position("wegame.profileId")].guard_any_of,
            vec!["wegame.profileReady"]
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
        target.reference_image_path = Some("reference.png".to_string());
        let (region, path) =
            calibration_template_test_input(&settings, "default", "wegame.loginMode").unwrap();
        assert_eq!(
            (region.x, region.y, region.width, region.height),
            (10, 20, 30, 40)
        );
        assert_eq!(path, "reference.png");
        assert_eq!(
            calibration_template_test_input(&settings, "default", "wegame.launchId").unwrap_err(),
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
            },
            CalibrationTarget {
                key: "craft.idle".to_string(),
                label: "旧通用空闲文字区域".to_string(),
                kind: CalibrationTargetKind::RecognitionRegion,
                rect: None,
                reference_image_path: None,
                recognition_method: None,
                guard_any_of: Vec::new(),
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
}
