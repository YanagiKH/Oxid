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
    let mut usage = budget::preflight(&raw, limits)?;
    let mut meter = budget::Meter {
        visits: 0,
        ceiling: usage.work,
    };
    let declarations = Declarations::check(&raw.records, sources)?;
    // Temporary Unit2A boundary: semantic carriers may describe arrays, but
    // neither unused carriers nor loop-only functions may obtain a witness.
    reject_array_carriers(&raw, &mut meter)?;
    shape::signatures(&raw, &declarations, sources)?;
    // Check every instruction in every function before accepting reachability
    // or any ownership result, including malformed unreachable operations.
    for f in &raw.functions {
        shape::check(f, &raw, &declarations, sources, &mut meter)?;
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
            let checked = shape::check(f, &raw, &declarations, sources, &mut meter)?;
            flow::check(f, &checked, &mut meter)?;
        }
    }
    Ok(VerifiedOwnedProgram {
        program: raw,
        declarations,
        usage,
        seal: OwnershipSeal,
    })
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
    }
    Ok(())
}
