//! Fallible source producer. The independent raw descriptor checker and ordinary
//! shape/flow proof certify the canonical suffix before either consumer sees it.
use super::super::*;
use super::{budget, builtin_lower::check_capacity};
use crate::frontend::{
    builtin_catalog::vector::{Function, Signature},
    declaration_index::DeclarationIndex,
};

pub(super) fn counts(kind: Function) -> super::super::budget::FunctionCounts {
    let signature = kind.signature(EnumId(0));
    let mut result = super::super::budget::FunctionCounts {
        parameters: signature.parameters().len(),
        blocks: 1,
        statements: 1,
        ownership_active: true,
        ..Default::default()
    };
    for parameter in signature.parameters() {
        match parameter {
            ParameterTy::Value(ValueTy::Scalar(_)) => result.locals += 1,
            ParameterTy::Reference { .. } => result.references += 1,
            _ => unreachable!("closed parameter"),
        }
    }
    match signature.result {
        ValueTy::Owned(_) => {
            result.owners = 1;
            result.statements += 1;
        }
        ValueTy::Scalar(_) => result.locals += 1,
    }
    result
}
fn exact<T>(count: usize) -> Result<Vec<T>, OwnedFailure> {
    let values = budget::reserve(count)?;
    check_capacity(&values, count)?;
    Ok(values)
}
pub(super) fn function(
    index: &DeclarationIndex<'_>,
    operation: Function,
) -> Result<RawOwnedFunction, OwnedFailure> {
    let fail = |_| OwnedFailure::malformed(Malformed::Binding, index.sources().eof());
    let id = index.vector_function_id(operation).map_err(fail)?;
    let span = index.vector_anchor(operation.item()).map_err(fail)?;
    let signature = index.vector_signature(operation).map_err(fail)?;
    let c = counts(operation);
    let mut raw = RawOwnedFunction {
        id,
        span,
        result: signature.result,
        parameters: exact(c.parameters)?,
        locals: exact(c.locals)?,
        places: Vec::new(),
        owners: exact(c.owners)?,
        references: exact(c.references)?,
        calls: Vec::new(),
        loans: Vec::new(),
        matches: Vec::new(),
        entry: BlockId(0),
        blocks: exact(1)?,
    };
    for (position, parameter) in signature.parameters().iter().enumerate() {
        let binding = match *parameter {
            ParameterTy::Value(ValueTy::Scalar(ty)) => {
                let local = LocalId(raw.locals.len());
                budget::append(
                    &mut raw.locals,
                    LocalDecl {
                        ty,
                        kind: LocalKind::Parameter,
                        span,
                    },
                    c.locals,
                    span,
                )?;
                ParameterBinding::Scalar(local)
            }
            ParameterTy::Reference { referent, kind } => {
                let reference = ReferenceParamId(raw.references.len());
                budget::append(
                    &mut raw.references,
                    ReferenceDecl {
                        referent: BorrowedSlot::check(referent)?,
                        kind,
                        position,
                        span,
                    },
                    c.references,
                    span,
                )?;
                ParameterBinding::Reference(reference)
            }
            _ => unreachable!("closed parameter"),
        };
        budget::append(&mut raw.parameters, binding, c.parameters, span)?;
    }
    let mut block = OwnedBlock {
        span,
        merge: None,
        statements: exact(c.statements)?,
        terminator: None,
    };
    let (destination, end) = match signature.result {
        ValueTy::Owned(aggregate) => {
            budget::append(
                &mut raw.owners,
                OwnerDecl {
                    aggregate: AggregateSlot::try_from_aggregate(aggregate)?,
                    kind: OwnerKind::Temporary,
                    span,
                },
                c.owners,
                span,
            )?;
            budget::append(
                &mut block.statements,
                OwnedStatement {
                    kind: OwnedInstruction::StorageLive(OwnerPlaceId(0)),
                    span,
                    diagnostic_origins: None,
                },
                c.statements,
                span,
            )?;
            (
                CallResult::Owned(OwnerPlaceId(0)),
                OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
            )
        }
        ValueTy::Scalar(ty) => {
            let local = LocalId(raw.locals.len());
            budget::append(
                &mut raw.locals,
                LocalDecl {
                    ty,
                    kind: LocalKind::Temporary,
                    span,
                },
                c.locals,
                span,
            )?;
            (
                CallResult::Scalar(local),
                OwnedTerminatorKind::ReturnScalar(Operand { local, span }),
            )
        }
    };
    budget::append(
        &mut block.statements,
        OwnedStatement {
            kind: OwnedInstruction::Vector {
                operation,
                destination,
            },
            span,
            diagnostic_origins: None,
        },
        c.statements,
        span,
    )?;
    block.terminator = Some(OwnedTerminator {
        kind: end,
        span,
        diagnostic_origins: None,
    });
    budget::append(&mut raw.blocks, block, 1, span)?;
    Ok(raw)
}
/// Supplemental roles, charged only for this experimental suffix against the
/// existing compiler-storage ceiling. Nested vector payload is counted above.
type LocalReservation = (Vec<LocalDecl>, Result<Vec<LocalDecl>, OwnedFailure>);
#[allow(dead_code)]
struct Carriers {
    signatures: [Signature; 2],
    signature_returns: [Result<Signature, Box<Diagnostic>>; 2],
    parameter_iteration: std::iter::Enumerate<std::slice::Iter<'static, ParameterTy>>,
    local_reservations: [LocalReservation; 2],
    local_rows: [LocalDecl; 2],
    operations: [Function; 4],
    iteration: std::iter::Enumerate<std::slice::Iter<'static, Function>>,
}
pub(super) fn extra_bytes() -> usize {
    std::mem::size_of::<Carriers>()
}
