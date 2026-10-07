//! Checked description of the existing LLVM arenas. No allocation or admission
//! authority is created here. The enclosing execution plan owns every table.
use super::*;
use std::ops::Range;

// Subdivision of the existing 32768-byte emitter transient envelope, not an
// additional resource charge or changed ceiling. Tests measure every new fixed
// descriptor/return/iterator role plus a conservative scalar-counter reserve.
pub(in super::super) const FIXED_CARRIER_ALLOWANCE: usize = 1024;

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

/// Dense physical length slots retain the original logical declaration names.
/// Exact references/loans yield an empty offset so emitter visit order is stable.
pub(in super::super) struct NativeSliceMapping {
    pub(in super::super) loan: bool,
    pub(in super::super) logical: usize,
    pub(in super::super) offset: Option<usize>,
}

pub(in super::super) struct NativeSliceMappings<'w> {
    function: &'w RawOwnedFunction,
    pointer: usize,
    length: usize,
}

impl Iterator for NativeSliceMappings<'_> {
    type Item = NativeSliceMapping;
    fn next(&mut self) -> Option<Self::Item> {
        let (loan, logical, referent) = if self.pointer < self.function.references.len() {
            (
                false,
                self.pointer,
                self.function.references[self.pointer].referent(),
            )
        } else {
            let logical = self.pointer - self.function.references.len();
            (true, logical, self.function.loans.get(logical)?.referent())
        };
        self.pointer += 1;
        let offset = if matches!(referent, BorrowedTy::ScalarSlice(_)) {
            let offset = self
                .length
                .checked_mul(4)
                .expect("checked native length offsets");
            self.length += 1;
            Some(offset)
        } else {
            None
        };
        Some(NativeSliceMapping {
            loan,
            logical,
            offset,
        })
    }
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
        // Keep the common physical accounting and logical fuel representation
        // independently reconciled, without replacing their admission policy.
        let scalar = add(u.scalar_slots, arguments)?;
        let mut expanded = add(scalar, cells)?;
        let mut reference_bytes = add(mul(scalar, size_of::<Option<Scalar>>())?, end)?;
        for (count, width, bytes) in [
            (f.owners.len(), 4, size_of::<OwnerRuntime>()),
            (f.references.len(), 10, size_of::<ReferenceHandle>()),
            (f.loans.len(), 14, size_of::<LoanRuntime>()),
            (f.calls.len(), 2, size_of::<CallRuntime>()),
        ] {
            expanded = add(expanded, mul(count, width)?)?;
            reference_bytes = add(reference_bytes, mul(count, bytes)?)?;
        }
        require(u.expanded_cells == expanded && u.reference_bytes == reference_bytes)?;
        self.check_slice_mappings(self.slice_mappings())
    }

    fn check_slice_mappings(
        &self,
        mut mappings: NativeSliceMappings<'_>,
    ) -> Result<(), AdmissionFailure> {
        require(
            std::ptr::eq(mappings.function, self.raw())
                && mappings.pointer == 0
                && mappings.length == 0,
        )?;
        let mut next = 0;
        for (loan, count) in [
            (false, self.raw().references.len()),
            (true, self.raw().loans.len()),
        ] {
            for logical in 0..count {
                let ty = if loan {
                    self.raw().loans[logical].referent()
                } else {
                    self.raw().references[logical].referent()
                };
                let expected = if matches!(ty, BorrowedTy::ScalarSlice(_)) {
                    let offset = mul(next, 4)?;
                    next = add(next, 1)?;
                    Some(offset)
                } else {
                    None
                };
                let row = mappings
                    .next()
                    .ok_or_else(|| AdmissionFailure::new("native slice mapping", None))?;
                require(row.loan == loan && row.logical == logical && row.offset == expected)?;
            }
        }
        require(mappings.next().is_none() && next == self.slice_slots)
    }

    pub(in super::super) fn slice_mappings(&self) -> NativeSliceMappings<'w> {
        NativeSliceMappings {
            function: self.raw(),
            pointer: 0,
            length: 0,
        }
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

// Phase 2 counting precursor. These fixed facts are test-only until the
// independently reviewed admission successor is enabled. They confer no
// emission authority and do not change the production X gate.
#[cfg(test)]
pub(in super::super) struct NativeInventories {
    items: usize,
    owner_width: usize,
}

#[cfg(test)]
impl NativeInventories {
    pub(in super::super) fn checked(
        execution: &ExecutionPlan<'_>,
    ) -> Result<Self, AdmissionFailure> {
        let mut inventory = Self {
            items: 0,
            owner_width: 0,
        };
        for function in &execution.functions {
            let usage = function.usage;
            for count in [
                usage.scalar_slots,
                usage.arguments,
                usage.owners,
                usage.references,
                usage.loans,
                usage.calls,
            ] {
                inventory.items = add(inventory.items, count)?;
            }
            inventory.owner_width = add(inventory.owner_width, usage.owner_cells)?;
        }
        inventory.check(execution)?;
        Ok(inventory)
    }

    // Independently count the immutable declarations, not another combination
    // of cached FrameUsage fields. The whole-program totals include unused and
    // untaken declarations; an argument descriptor counts even when its native
    // scalar arena position is an owned/borrow hole.
    fn check(&self, execution: &ExecutionPlan<'_>) -> Result<(), AdmissionFailure> {
        require(execution.functions.len() == execution.witness.functions().len())?;
        let (mut items, mut owner_width) = (0, 0);
        for (index, function) in execution.witness.functions().iter().enumerate() {
            require(function.id == hir::DefId(index))?;
            for count in [
                function.locals.len(),
                function.places.len(),
                function.owners.len(),
                function.references.len(),
                function.loans.len(),
                function.calls.len(),
            ] {
                items = add(items, count)?;
            }
            for call in &function.calls {
                items = add(items, call.arguments.len())?;
            }
            for owner in &function.owners {
                owner_width = add(
                    owner_width,
                    execution
                        .witness
                        .declarations()
                        .aggregate_width(owner.aggregate())
                        .map_err(|_| {
                            AdmissionFailure::new("native inventory owner width", Some(owner.span))
                        })?,
                )?;
            }
        }
        if (self.items, self.owner_width) != (items, owner_width) {
            return Err(AdmissionFailure::new(
                "native compiler inventory mismatch",
                None,
            ));
        }
        Ok(())
    }

    pub(in super::super) fn items(&self) -> usize {
        self.items
    }
    pub(in super::super) fn owner_width(&self) -> usize {
        self.owner_width
    }
}

#[cfg(test)]
#[path = "native_inventory_tests.rs"]
pub(in super::super) mod inventory_tests;

#[cfg(test)]
#[path = "native_storage_tests.rs"]
mod tests;
