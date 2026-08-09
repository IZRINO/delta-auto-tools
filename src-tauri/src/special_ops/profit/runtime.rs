use super::cutoff::FINAL_RETRY_DELAY_MS;
use super::model::AmmoProfitRule;
use super::query::ProfitQueryContext;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

const FIVE_MINUTES_MS: i64 = 5 * 60_000;
const FIFTY_MINUTES_MS: i64 = 50 * 60_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProfitRuntimePhase {
    #[default]
    Disabled,
    WaitingExchange,
    Querying,
    WaitingNextQuery,
    ActiveRound,
    CutoffQuerying,
    WaitingCutoffRetry,
    CutoffComplete,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfitTargetKey {
    pub account_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfitRuntimeSnapshot {
    pub phase: ProfitRuntimePhase,
    pub next_query_at_ms: Option<i64>,
    pub query_attempt: Option<u8>,
    pub qualified_rule_ids: Vec<String>,
    pub current_session_rule_ids: Vec<String>,
    pub active_round_targets: Vec<ProfitTargetKey>,
    pub last_summary: Option<String>,
    pub configuration_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfitQueryWindow {
    pub enabled: bool,
    pub paused: bool,
    pub active_round: bool,
    pub day: String,
    pub settings_revision: u64,
    pub now_ms: i64,
    pub exchange_at_ms: i64,
    pub cutoff_at_ms: i64,
    pub cutoff_complete: bool,
    pub cutoff_retry_at_ms: Option<i64>,
}

#[derive(Debug)]
pub(crate) struct QueryLease {
    pub generation: u64,
    pub settings_revision: u64,
    pub day: String,
    pub rules: Vec<AmmoProfitRule>,
    pub attempt: u8,
    pub mode: ProfitQueryMode,
    started_at_ms: i64,
    cancellation: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfitQueryMode {
    Regular,
    Cutoff { attempt: u8 },
}

impl QueryLease {
    pub(crate) fn query_context(&self) -> ProfitQueryContext {
        ProfitQueryContext {
            generation: self.generation,
            day: self.day.clone(),
            queried_at_ms: self.started_at_ms,
        }
    }

    pub(crate) fn cancellation(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancellation)
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancellation.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub(crate) struct ProfitQueryState {
    day: String,
    settings_revision: u64,
    group_attempt: u8,
    next_query_at_ms: Option<i64>,
    active_query: bool,
    active_cancellation: Option<Arc<AtomicBool>>,
    phase: ProfitRuntimePhase,
    qualified_rule_ids: HashSet<String>,
    current_session_rule_ids: HashSet<String>,
    active_round_targets: Vec<ProfitTargetKey>,
    last_summary: Option<String>,
    configuration_error: Option<String>,
}

impl Default for ProfitQueryState {
    fn default() -> Self {
        Self::new("", 0)
    }
}

impl ProfitQueryState {
    pub(crate) fn new(day: impl Into<String>, settings_revision: u64) -> Self {
        Self {
            day: day.into(),
            settings_revision,
            group_attempt: 0,
            next_query_at_ms: None,
            active_query: false,
            active_cancellation: None,
            phase: ProfitRuntimePhase::Disabled,
            qualified_rule_ids: HashSet::new(),
            current_session_rule_ids: HashSet::new(),
            active_round_targets: Vec::new(),
            last_summary: None,
            configuration_error: None,
        }
    }

    pub(crate) fn begin_group(&mut self, now_ms: i64) {
        self.group_attempt = 0;
        self.next_query_at_ms = Some(now_ms);
        self.active_query = false;
        self.active_cancellation = None;
        self.phase = ProfitRuntimePhase::WaitingNextQuery;
    }

    pub(crate) fn complete_attempt(&mut self, completed_at_ms: i64) {
        let delay = if self.group_attempt == 2 {
            self.group_attempt = 0;
            FIFTY_MINUTES_MS
        } else {
            self.group_attempt += 1;
            FIVE_MINUTES_MS
        };
        self.next_query_at_ms = Some(completed_at_ms.saturating_add(delay));
        self.phase = ProfitRuntimePhase::WaitingNextQuery;
    }

    fn reset_runtime(&mut self, phase: ProfitRuntimePhase, summary: Option<String>) {
        if let Some(cancellation) = self.active_cancellation.take() {
            cancellation.store(true, Ordering::SeqCst);
        }
        self.group_attempt = 0;
        self.next_query_at_ms = None;
        self.active_query = false;
        self.phase = phase;
        self.qualified_rule_ids.clear();
        self.current_session_rule_ids.clear();
        self.active_round_targets.clear();
        self.last_summary = summary;
        self.configuration_error = None;
    }

    fn snapshot(&self) -> ProfitRuntimeSnapshot {
        let mut qualified_rule_ids = self.qualified_rule_ids.iter().cloned().collect::<Vec<_>>();
        qualified_rule_ids.sort();
        let mut current_session_rule_ids = self
            .current_session_rule_ids
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        current_session_rule_ids.sort();
        let mut active_round_targets = self.active_round_targets.clone();
        active_round_targets.sort();
        active_round_targets.dedup();
        let query_attempt = matches!(
            self.phase,
            ProfitRuntimePhase::Querying | ProfitRuntimePhase::WaitingNextQuery
        )
        .then_some(self.group_attempt + 1);
        ProfitRuntimeSnapshot {
            phase: self.phase,
            next_query_at_ms: self.next_query_at_ms,
            query_attempt,
            qualified_rule_ids,
            current_session_rule_ids,
            active_round_targets,
            last_summary: self.last_summary.clone(),
            configuration_error: self.configuration_error.clone(),
        }
    }
}

pub(crate) struct ProfitQueryControl {
    inner: Mutex<ProfitQueryState>,
    generation: AtomicU64,
    cancel: Notify,
    validated_moligod_names: Mutex<HashSet<String>>,
}

impl Default for ProfitQueryControl {
    fn default() -> Self {
        Self {
            inner: Mutex::new(ProfitQueryState::default()),
            generation: AtomicU64::new(0),
            cancel: Notify::new(),
            validated_moligod_names: Mutex::new(HashSet::new()),
        }
    }
}

impl ProfitQueryControl {
    pub(crate) fn record_validated_moligod_name(&self, exact_name: String) -> Result<(), String> {
        self.validated_moligod_names
            .lock()
            .map_err(|_| "利润查询状态已损坏".to_string())?
            .insert(exact_name);
        Ok(())
    }

    pub(crate) fn validated_moligod_names(&self) -> Result<HashSet<String>, String> {
        self.validated_moligod_names
            .lock()
            .map(|names| names.clone())
            .map_err(|_| "利润查询状态已损坏".to_string())
    }

    pub(crate) fn snapshot(&self) -> Result<ProfitRuntimeSnapshot, String> {
        self.inner
            .lock()
            .map(|state| state.snapshot())
            .map_err(|_| "利润查询状态已损坏".to_string())
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub(crate) fn accepts(&self, generation: u64) -> bool {
        self.generation() == generation
    }

    pub(crate) fn invalidate(&self, reason: &str) {
        if let Ok(mut state) = self.inner.lock() {
            state.reset_runtime(ProfitRuntimePhase::Paused, Some(reason.to_string()));
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
        self.cancel.notify_waiters();
    }

    #[cfg(test)]
    pub(crate) async fn cancelled(&self, generation: u64) {
        loop {
            let notified = self.cancel.notified();
            if !self.accepts(generation) {
                return;
            }
            notified.await;
        }
    }

    pub(crate) fn sync_window(
        &self,
        window: ProfitQueryWindow,
    ) -> Result<ProfitRuntimeSnapshot, String> {
        let mut notify_cancel = false;
        let snapshot = {
            let mut state = self
                .inner
                .lock()
                .map_err(|_| "利润查询状态已损坏".to_string())?;
            let identity_changed =
                state.day != window.day || state.settings_revision != window.settings_revision;
            if identity_changed {
                state.reset_runtime(
                    ProfitRuntimePhase::Disabled,
                    Some("利润查询日期或配置已更新".to_string()),
                );
                state.day = window.day;
                state.settings_revision = window.settings_revision;
                self.generation.fetch_add(1, Ordering::SeqCst);
                notify_cancel = true;
            }

            if !window.enabled {
                if state.phase != ProfitRuntimePhase::Disabled
                    || !state.qualified_rule_ids.is_empty()
                    || !state.current_session_rule_ids.is_empty()
                {
                    state.reset_runtime(ProfitRuntimePhase::Disabled, None);
                    self.generation.fetch_add(1, Ordering::SeqCst);
                    notify_cancel = true;
                }
            } else if window.paused {
                if state.phase != ProfitRuntimePhase::Paused {
                    state.reset_runtime(
                        ProfitRuntimePhase::Paused,
                        Some("自动化已暂停".to_string()),
                    );
                    self.generation.fetch_add(1, Ordering::SeqCst);
                    notify_cancel = true;
                }
            } else if window.active_round || state.phase == ProfitRuntimePhase::ActiveRound {
                state.next_query_at_ms = None;
                state.phase = ProfitRuntimePhase::ActiveRound;
            } else if window.now_ms < window.exchange_at_ms {
                if state.active_query {
                    state.reset_runtime(ProfitRuntimePhase::WaitingExchange, None);
                    self.generation.fetch_add(1, Ordering::SeqCst);
                    notify_cancel = true;
                }
                state.phase = ProfitRuntimePhase::WaitingExchange;
                state.next_query_at_ms = Some(window.exchange_at_ms);
            } else if window.now_ms >= window.cutoff_at_ms {
                if state.active_query {
                    state.phase = ProfitRuntimePhase::CutoffQuerying;
                } else if window.cutoff_complete {
                    state.phase = ProfitRuntimePhase::CutoffComplete;
                    state.next_query_at_ms = None;
                } else {
                    state.phase = ProfitRuntimePhase::WaitingCutoffRetry;
                    state.next_query_at_ms = Some(
                        window
                            .cutoff_retry_at_ms
                            .unwrap_or(window.now_ms)
                            .max(window.now_ms),
                    );
                }
            } else if !matches!(
                state.phase,
                ProfitRuntimePhase::Querying | ProfitRuntimePhase::WaitingNextQuery
            ) {
                state.begin_group(window.now_ms);
            }
            state.snapshot()
        };
        if notify_cancel {
            self.cancel.notify_waiters();
        }
        Ok(snapshot)
    }

    pub(crate) fn begin_query(
        &self,
        day: &str,
        settings_revision: u64,
        started_at_ms: i64,
        rules: Vec<AmmoProfitRule>,
    ) -> Result<QueryLease, String> {
        if rules.is_empty() {
            return Err("利润查询目标为空".to_string());
        }
        let mut state = self
            .inner
            .lock()
            .map_err(|_| "利润查询状态已损坏".to_string())?;
        if state.phase == ProfitRuntimePhase::ActiveRound {
            return Err("当前轮次期间禁止启动利润查询".to_string());
        }
        if state.active_query {
            return Err("已有利润查询正在进行".to_string());
        }
        if state.day != day || state.settings_revision != settings_revision {
            state.reset_runtime(ProfitRuntimePhase::Disabled, None);
            state.day = day.to_string();
            state.settings_revision = settings_revision;
            self.generation.fetch_add(1, Ordering::SeqCst);
            self.cancel.notify_waiters();
        }
        if state.phase != ProfitRuntimePhase::WaitingNextQuery {
            state.begin_group(started_at_ms);
        }
        let attempt = state.group_attempt.saturating_add(1);
        let cancellation = Arc::new(AtomicBool::new(false));
        state.active_query = true;
        state.active_cancellation = Some(Arc::clone(&cancellation));
        state.next_query_at_ms = None;
        state.phase = ProfitRuntimePhase::Querying;
        Ok(QueryLease {
            generation: self.generation(),
            settings_revision,
            day: day.to_string(),
            rules,
            attempt,
            mode: ProfitQueryMode::Regular,
            started_at_ms,
            cancellation,
        })
    }

    pub(crate) fn begin_cutoff_query(
        &self,
        day: &str,
        settings_revision: u64,
        started_at_ms: i64,
        rules: Vec<AmmoProfitRule>,
        attempt: u8,
    ) -> Result<QueryLease, String> {
        if rules.is_empty() {
            return Err("截止利润查询目标为空".to_string());
        }
        if !(1..=2).contains(&attempt) {
            return Err("截止利润查询次数无效".to_string());
        }
        let mut state = self
            .inner
            .lock()
            .map_err(|_| "利润查询状态已损坏".to_string())?;
        if state.phase == ProfitRuntimePhase::ActiveRound {
            return Err("当前轮次期间禁止启动利润查询".to_string());
        }
        if state.active_query {
            return Err("已有利润查询正在进行".to_string());
        }
        if state.day != day || state.settings_revision != settings_revision {
            state.reset_runtime(ProfitRuntimePhase::Disabled, None);
            state.day = day.to_string();
            state.settings_revision = settings_revision;
            self.generation.fetch_add(1, Ordering::SeqCst);
            self.cancel.notify_waiters();
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        state.group_attempt = attempt - 1;
        state.active_query = true;
        state.active_cancellation = Some(Arc::clone(&cancellation));
        state.next_query_at_ms = None;
        state.phase = ProfitRuntimePhase::CutoffQuerying;
        Ok(QueryLease {
            generation: self.generation(),
            settings_revision,
            day: day.to_string(),
            rules,
            attempt,
            mode: ProfitQueryMode::Cutoff { attempt },
            started_at_ms,
            cancellation,
        })
    }

    #[cfg(test)]
    pub(crate) fn complete_query(
        &self,
        lease: &QueryLease,
        completed_at_ms: i64,
        qualified_rule_ids: HashSet<String>,
        summary: String,
    ) -> Result<bool, String> {
        self.complete_query_inner(lease, completed_at_ms, qualified_rule_ids, summary, None)
    }

    pub(crate) fn complete_query_at_revision(
        &self,
        lease: &QueryLease,
        completed_at_ms: i64,
        qualified_rule_ids: HashSet<String>,
        summary: String,
        persisted_settings_revision: u64,
    ) -> Result<bool, String> {
        self.complete_query_inner(
            lease,
            completed_at_ms,
            qualified_rule_ids,
            summary,
            Some(persisted_settings_revision),
        )
    }

    fn complete_query_inner(
        &self,
        lease: &QueryLease,
        completed_at_ms: i64,
        qualified_rule_ids: HashSet<String>,
        summary: String,
        persisted_settings_revision: Option<u64>,
    ) -> Result<bool, String> {
        if lease.mode != ProfitQueryMode::Regular {
            return Err("截止利润查询不能按常规 cadence 完成".to_string());
        }
        let mut state = self
            .inner
            .lock()
            .map_err(|_| "利润查询状态已损坏".to_string())?;
        let active_matches = state
            .active_cancellation
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, &lease.cancellation));
        if !self.accepts(lease.generation)
            || lease.is_cancelled()
            || state.day != lease.day
            || state.settings_revision != lease.settings_revision
            || !state.active_query
            || !active_matches
        {
            return Ok(false);
        }
        let requested_rule_ids = lease
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<HashSet<_>>();
        if qualified_rule_ids
            .iter()
            .any(|id| !requested_rule_ids.contains(id.as_str()))
        {
            return Err("利润查询结果包含未请求规则".to_string());
        }
        state.active_query = false;
        state.active_cancellation = None;
        if let Some(settings_revision) = persisted_settings_revision {
            state.settings_revision = settings_revision;
        }
        state
            .current_session_rule_ids
            .extend(lease.rules.iter().map(|rule| rule.id.clone()));
        state.qualified_rule_ids = qualified_rule_ids;
        state.last_summary = Some(summary);
        state.complete_attempt(completed_at_ms);
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn complete_cutoff_query_at_revision(
        &self,
        lease: &QueryLease,
        completed_at_ms: i64,
        qualified_rule_ids: HashSet<String>,
        summary: String,
        persisted_settings_revision: u64,
        retry_required: bool,
    ) -> Result<bool, String> {
        if !matches!(lease.mode, ProfitQueryMode::Cutoff { .. }) {
            return Err("常规利润查询不能按截止查询完成".to_string());
        }
        let mut state = self
            .inner
            .lock()
            .map_err(|_| "利润查询状态已损坏".to_string())?;
        let active_matches = state
            .active_cancellation
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, &lease.cancellation));
        if !self.accepts(lease.generation)
            || lease.is_cancelled()
            || state.day != lease.day
            || state.settings_revision != lease.settings_revision
            || !state.active_query
            || !active_matches
        {
            return Ok(false);
        }
        let requested_rule_ids = lease
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<HashSet<_>>();
        if qualified_rule_ids
            .iter()
            .any(|id| !requested_rule_ids.contains(id.as_str()))
        {
            return Err("截止利润查询结果包含未请求规则".to_string());
        }
        state.active_query = false;
        state.active_cancellation = None;
        state.settings_revision = persisted_settings_revision;
        state.qualified_rule_ids = qualified_rule_ids;
        state.last_summary = Some(summary);
        if retry_required {
            state.phase = ProfitRuntimePhase::WaitingCutoffRetry;
            state.next_query_at_ms = Some(completed_at_ms.saturating_add(FINAL_RETRY_DELAY_MS));
        } else {
            state.phase = ProfitRuntimePhase::CutoffComplete;
            state.next_query_at_ms = None;
        }
        Ok(true)
    }

    pub(crate) fn consume_for_round(
        &self,
        generation: u64,
        targets: Vec<ProfitTargetKey>,
    ) -> Result<(), String> {
        if !self.accepts(generation) {
            return Err("利润查询 generation 已失效".to_string());
        }
        let mut state = self
            .inner
            .lock()
            .map_err(|_| "利润查询状态已损坏".to_string())?;
        if state.active_query {
            return Err("利润查询进行中，禁止冻结 round".to_string());
        }
        state.qualified_rule_ids.clear();
        state.active_round_targets = targets;
        state.active_round_targets.sort();
        state.active_round_targets.dedup();
        state.next_query_at_ms = None;
        state.phase = ProfitRuntimePhase::ActiveRound;
        Ok(())
    }

    pub(crate) fn rollback_failed_round_start(&self, generation: u64) -> Result<bool, String> {
        let rolled_back = {
            let mut state = self
                .inner
                .lock()
                .map_err(|_| "利润查询状态已损坏".to_string())?;
            if self.generation() != generation || state.phase != ProfitRuntimePhase::ActiveRound {
                false
            } else {
                state.reset_runtime(
                    ProfitRuntimePhase::Paused,
                    Some("轮次启动失败，利润资格已撤销".to_string()),
                );
                self.generation.fetch_add(1, Ordering::SeqCst);
                true
            }
        };
        if rolled_back {
            self.cancel.notify_waiters();
        }
        Ok(rolled_back)
    }

    pub(crate) fn end_active_round(&self, reason: &str) {
        self.invalidate(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str) -> AmmoProfitRule {
        AmmoProfitRule {
            id: id.to_string(),
            display_name: format!("规则 {id}"),
            kkrb_match_name: format!("KKRB {id}"),
            moligod_match_name: Some(format!("Moligod {id}")),
            minimum_profit: 100,
        }
    }

    fn query_window(
        day: &str,
        now_ms: i64,
        exchange_at_ms: i64,
        cutoff_at_ms: i64,
    ) -> ProfitQueryWindow {
        ProfitQueryWindow {
            enabled: true,
            paused: false,
            active_round: false,
            day: day.to_string(),
            settings_revision: 4,
            now_ms,
            exchange_at_ms,
            cutoff_at_ms,
            cutoff_complete: false,
            cutoff_retry_at_ms: None,
        }
    }

    #[test]
    fn validated_moligod_names_are_process_local_and_deduplicated() {
        let control = ProfitQueryControl::default();

        control
            .record_validated_moligod_name("目标 A".to_string())
            .unwrap();
        control
            .record_validated_moligod_name("目标 A".to_string())
            .unwrap();

        assert_eq!(
            control.validated_moligod_names().unwrap(),
            std::collections::HashSet::from(["目标 A".to_string()])
        );
    }

    #[test]
    fn cadence_is_immediate_five_five_then_fifty_from_completion() {
        let mut state = ProfitQueryState::new("2026-08-02", 4);
        state.begin_group(1_000);
        assert_eq!(state.next_query_at_ms, Some(1_000));

        state.complete_attempt(2_000);
        assert_eq!(state.next_query_at_ms, Some(2_000 + 5 * 60_000));
        state.complete_attempt(10_000);
        assert_eq!(state.next_query_at_ms, Some(10_000 + 5 * 60_000));
        state.complete_attempt(20_000);
        assert_eq!(state.next_query_at_ms, Some(20_000 + 50 * 60_000));
    }

    #[test]
    fn persisted_query_completion_keeps_runtime_revision_in_sync() {
        let control = ProfitQueryControl::default();
        let lease = control
            .begin_query("2026-08-02", 4, 1_000, vec![rule("rule-a")])
            .unwrap();

        assert!(control
            .complete_query_at_revision(
                &lease,
                2_000,
                HashSet::from(["rule-a".to_string()]),
                "已达标".to_string(),
                5,
            )
            .unwrap());
        assert_eq!(control.inner.lock().unwrap().settings_revision, 5);
        assert_eq!(control.snapshot().unwrap().qualified_rule_ids, ["rule-a"]);
    }

    #[test]
    fn invalidate_discards_late_generation_and_qualification() {
        let control = ProfitQueryControl::default();
        let completed_lease = control
            .begin_query("2026-08-02", 4, 1_000, vec![rule("rule-a")])
            .unwrap();
        assert!(control
            .complete_query(
                &completed_lease,
                2_000,
                HashSet::from(["rule-a".to_string()]),
                "1 个达标".to_string(),
            )
            .unwrap());
        assert_eq!(control.snapshot().unwrap().qualified_rule_ids, ["rule-a"]);

        let lease = control
            .begin_query("2026-08-02", 4, 3_000, vec![rule("rule-a")])
            .unwrap();

        control.invalidate("设置已修改");

        assert!(!control.accepts(lease.generation));
        assert!(lease.is_cancelled());
        assert!(control.snapshot().unwrap().qualified_rule_ids.is_empty());
    }

    #[test]
    fn later_query_replaces_previous_qualification() {
        let control = ProfitQueryControl::default();
        let first = control
            .begin_query("2026-08-02", 4, 1_000, vec![rule("rule-a")])
            .unwrap();
        control
            .complete_query(
                &first,
                2_000,
                HashSet::from(["rule-a".to_string()]),
                "达标".to_string(),
            )
            .unwrap();

        let second = control
            .begin_query("2026-08-02", 4, 3_000, vec![rule("rule-a")])
            .unwrap();
        control
            .complete_query(&second, 4_000, HashSet::new(), "未达标".to_string())
            .unwrap();

        assert!(control.snapshot().unwrap().qualified_rule_ids.is_empty());
    }

    #[test]
    fn exchange_window_controls_when_a_query_can_be_scheduled() {
        let control = ProfitQueryControl::default();

        let waiting = control
            .sync_window(query_window("2026-08-02", 500, 1_000, 2_000))
            .unwrap();
        assert_eq!(waiting.phase, ProfitRuntimePhase::WaitingExchange);
        assert_eq!(waiting.next_query_at_ms, Some(1_000));

        let querying = control
            .sync_window(query_window("2026-08-02", 1_200, 1_000, 2_000))
            .unwrap();
        assert_eq!(querying.phase, ProfitRuntimePhase::WaitingNextQuery);
        assert_eq!(querying.next_query_at_ms, Some(1_200));

        let cutoff = control
            .sync_window(query_window("2026-08-02", 2_000, 1_000, 2_000))
            .unwrap();
        assert_eq!(cutoff.phase, ProfitRuntimePhase::WaitingCutoffRetry);
        assert_eq!(cutoff.next_query_at_ms, Some(2_000));
    }

    #[test]
    fn resume_inside_query_window_starts_a_new_group_immediately() {
        let control = ProfitQueryControl::default();
        let mut paused = query_window("2026-08-02", 1_200, 1_000, 2_000);
        paused.paused = true;
        assert_eq!(
            control.sync_window(paused).unwrap().phase,
            ProfitRuntimePhase::Paused
        );

        let resumed = control
            .sync_window(query_window("2026-08-02", 1_500, 1_000, 2_000))
            .unwrap();
        assert_eq!(resumed.phase, ProfitRuntimePhase::WaitingNextQuery);
        assert_eq!(resumed.next_query_at_ms, Some(1_500));
        assert_eq!(resumed.query_attempt, Some(1));
    }

    #[test]
    fn active_query_is_mutually_exclusive() {
        let control = ProfitQueryControl::default();
        let _lease = control
            .begin_query("2026-08-02", 4, 1_000, vec![rule("rule-a")])
            .unwrap();

        assert!(control
            .begin_query("2026-08-02", 4, 1_001, vec![rule("rule-a")])
            .unwrap_err()
            .contains("正在进行"));
    }

    #[test]
    fn natural_day_and_disabled_filter_clear_session_qualification() {
        let control = ProfitQueryControl::default();
        let lease = control
            .begin_query("2026-08-02", 4, 1_000, vec![rule("rule-a")])
            .unwrap();
        control
            .complete_query(
                &lease,
                2_000,
                HashSet::from(["rule-a".to_string()]),
                "达标".to_string(),
            )
            .unwrap();

        let next_day = control
            .sync_window(query_window("2026-08-03", 1_500, 1_000, 2_000))
            .unwrap();
        assert!(next_day.qualified_rule_ids.is_empty());
        assert!(next_day.current_session_rule_ids.is_empty());

        let lease = control
            .begin_query("2026-08-03", 4, 1_500, vec![rule("rule-a")])
            .unwrap();
        control
            .complete_query(
                &lease,
                1_600,
                HashSet::from(["rule-a".to_string()]),
                "再次达标".to_string(),
            )
            .unwrap();
        let mut disabled = query_window("2026-08-03", 1_700, 1_000, 2_000);
        disabled.enabled = false;
        let disabled = control.sync_window(disabled).unwrap();
        assert_eq!(disabled.phase, ProfitRuntimePhase::Disabled);
        assert!(disabled.qualified_rule_ids.is_empty());
        assert!(disabled.current_session_rule_ids.is_empty());
    }

    #[test]
    fn round_freeze_consumes_qualification_and_blocks_querying() {
        let control = ProfitQueryControl::default();
        let lease = control
            .begin_query("2026-08-02", 4, 1_000, vec![rule("rule-a")])
            .unwrap();
        control
            .complete_query(
                &lease,
                2_000,
                HashSet::from(["rule-a".to_string()]),
                "达标".to_string(),
            )
            .unwrap();

        control
            .consume_for_round(
                lease.generation,
                vec![ProfitTargetKey {
                    account_id: "account-a".to_string(),
                    target_id: "ammo-a".to_string(),
                }],
            )
            .unwrap();
        let snapshot = control.snapshot().unwrap();
        assert_eq!(snapshot.phase, ProfitRuntimePhase::ActiveRound);
        assert!(snapshot.qualified_rule_ids.is_empty());
        assert_eq!(snapshot.active_round_targets.len(), 1);
        assert!(control
            .begin_query("2026-08-02", 4, 2_100, vec![rule("rule-a")])
            .unwrap_err()
            .contains("轮次"));
    }

    #[test]
    fn failed_round_start_releases_only_matching_active_round() {
        let control = ProfitQueryControl::default();
        let lease = control
            .begin_query("2026-08-02", 4, 1_000, vec![rule("rule-a")])
            .unwrap();
        control
            .complete_query(
                &lease,
                2_000,
                HashSet::from(["rule-a".to_string()]),
                "达标".to_string(),
            )
            .unwrap();
        control
            .consume_for_round(
                lease.generation,
                vec![ProfitTargetKey {
                    account_id: "account-a".to_string(),
                    target_id: "ammo-a".to_string(),
                }],
            )
            .unwrap();

        assert!(control
            .rollback_failed_round_start(lease.generation)
            .unwrap());
        let snapshot = control.snapshot().unwrap();
        assert_eq!(snapshot.phase, ProfitRuntimePhase::Paused);
        assert!(snapshot.active_round_targets.is_empty());
        assert_ne!(control.generation(), lease.generation);
        assert!(!control
            .rollback_failed_round_start(lease.generation)
            .unwrap());
    }

    #[test]
    fn current_session_rule_ids_do_not_restore_from_history() {
        let first_session = ProfitQueryControl::default();
        let lease = first_session
            .begin_query("2026-08-02", 4, 1_000, vec![rule("rule-a")])
            .unwrap();
        first_session
            .complete_query(&lease, 2_000, HashSet::new(), "未达标".to_string())
            .unwrap();
        assert_eq!(
            first_session.snapshot().unwrap().current_session_rule_ids,
            ["rule-a"]
        );

        let restarted = ProfitQueryControl::default();
        assert!(restarted
            .snapshot()
            .unwrap()
            .current_session_rule_ids
            .is_empty());
    }

    #[test]
    fn cutoff_query_retries_once_then_completes() {
        let control = ProfitQueryControl::default();
        let first = control
            .begin_cutoff_query("2026-08-06", 4, 2_000, vec![rule("rule-a")], 1)
            .unwrap();
        assert_eq!(first.mode, ProfitQueryMode::Cutoff { attempt: 1 });
        assert!(control
            .complete_cutoff_query_at_revision(
                &first,
                3_000,
                HashSet::new(),
                "等待补查".to_string(),
                5,
                true,
            )
            .unwrap());
        let waiting = control.snapshot().unwrap();
        assert_eq!(waiting.phase, ProfitRuntimePhase::WaitingCutoffRetry);
        assert_eq!(waiting.next_query_at_ms, Some(3_000 + 5 * 60_000));

        let second = control
            .begin_cutoff_query("2026-08-06", 5, 3_000 + 5 * 60_000, vec![rule("rule-a")], 2)
            .unwrap();
        assert!(control
            .complete_cutoff_query_at_revision(
                &second,
                304_000,
                HashSet::new(),
                "截止处理完成".to_string(),
                6,
                false,
            )
            .unwrap());
        let completed = control.snapshot().unwrap();
        assert_eq!(completed.phase, ProfitRuntimePhase::CutoffComplete);
        assert_eq!(completed.next_query_at_ms, None);
    }

    #[tokio::test]
    async fn cancel_notify_wakes_generation_waiter() {
        let control = std::sync::Arc::new(ProfitQueryControl::default());
        let lease = control
            .begin_query("2026-08-02", 4, 1_000, vec![rule("rule-a")])
            .unwrap();
        let waiting_control = std::sync::Arc::clone(&control);
        let waiter = tokio::spawn(async move {
            waiting_control.cancelled(lease.generation).await;
        });
        tokio::task::yield_now().await;

        control.invalidate("用户暂停");

        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("取消通知必须唤醒等待者")
            .unwrap();
    }
}
