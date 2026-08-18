//! 息屏：原生 Win32 视觉遮罩，仿 UU 私密屏保。
//! 只挡画面，键鼠 / Alt+Tab 照常落到下面窗口；排除截图。不是 WebView。

use std::sync::{
    atomic::{AtomicBool, AtomicIsize, Ordering},
    mpsc, Mutex,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::hotkey_types::{ConflictPolicy, HotkeyAction};
use crate::hotkeys::HotkeyManager;
use crate::settings;

const SETTINGS_FILE_NAME: &str = "privacy_screen_settings.json";
const HOTKEY_SCOPE: &str = "privacy-screen";
const STATE_CHANGED_EVENT: &str = "privacy-screen://state-changed";
const CLASS_NAME: &str = "DeltaAutoToolsPrivacyCover";
const TOPMOST_TIMER_ID: usize = 1;
const TOPMOST_TIMER_MS: u32 = 250;

static COVER_HWND: AtomicIsize = AtomicIsize::new(0);
static COVER_BITMAP: AtomicIsize = AtomicIsize::new(0);
static COVER_HINT: Mutex<String> = Mutex::new(String::new());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyScreenSettings {
    #[serde(default)]
    pub close_hotkey: String,
    #[serde(default)]
    pub image_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyScreenBootstrap {
    pub settings: PrivacyScreenSettings,
    pub visible: bool,
    pub hotkey_error: Option<String>,
    pub image_data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyScreenChanged {
    pub visible: bool,
}

pub struct PrivacyScreenState {
    settings: Mutex<PrivacyScreenSettings>,
    visible: AtomicBool,
    hotkey_error: Mutex<Option<String>>,
}

impl PrivacyScreenState {
    fn new(settings: PrivacyScreenSettings) -> Self {
        Self {
            settings: Mutex::new(settings),
            visible: AtomicBool::new(false),
            hotkey_error: Mutex::new(None),
        }
    }
}

pub fn initialize(app: &AppHandle) -> Result<PrivacyScreenState, String> {
    let path = settings::settings_path(app, SETTINGS_FILE_NAME)?;
    let loaded = settings::load_settings::<PrivacyScreenSettings>(&path)?;
    Ok(PrivacyScreenState::new(loaded))
}

pub fn hide_if_visible(app: &AppHandle) {
    let _ = hide_internal(app);
}

fn require_close_hotkey(hotkey: &str) -> Result<String, String> {
    let trimmed = hotkey.trim();
    if trimmed.is_empty() {
        return Err("请先录制关闭快捷键".to_string());
    }
    Ok(trimmed.to_string())
}

fn image_data_url(path: Option<&str>) -> Option<String> {
    path.and_then(crate::recognition::watcher::read_reference_image_as_data_url)
}

fn build_bootstrap(state: &PrivacyScreenState) -> Result<PrivacyScreenBootstrap, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "息屏状态已损坏".to_string())?
        .clone();
    let hotkey_error = state
        .hotkey_error
        .lock()
        .map_err(|_| "息屏状态已损坏".to_string())?
        .clone();
    Ok(PrivacyScreenBootstrap {
        image_data_url: image_data_url(settings.image_path.as_deref()),
        settings,
        visible: state.visible.load(Ordering::SeqCst),
        hotkey_error,
    })
}

fn emit_changed(app: &AppHandle, visible: bool) {
    let _ = app.emit(STATE_CHANGED_EVENT, PrivacyScreenChanged { visible });
}

fn close_hint_label(hotkey: &str) -> String {
    format!("关闭：{hotkey}")
}

/// 主显示器工作区右下角内收，避开任务栏，不贴虚拟屏最外沿。
fn hint_plate_origin(
    virtual_x: i32,
    virtual_y: i32,
    work_right: i32,
    work_bottom: i32,
    plate_width: i32,
    plate_height: i32,
) -> (i32, i32) {
    const INSET: i32 = 28;
    (
        work_right - virtual_x - plate_width - INSET,
        work_bottom - virtual_y - plate_height - INSET,
    )
}

fn is_main_gui_thread(app: &AppHandle) -> bool {
    #[cfg(windows)]
    {
        let Some(main) = app.get_webview_window("main") else {
            return false;
        };
        let Ok(hwnd) = main.hwnd() else {
            return false;
        };
        let mut process_id = 0u32;
        // SAFETY: hwnd 来自现存 main 窗口。
        let window_tid = unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                hwnd.0 as _,
                &mut process_id,
            )
        };
        let current = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
        window_tid == current
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        true
    }
}

