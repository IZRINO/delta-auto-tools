use super::{
    desktop_runtime::{DesktopRuntime, WindowIdentity, WindowsDesktopRuntime},
    login_flow::{LoginDriver, LoginObservation, LoginRunConfig},
    remembered_account::{AccountRowClick, AccountRowSlot, RememberedAccountDriver},
    template_observer::{RuntimeSimilaritySampler, RuntimeTemplate},
};
use super::{login_flow::LoginStep, StationKind};
use serde::{Deserialize, Serialize};
use std::{
    future::Future,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};

pub const RUN_CHANGED: &str = "special-ops://run-changed";
pub(crate) const OPERATION_WINDOW_LABEL: &str = "special-ops-operation";
fn run_changed_target_labels() -> [&'static str; 2] {
    ["main", OPERATION_WINDOW_LABEL]
}

pub(crate) fn emit_run_changed(app: &AppHandle, snapshot: &LoginRunSnapshot) {
    for label in run_changed_target_labels() {
        let _ = app.emit_to(label, RUN_CHANGED, snapshot.clone());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoginRunSnapshot {
    pub run_id: u64,
    pub account_id: String,
    pub run_kind: LoginRunKind,
    pub status: LoginRunStatus,
    pub current_step: Option<LoginStep>,
    pub message: String,
    pub countdown_seconds: Option<u8>,
    #[serde(default)]
    pub round_progress: Option<RoundProgress>,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LoginRunKind {
    Login,
    Navigation,
    Craft,
    Ammo,
    LimitedSupply,
    Market,
    Round,
    StationWalkthrough,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoundProgress {
    pub account_index: usize,
    pub account_total: usize,
    pub qq_account: String,
    pub station_kind: Option<StationKind>,
    pub station_index: usize,
    pub station_total: usize,
}

impl LoginRunKind {
    pub(crate) fn query_value(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Navigation => "navigation",
            Self::Craft => "craft",
            Self::Ammo => "ammo",
            Self::LimitedSupply => "limitedSupply",
            Self::Market => "market",
            Self::Round => "round",
            Self::StationWalkthrough => "stationWalkthrough",
        }
    }

    fn preparing_message(self) -> &'static str {
        match self {
            Self::Login => "正在准备登录试运行",
            Self::Navigation => "正在准备游戏内导航试运行",
            Self::Craft => "正在准备制作试运行",
            Self::Ammo => "正在准备子弹兑换试运行",
            Self::LimitedSupply => "正在准备限时商品试运行",
            Self::Market => "正在准备交易行试运行",
            Self::Round => "正在准备多账号制作轮次",
            Self::StationWalkthrough => "正在准备多账号制作台更改",
        }
    }

    fn normal_cancel_message(self) -> &'static str {
        match self {
            Self::Login => "正在取消登录试运行",
            Self::Navigation => "正在取消游戏内导航试运行",
            Self::Craft => "正在取消制作试运行",
            Self::Ammo => "正在取消子弹兑换试运行",
            Self::LimitedSupply => "正在取消限时商品试运行",
            Self::Market => "正在取消交易行试运行",
            Self::Round => "正在停止多账号制作轮次",
            Self::StationWalkthrough => "正在取消多账号制作台更改",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LoginRunStatus {
    Starting,
    Waiting,
    Countdown,
    Inputting,
    Stopping,
    Succeeded,
    Failed,
    Stopped,
}

#[derive(Debug)]
pub(crate) struct StartedLoginRun {
    pub run_id: u64,
    pub cancelled: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StopReason {
    Normal,
    Emergency,
    Lifecycle { uncertain: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistenceKind {
    Flow,
    Stop(StopReason),
}

#[derive(Debug)]
pub(crate) enum PersistenceClaim<'a> {
    Acquired(PersistenceGuard<'a>),
    Pending,
    Persisted,
    NoPersistence,
    Stale,
    NoActive,
}

pub(crate) struct PersistenceGuard<'a> {
    runtime: &'a LoginRuntime,
    run_id: u64,
    kind: PersistenceKind,
    resolved: bool,
}

impl std::fmt::Debug for PersistenceGuard<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PersistenceGuard")
            .field("run_id", &self.run_id)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl PersistenceGuard<'_> {
    pub(crate) fn kind(&self) -> PersistenceKind {
        self.kind
    }

    pub(crate) fn complete(mut self) -> Result<bool, String> {
        let authoritative = self.runtime.complete_persistence(self.run_id, self.kind)?;
        self.resolved = true;
        Ok(authoritative)
    }

    pub(crate) fn fail(mut self, message: &str) -> Result<(), String> {
        self.runtime
            .fail_persistence(self.run_id, self.kind, message)?;
        self.resolved = true;
        Ok(())
    }
}

impl Drop for PersistenceGuard<'_> {
    fn drop(&mut self) {
        if !self.resolved {
            let _ = self.runtime.release_persistence(self.run_id, self.kind);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PersistenceState {
    Unclaimed,
    InProgress(PersistenceKind),
    Persisted,
}

struct ActiveLoginRun {
    snapshot: LoginRunSnapshot,
    cancelled: Arc<AtomicBool>,
    stop_reason: Option<StopReason>,
    entered_input: bool,
    first_input_countdown_claimed: bool,
    persistence_state: PersistenceState,
    cleanup_failed: bool,
    worker_handed_off: bool,
}

struct LoginRuntimeInner {
    next_run_id: u64,
    active: Option<ActiveLoginRun>,
}

pub(crate) struct LoginRuntime {
    inner: Mutex<LoginRuntimeInner>,
    event_serial: Mutex<()>,
    persistence_changed: Condvar,
}

impl Default for LoginRuntime {
    fn default() -> Self {
        Self {
            inner: Mutex::new(LoginRuntimeInner {
                next_run_id: 1,
                active: None,
            }),
            event_serial: Mutex::new(()),
            persistence_changed: Condvar::new(),
        }
    }
}

impl LoginRuntime {
    pub(crate) fn with_event_serialized<T>(
        &self,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _event = self
            .event_serial
            .lock()
            .map_err(|_| "登录试运行事件状态已损坏".to_string())?;
        operation()
    }

    #[cfg(test)]
    pub(crate) fn try_start(&self, account_id: String) -> Result<StartedLoginRun, String> {
        self.try_start_kind(account_id, LoginRunKind::Login)
    }

    pub(crate) fn try_start_kind(
        &self,
        account_id: String,
        run_kind: LoginRunKind,
    ) -> Result<StartedLoginRun, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏，拒绝启动".to_string())?;
        if inner.active.is_some() {
            return Err("已有登录试运行尚未完成清理".to_string());
        }
        let run_id = inner.next_run_id;
        inner.next_run_id = inner.next_run_id.saturating_add(1);
        let now = super::now_ms();
        let cancelled = Arc::new(AtomicBool::new(false));
        inner.active = Some(ActiveLoginRun {
            snapshot: LoginRunSnapshot {
                run_id,
                account_id,
                run_kind,
                status: LoginRunStatus::Starting,
                current_step: None,
                message: run_kind.preparing_message().to_string(),
                countdown_seconds: None,
                round_progress: None,
                started_at_ms: now,
                updated_at_ms: now,
            },
            cancelled: Arc::clone(&cancelled),
            stop_reason: None,
            entered_input: false,
            first_input_countdown_claimed: false,
            persistence_state: PersistenceState::Unclaimed,
            cleanup_failed: false,
            worker_handed_off: false,
        });
        Ok(StartedLoginRun { run_id, cancelled })
    }

    pub(crate) fn next_input_countdown_seconds(
        &self,
        run_id: u64,
        show_subsequent_countdown: bool,
    ) -> Result<Option<u8>, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        let active = inner
            .active
            .as_mut()
            .filter(|run| run.snapshot.run_id == run_id)
            .ok_or_else(|| "特勤处 run 已结束，不能开始倒计时".to_string())?;
        if !active.first_input_countdown_claimed {
            active.first_input_countdown_claimed = true;
            Ok(Some(5))
        } else if show_subsequent_countdown {
            Ok(Some(0))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn update(
        &self,
        run_id: u64,
        status: LoginRunStatus,
        current_step: Option<LoginStep>,
        message: impl Into<String>,
        countdown_seconds: Option<u8>,
    ) -> Result<Option<LoginRunSnapshot>, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        let Some(active) = inner
            .active
            .as_mut()
            .filter(|run| run.snapshot.run_id == run_id)
        else {
            return Ok(None);
        };
        if active.stop_reason.is_some() || active.cancelled.load(Ordering::SeqCst) {
            return Ok(None);
        }
        active.entered_input |= status == LoginRunStatus::Inputting;
        active.snapshot.status = status;
        active.snapshot.current_step = current_step;
        active.snapshot.message = message.into();
        active.snapshot.countdown_seconds = countdown_seconds;
        active.snapshot.updated_at_ms = super::now_ms();
        Ok(Some(active.snapshot.clone()))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn update_round_progress(
        &self,
        run_id: u64,
        account_index: usize,
        account_total: usize,
        account_id: impl Into<String>,
        qq_account: impl Into<String>,
        station_kind: Option<StationKind>,
        station_index: usize,
        station_total: usize,
    ) -> Result<Option<LoginRunSnapshot>, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        let Some(active) = inner
            .active
            .as_mut()
            .filter(|run| run.snapshot.run_id == run_id)
        else {
            return Ok(None);
        };
        if active.stop_reason.is_some() || active.cancelled.load(Ordering::SeqCst) {
            return Ok(None);
        }
        active.snapshot.account_id = account_id.into();
        active.snapshot.round_progress = Some(RoundProgress {
            account_index,
            account_total,
            qq_account: qq_account.into(),
            station_kind,
            station_index,
            station_total,
        });
        active.snapshot.updated_at_ms = super::now_ms();
        Ok(Some(active.snapshot.clone()))
    }

    pub(crate) fn finish(
        &self,
        run_id: u64,
        status: LoginRunStatus,
        message: impl Into<String>,
    ) -> Result<Option<LoginRunSnapshot>, String> {
        crate::log_debug!(
            "special_ops::runtime",
            "runtime finish 开始",
            "run_id" => run_id,
            "status" => format!("{status:?}")
        );
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏，拒绝释放单实例".to_string())?;
        let Some(mut active) = inner.active.take() else {
            return Ok(None);
        };
        if active.snapshot.run_id != run_id {
            inner.active = Some(active);
            return Ok(None);
        }
        active.snapshot.status = if active.stop_reason.is_some() {
            LoginRunStatus::Stopped
        } else {
            status
        };
        active.snapshot.current_step = None;
        active.snapshot.message = message.into();
        active.snapshot.countdown_seconds = None;
        active.snapshot.updated_at_ms = super::now_ms();
        crate::log_debug!(
            "special_ops::runtime",
            "runtime finish 完成",
            "run_id" => run_id,
            "status" => format!("{:?}", active.snapshot.status)
        );
        Ok(Some(active.snapshot))
    }

    pub(crate) fn request_stop(
        &self,
        run_id: u64,
        reason: StopReason,
    ) -> Result<Option<LoginRunSnapshot>, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏，拒绝停止".to_string())?;
        let Some(active) = inner
            .active
            .as_mut()
            .filter(|active| active.snapshot.run_id == run_id)
        else {
            return Ok(None);
        };
        Ok(Some(self.request_stop_locked(active, reason)))
    }

    pub(crate) fn request_lifecycle_stop(
        &self,
        run_id: u64,
    ) -> Result<Option<LoginRunSnapshot>, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏，拒绝停止".to_string())?;
        let Some(active) = inner
            .active
            .as_mut()
            .filter(|active| active.snapshot.run_id == run_id)
        else {
            return Ok(None);
        };
        let reason = StopReason::Lifecycle {
            uncertain: active.entered_input,
        };
        Ok(Some(self.request_stop_locked(active, reason)))
    }

    fn request_stop_locked(
        &self,
        active: &mut ActiveLoginRun,
        reason: StopReason,
    ) -> LoginRunSnapshot {
        let previous_reason = active.stop_reason;
        active.stop_reason = Some(authoritative_stop_reason(previous_reason, reason));
        let effective_reason = active.stop_reason.unwrap_or(reason);
        if previous_reason != active.stop_reason
            && !matches!(effective_reason, StopReason::Normal)
            && active.persistence_state == PersistenceState::Persisted
        {
            active.persistence_state = PersistenceState::Unclaimed;
            self.persistence_changed.notify_all();
        }
        active.cancelled.store(true, Ordering::SeqCst);
        active.snapshot.status = LoginRunStatus::Stopping;
        active.snapshot.current_step = None;
        active.snapshot.message = match effective_reason {
            StopReason::Normal => active.snapshot.run_kind.normal_cancel_message(),
            StopReason::Emergency => "正在执行紧急停止",
            StopReason::Lifecycle { .. } => "正在停止登录试运行",
        }
        .to_string();
        active.snapshot.countdown_seconds = None;
        active.snapshot.updated_at_ms = super::now_ms();
        active.snapshot.clone()
    }

    pub(crate) fn snapshot(&self) -> Result<Option<LoginRunSnapshot>, String> {
        self.inner
            .lock()
            .map(|inner| inner.active.as_ref().map(|active| active.snapshot.clone()))
            .map_err(|_| "登录试运行状态已损坏".to_string())
    }

    pub(crate) fn stop_reason(&self, run_id: u64) -> Result<Option<StopReason>, String> {
        self.inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())
            .map(|inner| {
                inner
                    .active
                    .as_ref()
                    .filter(|active| active.snapshot.run_id == run_id)
                    .and_then(|active| active.stop_reason)
            })
    }

    pub(crate) fn entered_input(&self, run_id: u64) -> Result<bool, String> {
        self.inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())
            .map(|inner| {
                inner
                    .active
                    .as_ref()
                    .filter(|active| active.snapshot.run_id == run_id)
                    .is_some_and(|active| active.entered_input)
            })
    }

    pub(crate) fn can_continue_start(&self, run_id: u64) -> Result<bool, String> {
        self.inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())
            .map(|inner| {
                inner
                    .active
                    .as_ref()
                    .filter(|active| active.snapshot.run_id == run_id)
                    .is_some_and(|active| active.stop_reason.is_none())
            })
    }

    pub(crate) fn claim_worker_handoff(&self, run_id: u64) -> Result<bool, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        let Some(active) = inner
            .active
            .as_mut()
            .filter(|active| active.snapshot.run_id == run_id)
        else {
            return Ok(false);
        };
        if active.stop_reason.is_some() || active.worker_handed_off {
            return Ok(false);
        }
        active.worker_handed_off = true;
        Ok(true)
    }

    pub(crate) fn mark_cleanup_failed(&self, run_id: u64) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        if let Some(active) = inner
            .active
            .as_mut()
            .filter(|active| active.snapshot.run_id == run_id)
        {
            active.cleanup_failed = true;
        }
        Ok(())
    }

    pub(crate) fn cleanup_failed(&self, run_id: u64) -> Result<bool, String> {
        self.inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())
            .map(|inner| {
                inner
                    .active
                    .as_ref()
                    .filter(|active| active.snapshot.run_id == run_id)
                    .is_some_and(|active| active.cleanup_failed)
            })
    }

    pub(crate) fn cleanup_ready(&self, run_id: u64) -> Result<bool, String> {
        self.inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())
            .map(|inner| {
                inner
                    .active
                    .as_ref()
                    .filter(|active| active.snapshot.run_id == run_id)
                    .is_none_or(|active| match active.stop_reason {
                        None | Some(StopReason::Normal) => true,
                        Some(StopReason::Emergency | StopReason::Lifecycle { .. }) => {
                            active.persistence_state == PersistenceState::Persisted
                        }
                    })
            })
    }

    pub(crate) fn claim_persistence(&self, run_id: u64) -> Result<PersistenceClaim<'_>, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        let Some(active) = inner.active.as_mut() else {
            return Ok(PersistenceClaim::NoActive);
        };
        if active.snapshot.run_id != run_id {
            return Ok(PersistenceClaim::Stale);
        };
        match active.persistence_state {
            PersistenceState::InProgress(_) => Ok(PersistenceClaim::Pending),
            PersistenceState::Persisted => Ok(PersistenceClaim::Persisted),
            PersistenceState::Unclaimed => {
                let Some(kind) = desired_persistence_kind(active.stop_reason) else {
                    return Ok(PersistenceClaim::NoPersistence);
                };
                active.persistence_state = PersistenceState::InProgress(kind);
                Ok(PersistenceClaim::Acquired(PersistenceGuard {
                    runtime: self,
                    run_id,
                    kind,
                    resolved: false,
                }))
            }
        }
    }

    pub(crate) fn complete_persistence(
        &self,
        run_id: u64,
        kind: PersistenceKind,
    ) -> Result<bool, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        let Some(active) = inner
            .active
            .as_mut()
            .filter(|active| active.snapshot.run_id == run_id)
        else {
            return Ok(false);
        };
        if active.persistence_state != PersistenceState::InProgress(kind) {
            return Err("登录结果持久化权限已失效".to_string());
        }
        let authoritative = desired_persistence_kind(active.stop_reason) == Some(kind);
        active.persistence_state = if authoritative {
            PersistenceState::Persisted
        } else {
            PersistenceState::Unclaimed
        };
        self.persistence_changed.notify_all();
        Ok(authoritative)
    }

    pub(crate) fn release_persistence(
        &self,
        run_id: u64,
        kind: PersistenceKind,
    ) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        let Some(active) = inner
            .active
            .as_mut()
            .filter(|active| active.snapshot.run_id == run_id)
        else {
            return Ok(());
        };
        if active.persistence_state != PersistenceState::InProgress(kind) {
            return Err("登录结果持久化权限已失效".to_string());
        }
        active.persistence_state = PersistenceState::Unclaimed;
        self.persistence_changed.notify_all();
        Ok(())
    }

    fn fail_persistence(
        &self,
        run_id: u64,
        kind: PersistenceKind,
        message: &str,
    ) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        let Some(active) = inner
            .active
            .as_mut()
            .filter(|active| active.snapshot.run_id == run_id)
        else {
            return Ok(());
        };
        if active.persistence_state != PersistenceState::InProgress(kind) {
            return Err("登录结果持久化权限已失效".to_string());
        }
        active.snapshot.status = LoginRunStatus::Failed;
        active.snapshot.current_step = None;
        active.snapshot.message = message.to_string();
        active.snapshot.countdown_seconds = None;
        active.snapshot.updated_at_ms = super::now_ms();
        active.persistence_state = PersistenceState::Unclaimed;
        self.persistence_changed.notify_all();
        Ok(())
    }

    pub(crate) fn wait_for_persistence_change(
        &self,
        run_id: u64,
        timeout: Duration,
    ) -> Result<(), String> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        let is_pending = inner
            .active
            .as_ref()
            .filter(|active| active.snapshot.run_id == run_id)
            .is_some_and(|active| {
                matches!(active.persistence_state, PersistenceState::InProgress(_))
            });
        if is_pending {
            let _ = self
                .persistence_changed
                .wait_timeout(inner, timeout)
                .map_err(|_| "登录试运行状态已损坏".to_string())?;
        }
        Ok(())
    }
}

