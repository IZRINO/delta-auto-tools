pub fn get_qr_token(value: &str) -> i64 {
    let mut hash = 0_i64;
    for byte in value.bytes() {
        hash += (hash << 5) + i64::from(byte);
        hash &= 0x7fff_ffff;
    }
    hash
}

pub fn get_gtk(value: &str) -> i64 {
    let mut hash = 5381_i64;
    for byte in value.bytes() {
        hash += (hash << 5) + i64::from(byte);
        hash &= 0x7fff_ffff;
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{get_gtk, get_qr_token};

    #[test]
    fn calculates_qr_token() {
        assert_eq!(get_qr_token("qrsig123"), 610575516);
    }

    #[test]
    fn calculates_gtk() {
        assert_eq!(get_gtk("p_skey-demo"), 1404090882);
    }
}
