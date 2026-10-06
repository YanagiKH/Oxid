//! Immutable, witness-bound consumer accounting. This is not a safety witness.
//! Only this module constructs plans; consumers retrieve their witness from it.
use super::{storage::*, verified::VerifiedOwnedProgram, *};
use std::mem::size_of;

pub(super) const ENUM_VALUE_COST: usize = 3;
pub(super) const MATCH_DISPATCH_COST: usize = 1;

pub(super) const MAX_PLAN_BYTES: usize = 32 * 1024 * 1024;
pub(super) const MAX_FUEL: usize = 1_000_000;
pub(super) const MAX_FRAMES: usize = 1_024;
pub(super) const MAX_SCALAR_SLOTS: usize = 200_000;
pub(super) const MAX_EXPANDED_CELLS: usize = 200_000;
pub(super) const MAX_REFERENCE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AdmissionFailure {
    pub name: &'static str,
    pub span: Option<Span>,
}
impl AdmissionFailure {
    pub fn new(name: &'static str, span: Option<Span>) -> Self {
        Self { name, span }
    }
}
pub(super) fn add(a: usize, b: usize) -> Result<usize, AdmissionFailure> {
    a.checked_add(b)
        .ok_or_else(|| AdmissionFailure::new("owned count overflow", None))
}
pub(super) fn mul(a: usize, b: usize) -> Result<usize, AdmissionFailure> {
    a.checked_mul(b)
        .ok_or_else(|| AdmissionFailure::new("owned count overflow", None))
}
fn align(n: usize, alignment: usize) -> Result<usize, AdmissionFailure> {
    Ok(add(n, alignment - 1)? / alignment * alignment)
}
pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, AdmissionFailure> {
    #[cfg(test)]
    allocation_test_point()?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| AdmissionFailure::new("owned allocation", None))?;
    Ok(result)
}
pub(super) fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, AdmissionFailure> {
    let mut result = reserve(count)?;
    result.resize(count, value);
    Ok(result)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct FrameUsage {
    pub scalar_slots: usize,
    pub arguments: usize,
    pub owners: usize,
    pub owner_cells: usize,
    pub payload_bytes: usize,
    pub references: usize,
    pub loans: usize,
    pub calls: usize,
    pub expanded_cells: usize,
    pub reference_bytes: usize,
    pub native_bytes: usize,
}
impl FrameUsage {
    /// Fuel counts stable logical slots, while admission counts physical metadata.
    /// Checked view sidecars add two 64-bit cells per reference/loan, not language work.
    pub(super) fn activation_fuel_cells(self) -> usize {
        self.expanded_cells - 2 * (self.references + self.loans)
    }
}
#[derive(Debug)]
pub(super) struct CallPlan {
    argument_start: usize,
    owned_start: usize,
    owned_len: usize,
    borrow_start: usize,
    borrow_len: usize,
    owned_width: usize,
}
impl CallPlan {
    pub fn argument_start(&self) -> usize {
        self.argument_start
    }
    pub fn owned_width(&self) -> usize {
        self.owned_width
    }
    pub fn borrowed_count(&self) -> usize {
        self.borrow_len
    }
    pub fn owned_count(&self) -> usize {
        self.owned_len
    }
}
#[derive(Debug)]
pub(super) struct FunctionPlan {
    usage: FrameUsage,
    owner_offsets: Vec<usize>,
    calls: Vec<CallPlan>,
    owned_stages: Vec<OwnerPlaceId>,
    borrowed_loans: Vec<LoanId>,
}
impl FunctionPlan {
    pub fn usage(&self) -> FrameUsage {
        self.usage
    }
    pub fn owner_offset(&self, owner: OwnerPlaceId) -> usize {
        self.owner_offsets[owner.0]
    }
    pub fn call(&self, call: CallSiteId) -> &CallPlan {
        &self.calls[call.0]
    }
    pub fn owned_stages(&self, call: CallSiteId) -> &[OwnerPlaceId] {
        let c = self.call(call);
        &self.owned_stages[c.owned_start..c.owned_start + c.owned_len]
    }
    pub fn borrowed_loans(&self, call: CallSiteId) -> &[LoanId] {
        let c = self.call(call);
        &self.borrowed_loans[c.borrow_start..c.borrow_start + c.borrow_len]
    }
}
#[derive(Debug)]
pub(super) struct ExecutionPlan<'a> {
    witness: &'a VerifiedOwnedProgram,
    functions: Vec<FunctionPlan>,
    metadata_bytes: usize,
}
impl<'a> ExecutionPlan<'a> {
    pub fn build(witness: &'a VerifiedOwnedProgram) -> Result<Self, AdmissionFailure> {
        Self::build_with_limit(witness, MAX_PLAN_BYTES)
    }
    /// Test-only lower admission limit; it never raises the production ceiling
    /// or constructs execution authority independently of the supplied witness.
    #[cfg(test)]
    pub(super) fn build_with_test_limit(
        witness: &'a VerifiedOwnedProgram,
        limit: usize,
    ) -> Result<Self, AdmissionFailure> {
        Self::build_with_limit(witness, limit.min(MAX_PLAN_BYTES))
    }
    pub fn witness(&self) -> &'a VerifiedOwnedProgram {
        self.witness
    }
    pub fn function(&self, id: hir::DefId) -> &FunctionPlan {
        &self.functions[id.0]
    }
    pub fn functions(&self) -> &[FunctionPlan] {
        &self.functions
    }
    pub fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    /// Scratch never extends a language owner's nominal extent. It exists only
    /// in the independently verified canonical input activation.
    pub(super) fn input_scratch_range(
        &self,
        function: hir::DefId,
    ) -> Option<std::ops::Range<usize>> {
        if self.witness.builtin_function() != Some(function) {
            return None;
        }
        let end = self.functions.get(function.0)?.usage.payload_bytes;
        let start = end.checked_sub(builtins::INPUT_SCRATCH_BYTES)?;
        Some(start..end)
    }
    /// Output staging belongs only to its own canonical activation, even when
    /// an input activation is present in the same program. This is physical
    /// scratch, outside the language result owner's nominal extent.
    pub(super) fn output_scratch_range(
        &self,
        function: hir::DefId,
    ) -> Option<std::ops::Range<usize>> {
        if self.witness.builtin_output_function() != Some(function) {
            return None;
        }
        let end = self.functions.get(function.0)?.usage.payload_bytes;
        let start = end.checked_sub(builtins::OUTPUT_SCRATCH_BYTES)?;
        Some(start..end)
    }
    pub fn owner_width(&self, f: hir::DefId, o: OwnerPlaceId) -> usize {
        width(self.witness, &self.witness.functions()[f.0], o)
    }
    pub fn statement_cost(&self, f: hir::DefId, instruction: &OwnedInstruction) -> usize {
        // Build preflights every cost with checked arithmetic before this read-only fast path.
        match instruction {
            // The consumer charges the validated capacity and each I/O attempt.
            OwnedInstruction::ReadStdin { .. } | OwnedInstruction::WriteStdout { .. } => 0,
            OwnedInstruction::ConstructEnum { .. } | OwnedInstruction::ConsumeVariant { .. } => {
                ENUM_VALUE_COST
            }
            OwnedInstruction::StorageEnd(o) | OwnedInstruction::Discard(o) => {
                1 + self.owner_width(f, *o)
            }
            OwnedInstruction::Construct { destination, .. }
            | OwnedInstruction::ConstructArray { destination, .. }
            | OwnedInstruction::ConstructComposite { destination, .. } => {
                1 + self.owner_width(f, *destination)
            }
            OwnedInstruction::MoveInitialize { source, .. }
            | OwnedInstruction::PrepareOwned { source, .. } => 1 + self.owner_width(f, *source),
            OwnedInstruction::Replace { source, .. } => 1 + 2 * self.owner_width(f, *source),
            OwnedInstruction::OpenCall(c) => 1 + self.function(f).call(*c).owned_len,
            _ => 1,
        }
    }
    pub fn terminator_cost(&self, f: hir::DefId, terminator: &OwnedTerminatorKind) -> usize {
        let function = &self.witness.functions()[f.0];
        let usage = self.function(f).usage;
        match terminator {
            OwnedTerminatorKind::MatchDispatch { .. } => MATCH_DISPATCH_COST,
            OwnedTerminatorKind::Invoke { call, .. } => {
                let descriptor = &function.calls[call.0];
                let c = self.function(f).call(*call);
                1 + descriptor.arguments.len()
                    + self
                        .function(descriptor.target)
                        .usage
                        .activation_fuel_cells()
                    + c.owned_width
                    + c.borrow_len * c.borrow_len.saturating_sub(1) / 2
            }
            OwnedTerminatorKind::ReturnScalar(_) | OwnedTerminatorKind::ReturnOwned(_) => {
                1 + usage.owner_cells
                    + usage.loans
                    + usage.calls
                    + usage.references
                    + match terminator {
                        OwnedTerminatorKind::ReturnOwned(o) => self.owner_width(f, *o),
                        _ => 0,
                    }
            }
            _ => 1,
        }
    }
    fn build_with_limit(
        witness: &'a VerifiedOwnedProgram,
        limit: usize,
    ) -> Result<Self, AdmissionFailure> {
        let functions = witness.functions();
        let mut metadata_bytes = mul(functions.len(), size_of::<FunctionPlan>())?;
        let mut largest_frame = 0;
        // Complete ALL metadata-length and activation-cost preflights before any allocation.
        for f in functions {
            metadata_bytes = add(metadata_bytes, mul(f.owners.len(), size_of::<usize>())?)?;
            metadata_bytes = add(metadata_bytes, mul(f.calls.len(), size_of::<CallPlan>())?)?;
            for c in &f.calls {
                for arg in &c.arguments {
                    metadata_bytes = add(
                        metadata_bytes,
                        match arg {
                            ArgumentSlot::Owned(_) => size_of::<OwnerPlaceId>(),
                            ArgumentSlot::Borrow(_) => size_of::<LoanId>(),
                            ArgumentSlot::Scalar => 0,
                        },
                    )?;
                }
            }
            largest_frame = largest_frame.max(usage(witness, f)?.expanded_cells);
        }
        // Conservative maxima preflight all operation-cost additions before reservation.
        for f in functions {
            let u = usage(witness, f)?;
            add(
                add(add(add(1, mul(2, u.owner_cells)?)?, u.loans)?, u.calls)?,
                u.references,
            )?;
            for c in &f.calls {
                let n = c.arguments.len();
                add(
                    add(add(add(1, n)?, largest_frame)?, u.owner_cells)?,
                    mul(n, n.saturating_sub(1))? / 2,
                )?;
            }
        }
        if metadata_bytes > limit.min(MAX_PLAN_BYTES) {
            return Err(AdmissionFailure::new("owned plan bytes", None));
        }
        let mut plans = reserve(functions.len())?;
        for f in functions {
            let current_usage = usage(witness, f)?;
            let mut owner_offsets = reserve(f.owners.len())?;
            let mut next = 0;
            for owner in &f.owners {
                let layout = witness
                    .declarations()
                    .aggregate_layout(owner.aggregate())
                    .expect("verified record");
                next = align(next, layout.align())?;
                owner_offsets.push(next);
                next = add(next, layout.size())?;
            }
            let owned_count = f
                .calls
                .iter()
                .flat_map(|c| &c.arguments)
                .filter(|a| matches!(a, ArgumentSlot::Owned(_)))
                .count();
            let borrow_count = f
                .calls
                .iter()
                .flat_map(|c| &c.arguments)
                .filter(|a| matches!(a, ArgumentSlot::Borrow(_)))
                .count();
            let mut calls = reserve(f.calls.len())?;
            let mut owned_stages = reserve(owned_count)?;
            let mut borrowed_loans = reserve(borrow_count)?;
            let mut argument_start = 0;
            for c in &f.calls {
                let (owned_start, borrow_start) = (owned_stages.len(), borrowed_loans.len());
                let mut owned_width = 0;
                for a in &c.arguments {
                    match a {
                        ArgumentSlot::Owned(o) => {
                            owned_stages.push(*o);
                            owned_width = add(owned_width, width(witness, f, *o))?;
                        }
                        ArgumentSlot::Borrow(l) => borrowed_loans.push(*l),
                        ArgumentSlot::Scalar => {}
                    }
                }
                calls.push(CallPlan {
                    argument_start,
                    owned_start,
                    owned_len: owned_stages.len() - owned_start,
                    borrow_start,
                    borrow_len: borrowed_loans.len() - borrow_start,
                    owned_width,
                });
                argument_start = add(argument_start, c.arguments.len())?;
            }
            plans.push(FunctionPlan {
                usage: current_usage,
                owner_offsets,
                calls,
                owned_stages,
                borrowed_loans,
            });
        }
        let result = Self {
            witness,
            functions: plans,
            metadata_bytes,
        };
        // Sum the maximal components, rather than trusting an unchecked cost expression.
        for f in functions {
            let u = result.function(f.id).usage;
            add(
                add(add(add(1, u.owner_cells)?, u.loans)?, u.calls)?,
                u.references,
            )?;
            for owner in 0..f.owners.len() {
                add(1, mul(2, result.owner_width(f.id, OwnerPlaceId(owner)))?)?;
            }
            for c in &f.calls {
                let mut cost = add(1, c.arguments.len())?;
                cost = add(
                    cost,
                    result.function(c.target).usage.activation_fuel_cells(),
                )?;
                let mut borrows: usize = 0;
                for a in &c.arguments {
                    match a {
                        ArgumentSlot::Owned(o) => cost = add(cost, width(witness, f, *o))?,
                        ArgumentSlot::Borrow(_) => borrows = add(borrows, 1)?,
                        _ => {}
                    }
                }
                add(cost, mul(borrows, borrows.saturating_sub(1))? / 2)?;
            }
        }
        Ok(result)
    }
}
fn width(witness: &VerifiedOwnedProgram, f: &RawOwnedFunction, o: OwnerPlaceId) -> usize {
    witness
        .declarations()
        .aggregate_width(f.owners[o.0].aggregate())
        .expect("verified record")
}
fn usage(
    witness: &VerifiedOwnedProgram,
    f: &RawOwnedFunction,
) -> Result<FrameUsage, AdmissionFailure> {
    let mut scratch_bytes = 0;
    if witness.builtin_function() == Some(f.id) {
        scratch_bytes = add(scratch_bytes, builtins::INPUT_SCRATCH_BYTES)?;
    }
    if witness.builtin_output_function() == Some(f.id) {
        scratch_bytes = add(scratch_bytes, builtins::OUTPUT_SCRATCH_BYTES)?;
    }
    frame_usage(witness.declarations(), f, scratch_bytes)
}