fn desired_persistence_kind(stop_reason: Option<StopReason>) -> Option<PersistenceKind> {
    match stop_reason {
        None => Some(PersistenceKind::Flow),
        Some(StopReason::Normal) => None,
        Some(reason) => Some(PersistenceKind::Stop(reason)),
    }
}

fn authoritative_stop_reason(current: Option<StopReason>, requested: StopReason) -> StopReason {
    match (current, requested) {
        (Some(StopReason::Emergency), _) | (_, StopReason::Emergency) => StopReason::Emergency,
        (Some(StopReason::Lifecycle { uncertain: true }), _)
        | (_, StopReason::Lifecycle { uncertain: true }) => {
            StopReason::Lifecycle { uncertain: true }
        }
        (Some(StopReason::Lifecycle { uncertain: false }), _)
        | (_, StopReason::Lifecycle { uncertain: false }) => {
            StopReason::Lifecycle { uncertain: false }
        }
        (Some(StopReason::Normal), StopReason::Normal) | (None, StopReason::Normal) => {
            StopReason::Normal
        }
    }
}

pub(crate) struct ProductionLoginDriver {
    app: AppHandle,
    runtime: Arc<LoginRuntime>,
    run_id: u64,
    config: Arc<LoginRunConfig>,
    observation: Mutex<LoginObservation>,
    login_takeover_active: AtomicBool,
}

