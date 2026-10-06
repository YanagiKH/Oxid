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
    /// True only after all request and fresh-vector checks admitted the gate.
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

/// Only vector_exact and this module's internal calibrations can call the gate.
/// All original checked request arithmetic precedes it. No owner, Vec or action
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
// These first banks are deliberately partial: anonymous TLS callbacks and the
// full eligibility/mismatch/thread calibration transports await the next audit.
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
/// manufacturing an Allocator/owner. More observer transport rows remain pending.
pub(in crate::frontend) fn selection_carriers_bytes<F, R>(_: &F) -> usize
where
    F: for<'a> FnOnce(&'a mut Allocator) -> R,
{
    std::mem::size_of::<SelectionCarriers<'_, F, R>>()
        + std::mem::size_of::<SelectionSizingCarriers<'_, F>>()
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
                let offsets = [$(std::mem::offset_of!($model, $field)),+];
                for offset in offsets { assert!(offset <= std::mem::size_of::<$model>()); }
                offsets.len()
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
        println!(
            "REAL_NULL_EARLY_LAYOUT target={} report={} state={} state_bank={} reserve_bank={}",
            std::mem::size_of::<Target>(),
            std::mem::size_of::<Report>(),
            std::mem::size_of::<State>(),
            std::mem::size_of::<StateCarriers>(),
            std::mem::size_of::<ExactReserveCarriers>()
        );
    }
}
