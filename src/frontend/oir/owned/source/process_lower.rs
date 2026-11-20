//! Canonical, separately admitted scalar descriptors for process notifications.
use super::super::*;
use super::{budget, builtin_lower::check_capacity};
use crate::frontend::{
    builtin_catalog::termination,
    declaration_index::{DeclarationIndex, WorkMeter},
};
use std::mem::size_of;

pub(super) fn counts() -> super::super::budget::FunctionCounts {
    super::super::budget::FunctionCounts {
        locals: 1,
        blocks: 1,
        statements: 1,
        ownership_active: true,
        ..Default::default()
    }
}
fn exact<T>(count: usize) -> Result<Vec<T>, OwnedFailure> {
    let result = budget::reserve(count)?;
    check_capacity(&result, count)?;
    Ok(result)
}
pub(super) fn function(
    index: &DeclarationIndex<'_>,
    operation: termination::Function,
    work: &WorkMeter,
) -> Result<RawOwnedFunction, OwnedFailure> {
    let fail = |_| OwnedFailure::malformed(Malformed::Binding, index.sources().eof());
    let id = index.process_function_id(operation).map_err(fail)?;
    let span = index.process_anchor(operation, work).map_err(fail)?;
    work.debit(1, span, "process canonical raw descriptor")
        .map_err(fail)?;
    let mut locals = exact(1)?;
    budget::append(
        &mut locals,
        LocalDecl {
            ty: hir::Ty::Bool,
            kind: LocalKind::Temporary,
            span,
        },
        1,
        span,
    )?;
    let mut statements = exact(1)?;
    budget::append(
        &mut statements,
        OwnedStatement {
            kind: OwnedInstruction::Experimental {
                operation: operation.operation(),
                destination: CallResult::Scalar(LocalId(0)),
            },
            span,
            diagnostic_origins: None,
        },
        1,
        span,
    )?;
    let mut blocks = exact(1)?;
    budget::append(
        &mut blocks,
        OwnedBlock {
            span,
            merge: None,
            statements,
            terminator: Some(OwnedTerminator {
                kind: OwnedTerminatorKind::ReturnScalar(Operand {
                    local: LocalId(0),
                    span,
                }),
                span,
                diagnostic_origins: None,
            }),
        },
        1,
        span,
    )?;
    Ok(RawOwnedFunction {
        id,
        span,
        result: ValueTy::Scalar(hir::Ty::Bool),
        parameters: Vec::new(),
        locals,
        places: Vec::new(),
        owners: Vec::new(),
        references: Vec::new(),
        calls: Vec::new(),
        loans: Vec::new(),
        matches: Vec::new(),
        entry: BlockId(0),
        blocks,
    })
}

#[allow(dead_code)]
struct Carriers {
    index: &'static DeclarationIndex<'static>,
    work: &'static WorkMeter,
    operation: termination::Function,
    id_return: Result<hir::DefId, Box<crate::frontend::diagnostic::Diagnostic>>,
    anchor_return: Result<Span, Box<crate::frontend::diagnostic::Diagnostic>>,
    raw: RawOwnedFunction,
    raw_return: Result<RawOwnedFunction, OwnedFailure>,
    block: OwnedBlock,
    statement: OwnedStatement,
    local: LocalDecl,
    counts: super::super::budget::FunctionCounts,
}
pub(super) const fn extra_bytes() -> usize {
    size_of::<Carriers>()
}
