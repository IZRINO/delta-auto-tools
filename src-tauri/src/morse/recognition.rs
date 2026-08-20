use std::collections::VecDeque;

use image::{DynamicImage, GrayImage, Luma, RgbaImage};

use super::{
    decoder,
    types::{MorseRegionDetail, MorseRunResult, MorseSettings, RegionRect},
};

const DASH_RATIO_THRESHOLD: f32 = 2.0;
const MIN_CONTOUR_AREA: usize = 10;
const TARGET_SYMBOL_COUNT: usize = 5;
const MAX_COMPONENTS_TO_KEEP: usize = 8;
const SCALE_FACTOR_TOLERANCE: f32 = 0.01;

struct DetectionSuccess {
    threshold_mode: &'static str,
    contour_count: usize,
    morse: String,
}

#[derive(Debug, Clone)]
struct DetectionFailure {
    threshold_mode: &'static str,
    contour_count: usize,
    morse: Option<String>,
    message: String,
}

#[derive(Debug, Clone)]
struct ComponentBounds {
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
    area: usize,
}

impl ComponentBounds {
    fn new(x: u32, y: u32) -> Self {
        Self {
            min_x: x,
            max_x: x,
            min_y: y,
            max_y: y,
            area: 0,
        }
    }

    fn include(&mut self, x: u32, y: u32) {
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
        self.area += 1;
    }

    fn width(&self) -> u32 {
        self.max_x - self.min_x + 1
    }

    fn height(&self) -> u32 {
        self.max_y - self.min_y + 1
    }
}

pub async fn run_recognition(
    settings: &MorseSettings,
    triggered_by: &str,
) -> Result<MorseRunResult, String> {
    let occurred_at_ms = chrono::Utc::now().timestamp_millis() as u64;
    let mut details = Vec::with_capacity(3);
    let mut decoded_digits = Vec::with_capacity(3);
    let mut errors = Vec::new();

    for (slot, region) in settings.regions.iter().enumerate() {
        let detail = match region {
            Some(region) => recognize_slot(slot, region, settings.binary_threshold),
            None => MorseRegionDetail {
                slot,
                threshold_mode: "not-run".to_string(),
                contour_count: 0,
                morse: None,
                digit: None,
                error: Some("该区域尚未配置".to_string()),
            },
        };

        if let Some(digit) = &detail.digit {
            decoded_digits.push(digit.clone());
        }

        if let Some(error) = &detail.error {
            errors.push(format!("区域 {}: {error}", slot + 1));
        }

        details.push(detail);
    }

    let value = if errors.is_empty() {
        Some(decoded_digits.join(""))
    } else {
        None
    };

    Ok(MorseRunResult {
        value,
        details,
        triggered_by: triggered_by.to_string(),
        auto_typed: false,
        occurred_at_ms,
        error: (!errors.is_empty()).then_some(errors.join("；")),
    })
}

pub fn missing_regions_details(regions: &[Option<RegionRect>; 3]) -> Vec<MorseRegionDetail> {
    regions
        .iter()
        .enumerate()
        .filter(|&(_slot, region)| region.is_none())
        .map(|(slot, _region)| MorseRegionDetail {
            slot,
            threshold_mode: "not-run".to_string(),
            contour_count: 0,
            morse: None,
            digit: None,
            error: Some("该区域尚未配置".to_string()),
        })
        .collect()
}

fn recognize_slot(slot: usize, region: &RegionRect, binary_threshold: u8) -> MorseRegionDetail {
    match capture_region(region).and_then(|image| detect_morse(&image, binary_threshold)) {
        Ok(success) => match decoder::decode(success.morse.as_str()) {
            Ok(digit) => MorseRegionDetail {
                slot,
                threshold_mode: success.threshold_mode.to_string(),
                contour_count: success.contour_count,
                morse: Some(success.morse),
                digit: Some(digit.to_string()),
                error: None,
            },
            Err(error) => MorseRegionDetail {
                slot,
                threshold_mode: success.threshold_mode.to_string(),
                contour_count: success.contour_count,
                morse: Some(success.morse),
                digit: None,
                error: Some(error),
            },
        },
        Err(failure) => MorseRegionDetail {
            slot,
            threshold_mode: failure.threshold_mode.to_string(),
            contour_count: failure.contour_count,
            morse: failure.morse,
            digit: None,
            error: Some(failure.message),
        },
    }
}

