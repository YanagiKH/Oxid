use super::*;

#[test]
fn byte_literal_format_preserves_exact_escape_spelling_and_idempotence() {
    let input = r##"fn main()->(){let bytes=b"a\xFF\xff\x00\0\"\\[]{}";let empty=b"";return;}"##;
    let expected = "fn main() -> () { let bytes = b\"a\\xFF\\xff\\x00\\0\\\"\\\\[]{}\"; let empty = b\"\"; return; }\n";
    let output = tests::formatted(input).unwrap();
    assert_eq!(output, expected);
    assert_eq!(tests::formatted(&output).unwrap(), output);
    let sources = tests::source(&output);
    let file = sources.get(crate::frontend::source::SourceFileId(0));
    let program = parse(file, &mut Allocator::default()).unwrap();
    assert!(matches!(
        program.expressions[0].kind,
        ExprKind::ByteLiteral { decoded_len: 11 }
    ));
    assert!(matches!(
        program.expressions[1].kind,
        ExprKind::ByteLiteral { decoded_len: 0 }
    ));
}

#[test]
fn byte_literal_formatter_rejects_invalid_source_spelling() {
    for literal in [
        r##"b"\u{41}""##,
        r##"b"\x0""##,
        "b\"é\"",
        "b\"\n\"",
        "b\"a\"u8",
    ] {
        let text = format!("fn main()->(){{let bytes={literal};}}");
        assert!(tests::formatted(&text).is_err(), "{literal}");
    }
}
