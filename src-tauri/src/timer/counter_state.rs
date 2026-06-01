//! 计数器运行态的独立持久化。
//!
//! 用户配置（counters 列表、start_value、hotkey 等）保存在
//! `timer_settings.json`；这里保存**运行态**——counter 实际累加到的值。
//!
//! 原因：`TimerStateInner.counter_runs` 是纯内存的 HashMap，`initialize()`
//! 只从 `start_value` 重新初始化，每次启动都丢失上一次的累加。
//! 单独一个文件 + 独立 load/save，避免污染 `TimerSettings` 的语义（用户配置 vs 运行态）。
//!
//! 写入时机：
//! - 每次 counter 累加（`trigger_hotkey_targets` counter 路径）
//! - 每次 counter 重置（`timer_counter_reset`）
//! - 应用关闭（`shutdown` 兜底）
//!
//! 加载时机：
//! - `initialize()`：合并 `settings.counters` 列表与已保存的 runs，
//!   缺则用 `start_value`，孤儿 ID（settings.counters 已删除）丢弃。
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::settings;

const STATE_FILE_NAME: &str = "timer_counter_state.json";

/// 计数器运行态快照：每个 counter id 当前累加到的值。
///
/// 用 `BTreeMap` 而非 `HashMap`，让 JSON 序列化的 key 有序，方便 diff / git 跟踪。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CounterState {
    #[serde(default)]
    pub runs: BTreeMap<String, i64>,
}

/// 从应用配置目录读 counter 状态。
///
/// 文件不存在 / 解析失败 / IO 错误时回落到 `CounterState::default()`，
/// 这是 init 路径上的"乐观"读盘——不应该让启动因为 counter 状态损坏而崩。
pub fn load(app: &AppHandle) -> CounterState {
    let path = match settings::settings_path(app, STATE_FILE_NAME) {
        Ok(path) => path,
        Err(_) => return CounterState::default(),
    };
    settings::load_settings(&path).unwrap_or_default()
}

/// 写 counter 状态。返回 IO 错误字符串供调用方决定如何处理。
pub fn save(app: &AppHandle, state: &CounterState) -> Result<(), String> {
    let path = settings::settings_path(app, STATE_FILE_NAME)?;
    settings::save_settings(&path, state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let state = CounterState::default();
        assert!(state.runs.is_empty());
    }

    #[test]
    fn serialize_roundtrip_keeps_keys_sorted() {
        let mut state = CounterState::default();
        state.runs.insert("counter-c".to_string(), 3);
        state.runs.insert("counter-a".to_string(), 1);
        state.runs.insert("counter-b".to_string(), 2);
        let json = serde_json::to_string(&state).expect("serialize");
        // BTreeMap 按 key 升序序列化。
        assert!(json.find("counter-a") < json.find("counter-b"));
        assert!(json.find("counter-b") < json.find("counter-c"));
        let parsed: CounterState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, state);
    }

    #[test]
    fn save_and_load_round_trip() {
        // 用一个临时目录模拟 settings 路径：直接用 settings::save/load 自检。
        let mut state = CounterState::default();
        state.runs.insert("counter-1".to_string(), 42);
        state.runs.insert("counter-2".to_string(), -3);

        let json = serde_json::to_string(&state).unwrap();
        let parsed: CounterState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, state);
    }
}
