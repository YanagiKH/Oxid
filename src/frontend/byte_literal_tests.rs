use super::*;
use crate::frontend::source::{SourceFileId, SourceMap};

fn source(text: &str) -> SourceMap {
    let mut sources = SourceMap::new();
    sources.add("byte-literal.ox".into(), text.into());
    sources
}

#[test]
fn byte_literal_scanner_fixed_values_and_original_atom_spans() {
    let sources = source(r##"b"A\n\r\t\0\\\"\x00\x7f\x80\xFF\xaB""##);
    let file = sources.get(SourceFileId(0));
    let mut scanner = Scanner::new(file, file.span(0, file.text().len())).unwrap();
    // Independent literal oracle: no expected byte or endpoint is decoded by
    // the scanner being tested, or by another language's escape parser.
    let expected = [
        (65, 2, 3),
        (10, 3, 5),
        (13, 5, 7),
        (9, 7, 9),
        (0, 9, 11),
        (92, 11, 13),
        (34, 13, 15),
        (0, 15, 19),
        (127, 19, 23),
        (128, 23, 27),
        (255, 27, 31),
        (171, 31, 35),
    ];
    for (value, start, end) in expected {
        assert_eq!(
            scanner.next_atom().unwrap(),
            Some(Atom {
                value,
                origin: file.span(start, end),
            })
        );
    }
    assert_eq!(scanner.next_atom().unwrap(), None);
    assert_eq!(scanner.next_atom().unwrap(), None);
}

#[test]
fn byte_literal_scanner_empty_printable_edges_and_shifted_source() {
    for (text, start, end, expected) in [
        (" b\"\";", 1, 4, &[][..]),
        (" b\" ~/[]'\";", 1, 10, &[32, 126, 47, 91, 93, 39][..]),
    ] {
        let sources = source(text);
        let file = sources.get(SourceFileId(0));
        let mut scanner = Scanner::new(file, file.span(start, end)).unwrap();
        for (ordinal, &value) in expected.iter().enumerate() {
            assert_eq!(
                scanner.next_atom().unwrap(),
                Some(Atom {
                    value,
                    origin: file.span(start + 2 + ordinal, start + 3 + ordinal),
                })
            );
        }
        assert_eq!(scanner.next_atom().unwrap(), None);
    }
}

#[test]
fn byte_literal_scanner_rejects_closed_spelling_exclusions() {
    for text in [
        r##"b"\u{41}""##,
        r##"b"\U00000041""##,
        r##"b"\q""##,
        r##"b"\x""##,
        r##"b"\x0""##,
        r##"b"\xGG""##,
        r##"b"\x０0""##,
        r##"b"\x0é""##,
        r##"b"é""##,
        "b\"\n\"",
        "b\"\r\"",
        "b\"\t\"",
        "b\"\0\"",
        "b\"\u{7f}\"",
        "b\"\\\n\"",
        "b\"\\\r\n\"",
        "b\"\\\"",
        "b\"a\"b\"",
    ] {
        let sources = source(text);
        let file = sources.get(SourceFileId(0));
        let mut scanner = Scanner::new(file, file.span(0, text.len())).unwrap();
        let error = loop {
            match scanner.next_atom() {
                Ok(Some(_)) => (),
                Ok(None) => panic!("accepted {text:?}"),
                Err(error) => break error,
            }
        };
        assert!(file.try_text(error.span).is_some(), "{text:?}: {error:?}");
    }
}

#[test]
fn byte_literal_scanner_rejects_envelopes_suffixes_and_foreign_spans() {
    for text in [
        "", "b", "b\"", "b\"abc", "b \"a\"", "B\"a\"", "br\"a\"", "\"a\"",
    ] {
        let sources = source(text);
        let file = sources.get(SourceFileId(0));
        assert!(
            Scanner::new(file, file.span(0, text.len())).is_err(),
            "{text:?}"
        );
    }
    for suffix in ["u8", "_", "9", "é"] {
        let sources = source(&format!("b\"a\"{suffix}"));
        let file = sources.get(SourceFileId(0));
        let error = Scanner::new(file, file.span(0, 4)).unwrap_err();
        assert_eq!(error.reason, Reason::Suffix);
        assert!(file.try_text(error.span).is_some());
    }
    let sources = source("b\"a\"");
    let file = sources.get(SourceFileId(0));
    let foreign = Span {
        file: SourceFileId(1),
        start: 0,
        end: 4,
    };
    assert!(Scanner::new(file, foreign).is_err());
    assert!(Scanner::new(
        file,
        Span {
            file: SourceFileId(0),
            start: 4,
            end: 0
        }
    )
    .is_err());
}

#[test]
fn byte_literal_scanner_hex_error_spans_cover_available_atom_and_utf8() {
    for (text, expected_end, spelling) in [
        (r##"b"\xG0""##, 6, r"\xG0"),
        (r##"b"\x0g""##, 6, r"\x0g"),
        (r##"b"\x4""##, 5, r"\x4"),
        (r##"b"\x""##, 4, r"\x"),
        (r##"b"\xé""##, 6, r"\xé"),
        (r##"b"\x0é""##, 7, r"\x0é"),
        (r##"b"\x０0""##, 7, r"\x０"),
        (r##"b"\x0😀""##, 9, r"\x0😀"),
        (r##"b"\x😀""##, 8, r"\x😀"),
    ] {
        let sources = source(text);
        let file = sources.get(SourceFileId(0));
        let mut scanner = Scanner::new(file, file.span(0, text.len())).unwrap();
        let error = scanner.next_atom().unwrap_err();
        assert_eq!(error.reason, Reason::Hex, "{text:?}");
        assert_eq!(error.span, file.span(2, expected_end), "{text:?}");
        assert_eq!(file.text_at(error.span), spelling, "{text:?}");
        assert!(
            error.span.end < text.len(),
            "closing quote included: {text:?}"
        );
    }
}