fn on_gui_thread<T: Send + 'static>(
    app: &AppHandle,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    if is_main_gui_thread(app) {
        return Ok(work());
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = sender.send(work());
    })
    .map_err(|error| format!("主线程调度失败: {error}"))?;
    receiver
        .recv()
        .map_err(|_| "主线程未返回息屏结果".to_string())
}

fn clear_close_hotkey(app: &AppHandle) {
    if let Some(manager) = app.try_state::<HotkeyManager>() {
        let _ = manager.replace_scope(
            HOTKEY_SCOPE,
            Vec::new(),
            "息屏关闭".to_string(),
            ConflictPolicy::Strict,
        );
    }
}

fn hide_internal(app: &AppHandle) -> Result<(), String> {
    on_gui_thread(app, destroy_cover)?;
    clear_close_hotkey(app);
    if let Some(state) = app.try_state::<PrivacyScreenState>() {
        state.visible.store(false, Ordering::SeqCst);
    }
    emit_changed(app, false);
    Ok(())
}

fn show_internal(app: &AppHandle, state: &PrivacyScreenState) -> Result<(), String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "息屏状态已损坏".to_string())?
        .clone();
    let close_hotkey = require_close_hotkey(&settings.close_hotkey)?;
    let image_path = settings.image_path.clone();
    let hint = close_hotkey.clone();
    on_gui_thread(app, move || {
        destroy_cover();
        create_cover(image_path.as_deref(), &hint)
    })??;

    let manager = app.state::<HotkeyManager>();
    let action: HotkeyAction = std::sync::Arc::new(|app| {
        if let Err(error) = hide_internal(&app) {
            crate::log_warn!(
                "privacy_screen",
                "关闭息屏失败",
                "error" => error
            );
        }
    });
    if let Err(error) = manager.replace_safety_scope(
        HOTKEY_SCOPE,
        vec![(close_hotkey, action)],
        "息屏关闭".to_string(),
        ConflictPolicy::Strict,
    ) {
        let _ = on_gui_thread(app, destroy_cover);
        *state
            .hotkey_error
            .lock()
            .map_err(|_| "息屏状态已损坏".to_string())? = Some(error.clone());
        return Err(error);
    }
    *state
        .hotkey_error
        .lock()
        .map_err(|_| "息屏状态已损坏".to_string())? = None;
    state.visible.store(true, Ordering::SeqCst);
    emit_changed(app, true);
    Ok(())
}

#[tauri::command]
pub fn privacy_screen_get_bootstrap(
    state: State<'_, PrivacyScreenState>,
) -> Result<PrivacyScreenBootstrap, String> {
    build_bootstrap(&state)
}

#[tauri::command]
pub fn privacy_screen_save_settings(
    app: AppHandle,
    state: State<'_, PrivacyScreenState>,
    settings_value: PrivacyScreenSettings,
) -> Result<PrivacyScreenBootstrap, String> {
    let path = settings::settings_path(&app, SETTINGS_FILE_NAME)?;
    settings::save_settings(&path, &settings_value)?;
    *state
        .settings
        .lock()
        .map_err(|_| "息屏状态已损坏".to_string())? = settings_value;
    build_bootstrap(&state)
}

#[tauri::command]
pub fn privacy_screen_show(
    app: AppHandle,
    state: State<'_, PrivacyScreenState>,
) -> Result<PrivacyScreenBootstrap, String> {
    show_internal(&app, &state)?;
    build_bootstrap(&state)
}

