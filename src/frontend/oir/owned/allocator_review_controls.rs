//! Independent controls for both original observers through the shared allocator.
use super::super::{reviewer_origins as raw, source::reviewer_source as source};
use std::alloc::{alloc, alloc_zeroed, dealloc, realloc, Layout};

fn real_operations() {
    let small = Layout::from_size_align(64, 8).unwrap();
    let zero_layout = Layout::from_size_align(128, 8).unwrap();
    let grown_layout = Layout::from_size_align(256, 8).unwrap();
    unsafe {
        let pointer = std::hint::black_box(alloc(small));
        assert!(!pointer.is_null());
        pointer.write_bytes(0x5a, small.size());
        let zero = std::hint::black_box(alloc_zeroed(zero_layout));
        assert!(!zero.is_null());
        assert!(std::slice::from_raw_parts(zero, zero_layout.size())
            .iter()
            .all(|byte| *byte == 0));
        let grown = std::hint::black_box(realloc(pointer, small, grown_layout.size()));
        assert!(!grown.is_null());
        assert!(std::slice::from_raw_parts(grown, small.size())
            .iter()
            .all(|byte| *byte == 0x5a));
        std::hint::black_box((grown, zero));
        dealloc(grown, grown_layout);
        dealloc(zero, zero_layout);
    }
}

#[test]
fn allocator_review_controls_both_observers_see_real_alloc_zeroed_and_realloc() {
    let (((), stats), attempts) =
        raw::integration_counted(|| source::integration_measured(real_operations));
    assert_eq!(attempts, 3);
    assert_eq!(stats, (3, 0, 384));
    assert!(!raw::integration_enabled());
    assert!(!source::integration_enabled());
}

#[test]
fn allocator_review_controls_each_observer_works_while_the_other_is_disabled() {
    assert!(!source::integration_enabled());
    let ((), attempts) = raw::integration_counted(real_operations);
    assert_eq!(attempts, 3);
    assert!(!raw::integration_enabled());
    let ((), stats) = source::integration_measured(real_operations);
    assert_eq!(stats, (3, 0, 384));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn allocator_review_controls_null_allocations_remain_attempts_but_not_successes() {
    // Above this target's address space, so libc cannot reserve these requests.
    // Direct allocator calls return null; no infallible Box/Vec abort is involved.
    let enormous = Layout::from_size_align(isize::MAX as usize - 4095, 8).unwrap();
    for zeroed in [false, true] {
        let (((), stats), attempts) = raw::integration_counted(|| {
            source::integration_measured(|| unsafe {
                let pointer = if zeroed {
                    alloc_zeroed(enormous)
                } else {
                    alloc(enormous)
                };
                assert!(std::hint::black_box(pointer).is_null());
            })
        });
        assert_eq!(attempts, 1);
        assert_eq!(stats, (0, 0, 0));
    }
    let small = Layout::from_size_align(64, 8).unwrap();
    let (((), stats), attempts) = raw::integration_counted(|| {
        source::integration_measured(|| unsafe {
            let pointer = std::hint::black_box(alloc(small));
            assert!(!pointer.is_null());
            pointer.write(0x5a);
            let denied = std::hint::black_box(realloc(pointer, small, enormous.size()));
            assert!(denied.is_null());
            assert_eq!(pointer.read(), 0x5a);
            dealloc(pointer, small);
        })
    });
    assert_eq!(attempts, 2);
    assert_eq!(stats, (1, 0, 64));
}

#[test]
fn allocator_review_controls_panic_restores_both_original_tracking_guards() {
    let caught = std::panic::catch_unwind(|| {
        raw::integration_counted(|| {
            source::integration_measured(|| panic!("intentional tracker-reset control"))
        });
    });
    assert!(caught.is_err());
    drop(caught);
    assert!(!raw::integration_enabled());
    assert!(!source::integration_enabled());
    let (((), stats), attempts) =
        raw::integration_counted(|| source::integration_measured(real_operations));
    assert_eq!(attempts, 3);
    assert_eq!(stats, (3, 0, 384));
}

#[test]
fn allocator_review_controls_other_thread_is_invisible_to_both_parent_trackers() {
    use std::sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    };
    let phase = Arc::new(AtomicU8::new(0));
    let child_phase = Arc::clone(&phase);
    let worker = std::thread::spawn(move || {
        while child_phase.load(Ordering::Acquire) != 1 {
            std::hint::spin_loop();
        }
        let observed = raw::integration_counted(|| source::integration_measured(real_operations));
        child_phase.store(2, Ordering::Release);
        observed
    });
    let (((), parent_stats), parent_attempts) = raw::integration_counted(|| {
        source::integration_measured(|| {
            phase.store(1, Ordering::Release);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while phase.load(Ordering::Acquire) != 2 {
                assert!(
                    std::time::Instant::now() < deadline,
                    "worker did not complete"
                );
                std::hint::spin_loop();
            }
        })
    });
    assert_eq!(parent_attempts, 0);
    assert_eq!(parent_stats, (0, 0, 0));
    let (((), child_stats), child_attempts) = worker.join().unwrap();
    assert_eq!(child_attempts, 3);
    assert_eq!(child_stats, (3, 0, 384));
}

