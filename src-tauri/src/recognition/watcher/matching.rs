//! NCC 图像匹配 + 颜色匹配算法

use crate::recognition::types::{ColorMatchMethod, ColorMatchMode, ColorProbe};

// ── 图像比较 ──────────────────────────────────────────────────

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
pub(crate) fn compare_images(
    screenshot: &image::DynamicImage,
    reference: &image::DynamicImage,
) -> CompareResult {
    let sw = screenshot.width();
    let sh = screenshot.height();
    let rw = reference.width();
    let rh = reference.height();

    if sw == 0 || sh == 0 || rw == 0 || rh == 0 {
        return CompareResult {
            similarity: 0.0,
            best_x: 0,
            best_y: 0,
        };
    }

    // 参考图比截图大，不可能匹配
    if rw > sw || rh > sh {
        return CompareResult {
            similarity: 0.0,
            best_x: 0,
            best_y: 0,
        };
    }

    // 同尺寸：直接比较
    if rw == sw && rh == sh {
        let s_rgba = screenshot.to_rgba8();
        let r_rgba = reference.to_rgba8();
        let r_has_alpha = has_alpha_channel(reference);
        let ncc = compute_ncc_rgb(&s_rgba, &r_rgba, r_has_alpha, 0, 0, rw, rh);
        let score = ncc_to_similarity(ncc);
        return CompareResult {
            similarity: score,
            best_x: 0,
            best_y: 0,
        };
    }

    // 参考图比截图小：模板匹配
    template_match(screenshot, reference)
}

/// 比较多个参考图，返回相似度最高的参考图下标与结果。
pub(crate) fn best_reference_match<'a>(
    screenshot: &image::DynamicImage,
    references: impl IntoIterator<Item = &'a image::DynamicImage>,
) -> Option<(usize, CompareResult)> {
    references
        .into_iter()
        .enumerate()
        .map(|(index, reference)| (index, compare_images(screenshot, reference)))
        .max_by(|(_, left), (_, right)| left.similarity.total_cmp(&right.similarity))
}

