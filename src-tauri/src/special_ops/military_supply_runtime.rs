use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct MilitarySupplyEntryConfig {
    pub(crate) supply_delay: Duration,
    pub(crate) enter_supply_delay: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MilitarySupplyEntryError {
    Cancelled,
    Target { step: String, message: String },
    System { step: String, message: String },
}

#[allow(async_fn_in_trait)]
pub(crate) trait MilitarySupplyEntryDriver: Send + Sync {
    async fn wait_and_click(
        &self,
        key: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), MilitarySupplyEntryError>;
    async fn click_unverified(
        &self,
        key: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), MilitarySupplyEntryError>;
    async fn delay(
        &self,
        duration: Duration,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), MilitarySupplyEntryError>;
}

pub(crate) async fn enter_military_supply<D: MilitarySupplyEntryDriver + ?Sized>(
    driver: &D,
    config: MilitarySupplyEntryConfig,
    cancelled: Arc<AtomicBool>,
) -> Result<(), MilitarySupplyEntryError> {
    if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(MilitarySupplyEntryError::Cancelled);
    }
    driver
        .wait_and_click("ammo.department", Arc::clone(&cancelled))
        .await?;
    for (delay, key) in [
        (config.supply_delay, "ammo.supply"),
        (config.enter_supply_delay, "ammo.enterSupply"),
    ] {
        driver.delay(delay, Arc::clone(&cancelled)).await?;
        driver.click_unverified(key, Arc::clone(&cancelled)).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{atomic::AtomicBool, Arc, Mutex},
        time::Duration,
    };

    struct ScriptedDriver {
        actions: Mutex<Vec<String>>,
    }

    impl ScriptedDriver {
        fn new() -> Self {
            Self {
                actions: Mutex::new(Vec::new()),
            }
        }
    }

    impl MilitarySupplyEntryDriver for ScriptedDriver {
        async fn wait_and_click(
            &self,
            key: &str,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), MilitarySupplyEntryError> {
            self.actions.lock().unwrap().push(format!("click:{key}"));
            Ok(())
        }

        async fn click_unverified(
            &self,
            key: &str,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), MilitarySupplyEntryError> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("unchecked:{key}"));
            Ok(())
        }

        async fn delay(
            &self,
            duration: Duration,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), MilitarySupplyEntryError> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("delay:{}", duration.as_millis()));
            Ok(())
        }
    }

    #[tokio::test]
    async fn shared_entry_runs_once_in_fixed_order() {
        let driver = ScriptedDriver::new();

        enter_military_supply(
            &driver,
            MilitarySupplyEntryConfig {
                supply_delay: Duration::from_millis(2_000),
                enter_supply_delay: Duration::from_millis(3_000),
            },
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();

        assert_eq!(
            *driver.actions.lock().unwrap(),
            [
                "click:ammo.department",
                "delay:2000",
                "unchecked:ammo.supply",
                "delay:3000",
                "unchecked:ammo.enterSupply",
            ]
        );
    }
}
