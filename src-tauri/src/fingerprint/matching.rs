//! 档位、占用判定、模板贪心分配。纯函数，不碰截屏。

use image::GrayImage;

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
    use image::Luma;

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
}
