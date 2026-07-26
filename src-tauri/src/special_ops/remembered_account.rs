use super::windows_ocr::{OcrBounds, OcrWord};
use std::collections::BTreeSet;
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;

const MIN_OVERLAP_RATIO: f32 = 0.5;

pub(crate) fn stable_target_bounds(
    first: &[OcrWord],
    second: &[OcrWord],
    target_qq: &str,
) -> Option<OcrBounds> {
    let first = first.iter().find(|word| word.text == target_qq)?;
    let second = second.iter().find(|word| word.text == target_qq)?;
    (overlap_ratio(first.bounds, second.bounds) >= MIN_OVERLAP_RATIO).then_some(second.bounds)
}

pub(crate) fn stable_account_screen(
    first: &[OcrWord],
    second: &[OcrWord],
) -> Option<BTreeSet<String>> {
    let first = first
        .iter()
        .map(|word| word.text.clone())
        .collect::<BTreeSet<_>>();
    let second = second
        .iter()
        .map(|word| word.text.clone())
        .collect::<BTreeSet<_>>();
    (!first.is_empty() && first == second).then_some(second)
}

fn overlap_ratio(first: OcrBounds, second: OcrBounds) -> f32 {
    let left = first.x.max(second.x);
    let top = first.y.max(second.y);
    let right = (first.x + first.width).min(second.x + second.width);
    let bottom = (first.y + first.height).min(second.y + second.height);
    let intersection = (right - left).max(0.0) * (bottom - top).max(0.0);
    let smaller_area = (first.width * first.height).min(second.width * second.height);
    if smaller_area <= 0.0 {
        0.0
    } else {
        intersection / smaller_area
    }
}

#[derive(Debug, Default)]
pub(crate) struct ScanProgress {
    seen: BTreeSet<String>,
    last_screen: Option<BTreeSet<String>>,
    repeated_unchanged_screens: u8,
}

