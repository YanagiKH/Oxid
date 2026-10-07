use super::*;
use crate::frontend::{
    oir::owned,
    project::budget::real_null_observer::{self, Operation, Target},
};
use std::alloc::Layout;

// Independent rich-fixture request oracle: kind, slots, element bytes, alignment.
// Zero-length main parameters have no allocator call and are omitted here.
const REQUESTS: [(&str, usize, usize, usize); 16] = [
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
fn kind(name: &str) -> &'static str {
    match name {
        "signatures" => "checked HIR candidate signatures",
        "functions" => "checked HIR candidate functions",
        "parameters" => "checked HIR candidate parameters",
        "locals" => "checked HIR candidate locals",
        "expressions" => "checked HIR candidate expressions",
        "blocks" => "checked HIR candidate blocks",
        "statements" => "checked HIR candidate statements",
        "arguments" => "checked HIR candidate arguments",
        _ => panic!("fixed request oracle"),
    }
}
fn observation() -> (usize, usize, isize, isize) {
    assert!(owned::hir_import_allocation_observers_idle());
    leaf::take_candidate_observation().expect("candidate interval must be observed")
}
fn check_trace(allocator: &Allocator, count: usize) {
    assert_eq!(allocator.attempts, count);
    assert_eq!(allocator.trace.len(), count);
    for (event, &(name, slots, width, _)) in allocator.trace.iter().zip(&REQUESTS) {
        assert_eq!(
            (event.kind, event.length, event.element_bytes),
            (kind(name), slots, width)
        );
    }
    assert!(!allocator.observer_trace_overflow);
}

#[test]
fn checked_hir_import_candidate_actual_payload_and_mismatch_cleanup() {
    let mut selected = allocator();
    let trace_capacity = selected.trace.capacity();
    let facts = compare_fixture(RICH.0, RICH.1, &mut selected, IndexLimits::default()).unwrap();
    assert!(facts.candidate.equal);
    assert_eq!(facts.candidate.allocation.candidate_request_bytes, 2241);
    assert_eq!(observation(), (16, 16, 0, 2241));
    check_trace(&selected, 16);
    assert_eq!(selected.trace.capacity(), trace_capacity);
    let mut changed = RICH.1.to_vec();
    let number = row_reference(&changed, 15);
    set_resolution(&mut changed, number, i32::MIN);
    assert!(matches!(
        compare_fixture(RICH.0, &changed, &mut allocator(), IndexLimits::default()),
        Err(leaf::Rejected::Mismatch(_))
    ));
    assert_eq!(observation(), (16, 16, 0, 2241));
}

#[test]
fn checked_hir_import_candidate_actual_logical_failure_prefix_cleanup() {
    let mut prefix = 0isize;
    for (index, &(_, slots, width, _)) in REQUESTS.iter().enumerate() {
        let mut selected = allocator();
        selected.fail_at = Some(index + 1);
        assert!(matches!(
            compare_fixture(RICH.0, RICH.1, &mut selected, IndexLimits::default()),
            Err(leaf::Rejected::Candidate(Failure::Allocation))
        ));
        // Logical failure increments Allocator.attempts but never calls GlobalAlloc.
        assert_eq!(observation(), (index, index, 0, prefix));
        check_trace(&selected, index + 1);
        assert!(!selected.trace[index].success);
        prefix += (slots * width) as isize;
    }
    assert_eq!(prefix, 2241);
}

#[test]
fn checked_hir_import_candidate_actual_null_failure_prefix_cleanup() {
    let mut prefix = 0isize;
    for (index, &(name, slots, width, align)) in REQUESTS.iter().enumerate() {
        let mut selected = allocator();
        let target = Target {
            attempt: index + 1,
            kind: kind(name),
            slots,
            element_bytes: width,
            layout: Layout::from_size_align(slots * width, align).unwrap(),
        };
        let (result, report) =
            real_null_observer::with_selected(&mut selected, target, |selected| {
                compare_fixture(RICH.0, RICH.1, selected, IndexLimits::default())
            })
            .unwrap();
        assert!(matches!(
            result,
            Err(leaf::Rejected::Candidate(Failure::Allocation))
        ));
        assert!(report.selected && report.matched && report.fired);
        assert_eq!(report.rejection, None);
        let actual = report.actual.unwrap();
        assert_eq!(actual.operation, Operation::Alloc);
        assert_eq!(actual.layout, target.layout);
        assert_eq!(actual.new_size, None);
        assert_eq!(observation(), (index + 1, index, 0, prefix));
        check_trace(&selected, index + 1);
        assert!(!selected.trace[index].success);
        prefix += (slots * width) as isize;
    }
    assert_eq!(prefix, 2241);
}

#[test]
fn checked_hir_import_candidate_observation_clears_before_early_rejection() {
    compare_fixture(RICH.0, RICH.1, &mut allocator(), IndexLimits::default()).unwrap();
    // Leave the prior observation unread; the next entry must clear it.
    let mut selected = allocator();
    assert!(matches!(
        compare_fixture(
            RICH.0,
            RICH.1,
            &mut selected,
            IndexLimits {
                work: 0,
                ..IndexLimits::default()
            }
        ),
        Err(leaf::Rejected::Budget)
    ));
    assert!(leaf::take_candidate_observation().is_none());
    assert!(owned::hir_import_allocation_observers_idle());
    assert_eq!(selected.attempts, 0);
}

#[test]
fn checked_hir_import_candidate_observer_nesting_unwind_and_thread_isolation() {
    assert!(owned::hir_import_allocation_observers_idle());
    let result = std::panic::catch_unwind(|| {
        owned::hir_import_measure_allocations(|| {
            owned::hir_import_measure_allocations(|| {});
        });
    });
    assert!(result.is_err());
    // Panic-hook allocations may be observed before unwinding resets the
    // existing guards. Only reset/recovery, not panic-path counts, is asserted.
    assert!(owned::hir_import_allocation_observers_idle());
    assert_eq!(owned::hir_import_measure_allocations(|| {}), (0, 0, 0, 0));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            std::thread::spawn(|| {
                assert!(leaf::take_candidate_observation().is_none());
                compare_fixture(RICH.0, RICH.1, &mut allocator(), IndexLimits::default()).unwrap();
                observation()
            })
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), (16, 16, 0, 2241));
    }
    assert!(owned::hir_import_allocation_observers_idle());
    println!(
        "HIR_IMPORT_OBSERVER tls={} fixed_result_slot={}",
        size_of::<std::cell::Cell<Option<(usize, usize, isize, isize)>>>(),
        size_of::<Option<Result<ComparisonFacts, Failure>>>()
    );
}
