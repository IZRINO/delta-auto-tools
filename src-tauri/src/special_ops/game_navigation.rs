use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::AppHandle;

use super::{
    desktop_runtime::{DesktopRuntime, WindowsDesktopRuntime},
    login_flow::LoginStep,
    login_runtime::{emit_run_changed, LoginRunStatus, LoginRuntime},
    template_observer::{RuntimeSimilaritySampler, RuntimeTarget},
};

const STEP_TIMEOUT: Duration = Duration::from_secs(180);
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GameNavigationStep {
    WaitModeReady,
    OpenBeaconMode,
    DismissActivityPopup,
    SwitchLobbyView,
    OpenSpecialOps,
    WaitStationGrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavigationKey {
    Space,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavigationDestination {
    Lobby,
    StationGrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NavigationDelays {
    pub beacon_ms: u32,
    pub space_ms: u32,
    pub tab_ms: u32,
    pub special_ops_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GameNavigationResult {
    Ready,
    TimedOut {
        failed_step: GameNavigationStep,
    },
    Paused {
        failed_step: GameNavigationStep,
        message: String,
    },
    EmergencyStopped,
}

pub(crate) struct NavigationRunConfig {
    pub game_executable_path: PathBuf,
    pub mouse_parking_region: crate::morse::types::RegionRect,
    pub targets: HashMap<String, RuntimeTarget>,
    pub delays: NavigationDelays,
    pub destination: NavigationDestination,
}

impl From<GameNavigationStep> for LoginStep {
    fn from(step: GameNavigationStep) -> Self {
        match step {
            GameNavigationStep::WaitModeReady => Self::WaitModeReady,
            GameNavigationStep::OpenBeaconMode => Self::OpenBeaconMode,
            GameNavigationStep::DismissActivityPopup => Self::DismissActivityPopup,
            GameNavigationStep::SwitchLobbyView => Self::SwitchLobbyView,
            GameNavigationStep::OpenSpecialOps => Self::OpenSpecialOps,
            GameNavigationStep::WaitStationGrid => Self::WaitStationGrid,
        }
    }
}

#[allow(async_fn_in_trait)]
pub(crate) trait GameNavigationDriver: Send + Sync {
    async fn wait_for_any(
        &self,
        target_keys: &[&str],
        cancelled: Arc<AtomicBool>,
    ) -> Result<String, String>;
    async fn click(&self, target_key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn press(&self, key: NavigationKey, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn wait_delay(&self, delay_ms: u32, cancelled: Arc<AtomicBool>) -> Result<(), String>;
}

/// 从游戏模式选择页导航到四制作台页面。
pub(crate) async fn run_game_navigation<D, F>(
    driver: &D,
    destination: NavigationDestination,
    delays: NavigationDelays,
    cancelled: Arc<AtomicBool>,
    mut on_step: F,
) -> GameNavigationResult
where
    D: GameNavigationDriver + ?Sized,
    F: FnMut(GameNavigationStep),
{
    if let Err(result) = run_navigation_step(
        GameNavigationStep::WaitModeReady,
        &cancelled,
        &mut on_step,
        driver.wait_for_any(&["game.modeReady"], Arc::clone(&cancelled)),
    )
    .await
    {
        return result;
    }
    if let Err(result) = run_navigation_step(
        GameNavigationStep::OpenBeaconMode,
        &cancelled,
        &mut on_step,
        driver.wait_delay(delays.beacon_ms, Arc::clone(&cancelled)),
    )
    .await
    {
        return result;
    }
    if let Err(result) = run_navigation_step(
        GameNavigationStep::OpenBeaconMode,
        &cancelled,
        &mut on_step,
        driver.click("game.beaconMode", Arc::clone(&cancelled)),
    )
    .await
    {
        return result;
    }

    if let Err(result) = run_navigation_step(
        GameNavigationStep::DismissActivityPopup,
        &cancelled,
        &mut on_step,
        driver.wait_delay(delays.space_ms, Arc::clone(&cancelled)),
    )
    .await
    {
        return result;
    }
    if let Err(result) = run_navigation_step(
        GameNavigationStep::DismissActivityPopup,
        &cancelled,
        &mut on_step,
        driver.press(NavigationKey::Space, Arc::clone(&cancelled)),
    )
    .await
    {
        return result;
    }
    if let Err(result) = run_navigation_step(
        GameNavigationStep::SwitchLobbyView,
        &cancelled,
        &mut on_step,
        driver.wait_delay(delays.tab_ms, Arc::clone(&cancelled)),
    )
    .await
    {
        return result;
    }

    if let Err(result) = run_navigation_step(
        GameNavigationStep::SwitchLobbyView,
        &cancelled,
        &mut on_step,
        driver.press(NavigationKey::Tab, Arc::clone(&cancelled)),
    )
    .await
    {
        return result;
    }
    if destination == NavigationDestination::Lobby {
        return GameNavigationResult::Ready;
    }
    if let Err(result) = run_navigation_step(
        GameNavigationStep::OpenSpecialOps,
        &cancelled,
        &mut on_step,
        driver.wait_delay(delays.special_ops_ms, Arc::clone(&cancelled)),
    )
    .await
    {
        return result;
    }
    if let Err(result) = run_navigation_step(
        GameNavigationStep::OpenSpecialOps,
        &cancelled,
        &mut on_step,
        driver.click("game.specialOps", Arc::clone(&cancelled)),
    )
    .await
    {
        return result;
    }
    if let Err(result) = run_navigation_step(
        GameNavigationStep::WaitStationGrid,
        &cancelled,
        &mut on_step,
        driver.wait_for_any(&["game.stationGrid"], Arc::clone(&cancelled)),
    )
    .await
    {
        return result;
    }
    GameNavigationResult::Ready
}

/// 为每个导航步骤提供独立超时预算和紧急停止优先级。
async fn run_navigation_step<T, F, Fut>(
    step: GameNavigationStep,
    cancelled: &Arc<AtomicBool>,
    on_step: &mut F,
    future: Fut,
) -> Result<T, GameNavigationResult>
where
    F: FnMut(GameNavigationStep),
    Fut: Future<Output = Result<T, String>>,
{
    on_step(step);
    tokio::select! {
        biased;
        _ = wait_for_cancellation(cancelled) => Err(GameNavigationResult::EmergencyStopped),
        result = future => result.map_err(|message| GameNavigationResult::Paused {
            failed_step: step,
            message,
        }),
        _ = tokio::time::sleep(STEP_TIMEOUT) => Err(GameNavigationResult::TimedOut {
            failed_step: step,
        }),
    }
}

/// 低频轮询取消标记，确保等待模板期间紧急停止可立即接管。
async fn wait_for_cancellation(cancelled: &AtomicBool) {
    while !cancelled.load(Ordering::SeqCst) {
        tokio::time::sleep(CANCEL_POLL_INTERVAL).await;
    }
}

pub(crate) struct ProductionGameNavigationDriver {
    app: AppHandle,
    runtime: Arc<LoginRuntime>,
    run_id: u64,
    config: Arc<NavigationRunConfig>,
}

impl ProductionGameNavigationDriver {
    pub(crate) fn new(
        app: AppHandle,
        runtime: Arc<LoginRuntime>,
        run_id: u64,
        config: Arc<NavigationRunConfig>,
    ) -> Self {
        Self {
            app,
            runtime,
            run_id,
            config,
        }
    }

    /// 发布导航试运行运行态，不触碰业务配置。
    fn emit_update(
        &self,
        status: LoginRunStatus,
        message: impl Into<String>,
        countdown_seconds: Option<u8>,
    ) -> Result<(), String> {
        let step = self
            .runtime
            .snapshot()?
            .filter(|snapshot| snapshot.run_id == self.run_id)
            .and_then(|snapshot| snapshot.current_step);
        if let Some(snapshot) =
            self.runtime
                .update(self.run_id, status, step, message, countdown_seconds)?
        {
            emit_run_changed(&self.app, &snapshot);
        }
        Ok(())
    }

    /// 按配置的 canonical 游戏路径恢复并聚焦唯一目标窗口。
    async fn focus_game(&self) -> Result<(), String> {
        let executable = self.config.game_executable_path.clone();
        tokio::task::spawn_blocking(move || {
            let runtime = WindowsDesktopRuntime;
            let window = runtime
                .find_primary_window(&executable)?
                .ok_or_else(|| "未找到游戏窗口".to_string())?;
            runtime.restore_and_focus(&executable, window)
        })
        .await
        .map_err(|error| format!("游戏窗口任务失败: {error}"))?
    }

    async fn initial_countdown_if_needed(&self, cancelled: &Arc<AtomicBool>) -> Result<(), String> {
        let Some(total) = self
            .runtime
            .next_input_countdown_seconds(self.run_id, false)?
        else {
            return Ok(());
        };
        for seconds in (1..=total).rev() {
            ensure_not_cancelled(cancelled)?;
            self.emit_update(
                LoginRunStatus::Countdown,
                format!("{seconds} 秒后执行键鼠操作"),
                Some(seconds),
            )?;
            wait_cancellable(Duration::from_secs(1), cancelled).await?;
        }
        Ok(())
    }
}

impl GameNavigationDriver for ProductionGameNavigationDriver {
    async fn wait_for_any(
        &self,
        target_keys: &[&str],
        cancelled: Arc<AtomicBool>,
    ) -> Result<String, String> {
        let templates = target_keys
            .iter()
            .map(|key| {
                self.config
                    .targets
                    .get(*key)
                    .and_then(|target| target.template.as_ref())
                    .ok_or_else(|| format!("导航识别目标 {key} 未配置已验证模板"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        super::template_observer::wait_for_any_consistent_match(
            &RuntimeSimilaritySampler,
            &templates,
            cancelled,
        )
        .await
        .map(|(key, _)| key)
    }

    async fn click(&self, target_key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        let region = self
            .config
            .targets
            .get(target_key)
            .ok_or_else(|| format!("导航校准目标 {target_key} 不存在"))?
            .region
            .clone();
        self.initial_countdown_if_needed(&cancelled).await?;
        ensure_not_cancelled(&cancelled)?;
        self.focus_game().await?;
        self.emit_update(LoginRunStatus::Inputting, "正在执行游戏点击", None)?;
        crate::input_simulation::click_region_center_held_cancellable(
            region,
            super::MOUSE_CLICK_HOLD_MS,
            Arc::clone(&cancelled),
        )
        .await?;
        crate::input_simulation::move_region_center_cancellable(
            self.config.mouse_parking_region.clone(),
            cancelled,
        )
        .await
    }

    async fn press(&self, key: NavigationKey, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        let (named_key, message) = match key {
            NavigationKey::Space => (crate::hotkey_types::NamedKey::Space, "正在关闭活动弹窗"),
            NavigationKey::Tab => (crate::hotkey_types::NamedKey::Tab, "正在切换大厅视角"),
        };
        self.initial_countdown_if_needed(&cancelled).await?;
        ensure_not_cancelled(&cancelled)?;
        self.focus_game().await?;
        self.emit_update(LoginRunStatus::Inputting, message, None)?;
        crate::input_simulation::press_named_key_cancellable(named_key, Arc::clone(&cancelled))
            .await?;
        crate::input_simulation::move_region_center_cancellable(
            self.config.mouse_parking_region.clone(),
            cancelled,
        )
        .await
    }

    async fn wait_delay(&self, delay_ms: u32, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        self.emit_update(
            LoginRunStatus::Waiting,
            format!("等待 {delay_ms}ms 后执行固定导航动作"),
            None,
        )?;
        wait_cancellable(
            Duration::from_millis(u64::from(delay_ms)),
            cancelled.as_ref(),
        )
        .await
    }
}

fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::SeqCst) {
        Err("游戏内导航试运行已取消".to_string())
    } else {
        Ok(())
    }
}

/// 固定等待期间每 50ms 检查取消，避免停止请求等待完整延时。
async fn wait_cancellable(duration: Duration, cancelled: &AtomicBool) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + duration;
    loop {
        ensure_not_cancelled(cancelled)?;
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(());
        }
        tokio::time::sleep((deadline - now).min(CANCEL_POLL_INTERVAL)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::{atomic::AtomicBool, Arc, Mutex},
    };

    #[derive(Default)]
    struct FakeDriver {
        waits: Mutex<VecDeque<Result<String, String>>>,
        actions: Mutex<Vec<String>>,
        delay_failure: Mutex<Option<String>>,
        cancel_on_delay: AtomicBool,
    }

    impl FakeDriver {
        fn with_waits(waits: impl IntoIterator<Item = &'static str>) -> Self {
            Self {
                waits: Mutex::new(waits.into_iter().map(|key| Ok(key.to_string())).collect()),
                actions: Mutex::new(Vec::new()),
                delay_failure: Mutex::new(None),
                cancel_on_delay: AtomicBool::new(false),
            }
        }

        fn actions(&self) -> Vec<String> {
            self.actions.lock().unwrap().clone()
        }
    }

    impl GameNavigationDriver for FakeDriver {
        async fn wait_for_any(&self, _: &[&str], _: Arc<AtomicBool>) -> Result<String, String> {
            self.waits.lock().unwrap().pop_front().unwrap()
        }

        async fn click(&self, target_key: &str, _: Arc<AtomicBool>) -> Result<(), String> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("click:{target_key}"));
            Ok(())
        }

        async fn press(&self, key: NavigationKey, _: Arc<AtomicBool>) -> Result<(), String> {
            self.actions.lock().unwrap().push(format!("key:{key:?}"));
            Ok(())
        }

        async fn wait_delay(
            &self,
            delay_ms: u32,
            cancelled: Arc<AtomicBool>,
        ) -> Result<(), String> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("delay:{delay_ms}"));
            if self.cancel_on_delay.load(Ordering::SeqCst) {
                cancelled.store(true, Ordering::SeqCst);
            }
            if let Some(error) = self.delay_failure.lock().unwrap().take() {
                return Err(error);
            }
            Ok(())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn navigation_timeout_is_distinct_from_execution_failure() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut on_step = |_| {};
        {
            let timeout = run_navigation_step(
                GameNavigationStep::WaitModeReady,
                &cancelled,
                &mut on_step,
                std::future::pending::<Result<(), String>>(),
            );
            tokio::pin!(timeout);
            tokio::task::yield_now().await;
            tokio::time::advance(STEP_TIMEOUT).await;
            assert_eq!(
                timeout.await,
                Err(GameNavigationResult::TimedOut {
                    failed_step: GameNavigationStep::WaitModeReady,
                })
            );
        }

        let execution_error = run_navigation_step(
            GameNavigationStep::OpenSpecialOps,
            &cancelled,
            &mut on_step,
            async { Err::<(), String>("游戏窗口恢复失败".to_string()) },
        )
        .await;
        assert_eq!(
            execution_error,
            Err(GameNavigationResult::Paused {
                failed_step: GameNavigationStep::OpenSpecialOps,
                message: "游戏窗口恢复失败".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn navigation_waits_after_mode_ready_before_beacon_click() {
        let driver = FakeDriver::with_waits(["game.modeReady", "game.stationGrid"]);

        let result = run_game_navigation(
            &driver,
            NavigationDestination::StationGrid,
            NavigationDelays {
                beacon_ms: 3_000,
                space_ms: 100,
                tab_ms: 200,
                special_ops_ms: 300,
            },
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await;

        assert_eq!(result, GameNavigationResult::Ready);
        assert_eq!(
            driver.actions(),
            [
                "delay:3000",
                "click:game.beaconMode",
                "delay:100",
                "key:Space",
                "delay:200",
                "key:Tab",
                "delay:300",
                "click:game.specialOps",
            ]
        );
    }

    #[tokio::test]
    async fn zero_delays_keep_full_action_order() {
        let driver = FakeDriver::with_waits(["game.modeReady", "game.stationGrid"]);

        let result = run_game_navigation(
            &driver,
            NavigationDestination::StationGrid,
            NavigationDelays {
                beacon_ms: 0,
                space_ms: 0,
                tab_ms: 0,
                special_ops_ms: 0,
            },
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await;

        assert_eq!(result, GameNavigationResult::Ready);
        assert_eq!(
            driver.actions(),
            [
                "delay:0",
                "click:game.beaconMode",
                "delay:0",
                "key:Space",
                "delay:0",
                "key:Tab",
                "delay:0",
                "click:game.specialOps"
            ]
        );
    }

    #[tokio::test]
    async fn lobby_destination_stops_after_tab() {
        let driver = FakeDriver::with_waits(["game.modeReady"]);
        let result = run_game_navigation(
            &driver,
            NavigationDestination::Lobby,
            NavigationDelays {
                beacon_ms: 0,
                space_ms: 0,
                tab_ms: 0,
                special_ops_ms: 0,
            },
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await;

        assert_eq!(result, GameNavigationResult::Ready);
        assert_eq!(
            driver.actions(),
            [
                "delay:0",
                "click:game.beaconMode",
                "delay:0",
                "key:Space",
                "delay:0",
                "key:Tab",
            ]
        );
    }

    #[tokio::test]
    async fn cancellation_during_fixed_delay_stops_before_next_input() {
        let driver = FakeDriver::with_waits(["game.modeReady", "game.stationGrid"]);
        driver.cancel_on_delay.store(true, Ordering::SeqCst);

        let result = run_game_navigation(
            &driver,
            NavigationDestination::StationGrid,
            NavigationDelays {
                beacon_ms: 3000,
                space_ms: 3000,
                tab_ms: 3000,
                special_ops_ms: 3000,
            },
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await;

        assert_eq!(result, GameNavigationResult::EmergencyStopped);
        assert_eq!(driver.actions(), ["delay:3000"]);
    }
}
