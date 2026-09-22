//! 截屏 → 认人 → 数格 → 配指纹 → 点序。

use image::{imageops, imageops::FilterType, DynamicImage, GrayImage};

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::{
    matching::{
        assign_from_scores, best_ncc_similarity, click_order, crop_gray_margin,
        difficulty_from_occupied_count, match_person_by_ocr_text, occupancy_energy,
        select_occupied_indices,
    },
    types::{FingerprintMatch, FingerprintPerson, FingerprintRunResult, FingerprintSettings},
};
use crate::recognition::watcher::{capture_region, capture_regions, load_reference_image};

const TEMPLATE_MARGIN: f32 = 0.15;

fn template_cache() -> &'static Mutex<HashMap<String, GrayImage>> {
    static CACHE: OnceLock<Mutex<HashMap<String, GrayImage>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn invalidate_template_cache() {
    if let Ok(mut cache) = template_cache().lock() {
        cache.clear();
    }
}

fn load_gray_cached(path: &str) -> Option<GrayImage> {
    if let Ok(cache) = template_cache().lock() {
        if let Some(image) = cache.get(path) {
            return Some(image.clone());
        }
    }
    let image = to_gray(&load_reference_image(path)?);
    if let Ok(mut cache) = template_cache().lock() {
        cache.insert(path.to_string(), image.clone());
    }
    Some(image)
}

pub struct PipelineOutput {
    pub result: FingerprintRunResult,
    pub points: Vec<(i32, i32, u64)>,
}

fn to_gray(image: &DynamicImage) -> GrayImage {
    image.to_luma8()
}

fn resize_gray(image: &GrayImage, width: u32, height: u32) -> GrayImage {
    if width == 0 || height == 0 {
        return image.clone();
    }
    imageops::resize(image, width, height, FilterType::Triangle)
}

fn score_matrix(
    candidate_grays: &[GrayImage],
    occupied_indices: &[usize],
    templates: &[GrayImage],
) -> Vec<Vec<f32>> {
    occupied_indices
        .iter()
        .map(|&index| {
            templates
                .iter()
                .map(|template| score_candidate(&candidate_grays[index], template))
                .collect()
        })
        .collect()
}

fn assignment_score(
    assignment: &[Option<usize>],
    occupied_indices: &[usize],
    vote_scores: &[Vec<f32>],
) -> (usize, f32) {
    let mut count = 0usize;
    let mut sum = 0.0_f32;
    for (template, candidate) in assignment.iter().enumerate() {
        let Some(candidate_index) = *candidate else {
            continue;
        };
        let Some(row) = occupied_indices
            .iter()
            .position(|&index| index == candidate_index)
        else {
            continue;
        };
        count += 1;
        sum += vote_scores[row][template];
    }
    (count, sum)
}

fn load_templates(person: &FingerprintPerson, count: usize) -> Result<Vec<GrayImage>, String> {
    let mut templates = Vec::with_capacity(count);
    for index in 0..count {
        let path = person.fingerprint_paths[index]
            .as_ref()
            .ok_or_else(|| format!("{} 缺少第 {} 枚指纹", person.name, index + 1))?;
        let image = load_gray_cached(path)
            .ok_or_else(|| format!("{} 第 {} 枚指纹读失败", person.name, index + 1))?;
        templates.push(image);
    }
    Ok(templates)
}

fn identify_person_by_name(settings: &FingerprintSettings) -> Option<usize> {
    let region = settings.name_region.as_ref()?;
    let image = capture_region(region)?;
    let words = crate::special_ops::windows_ocr::recognize_words(image).ok()?;
    let text: String = words.into_iter().map(|word| word.text).collect();
    let names: Vec<&str> = settings
        .people
        .iter()
        .map(|person| person.name.as_str())
        .collect();
    match_person_by_ocr_text(&text, &names)
}

