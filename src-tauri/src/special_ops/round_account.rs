use super::{
    round_planner::AccountRoundTask,
    round_runner::{AccountRunError, AccountRunSuccess},
};
use std::sync::{atomic::AtomicBool, Arc};

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
    async fn ammo(
        &self,
        task: &AccountRoundTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), AccountRunError>;
}

pub(crate) async fn run_account_session<D: AccountSessionDriver + ?Sized>(
    driver: &D,
    task: &AccountRoundTask,
    cancelled: Arc<AtomicBool>,
) -> Result<AccountRunSuccess, AccountRunError> {
    driver.login(task, Arc::clone(&cancelled)).await?;
    driver.navigate(task, Arc::clone(&cancelled)).await?;
    let processed_stations = if task.stations.is_empty() {
        0
    } else {
        driver.craft(task, Arc::clone(&cancelled)).await?
    };
    if !task.ammo_target_ids.is_empty() {
        driver.ammo(task, cancelled).await?;
    }
    Ok(AccountRunSuccess { processed_stations })
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

        async fn ammo(
            &self,
            _task: &AccountRoundTask,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), AccountRunError> {
            self.actions.lock().unwrap().push("ammo");
            Ok(())
        }
    }

    fn task() -> AccountRoundTask {
        AccountRoundTask {
            account_id: "a".to_string(),
            qq_account: "123456789".to_string(),
            account_order: 0,
            stations: vec![StationKind::TechnicalCenter, StationKind::Workbench],
            ammo_target_ids: Vec::new(),
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
                processed_stations: 2
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
            ["login", "navigation", "ammo"]
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
            ["login", "navigation", "craft", "ammo"]
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
}
