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
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| "进程结束超时范围无效".to_string())?;
        terminate_until_no_exact_matches(
            || remaining_until(deadline, None).map(drop),
            || {
                scan_process_entries_by_name(exe).map(|candidates| {
                    candidates
                        .into_iter()
                        .map(|(process_id, _)| process_id)
                        .collect()
                })
            },
            |process_id| terminate_verified_process(exe, process_id, deadline),
        )
    }

    fn launch(&self, exe: &Path) -> Result<u32, String> {
        let parent = exe
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| "程序路径缺少父目录".to_string())?;
        Command::new(exe)
            .current_dir(parent)
            .spawn()
            .map(|child| child.id())
            .map_err(|error| format!("启动程序失败: {error}"))
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
            candidates.push((entry.th32ProcessID, entry.th32ParentProcessID));
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
    query_process_path_from_handle(&process, process_id)
}

fn query_process_path_from_handle(
    process: &OwnedHandle,
    process_id: u32,
) -> Result<PathBuf, String> {
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

fn terminate_verified_process(
    exe: &Path,
    process_id: u32,
    deadline: Instant,
) -> Result<bool, String> {
    verify_terminate_and_wait_with_handle(
        exe,
        process_id,
        || remaining_until(deadline, Some(process_id)),
        |process_id| {
            open_process(
                process_id,
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                "打开待结束进程失败",
            )
        },
        |process| query_process_path_from_handle(process, process_id),
        |process| terminate_process(process, process_id),
        |process, remaining| wait_for_process(process, process_id, remaining),
    )
}

fn terminate_until_no_exact_matches(
    mut check_budget: impl FnMut() -> Result<(), String>,
    mut scan: impl FnMut() -> Result<Vec<u32>, String>,
    mut terminate: impl FnMut(u32) -> Result<bool, String>,
) -> Result<(), String> {
    loop {
        check_budget()?;
        let candidates = scan()?;
        check_budget()?;
        let mut exact_match = false;
        for process_id in candidates {
            check_budget()?;
            exact_match |= terminate(process_id)?;
        }
        if !exact_match {
            return Ok(());
        }
    }
}

fn verify_terminate_and_wait_with_handle<H>(
    exe: &Path,
    process_id: u32,
    mut remaining_budget: impl FnMut() -> Result<Duration, String>,
    open_process: impl FnOnce(u32) -> Result<H, String>,
    query_path: impl FnOnce(&H) -> Result<PathBuf, String>,
    terminate: impl FnOnce(&H) -> Result<(), String>,
    wait: impl FnOnce(&H, Duration) -> Result<(), String>,
) -> Result<bool, String> {
    remaining_budget()?;
    let process = open_process(process_id)?;
    if !windows_paths_equal(exe, &query_path(&process)?) {
        return Ok(false);
    }
    remaining_budget()?;
    terminate(&process)?;
    wait(&process, remaining_budget()?)?;
    Ok(true)
}

fn terminate_process(process: &OwnedHandle, process_id: u32) -> Result<(), String> {
    // SAFETY: process handle grants terminate and synchronize access.
    if unsafe { TerminateProcess(process.raw(), 1) } == 0 {
        return Err(last_error(&format!("结束进程失败: PID {process_id}")));
    }
    Ok(())
}

fn wait_for_process(
    process: &OwnedHandle,
    process_id: u32,
    timeout: Duration,
) -> Result<(), String> {
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

fn remaining_until(deadline: Instant, process_id: Option<u32>) -> Result<Duration, String> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| match process_id {
            Some(process_id) => format!("等待进程退出超时: PID {process_id}"),
            None => "等待进程退出超时".to_string(),
        })
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
    use std::cell::Cell;
    use std::path::{Path, PathBuf};

    #[derive(Debug)]
    struct FakeProcessHandle(u64);

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
                    Err("等待进程退出超时: PID 10".to_string())
                }
            },
            |_| Ok(FakeProcessHandle(41)),
            |_| Ok(PathBuf::from(r"C:\Apps\WeGame\WeGame.exe")),
            |_| {
                terminated.set(true);
                Ok(())
            },
            |_, _| Ok(()),
        )
        .unwrap_err();

        assert_eq!(error, "等待进程退出超时: PID 10");
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
            |_| Ok(FakeProcessHandle(41)),
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
                Ok(FakeProcessHandle(41))
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
                Ok(FakeProcessHandle(41))
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
