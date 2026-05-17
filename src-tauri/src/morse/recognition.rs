use std::collections::VecDeque;

use image::{DynamicImage, GrayImage, Luma, RgbaImage};
use xcap::Monitor;

use crate::utils::now_ms;

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
    let occurred_at_ms = now_ms();
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

pub fn missing_regions_details() -> Vec<MorseRegionDetail> {
    (0..3)
        .map(|slot| MorseRegionDetail {
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

    let monitors = Monitor::all().map_err(|error| DetectionFailure {
        threshold_mode: "not-run",
        contour_count: 0,
        morse: None,
        message: format!("读取显示器信息失败: {error}"),
    })?;

    for monitor in monitors {
        let monitor_left = monitor.x().map_err(|error| DetectionFailure {
            threshold_mode: "not-run",
            contour_count: 0,
            morse: None,
            message: format!("读取显示器坐标失败: {error}"),
        })?;
        let monitor_top = monitor.y().map_err(|error| DetectionFailure {
            threshold_mode: "not-run",
            contour_count: 0,
            morse: None,
            message: format!("读取显示器坐标失败: {error}"),
        })?;
        let monitor_width = monitor.width().map_err(|error| DetectionFailure {
            threshold_mode: "not-run",
            contour_count: 0,
            morse: None,
            message: format!("读取显示器宽度失败: {error}"),
        })?;
        let monitor_height = monitor.height().map_err(|error| DetectionFailure {
            threshold_mode: "not-run",
            contour_count: 0,
            morse: None,
            message: format!("读取显示器高度失败: {error}"),
        })?;
        let scale_factor = monitor.scale_factor().unwrap_or(1.0);

        if let Some((local_x, local_y, width, height)) = region_to_capture_bounds(
            region,
            monitor_left,
            monitor_top,
            monitor_width,
            monitor_height,
            scale_factor,
        ) {
            return monitor
                .capture_region(local_x, local_y, width, height)
                .map_err(|error| DetectionFailure {
                    threshold_mode: "not-run",
                    contour_count: 0,
                    morse: None,
                    message: format!("截图失败: {error}"),
                });
        }
    }

    Err(DetectionFailure {
        threshold_mode: "not-run",
        contour_count: 0,
        morse: None,
        message: "所选区域未完全落在单个显示器内，请重新框选".to_string(),
    })
}

fn region_to_capture_bounds(
    region: &RegionRect,
    monitor_left: i32,
    monitor_top: i32,
    monitor_width: u32,
    monitor_height: u32,
    scale_factor: f32,
) -> Option<(u32, u32, u32, u32)> {
    let monitor_right = monitor_left + monitor_width as i32;
    let monitor_bottom = monitor_top + monitor_height as i32;
    let physical = rect_bounds(region.x, region.y, region.width, region.height);

    if rect_fits_within(physical, monitor_left, monitor_top, monitor_right, monitor_bottom) {
        return Some(to_local_capture_bounds(physical, monitor_left, monitor_top));
    }

    if (scale_factor - 1.0).abs() <= SCALE_FACTOR_TOLERANCE {
        return None;
    }

    let logical_left = (monitor_left as f32 / scale_factor).round() as i32;
    let logical_top = (monitor_top as f32 / scale_factor).round() as i32;
    let logical_right = logical_left + (monitor_width as f32 / scale_factor).round() as i32;
    let logical_bottom = logical_top + (monitor_height as f32 / scale_factor).round() as i32;
    let logical = rect_bounds(region.x, region.y, region.width, region.height);

    if !rect_fits_within(logical, logical_left, logical_top, logical_right, logical_bottom) {
        return None;
    }

    let scaled_left = ((logical.0 - logical_left) as f32 * scale_factor).round() as u32;
    let scaled_top = ((logical.1 - logical_top) as f32 * scale_factor).round() as u32;
    let scaled_width = (region.width as f32 * scale_factor).round() as u32;
    let scaled_height = (region.height as f32 * scale_factor).round() as u32;

    Some((scaled_left, scaled_top, scaled_width.max(1), scaled_height.max(1)))
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

fn to_local_capture_bounds(rect: (i32, i32, i32, i32), monitor_left: i32, monitor_top: i32) -> (u32, u32, u32, u32) {
    (
        (rect.0 - monitor_left) as u32,
        (rect.1 - monitor_top) as u32,
        (rect.2 - rect.0) as u32,
        (rect.3 - rect.1) as u32,
    )
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
            Ok(success) => return Ok(success),
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
            morse: Some(components_to_morse(&select_components(components.as_slice()))),
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


