use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tokio::sync::oneshot;

use super::{
    types::{
        FingerprintSettings, RegionSelectionKind, RegionSelectionOutcome, RegionSelectionProgress,
    },
    FingerprintState,
};
use crate::morse::types::{ClickRegion, RegionRect};

const OVERLAY_LABEL: &str = "fingerprint-overlay";

#[derive(Debug)]
pub struct PendingSelection {
    pub target: String,
    pub slots: Vec<usize>,
    pub current_index: usize,
    pub staged: Vec<Option<RegionRect>>,
    pub sender: oneshot::Sender<RegionSelectionKind>,
}

#[derive(Debug)]
pub struct PreparedSelection {
    pub expected_slot: usize,
    pub is_complete: bool,
    pub progress: RegionSelectionProgress,
}

impl PendingSelection {
    fn current_slot(&self) -> Option<usize> {
        self.slots.get(self.current_index).copied()
    }

    fn completed_slots(&self, next_index: usize) -> Vec<usize> {
        self.slots.iter().take(next_index).copied().collect()
    }
}

pub(crate) fn target_slot_count(target: &str) -> Result<usize, String> {
    match target {
        "name" => Ok(1),
        "candidates" => Ok(9),
        "archive" => Ok(8),
        "click" => Ok(7),
        _ => Err(format!("未知框选目标: {target}")),
    }
}

fn click_regions_to_staged(click_regions: &[ClickRegion]) -> Vec<Option<RegionRect>> {
    let mut staged = vec![None; 7];
    for (index, region) in click_regions.iter().enumerate().take(7) {
        staged[index] = Some(region.rect.clone());
    }
    staged
}

fn staged_from_settings(settings: &FingerprintSettings, target: &str) -> Vec<Option<RegionRect>> {
    match target {
        "name" => vec![settings.name_region.clone()],
        "candidates" => settings.candidate_boxes.to_vec(),
        "archive" => settings.archive_slots.to_vec(),
        "click" => click_regions_to_staged(&settings.click_regions),
        _ => Vec::new(),
    }
}

fn apply_staged(settings: &mut FingerprintSettings, target: &str, staged: &[Option<RegionRect>]) {
    match target {
        "name" => {
            settings.name_region = staged.first().cloned().flatten();
        }
        "candidates" => {
            for (index, rect) in staged.iter().take(9).enumerate() {
                settings.candidate_boxes[index] = rect.clone();
            }
        }
        "archive" => {
            for (index, rect) in staged.iter().take(8).enumerate() {
                settings.archive_slots[index] = rect.clone();
            }
        }
        "click" => {
            let delays: Vec<u64> = settings
                .click_regions
                .iter()
                .map(|region| region.delay_ms)
                .collect();
            settings.click_regions = staged
                .iter()
                .enumerate()
                .filter_map(|(index, rect)| {
                    Some(ClickRegion {
                        rect: rect.clone()?,
                        delay_ms: delays.get(index).copied().unwrap_or(500),
                    })
                })
                .collect();
        }
        _ => {}
    }
}

pub(crate) fn cancel_active_overlay(app: &AppHandle) {
    resolve_pending(app, RegionSelectionKind::Cancelled);
    destroy_overlay_window(app);
}

fn resolve_pending(app: &AppHandle, kind: RegionSelectionKind) {
    let state = app.state::<FingerprintState>();
    if let Ok(mut inner) = state.inner.lock() {
        let Some(pending) = inner.logic.pending_selection.take() else {
            return;
        };
        let _ = pending.sender.send(kind);
    } else {
        crate::log_error!(
            "fingerprint::overlay",
            "区域选择状态已损坏，无法回收待处理选择流程"
        );
    };
}

fn destroy_overlay_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.destroy();
    }
}

fn parse_slots(slots: &[usize], max_slots: usize) -> Result<Vec<usize>, String> {
    if slots.is_empty() {
        return Err("至少需要选择一个区域槽位".to_string());
    }

    let mut seen = vec![false; max_slots];
    let mut parsed = Vec::with_capacity(slots.len());
    for &slot in slots {
        if slot >= max_slots {
            return Err(format!("无效区域槽位: {slot}（最大 {}）", max_slots - 1));
        }
        if seen[slot] {
            return Err(format!("区域槽位 {slot} 重复"));
        }
        seen[slot] = true;
        parsed.push(slot);
    }
    Ok(parsed)
}

