use std::cmp::Reverse;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LUID, RECT};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, TerminateProcess,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetForegroundWindow, GetWindow, GetWindowRect,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow,
    ShowWindowAsync, GW_OWNER, SW_RESTORE,
};

const PROCESS_PATH_BUFFER_LEN: usize = 32_768;
const FOREGROUND_SETTLE_TIMEOUT: Duration = Duration::from_millis(1_500);
const FOREGROUND_SETTLE_POLL: Duration = Duration::from_millis(50);
const TERMINATE_RESCAN_INTERVAL: Duration = Duration::from_millis(100);
const TERMINATE_RETRY_PAUSE: Duration = Duration::from_millis(400);
const TERMINATE_ATTEMPTS: usize = 2;

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
    fn find_primary_window_in_tree(&self, exe: &Path) -> Result<Option<WindowIdentity>, String>;
    fn restore_and_focus(&self, exe: &Path, window: WindowIdentity) -> Result<(), String>;
    fn restore_and_focus_in_tree(&self, exe: &Path, window: WindowIdentity) -> Result<(), String>;
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessEntry {
    pid: u32,
    parent_pid: u32,
    executable_name: Vec<u16>,
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
    fn from_valid(handle: HANDLE) -> Self {
        debug_assert!(!handle.is_null() && handle != INVALID_HANDLE_VALUE);
        Self(handle)
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
        let exe = canonicalize_executable_path(exe, "规范化程序路径失败")?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| "进程结束超时范围无效".to_string())?;
        terminate_until_no_exact_matches(
            || remaining_until(deadline).map(drop),
            || {
                scan_process_entries_by_name(&exe).map(|candidates| {
                    candidates
                        .into_iter()
                        .map(|(process_id, _)| process_id)
                        .collect()
                })
            },
            |process_id| terminate_verified_process_without_wait(&exe, process_id),
        )
    }

    fn launch(&self, exe: &Path) -> Result<u32, String> {
        let exe = canonicalize_executable_path(exe, "规范化程序路径失败")?;
        let parent = exe
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| "程序路径缺少父目录".to_string())?;
        Command::new(&exe)
            .current_dir(parent)
            .spawn()
            .map(|child| child.id())
            .map_err(|error| format_win32_error("启动程序失败", error))
    }

    fn find_primary_window(&self, exe: &Path) -> Result<Option<WindowIdentity>, String> {
        let exe = canonicalize_executable_path(exe, "规范化程序路径失败")?;
        let candidates = scan_process_candidates(&exe)?;
        let process_ids = matching_process_ids(&exe, &candidates);
        if process_ids.is_empty() {
            return Ok(None);
        }
        let windows = enumerate_windows()?;
        let Some(window) = select_primary_window(&process_ids, &windows) else {
            return Ok(None);
        };
        let current_path = query_process_path(window.process_id)?;
        if !windows_paths_equal(&exe, &current_path) {
            return Err(format!("窗口所属进程路径已变化: PID {}", window.process_id));
        }
        Ok(Some(window))
    }

    fn find_primary_window_in_tree(&self, exe: &Path) -> Result<Option<WindowIdentity>, String> {
        let exe = canonicalize_executable_path(exe, "规范化程序路径失败")?;
        let process_ids = process_tree_ids_for_executable(&exe)?;
        if process_ids.is_empty() {
            return Ok(None);
        }
        let windows = enumerate_windows()?;
        let Some(window) = select_primary_window(&process_ids, &windows) else {
            return Ok(None);
        };
        if !process_tree_contains_for_executable(&exe, window.process_id)? {
            return Err(format!(
                "窗口所属进程已离开目标进程树: PID {}",
                window.process_id
            ));
        }
        Ok(Some(window))
    }

    fn restore_and_focus(&self, exe: &Path, window: WindowIdentity) -> Result<(), String> {
        let exe = canonicalize_executable_path(exe, "规范化程序路径失败")?;
        let hwnd = validate_window_identity(window)?;
        let current_path = query_process_path(window.process_id)?;
        if !windows_paths_equal(&exe, &current_path) {
            return Err("目标窗口所属程序已变化".to_string());
        }
        restore_and_focus_verified(hwnd)
    }

    fn restore_and_focus_in_tree(&self, exe: &Path, window: WindowIdentity) -> Result<(), String> {
        let exe = canonicalize_executable_path(exe, "规范化程序路径失败")?;
        let hwnd = validate_window_identity(window)?;
        if !process_tree_contains_for_executable(&exe, window.process_id)? {
            return Err("目标窗口所属进程树已变化".to_string());
        }
        restore_and_focus_verified(hwnd)
    }
}

