//! Scoped, test-only real-null qualification. This module adds no production
//! allocator field or source authority. Selection alone never arms allocation.
//! Every action must preserve the selected Allocator's value and attempt history:
//! no replace/take/reset, fail_at mutation, retained identity, or nested selection.
use super::Allocator;
use std::alloc::{Layout, LayoutError};
use std::cell::Cell;
use std::collections::TryReserveError;
use std::marker::PhantomData;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct Target {
    pub attempt: usize,
    pub kind: &'static str,
    pub slots: usize,
    pub element_bytes: usize,
    pub layout: Layout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum Operation {
    Alloc,
    AllocZeroed,
    Realloc,
    Dealloc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct GlobalEvent {
    pub operation: Operation,
    pub layout: Layout,
    pub new_size: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum Reason {
    MissingTarget,
    WrongRequest,
    IneligibleVector,
    InvalidLayout,
    LogicalFailure,
    UnexpectedOperation,
    UnexpectedLayout,
    NestedReserve,
    NoGlobalCall,
    TlsUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct Report {
    pub target: Target,
    pub selected: bool,
    /// True only after all request and fresh-storage checks admitted the gate.
    pub matched: bool,
    pub fired: bool,
    pub rejection: Option<Reason>,
    pub actual: Option<GlobalEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum SetupError {
    InvalidTarget,
    NotFuture,
    LogicalFailure,
    Nested,
    TlsUnavailable,
}

/// Comparison only. Never dereferenced, exported in Report, or retained beyond
/// the short controller borrow. Stable address does not prohibit mem::replace;
/// that restriction is checked at each qualification action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AllocatorIdentity(usize);

pub(super) fn identity(allocator: &Allocator) -> AllocatorIdentity {
    AllocatorIdentity(std::ptr::from_ref(allocator) as usize)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    Selected {
        identity: AllocatorIdentity,
        target: Target,
    },
    Armed {
        identity: AllocatorIdentity,
        target: Target,
    },
    Finished(Report),
}

thread_local! {
    static STATE: Cell<State> = const { Cell::new(State::Idle) };
}

struct SelectionGuard {
    not_send_sync: PhantomData<Rc<()>>,
}

impl Drop for SelectionGuard {
    fn drop(&mut self) {
        let _ = STATE.try_with(|state| state.set(State::Idle));
    }
}

pub(super) struct ReserveGuard {
    not_send_sync: PhantomData<Rc<()>>,
}

/// The shared transition is also tested with a stack-local shadow Cell. Never
/// deliberately start an unwind while the real global-null decision is Armed.
fn reserve_cleanup(state: State) -> State {
    match state {
        State::Armed { target, .. } => State::Finished(Report {
            target,
            selected: true,
            matched: true,
            fired: false,
            rejection: Some(Reason::NoGlobalCall),
            actual: None,
        }),
        other => other,
    }
}

impl Drop for ReserveGuard {
    fn drop(&mut self) {
        let _ = STATE.try_with(|state| state.set(reserve_cleanup(state.get())));
    }
}

pub(in crate::frontend) fn with_selected<R>(
    allocator: &mut Allocator,
    target: Target,
    action: impl for<'a> FnOnce(&'a mut Allocator) -> R,
) -> Result<(R, Report), SetupError> {
    if target.slots == 0
        || target.element_bytes == 0
        || target.slots.checked_mul(target.element_bytes) != Some(target.layout.size())
    {
        return Err(SetupError::InvalidTarget);
    }
    if target.attempt <= allocator.attempts {
        return Err(SetupError::NotFuture);
    }
    if allocator.fail_at.is_some() {
        return Err(SetupError::LogicalFailure);
    }
    if !growth::is_idle().map_err(|_| SetupError::TlsUnavailable)? {
        return Err(SetupError::Nested);
    }
    let identity = identity(allocator);
    STATE
        .try_with(|state| {
            if state.get() != State::Idle {
                return Err(SetupError::Nested);
            }
            state.set(State::Selected { identity, target });
            Ok(())
        })
        .map_err(|_| SetupError::TlsUnavailable)??;
    let guard = SelectionGuard {
        not_send_sync: PhantomData,
    };
    // This is outside the TLS callback and only lends a shorter reborrow.
    let result = action(allocator);
    let report = STATE
        .try_with(|state| match reserve_cleanup(state.get()) {
            State::Finished(report) => report,
            _ => Report {
                target,
                selected: true,
                matched: false,
                fired: false,
                rejection: Some(Reason::MissingTarget),
                actual: None,
            },
        })
        .unwrap_or(Report {
            target,
            selected: true,
            matched: false,
            fired: false,
            rejection: Some(Reason::TlsUnavailable),
            actual: None,
        });
    drop(guard);
    Ok((result, report))
}

/// Only vector_exact, string and this module's internal calibrations call the gate.
/// All original checked request arithmetic precedes it. No owner, storage or action
/// enters the gate; only independently comparable fixed request facts do.
pub(super) fn enter_exact<T>(
    identity: AllocatorIdentity,
    attempt: usize,
    kind: &'static str,
    length: usize,
    capacity: usize,
    additional: usize,
    logical_failure: bool,
) -> Option<ReserveGuard> {
    STATE
        .try_with(|state| {
            let (selected_identity, target) = match state.get() {
                State::Selected { identity, target } => (identity, target),
                State::Armed { target, .. } => {
                    state.set(State::Finished(Report {
                        target,
                        selected: true,
                        matched: true,
                        fired: false,
                        rejection: Some(Reason::NestedReserve),
                        actual: None,
                    }));
                    return None;
                }
                State::Idle | State::Finished(_) => return None,
            };
            if identity != selected_identity || attempt < target.attempt {
                return None;
            }
            let rejection = if attempt > target.attempt {
                Some(Reason::MissingTarget)
            } else if logical_failure {
                Some(Reason::LogicalFailure)
            } else if kind != target.kind
                || additional != target.slots
                || std::mem::size_of::<T>() != target.element_bytes
            {
                Some(Reason::WrongRequest)
            } else if length != 0
                || capacity != 0
                || additional == 0
                || std::mem::size_of::<T>() == 0
            {
                Some(Reason::IneligibleVector)
            } else {
                match Layout::array::<T>(additional) {
                    Ok(layout) if layout == target.layout => None,
                    Ok(_) => Some(Reason::WrongRequest),
                    Err(_) => Some(Reason::InvalidLayout),
                }
            };
            if let Some(reason) = rejection {
                state.set(State::Finished(Report {
                    target,
                    selected: true,
                    matched: false,
                    fired: false,
                    rejection: Some(reason),
                    actual: None,
                }));
                return None;
            }
            state.set(State::Armed { identity, target });
            Some(ReserveGuard {
                not_send_sync: PhantomData,
            })
        })
        .ok()
        .flatten()
}

/// Called by the single existing test GlobalAlloc. All mismatches become
/// terminal before forwarding. Only an exact alloc returns the null decision.
pub(in crate::frontend) fn global_event(
    operation: Operation,
    layout: Layout,
    new_size: Option<usize>,
) -> bool {
    STATE
        .try_with(|state| {
            let State::Armed { target, .. } = state.get() else {
                return false;
            };
            let actual = GlobalEvent {
                operation,
                layout,
                new_size,
            };
            let rejection = if operation != Operation::Alloc || new_size.is_some() {
                Some(Reason::UnexpectedOperation)
            } else if layout != target.layout {
                Some(Reason::UnexpectedLayout)
            } else {
                None
            };
            let fired = rejection.is_none();
            state.set(State::Finished(Report {
                target,
                selected: true,
                matched: true,
                fired,
                rejection,
                actual: Some(actual),
            }));
            fired
        })
        .unwrap_or(false)
}

// Measurement-only observer banks. Excluded from the affected-HIR ledger by
// explicit scope, not evidence that their storage or transports are zero.
// These banks describe named observer controls, not universal callback/helper
// stack accounting. The approved observer exclusion does not change HIR prices.
#[allow(dead_code)]
struct StateCarriers {
    tls: Cell<State>,
    read: State,
    cleanup_input: State,
    cleanup_return: State,
    cleanup_caller: State,
    selection_guard: SelectionGuard,
    reserve_guard: ReserveGuard,
    target: Target,
    identity: AllocatorIdentity,
    report: Report,
    event: GlobalEvent,
}

#[allow(dead_code)]
struct ExactReserveCarriers {
    identity: AllocatorIdentity,
    attempt: usize,
    kind: &'static str,
    length: usize,
    capacity: usize,
    additional: usize,
    logical_failure: bool,
    selected_identity: AllocatorIdentity,
    target: Target,
    rejection: Option<Reason>,
    reason: Reason,
    layout_result: Result<Layout, LayoutError>,
    layout: Layout,
    returned_guard: Option<ReserveGuard>,
    caller_guard: Option<ReserveGuard>,
    reserve_result: Result<(), TryReserveError>,
}

#[allow(dead_code)]
struct SelectionCarriers<'a, F, R> {
    allocator: &'a mut Allocator,
    target: Target,
    caller_action: F,
    action: F,
    identity: AllocatorIdentity,
    install_return: Result<Result<(), SetupError>, std::thread::AccessError>,
    guard: SelectionGuard,
    result: R,
    report_return: Result<Report, std::thread::AccessError>,
    report: Report,
    returned: Result<(R, Report), SetupError>,
    caller: Result<(R, Report), SetupError>,
}

#[allow(dead_code)]
struct SelectionSizingCarriers<'a, F> {
    action: &'a F,
    returned: usize,
    caller: usize,
}

/// Uses each actual calibration closure/output type without invoking it or
/// manufacturing an Allocator/owner. This measures the named control banks,
/// not universal callback/library stack storage.
pub(in crate::frontend) fn selection_carriers_bytes<F, R>(_: &F) -> usize
where
    F: for<'a> FnOnce(&'a mut Allocator) -> R,
{
    std::mem::size_of::<SelectionCarriers<'_, F, R>>()
        + std::mem::size_of::<SelectionSizingCarriers<'_, F>>()
}

/// Closed primitive calibration output. No allocation pointer, guard, selected
/// identity or configurable arming capability escapes these drivers.
#[derive(Clone, Copy, Debug)]
pub(in crate::frontend) struct MismatchFacts {
    pub report: Report,
    pub armed: bool,
    pub operation_succeeded: bool,
    pub contents_preserved: bool,
    pub ordinary_succeeded: bool,
}

fn mismatch_calibration(operation: Operation) -> Result<MismatchFacts, SetupError> {
    use std::alloc::{alloc, alloc_zeroed, dealloc, realloc};
    let mut allocator = Allocator::default();
    let target = Target {
        attempt: 1,
        kind: "closed mismatch calibration",
        slots: 8,
        element_bytes: std::mem::size_of::<u64>(),
        layout: Layout::array::<u64>(8).unwrap(),
    };
    let old_layout = Layout::array::<u64>(4).unwrap();
    let ordinary_layout = Layout::array::<u64>(2).unwrap();
    let ((armed, operation_succeeded, contents_preserved, ordinary_succeeded), report) =
        with_selected(&mut allocator, target, |allocator| {
            // The caller measures the entire closed driver, so old backing is
            // allocated and freed inside the same interval, never before it.
            let old = if matches!(operation, Operation::Realloc | Operation::Dealloc) {
                unsafe { alloc(old_layout) }
            } else {
                std::ptr::null_mut()
            };
            if matches!(operation, Operation::Realloc | Operation::Dealloc) && old.is_null() {
                return (false, false, false, false);
            }
            if !old.is_null() {
                unsafe { old.write_bytes(0x5a, old_layout.size()) };
            }
            let guard = enter_exact::<u64>(identity(allocator), 1, target.kind, 0, 0, 8, false);
            // Only primitive branching and the direct nullable operation occur
            // while Armed. Each callback disarms before System forwarding.
            let pointer = unsafe {
                match operation {
                    Operation::Alloc => alloc(old_layout),
                    Operation::AllocZeroed => alloc_zeroed(target.layout),
                    Operation::Realloc => realloc(old, old_layout, target.layout.size()),
                    Operation::Dealloc => {
                        dealloc(old, old_layout);
                        std::ptr::null_mut()
                    }
                }
            };
            let armed = guard.is_some();
            drop(guard);
            let operation_succeeded = operation == Operation::Dealloc || !pointer.is_null();
            let contents_preserved = unsafe {
                match operation {
                    Operation::Alloc => operation_succeeded,
                    Operation::AllocZeroed => {
                        !pointer.is_null()
                            && std::slice::from_raw_parts(pointer, target.layout.size())
                                .iter()
                                .all(|byte| *byte == 0)
                    }
                    Operation::Realloc => {
                        !pointer.is_null()
                            && std::slice::from_raw_parts(pointer, old_layout.size())
                                .iter()
                                .all(|byte| *byte == 0x5a)
                    }
                    Operation::Dealloc => true,
                }
            };
            unsafe {
                if !pointer.is_null() {
                    dealloc(
                        pointer,
                        if operation == Operation::Alloc {
                            old_layout
                        } else {
                            target.layout
                        },
                    );
                } else if operation == Operation::Realloc {
                    // A genuine System realloc failure preserves the old block.
                    dealloc(old, old_layout);
                }
            }
            // Keep this measured allocation observable in optimized builds;
            // an unused alloc/dealloc pair can otherwise disappear entirely.
            let ordinary = std::hint::black_box(unsafe { alloc(ordinary_layout) });
            let ordinary_succeeded = !ordinary.is_null();
            if ordinary_succeeded {
                unsafe { dealloc(ordinary, ordinary_layout) };
            }
            (
                armed,
                operation_succeeded,
                contents_preserved,
                ordinary_succeeded,
            )
        })?;
    Ok(MismatchFacts {
        report,
        armed,
        operation_succeeded,
        contents_preserved,
        ordinary_succeeded,
    })
}

pub(in crate::frontend) fn calibrate_wrong_layout() -> Result<MismatchFacts, SetupError> {
    mismatch_calibration(Operation::Alloc)
}

pub(in crate::frontend) fn calibrate_zeroed_mismatch() -> Result<MismatchFacts, SetupError> {
    mismatch_calibration(Operation::AllocZeroed)
}

pub(in crate::frontend) fn calibrate_realloc_mismatch() -> Result<MismatchFacts, SetupError> {
    mismatch_calibration(Operation::Realloc)
}

pub(in crate::frontend) fn calibrate_dealloc_mismatch() -> Result<MismatchFacts, SetupError> {
    mismatch_calibration(Operation::Dealloc)
}

#[cfg(test)]
mod controls {
    use super::*;

    struct ShadowCleanupGuard<'a> {
        state: &'a Cell<State>,
        not_send_sync: PhantomData<Rc<()>>,
    }

    impl Drop for ShadowCleanupGuard<'_> {
        fn drop(&mut self) {
            self.state.set(reserve_cleanup(self.state.get()));
        }
    }

    #[allow(dead_code)]
    struct ShadowCarriers<'a, F> {
        state: Cell<State>,
        guard: ShadowCleanupGuard<'a>,
        closure: F,
        payload: Box<dyn std::any::Any + Send>,
        caught: Result<(), Box<dyn std::any::Any + Send>>,
    }

    fn shadow_bytes<F: FnOnce()>(_: &F) -> usize {
        std::mem::size_of::<ShadowCarriers<'_, F>>()
    }

    #[test]
    fn real_null_shadow_transition_cleans_up_on_unwind_with_real_tls_idle() {
        assert!(STATE.with(|state| state.get() == State::Idle));
        let allocator = Allocator::default();
        let target = Target {
            attempt: 1,
            kind: "shadow only",
            slots: 8,
            element_bytes: 8,
            layout: Layout::array::<u64>(8).unwrap(),
        };
        let state = Cell::new(State::Armed {
            identity: identity(&allocator),
            target,
        });
        let payload: Box<dyn std::any::Any + Send> = Box::new(73_u8);
        let action = || {
            let _guard = ShadowCleanupGuard {
                state: &state,
                not_send_sync: PhantomData,
            };
            std::panic::resume_unwind(payload);
        };
        let bytes = shadow_bytes(&action);
        // Unwinder allocation is permitted here: only the stack shadow is Armed.
        // The real null-injection state remains Idle for this entire control.
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action));
        assert!(caught.is_err());
        assert!(matches!(
            state.get(),
            State::Finished(Report {
                matched: true,
                fired: false,
                rejection: Some(Reason::NoGlobalCall),
                ..
            })
        ));
        assert!(STATE.with(|state| state.get() == State::Idle));
        assert!(bytes >= std::mem::size_of::<Cell<State>>());
        drop(caught);
    }

    #[test]
    fn real_null_initial_observer_fields_have_typed_layouts() {
        macro_rules! fields {
            ($model:ty; $($field:ident: $ty:ty),+ $(,)?) => {{
                $(let _: for<'a> fn(&'a $model) -> &'a $ty = |model| &model.$field;)+
                let extents = [$(
                    (std::mem::offset_of!($model, $field),
                     std::mem::size_of::<$ty>(), std::mem::align_of::<$ty>())
                ),+];
                for (index, &(offset, size, align)) in extents.iter().enumerate() {
                    assert_eq!(offset % align, 0);
                    assert!(offset.checked_add(size).unwrap() <= std::mem::size_of::<$model>());
                    for &(other, other_size, _) in &extents[..index] {
                        assert!(size == 0 || other_size == 0 || offset + size <= other
                            || other + other_size <= offset);
                    }
                }
                extents.len()
            }};
        }
        assert_eq!(
            fields!(SelectionGuard; not_send_sync: PhantomData<Rc<()>>),
            1
        );
        assert_eq!(fields!(ReserveGuard; not_send_sync: PhantomData<Rc<()>>), 1);
        assert_eq!(
            fields!(ShadowCleanupGuard<'static>; state: &'static Cell<State>,
            not_send_sync: PhantomData<Rc<()>>),
            2
        );
        assert_eq!(
            fields!(Target; attempt: usize, kind: &'static str, slots: usize,
            element_bytes: usize, layout: Layout),
            5
        );
        assert_eq!(
            fields!(GlobalEvent; operation: Operation, layout: Layout,
            new_size: Option<usize>),
            3
        );
        assert_eq!(
            fields!(Report; target: Target, selected: bool, matched: bool,
            fired: bool, rejection: Option<Reason>, actual: Option<GlobalEvent>),
            6
        );
        assert_eq!(
            fields!(StateCarriers; tls: Cell<State>, read: State,
            cleanup_input: State, cleanup_return: State, cleanup_caller: State,
            selection_guard: SelectionGuard, reserve_guard: ReserveGuard,
            target: Target, identity: AllocatorIdentity, report: Report, event: GlobalEvent),
            11
        );
        assert_eq!(
            fields!(ExactReserveCarriers; identity: AllocatorIdentity, attempt: usize,
            kind: &'static str, length: usize, capacity: usize, additional: usize,
            logical_failure: bool, selected_identity: AllocatorIdentity, target: Target,
            rejection: Option<Reason>, reason: Reason, layout_result: Result<Layout, LayoutError>,
            layout: Layout, returned_guard: Option<ReserveGuard>, caller_guard: Option<ReserveGuard>,
            reserve_result: Result<(), TryReserveError>),
            16
        );
        assert_eq!(
            fields!(MismatchFacts; report: Report, armed: bool,
            operation_succeeded: bool, contents_preserved: bool, ordinary_succeeded: bool),
            5
        );
        let _: for<'a> fn(&'a State) -> Option<(&'a AllocatorIdentity, &'a Target)> =
            |state| match state {
                State::Selected { identity, target } | State::Armed { identity, target } => {
                    Some((identity, target))
                }
                State::Idle | State::Finished(_) => None,
            };
        let _: for<'a> fn(&'a State) -> Option<&'a Report> = |state| {
            if let State::Finished(report) = state {
                Some(report)
            } else {
                None
            }
        };
        println!("REAL_NULL_FIXED_LAYOUT cell={}/{} selection_guard={}/{} reserve_guard={}/{} shadow_guard={}/{} mismatch={}/{} mismatch_result={}/{}",
            std::mem::size_of::<Cell<State>>(), std::mem::align_of::<Cell<State>>(),
            std::mem::size_of::<SelectionGuard>(), std::mem::align_of::<SelectionGuard>(),
            std::mem::size_of::<ReserveGuard>(), std::mem::align_of::<ReserveGuard>(),
            std::mem::size_of::<ShadowCleanupGuard<'_>>(), std::mem::align_of::<ShadowCleanupGuard<'_>>(),
            std::mem::size_of::<MismatchFacts>(), std::mem::align_of::<MismatchFacts>(),
            std::mem::size_of::<Result<MismatchFacts, SetupError>>(),
            std::mem::align_of::<Result<MismatchFacts, SetupError>>());
        println!(
            "REAL_NULL_EARLY_LAYOUT target={} report={} state={} state_bank={} reserve_bank={}",
            std::mem::size_of::<Target>(),
            std::mem::size_of::<Report>(),
            std::mem::size_of::<State>(),
            std::mem::size_of::<StateCarriers>(),
            std::mem::size_of::<ExactReserveCarriers>()
        );
    }

    fn target() -> Target {
        Target {
            attempt: 1,
            kind: "lifecycle calibration",
            slots: 8,
            element_bytes: std::mem::size_of::<u64>(),
            layout: Layout::array::<u64>(8).unwrap(),
        }
    }

    #[test]
    fn real_null_setup_refusals_do_not_call_action_or_install_selection() {
        for case in 0..7 {
            let mut allocator = Allocator::default();
            let mut target = target();
            let expected = match case {
                0 => {
                    target.slots = 0;
                    SetupError::InvalidTarget
                }
                1 => {
                    target.element_bytes = 0;
                    SetupError::InvalidTarget
                }
                2 => {
                    target.slots = usize::MAX;
                    SetupError::InvalidTarget
                }
                3 => {
                    target.slots = 7;
                    SetupError::InvalidTarget
                }
                4 => {
                    target.attempt = 0;
                    SetupError::NotFuture
                }
                5 => {
                    allocator.fail_at = Some(1);
                    SetupError::LogicalFailure
                }
                _ => {
                    allocator.fail_at = Some(2);
                    SetupError::LogicalFailure
                }
            };
            let called = Cell::new(false);
            let result = with_selected(&mut allocator, target, |_| called.set(true));
            assert_eq!(result, Err(expected));
            assert!(!called.get());
            assert_eq!(allocator.attempts, 0);
            assert!(allocator.trace.is_empty());
            assert!(STATE.with(|state| state.get() == State::Idle));
        }
    }

    #[test]
    fn real_null_gate_refusals_preserve_ordinary_vector_behavior() {
        for case in 0..9 {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(1).unwrap();
            let mut target = target();
            let mut values = if case >= 6 {
                Vec::<u64>::with_capacity(16)
            } else {
                Vec::new()
            };
            if case >= 7 {
                values.push(55);
            }
            if case == 8 {
                values.shrink_to_fit();
            }
            let before_len = values.len();
            let before_capacity = values.capacity();
            if case == 3 {
                target.layout =
                    Layout::from_size_align(target.layout.size(), target.layout.align() * 2)
                        .unwrap();
            } else if case == 2 {
                target.element_bytes = std::mem::size_of::<u32>();
                target.layout = Layout::array::<u32>(8).unwrap();
            }
            let (result, report) = with_selected(&mut allocator, target, |allocator| match case {
                0 => allocator.vector_exact(&mut values, 8, "wrong label"),
                1 => allocator.vector_exact(&mut values, 7, target.kind),
                4 => allocator.vector_exact(&mut values, 0, target.kind),
                5 => allocator.vector_exact(&mut Vec::<()>::new(), 8, target.kind),
                _ => allocator.vector_exact(&mut values, 8, target.kind),
            })
            .unwrap();
            assert_eq!(result, Ok(()));
            assert!(!report.matched && !report.fired);
            assert_eq!(
                report.rejection,
                Some(if case >= 6 {
                    Reason::IneligibleVector
                } else {
                    Reason::WrongRequest
                })
            );
            assert_eq!(report.actual, None);
            assert_eq!(values.len(), before_len);
            assert!(values.capacity() >= before_capacity);
            if before_len != 0 {
                assert_eq!(values[0], 55);
            }
            assert_eq!(allocator.attempts, 1);
            assert_eq!(allocator.trace.len(), 1);
            assert!(allocator.trace[0].success);
        }
    }

    #[test]
    fn real_null_pre_gate_overflow_never_arms_or_invents_attempts() {
        use super::super::ReserveFailure;
        for nonempty in [false, true] {
            let mut allocator = Allocator::default();
            let mut values = if nonempty { vec![7_u64] } else { Vec::new() };
            let (result, report) = with_selected(&mut allocator, target(), |allocator| {
                allocator.vector_exact(&mut values, usize::MAX, target().kind)
            })
            .unwrap();
            assert_eq!(result, Err(ReserveFailure::Overflow));
            assert_eq!(report.rejection, Some(Reason::MissingTarget));
            assert!(!report.fired);
            assert_eq!(allocator.attempts, 0);
            assert!(allocator.trace.is_empty());
            assert_eq!(values.len(), usize::from(nonempty));
        }
        // No valid future target exists here; leave selection disabled.
        let mut allocator = Allocator {
            attempts: usize::MAX,
            ..Allocator::default()
        };
        assert_eq!(
            allocator.vector_exact(&mut Vec::<u8>::new(), 0, "overflow"),
            Err(ReserveFailure::Overflow)
        );
        assert_eq!(allocator.attempts, usize::MAX);
        assert!(allocator.trace.is_empty());
    }

    #[test]
    fn real_null_missing_nested_and_terminal_scopes_cleanup_without_reset() {
        use super::super::ReserveFailure;
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(2).unwrap();
        let (_, missing) = with_selected(&mut allocator, target(), |_| ()).unwrap();
        assert_eq!(missing.rejection, Some(Reason::MissingTarget));
        let called = Cell::new(false);
        let ((before, failure, after), report) =
            with_selected(&mut allocator, target(), |allocator| {
                let before = with_selected(allocator, target(), |_| called.set(true));
                let failure = allocator.vector_exact(&mut Vec::<u64>::new(), 8, target().kind);
                let after = with_selected(
                    allocator,
                    Target {
                        attempt: 2,
                        ..target()
                    },
                    |_| called.set(true),
                );
                (before, failure, after)
            })
            .unwrap();
        assert_eq!(before, Err(SetupError::Nested));
        assert_eq!(after, Err(SetupError::Nested));
        assert!(!called.get());
        assert_eq!(failure, Err(ReserveFailure::Allocation));
        assert!(report.fired);
        assert!(STATE.with(|state| state.get() == State::Idle));
        let (failure, report) = with_selected(
            &mut allocator,
            Target {
                attempt: 2,
                ..target()
            },
            |allocator| allocator.vector_exact(&mut Vec::<u64>::new(), 8, target().kind),
        )
        .unwrap();
        assert_eq!(failure, Err(ReserveFailure::Allocation));
        assert!(report.fired);
        assert_eq!(allocator.attempts, 2);
    }

    #[test]
    fn real_null_private_guard_no_call_and_nested_entry_disarm() {
        for nested in [false, true] {
            let mut allocator = Allocator::default();
            let target = target();
            let (armed, report) = with_selected(&mut allocator, target, |allocator| {
                let first = enter_exact::<u64>(identity(allocator), 1, target.kind, 0, 0, 8, false);
                // No formatting, assertion, allocation, or unwind while Armed.
                let second = if nested {
                    enter_exact::<u64>(identity(allocator), 1, target.kind, 0, 0, 8, false)
                } else {
                    None
                };
                let armed = first.is_some();
                drop(second);
                drop(first);
                armed
            })
            .unwrap();
            assert!(armed && report.matched && !report.fired);
            assert_eq!(
                report.rejection,
                Some(if nested {
                    Reason::NestedReserve
                } else {
                    Reason::NoGlobalCall
                })
            );
            assert!(STATE.with(|state| state.get() == State::Idle));
        }
    }

    #[test]
    fn real_null_closed_driver_rejects_nested_armed_scope_without_disturbing_it() {
        let mut allocator = Allocator::default();
        let target = target();
        let (nested, report) = with_selected(&mut allocator, target, |allocator| {
            let guard = enter_exact::<u64>(identity(allocator), 1, target.kind, 0, 0, 8, false);
            // The closed driver's setup refuses before old-block allocation.
            let nested = calibrate_dealloc_mismatch();
            drop(guard);
            nested
        })
        .unwrap();
        assert!(matches!(nested, Err(SetupError::Nested)));
        assert_eq!(report.rejection, Some(Reason::NoGlobalCall));
        assert!(report.matched && !report.fired);
    }

    #[test]
    fn real_null_selected_unwind_clears_before_same_ordinal_reuse() {
        use super::super::ReserveFailure;
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(1).unwrap();
        let payload: Box<dyn std::any::Any + Send> = Box::new(91_u8);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_selected(&mut allocator, target(), |_: &mut Allocator| -> () {
                // Real TLS is only Selected: every unwinder allocation forwards.
                std::panic::resume_unwind(payload);
            })
        }));
        assert!(caught.is_err());
        drop(caught);
        assert!(STATE.with(|state| state.get() == State::Idle));
        assert_eq!(allocator.attempts, 0);
        let (result, report) = with_selected(&mut allocator, target(), |allocator| {
            allocator.vector_exact(&mut Vec::<u64>::new(), 8, target().kind)
        })
        .unwrap();
        assert_eq!(result, Err(ReserveFailure::Allocation));
        assert!(report.fired);
    }

    #[test]
    fn real_null_guards_are_neither_send_nor_sync() {
        trait AmbiguousIfSend<A> {
            fn check() {}
        }
        impl<T: ?Sized> AmbiguousIfSend<()> for T {}
        impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}
        trait AmbiguousIfSync<A> {
            fn check() {}
        }
        impl<T: ?Sized> AmbiguousIfSync<()> for T {}
        impl<T: ?Sized + Sync> AmbiguousIfSync<u8> for T {}
        let _ = <SelectionGuard as AmbiguousIfSend<_>>::check;
        let _ = <SelectionGuard as AmbiguousIfSync<_>>::check;
        let _ = <ReserveGuard as AmbiguousIfSend<_>>::check;
        let _ = <ReserveGuard as AmbiguousIfSync<_>>::check;
        let _ = <ShadowCleanupGuard<'static> as AmbiguousIfSend<_>>::check;
        let _ = <ShadowCleanupGuard<'static> as AmbiguousIfSync<_>>::check;
    }
}