/// Physical accounting over declarations established by the plan's witness.
fn frame_usage(
    declarations: &Declarations,
    f: &RawOwnedFunction,
    scratch_bytes: usize,
) -> Result<FrameUsage, AdmissionFailure> {
    let mut u = FrameUsage {
        scalar_slots: add(f.locals.len(), f.places.len())?,
        owners: f.owners.len(),
        references: f.references.len(),
        loans: f.loans.len(),
        calls: f.calls.len(),
        ..FrameUsage::default()
    };
    for c in &f.calls {
        u.arguments = add(u.arguments, c.arguments.len())?;
    }
    for owner in &f.owners {
        let layout = declarations
            .aggregate_layout(owner.aggregate())
            .expect("verified record");
        u.owner_cells = add(
            u.owner_cells,
            declarations
                .aggregate_width(owner.aggregate())
                .expect("verified record"),
        )?;
        u.payload_bytes = add(align(u.payload_bytes, layout.align())?, layout.size())?;
    }
    u.payload_bytes = add(u.payload_bytes, scratch_bytes)?;
    u.expanded_cells = add(add(u.scalar_slots, u.arguments)?, u.owner_cells)?;
    u.reference_bytes = add(
        mul(
            add(u.scalar_slots, u.arguments)?,
            size_of::<Option<Scalar>>(),
        )?,
        u.payload_bytes,
    )?;
    for (count, cells, bytes) in [
        (u.owners, 4, size_of::<OwnerRuntime>()),
        (u.references, 10, size_of::<ReferenceHandle>()),
        (u.loans, 14, size_of::<LoanRuntime>()),
        (u.calls, 2, size_of::<CallRuntime>()),
    ] {
        u.expanded_cells = add(u.expanded_cells, mul(count, cells)?)?;
        u.reference_bytes = add(u.reference_bytes, mul(count, bytes)?)?;
    }
    u.native_bytes = add(
        add(
            mul(add(u.scalar_slots, u.arguments)?, 8)?,
            mul(add(u.references, u.loans)?, 8)?,
        )?,
        align(u.payload_bytes, 4)?,
    )?;
    // Slice descriptors add native-only i32 length slots. The reference consumer
    // derives length from validated whole-owner provenance and stores no sidecar.
    let slice_references = f
        .references
        .iter()
        .filter(|r| matches!(r.referent(), BorrowedTy::ScalarSlice(_)))
        .count();
    let slice_loans = f
        .loans
        .iter()
        .filter(|l| matches!(l.referent(), BorrowedTy::ScalarSlice(_)))
        .count();
    u.native_bytes = add(u.native_bytes, mul(add(slice_references, slice_loans)?, 4)?)?;
    Ok(u)
}

