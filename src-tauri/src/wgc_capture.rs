//! 息屏打开时用 Windows Graphics Capture 截屏。
//! WGC 会尊重 `WDA_EXCLUDEFROMCAPTURE`，把遮罩从构图里抠掉，透视到下方画面。
//! 默认路径仍走 xcap GDI：Win10 上 WGC 会画黄框，且不能改已校准模板的截图基线。

use std::sync::mpsc::channel;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use image::RgbaImage;
use windows::core::{factory, Interface};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HMODULE, POINT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
    D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONULL};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

const FRAME_TIMEOUT: Duration = Duration::from_millis(1500);

struct D3d {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    dxgi: IDXGIDevice,
    gpu: Mutex<()>,
}

fn d3d() -> Option<&'static D3d> {
    static D3D: OnceLock<Option<D3d>> = OnceLock::new();
    D3D.get_or_init(|| {
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let mut device = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )
            .ok()?;
        }
        let device = device?;
        let context = unsafe { device.GetImmediateContext().ok()? };
        let dxgi = device.cast::<IDXGIDevice>().ok()?;
        Some(D3d {
            device,
            context,
            dxgi,
            gpu: Mutex::new(()),
        })
    })
    .as_ref()
}

fn bgra_to_rgba(mut buffer: Vec<u8>) -> Vec<u8> {
    for pixel in buffer.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    buffer
}

fn texture_region_to_image(
    d3d: &D3d,
    source: &ID3D11Texture2D,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Option<RgbaImage> {
    let mut src_desc = D3D11_TEXTURE2D_DESC::default();
    unsafe {
        source.GetDesc(&mut src_desc);
    }
    if x.saturating_add(width) > src_desc.Width || y.saturating_add(height) > src_desc.Height {
        return None;
    }

    let mut staging_desc = src_desc;
    staging_desc.Width = width;
    staging_desc.Height = height;
    staging_desc.BindFlags = 0;
    staging_desc.MiscFlags = 0;
    staging_desc.Usage = D3D11_USAGE_STAGING;
    staging_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;

    let mut staging = None;
    unsafe {
        d3d.device
            .CreateTexture2D(&staging_desc, None, Some(&mut staging))
            .ok()?;
    }
    let staging = staging?;
    let region = D3D11_BOX {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
        front: 0,
        back: 1,
    };
    let staging_resource: ID3D11Resource = staging.cast().ok()?;
    let source_resource: ID3D11Resource = source.cast().ok()?;
    let _gpu = d3d.gpu.lock().ok()?;
    unsafe {
        d3d.context.CopySubresourceRegion(
            Some(&staging_resource),
            0,
            0,
            0,
            0,
            Some(&source_resource),
            0,
            Some(&region),
        );
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        d3d.context
            .Map(
                Some(&staging_resource),
                0,
                D3D11_MAP_READ,
                0,
                Some(&mut mapped),
            )
            .ok()?;
        let mut bgra = vec![0u8; (width * height * 4) as usize];
        let src_ptr = mapped.pData as *const u8;
        for row in 0..height {
            let src_offset = (row * mapped.RowPitch) as usize;
            let dst_offset = (row * width * 4) as usize;
            let src_row = std::slice::from_raw_parts(src_ptr.add(src_offset), (width * 4) as usize);
            bgra[dst_offset..dst_offset + (width * 4) as usize].copy_from_slice(src_row);
        }
        d3d.context.Unmap(Some(&staging_resource), 0);
        RgbaImage::from_raw(width, height, bgra_to_rgba(bgra))
    }
}

fn capture_item(
    item: &GraphicsCaptureItem,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Option<RgbaImage> {
    let d3d = d3d()?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&d3d.dxgi).ok()? };
    let winrt_device: IDirect3DDevice = inspectable.cast().ok()?;
    let size = item.Size().ok()?;
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &winrt_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        size,
    )
    .ok()?;
    let (sender, receiver) = channel();
    pool.FrameArrived(&TypedEventHandler::new({
        let pool = pool.clone();
        move |_, _| {
            let frame = match pool.TryGetNextFrame() {
                Ok(frame) => frame,
                Err(_) => return Ok(()),
            };
            let surface = match frame.Surface() {
                Ok(surface) => surface,
                Err(_) => {
                    let _ = frame.Close();
                    return Ok(());
                }
            };
            let access: IDirect3DDxgiInterfaceAccess = match surface.cast() {
                Ok(access) => access,
                Err(_) => {
                    let _ = frame.Close();
                    return Ok(());
                }
            };
            let texture: ID3D11Texture2D = match unsafe { access.GetInterface() } {
                Ok(texture) => texture,
                Err(_) => {
                    let _ = frame.Close();
                    return Ok(());
                }
            };
            if let Some(image) = texture_region_to_image(d3d, &texture, x, y, width, height) {
                let _ = sender.send(image);
            }
            let _ = frame.Close();
            Ok(())
        }
    }))
    .ok()?;
    let session = pool.CreateCaptureSession(item).ok()?;
    let _ = session.SetIsBorderRequired(false);
    let _ = session.SetIsCursorCaptureEnabled(false);
    session.StartCapture().ok()?;
    let image = receiver.recv_timeout(FRAME_TIMEOUT).ok();
    let _ = session.Close();
    let _ = pool.Close();
    image
}

pub fn capture_monitor_region(
    origin_x: i32,
    origin_y: i32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Option<RgbaImage> {
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    let hmonitor = unsafe {
        MonitorFromPoint(
            POINT {
                x: origin_x.saturating_add(1),
                y: origin_y.saturating_add(1),
            },
            MONITOR_DEFAULTTONULL,
        )
    };
    if hmonitor.is_invalid() {
        return None;
    }
    let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().ok()?;
    let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(hmonitor).ok()? };
    capture_item(&item, x, y, width, height)
}