impl ProductionLoginDriver {
    pub(crate) fn new(
        app: AppHandle,
        runtime: Arc<LoginRuntime>,
        run_id: u64,
        config: Arc<LoginRunConfig>,
    ) -> Self {
        Self {
            app,
            runtime,
            run_id,
            config,
            observation: Mutex::new(LoginObservation::None),
            login_takeover_active: AtomicBool::new(false),
        }
    }

    fn emit_update(
        &self,
        status: LoginRunStatus,
        message: impl Into<String>,
        countdown_seconds: Option<u8>,
    ) -> Result<(), String> {
        let step = self
            .runtime
            .snapshot()?
            .filter(|snapshot| snapshot.run_id == self.run_id)
            .and_then(|snapshot| snapshot.current_step);
        if let Some(snapshot) =
            self.runtime
                .update(self.run_id, status, step, message, countdown_seconds)?
        {
            emit_run_changed(&self.app, &snapshot);
        }
        Ok(())
    }

    fn set_observation(&self, observation: LoginObservation) {
        if let Ok(mut current) = self.observation.lock() {
            *current = observation;
        }
    }

    async fn countdown(&self, cancelled: &Arc<AtomicBool>) -> Result<(), String> {
        if !countdown_required(self.login_takeover_active.load(Ordering::SeqCst)) {
            return Ok(());
        }
        let Some(total) = self
            .runtime
            .next_input_countdown_seconds(self.run_id, true)?
        else {
            return Ok(());
        };
        for seconds in (1..=total).rev() {
            ensure_not_cancelled(cancelled)?;
            self.emit_update(
                LoginRunStatus::Countdown,
                format!("{seconds} 秒后执行键鼠操作"),
                Some(seconds),
            )?;
            wait_cancellable(Duration::from_secs(1), cancelled).await?;
        }
        Ok(())
    }