fn prepare_selection_from_pending(
    pending: &PendingSelection,
    slot: usize,
    rect: RegionRect,
) -> Result<PreparedSelection, String> {
    if slot >= pending.staged.len() {
        return Err(format!("无效区域槽位: {slot}"));
    }
    if rect.width <= 10 || rect.height <= 5 {
        return Err("所选区域过小，无法保存".to_string());
    }

    let expected_slot = pending
        .current_slot()
        .ok_or_else(|| "当前区域选择流程没有可用槽位".to_string())?;
    if expected_slot != slot {
        return Err(format!(
            "区域选择槽位不匹配: 期望 {}, 实际 {}",
            expected_slot, slot
        ));
    }

    let mut staged = pending.staged.clone();
    staged[slot] = Some(rect);

    let next_index = pending.current_index + 1;
    let is_complete = next_index >= pending.slots.len();
    let current_slot = if is_complete {
        None
    } else {
        pending.slots.get(next_index).copied()
    };

    Ok(PreparedSelection {
        expected_slot,
        is_complete,
        progress: RegionSelectionProgress {
            current_slot,
            completed_slots: pending.completed_slots(next_index),
            target: pending.target.clone(),
            rects: staged,
        },
    })
}

pub async fn begin_region_selection(
    app: &AppHandle,
    slots: Vec<usize>,
    target: String,
    state: State<'_, FingerprintState>,
) -> Result<RegionSelectionOutcome, String> {
    let max_slots = target_slot_count(&target)?;
    let slots = parse_slots(&slots, max_slots)?;
    let (sender, receiver) = oneshot::channel();

    {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "区域选择状态已损坏".to_string())?;
        if inner.logic.pending_selection.is_some() {
            return Err("当前已有一个区域选择流程在进行中".to_string());
        }
        if inner.logic.run_in_progress {
            return Err("当前识别任务正在运行，请稍后再试".to_string());
        }
        inner.logic.pending_selection = Some(PendingSelection {
            target: target.clone(),
            slots: slots.clone(),
            current_index: 0,
            staged: staged_from_settings(&inner.settings, &target),
            sender,
        });
    }

    destroy_overlay_window(app);

    let overlay_url = WebviewUrl::App(
        format!(
            "index.html?mode=fingerprint-overlay&target={}&slots={}",
            crate::overlay_utils::encoded_query_value(&target),
            slots
                .iter()
                .map(|slot| slot.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
        .into(),
    );
    let builder = WebviewWindowBuilder::new(app, OVERLAY_LABEL, overlay_url)
        .title("选择指纹区域")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .visible(true)
        .resizable(false)
        .fullscreen(true);

    let window = match builder.build() {
        Ok(window) => window,
        Err(error) => {
            resolve_pending(app, RegionSelectionKind::Closed);
            return Err(format!("创建区域选择窗口失败: {error}"));
        }
    };

    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Destroyed | WindowEvent::CloseRequested { .. }
        ) {
            resolve_pending(&close_app, RegionSelectionKind::Closed);
        }
    });

    let kind = match receiver.await {
        Ok(kind) => kind,
        Err(_) => {
            resolve_pending(app, RegionSelectionKind::Closed);
            RegionSelectionKind::Closed
        }
    };
    destroy_overlay_window(app);

    let (target, rects) = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "区域选择状态已损坏".to_string())?;
        if let Some(pending) = inner.logic.pending_selection.as_ref() {
            (pending.target.clone(), pending.staged.clone())
        } else {
            (
                target.clone(),
                staged_from_settings(&inner.settings, &target),
            )
        }
    };

    Ok(RegionSelectionOutcome {
        kind,
        target,
        rects,
    })
}

pub fn prepare_selection(
    slot: usize,
    rect: RegionRect,
    state: &State<'_, FingerprintState>,
) -> Result<PreparedSelection, String> {
    let inner = state
        .inner
        .lock()
        .map_err(|_| "区域选择状态已损坏".to_string())?;
    let pending = inner
        .logic
        .pending_selection
        .as_ref()
        .ok_or_else(|| "当前没有等待中的区域选择流程".to_string())?;
    prepare_selection_from_pending(pending, slot, rect)
}

