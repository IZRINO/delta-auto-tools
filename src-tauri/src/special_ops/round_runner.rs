use super::{
    round_planner::{AccountRoundTask, RoundPlan},
    StationKind,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrorScope {
    Account,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountRunError {
    pub scope: ErrorScope,
    pub station: Option<StationKind>,
    pub ammo_target_id: Option<String>,
    pub step: String,
    pub message: String,
}

impl AccountRunError {
    pub(crate) fn account(step: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            scope: ErrorScope::Account,
            station: None,
            ammo_target_id: None,
            step: step.into(),
            message: message.into(),
        }
    }

    pub(crate) fn system(step: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            scope: ErrorScope::System,
            station: None,
            ammo_target_id: None,
            step: step.into(),
            message: message.into(),
        }
    }

    pub(crate) fn account_station(
        station: StationKind,
        step: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            scope: ErrorScope::Account,
            station: Some(station),
            ammo_target_id: None,
            step: step.into(),
            message: message.into(),
        }
    }

    pub(crate) fn account_ammo(
        target_id: impl Into<String>,
        step: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            scope: ErrorScope::Account,
            station: None,
            ammo_target_id: Some(target_id.into()),
            step: step.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountRunSuccess {
    pub processed_stations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RoundStop {
    Completed,
    PauseRequested,
    EmergencyStopped,
    SystemFailure { step: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RoundRunResult {
    pub completed_accounts: usize,
    pub stop: RoundStop,
}

#[allow(async_fn_in_trait)]
pub(crate) trait RoundDriver: Send + Sync {
    async fn run_account(
        &self,
        index: usize,
        total: usize,
        task: &AccountRoundTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<AccountRunSuccess, AccountRunError>;
    fn persist_account_failure(
        &self,
        task: &AccountRoundTask,
        error: &AccountRunError,
    ) -> Result<(), String>;
    async fn close_game(&self) -> Result<(), String>;
    fn pause_requested(&self) -> Result<bool, String>;
    fn persist_paused(&self, reason: &str) -> Result<(), String>;
}

pub(crate) async fn run_round<D: RoundDriver + ?Sized>(
    driver: &D,
    plan: &RoundPlan,
    cancelled: Arc<AtomicBool>,
) -> RoundRunResult {
    let mut completed_accounts = 0;
    for (offset, task) in plan.accounts.iter().enumerate() {
        if cancelled.load(Ordering::SeqCst) {
            return RoundRunResult {
                completed_accounts,
                stop: RoundStop::EmergencyStopped,
            };
        }
        match driver
            .run_account(
                offset + 1,
                plan.accounts.len(),
                task,
                Arc::clone(&cancelled),
            )
            .await
        {
            Ok(_) => completed_accounts += 1,
            Err(error) if error.scope == ErrorScope::Account => {
                if let Err(message) = driver.persist_account_failure(task, &error) {
                    let _ = driver.persist_paused("账号失败状态保存失败");
                    return RoundRunResult {
                        completed_accounts,
                        stop: RoundStop::SystemFailure {
                            step: "round.persistAccountFailure".to_string(),
                            message,
                        },
                    };
                }
            }
            Err(error) => {
                let message = match driver.persist_paused(&error.message) {
                    Ok(()) => error.message,
                    Err(persist_error) => {
                        format!("{}；暂停状态保存失败：{persist_error}", error.message)
                    }
                };
                return RoundRunResult {
                    completed_accounts,
                    stop: RoundStop::SystemFailure {
                        step: error.step,
                        message,
                    },
                };
            }
        }

        if cancelled.load(Ordering::SeqCst) {
            return RoundRunResult {
                completed_accounts,
                stop: RoundStop::EmergencyStopped,
            };
        }
        match driver.pause_requested() {
            Ok(true) => {
                let stop = match driver.persist_paused("用户请求暂停") {
                    Ok(()) => match driver.close_game().await {
                        Ok(()) => RoundStop::PauseRequested,
                        Err(message) => RoundStop::SystemFailure {
                            step: "round.closeGame".to_string(),
                            message,
                        },
                    },
                    Err(message) => RoundStop::SystemFailure {
                        step: "round.persistPause".to_string(),
                        message,
                    },
                };
                return RoundRunResult {
                    completed_accounts,
                    stop,
                };
            }
            Ok(false) => {}
            Err(message) => {
                let _ = driver.persist_paused("暂停请求读取失败");
                return RoundRunResult {
                    completed_accounts,
                    stop: RoundStop::SystemFailure {
                        step: "round.pauseRequested".to_string(),
                        message,
                    },
                };
            }
        }
    }
    let stop = match driver.close_game().await {
        Ok(()) => RoundStop::Completed,
        Err(message) => {
            let message = match driver.persist_paused("轮次结束关闭游戏失败") {
                Ok(()) => message,
                Err(persist_error) => format!("{message}；暂停状态保存失败：{persist_error}"),
            };
            RoundStop::SystemFailure {
                step: "round.closeGame".to_string(),
                message,
            }
        }
    };
    RoundRunResult {
        completed_accounts,
        stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_ops::{
        round_planner::{AccountRoundTask, RoundPlan, RoundTrigger},
        StationKind,
    };
    use std::sync::{atomic::AtomicBool, Arc, Mutex};

    struct FakeDriver {
        results: Mutex<Vec<Result<AccountRunSuccess, AccountRunError>>>,
        actions: Mutex<Vec<String>>,
        pause_requested: bool,
        close_result: Mutex<Option<Result<(), String>>>,
    }

    impl FakeDriver {
        fn new(
            results: Vec<Result<AccountRunSuccess, AccountRunError>>,
            pause_requested: bool,
        ) -> Self {
            Self {
                results: Mutex::new(results.into_iter().rev().collect()),
                actions: Mutex::new(Vec::new()),
                pause_requested,
                close_result: Mutex::new(Some(Ok(()))),
            }
        }

        fn with_close_failure(self, message: &str) -> Self {
            *self.close_result.lock().unwrap() = Some(Err(message.to_string()));
            self
        }

        async fn perform_close_game(&self) -> Result<(), String> {
            self.actions.lock().unwrap().push("close-game".to_string());
            self.close_result.lock().unwrap().take().unwrap_or(Ok(()))
        }

        fn actions(&self) -> Vec<String> {
            self.actions.lock().unwrap().clone()
        }
    }

    impl RoundDriver for FakeDriver {
        async fn run_account(
            &self,
            _index: usize,
            _total: usize,
            task: &AccountRoundTask,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<AccountRunSuccess, AccountRunError> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("run:{}", task.account_id));
            self.results.lock().unwrap().pop().unwrap()
        }

        fn persist_account_failure(
            &self,
            task: &AccountRoundTask,
            _error: &AccountRunError,
        ) -> Result<(), String> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("persist-account:{}", task.account_id));
            Ok(())
        }

        async fn close_game(&self) -> Result<(), String> {
            self.perform_close_game().await
        }

        fn pause_requested(&self) -> Result<bool, String> {
            Ok(self.pause_requested)
        }

        fn persist_paused(&self, _reason: &str) -> Result<(), String> {
            self.actions
                .lock()
                .unwrap()
                .push("persist-pause".to_string());
            Ok(())
        }
    }

    fn plan() -> RoundPlan {
        RoundPlan {
            created_at_ms: 1,
            trigger: RoundTrigger::Manual,
            accounts: ["a", "b"]
                .into_iter()
                .enumerate()
                .map(|(order, id)| AccountRoundTask {
                    account_id: id.to_string(),
                    qq_account: format!("100{order}"),
                    account_order: order as u32,
                    stations: vec![StationKind::TechnicalCenter],
                    ammo_target_ids: Vec::new(),
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn account_failure_is_persisted_then_next_account_runs() {
        let driver = FakeDriver::new(
            vec![
                Err(AccountRunError::account("login.scan", "目标 QQ 不存在")),
                Ok(AccountRunSuccess {
                    processed_stations: 1,
                }),
            ],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.completed_accounts, 1);
        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            ["run:a", "persist-account:a", "run:b", "close-game"]
        );
    }

    #[tokio::test]
    async fn navigation_timeout_persists_current_account_then_runs_next_account() {
        let driver = FakeDriver::new(
            vec![
                Err(AccountRunError::account(
                    "navigation.WaitStationGrid",
                    "步骤超时",
                )),
                Ok(AccountRunSuccess {
                    processed_stations: 1,
                }),
            ],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            ["run:a", "persist-account:a", "run:b", "close-game"]
        );
    }

    #[tokio::test]
    async fn account_confirmation_failure_persists_then_next_account_rebuilds_session() {
        let driver = FakeDriver::new(
            vec![
                Err(AccountRunError::account(
                    "ammo.confirm",
                    "未识别到置顶确认按钮",
                )),
                Ok(AccountRunSuccess {
                    processed_stations: 0,
                }),
            ],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            ["run:a", "persist-account:a", "run:b", "close-game"]
        );
    }

    #[tokio::test]
    async fn system_failure_pauses_and_never_runs_next_account() {
        let driver = FakeDriver::new(
            vec![Err(AccountRunError::system(
                "runtime.persistence",
                "保存失败",
            ))],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert!(matches!(result.stop, RoundStop::SystemFailure { .. }));
        assert_eq!(driver.actions(), ["run:a", "persist-pause"]);
    }

    #[tokio::test]
    async fn pause_request_is_applied_after_current_account() {
        let driver = FakeDriver::new(
            vec![Ok(AccountRunSuccess {
                processed_stations: 1,
            })],
            true,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.completed_accounts, 1);
        assert_eq!(result.stop, RoundStop::PauseRequested);
        assert_eq!(driver.actions(), ["run:a", "persist-pause", "close-game"]);
    }

    #[tokio::test]
    async fn completed_round_closes_game() {
        let driver = FakeDriver::new(
            vec![
                Ok(AccountRunSuccess {
                    processed_stations: 1,
                }),
                Ok(AccountRunSuccess {
                    processed_stations: 1,
                }),
            ],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(driver.actions(), ["run:a", "run:b", "close-game"]);
    }

    #[tokio::test]
    async fn pause_request_closes_game_after_persisting_pause() {
        let driver = FakeDriver::new(
            vec![Ok(AccountRunSuccess {
                processed_stations: 1,
            })],
            true,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::PauseRequested);
        assert_eq!(driver.actions(), ["run:a", "persist-pause", "close-game"]);
    }

    #[tokio::test]
    async fn system_failure_preserves_game_for_diagnosis() {
        let driver = FakeDriver::new(
            vec![Err(AccountRunError::system("runtime.capture", "截图失败"))],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert!(matches!(result.stop, RoundStop::SystemFailure { .. }));
        assert_eq!(driver.actions(), ["run:a", "persist-pause"]);
    }

    #[tokio::test]
    async fn emergency_stop_preserves_game_for_manual_confirmation() {
        let driver = FakeDriver::new(Vec::new(), false);
        let cancelled = Arc::new(AtomicBool::new(true));

        let result = run_round(&driver, &plan(), cancelled).await;

        assert_eq!(result.stop, RoundStop::EmergencyStopped);
        assert!(driver.actions().is_empty());
    }

    #[tokio::test]
    async fn completed_round_close_failure_pauses_automation() {
        let driver = FakeDriver::new(
            vec![
                Ok(AccountRunSuccess {
                    processed_stations: 1,
                }),
                Ok(AccountRunSuccess {
                    processed_stations: 1,
                }),
            ],
            false,
        )
        .with_close_failure("无法结束游戏进程");

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(
            result.stop,
            RoundStop::SystemFailure {
                step: "round.closeGame".to_string(),
                message: "无法结束游戏进程".to_string(),
            }
        );
        assert_eq!(
            driver.actions(),
            ["run:a", "run:b", "close-game", "persist-pause"]
        );
    }

    #[test]
    fn craft_account_error_keeps_station_context() {
        let error =
            AccountRunError::account_station(StationKind::Workbench, "craft.abort", "识别失败");

        assert_eq!(error.station, Some(StationKind::Workbench));
        assert_eq!(error.scope, ErrorScope::Account);
    }

    #[test]
    fn account_ammo_error_keeps_target_context() {
        let error = AccountRunError::account_ammo("ammo-a", "ammo.success", "未确认完成");

        assert_eq!(error.station, None);
        assert_eq!(error.ammo_target_id.as_deref(), Some("ammo-a"));
        assert_eq!(error.scope, ErrorScope::Account);
    }
}
