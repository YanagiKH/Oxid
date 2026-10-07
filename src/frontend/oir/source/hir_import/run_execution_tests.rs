//! Private Run parity through genuine source ownership and the complete import
//! leaf. Captures are unchanged producer observations; explicit tables below
//! are hand-authored, untrusted controls, never compiler-derived wire data.
use super::*;
use crate::frontend::{
    declaration_index::IndexLimits,
    lexer,
    oir::{execute, owned, source::check_source, RunFailure, Scalar},
    parser,
    project::budget::Allocator,
    source::{SourceMap, SourceView},
};

fn assert_runtime_parity(
    facts: &leaf::VerifyFacts,
    ordinary: Result<Scalar, Box<Diagnostic>>,
    sources: &SourceMap,
) {
    assert!(facts.verified.candidate.equal);
    assert!(facts.verified.entry_work > 0);
    assert_eq!(
        facts.total_work,
        facts.source_work
            + facts.canonical_work
            + facts.verified.candidate.charged_work
            + facts.verified.pass_work
            + facts.verified.typed_work
            + facts.verified.entry_work
    );
    match (facts.verified.runtime.as_ref().unwrap(), ordinary) {
        (Ok(actual), Ok(expected)) => assert_eq!(*actual, expected),
        (Err(actual), Err(expected)) => {
            let actual = actual.diagnostic(sources);
            assert_eq!(actual.code, expected.code);
            assert_eq!(actual.stage, expected.stage);
            assert_eq!(actual.message, expected.message);
            assert_eq!(actual.primary, expected.primary);
            assert_eq!(actual.secondary, expected.secondary);
            assert_eq!(actual.notes, expected.notes);
            assert_eq!(actual.render_json(sources), expected.render_json(sources));
        }
        (actual, expected) => panic!("import/source runtime mismatch: {actual:?} / {expected:?}"),
    }
}

// Returning only fixed facts also proves the result survives the source map,
// AST, original owner and ordinary checked-program owners being dropped.
fn original_parity(text: &str, bytes: &[u8]) -> leaf::VerifyFacts {
    assert!(text.is_ascii() && text.len() <= MAX_ROWS);
    let mut sources = SourceMap::new();
    let id = sources.add("private-run-parity.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    // Source/AST/capture and caller-retained validation trace are outside this
    // interval. Only the fixed outcome survives all imported compiler/runtime
    // owners, including expected arithmetic/fuel/frame failure teardown.
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(32).unwrap();
    let trace_capacity = allocator.trace.capacity();
    let imported_runtime = execute::measurement::begin();
    let mut result = None;
    let allocation = owned::hir_import_measure_allocations(|| {
        result = Some(leaf::run(
            owner,
            text.as_bytes(),
            bytes,
            &mut allocator,
            IndexLimits::default(),
        ));
    });
    let imported_runtime = imported_runtime.finish();
    let facts = result.unwrap().unwrap();
    assert_eq!(allocation.2, 0);
    assert_eq!(allocator.trace.capacity(), trace_capacity);
    assert!(!allocator.observer_trace_overflow);
    assert!(owned::hir_import_allocation_observers_idle());
    println!(
        "HIR_IMPORT_RUN_OUTCOME_CLEANUP bytes={} runtime={:?} allocation={allocation:?}",
        text.len(),
        facts.verified.runtime
    );
    assert_eq!(facts.verified.typed_cells, usize::from(bytes[8]));
    let checked = check_source(source, &ast, &sources).unwrap();
    let ordinary_runtime = execute::measurement::begin();
    let ordinary = checked.run();
    assert_eq!(imported_runtime, ordinary_runtime.finish());
    assert!(!imported_runtime.observation_overflowed);
    assert_runtime_parity(&facts, ordinary, &sources);
    facts
}

macro_rules! capture {
    ($name:literal) => {
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/",
                $name,
                "-source.txt"
            )),
            &include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/",
                $name,
                "-success.bin"
            ))[..],
        )
    };
}

#[test]
fn checked_hir_import_run_genuine_rich_capture_matches_source() {
    let (text, bytes) = capture!("rich");
    let facts = original_parity(text, bytes);
    assert_eq!(facts.verified.runtime, Some(Ok(Scalar::I32(1))));
    assert_eq!(facts.verified.entry_work, 33_792);
    assert_eq!(facts.total_work, 1_310_659);
}

