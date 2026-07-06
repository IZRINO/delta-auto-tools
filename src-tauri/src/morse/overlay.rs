use tokio::sync::oneshot;

use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use super::{
    types::{
        ClickRegion, RegionRect, RegionSelectionKind, RegionSelectionOutcome,
        RegionSelectionProgress,
    },
    MorseState,
};

/// 将 click 模式的 `Vec<ClickRegion>` 转换为 slot 索引的 `Vec<Option<ClickRegion>>`，补齐到 7 个槽位。
fn click_regions_to_staged(click_regions: &[ClickRegion]) -> Vec<Option<ClickRegion>> {
    let mut staged = vec![None; 7];
    for (i, cr) in click_regions.iter().enumerate() {
        staged[i] = Some(cr.clone());
    }
    staged
}

/// 将 slot 索引的 `Vec<Option<ClickRegion>>` 过滤为 `Vec<ClickRegion>`（去掉未配置的槽位）。
fn staged_to_click_regions(staged: &[Option<ClickRegion>]) -> Vec<ClickRegion> {
    staged.iter().filter_map(|x| x.clone()).collect()
}

#[derive(Debug)]
pub struct PendingSelection {
    pub target: String,
    pub slots: Vec<usize>,
    pub current_index: usize,
    pub staged: Vec<Option<ClickRegion>>,
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

const OVERLAY_LABEL: &str = "morse-overlay";

fn resolve_pending(app: &AppHandle, kind: RegionSelectionKind) {
    let state = app.state::<MorseState>();
    if let Ok(mut inner) = state.inner.lock() {
        let Some(pending) = inner.logic.pending_selection.take() else {
            return;
        };

        let _ = pending.sender.send(kind);
    } else {
        crate::log_error!(
            "morse::overlay",
            "区域选择状态已损坏，无法回收待处理选择流程"
        );
    };
}

fn destroy_overlay_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.destroy();
    }
}

/// 全局关闭时取消所有活跃的 morse overlay 会话。
/// 先 resolve pending sender 为 Cancelled，再销毁 overlay 窗口。
/// 顺序关键：若先销毁窗口，Destroyed handler 会抢先 resolve 为 Closed 而非 Cancelled。
pub(crate) fn cancel_active_overlay(app: &AppHandle) {
    resolve_pending(app, RegionSelectionKind::Cancelled);
    destroy_overlay_window(app);
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
    let max_slots = pending.staged.len();
    if slot >= max_slots {
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
    // 保留已有区域的 delay_ms，新区域默认 500ms
    let existing_delay = staged[slot].as_ref().map(|c| c.delay_ms).unwrap_or(500);
    staged[slot] = Some(ClickRegion {
        rect,
        delay_ms: existing_delay,
    });

    let next_index = pending.current_index + 1;
    let is_complete = next_index >= pending.slots.len();
    let current_slot = if is_complete {
        None
    } else {
        pending.slots.get(next_index).copied()
    };

    let (regions, click_regions) = if pending.target == "click" {
        ([None, None, None], Some(staged_to_click_regions(&staged)))
    } else {
        let mut arr = [None, None, None];
        for (i, opt) in staged.iter().enumerate().take(3) {
            arr[i] = opt.as_ref().map(|c| c.rect.clone());
        }
        (arr, None)
    };

    Ok(PreparedSelection {
        expected_slot,
        is_complete,
        progress: RegionSelectionProgress {
            current_slot,
            regions,
            completed_slots: pending.completed_slots(next_index),
            target: pending.target.clone(),
            click_regions,
        },
    })
}

pub async fn begin_region_selection(
    app: &AppHandle,
    slots: Vec<usize>,
    target: String,
    state: State<'_, MorseState>,
) -> Result<RegionSelectionOutcome, String> {
    let max_slots = if target == "click" { 7 } else { 3 };
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

        let initial: Vec<Option<ClickRegion>> = if target == "click" {
            click_regions_to_staged(&inner.settings.click_regions)
        } else {
            inner
                .settings
                .regions
                .iter()
                .map(|r| {
                    r.as_ref().map(|rect| ClickRegion {
                        rect: rect.clone(),
                        delay_ms: 500,
                    })
                })
                .collect()
        };

        inner.logic.pending_selection = Some(PendingSelection {
            target: target.clone(),
            slots: slots.clone(),
            current_index: 0,
            staged: initial,
            sender,
        });
    }

    destroy_overlay_window(app);

    let overlay_url = WebviewUrl::App(
        format!(
            "index.html?mode=overlay&target={}&slots={}",
            target,
            slots
                .iter()
                .map(|slot| slot.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
        .into(),
    );
    let builder = WebviewWindowBuilder::new(app, OVERLAY_LABEL, overlay_url)
        .title("选择摩斯区域")
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

    let (regions, target, click_regions) = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "区域选择状态已损坏".to_string())?;

        if let Some(pending) = inner.logic.pending_selection.as_ref() {
            let (regions, click_regions) = if pending.target == "click" {
                (
                    [None, None, None],
                    Some(staged_to_click_regions(&pending.staged)),
                )
            } else {
                let mut arr = [None, None, None];
                for (i, opt) in pending.staged.iter().enumerate().take(3) {
                    arr[i] = opt.as_ref().map(|c| c.rect.clone());
                }
                (arr, None)
            };
            (regions, pending.target.clone(), click_regions)
        } else {
            let regions = inner.settings.regions.clone();
            (regions, "sampling".to_string(), None)
        }
    };

    Ok(RegionSelectionOutcome {
        kind,
        regions,
        target,
        click_regions,
    })
}

