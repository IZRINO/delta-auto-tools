const MORSE_DIGIT_MAP: [(&str, char); 10] = [
    (".----", '1'),
    ("..---", '2'),
    ("...--", '3'),
    ("....-", '4'),
    (".....", '5'),
    ("-....", '6'),
    ("--...", '7'),
    ("---..", '8'),
    ("----.", '9'),
    ("-----", '0'),
];

pub fn decode(morse: &str) -> Result<char, String> {
    MORSE_DIGIT_MAP
        .iter()
        .find_map(|(pattern, digit)| (*pattern == morse).then_some(*digit))
        .ok_or_else(|| format!("无法识别的摩斯密码: {morse}"))
}

pub fn decode_sequence<'a>(
    morse_list: impl IntoIterator<Item = &'a str>,
) -> Result<String, String> {
    morse_list.into_iter().map(decode).collect()
}

#[cfg(test)]
mod tests {
    use super::{decode, decode_sequence};

    #[test]
    fn decodes_single_digit_patterns() {
        assert_eq!(decode(".----").unwrap(), '1');
        assert_eq!(decode("-----").unwrap(), '0');
        assert_eq!(decode("---..").unwrap(), '8');
    }

    #[test]
    fn decodes_digit_sequence() {
        let decoded = decode_sequence([".----", "..---", "-----"]).unwrap();
        assert_eq!(decoded, "120");
    }

    #[test]
    fn rejects_unknown_pattern() {
        let error = decode(".-.-.").unwrap_err();
        assert!(error.contains("无法识别的摩斯密码"));
    }
}
