use super::{
    round_planner::AccountRoundTask,
    round_runner::{AccountRunError, AccountRunSuccess},
};
use std::sync::{atomic::AtomicBool, Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct MilitarySupplySessionResult {
    pub(crate) limited_retry_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarketSessionResult {
    Completed,
    YieldedForCraft,
    PauseRequested,
    WindowClosed,
}

#[allow(async_fn_in_trait)]
pub(crate) trait AccountSessionDriver: Send + Sync {
    async fn login(
        &self,
        task: &AccountRoundTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), AccountRunError>;
    async fn navigate(
        &self,
        task: &AccountRoundTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), AccountRunError>;
    async fn craft(
        &self,
        task: &AccountRoundTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<usize, AccountRunError>;
    async fn military_supply(
        &self,
        task: &AccountRoundTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<MilitarySupplySessionResult, AccountRunError>;
    async fn market(
        &self,
        task: &AccountRoundTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<MarketSessionResult, AccountRunError>;
}

pub(crate) async fn run_account_session<D: AccountSessionDriver + ?Sized>(
    driver: &D,
    task: &AccountRoundTask,
    cancelled: Arc<AtomicBool>,
) -> Result<AccountRunSuccess, AccountRunError> {
    start_account_session(driver, task, Arc::clone(&cancelled)).await?;
    run_task_in_session(driver, task, cancelled).await
}

pub(crate) async fn start_account_session<D: AccountSessionDriver + ?Sized>(
    driver: &D,
    task: &AccountRoundTask,
    cancelled: Arc<AtomicBool>,
) -> Result<(), AccountRunError> {
    driver.login(task, Arc::clone(&cancelled)).await?;
    driver.navigate(task, cancelled).await
}

pub(crate) async fn run_task_in_session<D: AccountSessionDriver + ?Sized>(
    driver: &D,
    task: &AccountRoundTask,
    cancelled: Arc<AtomicBool>,
) -> Result<AccountRunSuccess, AccountRunError> {
    let processed_stations = if task.stations.is_empty() {
        0
    } else {
        driver.craft(task, Arc::clone(&cancelled)).await?
    };
    let military_supply =
        if !task.ammo_target_ids.is_empty() || task.limited_supply_cycle_id.is_some() {
            driver.military_supply(task, Arc::clone(&cancelled)).await?
        } else {
            MilitarySupplySessionResult::default()
        };
    let market_result = if task.market_purchase_day.is_some() {
        Some(driver.market(task, cancelled).await?)
    } else {
        None
    };
    Ok(AccountRunSuccess {
        processed_stations,
        limited_retry_requested: military_supply.limited_retry_requested,
        market_pending: matches!(
            market_result,
            Some(MarketSessionResult::YieldedForCraft | MarketSessionResult::PauseRequested)
        ),
        market_yielded: market_result == Some(MarketSessionResult::YieldedForCraft),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_ops::{
        round_planner::AccountRoundTask,
        round_runner::{AccountRunError, AccountRunSuccess},
        StationKind,
    };
    use std::sync::{atomic::AtomicBool, Arc, Mutex};

    struct FakeDriver {
        actions: Mutex<Vec<&'static str>>,
        login_error: Option<AccountRunError>,
        craft_error: Option<AccountRunError>,
    }

    impl FakeDriver {
        fn success() -> Self {
            Self {
                actions: Mutex::new(Vec::new()),
                login_error: None,
                craft_error: None,
            }
        }
    }

    impl AccountSessionDriver for FakeDriver {
        async fn login(
            &self,
            _task: &AccountRoundTask,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), AccountRunError> {
            self.actions.lock().unwrap().push("login");
            self.login_error.clone().map_or(Ok(()), Err)
        }

        async fn navigate(
            &self,
            _task: &AccountRoundTask,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), AccountRunError> {
            self.actions.lock().unwrap().push("navigation");
            Ok(())
        }

        async fn craft(
            &self,
            _task: &AccountRoundTask,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<usize, AccountRunError> {
            self.actions.lock().unwrap().push("craft");
            self.craft_error.clone().map_or(Ok(2), Err)
        }

        async fn military_supply(
            &self,
            _task: &AccountRoundTask,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<MilitarySupplySessionResult, AccountRunError> {
            self.actions.lock().unwrap().push("militarySupply");
            Ok(MilitarySupplySessionResult::default())
        }

        async fn market(
            &self,
            _task: &AccountRoundTask,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<MarketSessionResult, AccountRunError> {
            self.actions.lock().unwrap().push("market");
            Ok(MarketSessionResult::Completed)
        }
    }

    fn task() -> AccountRoundTask {
        AccountRoundTask {
            account_id: "a".to_string(),
            qq_account: "123456789".to_string(),
            account_order: 0,
            scheduled_at_ms: 0,
            stations: vec![StationKind::TechnicalCenter, StationKind::Workbench],
            ammo_target_ids: Vec::new(),
            limited_supply_cycle_id: None,
            market_purchase_day: None,
        }
    }

    #[tokio::test]
    async fn runs_login_navigation_then_craft() {
        let driver = FakeDriver::success();

        let result = run_account_session(&driver, &task(), Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert_eq!(
            result,
            AccountRunSuccess {
                processed_stations: 2,
                ..Default::default()
            }
        );
        assert_eq!(
            *driver.actions.lock().unwrap(),
            ["login", "navigation", "craft"]
        );
    }

    #[tokio::test]
    async fn login_failure_skips_navigation_and_craft() {
        let driver = FakeDriver {
            actions: Mutex::new(Vec::new()),
            login_error: Some(AccountRunError::account("login.scan", "未找到 QQ")),
            craft_error: None,
        };

        let result = run_account_session(&driver, &task(), Arc::new(AtomicBool::new(false))).await;

        assert!(result.is_err());
        assert_eq!(*driver.actions.lock().unwrap(), ["login"]);
    }

    #[tokio::test]
    async fn ammo_only_runs_login_navigation_then_ammo() {
        let mut task = task();
        task.stations.clear();
        task.ammo_target_ids = vec!["normal".to_string()];
        let driver = FakeDriver::success();

        run_account_session(&driver, &task, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert_eq!(
            *driver.actions.lock().unwrap(),
            ["login", "navigation", "militarySupply"]
        );
    }

    #[tokio::test]
    async fn combined_account_runs_craft_before_ammo() {
        let mut task = task();
        task.ammo_target_ids = vec!["normal".to_string()];
        let driver = FakeDriver::success();

        run_account_session(&driver, &task, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert_eq!(
            *driver.actions.lock().unwrap(),
            ["login", "navigation", "craft", "militarySupply"]
        );
    }

    #[tokio::test]
    async fn account_runs_craft_then_one_military_supply_action_then_market() {
        let mut task = task();
        task.ammo_target_ids = vec!["normal".to_string()];
        task.limited_supply_cycle_id = Some("2026-08-08T12:00".to_string());
        task.market_purchase_day = Some("2026-08-08".to_string());
        let driver = FakeDriver::success();

        run_task_in_session(&driver, &task, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert_eq!(
            *driver.actions.lock().unwrap(),
            ["craft", "militarySupply", "market"]
        );
    }

    #[tokio::test]
    async fn craft_failure_skips_ammo() {
        let mut task = task();
        task.ammo_target_ids = vec!["normal".to_string()];
        let driver = FakeDriver {
            actions: Mutex::new(Vec::new()),
            login_error: None,
            craft_error: Some(AccountRunError::account_station(
                StationKind::TechnicalCenter,
                "craft.abort",
                "中止状态未确认",
            )),
        };

        let result = run_account_session(&driver, &task, Arc::new(AtomicBool::new(false))).await;

        assert!(result.is_err());
        assert_eq!(
            *driver.actions.lock().unwrap(),
            ["login", "navigation", "craft"]
        );
    }

    #[tokio::test]
    async fn follow_up_task_in_same_session_skips_login_and_navigation() {
        let driver = FakeDriver::success();

        let result = run_task_in_session(&driver, &task(), Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert_eq!(result.processed_stations, 2);
        assert_eq!(*driver.actions.lock().unwrap(), ["craft"]);
    }
}
