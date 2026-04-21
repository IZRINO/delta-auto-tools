use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};

use crate::delta::constants::DF_REFERER;

pub fn browser_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36",
        ),
    );
    headers.insert(REFERER, HeaderValue::from_static(DF_REFERER));
    headers
}
