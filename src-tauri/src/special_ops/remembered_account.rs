use super::windows_ocr::OcrWord;
use std::collections::BTreeSet;
use std::sync::{atomic::AtomicBool, Arc};

const VISIBLE_ACCOUNT_ROWS: u8 = 3;
const OCR_ROW_STABILITY_TOLERANCE_PX: f32 = 8.0;
pub(crate) const ACCOUNT_LIST_UNAVAILABLE: &str = "已记住账号列表未确认";

pub(crate) fn account_list_visible_in_both_samples(first: &[OcrWord], second: &[OcrWord]) -> bool {
    !first.is_empty() && !second.is_empty()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountRowSlot {
    Ocr { index: u8, center_y: i32 },
    Fallback { index: u8 },
}

impl AccountRowSlot {
    pub(crate) const fn index(self) -> u8 {
        match self {
            Self::Ocr { index, .. } | Self::Fallback { index } => index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AccountRowClick {
    pub index: u8,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountScanAttempt {
    pub page: u32,
    pub slot: u8,
    pub click_x: i32,
    pub click_y: i32,
    pub copied_qq: String,
}

pub(crate) fn derive_visible_row_slots(
    first: &[OcrWord],
    second: &[OcrWord],
    list_height: i32,
) -> Vec<AccountRowSlot> {
    let Some(first_centers) = row_centers(first, list_height) else {
        return fallback_slots();
    };
    let Some(second_centers) = row_centers(second, list_height) else {
        return fallback_slots();
    };
    if first_centers.len() == usize::from(VISIBLE_ACCOUNT_ROWS)
        && second_centers.len() == usize::from(VISIBLE_ACCOUNT_ROWS)
        && first_centers
            .iter()
            .zip(&second_centers)
            .all(|(left, right)| (left - right).abs() <= OCR_ROW_STABILITY_TOLERANCE_PX)
    {
        return first_centers
            .into_iter()
            .zip(second_centers)
            .enumerate()
            .map(|(index, (left, right))| AccountRowSlot::Ocr {
                index: index as u8,
                center_y: ((left + right) / 2.0).round() as i32,
            })
            .collect();
    }
    fallback_slots()
}

fn row_centers(words: &[OcrWord], list_height: i32) -> Option<Vec<f32>> {
    if list_height <= 0 {
        return None;
    }
    let list_height = list_height as f32;
    let mut centers = Vec::with_capacity(words.len());
    for word in words {
        let bounds = word.bounds;
        let center = bounds.y + bounds.height / 2.0;
        if !(bounds.x.is_finite()
            && bounds.y.is_finite()
            && bounds.width.is_finite()
            && bounds.height.is_finite()
            && bounds.width > 0.0
            && bounds.height > 0.0
            && center.is_finite()
            && center >= 0.0
            && center <= list_height)
        {
            return None;
        }
        centers.push(center);
    }
    centers.sort_by(|left, right| left.total_cmp(right));

    let mut rows = Vec::<(f32, u32)>::new();
    for center in centers {
        if let Some((sum, count)) = rows.last_mut() {
            if (center - *sum / *count as f32).abs() <= OCR_ROW_STABILITY_TOLERANCE_PX {
                *sum += center;
                *count += 1;
                continue;
            }
        }
        rows.push((center, 1));
    }
    Some(
        rows.into_iter()
            .map(|(sum, count)| sum / count as f32)
            .collect(),
    )
}

fn fallback_slots() -> Vec<AccountRowSlot> {
    (0..VISIBLE_ACCOUNT_ROWS)
        .map(|index| AccountRowSlot::Fallback { index })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AccountSelectionError {
    NotFound { attempts: Vec<AccountScanAttempt> },
    ListUnavailable,
    Driver(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountSelectionPhase {
    Select,
    Verify,
}

#[allow(async_fn_in_trait)]
pub(crate) trait RememberedAccountDriver {
    async fn visible_account_rows(
        &self,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<AccountRowSlot>, String>;
    async fn open_account_list(&self, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn scroll_down(&self, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn select_row(
        &self,
        slot: AccountRowSlot,
        cancelled: Arc<AtomicBool>,
    ) -> Result<AccountRowClick, String>;
    async fn copy_selected_qq(&self, cancelled: Arc<AtomicBool>) -> Result<String, String>;
}

pub(crate) async fn select_remembered_account(
    driver: &(impl RememberedAccountDriver + ?Sized),
    target_qq: &str,
    cancelled: Arc<AtomicBool>,
    mut on_phase: impl FnMut(AccountSelectionPhase),
) -> Result<(), AccountSelectionError> {
    let mut seen = BTreeSet::new();
    let mut page = 1_u32;
    let mut all_attempts = Vec::new();
    loop {
        let rows = driver
            .visible_account_rows(Arc::clone(&cancelled))
            .await
            .map_err(selection_driver_error)?;
        let mut page_has_new_account = false;
        for row in rows {
            on_phase(AccountSelectionPhase::Select);
            let click = driver
                .select_row(row, Arc::clone(&cancelled))
                .await
                .map_err(selection_driver_error)?;
            on_phase(AccountSelectionPhase::Verify);
            let actual = driver
                .copy_selected_qq(Arc::clone(&cancelled))
                .await
                .map_err(selection_driver_error)?;
            all_attempts.push(AccountScanAttempt {
                page,
                slot: row.index(),
                click_x: click.x,
                click_y: click.y,
                copied_qq: redact_qq(&actual),
            });
            if actual == target_qq {
                return Ok(());
            }
            page_has_new_account |= seen.insert(actual);
            driver
                .open_account_list(Arc::clone(&cancelled))
                .await
                .map_err(selection_driver_error)?;
        }
        if !page_has_new_account {
            return Err(AccountSelectionError::NotFound {
                attempts: all_attempts,
            });
        }
        driver
            .scroll_down(Arc::clone(&cancelled))
            .await
            .map_err(selection_driver_error)?;
        page = page.saturating_add(1);
    }
}

pub(crate) fn redact_qq(value: &str) -> String {
    if value.chars().all(|ch| ch.is_ascii_digit()) && !value.is_empty() {
        let suffix = value
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        format!("***{suffix}")
    } else {
        "未复制到 QQ".to_string()
    }
}

pub(crate) fn format_scan_attempts(attempts: &[AccountScanAttempt]) -> String {
    if attempts.is_empty() {
        return "未找到目标 QQ；未完成任何账号行扫描".to_string();
    }
    let trace = attempts
        .iter()
        .map(|attempt| {
            format!(
                "页 {} 槽位 {} ({},{}) -> {}",
                attempt.page, attempt.slot, attempt.click_x, attempt.click_y, attempt.copied_qq
            )
        })
        .collect::<Vec<_>>()
        .join("；");
    format!("未找到目标 QQ；扫描轨迹：{trace}")
}

fn selection_driver_error(error: String) -> AccountSelectionError {
    if error == ACCOUNT_LIST_UNAVAILABLE {
        AccountSelectionError::ListUnavailable
    } else {
        AccountSelectionError::Driver(error)
    }
}

#[cfg(test)]
mod tests {
    use super::super::windows_ocr::OcrBounds;
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    fn word(text: &str, y: f32) -> OcrWord {
        OcrWord::new(text, OcrBounds::new(10.0, y, 80.0, 18.0))
    }

    #[test]
    fn account_list_visibility_requires_two_non_empty_samples() {
        let first = vec![word("111", 20.0), word("222", 40.0)];
        let same_accounts = vec![word("222", 42.0), word("111", 22.0)];
        let changed = vec![word("111", 20.0), word("333", 40.0)];

        assert!(account_list_visible_in_both_samples(&first, &same_accounts));
        assert!(account_list_visible_in_both_samples(&first, &changed));
        assert!(!account_list_visible_in_both_samples(&[], &[]));
    }

    #[test]
    fn different_ocr_fragments_still_confirm_account_list_is_visible() {
        let first = vec![word("3079", 40.0)];
        let second = vec![word("389", 40.0)];

        assert!(account_list_visible_in_both_samples(&first, &second));
    }

    #[test]
    fn stable_ocr_rows_use_detected_centers() {
        let first = vec![word("111", 9.0), word("222", 45.0), word("333", 81.0)];
        let second = vec![word("444", 10.0), word("555", 46.0), word("666", 82.0)];

        assert_eq!(
            derive_visible_row_slots(&first, &second, 108),
            vec![
                AccountRowSlot::Ocr {
                    index: 0,
                    center_y: 19,
                },
                AccountRowSlot::Ocr {
                    index: 1,
                    center_y: 55,
                },
                AccountRowSlot::Ocr {
                    index: 2,
                    center_y: 91,
                },
            ]
        );
    }

    #[test]
    fn unstable_ocr_rows_fall_back_to_three_slots() {
        let first = vec![word("111", 9.0), word("222", 45.0)];
        let second = vec![word("333", 9.0), word("444", 75.0)];

        assert_eq!(
            derive_visible_row_slots(&first, &second, 108),
            vec![
                AccountRowSlot::Fallback { index: 0 },
                AccountRowSlot::Fallback { index: 1 },
                AccountRowSlot::Fallback { index: 2 },
            ]
        );
    }

    #[test]
    fn ocr_centers_outside_list_height_fall_back_to_three_slots() {
        let first = vec![word("111", 9.0), word("222", 145.0)];
        let second = vec![word("333", 10.0), word("444", 146.0)];

        assert_eq!(
            derive_visible_row_slots(&first, &second, 108),
            fallback_slots()
        );
    }

    #[derive(Default)]
    struct FakeDriver {
        copied: Mutex<VecDeque<Result<String, String>>>,
        samples: Mutex<VecDeque<Result<Vec<OcrWord>, String>>>,
        actions: Mutex<Vec<String>>,
    }

    impl RememberedAccountDriver for FakeDriver {
        async fn visible_account_rows(
            &self,
            _: Arc<AtomicBool>,
        ) -> Result<Vec<AccountRowSlot>, String> {
            let first = self
                .samples
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| {
                    Ok(vec![
                        word("visible-1", 20.0),
                        word("visible-2", 50.0),
                        word("visible-3", 80.0),
                    ])
                })?;
            let second = self
                .samples
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| {
                    Ok(vec![
                        word("visible-1", 20.0),
                        word("visible-2", 50.0),
                        word("visible-3", 80.0),
                    ])
                })?;
            if !account_list_visible_in_both_samples(&first, &second) {
                return Err(ACCOUNT_LIST_UNAVAILABLE.to_string());
            }
            Ok(derive_visible_row_slots(&first, &second, 108))
        }

        async fn open_account_list(&self, _: Arc<AtomicBool>) -> Result<(), String> {
            self.actions.lock().unwrap().push("open".to_string());
            Ok(())
        }

        async fn scroll_down(&self, _: Arc<AtomicBool>) -> Result<(), String> {
            self.actions.lock().unwrap().push("scroll".to_string());
            Ok(())
        }

        async fn select_row(
            &self,
            slot: AccountRowSlot,
            _: Arc<AtomicBool>,
        ) -> Result<AccountRowClick, String> {
            let index = slot.index();
            self.actions.lock().unwrap().push(format!("select:{index}"));
            Ok(AccountRowClick {
                index,
                x: 1719,
                y: 754 + i32::from(index),
            })
        }

        async fn copy_selected_qq(&self, _: Arc<AtomicBool>) -> Result<String, String> {
            self.actions.lock().unwrap().push("copy".to_string());
            self.copied.lock().unwrap().pop_front().unwrap()
        }
    }

    #[tokio::test(start_paused = true)]
    async fn target_missed_by_ocr_is_found_by_copying_each_visible_row() {
        let driver = FakeDriver::default();
        driver
            .copied
            .lock()
            .unwrap()
            .extend([Ok("3674318142".to_string()), Ok("3079643589".to_string())]);

        select_remembered_account(
            &driver,
            "3079643589",
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(
            driver.actions.lock().unwrap().as_slice(),
            ["select:0", "copy", "open", "select:1", "copy"]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_page_after_scroll_reports_not_found() {
        let driver = FakeDriver::default();
        for _ in 0..2 {
            driver.copied.lock().unwrap().extend([
                Ok("111".to_string()),
                Ok("222".to_string()),
                Ok("333".to_string()),
            ]);
        }

        assert!(matches!(
            select_remembered_account(&driver, "999", Arc::new(AtomicBool::new(false)), |_| {},)
                .await,
            Err(AccountSelectionError::NotFound { .. })
        ));
        assert_eq!(
            driver
                .actions
                .lock()
                .unwrap()
                .iter()
                .filter(|action| action.as_str() == "scroll")
                .count(),
            1
        );
    }

    #[tokio::test(start_paused = true)]
    async fn partial_two_row_ocr_falls_back_to_all_three_visible_rows() {
        let driver = FakeDriver::default();
        let two_rows = vec![word("111", 20.0), word("222", 60.0)];
        driver.samples.lock().unwrap().extend([
            Ok(two_rows.clone()),
            Ok(two_rows.clone()),
            Ok(two_rows.clone()),
            Ok(two_rows),
        ]);
        driver.copied.lock().unwrap().extend([
            Ok("111".to_string()),
            Ok("222".to_string()),
            Ok("111".to_string()),
            Ok("222".to_string()),
            Ok("111".to_string()),
            Ok("222".to_string()),
        ]);

        assert!(matches!(
            select_remembered_account(&driver, "999", Arc::new(AtomicBool::new(false)), |_| {})
                .await,
            Err(AccountSelectionError::NotFound { attempts: _ })
        ));
        assert!(
            driver
                .actions
                .lock()
                .unwrap()
                .iter()
                .any(|action| action == "select:2"),
            "OCR 少于三行时必须回退到三行固定槽位，避免漏掉物理首行"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn not_found_error_contains_redacted_scan_trace() {
        let driver = FakeDriver::default();
        let three_rows = vec![word("111", 20.0), word("222", 50.0), word("333", 80.0)];
        driver.samples.lock().unwrap().extend([
            Ok(three_rows.clone()),
            Ok(three_rows.clone()),
            Ok(three_rows.clone()),
            Ok(three_rows),
        ]);
        driver.copied.lock().unwrap().extend([
            Ok("11112222".to_string()),
            Ok("33334444".to_string()),
            Ok("55556666".to_string()),
            Ok("11112222".to_string()),
            Ok("33334444".to_string()),
            Ok("55556666".to_string()),
        ]);

        let error = select_remembered_account(
            &driver,
            "99999999",
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await
        .unwrap_err();
        let diagnostic = format!("{error:?}");

        assert!(diagnostic.contains("***2222"));
        assert!(diagnostic.contains("click_x: 1719"));
        assert!(diagnostic.contains("click_y: 755"));
        assert!(!diagnostic.contains("11112222"));
        assert!(!diagnostic.contains("33334444"));
    }
}
