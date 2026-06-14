use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use rodio::{Decoder, OutputStream, Sink};

/// 播放指定音频文件，支持音量调节（0.0-1.0）
///
/// 在阻塞线程中执行，因为 rodio 的 Sink::sleep_until_end 会阻塞直到播放完成。
/// 如果需要在异步上下文使用，请用 tokio::task::spawn_blocking 包裹。
///
/// 注意：每次调用都会创建新的 OutputStream。rodio 的 OutputStream 不是 Send/Sync，
/// 无法全局缓存，因此每次播放时重新初始化音频设备。对于频繁播放的场景，
/// 建议改为在后台线程中维护一个 Sink 队列。
pub fn play_audio_file(path: &str, volume: f32) -> Result<(), String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(format!("音频文件不存在: {}", path.display()));
    }

    let file = File::open(path).map_err(|e| format!("打开音频文件失败: {e}"))?;
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("解码音频文件失败: {e}"))?;

    let (_stream, stream_handle) =
        OutputStream::try_default().map_err(|e| format!("初始化音频输出失败: {e}"))?;

    let sink = Sink::try_new(&stream_handle).map_err(|e| format!("创建音频播放器失败: {e}"))?;

    sink.set_volume(volume.clamp(0.0, 1.0));
    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}
