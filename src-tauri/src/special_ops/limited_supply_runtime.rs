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
    let aggregate = crate::recognition::watcher::match_color_probes(
        screenshots,
        &probes,
        crate::recognition::ColorMatchMode::Any,
        crate::recognition::ColorMatchMethod::AnyPixel,
    );
    let matched = aggregate
        .matched_probes
        .into_iter()
        .map(|item| item.index)
        .collect::<std::collections::HashSet<_>>();
    let regions = screenshots
        .iter()
        .zip(&probes)
        .enumerate()
        .map(|(index, (screenshot, probe))| {
            let hit = crate::recognition::watcher::probe_hit(
                screenshot,
                probe,
                crate::recognition::ColorMatchMethod::AnyPixel,
                true,
            );
            LimitedRegionMatch {
                region: (index + 1) as u8,
                matched_color: matched.contains(&index).then_some(hit.target_color),
                nearest_distance: hit.distance,
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
            error: None,
        });
    }
    let region = first_hits.intersection(&second_hits).next().copied()?;
    let matched_color = second
        .regions
        .iter()
        .find(|candidate| candidate.region == region)
        .and_then(|candidate| candidate.matched_color);
    Some(LimitedSupplyCheckResult {
        outcome: LimitedSupplyOutcome::HighValue,
        matched_region: Some(region),
        matched_color,
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
    if let Err(error) = driver
        .wait_ready(config.ready_timeout, Arc::clone(&cancelled))
        .await
    {
        return stopped(error, "limited.ready");
    }

    let deadline = tokio::time::Instant::now() + config.ready_timeout;
    let mut first = None;
    while tokio::time::Instant::now() < deadline {
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return LimitedRunStop::EmergencyStopped;
        }
        match driver.sample_colors(Arc::clone(&cancelled)).await {
            Ok(Some(sample)) => {
                if let Some(previous) = first.take() {
                    if let Some(result) = compare_samples(&previous, &sample) {
                        return persist_or_stop(driver, result).await;
                    }
                } else {
                    first = Some(sample);
                }
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
    }

    impl FakeDriver {
        fn with_samples(samples: impl IntoIterator<Item = Option<LimitedColorSample>>) -> Self {
            Self {
                samples: Mutex::new(samples.into_iter().collect()),
                ready_error: None,
                persisted: Mutex::new(Vec::new()),
            }
        }

        fn ready_timeout() -> Self {
            Self {
                samples: Mutex::new(VecDeque::new()),
                ready_error: Some(LimitedRunError::ReadyTimeout),
                persisted: Mutex::new(Vec::new()),
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
            _duration: Duration,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), LimitedRunError> {
            Ok(())
        }

        async fn wait_ready(
            &self,
            _timeout: Duration,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<(), LimitedRunError> {
            self.ready_error.clone().map_or(Ok(()), Err)
        }

        async fn sample_colors(
            &self,
            _cancelled: Arc<AtomicBool>,
        ) -> Result<Option<LimitedColorSample>, LimitedRunError> {
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
                                nearest_distance: 100.0,
                            },
                            |(_, color)| LimitedRegionMatch {
                                region: index,
                                matched_color: Some(*color),
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
    async fn high_value_requires_same_region_in_two_valid_samples() {
        let driver =
            FakeDriver::with_samples([sample(&[(2, [1, 2, 3])]), sample(&[(2, [4, 5, 6])])]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("预期完成");
        };
        assert_eq!(result.outcome, LimitedSupplyOutcome::HighValue);
        assert_eq!(result.matched_region, Some(2));
        assert_eq!(result.matched_color, Some([4, 5, 6]));
    }

    #[tokio::test]
    async fn inconsistent_samples_are_discarded_before_resampling() {
        let driver = FakeDriver::with_samples([
            sample(&[(1, [1, 1, 1])]),
            sample(&[(2, [2, 2, 2])]),
            sample(&[(3, [3, 3, 3])]),
            sample(&[(3, [4, 4, 4])]),
        ]);

        let stop =
            run_limited_supply_branch(&driver, config(), Arc::new(AtomicBool::new(false))).await;

        let LimitedRunStop::Completed(result) = stop else {
            panic!("预期完成");
        };
        assert_eq!(result.matched_region, Some(3));
        assert_eq!(driver.persisted.lock().unwrap().len(), 1);
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
        assert_eq!(sample.regions[6].region, 7);
        assert_eq!(sample.regions[6].matched_color, Some([200, 210, 220]));
        assert!(sample.regions[0].matched_color.is_none());
        assert!(sample.regions[0].nearest_distance.is_finite());
    }
}
