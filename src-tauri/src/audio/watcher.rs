use std::collections::HashMap;
use std::io::Cursor;
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
                let result = compare_images(&captured, &reference_image);
                if result.similarity >= threshold {
                    // 触发音频播放
                    eprintln!("[音频 watcher] 卡片 {card_id}: 匹配成功 similarity={:.4} >= threshold={threshold} (位置: {},{})", result.similarity, result.best_x, result.best_y);
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

/// 读取参考图像为 PNG base64 数据 URL（供前端预览）
pub(crate) fn read_reference_image_as_data_url(path: &str) -> Option<String> {
    let path = Path::new(path);
    if !path.exists() {
        return None;
    }
    let img = image::open(path).ok()?;
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    let b64 = base64_encode(&buf.into_inner());
    Some(format!("data:image/png;base64,{b64}"))
}

/// 简易 base64 编码（不引入额外 crate）
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        out.push(CHARS[(n & 0x3F) as usize] as char);
        i += 3;
    }
    if i + 1 < data.len() {
        let n = ((data[i] as u32) << 10) | ((data[i + 1] as u32) << 2);
        out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        out.push(CHARS[(n & 0x3F) as usize] as char);
        out.push('=');
    } else if i < data.len() {
        let n = (data[i] as u32) << 4;
        out.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        out.push(CHARS[(n & 0x3F) as usize] as char);
        out.push_str("==");
    }
    out
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

/// 图像比较结果
#[derive(Debug, Clone)]
pub(crate) struct CompareResult {
    /// 相似度 0.0-1.0
    pub similarity: f32,
    /// 最佳匹配位置的左上角 X 坐标（原图像素坐标）
    pub best_x: u32,
    /// 最佳匹配位置的左上角 Y 坐标（原图像素坐标）
    pub best_y: u32,
}

/// 比较两张图像的相似度（返回 0.0-1.0，1.0 表示完全相同）
///
/// 使用滑动窗口模板匹配 + RGB 三通道 NCC 算法：
/// - 参考图尺寸 = 截图尺寸：直接做 RGB NCC 比较
/// - 参考图尺寸 < 截图尺寸：滑动窗口模板匹配，搜索最佳匹配位置
/// - 参考图尺寸 > 截图尺寸：返回 0.0（不可能匹配）
///
/// 参考图若含 Alpha 通道，透明像素（alpha < 128）不参与比较。
/// 多尺度加速：大图先在低分辨率粗搜索，再在最佳位置附近精搜索。
pub(crate) fn compare_images(screenshot: &image::DynamicImage, reference: &image::DynamicImage) -> CompareResult {
    let sw = screenshot.width();
    let sh = screenshot.height();
    let rw = reference.width();
    let rh = reference.height();

    if sw == 0 || sh == 0 || rw == 0 || rh == 0 {
        return CompareResult { similarity: 0.0, best_x: 0, best_y: 0 };
    }

    // 参考图比截图大，不可能匹配
    if rw > sw || rh > sh {
        return CompareResult { similarity: 0.0, best_x: 0, best_y: 0 };
    }

    // 同尺寸：直接比较
    if rw == sw && rh == sh {
        let s_rgba = screenshot.to_rgba8();
        let r_rgba = reference.to_rgba8();
        let r_has_alpha = has_alpha_channel(reference);
        let ncc = compute_ncc_rgb(&s_rgba, &r_rgba, r_has_alpha, 0, 0, rw, rh);
        let score = ncc_to_similarity(ncc);
        return CompareResult { similarity: score, best_x: 0, best_y: 0 };
    }

    // 参考图比截图小：模板匹配
    template_match(screenshot, reference)
}