    async fn focus_wegame(&self) -> Result<(), String> {
        let executable = self.config.wegame_executable_path.clone();
        tokio::task::spawn_blocking(move || {
            let runtime = WindowsDesktopRuntime;
            let window = runtime
                .find_primary_window_in_tree(&executable)?
                .ok_or_else(|| "未找到 WeGame 窗口".to_string())?;
            runtime.restore_and_focus_in_tree(&executable, window)
        })
        .await
        .map_err(|error| format!("窗口任务失败: {error}"))?
    }

    async fn verify_action_guard(
        &self,
        target_key: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let target = self
            .config
            .targets
            .get(target_key)
            .ok_or_else(|| format!("登录校准目标 {target_key} 不存在"))?;
        let guard_keys = if target.guard_any_of.is_empty() {
            vec![target_key]
        } else {
            target.guard_any_of.iter().map(String::as_str).collect()
        };
        let templates = guard_keys
            .iter()
            .map(|key| {
                self.config
                    .targets
                    .get(*key)
                    .and_then(|target| target.template.as_ref())
                    .ok_or_else(|| format!("动作守卫 {key} 未配置已验证模板"))
            })
            .collect::<Result<Vec<&RuntimeTemplate>, String>>()?;
        match super::template_observer::wait_for_any_consistent_match_with_observer(
            &RuntimeSimilaritySampler,
            &templates,
            Arc::clone(&cancelled),
            |_, samples| {
                self.set_observation(LoginObservation::TemplateSamples { samples });
            },
        )
        .await
        {
            Ok((_, observation)) => {
                self.set_observation(LoginObservation::TemplateSamples {
                    samples: observation.samples,
                });
                Ok(())
            }
            Err(error) => {
                self.set_observation(observation_for_error(&error));
                Err(error)
            }
        }
    }

    async fn verify_account_list_open(&self, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        const MAX_ATTEMPTS: u8 = 3;
        for attempt in 1..=MAX_ATTEMPTS {
            self.emit_update(
                LoginRunStatus::Waiting,
                format!("正在确认已记住账号列表（第 {attempt}/{MAX_ATTEMPTS} 次）"),
                None,
            )?;
            let first = self.sample_accounts(Arc::clone(&cancelled)).await?;
            wait_cancellable(Duration::from_millis(400), &cancelled).await?;
            let second = self.sample_accounts(Arc::clone(&cancelled)).await?;
            if super::remembered_account::account_list_visible_in_both_samples(&first, &second) {
                return Ok(());
            }
            if attempt < MAX_ATTEMPTS {
                wait_cancellable(Duration::from_millis(400), &cancelled).await?;
            }
        }
        Err(super::remembered_account::ACCOUNT_LIST_UNAVAILABLE.to_string())
    }

    async fn sample_accounts(
        &self,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<super::windows_ocr::OcrWord>, String> {
        ensure_not_cancelled(&cancelled)?;
        let region = self
            .config
            .targets
            .get("wegame.accountList")
            .ok_or_else(|| "登录校准目标 wegame.accountList 不存在".to_string())?
            .region
            .clone();
        let result = tokio::task::spawn_blocking(move || {
            let image = crate::recognition::watcher::capture_region(&region)
                .ok_or_else(|| "账号列表截图失败".to_string())?;
            super::windows_ocr::recognize_numeric_words(image)
        })
        .await
        .map_err(|error| format!("账号列表 OCR 任务失败: {error}"))?;
        if result.is_err() {
            self.set_observation(LoginObservation::CaptureFailed);
        }
        result
    }
}

/// 判断当前动作是否需要显示键鼠接管倒计时。
fn countdown_required(login_takeover_active: bool) -> bool {
    !login_takeover_active
}

#[allow(async_fn_in_trait)]
impl LoginDriver for ProductionLoginDriver {
    fn reset_observation(&self) {
        self.set_observation(LoginObservation::None);
    }

    fn last_observation(&self) -> LoginObservation {
        self.observation
            .lock()
            .map(|observation| *observation)
            .unwrap_or(LoginObservation::None)
    }

    async fn initial_countdown(&self, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        self.countdown(&cancelled).await
    }

    async fn terminate_exact(&self, executable: &Path) -> Result<(), String> {
        let executable = executable.to_path_buf();
        let result = match tokio::time::timeout(
            Duration::from_secs(8),
            tokio::task::spawn_blocking(move || {
                super::desktop_runtime::terminate_exact_without_waiting(&executable)
            }),
        )
        .await
        {
            Ok(Ok(inner)) => inner,
            Ok(Err(error)) => Err(format!("进程结束任务失败: {error}")),
            Err(_) => Err("结束进程未在预算内返回".to_string()),
        };
        if let Err(error) = &result {
            let observation = process_observation(error);
            crate::log_error!(
                "special_ops::login",
                "结束进程失败",
                "error" => error.as_str()
            );
            self.set_observation(observation);
        }
        result
    }

    async fn launch(&self, executable: &Path) -> Result<u32, String> {
        let executable = executable.to_path_buf();
        let result = tokio::task::spawn_blocking(move || WindowsDesktopRuntime.launch(&executable))
            .await
            .map_err(|error| format!("程序启动任务失败: {error}"))?;
        if let Err(error) = &result {
            let observation = launch_observation(error);
            let windows_error_code = match observation {
                LoginObservation::LaunchFailed { windows_error_code } => windows_error_code,
                _ => None,
            };
            self.set_observation(observation);
            crate::log_error!(
                "special_ops::login",
                "WeGame 启动失败",
                "windows_error_code" => windows_error_code
            );
        }
        result
    }

    async fn wait_for_any(
        &self,
        target_keys: &[&str],
        cancelled: Arc<AtomicBool>,
    ) -> Result<String, String> {
        let templates = target_keys
            .iter()
            .map(|key| {
                self.config
                    .targets
                    .get(*key)
                    .and_then(|target| target.template.as_ref())
                    .ok_or_else(|| format!("登录识别目标 {key} 未配置已验证模板"))
            })
            .collect::<Result<Vec<&RuntimeTemplate>, String>>()?;
        match super::template_observer::wait_for_any_consistent_match_with_observer(
            &RuntimeSimilaritySampler,
            &templates,
            Arc::clone(&cancelled),
            |_, samples| {
                self.set_observation(LoginObservation::TemplateSamples { samples });
            },
        )
        .await
        {
            Ok((key, observation)) => {
                self.set_observation(LoginObservation::TemplateSamples {
                    samples: observation.samples,
                });
                if key == "wegame.gameEntry" {
                    self.login_takeover_active.store(false, Ordering::SeqCst);
                }
                Ok(key)
            }
            Err(error) => {
                self.set_observation(observation_for_error(&error));
                Err(error)
            }
        }
    }