/// 按 canonical 路径匹配进程后直接 `TerminateProcess`。最多两轮，每轮后看
/// 还在不在；两轮后仍在则失败，调用方不得继续启动 WeGame。
pub(crate) fn terminate_exact_without_waiting(exe: &Path) -> Result<(), String> {
    let exe = canonicalize_executable_path(exe, "规范化程序路径失败")?;
    match terminate_until_gone(
        || {
            scan_process_entries_by_name(&exe).map(|candidates| {
                candidates
                    .into_iter()
                    .map(|(process_id, _)| process_id)
                    .collect()
            })
        },
        |process_id| terminate_verified_process_without_wait(&exe, process_id),
        TERMINATE_ATTEMPTS,
        TERMINATE_RETRY_PAUSE,
    ) {
        Ok(()) => Ok(()),
        Err(error) => {
            if close_succeeded(false, !primary_window_absent(&exe)) {
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}

fn primary_window_absent(exe: &Path) -> bool {
    !matches!(
        WindowsDesktopRuntime.find_primary_window(exe),
        Ok(Some(_))
    )
}

fn close_succeeded(process_gone: bool, window_present: bool) -> bool {
    process_gone || !window_present
}

fn terminate_until_gone(
    mut scan: impl FnMut() -> Result<Vec<u32>, String>,
    mut terminate: impl FnMut(u32) -> Result<bool, String>,
    attempts: usize,
    pause: Duration,
) -> Result<(), String> {
    for attempt in 0..attempts {
        let process_ids = scan()?;
        if process_ids.is_empty() {
            return Ok(());
        }
        for process_id in process_ids {
            let _ = terminate(process_id);
        }
        if attempt + 1 < attempts && !pause.is_zero() {
            std::thread::sleep(pause);
        }
    }
    if scan()?.is_empty() {
        Ok(())
    } else {
        Err("目标进程仍在运行".to_string())
    }
}

fn terminate_verified_process_without_wait(exe: &Path, process_id: u32) -> Result<bool, String> {
    if !should_close_name_matched_process(exe, query_process_path(process_id))? {
        return Ok(false);
    }
    try_terminate_process(process_id);
    Ok(true)
}

fn should_close_name_matched_process(
    exe: &Path,
    queried: Result<PathBuf, String>,
) -> Result<bool, String> {
    match queried {
        Ok(path) => Ok(windows_paths_equal(exe, &path)),
        Err(error) if process_already_gone(&error) => Ok(false),
        // 路径查询被拒仍按文件名杀，不能把查询失败当成 StopGame 致命错误。
        Err(_) => Ok(true),
    }
}

fn try_terminate_process(process_id: u32) {
    enable_debug_privilege();
    let Ok(Some(process)) = open_live_process(process_id, PROCESS_TERMINATE, "打开待结束进程失败")
    else {
        return;
    };
    let _ = terminate_process(&process, process_id);
}

fn enable_debug_privilege() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = enable_se_debug_privilege();
    });
}

