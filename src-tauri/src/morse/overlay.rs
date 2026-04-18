use tokio::sync::oneshot;

use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use super::{
    types::{RegionRect, RegionSelectionKind, RegionSelectionOutcome, RegionSelectionProgress},
    MorseState,
};

#[derive(Debug)]
pub struct PendingSelection {
    pub slots: Vec<usize>,
    pub current_index: usize,
    pub staged_regions: [Option<RegionRect>; 3],
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
        let Some(pending) = inner.pending_selection.take() else {
            return;
        };

        let _ = pending.sender.send(kind);
    } else {
        eprintln!("区域选择状态已损坏，无法回收待处理选择流程");
    };
}

fn destroy_overlay_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.destroy();
    }
}

fn parse_slots(slots: &[usize]) -> Result<Vec<usize>, String> {
    if slots.is_empty() {
        return Err("至少需要选择一个区域槽位".to_string());
    }

    let mut seen = [false; 3];
    let mut parsed = Vec::with_capacity(slots.len());

    for &slot in slots {
        if slot >= 3 {
            return Err(format!("无效区域槽位: {slot}"));
        }

        if seen[slot] {
            return Err(format!("区域槽位重复: {slot}"));
        }

        seen[slot] = true;
        parsed.push(slot);
    }

    Ok(parsed)
}

pub async fn begin_region_selection(
    app: &AppHandle,
    slots: Vec<usize>,
    state: State<'_, MorseState>,
) -> Result<RegionSelectionOutcome, String> {
    let slots = parse_slots(&slots)?;
    let (sender, receiver) = oneshot::channel();

    {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "区域选择状态已损坏".to_string())?;

        if inner.pending_selection.is_some() {
            return Err("当前已有一个区域选择流程在进行中".to_string());
        }

        if inner.run_in_progress {
            return Err("当前识别任务正在运行，请稍后再试".to_string());
        }

        inner.pending_selection = Some(PendingSelection {
            slots: slots.clone(),
            current_index: 0,
            staged_regions: inner.settings.regions.clone(),
            sender,
        });
    }

    destroy_overlay_window(app);

    let overlay_url = WebviewUrl::App(
        format!(
            "index.html?mode=overlay&slots={}",
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

    let regions = {
        let inner = state
            .inner
            .lock()
            .map_err(|_| "区域选择状态已损坏".to_string())?;

        if let Some(pending) = inner.pending_selection.as_ref() {
            pending.staged_regions.clone()
        } else {
            inner.settings.regions.clone()
        }
    };

    Ok(RegionSelectionOutcome { kind, regions })
}

pub fn prepare_selection(
    slot: usize,
    rect: RegionRect,
    state: &State<'_, MorseState>,
) -> Result<PreparedSelection, String> {
    if slot >= 3 {
        return Err(format!("无效区域槽位: {slot}"));
    }

    if rect.width <= 10 || rect.height <= 5 {
        return Err("所选区域过小，无法保存".to_string());
    }

    let inner = state
        .inner
        .lock()
        .map_err(|_| "区域选择状态已损坏".to_string())?;

    let pending = inner
        .pending_selection
        .as_ref()
        .ok_or_else(|| "当前没有等待中的区域选择流程".to_string())?;

    let expected_slot = pending
        .current_slot()
        .ok_or_else(|| "当前区域选择流程没有可用槽位".to_string())?;

    if expected_slot != slot {
        return Err(format!(
            "区域选择槽位不匹配: 期望 {}, 实际 {}",
            expected_slot, slot
        ));
    }

    let mut regions = pending.staged_regions.clone();
    regions[slot] = Some(rect);

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
            regions,
            completed_slots: pending.completed_slots(next_index),
        },
    })
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
                .pending_selection
                .take()
                .ok_or_else(|| "当前没有等待中的区域选择流程".to_string())?;
            inner.settings.regions = prepared.progress.regions.clone();
            sender = Some(pending.sender);
        } else if let Some(pending) = inner.pending_selection.as_mut() {
            pending.staged_regions = prepared.progress.regions.clone();
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
    state: &State<'_, MorseState>,
) -> Result<(), String> {
    if slot >= 3 {
        return Err(format!("无效区域槽位: {slot}"));
    }

    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "区域选择状态已损坏".to_string())?;

    let pending = inner
        .pending_selection
        .take()
        .ok_or_else(|| "当前没有等待中的区域选择流程".to_string())?;

    let expected_slot = pending
        .current_slot()
        .ok_or_else(|| "当前区域选择流程没有可用槽位".to_string())?;

    if expected_slot != slot {
        inner.pending_selection = Some(pending);
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
