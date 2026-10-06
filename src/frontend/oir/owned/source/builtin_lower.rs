//! Fixed source producer for the admitted compiler suffix. The independent raw
//! descriptor checker and full ownership verifier still certify its output.
use super::super::{budget as raw_budget, *};
use super::budget;
use crate::frontend::{
    builtin_catalog::{BuiltinEnum, BuiltinFunction},
    declaration_index::DeclarationIndex,
};
use std::mem::size_of;

pub(super) fn counts() -> raw_budget::FunctionCounts {
    raw_budget::FunctionCounts {
        parameters: 1,
        references: 1,
        owners: 1,
        blocks: 1,
        statements: 2,
        ownership_active: true,
        ..raw_budget::FunctionCounts::default()
    }
}

fn bad(index: &DeclarationIndex<'_>) -> OwnedFailure {
    OwnedFailure::malformed(Malformed::Binding, index.sources().eof())
}

/// Called only after the complete source output and invocation preflight. Each
/// nonempty nested vector uses the same observed fallible reservation boundary
/// as ordinary source lowering; empty lanes allocate nothing.
pub(super) fn function(index: &DeclarationIndex<'_>) -> Result<RawOwnedFunction, OwnedFailure> {
    let id = index
        .builtin_function_id(BuiltinFunction::ReadStdin)
        .map_err(|_| bad(index))?;
    let enumeration = index
        .builtin_enum_id(BuiltinEnum::ReadStatus)
        .map_err(|_| bad(index))?;
    let span = index
        .builtin_function_anchor(BuiltinFunction::ReadStdin)
        .map_err(|_| bad(index))?;
    let mut raw = RawOwnedFunction {
        id,
        span,
        result: ValueTy::Owned(AggregateTy::Enum(enumeration)),
        parameters: budget::reserve(1)?,
        locals: Vec::new(),
        places: Vec::new(),
        owners: budget::reserve(1)?,
        references: budget::reserve(1)?,
        calls: Vec::new(),
        loans: Vec::new(),
        matches: Vec::new(),
        entry: BlockId(0),
        blocks: budget::reserve(1)?,
    };
    budget::append(
        &mut raw.parameters,
        ParameterBinding::Reference(ReferenceParamId(0)),
        1,
        span,
    )?;
    budget::append(
        &mut raw.owners,
        OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Enum(enumeration))?,
            kind: OwnerKind::Temporary,
            span,
        },
        1,
        span,
    )?;
    budget::append(
        &mut raw.references,
        ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::I32))?,
            kind: BorrowKind::Exclusive,
            position: 0,
            span,
        },
        1,
        span,
    )?;
    let mut block = OwnedBlock {
        span,
        merge: None,
        statements: budget::reserve(2)?,
        terminator: Some(OwnedTerminator {
            kind: OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
            span,
            diagnostic_origins: None,
        }),
    };
    budget::append(
        &mut block.statements,
        OwnedStatement {
            kind: OwnedInstruction::StorageLive(OwnerPlaceId(0)),
            span,
            diagnostic_origins: None,
        },
        2,
        span,
    )?;
    budget::append(
        &mut block.statements,
        OwnedStatement {
            kind: OwnedInstruction::ReadStdin {
                buffer: ReferenceParamId(0),
                destination: OwnerPlaceId(0),
            },
            span,
            diagnostic_origins: None,
        },
        2,
        span,
    )?;
    budget::append(&mut raw.blocks, block, 1, span)?;
    Ok(raw)
}

/// Named new producer/control roles, including actual by-value reservation and
/// return envelopes. Nested vector backing is charged by function_bytes; this
/// does not claim to model inherited helper stack frames or allocator overhead.
#[allow(dead_code)]
struct Carriers {
    index: &'static DeclarationIndex<'static>,
    function_id: hir::DefId,
    enum_id: EnumId,
    anchor: Span,
    function_lookup: Result<hir::DefId, Box<Diagnostic>>,
    enum_lookup: Result<EnumId, Box<Diagnostic>>,
    anchor_lookup: Result<Span, Box<Diagnostic>>,
    mapped_failure: Result<(), OwnedFailure>,
    raw: RawOwnedFunction,
    constructed: RawOwnedFunction,
    returned: Result<RawOwnedFunction, OwnedFailure>,
    caller_return: Result<RawOwnedFunction, OwnedFailure>,
    caller_value: RawOwnedFunction,
    append_value: RawOwnedFunction,
    blocks: [OwnedBlock; 2],
    statements: [OwnedStatement; 2],
    owner_rows: [OwnerDecl; 2],
    reference_rows: [ReferenceDecl; 2],
    parameters: [ParameterBinding; 2],
    parameter_reserve: (Vec<ParameterBinding>, Result<Vec<ParameterBinding>, OwnedFailure>),
    owner_reserve: (Vec<OwnerDecl>, Result<Vec<OwnerDecl>, OwnedFailure>),
    reference_reserve: (Vec<ReferenceDecl>, Result<Vec<ReferenceDecl>, OwnedFailure>),
    block_reserve: (Vec<OwnedBlock>, Result<Vec<OwnedBlock>, OwnedFailure>),
    statement_reserve: (Vec<OwnedStatement>, Result<Vec<OwnedStatement>, OwnedFailure>),
    aggregate: Result<AggregateSlot, DeclarationError>,
    referent: Result<BorrowedSlot, DeclarationError>,
    inventory: raw_budget::FunctionCounts,
}
pub(super) const fn carrier_bytes() -> usize {
    size_of::<Carriers>()
}
