const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 0x3f) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn osc52(text: &str) -> Vec<u8> {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes())).into_bytes()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::empty("", "")]
    #[case::one_byte("f", "Zg==")]
    #[case::two_bytes("fo", "Zm8=")]
    #[case::three_bytes("foo", "Zm9v")]
    #[case::url("https://x.dev/1", "aHR0cHM6Ly94LmRldi8x")]
    #[case::non_ascii("ñ", "w7E=")]
    fn encodes_like_rfc_4648(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(base64(input.as_bytes()), expected);
    }

    #[test]
    fn osc52_asks_the_terminal_to_set_the_clipboard() {
        assert_eq!(osc52("foo"), b"\x1b]52;c;Zm9v\x07");
    }
}
