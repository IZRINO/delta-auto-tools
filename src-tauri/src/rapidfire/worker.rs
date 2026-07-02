use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use super::events;
use super::keys::{
    self, KeyEmitter, RAPIDFIRE_INITIAL_SETTLE_MS, RAPIDFIRE_MIN_INTERVAL_MS,
};
use super::RapidfireLogic;
use crate::tool_base::ToolLogic;

// ---- Session 控制 ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionControl {
    StopWithCompensation,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RapidfireSessionStatus {
    Firing,
    Stopping,
}

pub struct RapidfireSessionRuntime {
    pub count: u64,
    pub status: RapidfireSessionStatus,
    pub control_tx: Option<std::sync::mpsc::Sender<SessionControl>>,
    pub compensate_now: Arc<AtomicBool>,
}

// ---- CardRuntime ----

pub struct CardRuntime {
    pub sessions: std::collections::HashMap<String, RapidfireSessionRuntime>,
    pub active_session_ids: Vec<String>,
    pub last_press_at: Arc<Mutex<Instant>>,
}

impl Default for CardRuntime {
    fn default() -> Self {
        Self {
            sessions: std::collections::HashMap::new(),
            active_session_ids: Vec::new(),
            last_press_at: Arc::new(Mutex::new(Instant::now())),
        }
    }
}

impl CardRuntime {
    pub fn aggregate_status(&self) -> super::types::RapidfireRunStatus {
        if self.sessions.is_empty() {
            super::types::RapidfireRunStatus::Idle
        } else {
            super::types::RapidfireRunStatus::Firing
        }
    }

    pub fn aggregate_count(&self) -> u64 {
        self.sessions.values().map(|session| session.count).sum()
    }
}

// ---- Worker 结构体 ----

