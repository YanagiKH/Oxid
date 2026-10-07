use super::*;
use crate::frontend::declaration_index::IndexLimits;
use crate::frontend::{
    lexer, parser,
    project::budget::Allocator,
    source::{SourceMap, SourceView},
};

#[allow(clippy::result_large_err)]
fn verify_original(
    text: &str,
    wire: &[u8],
    allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<leaf::VerifyFacts, leaf::VerifyRejected> {
    let mut sources = SourceMap::new();
    let id = sources.add("verify-import.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    leaf::verify(owner, text.as_bytes(), wire, allocator, limits)
}

#[test]
fn checked_hir_import_verify_genuine_rich_fixed_facts() {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    let wire = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-success.bin"
    ));
    let mut allocator = Allocator::default();
    let facts = verify_original(text, wire, &mut allocator, IndexLimits::default()).unwrap();
    assert_eq!(facts.verified.functions, 2);
    assert_eq!(facts.verified.typed_cells, usize::from(wire[8]));
    assert_eq!(allocator.attempts, 16);
    assert!(facts.verified.candidate.equal);
    assert_eq!(
        facts.total_work,
        facts.source_work
            + facts.canonical_work
            + facts.verified.candidate.charged_work
            + facts.verified.pass_work
            + facts.verified.typed_work
    );
    println!("HIR_IMPORT_VERIFY_SUCCESS {facts:?}");
}

#[test]
fn checked_hir_import_verify_exact_shared_work_and_byte_boundaries() {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    let wire = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-success.bin"
    ));
    let baseline = verify_original(
        text,
        wire,
        &mut Allocator::default(),
        IndexLimits::default(),
    )
    .unwrap();
    let receipt = baseline.verified.candidate.allocation;
    let exact = IndexLimits {
        retained: receipt.affected_bytes as u64,
        scratch: receipt.fixed_bytes as u64,
        work: baseline.total_work,
    };
    let accepted = verify_original(text, wire, &mut Allocator::default(), exact).unwrap();
    assert_eq!(accepted.total_work, baseline.total_work);
    for limits in [
        IndexLimits {
            retained: exact.retained - 1,
            ..exact
        },
        IndexLimits {
            scratch: exact.scratch - 1,
            ..exact
        },
        IndexLimits {
            work: exact.work - 1,
            ..exact
        },
    ] {
        let mut allocator = Allocator::default();
        let guard = crate::frontend::typeck::measurement::begin();
        let result = verify_original(text, wire, &mut allocator, limits);
        let observed = guard.finish();
        assert!(matches!(
            result,
            Err(leaf::VerifyRejected::Source(leaf::Rejected::Budget))
                | Err(leaf::VerifyRejected::Terminal(
                    candidate::VerifyRejected::Candidate(allocation::Failure::Admission)
                ))
        ));
        assert_eq!(allocator.attempts, 0);
        assert_eq!(observed.frame_bytes, 0);
    }
}

#[test]
fn checked_hir_import_verify_owned_pipeline_drops_success_and_failure_storage() {
    use crate::frontend::oir::owned;
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    let wire = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-success.bin"
    ));
    for ordinal in 0..=16 {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(32).unwrap();
        if ordinal != 0 {
            allocator.fail_at = Some(ordinal);
        }
        let mut accepted = false;
        let (attempts, calls, live, peak) = owned::hir_import_measure_allocations(|| {
            let result = verify_original(text, wire, &mut allocator, IndexLimits::default());
            accepted = if ordinal == 0 {
                result.is_ok()
            } else {
                matches!(
                    result,
                    Err(leaf::VerifyRejected::Terminal(
                        candidate::VerifyRejected::Candidate(allocation::Failure::Allocation)
                    ))
                )
            };
            // Any diagnostic/error owners are also dropped inside this interval.
        });
        assert!(accepted, "ordinal {ordinal}");
        assert_eq!(allocator.attempts, if ordinal == 0 { 16 } else { ordinal });
        assert_eq!(live, 0, "ordinal {ordinal}");
        assert!(attempts >= calls && peak > 0);
        assert!(owned::hir_import_allocation_observers_idle());
    }
}

#[test]
fn checked_hir_import_verify_real_null_reserves_drop_all_pipeline_storage() {
    use crate::frontend::{
        oir::owned,
        project::budget::real_null_observer::{self, Operation, Target},
    };
    use std::alloc::Layout;
    // Independent rich-fixture allocation oracle, inherited from the reviewed
    // candidate-only test. The observed interval here covers the whole pipeline.
    let requests = [
        ("signatures", 2, 56, 8),
        ("functions", 2, 112, 8),
        ("parameters", 1, 1, 1),
        ("locals", 1, 32, 8),
        ("expressions", 1, 72, 8),
        ("blocks", 1, 72, 8),
        ("statements", 1, 96, 8),
        ("locals", 1, 32, 8),
        ("expressions", 10, 72, 8),
        ("blocks", 4, 72, 8),
        ("statements", 3, 96, 8),
        ("statements", 1, 96, 8),
        ("statements", 1, 96, 8),
        ("statements", 1, 96, 8),
        ("arguments", 1, 8, 8),
        ("arguments", 1, 8, 8),
    ];
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    let wire = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-success.bin"
    ));
    for (index, (kind, slots, width, align)) in requests.into_iter().enumerate() {
        let kind = match kind {
            "signatures" => "checked HIR candidate signatures",
            "functions" => "checked HIR candidate functions",
            "parameters" => "checked HIR candidate parameters",
            "locals" => "checked HIR candidate locals",
            "expressions" => "checked HIR candidate expressions",
            "blocks" => "checked HIR candidate blocks",
            "statements" => "checked HIR candidate statements",
            "arguments" => "checked HIR candidate arguments",
            _ => unreachable!(),
        };
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(32).unwrap();
        let target = Target {
            attempt: index + 1,
            kind,
            slots,
            element_bytes: width,
            layout: Layout::from_size_align(slots * width, align).unwrap(),
        };
        let mut failed = false;
        let (observed, report) =
            real_null_observer::with_selected(&mut allocator, target, |allocator| {
                owned::hir_import_measure_allocations(|| {
                    failed = matches!(
                        verify_original(text, wire, allocator, IndexLimits::default()),
                        Err(leaf::VerifyRejected::Terminal(
                            candidate::VerifyRejected::Candidate(allocation::Failure::Allocation)
                        ))
                    );
                })
            })
            .unwrap();
        assert!(failed && report.selected && report.matched && report.fired);
        assert_eq!(report.rejection, None);
        let actual = report.actual.unwrap();
        assert_eq!(actual.operation, Operation::Alloc);
        assert_eq!(actual.layout, target.layout);
        assert_eq!(actual.new_size, None);
        assert_eq!(allocator.attempts, index + 1);
        assert_eq!(observed.0, observed.1 + 1);
        assert_eq!(observed.2, 0);
        assert!(owned::hir_import_allocation_observers_idle());
    }
}