fn enable_se_debug_privilege() -> Result<(), String> {
    let mut token = std::ptr::null_mut();
    capture_win32_result(
        || unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
                &mut token,
            )
        },
        |result| *result == 0,
        std::io::Error::last_os_error,
    )
    .map_err(|error| format_win32_error("打开进程令牌失败", error))?;
    let token = OwnedHandle::from_valid(token);
    let mut luid = LUID::default();
    let name = wide_null("SeDebugPrivilege");
    capture_win32_result(
        || unsafe { LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut luid) },
        |result| *result == 0,
        std::io::Error::last_os_error,
    )
    .map_err(|error| format_win32_error("查找 SeDebugPrivilege 失败", error))?;
    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    capture_win32_result(
        || unsafe {
            AdjustTokenPrivileges(
                token.raw(),
                0,
                &privileges,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        |result| *result == 0,
        std::io::Error::last_os_error,
    )
    .map_err(|error| format_win32_error("启用 SeDebugPrivilege 失败", error))?;
    Ok(())
}

fn wide_null(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn canonicalize_executable_path(exe: &Path, action: &str) -> Result<PathBuf, String> {
    std::fs::canonicalize(exe).map_err(|error| format!("{action}: {error}"))
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

fn process_tree_ids(root_ids: &[u32], entries: &[(u32, u32)]) -> Vec<u32> {
    let mut ids = root_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    loop {
        let before = ids.len();
        for &(pid, parent_pid) in entries {
            if ids.binary_search(&parent_pid).is_ok() && ids.binary_search(&pid).is_err() {
                ids.push(pid);
                ids.sort_unstable();
            }
        }
        if ids.len() == before {
            return ids;
        }
    }
}

fn process_tree_contains(root_ids: &[u32], entries: &[(u32, u32)], pid: u32) -> bool {
    process_tree_ids(root_ids, entries)
        .binary_search(&pid)
        .is_ok()
}

fn process_tree_ids_for_executable(exe: &Path) -> Result<Vec<u32>, String> {
    let entries = scan_process_entries()?;
    let root_ids = exact_root_ids(exe, &entries)?;
    let relationships = process_relationships(&entries);
    Ok(process_tree_ids(&root_ids, &relationships))
}

fn process_tree_contains_for_executable(exe: &Path, pid: u32) -> Result<bool, String> {
    let entries = scan_process_entries()?;
    let root_ids = exact_root_ids(exe, &entries)?;
    let relationships = process_relationships(&entries);
    Ok(process_tree_contains(&root_ids, &relationships, pid))
}

fn exact_root_ids(exe: &Path, entries: &[ProcessEntry]) -> Result<Vec<u32>, String> {
    let target_name = exe
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("exe 路径缺少文件名: {}", exe.display()))?;
    let mut root_ids = entries
        .iter()
        .filter(|entry| windows_names_equal(target_name, &entry.executable_name))
        .filter_map(|entry| {
            query_process_path(entry.pid)
                .ok()
                .filter(|path| windows_paths_equal(exe, path))
                .map(|_| entry.pid)
        })
        .collect::<Vec<_>>();
    root_ids.sort_unstable();
    root_ids.dedup();
    Ok(root_ids)
}

fn process_relationships(entries: &[ProcessEntry]) -> Vec<(u32, u32)> {
    entries
        .iter()
        .map(|entry| (entry.pid, entry.parent_pid))
        .collect()
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
    scan_process_entries_by_name(exe)?
        .into_iter()
        .map(|(pid, parent_pid)| {
            Ok(ProcessCandidate {
                pid,
                parent_pid,
                full_path: query_process_path(pid)?,
            })
        })
        .collect()
}

fn scan_process_entries_by_name(exe: &Path) -> Result<Vec<(u32, u32)>, String> {
    let target_name = exe
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("exe 路径缺少文件名: {}", exe.display()))?;
    Ok(scan_process_entries()?
        .into_iter()
        .filter(|entry| windows_names_equal(target_name, &entry.executable_name))
        .map(|entry| (entry.pid, entry.parent_pid))
        .collect())
}

fn scan_process_entries() -> Result<Vec<ProcessEntry>, String> {
    // SAFETY: arguments follow CreateToolhelp32Snapshot contract; returned handle is owned.
    let snapshot = capture_win32_result(
        || unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) },
        |handle| handle.is_null() || *handle == INVALID_HANDLE_VALUE,
        std::io::Error::last_os_error,
    )
    .map(OwnedHandle::from_valid)
    .map_err(|error| format_win32_error("创建进程快照失败", error))?;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: snapshot is valid and entry points to initialized writable storage.
    if let Err(error) = capture_win32_result(
        || unsafe { Process32FirstW(snapshot.raw(), &mut entry) },
        |result| *result == 0,
        std::io::Error::last_os_error,
    ) {
        return if error.raw_os_error()
            == Some(windows_sys::Win32::Foundation::ERROR_NO_MORE_FILES as i32)
        {
            Ok(Vec::new())
        } else {
            Err(format_win32_error("读取进程快照失败", error))
        };
    }

    let mut entries = Vec::new();
    loop {
        entries.push(ProcessEntry {
            pid: entry.th32ProcessID,
            parent_pid: entry.th32ParentProcessID,
            executable_name: entry.szExeFile.to_vec(),
        });
        // SAFETY: snapshot and entry remain valid for the enumeration lifetime.
        if let Err(error) = capture_win32_result(
            || unsafe { Process32NextW(snapshot.raw(), &mut entry) },
            |result| *result == 0,
            std::io::Error::last_os_error,
        ) {
            if error.raw_os_error()
                == Some(windows_sys::Win32::Foundation::ERROR_NO_MORE_FILES as i32)
            {
                break;
            }
            return Err(format_win32_error("继续读取进程快照失败", error));
        }
    }
    Ok(entries)
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
    query_process_path_from_handle(&process, process_id)
}

fn query_process_path_from_handle(
    process: &OwnedHandle,
    process_id: u32,
) -> Result<PathBuf, String> {
    let mut path = vec![0_u16; PROCESS_PATH_BUFFER_LEN];
    let mut path_len = path.len() as u32;
    // SAFETY: process is valid; path buffer and length pointer are writable and correctly sized.
    capture_win32_result(
        || unsafe {
            QueryFullProcessImageNameW(process.raw(), 0, path.as_mut_ptr(), &mut path_len)
        },
        |result| *result == 0,
        std::io::Error::last_os_error,
    )
    .map_err(|error| {
        format_win32_error(&format!("查询进程完整路径失败: PID {process_id}"), error)
    })?;
    path.truncate(path_len as usize);
    canonicalize_executable_path(
        &PathBuf::from(OsString::from_wide(&path)),
        &format!("规范化进程完整路径失败: PID {process_id}"),
    )
}

fn open_process(process_id: u32, access: u32, action: &str) -> Result<OwnedHandle, String> {
    // SAFETY: process ID and access mask are plain values; returned handle is owned.
    capture_win32_result(
        || unsafe { OpenProcess(access, 0, process_id) },
        |handle| handle.is_null() || *handle == INVALID_HANDLE_VALUE,
        std::io::Error::last_os_error,
    )
    .map(OwnedHandle::from_valid)
    .map_err(|error| format_win32_error(&format!("{action}: PID {process_id}"), error))
}

fn terminate_until_no_exact_matches(
    mut check_budget: impl FnMut() -> Result<(), String>,
    mut scan: impl FnMut() -> Result<Vec<u32>, String>,
    mut terminate: impl FnMut(u32) -> Result<bool, String>,
) -> Result<(), String> {
    let mut signaled = Vec::new();
    loop {
        let candidates = scan()?;
        let mut exact_match = false;
        for process_id in candidates {
            check_budget()?;
            if signaled.contains(&process_id) {
                exact_match = true;
                continue;
            }
            if terminate(process_id)? {
                signaled.push(process_id);
                exact_match = true;
            }
        }
        if !exact_match {
            return Ok(());
        }
        std::thread::sleep(TERMINATE_RESCAN_INTERVAL);
    }
}

