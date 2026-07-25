use super::login_flow::LoginStep;
use super::{
    desktop_runtime::{DesktopRuntime, WindowIdentity, WindowsDesktopRuntime},
    login_flow::{LoginDriver, LoginObservation, LoginRunConfig},
    template_observer::{RuntimeSimilaritySampler, RuntimeTemplate},
};
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoginRunSnapshot {
    pub run_id: u64,
    pub account_id: String,
    pub status: LoginRunStatus,
    pub current_step: Option<LoginStep>,
    pub message: String,
    pub countdown_seconds: Option<u8>,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LoginRunStatus {
    Starting,
    Waiting,
    Countdown,
    Inputting,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistenceClaim {
    Acquired(PersistenceKind),
    Pending,
    Resolved,
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
    persistence_state: PersistenceState,
    cleanup_failed: bool,
}

struct LoginRuntimeInner {
    next_run_id: u64,
    active: Option<ActiveLoginRun>,
}

pub(crate) struct LoginRuntime {
    inner: Mutex<LoginRuntimeInner>,
    persistence_changed: Condvar,
}

impl Default for LoginRuntime {
    fn default() -> Self {
        Self {
            inner: Mutex::new(LoginRuntimeInner {
                next_run_id: 1,
                active: None,
            }),
            persistence_changed: Condvar::new(),
        }
    }
}

impl LoginRuntime {
    pub(crate) fn try_start(&self, account_id: String) -> Result<StartedLoginRun, String> {
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
                status: LoginRunStatus::Starting,
                current_step: None,
                message: "正在准备登录试运行".to_string(),
                countdown_seconds: None,
                started_at_ms: now,
                updated_at_ms: now,
            },
            cancelled: Arc::clone(&cancelled),
            stop_reason: None,
            entered_input: false,
            persistence_state: PersistenceState::Unclaimed,
            cleanup_failed: false,
        });
        Ok(StartedLoginRun { run_id, cancelled })
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

    pub(crate) fn finish(
        &self,
        run_id: u64,
        status: LoginRunStatus,
        message: impl Into<String>,
    ) -> Result<Option<LoginRunSnapshot>, String> {
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
        Ok(Some(active.snapshot))
    }

    pub(crate) fn cancel_active(&self) -> Result<LoginRunSnapshot, String> {
        self.request_stop(StopReason::Normal)
    }

    pub(crate) fn request_stop(&self, reason: StopReason) -> Result<LoginRunSnapshot, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏，拒绝停止".to_string())?;
        let active = inner
            .active
            .as_mut()
            .ok_or_else(|| "当前没有运行中的登录试运行".to_string())?;
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
        active.snapshot.status = LoginRunStatus::Stopped;
        active.snapshot.current_step = None;
        active.snapshot.message = match effective_reason {
            StopReason::Normal => "正在取消登录试运行",
            StopReason::Emergency => "正在执行紧急停止",
            StopReason::Lifecycle { .. } => "正在停止登录试运行",
        }
        .to_string();
        active.snapshot.countdown_seconds = None;
        active.snapshot.updated_at_ms = super::now_ms();
        Ok(active.snapshot.clone())
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

    pub(crate) fn claim_persistence(&self, run_id: u64) -> Result<PersistenceClaim, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "登录试运行状态已损坏".to_string())?;
        let Some(active) = inner
            .active
            .as_mut()
            .filter(|active| active.snapshot.run_id == run_id)
        else {
            return Ok(PersistenceClaim::Resolved);
        };
        match active.persistence_state {
            PersistenceState::InProgress(_) => Ok(PersistenceClaim::Pending),
            PersistenceState::Persisted => Ok(PersistenceClaim::Resolved),
            PersistenceState::Unclaimed => {
                let Some(kind) = desired_persistence_kind(active.stop_reason) else {
                    return Ok(PersistenceClaim::Resolved);
                };
                active.persistence_state = PersistenceState::InProgress(kind);
                Ok(PersistenceClaim::Acquired(kind))
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
            let _ = self.app.emit_to("main", RUN_CHANGED, snapshot);
        }
        Ok(())
    }

    fn set_observation(&self, observation: LoginObservation) {
        if let Ok(mut current) = self.observation.lock() {
            *current = observation;
        }
    }

    async fn countdown(&self, cancelled: &Arc<AtomicBool>) -> Result<(), String> {
        for seconds in [3, 2, 1] {
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
                .find_primary_window(&executable)?
                .ok_or_else(|| "未找到 WeGame 窗口".to_string())?;
            runtime.restore_and_focus(&executable, window)
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

    async fn terminate_exact(&self, executable: &Path) -> Result<(), String> {
        let executable = executable.to_path_buf();
        tokio::task::spawn_blocking(move || {
            WindowsDesktopRuntime.terminate_exact(&executable, Duration::from_secs(15))
        })
        .await
        .map_err(|error| format!("进程结束任务失败: {error}"))?
    }

    async fn launch(&self, executable: &Path) -> Result<u32, String> {
        let executable = executable.to_path_buf();
        tokio::task::spawn_blocking(move || WindowsDesktopRuntime.launch(&executable))
            .await
            .map_err(|error| format!("程序启动任务失败: {error}"))?
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
        run_checked_action(
            self.countdown(&cancelled),
            async {
                ensure_not_cancelled(&focus_cancelled)?;
                self.focus_wegame().await.inspect_err(|_| {
                    self.set_observation(LoginObservation::WindowNotFound);
                })
            },
            async {
                ensure_not_cancelled(&guard_cancelled)?;
                self.verify_action_guard(target_key, guard_cancelled).await
            },
            async {
                ensure_not_cancelled(&action_cancelled)?;
                self.emit_update(LoginRunStatus::Inputting, "正在执行键鼠操作", None)?;
                crate::input_simulation::click_region_center_cancellable(region, action_cancelled)
                    .await
            },
        )
        .await
    }

    async fn replace_text(
        &self,
        target_key: &str,
        value: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let region = self
            .config
            .targets
            .get(target_key)
            .ok_or_else(|| format!("登录校准目标 {target_key} 不存在"))?
            .region
            .clone();
        let value = value.to_string();
        let focus_cancelled = Arc::clone(&cancelled);
        let guard_cancelled = Arc::clone(&cancelled);
        let action_cancelled = Arc::clone(&cancelled);
        run_checked_action(
            self.countdown(&cancelled),
            async {
                ensure_not_cancelled(&focus_cancelled)?;
                self.focus_wegame().await.inspect_err(|_| {
                    self.set_observation(LoginObservation::WindowNotFound);
                })
            },
            async {
                ensure_not_cancelled(&guard_cancelled)?;
                self.verify_action_guard(target_key, guard_cancelled).await
            },
            async {
                ensure_not_cancelled(&action_cancelled)?;
                self.emit_update(LoginRunStatus::Inputting, "正在执行键鼠操作", None)?;
                crate::input_simulation::replace_text_at_region_cancellable(
                    region,
                    value,
                    25,
                    action_cancelled,
                )
                .await
            },
        )
        .await
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
                Some(LoginStep::InputPassword),
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

        let cancelled = runtime.cancel_active().unwrap();
        assert_eq!(cancelled.status, LoginRunStatus::Stopped);
        assert!(current.cancelled.load(Ordering::SeqCst));
        assert!(runtime.try_start("account-b".to_string()).is_err());

        runtime
            .finish(current.run_id, LoginRunStatus::Stopped, "已停止")
            .unwrap();
        assert!(runtime.try_start("account-b".to_string()).is_ok());
    }

    #[test]
    fn stopped_snapshot_rejects_late_worker_update() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();
        runtime.request_stop(StopReason::Emergency).unwrap();

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
        assert_eq!(snapshot.status, LoginRunStatus::Stopped);
        assert_eq!(snapshot.current_step, None);
        assert_eq!(snapshot.message, "正在执行紧急停止");
    }

    #[test]
    fn emergency_persistence_claim_blocks_stale_flow_claim() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();
        runtime.request_stop(StopReason::Emergency).unwrap();

        let emergency = runtime.claim_persistence(current.run_id).unwrap();
        assert_eq!(
            emergency,
            PersistenceClaim::Acquired(PersistenceKind::Stop(StopReason::Emergency))
        );
        runtime
            .complete_persistence(current.run_id, PersistenceKind::Stop(StopReason::Emergency))
            .unwrap();

        assert_eq!(
            runtime.claim_persistence(current.run_id).unwrap(),
            PersistenceClaim::Resolved
        );
    }

    #[test]
    fn failed_emergency_persistence_releases_claim_for_retry() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();
        runtime.request_stop(StopReason::Emergency).unwrap();
        let kind = match runtime.claim_persistence(current.run_id).unwrap() {
            PersistenceClaim::Acquired(kind) => kind,
            other => panic!("应取得持久化权限，实际为 {other:?}"),
        };

        runtime.release_persistence(current.run_id, kind).unwrap();

        assert_eq!(
            runtime.claim_persistence(current.run_id).unwrap(),
            PersistenceClaim::Acquired(PersistenceKind::Stop(StopReason::Emergency))
        );
    }

    #[test]
    fn emergency_registered_during_flow_claim_requires_stop_persistence_after_flow() {
        let runtime = LoginRuntime::default();
        let current = runtime.try_start("account-a".to_string()).unwrap();
        assert_eq!(
            runtime.claim_persistence(current.run_id).unwrap(),
            PersistenceClaim::Acquired(PersistenceKind::Flow)
        );

        runtime.request_stop(StopReason::Emergency).unwrap();
        runtime
            .complete_persistence(current.run_id, PersistenceKind::Flow)
            .unwrap();

        assert_eq!(
            runtime.claim_persistence(current.run_id).unwrap(),
            PersistenceClaim::Acquired(PersistenceKind::Stop(StopReason::Emergency))
        );
    }

    #[test]
    fn run_snapshot_is_camel_case_and_contains_no_password_field() {
        let runtime = LoginRuntime::default();
        runtime.try_start("account-a".to_string()).unwrap();

        let json = serde_json::to_string(&runtime.snapshot().unwrap().unwrap()).unwrap();

        assert!(json.contains("\"runId\""));
        assert!(json.contains("\"accountId\""));
        assert!(json.contains("\"currentStep\""));
        assert!(json.contains("\"countdownSeconds\""));
        assert!(!json.contains("password"));
        assert!(!json.contains("settings"));
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
