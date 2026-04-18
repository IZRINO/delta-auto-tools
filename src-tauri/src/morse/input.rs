use std::{thread, time::Duration};

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

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
