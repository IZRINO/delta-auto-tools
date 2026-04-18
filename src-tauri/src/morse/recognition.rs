use std::{
    collections::VecDeque,
    time::{SystemTime, UNIX_EPOCH},
};

use image::{DynamicImage, GrayImage, Luma, RgbaImage};
use xcap::Monitor;

use super::{
    decoder,
    types::{MorseRegionDetail, MorseRunResult, MorseSettings, RegionRect},
};

const DASH_RATIO_THRESHOLD: f32 = 2.0;
const MIN_CONTOUR_AREA: usize = 10;

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

#[derive(Debug)]
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
        Some(decoder::decode_sequence(
            decoded_digits.iter().map(String::as_str),
        )?)
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

    let left = region.x;
    let top = region.y;
    let right = region.x + region.width;
    let bottom = region.y + region.height;

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

        let monitor_right = monitor_left + monitor_width as i32;
        let monitor_bottom = monitor_top + monitor_height as i32;

        if left >= monitor_left
            && top >= monitor_top
            && right <= monitor_right
            && bottom <= monitor_bottom
        {
            let local_x = (left - monitor_left) as u32;
            let local_y = (top - monitor_top) as u32;
            let width = region.width as u32;
            let height = region.height as u32;

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

    let mut marks = detect_components(&binary)
        .into_iter()
        .filter(|component| component.area >= MIN_CONTOUR_AREA)
        .map(|component| {
            let height = component.height().max(1) as f32;
            let width = component.width() as f32;
            let symbol = if width / height >= DASH_RATIO_THRESHOLD {
                '-'
            } else {
                '.'
            };

            (component.min_x as i32, symbol)
        })
        .collect::<Vec<_>>();

    marks.sort_by_key(|(x, _)| *x);
    let contour_count = marks.len();

    if contour_count != 5 {
        return Err(DetectionFailure {
            threshold_mode: mode,
            contour_count,
            morse: (contour_count > 0).then(|| marks.iter().map(|(_, symbol)| *symbol).collect()),
            message: format!("期望 5 个轮廓，实际 {contour_count}，当前阈值 {binary_threshold}"),
        });
    }

    let morse: String = marks.iter().map(|(_, symbol)| *symbol).collect();
    Ok(DetectionSuccess {
        threshold_mode: mode,
        contour_count,
        morse,
    })
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
