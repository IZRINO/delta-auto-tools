//! 音频播放协调器
//!
//! 通过专用音频线程 + mpsc channel 管理 OutputStream 生命周期，
//! 支持 exclusive（互斥）和 simultaneous（并发）两种播放模式。
//!
//! rodio 的 OutputStream 不是 Send/Sync，因此必须在创建它的线程中持有。
//! 专用线程通过 mpsc::channel 接收 AudioCommand，避免跨线程转移 OutputStream。

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::mpsc;

use rodio::{Decoder, OutputStream, Sink};

/// 播放命令
#[derive(Debug)]
pub enum AudioCommand {
    /// 播放音频（互斥模式：停止当前 primary sink 再播放）
    Play {
        /// 音频文件路径
        path: String,
        /// 播放音量
        volume: f32,
        /// 是否独占（互斥）：true 时停止当前 primary sink 再播放
        exclusive: bool,
    },
    /// 关闭音频线程
    Shutdown,
}

/// 启动专用音频线程，返回命令发送端
pub fn start_audio_thread() -> (
    mpsc::Sender<AudioCommand>,
    Option<std::thread::JoinHandle<()>>,
) {
    let (tx, rx) = mpsc::channel();
    let builder = std::thread::Builder::new().name("audio-playback".to_string());
    let worker = builder.spawn(move || audio_thread_main(rx)).ok();
    (tx, worker)
}

fn audio_thread_main(rx: mpsc::Receiver<AudioCommand>) {
    // 初始化音频输出设备
    let (stream, stream_handle) = match OutputStream::try_default() {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("[音频] 初始化音频输出失败: {e}，音频线程退出");
            return;
        }
    };

    let mut primary_sink: Option<Sink> = None;
    let mut simultaneous_sinks: Vec<Sink> = Vec::new();

    // stream 必须存活，否则音频无法播放
    let _stream = stream;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCommand::Play {
                path,
                volume,
                exclusive,
            } => {
                if exclusive {
                    // 互斥模式：停止当前 primary sink
                    if let Some(sink) = primary_sink.take() {
                        sink.stop();
                    }
                    match Sink::try_new(&stream_handle) {
                        Ok(sink) => {
                            sink.set_volume(volume.clamp(0.0, 1.0));
                            if let Ok(source) = open_audio_source(&path) {
                                sink.append(source);
                                primary_sink = Some(sink);
                            }
                        }
                        Err(e) => eprintln!("[音频] 创建互斥 Sink 失败: {e}"),
                    }
                } else {
                    // 并发模式：创建独立 Sink
                    match Sink::try_new(&stream_handle) {
                        Ok(sink) => {
                            sink.set_volume(volume.clamp(0.0, 1.0));
                            if let Ok(source) = open_audio_source(&path) {
                                sink.append(source);
                                simultaneous_sinks.push(sink);
                            }
                        }
                        Err(e) => eprintln!("[音频] 创建并发 Sink 失败: {e}"),
                    }
                }
                // 清理已结束的 concurrent sinks
                simultaneous_sinks.retain(|s| !s.empty());
            }
            AudioCommand::Shutdown => {
                // 停止所有播放并退出
                if let Some(sink) = primary_sink.take() {
                    sink.stop();
                }
                for sink in simultaneous_sinks.drain(..) {
                    sink.stop();
                }
                break;
            }
        }
    }
}

/// 打开音频文件并解码
fn open_audio_source(path: &str) -> Result<rodio::Decoder<BufReader<File>>, String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(format!("音频文件不存在: {}", path.display()));
    }
    let file = File::open(path).map_err(|e| format!("打开音频文件失败: {e}"))?;
    Decoder::new(BufReader::new(file)).map_err(|e| format!("解码音频文件失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-9: audio_thread_main 测试 — 启动/停止生命周期
    #[test]
    fn start_audio_thread_and_shutdown() {
        let (tx, worker) = start_audio_thread();
        assert!(worker.is_some(), "音频线程应启动");

        // 发送 Shutdown 命令
        let result = tx.send(AudioCommand::Shutdown);
        assert!(result.is_ok(), "发送 Shutdown 应成功");

        // 等待线程结束
        if let Some(handle) = worker {
            let join_result = handle.join();
            assert!(join_result.is_ok(), "音频线程应正常退出");
        }
    }

    /// T-9: audio_thread_main 异常路径 — 不存在的音频文件
    #[test]
    fn audio_thread_handles_nonexistent_file() {
        let (tx, worker) = start_audio_thread();

        // 发送不存在的文件播放命令
        let result = tx.send(AudioCommand::Play {
            path: "/nonexistent/audio/file.mp3".to_string(),
            volume: 0.5,
            exclusive: true,
        });
        assert!(result.is_ok(), "发送 Play 应成功");

        // 关闭线程
        let _ = tx.send(AudioCommand::Shutdown);
        if let Some(handle) = worker {
            let join_result = handle.join();
            assert!(join_result.is_ok(), "即使播放失败，线程也应正常退出");
        }
    }

    /// T-9: audio_thread_main 并发播放命令
    #[test]
    fn audio_thread_handles_concurrent_play() {
        let (tx, worker) = start_audio_thread();

        // 发送多条并发播放命令
        for _ in 0..5 {
            let result = tx.send(AudioCommand::Play {
                path: "/nonexistent/file.mp3".to_string(),
                volume: 0.8,
                exclusive: false,
            });
            assert!(result.is_ok(), "发送并发 Play 应成功");
        }

        // 关闭线程
        let _ = tx.send(AudioCommand::Shutdown);
        if let Some(handle) = worker {
            let join_result = handle.join();
            assert!(join_result.is_ok(), "并发播放后线程应正常退出");
        }
    }
}
