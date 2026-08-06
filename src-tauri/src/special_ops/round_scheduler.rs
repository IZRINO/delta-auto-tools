use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::Notify;

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const CLOCK_JUMP_TOLERANCE_MS: i64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SchedulerAction {
    QueryProfit,
    LaunchRound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchedulerActionOutcome {
    Completed,
    RetryAfter(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScheduledAction {
    pub at_ms: i64,
    pub kind: SchedulerAction,
}

impl ScheduledAction {
    pub(crate) fn due(at_ms: i64, kind: SchedulerAction) -> Self {
        Self { at_ms, kind }
    }
}

pub(crate) fn choose_next_action(
    query_at_ms: Option<i64>,
    round_at_ms: Option<i64>,
) -> Option<ScheduledAction> {
    [
        query_at_ms.map(|at_ms| ScheduledAction::due(at_ms, SchedulerAction::QueryProfit)),
        round_at_ms.map(|at_ms| ScheduledAction::due(at_ms, SchedulerAction::LaunchRound)),
    ]
    .into_iter()
    .flatten()
    .min_by_key(|action| (action.at_ms, action.kind))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SchedulerPoll {
    pub now_ms: i64,
    pub next_action: Option<ScheduledAction>,
    pub active_run: bool,
    pub enabled: bool,
    pub paused: bool,
}

#[allow(async_fn_in_trait)]
pub(crate) trait SchedulerDriver: Send + Sync + 'static {
    fn poll(&self) -> Result<SchedulerPoll, String>;
    async fn execute_action(
        &self,
        action: SchedulerAction,
    ) -> Result<SchedulerActionOutcome, String>;
    fn pause_automation(&self, reason: &str) -> Result<(), String>;
}

#[derive(Default)]
pub(crate) struct RoundScheduler {
    notify: Notify,
    armed: AtomicBool,
    shutdown: AtomicBool,
    wake_generation: AtomicU64,
}

impl RoundScheduler {
    pub(crate) fn arm(&self) {
        self.armed.store(true, Ordering::SeqCst);
        self.wake_generation.fetch_add(1, Ordering::SeqCst);
        self.notify.notify_one();
    }

    pub(crate) fn resume(&self) {
        self.arm();
    }

    pub(crate) fn disarm(&self) {
        self.armed.store(false, Ordering::SeqCst);
        self.wake_generation.fetch_add(1, Ordering::SeqCst);
        self.notify.notify_one();
    }

    pub(crate) fn is_armed(&self) -> bool {
        self.armed.load(Ordering::SeqCst)
    }

    pub(crate) fn wake(&self) {
        self.wake_generation.fetch_add(1, Ordering::SeqCst);
        self.notify.notify_one();
    }

    pub(crate) fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.wake_generation.fetch_add(1, Ordering::SeqCst);
        self.notify.notify_one();
    }

    fn wake_generation(&self) -> u64 {
        self.wake_generation.load(Ordering::SeqCst)
    }
}

enum WaitResult {
    Notified,
    Timer,
}

async fn wait_for_wake(scheduler: &RoundScheduler, duration: Duration) -> WaitResult {
    tokio::select! {
        _ = scheduler.notify.notified() => WaitResult::Notified,
        _ = tokio::time::sleep(duration) => WaitResult::Timer,
    }
}

async fn wait_for_new_wake(
    scheduler: &RoundScheduler,
    duration: Duration,
    observed_generation: u64,
) -> WaitResult {
    let timeout = tokio::time::sleep(duration);
    tokio::pin!(timeout);
    loop {
        if scheduler.wake_generation() != observed_generation {
            return WaitResult::Notified;
        }
        tokio::select! {
            _ = scheduler.notify.notified() => {}
            _ = &mut timeout => return WaitResult::Timer,
        }
    }
}

fn wait_duration(poll: SchedulerPoll) -> Duration {
    if poll.active_run {
        return POLL_INTERVAL;
    }
    let until_due = poll
        .next_action
        .map(|action| action.at_ms.saturating_sub(poll.now_ms).max(0) as u64)
        .map(Duration::from_millis)
        .unwrap_or(POLL_INTERVAL);
    until_due.min(POLL_INTERVAL)
}

pub(crate) async fn run_scheduler<D: SchedulerDriver>(
    scheduler: Arc<RoundScheduler>,
    driver: Arc<D>,
) {
    while !scheduler.shutdown.load(Ordering::SeqCst) {
        if !scheduler.is_armed() {
            scheduler.notify.notified().await;
            continue;
        }

        let poll = match driver.poll() {
            Ok(poll) => poll,
            Err(error) => {
                scheduler.disarm();
                let _ = driver.pause_automation(&format!("scheduler 读取失败：{error}"));
                continue;
            }
        };
        if !poll.enabled || poll.paused {
            scheduler.disarm();
            continue;
        }
        if !poll.active_run
            && poll
                .next_action
                .is_some_and(|action| action.at_ms <= poll.now_ms)
        {
            let action = poll.next_action.expect("已确认存在到期 scheduler action");
            let wake_generation = scheduler.wake_generation();
            match driver.execute_action(action.kind).await {
                Ok(SchedulerActionOutcome::Completed) => {}
                Ok(SchedulerActionOutcome::RetryAfter(delay)) => {
                    let _ = wait_for_new_wake(&scheduler, delay, wake_generation).await;
                }
                Err(error) => {
                    scheduler.disarm();
                    let _ = driver.pause_automation(&format!(
                        "scheduler 执行动作 {:?} 失败：{error}",
                        action.kind
                    ));
                }
            }
            continue;
        }

        let duration = wait_duration(poll);
        let expected_wake_at_ms = poll
            .now_ms
            .saturating_add(duration.as_millis().min(i64::MAX as u128) as i64);
        if matches!(wait_for_wake(&scheduler, duration).await, WaitResult::Timer) {
            let late_by_ms = driver
                .poll()
                .map(|next| next.now_ms.saturating_sub(expected_wake_at_ms))
                .unwrap_or(CLOCK_JUMP_TOLERANCE_MS + 1);
            if late_by_ms > CLOCK_JUMP_TOLERANCE_MS {
                scheduler.disarm();
                let _ = driver.pause_automation("检测到休眠或系统时间跳变");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
        Arc,
    };

    struct FakeDriver {
        now_ms: AtomicI64,
        due_at_ms: AtomicI64,
        active: AtomicBool,
        launches: AtomicUsize,
        pauses: AtomicUsize,
    }

    impl FakeDriver {
        fn due_now() -> Arc<Self> {
            Arc::new(Self {
                now_ms: AtomicI64::new(1_000),
                due_at_ms: AtomicI64::new(1_000),
                active: AtomicBool::new(false),
                launches: AtomicUsize::new(0),
                pauses: AtomicUsize::new(0),
            })
        }
    }

    struct RetryDriver {
        attempts: AtomicUsize,
        pauses: AtomicUsize,
        active: AtomicBool,
    }

    impl RetryDriver {
        fn due_now() -> Arc<Self> {
            Arc::new(Self {
                attempts: AtomicUsize::new(0),
                pauses: AtomicUsize::new(0),
                active: AtomicBool::new(false),
            })
        }
    }

    impl SchedulerDriver for RetryDriver {
        fn poll(&self) -> Result<SchedulerPoll, String> {
            Ok(SchedulerPoll {
                now_ms: 1_000,
                next_action: Some(ScheduledAction::due(1_000, SchedulerAction::LaunchRound)),
                active_run: self.active.load(Ordering::SeqCst),
                enabled: true,
                paused: false,
            })
        }

        async fn execute_action(
            &self,
            action: SchedulerAction,
        ) -> Result<SchedulerActionOutcome, String> {
            assert_eq!(action, SchedulerAction::LaunchRound);
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                return Ok(SchedulerActionOutcome::RetryAfter(Duration::from_secs(1)));
            }
            self.active.store(true, Ordering::SeqCst);
            Ok(SchedulerActionOutcome::Completed)
        }

        fn pause_automation(&self, _reason: &str) -> Result<(), String> {
            self.pauses.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    impl SchedulerDriver for FakeDriver {
        fn poll(&self) -> Result<SchedulerPoll, String> {
            Ok(SchedulerPoll {
                now_ms: self.now_ms.load(Ordering::SeqCst),
                next_action: Some(ScheduledAction::due(
                    self.due_at_ms.load(Ordering::SeqCst),
                    SchedulerAction::LaunchRound,
                )),
                active_run: self.active.load(Ordering::SeqCst),
                enabled: true,
                paused: false,
            })
        }

        async fn execute_action(
            &self,
            action: SchedulerAction,
        ) -> Result<SchedulerActionOutcome, String> {
            assert_eq!(action, SchedulerAction::LaunchRound);
            self.launches.fetch_add(1, Ordering::SeqCst);
            self.active.store(true, Ordering::SeqCst);
            Ok(SchedulerActionOutcome::Completed)
        }

        fn pause_automation(&self, _reason: &str) -> Result<(), String> {
            self.pauses.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    async fn settle() {
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
    }

    #[test]
    fn active_round_with_due_work_waits_for_poll_interval() {
        let duration = wait_duration(SchedulerPoll {
            now_ms: 1_000,
            next_action: Some(ScheduledAction::due(1_000, SchedulerAction::LaunchRound)),
            active_run: true,
            enabled: true,
            paused: false,
        });

        assert_eq!(duration, POLL_INTERVAL);
    }

    #[test]
    fn query_action_wins_equal_due_time_and_future_action_uses_earliest_time() {
        assert_eq!(
            choose_next_action(Some(2_000), Some(2_000)),
            Some(ScheduledAction::due(2_000, SchedulerAction::QueryProfit))
        );
        assert_eq!(
            choose_next_action(Some(3_000), Some(2_000)),
            Some(ScheduledAction::due(2_000, SchedulerAction::LaunchRound))
        );
        assert_eq!(choose_next_action(None, None), None);
    }

    #[tokio::test(start_paused = true)]
    async fn resume_launches_due_round_immediately() {
        let scheduler = Arc::new(RoundScheduler::default());
        let driver = FakeDriver::due_now();
        let worker = tokio::spawn(run_scheduler(Arc::clone(&scheduler), Arc::clone(&driver)));

        scheduler.resume();
        settle().await;

        assert_eq!(driver.launches.load(Ordering::SeqCst), 1);
        scheduler.shutdown();
        worker.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn resume_waits_until_future_task_is_due() {
        let scheduler = Arc::new(RoundScheduler::default());
        let driver = FakeDriver::due_now();
        driver.due_at_ms.store(31_000, Ordering::SeqCst);
        let worker = tokio::spawn(run_scheduler(Arc::clone(&scheduler), Arc::clone(&driver)));

        scheduler.resume();
        settle().await;
        assert_eq!(driver.launches.load(Ordering::SeqCst), 0);

        driver.now_ms.store(31_000, Ordering::SeqCst);
        tokio::time::advance(std::time::Duration::from_secs(30)).await;
        settle().await;

        assert_eq!(driver.launches.load(Ordering::SeqCst), 1);
        scheduler.shutdown();
        worker.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn retryable_action_keeps_scheduler_armed() {
        let scheduler = Arc::new(RoundScheduler::default());
        let driver = RetryDriver::due_now();
        let worker = tokio::spawn(run_scheduler(Arc::clone(&scheduler), Arc::clone(&driver)));

        scheduler.resume();
        settle().await;

        assert_eq!(driver.attempts.load(Ordering::SeqCst), 1);
        assert_eq!(driver.pauses.load(Ordering::SeqCst), 0);
        assert!(scheduler.is_armed());

        tokio::time::advance(Duration::from_secs(1)).await;
        settle().await;

        assert_eq!(driver.attempts.load(Ordering::SeqCst), 2);
        assert_eq!(driver.pauses.load(Ordering::SeqCst), 0);
        assert!(scheduler.is_armed());
        scheduler.shutdown();
        worker.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn active_run_blocks_duplicate_launch() {
        let scheduler = Arc::new(RoundScheduler::default());
        let driver = FakeDriver::due_now();
        driver.active.store(true, Ordering::SeqCst);
        let worker = tokio::spawn(run_scheduler(Arc::clone(&scheduler), Arc::clone(&driver)));

        scheduler.arm();
        settle().await;
        assert_eq!(driver.launches.load(Ordering::SeqCst), 0);
        scheduler.shutdown();
        worker.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn late_timer_pauses_instead_of_launching() {
        let scheduler = Arc::new(RoundScheduler::default());
        let driver = FakeDriver::due_now();
        driver.due_at_ms.store(31_000, Ordering::SeqCst);
        let worker = tokio::spawn(run_scheduler(Arc::clone(&scheduler), Arc::clone(&driver)));

        scheduler.arm();
        settle().await;
        driver.now_ms.store(100_000, Ordering::SeqCst);
        tokio::time::advance(std::time::Duration::from_secs(30)).await;
        settle().await;

        assert_eq!(driver.pauses.load(Ordering::SeqCst), 1);
        assert_eq!(driver.launches.load(Ordering::SeqCst), 0);
        assert!(!scheduler.is_armed());
        scheduler.shutdown();
        worker.await.unwrap();
    }

    struct ActionDriver {
        now_ms: AtomicI64,
        actions: std::sync::Mutex<Vec<ScheduledAction>>,
        executed: std::sync::Mutex<Vec<SchedulerAction>>,
        pauses: AtomicUsize,
    }

    impl ActionDriver {
        fn with_actions(actions: impl IntoIterator<Item = ScheduledAction>) -> Arc<Self> {
            let mut actions = actions.into_iter().collect::<Vec<_>>();
            actions.sort_by_key(|action| (action.at_ms, action.kind));
            Arc::new(Self {
                now_ms: AtomicI64::new(1_000),
                actions: std::sync::Mutex::new(actions),
                executed: std::sync::Mutex::new(Vec::new()),
                pauses: AtomicUsize::new(0),
            })
        }

        fn executed(&self) -> Vec<SchedulerAction> {
            self.executed.lock().unwrap().clone()
        }
    }

    impl SchedulerDriver for ActionDriver {
        fn poll(&self) -> Result<SchedulerPoll, String> {
            Ok(SchedulerPoll {
                now_ms: self.now_ms.load(Ordering::SeqCst),
                next_action: self.actions.lock().unwrap().first().copied(),
                active_run: false,
                enabled: true,
                paused: false,
            })
        }

        async fn execute_action(
            &self,
            action: SchedulerAction,
        ) -> Result<SchedulerActionOutcome, String> {
            let next = self.actions.lock().unwrap().remove(0);
            assert_eq!(next.kind, action);
            self.executed.lock().unwrap().push(action);
            Ok(SchedulerActionOutcome::Completed)
        }

        fn pause_automation(&self, _reason: &str) -> Result<(), String> {
            self.pauses.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn query_wins_tie_then_round_launches_after_query() {
        let scheduler = Arc::new(RoundScheduler::default());
        let driver = ActionDriver::with_actions([
            ScheduledAction::due(1_000, SchedulerAction::LaunchRound),
            ScheduledAction::due(1_000, SchedulerAction::QueryProfit),
        ]);
        let worker = tokio::spawn(run_scheduler(Arc::clone(&scheduler), Arc::clone(&driver)));

        scheduler.resume();
        settle().await;

        assert_eq!(
            driver.executed(),
            [SchedulerAction::QueryProfit, SchedulerAction::LaunchRound]
        );
        assert_eq!(driver.pauses.load(Ordering::SeqCst), 0);
        scheduler.shutdown();
        worker.await.unwrap();
    }
}