fn capture_region(region: &RegionRect) -> Result<RgbaImage, DetectionFailure> {
    if region.width <= 0 || region.height <= 0 {
        return Err(DetectionFailure {
            threshold_mode: "not-run",
            contour_count: 0,
            morse: None,
            message: "区域尺寸无效，请重新框选".to_string(),
        });
    }

    crate::recognition::watcher::capture_region(region)
        .map(|image| image.to_rgba8())
        .ok_or_else(|| DetectionFailure {
            threshold_mode: "not-run",
            contour_count: 0,
            morse: None,
            message: "截图失败".to_string(),
        })
}

pub fn region_to_capture_bounds(
    region: &RegionRect,
    monitor_left: i32,
    monitor_top: i32,
    monitor_width: u32,
    monitor_height: u32,
    scale_factor: f32,
) -> Option<(u32, u32, u32, u32)> {
    let physical = rect_bounds(region.x, region.y, region.width, region.height);

    if scale_factor > 0.0 && (scale_factor - 1.0).abs() > SCALE_FACTOR_TOLERANCE {
        let logical_left = (monitor_left as f32 / scale_factor).round() as i32;
        let logical_top = (monitor_top as f32 / scale_factor).round() as i32;
        let logical_right = logical_left + (monitor_width as f32 / scale_factor).round() as i32;
        let logical_bottom = logical_top + (monitor_height as f32 / scale_factor).round() as i32;

        if rect_fits_within(
            physical,
            logical_left,
            logical_top,
            logical_right,
            logical_bottom,
        ) {
            return Some(to_scaled_local_capture_bounds(
                physical,
                logical_left,
                logical_top,
                monitor_width,
                monitor_height,
                scale_factor,
            ));
        }
    }

    let monitor_right = monitor_left + monitor_width as i32;
    let monitor_bottom = monitor_top + monitor_height as i32;

    rect_fits_within(
        physical,
        monitor_left,
        monitor_top,
        monitor_right,
        monitor_bottom,
    )
    .then(|| to_local_capture_bounds(physical, monitor_left, monitor_top))
}

fn rect_bounds(x: i32, y: i32, width: i32, height: i32) -> (i32, i32, i32, i32) {
    (x, y, x + width, y + height)
}

fn rect_fits_within(
    rect: (i32, i32, i32, i32),
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) -> bool {
    rect.0 >= left && rect.1 >= top && rect.2 <= right && rect.3 <= bottom
}

fn to_local_capture_bounds(
    rect: (i32, i32, i32, i32),
    monitor_left: i32,
    monitor_top: i32,
) -> (u32, u32, u32, u32) {
    (
        (rect.0 - monitor_left) as u32,
        (rect.1 - monitor_top) as u32,
        (rect.2 - rect.0) as u32,
        (rect.3 - rect.1) as u32,
    )
}

fn to_scaled_local_capture_bounds(
    rect: (i32, i32, i32, i32),
    logical_left: i32,
    logical_top: i32,
    monitor_width: u32,
    monitor_height: u32,
    scale_factor: f32,
) -> (u32, u32, u32, u32) {
    let left = ((rect.0 - logical_left) as f32 * scale_factor).round() as i32;
    let top = ((rect.1 - logical_top) as f32 * scale_factor).round() as i32;
    let right = ((rect.2 - logical_left) as f32 * scale_factor).round() as i32;
    let bottom = ((rect.3 - logical_top) as f32 * scale_factor).round() as i32;

    let left = left.clamp(0, monitor_width.saturating_sub(1) as i32) as u32;
    let top = top.clamp(0, monitor_height.saturating_sub(1) as i32) as u32;
    let right = (right.max(left as i32 + 1) as u32).min(monitor_width);
    let bottom = (bottom.max(top as i32 + 1) as u32).min(monitor_height);

    (left, top, right - left, bottom - top)
}

