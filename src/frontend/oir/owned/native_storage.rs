//! Checked description of the existing LLVM arenas. No allocation or admission
//! authority is created here. The enclosing execution plan owns every table.
use super::*;
use std::ops::Range;

pub(in super::super) struct NativeStoragePlan<'p, 'w> {
    execution: &'p ExecutionPlan<'w>,
    guarded: bool,
}

/// Fixed, borrowed view, derived one function at a time. There is no F-sized
/// native table alongside the execution plan.
pub(in super::super) struct NativeFunctionStorage<'p, 'w> {
    execution: &'p ExecutionPlan<'w>,
    id: hir::DefId,
    scalar_slots: usize,
    owner_bytes: usize,
    reference_slots: usize,
    slice_slots: usize,
}

fn require(ok: bool) -> Result<(), AdmissionFailure> {
    if ok {
        Ok(())
    } else {
        Err(AdmissionFailure::new("native storage inventory", None))
    }
}

impl<'p, 'w> NativeStoragePlan<'p, 'w> {
    pub(in super::super) fn checked(
        execution: &'p ExecutionPlan<'w>,
        guarded: bool,
    ) -> Result<Self, AdmissionFailure> {
        require(execution.functions.len() == execution.witness.functions().len())?;
        for (i, f) in execution.witness.functions().iter().enumerate() {
            require(f.id == hir::DefId(i))?;
            let storage = NativeFunctionStorage::derive(execution, f.id)?;
            storage.check()?;
        }
        Ok(Self { execution, guarded })
    }

    pub(in super::super) fn execution(&self) -> &'p ExecutionPlan<'w> {
        self.execution
    }

    pub(in super::super) fn guarded(&self) -> bool {
        self.guarded
    }

    pub(in super::super) fn wrapper_fuel_bytes(&self) -> usize {
        if self.guarded {
            8
        } else {
            0
        }
    }

    pub(in super::super) fn function(&self, id: hir::DefId) -> NativeFunctionStorage<'p, 'w> {
        // Immutable tables: all derivation arithmetic was checked above.
        NativeFunctionStorage::derive(self.execution, id).expect("checked native storage")
    }
}

impl<'p, 'w> NativeFunctionStorage<'p, 'w> {
    fn derive(execution: &'p ExecutionPlan<'w>, id: hir::DefId) -> Result<Self, AdmissionFailure> {
        let f = execution
            .witness
            .functions()
            .get(id.0)
            .ok_or_else(|| AdmissionFailure::new("native storage function", None))?;
        let p = execution
            .functions
            .get(id.0)
            .ok_or_else(|| AdmissionFailure::new("native storage function plan", None))?;
        let slices = f
            .references
            .iter()
            .filter(|r| matches!(r.referent(), BorrowedTy::ScalarSlice(_)))
            .count();
        let loans = f
            .loans
            .iter()
            .filter(|l| matches!(l.referent(), BorrowedTy::ScalarSlice(_)))
            .count();
        Ok(Self {
            execution,
            id,
            scalar_slots: add(p.usage.scalar_slots, p.usage.arguments)?,
            owner_bytes: align(p.usage.payload_bytes, 4)?,
            reference_slots: add(p.usage.references, p.usage.loans)?,
            slice_slots: add(slices, loans)?,
        })
    }

