//! 档位、占用判定、模板贪心分配。纯函数，不碰截屏。

use image::{imageops, GrayImage};

const CORE_MARGIN: f32 = 0.15;

/// A=4 枚/5 格，B=6/7，C=8/9。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Difficulty {
    pub label: char,
    pub template_count: usize,
    pub box_count: usize,
}

pub fn luminance_variance(image: &GrayImage) -> f32 {
    let n = image.width().saturating_mul(image.height());
    if n == 0 {
        return 0.0;
    }
    let count = n as f32;
    let mut sum = 0.0_f32;
    let mut sum_sq = 0.0_f32;
    for pixel in image.pixels() {
        let value = f32::from(pixel.0[0]);
        sum += value;
        sum_sq += value * value;
    }
    let mean = sum / count;
    (sum_sq / count) - mean * mean
}

pub fn is_occupied(image: &GrayImage, threshold: f32) -> bool {
    luminance_variance(image) >= threshold
}

pub fn crop_gray_margin(image: &GrayImage, margin: f32) -> GrayImage {
    let width = image.width();
    let height = image.height();
    let margin_x = ((width as f32) * margin).round() as u32;
    let margin_y = ((height as f32) * margin).round() as u32;
    let crop_w = width.saturating_sub(margin_x.saturating_mul(2)).max(1);
    let crop_h = height.saturating_sub(margin_y.saturating_mul(2)).max(1);
    imageops::crop_imm(
        image,
        margin_x.min(width.saturating_sub(1)),
        margin_y.min(height.saturating_sub(1)),
        crop_w,
        crop_h,
    )
    .to_image()
}

/// 去掉槽边框后再算方差。空槽边框在全格上方差偏高，中心是平的。
pub fn occupancy_energy(image: &GrayImage) -> f32 {
    luminance_variance(&crop_gray_margin(image, CORE_MARGIN))
}

