use super::{desktop_runtime::WindowIdentity, template_observer::RuntimeTarget};
use serde::Serialize;
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

pub(crate) struct LoginRunConfig {
    pub account_id: String,
    pub qq_account: String,
    pub password: String,
    pub wegame_executable_path: PathBuf,
    pub game_executable_path: PathBuf,
    pub targets: HashMap<String, RuntimeTarget>,
}

#[allow(async_fn_in_trait)]
pub(crate) trait LoginDriver: Send + Sync {
    async fn terminate_exact(&self, executable: &Path) -> Result<(), String>;
    async fn launch(&self, executable: &Path) -> Result<u32, String>;
    async fn wait_for_any(
        &self,
        target_keys: &[&str],
        cancelled: Arc<AtomicBool>,
    ) -> Result<String, String>;
    async fn click(&self, target_key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn replace_text(
        &self,
        target_key: &str,
        value: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), String>;
    async fn find_process_window(
        &self,
        executable: &Path,
    ) -> Result<Option<WindowIdentity>, String>;
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LoginStep {
    StopGame,
    StopWeGame,
    StartWeGame,
    WaitLoginChoice,
    OpenLoginForm,
    InputAccount,
    InputPassword,
    SubmitLogin,
    WaitGameEntry,
    OpenGameEntry,
    WaitLaunchButton,
    LaunchGame,
    WaitGameWindow,
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
        LoginStep::StopGame,
        &config.account_id,
        &cancelled,
        &mut on_step,
        driver.terminate_exact(&config.game_executable_path),
    )
    .await
    {
        return result;
    }

    if let Err(result) = run_step(
        LoginStep::StopWeGame,
        &config.account_id,
        &cancelled,
        &mut on_step,
        driver.terminate_exact(&config.wegame_executable_path),
    )
    .await
    {
        return result;
    }

    if let Err(result) = run_step(
        LoginStep::StartWeGame,
        &config.account_id,
        &cancelled,
        &mut on_step,
        driver.launch(&config.wegame_executable_path),
    )
    .await
    {
        return result;
    }

    let login_choice = match run_step(
        LoginStep::WaitLoginChoice,
        &config.account_id,
        &cancelled,
        &mut on_step,
        async {
            let key = driver
                .wait_for_any(
                    &["wegame.loginFormReady", "wegame.loginMode"],
                    cancelled.clone(),
                )
                .await?;
            match key.as_str() {
                "wegame.loginFormReady" | "wegame.loginMode" => Ok(key),
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
        LoginStep::InputAccount,
        &config.account_id,
        &cancelled,
        &mut on_step,
        async {
            driver
                .wait_for_any(&["wegame.loginFormReady"], cancelled.clone())
                .await?;
            driver
                .replace_text("wegame.account", &config.qq_account, cancelled.clone())
                .await
        },
    )
    .await
    {
        return result;
    }

    if let Err(result) = run_step(
        LoginStep::InputPassword,
        &config.account_id,
        &cancelled,
        &mut on_step,
        async {
            driver
                .wait_for_any(&["wegame.loginFormReady"], cancelled.clone())
                .await?;
            driver
                .replace_text("wegame.password", &config.password, cancelled.clone())
                .await
        },
    )
    .await
    {
        return result;
    }

    if let Err(result) = run_step(
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

async fn run_step<T, F, Fut>(
    step: LoginStep,
    account_id: &str,
    cancelled: &Arc<AtomicBool>,
    on_step: &mut F,
    future: Fut,
) -> Result<T, LoginFlowResult>
where
    F: FnMut(LoginStep),
    Fut: Future<Output = Result<T, String>>,
{
    on_step(step.clone());
    let timeout = tokio::time::sleep(STEP_TIMEOUT);
    tokio::pin!(timeout);
    tokio::select! {
        biased;
        _ = wait_for_cancellation(cancelled) => Err(emergency_stopped(account_id)),
        result = future => {
            if cancelled.load(Ordering::SeqCst) {
                Err(emergency_stopped(account_id))
            } else {
                result.map_err(|_| LoginFlowResult::Paused {
                    failed_step: step.clone(),
                    last_observation: format!("{step:?}：步骤执行失败"),
                    failed_at: now_ms(),
                })
            }
        },
        _ = &mut timeout => {
            if cancelled.load(Ordering::SeqCst) {
                Err(emergency_stopped(account_id))
            } else {
                Err(LoginFlowResult::Paused {
                    failed_step: step.clone(),
                    last_observation: format!("{step:?}：步骤超时"),
                    failed_at: now_ms(),
                })
            }
        },
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
        Terminate(&'static str),
        StartWeGame,
        Wait(Vec<String>),
        Click(String),
        Replace(String),
        FindWindow,
    }

    struct FakeDriver {
        actions: Mutex<Vec<Action>>,
        wait_results: Mutex<VecDeque<Result<String, String>>>,
        windows: Mutex<VecDeque<Result<Option<WindowIdentity>, String>>>,
        delays: Mutex<VecDeque<Duration>>,
        wait_calls: AtomicUsize,
        find_calls: AtomicUsize,
        fail_wait_call: AtomicUsize,
        fail_replace_target: Mutex<Option<String>>,
        block_wait_call: AtomicUsize,
        cancel_error_wait_call: AtomicUsize,
        block_click_target: Mutex<Option<String>>,
        blocked: Notify,
    }

    impl Default for FakeDriver {
        fn default() -> Self {
            Self {
                actions: Mutex::new(Vec::new()),
                wait_results: Mutex::new(VecDeque::new()),
                windows: Mutex::new(VecDeque::from([Ok(Some(WindowIdentity {
                    process_id: 42,
                    handle: 84,
                }))])),
                delays: Mutex::new(VecDeque::new()),
                wait_calls: AtomicUsize::new(0),
                find_calls: AtomicUsize::new(0),
                fail_wait_call: AtomicUsize::new(0),
                fail_replace_target: Mutex::new(None),
                block_wait_call: AtomicUsize::new(0),
                cancel_error_wait_call: AtomicUsize::new(0),
                block_click_target: Mutex::new(None),
                blocked: Notify::new(),
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
            Ok(())
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

        async fn replace_text(
            &self,
            target_key: &str,
            _: &str,
            _: Arc<AtomicBool>,
        ) -> Result<(), String> {
            self.actions
                .lock()
                .unwrap()
                .push(Action::Replace(target_key.to_string()));
            self.delay().await;
            if self
                .fail_replace_target
                .lock()
                .unwrap()
                .as_deref()
                .is_some_and(|failed| failed == target_key)
            {
                return Err("驱动内部替换错误".to_string());
            }
            Ok(())
        }

        async fn find_process_window(&self, _: &Path) -> Result<Option<WindowIdentity>, String> {
            self.actions.lock().unwrap().push(Action::FindWindow);
            self.find_calls.fetch_add(1, Ordering::SeqCst);
            self.delay().await;
            self.windows.lock().unwrap().pop_front().unwrap_or(Ok(None))
        }
    }

    fn config() -> LoginRunConfig {
        LoginRunConfig {
            account_id: "account-id".to_string(),
            qq_account: "123456789".to_string(),
            password: "secret-password".to_string(),
            wegame_executable_path: "wegame.exe".into(),
            game_executable_path: "game.exe".into(),
            targets: Default::default(),
        }
    }

    fn ready_waits() -> [&'static str; 5] {
        [
            "wegame.loginFormReady",
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
    async fn login_button_is_submitted_once_and_never_replayed_after_timeout() {
        let driver = FakeDriver::with_waits([
            "wegame.loginFormReady",
            "wegame.loginFormReady",
            "wegame.loginFormReady",
        ]);
        driver.block_wait_call.store(4, Ordering::SeqCst);

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
        assert_eq!(
            driver
                .actions()
                .iter()
                .filter(|action| matches!(action, Action::Replace(_)))
                .count(),
            2
        );
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
    async fn action_order_rebuilds_session_before_credentials() {
        let driver = FakeDriver::with_waits([
            "wegame.loginMode",
            "wegame.loginFormReady",
            "wegame.loginFormReady",
            "wegame.gameEntry",
            "wegame.launch",
        ]);

        let result = run(&driver).await;

        assert!(matches!(result, LoginFlowResult::GameReady { .. }));
        assert_eq!(
            driver.actions(),
            vec![
                Action::Terminate("game"),
                Action::Terminate("wegame"),
                Action::StartWeGame,
                Action::Wait(vec![
                    "wegame.loginFormReady".to_string(),
                    "wegame.loginMode".to_string(),
                ]),
                Action::Click("wegame.loginMode".to_string()),
                Action::Wait(vec!["wegame.loginFormReady".to_string()]),
                Action::Replace("wegame.account".to_string()),
                Action::Wait(vec!["wegame.loginFormReady".to_string()]),
                Action::Replace("wegame.password".to_string()),
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
    async fn credentials_each_revalidate_ready_form_and_actions_do_not_store_values() {
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
            2
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
        assert_eq!(driver.actions().len(), 4);
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
            .iter()
            .any(|action| matches!(action, Action::Replace(_))));
    }

    #[tokio::test]
    async fn driver_error_maps_to_current_step_without_leaking_error_details() {
        let driver = FakeDriver::with_waits(["wegame.loginFormReady", "wegame.loginFormReady"]);
        *driver.fail_replace_target.lock().unwrap() = Some("wegame.account".to_string());

        let result = run(&driver).await;

        let LoginFlowResult::Paused {
            failed_step,
            last_observation,
            ..
        } = result
        else {
            panic!("预期流程暂停");
        };
        assert_eq!(failed_step, LoginStep::InputAccount);
        assert!(!last_observation.contains("secret-password"));
        assert!(!last_observation.contains("123456789"));
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

    #[tokio::test]
    async fn callback_reports_exact_step_order() {
        let driver = FakeDriver::with_waits([
            "wegame.loginMode",
            "wegame.loginFormReady",
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
                LoginStep::StopGame,
                LoginStep::StopWeGame,
                LoginStep::StartWeGame,
                LoginStep::WaitLoginChoice,
                LoginStep::OpenLoginForm,
                LoginStep::InputAccount,
                LoginStep::InputPassword,
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
            last_observation: "WaitGameEntry：步骤超时".to_string(),
            failed_at: 123,
        })
        .unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "kind": "paused",
                "failedStep": "waitGameEntry",
                "lastObservation": "WaitGameEntry：步骤超时",
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