/// Separate full-vector Realloc qualification. The original fresh API and
/// refusal types above are deliberately unchanged. All addresses here are
/// comparison-only and are discarded before forwarding after retirement.
pub(in crate::frontend) mod growth {
    use super::super::{ReserveEvent, ReserveFailure};
    use super::*;
    use std::mem::{align_of, size_of};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::frontend) struct GrowthTarget {
        pub attempt: usize,
        pub kind: &'static str,
        pub old_len: usize,
        pub old_capacity: usize,
        pub additional: usize,
        pub new_slots: usize,
        pub element_bytes: usize,
        pub element_align: usize,
        pub old_layout: Layout,
        pub new_layout: Layout,
        pub operation: Operation,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct GrowthBinding {
        allocator_identity: AllocatorIdentity,
        owner_address: usize,
        allocation_address: usize,
        attempt: usize,
        old_len: usize,
        old_capacity: usize,
        old_layout: Layout,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::frontend) struct GrowthEvent {
        pub operation: Operation,
        pub layout: Layout,
        pub new_size: Option<usize>,
        pub old_address_matches: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::frontend) struct GrowthDropEvent {
        pub layout: Layout,
        pub old_address_matches: bool,
        pub after_reserve_return: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::frontend) struct GrowthReport {
        pub target: GrowthTarget,
        pub selected: bool,
        pub matched: bool,
        pub fired: bool,
        pub rejection: Option<GrowthReason>,
        pub actual: Option<GrowthEvent>,
        pub reserve_failed: Option<bool>,
        pub owner_unchanged: bool,
        pub address_unchanged: bool,
        pub length_unchanged: bool,
        pub capacity_unchanged: bool,
        pub drop_event: Option<GrowthDropEvent>,
        pub drop_count: u8,
        pub trace_preserved: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::frontend) enum GrowthSetupError {
        InvalidTarget,
        NotFuture,
        LogicalFailure,
        UnpaidTrace,
        TraceTooShort,
        TraceInvalid,
        Nested,
        TlsUnavailable,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::frontend) enum GrowthReason {
        MissingTarget,
        WrongRequest,
        IneligibleVector,
        InvalidLayout,
        LogicalFailure,
        TraceInvalid,
        UnexpectedOperation,
        UnexpectedAddress,
        UnexpectedOldLayout,
        UnexpectedNewSize,
        NestedReserve,
        CrossArm,
        NoGlobalCall,
        ReserveResultMismatch,
        OwnerChanged,
        OldAllocationNotDropped,
        DeallocBeforeReserveReturn,
        UnexpectedDropLayout,
        PostNullRetry,
        TlsUnavailable,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct TraceSeal {
        start_attempt: usize,
        start_len: usize,
        limit: usize,
        capacity: usize,
        address: usize,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum GrowthState {
        Idle,
        Selected {
            identity: AllocatorIdentity,
            target: GrowthTarget,
            trace_seal: TraceSeal,
        },
        Armed {
            target: GrowthTarget,
            trace_seal: TraceSeal,
            binding: GrowthBinding,
        },
        AwaitingDrop {
            report: GrowthReport,
            trace_seal: TraceSeal,
            binding: GrowthBinding,
            reserve_checked: bool,
        },
        // Terminal state intentionally has no allocator, owner/base or trace
        // address. The selector's local seal still checks the final trace.
        Finished {
            report: GrowthReport,
        },
    }

    thread_local! {
        static GROWTH_STATE: Cell<GrowthState> = const { Cell::new(GrowthState::Idle) };
    }

    pub(super) fn is_idle() -> Result<bool, std::thread::AccessError> {
        GROWTH_STATE.try_with(|state| state.get() == GrowthState::Idle)
    }

    struct GrowthSelectionGuard {
        not_send_sync: PhantomData<Rc<()>>,
    }

    impl Drop for GrowthSelectionGuard {
        fn drop(&mut self) {
            let _ = GROWTH_STATE.try_with(|state| state.set(GrowthState::Idle));
        }
    }

    pub(in crate::frontend::project::budget) struct GrowthReserveGuard {
        not_send_sync: PhantomData<Rc<()>>,
    }

    impl Drop for GrowthReserveGuard {
        fn drop(&mut self) {
            let _ = GROWTH_STATE.try_with(|state| state.set(reserve_cleanup(state.get())));
        }
    }

    fn report(
        target: GrowthTarget,
        matched: bool,
        rejection: Option<GrowthReason>,
    ) -> GrowthReport {
        GrowthReport {
            target,
            selected: true,
            matched,
            fired: false,
            rejection,
            actual: None,
            reserve_failed: None,
            owner_unchanged: false,
            address_unchanged: false,
            length_unchanged: false,
            capacity_unchanged: false,
            drop_event: None,
            drop_count: 0,
            trace_preserved: false,
        }
    }

    fn reject(mut report: GrowthReport, reason: GrowthReason) -> GrowthReport {
        if report.rejection.is_none() {
            report.rejection = Some(reason);
        }
        report
    }

    fn valid_target(target: GrowthTarget) -> bool {
        target.operation == Operation::Realloc
            && target.old_len > 0
            && target.old_len == target.old_capacity
            && target.additional > 0
            && target.old_len.checked_add(target.additional) == Some(target.new_slots)
            && target.new_slots > target.old_capacity
            && target.element_bytes > 0
            && target
                .old_capacity
                .checked_mul(target.element_bytes)
                .and_then(|bytes| Layout::from_size_align(bytes, target.element_align).ok())
                == Some(target.old_layout)
            && target
                .new_slots
                .checked_mul(target.element_bytes)
                .and_then(|bytes| Layout::from_size_align(bytes, target.element_align).ok())
                == Some(target.new_layout)
    }

    fn seal_trace(
        allocator: &Allocator,
        target: GrowthTarget,
    ) -> Result<TraceSeal, GrowthSetupError> {
        let limit = allocator
            .observer_trace_limit
            .ok_or(GrowthSetupError::UnpaidTrace)?;
        if limit > 200_000
            || allocator.observer_trace_overflow
            || allocator.trace.len() > limit
            || allocator.trace.capacity() < limit
            || limit.checked_mul(size_of::<ReserveEvent>()).is_none()
        {
            return Err(GrowthSetupError::TraceInvalid);
        }
        let distance = target
            .attempt
            .checked_sub(allocator.attempts)
            .filter(|distance| *distance > 0)
            .ok_or(GrowthSetupError::NotFuture)?;
        if allocator
            .trace
            .len()
            .checked_add(distance)
            .is_none_or(|rows| rows > limit)
        {
            return Err(GrowthSetupError::TraceTooShort);
        }
        Ok(TraceSeal {
            start_attempt: allocator.attempts,
            start_len: allocator.trace.len(),
            limit,
            capacity: allocator.trace.capacity(),
            address: allocator.trace.as_ptr() as usize,
        })
    }

    fn trace_preserved(allocator: &Allocator, seal: TraceSeal) -> bool {
        allocator.observer_trace_limit == Some(seal.limit)
            && !allocator.observer_trace_overflow
            && allocator.trace.capacity() == seal.capacity
            && allocator.trace.as_ptr() as usize == seal.address
            && allocator.trace.len() <= seal.limit
    }

    fn trace_at_finish(allocator: &Allocator, seal: TraceSeal) -> bool {
        trace_preserved(allocator, seal)
            && allocator
                .attempts
                .checked_sub(seal.start_attempt)
                .and_then(|distance| seal.start_len.checked_add(distance))
                == Some(allocator.trace.len())
    }

    fn reserve_cleanup(state: GrowthState) -> GrowthState {
        match state {
            GrowthState::Armed { target, .. } => GrowthState::Finished {
                report: report(target, true, Some(GrowthReason::NoGlobalCall)),
            },
            other => other,
        }
    }

    fn unavailable_report(target: GrowthTarget) -> GrowthReport {
        report(target, false, Some(GrowthReason::TlsUnavailable))
    }

    fn selection_finish(state: GrowthState, target: GrowthTarget) -> GrowthReport {
        match reserve_cleanup(state) {
            GrowthState::Finished { report } => report,
            GrowthState::AwaitingDrop { report, .. } => {
                reject(report, GrowthReason::OldAllocationNotDropped)
            }
            _ => report(target, false, Some(GrowthReason::MissingTarget)),
        }
    }

    pub(in crate::frontend) fn with_selected_growth<R>(
        allocator: &mut Allocator,
        target: GrowthTarget,
        action: impl for<'a> FnOnce(&'a mut Allocator) -> R,
    ) -> Result<(R, GrowthReport), GrowthSetupError> {
        // The cross-family check precedes trace setup so even an unprepared
        // nested driver refuses without constructing any setup backing.
        if !STATE
            .try_with(|state| state.get() == State::Idle)
            .map_err(|_| GrowthSetupError::TlsUnavailable)?
            || !is_idle().map_err(|_| GrowthSetupError::TlsUnavailable)?
        {
            return Err(GrowthSetupError::Nested);
        }
        if !valid_target(target) {
            return Err(GrowthSetupError::InvalidTarget);
        }
        if target.attempt <= allocator.attempts {
            return Err(GrowthSetupError::NotFuture);
        }
        if allocator.fail_at.is_some() {
            return Err(GrowthSetupError::LogicalFailure);
        }
        let trace_seal = seal_trace(allocator, target)?;
        let identity = identity(allocator);
        GROWTH_STATE
            .try_with(|state| {
                state.set(GrowthState::Selected {
                    identity,
                    target,
                    trace_seal,
                });
            })
            .map_err(|_| GrowthSetupError::TlsUnavailable)?;
        let guard = GrowthSelectionGuard {
            not_send_sync: PhantomData,
        };
        let result = action(allocator);
        let mut report = GROWTH_STATE
            .try_with(|state| selection_finish(state.get(), target))
            .unwrap_or_else(|_| unavailable_report(target));
        report.trace_preserved = trace_at_finish(allocator, trace_seal);
        if !report.trace_preserved {
            report = reject(report, GrowthReason::TraceInvalid);
        }
        drop(guard);
        Ok((result, report))
    }

    /// Called only at vector_exact's synchronous exclusive borrow. Reading the
    /// live Vec header here establishes the binding; no pointer is dereferenced.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::frontend::project::budget) fn enter_full_exact<T>(
        identity: AllocatorIdentity,
        attempt: usize,
        kind: &'static str,
        vector: &Vec<T>,
        additional: usize,
        logical_failure: bool,
        trace_len: usize,
        trace_capacity: usize,
        trace_address: usize,
        trace_limit: Option<usize>,
        trace_overflow: bool,
    ) -> Option<GrowthReserveGuard> {
        GROWTH_STATE
            .try_with(|state| {
                let (selected_identity, target, trace_seal) = match state.get() {
                    GrowthState::Selected {
                        identity,
                        target,
                        trace_seal,
                    } => (identity, target, trace_seal),
                    GrowthState::Armed { target, .. } => {
                        state.set(GrowthState::Finished {
                            report: report(target, true, Some(GrowthReason::NestedReserve)),
                        });
                        return None;
                    }
                    _ => return None,
                };
                if identity != selected_identity || attempt < target.attempt {
                    return None;
                }
                let new_slots = vector.len().checked_add(additional);
                let old_layout = Layout::array::<T>(vector.capacity());
                let new_layout = Layout::array::<T>(new_slots.unwrap_or(usize::MAX));
                let rejection = if attempt > target.attempt {
                    Some(GrowthReason::MissingTarget)
                } else if logical_failure {
                    Some(GrowthReason::LogicalFailure)
                } else if !STATE
                    .try_with(|state| state.get() == State::Idle)
                    .unwrap_or(false)
                {
                    Some(GrowthReason::CrossArm)
                } else if trace_limit != Some(trace_seal.limit)
                    || trace_overflow
                    || trace_capacity != trace_seal.capacity
                    || trace_address != trace_seal.address
                    || attempt
                        .checked_sub(trace_seal.start_attempt)
                        .and_then(|distance| distance.checked_sub(1))
                        .and_then(|prefix| trace_seal.start_len.checked_add(prefix))
                        != Some(trace_len)
                {
                    Some(GrowthReason::TraceInvalid)
                } else if vector.is_empty()
                    || vector.len() != vector.capacity()
                    || additional == 0
                    || size_of::<T>() == 0
                {
                    Some(GrowthReason::IneligibleVector)
                } else if kind != target.kind
                    || additional != target.additional
                    || new_slots != Some(target.new_slots)
                    || vector.len() != target.old_len
                    || vector.capacity() != target.old_capacity
                    || size_of::<T>() != target.element_bytes
                    || align_of::<T>() != target.element_align
                {
                    Some(GrowthReason::WrongRequest)
                } else if old_layout.is_err() || new_layout.is_err() {
                    Some(GrowthReason::InvalidLayout)
                } else if old_layout.ok() != Some(target.old_layout)
                    || new_layout.ok() != Some(target.new_layout)
                {
                    Some(GrowthReason::WrongRequest)
                } else {
                    None
                };
                if let Some(reason) = rejection {
                    state.set(GrowthState::Finished {
                        report: report(target, false, Some(reason)),
                    });
                    return None;
                }
                let binding = GrowthBinding {
                    allocator_identity: identity,
                    owner_address: std::ptr::from_ref(vector) as usize,
                    allocation_address: vector.as_ptr() as usize,
                    attempt,
                    old_len: vector.len(),
                    old_capacity: vector.capacity(),
                    old_layout: target.old_layout,
                };
                state.set(GrowthState::Armed {
                    target,
                    trace_seal,
                    binding,
                });
                Some(GrowthReserveGuard {
                    not_send_sync: PhantomData,
                })
            })
            .ok()
            .flatten()
    }

    fn complete_transition(
        state: GrowthState,
        failed: bool,
        owner: usize,
        address: usize,
        length: usize,
        capacity: usize,
    ) -> GrowthState {
        let GrowthState::AwaitingDrop {
            mut report,
            trace_seal,
            binding,
            reserve_checked: false,
        } = state
        else {
            return state;
        };
        report.reserve_failed = Some(failed);
        report.owner_unchanged = owner == binding.owner_address;
        report.address_unchanged = address == binding.allocation_address;
        report.length_unchanged = length == binding.old_len;
        report.capacity_unchanged = capacity == binding.old_capacity;
        if !failed {
            return GrowthState::Finished {
                report: reject(report, GrowthReason::ReserveResultMismatch),
            };
        }
        if !report.owner_unchanged
            || !report.address_unchanged
            || !report.length_unchanged
            || !report.capacity_unchanged
        {
            return GrowthState::Finished {
                report: reject(report, GrowthReason::OwnerChanged),
            };
        }
        GrowthState::AwaitingDrop {
            report,
            trace_seal,
            binding,
            reserve_checked: true,
        }
    }

    pub(in crate::frontend::project::budget) fn reserve_complete<T>(vector: &Vec<T>, failed: bool) {
        let _ = GROWTH_STATE.try_with(|state| {
            state.set(complete_transition(
                state.get(),
                failed,
                std::ptr::from_ref(vector) as usize,
                vector.as_ptr() as usize,
                vector.len(),
                vector.capacity(),
            ))
        });
    }

    /// Pure transition also covers impossible lifecycle inputs without issuing
    /// invalid allocator operations. Finished states never compare addresses.
    fn global_transition(
        state: GrowthState,
        operation: Operation,
        address: Option<usize>,
        layout: Layout,
        new_size: Option<usize>,
        fresh_idle: bool,
    ) -> (GrowthState, bool) {
        match state {
            GrowthState::Armed {
                target,
                trace_seal,
                binding,
            } => {
                let actual = GrowthEvent {
                    operation,
                    layout,
                    new_size,
                    old_address_matches: address == Some(binding.allocation_address),
                };
                let rejection = if !fresh_idle {
                    Some(GrowthReason::CrossArm)
                } else if operation != Operation::Realloc {
                    Some(GrowthReason::UnexpectedOperation)
                } else if !actual.old_address_matches {
                    Some(GrowthReason::UnexpectedAddress)
                } else if layout != binding.old_layout {
                    Some(GrowthReason::UnexpectedOldLayout)
                } else if new_size
                    .and_then(|size| Layout::from_size_align(size, layout.align()).ok())
                    != Some(target.new_layout)
                {
                    Some(GrowthReason::UnexpectedNewSize)
                } else {
                    None
                };
                let mut report = report(target, true, rejection);
                report.actual = Some(actual);
                report.fired = rejection.is_none();
                if report.fired {
                    (
                        GrowthState::AwaitingDrop {
                            report,
                            trace_seal,
                            binding,
                            reserve_checked: false,
                        },
                        true,
                    )
                } else {
                    (GrowthState::Finished { report }, false)
                }
            }
            GrowthState::AwaitingDrop {
                mut report,
                binding,
                reserve_checked,
                ..
            } if address == Some(binding.allocation_address) && operation == Operation::Dealloc => {
                report.drop_event = Some(GrowthDropEvent {
                    layout,
                    old_address_matches: true,
                    after_reserve_return: reserve_checked,
                });
                report.drop_count = 1;
                if !reserve_checked {
                    report = reject(report, GrowthReason::DeallocBeforeReserveReturn);
                }
                if layout != binding.old_layout {
                    report = reject(report, GrowthReason::UnexpectedDropLayout);
                }
                (GrowthState::Finished { report }, false)
            }
            GrowthState::AwaitingDrop {
                report, binding, ..
            } if address == Some(binding.allocation_address) && operation == Operation::Realloc => {
                (
                    GrowthState::Finished {
                        report: reject(report, GrowthReason::PostNullRetry),
                    },
                    false,
                )
            }
            other => (other, false),
        }
    }

    pub(in crate::frontend) fn global_event(
        operation: Operation,
        address: Option<usize>,
        layout: Layout,
        new_size: Option<usize>,
    ) -> bool {
        GROWTH_STATE
            .try_with(|state| {
                let (next, decision) = global_transition(
                    state.get(),
                    operation,
                    address,
                    layout,
                    new_size,
                    STATE
                        .try_with(|fresh| fresh.get() == State::Idle)
                        .unwrap_or(false),
                );
                state.set(next);
                decision
            })
            .unwrap_or(false)
    }

    // Named conservative transport banks. Owners/backings remain in the driver
    // bank; these do not manufacture or duplicate owning Vec values.
    #[allow(dead_code)]
    struct GrowthStateCarriers {
        tls: Cell<GrowthState>,
        read: GrowthState,
        cleanup_input: GrowthState,
        cleanup_return: GrowthState,
        cleanup_caller: GrowthState,
        selection_guard: GrowthSelectionGuard,
        reserve_guard: GrowthReserveGuard,
        target: GrowthTarget,
        binding: GrowthBinding,
        trace_seal: TraceSeal,
        report: GrowthReport,
        event: GrowthEvent,
        drop_event: GrowthDropEvent,
    }

    #[allow(dead_code)]
    struct GrowthReserveCarriers<'a, T> {
        identity: AllocatorIdentity,
        attempt: usize,
        kind: &'static str,
        vector: &'a Vec<T>,
        owner_address: usize,
        allocation_address: usize,
        length: usize,
        capacity: usize,
        additional: usize,
        logical_failure: bool,
        selected_identity: AllocatorIdentity,
        target: GrowthTarget,
        trace_seal: TraceSeal,
        trace_len: usize,
        trace_capacity: usize,
        trace_address: usize,
        trace_limit: Option<usize>,
        trace_overflow: bool,
        checked_new_slots: Option<usize>,
        old_layout_result: Result<Layout, LayoutError>,
        new_layout_result: Result<Layout, LayoutError>,
        old_layout: Layout,
        new_layout: Layout,
        binding: GrowthBinding,
        rejection: Option<GrowthReason>,
        reason: GrowthReason,
        returned_guard: Option<GrowthReserveGuard>,
        caller_guard: Option<GrowthReserveGuard>,
        fresh_guard: Option<ReserveGuard>,
        reserve_result: Result<(), TryReserveError>,
        caller_result: Result<(), ReserveFailure>,
        post_owner: usize,
        post_address: usize,
        post_length: usize,
        post_capacity: usize,
        completion: bool,
    }

    #[allow(dead_code)]
    struct GrowthSelectionCarriers<'a, F, R> {
        allocator: &'a mut Allocator,
        target: GrowthTarget,
        caller_action: F,
        action: F,
        identity: AllocatorIdentity,
        trace_seal: TraceSeal,
        install_return: Result<Result<(), GrowthSetupError>, std::thread::AccessError>,
        guard: GrowthSelectionGuard,
        result: R,
        report_return: Result<GrowthReport, std::thread::AccessError>,
        report: GrowthReport,
        returned: Result<(R, GrowthReport), GrowthSetupError>,
        caller: Result<(R, GrowthReport), GrowthSetupError>,
    }

    #[allow(dead_code)]
    struct GrowthSelectionSizingCarriers<'a, F> {
        action: &'a F,
        returned: usize,
        caller: usize,
    }

    pub(in crate::frontend) fn selection_carriers_bytes<F, R>(_: &F) -> usize
    where
        F: for<'a> FnOnce(&'a mut Allocator) -> R,
    {
        size_of::<GrowthSelectionCarriers<'_, F, R>>()
            .checked_add(size_of::<GrowthSelectionSizingCarriers<'_, F>>())
            .unwrap()
    }

    #[allow(dead_code)]
    struct GrowthGlobalCarriers {
        operation: Operation,
        pointer: Option<usize>,
        address_matches: bool,
        layout: Layout,
        new_size: Option<usize>,
        state: GrowthState,
        target: GrowthTarget,
        binding: GrowthBinding,
        trace_seal: TraceSeal,
        event: GrowthEvent,
        rejection: Option<GrowthReason>,
        reason: GrowthReason,
        report: GrowthReport,
        state_to_store: GrowthState,
        returned: bool,
        fresh_decision: bool,
        growth_decision: bool,
        forwarded_result: *mut u8,
    }

    /// Conservative named growth banks plus both unchanged, simultaneously
    /// live fresh banks. This pure sizing operation never invokes a driver.
    pub(in crate::frontend) fn fixed_carriers_bytes<T>() -> usize {
        [
            size_of::<GrowthStateCarriers>(),
            size_of::<GrowthReserveCarriers<'_, T>>(),
            size_of::<GrowthGlobalCarriers>(),
            size_of::<GrowthDropCarriers>(),
            size_of::<StateCarriers>(),
            size_of::<ExactReserveCarriers>(),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .unwrap()
    }

    #[allow(dead_code)]
    struct GrowthDropCarriers {
        state: GrowthState,
        binding: GrowthBinding,
        pointer: Option<usize>,
        layout: Layout,
        event: GrowthDropEvent,
        report: GrowthReport,
        state_to_store: GrowthState,
        count: u8,
        checked: bool,
    }

    #[cfg(test)]
    mod controls {
        use super::*;

        fn target(attempt: usize) -> GrowthTarget {
            GrowthTarget {
                attempt,
                kind: "full growth control",
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

        fn allocator(rows: usize) -> Allocator {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(rows).unwrap();
            allocator
        }

        fn full() -> Vec<u64> {
            let mut values = Vec::new();
            values.try_reserve_exact(4).unwrap();
            values.extend_from_slice(&[11, 22, 33, 44]);
            assert_eq!(
                values.capacity(),
                4,
                "this qualification requires exact capacity"
            );
            values
        }

        fn enter(
            allocator: &Allocator,
            target: GrowthTarget,
            values: &Vec<u64>,
        ) -> Option<GrowthReserveGuard> {
            enter_full_exact(
                identity(allocator),
                target.attempt,
                target.kind,
                values,
                target.additional,
                allocator.fail_at.is_some(),
                allocator.trace.len(),
                allocator.trace.capacity(),
                allocator.trace.as_ptr() as usize,
                allocator.observer_trace_limit,
                allocator.observer_trace_overflow,
            )
        }

        fn accepted(report: GrowthReport, target: GrowthTarget) {
            assert_eq!(report.target, target);
            assert!(report.selected && report.matched && report.fired && report.trace_preserved);
            assert_eq!(report.rejection, None);
            assert_eq!(report.reserve_failed, Some(true));
            assert!(
                report.owner_unchanged
                    && report.address_unchanged
                    && report.length_unchanged
                    && report.capacity_unchanged
            );
            assert_eq!(
                report.actual,
                Some(GrowthEvent {
                    operation: Operation::Realloc,
                    layout: target.old_layout,
                    new_size: Some(target.new_layout.size()),
                    old_address_matches: true
                })
            );
            assert_eq!(
                report.drop_event,
                Some(GrowthDropEvent {
                    layout: target.old_layout,
                    old_address_matches: true,
                    after_reserve_return: true
                })
            );
            assert_eq!(report.drop_count, 1);
        }

        fn empty_action() -> impl for<'a> FnOnce(&'a mut Allocator) {
            |_: &mut Allocator| ()
        }
        fn called_action(called: &Cell<bool>) -> impl for<'a> FnOnce(&'a mut Allocator) + '_ {
            move |_: &mut Allocator| called.set(true)
        }

        fn full_action(
            target: GrowthTarget,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> (bool, bool, bool, bool) {
            move |allocator: &mut Allocator| {
                let mut values = Vec::new();
                let initial_ok = allocator.vector_exact(&mut values, 4, target.kind).is_ok();
                if initial_ok {
                    values.extend_from_slice(&[11_u64, 22, 33, 44]);
                }
                let failed = allocator.vector_exact(&mut values, 4, target.kind)
                    == Err(ReserveFailure::Allocation);
                let contents = values.as_slice() == [11, 22, 33, 44];
                let write_back = if failed && contents {
                    values[2] = 79;
                    values[2] == 79
                } else {
                    false
                };
                drop(values);
                (initial_ok, failed, contents, write_back)
            }
        }

        fn unrelated_action() -> impl for<'a> FnOnce(&'a mut Allocator) -> (bool, bool, bool) {
            |allocator: &mut Allocator| {
                let mut values = full();
                let failed = allocator.vector_exact(&mut values, 4, target(1).kind)
                    == Err(ReserveFailure::Allocation);
                let mut other = Vec::<u8>::new();
                let ordinary = allocator
                    .vector_exact(&mut other, 1, "unrelated while old owner lives")
                    .is_ok();
                let preserved = values.as_slice() == [11, 22, 33, 44];
                drop(other);
                drop(values);
                (failed, ordinary, preserved)
            }
        }

        fn sentinel_action<'a>(
            target: GrowthTarget,
            counters: &'a [Cell<u8>; 4],
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> (bool, bool, bool, bool) + 'a {
            move |allocator: &mut Allocator| {
                let mut values = Vec::new();
                allocator.vector_exact(&mut values, 4, target.kind).unwrap();
                for (i, drops) in counters.iter().enumerate() {
                    values.push(DropSentinel {
                        drops,
                        value: i as u64 + 17,
                    });
                }
                let failed = allocator.vector_exact(&mut values, 4, target.kind)
                    == Err(ReserveFailure::Allocation);
                let preserved = values
                    .iter()
                    .enumerate()
                    .all(|(i, value)| value.value == i as u64 + 17);
                let undropped = counters.iter().all(|counter| counter.get() == 0);
                values[1].value = 91;
                let written = values[1].value == 91;
                drop(values);
                (failed, preserved, undropped, written)
            }
        }

        fn gate_action<'a>(
            case: usize,
            expected: GrowthTarget,
            values: &'a mut Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> bool + 'a {
            move |allocator: &mut Allocator| match case {
                0 => allocator
                    .vector_exact(values, 4, "wrong growth kind")
                    .is_ok(),
                1 => allocator.vector_exact(values, 5, expected.kind).is_ok(),
                9 => allocator.vector_exact(values, 0, expected.kind).is_ok(),
                10 => allocator
                    .vector_exact(&mut vec![(); 4], 4, expected.kind)
                    .is_ok(),
                11 => {
                    let guard = enter_full_exact(
                        identity(allocator),
                        1,
                        expected.kind,
                        values,
                        4,
                        true,
                        allocator.trace.len(),
                        allocator.trace.capacity(),
                        allocator.trace.as_ptr() as usize,
                        allocator.observer_trace_limit,
                        allocator.observer_trace_overflow,
                    );
                    drop(guard);
                    allocator.vector_exact(values, 4, expected.kind).is_ok()
                }
                _ => allocator.vector_exact(values, 4, expected.kind).is_ok(),
            }
        }

        fn foreign_action<'a>(
            expected: GrowthTarget,
            foreign: &'a mut Allocator,
            foreign_values: &'a mut Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> (bool, bool, bool) + 'a {
            move |allocator: &mut Allocator| {
                let foreign_ok = foreign
                    .vector_exact(foreign_values, 4, expected.kind)
                    .is_ok();
                let earlier_ok = allocator
                    .vector_exact(&mut Vec::<u8>::new(), 0, "earlier")
                    .is_ok();
                let mut values = full();
                let failed = allocator.vector_exact(&mut values, 4, expected.kind)
                    == Err(ReserveFailure::Allocation);
                drop(values);
                (foreign_ok, earlier_ok, failed)
            }
        }

        fn missing_action(overshot: bool) -> impl for<'b> FnOnce(&'b mut Allocator) -> bool {
            move |allocator: &mut Allocator| {
                allocator
                    .vector(&mut Vec::<u8>::new(), 0, "different method")
                    .unwrap();
                if overshot {
                    let mut values = full();
                    allocator
                        .vector_exact(&mut values, 4, target(1).kind)
                        .is_ok()
                } else {
                    true
                }
            }
        }

        fn overflow_action<'a>(
            values: &'a mut Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> Result<(), ReserveFailure> + 'a {
            move |allocator: &mut Allocator| {
                allocator.vector_exact(values, usize::MAX, target(1).kind)
            }
        }

        fn mismatch_action<'a>(
            operation: Operation,
            layout: Layout,
            other: *mut u8,
            values: &'a mut Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> (bool, bool, bool, bool) + 'a {
            use std::alloc::{alloc, alloc_zeroed, dealloc, realloc};
            move |allocator: &mut Allocator| {
                let guard = enter(allocator, target(1), values);
                let pointer = unsafe {
                    match operation {
                        Operation::Alloc => alloc(layout),
                        Operation::AllocZeroed => alloc_zeroed(layout),
                        Operation::Realloc => realloc(other, layout, 64),
                        Operation::Dealloc => {
                            dealloc(other, layout);
                            std::ptr::null_mut()
                        }
                    }
                };
                let armed = guard.is_some();
                drop(guard);
                let operation_ok = operation == Operation::Dealloc || !pointer.is_null();
                let contents = match operation {
                    Operation::Realloc => {
                        !pointer.is_null()
                            && unsafe {
                                std::slice::from_raw_parts(pointer, 32)
                                    .iter()
                                    .all(|byte| *byte == 0x5a)
                            }
                    }
                    Operation::AllocZeroed => {
                        !pointer.is_null()
                            && unsafe {
                                std::slice::from_raw_parts(pointer, 32)
                                    .iter()
                                    .all(|byte| *byte == 0)
                            }
                    }
                    _ => true,
                };
                unsafe {
                    if !pointer.is_null() {
                        dealloc(
                            pointer,
                            if operation == Operation::Realloc {
                                target(1).new_layout
                            } else {
                                layout
                            },
                        );
                    }
                    if matches!(operation, Operation::Alloc | Operation::AllocZeroed)
                        || (operation == Operation::Realloc && pointer.is_null())
                    {
                        dealloc(other, layout);
                    }
                }
                let ordinary_ok = allocator.vector_exact(values, 4, target(1).kind).is_ok();
                (armed, operation_ok, contents, ordinary_ok)
            }
        }

        fn new_size_action<'a>(
            values: &'a mut Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> (bool, bool) + 'a {
            move |allocator: &mut Allocator| {
                let guard = enter(allocator, target(1), values);
                let ordinary = values.try_reserve_exact(5).is_ok();
                let armed = guard.is_some();
                drop(guard);
                (armed, ordinary)
            }
        }

        fn two_vectors_action<'a>(
            selected: &'a mut Vec<u64>,
            foreign: &'a mut Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> (bool, bool, bool) + 'a {
            move |allocator: &mut Allocator| {
                let guard = enter(allocator, target(1), selected);
                let foreign_ok = foreign.try_reserve_exact(4).is_ok();
                let armed = guard.is_some();
                drop(guard);
                let selected_ok = allocator.vector_exact(selected, 4, target(1).kind).is_ok();
                (armed, foreign_ok, selected_ok)
            }
        }

        fn retry_action<'a>(
            retry: bool,
            values: &'a mut Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> (bool, bool) + 'a {
            move |allocator: &mut Allocator| {
                let failed = allocator.vector_exact(values, 4, target(1).kind)
                    == Err(ReserveFailure::Allocation);
                let retry_ok = !retry || allocator.vector_exact(values, 4, target(1).kind).is_ok();
                (failed, retry_ok)
            }
        }

        fn exact_trace_action() -> impl for<'b> FnOnce(&'b mut Allocator) -> (bool, bool) {
            move |allocator: &mut Allocator| {
                let mut values = Vec::new();
                let initial = allocator
                    .vector_exact(&mut values, 4, target(2).kind)
                    .is_ok();
                values.extend_from_slice(&[11_u64, 22, 33, 44]);
                let failed = allocator.vector_exact(&mut values, 4, target(2).kind)
                    == Err(ReserveFailure::Allocation);
                drop(values);
                (initial, failed)
            }
        }

        fn trace_action(
            case: usize,
            mut values: Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) {
            move |allocator: &mut Allocator| {
                match case {
                    0 => {
                        allocator
                            .trace
                            .reserve_exact(allocator.trace.capacity() + 1);
                    }
                    1 => {
                        allocator.trace.push(ReserveEvent {
                            kind: "unrecorded",
                            length: 0,
                            element_bytes: 1,
                            success: true,
                        });
                    }
                    // Deliberately simulate forbidden controller ownership
                    // transfer as a negative control; never qualify it.
                    2 => {
                        let old = std::mem::take(allocator);
                        drop(old);
                    }
                    _ => {}
                }
                let _ = allocator.vector_exact(&mut values, 4, target(1).kind);
                drop(values);
                if case == 3 {
                    allocator
                        .vector_exact(&mut Vec::<u8>::new(), 0, "post target overflow")
                        .unwrap();
                }
            }
        }

        type FreshUnitResult = Result<((), Report), SetupError>;
        type GrowthUnitResult = Result<((), GrowthReport), GrowthSetupError>;
        type NestedGrowthFacts = (FreshUnitResult, GrowthUnitResult, bool);

        fn nested_growth_action<'a>(
            phase: usize,
            fresh: Target,
            values: &'a Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> NestedGrowthFacts + 'a {
            move |allocator: &mut Allocator| {
                let guard = if phase != 0 {
                    enter(allocator, target(1), values)
                } else {
                    None
                };
                let armed = guard.is_some();
                if phase == 2 {
                    drop(guard);
                } else {
                    let fresh_nested = with_selected(allocator, fresh, empty_action());
                    let growth_nested = with_selected_growth(allocator, target(1), empty_action());
                    drop(guard);
                    return (fresh_nested, growth_nested, armed);
                }
                (
                    with_selected(allocator, fresh, empty_action()),
                    with_selected_growth(allocator, target(1), empty_action()),
                    armed,
                )
            }
        }

        fn nested_fresh_action(
            phase: usize,
            fresh: Target,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> (GrowthUnitResult, bool) {
            move |allocator: &mut Allocator| {
                let guard = if phase != 0 {
                    enter_exact::<u64>(identity(allocator), 1, fresh.kind, 0, 0, 4, false)
                } else {
                    None
                };
                let armed = guard.is_some();
                if phase == 2 {
                    drop(guard);
                    (
                        with_selected_growth(allocator, target(1), empty_action()),
                        armed,
                    )
                } else {
                    let nested = with_selected_growth(allocator, target(1), empty_action());
                    drop(guard);
                    (nested, armed)
                }
            }
        }

        fn nested_reserve_action<'a>(
            values: &'a Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> (bool, bool) + 'a {
            move |allocator: &mut Allocator| {
                let first = enter(allocator, target(1), values);
                let second = enter(allocator, target(1), values);
                let result = (first.is_some(), second.is_some());
                drop(second);
                drop(first);
                result
            }
        }

        fn unwind_action(
            payload: Box<dyn std::any::Any + Send>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) {
            move |_: &mut Allocator| std::panic::resume_unwind(payload)
        }

        fn drop_action(mut values: Vec<u64>) -> impl for<'b> FnOnce(&'b mut Allocator) -> bool {
            move |allocator: &mut Allocator| {
                let failed = allocator.vector_exact(&mut values, 4, target(1).kind)
                    == Err(ReserveFailure::Allocation);
                drop(values);
                failed
            }
        }

        fn thread_action<'a>(
            phase: &'a std::sync::atomic::AtomicU8,
            mut values: Vec<u64>,
        ) -> impl for<'b> FnOnce(&'b mut Allocator) -> bool + 'a {
            use std::sync::atomic::Ordering;
            move |allocator: &mut Allocator| {
                phase.store(1, Ordering::Release);
                while phase.load(Ordering::Acquire) != 2 {
                    std::hint::spin_loop();
                }
                let failed = allocator.vector_exact(&mut values, 4, target(1).kind)
                    == Err(ReserveFailure::Allocation);
                drop(values);
                failed
            }
        }

        #[test]
        fn growth_real_null_full_owner_and_new_absolute_selection() {
            let mut allocator = allocator(4);
            for (initial, selected) in [(1, 2), (3, 4)] {
                let target = target(selected);
                let ((initial_ok, failed, contents, write_back), report) =
                    with_selected_growth(&mut allocator, target, full_action(target)).unwrap();
                assert!(initial_ok && failed && contents && write_back);
                accepted(report, target);
                assert_eq!(allocator.attempts, selected);
                assert!(allocator.trace[initial - 1].success);
                assert!(!allocator.trace[selected - 1].success);
            }
            let mut ordinary = Vec::<u64>::new();
            ordinary.try_reserve_exact(8).unwrap();
            ordinary.push(98);
            assert_eq!(ordinary[0], 98);
            drop(ordinary);
        }

        #[test]
        fn growth_real_null_unrelated_reserve_before_owner_drop_does_not_repeat_completion() {
            let mut allocator = allocator(2);
            let (facts, report) =
                with_selected_growth(&mut allocator, target(1), unrelated_action()).unwrap();
            assert_eq!(facts, (true, true, true));
            accepted(report, target(1));
            assert_eq!(allocator.attempts, 2);
            assert!(!allocator.trace[0].success);
            assert!(allocator.trace[1].success);
        }

        struct DropSentinel<'a> {
            drops: &'a Cell<u8>,
            value: u64,
        }
        impl Drop for DropSentinel<'_> {
            fn drop(&mut self) {
                self.drops.set(self.drops.get().saturating_add(1));
            }
        }

        #[test]
        fn growth_real_null_noncopy_elements_drop_once_after_null() {
            let mut allocator = allocator(2);
            let counters = [Cell::new(0_u8), Cell::new(0), Cell::new(0), Cell::new(0)];
            let target = GrowthTarget {
                element_bytes: size_of::<DropSentinel<'_>>(),
                element_align: align_of::<DropSentinel<'_>>(),
                old_layout: Layout::array::<DropSentinel<'_>>(4).unwrap(),
                new_layout: Layout::array::<DropSentinel<'_>>(8).unwrap(),
                ..target(2)
            };
            let ((failed, preserved, undropped, written), report) =
                with_selected_growth(&mut allocator, target, sentinel_action(target, &counters))
                    .unwrap();
            assert!(failed && preserved && undropped && written);
            assert!(counters.iter().all(|counter| counter.get() == 1));
            accepted(report, target);
        }

        #[test]
        fn growth_real_null_setup_refusal_matrix_keeps_action_and_history_untouched() {
            for case in 0..19 {
                let mut allocator = allocator(2);
                let mut target = target(1);
                let expected = match case {
                    0 => {
                        target.old_len = 0;
                        GrowthSetupError::InvalidTarget
                    }
                    1 => {
                        target.old_capacity = 0;
                        GrowthSetupError::InvalidTarget
                    }
                    2 => {
                        target.old_len = 3;
                        GrowthSetupError::InvalidTarget
                    }
                    3 => {
                        target.additional = 0;
                        GrowthSetupError::InvalidTarget
                    }
                    4 => {
                        target.new_slots = 4;
                        GrowthSetupError::InvalidTarget
                    }
                    5 => {
                        target.additional = usize::MAX;
                        GrowthSetupError::InvalidTarget
                    }
                    6 => {
                        target.element_bytes = usize::MAX;
                        GrowthSetupError::InvalidTarget
                    }
                    7 => {
                        target.element_bytes = 0;
                        GrowthSetupError::InvalidTarget
                    }
                    8 => {
                        target.element_align = 3;
                        GrowthSetupError::InvalidTarget
                    }
                    9 => {
                        target.old_layout = target.new_layout;
                        GrowthSetupError::InvalidTarget
                    }
                    10 => {
                        target.new_layout = target.old_layout;
                        GrowthSetupError::InvalidTarget
                    }
                    11 => {
                        target.operation = Operation::Alloc;
                        GrowthSetupError::InvalidTarget
                    }
                    12 => {
                        target.operation = Operation::AllocZeroed;
                        GrowthSetupError::InvalidTarget
                    }
                    13 => {
                        target.operation = Operation::Dealloc;
                        GrowthSetupError::InvalidTarget
                    }
                    14 => {
                        target.attempt = 0;
                        GrowthSetupError::NotFuture
                    }
                    15 => {
                        allocator.fail_at = Some(1);
                        GrowthSetupError::LogicalFailure
                    }
                    16 => {
                        allocator.fail_at = Some(99);
                        GrowthSetupError::LogicalFailure
                    }
                    17 => {
                        target.element_align *= 2;
                        GrowthSetupError::InvalidTarget
                    }
                    _ => {
                        target.old_len = usize::MAX;
                        target.old_capacity = usize::MAX;
                        GrowthSetupError::InvalidTarget
                    }
                };
                let called = Cell::new(false);
                assert_eq!(
                    with_selected_growth(&mut allocator, target, called_action(&called)),
                    Err(expected),
                    "case {case}"
                );
                assert!(!called.get());
                assert_eq!(allocator.attempts, 0);
                assert!(allocator.trace.is_empty());
                assert!(is_idle().unwrap());
            }
        }

        #[test]
        fn growth_real_null_gate_refusals_forward_and_preserve_prefix() {
            for case in 0..12 {
                let mut allocator = allocator(1);
                let mut expected = target(1);
                let mut values = match case {
                    4 => Vec::new(),
                    5 => Vec::with_capacity(4),
                    6 => {
                        let mut v = Vec::with_capacity(4);
                        v.push(11_u64);
                        v
                    }
                    7 => {
                        let mut v = Vec::with_capacity(8);
                        v.extend_from_slice(&[11, 22, 33, 44]);
                        v
                    }
                    8 => {
                        let mut v = Vec::with_capacity(8);
                        v.extend_from_slice(&[11, 22, 33, 44, 55, 66, 77, 88]);
                        v
                    }
                    _ => full(),
                };
                if case == 2 {
                    expected.element_bytes = size_of::<u32>();
                    expected.element_align = align_of::<u32>();
                    expected.old_layout = Layout::array::<u32>(4).unwrap();
                    expected.new_layout = Layout::array::<u32>(8).unwrap();
                }
                if case == 3 {
                    expected.element_align = 16;
                    expected.old_layout = Layout::from_size_align(32, 16).unwrap();
                    expected.new_layout = Layout::from_size_align(64, 16).unwrap();
                }
                let before = values.len();
                let (success, report) = with_selected_growth(
                    &mut allocator,
                    expected,
                    gate_action(case, expected, &mut values),
                )
                .unwrap();
                assert!(success);
                assert_eq!(values.len(), before);
                if before > 0 {
                    assert_eq!(values[0], 11);
                }
                assert!(!report.matched && !report.fired);
                assert_eq!(
                    report.rejection,
                    Some(if case == 11 {
                        GrowthReason::LogicalFailure
                    } else if matches!(case, 4..=7 | 9 | 10) {
                        GrowthReason::IneligibleVector
                    } else {
                        GrowthReason::WrongRequest
                    }),
                    "case {case}"
                );
                assert!(report.trace_preserved);
                assert_eq!(allocator.trace.len(), 1);
                assert!(allocator.trace[0].success);
            }
        }

        #[test]
        fn growth_real_null_foreign_earlier_and_missing_ordinals_never_retarget() {
            let mut selected = allocator(3);
            let mut foreign = allocator(2);
            let mut foreign_values = full();
            foreign
                .vector_exact(&mut Vec::<u8>::new(), 0, "prefix")
                .unwrap();
            let expected = target(2);
            let ((foreign_ok, earlier_ok, failed), report) = with_selected_growth(
                &mut selected,
                expected,
                foreign_action(expected, &mut foreign, &mut foreign_values),
            )
            .unwrap();
            assert!(foreign_ok && earlier_ok && failed);
            accepted(report, expected);
            assert_eq!(foreign_values.as_slice(), [11, 22, 33, 44]);
            for overshot in [false, true] {
                let mut allocator = allocator(2);
                let (ordinary, report) =
                    with_selected_growth(&mut allocator, target(1), missing_action(overshot))
                        .unwrap();
                assert!(ordinary && !report.fired);
                assert_eq!(report.rejection, Some(GrowthReason::MissingTarget));
            }
            let mut allocator = allocator(1);
            let mut values = full();
            let (outcome, report) =
                with_selected_growth(&mut allocator, target(1), overflow_action(&mut values))
                    .unwrap();
            assert_eq!(outcome, Err(ReserveFailure::Overflow));
            assert_eq!(allocator.attempts, 0);
            assert!(allocator.trace.is_empty());
            assert_eq!(report.rejection, Some(GrowthReason::MissingTarget));
        }

        fn shadow() -> GrowthState {
            GrowthState::Armed {
                target: target(1),
                trace_seal: TraceSeal {
                    start_attempt: 0,
                    start_len: 0,
                    limit: 1,
                    capacity: 1,
                    address: 900,
                },
                binding: GrowthBinding {
                    allocator_identity: AllocatorIdentity(100),
                    owner_address: 200,
                    allocation_address: 300,
                    attempt: 1,
                    old_len: 4,
                    old_capacity: 4,
                    old_layout: target(1).old_layout,
                },
            }
        }

        fn fired_shadow() -> GrowthState {
            let (state, fired) = global_transition(
                shadow(),
                Operation::Realloc,
                Some(300),
                target(1).old_layout,
                Some(64),
                true,
            );
            assert!(fired);
            state
        }

        fn retired(state: GrowthState, reason: GrowthReason) {
            let GrowthState::Finished { report } = state else {
                panic!("binding was not retired");
            };
            assert_eq!(report.rejection, Some(reason));
            for operation in [
                Operation::Alloc,
                Operation::AllocZeroed,
                Operation::Realloc,
                Operation::Dealloc,
            ] {
                let (after, denied) = global_transition(
                    state,
                    operation,
                    Some(300),
                    target(1).old_layout,
                    Some(64),
                    true,
                );
                assert!(!denied);
                assert_eq!(after, state);
            }
        }

        #[test]
        fn growth_real_null_shadow_global_matrix_disarms_before_forwarding() {
            for (operation, address, layout, size, expected) in [
                (
                    Operation::Alloc,
                    None,
                    target(1).old_layout,
                    None,
                    GrowthReason::UnexpectedOperation,
                ),
                (
                    Operation::AllocZeroed,
                    None,
                    target(1).old_layout,
                    None,
                    GrowthReason::UnexpectedOperation,
                ),
                (
                    Operation::Dealloc,
                    Some(300),
                    target(1).old_layout,
                    None,
                    GrowthReason::UnexpectedOperation,
                ),
                (
                    Operation::Realloc,
                    Some(301),
                    target(1).old_layout,
                    Some(64),
                    GrowthReason::UnexpectedAddress,
                ),
                (
                    Operation::Realloc,
                    None,
                    target(1).old_layout,
                    Some(64),
                    GrowthReason::UnexpectedAddress,
                ),
                (
                    Operation::Realloc,
                    Some(300),
                    Layout::from_size_align(32, 16).unwrap(),
                    Some(64),
                    GrowthReason::UnexpectedOldLayout,
                ),
                (
                    Operation::Realloc,
                    Some(300),
                    Layout::array::<u64>(3).unwrap(),
                    Some(64),
                    GrowthReason::UnexpectedOldLayout,
                ),
                (
                    Operation::Realloc,
                    Some(300),
                    target(1).old_layout,
                    Some(72),
                    GrowthReason::UnexpectedNewSize,
                ),
                (
                    Operation::Realloc,
                    Some(300),
                    target(1).old_layout,
                    None,
                    GrowthReason::UnexpectedNewSize,
                ),
                (
                    Operation::Realloc,
                    Some(300),
                    target(1).old_layout,
                    Some(usize::MAX),
                    GrowthReason::UnexpectedNewSize,
                ),
            ] {
                let (state, denied) =
                    global_transition(shadow(), operation, address, layout, size, true);
                assert!(!denied);
                retired(state, expected);
            }
            let (state, denied) = global_transition(
                shadow(),
                Operation::Realloc,
                Some(300),
                target(1).old_layout,
                Some(64),
                false,
            );
            assert!(!denied);
            retired(state, GrowthReason::CrossArm);
            retired(reserve_cleanup(shadow()), GrowthReason::NoGlobalCall);
        }

        #[test]
        fn growth_real_null_shadow_lifecycle_retirement_is_address_free() {
            for (failed, owner, address, length, capacity, reason) in [
                (false, 200, 300, 4, 4, GrowthReason::ReserveResultMismatch),
                (true, 201, 300, 4, 4, GrowthReason::OwnerChanged),
                (true, 200, 301, 4, 4, GrowthReason::OwnerChanged),
                (true, 200, 300, 3, 4, GrowthReason::OwnerChanged),
                (true, 200, 300, 4, 8, GrowthReason::OwnerChanged),
            ] {
                retired(
                    complete_transition(fired_shadow(), failed, owner, address, length, capacity),
                    reason,
                );
            }
            let (early, denied) = global_transition(
                fired_shadow(),
                Operation::Dealloc,
                Some(300),
                target(1).old_layout,
                None,
                true,
            );
            assert!(!denied);
            retired(early, GrowthReason::DeallocBeforeReserveReturn);
            let complete = complete_transition(fired_shadow(), true, 200, 300, 4, 4);
            assert_eq!(
                complete_transition(complete, false, 999, 998, 0, 1),
                complete,
                "unrelated subsequent reserves cannot repeat the selected completion"
            );
            let (wrong, denied) = global_transition(
                complete,
                Operation::Dealloc,
                Some(300),
                Layout::array::<u64>(3).unwrap(),
                None,
                true,
            );
            assert!(!denied);
            retired(wrong, GrowthReason::UnexpectedDropLayout);
            let (retry, denied) = global_transition(
                complete,
                Operation::Realloc,
                Some(300),
                target(1).old_layout,
                Some(64),
                true,
            );
            assert!(!denied);
            retired(retry, GrowthReason::PostNullRetry);
            let (dropped, denied) = global_transition(
                complete,
                Operation::Dealloc,
                Some(300),
                target(1).old_layout,
                None,
                true,
            );
            assert!(!denied);
            let GrowthState::Finished { report } = dropped else {
                panic!("drop retains binding");
            };
            assert_eq!(report.drop_count, 1);
            assert_eq!(report.rejection, None);
            assert_eq!(
                global_transition(
                    dropped,
                    Operation::Dealloc,
                    Some(300),
                    target(1).old_layout,
                    None,
                    true
                ),
                (dropped, false)
            );
            assert_eq!(
                selection_finish(complete, target(1)).rejection,
                Some(GrowthReason::OldAllocationNotDropped)
            );
        }

        #[test]
        fn growth_real_null_real_global_mismatches_and_interference_forward_once() {
            use std::alloc::alloc;
            for operation in [
                Operation::Alloc,
                Operation::AllocZeroed,
                Operation::Dealloc,
                Operation::Realloc,
            ] {
                let mut allocator = allocator(1);
                let mut values = full();
                let layout = target(1).old_layout;
                let other = unsafe { alloc(layout) };
                assert!(!other.is_null());
                unsafe {
                    other.write_bytes(0x5a, layout.size());
                }
                let ((armed, operation_ok, contents, ordinary_ok), report) = with_selected_growth(
                    &mut allocator,
                    target(1),
                    mismatch_action(operation, layout, other, &mut values),
                )
                .unwrap();
                assert!(armed && operation_ok && contents && ordinary_ok);
                assert_eq!(values.as_slice(), [11, 22, 33, 44]);
                assert!(!report.fired && report.matched);
                assert_eq!(
                    report.rejection,
                    Some(if operation == Operation::Realloc {
                        GrowthReason::UnexpectedAddress
                    } else {
                        GrowthReason::UnexpectedOperation
                    })
                );
                assert!(report.trace_preserved);
            }
            // A valid reserve using a different new size is a normal Vec-owned
            // operation; the mismatch retires the binding before it can move.
            let mut allocator = allocator(1);
            let mut values = full();
            let ((armed, ordinary), report) =
                with_selected_growth(&mut allocator, target(1), new_size_action(&mut values))
                    .unwrap();
            assert!(armed && ordinary);
            assert_eq!(report.rejection, Some(GrowthReason::UnexpectedNewSize));
            assert_eq!(values.as_slice(), [11, 22, 33, 44]);
        }

        #[test]
        fn growth_real_null_two_live_same_layout_vectors_bind_only_the_selected_owner() {
            let mut allocator = allocator(1);
            let mut selected = full();
            let mut foreign = full();
            assert_ne!(selected.as_ptr(), foreign.as_ptr());
            let ((armed, foreign_ok, selected_ok), report) = with_selected_growth(
                &mut allocator,
                target(1),
                two_vectors_action(&mut selected, &mut foreign),
            )
            .unwrap();
            assert!(armed && foreign_ok && selected_ok);
            assert_eq!(report.rejection, Some(GrowthReason::UnexpectedAddress));
            assert!(!report.fired);
            assert_eq!(selected.as_slice(), [11, 22, 33, 44]);
            assert_eq!(foreign.as_slice(), [11, 22, 33, 44]);
        }

        #[test]
        fn growth_real_null_live_owner_missing_drop_and_post_null_retry_refuse() {
            for retry in [false, true] {
                let mut allocator = allocator(if retry { 2 } else { 1 });
                let mut values = full();
                let ((failed, retry_ok), report) = with_selected_growth(
                    &mut allocator,
                    target(1),
                    retry_action(retry, &mut values),
                )
                .unwrap();
                assert!(failed && retry_ok);
                assert_eq!(values.as_slice(), [11, 22, 33, 44]);
                assert_eq!(
                    report.rejection,
                    Some(if retry {
                        GrowthReason::PostNullRetry
                    } else {
                        GrowthReason::OldAllocationNotDropped
                    })
                );
                assert_eq!(report.drop_count, 0);
                assert!(report.fired && report.trace_preserved);
                // The intentionally retained caller owner is dropped normally,
                // after the selection has cleared every comparison address.
                drop(values);
                assert!(is_idle().unwrap());
            }
        }

        #[test]
        fn growth_real_null_trace_setup_and_exact_one_short_matrix() {
            for case in 0..7 {
                let mut allocator = if case == 0 {
                    Allocator::default()
                } else {
                    allocator(if case == 1 { 0 } else { 2 })
                };
                let expected = match case {
                    0 => GrowthSetupError::UnpaidTrace,
                    1 => GrowthSetupError::TraceTooShort,
                    2 => {
                        allocator.observer_trace_limit = Some(1);
                        GrowthSetupError::TraceTooShort
                    }
                    3 => {
                        allocator.observer_trace_overflow = true;
                        GrowthSetupError::TraceInvalid
                    }
                    4 => {
                        allocator.observer_trace_limit = Some(allocator.trace.capacity() + 1);
                        GrowthSetupError::TraceInvalid
                    }
                    5 => {
                        allocator.observer_trace_limit = Some(200_001);
                        GrowthSetupError::TraceInvalid
                    }
                    _ => {
                        allocator.trace.push(ReserveEvent {
                            kind: "old",
                            length: 0,
                            element_bytes: 1,
                            success: true,
                        });
                        allocator.observer_trace_limit = Some(0);
                        GrowthSetupError::TraceInvalid
                    }
                };
                let called = Cell::new(false);
                assert_eq!(
                    with_selected_growth(&mut allocator, target(2), called_action(&called)),
                    Err(expected)
                );
                assert!(!called.get());
                assert_eq!(allocator.attempts, 0);
            }
            let mut allocator = allocator(2);
            let ((initial, failed), report) =
                with_selected_growth(&mut allocator, target(2), exact_trace_action()).unwrap();
            assert!(initial && failed);
            accepted(report, target(2));
            assert_eq!(allocator.trace.len(), 2);
            let mut own = Allocator::default();
            assert_eq!(
                own.observer_trace_bound_capacity_failure(2),
                Err(ReserveFailure::Allocation)
            );
            assert_eq!(own.attempts, 0);
            assert!(own.trace.is_empty());
            assert!(is_idle().unwrap());
        }

        #[test]
        fn growth_real_null_trace_storage_prefix_and_terminal_overflow_are_not_hidden() {
            for case in 0..4 {
                let mut allocator = allocator(if case == 3 { 1 } else { 2 });
                let values = full();
                let (_, report) =
                    with_selected_growth(&mut allocator, target(1), trace_action(case, values))
                        .unwrap();
                assert_eq!(report.rejection, Some(GrowthReason::TraceInvalid));
                assert!(!report.trace_preserved);
                assert_eq!(report.fired, case == 3);
                if case == 3 {
                    assert_eq!(report.drop_count, 1);
                    assert!(allocator.observer_trace_overflow);
                    assert_eq!(allocator.trace.len(), 1);
                }
            }
        }

        #[test]
        fn growth_real_null_nested_families_selected_armed_and_terminal_are_refused() {
            let mut allocator = allocator(2);
            let mut values = full();
            let fresh = Target {
                attempt: 1,
                kind: "fresh nested",
                slots: 4,
                element_bytes: 8,
                layout: Layout::array::<u64>(4).unwrap(),
            };
            for phase in 0..3 {
                let ((fresh_nested, growth_nested, armed), report) = with_selected_growth(
                    &mut allocator,
                    target(1),
                    nested_growth_action(phase, fresh, &values),
                )
                .unwrap();
                assert_eq!(fresh_nested, Err(SetupError::Nested));
                assert_eq!(growth_nested, Err(GrowthSetupError::Nested));
                assert_eq!(armed, phase != 0);
                assert_eq!(
                    report.rejection,
                    Some(if phase == 0 {
                        GrowthReason::MissingTarget
                    } else {
                        GrowthReason::NoGlobalCall
                    })
                );
            }
            for phase in 0..3 {
                let ((nested, armed), report) =
                    with_selected(&mut allocator, fresh, nested_fresh_action(phase, fresh))
                        .unwrap();
                assert_eq!(nested, Err(GrowthSetupError::Nested));
                assert_eq!(armed, phase != 0);
                assert_eq!(
                    report.rejection,
                    Some(if phase == 0 {
                        Reason::MissingTarget
                    } else {
                        Reason::NoGlobalCall
                    })
                );
            }
            let ((first, second), report) =
                with_selected_growth(&mut allocator, target(1), nested_reserve_action(&values))
                    .unwrap();
            assert!(first && !second);
            assert_eq!(report.rejection, Some(GrowthReason::NestedReserve));
            values[0] = 12;
        }

        struct ShadowGuard<'a> {
            state: &'a Cell<GrowthState>,
            not_send_sync: PhantomData<Rc<()>>,
        }
        impl Drop for ShadowGuard<'_> {
            fn drop(&mut self) {
                self.state.set(reserve_cleanup(self.state.get()));
            }
        }

        fn shadow_unwind_action<'a>(
            state: &'a Cell<GrowthState>,
            payload: Box<dyn std::any::Any + Send>,
        ) -> impl FnOnce() + 'a {
            move || {
                let _guard = ShadowGuard {
                    state,
                    not_send_sync: PhantomData,
                };
                std::panic::resume_unwind(payload);
            }
        }

        fn child_action(
            phase: std::sync::Arc<std::sync::atomic::AtomicU8>,
        ) -> impl FnOnce() -> bool {
            move || {
                use std::sync::atomic::Ordering;
                let mut allocator = allocator(1);
                let mut values = full();
                while phase.load(Ordering::Acquire) != 1 {
                    std::hint::spin_loop();
                }
                let ordinary = allocator
                    .vector_exact(&mut values, 4, target(1).kind)
                    .is_ok();
                drop(values);
                phase.store(2, Ordering::Release);
                ordinary
            }
        }

        #[test]
        fn growth_real_null_unwind_and_guard_traits_clean_without_armed_tls_unwind() {
            let mut allocator = allocator(1);
            let payload: Box<dyn std::any::Any + Send> = Box::new(17_u8);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_selected_growth(&mut allocator, target(1), unwind_action(payload))
            }));
            assert!(caught.is_err());
            drop(caught);
            assert!(is_idle().unwrap());
            assert_eq!(allocator.attempts, 0);
            let values = full();
            let (failed, report) =
                with_selected_growth(&mut allocator, target(1), drop_action(values)).unwrap();
            assert!(failed);
            accepted(report, target(1));
            let state = Cell::new(shadow());
            let payload: Box<dyn std::any::Any + Send> = Box::new(19_u8);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                shadow_unwind_action(&state, payload),
            ));
            assert!(caught.is_err());
            drop(caught);
            assert!(is_idle().unwrap());
            retired(state.get(), GrowthReason::NoGlobalCall);
            trait AmbiguousIfSend<A> {
                fn check() {}
            }
            impl<T: ?Sized> AmbiguousIfSend<()> for T {}
            impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}
            trait AmbiguousIfSync<A> {
                fn check() {}
            }
            impl<T: ?Sized> AmbiguousIfSync<()> for T {}
            impl<T: ?Sized + Sync> AmbiguousIfSync<u8> for T {}
            let _ = <GrowthSelectionGuard as AmbiguousIfSend<_>>::check;
            let _ = <GrowthSelectionGuard as AmbiguousIfSync<_>>::check;
            let _ = <GrowthReserveGuard as AmbiguousIfSend<_>>::check;
            let _ = <GrowthReserveGuard as AmbiguousIfSync<_>>::check;
            let _ = <ShadowGuard<'static> as AmbiguousIfSend<_>>::check;
            let _ = <ShadowGuard<'static> as AmbiguousIfSync<_>>::check;
        }

        #[test]
        fn growth_real_null_tls_unavailable_helper_is_closed_and_terminal() {
            // Destruction-order TLS unavailability is not forced on this host.
            // Exercise the same address-free report helper used by the real
            // unavailable read, then prove every later operation forwards.
            let report = unavailable_report(target(1));
            assert!(report.selected && !report.matched && !report.fired);
            assert_eq!(report.actual, None);
            assert_eq!(report.drop_count, 0);
            retired(
                GrowthState::Finished { report },
                GrowthReason::TlsUnavailable,
            );
        }

        #[test]
        fn growth_real_null_child_thread_never_consumes_parent_selection() {
            use std::sync::{atomic::AtomicU8, Arc};
            let phase = Arc::new(AtomicU8::new(0));
            let child_phase = Arc::clone(&phase);
            let child = std::thread::spawn(child_action(child_phase));
            let mut allocator = allocator(1);
            let values = full();
            let (failed, report) =
                with_selected_growth(&mut allocator, target(1), thread_action(&phase, values))
                    .unwrap();
            assert!(failed && child.join().unwrap());
            accepted(report, target(1));
        }

        #[allow(dead_code)]
        struct DriverCarriers<'a, F, R> {
            allocator: Allocator,
            vector: Vec<u64>,
            foreign_allocator: Allocator,
            foreign_vector: Vec<u64>,
            counters: [Cell<u8>; 4],
            expected: [u64; 4],
            sentinel_vector: Vec<DropSentinel<'a>>,
            target: GrowthTarget,
            report: GrowthReport,
            error: GrowthSetupError,
            result: Result<(R, GrowthReport), GrowthSetupError>,
            closure: F,
            probe: Vec<u64>,
            raw_guard: (),
            source_guard: (),
            raw_stats: usize,
            source_stats: (usize, isize, isize),
        }

        fn checked_named_admission(parts: &[usize], ceiling: usize) -> Option<usize> {
            parts
                .iter()
                .try_fold(0usize, |sum, part| sum.checked_add(*part))
                .filter(|sum| *sum <= ceiling)
        }

        fn measure_action<F, R>(
            name: &str,
            action: F,
            fixture_requested: usize,
            fixture_actual: usize,
        ) -> usize
        where
            F: for<'a> FnOnce(&'a mut Allocator) -> R,
        {
            let selection = selection_carriers_bytes(&action);
            let driver = size_of::<DriverCarriers<'_, F, R>>();
            let parts = [
                fixed_carriers_bytes::<u64>(),
                selection,
                driver,
                fixture_requested,
            ];
            let exact = parts
                .into_iter()
                .try_fold(0usize, usize::checked_add)
                .unwrap();
            assert_eq!(checked_named_admission(&parts, exact), Some(exact));
            assert_eq!(checked_named_admission(&parts, exact - 1), None);
            println!("GROWTH_DRIVER {name} F={}/{} R={}/{} selection={}/{} sizing={}/{} driver={}/{} fixture_requested={} fixture_actual={} named_plus_requested={}",
                size_of::<F>(),align_of::<F>(),size_of::<R>(),align_of::<R>(),
                size_of::<GrowthSelectionCarriers<'_,F,R>>(),align_of::<GrowthSelectionCarriers<'_,F,R>>(),
                size_of::<GrowthSelectionSizingCarriers<'_,F>>(),align_of::<GrowthSelectionSizingCarriers<'_,F>>(),
                driver,align_of::<DriverCarriers<'_,F,R>>(),fixture_requested,fixture_actual,exact);
            // Only the factory is invoked. Dropping its never-invoked closure
            // releases any ordinary fixture owner it captured, with TLS Idle.
            drop(action);
            exact
        }

        fn measure_fresh_action<F, R>(name: &str, action: F)
        where
            F: for<'a> FnOnce(&'a mut Allocator) -> R,
        {
            let bank = super::super::selection_carriers_bytes(&action);
            println!(
                "GROWTH_DRIVER_FRESH {name} F={}/{} R={}/{} selection={}/{} sizing={}/{} total={}",
                size_of::<F>(),
                align_of::<F>(),
                size_of::<R>(),
                align_of::<R>(),
                size_of::<SelectionCarriers<'_, F, R>>(),
                align_of::<SelectionCarriers<'_, F, R>>(),
                size_of::<SelectionSizingCarriers<'_, F>>(),
                align_of::<SelectionSizingCarriers<'_, F>>(),
                bank
            );
            drop(action);
        }

        #[allow(dead_code)]
        struct ShadowDriverCarriers<'a, F> {
            state: Cell<GrowthState>,
            guard: ShadowGuard<'a>,
            action: F,
            payload: Box<dyn std::any::Any + Send>,
            caught: Result<(), Box<dyn std::any::Any + Send>>,
        }

        #[test]
        fn growth_trace_layout_only_exact_one_short_never_selects() {
            let exact = allocator(2);
            let short = allocator(1);
            let zero = allocator(0);
            let expected = target(2);
            let seal = seal_trace(&exact, expected).unwrap();
            assert_eq!(seal.start_attempt, 0);
            assert_eq!(seal.start_len, 0);
            assert_eq!(seal.limit, 2);
            assert_eq!(
                seal_trace(&short, expected),
                Err(GrowthSetupError::TraceTooShort)
            );
            assert_eq!(
                seal_trace(&zero, target(1)),
                Err(GrowthSetupError::TraceTooShort)
            );
            assert_eq!(exact.attempts, 0);
            assert_eq!(short.attempts, 0);
            assert_eq!(zero.attempts, 0);
            assert!(exact.trace.is_empty() && short.trace.is_empty() && zero.trace.is_empty());
            println!("GROWTH_TRACE_ADMISSION exact_q=2 short_q=1 exact_requested={} exact_actual={} short_requested={} short_actual={}; pure_setup_only_no_selector_no_frontend_aux_meter",
                2*size_of::<ReserveEvent>(),exact.trace.capacity()*size_of::<ReserveEvent>(),
                size_of::<ReserveEvent>(),short.trace.capacity()*size_of::<ReserveEvent>());
            assert!(is_idle().unwrap());
            assert!(STATE.with(|state| state.get() == State::Idle));
        }

        #[test]
        fn growth_driver_layout_only_same_factories_never_invoke_callbacks() {
            use std::alloc::{alloc, dealloc};
            let mut maximum = 0usize;
            macro_rules! measured {
                ($name:expr,$action:expr,$requested:expr,$actual:expr) => {{
                    maximum = maximum.max(measure_action($name, $action, $requested, $actual));
                }};
            }
            let called = Cell::new(false);
            measured!("empty", empty_action(), 0, 0);
            measure_fresh_action("nested_empty", empty_action());
            measured!("called", called_action(&called), 0, 0);
            measured!("full", full_action(target(2)), 0, 0);
            measured!("unrelated_before_drop", unrelated_action(), 0, 0);
            let counters = [Cell::new(0u8), Cell::new(0), Cell::new(0), Cell::new(0)];
            let sentinel_target = GrowthTarget {
                element_bytes: size_of::<DropSentinel<'_>>(),
                element_align: align_of::<DropSentinel<'_>>(),
                old_layout: Layout::array::<DropSentinel<'_>>(4).unwrap(),
                new_layout: Layout::array::<DropSentinel<'_>>(8).unwrap(),
                ..target(2)
            };
            measured!(
                "sentinel",
                sentinel_action(sentinel_target, &counters),
                0,
                0
            );
            // These are the same initialized ordinary fixtures as the runtime
            // controls. No selector is installed and no action is invoked.
            let mut values = full();
            let payload = values.capacity().checked_mul(size_of::<u64>()).unwrap();
            measured!("gate", gate_action(0, target(1), &mut values), 32, payload);
            let mut foreign = allocator(2);
            foreign
                .vector_exact(&mut Vec::<u8>::new(), 0, "prefix")
                .unwrap();
            let foreign_trace = foreign
                .trace
                .capacity()
                .checked_mul(size_of::<ReserveEvent>())
                .unwrap();
            measured!(
                "foreign",
                foreign_action(target(2), &mut foreign, &mut values),
                112,
                payload + foreign_trace
            );
            measured!("missing", missing_action(false), 0, 0);
            measured!("overflow", overflow_action(&mut values), 32, payload);
            let layout = target(1).old_layout;
            let other = unsafe { alloc(layout) };
            assert!(!other.is_null());
            unsafe {
                other.write_bytes(0x5a, layout.size());
            }
            measured!(
                "global_mismatch",
                mismatch_action(Operation::Realloc, layout, other, &mut values),
                64,
                payload + layout.size()
            );
            unsafe {
                dealloc(other, layout);
            }
            measured!("new_size", new_size_action(&mut values), 32, payload);
            let mut second = full();
            let second_payload = second.capacity().checked_mul(size_of::<u64>()).unwrap();
            measured!(
                "two_vectors",
                two_vectors_action(&mut values, &mut second),
                64,
                payload + second_payload
            );
            measured!("retry", retry_action(true, &mut values), 32, payload);
            measured!("exact_trace", exact_trace_action(), 0, 0);
            let owned = full();
            let owned_payload = owned.capacity() * size_of::<u64>();
            measured!("trace_mutation", trace_action(0, owned), 32, owned_payload);
            let fresh = Target {
                attempt: 1,
                kind: "fresh nested",
                slots: 4,
                element_bytes: 8,
                layout,
            };
            measured!(
                "nested_growth",
                nested_growth_action(1, fresh, &values),
                32,
                payload
            );
            measure_fresh_action("nested_fresh", nested_fresh_action(1, fresh));
            measured!(
                "nested_reserve",
                nested_reserve_action(&values),
                32,
                payload
            );
            let payload: Box<dyn std::any::Any + Send> = Box::new(17_u8);
            measured!(
                "selected_unwind",
                unwind_action(payload),
                size_of::<u8>(),
                size_of::<u8>()
            );
            let owned = full();
            let owned_payload = owned.capacity() * size_of::<u64>();
            measured!("drop_owner", drop_action(owned), 32, owned_payload);
            let phase = std::sync::atomic::AtomicU8::new(0);
            let owned = full();
            let owned_payload = owned.capacity() * size_of::<u64>();
            measured!(
                "thread_parent",
                thread_action(&phase, owned),
                32,
                owned_payload
            );
            let shadow_state = Cell::new(shadow());
            let shadow_payload: Box<dyn std::any::Any + Send> = Box::new(19_u8);
            let shadow_action = shadow_unwind_action(&shadow_state, shadow_payload);
            fn measure_shadow<F: FnOnce()>(action: F) {
                println!(
                    "GROWTH_SHADOW_DRIVER F={}/{} bank={}/{} payload_requested=1 payload_actual=1",
                    size_of::<F>(),
                    align_of::<F>(),
                    size_of::<ShadowDriverCarriers<'_, F>>(),
                    align_of::<ShadowDriverCarriers<'_, F>>()
                );
                drop(action);
            }
            measure_shadow(shadow_action);
            let child_phase = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0));
            let child = child_action(std::sync::Arc::clone(&child_phase));
            fn measure_child<F: FnOnce() -> bool>(action: F) {
                println!("GROWTH_THREAD_DRIVER F={}/{} result={}/{} join_handle={}/{} per_thread_growth_tls={}/{} per_thread_fresh_tls={}/{}",
                    size_of::<F>(),align_of::<F>(),size_of::<bool>(),align_of::<bool>(),
                    size_of::<std::thread::JoinHandle<bool>>(),align_of::<std::thread::JoinHandle<bool>>(),
                    size_of::<Cell<GrowthState>>(),align_of::<Cell<GrowthState>>(),size_of::<Cell<State>>(),align_of::<Cell<State>>());
                drop(action);
            }
            measure_child(child);
            assert_eq!(child_phase.load(std::sync::atomic::Ordering::Relaxed), 0);
            assert!(!called.get());
            assert!(counters.iter().all(|counter| counter.get() == 0));
            assert_eq!(values.as_slice(), [11, 22, 33, 44]);
            assert_eq!(second.as_slice(), [11, 22, 33, 44]);
            assert_eq!(foreign.attempts, 1);
            assert_eq!(phase.load(std::sync::atomic::Ordering::Relaxed), 0);
            assert_eq!(checked_named_admission(&[usize::MAX, 1], usize::MAX), None);
            assert_eq!(checked_named_admission(&[1], 0), None);
            assert_eq!(checked_named_admission(&[0], 0), Some(0));
            println!("GROWTH_DRIVER_MAX conservative_named_plus_fixture_requested={maximum}; actual_fixture_backings_are_separate_from_closure_headers_and_selector_transports");
            assert!(is_idle().unwrap());
            assert!(STATE.with(|state| state.get() == State::Idle));
        }

        #[test]
        #[allow(clippy::type_complexity)] // Typed projections prove each enum field's exact type.
        fn growth_layout_manifest_only_no_selection_or_qualification() {
            // This is the sole prequalification measurement: size/offset checks
            // and closure type inspection only. It never calls any driver.
            macro_rules! fields {
                ($model:ty; $($field:ident:$ty:ty),+ $(,)?) => {{
                    $(let _:for<'a> fn(&'a $model)->&'a $ty=|model|&model.$field;)+
                    let extents=[$((std::mem::offset_of!($model,$field),size_of::<$ty>(),align_of::<$ty>())),+];
                    for (index,&(offset,size,align)) in extents.iter().enumerate(){
                        assert_eq!(offset%align,0);assert!(offset.checked_add(size).unwrap()<=size_of::<$model>());
                        for &(other,other_size,_) in &extents[..index]{assert!(size==0||other_size==0||offset+size<=other||other+other_size<=offset);}
                    }
                    println!("GROWTH_FIELDS {} {:?}",stringify!($model),extents);
                }};
            }
            fields!(GrowthTarget; attempt:usize,kind:&'static str,old_len:usize,old_capacity:usize,
                additional:usize,new_slots:usize,element_bytes:usize,element_align:usize,old_layout:Layout,new_layout:Layout,operation:Operation);
            fields!(GrowthBinding;allocator_identity:AllocatorIdentity,owner_address:usize,allocation_address:usize,
                attempt:usize,old_len:usize,old_capacity:usize,old_layout:Layout);
            fields!(TraceSeal;start_attempt:usize,start_len:usize,limit:usize,capacity:usize,address:usize);
            fields!(GrowthEvent;operation:Operation,layout:Layout,new_size:Option<usize>,old_address_matches:bool);
            fields!(GrowthDropEvent;layout:Layout,old_address_matches:bool,after_reserve_return:bool);
            fields!(GrowthReport;target:GrowthTarget,selected:bool,matched:bool,fired:bool,rejection:Option<GrowthReason>,
                actual:Option<GrowthEvent>,reserve_failed:Option<bool>,owner_unchanged:bool,address_unchanged:bool,
                length_unchanged:bool,capacity_unchanged:bool,drop_event:Option<GrowthDropEvent>,drop_count:u8,trace_preserved:bool);
            fields!(GrowthSelectionGuard;not_send_sync:PhantomData<Rc<()>>);
            fields!(GrowthReserveGuard;not_send_sync:PhantomData<Rc<()>>);
            fields!(GrowthStateCarriers;tls:Cell<GrowthState>,read:GrowthState,cleanup_input:GrowthState,
                cleanup_return:GrowthState,cleanup_caller:GrowthState,selection_guard:GrowthSelectionGuard,
                reserve_guard:GrowthReserveGuard,target:GrowthTarget,binding:GrowthBinding,trace_seal:TraceSeal,
                report:GrowthReport,event:GrowthEvent,drop_event:GrowthDropEvent);
            fields!(GrowthReserveCarriers<'static,u64>;identity:AllocatorIdentity,attempt:usize,kind:&'static str,vector:&'static Vec<u64>,
                owner_address:usize,allocation_address:usize,length:usize,capacity:usize,additional:usize,logical_failure:bool,
                selected_identity:AllocatorIdentity,target:GrowthTarget,trace_seal:TraceSeal,trace_len:usize,trace_capacity:usize,
                trace_address:usize,trace_limit:Option<usize>,trace_overflow:bool,checked_new_slots:Option<usize>,
                old_layout_result:Result<Layout,LayoutError>,new_layout_result:Result<Layout,LayoutError>,old_layout:Layout,new_layout:Layout,
                binding:GrowthBinding,rejection:Option<GrowthReason>,reason:GrowthReason,returned_guard:Option<GrowthReserveGuard>,
                caller_guard:Option<GrowthReserveGuard>,fresh_guard:Option<ReserveGuard>,reserve_result:Result<(),TryReserveError>,
                caller_result:Result<(),ReserveFailure>,post_owner:usize,post_address:usize,post_length:usize,post_capacity:usize,completion:bool);
            fields!(GrowthGlobalCarriers;operation:Operation,pointer:Option<usize>,address_matches:bool,layout:Layout,new_size:Option<usize>,
                state:GrowthState,target:GrowthTarget,binding:GrowthBinding,trace_seal:TraceSeal,event:GrowthEvent,rejection:Option<GrowthReason>,
                reason:GrowthReason,report:GrowthReport,state_to_store:GrowthState,returned:bool,fresh_decision:bool,growth_decision:bool,forwarded_result:*mut u8);
            fields!(GrowthDropCarriers;state:GrowthState,binding:GrowthBinding,pointer:Option<usize>,layout:Layout,event:GrowthDropEvent,
                report:GrowthReport,state_to_store:GrowthState,count:u8,checked:bool);
            fields!(GrowthSelectionCarriers<'static,fn(&mut Allocator)->bool,bool>;allocator:&'static mut Allocator,target:GrowthTarget,
                caller_action:fn(&mut Allocator)->bool,action:fn(&mut Allocator)->bool,identity:AllocatorIdentity,trace_seal:TraceSeal,
                install_return:Result<Result<(),GrowthSetupError>,std::thread::AccessError>,guard:GrowthSelectionGuard,result:bool,
                report_return:Result<GrowthReport,std::thread::AccessError>,report:GrowthReport,
                returned:Result<(bool,GrowthReport),GrowthSetupError>,caller:Result<(bool,GrowthReport),GrowthSetupError>);
            fields!(GrowthSelectionSizingCarriers<'static,fn(&mut Allocator)->bool>;action:&'static fn(&mut Allocator)->bool,returned:usize,caller:usize);
            let _: for<'a> fn(
                &'a GrowthState,
            )
                -> Option<(&'a GrowthTarget, &'a TraceSeal, &'a GrowthBinding)> =
                |state| match state {
                    GrowthState::Armed {
                        target,
                        trace_seal,
                        binding,
                    } => Some((target, trace_seal, binding)),
                    _ => None,
                };
            let _: for<'a> fn(
                &'a GrowthState,
            ) -> Option<(
                &'a GrowthReport,
                &'a TraceSeal,
                &'a GrowthBinding,
                &'a bool,
            )> = |state| match state {
                GrowthState::AwaitingDrop {
                    report,
                    trace_seal,
                    binding,
                    reserve_checked,
                } => Some((report, trace_seal, binding, reserve_checked)),
                _ => None,
            };
            let _: for<'a> fn(&'a GrowthState) -> Option<&'a GrowthReport> = |state| match state {
                GrowthState::Finished { report } => Some(report),
                _ => None,
            };
            macro_rules! row {
                ($type:ty) => {{
                    println!(
                        "GROWTH_LAYOUT {} {}/{}",
                        stringify!($type),
                        size_of::<$type>(),
                        align_of::<$type>()
                    );
                    size_of::<$type>()
                }};
            }
            let _ = row!(GrowthTarget);
            let _ = row!(GrowthBinding);
            let _ = row!(TraceSeal);
            let _ = row!(GrowthEvent);
            let _ = row!(GrowthDropEvent);
            let _ = row!(GrowthReport);
            let _ = row!(GrowthState);
            let _ = row!(Cell<GrowthState>);
            let _ = row!(GrowthSelectionGuard);
            let _ = row!(GrowthReserveGuard);
            let _ = row!(Allocator);
            let _ = row!(ReserveEvent);
            let _ = row!(Vec<u64>);
            let _ = row!(DropSentinel<'_>);
            let _ = row!([Cell<u8>; 4]);
            let _ = row!([u64; 4]);
            let _ = row!(
                [(
                    Operation,
                    Option<usize>,
                    Layout,
                    Option<usize>,
                    GrowthReason
                ); 10]
            );
            let _ = row!([(bool, usize, usize, usize, usize, GrowthReason); 5]);
            let banks = [
                row!(GrowthStateCarriers),
                row!(GrowthReserveCarriers<'_, u64>),
                row!(GrowthGlobalCarriers),
                row!(GrowthDropCarriers),
                row!(StateCarriers),
                row!(ExactReserveCarriers),
            ];
            let fixed = banks
                .into_iter()
                .try_fold(0usize, usize::checked_add)
                .unwrap();
            assert_eq!(fixed, fixed_carriers_bytes::<u64>());
            let action = full_action(target(2));
            let selection = selection_carriers_bytes(&action);
            fn driver_bytes<F, R>(_: &F) -> usize
            where
                F: for<'a> FnOnce(&'a mut Allocator) -> R,
            {
                size_of::<DriverCarriers<'_, F, R>>()
            }
            let driver = driver_bytes(&action);
            let total = fixed
                .checked_add(selection)
                .unwrap()
                .checked_add(driver)
                .unwrap();
            println!("GROWTH_BANK_SUM fixed={fixed} selection={selection} driver={driver} conservative_named_total={total}");
            println!("GROWTH_HEAP observer_added=0 trace_q2_requested={} old_u64_payload={} null_new_payload=0 sentinel_old_payload={}",
                2usize.checked_mul(size_of::<ReserveEvent>()).unwrap(),4*size_of::<u64>(),4*size_of::<DropSentinel<'_>>());
            println!("GROWTH_PHASE selected=driver+selection+fresh_tls+growth_tls; armed=selected+gate+global; awaiting_drop=selected+gate_completion_or_drop; terminal=selected_without_binding; banks_are_conservative_transports_not_universal_stack_or_RSS");
            assert!(is_idle().unwrap());
            assert!(STATE.with(|state| state.get() == State::Idle));
        }
    }
}
