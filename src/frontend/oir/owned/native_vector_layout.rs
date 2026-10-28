//! Shared physical scratch admission, independent from emitted text.
//! The execution plan and native storage use the actual carrier size.
//! Typed fixed-array/slice ranges need no such scratch.
use super::*;

#[repr(C)]
struct IndexScratch {
    pointer: *mut u8,
}
pub(in crate::frontend::oir::owned) const INDEX_BYTES: usize = size_of::<IndexScratch>();
pub(in crate::frontend::oir::owned) type IndexedAccess =
    (AccessBase, Operand, hir::Ty, Option<usize>);
pub(in crate::frontend::oir::owned) type IndexedAccessResult =
    Result<Option<IndexedAccess>, AdmissionFailure>;

pub(in crate::frontend::oir::owned) fn indexed(
    declarations: &Declarations,
    f: &RawOwnedFunction,
    instruction: &OwnedInstruction,
) -> IndexedAccessResult {
    let base_type = |base| match base {
        AccessBase::Owner(owner) => f
            .owners
            .get(owner.0)
            .map(|row| BorrowedTy::Exact(row.aggregate())),
        AccessBase::Parameter(reference) => {
            f.references.get(reference.0).map(ReferenceDecl::referent)
        }
        AccessBase::Loan(loan) => f.loans.get(loan.0).map(LoanDecl::referent),
    };
    match instruction {
        OwnedInstruction::ReadIndex { base, index, .. }
        | OwnedInstruction::WriteIndex { base, index, .. } => Ok(match base_type(*base) {
            Some(BorrowedTy::Exact(AggregateTy::Vector(element))) => {
                Some((*base, *index, element, None))
            }
            Some(_) => None,
            None => {
                return Err(AdmissionFailure::new(
                    "vector indexed authority",
                    Some(f.span),
                ))
            }
        }),
        OwnedInstruction::ReadProjection {
            base,
            path,
            index: Some(index),
            ..
        }
        | OwnedInstruction::WriteProjection {
            base,
            path,
            index: Some(index),
            ..
        } => {
            let Some(BorrowedTy::Exact(root)) = base_type(*base) else {
                return Err(AdmissionFailure::new(
                    "vector projection authority",
                    Some(f.span),
                ));
            };
            let (value, offset) = declarations
                .projection(root, path)
                .map_err(|_| AdmissionFailure::new("vector indexed projection", Some(f.span)))?;
            Ok(match value {
                ValueTy::Owned(AggregateTy::Vector(element)) => {
                    Some((*base, *index, element, Some(offset)))
                }
                _ => None,
            })
        }
        _ => Ok(None),
    }
}
pub(in crate::frontend::oir::owned) fn frame_bytes(
    declarations: &Declarations,
    f: &RawOwnedFunction,
) -> Result<usize, AdmissionFailure> {
    for block in &f.blocks {
        for statement in &block.statements {
            if indexed(declarations, f, &statement.kind)?.is_some() {
                return Ok(INDEX_BYTES);
            }
        }
    }
    Ok(0)
}

/// One reusable dense record value, paid as real activation/native payload.
pub(in crate::frontend::oir::owned) fn record_scratch_bytes(
    declarations: &Declarations,
    f: &RawOwnedFunction,
) -> Result<usize, AdmissionFailure> {
    let mut bytes = 0;
    for block in &f.blocks {
        for statement in &block.statements {
            let record = match &statement.kind {
                OwnedInstruction::CopyIndexWrite { source, .. } => {
                    match f.owners[source.0].aggregate() {
                        AggregateTy::Record(id) => Some(id),
                        _ => {
                            return Err(AdmissionFailure::new(
                                "record vector source type",
                                Some(statement.span),
                            ))
                        }
                    }
                }
                OwnedInstruction::Vector { operation, .. }
                    if matches!(
                        operation.operation(),
                        crate::frontend::builtin_catalog::vector::Operation::TryPush
                            | crate::frontend::builtin_catalog::vector::Operation::TryResize
                    ) =>
                {
                    operation.record_id()
                }
                _ => None,
            };
            if let Some(record) = record {
                bytes = bytes.max(declarations.record_packed_bytes(record).map_err(|_| {
                    AdmissionFailure::new("record vector packed extent", Some(statement.span))
                })?);
            }
        }
    }
    Ok(bytes)
}