fn score_candidate(candidate: &GrayImage, template: &GrayImage) -> f32 {
    let resized = resize_gray(template, candidate.width(), candidate.height());
    let candidate_core = crop_gray_margin(candidate, TEMPLATE_MARGIN);
    let template_core = crop_gray_margin(&resized, TEMPLATE_MARGIN);
    let aligned = if template_core.width() == candidate_core.width()
        && template_core.height() == candidate_core.height()
    {
        template_core
    } else {
        resize_gray(
            &template_core,
            candidate_core.width(),
            candidate_core.height(),
        )
    };
    best_ncc_similarity(&candidate_core, &aligned)
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

    if settings.candidate_boxes.iter().any(Option::is_none) {
        return Err("请先框完 9 个候选格".to_string());
    }
    let capture_rects: Vec<_> = settings
        .candidate_boxes
        .iter()
        .cloned()
        .map(|region| region.ok_or_else(|| "请先框完 9 个候选格".to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let captured = capture_regions(&capture_rects).ok_or_else(|| "截图失败".to_string())?;
    if captured.len() != 9 {
        return Err("截图失败".to_string());
    }

    let mut variances = Vec::with_capacity(9);
    let mut candidate_grays = Vec::with_capacity(9);
    for image in captured {
        let gray = to_gray(&image);
        variances.push(occupancy_energy(&gray));
        candidate_grays.push(gray);
    }
    let occupied_indices = select_occupied_indices(&variances)?;
    let occupied_count = occupied_indices.len();
    result.occupied_count = Some(occupied_count);
    let difficulty = difficulty_from_occupied_count(occupied_count)?;
    result.mode = Some(difficulty.label.to_string());

    let mut best: Option<(usize, Vec<Option<usize>>, Vec<Vec<f32>>, usize, f32)> = None;
    let mut consider = |person_index: usize| {
        let person = &settings.people[person_index];
        let Ok(templates) = load_templates(person, difficulty.template_count) else {
            return false;
        };
        let vote_scores = score_matrix(&candidate_grays, &occupied_indices, &templates);
        let assignment = assign_from_scores(
            &vote_scores,
            &occupied_indices,
            difficulty.template_count,
            settings.match_threshold,
        );
        let (assigned, sum) = assignment_score(&assignment, &occupied_indices, &vote_scores);
        let complete = assigned == difficulty.template_count;
        let better = match &best {
            None => true,
            Some((_, _, _, best_n, best_sum)) => {
                assigned > *best_n || (assigned == *best_n && sum > *best_sum)
            }
        };
        if better {
            best = Some((person_index, assignment, vote_scores, assigned, sum));
        }
        complete
    };

    let ocr_hit = identify_person_by_name(settings);
    if let Some(person_index) = ocr_hit {
        if consider(person_index) {
            // 名条读中且配齐，不再扫其他人。
        } else {
            for person_index in 0..settings.people.len() {
                if Some(person_index) != ocr_hit {
                    consider(person_index);
                }
            }
        }
    } else {
        for person_index in 0..settings.people.len() {
            consider(person_index);
        }
    }
    let Some((person_index, assignment, vote_scores, _, _)) = best else {
        return Err("图库里没有足够的档案指纹".to_string());
    };
    let person = &settings.people[person_index];
    result.person_id = Some(person.id.clone());
    result.person_name = Some(person.name.clone());
    result.matches = assignment
        .iter()
        .enumerate()
        .filter_map(|(template, candidate)| {
            let candidate_index = (*candidate)?;
            let row = occupied_indices
                .iter()
                .position(|&index| index == candidate_index)?;
            Some(FingerprintMatch {
                template_index: template + 1,
                candidate_index: candidate_index + 1,
                score: vote_scores[row][template],
            })
        })
        .collect();

    let order = match click_order(&assignment) {
        Ok(order) => order,
        Err(missing) => {
            let occupied_label = occupied_indices
                .iter()
                .map(|index| (index + 1).to_string())
                .collect::<Vec<_>>()
                .join(",");
            result.error = Some(format!(
                "{} · {}档 · {occupied_count}格有纹({occupied_label}) · {missing}",
                person.name, difficulty.label
            ));
            return Ok(PipelineOutput {
                result,
                points: Vec::new(),
            });
        }
    };

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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};
    use std::time::Instant;

    fn checker(width: u32, height: u32) -> GrayImage {
        let mut image = GrayImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = Luma([if (x + y) % 2 == 0 { 0 } else { 255 }]);
        }
        image
    }

    #[test]
    fn score_candidate_same_pattern_is_fast_and_high() {
        let image = checker(160, 160);
        let started = Instant::now();
        let score = score_candidate(&image, &image);
        assert!(
            started.elapsed().as_millis() < 50,
            "elapsed {:?}",
            started.elapsed()
        );
        assert!(score > 0.9, "score {score}");
    }

    #[test]
    fn assignment_score_counts_matched_templates() {
        let assignment = vec![Some(3), None, Some(0)];
        let occupied = vec![0, 3];
        let scores = vec![vec![0.1, 0.2, 0.9], vec![0.8, 0.1, 0.1]];
        let (count, sum) = assignment_score(&assignment, &occupied, &scores);
        assert_eq!(count, 2);
        assert!((sum - 1.7).abs() < 1e-5);
    }
}
