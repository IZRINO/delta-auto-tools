mod ammo_runtime;
mod craft_batch;
mod craft_runtime;
mod craft_trial;
pub(crate) mod desktop_runtime;
mod game_navigation;
mod limited_supply;
mod limited_supply_runtime;
#[allow(dead_code)]
pub(crate) mod login_flow;
mod login_runtime;
mod market_purchase;
mod market_runtime;
mod military_supply_runtime;
mod profit;
#[allow(dead_code)]
mod remembered_account;
mod round_account;
mod round_planner;
mod round_runner;
mod round_scheduler;
#[allow(dead_code)]
pub(crate) mod template_observer;
#[allow(dead_code)]
mod windows_clipboard;
#[allow(dead_code)]
mod windows_ocr;

use chrono::{FixedOffset, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::overlay_utils::{destroy_window, encoded_query_value, safe_label_component};
use crate::{app_error::AppError, settings::SettingsCoordinator};

use limited_supply::{LimitedSupplyAccountState, LimitedSupplySettings};
pub use login_runtime::{LoginRunKind, LoginRunSnapshot, LoginRunStatus};
use market_purchase::{MarketAccountState, MarketBusinessConfig, MarketPurchaseSettings};
use profit::model::{
    apply_profit_configuration, normalize_profit_settings, validate_profit_configuration,
    AmmoProfitAudit, AmmoProfitRule, ProfitConfigurationUpdate, ProfitCutoffSkipReason,
    ProfitCutoffState, ProfitCutoffTarget, ProfitFilterSettings,
};
use profit::query::query_profit_rules_with_cancel;
use profit::runtime::{
    ProfitQueryControl, ProfitQueryWindow, ProfitRuntimeSnapshot, ProfitTargetKey,
};
use profit::{
    cutoff::{classify_cutoff_audits, FINAL_MINIMUM_PROFIT},
    kkrb::{KkrbAdapter, ProfitCatalogSnapshot},
    moligod::{MoligodAdapter, MoligodRequestTarget, MoligodRuleStatus},
};
use round_planner::AmmoProfitGate;

const SETTINGS_FILE_NAME: &str = "special_ops_settings.json";
pub(crate) const MOUSE_CLICK_HOLD_MS: u64 = 100;
const PROFIT_QUERY_FIVE_MINUTES_MS: i64 = 5 * 60_000;
const PROFIT_QUERY_FIFTY_MINUTES_MS: i64 = 50 * 60_000;
const PROFIT_QUERY_STALE: &str = "利润查询结果已过期";
const OPERATION_WINDOW_LOAD_TIMEOUT: &str = "操作提示窗口加载超时，已取消本次试运行";
const OPERATION_WINDOW_RETRY_DELAY: Duration = Duration::from_secs(1);
/// poll 与 execute 判定不一致时的退避间隔，避免空转刷日志。
const SCHEDULER_TRANSIENT_RETRY_DELAY: Duration = Duration::from_secs(30);
pub const STATE_CHANGED: &str = "special-ops://state-changed";
const LOGIN_HOTKEY_SCOPE: &str = "special-ops-emergency";
static LOGIN_RESOURCE_CLEANUP_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
/// 单实例特勤处 run 收起的辅助窗口标签，统一由资源清理路径恢复。
static SPECIAL_OPS_HIDDEN_WINDOWS: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

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
    ManualCheckRequired,
    Uncertain,
    Isolated,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ManualStationState {
    ImmediateDue,
    Crafting,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StationCorrectionInput {
    pub kind: StationKind,
    pub state: ManualStationState,
    pub remaining_minutes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AmmoCorrectionInput {
    pub target_id: String,
    pub succeeded_today: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountFailure {
    pub step: String,
    pub message: String,
    pub at_ms: i64,
    #[serde(default)]
    pub station_kind: Option<StationKind>,
    #[serde(default)]
    pub ammo_target_id: Option<String>,
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
    #[serde(default)]
    pub retry_day: Option<String>,
    pub retry_count: u8,
    #[serde(default)]
    pub last_failure: Option<AccountFailure>,
}

fn prepare_ammo_retry_state(target: &mut AmmoTarget, day: &str) {
    if target.retry_day.as_deref() != Some(day) {
        target.retry_day = Some(day.to_string());
        target.retry_count = 0;
    }
}

fn apply_ammo_success(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    target_id: &str,
    day: &str,
) -> Result<(), String> {
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "子弹兑换账号已不存在".to_string())?;
    let target = account
        .ammo_targets
        .iter_mut()
        .find(|target| target.id == target_id)
        .ok_or_else(|| "子弹兑换目标已不存在".to_string())?;
    target.last_success_day = Some(day.to_string());
    target.retry_day = Some(day.to_string());
    target.retry_count = 0;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_ammo_failure(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    target_id: &str,
    day: &str,
    step: &str,
    message: &str,
    failed_at_ms: i64,
) -> Result<(), String> {
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "子弹兑换账号已不存在".to_string())?;
    let target = account
        .ammo_targets
        .iter_mut()
        .find(|target| target.id == target_id)
        .ok_or_else(|| "子弹兑换目标已不存在".to_string())?;
    prepare_ammo_retry_state(target, day);
    target.retry_count = target.retry_count.saturating_add(1);
    account.last_failure = Some(AccountFailure {
        step: step.to_string(),
        message: format!("目标 {target_id}：{message}"),
        at_ms: failed_at_ms,
        station_kind: None,
        ammo_target_id: Some(target_id.to_string()),
    });
    Ok(())
}

fn apply_ammo_isolated(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    target_id: &str,
    step: &str,
    message: &str,
    failed_at_ms: i64,
) -> Result<(), String> {
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "子弹兑换账号已不存在".to_string())?;
    set_ammo_manual_failure(account, target_id, step, message, failed_at_ms)?;
    account.status = AccountStatus::Isolated;
    Ok(())
}

fn set_ammo_manual_failure(
    account: &mut AccountPlan,
    target_id: &str,
    step: &str,
    message: &str,
    at_ms: i64,
) -> Result<(), String> {
    let failure = AccountFailure {
        step: step.to_string(),
        message: message.to_string(),
        at_ms,
        station_kind: None,
        ammo_target_id: Some(target_id.to_string()),
    };
    let target = account
        .ammo_targets
        .iter_mut()
        .find(|target| target.id == target_id)
        .ok_or_else(|| "子弹兑换目标已不存在".to_string())?;
    target.last_failure = Some(failure.clone());
    let has_station_failure = account
        .last_failure
        .as_ref()
        .is_some_and(|current| current.station_kind.is_some());
    let has_login_block = account_allows_manual_check(&account.status);
    if !has_station_failure && !has_login_block {
        account.last_failure = Some(failure);
        account.status = AccountStatus::Ready;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StationBusinessConfig {
    pub kind: StationKind,
    pub enabled: bool,
    pub duration_minutes: u32,
    #[serde(default)]
    pub recipe_note: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ScrollDirection {
    Up,
    #[default]
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AmmoBusinessTarget {
    pub id: String,
    #[serde(default, alias = "name")]
    pub note: String,
    pub enabled: bool,
    pub seasonal: bool,
    #[serde(default)]
    pub click_point: Option<CalibrationRect>,
    #[serde(default)]
    pub scroll_direction: ScrollDirection,
    #[serde(default)]
    pub scroll_steps: u32,
    pub order: u32,
    #[serde(default)]
    pub profit_rule_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BusinessConfig {
    pub stations: Vec<StationBusinessConfig>,
    #[serde(default)]
    pub recipe_points: Vec<AccountRecipePoint>,
    pub ammo_targets: Vec<AmmoBusinessTarget>,
    #[serde(default)]
    pub market: MarketBusinessConfig,
}

impl Default for BusinessConfig {
    fn default() -> Self {
        Self {
            stations: StationKind::all()
                .into_iter()
                .map(|kind| StationBusinessConfig {
                    kind,
                    enabled: true,
                    duration_minutes: 60,
                    recipe_note: String::new(),
                })
                .collect(),
            recipe_points: Vec::new(),
            ammo_targets: Vec::new(),
            market: MarketBusinessConfig::default(),
        }
    }
}

fn missing_business_config() -> BusinessConfig {
    BusinessConfig {
        stations: Vec::new(),
        recipe_points: Vec::new(),
        ammo_targets: Vec::new(),
        market: MarketBusinessConfig::default(),
    }
}

fn business_config_from_account(account: &AccountPlan) -> BusinessConfig {
    BusinessConfig {
        stations: StationKind::all()
            .into_iter()
            .map(|kind| {
                account
                    .stations
                    .iter()
                    .find(|station| station.kind == kind)
                    .map(|station| StationBusinessConfig {
                        kind: kind.clone(),
                        enabled: station.enabled,
                        duration_minutes: station.duration_minutes,
                        recipe_note: station.item_name.clone(),
                    })
                    .unwrap_or(StationBusinessConfig {
                        kind,
                        enabled: false,
                        duration_minutes: 240,
                        recipe_note: String::new(),
                    })
            })
            .collect(),
        recipe_points: Vec::new(),
        ammo_targets: account
            .ammo_targets
            .iter()
            .map(|target| AmmoBusinessTarget {
                id: target.id.clone(),
                note: target.name.clone(),
                enabled: target.enabled,
                seasonal: target.seasonal,
                click_point: None,
                scroll_direction: ScrollDirection::Down,
                scroll_steps: target.scroll_steps,
                order: target.order,
                profit_rule_id: None,
            })
            .collect(),
        market: MarketBusinessConfig::default(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountRecipePoint {
    pub kind: StationKind,
    pub rect: CalibrationRect,
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
    #[serde(default)]
    pub independent_settings_enabled: bool,
    #[serde(default)]
    pub independent_business_config: Option<BusinessConfig>,
    pub stations: Vec<StationPlan>,
    pub ammo_targets: Vec<AmmoTarget>,
    #[serde(default)]
    pub last_failure: Option<AccountFailure>,
    #[serde(default)]
    pub login_trial_signature: Option<String>,
    #[serde(default)]
    pub limited_supply: LimitedSupplyAccountState,
    #[serde(default)]
    pub market: MarketAccountState,
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
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CalibrationTestResult {
    Template {
        sample_similarities: [f32; 2],
        passed: bool,
        verified_at_ms: Option<i64>,
    },
    Ocr {
        first_texts: Vec<String>,
        second_texts: Vec<String>,
        passed: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LimitedSupplyColorTestResult {
    pub first_sampled_color: [u8; 3],
    pub first_matched_color: Option<[u8; 3]>,
    pub first_target_color: [u8; 3],
    pub first_tolerance: u8,
    pub first_matched: bool,
    pub second_matched_color: Option<[u8; 3]>,
    pub second_sampled_color: [u8; 3],
    pub second_target_color: [u8; 3],
    pub second_tolerance: u8,
    pub second_matched: bool,
    pub first_nearest_distance: f32,
    pub second_nearest_distance: f32,
    pub passed: bool,
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
    /// 最近一次自动暂停的原因，仅在 `paused` 为真时有意义。
    /// 用户手动点暂停不写；用户点继续时清空 -> UI 只在「不是我点的」时给出解释。
    #[serde(default)]
    pub paused_reason: Option<String>,
    pub daily_exchange_time: String,
    pub emergency_hotkey: String,
    #[serde(default = "default_navigation_delay_ms")]
    pub navigation_beacon_delay_ms: u32,
    #[serde(default = "default_navigation_delay_ms")]
    pub navigation_space_delay_ms: u32,
    #[serde(default = "default_navigation_delay_ms")]
    pub navigation_tab_delay_ms: u32,
    #[serde(default = "default_navigation_delay_ms")]
    pub navigation_special_ops_delay_ms: u32,
    #[serde(default = "default_navigation_delay_ms")]
    pub ammo_supply_delay_ms: u32,
    #[serde(default = "default_navigation_delay_ms")]
    pub ammo_tactical_delay_ms: u32,
    #[serde(default = "default_navigation_delay_ms")]
    pub craft_space_delay_ms: u32,
    #[serde(default = "default_navigation_delay_ms")]
    pub craft_reopen_delay_ms: u32,
    #[serde(default = "default_navigation_delay_ms")]
    pub craft_confirm_pinned_delay_ms: u32,
    #[serde(default)]
    pub wegame_executable_path: String,
    #[serde(default)]
    pub game_executable_path: String,
    #[serde(default = "missing_business_config")]
    pub default_business_config: BusinessConfig,
    #[serde(default)]
    pub profit_filter: ProfitFilterSettings,
    #[serde(default)]
    pub limited_supply: LimitedSupplySettings,
    #[serde(default)]
    pub market_purchase: MarketPurchaseSettings,
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
            paused_reason: None,
            daily_exchange_time: "08:00".to_string(),
            emergency_hotkey: "Ctrl+Shift+F12".to_string(),
            navigation_beacon_delay_ms: default_navigation_delay_ms(),
            navigation_space_delay_ms: default_navigation_delay_ms(),
            navigation_tab_delay_ms: default_navigation_delay_ms(),
            navigation_special_ops_delay_ms: default_navigation_delay_ms(),
            ammo_supply_delay_ms: default_navigation_delay_ms(),
            ammo_tactical_delay_ms: default_navigation_delay_ms(),
            craft_space_delay_ms: default_navigation_delay_ms(),
            craft_reopen_delay_ms: default_navigation_delay_ms(),
            craft_confirm_pinned_delay_ms: default_navigation_delay_ms(),
            wegame_executable_path: String::new(),
            game_executable_path: String::new(),
            default_business_config: BusinessConfig::default(),
            profit_filter: ProfitFilterSettings::default(),
            limited_supply: LimitedSupplySettings::default(),
            market_purchase: MarketPurchaseSettings::default(),
            accounts: Vec::new(),
            active_calibration_id: Some("default".to_string()),
            calibration_environments: vec![default_calibration_environment()],
        }
    }
}

fn default_navigation_delay_ms() -> u32 {
    3_000
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

/// 判断窗口是否属于尚未完成的桌面选择交互，不能静默隐藏以免遗留等待中的 sender。
fn is_active_selection_overlay(label: &str) -> bool {
    label == "morse-overlay"
        || label.starts_with("timer-position-")
        || label.starts_with("counter-position-")
        || label.starts_with("rapidfire-position-")
        || label.starts_with("special-ops-calibration-")
        || label.starts_with("recognition-selection-")
}

/// 特勤处操作提示窗口保留，其他已存在窗口均进入运行期隐藏列表。
///
/// ponytail: 禁止在 scheduler 启动路径读取同步 `is_visible`；它会等待 Tauri UI 线程，
/// 从“继续”命令回调启动轮次时可能阻塞前端响应。
fn should_hide_for_special_ops(label: &str) -> bool {
    label != "main" && label != login_runtime::OPERATION_WINDOW_LABEL
}

#[derive(Debug, Default, PartialEq, Eq)]
struct HiddenWindowRestorePlan {
    direct_labels: Vec<String>,
    reconcile_tool_windows: bool,
}

fn hidden_window_restore_plan(labels: impl IntoIterator<Item = String>) -> HiddenWindowRestorePlan {
    let mut plan = HiddenWindowRestorePlan::default();
    for label in labels {
        if label.starts_with("timer-display")
            || label.starts_with("counter-display")
            || label.starts_with("rapidfire-display")
            || label == "recognition-overlay"
        {
            plan.reconcile_tool_windows = true;
        } else {
            plan.direct_labels.push(label);
        }
    }
    plan
}

/// 隐藏其他功能窗口，截图与键鼠操作期间避免应用自身窗口遮挡游戏 UI。
fn hide_other_windows_for_special_ops(app: &AppHandle) -> Result<(), String> {
    let windows = app.webview_windows();
    if let Some(label) = windows
        .keys()
        .find(|label| is_active_selection_overlay(label))
    {
        return Err(format!("存在未结束的区域或位置选择窗口：{label}"));
    }

    let mut hidden = Vec::<String>::new();
    for (label, window) in windows {
        if !should_hide_for_special_ops(&label) {
            continue;
        }
        if let Err(error) = window.hide() {
            for restored_label in &hidden {
                if let Some(restored) = app.get_webview_window(restored_label) {
                    let _ = restored.show();
                }
            }
            return Err(format!("隐藏窗口 {label} 失败: {error}"));
        }
        hidden.push(label.to_string());
    }
    *SPECIAL_OPS_HIDDEN_WINDOWS
        .lock()
        .map_err(|_| "特勤处窗口快照已损坏".to_string())? = hidden;
    Ok(())
}

/// 恢复特勤处启动时收起的辅助窗口；运行期被销毁的窗口不重建。
fn restore_other_windows_after_special_ops(app: &AppHandle) -> Result<(), String> {
    let plan = hidden_window_restore_plan(std::mem::take(
        &mut *SPECIAL_OPS_HIDDEN_WINDOWS
            .lock()
            .map_err(|_| "特勤处窗口快照已损坏".to_string())?,
    ));
    let mut errors = Vec::new();
    for label in plan.direct_labels {
        if let Some(window) = app.get_webview_window(&label) {
            if let Err(error) = window.show() {
                errors.push(format!("恢复窗口 {label} 失败: {error}"));
            }
        }
    }
    let global_enabled = app
        .try_state::<crate::global_state::GlobalState>()
        .is_some_and(|state| state.enabled());
    if plan.reconcile_tool_windows && global_enabled {
        if let Err(error) = crate::global_state::restore_active_windows(app) {
            errors.push(format!("按工具开关恢复窗口失败: {error}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn default_guard_any_of(key: &str) -> &'static [&'static str] {
    if key.starts_with("limited.color.") {
        return &["limited.ready"];
    }
    match key {
        "wegame.accountDropdown" | "wegame.selectedAccount" => &["wegame.loginFormReady"],
        "game.beaconMode" | "market.entry" | "market.product" | "market.return" | "market.buy"
        | "market.confirm" => &["game.modeReady"],
        _ => &[],
    }
}

fn default_calibration_targets() -> Vec<CalibrationTarget> {
    use CalibrationTargetKind::{ClickPoint, InputRegion, RecognitionRegion};
    [
        ("runtime.mouseParking", "特勤处鼠标停放点", ClickPoint),
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
        ("wegame.selectedAccount", "已选账号双击复制区域", ClickPoint),
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
        ("game.beaconMode", "烽火地带入口点击点", ClickPoint),
        ("game.specialOps", "特勤处入口点击点", ClickPoint),
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
        ("craft.confirmPinned", "确认置顶点击点", ClickPoint),
        ("craft.returnToStationGrid", "制作中返回点击点", ClickPoint),
        (
            "craft.recipe.technicalCenter",
            "技术中心制作物品选择点击点",
            ClickPoint,
        ),
        (
            "craft.recipe.workbench",
            "工作台制作物品选择点击点",
            ClickPoint,
        ),
        (
            "craft.recipe.pharmacy",
            "制药台制作物品选择点击点",
            ClickPoint,
        ),
        (
            "craft.recipe.armorBench",
            "防具台制作物品选择点击点",
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
        ("ammo.supply", "军需处点击点", ClickPoint),
        ("ammo.enterSupply", "进入军需处点击点", ClickPoint),
        (
            "ammo.tacticalDepartment",
            "战术部门识别与点击区域",
            RecognitionRegion,
        ),
        ("ammo.seasonal", "赛季限定入口点击点", ClickPoint),
        ("ammo.fill", "子弹一键补齐区域", RecognitionRegion),
        (
            "ammo.purchase",
            "子弹材料购买按钮识别与点击区域",
            RecognitionRegion,
        ),
        ("ammo.exchange", "兑换按钮区域", RecognitionRegion),
        (
            "ammo.confirm",
            "兑换二次确认按钮识别与点击区域",
            RecognitionRegion,
        ),
        ("ammo.success", "兑换完成状态识别区域", RecognitionRegion),
        (
            "ammo.researchDepartment",
            "研发部门识别与点击区域",
            RecognitionRegion,
        ),
        (
            "limited.ready",
            "研发部门页面就绪识别区域",
            RecognitionRegion,
        ),
        ("limited.color.1", "限时商品识色区域 1", InputRegion),
        ("limited.color.2", "限时商品识色区域 2", InputRegion),
        ("limited.color.3", "限时商品识色区域 3", InputRegion),
        ("limited.color.4", "限时商品识色区域 4", InputRegion),
        ("limited.color.5", "限时商品识色区域 5", InputRegion),
        ("limited.color.6", "限时商品识色区域 6", InputRegion),
        ("limited.color.7", "限时商品识色区域 7", InputRegion),
        ("limited.color.8", "限时商品识色区域 8", InputRegion),
        ("limited.color.9", "限时商品识色区域 9", InputRegion),
        (
            "market.entry",
            "交易行入口识别与点击区域",
            RecognitionRegion,
        ),
        ("market.product", "默认商品入口点击点", ClickPoint),
        ("market.price", "交易行价格 OCR 区域", RecognitionRegion),
        ("market.return", "交易行高价返回点击点", ClickPoint),
        ("market.buy", "交易行达标购买点击点", ClickPoint),
        ("market.confirm", "交易行最终确认购买点击点", ClickPoint),
    ]
    .into_iter()
    .map(|(key, label, kind)| {
        let recognition_method = match (&kind, key) {
            (RecognitionRegion, "wegame.accountList" | "market.price") => {
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

fn mouse_parking_region(
    settings: &SpecialOpsSettings,
) -> Result<crate::morse::types::RegionRect, String> {
    let target = settings
        .calibration_environments
        .first()
        .and_then(|environment| {
            environment
                .targets
                .iter()
                .find(|target| target.key == "runtime.mouseParking")
        })
        .ok_or_else(|| "特勤处鼠标停放点不存在".to_string())?;
    let rect = target
        .rect
        .as_ref()
        .ok_or_else(|| "特勤处鼠标停放点尚未校准".to_string())?;
    Ok(crate::morse::types::RegionRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DueAccount {
    pub account_id: String,
    pub station_kinds: Vec<StationKind>,
    pub ammo_target_ids: Vec<String>,
    #[serde(default)]
    pub limited_supply_due: bool,
    #[serde(default)]
    pub market_purchase_due: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleSnapshot {
    pub due_accounts: Vec<DueAccount>,
    pub next_wake_at_ms: Option<i64>,
    pub timeline_start_ms: i64,
    pub timeline_end_ms: i64,
    pub timeline_tasks: Vec<TimelineTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TimelineTaskKind {
    Craft,
    Ammo,
    LimitedSupplyCheck,
    MarketPurchase,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TimelineProfitState {
    WaitingExchange,
    WaitingQuery,
    Unconfigured,
    Qualified,
    ActiveRound,
    CutoffQuerying,
    WaitingCutoffRetry,
    CutoffSkipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineTask {
    pub id: String,
    pub account_id: String,
    pub qq_account: String,
    pub kind: TimelineTaskKind,
    pub station_kind: Option<StationKind>,
    pub ammo_target_id: Option<String>,
    pub note: String,
    pub scheduled_at_ms: i64,
    pub overdue: bool,
    pub account_status: AccountStatus,
    #[serde(default)]
    pub profit_state: Option<TimelineProfitState>,
    #[serde(default)]
    pub may_execute_earlier: bool,
    #[serde(default)]
    pub manual_failure: Option<AccountFailure>,
    #[serde(default)]
    pub limited_cycle_id: Option<String>,
    #[serde(default)]
    pub limited_outcome: Option<limited_supply::LimitedSupplyOutcome>,
    #[serde(default)]
    pub market_completed_count: Option<u32>,
    #[serde(default)]
    pub market_target_count: Option<u32>,
    #[serde(default)]
    pub market_status: Option<market_purchase::MarketTaskStatus>,
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
    #[serde(default)]
    pub profit_runtime: profit::runtime::ProfitRuntimeSnapshot,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpecialOpsStateChanged {
    pub settings_revision: u64,
    pub now_ms: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MoligodBindingValidation {
    pub exact_name: String,
    pub profit: i64,
}

pub struct SpecialOpsState {
    settings: Arc<Mutex<SpecialOpsSettings>>,
    login_runtime: Arc<login_runtime::LoginRuntime>,
    profit_runtime: Arc<ProfitQueryControl>,
    round_control: Arc<RoundControl>,
    round_scheduler: Arc<round_scheduler::RoundScheduler>,
}

impl SpecialOpsState {
    pub(crate) fn settings_snapshot(&self) -> Result<SpecialOpsSettings, String> {
        self.settings
            .lock()
            .map(|settings| settings.clone())
            .map_err(|_| "特勤处状态已损坏".to_string())
    }

    pub(crate) fn ensure_profile_apply_allowed(&self) -> Result<(), String> {
        if self.login_runtime.snapshot()?.is_some() {
            return Err("特勤处正在运行，完成或停止当前操作后才能切换 Profile".to_string());
        }
        Ok(())
    }

    pub(crate) fn apply_profile_settings(
        &self,
        app: &AppHandle,
        incoming: SpecialOpsSettings,
    ) -> Result<(), String> {
        let mut next = normalize_settings(incoming)?;
        next.paused = true;

        let was_armed = self.round_scheduler.is_armed();
        self.round_scheduler.disarm();
        if let Err(error) = save_settings(app, &next) {
            if was_armed {
                self.round_scheduler.arm();
            }
            return Err(error);
        }

        self.profit_runtime
            .invalidate("Profile 已切换，利润查询已取消");
        *self
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next;

        let coordinator = app
            .try_state::<Arc<SettingsCoordinator>>()
            .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
        emit_state(app, coordinator.current_revision()?, now_ms());
        Ok(())
    }
}

#[derive(Default)]
struct RoundControl {
    pause_requested: std::sync::atomic::AtomicBool,
    preserve_game: std::sync::atomic::AtomicBool,
}

impl RoundControl {
    fn request_pause(&self) {
        self.preserve_game
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.pause_requested
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn request_system_pause(&self) {
        self.preserve_game
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.pause_requested
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn pause_requested(&self) -> bool {
        self.pause_requested
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    fn clear_pause_request(&self) {
        self.pause_requested
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.preserve_game
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    fn preserve_game(&self) -> bool {
        self.preserve_game.load(std::sync::atomic::Ordering::SeqCst)
    }
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

    fn from_calibration_suffix(suffix: &str) -> Option<Self> {
        Self::all()
            .into_iter()
            .find(|kind| kind.calibration_suffix() == suffix)
    }
}

fn resolve_account_business_config<'a>(
    settings: &'a SpecialOpsSettings,
    account: &'a AccountPlan,
) -> Result<&'a BusinessConfig, String> {
    if account.independent_settings_enabled {
        account.independent_business_config.as_ref().ok_or_else(|| {
            format!(
                "账号 {} 已开启独立设置，但独立业务配置缺失",
                account.qq_account
            )
        })
    } else {
        Ok(&settings.default_business_config)
    }
}

fn collect_pending_profit_rules(settings: &SpecialOpsSettings, day: &str) -> Vec<AmmoProfitRule> {
    if !settings.profit_filter.enabled {
        return Vec::new();
    }
    let rules = settings
        .profit_filter
        .rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule))
        .collect::<std::collections::HashMap<_, _>>();
    let mut pending_ids = std::collections::BTreeSet::new();
    for account in settings.accounts.iter().filter(|account| {
        account.enabled && account.initialized && account.status == AccountStatus::Ready
    }) {
        let Ok(business) = resolve_account_business_config(settings, account) else {
            continue;
        };
        for target in business.ammo_targets.iter().filter(|target| {
            target.enabled
                && target
                    .click_point
                    .as_ref()
                    .is_some_and(|point| point.width == 1 && point.height == 1)
        }) {
            let Some(rule_id) = target.profit_rule_id.as_deref() else {
                continue;
            };
            let pending = account
                .ammo_targets
                .iter()
                .find(|runtime| runtime.id == target.id)
                .is_none_or(|runtime| {
                    runtime.last_success_day.as_deref() != Some(day)
                        && (runtime.retry_day.as_deref() != Some(day) || runtime.retry_count < 2)
                });
            if pending && rules.contains_key(rule_id) {
                pending_ids.insert(rule_id);
            }
        }
    }
    pending_ids
        .into_iter()
        .filter_map(|rule_id| rules.get(rule_id).cloned())
        .cloned()
        .collect()
}

fn cutoff_target_is_pending(account: &AccountPlan, target_id: &str, day: &str) -> bool {
    account
        .ammo_targets
        .iter()
        .find(|runtime| runtime.id == target_id)
        .is_none_or(|runtime| {
            runtime.last_success_day.as_deref() != Some(day)
                && (runtime.retry_day.as_deref() != Some(day) || runtime.retry_count < 2)
        })
}

fn build_profit_cutoff_state(
    settings: &SpecialOpsSettings,
    day: &str,
    decided_at_ms: i64,
) -> ProfitCutoffState {
    let rules = settings
        .profit_filter
        .rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule))
        .collect::<std::collections::HashMap<_, _>>();
    let mut targets = Vec::new();
    for account in settings.accounts.iter().filter(|account| {
        account.enabled && account.initialized && account.status == AccountStatus::Ready
    }) {
        let Ok(business) = resolve_account_business_config(settings, account) else {
            continue;
        };
        for target in business.ammo_targets.iter().filter(|target| {
            target.enabled
                && target
                    .click_point
                    .as_ref()
                    .is_some_and(|point| point.width == 1 && point.height == 1)
                && cutoff_target_is_pending(account, &target.id, day)
        }) {
            let rule_id = target.profit_rule_id.clone();
            let unconfigured = rule_id
                .as_deref()
                .is_none_or(|rule_id| !rules.contains_key(rule_id));
            targets.push(ProfitCutoffTarget {
                account_id: account.id.clone(),
                target_id: target.id.clone(),
                rule_id,
                skip_reason: unconfigured.then_some(ProfitCutoffSkipReason::Unconfigured),
                decided_at_ms: unconfigured.then_some(decided_at_ms),
            });
        }
    }
    ProfitCutoffState {
        day: day.to_string(),
        targets,
    }
}

fn cutoff_state_for_day<'a>(
    settings: &'a SpecialOpsSettings,
    day: &str,
) -> Option<&'a ProfitCutoffState> {
    settings
        .profit_filter
        .cutoff_state
        .as_ref()
        .filter(|state| state.day == day)
}

fn cutoff_state_complete(settings: &SpecialOpsSettings, day: &str) -> bool {
    cutoff_state_for_day(settings, day).is_some_and(|state| {
        state
            .targets
            .iter()
            .all(|target| target.decided_at_ms.is_some())
    })
}

fn cutoff_qualified_targets(
    settings: &SpecialOpsSettings,
    day: &str,
) -> std::collections::HashSet<(String, String)> {
    cutoff_state_for_day(settings, day)
        .into_iter()
        .flat_map(|state| state.targets.iter())
        .filter(|target| target.decided_at_ms.is_some() && target.skip_reason.is_none())
        .map(|target| (target.account_id.clone(), target.target_id.clone()))
        .collect()
}

fn cutoff_pending_rule_ids(
    settings: &SpecialOpsSettings,
    day: &str,
) -> std::collections::HashSet<String> {
    cutoff_state_for_day(settings, day)
        .into_iter()
        .flat_map(|state| state.targets.iter())
        .filter(|target| target.decided_at_ms.is_none())
        .filter_map(|target| target.rule_id.clone())
        .collect()
}

fn cutoff_retry_at_ms(settings: &SpecialOpsSettings, day: &str) -> Option<i64> {
    let pending = cutoff_pending_rule_ids(settings, day);
    settings
        .profit_filter
        .audits
        .iter()
        .filter(|audit| audit.day == day && pending.contains(&audit.rule_id))
        .filter_map(|audit| audit.next_query_at_ms)
        .min()
}

fn replace_profit_audits(settings: &mut SpecialOpsSettings, audits: Vec<AmmoProfitAudit>) {
    let replaced = audits
        .iter()
        .map(|audit| (audit.day.clone(), audit.rule_id.clone()))
        .collect::<std::collections::HashSet<_>>();
    settings
        .profit_filter
        .audits
        .retain(|audit| !replaced.contains(&(audit.day.clone(), audit.rule_id.clone())));
    settings.profit_filter.audits.extend(audits);
    settings.profit_filter.audits.sort_by(|left, right| {
        (&left.day, left.queried_at_ms, &left.rule_id).cmp(&(
            &right.day,
            right.queried_at_ms,
            &right.rule_id,
        ))
    });
}

fn is_stale_profit_query_error(error: &str) -> bool {
    error == PROFIT_QUERY_STALE
        || error.contains("配置保存已陈旧")
        || error.contains("利润查询已取消")
}

fn build_profit_query_window(
    settings: &SpecialOpsSettings,
    now_ms: i64,
    settings_revision: u64,
    active_round: bool,
) -> Result<ProfitQueryWindow, String> {
    let day = local_day_and_minute(now_ms).0;
    let exchange_minute = daily_exchange_minutes(&settings.daily_exchange_time)
        .ok_or_else(|| "每日兑换时间必须是 HH:mm，范围 00:00-23:59".to_string())?;
    let exchange_at_ms = daily_exchange_at_ms(now_ms, exchange_minute)
        .ok_or_else(|| "无法计算每日兑换时间".to_string())?;
    let cutoff_at_ms = if settings.profit_filter.enabled {
        let cutoff_minute = daily_exchange_minutes(&settings.profit_filter.cutoff_time)
            .ok_or_else(|| "利润截止时间必须是 HH:mm，范围 00:00-23:59".to_string())?;
        daily_exchange_at_ms(now_ms, cutoff_minute)
            .ok_or_else(|| "无法计算利润截止时间".to_string())?
    } else {
        exchange_at_ms
    };
    let cutoff_complete = cutoff_state_complete(settings, &day);
    let cutoff_retry_at_ms = cutoff_retry_at_ms(settings, &day);
    Ok(ProfitQueryWindow {
        enabled: settings.profit_filter.enabled,
        paused: settings.paused,
        active_round,
        day,
        settings_revision,
        now_ms,
        exchange_at_ms,
        cutoff_at_ms,
        cutoff_complete,
        cutoff_retry_at_ms,
    })
}

fn profit_gate_for_round(
    settings: &SpecialOpsSettings,
    profit_runtime: &ProfitQueryControl,
    now_ms: i64,
    settings_revision: u64,
) -> Result<(AmmoProfitGate, Option<u64>), String> {
    if !settings.profit_filter.enabled {
        return Ok((AmmoProfitGate::Disabled, None));
    }
    let window = build_profit_query_window(settings, now_ms, settings_revision, false)?;
    let snapshot = profit_runtime.sync_window(window.clone())?;
    if now_ms >= window.cutoff_at_ms {
        return Ok((
            AmmoProfitGate::QualifiedTargets(cutoff_qualified_targets(settings, &window.day)),
            Some(profit_runtime.generation()),
        ));
    }
    Ok((
        AmmoProfitGate::Qualified(
            snapshot
                .qualified_rule_ids
                .into_iter()
                .collect::<std::collections::HashSet<_>>(),
        ),
        Some(profit_runtime.generation()),
    ))
}

fn apply_account_recipe_selection(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    station_kind: StationKind,
    rect: CalibrationRect,
) -> Result<(), String> {
    validate_calibration_selection(CalibrationTargetKind::ClickPoint, &rect)?;
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "账号不存在".to_string())?;
    if !account.independent_settings_enabled {
        return Err("账号未开启独立设置".to_string());
    }
    let business = account
        .independent_business_config
        .as_mut()
        .ok_or_else(|| "账号独立业务配置缺失".to_string())?;
    if let Some(point) = business
        .recipe_points
        .iter_mut()
        .find(|point| point.kind == station_kind)
    {
        point.rect = rect;
    } else {
        business.recipe_points.push(AccountRecipePoint {
            kind: station_kind,
            rect,
        });
    }
    Ok(())
}

const BUSINESS_AMMO_TARGET_PREFIX: &str = "business.ammo.";
const BUSINESS_MARKET_PRODUCT_TARGET: &str = "business.market.product";

fn business_ammo_target_id(target_key: &str) -> Option<&str> {
    target_key.strip_prefix(BUSINESS_AMMO_TARGET_PREFIX)
}

fn ammo_business_config_mut<'a>(
    settings: &'a mut SpecialOpsSettings,
    account_id: Option<&str>,
) -> Result<&'a mut BusinessConfig, String> {
    let Some(account_id) = account_id else {
        return Ok(&mut settings.default_business_config);
    };
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "账号不存在".to_string())?;
    if !account.independent_settings_enabled {
        return Err("账号未开启独立设置".to_string());
    }
    account
        .independent_business_config
        .as_mut()
        .ok_or_else(|| "账号独立业务配置缺失".to_string())
}

fn apply_ammo_business_selection(
    settings: &mut SpecialOpsSettings,
    account_id: Option<&str>,
    target_id: &str,
    rect: CalibrationRect,
) -> Result<(), String> {
    validate_calibration_selection(CalibrationTargetKind::ClickPoint, &rect)?;
    let business = ammo_business_config_mut(settings, account_id)?;
    let target = business
        .ammo_targets
        .iter_mut()
        .find(|target| target.id == target_id)
        .ok_or_else(|| "子弹目标不存在".to_string())?;
    target.click_point = Some(rect);
    Ok(())
}

fn apply_market_business_selection(
    settings: &mut SpecialOpsSettings,
    account_id: Option<&str>,
    rect: CalibrationRect,
) -> Result<(), String> {
    validate_calibration_selection(CalibrationTargetKind::ClickPoint, &rect)?;
    ammo_business_config_mut(settings, account_id)?
        .market
        .product_point = Some(rect);
    Ok(())
}

fn calibration_selection_kind(
    settings: &SpecialOpsSettings,
    environment_id: &str,
    target_key: &str,
    account_id: Option<&str>,
) -> Result<CalibrationTargetKind, String> {
    if target_key == BUSINESS_MARKET_PRODUCT_TARGET {
        let business = if let Some(account_id) = account_id {
            let account = settings
                .accounts
                .iter()
                .find(|account| account.id == account_id)
                .ok_or_else(|| "账号不存在".to_string())?;
            if !account.independent_settings_enabled {
                return Err("账号未开启独立设置".to_string());
            }
            account
                .independent_business_config
                .as_ref()
                .ok_or_else(|| "账号独立业务配置缺失".to_string())?
        } else {
            &settings.default_business_config
        };
        let _ = &business.market;
        return Ok(CalibrationTargetKind::ClickPoint);
    }
    if let Some(target_id) = business_ammo_target_id(target_key) {
        let business = if let Some(account_id) = account_id {
            let account = settings
                .accounts
                .iter()
                .find(|account| account.id == account_id)
                .ok_or_else(|| "账号不存在".to_string())?;
            if !account.independent_settings_enabled {
                return Err("账号未开启独立设置".to_string());
            }
            account
                .independent_business_config
                .as_ref()
                .ok_or_else(|| "账号独立业务配置缺失".to_string())?
        } else {
            &settings.default_business_config
        };
        business
            .ammo_targets
            .iter()
            .find(|target| target.id == target_id)
            .ok_or_else(|| "子弹目标不存在".to_string())?;
        return Ok(CalibrationTargetKind::ClickPoint);
    }

    let target = settings
        .calibration_environments
        .iter()
        .find(|item| item.id == environment_id)
        .and_then(|environment| {
            environment
                .targets
                .iter()
                .find(|item| item.key == target_key)
        })
        .ok_or_else(|| "校准目标不存在".to_string())?;
    if let Some(account_id) = account_id {
        if target.kind != CalibrationTargetKind::ClickPoint {
            return Err("账号级校准只允许点击点".to_string());
        }
        account_recipe_station(target_key)?;
        let account = settings
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .ok_or_else(|| "账号不存在".to_string())?;
        if !account.independent_settings_enabled {
            return Err("账号未开启独立设置".to_string());
        }
        account
            .independent_business_config
            .as_ref()
            .ok_or_else(|| "账号独立业务配置缺失".to_string())?;
    }
    Ok(target.kind.clone())
}

fn account_recipe_station(target_key: &str) -> Result<StationKind, String> {
    let suffix = target_key
        .strip_prefix("craft.recipe.")
        .ok_or_else(|| "账号级校准只允许制作物品选择点击点".to_string())?;
    StationKind::from_calibration_suffix(suffix)
        .ok_or_else(|| "账号级制作物品选择点击点不存在".to_string())
}

fn calibration_selection_label(
    environment_id: &str,
    target_key: &str,
    account_id: Option<&str>,
) -> String {
    format!(
        "special-ops-calibration-{}-{}-{}",
        safe_label_component(environment_id),
        safe_label_component(target_key),
        safe_label_component(account_id.unwrap_or("global"))
    )
}

/// 把异常状态下残留的 `Uncertain` 制作台按存量计时恢复成正常状态。
/// 失败落库只改 `status`，`started_at_ms` / `finishes_at_ms` 原样保留，因此这里
/// 不需要额外快照字段就能还原异常前的剩余时间。
fn restore_station_from_stored_timing(station: &mut StationPlan, now_ms: i64) {
    if station.status != StationStatus::Uncertain {
        return;
    }
    station.status = match station.finishes_at_ms {
        Some(finishes_at_ms) if finishes_at_ms > now_ms => StationStatus::Crafting,
        Some(_) => StationStatus::Ready,
        None => StationStatus::Idle,
    };
}

fn resolve_station_correction(
    correction: &StationCorrectionInput,
    confirmed_at_ms: i64,
    stored_finishes_at_ms: Option<i64>,
) -> Result<(Option<i64>, Option<i64>, StationStatus), String> {
    match correction.state {
        ManualStationState::ImmediateDue => {
            if correction.remaining_minutes.is_some() {
                return Err("立即到期不能填写剩余时间".to_string());
            }
            Ok((None, Some(confirmed_at_ms), StationStatus::Ready))
        }
        ManualStationState::Crafting => {
            let finishes_at_ms = match correction.remaining_minutes {
                Some(minutes) => {
                    if !(1..=10_080).contains(&minutes) {
                        return Err("正在制作的剩余时间必须为 1 分钟到 168 小时".to_string());
                    }
                    confirmed_at_ms
                        .checked_add(i64::from(minutes) * 60_000)
                        .ok_or_else(|| "制作完成时间超出可保存范围".to_string())?
                }
                // 未填剩余时间时继承异常前的存量完成时间，避免人工判定丢失制作进度。
                None => stored_finishes_at_ms
                    .filter(|value| *value > confirmed_at_ms)
                    .ok_or_else(|| "缺少可继承的剩余时间，请填写 1 分钟到 168 小时".to_string())?,
            };
            Ok((None, Some(finishes_at_ms), StationStatus::Crafting))
        }
        ManualStationState::Idle => {
            if correction.remaining_minutes.is_some() {
                return Err("空闲状态不能填写剩余时间".to_string());
            }
            Ok((None, None, StationStatus::Idle))
        }
    }
}

fn apply_manual_station_corrections(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    corrections: &[StationCorrectionInput],
    confirmed_at_ms: i64,
) -> Result<(), String> {
    if corrections.len() != StationKind::all().len() {
        return Err("必须一次确认四个制作台的实际状态".to_string());
    }
    // 先只读取存量完成时间，供未填剩余时间的「正在制作」继承；随后才取可变账号引用。
    let stored_finishes = settings
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .map(|account| {
            account
                .stations
                .iter()
                .map(|station| (station.kind.clone(), station.finishes_at_ms))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut kinds = Vec::new();
    let mut resolved = Vec::with_capacity(corrections.len());
    for correction in corrections {
        if kinds.contains(&correction.kind) {
            return Err("四制作台人工校正包含重复制作台".to_string());
        }
        kinds.push(correction.kind.clone());
        let stored_finishes_at_ms = stored_finishes
            .iter()
            .find(|(kind, _)| *kind == correction.kind)
            .and_then(|(_, value)| *value);
        let (started_at_ms, finishes_at_ms, status) =
            resolve_station_correction(correction, confirmed_at_ms, stored_finishes_at_ms)?;
        resolved.push((
            correction.kind.clone(),
            started_at_ms,
            finishes_at_ms,
            status,
        ));
    }
    if StationKind::all()
        .into_iter()
        .any(|kind| !kinds.contains(&kind))
    {
        return Err("必须一次确认四个制作台的实际状态".to_string());
    }

    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "人工校正账号不存在".to_string())?;
    if resolved
        .iter()
        .any(|(kind, ..)| !account.stations.iter().any(|station| station.kind == *kind))
    {
        return Err("账号缺少制作台运行状态，无法人工校正".to_string());
    }
    for (kind, started_at_ms, finishes_at_ms, status) in resolved {
        let station = account
            .stations
            .iter_mut()
            .find(|station| station.kind == kind)
            .expect("制作台存在性已校验");
        station.started_at_ms = started_at_ms;
        station.finishes_at_ms = finishes_at_ms;
        station.status = status;
    }
    account.initialized = true;
    if account.status == AccountStatus::Uncertain {
        account.status = AccountStatus::Ready;
    }
    Ok(())
}

fn refresh_account_failure(account: &mut AccountPlan) {
    if account
        .last_failure
        .as_ref()
        .is_some_and(|failure| failure.station_kind.is_some())
    {
        return;
    }
    account.last_failure = account
        .ammo_targets
        .iter()
        .filter_map(|target| target.last_failure.as_ref())
        .max_by_key(|failure| failure.at_ms)
        .cloned();
}

fn account_allows_manual_check(status: &AccountStatus) -> bool {
    matches!(
        status,
        AccountStatus::NeedsManualLogin
            | AccountStatus::LoginFailed
            | AccountStatus::ManualCheckRequired
    )
}

fn apply_account_manual_check(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    confirmed_at_ms: i64,
) -> Result<(), String> {
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "人工检查账号不存在".to_string())?;
    if !account_allows_manual_check(&account.status) {
        return Err("当前账号状态不能通过“已人工检查”恢复".to_string());
    }
    account.status = AccountStatus::Ready;
    account.last_failure = None;
    // 只清账号状态会留下 Uncertain 制作台，它在调度与任务栏双重过滤下永久消失。
    for station in account.stations.iter_mut() {
        restore_station_from_stored_timing(station, confirmed_at_ms);
    }
    Ok(())
}

/// 一键恢复：把异常状态清成正常可调度状态。
/// 账号状态回 `Ready`，`Uncertain` 制作台按存量计时还原异常前的剩余时间，
/// 子弹目标回未兑换（含清当天 `last_success_day`），限时商品 `Failed` 回 `Pending`。
/// 返回实际恢复的账号数。
fn restore_account_state(
    settings: &mut SpecialOpsSettings,
    account_id: Option<&str>,
    confirmed_at_ms: i64,
    current_day: &str,
) -> Result<usize, String> {
    if let Some(account_id) = account_id {
        if !settings
            .accounts
            .iter()
            .any(|account| account.id == account_id)
        {
            return Err("一键恢复账号不存在".to_string());
        }
    }
    let mut restored = 0usize;
    for account in settings.accounts.iter_mut() {
        if account_id.is_some_and(|target| target != account.id) {
            continue;
        }
        let mut changed = account.status != AccountStatus::Ready || account.last_failure.is_some();
        account.status = AccountStatus::Ready;
        account.last_failure = None;
        for station in account.stations.iter_mut() {
            if station.status == StationStatus::Uncertain {
                changed = true;
            }
            restore_station_from_stored_timing(station, confirmed_at_ms);
        }
        for target in account.ammo_targets.iter_mut() {
            if target.last_failure.is_some()
                || target.retry_count > 0
                || target.last_success_day.as_deref() == Some(current_day)
            {
                changed = true;
            }
            // 清 last_failure、当天成功标记与重试预算 -> 目标回到未兑换、可再次调度。
            // 当天已成功的目标也会被清：流程内还有资格与库存检查分支兜底重复兑换。
            target.last_failure = None;
            if target.last_success_day.as_deref() == Some(current_day) {
                target.last_success_day = None;
            }
            target.retry_day = Some(current_day.to_string());
            target.retry_count = 0;
        }
        if account.limited_supply.outcome == limited_supply::LimitedSupplyOutcome::Failed {
            account.limited_supply.outcome = limited_supply::LimitedSupplyOutcome::Pending;
            changed = true;
        }
        if changed {
            restored = restored.saturating_add(1);
        }
    }
    if restored == 0 {
        return Err("当前没有需要恢复的异常状态".to_string());
    }
    Ok(restored)
}

/// 只有登录环节被卡住的账号才禁止任务级人工判定。
/// `ManualCheckRequired` 说明轮次已经跑到业务步骤，任务栏单项判定是有效恢复手段。
fn account_blocks_task_correction(status: &AccountStatus) -> bool {
    matches!(
        status,
        AccountStatus::NeedsManualLogin | AccountStatus::LoginFailed
    )
}

fn apply_single_station_correction(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    correction: &StationCorrectionInput,
    confirmed_at_ms: i64,
) -> Result<(), String> {
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "人工校正账号不存在".to_string())?;
    if !account.initialized {
        return Err("账号尚未完成初始化，必须在账号页进行完整人工校正".to_string());
    }
    if account_blocks_task_correction(&account.status) {
        return Err("需要人工登录或登录失败账号不能通过单项校正恢复".to_string());
    }
    let station_scoped_failure = account
        .last_failure
        .as_ref()
        .and_then(|failure| failure.station_kind.as_ref())
        == Some(&correction.kind);
    // 账号级失败（如导航超时）没有制作台定位，此时允许按任务逐台判定。
    let account_scoped_failure =
        account.last_failure.as_ref().is_some_and(|failure| {
            failure.station_kind.is_none() && failure.ammo_target_id.is_none()
        }) || account.status == AccountStatus::ManualCheckRequired;
    if !station_scoped_failure && !account_scoped_failure {
        return Err("当前制作台没有待人工判定的失败记录".to_string());
    }
    let stored_finishes_at_ms = account
        .stations
        .iter()
        .find(|station| station.kind == correction.kind)
        .and_then(|station| station.finishes_at_ms);
    let resolved = resolve_station_correction(correction, confirmed_at_ms, stored_finishes_at_ms)?;
    let station = account
        .stations
        .iter_mut()
        .find(|station| station.kind == correction.kind)
        .ok_or_else(|| "账号缺少当前制作台运行状态".to_string())?;
    station.started_at_ms = resolved.0;
    station.finishes_at_ms = resolved.1;
    station.status = resolved.2;
    if station_scoped_failure {
        account.last_failure = None;
        refresh_account_failure(account);
        account.status = AccountStatus::Ready;
        return Ok(());
    }
    // 账号级失败要等所有制作台都脱离 Uncertain 才恢复，否则残留台会永久掉出调度。
    if account
        .stations
        .iter()
        .all(|station| station.status != StationStatus::Uncertain)
    {
        account.last_failure = None;
        refresh_account_failure(account);
        if account
            .last_failure
            .as_ref()
            .is_none_or(|failure| failure.station_kind.is_none())
        {
            account.status = AccountStatus::Ready;
        }
    }
    Ok(())
}

fn apply_single_ammo_correction(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    correction: &AmmoCorrectionInput,
    current_day: &str,
) -> Result<(), String> {
    {
        let account = settings
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .ok_or_else(|| "人工校正账号不存在".to_string())?;
        if !account.initialized {
            return Err("账号尚未完成初始化，必须在账号页进行完整人工校正".to_string());
        }
        if account_blocks_task_correction(&account.status) {
            return Err("需要人工登录或登录失败账号不能通过单项校正恢复".to_string());
        }
        let enabled = resolve_account_business_config(settings, account)?
            .ammo_targets
            .iter()
            .any(|target| target.id == correction.target_id && target.enabled);
        if !enabled {
            return Err("当前子弹目标不存在或未启用".to_string());
        }
    }

    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .expect("上方已校验账号存在");
    let target = account
        .ammo_targets
        .iter_mut()
        .find(|target| target.id == correction.target_id && target.enabled)
        .ok_or_else(|| "账号缺少当前子弹运行状态".to_string())?;
    if target
        .last_failure
        .as_ref()
        .and_then(|failure| failure.ammo_target_id.as_deref())
        != Some(target.id.as_str())
    {
        return Err("当前子弹没有待人工判定的失败记录".to_string());
    }
    if correction.succeeded_today {
        target.last_success_day = Some(current_day.to_string());
    } else if target.last_success_day.as_deref() == Some(current_day) {
        target.last_success_day = None;
    }
    target.retry_day = Some(current_day.to_string());
    target.retry_count = 0;
    target.last_failure = None;
    refresh_account_failure(account);
    if account
        .last_failure
        .as_ref()
        .is_none_or(|failure| failure.station_kind.is_none())
    {
        account.status = AccountStatus::Ready;
    }
    Ok(())
}

fn apply_manual_account_corrections(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    stations: &[StationCorrectionInput],
    ammo_targets: &[AmmoCorrectionInput],
    confirmed_at_ms: i64,
    current_day: &str,
) -> Result<(), String> {
    let source_account = settings
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "人工校正账号不存在".to_string())?;
    if source_account.initialized
        && !matches!(
            source_account.status,
            AccountStatus::Ready | AccountStatus::Uncertain | AccountStatus::Isolated
        )
    {
        return Err("需要人工登录或登录失败账号不能通过制作状态校正恢复".to_string());
    }

    let business_config = resolve_account_business_config(settings, source_account)?;
    let enabled_ids = business_config
        .ammo_targets
        .iter()
        .filter(|target| target.enabled)
        .map(|target| target.id.clone())
        .collect::<Vec<_>>();
    if ammo_targets.len() != enabled_ids.len() {
        return Err("必须一次确认全部启用子弹目标的当天状态".to_string());
    }
    let mut correction_ids = std::collections::HashSet::new();
    for correction in ammo_targets {
        if !correction_ids.insert(correction.target_id.as_str()) {
            return Err("子弹人工校正包含重复目标".to_string());
        }
        if !enabled_ids.contains(&correction.target_id) {
            return Err(format!(
                "子弹人工校正包含未启用目标：{}",
                correction.target_id
            ));
        }
    }

    let mut next = settings.clone();
    apply_manual_station_corrections(&mut next, account_id, stations, confirmed_at_ms)?;
    let account = next
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .expect("上方已校验账号存在");
    for correction in ammo_targets {
        let target = account
            .ammo_targets
            .iter_mut()
            .find(|target| target.id == correction.target_id)
            .ok_or_else(|| format!("账号缺少子弹运行状态：{}", correction.target_id))?;
        if correction.succeeded_today {
            target.last_success_day = Some(current_day.to_string());
        } else if target.last_success_day.as_deref() == Some(current_day) {
            target.last_success_day = None;
        }
        target.retry_day = Some(current_day.to_string());
        target.retry_count = 0;
    }
    for target in &mut account.ammo_targets {
        target.last_failure = None;
    }
    account.initialized = true;
    account.status = AccountStatus::Ready;
    account.last_failure = None;
    *settings = next;
    Ok(())
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
    let reference_path = target
        .reference_image_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| format!("{} 尚未上传参考图", target.label))?;
    calibration_signature_with_template(target, reference_path, target.match_threshold)
}

fn calibration_signature_with_template(
    target: &CalibrationTarget,
    reference_path: &str,
    match_threshold: f32,
) -> Result<String, String> {
    let rect = target
        .rect
        .as_ref()
        .ok_or_else(|| format!("{} 尚未框选", target.label))?;
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
        match_threshold
    ))
}

fn resolved_template_config<'a>(
    _environment: &'a CalibrationEnvironment,
    target: &'a CalibrationTarget,
) -> Result<(&'a str, f32), String> {
    let path = target
        .reference_image_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| format!("{} 尚未上传参考图", target.label))?;
    Ok((path, target.match_threshold))
}

fn resolved_calibration_signature(
    environment: &CalibrationEnvironment,
    target: &CalibrationTarget,
) -> Result<String, String> {
    let (reference_path, match_threshold) = resolved_template_config(environment, target)?;
    calibration_signature_with_template(target, reference_path, match_threshold)
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
                station_kind: None,
                ammo_target_id: None,
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
                    station_kind: None,
                    ammo_target_id: None,
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
                        station_kind: None,
                        ammo_target_id: None,
                    });
                }
            }
        },
        login_flow::LoginFlowResult::NeedsManualLogin {
            failed_step,
            failure_message,
            failed_at,
            ..
        } => {
            account.status = AccountStatus::NeedsManualLogin;
            account.last_failure = Some(AccountFailure {
                step: format!("{failed_step:?}"),
                message: failure_message.clone(),
                at_ms: *failed_at,
                station_kind: None,
                ammo_target_id: None,
            });
        }
    }
    Ok(())
}

fn apply_round_account_failure(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    error: &round_runner::AccountRunError,
    failed_at_ms: i64,
) -> Result<(), String> {
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "多账号轮次账号已不存在".to_string())?;
    if let Some(target_id) = error.ammo_target_id.as_deref() {
        set_ammo_manual_failure(
            account,
            target_id,
            &error.step,
            &error.message,
            failed_at_ms,
        )?;
        if error.step == "ammo.isolated" {
            account.status = AccountStatus::Isolated;
            account.last_failure = Some(AccountFailure {
                step: error.step.clone(),
                message: error.message.clone(),
                at_ms: failed_at_ms,
                station_kind: None,
                ammo_target_id: Some(target_id.to_string()),
            });
        }
        return Ok(());
    }
    let account_isolated = matches!(error.step.as_str(), "ammo.isolated" | "craft.isolated");
    account.status = if error.kind == round_runner::AccountRunErrorKind::NavigationTimedOut {
        AccountStatus::ManualCheckRequired
    } else if error.step == "login.needsManual" {
        AccountStatus::NeedsManualLogin
    } else if error.step.starts_with("login.") {
        AccountStatus::LoginFailed
    } else if account_isolated {
        AccountStatus::Isolated
    } else {
        AccountStatus::Uncertain
    };
    if !account_isolated {
        if let Some(station_kind) = error.station.as_ref() {
            let station = account
                .stations
                .iter_mut()
                .find(|station| station.kind == *station_kind)
                .ok_or_else(|| "多账号轮次当前制作台已不存在".to_string())?;
            station.status = StationStatus::Uncertain;
        }
    }
    account.last_failure = Some(AccountFailure {
        step: error.step.clone(),
        message: error.message.clone(),
        at_ms: failed_at_ms,
        station_kind: error.station.clone(),
        ammo_target_id: error.ammo_target_id.clone(),
    });
    Ok(())
}

fn mark_craft_cancel_uncertain(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    station: StationKind,
    at_ms: i64,
) -> Result<(), String> {
    mark_craft_uncertain(
        settings,
        account_id,
        station,
        at_ms,
        "craftCancel",
        "制作试运行取消时已执行键鼠输入，请人工确认制作状态并修正完成时间",
    )
}

fn mark_craft_uncertain(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    station: StationKind,
    at_ms: i64,
    step: &str,
    message: &str,
) -> Result<(), String> {
    settings.paused = true;
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "制作试运行账号已不存在".to_string())?;
    account.status = AccountStatus::Uncertain;
    account.last_failure = Some(AccountFailure {
        step: step.to_string(),
        message: message.to_string(),
        at_ms,
        station_kind: Some(station.clone()),
        ammo_target_id: None,
    });
    account
        .stations
        .iter_mut()
        .find(|candidate| candidate.kind == station)
        .ok_or_else(|| "制作台不存在".to_string())?
        .status = StationStatus::Uncertain;
    Ok(())
}

fn should_mark_craft_stop_uncertain(
    stop_reason: Option<login_runtime::StopReason>,
    entered_input: bool,
) -> bool {
    match stop_reason {
        Some(login_runtime::StopReason::Normal) => entered_input,
        Some(login_runtime::StopReason::Emergency) => true,
        Some(login_runtime::StopReason::Lifecycle { uncertain }) => uncertain,
        None => false,
    }
}

fn normalize_business_config(config: &mut BusinessConfig) -> Result<(), String> {
    for station in &mut config.stations {
        station.recipe_note = station.recipe_note.trim().to_string();
    }
    if config.market.purchase_count == 0 {
        return Err("交易行每日购买次数必须是正整数".to_string());
    }
    config.market.item_note = config.market.item_note.trim().to_string();
    config.ammo_targets.sort_by_key(|target| target.order);
    let mut ids = std::collections::HashSet::new();
    for (order, target) in config.ammo_targets.iter_mut().enumerate() {
        target.id = target.id.trim().to_string();
        target.note = target.note.trim().to_string();
        target.profit_rule_id = target
            .profit_rule_id
            .take()
            .map(|rule_id| rule_id.trim().to_string())
            .filter(|rule_id| !rule_id.is_empty());
        if target.id.is_empty() || !ids.insert(target.id.clone()) {
            return Err("子弹目标 ID 必须非空且唯一".to_string());
        }
        if let Some(point) = target.click_point.as_ref() {
            validate_calibration_selection(CalibrationTargetKind::ClickPoint, point)?;
        }
        target.scroll_direction = ScrollDirection::Down;
        target.order = order as u32;
    }
    if config.market.max_price == 0 {
        return Err("交易行设定价格必须是正整数".to_string());
    }
    if let Some(point) = config.market.product_point.as_ref() {
        validate_calibration_selection(CalibrationTargetKind::ClickPoint, point)?;
    }
    Ok(())
}

fn migrate_legacy_market_business_config(settings: &mut SpecialOpsSettings) {
    let legacy_market = settings.market_purchase.clone();
    let legacy_has_business_values = legacy_market.enabled
        || legacy_market.purchase_count != 1
        || !legacy_market.item_note.trim().is_empty();
    let market = &mut settings.default_business_config.market;
    if market.schema_version == 0 {
        market.enabled = settings.market_purchase.enabled;
        market.purchase_count = settings.market_purchase.purchase_count;
        market.item_note = settings.market_purchase.item_note.trim().to_string();
        market.schema_version = 1;
        settings.market_purchase.enabled = false;
        settings.market_purchase.purchase_count = 1;
        settings.market_purchase.item_note.clear();
    }
    let inherited_market = settings.default_business_config.market.clone();
    for account in settings
        .accounts
        .iter_mut()
        .filter(|account| account.independent_settings_enabled)
    {
        let Some(config) = account.independent_business_config.as_mut() else {
            continue;
        };
        let market = &mut config.market;
        if market.schema_version == 0 {
            if legacy_has_business_values {
                market.enabled = legacy_market.enabled;
                market.purchase_count = legacy_market.purchase_count;
                market.item_note = legacy_market.item_note.trim().to_string();
            } else {
                market.enabled = inherited_market.enabled;
                market.purchase_count = inherited_market.purchase_count;
                market.item_note = inherited_market.item_note.clone();
            }
            market.schema_version = 1;
        }
    }
}

fn sync_ammo_runtime_targets(
    runtime_targets: &mut Vec<AmmoTarget>,
    business_targets: &[AmmoBusinessTarget],
) {
    let mut existing = std::mem::take(runtime_targets)
        .into_iter()
        .map(|target| (target.id.clone(), target))
        .collect::<std::collections::HashMap<_, _>>();
    *runtime_targets = business_targets
        .iter()
        .map(|business| {
            let mut runtime = existing.remove(&business.id).unwrap_or_else(|| AmmoTarget {
                id: business.id.clone(),
                name: String::new(),
                enabled: business.enabled,
                seasonal: business.seasonal,
                scroll_steps: business.scroll_steps,
                order: business.order,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            });
            runtime.name = business.note.clone();
            runtime.enabled = business.enabled;
            runtime.seasonal = business.seasonal;
            runtime.scroll_steps = business.scroll_steps;
            runtime.order = business.order;
            runtime
        })
        .collect();
}

fn validate_failure_target(failure: &AccountFailure) -> Result<(), String> {
    if failure.station_kind.is_some() && failure.ammo_target_id.is_some() {
        return Err("一条失败记录不能同时指向制作台和子弹目标".to_string());
    }
    Ok(())
}

pub(crate) fn normalize_settings(
    mut settings: SpecialOpsSettings,
) -> Result<SpecialOpsSettings, String> {
    if daily_exchange_minutes(&settings.daily_exchange_time).is_none() {
        return Err("每日兑换时间必须是 HH:mm，范围 00:00-23:59".to_string());
    }
    if settings.emergency_hotkey.trim().is_empty() {
        return Err("紧急停止快捷键不能为空".to_string());
    }
    normalize_profit_settings(&mut settings.profit_filter)?;
    if !(5_000..=60_000).contains(&settings.limited_supply.ready_timeout_ms) {
        return Err("研发部门页面就绪超时必须是 5000–60000ms 的整数".to_string());
    }
    if settings.market_purchase.entry_delay_ms > 60_000 {
        return Err("交易行入口等待时间必须是 0–60000ms 的整数".to_string());
    }
    if settings.market_purchase.purchase_count == 0 {
        return Err("交易行每日购买次数必须是正整数".to_string());
    }
    settings.market_purchase.item_note = settings.market_purchase.item_note.trim().to_string();
    if [
        settings.navigation_beacon_delay_ms,
        settings.navigation_space_delay_ms,
        settings.navigation_tab_delay_ms,
        settings.navigation_special_ops_delay_ms,
    ]
    .into_iter()
    .any(|value| value > 60_000)
    {
        return Err("游戏内导航等待时间必须是 0–60000ms 的整数".to_string());
    }
    if [
        settings.craft_space_delay_ms,
        settings.craft_reopen_delay_ms,
        settings.craft_confirm_pinned_delay_ms,
    ]
    .into_iter()
    .any(|value| value > 60_000)
    {
        return Err("制作台固定等待时间必须是 0–60000ms 的整数".to_string());
    }
    if [
        settings.ammo_supply_delay_ms,
        settings.ammo_tactical_delay_ms,
    ]
    .into_iter()
    .any(|value| value > 60_000)
    {
        return Err("子弹入口固定等待时间必须是 0–60000ms 的整数".to_string());
    }

    if settings.default_business_config.stations.is_empty() {
        settings.default_business_config = settings
            .accounts
            .first()
            .map(business_config_from_account)
            .unwrap_or_default();
        let inherited = settings.default_business_config.clone();
        for account in &mut settings.accounts {
            let legacy_business = business_config_from_account(account);
            account.independent_settings_enabled = legacy_business != inherited;
            account.independent_business_config = account
                .independent_settings_enabled
                .then_some(legacy_business);
        }
    }
    migrate_legacy_market_business_config(&mut settings);
    normalize_business_config(&mut settings.default_business_config)?;
    for account in &mut settings.accounts {
        if account.independent_settings_enabled {
            if account.independent_business_config.is_none() {
                return Err(format!(
                    "账号 {} 已开启独立设置，但独立业务配置缺失",
                    account.qq_account
                ));
            }
            normalize_business_config(
                account
                    .independent_business_config
                    .as_mut()
                    .expect("上方已校验独立业务配置存在"),
            )?;
        } else {
            account.independent_business_config = None;
        }
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
                let kind_changed = target.kind != required.kind;
                target.label = required.label.clone();
                target.kind = required.kind.clone();
                target.recognition_method = required.recognition_method.clone();
                target.guard_any_of = required.guard_any_of.clone();
                if kind_changed {
                    target.rect = None;
                    target.reference_image_path = None;
                    target.verified_signature = None;
                    target.verified_at_ms = None;
                }
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
    let default_ammo_targets = &settings.default_business_config.ammo_targets;
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
        if let Some(failure) = account.last_failure.as_ref() {
            validate_failure_target(failure)?;
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
            if let Some(failure) = target.last_failure.as_ref() {
                validate_failure_target(failure)?;
                if failure.station_kind.is_some()
                    || failure.ammo_target_id.as_deref() != Some(target.id.as_str())
                {
                    return Err("子弹失败记录与目标 ID 不一致".to_string());
                }
            }
            target.order = order as u32;
        }
        let business_targets = account
            .independent_business_config
            .as_ref()
            .map_or(default_ammo_targets.as_slice(), |config| {
                config.ammo_targets.as_slice()
            });
        sync_ammo_runtime_targets(&mut account.ammo_targets, business_targets);
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
        .filter_map(|account| {
            resolve_account_business_config(settings, account)
                .ok()
                .map(|business| (account, business))
        })
        .filter(|(_, business)| {
            business.stations.iter().any(|station| station.enabled)
                || business.ammo_targets.iter().any(|target| target.enabled)
                || settings.limited_supply.enabled
                || business.market.enabled
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
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<std::collections::HashSet<_>>();

    let has_crafting = active_accounts
        .iter()
        .any(|(_, business)| business.stations.iter().any(|station| station.enabled));
    if has_crafting {
        keys.extend(
            [
                "game.specialOps",
                "game.stationGrid",
                "craft.confirmPinned",
                "craft.returnToStationGrid",
                "craft.fill",
                "craft.purchase",
                "craft.produce",
                "craft.abort",
            ]
            .into_iter()
            .map(str::to_string),
        );
        for kind in StationKind::all() {
            if active_accounts.iter().any(|(_, business)| {
                business
                    .stations
                    .iter()
                    .any(|station| station.enabled && station.kind == kind)
            }) {
                let suffix = kind.calibration_suffix();
                for prefix in ["craft.station", "craft.recipe"] {
                    keys.insert(format!("{prefix}.{suffix}"));
                }
            }
        }
    }

    let has_ammo = active_accounts
        .iter()
        .any(|(_, business)| business.ammo_targets.iter().any(|target| target.enabled));
    if has_ammo {
        keys.extend(
            [
                "ammo.tacticalDepartment",
                "ammo.fill",
                "ammo.purchase",
                "ammo.exchange",
                "ammo.confirm",
                "ammo.success",
            ]
            .into_iter()
            .map(str::to_string),
        );
        if active_accounts.iter().any(|(_, business)| {
            business
                .ammo_targets
                .iter()
                .any(|target| target.enabled && target.seasonal)
        }) {
            keys.insert("ammo.seasonal".to_string());
        }
    }
    if has_ammo || settings.limited_supply.enabled {
        keys.extend(
            ["ammo.department", "ammo.supply", "ammo.enterSupply"]
                .into_iter()
                .map(str::to_string),
        );
    }
    if settings.limited_supply.enabled {
        keys.extend(
            [
                "ammo.researchDepartment",
                "limited.ready",
                "limited.color.1",
                "limited.color.2",
                "limited.color.3",
                "limited.color.4",
                "limited.color.5",
                "limited.color.6",
                "limited.color.7",
                "limited.color.8",
                "limited.color.9",
            ]
            .into_iter()
            .map(str::to_string),
        );
    }
    keys
}

fn validate_execution_ready(settings: &SpecialOpsSettings) -> Result<(), String> {
    validate_profit_configuration(
        &settings.profit_filter,
        &settings.daily_exchange_time,
        &settings.default_business_config,
        &settings.accounts,
    )?;
    let required_keys = required_execution_target_keys(settings);
    if required_keys.is_empty() {
        return Ok(());
    }
    for account in settings
        .accounts
        .iter()
        .filter(|account| account.enabled && account.status == AccountStatus::Ready)
    {
        let business = resolve_account_business_config(settings, account)?;
        if !business.stations.iter().any(|station| station.enabled)
            && !business.ammo_targets.iter().any(|target| target.enabled)
            && !settings.limited_supply.enabled
            && !business.market.enabled
        {
            continue;
        }
        if account.qq_account.is_empty()
            || !account.qq_account.chars().all(|ch| ch.is_ascii_digit())
        {
            return Err(format!("账号 {} 的 QQ 必须为非空纯数字", account.id));
        }
        for station in business.stations.iter().filter(|station| station.enabled) {
            if !(1..=168 * 60).contains(&station.duration_minutes) {
                return Err(format!("账号 {} 的制作台配置不完整", account.id));
            }
        }
        if business
            .ammo_targets
            .iter()
            .any(|target| target.enabled && target.note.trim().is_empty())
        {
            return Err(format!("账号 {} 存在未命名的子弹目标", account.id));
        }
        if let Some(target) = business
            .ammo_targets
            .iter()
            .find(|target| target.enabled && target.click_point.is_none())
        {
            return Err(format!(
                "账号 {} 的子弹目标 {}（{}）点击点未配置",
                account.id, target.note, target.id
            ));
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
            if !verification_is_current(target) {
                return Err(format!("校准未完成：{} 尚未测试或验证失效", target.label));
            }
        }
    }
    Ok(())
}

fn apply_paused_state(settings: &mut SpecialOpsSettings, paused: bool) -> Result<(), String> {
    if !paused {
        validate_execution_ready(settings)?;
    }
    settings.paused = paused;
    // 用户显式切换暂停 -> 自动暂停原因失效，避免旧原因一直挂在 UI 上。
    settings.paused_reason = None;
    Ok(())
}

// 登录试运行 command 在后续任务接入。
#[allow(dead_code)]
fn validate_login_trial_ready(
    settings: &SpecialOpsSettings,
    account_id: &str,
) -> Result<(), String> {
    mouse_parking_region(settings)?;
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
        return Err(format!(
            "登录试运行账号 {account_id} 的 QQ 必须为非空纯数字"
        ));
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
            wegame_executable_path: std::fs::canonicalize(&settings.wegame_executable_path)
                .map_err(|_| "WeGame.exe 路径无法规范化".to_string())?,
            game_executable_path: std::fs::canonicalize(&settings.game_executable_path)
                .map_err(|_| "游戏 .exe 路径无法规范化".to_string())?,
            mouse_parking_region: mouse_parking_region(settings)?,
            targets,
        },
        login_trial_signature(settings, account)?,
    ))
}

/// 冻结游戏内导航试运行输入，只依赖当前游戏窗口、两个模板、两个点击点与三段等待。
fn freeze_navigation_run_config(
    settings: &SpecialOpsSettings,
    account_id: &str,
    destination: game_navigation::NavigationDestination,
) -> Result<game_navigation::NavigationRunConfig, String> {
    let account = settings
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .ok_or_else(|| format!("导航试运行账号 {account_id} 不存在"))?;
    if !account.enabled {
        return Err(format!("导航试运行账号 {account_id} 未启用"));
    }
    let game_path = std::path::Path::new(settings.game_executable_path.trim());
    if !game_path.is_absolute()
        || !game_path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        || !game_path.is_file()
    {
        return Err("导航试运行需要有效的游戏 .exe 绝对路径".to_string());
    }
    let environment = settings
        .calibration_environments
        .first()
        .ok_or_else(|| "导航试运行校准未完成：缺少显示环境".to_string())?;
    let mut targets = std::collections::HashMap::new();
    let mut keys = vec!["game.modeReady", "game.beaconMode"];
    if destination == game_navigation::NavigationDestination::StationGrid {
        keys.extend(["game.specialOps", "game.stationGrid"]);
    }
    for key in keys {
        let target = environment
            .targets
            .iter()
            .find(|target| target.key == key)
            .ok_or_else(|| format!("导航试运行校准目标 {key} 不存在"))?;
        let rect = target
            .rect
            .as_ref()
            .ok_or_else(|| format!("导航试运行校准未完成：{} 尚未框选", target.label))?;
        let region = crate::morse::types::RegionRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let template = match target.kind {
            CalibrationTargetKind::RecognitionRegion => {
                let reference = target
                    .reference_image_path
                    .as_deref()
                    .filter(|path| !path.trim().is_empty())
                    .ok_or_else(|| {
                        format!("导航试运行校准未完成：{} 尚未上传参考图", target.label)
                    })?;
                if !verification_is_current(target) {
                    return Err(format!(
                        "导航试运行校准未完成：{} 尚未测试或验证失效",
                        target.label
                    ));
                }
                Some(template_observer::RuntimeTemplate {
                    key: key.to_string(),
                    region: region.clone(),
                    reference_image_path: std::fs::canonicalize(reference)
                        .map_err(|_| format!("{} 的参考图文件不存在", target.label))?,
                    threshold: target.match_threshold,
                })
            }
            CalibrationTargetKind::ClickPoint => None,
            CalibrationTargetKind::InputRegion => {
                return Err(format!("导航校准目标 {} 类型无效", target.label));
            }
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
    Ok(game_navigation::NavigationRunConfig {
        game_executable_path: std::fs::canonicalize(game_path)
            .map_err(|_| "游戏 .exe 路径无法规范化".to_string())?,
        mouse_parking_region: mouse_parking_region(settings)?,
        targets,
        delays: game_navigation::NavigationDelays {
            beacon_ms: settings.navigation_beacon_delay_ms,
            space_ms: settings.navigation_space_delay_ms,
            tab_ms: settings.navigation_tab_delay_ms,
            special_ops_ms: settings.navigation_special_ops_delay_ms,
        },
        destination,
    })
}

/// 冻结单制作台试运行所需的游戏路径、制作台模板和制作时长。
fn freeze_craft_run_config(
    settings: &SpecialOpsSettings,
    account_id: &str,
    station_kind: StationKind,
) -> Result<(craft_runtime::CraftRunConfig, u32), String> {
    let account = settings
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "制作试运行账号不存在".to_string())?;
    if !account.enabled {
        return Err("制作试运行账号未启用".to_string());
    }
    account
        .stations
        .iter()
        .find(|station| station.kind == station_kind)
        .ok_or_else(|| "制作台配置不存在".to_string())?;
    let business_config = resolve_account_business_config(settings, account)?;
    let station = business_config
        .stations
        .iter()
        .find(|station| station.kind == station_kind)
        .ok_or_else(|| "制作台业务配置不存在".to_string())?;
    if !station.enabled || station.duration_minutes == 0 {
        return Err("制作台配置无效".to_string());
    }
    let environment = settings
        .calibration_environments
        .first()
        .ok_or_else(|| "制作校准环境不存在".to_string())?;
    let mut targets = std::collections::HashMap::new();
    let suffix = station_suffix(&station_kind);
    let keys = [
        format!("craft.station.{suffix}"),
        "craft.confirmPinned".to_string(),
        "craft.returnToStationGrid".to_string(),
        format!("craft.recipe.{suffix}"),
        "game.stationGrid".to_string(),
        "craft.fill".to_string(),
        "craft.purchase".to_string(),
        "craft.produce".to_string(),
        "craft.abort".to_string(),
    ];
    for key in keys {
        let target = environment
            .targets
            .iter()
            .find(|target| target.key == key)
            .ok_or_else(|| format!("制作校准目标 {key} 不存在"))?;
        let recipe_override = (key == format!("craft.recipe.{suffix}"))
            .then(|| {
                business_config
                    .recipe_points
                    .iter()
                    .find(|point| point.kind == station_kind)
                    .map(|point| &point.rect)
            })
            .flatten();
        let rect = recipe_override
            .or(target.rect.as_ref())
            .ok_or_else(|| format!("制作校准目标 {key} 未框选"))?;
        let region = crate::morse::types::RegionRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let template = if target.kind == CalibrationTargetKind::ClickPoint {
            None
        } else {
            let (reference, threshold) = resolved_template_config(environment, target)
                .map_err(|error| format!("制作校准目标 {key} 无效：{error}"))?;
            Some(template_observer::RuntimeTemplate {
                key: target.key.clone(),
                region: region.clone(),
                reference_image_path: std::fs::canonicalize(reference)
                    .map_err(|_| "制作参考图不存在".to_string())?,
                threshold,
            })
        };
        targets.insert(
            key.clone(),
            template_observer::RuntimeTarget {
                key,
                region,
                template,
                guard_any_of: target.guard_any_of.clone(),
            },
        );
    }
    Ok((
        craft_runtime::CraftRunConfig {
            game_executable_path: std::fs::canonicalize(&settings.game_executable_path)
                .map_err(|_| "游戏 exe 路径无效".to_string())?,
            mouse_parking_region: mouse_parking_region(settings)?,
            targets,
            delays: craft_runtime::CraftProbeDelays {
                space_ms: settings.craft_space_delay_ms,
                reopen_ms: settings.craft_reopen_delay_ms,
                confirm_pinned_ms: settings.craft_confirm_pinned_delay_ms,
            },
        },
        station.duration_minutes,
    ))
}

struct FrozenCraftBatchTask {
    task: craft_batch::CraftBatchTask,
    config: craft_runtime::CraftRunConfig,
}

#[derive(Debug)]
struct FrozenAmmoRun {
    game_executable_path: std::path::PathBuf,
    mouse_parking_region: crate::morse::types::RegionRect,
    targets: std::collections::HashMap<String, template_observer::RuntimeTarget>,
    ammo_targets: Vec<ammo_runtime::AmmoRunTarget>,
    day: String,
}

fn freeze_ammo_run(
    settings: &SpecialOpsSettings,
    account_id: &str,
    frozen_now_ms: i64,
    requested_target_ids: Option<&[String]>,
) -> Result<FrozenAmmoRun, String> {
    let account = settings
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "子弹兑换试运行账号不存在".to_string())?;
    if !account.enabled {
        return Err("子弹兑换试运行账号未启用".to_string());
    }
    if account.status != AccountStatus::Ready {
        return Err("当前账号状态不是 Ready，禁止启动子弹兑换试运行".to_string());
    }
    let business_config = resolve_account_business_config(settings, account)?;
    let mut business_targets = business_config
        .ammo_targets
        .iter()
        .filter(|target| target.enabled)
        .filter(|target| {
            requested_target_ids.is_none_or(|ids| ids.iter().any(|id| id == &target.id))
        })
        .collect::<Vec<_>>();
    if business_targets.is_empty() {
        return Err("当前账号没有启用的子弹兑换目标".to_string());
    }
    business_targets.sort_by_key(|target| (target.seasonal, target.order));

    let day = local_day_and_minute(frozen_now_ms).0;
    let ammo_targets = business_targets
        .iter()
        .map(|target| {
            let point = target
                .click_point
                .as_ref()
                .ok_or_else(|| format!("子弹目标 {} 尚未配置点击点", target.note))?;
            if point.width != 1 || point.height != 1 {
                return Err(format!("子弹目标 {} 点击点无效", target.note));
            }
            let runtime_state = account
                .ammo_targets
                .iter()
                .find(|item| item.id == target.id);
            Ok(ammo_runtime::AmmoRunTarget {
                id: target.id.clone(),
                note: target.note.clone(),
                seasonal: target.seasonal,
                click_point: crate::morse::types::RegionRect {
                    x: point.x,
                    y: point.y,
                    width: point.width,
                    height: point.height,
                },
                scroll_steps: target.scroll_steps,
                already_succeeded: runtime_state.and_then(|item| item.last_success_day.as_deref())
                    == Some(day.as_str()),
                retry_count: runtime_state
                    .filter(|item| item.retry_day.as_deref() == Some(day.as_str()))
                    .map_or(0, |item| item.retry_count),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let environment = settings
        .calibration_environments
        .first()
        .ok_or_else(|| "子弹兑换校准环境不存在".to_string())?;
    let mut keys = vec![
        "ammo.tacticalDepartment",
        "ammo.fill",
        "ammo.purchase",
        "ammo.exchange",
        "ammo.confirm",
        "ammo.success",
    ];
    if ammo_targets.iter().any(|target| target.seasonal) {
        keys.push("ammo.seasonal");
    }
    let mut targets = std::collections::HashMap::new();
    for key in keys {
        let target = environment
            .targets
            .iter()
            .find(|target| target.key == key)
            .ok_or_else(|| format!("子弹兑换校准目标 {key} 不存在"))?;
        let rect = target
            .rect
            .as_ref()
            .ok_or_else(|| format!("子弹兑换校准目标 {key} 未框选"))?;
        let region = crate::morse::types::RegionRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let template = match target.kind {
            CalibrationTargetKind::RecognitionRegion => {
                if !verification_is_current(target) {
                    return Err(format!("子弹兑换校准目标 {key} 尚未测试或验证失效"));
                }
                let (reference, threshold) = resolved_template_config(environment, target)
                    .map_err(|error| format!("子弹兑换校准目标 {key} 无效：{error}"))?;
                Some(template_observer::RuntimeTemplate {
                    key: key.to_string(),
                    region: region.clone(),
                    reference_image_path: std::fs::canonicalize(reference)
                        .map_err(|_| format!("子弹兑换校准目标 {key} 的参考图不存在"))?,
                    threshold,
                })
            }
            CalibrationTargetKind::ClickPoint => None,
            CalibrationTargetKind::InputRegion => {
                return Err(format!("子弹兑换校准目标 {key} 类型无效"));
            }
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

    Ok(FrozenAmmoRun {
        game_executable_path: std::fs::canonicalize(&settings.game_executable_path)
            .map_err(|_| "游戏 exe 路径无效".to_string())?,
        mouse_parking_region: mouse_parking_region(settings)?,
        targets,
        ammo_targets,
        day,
    })
}

struct FrozenMilitarySupplyEntry {
    game_executable_path: std::path::PathBuf,
    mouse_parking_region: crate::morse::types::RegionRect,
    targets: std::collections::HashMap<String, template_observer::RuntimeTarget>,
    config: military_supply_runtime::MilitarySupplyEntryConfig,
}

fn freeze_military_supply_entry(
    settings: &SpecialOpsSettings,
) -> Result<FrozenMilitarySupplyEntry, String> {
    Ok(FrozenMilitarySupplyEntry {
        game_executable_path: std::fs::canonicalize(&settings.game_executable_path)
            .map_err(|_| "游戏 exe 路径无效".to_string())?,
        mouse_parking_region: mouse_parking_region(settings)?,
        targets: freeze_runtime_targets(
            settings,
            "军需处入口",
            &[
                ("ammo.department", true),
                ("ammo.supply", false),
                ("ammo.enterSupply", false),
            ],
        )?,
        config: military_supply_runtime::MilitarySupplyEntryConfig {
            supply_delay: std::time::Duration::from_millis(u64::from(
                settings.ammo_supply_delay_ms,
            )),
            enter_supply_delay: std::time::Duration::from_millis(u64::from(
                settings.ammo_tactical_delay_ms,
            )),
        },
    })
}

fn freeze_runtime_targets(
    settings: &SpecialOpsSettings,
    context: &str,
    targets: &[(&str, bool)],
) -> Result<std::collections::HashMap<String, template_observer::RuntimeTarget>, String> {
    let environment = settings
        .calibration_environments
        .first()
        .ok_or_else(|| format!("{context}校准环境不存在"))?;
    targets
        .iter()
        .map(|(key, template_required)| {
            let target = environment
                .targets
                .iter()
                .find(|target| target.key == *key)
                .ok_or_else(|| format!("{context}校准目标 {key} 不存在"))?;
            let rect = target
                .rect
                .as_ref()
                .ok_or_else(|| format!("{context}校准目标 {key} 未框选"))?;
            let region = crate::morse::types::RegionRect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            };
            let template = if *template_required {
                if !verification_is_current(target) {
                    return Err(format!("{context}校准目标 {key} 尚未测试或验证失效"));
                }
                let (reference, threshold) = resolved_template_config(environment, target)
                    .map_err(|error| format!("{context}校准目标 {key} 无效：{error}"))?;
                Some(template_observer::RuntimeTemplate {
                    key: (*key).to_string(),
                    region: region.clone(),
                    reference_image_path: std::fs::canonicalize(reference)
                        .map_err(|_| format!("{context}校准目标 {key} 的参考图不存在"))?,
                    threshold,
                })
            } else {
                None
            };
            Ok((
                (*key).to_string(),
                template_observer::RuntimeTarget {
                    key: (*key).to_string(),
                    region,
                    template,
                    guard_any_of: target.guard_any_of.clone(),
                },
            ))
        })
        .collect()
}

struct FrozenLimitedSupplyRun {
    game_executable_path: std::path::PathBuf,
    mouse_parking_region: crate::morse::types::RegionRect,
    targets: std::collections::HashMap<String, template_observer::RuntimeTarget>,
    cycle_id: String,
    config: limited_supply_runtime::LimitedRunConfig,
    colors: [[u8; 3]; 2],
    color_tolerances: [u8; 2],
}

fn freeze_limited_supply_run(
    settings: &SpecialOpsSettings,
    account_id: &str,
    cycle_id: &str,
) -> Result<FrozenLimitedSupplyRun, String> {
    if !settings
        .accounts
        .iter()
        .any(|account| account.id == account_id)
    {
        return Err("限时商品账号不存在".to_string());
    }
    let mut keys = vec![("ammo.researchDepartment", true), ("limited.ready", true)];
    for key in [
        "limited.color.1",
        "limited.color.2",
        "limited.color.3",
        "limited.color.4",
        "limited.color.5",
        "limited.color.6",
        "limited.color.7",
        "limited.color.8",
        "limited.color.9",
    ] {
        keys.push((key, false));
    }
    Ok(FrozenLimitedSupplyRun {
        game_executable_path: std::fs::canonicalize(&settings.game_executable_path)
            .map_err(|_| "游戏 exe 路径无效".to_string())?,
        mouse_parking_region: mouse_parking_region(settings)?,
        targets: freeze_runtime_targets(settings, "限时商品", &keys)?,
        cycle_id: cycle_id.to_string(),
        config: limited_supply_runtime::LimitedRunConfig {
            ready_timeout: std::time::Duration::from_millis(u64::from(
                settings.limited_supply.ready_timeout_ms,
            )),
            sample_interval: std::time::Duration::from_millis(400),
        },
        colors: settings.limited_supply.colors,
        color_tolerances: settings.limited_supply.color_tolerances,
    })
}

struct FrozenMarketRun {
    game_executable_path: std::path::PathBuf,
    mouse_parking_region: crate::morse::types::RegionRect,
    targets: std::collections::HashMap<String, template_observer::RuntimeTarget>,
    day: String,
    config: market_runtime::MarketRunConfig,
}

fn freeze_market_run(
    settings: &SpecialOpsSettings,
    account_id: &str,
    day: &str,
) -> Result<FrozenMarketRun, String> {
    let account = settings
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "交易行账号不存在".to_string())?;
    let market = &resolve_account_business_config(settings, account)?.market;
    let product = market
        .product_point
        .as_ref()
        .ok_or_else(|| "交易行商品入口点击点未配置".to_string())?;
    let completed_count = if account.market.day.as_deref() == Some(day) {
        account.market.completed_count
    } else {
        0
    };
    Ok(FrozenMarketRun {
        game_executable_path: std::fs::canonicalize(&settings.game_executable_path)
            .map_err(|_| "游戏 exe 路径无效".to_string())?,
        mouse_parking_region: mouse_parking_region(settings)?,
        targets: freeze_runtime_targets(
            settings,
            "交易行",
            &[
                ("market.entry", true),
                ("market.price", false),
                ("market.return", false),
                ("market.buy", false),
                ("market.confirm", false),
            ],
        )?,
        day: day.to_string(),
        config: market_runtime::MarketRunConfig {
            product_point: crate::morse::types::RegionRect {
                x: product.x,
                y: product.y,
                width: product.width,
                height: product.height,
            },
            max_price: market.max_price,
            target_count: market.purchase_count,
            completed_count,
            entry_delay: std::time::Duration::from_millis(u64::from(
                settings.market_purchase.entry_delay_ms,
            )),
            ocr_interval: std::time::Duration::from_millis(500),
        },
    })
}

struct FrozenRoundAccount {
    login: Arc<login_flow::LoginRunConfig>,
    navigation: Arc<game_navigation::NavigationRunConfig>,
    craft: Arc<Vec<FrozenCraftBatchTask>>,
    military_supply_entry: Option<Arc<FrozenMilitarySupplyEntry>>,
    ammo: Option<Arc<FrozenAmmoRun>>,
    limited_supply: Option<Arc<FrozenLimitedSupplyRun>>,
    market: Option<Arc<FrozenMarketRun>>,
}

struct FrozenRoundRun {
    plan: round_planner::RoundPlan,
    accounts: std::collections::HashMap<(String, i64), Result<Arc<FrozenRoundAccount>, String>>,
    game_executable_path: std::path::PathBuf,
    profit_generation: Option<u64>,
}

fn freeze_round_run(
    settings: &SpecialOpsSettings,
    frozen_now_ms: i64,
    trigger: round_planner::RoundTrigger,
    gate: AmmoProfitGate,
    profit_generation: Option<u64>,
) -> Result<FrozenRoundRun, String> {
    validate_execution_ready(settings)?;
    let game_executable_path = std::fs::canonicalize(&settings.game_executable_path)
        .map_err(|_| "游戏 exe 路径无效".to_string())?;
    let plan = round_planner::build_round_plan_with_profit(settings, frozen_now_ms, trigger, gate)?;
    if plan.accounts.is_empty() {
        return Err("当前没有到期特勤处任务".to_string());
    }
    let accounts = plan
        .accounts
        .iter()
        .map(|task| {
            let frozen = (|| {
                let (login, _login_signature) =
                    freeze_login_run_config(settings, &task.account_id)?;
                let destination = if task.stations.is_empty() {
                    game_navigation::NavigationDestination::Lobby
                } else {
                    game_navigation::NavigationDestination::StationGrid
                };
                let navigation =
                    freeze_navigation_run_config(settings, &task.account_id, destination)?;
                let craft = if task.stations.is_empty() {
                    Vec::new()
                } else {
                    // 到期桶的 scheduled_at_ms 是桶内最早完成时间，直接用它过滤会丢掉同桶里
                    // 完成更晚但同样已到期的制作台；未来桶要保留自身计划时间才能通过过滤。
                    freeze_craft_batch_run_configs(
                        settings,
                        &task.account_id,
                        task.scheduled_at_ms.max(frozen_now_ms),
                        Some(&task.stations),
                    )?
                };
                let ammo = (!task.ammo_target_ids.is_empty())
                    .then(|| {
                        freeze_ammo_run(
                            settings,
                            &task.account_id,
                            frozen_now_ms,
                            Some(&task.ammo_target_ids),
                        )
                    })
                    .transpose()?
                    .map(Arc::new);
                let limited_supply = task
                    .limited_supply_cycle_id
                    .as_deref()
                    .map(|cycle_id| freeze_limited_supply_run(settings, &task.account_id, cycle_id))
                    .transpose()?
                    .map(Arc::new);
                let military_supply_entry = (ammo.is_some() || limited_supply.is_some())
                    .then(|| freeze_military_supply_entry(settings))
                    .transpose()?
                    .map(Arc::new);
                let market = task
                    .market_purchase_day
                    .as_deref()
                    .map(|day| freeze_market_run(settings, &task.account_id, day))
                    .transpose()?
                    .map(Arc::new);
                Ok(Arc::new(FrozenRoundAccount {
                    login: Arc::new(login),
                    navigation: Arc::new(navigation),
                    craft: Arc::new(craft),
                    military_supply_entry,
                    ammo,
                    limited_supply,
                    market,
                }))
            })();
            ((task.account_id.clone(), task.scheduled_at_ms), frozen)
        })
        .collect();
    Ok(FrozenRoundRun {
        plan,
        accounts,
        game_executable_path,
        profit_generation,
    })
}

fn freeze_craft_batch_run_configs(
    settings: &SpecialOpsSettings,
    account_id: &str,
    frozen_now_ms: i64,
    requested_stations: Option<&[StationKind]>,
) -> Result<Vec<FrozenCraftBatchTask>, String> {
    let account = settings
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "制作批处理账号不存在".to_string())?;
    let business_config = resolve_account_business_config(settings, account)?;
    let tasks = craft_batch::select_due_craft_tasks(account, business_config, frozen_now_ms)?
        .into_iter()
        .filter(|task| {
            requested_stations
                .is_none_or(|stations| stations.iter().any(|station| station == &task.station))
        })
        .collect::<Vec<_>>();
    if tasks.is_empty() {
        return Err(if requested_stations.is_some() {
            "当前账号没有计划内到期制作台"
        } else {
            "当前账号没有到期制作台"
        }
        .to_string());
    }

    tasks
        .into_iter()
        .map(|task| {
            let (config, _) = freeze_craft_run_config(settings, account_id, task.station.clone())?;
            Ok(FrozenCraftBatchTask { task, config })
        })
        .collect()
}

#[derive(Debug, Clone)]
struct CalibrationTemplateTestInput {
    region: crate::morse::types::RegionRect,
    reference_image_path: String,
    match_threshold: f32,
    calibration_signature: String,
}

#[derive(Debug, Clone)]
enum CalibrationTestInput {
    Template(CalibrationTemplateTestInput),
    Ocr {
        region: crate::morse::types::RegionRect,
    },
}

fn calibration_test_requires_game_context(target_key: &str) -> bool {
    ["game.", "craft.", "ammo.", "market."]
        .iter()
        .any(|prefix| target_key.starts_with(prefix))
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
    let environment_index = next
        .calibration_environments
        .iter()
        .position(|environment| environment.id == environment_id)
        .ok_or_else(|| "校准配置已变化，请重新测试".to_string())?;
    let target_index = next.calibration_environments[environment_index]
        .targets
        .iter()
        .position(|target| target.key == target_key)
        .ok_or_else(|| "校准配置已变化，请重新测试".to_string())?;
    let current_signature = resolved_calibration_signature(
        &next.calibration_environments[environment_index],
        &next.calibration_environments[environment_index].targets[target_index],
    )
    .map_err(|_| "校准配置已变化，请重新测试".to_string())?;
    if current_signature != tested_signature {
        return Err("校准配置已变化，请重新测试".to_string());
    }
    let target = &mut next.calibration_environments[environment_index].targets[target_index];
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

fn calibration_test_input(
    settings: &SpecialOpsSettings,
    environment_id: &str,
    target_key: &str,
) -> Result<CalibrationTestInput, String> {
    let environment = settings
        .calibration_environments
        .iter()
        .find(|environment| environment.id == environment_id)
        .ok_or_else(|| "校准目标不存在".to_string())?;
    let target = environment
        .targets
        .iter()
        .find(|target| target.key == target_key)
        .ok_or_else(|| "校准目标不存在".to_string())?;
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
    match target.recognition_method {
        Some(CalibrationRecognitionMethod::Template) => {
            let (reference_image_path, match_threshold) =
                resolved_template_config(environment, target)?;
            Ok(CalibrationTestInput::Template(
                CalibrationTemplateTestInput {
                    region,
                    reference_image_path: reference_image_path.to_string(),
                    match_threshold,
                    calibration_signature: resolved_calibration_signature(environment, target)?,
                },
            ))
        }
        Some(CalibrationRecognitionMethod::Ocr) => Ok(CalibrationTestInput::Ocr { region }),
        None => Err("点击点和输入区域不支持测试".to_string()),
    }
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

async fn sample_numeric_ocr(
    region: crate::morse::types::RegionRect,
) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || {
        let captured = crate::recognition::watcher::capture_region(&region)
            .ok_or_else(|| "截取 OCR 校准区域失败".to_string())?;
        Ok(windows_ocr::recognize_numeric_words(captured)?
            .into_iter()
            .map(|word| word.text)
            .collect())
    })
    .await
    .map_err(|error| format!("OCR 校准测试任务失败: {error}"))?
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
    crate::settings::save_settings(&path, settings)?;
    if !crate::profile::is_applying(app) {
        crate::profile::update_active_profile_snapshot(
            app,
            crate::profile::ActiveProfileSnapshotPatch::SpecialOps(Box::new(settings.clone())),
        )?;
    }
    Ok(())
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
        profit_runtime: ProfitRuntimeSnapshot::default(),
    }
}

fn build_bootstrap_with_runtime(
    settings: SpecialOpsSettings,
    settings_revision: u64,
    current_ms: i64,
    runtime: &login_runtime::LoginRuntime,
    profit_runtime: &ProfitQueryControl,
) -> Result<SpecialOpsBootstrap, String> {
    let mut bootstrap = build_bootstrap(settings.clone(), settings_revision, current_ms);
    let (snapshot, cutoff_day) = match build_profit_query_window(
        &settings,
        current_ms,
        settings_revision,
        runtime
            .snapshot()?
            .is_some_and(|snapshot| snapshot.run_kind == LoginRunKind::Round),
    ) {
        Ok(window) => {
            let cutoff_day = (current_ms >= window.cutoff_at_ms).then(|| window.day.clone());
            (profit_runtime.sync_window(window)?, cutoff_day)
        }
        Err(error) => {
            let mut snapshot = profit_runtime.snapshot()?;
            snapshot.configuration_error = Some(error);
            (snapshot, None)
        }
    };
    let gate = if !settings.profit_filter.enabled {
        AmmoProfitGate::Disabled
    } else if let Some(day) = cutoff_day {
        AmmoProfitGate::QualifiedTargets(cutoff_qualified_targets(&settings, &day))
    } else {
        AmmoProfitGate::Qualified(snapshot.qualified_rule_ids.iter().cloned().collect())
    };
    bootstrap.schedule =
        build_schedule_with_profit_runtime(&settings, current_ms, &gate, Some(&snapshot));
    bootstrap.run_snapshot = runtime.snapshot()?;
    bootstrap.profit_runtime = snapshot;
    Ok(bootstrap)
}

fn emit_state(app: &AppHandle, settings_revision: u64, current_ms: i64) {
    let _ = app.emit_to(
        "main",
        STATE_CHANGED,
        SpecialOpsStateChanged {
            settings_revision,
            now_ms: current_ms,
        },
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
    let _state = app
        .try_state::<SpecialOpsState>()
        .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
    let coordinator = app
        .try_state::<Arc<SettingsCoordinator>>()
        .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
    emit_state(app, coordinator.current_revision()?, now_ms());
    Ok(())
}

fn wait_for_operation_window_ready(
    ready: std::sync::mpsc::Receiver<()>,
    timeout: std::time::Duration,
) -> Result<(), String> {
    match ready.recv_timeout(timeout) {
        Ok(()) => Ok(()),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err(OPERATION_WINDOW_LOAD_TIMEOUT.to_string())
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err("操作提示窗口加载失败，已取消本次试运行".to_string())
        }
    }
}

fn create_operation_window(
    app: &AppHandle,
    emergency_hotkey: &str,
    run_kind: LoginRunKind,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(login_runtime::OPERATION_WINDOW_LABEL) {
        let show_result = window.show();
        show_result.map_err(|error| format!("显示操作提示窗口失败: {error}"))?;
        return Ok(());
    }
    let hotkey = encoded_query_value(emergency_hotkey);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let window = tauri::WebviewWindowBuilder::new(
        app,
        login_runtime::OPERATION_WINDOW_LABEL,
        tauri::WebviewUrl::App(
            format!(
                "index.html?mode=special-ops-operation&emergencyHotkey={hotkey}&runKind={}",
                run_kind.query_value()
            )
            .into(),
        ),
    )
    .title("特勤处操作中")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(false)
    .resizable(false)
    .inner_size(420.0, 180.0)
    .on_page_load(move |_, payload| {
        if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
            let _ = ready_tx.try_send(());
        }
    })
    .build()
    .map_err(|error| format!("创建登录试运行窗口失败: {error}"))?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| format!("设置登录试运行窗口点击穿透失败: {error}"))?;
    window
        .show()
        .map_err(|error| format!("显示操作提示窗口失败: {error}"))?;
    wait_for_operation_window_ready(ready_rx, std::time::Duration::from_secs(3))?;
    Ok(())
}

fn register_emergency_hotkey(app: &AppHandle, hotkey: String) -> Result<(), String> {
    let manager = app
        .try_state::<crate::hotkeys::HotkeyManager>()
        .ok_or_else(|| "热键管理器尚未初始化".to_string())?;
    let action: crate::hotkey_types::HotkeyAction = Arc::new(|app| {
        if let Err(error) = request_emergency_stop_core(&app) {
            crate::log_error!(
                "special_ops::runtime",
                "紧急停止登记失败",
                "error" => error
            );
        }
    });
    manager.replace_safety_scope(
        LOGIN_HOTKEY_SCOPE,
        vec![(hotkey, action)],
        "特勤处紧急停止".to_string(),
        crate::hotkey_types::ConflictPolicy::Strict,
    )
}

fn request_then_release_emergency<T>(
    request: impl FnOnce() -> Result<T, String>,
    release: impl FnOnce() -> Result<(), String>,
) -> Result<T, String> {
    let value = request()?;
    release()?;
    Ok(value)
}

fn ensure_global_automation_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        Ok(())
    } else {
        Err("全局总开关已关闭".to_string())
    }
}

fn ensure_app_global_automation_enabled(app: &AppHandle) -> Result<(), String> {
    ensure_global_automation_enabled(
        app.try_state::<crate::global_state::GlobalState>()
            .map(|state| state.enabled())
            .unwrap_or(true),
    )
}

fn clear_login_hotkey(app: &AppHandle) -> Result<(), String> {
    if let Some(manager) = app.try_state::<crate::hotkeys::HotkeyManager>() {
        manager.clear_scope(LOGIN_HOTKEY_SCOPE)?;
    }
    Ok(())
}

fn hide_operation_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(login_runtime::OPERATION_WINDOW_LABEL) {
        window
            .hide()
            .map_err(|error| format!("隐藏操作提示窗口失败: {error}"))?;
    }
    Ok(())
}

fn release_login_resources_with(
    release_inputs: impl FnOnce() -> Result<(), String>,
    clear_hotkey: impl FnOnce() -> Result<(), String>,
    destroy_window: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let mut errors = Vec::new();
    if let Err(error) = release_inputs() {
        errors.push(error);
    }
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
    let cleanup = release_login_resources_with(
        || {
            crate::log_debug!("special_ops::cleanup", "输入释放开始");
            let result = crate::input_simulation::release_tracked_injected_inputs();
            crate::log_debug!(
                "special_ops::cleanup",
                "输入释放结束",
                "success" => result.is_ok()
            );
            result
        },
        || {
            crate::log_debug!("special_ops::cleanup", "紧急热键清理开始");
            let result = clear_login_hotkey(app);
            crate::log_debug!(
                "special_ops::cleanup",
                "紧急热键清理结束",
                "success" => result.is_ok()
            );
            result
        },
        || {
            crate::log_debug!("special_ops::cleanup", "操作窗口隐藏开始");
            let result = hide_operation_window(app);
            crate::log_debug!(
                "special_ops::cleanup",
                "操作窗口隐藏结束",
                "success" => result.is_ok()
            );
            result
        },
    );
    let mut errors = Vec::new();
    if let Err(error) = cleanup {
        errors.push(error);
    }
    crate::log_debug!("special_ops::cleanup", "其他窗口恢复开始");
    let restore_result = restore_other_windows_after_special_ops(app);
    crate::log_debug!(
        "special_ops::cleanup",
        "其他窗口恢复结束",
        "success" => restore_result.is_ok()
    );
    if let Err(error) = restore_result {
        errors.push(error);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
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

// 该函数统一编排六类资源回调，保持启动、回滚和清理顺序可见。
#[allow(clippy::too_many_arguments)]
fn start_login_run_with_resources<R, H, C, A, L, S>(
    runtime: &login_runtime::LoginRuntime,
    account_id: String,
    run_kind: LoginRunKind,
    register_hotkey: R,
    hide_windows: H,
    create_window: C,
    announce_start: A,
    cleanup: L,
    spawn_worker: S,
) -> Result<(login_runtime::StartedLoginRun, LoginRunSnapshot), String>
where
    R: FnOnce() -> Result<(), String>,
    H: FnOnce() -> Result<(), String>,
    C: FnOnce() -> Result<(), String>,
    A: FnOnce(&LoginRunSnapshot),
    L: FnOnce() -> Result<(), String>,
    S: FnOnce(&login_runtime::StartedLoginRun) -> Result<(), String>,
{
    crate::log_info!(
        "special_ops::startup",
        "等待登录资源锁",
        "run_kind" => format!("{run_kind:?}")
    );
    let _resources = LOGIN_RESOURCE_CLEANUP_LOCK
        .lock()
        .map_err(|_| "登录试运行资源清理锁已损坏".to_string())?;
    crate::log_info!(
        "special_ops::startup",
        "登录资源锁已获取",
        "run_kind" => format!("{run_kind:?}")
    );
    let started = runtime.try_start_kind(account_id, run_kind)?;
    let snapshot = runtime
        .snapshot()?
        .ok_or_else(|| "登录试运行启动状态丢失".to_string())?;

    crate::log_info!("special_ops::startup", "开始隐藏其他窗口", "run_id" => started.run_id);
    if let Err(error) = hide_windows() {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            error,
            cleanup,
        ));
    }
    crate::log_info!("special_ops::startup", "隐藏其他窗口完成", "run_id" => started.run_id);

    if !runtime.can_continue_start(started.run_id)? {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            "登录试运行启动期间已停止".to_string(),
            cleanup,
        ));
    }
    crate::log_info!("special_ops::startup", "开始注册紧急热键", "run_id" => started.run_id);
    if let Err(error) = register_hotkey() {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            error,
            cleanup,
        ));
    }
    crate::log_info!("special_ops::startup", "注册紧急热键完成", "run_id" => started.run_id);
    if !runtime.can_continue_start(started.run_id)? {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            "登录试运行启动期间已停止".to_string(),
            cleanup,
        ));
    }
    crate::log_info!("special_ops::startup", "开始创建操作提示窗", "run_id" => started.run_id);
    if let Err(error) = create_window() {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            error,
            cleanup,
        ));
    }
    crate::log_info!("special_ops::startup", "创建操作提示窗完成", "run_id" => started.run_id);
    crate::log_info!("special_ops::startup", "开始登记 worker handoff", "run_id" => started.run_id);
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
    crate::log_info!("special_ops::startup", "worker handoff 登记完成", "run_id" => started.run_id);
    crate::log_info!("special_ops::startup", "开始提交 worker", "run_id" => started.run_id);
    if let Err(error) = spawn_worker(&started) {
        return Err(rollback_login_start_unlocked(
            runtime,
            started.run_id,
            error,
            cleanup,
        ));
    }
    crate::log_info!("special_ops::startup", "worker 提交完成", "run_id" => started.run_id);
    let snapshot = runtime
        .snapshot()?
        .filter(|snapshot| snapshot.run_id == started.run_id)
        .ok_or_else(|| "登录试运行启动状态已变化".to_string())?;
    crate::log_info!("special_ops::startup", "登录资源启动完成", "run_id" => started.run_id);
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
    if !runtime.cleanup_ready(run_id)? {
        return Ok(());
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
    let (_settings, revision) = coordinator.with_runtime_change(|| {
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
    emit_state(app, revision, now_ms());
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
                crate::log_debug!(
                    "special_ops::persistence",
                    "运行结果持久化开始",
                    "run_id" => run_id,
                    "kind" => format!("{kind:?}")
                );
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
                    crate::log_debug!(
                        "special_ops::persistence",
                        "运行结果持久化完成",
                        "run_id" => run_id,
                        "kind" => format!("{kind:?}")
                    );
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
        InitialCountdown => "即将开始轮换操作",
        StopGame => "正在结束旧游戏进程",
        StopWeGame => "正在结束旧 WeGame 进程",
        StartWeGame => "正在启动 WeGame",
        WaitLoginChoice => "正在识别登录入口",
        OpenLoginForm => "正在打开账号密码登录",
        OpenAccountList => "正在展开已记住账号列表",
        ScanRememberedAccounts => "正在扫描已记住账号",
        SelectRememberedAccount => "正在选择目标 QQ",
        VerifySelectedAccount => "正在复制并复核目标 QQ",
        SubmitLogin => "正在提交登录",
        WaitGameEntry => "正在等待游戏入口",
        OpenGameEntry => "正在打开游戏入口",
        WaitLaunchButton => "正在等待启动按钮",
        LaunchGame => "正在启动游戏",
        WaitGameWindow => "正在等待游戏窗口",
        WaitModeReady => "正在等待模式选择可用",
        OpenBeaconMode => "正在进入烽火地带",
        DismissActivityPopup => "正在关闭活动弹窗",
        SwitchLobbyView => "正在切换大厅视角",
        OpenSpecialOps => "正在进入特勤处",
        WaitStationGrid => "正在等待四制作台页面",
    }
}

fn cleanup_login_worker_after_persistence<T>(
    persist_result: &Result<T, String>,
    cleanup: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match (persist_result, cleanup()) {
        (Ok(_), cleanup_result) => cleanup_result,
        (Err(error), Ok(())) => Err(error.clone()),
        (Err(error), Err(cleanup_error)) => Err(format!("{error}; {cleanup_error}")),
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
    crate::log_debug!(
        "special_ops::login",
        "登录 worker 开始",
        "run_id" => run_id
    );
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
                Some(step),
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
            login_flow::LoginFlowResult::NeedsManualLogin { .. } => (
                LoginRunStatus::Failed,
                "未找到或无法复核目标 QQ，需要人工登录",
            ),
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
    crate::log_debug!(
        "special_ops::login",
        "登录 worker 结束",
        "run_id" => run_id
    );
}

/// 导航失败只暂停全局调度，不把账号误标成登录失败。
fn persist_navigation_pause(app: &AppHandle) -> Result<(), String> {
    let state = app
        .try_state::<SpecialOpsState>()
        .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
    let coordinator = app
        .try_state::<Arc<SettingsCoordinator>>()
        .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
    let (_settings, revision) = coordinator.with_runtime_change(|| {
        let mut next = state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        next.paused = true;
        save_settings(app, &next)?;
        *state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok::<_, String>(next)
    })?;
    state.profit_runtime.invalidate("导航失败，利润查询已取消");
    emit_state(app, revision, now_ms());
    Ok(())
}

/// 运行独立游戏内导航试运行，并复用登录试运行的资源生命周期。
async fn run_navigation_worker(
    app: AppHandle,
    runtime: Arc<login_runtime::LoginRuntime>,
    run_id: u64,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    config: Arc<game_navigation::NavigationRunConfig>,
) {
    crate::log_debug!(
        "special_ops::navigation",
        "导航 worker 开始",
        "run_id" => run_id
    );
    let driver = game_navigation::ProductionGameNavigationDriver::new(
        app.clone(),
        Arc::clone(&runtime),
        run_id,
        Arc::clone(&config),
    );
    let update_app = app.clone();
    let update_runtime = Arc::clone(&runtime);
    let update_cancelled = Arc::clone(&cancelled);
    let result = game_navigation::run_game_navigation(
        &driver,
        config.destination,
        config.delays,
        cancelled,
        move |step| {
            let step = login_flow::LoginStep::from(step);
            match update_runtime.update(
                run_id,
                LoginRunStatus::Waiting,
                Some(step),
                login_step_message(&step),
                None,
            ) {
                Ok(Some(snapshot)) => emit_run(&update_app, &snapshot),
                Ok(None) | Err(_) => {
                    update_cancelled.store(true, std::sync::atomic::Ordering::SeqCst)
                }
            }
        },
    )
    .await;
    let stop_reason = runtime.stop_reason(run_id).ok().flatten();
    let stopped = stop_reason.is_some();
    let persist_result = if let Some(reason) = stop_reason {
        runtime
            .snapshot()
            .and_then(|snapshot| {
                snapshot
                    .filter(|snapshot| snapshot.run_id == run_id)
                    .map(|snapshot| snapshot.account_id)
                    .ok_or_else(|| "导航运行状态已丢失".to_string())
            })
            .and_then(|account_id| {
                if let Some((outcome, _)) =
                    navigation_stop_outcome(Some(reason), &account_id, now_ms())
                {
                    persist_login_outcome(&app, &runtime, run_id, &account_id, &outcome, "")
                        .map(|_| ())
                } else {
                    Ok(())
                }
            })
    } else if matches!(
        result,
        game_navigation::GameNavigationResult::TimedOut { .. }
            | game_navigation::GameNavigationResult::Paused { .. }
    ) {
        persist_navigation_pause(&app)
    } else {
        Ok(())
    };
    let (status, message) = if stopped {
        (
            LoginRunStatus::Stopped,
            "游戏内导航试运行已停止".to_string(),
        )
    } else {
        match result {
            game_navigation::GameNavigationResult::Ready => (
                LoginRunStatus::Succeeded,
                "游戏内导航试运行成功".to_string(),
            ),
            game_navigation::GameNavigationResult::TimedOut { failed_step } => (
                LoginRunStatus::Failed,
                format!("游戏内导航超时（{failed_step:?}）"),
            ),
            game_navigation::GameNavigationResult::Paused {
                failed_step,
                message,
            } => (
                LoginRunStatus::Failed,
                format!("游戏内导航失败（{failed_step:?}）：{message}"),
            ),
            game_navigation::GameNavigationResult::EmergencyStopped => (
                LoginRunStatus::Stopped,
                "游戏内导航试运行已停止".to_string(),
            ),
        }
    };
    if let Err(error) = cleanup_login_worker_after_persistence(&persist_result, || {
        cleanup_login_run(&app, &runtime, run_id, status, &message)
    }) {
        crate::log_error!(
            "special_ops::navigation",
            "游戏内导航试运行持久化或清理失败",
            "error" => error
        );
    }
    crate::log_debug!(
        "special_ops::navigation",
        "导航 worker 结束",
        "run_id" => run_id
    );
}

fn navigation_stop_outcome(
    stop_reason: Option<login_runtime::StopReason>,
    account_id: &str,
    stopped_at: i64,
) -> Option<(login_flow::LoginFlowResult, login_runtime::StopReason)> {
    match stop_reason {
        Some(reason @ login_runtime::StopReason::Emergency)
        | Some(reason @ login_runtime::StopReason::Lifecycle { uncertain: true }) => Some((
            login_flow::LoginFlowResult::EmergencyStopped {
                account_id: account_id.to_string(),
                stopped_at,
            },
            reason,
        )),
        _ => None,
    }
}

/// 执行单制作台试运行，并在中止按钮双采样成功后保存下一次完成时间。
fn persist_craft_success_with<F>(
    settings: &Mutex<SpecialOpsSettings>,
    coordinator: &SettingsCoordinator,
    account_id: &str,
    station: &StationKind,
    started_at_ms: i64,
    duration_minutes: u32,
    persist: F,
) -> Result<(SpecialOpsSettings, u64), String>
where
    F: FnOnce(&SpecialOpsSettings) -> Result<(), String>,
{
    coordinator.with_runtime_change(|| {
        let mut next = settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        let account = next
            .accounts
            .iter_mut()
            .find(|account| account.id == account_id)
            .ok_or_else(|| "制作账号不存在".to_string())?;
        let station_plan = account
            .stations
            .iter_mut()
            .find(|candidate| candidate.kind == *station)
            .ok_or_else(|| "制作台不存在".to_string())?;
        station_plan.started_at_ms = Some(started_at_ms);
        station_plan.finishes_at_ms =
            Some(started_at_ms.saturating_add(i64::from(duration_minutes) * 60_000));
        station_plan.status = StationStatus::Crafting;
        persist(&next)?;
        *settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok(next)
    })
}

fn persist_craft_uncertain_with<F>(
    settings: &Mutex<SpecialOpsSettings>,
    coordinator: &SettingsCoordinator,
    account_id: &str,
    station: &StationKind,
    failed_at: i64,
    persist: F,
) -> Result<(SpecialOpsSettings, u64), String>
where
    F: FnOnce(&SpecialOpsSettings) -> Result<(), String>,
{
    coordinator.with_runtime_change(|| {
        let mut next = settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        mark_craft_cancel_uncertain(&mut next, account_id, station.clone(), failed_at)?;
        persist(&next)?;
        *settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok(next)
    })
}

#[allow(clippy::too_many_arguments)]
fn persist_craft_failure_uncertain_with<F>(
    settings: &Mutex<SpecialOpsSettings>,
    coordinator: &SettingsCoordinator,
    account_id: &str,
    station: &StationKind,
    failed_at: i64,
    step: &str,
    message: &str,
    persist: F,
) -> Result<(SpecialOpsSettings, u64), String>
where
    F: FnOnce(&SpecialOpsSettings) -> Result<(), String>,
{
    coordinator.with_runtime_change(|| {
        let mut next = settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        mark_craft_uncertain(
            &mut next,
            account_id,
            station.clone(),
            failed_at,
            step,
            message,
        )?;
        persist(&next)?;
        *settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok(next)
    })
}

#[allow(clippy::too_many_arguments)]
fn persist_craft_failure_isolated_with<F>(
    settings: &Mutex<SpecialOpsSettings>,
    coordinator: &SettingsCoordinator,
    account_id: &str,
    failed_at: i64,
    step: &str,
    message: &str,
    persist: F,
) -> Result<(SpecialOpsSettings, u64), String>
where
    F: FnOnce(&SpecialOpsSettings) -> Result<(), String>,
{
    coordinator.with_runtime_change(|| {
        let mut next = settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        let error = round_runner::AccountRunError::account(step, message);
        apply_round_account_failure(&mut next, account_id, &error, failed_at)?;
        persist(&next)?;
        *settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok(next)
    })
}

#[allow(clippy::too_many_arguments)]
fn persist_craft_stop_with<F>(
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
    settings: &Mutex<SpecialOpsSettings>,
    coordinator: &SettingsCoordinator,
    account_id: &str,
    station: &StationKind,
    failed_at: i64,
    entered_input: bool,
    mut persist: F,
) -> Result<Option<(SpecialOpsSettings, u64)>, String>
where
    F: FnMut(&SpecialOpsSettings) -> Result<(), String>,
{
    let stop_reason = runtime
        .stop_reason(run_id)?
        .ok_or_else(|| "制作试运行尚未收到停止请求".to_string())?;
    if stop_reason == login_runtime::StopReason::Normal {
        return if should_mark_craft_stop_uncertain(Some(stop_reason), entered_input) {
            persist_craft_uncertain_with(
                settings,
                coordinator,
                account_id,
                station,
                failed_at,
                |next| persist(next),
            )
            .map(Some)
        } else {
            Ok(None)
        };
    }

    let deadline = std::time::Instant::now()
        .checked_add(std::time::Duration::from_secs(5))
        .ok_or_else(|| "制作停止状态持久化等待期限无效".to_string())?;
    loop {
        match runtime.claim_persistence(run_id)? {
            login_runtime::PersistenceClaim::Pending => {
                let remaining = deadline
                    .checked_duration_since(std::time::Instant::now())
                    .ok_or_else(|| "等待制作停止状态持久化权限超时，保留当前试运行".to_string())?;
                runtime.wait_for_persistence_change(
                    run_id,
                    remaining.min(std::time::Duration::from_millis(50)),
                )?;
            }
            login_runtime::PersistenceClaim::Persisted
            | login_runtime::PersistenceClaim::NoPersistence => return Ok(None),
            login_runtime::PersistenceClaim::Stale => {
                return Err("制作停止状态持久化任务已过期".to_string());
            }
            login_runtime::PersistenceClaim::NoActive => {
                return Err("制作试运行已不存在，拒绝持久化停止状态".to_string());
            }
            login_runtime::PersistenceClaim::Acquired(guard) => {
                let login_runtime::PersistenceKind::Stop(reason) = guard.kind() else {
                    guard.fail("制作停止状态持久化类型错误")?;
                    return Err("制作停止状态持久化类型错误".to_string());
                };
                let updated = if should_mark_craft_stop_uncertain(Some(reason), entered_input) {
                    match persist_craft_uncertain_with(
                        settings,
                        coordinator,
                        account_id,
                        station,
                        failed_at,
                        |next| persist(next),
                    ) {
                        Ok(updated) => Some(updated),
                        Err(error) => {
                            guard.fail("制作停止状态保存失败")?;
                            return Err(error);
                        }
                    }
                } else {
                    None
                };
                if guard.complete()? {
                    return Ok(updated);
                }
            }
        }
    }
}

fn mark_ammo_uncertain(
    settings: &mut SpecialOpsSettings,
    account_id: &str,
    step: &str,
    message: &str,
    at_ms: i64,
) -> Result<(), String> {
    settings.paused = true;
    let account = settings
        .accounts
        .iter_mut()
        .find(|account| account.id == account_id)
        .ok_or_else(|| "子弹兑换账号已不存在".to_string())?;
    account.status = AccountStatus::Uncertain;
    account.last_failure = Some(AccountFailure {
        step: step.to_string(),
        message: message.to_string(),
        at_ms,
        station_kind: None,
        ammo_target_id: None,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn persist_ammo_uncertain_with<F>(
    settings: &Mutex<SpecialOpsSettings>,
    coordinator: &SettingsCoordinator,
    account_id: &str,
    target_id: Option<&str>,
    step: &str,
    message: &str,
    at_ms: i64,
    persist: F,
) -> Result<(SpecialOpsSettings, u64), String>
where
    F: FnOnce(&SpecialOpsSettings) -> Result<(), String>,
{
    coordinator.with_runtime_change(|| {
        let mut next = settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        next.paused = true;
        if let Some(target_id) = target_id {
            let account = next
                .accounts
                .iter_mut()
                .find(|account| account.id == account_id)
                .ok_or_else(|| "子弹兑换账号已不存在".to_string())?;
            set_ammo_manual_failure(account, target_id, step, message, at_ms)?;
        } else {
            mark_ammo_uncertain(&mut next, account_id, step, message, at_ms)?;
        }
        persist(&next)?;
        *settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok(next)
    })
}

fn ammo_stop_failure_detail(
    stop: &ammo_runtime::AmmoRunStop,
) -> Option<(Option<&str>, &str, String)> {
    match stop {
        ammo_runtime::AmmoRunStop::Uncertain {
            target_id,
            step,
            message,
        } => Some((Some(target_id), step, message.clone())),
        ammo_runtime::AmmoRunStop::SystemFailure { step, message } => {
            Some((None, step, message.clone()))
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn persist_ammo_stop_with<F>(
    runtime: &login_runtime::LoginRuntime,
    run_id: u64,
    settings: &Mutex<SpecialOpsSettings>,
    coordinator: &SettingsCoordinator,
    account_id: &str,
    at_ms: i64,
    entered_input: bool,
    mut persist: F,
) -> Result<Option<(SpecialOpsSettings, u64)>, String>
where
    F: FnMut(&SpecialOpsSettings) -> Result<(), String>,
{
    let stop_reason = runtime
        .stop_reason(run_id)?
        .ok_or_else(|| "子弹兑换试运行尚未收到停止请求".to_string())?;
    let should_mark = should_mark_craft_stop_uncertain(Some(stop_reason), entered_input);
    if stop_reason == login_runtime::StopReason::Normal {
        return if should_mark {
            persist_ammo_uncertain_with(
                settings,
                coordinator,
                account_id,
                None,
                "ammo.cancel",
                "子弹兑换取消时已执行键鼠输入，请人工确认当天兑换状态",
                at_ms,
                |next| persist(next),
            )
            .map(Some)
        } else {
            Ok(None)
        };
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match runtime.claim_persistence(run_id)? {
            login_runtime::PersistenceClaim::Pending => {
                let remaining = deadline
                    .checked_duration_since(std::time::Instant::now())
                    .ok_or_else(|| "等待子弹兑换停止状态持久化超时".to_string())?;
                runtime.wait_for_persistence_change(
                    run_id,
                    remaining.min(std::time::Duration::from_millis(50)),
                )?;
            }
            login_runtime::PersistenceClaim::Persisted
            | login_runtime::PersistenceClaim::NoPersistence => return Ok(None),
            login_runtime::PersistenceClaim::Stale => {
                return Err("子弹兑换停止状态持久化任务已过期".to_string());
            }
            login_runtime::PersistenceClaim::NoActive => {
                return Err("子弹兑换试运行已不存在".to_string());
            }
            login_runtime::PersistenceClaim::Acquired(guard) => {
                let login_runtime::PersistenceKind::Stop(reason) = guard.kind() else {
                    guard.fail("子弹兑换停止状态持久化类型错误")?;
                    return Err("子弹兑换停止状态持久化类型错误".to_string());
                };
                let updated = if should_mark_craft_stop_uncertain(Some(reason), entered_input) {
                    match persist_ammo_uncertain_with(
                        settings,
                        coordinator,
                        account_id,
                        None,
                        "ammo.emergencyStop",
                        "子弹兑换已紧急停止，账号状态需人工确认",
                        at_ms,
                        |next| persist(next),
                    ) {
                        Ok(updated) => Some(updated),
                        Err(error) => {
                            guard.fail("子弹兑换停止状态保存失败")?;
                            return Err(error);
                        }
                    }
                } else {
                    None
                };
                if guard.complete()? {
                    return Ok(updated);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CraftPersistenceDecision {
    NoChange,
    SaveStarted { started_at_ms: i64 },
    MarkUncertain { step: String, message: String },
    MarkIsolated { step: String, message: String },
    FailWithoutChange { step: String, message: String },
}

fn decide_craft_persistence(
    result: Result<craft_runtime::CraftStationOutcome, craft_trial::CraftTrialFailure>,
) -> CraftPersistenceDecision {
    match result {
        Ok(craft_runtime::CraftStationOutcome::StillInProgress) => {
            CraftPersistenceDecision::NoChange
        }
        Ok(craft_runtime::CraftStationOutcome::Started { started_at_ms }) => {
            CraftPersistenceDecision::SaveStarted { started_at_ms }
        }
        Err(error) if error.is_isolated() => CraftPersistenceDecision::MarkIsolated {
            step: error.step,
            message: error.message,
        },
        Err(error) if error.requires_uncertain => CraftPersistenceDecision::MarkUncertain {
            step: error.step,
            message: error.message,
        },
        Err(error) => CraftPersistenceDecision::FailWithoutChange {
            step: error.step,
            message: error.message,
        },
    }
}

fn batch_stop_context(failure: &craft_batch::CraftBatchFailure) -> (StationKind, bool) {
    (failure.station.clone(), failure.entered_input)
}

struct ProductionCraftBatchDriver {
    app: AppHandle,
    settings: Arc<Mutex<SpecialOpsSettings>>,
    coordinator: Arc<SettingsCoordinator>,
    runtime: Arc<login_runtime::LoginRuntime>,
    run_id: u64,
    account_id: String,
    frozen: Arc<Vec<FrozenCraftBatchTask>>,
    round_progress: Option<RoundCraftProgress>,
}

const AMMO_RESET_KEY_DELAY_MS: u64 = 100;
const AMMO_SCROLL_STEP_INTERVAL_MS: u64 = 100;
const AMMO_SCROLL_SETTLE_MS: u64 = 1_000;

fn ammo_reset_keys() -> [crate::hotkey_types::PrimaryKey; 2] {
    [
        crate::hotkey_types::PrimaryKey::Letter('A'),
        crate::hotkey_types::PrimaryKey::Letter('D'),
    ]
}

fn ammo_success_diagnostic_filename(timestamp_ms: i64, qq_account: &str) -> String {
    format!("{timestamp_ms}-{qq_account}-ammo.success.png")
}

fn ammo_scroll_segments(scroll_steps: u32) -> Vec<(i32, u32)> {
    if scroll_steps == 0 {
        Vec::new()
    } else {
        vec![(1, scroll_steps)]
    }
}

struct ProductionAmmoDriver {
    app: AppHandle,
    settings: Arc<Mutex<SpecialOpsSettings>>,
    coordinator: Arc<SettingsCoordinator>,
    runtime: Arc<login_runtime::LoginRuntime>,
    run_id: u64,
    account_id: String,
    day: String,
    game_executable_path: std::path::PathBuf,
    mouse_parking_region: crate::morse::types::RegionRect,
    targets: std::collections::HashMap<String, template_observer::RuntimeTarget>,
}

impl ProductionAmmoDriver {
    fn system_error(step: &str, message: impl Into<String>) -> ammo_runtime::AmmoDriverError {
        ammo_runtime::AmmoDriverError::System {
            step: step.to_string(),
            message: message.into(),
        }
    }

    fn cancelled_or_system(
        cancelled: &std::sync::atomic::AtomicBool,
        step: &str,
        message: String,
    ) -> ammo_runtime::AmmoDriverError {
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            ammo_runtime::AmmoDriverError::Cancelled
        } else {
            Self::system_error(step, message)
        }
    }

    fn emit_update(&self, status: LoginRunStatus, message: &str) -> Result<(), String> {
        if let Some(snapshot) = self
            .runtime
            .update(self.run_id, status, None, message, None)?
        {
            emit_run(&self.app, &snapshot);
        }
        Ok(())
    }

    async fn countdown(
        &self,
        cancelled: &Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), ammo_runtime::AmmoDriverError> {
        let Some(total) = self
            .runtime
            .next_input_countdown_seconds(self.run_id, true)
            .map_err(|error| Self::system_error("ammo.countdown", error))?
        else {
            return Ok(());
        };
        for seconds in (1..=total).rev() {
            if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(ammo_runtime::AmmoDriverError::Cancelled);
            }
            if let Some(snapshot) = self
                .runtime
                .update(
                    self.run_id,
                    LoginRunStatus::Countdown,
                    None,
                    format!("{seconds} 秒后执行键鼠操作"),
                    Some(seconds),
                )
                .map_err(|error| Self::system_error("ammo.countdown", error))?
            {
                emit_run(&self.app, &snapshot);
            }
            <Self as ammo_runtime::AmmoDriver>::delay(
                self,
                std::time::Duration::from_secs(1),
                Arc::clone(cancelled),
            )
            .await?;
        }
        Ok(())
    }

    async fn focus(&self) -> Result<(), String> {
        use desktop_runtime::DesktopRuntime;
        let executable = self.game_executable_path.clone();
        tokio::task::spawn_blocking(move || {
            let desktop = desktop_runtime::WindowsDesktopRuntime;
            let window = desktop
                .find_primary_window(&executable)?
                .ok_or_else(|| "未找到游戏窗口".to_string())?;
            desktop.restore_and_focus(&executable, window)
        })
        .await
        .map_err(|error| format!("游戏窗口任务失败: {error}"))?
    }

    async fn park_mouse(
        &self,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), String> {
        crate::input_simulation::move_region_center_cancellable(
            self.mouse_parking_region.clone(),
            cancelled,
        )
        .await
    }

    async fn click_region(
        &self,
        region: crate::morse::types::RegionRect,
        step: &str,
        countdown: bool,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), ammo_runtime::AmmoDriverError> {
        if countdown {
            self.countdown(&cancelled).await?;
        }
        self.focus()
            .await
            .map_err(|error| Self::system_error("ammo.window", error))?;
        self.emit_update(LoginRunStatus::Inputting, &format!("正在点击 {step}"))
            .map_err(|error| Self::system_error(step, error))?;
        crate::input_simulation::click_region_center_held_cancellable(
            region,
            MOUSE_CLICK_HOLD_MS,
            Arc::clone(&cancelled),
        )
        .await
        .map_err(|error| Self::cancelled_or_system(&cancelled, step, error))?;
        self.park_mouse(Arc::clone(&cancelled))
            .await
            .map_err(|error| Self::cancelled_or_system(&cancelled, step, error))
    }

    async fn save_ammo_success_diagnostic(&self) -> Result<std::path::PathBuf, String> {
        let region = self
            .targets
            .get("ammo.success")
            .ok_or_else(|| "ammo.success 校准目标不存在".to_string())?
            .region
            .clone();
        let settings_file = settings_path(&self.app)?;
        let diagnostics_dir = settings_file
            .parent()
            .ok_or_else(|| "无法解析特勤处配置目录".to_string())?
            .join("special_ops_diagnostics");
        let path =
            diagnostics_dir.join(ammo_success_diagnostic_filename(now_ms(), &self.account_id));
        let saved_path = path.clone();
        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&diagnostics_dir)
                .map_err(|error| format!("创建诊断目录失败: {error}"))?;
            let captured = crate::recognition::watcher::capture_region(&region)
                .ok_or_else(|| "截取 ammo.success 诊断区域失败".to_string())?;
            captured
                .save(&saved_path)
                .map_err(|error| format!("保存 ammo.success 诊断截图失败: {error}"))?;
            Ok::<_, String>(saved_path)
        })
        .await
        .map_err(|error| format!("诊断截图任务失败: {error}"))?
    }

    fn persist_change(
        &self,
        step: &str,
        apply: impl FnOnce(&mut SpecialOpsSettings) -> Result<(), String>,
    ) -> Result<(), ammo_runtime::AmmoDriverError> {
        let (_settings, revision) = self
            .coordinator
            .with_runtime_change(|| {
                let mut next = self
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?
                    .clone();
                apply(&mut next)?;
                save_settings(&self.app, &next)?;
                *self
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
                Ok::<_, String>(next)
            })
            .map_err(|message| Self::system_error(step, message))?;
        emit_state(&self.app, revision, now_ms());
        Ok(())
    }
}

impl ammo_runtime::AmmoDriver for ProductionAmmoDriver {
    fn update_stage(&self, message: &str) -> Result<(), ammo_runtime::AmmoDriverError> {
        self.emit_update(LoginRunStatus::Waiting, message)
            .map_err(|error| Self::system_error("ammo.progress", error))
    }

    async fn wait_and_click(
        &self,
        target: &str,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), ammo_runtime::AmmoDriverError> {
        let matched = <Self as ammo_runtime::AmmoDriver>::wait_target(
            self,
            &[target],
            Arc::clone(&cancelled),
        )
        .await?;
        if matched != target {
            return Err(Self::system_error(
                target,
                format!("识别结果与目标不一致：{matched}"),
            ));
        }
        let region = self
            .targets
            .get(target)
            .ok_or_else(|| Self::system_error(target, "校准目标不存在"))?
            .region
            .clone();
        self.countdown(&cancelled).await?;
        self.focus()
            .await
            .map_err(|error| Self::system_error("ammo.window", error))?;
        self.emit_update(LoginRunStatus::Inputting, &format!("正在点击 {target}"))
            .map_err(|error| Self::system_error(target, error))?;
        crate::input_simulation::click_region_center_held_cancellable(
            region,
            MOUSE_CLICK_HOLD_MS,
            Arc::clone(&cancelled),
        )
        .await
        .map_err(|error| Self::cancelled_or_system(&cancelled, target, error))?;
        self.park_mouse(Arc::clone(&cancelled))
            .await
            .map_err(|error| Self::cancelled_or_system(&cancelled, target, error))
    }

    async fn click_unverified(
        &self,
        target: &str,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), ammo_runtime::AmmoDriverError> {
        let region = self
            .targets
            .get(target)
            .ok_or_else(|| Self::system_error(target, "校准目标不存在"))?
            .region
            .clone();
        self.click_region(region, target, true, cancelled).await
    }

    async fn wait_target(
        &self,
        targets: &[&str],
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<String, ammo_runtime::AmmoDriverError> {
        self.emit_update(
            LoginRunStatus::Waiting,
            &format!("正在识别 {}", targets.join(" / ")),
        )
        .map_err(|error| Self::system_error("ammo.recognition", error))?;
        self.focus()
            .await
            .map_err(|error| Self::system_error("ammo.window", error))?;
        let templates = targets
            .iter()
            .map(|key| {
                self.targets
                    .get(*key)
                    .and_then(|target| target.template.as_ref())
                    .ok_or_else(|| Self::system_error(key, "目标未配置参考图"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        match template_observer::wait_for_any_consistent_match_until(
            &template_observer::RuntimeSimilaritySampler,
            &templates,
            Arc::clone(&cancelled),
            std::time::Duration::from_secs(30),
        )
        .await
        {
            Ok((key, _)) => Ok(key),
            Err(_error) if cancelled.load(std::sync::atomic::Ordering::SeqCst) => {
                Err(ammo_runtime::AmmoDriverError::Cancelled)
            }
            Err(error) if error.contains("超时") => {
                let message = if targets.len() == 1 && targets[0] == "ammo.success" {
                    match self.save_ammo_success_diagnostic().await {
                        Ok(path) => format!("{error}；诊断截图：{}", path.display()),
                        Err(diagnostic_error) => {
                            format!("{error}；诊断截图保存失败：{diagnostic_error}")
                        }
                    }
                } else {
                    error
                };
                Err(ammo_runtime::AmmoDriverError::Target(message))
            }
            Err(error) => Err(Self::system_error("ammo.recognition", error)),
        }
    }

    async fn position_and_click(
        &self,
        point: &crate::morse::types::RegionRect,
        scroll_steps: u32,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), ammo_runtime::AmmoDriverError> {
        self.countdown(&cancelled).await?;
        self.focus()
            .await
            .map_err(|error| Self::system_error("ammo.window", error))?;
        self.emit_update(LoginRunStatus::Inputting, "正在重置并定位子弹")
            .map_err(|error| Self::system_error("ammo.targetPosition", error))?;
        crate::input_simulation::press_primary_key_sequence_cancellable(
            ammo_reset_keys().to_vec(),
            AMMO_RESET_KEY_DELAY_MS,
            Arc::clone(&cancelled),
        )
        .await
        .map_err(|error| Self::cancelled_or_system(&cancelled, "ammo.targetPosition", error))?;
        let segments = ammo_scroll_segments(scroll_steps);
        if !segments.is_empty() {
            crate::input_simulation::scroll_region_segments_cancellable(
                point.clone(),
                segments,
                AMMO_SCROLL_STEP_INTERVAL_MS,
                Arc::clone(&cancelled),
            )
            .await
            .map_err(|error| Self::cancelled_or_system(&cancelled, "ammo.targetPosition", error))?;
        }
        <Self as ammo_runtime::AmmoDriver>::delay(
            self,
            std::time::Duration::from_millis(AMMO_SCROLL_SETTLE_MS),
            Arc::clone(&cancelled),
        )
        .await?;
        crate::input_simulation::click_region_center_held_cancellable(
            point.clone(),
            MOUSE_CLICK_HOLD_MS,
            Arc::clone(&cancelled),
        )
        .await
        .map_err(|error| Self::cancelled_or_system(&cancelled, "ammo.targetPosition", error))?;
        self.park_mouse(Arc::clone(&cancelled))
            .await
            .map_err(|error| Self::cancelled_or_system(&cancelled, "ammo.targetPosition", error))
    }

    async fn delay(
        &self,
        duration: std::time::Duration,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), ammo_runtime::AmmoDriverError> {
        let deadline = tokio::time::Instant::now() + duration;
        while tokio::time::Instant::now() < deadline {
            if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(ammo_runtime::AmmoDriverError::Cancelled);
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        Ok(())
    }

    fn persist_success(&self, target_id: &str) -> Result<(), ammo_runtime::AmmoDriverError> {
        self.persist_change("ammo.persistSuccess", |settings| {
            apply_ammo_success(settings, &self.account_id, target_id, &self.day)
        })
    }

    fn persist_failure(
        &self,
        target_id: &str,
        step: &str,
        message: &str,
    ) -> Result<(), ammo_runtime::AmmoDriverError> {
        self.persist_change("ammo.persistFailure", |settings| {
            apply_ammo_failure(
                settings,
                &self.account_id,
                target_id,
                &self.day,
                step,
                message,
                now_ms(),
            )
        })
    }

    fn persist_isolated(
        &self,
        target_id: &str,
        step: &str,
        message: &str,
    ) -> Result<(), ammo_runtime::AmmoDriverError> {
        self.persist_change("ammo.persistIsolated", |settings| {
            apply_ammo_isolated(
                settings,
                &self.account_id,
                target_id,
                step,
                message,
                now_ms(),
            )
        })
    }
}

impl military_supply_runtime::MilitarySupplyEntryDriver for ProductionAmmoDriver {
    async fn wait_and_click(
        &self,
        key: &str,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), military_supply_runtime::MilitarySupplyEntryError> {
        <Self as ammo_runtime::AmmoDriver>::wait_and_click(self, key, cancelled)
            .await
            .map_err(|error| map_military_supply_input_error(error, key))
    }

    async fn click_unverified(
        &self,
        key: &str,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), military_supply_runtime::MilitarySupplyEntryError> {
        <Self as ammo_runtime::AmmoDriver>::click_unverified(self, key, cancelled)
            .await
            .map_err(|error| map_military_supply_input_error(error, key))
    }

    async fn delay(
        &self,
        duration: std::time::Duration,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), military_supply_runtime::MilitarySupplyEntryError> {
        <Self as ammo_runtime::AmmoDriver>::delay(self, duration, cancelled)
            .await
            .map_err(|error| map_military_supply_input_error(error, "ammo.entryDelay"))
    }
}

fn map_military_supply_input_error(
    error: ammo_runtime::AmmoDriverError,
    fallback_step: &str,
) -> military_supply_runtime::MilitarySupplyEntryError {
    match error {
        ammo_runtime::AmmoDriverError::Cancelled => {
            military_supply_runtime::MilitarySupplyEntryError::Cancelled
        }
        ammo_runtime::AmmoDriverError::Target(message) => {
            military_supply_runtime::MilitarySupplyEntryError::Target {
                step: fallback_step.to_string(),
                message,
            }
        }
        ammo_runtime::AmmoDriverError::System { step, message } => {
            military_supply_runtime::MilitarySupplyEntryError::System { step, message }
        }
    }
}

struct RoundCraftProgress {
    account_index: usize,
    account_total: usize,
    qq_account: String,
}

impl ProductionCraftBatchDriver {
    fn config_for(
        &self,
        task: &craft_batch::CraftBatchTask,
    ) -> Result<&craft_runtime::CraftRunConfig, craft_trial::CraftTrialFailure> {
        self.frozen
            .iter()
            .find(|frozen| frozen.task.station == task.station)
            .map(|frozen| &frozen.config)
            .ok_or_else(|| craft_trial::CraftTrialFailure {
                step: "craft.batchConfig".to_string(),
                message: "制作批处理冻结配置缺失".to_string(),
                requires_uncertain: false,
            })
    }

    fn trial_driver(
        &self,
        task: &craft_batch::CraftBatchTask,
    ) -> Result<craft_runtime::ProductionCraftTrialDriver, craft_trial::CraftTrialFailure> {
        let config = self.config_for(task)?;
        Ok(craft_runtime::ProductionCraftTrialDriver::new(
            self.app.clone(),
            Arc::clone(&self.runtime),
            self.run_id,
            config.game_executable_path.clone(),
            config.mouse_parking_region.clone(),
            config.targets.clone(),
        ))
    }
}

impl craft_batch::CraftBatchDriver for ProductionCraftBatchDriver {
    async fn ensure_station_grid(
        &self,
        task: &craft_batch::CraftBatchTask,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), craft_trial::CraftTrialFailure> {
        let driver = self.trial_driver(task)?;
        craft_runtime::ensure_station_grid(&driver, cancelled).await
    }

    fn update_progress(
        &self,
        index: usize,
        total: usize,
        station: &StationKind,
    ) -> Result<(), String> {
        let label = match station {
            StationKind::TechnicalCenter => "技术中心",
            StationKind::Workbench => "工作台",
            StationKind::Pharmacy => "制药台",
            StationKind::ArmorBench => "防具台",
        };
        let driver = self
            .frozen
            .iter()
            .find(|frozen| frozen.task.station == *station)
            .ok_or_else(|| "制作批处理冻结配置缺失".to_string())
            .and_then(|frozen| {
                self.trial_driver(&frozen.task)
                    .map_err(|failure| failure.message)
            })?;
        if let Some(progress) = self.round_progress.as_ref() {
            if let Some(snapshot) = self.runtime.update_round_progress(
                self.run_id,
                progress.account_index,
                progress.account_total,
                &self.account_id,
                &progress.qq_account,
                Some(station.clone()),
                index,
                total,
            )? {
                emit_run(&self.app, &snapshot);
            }
        }
        craft_runtime::CraftTrialDriver::update_stage(
            &driver,
            LoginRunStatus::Waiting,
            &format!("正在处理 {index}/{total}：{label}"),
        )
    }

    async fn run_station(
        &self,
        task: &craft_batch::CraftBatchTask,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> craft_batch::StationAttempt {
        let driver = match self.trial_driver(task) {
            Ok(driver) => driver,
            Err(failure) => {
                return craft_batch::StationAttempt {
                    result: Err(failure),
                    entered_input: false,
                };
            }
        };
        let config = match self.config_for(task) {
            Ok(config) => config,
            Err(failure) => {
                return craft_batch::StationAttempt {
                    result: Err(failure),
                    entered_input: false,
                };
            }
        };
        let result = craft_runtime::run_craft_station(
            &driver,
            task.station.clone(),
            config.delays,
            cancelled,
        )
        .await;
        craft_batch::StationAttempt {
            result,
            entered_input: driver.input_started(),
        }
    }

    fn persist_started(
        &self,
        task: &craft_batch::CraftBatchTask,
        started_at_ms: i64,
    ) -> Result<(), String> {
        persist_craft_success_with(
            &self.settings,
            &self.coordinator,
            &self.account_id,
            &task.station,
            started_at_ms,
            task.duration_minutes,
            |next| save_settings(&self.app, next),
        )
        .map(|(_settings, revision)| {
            emit_state(&self.app, revision, now_ms());
        })
    }

    async fn return_started(
        &self,
        task: &craft_batch::CraftBatchTask,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), craft_trial::CraftTrialFailure> {
        let driver = self.trial_driver(task)?;
        craft_runtime::return_to_station_grid(&driver, cancelled).await
    }
}

struct ProductionLimitedSupplyDriver {
    input: ProductionAmmoDriver,
    cycle_id: String,
    colors: [[u8; 3]; 2],
    color_tolerances: [u8; 2],
    persist_result: bool,
}

impl ProductionLimitedSupplyDriver {
    fn map_input_error(
        error: ammo_runtime::AmmoDriverError,
    ) -> limited_supply_runtime::LimitedRunError {
        match error {
            ammo_runtime::AmmoDriverError::Cancelled => {
                limited_supply_runtime::LimitedRunError::Cancelled
            }
            ammo_runtime::AmmoDriverError::Target(message) => {
                limited_supply_runtime::LimitedRunError::System {
                    step: "limited.input".to_string(),
                    message,
                }
            }
            ammo_runtime::AmmoDriverError::System { step, message } => {
                limited_supply_runtime::LimitedRunError::System { step, message }
            }
        }
    }
}

impl limited_supply_runtime::LimitedSupplyDriver for ProductionLimitedSupplyDriver {
    async fn wait_and_click(
        &self,
        key: &str,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), limited_supply_runtime::LimitedRunError> {
        let result = <ProductionAmmoDriver as ammo_runtime::AmmoDriver>::wait_and_click(
            &self.input,
            key,
            cancelled,
        )
        .await;
        result.map_err(Self::map_input_error)
    }

    async fn delay(
        &self,
        duration: std::time::Duration,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), limited_supply_runtime::LimitedRunError> {
        <ProductionAmmoDriver as ammo_runtime::AmmoDriver>::delay(&self.input, duration, cancelled)
            .await
            .map_err(Self::map_input_error)
    }

    async fn wait_ready(
        &self,
        timeout: std::time::Duration,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), limited_supply_runtime::LimitedRunError> {
        self.input
            .emit_update(LoginRunStatus::Waiting, "正在识别限时商品页面")
            .map_err(|message| limited_supply_runtime::LimitedRunError::System {
                step: "limited.ready".to_string(),
                message,
            })?;
        self.input.focus().await.map_err(|message| {
            limited_supply_runtime::LimitedRunError::System {
                step: "limited.window".to_string(),
                message,
            }
        })?;
        let template = self
            .input
            .targets
            .get("limited.ready")
            .and_then(|target| target.template.as_ref())
            .ok_or_else(|| limited_supply_runtime::LimitedRunError::System {
                step: "limited.ready".to_string(),
                message: "限时商品页面就绪目标未配置已验证模板".to_string(),
            })?;
        match template_observer::wait_for_any_consistent_match_until(
            &template_observer::RuntimeSimilaritySampler,
            &[template],
            Arc::clone(&cancelled),
            timeout,
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(_) if cancelled.load(std::sync::atomic::Ordering::SeqCst) => {
                Err(limited_supply_runtime::LimitedRunError::Cancelled)
            }
            Err(error) if error.contains("超时") => {
                Err(limited_supply_runtime::LimitedRunError::ReadyTimeout)
            }
            Err(message) => Err(limited_supply_runtime::LimitedRunError::System {
                step: "limited.ready".to_string(),
                message,
            }),
        }
    }

    async fn sample_colors(
        &self,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<
        Option<limited_supply_runtime::LimitedColorSample>,
        limited_supply_runtime::LimitedRunError,
    > {
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(limited_supply_runtime::LimitedRunError::Cancelled);
        }
        self.input
            .emit_update(LoginRunStatus::Waiting, "正在识别限时商品")
            .map_err(|message| limited_supply_runtime::LimitedRunError::System {
                step: "limited.colors".to_string(),
                message,
            })?;
        self.input.focus().await.map_err(|message| {
            limited_supply_runtime::LimitedRunError::System {
                step: "limited.window".to_string(),
                message,
            }
        })?;
        self.input
            .park_mouse(Arc::clone(&cancelled))
            .await
            .map_err(|message| limited_supply_runtime::LimitedRunError::System {
                step: "limited.mouseParking".to_string(),
                message,
            })?;
        let regions = (1..=9)
            .map(|index| {
                self.input
                    .targets
                    .get(&format!("limited.color.{index}"))
                    .map(|target| target.region.clone())
                    .ok_or_else(|| limited_supply_runtime::LimitedRunError::System {
                        step: "limited.colors".to_string(),
                        message: format!("限时商品识色区域 {index} 未配置"),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let colors = self.colors;
        let tolerances = self.color_tolerances;
        tokio::task::spawn_blocking(move || {
            let screenshots = regions
                .iter()
                .map(crate::recognition::watcher::capture_region)
                .collect::<Option<Vec<_>>>()?;
            limited_supply_runtime::match_limited_color_sample(&screenshots, colors, tolerances)
        })
        .await
        .map_err(|error| limited_supply_runtime::LimitedRunError::System {
            step: "limited.colors".to_string(),
            message: format!("限时商品截图任务失败：{error}"),
        })
    }

    fn persist_result(
        &self,
        result: &limited_supply_runtime::LimitedSupplyCheckResult,
    ) -> Result<(), limited_supply_runtime::LimitedRunError> {
        if !self.persist_result {
            return Ok(());
        }
        let account_id = self.input.account_id.clone();
        let cycle_id = self.cycle_id.clone();
        let result = result.clone();
        self.input
            .persist_change("limited.persistResult", move |settings| {
                let account = settings
                    .accounts
                    .iter_mut()
                    .find(|account| account.id == account_id)
                    .ok_or_else(|| "限时商品账号不存在".to_string())?;
                account.limited_supply = limited_supply::LimitedSupplyAccountState {
                    cycle_id: Some(cycle_id),
                    outcome: result.outcome,
                    checked_at_ms: Some(now_ms()),
                    matched_region: result.matched_region,
                    matched_color: result.matched_color,
                    acknowledged: false,
                    last_error: result.error,
                };
                Ok(())
            })
            .map_err(Self::map_input_error)
    }
}

struct ProductionMarketDriver {
    input: ProductionAmmoDriver,
    control: Arc<RoundControl>,
    day: String,
    persist_official: bool,
    trial_completed: std::sync::atomic::AtomicU32,
}

impl ProductionMarketDriver {
    fn map_input_error(error: ammo_runtime::AmmoDriverError) -> market_runtime::MarketRunError {
        match error {
            ammo_runtime::AmmoDriverError::Cancelled => market_runtime::MarketRunError::Cancelled,
            ammo_runtime::AmmoDriverError::Target(message) => {
                market_runtime::MarketRunError::System {
                    step: "market.input".to_string(),
                    message,
                }
            }
            ammo_runtime::AmmoDriverError::System { step, message } => {
                market_runtime::MarketRunError::System { step, message }
            }
        }
    }

    fn system_error(step: &str, message: impl Into<String>) -> market_runtime::MarketRunError {
        market_runtime::MarketRunError::System {
            step: step.to_string(),
            message: message.into(),
        }
    }

    fn persist_status(
        &self,
        status: market_purchase::MarketTaskStatus,
        error: Option<String>,
    ) -> Result<(), market_runtime::MarketRunError> {
        let account_id = self.input.account_id.clone();
        let day = self.day.clone();
        self.input
            .persist_change("market.persistStatus", move |settings| {
                let account = settings
                    .accounts
                    .iter_mut()
                    .find(|account| account.id == account_id)
                    .ok_or_else(|| "交易行账号不存在".to_string())?;
                if account.market.day.as_deref() != Some(day.as_str()) {
                    account.market.day = Some(day);
                    account.market.completed_count = 0;
                }
                account.market.status = status;
                account.market.last_error = error;
                Ok(())
            })
            .map_err(Self::map_input_error)
    }
}

impl market_runtime::MarketDriver for ProductionMarketDriver {
    async fn click(
        &self,
        key: &str,
        countdown: bool,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), market_runtime::MarketRunError> {
        if key == "market.entry" {
            <ProductionAmmoDriver as ammo_runtime::AmmoDriver>::wait_target(
                &self.input,
                &[key],
                Arc::clone(&cancelled),
            )
            .await
            .map_err(Self::map_input_error)?;
        }
        let region = self
            .input
            .targets
            .get(key)
            .map(|target| target.region.clone())
            .ok_or_else(|| Self::system_error(key, "交易行校准目标不存在"))?;
        self.input
            .click_region(region, key, countdown, cancelled)
            .await
            .map_err(Self::map_input_error)
    }

    async fn click_point(
        &self,
        point: &crate::morse::types::RegionRect,
        countdown: bool,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), market_runtime::MarketRunError> {
        self.input
            .click_region(point.clone(), "market.product", countdown, cancelled)
            .await
            .map_err(Self::map_input_error)
    }

    async fn read_price(
        &self,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<String, market_runtime::MarketRunError> {
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(market_runtime::MarketRunError::Cancelled);
        }
        self.input
            .emit_update(LoginRunStatus::Waiting, "正在读取交易行价格")
            .map_err(|message| Self::system_error("market.price", message))?;
        self.input
            .focus()
            .await
            .map_err(|message| Self::system_error("market.window", message))?;
        self.input
            .park_mouse(Arc::clone(&cancelled))
            .await
            .map_err(|message| Self::system_error("market.mouseParking", message))?;
        let region = self
            .input
            .targets
            .get("market.price")
            .map(|target| target.region.clone())
            .ok_or_else(|| Self::system_error("market.price", "交易行价格 OCR 区域未配置"))?;
        tokio::task::spawn_blocking(move || {
            let image = crate::recognition::watcher::capture_region(&region)
                .ok_or_else(|| "截取交易行价格区域失败".to_string())?;
            let words = windows_ocr::recognize_words(image)?;
            Ok::<_, String>(
                words
                    .into_iter()
                    .map(|word| word.text)
                    .collect::<Vec<_>>()
                    .join(""),
            )
        })
        .await
        .map_err(|error| Self::system_error("market.price", format!("价格 OCR 任务失败：{error}")))?
        .map_err(|message| Self::system_error("market.price", message))
    }

    async fn delay(
        &self,
        duration: std::time::Duration,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), market_runtime::MarketRunError> {
        <ProductionAmmoDriver as ammo_runtime::AmmoDriver>::delay(&self.input, duration, cancelled)
            .await
            .map_err(Self::map_input_error)
    }

    fn persist_purchase_click(&self) -> Result<u32, market_runtime::MarketRunError> {
        if !self.persist_official {
            return Ok(self
                .trial_completed
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                .saturating_add(1));
        }
        let account_id = self.input.account_id.clone();
        let day = self.day.clone();
        let mut completed_count = 0;
        self.input
            .persist_change("market.persistPurchase", |settings| {
                let account = settings
                    .accounts
                    .iter_mut()
                    .find(|account| account.id == account_id)
                    .ok_or_else(|| "交易行账号不存在".to_string())?;
                if account.market.day.as_deref() != Some(day.as_str()) {
                    account.market.day = Some(day);
                    account.market.completed_count = 0;
                }
                account.market.completed_count = account.market.completed_count.saturating_add(1);
                account.market.status = market_purchase::MarketTaskStatus::Running;
                account.market.last_error = None;
                completed_count = account.market.completed_count;
                Ok(())
            })
            .map_err(Self::map_input_error)?;
        Ok(completed_count)
    }

    fn minute_of_day(&self) -> u16 {
        local_day_and_minute(now_ms()).1 as u16
    }

    fn now_ms(&self) -> i64 {
        now_ms()
    }

    fn pause_requested(&self) -> bool {
        self.control.pause_requested()
    }

    fn next_craft_at_ms(&self) -> Option<i64> {
        let settings = self.input.settings.lock().ok()?;
        build_schedule(&settings, now_ms())
            .timeline_tasks
            .into_iter()
            .filter(|task| {
                task.kind == TimelineTaskKind::Craft
                    && task.account_status == AccountStatus::Ready
                    && task.manual_failure.is_none()
            })
            .map(|task| task.scheduled_at_ms)
            .min()
    }
}

type FrozenRoundAccountCache =
    Mutex<std::collections::HashMap<(String, i64), Result<Arc<FrozenRoundAccount>, String>>>;

struct ProductionRoundDriver {
    app: AppHandle,
    settings: Arc<Mutex<SpecialOpsSettings>>,
    coordinator: Arc<SettingsCoordinator>,
    runtime: Arc<login_runtime::LoginRuntime>,
    control: Arc<RoundControl>,
    run_id: u64,
    accounts: std::collections::HashMap<(String, i64), Result<Arc<FrozenRoundAccount>, String>>,
    dynamic_accounts: FrozenRoundAccountCache,
    game_executable_path: std::path::PathBuf,
}

fn map_round_ammo_stop(
    stop: ammo_runtime::AmmoRunStop,
) -> Result<(), round_runner::AccountRunError> {
    match stop {
        ammo_runtime::AmmoRunStop::Completed => Ok(()),
        ammo_runtime::AmmoRunStop::Isolated {
            target_id, message, ..
        } => Err(round_runner::AccountRunError::account_ammo(
            target_id,
            "ammo.isolated",
            message,
        )),
        ammo_runtime::AmmoRunStop::Uncertain {
            target_id,
            step,
            message,
        } => Err(round_runner::AccountRunError::account_ammo(
            target_id, step, message,
        )),
        ammo_runtime::AmmoRunStop::SystemFailure { step, message } => {
            if step == "ammo.department" {
                Err(round_runner::AccountRunError::navigation_timeout_with_message(step, message))
            } else {
                Err(round_runner::AccountRunError::system(step, message))
            }
        }
        ammo_runtime::AmmoRunStop::EmergencyStopped => Err(round_runner::AccountRunError::system(
            "round.stopped",
            "多账号轮次已停止",
        )),
    }
}

fn map_round_ammo_driver_error(
    error: ammo_runtime::AmmoDriverError,
    fallback_step: &str,
) -> round_runner::AccountRunError {
    match error {
        ammo_runtime::AmmoDriverError::Cancelled => {
            round_runner::AccountRunError::system("round.stopped", "多账号轮次已停止")
        }
        ammo_runtime::AmmoDriverError::Target(message) => {
            round_runner::AccountRunError::system(fallback_step, message)
        }
        ammo_runtime::AmmoDriverError::System { step, message } => {
            round_runner::AccountRunError::system(step, message)
        }
    }
}

fn map_round_military_supply_entry_error(
    error: military_supply_runtime::MilitarySupplyEntryError,
) -> round_runner::AccountRunError {
    match error {
        military_supply_runtime::MilitarySupplyEntryError::Cancelled => {
            round_runner::AccountRunError::system("round.stopped", "多账号轮次已停止")
        }
        military_supply_runtime::MilitarySupplyEntryError::Target { step, message }
            if step == "ammo.department" =>
        {
            round_runner::AccountRunError::navigation_timeout_with_message(step, message)
        }
        military_supply_runtime::MilitarySupplyEntryError::Target { step, message }
        | military_supply_runtime::MilitarySupplyEntryError::System { step, message } => {
            round_runner::AccountRunError::system(step, message)
        }
    }
}

fn ammo_driver_error_to_stop(
    error: ammo_runtime::AmmoDriverError,
    fallback_step: &str,
) -> ammo_runtime::AmmoRunStop {
    match error {
        ammo_runtime::AmmoDriverError::Cancelled => ammo_runtime::AmmoRunStop::EmergencyStopped,
        ammo_runtime::AmmoDriverError::Target(message) => {
            ammo_runtime::AmmoRunStop::SystemFailure {
                step: fallback_step.to_string(),
                message,
            }
        }
        ammo_runtime::AmmoDriverError::System { step, message } => {
            ammo_runtime::AmmoRunStop::SystemFailure { step, message }
        }
    }
}

fn map_round_limited_error(
    error: limited_supply_runtime::LimitedRunError,
    fallback_step: &str,
) -> round_runner::AccountRunError {
    match error {
        limited_supply_runtime::LimitedRunError::Cancelled => {
            round_runner::AccountRunError::system("round.stopped", "多账号轮次已停止")
        }
        limited_supply_runtime::LimitedRunError::ReadyTimeout => {
            round_runner::AccountRunError::system(fallback_step, "限时商品页面就绪超时")
        }
        limited_supply_runtime::LimitedRunError::System { step, message } => {
            round_runner::AccountRunError::system(step, message)
        }
    }
}

fn limited_error_to_stop(
    error: limited_supply_runtime::LimitedRunError,
) -> limited_supply_runtime::LimitedRunStop {
    match error {
        limited_supply_runtime::LimitedRunError::Cancelled => {
            limited_supply_runtime::LimitedRunStop::EmergencyStopped
        }
        limited_supply_runtime::LimitedRunError::ReadyTimeout => {
            limited_supply_runtime::LimitedRunStop::RetryableReadyTimeout
        }
        limited_supply_runtime::LimitedRunError::System { step, message } => {
            limited_supply_runtime::LimitedRunStop::SystemFailure { step, message }
        }
    }
}

fn map_round_navigation_result(
    result: game_navigation::GameNavigationResult,
) -> Result<(), round_runner::AccountRunError> {
    match result {
        game_navigation::GameNavigationResult::Ready => Ok(()),
        game_navigation::GameNavigationResult::TimedOut { failed_step } => {
            Err(round_runner::AccountRunError::navigation_timeout(format!(
                "navigation.{failed_step:?}"
            )))
        }
        game_navigation::GameNavigationResult::Paused {
            failed_step,
            message,
        } => Err(round_runner::AccountRunError::system(
            format!("navigation.{failed_step:?}"),
            message,
        )),
        game_navigation::GameNavigationResult::EmergencyStopped => Err(
            round_runner::AccountRunError::system("round.stopped", "多账号轮次已停止"),
        ),
    }
}

fn map_round_login_failure(
    failed_step: login_flow::LoginStep,
    last_observation: &str,
) -> round_runner::AccountRunError {
    let message = format!("{failed_step:?}：{last_observation}");
    if matches!(
        failed_step,
        login_flow::LoginStep::InitialCountdown
            | login_flow::LoginStep::StopGame
            | login_flow::LoginStep::StopWeGame
            | login_flow::LoginStep::StartWeGame
    ) {
        round_runner::AccountRunError::system(format!("login.{failed_step:?}"), message)
    } else if matches!(
        failed_step,
        login_flow::LoginStep::WaitGameEntry
            | login_flow::LoginStep::OpenGameEntry
            | login_flow::LoginStep::WaitLaunchButton
            | login_flow::LoginStep::LaunchGame
            | login_flow::LoginStep::WaitGameWindow
    ) {
        round_runner::AccountRunError::navigation_timeout(format!("login.{failed_step:?}"))
    } else {
        round_runner::AccountRunError::account("login.failed", message)
    }
}

impl ProductionRoundDriver {
    fn frozen(
        &self,
        task: &round_planner::AccountRoundTask,
    ) -> Result<Arc<FrozenRoundAccount>, round_runner::AccountRunError> {
        let key = (task.account_id.clone(), task.scheduled_at_ms);
        if let Some(frozen) = self.accounts.get(&key) {
            return frozen.clone().map_err(|message| {
                round_runner::AccountRunError::account("round.preflight", message)
            });
        }
        self.dynamic_accounts
            .lock()
            .map_err(|_| {
                round_runner::AccountRunError::system("round.preflight", "动态冻结配置状态已损坏")
            })?
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                round_runner::AccountRunError::account("round.preflight", "账号冻结配置不存在")
            })?
            .map_err(|message| round_runner::AccountRunError::account("round.preflight", message))
    }

    fn emit_step(&self, step: login_flow::LoginStep, message: &str) {
        if let Ok(Some(snapshot)) = self.runtime.update(
            self.run_id,
            LoginRunStatus::Waiting,
            Some(step),
            message,
            None,
        ) {
            emit_run(&self.app, &snapshot);
        }
    }

    fn persist_account_error(
        &self,
        account_id: &str,
        error: &round_runner::AccountRunError,
    ) -> Result<(), String> {
        let (_settings, revision) = self.coordinator.with_runtime_change(|| {
            let mut next = self
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            apply_round_account_failure(&mut next, account_id, error, now_ms())?;
            save_settings(&self.app, &next)?;
            *self
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
            Ok::<_, String>(next)
        })?;
        emit_state(&self.app, revision, now_ms());
        Ok(())
    }

    fn persist_global_pause(&self, reason: &str) -> Result<(), String> {
        let (_settings, revision) = self.coordinator.with_runtime_change(|| {
            let mut next = self
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            next.paused = true;
            next.paused_reason = Some(reason.to_string());
            save_settings(&self.app, &next)?;
            *self
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
            Ok::<_, String>(next)
        })?;
        crate::log_warn!("special_ops::round", "多账号轮次已暂停", "reason" => reason);
        if let Some(state) = self.app.try_state::<SpecialOpsState>() {
            state
                .profit_runtime
                .invalidate("轮次已暂停，利润查询已取消");
        }
        emit_state(&self.app, revision, now_ms());
        Ok(())
    }

    async fn focus_existing_game(&self) -> Result<(), round_runner::AccountRunError> {
        use desktop_runtime::DesktopRuntime;
        let executable = self.game_executable_path.clone();
        tokio::task::spawn_blocking(move || {
            let runtime = desktop_runtime::WindowsDesktopRuntime;
            let window = runtime
                .find_primary_window(&executable)?
                .ok_or_else(|| "同账号等待结束后未找到游戏窗口".to_string())?;
            runtime.restore_and_focus(&executable, window)
        })
        .await
        .map_err(|error| {
            round_runner::AccountRunError::navigation_timeout_with_message(
                "round.sessionWindow",
                format!("游戏窗口验证任务失败：{error}"),
            )
        })?
        .map_err(|message| {
            round_runner::AccountRunError::navigation_timeout_with_message(
                "round.sessionWindow",
                message,
            )
        })
    }
}

impl round_account::AccountSessionDriver for ProductionRoundDriver {
    async fn login(
        &self,
        task: &round_planner::AccountRoundTask,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), round_runner::AccountRunError> {
        let frozen = self.frozen(task)?;
        let driver = login_runtime::ProductionLoginDriver::new(
            self.app.clone(),
            Arc::clone(&self.runtime),
            self.run_id,
            Arc::clone(&frozen.login),
        );
        let result = login_flow::run_login_flow(&driver, &frozen.login, cancelled, |step| {
            self.emit_step(step, login_step_message(&step))
        })
        .await;
        match result {
            login_flow::LoginFlowResult::GameReady { .. } => Ok(()),
            login_flow::LoginFlowResult::NeedsManualLogin {
                failure_message, ..
            } => Err(round_runner::AccountRunError::account(
                "login.needsManual",
                failure_message,
            )),
            login_flow::LoginFlowResult::Paused {
                failed_step,
                last_observation,
                ..
            } => Err(map_round_login_failure(failed_step, &last_observation)),
            login_flow::LoginFlowResult::EmergencyStopped { .. } => Err(
                round_runner::AccountRunError::system("round.stopped", "多账号轮次已停止"),
            ),
        }
    }

    async fn navigate(
        &self,
        task: &round_planner::AccountRoundTask,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), round_runner::AccountRunError> {
        crate::log_info!(
            "special_ops::round",
            "账号开始游戏内导航",
            "account_id" => &task.account_id,
            "qq_account" => &task.qq_account,
            "account_order" => task.account_order
        );
        let frozen = self.frozen(task)?;
        let driver = game_navigation::ProductionGameNavigationDriver::new(
            self.app.clone(),
            Arc::clone(&self.runtime),
            self.run_id,
            Arc::clone(&frozen.navigation),
        );
        let result = game_navigation::run_game_navigation(
            &driver,
            frozen.navigation.destination,
            frozen.navigation.delays,
            cancelled,
            |step| {
                let step = login_flow::LoginStep::from(step);
                self.emit_step(step, login_step_message(&step));
            },
        )
        .await;
        let mapped = map_round_navigation_result(result);
        crate::log_info!(
            "special_ops::round",
            "账号游戏内导航结束",
            "account_id" => &task.account_id,
            "qq_account" => &task.qq_account,
            "success" => mapped.is_ok()
        );
        mapped
    }

    async fn craft(
        &self,
        task: &round_planner::AccountRoundTask,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<usize, round_runner::AccountRunError> {
        let frozen = self.frozen(task)?;
        let tasks = frozen
            .craft
            .iter()
            .map(|item| item.task.clone())
            .collect::<Vec<_>>();
        let driver = ProductionCraftBatchDriver {
            app: self.app.clone(),
            settings: Arc::clone(&self.settings),
            coordinator: Arc::clone(&self.coordinator),
            runtime: Arc::clone(&self.runtime),
            run_id: self.run_id,
            account_id: task.account_id.clone(),
            frozen: Arc::clone(&frozen.craft),
            round_progress: self.runtime.snapshot().ok().flatten().and_then(|snapshot| {
                snapshot.round_progress.map(|progress| RoundCraftProgress {
                    account_index: progress.account_index,
                    account_total: progress.account_total,
                    qq_account: progress.qq_account,
                })
            }),
        };
        craft_batch::run_craft_batch(&driver, &tasks, cancelled)
            .await
            .map(|success| success.processed)
            .map_err(|failure| {
                round_runner::AccountRunError::account_station(
                    failure.station,
                    failure.failure.step,
                    failure.failure.message,
                )
            })
    }

    async fn military_supply(
        &self,
        task: &round_planner::AccountRoundTask,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<round_account::MilitarySupplySessionResult, round_runner::AccountRunError> {
        let frozen_account = self.frozen(task)?;
        let entry = frozen_account
            .military_supply_entry
            .as_ref()
            .ok_or_else(|| {
                round_runner::AccountRunError::account(
                    "ammo.entryPreflight",
                    "军需处入口冻结配置缺失",
                )
            })?;
        let entry_driver = ProductionAmmoDriver {
            app: self.app.clone(),
            settings: Arc::clone(&self.settings),
            coordinator: Arc::clone(&self.coordinator),
            runtime: Arc::clone(&self.runtime),
            run_id: self.run_id,
            account_id: task.account_id.clone(),
            day: local_day_and_minute(now_ms()).0,
            game_executable_path: entry.game_executable_path.clone(),
            mouse_parking_region: entry.mouse_parking_region.clone(),
            targets: entry.targets.clone(),
        };
        military_supply_runtime::enter_military_supply(
            &entry_driver,
            entry.config,
            Arc::clone(&cancelled),
        )
        .await
        .map_err(map_round_military_supply_entry_error)?;

        if let Some(frozen) = frozen_account.ammo.as_ref() {
            let driver = ProductionAmmoDriver {
                app: self.app.clone(),
                settings: Arc::clone(&self.settings),
                coordinator: Arc::clone(&self.coordinator),
                runtime: Arc::clone(&self.runtime),
                run_id: self.run_id,
                account_id: task.account_id.clone(),
                day: frozen.day.clone(),
                game_executable_path: frozen.game_executable_path.clone(),
                mouse_parking_region: frozen.mouse_parking_region.clone(),
                targets: frozen.targets.clone(),
            };
            <ProductionAmmoDriver as ammo_runtime::AmmoDriver>::wait_and_click(
                &driver,
                "ammo.tacticalDepartment",
                Arc::clone(&cancelled),
            )
            .await
            .map_err(|error| map_round_ammo_driver_error(error, "ammo.tacticalDepartment"))?;
            let result = ammo_runtime::run_ammo_targets(
                &driver,
                &frozen.ammo_targets,
                Arc::clone(&cancelled),
            )
            .await;
            map_round_ammo_stop(result.stop)?;
        }

        let mut result = round_account::MilitarySupplySessionResult::default();
        let Some(frozen) = frozen_account.limited_supply.as_ref() else {
            return Ok(result);
        };
        let driver = ProductionLimitedSupplyDriver {
            input: ProductionAmmoDriver {
                app: self.app.clone(),
                settings: Arc::clone(&self.settings),
                coordinator: Arc::clone(&self.coordinator),
                runtime: Arc::clone(&self.runtime),
                run_id: self.run_id,
                account_id: task.account_id.clone(),
                day: frozen.cycle_id.clone(),
                game_executable_path: frozen.game_executable_path.clone(),
                mouse_parking_region: frozen.mouse_parking_region.clone(),
                targets: frozen.targets.clone(),
            },
            cycle_id: frozen.cycle_id.clone(),
            colors: frozen.colors,
            color_tolerances: frozen.color_tolerances,
            persist_result: true,
        };
        <ProductionLimitedSupplyDriver as limited_supply_runtime::LimitedSupplyDriver>::wait_and_click(
            &driver,
            "ammo.researchDepartment",
            Arc::clone(&cancelled),
        )
        .await
        .map_err(|error| map_round_limited_error(error, "ammo.researchDepartment"))?;
        match limited_supply_runtime::run_limited_supply_branch(&driver, frozen.config, cancelled)
            .await
        {
            limited_supply_runtime::LimitedRunStop::Completed(_) => Ok(result),
            limited_supply_runtime::LimitedRunStop::RetryableReadyTimeout => {
                result.limited_retry_requested = true;
                Ok(result)
            }
            limited_supply_runtime::LimitedRunStop::EmergencyStopped => Err(
                round_runner::AccountRunError::system("round.stopped", "多账号轮次已停止"),
            ),
            limited_supply_runtime::LimitedRunStop::SystemFailure { step, message } => {
                Err(round_runner::AccountRunError::system(step, message))
            }
        }
    }

    async fn market(
        &self,
        task: &round_planner::AccountRoundTask,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<round_account::MarketSessionResult, round_runner::AccountRunError> {
        let frozen_account = self.frozen(task)?;
        let frozen = frozen_account.market.as_ref().ok_or_else(|| {
            round_runner::AccountRunError::account("market.preflight", "交易行冻结配置缺失")
        })?;
        let driver = ProductionMarketDriver {
            input: ProductionAmmoDriver {
                app: self.app.clone(),
                settings: Arc::clone(&self.settings),
                coordinator: Arc::clone(&self.coordinator),
                runtime: Arc::clone(&self.runtime),
                run_id: self.run_id,
                account_id: task.account_id.clone(),
                day: frozen.day.clone(),
                game_executable_path: frozen.game_executable_path.clone(),
                mouse_parking_region: frozen.mouse_parking_region.clone(),
                targets: frozen.targets.clone(),
            },
            control: Arc::clone(&self.control),
            day: frozen.day.clone(),
            persist_official: true,
            trial_completed: std::sync::atomic::AtomicU32::new(0),
        };
        match market_runtime::run_market(&driver, frozen.config.clone(), cancelled).await {
            market_runtime::MarketRunStop::Completed => {
                driver
                    .persist_status(market_purchase::MarketTaskStatus::Completed, None)
                    .map_err(|error| match error {
                        market_runtime::MarketRunError::Cancelled => {
                            round_runner::AccountRunError::system(
                                "round.stopped",
                                "多账号轮次已停止",
                            )
                        }
                        market_runtime::MarketRunError::System { step, message } => {
                            round_runner::AccountRunError::system(step, message)
                        }
                    })?;
                Ok(round_account::MarketSessionResult::Completed)
            }
            market_runtime::MarketRunStop::YieldedForCraft => {
                Ok(round_account::MarketSessionResult::YieldedForCraft)
            }
            market_runtime::MarketRunStop::PauseRequested => {
                Ok(round_account::MarketSessionResult::PauseRequested)
            }
            market_runtime::MarketRunStop::WindowClosed => {
                driver
                    .persist_status(market_purchase::MarketTaskStatus::WindowClosed, None)
                    .map_err(|error| match error {
                        market_runtime::MarketRunError::Cancelled => {
                            round_runner::AccountRunError::system(
                                "round.stopped",
                                "多账号轮次已停止",
                            )
                        }
                        market_runtime::MarketRunError::System { step, message } => {
                            round_runner::AccountRunError::system(step, message)
                        }
                    })?;
                Ok(round_account::MarketSessionResult::WindowClosed)
            }
            market_runtime::MarketRunStop::PriceRecognitionFailed => {
                driver
                    .persist_status(
                        market_purchase::MarketTaskStatus::PriceRecognitionFailed,
                        Some("连续三个商品页未识别到有效价格".to_string()),
                    )
                    .map_err(|error| match error {
                        market_runtime::MarketRunError::Cancelled => {
                            round_runner::AccountRunError::system(
                                "round.stopped",
                                "多账号轮次已停止",
                            )
                        }
                        market_runtime::MarketRunError::System { step, message } => {
                            round_runner::AccountRunError::system(step, message)
                        }
                    })?;
                Ok(round_account::MarketSessionResult::Completed)
            }
            market_runtime::MarketRunStop::EmergencyStopped => Err(
                round_runner::AccountRunError::system("round.stopped", "多账号轮次已停止"),
            ),
            market_runtime::MarketRunStop::SystemFailure { step, message } => {
                Err(round_runner::AccountRunError::system(step, message))
            }
        }
    }
}

impl round_runner::RoundDriver for ProductionRoundDriver {
    async fn run_account(
        &self,
        index: usize,
        total: usize,
        task: &round_planner::AccountRoundTask,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<round_runner::AccountRunSuccess, round_runner::AccountRunError> {
        crate::log_info!(
            "special_ops::round",
            "账号开始轮换",
            "account_id" => &task.account_id,
            "qq_account" => &task.qq_account,
            "account_index" => index,
            "account_total" => total,
            "station_count" => task.stations.len(),
            "ammo_target_count" => task.ammo_target_ids.len()
        );
        let snapshot = self
            .runtime
            .update_round_progress(
                self.run_id,
                index,
                total,
                &task.account_id,
                &task.qq_account,
                None,
                0,
                task.stations.len(),
            )
            .map_err(|message| round_runner::AccountRunError::system("round.progress", message))?
            .ok_or_else(|| {
                round_runner::AccountRunError::system("round.progress", "多账号轮次运行状态已变化")
            })?;
        emit_run(&self.app, &snapshot);
        round_account::run_account_session(self, task, cancelled).await
    }

    async fn continue_account(
        &self,
        index: usize,
        total: usize,
        task: &round_planner::AccountRoundTask,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<round_runner::AccountRunSuccess, round_runner::AccountRunError> {
        self.focus_existing_game().await?;
        let snapshot = self
            .runtime
            .update_round_progress(
                self.run_id,
                index,
                total,
                &task.account_id,
                &task.qq_account,
                None,
                0,
                task.stations.len(),
            )
            .map_err(|message| round_runner::AccountRunError::system("round.progress", message))?
            .ok_or_else(|| {
                round_runner::AccountRunError::system("round.progress", "多账号轮次运行状态已变化")
            })?;
        emit_run(&self.app, &snapshot);
        round_account::run_task_in_session(self, task, cancelled).await
    }

    async fn wait_until(
        &self,
        index: usize,
        total: usize,
        task: &round_planner::AccountRoundTask,
        keep_session: bool,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), round_runner::AccountRunError> {
        let message = if keep_session {
            "保持当前账号在线，等待同账号下一任务"
        } else {
            "游戏已关闭，等待切换下一账号"
        };
        let snapshot = self
            .runtime
            .update_round_progress(
                self.run_id,
                index,
                total,
                &task.account_id,
                &task.qq_account,
                None,
                0,
                task.stations.len(),
            )
            .map_err(|message| round_runner::AccountRunError::system("round.progress", message))?
            .ok_or_else(|| {
                round_runner::AccountRunError::system("round.progress", "多账号轮次运行状态已变化")
            })?;
        emit_run(&self.app, &snapshot);
        if let Ok(Some(snapshot)) =
            self.runtime
                .update(self.run_id, LoginRunStatus::Waiting, None, message, None)
        {
            emit_run(&self.app, &snapshot);
        }
        while now_ms() < task.scheduled_at_ms
            && !cancelled.load(std::sync::atomic::Ordering::SeqCst)
            && !self.control.pause_requested()
        {
            let wall_before_ms = now_ms();
            let monotonic_before = std::time::Instant::now();
            let remaining_ms = task.scheduled_at_ms.saturating_sub(now_ms()).max(1) as u64;
            tokio::time::sleep(std::time::Duration::from_millis(remaining_ms.min(1_000))).await;
            let wall_elapsed_ms = now_ms().saturating_sub(wall_before_ms);
            let monotonic_elapsed_ms =
                monotonic_before.elapsed().as_millis().min(i64::MAX as u128) as i64;
            if wall_elapsed_ms.abs_diff(monotonic_elapsed_ms) > 60_000 {
                return Err(round_runner::AccountRunError::system(
                    "round.clockJump",
                    "检测到休眠或系统时间跳变",
                ));
            }
        }
        Ok(())
    }

    fn now_ms(&self) -> i64 {
        now_ms()
    }

    fn persist_account_failure(
        &self,
        task: &round_planner::AccountRoundTask,
        error: &round_runner::AccountRunError,
    ) -> Result<(), String> {
        self.persist_account_error(&task.account_id, error)
    }

    fn persist_limited_failure(
        &self,
        task: &round_planner::AccountRoundTask,
        message: &str,
    ) -> Result<(), String> {
        let account_id = task.account_id.clone();
        let cycle_id = task
            .limited_supply_cycle_id
            .clone()
            .ok_or_else(|| "限时商品补偿任务缺少周期".to_string())?;
        let message = message.to_string();
        let (_settings, revision) = self.coordinator.with_runtime_change(|| {
            let mut next = self
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            let account = next
                .accounts
                .iter_mut()
                .find(|account| account.id == account_id)
                .ok_or_else(|| "限时商品账号不存在".to_string())?;
            account.limited_supply = limited_supply::LimitedSupplyAccountState {
                cycle_id: Some(cycle_id),
                outcome: limited_supply::LimitedSupplyOutcome::Failed,
                checked_at_ms: Some(now_ms()),
                matched_region: None,
                matched_color: None,
                acknowledged: false,
                last_error: Some(message),
            };
            save_settings(&self.app, &next)?;
            *self
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
            Ok::<_, String>(next)
        })?;
        emit_state(&self.app, revision, now_ms());
        Ok(())
    }

    fn market_window_open(&self) -> bool {
        market_purchase::market_window_open(local_day_and_minute(now_ms()).1 as u16)
    }

    fn refresh_due_craft_tasks(&self) -> Result<Vec<round_planner::AccountRoundTask>, String> {
        let current_ms = now_ms();
        let settings = self
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        let mut refreshed = freeze_round_run(
            &settings,
            current_ms,
            round_planner::RoundTrigger::Scheduled,
            AmmoProfitGate::DisplayOnly,
            None,
        )?;
        let tasks = refreshed
            .plan
            .accounts
            .into_iter()
            .filter(|task| !task.stations.is_empty() && task.scheduled_at_ms <= current_ms)
            .collect::<Vec<_>>();
        let mut dynamic = self
            .dynamic_accounts
            .lock()
            .map_err(|_| "动态冻结配置状态已损坏".to_string())?;
        for task in &tasks {
            let key = (task.account_id.clone(), task.scheduled_at_ms);
            if let Some(frozen) = refreshed.accounts.remove(&key) {
                dynamic.insert(key, frozen);
            }
        }
        Ok(tasks)
    }

    async fn close_game(&self) -> Result<(), String> {
        use desktop_runtime::DesktopRuntime;
        let executable = self.game_executable_path.clone();
        tokio::task::spawn_blocking(move || {
            desktop_runtime::WindowsDesktopRuntime
                .terminate_exact(&executable, std::time::Duration::from_secs(10))
        })
        .await
        .map_err(|error| format!("关闭游戏任务失败: {error}"))?
    }

    fn pause_requested(&self) -> Result<bool, String> {
        Ok(self.control.pause_requested())
    }

    fn pause_preserves_game(&self) -> bool {
        self.control.preserve_game()
    }

    fn persist_paused(&self, reason: &str) -> Result<(), String> {
        self.persist_global_pause(reason)
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_ammo_worker(
    app: AppHandle,
    settings: Arc<Mutex<SpecialOpsSettings>>,
    coordinator: Arc<SettingsCoordinator>,
    runtime: Arc<login_runtime::LoginRuntime>,
    run_id: u64,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    account_id: String,
    entry: FrozenMilitarySupplyEntry,
    frozen: FrozenAmmoRun,
) {
    let entry_driver = ProductionAmmoDriver {
        app: app.clone(),
        settings: Arc::clone(&settings),
        coordinator: Arc::clone(&coordinator),
        runtime: Arc::clone(&runtime),
        run_id,
        account_id: account_id.clone(),
        day: frozen.day.clone(),
        game_executable_path: entry.game_executable_path,
        mouse_parking_region: entry.mouse_parking_region,
        targets: entry.targets,
    };
    let driver = ProductionAmmoDriver {
        app: app.clone(),
        settings: Arc::clone(&settings),
        coordinator: Arc::clone(&coordinator),
        runtime: Arc::clone(&runtime),
        run_id,
        account_id: account_id.clone(),
        day: frozen.day,
        game_executable_path: frozen.game_executable_path,
        mouse_parking_region: frozen.mouse_parking_region,
        targets: frozen.targets,
    };
    let result = match military_supply_runtime::enter_military_supply(
        &entry_driver,
        entry.config,
        Arc::clone(&cancelled),
    )
    .await
    {
        Ok(()) => match <ProductionAmmoDriver as ammo_runtime::AmmoDriver>::wait_and_click(
            &driver,
            "ammo.tacticalDepartment",
            Arc::clone(&cancelled),
        )
        .await
        {
            Ok(()) => {
                ammo_runtime::run_ammo_targets(&driver, &frozen.ammo_targets, cancelled).await
            }
            Err(error) => ammo_runtime::AmmoRunResult {
                stop: ammo_driver_error_to_stop(error, "ammo.tacticalDepartment"),
            },
        },
        Err(military_supply_runtime::MilitarySupplyEntryError::Cancelled) => {
            ammo_runtime::AmmoRunResult {
                stop: ammo_runtime::AmmoRunStop::EmergencyStopped,
            }
        }
        Err(military_supply_runtime::MilitarySupplyEntryError::Target { step, message })
        | Err(military_supply_runtime::MilitarySupplyEntryError::System { step, message }) => {
            ammo_runtime::AmmoRunResult {
                stop: ammo_runtime::AmmoRunStop::SystemFailure { step, message },
            }
        }
    };
    let stop_reason = runtime.stop_reason(run_id).ok().flatten();
    let entered_input = runtime.entered_input(run_id).unwrap_or(false);
    let persist_result = if stop_reason.is_some() {
        persist_ammo_stop_with(
            &runtime,
            run_id,
            &settings,
            &coordinator,
            &account_id,
            now_ms(),
            entered_input,
            |next| save_settings(&app, next),
        )
        .map(|updated| {
            if let Some((_settings, revision)) = updated {
                emit_state(&app, revision, now_ms());
            }
        })
    } else {
        match &result.stop {
            ammo_runtime::AmmoRunStop::Completed | ammo_runtime::AmmoRunStop::Isolated { .. } => {
                Ok(())
            }
            stop @ (ammo_runtime::AmmoRunStop::Uncertain { .. }
            | ammo_runtime::AmmoRunStop::SystemFailure { .. }) => {
                let (target_id, step, message) =
                    ammo_stop_failure_detail(stop).expect("子弹失败结果必须包含步骤和消息");
                let saved = persist_ammo_uncertain_with(
                    &settings,
                    &coordinator,
                    &account_id,
                    target_id,
                    step,
                    &message,
                    now_ms(),
                    |next| save_settings(&app, next),
                );
                match saved {
                    Ok((_settings, revision)) => {
                        emit_state(&app, revision, now_ms());
                        Err(format!("子弹兑换步骤 {step} 失败：{message}"))
                    }
                    Err(error) => Err(format!(
                        "子弹兑换步骤 {step} 失败：{message}；不确定状态保存失败：{error}"
                    )),
                }
            }
            ammo_runtime::AmmoRunStop::EmergencyStopped => {
                let saved = persist_ammo_uncertain_with(
                    &settings,
                    &coordinator,
                    &account_id,
                    None,
                    "ammo.emergencyStop",
                    "子弹兑换意外停止，账号状态需人工确认",
                    now_ms(),
                    |next| save_settings(&app, next),
                );
                match saved {
                    Ok((_settings, revision)) => {
                        emit_state(&app, revision, now_ms());
                        Err("子弹兑换意外停止".to_string())
                    }
                    Err(error) => Err(format!("子弹兑换意外停止；状态保存失败：{error}")),
                }
            }
        }
    };
    let (status, message) = if stop_reason.is_some() {
        (LoginRunStatus::Stopped, "子弹兑换试运行已停止".to_string())
    } else {
        match &result.stop {
            ammo_runtime::AmmoRunStop::Completed => {
                (LoginRunStatus::Succeeded, "子弹兑换试运行完成".to_string())
            }
            ammo_runtime::AmmoRunStop::Isolated {
                target_id, message, ..
            } => (
                LoginRunStatus::Failed,
                format!("子弹目标 {target_id} 触发账号隔离：{message}"),
            ),
            _ => (
                LoginRunStatus::Failed,
                persist_result
                    .as_ref()
                    .err()
                    .cloned()
                    .unwrap_or_else(|| "子弹兑换试运行失败".to_string()),
            ),
        }
    };
    if let Err(error) = cleanup_login_worker_after_persistence(&persist_result, || {
        cleanup_login_run(&app, &runtime, run_id, status, &message)
    }) {
        crate::log_error!(
            "special_ops::ammo",
            "子弹兑换持久化或清理失败",
            "error" => error
        );
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_limited_supply_worker(
    app: AppHandle,
    settings: Arc<Mutex<SpecialOpsSettings>>,
    coordinator: Arc<SettingsCoordinator>,
    runtime: Arc<login_runtime::LoginRuntime>,
    run_id: u64,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    account_id: String,
    entry: FrozenMilitarySupplyEntry,
    frozen: FrozenLimitedSupplyRun,
) {
    let entry_driver = ProductionAmmoDriver {
        app: app.clone(),
        settings: Arc::clone(&settings),
        coordinator: Arc::clone(&coordinator),
        runtime: Arc::clone(&runtime),
        run_id,
        account_id: account_id.clone(),
        day: frozen.cycle_id.clone(),
        game_executable_path: entry.game_executable_path,
        mouse_parking_region: entry.mouse_parking_region,
        targets: entry.targets,
    };
    let driver = ProductionLimitedSupplyDriver {
        input: ProductionAmmoDriver {
            app: app.clone(),
            settings,
            coordinator,
            runtime: Arc::clone(&runtime),
            run_id,
            account_id,
            day: frozen.cycle_id.clone(),
            game_executable_path: frozen.game_executable_path,
            mouse_parking_region: frozen.mouse_parking_region,
            targets: frozen.targets,
        },
        cycle_id: frozen.cycle_id,
        colors: frozen.colors,
        color_tolerances: frozen.color_tolerances,
        persist_result: false,
    };
    let result = match military_supply_runtime::enter_military_supply(
        &entry_driver,
        entry.config,
        Arc::clone(&cancelled),
    )
    .await
    {
        Ok(()) => match <ProductionLimitedSupplyDriver as limited_supply_runtime::LimitedSupplyDriver>::wait_and_click(
            &driver,
            "ammo.researchDepartment",
            Arc::clone(&cancelled),
        )
        .await
        {
            Ok(()) => limited_supply_runtime::run_limited_supply_branch(
                &driver,
                frozen.config,
                Arc::clone(&cancelled),
            )
            .await,
            Err(error) => limited_error_to_stop(error),
        },
        Err(military_supply_runtime::MilitarySupplyEntryError::Cancelled) => {
            limited_supply_runtime::LimitedRunStop::EmergencyStopped
        }
        Err(military_supply_runtime::MilitarySupplyEntryError::Target { step, message })
        | Err(military_supply_runtime::MilitarySupplyEntryError::System { step, message }) => {
            limited_supply_runtime::LimitedRunStop::SystemFailure { step, message }
        }
    };
    let stop_reason = runtime.stop_reason(run_id).ok().flatten();
    if stop_reason.is_none()
        && !matches!(
            &result,
            limited_supply_runtime::LimitedRunStop::EmergencyStopped
        )
    {
        if let Err(error) = driver.input.park_mouse(Arc::clone(&cancelled)).await {
            crate::log_error!(
                "special_ops::limited",
                "限时商品试运行结束停放鼠标失败",
                "error" => error
            );
        }
    }
    let (status, message) = if stop_reason.is_some() {
        (LoginRunStatus::Stopped, "限时商品试运行已停止".to_string())
    } else {
        match result {
            limited_supply_runtime::LimitedRunStop::Completed(result) => (
                LoginRunStatus::Succeeded,
                format!("限时商品试运行完成：{:?}", result.outcome),
            ),
            limited_supply_runtime::LimitedRunStop::RetryableReadyTimeout => {
                (LoginRunStatus::Failed, "限时商品页面就绪超时".to_string())
            }
            limited_supply_runtime::LimitedRunStop::EmergencyStopped => (
                LoginRunStatus::Stopped,
                "限时商品试运行已紧急停止".to_string(),
            ),
            limited_supply_runtime::LimitedRunStop::SystemFailure { step, message } => (
                LoginRunStatus::Failed,
                format!("限时商品试运行步骤 {step} 失败：{message}"),
            ),
        }
    };
    let persist_result: Result<(), String> = Ok(());
    if let Err(error) = cleanup_login_worker_after_persistence(&persist_result, || {
        cleanup_login_run(&app, &runtime, run_id, status, &message)
    }) {
        crate::log_error!("special_ops::limited", "限时商品试运行清理失败", "error" => error);
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_market_trial_worker(
    app: AppHandle,
    settings: Arc<Mutex<SpecialOpsSettings>>,
    coordinator: Arc<SettingsCoordinator>,
    runtime: Arc<login_runtime::LoginRuntime>,
    run_id: u64,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    account_id: String,
    mut frozen: FrozenMarketRun,
    mode: market_runtime::MarketTrialMode,
) {
    frozen.config.target_count = 1;
    frozen.config.completed_count = 0;
    let driver = ProductionMarketDriver {
        input: ProductionAmmoDriver {
            app: app.clone(),
            settings,
            coordinator,
            runtime: Arc::clone(&runtime),
            run_id,
            account_id,
            day: frozen.day.clone(),
            game_executable_path: frozen.game_executable_path,
            mouse_parking_region: frozen.mouse_parking_region,
            targets: frozen.targets,
        },
        control: Arc::new(RoundControl::default()),
        day: frozen.day,
        persist_official: false,
        trial_completed: std::sync::atomic::AtomicU32::new(0),
    };
    let result =
        market_runtime::run_market_trial(&driver, &frozen.config, mode, Arc::clone(&cancelled))
            .await;
    let stop_reason = runtime.stop_reason(run_id).ok().flatten();
    if stop_reason.is_none() && !matches!(&result, Err(market_runtime::MarketRunError::Cancelled)) {
        if let Err(error) = driver.input.park_mouse(Arc::clone(&cancelled)).await {
            crate::log_error!(
                "special_ops::market",
                "交易行试运行结束停放鼠标失败",
                "error" => error
            );
        }
    }
    let (status, message) = if stop_reason.is_some() {
        (LoginRunStatus::Stopped, "交易行试运行已停止".to_string())
    } else {
        match result {
            Ok(result) => (
                LoginRunStatus::Succeeded,
                format!(
                    "交易行试运行完成：价格 {}，上限 {}，分支 {:?}",
                    result.raw_text, result.max_price, result.action
                ),
            ),
            Err(market_runtime::MarketRunError::Cancelled) => (
                LoginRunStatus::Stopped,
                "交易行试运行已紧急停止".to_string(),
            ),
            Err(market_runtime::MarketRunError::System { step, message }) => (
                LoginRunStatus::Failed,
                format!("交易行试运行步骤 {step} 失败：{message}"),
            ),
        }
    };
    let persist_result: Result<(), String> = Ok(());
    if let Err(error) = cleanup_login_worker_after_persistence(&persist_result, || {
        cleanup_login_run(&app, &runtime, run_id, status, &message)
    }) {
        crate::log_error!("special_ops::market", "交易行试运行清理失败", "error" => error);
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_craft_batch_worker(
    app: AppHandle,
    settings: Arc<Mutex<SpecialOpsSettings>>,
    coordinator: Arc<SettingsCoordinator>,
    runtime: Arc<login_runtime::LoginRuntime>,
    run_id: u64,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    account_id: String,
    frozen: Vec<FrozenCraftBatchTask>,
) {
    crate::log_debug!(
        "special_ops::craft",
        "制作批处理 worker 开始",
        "run_id" => run_id,
        "task_count" => frozen.len()
    );
    let tasks = frozen
        .iter()
        .map(|frozen| frozen.task.clone())
        .collect::<Vec<_>>();
    let driver = ProductionCraftBatchDriver {
        app: app.clone(),
        settings: Arc::clone(&settings),
        coordinator: Arc::clone(&coordinator),
        runtime: Arc::clone(&runtime),
        run_id,
        account_id: account_id.clone(),
        frozen: Arc::new(frozen),
        round_progress: None,
    };
    let result = craft_batch::run_craft_batch(&driver, &tasks, cancelled).await;
    let stop_reason = runtime.stop_reason(run_id).ok().flatten();
    let persist_result = match stop_reason {
        None => match &result {
            Ok(_) => Ok(()),
            Err(failure) if failure.failure.is_isolated() => {
                let failure_message = format!(
                    "制作批处理步骤 {} 失败：{}",
                    failure.failure.step, failure.failure.message
                );
                match persist_craft_failure_isolated_with(
                    &settings,
                    &coordinator,
                    &account_id,
                    now_ms(),
                    &failure.failure.step,
                    &failure.failure.message,
                    |next| save_settings(&app, next),
                ) {
                    Ok((_settings, revision)) => {
                        emit_state(&app, revision, now_ms());
                        Err(failure_message)
                    }
                    Err(error) => Err(format!(
                        "{failure_message}；隔离状态保存失败，必须人工确认：{error}"
                    )),
                }
            }
            Err(failure) if failure.failure.requires_uncertain => {
                let failure_message = format!(
                    "制作批处理步骤 {} 失败：{}",
                    failure.failure.step, failure.failure.message
                );
                match persist_craft_failure_uncertain_with(
                    &settings,
                    &coordinator,
                    &account_id,
                    &failure.station,
                    now_ms(),
                    &failure.failure.step,
                    &failure.failure.message,
                    |next| save_settings(&app, next),
                ) {
                    Ok((_settings, revision)) => {
                        emit_state(&app, revision, now_ms());
                        Err(failure_message)
                    }
                    Err(error) => Err(format!(
                        "{failure_message}；不确定状态保存失败，必须人工确认：{error}"
                    )),
                }
            }
            Err(failure) => Err(format!(
                "制作批处理步骤 {} 失败：{}",
                failure.failure.step, failure.failure.message
            )),
        },
        Some(_) => {
            let (station, entered_input) = result
                .as_ref()
                .err()
                .map(batch_stop_context)
                .or_else(|| {
                    tasks.last().map(|task| {
                        (
                            task.station.clone(),
                            runtime.entered_input(run_id).unwrap_or(false),
                        )
                    })
                })
                .unwrap_or((StationKind::TechnicalCenter, false));
            persist_craft_stop_with(
                &runtime,
                run_id,
                &settings,
                &coordinator,
                &account_id,
                &station,
                now_ms(),
                entered_input,
                |next| save_settings(&app, next),
            )
            .map(|updated| {
                if let Some((_settings, revision)) = updated {
                    emit_state(&app, revision, now_ms());
                }
            })
        }
    };
    let (status, message) = if stop_reason.is_some() {
        (LoginRunStatus::Stopped, "制作批处理已停止".to_string())
    } else if let Err(error) = &persist_result {
        (LoginRunStatus::Failed, error.clone())
    } else {
        let processed = result.as_ref().map_or(0, |success| success.processed);
        (
            LoginRunStatus::Succeeded,
            format!("制作批处理完成，共处理 {processed} 个到期制作台"),
        )
    };
    if let Err(error) = cleanup_login_worker_after_persistence(&persist_result, || {
        cleanup_login_run(&app, &runtime, run_id, status, &message)
    }) {
        crate::log_error!(
            "special_ops::craft",
            "制作批处理持久化或清理失败",
            "error" => error
        );
    }
    crate::log_debug!(
        "special_ops::craft",
        "制作批处理 worker 结束",
        "run_id" => run_id
    );
}

fn persist_round_stop_state(
    app: &AppHandle,
    settings: &Mutex<SpecialOpsSettings>,
    coordinator: &SettingsCoordinator,
    account_id: &str,
    station: Option<StationKind>,
    entered_input: bool,
) -> Result<(), String> {
    let (_next, revision) = coordinator.with_runtime_change(|| {
        let mut next = settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        next.paused = true;
        if entered_input {
            let error = station.map_or_else(
                || {
                    round_runner::AccountRunError::account(
                        "round.stop",
                        "多账号轮次停止时已执行键鼠输入，账号状态需人工确认",
                    )
                },
                |station| {
                    round_runner::AccountRunError::account_station(
                        station,
                        "round.stop",
                        "多账号轮次停止时已执行键鼠输入，制作状态需人工确认",
                    )
                },
            );
            apply_round_account_failure(&mut next, account_id, &error, now_ms())?;
        }
        save_settings(app, &next)?;
        *settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok::<_, String>(next)
    })?;
    emit_state(app, revision, now_ms());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_round_worker(
    app: AppHandle,
    settings: Arc<Mutex<SpecialOpsSettings>>,
    coordinator: Arc<SettingsCoordinator>,
    runtime: Arc<login_runtime::LoginRuntime>,
    control: Arc<RoundControl>,
    scheduler: Arc<round_scheduler::RoundScheduler>,
    run_id: u64,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    frozen: FrozenRoundRun,
) {
    let fallback_account_id = frozen
        .plan
        .accounts
        .first()
        .map(|task| task.account_id.clone())
        .unwrap_or_default();
    let driver = ProductionRoundDriver {
        app: app.clone(),
        settings: Arc::clone(&settings),
        coordinator: Arc::clone(&coordinator),
        runtime: Arc::clone(&runtime),
        control: Arc::clone(&control),
        run_id,
        accounts: frozen.accounts,
        dynamic_accounts: Mutex::new(std::collections::HashMap::new()),
        game_executable_path: frozen.game_executable_path,
    };
    let result = round_runner::run_round(&driver, &frozen.plan, cancelled).await;
    let stop_reason = runtime.stop_reason(run_id).ok().flatten();
    let active_snapshot = runtime.snapshot().ok().flatten();
    let active_account_id = active_snapshot
        .as_ref()
        .map(|snapshot| snapshot.account_id.clone())
        .unwrap_or(fallback_account_id);
    let active_station = active_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.round_progress.as_ref())
        .and_then(|progress| progress.station_kind.clone());
    let entered_input = runtime.entered_input(run_id).unwrap_or(false);
    let flow_result = login_flow::LoginFlowResult::GameReady {
        account_id: active_account_id.clone(),
        qq_account: String::new(),
        game_process_id: 0,
        game_window_handle: 0,
    };
    let persist_result = persist_login_outcome_with(
        &runtime,
        run_id,
        &active_account_id,
        &flow_result,
        "",
        |_, _, _| {
            if stop_reason.is_some() {
                persist_round_stop_state(
                    &app,
                    &settings,
                    &coordinator,
                    &active_account_id,
                    active_station.clone(),
                    entered_input,
                )
            } else {
                Ok(())
            }
        },
    )
    .map(|_| ());
    let (status, message) = match (&result.stop, stop_reason) {
        (_, Some(_)) | (round_runner::RoundStop::EmergencyStopped, _) => {
            (LoginRunStatus::Stopped, "多账号自动轮次已停止".to_string())
        }
        (round_runner::RoundStop::Completed, None) => (
            LoginRunStatus::Succeeded,
            format!(
                "多账号自动轮次完成，共完成 {} 个账号",
                result.completed_accounts
            ),
        ),
        (round_runner::RoundStop::PauseRequested, None) => (
            LoginRunStatus::Succeeded,
            "当前账号已完成，多账号自动轮次已暂停".to_string(),
        ),
        (round_runner::RoundStop::PauseRequestedPreservingGame, None) => (
            LoginRunStatus::Succeeded,
            "检测到休眠或系统暂停，已保留游戏现场".to_string(),
        ),
        (round_runner::RoundStop::SystemFailure { step, message }, None) => (
            LoginRunStatus::Failed,
            format!("多账号自动轮次步骤 {step} 失败：{message}"),
        ),
    };
    if let Err(error) = cleanup_login_worker_after_persistence(&persist_result, || {
        cleanup_login_run(&app, &runtime, run_id, status, &message)
    }) {
        crate::log_error!("special_ops::round", "多账号自动轮次持久化或清理失败", "error" => error);
    }
    if let Some(state) = app.try_state::<SpecialOpsState>() {
        let reason = match &result.stop {
            round_runner::RoundStop::Completed => "轮次已完成",
            round_runner::RoundStop::PauseRequested => "轮次已暂停",
            round_runner::RoundStop::PauseRequestedPreservingGame => "轮次已暂停并保留游戏现场",
            round_runner::RoundStop::EmergencyStopped => "轮次已紧急停止",
            round_runner::RoundStop::SystemFailure { .. } => "轮次发生系统错误",
        };
        state.profit_runtime.end_active_round(reason);
    }
    control.clear_pause_request();
    scheduler.wake();
}

#[allow(clippy::too_many_arguments)]
async fn run_craft_worker(
    app: AppHandle,
    settings: Arc<Mutex<SpecialOpsSettings>>,
    coordinator: Arc<SettingsCoordinator>,
    runtime: Arc<login_runtime::LoginRuntime>,
    run_id: u64,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    config: Arc<craft_runtime::CraftRunConfig>,
    station: StationKind,
    duration_minutes: u32,
) {
    crate::log_debug!(
        "special_ops::craft",
        "制作 worker 开始",
        "run_id" => run_id,
        "station" => format!("{station:?}")
    );
    let driver = craft_runtime::ProductionCraftTrialDriver::new(
        app.clone(),
        Arc::clone(&runtime),
        run_id,
        config.game_executable_path.clone(),
        config.mouse_parking_region.clone(),
        config.targets.clone(),
    );
    let result = craft_runtime::run_craft_station(
        &driver,
        station.clone(),
        config.delays,
        Arc::clone(&cancelled),
    )
    .await;
    let stop_reason = runtime.stop_reason(run_id).ok().flatten();
    let entered_input = runtime.entered_input(run_id).unwrap_or(false);
    let active_account_id = || {
        runtime.snapshot().and_then(|snapshot| {
            snapshot
                .filter(|snapshot| snapshot.run_id == run_id)
                .map(|snapshot| snapshot.account_id)
                .ok_or_else(|| "制作运行状态已丢失".to_string())
        })
    };
    let mut success_message = "制作试运行成功，已保存下一次完成时间".to_string();
    let persist_result = match stop_reason {
        None => match decide_craft_persistence(result) {
            CraftPersistenceDecision::NoChange => {
                success_message = "当前制作尚未完成，本次未执行点击".to_string();
                Ok(())
            }
            CraftPersistenceDecision::SaveStarted { started_at_ms } => active_account_id()
                .and_then(|account_id| {
                    persist_craft_success_with(
                        &settings,
                        &coordinator,
                        &account_id,
                        &station,
                        started_at_ms,
                        duration_minutes,
                        |next| save_settings(&app, next),
                    )
                })
                .map(|(_settings, revision)| {
                    emit_state(&app, revision, now_ms());
                })
                .map_err(|error| {
                    format!("制作已开始但完成时间保存失败，必须人工确认并修正完成时间：{error}")
                }),
            CraftPersistenceDecision::MarkIsolated { step, message } => {
                let failure_message = format!("制作试运行步骤 {step} 失败：{message}");
                match active_account_id().and_then(|account_id| {
                    persist_craft_failure_isolated_with(
                        &settings,
                        &coordinator,
                        &account_id,
                        now_ms(),
                        &step,
                        &message,
                        |next| save_settings(&app, next),
                    )
                }) {
                    Ok((_settings, revision)) => {
                        emit_state(&app, revision, now_ms());
                        Err(failure_message)
                    }
                    Err(error) => Err(format!(
                        "{failure_message}；隔离状态保存失败，必须人工确认：{error}"
                    )),
                }
            }
            CraftPersistenceDecision::MarkUncertain { step, message } => {
                let failure_message = format!("制作试运行步骤 {step} 失败：{message}");
                match active_account_id().and_then(|account_id| {
                    persist_craft_failure_uncertain_with(
                        &settings,
                        &coordinator,
                        &account_id,
                        &station,
                        now_ms(),
                        &step,
                        &message,
                        |next| save_settings(&app, next),
                    )
                }) {
                    Ok((_settings, revision)) => {
                        emit_state(&app, revision, now_ms());
                        Err(failure_message)
                    }
                    Err(error) => Err(format!(
                        "{failure_message}；不确定状态保存失败，必须人工确认：{error}"
                    )),
                }
            }
            CraftPersistenceDecision::FailWithoutChange { step, message } => {
                Err(format!("制作试运行步骤 {step} 失败：{message}"))
            }
        },
        Some(_) => {
            let account_id = active_account_id();
            account_id.and_then(|account_id| {
                persist_craft_stop_with(
                    &runtime,
                    run_id,
                    &settings,
                    &coordinator,
                    &account_id,
                    &station,
                    now_ms(),
                    entered_input,
                    |next| save_settings(&app, next),
                )
                .map(|updated| {
                    if let Some((_settings, revision)) = updated {
                        emit_state(&app, revision, now_ms());
                    }
                })
            })
        }
    };
    let (status, message) = if stop_reason.is_some() {
        (LoginRunStatus::Stopped, "制作试运行已停止".to_string())
    } else if let Err(error) = &persist_result {
        (LoginRunStatus::Failed, error.clone())
    } else {
        (LoginRunStatus::Succeeded, success_message)
    };
    if let Err(error) = cleanup_login_worker_after_persistence(&persist_result, || {
        cleanup_login_run(&app, &runtime, run_id, status, &message)
    }) {
        crate::log_error!("special_ops::craft", "制作试运行持久化或清理失败", "error" => error);
    }
    crate::log_debug!(
        "special_ops::craft",
        "制作 worker 结束",
        "run_id" => run_id,
        "station" => format!("{station:?}")
    );
}

struct ProductionRoundSchedulerDriver {
    app: AppHandle,
}

fn global_automation_enabled(app: &AppHandle) -> bool {
    app.try_state::<crate::global_state::GlobalState>()
        .map(|state| state.enabled())
        .unwrap_or(true)
}

async fn execute_cutoff_profit_query_action(app: &AppHandle) -> Result<(), String> {
    let state = app
        .try_state::<SpecialOpsState>()
        .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
    let coordinator = app
        .try_state::<Arc<SettingsCoordinator>>()
        .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
    let settings_lock = Arc::clone(&state.settings);
    let profit_runtime = Arc::clone(&state.profit_runtime);
    let login_runtime = Arc::clone(&state.login_runtime);
    let scheduler = Arc::clone(&state.round_scheduler);
    let initial_settings = settings_lock
        .lock()
        .map_err(|_| "特勤处状态已损坏".to_string())?
        .clone();
    let initial_revision = coordinator.current_revision()?;
    let started_at_ms = now_ms();
    if !initial_settings.enabled
        || initial_settings.paused
        || !global_automation_enabled(app)
        || !scheduler.is_armed()
        || login_runtime.snapshot()?.is_some()
    {
        return Ok(());
    }
    let initial_window =
        build_profit_query_window(&initial_settings, started_at_ms, initial_revision, false)?;
    if started_at_ms < initial_window.cutoff_at_ms {
        return Ok(());
    }

    let (settings, settings_revision) =
        if cutoff_state_for_day(&initial_settings, &initial_window.day).is_none() {
            let ((next, _), revision) = coordinator.with_expected_revision_change(
                initial_revision,
                || -> Result<(SpecialOpsSettings, i64), String> {
                    let mut next = settings_lock
                        .lock()
                        .map_err(|_| "特勤处状态已损坏".to_string())?
                        .clone();
                    let state =
                        build_profit_cutoff_state(&next, &initial_window.day, started_at_ms);
                    let pending_rule_ids = state
                        .targets
                        .iter()
                        .filter(|target| target.decided_at_ms.is_none())
                        .filter_map(|target| target.rule_id.clone())
                        .collect::<std::collections::HashSet<_>>();
                    next.profit_filter.cutoff_state = Some(state);
                    for audit in &mut next.profit_filter.audits {
                        if audit.day == initial_window.day
                            && pending_rule_ids.contains(&audit.rule_id)
                        {
                            audit.next_query_at_ms = None;
                        }
                    }
                    normalize_profit_settings(&mut next.profit_filter)?;
                    save_settings(app, &next)?;
                    *settings_lock
                        .lock()
                        .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
                    Ok((next, now_ms()))
                },
            )?;
            emit_state(app, revision, started_at_ms);
            (next, revision)
        } else {
            (initial_settings, initial_revision)
        };

    let window = build_profit_query_window(&settings, started_at_ms, settings_revision, false)?;
    let snapshot = profit_runtime.sync_window(window.clone())?;
    if snapshot
        .next_query_at_ms
        .is_some_and(|next_query_at_ms| next_query_at_ms > started_at_ms)
    {
        return Ok(());
    }
    let pending_rule_ids = cutoff_pending_rule_ids(&settings, &window.day);
    let cutoff_rules = settings
        .profit_filter
        .rules
        .iter()
        .filter(|rule| pending_rule_ids.contains(&rule.id))
        .cloned()
        .map(|mut rule| {
            rule.minimum_profit = FINAL_MINIMUM_PROFIT;
            rule
        })
        .collect::<Vec<_>>();
    if cutoff_rules.is_empty() {
        return Ok(());
    }
    let attempt = if cutoff_retry_at_ms(&settings, &window.day).is_some() {
        2
    } else {
        1
    };
    let lease = match profit_runtime.begin_cutoff_query(
        &window.day,
        settings_revision,
        started_at_ms,
        cutoff_rules,
        attempt,
    ) {
        Ok(lease) => lease,
        Err(error) if error.contains("已有利润查询正在进行") => return Ok(()),
        Err(error) => return Err(error),
    };
    let kkrb = KkrbAdapter::new()?;
    let moligod = MoligodAdapter::new(app.clone());
    let mut outcome = query_profit_rules_with_cancel(
        &kkrb,
        &moligod,
        &lease.rules,
        &lease.query_context(),
        lease.cancellation(),
    )
    .await?;
    if lease.is_cancelled() || !profit_runtime.accepts(lease.generation) {
        return Ok(());
    }
    let completed_at_ms = now_ms();
    let classification = classify_cutoff_audits(&outcome.audits, attempt);
    let retry_at_ms = completed_at_ms.saturating_add(profit::cutoff::FINAL_RETRY_DELAY_MS);
    for audit in &mut outcome.audits {
        audit.threshold = FINAL_MINIMUM_PROFIT;
        audit.next_query_at_ms = classification
            .retry_rule_ids
            .contains(&audit.rule_id)
            .then_some(retry_at_ms);
    }
    let commit = coordinator.with_expected_revision_change(
        settings_revision,
        || -> Result<(SpecialOpsSettings, i64), String> {
            let mut next = settings_lock
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            replace_profit_audits(&mut next, outcome.audits.clone());
            let current_state = next
                .profit_filter
                .cutoff_state
                .as_mut()
                .filter(|state| state.day == lease.day)
                .ok_or_else(|| PROFIT_QUERY_STALE.to_string())?;
            for target in &mut current_state.targets {
                let Some(rule_id) = target.rule_id.as_deref() else {
                    continue;
                };
                if classification.qualified_rule_ids.contains(rule_id) {
                    target.skip_reason = None;
                    target.decided_at_ms = Some(completed_at_ms);
                } else if let Some(reason) = classification.skipped.get(rule_id) {
                    target.skip_reason = Some(*reason);
                    target.decided_at_ms = Some(completed_at_ms);
                }
            }
            normalize_profit_settings(&mut next.profit_filter)?;
            save_settings(app, &next)?;
            *settings_lock
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
            let _ = profit_runtime.complete_cutoff_query_at_revision(
                &lease,
                completed_at_ms,
                classification.qualified_rule_ids.clone(),
                outcome.summary.clone(),
                settings_revision.saturating_add(1),
                !classification.retry_rule_ids.is_empty(),
            )?;
            Ok((next, completed_at_ms))
        },
    );
    match commit {
        Ok(((_settings, current_ms), revision)) => {
            emit_state(app, revision, current_ms);
            scheduler.wake();
            Ok(())
        }
        Err(error) if is_stale_profit_query_error(&error) => {
            profit_runtime.invalidate("截止利润查询结果已过期");
            scheduler.wake();
            Ok(())
        }
        Err(error) => {
            profit_runtime.invalidate(&format!("截止利润审计保存失败：{error}"));
            Err(error)
        }
    }
}

async fn execute_profit_query_action(app: &AppHandle) -> Result<(), String> {
    let state = app
        .try_state::<SpecialOpsState>()
        .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
    let coordinator = app
        .try_state::<Arc<SettingsCoordinator>>()
        .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
    let settings_lock = Arc::clone(&state.settings);
    let profit_runtime = Arc::clone(&state.profit_runtime);
    let login_runtime = Arc::clone(&state.login_runtime);
    let scheduler = Arc::clone(&state.round_scheduler);
    let settings = settings_lock
        .lock()
        .map_err(|_| "特勤处状态已损坏".to_string())?
        .clone();
    let settings_revision = coordinator.current_revision()?;
    let started_at_ms = now_ms();
    if !settings.enabled
        || settings.paused
        || !global_automation_enabled(app)
        || !scheduler.is_armed()
        || login_runtime.snapshot()?.is_some()
    {
        return Ok(());
    }

    let window = build_profit_query_window(&settings, started_at_ms, settings_revision, false)?;
    let snapshot = profit_runtime.sync_window(window.clone())?;
    if started_at_ms >= window.cutoff_at_ms {
        return execute_cutoff_profit_query_action(app).await;
    }
    if started_at_ms < window.exchange_at_ms {
        return Ok(());
    }
    if snapshot
        .next_query_at_ms
        .is_some_and(|next_query_at_ms| next_query_at_ms > started_at_ms)
    {
        return Ok(());
    }
    let rules = collect_pending_profit_rules(&settings, &window.day);
    if rules.is_empty() {
        return Ok(());
    }

    let lease =
        match profit_runtime.begin_query(&window.day, settings_revision, started_at_ms, rules) {
            Ok(lease) => lease,
            Err(error) if error.contains("已有利润查询正在进行") => return Ok(()),
            Err(error) => return Err(error),
        };
    let kkrb = KkrbAdapter::new()?;
    let moligod = MoligodAdapter::new(app.clone());
    let query_result = query_profit_rules_with_cancel(
        &kkrb,
        &moligod,
        &lease.rules,
        &lease.query_context(),
        lease.cancellation(),
    )
    .await;
    if lease.is_cancelled() || !profit_runtime.accepts(lease.generation) {
        return Ok(());
    }
    let mut outcome = match query_result {
        Ok(outcome) => outcome,
        Err(error) if is_stale_profit_query_error(&error) => return Ok(()),
        Err(error) => return Err(error),
    };
    let completed_at_ms = now_ms();
    let next_query_at_ms = completed_at_ms.saturating_add(if lease.attempt >= 3 {
        PROFIT_QUERY_FIFTY_MINUTES_MS
    } else {
        PROFIT_QUERY_FIVE_MINUTES_MS
    });
    for audit in &mut outcome.audits {
        audit.next_query_at_ms = Some(next_query_at_ms);
    }

    let commit = coordinator.with_expected_revision_change(
        settings_revision,
        || -> Result<(SpecialOpsSettings, i64), String> {
            let mut next = settings_lock
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            let current_now_ms = now_ms();
            let current_window =
                build_profit_query_window(&next, current_now_ms, settings_revision, false)?;
            let current_pending = collect_pending_profit_rules(&next, &current_window.day)
                .into_iter()
                .map(|rule| rule.id)
                .collect::<std::collections::HashSet<_>>();
            let requested = lease
                .rules
                .iter()
                .map(|rule| rule.id.as_str())
                .collect::<std::collections::HashSet<_>>();
            if !profit_runtime.accepts(lease.generation)
                || lease.is_cancelled()
                || !next.enabled
                || next.paused
                || !global_automation_enabled(app)
                || !scheduler.is_armed()
                || login_runtime.snapshot()?.is_some()
                || current_window.day != lease.day
                || current_now_ms < current_window.exchange_at_ms
                || current_now_ms >= current_window.cutoff_at_ms
                || !requested
                    .iter()
                    .all(|rule_id| current_pending.contains(*rule_id))
            {
                return Err(PROFIT_QUERY_STALE.to_string());
            }
            replace_profit_audits(&mut next, outcome.audits.clone());
            normalize_profit_settings(&mut next.profit_filter)?;
            save_settings(app, &next)?;
            *settings_lock
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
            let _ = profit_runtime.complete_query_at_revision(
                &lease,
                completed_at_ms,
                outcome.qualified_rule_ids.clone(),
                outcome.summary.clone(),
                settings_revision.saturating_add(1),
            )?;
            Ok((next, current_now_ms))
        },
    );
    match commit {
        Ok(((_settings, current_ms), revision)) => {
            emit_state(app, revision, current_ms);
            scheduler.wake();
            Ok(())
        }
        Err(error) if is_stale_profit_query_error(&error) => {
            profit_runtime.invalidate("利润查询结果已过期");
            scheduler.wake();
            Ok(())
        }
        Err(error) => {
            profit_runtime.invalidate(&format!("利润审计保存失败：{error}"));
            Err(error)
        }
    }
}

impl round_scheduler::SchedulerDriver for ProductionRoundSchedulerDriver {
    fn poll(&self) -> Result<round_scheduler::SchedulerPoll, String> {
        let state = self
            .app
            .try_state::<SpecialOpsState>()
            .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
        let settings = state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        let coordinator = self
            .app
            .try_state::<Arc<SettingsCoordinator>>()
            .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
        let settings_revision = coordinator.current_revision()?;
        let globally_enabled = global_automation_enabled(&self.app);
        let now = now_ms();
        let active_run = state.login_runtime.snapshot()?.is_some();
        let mut query_at_ms = None;
        let mut profit_snapshot = None;
        let gate = if settings.profit_filter.enabled {
            let window = build_profit_query_window(&settings, now, settings_revision, active_run)?;
            let snapshot = state.profit_runtime.sync_window(window.clone())?;
            profit_snapshot = Some(snapshot.clone());
            if now >= window.cutoff_at_ms {
                let cutoff_state_exists = cutoff_state_for_day(&settings, &window.day).is_some();
                let cutoff_pending = cutoff_pending_rule_ids(&settings, &window.day);
                if settings.enabled
                    && globally_enabled
                    && !settings.paused
                    && !active_run
                    && (!cutoff_state_exists || !cutoff_pending.is_empty())
                {
                    query_at_ms = snapshot.next_query_at_ms;
                }
                AmmoProfitGate::QualifiedTargets(cutoff_qualified_targets(&settings, &window.day))
            } else {
                let pending_rules = collect_pending_profit_rules(&settings, &window.day);
                if settings.enabled
                    && globally_enabled
                    && !settings.paused
                    && !active_run
                    && !pending_rules.is_empty()
                {
                    query_at_ms = snapshot.next_query_at_ms;
                }
                AmmoProfitGate::Qualified(
                    snapshot
                        .qualified_rule_ids
                        .into_iter()
                        .collect::<std::collections::HashSet<_>>(),
                )
            }
        } else {
            AmmoProfitGate::Disabled
        };
        let schedule =
            build_schedule_with_profit_runtime(&settings, now, &gate, profit_snapshot.as_ref());
        Ok(round_scheduler::SchedulerPoll {
            now_ms: now,
            next_action: round_scheduler::choose_next_action(query_at_ms, schedule.next_wake_at_ms),
            active_run,
            enabled: settings.enabled && globally_enabled,
            paused: settings.paused,
        })
    }

    async fn execute_action(
        &self,
        action: round_scheduler::SchedulerAction,
    ) -> Result<round_scheduler::SchedulerActionOutcome, String> {
        match action {
            round_scheduler::SchedulerAction::QueryProfit => {
                crate::log_info!("special_ops::scheduler", "scheduler 开始查询利润");
                let result = execute_profit_query_action(&self.app).await;
                crate::log_info!(
                    "special_ops::scheduler",
                    "scheduler 利润查询结束",
                    "success" => result.is_ok()
                );
                result.map(|_| round_scheduler::SchedulerActionOutcome::Completed)
            }
            round_scheduler::SchedulerAction::LaunchRound => {
                crate::log_info!("special_ops::scheduler", "scheduler 开始启动到期轮次");
                let state = self
                    .app
                    .try_state::<SpecialOpsState>()
                    .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
                let coordinator = self
                    .app
                    .try_state::<Arc<SettingsCoordinator>>()
                    .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
                let result = start_due_round_with_revision(
                    &self.app,
                    &state,
                    &coordinator,
                    coordinator.current_revision()?,
                    round_planner::RoundTrigger::Scheduled,
                );
                crate::log_info!(
                    "special_ops::scheduler",
                    "scheduler 启动到期轮次结束",
                    "success" => result.is_ok()
                );
                match result {
                    Ok(_) => Ok(round_scheduler::SchedulerActionOutcome::Completed),
                    Err(error) if error == OPERATION_WINDOW_LOAD_TIMEOUT => {
                        crate::log_warn!(
                            "special_ops::scheduler",
                            "操作提示窗口加载超时，轮次将在一秒后重试"
                        );
                        Ok(round_scheduler::SchedulerActionOutcome::RetryAfter(
                            OPERATION_WINDOW_RETRY_DELAY,
                        ))
                    }
                    Err(error) if is_transient_round_launch_error(&error) => {
                        crate::log_warn!(
                            "special_ops::scheduler",
                            "到期轮次本次未启动，等待下一轮 poll",
                            "reason" => error.as_str()
                        );
                        Ok(round_scheduler::SchedulerActionOutcome::RetryAfter(
                            SCHEDULER_TRANSIENT_RETRY_DELAY,
                        ))
                    }
                    Err(error) => Err(error),
                }
            }
        }
    }

    fn pause_automation(&self, reason: &str) -> Result<(), String> {
        persist_scheduler_pause(&self.app, reason)
    }
}

fn persist_scheduler_pause(app: &AppHandle, reason: &str) -> Result<(), String> {
    let state = app
        .try_state::<SpecialOpsState>()
        .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
    let coordinator = app
        .try_state::<Arc<SettingsCoordinator>>()
        .ok_or_else(|| "配置写入协调器尚未初始化".to_string())?;
    let (_settings, revision) = coordinator.with_runtime_change(|| {
        let mut next = state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?
            .clone();
        next.paused = true;
        next.paused_reason = Some(reason.to_string());
        save_settings(app, &next)?;
        *state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
        Ok::<_, String>(next)
    })?;
    state
        .profit_runtime
        .invalidate("scheduler 已暂停，利润查询已取消");
    if state.login_runtime.snapshot()?.is_some() {
        state.round_control.request_system_pause();
    }
    state.round_scheduler.disarm();
    emit_state(app, revision, now_ms());
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    crate::log_warn!("special_ops::scheduler", "制作 scheduler 已暂停", "reason" => reason);
    Ok(())
}

pub fn initialize(app: &AppHandle) -> Result<SpecialOpsState, String> {
    let mut settings = load_settings(app)?;
    if !settings.paused {
        settings.paused = true;
        save_settings(app, &settings)?;
    }
    Ok(SpecialOpsState {
        settings: Arc::new(Mutex::new(settings)),
        login_runtime: Arc::new(login_runtime::LoginRuntime::default()),
        profit_runtime: Arc::new(ProfitQueryControl::default()),
        round_control: Arc::new(RoundControl::default()),
        round_scheduler: Arc::new(round_scheduler::RoundScheduler::default()),
    })
}

pub fn start_runtime(app: &AppHandle) -> Result<(), String> {
    let state = app
        .try_state::<SpecialOpsState>()
        .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
    let scheduler = Arc::clone(&state.round_scheduler);
    let driver = Arc::new(ProductionRoundSchedulerDriver { app: app.clone() });
    tauri::async_runtime::spawn(round_scheduler::run_scheduler(scheduler, driver));
    Ok(())
}

pub fn shutdown(app: &AppHandle) -> Result<(), String> {
    if let Some(state) = app.try_state::<SpecialOpsState>() {
        state.profit_runtime.invalidate("应用关闭，利润查询已取消");
        state.round_scheduler.shutdown();
    }
    stop_registered(app)
}

pub(crate) fn stop_registered(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<SpecialOpsState>() else {
        return Ok(());
    };
    state
        .profit_runtime
        .invalidate("运行资源释放，利润查询已取消");
    state.round_scheduler.disarm();
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
    let (_settings, revision) = coordinator.with_runtime_change(|| {
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
    emit_state(app, revision, now_ms());
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
        &state.profit_runtime,
    )?;
    Ok(bootstrap)
}

fn ensure_no_active_special_ops_run(runtime: &login_runtime::LoginRuntime) -> Result<(), String> {
    if runtime.snapshot()?.is_some() {
        return Err("特勤处试运行尚未完成清理".to_string());
    }
    Ok(())
}

fn should_defer_round_pause(active: Option<LoginRunKind>, paused: bool) -> bool {
    paused && active == Some(LoginRunKind::Round)
}

/// scheduler 启动轮次失败是否属于「等下一轮 poll 即可」而非需要全局暂停。
///
/// poll 与 `freeze_round_run` 的过滤条件不完全一致（利润 gate、business config、
/// 运行态在两次读取之间可能变化），因此 poll 认为到期、execute 却拿到空计划是正常竞态。
/// 这类错误一律全局暂停会让自动化在用户毫不知情时停摆 —— 这正是「为什么全局暂停了」的来源。
fn is_transient_round_launch_error(error: &str) -> bool {
    const TRANSIENT: [&str; 5] = [
        "当前没有到期制作或子弹任务",
        "特勤处当前处于暂停状态，请先点击继续",
        "特勤处总开关已关闭",
        "特勤处试运行尚未完成清理",
        "配置保存已陈旧",
    ];
    TRANSIENT.iter().any(|pattern| error.contains(pattern))
}

#[tauri::command]
pub async fn special_ops_start_login_trial(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    settings_revision: u64,
) -> Result<LoginRunSnapshot, AppError> {
    ensure_app_global_automation_enabled(&app)?;
    ensure_no_active_special_ops_run(&state.login_runtime)?;
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
                LoginRunKind::Login,
                || register_emergency_hotkey(&app, registered_hotkey),
                || hide_other_windows_for_special_ops(&app),
                || create_operation_window(&app, &operation_hotkey, LoginRunKind::Login),
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
pub async fn special_ops_start_navigation_trial(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    settings_revision: u64,
) -> Result<LoginRunSnapshot, AppError> {
    ensure_app_global_automation_enabled(&app)?;
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let runtime = Arc::clone(&state.login_runtime);
    let snapshot =
        settings_coordinator.with_revision(settings_revision, || -> Result<_, String> {
            let settings = state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            let config = Arc::new(freeze_navigation_run_config(
                &settings,
                &account_id,
                game_navigation::NavigationDestination::StationGrid,
            )?);
            let worker_app = app.clone();
            let worker_runtime = Arc::clone(&runtime);
            let worker_config = Arc::clone(&config);
            let registered_hotkey = settings.emergency_hotkey.clone();
            let operation_hotkey = settings.emergency_hotkey.clone();
            let (_, snapshot) = start_login_run_with_resources(
                &runtime,
                account_id.clone(),
                LoginRunKind::Navigation,
                || register_emergency_hotkey(&app, registered_hotkey),
                || hide_other_windows_for_special_ops(&app),
                || create_operation_window(&app, &operation_hotkey, LoginRunKind::Navigation),
                |snapshot| emit_run(&app, snapshot),
                || release_login_resources_unlocked(&app),
                |started| {
                    let run_id = started.run_id;
                    let cancelled = Arc::clone(&started.cancelled);
                    tauri::async_runtime::spawn(async move {
                        run_navigation_worker(
                            worker_app,
                            worker_runtime,
                            run_id,
                            cancelled,
                            worker_config,
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
pub async fn special_ops_start_craft_trial(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    station_kind: StationKind,
    settings_revision: u64,
) -> Result<LoginRunSnapshot, AppError> {
    ensure_app_global_automation_enabled(&app)?;
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let runtime = Arc::clone(&state.login_runtime);
    let settings_lock = Arc::clone(&state.settings);
    let snapshot =
        settings_coordinator.with_revision(settings_revision, || -> Result<_, String> {
            let settings = state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            let (config, duration_minutes) =
                freeze_craft_run_config(&settings, &account_id, station_kind.clone())?;
            let config = Arc::new(config);
            let worker_app = app.clone();
            let worker_runtime = Arc::clone(&runtime);
            let worker_coordinator = Arc::clone(&*settings_coordinator);
            let worker_settings = Arc::clone(&settings_lock);
            let worker_config = Arc::clone(&config);
            let registered_hotkey = settings.emergency_hotkey.clone();
            let operation_hotkey = settings.emergency_hotkey.clone();
            let (_, snapshot) = start_login_run_with_resources(
                &runtime,
                account_id.clone(),
                LoginRunKind::Craft,
                || register_emergency_hotkey(&app, registered_hotkey),
                || hide_other_windows_for_special_ops(&app),
                || create_operation_window(&app, &operation_hotkey, LoginRunKind::Craft),
                |snapshot| emit_run(&app, snapshot),
                || release_login_resources_unlocked(&app),
                |started| {
                    let run_id = started.run_id;
                    let cancelled = Arc::clone(&started.cancelled);
                    let station = station_kind.clone();
                    tauri::async_runtime::spawn(async move {
                        run_craft_worker(
                            worker_app,
                            worker_settings,
                            worker_coordinator,
                            worker_runtime,
                            run_id,
                            cancelled,
                            worker_config,
                            station,
                            duration_minutes,
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
pub async fn special_ops_start_craft_batch_trial(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    settings_revision: u64,
) -> Result<LoginRunSnapshot, AppError> {
    ensure_app_global_automation_enabled(&app)?;
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let runtime = Arc::clone(&state.login_runtime);
    let settings_lock = Arc::clone(&state.settings);
    let snapshot =
        settings_coordinator.with_revision(settings_revision, || -> Result<_, String> {
            let settings = state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            let frozen_now_ms = now_ms();
            let frozen =
                freeze_craft_batch_run_configs(&settings, &account_id, frozen_now_ms, None)?;
            let worker_app = app.clone();
            let worker_runtime = Arc::clone(&runtime);
            let worker_coordinator = Arc::clone(&*settings_coordinator);
            let worker_settings = Arc::clone(&settings_lock);
            let worker_account_id = account_id.clone();
            let registered_hotkey = settings.emergency_hotkey.clone();
            let operation_hotkey = settings.emergency_hotkey.clone();
            let (_, snapshot) = start_login_run_with_resources(
                &runtime,
                account_id.clone(),
                LoginRunKind::Craft,
                || register_emergency_hotkey(&app, registered_hotkey),
                || hide_other_windows_for_special_ops(&app),
                || create_operation_window(&app, &operation_hotkey, LoginRunKind::Craft),
                |snapshot| emit_run(&app, snapshot),
                || release_login_resources_unlocked(&app),
                |started| {
                    let run_id = started.run_id;
                    let cancelled = Arc::clone(&started.cancelled);
                    tauri::async_runtime::spawn(async move {
                        run_craft_batch_worker(
                            worker_app,
                            worker_settings,
                            worker_coordinator,
                            worker_runtime,
                            run_id,
                            cancelled,
                            worker_account_id,
                            frozen,
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
pub async fn special_ops_start_ammo_trial(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    settings_revision: u64,
) -> Result<LoginRunSnapshot, AppError> {
    ensure_app_global_automation_enabled(&app)?;
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let runtime = Arc::clone(&state.login_runtime);
    let settings_lock = Arc::clone(&state.settings);
    let snapshot =
        settings_coordinator.with_revision(settings_revision, || -> Result<_, String> {
            let settings = state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            let entry = freeze_military_supply_entry(&settings)?;
            let frozen = freeze_ammo_run(&settings, &account_id, now_ms(), None)?;
            let worker_app = app.clone();
            let worker_runtime = Arc::clone(&runtime);
            let worker_coordinator = Arc::clone(&*settings_coordinator);
            let worker_settings = Arc::clone(&settings_lock);
            let worker_account_id = account_id.clone();
            let registered_hotkey = settings.emergency_hotkey.clone();
            let operation_hotkey = settings.emergency_hotkey.clone();
            let (_, snapshot) = start_login_run_with_resources(
                &runtime,
                account_id.clone(),
                LoginRunKind::Ammo,
                || register_emergency_hotkey(&app, registered_hotkey),
                || hide_other_windows_for_special_ops(&app),
                || create_operation_window(&app, &operation_hotkey, LoginRunKind::Ammo),
                |snapshot| emit_run(&app, snapshot),
                || release_login_resources_unlocked(&app),
                |started| {
                    let run_id = started.run_id;
                    let cancelled = Arc::clone(&started.cancelled);
                    tauri::async_runtime::spawn(async move {
                        run_ammo_worker(
                            worker_app,
                            worker_settings,
                            worker_coordinator,
                            worker_runtime,
                            run_id,
                            cancelled,
                            worker_account_id,
                            entry,
                            frozen,
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
pub async fn special_ops_start_limited_supply_trial(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    settings_revision: u64,
) -> Result<LoginRunSnapshot, AppError> {
    ensure_app_global_automation_enabled(&app)?;
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let runtime = Arc::clone(&state.login_runtime);
    let settings_lock = Arc::clone(&state.settings);
    let snapshot =
        settings_coordinator.with_revision(settings_revision, || -> Result<_, String> {
            let settings = state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            let entry = freeze_military_supply_entry(&settings)?;
            let frozen = freeze_limited_supply_run(&settings, &account_id, "trial")?;
            let worker_app = app.clone();
            let worker_runtime = Arc::clone(&runtime);
            let worker_settings = Arc::clone(&settings_lock);
            let worker_coordinator = Arc::clone(&*settings_coordinator);
            let worker_account_id = account_id.clone();
            let registered_hotkey = settings.emergency_hotkey.clone();
            let operation_hotkey = settings.emergency_hotkey.clone();
            let (_, snapshot) = start_login_run_with_resources(
                &runtime,
                account_id.clone(),
                LoginRunKind::LimitedSupply,
                || register_emergency_hotkey(&app, registered_hotkey),
                || hide_other_windows_for_special_ops(&app),
                || create_operation_window(&app, &operation_hotkey, LoginRunKind::LimitedSupply),
                |snapshot| emit_run(&app, snapshot),
                || release_login_resources_unlocked(&app),
                |started| {
                    let run_id = started.run_id;
                    let cancelled = Arc::clone(&started.cancelled);
                    tauri::async_runtime::spawn(async move {
                        run_limited_supply_worker(
                            worker_app,
                            worker_settings,
                            worker_coordinator,
                            worker_runtime,
                            run_id,
                            cancelled,
                            worker_account_id,
                            entry,
                            frozen,
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
pub async fn special_ops_start_market_trial(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    mode: market_runtime::MarketTrialMode,
    settings_revision: u64,
) -> Result<LoginRunSnapshot, AppError> {
    ensure_app_global_automation_enabled(&app)?;
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let runtime = Arc::clone(&state.login_runtime);
    let settings_lock = Arc::clone(&state.settings);
    let snapshot =
        settings_coordinator.with_revision(settings_revision, || -> Result<_, String> {
            let settings = state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            let day = local_day_and_minute(now_ms()).0;
            let frozen = freeze_market_run(&settings, &account_id, &day)?;
            let worker_app = app.clone();
            let worker_runtime = Arc::clone(&runtime);
            let worker_settings = Arc::clone(&settings_lock);
            let worker_coordinator = Arc::clone(&*settings_coordinator);
            let worker_account_id = account_id.clone();
            let registered_hotkey = settings.emergency_hotkey.clone();
            let operation_hotkey = settings.emergency_hotkey.clone();
            let (_, snapshot) = start_login_run_with_resources(
                &runtime,
                account_id.clone(),
                LoginRunKind::Market,
                || register_emergency_hotkey(&app, registered_hotkey),
                || hide_other_windows_for_special_ops(&app),
                || create_operation_window(&app, &operation_hotkey, LoginRunKind::Market),
                |snapshot| emit_run(&app, snapshot),
                || release_login_resources_unlocked(&app),
                |started| {
                    let run_id = started.run_id;
                    let cancelled = Arc::clone(&started.cancelled);
                    tauri::async_runtime::spawn(async move {
                        run_market_trial_worker(
                            worker_app,
                            worker_settings,
                            worker_coordinator,
                            worker_runtime,
                            run_id,
                            cancelled,
                            worker_account_id,
                            frozen,
                            mode,
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

fn start_due_round_with_revision(
    app: &AppHandle,
    state: &SpecialOpsState,
    settings_coordinator: &Arc<SettingsCoordinator>,
    settings_revision: u64,
    trigger: round_planner::RoundTrigger,
) -> Result<LoginRunSnapshot, String> {
    crate::log_info!("special_ops::startup", "到期轮次启动校验开始");
    ensure_app_global_automation_enabled(app)?;
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let runtime = Arc::clone(&state.login_runtime);
    let settings_lock = Arc::clone(&state.settings);
    let control = Arc::clone(&state.round_control);
    let scheduler = Arc::clone(&state.round_scheduler);
    let snapshot =
        settings_coordinator.with_revision(settings_revision, || -> Result<_, String> {
            crate::log_info!("special_ops::startup", "到期轮次 revision 锁已获取");
            let settings = state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?
                .clone();
            if !settings.enabled {
                return Err("特勤处总开关已关闭".to_string());
            }
            if settings.paused {
                return Err("特勤处当前处于暂停状态，请先点击继续".to_string());
            }
            crate::log_info!("special_ops::startup", "开始冻结到期轮次计划");
            let frozen_now_ms = now_ms();
            let (profit_gate, profit_generation) = profit_gate_for_round(
                &settings,
                &state.profit_runtime,
                frozen_now_ms,
                settings_revision,
            )?;
            let frozen = freeze_round_run(
                &settings,
                frozen_now_ms,
                trigger,
                profit_gate,
                profit_generation,
            )?;
            let initial_account_id = frozen
                .plan
                .accounts
                .first()
                .map(|task| task.account_id.clone())
                .ok_or_else(|| "当前没有到期制作或子弹任务".to_string())?;
            let consumed_profit_generation = if let Some(generation) = frozen.profit_generation {
                let targets = frozen
                    .plan
                    .accounts
                    .iter()
                    .flat_map(|task| {
                        task.ammo_target_ids
                            .iter()
                            .map(|target_id| ProfitTargetKey {
                                account_id: task.account_id.clone(),
                                target_id: target_id.clone(),
                            })
                    })
                    .collect::<Vec<_>>();
                if !targets.is_empty() {
                    state
                        .profit_runtime
                        .consume_for_round(generation, targets)?;
                    Some(generation)
                } else {
                    None
                }
            } else {
                None
            };
            crate::log_info!(
                "special_ops::startup",
                "到期轮次计划冻结完成",
                "account_count" => frozen.plan.accounts.len()
            );
            control.clear_pause_request();
            let worker_app = app.clone();
            let worker_runtime = Arc::clone(&runtime);
            let worker_control = Arc::clone(&control);
            let worker_coordinator = Arc::clone(settings_coordinator);
            let worker_settings = Arc::clone(&settings_lock);
            let worker_scheduler = Arc::clone(&scheduler);
            let registered_hotkey = settings.emergency_hotkey.clone();
            let operation_hotkey = settings.emergency_hotkey.clone();
            crate::log_info!("special_ops::startup", "开始获取到期轮次运行资源");
            let start_result = start_login_run_with_resources(
                &runtime,
                initial_account_id.clone(),
                LoginRunKind::Round,
                || register_emergency_hotkey(app, registered_hotkey),
                || hide_other_windows_for_special_ops(app),
                || create_operation_window(app, &operation_hotkey, LoginRunKind::Round),
                |snapshot| emit_run(app, snapshot),
                || release_login_resources_unlocked(app),
                |started| {
                    let run_id = started.run_id;
                    let cancelled = Arc::clone(&started.cancelled);
                    tauri::async_runtime::spawn(async move {
                        run_round_worker(
                            worker_app,
                            worker_settings,
                            worker_coordinator,
                            worker_runtime,
                            worker_control,
                            worker_scheduler,
                            run_id,
                            cancelled,
                            frozen,
                        )
                        .await;
                    });
                    Ok(())
                },
            );
            let (_, snapshot) = match start_result {
                Ok(started) => started,
                Err(error) => {
                    if let Some(generation) = consumed_profit_generation {
                        if let Err(rollback_error) =
                            state.profit_runtime.rollback_failed_round_start(generation)
                        {
                            return Err(format!(
                                "{error}；轮次启动利润状态回滚失败：{rollback_error}"
                            ));
                        }
                    }
                    return Err(error);
                }
            };
            crate::log_info!("special_ops::startup", "到期轮次运行资源获取完成");
            Ok(snapshot)
        })?;
    crate::log_info!("special_ops::startup", "到期轮次启动校验结束");
    Ok(snapshot)
}

#[tauri::command]
pub async fn special_ops_start_due_round(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    settings_revision: u64,
) -> Result<LoginRunSnapshot, AppError> {
    let snapshot = start_due_round_with_revision(
        &app,
        &state,
        &settings_coordinator,
        settings_revision,
        round_planner::RoundTrigger::Manual,
    )
    .map_err(AppError::from)?;
    state.round_scheduler.arm();
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

fn request_emergency_stop_core(app: &AppHandle) -> Result<LoginRunSnapshot, String> {
    let state = app
        .try_state::<SpecialOpsState>()
        .ok_or_else(|| "特勤处状态尚未初始化".to_string())?;
    let active = state
        .login_runtime
        .snapshot()?
        .ok_or_else(|| "当前没有运行中的登录试运行".to_string())?;
    request_then_release_emergency(
        || {
            emit_login_run_change(app, &state.login_runtime, || {
                state
                    .login_runtime
                    .request_stop(active.run_id, login_runtime::StopReason::Emergency)
            })
            .map_err(|error| {
                fail_closed_login_error(app, &state.login_runtime, active.run_id, error)
            })?
            .ok_or_else(|| "登录试运行状态已变化".to_string())
        },
        crate::input_simulation::release_tracked_injected_inputs,
    )
}

#[tauri::command]
pub fn special_ops_emergency_stop(app: AppHandle) -> Result<LoginRunSnapshot, AppError> {
    request_emergency_stop_core(&app).map_err(AppError::from)
}

#[tauri::command]
pub fn special_ops_save_settings(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    settings_value: SpecialOpsSettings,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let mut settings_value = normalize_settings(settings_value)?;
    // paused / paused_reason 是运行态，只有 set_paused 与自动暂停能改。
    // 前端草稿可能是自动暂停之前的快照，直接采信会把 paused 回滚成旧值。
    {
        let current = state
            .settings
            .lock()
            .map_err(|_| "特勤处状态已损坏".to_string())?;
        settings_value.paused = current.paused;
        settings_value.paused_reason = current.paused_reason.clone();
    }
    let should_arm_scheduler = settings_value.enabled && !settings_value.paused;
    if settings_value.enabled && !settings_value.paused {
        validate_execution_ready(&settings_value)?;
    }
    let ((settings, current_ms), revision) = settings_coordinator
        .with_expected_revision_change(
            settings_revision,
            || -> Result<(SpecialOpsSettings, i64), String> {
                ensure_no_active_special_ops_run(&state.login_runtime)?;
                save_settings(&app, &settings_value)?;
                {
                    let mut settings = state
                        .settings
                        .lock()
                        .map_err(|_| "特勤处状态已损坏".to_string())?;
                    *settings = settings_value.clone();
                }
                Ok((settings_value, now_ms()))
            },
        )
        .map_err(AppError::from)?;
    state
        .profit_runtime
        .invalidate("配置已修改，利润查询已取消");
    let bootstrap = build_bootstrap_with_runtime(
        settings,
        revision,
        current_ms,
        &state.login_runtime,
        &state.profit_runtime,
    )?;
    emit_state(&app, bootstrap.settings_revision, bootstrap.now_ms);
    if should_arm_scheduler {
        state.round_scheduler.arm();
    } else {
        state.round_scheduler.disarm();
    }
    Ok(bootstrap)
}

#[tauri::command]
pub fn special_ops_save_profit_settings(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    update: ProfitConfigurationUpdate,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    let ((settings, current_ms), revision) = settings_coordinator
        .with_expected_revision_change(
            settings_revision,
            || -> Result<(SpecialOpsSettings, i64), String> {
                let current = state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?
                    .clone();
                let validated = state.profit_runtime.validated_moligod_names()?;
                let next = apply_profit_configuration(&current, update, &validated)?;
                save_settings(&app, &next)?;
                *state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
                Ok((next, now_ms()))
            },
        )
        .map_err(AppError::from)?;
    state
        .profit_runtime
        .invalidate("利润配置已修改，旧查询已取消");
    let bootstrap = build_bootstrap_with_runtime(
        settings,
        revision,
        current_ms,
        &state.login_runtime,
        &state.profit_runtime,
    )?;
    emit_state(&app, bootstrap.settings_revision, bootstrap.now_ms);
    state.round_scheduler.wake();
    Ok(bootstrap)
}

#[tauri::command]
pub async fn special_ops_fetch_profit_catalog() -> Result<ProfitCatalogSnapshot, AppError> {
    let adapter = KkrbAdapter::new().map_err(AppError::from)?;
    adapter
        .fetch_catalog_with_busy_retry()
        .await
        .map_err(|error| AppError::from(error.to_string()))
}

#[tauri::command]
pub async fn special_ops_validate_moligod_binding(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    exact_name: String,
) -> Result<MoligodBindingValidation, AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let exact_name = exact_name.trim().to_string();
    if exact_name.is_empty() {
        return Err(AppError::from("Moligod 精确名称不能为空"));
    }
    let generation = state.profit_runtime.generation();
    let adapter = MoligodAdapter::new(app);
    let snapshot = adapter
        .fetch(
            generation,
            vec![MoligodRequestTarget {
                rule_id: "binding-validation".to_string(),
                exact_name: exact_name.clone(),
            }],
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await
        .map_err(AppError::from)?;
    if snapshot.generation != generation || snapshot.results.len() != 1 {
        return Err(AppError::from("Moligod 绑定验证结果无效"));
    }
    let result = snapshot
        .results
        .into_iter()
        .next()
        .ok_or_else(|| AppError::from("Moligod 绑定验证缺少结果"))?;
    if result.rule_id != "binding-validation" || result.exact_name != exact_name {
        return Err(AppError::from("Moligod 精确名称未唯一命中"));
    }
    if result.status != MoligodRuleStatus::Matched {
        return Err(AppError::from(
            result
                .detail
                .unwrap_or_else(|| "Moligod 绑定验证失败".to_string()),
        ));
    }
    let profit = result
        .profit
        .ok_or_else(|| AppError::from("Moligod 绑定验证结果缺少利润"))?;
    state
        .profit_runtime
        .record_validated_moligod_name(exact_name.clone())?;
    Ok(MoligodBindingValidation { exact_name, profit })
}

#[tauri::command]
pub fn special_ops_confirm_account_station_states(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    stations: Vec<StationCorrectionInput>,
    ammo_targets: Vec<AmmoCorrectionInput>,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let ((settings, current_ms), revision) = settings_coordinator
        .with_expected_revision_change(
            settings_revision,
            || -> Result<(SpecialOpsSettings, i64), String> {
                ensure_no_active_special_ops_run(&state.login_runtime)?;
                let mut next = state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?
                    .clone();
                let confirmed_at_ms = now_ms();
                let current_day = local_day_and_minute(confirmed_at_ms).0;
                apply_manual_account_corrections(
                    &mut next,
                    &account_id,
                    &stations,
                    &ammo_targets,
                    confirmed_at_ms,
                    &current_day,
                )?;
                let next = normalize_settings(next)?;
                save_settings(&app, &next)?;
                *state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
                Ok((next, now_ms()))
            },
        )
        .map_err(AppError::from)?;
    state
        .profit_runtime
        .invalidate("账号状态已人工校正，旧查询已取消");
    let bootstrap = build_bootstrap_with_runtime(
        settings,
        revision,
        current_ms,
        &state.login_runtime,
        &state.profit_runtime,
    )?;
    emit_state(&app, bootstrap.settings_revision, bootstrap.now_ms);
    state.round_scheduler.wake();
    Ok(bootstrap)
}

#[tauri::command]
pub fn special_ops_confirm_account_manual_check(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let ((settings, current_ms), revision) = settings_coordinator
        .with_expected_revision_change(
            settings_revision,
            || -> Result<(SpecialOpsSettings, i64), String> {
                ensure_no_active_special_ops_run(&state.login_runtime)?;
                let mut next = state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?
                    .clone();
                apply_account_manual_check(&mut next, &account_id, now_ms())?;
                let next = normalize_settings(next)?;
                save_settings(&app, &next)?;
                *state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
                Ok((next, now_ms()))
            },
        )
        .map_err(AppError::from)?;
    state
        .profit_runtime
        .invalidate("账号已人工检查，旧查询已取消");
    let bootstrap = build_bootstrap_with_runtime(
        settings,
        revision,
        current_ms,
        &state.login_runtime,
        &state.profit_runtime,
    )?;
    emit_state(&app, bootstrap.settings_revision, bootstrap.now_ms);
    state.round_scheduler.wake();
    Ok(bootstrap)
}

/// 一键恢复状态：`account_id` 为空时恢复全部异常账号。
#[tauri::command]
pub fn special_ops_restore_account_state(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: Option<String>,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let ((settings, current_ms), revision) = settings_coordinator
        .with_expected_revision_change(
            settings_revision,
            || -> Result<(SpecialOpsSettings, i64), String> {
                ensure_no_active_special_ops_run(&state.login_runtime)?;
                let mut next = state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?
                    .clone();
                let confirmed_at_ms = now_ms();
                let current_day = local_day_and_minute(confirmed_at_ms).0;
                restore_account_state(
                    &mut next,
                    account_id.as_deref(),
                    confirmed_at_ms,
                    &current_day,
                )?;
                let next = normalize_settings(next)?;
                save_settings(&app, &next)?;
                *state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
                Ok((next, now_ms()))
            },
        )
        .map_err(AppError::from)?;
    state
        .profit_runtime
        .invalidate("账号状态已一键恢复，旧查询已取消");
    let bootstrap = build_bootstrap_with_runtime(
        settings,
        revision,
        current_ms,
        &state.login_runtime,
        &state.profit_runtime,
    )?;
    emit_state(&app, bootstrap.settings_revision, bootstrap.now_ms);
    state.round_scheduler.wake();
    Ok(bootstrap)
}

#[tauri::command]
pub fn special_ops_confirm_station_state(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    correction: StationCorrectionInput,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let ((settings, current_ms), revision) = settings_coordinator
        .with_expected_revision_change(
            settings_revision,
            || -> Result<(SpecialOpsSettings, i64), String> {
                ensure_no_active_special_ops_run(&state.login_runtime)?;
                let mut next = state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?
                    .clone();
                let confirmed_at_ms = now_ms();
                apply_single_station_correction(
                    &mut next,
                    &account_id,
                    &correction,
                    confirmed_at_ms,
                )?;
                let next = normalize_settings(next)?;
                save_settings(&app, &next)?;
                *state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
                Ok((next, now_ms()))
            },
        )
        .map_err(AppError::from)?;
    state
        .profit_runtime
        .invalidate("制作台状态已人工判定，旧查询已取消");
    let bootstrap = build_bootstrap_with_runtime(
        settings,
        revision,
        current_ms,
        &state.login_runtime,
        &state.profit_runtime,
    )?;
    emit_state(&app, bootstrap.settings_revision, bootstrap.now_ms);
    state.round_scheduler.wake();
    Ok(bootstrap)
}

#[tauri::command]
pub fn special_ops_confirm_ammo_state(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    correction: AmmoCorrectionInput,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let ((settings, current_ms), revision) = settings_coordinator
        .with_expected_revision_change(
            settings_revision,
            || -> Result<(SpecialOpsSettings, i64), String> {
                ensure_no_active_special_ops_run(&state.login_runtime)?;
                let mut next = state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?
                    .clone();
                let confirmed_at_ms = now_ms();
                let current_day = local_day_and_minute(confirmed_at_ms).0;
                apply_single_ammo_correction(&mut next, &account_id, &correction, &current_day)?;
                let next = normalize_settings(next)?;
                save_settings(&app, &next)?;
                *state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
                Ok((next, now_ms()))
            },
        )
        .map_err(AppError::from)?;
    state
        .profit_runtime
        .invalidate("子弹状态已人工判定，旧查询已取消");
    let bootstrap = build_bootstrap_with_runtime(
        settings,
        revision,
        current_ms,
        &state.login_runtime,
        &state.profit_runtime,
    )?;
    emit_state(&app, bootstrap.settings_revision, bootstrap.now_ms);
    state.round_scheduler.wake();
    Ok(bootstrap)
}

#[tauri::command]
pub fn special_ops_acknowledge_limited_supply(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    account_id: String,
    cycle_id: String,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let ((settings, current_ms), revision) = settings_coordinator
        .with_expected_revision_change(
            settings_revision,
            || -> Result<(SpecialOpsSettings, i64), String> {
                let mut next = state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?
                    .clone();
                let account = next
                    .accounts
                    .iter_mut()
                    .find(|account| account.id == account_id)
                    .ok_or_else(|| "限时商品账号不存在".to_string())?;
                if account.limited_supply.cycle_id.as_deref() != Some(cycle_id.as_str()) {
                    return Err("限时商品提醒周期已变化，请刷新后重试".to_string());
                }
                if account.limited_supply.outcome != limited_supply::LimitedSupplyOutcome::HighValue
                {
                    return Err("当前限时商品任务没有待确认高价值提醒".to_string());
                }
                account.limited_supply.acknowledged = true;
                save_settings(&app, &next)?;
                *state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())? = next.clone();
                Ok((next, now_ms()))
            },
        )
        .map_err(AppError::from)?;
    let bootstrap = build_bootstrap_with_runtime(
        settings,
        revision,
        current_ms,
        &state.login_runtime,
        &state.profit_runtime,
    )?;
    emit_state(&app, bootstrap.settings_revision, bootstrap.now_ms);
    state.round_scheduler.wake();
    Ok(bootstrap)
}

#[tauri::command]
pub async fn special_ops_set_paused(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    paused: bool,
    settings_revision: u64,
) -> Result<SpecialOpsBootstrap, AppError> {
    crate::log_info!(
        "special_ops::scheduler",
        "暂停状态切换开始",
        "paused" => paused,
        "settings_revision" => settings_revision
    );
    let active = state.login_runtime.snapshot()?;
    if should_defer_round_pause(active.as_ref().map(|snapshot| snapshot.run_kind), paused) {
        return settings_coordinator
            .with_revision(
                settings_revision,
                || -> Result<SpecialOpsBootstrap, String> {
                    state.round_control.request_pause();
                    if let Some(active) = active.as_ref() {
                        if let Some(snapshot) = state.login_runtime.update(
                            active.run_id,
                            LoginRunStatus::Waiting,
                            active.current_step,
                            "已请求暂停，当前账号完成后停止切换",
                            None,
                        )? {
                            emit_run(&app, &snapshot);
                        }
                    }
                    let settings = state
                        .settings
                        .lock()
                        .map_err(|_| "特勤处状态已损坏".to_string())?
                        .clone();
                    build_bootstrap_with_runtime(
                        settings,
                        settings_revision,
                        now_ms(),
                        &state.login_runtime,
                        &state.profit_runtime,
                    )
                },
            )
            .map_err(AppError::from);
    }
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let ((settings, current_ms), revision) = settings_coordinator
        .with_expected_revision_change(
            settings_revision,
            || -> Result<(SpecialOpsSettings, i64), String> {
                ensure_no_active_special_ops_run(&state.login_runtime)?;
                let mut settings = state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?
                    .clone();
                apply_paused_state(&mut settings, paused)?;
                if !paused {
                    state.round_control.clear_pause_request();
                }
                crate::log_info!(
                    "special_ops::scheduler",
                    "暂停状态校验完成",
                    "paused" => paused
                );
                save_settings(&app, &settings)?;
                crate::log_info!(
                    "special_ops::scheduler",
                    "暂停状态持久化完成",
                    "paused" => paused
                );
                *state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())? = settings.clone();
                Ok((settings, now_ms()))
            },
        )
        .map_err(AppError::from)?;
    state.profit_runtime.invalidate(if paused {
        "自动化已暂停，利润查询已取消"
    } else {
        "自动化恢复，重新建立利润查询 generation"
    });
    let bootstrap = build_bootstrap_with_runtime(
        settings,
        revision,
        current_ms,
        &state.login_runtime,
        &state.profit_runtime,
    )?;
    crate::log_info!(
        "special_ops::scheduler",
        "暂停状态 bootstrap 构建完成",
        "paused" => paused
    );
    emit_state(&app, bootstrap.settings_revision, bootstrap.now_ms);
    crate::log_info!(
        "special_ops::scheduler",
        "暂停状态事件发送完成",
        "paused" => paused
    );
    if paused {
        state.round_scheduler.disarm();
    } else {
        state.round_scheduler.resume();
    }
    crate::log_info!(
        "special_ops::scheduler",
        "scheduler armed 状态已更新",
        "paused" => paused,
        "armed" => state.round_scheduler.is_armed()
    );
    crate::log_info!(
        "special_ops::scheduler",
        "暂停状态切换结束",
        "paused" => paused,
        "success" => true
    );
    Ok(bootstrap)
}

#[tauri::command]
pub async fn special_ops_test_limited_supply_colors(
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    environment_id: String,
    region_index: u8,
    settings_revision: u64,
) -> Result<LimitedSupplyColorTestResult, AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    if !(1..=9).contains(&region_index) {
        return Err(AppError::from("限时商品识色区域编号必须为 1-9"));
    }
    let (region, colors, tolerances, game_path, parking) = settings_coordinator
        .with_revision(settings_revision, || -> Result<_, String> {
            let settings = state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())?;
            let environment = settings
                .calibration_environments
                .iter()
                .find(|environment| environment.id == environment_id)
                .ok_or_else(|| "显示环境不存在".to_string())?;
            let key = format!("limited.color.{region_index}");
            let target = environment
                .targets
                .iter()
                .find(|target| target.key == key)
                .ok_or_else(|| format!("未找到校准目标 {key}"))?;
            let rect = target
                .rect
                .clone()
                .ok_or_else(|| format!("校准目标 {key} 尚未配置"))?;
            let game_path = std::fs::canonicalize(settings.game_executable_path.trim())
                .map_err(|_| "游戏 exe 路径无效".to_string())?;
            Ok((
                crate::morse::types::RegionRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                },
                settings.limited_supply.colors,
                settings.limited_supply.color_tolerances,
                game_path,
                mouse_parking_region(&settings)?,
            ))
        })
        .map_err(AppError::from)?;
    tokio::task::spawn_blocking(move || {
        use desktop_runtime::DesktopRuntime;
        let desktop = desktop_runtime::WindowsDesktopRuntime;
        let window = desktop
            .find_primary_window(&game_path)?
            .ok_or_else(|| "未找到游戏窗口".to_string())?;
        desktop.restore_and_focus(&game_path, window)?;
        Ok::<_, String>(())
    })
    .await
    .map_err(|error| AppError::from(format!("游戏窗口任务失败：{error}")))??;
    crate::input_simulation::move_region_center_cancellable(
        parking,
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
    .await
    .map_err(AppError::from)?;
    let sample = |region: crate::morse::types::RegionRect| async move {
        tokio::task::spawn_blocking(move || {
            let screenshot = crate::recognition::watcher::capture_region(&region)
                .ok_or_else(|| "识色区域截图失败".to_string())?;
            let probe = crate::recognition::ColorProbe {
                region: None,
                targets: colors
                    .into_iter()
                    .zip(tolerances)
                    .map(|(color, tolerance)| crate::recognition::ColorTarget { color, tolerance })
                    .collect(),
                probe_match_mode: crate::recognition::ColorMatchMode::Any,
                legacy_target_color: None,
                legacy_tolerance: None,
            };
            Ok::<_, String>(crate::recognition::watcher::probe_hit(
                &screenshot,
                &probe,
                crate::recognition::ColorMatchMethod::AnyPixel,
                true,
            ))
        })
        .await
        .map_err(|error| format!("识色截图任务失败：{error}"))?
    };
    let first = sample(region.clone()).await.map_err(AppError::from)?;
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    let second = sample(region).await.map_err(AppError::from)?;
    Ok(LimitedSupplyColorTestResult {
        first_sampled_color: first.sampled_color,
        first_matched_color: first.matched.then_some(first.target_color),
        first_target_color: first.target_color,
        first_tolerance: first.tolerance,
        first_matched: first.matched,
        second_matched_color: second.matched.then_some(second.target_color),
        second_sampled_color: second.sampled_color,
        second_target_color: second.target_color,
        second_tolerance: second.tolerance,
        second_matched: second.matched,
        first_nearest_distance: first.distance,
        second_nearest_distance: second.distance,
        passed: first.matched && second.matched,
    })
}

#[tauri::command]
pub async fn special_ops_test_calibration_target(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    settings_coordinator: State<'_, Arc<SettingsCoordinator>>,
    environment_id: String,
    target_key: String,
    settings_revision: u64,
) -> Result<CalibrationTestResult, AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let (input, game_context) = {
        let settings = state
            .settings
            .lock()
            .map_err(|_| AppError::from("特勤处状态已损坏"))?;
        let input = calibration_test_input(&settings, &environment_id, &target_key)?;
        let game_context = if calibration_test_requires_game_context(&target_key) {
            let game_path = std::fs::canonicalize(settings.game_executable_path.trim())
                .map_err(|_| AppError::from("游戏 exe 路径无效"))?;
            Some((game_path, mouse_parking_region(&settings)?))
        } else {
            None
        };
        (input, game_context)
    };
    if let Some((game_path, mouse_parking)) = game_context {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        ensure_no_active_special_ops_run(&state.login_runtime)?;
        tokio::task::spawn_blocking(move || {
            use desktop_runtime::DesktopRuntime;
            let desktop = desktop_runtime::WindowsDesktopRuntime;
            let window = desktop
                .find_primary_window(&game_path)?
                .ok_or_else(|| "未找到游戏窗口".to_string())?;
            desktop.restore_and_focus(&game_path, window)
        })
        .await
        .map_err(|error| AppError::from(format!("游戏窗口任务失败: {error}")))??;
        crate::input_simulation::move_region_center_cancellable(
            mouse_parking,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await
        .map_err(AppError::from)?;
    }
    match input {
        CalibrationTestInput::Ocr { region } => {
            let first_texts = sample_numeric_ocr(region.clone()).await?;
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            let second_texts = sample_numeric_ocr(region).await?;
            let passed = !first_texts.is_empty() && !second_texts.is_empty();
            Ok(CalibrationTestResult::Ocr {
                first_texts,
                second_texts,
                passed,
            })
        }
        CalibrationTestInput::Template(input) => {
            let first = sample_template_similarity(
                input.region.clone(),
                input.reference_image_path.clone(),
            )
            .await?;
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            let second = sample_template_similarity(
                input.region.clone(),
                input.reference_image_path.clone(),
            )
            .await?;
            let sample_similarities = [first, second];
            let passed = template_test_passed(sample_similarities, input.match_threshold);
            let verified_at_ms = passed.then(now_ms);
            settings_coordinator
                .with_revision(settings_revision, || -> Result<(), String> {
                    ensure_no_active_special_ops_run(&state.login_runtime)?;
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
                    drop(settings);
                    emit_state(&app, settings_revision, now_ms());
                    Ok(())
                })
                .map_err(AppError::from)?;
            Ok(CalibrationTestResult::Template {
                sample_similarities,
                passed,
                verified_at_ms,
            })
        }
    }
}

#[tauri::command]
pub async fn special_ops_begin_calibration_selection(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    environment_id: String,
    target_key: String,
    account_id: Option<String>,
    settings_revision: u64,
) -> Result<(), AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let target_kind = {
        let settings = state
            .settings
            .lock()
            .map_err(|_| AppError::from("特勤处状态已损坏"))?;
        calibration_selection_kind(
            &settings,
            &environment_id,
            &target_key,
            account_id.as_deref(),
        )
        .map_err(AppError::from)?
    };
    let label = calibration_selection_label(&environment_id, &target_key, account_id.as_deref());
    destroy_window(&app, &label);
    let url = format!(
        "index.html?mode=special-ops-calibration&environment_id={}&target_key={}&settings_revision={}",
        encoded_query_value(&environment_id),
        encoded_query_value(&target_key),
        settings_revision
    );
    let url = format!(
        "{url}&target_kind={}",
        encoded_query_value(&format!("{target_kind:?}"))
    );
    let url = match account_id.as_deref() {
        Some(account_id) => format!("{url}&account_id={}", encoded_query_value(account_id)),
        None => url,
    };
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
    environment_id: String,
    target_key: String,
    account_id: Option<String>,
    region: CalibrationRect,
    settings_revision: u64,
) -> Result<(), AppError> {
    let settings_coordinator = app.state::<Arc<SettingsCoordinator>>();
    settings_coordinator
        .with_revision(settings_revision, || -> Result<(), String> {
            ensure_no_active_special_ops_run(&state.login_runtime)?;
            let settings = {
                let settings = state
                    .settings
                    .lock()
                    .map_err(|_| "特勤处状态已损坏".to_string())?;
                let mut next = settings.clone();
                let target_kind = calibration_selection_kind(
                    &next,
                    &environment_id,
                    &target_key,
                    account_id.as_deref(),
                )?;
                validate_calibration_selection(target_kind, &region)?;
                if target_key == BUSINESS_MARKET_PRODUCT_TARGET {
                    apply_market_business_selection(&mut next, account_id.as_deref(), region)?;
                } else if let Some(target_id) = business_ammo_target_id(&target_key) {
                    apply_ammo_business_selection(
                        &mut next,
                        account_id.as_deref(),
                        target_id,
                        region,
                    )?;
                } else if let Some(account_id) = account_id.as_deref() {
                    let station = account_recipe_station(&target_key)?;
                    apply_account_recipe_selection(&mut next, account_id, station, region)?;
                } else {
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
                }
                next
            };
            save_settings(&app, &settings)?;
            *state
                .settings
                .lock()
                .map_err(|_| "特勤处状态已损坏".to_string())? = settings.clone();
            emit_state(&app, settings_revision, now_ms());
            let label =
                calibration_selection_label(&environment_id, &target_key, account_id.as_deref());
            destroy_window(&app, &label);
            restore_main_window(&app);
            Ok(())
        })
        .map_err(AppError::from)
}

fn station_suffix(station: &StationKind) -> &'static str {
    match station {
        StationKind::TechnicalCenter => "technicalCenter",
        StationKind::Workbench => "workbench",
        StationKind::Pharmacy => "pharmacy",
        StationKind::ArmorBench => "armorBench",
    }
}

/// 校验校准提交尺寸；点击点使用单像素坐标，识别区域必须是可采样矩形。
fn validate_calibration_selection(
    kind: CalibrationTargetKind,
    region: &CalibrationRect,
) -> Result<(), String> {
    match kind {
        CalibrationTargetKind::ClickPoint if region.width == 1 && region.height == 1 => Ok(()),
        CalibrationTargetKind::ClickPoint => Err("点击点必须提交单点坐标".to_string()),
        CalibrationTargetKind::InputRegion | CalibrationTargetKind::RecognitionRegion
            if region.width > 2 && region.height > 2 =>
        {
            Ok(())
        }
        CalibrationTargetKind::InputRegion | CalibrationTargetKind::RecognitionRegion => {
            Err("校准区域太小".to_string())
        }
    }
}

#[tauri::command]
pub fn special_ops_cancel_calibration_selection(
    app: AppHandle,
    state: State<'_, SpecialOpsState>,
    environment_id: String,
    target_key: String,
    account_id: Option<String>,
) -> Result<(), AppError> {
    ensure_no_active_special_ops_run(&state.login_runtime)?;
    let label = calibration_selection_label(&environment_id, &target_key, account_id.as_deref());
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

fn limited_cycle_projections(
    now_ms: i64,
    timeline_end_ms: i64,
) -> Vec<(limited_supply::LimitedSupplyCycle, i64)> {
    let (day, minute) = local_day_and_minute(now_ms);
    let Some(current) = limited_supply::LimitedSupplyCycle::for_day_and_minute(
        &day,
        u16::try_from(minute).unwrap_or(u16::MAX),
    ) else {
        return Vec::new();
    };
    let Some(today_noon) = daily_exchange_at_ms(now_ms, 12 * 60) else {
        return Vec::new();
    };
    let mut scheduled_at_ms = if minute < 12 * 60 {
        today_noon.saturating_sub(16 * 60 * 60_000)
    } else if minute < 20 * 60 {
        today_noon
    } else {
        today_noon.saturating_add(8 * 60 * 60_000)
    };
    let mut projections = vec![(current, scheduled_at_ms)];
    loop {
        let start_minute = local_day_and_minute(scheduled_at_ms).1;
        let delta = if start_minute == 12 * 60 {
            8 * 60 * 60_000
        } else {
            16 * 60 * 60_000
        };
        scheduled_at_ms = scheduled_at_ms.saturating_add(delta);
        if scheduled_at_ms > timeline_end_ms {
            break;
        }
        let (day, minute) = local_day_and_minute(scheduled_at_ms);
        let Some(cycle) = limited_supply::LimitedSupplyCycle::for_day_and_minute(
            &day,
            u16::try_from(minute).unwrap_or(u16::MAX),
        ) else {
            break;
        };
        projections.push((cycle, scheduled_at_ms));
    }
    projections
}

fn market_start_projections(now_ms: i64, timeline_end_ms: i64) -> Vec<i64> {
    let minute = local_day_and_minute(now_ms).1;
    let Some(today_start) = daily_exchange_at_ms(now_ms, 2 * 60) else {
        return Vec::new();
    };
    let mut scheduled_at_ms = if minute < 4 * 60 {
        today_start
    } else {
        today_start.saturating_add(24 * 60 * 60_000)
    };
    let mut projections = Vec::new();
    while scheduled_at_ms <= timeline_end_ms {
        projections.push(scheduled_at_ms);
        scheduled_at_ms = scheduled_at_ms.saturating_add(24 * 60 * 60_000);
    }
    projections
}

fn build_timeline_tasks(
    settings: &SpecialOpsSettings,
    now_ms: i64,
    timeline_end_ms: i64,
    profit_snapshot: Option<&ProfitRuntimeSnapshot>,
) -> Vec<TimelineTask> {
    let exchange_minute = daily_exchange_minutes(&settings.daily_exchange_time).unwrap_or(0);
    let today_exchange_at_ms = daily_exchange_at_ms(now_ms, exchange_minute).unwrap_or(now_ms);
    let tomorrow_exchange_at_ms = today_exchange_at_ms.saturating_add(24 * 60 * 60_000);
    let today = local_day_and_minute(today_exchange_at_ms).0;
    let tomorrow = local_day_and_minute(tomorrow_exchange_at_ms).0;
    let current_day = local_day_and_minute(now_ms).0;
    let cutoff_at_ms = if settings.profit_filter.enabled {
        daily_exchange_minutes(&settings.profit_filter.cutoff_time)
            .and_then(|minute| daily_exchange_at_ms(now_ms, minute))
    } else {
        None
    };
    let mut tasks = Vec::new();

    for account in settings.accounts.iter().filter(|account| account.enabled) {
        let Ok(business) = resolve_account_business_config(settings, account) else {
            continue;
        };
        for station_kind in StationKind::all() {
            let Some(station) = account
                .stations
                .iter()
                .find(|station| station.kind == station_kind)
            else {
                continue;
            };
            let Some(station_business) = business
                .stations
                .iter()
                .find(|candidate| candidate.kind == station.kind && candidate.enabled)
            else {
                continue;
            };
            let manual_failure = account
                .last_failure
                .as_ref()
                .filter(|failure| failure.station_kind.as_ref() == Some(&station.kind))
                .cloned();
            // Uncertain 台子在调度与任务栏双重过滤下会永久消失，人工判定就没了入口。
            // 保留在任务栏（不进调度，因为 due 集合仍然跳过 Uncertain）才能人工恢复。
            let station_uncertain = station.status == StationStatus::Uncertain;
            let scheduled_at_ms = if manual_failure.is_some() || station_uncertain {
                now_ms
            } else {
                let Some(value) = station.finishes_at_ms else {
                    continue;
                };
                value
            };
            if manual_failure.is_none()
                && !station_uncertain
                && (station.status == StationStatus::Idle || scheduled_at_ms > timeline_end_ms)
            {
                continue;
            }
            let station_order = StationKind::all()
                .iter()
                .position(|kind| *kind == station.kind)
                .unwrap_or(0) as u32;
            let id = format!(
                "craft:{}:{}:{}",
                account.id,
                station.kind.calibration_suffix(),
                scheduled_at_ms
            );
            let task = TimelineTask {
                id: id.clone(),
                account_id: account.id.clone(),
                qq_account: account.qq_account.clone(),
                kind: TimelineTaskKind::Craft,
                station_kind: Some(station.kind.clone()),
                ammo_target_id: None,
                note: station_business.recipe_note.clone(),
                scheduled_at_ms,
                overdue: scheduled_at_ms <= now_ms,
                account_status: account.status.clone(),
                profit_state: None,
                may_execute_earlier: false,
                manual_failure,
                limited_cycle_id: None,
                limited_outcome: None,
                market_completed_count: None,
                market_target_count: None,
                market_status: None,
            };
            tasks.push((scheduled_at_ms, account.order, station_order, id, task));
        }

        let mut ammo_targets = business
            .ammo_targets
            .iter()
            .filter(|target| target.enabled)
            .collect::<Vec<_>>();
        ammo_targets.sort_by_key(|target| target.order);
        for target in ammo_targets {
            let runtime = account
                .ammo_targets
                .iter()
                .find(|candidate| candidate.id == target.id);
            if let Some(manual_failure) = runtime.and_then(|state| state.last_failure.clone()) {
                let id = format!("ammo:{current_day}:{}:{}", account.id, target.id);
                let task = TimelineTask {
                    id: id.clone(),
                    account_id: account.id.clone(),
                    qq_account: account.qq_account.clone(),
                    kind: TimelineTaskKind::Ammo,
                    station_kind: None,
                    ammo_target_id: Some(target.id.clone()),
                    note: target.note.clone(),
                    scheduled_at_ms: now_ms,
                    overdue: true,
                    account_status: account.status.clone(),
                    profit_state: None,
                    may_execute_earlier: false,
                    manual_failure: Some(manual_failure),
                    limited_cycle_id: None,
                    limited_outcome: None,
                    market_completed_count: None,
                    market_target_count: None,
                    market_status: None,
                };
                tasks.push((now_ms, account.order, 100 + target.order, id, task));
                continue;
            }
            for (day, scheduled_at_ms, day_order) in [
                (today.as_str(), today_exchange_at_ms, 0_u32),
                (tomorrow.as_str(), tomorrow_exchange_at_ms, 1_u32),
            ] {
                if scheduled_at_ms > timeline_end_ms
                    || runtime.is_some_and(|state| state.last_success_day.as_deref() == Some(day))
                {
                    continue;
                }
                let active_round_target = profit_snapshot.is_some_and(|snapshot| {
                    snapshot
                        .active_round_targets
                        .iter()
                        .any(|key| key.account_id == account.id && key.target_id == target.id)
                });
                let cutoff_target =
                    cutoff_state_for_day(settings, &current_day).and_then(|state| {
                        state.targets.iter().find(|candidate| {
                            candidate.account_id == account.id && candidate.target_id == target.id
                        })
                    });
                let (projected_at_ms, profit_state, may_execute_earlier) =
                    if !settings.profit_filter.enabled || day != current_day {
                        (scheduled_at_ms, None, false)
                    } else if now_ms < today_exchange_at_ms {
                        (
                            today_exchange_at_ms,
                            Some(TimelineProfitState::WaitingExchange),
                            false,
                        )
                    } else if active_round_target {
                        (now_ms, Some(TimelineProfitState::ActiveRound), false)
                    } else if cutoff_at_ms.is_some_and(|cutoff| now_ms >= cutoff) {
                        match cutoff_target {
                            Some(target_state)
                                if target_state.decided_at_ms.is_some()
                                    && target_state.skip_reason.is_none() =>
                            {
                                (now_ms, Some(TimelineProfitState::Qualified), false)
                            }
                            Some(target_state) if target_state.decided_at_ms.is_some() => (
                                scheduled_at_ms,
                                Some(TimelineProfitState::CutoffSkipped),
                                false,
                            ),
                            Some(_) => {
                                let phase = profit_snapshot.map(|snapshot| snapshot.phase);
                                if phase
                                    == Some(profit::runtime::ProfitRuntimePhase::CutoffQuerying)
                                {
                                    (now_ms, Some(TimelineProfitState::CutoffQuerying), false)
                                } else {
                                    (
                                        profit_snapshot
                                            .and_then(|snapshot| snapshot.next_query_at_ms)
                                            .unwrap_or(now_ms),
                                        Some(TimelineProfitState::WaitingCutoffRetry),
                                        false,
                                    )
                                }
                            }
                            None => (
                                scheduled_at_ms,
                                Some(TimelineProfitState::CutoffSkipped),
                                false,
                            ),
                        }
                    } else {
                        match target.profit_rule_id.as_deref() {
                            None => (
                                cutoff_at_ms.unwrap_or(scheduled_at_ms),
                                Some(TimelineProfitState::Unconfigured),
                                false,
                            ),
                            Some(rule_id)
                                if profit_snapshot.is_some_and(|snapshot| {
                                    snapshot
                                        .qualified_rule_ids
                                        .iter()
                                        .any(|qualified| qualified == rule_id)
                                }) =>
                            {
                                (now_ms, Some(TimelineProfitState::Qualified), false)
                            }
                            Some(_) => (
                                cutoff_at_ms.unwrap_or(scheduled_at_ms),
                                Some(TimelineProfitState::WaitingQuery),
                                true,
                            ),
                        }
                    };
                if profit_state == Some(TimelineProfitState::CutoffSkipped) {
                    continue;
                }
                let id = format!("ammo:{day}:{}:{}", account.id, target.id);
                let task = TimelineTask {
                    id: id.clone(),
                    account_id: account.id.clone(),
                    qq_account: account.qq_account.clone(),
                    kind: TimelineTaskKind::Ammo,
                    station_kind: None,
                    ammo_target_id: Some(target.id.clone()),
                    note: target.note.clone(),
                    scheduled_at_ms: projected_at_ms,
                    overdue: projected_at_ms <= now_ms,
                    account_status: account.status.clone(),
                    profit_state,
                    may_execute_earlier,
                    manual_failure: None,
                    limited_cycle_id: None,
                    limited_outcome: None,
                    market_completed_count: None,
                    market_target_count: None,
                    market_status: None,
                };
                tasks.push((
                    projected_at_ms,
                    account.order,
                    100 + day_order * 10_000 + target.order,
                    id,
                    task,
                ));
            }
        }

        if settings.limited_supply.enabled {
            for (cycle, scheduled_at_ms) in limited_cycle_projections(now_ms, timeline_end_ms) {
                let is_current = scheduled_at_ms <= now_ms;
                let state_matches =
                    account.limited_supply.cycle_id.as_deref() == Some(cycle.id.as_str());
                let outcome = if is_current && state_matches {
                    account.limited_supply.outcome.clone()
                } else {
                    limited_supply::LimitedSupplyOutcome::Pending
                };
                let acknowledged =
                    is_current && state_matches && account.limited_supply.acknowledged;
                if outcome == limited_supply::LimitedSupplyOutcome::NoHighValue
                    || (outcome == limited_supply::LimitedSupplyOutcome::HighValue && acknowledged)
                {
                    continue;
                }
                let id = format!("limited:{}:{}", cycle.id, account.id);
                let task = TimelineTask {
                    id: id.clone(),
                    account_id: account.id.clone(),
                    qq_account: account.qq_account.clone(),
                    kind: TimelineTaskKind::LimitedSupplyCheck,
                    station_kind: None,
                    ammo_target_id: None,
                    note: "限时商品检查".to_string(),
                    scheduled_at_ms,
                    overdue: scheduled_at_ms <= now_ms,
                    account_status: account.status.clone(),
                    profit_state: None,
                    may_execute_earlier: false,
                    manual_failure: None,
                    limited_cycle_id: Some(cycle.id),
                    limited_outcome: Some(outcome),
                    market_completed_count: None,
                    market_target_count: None,
                    market_status: None,
                };
                tasks.push((scheduled_at_ms, account.order, 200, id, task));
            }
        }

        if business.market.enabled {
            for scheduled_at_ms in market_start_projections(now_ms, timeline_end_ms) {
                let day = local_day_and_minute(scheduled_at_ms).0;
                let is_current = scheduled_at_ms <= now_ms;
                let state_matches = account.market.day.as_deref() == Some(day.as_str());
                let status = if is_current && state_matches {
                    account.market.status.clone()
                } else {
                    market_purchase::MarketTaskStatus::Pending
                };
                let completed_count = if is_current && state_matches {
                    account.market.completed_count
                } else {
                    0
                };
                if matches!(
                    status,
                    market_purchase::MarketTaskStatus::Completed
                        | market_purchase::MarketTaskStatus::WindowClosed
                ) {
                    continue;
                }
                let id = format!("market:{day}:{}", account.id);
                let task = TimelineTask {
                    id: id.clone(),
                    account_id: account.id.clone(),
                    qq_account: account.qq_account.clone(),
                    kind: TimelineTaskKind::MarketPurchase,
                    station_kind: None,
                    ammo_target_id: None,
                    note: if business.market.item_note.is_empty() {
                        "交易行购买".to_string()
                    } else {
                        business.market.item_note.clone()
                    },
                    scheduled_at_ms,
                    overdue: scheduled_at_ms <= now_ms,
                    account_status: account.status.clone(),
                    profit_state: None,
                    may_execute_earlier: false,
                    manual_failure: None,
                    limited_cycle_id: None,
                    limited_outcome: None,
                    market_completed_count: Some(completed_count),
                    market_target_count: Some(business.market.purchase_count),
                    market_status: Some(status),
                };
                tasks.push((scheduled_at_ms, account.order, 300, id, task));
            }
        }
    }

    // 任务栏按执行顺序排序，对齐 build_round_plan_with_profit：
    // 到期任务先按账号分桶（账号内按时间），未到期任务整体排在后面并保持时间优先。
    // 未到期桶保留 account.order 作次键 —— 同毫秒未来制作台必须同账号相邻，
    // 否则 planner 的 future_accounts 合并会把一个账号拆成多个 AccountRoundTask。
    let execution_key = |entry: &(i64, u32, u32, String, TimelineTask)| -> (u8, i64, i64) {
        if entry.0 <= now_ms {
            (0, i64::from(entry.1), entry.0)
        } else {
            (1, entry.0, i64::from(entry.1))
        }
    };
    tasks.sort_by(|left, right| {
        execution_key(left)
            .cmp(&execution_key(right))
            .then_with(|| (&left.2, &left.3).cmp(&(&right.2, &right.3)))
    });
    tasks.into_iter().map(|(_, _, _, _, task)| task).collect()
}

pub fn build_schedule(settings: &SpecialOpsSettings, now_ms: i64) -> ScheduleSnapshot {
    build_schedule_with_profit(settings, now_ms, &AmmoProfitGate::DisplayOnly)
}

pub(crate) fn build_schedule_with_profit(
    settings: &SpecialOpsSettings,
    now_ms: i64,
    gate: &AmmoProfitGate,
) -> ScheduleSnapshot {
    build_schedule_with_profit_runtime(settings, now_ms, gate, None)
}

pub(crate) fn build_schedule_with_profit_runtime(
    settings: &SpecialOpsSettings,
    now_ms: i64,
    gate: &AmmoProfitGate,
    profit_snapshot: Option<&ProfitRuntimeSnapshot>,
) -> ScheduleSnapshot {
    let timeline_end_ms = now_ms.saturating_add(24 * 60 * 60_000);
    let timeline_tasks = build_timeline_tasks(settings, now_ms, timeline_end_ms, profit_snapshot);
    if !settings.enabled || settings.paused {
        return ScheduleSnapshot {
            due_accounts: Vec::new(),
            next_wake_at_ms: None,
            timeline_start_ms: now_ms,
            timeline_end_ms,
            timeline_tasks,
        };
    }

    let mut accounts = settings.accounts.iter().collect::<Vec<_>>();
    accounts.sort_by_key(|account| account.order);

    let exchange_minute = daily_exchange_minutes(&settings.daily_exchange_time);
    let today = local_day_and_minute(now_ms).0;
    let exchange_at_ms = exchange_minute.and_then(|minute| daily_exchange_at_ms(now_ms, minute));
    let mut due_accounts = Vec::new();
    let mut next_wake_at_ms = timeline_tasks
        .iter()
        .filter(|task| {
            task.scheduled_at_ms > now_ms
                && matches!(
                    task.kind,
                    TimelineTaskKind::LimitedSupplyCheck | TimelineTaskKind::MarketPurchase
                )
        })
        .map(|task| task.scheduled_at_ms)
        .min();
    for account in accounts {
        if !account.enabled || !account.initialized || account.status != AccountStatus::Ready {
            continue;
        }
        let Ok(business_config) = resolve_account_business_config(settings, account) else {
            continue;
        };

        let mut station_kinds = Vec::new();
        for station_kind in StationKind::all() {
            let Some(station) = account
                .stations
                .iter()
                .find(|station| station.kind == station_kind)
            else {
                continue;
            };
            let enabled = business_config
                .stations
                .iter()
                .find(|business| business.kind == station.kind)
                .is_some_and(|business| business.enabled);
            if !enabled
                || matches!(
                    station.status,
                    StationStatus::Idle | StationStatus::Uncertain
                )
            {
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

        let mut ammo_target_ids = business_config
            .ammo_targets
            .iter()
            .filter(|target| target.enabled)
            .filter(|target| gate.allows(&account.id, &target.id, target.profit_rule_id.as_deref()))
            .filter(|target| {
                account
                    .ammo_targets
                    .iter()
                    .find(|runtime| runtime.id == target.id)
                    .is_none_or(|runtime| {
                        runtime.last_failure.is_none()
                            && runtime.last_success_day.as_deref() != Some(today.as_str())
                            && (runtime.retry_day.as_deref() != Some(today.as_str())
                                || runtime.retry_count < 2)
                    })
            })
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

        let (current_day, current_minute) = local_day_and_minute(now_ms);
        let limited_supply_due = settings.limited_supply.enabled
            && limited_supply::LimitedSupplyCycle::for_day_and_minute(
                &current_day,
                u16::try_from(current_minute).unwrap_or(u16::MAX),
            )
            .is_some_and(|cycle| {
                account.limited_supply.cycle_id.as_deref() != Some(cycle.id.as_str())
                    || account.limited_supply.outcome
                        == limited_supply::LimitedSupplyOutcome::Pending
            });
        let market_purchase_due = business_config.market.enabled
            && market_purchase::market_window_open(
                u16::try_from(current_minute).unwrap_or(u16::MAX),
            )
            && (account.market.day.as_deref() != Some(current_day.as_str())
                || (account.market.completed_count < business_config.market.purchase_count
                    && matches!(
                        account.market.status,
                        market_purchase::MarketTaskStatus::Pending
                            | market_purchase::MarketTaskStatus::Running
                    )));

        if station_kinds.is_empty() && ammo_target_ids.is_empty() {
            if let Some(exchange_at) = exchange_at_ms.filter(|exchange_at| *exchange_at > now_ms) {
                let has_pending_ammo = business_config.ammo_targets.iter().any(|target| {
                    target.enabled
                        && gate.allows(&account.id, &target.id, target.profit_rule_id.as_deref())
                        && account
                            .ammo_targets
                            .iter()
                            .find(|runtime| runtime.id == target.id)
                            .is_none_or(|runtime| {
                                runtime.last_failure.is_none()
                                    && runtime.last_success_day.as_deref() != Some(today.as_str())
                                    && (runtime.retry_day.as_deref() != Some(today.as_str())
                                        || runtime.retry_count < 2)
                            })
                });
                if has_pending_ammo {
                    next_wake_at_ms = Some(
                        next_wake_at_ms.map_or(exchange_at, |current| current.min(exchange_at)),
                    );
                }
            }
        }
        if !station_kinds.is_empty()
            || !ammo_target_ids.is_empty()
            || limited_supply_due
            || market_purchase_due
        {
            due_accounts.push(DueAccount {
                account_id: account.id.clone(),
                station_kinds,
                ammo_target_ids,
                limited_supply_due,
                market_purchase_due,
            });
        }
    }

    if !due_accounts.is_empty() {
        next_wake_at_ms = Some(now_ms);
    }

    ScheduleSnapshot {
        due_accounts,
        next_wake_at_ms,
        timeline_start_ms: now_ms,
        timeline_end_ms,
        timeline_tasks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_special_ops_run_rejects_mutating_commands_until_cleanup() {
        let runtime = login_runtime::LoginRuntime::default();
        ensure_no_active_special_ops_run(&runtime).unwrap();

        runtime.try_start("account-a".to_string()).unwrap();

        assert_eq!(
            ensure_no_active_special_ops_run(&runtime).unwrap_err(),
            "特勤处试运行尚未完成清理"
        );
    }

    #[test]
    fn round_pause_request_does_not_cancel_active_run() {
        let control = RoundControl::default();
        let runtime = login_runtime::LoginRuntime::default();
        let started = runtime
            .try_start_kind("account-a".to_string(), LoginRunKind::Round)
            .unwrap();
        control.request_pause();

        assert!(control.pause_requested());
        assert!(!started.cancelled.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn only_pause_true_for_active_round_can_be_deferred() {
        assert!(should_defer_round_pause(Some(LoginRunKind::Round), true));
        assert!(!should_defer_round_pause(Some(LoginRunKind::Round), false));
        assert!(!should_defer_round_pause(Some(LoginRunKind::Craft), true));
        assert!(!should_defer_round_pause(None, true));
    }

    #[test]
    fn round_account_failure_preserves_started_time_and_does_not_pause_global() {
        let mut settings = SpecialOpsSettings {
            paused: false,
            accounts: vec![account(
                "selected",
                AccountStatus::Ready,
                vec![station(StationKind::Workbench, 100)],
            )],
            ..SpecialOpsSettings::default()
        };
        let started_at = settings.accounts[0].stations[0].started_at_ms;
        let error = round_runner::AccountRunError::account_station(
            StationKind::Workbench,
            "craft.abort",
            "识别失败",
        );

        apply_round_account_failure(&mut settings, "selected", &error, 200).unwrap();

        assert!(!settings.paused);
        assert_eq!(settings.accounts[0].status, AccountStatus::Uncertain);
        assert_eq!(
            settings.accounts[0].stations[0].status,
            StationStatus::Uncertain
        );
        assert_eq!(settings.accounts[0].stations[0].started_at_ms, started_at);
    }

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
            calibration_target_mut(&mut settings, "runtime.mouseParking").rect =
                Some(CalibrationRect {
                    x: 1,
                    y: 1,
                    width: 1,
                    height: 1,
                });
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

    fn configure_required_execution_targets(
        settings: &mut SpecialOpsSettings,
    ) -> Vec<tempfile::NamedTempFile> {
        let required_keys = required_execution_target_keys(settings);
        let mut references = Vec::new();
        for target in settings.calibration_environments[0]
            .targets
            .iter_mut()
            .filter(|target| required_keys.contains(&target.key))
        {
            let side = if target.kind == CalibrationTargetKind::ClickPoint {
                1
            } else {
                30
            };
            target.rect = Some(CalibrationRect {
                x: 10,
                y: 20,
                width: side,
                height: side,
            });
            if target.recognition_method == Some(CalibrationRecognitionMethod::Template) {
                let reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
                std::fs::write(reference.path(), target.key.as_bytes()).unwrap();
                target.reference_image_path = Some(reference.path().display().to_string());
                target.verified_signature = Some(calibration_signature(target).unwrap());
                target.verified_at_ms = Some(1);
                references.push(reference);
            }
        }
        references
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
            qq_account: format!("10{:05}", id.bytes().map(u32::from).sum::<u32>()),
            enabled: true,
            initialized: true,
            order: 0,
            status,
            independent_settings_enabled: false,
            independent_business_config: None,
            stations,
            ammo_targets: Vec::new(),
            last_failure: None,
            login_trial_signature: None,
            limited_supply: Default::default(),
            market: Default::default(),
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
        calibration_target_mut(&mut calibration_changed, "wegame.loginMode")
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
    fn manual_login_failure_persists_redacted_message_only() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        apply_login_flow_result(
            &mut settings,
            "selected",
            &login_flow::LoginFlowResult::NeedsManualLogin {
                account_id: "selected".to_string(),
                failed_step: login_flow::LoginStep::ScanRememberedAccounts,
                failure_message: "未找到目标 QQ；扫描轨迹：页 1 槽位 1 (1719,755) -> ***3589"
                    .to_string(),
                failed_at: 42,
            },
            login_runtime::StopReason::Normal,
            "v1-deadbeef",
        )
        .unwrap();

        let account = &settings.accounts[0];
        assert_eq!(account.status, AccountStatus::NeedsManualLogin);
        let failure = account.last_failure.as_ref().unwrap();
        assert_eq!(
            failure.message,
            "未找到目标 QQ；扫描轨迹：页 1 槽位 1 (1719,755) -> ***3589"
        );
        assert!(!failure.message.contains("3079643589"));
    }

    #[test]
    fn craft_cancel_after_input_marks_account_and_station_uncertain() {
        let fixture = LoginFixture::complete();
        let mut settings = fixture.settings;
        settings.accounts[0].stations = vec![station(StationKind::Workbench, 10_000)];

        mark_craft_cancel_uncertain(&mut settings, "selected", StationKind::Workbench, 42).unwrap();

        assert!(settings.paused);
        assert_eq!(settings.accounts[0].status, AccountStatus::Uncertain);
        assert_eq!(
            settings.accounts[0].stations[0].status,
            StationStatus::Uncertain
        );
        assert_eq!(
            settings.accounts[0].last_failure.as_ref().unwrap().step,
            "craftCancel"
        );
    }

    #[test]
    fn craft_success_persistence_returns_revision_without_reentrant_lock() {
        let coordinator = SettingsCoordinator::new();
        let initial_revision = coordinator.current_revision().unwrap();
        let fixture = LoginFixture::complete();
        let mut initial = fixture.settings;
        initial.accounts[0].stations = vec![station(StationKind::TechnicalCenter, 10_000)];
        let settings = Mutex::new(initial);

        let (_, revision) = persist_craft_success_with(
            &settings,
            &coordinator,
            "selected",
            &StationKind::TechnicalCenter,
            1_000,
            60,
            |_| Ok(()),
        )
        .unwrap();

        assert_eq!(revision, initial_revision + 1);
        let stored = settings.lock().unwrap();
        let station = stored.accounts[0]
            .stations
            .iter()
            .find(|station| station.kind == StationKind::TechnicalCenter)
            .unwrap();
        assert_eq!(station.started_at_ms, Some(1_000));
        assert_eq!(station.finishes_at_ms, Some(3_601_000));
    }

    #[test]
    fn craft_success_persistence_failure_keeps_memory_unchanged() {
        let coordinator = SettingsCoordinator::new();
        let fixture = LoginFixture::complete();
        let mut initial = fixture.settings;
        initial.accounts[0].stations = vec![station(StationKind::TechnicalCenter, 10_000)];
        let settings = Mutex::new(initial);
        let before = settings.lock().unwrap().clone();

        let error = persist_craft_success_with(
            &settings,
            &coordinator,
            "selected",
            &StationKind::TechnicalCenter,
            1_000,
            60,
            |_| Err("测试保存失败".to_string()),
        )
        .unwrap_err();

        assert_eq!(error, "测试保存失败");
        assert_eq!(*settings.lock().unwrap(), before);
    }

    #[test]
    fn craft_cancel_before_first_input_keeps_business_state() {
        assert!(!should_mark_craft_stop_uncertain(
            Some(login_runtime::StopReason::Normal),
            false,
        ));
        assert!(should_mark_craft_stop_uncertain(
            Some(login_runtime::StopReason::Normal),
            true,
        ));
        assert!(should_mark_craft_stop_uncertain(
            Some(login_runtime::StopReason::Emergency),
            true,
        ));
    }

    #[test]
    fn craft_emergency_stop_requires_uncertain_state() {
        assert!(should_mark_craft_stop_uncertain(
            Some(login_runtime::StopReason::Emergency),
            false,
        ));
        assert!(should_mark_craft_stop_uncertain(
            Some(login_runtime::StopReason::Lifecycle { uncertain: true }),
            false,
        ));
        assert!(!should_mark_craft_stop_uncertain(
            Some(login_runtime::StopReason::Lifecycle { uncertain: false }),
            true,
        ));
    }

    #[test]
    fn craft_outcome_maps_to_persistence_decision() {
        assert_eq!(
            decide_craft_persistence(Ok(craft_runtime::CraftStationOutcome::StillInProgress)),
            CraftPersistenceDecision::NoChange,
        );
        assert_eq!(
            decide_craft_persistence(Ok(craft_runtime::CraftStationOutcome::Started {
                started_at_ms: 42,
            })),
            CraftPersistenceDecision::SaveStarted { started_at_ms: 42 },
        );
        let uncertain = craft_trial::CraftTrialFailure {
            step: "craft.stationOpen".to_string(),
            message: "三次点击后均未识别到奖励页或制作列表".to_string(),
            requires_uncertain: true,
        };
        assert!(matches!(
            decide_craft_persistence(Err(uncertain)),
            CraftPersistenceDecision::MarkUncertain { .. }
        ));
        let isolated = craft_trial::CraftTrialFailure {
            step: "craft.isolated".to_string(),
            message: "材料购买重试后仍停留在购买页面".to_string(),
            requires_uncertain: false,
        };
        assert!(matches!(
            decide_craft_persistence(Err(isolated)),
            CraftPersistenceDecision::MarkIsolated { ref step, .. }
                if step == "craft.isolated"
        ));
    }

    #[test]
    fn batch_failure_uses_current_station_and_local_input_state() {
        let failure = craft_batch::CraftBatchFailure {
            station: StationKind::Workbench,
            failure: craft_trial::CraftTrialFailure {
                step: "craft.space".to_string(),
                message: "已取消".to_string(),
                requires_uncertain: false,
            },
            entered_input: false,
        };

        assert_eq!(
            batch_stop_context(&failure),
            (StationKind::Workbench, false)
        );
    }

    #[test]
    fn later_batch_failure_preserves_earlier_station_success() {
        let coordinator = SettingsCoordinator::new();
        let fixture = LoginFixture::complete();
        let mut initial = fixture.settings;
        initial.accounts[0].stations = vec![
            station(StationKind::TechnicalCenter, 10_000),
            station(StationKind::Workbench, 10_000),
        ];
        let settings = Mutex::new(initial);

        persist_craft_success_with(
            &settings,
            &coordinator,
            "selected",
            &StationKind::TechnicalCenter,
            100,
            60,
            |_| Ok(()),
        )
        .unwrap();
        persist_craft_failure_uncertain_with(
            &settings,
            &coordinator,
            "selected",
            &StationKind::Workbench,
            200,
            "craft.abort",
            "识别失败",
            |_| Ok(()),
        )
        .unwrap();

        let saved = settings.lock().unwrap();
        assert_eq!(saved.accounts[0].stations[0].started_at_ms, Some(100));
        assert_eq!(
            saved.accounts[0].stations[0].finishes_at_ms,
            Some(3_600_100)
        );
        assert_eq!(
            saved.accounts[0].stations[0].status,
            StationStatus::Crafting
        );
        assert_eq!(
            saved.accounts[0].stations[1].status,
            StationStatus::Uncertain
        );
    }

    #[test]
    fn craft_stop_persistence_failure_keeps_memory_unchanged() {
        let coordinator = SettingsCoordinator::new();
        let fixture = LoginFixture::complete();
        let mut initial = fixture.settings;
        initial.accounts[0].stations = vec![station(StationKind::Workbench, 10_000)];
        let settings = Mutex::new(initial);
        let before = settings.lock().unwrap().clone();

        let error = persist_craft_uncertain_with(
            &settings,
            &coordinator,
            "selected",
            &StationKind::Workbench,
            42,
            |_| Err("测试保存失败".to_string()),
        )
        .unwrap_err();

        assert_eq!(error, "测试保存失败");
        assert_eq!(*settings.lock().unwrap(), before);
    }

    #[test]
    fn craft_runtime_uncertain_persists_actual_failure_details() {
        let coordinator = SettingsCoordinator::new();
        let fixture = LoginFixture::complete();
        let mut initial = fixture.settings;
        initial.accounts[0].stations = vec![station(StationKind::Workbench, 10_000)];
        let settings = Mutex::new(initial);

        persist_craft_failure_uncertain_with(
            &settings,
            &coordinator,
            "selected",
            &StationKind::Workbench,
            42,
            "craft.stationOpen",
            "3 次点击后仍未识别到奖励页或制作列表",
            |_| Ok(()),
        )
        .unwrap();

        let saved = settings.lock().unwrap();
        let account = &saved.accounts[0];
        assert_eq!(account.status, AccountStatus::Uncertain);
        assert_eq!(
            account.last_failure.as_ref().unwrap().step,
            "craft.stationOpen"
        );
        assert_eq!(
            account.last_failure.as_ref().unwrap().message,
            "3 次点击后仍未识别到奖励页或制作列表"
        );
        assert_eq!(account.stations[0].status, StationStatus::Uncertain);
    }

    #[test]
    fn craft_emergency_stop_persistence_unlocks_runtime_cleanup() {
        let runtime = login_runtime::LoginRuntime::default();
        let started = runtime
            .try_start_kind("selected".to_string(), login_runtime::LoginRunKind::Craft)
            .unwrap();
        runtime
            .request_stop(started.run_id, login_runtime::StopReason::Emergency)
            .unwrap()
            .unwrap();
        let coordinator = SettingsCoordinator::new();
        let fixture = LoginFixture::complete();
        let mut initial = fixture.settings;
        initial.accounts[0].stations = vec![station(StationKind::Workbench, 10_000)];
        let settings = Mutex::new(initial);

        persist_craft_stop_with(
            &runtime,
            started.run_id,
            &settings,
            &coordinator,
            "selected",
            &StationKind::Workbench,
            42,
            true,
            |_| Ok(()),
        )
        .unwrap();

        assert!(runtime.cleanup_ready(started.run_id).unwrap());
    }

    #[test]
    fn operation_window_must_finish_loading_before_worker_handoff() {
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        ready_tx.send(()).unwrap();

        wait_for_operation_window_ready(ready_rx, std::time::Duration::from_millis(10)).unwrap();
    }

    #[test]
    fn operation_window_load_timeout_rejects_worker_handoff() {
        let (_ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);

        let error = wait_for_operation_window_ready(ready_rx, std::time::Duration::from_millis(1))
            .unwrap_err();

        assert_eq!(error, OPERATION_WINDOW_LOAD_TIMEOUT);
    }

    #[test]
    fn operation_window_reuses_existing_label_and_hides_between_runs() {
        let source = include_str!("mod.rs");
        let create_start = source.find("fn create_operation_window(").unwrap();
        let builder_offset = source[create_start..]
            .find("tauri::WebviewWindowBuilder::new(")
            .unwrap();
        let setup = &source[create_start..create_start + builder_offset];

        assert!(setup.contains("if let Some(window) = app.get_webview_window"));
        assert!(setup.contains("window.show()"));
        assert!(!setup.contains("destroy_operation_window"));
        assert!(source.contains("fn hide_operation_window("));
        assert!(source.contains("let result = hide_operation_window(app);"));
    }

    #[test]
    fn navigation_emergency_stop_requires_uncertain_persistence() {
        let outcome =
            navigation_stop_outcome(Some(login_runtime::StopReason::Emergency), "selected", 42)
                .unwrap();

        assert_eq!(outcome.1, login_runtime::StopReason::Emergency);
        assert!(matches!(
            outcome.0,
            login_flow::LoginFlowResult::EmergencyStopped {
                account_id,
                stopped_at: 42,
            } if account_id == "selected"
        ));
        assert!(
            navigation_stop_outcome(Some(login_runtime::StopReason::Normal), "selected", 42,)
                .is_none()
        );
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
    fn failed_worker_persistence_still_cleans_up_and_releases_active_run() {
        let runtime = login_runtime::LoginRuntime::default();
        let started = runtime.try_start("selected".to_string()).unwrap();
        let cleanup_calls = std::sync::atomic::AtomicUsize::new(0);
        let persist_result: Result<Option<login_runtime::PersistenceKind>, String> =
            Err("测试持久化失败".to_string());

        let error = cleanup_login_worker_after_persistence(&persist_result, || {
            cleanup_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            runtime.finish(started.run_id, LoginRunStatus::Failed, "已清理")?;
            Ok(())
        })
        .unwrap_err();

        assert_eq!(error, "测试持久化失败");
        assert_eq!(cleanup_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(runtime.snapshot().unwrap().is_none());
        assert!(runtime.try_start("next".to_string()).is_ok());
    }

    #[test]
    fn failed_worker_persistence_combines_cleanup_error() {
        let persist_result: Result<(), String> = Err("业务失败".to_string());

        let error = cleanup_login_worker_after_persistence(&persist_result, || {
            Err("资源清理失败".to_string())
        })
        .unwrap_err();

        assert_eq!(error, "业务失败; 资源清理失败");
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
            LoginRunKind::Login,
            || Ok(()),
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
                LoginRunKind::Login,
                || Ok(()),
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

        assert_eq!(stopped.status, LoginRunStatus::Stopping);
        assert!(start.join().unwrap().is_err());
        assert_eq!(
            events.lock().unwrap().as_slice(),
            &[LoginRunStatus::Stopping]
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
                LoginRunKind::Login,
                || {
                    register_entered_tx.send(()).unwrap();
                    register_release_rx.recv().unwrap();
                    Ok(())
                },
                || Ok(()),
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
        for (input_fails, hotkey_fails, window_fails) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
            (true, true, true),
        ] {
            let calls = Mutex::new(Vec::new());
            let error = release_login_resources_with(
                || {
                    calls.lock().unwrap().push("inputs");
                    if input_fails {
                        Err("输入释放失败".to_string())
                    } else {
                        Ok(())
                    }
                },
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
            assert_eq!(error.contains("输入释放失败"), input_fails);
            assert_eq!(error.contains("热键清理失败"), hotkey_fails);
            assert_eq!(error.contains("窗口销毁失败"), window_fails);
        }
    }

    #[test]
    fn emergency_dispatch_requests_stop_before_releasing_inputs() {
        let calls = Mutex::new(Vec::new());

        let snapshot = request_then_release_emergency(
            || {
                calls.lock().unwrap().push("request");
                Ok(42_u64)
            },
            || {
                calls.lock().unwrap().push("release");
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(snapshot, 42);
        assert_eq!(*calls.lock().unwrap(), ["request", "release"]);
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
            station_kind: None,
            ammo_target_id: None,
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
            retry_day: Some("2026-07-25".to_string()),
            retry_count: 3,
            last_failure: None,
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
        let profit_runtime = ProfitQueryControl::default();
        let settings = SpecialOpsSettings {
            paused: false,
            profit_filter: ProfitFilterSettings {
                enabled: true,
                ..ProfitFilterSettings::default()
            },
            ..SpecialOpsSettings::default()
        };

        let bootstrap =
            build_bootstrap_with_runtime(settings, 7, 0, &runtime, &profit_runtime).unwrap();

        assert_eq!(bootstrap.run_snapshot.unwrap().run_id, started.run_id);
        assert_ne!(
            bootstrap.profit_runtime.phase,
            profit::runtime::ProfitRuntimePhase::Disabled
        );
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
        let mut settings = SpecialOpsSettings {
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
        settings.default_business_config.ammo_targets =
            business_config_from_account(&settings.accounts[0]).ammo_targets;

        let snapshot = build_schedule(&settings, now);

        assert_eq!(snapshot.due_accounts.len(), 1);
        assert_eq!(snapshot.due_accounts[0].account_id, "active");
        assert_eq!(
            snapshot.due_accounts[0].station_kinds,
            vec![StationKind::TechnicalCenter, StationKind::Workbench]
        );
    }

    #[test]
    fn ammo_scroll_segments_only_scroll_down_from_ad_reset_state() {
        assert_eq!(
            ammo_reset_keys(),
            [
                crate::hotkey_types::PrimaryKey::Letter('A'),
                crate::hotkey_types::PrimaryKey::Letter('D')
            ]
        );
        assert_eq!(AMMO_RESET_KEY_DELAY_MS, 100);
        assert!(ammo_scroll_segments(0).is_empty());
        assert_eq!(ammo_scroll_segments(11), vec![(1, 11)]);
        assert_eq!(AMMO_SCROLL_STEP_INTERVAL_MS, 100);
        assert_eq!(AMMO_SCROLL_SETTLE_MS, 1_000);
    }

    #[test]
    fn business_action_migration_adds_notes_click_points_and_scroll_direction() {
        let mut value = serde_json::to_value(SpecialOpsSettings::default()).unwrap();
        value["defaultBusinessConfig"]["ammoTargets"] = serde_json::json!([{
            "id": "legacy-a",
            "name": "5.45 BT",
            "enabled": true,
            "seasonal": false,
            "scrollDirection": "up",
            "scrollSteps": 3,
            "order": 0
        }]);

        let loaded: SpecialOpsSettings = serde_json::from_value(value).unwrap();
        let once = normalize_settings(loaded).unwrap();
        let twice = normalize_settings(once.clone()).unwrap();
        let serialized = serde_json::to_value(&twice).unwrap();

        assert_eq!(twice, once);
        assert_eq!(
            serialized["defaultBusinessConfig"]["stations"][0]["recipeNote"],
            ""
        );
        let target = &serialized["defaultBusinessConfig"]["ammoTargets"][0];
        assert_eq!(target["note"], "5.45 BT");
        assert!(target["clickPoint"].is_null());
        assert_eq!(target["scrollDirection"], "down");
        assert_eq!(target["scrollSteps"], 3);
    }

    #[test]
    fn business_point_scope_updates_only_selected_ammo_target() {
        let target = AmmoBusinessTarget {
            id: "ammo-a".to_string(),
            note: "目标 A".to_string(),
            enabled: true,
            seasonal: false,
            click_point: None,
            scroll_direction: ScrollDirection::Down,
            scroll_steps: 0,
            order: 0,
            profit_rule_id: None,
        };
        let mut settings = SpecialOpsSettings::default();
        settings.default_business_config.ammo_targets = vec![target.clone()];
        let mut independent = account("independent", AccountStatus::Ready, Vec::new());
        independent.independent_settings_enabled = true;
        independent.independent_business_config = Some(BusinessConfig {
            ammo_targets: vec![target],
            ..settings.default_business_config.clone()
        });
        settings.accounts.push(independent);
        let default_point = CalibrationRect {
            x: 10,
            y: 20,
            width: 1,
            height: 1,
        };
        let account_point = CalibrationRect {
            x: 30,
            y: 40,
            width: 1,
            height: 1,
        };

        apply_ammo_business_selection(&mut settings, None, "ammo-a", default_point.clone())
            .unwrap();
        apply_ammo_business_selection(
            &mut settings,
            Some("independent"),
            "ammo-a",
            account_point.clone(),
        )
        .unwrap();

        assert_eq!(
            settings.default_business_config.ammo_targets[0].click_point,
            Some(default_point)
        );
        assert_eq!(
            settings.accounts[0]
                .independent_business_config
                .as_ref()
                .unwrap()
                .ammo_targets[0]
                .click_point,
            Some(account_point)
        );
    }

    #[test]
    fn recipe_reselection_keeps_recipe_note() {
        let mut settings = SpecialOpsSettings::default();
        let mut selected = account("selected", AccountStatus::Ready, Vec::new());
        selected.independent_settings_enabled = true;
        selected.independent_business_config = Some(settings.default_business_config.clone());
        selected
            .independent_business_config
            .as_mut()
            .unwrap()
            .stations[0]
            .recipe_note = "高级燃料".to_string();
        settings.accounts.push(selected);

        apply_account_recipe_selection(
            &mut settings,
            "selected",
            StationKind::TechnicalCenter,
            CalibrationRect {
                x: 1,
                y: 2,
                width: 1,
                height: 1,
            },
        )
        .unwrap();

        assert_eq!(
            settings.accounts[0]
                .independent_business_config
                .as_ref()
                .unwrap()
                .stations[0]
                .recipe_note,
            "高级燃料"
        );
    }

    #[test]
    fn schedule_timeline_projects_overdue_and_future_craft_tasks_for_all_statuses() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-30T10:00:00+08:00")
            .unwrap()
            .timestamp_millis();
        let settings = SpecialOpsSettings {
            enabled: true,
            paused: true,
            accounts: vec![
                account(
                    "ready",
                    AccountStatus::Ready,
                    vec![station(StationKind::TechnicalCenter, now - 60_000)],
                ),
                account(
                    "isolated",
                    AccountStatus::Isolated,
                    vec![station(StationKind::ArmorBench, now + 60 * 60_000)],
                ),
            ],
            ..SpecialOpsSettings::default()
        };

        let serialized = serde_json::to_value(build_schedule(&settings, now)).unwrap();
        let tasks = serialized["timelineTasks"].as_array().unwrap();

        assert_eq!(serialized["timelineStartMs"], now);
        assert_eq!(serialized["timelineEndMs"], now + 24 * 60 * 60_000);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0]["accountId"], "ready");
        assert_eq!(tasks[0]["scheduledAtMs"], now - 60_000);
        assert_eq!(tasks[0]["overdue"], true);
        assert_eq!(tasks[1]["accountId"], "isolated");
        assert_eq!(tasks[1]["accountStatus"], "isolated");
    }

    #[test]
    fn timeline_projects_uncertain_station_as_due_manual_task() {
        let mut selected = account(
            "selected",
            AccountStatus::Uncertain,
            vec![station(StationKind::TechnicalCenter, 100)],
        );
        selected.stations[0].status = StationStatus::Uncertain;
        selected.stations[0].finishes_at_ms = None;
        selected.last_failure = Some(AccountFailure {
            step: "craft.abort".to_string(),
            message: "制作状态未确认".to_string(),
            at_ms: 100,
            station_kind: Some(StationKind::TechnicalCenter),
            ammo_target_id: None,
        });
        let settings = SpecialOpsSettings {
            enabled: true,
            paused: true,
            accounts: vec![selected],
            ..SpecialOpsSettings::default()
        };

        let schedule = build_schedule(&settings, 1_000);
        let task = schedule
            .timeline_tasks
            .iter()
            .find(|task| task.station_kind == Some(StationKind::TechnicalCenter))
            .unwrap();

        assert_eq!(task.scheduled_at_ms, 1_000);
        assert!(task.overdue);
        assert!(task.manual_failure.is_some());
    }

    #[test]
    fn legacy_unlocated_failure_never_gets_task_level_controls() {
        let mut selected = account(
            "selected",
            AccountStatus::Uncertain,
            vec![station(StationKind::TechnicalCenter, 900)],
        );
        selected.last_failure = Some(AccountFailure {
            step: "navigation.WaitStationGrid".to_string(),
            message: "步骤超时".to_string(),
            at_ms: 100,
            station_kind: None,
            ammo_target_id: None,
        });
        let settings = SpecialOpsSettings {
            accounts: vec![selected],
            ..SpecialOpsSettings::default()
        };

        let schedule = build_schedule(&settings, 1_000);

        assert!(!schedule.timeline_tasks.is_empty());
        assert!(schedule
            .timeline_tasks
            .iter()
            .all(|task| task.manual_failure.is_none()));
    }

    #[test]
    fn timeline_projects_failed_ammo_once_even_when_retry_exhausted() {
        let mut runtime = ammo_runtime_target("ammo-a");
        runtime.enabled = true;
        runtime.last_success_day = None;
        runtime.retry_day = Some("1970-01-01".to_string());
        runtime.retry_count = 2;
        runtime.last_failure = Some(AccountFailure {
            step: "ammo.success".to_string(),
            message: "兑换状态未确认".to_string(),
            at_ms: 100,
            station_kind: None,
            ammo_target_id: Some("ammo-a".to_string()),
        });
        let selected = AccountPlan {
            ammo_targets: vec![runtime],
            ..account("selected", AccountStatus::Ready, Vec::new())
        };
        let settings = SpecialOpsSettings {
            enabled: true,
            paused: true,
            daily_exchange_time: "08:00".to_string(),
            default_business_config: business_config_from_account(&selected),
            accounts: vec![selected],
            ..SpecialOpsSettings::default()
        };

        let schedule = build_schedule(&settings, 1_000);
        let tasks = schedule
            .timeline_tasks
            .iter()
            .filter(|task| task.ammo_target_id.as_deref() == Some("ammo-a"))
            .collect::<Vec<_>>();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].scheduled_at_ms, 1_000);
        assert!(tasks[0].manual_failure.is_some());
    }

    #[test]
    fn timeline_orders_due_tasks_by_account_then_future_tasks_by_time() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-30T10:00:00+08:00")
            .unwrap()
            .timestamp_millis();
        // alpha 排在最后一位但有更早的到期时间 -> 到期桶按账号顺序，不按时间。
        let alpha = AccountPlan {
            order: 2,
            ..account(
                "alpha",
                AccountStatus::Ready,
                vec![
                    station(StationKind::TechnicalCenter, now - 10 * 60_000),
                    station(StationKind::ArmorBench, now + 30 * 60_000),
                ],
            )
        };
        let beta = AccountPlan {
            order: 0,
            ..account(
                "beta",
                AccountStatus::Ready,
                vec![station(StationKind::TechnicalCenter, now - 60_000)],
            )
        };
        let gamma = AccountPlan {
            order: 1,
            ..account(
                "gamma",
                AccountStatus::Ready,
                vec![station(StationKind::TechnicalCenter, now + 60 * 60_000)],
            )
        };
        let settings = SpecialOpsSettings {
            enabled: true,
            paused: true,
            accounts: vec![alpha, beta, gamma],
            ..SpecialOpsSettings::default()
        };

        let schedule = build_schedule(&settings, now);
        let order = schedule
            .timeline_tasks
            .iter()
            .map(|task| (task.account_id.as_str(), task.station_kind.clone()))
            .collect::<Vec<_>>();

        assert_eq!(
            order,
            vec![
                // 到期桶：按 account.order 分桶，账号内按时间。
                ("beta", Some(StationKind::TechnicalCenter)),
                ("alpha", Some(StationKind::TechnicalCenter)),
                // 未到期桶：整体排在到期任务之后，按时间优先。
                ("alpha", Some(StationKind::ArmorBench)),
                ("gamma", Some(StationKind::TechnicalCenter)),
            ]
        );
    }

    #[test]
    fn transient_round_launch_errors_never_pause_automation() {
        for error in [
            "当前没有到期制作或子弹任务",
            "特勤处当前处于暂停状态，请先点击继续",
            "特勤处总开关已关闭",
            "特勤处试运行尚未完成清理",
            "配置保存已陈旧，请刷新后重试",
        ] {
            assert!(is_transient_round_launch_error(error), "{error}");
        }
        // 真实故障必须冒泡成全局暂停，否则问题会被静默重试掩盖。
        for error in ["校准未完成", "账号配置缺失", "特勤处状态已损坏"] {
            assert!(!is_transient_round_launch_error(error), "{error}");
        }
    }

    #[test]
    fn manual_pause_toggle_clears_auto_pause_reason() {
        let mut settings = SpecialOpsSettings {
            paused: false,
            paused_reason: Some("检测到休眠或系统时间跳变".to_string()),
            ..SpecialOpsSettings::default()
        };

        apply_paused_state(&mut settings, true).unwrap();

        assert!(settings.paused);
        assert_eq!(settings.paused_reason, None);
    }

    #[test]
    fn schedule_timeline_keeps_tomorrow_ammo_after_today_succeeded() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-30T09:00:00+08:00")
            .unwrap()
            .timestamp_millis();
        let active = AccountPlan {
            ammo_targets: vec![AmmoTarget {
                id: "alpha".to_string(),
                name: "目标 A".to_string(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 0,
                last_success_day: Some("2026-07-30".to_string()),
                retry_day: Some("2026-07-30".to_string()),
                retry_count: 0,
                last_failure: None,
            }],
            ..account("active", AccountStatus::Ready, Vec::new())
        };
        let settings = SpecialOpsSettings {
            enabled: true,
            paused: false,
            daily_exchange_time: "08:00".to_string(),
            default_business_config: business_config_from_account(&active),
            accounts: vec![active],
            ..SpecialOpsSettings::default()
        };

        let snapshot = build_schedule(&settings, now);

        assert!(snapshot.due_accounts.is_empty());
        assert_eq!(snapshot.timeline_tasks.len(), 1);
        assert_eq!(snapshot.timeline_tasks[0].kind, TimelineTaskKind::Ammo);
        assert_eq!(snapshot.timeline_tasks[0].note, "目标 A");
        assert_eq!(
            snapshot.timeline_tasks[0].scheduled_at_ms,
            chrono::DateTime::parse_from_rfc3339("2026-07-31T08:00:00+08:00")
                .unwrap()
                .timestamp_millis()
        );
        assert!(!snapshot.timeline_tasks[0].overdue);
    }

    #[test]
    fn profit_timeline_projects_query_wait_qualification_and_cutoff_without_changing_runtime_plan()
    {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-30T10:00:00+08:00")
            .unwrap()
            .timestamp_millis();
        let mut active = account("active", AccountStatus::Ready, Vec::new());
        active.ammo_targets = vec![ammo_runtime_target("alpha")];
        active.ammo_targets[0].enabled = true;
        active.ammo_targets[0].last_success_day = None;
        active.ammo_targets[0].retry_day = None;
        active.ammo_targets[0].retry_count = 0;
        let mut settings = SpecialOpsSettings {
            enabled: true,
            paused: false,
            daily_exchange_time: "08:00".to_string(),
            default_business_config: business_config_from_account(&active),
            accounts: vec![active],
            ..SpecialOpsSettings::default()
        };
        settings.default_business_config.ammo_targets[0].profit_rule_id =
            Some("rule-a".to_string());
        settings.profit_filter.enabled = true;
        settings.profit_filter.cutoff_time = "17:00".to_string();
        settings.profit_filter.rules = vec![profit::model::AmmoProfitRule {
            id: "rule-a".to_string(),
            display_name: "规则 A".to_string(),
            kkrb_match_name: "KKRB A".to_string(),
            moligod_match_name: None,
            minimum_profit: 0,
        }];

        let waiting = build_schedule_with_profit_runtime(
            &settings,
            now,
            &AmmoProfitGate::Qualified(std::collections::HashSet::new()),
            Some(&ProfitRuntimeSnapshot::default()),
        );
        let task = &waiting.timeline_tasks[0];
        assert_eq!(task.profit_state, Some(TimelineProfitState::WaitingQuery));
        assert!(task.may_execute_earlier);
        assert_eq!(
            task.scheduled_at_ms,
            chrono::DateTime::parse_from_rfc3339("2026-07-30T17:00:00+08:00")
                .unwrap()
                .timestamp_millis()
        );

        let qualified = build_schedule_with_profit_runtime(
            &settings,
            now,
            &AmmoProfitGate::Qualified(std::collections::HashSet::from(["rule-a".to_string()])),
            Some(&ProfitRuntimeSnapshot {
                qualified_rule_ids: vec!["rule-a".to_string()],
                ..ProfitRuntimeSnapshot::default()
            }),
        );
        assert_eq!(
            qualified.timeline_tasks[0].profit_state,
            Some(TimelineProfitState::Qualified)
        );
        assert_eq!(qualified.timeline_tasks[0].scheduled_at_ms, now);

        let after_cutoff = chrono::DateTime::parse_from_rfc3339("2026-07-30T18:00:00+08:00")
            .unwrap()
            .timestamp_millis();
        settings.profit_filter.cutoff_state = Some(ProfitCutoffState {
            day: "2026-07-30".to_string(),
            targets: vec![ProfitCutoffTarget {
                account_id: "active".to_string(),
                target_id: "alpha".to_string(),
                rule_id: Some("rule-a".to_string()),
                skip_reason: Some(ProfitCutoffSkipReason::BelowThreshold),
                decided_at_ms: Some(after_cutoff),
            }],
        });
        let skipped = build_schedule_with_profit_runtime(
            &settings,
            after_cutoff,
            &AmmoProfitGate::QualifiedTargets(std::collections::HashSet::new()),
            Some(&ProfitRuntimeSnapshot {
                phase: profit::runtime::ProfitRuntimePhase::CutoffComplete,
                ..ProfitRuntimeSnapshot::default()
            }),
        );
        assert!(skipped
            .timeline_tasks
            .iter()
            .all(|task| task.profit_state != Some(TimelineProfitState::CutoffSkipped)));
        assert!(skipped
            .timeline_tasks
            .iter()
            .any(|task| task.scheduled_at_ms > after_cutoff));
        assert!(skipped.due_accounts.is_empty());

        settings
            .profit_filter
            .cutoff_state
            .as_mut()
            .unwrap()
            .targets[0]
            .skip_reason = None;
        settings
            .profit_filter
            .cutoff_state
            .as_mut()
            .unwrap()
            .targets[0]
            .decided_at_ms = None;
        let retry_at_ms = after_cutoff + profit::cutoff::FINAL_RETRY_DELAY_MS;
        let retrying = build_schedule_with_profit_runtime(
            &settings,
            after_cutoff,
            &AmmoProfitGate::QualifiedTargets(std::collections::HashSet::new()),
            Some(&ProfitRuntimeSnapshot {
                phase: profit::runtime::ProfitRuntimePhase::WaitingCutoffRetry,
                next_query_at_ms: Some(retry_at_ms),
                ..ProfitRuntimeSnapshot::default()
            }),
        );
        assert_eq!(
            retrying.timeline_tasks[0].profit_state,
            Some(TimelineProfitState::WaitingCutoffRetry)
        );
        assert_eq!(retrying.timeline_tasks[0].scheduled_at_ms, retry_at_ms);

        settings
            .profit_filter
            .cutoff_state
            .as_mut()
            .unwrap()
            .targets[0]
            .decided_at_ms = Some(after_cutoff);
        let qualified_after_cutoff = build_schedule_with_profit_runtime(
            &settings,
            after_cutoff,
            &AmmoProfitGate::QualifiedTargets(std::collections::HashSet::from([(
                "active".to_string(),
                "alpha".to_string(),
            )])),
            Some(&ProfitRuntimeSnapshot {
                phase: profit::runtime::ProfitRuntimePhase::CutoffComplete,
                ..ProfitRuntimeSnapshot::default()
            }),
        );
        assert_eq!(
            qualified_after_cutoff.timeline_tasks[0].profit_state,
            Some(TimelineProfitState::Qualified)
        );
        assert_eq!(
            qualified_after_cutoff.timeline_tasks[0].scheduled_at_ms,
            after_cutoff
        );
        assert_eq!(qualified_after_cutoff.due_accounts.len(), 1);
    }

    #[test]
    fn schedule_includes_only_unredeemed_ammo_after_daily_time() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-23T09:00:00+08:00")
            .unwrap()
            .timestamp_millis();
        let mut settings = SpecialOpsSettings {
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
                        retry_day: None,
                        retry_count: 0,
                        last_failure: None,
                    },
                    AmmoTarget {
                        id: "beta".to_string(),
                        name: "目标 B".to_string(),
                        enabled: true,
                        seasonal: false,
                        scroll_steps: 2,
                        order: 2,
                        last_success_day: Some("2026-07-23".to_string()),
                        retry_day: Some("2026-07-23".to_string()),
                        retry_count: 0,
                        last_failure: None,
                    },
                ],
                ..account("active", AccountStatus::Ready, Vec::new())
            }],
            ..SpecialOpsSettings::default()
        };
        settings.default_business_config.ammo_targets =
            business_config_from_account(&settings.accounts[0]).ammo_targets;

        let snapshot = build_schedule(&settings, now);

        assert_eq!(snapshot.due_accounts[0].ammo_target_ids, vec!["alpha"]);
    }

    #[test]
    fn schedule_wakes_at_daily_exchange_time_before_exchange() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-23T07:00:00+08:00")
            .unwrap()
            .timestamp_millis();
        let mut settings = SpecialOpsSettings {
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
                    retry_day: None,
                    retry_count: 0,
                    last_failure: None,
                }],
                ..account("active", AccountStatus::Ready, Vec::new())
            }],
            ..SpecialOpsSettings::default()
        };
        settings.default_business_config.ammo_targets =
            business_config_from_account(&settings.accounts[0]).ammo_targets;

        let snapshot = build_schedule(&settings, now);

        assert!(snapshot.due_accounts.is_empty());
        assert_eq!(snapshot.next_wake_at_ms, Some(now + 60 * 60 * 1000));
    }

    #[test]
    fn due_crafting_within_five_minutes_of_exchange_is_deferred() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-23T07:57:00+08:00")
            .unwrap()
            .timestamp_millis();
        let active = AccountPlan {
            ammo_targets: vec![AmmoTarget {
                id: "alpha".to_string(),
                name: "目标 A".to_string(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 0,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            }],
            ..account(
                "active",
                AccountStatus::Ready,
                vec![station(StationKind::TechnicalCenter, now - 1)],
            )
        };
        let settings = SpecialOpsSettings {
            enabled: true,
            paused: false,
            daily_exchange_time: "08:00".to_string(),
            emergency_hotkey: "Ctrl+Shift+F12".to_string(),
            default_business_config: business_config_from_account(&active),
            accounts: vec![active],
            ..SpecialOpsSettings::default()
        };

        let snapshot = build_schedule(&settings, now);

        assert!(snapshot.due_accounts.is_empty());
        assert_eq!(snapshot.next_wake_at_ms, Some(now + 3 * 60 * 1000));

        let due = build_schedule(&settings, now + 3 * 60 * 1000);
        assert_eq!(due.due_accounts.len(), 1);
        assert_eq!(
            due.due_accounts[0].station_kinds,
            [StationKind::TechnicalCenter]
        );
        assert_eq!(due.due_accounts[0].ammo_target_ids, ["alpha"]);
        assert_eq!(due.next_wake_at_ms, Some(now + 3 * 60 * 1000));
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
    fn execution_preflight_rejects_unverified_required_template() {
        let mut settings = SpecialOpsSettings {
            accounts: vec![account(
                "active",
                AccountStatus::Ready,
                vec![station(StationKind::TechnicalCenter, 1)],
            )],
            ..SpecialOpsSettings::default()
        };
        let _references = configure_required_execution_targets(&mut settings);
        assert!(validate_execution_ready(&settings).is_ok());

        calibration_target_mut(&mut settings, "craft.produce").verified_signature = None;

        assert!(validate_execution_ready(&settings)
            .unwrap_err()
            .contains("生产按钮识别与点击区域 尚未测试或验证失效"));
    }

    #[test]
    fn resume_keeps_paused_when_execution_preflight_fails() {
        let mut settings = SpecialOpsSettings {
            paused: true,
            accounts: vec![account(
                "active",
                AccountStatus::Ready,
                vec![station(StationKind::TechnicalCenter, 1)],
            )],
            ..SpecialOpsSettings::default()
        };

        let error = apply_paused_state(&mut settings, false).unwrap_err();

        assert!(error.contains("校准未完成"));
        assert!(settings.paused);
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
        let reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        std::fs::write(reference.path(), b"execution-preflight").unwrap();
        let environment = &mut settings.calibration_environments[0];
        for target in &mut environment.targets {
            target.rect = Some(CalibrationRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            });
            if target.recognition_method == Some(CalibrationRecognitionMethod::Template) {
                target.reference_image_path = Some(reference.path().display().to_string());
                target.verified_signature = Some(calibration_signature(target).unwrap());
                target.verified_at_ms = Some(1);
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
            retry_day: None,
            retry_count: 0,
            last_failure: None,
        });
        let default_business_config = business_config_from_account(&active);
        let settings = SpecialOpsSettings {
            default_business_config,
            accounts: vec![active],
            ..SpecialOpsSettings::default()
        };

        let keys = required_execution_target_keys(&settings);
        assert!(keys.contains("craft.station.technicalCenter"));
        assert!(keys.contains("craft.confirmPinned"));
        assert!(keys.contains("craft.returnToStationGrid"));
        assert!(keys.contains("craft.recipe.technicalCenter"));
        assert!(!keys.contains("craft.recipeListReady.technicalCenter"));
        assert!(!keys.contains("craft.openRecipeList.technicalCenter"));
        assert!(!keys.contains("craft.station.workbench"));
        assert!(!keys.contains("ammo.target"));
        assert!(!keys.contains("ammo.selectedTargetName"));
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
        let payload = SpecialOpsStateChanged {
            settings_revision: 17,
            now_ms: 23,
        };
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
    fn only_game_templates_require_game_test_context() {
        for key in [
            "game.stationGrid",
            "craft.abort",
            "ammo.success",
            "market.entry",
        ] {
            assert!(calibration_test_requires_game_context(key));
        }
        assert!(!calibration_test_requires_game_context("wegame.launch"));
    }

    #[test]
    fn ammo_success_diagnostic_filename_contains_safe_context() {
        assert_eq!(
            ammo_success_diagnostic_filename(1234, "3079643589"),
            "1234-3079643589-ammo.success.png"
        );
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
                target.key != "runtime.mouseParking"
                    && target.key != "game.specialOps"
                    && target.key != "ammo.supply"
                    && target.key != "ammo.enterSupply"
                    && target.key != "ammo.seasonal"
                    && target.key != "craft.confirmPinned"
                    && target.key != "craft.returnToStationGrid"
                    && !target.key.starts_with("craft.station.")
                    && !target.key.starts_with("craft.recipe.")
                    && matches!(
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
    fn beacon_mode_is_click_point_guarded_by_mode_ready() {
        let target = default_calibration_targets()
            .into_iter()
            .find(|target| target.key == "game.beaconMode")
            .unwrap();

        assert_eq!(target.kind, CalibrationTargetKind::ClickPoint);
        assert_eq!(target.guard_any_of, vec!["game.modeReady".to_string()]);
    }

    #[test]
    fn click_point_calibration_accepts_single_pixel_coordinate() {
        assert!(validate_calibration_selection(
            CalibrationTargetKind::ClickPoint,
            &CalibrationRect {
                x: 10,
                y: 20,
                width: 1,
                height: 1,
            },
        )
        .is_ok());
    }

    #[test]
    fn recognition_calibration_rejects_single_pixel_coordinate() {
        assert_eq!(
            validate_calibration_selection(
                CalibrationTargetKind::RecognitionRegion,
                &CalibrationRect {
                    x: 10,
                    y: 20,
                    width: 1,
                    height: 1,
                },
            )
            .unwrap_err(),
            "校准区域太小"
        );
    }

    #[test]
    fn normalize_clears_legacy_beacon_recognition_region() {
        let mut settings = SpecialOpsSettings::default();
        let target = calibration_target_mut(&mut settings, "game.beaconMode");
        target.kind = CalibrationTargetKind::RecognitionRegion;
        target.rect = Some(CalibrationRect {
            x: 1,
            y: 2,
            width: 30,
            height: 40,
        });
        target.reference_image_path = Some("legacy.png".to_string());
        target.verified_signature = Some("legacy".to_string());
        target.verified_at_ms = Some(1);

        let normalized = normalize_settings(settings).unwrap();
        let target = normalized.calibration_environments[0]
            .targets
            .iter()
            .find(|target| target.key == "game.beaconMode")
            .unwrap();

        assert_eq!(target.kind, CalibrationTargetKind::ClickPoint);
        assert!(target.rect.is_none());
        assert!(target.reference_image_path.is_none());
        assert!(target.verified_signature.is_none());
        assert!(target.verified_at_ms.is_none());
    }

    #[test]
    fn default_dynamic_text_targets_only_keep_remembered_account_ocr() {
        let targets = default_calibration_targets();

        assert!(!targets
            .iter()
            .any(|target| target.key == "ammo.selectedTargetName"));
        assert!(!targets.iter().any(|target| target.key == "ammo.target"));
        let target = targets
            .iter()
            .find(|target| target.key == "wegame.accountList")
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
    fn calibration_test_requires_template_region_and_reference() {
        let mut settings = SpecialOpsSettings::default();
        assert_eq!(
            calibration_test_input(&settings, "default", "wegame.loginMode").unwrap_err(),
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
        let CalibrationTestInput::Template(input) =
            calibration_test_input(&settings, "default", "wegame.loginMode").unwrap()
        else {
            panic!("模板校准目标应返回模板测试输入");
        };
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
    }

    #[test]
    fn calibration_test_accepts_account_list_ocr_region() {
        let mut settings = SpecialOpsSettings::default();
        let target = settings.calibration_environments[0]
            .targets
            .iter_mut()
            .find(|target| target.key == "wegame.accountList")
            .unwrap();
        target.rect = Some(CalibrationRect {
            x: 10,
            y: 20,
            width: 30,
            height: 40,
        });

        assert!(matches!(
            calibration_test_input(&settings, "default", "wegame.accountList"),
            Ok(CalibrationTestInput::Ocr { .. })
        ));
    }

    #[test]
    fn calibration_ocr_test_result_serializes_camel_case_fields() {
        let value = serde_json::to_value(CalibrationTestResult::Ocr {
            first_texts: vec!["3079643589".to_string()],
            second_texts: vec!["3079643589".to_string()],
            passed: true,
        })
        .unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "method": "ocr",
                "firstTexts": ["3079643589"],
                "secondTexts": ["3079643589"],
                "passed": true,
            })
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
    fn military_supply_targets_replace_legacy_tactical_and_research_points() {
        let keys = default_calibration_targets()
            .into_iter()
            .map(|target| target.key)
            .collect::<std::collections::HashSet<_>>();

        for key in [
            "ammo.enterSupply",
            "ammo.tacticalDepartment",
            "ammo.researchDepartment",
        ] {
            assert!(keys.contains(key), "缺少新军需处校准目标 {key}");
        }
        assert!(!keys.contains("ammo.tactical"));
        assert!(!keys.contains("limited.research"));
    }

    // 验证特勤处运行期不读取同步可见性，直接隐藏非主窗口和非操作提示窗。
    #[test]
    fn special_ops_window_guard_filters_operation_and_detects_selection_overlays() {
        assert!(!should_hide_for_special_ops("main"));
        assert!(!should_hide_for_special_ops("special-ops-operation"));
        assert!(should_hide_for_special_ops("timer-display-main"));
        assert!(is_active_selection_overlay("morse-overlay"));
        assert!(is_active_selection_overlay("timer-position-group"));
        assert!(is_active_selection_overlay("counter-position-group"));
        assert!(is_active_selection_overlay("rapidfire-position-group"));
        assert!(is_active_selection_overlay(
            "special-ops-calibration-default"
        ));
        assert!(!is_active_selection_overlay("timer-display-group"));
    }

    #[test]
    fn hidden_tool_windows_use_tool_reconciliation_instead_of_direct_show() {
        let plan = hidden_window_restore_plan([
            "timer-display-main".to_string(),
            "counter-display-main".to_string(),
            "rapidfire-display-main".to_string(),
            "recognition-overlay".to_string(),
            "custom-window".to_string(),
        ]);

        assert!(plan.reconcile_tool_windows);
        assert_eq!(plan.direct_labels, vec!["custom-window".to_string()]);
    }

    #[test]
    fn craft_station_click_has_no_positive_guard() {
        for kind in StationKind::all() {
            let suffix = kind.calibration_suffix();
            let guards = default_guard_any_of(&format!("craft.station.{suffix}"));
            assert!(guards.is_empty());
        }
    }

    // 验证统一资源清理会在输入、热键和操作窗口释放后执行窗口恢复闭包。
    #[test]
    fn release_login_resources_with_runs_window_restore_after_other_cleanup() {
        let events = Mutex::new(Vec::new());
        let result = release_login_resources_with(
            || {
                events.lock().unwrap().push("inputs");
                Ok(())
            },
            || {
                events.lock().unwrap().push("hotkey");
                Ok(())
            },
            || {
                events.lock().unwrap().push("window");
                Ok(())
            },
        );

        assert!(result.is_ok());
        assert_eq!(
            events.into_inner().unwrap(),
            vec!["inputs", "hotkey", "window"]
        );
    }

    // 验证固定探测只保留制作台、共享确认置顶点和每台物品选择点。
    #[test]
    fn craft_fixed_probe_removes_recipe_list_ready_targets() {
        let keys = default_calibration_targets()
            .into_iter()
            .map(|target| target.key)
            .collect::<std::collections::HashSet<_>>();

        for kind in StationKind::all() {
            let suffix = kind.calibration_suffix();
            assert!(keys.contains(&format!("craft.station.{suffix}")));
            assert!(keys.contains(&format!("craft.recipe.{suffix}")));
            assert!(!keys.contains(&format!("craft.recipeListReady.{suffix}")));
            assert!(!keys.contains(&format!("craft.openRecipeList.{suffix}")));
        }
        assert!(keys.contains("craft.confirmPinned"));
    }

    // 验证旧配置中的独立进入列表点击点会在规范化时被白名单清理。
    #[test]
    fn normalize_removes_legacy_open_recipe_list_targets() {
        let mut settings = SpecialOpsSettings::default();
        settings.calibration_environments[0]
            .targets
            .push(CalibrationTarget {
                key: "craft.openRecipeList.technicalCenter".to_string(),
                label: "旧技术中心进入制作列表点击区域".to_string(),
                kind: CalibrationTargetKind::ClickPoint,
                rect: Some(CalibrationRect {
                    x: 10,
                    y: 20,
                    width: 1,
                    height: 1,
                }),
                reference_image_path: None,
                recognition_method: None,
                guard_any_of: vec!["craft.idle.technicalCenter".to_string()],
                match_threshold: default_match_threshold(),
                verified_signature: None,
                verified_at_ms: None,
            });

        let normalized = normalize_settings(settings).unwrap();
        let keys = normalized.calibration_environments[0]
            .targets
            .iter()
            .map(|target| target.key.as_str())
            .collect::<std::collections::HashSet<_>>();

        assert!(!keys.contains("craft.openRecipeList.technicalCenter"));
        assert!(keys.contains("craft.station.technicalCenter"));
    }

    #[test]
    fn normalize_removes_legacy_claim_ready_and_in_progress_targets() {
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
            0
        );
        assert!(!targets
            .iter()
            .any(|target| target.key.starts_with("craft.inProgress.")));
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
            0
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
    fn navigation_delays_default_to_three_seconds() {
        let settings = SpecialOpsSettings::default();

        assert_eq!(settings.navigation_space_delay_ms, 3_000);
        assert_eq!(settings.navigation_tab_delay_ms, 3_000);
        assert_eq!(settings.navigation_special_ops_delay_ms, 3_000);
        assert_eq!(settings.navigation_beacon_delay_ms, 3_000);
    }

    #[test]
    fn navigation_delays_accept_boundaries_and_reject_above_sixty_seconds() {
        let mut settings = SpecialOpsSettings {
            navigation_space_delay_ms: 0,
            navigation_tab_delay_ms: 60_000,
            ..SpecialOpsSettings::default()
        };
        assert!(normalize_settings(settings.clone()).is_ok());

        settings.navigation_special_ops_delay_ms = 60_001;
        assert_eq!(
            normalize_settings(settings).unwrap_err(),
            "游戏内导航等待时间必须是 0–60000ms 的整数"
        );
    }

    #[test]
    fn ammo_entry_delays_default_to_three_seconds_and_validate_range() {
        let mut settings = SpecialOpsSettings::default();
        assert_eq!(settings.ammo_supply_delay_ms, 3_000);
        assert_eq!(settings.ammo_tactical_delay_ms, 3_000);

        settings.ammo_supply_delay_ms = 0;
        settings.ammo_tactical_delay_ms = 60_000;
        assert!(normalize_settings(settings.clone()).is_ok());

        settings.ammo_tactical_delay_ms = 60_001;
        assert_eq!(
            normalize_settings(settings).unwrap_err(),
            "子弹入口固定等待时间必须是 0–60000ms 的整数"
        );
    }

    #[test]
    fn ammo_entry_normalization_uses_shared_points_and_department_templates() {
        let settings = normalize_settings(SpecialOpsSettings::default()).unwrap();
        let targets = &settings.calibration_environments[0].targets;

        assert!(targets
            .iter()
            .all(|target| !matches!(target.key.as_str(), "ammo.list" | "ammo.seasonalList")));
        for key in ["ammo.supply", "ammo.enterSupply", "ammo.seasonal"] {
            let target = targets.iter().find(|target| target.key == key).unwrap();
            assert_eq!(target.kind, CalibrationTargetKind::ClickPoint);
            assert_eq!(target.recognition_method, None);
            assert_eq!(target.reference_image_path, None);
        }
        for key in ["ammo.tacticalDepartment", "ammo.researchDepartment"] {
            let target = targets.iter().find(|target| target.key == key).unwrap();
            assert_eq!(target.kind, CalibrationTargetKind::RecognitionRegion);
            assert_eq!(
                target.recognition_method,
                Some(CalibrationRecognitionMethod::Template)
            );
        }
    }

    #[test]
    fn ammo_retry_count_resets_when_retry_day_changes() {
        let mut target = AmmoTarget {
            id: "ammo".to_string(),
            name: "测试子弹".to_string(),
            enabled: true,
            seasonal: false,
            scroll_steps: 0,
            order: 0,
            last_success_day: None,
            retry_day: Some("2026-07-30".to_string()),
            retry_count: 2,
            last_failure: None,
        };

        prepare_ammo_retry_state(&mut target, "2026-07-31");

        assert_eq!(target.retry_day.as_deref(), Some("2026-07-31"));
        assert_eq!(target.retry_count, 0);
    }

    #[test]
    fn legacy_ammo_runtime_without_retry_day_migrates_to_none() {
        let target: AmmoTarget = serde_json::from_str(
            r#"{"id":"ammo","name":"测试子弹","enabled":true,"seasonal":false,"scrollSteps":0,"order":0,"lastSuccessDay":null,"retryCount":1}"#,
        )
        .unwrap();

        assert_eq!(target.retry_day, None);
        assert_eq!(target.retry_count, 1);
    }

    #[test]
    fn craft_probe_defaults_replace_legacy_recognition_targets() {
        let settings = SpecialOpsSettings::default();
        assert_eq!(settings.craft_space_delay_ms, 3_000);
        assert_eq!(settings.craft_reopen_delay_ms, 3_000);
        assert_eq!(settings.craft_confirm_pinned_delay_ms, 3_000);

        let targets = default_calibration_targets();
        let keys = targets
            .iter()
            .map(|target| target.key.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert!(keys.contains("craft.confirmPinned"));
        assert!(keys.contains("craft.returnToStationGrid"));
        assert!(!keys.contains("craft.reward"));
        assert!(!keys.iter().any(|key| key.starts_with("craft.inProgress.")));
        assert!(!keys
            .iter()
            .any(|key| key.starts_with("craft.recipeListReady.")));
        for kind in StationKind::all() {
            let suffix = kind.calibration_suffix();
            let recipe = targets
                .iter()
                .find(|target| target.key == format!("craft.recipe.{suffix}"))
                .unwrap();
            assert!(recipe.label.contains("制作物品选择点击点"));
            assert!(recipe.guard_any_of.is_empty());
        }
    }

    #[test]
    fn ammo_confirmation_uses_shared_template_click_target() {
        let targets = default_calibration_targets();
        let confirm = targets
            .iter()
            .find(|target| target.key == "ammo.confirm")
            .expect("默认校准必须包含子弹二次确认");
        assert_eq!(confirm.label, "兑换二次确认按钮识别与点击区域");
        assert_eq!(confirm.kind, CalibrationTargetKind::RecognitionRegion);
        assert_eq!(
            confirm.recognition_method,
            Some(CalibrationRecognitionMethod::Template)
        );

        let success = targets
            .iter()
            .find(|target| target.key == "ammo.success")
            .expect("默认校准必须包含兑换完成状态");
        assert_eq!(success.label, "兑换完成状态识别区域");
    }

    #[test]
    fn enabled_ammo_requires_confirm_template_before_execution() {
        let mut settings = LoginFixture::complete().settings;
        settings.default_business_config.ammo_targets =
            vec![ammo_business_target("normal", false, 0)];

        let keys = required_execution_target_keys(&settings);

        assert!(keys.contains("ammo.confirm"));
    }

    #[test]
    fn craft_probe_delays_accept_boundaries_and_reject_above_limit() {
        let settings = SpecialOpsSettings {
            craft_space_delay_ms: 0,
            craft_reopen_delay_ms: 60_000,
            craft_confirm_pinned_delay_ms: 0,
            ..SpecialOpsSettings::default()
        };
        assert!(normalize_settings(settings.clone()).is_ok());

        let mut invalid = settings;
        invalid.craft_confirm_pinned_delay_ms = 60_001;
        assert_eq!(
            normalize_settings(invalid).unwrap_err(),
            "制作台固定等待时间必须是 0–60000ms 的整数"
        );
    }

    #[test]
    fn navigation_targets_remove_intermediate_templates() {
        let targets = default_calibration_targets();

        assert!(!targets
            .iter()
            .any(|target| target.key == "game.activityPopup"));
        assert!(!targets.iter().any(|target| target.key == "game.startGame"));
        let target = targets
            .iter()
            .find(|target| target.key == "game.specialOps")
            .unwrap();
        assert_eq!(target.kind, CalibrationTargetKind::ClickPoint);
        assert_eq!(target.recognition_method, None);
        assert!(target.guard_any_of.is_empty());
    }

    #[test]
    fn normalize_clears_legacy_special_ops_template() {
        let mut settings = SpecialOpsSettings::default();
        let target = settings.calibration_environments[0]
            .targets
            .iter_mut()
            .find(|target| target.key == "game.specialOps")
            .unwrap();
        target.kind = CalibrationTargetKind::RecognitionRegion;
        target.rect = Some(CalibrationRect {
            x: 1,
            y: 2,
            width: 20,
            height: 20,
        });
        target.reference_image_path = Some("legacy.png".to_string());
        target.verified_signature = Some("legacy".to_string());

        let normalized = normalize_settings(settings).unwrap();
        let migrated = normalized.calibration_environments[0]
            .targets
            .iter()
            .find(|target| target.key == "game.specialOps")
            .unwrap();
        assert_eq!(migrated.kind, CalibrationTargetKind::ClickPoint);
        assert_eq!(migrated.rect, None);
        assert_eq!(migrated.reference_image_path, None);
        assert_eq!(migrated.verified_signature, None);
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
    fn default_calibration_contains_special_ops_mouse_parking_point() {
        let target = default_calibration_targets()
            .into_iter()
            .find(|target| target.key == "runtime.mouseParking")
            .expect("应包含特勤处鼠标停放点");

        assert_eq!(target.label, "特勤处鼠标停放点");
        assert_eq!(target.kind, CalibrationTargetKind::ClickPoint);
        assert!(target.reference_image_path.is_none());
        assert!(target.recognition_method.is_none());
    }

    #[test]
    fn special_ops_trials_reject_disabled_global_state() {
        assert_eq!(
            ensure_global_automation_enabled(false).unwrap_err(),
            "全局总开关已关闭"
        );
        assert!(ensure_global_automation_enabled(true).is_ok());
    }

    #[test]
    fn mouse_parking_region_requires_calibrated_point() {
        let mut fixture = LoginFixture::complete();
        let target = calibration_target_mut(&mut fixture.settings, "runtime.mouseParking");
        target.rect = None;

        assert_eq!(
            mouse_parking_region(&fixture.settings).unwrap_err(),
            "特勤处鼠标停放点尚未校准"
        );
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

    #[test]
    fn navigation_trial_preflight_requires_two_templates_and_two_click_points() {
        let mut fixture = LoginFixture::complete();
        fixture.settings.wegame_executable_path.clear();
        for key in ["game.modeReady", "game.stationGrid"] {
            let reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
            std::fs::write(reference.path(), key).unwrap();
            let target = calibration_target_mut(&mut fixture.settings, key);
            target.rect = Some(CalibrationRect {
                x: 10,
                y: 20,
                width: 30,
                height: 40,
            });
            target.reference_image_path = Some(reference.path().display().to_string());
            target.verified_signature = Some(calibration_signature(target).unwrap());
            target.verified_at_ms = Some(1);
            fixture._reference_files.push(reference);
        }
        let beacon = calibration_target_mut(&mut fixture.settings, "game.beaconMode");
        beacon.rect = Some(CalibrationRect {
            x: 10,
            y: 20,
            width: 1,
            height: 1,
        });
        let special_ops = calibration_target_mut(&mut fixture.settings, "game.specialOps");
        special_ops.rect = Some(CalibrationRect {
            x: 40,
            y: 50,
            width: 1,
            height: 1,
        });

        let config = freeze_navigation_run_config(
            &fixture.settings,
            "selected",
            game_navigation::NavigationDestination::StationGrid,
        )
        .unwrap();
        let beacon = config.targets.get("game.beaconMode").unwrap();
        let special_ops = config.targets.get("game.specialOps").unwrap();

        assert_eq!(config.targets.len(), 4);
        assert!(beacon.template.is_none());
        assert_eq!(beacon.guard_any_of, vec!["game.modeReady".to_string()]);
        assert!(special_ops.template.is_none());
        assert!(special_ops.guard_any_of.is_empty());
        assert_eq!(
            config.destination,
            game_navigation::NavigationDestination::StationGrid
        );
    }

    #[test]
    fn lobby_navigation_preflight_does_not_require_special_ops_targets() {
        let mut fixture = LoginFixture::complete();
        let reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        std::fs::write(reference.path(), "game.modeReady").unwrap();
        let ready = calibration_target_mut(&mut fixture.settings, "game.modeReady");
        ready.rect = Some(CalibrationRect {
            x: 10,
            y: 20,
            width: 30,
            height: 40,
        });
        ready.reference_image_path = Some(reference.path().display().to_string());
        ready.verified_signature = Some(calibration_signature(ready).unwrap());
        ready.verified_at_ms = Some(1);
        fixture._reference_files.push(reference);
        calibration_target_mut(&mut fixture.settings, "game.beaconMode").rect =
            Some(CalibrationRect {
                x: 10,
                y: 20,
                width: 1,
                height: 1,
            });

        let config = freeze_navigation_run_config(
            &fixture.settings,
            "selected",
            game_navigation::NavigationDestination::Lobby,
        )
        .unwrap();

        assert_eq!(config.targets.len(), 2);
        assert!(!config.targets.contains_key("game.specialOps"));
        assert!(!config.targets.contains_key("game.stationGrid"));
        assert_eq!(
            config.destination,
            game_navigation::NavigationDestination::Lobby
        );
    }

    #[test]
    fn craft_trial_preflight_uses_fixed_probe_targets() {
        let mut fixture = LoginFixture::complete();
        fixture.settings.accounts[0].stations.push(StationPlan {
            kind: StationKind::TechnicalCenter,
            enabled: true,
            item_name: String::new(),
            duration_minutes: 60,
            started_at_ms: None,
            finishes_at_ms: None,
            status: StationStatus::Ready,
        });
        fixture.settings.craft_space_delay_ms = 100;
        fixture.settings.craft_reopen_delay_ms = 200;
        fixture.settings.craft_confirm_pinned_delay_ms = 300;
        let keys = [
            "craft.station.technicalCenter",
            "craft.confirmPinned",
            "craft.returnToStationGrid",
            "craft.recipe.technicalCenter",
            "game.stationGrid",
            "craft.fill",
            "craft.purchase",
            "craft.produce",
            "craft.abort",
        ];
        for key in keys {
            let target = calibration_target_mut(&mut fixture.settings, key);
            target.rect = Some(CalibrationRect {
                x: 10,
                y: 20,
                width: 30,
                height: 40,
            });
            if target.kind != CalibrationTargetKind::ClickPoint {
                let reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
                std::fs::write(reference.path(), key).unwrap();
                target.reference_image_path = Some(reference.path().display().to_string());
                fixture._reference_files.push(reference);
            }
        }

        let (config, _) =
            freeze_craft_run_config(&fixture.settings, "selected", StationKind::TechnicalCenter)
                .unwrap();

        assert_eq!(
            config
                .targets
                .keys()
                .map(String::as_str)
                .collect::<std::collections::HashSet<_>>(),
            keys.into_iter().collect()
        );
        assert_eq!(
            config.delays,
            craft_runtime::CraftProbeDelays {
                space_ms: 100,
                reopen_ms: 200,
                confirm_pinned_ms: 300,
            }
        );
        assert!(!config.targets.contains_key("craft.reward"));
        assert!(!config
            .targets
            .keys()
            .any(|key| key.starts_with("craft.inProgress.")));
        assert!(!config
            .targets
            .keys()
            .any(|key| key.starts_with("craft.recipeListReady.")));

        for missing_key in [
            "craft.confirmPinned",
            "craft.returnToStationGrid",
            "craft.recipe.technicalCenter",
        ] {
            let mut missing = fixture.settings.clone();
            calibration_target_mut(&mut missing, missing_key).rect = None;
            assert!(
                freeze_craft_run_config(&missing, "selected", StationKind::TechnicalCenter,)
                    .err()
                    .unwrap()
                    .contains("未框选")
            );
        }

        let mut missing_abort = fixture.settings.clone();
        calibration_target_mut(&mut missing_abort, "craft.abort").reference_image_path = None;
        assert!(
            freeze_craft_run_config(&missing_abort, "selected", StationKind::TechnicalCenter,)
                .err()
                .unwrap()
                .contains("尚未上传参考图")
        );
    }

    #[test]
    fn craft_batch_preflight_freezes_all_due_stations() {
        let mut fixture = LoginFixture::complete();
        fixture.settings.accounts[0].stations = vec![
            StationPlan {
                kind: StationKind::Workbench,
                enabled: true,
                item_name: String::new(),
                duration_minutes: 120,
                started_at_ms: Some(0),
                finishes_at_ms: Some(100),
                status: StationStatus::Crafting,
            },
            StationPlan {
                kind: StationKind::TechnicalCenter,
                enabled: true,
                item_name: String::new(),
                duration_minutes: 60,
                started_at_ms: Some(0),
                finishes_at_ms: Some(100),
                status: StationStatus::Crafting,
            },
        ];
        for station in &mut fixture.settings.default_business_config.stations {
            match station.kind {
                StationKind::TechnicalCenter => station.duration_minutes = 480,
                StationKind::Workbench => station.duration_minutes = 360,
                _ => {}
            }
        }
        let keys = [
            "craft.station.technicalCenter",
            "craft.station.workbench",
            "craft.confirmPinned",
            "craft.returnToStationGrid",
            "craft.recipe.technicalCenter",
            "craft.recipe.workbench",
            "game.stationGrid",
            "craft.fill",
            "craft.purchase",
            "craft.produce",
            "craft.abort",
        ];
        for key in keys {
            let target = calibration_target_mut(&mut fixture.settings, key);
            target.rect = Some(CalibrationRect {
                x: 10,
                y: 20,
                width: 30,
                height: 40,
            });
            if target.kind != CalibrationTargetKind::ClickPoint {
                let reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
                std::fs::write(reference.path(), key).unwrap();
                target.reference_image_path = Some(reference.path().display().to_string());
                fixture._reference_files.push(reference);
            }
        }

        let frozen =
            freeze_craft_batch_run_configs(&fixture.settings, "selected", 100, None).unwrap();

        assert_eq!(frozen.len(), 2);
        assert_eq!(frozen[0].task.station, StationKind::TechnicalCenter);
        assert_eq!(frozen[0].task.duration_minutes, 480);
        assert!(frozen[0]
            .config
            .targets
            .contains_key("craft.recipe.technicalCenter"));
        assert_eq!(frozen[1].task.station, StationKind::Workbench);
        assert_eq!(frozen[1].task.duration_minutes, 360);
        assert!(frozen[1]
            .config
            .targets
            .contains_key("craft.recipe.workbench"));

        let mut missing_second_recipe = fixture.settings.clone();
        calibration_target_mut(&mut missing_second_recipe, "craft.recipe.workbench").rect = None;
        assert!(
            freeze_craft_batch_run_configs(&missing_second_recipe, "selected", 100, None)
                .err()
                .unwrap()
                .contains("craft.recipe.workbench")
        );

        fixture.settings.accounts[0]
            .stations
            .iter_mut()
            .for_each(|station| station.finishes_at_ms = Some(101));
        assert_eq!(
            freeze_craft_batch_run_configs(&fixture.settings, "selected", 100, None)
                .err()
                .unwrap(),
            "当前账号没有到期制作台"
        );
    }

    #[test]
    fn default_business_config_enables_all_four_stations() {
        let settings = SpecialOpsSettings::default();

        assert_eq!(settings.default_business_config.stations.len(), 4);
        assert!(settings
            .default_business_config
            .stations
            .iter()
            .all(|station| station.enabled));
        assert!(settings.default_business_config.ammo_targets.is_empty());
    }

    #[test]
    fn inherited_account_resolves_default_station_business_values() {
        let mut settings = SpecialOpsSettings::default();
        settings
            .default_business_config
            .stations
            .iter_mut()
            .find(|station| station.kind == StationKind::TechnicalCenter)
            .unwrap()
            .duration_minutes = 480;
        let mut inherited = account(
            "inherited",
            AccountStatus::Ready,
            vec![StationPlan {
                kind: StationKind::TechnicalCenter,
                enabled: true,
                item_name: String::new(),
                duration_minutes: 120,
                started_at_ms: Some(10),
                finishes_at_ms: Some(20),
                status: StationStatus::Crafting,
            }],
        );
        inherited.independent_settings_enabled = false;
        inherited.independent_business_config = None;
        settings.accounts.push(inherited);

        let resolved = resolve_account_business_config(&settings, &settings.accounts[0]).unwrap();

        assert_eq!(resolved.stations[0].duration_minutes, 480);
        assert_eq!(settings.accounts[0].stations[0].finishes_at_ms, Some(20));
    }

    #[test]
    fn account_recipe_selection_updates_only_independent_business_config() {
        let mut settings = LoginFixture::complete().settings;
        let global_rect = calibration_target_mut(&mut settings, "craft.recipe.technicalCenter")
            .rect
            .clone();
        settings.accounts[0].independent_settings_enabled = true;
        settings.accounts[0].independent_business_config =
            Some(settings.default_business_config.clone());
        let selected = CalibrationRect {
            x: 321,
            y: 654,
            width: 1,
            height: 1,
        };

        apply_account_recipe_selection(
            &mut settings,
            "selected",
            StationKind::TechnicalCenter,
            selected.clone(),
        )
        .unwrap();

        let business = settings.accounts[0]
            .independent_business_config
            .as_ref()
            .unwrap();
        assert_eq!(business.recipe_points.len(), 1);
        assert_eq!(business.recipe_points[0].kind, StationKind::TechnicalCenter);
        assert_eq!(business.recipe_points[0].rect, selected);
        assert_eq!(
            calibration_target_mut(&mut settings, "craft.recipe.technicalCenter").rect,
            global_rect
        );
    }

    #[test]
    fn account_recipe_selection_rejects_inherited_account() {
        let mut settings = LoginFixture::complete().settings;

        let error = apply_account_recipe_selection(
            &mut settings,
            "selected",
            StationKind::TechnicalCenter,
            CalibrationRect {
                x: 1,
                y: 2,
                width: 1,
                height: 1,
            },
        )
        .unwrap_err();

        assert!(error.contains("未开启独立设置"));
    }

    #[test]
    fn manual_correction_initializes_and_restores_uncertain_account_after_all_four_stations_are_confirmed(
    ) {
        let mut settings = SpecialOpsSettings::default();
        settings.accounts.push(account(
            "selected",
            AccountStatus::Uncertain,
            StationKind::all()
                .into_iter()
                .map(|kind| station(kind, 1))
                .collect(),
        ));
        settings.accounts[0].initialized = false;
        let corrections = vec![
            StationCorrectionInput {
                kind: StationKind::TechnicalCenter,
                state: ManualStationState::Crafting,
                remaining_minutes: Some(160),
            },
            StationCorrectionInput {
                kind: StationKind::Workbench,
                state: ManualStationState::ImmediateDue,
                remaining_minutes: None,
            },
            StationCorrectionInput {
                kind: StationKind::Pharmacy,
                state: ManualStationState::ImmediateDue,
                remaining_minutes: None,
            },
            StationCorrectionInput {
                kind: StationKind::ArmorBench,
                state: ManualStationState::ImmediateDue,
                remaining_minutes: None,
            },
        ];

        apply_manual_station_corrections(&mut settings, "selected", &corrections, 1_000).unwrap();

        let account = &settings.accounts[0];
        assert!(account.initialized);
        assert_eq!(account.status, AccountStatus::Ready);
        assert_eq!(account.stations[0].status, StationStatus::Crafting);
        assert_eq!(account.stations[0].finishes_at_ms, Some(9_601_000));
        assert!(account.stations[1..]
            .iter()
            .all(|station| station.status == StationStatus::Ready
                && station.finishes_at_ms == Some(1_000)));
        settings.paused = false;
        let plan =
            round_planner::build_round_plan(&settings, 1_000, round_planner::RoundTrigger::Manual)
                .unwrap();
        assert_eq!(plan.accounts.len(), 2);
        assert_eq!(plan.accounts[0].scheduled_at_ms, 1_000);
        assert_eq!(plan.accounts[0].stations.len(), 3);
        assert_eq!(plan.accounts[1].scheduled_at_ms, 9_601_000);
        assert_eq!(plan.accounts[1].stations, [StationKind::TechnicalCenter]);
    }

    #[test]
    fn manual_account_correction_allows_initialized_ready_account() {
        let mut settings = SpecialOpsSettings::default();
        settings.accounts.push(account(
            "selected",
            AccountStatus::Ready,
            StationKind::all()
                .into_iter()
                .map(|kind| station(kind, 1))
                .collect(),
        ));
        let corrections = StationKind::all()
            .into_iter()
            .map(|kind| StationCorrectionInput {
                kind,
                state: ManualStationState::ImmediateDue,
                remaining_minutes: None,
            })
            .collect::<Vec<_>>();

        apply_manual_account_corrections(
            &mut settings,
            "selected",
            &corrections,
            &[],
            1_000,
            "1970-01-01",
        )
        .unwrap();

        let account = &settings.accounts[0];
        assert_eq!(account.status, AccountStatus::Ready);
        assert!(account.stations.iter().all(|station| {
            station.status == StationStatus::Ready && station.finishes_at_ms == Some(1_000)
        }));
    }

    fn settings_with_station_failure(failed_kind: StationKind) -> SpecialOpsSettings {
        let mut selected = account(
            "selected",
            AccountStatus::Uncertain,
            vec![
                station(StationKind::TechnicalCenter, 100),
                station(StationKind::Workbench, 200),
            ],
        );
        selected.last_failure = Some(AccountFailure {
            step: "craft.abort".to_string(),
            message: "制作状态未确认".to_string(),
            at_ms: 100,
            station_kind: Some(failed_kind),
            ammo_target_id: None,
        });
        SpecialOpsSettings {
            accounts: vec![selected],
            ..SpecialOpsSettings::default()
        }
    }

    #[test]
    fn station_correction_keeps_other_ammo_failure() {
        let mut settings = settings_with_station_failure(StationKind::Workbench);
        let untouched = settings.accounts[0].stations[0].clone();
        let ammo_failure = AccountFailure {
            step: "ammo.success".to_string(),
            message: "兑换状态未确认".to_string(),
            at_ms: 200,
            station_kind: None,
            ammo_target_id: Some("ammo-a".to_string()),
        };
        settings.accounts[0].ammo_targets.push(AmmoTarget {
            id: "ammo-a".to_string(),
            name: "子弹 A".to_string(),
            enabled: true,
            seasonal: false,
            scroll_steps: 0,
            order: 0,
            last_success_day: None,
            retry_day: None,
            retry_count: 0,
            last_failure: Some(ammo_failure.clone()),
        });

        apply_single_station_correction(
            &mut settings,
            "selected",
            &StationCorrectionInput {
                kind: StationKind::Workbench,
                state: ManualStationState::Crafting,
                remaining_minutes: Some(160),
            },
            1_000,
        )
        .unwrap();

        let account = &settings.accounts[0];
        assert_eq!(account.status, AccountStatus::Ready);
        assert_eq!(account.stations[0], untouched);
        assert_eq!(account.stations[1].finishes_at_ms, Some(9_601_000));
        assert_eq!(
            account.ammo_targets[0].last_failure,
            Some(ammo_failure.clone())
        );
        assert_eq!(account.last_failure, Some(ammo_failure));
    }

    #[test]
    fn single_station_correction_maps_immediate_due_and_idle() {
        let due = resolve_station_correction(
            &StationCorrectionInput {
                kind: StationKind::Workbench,
                state: ManualStationState::ImmediateDue,
                remaining_minutes: None,
            },
            1_000,
            None,
        )
        .unwrap();
        assert_eq!(due, (None, Some(1_000), StationStatus::Ready));

        let idle = resolve_station_correction(
            &StationCorrectionInput {
                kind: StationKind::Workbench,
                state: ManualStationState::Idle,
                remaining_minutes: None,
            },
            1_000,
            None,
        )
        .unwrap();
        assert_eq!(idle, (None, None, StationStatus::Idle));
    }

    #[test]
    fn single_station_correction_validates_remaining_minutes_boundaries() {
        for remaining_minutes in [Some(0), Some(10_081)] {
            let error = resolve_station_correction(
                &StationCorrectionInput {
                    kind: StationKind::Workbench,
                    state: ManualStationState::Crafting,
                    remaining_minutes,
                },
                1_000,
                None,
            )
            .unwrap_err();
            assert_eq!(error, "正在制作的剩余时间必须为 1 分钟到 168 小时");
        }
        for remaining_minutes in [1, 10_080] {
            assert!(resolve_station_correction(
                &StationCorrectionInput {
                    kind: StationKind::Workbench,
                    state: ManualStationState::Crafting,
                    remaining_minutes: Some(remaining_minutes),
                },
                1_000,
                None,
            )
            .is_ok());
        }
    }

    #[test]
    fn crafting_correction_inherits_stored_remaining_time_when_unspecified() {
        let correction = StationCorrectionInput {
            kind: StationKind::Workbench,
            state: ManualStationState::Crafting,
            remaining_minutes: None,
        };

        // 有存量完成时间 → 继承，剩余时间保持异常前的进度。
        let inherited = resolve_station_correction(&correction, 1_000, Some(9_000)).unwrap();
        assert_eq!(inherited, (None, Some(9_000), StationStatus::Crafting));

        // 显式剩余时间优先于存量值。
        let explicit = resolve_station_correction(
            &StationCorrectionInput {
                remaining_minutes: Some(2),
                ..correction.clone()
            },
            1_000,
            Some(9_000),
        )
        .unwrap();
        assert_eq!(explicit, (None, Some(121_000), StationStatus::Crafting));

        // 存量完成时间已过或缺失 → 无可继承值，要求填写。
        for stored in [None, Some(1_000), Some(500)] {
            let error = resolve_station_correction(&correction, 1_000, stored).unwrap_err();
            assert_eq!(error, "缺少可继承的剩余时间，请填写 1 分钟到 168 小时");
        }
    }

    #[test]
    fn single_station_correction_rejects_wrong_failure_locator() {
        let mut settings = settings_with_station_failure(StationKind::Workbench);
        let error = apply_single_station_correction(
            &mut settings,
            "selected",
            &StationCorrectionInput {
                kind: StationKind::TechnicalCenter,
                state: ManualStationState::ImmediateDue,
                remaining_minutes: None,
            },
            1_000,
        )
        .unwrap_err();

        assert_eq!(error, "当前制作台没有待人工判定的失败记录");
    }

    #[test]
    fn single_station_correction_cannot_bypass_login_failures() {
        for status in [AccountStatus::NeedsManualLogin, AccountStatus::LoginFailed] {
            let mut settings = settings_with_station_failure(StationKind::Workbench);
            settings.accounts[0].status = status;
            let error = apply_single_station_correction(
                &mut settings,
                "selected",
                &StationCorrectionInput {
                    kind: StationKind::Workbench,
                    state: ManualStationState::ImmediateDue,
                    remaining_minutes: None,
                },
                1_000,
            )
            .unwrap_err();
            assert_eq!(error, "需要人工登录或登录失败账号不能通过单项校正恢复");
        }
    }

    #[test]
    fn account_manual_check_only_restores_account_level_failures() {
        for status in [
            AccountStatus::NeedsManualLogin,
            AccountStatus::LoginFailed,
            AccountStatus::ManualCheckRequired,
        ] {
            let mut settings = SpecialOpsSettings::default();
            let mut selected = account(
                "selected",
                status,
                vec![station(StationKind::Workbench, 9_000)],
            );
            selected.last_failure = Some(AccountFailure {
                step: "navigation.WaitStationGrid".to_string(),
                message: "步骤超时".to_string(),
                at_ms: 1,
                station_kind: None,
                ammo_target_id: None,
            });
            let stations_before = selected.stations.clone();
            settings.accounts.push(selected);

            apply_account_manual_check(&mut settings, "selected", 1_000).unwrap();

            assert_eq!(settings.accounts[0].status, AccountStatus::Ready);
            assert_eq!(settings.accounts[0].stations, stations_before);
            assert_eq!(settings.accounts[0].last_failure, None);
        }

        let mut settings = SpecialOpsSettings::default();
        settings
            .accounts
            .push(account("selected", AccountStatus::Uncertain, Vec::new()));
        assert!(apply_account_manual_check(&mut settings, "selected", 1_000).is_err());
    }

    #[test]
    fn account_manual_check_restores_uncertain_stations_from_stored_timing() {
        let mut settings = SpecialOpsSettings::default();
        let mut selected = account(
            "selected",
            AccountStatus::ManualCheckRequired,
            vec![
                station(StationKind::Workbench, 9_000),
                station(StationKind::Pharmacy, 500),
                station(StationKind::ArmorBench, 9_000),
            ],
        );
        selected.stations[0].status = StationStatus::Uncertain;
        selected.stations[1].status = StationStatus::Uncertain;
        selected.stations[2].status = StationStatus::Uncertain;
        selected.stations[2].started_at_ms = None;
        selected.stations[2].finishes_at_ms = None;
        settings.accounts.push(selected);

        apply_account_manual_check(&mut settings, "selected", 1_000).unwrap();

        let stations = &settings.accounts[0].stations;
        // 完成时间还在未来 → 继续制作，剩余时间沿用存量 finishes_at_ms。
        assert_eq!(stations[0].status, StationStatus::Crafting);
        assert_eq!(stations[0].finishes_at_ms, Some(9_000));
        // 完成时间已过 → 待收取。
        assert_eq!(stations[1].status, StationStatus::Ready);
        // 无计时 → 空闲。
        assert_eq!(stations[2].status, StationStatus::Idle);
    }

    #[test]
    fn restore_account_state_clears_all_manual_blockers() {
        let mut settings = SpecialOpsSettings::default();
        let mut selected = account(
            "selected",
            AccountStatus::NeedsManualLogin,
            vec![station(StationKind::Workbench, 9_000)],
        );
        selected.stations[0].status = StationStatus::Uncertain;
        selected.last_failure = Some(AccountFailure {
            step: "login.WaitLobby".to_string(),
            message: "步骤超时".to_string(),
            at_ms: 1,
            station_kind: None,
            ammo_target_id: None,
        });
        selected.ammo_targets = vec![AmmoTarget {
            id: "ammo-a".to_string(),
            name: "ammo-a".to_string(),
            enabled: true,
            seasonal: false,
            scroll_steps: 0,
            order: 0,
            last_success_day: Some("2026-08-09".to_string()),
            retry_day: Some("2026-08-09".to_string()),
            retry_count: 2,
            last_failure: Some(AccountFailure {
                step: "ammo.Confirm".to_string(),
                message: "确认异常".to_string(),
                at_ms: 2,
                station_kind: None,
                ammo_target_id: Some("ammo-a".to_string()),
            }),
        }];
        selected.limited_supply = LimitedSupplyAccountState {
            outcome: limited_supply::LimitedSupplyOutcome::Failed,
            ..Default::default()
        };
        settings.accounts.push(selected);
        settings
            .accounts
            .push(account("other", AccountStatus::Ready, Vec::new()));

        let restored =
            restore_account_state(&mut settings, Some("selected"), 1_000, "2026-08-09").unwrap();

        assert_eq!(restored, 1);
        let selected = &settings.accounts[0];
        assert_eq!(selected.status, AccountStatus::Ready);
        assert_eq!(selected.last_failure, None);
        assert_eq!(selected.stations[0].status, StationStatus::Crafting);
        assert_eq!(selected.stations[0].finishes_at_ms, Some(9_000));
        // 目标全量解冻回未兑换：当天成功标记一起清，重复兑换由流程内检查分支兜底。
        assert_eq!(selected.ammo_targets[0].last_success_day, None);
        assert_eq!(
            selected.ammo_targets[0].retry_day.as_deref(),
            Some("2026-08-09")
        );
        assert_eq!(selected.ammo_targets[0].retry_count, 0);
        assert_eq!(selected.ammo_targets[0].last_failure, None);
        assert_eq!(
            selected.limited_supply.outcome,
            limited_supply::LimitedSupplyOutcome::Pending
        );

        // 无异常时报错，避免空转产生一次 settings revision。
        assert!(restore_account_state(&mut settings, Some("other"), 1_000, "2026-08-09").is_err());
        assert!(
            restore_account_state(&mut settings, Some("missing"), 1_000, "2026-08-09").is_err()
        );
    }

    fn settings_with_two_failed_ammo_targets() -> SpecialOpsSettings {
        let mut selected = account("selected", AccountStatus::Ready, Vec::new());
        selected.ammo_targets = ["ammo-a", "ammo-b"]
            .into_iter()
            .enumerate()
            .map(|(index, id)| AmmoTarget {
                id: id.to_string(),
                name: id.to_string(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: index as u32,
                last_success_day: None,
                retry_day: Some("2026-08-05".to_string()),
                retry_count: 1,
                last_failure: Some(AccountFailure {
                    step: "ammo.success".to_string(),
                    message: "兑换状态未确认".to_string(),
                    at_ms: 100 + index as i64,
                    station_kind: None,
                    ammo_target_id: Some(id.to_string()),
                }),
            })
            .collect();
        selected.last_failure = selected.ammo_targets[1].last_failure.clone();
        SpecialOpsSettings {
            default_business_config: business_config_from_account(&selected),
            accounts: vec![selected],
            ..SpecialOpsSettings::default()
        }
    }

    #[test]
    fn single_ammo_correction_clears_only_selected_failure() {
        let mut settings = settings_with_two_failed_ammo_targets();

        apply_single_ammo_correction(
            &mut settings,
            "selected",
            &AmmoCorrectionInput {
                target_id: "ammo-a".to_string(),
                succeeded_today: true,
            },
            "2026-08-05",
        )
        .unwrap();

        let account = &settings.accounts[0];
        assert_eq!(
            account.ammo_targets[0].last_success_day.as_deref(),
            Some("2026-08-05")
        );
        assert_eq!(account.ammo_targets[0].last_failure, None);
        assert!(account.ammo_targets[1].last_failure.is_some());
        assert_eq!(
            account
                .last_failure
                .as_ref()
                .and_then(|failure| failure.ammo_target_id.as_deref()),
            Some("ammo-b")
        );
    }

    #[test]
    fn ammo_correction_false_reenters_today_schedule() {
        let mut settings = settings_with_two_failed_ammo_targets();
        settings.enabled = true;
        settings.paused = false;
        settings.daily_exchange_time = "08:00".to_string();
        settings.accounts[0].ammo_targets[0].last_success_day = Some("2026-08-05".to_string());
        settings.accounts[0].ammo_targets[0].retry_count = 2;

        apply_single_ammo_correction(
            &mut settings,
            "selected",
            &AmmoCorrectionInput {
                target_id: "ammo-a".to_string(),
                succeeded_today: false,
            },
            "2026-08-05",
        )
        .unwrap();

        let target = &settings.accounts[0].ammo_targets[0];
        assert_eq!(target.last_success_day, None);
        assert_eq!(target.retry_day.as_deref(), Some("2026-08-05"));
        assert_eq!(target.retry_count, 0);
        assert_eq!(target.last_failure, None);
        let now = chrono::DateTime::parse_from_rfc3339("2026-08-05T09:00:00+08:00")
            .unwrap()
            .timestamp_millis();
        let schedule = build_schedule(&settings, now);
        assert!(schedule.due_accounts.iter().any(|account| {
            account.account_id == "selected"
                && account
                    .ammo_target_ids
                    .iter()
                    .any(|target| target == "ammo-a")
        }));
    }

    #[test]
    fn full_manual_correction_clears_all_target_failures() {
        let mut settings = settings_with_two_failed_ammo_targets();
        settings.accounts[0].stations = StationKind::all()
            .into_iter()
            .map(|kind| station(kind, 1))
            .collect();
        settings.default_business_config = business_config_from_account(&settings.accounts[0]);
        let stations = StationKind::all()
            .into_iter()
            .map(|kind| StationCorrectionInput {
                kind,
                state: ManualStationState::ImmediateDue,
                remaining_minutes: None,
            })
            .collect::<Vec<_>>();
        let ammo = ["ammo-a", "ammo-b"]
            .into_iter()
            .map(|target_id| AmmoCorrectionInput {
                target_id: target_id.to_string(),
                succeeded_today: false,
            })
            .collect::<Vec<_>>();

        apply_manual_account_corrections(
            &mut settings,
            "selected",
            &stations,
            &ammo,
            1_000,
            "2026-08-05",
        )
        .unwrap();

        assert!(settings.accounts[0]
            .ammo_targets
            .iter()
            .all(|target| target.last_failure.is_none()));
        assert_eq!(settings.accounts[0].last_failure, None);
    }

    #[test]
    fn normal_ammo_retry_failure_does_not_require_manual_correction() {
        let mut selected = account("selected", AccountStatus::Ready, Vec::new());
        let mut runtime = ammo_runtime_target("ammo-a");
        runtime.enabled = true;
        runtime.last_success_day = None;
        runtime.retry_day = None;
        runtime.retry_count = 0;
        selected.ammo_targets = vec![runtime];
        let mut settings = SpecialOpsSettings {
            enabled: true,
            paused: false,
            daily_exchange_time: "08:00".to_string(),
            default_business_config: BusinessConfig {
                ammo_targets: vec![ammo_business_target("ammo-a", false, 0)],
                ..BusinessConfig::default()
            },
            accounts: vec![selected],
            ..SpecialOpsSettings::default()
        };

        apply_ammo_failure(
            &mut settings,
            "selected",
            "ammo-a",
            "2026-08-05",
            "ammo.exchange",
            "本次兑换失败",
            1_000,
        )
        .unwrap();

        let target = &settings.accounts[0].ammo_targets[0];
        assert_eq!(settings.accounts[0].status, AccountStatus::Ready);
        assert_eq!(target.retry_count, 1);
        assert_eq!(target.last_failure, None);
    }

    #[test]
    fn legacy_json_uses_first_account_business_values_as_defaults() {
        let mut legacy = SpecialOpsSettings::default();
        legacy.accounts.push(account(
            "first",
            AccountStatus::Ready,
            vec![StationPlan {
                kind: StationKind::TechnicalCenter,
                enabled: true,
                item_name: String::new(),
                duration_minutes: 240,
                started_at_ms: Some(10),
                finishes_at_ms: Some(20),
                status: StationStatus::Crafting,
            }],
        ));
        let mut json = serde_json::to_value(legacy).unwrap();
        json.as_object_mut()
            .unwrap()
            .remove("defaultBusinessConfig");
        let parsed: SpecialOpsSettings = serde_json::from_value(json).unwrap();

        let normalized = normalize_settings(parsed).unwrap();

        let technical = normalized
            .default_business_config
            .stations
            .iter()
            .find(|station| station.kind == StationKind::TechnicalCenter)
            .unwrap();
        assert_eq!(technical.duration_minutes, 240);
        assert!(!normalized.accounts[0].independent_settings_enabled);
        assert_eq!(normalized.accounts[0].stations[0].finishes_at_ms, Some(20));
    }

    fn ammo_business_target(id: &str, seasonal: bool, order: u32) -> AmmoBusinessTarget {
        AmmoBusinessTarget {
            id: id.to_string(),
            note: format!("目标 {id}"),
            enabled: true,
            seasonal,
            click_point: Some(CalibrationRect {
                x: 100 + order as i32,
                y: 200,
                width: 1,
                height: 1,
            }),
            scroll_direction: ScrollDirection::Down,
            scroll_steps: order,
            order,
            profit_rule_id: None,
        }
    }

    fn ammo_runtime_target(id: &str) -> AmmoTarget {
        AmmoTarget {
            id: id.to_string(),
            name: format!("旧目标 {id}"),
            enabled: false,
            seasonal: false,
            scroll_steps: 99,
            order: 99,
            last_success_day: Some("2026-07-30".to_string()),
            retry_day: Some("2026-07-31".to_string()),
            retry_count: 1,
            last_failure: None,
        }
    }

    #[test]
    fn cutoff_state_freezes_initial_targets_and_qualifies_by_account_and_target() {
        let day = "2026-08-06";
        let mut settings = SpecialOpsSettings::default();
        settings.profit_filter.enabled = true;
        settings.profit_filter.rules = vec![profit::model::AmmoProfitRule {
            id: "rule-a".to_string(),
            display_name: "规则 A".to_string(),
            kkrb_match_name: "KKRB A".to_string(),
            moligod_match_name: None,
            minimum_profit: 1,
        }];
        settings.default_business_config.ammo_targets = vec![AmmoBusinessTarget {
            profit_rule_id: Some("rule-a".to_string()),
            ..ammo_business_target("alpha", false, 0)
        }];
        let mut ready = account("ready", AccountStatus::Ready, Vec::new());
        ready.ammo_targets = vec![ammo_runtime_target("alpha")];
        settings.accounts = vec![ready];

        let mut cutoff = build_profit_cutoff_state(&settings, day, 1_000);
        cutoff.targets[0].decided_at_ms = Some(2_000);
        settings.profit_filter.cutoff_state = Some(cutoff);

        settings
            .default_business_config
            .ammo_targets
            .push(AmmoBusinessTarget {
                profit_rule_id: Some("rule-a".to_string()),
                ..ammo_business_target("beta", false, 1)
            });
        settings.accounts[0]
            .ammo_targets
            .push(ammo_runtime_target("beta"));

        assert_eq!(
            settings
                .profit_filter
                .cutoff_state
                .as_ref()
                .unwrap()
                .targets
                .len(),
            1
        );
        assert_eq!(
            cutoff_qualified_targets(&settings, day),
            std::collections::HashSet::from([("ready".to_string(), "alpha".to_string())])
        );
    }

    #[test]
    fn pending_profit_rules_only_include_ready_configured_retryable_targets_once() {
        let day = "2026-08-02";
        let mut settings = SpecialOpsSettings::default();
        settings.default_business_config.ammo_targets = vec![
            AmmoBusinessTarget {
                profit_rule_id: Some("rule-a".to_string()),
                ..ammo_business_target("shared", false, 0)
            },
            AmmoBusinessTarget {
                click_point: None,
                profit_rule_id: Some("rule-b".to_string()),
                ..ammo_business_target("missing-point", false, 1)
            },
            AmmoBusinessTarget {
                profit_rule_id: Some("rule-c".to_string()),
                ..ammo_business_target("exhausted", false, 2)
            },
        ];
        settings.profit_filter.enabled = true;
        settings.profit_filter.rules = vec![
            profit::model::AmmoProfitRule {
                id: "rule-a".to_string(),
                display_name: "规则 A".to_string(),
                kkrb_match_name: "KKRB A".to_string(),
                moligod_match_name: None,
                minimum_profit: 1,
            },
            profit::model::AmmoProfitRule {
                id: "rule-b".to_string(),
                display_name: "规则 B".to_string(),
                kkrb_match_name: "KKRB B".to_string(),
                moligod_match_name: None,
                minimum_profit: 1,
            },
            profit::model::AmmoProfitRule {
                id: "rule-c".to_string(),
                display_name: "规则 C".to_string(),
                kkrb_match_name: "KKRB C".to_string(),
                moligod_match_name: None,
                minimum_profit: 1,
            },
        ];

        let mut ready = account("ready", AccountStatus::Ready, Vec::new());
        ready.ammo_targets = vec![
            AmmoTarget {
                id: "shared".to_string(),
                name: String::new(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 0,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            },
            AmmoTarget {
                id: "exhausted".to_string(),
                name: String::new(),
                enabled: true,
                seasonal: false,
                scroll_steps: 2,
                order: 2,
                last_success_day: None,
                retry_day: Some(day.to_string()),
                retry_count: 2,
                last_failure: None,
            },
        ];
        let mut duplicate = ready.clone();
        duplicate.id = "duplicate".to_string();
        let mut succeeded = ready.clone();
        succeeded.id = "succeeded".to_string();
        succeeded.ammo_targets[0].last_success_day = Some(day.to_string());
        let mut uninitialized = ready.clone();
        uninitialized.id = "uninitialized".to_string();
        uninitialized.initialized = false;
        let mut isolated = ready.clone();
        isolated.id = "isolated".to_string();
        isolated.status = AccountStatus::Isolated;
        settings.accounts = vec![ready, duplicate, succeeded, uninitialized, isolated];

        let rules = collect_pending_profit_rules(&settings, day);

        assert_eq!(
            rules
                .iter()
                .map(|rule| rule.id.as_str())
                .collect::<Vec<_>>(),
            ["rule-a"]
        );
    }

    #[test]
    fn normalize_syncs_inherited_ammo_runtime_with_business_targets() {
        let mut fixture = LoginFixture::complete();
        fixture.settings.default_business_config.ammo_targets = vec![
            ammo_business_target("kept", false, 0),
            ammo_business_target("new", true, 1),
        ];
        fixture.settings.accounts[0].ammo_targets =
            vec![ammo_runtime_target("orphan"), ammo_runtime_target("kept")];

        let normalized = normalize_settings(fixture.settings).unwrap();
        let targets = &normalized.accounts[0].ammo_targets;

        assert_eq!(
            targets
                .iter()
                .map(|target| target.id.as_str())
                .collect::<Vec<_>>(),
            ["kept", "new"]
        );
        assert_eq!(targets[0].name, "目标 kept");
        assert!(targets[0].enabled);
        assert_eq!(targets[0].scroll_steps, 0);
        assert_eq!(targets[0].last_success_day.as_deref(), Some("2026-07-30"));
        assert_eq!(targets[0].retry_day.as_deref(), Some("2026-07-31"));
        assert_eq!(targets[0].retry_count, 1);
        assert_eq!(targets[1].last_success_day, None);
        assert_eq!(targets[1].retry_day, None);
        assert_eq!(targets[1].retry_count, 0);
    }

    #[test]
    fn normalize_syncs_independent_ammo_runtime_with_effective_targets() {
        let mut fixture = LoginFixture::complete();
        fixture.settings.default_business_config.ammo_targets =
            vec![ammo_business_target("default", false, 0)];
        fixture.settings.accounts[0].independent_settings_enabled = true;
        fixture.settings.accounts[0].independent_business_config = Some(BusinessConfig {
            stations: fixture.settings.default_business_config.stations.clone(),
            recipe_points: Vec::new(),
            ammo_targets: vec![ammo_business_target("independent", true, 0)],
            market: Default::default(),
        });
        fixture.settings.accounts[0].ammo_targets = vec![ammo_runtime_target("default")];

        let normalized = normalize_settings(fixture.settings).unwrap();
        let targets = &normalized.accounts[0].ammo_targets;

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id, "independent");
        assert_eq!(targets[0].name, "目标 independent");
        assert!(targets[0].seasonal);
        assert_eq!(targets[0].last_success_day, None);
    }

    #[test]
    fn ammo_trial_freezes_ready_account_targets_and_daily_state() {
        let mut fixture = LoginFixture::complete();
        fixture.settings.default_business_config.ammo_targets = vec![
            ammo_business_target("seasonal", true, 2),
            ammo_business_target("normal", false, 1),
        ];
        fixture.settings.accounts[0].ammo_targets = vec![
            AmmoTarget {
                id: "normal".to_string(),
                name: "普通".to_string(),
                enabled: true,
                seasonal: false,
                scroll_steps: 1,
                order: 1,
                last_success_day: Some("1970-01-01".to_string()),
                retry_day: Some("1970-01-01".to_string()),
                retry_count: 1,
                last_failure: None,
            },
            AmmoTarget {
                id: "seasonal".to_string(),
                name: "赛季".to_string(),
                enabled: true,
                seasonal: true,
                scroll_steps: 2,
                order: 2,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            },
        ];
        fixture._reference_files = configure_required_execution_targets(&mut fixture.settings);

        let frozen = freeze_ammo_run(&fixture.settings, "selected", 0, None).unwrap();

        assert_eq!(frozen.day, "1970-01-01");
        assert_eq!(
            frozen
                .ammo_targets
                .iter()
                .map(|target| target.id.as_str())
                .collect::<Vec<_>>(),
            ["normal", "seasonal"]
        );
        assert!(frozen.ammo_targets[0].already_succeeded);
        assert_eq!(frozen.ammo_targets[0].retry_count, 1);
        assert!(!frozen.targets.contains_key("ammo.list"));
        assert!(!frozen.targets.contains_key("ammo.seasonalList"));
        assert!(frozen.targets["ammo.tacticalDepartment"].template.is_some());
        assert!(frozen.targets["ammo.seasonal"].template.is_none());
        let entry = freeze_military_supply_entry(&fixture.settings).unwrap();
        assert!(entry.targets["ammo.department"].template.is_some());
        assert!(entry.targets["ammo.supply"].template.is_none());
        assert!(entry.targets["ammo.enterSupply"].template.is_none());
        assert_eq!(entry.config.supply_delay.as_millis(), 3_000);
        assert_eq!(entry.config.enter_supply_delay.as_millis(), 3_000);
    }

    #[test]
    fn ammo_trial_preflight_rejects_non_ready_and_missing_click_points() {
        let mut fixture = LoginFixture::complete();
        fixture.settings.default_business_config.ammo_targets =
            vec![ammo_business_target("seasonal", true, 1)];
        fixture._reference_files = configure_required_execution_targets(&mut fixture.settings);

        fixture.settings.accounts[0].status = AccountStatus::Isolated;
        assert!(freeze_ammo_run(&fixture.settings, "selected", 0, None)
            .unwrap_err()
            .contains("Ready"));

        fixture.settings.accounts[0].status = AccountStatus::Ready;
        fixture.settings.default_business_config.ammo_targets[0].click_point = None;
        assert!(freeze_ammo_run(&fixture.settings, "selected", 0, None)
            .unwrap_err()
            .contains("点击点"));

        fixture.settings.default_business_config.ammo_targets[0].click_point =
            Some(CalibrationRect {
                x: 1,
                y: 2,
                width: 1,
                height: 1,
            });
        calibration_target_mut(&mut fixture.settings, "ammo.seasonal").rect = None;
        assert!(freeze_ammo_run(&fixture.settings, "selected", 0, None)
            .unwrap_err()
            .contains("ammo.seasonal"));
    }

    #[test]
    fn round_freezes_only_planned_ammo_targets() {
        let mut fixture = LoginFixture::complete();
        fixture.settings.paused = false;
        fixture.settings.default_business_config.ammo_targets = vec![
            ammo_business_target("pending", false, 0),
            ammo_business_target("done", false, 1),
        ];
        fixture.settings.accounts[0].ammo_targets = vec![
            AmmoTarget {
                id: "pending".to_string(),
                name: String::new(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 0,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            },
            AmmoTarget {
                id: "done".to_string(),
                name: String::new(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 1,
                last_success_day: Some("1970-01-01".to_string()),
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            },
        ];
        fixture._reference_files = configure_required_execution_targets(&mut fixture.settings);

        let frozen = freeze_round_run(
            &fixture.settings,
            0,
            round_planner::RoundTrigger::Manual,
            AmmoProfitGate::Disabled,
            None,
        )
        .unwrap();
        let task = &frozen.plan.accounts[0];
        let account = frozen
            .accounts
            .get(&(task.account_id.clone(), task.scheduled_at_ms))
            .unwrap()
            .as_ref()
            .unwrap();

        assert_eq!(task.ammo_target_ids, ["pending"]);
        assert_eq!(
            account
                .ammo
                .as_ref()
                .unwrap()
                .ammo_targets
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["pending"]
        );
    }

    #[test]
    fn round_freezes_each_same_account_time_bucket_without_overwrite() {
        let mut fixture = LoginFixture::complete();
        fixture.settings.paused = false;
        fixture.settings.accounts[0].stations = vec![
            station(StationKind::TechnicalCenter, 1_000),
            station(StationKind::Workbench, 5_000),
        ];
        fixture._reference_files = configure_required_execution_targets(&mut fixture.settings);

        let frozen = freeze_round_run(
            &fixture.settings,
            1_000,
            round_planner::RoundTrigger::Scheduled,
            AmmoProfitGate::Disabled,
            None,
        )
        .unwrap();

        assert_eq!(
            frozen
                .plan
                .accounts
                .iter()
                .map(|task| (task.scheduled_at_ms, task.stations.clone()))
                .collect::<Vec<_>>(),
            [
                (1_000, vec![StationKind::TechnicalCenter]),
                (5_000, vec![StationKind::Workbench]),
            ]
        );
        for task in &frozen.plan.accounts {
            let account = frozen
                .accounts
                .get(&(task.account_id.clone(), task.scheduled_at_ms))
                .unwrap()
                .as_ref()
                .unwrap();
            assert_eq!(
                account
                    .craft
                    .iter()
                    .map(|item| item.task.station.clone())
                    .collect::<Vec<_>>(),
                task.stations
            );
        }
    }

    #[test]
    fn round_freezes_all_overdue_stations_in_one_account_bucket() {
        let mut fixture = LoginFixture::complete();
        fixture.settings.paused = false;
        // 两个制作台都已到期但完成时间不同：桶的 scheduled_at_ms 取最早值 1_000，
        // 直接拿它当 frozen_now 过滤会丢掉 3_000 那台，退化成一台一轮。
        fixture.settings.accounts[0].stations = vec![
            station(StationKind::TechnicalCenter, 1_000),
            station(StationKind::Workbench, 3_000),
        ];
        fixture._reference_files = configure_required_execution_targets(&mut fixture.settings);

        let frozen = freeze_round_run(
            &fixture.settings,
            5_000,
            round_planner::RoundTrigger::Scheduled,
            AmmoProfitGate::Disabled,
            None,
        )
        .unwrap();

        assert_eq!(
            frozen
                .plan
                .accounts
                .iter()
                .map(|task| (task.scheduled_at_ms, task.stations.clone()))
                .collect::<Vec<_>>(),
            [(
                1_000,
                vec![StationKind::TechnicalCenter, StationKind::Workbench]
            )]
        );
        let account = frozen
            .accounts
            .get(&("selected".to_string(), 1_000))
            .unwrap()
            .as_ref()
            .unwrap();
        assert_eq!(
            account
                .craft
                .iter()
                .map(|item| item.task.station.clone())
                .collect::<Vec<_>>(),
            vec![StationKind::TechnicalCenter, StationKind::Workbench]
        );
    }

    #[test]
    fn ammo_isolation_keeps_account_isolated_in_round_failure_mapping() {
        let mut settings = LoginFixture::complete().settings;
        let error =
            round_runner::AccountRunError::account("ammo.isolated", "材料购买重试后仍无法继续");

        apply_round_account_failure(&mut settings, "selected", &error, 1_000).unwrap();

        assert_eq!(settings.accounts[0].status, AccountStatus::Isolated);
    }

    #[test]
    fn round_ammo_failure_blocks_only_target() {
        let mut settings = LoginFixture::complete().settings;
        settings.default_business_config.ammo_targets = vec![
            ammo_business_target("ammo-a", false, 0),
            ammo_business_target("ammo-b", false, 1),
        ];
        settings.accounts[0].ammo_targets = vec![
            AmmoTarget {
                id: "ammo-a".to_string(),
                name: "子弹 A".to_string(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 0,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            },
            AmmoTarget {
                id: "ammo-b".to_string(),
                name: "子弹 B".to_string(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 1,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            },
        ];
        let error =
            round_runner::AccountRunError::account_ammo("ammo-a", "ammo.success", "完成状态未命中");

        apply_round_account_failure(&mut settings, "selected", &error, 1_000).unwrap();

        let account = &settings.accounts[0];
        assert_eq!(account.status, AccountStatus::Ready);
        assert!(account.ammo_targets[0].last_failure.is_some());
        assert!(account.ammo_targets[1].last_failure.is_none());
    }

    #[test]
    fn round_ammo_isolation_marks_account_isolated() {
        let mut settings = LoginFixture::complete().settings;
        settings.accounts[0].ammo_targets = vec![AmmoTarget {
            id: "ammo-a".to_string(),
            name: "子弹 A".to_string(),
            enabled: true,
            seasonal: false,
            scroll_steps: 0,
            order: 0,
            last_success_day: None,
            retry_day: None,
            retry_count: 0,
            last_failure: None,
        }];
        let error =
            round_runner::AccountRunError::account_ammo("ammo-a", "ammo.isolated", "仓库空间不足");

        apply_round_account_failure(&mut settings, "selected", &error, 1_000).unwrap();

        assert_eq!(settings.accounts[0].status, AccountStatus::Isolated);
        assert!(settings.accounts[0].ammo_targets[0].last_failure.is_some());
    }

    #[test]
    fn craft_isolation_keeps_station_state_unchanged_in_round_failure_mapping() {
        let mut settings = SpecialOpsSettings {
            accounts: vec![account(
                "selected",
                AccountStatus::Ready,
                vec![station(StationKind::Workbench, 100)],
            )],
            ..SpecialOpsSettings::default()
        };
        let initial_station_status = settings.accounts[0]
            .stations
            .iter()
            .find(|station| station.kind == StationKind::Workbench)
            .unwrap()
            .status
            .clone();
        let error = round_runner::AccountRunError::account_station(
            StationKind::Workbench,
            "craft.isolated",
            "材料购买重试后仍停留在购买页面",
        );

        apply_round_account_failure(&mut settings, "selected", &error, 1_000).unwrap();

        assert_eq!(settings.accounts[0].status, AccountStatus::Isolated);
        assert_eq!(
            settings.accounts[0]
                .stations
                .iter()
                .find(|station| station.kind == StationKind::Workbench)
                .unwrap()
                .status,
            initial_station_status
        );
    }

    #[test]
    fn round_ammo_result_preserves_account_and_system_error_scopes() {
        let uncertain = map_round_ammo_stop(ammo_runtime::AmmoRunStop::Uncertain {
            target_id: "normal".to_string(),
            step: "ammo.confirm".to_string(),
            message: "未识别到确认按钮".to_string(),
        })
        .unwrap_err();
        assert_eq!(uncertain.scope, round_runner::ErrorScope::Account);
        assert_eq!(uncertain.step, "ammo.confirm");
        assert_eq!(uncertain.ammo_target_id.as_deref(), Some("normal"));

        let isolated = map_round_ammo_stop(ammo_runtime::AmmoRunStop::Isolated {
            target_id: "normal".to_string(),
            step: "ammo.purchase".to_string(),
            message: "材料购买重试耗尽".to_string(),
        })
        .unwrap_err();
        assert_eq!(isolated.scope, round_runner::ErrorScope::Account);
        assert_eq!(isolated.step, "ammo.isolated");
        assert_eq!(isolated.ammo_target_id.as_deref(), Some("normal"));

        let system = map_round_ammo_stop(ammo_runtime::AmmoRunStop::SystemFailure {
            step: "ammo.window".to_string(),
            message: "窗口异常".to_string(),
        })
        .unwrap_err();
        assert_eq!(system.scope, round_runner::ErrorScope::System);
        assert_eq!(system.step, "ammo.window");
    }

    #[test]
    fn round_ammo_department_timeout_is_account_scoped_for_retry() {
        let error = map_round_ammo_stop(ammo_runtime::AmmoRunStop::SystemFailure {
            step: "ammo.department".to_string(),
            message: "模板识别超时".to_string(),
        })
        .unwrap_err();

        assert_eq!(error.scope, round_runner::ErrorScope::Account);
        assert_eq!(
            error.kind,
            round_runner::AccountRunErrorKind::NavigationTimedOut
        );
        assert_eq!(error.step, "ammo.department");
    }

    #[test]
    fn military_supply_target_error_keeps_target_classification() {
        let error = map_military_supply_input_error(
            ammo_runtime::AmmoDriverError::Target("模板识别超时".to_string()),
            "ammo.department",
        );

        assert_eq!(
            error,
            military_supply_runtime::MilitarySupplyEntryError::Target {
                step: "ammo.department".to_string(),
                message: "模板识别超时".to_string(),
            }
        );
    }

    #[test]
    fn round_military_supply_target_error_becomes_navigation_timeout() {
        let error = map_round_military_supply_entry_error(
            military_supply_runtime::MilitarySupplyEntryError::Target {
                step: "ammo.department".to_string(),
                message: "模板识别超时".to_string(),
            },
        );

        assert_eq!(error.scope, round_runner::ErrorScope::Account);
        assert_eq!(
            error.kind,
            round_runner::AccountRunErrorKind::NavigationTimedOut
        );
        assert_eq!(error.step, "ammo.department");
        assert_eq!(error.message, "模板识别超时");
    }

    #[test]
    fn round_navigation_timeout_is_account_scoped_without_station() {
        let error = map_round_navigation_result(game_navigation::GameNavigationResult::TimedOut {
            failed_step: game_navigation::GameNavigationStep::WaitStationGrid,
        })
        .unwrap_err();
        assert_eq!(error.scope, round_runner::ErrorScope::Account);
        assert_eq!(error.station, None);
        assert_eq!(error.step, "navigation.WaitStationGrid");
        assert_eq!(error.message, "步骤超时");

        let mut settings = LoginFixture::complete().settings;
        let before = settings.accounts[0].stations.clone();
        apply_round_account_failure(&mut settings, "selected", &error, 1_000).unwrap();
        assert_eq!(
            settings.accounts[0].status,
            AccountStatus::ManualCheckRequired
        );
        assert_eq!(settings.accounts[0].stations, before);
    }

    #[test]
    fn round_navigation_execution_failure_remains_system_scoped() {
        let error = map_round_navigation_result(game_navigation::GameNavigationResult::Paused {
            failed_step: game_navigation::GameNavigationStep::OpenSpecialOps,
            message: "游戏窗口恢复失败".to_string(),
        })
        .unwrap_err();
        assert_eq!(error.scope, round_runner::ErrorScope::System);
        assert_eq!(error.step, "navigation.OpenSpecialOps");
    }

    #[test]
    fn single_ammo_uncertain_message_keeps_target_context() {
        let stop = ammo_runtime::AmmoRunStop::Uncertain {
            target_id: "normal".to_string(),
            step: "ammo.confirm".to_string(),
            message: "未识别到确认按钮".to_string(),
        };

        let (target_id, step, message) = ammo_stop_failure_detail(&stop).unwrap();

        assert_eq!(target_id, Some("normal"));
        assert_eq!(step, "ammo.confirm");
        assert_eq!(message, "未识别到确认按钮");
    }

    #[test]
    fn round_login_stop_game_failure_is_system_failure() {
        let error = map_round_login_failure(login_flow::LoginStep::StopGame, "无法结束旧游戏进程");

        assert_eq!(error.scope, round_runner::ErrorScope::System);
        assert_eq!(error.step, "login.StopGame");
    }

    #[test]
    fn round_login_game_start_failure_is_navigation_timeout() {
        for step in [
            login_flow::LoginStep::WaitGameEntry,
            login_flow::LoginStep::OpenGameEntry,
            login_flow::LoginStep::WaitLaunchButton,
            login_flow::LoginStep::LaunchGame,
            login_flow::LoginStep::WaitGameWindow,
        ] {
            let error = map_round_login_failure(step, "登录后未找到游戏入口");

            assert_eq!(error.scope, round_runner::ErrorScope::Account);
            assert_eq!(
                error.kind,
                round_runner::AccountRunErrorKind::NavigationTimedOut
            );
            assert_eq!(error.step, format!("login.{step:?}"));
        }
    }

    #[test]
    fn round_login_account_steps_stay_regular_account_failure() {
        for step in [
            login_flow::LoginStep::WaitLoginChoice,
            login_flow::LoginStep::SubmitLogin,
        ] {
            let error = map_round_login_failure(step, "登录步骤失败");

            assert_eq!(error.scope, round_runner::ErrorScope::Account);
            assert_eq!(error.kind, round_runner::AccountRunErrorKind::Regular);
            assert_eq!(error.step, "login.failed");
        }
    }

    #[test]
    fn ammo_success_failure_and_isolation_update_account_runtime_state() {
        let mut settings = LoginFixture::complete().settings;
        settings.accounts[0].ammo_targets = vec![AmmoTarget {
            id: "normal".to_string(),
            name: "普通".to_string(),
            enabled: true,
            seasonal: false,
            scroll_steps: 0,
            order: 0,
            last_success_day: None,
            retry_day: Some("2026-07-30".to_string()),
            retry_count: 2,
            last_failure: None,
        }];

        apply_ammo_failure(
            &mut settings,
            "selected",
            "normal",
            "2026-07-31",
            "ammo.success",
            "兑换未成功",
            100,
        )
        .unwrap();
        assert_eq!(settings.accounts[0].ammo_targets[0].retry_count, 1);
        assert_eq!(
            settings.accounts[0].ammo_targets[0].retry_day.as_deref(),
            Some("2026-07-31")
        );
        assert_eq!(
            settings.accounts[0].last_failure.as_ref().unwrap().step,
            "ammo.success"
        );

        apply_ammo_success(&mut settings, "selected", "normal", "2026-07-31").unwrap();
        assert_eq!(settings.accounts[0].ammo_targets[0].retry_count, 0);
        assert_eq!(
            settings.accounts[0].ammo_targets[0]
                .last_success_day
                .as_deref(),
            Some("2026-07-31")
        );

        apply_ammo_isolated(
            &mut settings,
            "selected",
            "normal",
            "ammo.purchase",
            "仓库空间不足",
            200,
        )
        .unwrap();
        assert_eq!(settings.accounts[0].status, AccountStatus::Isolated);
        assert_eq!(
            settings.accounts[0].last_failure.as_ref().unwrap().message,
            "仓库空间不足"
        );
        assert!(settings.accounts[0].ammo_targets[0].last_failure.is_some());
    }

    #[test]
    fn account_business_correction_updates_stations_and_all_enabled_ammo_atomically() {
        let mut settings = SpecialOpsSettings::default();
        settings.accounts.push(account(
            "selected",
            AccountStatus::Isolated,
            StationKind::all()
                .into_iter()
                .map(|kind| station(kind, 1))
                .collect(),
        ));
        settings.default_business_config.ammo_targets = vec![
            ammo_business_target("normal", false, 0),
            ammo_business_target("seasonal", true, 1),
        ];
        settings.accounts[0].ammo_targets = vec![
            AmmoTarget {
                id: "normal".to_string(),
                name: "普通".to_string(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 0,
                last_success_day: None,
                retry_day: None,
                retry_count: 1,
                last_failure: None,
            },
            AmmoTarget {
                id: "seasonal".to_string(),
                name: "赛季".to_string(),
                enabled: true,
                seasonal: true,
                scroll_steps: 1,
                order: 1,
                last_success_day: Some("1970-01-01".to_string()),
                retry_day: Some("1970-01-01".to_string()),
                retry_count: 2,
                last_failure: None,
            },
        ];
        let stations = StationKind::all()
            .into_iter()
            .map(|kind| StationCorrectionInput {
                kind,
                state: ManualStationState::ImmediateDue,
                remaining_minutes: None,
            })
            .collect::<Vec<_>>();
        let ammo = vec![
            AmmoCorrectionInput {
                target_id: "normal".to_string(),
                succeeded_today: true,
            },
            AmmoCorrectionInput {
                target_id: "seasonal".to_string(),
                succeeded_today: false,
            },
        ];

        apply_manual_account_corrections(
            &mut settings,
            "selected",
            &stations,
            &ammo,
            1_000,
            "1970-01-01",
        )
        .unwrap();

        let account = &settings.accounts[0];
        assert_eq!(account.status, AccountStatus::Ready);
        assert_eq!(
            account.ammo_targets[0].last_success_day.as_deref(),
            Some("1970-01-01")
        );
        assert_eq!(account.ammo_targets[0].retry_count, 0);
        assert_eq!(account.ammo_targets[1].last_success_day, None);
        assert_eq!(account.ammo_targets[1].retry_count, 0);

        let before = settings.clone();
        assert!(apply_manual_account_corrections(
            &mut settings,
            "selected",
            &stations,
            &ammo[..1],
            2_000,
            "1970-01-01",
        )
        .is_err());
        assert_eq!(settings, before);
    }

    #[test]
    fn account_failure_legacy_json_defaults_target_fields_to_none() {
        let failure: AccountFailure = serde_json::from_str(
            r#"{"step":"navigation.WaitStationGrid","message":"步骤超时","atMs":10}"#,
        )
        .unwrap();

        assert_eq!(failure.station_kind, None);
        assert_eq!(failure.ammo_target_id, None);
    }

    #[test]
    fn normalize_rejects_failure_with_station_and_ammo_target() {
        let mut settings = LoginFixture::complete().settings;
        settings.accounts[0].last_failure = Some(AccountFailure {
            step: "invalid".to_string(),
            message: "两个目标".to_string(),
            at_ms: 10,
            station_kind: Some(StationKind::Workbench),
            ammo_target_id: Some("ammo-a".to_string()),
        });

        assert_eq!(
            normalize_settings(settings).unwrap_err(),
            "一条失败记录不能同时指向制作台和子弹目标"
        );
    }

    #[test]
    fn normalize_rejects_ammo_failure_attached_to_different_target() {
        let mut settings = LoginFixture::complete().settings;
        settings.default_business_config.ammo_targets =
            vec![ammo_business_target("ammo-a", false, 0)];
        settings.accounts[0].ammo_targets = vec![AmmoTarget {
            id: "ammo-a".to_string(),
            name: "测试子弹".to_string(),
            enabled: true,
            seasonal: false,
            scroll_steps: 0,
            order: 0,
            last_success_day: None,
            retry_day: None,
            retry_count: 0,
            last_failure: Some(AccountFailure {
                step: "ammo.success".to_string(),
                message: "错误目标".to_string(),
                at_ms: 10,
                station_kind: None,
                ammo_target_id: Some("other-target".to_string()),
            }),
        }];

        assert_eq!(
            normalize_settings(settings).unwrap_err(),
            "子弹失败记录与目标 ID 不一致"
        );
    }

    #[test]
    fn legacy_settings_gain_disabled_limited_and_market_features() {
        let mut value = serde_json::to_value(SpecialOpsSettings::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("limitedSupply");
        object.remove("marketPurchase");

        let normalized = normalize_settings(serde_json::from_value(value).unwrap()).unwrap();

        assert!(!normalized.limited_supply.enabled);
        assert!(!normalized.market_purchase.enabled);
        assert_eq!(normalized.default_business_config.market.max_price, 1);
        assert_eq!(normalized.market_purchase.purchase_count, 1);
    }

    #[test]
    fn legacy_market_purchase_fields_migrate_to_default_business_config() {
        let mut value = serde_json::to_value(SpecialOpsSettings::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object["marketPurchase"] = serde_json::json!({
            "enabled": true,
            "entryDelayMs": 3000,
            "purchaseCount": 7,
            "itemNote": "旧交易行目标"
        });
        let market = object["defaultBusinessConfig"]["market"]
            .as_object_mut()
            .unwrap();
        market.remove("schemaVersion");
        market.remove("enabled");
        market.remove("purchaseCount");
        market.remove("itemNote");

        let normalized = normalize_settings(serde_json::from_value(value).unwrap()).unwrap();

        assert!(normalized.default_business_config.market.enabled);
        assert_eq!(normalized.default_business_config.market.purchase_count, 7);
        assert_eq!(
            normalized.default_business_config.market.item_note,
            "旧交易行目标"
        );
        assert_eq!(normalized.market_purchase.entry_delay_ms, 3000);
    }

    #[test]
    fn explicit_default_market_business_config_wins_over_legacy_values() {
        let mut settings = SpecialOpsSettings::default();
        settings.default_business_config.market.enabled = true;
        settings.default_business_config.market.purchase_count = 2;
        settings.market_purchase.enabled = false;
        settings.market_purchase.purchase_count = 9;

        let normalized = normalize_settings(settings).unwrap();

        assert!(normalized.default_business_config.market.enabled);
        assert_eq!(normalized.default_business_config.market.purchase_count, 2);
    }

    #[test]
    fn legacy_independent_market_config_inherits_migrated_business_fields() {
        let mut settings = LoginFixture::complete().settings;
        settings.market_purchase.enabled = true;
        settings.market_purchase.purchase_count = 3;
        settings.market_purchase.item_note = "默认旧目标".to_string();
        settings.accounts[0].independent_settings_enabled = true;
        settings.accounts[0].independent_business_config = Some(BusinessConfig::default());
        settings.accounts[0]
            .independent_business_config
            .as_mut()
            .unwrap()
            .market
            .schema_version = 0;

        let normalized = normalize_settings(settings).unwrap();
        let independent = normalized.accounts[0]
            .independent_business_config
            .as_ref()
            .unwrap();

        assert!(independent.market.enabled);
        assert_eq!(independent.market.purchase_count, 3);
        assert_eq!(independent.market.item_note, "默认旧目标");
    }

    #[test]
    fn default_calibration_contains_limited_and_market_targets() {
        let targets = default_calibration_targets();
        let keys = targets
            .iter()
            .map(|target| target.key.clone())
            .collect::<std::collections::HashSet<_>>();

        for key in [
            "ammo.researchDepartment",
            "limited.ready",
            "limited.color.1",
            "limited.color.9",
            "market.entry",
            "market.product",
            "market.price",
            "market.return",
            "market.buy",
            "market.confirm",
        ] {
            assert!(keys.contains(key), "缺少校准目标 {key}");
        }
        assert_eq!(
            targets
                .iter()
                .find(|target| target.key == "market.entry")
                .map(|target| target.kind.clone()),
            Some(CalibrationTargetKind::RecognitionRegion)
        );
        assert_eq!(
            targets
                .iter()
                .find(|target| target.key == "market.confirm")
                .map(|target| target.kind.clone()),
            Some(CalibrationTargetKind::ClickPoint)
        );
    }

    #[test]
    fn market_trial_freezes_entry_template_for_recognition() {
        let mut fixture = LoginFixture::complete();
        fixture
            .settings
            .default_business_config
            .market
            .product_point = Some(CalibrationRect {
            x: 50,
            y: 60,
            width: 1,
            height: 1,
        });

        let reference = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        std::fs::write(reference.path(), b"market.entry").unwrap();
        let entry = calibration_target_mut(&mut fixture.settings, "market.entry");
        entry.rect = Some(CalibrationRect {
            x: 10,
            y: 20,
            width: 30,
            height: 40,
        });
        entry.reference_image_path = Some(reference.path().display().to_string());
        entry.verified_signature = Some(calibration_signature(entry).unwrap());
        entry.verified_at_ms = Some(1);
        fixture._reference_files.push(reference);

        for key in [
            "market.price",
            "market.return",
            "market.buy",
            "market.confirm",
        ] {
            calibration_target_mut(&mut fixture.settings, key).rect = Some(CalibrationRect {
                x: 10,
                y: 20,
                width: 30,
                height: 40,
            });
        }

        let frozen = freeze_market_run(&fixture.settings, "selected", "2026-08-09").unwrap();
        assert!(frozen.targets["market.entry"].template.is_some());
    }

    #[test]
    fn account_market_selection_only_updates_selected_business_config() {
        let mut settings = LoginFixture::complete().settings;
        settings.accounts[0].independent_settings_enabled = true;
        settings.accounts[0].independent_business_config =
            Some(settings.default_business_config.clone());
        let global_point = CalibrationRect {
            x: 1,
            y: 2,
            width: 1,
            height: 1,
        };
        let account_point = CalibrationRect {
            x: 3,
            y: 4,
            width: 1,
            height: 1,
        };

        apply_market_business_selection(&mut settings, None, global_point.clone()).unwrap();
        apply_market_business_selection(&mut settings, Some("selected"), account_point.clone())
            .unwrap();

        assert_eq!(
            settings.default_business_config.market.product_point,
            Some(global_point)
        );
        assert_eq!(
            settings.accounts[0]
                .independent_business_config
                .as_ref()
                .unwrap()
                .market
                .product_point,
            Some(account_point)
        );
    }

    fn shanghai_test_ms(value: &str) -> i64 {
        let offset = FixedOffset::east_opt(8 * 60 * 60).unwrap();
        chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M")
            .unwrap()
            .and_local_timezone(offset)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn schedule_projects_current_limited_cycle_and_market_window() {
        let mut settings = LoginFixture::complete().settings;
        settings.paused = false;
        settings.limited_supply.enabled = true;
        settings.default_business_config.market.enabled = true;
        let now_ms = shanghai_test_ms("2026-08-08 02:30");

        let snapshot = build_schedule(&settings, now_ms);

        assert!(snapshot
            .timeline_tasks
            .iter()
            .any(|task| task.kind == TimelineTaskKind::LimitedSupplyCheck
                && task.limited_cycle_id.as_deref() == Some("2026-08-07T20:00")));
        assert!(snapshot
            .timeline_tasks
            .iter()
            .any(|task| task.kind == TimelineTaskKind::MarketPurchase
                && task.scheduled_at_ms == shanghai_test_ms("2026-08-08 02:00")));
        assert!(snapshot.due_accounts.iter().any(|due| {
            due.account_id == "selected" && due.limited_supply_due && due.market_purchase_due
        }));
    }

    #[test]
    fn schedule_uses_independent_market_business_config() {
        let mut settings = LoginFixture::complete().settings;
        settings.paused = false;
        settings.default_business_config.market.enabled = false;
        settings.accounts[0].independent_settings_enabled = true;
        let mut independent = settings.default_business_config.clone();
        independent.market.enabled = true;
        independent.market.purchase_count = 4;
        independent.market.item_note = "账号专用交易行目标".to_string();
        settings.accounts[0].independent_business_config = Some(independent);

        let snapshot = build_schedule(&settings, shanghai_test_ms("2026-08-08 02:30"));
        let task = snapshot
            .timeline_tasks
            .iter()
            .find(|task| task.kind == TimelineTaskKind::MarketPurchase)
            .unwrap();

        assert_eq!(task.account_id, "selected");
        assert_eq!(task.note, "账号专用交易行目标");
        assert_eq!(task.market_target_count, Some(4));
        assert!(snapshot
            .due_accounts
            .iter()
            .any(|due| due.account_id == "selected" && due.market_purchase_due));
    }

    #[test]
    fn new_limited_cycle_discards_old_high_value_reminder() {
        let mut settings = LoginFixture::complete().settings;
        settings.paused = false;
        settings.limited_supply.enabled = true;
        settings.accounts[0].limited_supply = LimitedSupplyAccountState {
            cycle_id: Some("2026-08-08T12:00".to_string()),
            outcome: limited_supply::LimitedSupplyOutcome::HighValue,
            checked_at_ms: Some(shanghai_test_ms("2026-08-08 12:01")),
            matched_region: Some(3),
            matched_color: Some([1, 2, 3]),
            acknowledged: false,
            last_error: None,
        };

        let snapshot = build_schedule(&settings, shanghai_test_ms("2026-08-08 20:00"));

        assert!(!snapshot
            .timeline_tasks
            .iter()
            .any(|task| { task.limited_cycle_id.as_deref() == Some("2026-08-08T12:00") }));
        assert!(snapshot.timeline_tasks.iter().any(|task| {
            task.limited_cycle_id.as_deref() == Some("2026-08-08T20:00") && task.overdue
        }));
    }

    #[test]
    fn non_ready_new_tasks_project_but_do_not_become_due() {
        let mut settings = LoginFixture::complete().settings;
        settings.paused = false;
        settings.limited_supply.enabled = true;
        settings.default_business_config.market.enabled = true;
        settings.accounts[0].status = AccountStatus::ManualCheckRequired;

        let snapshot = build_schedule(&settings, shanghai_test_ms("2026-08-08 02:30"));

        assert!(snapshot.timeline_tasks.iter().any(|task| {
            matches!(
                task.kind,
                TimelineTaskKind::LimitedSupplyCheck | TimelineTaskKind::MarketPurchase
            )
        }));
        assert!(snapshot.due_accounts.is_empty());
    }
}