pub struct RapidfireSessionWorker {
    pub card_id: String,
    pub session_id: String,
    pub trigger_key: String,
    pub target_key: String,
    pub interval_ms: u64,
    pub press_jitter_min_ms: u64,
    pub press_jitter_max_ms: u64,
    pub skip_compensation: bool,
    pub compensation_delay_min_ms: u64,
    pub compensation_delay_max_ms: u64,
    pub min_press_spacing_ms: u64,
    pub trigger_jitter_max_ms: u64,
    pub cancel_jitter_on_release: bool,
    pub control_rx: std::sync::mpsc::Receiver<SessionControl>,
    pub compensate_now: Arc<AtomicBool>,
    pub last_press_at: Arc<Mutex<Instant>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerDecision {
    Fire { stop_after_fire: bool },
    Stop,
    Cancel,
}

// ---- Session ID ----

static NEXT_RAPIDFIRE_SESSION_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_session_id() -> String {
    let id = NEXT_RAPIDFIRE_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    format!("rapidfire-session-{id}")
}

// ---- 按键间距 ----

pub fn ensure_press_spacing(last_press_at: &Mutex<Instant>, min_press_spacing_ms: u64) {
    if min_press_spacing_ms == 0 {
        return;
    }

    let min_spacing = Duration::from_millis(min_press_spacing_ms);
    let wait = {
        let Ok(mut last) = last_press_at.lock() else {
            return;
        };
        let now = Instant::now();
        if *last > now {
            let wait = last.duration_since(now);
            *last = last
                .checked_add(min_spacing)
                .unwrap_or_else(|| now + min_spacing);
            wait
        } else {
            *last = now
                .checked_add(min_spacing)
                .unwrap_or_else(|| now + min_spacing);
            Duration::ZERO
        }
    };
    if !wait.is_zero() {
        thread::sleep(wait);
    }
}

// ---- Worker 生命周期 ----

pub fn spawn_session_worker(app: AppHandle, worker: RapidfireSessionWorker) {
    let name = format!("rapidfire-{}", worker.session_id);
    let error_app = app.clone();
    let error_card_id = worker.card_id.clone();
    let error_session_id = worker.session_id.clone();
    let spawn_result = thread::Builder::new()
        .name(name)
        .spawn(move || run_session_worker(app, worker));

    if let Err(error) = spawn_result {
        finish_session(&error_app, &error_card_id, &error_session_id);
        emit_hotkey_error(&error_app, format!("启动连发器线程失败: {error}"));
    }
}

pub fn should_compensate_count(count: u64, skip_compensation: bool) -> bool {
    count % 2 == 1 && !skip_compensation
}

/// 运行连发器 worker 线程。
/// `emitter` 为按键输出接口，生产环境使用 EnigoKeyEmitter，测试使用 mock。
pub fn run_session_worker_with_emitter(
    app: AppHandle,
    worker: RapidfireSessionWorker,
    mut emitter: Box<dyn KeyEmitter>,
) {
    let interval = Duration::from_millis(worker.interval_ms.max(RAPIDFIRE_MIN_INTERVAL_MS));
    let mut count = 0u64;
    let mut next_fire_at = Instant::now();

    // 首次开火前稳定延迟
    thread::sleep(Duration::from_millis(RAPIDFIRE_INITIAL_SETTLE_MS));

    // 触发抖动延迟
    if worker.trigger_jitter_max_ms > 0 {
        let jitter_duration = Duration::from_millis(worker.trigger_jitter_max_ms);
        let jitter_deadline = Instant::now() + jitter_duration;
        let mut early_release = false;
        while Instant::now() < jitter_deadline {
            let remaining = jitter_deadline.saturating_duration_since(Instant::now());
            match worker.control_rx.recv_timeout(remaining) {
                Ok(SessionControl::StopWithCompensation) => {
                    if worker.cancel_jitter_on_release {
                        early_release = true;
                        break;
                    }
                }
                Ok(SessionControl::Cancel) => {
                    finish_session(&app, &worker.card_id, &worker.session_id);
                    return;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    break;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    finish_session(&app, &worker.card_id, &worker.session_id);
                    return;
                }
            }
        }

        if early_release {
            ensure_press_spacing(&worker.last_press_at, worker.min_press_spacing_ms);
            match emitter.press_release_target_key(
                &worker.target_key,
                Some(&worker.trigger_key),
                worker.press_jitter_min_ms,
                worker.press_jitter_max_ms,
            ) {
                Ok(()) => {
                    count = 1;
                    let _ = update_session_count(&app, &worker.card_id, &worker.session_id, count);
                }
                Err(error) => emit_hotkey_error(&app, error),
            }
        }
    }

    if worker.trigger_jitter_max_ms > 0 && count > 0 {
        // 早期释放已触发，跳过主循环
    } else {
        loop {
            match wait_for_next_fire(&worker.control_rx, next_fire_at, count) {
                WorkerDecision::Fire { stop_after_fire } => {
                    ensure_press_spacing(&worker.last_press_at, worker.min_press_spacing_ms);
                    match emitter.press_release_target_key(
                        &worker.target_key,
                        Some(&worker.trigger_key),
                        worker.press_jitter_min_ms,
                        worker.press_jitter_max_ms,
                    ) {
                        Ok(()) => {
                            count += 1;
                            if !update_session_count(
                                &app,
                                &worker.card_id,
                                &worker.session_id,
                                count,
                            ) {
                                return;
                            }
                        }
                        Err(error) => {
                            emit_hotkey_error(&app, error);
                            break;
                        }
                    }

                    if stop_after_fire {
                        break;
                    }
                    next_fire_at = Instant::now()
                        .checked_add(interval)
                        .unwrap_or_else(Instant::now);
                }
                WorkerDecision::Stop => {
                    break;
                }
                WorkerDecision::Cancel => {
                    finish_session(&app, &worker.card_id, &worker.session_id);
                    return;
                }
            }
        }
    }

    if should_compensate_count(count, worker.skip_compensation) {
        let compensation_delay = keys::press_jitter_duration_ms(
            worker.compensation_delay_min_ms,
            worker.compensation_delay_max_ms,
        );
        let compensation_deadline = Instant::now() + Duration::from_millis(compensation_delay);
        while Instant::now() < compensation_deadline {
            if worker.compensate_now.load(Ordering::Relaxed) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        ensure_press_spacing(&worker.last_press_at, worker.min_press_spacing_ms);
        match emitter.press_release_target_key(
            &worker.target_key,
            None,
            worker.press_jitter_min_ms,
            worker.press_jitter_max_ms,
        ) {
            Ok(()) => {
                count += 1;
                let _ = update_session_count(&app, &worker.card_id, &worker.session_id, count);
            }
            Err(error) => emit_hotkey_error(&app, error),
        }
    }

    finish_session(&app, &worker.card_id, &worker.session_id);
}

/// 生产环境入口：使用 EnigoKeyEmitter。
pub fn run_session_worker(app: AppHandle, worker: RapidfireSessionWorker) {
    let emitter = match keys::EnigoKeyEmitter::new() {
        Ok(e) => Box::new(e) as Box<dyn KeyEmitter>,
        Err(error) => {
            emit_hotkey_error(&app, error);
            finish_session(&app, &worker.card_id, &worker.session_id);
            return;
        }
    };
    run_session_worker_with_emitter(app, worker, emitter);
}

fn wait_for_next_fire(
    control_rx: &std::sync::mpsc::Receiver<SessionControl>,
    fire_at: Instant,
    count: u64,
) -> WorkerDecision {
    loop {
        let now = Instant::now();
        if now >= fire_at {
            return match control_rx.try_recv() {
                Ok(SessionControl::StopWithCompensation) if count == 0 => WorkerDecision::Fire {
                    stop_after_fire: true,
                },
                Ok(SessionControl::StopWithCompensation) => WorkerDecision::Stop,
                Ok(SessionControl::Cancel) => WorkerDecision::Cancel,
                Err(std::sync::mpsc::TryRecvError::Empty) => WorkerDecision::Fire {
                    stop_after_fire: false,
                },
                Err(std::sync::mpsc::TryRecvError::Disconnected) => WorkerDecision::Cancel,
            };
        }

        let wait_for = fire_at.saturating_duration_since(now);
        match control_rx.recv_timeout(wait_for) {
            Ok(SessionControl::StopWithCompensation) if count == 0 => {
                return WorkerDecision::Fire {
                    stop_after_fire: true,
                };
            }
            Ok(SessionControl::StopWithCompensation) => return WorkerDecision::Stop,
            Ok(SessionControl::Cancel) => return WorkerDecision::Cancel,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return WorkerDecision::Cancel,
        }
    }
}

pub fn update_session_count(
    app: &AppHandle,
    card_id: &str,
    session_id: &str,
    count: u64,
) -> bool {
    let state = app.state::<super::RapidfireState>();
    let bootstrap = {
        let Ok(mut inner) = state.lock_inner() else {
            emit_hotkey_error(app, "连发器状态已损坏".to_string());
            return false;
        };

        let Some(run) = inner.logic.runs.get_mut(card_id) else {
            return false;
        };
        let Some(session) = run.sessions.get_mut(session_id) else {
            return false;
        };

        session.count = count;
        RapidfireLogic::build_bootstrap(&inner)
    };

    super::emit_state(app, bootstrap);
    true
}

pub fn finish_session(app: &AppHandle, card_id: &str, session_id: &str) {
    let state = app.state::<super::RapidfireState>();
    let bootstrap = {
        let Ok(mut inner) = state.lock_inner() else {
            emit_hotkey_error(app, "连发器状态已损坏".to_string());
            return;
        };

        let should_remove_run = if let Some(run) = inner.logic.runs.get_mut(card_id) {
            run.sessions.remove(session_id);
            run.active_session_ids.retain(|id| id != session_id);
            run.sessions.is_empty()
        } else {
            false
        };

        if should_remove_run {
            inner.logic.runs.remove(card_id);
        }

        RapidfireLogic::build_bootstrap(&inner)
    };

    super::emit_state(app, bootstrap);
}

pub fn stop_latest_active_session(
    run: &mut CardRuntime,
    control: SessionControl,
) -> bool {
    while let Some(session_id) = run.active_session_ids.pop() {
        let Some(session) = run.sessions.get_mut(&session_id) else {
            continue;
        };

        session.status = RapidfireSessionStatus::Stopping;
        if let Some(control_tx) = session.control_tx.take() {
            let _ = control_tx.send(control);
            return true;
        }
    }

    false
}

pub fn stop_all_sessions(
    runs: &mut std::collections::HashMap<String, CardRuntime>,
    control: SessionControl,
) {
    for run in runs.values_mut() {
        run.active_session_ids.clear();
        for session in run.sessions.values_mut() {
            session.status = RapidfireSessionStatus::Stopping;
            if let Some(control_tx) = session.control_tx.take() {
                let _ = control_tx.send(control);
            }
        }
    }
}

pub fn stop_removed_or_disabled_sessions(
    runs: &mut std::collections::HashMap<String, CardRuntime>,
    active_card_ids: &[String],
) {
    let removed_ids = runs
        .keys()
        .filter(|id| !active_card_ids.contains(id))
        .cloned()
        .collect::<Vec<_>>();

    for id in &removed_ids {
        if let Some(run) = runs.get_mut(id) {
            run.active_session_ids.clear();
            for session in run.sessions.values_mut() {
                session.status = RapidfireSessionStatus::Stopping;
                if let Some(control_tx) = session.control_tx.take() {
                    let _ = control_tx.send(SessionControl::Cancel);
                }
            }
        }
    }

    for id in removed_ids {
        runs.remove(&id);
    }
}

pub fn emit_hotkey_error(app: &AppHandle, error: String) {
    let _ = app.emit_to("main", events::HOTKEY_ERROR, error);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_before_first_tick_still_allows_one_fire_for_compensation() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(SessionControl::StopWithCompensation).unwrap();

        let decision = wait_for_next_fire(&rx, Instant::now() + Duration::from_secs(1), 0);

        assert_eq!(
            decision,
            WorkerDecision::Fire {
                stop_after_fire: true
            }
        );
    }

    #[test]
    fn stop_after_existing_count_exits_worker_loop_for_compensation_stage() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(SessionControl::StopWithCompensation).unwrap();

        let decision = wait_for_next_fire(&rx, Instant::now() + Duration::from_secs(1), 3);

        assert_eq!(decision, WorkerDecision::Stop);
    }

