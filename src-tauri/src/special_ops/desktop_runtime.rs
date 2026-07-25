use std::cmp::Reverse;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, HWND, INVALID_HANDLE_VALUE, RECT, WAIT_FAILED,
    WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetForegroundWindow, GetWindow, GetWindowRect,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow,
    ShowWindowAsync, GW_OWNER, SW_RESTORE,
};

const PROCESS_PATH_BUFFER_LEN: usize = 32_768;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowIdentity {
    pub process_id: u32,
    pub handle: u64,
}

#[allow(dead_code)]
pub(crate) trait DesktopRuntime: Send + Sync {
    fn terminate_exact(&self, exe: &Path, timeout: Duration) -> Result<(), String>;
    fn launch(&self, exe: &Path) -> Result<u32, String>;
    fn find_primary_window(&self, exe: &Path) -> Result<Option<WindowIdentity>, String>;
    fn restore_and_focus(&self, exe: &Path, window: WindowIdentity) -> Result<(), String>;
}

#[allow(dead_code)]
pub(crate) struct WindowsDesktopRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessCandidate {
    pid: u32,
    // v1 不递归结束子进程，仅保留该字段表达进程快照关系。
    #[allow(dead_code)]
    parent_pid: u32,
    full_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowCandidate {
    hwnd: u64,
    pid: u32,
    visible: bool,
    owned: bool,
    area: u64,
}

struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn new(handle: HANDLE, action: &str) -> Result<Self, String> {
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(last_error(action));
        }
        Ok(Self(handle))
    }

    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: OwnedHandle only stores valid owned handles and closes each exactly once.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

impl DesktopRuntime for WindowsDesktopRuntime {
    fn terminate_exact(&self, exe: &Path, timeout: Duration) -> Result<(), String> {
        let started = Instant::now();
        loop {
            let candidates = scan_process_candidates(exe)?;
            let process_ids = matching_process_ids(exe, &candidates);
            if process_ids.is_empty() {
                return Ok(());
            }
            for process_id in process_ids {
                let remaining = timeout
                    .checked_sub(started.elapsed())
                    .filter(|duration| !duration.is_zero())
                    .ok_or_else(|| format!("等待进程退出超时: PID {process_id}"))?;
                terminate_process(process_id, remaining)?;
            }
        }
    }

    fn launch(&self, exe: &Path) -> Result<u32, String> {
        let parent = exe
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| format!("启动程序失败，exe 路径缺少父目录: {}", exe.display()))?;
        Command::new(exe)
            .current_dir(parent)
            .spawn()
            .map(|child| child.id())
            .map_err(|error| format!("启动程序失败 {}: {error}", exe.display()))
    }

    fn find_primary_window(&self, exe: &Path) -> Result<Option<WindowIdentity>, String> {
        let candidates = scan_process_candidates(exe)?;
        let process_ids = matching_process_ids(exe, &candidates);
        if process_ids.is_empty() {
            return Ok(None);
        }
        let windows = enumerate_windows()?;
        let Some(window) = select_primary_window(&process_ids, &windows) else {
            return Ok(None);
        };
        let current_path = query_process_path(window.process_id)?;
        if !windows_paths_equal(exe, &current_path) {
            return Err(format!("窗口所属进程路径已变化: PID {}", window.process_id));
        }
        Ok(Some(window))
    }

    fn restore_and_focus(&self, exe: &Path, window: WindowIdentity) -> Result<(), String> {
        let hwnd = hwnd_from_identity(window)?;
        // SAFETY: hwnd is treated as an opaque borrowed handle and validated before use.
        unsafe {
            if IsWindow(hwnd) == 0 {
                return Err("目标窗口已失效".to_string());
            }
        }

        let mut process_id = 0;
        // SAFETY: process_id points to valid writable memory for the duration of the call.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut process_id);
        }
        if process_id == 0 || process_id != window.process_id {
            return Err("目标窗口归属已变化".to_string());
        }
        let current_path = query_process_path(process_id)?;
        if !windows_paths_equal(exe, &current_path) {
            return Err("目标窗口所属程序已变化".to_string());
        }

        // SAFETY: hwnd remains valid and belongs to the expected executable at this point.
        unsafe {
            if IsIconic(hwnd) != 0 && ShowWindowAsync(hwnd, SW_RESTORE) == 0 {
                return Err(last_error("恢复目标窗口失败"));
            }
            if SetForegroundWindow(hwnd) == 0 {
                return Err("聚焦目标窗口失败".to_string());
            }
            if GetForegroundWindow() != hwnd {
                return Err("目标窗口未成为前台窗口".to_string());
            }
        }
        Ok(())
    }
}