#[test]
fn checked_hir_import_run_genuine_library_captures_keep_missing_main() {
    for (text, bytes) in [
        capture!("scalar-arithmetic"),
        capture!("scalar-boolean"),
        capture!("scalar-comparison"),
        capture!("scalar-unit"),
        capture!("scalar-loop"),
        capture!("scalar-assignment"),
    ] {
        let facts = original_parity(text, bytes);
        assert_eq!(facts.verified.runtime, Some(Err(RunFailure::Entry(None))));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn checked_hir_import_run_genuine_public_capture_matches_project() {
    use crate::frontend::{declaration_index::WorkMeter, oir::project, project::SyntaxFlavor};
    let (text, bytes) = capture!("public");
    assert!(text.is_ascii() && text.len() <= MAX_ROWS);
    let facts = {
        let sources = super::tests::project(text);
        assert_eq!(sources.syntax_flavor(), SyntaxFlavor::ProjectSyntax);
        let owner = SourceOwner::project(&sources);
        assert_eq!(owner.count(), 1);
        let facts = leaf::run(
            owner,
            text.as_bytes(),
            bytes,
            &mut Allocator::default(),
            IndexLimits::default(),
        )
        .unwrap();
        let ordinary = project::check_project_executable(
            &sources,
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap()
        .run();
        assert_runtime_parity(&facts, ordinary, sources.sources());
        facts
    };
    assert_eq!(facts.verified.runtime, Some(Ok(Scalar::I32(1))));
}

// Encode only the explicit hand-authored rows below. The parser, resolver,
// checker and canonical HIR never supply or repair any observation cell.
fn hand_authored_frame(text: &str, rows: &[([u8; 8], i32, i32)]) -> [u8; SUCCESS_BYTES] {
    assert!(text.is_ascii() && text.len() <= MAX_ROWS && rows.len() <= MAX_ROWS);
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
            for (plane, byte) in value.to_le_bytes().into_iter().enumerate() {
                bytes[COLUMN_STARTS[column] + plane * CELLS + index] = byte;
            }
        }
    }
    bytes
}

#[test]
fn checked_hir_import_run_hand_authored_bool_and_unit_controls() {
    let boolean = "fn main()->bool{return true;}";
    let boolean_bytes = hand_authored_frame(
        boolean,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 1),
            ([3, 11, 15, 0, 7, 0, 0, 0], 1, 0),
            ([5, 15, 29, 0, 13, 4, 0, 0], 0, 2),
            ([10, 16, 28, 0, 5, 0, 0, 0], 0, 0),
            ([17, 23, 27, 0, 0, 0, 0, 1], 0, 1),
        ],
    );
    let boolean_facts = original_parity(boolean, &boolean_bytes);
    assert_eq!(boolean_facts.verified.runtime, Some(Ok(Scalar::Bool(true))));

    let unit = "fn main()->(){return;}";
    let unit_bytes = hand_authored_frame(
        unit,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 3),
            ([4, 11, 13, 0, 0, 0, 0, 0], 3, 0),
            ([5, 13, 22, 0, 12, 4, 0, 0], 0, 2),
            ([10, 14, 21, 0, 0, 0, 0, 0], 0, 0),
        ],
    );
    let unit_facts = original_parity(unit, &unit_bytes);
    assert_eq!(unit_facts.verified.runtime, Some(Ok(Scalar::Unit)));
}

#[test]
fn checked_hir_import_run_hand_authored_empty_and_wrong_arity_controls() {
    let empty = original_parity("", &hand_authored_frame("", &[]));
    assert_eq!(empty.verified.runtime, Some(Err(RunFailure::Entry(None))));

    let text = "fn main(x:i32)->i32{return x;}";
    let bytes = hand_authored_frame(
        text,
        &[
            ([1, 3, 7, 0, 0, 2, 4, 5], 1, 2),
            ([2, 8, 9, 0, 3, 0, 0, 0], 1, 2),
            ([3, 10, 13, 0, 7, 0, 0, 0], 2, 0),
            ([3, 16, 19, 0, 10, 0, 0, 0], 2, 0),
            ([5, 19, 30, 0, 16, 6, 0, 0], 0, 2),
            ([10, 20, 29, 0, 7, 0, 0, 0], 0, 0),
            ([19, 27, 28, 0, 14, 0, 0, 1], 2, 2),
        ],
    );
    let facts = original_parity(text, &bytes);
    let Some(Err(RunFailure::Entry(Some(span)))) = facts.verified.runtime else {
        panic!("wrong arity must remain an ordinary entry error: {facts:?}");
    };
    assert_eq!((span.start, span.end), (3, 7));
}

