//! Bounded, fallible test observations; never ownership authority or a consumer.
use super::*;

pub(super) const MAX_EVENTS: usize = 16_384;
pub(super) const MAX_SNAPSHOTS: usize = 64;
pub(super) const MAX_PAYLOAD_BYTES: usize = MAX_SNAPSHOTS * 4096;

#[derive(Clone, Copy, Debug, Default)]
pub(in super::super) struct ObservationControl {
    pub poison_destinations: bool,
    pub fault: Option<FaultInjection>,
    pub allocation_failure: Option<ObservationAllocationSite>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum StorageObservationKind {
    Construction,
    Transfer,
    Incoming,
    IndexWrite,
    Return,
    Failure,
}
#[derive(Debug, PartialEq, Eq)]
pub(in super::super) struct StorageSnapshot {
    pub kind: StorageObservationKind,
    /// Construction/Transfer: destination identity/state before the write;
    /// bytes below are after the copy. Incoming: intended installed identity.
    /// IndexWrite/Return/Failure: current identity and state at observation.
    pub key: OwnerKey,
    pub state: u64,
    pub bytes: Vec<u8>,
    pub guards_before: [Option<u8>; 2],
    pub guards_after: [Option<u8>; 2],
    pub poisoned: bool,
}
#[derive(Debug)]
pub(in super::super) struct ReferenceObservation {
    pub result: Result<Scalar>,
    pub events: Vec<Event>,
    pub storage: Vec<StorageSnapshot>,
    pub remaining_fuel: usize,
    pub truncated: bool,
    pub fault_applied: bool,
    pub allocation_fault_applied: bool,
}
#[derive(Default)]
pub(super) struct Observer {
    pub(super) fault_attempted: bool,
    pub enabled: bool,
    pub control: ObservationControl,
    pub storage: Vec<StorageSnapshot>,
    pub payload_bytes: usize,
    pub truncated: bool,
    pub fault_applied: bool,
    pub allocation_fault_applied: bool,
}
pub(super) struct Before {
    kind: StorageObservationKind,
    key: OwnerKey,
    state: u64,
    extent: std::ops::Range<usize>,
    guards: [Option<u8>; 2],
    poisoned: bool,
}
fn guards(bytes: &[u8], extent: &std::ops::Range<usize>) -> [Option<u8>; 2] {
    [
        extent
            .start
            .checked_sub(1)
            .and_then(|i| bytes.get(i).copied()),
        bytes.get(extent.end).copied(),
    ]
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum ObservationAllocationSite {
    Event,
    StorageHeader,
    Payload,
}
impl Observer {
    pub(super) fn reject_allocation(&mut self, site: ObservationAllocationSite) -> bool {
        if self.control.allocation_failure == Some(site) && !self.allocation_fault_applied {
            self.allocation_fault_applied = true;
            true
        } else {
            false
        }
    }
    pub fn collecting(&self) -> bool {
        self.enabled && !self.truncated
    }
    fn admits(&mut self, payload: usize) -> bool {
        if !self.collecting() {
            return false;
        }
        let admitted = self
            .storage
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(size_of::<StorageSnapshot>()))
            .is_some_and(|n| n <= MAX_SNAPSHOTS * 128)
            && self.storage.len() < MAX_SNAPSHOTS
            && payload <= 4096
            && self
                .payload_bytes
                .checked_add(payload)
                .is_some_and(|n| n <= MAX_PAYLOAD_BYTES);
        if !admitted {
            self.truncated = true;
        }
        admitted
    }
    pub fn begin(
        &mut self,
        bytes: &mut [u8],
        extent: std::ops::Range<usize>,
        key: OwnerKey,
        state: u64,
        kind: StorageObservationKind,
        destination: bool,
    ) -> Option<Before> {
        let Some(size) = extent.end.checked_sub(extent.start) else {
            self.truncated = self.enabled;
            return None;
        };
        if !self.admits(size) {
            return None;
        }
        let before = guards(bytes, &extent);
        let Some(payload) = bytes.get_mut(extent.clone()) else {
            self.truncated = true;
            return None;
        };
        let poisoned = destination && self.control.poison_destinations;
        if poisoned {
            payload.fill(0xa5);
        }
        Some(Before {
            kind,
            key,
            state,
            extent,
            guards: before,
            poisoned,
        })
    }
    pub fn end(&mut self, bytes: &[u8], before: Option<Before>) {
        let Some(before) = before else {
            return;
        };
        if !self.admits(before.extent.len()) {
            return;
        }
        let Some(payload) = bytes.get(before.extent.clone()) else {
            self.truncated = true;
            return;
        };
        // Observer allocations do not pass through production plan/frame failpoints.
        if self.reject_allocation(ObservationAllocationSite::StorageHeader)
            || self.storage.try_reserve_exact(1).is_err()
        {
            self.truncated = true;
            return;
        }
        let mut copy = Vec::new();
        if self.reject_allocation(ObservationAllocationSite::Payload)
            || copy.try_reserve_exact(payload.len()).is_err()
        {
            self.truncated = true;
            return;
        }
        copy.extend_from_slice(payload);
        self.payload_bytes += payload.len(); // admits checked this addition before reservation
        self.storage.push(StorageSnapshot {
            kind: before.kind,
            key: before.key,
            state: before.state,
            bytes: copy,
            guards_before: before.guards,
            guards_after: guards(bytes, &before.extent),
            poisoned: before.poisoned,
        });
    }
}
impl Machine<'_, '_> {
    pub(super) fn record_event(&mut self, event: Event) {
        if !self.observer.enabled {
            self.events.push(event);
            return;
        }
        if !self.observer.collecting() {
            return;
        }
        if self
            .events
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(size_of::<Event>()))
            .is_none_or(|n| n > MAX_EVENTS * 80)
            || self.events.len() >= MAX_EVENTS
            || self
                .observer
                .reject_allocation(ObservationAllocationSite::Event)
            || self.events.try_reserve_exact(1).is_err()
        {
            self.observer.truncated = true;
            return;
        }
        self.events.push(event);
    }
    pub(super) fn observe_begin(
        &mut self,
        key: OwnerKey,
        kind: StorageObservationKind,
        destination: bool,
        span: Span,
    ) -> Option<Before> {
        if !self.observer.collecting() {
            return None;
        }
        let extent = match self.owner_extent(key, span) {
            Ok(extent) => extent,
            Err(_) => {
                self.observer.truncated = true;
                return None;
            }
        };
        let frame = &mut self.frames[key.frame as usize];
        let state = frame.owners[key.owner as usize].state;
        self.observer
            .begin(&mut frame.payload, extent, key, state, kind, destination)
    }
    pub(super) fn observe_end(&mut self, before: Option<Before>) {
        let Some(before) = before else {
            return;
        };
        let frame = before.key.frame as usize;
        self.observer.end(&self.frames[frame].payload, Some(before));
    }
    pub(super) fn observe_frame(&mut self, frame: usize, kind: StorageObservationKind, span: Span) {
        for owner in 0..self.frames[frame].owners.len() {
            if !self.observer.collecting() {
                break;
            }
            let key = self.raw_key(frame, OwnerPlaceId(owner));
            let before = self.observe_begin(key, kind, false, span);
            self.observe_end(before);
        }
    }
}