pub fn commit_selection(
    app: &AppHandle,
    prepared: PreparedSelection,
    state: &State<'_, FingerprintState>,
) -> Result<(), String> {
    let mut sender = None;
    {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "区域选择状态已损坏".to_string())?;
        let pending = inner
            .logic
            .pending_selection
            .as_ref()
            .ok_or_else(|| "当前没有等待中的区域选择流程".to_string())?;
        let expected_slot = pending
            .current_slot()
            .ok_or_else(|| "当前区域选择流程没有可用槽位".to_string())?;
        if expected_slot != prepared.expected_slot {
            return Err(format!(
                "区域选择槽位不匹配: 期望 {}, 实际 {}",
                expected_slot, prepared.expected_slot
            ));
        }

        if prepared.is_complete {
            let pending = inner
                .logic
                .pending_selection
                .take()
                .ok_or_else(|| "当前没有等待中的区域选择流程".to_string())?;
            apply_staged(
                &mut inner.settings,
                &pending.target,
                &prepared.progress.rects,
            );
            sender = Some(pending.sender);
        } else if let Some(pending) = inner.logic.pending_selection.as_mut() {
            pending.staged = prepared.progress.rects.clone();
            pending.current_index = prepared.progress.completed_slots.len();
        }
    }

    if let Some(sender) = sender {
        destroy_overlay_window(app);
        sender
            .send(RegionSelectionKind::Selected)
            .map_err(|_| "无法完成区域选择回传".to_string())?;
    }
    Ok(())
}

pub fn cancel_selection(
    app: &AppHandle,
    slot: usize,
    state: &State<'_, FingerprintState>,
) -> Result<(), String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "区域选择状态已损坏".to_string())?;
    let pending = inner
        .logic
        .pending_selection
        .take()
        .ok_or_else(|| "当前没有等待中的区域选择流程".to_string())?;
    if slot >= pending.staged.len() {
        return Err(format!("无效区域槽位: {slot}"));
    }
    let expected_slot = pending
        .current_slot()
        .ok_or_else(|| "当前区域选择流程没有可用槽位".to_string())?;
    if expected_slot != slot {
        inner.logic.pending_selection = Some(pending);
        return Err(format!(
            "区域选择槽位不匹配: 期望 {}, 实际 {}",
            expected_slot, slot
        ));
    }
    drop(inner);
    destroy_overlay_window(app);
    pending
        .sender
        .send(RegionSelectionKind::Cancelled)
        .map_err(|_| "无法完成区域取消回传".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    fn pending(target: &str, slots: Vec<usize>) -> PendingSelection {
        let (sender, _receiver) = oneshot::channel();
        let max = target_slot_count(target).unwrap();
        PendingSelection {
            target: target.to_string(),
            slots,
            current_index: 0,
            staged: vec![None; max],
            sender,
        }
    }

    fn sample_rect() -> RegionRect {
        RegionRect {
            x: 10,
            y: 12,
            width: 40,
            height: 22,
        }
    }

    #[test]
    fn target_slot_counts() {
        assert_eq!(target_slot_count("name").unwrap(), 1);
        assert_eq!(target_slot_count("candidates").unwrap(), 9);
        assert_eq!(target_slot_count("archive").unwrap(), 8);
        assert_eq!(target_slot_count("click").unwrap(), 7);
        assert!(target_slot_count("nope").is_err());
    }

    #[test]
    fn apply_click_regions_preserves_delay_and_compacts() {
        let mut settings = FingerprintSettings {
            click_regions: vec![ClickRegion {
                rect: RegionRect {
                    x: 1,
                    y: 2,
                    width: 12,
                    height: 12,
                },
                delay_ms: 800,
            }],
            ..FingerprintSettings::default()
        };
        let staged = vec![
            Some(RegionRect {
                x: 3,
                y: 4,
                width: 20,
                height: 20,
            }),
            None,
            Some(RegionRect {
                x: 5,
                y: 6,
                width: 22,
                height: 22,
            }),
        ];
        apply_staged(&mut settings, "click", &staged);
        assert_eq!(settings.click_regions.len(), 2);
        assert_eq!(settings.click_regions[0].delay_ms, 800);
        assert_eq!(settings.click_regions[0].rect.x, 3);
        assert_eq!(settings.click_regions[1].delay_ms, 500);
        assert_eq!(settings.click_regions[1].rect.x, 5);
    }

    #[test]
    fn parse_slots_validates() {
        assert_eq!(parse_slots(&[0, 8], 9).unwrap(), vec![0, 8]);
        assert!(parse_slots(&[], 9).is_err());
        assert!(parse_slots(&[9], 9).is_err());
        assert!(parse_slots(&[1, 1], 9).is_err());
    }

    #[test]
    fn prepare_advances_and_completes() {
        let pending = pending("candidates", vec![0, 2]);
        let first = prepare_selection_from_pending(&pending, 0, sample_rect()).unwrap();
        assert!(!first.is_complete);
        assert_eq!(first.progress.current_slot, Some(2));

        let mut next = pending;
        next.current_index = 1;
        next.staged = first.progress.rects;
        let done = prepare_selection_from_pending(&next, 2, sample_rect()).unwrap();
        assert!(done.is_complete);
        assert_eq!(done.progress.current_slot, None);
        assert_eq!(done.progress.completed_slots, vec![0, 2]);
    }
}
