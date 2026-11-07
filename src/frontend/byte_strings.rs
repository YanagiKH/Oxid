//! Bounded byte-string decoding over retained UTF-8 source, without allocation.
use std::ops::Range;

pub(super) const MAX_BYTES: usize = super::parser::MAX_ARRAY_ELEMENTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reason {
    Escape,
    Scalar,
    LineBreak,
    Length,
}
impl Reason {
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::Escape => "invalid byte string escape",
            Self::Scalar => "byte string escape requires a Unicode scalar value",
            Self::LineBreak => "byte strings require escaped line breaks",
            Self::Length => "byte string exceeds 1024 decoded bytes",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Failure {
    pub(super) reason: Reason,
    pub(super) range: Range<usize>,
}

pub(super) struct Decoder<'a> {
    input: &'a str,
    cursor: usize,
    pending: [u8; 4],
    next: usize,
    end: usize,
    emitted: usize,
    failed: bool,
}
impl<'a> Decoder<'a> {
    pub(super) fn new(input: &'a str) -> Self {
        Self {
            input,
            cursor: 0,
            pending: [0; 4],
            next: 0,
            end: 0,
            emitted: 0,
            failed: false,
        }
    }
    fn failure(&mut self, reason: Reason, start: usize, end: usize) -> Failure {
        self.failed = true;
        let mut start = start.min(self.input.len());
        let mut end = end.min(self.input.len());
        while !self.input.is_char_boundary(start) {
            start -= 1;
        }
        while !self.input.is_char_boundary(end) {
            end += 1;
        }
        Failure {
            reason,
            range: start..end,
        }
    }
    fn hex(&mut self, start: usize) -> Result<u32, Failure> {
        let Some(byte) = self.input.as_bytes().get(self.cursor).copied() else {
            return Err(self.failure(Reason::Escape, start, self.cursor));
        };
        let value = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return Err(self.failure(Reason::Escape, start, self.cursor + 1)),
        };
        self.cursor += 1;
        Ok(u32::from(value))
    }
    fn byte(&mut self) -> Option<Result<u8, Failure>> {
        if self.next < self.end {
            let byte = self.pending[self.next];
            self.next += 1;
            return Some(Ok(byte));
        }
        let start = self.cursor;
        let byte = self.input.as_bytes().get(self.cursor).copied()?;
        self.cursor += 1;
        match byte {
            b'\n' | b'\r' => Some(Err(self.failure(Reason::LineBreak, start, self.cursor))),
            b'"' => Some(Err(self.failure(Reason::Escape, start, self.cursor))),
            b'\\' => Some(self.escape(start)),
            _ => Some(Ok(byte)),
        }
    }
    fn escape(&mut self, start: usize) -> Result<u8, Failure> {
        let Some(byte) = self.input.as_bytes().get(self.cursor).copied() else {
            return Err(self.failure(Reason::Escape, start, self.cursor));
        };
        self.cursor += 1;
        Ok(match byte {
            b'\\' | b'"' => byte,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'0' => 0,
            b'x' => {
                let high = self.hex(start)?;
                let low = self.hex(start)?;
                (high * 16 + low) as u8
            }
            b'u' => {
                if self.input.as_bytes().get(self.cursor) != Some(&b'{') {
                    return Err(self.failure(Reason::Escape, start, self.cursor + 1));
                }
                self.cursor += 1;
                let mut scalar = 0;
                let mut digits = 0;
                while self.input.as_bytes().get(self.cursor) != Some(&b'}') {
                    if digits == 6 {
                        return Err(self.failure(Reason::Escape, start, self.cursor + 1));
                    }
                    scalar = scalar * 16 + self.hex(start)?;
                    digits += 1;
                }
                self.cursor += 1;
                if digits == 0 {
                    return Err(self.failure(Reason::Escape, start, self.cursor));
                }
                let Some(value) = char::from_u32(scalar) else {
                    return Err(self.failure(Reason::Scalar, start, self.cursor));
                };
                self.end = value.encode_utf8(&mut self.pending).len();
                self.next = 1;
                self.pending[0]
            }
            _ => return Err(self.failure(Reason::Escape, start, self.cursor)),
        })
    }
}
impl Iterator for Decoder<'_> {
    type Item = Result<u8, Failure>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let start = self.cursor;
        let result = self.byte()?;
        Some(match result {
            Ok(_) if self.emitted == MAX_BYTES => {
                Err(self.failure(Reason::Length, start, self.cursor))
            }
            Ok(byte) => {
                self.emitted += 1;
                Ok(byte)
            }
            Err(error) => Err(error),
        })
    }
}