    async fn click(&self, target_key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        let region = self
            .config
            .targets
            .get(target_key)
            .ok_or_else(|| format!("登录校准目标 {target_key} 不存在"))?
            .region
            .clone();
        let focus_cancelled = Arc::clone(&cancelled);
        let guard_cancelled = Arc::clone(&cancelled);
        let action_cancelled = Arc::clone(&cancelled);
        let mouse_parking_region = self.config.mouse_parking_region.clone();
        run_checked_action(
            self.countdown(&cancelled),
            async {
                ensure_not_cancelled(&focus_cancelled)?;
                self.focus_wegame().await.inspect_err(|error| {
                    self.set_observation(window_observation(error));
                })
            },
            async {
                ensure_not_cancelled(&guard_cancelled)?;
                self.verify_action_guard(target_key, guard_cancelled).await
            },
            async {
                ensure_not_cancelled(&action_cancelled)?;
                self.emit_update(LoginRunStatus::Inputting, "正在执行键鼠操作", None)?;
                crate::input_simulation::click_region_center_held_cancellable(
                    region,
                    super::MOUSE_CLICK_HOLD_MS,
                    Arc::clone(&action_cancelled),
                )
                .await?;
                crate::input_simulation::move_region_center_cancellable(
                    mouse_parking_region,
                    action_cancelled,
                )
                .await
            },
        )
        .await?;
        if matches!(target_key, "wegame.loginMode" | "wegame.accountDropdown") {
            self.login_takeover_active.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    async fn find_process_window(
        &self,
        executable: &Path,
    ) -> Result<Option<WindowIdentity>, String> {
        let executable = executable.to_path_buf();
        let window = tokio::task::spawn_blocking(move || {
            WindowsDesktopRuntime.find_primary_window(&executable)
        })
        .await
        .map_err(|error| format!("窗口查找任务失败: {error}"))??;
        if window.is_none() {
            self.set_observation(LoginObservation::WindowNotFound);
        }
        Ok(window)
    }
}

impl RememberedAccountDriver for ProductionLoginDriver {
    async fn visible_account_rows(
        &self,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<AccountRowSlot>, String> {
        const MAX_ATTEMPTS: u8 = 3;
        let list_height = self
            .config
            .targets
            .get("wegame.accountList")
            .ok_or_else(|| "登录校准目标 wegame.accountList 不存在".to_string())?
            .region
            .height;
        for attempt in 1..=MAX_ATTEMPTS {
            self.emit_update(
                LoginRunStatus::Waiting,
                format!("正在确认已记住账号列表（第 {attempt}/{MAX_ATTEMPTS} 次）"),
                None,
            )?;
            let first = self.sample_accounts(Arc::clone(&cancelled)).await?;
            wait_cancellable(Duration::from_millis(400), &cancelled).await?;
            let second = self.sample_accounts(Arc::clone(&cancelled)).await?;
            if super::remembered_account::account_list_visible_in_both_samples(&first, &second) {
                return Ok(super::remembered_account::derive_visible_row_slots(
                    &first,
                    &second,
                    list_height,
                ));
            }
            if attempt < MAX_ATTEMPTS {
                wait_cancellable(Duration::from_millis(400), &cancelled).await?;
            }
        }
        Err(super::remembered_account::ACCOUNT_LIST_UNAVAILABLE.to_string())
    }

    async fn open_account_list(&self, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        <Self as LoginDriver>::click(self, "wegame.accountDropdown", cancelled).await
    }

    async fn scroll_down(&self, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        let region = self
            .config
            .targets
            .get("wegame.accountList")
            .ok_or_else(|| "登录校准目标 wegame.accountList 不存在".to_string())?
            .region
            .clone();
        let focus_cancelled = Arc::clone(&cancelled);
        let guard_cancelled = Arc::clone(&cancelled);
        let action_cancelled = Arc::clone(&cancelled);
        let mouse_parking_region = self.config.mouse_parking_region.clone();
        run_checked_action(
            self.countdown(&cancelled),
            async {
                ensure_not_cancelled(&focus_cancelled)?;
                self.focus_wegame().await
            },
            async {
                ensure_not_cancelled(&guard_cancelled)?;
                self.verify_account_list_open(guard_cancelled).await
            },
            async {
                ensure_not_cancelled(&action_cancelled)?;
                self.emit_update(LoginRunStatus::Inputting, "正在滚动账号列表", None)?;
                crate::input_simulation::scroll_region_down_cancellable(
                    region,
                    3,
                    Arc::clone(&action_cancelled),
                )
                .await?;
                crate::input_simulation::move_region_center_cancellable(
                    mouse_parking_region,
                    action_cancelled,
                )
                .await
            },
        )
        .await
    }

    async fn select_row(
        &self,
        slot: AccountRowSlot,
        cancelled: Arc<AtomicBool>,
    ) -> Result<AccountRowClick, String> {
        let row = slot.index();
        if row >= 3 {
            return Err(format!("账号列表行号无效: {row}"));
        }
        let region = self
            .config
            .targets
            .get("wegame.accountList")
            .ok_or_else(|| "登录校准目标 wegame.accountList 不存在".to_string())?
            .region
            .clone();
        let x = region.x.saturating_add(region.width / 2);
        let y = match slot {
            AccountRowSlot::Ocr { center_y, .. } => region.y.saturating_add(center_y),
            AccountRowSlot::Fallback { .. } => region
                .y
                .saturating_add(region.height.saturating_mul(i32::from(row) * 2 + 1) / 6),
        };
        let focus_cancelled = Arc::clone(&cancelled);
        let guard_cancelled = Arc::clone(&cancelled);
        let action_cancelled = Arc::clone(&cancelled);
        let mouse_parking_region = self.config.mouse_parking_region.clone();
        run_checked_action(
            self.countdown(&cancelled),
            async {
                ensure_not_cancelled(&focus_cancelled)?;
                self.focus_wegame().await
            },
            async {
                ensure_not_cancelled(&guard_cancelled)?;
                Ok(())
            },
            async {
                ensure_not_cancelled(&action_cancelled)?;
                self.emit_update(LoginRunStatus::Inputting, "正在选择目标 QQ", None)?;
                crate::input_simulation::click_screen_point_held_cancellable(
                    x,
                    y,
                    super::MOUSE_CLICK_HOLD_MS,
                    Arc::clone(&action_cancelled),
                )
                .await?;
                crate::input_simulation::move_region_center_cancellable(
                    mouse_parking_region,
                    action_cancelled,
                )
                .await
            },
        )
        .await?;
        Ok(AccountRowClick { index: row, x, y })
    }

    async fn copy_selected_qq(&self, cancelled: Arc<AtomicBool>) -> Result<String, String> {
        ensure_not_cancelled(&cancelled)?;
        tokio::task::spawn_blocking(super::windows_clipboard::clear_clipboard)
            .await
            .map_err(|error| format!("清空剪贴板任务失败: {error}"))??;
        let region = self
            .config
            .targets
            .get("wegame.selectedAccount")
            .ok_or_else(|| "登录校准目标 wegame.selectedAccount 不存在".to_string())?
            .region
            .clone();
        let focus_cancelled = Arc::clone(&cancelled);
        let guard_cancelled = Arc::clone(&cancelled);
        let action_cancelled = Arc::clone(&cancelled);
        let mouse_parking_region = self.config.mouse_parking_region.clone();
        run_checked_action(
            self.countdown(&cancelled),
            async {
                ensure_not_cancelled(&focus_cancelled)?;
                self.focus_wegame().await
            },
            async {
                ensure_not_cancelled(&guard_cancelled)?;
                self.verify_action_guard("wegame.selectedAccount", guard_cancelled)
                    .await
            },
            async {
                ensure_not_cancelled(&action_cancelled)?;
                self.emit_update(LoginRunStatus::Inputting, "正在复制目标 QQ", None)?;
                crate::input_simulation::double_click_region_and_copy_held_cancellable(
                    region,
                    super::MOUSE_CLICK_HOLD_MS,
                    Arc::clone(&action_cancelled),
                )
                .await?;
                crate::input_simulation::move_region_center_cancellable(
                    mouse_parking_region,
                    action_cancelled,
                )
                .await
            },
        )
        .await?;
        ensure_not_cancelled(&cancelled)?;
        tokio::task::spawn_blocking(super::windows_clipboard::read_copied_qq)
            .await
            .map_err(|error| format!("读取剪贴板任务失败: {error}"))?
    }
}

async fn run_checked_action<C, F, G, A, T>(
    countdown: C,
    focus: F,
    guard: G,
    action: A,
) -> Result<T, String>
where
    C: Future<Output = Result<(), String>> + Send,
    F: Future<Output = Result<(), String>> + Send,
    G: Future<Output = Result<(), String>> + Send,
    A: Future<Output = Result<T, String>> + Send,
    T: Send,
{
    countdown.await?;
    focus.await?;
    guard.await?;
    action.await
}

fn observation_for_error(error: &str) -> LoginObservation {
    if error.contains("参考图") {
        LoginObservation::ReferenceImageFailed
    } else {
        LoginObservation::CaptureFailed
    }
}

fn launch_observation(error: &str) -> LoginObservation {
    LoginObservation::LaunchFailed {
        windows_error_code: windows_error_code(error),
    }
}

fn process_observation(error: &str) -> LoginObservation {
    LoginObservation::ProcessFailed {
        windows_error_code: windows_error_code(error),
    }
}

fn windows_error_code(error: &str) -> Option<i32> {
    error
        .rsplit_once("（Windows 错误 ")
        .map(|(_, value)| value)
        .and_then(|value| value.strip_suffix('）'))
        .and_then(|value| value.parse().ok())
}

fn window_observation(error: &str) -> LoginObservation {
    if error.contains("未找到 WeGame 窗口") {
        LoginObservation::WindowNotFound
    } else {
        LoginObservation::WindowOperationFailed
    }
}

fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::SeqCst) {
        Err("登录试运行已取消".to_string())
    } else {
        Ok(())
    }
}

async fn wait_cancellable(duration: Duration, cancelled: &AtomicBool) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + duration;
    loop {
        ensure_not_cancelled(cancelled)?;
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Ok(());
        }
        tokio::time::sleep(remaining.min(Duration::from_millis(50))).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_changed_targets_main_and_operation_overlay() {
        assert_eq!(
            run_changed_target_labels(),
            ["main", "special-ops-operation"]
        );
    }

