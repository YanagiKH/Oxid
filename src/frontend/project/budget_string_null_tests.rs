//! Standalone String storage controls. No source/compiler owner, private Emit
//! entry point or native tool participates; selected callbacks return facts only.
use super::{real_null_observer as null, Allocator, ReserveFailure};
use std::alloc::Layout;
use std::fmt::Write;

const TEXT: &str = "scalar sink";
const KIND: &str = "standalone String sink";

fn target(attempt: usize) -> null::Target {
    null::Target {
        attempt,
        kind: KIND,
        slots: TEXT.len(),
        element_bytes: 1,
        layout: Layout::array::<u8>(TEXT.len()).unwrap(),
    }
}

fn assert_fired(report: null::Report, target: null::Target) {
    assert_eq!(report.target, target);
    assert!(report.selected && report.matched && report.fired);
    assert_eq!(report.rejection, None);
    assert_eq!(
        report.actual,
        Some(null::GlobalEvent {
            operation: null::Operation::Alloc,
            layout: target.layout,
            new_size: None,
        })
    );
}

#[test]
fn string_real_null_fresh_sink_reserves_and_writes() {
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(1).unwrap();
    let mut sink = String::new();
    assert_eq!((sink.len(), sink.capacity()), (0, 0));
    allocator.string(&mut sink, TEXT.len(), KIND).unwrap();
    assert_eq!(sink.capacity(), TEXT.len());
    let storage = sink.as_ptr();
    sink.write_str(TEXT).unwrap();
    assert_eq!(sink, TEXT);
    assert_eq!(sink.capacity(), TEXT.len());
    assert_eq!(sink.as_ptr(), storage);
    drop(sink);
    assert_eq!(allocator.attempts, 1);
    assert_eq!(allocator.trace.len(), 1);
    let event = &allocator.trace[0];
    assert_eq!(
        (event.kind, event.length, event.element_bytes),
        (KIND, TEXT.len(), 1)
    );
    assert!(event.success);
    assert!(!allocator.observer_trace_overflow);
}

#[test]
fn string_real_null_exact_failure_is_one_shot_and_selection_cleans_up() {
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(4).unwrap();
    let trace_capacity = allocator.trace.capacity();
    let trace_pointer = allocator.trace.as_ptr();
    let ((failed, untouched, succeeded, written), report) =
        null::with_selected(&mut allocator, target(1), |allocator| {
            let mut denied = String::new();
            let failed =
                allocator.string(&mut denied, TEXT.len(), KIND) == Err(ReserveFailure::Allocation);
            let untouched = denied.is_empty() && denied.capacity() == 0;
            let mut sink = String::new();
            let succeeded = allocator.string(&mut sink, TEXT.len(), KIND).is_ok();
            let written = succeeded
                && sink.capacity() == TEXT.len()
                && sink.write_str(TEXT).is_ok()
                && sink == TEXT
                && sink.capacity() == TEXT.len();
            drop(sink);
            drop(denied);
            (failed, untouched, succeeded, written)
        })
        .unwrap();
    assert!(failed && untouched && succeeded && written);
    assert_fired(report, target(1));

    // A new selection succeeds at the current allocator's next absolute ordinal.
    // Neither the allocator nor its history is reset between selections.
    let ((failed, untouched), report) =
        null::with_selected(&mut allocator, target(3), |allocator| {
            let mut denied = String::new();
            let failed =
                allocator.string(&mut denied, TEXT.len(), KIND) == Err(ReserveFailure::Allocation);
            let untouched = denied.is_empty() && denied.capacity() == 0;
            drop(denied);
            (failed, untouched)
        })
        .unwrap();
    assert!(failed && untouched);
    assert_fired(report, target(3));
    let mut ordinary = String::new();
    allocator.string(&mut ordinary, TEXT.len(), KIND).unwrap();
    ordinary.write_str(TEXT).unwrap();
    assert_eq!(ordinary, TEXT);
    drop(ordinary);
    assert_eq!(allocator.attempts, 4);
    assert_eq!(allocator.trace.len(), 4);
    for (event, success) in allocator.trace.iter().zip([false, true, false, true]) {
        assert_eq!(
            (event.kind, event.length, event.element_bytes),
            (KIND, TEXT.len(), 1)
        );
        assert_eq!(event.success, success);
    }
    assert_eq!(allocator.trace.capacity(), trace_capacity);
    assert_eq!(allocator.trace.as_ptr(), trace_pointer);
    assert!(!allocator.observer_trace_overflow);
    // Every fired report above contains an actual GlobalAlloc event, not the
    // separate logical fail_at/capacity-overflow test seam.
    assert_eq!(allocator.fail_at, None);
}