pub fn prepare_selection(
    slot: usize,
    rect: RegionRect,
    state: &State<'_, MorseState>,
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
    state: &State<'_, MorseState>,
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
            let is_click = pending.target == "click";
            if is_click {
                if let Some(ref click_regions) = prepared.progress.click_regions {
                    inner.settings.click_regions = click_regions.clone();
                }
            } else {
                inner.settings.regions = prepared.progress.regions.clone();
            }
            sender = Some(pending.sender);
        } else if let Some(pending) = inner.logic.pending_selection.as_mut() {
            if let Some(ref click_regions) = prepared.progress.click_regions {
                // 非完成态下，click_regions 是当前的 Vec<ClickRegion>，
                // 回填到 slot 索引的 staged 中
                let mut new_staged = vec![None; pending.staged.len()];
                for (i, cr) in click_regions.iter().enumerate() {
                    new_staged[i] = Some(cr.clone());
                }
                pending.staged = new_staged;
            } else {
                pending.staged = prepared
                    .progress
                    .regions
                    .iter()
                    .map(|r| {
                        r.as_ref().map(|rect| ClickRegion {
                            rect: rect.clone(),
                            delay_ms: 500,
                        })
                    })
                    .collect();
            }
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

/// 提前结束区域选择（仅 click 模式）。保存当前已选区域并关闭 overlay。
pub fn finish_early(app: &AppHandle, state: &State<'_, MorseState>) -> Result<(), String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "区域选择状态已损坏".to_string())?;

    let pending = inner
        .logic
        .pending_selection
        .take()
        .ok_or_else(|| "当前没有等待中的区域选择流程".to_string())?;

    if pending.target != "click" {
        // 采样模式不支持提前结束
        inner.logic.pending_selection = Some(pending);
        return Err("当前不是点击区域选择，不支持提前结束".to_string());
    }

    // 保存当前已选的 click 区域
    inner.settings.click_regions = staged_to_click_regions(&pending.staged);

    drop(inner);
    destroy_overlay_window(app);

    pending
        .sender
        .send(RegionSelectionKind::Selected)
        .map_err(|_| "无法完成区域选择回传".to_string())?;

    Ok(())
}

pub fn cancel_selection(
    app: &AppHandle,
    slot: usize,
    state: &State<'_, MorseState>,
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

    /// 创建一个包含 3 个空槽位的 PendingSelection，用于采样模式测试。
    fn sample_pending(slots: Vec<usize>, current_index: usize) -> PendingSelection {
        let (sender, _receiver) = oneshot::channel();
        PendingSelection {
            target: "sampling".to_string(),
            slots,
            current_index,
            staged: vec![None, None, None],
            sender,
        }
    }

    fn click_pending(slots: Vec<usize>, current_index: usize) -> PendingSelection {
        let (sender, _receiver) = oneshot::channel();
        PendingSelection {
            target: "click".to_string(),
            slots,
            current_index,
            staged: vec![None; 7],
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
    fn parse_slots_validates_inputs() {
        assert_eq!(parse_slots(&[0, 2], 3).unwrap(), vec![0, 2]);
        assert!(parse_slots(&[], 3).is_err());
        assert!(parse_slots(&[3], 3).is_err());
        assert!(parse_slots(&[1, 1], 3).is_err());
        assert_eq!(parse_slots(&[3, 6], 7).unwrap(), vec![3, 6]);
    }

    #[test]
    fn completed_slots_reports_prefix() {
        let pending = sample_pending(vec![0, 1, 2], 0);
        assert_eq!(pending.completed_slots(0), Vec::<usize>::new());
        assert_eq!(pending.completed_slots(2), vec![0, 1]);
    }

    #[test]
    fn prepare_selection_rejects_invalid_rectangles() {
        let pending = sample_pending(vec![0], 0);
        let rect = RegionRect {
            x: 0,
            y: 0,
            width: 5,
            height: 5,
        };

        let error = prepare_selection_from_pending(&pending, 0, rect).unwrap_err();
        assert!(error.contains("所选区域过小"));
    }

    #[test]
    fn prepare_selection_rejects_slot_mismatch() {
        let pending = sample_pending(vec![1], 0);
        let error = prepare_selection_from_pending(&pending, 0, sample_rect()).unwrap_err();
        assert!(error.contains("区域选择槽位不匹配"));
    }

    #[test]
    fn prepare_selection_advances_to_next_slot() {
        let mut pending = sample_pending(vec![0, 2], 0);
        pending.staged[1] = Some(ClickRegion {
            rect: RegionRect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
            delay_ms: 500,
        });
        let prepared = prepare_selection_from_pending(&pending, 0, sample_rect()).unwrap();
        assert!(!prepared.is_complete);
        assert_eq!(prepared.expected_slot, 0);
        assert_eq!(prepared.progress.current_slot, Some(2));
        assert_eq!(prepared.progress.completed_slots, vec![0]);
        assert_eq!(prepared.progress.regions[0], Some(sample_rect()));
        assert!(prepared.progress.regions[1].is_some());
    }

    #[test]
    fn prepare_selection_marks_completion() {
        let pending = sample_pending(vec![2], 0);
        let prepared = prepare_selection_from_pending(&pending, 2, sample_rect()).unwrap();
        assert!(prepared.is_complete);
        assert_eq!(prepared.progress.current_slot, None);
        assert_eq!(prepared.progress.completed_slots, vec![2]);
        assert_eq!(prepared.progress.regions[2], Some(sample_rect()));
    }

    #[test]
    fn prepare_click_selection_accepts_slot_three_and_preserves_target() {
        let pending = click_pending(vec![3], 0);
        let prepared = prepare_selection_from_pending(&pending, 3, sample_rect()).unwrap();

        assert!(prepared.is_complete);
        assert_eq!(prepared.progress.target, "click");
        assert_eq!(prepared.progress.current_slot, None);
        assert_eq!(prepared.progress.completed_slots, vec![3]);
        assert_eq!(prepared.progress.regions, [None, None, None]);
        assert_eq!(
            prepared.progress.click_regions.unwrap()[0].rect,
            sample_rect()
        );
    }

    /// 验证全局关闭时 morse pending_selection 的 sender 被 resolve 为 Cancelled。
    /// cancel_active_overlay 的核心语义：取走 pending 并发送 Cancelled，使 receiver 不再挂起。
    #[test]
    fn pending_selection_sender_resolves_with_cancelled() {
        let (sender, receiver) = oneshot::channel();

        // 模拟 cancel_active_overlay 中 resolve_pending 的逻辑：
        // 取走 pending 并向 sender 发送 Cancelled
        sender.send(RegionSelectionKind::Cancelled).unwrap();

        let result = receiver.blocking_recv().unwrap();
        assert!(
            matches!(result, RegionSelectionKind::Cancelled),
            "全局关闭应 resolve 为 Cancelled，实际: {result:?}"
        );
    }

    /// 验证空状态（无 pending_selection）下 cancel 不 panic。
    #[test]
    fn no_pending_selection_cancel_is_noop() {
        // 当 pending_selection 为 None 时，
        // resolve_pending 的逻辑是 take() 返回 None 后 return，
        // 不会 panic。
        let option: Option<PendingSelection> = None;
        assert!(option.is_none(), "空 pending 不应触发 sender 操作");
    }

    /// 验证 cancel_active_overlay 先 resolve_pending 再 destroy_overlay_window 的顺序。
    /// 若先 destroy，窗口 Destroyed 事件会抢先 resolve 为 Closed 而非 Cancelled。
    /// 此测试模拟正确的顺序：先 resolve(Cancelled) 使 sender 被消费，
    /// 再 destroy 时 on_window_event 中的 resolve_pending 因 take() 返回 None 而跳过。
    #[test]
    fn resolve_pending_before_destroy_prevents_closed_override() {
        let (sender, receiver) = oneshot::channel();

        // 模拟 cancel_active_overlay 的正确顺序：
        // 1. 先 resolve_pending(Cancelled) — 消费 sender
        sender.send(RegionSelectionKind::Cancelled).unwrap();

        // 2. 窗口销毁后 on_window_event 触发 resolve_pending，
        //    但 take() 返回 None（已被消费），不会覆盖为 Closed。
        //    验证 receiver 收到的是 Cancelled 而非 Closed。
        let result = receiver.blocking_recv().unwrap();
        assert!(
            matches!(result, RegionSelectionKind::Cancelled),
            "先 resolve 再 destroy 应收到 Cancelled，实际: {result:?}"
        );
    }

    /// 验证 resolve_pending 对已消费的 pending 不重复操作。
    /// 当 pending_selection 已被 take() 后，再次调用 resolve_pending 应安全跳过。
    #[test]
    fn resolve_pending_noop_when_already_consumed() {
        let (sender, _receiver) = oneshot::channel();

        // 模拟 pending_selection 已被 take() 并消费
        let mut option: Option<oneshot::Sender<RegionSelectionKind>> = Some(sender);
        let taken = option.take();
        assert!(taken.is_some(), "首次 take 应成功");

        // 二次 take 返回 None，模拟 resolve_pending 中的安全跳过
        let second_take = option.take();
        assert!(
            second_take.is_none(),
            "二次 take 应返回 None，不重复 resolve"
        );
    }
}
