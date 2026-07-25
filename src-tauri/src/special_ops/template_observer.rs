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
        let first = sampler.sample(target).await?;
        ensure_not_cancelled(&cancelled)?;
        wait_for_sample_interval(&cancelled).await?;

        ensure_not_cancelled(&cancelled)?;
        let second = sampler.sample(target).await?;
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

pub(crate) async fn wait_for_any_consistent_match<S: SimilaritySampler>(
    sampler: &S,
    targets: &[&RuntimeTemplate],
    cancelled: Arc<AtomicBool>,
) -> Result<(String, TemplateObservation), String> {
    wait_for_any_consistent_match_with_observer(sampler, targets, cancelled, |_, _| {}).await
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
            let current = sampler.sample(target).await?;
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

fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::SeqCst) {
        return Err("模板识别已取消".to_string());
    }
    Ok(())
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
}
