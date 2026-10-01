use super::*;
use std::mem::size_of;
#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub owners: usize,
    pub events: usize,
    pub work: usize,
    pub scratch: usize,
    pub metadata: usize,
}
impl Limits {
    pub const DEFAULT: Self = Self {
        owners: MAX_LOCALS,
        events: MAX_ASSIGNMENTS,
        work: 100_000_000,
        scratch: 32 * 1024 * 1024,
        metadata: 32 * 1024 * 1024,
    };
    fn bounded(self) -> Self {
        let d = Self::DEFAULT;
        Self {
            owners: self.owners.min(d.owners),
            events: self.events.min(d.events),
            work: self.work.min(d.work),
            scratch: self.scratch.min(d.scratch),
            metadata: self.metadata.min(d.metadata),
        }
    }
}
pub(super) fn add(a: usize, b: usize) -> Result<usize, OwnedFailure> {
    a.checked_add(b)
        .ok_or_else(|| OwnedFailure::resource("count overflow"))
}
pub(super) fn mul(a: usize, b: usize) -> Result<usize, OwnedFailure> {
    a.checked_mul(b)
        .ok_or_else(|| OwnedFailure::resource("count overflow"))
}
fn cap(value: usize, limit: usize, name: &'static str) -> Result<usize, OwnedFailure> {
    if value > limit {
        Err(OwnedFailure::resource(name))
    } else {
        Ok(value)
    }
}
pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, OwnedFailure> {
    #[cfg(test)]
    allocation_test_point()?;
    let mut v = Vec::new();
    v.try_reserve_exact(count)
        .map_err(|_| OwnedFailure::resource("allocation"))?;
    Ok(v)
}
pub(super) fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, OwnedFailure> {
    let mut v = reserve(count)?;
    v.resize(count, value);
    Ok(v)
}
pub(super) fn active(f: &RawOwnedFunction) -> bool {
    !f.owners.is_empty()
        || !f.references.is_empty()
        || !f.calls.is_empty()
        || !f.loans.is_empty()
        || f.blocks.iter().any(|b| {
            b.statements
                .iter()
                .any(|s| !matches!(s.kind, OwnedInstruction::Scalar(_)))
                || matches!(
                    b.terminator.as_ref().map(|e| &e.kind),
                    Some(OwnedTerminatorKind::ReturnOwned(_) | OwnedTerminatorKind::Invoke { .. })
                )
        })
}
/// Preflight raw nested lengths before validation or analysis allocations.
/// Work units are bounded inventory/statement/block/edge visits, not CPU instructions.
pub(super) fn preflight(
    raw: &RawOwnedProgram,
    limits: Limits,
) -> Result<OwnershipUsage, OwnedFailure> {
    let limits = limits.bounded();
    cap(raw.functions.len(), MAX_BLOCKS, "functions")?;
    let mut u = OwnershipUsage::default();
    let (mut slots, mut blocks, mut statements) = (0, 0, 0);
    for f in &raw.functions {
        slots = cap(
            add(
                slots,
                add(add(f.locals.len(), f.places.len())?, f.owners.len())?,
            )?,
            MAX_LOCALS,
            "locals",
        )?;
        blocks = cap(add(blocks, f.blocks.len())?, MAX_BLOCKS, "blocks")?;
        cap(
            f.parameters.len(),
            super::super::super::parser::MAX_PARAMS,
            "parameters",
        )?;
        let (mut s, mut e, mut a, mut fields) = (0, 0, 0, 0);
        for c in &f.calls {
            cap(
                c.arguments.len(),
                super::super::super::parser::MAX_PARAMS,
                "call arguments",
            )?;
            a = add(a, c.arguments.len())?;
        }
        let descriptor_args = a;
        let mut ownership = false;
        let mut field_scratch = 0;
        for b in &f.blocks {
            s = add(s, add(b.statements.len(), usize::from(b.merge.is_some()))?)?;
            e = add(
                e,
                match b.terminator.as_ref().map(|e| &e.kind) {
                    Some(OwnedTerminatorKind::Branch { .. }) => 2,
                    Some(OwnedTerminatorKind::Goto(_) | OwnedTerminatorKind::Invoke { .. }) => 1,
                    _ => 0,
                },
            )?;
            ownership |= matches!(
                b.terminator.as_ref().map(|e| &e.kind),
                Some(OwnedTerminatorKind::Invoke { .. } | OwnedTerminatorKind::ReturnOwned(_))
            );
            for i in &b.statements {
                ownership |= !matches!(i.kind, OwnedInstruction::Scalar(_));
                match &i.kind {
                    OwnedInstruction::Construct { fields: values, .. } => {
                        fields = add(fields, values.len())?;
                        field_scratch = field_scratch.max(values.len().min(1024));
                    }
                    OwnedInstruction::PrepareScalar { .. }
                    | OwnedInstruction::PrepareOwned { .. }
                    | OwnedInstruction::PrepareBorrow { .. } => a = add(a, 1)?,
                    _ => {}
                }
            }
        }
        statements = cap(add(statements, s)?, MAX_ASSIGNMENTS, "assignments")?;
        if !ownership
            && f.owners.is_empty()
            && f.references.is_empty()
            && f.calls.is_empty()
            && f.loans.is_empty()
        {
            continue;
        }
        u.owners = cap(add(u.owners, f.owners.len())?, limits.owners, "owners")?;
        u.expanded_events = cap(
            add(u.expanded_events, add(add(s, a)?, fields)?)?,
            limits.events,
            "expanded ownership events",
        )?;
        let mut n = 1;
        for count in [
            mul(2, f.blocks.len())?,
            e,
            s,
            a,
            fields,
            f.owners.len(),
            f.loans.len(),
            f.calls.len(),
            f.parameters.len(),
            f.references.len(),
            f.locals.len(),
            f.places.len(),
        ] {
            n = add(n, count)?;
        }
        let multiplier = add(
            add(add(mul(4, f.owners.len())?, f.loans.len())?, f.calls.len())?,
            32,
        )?;
        u.work = cap(
            add(u.work, mul(multiplier, n)?)?,
            limits.work,
            "ownership work",
        )?;
        // Transient derived metadata is dropped before final witness construction.
        // Descriptor payload is charged even when it already resides in caller-owned raw input.
        let mut metadata = 0;
        for (count, size) in [
            (f.owners.len(), size_of::<shape::OwnerSites>()),
            (f.calls.len(), size_of::<shape::CallSites>()),
            (descriptor_args, size_of::<Option<shape::Site>>()),
            (f.loans.len(), size_of::<Option<shape::Site>>()),
            (
                f.calls.len(),
                2 * size_of::<usize>() + size_of::<(usize, bool)>(),
            ),
            (f.parameters.len(), size_of::<ParameterBinding>()),
            (f.references.len(), size_of::<ReferenceDecl>()),
            (f.loans.len(), size_of::<LoanDecl>()),
            (fields, size_of::<(FieldId, Operand)>()),
        ] {
            metadata = add(metadata, mul(count, size)?)?;
        }
        u.metadata_bytes = cap(
            add(u.metadata_bytes, metadata)?,
            limits.metadata,
            "ownership metadata",
        )?;
        // Availability queue is explicitly dropped before failure reconstruction.
        // Reconstruction retains one seen byte per block, CSR offsets/edges,
        // then either insertion cursors or (visited bytes + B-entry queue).
        let availability = mul(f.blocks.len(), 1 + 4 * size_of::<usize>())?;
        let reconstruction = add(
            mul(2, f.blocks.len())?,
            mul(
                add(add(mul(2, f.blocks.len())?, 1)?, e)?,
                size_of::<usize>(),
            )?,
        )?;
        let scratch = availability.max(reconstruction).max(field_scratch);
        u.scratch_bytes = cap(
            u.scratch_bytes.max(scratch),
            limits.scratch,
            "ownership scratch",
        )?;
    }
    Ok(u)
}
#[derive(Default)]
pub(super) struct Meter {
    pub visits: usize,
    pub ceiling: usize,
}
impl Meter {
    pub fn visit(&mut self) -> Result<(), OwnedFailure> {
        self.visits = add(self.visits, 1)?;
        if self.visits > self.ceiling {
            Err(OwnedFailure::resource("ownership work ledger"))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
thread_local! { static ALLOCATION_FAILURE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
fn allocation_test_point() -> Result<(), OwnedFailure> {
    ALLOCATION_FAILURE.with(|point| match point.get() {
        Some(0) => Err(OwnedFailure::resource("injected allocation failure")),
        Some(n) => {
            point.set(Some(n - 1));
            Ok(())
        }
        None => Ok(()),
    })
}
#[cfg(test)]
pub(super) fn fail_allocation_after<T>(count: usize, operation: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ALLOCATION_FAILURE.with(|p| p.set(None));
        }
    }
    ALLOCATION_FAILURE.with(|p| p.set(Some(count)));
    let _reset = Reset;
    operation()
}