#[tauri::command]
pub fn privacy_screen_hide(app: AppHandle) -> Result<PrivacyScreenBootstrap, String> {
    hide_internal(&app)?;
    let state = app.state::<PrivacyScreenState>();
    build_bootstrap(&state)
}

#[cfg(windows)]
mod native {
    use super::{
        CLASS_NAME, COVER_BITMAP, COVER_HINT, COVER_HWND, TOPMOST_TIMER_ID, TOPMOST_TIMER_MS,
    };
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use std::sync::atomic::Ordering;
    use std::sync::OnceLock;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush, DeleteDC,
        DeleteObject, EndPaint, FillRect, GetDC, GetObjectW, GetStockObject, GetTextExtentPoint32W,
        CreatePen, InvalidateRect, ReleaseDC, RoundRect, SelectObject, SetBkMode,
        SetStretchBltMode, SetTextColor, StretchBlt, TextOutW, UpdateWindow, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, BLACK_BRUSH, DIB_RGB_COLORS, HALFTONE, HBITMAP, HDC, PAINTSTRUCT,
        PS_SOLID, SRCCOPY,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetSystemMetrics,
        KillTimer, RegisterClassExW, SetTimer, SetWindowDisplayAffinity, SetWindowPos, ShowWindow,
        CS_HREDRAW, CS_VREDRAW, HWND_TOPMOST, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN,
        SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SPI_GETWORKAREA,
        SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, SystemParametersInfoW,
        WDA_EXCLUDEFROMCAPTURE, WM_CLOSE,
        WM_DESTROY, WM_DISPLAYCHANGE, WM_ERASEBKGND, WM_PAINT, WM_SYSCOMMAND, WM_TIMER,
        WM_WINDOWPOSCHANGING, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
    };

    const SC_MINIMIZE: usize = 0xF020;
    const SC_CLOSE: usize = 0xF060;
    const SC_RESTORE: usize = 0xF120;
    const SC_MAXIMIZE: usize = 0xF030;
    const SWP_NOZORDER: u32 = 0x0004;

    #[repr(C)]
    struct Windowpos {
        hwnd: HWND,
        hwnd_insert_after: HWND,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    }

    fn wide(text: &str) -> Vec<u16> {
        OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    pub(super) fn virtual_screen_bounds() -> (i32, i32, i32, i32) {
        // SAFETY: GetSystemMetrics 读系统度量，无句柄所有权。
        unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1),
                GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1),
            )
        }
    }

    fn primary_work_area() -> RECT {
        let mut work = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: SPI_GETWORKAREA 写入主显示器工作区。
        let ok = unsafe {
            SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut RECT).cast(), 0)
        };
        if ok == 0 || work.right <= work.left || work.bottom <= work.top {
            unsafe {
                work.right = GetSystemMetrics(SM_CXSCREEN).max(1);
                work.bottom = GetSystemMetrics(SM_CYSCREEN).max(1);
            }
        }
        work
    }

    fn load_cover_bitmap(path: &str) -> Option<HBITMAP> {
        let image = image::open(path).ok()?.to_rgba8();
        let width = i32::try_from(image.width()).ok()?;
        let height = i32::try_from(image.height()).ok()?;
        if width <= 0 || height <= 0 {
            return None;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default()],
        };
        let mut bits: *mut core::ffi::c_void = ptr::null_mut();
        // SAFETY: info 指向本栈 BITMAPINFO，bits 由系统写回 DIB 指针。
        let bitmap = unsafe {
            CreateDIBSection(
                ptr::null_mut(),
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                ptr::null_mut(),
                0,
            )
        };
        if bitmap.is_null() || bits.is_null() {
            return None;
        }
        let pixel_count = image.len() / 4;
        // SAFETY: bits 指向 CreateDIBSection 分配的 width*height*4 字节。
        let dest = unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, pixel_count * 4) };
        for (src, dst) in image.chunks_exact(4).zip(dest.chunks_exact_mut(4)) {
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = src[3];
        }
        Some(bitmap)
    }

    fn paint_cover(hwnd: HWND) {
        let mut paint = PAINTSTRUCT {
            hdc: ptr::null_mut(),
            fErase: 0,
            rcPaint: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            fRestore: 0,
            fIncUpdate: 0,
            rgbReserved: [0; 32],
        };
        // SAFETY: hwnd 是本模块创建的遮罩窗口。
        let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
        if hdc.is_null() {
            return;
        }
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe {
            GetClientRect(hwnd, &mut rect);
        }
        let bitmap = COVER_BITMAP.load(Ordering::SeqCst) as HBITMAP;
        if !bitmap.is_null() {
            blit_bitmap(hdc, bitmap, rect);
        } else {
            // SAFETY: 纯色刷子用完即删。
            unsafe {
                let brush = CreateSolidBrush(0x0000_0000);
                FillRect(hdc, &rect, brush);
                DeleteObject(brush);
            }
        }
        paint_close_hint(hdc, rect);
        unsafe {
            EndPaint(hwnd, &paint);
        }
    }

    fn paint_close_hint(hdc: HDC, rect: RECT) {
        let hint = COVER_HINT
            .lock()
            .ok()
            .map(|guard| guard.clone())
            .filter(|text| !text.is_empty());
        let Some(hint) = hint else {
            return;
        };
        let text = wide(&hint);
        let font_name = wide("Microsoft YaHei UI");
        let chars = text.len() as i32 - 1;
        // SAFETY: 仅用于本窗口绘制，字体用完即删。
        unsafe {
            let font = CreateFontW(
                -20,
                0,
                0,
                0,
                600,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                0,
                font_name.as_ptr(),
            );
            let previous = if font.is_null() {
                std::ptr::null_mut()
            } else {
                SelectObject(hdc, font)
            };
            SetBkMode(hdc, 1);
            let mut size = SIZE { cx: 0, cy: 0 };
            GetTextExtentPoint32W(hdc, text.as_ptr(), chars, &mut size);
            let pad_x = 14;
            let pad_y = 8;
            let plate_w = size.cx + pad_x * 2;
            let plate_h = size.cy + pad_y * 2;
            let (virtual_x, virtual_y, _, _) = virtual_screen_bounds();
            let work = primary_work_area();
            let (origin_x, origin_y) = super::hint_plate_origin(
                virtual_x,
                virtual_y,
                work.right,
                work.bottom,
                plate_w,
                plate_h,
            );
            let x = origin_x.clamp(rect.left + 8, (rect.right - plate_w - 8).max(rect.left + 8));
            let y = origin_y.clamp(rect.top + 8, (rect.bottom - plate_h - 8).max(rect.top + 8));
            let pen = CreatePen(PS_SOLID, 1, 0x0000_88C8);
            let brush = CreateSolidBrush(0x0000_A0E8);
            let previous_pen = SelectObject(hdc, pen);
            let previous_brush = SelectObject(hdc, brush);
            RoundRect(hdc, x, y, x + plate_w, y + plate_h, 16, 16);
            SelectObject(hdc, previous_pen);
            SelectObject(hdc, previous_brush);
            DeleteObject(pen);
            DeleteObject(brush);
            SetTextColor(hdc, 0x0014_1414);
            TextOutW(
                hdc,
                x + pad_x,
                y + pad_y,
                text.as_ptr(),
                chars,
            );
            if !font.is_null() {
                SelectObject(hdc, previous);
                DeleteObject(font);
            }
        }
    }

    fn blit_bitmap(hdc: HDC, bitmap: HBITMAP, rect: RECT) {
        let mut header = BITMAP {
            bmType: 0,
            bmWidth: 0,
            bmHeight: 0,
            bmWidthBytes: 0,
            bmPlanes: 0,
            bmBitsPixel: 0,
            bmBits: ptr::null_mut(),
        };
        // SAFETY: bitmap 由 load_cover_bitmap 创建，GetObjectW 写 BITMAP。
        let ok = unsafe {
            GetObjectW(
                bitmap,
                std::mem::size_of::<BITMAP>() as i32,
                (&mut header as *mut BITMAP).cast(),
            )
        };
        if ok == 0 || header.bmWidth <= 0 || header.bmHeight <= 0 {
            unsafe {
                let brush = CreateSolidBrush(0x0000_0000);
                FillRect(hdc, &rect, brush);
                DeleteObject(brush);
            }
            return;
        }
        unsafe {
            let memory = CreateCompatibleDC(hdc);
            if memory.is_null() {
                return;
            }
            let previous = SelectObject(memory, bitmap);
            SetStretchBltMode(hdc, HALFTONE);
            StretchBlt(
                hdc,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                memory,
                0,
                0,
                header.bmWidth,
                header.bmHeight,
                SRCCOPY,
            );
            SelectObject(memory, previous);
            DeleteDC(memory);
        }
    }

    fn keep_topmost(hwnd: HWND) {
        let (x, y, width, height) = virtual_screen_bounds();
        // SAFETY: hwnd 仍有效时把遮罩钉回虚拟屏顶层。
        unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x,
                y,
                width,
                height,
                SWP_SHOWWINDOW | SWP_NOACTIVATE,
            );
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        message: u32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        match message {
            WM_PAINT => {
                paint_cover(hwnd);
                0
            }
            WM_ERASEBKGND => 1,
            WM_TIMER if w_param == TOPMOST_TIMER_ID => {
                keep_topmost(hwnd);
                0
            }
            WM_DISPLAYCHANGE => {
                keep_topmost(hwnd);
                0
            }
            WM_WINDOWPOSCHANGING => {
                if l_param != 0 {
                    let pos = l_param as *mut Windowpos;
                    // SAFETY: l_param 是系统传入的 WINDOWPOS。
                    unsafe {
                        (*pos).hwnd_insert_after = HWND_TOPMOST;
                        (*pos).flags &= !SWP_NOZORDER;
                    }
                }
                unsafe { DefWindowProcW(hwnd, message, w_param, l_param) }
            }
            WM_SYSCOMMAND => {
                let command = w_param & 0xFFF0;
                if matches!(command, SC_MINIMIZE | SC_CLOSE | SC_RESTORE | SC_MAXIMIZE) {
                    0
                } else {
                    unsafe { DefWindowProcW(hwnd, message, w_param, l_param) }
                }
            }
            WM_CLOSE => {
                unsafe {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_DESTROY => {
                COVER_HWND.store(0, Ordering::SeqCst);
                if let Ok(mut hint) = COVER_HINT.lock() {
                    hint.clear();
                }
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, message, w_param, l_param) },
        }
    }

    fn register_class() -> Result<(), String> {
        static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
        REGISTERED
            .get_or_init(|| {
                let class_name = wide(CLASS_NAME);
                let class = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: Some(wnd_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: unsafe { GetModuleHandleW(ptr::null()) },
                    hIcon: ptr::null_mut(),
                    hCursor: ptr::null_mut(),
                    hbrBackground: unsafe { GetStockObject(BLACK_BRUSH) },
                    lpszMenuName: ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                    hIconSm: ptr::null_mut(),
                };
                // SAFETY: class 在调用期间有效，类名带 NUL。
                let atom = unsafe { RegisterClassExW(&class) };
                if atom == 0 {
                    Err("注册息屏窗口类失败".to_string())
                } else {
                    Ok(())
                }
            })
            .clone()
    }

    pub(super) fn create_cover(image_path: Option<&str>, close_hotkey: &str) -> Result<(), String> {
        register_class()?;
        if let Ok(mut hint) = COVER_HINT.lock() {
            *hint = super::close_hint_label(close_hotkey);
        }
        let bitmap = image_path
            .and_then(load_cover_bitmap)
            .unwrap_or(ptr::null_mut());
        COVER_BITMAP.store(bitmap as isize, Ordering::SeqCst);

        let (x, y, width, height) = virtual_screen_bounds();
        let class_name = wide(CLASS_NAME);
        let title = wide("息屏");
        // SAFETY: 类已注册；弹出层覆盖虚拟屏。
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_VISIBLE,
                x,
                y,
                width,
                height,
                ptr::null_mut(),
                ptr::null_mut(),
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            )
        };
        if hwnd.is_null() {
            destroy_cover();
            return Err("创建息屏窗口失败".to_string());
        }
        COVER_HWND.store(hwnd as isize, Ordering::SeqCst);

        unsafe {
            let affinity = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
            if affinity == 0 {
                destroy_cover();
                return Err("息屏窗口无法排除截图，已取消打开".to_string());
            }
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x,
                y,
                width,
                height,
                SWP_SHOWWINDOW | SWP_NOACTIVATE,
            );
            InvalidateRect(hwnd, ptr::null(), 1);
            UpdateWindow(hwnd);
            let hdc = GetDC(hwnd);
            if !hdc.is_null() {
                let mut rect = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                GetClientRect(hwnd, &mut rect);
                paint_close_hint(hdc, rect);
                ReleaseDC(hwnd, hdc);
            }
            SetTimer(hwnd, TOPMOST_TIMER_ID, TOPMOST_TIMER_MS, None);
        }
        Ok(())
    }

    pub(super) fn destroy_cover() {
        let hwnd = COVER_HWND.swap(0, Ordering::SeqCst) as HWND;
        if !hwnd.is_null() {
            unsafe {
                KillTimer(hwnd, TOPMOST_TIMER_ID);
                DestroyWindow(hwnd);
            }
        }
        let bitmap = COVER_BITMAP.swap(0, Ordering::SeqCst) as HBITMAP;
        if !bitmap.is_null() {
            unsafe {
                DeleteObject(bitmap);
            }
        }
    }
}