fn detect_morse(
    image: &RgbaImage,
    binary_threshold: u8,
) -> Result<DetectionSuccess, DetectionFailure> {
    let gray = rgba_to_gray(image);
    let otsu = otsu_threshold(&gray);

    let stages = [
        ("otsu-forward", otsu, false),
        ("otsu-inverse", otsu, true),
        ("manual", binary_threshold, false),
    ];

    let mut last_failure = DetectionFailure {
        threshold_mode: "not-run",
        contour_count: 0,
        morse: None,
        message: format!("期望 5 个轮廓，实际 0，当前阈值 {binary_threshold}"),
    };

    for (mode, threshold_value, invert) in stages {
        match detect_morse_with_threshold(&gray, mode, threshold_value, invert, binary_threshold) {
            Ok(success) if decoder::decode(success.morse.as_str()).is_ok() => return Ok(success),
            Ok(success) => {
                last_failure = DetectionFailure {
                    threshold_mode: success.threshold_mode,
                    contour_count: success.contour_count,
                    morse: Some(success.morse.clone()),
                    message: format!("无法识别的摩斯密码: {}", success.morse),
                };
            }
            Err(failure) => last_failure = failure,
        }
    }

    Err(last_failure)
}

fn detect_morse_with_threshold(
    gray: &GrayImage,
    mode: &'static str,
    threshold_value: u8,
    invert: bool,
    binary_threshold: u8,
) -> Result<DetectionSuccess, DetectionFailure> {
    let binary = apply_threshold(gray, threshold_value, invert);
    let components = detect_components(&binary)
        .into_iter()
        .filter(|component| component.area >= MIN_CONTOUR_AREA)
        .collect::<Vec<_>>();
    let contour_count = components.len();

    if contour_count < TARGET_SYMBOL_COUNT {
        return Err(DetectionFailure {
            threshold_mode: mode,
            contour_count,
            morse: (contour_count > 0)
                .then(|| components_to_morse(&select_components(components.as_slice()))),
            message: format!(
                "期望至少 {TARGET_SYMBOL_COUNT} 个轮廓，实际 {contour_count}，当前阈值 {binary_threshold}"
            ),
        });
    }

    if contour_count > MAX_COMPONENTS_TO_KEEP {
        return Err(DetectionFailure {
            threshold_mode: mode,
            contour_count,
            morse: Some(components_to_morse(&select_components(
                components.as_slice(),
            ))),
            message: format!(
                "轮廓过多（{contour_count}），当前阈值 {binary_threshold}，请重新框选或调整阈值"
            ),
        });
    }

    let selected = select_components(components.as_slice());
    let morse = components_to_morse(selected.as_slice());

    Ok(DetectionSuccess {
        threshold_mode: mode,
        contour_count,
        morse,
    })
}

fn select_components(components: &[ComponentBounds]) -> Vec<ComponentBounds> {
    let mut selected = components.to_vec();
    selected.sort_by(|left, right| {
        right
            .area
            .cmp(&left.area)
            .then_with(|| left.min_x.cmp(&right.min_x))
    });
    selected.truncate(TARGET_SYMBOL_COUNT);
    selected.sort_by_key(|component| component.min_x);
    selected
}

fn components_to_morse(components: &[ComponentBounds]) -> String {
    components
        .iter()
        .map(|component| {
            let height = component.height().max(1) as f32;
            let width = component.width() as f32;
            if width / height >= DASH_RATIO_THRESHOLD {
                '-'
            } else {
                '.'
            }
        })
        .collect()
}

fn rgba_to_gray(image: &RgbaImage) -> GrayImage {
    DynamicImage::ImageRgba8(image.clone()).into_luma8()
}

