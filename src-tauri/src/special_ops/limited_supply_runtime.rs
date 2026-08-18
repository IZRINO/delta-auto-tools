use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use super::limited_supply::LimitedSupplyOutcome;

pub(crate) fn match_limited_color_sample(
    screenshots: &[image::DynamicImage],
    colors: [[u8; 3]; 2],
    tolerances: [u8; 2],
) -> Option<LimitedColorSample> {
    if screenshots.len() != 9 {
        return None;
    }
    let probes = (0..screenshots.len())
        .map(|_| crate::recognition::ColorProbe {
            region: None,
            targets: colors
                .into_iter()
                .zip(tolerances)
                .map(|(color, tolerance)| crate::recognition::ColorTarget { color, tolerance })
                .collect(),
            probe_match_mode: crate::recognition::ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        })
        .collect::<Vec<_>>();
    let regions = screenshots
        .iter()
        .zip(&probes)
        .enumerate()
        .map(|(index, (screenshot, probe))| {
            let hits = crate::recognition::watcher::probe_hit_targets(
                screenshot,
                probe,
                crate::recognition::ColorMatchMethod::AnyPixel,
                true,
            );
            let matched_indexes = hits
                .iter()
                .enumerate()
                .filter(|(_, hit)| hit.matched)
                .map(|(target_index, _)| (target_index + 1) as u8)
                .collect::<Vec<_>>();
            let matched_color = matched_indexes
                .first()
                .and_then(|&color_index| colors.get((color_index as usize) - 1).copied());
            let nearest_distance = hits
                .iter()
                .map(|hit| hit.distance)
                .fold(f32::INFINITY, f32::min);
            LimitedRegionMatch {
                region: (index + 1) as u8,
                matched_color,
                matched_indexes,
                nearest_distance,
            }
        })
        .collect();
    Some(LimitedColorSample { regions })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LimitedRunError {
    Cancelled,
    ReadyTimeout,
    System { step: String, message: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LimitedRegionMatch {
    pub(crate) region: u8,
    pub(crate) matched_color: Option<[u8; 3]>,
    pub(crate) matched_indexes: Vec<u8>,
    pub(crate) nearest_distance: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LimitedColorSample {
    pub(crate) regions: Vec<LimitedRegionMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LimitedSupplyCheckResult {
    pub(crate) outcome: LimitedSupplyOutcome,
    pub(crate) matched_region: Option<u8>,
    pub(crate) matched_color: Option<[u8; 3]>,
    pub(crate) matched_color_indexes: Vec<u8>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LimitedRunStop {
    Completed(LimitedSupplyCheckResult),
    RetryableReadyTimeout,
    EmergencyStopped,
    SystemFailure { step: String, message: String },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LimitedRunConfig {
    pub(crate) ready_timeout: Duration,
    pub(crate) sample_interval: Duration,
    /// 点完研发部门到开始识别页面之间的固定等待。研发部门点击后立刻采样，`limited.ready`
    /// 有机会在**上一页**连续命中两次 -> 识色跑在错误页面上 -> 误判高价值。
    pub(crate) enter_delay: Duration,
}

#[allow(async_fn_in_trait)]
pub(crate) trait LimitedSupplyDriver: Send + Sync {
    async fn wait_and_click(
        &self,
        key: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), LimitedRunError>;
    async fn delay(
        &self,
        duration: Duration,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), LimitedRunError>;
    async fn wait_ready(
        &self,
        timeout: Duration,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), LimitedRunError>;
    async fn sample_colors(
        &self,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Option<LimitedColorSample>, LimitedRunError>;
    fn persist_result(&self, result: &LimitedSupplyCheckResult) -> Result<(), LimitedRunError>;
}

fn stopped(error: LimitedRunError, fallback_step: &str) -> LimitedRunStop {
    match error {
        LimitedRunError::Cancelled => LimitedRunStop::EmergencyStopped,
        LimitedRunError::ReadyTimeout => LimitedRunStop::RetryableReadyTimeout,
        LimitedRunError::System { step, message } => LimitedRunStop::SystemFailure {
            step: if step.is_empty() {
                fallback_step.to_string()
            } else {
                step
            },
            message,
        },
    }
}

fn compare_samples(
    first: &LimitedColorSample,
    second: &LimitedColorSample,
) -> Option<LimitedSupplyCheckResult> {
    let first_hits = first
        .regions
        .iter()
        .filter(|region| region.matched_color.is_some())
        .map(|region| region.region)
        .collect::<std::collections::BTreeSet<_>>();
    let second_hits = second
        .regions
        .iter()
        .filter(|region| region.matched_color.is_some())
        .map(|region| region.region)
        .collect::<std::collections::BTreeSet<_>>();
    if first_hits.is_empty() && second_hits.is_empty() {
        return Some(LimitedSupplyCheckResult {
            outcome: LimitedSupplyOutcome::NoHighValue,
            matched_region: None,
            matched_color: None,
            matched_color_indexes: Vec::new(),
            error: None,
        });
    }
    // 命中集合必须两次完全相同。取交集非空就判定会让「第一次命中 1、3，第二次只命中 3」
    // 这种抖动算成一致 -> 误报高价值。
    if first_hits != second_hits {
        return None;
    }
    let region = first_hits.iter().next().copied()?;
    let color_of = |sample: &LimitedColorSample, region: u8| {
        sample
            .regions
            .iter()
            .find(|candidate| candidate.region == region)
            .and_then(|candidate| candidate.matched_color)
    };
    let indexes_of = |sample: &LimitedColorSample, region: u8| {
        sample
            .regions
            .iter()
            .find(|candidate| candidate.region == region)
            .map(|candidate| candidate.matched_indexes.clone())
            .unwrap_or_default()
    };
    for hit_region in &first_hits {
        let first_color = color_of(first, *hit_region);
        // 同一区域两次命中的目标颜色也必须相同：只读第二次的颜色会让「第一次命中颜色 1、
        // 第二次命中颜色 2」被当成稳定结果，同样是误报。
        if first_color.is_none()
            || first_color != color_of(second, *hit_region)
            || indexes_of(first, *hit_region) != indexes_of(second, *hit_region)
        {
            return None;
        }
    }
    let first_color = color_of(first, region);
    let matched_color_indexes = first_hits
        .iter()
        .flat_map(|hit_region| indexes_of(first, *hit_region))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    Some(LimitedSupplyCheckResult {
        outcome: LimitedSupplyOutcome::HighValue,
        matched_region: Some(region),
        matched_color: first_color,
        matched_color_indexes,
        error: None,
    })
}

async fn persist_or_stop<D: LimitedSupplyDriver + ?Sized>(
    driver: &D,
    result: LimitedSupplyCheckResult,
) -> LimitedRunStop {
    match driver.persist_result(&result) {
        Ok(()) => LimitedRunStop::Completed(result),
        Err(error) => stopped(error, "limited.persistResult"),
    }
}

pub(crate) async fn run_limited_supply_branch<D: LimitedSupplyDriver + ?Sized>(
    driver: &D,
    config: LimitedRunConfig,
    cancelled: Arc<AtomicBool>,
) -> LimitedRunStop {
    // 先等页面落地再开始识别：研发部门点击后立刻采样，`limited.ready` 可能在上一页
    // 连续命中两次直接放行，后面的识色就跑在错误页面上，两次采样同样稳定 ->
    // 800ms 内写下「发现高价值」，看起来就是没检查就标记。
    if let Err(error) = driver
        .delay(config.enter_delay, Arc::clone(&cancelled))
        .await
    {
        return stopped(error, "limited.enterDelay");
    }
    if let Err(error) = driver
        .wait_ready(config.ready_timeout, Arc::clone(&cancelled))
        .await
    {
        return stopped(error, "limited.ready");
    }

    let deadline = tokio::time::Instant::now() + config.ready_timeout;
    let mut previous: Option<LimitedColorSample> = None;
    while tokio::time::Instant::now() < deadline {
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return LimitedRunStop::EmergencyStopped;
        }
        match driver.sample_colors(Arc::clone(&cancelled)).await {
            Ok(Some(sample)) => {
                // 滑动窗口：比对失败时保留本次采样当下一轮的前项。`take()` 后丢弃会把
                // 采样切成互不相交的 (1,2) (3,4) 对 -> 一次抖动毁掉整对，连续一致
                // 的语义也丢了。
                if let Some(first) = previous.as_ref() {
                    if let Some(result) = compare_samples(first, &sample) {
                        return persist_or_stop(driver, result).await;
                    }
                }
                previous = Some(sample);
            }
            Ok(None) => {}
            Err(error) => return stopped(error, "limited.colors"),
        }
        if let Err(error) = driver
            .delay(config.sample_interval, Arc::clone(&cancelled))
            .await
        {
            return stopped(error, "limited.colors");
        }
    }
    persist_or_stop(
        driver,
        LimitedSupplyCheckResult {
            outcome: LimitedSupplyOutcome::Failed,
            matched_region: None,
            matched_color: None,
            matched_color_indexes: Vec::new(),
            error: Some("限时商品识色未形成连续一致结果".to_string()),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{atomic::AtomicBool, Arc, Mutex},
        time::Duration,
    };

    use super::{
        run_limited_supply_branch, LimitedColorSample, LimitedRegionMatch, LimitedRunConfig,
        LimitedRunError, LimitedRunStop, LimitedSupplyDriver,
    };
    use crate::special_ops::limited_supply::LimitedSupplyOutcome;
    use image::{DynamicImage, Rgba, RgbaImage};

    struct FakeDriver {
        samples: Mutex<VecDeque<Option<LimitedColorSample>>>,
        ready_error: Option<LimitedRunError>,
        persisted: Mutex<Vec<LimitedSupplyOutcome>>,
        actions: Mutex<Vec<String>>,
    }

    impl FakeDriver {
        fn with_samples(samples: impl IntoIterator<Item = Option<LimitedColorSample>>) -> Self {
            Self {
                samples: Mutex::new(samples.into_iter().collect()),
                ready_error: None,
                persisted: Mutex::new(Vec::new()),
                actions: Mutex::new(Vec::new()),
            }
        }

        fn ready_timeout() -> Self {
            Self {
                samples: Mutex::new(VecDeque::new()),
                ready_error: Some(LimitedRunError::ReadyTimeout),
                persisted: Mutex::new(Vec::new()),
                actions: Mutex::new(Vec::new()),
            }
        }
    }

    #[allow(async_fn_in_trait)]
    impl LimitedSupplyDriver for FakeDriver {
        async fn wait_and_click(
            &self,
            _key: &str,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), LimitedRunError> {
            Ok(())
        }

        async fn delay(
            &self,
            duration: Duration,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), LimitedRunError> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("delay:{}", duration.as_millis()));
            Ok(())
        }

        async fn wait_ready(
            &self,
            _timeout: Duration,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), LimitedRunError> {
            self.actions.lock().unwrap().push("ready".to_string());
            self.ready_error.clone().map_or(Ok(()), Err)
        }

        async fn sample_colors(
            &self,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<Option<LimitedColorSample>, LimitedRunError> {
            self.actions.lock().unwrap().push("sample".to_string());
            Ok(self.samples.lock().unwrap().pop_front().flatten())
        }

        fn persist_result(
            &self,
            result: &super::LimitedSupplyCheckResult,
        ) -> Result<(), LimitedRunError> {
            self.persisted.lock().unwrap().push(result.outcome.clone());
            Ok(())
        }
    }

    fn sample(hits: &[(u8, [u8; 3])]) -> Option<LimitedColorSample> {
        Some(LimitedColorSample {
            regions: (1..=9)
                .map(|index| {
                    hits.iter()
                        .find(|(candidate, _)| *candidate == index)
                        .map_or(
                            LimitedRegionMatch {
                                region: index,
                                matched_color: None,
                                matched_indexes: Vec::new(),
                                nearest_distance: 100.0,
                            },
                            |(_, color)| LimitedRegionMatch {
                                region: index,
                                matched_color: Some(*color),
                                matched_indexes: vec![1],
                                nearest_distance: 0.0,
                            },
                        )
                })
                .collect(),
        })
    }

    fn config() -> LimitedRunConfig {
        LimitedRunConfig {
            ready_timeout: Duration::from_secs(1),
            sample_interval: Duration::ZERO,
            enter_delay: Duration::ZERO,
        }
    }

    #[tokio::test]
    async fn branch_checks_ready_and_colors_without_repeating_entry() {
        let driver = FakeDriver::with_samples([sample(&[]), sample(&[])]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("限时商品分支应完成识色");
        };
        assert_eq!(result.outcome, LimitedSupplyOutcome::NoHighValue);
    }

    #[tokio::test]
    async fn high_value_when_consistent_regions_hit_in_two_samples() {
        let driver = FakeDriver::with_samples([
            sample(&[
                (1, [1, 1, 1]),
                (2, [1, 1, 1]),
                (3, [1, 1, 1]),
                (4, [1, 1, 1]),
                (5, [1, 1, 1]),
                (6, [1, 1, 1]),
                (7, [1, 1, 1]),
                (8, [1, 1, 1]),
                (9, [1, 1, 1]),
            ]),
            sample(&[
                (1, [1, 1, 1]),
                (2, [1, 1, 1]),
                (3, [1, 1, 1]),
                (4, [1, 1, 1]),
                (5, [1, 1, 1]),
                (6, [1, 1, 1]),
                (7, [1, 1, 1]),
                (8, [1, 1, 1]),
                (9, [1, 1, 1]),
            ]),
        ]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("预期完成");
        };
        assert_eq!(result.outcome, LimitedSupplyOutcome::HighValue);
        assert_eq!(result.matched_region, Some(1));
        assert_eq!(result.matched_color, Some([1, 1, 1]));
        assert_eq!(result.matched_color_indexes, vec![1]);
    }

    #[tokio::test]
    async fn partial_region_match_is_high_value() {
        // 任意区域两次稳定命中即判高价值，无需全部 9 个区域命中。
        let driver =
            FakeDriver::with_samples([sample(&[(2, [1, 2, 3])]), sample(&[(2, [1, 2, 3])])]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("预期完成");
        };
        assert_eq!(result.outcome, LimitedSupplyOutcome::HighValue);
        assert_eq!(result.matched_region, Some(2));
        assert_eq!(result.matched_color, Some([1, 2, 3]));
        assert_eq!(result.matched_color_indexes, vec![1]);
    }

    #[tokio::test]
    async fn same_region_with_different_colors_is_not_high_value() {
        // 同一区域两次命中不同目标颜色不是稳定结果：只读第二次颜色会误报高价值。
        let driver =
            FakeDriver::with_samples([sample(&[(2, [1, 2, 3])]), sample(&[(2, [4, 5, 6])])]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("预期完成");
        };
        assert_eq!(result.outcome, LimitedSupplyOutcome::Failed);
        assert_eq!(result.matched_region, None);
    }

    #[tokio::test]
    async fn partially_overlapping_hit_sets_are_not_high_value() {
        // 命中集合抖动（1,3 -> 3）取交集非空就判定会误报，必须要求两次集合完全相同。
        let driver = FakeDriver::with_samples([
            sample(&[(1, [1, 1, 1]), (3, [3, 3, 3])]),
            sample(&[(3, [3, 3, 3])]),
        ]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("预期完成");
        };
        assert_eq!(result.outcome, LimitedSupplyOutcome::Failed);
        assert_eq!(result.matched_region, None);
    }

    #[tokio::test]
    async fn inconsistent_samples_are_discarded_before_resampling() {
        let all_nine = &[
            (1, [3, 3, 3]),
            (2, [3, 3, 3]),
            (3, [3, 3, 3]),
            (4, [3, 3, 3]),
            (5, [3, 3, 3]),
            (6, [3, 3, 3]),
            (7, [3, 3, 3]),
            (8, [3, 3, 3]),
            (9, [3, 3, 3]),
        ];
        let driver = FakeDriver::with_samples([
            sample(&[(1, [1, 1, 1])]),
            sample(&[(2, [2, 2, 2])]),
            sample(all_nine),
            sample(all_nine),
        ]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("预期完成");
        };
        assert_eq!(result.outcome, LimitedSupplyOutcome::HighValue);
        assert_eq!(result.matched_region, Some(1));
        assert_eq!(driver.persisted.lock().unwrap().len(), 1);
    }

    /// 采样必须是滑动窗口：第 2、3 次一致就应判定。旧实现把采样切成互不相交的
    /// (1,2) (3,4) 对，第 1 次抖动会让第 2 次被整个丢掉 -> 这里只有 3 次采样时
    /// 永远凑不出一对 -> 超时写 Failed。
    #[tokio::test]
    async fn consecutive_samples_are_compared_as_sliding_window() {
        let all_nine = &[
            (1, [3, 3, 3]),
            (2, [3, 3, 3]),
            (3, [3, 3, 3]),
            (4, [3, 3, 3]),
            (5, [3, 3, 3]),
            (6, [3, 3, 3]),
            (7, [3, 3, 3]),
            (8, [3, 3, 3]),
            (9, [3, 3, 3]),
        ];
        let driver = FakeDriver::with_samples([
            sample(&[(1, [1, 1, 1])]),
            sample(all_nine),
            sample(all_nine),
        ]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("预期完成");
        };
        assert_eq!(result.outcome, LimitedSupplyOutcome::HighValue);
        assert_eq!(result.matched_region, Some(1));
    }

    /// 研发部门点击后必须先等 `enter_delay` 再识别页面，否则 `limited.ready` 有机会在
    /// 上一页连续命中两次 -> 识色跑在错误页面 -> 误报高价值。
    #[tokio::test]
    async fn enter_delay_runs_before_page_ready_check() {
        let driver = FakeDriver::with_samples([sample(&[]), sample(&[])]);

        let stop = run_limited_supply_branch(
            &driver,
            LimitedRunConfig {
                ready_timeout: Duration::from_secs(1),
                sample_interval: Duration::ZERO,
                enter_delay: Duration::from_millis(7),
            },
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert!(matches!(stop, LimitedRunStop::Completed(_)));
        let actions = driver.actions.lock().unwrap().clone();
        let delay_index = actions
            .iter()
            .position(|action| action == "delay:7")
            .expect("应在识别页面前等待 enter_delay");
        let ready_index = actions
            .iter()
            .position(|action| action == "ready")
            .expect("应识别限时商品页面");
        assert!(delay_index < ready_index, "实际顺序：{actions:?}");
    }

    #[tokio::test]
    async fn ready_timeout_requests_deferred_retry_without_persisting() {
        let driver = FakeDriver::ready_timeout();

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        assert_eq!(stop, LimitedRunStop::RetryableReadyTimeout);
        assert!(driver.persisted.lock().unwrap().is_empty());
    }

    #[test]
    fn color_sample_matches_shared_targets_by_region() {
        let screenshots = (0..9)
            .map(|index| {
                let color = match index {
                    1 => [100, 110, 120, 255],
                    6 => [201, 210, 220, 255],
                    _ => [1, 2, 3, 255],
                };
                DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba(color)))
            })
            .collect::<Vec<_>>();

        let sample = super::match_limited_color_sample(
            &screenshots,
            [[100, 110, 120], [200, 210, 220]],
            [0, 2],
        )
        .expect("九个截图应生成有效样本");

        assert_eq!(sample.regions.len(), 9);
        assert_eq!(sample.regions[1].region, 2);
        assert_eq!(sample.regions[1].matched_color, Some([100, 110, 120]));
        assert_eq!(sample.regions[1].matched_indexes, vec![1]);
        assert_eq!(sample.regions[6].region, 7);
        assert_eq!(sample.regions[6].matched_color, Some([200, 210, 220]));
        assert_eq!(sample.regions[6].matched_indexes, vec![2]);
        assert!(sample.regions[0].matched_color.is_none());
        assert!(sample.regions[0].matched_indexes.is_empty());
        assert!(sample.regions[0].nearest_distance.is_finite());
    }

    fn sample_indexed(hits: &[(u8, [u8; 3], &[u8])]) -> Option<LimitedColorSample> {
        Some(LimitedColorSample {
            regions: (1..=9)
                .map(|index| {
                    hits.iter()
                        .find(|(candidate, _, _)| *candidate == index)
                        .map_or(
                            LimitedRegionMatch {
                                region: index,
                                matched_color: None,
                                matched_indexes: Vec::new(),
                                nearest_distance: 100.0,
                            },
                            |(_, color, indexes)| LimitedRegionMatch {
                                region: index,
                                matched_color: Some(*color),
                                matched_indexes: indexes.to_vec(),
                                nearest_distance: 0.0,
                            },
                        )
                })
                .collect(),
        })
    }

    #[tokio::test]
    async fn high_value_reports_both_color_indexes_when_regions_hit_different_targets() {
        let driver = FakeDriver::with_samples([
            sample_indexed(&[(2, [1, 2, 3], &[1]), (5, [4, 5, 6], &[2])]),
            sample_indexed(&[(2, [1, 2, 3], &[1]), (5, [4, 5, 6], &[2])]),
        ]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("预期完成");
        };
        assert_eq!(result.outcome, LimitedSupplyOutcome::HighValue);
        assert_eq!(result.matched_color_indexes, vec![1, 2]);
    }

    #[tokio::test]
    async fn high_value_reports_single_region_hitting_both_targets() {
        let driver = FakeDriver::with_samples([
            sample_indexed(&[(3, [1, 2, 3], &[1, 2])]),
            sample_indexed(&[(3, [1, 2, 3], &[1, 2])]),
        ]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("预期完成");
        };
        assert_eq!(result.outcome, LimitedSupplyOutcome::HighValue);
        assert_eq!(result.matched_color_indexes, vec![1, 2]);
    }
}
