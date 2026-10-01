//! Private, check-only straight-line OIR. No executable or serialized contract.
mod lower;
mod verify;

use super::{
    diagnostic::Diagnostic,
    hir,
    source::{SourceMap, Span},
    typeck,
};

const MAX_LOCALS: usize = super::parser::MAX_NODES;
const MAX_BLOCKS: usize = super::parser::MAX_NODES;
const MAX_ASSIGNMENTS: usize = super::parser::MAX_NODES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalId(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BlockId(usize);
#[derive(Clone, Debug, PartialEq, Eq)]
struct Program {
    functions: Vec<Function>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Function {
    id: hir::DefId,
    span: Span,
    result: hir::Ty,
    param_count: usize,
    locals: Vec<LocalDecl>,
    entry: BlockId,
    blocks: Vec<BasicBlock>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalDecl {
    ty: hir::Ty,
    kind: LocalKind,
    span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalKind {
    Parameter,
    Binding,
    Temporary,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct BasicBlock {
    span: Span,
    statements: Vec<Assign>,
    terminator: Option<Terminator>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Operand {
    local: LocalId,
    span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Assign {
    destination: LocalId,
    value: Rvalue,
    span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Rvalue {
    Bool(bool),
    Unit,
    Copy(Operand),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Terminator {
    kind: TerminatorKind,
    span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum TerminatorKind {
    Call {
        target: hir::DefId,
        args: Vec<Operand>,
        destination: LocalId,
        continuation: BlockId,
    },
    Return(Operand),
}

/// Only verification may construct this witness. No mutable/raw access is exposed.
#[derive(Debug)]
pub(super) struct VerifiedProgram {
    program: Program,
}
impl VerifiedProgram {
    pub(super) fn function_count(&self) -> usize {
        self.program.functions.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailureKind {
    ResourceLimit(&'static str),
    InvalidFunctionId,
    ParameterCount,
    ParameterKind,
    NoBlocks,
    InvalidBlock,
    MissingTerminator,
    InvalidLocal,
    InvalidTarget,
    TypeMismatch,
    Arity,
    Uninitialized,
    AlreadyInitialized,
    Cycle,
    Unreachable,
    InvalidSpan,
    BuilderClosed,
    IncompleteBody,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct OirFailure {
    kind: FailureKind,
    stage: &'static str,
    span: Option<Span>,
}
impl OirFailure {
    fn new(kind: FailureKind, stage: &'static str, span: Option<Span>) -> Self {
        Self { kind, stage, span }
    }
    pub(super) fn diagnostic(&self, sources: &SourceMap) -> Box<Diagnostic> {
        let resource =
            matches!(self.kind, FailureKind::ResourceLimit(_)) && self.stage == "oir-lower";
        Diagnostic::new(
            if resource { "E0400" } else { "E0500" },
            self.stage,
            if resource {
                format!("OIR resource limit: {:?}", self.kind)
            } else {
                format!("internal compiler error: OIR invariant {:?}", self.kind)
            },
            self.span.filter(|span| sources.is_valid_span(*span)),
        )
    }
}

/// One mandatory boundary for all successful typed-preview checking.
pub(super) fn lower_and_verify(
    typed: &typeck::TypedProgram,
    sources: &SourceMap,
) -> Result<VerifiedProgram, OirFailure> {
    verify::verify(lower::lower(typed)?, sources)
}

/// Shared checked accounting, before lowerer allocations and verifier traversal.
#[derive(Default)]
struct Budget {
    locals: usize,
    blocks: usize,
    assignments: usize,
}
impl Budget {
    fn add(
        counter: &mut usize,
        amount: usize,
        limit: usize,
        name: &'static str,
        stage: &'static str,
        span: Option<Span>,
    ) -> Result<(), OirFailure> {
        *counter = counter
            .checked_add(amount)
            .filter(|&n| n <= limit)
            .ok_or_else(|| OirFailure::new(FailureKind::ResourceLimit(name), stage, span))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
