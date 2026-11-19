//! Fallible source producer. The independent raw descriptor checker and ordinary
//! shape/flow proof certify the canonical suffix before either consumer sees it.
use super::super::*;
use super::{budget, builtin_lower::check_capacity};
use crate::frontend::{
    builtin_catalog::vector::{Function, Signature},
    declaration_index::DeclarationIndex,
};

macro_rules! signature_counts {
    ($signature:ident) => {{
        let mut result = super::super::budget::FunctionCounts {
            parameters: $signature.parameters().len(),
            blocks: 1,
            statements: 1,
            ownership_active: true,
            ..Default::default()
        };
        for parameter in $signature.parameters() {
            match parameter {
                ParameterTy::Value(ValueTy::Scalar(_)) => result.locals += 1,
                ParameterTy::Value(ValueTy::Owned(_)) => result.owners += 1,
                ParameterTy::Reference { .. } => result.references += 1,
            }
        }
        match $signature.result {
            ValueTy::Owned(_) => {
                result.owners += 1;
                result.statements += 1;
            }
            ValueTy::Scalar(_) => result.locals += 1,
        }
        result
    }};
}

pub(super) fn counts(kind: Function) -> super::super::budget::FunctionCounts {
    let signature = kind.signature(EnumId(0));
    signature_counts!(signature)
}

