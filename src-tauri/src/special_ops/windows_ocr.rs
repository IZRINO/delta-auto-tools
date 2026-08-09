const OCR_IMAGE_SCALE: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OcrBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

fn restore_bounds_after_ocr_scale(bounds: OcrBounds) -> OcrBounds {
    let scale = OCR_IMAGE_SCALE as f32;
    OcrBounds::new(
        bounds.x / scale,
        bounds.y / scale,
        bounds.width / scale,
        bounds.height / scale,
    )
}

impl OcrBounds {
    pub(crate) const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OcrWord {
    pub text: String,
    pub bounds: OcrBounds,
}

impl OcrWord {
    pub(crate) fn new(text: impl Into<String>, bounds: OcrBounds) -> Self {
        Self {
            text: text.into(),
            bounds,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub(crate) fn numeric_words(words: Vec<OcrWord>) -> Vec<OcrWord> {
    words
        .into_iter()
        .filter_map(|mut word| {
            word.text.retain(|ch| !ch.is_whitespace());
            (!word.text.is_empty() && word.text.chars().all(|ch| ch.is_ascii_digit()))
                .then_some(word)
        })
        .collect()
}

pub(crate) fn to_screen_bounds(
    bounds: OcrBounds,
    region: &crate::morse::types::RegionRect,
) -> ScreenBounds {
    ScreenBounds {
        x: region.x.saturating_add(bounds.x.round() as i32),
        y: region.y.saturating_add(bounds.y.round() as i32),
        width: bounds.width.round() as i32,
        height: bounds.height.round() as i32,
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn recognize_words(image: image::DynamicImage) -> Result<Vec<OcrWord>, String> {
    use windows::{
        Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
        Media::Ocr::OcrEngine,
        Storage::Streams::DataWriter,
    };

    let width = image
        .width()
        .checked_mul(OCR_IMAGE_SCALE)
        .ok_or_else(|| "OCR 截图宽度超出范围".to_string())?;
    let height = image
        .height()
        .checked_mul(OCR_IMAGE_SCALE)
        .ok_or_else(|| "OCR 截图高度超出范围".to_string())?;
    let rgba = image
        .resize_exact(width, height, image::imageops::FilterType::Nearest)
        .to_rgba8();
    let width = i32::try_from(rgba.width()).map_err(|_| "OCR 截图宽度超出范围".to_string())?;
    let height = i32::try_from(rgba.height()).map_err(|_| "OCR 截图高度超出范围".to_string())?;
    let mut bgra = rgba.into_raw();
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    let writer = DataWriter::new().map_err(|error| format!("初始化 OCR 像素缓冲失败: {error}"))?;
    writer
        .WriteBytes(&bgra)
        .map_err(|error| format!("写入 OCR 像素缓冲失败: {error}"))?;
    let buffer = writer
        .DetachBuffer()
        .map_err(|error| format!("获取 OCR 像素缓冲失败: {error}"))?;
    let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        width,
        height,
        BitmapAlphaMode::Ignore,
    )
    .map_err(|error| format!("创建 OCR 位图失败: {error}"))?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|error| format!("Windows OCR 不可用: {error}"))?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .map_err(|error| format!("启动 Windows OCR 失败: {error}"))?
        .get()
        .map_err(|error| format!("Windows OCR 识别失败: {error}"))?;

    let lines = result
        .Lines()
        .map_err(|error| format!("读取 Windows OCR 行失败: {error}"))?;
    let mut words = Vec::new();
    for line_index in 0..lines
        .Size()
        .map_err(|error| format!("读取 Windows OCR 行数失败: {error}"))?
    {
        let line = lines
            .GetAt(line_index)
            .map_err(|error| format!("读取 Windows OCR 行失败: {error}"))?;
        let line_words = line
            .Words()
            .map_err(|error| format!("读取 Windows OCR 文字失败: {error}"))?;
        for word_index in 0..line_words
            .Size()
            .map_err(|error| format!("读取 Windows OCR 文字数失败: {error}"))?
        {
            let word = line_words
                .GetAt(word_index)
                .map_err(|error| format!("读取 Windows OCR 文字失败: {error}"))?;
            let text = word
                .Text()
                .map_err(|error| format!("读取 Windows OCR 文本失败: {error}"))?
                .to_string_lossy();
            let rect = word
                .BoundingRect()
                .map_err(|error| format!("读取 Windows OCR 坐标失败: {error}"))?;
            words.push(OcrWord::new(
                text,
                restore_bounds_after_ocr_scale(OcrBounds::new(
                    rect.X,
                    rect.Y,
                    rect.Width,
                    rect.Height,
                )),
            ));
        }
    }
    Ok(words)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn recognize_words(_image: image::DynamicImage) -> Result<Vec<OcrWord>, String> {
    Err("Windows OCR 仅支持 Windows".to_string())
}

pub(crate) fn recognize_numeric_words(image: image::DynamicImage) -> Result<Vec<OcrWord>, String> {
    recognize_words(image).map(numeric_words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_words_normalize_whitespace_and_reject_non_numeric_text() {
        let words = vec![
            OcrWord::new("123456", OcrBounds::new(10.0, 20.0, 60.0, 16.0)),
            OcrWord::new("123 456", OcrBounds::new(10.0, 40.0, 60.0, 16.0)),
            OcrWord::new("账号123", OcrBounds::new(10.0, 60.0, 60.0, 16.0)),
        ];

        assert_eq!(
            numeric_words(words),
            vec![
                OcrWord::new("123456", OcrBounds::new(10.0, 20.0, 60.0, 16.0)),
                OcrWord::new("123456", OcrBounds::new(10.0, 40.0, 60.0, 16.0)),
            ]
        );
    }

    #[test]
    fn numeric_words_remove_internal_whitespace_before_filtering() {
        let words = vec![OcrWord::new(
            "3079 3589",
            OcrBounds::new(10.0, 20.0, 60.0, 16.0),
        )];

        assert_eq!(
            numeric_words(words),
            vec![OcrWord::new(
                "30793589",
                OcrBounds::new(10.0, 20.0, 60.0, 16.0),
            )]
        );
    }

    #[test]
    fn relative_ocr_bounds_convert_to_absolute_screen_bounds() {
        let region = crate::morse::types::RegionRect {
            x: 100,
            y: 200,
            width: 300,
            height: 400,
        };

        assert_eq!(
            to_screen_bounds(OcrBounds::new(10.4, 20.6, 60.2, 16.8), &region),
            ScreenBounds {
                x: 110,
                y: 221,
                width: 60,
                height: 17,
            }
        );
    }

    #[test]
    fn scaled_ocr_bounds_return_to_source_coordinates() {
        assert_eq!(
            restore_bounds_after_ocr_scale(OcrBounds::new(31.5, 61.5, 180.0, 51.0)),
            OcrBounds::new(10.5, 20.5, 60.0, 17.0),
        );
    }
}