#[test]
fn unit2c_observation_bounds_and_layout() {
    assert_eq!(size_of::<Event>(), 72);
    assert!(size_of::<StorageSnapshot>() <= 128);
    assert_eq!(size_of::<Frame>(), 272);
    println!(
        "Unit2C observer sizes: Event={} StorageSnapshot={} Frame={}",
        size_of::<Event>(),
        size_of::<StorageSnapshot>(),
        size_of::<Frame>()
    );
    let mut observer = Observer {
        enabled: true,
        ..Observer::default()
    };
    let key = OwnerKey::default();
    let mut payload = vec![0x51; 4096];
    for _ in 0..MAX_SNAPSHOTS {
        let before = observer.begin(
            &mut payload,
            0..4096,
            key,
            AVAILABLE,
            StorageObservationKind::Failure,
            false,
        );
        observer.end(&payload, before);
    }
    assert_eq!(observer.payload_bytes, MAX_PAYLOAD_BYTES);
    assert!(!observer.truncated);
    assert!(observer
        .begin(
            &mut payload,
            0..4096,
            key,
            AVAILABLE,
            StorageObservationKind::Failure,
            true
        )
        .is_none());
    assert!(observer.truncated);
    assert!(payload.iter().all(|byte| *byte == 0x51));
    assert_eq!(observer.storage.len(), MAX_SNAPSHOTS);
}

