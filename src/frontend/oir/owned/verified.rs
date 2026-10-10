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
    /// Closed to the one empty-record fixture; not a generic ownership ledger.
    /// A rejected shape cannot silently omit an unhandled nested payload.
    #[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
    pub(super) fn observer_empty_record_capacity_bytes(&self) -> Option<usize> {
        fn payload<T>(values: &Vec<T>) -> Option<usize> {
            values.capacity().checked_mul(std::mem::size_of::<T>())
        }
        let raw = &self.program;
        if raw.builtins != BuiltinOrigins::None || !raw.enums.is_empty()
            || raw.records.len() != 1 || !raw.records[0].fields.is_empty()
            || raw.functions.len() != 1 { return None; }
        let f = &raw.functions[0];
        if !f.parameters.is_empty() || f.locals.len() != 1 || f.owners.len() != 1
            || !f.places.is_empty() || !f.references.is_empty() || !f.calls.is_empty()
            || !f.loans.is_empty() || !f.matches.is_empty() || f.blocks.len() != 1 {
            return None;
        }
        let block = &f.blocks[0];
        if block.merge.is_some() || block.statements.len() != 5
            || !matches!(block.terminator.as_ref()?.kind, OwnedTerminatorKind::ReturnScalar(_)) {
            return None;
        }
        let mut bytes = 0usize;
        for part in [payload(&raw.enums)?, payload(&raw.records)?, payload(&raw.functions)?,
            payload(&raw.records[0].fields)?, payload(&f.parameters)?, payload(&f.locals)?,
            payload(&f.owners)?, payload(&f.places)?, payload(&f.references)?, payload(&f.calls)?,
            payload(&f.loans)?, payload(&f.matches)?, payload(&f.blocks)?, payload(&block.statements)?] {
            bytes = bytes.checked_add(part)?;
        }
        for statement in &block.statements {
            match &statement.kind {
                OwnedInstruction::Construct { fields, .. } if fields.is_empty() => {
                    bytes = bytes.checked_add(payload(fields)?)?;
                }
                OwnedInstruction::StorageLive(_) | OwnedInstruction::Discard(_)
                | OwnedInstruction::StorageEnd(_) => {}
                OwnedInstruction::Scalar(Statement::Assign(Assign { value: Rvalue::Unit, .. })) => {}
                _ => return None,
            }
        }
        bytes.checked_add(self.declarations.observer_capacity_bytes()?)
    }

    /// Closed to the owned-relay fixture's F=2, E=1, B=3 retained shape.
    /// Every admitted Vec payload uses its actual capacity, including empty Vecs.
    /// Other shapes fail closed; this is not a generic ownership ledger.
    #[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
    pub(super) fn observer_owned_relay_capacity_bytes(&self) -> Option<usize> {
        fn payload<T>(values: &Vec<T>) -> Option<usize> {
            values.capacity().checked_mul(std::mem::size_of::<T>())
        }
        let raw = &self.program;
        if raw.builtins != BuiltinOrigins::None || !raw.enums.is_empty()
            || raw.records.len() != 1 || raw.functions.len() != 2 {
            return None;
        }
        let record = &raw.records[0];
        let field = FieldId { record: RecordId(0), index: 0 };
        if record.id != RecordId(0) || record.fields.len() != 1
            || record.fields[0].id != field
            || record.fields[0].ty != ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)) {
            return None;
        }
        let caller = &raw.functions[0];
        let callee = &raw.functions[1];
        if caller.id != hir::DefId(0) || callee.id != hir::DefId(1)
            || caller.entry != BlockId(0) || callee.entry != BlockId(0)
            || caller.result != ValueTy::Scalar(hir::Ty::I32)
            || callee.result != ValueTy::Owned(AggregateTy::Record(RecordId(0)))
            || !caller.parameters.is_empty() || callee.parameters.len() != 1
            || !matches!(callee.parameters[0], ParameterBinding::Owned(OwnerPlaceId(0)))
            || caller.locals.len() != 2 || !callee.locals.is_empty()
            || caller.owners.len() != 3 || callee.owners.len() != 1
            || caller.calls.len() != 1 || !callee.calls.is_empty()
            || caller.blocks.len() != 2 || callee.blocks.len() != 1 {
            return None;
        }
        for f in &raw.functions {
            // Empty outer vectors exclude all nested loan projections/match arms.
            if !f.places.is_empty() || !f.references.is_empty()
                || !f.loans.is_empty() || !f.matches.is_empty() {
                return None;
            }
            if f.owners.iter().any(|owner| owner.aggregate() != AggregateTy::Record(RecordId(0))) {
                return None;
            }
            for block in &f.blocks {
                if block.merge.is_some()
                    || block.terminator.as_ref()?.diagnostic_origins.is_some()
                    || block.statements.iter().any(|s| s.diagnostic_origins.is_some()) {
                    return None;
                }
            }
        }
        if caller.locals.iter().any(|local| local.ty != hir::Ty::I32 || local.kind != LocalKind::Temporary)
            || caller.owners[0].kind != (OwnerKind::Local { mutable: false })
            || caller.owners[1].kind != (OwnerKind::StagedArgument { call: CallSiteId(0), argument: 0 })
            || caller.owners[2].kind != (OwnerKind::CallResult { call: CallSiteId(0) })
            || callee.owners[0].kind != (OwnerKind::Parameter { position: 0 }) {
            return None;
        }
        let call = &caller.calls[0];
        if call.target != hir::DefId(1) || call.parent.is_some()
            || call.arguments.as_slice() != [ArgumentSlot::Owned(OwnerPlaceId(1))]
            || call.result != CallResult::Owned(OwnerPlaceId(2)) {
            return None;
        }
        let first = &caller.blocks[0];
        let second = &caller.blocks[1];
        let returned = &callee.blocks[0];
        if first.statements.len() != 5 || second.statements.len() != 4
            || !returned.statements.is_empty()
            || !matches!(&first.terminator.as_ref()?.kind,
                OwnedTerminatorKind::Invoke { call: CallSiteId(0), continuation: BlockId(1) })
            || !matches!(&second.terminator.as_ref()?.kind,
                OwnedTerminatorKind::ReturnScalar(Operand { local: LocalId(1), .. }))
            || !matches!(&returned.terminator.as_ref()?.kind,
                OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0))) {
            return None;
        }
        // Positional allowlist excludes every other instruction, including all
        // unhandled nested Vec variants (arrays, composites, and projections).
        if !matches!(&first.statements[0].kind,
                OwnedInstruction::Scalar(Statement::Assign(Assign {
                    destination: LocalId(0), value: Rvalue::I32(73), .. })))
            || !matches!(&first.statements[1].kind, OwnedInstruction::StorageLive(OwnerPlaceId(0)))
            || !matches!(&first.statements[3].kind, OwnedInstruction::OpenCall(CallSiteId(0)))
            || !matches!(&first.statements[4].kind, OwnedInstruction::PrepareOwned {
                call: CallSiteId(0), argument: 0, source: OwnerPlaceId(0) })
            || !matches!(&second.statements[0].kind, OwnedInstruction::ReadField {
                destination: LocalId(1), base: AccessBase::Owner(OwnerPlaceId(2)), field: f } if *f == field)
            || !matches!(&second.statements[1].kind, OwnedInstruction::Discard(OwnerPlaceId(2)))
            || !matches!(&second.statements[2].kind, OwnedInstruction::StorageEnd(OwnerPlaceId(0)))
            || !matches!(&second.statements[3].kind, OwnedInstruction::StorageEnd(OwnerPlaceId(2))) {
            return None;
        }
        let fields = match &first.statements[2].kind {
            OwnedInstruction::Construct { destination: OwnerPlaceId(0), fields }
                if fields.len() == 1 && fields[0].0 == field
                    && fields[0].1.local == LocalId(0) => fields,
            _ => return None,
        };
        let mut bytes = 0usize;
        for part in [payload(&raw.enums)?, payload(&raw.records)?, payload(&raw.functions)?,
            payload(&record.fields)?, payload(&call.arguments)?, payload(fields)?] {
            bytes = bytes.checked_add(part)?;
        }
        for f in &raw.functions {
            for part in [payload(&f.parameters)?, payload(&f.locals)?, payload(&f.places)?,
                payload(&f.owners)?, payload(&f.references)?, payload(&f.calls)?,
                payload(&f.loans)?, payload(&f.matches)?, payload(&f.blocks)?] {
                bytes = bytes.checked_add(part)?;
            }
            for block in &f.blocks {
                bytes = bytes.checked_add(payload(&block.statements)?)?;
            }
        }
        bytes.checked_add(self.declarations.observer_capacity_bytes()?)
    }

    /// Closed to the fixed five-caller star: F=6, E=5, B=11.
    /// Counts actual retained raw/declaration Vec payload capacities, even empty
    /// vectors. No source-map, stack, allocator overhead or logical-size estimate.
    /// Unknown nested payload shapes fail closed before any total can escape.
    #[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
    pub(super) fn observer_fixed_owned_relay_star_capacity_bytes(&self) -> Option<usize> {
        fn payload<T>(values: &Vec<T>) -> Option<usize> {
            values.capacity().checked_mul(std::mem::size_of::<T>())
        }
        let raw = &self.program;
        if raw.builtins != BuiltinOrigins::None || !raw.enums.is_empty()
            || raw.records.len() != 1 || raw.functions.len() != 6 {
            return None;
        }
        let record = &raw.records[0];
        let field = FieldId { record: RecordId(0), index: 0 };
        if record.id != RecordId(0) || record.fields.len() != 1
            || record.fields[0].id != field
            || record.fields[0].ty != ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)) {
            return None;
        }
        let callee = &raw.functions[5];
        if callee.id != hir::DefId(5) || callee.entry != BlockId(0)
            || callee.result != ValueTy::Owned(AggregateTy::Record(RecordId(0)))
            || callee.parameters.len() != 1
            || !matches!(callee.parameters[0], ParameterBinding::Owned(OwnerPlaceId(0)))
            || !callee.locals.is_empty() || callee.owners.len() != 1
            || !callee.calls.is_empty() || callee.blocks.len() != 1
            || callee.owners[0].kind != (OwnerKind::Parameter { position: 0 }) {
            return None;
        }
        let returned = &callee.blocks[0];
        if !returned.statements.is_empty()
            || !matches!(&returned.terminator.as_ref()?.kind,
                OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0))) {
            return None;
        }
        for f in &raw.functions {
            // Empty outer vectors exclude all nested loan projections/match arms.
            if !f.places.is_empty() || !f.references.is_empty()
                || !f.loans.is_empty() || !f.matches.is_empty() {
                return None;
            }
            if f.owners.iter().any(|owner| owner.aggregate() != AggregateTy::Record(RecordId(0))) {
                return None;
            }
            for block in &f.blocks {
                if block.merge.is_some()
                    || block.terminator.as_ref()?.diagnostic_origins.is_some()
                    || block.statements.iter().any(|s| s.diagnostic_origins.is_some()) {
                    return None;
                }
            }
        }
        let mut bytes = 0usize;
        for part in [payload(&raw.enums)?, payload(&raw.records)?, payload(&raw.functions)?,
            payload(&record.fields)?] {
            bytes = bytes.checked_add(part)?;
        }
        for (id, caller) in raw.functions[..5].iter().enumerate() {
            if caller.id != hir::DefId(id) || caller.entry != BlockId(0)
                || caller.result != ValueTy::Scalar(hir::Ty::I32)
                || !caller.parameters.is_empty() || caller.locals.len() != 2
                || caller.owners.len() != 3 || caller.calls.len() != 1
                || caller.blocks.len() != 2 {
                return None;
            }
            if caller.locals.iter().any(|local| local.ty != hir::Ty::I32 || local.kind != LocalKind::Temporary)
                || caller.owners[0].kind != (OwnerKind::Local { mutable: false })
                || caller.owners[1].kind != (OwnerKind::StagedArgument { call: CallSiteId(0), argument: 0 })
                || caller.owners[2].kind != (OwnerKind::CallResult { call: CallSiteId(0) }) {
                return None;
            }
            let call = &caller.calls[0];
            if call.target != hir::DefId(5) || call.parent.is_some()
                || call.arguments.as_slice() != [ArgumentSlot::Owned(OwnerPlaceId(1))]
                || call.result != CallResult::Owned(OwnerPlaceId(2)) {
                return None;
            }
            let first = &caller.blocks[0];
            let second = &caller.blocks[1];
            if first.statements.len() != 5 || second.statements.len() != 4
                || !matches!(&first.terminator.as_ref()?.kind,
                    OwnedTerminatorKind::Invoke { call: CallSiteId(0), continuation: BlockId(1) })
                || !matches!(&second.terminator.as_ref()?.kind,
                    OwnedTerminatorKind::ReturnScalar(Operand { local: LocalId(1), .. })) {
                return None;
            }
            // Positional allowlist excludes every other instruction, including all
            // unhandled nested Vec variants (arrays, composites, and projections).
            if !matches!(&first.statements[0].kind,
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        destination: LocalId(0), value: Rvalue::I32(73), .. })))
                || !matches!(&first.statements[1].kind, OwnedInstruction::StorageLive(OwnerPlaceId(0)))
                || !matches!(&first.statements[3].kind, OwnedInstruction::OpenCall(CallSiteId(0)))
                || !matches!(&first.statements[4].kind, OwnedInstruction::PrepareOwned {
                    call: CallSiteId(0), argument: 0, source: OwnerPlaceId(0) })
                || !matches!(&second.statements[0].kind, OwnedInstruction::ReadField {
                    destination: LocalId(1), base: AccessBase::Owner(OwnerPlaceId(2)), field: f } if *f == field)
                || !matches!(&second.statements[1].kind, OwnedInstruction::Discard(OwnerPlaceId(2)))
                || !matches!(&second.statements[2].kind, OwnedInstruction::StorageEnd(OwnerPlaceId(0)))
                || !matches!(&second.statements[3].kind, OwnedInstruction::StorageEnd(OwnerPlaceId(2))) {
                return None;
            }
            let fields = match &first.statements[2].kind {
                OwnedInstruction::Construct { destination: OwnerPlaceId(0), fields }
                    if fields.len() == 1 && fields[0].0 == field
                        && fields[0].1.local == LocalId(0) => fields,
                _ => return None,
            };
            bytes = bytes.checked_add(payload(&call.arguments)?)?;
            bytes = bytes.checked_add(payload(fields)?)?;
        }
        for f in &raw.functions {
            for part in [payload(&f.parameters)?, payload(&f.locals)?, payload(&f.places)?,
                payload(&f.owners)?, payload(&f.references)?, payload(&f.calls)?,
                payload(&f.loans)?, payload(&f.matches)?, payload(&f.blocks)?] {
                bytes = bytes.checked_add(part)?;
            }
            for block in &f.blocks {
                bytes = bytes.checked_add(payload(&block.statements)?)?;
            }
        }
        bytes.checked_add(self.declarations.observer_capacity_bytes()?)
    }

    /// Closed to the fixed empty-record fanout: F=1, call E=0, B=9, CFG E=8.
    /// Topology, literals, and every admitted span are checked before counting.
    /// Counts actual retained raw/declaration Vec capacities, including empty
    /// vectors. Unknown nested payloads fail closed; source maps are separate.
    #[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
    pub(super) fn observer_fixed_empty_record_cfg_fanout_capacity_bytes(&self) -> Option<usize> {
        fn payload<T>(values: &Vec<T>) -> Option<usize> {
            values.capacity().checked_mul(std::mem::size_of::<T>())
        }
        let raw = &self.program;
        if raw.builtins != BuiltinOrigins::None || !raw.enums.is_empty()
            || raw.records.len() != 1 || raw.functions.len() != 1 {
            return None;
        }
        let record = &raw.records[0];
        let f = &raw.functions[0];
        let file = f.span.file;
        let s = |i: usize| Span { file, start: i * 2, end: i * 2 + 1 };
        if record.id != RecordId(0) || record.span != s(0) || !record.fields.is_empty()
            || f.id != hir::DefId(0) || f.span != s(0) || f.entry != BlockId(0)
            || f.result != ValueTy::Scalar(hir::Ty::Unit)
            || !f.parameters.is_empty() || f.locals.len() != 2 || f.owners.len() != 1
            || !f.places.is_empty() || !f.references.is_empty() || !f.calls.is_empty()
            || !f.loans.is_empty() || !f.matches.is_empty() || f.blocks.len() != 9 {
            return None;
        }
        // Empty outer vectors exclude call arguments, loan projections, match
        // arms, enum variants, and every other unaccounted nested Vec family.
        if f.locals[0].ty != hir::Ty::Unit || f.locals[0].span != s(0)
            || f.locals[1].ty != hir::Ty::Bool || f.locals[1].span != s(6)
            || f.locals.iter().any(|local| local.kind != LocalKind::Temporary)
            || f.owners[0].aggregate() != AggregateTy::Record(RecordId(0))
            || f.owners[0].kind != (OwnerKind::Local { mutable: false })
            || f.owners[0].span != s(0) {
            return None;
        }
        for (id, block) in f.blocks.iter().enumerate() {
            let term = block.terminator.as_ref()?;
            let span = s(7 + id);
            if block.merge.is_some() || term.diagnostic_origins.is_some()
                || block.span != (if id == 0 { s(0) } else { span })
                || term.span != span
                || (id == 0 && block.statements.len() != 6)
                || (id != 0 && !block.statements.is_empty()) {
                return None;
            }
            match (id, &term.kind) {
                (0 | 2 | 4 | 6, OwnedTerminatorKind::Branch {
                    condition, then_block, else_block,
                }) if condition.local == LocalId(1) && condition.span == span
                    && *then_block == BlockId(id + 1) && *else_block == BlockId(id + 2) => {}
                (1 | 3 | 5 | 7 | 8, OwnedTerminatorKind::ReturnScalar(value))
                    if value.local == LocalId(0) && value.span == span => {}
                _ => return None,
            }
        }
        let root = &f.blocks[0];
        for (index, statement) in root.statements.iter().enumerate() {
            if statement.diagnostic_origins.is_some() || statement.span != s(index + 1) {
                return None;
            }
        }
        // Positional allowlist rejects arrays, composites, projections, and all
        // scalar variants other than the exact Unit and true assignments.
        if !matches!(&root.statements[0].kind, OwnedInstruction::StorageLive(OwnerPlaceId(0)))
            || !matches!(&root.statements[2].kind, OwnedInstruction::Discard(OwnerPlaceId(0)))
            || !matches!(&root.statements[3].kind, OwnedInstruction::StorageEnd(OwnerPlaceId(0)))
            || !matches!(&root.statements[4].kind,
                OwnedInstruction::Scalar(Statement::Assign(Assign {
                    destination: LocalId(0), value: Rvalue::Unit, span,
                })) if *span == s(5))
            || !matches!(&root.statements[5].kind,
                OwnedInstruction::Scalar(Statement::Assign(Assign {
                    destination: LocalId(1), value: Rvalue::Bool(true), span,
                })) if *span == s(6)) {
            return None;
        }
        let fields = match &root.statements[1].kind {
            OwnedInstruction::Construct { destination: OwnerPlaceId(0), fields }
                if fields.is_empty() => fields,
            _ => return None,
        };
        let mut bytes = 0usize;
        for part in [payload(&raw.enums)?, payload(&raw.records)?, payload(&raw.functions)?,
            payload(&record.fields)?, payload(&f.parameters)?, payload(&f.locals)?,
            payload(&f.places)?, payload(&f.owners)?, payload(&f.references)?,
            payload(&f.calls)?, payload(&f.loans)?, payload(&f.matches)?,
            payload(&f.blocks)?, payload(fields)?] {
            bytes = bytes.checked_add(part)?;
        }
        for block in &f.blocks {
            // Empty statement vectors may still retain nonzero capacity.
            bytes = bytes.checked_add(payload(&block.statements)?)?;
        }
        bytes.checked_add(self.declarations.observer_capacity_bytes()?)
    }

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

