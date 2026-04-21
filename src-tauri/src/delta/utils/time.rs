use std::time::{SystemTime, UNIX_EPOCH};

pub fn current_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::current_millis;

    #[test]
    fn returns_non_zero_timestamp() {
        assert!(current_millis() > 0);
    }
}
