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
pub fn restart_watchers(app: &AppHandle, settings: &AudioSettings, playback_tx: std::sync::mpsc::Sender<player::AudioCommand>) -> Result<(), String> {
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
        let allow_simultaneous = card.allow_simultaneous;
        let playback_tx_clone = playback_tx.clone();

        cancel_map.insert(card_id.clone(), cancel);

        tauri::async_runtime::spawn(async move {
            run_region_watcher(
                app_clone,
                card_id,
                region_clone,
                ref_path_clone,
                audio_path,
                volume,
                allow_simultaneous,
                playback_tx_clone,
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
    allow_simultaneous: bool,
    playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
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
                    let tx = playback_tx.clone();
                    let exclusive = !allow_simultaneous;
                    let _ = tx.send(player::AudioCommand::Play { path, volume: vol, exclusive });
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
pub(crate) fn load_reference_image(path: &str) -> Option<image::DynamicImage> {
    let path = Path::new(path);
    if !path.exists() {
        return None;
    }
    image::open(path).ok()
}

/// 截取屏幕区域（使用 xcap）
pub(crate) fn capture_region(region: &crate::morse::types::RegionRect) -> Option<image::DynamicImage> {
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
/// 使用灰度 ZNCC（归一化互相关）算法，对光照变化鲁棒。
/// 参考图若含 Alpha 通道，透明像素（alpha < 128）不参与比较。
pub(crate) fn compare_images(a: &image::DynamicImage, b: &image::DynamicImage) -> f32 {
    let width = a.width().min(b.width());
    let height = a.height().min(b.height());

    if width == 0 || height == 0 {
        return 0.0;
    }

    let a_resized = a.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
    let b_resized = b.resize_exact(width, height, image::imageops::FilterType::Lanczos3);

    let a_has_alpha = matches!(
        a.color(),
        image::ColorType::Rgba8 | image::ColorType::Rgba16 | image::ColorType::Rgba32F
    );

    let a_rgba = a_resized.to_rgba8();
    let b_rgba = b_resized.to_rgba8();

    let mut a_vals: Vec<f32> = Vec::new();
    let mut b_vals: Vec<f32> = Vec::new();

    for (a_pixel, b_pixel) in a_rgba.pixels().zip(b_rgba.pixels()) {
        if a_has_alpha && a_pixel[3] < 128 {
            continue;
        }
        let a_gray =
            0.299 * a_pixel[0] as f32 + 0.587 * a_pixel[1] as f32 + 0.114 * a_pixel[2] as f32;
        let b_gray =
            0.299 * b_pixel[0] as f32 + 0.587 * b_pixel[1] as f32 + 0.114 * b_pixel[2] as f32;
        a_vals.push(a_gray);
        b_vals.push(b_gray);
    }

    let n = a_vals.len();
    if n == 0 {
        return 0.0;
    }

    let mean_a: f32 = a_vals.iter().sum::<f32>() / n as f32;
    let mean_b: f32 = b_vals.iter().sum::<f32>() / n as f32;

    let mut numerator: f32 = 0.0;
    let mut denom_a: f32 = 0.0;
    let mut denom_b: f32 = 0.0;

    for i in 0..n {
        let da = a_vals[i] - mean_a;
        let db = b_vals[i] - mean_b;
        numerator += da * db;
        denom_a += da * da;
        denom_b += db * db;
    }

    let denominator = (denom_a * denom_b).sqrt();
    if denominator == 0.0 {
        return if numerator == 0.0 { 1.0 } else { 0.0 };
    }

    let zncc = numerator / denominator;
    // ZNCC 范围 [-1, 1]，映射到 [0, 1]
    ((zncc + 1.0) / 2.0).clamp(0.0, 1.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GrayImage, Luma, Rgb, RgbImage, Rgba, RgbaImage};

    #[test]
    fn same_image_returns_near_one() {
        let img = RgbaImage::from_pixel(4, 4, Rgba([128, 64, 32, 255]));
        let a = DynamicImage::ImageRgba8(img.clone());
        let b = DynamicImage::ImageRgba8(img);
        let score = compare_images(&a, &b);
        assert!(score > 0.99, "同一图像 ZNCC 应接近 1.0，实际 {}", score);
    }

    #[test]
    fn uniform_image_returns_one() {
        let img = GrayImage::from_pixel(4, 4, Luma([100]));
        let a = DynamicImage::ImageLuma8(img.clone());
        let b = DynamicImage::ImageLuma8(img);
        let score = compare_images(&a, &b);
        assert_eq!(score, 1.0, "均匀图像应返回 1.0");
    }

    #[test]
    fn different_images_returns_low() {
        let mut a = RgbaImage::new(4, 4);
        let mut b = RgbaImage::new(4, 4);
        for (x, y, pixel) in a.enumerate_pixels_mut() {
            *pixel = if y < 2 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 255])
            };
        }
        for (x, y, pixel) in b.enumerate_pixels_mut() {
            *pixel = if y < 2 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
        }
        let score = compare_images(
            &DynamicImage::ImageRgba8(a),
            &DynamicImage::ImageRgba8(b),
        );
        assert!(score < 0.1, "反差图像相似度应很低，实际 {}", score);
    }

    #[test]
    fn alpha_transparent_pixels_excluded() {
        let mut reference = RgbaImage::new(4, 4);
        for (x, y, pixel) in reference.enumerate_pixels_mut() {
            if y < 2 {
                *pixel = Rgba([0, 0, 0, 0]);
            } else {
                *pixel = Rgba([128, 128, 128, 255]);
            }
        }
        let screenshot = RgbImage::from_pixel(4, 4, Rgb([128, 128, 128]));
        let score = compare_images(
            &DynamicImage::ImageRgba8(reference),
            &DynamicImage::ImageRgb8(screenshot),
        );
        assert!(score > 0.99, "透明像素应被排除，剩余像素完全匹配，实际 {}", score);
    }

    #[test]
    fn zero_width_returns_zero() {
        let a = DynamicImage::ImageRgba8(RgbaImage::new(0, 4));
        let b = DynamicImage::ImageRgba8(RgbaImage::new(0, 4));
        let score = compare_images(&a, &b);
        assert_eq!(score, 0.0, "零宽度应返回 0.0");
    }
}
