//! Iterative reference execution of the sealed owned witness. No raw entry point.
use super::{
    plan::{self, ExecutionPlan},
    storage::*,
    verified::VerifiedOwnedProgram,
    *,
};
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
        Ok((
            self.plan
                .function(f.id)
                .owner_offset(OwnerPlaceId(key.owner as usize))
                + field.offset(),
            field.ty(),
        ))
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
            AggregateTy::Record(_) => Err(bad("array aggregate type", span)),
        }
    }
    fn array_base(
        &self,
        frame: usize,
        base: AccessBase,
        access: Access,
        span: Span,
    ) -> Result<(OwnerKey, FixedArrayTy)> {
        let key = self.base(frame, base, access, span)?;
        let f = self.function(self.frames[frame].function);
        let expected = match base {
            AccessBase::Owner(owner) => f.owners[owner.0].aggregate(),
            AccessBase::Parameter(reference) => f
                .references
                .get(reference.0)
                .ok_or_else(|| bad("reference declaration", span))?
                .aggregate(),
        };
        let actual = self.aggregate(key, span)?;
        self.plan
            .witness()
            .declarations()
            .same_aggregate_type(actual, expected)
            .map_err(|_| bad("array base type", span))?;
        Ok((key, self.array_type(key, span)?))
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
        if let AggregateTy::FixedArray(array) = record {
            let source = self.owner_extent(from, span)?;
            let destination = self.owner_extent(to, span)?;
            if from.frame == to.frame
                && source.start < destination.end
                && destination.start < source.end
            {
                return Err(bad("overlapping whole transfer", span));
            }
            #[cfg(test)]
            let observation = self.observe_begin(to, StorageObservationKind::Transfer, true, span);
            if array.length() == 0 {
                self.zero_sentinel(to, span)?;
            }
            for ordinal in 0..array.length() {
                let value = self.load_element(from, array, ordinal, span)?;
                self.store_element(to, array, ordinal, value, span)?;
            }
            #[cfg(test)]
            {
                self.record_event(Event::Transfer(from, initialized_destination));
                self.observe_end(observation);
            }
            return Ok(());
        }
        let record = record_type(record, span)?;
        let fields = self.plan.witness().declarations().fields(record).unwrap();
        if fields.is_empty() {
            let source_offset = self
                .plan
                .function(f.id)
                .owner_offset(OwnerPlaceId(from.owner as usize));
            let target_offset = self
                .plan
                .function(self.frames[to.frame as usize].function)
                .owner_offset(OwnerPlaceId(to.owner as usize));
            let byte = self.frames[from.frame as usize].payload[source_offset];
            self.frames[to.frame as usize].payload[target_offset] = byte;
        } else {
            for field in fields {
                let value = self.load_field(from, field.id(), span)?;
                self.store_field(to, field.id(), value, span)?;
            }
        }
        #[cfg(test)]
        self.record_event(Event::Transfer(from, initialized_destination));
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
                Scalar::I32(
                    match op {
                        hir::ArithmeticOp::Add => a.checked_add(b),
                        hir::ArithmeticOp::Subtract => a.checked_sub(b),
                        hir::ArithmeticOp::Multiply => a.checked_mul(b),
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
    fn statement(
        &mut self,
        frame: usize,
        instruction: &OwnedInstruction,
        span: Span,
    ) -> Result<()> {
        let f = self.function(self.frames[frame].function);
        match instruction {
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
                let (key, array) = self.array_base(frame, *base, Access::Read, span)?;
                if f.locals.get(destination.0).map(|local| local.ty) != Some(array.element()) {
                    return Err(bad("array read type", span));
                }
                let ordinal = array_index(self.read(frame, *index)?, array, span)?;
                let value = self.load_element(key, array, ordinal, span)?;
                self.write(frame, *destination, value, span)?;
                #[cfg(test)]
                self.record_event(Event::ReadIndex(key, ordinal, value));
            }
            OwnedInstruction::WriteIndex { base, index, value } => {
                let (key, array) = self.array_base(frame, *base, Access::Write, span)?;
                let value = self.read(frame, *value)?;
                if value.ty() != array.element() {
                    return Err(bad("array write type", span));
                }
                let ordinal = array_index(self.read(frame, *index)?, array, span)?;
                self.store_element(key, array, ordinal, value, span)?;
                #[cfg(test)]
                {
                    self.record_event(Event::WriteIndex(key, ordinal, value));
                    let observation =
                        self.observe_begin(key, StorageObservationKind::IndexWrite, false, span);
                    self.observe_end(observation);
                }
            }
            OwnedInstruction::ArrayLength { destination, base } => {
                let (_key, array) = self.array_base(frame, *base, Access::Read, span)?;
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
                self.base(frame, AccessBase::Owner(*o), Access::Consume, span)?;
                self.expect_owner(frame, *o, &[AVAILABLE], span)?;
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
                    if d.ty() != v.ty() {
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
                    let root_function =
                        self.function(self.frames[handle.root.frame as usize].function);
                    if self
                        .plan
                        .witness()
                        .declarations()
                        .same_aggregate_type(
                            root_function.owners[handle.root.owner as usize].aggregate(),
                            declared.aggregate(),
                        )
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
                    match record {
                        AggregateTy::Record(record) => {
                            let fields = self.plan.witness().declarations().fields(record).unwrap();
                            if fields.is_empty() {
                                child.payload[offset] = 0;
                            }
                            for field in fields {
                                let value = self.load_field(key, field.id(), span)?;
                                encode(&mut child.payload, offset + field.offset(), value, span)?;
                            }
                        }
                        AggregateTy::FixedArray(array) => {
                            let size = self
                                .plan
                                .witness()
                                .declarations()
                                .aggregate_layout(record)
                                .map_err(|_| bad("incoming array layout", span))?
                                .size();
                            let end = offset
                                .checked_add(size)
                                .filter(|end| *end <= child.payload.len())
                                .ok_or_else(|| bad("incoming array range", span))?;
                            #[cfg(test)]
                            let observation = self.observer.begin(
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
                            );
                            if array.length() == 0 {
                                child.payload[offset..end].fill(0);
                            }
                            for ordinal in 0..array.length() {
                                let value = self.load_element(key, array, ordinal, span)?;
                                let element = ordinal
                                    .checked_mul(array.stride())
                                    .and_then(|n| offset.checked_add(n))
                                    .ok_or_else(|| bad("incoming array offset", span))?;
                                element
                                    .checked_add(array.stride())
                                    .filter(|n| *n <= end)
                                    .ok_or_else(|| bad("incoming array range", span))?;
                                encode(&mut child.payload, element, value, span)?;
                            }
                            #[cfg(test)]
                            self.observer.end(&child.payload, observation);
                        }
                    }
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
                self.charge(self.plan.statement_cost(f.id, &statement.kind), span)?;
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
                    if let Some(value) = self.return_value(frame, &end.kind, end.span)? {
                        return Ok(value);
                    }
                }
                OwnedTerminatorKind::Invoke { .. } => unreachable!(),
            }
        }
    }
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
    let entry = entry.ok_or(RunFailure::Entry(None))?;
    let f = witness
        .functions()
        .get(entry.0)
        .filter(|f| f.id == entry)
        .ok_or(OwnedRunFailure::Invariant("entry identity", None))?;
    if !f.parameters.is_empty() {
        return Err(RunFailure::Entry(Some(f.span)).into());
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
    #[cfg(test)] events: Option<&mut Vec<Event>>,
    #[cfg(test)] mut observation: Option<&mut array_observe::Observer>,
    #[cfg(test)] remaining_fuel: Option<&mut usize>,
) -> Result<Scalar> {
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
    let result = (|| {
        machine.activation_preflight(
            entry,
            plan::add(1, plan.function(entry).usage().expanded_cells)?,
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
    result
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

fn record_type(aggregate: AggregateTy, span: Span) -> Result<RecordId> {
    match aggregate {
        AggregateTy::Record(record) => Ok(record),
        AggregateTy::FixedArray(_) => Err(bad("unsupported aggregate carrier", span)),
    }
}
