use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub(crate) const SAMPLE_INTERVAL: Duration = Duration::from_millis(400);
const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone)]
pub(crate) struct RuntimeTemplate {
    pub key: String,
    pub region: crate::morse::types::RegionRect,
    pub reference_image_path: PathBuf,
    pub threshold: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct RuntimeTarget {
    pub key: String,
    pub region: crate::morse::types::RegionRect,
    pub template: Option<RuntimeTemplate>,
    pub guard_any_of: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TemplateObservation {
    pub samples: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SingleConsistency {
    Matched { samples: [f32; 2] },
    NotMatched { samples: [f32; 2] },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BoundedAnyMatch {
    Matched {
        key: String,
        observation: TemplateObservation,
    },
    TimedOut {
        last_samples: Vec<(String, [f32; 2])>,
    },
}

#[allow(async_fn_in_trait)]
pub(crate) trait SimilaritySampler {
    async fn sample(&self, target: &RuntimeTemplate) -> Result<f32, String>;
}

#[allow(dead_code)]
pub(crate) struct RuntimeSimilaritySampler;

impl SimilaritySampler for RuntimeSimilaritySampler {
    async fn sample(&self, target: &RuntimeTemplate) -> Result<f32, String> {
        let region = target.region.clone();
        let reference_image_path = target.reference_image_path.clone();
        tokio::task::spawn_blocking(move || {
            let captured = crate::recognition::watcher::capture_region(&region)
                .ok_or_else(|| "截取识别区域失败".to_string())?;
            let reference_path = reference_image_path
                .to_str()
                .ok_or_else(|| "参考图路径包含无效字符".to_string())?;
            let reference = crate::recognition::watcher::load_reference_image(reference_path)
                .ok_or_else(|| "无法读取参考图".to_string())?;
            let (_, result) =
                crate::recognition::watcher::best_reference_match(&captured, [&reference])
                    .ok_or_else(|| "模板匹配失败".to_string())?;
            Ok(result.similarity)
        })
        .await
        .map_err(|error| format!("模板采样任务失败: {error}"))?
    }
}

pub(crate) async fn wait_for_consistent_match<S: SimilaritySampler>(
    sampler: &S,
    target: &RuntimeTemplate,
    cancelled: Arc<AtomicBool>,
) -> Result<TemplateObservation, String> {
    loop {
        ensure_not_cancelled(&cancelled)?;
        let first = sample_cancellable(sampler, target, &cancelled).await?;
        ensure_not_cancelled(&cancelled)?;
        wait_for_sample_interval(&cancelled).await?;

        ensure_not_cancelled(&cancelled)?;
        let second = sample_cancellable(sampler, target, &cancelled).await?;
        ensure_not_cancelled(&cancelled)?;

        if first >= target.threshold && second >= target.threshold {
            return Ok(TemplateObservation {
                samples: [first, second],
            });
        }

        wait_for_sample_interval(&cancelled).await?;
    }
}

pub(crate) async fn wait_for_target_match<S: SimilaritySampler>(
    sampler: &S,
    target: &RuntimeTarget,
    cancelled: Arc<AtomicBool>,
) -> Result<TemplateObservation, String> {
    let template = target
        .template
        .as_ref()
        .ok_or_else(|| "目标未配置参考图".to_string())?;
    wait_for_consistent_match(sampler, template, cancelled).await
}

pub(crate) async fn sample_single_consistent_once<S: SimilaritySampler>(
    sampler: &S,
    target: &RuntimeTemplate,
    cancelled: Arc<AtomicBool>,
) -> Result<SingleConsistency, String> {
    ensure_not_cancelled(&cancelled)?;
    let first = sample_cancellable(sampler, target, &cancelled).await?;
    wait_for_sample_interval(&cancelled).await?;
    let second = sample_cancellable(sampler, target, &cancelled).await?;
    ensure_not_cancelled(&cancelled)?;

    match (first >= target.threshold, second >= target.threshold) {
        (true, true) => Ok(SingleConsistency::Matched {
            samples: [first, second],
        }),
        (false, false) => Ok(SingleConsistency::NotMatched {
            samples: [first, second],
        }),
        _ => Err(format!(
            "模板 {} 两次采样不一致：{first:.4} / {second:.4}",
            target.key
        )),
    }
}

pub(crate) async fn wait_for_any_consistent_match<S: SimilaritySampler>(
    sampler: &S,
    targets: &[&RuntimeTemplate],
    cancelled: Arc<AtomicBool>,
) -> Result<(String, TemplateObservation), String> {
    wait_for_any_consistent_match_with_observer(sampler, targets, cancelled, |_, _| {}).await
}

pub(crate) async fn sample_any_consistent_once<S: SimilaritySampler>(
    sampler: &S,
    targets: &[&RuntimeTemplate],
    cancelled: Arc<AtomicBool>,
) -> Result<Option<(String, TemplateObservation)>, String> {
    if targets.is_empty() {
        return Err("没有可识别的模板目标".to_string());
    }

    let mut first_samples = Vec::with_capacity(targets.len());
    for target in targets {
        ensure_not_cancelled(&cancelled)?;
        first_samples.push(sample_cancellable(sampler, target, &cancelled).await?);
        ensure_not_cancelled(&cancelled)?;
    }

    wait_for_sample_interval(&cancelled).await?;

    let mut second_samples = Vec::with_capacity(targets.len());
    for target in targets {
        ensure_not_cancelled(&cancelled)?;
        second_samples.push(sample_cancellable(sampler, target, &cancelled).await?);
        ensure_not_cancelled(&cancelled)?;
    }

    Ok(targets.iter().enumerate().find_map(|(index, target)| {
        let samples = [first_samples[index], second_samples[index]];
        (samples[0] >= target.threshold && samples[1] >= target.threshold)
            .then(|| (target.key.clone(), TemplateObservation { samples }))
    }))
}

pub(crate) async fn wait_for_any_consistent_match_with_observer<
    S: SimilaritySampler,
    F: FnMut(&str, [f32; 2]) + Send,
>(
    sampler: &S,
    targets: &[&RuntimeTemplate],
    cancelled: Arc<AtomicBool>,
    mut observe: F,
) -> Result<(String, TemplateObservation), String> {
    if targets.is_empty() {
        return Err("没有可识别的模板目标".to_string());
    }
    let mut previous = vec![None; targets.len()];
    loop {
        for (index, target) in targets.iter().enumerate() {
            ensure_not_cancelled(&cancelled)?;
            let current = sample_cancellable(sampler, target, &cancelled).await?;
            ensure_not_cancelled(&cancelled)?;
            let samples = [previous[index].unwrap_or(current), current];
            observe(&target.key, samples);
            if current >= target.threshold {
                if let Some(first) = previous[index] {
                    return Ok((
                        target.key.clone(),
                        TemplateObservation {
                            samples: [first, current],
                        },
                    ));
                }
                previous[index] = Some(current);
            } else {
                previous[index] = None;
            }
        }
        wait_for_sample_interval(&cancelled).await?;
    }
}

pub(crate) async fn wait_for_consistent_match_until<S: SimilaritySampler>(
    sampler: &S,
    target: &RuntimeTemplate,
    cancelled: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<TemplateObservation, String> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut last_samples = [0.0, 0.0];
    loop {
        ensure_not_cancelled(&cancelled)?;
        let first = match tokio::time::timeout_at(
            deadline,
            sample_cancellable(sampler, target, &cancelled),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => return Err(timeout_message(&[target], &[last_samples])),
        };
        match tokio::time::timeout_at(deadline, wait_for_sample_interval(&cancelled)).await {
            Ok(result) => result?,
            Err(_) => return Err(timeout_message(&[target], &[last_samples])),
        }
        let second = match tokio::time::timeout_at(
            deadline,
            sample_cancellable(sampler, target, &cancelled),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => return Err(timeout_message(&[target], &[last_samples])),
        };
        last_samples = [first, second];
        if first >= target.threshold && second >= target.threshold {
            return Ok(TemplateObservation {
                samples: last_samples,
            });
        }
        match tokio::time::timeout_at(deadline, wait_for_sample_interval(&cancelled)).await {
            Ok(result) => result?,
            Err(_) => return Err(timeout_message(&[target], &[last_samples])),
        }
    }
}

pub(crate) async fn wait_for_target_match_until<S: SimilaritySampler>(
    sampler: &S,
    target: &RuntimeTarget,
    cancelled: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<TemplateObservation, String> {
    let template = target
        .template
        .as_ref()
        .ok_or_else(|| "目标未配置参考图".to_string())?;
    wait_for_consistent_match_until(sampler, template, cancelled, timeout).await
}

pub(crate) async fn wait_for_any_consistent_match_until<S: SimilaritySampler>(
    sampler: &S,
    targets: &[&RuntimeTemplate],
    cancelled: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<(String, TemplateObservation), String> {
    match try_wait_for_any_consistent_match_until(sampler, targets, cancelled, timeout).await? {
        BoundedAnyMatch::Matched { key, observation } => Ok((key, observation)),
        BoundedAnyMatch::TimedOut { last_samples } => {
            let samples = last_samples
                .iter()
                .map(|(_, samples)| *samples)
                .collect::<Vec<_>>();
            Err(timeout_message(targets, &samples))
        }
    }
}

pub(crate) async fn try_wait_for_any_consistent_match_until<S: SimilaritySampler>(
    sampler: &S,
    targets: &[&RuntimeTemplate],
    cancelled: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<BoundedAnyMatch, String> {
    if targets.is_empty() {
        return Err("没有可识别的模板目标".to_string());
    }
    let deadline = tokio::time::Instant::now() + timeout;
    let mut previous = vec![None; targets.len()];
    let mut last_samples = vec![[0.0, 0.0]; targets.len()];
    loop {
        for (index, target) in targets.iter().enumerate() {
            ensure_not_cancelled(&cancelled)?;
            let current = match tokio::time::timeout_at(
                deadline,
                sample_cancellable(sampler, target, &cancelled),
            )
            .await
            {
                Ok(result) => result?,
                Err(_) => {
                    return Ok(BoundedAnyMatch::TimedOut {
                        last_samples: targets
                            .iter()
                            .zip(last_samples)
                            .map(|(target, samples)| (target.key.clone(), samples))
                            .collect(),
                    });
                }
            };
            let samples = [previous[index].unwrap_or(current), current];
            last_samples[index] = samples;
            if current >= target.threshold {
                if let Some(first) = previous[index] {
                    return Ok(BoundedAnyMatch::Matched {
                        key: target.key.clone(),
                        observation: TemplateObservation {
                            samples: [first, current],
                        },
                    });
                }
                previous[index] = Some(current);
            } else {
                previous[index] = None;
            }
        }
        match tokio::time::timeout_at(deadline, wait_for_sample_interval(&cancelled)).await {
            Ok(result) => result?,
            Err(_) => {
                return Ok(BoundedAnyMatch::TimedOut {
                    last_samples: targets
                        .iter()
                        .zip(last_samples)
                        .map(|(target, samples)| (target.key.clone(), samples))
                        .collect(),
                });
            }
        }
    }
}

fn timeout_message(targets: &[&RuntimeTemplate], samples: &[[f32; 2]]) -> String {
    let details = targets
        .iter()
        .zip(samples)
        .map(|(target, samples)| {
            format!(
                "{} threshold={:.4} samples=[{:.4}, {:.4}]",
                target.key, target.threshold, samples[0], samples[1]
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("模板识别超时：{details}")
}

fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::SeqCst) {
        return Err("模板识别已取消".to_string());
    }
    Ok(())
}

async fn sample_cancellable<S: SimilaritySampler>(
    sampler: &S,
    target: &RuntimeTemplate,
    cancelled: &AtomicBool,
) -> Result<f32, String> {
    ensure_not_cancelled(cancelled)?;
    tokio::select! {
        biased;
        _ = wait_until_cancelled(cancelled) => Err("模板识别已取消".to_string()),
        result = sampler.sample(target) => result,
    }
}

async fn wait_until_cancelled(cancelled: &AtomicBool) {
    while !cancelled.load(Ordering::SeqCst) {
        tokio::time::sleep(CANCELLATION_POLL_INTERVAL).await;
    }
}

async fn wait_for_sample_interval(cancelled: &AtomicBool) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + SAMPLE_INTERVAL;
    loop {
        ensure_not_cancelled(cancelled)?;
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return ensure_not_cancelled(cancelled);
        }
        tokio::time::sleep(remaining.min(CANCELLATION_POLL_INTERVAL)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::Duration,
    };

    struct ScriptedSampler {
        samples: Mutex<VecDeque<f32>>,
        started_at: tokio::time::Instant,
    }

    struct KeyedSampler {
        samples: Mutex<std::collections::HashMap<String, VecDeque<f32>>>,
        sampled_keys: Mutex<Vec<String>>,
    }

    struct PendingSampler;

    struct ConstantSampler(f32);

    struct ErrorSampler;

    impl SimilaritySampler for PendingSampler {
        async fn sample(&self, _: &RuntimeTemplate) -> Result<f32, String> {
            std::future::pending().await
        }
    }

    impl SimilaritySampler for ConstantSampler {
        async fn sample(&self, _: &RuntimeTemplate) -> Result<f32, String> {
            Ok(self.0)
        }
    }

    impl SimilaritySampler for ErrorSampler {
        async fn sample(&self, _: &RuntimeTemplate) -> Result<f32, String> {
            Err("截取识别区域失败".to_string())
        }
    }

    impl SimilaritySampler for KeyedSampler {
        async fn sample(&self, target: &RuntimeTemplate) -> Result<f32, String> {
            self.sampled_keys.lock().unwrap().push(target.key.clone());
            self.samples
                .lock()
                .unwrap()
                .get_mut(&target.key)
                .and_then(VecDeque::pop_front)
                .ok_or_else(|| "测试采样序列已耗尽".to_string())
        }
    }

    impl ScriptedSampler {
        fn new(samples: impl IntoIterator<Item = f32>) -> Self {
            Self {
                samples: Mutex::new(samples.into_iter().collect()),
                started_at: tokio::time::Instant::now(),
            }
        }

        fn started_at(&self) -> tokio::time::Instant {
            self.started_at
        }
    }

    impl SimilaritySampler for ScriptedSampler {
        async fn sample(&self, _: &RuntimeTemplate) -> Result<f32, String> {
            self.samples
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| "测试采样序列已耗尽".to_string())
        }
    }

    fn target() -> RuntimeTemplate {
        RuntimeTemplate {
            key: "test-target".to_string(),
            region: crate::morse::types::RegionRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            reference_image_path: "reference.png".into(),
            threshold: 0.8,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn single_consistency_distinguishes_match_absence_and_mismatch() {
        let matched = sample_single_consistent_once(
            &ScriptedSampler::new([0.91, 0.92]),
            &target(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
        assert!(matches!(matched, SingleConsistency::Matched { .. }));

        let absent = sample_single_consistent_once(
            &ScriptedSampler::new([0.21, 0.20]),
            &target(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
        assert!(matches!(absent, SingleConsistency::NotMatched { .. }));

        let error = sample_single_consistent_once(
            &ScriptedSampler::new([0.91, 0.20]),
            &target(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();
        assert!(error.contains("两次采样不一致"));
    }

    #[tokio::test]
    async fn single_consistency_propagates_sampling_error() {
        let error = sample_single_consistent_once(
            &ErrorSampler,
            &target(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();

        assert_eq!(error, "截取识别区域失败");
    }

    #[tokio::test(start_paused = true)]
    async fn bounded_any_match_returns_timeout_with_last_samples() {
        let template = target();
        let result = try_wait_for_any_consistent_match_until(
            &ConstantSampler(0.2),
            &[&template],
            Arc::new(AtomicBool::new(false)),
            Duration::from_secs(1),
        )
        .await
        .unwrap();

        match result {
            BoundedAnyMatch::TimedOut { last_samples } => {
                assert_eq!(last_samples[0].0, "test-target");
                assert_eq!(last_samples[0].1, [0.2, 0.2]);
            }
            other => panic!("应返回结构化 timeout，实际为 {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn bounded_single_target_reports_last_samples() {
        let error = wait_for_consistent_match_until(
            &ConstantSampler(0.42),
            &target(),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Duration::from_millis(900),
        )
        .await
        .unwrap_err();

        assert!(error.contains("test-target"));
        assert!(error.contains("threshold=0.8000"));
        assert!(error.contains("samples=[0.4200, 0.4200]"));
    }

    #[tokio::test(start_paused = true)]
    async fn bounded_any_target_reports_each_last_pair() {
        let mut first = target();
        first.key = "produce".to_string();
        let mut second = target();
        second.key = "fill".to_string();
        let error = wait_for_any_consistent_match_until(
            &ConstantSampler(0.2),
            &[&first, &second],
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Duration::from_millis(900),
        )
        .await
        .unwrap_err();

        assert!(error.contains("produce"));
        assert!(error.contains("fill"));
        assert!(error.contains("samples=[0.2000, 0.2000]"));
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_wins_over_deadline() {
        let error = wait_for_consistent_match_until(
            &ConstantSampler(0.2),
            &target(),
            Arc::new(std::sync::atomic::AtomicBool::new(true)),
            Duration::from_secs(30),
        )
        .await
        .unwrap_err();

        assert_eq!(error, "模板识别已取消");
    }

    #[tokio::test(start_paused = true)]
    async fn hit_then_miss_restarts_two_sample_window() {
        let sampler = ScriptedSampler::new([0.9, 0.2, 0.91, 0.92]);
        let result = wait_for_consistent_match(
            &sampler,
            &target(),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await
        .unwrap();

        assert_eq!(result.samples, [0.91, 0.92]);
        assert!(
            tokio::time::Instant::now().duration_since(sampler.started_at())
                >= Duration::from_millis(1200)
        );
    }

    #[tokio::test]
    async fn target_without_template_returns_error_without_sampling() {
        let sampler = ScriptedSampler::new([]);
        let target = RuntimeTarget {
            key: "test-target".to_string(),
            region: target().region,
            template: None,
            guard_any_of: Vec::new(),
        };

        let error = wait_for_target_match(
            &sampler,
            &target,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await
        .unwrap_err();

        assert_eq!(error, "目标未配置参考图");
    }

    #[tokio::test(start_paused = true)]
    async fn wait_for_any_samples_every_target_without_starvation() {
        let first = RuntimeTemplate {
            key: "first".to_string(),
            ..target()
        };
        let second = RuntimeTemplate {
            key: "second".to_string(),
            ..target()
        };
        let sampler = KeyedSampler {
            samples: Mutex::new(std::collections::HashMap::from([
                ("first".to_string(), VecDeque::from([0.1, 0.1])),
                ("second".to_string(), VecDeque::from([0.9, 0.9])),
            ])),
            sampled_keys: Mutex::new(Vec::new()),
        };

        let matched = wait_for_any_consistent_match(
            &sampler,
            &[&first, &second],
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();

        assert_eq!(matched.0, "second");
        assert_eq!(matched.1.samples, [0.9, 0.9]);
        assert_eq!(
            *sampler.sampled_keys.lock().unwrap(),
            ["first", "second", "first", "second"]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn wait_for_any_reports_latest_samples_each_round() {
        let template = target();
        let sampler = ScriptedSampler::new([0.4, 0.9, 0.95]);
        let mut observed = Vec::new();

        wait_for_any_consistent_match_with_observer(
            &sampler,
            &[&template],
            Arc::new(AtomicBool::new(false)),
            |key, samples| observed.push((key.to_string(), samples)),
        )
        .await
        .unwrap();

        assert_eq!(
            observed,
            [
                ("test-target".to_string(), [0.4, 0.4]),
                ("test-target".to_string(), [0.9, 0.9]),
                ("test-target".to_string(), [0.9, 0.95]),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn sample_any_once_requires_same_target_to_match_both_rounds() {
        let first = RuntimeTemplate {
            key: "first".to_string(),
            ..target()
        };
        let second = RuntimeTemplate {
            key: "second".to_string(),
            ..target()
        };
        let sampler = KeyedSampler {
            samples: Mutex::new(std::collections::HashMap::from([
                ("first".to_string(), VecDeque::from([0.9, 0.2])),
                ("second".to_string(), VecDeque::from([0.9, 0.9])),
            ])),
            sampled_keys: Mutex::new(Vec::new()),
        };

        let matched = sample_any_consistent_once(
            &sampler,
            &[&first, &second],
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();

        assert_eq!(matched.unwrap().0, "second");
        assert_eq!(
            *sampler.sampled_keys.lock().unwrap(),
            ["first", "second", "first", "second"]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn sample_any_once_returns_none_when_no_target_matches_twice() {
        let template = target();
        let sampler = ScriptedSampler::new([0.9, 0.2]);

        let matched =
            sample_any_consistent_once(&sampler, &[&template], Arc::new(AtomicBool::new(false)))
                .await
                .unwrap();

        assert!(matched.is_none());
    }

    #[tokio::test]
    async fn pending_sample_in_single_target_wait_is_cancelled() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_cancelled = Arc::clone(&cancelled);
        let task = tokio::spawn(async move {
            wait_for_consistent_match(&PendingSampler, &target(), task_cancelled).await
        });
        tokio::task::yield_now().await;

        cancelled.store(true, Ordering::SeqCst);
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("取消后采样等待应及时退出")
            .unwrap();

        assert_eq!(result.unwrap_err(), "模板识别已取消");
    }

    #[tokio::test]
    async fn pending_sample_in_multi_target_wait_is_cancelled() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_cancelled = Arc::clone(&cancelled);
        let task = tokio::spawn(async move {
            let template = target();
            wait_for_any_consistent_match(&PendingSampler, &[&template], task_cancelled).await
        });
        tokio::task::yield_now().await;

        cancelled.store(true, Ordering::SeqCst);
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("取消后多目标采样等待应及时退出")
            .unwrap();

        assert_eq!(result.unwrap_err(), "模板识别已取消");
    }

    #[tokio::test]
    async fn pending_sample_in_one_shot_wait_is_cancelled() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_cancelled = Arc::clone(&cancelled);
        let task = tokio::spawn(async move {
            let template = target();
            sample_any_consistent_once(&PendingSampler, &[&template], task_cancelled).await
        });
        tokio::task::yield_now().await;

        cancelled.store(true, Ordering::SeqCst);
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("取消后一次性采样等待应及时退出")
            .unwrap();

        assert_eq!(result.unwrap_err(), "模板识别已取消");
    }
}