/// 滑动窗口模板匹配（含多尺度加速）
fn template_match(screenshot: &image::DynamicImage, reference: &image::DynamicImage) -> CompareResult {
    let sw = screenshot.width();
    let sh = screenshot.height();
    let rw = reference.width();
    let rh = reference.height();
    let r_has_alpha = has_alpha_channel(reference);

    // 决定缩放因子
    let scale = choose_scale(sw, sh, rw, rh);

    if scale == 1 {
        // 直接全分辨率搜索
        let s_rgba = screenshot.to_rgba8();
        let r_rgba = reference.to_rgba8();
        let (best_ncc, best_x, best_y) = sliding_search(&s_rgba, &r_rgba, r_has_alpha, sw, sh, rw, rh, 0, 0, sw - rw, sh - rh);
        return CompareResult {
            similarity: ncc_to_similarity(best_ncc),
            best_x,
            best_y,
        };
    }

    // Phase 1: 粗搜索（低分辨率）
    let new_sw = (sw as f32 / scale as f32).round() as u32;
    let new_sh = (sh as f32 / scale as f32).round() as u32;
    let new_rw = (rw as f32 / scale as f32).round() as u32;
    let new_rh = (rh as f32 / scale as f32).round() as u32;

    // 确保缩放后尺寸有效
    if new_sw == 0 || new_sh == 0 || new_rw == 0 || new_rh == 0 || new_rw > new_sw || new_rh > new_sh {
        // 退化到全分辨率
        let s_rgba = screenshot.to_rgba8();
        let r_rgba = reference.to_rgba8();
        let (best_ncc, best_x, best_y) = sliding_search(&s_rgba, &r_rgba, r_has_alpha, sw, sh, rw, rh, 0, 0, sw - rw, sh - rh);
        return CompareResult {
            similarity: ncc_to_similarity(best_ncc),
            best_x,
            best_y,
        };
    }

    let s_small = screenshot.resize_exact(new_sw, new_sh, image::imageops::FilterType::Lanczos3).to_rgba8();
    let r_small = reference.resize_exact(new_rw, new_rh, image::imageops::FilterType::Lanczos3).to_rgba8();
    // 缩放后参考图不再有有意义的 alpha，按无 alpha 处理
    let (_, coarse_x, coarse_y) = sliding_search(&s_small, &r_small, false, new_sw, new_sh, new_rw, new_rh, 0, 0, new_sw - new_rw, new_sh - new_rh);

    // 将粗搜索位置映射回全分辨率
    let fine_cx = (coarse_x as f32 * scale as f32).round() as u32;
    let fine_cy = (coarse_y as f32 * scale as f32).round() as u32;

    // Phase 2: 精搜索（全分辨率，在粗搜索最佳位置 ± margin 范围内）
    let margin = scale * 2; // 在粗搜索位置 ± (scale*2) 像素范围搜索
    let x_start = fine_cx.saturating_sub(margin);
    let y_start = fine_cy.saturating_sub(margin);
    let x_end = (fine_cx + margin).min(sw - rw);
    let y_end = (fine_cy + margin).min(sh - rh);

    let s_rgba = screenshot.to_rgba8();
    let r_rgba = reference.to_rgba8();
    let (best_ncc, best_x, best_y) = sliding_search(&s_rgba, &r_rgba, r_has_alpha, sw, sh, rw, rh, x_start, y_start, x_end, y_end);

    CompareResult {
        similarity: ncc_to_similarity(best_ncc),
        best_x,
        best_y,
    }
}

/// 选择多尺度缩放因子
fn choose_scale(sw: u32, _sh: u32, rw: u32, rh: u32) -> u32 {
    // 参考图缩放后至少 8x8
    if sw > 256 && rw > 32 && rh > 32 {
        4
    } else if sw > 128 && rw > 16 && rh > 16 {
        2
    } else {
        1
    }
}

/// 滑动窗口搜索：在 [x_start..=x_end, y_start..=y_end] 范围内搜索最佳匹配
fn sliding_search(
    s_rgba: &image::RgbaImage,
    r_rgba: &image::RgbaImage,
    r_has_alpha: bool,
    _sw: u32,
    _sh: u32,
    rw: u32,
    rh: u32,
    x_start: u32,
    y_start: u32,
    x_end: u32,
    y_end: u32,
) -> (f32, u32, u32) {
    let mut best_ncc: f32 = f32::NEG_INFINITY;
    let mut best_x: u32 = 0;
    let mut best_y: u32 = 0;

    for y in y_start..=y_end {
        for x in x_start..=x_end {
            let ncc = compute_ncc_rgb(s_rgba, r_rgba, r_has_alpha, x, y, rw, rh);
            if ncc > best_ncc {
                best_ncc = ncc;
                best_x = x;
                best_y = y;
            }
        }
    }

    // 如果没有任何有效位置（边界情况），确保返回值合理
    if best_ncc == f32::NEG_INFINITY {
        best_ncc = 0.0;
    }

    (best_ncc, best_x, best_y)
}

/// 判断图像是否含 Alpha 通道
fn has_alpha_channel(img: &image::DynamicImage) -> bool {
    matches!(
        img.color(),
        image::ColorType::Rgba8 | image::ColorType::Rgba16 | image::ColorType::Rgba32F
    )
}

