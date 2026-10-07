//! Test-only, untrusted controls for the private Verify terminal. The explicit
//! row tables below are synthetic inputs, not producer-success fixtures. None
//! constructs or repairs a compiler owner, HIR, typed program, or index.
use super::*;
use crate::frontend::{
    declaration_index::IndexLimits,
    lexer, parser,
    project::budget::Allocator,
    source::{SourceMap, SourceView},
    typeck,
};

fn put_word(bytes: &mut [u8], column: usize, reference: usize, value: i32) {
    for (plane, byte) in value.to_le_bytes().into_iter().enumerate() {
        bytes[COLUMN_STARTS[column] + plane * CELLS + reference - 1] = byte;
    }
}

// A small encoder for these fixed, hand-authored adversarial/control tables.
// Each tuple supplies all eight OPA fields, resolution, and semantic value.
// No source parser, resolver, typechecker, or canonical owner supplies a cell.
fn hand_authored_frame(text: &str, rows: &[([u8; 8], i32, i32)]) -> [u8; SUCCESS_BYTES] {
    assert!(text.len() <= MAX_ROWS && rows.len() <= MAX_ROWS);
    let mut bytes = [0; SUCCESS_BYTES];
    bytes[..4].copy_from_slice(b"OPA1");
    bytes[8] = rows.len() as u8;
    bytes[9] = u8::from(!rows.is_empty());
    bytes[10] = text.len() as u8;
    bytes[OPA_BYTES..OPA_BYTES + 4].copy_from_slice(b"STF1");
    bytes[OPA_BYTES + 5] = bytes[8];
    for (index, &(fields, resolution, semantic)) in rows.iter().enumerate() {
        let [kind, start, end, next, a, b, c, d] = fields.map(i32::from);
        for (column, value) in [
            kind | (start << 6) | (end << 14) | (next << 22),
            a | (b << 8),
            c | (d << 8),
            resolution,
            semantic,
        ]
        .into_iter()
        .enumerate()
        {
            put_word(&mut bytes, column, index + 1, value);
        }
    }
    bytes
}

fn authentic_type_diagnostics(text: &str, bytes: &[u8]) -> Vec<Diagnostic> {
    let mut sources = SourceMap::new();
    let id = sources.add("verify-diagnostics.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();

    // Independently prove the supplied syntax/resolution candidate equals the
    // genuine canonical HIR. This observation neither repairs the wire nor
    // makes an owner available to the following Verify request.
    let mut comparison_allocator = Allocator::default();
    let comparison = leaf::denied(
        owner,
        text.as_bytes(),
        bytes,
        &mut comparison_allocator,
        IndexLimits::default(),
    );
    let Err(leaf::Rejected::Compared(facts)) = comparison else {
        panic!("diagnostic input must pass complete HIR equality: {comparison:?}");
    };
    assert!(facts.candidate.equal);
    assert!(comparison_allocator.attempts > 0);

    // The baseline takes only genuine parser/source ownership and resolution.
    // It is never used to create, adjust, or fill observation cells.
    let expected = typeck::check(hir::resolve_sources(owner).unwrap()).unwrap_err();
    assert!(!expected.is_empty());
    let mut allocator = Allocator::default();
    let result = leaf::verify(
        owner,
        text.as_bytes(),
        bytes,
        &mut allocator,
        IndexLimits::default(),
    );
    let Err(leaf::VerifyRejected::Terminal(candidate::VerifyRejected::Typed(actual))) = result
    else {
        panic!("candidate-owned HIR must reach the genuine checker: {result:?}");
    };
    assert_eq!(allocator.attempts, comparison_allocator.attempts);
    assert_eq!(
        actual.len(),
        expected.len(),
        "retain the entire error vector"
    );
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual.code, expected.code, "diagnostic {index}");
        assert_eq!(actual.stage, expected.stage, "diagnostic {index}");
        assert_eq!(actual.message, expected.message, "diagnostic {index}");
        assert_eq!(actual.primary, expected.primary, "diagnostic {index}");
        assert_eq!(actual.secondary, expected.secondary, "diagnostic {index}");
        assert_eq!(actual.notes, expected.notes, "diagnostic {index}");
        assert_eq!(actual.render_json(&sources), expected.render_json(&sources));
    }
    actual
}

