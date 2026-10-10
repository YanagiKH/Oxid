//! Proposed observation only: never selects or injects allocator failure.
//! Trace setup happens before the native call; record never grows the trace.
use std::cell::RefCell;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Site {
    Callers, Remaining, CallerEdges, CallReady, Bounds, Incoming, CfgReady,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Event {
    pub site: Site,
    // usize::MAX identifies whole-program carriers or an initial observation.
    // Otherwise these are immutable witness IDs, never raw addresses.
    pub owner: usize,
    pub actor: usize,
    pub len_before: usize,
    pub capacity_before: usize,
    pub len_after: usize,
    pub capacity_after: usize,
    pub element_bytes: usize,
}
#[derive(Debug)]
pub(super) struct Trace {
    pub rows: Vec<Event>,
    pub limit: usize,
    pub overflow: bool,
    pub requested_bytes: usize,
    pub actual_capacity: usize,
    pub actual_bytes: usize,
    pub observer_storage_cap: usize,
}
thread_local! {
    static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) };
}
struct Reset;

// These are named Rust carrier sizes, not the hidden TLS wrapper or stack peak.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(super) fn layout_roles() -> [usize; 6] {
    [size_of::<Trace>(), size_of::<Option<Trace>>(), size_of::<RefCell<Option<Trace>>>(),
     size_of::<std::cell::Ref<'_, Option<Trace>>>(), size_of::<std::cell::RefMut<'_, Option<Trace>>>(),
     size_of::<Reset>()]
}

impl Drop for Reset {
    fn drop(&mut self) {
        TRACE.with(|slot| { slot.borrow_mut().take(); });
    }
}

// Separate auxiliary payload ceiling; never charged as native graph storage.
const MAX_OBSERVER_STORAGE_BYTES: usize = 8 * 1024 * 1024;
fn admit_observer_storage(capacity: usize, observer_storage_cap: usize) -> Result<usize, &'static str> {
    let bytes = capacity.checked_mul(size_of::<Event>())
        .ok_or("graph observation storage overflow")?;
    if bytes > observer_storage_cap {
        return Err("graph observation storage byte cap");
    }
    Ok(bytes)
}

// The requested payload is admitted before allocation; returned capacity is
// admitted again before installing the session or invoking the native action.
// Over-return is allowed within the separate fixed observer payload cap.
// A row shortage invalidates evidence, never native output.
#[allow(dead_code)] // Reviewed fixture driver is a separate next step.
pub(super) fn observe<R>(
    rows: usize,
    observer_storage_cap: usize,
    action: impl FnOnce() -> R,
) -> Result<(R, Trace), &'static str> {
    if rows > 65_536 {
        return Err("graph observation row bound");
    }
    if observer_storage_cap > MAX_OBSERVER_STORAGE_BYTES {
        return Err("graph observation storage cap bound");
    }
    let requested_bytes = admit_observer_storage(rows, observer_storage_cap)?;
    if TRACE.with(|slot| slot.borrow().is_some()) {
        return Err("nested graph observation");
    }
    let mut storage = Vec::new();
    storage.try_reserve_exact(rows).map_err(|_| "graph observation allocation")?;
    let actual_capacity = storage.capacity();
    let actual_bytes = admit_observer_storage(actual_capacity, observer_storage_cap)?;
    TRACE.with(|slot| *slot.borrow_mut() = Some(Trace {
        rows: storage, limit: rows, overflow: false,
        requested_bytes, actual_capacity, actual_bytes, observer_storage_cap,
    }));
    let _reset = Reset;
    let result = action();
    let trace = TRACE.with(|slot| slot.borrow_mut().take().expect("active graph observer"));
    Ok((result, trace))
}

pub(super) fn record<T>(site: Site, owner: usize, actor: usize, before: (usize, usize), vector: &Vec<T>) {
    TRACE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(trace) = slot.as_mut() else { return; };
        if trace.rows.len() >= trace.limit || trace.rows.len() >= trace.rows.capacity() {
            trace.overflow = true;
            return;
        }
        trace.rows.push(Event {
            site, owner, actor,
            len_before: before.0, capacity_before: before.1,
            len_after: vector.len(), capacity_after: vector.capacity(),
            element_bytes: size_of::<T>(),
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn graph_observer_zero_and_one_row_never_grow_trace() {
        for limit in [0, 1] {
            let ((), trace) = observe(limit, MAX_OBSERVER_STORAGE_BYTES, || {
                let values = vec![7usize];
                record(Site::Remaining, usize::MAX, usize::MAX, (0, 0), &values);
                record(Site::Remaining, usize::MAX, usize::MAX, (1, values.capacity()), &values);
            }).unwrap();
            assert_eq!(trace.rows.len(), limit);
            assert!(trace.overflow);
            assert_eq!(trace.requested_bytes, limit * size_of::<Event>());
            assert_eq!(trace.actual_capacity, trace.rows.capacity());
            assert_eq!(trace.actual_bytes, trace.actual_capacity * size_of::<Event>());
            assert!(trace.actual_bytes <= trace.observer_storage_cap);
        }
    }
    #[test]
    fn graph_observer_nested_scope_is_refused_and_outer_scope_survives() {
        let (nested, trace) = observe(1, MAX_OBSERVER_STORAGE_BYTES, || observe(0, 0, || panic!("nested action ran"))).unwrap();
        assert_eq!(nested.unwrap_err(), "nested graph observation");
        assert!(trace.rows.is_empty());
        assert!(!trace.overflow);
        assert!(observe(0, 0, || ()).is_ok());
    }
}

#[cfg(test)]
mod storage_admission_tests {
    use super::*;
    #[test]
    fn graph_observer_overreturn_is_admitted_by_bytes_not_capacity_equality() {
        // Pure returned-capacity facts: this does not claim that a real allocator
        // over-returned, or force a particular allocator's rounding behavior.
        let requested_rows = 2;
        let returned_capacity = requested_rows + 1;
        let cap = 4 * size_of::<Event>();
        assert_eq!(admit_observer_storage(requested_rows, cap), Ok(2 * size_of::<Event>()));
        assert_eq!(admit_observer_storage(returned_capacity, cap), Ok(3 * size_of::<Event>()));
        assert_eq!(admit_observer_storage(4, cap), Ok(cap));
        assert_eq!(admit_observer_storage(5, cap), Err("graph observation storage byte cap"));
        assert_eq!(admit_observer_storage(usize::MAX, usize::MAX), Err("graph observation storage overflow"));
    }
    #[test]
    fn graph_observer_preallocation_refusal_never_runs_action_or_installs_session() {
        let calls = std::cell::Cell::new(0);
        let result = observe(1, 0, || calls.set(calls.get() + 1));
        assert_eq!(result.unwrap_err(), "graph observation storage byte cap");
        assert_eq!(calls.get(), 0);
        assert!(TRACE.with(|slot| slot.borrow().is_none()));
        assert!(observe(0, 0, || ()).is_ok());
    }
}