/// 计算截图在位置 (sx, sy) 处的 rw×rh 子区域与参考图的 RGB NCC
///
/// NCC = Σ((a_i - mean_a)(b_i - mean_b)) / √(Σ(a_i - mean_a)² × Σ(b_i - mean_b)²)
/// 分别计算 R、G、B 三通道的 NCC，取平均值。
/// 参考图含 Alpha 时，alpha < 128 的像素不参与计算。
fn compute_ncc_rgb(
    s_rgba: &image::RgbaImage,
    r_rgba: &image::RgbaImage,
    r_has_alpha: bool,
    sx: u32,
    sy: u32,
    rw: u32,
    rh: u32,
) -> f32 {
    let mut sum_ra: f32 = 0.0;
    let mut sum_ga: f32 = 0.0;
    let mut sum_ba: f32 = 0.0;
    let mut sum_rb: f32 = 0.0;
    let mut sum_gb: f32 = 0.0;
    let mut sum_bb: f32 = 0.0;
    let mut n: u32 = 0;

    // 第一遍：求均值
    for ry in 0..rh {
        for rx in 0..rw {
            let s_pixel = s_rgba.get_pixel(sx + rx, sy + ry);
            let r_pixel = r_rgba.get_pixel(rx, ry);

            if r_has_alpha && r_pixel[3] < 128 {
                continue;
            }

            sum_ra += r_pixel[0] as f32;
            sum_ga += r_pixel[1] as f32;
            sum_ba += r_pixel[2] as f32;
            sum_rb += s_pixel[0] as f32;
            sum_gb += s_pixel[1] as f32;
            sum_bb += s_pixel[2] as f32;
            n += 1;
        }
    }

    if n == 0 {
        return 0.0;
    }

    let n_f = n as f32;
    let mean_ra = sum_ra / n_f;
    let mean_ga = sum_ga / n_f;
    let mean_ba = sum_ba / n_f;
    let mean_rb = sum_rb / n_f;
    let mean_gb = sum_gb / n_f;
    let mean_bb = sum_bb / n_f;

    // 第二遍：计算 NCC 分子和各方差
    let mut num_r: f32 = 0.0;
    let mut num_g: f32 = 0.0;
    let mut num_b: f32 = 0.0;
    let mut denom_ra: f32 = 0.0;
    let mut denom_rb: f32 = 0.0;
    let mut denom_ga: f32 = 0.0;
    let mut denom_gb: f32 = 0.0;
    let mut denom_ba: f32 = 0.0;
    let mut denom_bb: f32 = 0.0;

    for ry in 0..rh {
        for rx in 0..rw {
            let s_pixel = s_rgba.get_pixel(sx + rx, sy + ry);
            let r_pixel = r_rgba.get_pixel(rx, ry);

            if r_has_alpha && r_pixel[3] < 128 {
                continue;
            }

            let da_r = r_pixel[0] as f32 - mean_ra;
            let da_g = r_pixel[1] as f32 - mean_ga;
            let da_b = r_pixel[2] as f32 - mean_ba;
            let db_r = s_pixel[0] as f32 - mean_rb;
            let db_g = s_pixel[1] as f32 - mean_gb;
            let db_b = s_pixel[2] as f32 - mean_bb;

            num_r += da_r * db_r;
            num_g += da_g * db_g;
            num_b += da_b * db_b;

            denom_ra += da_r * da_r;
            denom_rb += db_r * db_r;
            denom_ga += da_g * da_g;
            denom_gb += db_g * db_g;
            denom_ba += da_b * da_b;
            denom_bb += db_b * db_b;
        }
    }

    // 计算各通道 NCC
    // 只有当双方都有方差时才计算真实 NCC
    // 若双方都是零方差且均值相同 → 完全相关 (1.0)
    // 若双方都是零方差但均值不同 → 不相关 (0.0)
    // 若一方有方差另一方没有 → 不相关 (0.0)
    let ncc_r = if denom_ra > 0.0 && denom_rb > 0.0 {
        num_r / (denom_ra * denom_rb).sqrt()
    } else if denom_ra < 1e-10 && denom_rb < 1e-10 {
        if (mean_ra - mean_rb).abs() < 1.0 { 1.0 } else { 0.0 }
    } else {
        0.0
    };
    let ncc_g = if denom_ga > 0.0 && denom_gb > 0.0 {
        num_g / (denom_ga * denom_gb).sqrt()
    } else if denom_ga < 1e-10 && denom_gb < 1e-10 {
        if (mean_ga - mean_gb).abs() < 1.0 { 1.0 } else { 0.0 }
    } else {
        0.0
    };
    let ncc_b = if denom_ba > 0.0 && denom_bb > 0.0 {
        num_b / (denom_ba * denom_bb).sqrt()
    } else if denom_ba < 1e-10 && denom_bb < 1e-10 {
        if (mean_ba - mean_bb).abs() < 1.0 { 1.0 } else { 0.0 }
    } else {
        0.0
    };

    (ncc_r + ncc_g + ncc_b) / 3.0
}

