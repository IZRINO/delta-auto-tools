use encoding_rs::GBK;

pub fn decode_gbk(bytes: &[u8]) -> String {
    let (decoded, _, _) = GBK.decode(bytes);
    decoded.into_owned()
}

#[cfg(test)]
mod tests {
    use super::decode_gbk;

    #[test]
    fn decodes_gbk_payload() {
        assert_eq!(decode_gbk(&[0xB2, 0xE2, 0xCA, 0xD4]), "测试");
    }
}
