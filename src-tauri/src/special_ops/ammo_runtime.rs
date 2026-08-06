use crate::morse::types::RegionRect;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AmmoDriverError {
    Cancelled,
    Target(String),
    System { step: String, message: String },
}

#[derive(Debug, Clone)]
pub(crate) struct AmmoRunTarget {
    pub id: String,
    pub note: String,
    pub seasonal: bool,
    pub click_point: RegionRect,
    pub scroll_steps: u32,
    pub already_succeeded: bool,
    pub retry_count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AmmoEntryDelays {
    pub supply_ms: u32,
    pub tactical_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AmmoRunStop {
    Completed,
    EmergencyStopped,
    Isolated {
        target_id: String,
        step: String,
        message: String,
    },
    Uncertain {
        target_id: String,
        step: String,
        message: String,
    },
    SystemFailure {
        step: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AmmoRunResult {
    pub stop: AmmoRunStop,
}

#[allow(async_fn_in_trait)]
pub(crate) trait AmmoDriver: Send + Sync {
    fn update_stage(&self, message: &str) -> Result<(), AmmoDriverError>;
    async fn wait_and_click(
        &self,
        target: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), AmmoDriverError>;
    async fn click_unverified(
        &self,
        target: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), AmmoDriverError>;
    async fn wait_target(
        &self,
        targets: &[&str],
        cancelled: Arc<AtomicBool>,
    ) -> Result<String, AmmoDriverError>;
    async fn position_and_click(
        &self,
        point: &RegionRect,
        scroll_steps: u32,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), AmmoDriverError>;
    async fn delay(
        &self,
        duration: Duration,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), AmmoDriverError>;
    fn persist_success(&self, target_id: &str) -> Result<(), AmmoDriverError>;
    fn persist_failure(
        &self,
        target_id: &str,
        step: &str,
        message: &str,
    ) -> Result<(), AmmoDriverError>;
    fn persist_isolated(
        &self,
        target_id: &str,
        step: &str,
        message: &str,
    ) -> Result<(), AmmoDriverError>;
}

enum TargetAttempt {
    Succeeded,
    Failed { step: String, message: String },
    Isolated { step: String, message: String },
    Uncertain { step: String, message: String },
    Stopped(AmmoRunStop),
}

fn stopped(error: AmmoDriverError, fallback_step: &str) -> AmmoRunStop {
    match error {
        AmmoDriverError::Cancelled => AmmoRunStop::EmergencyStopped,
        AmmoDriverError::Target(message) => AmmoRunStop::SystemFailure {
            step: fallback_step.to_string(),
            message,
        },
        AmmoDriverError::System { step, message } => AmmoRunStop::SystemFailure { step, message },
    }
}

fn observation_failed(error: AmmoDriverError, step: &str) -> TargetAttempt {
    match error {
        AmmoDriverError::Target(message) => TargetAttempt::Failed {
            step: step.to_string(),
            message,
        },
        other => TargetAttempt::Stopped(stopped(other, step)),
    }
}

fn confirmation_failed(error: AmmoDriverError, step: &str) -> TargetAttempt {
    match error {
        AmmoDriverError::Cancelled => TargetAttempt::Stopped(AmmoRunStop::EmergencyStopped),
        AmmoDriverError::Target(message) => TargetAttempt::Uncertain {
            step: step.to_string(),
            message,
        },
        AmmoDriverError::System { step, message } => {
            TargetAttempt::Stopped(AmmoRunStop::SystemFailure { step, message })
        }
    }
}

fn ensure_active(cancelled: &AtomicBool) -> Result<(), AmmoRunStop> {
    if cancelled.load(Ordering::SeqCst) {
        Err(AmmoRunStop::EmergencyStopped)
    } else {
        Ok(())
    }
}

async fn exchange_target<D: AmmoDriver + ?Sized>(
    driver: &D,
    cancelled: Arc<AtomicBool>,
) -> TargetAttempt {
    if let Err(stop) = ensure_active(&cancelled) {
        return TargetAttempt::Stopped(stop);
    }
    if let Err(error) = driver
        .wait_and_click("ammo.exchange", Arc::clone(&cancelled))
        .await
    {
        return TargetAttempt::Stopped(stopped(error, "ammo.exchange"));
    }
    if let Err(error) = driver
        .wait_and_click("ammo.confirm", Arc::clone(&cancelled))
        .await
    {
        return confirmation_failed(error, "ammo.confirm");
    }
    match driver.wait_target(&["ammo.success"], cancelled).await {
        Ok(target) if target == "ammo.success" => TargetAttempt::Succeeded,
        Ok(target) => TargetAttempt::Uncertain {
            step: "ammo.success".to_string(),
            message: format!("二次确认后命中非完成状态：{target}"),
        },
        Err(error) => confirmation_failed(error, "ammo.success"),
    }
}

async fn fill_target<D: AmmoDriver + ?Sized>(
    driver: &D,
    cancelled: Arc<AtomicBool>,
) -> TargetAttempt {
    if let Err(stop) = ensure_active(&cancelled) {
        return TargetAttempt::Stopped(stop);
    }
    if let Err(error) = driver
        .wait_and_click("ammo.fill", Arc::clone(&cancelled))
        .await
    {
        return TargetAttempt::Stopped(stopped(error, "ammo.fill"));
    }

    for attempt in 0..3 {
        if let Err(stop) = ensure_active(&cancelled) {
            return TargetAttempt::Stopped(stop);
        }
        if let Err(error) = driver
            .wait_and_click("ammo.purchase", Arc::clone(&cancelled))
            .await
        {
            return TargetAttempt::Stopped(stopped(error, "ammo.purchase"));
        }
        if let Err(error) = driver
            .delay(Duration::from_secs(1), Arc::clone(&cancelled))
            .await
        {
            return TargetAttempt::Stopped(stopped(error, "ammo.purchaseDelay"));
        }
        match driver
            .wait_target(&["ammo.exchange", "ammo.purchase"], Arc::clone(&cancelled))
            .await
        {
            Ok(target) if target == "ammo.exchange" => {
                return exchange_target(driver, cancelled).await;
            }
            Ok(target) if target == "ammo.purchase" && attempt < 2 => {}
            Ok(target) if target == "ammo.purchase" => {
                return TargetAttempt::Isolated {
                    step: "ammo.purchase".to_string(),
                    message: "材料购买重试 3 次后仍停留在购买页面，按仓库空间不足隔离账号"
                        .to_string(),
                };
            }
            Ok(target) => {
                return TargetAttempt::Uncertain {
                    step: "ammo.purchase".to_string(),
                    message: format!("购买后命中未知状态：{target}"),
                };
            }
            Err(error) => return confirmation_failed(error, "ammo.purchase"),
        }
    }

    unreachable!("第三次购买反馈必须在循环内返回")
}

async fn run_target<D: AmmoDriver + ?Sized>(
    driver: &D,
    target: &AmmoRunTarget,
    cancelled: Arc<AtomicBool>,
) -> TargetAttempt {
    if let Err(stop) = ensure_active(&cancelled) {
        return TargetAttempt::Stopped(stop);
    }
    if let Err(error) = driver.update_stage(&format!("正在兑换：{}", target.note)) {
        return TargetAttempt::Stopped(stopped(error, "ammo.progress"));
    }
    if let Err(error) = driver
        .position_and_click(
            &target.click_point,
            target.scroll_steps,
            Arc::clone(&cancelled),
        )
        .await
    {
        return TargetAttempt::Stopped(stopped(error, "ammo.targetPosition"));
    }

    match driver
        .wait_target(
            &["ammo.success", "ammo.exchange", "ammo.fill"],
            Arc::clone(&cancelled),
        )
        .await
    {
        Ok(observed) if observed == "ammo.success" => TargetAttempt::Succeeded,
        Ok(observed) if observed == "ammo.exchange" => exchange_target(driver, cancelled).await,
        Ok(observed) if observed == "ammo.fill" => fill_target(driver, cancelled).await,
        Ok(observed) => TargetAttempt::Failed {
            step: "ammo.targetState".to_string(),
            message: format!("命中未知子弹状态：{observed}"),
        },
        Err(error) => observation_failed(error, "ammo.targetState"),
    }
}

pub(crate) async fn run_ammo_trial<D: AmmoDriver + ?Sized>(
    driver: &D,
    targets: &[AmmoRunTarget],
    entry_delays: AmmoEntryDelays,
    cancelled: Arc<AtomicBool>,
) -> AmmoRunResult {
    let runnable = targets
        .iter()
        .filter(|target| !target.already_succeeded && target.retry_count < 2)
        .collect::<Vec<_>>();
    if runnable.is_empty() {
        return AmmoRunResult {
            stop: AmmoRunStop::Completed,
        };
    }
    if let Err(stop) = ensure_active(&cancelled) {
        return AmmoRunResult { stop };
    }

    if let Err(error) = driver
        .wait_and_click("ammo.department", Arc::clone(&cancelled))
        .await
    {
        return AmmoRunResult {
            stop: stopped(error, "ammo.department"),
        };
    }
    for (delay_ms, entry) in [
        (entry_delays.supply_ms, "ammo.supply"),
        (entry_delays.tactical_ms, "ammo.tactical"),
    ] {
        if let Err(error) = driver
            .delay(
                Duration::from_millis(u64::from(delay_ms)),
                Arc::clone(&cancelled),
            )
            .await
        {
            return AmmoRunResult {
                stop: stopped(error, entry),
            };
        }
        if let Err(error) = driver.click_unverified(entry, Arc::clone(&cancelled)).await {
            return AmmoRunResult {
                stop: stopped(error, entry),
            };
        }
    }

    for seasonal in [false, true] {
        let group = runnable
            .iter()
            .copied()
            .filter(|target| target.seasonal == seasonal)
            .collect::<Vec<_>>();
        if group.is_empty() {
            continue;
        }
        if seasonal {
            if let Err(error) = driver
                .click_unverified("ammo.seasonal", Arc::clone(&cancelled))
                .await
            {
                return AmmoRunResult {
                    stop: stopped(error, "ammo.seasonal"),
                };
            }
        }

        for target in group {
            match run_target(driver, target, Arc::clone(&cancelled)).await {
                TargetAttempt::Succeeded => {
                    if let Err(error) = driver.persist_success(&target.id) {
                        return AmmoRunResult {
                            stop: stopped(error, "ammo.persistSuccess"),
                        };
                    }
                }
                TargetAttempt::Failed { step, message } => {
                    if let Err(error) = driver.persist_failure(&target.id, &step, &message) {
                        return AmmoRunResult {
                            stop: stopped(error, "ammo.persistFailure"),
                        };
                    }
                }
                TargetAttempt::Isolated { step, message } => {
                    if let Err(error) = driver.persist_isolated(&target.id, &step, &message) {
                        return AmmoRunResult {
                            stop: stopped(error, "ammo.persistIsolated"),
                        };
                    }
                    return AmmoRunResult {
                        stop: AmmoRunStop::Isolated {
                            target_id: target.id.clone(),
                            step,
                            message,
                        },
                    };
                }
                TargetAttempt::Uncertain { step, message } => {
                    return AmmoRunResult {
                        stop: AmmoRunStop::Uncertain {
                            target_id: target.id.clone(),
                            step,
                            message,
                        },
                    };
                }
                TargetAttempt::Stopped(stop) => return AmmoRunResult { stop },
            }
        }
    }

    AmmoRunResult {
        stop: AmmoRunStop::Completed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morse::types::RegionRect;
    use std::{
        collections::VecDeque,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };

    struct ScriptedDriver {
        observations: Mutex<VecDeque<Result<String, AmmoDriverError>>>,
        actions: Mutex<Vec<String>>,
        cancel_on_delay: AtomicBool,
        wait_click_failure: Mutex<Option<(String, AmmoDriverError)>>,
    }

    impl ScriptedDriver {
        fn new(observations: impl IntoIterator<Item = &'static str>) -> Self {
            Self::from_observations(observations.into_iter().map(|value| Ok(value.to_string())))
        }

        fn from_observations(
            observations: impl IntoIterator<Item = Result<String, AmmoDriverError>>,
        ) -> Self {
            Self {
                observations: Mutex::new(observations.into_iter().collect()),
                actions: Mutex::new(Vec::new()),
                cancel_on_delay: AtomicBool::new(false),
                wait_click_failure: Mutex::new(None),
            }
        }

        fn fail_wait_and_click(&self, target: &str, error: AmmoDriverError) {
            *self.wait_click_failure.lock().unwrap() = Some((target.to_string(), error));
        }

        fn actions(&self) -> Vec<String> {
            self.actions.lock().unwrap().clone()
        }

        fn push(&self, action: impl Into<String>) {
            self.actions.lock().unwrap().push(action.into());
        }
    }

    impl AmmoDriver for ScriptedDriver {
        fn update_stage(&self, message: &str) -> Result<(), AmmoDriverError> {
            self.push(format!("stage:{message}"));
            Ok(())
        }

        async fn wait_and_click(
            &self,
            target: &str,
            _: Arc<AtomicBool>,
        ) -> Result<(), AmmoDriverError> {
            self.push(format!("click:{target}"));
            let failure = self.wait_click_failure.lock().unwrap().take();
            if let Some((failed_target, error)) = failure {
                if failed_target == target {
                    return Err(error);
                }
                *self.wait_click_failure.lock().unwrap() = Some((failed_target, error));
            }
            Ok(())
        }

        async fn click_unverified(
            &self,
            target: &str,
            _: Arc<AtomicBool>,
        ) -> Result<(), AmmoDriverError> {
            self.push(format!("unchecked:{target}"));
            Ok(())
        }

        async fn wait_target(
            &self,
            _: &[&str],
            _: Arc<AtomicBool>,
        ) -> Result<String, AmmoDriverError> {
            self.observations.lock().unwrap().pop_front().unwrap()
        }

        async fn position_and_click(
            &self,
            point: &RegionRect,
            scroll_steps: u32,
            _: Arc<AtomicBool>,
        ) -> Result<(), AmmoDriverError> {
            self.push(format!("position:{}:{}:{scroll_steps}", point.x, point.y));
            Ok(())
        }

        async fn delay(
            &self,
            duration: Duration,
            cancelled: Arc<AtomicBool>,
        ) -> Result<(), AmmoDriverError> {
            self.push(format!("delay:{}", duration.as_millis()));
            if self.cancel_on_delay.load(Ordering::SeqCst) {
                cancelled.store(true, Ordering::SeqCst);
                return Err(AmmoDriverError::Cancelled);
            }
            Ok(())
        }

        fn persist_success(&self, target_id: &str) -> Result<(), AmmoDriverError> {
            self.push(format!("success:{target_id}"));
            Ok(())
        }

        fn persist_failure(
            &self,
            target_id: &str,
            _: &str,
            _: &str,
        ) -> Result<(), AmmoDriverError> {
            self.push(format!("failure:{target_id}"));
            Ok(())
        }

        fn persist_isolated(
            &self,
            target_id: &str,
            _: &str,
            _: &str,
        ) -> Result<(), AmmoDriverError> {
            self.push(format!("isolated:{target_id}"));
            Ok(())
        }
    }

    fn target(id: &str, seasonal: bool, x: i32) -> AmmoRunTarget {
        AmmoRunTarget {
            id: id.to_string(),
            note: id.to_string(),
            seasonal,
            click_point: RegionRect {
                x,
                y: 20,
                width: 1,
                height: 1,
            },
            scroll_steps: 1,
            already_succeeded: false,
            retry_count: 0,
        }
    }

    fn entry_delays() -> AmmoEntryDelays {
        AmmoEntryDelays {
            supply_ms: 100,
            tactical_ms: 200,
        }
    }

    #[tokio::test]
    async fn all_succeeded_targets_are_skipped_without_input() {
        let driver = ScriptedDriver::new([]);
        let mut skipped = target("skip", false, 10);
        skipped.already_succeeded = true;

        let result = run_ammo_trial(
            &driver,
            &[skipped],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, AmmoRunStop::Completed);
        assert!(driver.actions().is_empty());
    }

    #[tokio::test]
    async fn skips_succeeded_and_runs_normal_before_seasonal() {
        let driver = ScriptedDriver::new(["ammo.success", "ammo.success"]);
        let mut skipped = target("skip", false, 10);
        skipped.already_succeeded = true;
        let result = run_ammo_trial(
            &driver,
            &[
                skipped,
                target("normal", false, 11),
                target("seasonal", true, 12),
            ],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, AmmoRunStop::Completed);
        let actions = driver.actions();
        assert!(!actions.iter().any(|action| action == "point:10:20"));
        assert!(
            actions
                .iter()
                .position(|action| action == "success:normal")
                .unwrap()
                < actions
                    .iter()
                    .position(|action| action == "unchecked:ammo.seasonal")
                    .unwrap()
        );
        assert_eq!(
            actions
                .iter()
                .filter(|action| *action == "unchecked:ammo.seasonal")
                .count(),
            1
        );
        assert!(actions.contains(&"success:seasonal".to_string()));
    }

    #[tokio::test]
    async fn fixed_entries_skip_list_templates_and_open_seasonal_once() {
        let driver = ScriptedDriver::new(["ammo.success", "ammo.success", "ammo.success"]);
        let result = run_ammo_trial(
            &driver,
            &[
                target("normal", false, 11),
                target("seasonal-a", true, 12),
                target("seasonal-b", true, 13),
            ],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, AmmoRunStop::Completed);
        let actions = driver.actions();
        assert_eq!(
            &actions[..5],
            [
                "click:ammo.department",
                "delay:100",
                "unchecked:ammo.supply",
                "delay:200",
                "unchecked:ammo.tactical",
            ]
        );
        assert!(!actions.iter().any(|action| {
            action.contains("ammo.list") || action.contains("ammo.seasonalList")
        }));
        assert_eq!(
            actions
                .iter()
                .filter(|action| *action == "unchecked:ammo.seasonal")
                .count(),
            1
        );
        assert!(
            actions
                .iter()
                .position(|action| action == "success:normal")
                .unwrap()
                < actions
                    .iter()
                    .position(|action| action == "unchecked:ammo.seasonal")
                    .unwrap()
        );
    }

    #[tokio::test]
    async fn positions_each_runnable_target_from_top_before_exchange() {
        let driver = ScriptedDriver::new(["ammo.success", "ammo.success"]);
        let mut first = target("first", false, 11);
        first.scroll_steps = 0;
        let mut second = target("second", false, 12);
        second.scroll_steps = 11;

        let result = run_ammo_trial(
            &driver,
            &[first, second],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, AmmoRunStop::Completed);
        let actions = driver.actions();
        assert!(actions
            .windows(2)
            .any(|items| items == ["position:11:20:0", "success:first"]));
        assert!(actions
            .windows(2)
            .any(|items| items == ["position:12:20:11", "success:second"]));
        assert!(!actions.iter().any(|item| item.starts_with("scroll:")));
    }

    #[tokio::test]
    async fn exchange_requires_confirm_before_success() {
        let driver = ScriptedDriver::new(["ammo.exchange", "ammo.success"]);
        let result = run_ammo_trial(
            &driver,
            &[target("normal", false, 11)],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, AmmoRunStop::Completed);
        let actions = driver.actions();
        let exchange = actions
            .iter()
            .position(|item| item == "click:ammo.exchange")
            .unwrap();
        let confirm = actions
            .iter()
            .position(|item| item == "click:ammo.confirm")
            .unwrap();
        let success = actions
            .iter()
            .position(|item| item == "success:normal")
            .unwrap();
        assert!(exchange < confirm && confirm < success);
    }

    #[tokio::test]
    async fn confirm_failure_marks_account_uncertain_and_stops_remaining_targets() {
        let driver = ScriptedDriver::new(["ammo.exchange"]);
        driver.fail_wait_and_click(
            "ammo.confirm",
            AmmoDriverError::Target("未识别到置顶确认按钮".to_string()),
        );
        let result = run_ammo_trial(
            &driver,
            &[target("first", false, 11), target("second", false, 12)],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert!(matches!(
            result.stop,
            AmmoRunStop::Uncertain { ref target_id, ref step, .. }
                if target_id == "first" && step == "ammo.confirm"
        ));
        assert!(!driver.actions().iter().any(|item| item == "point:12:20"));
        assert!(!driver
            .actions()
            .iter()
            .any(|item| item.starts_with("scroll:")));
    }

    #[tokio::test]
    async fn success_probe_failure_after_confirm_is_uncertain() {
        let driver = ScriptedDriver {
            observations: Mutex::new(VecDeque::from([
                Ok("ammo.exchange".to_string()),
                Err(AmmoDriverError::Target("兑换完成状态未命中".to_string())),
            ])),
            actions: Mutex::new(Vec::new()),
            cancel_on_delay: AtomicBool::new(false),
            wait_click_failure: Mutex::new(None),
        };
        let result = run_ammo_trial(
            &driver,
            &[target("normal", false, 11)],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert!(matches!(
            result.stop,
            AmmoRunStop::Uncertain { ref step, .. } if step == "ammo.success"
        ));
    }

    #[tokio::test]
    async fn confirmation_system_failure_is_not_downgraded_to_account_uncertain() {
        let driver = ScriptedDriver::new(["ammo.exchange"]);
        driver.fail_wait_and_click(
            "ammo.confirm",
            AmmoDriverError::System {
                step: "ammo.capture".to_string(),
                message: "目标窗口截图失败".to_string(),
            },
        );

        let result = run_ammo_trial(
            &driver,
            &[target("normal", false, 11)],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert!(matches!(
            result.stop,
            AmmoRunStop::SystemFailure { ref step, .. } if step == "ammo.capture"
        ));
    }

    #[tokio::test]
    async fn purchase_button_remaining_three_times_isolates_without_scroll() {
        let driver = ScriptedDriver::new([
            "ammo.fill",
            "ammo.purchase",
            "ammo.purchase",
            "ammo.purchase",
        ]);
        let result = run_ammo_trial(
            &driver,
            &[target("normal", false, 11)],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert!(matches!(result.stop, AmmoRunStop::Isolated { .. }));
        let actions = driver.actions();
        assert_eq!(
            actions
                .iter()
                .filter(|action| *action == "click:ammo.purchase")
                .count(),
            3
        );
        assert_eq!(
            actions
                .iter()
                .filter(|action| *action == "delay:1000")
                .count(),
            3
        );
        assert!(actions.contains(&"isolated:normal".to_string()));
        assert!(!actions.iter().any(|action| action.starts_with("scroll:")));
    }

    #[tokio::test]
    async fn purchase_button_twice_then_exchange_continues_same_target() {
        let driver = ScriptedDriver::new([
            "ammo.fill",
            "ammo.purchase",
            "ammo.purchase",
            "ammo.exchange",
            "ammo.success",
        ]);
        let result = run_ammo_trial(
            &driver,
            &[target("normal", false, 11)],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, AmmoRunStop::Completed);
        let actions = driver.actions();
        assert_eq!(
            actions
                .iter()
                .filter(|action| *action == "click:ammo.purchase")
                .count(),
            3
        );
        assert!(actions.contains(&"click:ammo.exchange".to_string()));
        assert!(actions.contains(&"success:normal".to_string()));
    }

    #[tokio::test]
    async fn purchase_feedback_without_stable_button_marks_target_uncertain() {
        let driver = ScriptedDriver::from_observations([
            Ok("ammo.fill".to_string()),
            Err(AmmoDriverError::Target("购买后未命中稳定按钮".to_string())),
        ]);
        let result = run_ammo_trial(
            &driver,
            &[target("normal", false, 11)],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert!(matches!(
            result.stop,
            AmmoRunStop::Uncertain { ref target_id, ref step, .. }
                if target_id == "normal" && step == "ammo.purchase"
        ));
    }

    #[tokio::test]
    async fn target_failure_is_persisted_and_next_target_runs() {
        let driver = ScriptedDriver {
            observations: Mutex::new(VecDeque::from([
                Err(AmmoDriverError::Target("兑换未成功".to_string())),
                Ok("ammo.success".to_string()),
            ])),
            actions: Mutex::new(Vec::new()),
            cancel_on_delay: AtomicBool::new(false),
            wait_click_failure: Mutex::new(None),
        };
        let result = run_ammo_trial(
            &driver,
            &[target("first", false, 11), target("second", false, 12)],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, AmmoRunStop::Completed);
        assert!(driver.actions().contains(&"failure:first".to_string()));
        assert!(driver.actions().contains(&"success:second".to_string()));
    }

    #[tokio::test]
    async fn cancellation_before_navigation_sends_no_input() {
        let driver = ScriptedDriver::new([]);
        let cancelled = Arc::new(AtomicBool::new(true));

        let result = run_ammo_trial(
            &driver,
            &[target("normal", false, 11)],
            entry_delays(),
            cancelled,
        )
        .await;

        assert_eq!(result.stop, AmmoRunStop::EmergencyStopped);
        assert!(driver.actions().is_empty());
    }

    #[tokio::test]
    async fn cancellation_during_supply_delay_stops_before_click() {
        let driver = ScriptedDriver::new([]);
        driver.cancel_on_delay.store(true, Ordering::SeqCst);

        let result = run_ammo_trial(
            &driver,
            &[target("normal", false, 11)],
            entry_delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, AmmoRunStop::EmergencyStopped);
        assert_eq!(driver.actions(), ["click:ammo.department", "delay:100"]);
    }
}