fn exact<T>(count: usize) -> Result<Vec<T>, OwnedFailure> {
    let values = budget::reserve(count)?;
    check_capacity(&values, count)?;
    Ok(values)
}
macro_rules! source_function {
    ($id:ident, $span:ident, $signature:ident, $c:ident, $operation:ident, $variant:ident) => {{
        let mut raw = RawOwnedFunction {
            $id,
            $span,
            result: $signature.result,
            parameters: exact($c.parameters)?,
            locals: exact($c.locals)?,
            places: Vec::new(),
            owners: exact($c.owners)?,
            references: exact($c.references)?,
            calls: Vec::new(),
            loans: Vec::new(),
            matches: Vec::new(),
            entry: BlockId(0),
            blocks: exact(1)?,
        };
        for (position, parameter) in $signature.parameters().iter().enumerate() {
            let binding = match *parameter {
                ParameterTy::Value(ValueTy::Scalar(ty)) => {
                    let local = LocalId(raw.locals.len());
                    budget::append(
                        &mut raw.locals,
                        LocalDecl {
                            ty,
                            kind: LocalKind::Parameter,
                            $span,
                        },
                        $c.locals,
                        $span,
                    )?;
                    ParameterBinding::Scalar(local)
                }
                ParameterTy::Value(ValueTy::Owned(aggregate)) => {
                    let owner = OwnerPlaceId(raw.owners.len());
                    budget::append(
                        &mut raw.owners,
                        OwnerDecl {
                            aggregate: AggregateSlot::try_from_aggregate(aggregate)?,
                            kind: OwnerKind::Parameter { position },
                            $span,
                        },
                        $c.owners,
                        $span,
                    )?;
                    ParameterBinding::Owned(owner)
                }
                ParameterTy::Reference { referent, kind } => {
                    let reference = ReferenceParamId(raw.references.len());
                    budget::append(
                        &mut raw.references,
                        ReferenceDecl {
                            referent: BorrowedSlot::check(referent)?,
                            kind,
                            position,
                            $span,
                        },
                        $c.references,
                        $span,
                    )?;
                    ParameterBinding::Reference(reference)
                }
            };
            budget::append(&mut raw.parameters, binding, $c.parameters, $span)?;
        }
        let mut block = OwnedBlock {
            $span,
            merge: None,
            statements: exact($c.statements)?,
            terminator: None,
        };
        let (destination, end) = match $signature.result {
            ValueTy::Owned(aggregate) => {
                let owner = OwnerPlaceId(raw.owners.len());
                budget::append(
                    &mut raw.owners,
                    OwnerDecl {
                        aggregate: AggregateSlot::try_from_aggregate(aggregate)?,
                        kind: OwnerKind::Temporary,
                        $span,
                    },
                    $c.owners,
                    $span,
                )?;
                budget::append(
                    &mut block.statements,
                    OwnedStatement {
                        kind: OwnedInstruction::StorageLive(owner),
                        $span,
                        diagnostic_origins: None,
                    },
                    $c.statements,
                    $span,
                )?;
                (
                    CallResult::Owned(owner),
                    OwnedTerminatorKind::ReturnOwned(owner),
                )
            }
            ValueTy::Scalar(ty) => {
                let local = LocalId(raw.locals.len());
                budget::append(
                    &mut raw.locals,
                    LocalDecl {
                        ty,
                        kind: LocalKind::Temporary,
                        $span,
                    },
                    $c.locals,
                    $span,
                )?;
                (
                    CallResult::Scalar(local),
                    OwnedTerminatorKind::ReturnScalar(Operand { local, $span }),
                )
            }
        };
        budget::append(
            &mut block.statements,
            OwnedStatement {
                kind: OwnedInstruction::$variant {
                    $operation,
                    destination,
                },
                $span,
                diagnostic_origins: None,
            },
            $c.statements,
            $span,
        )?;
        block.terminator = Some(OwnedTerminator {
            kind: end,
            $span,
            diagnostic_origins: None,
        });
        budget::append(&mut raw.blocks, block, 1, $span)?;
        Ok(raw)
    }};
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
    source_function!(id, span, signature, c, operation, Vector)
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

/// The nominal owned suffix uses the same fallible row constructor. Source
/// facts select only its signature; independent descriptor/flow checks follow.
pub(super) fn owned_counts(
    operation: crate::frontend::builtin_catalog::owned_vector::Function,
) -> super::super::budget::FunctionCounts {
    let signature = operation.concrete_signature(EnumId(0));
    signature_counts!(signature)
}

pub(super) fn owned_function(
    index: &DeclarationIndex<'_>,
    work: &crate::frontend::declaration_index::WorkMeter,
    rank: usize,
    operation: crate::frontend::builtin_catalog::owned_vector::Function,
) -> Result<RawOwnedFunction, OwnedFailure> {
    let selected = super::owned_vector_source::signature(index, work, rank, operation)
        .map_err(super::vector_source::lower_error)?;
    let id = selected.id;
    let span = selected.anchor;
    let signature = selected.signature;
    let c = owned_counts(operation);
    source_function!(id, span, signature, c, operation, OwnedVector)
}

#[allow(dead_code)]
struct OwnedCarriers {
    local_reservations: [LocalReservation; 2],
    local_rows: [LocalDecl; 2],
    selected: super::owned_vector_source::SelectedSignature,
    returned_selection: Result<super::owned_vector_source::SelectedSignature, Box<Diagnostic>>,
    signature: crate::frontend::builtin_catalog::owned_vector::Signature,
    signature_return:
        Result<crate::frontend::builtin_catalog::owned_vector::Signature, Box<Diagnostic>>,
    keys: &'static [crate::frontend::builtin_catalog::owned_vector::Function],
    iterations: std::iter::Enumerate<
        std::iter::Copied<
            std::slice::Iter<'static, crate::frontend::builtin_catalog::owned_vector::Function>,
        >,
    >,
    next: Option<(
        usize,
        crate::frontend::builtin_catalog::owned_vector::Function,
    )>,
    operation: crate::frontend::builtin_catalog::owned_vector::Function,
    function: RawOwnedFunction,
    returned_function: Result<RawOwnedFunction, OwnedFailure>,
    parameters: std::iter::Enumerate<std::slice::Iter<'static, ParameterTy>>,
    work: &'static crate::frontend::declaration_index::WorkMeter,
    id: hir::DefId,
    span: Span,
    count: super::super::budget::FunctionCounts,
}
pub(super) const fn owned_extra_bytes() -> usize {
    std::mem::size_of::<OwnedCarriers>()
}