fn otsu_threshold(gray: &GrayImage) -> u8 {
    let mut histogram = [0u64; 256];
    for pixel in gray.pixels() {
        histogram[pixel[0] as usize] += 1;
    }

    let total = u64::from(gray.width()) * u64::from(gray.height());
    if total == 0 {
        return 0;
    }

    let mut sum_total = 0u64;
    for (level, count) in histogram.iter().enumerate() {
        sum_total += (level as u64) * count;
    }

    let mut sum_background = 0u64;
    let mut weight_background = 0u64;
    let mut max_variance = 0f64;
    let mut best_threshold = 0u8;

    for (level, count) in histogram.iter().enumerate() {
        weight_background += count;
        if weight_background == 0 {
            continue;
        }

        let weight_foreground = total - weight_background;
        if weight_foreground == 0 {
            break;
        }

        sum_background += (level as u64) * count;

        let mean_background = sum_background as f64 / weight_background as f64;
        let mean_foreground = (sum_total - sum_background) as f64 / weight_foreground as f64;
        let variance = (weight_background as f64)
            * (weight_foreground as f64)
            * (mean_background - mean_foreground).powi(2);

        if variance > max_variance {
            max_variance = variance;
            best_threshold = level as u8;
        }
    }

    best_threshold
}

fn apply_threshold(gray: &GrayImage, threshold: u8, invert: bool) -> GrayImage {
    let mut binary = GrayImage::new(gray.width(), gray.height());

    for (x, y, pixel) in gray.enumerate_pixels() {
        let foreground = if invert {
            pixel[0] <= threshold
        } else {
            pixel[0] > threshold
        };

        binary.put_pixel(x, y, Luma([if foreground { 255 } else { 0 }]));
    }

    binary
}

