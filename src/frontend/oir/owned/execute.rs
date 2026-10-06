//! Iterative reference execution of the sealed owned witness. No raw entry point.
use super::{
    plan::{self, ExecutionPlan},
    storage::*,
    verified::VerifiedOwnedProgram,
    *,
};
use crate::frontend::options::EntryPolicy;
use std::mem::size_of;

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
                "bounded stdin execution requires Linux x86_64",
                Some(*span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::OutputHost(span) => Diagnostic::new(
                "E0608",
                "oir-owned-run",
                "bounded stdout execution requires Linux x86_64",
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
                "process execution requires Linux x86_64",
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
struct Frame {
    function: hir::DefId,
    activation: u64,
    block: BlockId,
    next: usize,
    predecessor: Option<BlockId>,
    merge_pending: bool,
    slots: Vec<Option<Scalar>>,
    snapshots: Vec<Option<Scalar>>,
    payload: Vec<u8>,
    owners: Vec<OwnerRuntime>,
    references: Vec<ReferenceHandle>,
    loans: Vec<LoanRuntime>,
    calls: Vec<CallRuntime>,
    return_to: Option<Resume>,
}
impl Frame {
    fn allocate(
        plan: &ExecutionPlan<'_>,
        function: hir::DefId,
        activation: u64,
        return_to: Option<Resume>,
    ) -> Result<Self> {
        let f = &plan.witness().functions()[function.0];
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
pub(super) enum Event {
    Charge(Span, usize),
    Enter(hir::DefId, u64),
    WriteField(OwnerKey, FieldId, Scalar),
    ReadIndex(OwnerKey, usize, Scalar),
    WriteIndex(OwnerKey, usize, Scalar),
    ArrayLength(OwnerKey, usize),
    Transfer(OwnerKey, OwnerKey),
    Acquire(LoanKey, OwnerKey, BorrowKind),
    Release(LoanKey),
    Return(hir::DefId),
    EnumTagRead(OwnerKey, u32),
    EnumPayloadRead(OwnerKey, usize),
    EnumBind(OwnerKey, Option<LocalId>, Option<Scalar>),
}
/// A fixed-size active-value snapshot, never the inactive union or its padding.
#[derive(Clone, Copy)]
struct EnumValue {
    tag: u32,
    payload: Option<Scalar>,
}
#[derive(Clone, Copy)]
enum Access {
    Read,
    Write,
    Consume,
    Borrow(BorrowKind),
}
struct Machine<'p, 'w> {
    plan: &'p ExecutionPlan<'w>,
    frames: Vec<Frame>,
    limits: Limits,
    fuel: usize,
    next_activation: u64,
    live_slots: usize,
    live_cells: usize,
    live_bytes: usize,
    header_bytes: usize,
    #[cfg(test)]
    events: Vec<Event>,
    #[cfg(test)]
    observer: array_observe::Observer,
}
impl<'p, 'w> Machine<'p, 'w> {
    fn function(&self, id: hir::DefId) -> &'w RawOwnedFunction {
        &self.plan.witness().functions()[id.0]
    }
    fn charge(&mut self, cost: usize, span: Span) -> Result<()> {
        self.fuel = self.fuel.checked_sub(cost).ok_or(RunFailure::Fuel(span))?;
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
            plan::add(self.header_bytes, size_of::<Scalar>())?,
            plan::add(self.live_bytes, u.reference_bytes)?,
        )?;
        if bytes > self.limits.bytes {
            return Err(plan::AdmissionFailure::new("live requested bytes", Some(span)).into());
        }
        epoch(self.next_activation, span)?;
        Ok(())
    }
    fn install(&mut self, frame: Frame) {
        let u = self.plan.function(frame.function).usage();
        self.live_slots += u.scalar_slots;
        self.live_cells += u.expanded_cells;
        self.live_bytes += u.reference_bytes;
        self.next_activation += 1; // preflight checked this epoch before allocation/transfer
        #[cfg(test)]
        self.record_event(Event::Enter(frame.function, frame.activation));
        self.frames.push(frame);
    }
    fn read(&self, frame: usize, operand: Operand) -> Result<Scalar> {
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
        value: Scalar,
        span: Span,
    ) -> Result<()> {
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
        self.validate_handle(result, Access::Read, span)?;
        Ok(result)
    }
    fn validate_handle(
        &self,
        handle: ReferenceHandle,
        access: Access,
        span: Span,
    ) -> Result<OwnerKey> {
        self.owner(handle.root, span)?;
        let l = self.loan(handle.permission, span)?;
        if l.view != handle.view {
            return Err(bad("loan view mismatch", span));
        }
        if l.root != handle.root {
            return Err(bad("loan root mismatch", span));
        }
        let shared_access = matches!(access, Access::Read | Access::Borrow(BorrowKind::Shared));
        if l.exclusive_children != 0 || (!shared_access && l.shared_children != 0) {
            return Err(bad("suspended parent permission", span));
        }
        if !shared_access && self.loan_kind(handle.permission, span)? != BorrowKind::Exclusive {
            return Err(bad("shared permission upgrade", span));
        }
        Ok(handle.root)
    }
    fn base(&self, frame: usize, base: AccessBase, access: Access, span: Span) -> Result<OwnerKey> {
        let active = &self.frames[frame];
        match base {
            AccessBase::Owner(id) => {
                let key = self.owner_key(frame, id, span)?;
                let state = self.owner(key, span)?;
                if state.exclusive_children != 0
                    || (state.shared_children != 0
                        && !matches!(access, Access::Read | Access::Borrow(BorrowKind::Shared)))
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
            AccessBase::Parameter(id) => {
                let handle = *active
                    .references
                    .get(id.0)
                    .ok_or_else(|| bad("reference parameter", span))?;
                self.validate_handle(handle, access, span)
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

    fn load_field(&self, key: OwnerKey, field: FieldId, span: Span) -> Result<Scalar> {
        let (offset, ty) = self.payload_field(key, field, span)?;
        decode(&self.frames[key.frame as usize].payload, offset, ty, span)
    }
    fn store_field(
        &mut self,
        key: OwnerKey,
        field: FieldId,
        value: Scalar,
        span: Span,
    ) -> Result<()> {
        let (offset, ty) = self.payload_field(key, field, span)?;
        if ty != value.ty() {
            return Err(bad("field value type", span));
        }
        encode(
            &mut self.frames[key.frame as usize].payload,
            offset,
            value,
            span,
        )
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
            .declarations()
            .check_aggregate_type(aggregate)
            .map_err(|_| bad("payload aggregate type", span))?;
        Ok(aggregate)
    }
    fn array_type(&self, key: OwnerKey, span: Span) -> Result<FixedArrayTy> {
        match self.aggregate(key, span)? {
            AggregateTy::FixedArray(array) => Ok(array),
            AggregateTy::Record(_) | AggregateTy::Enum(_) => Err(bad("array aggregate type", span)),
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
        if (matches!(root_type, AggregateTy::FixedArray(_))
            || matches!(aggregate, AggregateTy::Record(_)))
            && (view.offset != 0 || root_type != aggregate)
        {
            return Err(bad("view root type", span));
        }
        let size = self
            .plan
            .witness()
            .declarations()
            .aggregate_layout(aggregate)
            .map_err(|_| bad("view layout", span))?
            .size();
        let extent = self.owner_extent(key, span)?;
        index(view.offset, span)?
            .checked_add(size)
            .filter(|end| *end <= extent.len())
            .ok_or_else(|| bad("view extent", span))?;
        Ok(aggregate)
    }
    /// Re-derive projected storage from immutable nominal paths, rather than
    /// trusting a retained offset/extent. Slice reborrows may cross frames; the
    /// existing activation limit bounds this allocation-free parent walk.
    fn projected_view(
        &self,
        permission: LoanKey,
        root: OwnerKey,
        span: Span,
    ) -> Result<BorrowView> {
        let mut permission = permission;
        for _ in 0..plan::MAX_FRAMES {
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
                let (ValueTy::Owned(aggregate @ AggregateTy::FixedArray(_)), offset) = self
                    .plan
                    .witness()
                    .declarations()
                    .projection(self.aggregate(root, span)?, &descriptor.projection)
                    .map_err(|_| bad("view projection path", span))?
                else {
                    return Err(bad("view projection array", span));
                };
                return Ok(BorrowView {
                    offset: offset as u64,
                    aggregate: Some(
                        AggregateSlot::try_from_aggregate(aggregate)
                            .map_err(|_| bad("view projection descriptor", span))?,
                    ),
                });
            }
            if loan.parent.instance == 0 {
                break;
            }
            permission = loan.parent;
        }
        Err(bad("projected view provenance", span))
    }
    fn checked_handle_view(&self, handle: ReferenceHandle, span: Span) -> Result<AggregateTy> {
        let actual = self.view_extent(handle.root, handle.view, span)?;
        if matches!(
            (self.aggregate(handle.root, span)?, actual),
            (AggregateTy::Record(_), AggregateTy::FixedArray(_))
        ) && self.projected_view(handle.permission, handle.root, span)? != handle.view
        {
            return Err(bad("projected view mismatch", span));
        }
        Ok(actual)
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
        };
        let actual = match base {
            AccessBase::Owner(_) => self.view_extent(key, view, span)?,
            AccessBase::Parameter(id) => {
                self.checked_handle_view(self.frames[frame].references[id.0], span)?
            }
        };
        let f = self.function(self.frames[frame].function);
        let expected = match base {
            AccessBase::Owner(owner) => BorrowedTy::Exact(f.owners[owner.0].aggregate()),
            AccessBase::Parameter(reference) => f.references[reference.0].referent(),
        };
        self.plan
            .witness()
            .declarations()
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
        let AggregateTy::FixedArray(array) = self.view_extent(key, view, span)? else {
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
            .declarations()
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
    fn load_leaf(&self, key: OwnerKey, leaf: ScalarLeaf, span: Span) -> Result<Scalar> {
        let offset = self.leaf_offset(key, leaf.offset, leaf.ty, span)?;
        decode(
            &self.frames[key.frame as usize].payload,
            offset,
            leaf.ty,
            span,
        )
    }
    fn store_leaf(
        &mut self,
        key: OwnerKey,
        leaf: ScalarLeaf,
        value: Scalar,
        span: Span,
    ) -> Result<()> {
        if leaf.ty != value.ty() {
            return Err(bad("leaf value type", span));
        }
        let offset = self.leaf_offset(key, leaf.offset, leaf.ty, span)?;
        encode(
            &mut self.frames[key.frame as usize].payload,
            offset,
            value,
            span,
        )
    }
    fn projection_base(
        &self,
        frame: usize,
        base: AccessBase,
        access: Access,
        path: &[FieldId],
        span: Span,
    ) -> Result<(OwnerKey, ValueTy, usize)> {
        let key = self.base(frame, base, access, span)?;
        let function = self.function(self.frames[frame].function);
        let expected = match base {
            AccessBase::Owner(owner) => BorrowedTy::Exact(function.owners[owner.0].aggregate()),
            AccessBase::Parameter(reference) => function
                .references
                .get(reference.0)
                .ok_or_else(|| bad("projection reference declaration", span))?
                .referent(),
        };
        let actual = self.aggregate(key, span)?;
        let declarations = self.plan.witness().declarations();
        declarations
            .check_borrowed_view(BorrowedTy::Exact(actual), expected)
            .map_err(|_| bad("projection base type", span))?;
        let (ty, relative) = declarations
            .projection(actual, path)
            .map_err(|_| bad("projection path", span))?;
        let size = match ty {
            ValueTy::Scalar(ty) => scalar_size(ty),
            ValueTy::Owned(aggregate) => declarations
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
    ) -> Result<Scalar> {
        let offset = self.array_offset(key, array, ordinal, span)?;
        decode(
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
        value: Scalar,
        span: Span,
    ) -> Result<()> {
        if value.ty() != array.element() {
            return Err(bad("array element type", span));
        }
        let offset = self.array_offset(key, array, ordinal, span)?;
        encode(
            &mut self.frames[key.frame as usize].payload,
            offset,
            value,
            span,
        )
    }
    fn zero_sentinel(&mut self, key: OwnerKey, span: Span) -> Result<()> {
        let extent = self.owner_extent(key, span)?;
        self.frames[key.frame as usize].payload[extent].fill(0);
        Ok(())
    }

    /// The checked nominal table is the only tag/type authority. Dispatch uses
    /// this tag-only operation, so neither a failed nor a successful test exposes
    /// the payload before ConsumeVariant has paid its separate charge.
    fn enum_tag(&mut self, key: OwnerKey, span: Span) -> Result<(VariantId, Option<hir::Ty>)> {
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
        Ok((variant.id(), variant.payload()))
    }
    fn load_enum(
        &mut self,
        key: OwnerKey,
        expected: Option<VariantId>,
        span: Span,
    ) -> Result<EnumValue> {
        let (variant, payload_ty) = self.enum_tag(key, span)?;
        if expected.is_some_and(|expected| expected != variant) {
            return Err(bad("enum tag", span));
        }
        let payload = if let Some(ty) = payload_ty {
            let extent = self.owner_extent(key, span)?;
            let offset = scalar_offset(extent, 4, ty, span)?;
            #[cfg(test)]
            self.record_event(Event::EnumPayloadRead(key, scalar_size(ty)));
            Some(
                decode(&self.frames[key.frame as usize].payload, offset, ty, span)
                    .map_err(|_| bad("enum payload", span))?,
            )
        } else {
            None
        };
        Ok(EnumValue {
            tag: variant.index as u32,
            payload,
        })
    }
    fn preflight_enum_binding(
        &self,
        frame: usize,
        destination: Option<LocalId>,
        value: EnumValue,
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
            .declarations()
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
        let f = self.function(self.frames[frame].function);
        let descriptor = &f.loans[id.0];
        let root = self.base(
            frame,
            descriptor.authority,
            Access::Borrow(descriptor.kind),
            span,
        )?;
        let mut view = self.base_view(frame, descriptor.authority, root, span)?;
        let mut actual = self.view_extent(root, view, span)?;
        if !descriptor.projection.is_empty() {
            let (ValueTy::Owned(projected @ AggregateTy::FixedArray(_)), relative) = self
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
            .declarations()
            .check_borrowed_view(BorrowedTy::Exact(actual), descriptor.referent())
            .map_err(|_| bad("loan borrowed view", span))?;
        let parent = match descriptor.authority {
            AccessBase::Owner(_) => LoanKey::default(),
            AccessBase::Parameter(p) => self.frames[frame].references[p.0].permission,
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
            BorrowKind::Exclusive => counts.1 != 1,
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
        let value = match assign.value {
            Rvalue::Bool(v) => Scalar::Bool(v),
            Rvalue::I32(v) => Scalar::I32(v),
            Rvalue::Unit => Scalar::Unit,
            Rvalue::Copy(v) => self.read(frame, v)?,
            Rvalue::Load(p) => *self.frames[frame]
                .slots
                .get(f.locals.len() + p.id.0)
                .ok_or_else(|| bad("scalar place index", p.span))?
                .as_ref()
                .ok_or_else(|| bad("uninitialized scalar place", p.span))?,
            Rvalue::NotBool { operand, .. } => {
                let Scalar::Bool(v) = self.read(frame, operand)? else {
                    return Err(bad("not bool", operand.span));
                };
                Scalar::Bool(!v)
            }
            Rvalue::CheckedNegateI32 {
                operand,
                operator_span,
            } => {
                let Scalar::I32(value) = self.read(frame, operand)? else {
                    return Err(bad("negation type", operand.span));
                };
                Scalar::I32(
                    value
                        .checked_neg()
                        .ok_or(RunFailure::Overflow(operator_span))?,
                )
            }
            Rvalue::CheckedI32 {
                op,
                left,
                right,
                operator_span,
            } => {
                let (Scalar::I32(a), Scalar::I32(b)) =
                    (self.read(frame, left)?, self.read(frame, right)?)
                else {
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
            }
            Rvalue::CompareScalar {
                op, left, right, ..
            } => {
                let value = match (self.read(frame, left)?, self.read(frame, right)?) {
                    (Scalar::I32(a), Scalar::I32(b)) => match op {
                        hir::ComparisonOp::Equal => a == b,
                        hir::ComparisonOp::NotEqual => a != b,
                        hir::ComparisonOp::Less => a < b,
                        hir::ComparisonOp::LessEqual => a <= b,
                        hir::ComparisonOp::Greater => a > b,
                        hir::ComparisonOp::GreaterEqual => a >= b,
                    },
                    (Scalar::Bool(a), Scalar::Bool(b)) => match op {
                        hir::ComparisonOp::Equal => a == b,
                        hir::ComparisonOp::NotEqual => a != b,
                        _ => return Err(bad("boolean ordering", assign.span)),
                    },
                    _ => return Err(bad("comparison type", assign.span)),
                };
                Scalar::Bool(value)
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
                    .check_payload(enumeration, *variant, payload.map(Scalar::ty))
                    .map_err(|_| bad("enum construction type", span))?;
                let tag = declarations
                    .variant(enumeration, *variant)
                    .map_err(|_| bad("enum tag", span))?
                    .tag();
                let value = EnumValue { tag, payload };
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
                            declarations
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
                        Scalar::Unit,
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
                            for leaf in declarations
                                .leaves(self.aggregate(from, span)?)
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
                let (_key, ty, _) = self.projection_base(frame, *base, Access::Read, path, span)?;
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
            OwnedInstruction::ReadIndex {
                destination,
                base,
                index,
            } => {
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
                self.change_owner(frame, *o, DEAD, span)?;
            }
            OwnedInstruction::Discard(o) => {
                let key = self.base(frame, AccessBase::Owner(*o), Access::Consume, span)?;
                self.expect_owner(frame, *o, &[AVAILABLE], span)?;
                if matches!(self.aggregate(key, span)?, AggregateTy::Enum(_)) {
                    self.load_enum(key, None, span)?;
                }
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
            OwnedInstruction::MoveInitialize {
                destination,
                source,
            }
            | OwnedInstruction::Replace {
                destination,
                source,
            } => {
                let from = self.base(frame, AccessBase::Owner(*source), Access::Consume, span)?;
                self.expect_owner(frame, *source, &[AVAILABLE], span)?;
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
                self.transfer_payload(from, to, span)?;
                self.change_owner(frame, *source, MOVED, span)?;
                self.change_owner(frame, *destination, AVAILABLE, span)?;
            }
            OwnedInstruction::ReadField {
                destination,
                base,
                field,
            } => {
                let key = self.base(frame, *base, Access::Read, span)?;
                let value = self.load_field(key, *field, span)?;
                self.write(frame, *destination, value, span)?;
            }
            OwnedInstruction::WriteField { base, field, value } => {
                let key = self.base(frame, *base, Access::Write, span)?;
                let value = self.read(frame, *value)?;
                self.store_field(key, *field, value, span)?;
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
                {
                    return Err(bad("incoming reference alias contract", span));
                }
            }
        }
        // Allocation is after fuel/frame/slot/cell/byte preflight in the dispatcher loop.
        let mut child = Frame::allocate(
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
                        .declarations()
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
                        .declarations()
                        .same_aggregate_type(record, f.owners[source.0].aggregate())
                        .map_err(|_| bad("incoming owned type", span))?;
                    let offset = self.plan.function(callee.id).owner_offset(*destination);
                    let declarations = self.plan.witness().declarations();
                    let size = declarations
                        .aggregate_layout(record)
                        .map_err(|_| bad("incoming aggregate layout", span))?
                        .size();
                    let end = offset
                        .checked_add(size)
                        .filter(|end| *end <= child.payload.len())
                        .ok_or_else(|| bad("incoming aggregate range", span))?;
                    let active = if matches!(record, AggregateTy::Enum(_)) {
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
                    if let Some(value) = active {
                        encode_enum(&mut child.payload, offset..end, value, span)?;
                    } else {
                        for leaf in declarations
                            .leaves(record)
                            .map_err(|_| bad("incoming leaves", span))?
                        {
                            let value = self.load_leaf(key, leaf, span)?;
                            let leaf_offset =
                                scalar_offset(offset..end, leaf.offset, leaf.ty, span)?;
                            encode(&mut child.payload, leaf_offset, value, span)?;
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
    ) -> Result<Option<Scalar>> {
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
    fn execute(&mut self) -> Result<Scalar> {
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
                    OwnedInstruction::ReadStdin { .. } | OwnedInstruction::WriteStdout { .. }
                ) {
                    self.charge(self.plan.statement_cost(f.id, &statement.kind), span)?;
                }
                #[cfg(test)]
                self.inject_statement(frame, &statement.kind, span);
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
                    let Scalar::Bool(value) = self.read(frame, condition)? else {
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
/// Validate every destination byte range before the first write. The snapshot
/// contains no inactive bytes and encoding cannot inspect them.
fn enum_offsets(
    extent: std::ops::Range<usize>,
    value: EnumValue,
    span: Span,
) -> Result<(usize, Option<usize>)> {
    let tag = scalar_offset(extent.clone(), 0, hir::Ty::I32, span)?;
    let payload = value
        .payload
        .map(|payload| scalar_offset(extent, 4, payload.ty(), span))
        .transpose()?;
    Ok((tag, payload))
}
fn encode_enum(
    bytes: &mut [u8],
    extent: std::ops::Range<usize>,
    value: EnumValue,
    span: Span,
) -> Result<()> {
    if extent.end > bytes.len() {
        return Err(bad("enum storage", span));
    }
    let (tag, payload_offset) = enum_offsets(extent, value, span)?;
    bytes[tag..tag + 4].copy_from_slice(&value.tag.to_le_bytes());
    if let (Some(offset), Some(payload)) = (payload_offset, value.payload) {
        encode(bytes, offset, payload, span)?;
    }
    Ok(())
}

fn scalar_size(ty: hir::Ty) -> usize {
    match ty {
        hir::Ty::I32 => 4,
        hir::Ty::Bool | hir::Ty::Unit => 1,
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
fn array_index(value: Scalar, array: FixedArrayTy, span: Span) -> Result<usize> {
    let Scalar::I32(index) = value else {
        return Err(bad("array index type", span));
    };
    if index < 0 || index >= array.length() as i32 {
        return Err(OwnedRunFailure::Bounds(span));
    }
    usize::try_from(index).map_err(|_| bad("array index conversion", span))
}
fn decode(bytes: &[u8], offset: usize, ty: hir::Ty, span: Span) -> Result<Scalar> {
    Ok(match ty {
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
pub(super) fn run(witness: &VerifiedOwnedProgram, entry: Option<hir::DefId>) -> Result<Scalar> {
    run_limits(witness, entry, Limits::default())
}
pub(super) fn run_limits(
    witness: &VerifiedOwnedProgram,
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
fn checked_entry(witness: &VerifiedOwnedProgram, entry: Option<hir::DefId>) -> Result<hir::DefId> {
    checked_entry_policy(witness, entry, EntryPolicy::Result)
}
fn checked_entry_policy(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    policy: EntryPolicy,
) -> Result<hir::DefId> {
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
fn execute_plan(
    plan: &ExecutionPlan<'_>,
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
fn execute_plan_inner(
    plan: &ExecutionPlan<'_>,
    entry: hir::DefId,
    limits: Limits,
    policy: EntryPolicy,
    #[cfg(test)] events: Option<&mut Vec<Event>>,
    #[cfg(test)] mut observation: Option<&mut array_observe::Observer>,
    #[cfg(test)] remaining_fuel: Option<&mut usize>,
) -> Result<Scalar> {
    // All callers share entry validation. Every sealed Process caller
    // establishes host support and signal policy before reaching this point.
    checked_entry_policy(plan.witness(), Some(entry), policy)?;
    let limits = limits.bounded();
    let f = &plan.witness().functions()[entry.0];
    let mut machine = Machine {
        plan,
        frames: Vec::new(),
        limits,
        fuel: limits.fuel,
        next_activation: 1,
        live_slots: 0,
        live_cells: 0,
        live_bytes: 0,
        header_bytes: plan::mul(limits.frames, size_of::<Frame>())?,
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
    let result = (|| {
        machine.activation_preflight(
            entry,
            plan::add(1, plan.function(entry).usage().activation_fuel_cells())?,
            f.span,
        )?;
        machine.frames = plan::reserve(limits.frames)
            .map_err(|error| OwnedRunFailure::from(error).at_runtime(f.span))?;
        let root = Frame::allocate(plan, entry, machine.next_activation, None)
            .map_err(|error| error.at_runtime(f.span))?;
        machine.install(root);
        machine.execute()
    })();
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
        match result? {
            scalar @ Scalar::I32(0..=255) => Ok(scalar),
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
pub(super) fn run_process_limits(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    limits: Limits,
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
    execute_plan_inner(
        &plan,
        entry,
        limits,
        EntryPolicy::Process,
        #[cfg(test)]
        None,
        #[cfg(test)]
        None,
        #[cfg(test)]
        None,
    )
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
        AggregateTy::FixedArray(_) | AggregateTy::Enum(_) => {
            Err(bad("unsupported aggregate carrier", span))
        }
    }
}
