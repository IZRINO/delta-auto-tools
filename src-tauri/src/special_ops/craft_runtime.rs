//! 制作台 runtime 驱动：把模板观察与键鼠动作接到纯流程。

use super::{
    craft_trial::CraftTrialFailure,
    template_observer::{RuntimeSimilaritySampler, RuntimeTarget, SingleConsistency},
    StationKind,
};
use super::{
    desktop_runtime::{DesktopRuntime, WindowsDesktopRuntime},
    login_runtime::{emit_run_changed, LoginRunStatus, LoginRuntime},
};
use crate::hotkey_types::NamedKey;
use crate::input_simulation;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tauri::AppHandle;

const CRAFT_RECOGNITION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CraftButton {
    Produce,
    Fill,
    Purchase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CraftStationOutcome {
    StillInProgress,
    Started { started_at_ms: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CraftProbeDelays {
    pub space_ms: u32,
    pub reopen_ms: u32,
    pub confirm_pinned_ms: u32,
}

#[allow(async_fn_in_trait)]
pub(crate) trait CraftTrialDriver: Send + Sync {
    fn update_stage(&self, status: LoginRunStatus, message: &str) -> Result<(), String>;
    async fn click_unverified(
        &self,
        target_key: &str,
        countdown: bool,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), String>;
    async fn fixed_delay(&self, delay_ms: u32, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn press_space_without_countdown(&self, cancelled: Arc<AtomicBool>)
        -> Result<(), String>;
    async fn inspect_abort_once(
        &self,
        cancelled: Arc<AtomicBool>,
    ) -> Result<SingleConsistency, String>;
    async fn wait_ready(&self, target_key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn click(&self, target_key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn wait_button(
        &self,
        buttons: &[CraftButton],
        cancelled: Arc<AtomicBool>,
    ) -> Result<CraftButton, String>;
    async fn wait_abort(&self, cancelled: Arc<AtomicBool>) -> Result<i64, String>;
}

pub(crate) struct ProductionCraftTrialDriver {
    app: AppHandle,
    runtime: Arc<LoginRuntime>,
    run_id: u64,
    game_executable_path: PathBuf,
    mouse_parking_region: crate::morse::types::RegionRect,
    targets: HashMap<String, RuntimeTarget>,
    input_started: Arc<AtomicBool>,
}

pub(crate) struct CraftRunConfig {
    pub game_executable_path: PathBuf,
    pub mouse_parking_region: crate::morse::types::RegionRect,
    pub targets: HashMap<String, RuntimeTarget>,
    pub delays: CraftProbeDelays,
}

fn update_craft_stage(
    runtime: &LoginRuntime,
    run_id: u64,
    status: LoginRunStatus,
    message: impl Into<String>,
) -> Result<Option<super::LoginRunSnapshot>, String> {
    runtime.update(run_id, status, None, message, None)
}

impl ProductionCraftTrialDriver {
    pub(crate) fn new(
        app: AppHandle,
        runtime: Arc<LoginRuntime>,
        run_id: u64,
        game_executable_path: PathBuf,
        mouse_parking_region: crate::morse::types::RegionRect,
        targets: HashMap<String, RuntimeTarget>,
    ) -> Self {
        Self {
            app,
            runtime,
            run_id,
            game_executable_path,
            mouse_parking_region,
            targets,
            input_started: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn input_started(&self) -> bool {
        self.input_started.load(Ordering::SeqCst)
    }

    fn emit_update(
        &self,
        status: LoginRunStatus,
        message: impl Into<String>,
    ) -> Result<(), String> {
        if let Some(snapshot) = update_craft_stage(&self.runtime, self.run_id, status, message)? {
            emit_run_changed(&self.app, &snapshot);
        }
        Ok(())
    }

    async fn countdown(
        &self,
        show_subsequent_countdown: bool,
        cancelled: &Arc<AtomicBool>,
    ) -> Result<(), String> {
        let Some(total) = self
            .runtime
            .next_input_countdown_seconds(self.run_id, show_subsequent_countdown)?
        else {
            return Ok(());
        };
        for seconds in (1..=total).rev() {
            ensure_not_cancelled(cancelled)?;
            if let Some(snapshot) = self.runtime.update(
                self.run_id,
                LoginRunStatus::Countdown,
                None,
                format!("{seconds} 秒后执行键鼠操作"),
                Some(seconds),
            )? {
                emit_run_changed(&self.app, &snapshot);
            }
            wait_cancellable(Duration::from_secs(1), cancelled).await?;
        }
        Ok(())
    }

    async fn verify(&self, key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        let target = self
            .targets
            .get(key)
            .ok_or_else(|| format!("制作校准目标 {key} 不存在"))?;
        if target.template.is_some() {
            super::template_observer::wait_for_target_match_until(
                &RuntimeSimilaritySampler,
                target,
                cancelled,
                CRAFT_RECOGNITION_TIMEOUT,
            )
            .await
            .map(|_| ())
        } else if target.guard_any_of.is_empty() {
            Err(format!("制作动作 {key} 缺少识别守卫"))
        } else {
            let templates = target
                .guard_any_of
                .iter()
                .map(|guard| {
                    self.targets
                        .get(guard)
                        .and_then(|item| item.template.as_ref())
                        .ok_or_else(|| format!("制作识别守卫 {guard} 未配置模板"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            super::template_observer::wait_for_any_consistent_match_until(
                &RuntimeSimilaritySampler,
                &templates,
                cancelled,
                CRAFT_RECOGNITION_TIMEOUT,
            )
            .await
            .map(|_| ())
        }
    }

    async fn focus(&self) -> Result<(), String> {
        let executable = self.game_executable_path.clone();
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

    async fn perform_click(
        &self,
        key: &str,
        verify: bool,
        countdown: bool,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), String> {
        // 复核仍用 key 的识别区域，点击改用独立点击点；冻结配置缺点击点时回落识别区域。
        let click_key = super::click_target_key(key);
        let region = self
            .targets
            .get(click_key)
            .or_else(|| self.targets.get(key))
            .ok_or_else(|| format!("制作点击目标 {key} 不存在"))?
            .region
            .clone();
        self.countdown(countdown, &cancelled).await?;
        ensure_not_cancelled(&cancelled)?;
        self.emit_update(LoginRunStatus::Waiting, "正在聚焦游戏窗口")?;
        self.focus().await?;
        if verify {
            self.emit_update(LoginRunStatus::Waiting, format!("正在复核 {key}"))?;
            self.verify(key, Arc::clone(&cancelled)).await?;
        }
        self.emit_update(LoginRunStatus::Inputting, "正在执行游戏点击")?;
        mark_input_started(&self.input_started);
        input_simulation::click_region_center_held_cancellable(
            region,
            super::MOUSE_CLICK_HOLD_MS,
            Arc::clone(&cancelled),
        )
        .await?;
        input_simulation::move_region_center_cancellable(
            self.mouse_parking_region.clone(),
            cancelled,
        )
        .await
    }
}

impl CraftTrialDriver for ProductionCraftTrialDriver {
    fn update_stage(&self, status: LoginRunStatus, message: &str) -> Result<(), String> {
        self.emit_update(status, message)
    }

    async fn click_unverified(
        &self,
        key: &str,
        countdown: bool,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), String> {
        self.perform_click(key, false, countdown, cancelled).await
    }

    async fn fixed_delay(&self, delay_ms: u32, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        wait_cancellable(Duration::from_millis(u64::from(delay_ms)), &cancelled).await
    }

    async fn press_space_without_countdown(
        &self,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), String> {
        self.countdown(false, &cancelled).await?;
        ensure_not_cancelled(&cancelled)?;
        self.emit_update(LoginRunStatus::Waiting, "正在聚焦游戏窗口")?;
        self.focus().await?;
        self.emit_update(LoginRunStatus::Inputting, "正在按 Space")?;
        mark_input_started(&self.input_started);
        input_simulation::press_named_key_cancellable(NamedKey::Space, Arc::clone(&cancelled))
            .await?;
        input_simulation::move_region_center_cancellable(
            self.mouse_parking_region.clone(),
            cancelled,
        )
        .await
    }

    async fn inspect_abort_once(
        &self,
        cancelled: Arc<AtomicBool>,
    ) -> Result<SingleConsistency, String> {
        let template = self
            .targets
            .get("craft.abort")
            .and_then(|target| target.template.as_ref())
            .ok_or_else(|| "制作中止目标未配置模板".to_string())?;
        super::template_observer::sample_single_consistent_once(
            &RuntimeSimilaritySampler,
            template,
            cancelled,
        )
        .await
    }

    async fn wait_ready(&self, key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        let target = self
            .targets
            .get(key)
            .ok_or_else(|| format!("制作识别目标 {key} 不存在"))?;
        let template = target
            .template
            .as_ref()
            .ok_or_else(|| format!("制作识别目标 {key} 未配置模板"))?;
        super::template_observer::wait_for_target_match_until(
            &RuntimeSimilaritySampler,
            &RuntimeTarget {
                key: target.key.clone(),
                region: target.region.clone(),
                template: Some(template.clone()),
                guard_any_of: Vec::new(),
            },
            cancelled,
            CRAFT_RECOGNITION_TIMEOUT,
        )
        .await
        .map(|_| ())
    }
    async fn click(&self, key: &str, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        self.perform_click(key, true, true, cancelled).await
    }
    async fn wait_button(
        &self,
        buttons: &[CraftButton],
        cancelled: Arc<AtomicBool>,
    ) -> Result<CraftButton, String> {
        let keys = buttons
            .iter()
            .map(|button| match button {
                CraftButton::Produce => "craft.produce",
                CraftButton::Fill => "craft.fill",
                CraftButton::Purchase => "craft.purchase",
            })
            .collect::<Vec<_>>();
        let templates = keys
            .iter()
            .map(|key| {
                self.targets
                    .get(*key)
                    .and_then(|target| target.template.as_ref())
                    .ok_or_else(|| format!("制作按钮 {key} 未配置模板"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (key, _) = super::template_observer::wait_for_any_consistent_match_until(
            &RuntimeSimilaritySampler,
            &templates,
            cancelled,
            CRAFT_RECOGNITION_TIMEOUT,
        )
        .await?;
        buttons
            .iter()
            .find(|button| match button {
                CraftButton::Produce => key == "craft.produce",
                CraftButton::Fill => key == "craft.fill",
                CraftButton::Purchase => key == "craft.purchase",
            })
            .copied()
            .ok_or_else(|| "未识别制作按钮".to_string())
    }
    async fn wait_abort(&self, cancelled: Arc<AtomicBool>) -> Result<i64, String> {
        let target = self
            .targets
            .get("craft.abort")
            .ok_or_else(|| "制作中止目标不存在".to_string())?;
        let template = target
            .template
            .as_ref()
            .ok_or_else(|| "制作中止目标未配置模板".to_string())?;
        let runtime_target = RuntimeTarget {
            key: target.key.clone(),
            region: target.region.clone(),
            template: Some(template.clone()),
            guard_any_of: Vec::new(),
        };
        super::template_observer::wait_for_target_match_until(
            &RuntimeSimilaritySampler,
            &runtime_target,
            cancelled,
            CRAFT_RECOGNITION_TIMEOUT,
        )
        .await?;
        Ok(crate::special_ops::now_ms())
    }
}

async fn wait_cancellable(duration: Duration, cancelled: &AtomicBool) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + duration;
    while tokio::time::Instant::now() < deadline {
        ensure_not_cancelled(cancelled)?;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Ok(())
}

fn mark_input_started(input_started: &AtomicBool) {
    input_started.store(true, Ordering::SeqCst);
}

pub(crate) async fn ensure_station_grid<D: CraftTrialDriver + ?Sized>(
    driver: &D,
    cancelled: Arc<AtomicBool>,
) -> Result<(), CraftTrialFailure> {
    driver
        .wait_ready("game.stationGrid", cancelled)
        .await
        .map_err(|message| failure("game.stationGrid", &message))
}

pub(crate) async fn return_to_station_grid<D: CraftTrialDriver + ?Sized>(
    driver: &D,
    cancelled: Arc<AtomicBool>,
) -> Result<(), CraftTrialFailure> {
    driver
        .click_unverified("craft.returnToStationGrid", false, Arc::clone(&cancelled))
        .await
        .map_err(|message| failure_after_input("craft.returnToStationGrid", &message))?;
    driver
        .wait_ready("game.stationGrid", cancelled)
        .await
        .map_err(|message| failure_after_input("game.stationGrid", &message))
}

/// 执行单台收取并重做。状态成功后由调用方持久化开始/完成时间。
pub(crate) async fn run_craft_station<D: CraftTrialDriver + ?Sized>(
    driver: &D,
    station: StationKind,
    delays: CraftProbeDelays,
    cancelled: Arc<AtomicBool>,
) -> Result<CraftStationOutcome, CraftTrialFailure> {
    let label = station_label(&station);
    let suffix = suffix(station);
    let station_key = format!("craft.station.{suffix}");
    let recipe_key = format!("craft.recipe.{suffix}");

    driver
        .update_stage(LoginRunStatus::Waiting, &format!("正在探测{label}当前状态"))
        .map_err(|message| failure(&station_key, &message))?;
    driver
        .click_unverified(&station_key, false, Arc::clone(&cancelled))
        .await
        .map_err(|message| failure_after_input(&station_key, &message))?;
    driver
        .fixed_delay(delays.space_ms, Arc::clone(&cancelled))
        .await
        .map_err(|message| failure_after_input("craft.spaceDelay", &message))?;
    driver
        .press_space_without_countdown(Arc::clone(&cancelled))
        .await
        .map_err(|message| failure_after_input("craft.space", &message))?;
    driver
        .fixed_delay(delays.reopen_ms, Arc::clone(&cancelled))
        .await
        .map_err(|message| failure_after_input("craft.reopenDelay", &message))?;
    driver
        .click_unverified(&station_key, false, Arc::clone(&cancelled))
        .await
        .map_err(|message| failure_after_input(&station_key, &message))?;
    driver
        .fixed_delay(delays.confirm_pinned_ms, Arc::clone(&cancelled))
        .await
        .map_err(|message| failure_after_input("craft.confirmPinnedDelay", &message))?;
    driver
        .click_unverified("craft.confirmPinned", false, Arc::clone(&cancelled))
        .await
        .map_err(|message| failure_after_input("craft.confirmPinned", &message))?;
    driver
        .update_stage(LoginRunStatus::Waiting, "正在检查中止按钮")
        .map_err(|message| failure_after_input("craft.abort", &message))?;
    match driver
        .inspect_abort_once(Arc::clone(&cancelled))
        .await
        .map_err(|message| failure_after_input("craft.abort", &message))?
    {
        SingleConsistency::Matched { .. } => {
            return_to_station_grid(driver, cancelled).await?;
            return Ok(CraftStationOutcome::StillInProgress);
        }
        SingleConsistency::NotMatched { .. } => driver
            .click_unverified(&recipe_key, true, Arc::clone(&cancelled))
            .await
            .map_err(|message| failure_after_input(&recipe_key, &message))?,
    }

    driver
        .update_stage(LoginRunStatus::Waiting, "正在确认生产或一键补齐按钮")
        .map_err(|message| failure_after_input("craft.produce", &message))?;

    match driver
        .wait_button(
            &[CraftButton::Produce, CraftButton::Fill],
            Arc::clone(&cancelled),
        )
        .await
    {
        Ok(CraftButton::Produce) => driver
            .click("craft.produce", Arc::clone(&cancelled))
            .await
            .map_err(|message| failure_after_input("craft.produce", &message))?,
        Ok(CraftButton::Fill) => {
            driver
                .click("craft.fill", Arc::clone(&cancelled))
                .await
                .map_err(|message| failure_after_input("craft.fill", &message))?;
            driver
                .update_stage(LoginRunStatus::Waiting, "正在确认购买材料页面")
                .map_err(|message| failure_after_input("craft.purchase", &message))?;
            driver
                .wait_ready("craft.purchase", Arc::clone(&cancelled))
                .await
                .map_err(|message| failure_after_input("craft.purchase", &message))?;
            for attempt in 0..3 {
                driver
                    .click("craft.purchase", Arc::clone(&cancelled))
                    .await
                    .map_err(|message| failure_after_input("craft.purchase", &message))?;
                driver
                    .fixed_delay(1_000, Arc::clone(&cancelled))
                    .await
                    .map_err(|message| failure_after_input("craft.purchaseDelay", &message))?;
                driver
                    .update_stage(LoginRunStatus::Waiting, "正在确认购买材料结果")
                    .map_err(|message| failure_after_input("craft.purchase", &message))?;
                match driver
                    .wait_button(
                        &[
                            CraftButton::Produce,
                            CraftButton::Fill,
                            CraftButton::Purchase,
                        ],
                        Arc::clone(&cancelled),
                    )
                    .await
                {
                    Ok(CraftButton::Produce) => {
                        driver
                            .click("craft.produce", Arc::clone(&cancelled))
                            .await
                            .map_err(|message| failure_after_input("craft.produce", &message))?;
                        break;
                    }
                    Ok(CraftButton::Fill) => {
                        if attempt >= 2 {
                            return Err(isolated_failure(
                                "材料购买重试 3 次后仍回到一键补齐，按仓库空间不足隔离账号",
                            ));
                        }
                        driver
                            .click("craft.fill", Arc::clone(&cancelled))
                            .await
                            .map_err(|message| failure_after_input("craft.fill", &message))?;
                        driver
                            .wait_ready("craft.purchase", Arc::clone(&cancelled))
                            .await
                            .map_err(|message| failure_after_input("craft.purchase", &message))?;
                    }
                    Ok(CraftButton::Purchase) if attempt < 2 => {}
                    Ok(CraftButton::Purchase) => {
                        return Err(isolated_failure(
                            "材料购买重试 3 次后仍停留在购买页面，按仓库空间不足隔离账号",
                        ));
                    }
                    Err(message) => return Err(failure_after_input("craft.purchase", &message)),
                }
            }
        }
        Ok(CraftButton::Purchase) => {
            return Err(failure_after_input(
                "craft.produce",
                "生产前错误命中购买材料按钮",
            ));
        }
        Err(message) => return Err(failure_after_input("craft.produce", &message)),
    }
    driver
        .update_stage(LoginRunStatus::Waiting, "正在确认制作已开始")
        .map_err(|message| failure_after_input("craft.abort", &message))?;
    let started_at_ms = driver
        .wait_abort(cancelled)
        .await
        .map_err(|message| failure_after_input("craft.abort", &message))?;
    Ok(CraftStationOutcome::Started { started_at_ms })
}

fn station_label(station: &StationKind) -> &'static str {
    match station {
        StationKind::TechnicalCenter => "技术中心",
        StationKind::Workbench => "工作台",
        StationKind::Pharmacy => "制药台",
        StationKind::ArmorBench => "防具台",
    }
}

fn suffix(station: StationKind) -> &'static str {
    match station {
        StationKind::TechnicalCenter => "technicalCenter",
        StationKind::Workbench => "workbench",
        StationKind::Pharmacy => "pharmacy",
        StationKind::ArmorBench => "armorBench",
    }
}

fn failure(step: &str, message: &str) -> CraftTrialFailure {
    CraftTrialFailure {
        step: step.to_string(),
        message: message.to_string(),
        requires_uncertain: false,
    }
}

fn failure_after_input(step: &str, message: &str) -> CraftTrialFailure {
    CraftTrialFailure {
        step: step.to_string(),
        message: message.to_string(),
        requires_uncertain: true,
    }
}

fn isolated_failure(message: &str) -> CraftTrialFailure {
    CraftTrialFailure {
        step: "craft.isolated".to_string(),
        message: message.to_string(),
        requires_uncertain: false,
    }
}

fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::SeqCst) {
        Err("制作试运行已取消".to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod fixed_probe_tests {
    use super::*;
    use std::{collections::VecDeque, sync::Mutex};

    struct FixedProbeDriver {
        actions: Mutex<Vec<String>>,
        abort: Result<SingleConsistency, String>,
        buttons: Mutex<VecDeque<Result<CraftButton, String>>>,
    }

    impl FixedProbeDriver {
        fn new(abort: Result<SingleConsistency, String>) -> Self {
            Self::with_buttons(abort, [Ok(CraftButton::Produce)])
        }

        fn with_buttons(
            abort: Result<SingleConsistency, String>,
            buttons: impl IntoIterator<Item = Result<CraftButton, String>>,
        ) -> Self {
            Self {
                actions: Mutex::new(Vec::new()),
                abort,
                buttons: Mutex::new(buttons.into_iter().collect()),
            }
        }

        fn actions(&self) -> Vec<String> {
            self.actions.lock().unwrap().clone()
        }

        fn push(&self, action: impl Into<String>) {
            self.actions.lock().unwrap().push(action.into());
        }
    }

    impl CraftTrialDriver for FixedProbeDriver {
        fn update_stage(&self, _: LoginRunStatus, _: &str) -> Result<(), String> {
            Ok(())
        }

        async fn click_unverified(
            &self,
            key: &str,
            countdown: bool,
            _: Arc<AtomicBool>,
        ) -> Result<(), String> {
            self.push(format!("unchecked:{key}:{countdown}"));
            Ok(())
        }

        async fn fixed_delay(&self, delay_ms: u32, _: Arc<AtomicBool>) -> Result<(), String> {
            self.push(format!("delay:{delay_ms}"));
            Ok(())
        }

        async fn press_space_without_countdown(&self, _: Arc<AtomicBool>) -> Result<(), String> {
            self.push("space");
            Ok(())
        }

        async fn inspect_abort_once(
            &self,
            _: Arc<AtomicBool>,
        ) -> Result<SingleConsistency, String> {
            self.push("inspect:craft.abort");
            self.abort.clone()
        }

        async fn wait_ready(&self, key: &str, _: Arc<AtomicBool>) -> Result<(), String> {
            self.push(format!("wait:{key}"));
            Ok(())
        }

        async fn click(&self, key: &str, _: Arc<AtomicBool>) -> Result<(), String> {
            self.push(format!("click:{key}"));
            Ok(())
        }

        async fn wait_button(
            &self,
            _: &[CraftButton],
            _: Arc<AtomicBool>,
        ) -> Result<CraftButton, String> {
            self.buttons
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(CraftButton::Produce))
        }

        async fn wait_abort(&self, _: Arc<AtomicBool>) -> Result<i64, String> {
            self.push("wait:craft.abort");
            Ok(100)
        }
    }

    fn delays() -> CraftProbeDelays {
        CraftProbeDelays {
            space_ms: 100,
            reopen_ms: 200,
            confirm_pinned_ms: 300,
        }
    }

    #[tokio::test]
    async fn ensure_station_grid_only_waits_for_grid() {
        let driver = FixedProbeDriver::new(Ok(SingleConsistency::Matched {
            samples: [0.9, 0.91],
        }));

        ensure_station_grid(&driver, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert_eq!(driver.actions(), ["wait:game.stationGrid"]);
    }

    #[tokio::test]
    async fn return_to_station_grid_clicks_then_waits() {
        let driver = FixedProbeDriver::new(Ok(SingleConsistency::Matched {
            samples: [0.9, 0.91],
        }));

        return_to_station_grid(&driver, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert_eq!(
            driver.actions(),
            [
                "unchecked:craft.returnToStationGrid:false",
                "wait:game.stationGrid",
            ]
        );
    }

    #[test]
    fn input_marker_changes_only_when_explicitly_marked() {
        let input_started = AtomicBool::new(false);

        assert!(!input_started.load(Ordering::SeqCst));
        mark_input_started(&input_started);
        assert!(input_started.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn matched_abort_returns_to_station_grid_without_recipe_click() {
        let driver = FixedProbeDriver::new(Ok(SingleConsistency::Matched {
            samples: [0.9, 0.91],
        }));

        let result = run_craft_station(
            &driver,
            StationKind::TechnicalCenter,
            delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();

        assert_eq!(result, CraftStationOutcome::StillInProgress);
        assert_eq!(
            driver.actions(),
            [
                "unchecked:craft.station.technicalCenter:false",
                "delay:100",
                "space",
                "delay:200",
                "unchecked:craft.station.technicalCenter:false",
                "delay:300",
                "unchecked:craft.confirmPinned:false",
                "inspect:craft.abort",
                "unchecked:craft.returnToStationGrid:false",
                "wait:game.stationGrid",
            ]
        );
        assert!(!driver.actions().iter().any(|action| action == "escape"));
    }

    #[tokio::test]
    async fn two_low_abort_samples_enter_recipe_and_start_craft() {
        let driver = FixedProbeDriver::new(Ok(SingleConsistency::NotMatched {
            samples: [0.1, 0.2],
        }));

        let result = run_craft_station(
            &driver,
            StationKind::Workbench,
            delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();

        assert_eq!(result, CraftStationOutcome::Started { started_at_ms: 100 });
        let actions = driver.actions();
        assert!(actions.contains(&"unchecked:craft.recipe.workbench:true".to_string()));
        assert!(actions.contains(&"click:craft.produce".to_string()));
        assert!(actions.contains(&"wait:craft.abort".to_string()));
    }

    #[tokio::test]
    async fn purchase_button_remaining_three_times_returns_isolated() {
        let driver = FixedProbeDriver::with_buttons(
            Ok(SingleConsistency::NotMatched {
                samples: [0.1, 0.2],
            }),
            [
                Ok(CraftButton::Fill),
                Ok(CraftButton::Purchase),
                Ok(CraftButton::Purchase),
                Ok(CraftButton::Purchase),
            ],
        );

        let error = run_craft_station(
            &driver,
            StationKind::TechnicalCenter,
            delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();

        assert_eq!(error.step, "craft.isolated");
        assert!(!error.requires_uncertain);
        assert_eq!(
            driver
                .actions()
                .iter()
                .filter(|action| *action == "click:craft.purchase")
                .count(),
            3
        );
    }

    #[tokio::test]
    async fn purchase_button_twice_then_produce_continues_crafting() {
        let driver = FixedProbeDriver::with_buttons(
            Ok(SingleConsistency::NotMatched {
                samples: [0.1, 0.2],
            }),
            [
                Ok(CraftButton::Fill),
                Ok(CraftButton::Purchase),
                Ok(CraftButton::Purchase),
                Ok(CraftButton::Produce),
            ],
        );

        let result = run_craft_station(
            &driver,
            StationKind::TechnicalCenter,
            delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();

        assert_eq!(result, CraftStationOutcome::Started { started_at_ms: 100 });
        let actions = driver.actions();
        assert_eq!(
            actions
                .iter()
                .filter(|action| *action == "click:craft.purchase")
                .count(),
            3
        );
        assert!(actions.contains(&"click:craft.produce".to_string()));
    }

    #[tokio::test]
    async fn purchase_feedback_without_stable_button_requires_uncertain() {
        let driver = FixedProbeDriver::with_buttons(
            Ok(SingleConsistency::NotMatched {
                samples: [0.1, 0.2],
            }),
            [
                Ok(CraftButton::Fill),
                Err("购买后未命中稳定按钮".to_string()),
            ],
        );

        let error = run_craft_station(
            &driver,
            StationKind::TechnicalCenter,
            delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();

        assert_eq!(error.step, "craft.purchase");
        assert!(error.requires_uncertain);
    }

    #[tokio::test]
    async fn purchase_returns_to_fill_then_reopens_and_continues_production() {
        let driver = FixedProbeDriver::with_buttons(
            Ok(SingleConsistency::NotMatched {
                samples: [0.1, 0.2],
            }),
            [
                Ok(CraftButton::Fill),
                Ok(CraftButton::Fill),
                Ok(CraftButton::Produce),
            ],
        );

        let result = run_craft_station(
            &driver,
            StationKind::TechnicalCenter,
            delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();

        assert_eq!(result, CraftStationOutcome::Started { started_at_ms: 100 });
        let actions = driver.actions();
        assert_eq!(
            actions
                .iter()
                .filter(|action| *action == "click:craft.fill")
                .count(),
            2
        );
        assert_eq!(
            actions
                .iter()
                .filter(|action| *action == "click:craft.purchase")
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn purchase_returns_to_fill_three_times_returns_isolated() {
        let driver = FixedProbeDriver::with_buttons(
            Ok(SingleConsistency::NotMatched {
                samples: [0.1, 0.2],
            }),
            [
                Ok(CraftButton::Fill),
                Ok(CraftButton::Fill),
                Ok(CraftButton::Fill),
                Ok(CraftButton::Fill),
            ],
        );

        let error = run_craft_station(
            &driver,
            StationKind::TechnicalCenter,
            delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();

        assert_eq!(error.step, "craft.isolated");
        let actions = driver.actions();
        assert_eq!(
            actions
                .iter()
                .filter(|action| *action == "click:craft.fill")
                .count(),
            3
        );
        assert_eq!(
            actions
                .iter()
                .filter(|action| *action == "click:craft.purchase")
                .count(),
            3
        );
    }

    #[tokio::test]
    async fn inconsistent_abort_samples_stop_before_recipe_and_require_uncertain() {
        let driver = FixedProbeDriver::new(Err(
            "模板 craft.abort 两次采样不一致：0.9000 / 0.1000".to_string()
        ));

        let error = run_craft_station(
            &driver,
            StationKind::Pharmacy,
            delays(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();

        assert!(error.requires_uncertain);
        assert_eq!(error.step, "craft.abort");
        assert!(!driver
            .actions()
            .iter()
            .any(|action| action.contains("craft.recipe.pharmacy")));
    }
}
