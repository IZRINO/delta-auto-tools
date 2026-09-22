//! 截图 + 参考图像 I/O + base64 编码

use std::io::Cursor;
use std::path::Path;

/// 截取屏幕区域。默认 xcap GDI；息屏打开时改 WGC，避开遮罩。
pub(crate) fn capture_region(
    region: &crate::morse::types::RegionRect,
) -> Option<image::DynamicImage> {
    #[cfg(target_os = "windows")]
    {
        use crate::morse::recognition::region_to_capture_bounds;
        use xcap::Monitor;

        let monitors = Monitor::all().ok()?;
        for monitor in monitors {
            let (Ok(monitor_left), Ok(monitor_top), Ok(monitor_width), Ok(monitor_height)) =
                (monitor.x(), monitor.y(), monitor.width(), monitor.height())
            else {
                continue;
            };
            let scale_factor = monitor.scale_factor().unwrap_or(1.0);

            let Some((x, y, width, height)) = region_to_capture_bounds(
                region,
                monitor_left,
                monitor_top,
                monitor_width,
                monitor_height,
                scale_factor,
            ) else {
                continue;
            };

            let rgba = if crate::privacy_screen::is_cover_visible() {
                crate::wgc_capture::capture_monitor_region(
                    monitor_left,
                    monitor_top,
                    x,
                    y,
                    width,
                    height,
                )
            } else {
                let Ok(capture) = monitor.capture_region(x, y, width, height) else {
                    continue;
                };
                image::RgbaImage::from_raw(capture.width(), capture.height(), capture.into_raw())
            };
            let Some(rgba) = rgba else {
                continue;
            };

            return Some(image::DynamicImage::ImageRgba8(rgba));
        }

        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// 同一监视器上一次截取包围盒，再裁出各区域。跨屏或不完整时回落逐块截。
pub(crate) fn capture_regions(
    regions: &[crate::morse::types::RegionRect],
) -> Option<Vec<image::DynamicImage>> {
    if regions.is_empty() {
        return Some(Vec::new());
    }
    #[cfg(target_os = "windows")]
    {
        capture_regions_once(regions).or_else(|| {
            regions
                .iter()
                .map(capture_region)
                .collect::<Option<Vec<_>>>()
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(target_os = "windows")]
fn capture_regions_once(
    regions: &[crate::morse::types::RegionRect],
) -> Option<Vec<image::DynamicImage>> {
    use crate::morse::recognition::region_to_capture_bounds;
    use xcap::Monitor;

    let monitors = Monitor::all().ok()?;
    for monitor in monitors {
        let (Ok(monitor_left), Ok(monitor_top), Ok(monitor_width), Ok(monitor_height)) =
            (monitor.x(), monitor.y(), monitor.width(), monitor.height())
        else {
            continue;
        };
        let scale_factor = monitor.scale_factor().unwrap_or(1.0);
        let mut bounds = Vec::with_capacity(regions.len());
        let mut fits = true;
        for region in regions {
            let Some(bound) = region_to_capture_bounds(
                region,
                monitor_left,
                monitor_top,
                monitor_width,
                monitor_height,
                scale_factor,
            ) else {
                fits = false;
                break;
            };
            bounds.push(bound);
        }
        if !fits {
            continue;
        }
        let union_x = bounds.iter().map(|bound| bound.0).min()?;
        let union_y = bounds.iter().map(|bound| bound.1).min()?;
        let union_right = bounds
            .iter()
            .map(|bound| bound.0.saturating_add(bound.2))
            .max()?;
        let union_bottom = bounds
            .iter()
            .map(|bound| bound.1.saturating_add(bound.3))
            .max()?;
        let union_w = union_right.saturating_sub(union_x);
        let union_h = union_bottom.saturating_sub(union_y);
        if union_w == 0 || union_h == 0 {
            continue;
        }
        let rgba = if crate::privacy_screen::is_cover_visible() {
            crate::wgc_capture::capture_monitor_region(
                monitor_left,
                monitor_top,
                union_x,
                union_y,
                union_w,
                union_h,
            )
        } else {
            let Ok(capture) = monitor.capture_region(union_x, union_y, union_w, union_h) else {
                continue;
            };
            image::RgbaImage::from_raw(capture.width(), capture.height(), capture.into_raw())
        };
        let Some(rgba) = rgba else {
            continue;
        };
        let image = image::DynamicImage::ImageRgba8(rgba);
        let mut crops = Vec::with_capacity(bounds.len());
        for (x, y, width, height) in bounds {
            let local_x = x.saturating_sub(union_x);
            let local_y = y.saturating_sub(union_y);
            let crop_w = width.min(image.width().saturating_sub(local_x)).max(1);
            let crop_h = height.min(image.height().saturating_sub(local_y)).max(1);
            crops.push(image.crop_imm(local_x, local_y, crop_w, crop_h));
        }
        return Some(crops);
    }
    None
}

/// 加载参考图像
pub(crate) fn load_reference_image(path: &str) -> Option<image::DynamicImage> {
    let path = Path::new(path);
    if !path.exists() {
        return None;
    }
    image::open(path).ok()
}

/// 读取参考图像为 PNG base64 数据 URL（供前端预览）
pub(crate) fn read_reference_image_as_data_url(path: &str) -> Option<String> {
    let path = Path::new(path);
    if !path.exists() {
        return None;
    }
    let img = image::open(path).ok()?;
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    let b64 = base64_encode(&buf.into_inner());
    Some(format!("data:image/png;base64,{b64}"))
}

/// 简易 base64 编码（不引入额外 crate）
pub(crate) fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        out.push(CHARS[(n & 0x3F) as usize] as char);
        i += 3;
    }
    if i + 1 < data.len() {
        let n = ((data[i] as u32) << 10) | ((data[i + 1] as u32) << 2);
        out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        out.push(CHARS[(n & 0x3F) as usize] as char);
        out.push('=');
    } else if i < data.len() {
        let n = (data[i] as u32) << 4;
        out.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        out.push(CHARS[(n & 0x3F) as usize] as char);
        out.push_str("==");
    }
    out
}