#[cfg(test)]
fn verify_terminate_and_wait_with_handle<H>(
    exe: &Path,
    process_id: u32,
    mut remaining_budget: impl FnMut() -> Result<Duration, String>,
    open_process: impl FnOnce(u32) -> Result<Option<H>, String>,
    query_path: impl FnOnce(&H) -> Result<PathBuf, String>,
    terminate: impl FnOnce(&H) -> Result<(), String>,
    wait: impl FnOnce(&H, Duration) -> Result<(), String>,
) -> Result<bool, String> {
    remaining_budget()?;
    let Some(process) = open_process(process_id)? else {
        return Ok(false);
    };
    if !windows_paths_equal(exe, &query_path(&process)?) {
        return Ok(false);
    }
    remaining_budget()?;
    terminate(&process)?;
    wait(&process, remaining_budget()?)?;
    Ok(true)
}

fn terminate_process(process: &OwnedHandle, process_id: u32) -> Result<(), String> {
    // SAFETY: process handle grants terminate access.
    match capture_win32_result(
        || unsafe { TerminateProcess(process.raw(), 1) },
        |result| *result == 0,
        std::io::Error::last_os_error,
    ) {
        Ok(_) => Ok(()),
        Err(error) => {
            let formatted = format_win32_error(&format!("结束进程失败: PID {process_id}"), error);
            if access_denied(&formatted) {
                Ok(())
            } else {
                Err(formatted)
            }
        }
    }
}

fn remaining_until(deadline: Instant) -> Result<Duration, String> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| "结束进程预算耗尽".to_string())
}

fn process_already_gone(error: &str) -> bool {
    error.contains(&format!(
        "（Windows 错误 {}）",
        windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER
    ))
}

fn access_denied(error: &str) -> bool {
    error.contains(&format!(
        "（Windows 错误 {}）",
        windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED
    ))
}

