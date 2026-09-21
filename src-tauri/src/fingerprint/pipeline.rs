//! 截屏 → 认人 → 数格 → 配指纹 → 点序。

use image::{imageops, imageops::FilterType, DynamicImage, GrayImage};

use super::{
    matching::{
        assign_templates, best_index_above, click_order, difficulty_from_occupied_count,
        is_occupied,
    },
    types::{FingerprintMatch, FingerprintPerson, FingerprintRunResult, FingerprintSettings},
};
use crate::morse::types::RegionRect;
use crate::recognition::watcher::{capture_region, compare_images, load_reference_image};

const TEMPLATE_MARGIN: f32 = 0.15;

pub struct PipelineOutput {
    pub result: FingerprintRunResult,
    pub points: Vec<(i32, i32, u64)>,
}

fn capture_required(region: &Option<RegionRect>, label: &str) -> Result<DynamicImage, String> {
    let region = region.as_ref().ok_or_else(|| format!("未校准{label}"))?;
    capture_region(region).ok_or_else(|| format!("{label}截图失败"))
}

fn to_gray(image: &DynamicImage) -> GrayImage {
    image.to_luma8()
}

fn resize_to(image: &DynamicImage, width: u32, height: u32) -> DynamicImage {
    if width == 0 || height == 0 {
        return image.clone();
    }
    DynamicImage::ImageRgba8(imageops::resize(
        &image.to_rgba8(),
        width,
        height,
        FilterType::Triangle,
    ))
}

fn crop_core(image: &DynamicImage) -> DynamicImage {
    let width = image.width();
    let height = image.height();
    let margin_x = ((width as f32) * TEMPLATE_MARGIN).round() as u32;
    let margin_y = ((height as f32) * TEMPLATE_MARGIN).round() as u32;
    let crop_w = width.saturating_sub(margin_x.saturating_mul(2)).max(1);
    let crop_h = height.saturating_sub(margin_y.saturating_mul(2)).max(1);
    image.crop_imm(
        margin_x.min(width.saturating_sub(1)),
        margin_y.min(height.saturating_sub(1)),
        crop_w,
        crop_h,
    )
}

fn ncc(screenshot: &DynamicImage, reference: &DynamicImage) -> f32 {
    compare_images(screenshot, reference).similarity
}

fn match_name<'a>(
    name_crop: &DynamicImage,
    people: &'a [FingerprintPerson],
    threshold: f32,
) -> Result<&'a FingerprintPerson, String> {
    if people.is_empty() {
        return Err("还没有采集任何人名".to_string());
    }
    let mut best_index = None;
    let mut best_score = f32::NEG_INFINITY;
    for (index, person) in people.iter().enumerate() {
        let Some(reference) = load_reference_image(&person.name_image_path) else {
            continue;
        };
        let resized = resize_to(&reference, name_crop.width(), name_crop.height());
        let score = ncc(name_crop, &resized);
        if score > best_score {
            best_score = score;
            best_index = Some(index);
        }
    }
    let index = best_index.ok_or_else(|| "人名参考图都读不出来".to_string())?;
    if best_score < threshold {
        return Err(format!(
            "无法认出人名（最高 {best_score:.2}，阈值 {threshold:.2}）"
        ));
    }
    Ok(&people[index])
}

fn load_templates(person: &FingerprintPerson, count: usize) -> Result<Vec<DynamicImage>, String> {
    let mut templates = Vec::with_capacity(count);
    for index in 0..count {
        let path = person.fingerprint_paths[index]
            .as_ref()
            .ok_or_else(|| format!("{} 缺少第 {} 枚指纹", person.name, index + 1))?;
        let image = load_reference_image(path)
            .ok_or_else(|| format!("{} 第 {} 枚指纹读失败", person.name, index + 1))?;
        templates.push(image);
    }
    Ok(templates)
}

fn score_candidate(candidate: &DynamicImage, template: &DynamicImage) -> f32 {
    let resized = resize_to(template, candidate.width(), candidate.height());
    let core = crop_core(&resized);
    let gray_candidate = DynamicImage::ImageLuma8(to_gray(candidate));
    let gray_core = DynamicImage::ImageLuma8(to_gray(&core));
    ncc(&gray_candidate, &gray_core)
}

pub fn run_pipeline_with_points(
    settings: &FingerprintSettings,
    triggered_by: &str,
    auto_click: bool,
) -> Result<PipelineOutput, String> {
    let occurred_at_ms = chrono::Utc::now().timestamp_millis() as u64;
    let mut result = FingerprintRunResult {
        person_id: None,
        person_name: None,
        mode: None,
        occupied_count: None,
        matches: Vec::new(),
        clicked: false,
        triggered_by: triggered_by.to_string(),
        occurred_at_ms,
        error: None,
    };

    let name_crop = capture_required(&settings.name_region, "名条")?;
    let person = match_name(&name_crop, &settings.people, settings.match_threshold)?;
    result.person_id = Some(person.id.clone());
    result.person_name = Some(person.name.clone());

    if settings.candidate_boxes.iter().any(Option::is_none) {
        return Err("请先框完 9 个候选格".to_string());
    }

    let mut occupied_flags = Vec::with_capacity(9);
    let mut candidate_images = Vec::with_capacity(9);
    for (index, region) in settings.candidate_boxes.iter().enumerate() {
        let image = capture_required(region, &format!("候选{}", index + 1))?;
        occupied_flags.push(is_occupied(&to_gray(&image), settings.occupancy_threshold));
        candidate_images.push(image);
    }
    let occupied_count = occupied_flags.iter().filter(|flag| **flag).count();
    result.occupied_count = Some(occupied_count);
    let difficulty = difficulty_from_occupied_count(occupied_count)?;
    result.mode = Some(difficulty.label.to_string());

    let used_boxes = &candidate_images[..difficulty.box_count];
    let templates = load_templates(person, difficulty.template_count)?;

    let mut votes = Vec::with_capacity(difficulty.box_count);
    let mut vote_scores = Vec::with_capacity(difficulty.box_count);
    for candidate in used_boxes {
        let scores: Vec<f32> = templates
            .iter()
            .map(|template| score_candidate(candidate, template))
            .collect();
        let best = best_index_above(&scores, settings.match_threshold);
        let score = best.map(|index| scores[index]).unwrap_or(0.0);
        votes.push((best, score));
        vote_scores.push(scores);
    }

    let assignment = assign_templates(&votes, difficulty.template_count, settings.match_threshold);
    let order = click_order(&assignment)?;
    result.matches = assignment
        .iter()
        .enumerate()
        .filter_map(|(template, candidate)| {
            candidate.map(|candidate_index| FingerprintMatch {
                template_index: template + 1,
                candidate_index: candidate_index + 1,
                score: vote_scores[candidate_index][template],
            })
        })
        .collect();

    let points: Vec<(i32, i32, u64)> = if auto_click && settings.auto_click_enabled {
        order
            .iter()
            .filter_map(|&candidate_index| {
                let rect = settings.candidate_boxes[candidate_index].as_ref()?;
                Some((
                    rect.x + rect.width / 2,
                    rect.y + rect.height / 2,
                    settings.click_delay_ms,
                ))
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(PipelineOutput { result, points })
}
