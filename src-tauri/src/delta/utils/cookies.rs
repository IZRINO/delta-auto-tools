use std::{collections::HashMap, sync::Arc};

use reqwest::cookie::{CookieStore, Jar};
use url::Url;

use crate::delta::error::DeltaError;

pub fn restore_cookie_json(jar: &Arc<Jar>, url: &str, cookie_json: &str) -> Result<(), DeltaError> {
    let parsed = Url::parse(url)?;
    let cookies: HashMap<String, String> = serde_json::from_str(cookie_json)?;
    for (name, value) in cookies {
        jar.add_cookie_str(&format!("{name}={value}; Path=/"), &parsed);
    }
    Ok(())
}

pub fn dump_cookie_json(jar: &Arc<Jar>, url: &str) -> Result<String, DeltaError> {
    let parsed = Url::parse(url)?;
    let mut map = HashMap::new();
    if let Some(value) = jar.cookies(&parsed) {
        let text = value
            .to_str()
            .map_err(|error| DeltaError::Parse(error.to_string()))?;
        for part in text.split(';') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some((name, cookie_value)) = trimmed.split_once('=') {
                map.insert(name.to_string(), cookie_value.to_string());
            }
        }
    }
    serde_json::to_string(&map).map_err(Into::into)
}

pub fn insert_cookie(jar: &Arc<Jar>, url: &str, name: &str, value: &str) -> Result<(), DeltaError> {
    let parsed = Url::parse(url)?;
    jar.add_cookie_str(&format!("{name}={value}; Path=/"), &parsed);
    Ok(())
}

pub fn must_cookie(jar: &Arc<Jar>, url: &str, name: &str) -> Result<String, DeltaError> {
    let parsed = Url::parse(url)?;
    let header = jar
        .cookies(&parsed)
        .ok_or_else(|| DeltaError::Parse(format!("cookie {name} not found")))?;
    let text = header
        .to_str()
        .map_err(|error| DeltaError::Parse(error.to_string()))?;

    text.split(';')
        .map(str::trim)
        .find_map(|part| {
            let (cookie_name, cookie_value) = part.split_once('=')?;
            (cookie_name == name).then(|| cookie_value.to_string())
        })
        .ok_or_else(|| DeltaError::Parse(format!("cookie {name} not found")))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use reqwest::cookie::Jar;

    use super::{dump_cookie_json, insert_cookie, must_cookie, restore_cookie_json};

    #[test]
    fn restores_and_dumps_cookie_json() {
        let jar = Arc::new(Jar::default());
        restore_cookie_json(
            &jar,
            "https://graph.qq.com/",
            r#"{"p_skey":"abc","uin":"123"}"#,
        )
        .unwrap();
        assert_eq!(
            must_cookie(&jar, "https://graph.qq.com/", "p_skey").unwrap(),
            "abc"
        );

        let dumped = dump_cookie_json(&jar, "https://graph.qq.com/").unwrap();
        assert!(dumped.contains("p_skey"));
        assert!(dumped.contains("uin"));
    }

    #[test]
    fn inserts_cookie_into_jar() {
        let jar = Arc::new(Jar::default());
        insert_cookie(&jar, "https://gamesafe.qq.com/", "gs_code", "jwt").unwrap();
        assert_eq!(
            must_cookie(&jar, "https://gamesafe.qq.com/", "gs_code").unwrap(),
            "jwt"
        );
    }

    #[test]
    fn finds_named_cookie_even_when_not_first_in_header() {
        let jar = Arc::new(Jar::default());
        insert_cookie(&jar, "https://graph.qq.com/", "uin", "123").unwrap();
        insert_cookie(&jar, "https://graph.qq.com/", "p_skey", "abc").unwrap();

        assert_eq!(
            must_cookie(&jar, "https://graph.qq.com/", "p_skey").unwrap(),
            "abc"
        );
    }
}
