//! Test-only wiring for the two independent allocation observers.
//! Raw tests count attempts, including null results. Source tests count only
//! successful allocation calls and their live/peak requested payload bytes.
use crate::frontend::project::budget::real_null_observer::{global_event, growth, Operation};
use std::alloc::{GlobalAlloc, Layout, System};

struct ReviewerAllocator;
#[global_allocator]
static ALLOCATOR: ReviewerAllocator = ReviewerAllocator;

unsafe impl GlobalAlloc for ReviewerAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        super::reviewer_origins::note_alloc();
        let fresh_null = global_event(Operation::Alloc, layout, None);
        growth::global_event(Operation::Alloc, None, layout, None);
        if fresh_null {
            return std::ptr::null_mut();
        }
        let result = unsafe { System.alloc(layout) };
        if !result.is_null() {
            super::source::reviewer_source::account(layout.size() as isize, true);
        }
        result
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        super::reviewer_origins::note_alloc();
        global_event(Operation::AllocZeroed, layout, None);
        growth::global_event(Operation::AllocZeroed, None, layout, None);
        let result = unsafe { System.alloc_zeroed(layout) };
        if !result.is_null() {
            super::source::reviewer_source::account(layout.size() as isize, true);
        }
        result
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        global_event(Operation::Dealloc, layout, None);
        growth::global_event(Operation::Dealloc, Some(pointer as usize), layout, None);
        super::source::reviewer_source::account(-(layout.size() as isize), false);
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        super::reviewer_origins::note_alloc();
        global_event(Operation::Realloc, layout, Some(size));
        if growth::global_event(
            Operation::Realloc,
            Some(pointer as usize),
            layout,
            Some(size),
        ) {
            return std::ptr::null_mut();
        }
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            super::source::reviewer_source::account(size as isize - layout.size() as isize, true);
        }
        result
    }
}

#[cfg(test)]
#[path = "allocator_review_controls.rs"]
mod allocator_review_controls;

#[cfg(test)]
mod growth_review_controls {
    use super::super::{reviewer_origins as raw, source::reviewer_source as source};
    use crate::frontend::project::budget::{
        real_null_observer::{growth::*, Operation},
        Allocator, ReserveFailure,
    };
    use std::alloc::Layout;
    use std::mem::{align_of, size_of, size_of_val};

    fn target(attempt: usize) -> GrowthTarget {
        GrowthTarget {
            attempt,
            kind: "accounted full growth",
            old_len: 4,
            old_capacity: 4,
            additional: 4,
            new_slots: 8,
            element_bytes: size_of::<u64>(),
            element_align: align_of::<u64>(),
            old_layout: Layout::array::<u64>(4).unwrap(),
            new_layout: Layout::array::<u64>(8).unwrap(),
            operation: Operation::Realloc,
        }
    }

    fn full_lifetime(allocator: &mut Allocator) -> (bool, bool, bool) {
        let mut values = Vec::new();
        let initial = allocator
            .vector_exact(&mut values, 4, target(2).kind)
            .is_ok();
        if initial {
            values.extend_from_slice(&[11_u64, 22, 33, 44]);
        }
        let failed = allocator.vector_exact(&mut values, 4, target(2).kind)
            == Err(ReserveFailure::Allocation);
        let contents = values.as_slice() == [11, 22, 33, 44];
        drop(values);
        (initial, failed, contents)
    }

    fn valid(report: GrowthReport) {
        assert!(report.fired && report.matched && report.trace_preserved);
        assert_eq!(report.rejection, None);
        assert_eq!(report.reserve_failed, Some(true));
        assert_eq!(report.drop_count, 1);
        assert!(
            report.owner_unchanged
                && report.address_unchanged
                && report.length_unchanged
                && report.capacity_unchanged
        );
    }

    type OwnerFacts = (bool, bool, bool);
    type AllocationFacts = (usize, isize, isize);
    type ModeFacts = (OwnerFacts, Option<usize>, Option<AllocationFacts>);
    type MeasuredFacts = ((OwnerFacts, AllocationFacts), usize);

