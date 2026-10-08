//! Final String failures through the genuine, paid private Emit leaf. Source,
//! AST and reserve-trace storage are baseline; every produced owner and error
//! drops inside its measured interval. No LLVM tool or default route is used.
use super::{candidate, leaf, SourceOwner};
use crate::frontend::{
    declaration_index::IndexLimits,
    lexer,
    oir::{native::private_emit, owned},
    parser,
    project::budget::{real_null_observer as null, Allocator},
    source::{SourceMap, SourceView},
};
use std::alloc::Layout;

const RICH_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-source.txt"
));
const RICH_WIRE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-success.bin"
));
const FINAL_ORDINAL: usize = 17;
const FINAL_BYTES: usize = 14_325;
const FINAL_KIND: &str = "private LLVM text";

fn assert_final_trace(allocator: &Allocator, start: usize, success: bool) {
    let rows = &allocator.trace[start..start + FINAL_ORDINAL];
    assert!(rows[..FINAL_ORDINAL - 1].iter().all(|row| row.success));
    assert_eq!(rows.iter().filter(|row| row.kind == FINAL_KIND).count(), 1);
    let final_row = &rows[FINAL_ORDINAL - 1];
    assert_eq!(final_row.kind, FINAL_KIND);
    assert_eq!(final_row.length, FINAL_BYTES);
    assert_eq!(final_row.element_bytes, 1);
    assert_eq!(final_row.success, success);
    assert!(!allocator.observer_trace_overflow);
}