fn windows_paths_equal(left: &Path, right: &Path) -> bool {
    normalized_windows_path(left) == normalized_windows_path(right)
}

fn normalized_windows_path(path: &Path) -> Vec<u16> {
    let path = path
        .as_os_str()
        .encode_wide()
        .map(normalize_windows_char)
        .collect::<Vec<_>>();
    const DEVICE_PREFIX: [u16; 4] = [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
    const UNC_PREFIX: [u16; 4] = [b'u' as u16, b'n' as u16, b'c' as u16, b'\\' as u16];
    if !path.starts_with(&DEVICE_PREFIX) {
        return path;
    }
    if path[DEVICE_PREFIX.len()..].starts_with(&UNC_PREFIX) {
        let mut normalized = vec![b'\\' as u16, b'\\' as u16];
        normalized.extend_from_slice(&path[DEVICE_PREFIX.len() + UNC_PREFIX.len()..]);
        return normalized;
    }
    path[DEVICE_PREFIX.len()..].to_vec()
}

fn normalize_windows_char(character: u16) -> u16 {
    match character {
        value if value == b'/' as u16 => b'\\' as u16,
        value if (b'A' as u16..=b'Z' as u16).contains(&value) => value + 32,
        value => value,
    }
}

fn matching_process_ids(target: &Path, candidates: &[ProcessCandidate]) -> Vec<u32> {
    let mut process_ids = candidates
        .iter()
        .filter_map(|candidate| {
            windows_paths_equal(target, &candidate.full_path).then_some(candidate.pid)
        })
        .collect::<Vec<_>>();
    process_ids.sort_unstable();
    process_ids
}

fn select_primary_window(
    target_process_ids: &[u32],
    candidates: &[WindowCandidate],
) -> Option<WindowIdentity> {
    candidates
        .iter()
        .filter(|candidate| {
            target_process_ids.contains(&candidate.pid)
                && candidate.visible
                && !candidate.owned
                && candidate.area > 0
        })
        .min_by_key(|candidate| (Reverse(candidate.area), candidate.hwnd))
        .map(|candidate| WindowIdentity {
            process_id: candidate.pid,
            handle: candidate.hwnd,
        })
}

fn scan_process_candidates(exe: &Path) -> Result<Vec<ProcessCandidate>, String> {
    let target_name = exe
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("exe 路径缺少文件名: {}", exe.display()))?;
    // SAFETY: arguments follow CreateToolhelp32Snapshot contract; returned handle is owned.
    let snapshot = unsafe {
        OwnedHandle::new(
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0),
            "创建进程快照失败",
        )?
    };
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: snapshot is valid and entry points to initialized writable storage.
    if unsafe { Process32FirstW(snapshot.raw(), &mut entry) } == 0 {
        // SAFETY: called immediately after failed Win32 call.
        return if unsafe { GetLastError() } == windows_sys::Win32::Foundation::ERROR_NO_MORE_FILES {
            Ok(Vec::new())
        } else {
            Err(last_error("读取进程快照失败"))
        };
    }

    let mut candidates = Vec::new();
    loop {
        if windows_names_equal(target_name, &entry.szExeFile) {
            candidates.push(ProcessCandidate {
                pid: entry.th32ProcessID,
                parent_pid: entry.th32ParentProcessID,
                full_path: query_process_path(entry.th32ProcessID)?,
            });
        }
        // SAFETY: snapshot and entry remain valid for the enumeration lifetime.
        if unsafe { Process32NextW(snapshot.raw(), &mut entry) } == 0 {
            // SAFETY: called immediately after failed Win32 call.
            if unsafe { GetLastError() } == windows_sys::Win32::Foundation::ERROR_NO_MORE_FILES {
                break;
            }
            return Err(last_error("继续读取进程快照失败"));
        }
    }
    Ok(candidates)
}