pub fn compact_text(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

/// OCR 读出名条后，按最长人名命中。位置框一次即可，不用每人采图。
pub fn match_person_by_ocr_text(text: &str, names: &[&str]) -> Option<usize> {
    let hay = compact_text(text);
    if hay.is_empty() {
        return None;
    }
    let mut order: Vec<usize> = (0..names.len()).collect();
    order.sort_by_key(|&index| std::cmp::Reverse(compact_text(names[index]).chars().count()));
    for index in order {
        let name = compact_text(names[index]);
        if !name.is_empty() && hay.contains(&name) {
            return Some(index);
        }
    }
    None
}

/// 按方差缺口选 5/7/9 格。输入应是 occupancy_energy（中心纹路），不是整格含边框。
pub fn select_occupied_indices(variances: &[f32]) -> Result<Vec<usize>, String> {
    if variances.len() != 9 {
        return Err(format!("候选格方差必须是 9 个，实际 {}", variances.len()));
    }
    let mut order: Vec<usize> = (0..9).collect();
    order.sort_by(|&left, &right| variances[right].total_cmp(&variances[left]));
    let gap = |index: usize| variances[order[index]] / (variances[order[index + 1]] + 1.0);
    let gap5 = gap(4);
    let gap7 = gap(6);
    const MIN_GAP: f32 = 2.0;
    let count = if gap5 >= MIN_GAP && gap5 >= gap7 {
        5
    } else if gap7 >= MIN_GAP {
        7
    } else if variances[order[8]] >= variances[order[0]] * 0.3 {
        9
    } else {
        return Err(format!(
            "有纹格子无法分成 5/7/9（最大方差 {:.0}，第 5/7/9 为 {:.0}/{:.0}/{:.0}）",
            variances[order[0]], variances[order[4]], variances[order[6]], variances[order[8]]
        ));
    };
    let mut indices = order[..count].to_vec();
    indices.sort_unstable();
    Ok(indices)
}

const SEARCH_RADIUS: u32 = 10;

struct NeedleStats {
    width: usize,
    height: usize,
    centered: Vec<f32>,
    denom: f32,
    mean: f32,
}

/// 灰度 NCC。针图更小时只在中心 ±10px 搜，针图统计只算一次。返回 (ncc+1)/2。
pub fn best_ncc_similarity(haystack: &GrayImage, needle: &GrayImage) -> f32 {
    if haystack.width() == 0
        || haystack.height() == 0
        || needle.width() == 0
        || needle.height() == 0
        || needle.width() > haystack.width()
        || needle.height() > haystack.height()
    {
        return 0.0;
    }
    let stats = needle_stats(needle);
    let ncc = if needle.width() == haystack.width() && needle.height() == haystack.height() {
        ncc_at(haystack, &stats, 0, 0)
    } else {
        sliding_ncc(haystack, &stats)
    };
    ((ncc + 1.0) / 2.0).clamp(0.0, 1.0)
}

fn needle_stats(needle: &GrayImage) -> NeedleStats {
    let width = needle.width() as usize;
    let height = needle.height() as usize;
    let raw = needle.as_raw();
    let n = (width * height) as f32;
    let mut sum = 0.0_f32;
    for value in raw {
        sum += f32::from(*value);
    }
    let mean = sum / n;
    let mut centered = Vec::with_capacity(raw.len());
    let mut denom_sq = 0.0_f32;
    for value in raw {
        let delta = f32::from(*value) - mean;
        denom_sq += delta * delta;
        centered.push(delta);
    }
    NeedleStats {
        width,
        height,
        centered,
        denom: denom_sq.sqrt(),
        mean,
    }
}

fn sliding_ncc(haystack: &GrayImage, needle: &NeedleStats) -> f32 {
    let max_x = haystack.width() - needle.width as u32;
    let max_y = haystack.height() - needle.height as u32;
    let center_x = max_x / 2;
    let center_y = max_y / 2;
    let x0 = center_x.saturating_sub(SEARCH_RADIUS);
    let y0 = center_y.saturating_sub(SEARCH_RADIUS);
    let x1 = (center_x + SEARCH_RADIUS).min(max_x);
    let y1 = (center_y + SEARCH_RADIUS).min(max_y);
    search_best(haystack, needle, x0, y0, x1, y1).2
}

fn search_best(
    haystack: &GrayImage,
    needle: &NeedleStats,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
) -> (u32, u32, f32) {
    let mut best_x = x0;
    let mut best_y = y0;
    let mut best = f32::NEG_INFINITY;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let score = ncc_at(haystack, needle, x, y);
            if score > best {
                best = score;
                best_x = x;
                best_y = y;
            }
        }
    }
    if best == f32::NEG_INFINITY {
        best = 0.0;
    }
    (best_x, best_y, best)
}

fn ncc_at(haystack: &GrayImage, needle: &NeedleStats, ox: u32, oy: u32) -> f32 {
    let hay_w = haystack.width() as usize;
    let hay_raw = haystack.as_raw();
    let n = (needle.width * needle.height) as f32;
    let mut sum_hay = 0.0_f32;
    for row in 0..needle.height {
        let hay_off = (oy as usize + row) * hay_w + ox as usize;
        for col in 0..needle.width {
            sum_hay += f32::from(hay_raw[hay_off + col]);
        }
    }
    let mean_hay = sum_hay / n;
    let mut num = 0.0_f32;
    let mut denom_hay = 0.0_f32;
    for row in 0..needle.height {
        let hay_off = (oy as usize + row) * hay_w + ox as usize;
        let needle_off = row * needle.width;
        for col in 0..needle.width {
            let da = f32::from(hay_raw[hay_off + col]) - mean_hay;
            let db = needle.centered[needle_off + col];
            num += da * db;
            denom_hay += da * da;
        }
    }
    if denom_hay < 1e-6 || needle.denom < 1e-6 {
        return if (mean_hay - needle.mean).abs() < 1.0 {
            1.0
        } else {
            0.0
        };
    }
    num / (denom_hay.sqrt() * needle.denom)
}

