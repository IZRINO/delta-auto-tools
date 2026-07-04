use std::collections::HashSet;

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use super::keys::RAPIDFIRE_DISPLAY_MIN_HEIGHT;
use super::types::{RapidfireGroup, RapidfireSettings, DEFAULT_RAPIDFIRE_GROUP_ID};
use crate::overlay_utils::{
    destroy_stale_windows, destroy_windows_with_prefix, encoded_query_value, hide_window,
    safe_label_component,
};

const RAPIDFIRE_DISPLAY_LABEL: &str = "rapidfire-display";
const RAPIDFIRE_POSITION_LABEL: &str = "rapidfire-position";

pub fn display_height(item_count: usize) -> i32 {
    RAPIDFIRE_DISPLAY_MIN_HEIGHT.max(32 + item_count.max(1) as i32 * 28)
}

pub fn ensure_overlay_window(
    app: &AppHandle,
    settings_value: &RapidfireSettings,
) -> Result<(), String> {
    let mut active_labels = HashSet::new();
    for group in &settings_value.groups {
        let label = display_label_for_group(&group.id);
        active_labels.insert(label.clone());
        ensure_overlay_window_for_group(app, settings_value, group, &label)?;
    }
    destroy_stale_windows(app, RAPIDFIRE_DISPLAY_LABEL, &active_labels);
    Ok(())
}

fn ensure_overlay_window_for_group(
    app: &AppHandle,
    settings_value: &RapidfireSettings,
    group: &RapidfireGroup,
    label: &str,
) -> Result<(), String> {
    if !group.show_overlay || !group.enabled || !settings_value.rapidfire_enabled {
        hide_window(app, label);
        return Ok(());
    }

    let enabled_count = settings_value
        .cards
        .iter()
        .filter(|c| c.enabled && c.group_id == group.id)
        .count();
    let height = display_height(enabled_count);
    let width = group.overlay_width;
    let pos = group
        .overlay_position
        .as_ref()
        .map(|p| (p.x, p.y))
        .unwrap_or((100, 100));

    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_size(PhysicalSize::new(width as u32, height as u32));
        let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
        let _ = window.set_always_on_top(true);
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.show();
        return Ok(());
    }

    let group_query_id = encoded_query_value(&group.id);
    let window = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(
            format!("index.html?mode=rapidfire-display&groupId={group_query_id}").into(),
        ),
    )
    .title(format!("连发器透明窗口 - {}", group.name))
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(true)
    .resizable(false)
    .inner_size(width as f64, height as f64)
    .position(pos.0 as f64, pos.1 as f64)
    .build()
    .map_err(|error| format!("创建连发器透明窗口失败: {error}"))?;

    let _ = window.set_ignore_cursor_events(true);
    Ok(())
}

pub fn display_label_for_group(group_id: &str) -> String {
    if group_id == DEFAULT_RAPIDFIRE_GROUP_ID {
        RAPIDFIRE_DISPLAY_LABEL.to_string()
    } else {
        format!(
            "{}-{}",
            RAPIDFIRE_DISPLAY_LABEL,
            safe_label_component(group_id)
        )
    }
}

pub fn position_label_for_group(group_id: &str) -> String {
    if group_id == DEFAULT_RAPIDFIRE_GROUP_ID {
        RAPIDFIRE_POSITION_LABEL.to_string()
    } else {
        format!(
            "{}-{}",
            RAPIDFIRE_POSITION_LABEL,
            safe_label_component(group_id)
        )
    }
}

pub fn destroy_display_windows(app: &AppHandle) {
    destroy_windows_with_prefix(app, RAPIDFIRE_DISPLAY_LABEL);
}

pub fn destroy_position_windows(app: &AppHandle) {
    destroy_windows_with_prefix(app, RAPIDFIRE_POSITION_LABEL);
}

pub fn hide_display_windows(app: &AppHandle) {
    crate::overlay_utils::hide_windows_with_prefix(app, RAPIDFIRE_DISPLAY_LABEL);
}

#[cfg(test)]
mod tests {
    use super::super::keys::RAPIDFIRE_DISPLAY_MIN_HEIGHT;
    use super::*;

    #[test]
    fn display_height_has_minimum() {
        assert_eq!(display_height(0), RAPIDFIRE_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(1), RAPIDFIRE_DISPLAY_MIN_HEIGHT);
        assert_eq!(display_height(5), 172);
    }
}
