use crate::{input_simulation, morse::types::ClickRegion};

pub async fn type_result(value: &str, delay_ms: u64) -> Result<(), String> {
    input_simulation::type_text(value, delay_ms).await
}

pub async fn press_hotkey_once(hotkey: &str) -> Result<(), String> {
    input_simulation::press_hotkey_once(hotkey, "点击完成后按键").await
}

/// 按顺序点击已配置的点击区域，每个区域使用独立的延迟。
pub async fn click_regions(regions: &[ClickRegion]) -> Result<(), String> {
    let points = regions
        .iter()
        .map(|c| {
            let center_x = c.rect.x + c.rect.width / 2;
            let center_y = c.rect.y + c.rect.height / 2;
            (center_x, center_y, c.delay_ms)
        })
        .collect::<Vec<_>>();
    input_simulation::click_points(&points).await
}
