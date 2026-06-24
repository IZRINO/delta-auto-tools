use std::collections::HashSet;
use tauri::{AppHandle, Manager};

/// 将字符串中的非 ASCII 字母数字 / 连字符 / 下划线替换为 `-`，
/// 确保结果可用作 Tauri WebView label 组件。
pub fn safe_label_component(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

/// 对查询字符串值做 URL 编码。
pub fn encoded_query_value(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// 隐藏指定 label 的窗口（如存在）。
pub fn hide_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.hide();
    }
}

/// 销毁指定 label 的窗口（如存在）。
pub fn destroy_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.destroy();
    }
}

/// 隐藏所有以 `base_label` 为前缀（含 base_label 本身）的窗口。
///
/// 与 `destroy_windows_with_prefix` 不同，只隐藏不销毁，
/// 便于全局开关重新打开时直接 show 恢复，避免窗口重建的时序问题与加载空白。
pub fn hide_windows_with_prefix(app: &AppHandle, base_label: &str) {
    let prefix = format!("{base_label}-");
    let labels = app
        .webview_windows()
        .keys()
        .filter(|label| *label == base_label || label.starts_with(&prefix))
        .cloned()
        .collect::<Vec<_>>();
    for label in labels {
        hide_window(app, &label);
    }
}

/// 销毁所有以 `base_label` 为前缀（含 base_label 本身）的窗口。
pub fn destroy_windows_with_prefix(app: &AppHandle, base_label: &str) {
    let prefix = format!("{base_label}-");
    let labels = app
        .webview_windows()
        .keys()
        .filter(|label| *label == base_label || label.starts_with(&prefix))
        .cloned()
        .collect::<Vec<_>>();
    for label in labels {
        destroy_window(app, &label);
    }
}

/// 销毁不在活跃集合中的陈旧窗口（以 base_label 为前缀）。
pub fn destroy_stale_windows(app: &AppHandle, base_label: &str, active_labels: &HashSet<String>) {
    let prefix = format!("{base_label}-");
    let labels = app
        .webview_windows()
        .keys()
        .filter(|label| {
            (*label == base_label || label.starts_with(&prefix)) && !active_labels.contains(*label)
        })
        .cloned()
        .collect::<Vec<_>>();
    for label in labels {
        destroy_window(app, &label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_label_component_ascii() {
        assert_eq!(safe_label_component("hello-world_123"), "hello-world_123");
    }

    #[test]
    fn safe_label_component_replaces_special() {
        assert_eq!(safe_label_component("默认分组"), "----");
    }

    #[test]
    fn safe_label_component_mixed() {
        assert_eq!(safe_label_component("组 1"), "--1");
    }

    #[test]
    fn encoded_query_value_simple() {
        assert_eq!(encoded_query_value("abc"), "abc");
    }

    #[test]
    fn encoded_query_value_chinese() {
        let result = encoded_query_value("默认");
        assert!(result.contains('%'));
    }
}
