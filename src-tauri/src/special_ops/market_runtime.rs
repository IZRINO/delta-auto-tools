use serde::{Deserialize, Serialize};
use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use super::market_purchase::{parse_market_price, price_decision, PriceDecision};
use crate::morse::types::RegionRect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MarketRunError {
    Cancelled,
    System { step: String, message: String },
}

#[derive(Debug, Clone)]
pub(crate) struct MarketRunConfig {
    pub(crate) product_point: RegionRect,
    pub(crate) max_price: u64,
    pub(crate) target_count: u32,
    pub(crate) completed_count: u32,
    pub(crate) entry_delay: Duration,
    pub(crate) ocr_interval: Duration,
    /// 交易行开放结束时间（分钟，从 0 点起算）。
    pub(crate) window_end_minute: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MarketAtomicResult {
    Purchased { completed_count: u32 },
    Returned,
    OcrFailedPage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MarketRunStop {
    Completed,
    YieldedForCraft,
    PauseRequested,
    WindowClosed,
    PriceRecognitionFailed,
    EmergencyStopped,
    SystemFailure { step: String, message: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MarketTrialMode {
    InspectOnly,
    RealSingleAttempt,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MarketTrialAction {
    Buy,
    Return,
    OcrFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketTrialResult {
    pub raw_text: String,
    pub parsed_price: Option<u64>,
    pub max_price: u64,
    pub action: MarketTrialAction,
}

#[allow(async_fn_in_trait)]
pub(crate) trait MarketDriver: Send + Sync {
    async fn click(
        &self,
        key: &str,
        countdown: bool,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), MarketRunError>;
    async fn click_point(
        &self,
        point: &RegionRect,
        countdown: bool,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), MarketRunError>;
    async fn read_price(&self, cancelled: Arc<AtomicBool>) -> Result<String, MarketRunError>;
    async fn delay(
        &self,
        duration: Duration,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), MarketRunError>;
    fn persist_purchase_click(&self) -> Result<u32, MarketRunError>;
    fn minute_of_day(&self) -> u16;
    fn pause_requested(&self) -> bool;
    /// 当前已到期制作任务里最晚的计划时间，没有到期任务时为 `None`。
    /// 取 max 而非 min：让位判定要回答“有没有**新**制作到期”，新完成的制作台
    /// 计划时间必然晚于开跑前就已逾期的那些，min 会被陈旧任务永久钉住。
    fn latest_due_craft_at_ms(&self) -> Option<i64>;
}

fn stopped(error: MarketRunError) -> MarketRunStop {
    match error {
        MarketRunError::Cancelled => MarketRunStop::EmergencyStopped,
        MarketRunError::System { step, message } => MarketRunStop::SystemFailure { step, message },
    }
}

pub(crate) async fn run_market_atomic<D: MarketDriver + ?Sized>(
    driver: &D,
    config: &MarketRunConfig,
    cancelled: Arc<AtomicBool>,
) -> Result<MarketAtomicResult, MarketRunError> {
    if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(MarketRunError::Cancelled);
    }
    driver
        .click_point(&config.product_point, false, Arc::clone(&cancelled))
        .await?;
    let mut price = None;
    for attempt in 0..3 {
        let text = driver.read_price(Arc::clone(&cancelled)).await?;
        if let Some(value) = parse_market_price(&text) {
            price = Some(value);
            break;
        }
        if attempt < 2 {
            driver
                .delay(config.ocr_interval, Arc::clone(&cancelled))
                .await?;
        }
    }
    let Some(price) = price else {
        driver.click("market.return", false, cancelled).await?;
        return Ok(MarketAtomicResult::OcrFailedPage);
    };
    match price_decision(price, config.max_price) {
        PriceDecision::Return => {
            driver.click("market.return", false, cancelled).await?;
            Ok(MarketAtomicResult::Returned)
        }
        PriceDecision::Buy => {
            driver
                .click("market.buy", false, Arc::clone(&cancelled))
                .await?;
            driver
                .click("market.confirm", false, Arc::clone(&cancelled))
                .await?;
            let completed_count = driver.persist_purchase_click()?;
            // 确认购买后点击高价返回，回到商品列表继续下一件。
            driver.click("market.return", false, cancelled).await?;
            Ok(MarketAtomicResult::Purchased { completed_count })
        }
    }
}

pub(crate) async fn run_market_trial<D: MarketDriver + ?Sized>(
    driver: &D,
    config: &MarketRunConfig,
    mode: MarketTrialMode,
    cancelled: Arc<AtomicBool>,
) -> Result<MarketTrialResult, MarketRunError> {
    if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(MarketRunError::Cancelled);
    }
    driver
        .click("market.entry", true, Arc::clone(&cancelled))
        .await?;
    driver
        .delay(config.entry_delay, Arc::clone(&cancelled))
        .await?;
    driver
        .click_point(&config.product_point, false, Arc::clone(&cancelled))
        .await?;

    let mut raw_text = String::new();
    let mut parsed_price = None;
    for attempt in 0..3 {
        raw_text = driver.read_price(Arc::clone(&cancelled)).await?;
        parsed_price = parse_market_price(&raw_text);
        if parsed_price.is_some() {
            break;
        }
        if attempt < 2 {
            driver
                .delay(config.ocr_interval, Arc::clone(&cancelled))
                .await?;
        }
    }
    let action = parsed_price.map_or(MarketTrialAction::OcrFailed, |price| {
        match price_decision(price, config.max_price) {
            PriceDecision::Buy => MarketTrialAction::Buy,
            PriceDecision::Return => MarketTrialAction::Return,
        }
    });

    if mode == MarketTrialMode::RealSingleAttempt {
        match action {
            MarketTrialAction::Buy => {
                driver
                    .click("market.buy", false, Arc::clone(&cancelled))
                    .await?;
                driver
                    .click("market.confirm", false, Arc::clone(&cancelled))
                    .await?;
                driver.persist_purchase_click()?;
                driver.click("market.return", false, cancelled).await?;
            }
            MarketTrialAction::Return | MarketTrialAction::OcrFailed => {
                driver.click("market.return", false, cancelled).await?;
            }
        }
    }

    Ok(MarketTrialResult {
        raw_text,
        parsed_price,
        max_price: config.max_price,
        action,
    })
}

pub(crate) async fn run_market<D: MarketDriver + ?Sized>(
    driver: &D,
    config: MarketRunConfig,
    cancelled: Arc<AtomicBool>,
) -> MarketRunStop {
    if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
        return MarketRunStop::EmergencyStopped;
    }
    if driver.minute_of_day() >= config.window_end_minute {
        return MarketRunStop::WindowClosed;
    }
    if config.completed_count >= config.target_count {
        return MarketRunStop::Completed;
    }
    if let Err(error) = driver
        .click("market.entry", true, Arc::clone(&cancelled))
        .await
    {
        return stopped(error);
    }
    if let Err(error) = driver
        .delay(config.entry_delay, Arc::clone(&cancelled))
        .await
    {
        return stopped(error);
    }

    // 入口点击与固定等待之后再取基线：这段时间本身可能有制作到期，算进基线才不会
    // 在第一件商品后就误判成“新到期”。
    let due_craft_baseline_ms = driver.latest_due_craft_at_ms();
    let mut completed_count = config.completed_count;
    let mut consecutive_failed_pages = 0u8;
    loop {
        match run_market_atomic(driver, &config, Arc::clone(&cancelled)).await {
            Ok(MarketAtomicResult::Purchased {
                completed_count: persisted,
            }) => {
                completed_count = persisted;
                consecutive_failed_pages = 0;
            }
            Ok(MarketAtomicResult::Returned) => consecutive_failed_pages = 0,
            Ok(MarketAtomicResult::OcrFailedPage) => {
                consecutive_failed_pages += 1;
                if consecutive_failed_pages >= 3 {
                    return MarketRunStop::PriceRecognitionFailed;
                }
            }
            Err(error) => return stopped(error),
        }

        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return MarketRunStop::EmergencyStopped;
        }
        if driver.pause_requested() {
            return MarketRunStop::PauseRequested;
        }
        if driver.minute_of_day() >= config.window_end_minute {
            return MarketRunStop::WindowClosed;
        }
        if completed_count >= config.target_count {
            return MarketRunStop::Completed;
        }
        // 只让位给开跑之后**新**到期的制作。开跑前就已逾期的制作是本轮队列自己的
        // 业务（交易行跑在同账号 craft 之后，其他账号的到期制作还排在后面），
        // 拿它当让位理由会让每买一件就退出换号 -> 购买次数永远停在 1。
        if driver.latest_due_craft_at_ms() > due_craft_baseline_ms {
            return MarketRunStop::YieldedForCraft;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            atomic::{AtomicBool, AtomicI64, AtomicU16, AtomicU32, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };

    use super::{
        run_market, run_market_atomic, run_market_trial, MarketAtomicResult, MarketDriver,
        MarketRunConfig, MarketRunError, MarketRunStop, MarketTrialAction, MarketTrialMode,
    };
    use crate::morse::types::RegionRect;

    #[derive(Clone, Copy)]
    enum Boundary {
        None,
        Pause,
        Close,
        Craft,
    }

    struct FakeDriver {
        ocr: Mutex<VecDeque<Result<String, MarketRunError>>>,
        actions: Mutex<Vec<String>>,
        completed: AtomicU32,
        minute: AtomicU16,
        now_ms: AtomicI64,
        craft_at_ms: AtomicI64,
        pause: AtomicBool,
        boundary_after_purchase: Boundary,
    }

    impl FakeDriver {
        fn with_ocr(values: impl IntoIterator<Item = &'static str>) -> Self {
            Self {
                ocr: Mutex::new(
                    values
                        .into_iter()
                        .map(|value| Ok(value.to_string()))
                        .collect(),
                ),
                actions: Mutex::new(Vec::new()),
                completed: AtomicU32::new(0),
                minute: AtomicU16::new(2 * 60),
                now_ms: AtomicI64::new(1_000),
                craft_at_ms: AtomicI64::new(-1),
                pause: AtomicBool::new(false),
                boundary_after_purchase: Boundary::None,
            }
        }

        fn boundary(boundary: Boundary) -> Self {
            Self {
                boundary_after_purchase: boundary,
                ..Self::with_ocr(["100"])
            }
        }

        fn clicks(&self) -> Vec<String> {
            self.actions
                .lock()
                .unwrap()
                .iter()
                .filter(|action| action.starts_with("click:"))
                .cloned()
                .collect()
        }

        fn apply_boundary(&self) {
            match self.boundary_after_purchase {
                Boundary::None => {}
                Boundary::Pause => self.pause.store(true, Ordering::SeqCst),
                Boundary::Close => self.minute.store(24 * 60, Ordering::SeqCst),
                Boundary::Craft => self.craft_at_ms.store(1_000, Ordering::SeqCst),
            }
        }
    }

    impl MarketDriver for FakeDriver {
        async fn click(
            &self,
            key: &str,
            _countdown: bool,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), MarketRunError> {
            self.actions.lock().unwrap().push(format!("click:{key}"));
            Ok(())
        }

        async fn click_point(
            &self,
            _point: &RegionRect,
            _countdown: bool,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), MarketRunError> {
            self.actions
                .lock()
                .unwrap()
                .push("click:market.product".to_string());
            Ok(())
        }

        async fn read_price(&self, _cancelled: Arc<AtomicBool>) -> Result<String, MarketRunError> {
            self.actions.lock().unwrap().push("ocr".to_string());
            self.ocr
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(String::new()))
        }

        async fn delay(
            &self,
            _duration: Duration,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), MarketRunError> {
            Ok(())
        }

        fn persist_purchase_click(&self) -> Result<u32, MarketRunError> {
            let completed = self.completed.fetch_add(1, Ordering::SeqCst) + 1;
            self.actions
                .lock()
                .unwrap()
                .push(format!("persist:{completed}"));
            self.apply_boundary();
            Ok(completed)
        }

        fn minute_of_day(&self) -> u16 {
            self.minute.load(Ordering::SeqCst)
        }

        fn pause_requested(&self) -> bool {
            self.pause.load(Ordering::SeqCst)
        }

        fn latest_due_craft_at_ms(&self) -> Option<i64> {
            let value = self.craft_at_ms.load(Ordering::SeqCst);
            let now_ms = self.now_ms.load(Ordering::SeqCst);
            (value >= 0 && value <= now_ms).then_some(value)
        }
    }

    fn config(target_count: u32) -> MarketRunConfig {
        MarketRunConfig {
            product_point: RegionRect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            },
            max_price: 12_000,
            target_count,
            completed_count: 0,
            entry_delay: Duration::ZERO,
            ocr_interval: Duration::ZERO,
            window_end_minute: 20 * 60,
        }
    }

    #[tokio::test]
    async fn high_price_returns_without_incrementing() {
        let driver = FakeDriver::with_ocr(["12,001"]);

        let result = run_market_atomic(&driver, &config(1), Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert_eq!(result, MarketAtomicResult::Returned);
        assert_eq!(
            driver.clicks(),
            ["click:market.product", "click:market.return"]
        );
        assert_eq!(driver.completed.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn equal_price_buys_and_persists_after_final_click() {
        let driver = FakeDriver::with_ocr(["12,000"]);

        let result = run_market_atomic(&driver, &config(1), Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert_eq!(result, MarketAtomicResult::Purchased { completed_count: 1 });
        assert_eq!(
            driver.clicks(),
            [
                "click:market.product",
                "click:market.buy",
                "click:market.confirm",
                "click:market.return",
            ]
        );
        assert_eq!(driver.completed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn inspect_trial_reads_price_without_clicking_decision_or_persisting() {
        let driver = FakeDriver::with_ocr(["12,000"]);

        let result = run_market_trial(
            &driver,
            &config(10),
            MarketTrialMode::InspectOnly,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();

        assert_eq!(result.raw_text, "12,000");
        assert_eq!(result.parsed_price, Some(12_000));
        assert_eq!(result.action, MarketTrialAction::Buy);
        assert_eq!(
            driver.clicks(),
            ["click:market.entry", "click:market.product"]
        );
        assert_eq!(driver.completed.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn real_trial_executes_exactly_one_price_branch() {
        let driver = FakeDriver::with_ocr(["12,001"]);

        let result = run_market_trial(
            &driver,
            &config(10),
            MarketTrialMode::RealSingleAttempt,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();

        assert_eq!(result.action, MarketTrialAction::Return);
        assert_eq!(
            driver.clicks(),
            [
                "click:market.entry",
                "click:market.product",
                "click:market.return",
            ]
        );
        assert_eq!(driver.completed.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn three_failed_pages_end_current_account_after_nine_reads() {
        let driver = FakeDriver::with_ocr(std::iter::repeat_n("无价格", 9));

        let stop = run_market(&driver, config(1), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(stop, MarketRunStop::PriceRecognitionFailed);
        assert_eq!(
            driver
                .actions
                .lock()
                .unwrap()
                .iter()
                .filter(|action| action.as_str() == "ocr")
                .count(),
            9
        );
        assert_eq!(
            driver
                .clicks()
                .iter()
                .filter(|action| action.as_str() == "click:market.return")
                .count(),
            3
        );
    }

    #[tokio::test]
    async fn completed_atomic_flow_observes_pause_boundary() {
        let driver = FakeDriver::boundary(Boundary::Pause);
        assert_eq!(
            run_market(&driver, config(2), Arc::new(AtomicBool::new(false))).await,
            MarketRunStop::PauseRequested
        );
    }

    #[tokio::test]
    async fn completed_atomic_flow_stops_when_window_closes() {
        let driver = FakeDriver::boundary(Boundary::Close);
        assert_eq!(
            run_market(&driver, config(2), Arc::new(AtomicBool::new(false))).await,
            MarketRunStop::WindowClosed
        );
    }

    #[tokio::test]
    async fn completed_atomic_flow_yields_for_due_craft() {
        let driver = FakeDriver::boundary(Boundary::Craft);
        assert_eq!(
            run_market(&driver, config(2), Arc::new(AtomicBool::new(false))).await,
            MarketRunStop::YieldedForCraft
        );
    }

    #[tokio::test]
    async fn craft_already_due_at_start_does_not_cut_the_purchase_count() {
        // 开跑前就逾期的制作是本轮队列自己的业务：其他账号的到期制作排在交易行之后，
        // 拿它让位会让账号每买一件就退出换号，配置的 3 次永远停在 1 次。
        let driver = FakeDriver::with_ocr(["100", "100", "100"]);
        driver.craft_at_ms.store(500, Ordering::SeqCst);

        let stop = run_market(&driver, config(3), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(stop, MarketRunStop::Completed);
        assert_eq!(driver.completed.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn craft_newly_due_during_loop_still_yields() {
        // 陈旧到期任务不让位，但跑动过程中新完成的制作台必须仍能抢回控制权。
        let driver = FakeDriver {
            boundary_after_purchase: Boundary::Craft,
            ..FakeDriver::with_ocr(["100", "100", "100"])
        };
        driver.craft_at_ms.store(500, Ordering::SeqCst);

        let stop = run_market(&driver, config(3), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(stop, MarketRunStop::YieldedForCraft);
        assert_eq!(driver.completed.load(Ordering::SeqCst), 1);
    }
}
