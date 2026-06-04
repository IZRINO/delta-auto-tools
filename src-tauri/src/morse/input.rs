use std::{thread, time::Duration};

use enigo::{Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use crate::morse::types::RegionRect;
pub async fn type_result(value: &str, delay_ms: u64) -> Result<(), String> {
    let value = value.to_string();

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化自动输入失败: {error}"))?;

        for ch in value.chars() {
            enigo
                .key(Key::Unicode(ch), Direction::Click)
                .map_err(|error| format!("自动输入字符 {ch} 失败: {error}"))?;

            if delay_ms > 0 {
                thread::sleep(Duration::from_millis(delay_ms));
            }
        }

        Ok(())
    })
    .await
    .map_err(|error| format!("自动输入任务执行失败: {error}"))?
}

/// 按顺序点击已配置的点击区域
pub async fn click_regions(
    regions: &[Option<RegionRect>],
    delay_ms: u64,
) -> Result<(), String> {
    let regions: Vec<RegionRect> = regions
        .iter()
        .filter_map(|r| r.clone())
        .collect();

    if regions.is_empty() {
        return Ok(());
    }

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("初始化鼠标点击失败: {error}"))?;

        for region in &regions {
            if delay_ms > 0 {
                thread::sleep(Duration::from_millis(delay_ms));
            }
            let center_x = region.x + region.width / 2;
            let center_y = region.y + region.height / 2;
            enigo
                .move_mouse(center_x, center_y, Coordinate::Abs)
                .map_err(|error| format!("移动鼠标到 ({center_x}, {center_y}) 失败: {error}"))?;
            enigo
                .button(enigo::Button::Left, Direction::Click)
                .map_err(|error| format!("鼠标左键点击失败: {error}"))?;
        }

        Ok(())
    })
    .await
    .map_err(|error| format!("自动点击任务执行失败: {error}"))?
}