fn windows_names_equal(target: &OsStr, candidate: &[u16]) -> bool {
    let candidate_len = candidate
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(candidate.len());
    target
        .encode_wide()
        .map(normalize_windows_char)
        .eq(candidate[..candidate_len]
            .iter()
            .copied()
            .map(normalize_windows_char))
}

fn query_process_path(process_id: u32) -> Result<PathBuf, String> {
    let process = open_process(
        process_id,
        PROCESS_QUERY_LIMITED_INFORMATION,
        "打开进程查询路径失败",
    )?;
    let mut path = vec![0_u16; PROCESS_PATH_BUFFER_LEN];
    let mut path_len = path.len() as u32;
    // SAFETY: process is valid; path buffer and length pointer are writable and correctly sized.
    if unsafe { QueryFullProcessImageNameW(process.raw(), 0, path.as_mut_ptr(), &mut path_len) }
        == 0
    {
        return Err(last_error(&format!(
            "查询进程完整路径失败: PID {process_id}"
        )));
    }
    path.truncate(path_len as usize);
    Ok(PathBuf::from(OsString::from_wide(&path)))
}

fn open_process(process_id: u32, access: u32, action: &str) -> Result<OwnedHandle, String> {
    // SAFETY: process ID and access mask are plain values; returned handle is owned.
    unsafe {
        OwnedHandle::new(
            OpenProcess(access, 0, process_id),
            &format!("{action}: PID {process_id}"),
        )
    }
}

fn terminate_process(process_id: u32, timeout: Duration) -> Result<(), String> {
    let process = open_process(
        process_id,
        PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
        "打开待结束进程失败",
    )?;
    // SAFETY: process handle grants terminate and synchronize access.
    if unsafe { TerminateProcess(process.raw(), 1) } == 0 {
        return Err(last_error(&format!("结束进程失败: PID {process_id}")));
    }
    // SAFETY: process handle is valid and wait duration is bounded to u32 milliseconds.
    match unsafe { WaitForSingleObject(process.raw(), wait_millis(timeout)) } {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_TIMEOUT => Err(format!("等待进程退出超时: PID {process_id}")),
        WAIT_FAILED => Err(last_error(&format!("等待进程退出失败: PID {process_id}"))),
        result => Err(format!(
            "等待进程退出返回未知状态 {result}: PID {process_id}"
        )),
    }
}

fn wait_millis(timeout: Duration) -> u32 {
    timeout.as_millis().clamp(1, u32::MAX as u128) as u32
}

#[derive(Default)]
struct EnumWindowsContext {
    candidates: Vec<WindowCandidate>,
}

fn enumerate_windows() -> Result<Vec<WindowCandidate>, String> {
    let mut context = EnumWindowsContext::default();
    // SAFETY: EnumWindows invokes the callback synchronously while context remains alive.
    if unsafe {
        EnumWindows(
            Some(collect_window),
            (&mut context as *mut EnumWindowsContext) as isize,
        )
    } == 0
    {
        return Err(last_error("枚举顶层窗口失败"));
    }
    Ok(context.candidates)
}

