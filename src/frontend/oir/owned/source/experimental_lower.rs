//! Fallible source producer. The independent raw descriptor checker and ordinary
//! shape/flow proof certify the canonical suffix before either consumer sees it.
use super::super::*;
use super::{budget, builtin_lower::check_capacity};
use crate::frontend::{
    builtin_catalog::experimental::{Function, Signature},
    declaration_index::DeclarationIndex,
};

pub(super) fn counts(kind: Function) -> super::super::budget::FunctionCounts {
    let signature = kind.signature(|_| EnumId(0));
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
    let id = index.experimental_function_id(operation).map_err(fail)?;
    let span = index.experimental_anchor(operation.item()).map_err(fail)?;
    let signature = index.experimental_signature(operation).map_err(fail)?;
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
            kind: OwnedInstruction::Experimental {
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
    iteration: std::array::IntoIter<Function, 10>,
}
pub(super) fn extra_bytes() -> usize {
    std::mem::size_of::<Carriers>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{
        builtin_catalog::experimental::{Enumeration, Item},
        declaration_index::{collect_originals, IndexLimits, SourceOwner, WorkMeter},
        project::{budget::Allocator, ProjectLimits, ProjectSources},
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    #[test]
    fn experimental_catalog_raw_descriptors_are_closed_and_verified() {
        let path = std::env::temp_dir().join(format!(
            "oxid-experimental-catalog-{}-{}.ox",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let text: String = Item::ALL
            .into_iter()
            .map(|item| {
                format!(
                    "use {}::{}::{};\n",
                    item.path()[0],
                    item.path()[1],
                    item.name()
                )
            })
            .collect();
        std::fs::write(&path, text).unwrap();
        let project =
            ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default()).unwrap();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = collect_originals(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        let mut raw = RawOwnedProgram {
            builtins: index.builtin_set(),
            enums: Vec::new(),
            records: Vec::new(),
            functions: Vec::new(),
        };
        for item in Enumeration::ALL {
            let id = index.experimental_enum_id(item).unwrap();
            let span = index.experimental_anchor(item.item()).unwrap();
            raw.enums.push(RawEnumDecl {
                id,
                span,
                variants: item
                    .members()
                    .iter()
                    .enumerate()
                    .map(|(index, _)| RawVariantDecl {
                        id: VariantId {
                            enumeration: id,
                            index,
                        },
                        span,
                        payload: item
                            .payload(index)
                            .map(|ty| ParameterTy::Value(ValueTy::Scalar(ty))),
                    })
                    .collect(),
            });
        }
        for item in Function::ALL {
            raw.functions.push(function(&index, item).unwrap());
        }
        builtins::check(&raw).unwrap();
        for ordinal in 0..raw.functions.len() {
            let saved = raw.functions[ordinal].clone();
            raw.functions[ordinal].blocks[0]
                .statements
                .last_mut()
                .unwrap()
                .kind = OwnedInstruction::Experimental {
                operation: Function::New,
                destination: CallResult::Scalar(LocalId(999)),
            };
            assert!(builtins::check(&raw).is_err(), "forged operation {ordinal}");
            raw.functions[ordinal] = saved;
            let result = raw.functions[ordinal].result;
            raw.functions[ordinal].result = if result == ValueTy::Scalar(hir::Ty::I32) {
                ValueTy::Scalar(hir::Ty::Unit)
            } else {
                ValueTy::Scalar(hir::Ty::I32)
            };
            assert!(builtins::check(&raw).is_err(), "forged signature {ordinal}");
            raw.functions[ordinal].result = result;
        }
        let saved = raw.enums[0].variants[0].payload;
        raw.enums[0].variants[0].payload =
            Some(ParameterTy::Value(ValueTy::Owned(AggregateTy::ByteBuf)));
        assert!(builtins::check(&raw).is_err());
        raw.enums[0].variants[0].payload = saved;
        builtins::check(&raw).unwrap();
        let crate::frontend::source::SourceView::Map(sources) = index.sources().view() else {
            panic!("project source")
        };
        verified::verify_owned(
            RawOwnedProgram {
                builtins: raw.builtins,
                enums: raw.enums.clone(),
                records: vec![],
                functions: raw.functions.clone(),
            },
            sources,
        )
        .unwrap();
        // A complete canonical suffix grants no intrinsic authority to an
        // otherwise unused ordinary function, even with identical body/spelling.
        raw.functions.insert(0, raw.functions[0].clone());
        for (ordinal, function) in raw.functions.iter_mut().enumerate() {
            function.id = hir::DefId(ordinal);
        }
        builtins::check(&raw).unwrap();
        assert!(verified::verify_owned(raw, sources).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