fn open_live_process(
    process_id: u32,
    access: u32,
    action: &str,
) -> Result<Option<OwnedHandle>, String> {
    match open_process(process_id, access, action) {
        Ok(handle) => Ok(Some(handle)),
        Err(error) if process_already_gone(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

#[derive(Default)]
struct EnumWindowsContext {
    candidates: Vec<WindowCandidate>,
}

fn enumerate_windows() -> Result<Vec<WindowCandidate>, String> {
    let mut context = EnumWindowsContext::default();
    // SAFETY: EnumWindows invokes the callback synchronously while context remains alive.
    capture_win32_result(
        || unsafe {
            EnumWindows(
                Some(collect_window),
                (&mut context as *mut EnumWindowsContext) as isize,
            )
        },
        |result| *result == 0,
        std::io::Error::last_os_error,
    )
    .map_err(|error| format_win32_error("枚举顶层窗口失败", error))?;
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

fn validate_window_identity(window: WindowIdentity) -> Result<HWND, String> {
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
    Ok(hwnd)
}

fn restore_and_focus_verified(hwnd: HWND) -> Result<(), String> {
    // SAFETY: caller validated hwnd and its process ownership immediately before this call.
    if unsafe { IsIconic(hwnd) } != 0 {
        capture_win32_result(
            || unsafe { ShowWindowAsync(hwnd, SW_RESTORE) },
            |result| *result == 0,
            std::io::Error::last_os_error,
        )
        .map_err(|error| format_win32_error("恢复目标窗口失败", error))?;
    }
    focus_with_foreground_unlock(
        hwnd,
        |target| {
            // SAFETY: caller validated hwnd and its process ownership immediately before this call.
            unsafe { SetForegroundWindow(target) != 0 }
        },
        || {
            // SAFETY: GetForegroundWindow has no preconditions.
            unsafe { GetForegroundWindow() }
        },
        tap_alt_for_foreground_permission,
    )
}

fn focus_with_foreground_unlock(
    hwnd: HWND,
    mut focus: impl FnMut(HWND) -> bool,
    mut foreground: impl FnMut() -> HWND,
    mut unlock: impl FnMut(),
) -> Result<(), String> {
    if foreground() == hwnd || (focus(hwnd) && wait_for_foreground(hwnd, &mut foreground)) {
        return Ok(());
    }
    unlock();
    if !focus(hwnd) {
        return Err("聚焦目标窗口失败".to_string());
    }
    if !wait_for_foreground(hwnd, &mut foreground) {
        return Err("目标窗口未成为前台窗口".to_string());
    }
    Ok(())
}

fn wait_for_foreground(hwnd: HWND, foreground: &mut impl FnMut() -> HWND) -> bool {
    let deadline = Instant::now() + FOREGROUND_SETTLE_TIMEOUT;
    loop {
        if foreground() == hwnd {
            return true;
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return false;
        };
        std::thread::sleep(FOREGROUND_SETTLE_POLL.min(remaining));
    }
}

fn tap_alt_for_foreground_permission() {
    // SAFETY: 注入一次成对的 Alt 按下/释放，用于解除 Windows 前台切换限制，不保留按键状态。
    unsafe {
        keybd_event(VK_MENU as u8, 0, 0, 0);
        keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

fn capture_win32_result<T>(
    call: impl FnOnce() -> T,
    failed: impl FnOnce(&T) -> bool,
    read_error: impl FnOnce() -> std::io::Error,
) -> Result<T, std::io::Error> {
    let result = call();
    if failed(&result) {
        return Err(read_error());
    }
    Ok(result)
}

fn format_win32_error(action: &str, error: std::io::Error) -> String {
    error.raw_os_error().map_or_else(
        || action.to_string(),
        |code| format!("{action}（Windows 错误 {code}）"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::thread;

    #[derive(Debug)]
    struct FakeProcessHandle(u64);

    struct ChildGuard(Child);

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    fn spawn_helper(exe: &Path) -> ChildGuard {
        ChildGuard(
            Command::new(exe)
                .args([
                    "--ignored",
                    "--exact",
                    "special_ops::desktop_runtime::tests::desktop_runtime_helper_process",
                    "--test-threads=1",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }

    fn wait_for_helper_path(process_id: u32, expected: &Path) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if query_process_path(process_id)
                .is_ok_and(|actual| windows_paths_equal(expected, &actual))
            {
                return;
            }
            assert!(Instant::now() < deadline, "helper 进程未就绪");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn failed_raw_call_captures_error_before_formatting() {
        let phase = Cell::new(0);
        let error = capture_win32_result(
            || {
                assert_eq!(phase.get(), 0);
                phase.set(1);
                0_i32
            },
            |result| *result == 0,
            || {
                assert_eq!(phase.get(), 1);
                phase.set(2);
                std::io::Error::from_raw_os_error(5)
            },
        )
        .unwrap_err();

        assert_eq!(phase.get(), 2);
        assert_eq!(
            format_win32_error("原生调用失败", error),
            "原生调用失败（Windows 错误 5）"
        );
    }

    #[test]
    fn foreground_lock_triggers_alt_unlock_then_retries_focus() {
        let attempts = Cell::new(0);
        let foreground = Cell::new(0_isize as HWND);
        let events = RefCell::new(Vec::new());
        let target = 42_isize as HWND;

        focus_with_foreground_unlock(
            target,
            |_| {
                let attempt = attempts.get() + 1;
                attempts.set(attempt);
                events.borrow_mut().push(format!("focus:{attempt}"));
                if attempt == 2 {
                    foreground.set(target);
                    true
                } else {
                    false
                }
            },
            || foreground.get(),
            || events.borrow_mut().push("unlock".to_string()),
        )
        .unwrap();

        assert_eq!(events.into_inner(), vec!["focus:1", "unlock", "focus:2"]);
    }

    #[test]
    fn successful_focus_waits_for_foreground_transition() {
        let foreground_reads = Cell::new(0);
        let target = 42_isize as HWND;

        focus_with_foreground_unlock(
            target,
            |_| true,
            || {
                let read = foreground_reads.get() + 1;
                foreground_reads.set(read);
                if read >= 4 {
                    target
                } else {
                    std::ptr::null_mut()
                }
            },
            || {},
        )
        .unwrap();

        assert!(foreground_reads.get() >= 4);
    }

    #[test]
    #[ignore = "仅作为受控 helper 子进程启动"]
    fn desktop_runtime_helper_process() {
        loop {
            thread::park_timeout(Duration::from_secs(60));
        }
    }

    #[test]
    fn second_kill_clears_remaining_process() {
        let scans = RefCell::new(vec![vec![10], vec![10], vec![]]);
        let kills = RefCell::new(Vec::new());

        terminate_until_gone(
            || Ok(scans.borrow_mut().remove(0)),
            |pid| {
                kills.borrow_mut().push(pid);
                Ok(true)
            },
            2,
            Duration::ZERO,
        )
        .unwrap();

        assert_eq!(kills.into_inner(), vec![10, 10]);
    }

    #[test]
    fn still_alive_after_two_kills_is_error() {
        let scans = RefCell::new(vec![vec![10], vec![10], vec![10]]);
        let kills = RefCell::new(Vec::new());

        let error = terminate_until_gone(
            || Ok(scans.borrow_mut().remove(0)),
            |pid| {
                kills.borrow_mut().push(pid);
                Ok(true)
            },
            2,
            Duration::ZERO,
        )
        .unwrap_err();

        assert_eq!(error, "目标进程仍在运行");
        assert_eq!(kills.into_inner(), vec![10, 10]);
    }

    #[test]
    fn window_gone_counts_as_closed_even_if_process_lingers() {
        assert!(close_succeeded(true, true));
        assert!(close_succeeded(true, false));
        assert!(close_succeeded(false, false));
        assert!(!close_succeeded(false, true));
    }

    #[test]
    fn missing_process_does_not_kill() {
        let kills = RefCell::new(Vec::new());

        terminate_until_gone(
            || Ok(Vec::new()),
            |pid| {
                kills.borrow_mut().push(pid);
                Ok(true)
            },
            2,
            Duration::ZERO,
        )
        .unwrap();

        assert!(kills.into_inner().is_empty());
    }

    #[test]
    fn real_process_path_query_matches_current_executable() {
        let expected = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        let actual = query_process_path(std::process::id()).unwrap();
        assert!(windows_paths_equal(&expected, &actual));
    }

    #[test]
    #[ignore = "需要交互式 Windows 桌面，沙箱和 CI 无可枚举窗口"]
    fn real_enum_windows_call_succeeds() {
        enumerate_windows().unwrap();
    }

    #[test]
    fn real_terminate_exact_preserves_same_basename_in_other_directory() {
        let temp = tempfile::tempdir().unwrap();
        let source = std::env::current_exe().unwrap();
        let target_dir = temp.path().join("target");
        let other_dir = temp.path().join("other");
        fs::create_dir_all(&target_dir).unwrap();
        fs::create_dir_all(&other_dir).unwrap();
        let target_exe = target_dir.join("runtime-helper.exe");
        let other_exe = other_dir.join("runtime-helper.exe");
        fs::copy(&source, &target_exe).unwrap();
        fs::copy(&source, &other_exe).unwrap();
        let target_exe = fs::canonicalize(target_exe).unwrap();
        let other_exe = fs::canonicalize(other_exe).unwrap();
        let mut target = spawn_helper(&target_exe);
        let mut other = spawn_helper(&other_exe);
        wait_for_helper_path(target.0.id(), &target_exe);
        wait_for_helper_path(other.0.id(), &other_exe);

        WindowsDesktopRuntime
            .terminate_exact(&target_exe, Duration::from_secs(5))
            .unwrap();

        assert!(target.0.try_wait().unwrap().is_some());
        assert!(other.0.try_wait().unwrap().is_none());
        assert!(query_process_path(other.0.id())
            .is_ok_and(|actual| windows_paths_equal(&other_exe, &actual)));
    }

    #[test]
    fn real_terminate_exact_resolves_parent_segments_before_matching() {
        let temp = tempfile::tempdir().unwrap();
        let source = std::env::current_exe().unwrap();
        let target_dir = temp.path().join("target");
        let other_dir = temp.path().join("other");
        let nested_dir = target_dir.join("nested");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::create_dir_all(&other_dir).unwrap();
        let target_exe = target_dir.join("runtime-helper.exe");
        let other_exe = other_dir.join("runtime-helper.exe");
        fs::copy(&source, &target_exe).unwrap();
        fs::copy(&source, &other_exe).unwrap();
        let target_exe = fs::canonicalize(target_exe).unwrap();
        let other_exe = fs::canonicalize(other_exe).unwrap();
        let target_with_parent_segment = nested_dir.join("..").join("runtime-helper.exe");
        let mut target = spawn_helper(&target_exe);
        let mut other = spawn_helper(&other_exe);
        wait_for_helper_path(target.0.id(), &target_exe);
        wait_for_helper_path(other.0.id(), &other_exe);

        WindowsDesktopRuntime
            .terminate_exact(&target_with_parent_segment, Duration::from_secs(5))
            .unwrap();

        assert!(target.0.try_wait().unwrap().is_some());
        assert!(other.0.try_wait().unwrap().is_none());
        assert!(query_process_path(other.0.id())
            .is_ok_and(|actual| windows_paths_equal(&other_exe, &actual)));
    }

    #[test]
    fn first_kill_round_clears_all_matches() {
        let remaining = RefCell::new(vec![vec![10, 20], vec![]]);
        let terminated = RefCell::new(Vec::new());

        terminate_until_gone(
            || Ok(remaining.borrow_mut().remove(0)),
            |process_id| {
                terminated.borrow_mut().push(process_id);
                Ok(true)
            },
            2,
            Duration::ZERO,
        )
        .unwrap();

        assert_eq!(*terminated.borrow(), vec![10, 20]);
    }

    #[test]
    fn different_path_basename_finishes_after_one_scan() {
        let scan_count = Cell::new(0);

        terminate_until_no_exact_matches(
            || Ok(()),
            || {
                scan_count.set(scan_count.get() + 1);
                Ok(vec![10])
            },
            |_| Ok(false),
        )
        .unwrap();

        assert_eq!(scan_count.get(), 1);
    }

    #[test]
    fn exact_match_causes_another_scan() {
        let scan_count = Cell::new(0);

        terminate_until_no_exact_matches(
            || Ok(()),
            || {
                let scan = scan_count.get() + 1;
                scan_count.set(scan);
                Ok(vec![if scan == 1 { 10 } else { 20 }])
            },
            |process_id| Ok(process_id == 10),
        )
        .unwrap();

        assert_eq!(scan_count.get(), 2);
    }

    #[test]
    fn already_signaled_pid_is_not_terminated_again() {
        let terminate_count = Cell::new(0);
        let scan_count = Cell::new(0);

        terminate_until_no_exact_matches(
            || Ok(()),
            || {
                let scan = scan_count.get() + 1;
                scan_count.set(scan);
                Ok(if scan < 3 { vec![10] } else { Vec::new() })
            },
            |_| {
                terminate_count.set(terminate_count.get() + 1);
                Ok(true)
            },
        )
        .unwrap();

        assert_eq!(terminate_count.get(), 1);
        assert_eq!(scan_count.get(), 3);
    }

    #[test]
    fn empty_scan_succeeds_even_when_budget_already_exhausted() {
        terminate_until_no_exact_matches(
            || Err("结束进程预算耗尽".to_string()),
            || Ok(Vec::new()),
            |_| panic!("空快照不应再结束进程"),
        )
        .unwrap();
    }

    #[test]
    fn budget_exhausted_after_kill_still_succeeds_when_rescan_empty() {
        let scan_count = Cell::new(0);
        let budget_checks = Cell::new(0);

        terminate_until_no_exact_matches(
            || {
                let check = budget_checks.get();
                budget_checks.set(check + 1);
                if check == 0 {
                    Ok(())
                } else {
                    Err("结束进程预算耗尽".to_string())
                }
            },
            || {
                let scan = scan_count.get();
                scan_count.set(scan + 1);
                Ok(if scan == 0 { vec![10] } else { Vec::new() })
            },
            |_| Ok(true),
        )
        .unwrap();

        assert_eq!(scan_count.get(), 2);
    }

    #[test]
    fn vanished_pid_is_not_an_exact_match() {
        let terminated = Cell::new(false);

        let matched = verify_terminate_and_wait_with_handle(
            Path::new(r"C:\Games\DeltaForce.exe"),
            10,
            || Ok(Duration::from_millis(100)),
            |_| Ok(None::<FakeProcessHandle>),
            |_| panic!("消失 PID 不应查询路径"),
            |_| {
                terminated.set(true);
                Ok(())
            },
            |_, _| Ok(()),
        )
        .unwrap();

        assert!(!matched);
        assert!(!terminated.get());
    }

    #[test]
    fn process_gone_error_only_matches_invalid_parameter() {
        assert!(process_already_gone(&format!(
            "打开待结束进程失败: PID 10（Windows 错误 {}）",
            windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER
        )));
        assert!(!process_already_gone(
            "打开待结束进程失败: PID 10（Windows 错误 5）"
        ));
        assert!(!process_already_gone("结束进程预算耗尽: PID 10"));
    }

    #[test]
    fn access_denied_matches_windows_error_five() {
        assert!(access_denied("结束进程失败: PID 10（Windows 错误 5）"));
        assert!(access_denied(
            "打开进程查询路径失败: PID 10（Windows 错误 5）"
        ));
        assert!(!access_denied(&format!(
            "打开待结束进程失败: PID 10（Windows 错误 {}）",
            windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER
        )));
    }

    #[test]
    fn access_denied_name_match_still_closes() {
        let exe = Path::new(r"C:\Games\DeltaForce.exe");
        assert!(should_close_name_matched_process(
            exe,
            Err("打开进程查询路径失败: PID 10（Windows 错误 5）".to_string()),
        )
        .unwrap());
        assert!(!should_close_name_matched_process(
            exe,
            Ok(PathBuf::from(r"D:\Other\DeltaForce.exe")),
        )
        .unwrap());
        assert!(!should_close_name_matched_process(
            exe,
            Err(format!(
                "打开进程查询路径失败: PID 10（Windows 错误 {}）",
                windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER
            )),
        )
        .unwrap());
    }

    #[test]
    fn ace_gen_failure_query_still_closes_by_name() {
        let exe = Path::new(r"C:\Games\DeltaForce.exe");
        assert!(should_close_name_matched_process(
            exe,
            Err(format!(
                "打开进程查询路径失败: PID 10（Windows 错误 {}）",
                windows_sys::Win32::Foundation::ERROR_GEN_FAILURE
            )),
        )
        .unwrap());
        assert!(should_close_name_matched_process(
            exe,
            Err("查询进程完整路径失败: PID 10（Windows 错误 31）".to_string()),
        )
        .unwrap());
    }

    #[test]
    fn exhausted_budget_after_query_does_not_terminate() {
        let budget_checks = Cell::new(0);
        let terminated = Cell::new(false);

        let error = verify_terminate_and_wait_with_handle(
            Path::new(r"C:\Apps\WeGame\WeGame.exe"),
            10,
            || {
                let check = budget_checks.get();
                budget_checks.set(check + 1);
                if check == 0 {
                    Ok(Duration::from_millis(100))
                } else {
                    Err("结束进程预算耗尽: PID 10".to_string())
                }
            },
            |_| Ok(Some(FakeProcessHandle(41))),
            |_| Ok(PathBuf::from(r"C:\Apps\WeGame\WeGame.exe")),
            |_| {
                terminated.set(true);
                Ok(())
            },
            |_, _| Ok(()),
        )
        .unwrap_err();

        assert_eq!(error, "结束进程预算耗尽: PID 10");
        assert_eq!(budget_checks.get(), 2);
        assert!(!terminated.get());
    }

    #[test]
    fn wait_receives_fresh_budget_after_query_and_termination() {
        let budget_checks = Cell::new(0);
        let wait_budget = Cell::new(Duration::ZERO);

        let matched = verify_terminate_and_wait_with_handle(
            Path::new(r"C:\Apps\WeGame\WeGame.exe"),
            10,
            || {
                let check = budget_checks.get();
                budget_checks.set(check + 1);
                Ok(match check {
                    0 => Duration::from_millis(100),
                    1 => Duration::from_millis(60),
                    _ => Duration::from_millis(25),
                })
            },
            |_| Ok(Some(FakeProcessHandle(41))),
            |handle| {
                assert_eq!(handle.0, 41);
                Ok(PathBuf::from(r"C:\Apps\WeGame\WeGame.exe"))
            },
            |handle| {
                assert_eq!(handle.0, 41);
                Ok(())
            },
            |handle, remaining| {
                assert_eq!(handle.0, 41);
                wait_budget.set(remaining);
                Ok(())
            },
        )
        .unwrap();

        assert!(matched);
        assert_eq!(budget_checks.get(), 3);
        assert_eq!(wait_budget.get(), Duration::from_millis(25));
    }

    #[test]
    fn missing_parent_error_does_not_echo_executable_path() {
        let runtime = WindowsDesktopRuntime;
        let missing_parent = Path::new("secret-account.exe");
        let error = runtime.launch(missing_parent).unwrap_err();
        assert!(!error.contains("secret-account.exe"), "{error}");
    }

    #[test]
    fn launch_failure_does_not_echo_executable_path() {
        let runtime = WindowsDesktopRuntime;
        let missing_executable = Path::new(r"C:\Users\secret-account\missing-wegame.exe");
        let error = runtime.launch(missing_executable).unwrap_err();
        assert!(!error.contains("C:\\Users\\secret-account"), "{error}");
    }

    #[test]
    fn path_mismatch_on_opened_handle_is_not_terminated() {
        let open_count = Cell::new(0);
        let terminated = Cell::new(false);

        let matched = verify_terminate_and_wait_with_handle(
            Path::new(r"C:\Apps\WeGame\WeGame.exe"),
            10,
            || Ok(Duration::from_secs(1)),
            |_| {
                open_count.set(open_count.get() + 1);
                Ok(Some(FakeProcessHandle(41)))
            },
            |handle| {
                assert_eq!(handle.0, 41);
                Ok(PathBuf::from(r"D:\Other\WeGame.exe"))
            },
            |_| {
                terminated.set(true);
                Ok(())
            },
            |_, _| Ok(()),
        )
        .unwrap();

        assert!(!matched);
        assert_eq!(open_count.get(), 1);
        assert!(!terminated.get());
    }

    #[test]
    fn matching_path_is_terminated_through_the_same_opened_handle() {
        let open_count = Cell::new(0);
        let queried_identity = Cell::new(0);
        let terminated_identity = Cell::new(0);
        let waited_identity = Cell::new(0);

        let matched = verify_terminate_and_wait_with_handle(
            Path::new(r"C:\Apps\WeGame\WeGame.exe"),
            10,
            || Ok(Duration::from_secs(1)),
            |_| {
                open_count.set(open_count.get() + 1);
                Ok(Some(FakeProcessHandle(41)))
            },
            |handle| {
                queried_identity.set(handle.0);
                Ok(PathBuf::from(r"\\?\c:\apps\wegame\WEGAME.EXE"))
            },
            |handle| {
                terminated_identity.set(handle.0);
                Ok(())
            },
            |handle, _| {
                waited_identity.set(handle.0);
                Ok(())
            },
        )
        .unwrap();

        assert!(matched);
        assert_eq!(open_count.get(), 1);
        assert_eq!(queried_identity.get(), 41);
        assert_eq!(terminated_identity.get(), 41);
        assert_eq!(waited_identity.get(), 41);
    }

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
    fn process_tree_includes_direct_and_nested_descendants_only() {
        let entries = vec![(10, 0), (11, 10), (12, 11), (20, 0), (21, 20)];

        assert_eq!(process_tree_ids(&[10], &entries), vec![10, 11, 12]);
    }

    #[test]
    fn process_tree_supports_multiple_matching_roots() {
        let entries = vec![(10, 0), (11, 10), (20, 0), (21, 20), (30, 0)];

        assert_eq!(process_tree_ids(&[10, 20], &entries), vec![10, 11, 20, 21]);
    }

    #[test]
    fn process_tree_window_selects_descendant_and_rejects_foreign_browser() {
        let entries = vec![(10, 0), (11, 10), (12, 11), (99, 0)];
        let windows = vec![
            WindowCandidate {
                hwnd: 1,
                pid: 99,
                visible: true,
                owned: false,
                area: 2_000,
            },
            WindowCandidate {
                hwnd: 2,
                pid: 12,
                visible: true,
                owned: false,
                area: 1_000,
            },
        ];

        let ids = process_tree_ids(&[10], &entries);
        assert_eq!(
            select_primary_window(&ids, &windows),
            Some(WindowIdentity {
                process_id: 12,
                handle: 2,
            })
        );
    }

    #[test]
    fn selected_pid_must_still_belong_to_current_process_tree() {
        assert!(process_tree_contains(&[10], &[(10, 0), (11, 10)], 11));
        assert!(!process_tree_contains(&[10], &[(10, 0), (11, 99)], 11));
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
