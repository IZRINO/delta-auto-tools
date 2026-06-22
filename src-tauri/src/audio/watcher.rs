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
use crate::audio::resolve_play_for_card;
use crate::audio::types::AudioSettings;
use crate::audio::types::{ColorMatchMode, ColorMatchMethod, ColorProbe};
use crate::global_state::GlobalState;
use tauri::{AppHandle, Emitter, Manager};

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

    // watcher 启动时快照音频模块开关（全局开关由循环内 watcher_should_run 实时检查）
    let audio_enabled = settings.audio_enabled;

    // 为每张区域监听 / 识色卡片启动 watcher
    for card in &settings.cards {
        if !card.enabled {
            continue;
        }
        // 按触发模式校验必要字段，缺字段则跳过
        match card.trigger_mode {
            super::types::AudioTriggerMode::RegionWatch => {
                let Some(ref_path) = &card.watch_reference_image_path else { continue };
                if ref_path.is_empty() || card.audio_files.is_empty() {
                    continue;
                }
                if card.watch_region.is_none() {
                    continue;
                }
            }
            super::types::AudioTriggerMode::ColorWatch => {
                if card.color_probes.is_empty() || card.audio_files.is_empty() {
                    continue;
                }
                // 含未框选（region=None）探针的卡片视为未就绪草稿，不启动 watcher
                if card.color_probes.iter().any(|p| p.region.is_none()) {
                    continue;
                }
            }
            super::types::AudioTriggerMode::Hotkey => continue,
        }

        let cancel = Arc::new(AtomicBool::new(false));
        let app_clone = app.clone();
        let card_id = card.id.clone();
        let cooldown_ms = card.cooldown_ms;
        let poll_interval_ms = card.watch_poll_interval_ms;
        let playback_tx_clone = playback_tx.clone();
        let cancel_clone = Arc::clone(&cancel);

        cancel_map.insert(card_id.clone(), cancel);

        match card.trigger_mode {
            super::types::AudioTriggerMode::RegionWatch => {
                let Some(region) = &card.watch_region else { continue };
                let Some(ref_path) = &card.watch_reference_image_path else { continue };
                let threshold = card.watch_match_threshold;
                let region_clone = region.clone();
                let ref_path_clone = ref_path.clone();
                tauri::async_runtime::spawn(async move {
                    run_region_watcher(
                        app_clone,
                        card_id,
                        region_clone,
                        ref_path_clone,
                        playback_tx_clone,
                        cooldown_ms,
                        threshold,
                        poll_interval_ms,
                        audio_enabled,
                        cancel_clone,
                    )
                        .await;
                });
            }
            super::types::AudioTriggerMode::ColorWatch => {
                let probes = card.color_probes.clone();
                let match_mode = card.color_match_mode.clone();
                let match_method = card.color_match_method.clone();
                tauri::async_runtime::spawn(async move {
                    run_color_watcher(
                        app_clone,
                        card_id,
                        probes,
                        match_mode,
                        match_method,
                        playback_tx_clone,
                        cooldown_ms,
                        poll_interval_ms,
                        audio_enabled,
                        cancel_clone,
                    )
                        .await;
                });
            }
            super::types::AudioTriggerMode::Hotkey => {}
        }
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

/// 读取全局总开关当前状态。`GlobalState` 缺失时视为已启用，不阻断 watcher。
fn global_enabled(app: &AppHandle) -> bool {
    app.try_state::<GlobalState>()
        .map(|state| state.enabled())
        .unwrap_or(true)
}

/// watcher 每轮 tick 的执行门：全局总开关与音频模块开关均开启时才执行。
///
/// - `global_enabled`：实时读取 `GlobalState`。全局总开关切换不会触发 `restart_watchers`，
///   故必须在 watcher 循环内实时检查（参考 `hotkeys.rs` 对热键的同类门控）。
/// - `audio_enabled`：watcher 启动时（`restart_watchers`）快照传入。音频模块自身开关
///   变更会触发 `restart_watchers` 重建 watcher，故启动快照即代表当前模块开关。
pub(crate) fn watcher_should_run(global_enabled: bool, audio_enabled: bool) -> bool {
    global_enabled && audio_enabled
}

async fn run_region_watcher(
    app: AppHandle,
    card_id: String,
    region: crate::morse::types::RegionRect,
    reference_image_path: String,
    playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
    cooldown_ms: u32,
    threshold: f32,
    poll_interval_ms: u32,
    audio_enabled: bool,
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

        // 全局总开关 / 音频模块开关关闭时，跳过本轮截图与匹配（不触发播放）
        if !watcher_should_run(global_enabled(&app), audio_enabled) {
            continue;
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
                    let resolved = resolve_play_for_card(&app, &card_id);
                    if let Some(resolved) = resolved {
                        let tx = playback_tx.clone();
                        let exclusive = !resolved.allow_simultaneous;
                        let _ = tx.send(player::AudioCommand::Play { path: resolved.path, volume: resolved.volume, exclusive });
                        last_triggered = Some(Instant::now());
                    }
                }
            }
            None => {
                // 截图失败，静默跳过
            }
        }
    }
}