/// Only the consuming source-association wrapper can select conversion authority.
pub(super) fn verify_associated(
    associated: super::source::association::AssociatedOwned<'_>,
) -> Result<VerifiedOwnedProgram, OwnedFailure> {
    let (raw, sources) = associated.into_parts();
    let (mut usage, declarations, mut meter) = prepare(&raw, sources, budget::Limits::DEFAULT)?;
    inventory_carriers(&raw, &mut meter)?;
    validate_proof_impl(&raw, &declarations, sources, &mut usage, &mut meter, true)?;
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
    validate_proof_impl(raw, declarations, sources, usage, meter, false)
}
fn validate_proof_impl(
    raw: &RawOwnedProgram,
    declarations: &Declarations,
    sources: &SourceMap,
    usage: &mut OwnershipUsage,
    meter: &mut budget::Meter,
    source_associated: bool,
) -> Result<(), OwnedFailure> {
    shape::signatures(raw, declarations, sources)?;
    // Check every instruction in every function before accepting reachability
    // or any ownership result, including malformed unreachable operations.
    for f in &raw.functions {
        shape::check_with_conversion_authority(
            f,
            raw,
            declarations,
            sources,
            meter,
            source_associated,
        )?;
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
            let checked = shape::check_with_conversion_authority(
                f,
                raw,
                declarations,
                sources,
                meter,
                source_associated,
            )?;
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
    let (mut usage, declarations, mut meter) = prepare(raw, sources, limits)?;
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

/// Named transport inventory for the new proof-dispatch layer on both bare and
/// associated routes. The inherited raw `Limits::scratch` and `metadata` model
/// requested vector/table payload, not inline Rust call stacks. Accordingly this
/// measurement is separate; the source HirPlan is not claimed to cover bare raw
/// verification. There are no new raw-route heap allocations or retained rows.
#[allow(dead_code)]
struct ConversionProofDispatchCarriers {
    raw: &'static RawOwnedProgram,
    declarations: &'static Declarations,
    sources: &'static SourceMap,
    usage: &'static mut OwnershipUsage,
    meter: &'static mut budget::Meter,
    selected: bool,
    validation_return: Result<(), OwnedFailure>,
    function: &'static RawOwnedFunction,
    shape_raw: &'static RawOwnedProgram,
    shape_declarations: &'static Declarations,
    shape_sources: &'static SourceMap,
    shape_meter: &'static mut budget::Meter,
    shape_selected: bool,
    shape_return: Result<shape::Shape, OwnedFailure>,
    rejected: OirFailure,
    normalized: OwnedFailure,
}
#[test]
fn owned_u8_raw_proof_dispatch_carriers_are_separate_from_payload_limits() {
    println!("RFC0030 owned raw proof dispatch inline_bytes={} align={} raw_scratch_scope=requested_vector_payload raw_metadata_scope=requested_table_payload",
        std::mem::size_of::<ConversionProofDispatchCarriers>(), std::mem::align_of::<ConversionProofDispatchCarriers>());
    assert!(std::mem::size_of::<ConversionProofDispatchCarriers>() > 0);
}

// Optional source-only test proposal to append to verified.rs. Not compiled/run.
// All witnesses below go through verify_owned; no fabricated or mutated seal.
#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn owned_relay_capacity_observer_accepts_exact_fixture() {
    let (sources, raw, _) = super::consumer_fixtures::owned_relay();
    let witness = verify_owned(raw, &sources).unwrap();
    assert!(witness.observer_owned_relay_capacity_bytes().is_some());
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn owned_relay_capacity_observer_rejects_alternate_literal() {
    let (sources, mut raw, _) = super::consumer_fixtures::owned_relay();
    // Change a cloned raw function before verification, never a sealed witness.
    let mut caller = raw.functions[0].clone();
    match &mut caller.blocks[0].statements[0].kind {
        OwnedInstruction::Scalar(Statement::Assign(assign)) => assign.value = Rvalue::I32(74),
        _ => panic!("owned_relay scalar assignment changed"),
    }
    raw.functions[0] = caller;
    let witness = verify_owned(raw, &sources).unwrap();
    assert_eq!(witness.observer_owned_relay_capacity_bytes(), None);
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn owned_relay_capacity_observer_rejects_extra_unused_local() {
    let (sources, mut raw, _) = super::consumer_fixtures::owned_relay();
    let mut caller = raw.functions[0].clone();
    caller.locals.push(caller.locals[0].clone());
    raw.functions[0] = caller;
    let witness = verify_owned(raw, &sources).unwrap();
    assert_eq!(witness.observer_owned_relay_capacity_bytes(), None);
}

// Source-only controls: not compiled or executed by this proposal.
// All witnesses use normal verify_owned; no sealed witness is mutated.
#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn fixed_owned_relay_star_capacity_observer_accepts_exact_fixture() {
    let (sources, raw, schedule) = super::consumer_fixtures::fixed_owned_relay_star();
    assert_eq!(raw.functions.len(), 6);
    assert_eq!(raw.functions.iter().map(|f| f.calls.len()).sum::<usize>(), 5);
    assert_eq!(raw.functions.iter().map(|f| f.blocks.len()).sum::<usize>(), 11);
    assert_eq!(schedule.entry, hir::DefId(0));
    assert_eq!(schedule.result, Scalar::I32(73));
    assert_eq!(schedule.fuel(), 52);
    assert_eq!(schedule.events.len(), 13);
    let (_, original, original_schedule) = super::consumer_fixtures::owned_relay();
    assert_eq!(schedule.events, original_schedule.events);
    assert_eq!(raw.functions[0].span, original.functions[0].span);
    assert_eq!(raw.functions[5].span, original.functions[1].span);
    for id in 1..5 {
        assert_eq!(raw.functions[id].span.start, 2 * (40 + id));
        assert_eq!(raw.functions[id].span.end, 2 * (40 + id) + 1);
        assert_eq!(raw.functions[id].span.file, raw.functions[0].span.file);
    }
    let witness = verify_owned(raw, &sources).unwrap();
    assert!(witness.observer_fixed_owned_relay_star_capacity_bytes().is_some());
    assert_eq!(witness.observer_owned_relay_capacity_bytes(), None);
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn fixed_owned_relay_star_capacity_observer_rejects_alternate_literal() {
    // Cover every caller, including otherwise unexecuted copies.
    for id in 0..5 {
        let (sources, mut raw, _) = super::consumer_fixtures::fixed_owned_relay_star();
        match &mut raw.functions[id].blocks[0].statements[0].kind {
            OwnedInstruction::Scalar(Statement::Assign(assign)) => assign.value = Rvalue::I32(74),
            _ => panic!("owned_relay scalar assignment changed"),
        }
        let witness = verify_owned(raw, &sources).unwrap();
        assert_eq!(witness.observer_fixed_owned_relay_star_capacity_bytes(), None);
    }
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn fixed_owned_relay_star_capacity_observer_rejects_extra_unused_local() {
    for id in 0..5 {
        let (sources, mut raw, _) = super::consumer_fixtures::fixed_owned_relay_star();
        let local = raw.functions[id].locals[0].clone();
        raw.functions[id].locals.push(local);
        let witness = verify_owned(raw, &sources).unwrap();
        assert_eq!(witness.observer_fixed_owned_relay_star_capacity_bytes(), None);
    }
}

// Source-only controls: not compiled or executed by this proposal.
// Every witness uses normal verify_owned; no sealed witness is mutated.
#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn fixed_empty_record_cfg_fanout_capacity_observer_accepts_exact_fixture() {
    let (sources, raw, schedule) = super::consumer_fixtures::fixed_empty_record_cfg_fanout();
    assert_eq!(raw.functions.len(), 1);
    let f = &raw.functions[0];
    let file = f.span.file;
    let s = |i: usize| Span { file, start: i * 2, end: i * 2 + 1 };
    assert!(f.calls.is_empty());
    assert_eq!(f.blocks.len(), 9);
    assert_eq!(f.blocks[0].span, s(0));
    assert_eq!(f.blocks[0].statements.len(), 6);
    assert_eq!(schedule.entry, hir::DefId(0));
    assert_eq!(schedule.result, Scalar::Unit);
    assert_eq!(schedule.fuel(), 20);
    assert_eq!(schedule.events, vec![
        (s(0), 8), (s(1), 1), (s(2), 2), (s(3), 2), (s(4), 2),
        (s(5), 1), (s(6), 1), (s(7), 1), (s(8), 2),
    ]);
    let mut edges = 0usize;
    for (id, then_id, else_id) in [(0, 1, 2), (2, 3, 4), (4, 5, 6), (6, 7, 8)] {
        let term = f.blocks[id].terminator.as_ref().unwrap();
        assert_eq!(term.span, s(7 + id));
        assert!(term.diagnostic_origins.is_none());
        assert!(matches!(&term.kind, OwnedTerminatorKind::Branch {
            condition, then_block, else_block,
        } if condition.local == LocalId(1) && condition.span == s(7 + id)
            && *then_block == BlockId(then_id) && *else_block == BlockId(else_id)));
        edges += 2;
    }
    assert_eq!(edges, 8);
    for id in [1, 3, 5, 7, 8] {
        let term = f.blocks[id].terminator.as_ref().unwrap();
        assert_eq!(term.span, s(7 + id));
        assert!(term.diagnostic_origins.is_none());
        assert!(matches!(&term.kind, OwnedTerminatorKind::ReturnScalar(value)
            if value.local == LocalId(0) && value.span == s(7 + id)));
    }
    for (id, block) in f.blocks.iter().enumerate().skip(1) {
        assert_eq!(block.span, s(7 + id));
        assert!(block.statements.is_empty());
        assert!(block.merge.is_none());
    }
    let witness = verify_owned(raw, &sources).unwrap();
    assert!(witness.observer_fixed_empty_record_cfg_fanout_capacity_bytes().is_some());
    assert_eq!(witness.observer_empty_record_capacity_bytes(), None);
    assert_eq!(witness.observer_owned_relay_capacity_bytes(), None);
    assert_eq!(witness.observer_fixed_owned_relay_star_capacity_bytes(), None);
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn fixed_empty_record_cfg_fanout_capacity_observer_rejects_alternate_bool() {
    let (sources, mut raw, _) = super::consumer_fixtures::fixed_empty_record_cfg_fanout();
    match &mut raw.functions[0].blocks[0].statements[5].kind {
        OwnedInstruction::Scalar(Statement::Assign(assign)) => assign.value = Rvalue::Bool(false),
        _ => panic!("fixed fanout Bool assignment changed"),
    }
    let witness = verify_owned(raw, &sources).unwrap();
    assert_eq!(witness.observer_fixed_empty_record_cfg_fanout_capacity_bytes(), None);
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn fixed_empty_record_cfg_fanout_capacity_observer_rejects_extra_unused_local() {
    let (sources, mut raw, _) = super::consumer_fixtures::fixed_empty_record_cfg_fanout();
    let local = raw.functions[0].locals[0].clone();
    raw.functions[0].locals.push(local);
    let witness = verify_owned(raw, &sources).unwrap();
    assert_eq!(witness.observer_fixed_empty_record_cfg_fanout_capacity_bytes(), None);
}