impl ScanProgress {
    pub(crate) fn note_screen(&mut self, screen: BTreeSet<String>) -> bool {
        let has_new_account = screen.iter().any(|qq| !self.seen.contains(qq));
        self.seen.extend(screen.iter().cloned());
        if !has_new_account && self.last_screen.as_ref() == Some(&screen) {
            self.repeated_unchanged_screens = self.repeated_unchanged_screens.saturating_add(1);
        } else {
            self.repeated_unchanged_screens = 0;
        }
        self.last_screen = Some(screen);
        self.repeated_unchanged_screens >= 2
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AccountSelectionError {
    NotFound,
    Mismatch { actual: String },
    Driver(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountSelectionPhase {
    Select,
    Verify,
}

#[allow(async_fn_in_trait)]
pub(crate) trait RememberedAccountDriver {
    async fn sample_accounts(&self, cancelled: Arc<AtomicBool>) -> Result<Vec<OcrWord>, String>;
    async fn scroll_down(&self, cancelled: Arc<AtomicBool>) -> Result<(), String>;
    async fn select_bounds(
        &self,
        bounds: OcrBounds,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), String>;
    async fn copy_selected_qq(&self, cancelled: Arc<AtomicBool>) -> Result<String, String>;
}

pub(crate) async fn select_remembered_account(
    driver: &(impl RememberedAccountDriver + ?Sized),
    target_qq: &str,
    cancelled: Arc<AtomicBool>,
    mut on_phase: impl FnMut(AccountSelectionPhase),
) -> Result<(), AccountSelectionError> {
    let mut progress = ScanProgress::default();
    loop {
        let first = driver
            .sample_accounts(Arc::clone(&cancelled))
            .await
            .map_err(AccountSelectionError::Driver)?;
        tokio::time::sleep(Duration::from_millis(400)).await;
        let second = driver
            .sample_accounts(Arc::clone(&cancelled))
            .await
            .map_err(AccountSelectionError::Driver)?;

        if let Some(bounds) = stable_target_bounds(&first, &second, target_qq) {
            on_phase(AccountSelectionPhase::Select);
            driver
                .select_bounds(bounds, Arc::clone(&cancelled))
                .await
                .map_err(AccountSelectionError::Driver)?;
            on_phase(AccountSelectionPhase::Verify);
            let actual = driver
                .copy_selected_qq(Arc::clone(&cancelled))
                .await
                .map_err(AccountSelectionError::Driver)?;
            return if actual == target_qq {
                Ok(())
            } else {
                Err(AccountSelectionError::Mismatch { actual })
            };
        }

        let Some(stable_screen) = stable_account_screen(&first, &second) else {
            continue;
        };
        if progress.note_screen(stable_screen) {
            return Err(AccountSelectionError::NotFound);
        }
        driver
            .scroll_down(Arc::clone(&cancelled))
            .await
            .map_err(AccountSelectionError::Driver)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    fn word(text: &str, x: f32, y: f32) -> OcrWord {
        OcrWord::new(text, OcrBounds::new(x, y, 80.0, 18.0))
    }

    #[test]
    fn stable_target_requires_both_samples_and_overlapping_bounds() {
        let first = vec![word("123456", 10.0, 20.0)];
        let second = vec![word("123456", 12.0, 21.0)];
        assert_eq!(
            stable_target_bounds(&first, &second, "123456"),
            Some(second[0].bounds)
        );

        let moved = vec![word("123456", 200.0, 100.0)];
        assert_eq!(stable_target_bounds(&first, &moved, "123456"), None);
        assert_eq!(stable_target_bounds(&[], &second, "123456"), None);
    }

    #[test]
    fn two_identical_screens_without_new_accounts_reach_bottom() {
        let mut progress = ScanProgress::default();
        assert!(!progress.note_screen(BTreeSet::from(["1".to_string(), "2".to_string()])));
        assert!(!progress.note_screen(BTreeSet::from(["2".to_string(), "3".to_string()])));
        assert!(!progress.note_screen(BTreeSet::from(["2".to_string(), "3".to_string()])));
        assert!(progress.note_screen(BTreeSet::from(["2".to_string(), "3".to_string()])));
    }

    #[test]
    fn changing_known_screen_does_not_count_as_same_bottom_screen() {
        let mut progress = ScanProgress::default();
        progress.note_screen(BTreeSet::from(["1".to_string(), "2".to_string()]));
        progress.note_screen(BTreeSet::from(["2".to_string(), "3".to_string()]));
        assert!(!progress.note_screen(BTreeSet::from(["1".to_string(), "3".to_string()])));
        assert!(!progress.note_screen(BTreeSet::from(["2".to_string(), "3".to_string()])));
    }

    #[test]
    fn stable_account_screen_requires_equal_non_empty_samples() {
        let first = vec![word("111", 10.0, 20.0), word("222", 10.0, 40.0)];
        let same_accounts = vec![word("222", 12.0, 42.0), word("111", 12.0, 22.0)];
        let changed = vec![word("111", 10.0, 20.0), word("333", 10.0, 40.0)];

        assert_eq!(
            stable_account_screen(&first, &same_accounts),
            Some(BTreeSet::from(["111".to_string(), "222".to_string()]))
        );
        assert_eq!(stable_account_screen(&first, &changed), None);
        assert_eq!(stable_account_screen(&[], &[]), None);
    }

    #[derive(Default)]
    struct FakeDriver {
        samples: Mutex<VecDeque<Result<Vec<OcrWord>, String>>>,
        copied: Mutex<VecDeque<Result<String, String>>>,
        actions: Mutex<Vec<String>>,
    }

    impl RememberedAccountDriver for FakeDriver {
        async fn sample_accounts(&self, _: Arc<AtomicBool>) -> Result<Vec<OcrWord>, String> {
            self.samples.lock().unwrap().pop_front().unwrap()
        }

        async fn scroll_down(&self, _: Arc<AtomicBool>) -> Result<(), String> {
            self.actions.lock().unwrap().push("scroll".to_string());
            Ok(())
        }

        async fn select_bounds(&self, bounds: OcrBounds, _: Arc<AtomicBool>) -> Result<(), String> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("select:{:.0}", bounds.y));
            Ok(())
        }

        async fn copy_selected_qq(&self, _: Arc<AtomicBool>) -> Result<String, String> {
            self.actions.lock().unwrap().push("copy".to_string());
            self.copied.lock().unwrap().pop_front().unwrap()
        }
    }

    #[tokio::test(start_paused = true)]
    async fn selection_uses_second_stable_bounds_then_verifies_exact_qq() {
        let driver = FakeDriver::default();
        driver.samples.lock().unwrap().extend([
            Ok(vec![word("123456", 10.0, 20.0)]),
            Ok(vec![word("123456", 11.0, 21.0)]),
        ]);
        driver
            .copied
            .lock()
            .unwrap()
            .push_back(Ok("123456".to_string()));

        select_remembered_account(&driver, "123456", Arc::new(AtomicBool::new(false)), |_| {})
            .await
            .unwrap();

        assert_eq!(
            driver.actions.lock().unwrap().as_slice(),
            ["select:21", "copy"]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn missing_target_scrolls_until_two_unchanged_screens_then_stops() {
        let driver = FakeDriver::default();
        for _ in 0..4 {
            driver.samples.lock().unwrap().extend([
                Ok(vec![word("111", 10.0, 20.0)]),
                Ok(vec![word("111", 10.0, 20.0)]),
            ]);
        }

        assert_eq!(
            select_remembered_account(&driver, "999", Arc::new(AtomicBool::new(false)), |_| {},)
                .await,
            Err(AccountSelectionError::NotFound)
        );
        assert_eq!(
            driver.actions.lock().unwrap().as_slice(),
            ["scroll", "scroll"]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn copied_different_qq_never_reports_success() {
        let driver = FakeDriver::default();
        driver.samples.lock().unwrap().extend([
            Ok(vec![word("123456", 10.0, 20.0)]),
            Ok(vec![word("123456", 10.0, 20.0)]),
        ]);
        driver
            .copied
            .lock()
            .unwrap()
            .push_back(Ok("654321".to_string()));

        assert_eq!(
            select_remembered_account(&driver, "123456", Arc::new(AtomicBool::new(false)), |_| {},)
                .await,
            Err(AccountSelectionError::Mismatch {
                actual: "654321".to_string()
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn inconsistent_samples_resample_without_scrolling_or_clicking() {
        let driver = FakeDriver::default();
        driver.samples.lock().unwrap().extend([
            Ok(vec![word("123456", 10.0, 20.0)]),
            Ok(vec![word("999999", 10.0, 20.0)]),
            Ok(vec![word("123456", 10.0, 20.0)]),
            Ok(vec![word("123456", 11.0, 21.0)]),
        ]);
        driver
            .copied
            .lock()
            .unwrap()
            .push_back(Ok("123456".to_string()));

        select_remembered_account(&driver, "123456", Arc::new(AtomicBool::new(false)), |_| {})
            .await
            .unwrap();

        assert!(!driver
            .actions
            .lock()
            .unwrap()
            .contains(&"scroll".to_string()));
    }
}
