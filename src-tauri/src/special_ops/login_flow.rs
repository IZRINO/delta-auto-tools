use super::{
    desktop_runtime::WindowIdentity,
    remembered_account::{
        format_scan_attempts, redact_qq, select_remembered_account, AccountSelectionError,
        AccountSelectionPhase, RememberedAccountDriver,
    },
    template_observer::RuntimeTarget,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(crate) const STEP_TIMEOUT: Duration = Duration::from_secs(180);
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(50);
const WINDOW_POLL_INTERVAL: Duration = Duration::from_millis(250);
const TERMINATE_RETRY_WAIT: Duration = Duration::from_secs(60);

pub(crate) struct LoginRunConfig {
    pub account_id: String,
    pub qq_account: String,
    pub wegame_executable_path: PathBuf,
    pub game_executable_path: PathBuf,
    pub mouse_parking_region: crate::morse::types::RegionRect,
    pub targets: HashMap<String, RuntimeTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LoginObservation {
    None,
    TemplateSamples { samples: [f32; 2] },
    CaptureFailed,
    ReferenceImageFailed,
    WindowNotFound,
    WindowOperationFailed,
    LaunchFailed { windows_error_code: Option<i32> },
    ProcessFailed { windows_error_code: Option<i32> },
}

#[allow(async_fn_in_trait)]
pub(crate) trait LoginDriver: RememberedAccountDriver + Send + Sync {
    fn reset_observation(&self);
    fn last_observation(&self) -> LoginObservation;
    async fn initial_countdown(&self, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn terminate_exact(&self, executable: &Path) -> Result<(), String>;
    async fn launch(&self, executable: &Path) -> Result<u32, String>;
    async fn wait_for_any(
        &self,
        target_keys: &[&str],
        cancelled: Arc<AtomicBool>,
    ) -> Result<String, String>;
    async fn click(&self, target_key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn find_process_window(
        &self,
        executable: &Path,
    ) -> Result<Option<WindowIdentity>, String>;
    async fn restore_window(&self, executable: &Path) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LoginStep {
    InitialCountdown,
    StopGame,
    StopWeGame,
    StartWeGame,
    WaitLoginChoice,
    OpenLoginForm,
    OpenAccountList,
    ScanRememberedAccounts,
    SelectRememberedAccount,
    VerifySelectedAccount,
    SubmitLogin,
    WaitGameEntry,
    OpenGameEntry,
    WaitLaunchButton,
    LaunchGame,
    WaitGameWindow,
    WaitModeReady,
    OpenBeaconMode,
    DismissActivityPopup,
    SwitchLobbyView,
    OpenSpecialOps,
    WaitStationGrid,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum LoginFlowResult {
    GameReady {
        account_id: String,
        qq_account: String,
        game_process_id: u32,
        game_window_handle: u64,
    },
    Paused {
        failed_step: LoginStep,
        last_observation: String,
        failed_at: i64,
    },
    EmergencyStopped {
        account_id: String,
        stopped_at: i64,
    },
    NeedsManualLogin {
        account_id: String,
        failed_step: LoginStep,
        failure_message: String,
        failed_at: i64,
    },
}

pub(crate) async fn run_login_flow<D, F>(
    driver: &D,
    config: &LoginRunConfig,
    cancelled: Arc<AtomicBool>,
    mut on_step: F,
) -> LoginFlowResult
where
    D: LoginDriver + ?Sized,
    F: FnMut(LoginStep),
{
    if let Err(result) = run_step(
        driver,
        LoginStep::InitialCountdown,
        &config.account_id,
        &cancelled,
        &mut on_step,
        driver.initial_countdown(cancelled.clone()),
    )
    .await
    {
        return result;
    }

    if let Err(result) = terminate_until_stopped(
        driver,
        LoginStep::StopGame,
        &config.game_executable_path,
        &config.account_id,
        &cancelled,
        &mut on_step,
    )
    .await
    {
        return result;
    }
    if let Err(result) = stop_wegame_then_start(
        driver,
        &config,
        &cancelled,
        &mut on_step,
    )
    .await
    {
        return result;
    }

    let login_choice = match run_step(
        driver,
        LoginStep::WaitLoginChoice,
        &config.account_id,
        &cancelled,
        &mut on_step,
        async {
            let key = driver
                .wait_for_any(
                    &[
                        "wegame.loginFormReady",
                        "wegame.loginMode",
                        "wegame.gameEntry",
                    ],
                    cancelled.clone(),
                )
                .await?;
            match key.as_str() {
                "wegame.loginFormReady" | "wegame.loginMode" => Ok(key),
                "wegame.gameEntry" => Err("WeGame 已进入游戏入口，未出现登录表单".to_string()),
                _ => Err("登录入口识别结果无效".to_string()),
            }
        },
    )
    .await
    {
        Ok(login_choice) => login_choice,
        Err(result) => return result,
    };

    if login_choice == "wegame.loginMode" {
        if let Err(result) = run_step(
            driver,
            LoginStep::OpenLoginForm,
            &config.account_id,
            &cancelled,
            &mut on_step,
            driver.click("wegame.loginMode", cancelled.clone()),
        )
        .await
        {
            return result;
        }
    }

    if let Err(result) = run_step(
        driver,
        LoginStep::OpenAccountList,
        &config.account_id,
        &cancelled,
        &mut on_step,
        async {
            driver
                .wait_for_any(&["wegame.loginFormReady"], cancelled.clone())
                .await?;
            driver
                .click("wegame.accountDropdown", cancelled.clone())
                .await
        },
    )
    .await
    {
        return result;
    }

    match run_account_selection(driver, config, &cancelled, &mut on_step).await {
        Ok(()) => {}
        Err(SelectionAttemptFailure::Flow(result)) => return result,
        Err(SelectionAttemptFailure::Manual {
            failure_message,
            failed_step,
        }) => {
            return LoginFlowResult::NeedsManualLogin {
                account_id: config.account_id.clone(),
                failed_step,
                failure_message,
                failed_at: now_ms(),
            }
        }
    }

    if let Err(result) = run_step(
        driver,
        LoginStep::SubmitLogin,
        &config.account_id,
        &cancelled,
        &mut on_step,
        driver.click("wegame.login", cancelled.clone()),
    )
    .await
    {
        return result;
    }

    if let Err(result) = run_step(
        driver,
        LoginStep::WaitGameEntry,
        &config.account_id,
        &cancelled,
        &mut on_step,
        driver.wait_for_any(&["wegame.gameEntry"], cancelled.clone()),
    )
    .await
    {
        return result;
    }

    if let Err(result) = run_step(
        driver,
        LoginStep::OpenGameEntry,
        &config.account_id,
        &cancelled,
        &mut on_step,
        driver.click("wegame.gameEntry", cancelled.clone()),
    )
    .await
    {
        return result;
    }

    if let Err(result) = run_step(
        driver,
        LoginStep::WaitLaunchButton,
        &config.account_id,
        &cancelled,
        &mut on_step,
        driver.wait_for_any(&["wegame.launch"], cancelled.clone()),
    )
    .await
    {
        return result;
    }

    if let Err(result) = run_step(
        driver,
        LoginStep::LaunchGame,
        &config.account_id,
        &cancelled,
        &mut on_step,
        driver.click("wegame.launch", cancelled.clone()),
    )
    .await
    {
        return result;
    }

    let window = match run_step(
        driver,
        LoginStep::WaitGameWindow,
        &config.account_id,
        &cancelled,
        &mut on_step,
        async {
            loop {
                if let Some(window) = driver
                    .find_process_window(&config.game_executable_path)
                    .await?
                {
                    return Ok(window);
                }
                tokio::time::sleep(WINDOW_POLL_INTERVAL).await;
            }
        },
    )
    .await
    {
        Ok(window) => window,
        Err(result) => return result,
    };

    LoginFlowResult::GameReady {
        account_id: config.account_id.clone(),
        qq_account: config.qq_account.clone(),
        game_process_id: window.process_id,
        game_window_handle: window.handle,
    }
}

enum SelectionAttemptFailure {
    Flow(LoginFlowResult),
    Manual {
        failed_step: LoginStep,
        failure_message: String,
    },
}

async fn run_account_selection<D, F>(
    driver: &D,
    config: &LoginRunConfig,
    cancelled: &Arc<AtomicBool>,
    on_step: &mut F,
) -> Result<(), SelectionAttemptFailure>
where
    D: LoginDriver + ?Sized,
    F: FnMut(LoginStep),
{
    driver.reset_observation();
    let current_step = std::sync::Mutex::new(LoginStep::ScanRememberedAccounts);
    on_step(LoginStep::ScanRememberedAccounts);
    let selection =
        select_remembered_account(driver, &config.qq_account, Arc::clone(cancelled), |phase| {
            let step = match phase {
                AccountSelectionPhase::Select => LoginStep::SelectRememberedAccount,
                AccountSelectionPhase::Verify => LoginStep::VerifySelectedAccount,
            };
            *current_step
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = step;
            on_step(step);
        });
    tokio::pin!(selection);
    let timeout = tokio::time::sleep(STEP_TIMEOUT);
    tokio::pin!(timeout);
    tokio::select! {
        biased;
        _ = wait_for_cancellation(cancelled) => {
            Err(SelectionAttemptFailure::Flow(emergency_stopped(&config.account_id)))
        }
        result = &mut selection => match result {
            Ok(()) => Ok(()),
            Err(AccountSelectionError::NotFound { attempts }) => {
                Err(SelectionAttemptFailure::Manual {
                    failed_step: LoginStep::ScanRememberedAccounts,
                    failure_message: format_scan_attempts(&attempts),
                })
            }
            Err(AccountSelectionError::ListUnavailable) => Err(SelectionAttemptFailure::Manual {
                failed_step: LoginStep::ScanRememberedAccounts,
                failure_message: "已记住账号列表未确认".to_string(),
            }),
            Err(AccountSelectionError::Driver(error)) => {
                let failed_step = *current_step
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if failed_step == LoginStep::VerifySelectedAccount {
                    if let Some(failure_message) =
                        copied_account_failure_message(&error)
                    {
                        return Err(SelectionAttemptFailure::Manual {
                            failed_step,
                            failure_message,
                        });
                    }
                }
                Err(SelectionAttemptFailure::Flow(paused(
                    driver,
                    &failed_step,
                    "账号选择步骤执行失败",
                )))
            }
        },
        _ = &mut timeout => {
            if cancelled.load(Ordering::SeqCst) {
                Err(SelectionAttemptFailure::Flow(emergency_stopped(&config.account_id)))
            } else {
                Err(SelectionAttemptFailure::Flow(paused(
                    driver,
                    &current_step
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()),
                    "账号选择步骤超时",
                )))
            }
        }
    }
}

/// 关进程两轮后仍在则等 1 分钟再两轮，循环到成功或紧急停止。不是账号问题。
async fn terminate_until_stopped<D, F>(
    driver: &D,
    step: LoginStep,
    executable: &Path,
    account_id: &str,
    cancelled: &Arc<AtomicBool>,
    on_step: &mut F,
) -> Result<(), LoginFlowResult>
where
    D: LoginDriver + ?Sized,
    F: FnMut(LoginStep),
{
    driver.reset_observation();
    on_step(step);
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err(emergency_stopped(account_id));
        }
        match driver.terminate_exact(executable).await {
            Ok(()) => return Ok(()),
            Err(_) if cancelled.load(Ordering::SeqCst) => {
                return Err(emergency_stopped(account_id));
            }
            Err(_) => {
                wait_interruptible(cancelled, account_id, TERMINATE_RETRY_WAIT).await?
            }
        }
    }
}

async fn stop_wegame_then_start<D, F>(
    driver: &D,
    config: &LoginRunConfig,
    cancelled: &Arc<AtomicBool>,
    on_step: &mut F,
) -> Result<(), LoginFlowResult>
where
    D: LoginDriver + ?Sized,
    F: FnMut(LoginStep),
{
    driver.reset_observation();
    on_step(LoginStep::StopWeGame);
    if cancelled.load(Ordering::SeqCst) {
        return Err(emergency_stopped(&config.account_id));
    }
    match driver
        .terminate_exact(&config.wegame_executable_path)
        .await
    {
        Ok(()) => run_step(
            driver,
            LoginStep::StartWeGame,
            &config.account_id,
            cancelled,
            on_step,
            driver.launch(&config.wegame_executable_path),
        )
        .await
        .map(|_| ()),
        Err(_) => {
            let _ = driver
                .restore_window(&config.wegame_executable_path)
                .await;
            Ok(())
        }
    }
}

async fn wait_interruptible(
    cancelled: &AtomicBool,
    account_id: &str,
    wait: Duration,
) -> Result<(), LoginFlowResult> {
    if wait.is_zero() {
        if cancelled.load(Ordering::SeqCst) {
            return Err(emergency_stopped(account_id));
        }
        return Ok(());
    }
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err(emergency_stopped(account_id));
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(());
        }
        tokio::time::sleep((deadline - now).min(CANCEL_POLL_INTERVAL)).await;
    }
}

fn copied_account_failure_message(error: &str) -> Option<String> {
    if error == "剪贴板未出现 QQ 文本" {
        return Some("账号复核未读取到 QQ".to_string());
    }
    error
        .strip_prefix("剪贴板内容不是纯数字 QQ: ")
        .map(|actual| format!("账号复核不匹配，实际复制 QQ: {}", redact_qq(actual.trim())))
}

async fn run_step<D, T, F, Fut>(
    driver: &D,
    step: LoginStep,
    account_id: &str,
    cancelled: &Arc<AtomicBool>,
    on_step: &mut F,
    future: Fut,
) -> Result<T, LoginFlowResult>
where
    D: LoginDriver + ?Sized,
    F: FnMut(LoginStep),
    Fut: Future<Output = Result<T, String>>,
{
    driver.reset_observation();
    on_step(step);
    let timeout = tokio::time::sleep(STEP_TIMEOUT);
    tokio::pin!(timeout);
    tokio::select! {
        biased;
        _ = wait_for_cancellation(cancelled) => Err(emergency_stopped(account_id)),
        result = future => {
            if cancelled.load(Ordering::SeqCst) {
                Err(emergency_stopped(account_id))
            } else {
                result.map_err(|_| paused(driver, &step, "步骤执行失败"))
            }
        },
        _ = &mut timeout => {
            if cancelled.load(Ordering::SeqCst) {
                Err(emergency_stopped(account_id))
            } else {
                Err(paused(driver, &step, "步骤超时"))
            }
        },
    }
}

fn paused(driver: &(impl LoginDriver + ?Sized), step: &LoginStep, kind: &str) -> LoginFlowResult {
    LoginFlowResult::Paused {
        failed_step: *step,
        last_observation: format_observation(kind, driver.last_observation()),
        failed_at: now_ms(),
    }
}

fn format_observation(kind: &str, observation: LoginObservation) -> String {
    match observation {
        LoginObservation::None => kind.to_string(),
        LoginObservation::LaunchFailed {
            windows_error_code: Some(code),
        } => format!("启动程序失败（Windows 错误 {code}）"),
        LoginObservation::LaunchFailed {
            windows_error_code: None,
        } => "启动程序失败".to_string(),
        LoginObservation::ProcessFailed {
            windows_error_code: Some(code),
        } => format!("结束进程失败（Windows 错误 {code}）"),
        LoginObservation::ProcessFailed {
            windows_error_code: None,
        } => kind.to_string(),
        LoginObservation::TemplateSamples { samples } => format!(
            "{kind}；最后识别结果：双采样相似度 {:.2}% / {:.2}%",
            samples[0] * 100.0,
            samples[1] * 100.0
        ),
        LoginObservation::CaptureFailed => format!("{kind}；最后识别结果：截图失败"),
        LoginObservation::ReferenceImageFailed => {
            format!("{kind}；最后识别结果：参考图读取失败")
        }
        LoginObservation::WindowNotFound => format!("{kind}；最后识别结果：未找到游戏窗口"),
        LoginObservation::WindowOperationFailed => {
            format!("{kind}；最后识别结果：WeGame 窗口恢复或聚焦失败")
        }
    }
}

fn emergency_stopped(account_id: &str) -> LoginFlowResult {
    LoginFlowResult::EmergencyStopped {
        account_id: account_id.to_string(),
        stopped_at: now_ms(),
    }
}

async fn wait_for_cancellation(cancelled: &AtomicBool) {
    while !cancelled.load(Ordering::SeqCst) {
        tokio::time::sleep(CANCEL_POLL_INTERVAL).await;
    }
}

fn now_ms() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    millis.min(i64::MAX as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_ops::desktop_runtime::WindowIdentity;
    use std::{
        collections::VecDeque,
        path::Path,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };
    use tokio::sync::Notify;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Action {
        InitialCountdown,
        Terminate(&'static str),
        RestoreWindow(&'static str),
        StartWeGame,
        Wait(Vec<String>),
        Click(String),
        FindWindow,
    }

    struct FakeDriver {
        actions: Mutex<Vec<Action>>,
        wait_results: Mutex<VecDeque<Result<String, String>>>,
        wait_observations: Mutex<VecDeque<LoginObservation>>,
        last_observation: Mutex<LoginObservation>,
        windows: Mutex<VecDeque<Result<Option<WindowIdentity>, String>>>,
        delays: Mutex<VecDeque<Duration>>,
        wait_calls: AtomicUsize,
        find_calls: AtomicUsize,
        fail_wait_call: AtomicUsize,
        copied_accounts: Mutex<VecDeque<Result<String, String>>>,
        select_row_error: Mutex<Option<String>>,
        block_wait_call: AtomicUsize,
        cancel_error_wait_call: AtomicUsize,
        block_click_target: Mutex<Option<String>>,
        blocked: Notify,
        terminate_results: Mutex<VecDeque<Result<(), String>>>,
    }

    impl Default for FakeDriver {
        fn default() -> Self {
            Self {
                actions: Mutex::new(Vec::new()),
                wait_results: Mutex::new(VecDeque::new()),
                wait_observations: Mutex::new(VecDeque::new()),
                last_observation: Mutex::new(LoginObservation::None),
                windows: Mutex::new(VecDeque::from([Ok(Some(WindowIdentity {
                    process_id: 42,
                    handle: 84,
                }))])),
                delays: Mutex::new(VecDeque::new()),
                wait_calls: AtomicUsize::new(0),
                find_calls: AtomicUsize::new(0),
                fail_wait_call: AtomicUsize::new(0),
                copied_accounts: Mutex::new(VecDeque::new()),
                select_row_error: Mutex::new(None),
                block_wait_call: AtomicUsize::new(0),
                cancel_error_wait_call: AtomicUsize::new(0),
                block_click_target: Mutex::new(None),
                blocked: Notify::new(),
                terminate_results: Mutex::new(VecDeque::new()),
            }
        }
    }

    impl FakeDriver {
        fn with_waits(waits: impl IntoIterator<Item = &'static str>) -> Self {
            let driver = Self::default();
            driver.wait_results.lock().unwrap().extend(
                waits
                    .into_iter()
                    .map(|key| Ok::<_, String>(key.to_string())),
            );
            driver
        }

        fn actions(&self) -> Vec<Action> {
            self.actions.lock().unwrap().clone()
        }

        async fn delay(&self) {
            let delay = self.delays.lock().unwrap().pop_front();
            if let Some(delay) = delay {
                tokio::time::sleep(delay).await;
            }
        }
    }

    impl LoginDriver for FakeDriver {
        fn reset_observation(&self) {
            *self.last_observation.lock().unwrap() = LoginObservation::None;
        }

        fn last_observation(&self) -> LoginObservation {
            *self.last_observation.lock().unwrap()
        }

        async fn initial_countdown(&self, _: Arc<AtomicBool>) -> Result<(), String> {
            self.actions.lock().unwrap().push(Action::InitialCountdown);
            Ok(())
        }

        async fn terminate_exact(&self, executable: &Path) -> Result<(), String> {
            let executable = if executable == Path::new("game.exe") {
                "game"
            } else {
                "wegame"
            };
            self.actions
                .lock()
                .unwrap()
                .push(Action::Terminate(executable));
            self.delay().await;
            self.terminate_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(()))
        }

        async fn launch(&self, _: &Path) -> Result<u32, String> {
            self.actions.lock().unwrap().push(Action::StartWeGame);
            self.delay().await;
            Ok(7)
        }

        async fn wait_for_any(
            &self,
            target_keys: &[&str],
            cancelled: Arc<AtomicBool>,
        ) -> Result<String, String> {
            *self.last_observation.lock().unwrap() = self
                .wait_observations
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(LoginObservation::None);
            self.actions.lock().unwrap().push(Action::Wait(
                target_keys.iter().map(|key| (*key).to_string()).collect(),
            ));
            let call = self.wait_calls.fetch_add(1, Ordering::SeqCst) + 1;
            if call == self.block_wait_call.load(Ordering::SeqCst) {
                self.blocked.notify_one();
                std::future::pending::<()>().await;
            }
            if call == self.cancel_error_wait_call.load(Ordering::SeqCst) {
                self.blocked.notify_one();
                while !cancelled.load(Ordering::SeqCst) {
                    tokio::task::yield_now().await;
                }
                return Err("驱动报告动作取消".to_string());
            }
            self.delay().await;
            if call == self.fail_wait_call.load(Ordering::SeqCst) {
                return Err("驱动内部等待错误".to_string());
            }
            self.wait_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(target_keys[0].to_string()))
        }

        async fn click(&self, target_key: &str, _: Arc<AtomicBool>) -> Result<(), String> {
            self.actions
                .lock()
                .unwrap()
                .push(Action::Click(target_key.to_string()));
            let blocked = self
                .block_click_target
                .lock()
                .unwrap()
                .as_deref()
                .is_some_and(|blocked| blocked == target_key);
            if blocked {
                self.blocked.notify_one();
                std::future::pending::<()>().await;
            }
            self.delay().await;
            Ok(())
        }

        async fn find_process_window(&self, _: &Path) -> Result<Option<WindowIdentity>, String> {
            self.actions.lock().unwrap().push(Action::FindWindow);
            self.find_calls.fetch_add(1, Ordering::SeqCst);
            self.delay().await;
            let result = self.windows.lock().unwrap().pop_front().unwrap_or(Ok(None));
            if matches!(result, Ok(None)) {
                *self.last_observation.lock().unwrap() = LoginObservation::WindowNotFound;
            }
            result
        }

        async fn restore_window(&self, executable: &Path) -> Result<(), String> {
            let label = if executable == Path::new("game.exe") {
                "game"
            } else {
                "wegame"
            };
            self.actions
                .lock()
                .unwrap()
                .push(Action::RestoreWindow(label));
            Ok(())
        }
    }

    impl RememberedAccountDriver for FakeDriver {
        async fn visible_account_rows(
            &self,
            _: Arc<AtomicBool>,
        ) -> Result<Vec<super::super::remembered_account::AccountRowSlot>, String> {
            Ok(vec![
                super::super::remembered_account::AccountRowSlot::Fallback { index: 0 },
                super::super::remembered_account::AccountRowSlot::Fallback { index: 1 },
                super::super::remembered_account::AccountRowSlot::Fallback { index: 2 },
            ])
        }

        async fn open_account_list(&self, _: Arc<AtomicBool>) -> Result<(), String> {
            self.actions
                .lock()
                .unwrap()
                .push(Action::Click("open-account-list".to_string()));
            Ok(())
        }

        async fn scroll_down(&self, _: Arc<AtomicBool>) -> Result<(), String> {
            self.actions
                .lock()
                .unwrap()
                .push(Action::Click("scroll".to_string()));
            Ok(())
        }

        async fn select_row(
            &self,
            slot: super::super::remembered_account::AccountRowSlot,
            _: Arc<AtomicBool>,
        ) -> Result<super::super::remembered_account::AccountRowClick, String> {
            if let Some(error) = self.select_row_error.lock().unwrap().take() {
                return Err(error);
            }
            self.actions
                .lock()
                .unwrap()
                .push(Action::Click("ocr-account".to_string()));
            Ok(super::super::remembered_account::AccountRowClick {
                index: slot.index(),
                x: 0,
                y: 0,
            })
        }

        async fn copy_selected_qq(&self, _: Arc<AtomicBool>) -> Result<String, String> {
            self.actions
                .lock()
                .unwrap()
                .push(Action::Click("copy-account".to_string()));
            self.copied_accounts
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok("123456789".to_string()))
        }
    }

    fn config() -> LoginRunConfig {
        LoginRunConfig {
            account_id: "account-id".to_string(),
            qq_account: "123456789".to_string(),
            wegame_executable_path: "wegame.exe".into(),
            game_executable_path: "game.exe".into(),
            mouse_parking_region: crate::morse::types::RegionRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            targets: Default::default(),
        }
    }

    fn ready_waits() -> [&'static str; 4] {
        [
            "wegame.loginFormReady",
            "wegame.loginFormReady",
            "wegame.gameEntry",
            "wegame.launch",
        ]
    }

    async fn run(driver: &FakeDriver) -> LoginFlowResult {
        run_login_flow(driver, &config(), Arc::new(AtomicBool::new(false)), |_| {}).await
    }

    #[tokio::test(start_paused = true)]
    async fn stop_game_retries_after_one_minute_then_continues() {
        let driver = FakeDriver::with_waits(ready_waits());
        driver.terminate_results.lock().unwrap().extend([
            Err("目标进程仍在运行".to_string()),
            Ok(()),
        ]);

        let run = run(&driver);
        tokio::pin!(run);
        tokio::select! {
            biased;
            _ = &mut run => panic!("关进程失败后应等待再试"),
            _ = tokio::task::yield_now() => {}
        }
        tokio::time::advance(TERMINATE_RETRY_WAIT).await;
        let result = run.await;

        assert!(matches!(result, LoginFlowResult::GameReady { .. }));
        assert_eq!(
            driver
                .actions()
                .into_iter()
                .filter(|action| matches!(action, Action::Terminate(_)))
                .collect::<Vec<_>>(),
            vec![
                Action::Terminate("game"),
                Action::Terminate("game"),
                Action::Terminate("wegame"),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn stop_game_cancel_during_retry_wait_does_not_start_wegame() {
        let driver = FakeDriver::with_waits(ready_waits());
        driver
            .terminate_results
            .lock()
            .unwrap()
            .push_back(Err("目标进程仍在运行".to_string()));
        let cancelled = Arc::new(AtomicBool::new(false));
        let config = config();
        let run = run_login_flow(&driver, &config, Arc::clone(&cancelled), |_| {});
        tokio::pin!(run);
        tokio::select! {
            biased;
            _ = &mut run => panic!("关进程失败后应等待再试"),
            _ = tokio::task::yield_now() => {}
        }
        cancelled.store(true, Ordering::SeqCst);
        tokio::time::advance(CANCEL_POLL_INTERVAL).await;
        let result = run.await;

        assert!(matches!(result, LoginFlowResult::EmergencyStopped { .. }));
        assert!(!driver.actions().contains(&Action::StartWeGame));
    }

    #[tokio::test]
    async fn wegame_still_running_skips_launch_and_continues_login() {
        let driver = FakeDriver::with_waits(ready_waits());
        driver.terminate_results.lock().unwrap().extend([
            Ok(()),
            Err("目标进程仍在运行".to_string()),
        ]);

        let result = run(&driver).await;

        assert!(matches!(result, LoginFlowResult::GameReady { .. }));
        assert!(!driver.actions().contains(&Action::StartWeGame));
        assert!(driver
            .actions()
            .contains(&Action::RestoreWindow("wegame")));
    }

    #[tokio::test(start_paused = true)]
    async fn wait_interruptible_holds_thirty_seconds() {
        let cancelled = AtomicBool::new(false);
        let wait = wait_interruptible(&cancelled, "account-id", Duration::from_secs(30));
        tokio::pin!(wait);
        tokio::select! {
            biased;
            _ = &mut wait => panic!("30 秒等待提前结束"),
            _ = tokio::task::yield_now() => {}
        }
        tokio::time::advance(Duration::from_secs(29)).await;
        tokio::select! {
            biased;
            _ = &mut wait => panic!("29 秒不应结束"),
            _ = tokio::task::yield_now() => {}
        }
        tokio::time::advance(Duration::from_secs(1)).await;
        wait.await.unwrap();
    }

    #[tokio::test]
    async fn initial_countdown_finishes_before_old_processes_are_terminated() {
        let driver = FakeDriver::with_waits(ready_waits());

        assert!(matches!(
            run(&driver).await,
            LoginFlowResult::GameReady { .. }
        ));
        assert_eq!(
            driver.actions()[..3],
            [
                Action::InitialCountdown,
                Action::Terminate("game"),
                Action::Terminate("wegame"),
            ]
        );
    }

    #[tokio::test]
    async fn auto_login_game_entry_fails_wait_login_choice_instead_of_waiting_for_form() {
        let driver = FakeDriver::with_waits(["wegame.gameEntry"]);

        let result = run(&driver).await;

        assert!(matches!(
            result,
            LoginFlowResult::Paused {
                failed_step: LoginStep::WaitLoginChoice,
                ..
            }
        ));
        assert!(driver.actions().iter().any(|action| {
            matches!(
                action,
                Action::Wait(keys) if keys.iter().any(|key| key == "wegame.gameEntry")
            )
        }));
        assert!(!driver
            .actions()
            .contains(&Action::Click("wegame.login".to_string())));
    }

    #[tokio::test]
    async fn unavailable_remembered_account_list_marks_account_needs_manual_login() {
        let driver = FakeDriver::default();
        *driver.select_row_error.lock().unwrap() = Some("已记住账号列表未确认".to_string());

        assert!(matches!(
            run(&driver).await,
            LoginFlowResult::NeedsManualLogin {
                failed_step: LoginStep::ScanRememberedAccounts,
                failure_message,
                ..
            } if failure_message == "已记住账号列表未确认"
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn login_button_is_submitted_once_and_never_replayed_after_timeout() {
        let driver = FakeDriver::with_waits(["wegame.loginFormReady", "wegame.loginFormReady"]);
        driver.block_wait_call.store(3, Ordering::SeqCst);

        let result = run(&driver).await;

        assert!(matches!(
            result,
            LoginFlowResult::Paused {
                failed_step: LoginStep::WaitGameEntry,
                ..
            }
        ));
        assert_eq!(
            driver
                .actions()
                .iter()
                .filter(|action| action == &&Action::Click("wegame.login".to_string()))
                .count(),
            1
        );
    }

    #[tokio::test(start_paused = true)]
    async fn wait_game_entry_timeout_preserves_latest_template_samples() {
        let driver = FakeDriver::with_waits(["wegame.loginFormReady", "wegame.loginFormReady"]);
        driver.wait_observations.lock().unwrap().extend([
            LoginObservation::CaptureFailed,
            LoginObservation::None,
            LoginObservation::TemplateSamples {
                samples: [0.41, 0.42],
            },
        ]);
        driver.block_wait_call.store(3, Ordering::SeqCst);

        let result = run(&driver).await;

        let LoginFlowResult::Paused {
            failed_step,
            last_observation,
            ..
        } = result
        else {
            panic!("预期流程暂停");
        };
        assert_eq!(failed_step, LoginStep::WaitGameEntry);
        assert_eq!(
            last_observation,
            "步骤超时；最后识别结果：双采样相似度 41.00% / 42.00%"
        );
    }

    #[test]
    fn launch_failure_formats_safe_windows_error_code() {
        assert_eq!(
            format_observation(
                "步骤执行失败",
                LoginObservation::LaunchFailed {
                    windows_error_code: Some(740),
                },
            ),
            "启动程序失败（Windows 错误 740）"
        );
    }

    #[test]
    fn process_failure_formats_safe_windows_error_code() {
        assert_eq!(
            format_observation(
                "步骤执行失败",
                LoginObservation::ProcessFailed {
                    windows_error_code: Some(5),
                },
            ),
            "结束进程失败（Windows 错误 5）"
        );
        assert_eq!(
            format_observation(
                "步骤执行失败",
                LoginObservation::ProcessFailed {
                    windows_error_code: None,
                },
            ),
            "步骤执行失败"
        );
    }

    #[test]
    fn window_operation_failure_has_distinct_safe_message() {
        assert_eq!(
            format_observation("步骤执行失败", LoginObservation::WindowOperationFailed),
            "步骤执行失败；最后识别结果：WeGame 窗口恢复或聚焦失败"
        );
    }

    #[tokio::test]
    async fn step_clears_stale_observation_before_business_future_runs() {
        let driver = FakeDriver::default();
        *driver.last_observation.lock().unwrap() = LoginObservation::CaptureFailed;
        let cancelled = Arc::new(AtomicBool::new(false));

        let result = run_step(
            &driver,
            LoginStep::StopGame,
            "account-id",
            &cancelled,
            &mut |_| {},
            async { Err::<(), _>("RAW_DRIVER_SECRET".to_string()) },
        )
        .await;

        assert!(matches!(
            result,
            Err(LoginFlowResult::Paused {
                last_observation,
                ..
            }) if last_observation == "步骤执行失败"
        ));
    }

    #[tokio::test]
    async fn ready_form_skips_login_mode_click() {
        let driver = FakeDriver::with_waits(ready_waits());

        let result = run(&driver).await;

        assert!(matches!(result, LoginFlowResult::GameReady { .. }));
        assert!(!driver
            .actions()
            .contains(&Action::Click("wegame.loginMode".to_string())));
    }

    #[tokio::test]
    async fn action_order_rebuilds_session_before_remembered_account_selection() {
        let driver = FakeDriver::with_waits([
            "wegame.loginMode",
            "wegame.loginFormReady",
            "wegame.gameEntry",
            "wegame.launch",
        ]);
        let result = run(&driver).await;

        assert!(matches!(result, LoginFlowResult::GameReady { .. }));
        assert_eq!(
            driver.actions(),
            vec![
                Action::InitialCountdown,
                Action::Terminate("game"),
                Action::Terminate("wegame"),
                Action::StartWeGame,
                Action::Wait(vec![
                    "wegame.loginFormReady".to_string(),
                    "wegame.loginMode".to_string(),
                    "wegame.gameEntry".to_string(),
                ]),
                Action::Click("wegame.loginMode".to_string()),
                Action::Wait(vec!["wegame.loginFormReady".to_string()]),
                Action::Click("wegame.accountDropdown".to_string()),
                Action::Click("ocr-account".to_string()),
                Action::Click("copy-account".to_string()),
                Action::Click("wegame.login".to_string()),
                Action::Wait(vec!["wegame.gameEntry".to_string()]),
                Action::Click("wegame.gameEntry".to_string()),
                Action::Wait(vec!["wegame.launch".to_string()]),
                Action::Click("wegame.launch".to_string()),
                Action::FindWindow,
            ]
        );
    }

    #[tokio::test]
    async fn remembered_account_selection_revalidates_form_once_and_stores_no_credential_value() {
        let driver = FakeDriver::with_waits(ready_waits());

        let result = run(&driver).await;

        assert!(matches!(result, LoginFlowResult::GameReady { .. }));
        let actions = driver.actions();
        assert_eq!(
            actions
                .iter()
                .filter(|action| {
                    action == &&Action::Wait(vec!["wegame.loginFormReady".to_string()])
                })
                .count(),
            1
        );
        let snapshot = format!("{actions:?}");
        assert!(!snapshot.contains("123456789"));
        assert!(!snapshot.contains("secret-password"));
    }

    #[tokio::test(start_paused = true)]
    async fn each_step_gets_an_independent_timeout_budget() {
        let driver = FakeDriver::with_waits(ready_waits());
        driver
            .delays
            .lock()
            .unwrap()
            .extend(std::iter::repeat_n(Duration::from_secs(20), 14));
        let started = tokio::time::Instant::now();

        let result = run(&driver).await;

        assert!(matches!(result, LoginFlowResult::GameReady { .. }));
        assert!(tokio::time::Instant::now().duration_since(started) > STEP_TIMEOUT);
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_during_wait_returns_emergency_stop_without_next_action() {
        let driver = FakeDriver::default();
        driver.block_wait_call.store(1, Ordering::SeqCst);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        let config = config();
        let flow = run_login_flow(&driver, &config, cancelled, |_| {});
        let cancel = async {
            driver.blocked.notified().await;
            stop.store(true, Ordering::SeqCst);
        };

        let (result, ()) = tokio::join!(flow, cancel);

        assert!(matches!(result, LoginFlowResult::EmergencyStopped { .. }));
        assert_eq!(driver.actions().len(), 5);
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_wins_when_driver_reports_cancellation_error_first() {
        let driver = FakeDriver::default();
        driver.cancel_error_wait_call.store(1, Ordering::SeqCst);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        let config = config();
        let flow = run_login_flow(&driver, &config, cancelled, |_| {});
        let cancel = async {
            driver.blocked.notified().await;
            stop.store(true, Ordering::SeqCst);
        };

        let (result, ()) = tokio::join!(flow, cancel);

        assert!(matches!(result, LoginFlowResult::EmergencyStopped { .. }));
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_during_action_returns_emergency_stop_without_credentials() {
        let driver = FakeDriver::with_waits(["wegame.loginMode"]);
        *driver.block_click_target.lock().unwrap() = Some("wegame.loginMode".to_string());
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        let config = config();
        let flow = run_login_flow(&driver, &config, cancelled, |_| {});
        let cancel = async {
            driver.blocked.notified().await;
            stop.store(true, Ordering::SeqCst);
        };

        let (result, ()) = tokio::join!(flow, cancel);

        assert!(matches!(result, LoginFlowResult::EmergencyStopped { .. }));
        assert!(!driver
            .actions()
            .contains(&Action::Click("wegame.login".to_string())));
    }

    #[tokio::test]
    async fn non_target_rows_continue_scanning_without_restarting_wegame() {
        let driver = FakeDriver::with_waits([
            "wegame.loginFormReady",
            "wegame.loginFormReady",
            "wegame.loginFormReady",
            "wegame.loginFormReady",
        ]);
        driver
            .copied_accounts
            .lock()
            .unwrap()
            .extend([Ok("111111".to_string()), Ok("222222".to_string())]);

        let result = run(&driver).await;

        assert!(matches!(result, LoginFlowResult::GameReady { .. }));
        assert_eq!(
            driver
                .actions()
                .iter()
                .filter(|action| action == &&Action::StartWeGame)
                .count(),
            1
        );
        assert_eq!(
            driver
                .actions()
                .iter()
                .filter(|action| action == &&Action::Click("ocr-account".to_string()))
                .count(),
            3
        );
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("secret-password"));
        assert!(!json.contains("game.exe"));
        assert!(!json.contains("RAW_DRIVER_SECRET"));
    }

    #[tokio::test]
    async fn account_scan_failure_does_not_restart_wegame() {
        let driver = FakeDriver::with_waits([
            "wegame.loginFormReady",
            "wegame.loginFormReady",
            "wegame.loginFormReady",
            "wegame.loginFormReady",
        ]);
        driver.copied_accounts.lock().unwrap().extend([
            Err("剪贴板未出现 QQ 文本".to_string()),
            Err("剪贴板未出现 QQ 文本".to_string()),
        ]);

        let result = run(&driver).await;

        assert!(matches!(
            result,
            LoginFlowResult::NeedsManualLogin {
                failed_step: LoginStep::VerifySelectedAccount,
                failure_message,
                ..
            } if failure_message == "账号复核未读取到 QQ"
        ));
        assert_eq!(
            driver
                .actions()
                .iter()
                .filter(|action| action == &&Action::StartWeGame)
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn not_found_scan_trace_reaches_manual_login_without_full_qq() {
        let driver = FakeDriver::with_waits(["wegame.loginFormReady", "wegame.loginFormReady"]);
        driver.copied_accounts.lock().unwrap().extend([
            Ok("11112222".to_string()),
            Ok("33334444".to_string()),
            Ok("55556666".to_string()),
            Ok("11112222".to_string()),
            Ok("33334444".to_string()),
            Ok("55556666".to_string()),
        ]);

        let LoginFlowResult::NeedsManualLogin {
            failure_message, ..
        } = run(&driver).await
        else {
            panic!("扫描到底后应转为需人工登录");
        };

        assert!(failure_message.contains("页 1 槽位 0"));
        assert!(failure_message.contains("***2222"));
        assert!(failure_message.contains("页 2 槽位 2"));
        assert!(!failure_message.contains("11112222"));
        assert!(!failure_message.contains("3079643589"));
    }

    #[tokio::test]
    async fn manual_login_failure_keeps_only_redacted_copied_qq() {
        let driver = FakeDriver::with_waits(["wegame.loginFormReady", "wegame.loginFormReady"]);
        driver
            .copied_accounts
            .lock()
            .unwrap()
            .push_back(Err("剪贴板内容不是纯数字 QQ: 3079643589".to_string()));

        let result = run(&driver).await;
        let LoginFlowResult::NeedsManualLogin {
            failure_message, ..
        } = result
        else {
            panic!("应返回需人工登录结果");
        };

        assert!(failure_message.contains("***3589"));
        assert!(!failure_message.contains("3079643589"));
        let serialized = serde_json::to_string(&failure_message).unwrap();
        assert!(!serialized.contains("3079643589"));
    }

    #[tokio::test(start_paused = true)]
    async fn missing_window_keeps_polling_until_real_identity_exists() {
        let driver = FakeDriver::with_waits(ready_waits());
        *driver.windows.lock().unwrap() = VecDeque::from([
            Ok(None),
            Ok(None),
            Ok(Some(WindowIdentity {
                process_id: 314,
                handle: 159,
            })),
        ]);

        let result = run(&driver).await;

        assert_eq!(driver.find_calls.load(Ordering::SeqCst), 3);
        assert!(matches!(
            result,
            LoginFlowResult::GameReady {
                game_process_id: 314,
                game_window_handle: 159,
                ..
            }
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn game_window_timeout_preserves_window_not_found_observation() {
        let driver = FakeDriver::with_waits(ready_waits());
        driver.windows.lock().unwrap().clear();

        let result = run(&driver).await;

        let LoginFlowResult::Paused {
            failed_step,
            last_observation,
            ..
        } = result
        else {
            panic!("预期流程暂停");
        };
        assert_eq!(failed_step, LoginStep::WaitGameWindow);
        assert_eq!(last_observation, "步骤超时；最后识别结果：未找到游戏窗口");
    }

    #[tokio::test]
    async fn callback_reports_exact_step_order() {
        let driver = FakeDriver::with_waits([
            "wegame.loginMode",
            "wegame.loginFormReady",
            "wegame.gameEntry",
            "wegame.launch",
        ]);
        let mut steps = Vec::new();
        let result = run_login_flow(
            &driver,
            &config(),
            Arc::new(AtomicBool::new(false)),
            |step| steps.push(step),
        )
        .await;

        assert!(matches!(result, LoginFlowResult::GameReady { .. }));
        assert_eq!(
            steps,
            vec![
                LoginStep::InitialCountdown,
                LoginStep::StopGame,
                LoginStep::StopWeGame,
                LoginStep::StartWeGame,
                LoginStep::WaitLoginChoice,
                LoginStep::OpenLoginForm,
                LoginStep::OpenAccountList,
                LoginStep::ScanRememberedAccounts,
                LoginStep::SelectRememberedAccount,
                LoginStep::VerifySelectedAccount,
                LoginStep::SubmitLogin,
                LoginStep::WaitGameEntry,
                LoginStep::OpenGameEntry,
                LoginStep::WaitLaunchButton,
                LoginStep::LaunchGame,
                LoginStep::WaitGameWindow,
            ]
        );
    }

    #[test]
    fn game_ready_serializes_kind_and_fields_as_camel_case_without_password() {
        let value = serde_json::to_value(LoginFlowResult::GameReady {
            account_id: "account-id".to_string(),
            qq_account: "123456789".to_string(),
            game_process_id: 42,
            game_window_handle: 84,
        })
        .unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "kind": "gameReady",
                "accountId": "account-id",
                "qqAccount": "123456789",
                "gameProcessId": 42,
                "gameWindowHandle": 84,
            })
        );
        let json = value.to_string();
        assert!(!json.contains("account_id"));
        assert!(!json.contains("qq_account"));
        assert!(!json.contains("game_process_id"));
        assert!(!json.contains("game_window_handle"));
        assert!(!json.contains("password"));
    }

    #[test]
    fn paused_serializes_kind_and_fields_as_camel_case_without_password() {
        let value = serde_json::to_value(LoginFlowResult::Paused {
            failed_step: LoginStep::WaitGameEntry,
            last_observation: "步骤超时".to_string(),
            failed_at: 123,
        })
        .unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "kind": "paused",
                "failedStep": "waitGameEntry",
                "lastObservation": "步骤超时",
                "failedAt": 123,
            })
        );
        let json = value.to_string();
        assert!(!json.contains("failed_step"));
        assert!(!json.contains("last_observation"));
        assert!(!json.contains("failed_at"));
        assert!(!json.contains("password"));
    }

    #[test]
    fn emergency_stopped_serializes_kind_and_fields_as_camel_case_without_password() {
        let value = serde_json::to_value(LoginFlowResult::EmergencyStopped {
            account_id: "account-id".to_string(),
            stopped_at: 456,
        })
        .unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "kind": "emergencyStopped",
                "accountId": "account-id",
                "stoppedAt": 456,
            })
        );
        let json = value.to_string();
        assert!(!json.contains("account_id"));
        assert!(!json.contains("stopped_at"));
        assert!(!json.contains("password"));
    }
}
