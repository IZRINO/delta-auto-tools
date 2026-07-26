use std::time::{Duration, Instant};

const CLIPBOARD_RETRY_INTERVAL: Duration = Duration::from_millis(50);
const CLIPBOARD_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) fn normalize_copied_qq(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| value.to_string())
}

fn decode_utf16_clipboard(units: &[u16]) -> Result<String, String> {
    let end = units.iter().position(|unit| *unit == 0).unwrap_or(units.len());
    String::from_utf16(&units[..end]).map_err(|error| format!("剪贴板 Unicode 文本无效: {error}"))
}

#[cfg(target_os = "windows")]
struct ClipboardGuard;

#[cfg(target_os = "windows")]
impl ClipboardGuard {
    fn try_open() -> Option<Self> {
        use windows_sys::Win32::System::DataExchange::OpenClipboard;

        // SAFETY: null owner is allowed; successful open is paired with Drop.
        (unsafe { OpenClipboard(std::ptr::null_mut()) } != 0).then_some(Self)
    }
}

#[cfg(target_os = "windows")]
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::System::DataExchange::CloseClipboard;

        // SAFETY: this guard exists only after OpenClipboard succeeds.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

#[cfg(target_os = "windows")]
fn with_open_clipboard<T>(mut action: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    let deadline = Instant::now() + CLIPBOARD_TIMEOUT;
    loop {
        if let Some(_guard) = ClipboardGuard::try_open() {
            return action();
        }
        if Instant::now() >= deadline {
            return Err("系统剪贴板被占用".to_string());
        }
        std::thread::sleep(CLIPBOARD_RETRY_INTERVAL);
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn clear_clipboard() -> Result<(), String> {
    use windows_sys::Win32::System::DataExchange::EmptyClipboard;

    with_open_clipboard(|| {
        // SAFETY: clipboard is open for current thread.
        if unsafe { EmptyClipboard() } == 0 {
            Err("清空系统剪贴板失败".to_string())
        } else {
            Ok(())
        }
    })
}

#[cfg(target_os = "windows")]
fn read_clipboard_text_once() -> Result<Option<String>, String> {
    use windows_sys::Win32::System::{
        DataExchange::{GetClipboardData, IsClipboardFormatAvailable},
        Memory::{GlobalLock, GlobalSize, GlobalUnlock},
    };

    const CF_UNICODETEXT: u32 = 13;
    with_open_clipboard(|| {
        // SAFETY: clipboard is open and format query does not retain pointers.
        if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) } == 0 {
            return Ok(None);
        }
        // SAFETY: clipboard is open; handle remains owned by clipboard.
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
        if handle.is_null() {
            return Ok(None);
        }
        // SAFETY: clipboard handle for CF_UNICODETEXT is a movable global allocation.
        let size_bytes = unsafe { GlobalSize(handle) };
        let pointer = unsafe { GlobalLock(handle) }.cast::<u16>();
        if pointer.is_null() {
            return Err("锁定剪贴板 Unicode 文本失败".to_string());
        }
        // SAFETY: GlobalSize bounds the allocation; u16 is correctly aligned by GlobalLock.
        let units = unsafe { std::slice::from_raw_parts(pointer, size_bytes / 2) };
        let decoded = decode_utf16_clipboard(units);
        // SAFETY: pointer came from successful GlobalLock for this handle.
        unsafe {
            let _ = GlobalUnlock(handle);
        }
        decoded.map(Some)
    })
}

#[cfg(target_os = "windows")]
pub(crate) fn read_copied_qq() -> Result<String, String> {
    let deadline = Instant::now() + CLIPBOARD_TIMEOUT;
    loop {
        if let Some(value) = read_clipboard_text_once()? {
            return normalize_copied_qq(&value)
                .ok_or_else(|| format!("剪贴板内容不是纯数字 QQ: {value}"));
        }
        if Instant::now() >= deadline {
            return Err("剪贴板未出现 QQ 文本".to_string());
        }
        std::thread::sleep(CLIPBOARD_RETRY_INTERVAL);
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn clear_clipboard() -> Result<(), String> {
    Err("剪贴板复核仅支持 Windows".to_string())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn read_copied_qq() -> Result<String, String> {
    Err("剪贴板复核仅支持 Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_qq_must_be_non_empty_ascii_digits_after_trim() {
        assert_eq!(normalize_copied_qq(" 123456\r\n"), Some("123456".to_string()));
        assert_eq!(normalize_copied_qq(""), None);
        assert_eq!(normalize_copied_qq("123 456"), None);
        assert_eq!(normalize_copied_qq("账号123"), None);
        assert_eq!(normalize_copied_qq("１２３"), None);
    }

    #[test]
    fn unicode_clipboard_text_stops_at_first_nul() {
        let units = "123456\0stale".encode_utf16().collect::<Vec<_>>();

        assert_eq!(decode_utf16_clipboard(&units).unwrap(), "123456");
    }
}