    #[test]
    fn launch_error_observation_only_keeps_windows_error_code() {
        assert_eq!(
            launch_observation("启动程序失败（Windows 错误 740）"),
            LoginObservation::LaunchFailed {
                windows_error_code: Some(740),
            }
        );
        assert_eq!(
            launch_observation("RAW_DRIVER_SECRET|C:\\private\\wegame.exe"),
            LoginObservation::LaunchFailed {
                windows_error_code: None,
            }
        );
        assert_eq!(
            launch_observation("规范化程序路径失败（Windows 错误 2）"),
            LoginObservation::LaunchFailed {
                windows_error_code: Some(2),
            }
        );
    }

    #[test]
    fn process_error_observation_only_keeps_windows_error_code() {
        assert_eq!(
            process_observation("打开待结束进程失败: PID 10（Windows 错误 5）"),
            LoginObservation::ProcessFailed {
                windows_error_code: Some(5),
            }
        );
        assert_eq!(
            process_observation("RAW_DRIVER_SECRET|C:\\private\\game.exe"),
            LoginObservation::ProcessFailed {
                windows_error_code: None,
            }
        );
    }

    #[test]
    fn window_error_observation_distinguishes_search_from_window_operation() {
        assert_eq!(
            window_observation("未找到 WeGame 窗口"),
            LoginObservation::WindowNotFound
        );
        assert_eq!(
            window_observation("聚焦目标窗口失败：C:\\private\\wegame.exe"),
            LoginObservation::WindowOperationFailed
        );
    }

    #[test]
    fn only_one_login_run_can_be_active() {
        let runtime = LoginRuntime::default();
        let first = runtime.try_start("account-a".to_string()).unwrap();

        let error = runtime.try_start("account-b".to_string()).unwrap_err();

        assert!(error.contains("已有登录试运行"));
        assert_eq!(runtime.snapshot().unwrap().unwrap().run_id, first.run_id);
    }

    #[test]
    fn stale_run_update_cannot_overwrite_new_run() {
        let runtime = LoginRuntime::default();
        let old = runtime.try_start("old".to_string()).unwrap();
        runtime
            .finish(old.run_id, LoginRunStatus::Succeeded, "完成")
            .unwrap();
        let current = runtime.try_start("current".to_string()).unwrap();

        assert!(runtime
            .update(
                old.run_id,
                LoginRunStatus::Failed,
                Some(LoginStep::SelectRememberedAccount),
                "旧任务失败",
                None,
            )
            .unwrap()
            .is_none());

        let snapshot = runtime.snapshot().unwrap().unwrap();
        assert_eq!(snapshot.run_id, current.run_id);
        assert_eq!(snapshot.status, LoginRunStatus::Starting);
    }

    #[test]
    fn cancel_keeps_singleton_until_finish_then_allows_new_run() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();

        let cancelled = runtime
            .request_stop(current.run_id, StopReason::Normal)
            .unwrap()
            .unwrap();
        assert_eq!(cancelled.status, LoginRunStatus::Stopping);
        assert!(current.cancelled.load(Ordering::SeqCst));
        assert!(runtime.try_start("account-b".to_string()).is_err());

        runtime
            .finish(current.run_id, LoginRunStatus::Stopped, "已停止")
            .unwrap();
        assert!(runtime.try_start("account-b".to_string()).is_ok());
    }

    #[test]
    fn stop_request_stays_stopping_until_worker_finishes() {
        let runtime = LoginRuntime::default();
        let run = runtime.try_start("account-a".to_string()).unwrap();

        let stopping = runtime
            .request_stop(run.run_id, StopReason::Normal)
            .unwrap()
            .unwrap();
        assert_eq!(stopping.status, LoginRunStatus::Stopping);
        assert!(runtime.snapshot().unwrap().is_some());

        let stopped = runtime
            .finish(run.run_id, LoginRunStatus::Failed, "制作试运行已停止")
            .unwrap()
            .unwrap();
        assert_eq!(stopped.status, LoginRunStatus::Stopped);
        assert!(runtime.snapshot().unwrap().is_none());
    }

