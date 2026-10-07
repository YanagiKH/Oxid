use super::super::{leaf, CELLS, COLUMN_STARTS};
use super::*;
use crate::frontend::{
    declaration_index::SourceOwner,
    lexer, parser,
    source::{SourceMap, SourceView},
};

macro_rules! fixture {
    ($name:literal) => {
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/",
                $name,
                "-source.txt"
            )),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/",
                $name,
                "-success.bin"
            )) as &[u8],
        )
    };
}
const RICH: (&str, &[u8]) = fixture!("rich");
const SCALARS: &[(&str, &[u8])] = &[
    fixture!("scalar-arithmetic"),
    fixture!("scalar-boolean"),
    fixture!("scalar-comparison"),
    fixture!("scalar-unit"),
    fixture!("scalar-loop"),
    fixture!("scalar-assignment"),
];

// Only fixed rejection facts leave the fully paid, still-denied source leaf.
fn compare_fixture(
    text: &str,
    bytes: &[u8],
    allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<leaf::Facts, leaf::Rejected> {
    let mut sources = SourceMap::new();
    let id = sources.add("contained-candidate.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    match leaf::denied(owner, text.as_bytes(), bytes, allocator, limits) {
        Err(leaf::Rejected::Compared(facts)) => Ok(facts),
        Err(rejection) => Err(rejection),
        Ok(never) => match never {},
    }
}
fn allocator() -> Allocator {
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(32).unwrap();
    allocator
}
fn set_resolution(bytes: &mut [u8], reference: u8, value: i32) {
    for (plane, byte) in value.to_le_bytes().into_iter().enumerate() {
        bytes[COLUMN_STARTS[3] + plane * CELLS + usize::from(reference - 1)] = byte;
    }
}
fn row_reference(bytes: &[u8], kind: u8) -> u8 {
    (1..=bytes[8])
        .find(|&reference| bytes[COLUMN_STARTS[0] + usize::from(reference - 1)] & 63 == kind)
        .unwrap()
}

#[test]
fn checked_hir_import_candidate_all_scalar_fixtures_match() {
    for &(source, wire) in SCALARS.iter().chain(std::iter::once(&RICH)) {
        let mut allocator = allocator();
        let trace_capacity = allocator.trace.capacity();
        let facts = compare_fixture(source, wire, &mut allocator, IndexLimits::default()).unwrap();
        assert!(facts.candidate.equal);
        assert_eq!(facts.candidate.allocation.reserves, allocator.attempts);
        assert_eq!(
            facts.candidate.charged_work,
            facts.candidate.builder_work + facts.candidate.allocation.helper_work
        );
        assert_eq!(
            facts.candidate.builder_named_bytes,
            builder_named_bytes().unwrap()
        );
        assert_eq!(allocator.trace.capacity(), trace_capacity);
        assert!(!allocator.observer_trace_overflow);
    }
}

#[test]
fn checked_hir_import_candidate_rich_shape_and_all_failure_ordinals() {
    let mut observed = allocator();
    let facts = compare_fixture(RICH.0, RICH.1, &mut observed, IndexLimits::default()).unwrap();
    assert!(facts.candidate.equal);
    assert_eq!(
        facts.candidate.allocation.requested.0,
        [2, 2, 1, 2, 11, 5, 7, 2]
    );
    assert_eq!(facts.candidate.allocation.vectors, 17);
    assert_eq!(facts.candidate.allocation.reserves, 16);
    for ordinal in 1..=16 {
        let mut failing = allocator();
        failing.fail_at = Some(ordinal);
        assert!(matches!(
            compare_fixture(RICH.0, RICH.1, &mut failing, IndexLimits::default()),
            Err(leaf::Rejected::Candidate(Failure::Allocation))
        ));
        assert_eq!(failing.attempts, ordinal);
        assert_eq!(failing.trace.len(), ordinal);
        assert!(failing.trace[..ordinal - 1]
            .iter()
            .all(|event| event.success));
        assert!(!failing.trace[ordinal - 1].success);
        assert!(!failing.observer_trace_overflow);
    }
    // These logical prefix controls do not claim independent live-byte/null
    // evidence. The parent integration must measure actual contained cleanup.
}

#[test]
fn checked_hir_import_candidate_resolution_mismatch_is_not_repaired() {
    let mut bytes = RICH.1.to_vec();
    let number = row_reference(&bytes, 15);
    set_resolution(&mut bytes, number, i32::MIN);
    let mut allocator = allocator();
    let Err(leaf::Rejected::Mismatch(facts)) =
        compare_fixture(RICH.0, &bytes, &mut allocator, IndexLimits::default())
    else {
        panic!("changed supplied literal must be rejected, never repaired");
    };
    assert!(!facts.candidate.equal);
    assert_eq!(facts.candidate.allocation.reserves, 16);
    assert_eq!(allocator.attempts, 16);
}

#[test]
fn checked_hir_import_candidate_resolution_role_errors_precede_reserve() {
    for (kind, invalid) in [(1, 0), (2, 0), (3, 4), (5, 1), (8, -1), (19, 0), (20, 0)] {
        let mut bytes = RICH.1.to_vec();
        let reference = row_reference(&bytes, kind);
        set_resolution(&mut bytes, reference, invalid);
        let mut allocator = allocator();
        assert!(matches!(
            compare_fixture(RICH.0, &bytes, &mut allocator, IndexLimits::default()),
            Err(leaf::Rejected::Candidate(Failure::Shape))
        ));
        assert_eq!(allocator.attempts, 0);
    }
    let mut bytes = RICH.1.to_vec();
    for reference in 1..=bytes[8] {
        set_resolution(&mut bytes, reference, 0);
    }
    let mut allocator = allocator();
    assert!(matches!(
        compare_fixture(RICH.0, &bytes, &mut allocator, IndexLimits::default()),
        Err(leaf::Rejected::Candidate(Failure::Shape))
    ));
    assert_eq!(allocator.attempts, 0);
}

#[test]
fn checked_hir_import_candidate_shared_work_exact_limit() {
    let facts = compare_fixture(RICH.0, RICH.1, &mut allocator(), IndexLimits::default()).unwrap();
    let exact = IndexLimits {
        work: facts.total_work,
        ..IndexLimits::default()
    };
    assert!(
        compare_fixture(RICH.0, RICH.1, &mut allocator(), exact)
            .unwrap()
            .candidate
            .equal
    );
    let mut denied = allocator();
    assert!(matches!(
        compare_fixture(
            RICH.0,
            RICH.1,
            &mut denied,
            IndexLimits {
                work: exact.work - 1,
                ..exact
            }
        ),
        Err(leaf::Rejected::Candidate(Failure::Admission))
    ));
    assert_eq!(denied.attempts, 0);
    assert!(work_bound(MAX_ROWS).is_ok());
    assert!(matches!(work_bound(MAX_ROWS + 1), Err(Failure::Shape)));
    assert!(matches!(copies::<u64>(usize::MAX), Err(Failure::Overflow)));
}

#[test]
fn checked_hir_import_candidate_function_window_uses_local_expression_ids() {
    let window = FunctionWindow {
        function: 1,
        row_begin: 12,
        row_end: 129,
        expression_begin: 7,
        expression_end: 11,
        locals: 2,
    };
    assert_eq!(window.expression(ast::ExprId(7)).unwrap(), hir::ExprId(0));
    assert_eq!(window.child(ast::ExprId(9), 10).unwrap(), hir::ExprId(2));
    assert!(window.expression(ast::ExprId(6)).is_err());
    assert!(window.expression(ast::ExprId(11)).is_err());
    assert!(window.child(ast::ExprId(10), 10).is_err());
    assert!(window.contains(128));
    assert!(!window.contains(11));
    let empty = FunctionWindow {
        expression_end: 7,
        ..window
    };
    assert!(empty.expression(ast::ExprId(7)).is_err());
}

#[test]
fn checked_hir_import_candidate_layout_only() {
    println!("HIR_IMPORT_CANDIDATE named_bytes={} work_at_128={} facts={} result={} window={} cursors={} mapping={}",
        builder_named_bytes().unwrap(), work_bound(MAX_ROWS).unwrap(), size_of::<ComparisonFacts>(), size_of::<Result<ComparisonFacts, Failure>>(), size_of::<FunctionWindow>(), size_of::<BuilderCursors>(), size_of::<Mapping<'_, '_, '_, '_>>());
    assert!(builder_named_bytes().unwrap() >= size_of::<hir::Program>());
    assert!(work_bound(MAX_ROWS).unwrap() < IndexLimits::default().work);
}
