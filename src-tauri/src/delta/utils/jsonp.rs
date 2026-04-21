use crate::delta::error::DeltaError;

pub fn extract_jsonp_args(body: &str, callback: &str) -> Result<Vec<String>, DeltaError> {
    let prefix = format!("{callback}(");
    let start = body
        .find(&prefix)
        .ok_or_else(|| DeltaError::Parse(format!("callback {callback} not found")))?
        + prefix.len();
    let end = body
        .rfind(')')
        .ok_or_else(|| DeltaError::Parse("missing jsonp closing bracket".to_string()))?;

    let inner = &body[start..end];
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut string_char = '\0';
    let mut escaped = false;

    for ch in inner.chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == string_char {
                in_string = false;
            }
            continue;
        }

        match ch {
            '\'' | '"' => {
                in_string = true;
                string_char = ch;
                current.push(ch);
            }
            '{' | '[' | '(' => {
                depth += 1;
                current.push(ch);
            }
            '}' | ']' | ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                args.push(trim_arg(&current));
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        args.push(trim_arg(&current));
    }

    Ok(args)
}

fn trim_arg(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let first = bytes[0] as char;
        let last = bytes[trimmed.len() - 1] as char;
        if (first == '\'' && last == '\'') || (first == '"' && last == '"') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::extract_jsonp_args;

    #[test]
    fn parses_quoted_and_json_arguments() {
        let args = extract_jsonp_args(
            "coolxitech({\"access_token\":\"abc\",\"openid\":\"u1\"}, 'ok')",
            "coolxitech",
        )
        .unwrap();

        assert_eq!(args[0], "{\"access_token\":\"abc\",\"openid\":\"u1\"}");
        assert_eq!(args[1], "ok");
    }

    #[test]
    fn parses_ptui_callback() {
        let args = extract_jsonp_args("ptuiCB('66','0','','0','msg','')", "ptuiCB").unwrap();
        assert_eq!(args[0], "66");
        assert_eq!(args[4], "msg");
    }
}
