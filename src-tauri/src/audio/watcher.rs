use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tokio::time::{interval, MissedTickBehavior};

use crate::audio::events::REGION_MATCHED;
use crate::audio::player;
use crate::audio::types::AudioSettings;
use tauri::{AppHandle, Emitter};

/// 全局 watcher 状态：卡片 ID -> 取消标记
static WATCHER_CANCEL_MAP: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

/// 启动/重启所有区域监听 watcher
pub fn restart_watchers(app: &AppHandle, settings: &AudioSettings) -> Result<(), String> {
    if !settings.audio_enabled {
        return stop_all_watchers(app);
    }

    let mut cancel_map = WATCHER_CANCEL_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "音频监听状态已损坏".to_string())?;

    // 先取消所有现有 watcher
    for (_, cancel) in cancel_map.drain() {
        cancel.store(true, Ordering::SeqCst);
    }

    // 为每张区域监听卡片启动 watcher
    for card in &settings.cards {
        if !card.enabled {
            continue;
        }
        if card.trigger_mode != super::types::AudioTriggerMode::RegionWatch {
            continue;
        }

        let Some(region) = &card.watch_region else { continue };
        let Some(ref_path) = &card.watch_reference_image_path else { continue };
        if ref_path.is_empty() || card.audio_file_path.is_empty() {
            continue;
        }

        let cancel = Arc::new(AtomicBool::new(false));
        let app_clone = app.clone();
        let card_id = card.id.clone();
        let audio_path = card.audio_file_path.clone();
        let volume = card.volume;
        let cooldown_ms = card.cooldown_ms;
        let threshold = card.watch_match_threshold;
        let poll_interval_ms = card.watch_poll_interval_ms;
        let region_clone = region.clone();
        let ref_path_clone = ref_path.clone();
        let cancel_clone = Arc::clone(&cancel);

        cancel_map.insert(card_id.clone(), cancel);

        tauri::async_runtime::spawn(async move {
            run_region_watcher(
                app_clone,
                card_id,
                region_clone,
                ref_path_clone,
                audio_path,
                volume,
                cooldown_ms,
                threshold,
                poll_interval_ms,
                cancel_clone,
            )
                .await;
        });
    }

    Ok(())
}

/// 停止所有区域监听 watcher
pub fn stop_all_watchers(_app: &AppHandle) -> Result<(), String> {
    let mut cancel_map = WATCHER_CANCEL_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "音频监听状态已损坏".to_string())?;
    for (_, cancel) in cancel_map.drain() {
        cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

async fn run_region_watcher(
    app: AppHandle,
    card_id: String,
    region: crate::morse::types::RegionRect,
    reference_image_path: String,
    audio_path: String,
    volume: f32,
    cooldown_ms: u32,
    threshold: f32,
    poll_interval_ms: u32,
    cancel: Arc<AtomicBool>,
) {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms.max(100) as u64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut last_triggered: Option<Instant> = None;

    // 加载参考图像
    let reference_image = match load_reference_image(&reference_image_path) {
        Some(img) => {
            eprintln!("[音频 watcher] 卡片 {card_id}: 参考图像加载成功 ({reference_image_path}), {}x{}", img.width(), img.height());
            img
        }
        None => {
            eprintln!("[音频 watcher] 卡片 {card_id}: 无法加载参考图像: {reference_image_path}");
            return;
        }
    };

    while !cancel.load(Ordering::SeqCst) {
        ticker.tick().await;

        if cancel.load(Ordering::SeqCst) {
            break;
        }

        // 检查冷却
        if let Some(last) = last_triggered {
            if last.elapsed() < Duration::from_millis(cooldown_ms as u64) {
                continue;
            }
        }

        // 截取屏幕区域
        match capture_region(&region) {
            Some(captured) => {
                let similarity = compare_images(&captured, &reference_image);
                if similarity >= threshold {
                    // 触发音频播放
                    eprintln!("[音频 watcher] 卡片 {card_id}: 匹配成功 similarity={similarity:.4} >= threshold={threshold}");
                    let _ = app.emit(REGION_MATCHED, &card_id);
                    let path = audio_path.clone();
                    let vol = volume;
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = player::play_audio_file(&path, vol) {
                            eprintln!("[音频 watcher] 播放失败: {e}");
                        }
                    });
                    last_triggered = Some(Instant::now());
                }
            }
            None => {
                // 截图失败，静默跳过
            }
        }
    }
}

/// 加载参考图像
fn load_reference_image(path: &str) -> Option<image::DynamicImage> {
    let path = Path::new(path);
    if !path.exists() {
        return None;
    }
    image::open(path).ok()
}

/// 截取屏幕区域（使用 xcap）
fn capture_region(region: &crate::morse::types::RegionRect) -> Option<image::DynamicImage> {
    #[cfg(target_os = "windows")]
    {
        use xcap::Monitor;
        use crate::morse::recognition::region_to_capture_bounds;

        let monitors = Monitor::all().ok()?;
        for monitor in monitors {
            let (Ok(monitor_left), Ok(monitor_top), Ok(monitor_width), Ok(monitor_height)) = (
                monitor.x(),
                monitor.y(),
                monitor.width(),
                monitor.height(),
            ) else {
                continue;
            };
            let scale_factor = monitor.scale_factor().unwrap_or(1.0);

            let Some((x, y, width, height)) = region_to_capture_bounds(
                region,
                monitor_left,
                monitor_top,
                monitor_width,
                monitor_height,
                scale_factor,
            ) else {
                continue;
            };

            let Ok(capture) = monitor.capture_region(x, y, width, height) else {
                continue;
            };

            let Some(rgba) = image::RgbaImage::from_raw(
                capture.width() as u32,
                capture.height() as u32,
                capture.into_raw(),
            ) else {
                continue;
            };

            return Some(image::DynamicImage::ImageRgba8(rgba));
        }

        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// 比较两张图像的相似度（返回 0.0-1.0，1.0 表示完全相同）
fn compare_images(a: &image::DynamicImage, b: &image::DynamicImage) -> f32 {
    // 统一尺寸为最小尺寸
    let width = a.width().min(b.width());
    let height = a.height().min(b.height());

    if width == 0 || height == 0 {
        return 0.0;
    }

    let a_resized = a.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
    let b_resized = b.resize_exact(width, height, image::imageops::FilterType::Lanczos3);

    let a_rgb = a_resized.to_rgb8();
    let b_rgb = b_resized.to_rgb8();

    let mut total_diff: u64 = 0;
    let mut total_pixels: u64 = 0;

    for (a_pixel, b_pixel) in a_rgb.pixels().zip(b_rgb.pixels()) {
        let dr = (a_pixel[0] as i32 - b_pixel[0] as i32).abs() as u64;
        let dg = (a_pixel[1] as i32 - b_pixel[1] as i32).abs() as u64;
        let db = (a_pixel[2] as i32 - b_pixel[2] as i32).abs() as u64;

        total_diff += dr + dg + db;
        total_pixels += 1;
    }

    if total_pixels == 0 {
        return 0.0;
    }

    // 最大可能差值 per pixel = 255 * 3 = 765
    let max_diff = total_pixels * 765;
    let normalized_diff = total_diff as f32 / max_diff as f32;

    // 相似度 = 1 - 归一化差值
    (1.0 - normalized_diff).clamp(0.0, 1.0)
}