#[test]
fn allocator_review_controls_real_null_exact_reserve_is_one_shot_and_accounted() {
    use crate::frontend::project::budget::{real_null_observer as null, Allocator, ReserveFailure};
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(2).unwrap();
    let target = null::Target {
        attempt: 1,
        kind: "real null calibration",
        slots: 8,
        element_bytes: std::mem::size_of::<u64>(),
        layout: Layout::array::<u64>(8).unwrap(),
    };
    let action = |allocator: &mut Allocator| {
        raw::integration_counted(|| {
            source::integration_measured(|| {
                let mut denied = Vec::<u64>::new();
                let failure = allocator.vector_exact(&mut denied, 8, "real null calibration");
                let denied_capacity = denied.capacity();
                let mut ordinary = Vec::<u64>::new();
                let success = allocator.vector_exact(&mut ordinary, 8, "after real null");
                let ordinary_capacity = ordinary.capacity();
                drop(ordinary);
                drop(denied);
                (failure, success, denied_capacity, ordinary_capacity)
            })
        })
    };
    let control_bytes = null::selection_carriers_bytes(&action);
    let ((outcome, attempts), report) =
        null::with_selected(&mut allocator, target, action).unwrap();
    let ((failure, success, denied_capacity, ordinary_capacity), stats) = outcome;
    assert_eq!(failure, Err(ReserveFailure::Allocation));
    assert_eq!(success, Ok(()));
    assert_eq!(denied_capacity, 0);
    assert_eq!(ordinary_capacity, 8);
    assert_eq!(attempts, 2);
    assert_eq!(stats, (1, 0, 64));
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
    assert_eq!(allocator.attempts, 2);
    assert_eq!(allocator.trace.len(), 2);
    assert!(!allocator.trace[0].success);
    assert!(allocator.trace[1].success);
    assert!(!allocator.observer_trace_overflow);
    assert!(!raw::integration_enabled());
    assert!(!source::integration_enabled());
    println!("REAL_NULL_EXACT_CONTROL_BYTES {control_bytes}");
}

#[test]
fn allocator_review_controls_real_null_selected_unarmed_forwards_all_operations() {
    use crate::frontend::project::budget::{real_null_observer as null, Allocator};
    let mut allocator = Allocator::default();
    let target = null::Target {
        attempt: 1,
        kind: "unreached calibration",
        slots: 8,
        element_bytes: std::mem::size_of::<u64>(),
        layout: Layout::array::<u64>(8).unwrap(),
    };
    let action = |_: &mut Allocator| {
        raw::integration_counted(|| source::integration_measured(real_operations))
    };
    let control_bytes = null::selection_carriers_bytes(&action);
    let ((((), stats), attempts), report) =
        null::with_selected(&mut allocator, target, action).unwrap();
    assert_eq!(attempts, 3);
    assert_eq!(stats, (3, 0, 384));
    assert!(report.selected);
    assert!(!report.matched && !report.fired);
    assert_eq!(report.rejection, Some(null::Reason::MissingTarget));
    assert_eq!(report.actual, None);
    assert_eq!(allocator.attempts, 0);
    assert!(allocator.trace.is_empty());
    assert!(!raw::integration_enabled());
    assert!(!source::integration_enabled());
    println!("REAL_NULL_SELECTED_CONTROL_BYTES {control_bytes}");
}