pub fn difficulty_from_occupied_count(count: usize) -> Result<Difficulty, String> {
    match count {
        5 => Ok(Difficulty {
            label: 'A',
            template_count: 4,
            box_count: 5,
        }),
        7 => Ok(Difficulty {
            label: 'B',
            template_count: 6,
            box_count: 7,
        }),
        9 => Ok(Difficulty {
            label: 'C',
            template_count: 8,
            box_count: 9,
        }),
        _ => Err(format!(
            "有纹格子数是 {count}，只接受 5/7/9（A/B/C）。调占用阈值或检查候选框"
        )),
    }
}

/// 每个候选格投一票给最高分模板；同一模板只留最高分格子。
///
/// `votes[candidate] = (template_index, score)`，模板下标从 0。
pub fn assign_templates(
    votes: &[(Option<usize>, f32)],
    template_count: usize,
    threshold: f32,
) -> Vec<Option<usize>> {
    let mut best_scores = vec![f32::NEG_INFINITY; template_count];
    let mut assigned = vec![None; template_count];

    for (candidate, (template, score)) in votes.iter().enumerate() {
        let Some(template) = *template else {
            continue;
        };
        if *score < threshold || template >= template_count {
            continue;
        }
        if assigned[template].is_none() || *score > best_scores[template] {
            best_scores[template] = *score;
            assigned[template] = Some(candidate);
        }
    }

    assigned
}

/// 最大权分配：先配齐枚数，再比总分。避免贪心把共用格给高分模板后缺枚。
pub fn assign_from_scores(
    scores: &[Vec<f32>],
    candidate_indices: &[usize],
    template_count: usize,
    threshold: f32,
) -> Vec<Option<usize>> {
    let assigned = max_weight_assign(scores, candidate_indices, template_count, threshold);
    if assigned.iter().all(Option::is_some) || threshold <= 0.4 {
        return assigned;
    }
    let relaxed = max_weight_assign(scores, candidate_indices, template_count, 0.4);
    if relaxed.iter().filter(|slot| slot.is_some()).count()
        > assigned.iter().filter(|slot| slot.is_some()).count()
    {
        relaxed
    } else {
        assigned
    }
}

fn max_weight_assign(
    scores: &[Vec<f32>],
    candidate_indices: &[usize],
    template_count: usize,
    threshold: f32,
) -> Vec<Option<usize>> {
    let cell_count = candidate_indices.len();
    let mut best = vec![None; template_count];
    let mut best_count = 0usize;
    let mut best_sum = f32::NEG_INFINITY;
    let mut current = vec![None; template_count];
    let mut used = vec![false; cell_count];
    search_assignment(
        0,
        0,
        0.0,
        scores,
        candidate_indices,
        template_count,
        threshold,
        &mut current,
        &mut used,
        &mut best,
        &mut best_count,
        &mut best_sum,
    );
    best
}

fn search_assignment(
    template: usize,
    assigned_count: usize,
    sum: f32,
    scores: &[Vec<f32>],
    candidate_indices: &[usize],
    template_count: usize,
    threshold: f32,
    current: &mut [Option<usize>],
    used: &mut [bool],
    best: &mut [Option<usize>],
    best_count: &mut usize,
    best_sum: &mut f32,
) {
    if template == template_count {
        if assigned_count > *best_count || (assigned_count == *best_count && sum > *best_sum) {
            *best_count = assigned_count;
            *best_sum = sum;
            best.copy_from_slice(current);
        }
        return;
    }
    search_assignment(
        template + 1,
        assigned_count,
        sum,
        scores,
        candidate_indices,
        template_count,
        threshold,
        current,
        used,
        best,
        best_count,
        best_sum,
    );
    for cell in 0..used.len() {
        if used[cell] {
            continue;
        }
        let Some(score) = scores.get(cell).and_then(|row| row.get(template)).copied() else {
            continue;
        };
        if score < threshold {
            continue;
        }
        used[cell] = true;
        current[template] = Some(candidate_indices[cell]);
        search_assignment(
            template + 1,
            assigned_count + 1,
            sum + score,
            scores,
            candidate_indices,
            template_count,
            threshold,
            current,
            used,
            best,
            best_count,
            best_sum,
        );
        used[cell] = false;
        current[template] = None;
    }
}