/// 滑动窗口模板匹配（含多尺度加速）
fn template_match(
    screenshot: &image::DynamicImage,
    reference: &image::DynamicImage,
) -> CompareResult {
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
        let (best_ncc, best_x, best_y) = sliding_search(
            &s_rgba,
            &r_rgba,
            r_has_alpha,
            rw,
            rh,
            SearchBounds::new(0, 0, sw - rw, sh - rh),
        );
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
    if new_sw == 0
        || new_sh == 0
        || new_rw == 0
        || new_rh == 0
        || new_rw > new_sw
        || new_rh > new_sh
    {
        // 退化到全分辨率
        let s_rgba = screenshot.to_rgba8();
        let r_rgba = reference.to_rgba8();
        let (best_ncc, best_x, best_y) = sliding_search(
            &s_rgba,
            &r_rgba,
            r_has_alpha,
            rw,
            rh,
            SearchBounds::new(0, 0, sw - rw, sh - rh),
        );
        return CompareResult {
            similarity: ncc_to_similarity(best_ncc),
            best_x,
            best_y,
        };
    }

    let s_small = screenshot
        .resize_exact(new_sw, new_sh, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let r_small = reference
        .resize_exact(new_rw, new_rh, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    // 缩放后参考图不再有有意义的 alpha，按无 alpha 处理
    let (_, coarse_x, coarse_y) = sliding_search(
        &s_small,
        &r_small,
        false,
        new_rw,
        new_rh,
        SearchBounds::new(0, 0, new_sw - new_rw, new_sh - new_rh),
    );

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
    let (best_ncc, best_x, best_y) = sliding_search(
        &s_rgba,
        &r_rgba,
        r_has_alpha,
        rw,
        rh,
        SearchBounds::new(x_start, y_start, x_end, y_end),
    );

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

/// 滑动窗口搜索：在指定边界内搜索最佳匹配。
#[derive(Clone, Copy)]
struct SearchBounds {
    x_start: u32,
    y_start: u32,
    x_end: u32,
    y_end: u32,
}

impl SearchBounds {
    const fn new(x_start: u32, y_start: u32, x_end: u32, y_end: u32) -> Self {
        Self {
            x_start,
            y_start,
            x_end,
            y_end,
        }
    }
}

fn sliding_search(
    s_rgba: &image::RgbaImage,
    r_rgba: &image::RgbaImage,
    r_has_alpha: bool,
    rw: u32,
    rh: u32,
    bounds: SearchBounds,
) -> (f32, u32, u32) {
    let mut best_ncc: f32 = f32::NEG_INFINITY;
    let mut best_x: u32 = 0;
    let mut best_y: u32 = 0;

    for y in bounds.y_start..=bounds.y_end {
        for x in bounds.x_start..=bounds.x_end {
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
    let ncc_r = if denom_ra > 0.0 && denom_rb > 0.0 {
        num_r / (denom_ra * denom_rb).sqrt()
    } else if denom_ra < 1e-10 && denom_rb < 1e-10 {
        if (mean_ra - mean_rb).abs() < 1.0 {
            1.0
        } else {
            0.0
        }
    } else {
        0.0
    };
    let ncc_g = if denom_ga > 0.0 && denom_gb > 0.0 {
        num_g / (denom_ga * denom_gb).sqrt()
    } else if denom_ga < 1e-10 && denom_gb < 1e-10 {
        if (mean_ga - mean_gb).abs() < 1.0 {
            1.0
        } else {
            0.0
        }
    } else {
        0.0
    };
    let ncc_b = if denom_ba > 0.0 && denom_bb > 0.0 {
        num_b / (denom_ba * denom_bb).sqrt()
    } else if denom_ba < 1e-10 && denom_bb < 1e-10 {
        if (mean_ba - mean_bb).abs() < 1.0 {
            1.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    (ncc_r + ncc_g + ncc_b) / 3.0
}

/// NCC 值（范围 [-1, 1]）映射到相似度（范围 [0, 1]）
fn ncc_to_similarity(ncc: f32) -> f32 {
    ((ncc + 1.0) / 2.0).clamp(0.0, 1.0)
}

// ── 颜色匹配 ──────────────────────────────────────────────────

/// 颜色匹配结果
#[derive(Debug, Clone)]
pub(crate) struct ColorMatchResult {
    /// 是否触发（按 mode 聚合后）
    pub matched: bool,
    /// 命中的 probe 数量
    pub hit_count: usize,
    /// 命中的 probe 与局部像素坐标
    pub matched_probes: Vec<MatchedColorProbe>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchedColorProbe {
    pub index: usize,
    pub match_position: Option<(u32, u32)>,
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
    [(sum_r / n) as u8, (sum_g / n) as u8, (sum_b / n) as u8]
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
    /// 容差内与目标色距离最小的像素坐标
    pub match_position: Option<(u32, u32)>,
}

/// 扫描区域，返回命中像素数与最接近目标色的像素。
///
/// - `count_only=true`：精确命中（距离 0）时早退；非精确命中全扫以选择距离最小的像素。
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
    let mut best_match_distance = f32::INFINITY;
    let mut match_position = None;

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
                if dist < best_match_distance {
                    best_match_distance = dist;
                    match_position = Some((x, y));
                }
                if count_only && dist == 0.0 {
                    return PixelScanResult {
                        matching_count,
                        nearest_color,
                        nearest_distance,
                        match_position,
                    };
                }
            }
        }
    }

    if nearest_distance == f32::INFINITY {
        // 全透明或空图
        nearest_distance = 0.0;
    }

    PixelScanResult {
        matching_count,
        nearest_color,
        nearest_distance,
        match_position,
    }
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
    /// anyPixel 实际命中像素坐标；average 为 None
    pub match_position: Option<(u32, u32)>,
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
    pub match_position: Option<(u32, u32)>,
}

/// 单探针内多目标判定：对 `probe.targets` 每个目标分别按 method 判定，返回每个目标的命中详情。
///
/// - `count_only=true`：anyPixel 精确命中时早退，非精确命中全扫以保留最佳坐标。
/// - `count_only=false`：全扫拿真实命中数与最近像素（test 用，调试优先）。
///
/// 返回值顺序与 `probe.targets` 一致。
pub(crate) fn probe_hit_targets(
    screenshot: &image::DynamicImage,
    probe: &ColorProbe,
    method: ColorMatchMethod,
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
                match_position: hit.match_position,
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
    match_position: Option<(u32, u32)>,
}

/// 单探针对单个 `ColorTarget` 的判定：按 method 计算 sampled 与目标色的距离。
fn probe_hit_single_target(
    screenshot: &image::DynamicImage,
    target: &crate::recognition::types::ColorTarget,
    method: ColorMatchMethod,
    count_only: bool,
) -> SingleTargetHit {
    match method {
        ColorMatchMethod::Average => {
            let avg = average_region_rgb(screenshot);
            let dist = color_distance(avg, target.color);
            SingleTargetHit {
                matched: dist <= target.tolerance as f32,
                sampled_color: avg,
                distance: dist,
                matching_pixel_count: 0,
                match_position: None,
            }
        }
        ColorMatchMethod::AnyPixel => {
            let scan = scan_region_for_color(
                screenshot,
                target.color,
                target.tolerance as f32,
                count_only,
            );
            SingleTargetHit {
                matched: scan.matching_count > 0,
                sampled_color: scan.nearest_color,
                distance: scan.nearest_distance,
                matching_pixel_count: scan.matching_count,
                match_position: scan.match_position,
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
fn aggregate_probe_hits(hits: &[TargetHit], probe_match_mode: ColorMatchMode) -> ProbeHit {
    if hits.is_empty() {
        return ProbeHit {
            matched: false,
            sampled_color: [0, 0, 0],
            distance: f32::INFINITY,
            target_color: [0, 0, 0],
            tolerance: 0,
            matching_pixel_count: 0,
            match_position: None,
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
        .min_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();
    let match_position = hits
        .iter()
        .filter(|hit| hit.matched)
        .min_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|hit| hit.match_position);
    ProbeHit {
        matched,
        sampled_color: nearest.sampled_color,
        distance: nearest.distance,
        target_color: nearest.target_color,
        tolerance: nearest.tolerance,
        matching_pixel_count: nearest.matching_pixel_count,
        match_position,
    }
}

/// pub(crate) wrapper：供 mod.rs 的 recognition_test_color_match 命令复用
pub(crate) fn aggregate_probe_hits_pub(
    hits: &[TargetHit],
    probe_match_mode: ColorMatchMode,
) -> ProbeHit {
    aggregate_probe_hits(hits, probe_match_mode)
}

/// 单探针判定：对 `probe.targets` 按 `probe.probe_match_mode` 聚合后返回探针级摘要。
///
/// - `count_only=true`：anyPixel 精确命中时早退，非精确命中全扫以保留最佳坐标。
/// - `count_only=false`：全扫拿真实命中数与最近像素（test 用，调试优先）。
pub(crate) fn probe_hit(
    screenshot: &image::DynamicImage,
    probe: &ColorProbe,
    method: ColorMatchMethod,
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
        return ColorMatchResult {
            matched: false,
            hit_count: 0,
            matched_probes: Vec::new(),
        };
    }
    let mut hit_count = 0usize;
    let mut matched_probes = Vec::new();
    for (i, probe) in probes.iter().enumerate() {
        let hit = probe_hit(&screenshots[i], probe, method.clone(), true);
        if hit.matched {
            hit_count += 1;
            matched_probes.push(MatchedColorProbe {
                index: i,
                match_position: hit.match_position,
            });
        }
    }
    let matched = match mode {
        ColorMatchMode::All => hit_count == probes.len(),
        ColorMatchMode::Any => hit_count > 0,
    };
    ColorMatchResult {
        matched,
        hit_count,
        matched_probes,
    }
}