fn detect_components(binary: &GrayImage) -> Vec<ComponentBounds> {
    let width = binary.width() as usize;
    let mut visited = vec![false; width * binary.height() as usize];
    let mut components = Vec::new();

    for y in 0..binary.height() {
        for x in 0..binary.width() {
            let index = y as usize * width + x as usize;
            if visited[index] || binary.get_pixel(x, y)[0] == 0 {
                continue;
            }

            visited[index] = true;
            let mut queue = VecDeque::from([(x, y)]);
            let mut component = ComponentBounds::new(x, y);

            while let Some((cx, cy)) = queue.pop_front() {
                component.include(cx, cy);

                let min_x = cx.saturating_sub(1);
                let max_x = (cx + 1).min(binary.width().saturating_sub(1));
                let min_y = cy.saturating_sub(1);
                let max_y = (cy + 1).min(binary.height().saturating_sub(1));

                for ny in min_y..=max_y {
                    for nx in min_x..=max_x {
                        let neighbor_index = ny as usize * width + nx as usize;
                        if visited[neighbor_index] || binary.get_pixel(nx, ny)[0] == 0 {
                            continue;
                        }

                        visited[neighbor_index] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }

            components.push(component);
        }
    }

    components
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> RegionRect {
        RegionRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn region_to_capture_bounds_keeps_physical_region_when_scale_is_one() {
        let bounds = region_to_capture_bounds(&rect(100, 120, 80, 30), 0, 0, 1920, 1080, 1.0);

        assert_eq!(bounds, Some((100, 120, 80, 30)));
    }

    #[test]
    fn region_to_capture_bounds_scales_logical_overlay_region_on_high_dpi_monitor() {
        let bounds = region_to_capture_bounds(&rect(100, 120, 80, 30), 0, 0, 1920, 1080, 2.0);

        assert_eq!(bounds, Some((200, 240, 160, 60)));
    }

    #[test]
    fn region_to_capture_bounds_scales_logical_region_for_non_primary_monitor() {
        let bounds = region_to_capture_bounds(&rect(1060, 100, 100, 40), 1920, 0, 2560, 1440, 2.0);

        assert_eq!(bounds, Some((200, 200, 200, 80)));
    }

    #[test]
    fn region_to_capture_bounds_rejects_cross_monitor_region() {
        let bounds = region_to_capture_bounds(&rect(900, 100, 200, 40), 0, 0, 960, 540, 2.0);

        assert_eq!(bounds, None);
    }

    // --- Morse 识别核心算法单测 (VAL-AR-017 ~ VAL-AR-020) ---

    /// 构造双峰灰度图像：左半暗（50）、右半亮（200），验证 otsu 阈值在两者之间
    #[test]
    fn test_otsu_threshold_bimodal_histogram() {
        let width = 100u32;
        let height = 10u32;
        let mut gray = GrayImage::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let value = if x < width / 2 { 50 } else { 200 };
                gray.put_pixel(x, y, Luma([value]));
            }
        }

        let threshold = otsu_threshold(&gray);

        // 阈值应在 50 和 200 之间（理想值约为 125）
        assert!(
            (50..200).contains(&threshold),
            "otsu 阈值 {threshold} 应在 50 与 200 之间"
        );
    }

    /// 单峰图像（全灰 128），otsu 应返回 0（首个使方差最大的层级）
    #[test]
    fn test_otsu_threshold_single_peak() {
        let width = 50u32;
        let height = 50u32;
        let mut gray = GrayImage::new(width, height);

        for y in 0..height {
            for x in 0..width {
                gray.put_pixel(x, y, Luma([128]));
            }
        }

        let threshold = otsu_threshold(&gray);

        // 单峰直方图：方差始终 0，best_threshold 保持初始值 0
        assert_eq!(threshold, 0, "单峰图像 otsu 阈值应为 0");
    }

    /// 空图像（0×0），otsu 返回 0
    #[test]
    fn test_otsu_threshold_empty_image() {
        let gray = GrayImage::new(0, 0);
        let threshold = otsu_threshold(&gray);
        assert_eq!(threshold, 0, "空图像 otsu 阈值应为 0");
    }

    /// 构造 3 个不相连白色区域，验证 detect_components BFS 检测到 3 个连通域
    #[test]
    fn test_detect_components_three_islands() {
        // 30×10 二值图，在 x=[2..4] y=[2..4]、x=[10..14] y=[2..6]、x=[22..26] y=[4..8] 放白色块
        let width = 30u32;
        let height = 10u32;
        let mut binary = GrayImage::new(width, height);

        // 岛屿 1：3×3
        for y in 2..5u32 {
            for x in 2..5u32 {
                binary.put_pixel(x, y, Luma([255]));
            }
        }

        // 岛屿 2：4×4
        for y in 2..6u32 {
            for x in 10..14u32 {
                binary.put_pixel(x, y, Luma([255]));
            }
        }

        // 岛屿 3：4×4
        for y in 4..8u32 {
            for x in 22..26u32 {
                binary.put_pixel(x, y, Luma([255]));
            }
        }

        let components = detect_components(&binary);

        assert_eq!(
            components.len(),
            3,
            "应检测到 3 个连通域，实际 {}",
            components.len()
        );

        // 验证面积
        let mut areas: Vec<usize> = components.iter().map(|c| c.area).collect();
        areas.sort();
        assert_eq!(areas, [9, 16, 16], "连通域面积应为 [9, 16, 16]");
    }

    /// 全黑图像，detect_components 返回空
    #[test]
    fn test_detect_components_all_black() {
        let binary = GrayImage::new(20, 20);
        let components = detect_components(&binary);
        assert!(components.is_empty(), "全黑图像不应有连通域");
    }

    /// 单个白色像素，detect_components 返回 1 个面积=1 的连通域
    #[test]
    fn test_detect_components_single_pixel() {
        let mut binary = GrayImage::new(10, 10);
        binary.put_pixel(5, 5, Luma([255]));

        let components = detect_components(&binary);
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].area, 1);
    }

    /// components_to_morse：窄组件映射为 '.'，宽组件映射为 '-'
    #[test]
    fn test_components_to_morse_dot_and_dash() {
        // 窄组件（宽 2 高 10）→ '.'
        let dot = ComponentBounds {
            min_x: 0,
            max_x: 1,
            min_y: 0,
            max_y: 9,
            area: 20,
        };

        // 宽组件（宽 20 高 10）→ '-'
        let dash = ComponentBounds {
            min_x: 10,
            max_x: 29,
            min_y: 0,
            max_y: 9,
            area: 200,
        };

        let morse = components_to_morse(&[dot, dash]);
        assert_eq!(morse, ".-", "窄+宽组件应解码为 '.-'");
    }

    /// 5 个等宽窄组件 → "....."（对应数字 5）
    #[test]
    fn test_components_to_morse_five_dots() {
        let components: Vec<ComponentBounds> = (0..5)
            .map(|i| ComponentBounds {
                min_x: i * 6,
                max_x: i * 6 + 2,
                min_y: 0,
                max_y: 9,
                area: 30,
            })
            .collect();

        let morse = components_to_morse(&components);
        assert_eq!(morse, ".....", "5 个窄组件应解码为 '.....'");
    }

    /// 合成 5 组件 Morse 图像，验证 detect_morse 全链路解码
    /// 构造 "....."（数字 5）的合成图像
    #[test]
    fn test_detect_morse_synthetic_image_five_dots() {
        let width = 80u32;
        let height = 20u32;
        let mut image = RgbaImage::new(width, height);

        // 黑色背景
        for y in 0..height {
            for x in 0..width {
                image.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }

        // 5 个窄白色竖条（dot），间距 10px，宽 3px，高 16px
        for (i, x_offset) in [5, 18, 31, 44, 57].iter().enumerate() {
            // 确保至少 5 个组件
            let _ = i;
            for y in 2..18u32 {
                for dx in 0..3u32 {
                    image.put_pixel(x_offset + dx, y, image::Rgba([255, 255, 255, 255]));
                }
            }
        }

        let result = detect_morse(&image, 127);
        assert!(result.is_ok(), "detect_morse 应成功识别 5-dot 图像");

        let success = result.unwrap();
        assert_eq!(success.morse, ".....", "Morse 码应为 '.....'");
    }

    /// 合成 "-----"（数字 0）的图像，5 个宽横条
    #[test]
    fn test_detect_morse_synthetic_image_five_dashes() {
        let width = 160u32;
        let height = 20u32;
        let mut image = RgbaImage::new(width, height);

        // 黑色背景
        for y in 0..height {
            for x in 0..width {
                image.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }

        // 5 个宽白色横条（dash），宽 16px，高 4px
        for x_offset in [5, 30, 55, 80, 105] {
            for y in 8..12u32 {
                for dx in 0..16u32 {
                    image.put_pixel(x_offset + dx, y, image::Rgba([255, 255, 255, 255]));
                }
            }
        }

        let result = detect_morse(&image, 127);
        assert!(result.is_ok(), "detect_morse 应成功识别 5-dash 图像");

        let success = result.unwrap();
        assert_eq!(success.morse, "-----", "Morse 码应为 '-----'");
    }

    /// 组件不足 5 个时，detect_morse 返回 Err
    #[test]
    fn test_detect_morse_too_few_components() {
        let width = 40u32;
        let height = 20u32;
        let mut image = RgbaImage::new(width, height);

        // 黑色背景
        for y in 0..height {
            for x in 0..width {
                image.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }

        // 仅 2 个白色块
        for dx in 0..3u32 {
            for y in 5..15u32 {
                image.put_pixel(5 + dx, y, image::Rgba([255, 255, 255, 255]));
                image.put_pixel(20 + dx, y, image::Rgba([255, 255, 255, 255]));
            }
        }

        let result = detect_morse(&image, 127);
        assert!(result.is_err(), "少于 5 个组件应返回 Err");
    }
}