async fn run_color_watcher(
    app: AppHandle,
    card_id: String,
    probes: Vec<crate::audio::types::ColorProbe>,
    match_mode: crate::audio::types::ColorMatchMode,
    match_method: crate::audio::types::ColorMatchMethod,
    playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
    cooldown_ms: u32,
    poll_interval_ms: u32,
    audio_enabled: bool,
    cancel: Arc<AtomicBool>,
) {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms.max(100) as u64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut last_triggered: Option<Instant> = None;

    while !cancel.load(Ordering::SeqCst) {
        ticker.tick().await;

        if cancel.load(Ordering::SeqCst) {
            break;
        }

        // 全局总开关 / 音频模块开关关闭时，跳过本轮截图与匹配（不触发播放）
        if !watcher_should_run(global_enabled(&app), audio_enabled) {
            continue;
        }

        // 检查冷却
        if let Some(last) = last_triggered {
            if last.elapsed() < Duration::from_millis(cooldown_ms as u64) {
                continue;
            }
        }

        // 逐个截取 probe 区域；region 缺失（None）的探针视为未就绪，跳过本轮
        let mut screenshots: Vec<image::DynamicImage> = Vec::with_capacity(probes.len());
        let mut all_captured = true;
        for probe in &probes {
            let Some(region) = probe.region.as_ref() else {
                all_captured = false;
                break;
            };
            match capture_region(region) {
                Some(img) => screenshots.push(img),
                None => {
                    all_captured = false;
                    break;
                }
            }
        }

        if !all_captured {
            continue;
        }

        let result = match_color_probes(&screenshots, &probes, match_mode.clone(), match_method.clone());
        if result.matched {
            eprintln!("[音频 color watcher] 卡片 {card_id}: 识色命中 {}/{} probes", result.hit_count, probes.len());
            let _ = app.emit(REGION_MATCHED, &card_id);
            if let Some(resolved) = resolve_play_for_card(&app, &card_id) {
                let tx = playback_tx.clone();
                let exclusive = !resolved.allow_simultaneous;
                let _ = tx.send(player::AudioCommand::Play { path: resolved.path, volume: resolved.volume, exclusive });
                last_triggered = Some(Instant::now());
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

/// 颜色匹配结果
#[derive(Debug, Clone)]
pub(crate) struct ColorMatchResult {
    /// 是否触发（按 mode 聚合后）
    pub matched: bool,
    /// 命中的 probe 数量
    pub hit_count: usize,
}

/// 计算两个 RGB 颜色的欧氏距离
pub(crate) fn color_distance(a: [u8; 3], b: [u8; 3]) -> f32 {
    let dr = (a[0] as f32 - b[0] as f32).powi(2);
    let dg = (a[1] as f32 - b[1] as f32).powi(2);
    let db = (a[2] as f32 - b[2] as f32).powi(2);
    (dr + dg + db).sqrt()
}

/// 取图像区域平均 RGB（alpha < 128 的透明像素不计入）
pub(crate) fn average_region_rgb(img: &image::DynamicImage) -> [u8; 3] {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return [0, 0, 0];
    }
    let mut sum_r: u64 = 0;
    let mut sum_g: u64 = 0;
    let mut sum_b: u64 = 0;
    let mut n: u64 = 0;
    for y in 0..h {
        for x in 0..w {
            let p = rgba.get_pixel(x, y);
            if p[3] < 128 {
                continue;
            }
            sum_r += p[0] as u64;
            sum_g += p[1] as u64;
            sum_b += p[2] as u64;
            n += 1;
        }
    }
    if n == 0 {
        return [0, 0, 0];
    }
    [
        (sum_r / n) as u8,
        (sum_g / n) as u8,
        (sum_b / n) as u8,
    ]
}

/// 像素扫描结果
#[derive(Debug, Clone)]
pub(crate) struct PixelScanResult {
    /// 命中像素数
    pub matching_count: usize,
    /// 最接近目标的像素色（无命中时为全图最近像素）
    pub nearest_color: [u8; 3],
    /// nearest_color 与目标的欧氏距离
    pub nearest_distance: f32,
}

/// 扫描区域，返回命中像素数与最接近目标色的像素。
///
/// - `count_only=true`：命中首个像素即早退，`matching_count` 至多为 1（watcher 用，性能优先）。
/// - `count_only=false`：全扫，`matching_count` 为真实命中数（test 用，调试优先）。
///
/// `nearest_color`/`nearest_distance` 始终为全图最接近目标的像素（命中时即目标色本身，距离 0）。
/// alpha < 128 的透明像素跳过。
pub(crate) fn scan_region_for_color(
    img: &image::DynamicImage,
    target: [u8; 3],
    tolerance: f32,
    count_only: bool,
) -> PixelScanResult {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut matching_count: usize = 0;
    let mut nearest_color: [u8; 3] = [0, 0, 0];
    let mut nearest_distance: f32 = f32::INFINITY;

    for y in 0..h {
        for x in 0..w {
            let p = rgba.get_pixel(x, y);
            if p[3] < 128 {
                continue;
            }
            let c = [p[0], p[1], p[2]];
            let dist = color_distance(c, target);
            if dist < nearest_distance {
                nearest_distance = dist;
                nearest_color = c;
            }
            if dist <= tolerance {
                matching_count += 1;
                if count_only {
                    // 早退：nearest 已更新为目标色（距离 0）
                    return PixelScanResult { matching_count, nearest_color, nearest_distance };
                }
            }
        }
    }

    if nearest_distance == f32::INFINITY {
        // 全透明或空图
        nearest_distance = 0.0;
    }

    PixelScanResult { matching_count, nearest_color, nearest_distance }
}

/// 单探针判定结果
#[derive(Debug, Clone)]
pub(crate) struct ProbeHit {
    /// 是否命中（按 probe.probe_match_mode 聚合后）
    pub matched: bool,
    /// average:区域平均色；anyPixel:最近像素（取距离最小的目标对应的采样色）
    pub sampled_color: [u8; 3],
    /// 采样色与最近目标的距离
    pub distance: f32,
    /// 最近目标颜色（聚合摘要）
    pub target_color: [u8; 3],
    /// 最近目标容差（聚合摘要）
    pub tolerance: u8,
    /// anyPixel 命中像素数（聚合摘要，取最近目标的）；average 恒 0
    pub matching_pixel_count: usize,
}

/// 单个目标颜色的命中详情（供 test 命令与多目标聚合使用）
#[derive(Debug, Clone)]
pub(crate) struct TargetHit {
    pub matched: bool,
    pub target_color: [u8; 3],
    pub tolerance: u8,
    pub sampled_color: [u8; 3],
    pub distance: f32,
    pub matching_pixel_count: usize,
}

/// 单探针内多目标判定：对 `probe.targets` 每个目标分别按 method 判定，返回每个目标的命中详情。
///
/// - `count_only=true`：anyPixel 命中即早退（watcher 用，性能优先）。
/// - `count_only=false`：全扫拿真实命中数与最近像素（test 用，调试优先）。
///
/// 返回值顺序与 `probe.targets` 一致。
pub(crate) fn probe_hit_targets(
    screenshot: &image::DynamicImage,
    probe: &crate::audio::types::ColorProbe,
    method: crate::audio::types::ColorMatchMethod,
    count_only: bool,
) -> Vec<TargetHit> {
    probe
        .targets
        .iter()
        .map(|target| {
            let hit = probe_hit_single_target(screenshot, target, method.clone(), count_only);
            TargetHit {
                matched: hit.matched,
                target_color: target.color,
                tolerance: target.tolerance,
                sampled_color: hit.sampled_color,
                distance: hit.distance,
                matching_pixel_count: hit.matching_pixel_count,
            }
        })
        .collect()
}

/// 单探针对单个目标的判定结果（内部用）
struct SingleTargetHit {
    matched: bool,
    sampled_color: [u8; 3],
    distance: f32,
    matching_pixel_count: usize,
}

/// 单探针对单个 `ColorTarget` 的判定：按 method 计算 sampled 与目标色的距离。
fn probe_hit_single_target(
    screenshot: &image::DynamicImage,
    target: &crate::audio::types::ColorTarget,
    method: crate::audio::types::ColorMatchMethod,
    count_only: bool,
) -> SingleTargetHit {
    match method {
        crate::audio::types::ColorMatchMethod::Average => {
            let avg = average_region_rgb(screenshot);
            let dist = color_distance(avg, target.color);
            SingleTargetHit {
                matched: dist <= target.tolerance as f32,
                sampled_color: avg,
                distance: dist,
                matching_pixel_count: 0,
            }
        }
        crate::audio::types::ColorMatchMethod::AnyPixel => {
            let scan = scan_region_for_color(screenshot, target.color, target.tolerance as f32, count_only);
            SingleTargetHit {
                matched: scan.matching_count > 0,
                sampled_color: scan.nearest_color,
                distance: scan.nearest_distance,
                matching_pixel_count: scan.matching_count,
            }
        }
    }
}

/// 探针级聚合：对 `probe.targets` 的命中详情按 `probe.probe_match_mode` 聚合，返回探针级摘要。
///
/// - Any：任一目标命中即视为探针命中
/// - All：所有目标都命中才视为探针命中
///
/// 聚合摘要字段（sampled_color/distance/target_color/tolerance/matching_pixel_count）
/// 取距离最小（最接近命中）的目标作为代表。
fn aggregate_probe_hits(
    hits: &[TargetHit],
    probe_match_mode: crate::audio::types::ColorMatchMode,
) -> ProbeHit {
    if hits.is_empty() {
        return ProbeHit {
            matched: false,
            sampled_color: [0, 0, 0],
            distance: f32::INFINITY,
            target_color: [0, 0, 0],
            tolerance: 0,
            matching_pixel_count: 0,
        };
    }
    let hit_count = hits.iter().filter(|h| h.matched).count();
    let matched = match probe_match_mode {
        ColorMatchMode::All => hit_count == hits.len(),
        ColorMatchMode::Any => hit_count > 0,
    };
    // 取距离最小的目标作为摘要代表
    let nearest = hits
        .iter()
        .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();
    ProbeHit {
        matched,
        sampled_color: nearest.sampled_color,
        distance: nearest.distance,
        target_color: nearest.target_color,
        tolerance: nearest.tolerance,
        matching_pixel_count: nearest.matching_pixel_count,
    }
}

/// pub(crate) wrapper：供 mod.rs 的 audio_test_color_match 命令复用
pub(crate) fn aggregate_probe_hits_pub(
    hits: &[TargetHit],
    probe_match_mode: crate::audio::types::ColorMatchMode,
) -> ProbeHit {
    aggregate_probe_hits(hits, probe_match_mode)
}

/// 单探针判定：对 `probe.targets` 按 `probe.probe_match_mode` 聚合后返回探针级摘要。
///
/// - `count_only=true`：anyPixel 命中即早退（watcher 用，性能优先）。
/// - `count_only=false`：全扫拿真实命中数与最近像素（test 用，调试优先）。
pub(crate) fn probe_hit(
    screenshot: &image::DynamicImage,
    probe: &crate::audio::types::ColorProbe,
    method: crate::audio::types::ColorMatchMethod,
    count_only: bool,
) -> ProbeHit {
    let hits = probe_hit_targets(screenshot, probe, method, count_only);
    aggregate_probe_hits(&hits, probe.probe_match_mode.clone())
}

/// 对一组已截取的区域图像与对应探针做颜色匹配，按 mode 聚合
pub(crate) fn match_color_probes(
    screenshots: &[image::DynamicImage],
    probes: &[ColorProbe],
    mode: ColorMatchMode,
    method: ColorMatchMethod,
) -> ColorMatchResult {
    if probes.is_empty() || screenshots.len() < probes.len() {
        return ColorMatchResult { matched: false, hit_count: 0 };
    }
    let mut hit_count = 0usize;
    for (i, probe) in probes.iter().enumerate() {
        if probe_hit(&screenshots[i], probe, method.clone(), true).matched {
            hit_count += 1;
        }
    }
    let matched = match mode {
        ColorMatchMode::All => hit_count == probes.len(),
        ColorMatchMode::Any => hit_count > 0,
    };
    ColorMatchResult { matched, hit_count }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GrayImage, Luma, Rgb, RgbImage, Rgba, RgbaImage};
    use crate::audio::types::{ColorMatchMethod, ColorMatchMode, ColorProbe, ColorTarget};
    use crate::morse::types::RegionRect;

    /// 辅助：取比较结果的 similarity
    fn score(a: &DynamicImage, b: &DynamicImage) -> f32 {
        compare_images(a, b).similarity
    }

    // ---- 全局总开关门控（Issue #60：全局关闭时音频 watcher 不应执行）----

    #[test]
    fn watcher_should_run_enabled_when_both_on() {
        assert!(watcher_should_run(true, true));
    }

    #[test]
    fn watcher_should_run_disabled_when_global_off() {
        assert!(!watcher_should_run(false, true));
    }

    #[test]
    fn watcher_should_run_disabled_when_audio_off() {
        assert!(!watcher_should_run(true, false));
    }

    #[test]
    fn watcher_should_run_disabled_when_both_off() {
        assert!(!watcher_should_run(false, false));
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

    #[test]
    fn color_distance_zero_for_same_color() {
        assert_eq!(color_distance([100, 100, 100], [100, 100, 100]), 0.0);
    }

    #[test]
    fn color_distance_orthogonal_channels() {
        // R 差 30 → 距离 30
        assert!((color_distance([130, 100, 100], [100, 100, 100]) - 30.0).abs() < 0.01);
        // 三轴各差 10 → 距离 sqrt(300) ≈ 17.32
        let d = color_distance([110, 110, 110], [100, 100, 100]);
        assert!((d - 17.32).abs() < 0.1, "实际 {}", d);
    }

    #[test]
    fn average_region_rgb_uniform() {
        let img = RgbaImage::from_pixel(3, 3, Rgba([10, 20, 30, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        assert_eq!(average_region_rgb(&dyn_img), [10, 20, 30]);
    }

    #[test]
    fn average_region_rgb_mixed() {
        // 2x2：四个角颜色平均
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([100, 0, 0, 255]));
        img.put_pixel(0, 1, Rgba([0, 100, 0, 255]));
        img.put_pixel(1, 1, Rgba([100, 100, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        assert_eq!(average_region_rgb(&dyn_img), [50, 50, 0]);
    }

    #[test]
    fn average_region_rgb_ignores_alpha() {
        // alpha < 128 的像素直接跳过，不参与平均
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([200, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 0])); // 完全透明，被跳过
        let dyn_img = DynamicImage::ImageRgba8(img);
        // 只有不透明像素 [200,0,0] 计入
        assert_eq!(average_region_rgb(&dyn_img), [200, 0, 0]);
    }

    #[test]
    fn match_color_probes_all_mode_all_hit() {
        // 两个 probe，截图颜色都匹配
        let screenshots = vec![
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]))),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]))),
        ];
        let probes = vec![
            ColorProbe { region: Some(RegionRect { x: 0, y: 0, width: 2, height: 2 }), targets: vec![ColorTarget { color: [200, 100, 50], tolerance: 10 }], probe_match_mode: ColorMatchMode::Any, legacy_target_color: None, legacy_tolerance: None },
            ColorProbe { region: Some(RegionRect { x: 0, y: 0, width: 2, height: 2 }), targets: vec![ColorTarget { color: [10, 20, 30], tolerance: 10 }], probe_match_mode: ColorMatchMode::Any, legacy_target_color: None, legacy_tolerance: None },
        ];
        let result = match_color_probes(&screenshots, &probes, ColorMatchMode::All, ColorMatchMethod::Average);
        assert!(result.matched, "All 模式全命中应触发");
        assert_eq!(result.hit_count, 2);
    }

    #[test]
    fn match_color_probes_all_mode_partial_miss() {
        let screenshots = vec![
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]))),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]))), // 不匹配
        ];
        let probes = vec![
            ColorProbe { region: Some(RegionRect { x: 0, y: 0, width: 2, height: 2 }), targets: vec![ColorTarget { color: [200, 100, 50], tolerance: 10 }], probe_match_mode: ColorMatchMode::Any, legacy_target_color: None, legacy_tolerance: None },
            ColorProbe { region: Some(RegionRect { x: 0, y: 0, width: 2, height: 2 }), targets: vec![ColorTarget { color: [10, 20, 30], tolerance: 10 }], probe_match_mode: ColorMatchMode::Any, legacy_target_color: None, legacy_tolerance: None },
        ];
        let result = match_color_probes(&screenshots, &probes, ColorMatchMode::All, ColorMatchMethod::Average);
        assert!(!result.matched, "All 模式部分未命中不应触发");
        assert_eq!(result.hit_count, 1);
    }

    #[test]
    fn match_color_probes_any_mode_one_hit() {
        let screenshots = vec![
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]))),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]))),
        ];
        let probes = vec![
            ColorProbe { region: Some(RegionRect { x: 0, y: 0, width: 2, height: 2 }), targets: vec![ColorTarget { color: [200, 100, 50], tolerance: 10 }], probe_match_mode: ColorMatchMode::Any, legacy_target_color: None, legacy_tolerance: None },
            ColorProbe { region: Some(RegionRect { x: 0, y: 0, width: 2, height: 2 }), targets: vec![ColorTarget { color: [10, 20, 30], tolerance: 10 }], probe_match_mode: ColorMatchMode::Any, legacy_target_color: None, legacy_tolerance: None },
        ];
        let result = match_color_probes(&screenshots, &probes, ColorMatchMode::Any, ColorMatchMethod::Average);
        assert!(result.matched, "Any 模式任一命中即触发");
        assert_eq!(result.hit_count, 1);
    }

    #[test]
    fn match_color_probes_empty_returns_false() {
        let result = match_color_probes(&[], &[], ColorMatchMode::All, ColorMatchMethod::Average);
        assert!(!result.matched, "无探针不应触发");
        assert_eq!(result.hit_count, 0);
    }

    #[test]
    fn match_color_probes_any_pixel_method_hits() {
        // 单探针：全黑图中央 1 红像素，average 未中、anyPixel 命中
        let mut img = RgbaImage::new(3, 3);
        for y in 0..3 {
            for x in 0..3 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        img.put_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let screenshots = vec![DynamicImage::ImageRgba8(img)];
        let probes = vec![ColorProbe {
            region: Some(RegionRect { x: 0, y: 0, width: 3, height: 3 }),
            targets: vec![ColorTarget { color: [255, 0, 0], tolerance: 10 }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        }];
        let avg = match_color_probes(&screenshots, &probes, ColorMatchMode::All, ColorMatchMethod::Average);
        assert!(!avg.matched, "average 模式平均色为黑，距红远，未中");
        let any = match_color_probes(&screenshots, &probes, ColorMatchMode::All, ColorMatchMethod::AnyPixel);
        assert!(any.matched, "anyPixel 模式存在红像素应命中");
        assert_eq!(any.hit_count, 1);
    }

    #[test]
    fn match_color_probes_any_pixel_combined_with_mode_all() {
        // 两探针：第一命中、第二未中 → mode=All 不触发，mode=Any 触发
        let mut img1 = RgbaImage::new(2, 2);
        for y in 0..2 {
            for x in 0..2 {
                img1.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        let img2 = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        let screenshots = vec![
            DynamicImage::ImageRgba8(img1),
            DynamicImage::ImageRgba8(img2),
        ];
        let probes = vec![
            ColorProbe { region: Some(RegionRect { x: 0, y: 0, width: 2, height: 2 }), targets: vec![ColorTarget { color: [255, 0, 0], tolerance: 10 }], probe_match_mode: ColorMatchMode::Any, legacy_target_color: None, legacy_tolerance: None },
            ColorProbe { region: Some(RegionRect { x: 0, y: 0, width: 2, height: 2 }), targets: vec![ColorTarget { color: [255, 0, 0], tolerance: 10 }], probe_match_mode: ColorMatchMode::Any, legacy_target_color: None, legacy_tolerance: None },
        ];
        let all = match_color_probes(&screenshots, &probes, ColorMatchMode::All, ColorMatchMethod::AnyPixel);
        assert!(!all.matched, "mode=All 第二探针未中，不触发");
        assert_eq!(all.hit_count, 1);
        let any = match_color_probes(&screenshots, &probes, ColorMatchMode::Any, ColorMatchMethod::AnyPixel);
        assert!(any.matched, "mode=Any 任一命中即触发");
    }

    // ---- scan_region_for_color (Task 2) ----

    #[test]
    fn any_pixel_hit_single_pixel() {
        // 4×4 全黑，放 1 个目标色像素 → matching_count >= 1 且 nearest_color == 目标色
        let mut img = RgbaImage::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        img.put_pixel(2, 1, Rgba([200, 100, 50, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let result = scan_region_for_color(&dyn_img, [200, 100, 50], 10.0, false);
        assert!(result.matching_count >= 1, "应至少命中 1 像素");
        assert_eq!(result.nearest_color, [200, 100, 50], "最近像素应为目标色");
    }

    #[test]
    fn any_pixel_no_hit_returns_nearest() {
        // 全黑图，目标色为白 → 无命中，nearest 为全图最近像素（黑）
        let img = RgbaImage::from_pixel(3, 3, Rgba([0, 0, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let result = scan_region_for_color(&dyn_img, [255, 255, 255], 10.0, false);
        assert_eq!(result.matching_count, 0, "无命中像素");
        assert_eq!(result.nearest_color, [0, 0, 0], "最近像素为黑");
    }

    #[test]
    fn any_pixel_early_exit_count_only() {
        // 2×2 全为目标色，count_only=true → matching_count == 1（早退）
        let img = RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let result = scan_region_for_color(&dyn_img, [200, 100, 50], 10.0, true);
        assert_eq!(result.matching_count, 1, "count_only=true 命中后应早退，count 为 1");
    }

    #[test]
    fn any_pixel_tolerance_boundary() {
        // 像素距离恰等于容差 → 命中（<= 边界）
        // 目标 [100,100,100]，像素 [110,100,100] 距离 10，容差 10 → 命中
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([110, 100, 100, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let result = scan_region_for_color(&dyn_img, [100, 100, 100], 10.0, false);
        assert!(result.matching_count >= 1, "距离恰等于容差应命中");
    }

    // ---- probe_hit (Task 2) ----

    #[test]
    fn probe_hit_average_uses_region_avg() {
        // 2×2：一半红一半黑，平均色 ≈ [127,0,0]，目标 [127,0,0] 容差 5 → average 命中
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 1, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let probe = ColorProbe {
            region: Some(RegionRect { x: 0, y: 0, width: 2, height: 2 }),
            targets: vec![ColorTarget { color: [127, 0, 0], tolerance: 5 }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        };
        let hit = probe_hit(&dyn_img, &probe, ColorMatchMethod::Average, false);
        assert!(hit.matched, "average 模式平均色应命中");
        assert_eq!(hit.matching_pixel_count, 0, "average 模式 matching_pixel_count 恒 0");
    }

    #[test]
    fn probe_hit_any_pixel_finds_single_pixel() {
        // 同一图：average 未中（平均色距目标远），anyPixel 命中（存在红像素）
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 1, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let probe = ColorProbe {
            region: Some(RegionRect { x: 0, y: 0, width: 2, height: 2 }),
            targets: vec![ColorTarget { color: [255, 0, 0], tolerance: 10 }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        };
        let avg_hit = probe_hit(&dyn_img, &probe, ColorMatchMethod::Average, false);
        assert!(!avg_hit.matched, "average 模式平均色 [127,0,0] 距 [255,0,0] 远，应未中");
        let any_hit = probe_hit(&dyn_img, &probe, ColorMatchMethod::AnyPixel, false);
        assert!(any_hit.matched, "anyPixel 模式存在红像素应命中");
        assert!(any_hit.matching_pixel_count >= 1, "anyPixel 命中数应 >= 1");
    }
}
