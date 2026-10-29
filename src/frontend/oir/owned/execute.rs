//! Iterative reference execution of the sealed owned witness. No raw entry point.
use super::{
    plan::{self, ExecutionPlan},
    storage::*,
    verified::{
        PlanWitness, ProjectionAccess, ProjectionPermission, ProjectionRequest,
        VerifiedOwnedProgram,
    },
    *,
};
use crate::frontend::build_config::{ExecutionPolicy, HOSTED_FUEL};
use crate::frontend::options::EntryPolicy;
use std::mem::size_of;

#[path = "execute_floats.rs"]
mod floats;
#[path = "execute_values.rs"]
mod values;
use values::StoredValue;
#[path = "execute_bitwise.rs"]
mod bitwise;
#[path = "execute_integer128.rs"]
mod integer128;
#[path = "execute_integers.rs"]
mod integers;

#[path = "execute_experimental.rs"]
mod experimental;
#[path = "execute_owned_vector.rs"]
mod owned_vector;
#[path = "execute_vector.rs"]
mod vector;
pub(super) fn binary_delivery_control_bytes() -> usize {
    experimental::binary_delivery_control_bytes()
}
#[path = "execute_composition.rs"]
mod composition;
#[path = "execute_resources.rs"]
mod resources;

const DEAD: u64 = 0;
const UNINITIALIZED: u64 = 1;
const AVAILABLE: u64 = 2;
const MOVED: u64 = 3;
const CLOSED: u64 = 0;
const PREPARING: u64 = 1;
const IN_FLIGHT: u64 = 2;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum OwnedRunFailure {
    Scalar(RunFailure),
    EntryResult(Span),
    Bounds(Span),
    InputHost(Span),
    OutputHost(Span),
    OutputEntry(Span),
    BinaryEntry(Span),
    BinaryHost(Span),
    ProcessHost,
    ProcessSetup,
    ProcessEntry(Span),
    ProcessResult(Span),
    Resource(plan::AdmissionFailure),
    Invariant(&'static str, Option<Span>),
}
impl From<RunFailure> for OwnedRunFailure {
    fn from(value: RunFailure) -> Self {
        Self::Scalar(value)
    }
}
impl From<plan::AdmissionFailure> for OwnedRunFailure {
    fn from(value: plan::AdmissionFailure) -> Self {
        Self::Resource(value)
    }
}
impl OwnedRunFailure {
    fn at_runtime(mut self, span: Span) -> Self {
        if let Self::Resource(error) = &mut self {
            if error.span.is_none() {
                error.span = Some(span);
            }
        }
        self
    }
    pub fn diagnostic(&self, sources: &SourceMap) -> Box<Diagnostic> {
        match self {
            Self::BinaryEntry(span) => Diagnostic::new(
                "E0609",
                "oir-owned-run",
                "binary I/O requires process entry mode",
                Some(*span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::BinaryHost(span) => Diagnostic::new(
                "E0608",
                "oir-owned-run",
                "binary I/O requires Linux or macOS on x86_64 or ARM64",
                Some(*span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::Scalar(e) => e.diagnostic(sources),
            Self::EntryResult(s) => Diagnostic::new(
                "E0600",
                "oir-run",
                "typed-preview main must return bool, i32 or ()",
                Some(*s),
            ),
            Self::Bounds(span) => Diagnostic::new(
                "E0606",
                "oir-owned-run",
                "array index out of bounds",
                Some(*span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::InputHost(span) => Diagnostic::new(
                "E0608",
                "oir-owned-run",
                "bounded stdin execution requires Linux or macOS on x86_64 or ARM64",
                Some(*span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::OutputHost(span) => Diagnostic::new(
                "E0608",
                "oir-owned-run",
                "bounded stdout execution requires Linux or macOS on x86_64 or ARM64",
                Some(*span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::OutputEntry(span) => Diagnostic::new(
                "E0609",
                "oir-owned-run",
                "bounded stdout execution requires process entry mode",
                Some(*span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::ProcessHost => Diagnostic::new(
                "E0608",
                "oir-owned-run",
                "process execution requires Linux or macOS on x86_64 or ARM64",
                None,
            ),
            // Process drivers must intercept this and exit 74 silently. This
            // fallback identifies a violated internal reporting contract; it
            // must never itself be sent to a potentially unsafe descriptor.
            Self::ProcessSetup => Diagnostic::new(
                "E0500",
                "oir-owned-run",
                "internal compiler error: process setup failure requires silent exit 74",
                None,
            ),
            Self::ProcessEntry(span) => Diagnostic::new(
                "E0600",
                "oir-run",
                "process main must return i32",
                Some(*span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::ProcessResult(span) => Diagnostic::new(
                "E0600",
                "oir-run",
                "process main must return a status in 0..255",
                Some(*span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::Resource(e) => Diagnostic::new(
                "E0605",
                "oir-owned-run",
                format!("owned execution resource limit: {}", e.name),
                e.span.filter(|s| sources.is_valid_span(*s)),
            ),
            Self::Invariant(name, span) => Diagnostic::new(
                "E0500",
                "oir-owned-run",
                format!("internal compiler error: owned execution invariant {name}"),
                span.filter(|s| sources.is_valid_span(*s)),
            ),
        }
    }
}
type Result<T> = std::result::Result<T, OwnedRunFailure>;
fn bad(name: &'static str, span: Span) -> OwnedRunFailure {
    OwnedRunFailure::Invariant(name, Some(span))
}
fn index(value: u64, span: Span) -> Result<usize> {
    usize::try_from(value).map_err(|_| bad("identity index overflow", span))
}
fn epoch(value: u64, span: Span) -> Result<u64> {
    value
        .checked_add(1)
        .filter(|n| *n != 0)
        .ok_or_else(|| bad("identity epoch overflow", span))
}
#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub fuel: usize,
    pub frames: usize,
    pub slots: usize,
    pub cells: usize,
    pub bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            fuel: plan::MAX_FUEL,
            frames: plan::MAX_FRAMES,
            slots: plan::MAX_SCALAR_SLOTS,
            cells: plan::MAX_EXPANDED_CELLS,
            bytes: plan::MAX_REFERENCE_BYTES,
        }
    }
}
impl Limits {
    fn bounded(self) -> Self {
        let d = Self::default();
        Self {
            fuel: self.fuel.min(d.fuel),
            frames: self.frames.min(d.frames),
            slots: self.slots.min(d.slots),
            cells: self.cells.min(d.cells),
            bytes: self.bytes.min(d.bytes),
        }
    }
}
#[derive(Clone, Copy)]
struct Resume {
    call: CallSiteId,
    continuation: BlockId,
    origin: Span,
}
type Frame = ValueFrame<Scalar>;
struct ValueFrame<V> {
    function: hir::DefId,
    activation: u64,
    block: BlockId,
    next: usize,
    predecessor: Option<BlockId>,
    merge_pending: bool,
    slots: Vec<Option<V>>,
    snapshots: Vec<Option<V>>,
    payload: Vec<u8>,
    owners: Vec<OwnerRuntime>,
    references: Vec<ReferenceHandle>,
    loans: Vec<LoanRuntime>,
    calls: Vec<CallRuntime>,
    return_to: Option<Resume>,
}
impl<V: StoredValue> ValueFrame<V> {
    fn allocate<W: PlanWitness>(
        plan: &ExecutionPlan<'_, W>,
        function: hir::DefId,
        activation: u64,
        return_to: Option<Resume>,
    ) -> Result<Self> {
        let f = &plan.witness().functions()[function.0];
        if (plan.uses_wide_scalars() || plan.uses_quad_scalars()) != V::WIDE
            || plan.uses_quad_scalars() != V::QUAD
        {
            return Err(bad("scalar storage representation", f.span));
        }
        let u = plan.function(function).usage();
        Ok(Self {
            function,
            activation,
            block: f.entry,
            next: 0,
            predecessor: None,
            merge_pending: true,
            slots: plan::filled(u.scalar_slots, None)?,
            snapshots: plan::filled(u.arguments, None)?,
            payload: plan::filled(u.payload_bytes, 0)?,
            owners: plan::filled(u.owners, OwnerRuntime::default())?,
            references: plan::filled(u.references, ReferenceHandle::default())?,
            loans: plan::filled(u.loans, LoanRuntime::default())?,
            calls: plan::filled(u.calls, CallRuntime::default())?,
            return_to,
        })
    }
}
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Event<V = Scalar> {
    Charge(Span, usize),
    Enter(hir::DefId, u64),
    WriteField(OwnerKey, FieldId, V),
    ReadIndex(OwnerKey, usize, V),
    WriteIndex(OwnerKey, usize, V),
    ArrayLength(OwnerKey, usize),
    Transfer(OwnerKey, OwnerKey),
    Acquire(LoanKey, OwnerKey, BorrowKind),
    Release(LoanKey),
    Return(hir::DefId),
    EnumTagRead(OwnerKey, u32),
    EnumPayloadRead(OwnerKey, usize),
    EnumBind(OwnerKey, Option<LocalId>, Option<V>),
}
/// A fixed-size active-value snapshot, never the inactive union or its padding.
type EnumValue = StoredEnumValue<Scalar>;
#[derive(Clone, Copy)]
struct StoredEnumValue<V> {
    tag: u32,
    payload: Option<V>,
    payload_offset: u8,
}
#[derive(Clone, Copy)]
enum Access {
    Read,
    Write,
    Consume,
    Borrow(BorrowKind),
}
type Machine<'p, 'w> = ValueMachine<'p, 'w, Scalar>;
struct ValueMachine<'p, 'w, V, W: PlanWitness = VerifiedOwnedProgram> {
    plan: &'p ExecutionPlan<'w, W>,
    frames: Vec<ValueFrame<V>>,
    limits: Limits,
    fuel: usize,
    next_activation: u64,
    live_slots: usize,
    live_cells: usize,
    live_bytes: usize,
    header_bytes: usize,
    heap: byte_buffer::Heap,
    #[cfg(test)]
    events: Vec<Event<V>>,
    #[cfg(test)]
    observer: array_observe::Observer,
}
impl<'p, 'w, V: StoredValue, W: PlanWitness> ValueMachine<'p, 'w, V, W> {
    fn function(&self, id: hir::DefId) -> &'w RawOwnedFunction {
        &self.plan.witness().functions()[id.0]
    }
    fn charge(&mut self, cost: usize, span: Span) -> Result<()> {
        if self.fuel != HOSTED_FUEL {
            self.fuel = self.fuel.checked_sub(cost).ok_or(RunFailure::Fuel(span))?;
        }
        #[cfg(test)]
        self.record_event(Event::Charge(span, cost));
        Ok(())
    }
    fn activation_preflight(&mut self, id: hir::DefId, cost: usize, span: Span) -> Result<()> {
        self.charge(cost, span)?;
        let u = self.plan.function(id).usage();
        if plan::add(self.frames.len(), 1)? > self.limits.frames {
            return Err(RunFailure::Frames(span).into());
        }
        if plan::add(self.live_slots, u.scalar_slots)? > self.limits.slots {
            return Err(RunFailure::Slots(span).into());
        }
        if plan::add(self.live_cells, u.expanded_cells)? > self.limits.cells {
            return Err(plan::AdmissionFailure::new("live expanded cells", Some(span)).into());
        }
        let bytes = plan::add(
            plan::add(self.header_bytes, size_of::<V>())?,
            plan::add(self.live_bytes, u.reference_bytes)?,
        )?;
        if bytes > self.limits.bytes {
            return Err(plan::AdmissionFailure::new("live requested bytes", Some(span)).into());
        }
        epoch(self.next_activation, span)?;
        Ok(())
    }
    fn install(&mut self, frame: ValueFrame<V>) {
        let u = self.plan.function(frame.function).usage();
        self.live_slots += u.scalar_slots;
        self.live_cells += u.expanded_cells;
        self.live_bytes += u.reference_bytes;
        self.next_activation += 1; // preflight checked this epoch before allocation/transfer
        #[cfg(test)]
        self.record_event(Event::Enter(frame.function, frame.activation));
        self.frames.push(frame);
    }
    fn read(&self, frame: usize, operand: Operand) -> Result<V> {
        self.frames
            .get(frame)
            .and_then(|f| f.slots.get(operand.local.0))
            .and_then(|v| *v)
            .ok_or_else(|| bad("uninitialized scalar", operand.span))
    }
    fn write(
        &mut self,
        frame: usize,
        destination: LocalId,
        value: impl Into<V>,
        span: Span,
    ) -> Result<()> {
        let value = value.into();
        let f = self.function(self.frames[frame].function);
        let d = f
            .locals
            .get(destination.0)
            .ok_or_else(|| bad("scalar identity", span))?;
        if d.ty != value.ty() {
            return Err(bad("scalar type", span));
        }
        let slot = self.frames[frame]
            .slots
            .get_mut(destination.0)
            .ok_or_else(|| bad("scalar storage", span))?;
        if d.kind == LocalKind::Parameter && slot.is_some() {
            return Err(bad("parameter reassignment", span));
        }
        *slot = Some(value);
        Ok(())
    }
    fn owner(&self, key: OwnerKey, span: Span) -> Result<&OwnerRuntime> {
        let frame = self
            .frames
            .get(index(key.frame, span)?)
            .filter(|f| f.activation == key.activation && key.activation != 0)
            .ok_or_else(|| bad("stale owner activation", span))?;
        self.plan
            .witness()
            .functions()
            .get(frame.function.0)
            .filter(|f| f.id == frame.function)
            .ok_or_else(|| bad("dynamic owner function identity", span))?;
        frame
            .owners
            .get(index(key.owner, span)?)
            .filter(|o| {
                o.generation == key.generation && key.generation != 0 && o.state == AVAILABLE
            })
            .ok_or_else(|| bad("stale owner generation", span))
    }
    fn owner_key(&self, frame: usize, owner: OwnerPlaceId, span: Span) -> Result<OwnerKey> {
        let active = self
            .frames
            .get(frame)
            .ok_or_else(|| bad("owner frame", span))?;
        let state = active
            .owners
            .get(owner.0)
            .ok_or_else(|| bad("owner index", span))?;
        let key = OwnerKey {
            frame: frame as u64,
            activation: active.activation,
            owner: owner.0 as u64,
            generation: state.generation,
        };
        self.owner(key, span)?;
        Ok(key)
    }
    fn loan(&self, key: LoanKey, span: Span) -> Result<&LoanRuntime> {
        let frame = self
            .frames
            .get(index(key.frame, span)?)
            .filter(|f| f.activation == key.activation && key.activation != 0)
            .ok_or_else(|| bad("stale loan activation", span))?;
        self.plan
            .witness()
            .functions()
            .get(frame.function.0)
            .filter(|f| f.id == frame.function)
            .ok_or_else(|| bad("dynamic loan function identity", span))?;
        frame
            .loans
            .get(index(key.loan, span)?)
            .filter(|l| l.instance == key.instance && key.instance != 0 && l.state == 1)
            .ok_or_else(|| bad("stale loan instance", span))
    }
    fn loan_kind(&self, key: LoanKey, span: Span) -> Result<BorrowKind> {
        self.loan(key, span)?;
        Ok(self
            .function(self.frames[key.frame as usize].function)
            .loans[key.loan as usize]
            .kind)
    }
    fn handle(&self, frame: usize, loan: LoanId, span: Span) -> Result<ReferenceHandle> {
        let result = self.reference_from_loan(frame, loan, span)?;
        self.validate_handle(result, Access::Read, span)?;
        Ok(result)
    }
    fn reference_from_loan(
        &self,
        frame: usize,
        loan: LoanId,
        span: Span,
    ) -> Result<ReferenceHandle> {
        let active = self
            .frames
            .get(frame)
            .ok_or_else(|| bad("loan frame", span))?;
        let state = active
            .loans
            .get(loan.0)
            .ok_or_else(|| bad("loan index", span))?;
        let result = ReferenceHandle {
            root: state.root,
            view: state.view,
            permission: LoanKey {
                frame: frame as u64,
                activation: active.activation,
                loan: loan.0 as u64,
                instance: state.instance,
            },
        };
        self.validate_handle_identity(result, span)?;
        Ok(result)
    }
    fn validate_handle(
        &self,
        handle: ReferenceHandle,
        access: Access,
        span: Span,
    ) -> Result<OwnerKey> {
        let l = self.validate_handle_identity(handle, span)?;
        let shared_access = matches!(access, Access::Read | Access::Borrow(BorrowKind::Shared));
        if l.exclusive_children != 0 || (!shared_access && l.shared_children != 0) {
            return Err(bad("suspended parent permission", span));
        }
        if !shared_access && self.loan_kind(handle.permission, span)? != BorrowKind::Exclusive {
            return Err(bad("shared permission upgrade", span));
        }
        Ok(handle.root)
    }
    fn validate_handle_identity(
        &self,
        handle: ReferenceHandle,
        span: Span,
    ) -> Result<&LoanRuntime> {
        self.owner(handle.root, span)?;
        let loan = self.loan(handle.permission, span)?;
        if loan.view != handle.view {
            return Err(bad("loan view mismatch", span));
        }
        if loan.root != handle.root {
            return Err(bad("loan root mismatch", span));
        }
        Ok(loan)
    }
    fn reference_extents_overlap(
        &self,
        left: ReferenceHandle,
        right: ReferenceHandle,
        span: Span,
    ) -> Result<bool> {
        if matches!(
            left.view.aggregate.map(AggregateSlot::aggregate),
            Some(AggregateTy::Vector(_) | AggregateTy::RecordVector(_))
        ) || matches!(
            right.view.aggregate.map(AggregateSlot::aggregate),
            Some(AggregateTy::Vector(_) | AggregateTy::RecordVector(_))
        ) {
            return self.vector_reference_overlap(left, right, span);
        }
        let extent = |handle: ReferenceHandle| -> Result<(u64, u64)> {
            let aggregate = handle
                .view
                .aggregate
                .ok_or_else(|| bad("alias view type", span))?
                .aggregate();
            let size = match aggregate {
                AggregateTy::FixedArray(array) if array.length() == 0 => 0,
                _ => self
                    .plan
                    .witness()
                    .aggregate_layout(aggregate)
                    .map_err(|_| bad("alias view layout", span))?
                    .size() as u64,
            };
            let end = handle
                .view
                .offset
                .checked_add(size)
                .ok_or_else(|| bad("alias view extent", span))?;
            Ok((handle.view.offset, end))
        };
        let (left_start, left_end) = extent(left)?;
        let (right_start, right_end) = extent(right)?;
        Ok(left_start < right_end && right_start < left_end)
    }
    fn base(&self, frame: usize, base: AccessBase, access: Access, span: Span) -> Result<OwnerKey> {
        self.base_with_permission(frame, base, access, span, None)
    }
    fn projected_base(
        &self,
        frame: usize,
        base: AccessBase,
        access: Access,
        span: Span,
        request: ProjectionRequest<'_>,
    ) -> Result<OwnerKey> {
        let current = &self.frames[frame];
        // Only a function with local loans can hold children while preparing
        // another argument. A leaf callee cannot explain a suspended incoming
        // permission and must retain the original immediate rejection.
        if self.function(current.function).loans.is_empty() {
            return self.base(frame, base, access, span);
        }
        let permission = self.plan.witness().projection_permission(
            current.function,
            current.block,
            current.next,
            request,
        );
        self.base_with_permission(frame, base, access, span, permission.as_ref())
    }
    fn base_with_permission(
        &self,
        frame: usize,
        base: AccessBase,
        access: Access,
        span: Span,
        permission: Option<&ProjectionPermission<'_, W>>,
    ) -> Result<OwnerKey> {
        let active = &self.frames[frame];
        let granted = permission.is_some_and(|permission| {
            let requested = match access {
                Access::Read => ProjectionAccess::Read,
                Access::Write => ProjectionAccess::Write,
                Access::Borrow(kind) => ProjectionAccess::Borrow(kind),
                Access::Consume => return false,
            };
            permission.permits(
                self.plan.witness(),
                active.function,
                active.block,
                active.next,
                base,
                requested,
            )
        });
        match base {
            AccessBase::Owner(id) => {
                let key = self.owner_key(frame, id, span)?;
                let state = self.owner(key, span)?;
                if !granted
                    && (state.exclusive_children != 0
                        || (state.shared_children != 0
                            && !matches!(
                                access,
                                Access::Read | Access::Borrow(BorrowKind::Shared)
                            )))
                {
                    return Err(bad("owner has conflicting loan", span));
                }
                if matches!(
                    access,
                    Access::Write | Access::Borrow(BorrowKind::Exclusive)
                ) && self.function(active.function).owners[id.0].kind
                    != (OwnerKind::Local { mutable: true })
                {
                    return Err(bad("immutable owner", span));
                }
                Ok(key)
            }
            AccessBase::Parameter(_) | AccessBase::Loan(_) => {
                let handle = match base {
                    AccessBase::Parameter(id) => *active
                        .references
                        .get(id.0)
                        .ok_or_else(|| bad("reference parameter", span))?,
                    AccessBase::Loan(id) => self.reference_from_loan(frame, id, span)?,
                    AccessBase::Owner(_) => return Err(bad("reference base", span)),
                };
                if !granted {
                    return self.validate_handle(handle, access, span);
                }
                // The current verified field access can coexist with disjoint
                // children. Epochs, views and exclusive authority still apply.
                self.validate_handle_identity(handle, span)?;
                if !matches!(access, Access::Read | Access::Borrow(BorrowKind::Shared))
                    && self.loan_kind(handle.permission, span)? != BorrowKind::Exclusive
                {
                    return Err(bad("shared permission upgrade", span));
                }
                Ok(handle.root)
            }
        }
    }
    fn payload_field(&self, key: OwnerKey, field: FieldId, span: Span) -> Result<(usize, hir::Ty)> {
        let frame = self
            .frames
            .get(index(key.frame, span)?)
            .ok_or_else(|| bad("payload frame", span))?;
        let f = self
            .plan
            .witness()
            .functions()
            .get(frame.function.0)
            .filter(|f| f.id == frame.function)
            .ok_or_else(|| bad("dynamic payload function identity", span))?;
        let owner = f
            .owners
            .get(index(key.owner, span)?)
            .ok_or_else(|| bad("payload owner", span))?;
        let field = self
            .plan
            .witness()
            .declarations()
            .field(record_type(owner.aggregate(), span)?, field)
            .map_err(|_| bad("payload field type", span))?;
        let ValueTy::Scalar(ty) = field.value_ty() else {
            return Err(bad("aggregate field scalar access", span));
        };
        Ok((self.leaf_offset(key, field.offset(), ty, span)?, ty))
    }

    fn accessed_field(
        &self,
        frame: usize,
        base: AccessBase,
        key: OwnerKey,
        field: FieldId,
        span: Span,
    ) -> Result<(usize, hir::Ty)> {
        let view = self.base_view(frame, base, key, span)?;
        let aggregate = view
            .aggregate
            .ok_or_else(|| bad("field view type", span))?
            .aggregate();
        let field = self
            .plan
            .witness()
            .declarations()
            .field(record_type(aggregate, span)?, field)
            .map_err(|_| bad("field view identity", span))?;
        let ValueTy::Scalar(ty) = field.value_ty() else {
            return Err(bad("aggregate field scalar access", span));
        };
        let relative = index(view.offset, span)?
            .checked_add(field.offset())
            .ok_or_else(|| bad("field view offset", span))?;
        Ok((self.leaf_offset(key, relative, ty, span)?, ty))
    }
    fn load_field(
        &self,
        frame: usize,
        base: AccessBase,
        key: OwnerKey,
        field: FieldId,
        span: Span,
    ) -> Result<V> {
        let (offset, ty) = self.accessed_field(frame, base, key, field, span)?;
        V::decode(&self.frames[key.frame as usize].payload, offset, ty, span)
    }
    fn store_field(&mut self, key: OwnerKey, field: FieldId, value: V, span: Span) -> Result<()> {
        let (offset, ty) = self.payload_field(key, field, span)?;
        if ty != value.ty() {
            return Err(bad("field value type", span));
        }
        value.encode(&mut self.frames[key.frame as usize].payload, offset, span)
    }
    fn aggregate(&self, key: OwnerKey, span: Span) -> Result<AggregateTy> {
        let frame = self
            .frames
            .get(index(key.frame, span)?)
            .filter(|frame| frame.activation == key.activation && key.activation != 0)
            .ok_or_else(|| bad("payload frame", span))?;
        let function = self
            .plan
            .witness()
            .functions()
            .get(frame.function.0)
            .filter(|f| f.id == frame.function)
            .ok_or_else(|| bad("dynamic payload function identity", span))?;
        let owner = function
            .owners
            .get(index(key.owner, span)?)
            .ok_or_else(|| bad("payload owner", span))?;
        let aggregate = owner.aggregate();
        self.plan
            .witness()
            .aggregate_layout(aggregate)
            .map_err(|_| bad("payload aggregate type", span))?;
        Ok(aggregate)
    }
    fn array_type(&self, key: OwnerKey, span: Span) -> Result<FixedArrayTy> {
        match self.aggregate(key, span)? {
            AggregateTy::FixedArray(array) => Ok(array),
            AggregateTy::Record(_)
            | AggregateTy::Enum(_)
            | AggregateTy::ByteBuf
            | AggregateTy::File
            | AggregateTy::Vector(_)
            | AggregateTy::RecordVector(_)
            | AggregateTy::OwnedRecordVector(_) => Err(bad("array aggregate type", span)),
        }
    }
    fn whole_view(&self, key: OwnerKey, span: Span) -> Result<BorrowView> {
        Ok(BorrowView {
            offset: 0,
            aggregate: Some(
                AggregateSlot::try_from_aggregate(self.aggregate(key, span)?)
                    .map_err(|_| bad("whole view type", span))?,
            ),
        })
    }
    fn view_extent(&self, key: OwnerKey, view: BorrowView, span: Span) -> Result<AggregateTy> {
        let aggregate = view
            .aggregate
            .ok_or_else(|| bad("missing view type", span))?
            .aggregate();
        let root_type = self.aggregate(key, span)?;
        if matches!(aggregate, AggregateTy::Record(_))
            && !matches!(root_type, AggregateTy::Record(_))
        {
            return Err(bad("view record root type", span));
        }
        if matches!(root_type, AggregateTy::FixedArray(_))
            && (view.offset != 0 || root_type != aggregate)
        {
            return Err(bad("view root type", span));
        }
        if matches!(
            root_type,
            AggregateTy::Vector(_)
                | AggregateTy::RecordVector(_)
                | AggregateTy::OwnedRecordVector(_)
        ) && (view.offset != 0 || root_type != aggregate)
        {
            // Dynamic scalar slices take their checked packed-range path in
            // checked_handle_view. A vector descriptor cannot become a fixed
            // activation array, even when the forged extent fits 16 bytes.
            return Err(bad("vector view root type", span));
        }
        let size = match aggregate {
            AggregateTy::FixedArray(array) if array.length() == 0 => 0,
            _ => self
                .plan
                .witness()
                .aggregate_layout(aggregate)
                .map_err(|_| bad("view layout", span))?
                .size(),
        };
        let extent = self.owner_extent(key, span)?;
        index(view.offset, span)?
            .checked_add(size)
            .filter(|end| *end <= extent.len())
            .ok_or_else(|| bad("view extent", span))?;
        Ok(aggregate)
    }
    /// Re-derive projected storage from immutable nominal paths, rather than
    /// trusting a retained offset/extent. Slice reborrows may cross frames; the
    /// activation and local reference depth limits bound this parent walk.
    fn projected_view(
        &self,
        permission: LoanKey,
        root: OwnerKey,
        span: Span,
    ) -> Result<BorrowView> {
        let mut permission = permission;
        let mut offset = 0u64;
        let mut selected = None;
        for _ in 0..plan::MAX_FRAMES * (MAX_LOCAL_REFERENCE_DEPTH + 1) {
            let loan = self.loan(permission, span)?;
            if loan.root != root {
                return Err(bad("view ancestry root", span));
            }
            let function = self.function(self.frames[index(permission.frame, span)?].function);
            let descriptor = function
                .loans
                .get(index(permission.loan, span)?)
                .ok_or_else(|| bad("view loan descriptor", span))?;
            if !descriptor.projection.is_empty() {
                let base = match descriptor.authority {
                    AccessBase::Owner(owner) => function
                        .owners
                        .get(owner.0)
                        .map(|owner| BorrowedTy::Exact(owner.aggregate())),
                    AccessBase::Parameter(parameter) => function
                        .references
                        .get(parameter.0)
                        .map(ReferenceDecl::referent),
                    AccessBase::Loan(loan) => function.loans.get(loan.0).map(LoanDecl::referent),
                }
                .ok_or_else(|| bad("view projection authority", span))?;
                let BorrowedTy::Exact(base) = base else {
                    return Err(bad("view projection root", span));
                };
                let (
                    ValueTy::Owned(
                        aggregate @ (AggregateTy::Record(_)
                        | AggregateTy::FixedArray(_)
                        | AggregateTy::ByteBuf
                        | AggregateTy::File
                        | AggregateTy::Vector(_)
                        | AggregateTy::RecordVector(_)
                        | AggregateTy::OwnedRecordVector(_)),
                    ),
                    relative,
                ) = self
                    .plan
                    .witness()
                    .declarations()
                    .projection(base, &descriptor.projection)
                    .map_err(|_| bad("view projection path", span))?
                else {
                    return Err(bad("view projection array", span));
                };
                if selected.is_none() {
                    selected = Some(
                        AggregateSlot::try_from_aggregate(aggregate)
                            .map_err(|_| bad("view projection descriptor", span))?,
                    );
                }
                offset = offset
                    .checked_add(relative as u64)
                    .ok_or_else(|| bad("view projection offset", span))?;
            }
            if loan.parent.instance == 0 {
                let AccessBase::Owner(owner) = descriptor.authority else {
                    return Err(bad("view projection root authority", span));
                };
                let frame = &self.frames[index(permission.frame, span)?];
                if root.frame != permission.frame
                    || root.activation != frame.activation
                    || root.owner != owner.0 as u64
                {
                    return Err(bad("view projection root identity", span));
                }
                return selected
                    .map(|aggregate| BorrowView {
                        offset,
                        aggregate: Some(aggregate),
                    })
                    .ok_or_else(|| bad("projected view provenance", span));
            }
            permission = loan.parent;
        }
        Err(bad("projected view provenance", span))
    }
    fn checked_handle_view(&self, handle: ReferenceHandle, span: Span) -> Result<AggregateTy> {
        let actual = handle
            .view
            .aggregate
            .ok_or_else(|| bad("handle view type", span))?
            .aggregate();
        let frame = self
            .frames
            .get(index(handle.permission.frame, span)?)
            .ok_or_else(|| bad("view loan frame", span))?;
        let f = self.function(frame.function);
        let loan = f
            .loans
            .get(index(handle.permission.loan, span)?)
            .ok_or_else(|| bad("view loan descriptor", span))?;
        if let Some(element) = super::vector_type::VectorTy::from_aggregate(actual) {
            if loan.referent() == element.slice() {
                self.vector_slice(handle, element, span)?;
                return Ok(actual);
            }
        }
        if actual == AggregateTy::ByteBuf && loan.referent() == BorrowedTy::ScalarSlice(hir::Ty::U8)
        {
            self.buffer_slice(handle, span)?;
            return Ok(actual);
        }
        let root_type = self.aggregate(handle.root, span)?;
        if let (AggregateTy::FixedArray(root), AggregateTy::FixedArray(view)) = (root_type, actual)
        {
            if root.element() == view.element() && (handle.view.offset != 0 || root != view) {
                self.checked_byte_array_provenance(handle, span)?;
                handle
                    .view
                    .offset
                    .checked_add(view.length() as u64 * view.stride() as u64)
                    .filter(|end| *end <= root.length() as u64 * root.stride() as u64)
                    .ok_or_else(|| bad("view root type", span))?;
            } else {
                self.view_extent(handle.root, handle.view, span)?;
            }
        } else {
            self.view_extent(handle.root, handle.view, span)?;
        }
        if matches!(self.aggregate(handle.root, span)?, AggregateTy::Record(_))
            && (matches!(
                actual,
                AggregateTy::FixedArray(_)
                    | AggregateTy::ByteBuf
                    | AggregateTy::File
                    | AggregateTy::Vector(_)
                    | AggregateTy::RecordVector(_)
            ) || (matches!(actual, AggregateTy::Record(_))
                && (actual != root_type || handle.view.offset != 0)))
        {
            let original = self.projected_view(handle.permission, handle.root, span)?;
            match (original.aggregate.map(AggregateSlot::aggregate), actual) {
                (Some(AggregateTy::FixedArray(original_array)), AggregateTy::FixedArray(array))
                    if original_array.element() == array.element() =>
                {
                    self.checked_byte_array_provenance(handle, span)?;
                    let relative = handle
                        .view
                        .offset
                        .checked_sub(original.offset)
                        .ok_or_else(|| bad("projected range start", span))?;
                    if !relative.is_multiple_of(array.stride() as u64)
                        || relative
                            .checked_add(array.length() as u64 * array.stride() as u64)
                            .is_none_or(|end| {
                                end > original_array.length() as u64
                                    * original_array.stride() as u64
                            })
                    {
                        return Err(bad("projected range end", span));
                    }
                }
                _ if original == handle.view => (),
                _ => return Err(bad("projected view mismatch", span)),
            }
        }
        Ok(actual)
    }
    /// Only a checked scalar-slice loan may retain a subrange. Re-derive its root from
    /// immutable owner/parameter authority and require each retained child view
    /// to stay inside its parent. A matching element type alone is no proof.
    fn checked_byte_array_provenance(&self, mut handle: ReferenceHandle, span: Span) -> Result<()> {
        for _ in 0..plan::MAX_FRAMES * (MAX_LOCAL_REFERENCE_DEPTH + 1) {
            let row = self.loan(handle.permission, span)?;
            if row.root != handle.root || row.view != handle.view {
                return Err(bad("view root type", span));
            }
            let frame = &self.frames[index(handle.permission.frame, span)?];
            let declaration = self
                .function(frame.function)
                .loans
                .get(index(handle.permission.loan, span)?)
                .ok_or_else(|| bad("view loan descriptor", span))?;
            let actual = handle
                .view
                .aggregate
                .map(AggregateSlot::aggregate)
                .ok_or_else(|| bad("view root type", span))?;
            let scalar_slice = matches!(actual, AggregateTy::FixedArray(array)
                if declaration.referent() == BorrowedTy::ScalarSlice(array.element()));
            if !scalar_slice && declaration.referent() != BorrowedTy::Exact(actual) {
                return Err(bad("view root type", span));
            }
            if !scalar_slice {
                self.view_extent(handle.root, handle.view, span)?;
            }
            match declaration.authority {
                AccessBase::Owner(owner) => {
                    if row.parent.instance != 0
                        || handle.root.frame != handle.permission.frame
                        || handle.root.activation != frame.activation
                        || handle.root.owner != owner.0 as u64
                    {
                        return Err(bad("view root type", span));
                    }
                    return Ok(());
                }
                AccessBase::Parameter(_) | AccessBase::Loan(_) => {
                    let parent = match declaration.authority {
                        AccessBase::Parameter(parameter) => *frame
                            .references
                            .get(parameter.0)
                            .ok_or_else(|| bad("view parent reference", span))?,
                        AccessBase::Loan(loan) => self.reference_from_loan(
                            index(handle.permission.frame, span)?,
                            loan,
                            span,
                        )?,
                        AccessBase::Owner(_) => return Err(bad("view parent reference", span)),
                    };
                    if row.parent != parent.permission || row.root != parent.root {
                        return Err(bad("view root type", span));
                    }
                    let mut base_view = parent.view;
                    if !declaration.projection.is_empty() {
                        let aggregate = base_view
                            .aggregate
                            .map(AggregateSlot::aggregate)
                            .ok_or_else(|| bad("view parent type", span))?;
                        let (
                            ValueTy::Owned(
                                projected @ (AggregateTy::Record(_)
                                | AggregateTy::FixedArray(_)
                                | AggregateTy::ByteBuf
                                | AggregateTy::File
                                | AggregateTy::Vector(_)
                                | AggregateTy::RecordVector(_)),
                            ),
                            offset,
                        ) = self
                            .plan
                            .witness()
                            .declarations()
                            .projection(aggregate, &declaration.projection)
                            .map_err(|_| bad("view parent projection", span))?
                        else {
                            return Err(bad("view root type", span));
                        };
                        base_view.offset = base_view
                            .offset
                            .checked_add(offset as u64)
                            .ok_or_else(|| bad("view root type", span))?;
                        base_view.aggregate = Some(
                            AggregateSlot::try_from_aggregate(projected)
                                .map_err(|_| bad("view root type", span))?,
                        );
                    }
                    match (actual, base_view.aggregate.map(AggregateSlot::aggregate)) {
                        (AggregateTy::FixedArray(child), Some(AggregateTy::FixedArray(base)))
                            if child.element() == base.element() =>
                        {
                            if handle
                                .view
                                .offset
                                .checked_sub(base_view.offset)
                                .filter(|start| start.is_multiple_of(child.stride() as u64))
                                .and_then(|start| {
                                    start.checked_add(child.length() as u64 * child.stride() as u64)
                                })
                                .is_none_or(|end| end > base.length() as u64 * base.stride() as u64)
                            {
                                return Err(bad("view root type", span));
                            }
                        }
                        (AggregateTy::Record(_), _) if handle.view == base_view => (),
                        _ => return Err(bad("view root type", span)),
                    }
                    handle = parent;
                }
            }
        }
        Err(bad("view ancestry", span))
    }
    fn base_view(
        &self,
        frame: usize,
        base: AccessBase,
        key: OwnerKey,
        span: Span,
    ) -> Result<BorrowView> {
        let view = match base {
            AccessBase::Owner(_) => self.whole_view(key, span)?,
            AccessBase::Parameter(id) => self.frames[frame].references[id.0].view,
            AccessBase::Loan(id) => self.reference_from_loan(frame, id, span)?.view,
        };
        let actual = match base {
            AccessBase::Owner(_) => self.view_extent(key, view, span)?,
            AccessBase::Parameter(id) => {
                self.checked_handle_view(self.frames[frame].references[id.0], span)?
            }
            AccessBase::Loan(id) => {
                self.checked_handle_view(self.reference_from_loan(frame, id, span)?, span)?
            }
        };
        let f = self.function(self.frames[frame].function);
        let expected = match base {
            AccessBase::Owner(owner) => BorrowedTy::Exact(f.owners[owner.0].aggregate()),
            AccessBase::Parameter(reference) => f.references[reference.0].referent(),
            AccessBase::Loan(loan) => f.loans[loan.0].referent(),
        };
        self.plan
            .witness()
            .check_borrowed_view(BorrowedTy::Exact(actual), expected)
            .map_err(|_| bad("base view type", span))?;
        Ok(view)
    }
    fn array_base(
        &self,
        frame: usize,
        base: AccessBase,
        access: Access,
        span: Span,
    ) -> Result<(OwnerKey, FixedArrayTy, usize)> {
        let key = self.base(frame, base, access, span)?;
        let view = self
            .base_view(frame, base, key, span)
            .map_err(|_| bad("array base type", span))?;
        // base_view already validates the exact whole view or the complete
        // checked byte-range ancestry. Reapplying whole-owner layout rules
        // here would incorrectly reject a genuine byte subrange.
        let Some(AggregateTy::FixedArray(array)) = view.aggregate.map(AggregateSlot::aggregate)
        else {
            return Err(bad("array view type", span));
        };
        Ok((key, array, index(view.offset, span)?))
    }
    fn owner_extent(&self, key: OwnerKey, span: Span) -> Result<std::ops::Range<usize>> {
        let aggregate = self.aggregate(key, span)?;
        let frame = &self.frames[key.frame as usize];
        let offset = self
            .plan
            .function(frame.function)
            .owner_offset(OwnerPlaceId(key.owner as usize));
        let size = self
            .plan
            .witness()
            .aggregate_layout(aggregate)
            .map_err(|_| bad("payload aggregate layout", span))?
            .size();
        let end = offset
            .checked_add(size)
            .filter(|end| *end <= frame.payload.len())
            .ok_or_else(|| bad("payload range", span))?;
        Ok(offset..end)
    }
    fn leaf_offset(
        &self,
        key: OwnerKey,
        relative: usize,
        ty: hir::Ty,
        span: Span,
    ) -> Result<usize> {
        let extent = self.owner_extent(key, span)?;
        scalar_offset(extent, relative, ty, span)
    }
    fn load_leaf(&self, key: OwnerKey, leaf: ScalarLeaf, span: Span) -> Result<V> {
        let offset = self.leaf_offset(key, leaf.offset, leaf.ty, span)?;
        V::decode(
            &self.frames[key.frame as usize].payload,
            offset,
            leaf.ty,
            span,
        )
    }
    fn store_leaf(&mut self, key: OwnerKey, leaf: ScalarLeaf, value: V, span: Span) -> Result<()> {
        if leaf.ty != value.ty() {
            return Err(bad("leaf value type", span));
        }
        let offset = self.leaf_offset(key, leaf.offset, leaf.ty, span)?;
        value.encode(&mut self.frames[key.frame as usize].payload, offset, span)
    }
    fn projection_base(
        &self,
        frame: usize,
        base: AccessBase,
        access: Access,
        path: &[FieldId],
        span: Span,
    ) -> Result<(OwnerKey, ValueTy, usize)> {
        let key = self.projected_base(
            frame,
            base,
            access,
            span,
            ProjectionRequest::Path {
                base,
                path,
                write: matches!(access, Access::Write),
            },
        )?;
        let function = self.function(self.frames[frame].function);
        let expected = match base {
            AccessBase::Owner(owner) => BorrowedTy::Exact(function.owners[owner.0].aggregate()),
            AccessBase::Parameter(reference) => function
                .references
                .get(reference.0)
                .ok_or_else(|| bad("projection reference declaration", span))?
                .referent(),
            AccessBase::Loan(loan) => function
                .loans
                .get(loan.0)
                .ok_or_else(|| bad("projection local reference declaration", span))?
                .referent(),
        };
        let view = self
            .base_view(frame, base, key, span)
            .map_err(|_| bad("projection base type", span))?;
        let actual = view
            .aggregate
            .ok_or_else(|| bad("projection view type", span))?
            .aggregate();
        let declarations = self.plan.witness().declarations();
        self.plan
            .witness()
            .check_borrowed_view(BorrowedTy::Exact(actual), expected)
            .map_err(|_| bad("projection base type", span))?;
        let (ty, relative) = declarations
            .projection(actual, path)
            .map_err(|_| bad("projection path", span))?;
        let relative = index(view.offset, span)?
            .checked_add(relative)
            .ok_or_else(|| bad("projection view offset", span))?;
        let size = match ty {
            ValueTy::Scalar(ty) => scalar_size(ty),
            ValueTy::Owned(aggregate) => self
                .plan
                .witness()
                .aggregate_layout(aggregate)
                .map_err(|_| bad("projection layout", span))?
                .size(),
        };
        let extent = self.owner_extent(key, span)?;
        relative
            .checked_add(size)
            .filter(|end| *end <= extent.len())
            .ok_or_else(|| bad("projection extent", span))?;
        Ok((key, ty, relative))
    }
    fn projection_leaf(
        &self,
        frame: usize,
        ty: ValueTy,
        relative: usize,
        operand: Option<Operand>,
        span: Span,
    ) -> Result<(ScalarLeaf, Option<usize>)> {
        match (ty, operand) {
            (ValueTy::Scalar(ty), None) => Ok((
                ScalarLeaf {
                    offset: relative,
                    ty,
                },
                None,
            )),
            (ValueTy::Owned(AggregateTy::FixedArray(array)), Some(operand)) => {
                let ordinal = array_index(self.read(frame, operand)?, array, span)?;
                let offset = ordinal
                    .checked_mul(array.stride())
                    .and_then(|n| relative.checked_add(n))
                    .ok_or_else(|| bad("projection element offset", span))?;
                Ok((
                    ScalarLeaf {
                        offset,
                        ty: array.element(),
                    },
                    Some(ordinal),
                ))
            }
            _ => Err(bad("projection leaf type", span)),
        }
    }
    fn array_offset(
        &self,
        key: OwnerKey,
        array: FixedArrayTy,
        ordinal: usize,
        span: Span,
    ) -> Result<usize> {
        if self.array_type(key, span)? != array || ordinal >= array.length() {
            return Err(bad("array element identity", span));
        }
        let extent = self.owner_extent(key, span)?;
        let offset = ordinal
            .checked_mul(array.stride())
            .and_then(|offset| extent.start.checked_add(offset))
            .ok_or_else(|| bad("array offset overflow", span))?;
        offset
            .checked_add(array.stride())
            .filter(|end| *end <= extent.end)
            .ok_or_else(|| bad("array element range", span))?;
        Ok(offset)
    }
    fn load_element(
        &self,
        key: OwnerKey,
        array: FixedArrayTy,
        ordinal: usize,
        span: Span,
    ) -> Result<V> {
        let offset = self.array_offset(key, array, ordinal, span)?;
        V::decode(
            &self.frames[key.frame as usize].payload,
            offset,
            array.element(),
            span,
        )
    }
    fn store_element(
        &mut self,
        key: OwnerKey,
        array: FixedArrayTy,
        ordinal: usize,
        value: V,
        span: Span,
    ) -> Result<()> {
        if value.ty() != array.element() {
            return Err(bad("array element type", span));
        }
        let offset = self.array_offset(key, array, ordinal, span)?;
        value.encode(&mut self.frames[key.frame as usize].payload, offset, span)
    }
    fn zero_sentinel(&mut self, key: OwnerKey, span: Span) -> Result<()> {
        let extent = self.owner_extent(key, span)?;
        self.frames[key.frame as usize].payload[extent].fill(0);
        Ok(())
    }

    /// The checked nominal table is the only tag/type authority. Dispatch uses
    /// this tag-only operation, so neither a failed nor a successful test exposes
    /// the payload before ConsumeVariant has paid its separate charge.
    fn enum_tag(&mut self, key: OwnerKey, span: Span) -> Result<(VariantId, Option<ValueTy>)> {
        let AggregateTy::Enum(enumeration) = self.aggregate(key, span)? else {
            return Err(bad("enum type", span));
        };
        let extent = self.owner_extent(key, span)?;
        let offset = scalar_offset(extent, 0, hir::Ty::I32, span)?;
        let tag = u32::from_le_bytes(
            self.frames[key.frame as usize].payload[offset..offset + 4]
                .try_into()
                .map_err(|_| bad("enum tag", span))?,
        );
        #[cfg(test)]
        self.record_event(Event::EnumTagRead(key, tag));
        let variant = self
            .plan
            .witness()
            .declarations()
            .enums()
            .variant_for_tag(enumeration, tag as usize)
            .map_err(|_| bad("enum tag", span))?;
        Ok((variant.id(), variant.value_payload()))
    }
    fn load_enum(
        &mut self,
        key: OwnerKey,
        expected: Option<VariantId>,
        span: Span,
    ) -> Result<StoredEnumValue<V>> {
        let (variant, payload_ty) = self.enum_tag(key, span)?;
        if expected.is_some_and(|expected| expected != variant) {
            return Err(bad("enum tag", span));
        }
        let payload_offset = self
            .plan
            .witness()
            .declarations()
            .enums()
            .variant(variant.enumeration, variant)
            .map_err(|_| bad("enum variant", span))?
            .payload_offset()
            .unwrap_or(0);
        let payload = if let Some(ValueTy::Scalar(ty)) = payload_ty {
            let extent = self.owner_extent(key, span)?;
            let offset = scalar_offset(extent, payload_offset, ty, span)?;
            #[cfg(test)]
            self.record_event(Event::EnumPayloadRead(key, scalar_size(ty)));
            Some(
                V::decode(&self.frames[key.frame as usize].payload, offset, ty, span)
                    .map_err(|_| bad("enum payload", span))?,
            )
        } else if payload_ty.is_none() {
            None
        } else {
            return Err(bad("scalar enum payload", span));
        };
        Ok(StoredEnumValue {
            tag: variant.index as u32,
            payload,
            payload_offset: payload_offset as u8,
        })
    }
    fn preflight_enum_binding(
        &self,
        frame: usize,
        destination: Option<LocalId>,
        value: StoredEnumValue<V>,
        span: Span,
    ) -> Result<()> {
        match (destination, value.payload) {
            (None, None) => Ok(()),
            (Some(destination), Some(value)) => {
                let declaration = self
                    .function(self.frames[frame].function)
                    .locals
                    .get(destination.0)
                    .ok_or_else(|| bad("scalar identity", span))?;
                let slot = self.frames[frame]
                    .slots
                    .get(destination.0)
                    .ok_or_else(|| bad("scalar storage", span))?;
                if declaration.ty != value.ty() {
                    return Err(bad("scalar type", span));
                }
                if declaration.kind == LocalKind::Parameter && slot.is_some() {
                    return Err(bad("parameter reassignment", span));
                }
                Ok(())
            }
            _ => Err(bad("enum binding", span)),
        }
    }

    fn transfer_payload(&mut self, from: OwnerKey, to: OwnerKey, span: Span) -> Result<()> {
        // Observation identifies the newly installed value, matching parameter transfers.
        // Every caller preflights this same epoch before the logical transition.
        #[cfg(test)]
        let initialized_destination = OwnerKey {
            generation: epoch(to.generation, span)?,
            ..to
        };
        let f = self.function(self.frames[from.frame as usize].function);
        let record = f.owners[from.owner as usize].aggregate();
        let target = self
            .function(self.frames[to.frame as usize].function)
            .owners[to.owner as usize]
            .aggregate();
        self.plan
            .witness()
            .same_aggregate_type(record, target)
            .map_err(|_| bad("nominal transfer", span))?;
        let source = self.owner_extent(from, span)?;
        let destination = self.owner_extent(to, span)?;
        if from.frame == to.frame
            && source.start < destination.end
            && destination.start < source.end
        {
            return Err(bad("overlapping whole transfer", span));
        }
        // Sums have no static leaves. Snapshot only the checked active value,
        // before any destination write; caller state/epoch preflights precede us.
        if self.active_aggregate(record) {
            self.preflight_active::<false>(
                from,
                source.clone(),
                record,
                destination.clone(),
                span,
            )?;
            return self.copy_prevalidated_transfer(from, source, to, destination, record, span);
        }
        if matches!(record, AggregateTy::Enum(_)) {
            let value = self.load_enum(from, None, span)?;
            enum_offsets(destination.clone(), value, span)?;
            #[cfg(test)]
            let observation = self.observe_begin(to, StorageObservationKind::Transfer, true, span);
            encode_enum(
                &mut self.frames[to.frame as usize].payload,
                destination,
                value,
                span,
            )?;
            #[cfg(test)]
            {
                self.record_event(Event::Transfer(from, initialized_destination));
                self.observe_end(observation);
            }
            return Ok(());
        }
        let declarations = self.plan.witness().declarations();
        // Preflight all scalar leaves, including positive empty-value sentinels,
        // before changing payload or ownership. Padding is never copied.
        for leaf in declarations
            .leaves(record)
            .map_err(|_| bad("transfer leaves", span))?
        {
            self.load_leaf(from, leaf, span)?;
            self.leaf_offset(to, leaf.offset, leaf.ty, span)?;
        }
        #[cfg(test)]
        let observation = if matches!(record, AggregateTy::FixedArray(_)) {
            self.observe_begin(to, StorageObservationKind::Transfer, true, span)
        } else {
            None
        };
        for leaf in declarations
            .leaves(record)
            .map_err(|_| bad("transfer leaves", span))?
        {
            let value = self.load_leaf(from, leaf, span)?;
            self.store_leaf(to, leaf, value, span)?;
        }
        #[cfg(test)]
        {
            self.record_event(Event::Transfer(from, initialized_destination));
            self.observe_end(observation);
        }
        Ok(())
    }
    /// Scalar-only sums keep their original snapshot and event path. An owned
    /// alternative requires tag-directed traversal even when another arm is active.
    fn active_aggregate(&self, aggregate: AggregateTy) -> bool {
        if self
            .plan
            .witness()
            .has_enum_record_nesting(aggregate)
            .expect("verified aggregate nesting")
        {
            return true;
        }
        if self
            .plan
            .witness()
            .contains_resources(aggregate)
            .expect("verified resource containment")
        {
            return true;
        }
        let AggregateTy::Enum(enumeration) = aggregate else {
            return false;
        };
        self.plan
            .witness()
            .declarations()
            .enums()
            .variants(enumeration)
            .expect("verified enum")
            .iter()
            .any(|variant| matches!(variant.value_payload(), Some(ValueTy::Owned(_))))
    }
    fn owned_record_field_extent(
        &self,
        root: OwnerKey,
        record: RecordId,
        field: FieldId,
        span: Span,
    ) -> Result<(AggregateTy, std::ops::Range<usize>)> {
        let declarations = self.plan.witness().declarations();
        let member = declarations
            .field(record, field)
            .map_err(|_| bad("record pattern field", span))?;
        let ValueTy::Owned(aggregate) = member.value_ty() else {
            return Err(bad("owned record pattern field", span));
        };
        let extent = self.owner_extent(root, span)?;
        let start = extent
            .start
            .checked_add(member.offset())
            .ok_or_else(|| bad("record pattern field extent", span))?;
        let size = self
            .plan
            .witness()
            .aggregate_layout(aggregate)
            .map_err(|_| bad("record pattern field layout", span))?
            .size();
        let end = start
            .checked_add(size)
            .filter(|end| *end <= extent.end)
            .ok_or_else(|| bad("record pattern field extent", span))?;
        Ok((aggregate, start..end))
    }
    /// No new admission or fuel debit after the caller has validated the full
    /// active source and destination. Replacement can release its old value
    /// between that preflight and this commit without a second fallible scan.
    fn copy_prevalidated_transfer(
        &mut self,
        from: OwnerKey,
        source: std::ops::Range<usize>,
        to: OwnerKey,
        destination: std::ops::Range<usize>,
        aggregate: AggregateTy,
        span: Span,
    ) -> Result<()> {
        #[cfg(test)]
        let initialized_destination = OwnerKey {
            generation: epoch(to.generation, span)?,
            ..to
        };
        #[cfg(test)]
        let observation = self.observe_begin(to, StorageObservationKind::Transfer, true, span);
        self.copy_active(from, source, to, destination, aggregate, span)?;
        #[cfg(test)]
        {
            self.record_event(Event::Transfer(from, initialized_destination));
            self.observe_end(observation);
        }
        Ok(())
    }

    // Moves pay for resource admission. Release maintenance performs the same
    // scalar, tag and descriptor checks without spending user execution fuel.
    fn preflight_active<const MAINTENANCE: bool>(
        &mut self,
        from: OwnerKey,
        source: std::ops::Range<usize>,
        aggregate: AggregateTy,
        destination: std::ops::Range<usize>,
        span: Span,
    ) -> Result<()> {
        let declarations = self.plan.witness().declarations();
        if self
            .plan
            .witness()
            .has_enum_record_nesting(aggregate)
            .map_err(|_| bad("composed aggregate identity", span))?
        {
            for cell in composition::Cells::<V>::new(
                declarations,
                &self.frames[from.frame as usize].payload,
                source.clone(),
                aggregate,
                span,
            ) {
                let (leaf, _) = cell?;
                scalar_offset(destination.clone(), leaf.offset, leaf.ty, span)?;
            }
        } else {
            for cell in ValueCells::<V>::new(
                declarations,
                &self.frames[from.frame as usize].payload,
                source.clone(),
                aggregate,
                span,
            ) {
                let (leaf, _) = cell?;
                scalar_offset(destination.clone(), leaf.offset, leaf.ty, span)?;
            }
        }
        if self.plan.uses_owned_vectors() {
            let scope = if MAINTENANCE {
                resources::RuntimeResourceScope::maintenance(self.plan.witness())
            } else {
                resources::RuntimeResourceScope::paid(self.plan.witness(), &mut self.fuel)
            };
            resources::preflight(
                scope,
                &self.heap,
                aggregate,
                &self.frames[from.frame as usize].payload[source],
                destination.len(),
                span,
            )?;
        } else {
            resources::preflight(
                declarations,
                &self.heap,
                aggregate,
                &self.frames[from.frame as usize].payload[source],
                destination.len(),
                span,
            )?;
        }
        Ok(())
    }
    fn release_resources(&mut self, key: OwnerKey, span: Span) -> Result<()> {
        let aggregate = self.aggregate(key, span)?;
        let declarations = self.plan.witness().declarations();
        if !self
            .plan
            .witness()
            .contains_resources(aggregate)
            .map_err(|_| bad("resource type", span))?
        {
            return Ok(());
        }
        let extent = self.owner_extent(key, span)?;
        if self.plan.uses_owned_vectors() {
            resources::release(
                resources::RuntimeResourceScope::maintenance(self.plan.witness()),
                &mut self.heap,
                aggregate,
                &self.frames[key.frame as usize].payload[extent],
                span,
            )
        } else {
            resources::release(
                declarations,
                &mut self.heap,
                aggregate,
                &self.frames[key.frame as usize].payload[extent],
                span,
            )
        }
    }
    /// The caller validates every active cell and both ownership transitions
    /// before entering here. Source and destination have disjoint storage, so
    /// this second bounded traversal cannot invalidate a later source read.
    fn copy_active(
        &mut self,
        from: OwnerKey,
        source: std::ops::Range<usize>,
        to: OwnerKey,
        destination: std::ops::Range<usize>,
        aggregate: AggregateTy,
        span: Span,
    ) -> Result<()> {
        let witness = self.plan.witness();
        let source_frame = from.frame as usize;
        let destination_frame = to.frame as usize;
        if source_frame == destination_frame {
            let bytes = &mut self.frames[source_frame].payload;
            if source.end <= destination.start {
                let (before, after) = bytes.split_at_mut(destination.start);
                copy_active_bytes_with_witness::<V, W>(
                    witness,
                    aggregate,
                    &before[source],
                    &mut after[..destination.len()],
                    span,
                )
            } else if destination.end <= source.start {
                let (before, after) = bytes.split_at_mut(source.start);
                copy_active_bytes_with_witness::<V, W>(
                    witness,
                    aggregate,
                    &after[..source.len()],
                    &mut before[destination],
                    span,
                )
            } else {
                Err(bad("overlapping whole transfer", span))
            }
        } else if source_frame < destination_frame {
            let (before, after) = self.frames.split_at_mut(destination_frame);
            copy_active_bytes_with_witness::<V, W>(
                witness,
                aggregate,
                &before[source_frame].payload[source],
                &mut after[0].payload[destination],
                span,
            )
        } else {
            let (before, after) = self.frames.split_at_mut(source_frame);
            copy_active_bytes_with_witness::<V, W>(
                witness,
                aggregate,
                &after[0].payload[source],
                &mut before[destination_frame].payload[destination],
                span,
            )
        }
    }
    fn owned_variant_extent(
        &self,
        key: OwnerKey,
        variant: VariantId,
        aggregate: AggregateTy,
        span: Span,
    ) -> Result<std::ops::Range<usize>> {
        let AggregateTy::Enum(enumeration) = self.aggregate(key, span)? else {
            return Err(bad("enum type", span));
        };
        let declarations = self.plan.witness().declarations();
        let declaration = declarations
            .enums()
            .variant(enumeration, variant)
            .map_err(|_| bad("enum tag", span))?;
        if declaration.value_payload() != Some(ValueTy::Owned(aggregate)) {
            return Err(bad("enum construction type", span));
        }
        let extent = self.owner_extent(key, span)?;
        let offset = extent
            .start
            .checked_add(
                declaration
                    .payload_offset()
                    .ok_or_else(|| bad("enum payload offset", span))?,
            )
            .ok_or_else(|| bad("enum payload range", span))?;
        let size = self
            .plan
            .witness()
            .aggregate_layout(aggregate)
            .map_err(|_| bad("enum payload layout", span))?
            .size();
        let end = offset
            .checked_add(size)
            .filter(|end| *end <= extent.end)
            .ok_or_else(|| bad("enum payload range", span))?;
        Ok(offset..end)
    }
    fn raw_key(&self, frame: usize, owner: OwnerPlaceId) -> OwnerKey {
        OwnerKey {
            frame: frame as u64,
            activation: self.frames[frame].activation,
            owner: owner.0 as u64,
            generation: self.frames[frame].owners[owner.0].generation,
        }
    }
    fn change_owner(
        &mut self,
        frame: usize,
        owner: OwnerPlaceId,
        state: u64,
        span: Span,
    ) -> Result<()> {
        let o = self.frames[frame]
            .owners
            .get_mut(owner.0)
            .ok_or_else(|| bad("owner transition", span))?;
        o.generation = epoch(o.generation, span)?;
        o.state = state;
        Ok(())
    }
    fn expect_owner(
        &self,
        frame: usize,
        id: OwnerPlaceId,
        legal: &[u64],
        span: Span,
    ) -> Result<()> {
        let o = self.frames[frame]
            .owners
            .get(id.0)
            .ok_or_else(|| bad("owner transition identity", span))?;
        if !legal.contains(&o.state) || o.shared_children != 0 || o.exclusive_children != 0 {
            return Err(bad("owner transition state", span));
        }
        epoch(o.generation, span)?;
        Ok(())
    }
    fn prepared(
        &self,
        frame: usize,
        call: CallSiteId,
        argument: usize,
        span: Span,
    ) -> Result<usize> {
        let f = self.function(self.frames[frame].function);
        let c = self.frames[frame]
            .calls
            .get(call.0)
            .ok_or_else(|| bad("call identity", span))?;
        if c.phase != PREPARING
            || c.next_argument as usize != argument
            || argument >= f.calls[call.0].arguments.len()
        {
            return Err(bad("argument preparation order", span));
        }
        Ok(self.plan.function(f.id).call(call).argument_start() + argument)
    }
    fn acquire(&mut self, frame: usize, id: LoanId, span: Span) -> Result<()> {
        self.acquire_range(frame, id, None, span)
    }
    fn acquire_range(
        &mut self,
        frame: usize,
        id: LoanId,
        range: Option<(i32, i32)>,
        span: Span,
    ) -> Result<()> {
        let f = self.function(self.frames[frame].function);
        let descriptor = &f.loans[id.0];
        let root = self.projected_base(
            frame,
            descriptor.authority,
            Access::Borrow(descriptor.kind),
            span,
            ProjectionRequest::Borrow(id),
        )?;
        let mut view = self.base_view(frame, descriptor.authority, root, span)?;
        let mut actual = view
            .aggregate
            .ok_or_else(|| bad("loan view type", span))?
            .aggregate();
        if !descriptor.projection.is_empty() {
            let (
                ValueTy::Owned(
                    projected @ (AggregateTy::Record(_)
                    | AggregateTy::FixedArray(_)
                    | AggregateTy::ByteBuf
                    | AggregateTy::File
                    | AggregateTy::Vector(_)
                    | AggregateTy::RecordVector(_)),
                ),
                relative,
            ) = self
                .plan
                .witness()
                .declarations()
                .projection(actual, &descriptor.projection)
                .map_err(|_| bad("loan projection", span))?
            else {
                return Err(bad("loan projection type", span));
            };
            view.offset = view
                .offset
                .checked_add(relative as u64)
                .ok_or_else(|| bad("loan view offset", span))?;
            view.aggregate = Some(
                AggregateSlot::try_from_aggregate(projected)
                    .map_err(|_| bad("loan view descriptor", span))?,
            );
            actual = self.view_extent(root, view, span)?;
        }
        self.plan
            .witness()
            .check_borrowed_view(BorrowedTy::Exact(actual), descriptor.referent())
            .map_err(|_| bad("loan borrowed view", span))?;
        view = if matches!(
            actual,
            AggregateTy::Vector(_) | AggregateTy::RecordVector(_)
        ) {
            self.stage_vector_view(frame, descriptor, root, view, range, span)?
        } else if matches!(actual, AggregateTy::FixedArray(array) if array.element() != hir::Ty::U8)
            && range.is_some()
        {
            self.stage_typed_array_view(descriptor, view, range.unwrap(), span)?
        } else {
            self.stage_byte_view(frame, descriptor, root, view, range, span)?
        };
        let parent = match descriptor.authority {
            AccessBase::Owner(_) => LoanKey::default(),
            AccessBase::Parameter(p) => self.frames[frame].references[p.0].permission,
            AccessBase::Loan(loan) => self.reference_from_loan(frame, loan, span)?.permission,
        };
        let previous = self.frames[frame].loans[id.0];
        if previous.state != 0 {
            return Err(bad("loan already active", span));
        }
        let instance = epoch(previous.instance, span)?;
        let counts = if parent.instance == 0 {
            let o = self.owner(root, span)?;
            (o.shared_children, o.exclusive_children)
        } else {
            let l = self.loan(parent, span)?;
            (l.shared_children, l.exclusive_children)
        };
        let count = match descriptor.kind {
            BorrowKind::Shared => counts.0,
            BorrowKind::Exclusive => counts.1,
        };
        let next = count
            .checked_add(1)
            .ok_or_else(|| bad("loan counter overflow", span))?;
        if parent.instance == 0 {
            let o = &mut self.frames[root.frame as usize].owners[root.owner as usize];
            match descriptor.kind {
                BorrowKind::Shared => o.shared_children = next,
                BorrowKind::Exclusive => o.exclusive_children = next,
            }
        } else {
            let l = &mut self.frames[parent.frame as usize].loans[parent.loan as usize];
            match descriptor.kind {
                BorrowKind::Shared => l.shared_children = next,
                BorrowKind::Exclusive => l.exclusive_children = next,
            }
        }
        self.frames[frame].loans[id.0] = LoanRuntime {
            instance,
            state: 1,
            root,
            parent,
            view,
            shared_children: 0,
            exclusive_children: 0,
        };
        #[cfg(test)]
        self.record_event(Event::Acquire(
            LoanKey {
                frame: frame as u64,
                activation: self.frames[frame].activation,
                loan: id.0 as u64,
                instance,
            },
            root,
            descriptor.kind,
        ));
        Ok(())
    }
    fn release_preflight(&self, key: LoanKey, span: Span) -> Result<()> {
        let loan = self.loan(key, span)?;
        self.owner(loan.root, span)?;
        if loan.shared_children != 0 || loan.exclusive_children != 0 {
            return Err(bad("release with active child", span));
        }
        let kind = self.loan_kind(key, span)?;
        let counts = if loan.parent.instance == 0 {
            let o = self.owner(loan.root, span)?;
            (o.shared_children, o.exclusive_children)
        } else {
            let p = self.loan(loan.parent, span)?;
            (p.shared_children, p.exclusive_children)
        };
        if match kind {
            BorrowKind::Shared => counts.0 == 0,
            BorrowKind::Exclusive => counts.1 == 0,
        } {
            return Err(bad("release counter", span));
        }
        Ok(())
    }
    fn release(&mut self, key: LoanKey, span: Span) -> Result<()> {
        self.release_preflight(key, span)?;
        let loan = *self.loan(key, span)?;
        let kind = self.loan_kind(key, span)?;
        if loan.parent.instance == 0 {
            let o = &mut self.frames[loan.root.frame as usize].owners[loan.root.owner as usize];
            match kind {
                BorrowKind::Shared => o.shared_children -= 1,
                BorrowKind::Exclusive => o.exclusive_children -= 1,
            }
        } else {
            let p = &mut self.frames[loan.parent.frame as usize].loans[loan.parent.loan as usize];
            match kind {
                BorrowKind::Shared => p.shared_children -= 1,
                BorrowKind::Exclusive => p.exclusive_children -= 1,
            }
        }
        self.frames[key.frame as usize].loans[key.loan as usize].state = 0;
        #[cfg(test)]
        self.record_event(Event::Release(key));
        Ok(())
    }
    fn scalar_statement(&mut self, frame: usize, statement: &Statement) -> Result<()> {
        let f = self.function(self.frames[frame].function);
        let assign = match statement {
            Statement::Assign(a) => a,
            Statement::Initialize { place, value, span }
            | Statement::Store {
                place, value, span, ..
            } => {
                let value = self.read(frame, *value)?;
                if f.places.get(place.id.0).is_none_or(|p| p.ty != value.ty()) {
                    return Err(bad("scalar place type", *span));
                }
                let index = f.locals.len() + place.id.0;
                if matches!(statement, Statement::Store { .. })
                    && self.frames[frame].slots[index].is_none()
                {
                    return Err(bad("uninitialized scalar place", *span));
                }
                self.frames[frame].slots[index] = Some(value);
                return Ok(());
            }
        };
        let value: V = match assign.value {
            Rvalue::BitNot {
                operand,
                operator_span,
            } => bitwise::complement(self.read(frame, operand)?, operator_span)?,
            Rvalue::Bitwise {
                op,
                left,
                right,
                operator_span,
            } => {
                let left = self.read(frame, left)?;
                let right = self.read(frame, right)?;
                bitwise::binary(op, left, right, operator_span)?
            }
            Rvalue::Bool(value) => Scalar::Bool(value).into(),
            Rvalue::Character(value) => V::from_character(value)
                .ok_or_else(|| bad("character literal storage", assign.span))?,
            Rvalue::I32(value) => Scalar::I32(value).into(),
            Rvalue::U8(value) => Scalar::U8(value).into(),
            Rvalue::Unit => Scalar::Unit.into(),
            Rvalue::CharacterConvert {
                op,
                operand,
                name_span,
                ..
            } => values::convert_character(op, self.read(frame, operand)?, name_span)?,
            Rvalue::FloatConvert {
                op,
                operand,
                name_span,
                ..
            } => floats::convert(op, self.read(frame, operand)?, name_span)?,
            Rvalue::Float { bits, ty } => {
                V::from_float(ty, bits).ok_or_else(|| bad("float literal storage", assign.span))?
            }
            Rvalue::FloatNegate {
                operand,
                operator_span,
            } => floats::negate(self.read(frame, operand)?, operator_span)?,
            Rvalue::FloatArithmetic {
                op,
                left,
                right,
                operator_span,
            } => floats::arithmetic(
                op,
                self.read(frame, left)?,
                self.read(frame, right)?,
                operator_span,
            )?,
            Rvalue::I64(value) => values::checked_wide(value, assign.span)?,
            Rvalue::Integer128 { ty, low, high } => {
                V::from_integer128(ty, u128::from(low) | (u128::from(high) << 64))
                    .ok_or_else(|| bad("128-bit literal storage", assign.span))?
            }
            Rvalue::Integer { ty, bits } => V::from_integer(ty, bits)
                .ok_or_else(|| bad("integer literal storage", assign.span))?,
            Rvalue::CheckedInteger {
                op,
                left,
                right,
                operator_span,
            } => {
                let left = self.read(frame, left)?;
                let right = self.read(frame, right)?;
                integers::arithmetic(op, left, right, operator_span)?
            }
            Rvalue::CheckedNegateInteger {
                operand,
                operator_span,
            } => integers::negate(self.read(frame, operand)?, operator_span)?,
            Rvalue::CheckedIntegerCast {
                operand,
                target,
                name_span,
                ..
            } => integers::cast(self.read(frame, operand)?, target, name_span)?,
            Rvalue::Copy(value) => self.read(frame, value)?,
            Rvalue::Load(place) => *self.frames[frame]
                .slots
                .get(f.locals.len() + place.id.0)
                .ok_or_else(|| bad("scalar place index", place.span))?
                .as_ref()
                .ok_or_else(|| bad("uninitialized scalar place", place.span))?,
            Rvalue::CheckedI32ToU8 {
                operand, name_span, ..
            } => {
                let Some(Scalar::I32(value)) = self.read(frame, operand)?.narrow() else {
                    return Err(bad("narrowing type", operand.span));
                };
                Scalar::U8(u8::try_from(value).map_err(|_| RunFailure::ByteRange(name_span))?)
                    .into()
            }
            Rvalue::U8ToI32 { operand, .. } => {
                let Some(Scalar::U8(value)) = self.read(frame, operand)?.narrow() else {
                    return Err(bad("widening type", operand.span));
                };
                Scalar::I32(i32::from(value)).into()
            }
            Rvalue::WidenI64 { operand, .. } => {
                let value = match self.read(frame, operand)?.narrow() {
                    Some(Scalar::I32(value)) => i64::from(value),
                    Some(Scalar::U8(value)) => i64::from(value),
                    _ => return Err(bad("i64 widening type", operand.span)),
                };
                values::checked_wide(value, assign.span)?
            }
            Rvalue::CheckedI64ToI32 {
                operand, name_span, ..
            } => {
                let value = self
                    .read(frame, operand)?
                    .wide()
                    .ok_or_else(|| bad("i64 narrowing type", operand.span))?;
                Scalar::I32(i32::try_from(value).map_err(|_| RunFailure::I64ToI32Range(name_span))?)
                    .into()
            }
            Rvalue::CheckedI64ToU8 {
                operand, name_span, ..
            } => {
                let value = self
                    .read(frame, operand)?
                    .wide()
                    .ok_or_else(|| bad("i64 narrowing type", operand.span))?;
                Scalar::U8(u8::try_from(value).map_err(|_| RunFailure::I64ToU8Range(name_span))?)
                    .into()
            }
            Rvalue::NotBool { operand, .. } => {
                let Some(Scalar::Bool(value)) = self.read(frame, operand)?.narrow() else {
                    return Err(bad("not bool", operand.span));
                };
                Scalar::Bool(!value).into()
            }
            Rvalue::CheckedNegateI32 {
                operand,
                operator_span,
            } => {
                let Some(Scalar::I32(value)) = self.read(frame, operand)?.narrow() else {
                    return Err(bad("negation type", operand.span));
                };
                Scalar::I32(
                    value
                        .checked_neg()
                        .ok_or(RunFailure::Overflow(operator_span))?,
                )
                .into()
            }
            Rvalue::CheckedNegateI64 {
                operand,
                operator_span,
            } => {
                let value = self
                    .read(frame, operand)?
                    .wide()
                    .ok_or_else(|| bad("i64 negation type", operand.span))?;
                values::checked_wide(
                    value
                        .checked_neg()
                        .ok_or(RunFailure::WideOverflow(operator_span))?,
                    assign.span,
                )?
            }
            Rvalue::CheckedI32 {
                op,
                left,
                right,
                operator_span,
            } => {
                let (Some(Scalar::I32(a)), Some(Scalar::I32(b))) = (
                    self.read(frame, left)?.narrow(),
                    self.read(frame, right)?.narrow(),
                ) else {
                    return Err(bad("arithmetic type", assign.span));
                };
                if matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder) && b == 0
                {
                    return Err(RunFailure::DivisionByZero(operator_span).into());
                }
                Scalar::I32(
                    match op {
                        hir::ArithmeticOp::Add => a.checked_add(b),
                        hir::ArithmeticOp::Subtract => a.checked_sub(b),
                        hir::ArithmeticOp::Multiply => a.checked_mul(b),
                        hir::ArithmeticOp::Divide => a.checked_div(b),
                        hir::ArithmeticOp::Remainder => a.checked_rem(b),
                    }
                    .ok_or(RunFailure::Overflow(operator_span))?,
                )
                .into()
            }
            Rvalue::CheckedI64 {
                op,
                left,
                right,
                operator_span,
            } => {
                let (Some(a), Some(b)) = (
                    self.read(frame, left)?.wide(),
                    self.read(frame, right)?.wide(),
                ) else {
                    return Err(bad("i64 arithmetic type", assign.span));
                };
                if matches!(op, hir::ArithmeticOp::Divide | hir::ArithmeticOp::Remainder) && b == 0
                {
                    return Err(RunFailure::WideDivisionByZero(operator_span).into());
                }
                let value = match op {
                    hir::ArithmeticOp::Add => a.checked_add(b),
                    hir::ArithmeticOp::Subtract => a.checked_sub(b),
                    hir::ArithmeticOp::Multiply => a.checked_mul(b),
                    hir::ArithmeticOp::Divide => a.checked_div(b),
                    hir::ArithmeticOp::Remainder => a.checked_rem(b),
                }
                .ok_or(RunFailure::WideOverflow(operator_span))?;
                values::checked_wide(value, assign.span)?
            }
            Rvalue::CompareScalar {
                op, left, right, ..
            } => {
                let a = self.read(frame, left)?;
                let b = self.read(frame, right)?;
                Scalar::Bool(values::comparison(a, b, op, assign.span)?).into()
            }
        };
        self.write(frame, assign.destination, value, assign.span)
    }
    fn read_stdin(
        &mut self,
        frame: usize,
        buffer: ReferenceParamId,
        destination: OwnerPlaceId,
        span: Span,
    ) -> Result<()> {
        let function = self.function(self.frames[frame].function);
        if self.plan.witness().builtin_function() != Some(function.id)
            || buffer != ReferenceParamId(0)
            || destination != OwnerPlaceId(0)
        {
            return Err(bad("input builtin identity", span));
        }
        if !input::supported_host() {
            return Err(OwnedRunFailure::InputHost(span));
        }
        function
            .references
            .get(buffer.0)
            .filter(|reference| {
                reference.kind == BorrowKind::Exclusive
                    && reference.referent() == BorrowedTy::ScalarSlice(hir::Ty::I32)
            })
            .ok_or_else(|| bad("input reference type", span))?;
        // Revalidate the active loan, epochs, exclusive authority, nominal
        // projection provenance and complete view before deriving its capacity.
        let (root, array, relative) =
            self.array_base(frame, AccessBase::Parameter(buffer), Access::Write, span)?;
        if matches!(self.aggregate(root, span)?, AggregateTy::Enum(_)) {
            return Err(bad("input buffer type", span));
        }
        if array.element() != hir::Ty::I32 || array.length() > 1024 {
            return Err(bad("input capacity", span));
        }
        let capacity = array.length();
        let root_frame = index(root.frame, span)?;
        if root_frame >= frame {
            return Err(bad("input buffer activation", span));
        }
        let buffer_extent = self.owner_extent(root, span)?;
        let buffer_start = buffer_extent
            .start
            .checked_add(relative)
            .ok_or_else(|| bad("input buffer offset", span))?;
        let buffer_end = capacity
            .checked_mul(4)
            .and_then(|bytes| buffer_start.checked_add(bytes))
            .filter(|end| *end <= buffer_extent.end)
            .ok_or_else(|| bad("input buffer extent", span))?;

        self.expect_owner(frame, destination, &[UNINITIALIZED], span)?;
        let result_key = self.raw_key(frame, destination);
        let enumeration = self
            .plan
            .witness()
            .builtin_enumeration()
            .ok_or_else(|| bad("input result identity", span))?;
        if result_key.generation == 0
            || function.owners[destination.0].kind != OwnerKind::Temporary
            || self.aggregate(result_key, span)? != AggregateTy::Enum(enumeration)
        {
            return Err(bad("input result owner", span));
        }
        let generation = epoch(result_key.generation, span)?;
        let variants = self
            .plan
            .witness()
            .declarations()
            .enums()
            .variants(enumeration)
            .map_err(|_| bad("input result variants", span))?;
        if variants.len() != 3
            || variants[0].payload() != Some(hir::Ty::I32)
            || variants[1].payload().is_some()
            || variants[2].payload().is_some()
        {
            return Err(bad("input result variants", span));
        }
        let result_extent = self.owner_extent(result_key, span)?;
        let tag_offset = scalar_offset(result_extent.clone(), 0, hir::Ty::I32, span)?;
        let count_offset = scalar_offset(result_extent.clone(), 4, hir::Ty::I32, span)?;

        // All result construction and maximum prefix stores are prepaid. The
        // ordinary activation has already admitted and allocated its scratch.
        self.charge(4 + capacity, span)?;
        let scratch = self
            .plan
            .input_scratch_range(function.id)
            .filter(|scratch| {
                scratch.len() == 1024
                    && scratch.end == self.frames[frame].payload.len()
                    && result_extent.end <= scratch.start
            })
            .ok_or_else(|| bad("input scratch extent", span))?;

        let mut staged = 0usize;
        let mut tag = 1u32; // Full, including the zero-capacity case.
        while staged < capacity {
            self.charge(1, span)?;
            match input::read_one() {
                input::Attempt::Byte(byte) => {
                    self.frames[frame].payload[scratch.start + staged] = byte;
                    staged += 1;
                }
                input::Attempt::Eof => {
                    tag = 0;
                    break;
                }
                input::Attempt::Interrupted => {}
                input::Attempt::Error => {
                    tag = 2;
                    break;
                }
            }
        }

        // The canonical builtin cannot invoke or mutate ownership while reading.
        // Every range/epoch was checked above. From the first destination store
        // onward there is no fallible helper, allocation, validation or fuel debit.
        let (ancestors, current) = self.frames.split_at_mut(frame);
        let current = &mut current[0];
        if tag != 2 {
            let prefix = &current.payload[scratch.start..scratch.start + staged];
            let destination_bytes = &mut ancestors[root_frame].payload[buffer_start..buffer_end];
            for (cell, byte) in destination_bytes
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(prefix)
            {
                cell.copy_from_slice(&i32::from(*byte).to_le_bytes());
            }
        }
        current.payload[tag_offset..tag_offset + 4].copy_from_slice(&tag.to_le_bytes());
        if tag == 0 {
            current.payload[count_offset..count_offset + 4]
                .copy_from_slice(&(staged as i32).to_le_bytes());
        }
        let result = &mut current.owners[destination.0];
        result.generation = generation;
        result.state = AVAILABLE;
        Ok(())
    }
    fn write_stdout(
        &mut self,
        frame: usize,
        buffer: ReferenceParamId,
        destination: OwnerPlaceId,
        span: Span,
    ) -> Result<()> {
        let active = self
            .frames
            .get(frame)
            .ok_or_else(|| bad("output activation", span))?;
        let function = self
            .plan
            .witness()
            .functions()
            .get(active.function.0)
            .filter(|function| function.id == active.function)
            .ok_or_else(|| bad("output function identity", span))?;
        if self.plan.witness().builtin_output_function() != Some(function.id)
            || buffer != ReferenceParamId(0)
            || destination != OwnerPlaceId(0)
        {
            return Err(bad("output builtin identity", span));
        }
        if !output::supported_host() {
            return Err(OwnedRunFailure::OutputHost(span));
        }
        function
            .references
            .get(buffer.0)
            .filter(|reference| {
                reference.kind == BorrowKind::Shared
                    && reference.referent() == BorrowedTy::ScalarSlice(hir::Ty::I32)
            })
            .ok_or_else(|| bad("output reference type", span))?;
        // Revalidate live owner/loan epochs, readable authority, nominal
        // projection provenance and the complete view before reading any cell.
        let (root, array, relative) =
            self.array_base(frame, AccessBase::Parameter(buffer), Access::Read, span)?;
        if matches!(self.aggregate(root, span)?, AggregateTy::Enum(_)) {
            return Err(bad("output buffer type", span));
        }
        if array.element() != hir::Ty::I32 || array.length() > 1024 {
            return Err(bad("output capacity", span));
        }
        let capacity = array.length();
        let root_frame = index(root.frame, span)?;
        if root_frame >= frame {
            return Err(bad("output buffer activation", span));
        }
        let buffer_extent = self.owner_extent(root, span)?;
        let buffer_start = buffer_extent
            .start
            .checked_add(relative)
            .ok_or_else(|| bad("output buffer offset", span))?;
        let buffer_end = capacity
            .checked_mul(4)
            .and_then(|bytes| buffer_start.checked_add(bytes))
            .filter(|end| *end <= buffer_extent.end)
            .ok_or_else(|| bad("output buffer extent", span))?;

        self.expect_owner(frame, destination, &[UNINITIALIZED], span)?;
        let result_key = self.raw_key(frame, destination);
        let enumeration = self
            .plan
            .witness()
            .builtin_output_enumeration()
            .ok_or_else(|| bad("output result identity", span))?;
        if result_key.generation == 0
            || function.owners[destination.0].kind != OwnerKind::Temporary
            || self.aggregate(result_key, span)? != AggregateTy::Enum(enumeration)
        {
            return Err(bad("output result owner", span));
        }
        let generation = epoch(result_key.generation, span)?;
        let variants = self
            .plan
            .witness()
            .declarations()
            .enums()
            .variants(enumeration)
            .map_err(|_| bad("output result variants", span))?;
        if variants.len() != 3
            || variants[0].payload().is_some()
            || variants[1].payload().is_some()
            || variants[2].payload() != Some(hir::Ty::I32)
        {
            return Err(bad("output result variants", span));
        }
        let result_extent = self.owner_extent(result_key, span)?;
        let tag_offset = scalar_offset(result_extent.clone(), 0, hir::Ty::I32, span)?;
        let count_offset = scalar_offset(result_extent.clone(), 4, hir::Ty::I32, span)?;
        let scratch = self
            .plan
            .output_scratch_range(function.id)
            .filter(|scratch| {
                scratch.len() == 1024
                    && scratch.end == self.frames[frame].payload.len()
                    && result_extent.end <= scratch.start
            })
            .ok_or_else(|| bad("output scratch extent", span))?;

        // Result materialization and the entire scan/stage are prepaid. The
        // activation already admitted its fixed scratch allocation. No source
        // byte is read until all runtime authority and storage checks succeed.
        self.charge(4 + capacity, span)?;
        let valid = {
            let (ancestors, current) = self.frames.split_at_mut(frame);
            stage_stdout(
                &ancestors[root_frame].payload[buffer_start..buffer_end],
                &mut current[0].payload[scratch.start..scratch.start + capacity],
            )
        };
        let mut tag = if valid { 0u32 } else { 1u32 };
        let mut accepted = 0usize;
        // An invalid last cell cannot leak the earlier staged prefix. Empty
        // views also skip fd 1 entirely. Each attempt, including EINTR, pays
        // one immediately before the single unbuffered syscall.
        while valid && accepted < capacity {
            self.charge(1, span)?;
            let byte = self.frames[frame].payload[scratch.start + accepted];
            match output::write_one(byte) {
                output::Attempt::Accepted => accepted += 1,
                output::Attempt::Interrupted => {}
                output::Attempt::Error => {
                    tag = 2;
                    break;
                }
            }
        }

        // All offsets, authority and next generation were checked before the
        // first write. The builtin cannot invoke or mutate ownership mid-call.
        // Materialization is infallible and allocation/fuel-free. Nullary
        // variants never inspect or initialize their inactive payload bytes.
        let current = &mut self.frames[frame];
        current.payload[tag_offset..tag_offset + 4].copy_from_slice(&tag.to_le_bytes());
        if tag == 2 {
            current.payload[count_offset..count_offset + 4]
                .copy_from_slice(&(accepted as i32).to_le_bytes());
        }
        let result = &mut current.owners[destination.0];
        result.generation = generation;
        result.state = AVAILABLE;
        Ok(())
    }
    fn statement(
        &mut self,
        frame: usize,
        instruction: &OwnedInstruction,
        span: Span,
    ) -> Result<()> {
        let f = self.function(self.frames[frame].function);
        match instruction {
            OwnedInstruction::Experimental {
                operation,
                destination,
            } => self.experimental(frame, *operation, *destination, span)?,
            OwnedInstruction::OwnedVector { .. } => {
                return Err(bad(
                    "owned vector operation requires a complete consumer",
                    span,
                ));
            }
            OwnedInstruction::Vector { .. } => {
                return Err(bad("vector operation requires prepared delivery", span));
            }
            OwnedInstruction::ReadStdin {
                buffer,
                destination,
            } => {
                self.read_stdin(frame, *buffer, *destination, span)?;
            }
            OwnedInstruction::WriteStdout {
                buffer,
                destination,
            } => {
                self.write_stdout(frame, *buffer, *destination, span)?;
            }
            OwnedInstruction::ConstructOwnedEnum {
                destination,
                variant,
                payload,
            } => {
                self.expect_owner(frame, *destination, &[UNINITIALIZED], span)?;
                self.expect_owner(frame, *payload, &[AVAILABLE], span)?;
                let from = self.base(frame, AccessBase::Owner(*payload), Access::Consume, span)?;
                let to = self.raw_key(frame, *destination);
                let aggregate = self.aggregate(from, span)?;
                let source = self.owner_extent(from, span)?;
                let destination_extent =
                    self.owned_variant_extent(to, *variant, aggregate, span)?;
                if source.start < destination_extent.end && destination_extent.start < source.end {
                    return Err(bad("overlapping whole transfer", span));
                }
                let tag_offset =
                    scalar_offset(self.owner_extent(to, span)?, 0, hir::Ty::I32, span)?;
                self.preflight_active::<false>(
                    from,
                    source.clone(),
                    aggregate,
                    destination_extent.clone(),
                    span,
                )?;
                #[cfg(test)]
                let observation =
                    self.observe_begin(to, StorageObservationKind::Construction, true, span);
                self.copy_active(from, source, to, destination_extent, aggregate, span)?;
                self.frames[frame].payload[tag_offset..tag_offset + 4]
                    .copy_from_slice(&(variant.index as u32).to_le_bytes());
                self.change_owner(frame, *payload, MOVED, span)?;
                self.change_owner(frame, *destination, AVAILABLE, span)?;
                #[cfg(test)]
                self.observe_end(observation);
            }
            OwnedInstruction::ConsumeOwnedVariant {
                match_id,
                arm,
                destination,
            } => {
                let descriptor = f
                    .matches
                    .get(match_id.0)
                    .ok_or_else(|| bad("enum match", span))?;
                let selected = descriptor
                    .arms
                    .get(*arm)
                    .ok_or_else(|| bad("enum arm", span))?;
                let from = self.base(
                    frame,
                    AccessBase::Owner(descriptor.source),
                    Access::Consume,
                    span,
                )?;
                self.expect_owner(frame, descriptor.source, &[AVAILABLE], span)?;
                self.expect_owner(frame, *destination, &[DEAD], span)?;
                let destination_generation = epoch(
                    epoch(self.frames[frame].owners[destination.0].generation, span)?,
                    span,
                )?;
                let to = self.raw_key(frame, *destination);
                let aggregate = self.aggregate(to, span)?;
                let (variant, _) = self.enum_tag(from, span)?;
                if variant != selected.variant {
                    return Err(bad("enum tag", span));
                }
                let source = self.owned_variant_extent(from, selected.variant, aggregate, span)?;
                let destination_extent = self.owner_extent(to, span)?;
                if source.start < destination_extent.end && destination_extent.start < source.end {
                    return Err(bad("overlapping whole transfer", span));
                }
                self.preflight_active::<false>(
                    from,
                    source.clone(),
                    aggregate,
                    destination_extent.clone(),
                    span,
                )?;
                #[cfg(test)]
                let observation =
                    self.observe_begin(to, StorageObservationKind::Transfer, true, span);
                self.copy_active(from, source, to, destination_extent, aggregate, span)?;
                self.change_owner(frame, descriptor.source, MOVED, span)?;
                self.frames[frame].owners[destination.0].generation = destination_generation;
                self.frames[frame].owners[destination.0].state = AVAILABLE;
                #[cfg(test)]
                {
                    self.record_event(Event::Transfer(
                        from,
                        OwnerKey {
                            generation: destination_generation,
                            ..to
                        },
                    ));
                    self.observe_end(observation);
                }
            }
            OwnedInstruction::ConstructEnum {
                destination,
                variant,
                payload,
            } => {
                self.expect_owner(frame, *destination, &[UNINITIALIZED], span)?;
                let key = self.raw_key(frame, *destination);
                let AggregateTy::Enum(enumeration) = self.aggregate(key, span)? else {
                    return Err(bad("enum type", span));
                };
                let payload = payload
                    .map(|operand| self.read(frame, operand))
                    .transpose()?;
                let declarations = self.plan.witness().declarations().enums();
                declarations
                    .check_payload(enumeration, *variant, payload.map(StoredValue::ty))
                    .map_err(|_| bad("enum construction type", span))?;
                let variant = declarations
                    .variant(enumeration, *variant)
                    .map_err(|_| bad("enum tag", span))?;
                let value = StoredEnumValue {
                    tag: variant.tag(),
                    payload,
                    payload_offset: variant.payload_offset().unwrap_or(0) as u8,
                };
                let extent = self.owner_extent(key, span)?;
                enum_offsets(extent.clone(), value, span)?;
                #[cfg(test)]
                let observation =
                    self.observe_begin(key, StorageObservationKind::Construction, true, span);
                encode_enum(&mut self.frames[frame].payload, extent, value, span)?;
                self.change_owner(frame, *destination, AVAILABLE, span)?;
                #[cfg(test)]
                self.observe_end(observation);
            }
            OwnedInstruction::ConsumeVariant {
                match_id,
                arm,
                destination,
            } => {
                let descriptor = f
                    .matches
                    .get(match_id.0)
                    .ok_or_else(|| bad("enum match", span))?;
                let selected = descriptor
                    .arms
                    .get(*arm)
                    .ok_or_else(|| bad("enum arm", span))?;
                let key = self.base(
                    frame,
                    AccessBase::Owner(descriptor.source),
                    Access::Consume,
                    span,
                )?;
                self.expect_owner(frame, descriptor.source, &[AVAILABLE], span)?;
                let value = self.load_enum(key, Some(selected.variant), span)?;
                self.preflight_enum_binding(frame, *destination, value, span)?;
                // Every operation that can fail has completed. Binding and the
                // already-preflighted owner transition form one paid consumption.
                if let (Some(destination), Some(payload)) = (*destination, value.payload) {
                    self.frames[frame].slots[destination.0] = Some(payload);
                }
                self.change_owner(frame, descriptor.source, MOVED, span)?;
                #[cfg(test)]
                self.record_event(Event::EnumBind(key, *destination, value.payload));
            }
            OwnedInstruction::DestructureRecord { source, fields } => {
                let from = self.base(frame, AccessBase::Owner(*source), Access::Consume, span)?;
                self.expect_owner(frame, *source, &[AVAILABLE], span)?;
                let record = record_type(self.aggregate(from, span)?, span)?;
                let root_extent = self.owner_extent(from, span)?;
                // Every destination, type, active resource and byte range is
                // checked before any transfer. The verifier proves coverage
                // of all owned fields and unique fresh destinations.
                for (field, destination) in fields {
                    self.expect_owner(frame, *destination, &[UNINITIALIZED], span)?;
                    let to = self.raw_key(frame, *destination);
                    let (aggregate, source_extent) =
                        self.owned_record_field_extent(from, record, *field, span)?;
                    let destination_extent = self.owner_extent(to, span)?;
                    self.plan
                        .witness()
                        .same_aggregate_type(aggregate, self.aggregate(to, span)?)
                        .map_err(|_| bad("record pattern destination type", span))?;
                    if root_extent.start < destination_extent.end
                        && destination_extent.start < root_extent.end
                    {
                        return Err(bad("overlapping record pattern transfer", span));
                    }
                    self.preflight_active::<false>(
                        from,
                        source_extent,
                        aggregate,
                        destination_extent,
                        span,
                    )?;
                }
                // Keep destinations uninitialized until all bytes have moved.
                // An unexpected copy error therefore leaves cleanup authority
                // solely with the complete source record.
                for (field, destination) in fields {
                    let to = self.raw_key(frame, *destination);
                    let (aggregate, source_extent) =
                        self.owned_record_field_extent(from, record, *field, span)?;
                    self.copy_active(
                        from,
                        source_extent,
                        to,
                        self.owner_extent(to, span)?,
                        aggregate,
                        span,
                    )?;
                }
                self.change_owner(frame, *source, MOVED, span)?;
                for (_, destination) in fields {
                    self.change_owner(frame, *destination, AVAILABLE, span)?;
                }
            }
            OwnedInstruction::ConstructComposite {
                destination,
                fields,
            } => {
                self.expect_owner(frame, *destination, &[UNINITIALIZED], span)?;
                let key = self.raw_key(frame, *destination);
                let aggregate = self.aggregate(key, span)?;
                let record = record_type(aggregate, span)?;
                let declarations = self.plan.witness().declarations();
                if declarations
                    .fields(record)
                    .map_err(|_| bad("construction record", span))?
                    .len()
                    != fields.len()
                {
                    return Err(bad("construction field count", span));
                }
                let destination_extent = self.owner_extent(key, span)?;
                // Complete source snapshots, authority, type, range, and transition
                // checks all precede the first payload write or child consumption.
                // Static field/source uniqueness is established by the sealed verifier.
                for (field, initializer) in fields {
                    let field = declarations
                        .field(record, *field)
                        .map_err(|_| bad("construction field", span))?;
                    match (field.value_ty(), initializer) {
                        (ValueTy::Scalar(ty), FieldInitializer::Scalar(operand)) => {
                            if self.read(frame, *operand)?.ty() != ty {
                                return Err(bad("construction type", span));
                            }
                            self.leaf_offset(key, field.offset(), ty, span)?;
                        }
                        (ValueTy::Owned(expected), FieldInitializer::Owned(source)) => {
                            let source_decl = f
                                .owners
                                .get(source.0)
                                .filter(|owner| owner.kind == OwnerKind::Temporary)
                                .ok_or_else(|| bad("construction owned stage", span))?;
                            self.plan
                                .witness()
                                .same_aggregate_type(source_decl.aggregate(), expected)
                                .map_err(|_| bad("construction owned type", span))?;
                            let from = self.base(
                                frame,
                                AccessBase::Owner(*source),
                                Access::Consume,
                                span,
                            )?;
                            self.expect_owner(frame, *source, &[AVAILABLE], span)?;
                            let source_extent = self.owner_extent(from, span)?;
                            if source_extent.start < destination_extent.end
                                && destination_extent.start < source_extent.end
                            {
                                return Err(bad("overlapping composite transfer", span));
                            }
                            if self.active_aggregate(expected)
                                || matches!(expected, AggregateTy::Enum(_))
                            {
                                let start = destination_extent
                                    .start
                                    .checked_add(field.offset())
                                    .ok_or_else(|| bad("construction resource extent", span))?;
                                let end = start
                                    .checked_add(
                                        self.plan
                                            .witness()
                                            .aggregate_layout(expected)
                                            .map_err(|_| bad("construction resource layout", span))?
                                            .size(),
                                    )
                                    .filter(|end| *end <= destination_extent.end)
                                    .ok_or_else(|| bad("construction resource extent", span))?;
                                self.preflight_active::<false>(
                                    from,
                                    source_extent,
                                    expected,
                                    start..end,
                                    span,
                                )?;
                                continue;
                            }
                            for leaf in declarations
                                .leaves(expected)
                                .map_err(|_| bad("construction leaves", span))?
                            {
                                self.load_leaf(from, leaf, span)?;
                                let relative = field
                                    .offset()
                                    .checked_add(leaf.offset)
                                    .ok_or_else(|| bad("construction leaf offset", span))?;
                                self.leaf_offset(key, relative, leaf.ty, span)?;
                            }
                        }
                        _ => return Err(bad("construction initializer type", span)),
                    }
                }
                #[cfg(test)]
                let observation =
                    self.observe_begin(key, StorageObservationKind::Construction, true, span);
                if fields.is_empty() {
                    self.store_leaf(
                        key,
                        ScalarLeaf {
                            offset: 0,
                            ty: hir::Ty::Unit,
                        },
                        Scalar::Unit.into(),
                        span,
                    )?;
                }
                for (id, initializer) in fields {
                    let field = declarations
                        .field(record, *id)
                        .map_err(|_| bad("construction field", span))?;
                    match initializer {
                        FieldInitializer::Scalar(operand) => {
                            let value = self.read(frame, *operand)?;
                            self.store_field(key, *id, value, span)?;
                        }
                        FieldInitializer::Owned(source) => {
                            let from = self.owner_key(frame, *source, span)?;
                            let aggregate = self.aggregate(from, span)?;
                            if self.active_aggregate(aggregate)
                                || matches!(aggregate, AggregateTy::Enum(_))
                            {
                                let start = destination_extent.start + field.offset();
                                let end = start
                                    + self
                                        .plan
                                        .witness()
                                        .aggregate_layout(aggregate)
                                        .map_err(|_| bad("construction resource layout", span))?
                                        .size();
                                self.copy_active(
                                    from,
                                    self.owner_extent(from, span)?,
                                    key,
                                    start..end,
                                    aggregate,
                                    span,
                                )?;
                                continue;
                            }
                            for leaf in declarations
                                .leaves(aggregate)
                                .map_err(|_| bad("construction leaves", span))?
                            {
                                let value = self.load_leaf(from, leaf, span)?;
                                let relative = field
                                    .offset()
                                    .checked_add(leaf.offset)
                                    .ok_or_else(|| bad("construction leaf offset", span))?;
                                self.store_leaf(
                                    key,
                                    ScalarLeaf {
                                        offset: relative,
                                        ty: leaf.ty,
                                    },
                                    value,
                                    span,
                                )?;
                            }
                        }
                    }
                }
                for (_, initializer) in fields {
                    if let FieldInitializer::Owned(source) = initializer {
                        self.change_owner(frame, *source, MOVED, span)?;
                    }
                }
                self.change_owner(frame, *destination, AVAILABLE, span)?;
                #[cfg(test)]
                self.observe_end(observation);
            }
            OwnedInstruction::ReadProjection {
                destination,
                base,
                path,
                index,
            } => {
                let (key, ty, relative) =
                    self.projection_base(frame, *base, Access::Read, path, span)?;
                if let (ValueTy::Owned(AggregateTy::Vector(element)), Some(index)) = (ty, *index) {
                    return self.vector_projection_read(
                        frame,
                        *destination,
                        key,
                        element,
                        relative,
                        index,
                        span,
                    );
                }
                let (leaf, _ordinal) = self.projection_leaf(frame, ty, relative, *index, span)?;
                if f.locals.get(destination.0).map(|local| local.ty) != Some(leaf.ty) {
                    return Err(bad("projection read type", span));
                }
                let value = self.load_leaf(key, leaf, span)?;
                self.write(frame, *destination, value, span)?;
                #[cfg(test)]
                if let Some(ordinal) = _ordinal {
                    self.record_event(Event::ReadIndex(key, ordinal, value));
                }
            }
            OwnedInstruction::WriteProjection {
                base,
                path,
                index,
                value,
            } => {
                let (key, ty, relative) =
                    self.projection_base(frame, *base, Access::Write, path, span)?;
                if let (ValueTy::Owned(AggregateTy::Vector(element)), Some(index)) = (ty, *index) {
                    return self.vector_projection_write(
                        frame, key, element, relative, index, *value, span,
                    );
                }
                let value = self.read(frame, *value)?;
                let expected = match (ty, index) {
                    (ValueTy::Scalar(ty), None) => ty,
                    (ValueTy::Owned(AggregateTy::FixedArray(array)), Some(_)) => array.element(),
                    _ => return Err(bad("projection write leaf type", span)),
                };
                if value.ty() != expected {
                    return Err(bad("projection write type", span));
                }
                let (leaf, _ordinal) = self.projection_leaf(frame, ty, relative, *index, span)?;
                self.store_leaf(key, leaf, value, span)?;
                #[cfg(test)]
                if let Some(ordinal) = _ordinal {
                    self.record_event(Event::WriteIndex(key, ordinal, value));
                    let observation =
                        self.observe_begin(key, StorageObservationKind::IndexWrite, false, span);
                    self.observe_end(observation);
                } else if let Some(field) = path.last() {
                    self.record_event(Event::WriteField(key, *field, value));
                }
            }
            OwnedInstruction::ProjectionLength {
                destination,
                base,
                path,
            } => {
                let (_key, ty, relative) =
                    self.projection_base(frame, *base, Access::Read, path, span)?;
                if let ValueTy::Owned(aggregate) = ty {
                    if let Some(element) = super::vector_type::VectorTy::from_aggregate(aggregate) {
                        return self.vector_projection_length(
                            frame,
                            *destination,
                            _key,
                            element,
                            relative,
                            span,
                        );
                    }
                }
                let ValueTy::Owned(AggregateTy::FixedArray(array)) = ty else {
                    return Err(bad("projection length type", span));
                };
                self.write(
                    frame,
                    *destination,
                    Scalar::I32(array.length() as i32),
                    span,
                )?;
                #[cfg(test)]
                self.record_event(Event::ArrayLength(_key, array.length()));
            }
            OwnedInstruction::ConstructArray {
                destination,
                elements,
            } => {
                self.expect_owner(frame, *destination, &[UNINITIALIZED], span)?;
                let key = self.raw_key(frame, *destination);
                let array = self.array_type(key, span)?;
                if elements.len() != array.length() {
                    return Err(bad("array construction length", span));
                }
                // Check every immutable snapshot before any payload write. No element scratch.
                for operand in elements {
                    if self.read(frame, *operand)?.ty() != array.element() {
                        return Err(bad("array construction type", span));
                    }
                }
                #[cfg(test)]
                let observation =
                    self.observe_begin(key, StorageObservationKind::Construction, true, span);
                if array.length() == 0 {
                    self.zero_sentinel(key, span)?;
                }
                for (ordinal, operand) in elements.iter().enumerate() {
                    let value = self.read(frame, *operand)?;
                    self.store_element(key, array, ordinal, value, span)?;
                }
                self.change_owner(frame, *destination, AVAILABLE, span)?;
                #[cfg(test)]
                self.observe_end(observation);
            }
            OwnedInstruction::CopyIndexInitialize {
                destination,
                base,
                path,
                index,
            } => {
                self.record_vector_index_read(frame, *destination, *base, path, *index, span)?;
            }
            OwnedInstruction::CopyIndexWrite {
                base,
                path,
                index,
                source,
            } => {
                self.record_vector_index_write(frame, *base, path, *index, *source, span)?;
            }
            OwnedInstruction::ReadIndex {
                destination,
                base,
                index,
            } => {
                if self.vector_view(frame, *base) {
                    return self.vector_index_read(frame, *destination, *base, *index, span);
                }
                if self.dynamic_byte_view(frame, *base) {
                    return self.dynamic_index_read(frame, *destination, *base, *index, span);
                }
                let (key, array, relative) = self.array_base(frame, *base, Access::Read, span)?;
                if f.locals.get(destination.0).map(|local| local.ty) != Some(array.element()) {
                    return Err(bad("array read type", span));
                }
                let ordinal = array_index(self.read(frame, *index)?, array, span)?;
                let value = self.load_leaf(
                    key,
                    ScalarLeaf {
                        offset: relative
                            .checked_add(
                                ordinal
                                    .checked_mul(array.stride())
                                    .ok_or_else(|| bad("array view offset", span))?,
                            )
                            .ok_or_else(|| bad("array view offset", span))?,
                        ty: array.element(),
                    },
                    span,
                )?;
                self.write(frame, *destination, value, span)?;
                #[cfg(test)]
                self.record_event(Event::ReadIndex(key, ordinal, value));
            }
            OwnedInstruction::WriteIndex { base, index, value } => {
                if self.vector_view(frame, *base) {
                    return self.vector_index_write(frame, *base, *index, *value, span);
                }
                if self.dynamic_byte_view(frame, *base) {
                    return self.dynamic_index_write(frame, *base, *index, *value, span);
                }
                let (key, array, relative) = self.array_base(frame, *base, Access::Write, span)?;
                let value = self.read(frame, *value)?;
                if value.ty() != array.element() {
                    return Err(bad("array write type", span));
                }
                let ordinal = array_index(self.read(frame, *index)?, array, span)?;
                self.store_leaf(
                    key,
                    ScalarLeaf {
                        offset: relative
                            .checked_add(
                                ordinal
                                    .checked_mul(array.stride())
                                    .ok_or_else(|| bad("array view offset", span))?,
                            )
                            .ok_or_else(|| bad("array view offset", span))?,
                        ty: array.element(),
                    },
                    value,
                    span,
                )?;
                #[cfg(test)]
                {
                    self.record_event(Event::WriteIndex(key, ordinal, value));
                    let observation =
                        self.observe_begin(key, StorageObservationKind::IndexWrite, false, span);
                    self.observe_end(observation);
                }
            }
            OwnedInstruction::ArrayLength { destination, base } => {
                if self.vector_view(frame, *base) {
                    return self.vector_length(frame, *destination, *base, span);
                }
                if self.dynamic_byte_view(frame, *base) {
                    return self.dynamic_length(frame, *destination, *base, span);
                }
                let (_key, array, _relative) = self.array_base(frame, *base, Access::Read, span)?;
                self.write(
                    frame,
                    *destination,
                    Scalar::I32(array.length() as i32),
                    span,
                )?;
                #[cfg(test)]
                self.record_event(Event::ArrayLength(_key, array.length()));
            }
            OwnedInstruction::Scalar(s) => self.scalar_statement(frame, s)?,
            OwnedInstruction::StorageLive(o) => {
                self.expect_owner(frame, *o, &[DEAD], span)?;
                self.change_owner(frame, *o, UNINITIALIZED, span)?;
            }
            OwnedInstruction::StorageEnd(o) => {
                self.expect_owner(frame, *o, &[UNINITIALIZED, AVAILABLE, MOVED], span)?;
                if self.frames[frame].owners[o.0].state == AVAILABLE {
                    self.release_resources(self.raw_key(frame, *o), span)?;
                }
                self.change_owner(frame, *o, DEAD, span)?;
            }
            OwnedInstruction::Discard(o) => {
                let key = self.base(frame, AccessBase::Owner(*o), Access::Consume, span)?;
                self.expect_owner(frame, *o, &[AVAILABLE], span)?;
                let aggregate = self.aggregate(key, span)?;
                if self.active_aggregate(aggregate) {
                    let extent = self.owner_extent(key, span)?;
                    self.preflight_active::<true>(key, extent.clone(), aggregate, extent, span)?;
                } else if matches!(aggregate, AggregateTy::Enum(_)) {
                    self.load_enum(key, None, span)?;
                }
                self.release_resources(key, span)?;
                self.change_owner(frame, *o, MOVED, span)?;
            }
            OwnedInstruction::Construct {
                destination,
                fields,
            } => {
                self.expect_owner(frame, *destination, &[UNINITIALIZED], span)?;
                // All operand checks precede any payload writes. Their fixed-factor reread is bounded by w.
                for (field, operand) in fields {
                    let v = self.read(frame, *operand)?;
                    let d = self
                        .plan
                        .witness()
                        .declarations()
                        .field(
                            record_type(f.owners[destination.0].aggregate(), span)?,
                            *field,
                        )
                        .map_err(|_| bad("construction field", span))?;
                    if d.value_ty() != ValueTy::Scalar(v.ty()) {
                        return Err(bad("construction type", span));
                    }
                }
                let key = self.raw_key(frame, *destination);
                if fields.is_empty() {
                    let offset = self.plan.function(f.id).owner_offset(*destination);
                    self.frames[frame].payload[offset] = 0;
                }
                for (field, operand) in fields {
                    let v = self.read(frame, *operand)?;
                    self.store_field(key, *field, v, span)?;
                }
                self.change_owner(frame, *destination, AVAILABLE, span)?;
            }
            OwnedInstruction::CopyInitialize {
                destination,
                source,
            }
            | OwnedInstruction::MoveInitialize {
                destination,
                source,
            }
            | OwnedInstruction::Replace {
                destination,
                source,
            } => {
                let copying = matches!(instruction, OwnedInstruction::CopyInitialize { .. });
                let from = self.base(
                    frame,
                    AccessBase::Owner(*source),
                    if copying {
                        Access::Read
                    } else {
                        Access::Consume
                    },
                    span,
                )?;
                // A copy is a read: shared child loans and the source epoch
                // remain valid. base() already proves the source is available.
                if !copying {
                    self.expect_owner(frame, *source, &[AVAILABLE], span)?;
                }
                if copying
                    && !self
                        .plan
                        .witness()
                        .declarations()
                        .is_copy(self.aggregate(from, span)?)
                {
                    return Err(bad("copy requires a checked Copy record", span));
                }
                let replacing = matches!(instruction, OwnedInstruction::Replace { .. });
                self.expect_owner(
                    frame,
                    *destination,
                    if replacing {
                        &[AVAILABLE, MOVED]
                    } else {
                        &[UNINITIALIZED]
                    },
                    span,
                )?;
                if source == destination {
                    return Err(bad("overlapping whole transfer", span));
                }
                let to = self.raw_key(frame, *destination);
                let mut admitted_replacement = None;
                if replacing && self.frames[frame].owners[destination.0].state == AVAILABLE {
                    let aggregate = self.aggregate(from, span)?;
                    if self
                        .plan
                        .witness()
                        .contains_resources(aggregate)
                        .map_err(|_| bad("resource type", span))?
                    {
                        let source_extent = self.owner_extent(from, span)?;
                        let destination_extent = self.owner_extent(to, span)?;
                        if self.plan.uses_owned_vectors() {
                            self.plan
                                .witness()
                                .same_aggregate_type(aggregate, self.aggregate(to, span)?)
                                .map_err(|_| bad("nominal replacement", span))?;
                        }
                        self.preflight_active::<false>(
                            from,
                            source_extent.clone(),
                            aggregate,
                            destination_extent.clone(),
                            span,
                        )?;
                        if self.plan.uses_owned_vectors() {
                            admitted_replacement =
                                Some((source_extent, aggregate, destination_extent));
                        }
                        self.release_resources(to, span)?;
                    }
                }
                if let Some((source_extent, aggregate, destination_extent)) = admitted_replacement {
                    self.copy_prevalidated_transfer(
                        from,
                        source_extent,
                        to,
                        destination_extent,
                        aggregate,
                        span,
                    )?;
                } else {
                    self.transfer_payload(from, to, span)?;
                }
                if !copying {
                    self.change_owner(frame, *source, MOVED, span)?;
                }
                self.change_owner(frame, *destination, AVAILABLE, span)?;
            }
            OwnedInstruction::ReadField {
                destination,
                base,
                field,
            } => {
                let key = self.projected_base(
                    frame,
                    *base,
                    Access::Read,
                    span,
                    ProjectionRequest::Field {
                        base: *base,
                        field: *field,
                        write: false,
                    },
                )?;
                let value = self.load_field(frame, *base, key, *field, span)?;
                self.write(frame, *destination, value, span)?;
            }
            OwnedInstruction::WriteField { base, field, value } => {
                let key = self.projected_base(
                    frame,
                    *base,
                    Access::Write,
                    span,
                    ProjectionRequest::Field {
                        base: *base,
                        field: *field,
                        write: true,
                    },
                )?;
                let value = self.read(frame, *value)?;
                let (offset, ty) = self.accessed_field(frame, *base, key, *field, span)?;
                if ty != value.ty() {
                    return Err(bad("field value type", span));
                }
                value.encode(&mut self.frames[key.frame as usize].payload, offset, span)?;
                #[cfg(test)]
                self.record_event(Event::WriteField(key, *field, value));
            }
            OwnedInstruction::OpenCall(call) => {
                if self.frames[frame].calls[call.0].phase != CLOSED {
                    return Err(bad("call already open", span));
                }
                for &owner in self.plan.function(f.id).owned_stages(*call) {
                    self.expect_owner(frame, owner, &[DEAD], span)?;
                }
                for &owner in self.plan.function(f.id).owned_stages(*call) {
                    self.change_owner(frame, owner, UNINITIALIZED, span)?;
                }
                self.frames[frame].calls[call.0] = CallRuntime {
                    phase: PREPARING,
                    next_argument: 0,
                };
            }
            OwnedInstruction::PrepareScalar {
                call,
                argument,
                value,
            } => {
                let index = self.prepared(frame, *call, *argument, span)?;
                let value = self.read(frame, *value)?;
                self.frames[frame].snapshots[index] = Some(value);
                self.frames[frame].calls[call.0].next_argument += 1;
            }
            OwnedInstruction::PrepareOwned {
                call,
                argument,
                source,
            } => {
                self.prepared(frame, *call, *argument, span)?;
                let ArgumentSlot::Owned(destination) = f.calls[call.0].arguments[*argument] else {
                    return Err(bad("owned argument class", span));
                };
                let from = self.base(frame, AccessBase::Owner(*source), Access::Consume, span)?;
                self.expect_owner(frame, *source, &[AVAILABLE], span)?;
                self.expect_owner(frame, destination, &[UNINITIALIZED], span)?;
                self.transfer_payload(from, self.raw_key(frame, destination), span)?;
                self.change_owner(frame, *source, MOVED, span)?;
                self.change_owner(frame, destination, AVAILABLE, span)?;
                self.frames[frame].calls[call.0].next_argument += 1;
            }
            OwnedInstruction::AcquireLocalBorrow(loan) => self.acquire(frame, *loan, span)?,
            OwnedInstruction::ReleaseLocalBorrow(loan) => {
                let handle = self.reference_from_loan(frame, *loan, span)?;
                self.release(handle.permission, span)?;
            }
            OwnedInstruction::PrepareBorrowRange {
                call,
                argument,
                loan,
                start,
                end,
            } => {
                self.prepared(frame, *call, *argument, span)?;
                let (Some(Scalar::I32(start)), Some(Scalar::I32(end))) = (
                    self.read(frame, *start)?.narrow(),
                    self.read(frame, *end)?.narrow(),
                ) else {
                    return Err(bad("byte range endpoint type", span));
                };
                self.acquire_range(frame, *loan, Some((start, end)), span)?;
                self.frames[frame].calls[call.0].next_argument += 1;
            }
            OwnedInstruction::PrepareBorrow {
                call,
                argument,
                loan,
            } => {
                self.prepared(frame, *call, *argument, span)?;
                self.acquire(frame, *loan, span)?;
                self.frames[frame].calls[call.0].next_argument += 1;
            }
        }
        Ok(())
    }
    fn branch(&mut self, frame: usize, target: BlockId) {
        let f = &mut self.frames[frame];
        f.predecessor = Some(f.block);
        f.block = target;
        f.next = 0;
        f.merge_pending = true;
    }
    fn dispatch(
        &mut self,
        frame: usize,
        call: CallSiteId,
        continuation: BlockId,
        span: Span,
    ) -> Result<()> {
        let f = self.function(self.frames[frame].function);
        let c = &f.calls[call.0];
        let callee = self.function(c.target);
        let state = self.frames[frame].calls[call.0];
        if state.phase != PREPARING || state.next_argument as usize != c.arguments.len() {
            return Err(bad("incomplete invocation", span));
        }
        let cp = self.plan.function(f.id).call(call);
        for &owner in self.plan.function(f.id).owned_stages(call) {
            self.expect_owner(frame, owner, &[AVAILABLE], span)?;
        }
        // No transient argument Vec: fixed scalar snapshots and loan records are the inputs.
        // Separate fields can share an owner. Composed views are re-derived
        // from immutable paths below before any child frame is installed.
        for (i, &loan) in self
            .plan
            .function(f.id)
            .borrowed_loans(call)
            .iter()
            .enumerate()
        {
            let a = self.handle(frame, loan, span)?;
            for &other in &self.plan.function(f.id).borrowed_loans(call)[..i] {
                let b = self.handle(frame, other, span)?;
                if a.root == b.root
                    && (self.loan_kind(a.permission, span)? == BorrowKind::Exclusive
                        || self.loan_kind(b.permission, span)? == BorrowKind::Exclusive)
                    && self.reference_extents_overlap(a, b, span)?
                {
                    return Err(bad("incoming reference alias contract", span));
                }
            }
        }
        // Allocation is after fuel/frame/slot/cell/byte preflight in the dispatcher loop.
        let mut child = ValueFrame::<V>::allocate(
            self.plan,
            c.target,
            self.next_activation,
            Some(Resume {
                call,
                continuation,
                origin: span,
            }),
        )
        .map_err(|error| error.at_runtime(span))?;
        for (position, (argument, parameter)) in
            c.arguments.iter().zip(&callee.parameters).enumerate()
        {
            match (argument, parameter) {
                (ArgumentSlot::Scalar, ParameterBinding::Scalar(local)) => {
                    let value = self.frames[frame].snapshots[cp.argument_start() + position]
                        .ok_or_else(|| bad("missing scalar snapshot", span))?;
                    if callee.locals[local.0].ty != value.ty() {
                        return Err(bad("incoming scalar type", span));
                    }
                    child.slots[local.0] = Some(value);
                }
                (ArgumentSlot::Borrow(loan), ParameterBinding::Reference(reference)) => {
                    let handle = self.handle(frame, *loan, span)?;
                    let declared = &callee.references[reference.0];
                    let actual_view = self
                        .checked_handle_view(handle, span)
                        .map_err(|_| bad("incoming reference type", span))?;
                    if self
                        .plan
                        .witness()
                        .check_borrowed_view(BorrowedTy::Exact(actual_view), declared.referent())
                        .is_err()
                        || self.loan_kind(handle.permission, span)? != declared.kind
                    {
                        return Err(bad("incoming reference type", span));
                    }
                    child.references[reference.0] = handle;
                }
                (ArgumentSlot::Owned(source), ParameterBinding::Owned(destination)) => {
                    let key = self.owner_key(frame, *source, span)?;
                    let record = callee.owners[destination.0].aggregate();
                    self.plan
                        .witness()
                        .same_aggregate_type(record, f.owners[source.0].aggregate())
                        .map_err(|_| bad("incoming owned type", span))?;
                    let offset = self.plan.function(callee.id).owner_offset(*destination);
                    let declarations = self.plan.witness().declarations();
                    let size = self
                        .plan
                        .witness()
                        .aggregate_layout(record)
                        .map_err(|_| bad("incoming aggregate layout", span))?
                        .size();
                    let end = offset
                        .checked_add(size)
                        .filter(|end| *end <= child.payload.len())
                        .ok_or_else(|| bad("incoming aggregate range", span))?;
                    let owned_enum = self.active_aggregate(record);
                    let active = if owned_enum {
                        self.preflight_active::<false>(
                            key,
                            self.owner_extent(key, span)?,
                            record,
                            offset..end,
                            callee.span,
                        )?;
                        let owner = child
                            .owners
                            .get(destination.0)
                            .ok_or_else(|| bad("incoming owner", span))?;
                        if owner.state != DEAD || owner.generation != 0 {
                            return Err(bad("incoming owner state", span));
                        }
                        epoch(owner.generation, span)?;
                        None
                    } else if matches!(record, AggregateTy::Enum(_)) {
                        let value = self.load_enum(key, None, callee.span)?;
                        enum_offsets(offset..end, value, span)?;
                        let owner = child
                            .owners
                            .get(destination.0)
                            .ok_or_else(|| bad("incoming owner", span))?;
                        if owner.state != DEAD || owner.generation != 0 {
                            return Err(bad("incoming owner state", span));
                        }
                        epoch(owner.generation, span)?;
                        Some(value)
                    } else {
                        for leaf in declarations
                            .leaves(record)
                            .map_err(|_| bad("incoming leaves", span))?
                        {
                            self.load_leaf(key, leaf, span)?;
                            scalar_offset(offset..end, leaf.offset, leaf.ty, span)?;
                        }
                        None
                    };
                    #[cfg(test)]
                    let observation =
                        if matches!(record, AggregateTy::FixedArray(_) | AggregateTy::Enum(_)) {
                            self.observer.begin(
                                &mut child.payload,
                                offset..end,
                                OwnerKey {
                                    frame: self.frames.len() as u64,
                                    activation: child.activation,
                                    owner: destination.0 as u64,
                                    generation: 1,
                                },
                                AVAILABLE,
                                StorageObservationKind::Incoming,
                                true,
                            )
                        } else {
                            None
                        };
                    if owned_enum {
                        let source_extent = self.owner_extent(key, span)?;
                        copy_active_bytes_with_witness::<V, W>(
                            self.plan.witness(),
                            record,
                            &self.frames[frame].payload[source_extent],
                            &mut child.payload[offset..end],
                            span,
                        )?;
                    } else if let Some(value) = active {
                        encode_enum(&mut child.payload, offset..end, value, span)?;
                    } else {
                        for leaf in declarations
                            .leaves(record)
                            .map_err(|_| bad("incoming leaves", span))?
                        {
                            let value = self.load_leaf(key, leaf, span)?;
                            let leaf_offset =
                                scalar_offset(offset..end, leaf.offset, leaf.ty, span)?;
                            value.encode(&mut child.payload, leaf_offset, span)?;
                        }
                    }
                    #[cfg(test)]
                    self.observer.end(&child.payload, observation);
                    #[cfg(test)]
                    self.record_event(Event::Transfer(
                        key,
                        OwnerKey {
                            frame: self.frames.len() as u64,
                            activation: child.activation,
                            owner: destination.0 as u64,
                            generation: 1,
                        },
                    ));
                    child.owners[destination.0] = OwnerRuntime {
                        generation: 1,
                        state: AVAILABLE,
                        shared_children: 0,
                        exclusive_children: 0,
                    };
                }
                _ => return Err(bad("incoming argument class", span)),
            }
        }
        for &owner in self.plan.function(f.id).owned_stages(call) {
            self.change_owner(frame, owner, DEAD, span)?;
        }
        self.frames[frame].calls[call.0].phase = IN_FLIGHT;
        self.install(child);
        Ok(())
    }
    fn return_value(
        &mut self,
        frame: usize,
        kind: &OwnedTerminatorKind,
        span: Span,
    ) -> Result<Option<V>> {
        let f = self.function(self.frames[frame].function);
        // The fixed P+L+C return charge covers every owner/outgoing-region check.
        if self.frames[frame].loans.iter().any(|l| l.state != 0)
            || self.frames[frame].calls.iter().any(|c| c.phase != CLOSED)
            || self.frames[frame]
                .owners
                .iter()
                .any(|o| o.shared_children != 0 || o.exclusive_children != 0)
        {
            return Err(bad("active region at return", span));
        }
        let scalar = match kind {
            OwnedTerminatorKind::ReturnScalar(v) => Some(self.read(frame, *v)?),
            _ => None,
        };
        let owned = match kind {
            OwnedTerminatorKind::ReturnOwned(o) => {
                self.expect_owner(frame, *o, &[AVAILABLE], span)?;
                Some(self.owner_key(frame, *o, span)?)
            }
            _ => None,
        };
        let resume = self.frames[frame].return_to;
        if let Some(resume) = resume {
            let caller = frame
                .checked_sub(1)
                .ok_or_else(|| bad("missing caller", span))?;
            let parent = self.function(self.frames[caller].function);
            let c = &parent.calls[resume.call.0];
            if self.frames[caller].calls[resume.call.0].phase != IN_FLIGHT {
                return Err(bad("normal-edge call state", span));
            }
            for &loan in self.plan.function(parent.id).borrowed_loans(resume.call) {
                self.release_preflight(self.handle(caller, loan, span)?.permission, span)?;
            }
            match (c.result, scalar, owned) {
                (CallResult::Scalar(destination), Some(value), None) => {
                    if parent.locals[destination.0].ty != value.ty() {
                        return Err(bad("normal-edge scalar type", span));
                    }
                    // All legal scalar destinations are nonparameters, verified before execution.
                    self.write(caller, destination, value, resume.origin)?;
                }
                (CallResult::Owned(destination), None, Some(from)) => {
                    self.expect_owner(caller, destination, &[DEAD], span)?;
                    let to = self.raw_key(caller, destination);
                    self.transfer_payload(from, to, span)?;
                    self.change_owner(frame, OwnerPlaceId(from.owner as usize), MOVED, span)?;
                    self.change_owner(caller, destination, AVAILABLE, span)?;
                }
                _ => return Err(bad("normal-edge result class", span)),
            }
            // Distinct active loan records and increment/decrement induction guarantee their
            // authority counts, including shared siblings. Arbitrarily corrupted tables are
            // outside this invariant; checked release still rejects direct stale/child-active keys.
            for &loan in self.plan.function(parent.id).borrowed_loans(resume.call) {
                let key = self.handle(caller, loan, span)?.permission;
                self.release(key, span)?;
            }
            self.frames[caller].calls[resume.call.0] = CallRuntime::default();
            self.branch(caller, resume.continuation);
        } else if frame != 0 || scalar.is_none() || !matches!(f.result, ValueTy::Scalar(_)) {
            return Err(bad("entry return", span));
        }
        #[cfg(test)]
        self.record_event(Event::Return(f.id));
        #[cfg(test)]
        self.observe_frame(frame, StorageObservationKind::Return, span);
        // The result has transferred before ordinary frame cleanup. StorageEnd
        // handles lexical exits; this also releases live parameters and stages.
        for owner in (0..f.owners.len()).rev() {
            if self.frames[frame].owners[owner].state == AVAILABLE {
                self.release_resources(self.raw_key(frame, OwnerPlaceId(owner)), span)?;
            }
        }
        let u = self.plan.function(f.id).usage();
        self.frames.pop();
        self.live_slots -= u.scalar_slots;
        self.live_cells -= u.expanded_cells;
        self.live_bytes -= u.reference_bytes;
        if resume.is_none() {
            Ok(scalar)
        } else {
            Ok(None)
        }
    }
    fn execute(&mut self) -> Result<V> {
        loop {
            let frame = self
                .frames
                .len()
                .checked_sub(1)
                .ok_or(OwnedRunFailure::Invariant("empty machine", None))?;
            // Dynamic identities are checked before any canonical plan accessor.
            let function_id = self.frames[frame].function;
            let f = self
                .plan
                .witness()
                .functions()
                .get(function_id.0)
                .filter(|f| f.id == function_id)
                .ok_or(OwnedRunFailure::Invariant(
                    "dynamic function identity",
                    None,
                ))?;
            let b = f
                .blocks
                .get(self.frames[frame].block.0)
                .ok_or_else(|| bad("dynamic block identity", f.span))?;
            if self.frames[frame].merge_pending {
                if let Some(merge) = &b.merge {
                    self.charge(1, merge.span)?;
                    let input = merge
                        .incoming
                        .iter()
                        .find(|i| Some(i.predecessor) == self.frames[frame].predecessor)
                        .ok_or_else(|| bad("merge predecessor", merge.span))?;
                    let value = self.read(frame, input.value)?;
                    self.write(frame, merge.destination, value, merge.span)?;
                }
                self.frames[frame].merge_pending = false;
            }
            if let Some(statement) = b.statements.get(self.frames[frame].next) {
                let span = plan::instruction_span(statement);
                if !matches!(
                    statement.kind,
                    OwnedInstruction::ReadStdin { .. }
                        | OwnedInstruction::WriteStdout { .. }
                        | OwnedInstruction::OwnedVector { .. }
                ) {
                    self.charge(self.plan.statement_cost(f.id, &statement.kind), span)?;
                }
                #[cfg(test)]
                self.inject_statement(frame, &statement.kind, span);
                if let OwnedInstruction::Experimental {
                    operation,
                    destination,
                } = statement.kind
                {
                    use crate::frontend::builtin_catalog::experimental::Function;
                    if operation.is_file() {
                        self.file_delivery(frame, operation, destination, span)?;
                        continue;
                    }
                    if matches!(
                        operation,
                        Function::ReadSome | Function::WriteSome | Function::ArgBytes
                    ) {
                        self.binary_delivery(frame, operation, destination, span)?;
                        // The canonical frame has completed its prepaid return.
                        continue;
                    }
                }
                if let OwnedInstruction::Vector {
                    operation,
                    destination,
                } = statement.kind
                {
                    self.vector_delivery(frame, operation, destination, span)?;
                    continue;
                }
                if let OwnedInstruction::OwnedVector {
                    operation,
                    destination,
                } = statement.kind
                {
                    self.owned_vector_delivery(frame, operation, destination, span)?;
                    continue;
                }
                self.statement(frame, &statement.kind, span)?;
                self.frames[frame].next += 1;
                continue;
            }
            if self.frames[frame].next != b.statements.len() {
                return Err(bad("instruction cursor", b.span));
            }
            let end = b
                .terminator
                .as_ref()
                .ok_or_else(|| bad("missing terminator", b.span))?;
            let cost = self.plan.terminator_cost(f.id, &end.kind);
            if let OwnedTerminatorKind::Invoke { call, continuation } = end.kind {
                self.activation_preflight(f.calls[call.0].target, cost, end.span)?;
                #[cfg(test)]
                self.inject_invoke(frame, call, end.span);
                self.dispatch(frame, call, continuation, end.span)?;
                continue;
            }
            self.charge(cost, end.span)?;
            match end.kind {
                OwnedTerminatorKind::MatchDispatch { match_id, arm } => {
                    #[cfg(test)]
                    self.inject_enum_dispatch(frame, match_id, arm, end.span);
                    let descriptor = f
                        .matches
                        .get(match_id.0)
                        .ok_or_else(|| bad("enum match", end.span))?;
                    let selected = descriptor
                        .arms
                        .get(arm)
                        .ok_or_else(|| bad("enum arm", end.span))?;
                    let key = self.base(
                        frame,
                        AccessBase::Owner(descriptor.source),
                        Access::Read,
                        end.span,
                    )?;
                    let (variant, _) = self.enum_tag(key, end.span)?;
                    let target = if variant == selected.variant {
                        selected.entry
                    } else {
                        descriptor
                            .arms
                            .get(arm + 1)
                            .map(|next| next.dispatch)
                            .ok_or_else(|| bad("enum tag", end.span))?
                    };
                    self.branch(frame, target);
                }
                OwnedTerminatorKind::Branch {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let Some(Scalar::Bool(value)) = self.read(frame, condition)?.narrow() else {
                        return Err(bad("branch bool", condition.span));
                    };
                    self.branch(frame, if value { then_block } else { else_block });
                }
                OwnedTerminatorKind::Goto(target) => self.branch(frame, target),
                OwnedTerminatorKind::ReturnScalar(_) | OwnedTerminatorKind::ReturnOwned(_) => {
                    #[cfg(test)]
                    self.inject_enum_return(frame, &end.kind, end.span);
                    if let Some(value) = self.return_value(frame, &end.kind, end.span)? {
                        return Ok(value);
                    }
                }
                OwnedTerminatorKind::Invoke { .. } => unreachable!(),
            }
        }
    }
}
/// An enum has one active containment chain. Once that chain reaches a record
/// or fixed array, the checked static leaf iterator handles its remaining shape.
/// This retains no expanded value and never reads padding or inactive arms.
type ActiveCells<'a> = ValueCells<'a, Scalar>;
struct ValueCells<'a, V> {
    _value: std::marker::PhantomData<V>,
    declarations: &'a Declarations,
    bytes: &'a [u8],
    extent: std::ops::Range<usize>,
    pending: Option<(ValueTy, usize)>,
    leaves: Option<ScalarLeaves<'a>>,
    leaf_base: usize,
    span: Span,
}
impl<'a, V: StoredValue> ValueCells<'a, V> {
    fn new(
        declarations: &'a Declarations,
        bytes: &'a [u8],
        extent: std::ops::Range<usize>,
        aggregate: AggregateTy,
        span: Span,
    ) -> Self {
        Self {
            _value: std::marker::PhantomData,
            declarations,
            bytes,
            extent,
            pending: Some((ValueTy::Owned(aggregate), 0)),
            leaves: None,
            leaf_base: 0,
            span,
        }
    }
    fn read(&self, leaf: ScalarLeaf) -> Result<(ScalarLeaf, V)> {
        let offset = scalar_offset(self.extent.clone(), leaf.offset, leaf.ty, self.span)?;
        Ok((
            leaf,
            V::decode(self.bytes, offset, leaf.ty, self.span)
                .map_err(|_| bad("enum payload", self.span))?,
        ))
    }
}
impl<V: StoredValue> Iterator for ValueCells<'_, V> {
    type Item = Result<(ScalarLeaf, V)>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(leaves) = &mut self.leaves {
            let mut leaf = leaves.next()?;
            leaf.offset = match self.leaf_base.checked_add(leaf.offset) {
                Some(offset) => offset,
                None => return Some(Err(bad("enum payload range", self.span))),
            };
            return Some(self.read(leaf));
        }
        let (value, relative) = self.pending.take()?;
        match value {
            ValueTy::Scalar(ty) => Some(self.read(ScalarLeaf {
                offset: relative,
                ty,
            })),
            // Resource identities are never scalar cells. A separate checked
            // traversal validates and transfers their allocation obligations.
            ValueTy::Owned(
                AggregateTy::ByteBuf
                | AggregateTy::File
                | AggregateTy::Vector(_)
                | AggregateTy::RecordVector(_)
                | AggregateTy::OwnedRecordVector(_),
            ) => None,
            ValueTy::Owned(AggregateTy::Enum(enumeration)) => {
                let result = (|| {
                    let leaf = ScalarLeaf {
                        offset: relative,
                        ty: hir::Ty::I32,
                    };
                    let offset =
                        scalar_offset(self.extent.clone(), relative, hir::Ty::I32, self.span)?;
                    let Some(Scalar::I32(tag)) =
                        V::decode(self.bytes, offset, hir::Ty::I32, self.span)?.narrow()
                    else {
                        unreachable!()
                    };
                    let variant = self
                        .declarations
                        .enums()
                        .variant_for_tag(enumeration, tag as u32 as usize)
                        .map_err(|_| bad("enum tag", self.span))?;
                    if let Some(payload) = variant.value_payload() {
                        let offset = relative
                            .checked_add(
                                variant
                                    .payload_offset()
                                    .ok_or_else(|| bad("enum payload offset", self.span))?,
                            )
                            .ok_or_else(|| bad("enum payload range", self.span))?;
                        self.pending = Some((payload, offset));
                    }
                    Ok((leaf, V::from(Scalar::I32(tag))))
                })();
                Some(result)
            }
            ValueTy::Owned(aggregate) => match self.declarations.leaves(aggregate) {
                Ok(leaves) => {
                    self.leaves = Some(leaves);
                    self.leaf_base = relative;
                    self.next()
                }
                Err(_) => Some(Err(bad("enum payload leaves", self.span))),
            },
        }
    }
}
fn copy_active_bytes(
    declarations: &Declarations,
    aggregate: AggregateTy,
    source: &[u8],
    destination: &mut [u8],
    span: Span,
) -> Result<()> {
    copy_active_bytes_for::<Scalar, _>(declarations, aggregate, source, destination, span)
}

fn copy_active_bytes_with_witness<V: StoredValue, W: PlanWitness>(
    witness: &W,
    aggregate: AggregateTy,
    source: &[u8],
    destination: &mut [u8],
    span: Span,
) -> Result<()> {
    if witness.uses_owned_vectors() {
        copy_active_bytes_for::<V, _>(
            resources::RuntimeResourceScope::maintenance(witness),
            aggregate,
            source,
            destination,
            span,
        )
    } else {
        copy_active_bytes_for::<V, _>(witness.declarations(), aggregate, source, destination, span)
    }
}

fn copy_active_bytes_for<'a, V: StoredValue, Q: resources::ResourceQueries<'a>>(
    queries: Q,
    aggregate: AggregateTy,
    source: &[u8],
    destination: &mut [u8],
    span: Span,
) -> Result<()> {
    let declarations = queries.declarations();
    if !matches!(aggregate, AggregateTy::OwnedRecordVector(_))
        && declarations
            .has_enum_record_nesting(aggregate)
            .map_err(|_| bad("composed aggregate identity", span))?
    {
        for cell in
            composition::Cells::<V>::new(declarations, source, 0..source.len(), aggregate, span)
        {
            let (leaf, value) = cell?;
            let offset = scalar_offset(0..destination.len(), leaf.offset, leaf.ty, span)?;
            value.encode(destination, offset, span)?;
        }
    } else {
        for cell in ValueCells::<V>::new(declarations, source, 0..source.len(), aggregate, span) {
            let (leaf, value) = cell?;
            let offset = scalar_offset(0..destination.len(), leaf.offset, leaf.ty, span)?;
            value.encode(destination, offset, span)?;
        }
    }
    resources::copy(queries, aggregate, source, destination, span)?;
    Ok(())
}

/// Validate every destination byte range before the first write. The snapshot
/// contains no inactive bytes and encoding cannot inspect them.
fn enum_offsets<V: StoredValue>(
    extent: std::ops::Range<usize>,
    value: StoredEnumValue<V>,
    span: Span,
) -> Result<(usize, Option<usize>)> {
    let tag = scalar_offset(extent.clone(), 0, hir::Ty::I32, span)?;
    let payload = value
        .payload
        .map(|payload| {
            scalar_offset(
                extent,
                usize::from(value.payload_offset),
                payload.ty(),
                span,
            )
        })
        .transpose()?;
    Ok((tag, payload))
}
fn encode_enum<V: StoredValue>(
    bytes: &mut [u8],
    extent: std::ops::Range<usize>,
    value: StoredEnumValue<V>,
    span: Span,
) -> Result<()> {
    if extent.end > bytes.len() {
        return Err(bad("enum storage", span));
    }
    let (tag, payload_offset) = enum_offsets(extent, value, span)?;
    bytes[tag..tag + 4].copy_from_slice(&value.tag.to_le_bytes());
    if let (Some(offset), Some(payload)) = (payload_offset, value.payload) {
        payload.encode(bytes, offset, span)?;
    }
    Ok(())
}

fn scalar_size(ty: hir::Ty) -> usize {
    match ty {
        hir::Ty::I16 | hir::Ty::U16 => 2,
        hir::Ty::Char | hir::Ty::F32 | hir::Ty::I32 | hir::Ty::U32 => 4,
        hir::Ty::F64 | hir::Ty::I64 | hir::Ty::U64 | hir::Ty::Isize | hir::Ty::Usize => 8,
        hir::Ty::I128 | hir::Ty::U128 => 16,
        hir::Ty::Bool | hir::Ty::U8 | hir::Ty::I8 | hir::Ty::Unit => 1,
    }
}
fn scalar_offset(
    extent: std::ops::Range<usize>,
    relative: usize,
    ty: hir::Ty,
    span: Span,
) -> Result<usize> {
    let offset = extent
        .start
        .checked_add(relative)
        .ok_or_else(|| bad("leaf offset overflow", span))?;
    offset
        .checked_add(scalar_size(ty))
        .filter(|end| *end <= extent.end)
        .ok_or_else(|| bad("leaf payload range", span))?;
    Ok(offset)
}
fn array_index<V: StoredValue>(value: V, array: FixedArrayTy, span: Span) -> Result<usize> {
    let Some(Scalar::I32(index)) = value.narrow() else {
        return Err(bad("array index type", span));
    };
    if index < 0 || index >= array.length() as i32 {
        return Err(OwnedRunFailure::Bounds(span));
    }
    usize::try_from(index).map_err(|_| bad("array index conversion", span))
}
fn decode(bytes: &[u8], offset: usize, ty: hir::Ty, span: Span) -> Result<Scalar> {
    Ok(match ty {
        hir::Ty::I64
        | hir::Ty::I8
        | hir::Ty::I16
        | hir::Ty::U16
        | hir::Ty::U32
        | hir::Ty::U64
        | hir::Ty::Isize
        | hir::Ty::Usize
        | hir::Ty::I128
        | hir::Ty::Char
        | hir::Ty::F32
        | hir::Ty::F64
        | hir::Ty::U128 => return Err(bad("wide payload in narrow execution", span)),
        hir::Ty::U8 => Scalar::U8(
            *bytes
                .get(offset)
                .ok_or_else(|| bad("payload range", span))?,
        ),
        hir::Ty::I32 => Scalar::I32(i32::from_le_bytes(
            bytes
                .get(
                    offset
                        ..offset
                            .checked_add(4)
                            .ok_or_else(|| bad("payload range", span))?,
                )
                .ok_or_else(|| bad("payload range", span))?
                .try_into()
                .unwrap(),
        )),
        hir::Ty::Bool => match bytes.get(offset) {
            Some(0) => Scalar::Bool(false),
            Some(1) => Scalar::Bool(true),
            _ => return Err(bad("bool payload", span)),
        },
        hir::Ty::Unit => {
            if bytes.get(offset) != Some(&0) {
                return Err(bad("unit payload", span));
            }
            Scalar::Unit
        }
    })
}
fn encode(bytes: &mut [u8], offset: usize, value: Scalar, span: Span) -> Result<()> {
    match value {
        Scalar::U8(v) => {
            *bytes
                .get_mut(offset)
                .ok_or_else(|| bad("payload range", span))? = v
        }
        Scalar::I32(v) => bytes
            .get_mut(
                offset
                    ..offset
                        .checked_add(4)
                        .ok_or_else(|| bad("payload range", span))?,
            )
            .ok_or_else(|| bad("payload range", span))?
            .copy_from_slice(&v.to_le_bytes()),
        Scalar::Bool(v) => {
            *bytes
                .get_mut(offset)
                .ok_or_else(|| bad("payload range", span))? = u8::from(v)
        }
        Scalar::Unit => {
            *bytes
                .get_mut(offset)
                .ok_or_else(|| bad("payload range", span))? = 0
        }
    }
    Ok(())
}
pub(super) fn run<W: PlanWitness>(witness: &W, entry: Option<hir::DefId>) -> Result<Scalar> {
    run_limits(witness, entry, Limits::default())
}

pub(super) fn run_result<W: PlanWitness>(
    witness: &W,
    entry: Option<hir::DefId>,
    limits: Limits,
    execution: ExecutionPolicy,
) -> Result<RuntimeValue> {
    let entry = checked_entry_policy(witness, entry, EntryPolicy::Result)?;
    let plan = ExecutionPlan::build(witness)?;
    execute_result_plan(&plan, entry, limits, EntryPolicy::Result, execution)
}

fn execute_result_plan<W: PlanWitness>(
    plan: &ExecutionPlan<'_, W>,
    entry: hir::DefId,
    limits: Limits,
    policy: EntryPolicy,
    execution: ExecutionPolicy,
) -> Result<RuntimeValue> {
    if plan.uses_quad_scalars() {
        execute_plan_policy::<RuntimeValue, W>(
            plan,
            entry,
            limits,
            policy,
            execution,
            #[cfg(test)]
            None,
            #[cfg(test)]
            None,
            #[cfg(test)]
            None,
        )
    } else {
        execute_value_plan(plan, entry, limits, policy, execution).map(RuntimeValue::from)
    }
}

pub(super) fn run_value<W: PlanWitness>(
    witness: &W,
    entry: Option<hir::DefId>,
    limits: Limits,
    execution: ExecutionPolicy,
) -> Result<RuntimeScalar> {
    let entry = checked_entry_policy(witness, entry, EntryPolicy::Result)?;
    let plan = ExecutionPlan::build(witness)?;
    if plan.uses_quad_scalars() {
        return Err(RunFailure::QuadEntry(witness.functions()[entry.0].span).into());
    }
    execute_value_plan(&plan, entry, limits, EntryPolicy::Result, execution)
}

fn execute_value_plan<W: PlanWitness>(
    plan: &ExecutionPlan<'_, W>,
    entry: hir::DefId,
    limits: Limits,
    policy: EntryPolicy,
    execution: ExecutionPolicy,
) -> Result<RuntimeScalar> {
    if plan.uses_wide_scalars() {
        execute_plan_policy::<RuntimeScalar, W>(
            plan,
            entry,
            limits,
            policy,
            execution,
            #[cfg(test)]
            None,
            #[cfg(test)]
            None,
            #[cfg(test)]
            None,
        )
    } else {
        execute_plan_policy::<Scalar, W>(
            plan,
            entry,
            limits,
            policy,
            execution,
            #[cfg(test)]
            None,
            #[cfg(test)]
            None,
            #[cfg(test)]
            None,
        )
        .map(RuntimeScalar::from)
    }
}
pub(super) fn run_with_execution<W: PlanWitness>(
    witness: &W,
    entry: Option<hir::DefId>,
    execution: ExecutionPolicy,
) -> Result<Scalar> {
    if !execution.hosted() {
        return run(witness, entry);
    }
    let entry = checked_entry(witness, entry)?;
    let plan = ExecutionPlan::build(witness)?;
    execute_plan_policy(
        &plan,
        entry,
        Limits::default(),
        EntryPolicy::Result,
        execution,
        #[cfg(test)]
        None,
        #[cfg(test)]
        None,
        #[cfg(test)]
        None,
    )
}
pub(super) fn run_limits<W: PlanWitness>(
    witness: &W,
    entry: Option<hir::DefId>,
    limits: Limits,
) -> Result<Scalar> {
    let entry = checked_entry(witness, entry)?;
    let plan = ExecutionPlan::build(witness)?;
    execute_plan(
        &plan,
        entry,
        limits,
        #[cfg(test)]
        None,
    )
}
fn checked_entry<W: PlanWitness>(witness: &W, entry: Option<hir::DefId>) -> Result<hir::DefId> {
    let entry = checked_entry_policy(witness, entry, EntryPolicy::Result)?;
    let function = &witness.functions()[entry.0];
    if function.result == ValueTy::Scalar(hir::Ty::U8) {
        return Err(RunFailure::ByteEntry(function.span).into());
    }
    Ok(entry)
}
fn checked_entry_policy<W: PlanWitness>(
    witness: &W,
    entry: Option<hir::DefId>,
    policy: EntryPolicy,
) -> Result<hir::DefId> {
    for operation in crate::frontend::builtin_catalog::experimental::Function::ALL {
        if (operation.is_file() || operation.is_arguments())
            && witness.experimental_function(operation).is_some()
            && !input::supported_host()
        {
            return Err(OwnedRunFailure::BinaryHost(
                witness.functions()[witness.experimental_function(operation).unwrap().0].span,
            ));
        }
    }
    for operation in [
        crate::frontend::builtin_catalog::experimental::Function::ReadSome,
        crate::frontend::builtin_catalog::experimental::Function::WriteSome,
    ] {
        if let Some(function) = witness.experimental_function(operation) {
            let span = witness.functions()[function.0].span;
            if policy == EntryPolicy::Result {
                return Err(OwnedRunFailure::BinaryEntry(span));
            }
            if !input::supported_host() {
                return Err(OwnedRunFailure::BinaryHost(span));
            }
        }
    }
    if policy == EntryPolicy::Result {
        if let Some(function) = witness.builtin_output_function() {
            // The ordinary result-mode route must deny even an unused output
            // function before planning, activation, source effects or fuel debits.
            return Err(OwnedRunFailure::OutputEntry(
                witness.functions()[function.0].span,
            ));
        }
    }
    if !output::supported_host() {
        if let Some(function) = witness.builtin_output_function() {
            return Err(OwnedRunFailure::OutputHost(
                witness.functions()[function.0].span,
            ));
        }
        if policy == EntryPolicy::Process {
            return Err(OwnedRunFailure::ProcessHost);
        }
    }
    if !input::supported_host() {
        if let Some(function) = witness.builtin_function() {
            return Err(OwnedRunFailure::InputHost(
                witness.functions()[function.0].span,
            ));
        }
    }
    let entry = entry.ok_or(RunFailure::Entry(None))?;
    let f = witness
        .functions()
        .get(entry.0)
        .filter(|f| f.id == entry)
        .ok_or(OwnedRunFailure::Invariant("entry identity", None))?;
    if !f.parameters.is_empty() {
        return Err(RunFailure::Entry(Some(f.span)).into());
    }
    if policy == EntryPolicy::Process && f.result != ValueTy::Scalar(hir::Ty::I32) {
        return Err(OwnedRunFailure::ProcessEntry(f.span));
    }
    if matches!(f.result, ValueTy::Owned(_)) {
        return Err(OwnedRunFailure::EntryResult(f.span));
    }
    Ok(entry)
}
fn execute_plan<W: PlanWitness>(
    plan: &ExecutionPlan<'_, W>,
    entry: hir::DefId,
    limits: Limits,
    #[cfg(test)] events: Option<&mut Vec<Event>>,
) -> Result<Scalar> {
    execute_plan_inner(
        plan,
        entry,
        limits,
        EntryPolicy::Result,
        #[cfg(test)]
        events,
        #[cfg(test)]
        None,
        #[cfg(test)]
        None,
    )
}
fn execute_plan_inner<W: PlanWitness>(
    plan: &ExecutionPlan<'_, W>,
    entry: hir::DefId,
    limits: Limits,
    policy: EntryPolicy,
    #[cfg(test)] events: Option<&mut Vec<Event>>,
    #[cfg(test)] observation: Option<&mut array_observe::Observer>,
    #[cfg(test)] remaining_fuel: Option<&mut usize>,
) -> Result<Scalar> {
    execute_plan_policy(
        plan,
        entry,
        limits,
        policy,
        ExecutionPolicy::Bounded,
        #[cfg(test)]
        events,
        #[cfg(test)]
        observation,
        #[cfg(test)]
        remaining_fuel,
    )
}

#[allow(clippy::too_many_arguments)] // Execution policy plus unchanged private observation sinks.
fn execute_plan_policy<V: StoredValue, W: PlanWitness>(
    plan: &ExecutionPlan<'_, W>,
    entry: hir::DefId,
    limits: Limits,
    policy: EntryPolicy,
    execution: ExecutionPolicy,
    #[cfg(test)] events: Option<&mut Vec<Event<V>>>,
    #[cfg(test)] mut observation: Option<&mut array_observe::Observer>,
    #[cfg(test)] remaining_fuel: Option<&mut usize>,
) -> Result<V> {
    // All callers share entry validation. Every sealed Process caller
    // establishes host support and signal policy before reaching this point.
    checked_entry_policy(plan.witness(), Some(entry), policy)?;
    if plan.has_file_delivery() && !process::setup() {
        return Err(OwnedRunFailure::BinaryHost(
            plan.witness().functions()[entry.0].span,
        ));
    }
    if (plan.uses_wide_scalars() || plan.uses_quad_scalars()) != V::WIDE
        || plan.uses_quad_scalars() != V::QUAD
    {
        return Err(bad(
            "scalar storage representation",
            plan.witness().functions()[entry.0].span,
        ));
    }
    let limits = limits.bounded();
    let f = &plan.witness().functions()[entry.0];
    let mut machine = ValueMachine::<V, W> {
        plan,
        frames: Vec::new(),
        limits,
        fuel: execution.initial_fuel(limits.fuel),
        next_activation: 1,
        live_slots: 0,
        live_cells: 0,
        live_bytes: 0,
        header_bytes: plan::add(
            plan::mul(limits.frames, size_of::<ValueFrame<V>>())?,
            plan::add(
                if plan.uses_dynamic_resources() {
                    plan::add(
                        plan::add(size_of::<byte_buffer::Heap>(), resources::control_bytes())?,
                        if plan.uses_owned_vectors() {
                            resources::owned_control_bytes::<W>()
                        } else {
                            0
                        },
                    )?
                } else {
                    0
                },
                if plan.has_binary_delivery() {
                    binary_delivery_control_bytes()
                } else {
                    0
                } + if plan.has_file_delivery() {
                    experimental::file_delivery_control_bytes()
                } else {
                    0
                } + if plan.has_file_seek() {
                    experimental::file_seek_control_bytes::<V>()
                } else {
                    0
                } + if plan.uses_vectors() {
                    vector::control_bytes::<V>()
                        + if plan.uses_record_vectors() {
                            vector::record_control_bytes()
                        } else {
                            0
                        }
                } else if plan.uses_typed_ranges() {
                    vector::range_control_bytes()
                } else {
                    0
                } + if plan.uses_owned_vectors() {
                    owned_vector::control_bytes::<V, W>()
                } else {
                    0
                } + if plan.has_program_arguments() {
                    plan::add(
                        crate::program_arguments::control_bytes(),
                        experimental::argument_delivery_control_bytes(f.span),
                    )?
                } else {
                    0
                },
            )?,
        )
        .and_then(|bytes| plan::add(bytes, values::control_bytes::<V>()))
        .and_then(|bytes| {
            plan::add(
                bytes,
                if plan.uses_enum_record_nesting() {
                    composition::control_bytes::<V>()
                } else {
                    0
                },
            )
        })
        .and_then(|bytes| {
            plan::add(
                bytes,
                if plan.uses_characters() {
                    values::character_control_bytes::<V>()
                } else {
                    0
                },
            )
        })
        .and_then(|bytes| {
            plan::add(
                bytes,
                if plan.uses_floats() {
                    floats::control_bytes::<V>() + 8192
                } else {
                    0
                },
            )
        })
        .and_then(|bytes| {
            plan::add(
                bytes,
                if plan.uses_bitwise() {
                    bitwise::control_bytes::<V>()
                } else {
                    0
                },
            )
        })
        .and_then(|bytes| {
            plan::add(
                bytes,
                if plan.uses_integer_extension() || plan.uses_quad_scalars() {
                    integers::control_bytes::<V>()
                } else {
                    0
                },
            )
        })?,
        heap: byte_buffer::Heap::for_execution(execution.heap_limit()),
        #[cfg(test)]
        events: Vec::new(),
        #[cfg(test)]
        observer: observation
            .as_mut()
            .map(|observer| std::mem::take(*observer))
            .unwrap_or_default(),
    };
    #[cfg(test)]
    if policy == EntryPolicy::Process {
        // Process effects cannot trigger test-only event Vec allocations after
        // the first write. The private process seam has no observation sink.
        machine.observer.silent = true;
    }
    let mut result = (|| {
        machine.activation_preflight(
            entry,
            plan::add(1, plan.function(entry).usage().activation_fuel_cells())?,
            f.span,
        )?;
        machine.frames = plan::reserve(limits.frames)
            .map_err(|error| OwnedRunFailure::from(error).at_runtime(f.span))?;
        let root = ValueFrame::<V>::allocate(plan, entry, machine.next_activation, None)
            .map_err(|error| error.at_runtime(f.span))?;
        machine.install(root);
        machine.execute()
    })();
    if result.is_ok() && machine.heap.ledger().live_allocations != 0 {
        result = Err(bad("resource obligations at execution end", f.span));
    }
    // Terminal reclamation is outside all user fuel/admission paths. It also
    // runs on ordinary completion, after the zero-obligation check above.
    #[cfg(test)]
    let before_cleanup = machine.heap.ledger();
    machine.heap.cleanup();
    #[cfg(test)]
    byte_buffer::testing::record(&machine.heap, before_cleanup);
    #[cfg(test)]
    {
        if result.is_err() && machine.observer.collecting() {
            for frame in 0..machine.frames.len() {
                if !machine.observer.collecting() {
                    break;
                }
                let span = machine.function(machine.frames[frame].function).span;
                machine.observe_frame(frame, StorageObservationKind::Failure, span);
            }
        }
        if let Some(observation) = observation {
            *observation = machine.observer;
        }
        if let Some(fuel) = remaining_fuel {
            *fuel = machine.fuel;
        }
    }
    #[cfg(test)]
    if let Some(events) = events {
        *events = machine.events;
    }
    if policy == EntryPolicy::Process {
        match result?.narrow() {
            Some(scalar @ Scalar::I32(0..=255)) => Ok(scalar.into()),
            _ => Err(OwnedRunFailure::ProcessResult(f.span)),
        }
    } else {
        result
    }
}
/// Sealed process execution. Only a scalar result or failure escapes;
/// machine storage and all owners are dropped inside the ordinary executor.
/// SIGPIPE setup precedes entry diagnostics, planning and the activation guard.
/// Every driver must turn ProcessSetup into a silent status 74.
pub(super) fn run_process_limits<W: PlanWitness>(
    witness: &W,
    entry: Option<hir::DefId>,
    limits: Limits,
) -> Result<Scalar> {
    run_process_policy(witness, entry, limits, ExecutionPolicy::Bounded)
}

pub(super) fn run_process_with_execution<W: PlanWitness>(
    witness: &W,
    entry: Option<hir::DefId>,
    execution: ExecutionPolicy,
) -> Result<Scalar> {
    run_process_policy(witness, entry, Limits::default(), execution)
}

fn run_process_policy<W: PlanWitness>(
    witness: &W,
    entry: Option<hir::DefId>,
    limits: Limits,
    execution: ExecutionPolicy,
) -> Result<Scalar> {
    // This host-only check intentionally precedes setup and does not validate
    // the entry, inspect source cells, allocate a plan or debit source fuel.
    if !output::supported_host() {
        return Err(OwnedRunFailure::ProcessHost);
    }
    if !process::setup() {
        return Err(OwnedRunFailure::ProcessSetup);
    }
    let entry = checked_entry_policy(witness, entry, EntryPolicy::Process)?;
    let plan = ExecutionPlan::build(witness)?;
    execute_result_plan(&plan, entry, limits, EntryPolicy::Process, execution)?
        .narrow()
        .ok_or(OwnedRunFailure::ProcessResult(
            witness.functions()[entry.0].span,
        ))
}
#[cfg(test)]
pub(super) fn observe_witness<W: PlanWitness>(
    witness: &W,
    entry: Option<hir::DefId>,
    limits: Limits,
    execution: ExecutionPolicy,
) -> (Result<Scalar>, usize) {
    let mut remaining = execution.initial_fuel(limits.bounded().fuel);
    let result = (|| {
        let entry = checked_entry_policy(witness, entry, EntryPolicy::Result)?;
        let plan = ExecutionPlan::build(witness)?;
        let span = witness.functions()[entry.0].span;
        if plan.uses_quad_scalars() {
            execute_plan_policy::<RuntimeValue, W>(
                &plan,
                entry,
                limits,
                EntryPolicy::Result,
                execution,
                None,
                None,
                Some(&mut remaining),
            )?
            .narrow()
            .ok_or_else(|| bad("test scalar observation", span))
        } else if plan.uses_wide_scalars() {
            execute_plan_policy::<RuntimeScalar, W>(
                &plan,
                entry,
                limits,
                EntryPolicy::Result,
                execution,
                None,
                None,
                Some(&mut remaining),
            )?
            .narrow()
            .ok_or_else(|| bad("test scalar observation", span))
        } else {
            execute_plan_policy::<Scalar, W>(
                &plan,
                entry,
                limits,
                EntryPolicy::Result,
                execution,
                None,
                None,
                Some(&mut remaining),
            )
        }
    })();
    (result, remaining)
}

#[cfg(test)]
pub(super) fn run_observed(
    witness: &VerifiedOwnedProgram,
    entry: hir::DefId,
    limits: Limits,
    events: &mut Vec<Event>,
) -> Result<Scalar> {
    let plan = ExecutionPlan::build(witness)?;
    execute_plan(&plan, entry, limits, Some(events))
}
#[cfg(test)]
#[path = "array_observe.rs"]
mod array_observe;
#[cfg(test)]
pub(super) use array_observe::{
    FaultInjection, FaultKind, ObservationAllocationSite, ObservationControl, ReferenceObservation,
    StorageObservationKind,
};
#[cfg(test)]
pub(super) type StorageSnapshot = array_observe::StorageSnapshot;
#[cfg(test)]
pub(super) fn run_array_observed(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    limits: Limits,
    control: ObservationControl,
) -> ReferenceObservation {
    let mut events = Vec::new();
    let mut observer = array_observe::Observer {
        enabled: true,
        control,
        ..Default::default()
    };
    let mut remaining_fuel = limits.bounded().fuel;
    let result = (|| {
        let entry = checked_entry(witness, entry)?;
        let plan = ExecutionPlan::build(witness)?;
        execute_plan_inner(
            &plan,
            entry,
            limits,
            EntryPolicy::Result,
            Some(&mut events),
            Some(&mut observer),
            Some(&mut remaining_fuel),
        )
    })();
    ReferenceObservation {
        fault_applied: observer.fault_applied,
        allocation_fault_applied: observer.allocation_fault_applied,
        result,
        events,
        storage: observer.storage,
        remaining_fuel,
        truncated: observer.truncated,
    }
}
#[cfg(test)]
#[path = "execute_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "execute_owned_vector_tests.rs"]
mod owned_vector_tests;

#[cfg(test)]
#[path = "execute_owned_vector_entry_tests.rs"]
mod owned_vector_entry_tests;

#[cfg(test)]
#[path = "builtin_output_reference_tests.rs"]
mod output_tests;

/// Pure staging over already-preflighted physical storage. This has no access
/// to a machine, witness, owner, syscall or external effect. It returns false
/// for any non-byte cell; the caller cannot emit the partially staged prefix.
fn stage_stdout(source: &[u8], staged: &mut [u8]) -> bool {
    if staged.len() > 1024 || source.len() != staged.len() * 4 {
        return false;
    }
    for (cell, destination) in source.as_chunks::<4>().0.iter().zip(staged) {
        let value = i32::from_le_bytes(*cell);
        let Ok(byte) = u8::try_from(value) else {
            return false;
        };
        *destination = byte;
    }
    true
}

fn record_type(aggregate: AggregateTy, span: Span) -> Result<RecordId> {
    match aggregate {
        AggregateTy::Record(record) => Ok(record),
        AggregateTy::FixedArray(_)
        | AggregateTy::Enum(_)
        | AggregateTy::ByteBuf
        | AggregateTy::File
        | AggregateTy::Vector(_)
        | AggregateTy::RecordVector(_)
        | AggregateTy::OwnedRecordVector(_) => Err(bad("unsupported aggregate carrier", span)),
    }
}

#[cfg(test)]
pub(super) mod record_field_view_tests {
    use super::*;

    fn machine<'p, 'w>(plan: &'p ExecutionPlan<'w>) -> Machine<'p, 'w> {
        let mut machine = Machine {
            plan,
            frames: Vec::new(),
            limits: Limits {
                frames: 8,
                ..Limits::default()
            },
            fuel: 0,
            next_activation: 1,
            live_slots: 0,
            live_cells: 0,
            live_bytes: 0,
            header_bytes: 8 * size_of::<Frame>(),
            heap: byte_buffer::Heap::default(),
            events: Vec::new(),
            observer: array_observe::Observer::default(),
        };
        machine.install(Frame::allocate(plan, hir::DefId(0), 1, None).unwrap());
        machine
    }

    // Accept a sealed witness only from its existing source owner. The
    // corruption exercise returns bounded counts, never execution authority.
    pub(in crate::frontend::oir::owned) fn exercise_record_views(
        witness: &verified::VerifiedOwnedProgram,
    ) -> usize {
        assert_eq!(run(witness, Some(hir::DefId(0))), Ok(Scalar::I32(37)));
        let plan = ExecutionPlan::build(witness).unwrap();
        for mutation in 0..5 {
            let mut machine = machine(&plan);
            let (frame, base, at) = loop {
                if let Some(frame) = machine.frames.last() {
                    let function = machine.function(frame.function);
                    if frame.function == hir::DefId(2) {
                        if let Some(statement) =
                            function.blocks[frame.block.0].statements.get(frame.next)
                        {
                            if let OwnedInstruction::ReadField { base, .. } = statement.kind {
                                break (machine.frames.len() - 1, base, statement.span);
                            }
                        }
                    }
                }
                machine.fuel += 1;
                assert!(matches!(
                    machine.execute(),
                    Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_)))
                ));
            };
            let AccessBase::Parameter(parameter) = base else {
                panic!("reference read required");
            };
            let original = machine.frames[frame].references[parameter.0];
            assert_eq!(original.view.offset, 8);
            assert!(machine.checked_handle_view(original, at).is_ok());
            let mut forged = original;
            match mutation {
                0 => forged.view.offset += 4, // same nominal type in the sibling field
                1 => forged.view.offset = 0,
                2 => forged.view.offset += 1,
                3 => forged.view.offset = u64::MAX,
                _ => {
                    forged.view.aggregate = Some(
                        AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(3)))
                            .unwrap(),
                    )
                }
            }
            // Keep the mutable transport rows coherent: an equality check of
            // retained handles alone must not grant another field's storage.
            machine.frames[frame].references[parameter.0] = forged;
            machine.frames[forged.permission.frame as usize].loans
                [forged.permission.loan as usize]
                .view = forged.view;
            assert!(machine.validate_handle(forged, Access::Read, at).is_ok());
            assert!(machine.checked_handle_view(forged, at).is_err());
            machine.fuel = plan::MAX_FUEL;
            assert!(matches!(
                machine.execute(),
                Err(OwnedRunFailure::Invariant { .. })
            ));
        }
        5
    }
}

#[cfg(test)]
mod u8_payment_tests {
    use super::*;

    #[test]
    fn owned_u8_payment_precedes_operand_read_range_check_and_destination_write() {
        let (sources, raw, _) = super::super::u8_tests::raw(256, false);
        let witness = source::u8_tests::verify_source_raw(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        for (at, input, fuel) in [
            (3, None, 0),
            (3, Some(Scalar::I32(256)), 1),
            (6, None, 0),
            (6, Some(Scalar::U8(255)), 1),
        ] {
            let f = &witness.functions()[0];
            let OwnedInstruction::Scalar(Statement::Assign(assign)) =
                &f.blocks[0].statements[at].kind
            else {
                unreachable!()
            };
            let (operand, name) = match assign.value {
                Rvalue::CheckedI32ToU8 {
                    operand, name_span, ..
                }
                | Rvalue::U8ToI32 {
                    operand, name_span, ..
                } => (operand, name_span),
                _ => unreachable!(),
            };
            let mut frame = Frame::allocate(&plan, hir::DefId(0), 1, None).unwrap();
            frame.next = at;
            frame.merge_pending = false;
            frame.slots[operand.local.0] = input;
            let mut machine = Machine {
                plan: &plan,
                frames: vec![frame],
                limits: Limits::default(),
                fuel,
                next_activation: 2,
                live_slots: 7,
                live_cells: 7,
                live_bytes: 56,
                header_bytes: 0,
                heap: byte_buffer::Heap::default(),
                events: vec![],
                observer: Default::default(),
            };
            let actual = machine.execute();
            if fuel == 0 {
                assert_eq!(
                    actual,
                    Err(OwnedRunFailure::Scalar(RunFailure::Fuel(assign.span)))
                );
                assert_eq!(machine.frames[0].slots[assign.destination.0], None);
            } else if at == 3 {
                assert_eq!(
                    actual,
                    Err(OwnedRunFailure::Scalar(RunFailure::ByteRange(name)))
                );
                assert_eq!(machine.frames[0].slots[assign.destination.0], None);
            } else {
                assert!(matches!(
                    actual,
                    Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_)))
                ));
                assert_eq!(
                    machine.frames[0].slots[assign.destination.0],
                    Some(Scalar::I32(255))
                );
            }
        }
    }
}

#[cfg(test)]
#[test]
fn owned_u8_runtime_containing_transport_layouts() {
    use std::mem::{align_of, size_of};
    macro_rules! report {($($t:ty),+)=>{$(println!("RFC0030 owned runtime {} size={} align={}",stringify!($t),size_of::<$t>(),align_of::<$t>());)+}}
    report!(
        Frame,
        Resume,
        Option<Resume>,
        Machine<'_, '_>,
        Vec<Option<Scalar>>,
        OwnedRunFailure,
        Result<Scalar>
    );
}

#[cfg(test)]
#[path = "byte_storage_codec_tests.rs"]
mod byte_storage_codec_tests;

#[cfg(test)]
pub(super) mod owned_enum_payload_tests {
    use super::*;
    use crate::frontend::oir::owned::consumer_fixtures as raw;

    pub(in crate::frontend::oir::owned) fn fixture() -> (SourceMap, RawOwnedProgram, Span, Span) {
        let (sources, span) = raw::context();
        let record = AggregateTy::Record(RecordId(0));
        let inner = AggregateTy::Enum(EnumId(0));
        let outer = AggregateTy::Enum(EnumId(1));
        let mut function = raw::function(0, ValueTy::Scalar(hir::Ty::I32), span(0));
        function.locals = vec![
            raw::scalar(hir::Ty::Bool, span(1)),
            raw::scalar(hir::Ty::I32, span(2)),
        ];
        function.owners = [record, inner, outer, outer]
            .into_iter()
            .enumerate()
            .map(|(index, aggregate)| OwnerDecl {
                aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
                kind: if index < 2 {
                    OwnerKind::Temporary
                } else {
                    OwnerKind::Local { mutable: false }
                },
                span: span(0),
            })
            .collect();
        let variant = |enumeration| VariantId {
            enumeration: EnumId(enumeration),
            index: 0,
        };
        let statements = vec![
            raw::assign(0, Rvalue::Bool(true), span(1)),
            raw::assign(1, Rvalue::I32(42), span(2)),
            raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), span(3)),
            raw::instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(0),
                    fields: (0..2)
                        .map(|index| {
                            (
                                FieldId {
                                    record: RecordId(0),
                                    index,
                                },
                                raw::operand(index, span(index + 1)),
                            )
                        })
                        .collect(),
                },
                span(4),
            ),
            raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), span(5)),
            raw::instruction(
                OwnedInstruction::ConstructOwnedEnum {
                    destination: OwnerPlaceId(1),
                    variant: variant(0),
                    payload: OwnerPlaceId(0),
                },
                span(6),
            ),
            raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(2)), span(7)),
            raw::instruction(
                OwnedInstruction::ConstructOwnedEnum {
                    destination: OwnerPlaceId(2),
                    variant: variant(1),
                    payload: OwnerPlaceId(1),
                },
                span(8),
            ),
            raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(3)), span(9)),
            raw::instruction(
                OwnedInstruction::MoveInitialize {
                    destination: OwnerPlaceId(3),
                    source: OwnerPlaceId(2),
                },
                span(10),
            ),
            raw::instruction(OwnedInstruction::Discard(OwnerPlaceId(3)), span(11)),
            raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(3)), span(12)),
            raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(2)), span(13)),
            raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), span(14)),
            raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), span(15)),
        ];
        function.blocks = vec![OwnedBlock {
            merge: None,
            span: span(0),
            statements,
            terminator: raw::end(
                OwnedTerminatorKind::ReturnScalar(raw::operand(1, span(16))),
                span(16),
            ),
        }];
        let enums = [record, inner]
            .into_iter()
            .enumerate()
            .map(|(id, aggregate)| RawEnumDecl {
                id: EnumId(id),
                span: span(0),
                variants: vec![
                    RawVariantDecl {
                        id: variant(id),
                        payload: Some(ParameterTy::Value(ValueTy::Owned(aggregate))),
                        span: span(0),
                    },
                    RawVariantDecl {
                        id: VariantId {
                            enumeration: EnumId(id),
                            index: 1,
                        },
                        payload: None,
                        span: span(0),
                    },
                ],
            })
            .collect();
        (
            sources,
            RawOwnedProgram {
                vector_functions: Vec::new(),
                builtins: BuiltinOrigins::None,
                records: vec![raw::record(&[hir::Ty::Bool, hir::Ty::I32], span(0))],
                enums,
                functions: vec![function],
            },
            span(8),
            span(10),
        )
    }

    pub(in crate::frontend::oir::owned) fn late_leaf_fixture(
        extraction: bool,
    ) -> (SourceMap, RawOwnedProgram, [Span; 3]) {
        let (sources, mut program, construction, transfer) = fixture();
        program.records[0].fields[0].ty = ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32));
        program.records[0].fields[1].ty = ParameterTy::Value(ValueTy::Scalar(hir::Ty::Bool));
        let OwnedInstruction::Construct { fields, .. } =
            &mut program.functions[0].blocks[0].statements[3].kind
        else {
            unreachable!()
        };
        fields[0].1.local = LocalId(1);
        fields[1].1.local = LocalId(0);
        let span = program.functions[0].span;
        let at = Span {
            start: span.start + 40,
            end: span.end + 40,
            ..span
        };
        if extraction {
            program.enums[1].variants.truncate(1);
            let function = &mut program.functions[0];
            function.owners.push(OwnerDecl {
                aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(0))).unwrap(),
                kind: OwnerKind::Local { mutable: false },
                span: at,
            });
            function.matches = vec![MatchDecl {
                source: OwnerPlaceId(3),
                arms: vec![MatchArm {
                    variant: VariantId {
                        enumeration: EnumId(1),
                        index: 0,
                    },
                    dispatch: BlockId(0),
                    entry: BlockId(1),
                }],
                span: at,
            }];
            function.blocks[0].statements.truncate(10);
            function.blocks[0].terminator = raw::end(
                OwnedTerminatorKind::MatchDispatch {
                    match_id: MatchId(0),
                    arm: 0,
                },
                at,
            );
            let mut statements = vec![
                raw::instruction(
                    OwnedInstruction::ConsumeOwnedVariant {
                        match_id: MatchId(0),
                        arm: 0,
                        destination: OwnerPlaceId(4),
                    },
                    at,
                ),
                raw::instruction(OwnedInstruction::Discard(OwnerPlaceId(4)), span),
            ];
            statements.extend((0..5).rev().map(|owner| {
                raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(owner)), span)
            }));
            function.blocks.push(OwnedBlock {
                merge: None,
                span,
                statements,
                terminator: raw::end(
                    OwnedTerminatorKind::ReturnScalar(raw::operand(1, span)),
                    span,
                ),
            });
        }
        (sources, program, [construction, transfer, at])
    }

    #[test]
    fn owned_enum_reference_late_invalid_leaf_preserves_all_destination_bytes_and_state() {
        // R { first:i32, last:bool } makes the valid first leaf observable.
        // A validate-and-copy-per-leaf implementation would write 42 before
        // discovering the noncanonical final bool in each of these operations.
        for (site, owner, destination, offset, state, generation) in [
            (0, 1, 2, 8, UNINITIALIZED, 1),
            (1, 2, 3, 12, UNINITIALIZED, 1),
            (2, 3, 4, 12, DEAD, 0),
        ] {
            let (sources, raw, spans) = late_leaf_fixture(site == 2);
            let witness = verified::verify_owned(raw, &sources).unwrap();
            assert_eq!(run(&witness, Some(hir::DefId(0))), Ok(Scalar::I32(42)));
            let at = spans[site];
            let observed = run_array_observed(
                &witness,
                Some(hir::DefId(0)),
                Limits::default(),
                ObservationControl {
                    poison_destinations: true,
                    fault: Some(FaultInjection {
                        at,
                        kind: FaultKind::EnumOwnerByte {
                            owner: OwnerPlaceId(owner),
                            offset,
                            byte: 2,
                        },
                    }),
                    ..Default::default()
                },
            );
            assert!(observed.fault_applied);
            assert!(!observed.truncated);
            assert_eq!(observed.result, Err(bad("enum payload", at)));
            let snapshot = |id| {
                observed
                    .storage
                    .iter()
                    .find(|snapshot| {
                        snapshot.kind == StorageObservationKind::Failure
                            && snapshot.key.owner == id as u64
                    })
                    .unwrap()
            };
            let destination = snapshot(destination);
            assert_eq!(
                (destination.state, destination.key.generation),
                (state, generation)
            );
            assert!(destination.bytes.iter().all(|byte| *byte == 0));
            assert_eq!(destination.guards_before, destination.guards_after);
            let source = snapshot(owner);
            assert_eq!((source.state, source.key.generation), (AVAILABLE, 2));
            assert_eq!(&source.bytes[offset - 4..offset], &[42, 0, 0, 0]);
            assert_eq!(source.bytes[offset], 2);
            assert_eq!(source.guards_before, source.guards_after);
        }
    }

    #[test]
    fn owned_enum_reference_consume_validates_before_starting_arm_owner() {
        let (sources, mut raw, _, _) = fixture();
        let span = raw.functions[0].span;
        let at = Span {
            start: span.start + 40,
            end: span.end + 40,
            ..span
        };
        raw.enums[1].variants.truncate(1);
        let function = &mut raw.functions[0];
        function.owners.push(OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(0))).unwrap(),
            kind: OwnerKind::Local { mutable: false },
            span: at,
        });
        function.matches = vec![MatchDecl {
            source: OwnerPlaceId(3),
            arms: vec![MatchArm {
                variant: VariantId {
                    enumeration: EnumId(1),
                    index: 0,
                },
                dispatch: BlockId(0),
                entry: BlockId(1),
            }],
            span: at,
        }];
        function.blocks[0].statements.truncate(10);
        function.blocks[0].terminator = raw::end(
            OwnedTerminatorKind::MatchDispatch {
                match_id: MatchId(0),
                arm: 0,
            },
            at,
        );
        let mut statements = vec![
            raw::instruction(
                OwnedInstruction::ConsumeOwnedVariant {
                    match_id: MatchId(0),
                    arm: 0,
                    destination: OwnerPlaceId(4),
                },
                at,
            ),
            raw::instruction(OwnedInstruction::Discard(OwnerPlaceId(4)), span),
        ];
        statements.extend((0..5).rev().map(|owner| {
            raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(owner)), span)
        }));
        function.blocks.push(OwnedBlock {
            merge: None,
            span,
            statements,
            terminator: raw::end(
                OwnedTerminatorKind::ReturnScalar(raw::operand(1, span)),
                span,
            ),
        });
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let observed = run_array_observed(
            &witness,
            Some(hir::DefId(0)),
            Limits::default(),
            ObservationControl {
                poison_destinations: true,
                ..Default::default()
            },
        );
        assert_eq!(observed.result, Ok(Scalar::I32(42)));
        assert!(observed.events.iter().any(
            |event| matches!(event, Event::Transfer(_, to) if to.owner == 4 && to.generation == 2)
        ));
        for (offset, byte, invariant) in [(4, 255, "enum tag"), (8, 2, "enum payload")] {
            let observed = run_array_observed(
                &witness,
                Some(hir::DefId(0)),
                Limits::default(),
                ObservationControl {
                    fault: Some(FaultInjection {
                        at,
                        kind: FaultKind::EnumOwnerByte {
                            owner: OwnerPlaceId(3),
                            offset,
                            byte,
                        },
                    }),
                    ..Default::default()
                },
            );
            assert!(observed.fault_applied);
            assert_eq!(observed.result, Err(bad(invariant, at)));
            let destination = observed
                .storage
                .iter()
                .find(|snapshot| {
                    snapshot.kind == StorageObservationKind::Failure && snapshot.key.owner == 4
                })
                .unwrap();
            assert_eq!((destination.state, destination.key.generation), (DEAD, 0));
            assert!(destination.bytes.iter().all(|byte| *byte == 0));
            let source = observed
                .storage
                .iter()
                .find(|snapshot| {
                    snapshot.kind == StorageObservationKind::Failure && snapshot.key.owner == 3
                })
                .unwrap();
            assert_eq!((source.state, source.key.generation), (AVAILABLE, 2));
        }
    }

    #[test]
    fn owned_enum_reference_nested_transfer_preserves_only_active_cells() {
        let (sources, raw, _, _) = fixture();
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let observed = run_array_observed(
            &witness,
            Some(hir::DefId(0)),
            Limits::default(),
            ObservationControl {
                poison_destinations: true,
                ..Default::default()
            },
        );
        assert_eq!(observed.result, Ok(Scalar::I32(42)));
        assert!(!observed.truncated);
        let transfer = observed
            .storage
            .iter()
            .find(|snapshot| {
                snapshot.kind == StorageObservationKind::Transfer && snapshot.key.owner == 3
            })
            .unwrap();
        assert_eq!(
            transfer.bytes,
            [0, 0, 0, 0, 0, 0, 0, 0, 1, 0xa5, 0xa5, 0xa5, 42, 0, 0, 0]
        );
        assert_eq!(transfer.guards_before, transfer.guards_after);
    }

    #[test]
    fn owned_enum_reference_invalid_nested_values_leave_destination_and_owners_unchanged() {
        let (sources, raw, construction, transfer) = fixture();
        let witness = verified::verify_owned(raw, &sources).unwrap();
        for (at, owner, destination, offset, byte, invariant) in [
            (construction, 1, 2, 0, 255, "enum tag"),
            (construction, 1, 2, 4, 2, "enum payload"),
            (transfer, 2, 3, 4, 255, "enum tag"),
            (transfer, 2, 3, 8, 2, "enum payload"),
        ] {
            let observed = run_array_observed(
                &witness,
                Some(hir::DefId(0)),
                Limits::default(),
                ObservationControl {
                    poison_destinations: true,
                    fault: Some(FaultInjection {
                        at,
                        kind: FaultKind::EnumOwnerByte {
                            owner: OwnerPlaceId(owner),
                            offset,
                            byte,
                        },
                    }),
                    ..Default::default()
                },
            );
            assert!(observed.fault_applied);
            assert!(!observed.truncated);
            assert_eq!(observed.result, Err(bad(invariant, at)));
            let destination = observed
                .storage
                .iter()
                .find(|snapshot| {
                    snapshot.kind == StorageObservationKind::Failure
                        && snapshot.key.owner == destination as u64
                })
                .unwrap();
            assert_eq!(destination.state, UNINITIALIZED);
            assert_eq!(destination.key.generation, 1);
            assert!(destination.bytes.iter().all(|byte| *byte == 0));
            let source = observed
                .storage
                .iter()
                .find(|snapshot| {
                    snapshot.kind == StorageObservationKind::Failure
                        && snapshot.key.owner == owner as u64
                })
                .unwrap();
            assert_eq!(source.state, AVAILABLE);
            assert_eq!(source.key.generation, 2);
        }
    }
}
