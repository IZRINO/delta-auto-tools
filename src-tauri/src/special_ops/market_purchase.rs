use serde::{Deserialize, Serialize};

use super::CalibrationRect;

fn default_entry_delay_ms() -> u32 {
    3_000
}

fn default_window_start_minute() -> u16 {
    2 * 60
}

fn default_window_end_minute() -> u16 {
    20 * 60
}

fn default_purchase_count() -> u32 {
    1
}

fn default_max_price() -> u64 {
    1
}

const MARKET_BUSINESS_SCHEMA_VERSION: u8 = 1;

/// 价格 OCR 队尾补偿仍失败后，等到这个间隔再开一轮（每轮仍是三页失败→队尾重试）。
pub(crate) const PRICE_OCR_RETRY_INTERVAL_MS: i64 = 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketPurchaseSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_entry_delay_ms")]
    pub entry_delay_ms: u32,
    #[serde(default = "default_purchase_count")]
    pub purchase_count: u32,
    #[serde(default)]
    pub item_note: String,
    /// 交易行开放开始时间（分钟，从 0 点起算），默认 02:00 = 120。
    #[serde(default = "default_window_start_minute")]
    pub window_start_minute: u16,
    /// 交易行开放结束时间（分钟，从 0 点起算），默认 20:00 = 1200。
    #[serde(default = "default_window_end_minute")]
    pub window_end_minute: u16,
}

impl Default for MarketPurchaseSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            entry_delay_ms: default_entry_delay_ms(),
            purchase_count: default_purchase_count(),
            item_note: String::new(),
            window_start_minute: default_window_start_minute(),
            window_end_minute: default_window_end_minute(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceDecision {
    Buy,
    Return,
}

/// 从价格 OCR 文本里取价格。
///
/// 价格区域不可能框得刚好贴住数字：位数会变（`700` / `7000`），框宽了就会同时
/// 捕获旁边的数量、库存等数字。因此按空白切 token（`join_words_by_gap` 已按横向
/// 间距插入空格），token 内部去掉千分位逗号，再取**位数最多**的候选：价格永远比
/// 相邻的数量/序号长。位数相同时取更大的那个。
pub(crate) fn parse_market_price(text: &str) -> Option<u64> {
    text.split_whitespace()
        .filter_map(|token| {
            let digits = token
                .chars()
                .filter(|character| character.is_ascii_digit())
                .collect::<String>();
            digits
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .map(|value| (digits.len(), value))
        })
        .max()
        .map(|(_, value)| value)
}

pub(crate) const fn price_decision(price: u64, max_price: u64) -> PriceDecision {
    if price <= max_price {
        PriceDecision::Buy
    } else {
        PriceDecision::Return
    }
}

pub(crate) const fn market_window_open(minute: u16, start: u16, end: u16) -> bool {
    minute >= start && minute < end
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketBusinessConfig {
    #[serde(default)]
    pub schema_version: u8,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_purchase_count")]
    pub purchase_count: u32,
    #[serde(default)]
    pub item_note: String,
    #[serde(default)]
    pub product_point: Option<CalibrationRect>,
    #[serde(default = "default_max_price")]
    pub max_price: u64,
}

impl Default for MarketBusinessConfig {
    fn default() -> Self {
        Self {
            schema_version: MARKET_BUSINESS_SCHEMA_VERSION,
            enabled: false,
            purchase_count: default_purchase_count(),
            item_note: String::new(),
            product_point: None,
            max_price: default_max_price(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum MarketTaskStatus {
    #[default]
    Pending,
    Running,
    Completed,
    PriceRecognitionFailed,
    WindowClosed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MarketAccountState {
    pub day: Option<String>,
    pub completed_count: u32,
    pub status: MarketTaskStatus,
    pub last_error: Option<String>,
    /// 价格 OCR 本轮队尾补偿仍失败后的下一次可执行时间。窗口关闭后自然不再到期。
    #[serde(default)]
    pub price_retry_at_ms: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::{market_window_open, parse_market_price, price_decision, PriceDecision};

    #[test]
    fn price_parser_keeps_only_ascii_digits() {
        assert_eq!(parse_market_price("价格 12,345"), Some(12_345));
        assert_eq!(parse_market_price("价格 １２３"), None);
        assert_eq!(parse_market_price("无价格"), None);
        assert_eq!(parse_market_price("0"), None);
    }

    #[test]
    fn price_parser_takes_longest_token_from_wide_region() {
        // 价格位数会变（700 / 7000），用户只能把区域框大。宽框会连带捕获数量、
        // 序号等数字，旧实现把区域内所有数字拼成一个值（700 + 12 -> 70012）。
        assert_eq!(parse_market_price("700 12"), Some(700));
        assert_eq!(parse_market_price("7000 12"), Some(7_000));
        assert_eq!(parse_market_price("12 700"), Some(700));
        // 千分位逗号在同一 token 内，位数按数字算：7,000 是 4 位 -> 胜过 3 位的 999。
        assert_eq!(parse_market_price("999 7,000"), Some(7_000));
    }

    #[test]
    fn price_parser_breaks_equal_length_ties_by_value() {
        assert_eq!(parse_market_price("620 700"), Some(700));
    }

    #[test]
    fn equal_price_is_buyable() {
        assert_eq!(price_decision(10_000, 10_000), PriceDecision::Buy);
        assert_eq!(price_decision(10_001, 10_000), PriceDecision::Return);
        assert_eq!(price_decision(9_999, 10_000), PriceDecision::Buy);
    }

    #[test]
    fn market_window_stops_at_eight_pm() {
        let start = 2 * 60;
        let end = 20 * 60;
        assert!(!market_window_open(119, start, end));
        assert!(market_window_open(2 * 60, start, end));
        assert!(market_window_open(19 * 60 + 59, start, end));
        assert!(!market_window_open(20 * 60, start, end));
    }
}