/// NCC 值（范围 [-1, 1]）映射到相似度（范围 [0, 1]）
fn ncc_to_similarity(ncc: f32) -> f32 {
    ((ncc + 1.0) / 2.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GrayImage, Luma, Rgb, RgbImage, Rgba, RgbaImage};

    /// 辅助：取比较结果的 similarity
    fn score(a: &DynamicImage, b: &DynamicImage) -> f32 {
        compare_images(a, b).similarity
    }

    #[test]
    fn same_image_returns_near_one() {
        let img = RgbaImage::from_pixel(4, 4, Rgba([128, 64, 32, 255]));
        let a = DynamicImage::ImageRgba8(img.clone());
        let b = DynamicImage::ImageRgba8(img);
        let s = score(&a, &b);
        assert!(s > 0.99, "同一图像 NCC 应接近 1.0，实际 {}", s);
    }

    #[test]
    fn uniform_image_returns_one() {
        let img = GrayImage::from_pixel(4, 4, Luma([100]));
        let a = DynamicImage::ImageLuma8(img.clone());
        let b = DynamicImage::ImageLuma8(img);
        let s = score(&a, &b);
        assert_eq!(s, 1.0, "均匀图像应返回 1.0");
    }

    #[test]
    fn different_images_returns_low() {
        let mut a = RgbaImage::new(4, 4);
        let mut b = RgbaImage::new(4, 4);
        for (_x, y, pixel) in a.enumerate_pixels_mut() {
            *pixel = if y < 2 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 255])
            };
        }
        for (_x, y, pixel) in b.enumerate_pixels_mut() {
            *pixel = if y < 2 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
        }
        let s = score(
            &DynamicImage::ImageRgba8(a),
            &DynamicImage::ImageRgba8(b),
        );
        assert!(s < 0.1, "反差图像相似度应很低，实际 {}", s);
    }

    #[test]
    fn alpha_transparent_pixels_excluded() {
        let mut reference = RgbaImage::new(4, 4);
        for (_x, y, pixel) in reference.enumerate_pixels_mut() {
            if y < 2 {
                *pixel = Rgba([0, 0, 0, 0]);
            } else {
                *pixel = Rgba([128, 128, 128, 255]);
            }
        }
        let screenshot = RgbImage::from_pixel(4, 4, Rgb([128, 128, 128]));
        let s = score(
            &DynamicImage::ImageRgb8(screenshot),
            &DynamicImage::ImageRgba8(reference),
        );
        assert!(s > 0.99, "透明像素应被排除，剩余像素完全匹配，实际 {}", s);
    }

    #[test]
    fn zero_width_returns_zero() {
        let a = DynamicImage::ImageRgba8(RgbaImage::new(0, 4));
        let b = DynamicImage::ImageRgba8(RgbaImage::new(0, 4));
        let s = score(&a, &b);
        assert_eq!(s, 0.0, "零宽度应返回 0.0");
    }

    // ---- 新增测试 ----

    #[test]
    fn reference_larger_than_screenshot_returns_zero() {
        let screenshot = DynamicImage::ImageRgba8(RgbaImage::from_pixel(10, 10, Rgba([100, 100, 100, 255])));
        let reference = DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([100, 100, 100, 255])));
        let result = compare_images(&screenshot, &reference);
        assert_eq!(result.similarity, 0.0, "参考图比截图大应返回 0.0");
    }

    #[test]
    fn template_match_finds_offset_reference() {
        // 在 100x100 的截图中，将 10x10 的非均匀参考图放在 (30, 20) 位置
        let mut screenshot = RgbImage::from_pixel(100, 100, Rgb([50, 50, 50]));
        // 放置一个非均匀图案：棋盘格
        for y in 20..30 {
            for x in 30..40 {
                let color = if (x + y) % 2 == 0 {
                    Rgb([200, 100, 50])
                } else {
                    Rgb([50, 150, 200])
                };
                screenshot.put_pixel(x, y, color);
            }
        }

        // 参考图就是那个 10x10 的棋盘格
        let mut reference = RgbImage::new(10, 10);
        for y in 0..10u32 {
            for x in 0..10u32 {
                let color = if (x + 30 + y + 20) % 2 == 0 {
                    Rgb([200, 100, 50])
                } else {
                    Rgb([50, 150, 200])
                };
                reference.put_pixel(x, y, color);
            }
        }

        let result = compare_images(
            &DynamicImage::ImageRgb8(screenshot),
            &DynamicImage::ImageRgb8(reference),
        );
        assert!(result.similarity > 0.9, "模板匹配应找到偏移的参考图，实际 {}", result.similarity);
        // 允许 ±3 像素误差（多尺度搜索精度）
        assert!((result.best_x as i32 - 30).abs() <= 3, "最佳匹配 X 坐标应接近 30，实际 {}", result.best_x);
        assert!((result.best_y as i32 - 20).abs() <= 3, "最佳匹配 Y 坐标应接近 20，实际 {}", result.best_y);
    }

    #[test]
    fn rgb_ncc_rejects_grayscale_alias() {
        // 非均匀图像：两种不同颜色排列不会因灰度化混淆
        // 图 A: 左半红 (200,50,50) 右半蓝 (50,50,200)
        let mut a = RgbaImage::new(8, 8);
        for (x, _y, pixel) in a.enumerate_pixels_mut() {
            if x < 4 {
                *pixel = Rgba([200, 50, 50, 255]);
            } else {
                *pixel = Rgba([50, 50, 200, 255]);
            }
        }
        // 图 B: 左半绿 (50,200,50) 右半紫 (200,50,200)
        let mut b = RgbaImage::new(8, 8);
        for (x, _y, pixel) in b.enumerate_pixels_mut() {
            if x < 4 {
                *pixel = Rgba([50, 200, 50, 255]);
            } else {
                *pixel = Rgba([200, 50, 200, 255]);
            }
        }
        let s = score(
            &DynamicImage::ImageRgba8(a),
            &DynamicImage::ImageRgba8(b),
        );
        // RGB NCC 应能区分不同颜色排列
        assert!(s < 0.7, "RGB NCC 应能区分不同颜色排列，实际 {}", s);
    }

    #[test]
    fn same_size_rgb_ncc_high() {
        let img = RgbImage::from_pixel(10, 10, Rgb([128, 64, 200]));
        let a = DynamicImage::ImageRgb8(img.clone());
        let b = DynamicImage::ImageRgb8(img);
        let result = compare_images(&a, &b);
        assert!(result.similarity > 0.99, "同尺寸同图像 RGB NCC 应接近 1.0，实际 {}", result.similarity);
    }

    #[test]
    fn template_match_with_alpha_mask() {
        // 参考图：8x8，中心 4x4 不透明，四周透明
        let mut reference = RgbaImage::new(8, 8);
        for y in 0..8 {
            for x in 0..8 {
                if x >= 2 && x < 6 && y >= 2 && y < 6 {
                    reference.put_pixel(x, y, Rgba([200, 100, 50, 255]));
                } else {
                    reference.put_pixel(x, y, Rgba([0, 0, 0, 0])); // 透明
                }
            }
        }

        // 截图：40x40，在 (10, 12) 位置放置整个 8x8 区域（含背景），但只有中心 4x4 与参考图不透明部分匹配
        let mut screenshot = RgbImage::from_pixel(40, 40, Rgb([30, 30, 30]));
        for y in 12..20 {
            for x in 10..18 {
                // 不透明区域与参考图匹配，透明区域可以是任意颜色
                if x >= 12 && x < 16 && y >= 14 && y < 18 {
                    screenshot.put_pixel(x, y, Rgb([200, 100, 50]));
                } else {
                    screenshot.put_pixel(x, y, Rgb([100, 200, 80]));
                }
            }
        }

        let result = compare_images(
            &DynamicImage::ImageRgb8(screenshot),
            &DynamicImage::ImageRgba8(reference),
        );
        // Alpha mask 模板匹配应找到正确位置
        assert!(result.similarity > 0.8, "Alpha mask 模板匹配应能找到目标，实际 {}", result.similarity);
    }

    #[test]
    fn base64_encode_empty() {
        assert_eq!(base64_encode(&[]), "");
    }

    #[test]
    fn base64_encode_hello() {
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
    }

    #[test]
    fn base64_encode_padding() {
        // 1 byte → 2 chars + "=="
        assert_eq!(base64_encode(b"A"), "QQ==");
        // 2 bytes → 3 chars + "="
        assert_eq!(base64_encode(b"AB"), "QUI=");
    }
}