pub fn click_order(assignment: &[Option<usize>]) -> Result<Vec<usize>, String> {
    let missing: Vec<String> = assignment
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.is_none())
        .map(|(index, _)| format!("{}", index + 1))
        .collect();
    if !missing.is_empty() {
        return Err(format!("缺少指纹 {}", missing.join("、")));
    }
    Ok(assignment.iter().flatten().copied().collect())
}

pub fn best_index_above(scores: &[f32], threshold: f32) -> Option<usize> {
    scores
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .and_then(|(index, score)| (*score >= threshold).then_some(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{imageops, Luma};

    fn solid(width: u32, height: u32, value: u8) -> GrayImage {
        GrayImage::from_pixel(width, height, Luma([value]))
    }

    fn checker(width: u32, height: u32) -> GrayImage {
        let mut image = GrayImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = Luma([if (x + y) % 2 == 0 { 0 } else { 255 }]);
        }
        image
    }

    #[test]
    fn solid_has_zero_variance_checker_is_occupied() {
        assert_eq!(luminance_variance(&solid(8, 8, 12)), 0.0);
        assert!(is_occupied(&checker(8, 8), 100.0));
        assert!(!is_occupied(&solid(8, 8, 12), 1.0));
    }

    #[test]
    fn difficulty_maps_5_7_9_only() {
        assert_eq!(
            difficulty_from_occupied_count(5).unwrap(),
            Difficulty {
                label: 'A',
                template_count: 4,
                box_count: 5,
            }
        );
        assert_eq!(difficulty_from_occupied_count(7).unwrap().label, 'B');
        assert_eq!(difficulty_from_occupied_count(9).unwrap().template_count, 8);
        assert!(difficulty_from_occupied_count(6).is_err());
        assert!(difficulty_from_occupied_count(0).is_err());
    }

    #[test]
    fn click_order_follows_template_index_not_box_number() {
        // 1 在五、2 在一、3 在六、4 在二 → 点 五、一、六、二
        let votes = vec![
            (Some(1), 0.9),  // 候选1(一) → 模板2
            (Some(3), 0.88), // 候选2(二) → 模板4
            (None, 0.1),
            (None, 0.1),
            (Some(0), 0.91), // 候选5(五) → 模板1
            (Some(2), 0.87), // 候选6(六) → 模板3
            (None, 0.0),
            (None, 0.0),
            (None, 0.0),
        ];
        let assigned = assign_templates(&votes, 4, 0.5);
        assert_eq!(assigned, vec![Some(4), Some(0), Some(5), Some(1)]);
        let order = click_order(&assigned).unwrap();
        assert_eq!(order, vec![4, 0, 5, 1]);
    }

    #[test]
    fn higher_score_wins_same_template() {
        let votes = vec![(Some(0), 0.6), (Some(0), 0.8)];
        let assigned = assign_templates(&votes, 1, 0.5);
        assert_eq!(assigned, vec![Some(1)]);
    }

    #[test]
    fn below_threshold_is_ignored() {
        let votes = vec![(Some(0), 0.4)];
        let assigned = assign_templates(&votes, 1, 0.5);
        assert_eq!(assigned, vec![None]);
        assert!(click_order(&assigned).unwrap_err().contains("缺少指纹 1"));
    }

    #[test]
    fn best_index_picks_highest_above_threshold() {
        assert_eq!(best_index_above(&[0.2, 0.9, 0.8], 0.5), Some(1));
        assert_eq!(best_index_above(&[0.2, 0.3], 0.5), None);
    }

    #[test]
    fn occupancy_uses_variance_gap_not_raw_threshold() {
        // 5 枚真指纹 + 4 格空面板噪点。阈值 80 会把空格也算有纹。
        let variances = [
            2100.0, 2000.0, 90.0, 1900.0, 1800.0, 85.0, 2050.0, 70.0, 60.0,
        ];
        let occupied = select_occupied_indices(&variances).unwrap();
        assert_eq!(occupied, vec![0, 1, 3, 4, 6]);
    }

    #[test]
    fn occupancy_picks_seven_when_gap_is_after_seventh() {
        let variances = [
            2000.0, 1900.0, 1800.0, 1700.0, 1600.0, 1500.0, 1400.0, 80.0, 70.0,
        ];
        let occupied = select_occupied_indices(&variances).unwrap();
        assert_eq!(occupied, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn occupancy_does_not_promote_empty_frames_to_b() {
        // 中心能量：空槽边框被裁掉后只剩低方差。
        let variances = [
            2000.0, 1900.0, 1800.0, 1700.0, 1600.0, 30.0, 25.0, 20.0, 15.0,
        ];
        let occupied = select_occupied_indices(&variances).unwrap();
        assert_eq!(occupied, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn occupancy_keeps_weaker_fingerprints_in_b() {
        let variances = [
            2000.0, 1800.0, 1600.0, 1400.0, 1200.0, 900.0, 700.0, 25.0, 20.0,
        ];
        let occupied = select_occupied_indices(&variances).unwrap();
        assert_eq!(occupied, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn occupancy_energy_ignores_slot_frame() {
        let fingerprint = checker(80, 80);
        let mut frame = solid(80, 80, 40);
        for x in 0..80 {
            for inset in 0..4 {
                frame.put_pixel(x, inset, Luma([220]));
                frame.put_pixel(x, 79 - inset, Luma([220]));
                frame.put_pixel(inset, x, Luma([220]));
                frame.put_pixel(79 - inset, x, Luma([220]));
            }
        }
        assert!(occupancy_energy(&fingerprint) > occupancy_energy(&frame) * 5.0);
    }

    #[test]
    fn ncc_recovers_shifted_pattern() {
        let pattern = checker(64, 64);
        let mut haystack = GrayImage::from_pixel(80, 80, Luma([40]));
        imageops::replace(&mut haystack, &pattern, 8, 6);
        let score = best_ncc_similarity(&haystack, &pattern);
        assert!(score > 0.9, "score {score}");
    }

    #[test]
    fn ocr_text_picks_longest_matching_name() {
        let names = ["无名", "格赫罗斯", "克莱尔"];
        assert_eq!(match_person_by_ocr_text("无名", &names), Some(0));
        assert_eq!(
            match_person_by_ocr_text("格赫罗斯 N.145822", &names),
            Some(1)
        );
        assert_eq!(match_person_by_ocr_text(" 克 莱 尔 ", &names), Some(2));
        assert_eq!(match_person_by_ocr_text("路人甲", &names), None);
    }

    #[test]
    fn occupancy_picks_nine_when_all_are_strong() {
        let variances = [
            2000.0, 1900.0, 1850.0, 1800.0, 1750.0, 1700.0, 1650.0, 1600.0, 1550.0,
        ];
        let occupied = select_occupied_indices(&variances).unwrap();
        assert_eq!(occupied, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn second_best_template_fills_when_best_is_taken() {
        let scores = vec![
            vec![0.9, 0.6, 0.4, 0.3],
            vec![0.85, 0.8, 0.4, 0.3],
            vec![0.4, 0.3, 0.9, 0.2],
            vec![0.4, 0.3, 0.2, 0.88],
        ];
        let assigned = assign_from_scores(&scores, &[0, 1, 3, 4], 4, 0.5);
        assert_eq!(assigned, vec![Some(0), Some(1), Some(3), Some(4)]);
    }

    #[test]
    fn contested_cell_keeps_complete_assignment() {
        // 格7 对模板3略高、对模板2也够格；贪心把格7给3 → 缺2。
        // 全局应把格7给2，格1给3，四枚都配上。
        let scores = vec![
            vec![0.40, 0.35, 0.72, 0.30],
            vec![0.40, 0.35, 0.30, 0.88],
            vec![0.91, 0.40, 0.35, 0.30],
            vec![0.42, 0.41, 0.39, 0.38],
            vec![0.45, 0.80, 0.85, 0.40],
        ];
        let assigned = assign_from_scores(&scores, &[0, 2, 3, 4, 6], 4, 0.55);
        assert_eq!(assigned, vec![Some(3), Some(6), Some(0), Some(2)]);
    }
}