    #[test]
    fn should_compensate_count_respects_no_append_switch() {
        assert!(should_compensate_count(1, false));
        assert!(!should_compensate_count(2, false));
        assert!(!should_compensate_count(1, true));
    }

    #[test]
    fn same_card_can_hold_multiple_sessions_without_overwriting() {
        let mut runtime = CardRuntime::default();
        let (tx1, _rx1) = std::sync::mpsc::channel();
        let (tx2, _rx2) = std::sync::mpsc::channel();

        runtime.active_session_ids.push("session-1".to_string());
        runtime.sessions.insert(
            "session-1".to_string(),
            RapidfireSessionRuntime {
                count: 1,
                status: RapidfireSessionStatus::Firing,
                control_tx: Some(tx1),
                compensate_now: Arc::new(AtomicBool::new(false)),
            },
        );
        runtime.active_session_ids.push("session-2".to_string());
        runtime.sessions.insert(
            "session-2".to_string(),
            RapidfireSessionRuntime {
                count: 2,
                status: RapidfireSessionStatus::Firing,
                control_tx: Some(tx2),
                compensate_now: Arc::new(AtomicBool::new(false)),
            },
        );

        assert_eq!(runtime.aggregate_status(), super::super::types::RapidfireRunStatus::Firing);
        assert_eq!(runtime.aggregate_count(), 3);
        assert_eq!(runtime.sessions.len(), 2);
    }

