use super::{
    craft_runtime::CraftStationOutcome, craft_trial::CraftTrialFailure, AccountPlan, AccountStatus,
    BusinessConfig, StationKind, StationStatus,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CraftBatchTask {
    pub station: StationKind,
    pub duration_minutes: u32,
}

pub(crate) fn select_due_craft_tasks(
    account: &AccountPlan,
    business_config: &BusinessConfig,
    frozen_now_ms: i64,
) -> Result<Vec<CraftBatchTask>, String> {
    if account.status != AccountStatus::Ready {
        return Err("当前账号状态不是 Ready，禁止启动制作批处理".to_string());
    }

    let tasks = StationKind::all()
        .into_iter()
        .filter_map(|kind| {
            let station = account.stations.iter().find(|item| item.kind == kind)?;
            let business = business_config
                .stations
                .iter()
                .find(|item| item.kind == kind)?;
            if !business.enabled || station.status == StationStatus::Uncertain {
                return None;
            }
            station
                .finishes_at_ms
                .filter(|finishes_at_ms| *finishes_at_ms <= frozen_now_ms)
                .map(|_| CraftBatchTask {
                    station: kind,
                    duration_minutes: business.duration_minutes,
                })
        })
        .collect();

    Ok(tasks)
}

pub(crate) struct StationAttempt {
    pub result: Result<CraftStationOutcome, CraftTrialFailure>,
    pub entered_input: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CraftBatchSuccess {
    pub processed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CraftBatchFailure {
    pub station: StationKind,
    pub failure: CraftTrialFailure,
    pub entered_input: bool,
}

#[allow(async_fn_in_trait)]
pub(crate) trait CraftBatchDriver: Send + Sync {
    async fn ensure_station_grid(
        &self,
        task: &CraftBatchTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), CraftTrialFailure>;
    fn update_progress(
        &self,
        index: usize,
        total: usize,
        station: &StationKind,
    ) -> Result<(), String>;
    async fn run_station(
        &self,
        task: &CraftBatchTask,
        cancelled: Arc<AtomicBool>,
    ) -> StationAttempt;
    fn persist_started(&self, task: &CraftBatchTask, started_at_ms: i64) -> Result<(), String>;
    async fn return_started(
        &self,
        task: &CraftBatchTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), CraftTrialFailure>;
}

pub(crate) async fn run_craft_batch<D: CraftBatchDriver + ?Sized>(
    driver: &D,
    tasks: &[CraftBatchTask],
    cancelled: Arc<AtomicBool>,
) -> Result<CraftBatchSuccess, CraftBatchFailure> {
    let Some(first_task) = tasks.first() else {
        return Ok(CraftBatchSuccess { processed: 0 });
    };
    driver
        .ensure_station_grid(first_task, Arc::clone(&cancelled))
        .await
        .map_err(|failure| CraftBatchFailure {
            station: first_task.station.clone(),
            failure,
            entered_input: false,
        })?;

    for (offset, task) in tasks.iter().enumerate() {
        if cancelled.load(Ordering::SeqCst) {
            return Err(CraftBatchFailure {
                station: task.station.clone(),
                failure: CraftTrialFailure {
                    step: "craft.batchCancelled".to_string(),
                    message: "制作批处理已取消".to_string(),
                    requires_uncertain: false,
                },
                entered_input: false,
            });
        }
        driver
            .update_progress(offset + 1, tasks.len(), &task.station)
            .map_err(|message| CraftBatchFailure {
                station: task.station.clone(),
                failure: CraftTrialFailure {
                    step: "craft.batchProgress".to_string(),
                    message,
                    requires_uncertain: false,
                },
                entered_input: false,
            })?;

        let attempt = driver.run_station(task, Arc::clone(&cancelled)).await;
        let outcome = attempt.result.map_err(|failure| CraftBatchFailure {
            station: task.station.clone(),
            failure,
            entered_input: attempt.entered_input,
        })?;
        if let CraftStationOutcome::Started { started_at_ms } = outcome {
            driver
                .persist_started(task, started_at_ms)
                .map_err(|message| CraftBatchFailure {
                    station: task.station.clone(),
                    failure: CraftTrialFailure {
                        step: "craft.persistStarted".to_string(),
                        message,
                        requires_uncertain: true,
                    },
                    entered_input: true,
                })?;
            driver
                .return_started(task, Arc::clone(&cancelled))
                .await
                .map_err(|failure| CraftBatchFailure {
                    station: task.station.clone(),
                    failure,
                    entered_input: true,
                })?;
        }
    }

    Ok(CraftBatchSuccess {
        processed: tasks.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_ops::{craft_runtime::CraftStationOutcome, craft_trial::CraftTrialFailure};
    use crate::special_ops::{
        AccountPlan, AccountStatus, BusinessConfig, StationBusinessConfig, StationKind,
        StationPlan, StationStatus,
    };
    use std::collections::VecDeque;
    use std::sync::{atomic::AtomicBool, Arc, Mutex};

    fn station(
        kind: StationKind,
        enabled: bool,
        status: StationStatus,
        finishes_at_ms: Option<i64>,
    ) -> StationPlan {
        StationPlan {
            kind,
            enabled,
            item_name: "测试物品".to_string(),
            duration_minutes: 240,
            started_at_ms: None,
            finishes_at_ms,
            status,
        }
    }

    fn account_with_stations(status: AccountStatus, stations: Vec<StationPlan>) -> AccountPlan {
        AccountPlan {
            id: "selected".to_string(),
            qq_account: "10001".to_string(),
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

    fn business_config(account: &AccountPlan) -> BusinessConfig {
        BusinessConfig {
            stations: account
                .stations
                .iter()
                .map(|station| StationBusinessConfig {
                    kind: station.kind.clone(),
                    enabled: station.enabled,
                    duration_minutes: station.duration_minutes,
                    recipe_note: station.item_name.clone(),
                })
                .collect(),
            recipe_points: Vec::new(),
            ammo_targets: Vec::new(),
            market: Default::default(),
        }
    }

    #[test]
    fn selects_due_enabled_stations_in_fixed_order() {
        let account = account_with_stations(
            AccountStatus::Ready,
            vec![
                station(
                    StationKind::ArmorBench,
                    true,
                    StationStatus::Crafting,
                    Some(100),
                ),
                station(
                    StationKind::Pharmacy,
                    false,
                    StationStatus::Crafting,
                    Some(100),
                ),
                station(
                    StationKind::Workbench,
                    true,
                    StationStatus::Uncertain,
                    Some(100),
                ),
                station(
                    StationKind::TechnicalCenter,
                    true,
                    StationStatus::Crafting,
                    Some(100),
                ),
            ],
        );

        let tasks = select_due_craft_tasks(&account, &business_config(&account), 100).unwrap();

        assert_eq!(
            tasks
                .iter()
                .map(|task| task.station.clone())
                .collect::<Vec<_>>(),
            [StationKind::TechnicalCenter, StationKind::ArmorBench]
        );
    }

    #[test]
    fn rejects_account_that_is_not_ready() {
        let account = account_with_stations(AccountStatus::Uncertain, Vec::new());

        assert_eq!(
            select_due_craft_tasks(&account, &business_config(&account), 100).unwrap_err(),
            "当前账号状态不是 Ready，禁止启动制作批处理"
        );
    }

    #[test]
    fn excludes_missing_and_future_finish_times() {
        let account = account_with_stations(
            AccountStatus::Ready,
            vec![
                station(
                    StationKind::TechnicalCenter,
                    true,
                    StationStatus::Crafting,
                    None,
                ),
                station(
                    StationKind::Workbench,
                    true,
                    StationStatus::Crafting,
                    Some(101),
                ),
            ],
        );

        assert!(
            select_due_craft_tasks(&account, &business_config(&account), 100)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn ready_account_without_due_stations_returns_empty_tasks() {
        let account = account_with_stations(AccountStatus::Ready, Vec::new());

        assert!(
            select_due_craft_tasks(&account, &business_config(&account), 100)
                .unwrap()
                .is_empty()
        );
    }

    struct FakeBatchDriver {
        actions: Mutex<Vec<String>>,
        attempts: Mutex<VecDeque<StationAttempt>>,
        persist_failure: Mutex<Option<String>>,
        return_failure: Mutex<Option<CraftTrialFailure>>,
    }

    impl FakeBatchDriver {
        fn with_attempts(attempts: impl IntoIterator<Item = StationAttempt>) -> Self {
            Self {
                actions: Mutex::new(Vec::new()),
                attempts: Mutex::new(attempts.into_iter().collect()),
                persist_failure: Mutex::new(None),
                return_failure: Mutex::new(None),
            }
        }

        fn actions(&self) -> Vec<String> {
            self.actions.lock().unwrap().clone()
        }

        fn push(&self, action: impl Into<String>) {
            self.actions.lock().unwrap().push(action.into());
        }
    }

    impl CraftBatchDriver for FakeBatchDriver {
        async fn ensure_station_grid(
            &self,
            _: &CraftBatchTask,
            _: Arc<AtomicBool>,
        ) -> Result<(), CraftTrialFailure> {
            self.push("ensure-grid");
            Ok(())
        }

        fn update_progress(
            &self,
            index: usize,
            total: usize,
            station: &StationKind,
        ) -> Result<(), String> {
            self.push(format!(
                "progress:{index}/{total}:{}",
                station_suffix(station)
            ));
            Ok(())
        }

        async fn run_station(&self, task: &CraftBatchTask, _: Arc<AtomicBool>) -> StationAttempt {
            self.push(format!("run:{}", station_suffix(&task.station)));
            self.attempts.lock().unwrap().pop_front().unwrap()
        }

        fn persist_started(&self, task: &CraftBatchTask, started_at_ms: i64) -> Result<(), String> {
            self.push(format!(
                "persist:{}:{started_at_ms}:{}",
                station_suffix(&task.station),
                task.duration_minutes
            ));
            match self.persist_failure.lock().unwrap().take() {
                Some(message) => Err(message),
                None => Ok(()),
            }
        }

        async fn return_started(
            &self,
            task: &CraftBatchTask,
            _: Arc<AtomicBool>,
        ) -> Result<(), CraftTrialFailure> {
            self.push(format!("return:{}", station_suffix(&task.station)));
            match self.return_failure.lock().unwrap().take() {
                Some(failure) => Err(failure),
                None => Ok(()),
            }
        }
    }

    fn station_suffix(station: &StationKind) -> &'static str {
        match station {
            StationKind::TechnicalCenter => "technicalCenter",
            StationKind::Workbench => "workbench",
            StationKind::Pharmacy => "pharmacy",
            StationKind::ArmorBench => "armorBench",
        }
    }

    fn tasks() -> Vec<CraftBatchTask> {
        vec![
            CraftBatchTask {
                station: StationKind::TechnicalCenter,
                duration_minutes: 60,
            },
            CraftBatchTask {
                station: StationKind::Workbench,
                duration_minutes: 120,
            },
        ]
    }

    fn success(outcome: CraftStationOutcome) -> StationAttempt {
        StationAttempt {
            result: Ok(outcome),
            entered_input: true,
        }
    }

    #[tokio::test]
    async fn started_persists_before_return_and_next_station() {
        let driver = FakeBatchDriver::with_attempts([
            success(CraftStationOutcome::Started { started_at_ms: 10 }),
            success(CraftStationOutcome::StillInProgress),
        ]);

        let result = run_craft_batch(&driver, &tasks(), Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert_eq!(result.processed, 2);
        assert_eq!(
            driver.actions(),
            [
                "ensure-grid",
                "progress:1/2:technicalCenter",
                "run:technicalCenter",
                "persist:technicalCenter:10:60",
                "return:technicalCenter",
                "progress:2/2:workbench",
                "run:workbench",
            ]
        );
    }

    #[tokio::test]
    async fn final_started_station_still_returns_to_grid() {
        let driver = FakeBatchDriver::with_attempts([success(CraftStationOutcome::Started {
            started_at_ms: 20,
        })]);
        let one_task = vec![tasks()[0].clone()];

        run_craft_batch(&driver, &one_task, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert!(driver.actions().ends_with(&[
            "persist:technicalCenter:20:60".to_string(),
            "return:technicalCenter".to_string(),
        ]));
    }

    #[tokio::test]
    async fn station_failure_stops_remaining_tasks() {
        let driver = FakeBatchDriver::with_attempts([StationAttempt {
            result: Err(CraftTrialFailure {
                step: "craft.abort".to_string(),
                message: "识别失败".to_string(),
                requires_uncertain: true,
            }),
            entered_input: true,
        }]);

        let failure = run_craft_batch(&driver, &tasks(), Arc::new(AtomicBool::new(false)))
            .await
            .unwrap_err();

        assert_eq!(failure.station, StationKind::TechnicalCenter);
        assert!(failure.entered_input);
        assert_eq!(failure.failure.step, "craft.abort");
        assert!(!driver
            .actions()
            .iter()
            .any(|action| action == "run:workbench"));
    }

    #[tokio::test]
    async fn persistence_failure_stops_before_return_and_marks_input() {
        let driver = FakeBatchDriver::with_attempts([success(CraftStationOutcome::Started {
            started_at_ms: 10,
        })]);
        *driver.persist_failure.lock().unwrap() = Some("写入失败".to_string());

        let failure = run_craft_batch(&driver, &tasks(), Arc::new(AtomicBool::new(false)))
            .await
            .unwrap_err();

        assert_eq!(failure.failure.step, "craft.persistStarted");
        assert!(failure.failure.requires_uncertain);
        assert!(failure.entered_input);
        assert!(!driver
            .actions()
            .iter()
            .any(|action| action.starts_with("return:")));
    }

    #[tokio::test]
    async fn cancellation_before_station_stops_without_input() {
        let driver = FakeBatchDriver::with_attempts([]);
        let cancelled = Arc::new(AtomicBool::new(true));

        let failure = run_craft_batch(&driver, &tasks(), cancelled)
            .await
            .unwrap_err();

        assert_eq!(failure.station, StationKind::TechnicalCenter);
        assert!(!failure.entered_input);
        assert!(!failure.failure.requires_uncertain);
        assert_eq!(driver.actions(), ["ensure-grid"]);
    }
}
