//! The sealed boundary. Sibling consumers cannot construct or mutate fields.
use super::*;

#[derive(Debug)]
pub(super) struct VerifiedOwnedProgram {
    program: RawOwnedProgram,
    declarations: Declarations,
    usage: OwnershipUsage,
    seal: OwnershipSeal,
}
#[derive(Debug)]
struct OwnershipSeal;
impl VerifiedOwnedProgram {
    pub(super) fn functions(&self) -> &[RawOwnedFunction] {
        &self.program.functions
    }
    pub(super) fn declarations(&self) -> &Declarations {
        &self.declarations
    }
    pub(super) fn usage(&self) -> OwnershipUsage {
        self.usage
    }
}
pub(super) fn verify_owned(
    raw: RawOwnedProgram,
    sources: &SourceMap,
) -> Result<VerifiedOwnedProgram, OwnedFailure> {
    verify_with_limits(raw, sources, budget::Limits::DEFAULT)
}
pub(super) fn verify_with_limits(
    raw: RawOwnedProgram,
    sources: &SourceMap,
    limits: budget::Limits,
) -> Result<VerifiedOwnedProgram, OwnedFailure> {
    let (mut usage, declarations, mut meter) = prepare(&raw, sources, limits)?;
    // Mandatory production boundary until both array consumers are complete.
    // This call cannot be selected away by a caller or by a test-only flag.
    reject_array_carriers(&raw, &mut meter)?;
    validate(&raw, &declarations, sources, &mut usage, &mut meter)?;
    Ok(VerifiedOwnedProgram {
        program: raw,
        declarations,
        usage,
        seal: OwnershipSeal,
    })
}

fn prepare(
    raw: &RawOwnedProgram,
    sources: &SourceMap,
    limits: budget::Limits,
) -> Result<(OwnershipUsage, Declarations, budget::Meter), OwnedFailure> {
    let usage = budget::preflight(raw, limits)?;
    let declarations = Declarations::check(&raw.records, sources)?;
    Ok((
        usage,
        declarations,
        budget::Meter {
            visits: 0,
            ceiling: usage.work,
        },
    ))
}

/// One authoritative continuation for production and the non-executable test
/// probe. It never constructs a seal and cannot return an executable value.
fn validate(
    raw: &RawOwnedProgram,
    declarations: &Declarations,
    sources: &SourceMap,
    usage: &mut OwnershipUsage,
    meter: &mut budget::Meter,
) -> Result<(), OwnedFailure> {
    shape::signatures(raw, declarations, sources)?;
    // Check every instruction in every function before accepting reachability
    // or any ownership result, including malformed unreachable operations.
    for f in &raw.functions {
        shape::check(f, raw, declarations, sources, meter)?;
    }
    for f in &raw.functions {
        super::super::verify::cfg(f)?;
    }
    for f in &raw.functions {
        for owner in &f.owners {
            usage.owner_cells = budget::add(
                usage.owner_cells,
                declarations.aggregate_width(owner.aggregate())?,
            )?;
            usage.owner_layout_bytes = budget::add(
                usage.owner_layout_bytes,
                declarations.aggregate_layout(owner.aggregate())?.size(),
            )?;
        }
        if budget::active(f) {
            let checked = shape::check(f, raw, declarations, sources, meter)?;
            flow::check(f, &checked, meter)?;
        }
    }
    Ok(())
}

/// Exercise the exact authoritative checks while array execution is gated.
/// Only unprivileged usage or a failure escapes; neither raw data, declarations,
/// a plan nor an executable witness is returned, even under cfg(test).
#[cfg(test)]
pub(super) fn probe_array_validation(
    raw: &RawOwnedProgram,
    sources: &SourceMap,
    limits: budget::Limits,
) -> Result<OwnershipUsage, OwnedFailure> {
    let (mut usage, declarations, mut meter) = prepare(raw, sources, limits)?;
    validate(raw, &declarations, sources, &mut usage, &mut meter)?;
    Ok(usage)
}

/// Qualification runs the real reference consumer while the witness remains
/// sealed inside this trusted boundary. No privileged object can escape.
#[cfg(test)]
pub(super) fn probe_array_reference(
    raw: RawOwnedProgram,
    sources: &SourceMap,
    verification_limits: budget::Limits,
    entry: Option<hir::DefId>,
    execution_limits: execute::Limits,
    observation: execute::ObservationControl,
) -> Result<execute::ReferenceObservation, OwnedFailure> {
    let (mut usage, declarations, mut meter) = prepare(&raw, sources, verification_limits)?;
    validate(&raw, &declarations, sources, &mut usage, &mut meter)?;
    let witness = VerifiedOwnedProgram {
        program: raw,
        declarations,
        usage,
        seal: OwnershipSeal,
    };
    Ok(execute::run_array_observed(
        &witness,
        entry,
        execution_limits,
        observation,
    ))
}

/// One fixed inventory pass, no allocation. Active rows fit the existing
/// 32*n fixed-pass allowance: owners + references + loans <= n; the meter
/// records each retained-row visit. Per-function result inspection is ordinary
/// signature inventory under the unchanged program function cap. Inactive
/// functions have no retained rows and only inspect their result, as signatures
/// already does, under the existing program function cap. Admission costs and
/// preflight/declaration error precedence are unchanged.
fn reject_array_carriers(
    raw: &RawOwnedProgram,
    meter: &mut budget::Meter,
) -> Result<(), OwnedFailure> {
    let check = |aggregate, span| match aggregate {
        AggregateTy::Record(_) => Ok(()),
        AggregateTy::FixedArray(_) => {
            Err(OwnedFailure::malformed(Malformed::UnsupportedArray, span))
        }
    };
    for f in &raw.functions {
        if let ValueTy::Owned(aggregate) = f.result {
            check(aggregate, f.span)?;
        }
        for owner in &f.owners {
            meter.visit()?;
            check(owner.aggregate(), owner.span)?;
        }
        for reference in &f.references {
            meter.visit()?;
            check(reference.aggregate(), reference.span)?;
        }
        for loan in &f.loans {
            meter.visit()?;
            check(loan.aggregate(), loan.span)?;
        }
        // A malformed array operation may have no array carrier at all. It
        // must not reach a seal merely because the carrier inventory was empty.
        // This fixed scan is bounded by the already admitted statement count;
        // it allocates nothing and does not alter array-free admission costs.
        for block in &f.blocks {
            for instruction in &block.statements {
                if matches!(
                    instruction.kind,
                    OwnedInstruction::ConstructArray { .. }
                        | OwnedInstruction::ReadIndex { .. }
                        | OwnedInstruction::WriteIndex { .. }
                        | OwnedInstruction::ArrayLength { .. }
                ) {
                    return Err(OwnedFailure::malformed(
                        Malformed::UnsupportedArray,
                        instruction.span,
                    ));
                }
            }
        }
    }
    Ok(())
}