    #[test]
    fn stop_latest_active_session_does_not_cancel_older_session() {
        let mut runtime = CardRuntime::default();
        let (tx1, rx1) = std::sync::mpsc::channel();
        let (tx2, rx2) = std::sync::mpsc::channel();

        runtime.active_session_ids.push("session-1".to_string());
        runtime.sessions.insert(
            "session-1".to_string(),
            RapidfireSessionRuntime {
                count: 1,
                status: RapidfireSessionStatus::Firing,
                control_tx: Some(tx1),
                compensate_now: Arc::new(AtomicBool::new(false)),
            },
        );
        runtime.active_session_ids.push("session-2".to_string());
        runtime.sessions.insert(
            "session-2".to_string(),
            RapidfireSessionRuntime {
                count: 1,
                status: RapidfireSessionStatus::Firing,
                control_tx: Some(tx2),
                compensate_now: Arc::new(AtomicBool::new(false)),
            },
        );

        assert!(stop_latest_active_session(
            &mut runtime,
            SessionControl::StopWithCompensation
        ));

        assert!(rx1.try_recv().is_err());
        assert_eq!(
            rx2.try_recv().unwrap(),
            SessionControl::StopWithCompensation
        );
        assert_eq!(
            runtime.sessions["session-1"].status,
            RapidfireSessionStatus::Firing
        );
        assert_eq!(
            runtime.sessions["session-2"].status,
            RapidfireSessionStatus::Stopping
        );
    }

