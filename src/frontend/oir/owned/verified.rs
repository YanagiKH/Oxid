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
    let declarations = Declarations::check(&raw.records, sources)?;
    shape::signatures(&raw, &declarations, sources)?;
    // Check every instruction in every function before accepting reachability
    // or any ownership result, including malformed unreachable operations.
    for f in &raw.functions {
        shape::check(f, &raw, &declarations, sources)?;
    }
    for f in &raw.functions {
        super::super::verify::cfg(f)?;
    }
    let mut meter = budget::Meter {
        visits: 0,
        ceiling: usage.work,
    };
    for f in &raw.functions {
        for owner in &f.owners {
            usage.owner_cells = budget::add(
                usage.owner_cells,
                declarations.fields(owner.record)?.len().max(1),
            )?;
            usage.owner_layout_bytes = budget::add(
                usage.owner_layout_bytes,
                declarations.record(owner.record)?.layout().size(),
            )?;
        }
        if budget::active(f) {
            let checked = shape::check(f, &raw, &declarations, sources)?;
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