pub(super) fn length(input: &str) -> Result<u16, Failure> {
    let mut count = 0;
    for byte in Decoder::new(input) {
        byte?;
        count += 1;
    }
    u16::try_from(count).map_err(|_| Failure {
        reason: Reason::Length,
        range: 0..input.len(),
    })
}

/// Resolution validates and then decodes the retained spelling. Charge both
/// source-byte walks and every produced byte before either walk or allocation.
pub(super) fn resolve_work(input_bytes: usize, decoded_bytes: u16) -> Option<u64> {
    u64::try_from(input_bytes)
        .ok()?
        .checked_mul(2)?
        .checked_add(u64::from(decoded_bytes))?
        .checked_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn decode(input: &str) -> Result<Vec<u8>, Failure> {
        Decoder::new(input).collect()
    }
    #[test]
    fn byte_strings_resolution_pays_both_spelling_walks_and_decoded_bytes() {
        assert_eq!(resolve_work(0, 0), Some(1));
        assert_eq!(resolve_work(10240, 1024), Some(21505));
        #[cfg(target_pointer_width = "64")]
        assert_eq!(resolve_work(usize::MAX, 1), None);
    }
    #[test]
    fn byte_strings_raw_utf8_and_all_octet_escapes_are_exact() {
        assert_eq!(decode("A¢€👋").unwrap(), "A¢€👋".as_bytes());
        assert_eq!(decode(r#"\0\n\r\t\\\""#).unwrap(), b"\0\n\r\t\\\"");
        let escaped: String = (0..=255).map(|n| format!("\\x{n:02X}")).collect();
        assert_eq!(decode(&escaped).unwrap(), (0..=255).collect::<Vec<u8>>());
    }
    #[test]
    fn byte_strings_unicode_scalar_edges_encode_to_utf8() {
        for value in [0, 127, 128, 2047, 2048, 55295, 57344, 65535, 65536, 1114111] {
            let escaped = format!("\\u{{{value:X}}}");
            assert_eq!(
                decode(&escaped).unwrap(),
                char::from_u32(value).unwrap().to_string().as_bytes()
            );
        }
    }
    #[test]
    fn byte_strings_limits_apply_to_decoded_bytes() {
        for raw in ["A".repeat(1024), "\\xFF".repeat(1024), "👋".repeat(256)] {
            assert_eq!(length(&raw).unwrap(), 1024);
            assert_eq!(length(&(raw + "a")).unwrap_err().reason, Reason::Length);
        }
        assert_eq!(length("").unwrap(), 0);
    }
    #[test]
    fn byte_strings_malformed_escapes_terminate_and_blame_utf8_boundaries() {
        for raw in [
            "\\",
            "\\é",
            "\\x",
            "\\x0",
            "\\x你",
            "\\u",
            "\\u{",
            "\\u{}",
            "\\u{1234567}",
            "\\u{D800}",
            "\\u{110000}",
            "a\nb",
            "a\rb",
        ] {
            let error = length(raw).unwrap_err();
            assert!(raw.is_char_boundary(error.range.start));
            assert!(raw.is_char_boundary(error.range.end));
            let mut decoder = Decoder::new(raw);
            for value in decoder.by_ref() {
                if value.is_err() {
                    break;
                }
            }
            assert!(decoder.next().is_none());
            assert!(decoder.next().is_none());
        }
    }
}