#[cfg(not(windows))]
mod native {
    pub(super) fn create_cover(
        _image_path: Option<&str>,
        _close_hotkey: &str,
    ) -> Result<(), String> {
        Err("息屏仅支持 Windows".to_string())
    }

    pub(super) fn destroy_cover() {}
}

use native::{create_cover, destroy_cover};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_hotkey_is_required() {
        assert!(require_close_hotkey("").is_err());
        assert!(require_close_hotkey("   ").is_err());
        assert_eq!(require_close_hotkey(" F8 ").unwrap(), "F8");
    }

    #[test]
    fn settings_default_is_black_without_hotkey() {
        let settings = PrivacyScreenSettings::default();
        assert!(settings.close_hotkey.is_empty());
        assert!(settings.image_path.is_none());
    }

    #[test]
    fn settings_round_trip_keeps_camel_case() {
        let settings = PrivacyScreenSettings {
            close_hotkey: "Ctrl+F12".to_string(),
            image_path: Some(r"C:\covers\night.png".to_string()),
        };
        let value = serde_json::to_value(&settings).unwrap();
        assert_eq!(value["closeHotkey"], "Ctrl+F12");
        assert_eq!(value["imagePath"], r"C:\covers\night.png");
        let loaded: PrivacyScreenSettings = serde_json::from_value(value).unwrap();
        assert_eq!(loaded, settings);
    }

    #[test]
    fn close_hint_includes_hotkey() {
        assert_eq!(close_hint_label("F8"), "关闭：F8");
        assert_eq!(close_hint_label("Ctrl+Shift+Q"), "关闭：Ctrl+Shift+Q");
    }

    #[test]
    fn hint_sits_on_primary_work_area_bottom_right() {
        let (x, y) = hint_plate_origin(0, 0, 1920, 1040, 160, 36);
        assert_eq!(x, 1920 - 160 - 28);
        assert_eq!(y, 1040 - 36 - 28);
        let (x, y) = hint_plate_origin(-1920, 0, 1920, 1080, 160, 36);
        assert_eq!(x, 1920 + 1920 - 160 - 28);
        assert_eq!(y, 1080 - 36 - 28);
    }
}