#[test]
fn checked_hir_import_emit_success_drops_output_inside_complete_interval() {
    let (facts, allocation) = {
        let mut sources = SourceMap::new();
        let id = sources.add("main.ox".into(), RICH_SOURCE.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(FINAL_ORDINAL).unwrap();
        let trace_capacity = allocator.trace.capacity();
        let mut facts = (false, 0, 0, false);
        let allocation = owned::hir_import_measure_allocations(|| {
            let outcome = leaf::emit(
                owner,
                RICH_SOURCE.as_bytes(),
                RICH_WIRE,
                &mut allocator,
                IndexLimits::default(),
            );
            if let Ok(output) = &outcome {
                facts = (
                    output.artifact.verified.candidate.equal,
                    output.artifact.bytes,
                    output.artifact.capacity,
                    output.artifact.text.len() == FINAL_BYTES
                        && output.artifact.text.contains("define i32 @main"),
                );
            }
            drop(outcome);
        });
        assert!(owned::hir_import_allocation_observers_idle());
        assert_eq!(allocator.attempts, FINAL_ORDINAL);
        assert_eq!(allocator.trace.len(), FINAL_ORDINAL);
        assert_eq!(allocator.trace.capacity(), trace_capacity);
        assert_final_trace(&allocator, 0, true);
        (facts, allocation)
    };
    // The real SourceMap/path/text/AST and every compiler/output owner are gone.
    assert_eq!(facts, (true, FINAL_BYTES, FINAL_BYTES, true));
    assert!(allocation.0 > 0);
    assert_eq!(allocation.0, allocation.1);
    assert_eq!(allocation.2, 0);
    assert!(allocation.3 >= FINAL_BYTES as isize);
    println!("HIR_IMPORT_EMIT_SUCCESS_DROP allocation={allocation:?}");
}

#[test]
fn checked_hir_import_emit_final_string_logical_failure_drops_complete_pipeline() {
    let (failed, allocation) = {
        let mut sources = SourceMap::new();
        let id = sources.add("main.ox".into(), RICH_SOURCE.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        let mut allocator = Allocator {
            fail_at: Some(FINAL_ORDINAL),
            ..Allocator::default()
        };
        allocator.observer_trace_bound(FINAL_ORDINAL).unwrap();
        let trace_capacity = allocator.trace.capacity();
        let mut failed = false;
        let allocation = owned::hir_import_measure_allocations(|| {
            let outcome = leaf::emit(
                owner,
                RICH_SOURCE.as_bytes(),
                RICH_WIRE,
                &mut allocator,
                IndexLimits::default(),
            );
            failed = matches!(
                &outcome,
                Err(leaf::VerifyRejected::Terminal(
                    candidate::VerifyRejected::Native(private_emit::Failure::Allocation)
                ))
            );
            // Even an unexpected owned success/error is destroyed before the
            // observer closes; only the fixed discriminator leaves this scope.
            drop(outcome);
        });
        assert!(owned::hir_import_allocation_observers_idle());
        assert_eq!(allocator.attempts, FINAL_ORDINAL);
        assert_eq!(allocator.trace.len(), FINAL_ORDINAL);
        assert_eq!(allocator.trace.capacity(), trace_capacity);
        assert_final_trace(&allocator, 0, false);
        (failed, allocation)
    };
    assert!(failed);
    assert!(allocation.0 > 0);
    // fail_at forces capacity overflow and does not issue a null allocation.
    assert_eq!(allocation.0, allocation.1);
    assert_eq!(allocation.2, 0);
    assert!(allocation.3 > 0);
    println!("HIR_IMPORT_EMIT_LOGICAL_FAILURE_DROP allocation={allocation:?}");
}

#[test]
fn checked_hir_import_emit_final_string_real_null_cleans_up_and_recovers_once() {
    let (failed, failure_allocation, recovered, recovery_allocation, report) = {
        let mut sources = SourceMap::new();
        let id = sources.add("main.ox".into(), RICH_SOURCE.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(FINAL_ORDINAL * 2).unwrap();
        let trace_capacity = allocator.trace.capacity();
        let target = null::Target {
            attempt: FINAL_ORDINAL,
            kind: FINAL_KIND,
            slots: FINAL_BYTES,
            element_bytes: 1,
            layout: Layout::array::<u8>(FINAL_BYTES).unwrap(),
        };
        let ((failed, failure_allocation, recovered, recovery_allocation), report) =
            null::with_selected(&mut allocator, target, |allocator| {
                let mut failed = false;
                let failure_allocation = owned::hir_import_measure_allocations(|| {
                    let outcome = leaf::emit(
                        owner,
                        RICH_SOURCE.as_bytes(),
                        RICH_WIRE,
                        allocator,
                        IndexLimits::default(),
                    );
                    failed = matches!(
                        &outcome,
                        Err(leaf::VerifyRejected::Terminal(
                            candidate::VerifyRejected::Native(private_emit::Failure::Allocation)
                        ))
                    );
                    drop(outcome);
                });
                // Execute a fresh complete pipeline while this same selection
                // remains installed. Its one null decision must already be spent;
                // no allocator replacement, ordinal reset or fail_at mutation.
                let mut recovered = (false, 0, 0, false);
                let recovery_allocation = owned::hir_import_measure_allocations(|| {
                    let outcome = leaf::emit(
                        owner,
                        RICH_SOURCE.as_bytes(),
                        RICH_WIRE,
                        allocator,
                        IndexLimits::default(),
                    );
                    if let Ok(output) = &outcome {
                        recovered = (
                            output.artifact.verified.candidate.equal,
                            output.artifact.bytes,
                            output.artifact.capacity,
                            output.artifact.text.len() == FINAL_BYTES
                                && output.artifact.text.contains("define i32 @main"),
                        );
                    }
                    drop(outcome);
                });
                (failed, failure_allocation, recovered, recovery_allocation)
            })
            .unwrap();
        assert!(owned::hir_import_allocation_observers_idle());
        assert_eq!(allocator.fail_at, None);
        assert_eq!(allocator.attempts, FINAL_ORDINAL * 2);
        assert_eq!(allocator.trace.len(), FINAL_ORDINAL * 2);
        assert_eq!(allocator.trace.capacity(), trace_capacity);
        assert_final_trace(&allocator, 0, false);
        assert_final_trace(&allocator, FINAL_ORDINAL, true);
        assert_eq!(report.target, target);
        (
            failed,
            failure_allocation,
            recovered,
            recovery_allocation,
            report,
        )
    };
    // Primitive outcome facts and the fixed observer report survive real source
    // backing teardown. Neither the output String nor an error owner escapes.
    assert!(failed);
    assert!(report.selected && report.matched && report.fired);
    assert_eq!(report.rejection, None);
    assert_eq!(
        report.actual,
        Some(null::GlobalEvent {
            operation: null::Operation::Alloc,
            layout: Layout::array::<u8>(FINAL_BYTES).unwrap(),
            new_size: None,
        })
    );
    assert!(failure_allocation.1 > 0);
    assert_eq!(failure_allocation.0, failure_allocation.1 + 1);
    assert_eq!(failure_allocation.2, 0);
    assert!(failure_allocation.3 > 0);
    assert_eq!(recovered, (true, FINAL_BYTES, FINAL_BYTES, true));
    assert!(recovery_allocation.0 > 0);
    assert_eq!(recovery_allocation.0, recovery_allocation.1);
    assert_eq!(recovery_allocation.2, 0);
    assert!(recovery_allocation.3 >= FINAL_BYTES as isize);
    println!("HIR_IMPORT_EMIT_REAL_NULL_DROP failure={failure_allocation:?} recovery={recovery_allocation:?}");
}
