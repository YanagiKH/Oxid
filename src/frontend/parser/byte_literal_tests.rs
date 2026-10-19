use super::*;
use crate::frontend::{
    declaration_index::{SourceOwner, WorkMeter},
    lexer,
    source::{SourceFileId, SourceMap, SourceView},
};

fn source(text: &str) -> SourceMap {
    let mut sources = SourceMap::new();
    sources.add("byte-literal-syntax.ox".into(), text.into());
    sources
}

fn typed(file: &SourceFile) -> Result<Program, Vec<Diagnostic>> {
    let tokens = lexer::lex(file).map_err(|error| vec![*error])?;
    parse_typed_counted(
        file,
        tokens,
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut Allocator::default(),
        &mut SyntaxStorage::default(),
    )
    .map(|(program, _)| program)
}

#[test]
fn byte_literal_parser_compact_leaf_original_token_and_owned_routing() {
    for (literal, length) in [(r##"b"""##, 0), (r##"b"a\0\xFf""##, 3)] {
        let sources = source(&format!("fn main()->(){{let bytes={literal};return;}}"));
        let file = sources.get(SourceFileId(0));
        let program = typed(file).unwrap();
        assert_eq!(program.expressions.len(), 1);
        let expression = &program.expressions[0];
        assert!(
            matches!(expression.kind, ExprKind::ByteLiteral { decoded_len } if decoded_len == length)
        );
        assert_eq!(file.text_at(expression.span), literal);
        assert!(program.validate_spans_and_ids(|span| file.try_text(span).is_some()));
        assert!(program.uses_owned_syntax(file));
        assert!(
            SourceOwner::original(file, &program, SourceView::Single(file))
                .unwrap()
                .owned(&WorkMeter::default())
                .unwrap()
        );
        let token = program
            .tokens
            .iter()
            .find(|token| token.kind == Kind::ByteString)
            .unwrap();
        assert_eq!(token.span, expression.span);
    }
}

#[test]
fn byte_literal_parser_lexical_and_grammar_rejections() {
    for literal in [
        "b \"a\"",
        "B\"a\"",
        "bb\"a\"",
        "br\"a\"",
        "br#\"a\"#",
        "b\"a\"u8",
        "b\"a\"_",
        "b\"a\"b\"b\"",
        "b\"a\" \"b\"",
        "b\"a\" \"\"",
        "b\"unterminated",
        "\"ordinary\"",
        r##"b"\q""##,
        r##"b"\x0""##,
        r##"b"\xGG""##,
        r##"b"\u{41}""##,
        "b\"é\"",
        "b\"\n\"",
        "b\"\t\"",
        "b\"\\\n\"",
    ] {
        let sources = source(&format!("fn main()->(){{let bytes={literal};return;}}"));
        let file = sources.get(SourceFileId(0));
        assert!(typed(file).is_err(), "accepted {literal:?}");
    }
}

#[test]
fn byte_literal_parser_closed_array_and_scalar_routes_reject() {
    let sources = source("fn main()->(){let bytes=b\"x\";return;}");
    let file = sources.get(SourceFileId(0));
    for mode in [SourceMode::ScalarOnly, SourceMode::OwnedCandidate] {
        assert!(parse_with_mode(file, lexer::lex(file).unwrap(), mode).is_err());
    }
    assert!(parse_counted_with_arrays(
        file,
        lexer::lex(file).unwrap(),
        SourceMode::ScalarOnly,
        MAX_NODES,
        &mut Allocator::default(),
        ArraySyntaxPolicy::Enabled,
    )
    .is_err());
}

#[test]
fn byte_literal_lexer_requires_lowercase_contiguous_prefix() {
    let sources = source(r##"b"x" b "x" B"x" bb"x" br"x""##);
    let file = sources.get(SourceFileId(0));
    let tokens = lexer::lex(file).unwrap();
    let kinds: Vec<_> = tokens
        .iter()
        .filter(|token| token.kind != Kind::Trivia)
        .map(|token| token.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            Kind::ByteString,
            Kind::Ident,
            Kind::String,
            Kind::Ident,
            Kind::String,
            Kind::Ident,
            Kind::String,
            Kind::Ident,
            Kind::String,
            Kind::Eof,
        ]
    );
    assert_eq!(file.text_at(tokens[0].span), r##"b"x""##);
}

#[test]
fn byte_literal_parser_fixed_negative_diagnostic_origins() {
    // Original-source offsets relative to the exact rejected spelling. These
    // are fixed expectations, not spans or messages derived from the scanner.
    let escape = "unsupported or incomplete byte literal escape";
    let hex = "byte literal hex escapes require exactly two ASCII hex digits";
    let control = "byte literals require escapes for control bytes and newlines";
    let unsupported = "unsupported typed-preview construct `\"a\"`";
    for (spelling, code, stage, message, start, end) in [
        ("b\"\\q\"", "E0101", "parse", escape, 2, 4),
        ("b\"\\x4\"", "E0101", "parse", hex, 2, 5),
        ("b\"\\xG0\"", "E0101", "parse", hex, 2, 6),
        ("b\"\\x0g\"", "E0101", "parse", hex, 2, 6),
        ("b\"\\u{41}\"", "E0101", "parse", escape, 2, 4),
        ("b\"\\'\"", "E0101", "parse", escape, 2, 4),
        (
            "b\"é\"",
            "E0101",
            "parse",
            "byte literals accept raw printable ASCII only",
            2,
            4,
        ),
        ("b\"\u{7f}\"", "E0101", "parse", control, 2, 3),
        ("b\"\t\"", "E0101", "parse", control, 2, 3),
        ("b\"a\nb\"", "E0101", "parse", control, 3, 4),
        ("b\"a\\\nb\"", "E0101", "parse", escape, 3, 5),
        ("br\"a\"", "E0101", "parse", unsupported, 2, 5),
        (
            "b\"a\"u8",
            "E0101",
            "parse",
            "byte literal suffixes are unsupported",
            4,
            5,
        ),
        ("b \"a\"", "E0101", "parse", unsupported, 2, 5),
        ("b/**/\"a\"", "E0101", "parse", unsupported, 5, 8),
        ("B\"a\"", "E0101", "parse", unsupported, 1, 4),
        ("b\"a", "E0100", "lex", "unterminated string literal", 0, 20),
    ] {
        let prefix = "fn main()->i32{let a=";
        let sources = source(&format!("{prefix}{spelling};return a.len();}}"));
        let file = sources.get(SourceFileId(0));
        let errors = typed(file).unwrap_err();
        assert_eq!(errors.len(), 1, "{spelling:?}");
        let error = &errors[0];
        assert_eq!(
            (error.code, error.stage, error.message.as_str()),
            (code, stage, message),
            "{spelling:?}"
        );
        assert_eq!(
            error.primary,
            Some(file.span(prefix.len() + start, prefix.len() + end)),
            "{spelling:?}"
        );
    }
}