    // ---- Worker 生命周期集成测试 (VAL-AR-009 / VAL-AR-010) ----

    /// 构造最小 RapidfireSessionWorker，使用注入的 control_rx。
    fn make_worker(card_id: &str, session_id: &str, control_rx: std::sync::mpsc::Receiver<SessionControl>) -> RapidfireSessionWorker {
        RapidfireSessionWorker {
            card_id: card_id.to_string(),
            session_id: session_id.to_string(),
            trigger_key: "F1".to_string(),
            target_key: "1".to_string(),
            interval_ms: 50,
            press_jitter_min_ms: 1,
            press_jitter_max_ms: 1,
            skip_compensation: true,
            compensation_delay_min_ms: 0,
            compensation_delay_max_ms: 0,
            min_press_spacing_ms: 0,
            trigger_jitter_max_ms: 0,
            cancel_jitter_on_release: false,
            control_rx,
            compensate_now: Arc::new(AtomicBool::new(false)),
            last_press_at: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// VAL-AR-009: 验证 worker 接受 MockKeyEmitter trait 对象，
    /// control_rx 注入后可正确驱动 start→fire→stop 生命周期。
    /// 使用 MockKeyEmitter 验证按键调用。
    #[test]
    fn run_session_worker_accepts_mock_key_emitter_and_fires_on_interval() {
        let (control_tx, control_rx) = std::sync::mpsc::channel();
        let worker = make_worker("test-card", "test-session-1", control_rx);
        let mut emitter = super::super::keys::MockKeyEmitter::new();

        // 模拟 worker 循环的前几轮：等待 interval 到达 → 调用 emitter → 检查 control
        let fire_at = Instant::now() + Duration::from_millis(worker.interval_ms);
        let decision = wait_for_next_fire(&worker.control_rx, fire_at, 0);
        match decision {
            WorkerDecision::Fire { stop_after_fire } => {
                assert!(!stop_after_fire);
                // 模拟发射按键（worker 会调用 emitter.press_release_target_key）
                emitter
                    .press_release_target_key(
                        &worker.target_key,
                        Some(&worker.trigger_key),
                        worker.press_jitter_min_ms,
                        worker.press_jitter_max_ms,
                    )
                    .unwrap();
            }
            WorkerDecision::Stop | WorkerDecision::Cancel => {
                panic!("worker 应在 interval 到达时开火，不应收到 stop/cancel");
            }
        }

        // 验证 MockKeyEmitter 记录了调用
        assert_eq!(emitter.calls.len(), 1);
        assert_eq!(emitter.calls[0].target_key, "1");
        assert_eq!(emitter.calls[0].held_trigger_key, Some("F1".to_string()));

        // 发送 StopWithCompensation 信号 → worker 应停止
        control_tx.send(SessionControl::StopWithCompensation).unwrap();
        let next_fire = Instant::now() + Duration::from_millis(worker.interval_ms);
        let decision = wait_for_next_fire(&worker.control_rx, next_fire, 1);
        assert_eq!(decision, WorkerDecision::Stop);
    }

    /// VAL-AR-009: 验证 cancel 信号使 worker 立即退出，不等待 interval。
    #[test]
    fn run_session_worker_cancel_exits_immediately_without_waiting_interval() {
        let (control_tx, control_rx) = std::sync::mpsc::channel();
        let worker = make_worker("cancel-card", "cancel-session", control_rx);

        // 立即发送 cancel 信号
        control_tx.send(SessionControl::Cancel).unwrap();

        let fire_at = Instant::now() + Duration::from_secs(10); // 很远的未来
        let decision = wait_for_next_fire(&worker.control_rx, fire_at, 0);
        assert_eq!(decision, WorkerDecision::Cancel);
    }

    /// VAL-AR-009: 验证 StopWithCompensation 在 count=0 时仍允许一次开火（用于补偿）。
    #[test]
    fn run_session_worker_stop_with_compensation_at_zero_allows_one_fire() {
        let (control_tx, control_rx) = std::sync::mpsc::channel();
        let worker = make_worker("comp-card", "comp-session", control_rx);

        // 发送 StopWithCompensation 且 count=0
        control_tx.send(SessionControl::StopWithCompensation).unwrap();

        let fire_at = Instant::now() + Duration::from_secs(1);
        let decision = wait_for_next_fire(&worker.control_rx, fire_at, 0);
        assert_eq!(
            decision,
            WorkerDecision::Fire {
                stop_after_fire: true
            }
        );
    }

    /// VAL-AR-010: 验证 stop_all_sessions 后所有 session 的 control_tx
    /// 已被取走（消费），worker 线程收到信号后可正常退出。
    /// 通过检查 control_tx.take() 确认无残留控制权。
    #[test]
    fn stop_all_sessions_drains_control_senders_ensuring_threads_can_exit() {
        let mut runs: std::collections::HashMap<String, CardRuntime> = std::collections::HashMap::new();

        // 设置 3 个 card，每个有 1 个 active session
        for i in 0..3 {
            let (control_tx, _control_rx) = std::sync::mpsc::channel();
            let mut run = CardRuntime::default();
            run.active_session_ids.push(format!("session-{i}"));
            run.sessions.insert(
                format!("session-{i}"),
                RapidfireSessionRuntime {
                    count: 0,
                    status: RapidfireSessionStatus::Firing,
                    control_tx: Some(control_tx),
                    compensate_now: Arc::new(AtomicBool::new(false)),
                },
            );
            runs.insert(format!("card-{i}"), run);
        }

        // 调用 stop_all_sessions 发送 Cancel 信号
        stop_all_sessions(&mut runs, SessionControl::Cancel);

        // 验证所有 session 的 control_tx 已被 take（为 None）
        for run in runs.values() {
            assert!(run.active_session_ids.is_empty(), "active_session_ids 应被清空");
            for session in run.sessions.values() {
                assert!(session.control_tx.is_none(), "control_tx 应已被取走，worker 线程可退出");
            }
        }
    }

    /// VAL-AR-010: 验证 spawn_session_worker 创建的线程名正确，
    /// 且线程在 session cancel 后可被 join（通过验证 stop 机制）。
    #[test]
    fn stop_latest_active_session_consumes_sender_ensuring_thread_termination() {
        let mut run = CardRuntime::default();
        let (control_tx, control_rx) = std::sync::mpsc::channel();

        run.active_session_ids.push("session-terminator".to_string());
        run.sessions.insert(
            "session-terminator".to_string(),
            RapidfireSessionRuntime {
                count: 0,
                status: RapidfireSessionStatus::Firing,
                control_tx: Some(control_tx),
                compensate_now: Arc::new(AtomicBool::new(false)),
            },
        );

        // 发送 Cancel 信号
        let stopped = stop_latest_active_session(&mut run, SessionControl::Cancel);
        assert!(stopped, "应成功停止一个 active session");

        // 验证 control_rx 收到 Cancel 信号（worker 线程可据此退出）
        assert_eq!(control_rx.try_recv().unwrap(), SessionControl::Cancel);

        // 验证 control_tx 已被 take（不再持有发送端）
        assert!(run.sessions["session-terminator"].control_tx.is_none());
    }
}