    /// Reconcile from declarations rather than accepting the usage totals as
    /// proof. Exact sequential ranges also prove non-overlap and no omitted tail.
    fn check(&self) -> Result<(), AdmissionFailure> {
        let f = self.raw();
        let p = &self.execution.functions[self.id.0];
        let u = p.usage;
        require(u.scalar_slots == add(f.locals.len(), f.places.len())?)?;
        require(u.owners == f.owners.len() && p.owner_offsets.len() == f.owners.len())?;
        require(u.references == f.references.len() && u.loans == f.loans.len())?;
        require(u.calls == f.calls.len() && p.calls.len() == f.calls.len())?;
        let mut end = 0;
        let mut cells = 0;
        for (i, owner) in f.owners.iter().enumerate() {
            let layout = self
                .execution
                .witness
                .declarations()
                .aggregate_layout(owner.aggregate())
                .map_err(|_| AdmissionFailure::new("native storage aggregate", Some(owner.span)))?;
            end = align(end, layout.align())?;
            require(p.owner_offsets[i] == end)?;
            end = add(end, layout.size())?;
            cells = add(
                cells,
                self.execution
                    .witness
                    .declarations()
                    .aggregate_width(owner.aggregate())
                    .map_err(|_| AdmissionFailure::new("native storage width", Some(owner.span)))?,
            )?;
        }
        require(u.owner_cells == cells)?;
        let input = self.execution.witness.builtin_function() == Some(self.id);
        let output = self.execution.witness.builtin_output_function() == Some(self.id);
        // Canonical builtin activations are distinct; no shared scratch identity.
        require(!(input && output))?;
        for (present, bytes, range) in [
            (
                input,
                builtins::INPUT_SCRATCH_BYTES,
                self.execution.input_scratch_range(self.id),
            ),
            (
                output,
                builtins::OUTPUT_SCRATCH_BYTES,
                self.execution.output_scratch_range(self.id),
            ),
        ] {
            if present {
                let next = add(end, bytes)?;
                require(range == Some(end..next))?;
                end = next;
            } else {
                require(range.is_none())?;
            }
        }
        require(end == u.payload_bytes && align(end, 4)? == self.owner_bytes)?;
        let (mut arguments, mut owned, mut borrowed) = (0, 0, 0);
        for (call, cp) in f.calls.iter().zip(&p.calls) {
            require(
                cp.argument_start == arguments
                    && cp.owned_start == owned
                    && cp.borrow_start == borrowed,
            )?;
            let (old_owned, old_borrowed) = (owned, borrowed);
            let mut width = 0;
            for argument in &call.arguments {
                match argument {
                    ArgumentSlot::Scalar => {}
                    ArgumentSlot::Owned(o) => {
                        require(p.owned_stages.get(owned) == Some(o))?;
                        let owner = f
                            .owners
                            .get(o.0)
                            .ok_or_else(|| AdmissionFailure::new("native storage stage", None))?;
                        width = add(
                            width,
                            self.execution
                                .witness
                                .declarations()
                                .aggregate_width(owner.aggregate())
                                .map_err(|_| {
                                    AdmissionFailure::new("native storage stage width", None)
                                })?,
                        )?;
                        owned = add(owned, 1)?;
                    }
                    ArgumentSlot::Borrow(l) => {
                        require(p.borrowed_loans.get(borrowed) == Some(l) && l.0 < f.loans.len())?;
                        borrowed = add(borrowed, 1)?;
                    }
                }
            }
            require(
                cp.owned_len == owned - old_owned
                    && cp.borrow_len == borrowed - old_borrowed
                    && cp.owned_width == width,
            )?;
            arguments = add(arguments, call.arguments.len())?;
        }
        require(
            arguments == u.arguments
                && owned == p.owned_stages.len()
                && borrowed == p.borrowed_loans.len(),
        )?;
        require(self.scalar_slots == add(add(f.locals.len(), f.places.len())?, arguments)?)?;
        require(self.reference_slots == add(f.references.len(), f.loans.len())?)?;
        // Slice lengths have no holes: reference declaration order, then loans.
        let mut slices = 0;
        for ty in f
            .references
            .iter()
            .map(|r| r.referent())
            .chain(f.loans.iter().map(|l| l.referent()))
        {
            if matches!(ty, BorrowedTy::ScalarSlice(_)) {
                slices = add(slices, 1)?;
            }
        }
        require(slices == self.slice_slots)?;
        require(self.native_bytes()? == u.native_bytes)?;
        Ok(())
    }

    pub(in super::super) fn execution(&self) -> &'p ExecutionPlan<'w> {
        self.execution
    }
    pub(in super::super) fn id(&self) -> hir::DefId {
        self.id
    }
    pub(in super::super) fn raw(&self) -> &'w RawOwnedFunction {
        &self.execution.witness.functions()[self.id.0]
    }
    pub(in super::super) fn scalar_slots(&self) -> usize {
        self.scalar_slots
    }
    pub(in super::super) fn owner_bytes(&self) -> usize {
        self.owner_bytes
    }
    pub(in super::super) fn reference_slots(&self) -> usize {
        self.reference_slots
    }
    pub(in super::super) fn slice_slots(&self) -> usize {
        self.slice_slots
    }
    pub(in super::super) fn native_bytes(&self) -> Result<usize, AdmissionFailure> {
        add(
            add(mul(self.scalar_slots, 8)?, self.owner_bytes)?,
            add(mul(self.reference_slots, 8)?, mul(self.slice_slots, 4)?)?,
        )
    }
    pub(in super::super) fn scalar_offset(&self, slot: usize) -> usize {
        assert!(slot < self.scalar_slots);
        slot * 8
    }
    pub(in super::super) fn reference_offset(&self, slot: usize) -> usize {
        assert!(slot < self.reference_slots);
        slot * 8
    }
    pub(in super::super) fn slice_offset(&self, slot: usize) -> usize {
        assert!(slot < self.slice_slots);
        slot * 4
    }
    pub(in super::super) fn owner_offset(&self, owner: OwnerPlaceId) -> usize {
        self.execution.functions[self.id.0].owner_offsets[owner.0]
    }
    pub(in super::super) fn input_scratch(&self) -> Option<Range<usize>> {
        self.execution.input_scratch_range(self.id)
    }
    pub(in super::super) fn output_scratch(&self) -> Option<Range<usize>> {
        self.execution.output_scratch_range(self.id)
    }
    pub(in super::super) fn place_slot(&self, place: PlaceId) -> usize {
        assert!(place.0 < self.raw().places.len());
        self.raw().locals.len() + place.0
    }
    pub(in super::super) fn loan_slot(&self, loan: LoanId) -> usize {
        assert!(loan.0 < self.raw().loans.len());
        self.raw().references.len() + loan.0
    }
    pub(in super::super) fn argument_slot(&self, call: CallSiteId, argument: usize) -> usize {
        assert!(argument < self.raw().calls[call.0].arguments.len());
        let p = &self.execution.functions[self.id.0];
        p.usage.scalar_slots + p.calls[call.0].argument_start + argument
    }
}