    fn whole_action(enabled: usize) -> impl for<'a> FnOnce(&'a mut Allocator) -> ModeFacts {
        move |allocator: &mut Allocator| match enabled {
            0 => {
                let ((facts, stats), attempts) = raw::integration_counted(|| {
                    source::integration_measured(|| full_lifetime(allocator))
                });
                (facts, Some(attempts), Some(stats))
            }
            1 => {
                let (facts, attempts) = raw::integration_counted(|| full_lifetime(allocator));
                (facts, Some(attempts), None)
            }
            2 => {
                let (facts, stats) = source::integration_measured(|| full_lifetime(allocator));
                (facts, None, Some(stats))
            }
            _ => (full_lifetime(allocator), None, None),
        }
    }

    fn old_live_action() -> impl for<'a> FnOnce(&'a mut Allocator) -> MeasuredFacts {
        move |allocator: &mut Allocator| {
            let mut values = Vec::new();
            let observed = raw::integration_counted(|| {
                source::integration_measured(|| {
                    let initial = allocator
                        .vector_exact(&mut values, 4, target(2).kind)
                        .is_ok();
                    if initial {
                        values.extend_from_slice(&[11_u64, 22, 33, 44]);
                    }
                    let failed = allocator.vector_exact(&mut values, 4, target(2).kind)
                        == Err(ReserveFailure::Allocation);
                    (initial, failed, values.as_slice() == [11, 22, 33, 44])
                })
            });
            // This interval intentionally reports the old live block after Err.
            // The observer selection continues through this one ordinary drop.
            drop(values);
            observed
        }
    }

    #[test]
    fn growth_real_null_accounted_whole_lifetime_both_each_and_disabled() {
        for enabled in 0..4 {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(2).unwrap();
            let ((facts, attempts, stats), report) =
                with_selected_growth(&mut allocator, target(2), whole_action(enabled)).unwrap();
            assert_eq!(facts, (true, true, true));
            valid(report);
            if let Some(attempts) = attempts {
                assert_eq!(attempts, 2);
            }
            if let Some(stats) = stats {
                assert_eq!(stats, (1, 0, 32));
            }
            assert_eq!(allocator.attempts, 2);
            assert_eq!(allocator.trace.len(), 2);
            assert!(allocator.trace[0].success);
            assert!(!allocator.trace[1].success);
            assert!(!raw::integration_enabled() && !source::integration_enabled());
        }
    }

    #[test]
    fn growth_real_null_accounted_old_live_interval_ends_before_ordinary_drop() {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(2).unwrap();
        let (((facts, stats), attempts), report) =
            with_selected_growth(&mut allocator, target(2), old_live_action()).unwrap();
        assert_eq!(facts, (true, true, true));
        assert_eq!(attempts, 2);
        assert_eq!(stats, (1, 32, 32));
        valid(report);
        assert!(!raw::integration_enabled() && !source::integration_enabled());
    }

    #[test]
    fn source_tracker_layout_only_no_selection_or_qualification() {
        let actual = source::integration_tracker_layout();
        let accessor_return = size_of::<(usize, usize, usize, usize, usize, usize)>();
        let accessor_caller = size_of_val(&actual);
        let raw_tls = size_of::<std::cell::Cell<bool>>()
            .checked_add(size_of::<std::cell::Cell<usize>>())
            .unwrap();
        let callback_payload = actual.2;
        let returned_payload = actual.2;
        let source_read = actual.4;
        let callback_scalars = size_of::<isize>().checked_add(size_of::<bool>()).unwrap();
        let raw_callback_scalars = size_of::<bool>().checked_add(size_of::<usize>()).unwrap();
        let callback_references = 3usize
            .checked_mul(size_of::<&std::cell::Cell<usize>>())
            .unwrap();
        let conservative = [
            actual.0,
            raw_tls,
            callback_payload,
            returned_payload,
            source_read,
            callback_scalars,
            raw_callback_scalars,
            callback_references,
            accessor_return,
            accessor_caller,
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .unwrap();
        println!("GROWTH_EXISTING_TRACKERS source_actual_tls={}/{} raw_tls={} source_callback_payload={}/{} source_returned_payload={} source_read_transport={}/{} callback_scalars={} raw_callback_scalars={} callback_references={} accessor_return={} accessor_caller={} conservative_named_sum={}",
            actual.0, actual.1, raw_tls, callback_payload, actual.3, returned_payload, source_read, actual.5, callback_scalars,
            raw_callback_scalars, callback_references, accessor_return, accessor_caller, conservative);
        assert!(actual.0 >= callback_payload);
        assert!(!raw::integration_enabled() && !source::integration_enabled());
    }

    #[test]
    fn growth_accounting_layout_only_no_selection_or_qualification() {
        let whole = whole_action(0);
        let old_live = old_live_action();
        fn measured<F, R>(name: &str, action: &F)
        where
            F: for<'a> FnOnce(&'a mut Allocator) -> R,
        {
            let selection = selection_carriers_bytes(action);
            let fixed = fixed_carriers_bytes::<u64>();
            let direct_owners = size_of::<Allocator>()
                .checked_add(size_of::<Vec<u64>>())
                .unwrap();
            let trace = 2usize
                .checked_mul(size_of::<crate::frontend::project::budget::ReserveEvent>())
                .unwrap();
            let selected_old = 4usize.checked_mul(size_of::<u64>()).unwrap();
            let parts = [selection, fixed, direct_owners, trace, selected_old];
            let admitted = parts
                .into_iter()
                .try_fold(0usize, usize::checked_add)
                .unwrap();
            assert!(parts
                .into_iter()
                .try_fold(0usize, usize::checked_add)
                .is_some_and(|bytes| bytes <= admitted));
            assert!(!parts
                .into_iter()
                .try_fold(0usize, usize::checked_add)
                .is_some_and(|bytes| bytes < admitted));
            println!("GROWTH_ACCOUNTING_FACTORY {name} F={}/{} R={}/{} selection={} named_fixed={} direct_owners={} trace_requested={} old_payload={} conservative_sum={}",
                size_of::<F>(),align_of::<F>(),size_of::<R>(),align_of::<R>(),selection,fixed,direct_owners,trace,selected_old,admitted);
        }
        measured("whole_modes", &whole);
        measured("old_live", &old_live);
        assert!(!raw::integration_enabled() && !source::integration_enabled());
    }
}
