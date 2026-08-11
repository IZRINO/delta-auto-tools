use super::{
    round_planner::{can_chain_follow_up, should_continue_round, AccountRoundTask, RoundPlan},
    StationKind,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrorScope {
    Account,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountRunErrorKind {
    Regular,
    NavigationTimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountRunError {
    pub scope: ErrorScope,
    pub kind: AccountRunErrorKind,
    pub station: Option<StationKind>,
    pub ammo_target_id: Option<String>,
    pub step: String,
    pub message: String,
}

impl AccountRunError {
    pub(crate) fn account(step: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            scope: ErrorScope::Account,
            kind: AccountRunErrorKind::Regular,
            station: None,
            ammo_target_id: None,
            step: step.into(),
            message: message.into(),
        }
    }

    pub(crate) fn system(step: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            scope: ErrorScope::System,
            kind: AccountRunErrorKind::Regular,
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
            kind: AccountRunErrorKind::Regular,
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
            kind: AccountRunErrorKind::Regular,
            station: None,
            ammo_target_id: Some(target_id.into()),
            step: step.into(),
            message: message.into(),
        }
    }

    pub(crate) fn navigation_timeout(step: impl Into<String>) -> Self {
        Self::navigation_timeout_with_message(step, "步骤超时")
    }

    pub(crate) fn navigation_timeout_with_message(
        step: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            scope: ErrorScope::Account,
            kind: AccountRunErrorKind::NavigationTimedOut,
            station: None,
            ammo_target_id: None,
            step: step.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct AccountRunSuccess {
    pub processed_stations: usize,
    pub limited_retry_requested: bool,
    pub market_pending: bool,
    pub market_yielded: bool,
}

impl AccountRunSuccess {
    #[cfg(test)]
    fn processed(processed_stations: usize) -> Self {
        Self {
            processed_stations,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RoundStop {
    Completed,
    PauseRequested,
    PauseRequestedPreservingGame,
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
    async fn continue_account(
        &self,
        index: usize,
        total: usize,
        task: &AccountRoundTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<AccountRunSuccess, AccountRunError>;
    async fn wait_until(
        &self,
        index: usize,
        total: usize,
        task: &AccountRoundTask,
        keep_session: bool,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), AccountRunError>;
    fn now_ms(&self) -> i64;
    fn persist_account_failure(
        &self,
        task: &AccountRoundTask,
        error: &AccountRunError,
    ) -> Result<(), String>;
    fn persist_limited_failure(&self, task: &AccountRoundTask, message: &str)
        -> Result<(), String>;
    fn market_window_open(&self) -> bool;
    fn refresh_due_craft_tasks(&self) -> Result<Vec<AccountRoundTask>, String>;
    async fn close_game(&self) -> Result<(), String>;
    fn report_close_game_failure(&self, reason: &str, message: &str);
    /// 导航超时把账号整体挪到队尾时上报。没有这条记录时，日志里
    /// `账号游戏内导航结束 success:false` 之后是一段完全的空白，
    /// 无法区分「重排队了但下一账号卡住」和「压根没走到重排队」。
    fn report_navigation_retry_deferred(&self, task: &AccountRoundTask, deferred_tasks: usize);
    fn pause_requested(&self) -> Result<bool, String>;
    fn pause_preserves_game(&self) -> bool;
    fn persist_paused(&self, reason: &str) -> Result<(), String>;
}

#[derive(Clone)]
struct QueuedRoundTask {
    original_index: usize,
    task: AccountRoundTask,
    navigation_retries: u8,
    business_retries: u8,
}

fn limited_retry_task(queued: &QueuedRoundTask) -> QueuedRoundTask {
    let mut retry = queued.clone();
    retry.task.stations.clear();
    retry.task.ammo_target_ids.clear();
    retry.task.market_purchase_day = None;
    retry.business_retries = retry.business_retries.saturating_add(1);
    retry
}

fn market_retry_task(queued: &QueuedRoundTask) -> QueuedRoundTask {
    let mut retry = queued.clone();
    retry.task.stations.clear();
    retry.task.ammo_target_ids.clear();
    retry.task.limited_supply_cycle_id = None;
    retry
}

fn persist_system_failure<D: RoundDriver + ?Sized>(
    driver: &D,
    step: impl Into<String>,
    message: String,
) -> RoundStop {
    let message = match driver.persist_paused(&message) {
        Ok(()) => message,
        Err(persist_error) => format!("{message}；暂停状态保存失败：{persist_error}"),
    };
    RoundStop::SystemFailure {
        step: step.into(),
        message,
    }
}

/// 转场关闭游戏的兜底预算。`close_game` 自身已按 `ROUND_CLOSE_GAME_TIMEOUT` 限时，
/// 这里再包一层是防「future 根本不 resolve」：blocking 线程池饥饿、底层 Win32 调用
/// 卡在内核态时，前者的 deadline 检查压根不会被执行到。转场关闭只是清场，卡住它
/// 就把整轮队列一起卡死（账号不重排、下一账号不登录、日志全无），代价远高于
/// 少杀一次进程。
const TRANSITION_CLOSE_GAME_BUDGET: Duration = Duration::from_secs(60);

/// 轮次切换关闭游戏是清场，不是正确性前提：登录流程头两步 StopGame / StopWeGame 会用
/// 各自预算无条件重杀两个 exe -> 残留进程下轮自愈。这里全局暂停会把一次慢退出变成
/// 停摆到人工点继续，代价远高于收益，因此只报告不中断。
async fn close_game_for_transition<D: RoundDriver + ?Sized>(driver: &D, reason: &str) {
    match tokio::time::timeout(TRANSITION_CLOSE_GAME_BUDGET, driver.close_game()).await {
        Ok(Ok(())) => {}
        Ok(Err(message)) => driver.report_close_game_failure(reason, &message),
        Err(_) => driver.report_close_game_failure(reason, "关闭游戏未在兜底预算内返回"),
    }
}

async fn stop_for_pause<D: RoundDriver + ?Sized>(driver: &D) -> RoundStop {
    let preserve_game = driver.pause_preserves_game();
    let reason = if preserve_game {
        "检测到系统暂停"
    } else {
        "用户请求暂停"
    };
    if let Err(message) = driver.persist_paused(reason) {
        return RoundStop::SystemFailure {
            step: "round.persistPause".to_string(),
            message,
        };
    }
    if preserve_game {
        return RoundStop::PauseRequestedPreservingGame;
    }
    match driver.close_game().await {
        Ok(()) => RoundStop::PauseRequested,
        Err(message) => RoundStop::SystemFailure {
            step: "round.closeGame".to_string(),
            message,
        },
    }
}

pub(crate) async fn run_round<D: RoundDriver + ?Sized>(
    driver: &D,
    plan: &RoundPlan,
    cancelled: Arc<AtomicBool>,
) -> RoundRunResult {
    let mut completed_account_ids = HashSet::new();
    let mut ordered_accounts = plan
        .accounts
        .iter()
        .map(|task| (task.account_order, task.account_id.clone()))
        .collect::<Vec<_>>();
    ordered_accounts.sort_unstable();
    ordered_accounts.dedup_by(|left, right| left.1 == right.1);
    let account_positions = ordered_accounts
        .iter()
        .enumerate()
        .map(|(index, (_, account_id))| (account_id.clone(), index + 1))
        .collect::<HashMap<_, _>>();
    let total = account_positions.len();
    let mut queue = plan
        .accounts
        .iter()
        .enumerate()
        .map(|(index, task)| QueuedRoundTask {
            original_index: index,
            task: task.clone(),
            navigation_retries: 0,
            business_retries: 0,
        })
        .collect::<VecDeque<_>>();
    if !driver.market_window_open() {
        queue.retain(|queued| queued.task.market_purchase_day.is_none());
    }
    let mut session_account_id: Option<String> = None;
    while let Some(mut queued) = queue.pop_front() {
        let task = &queued.task;
        let index = queued.original_index;
        let account_index = account_positions
            .get(&task.account_id)
            .copied()
            .unwrap_or(index + 1);
        if cancelled.load(Ordering::SeqCst) {
            return RoundRunResult {
                completed_accounts: completed_account_ids.len(),
                stop: RoundStop::EmergencyStopped,
            };
        }
        let continuing = session_account_id.as_deref() == Some(task.account_id.as_str());
        let waited = task.scheduled_at_ms > driver.now_ms();
        if waited {
            if let Err(error) = driver
                .wait_until(
                    account_index,
                    total,
                    task,
                    continuing,
                    Arc::clone(&cancelled),
                )
                .await
            {
                return RoundRunResult {
                    completed_accounts: completed_account_ids.len(),
                    stop: persist_system_failure(driver, error.step, error.message),
                };
            }
        }
        if cancelled.load(Ordering::SeqCst) {
            return RoundRunResult {
                completed_accounts: completed_account_ids.len(),
                stop: RoundStop::EmergencyStopped,
            };
        }
        if waited {
            match driver.pause_requested() {
                Ok(true) => {
                    let stop = stop_for_pause(driver).await;
                    return RoundRunResult {
                        completed_accounts: completed_account_ids.len(),
                        stop,
                    };
                }
                Ok(false) => {}
                Err(message) => {
                    return RoundRunResult {
                        completed_accounts: completed_account_ids.len(),
                        stop: persist_system_failure(driver, "round.pauseRequested", message),
                    };
                }
            }
        }

        let run_result = if continuing {
            driver
                .continue_account(account_index, total, task, Arc::clone(&cancelled))
                .await
        } else {
            driver
                .run_account(account_index, total, task, Arc::clone(&cancelled))
                .await
        };
        let mut success = None;
        match run_result {
            Ok(result) => {
                completed_account_ids.insert(task.account_id.clone());
                session_account_id = Some(task.account_id.clone());
                success = Some(result);
            }
            Err(error)
                if error.scope == ErrorScope::Account
                    && error.kind == AccountRunErrorKind::NavigationTimedOut
                    && queued.navigation_retries == 0 =>
            {
                close_game_for_transition(driver, "导航超时后关闭游戏失败").await;
                session_account_id = None;
                let mut retained = VecDeque::new();
                queued.navigation_retries = 1;
                let account_id = queued.task.account_id.clone();
                let mut deferred = VecDeque::from([queued.clone()]);
                while let Some(mut candidate) = queue.pop_front() {
                    if candidate.task.account_id == account_id {
                        candidate.navigation_retries = 1;
                        deferred.push_back(candidate);
                    } else {
                        retained.push_back(candidate);
                    }
                }
                driver.report_navigation_retry_deferred(&queued.task, deferred.len());
                retained.extend(deferred);
                queue = retained;
            }
            Err(error) if error.scope == ErrorScope::Account => {
                if let Err(message) = driver.persist_account_failure(task, &error) {
                    let _ = driver.persist_paused("账号失败状态保存失败");
                    return RoundRunResult {
                        completed_accounts: completed_account_ids.len(),
                        stop: RoundStop::SystemFailure {
                            step: "round.persistAccountFailure".to_string(),
                            message,
                        },
                    };
                }
                let failed_account_id = task.account_id.clone();
                queue.retain(|queued| queued.task.account_id != failed_account_id);
                close_game_for_transition(driver, "账号失败后关闭游戏失败").await;
                session_account_id = None;
            }
            Err(error) => {
                return RoundRunResult {
                    completed_accounts: completed_account_ids.len(),
                    stop: persist_system_failure(driver, error.step, error.message),
                };
            }
        }

        if cancelled.load(Ordering::SeqCst) {
            return RoundRunResult {
                completed_accounts: completed_account_ids.len(),
                stop: RoundStop::EmergencyStopped,
            };
        }
        match driver.pause_requested() {
            Ok(true) => {
                let stop = stop_for_pause(driver).await;
                return RoundRunResult {
                    completed_accounts: completed_account_ids.len(),
                    stop,
                };
            }
            Ok(false) => {}
            Err(message) => {
                let _ = driver.persist_paused("暂停请求读取失败");
                return RoundRunResult {
                    completed_accounts: completed_account_ids.len(),
                    stop: RoundStop::SystemFailure {
                        step: "round.pauseRequested".to_string(),
                        message,
                    },
                };
            }
        }

        if let Some(success) = success {
            let now_ms = driver.now_ms();
            let force_new_session = success.limited_retry_requested || success.market_yielded;
            if success.limited_retry_requested {
                if queued.business_retries == 0 {
                    // 补偿重试必须排队首：退出判定只看 queue.front()，排到队尾会在
                    // front 是远期任务时随 break 一起丢弃，导致本次检查永不落终态。
                    queue.push_front(limited_retry_task(&queued));
                } else if let Err(message) =
                    driver.persist_limited_failure(&queued.task, "研发部门页面补偿重试后仍未就绪")
                {
                    return RoundRunResult {
                        completed_accounts: completed_account_ids.len(),
                        stop: persist_system_failure(
                            driver,
                            "round.persistLimitedFailure",
                            message,
                        ),
                    };
                }
            }
            if success.market_yielded && success.market_pending {
                let refreshed = match driver.refresh_due_craft_tasks() {
                    Ok(tasks) => tasks,
                    Err(message) => {
                        return RoundRunResult {
                            completed_accounts: completed_account_ids.len(),
                            stop: persist_system_failure(driver, "round.refreshDueCraft", message),
                        };
                    }
                };
                for task in refreshed {
                    let duplicate = queue.iter().any(|candidate| {
                        candidate.task.account_id == task.account_id
                            && candidate.task.scheduled_at_ms == task.scheduled_at_ms
                            && candidate.task.stations == task.stations
                    });
                    if !duplicate {
                        queue.push_back(QueuedRoundTask {
                            original_index: queue.len(),
                            task,
                            navigation_retries: 0,
                            business_retries: 0,
                        });
                    }
                }
                let market = market_retry_task(&queued);
                let insert_at = queue
                    .iter()
                    .rposition(|candidate| {
                        !candidate.task.stations.is_empty()
                            && candidate.task.scheduled_at_ms <= now_ms
                    })
                    .map_or(0, |position| position + 1);
                queue.insert(insert_at, market);
            }
            if !driver.market_window_open() {
                queue.retain(|queued| queued.task.market_purchase_day.is_none());
            }
            let continue_round = queue
                .front()
                .is_some_and(|next| should_continue_round(task, &next.task, now_ms));
            let keep_session = queue
                .front()
                .is_some_and(|next| can_chain_follow_up(task, &next.task, now_ms))
                && !force_new_session;
            if !keep_session {
                close_game_for_transition(driver, "会话结束关闭游戏失败").await;
                session_account_id = None;
            }
            if !continue_round {
                break;
            }
        }
    }
    RoundRunResult {
        completed_accounts: completed_account_ids.len(),
        stop: RoundStop::Completed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_ops::{
        round_planner::{AccountRoundTask, RoundPlan, RoundTrigger},
        StationKind,
    };
    use std::sync::{
        atomic::{AtomicBool, AtomicI64, AtomicUsize},
        Arc, Mutex,
    };

    struct FakeDriver {
        results: Mutex<Vec<Result<AccountRunSuccess, AccountRunError>>>,
        actions: Mutex<Vec<String>>,
        pause_requested: bool,
        pause_preserves_game: bool,
        close_result: Mutex<Option<Result<(), String>>>,
        now_ms: AtomicI64,
        account_runs: AtomicUsize,
        market_window_closes_after_runs: AtomicUsize,
        refreshed_tasks: Mutex<Option<Result<Vec<AccountRoundTask>, String>>>,
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
                pause_preserves_game: false,
                close_result: Mutex::new(Some(Ok(()))),
                now_ms: AtomicI64::new(1_000),
                account_runs: AtomicUsize::new(0),
                market_window_closes_after_runs: AtomicUsize::new(usize::MAX),
                refreshed_tasks: Mutex::new(None),
            }
        }

        fn with_close_failure(self, message: &str) -> Self {
            *self.close_result.lock().unwrap() = Some(Err(message.to_string()));
            self
        }

        fn with_now_ms(self, now_ms: i64) -> Self {
            self.now_ms.store(now_ms, Ordering::SeqCst);
            self
        }

        fn preserving_game_on_pause(mut self) -> Self {
            self.pause_preserves_game = true;
            self
        }

        fn with_market_window_closing_after_runs(self, runs: usize) -> Self {
            self.market_window_closes_after_runs
                .store(runs, Ordering::SeqCst);
            self
        }

        fn with_refresh_result(self, result: Result<Vec<AccountRoundTask>, String>) -> Self {
            *self.refreshed_tasks.lock().unwrap() = Some(result);
            self
        }

        fn record_account_run(&self, action: String) {
            self.actions.lock().unwrap().push(action);
            self.account_runs.fetch_add(1, Ordering::SeqCst);
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
            self.record_account_run(format!("run:{}", task.account_id));
            self.results.lock().unwrap().pop().unwrap()
        }

        async fn continue_account(
            &self,
            _index: usize,
            _total: usize,
            task: &AccountRoundTask,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<AccountRunSuccess, AccountRunError> {
            self.record_account_run(format!("continue:{}", task.account_id));
            self.results.lock().unwrap().pop().unwrap()
        }

        async fn wait_until(
            &self,
            _index: usize,
            _total: usize,
            task: &AccountRoundTask,
            _keep_session: bool,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), AccountRunError> {
            let scheduled_at_ms = task.scheduled_at_ms;
            self.actions
                .lock()
                .unwrap()
                .push(format!("wait:{scheduled_at_ms}"));
            self.now_ms.store(scheduled_at_ms, Ordering::SeqCst);
            Ok(())
        }

        fn now_ms(&self) -> i64 {
            self.now_ms.load(Ordering::SeqCst)
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

        fn persist_limited_failure(
            &self,
            task: &AccountRoundTask,
            _message: &str,
        ) -> Result<(), String> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("persist-limited:{}", task.account_id));
            Ok(())
        }

        fn market_window_open(&self) -> bool {
            self.account_runs.load(Ordering::SeqCst)
                < self.market_window_closes_after_runs.load(Ordering::SeqCst)
        }

        fn refresh_due_craft_tasks(&self) -> Result<Vec<AccountRoundTask>, String> {
            self.refreshed_tasks
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Ok(Vec::new()))
        }

        async fn close_game(&self) -> Result<(), String> {
            self.perform_close_game().await
        }

        fn report_close_game_failure(&self, _reason: &str, message: &str) {
            self.actions
                .lock()
                .unwrap()
                .push(format!("report-close-failure:{message}"));
        }

        fn report_navigation_retry_deferred(&self, task: &AccountRoundTask, deferred_tasks: usize) {
            self.actions.lock().unwrap().push(format!(
                "defer-navigation:{}:{}",
                task.account_id, deferred_tasks
            ));
        }

        fn pause_requested(&self) -> Result<bool, String> {
            Ok(self.pause_requested)
        }

        fn pause_preserves_game(&self) -> bool {
            self.pause_preserves_game
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
                    scheduled_at_ms: 0,
                    stations: vec![StationKind::TechnicalCenter],
                    ammo_target_ids: Vec::new(),
                    limited_supply_cycle_id: None,
                    market_purchase_day: None,
                })
                .collect(),
        }
    }

    fn plan_tasks(tasks: &[(&str, i64)]) -> RoundPlan {
        RoundPlan {
            created_at_ms: 1_000,
            trigger: RoundTrigger::Scheduled,
            accounts: tasks
                .iter()
                .enumerate()
                .map(|(order, (id, scheduled_at_ms))| AccountRoundTask {
                    account_id: (*id).to_string(),
                    qq_account: format!("100{order}"),
                    account_order: order as u32,
                    scheduled_at_ms: *scheduled_at_ms,
                    stations: vec![StationKind::TechnicalCenter],
                    ammo_target_ids: Vec::new(),
                    limited_supply_cycle_id: None,
                    market_purchase_day: None,
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn same_account_follow_up_waits_and_reuses_session() {
        let driver = FakeDriver::new(
            vec![
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::processed(1)),
            ],
            false,
        );

        let result = run_round(
            &driver,
            &plan_tasks(&[("a", 1_000), ("a", 5_000)]),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            ["run:a", "wait:5000", "continue:a", "close-game"]
        );
    }

    #[tokio::test]
    async fn intervening_account_runs_before_same_account_follow_ups() {
        let driver = FakeDriver::new(
            (0..4)
                .map(|_| Ok(AccountRunSuccess::processed(1)))
                .collect(),
            false,
        );

        let result = run_round(
            &driver,
            &plan_tasks(&[("a", 1_000), ("b", 2_000), ("a", 4_000), ("a", 8_000)]),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "close-game",
                "wait:2000",
                "run:b",
                "close-game",
                "wait:4000",
                "run:a",
                "wait:8000",
                "continue:a",
                "close-game",
            ]
        );
    }

    #[tokio::test]
    async fn first_navigation_timeout_defers_all_tasks_for_account() {
        let driver = FakeDriver::new(
            vec![
                Err(AccountRunError::navigation_timeout(
                    "navigation.WaitStationGrid",
                )),
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::processed(1)),
            ],
            false,
        );

        let result = run_round(
            &driver,
            &plan_tasks(&[("a", 1_000), ("b", 2_000), ("a", 4_000)]),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "close-game",
                "defer-navigation:a:2",
                "wait:2000",
                "run:b",
                "close-game",
                "run:a",
                "wait:4000",
                "continue:a",
                "close-game",
            ]
        );
    }

    #[tokio::test]
    async fn navigation_timeout_close_failure_still_defers_account() {
        let driver = FakeDriver::new(
            vec![
                Err(AccountRunError::navigation_timeout(
                    "navigation.WaitStationGrid",
                )),
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::processed(1)),
            ],
            false,
        )
        .with_close_failure("WeGame 仍在运行");

        let result = run_round(
            &driver,
            &plan_tasks(&[("a", 1_000), ("b", 2_000), ("a", 4_000)]),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "close-game",
                "report-close-failure:WeGame 仍在运行",
                "defer-navigation:a:2",
                "wait:2000",
                "run:b",
                "close-game",
                "run:a",
                "wait:4000",
                "continue:a",
                "close-game",
            ]
        );
    }

    #[tokio::test]
    async fn limited_ready_timeout_retries_once_then_persists_failure() {
        let retry = AccountRunSuccess {
            limited_retry_requested: true,
            ..AccountRunSuccess::default()
        };
        let driver = FakeDriver::new(vec![Ok(retry.clone()), Ok(retry)], false);
        let mut plan = plan_tasks(&[("a", 1_000)]);
        plan.accounts[0].stations.clear();
        plan.accounts[0].limited_supply_cycle_id = Some("2026-08-08T12:00".to_string());

        let result = run_round(&driver, &plan, Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "close-game",
                "run:a",
                "persist-limited:a",
                "close-game",
            ]
        );
    }

    #[tokio::test]
    async fn limited_ready_timeout_retry_survives_far_future_queue_front() {
        let retry = AccountRunSuccess {
            limited_retry_requested: true,
            ..AccountRunSuccess::default()
        };
        let driver = FakeDriver::new(vec![Ok(retry.clone()), Ok(retry)], false);
        // 队列里跟着一个远超会话串联窗口的制作任务：补偿重试排到队尾时
        // 退出判定只看 front（远期任务）→ 直接 break，重试连同终态标记一起丢掉。
        let mut plan = plan_tasks(&[("a", 1_000), ("b", 5_000_000)]);
        plan.accounts[0].stations.clear();
        plan.accounts[0].limited_supply_cycle_id = Some("2026-08-08T12:00".to_string());

        let result = run_round(&driver, &plan, Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "close-game",
                "run:a",
                "persist-limited:a",
                "close-game",
            ]
        );
    }

    #[tokio::test]
    async fn yielded_market_runs_due_craft_before_resuming_market() {
        let yielded = AccountRunSuccess {
            market_pending: true,
            market_yielded: true,
            ..AccountRunSuccess::default()
        };
        let driver = FakeDriver::new(
            vec![
                Ok(yielded),
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::default()),
            ],
            false,
        )
        .with_now_ms(2_000);
        let mut plan = plan_tasks(&[("market", 1_000), ("craft", 1_500)]);
        plan.accounts[0].stations.clear();
        plan.accounts[0].market_purchase_day = Some("2026-08-08".to_string());

        let result = run_round(&driver, &plan, Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:market",
                "close-game",
                "run:craft",
                "close-game",
                "run:market",
                "close-game",
            ]
        );
    }

    #[tokio::test]
    async fn yielded_market_injects_newly_due_craft_before_resuming_market() {
        let yielded = AccountRunSuccess {
            market_pending: true,
            market_yielded: true,
            ..AccountRunSuccess::default()
        };
        let refreshed_craft = plan_tasks(&[("craft", 1_500)]).accounts.remove(0);
        let driver = FakeDriver::new(
            vec![
                Ok(yielded),
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::default()),
            ],
            false,
        )
        .with_now_ms(2_000)
        .with_refresh_result(Ok(vec![refreshed_craft]));
        let mut plan = plan_tasks(&[("market", 1_000)]);
        plan.accounts[0].stations.clear();
        plan.accounts[0].market_purchase_day = Some("2026-08-08".to_string());

        let result = run_round(&driver, &plan, Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:market",
                "close-game",
                "run:craft",
                "close-game",
                "run:market",
                "close-game",
            ]
        );
    }

    #[tokio::test]
    async fn four_oclock_discards_all_remaining_market_tasks() {
        let driver = FakeDriver::new(
            vec![
                Ok(AccountRunSuccess::default()),
                Ok(AccountRunSuccess::default()),
            ],
            false,
        )
        .with_market_window_closing_after_runs(1);
        let mut plan = plan_tasks(&[("market-a", 1_000), ("market-b", 1_000)]);
        for task in &mut plan.accounts {
            task.stations.clear();
            task.market_purchase_day = Some("2026-08-08".to_string());
        }

        let result = run_round(&driver, &plan, Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(driver.actions(), ["run:market-a", "close-game"]);
    }

    #[tokio::test]
    async fn dynamic_craft_refresh_failure_pauses_round() {
        let yielded = AccountRunSuccess {
            market_pending: true,
            market_yielded: true,
            ..AccountRunSuccess::default()
        };
        let driver = FakeDriver::new(vec![Ok(yielded)], false)
            .with_refresh_result(Err("动态冻结失败".to_string()));
        let mut plan = plan_tasks(&[("market", 1_000)]);
        plan.accounts[0].stations.clear();
        plan.accounts[0].market_purchase_day = Some("2026-08-08".to_string());

        let result = run_round(&driver, &plan, Arc::new(AtomicBool::new(false))).await;

        assert_eq!(
            result.stop,
            RoundStop::SystemFailure {
                step: "round.refreshDueCraft".to_string(),
                message: "动态冻结失败".to_string(),
            }
        );
        assert_eq!(driver.actions(), ["run:market", "persist-pause"]);
    }

    #[tokio::test]
    async fn future_task_after_long_gap_is_left_for_scheduler() {
        let driver = FakeDriver::new(vec![Ok(AccountRunSuccess::processed(1))], false);

        let result = run_round(
            &driver,
            &plan_tasks(&[("a", 1_000), ("a", 700_001)]),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(driver.actions(), ["run:a", "close-game"]);
    }

    #[tokio::test]
    async fn long_gap_follow_up_reuses_session_when_already_overdue() {
        let driver = FakeDriver::new(
            vec![
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::processed(1)),
            ],
            false,
        )
        .with_now_ms(800_000);

        let result = run_round(
            &driver,
            &plan_tasks(&[("a", 1_000), ("a", 700_001)]),
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(driver.actions(), ["run:a", "continue:a", "close-game"]);
    }

    #[tokio::test]
    async fn account_failure_is_persisted_then_next_account_runs() {
        let driver = FakeDriver::new(
            vec![
                Err(AccountRunError::account("login.scan", "目标 QQ 不存在")),
                Ok(AccountRunSuccess::processed(1)),
            ],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.completed_accounts, 1);
        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "persist-account:a",
                "close-game",
                "run:b",
                "close-game"
            ]
        );
    }

    #[tokio::test]
    async fn navigation_timeout_retries_account_after_other_accounts() {
        let driver = FakeDriver::new(
            vec![
                Err(AccountRunError::navigation_timeout(
                    "navigation.WaitStationGrid",
                )),
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::processed(1)),
            ],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "close-game",
                "defer-navigation:a:1",
                "run:b",
                "close-game",
                "run:a",
                "close-game"
            ]
        );
    }

    #[tokio::test]
    async fn second_navigation_timeout_persists_account_once() {
        let driver = FakeDriver::new(
            vec![
                Err(AccountRunError::navigation_timeout(
                    "navigation.WaitStationGrid",
                )),
                Ok(AccountRunSuccess::processed(1)),
                Err(AccountRunError::navigation_timeout(
                    "navigation.WaitStationGrid",
                )),
            ],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "close-game",
                "defer-navigation:a:1",
                "run:b",
                "close-game",
                "run:a",
                "persist-account:a",
                "close-game"
            ]
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
                Ok(AccountRunSuccess::processed(0)),
            ],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "persist-account:a",
                "close-game",
                "run:b",
                "close-game"
            ]
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
        let driver = FakeDriver::new(vec![Ok(AccountRunSuccess::processed(1))], true);

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.completed_accounts, 1);
        assert_eq!(result.stop, RoundStop::PauseRequested);
        assert_eq!(driver.actions(), ["run:a", "persist-pause", "close-game"]);
    }

    #[tokio::test]
    async fn completed_round_closes_game() {
        let driver = FakeDriver::new(
            vec![
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::processed(1)),
            ],
            false,
        );

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(
            driver.actions(),
            ["run:a", "close-game", "run:b", "close-game"]
        );
    }

    #[tokio::test]
    async fn pause_request_closes_game_after_persisting_pause() {
        let driver = FakeDriver::new(vec![Ok(AccountRunSuccess::processed(1))], true);

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::PauseRequested);
        assert_eq!(driver.actions(), ["run:a", "persist-pause", "close-game"]);
    }

    #[tokio::test]
    async fn system_pause_stops_after_current_task_and_preserves_game() {
        let driver = FakeDriver::new(vec![Ok(AccountRunSuccess::processed(1))], true)
            .preserving_game_on_pause();

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(result.stop, RoundStop::PauseRequestedPreservingGame);
        assert_eq!(driver.actions(), ["run:a", "persist-pause"]);
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
    async fn session_close_failure_reports_and_continues_round() {
        let driver = FakeDriver::new(
            vec![
                Ok(AccountRunSuccess::processed(1)),
                Ok(AccountRunSuccess::processed(1)),
            ],
            false,
        )
        .with_close_failure("无法结束游戏进程");

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        // 慢退出不能变成停摆：下轮登录 StopGame 会重杀，残留进程自愈。
        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(result.completed_accounts, 2);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "close-game",
                "report-close-failure:无法结束游戏进程",
                "run:b",
                "close-game",
            ]
        );
        assert!(!driver.actions().contains(&"persist-pause".to_string()));
    }

    #[tokio::test]
    async fn isolated_account_close_failure_continues_to_next_account() {
        let driver = FakeDriver::new(
            vec![
                Err(AccountRunError::account("ammo.isolated", "仓库空间不足")),
                Ok(AccountRunSuccess::processed(1)),
            ],
            false,
        )
        .with_close_failure("无法结束游戏进程");

        let result = run_round(&driver, &plan(), Arc::new(AtomicBool::new(false))).await;

        // 账号已被隔离并出队，关闭失败不该再连带掐掉后面账号。
        assert_eq!(result.stop, RoundStop::Completed);
        assert_eq!(result.completed_accounts, 1);
        assert_eq!(
            driver.actions(),
            [
                "run:a",
                "persist-account:a",
                "close-game",
                "report-close-failure:无法结束游戏进程",
                "run:b",
                "close-game",
            ]
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