#[derive(Clone, Copy, Debug)]
pub(in super::super) struct FaultInjection {
    pub at: Span,
    pub kind: FaultKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum FaultKind {
    ConstructorLastScalarType,
    StaleOwnerActivation,
    StaleOwnerGeneration,
    StaleLoanActivation,
    StaleLoanInstance,
    SuspendedReferencePermission,
    LoanRootMismatch,
    ArrayReferenceDifferentType,
    IncomingExclusiveAlias,
    IncomingReferenceDifferentType,
}
impl Machine<'_, '_> {
    fn pending_fault(&mut self, span: Span) -> Option<FaultKind> {
        if !self.observer.collecting() || self.observer.fault_attempted {
            return None;
        }
        let fault = self
            .observer
            .control
            .fault
            .filter(|fault| fault.at == span)?;
        // Even an inapplicable first matching charged operation consumes the selector.
        self.observer.fault_attempted = true;
        Some(fault.kind)
    }
    fn other_root(&self, root: OwnerKey, different_type: bool, span: Span) -> Option<OwnerKey> {
        let frame = usize::try_from(root.frame).ok()?;
        let aggregate = self.aggregate(root, span).ok()?;
        for owner in 0..self.frames.get(frame)?.owners.len() {
            let key = self.owner_key(frame, OwnerPlaceId(owner), span).ok();
            if let Some(key) = key {
                if key != root && (!different_type || self.aggregate(key, span).ok()? != aggregate)
                {
                    return Some(key);
                }
            }
        }
        None
    }
    pub(super) fn inject_statement(
        &mut self,
        frame: usize,
        instruction: &OwnedInstruction,
        span: Span,
    ) {
        let Some(kind) = self.pending_fault(span) else {
            return;
        };
        if kind == FaultKind::ConstructorLastScalarType {
            let OwnedInstruction::ConstructArray { elements, .. } = instruction else {
                return;
            };
            let Some(last) = elements.last() else {
                return;
            };
            let Some(Some(value)) = self.frames[frame].slots.get_mut(last.local.0) else {
                return;
            };
            *value = if value.ty() == hir::Ty::Bool {
                Scalar::I32(0)
            } else {
                Scalar::Bool(false)
            };
            self.observer.fault_applied = true;
            return;
        }
        let base = match instruction {
            OwnedInstruction::ReadIndex { base, .. }
            | OwnedInstruction::WriteIndex { base, .. }
            | OwnedInstruction::ArrayLength { base, .. } => *base,
            _ => return,
        };
        let AccessBase::Parameter(reference) = base else {
            return;
        };
        let Some(mut handle) = self.frames[frame].references.get(reference.0).copied() else {
            return;
        };
        let applied = match kind {
            FaultKind::StaleOwnerActivation => handle
                .root
                .activation
                .checked_add(1)
                .map(|n| handle.root.activation = n),
            FaultKind::StaleOwnerGeneration => handle
                .root
                .generation
                .checked_add(1)
                .map(|n| handle.root.generation = n),
            FaultKind::StaleLoanActivation => handle
                .permission
                .activation
                .checked_add(1)
                .map(|n| handle.permission.activation = n),
            FaultKind::StaleLoanInstance => handle
                .permission
                .instance
                .checked_add(1)
                .map(|n| handle.permission.instance = n),
            FaultKind::LoanRootMismatch | FaultKind::ArrayReferenceDifferentType => {
                let Some(root) = self.other_root(
                    handle.root,
                    kind == FaultKind::ArrayReferenceDifferentType,
                    span,
                ) else {
                    return;
                };
                handle.root = root;
                if kind == FaultKind::ArrayReferenceDifferentType {
                    self.frames[handle.permission.frame as usize].loans
                        [handle.permission.loan as usize]
                        .root = root;
                }
                Some(())
            }
            FaultKind::SuspendedReferencePermission => {
                let loan = &mut self.frames[handle.permission.frame as usize].loans
                    [handle.permission.loan as usize];
                if loan.exclusive_children != 0 {
                    return;
                }
                loan.exclusive_children = 1;
                Some(())
            }
            _ => None,
        };
        if applied.is_some() {
            self.frames[frame].references[reference.0] = handle;
            self.observer.fault_applied = true;
        }
    }
    pub(super) fn inject_invoke(&mut self, frame: usize, call: CallSiteId, span: Span) {
        let Some(kind) = self.pending_fault(span) else {
            return;
        };
        let function = self.frames[frame].function;
        let loans = self.plan.function(function).borrowed_loans(call);
        let Some(&first) = loans.first() else {
            return;
        };
        let Ok(handle) = self.handle(frame, first, span) else {
            return;
        };
        match kind {
            FaultKind::IncomingExclusiveAlias => {
                let Some(&second) = loans.get(1) else {
                    return;
                };
                let Ok(other) = self.handle(frame, second, span) else {
                    return;
                };
                if self.loan_kind(handle.permission, span).ok() != Some(BorrowKind::Exclusive)
                    && self.loan_kind(other.permission, span).ok() != Some(BorrowKind::Exclusive)
                {
                    return;
                }
                self.frames[frame].loans[second.0].root = handle.root;
            }
            FaultKind::IncomingReferenceDifferentType => {
                let Some(root) = self.other_root(handle.root, true, span) else {
                    return;
                };
                self.frames[frame].loans[first.0].root = root;
            }
            _ => return,
        }
        self.observer.fault_applied = true;
    }
}

#[path = "reviewer_array_observer_tests.rs"]
mod reviewer;