    #[test]
    fn stopped_snapshot_rejects_late_worker_update() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();
        runtime
            .request_stop(current.run_id, StopReason::Emergency)
            .unwrap()
            .unwrap();

        let updated = runtime
            .update(
                current.run_id,
                LoginRunStatus::Waiting,
                Some(LoginStep::WaitGameWindow),
                "旧 worker 更新",
                None,
            )
            .unwrap();

        assert!(updated.is_none());
        let snapshot = runtime.snapshot().unwrap().unwrap();
        assert_eq!(snapshot.status, LoginRunStatus::Stopping);
        assert_eq!(snapshot.current_step, None);
        assert_eq!(snapshot.message, "正在执行紧急停止");
    }

    #[test]
    fn stale_lifecycle_stop_request_does_not_cancel_replacement_run() {
        let runtime = LoginRuntime::default();
        let old = runtime.try_start("old".to_string()).unwrap();
        runtime
            .update(
                old.run_id,
                LoginRunStatus::Inputting,
                Some(LoginStep::SelectRememberedAccount),
                "旧 run 已进入输入阶段",
                None,
            )
            .unwrap();
        runtime
            .finish(old.run_id, LoginRunStatus::Succeeded, "旧 run 结束")
            .unwrap();
        let current = runtime.try_start("current".to_string()).unwrap();

        let stopped = runtime.request_lifecycle_stop(old.run_id).unwrap();

        assert!(stopped.is_none());
        assert!(!current.cancelled.load(Ordering::SeqCst));
        let snapshot = runtime.snapshot().unwrap().unwrap();
        assert_eq!(snapshot.run_id, current.run_id);
        assert_eq!(snapshot.status, LoginRunStatus::Starting);
    }

    #[test]
    fn stale_cancel_request_does_not_cancel_replacement_run() {
        let runtime = LoginRuntime::default();
        let old = runtime.try_start("old".to_string()).unwrap();
        runtime
            .finish(old.run_id, LoginRunStatus::Succeeded, "旧 run 结束")
            .unwrap();
        let current = runtime.try_start("current".to_string()).unwrap();

        let stopped = runtime
            .request_stop(old.run_id, StopReason::Normal)
            .unwrap();

        assert!(stopped.is_none());
        assert!(!current.cancelled.load(Ordering::SeqCst));
        let snapshot = runtime.snapshot().unwrap().unwrap();
        assert_eq!(snapshot.run_id, current.run_id);
        assert_eq!(snapshot.status, LoginRunStatus::Starting);
    }

    #[test]
    fn stale_emergency_request_does_not_cancel_or_persist_replacement_run() {
        let runtime = LoginRuntime::default();
        let old = runtime.try_start("old".to_string()).unwrap();
        runtime
            .finish(old.run_id, LoginRunStatus::Succeeded, "旧 run 结束")
            .unwrap();
        let current = runtime.try_start("current".to_string()).unwrap();
        let writes = std::sync::atomic::AtomicUsize::new(0);

        let stopped = runtime
            .request_stop(old.run_id, StopReason::Emergency)
            .unwrap();
        if stopped.is_some() {
            writes.fetch_add(1, Ordering::SeqCst);
        }

        assert!(stopped.is_none());
        assert_eq!(writes.load(Ordering::SeqCst), 0);
        assert!(!current.cancelled.load(Ordering::SeqCst));
        let snapshot = runtime.snapshot().unwrap().unwrap();
        assert_eq!(snapshot.run_id, current.run_id);
        assert_eq!(snapshot.status, LoginRunStatus::Starting);
    }

    #[test]
    fn emergency_persistence_claim_blocks_stale_flow_claim() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();
        runtime
            .request_stop(current.run_id, StopReason::Emergency)
            .unwrap()
            .unwrap();

        let PersistenceClaim::Acquired(emergency) =
            runtime.claim_persistence(current.run_id).unwrap()
        else {
            panic!("应取得紧急停止持久化权限");
        };
        assert_eq!(
            emergency.kind(),
            PersistenceKind::Stop(StopReason::Emergency)
        );
        assert!(emergency.complete().unwrap());

        assert!(matches!(
            runtime.claim_persistence(current.run_id).unwrap(),
            PersistenceClaim::Persisted
        ));
    }

    #[test]
    fn emergency_stop_blocks_worker_cleanup_until_persisted() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();
        runtime
            .request_stop(current.run_id, StopReason::Emergency)
            .unwrap()
            .unwrap();

        assert!(!runtime.cleanup_ready(current.run_id).unwrap());
        let PersistenceClaim::Acquired(guard) = runtime.claim_persistence(current.run_id).unwrap()
        else {
            panic!("应取得紧急停止持久化权限");
        };
        assert!(guard.complete().unwrap());
        assert!(runtime.cleanup_ready(current.run_id).unwrap());
    }

    #[test]
    fn failed_emergency_persistence_releases_claim_for_retry() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();
        runtime
            .request_stop(current.run_id, StopReason::Emergency)
            .unwrap()
            .unwrap();
        let guard = match runtime.claim_persistence(current.run_id).unwrap() {
            PersistenceClaim::Acquired(guard) => guard,
            other => panic!("应取得持久化权限，实际为 {other:?}"),
        };

        drop(guard);

        let PersistenceClaim::Acquired(retried) =
            runtime.claim_persistence(current.run_id).unwrap()
        else {
            panic!("应重新取得紧急停止持久化权限");
        };
        assert_eq!(retried.kind(), PersistenceKind::Stop(StopReason::Emergency));
    }

    #[test]
    fn emergency_registered_during_flow_claim_requires_stop_persistence_after_flow() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();
        let PersistenceClaim::Acquired(flow) = runtime.claim_persistence(current.run_id).unwrap()
        else {
            panic!("应取得流程持久化权限");
        };
        assert_eq!(flow.kind(), PersistenceKind::Flow);

        runtime
            .request_stop(current.run_id, StopReason::Emergency)
            .unwrap()
            .unwrap();
        assert!(!flow.complete().unwrap());

        let PersistenceClaim::Acquired(emergency) =
            runtime.claim_persistence(current.run_id).unwrap()
        else {
            panic!("应取得紧急停止持久化权限");
        };
        assert_eq!(
            emergency.kind(),
            PersistenceKind::Stop(StopReason::Emergency)
        );
    }

    #[test]
    fn persistence_claim_distinguishes_terminal_states() {
        let runtime = LoginRuntime::default();

        assert!(matches!(
            runtime.claim_persistence(1).unwrap(),
            PersistenceClaim::NoActive
        ));

        let current = runtime.try_start("account-a".to_string()).unwrap();
        assert!(matches!(
            runtime.claim_persistence(current.run_id + 1).unwrap(),
            PersistenceClaim::Stale
        ));
        runtime
            .request_stop(current.run_id, StopReason::Normal)
            .unwrap()
            .unwrap();
        assert!(matches!(
            runtime.claim_persistence(current.run_id).unwrap(),
            PersistenceClaim::NoPersistence
        ));
    }

    #[test]
    fn persistence_guard_drop_after_panic_releases_claim() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();

        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let PersistenceClaim::Acquired(_guard) =
                runtime.claim_persistence(current.run_id).unwrap()
            else {
                panic!("应取得持久化权限");
            };
            panic!("模拟持久化 owner panic");
        }));

        assert!(matches!(
            runtime.claim_persistence(current.run_id).unwrap(),
            PersistenceClaim::Acquired(_)
        ));
    }

    #[test]
    fn run_snapshot_is_camel_case_and_contains_no_password_field() {
        let runtime = LoginRuntime::default();
        runtime
            .try_start_kind("account-a".to_string(), LoginRunKind::Craft)
            .unwrap();

        let json = serde_json::to_string(&runtime.snapshot().unwrap().unwrap()).unwrap();

        assert!(json.contains("\"runId\""));
        assert!(json.contains("\"accountId\""));
        assert!(json.contains("\"runKind\":\"craft\""));
        assert!(json.contains("\"currentStep\""));
        assert!(json.contains("\"countdownSeconds\""));
        assert!(!json.contains("password"));
        assert!(!json.contains("settings"));
    }

    #[test]
    fn round_snapshot_serializes_progress_in_camel_case() {
        let runtime = LoginRuntime::default();
        let started = runtime
            .try_start_kind("account-a".to_string(), LoginRunKind::Round)
            .unwrap();

        runtime
            .update_round_progress(
                started.run_id,
                2,
                4,
                "account-b",
                "123456789",
                Some(crate::special_ops::StationKind::Workbench),
                1,
                3,
            )
            .unwrap();

        let value = serde_json::to_value(runtime.snapshot().unwrap().unwrap()).unwrap();
        assert_eq!(value["runKind"], "round");
        assert_eq!(value["accountId"], "account-b");
        assert_eq!(value["roundProgress"]["accountIndex"], 2);
        assert_eq!(value["roundProgress"]["stationKind"], "workbench");
    }

    #[test]
    fn poisoned_runtime_fails_closed() {
        let runtime = LoginRuntime::default();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = runtime.inner.lock().unwrap();
            panic!("测试 mutex poison");
        }));

        let error = runtime.try_start("account-a".to_string()).unwrap_err();

        assert!(error.contains("已损坏"));
        assert!(error.contains("拒绝启动"));
    }

    #[tokio::test(start_paused = true)]
    async fn countdown_emits_three_two_one_and_stops_on_cancel() {
        let runtime = Arc::new(LoginRuntime::default());
        let started = runtime.try_start("account-a".to_string()).unwrap();
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let worker_runtime = Arc::clone(&runtime);
        let worker_cancelled = Arc::clone(&started.cancelled);
        let worker_emitted = Arc::clone(&emitted);
        let task = tokio::spawn(async move {
            for seconds in [3, 2, 1] {
                ensure_not_cancelled(&worker_cancelled)?;
                let snapshot = worker_runtime
                    .update(
                        started.run_id,
                        LoginRunStatus::Countdown,
                        None,
                        "倒计时",
                        Some(seconds),
                    )?
                    .unwrap();
                worker_emitted
                    .lock()
                    .unwrap()
                    .push(snapshot.countdown_seconds.unwrap());
                wait_cancellable(Duration::from_secs(1), &worker_cancelled).await?;
            }
            Ok::<(), String>(())
        });
        tokio::time::advance(Duration::from_secs(3)).await;
        task.await.unwrap().unwrap();
        assert_eq!(*emitted.lock().unwrap(), [3, 2, 1]);

        let second = LoginRuntime::default();
        let started = second.try_start("account-b".to_string()).unwrap();
        started.cancelled.store(true, Ordering::SeqCst);
        assert!(wait_cancellable(Duration::from_secs(1), &started.cancelled)
            .await
            .is_err());
    }

    #[test]
    fn normal_cancel_message_matches_run_kind() {
        assert_eq!(
            LoginRunKind::Login.normal_cancel_message(),
            "正在取消登录试运行"
        );
        assert_eq!(
            LoginRunKind::Navigation.normal_cancel_message(),
            "正在取消游戏内导航试运行"
        );
        assert_eq!(
            LoginRunKind::Craft.normal_cancel_message(),
            "正在取消制作试运行"
        );
        assert_eq!(LoginRunKind::Ammo.query_value(), "ammo");
        assert_eq!(
            LoginRunKind::Ammo.normal_cancel_message(),
            "正在取消子弹兑换试运行"
        );
        assert_eq!(LoginRunKind::LimitedSupply.query_value(), "limitedSupply");
        assert_eq!(
            LoginRunKind::LimitedSupply.normal_cancel_message(),
            "正在取消限时商品试运行"
        );
        assert_eq!(LoginRunKind::Market.query_value(), "market");
        assert_eq!(
            LoginRunKind::Market.normal_cancel_message(),
            "正在取消交易行试运行"
        );
        assert_eq!(
            LoginRunKind::StationWalkthrough.query_value(),
            "stationWalkthrough"
        );
        assert_eq!(
            LoginRunKind::StationWalkthrough.normal_cancel_message(),
            "正在取消多账号制作台更改"
        );
    }

    #[test]
    fn login_takeover_suppresses_countdown_until_released() {
        assert!(countdown_required(false));
        assert!(!countdown_required(true));
        assert!(countdown_required(false));
    }

    #[test]
    fn first_input_gets_five_and_followups_get_zero() {
        let runtime = LoginRuntime::default();
        let started = runtime.try_start("account-a".to_string()).unwrap();

        assert_eq!(
            runtime
                .next_input_countdown_seconds(started.run_id, false)
                .unwrap(),
            Some(5)
        );
        assert_eq!(
            runtime
                .next_input_countdown_seconds(started.run_id, false)
                .unwrap(),
            None
        );
        assert_eq!(
            runtime
                .next_input_countdown_seconds(started.run_id, true)
                .unwrap(),
            Some(0)
        );
    }

    #[tokio::test]
    async fn focus_or_guard_failure_prevents_input_action() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let focus_calls = Arc::clone(&calls);
        let guard_calls = Arc::clone(&calls);
        let action_calls = Arc::clone(&calls);
        let error = run_checked_action(
            async { Ok::<(), String>(()) },
            async move {
                focus_calls.lock().unwrap().push("focus");
                Err::<(), String>("聚焦失败".to_string())
            },
            async move {
                guard_calls.lock().unwrap().push("guard");
                Ok::<(), String>(())
            },
            async move {
                action_calls.lock().unwrap().push("action");
                Ok::<(), String>(())
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error, "聚焦失败");
        assert_eq!(*calls.lock().unwrap(), ["focus"]);

        calls.lock().unwrap().clear();
        let guard_calls = Arc::clone(&calls);
        let action_calls = Arc::clone(&calls);
        assert!(run_checked_action(
            async { Ok::<(), String>(()) },
            async { Ok::<(), String>(()) },
            async move {
                guard_calls.lock().unwrap().push("guard");
                Err::<(), String>("守卫失败".to_string())
            },
            async move {
                action_calls.lock().unwrap().push("action");
                Ok::<(), String>(())
            },
        )
        .await
        .is_err());
        assert_eq!(*calls.lock().unwrap(), ["guard"]);
    }
}