#[test]
fn string_real_null_binds_identity_and_ordinal_and_forwards_unrelated_storage() {
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(3).unwrap();
    allocator
        .string(&mut String::new(), 0, "selected prefix")
        .unwrap();
    let mut other = Allocator::default();
    other.observer_trace_bound(3).unwrap();
    for _ in 0..2 {
        other.string(&mut String::new(), 0, "other prefix").unwrap();
    }
    let ((unrelated, foreign, earlier, failed, untouched), report) =
        null::with_selected(&mut allocator, target(3), |allocator| {
            // Selection alone cannot affect an unrelated, same-layout request.
            let mut ordinary = String::new();
            let unrelated = ordinary.try_reserve_exact(TEXT.len()).is_ok()
                && ordinary.write_str(TEXT).is_ok()
                && ordinary == TEXT;
            let mut foreign = String::new();
            // This allocator reaches the exact kind, ordinal and layout first.
            let foreign_ok = other.string(&mut foreign, TEXT.len(), KIND).is_ok();
            let mut earlier = String::new();
            let earlier_ok = allocator.string(&mut earlier, TEXT.len(), KIND).is_ok();
            let mut denied = String::new();
            let failed =
                allocator.string(&mut denied, TEXT.len(), KIND) == Err(ReserveFailure::Allocation);
            let untouched = denied.is_empty() && denied.capacity() == 0;
            drop(denied);
            drop(earlier);
            drop(foreign);
            drop(ordinary);
            (unrelated, foreign_ok, earlier_ok, failed, untouched)
        })
        .unwrap();
    assert!(unrelated && foreign && earlier && failed && untouched);
    assert_fired(report, target(3));
    assert_eq!(allocator.attempts, 3);
    assert_eq!(other.attempts, 3);
    assert_eq!(allocator.trace.len(), 3);
    assert_eq!(other.trace.len(), 3);
    assert_eq!(allocator.trace[0].kind, "selected prefix");
    assert!(allocator.trace[0].success && allocator.trace[1].success);
    assert!(!allocator.trace[2].success);
    assert!(other.trace.iter().all(|event| event.success));
    assert!(!allocator.observer_trace_overflow && !other.observer_trace_overflow);
}

#[test]
fn string_real_null_rejects_wrong_request_and_existing_storage() {
    #[derive(Clone, Copy)]
    enum Case {
        Kind,
        Layout,
        Ordinal,
        EmptyWithCapacity,
        Nonempty,
    }
    for case in [
        Case::Kind,
        Case::Layout,
        Case::Ordinal,
        Case::EmptyWithCapacity,
        Case::Nonempty,
    ] {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(1).unwrap();
        let mut selected = target(if matches!(case, Case::Ordinal) { 2 } else { 1 });
        if matches!(case, Case::Layout) {
            selected.layout = Layout::from_size_align(TEXT.len(), 2).unwrap();
        }
        let ((succeeded, length_preserved, capacity_preserved, contents_preserved), report) =
            null::with_selected(&mut allocator, selected, |allocator| {
                let mut sink = match case {
                    Case::EmptyWithCapacity => String::with_capacity(TEXT.len() * 2),
                    Case::Nonempty => String::from("seed"),
                    _ => String::new(),
                };
                let length = sink.len();
                let capacity = sink.capacity();
                let kind = if matches!(case, Case::Kind) {
                    "wrong String sink"
                } else {
                    KIND
                };
                let succeeded = allocator.string(&mut sink, TEXT.len(), kind).is_ok();
                let facts = (
                    succeeded,
                    sink.len() == length,
                    sink.capacity() >= capacity,
                    if matches!(case, Case::Nonempty) {
                        sink == "seed"
                    } else {
                        sink.is_empty()
                    },
                );
                drop(sink);
                facts
            })
            .unwrap();
        assert!(succeeded && length_preserved && capacity_preserved && contents_preserved);
        assert!(report.selected && !report.matched && !report.fired);
        assert_eq!(report.actual, None);
        assert_eq!(
            report.rejection,
            Some(match case {
                Case::Kind | Case::Layout => null::Reason::WrongRequest,
                Case::Ordinal => null::Reason::MissingTarget,
                Case::EmptyWithCapacity | Case::Nonempty => null::Reason::IneligibleVector,
            })
        );
        assert_eq!(allocator.attempts, 1);
        assert_eq!(allocator.trace.len(), 1);
        assert!(allocator.trace[0].success);
        assert!(!allocator.observer_trace_overflow);
    }
}