#[test]
fn checked_hir_import_run_hand_authored_arithmetic_failure_controls() {
    let overflow = "fn main()->i32{return 2147483647+1;}";
    let overflow_bytes = hand_authored_frame(
        overflow,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 2),
            ([3, 11, 14, 0, 7, 0, 0, 0], 2, 0),
            ([5, 14, 36, 0, 15, 4, 0, 0], 0, 2),
            ([10, 15, 35, 0, 6, 0, 0, 0], 0, 0),
            ([15, 22, 32, 0, 11, 0, 0, 1], i32::MAX, 2),
            ([24, 22, 34, 0, 5, 7, 12, 2], 0, 2),
            ([15, 33, 34, 0, 13, 0, 0, 1], 1, 2),
        ],
    );
    let overflow_facts = original_parity(overflow, &overflow_bytes);
    let Some(Err(RunFailure::Overflow(span))) = overflow_facts.verified.runtime else {
        panic!("overflow must retain its runtime variant: {overflow_facts:?}");
    };
    assert_eq!((span.start, span.end), (32, 33));

    let division = "fn main()->i32{return 1/0;}";
    let division_bytes = hand_authored_frame(
        division,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 2),
            ([3, 11, 14, 0, 7, 0, 0, 0], 2, 0),
            ([5, 14, 27, 0, 15, 4, 0, 0], 0, 2),
            ([10, 15, 26, 0, 6, 0, 0, 0], 0, 0),
            ([15, 22, 23, 0, 11, 0, 0, 1], 1, 2),
            ([27, 22, 25, 0, 5, 7, 12, 2], 0, 2),
            ([15, 24, 25, 0, 13, 0, 0, 1], 0, 2),
        ],
    );
    let division_facts = original_parity(division, &division_bytes);
    let Some(Err(RunFailure::DivisionByZero(span))) = division_facts.verified.runtime else {
        panic!("division must retain its runtime variant: {division_facts:?}");
    };
    assert_eq!((span.start, span.end), (23, 24));
}

#[test]
fn checked_hir_import_run_hand_authored_loop_uses_default_fuel() {
    let text = "fn main()->(){while true{}return;}";
    let bytes = hand_authored_frame(
        text,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 3),
            ([4, 11, 13, 0, 0, 0, 0, 0], 3, 0),
            ([5, 13, 34, 0, 17, 4, 0, 0], 0, 2),
            ([14, 14, 26, 7, 5, 6, 0, 0], 0, 0),
            ([17, 20, 24, 0, 0, 0, 0, 1], 0, 1),
            ([5, 24, 26, 0, 14, 0, 0, 0], 0, 1),
            ([10, 26, 33, 0, 0, 0, 0, 0], 0, 0),
        ],
    );
    let facts = original_parity(text, &bytes);
    let Some(Err(RunFailure::Fuel(span))) = facts.verified.runtime else {
        panic!("the infinite loop must exhaust ordinary default fuel: {facts:?}");
    };
    // Default fuel is 1,000,000: entry setup costs three, initial goto one,
    // then 333,332 three-step iterations exhaust it before the next literal.
    assert_eq!((span.start, span.end), (20, 24));
}

#[test]
fn checked_hir_import_run_hand_authored_recursion_uses_default_frames() {
    let text = "fn main()->i32{return main();}";
    let bytes = hand_authored_frame(
        text,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 2),
            ([3, 11, 14, 0, 7, 0, 0, 0], 2, 0),
            ([5, 14, 30, 0, 15, 4, 0, 0], 0, 2),
            ([10, 15, 29, 0, 5, 0, 0, 0], 0, 0),
            ([20, 22, 28, 0, 11, 0, 0, 1], 1, 2),
        ],
    );
    let facts = original_parity(text, &bytes);
    let Some(Err(RunFailure::Frames(span))) = facts.verified.runtime else {
        panic!("self recursion must reach the ordinary frame limit: {facts:?}");
    };
    assert_eq!((span.start, span.end), (22, 28));
}
