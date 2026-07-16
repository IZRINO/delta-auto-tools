//! 计数器运行态的独立持久化。
//!
//! 用户配置（counters 列表、start_value、hotkey 等）保存在
//! `counter_settings.json`；这里保存**运行态**——counter 实际累加到的值。
//!
//! 入队时机：每次 counter 累加/重置/adjust；writer 线程 50ms 合并 latest snapshot。
//! 应用关闭时 `shutdown` 强制写最终 snapshot 并等待线程退出。
//!
//! 加载时机：
//! - `initialize()`：合并 `settings.counters` 列表与已保存的 runs，
//!   缺则用 `start_value`，孤儿 ID（settings.counters 已删除）丢弃。

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::settings;

const STATE_FILE_NAME: &str = "counter_state.json";
const LEGACY_STATE_FILE_NAME: &str = "timer_counter_state.json";
const WRITE_DEBOUNCE: Duration = Duration::from_millis(50);

/// 计数器运行态快照：每个 counter id 当前累加到的值。
///
/// 用 `BTreeMap` 而非 `HashMap`，让 JSON 序列化的 key 有序，方便 diff / git 跟踪。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CounterRunStateSnapshot {
    #[serde(default)]
    pub runs: BTreeMap<String, i64>,
}

/// 从应用配置目录读 counter 状态。
///
/// 文件不存在时尝试从旧文件 `timer_counter_state.json` 迁移。
/// 解析失败 / IO 错误时回落到 `CounterRunStateSnapshot::default()`。
pub fn load(app: &AppHandle) -> CounterRunStateSnapshot {
    let path = match settings::settings_path(app, STATE_FILE_NAME) {
        Ok(path) => path,
        Err(_) => return CounterRunStateSnapshot::default(),
    };

    if let Ok(state) = settings::load_settings::<CounterRunStateSnapshot>(&path) {
        return state;
    }

    // 迁移：旧文件 timer_counter_state.json 存在时读取并写入新文件
    let legacy_path = match settings::settings_path(app, LEGACY_STATE_FILE_NAME) {
        Ok(p) => p,
        Err(_) => return CounterRunStateSnapshot::default(),
    };

    if let Ok(legacy) = settings::load_settings::<CounterRunStateSnapshot>(&legacy_path) {
        let _ = settings::save_settings(&path, &legacy);
        return legacy;
    }

    CounterRunStateSnapshot::default()
}

enum WriterMessage {
    Save(CounterRunStateSnapshot),
    Shutdown(CounterRunStateSnapshot),
}

pub struct CounterStateWriter {
    sender: Sender<WriterMessage>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl CounterStateWriter {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        Self::spawn(settings::settings_path(app, STATE_FILE_NAME)?)
    }

    fn spawn(path: PathBuf) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("counter-state-writer".to_string())
            .spawn(move || run_writer(path, receiver))
            .map_err(|error| format!("无法启动计数器状态写入线程: {error}"))?;
        Ok(Self {
            sender,
            worker: Mutex::new(Some(worker)),
        })
    }

    pub fn save_latest(&self, snapshot: CounterRunStateSnapshot) -> Result<(), String> {
        self.sender
            .send(WriterMessage::Save(snapshot))
            .map_err(|_| "计数器状态写入线程已退出".to_string())
    }

    pub fn shutdown(&self, snapshot: CounterRunStateSnapshot) -> Result<(), String> {
        self.sender
            .send(WriterMessage::Shutdown(snapshot))
            .map_err(|_| "计数器状态写入线程已退出".to_string())?;
        let worker = self
            .worker
            .lock()
            .map_err(|_| "计数器状态写入线程锁已损坏".to_string())?
            .take();
        if let Some(worker) = worker {
            worker
                .join()
                .map_err(|_| "计数器状态写入线程异常退出".to_string())?;
        }
        Ok(())
    }
}

fn run_writer(path: PathBuf, receiver: Receiver<WriterMessage>) {
    while let Ok(first) = receiver.recv() {
        if matches!(&first, WriterMessage::Save(_)) {
            thread::sleep(WRITE_DEBOUNCE);
        }
        let (latest, shutdown) = drain_latest(first, &receiver);
        if let Err(error) = settings::save_settings(&path, &latest) {
            crate::log_error!(
                "counter",
                "保存计数器运行态失败",
                "error" => error
            );
        }
        if shutdown {
            return;
        }
    }
}

fn drain_latest(
    first: WriterMessage,
    receiver: &Receiver<WriterMessage>,
) -> (CounterRunStateSnapshot, bool) {
    let (mut latest, mut shutdown) = match first {
        WriterMessage::Save(snapshot) => (snapshot, false),
        WriterMessage::Shutdown(snapshot) => (snapshot, true),
    };
    while !shutdown {
        match receiver.try_recv() {
            Ok(WriterMessage::Save(snapshot)) => latest = snapshot,
            Ok(WriterMessage::Shutdown(snapshot)) => {
                latest = snapshot;
                shutdown = true;
            }
            Err(_) => break,
        }
    }
    (latest, shutdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let state = CounterRunStateSnapshot::default();
        assert!(state.runs.is_empty());
    }

    #[test]
    fn serialize_roundtrip_keeps_keys_sorted() {
        let mut state = CounterRunStateSnapshot::default();
        state.runs.insert("counter-c".to_string(), 3);
        state.runs.insert("counter-a".to_string(), 1);
        state.runs.insert("counter-b".to_string(), 2);
        let json = serde_json::to_string(&state).expect("serialize");
        // BTreeMap 按 key 升序序列化。
        assert!(json.find("counter-a") < json.find("counter-b"));
        assert!(json.find("counter-b") < json.find("counter-c"));
        let parsed: CounterRunStateSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, state);
    }

    #[test]
    fn save_and_load_round_trip() {
        let mut state = CounterRunStateSnapshot::default();
        state.runs.insert("counter-1".to_string(), 42);
        state.runs.insert("counter-2".to_string(), -3);

        let json = serde_json::to_string(&state).unwrap();
        let parsed: CounterRunStateSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, state);
    }

    #[test]
    fn writer_queue_keeps_only_latest_snapshot_and_shutdown() {
        let (sender, receiver) = std::sync::mpsc::channel();
        for value in 0..500 {
            let mut state = CounterRunStateSnapshot::default();
            state.runs.insert("counter".to_string(), value);
            sender.send(WriterMessage::Save(state)).unwrap();
        }
        let mut final_state = CounterRunStateSnapshot::default();
        final_state.runs.insert("counter".to_string(), 999);
        sender
            .send(WriterMessage::Shutdown(final_state.clone()))
            .unwrap();

        let first = receiver.recv().unwrap();
        let (latest, shutdown) = drain_latest(first, &receiver);

        assert!(shutdown);
        assert_eq!(latest, final_state);
    }
}
