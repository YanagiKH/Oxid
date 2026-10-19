//! Allocation-free decoding of the closed, original-source `b"..."` spelling.
//!
//! This scanner proves spelling only. Its caller must independently establish
//! source ownership, the original AST occurrence, and any emitted witnesses.
use super::source::{SourceFile, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Atom {
    pub(super) value: u8,
    pub(super) origin: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reason {
    Envelope,
    Suffix,
    RawControl,
    NonAscii,
    RawQuote,
    Escape,
    Hex,
    Overflow,
}

impl Reason {
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::Envelope => "expected a complete adjacent byte literal b\"...\"",
            Self::Suffix => "byte literal suffixes are unsupported",
            Self::RawControl => "byte literals require escapes for control bytes and newlines",
            Self::NonAscii => "byte literals accept raw printable ASCII only",
            Self::RawQuote => "unescaped quote inside byte literal",
            Self::Escape => "unsupported or incomplete byte literal escape",
            Self::Hex => "byte literal hex escapes require exactly two ASCII hex digits",
            Self::Overflow => "byte literal source offset overflow",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Error {
    pub(super) reason: Reason,
    pub(super) span: Span,
}

/// One borrowed token and one cursor, with no decoded buffer or atom table.
#[derive(Debug)]
pub(super) struct Scanner<'a> {
    spelling: &'a str,
    literal: Span,
    cursor: usize,
}

impl<'a> Scanner<'a> {
    pub(super) fn new(source: &'a SourceFile, literal: Span) -> Result<Self, Error> {
        let spelling = source.try_text(literal).ok_or(Error {
            reason: Reason::Envelope,
            // Invalid/foreign input cannot supply a valid diagnostic origin.
            span: source.span(0, 0),
        })?;
        let invalid = Error {
            reason: Reason::Envelope,
            span: literal,
        };
        let bytes = spelling.as_bytes();
        if bytes.len() < 3 || !bytes.starts_with(b"b\"") || bytes.last() != Some(&b'"') {
            return Err(invalid);
        }
        // Reject an attached suffix even if a caller supplies only the quoted
        // prefix span. UTF-8 character boundaries remain valid on every error.
        if let Some(suffix) = source.text()[literal.end..].chars().next() {
            if suffix.is_alphanumeric() || suffix == '_' {
                let end = literal.end.checked_add(suffix.len_utf8()).ok_or(Error {
                    reason: Reason::Overflow,
                    span: literal,
                })?;
                return Err(Error {
                    reason: Reason::Suffix,
                    span: Span {
                        file: literal.file,
                        start: literal.end,
                        end,
                    },
                });
            }
        }
        Ok(Self {
            spelling,
            literal,
            cursor: 2,
        })
    }

    fn error(&self, reason: Reason, start: usize, end: usize) -> Error {
        match (
            self.literal.start.checked_add(start),
            self.literal.start.checked_add(end),
        ) {
            (Some(start), Some(end)) => Error {
                reason,
                span: Span {
                    file: self.literal.file,
                    start,
                    end,
                },
            },
            _ => Error {
                reason: Reason::Overflow,
                span: self.literal,
            },
        }
    }

    /// A valid atom consumes one, two or four source bytes. A closing quote is
    /// never an atom; the entire interior must be consumed before success.
    pub(super) fn next_atom(&mut self) -> Result<Option<Atom>, Error> {
        let end = self.spelling.len() - 1; // Constructor established len >= 3.
        if self.cursor == end {
            return Ok(None);
        }
        let start = self.cursor;
        let bytes = self.spelling.as_bytes();
        let byte = bytes[start];
        let (value, width) = match byte {
            b'\\' => {
                let escaped = start
                    .checked_add(1)
                    .ok_or_else(|| self.error(Reason::Overflow, start, end))?;
                if escaped >= end {
                    return Err(self.error(Reason::Escape, start, end));
                }
                match bytes[escaped] {
                    b'\\' => (b'\\', 2),
                    b'"' => (b'"', 2),
                    b'n' => (b'\n', 2),
                    b'r' => (b'\r', 2),
                    b't' => (b'\t', 2),
                    b'0' => (0, 2),
                    b'x' => {
                        let next = start
                            .checked_add(4)
                            .ok_or_else(|| self.error(Reason::Overflow, start, end))?;
                        if next > end {
                            return Err(self.error(Reason::Hex, start, end));
                        }
                        let high = hex(bytes[start + 2]);
                        let low = hex(bytes[start + 3]);
                        let (Some(high), Some(low)) = (high, low) else {
                            // Cover the complete four-byte escape candidate.
                            // If its endpoint splits an invalid UTF-8 digit,
                            // include that whole character, never the closing
                            // quote. At most three continuation bytes follow.
                            let mut error_end = next;
                            while error_end < end && !self.spelling.is_char_boundary(error_end) {
                                error_end = error_end
                                    .checked_add(1)
                                    .ok_or_else(|| self.error(Reason::Overflow, start, end))?;
                            }
                            return Err(self.error(Reason::Hex, start, error_end));
                        };
                        ((high << 4) | low, 4)
                    }
                    _ => {
                        let width = self.spelling[escaped..].chars().next().unwrap().len_utf8();
                        return Err(self.error(Reason::Escape, start, escaped + width));
                    }
                }
            }
            b'"' => return Err(self.error(Reason::RawQuote, start, start + 1)),
            0x20..=0x7e => (byte, 1),
            0x00..=0x1f | 0x7f => return Err(self.error(Reason::RawControl, start, start + 1)),
            _ => {
                let width = self.spelling[start..].chars().next().unwrap().len_utf8();
                return Err(self.error(Reason::NonAscii, start, start + width));
            }
        };
        let next = start
            .checked_add(width)
            .ok_or_else(|| self.error(Reason::Overflow, start, end))?;
        let origin = Span {
            file: self.literal.file,
            start: self
                .literal
                .start
                .checked_add(start)
                .ok_or_else(|| self.error(Reason::Overflow, start, end))?,
            end: self
                .literal
                .start
                .checked_add(next)
                .ok_or_else(|| self.error(Reason::Overflow, start, end))?,
        };
        self.cursor = next;
        Ok(Some(Atom { value, origin }))
    }
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Conservative source-work visits for one complete pass, including envelope,
/// suffix lookahead, atom dispatch and EOF. Charge independently on each pass.
pub(super) fn scan_work(literal: Span) -> Option<usize> {
    literal
        .end
        .checked_sub(literal.start)?
        .checked_mul(4)?
        .checked_add(16)
}

// Named conservative live value/return carriers, not machine stack or RSS.
// Includes caller count/ordinal/length roles; borrowed source bytes are owned
// and charged by the source owner, never allocated by the scanner.
#[allow(dead_code)]
struct ScannerCarriers<'a> {
    source: &'a SourceFile,
    spelling: &'a str,
    source_bytes: &'a [u8],
    suffix_chars: std::str::Chars<'a>,
    error_chars: std::str::Chars<'a>,
    scanner: Scanner<'a>,
    constructed: Result<Scanner<'a>, Error>,
    atom: Atom,
    atom_return: Result<Option<Atom>, Error>,
    error: Error,
    span: Span,
    offsets_and_counts: [usize; 12],
    checked_offsets: [Option<usize>; 4],
    hex_returns: [Option<u8>; 2],
    suffix: Option<char>,
}

pub(super) const fn scanner_scratch_bytes() -> usize {
    std::mem::size_of::<ScannerCarriers<'_>>()
}

#[cfg(test)]
#[path = "byte_literal_tests.rs"]
mod tests;