#[allow(clippy::result_large_err)]
fn verify_control(text: &str, bytes: &[u8]) -> leaf::VerifyFacts {
    let mut sources = SourceMap::new();
    let id = sources.add("verify-control.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let mut allocator = Allocator::default();
    let facts = leaf::verify(
        owner,
        text.as_bytes(),
        bytes,
        &mut allocator,
        IndexLimits::default(),
    )
    .unwrap();
    assert!(facts.verified.candidate.equal);
    assert_eq!(facts.verified.typed_cells, usize::from(bytes[8]));
    assert_eq!(
        facts.total_work,
        facts.source_work
            + facts.canonical_work
            + facts.verified.candidate.charged_work
            + facts.verified.pass_work
            + facts.verified.typed_work
    );
    facts
}

#[test]
fn checked_hir_import_verify_authentic_type_error_from_mutated_boolean_fixture() {
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/scalar-boolean-source.txt"
    ));
    let fixture = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/scalar-boolean-success.bin"
    ));
    let mut text = original.to_owned();
    assert_eq!(&text[35..36], "a");
    text.replace_range(35..36, "1");
    let mut bytes = fixture.to_vec();
    let header = Wire::decode(fixture, original.len())
        .unwrap()
        .word(0, 10)
        .unwrap();
    assert_eq!(header & 63, 19); // original name expression, row 11
    put_word(&mut bytes, 0, 11, (header & !63) | 15); // integer expression
    put_word(&mut bytes, 3, 11, 1); // supplied integer magnitude
    assert_eq!(text.len(), original.len());
    // The original supplied bool type remains stale. Genuine type errors must
    // propagate before any comparison against that supplied semantic table.
    assert_eq!(&bytes[COLUMN_STARTS[4]..], &fixture[COLUMN_STARTS[4]..]);
    let diagnostics = authentic_type_diagnostics(&text, &bytes);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0300");
    assert_eq!(
        diagnostics[0].message,
        "type mismatch: expected bool, found i32"
    );
    let span = diagnostics[0].primary.unwrap();
    assert_eq!((span.start, span.end), (35, 36));
}

#[test]
fn checked_hir_import_verify_authentic_diagnostic_vector_preserves_function_order() {
    let text = "fn f()->bool{return 1;}fn g()->i32{return true;}";
    // Synthetic untrusted request, not a successful producer observation.
    // These ten fixed OPA rows describe two independently ill-typed functions;
    // the deliberately zero semantic column cannot construct typed authority.
    let bytes = hand_authored_frame(
        text,
        &[
            ([1, 3, 4, 6, 0, 0, 2, 3], 1, 0),
            ([3, 8, 12, 0, 7, 0, 0, 0], 1, 0),
            ([5, 12, 23, 0, 13, 4, 0, 0], 0, 0),
            ([10, 13, 22, 0, 5, 0, 0, 0], 0, 0),
            ([15, 20, 21, 0, 11, 0, 0, 1], 1, 0),
            ([1, 26, 27, 0, 0, 0, 7, 8], 2, 0),
            ([3, 31, 34, 0, 20, 0, 0, 0], 2, 0),
            ([5, 34, 48, 0, 26, 9, 0, 0], 0, 0),
            ([10, 35, 47, 0, 10, 0, 0, 0], 0, 0),
            ([17, 42, 46, 0, 0, 0, 0, 1], 0, 0),
        ],
    );
    let diagnostics = authentic_type_diagnostics(text, &bytes);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["E0300", "E0300"]
    );
    assert_eq!(
        diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        [
            "type mismatch: expected bool, found i32",
            "type mismatch: expected i32, found bool",
        ]
    );
    assert_eq!(
        diagnostics
            .iter()
            .map(|d| {
                let span = d.primary.unwrap();
                (span.start, span.end)
            })
            .collect::<Vec<_>>(),
        [(20, 21), (42, 46)]
    );
}

#[test]
fn checked_hir_import_verify_empty_synthetic_control() {
    let bytes = hand_authored_frame("", &[]);
    let facts = verify_control("", &bytes);
    assert_eq!(facts.verified.functions, 0);
    assert_eq!(facts.verified.typed_cells, 0);
    assert_eq!(facts.verified.candidate.allocation.reserves, 0);
}

#[test]
fn checked_hir_import_verify_recursive_synthetic_controls() {
    // These hand-authored observations exercise self recursion, a forward
    // call, and a mutual cycle. Verify does not run the resulting program.
    let self_recursive = "fn f()->i32{return f();}";
    let self_bytes = hand_authored_frame(
        self_recursive,
        &[
            ([1, 3, 4, 0, 0, 0, 2, 3], 1, 2),
            ([3, 8, 11, 0, 7, 0, 0, 0], 2, 0),
            ([5, 11, 24, 0, 15, 4, 0, 0], 0, 2),
            ([10, 12, 23, 0, 5, 0, 0, 0], 0, 0),
            ([20, 19, 22, 0, 11, 0, 0, 1], 1, 2),
        ],
    );
    assert_eq!(
        verify_control(self_recursive, &self_bytes)
            .verified
            .functions,
        1
    );

    let mutual = "fn f()->i32{return g();}fn g()->i32{return f();}";
    let mutual_bytes = hand_authored_frame(
        mutual,
        &[
            ([1, 3, 4, 6, 0, 0, 2, 3], 1, 2),
            ([3, 8, 11, 0, 7, 0, 0, 0], 2, 0),
            ([5, 11, 24, 0, 15, 4, 0, 0], 0, 2),
            ([10, 12, 23, 0, 5, 0, 0, 0], 0, 0),
            ([20, 19, 22, 0, 11, 0, 0, 1], 6, 2),
            ([1, 27, 28, 0, 0, 0, 7, 8], 2, 2),
            ([3, 32, 35, 0, 22, 0, 0, 0], 2, 0),
            ([5, 35, 48, 0, 30, 9, 0, 0], 0, 2),
            ([10, 36, 47, 0, 10, 0, 0, 0], 0, 0),
            ([20, 43, 46, 0, 26, 0, 0, 1], 1, 2),
        ],
    );
    assert_eq!(verify_control(mutual, &mutual_bytes).verified.functions, 2);
}
