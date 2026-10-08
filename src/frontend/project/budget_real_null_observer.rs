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
            let ordinary = unsafe { alloc(ordinary_layout) };
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