pub(super) fn instruction_span(statement: &OwnedStatement) -> Span {
    match &statement.kind {
        OwnedInstruction::Scalar(s) => s.span(),
        OwnedInstruction::ReadIndex { .. }
        | OwnedInstruction::WriteIndex { .. }
        | OwnedInstruction::ArrayLength { .. } => statement.primary_span(),
        _ => statement.span,
    }
}

#[cfg(test)]
thread_local! { static ALLOCATION_FAILURE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
fn allocation_test_point() -> Result<(), AdmissionFailure> {
    ALLOCATION_FAILURE.with(|p| match p.get() {
        Some(0) => Err(AdmissionFailure::new(
            "injected owned allocation failure",
            None,
        )),
        Some(n) => {
            p.set(Some(n - 1));
            Ok(())
        }
        None => Ok(()),
    })
}
#[cfg(test)]
pub(super) fn fail_allocation_after<T>(count: usize, action: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ALLOCATION_FAILURE.with(|p| p.set(None));
        }
    }
    ALLOCATION_FAILURE.with(|p| p.set(Some(count)));
    let _reset = Reset;
    action()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregate_seam_retains_raw_and_runtime_representation() {
        macro_rules! sizes { ($($ty:ty => $bytes:expr),* $(,)?) => { $(
            println!("layout {} {}", stringify!($ty), size_of::<$ty>());
            #[cfg(target_pointer_width = "64")]
            assert_eq!(size_of::<$ty>(), $bytes, "{} accounting changed", stringify!($ty));
        )* }; }
        sizes!(
            AggregateSlot => 8,
            AggregateTy => 16,
            ValueTy => 16,
            Option<ValueTy> => 16,
            ParameterTy => 24,
            Option<ParameterTy> => 24,
            RawOwnedProgram => 56 + size_of::<Vec<RawEnumDecl>>(),
            RawOwnedFunction => 248 + size_of::<Vec<MatchDecl>>(),
            MatchDecl => 56,
            MatchArm => 32,
            shape::MatchBlockRole => 24,
            OwnerDecl => 56,
            ReferenceDecl => 48,
            LoanDecl => 96,
            CallDecl => 96,
            OwnedInstruction => 128,
            OwnedStatement => 208,
            OwnedBlock => 328,
            Operand => 32,
            ParameterBinding => 16,
            FunctionPlan => 184,
            CallPlan => 48,
            FrameUsage => 88,
            OwnerRuntime => 32,
            ReferenceHandle => 80,
            LoanRuntime => 112,
            CallRuntime => 16,
            flow::DenialContext => 136,
            flow::DenialFacts => 136,
            flow::OwnerSubject => 64,
            flow::DeniedSubject => 64,
            OwnedFailure => 240
        );
    }
    #[test]
    fn checked_arithmetic_and_plan_preflight_precede_allocation() {
        assert!(add(usize::MAX, 1).is_err());
        assert!(mul(usize::MAX, 2).is_err());
        assert!(align(usize::MAX, 4).is_err());
        let (sources, raw, _) = super::super::consumer_fixtures::empty_record();
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let bytes = ExecutionPlan::build(&witness).unwrap().metadata_bytes();
        assert!(ExecutionPlan::build_with_limit(&witness, bytes).is_ok());
        let failure =
            fail_allocation_after(0, || ExecutionPlan::build_with_limit(&witness, bytes - 1))
                .unwrap_err();
        assert_eq!(failure.name, "owned plan bytes");
        let mut failures = 0;
        loop {
            match fail_allocation_after(failures, || ExecutionPlan::build(&witness)) {
                Ok(_) => break,
                Err(e) => assert_eq!(e.name, "injected owned allocation failure"),
            }
            failures += 1;
        }
        assert_eq!(failures, 5); // function plans + four per-function arrays
    }
}
