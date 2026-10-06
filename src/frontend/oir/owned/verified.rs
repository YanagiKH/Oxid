//! The sealed boundary. Sibling consumers cannot construct or mutate fields.
use super::*;
use crate::frontend::builtin_catalog::{BuiltinEnum, BuiltinFunction};

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
    pub(super) fn builtin_function(&self) -> Option<hir::DefId> {
        let rank = self
            .program
            .builtins
            .function_rank(BuiltinFunction::ReadStdin)?;
        self.program
            .functions
            .len()
            .checked_sub(self.program.builtins.extra_functions())?
            .checked_add(rank)
            .map(hir::DefId)
    }
    pub(super) fn builtin_enumeration(&self) -> Option<EnumId> {
        let rank = self.program.builtins.enum_rank(BuiltinEnum::ReadStatus)?;
        self.program
            .enums
            .len()
            .checked_sub(self.program.builtins.extra_enums())?
            .checked_add(rank)
            .map(EnumId)
    }
    pub(super) fn builtin_output_function(&self) -> Option<hir::DefId> {
        let rank = self
            .program
            .builtins
            .function_rank(BuiltinFunction::WriteStdout)?;
        self.program
            .functions
            .len()
            .checked_sub(self.program.builtins.extra_functions())?
            .checked_add(rank)
            .map(hir::DefId)
    }
    pub(super) fn builtin_output_enumeration(&self) -> Option<EnumId> {
        let rank = self.program.builtins.enum_rank(BuiltinEnum::WriteStatus)?;
        self.program
            .enums
            .len()
            .checked_sub(self.program.builtins.extra_enums())?
            .checked_add(rank)
            .map(EnumId)
    }
    pub(super) fn has_builtin_origins(&self) -> bool {
        self.program.builtins != BuiltinOrigins::None
    }
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
    // Retain the pre-existing fixed carrier-inventory work charges for all
    // programs. Array admission still requires every authoritative check below.
    inventory_carriers(&raw, &mut meter)?;
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
    // Canonical builtin descriptors are an additional untrusted-raw check;
    // declarations and every ordinary shape/CFG/ownership pass still follow.
    builtins::check(raw)?;
    let declarations = Declarations::check_combined(&raw.records, &raw.enums, sources)?;
    let mut meter = budget::Meter {
        visits: 0,
        ceiling: usage.work,
    };
    for _ in 0..builtins::descriptor_visits(raw.builtins) {
        meter.visit()?;
    }
    Ok((usage, declarations, meter))
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
    validate_proof(raw, declarations, sources, usage, meter)
}

// Enum consumers share this authoritative proof. Source production still has
// no enum syntax/HIR/lowering route; no alternate witness path is introduced.
fn validate_proof(
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

/// Non-executable enum observation: only inert usage or denial escapes. This
/// uses the exact production proof, with no enum witness, plan, raw data,
/// declarations or consumer callback exposed.
#[cfg(test)]
pub(super) fn probe_enum_validation(
    raw: &RawOwnedProgram,
    sources: &SourceMap,
    limits: budget::Limits,
) -> Result<OwnershipUsage, OwnedFailure> {
    let (mut usage, declarations, mut meter) = prepare(raw, sources, limits)?;
    inventory_carriers(raw, &mut meter)?;
    validate_proof(raw, &declarations, sources, &mut usage, &mut meter)?;
    Ok(usage)
}

/// Closed output observation shares the authoritative proof, but returns only
/// inert usage. It cannot construct a witness or enable an effect consumer.
#[cfg(test)]
pub(super) fn probe_output_validation(
    raw: &RawOwnedProgram,
    sources: &SourceMap,
    limits: budget::Limits,
) -> Result<OwnershipUsage, OwnedFailure> {
    let mut usage = budget::preflight(raw, limits)?;
    builtins::check_candidate(raw)?;
    let declarations = Declarations::check_combined(&raw.records, &raw.enums, sources)?;
    let mut meter = budget::Meter {
        visits: 0,
        ceiling: usage.work,
    };
    for _ in 0..builtins::descriptor_visits(raw.builtins) {
        meter.visit()?;
    }
    inventory_carriers(raw, &mut meter)?;
    validate_proof(raw, &declarations, sources, &mut usage, &mut meter)?;
    Ok(usage)
}

/// Observe the authoritative checks without acquiring execution authority.
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

/// Qualification invokes the complete real native consumer synchronously while
/// the authoritative witness stays inside this boundary. Only bounded emitted
/// text/error and inert accounting escape; no callback receives privileged data.
#[cfg(test)]
pub(super) fn probe_array_native(
    raw: RawOwnedProgram,
    verification_sources: &SourceMap,
    verification_limits: budget::Limits,
    entry: Option<hir::DefId>,
    rendering_sources: &SourceMap,
    control: native::NativeControl,
) -> Result<native::NativeObservation, OwnedFailure> {
    let (mut usage, declarations, mut meter) =
        prepare(&raw, verification_sources, verification_limits)?;
    validate(
        &raw,
        &declarations,
        verification_sources,
        &mut usage,
        &mut meter,
    )?;
    let witness = VerifiedOwnedProgram {
        program: raw,
        declarations,
        usage,
        seal: OwnershipSeal,
    };
    Ok(native::run_array_observed(
        &witness,
        entry,
        rendering_sources,
        control,
    ))
}

/// Preserve the fixed carrier-inventory charge used before array activation.
/// Shape, identity, availability and loan checks remain mandatory in `validate`,
/// including every malformed instruction in unreachable blocks.
fn inventory_carriers(
    raw: &RawOwnedProgram,
    meter: &mut budget::Meter,
) -> Result<(), OwnedFailure> {
    for enumeration in &raw.enums {
        meter.visit()?;
        for _ in &enumeration.variants {
            meter.visit()?;
        }
    }
    for f in &raw.functions {
        for descriptor in &f.matches {
            meter.visit()?;
            for _ in &descriptor.arms {
                meter.visit()?;
            }
        }
        for _ in &f.owners {
            meter.visit()?;
        }
        for _ in &f.references {
            meter.visit()?;
        }
        for _ in &f.loans {
            meter.visit()?;
        }
    }
    Ok(())
}
