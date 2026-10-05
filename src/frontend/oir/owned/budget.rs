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
/// Plain unprivileged inventory. Producers and raw verification derive these
/// independently; using this arithmetic does not validate or seal a program.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct FunctionCounts {
    pub locals: usize,
    pub places: usize,
    pub owners: usize,
    pub references: usize,
    pub parameters: usize,
    pub calls: usize,
    pub loans: usize,
    pub blocks: usize,
    pub edges: usize,
    pub statements: usize,
    pub merges: usize,
    pub descriptor_arguments: usize,
    pub preparations: usize,
    pub constructed_fields: usize,
    pub constructed_elements: usize,
    pub composite_fields: usize,
    pub projection_fields: usize,
    pub diagnostic_origins: usize,
    pub max_constructor_fields: usize,
    pub ownership_active: bool,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProgramCounts {
    functions: usize,
    slots: usize,
    blocks: usize,
    statements: usize,
    usage: OwnershipUsage,
}
impl ProgramCounts {
    pub(super) fn usage(&self) -> OwnershipUsage {
        self.usage
    }
}
/// Shared checked counts only: no ownership state, witness, or allocations.
pub(super) fn account_function(
    c: FunctionCounts,
    limits: Limits,
    program: &mut ProgramCounts,
) -> Result<(), OwnedFailure> {
    let limits = limits.bounded();
    program.functions = cap(add(program.functions, 1)?, MAX_BLOCKS, "functions")?;
    program.slots = cap(
        add(program.slots, add(add(c.locals, c.places)?, c.owners)?)?,
        MAX_LOCALS,
        "locals",
    )?;
    program.blocks = cap(add(program.blocks, c.blocks)?, MAX_BLOCKS, "blocks")?;
    cap(
        c.parameters,
        super::super::super::parser::MAX_PARAMS,
        "parameters",
    )?;
    let s = add(c.statements, c.merges)?;
    program.statements = cap(add(program.statements, s)?, MAX_ASSIGNMENTS, "assignments")?;
    if !c.ownership_active && c.diagnostic_origins == 0 {
        return Ok(());
    }
    let u = &mut program.usage;
    let mut work = mul(if c.ownership_active { 4 } else { 2 }, c.diagnostic_origins)?;
    let mut metadata = mul(
        add(c.statements, c.blocks)?,
        size_of::<Option<DiagnosticOrigins>>(),
    )?;
    if c.ownership_active {
        let a = add(c.descriptor_arguments, c.preparations)?;
        let composition = add(c.composite_fields, c.projection_fields)?;
        u.owners = cap(add(u.owners, c.owners)?, limits.owners, "owners")?;
        u.expanded_events = cap(
            add(
                u.expanded_events,
                add(
                    add(add(s, a)?, c.constructed_fields)?,
                    add(c.constructed_elements, composition)?,
                )?,
            )?,
            limits.events,
            "expanded ownership events",
        )?;
        let mut n = 1;
        for count in [
            mul(2, c.blocks)?,
            c.edges,
            s,
            a,
            c.constructed_fields,
            c.constructed_elements,
            composition,
            c.owners,
            c.loans,
            c.calls,
            c.parameters,
            c.references,
            c.locals,
            c.places,
        ] {
            n = add(n, count)?;
        }
        let multiplier = add(add(add(mul(4, c.owners)?, c.loans)?, c.calls)?, 32)?;
        work = add(work, mul(multiplier, n)?)?;
        // Retained acquisition sites are included here once. Their lifetime
        // extends through flow; no duplicate loan-origin vector is allocated.
        for (count, size) in [
            (c.owners, size_of::<shape::OwnerSites>()),
            (c.calls, size_of::<shape::CallSites>()),
            (c.descriptor_arguments, size_of::<Option<shape::Site>>()),
            (c.loans, size_of::<Option<shape::Site>>()),
            (c.calls, 2 * size_of::<usize>() + size_of::<(usize, bool)>()),
            (c.parameters, size_of::<ParameterBinding>()),
            (c.references, size_of::<ReferenceDecl>()),
            (c.loans, size_of::<LoanDecl>()),
            (c.constructed_fields, size_of::<(FieldId, Operand)>()),
            (c.constructed_elements, size_of::<Operand>()),
            (c.composite_fields, size_of::<(FieldId, FieldInitializer)>()),
            (c.projection_fields, size_of::<FieldId>()),
        ] {
            metadata = add(metadata, mul(count, size)?)?;
        }
    }
    u.work = cap(add(u.work, work)?, limits.work, "ownership work")?;
    u.metadata_bytes = cap(
        add(u.metadata_bytes, metadata)?,
        limits.metadata,
        "ownership metadata",
    )?;
    if c.ownership_active {
        // Availability queue is dropped before reverse-path reconstruction.
        let availability = mul(c.blocks, 1 + 4 * size_of::<usize>())?;
        let reconstruction = add(
            mul(2, c.blocks)?,
            mul(
                add(add(mul(2, c.blocks)?, 1)?, c.edges)?,
                size_of::<usize>(),
            )?,
        )?;
        let scratch = availability
            .max(reconstruction)
            .max(c.max_constructor_fields.min(1024));
        u.scratch_bytes = cap(
            u.scratch_bytes.max(scratch),
            limits.scratch,
            "ownership scratch",
        )?;
    }
    Ok(())
}
/// Preflight actual raw nested lengths before validation or analysis allocation.
/// Work units are inventory/statement/block/edge visits, not CPU instructions.
pub(super) fn preflight(
    raw: &RawOwnedProgram,
    limits: Limits,
) -> Result<OwnershipUsage, OwnedFailure> {
    cap(raw.functions.len(), MAX_BLOCKS, "functions")?;
    let mut program = ProgramCounts::default();
    for f in &raw.functions {
        // Preserve the original early general-cap ordering before nested scans.
        cap(
            add(
                program.slots,
                add(add(f.locals.len(), f.places.len())?, f.owners.len())?,
            )?,
            MAX_LOCALS,
            "locals",
        )?;
        cap(add(program.blocks, f.blocks.len())?, MAX_BLOCKS, "blocks")?;
        cap(
            f.parameters.len(),
            super::super::super::parser::MAX_PARAMS,
            "parameters",
        )?;
        let mut c = FunctionCounts {
            locals: f.locals.len(),
            places: f.places.len(),
            owners: f.owners.len(),
            references: f.references.len(),
            parameters: f.parameters.len(),
            calls: f.calls.len(),
            loans: f.loans.len(),
            blocks: f.blocks.len(),
            ownership_active: !f.owners.is_empty()
                || !f.references.is_empty()
                || !f.calls.is_empty()
                || !f.loans.is_empty(),
            ..FunctionCounts::default()
        };
        for loan in &f.loans {
            cap(
                loan.projection.len(),
                MAX_CONTAINMENT_DEPTH,
                "loan projection depth",
            )?;
            c.projection_fields = add(c.projection_fields, loan.projection.len())?;
        }
        for call in &f.calls {
            cap(
                call.arguments.len(),
                super::super::super::parser::MAX_PARAMS,
                "call arguments",
            )?;
            c.descriptor_arguments = add(c.descriptor_arguments, call.arguments.len())?;
        }
        for b in &f.blocks {
            c.statements = add(c.statements, b.statements.len())?;
            c.merges = add(c.merges, usize::from(b.merge.is_some()))?;
            if let Some(end) = &b.terminator {
                c.diagnostic_origins = add(
                    c.diagnostic_origins,
                    usize::from(end.diagnostic_origins.is_some()),
                )?;
                c.edges = add(
                    c.edges,
                    match end.kind {
                        OwnedTerminatorKind::Branch { .. } => 2,
                        OwnedTerminatorKind::Goto(_) | OwnedTerminatorKind::Invoke { .. } => 1,
                        _ => 0,
                    },
                )?;
                c.ownership_active |= matches!(
                    end.kind,
                    OwnedTerminatorKind::Invoke { .. } | OwnedTerminatorKind::ReturnOwned(_)
                );
            }
            for i in &b.statements {
                c.diagnostic_origins = add(
                    c.diagnostic_origins,
                    usize::from(i.diagnostic_origins.is_some()),
                )?;
                c.ownership_active |= !matches!(i.kind, OwnedInstruction::Scalar(_));
                match &i.kind {
                    OwnedInstruction::Construct { fields, .. } => {
                        c.constructed_fields = add(c.constructed_fields, fields.len())?;
                        c.max_constructor_fields = c.max_constructor_fields.max(fields.len());
                    }
                    OwnedInstruction::ConstructComposite { fields, .. } => {
                        cap(fields.len(), 1024, "composite constructor fields")?;
                        c.composite_fields = add(c.composite_fields, fields.len())?;
                        c.max_constructor_fields = c.max_constructor_fields.max(fields.len());
                    }
                    OwnedInstruction::ReadProjection { path, .. }
                    | OwnedInstruction::WriteProjection { path, .. }
                    | OwnedInstruction::ProjectionLength { path, .. } => {
                        cap(path.len(), MAX_CONTAINMENT_DEPTH, "record projection depth")?;
                        c.projection_fields = add(c.projection_fields, path.len())?;
                    }
                    OwnedInstruction::ConstructArray { elements, .. } => {
                        // Inspect only the caller-owned vector length here. The
                        // envelope precedes every element/span/type/CFG walk.
                        cap(elements.len(), 1024, "array constructor elements")?;
                        c.constructed_elements = add(c.constructed_elements, elements.len())?;
                    }
                    OwnedInstruction::PrepareScalar { .. }
                    | OwnedInstruction::PrepareOwned { .. }
                    | OwnedInstruction::PrepareBorrow { .. } => {
                        c.preparations = add(c.preparations, 1)?
                    }
                    _ => {}
                }
            }
        }
        account_function(c, limits, &mut program)?;
    }
    Ok(program.usage())
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