unsafe extern "system" fn collect_window(hwnd: HWND, context: isize) -> i32 {
    // SAFETY: context is the non-null pointer supplied by enumerate_windows for this call.
    let context = unsafe { &mut *(context as *mut EnumWindowsContext) };
    let mut process_id = 0;
    // SAFETY: process_id points to writable memory and hwnd comes from EnumWindows.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut process_id);
    }
    context.candidates.push(WindowCandidate {
        hwnd: hwnd as usize as u64,
        pid: process_id,
        // SAFETY: hwnd comes from EnumWindows and is valid during callback execution.
        visible: unsafe { IsWindowVisible(hwnd) } != 0,
        // SAFETY: hwnd comes from EnumWindows; null owner means unowned top-level window.
        owned: !unsafe { GetWindow(hwnd, GW_OWNER) }.is_null(),
        area: window_area(hwnd),
    });
    1
}

fn window_area(hwnd: HWND) -> u64 {
    let mut rect = RECT::default();
    // SAFETY: rect points to writable memory and hwnd is borrowed from EnumWindows.
    if unsafe { GetClientRect(hwnd, &mut rect) } != 0 {
        let area = rect_area(rect);
        if area > 0 {
            return area;
        }
    }
    // SAFETY: rect points to writable memory and hwnd is borrowed from EnumWindows.
    if unsafe { GetWindowRect(hwnd, &mut rect) } != 0 {
        return rect_area(rect);
    }
    0
}

fn rect_area(rect: RECT) -> u64 {
    let width = i64::from(rect.right) - i64::from(rect.left);
    let height = i64::from(rect.bottom) - i64::from(rect.top);
    u64::try_from(width)
        .ok()
        .zip(u64::try_from(height).ok())
        .map_or(0, |(width, height)| width.saturating_mul(height))
}

fn hwnd_from_identity(window: WindowIdentity) -> Result<HWND, String> {
    usize::try_from(window.handle)
        .map(|handle| handle as HWND)
        .map_err(|_| "目标窗口句柄超出当前平台宽度".to_string())
}

fn last_error(action: &str) -> String {
    format!("{action}: {}", std::io::Error::last_os_error())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn same_basename_in_other_directory_is_not_a_match() {
        assert!(!windows_paths_equal(
            Path::new(r"C:\Apps\WeGame\WeGame.exe"),
            Path::new(r"D:\Copy\WeGame.exe"),
        ));
    }

    #[test]
    fn equivalent_windows_paths_match_case_insensitively() {
        assert!(windows_paths_equal(
            Path::new(r"C:\Apps\WeGame\WeGame.exe"),
            Path::new(r"\\?\c:\apps\wegame\WEGAME.EXE"),
        ));
    }

    #[test]
    fn child_process_is_not_selected_without_exact_path_match() {
        let target = Path::new(r"C:\Apps\WeGame\WeGame.exe");
        let candidates = vec![
            ProcessCandidate {
                pid: 10,
                parent_pid: 0,
                full_path: PathBuf::from(r"C:\Apps\WeGame\WeGame.exe"),
            },
            ProcessCandidate {
                pid: 11,
                parent_pid: 10,
                full_path: PathBuf::from(r"C:\Apps\WeGame\helper.exe"),
            },
        ];

        assert_eq!(matching_process_ids(target, &candidates), vec![10]);
    }

    #[test]
    fn primary_window_ignores_hidden_and_foreign_windows() {
        let windows = vec![
            WindowCandidate {
                hwnd: 1,
                pid: 10,
                visible: false,
                owned: false,
                area: 1_000,
            },
            WindowCandidate {
                hwnd: 2,
                pid: 99,
                visible: true,
                owned: false,
                area: 900,
            },
            WindowCandidate {
                hwnd: 3,
                pid: 10,
                visible: true,
                owned: true,
                area: 800,
            },
            WindowCandidate {
                hwnd: 4,
                pid: 10,
                visible: true,
                owned: false,
                area: 0,
            },
            WindowCandidate {
                hwnd: 6,
                pid: 10,
                visible: true,
                owned: false,
                area: 500,
            },
            WindowCandidate {
                hwnd: 5,
                pid: 10,
                visible: true,
                owned: false,
                area: 500,
            },
        ];

        assert_eq!(
            select_primary_window(&[10], &windows),
            Some(WindowIdentity {
                process_id: 10,
                handle: 5,
            })
        );
    }
}
