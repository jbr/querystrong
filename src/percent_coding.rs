use std::borrow::Cow;
use std::fmt::Write;

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn decode<'a>(s: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
    let s = s.into();
    let Some(i) = s.bytes().position(|b| b == b'%' || b == b'+') else {
        return s;
    };
    let mut out = Vec::with_capacity(s.len());
    out.extend_from_slice(&s.as_bytes()[..i]);
    let bytes = s.as_bytes();
    let mut i = i;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => match (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push((hi << 4) | lo);
                    i += 3;
                }
                _ => {
                    out.push(b'%');
                    i += 1;
                }
            },
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    Cow::Owned(String::from_utf8_lossy(&out).into_owned())
}

pub(crate) fn encode(s: &str) -> Cow<'_, str> {
    let Some(i) = s
        .bytes()
        .position(|b| !b.is_ascii_alphanumeric() && b != b'_' && b != b'-')
    else {
        return Cow::Borrowed(s);
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push_str(&s[..i]);
    for b in s[i..].bytes() {
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_lowercase_hex() {
        assert_eq!(decode("foo%20bar%3f"), "foo bar?");
    }

    #[test]
    fn decode_borrowed_fast_path() {
        assert!(matches!(decode("foo_bar-baz"), Cow::Borrowed(_)));
        assert!(matches!(decode("foo%20bar"), Cow::Owned(_)));
        assert!(matches!(decode("foo+bar"), Cow::Owned(_)));
    }

    #[test]
    fn decode_owned_fast_path() {
        // Cow::Owned passes through without cloning when no decoding is needed
        assert!(matches!(decode(String::from("foo_bar-baz")), Cow::Owned(_)));
    }

    #[test]
    fn decode_unicode() {
        assert_eq!(decode("%F0%9F%92%96"), "💖");
    }

    #[test]
    fn decode_invalid_utf8_lossy() {
        assert_eq!(decode("%00%9F%92%96"), "\u{0}\u{FFFD}\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn encode_borrowed_fast_path() {
        assert!(matches!(encode("foo_bar-baz"), Cow::Borrowed(_)));
        assert!(matches!(encode("foo bar"), Cow::Owned(_)));
    }

    #[test]
    fn encode_charset() {
        assert_eq!(encode("foo bar?"), "foo%20bar%3F");
        assert_eq!(encode("under_score-dash"), "under_score-dash");
        assert_eq!(encode("💖"), "%F0%9F%92%96");
    }
}
