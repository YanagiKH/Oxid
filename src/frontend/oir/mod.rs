//! Private verified scalar/owned OIR with bounded consumers. No serialized contract.
mod execute;
mod lower;
mod native;
mod owned;
mod owned_types;
mod source;
mod verify;

pub(super) use source::{check_source, CheckedSourceProgram};

use super::{
    diagnostic::Diagnostic,
    hir,
    source::{SourceMap, Span},
    typeck,
};

const MAX_LOCALS: usize = super::parser::MAX_NODES;
const MAX_BLOCKS: usize = 3 * super::parser::MAX_NODES;
const MAX_ASSIGNMENTS: usize = super::parser::MAX_NODES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalId(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BlockId(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlaceId(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Place {
    id: PlaceId,
    span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct PlaceDecl {
    ty: hir::Ty,
    span: Span,
}
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
    places: Vec<PlaceDecl>,
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
    merge: Option<BoolMerge>,
    span: Span,
    statements: Vec<Statement>,
    terminator: Option<Terminator>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MergeInput {
    predecessor: BlockId,
    value: Operand,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct BoolMerge {
    operator_span: Span,
    destination: LocalId,
    incoming: [MergeInput; 2],
    span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Operand {
    local: LocalId,
    span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Statement {
    Assign(Assign),
    Initialize {
        place: Place,
        value: Operand,
        span: Span,
    },
    Store {
        place: Place,
        value: Operand,
        operator_span: Span,
        span: Span,
    },
}
impl Statement {
    fn span(&self) -> Span {
        match self {
            Self::Assign(assign) => assign.span,
            Self::Initialize { span, .. } | Self::Store { span, .. } => *span,
        }
    }
    fn as_assignment(&self) -> Option<&Assign> {
        match self {
            Self::Assign(assign) => Some(assign),
            _ => None,
        }
    }
    #[cfg(test)]
    fn assignment(&self) -> &Assign {
        self.as_assignment().expect("expected SSA assignment")
    }
    #[cfg(test)]
    fn assignment_mut(&mut self) -> &mut Assign {
        match self {
            Self::Assign(assign) => assign,
            _ => panic!("expected SSA assignment"),
        }
    }
}
impl Function {
    fn slot_count(&self) -> usize {
        self.locals.len() + self.places.len()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Assign {
    destination: LocalId,
    value: Rvalue,
    span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Rvalue {
    Load(Place),
    NotBool {
        operand: Operand,
        operator_span: Span,
    },
    Bool(bool),
    I32(i32),
    Unit,
    Copy(Operand),
    CompareScalar {
        op: hir::ComparisonOp,
        left: Operand,
        right: Operand,
        operator_span: Span,
    },
    CheckedI32 {
        op: hir::ArithmeticOp,
        left: Operand,
        right: Operand,
        operator_span: Span,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Terminator {
    kind: TerminatorKind,
    span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum TerminatorKind {
    Branch {
        condition: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
    Goto {
        target: BlockId,
    },
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
    /// Entry identity is carried from resolved source declarations, never guessed
    /// from arbitrary verified-IR origin text. The verifier owns the signature.
    pub(super) fn run(&self, entry: Option<hir::DefId>) -> Result<Scalar, RunFailure> {
        let id = entry.ok_or(RunFailure::Entry(None))?;
        let function = self
            .program
            .functions
            .get(id.0)
            .filter(|f| f.id == id)
            .ok_or_else(|| {
                RunFailure::Internal(OirFailure::new(
                    FailureKind::InvalidFunctionId,
                    "oir-run",
                    None,
                ))
            })?;
        if function.param_count != 0 {
            return Err(RunFailure::Entry(Some(function.span)));
        }
        execute::run(self, id)
    }
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
    InvalidMerge,
    MissingTerminator,
    InvalidLocal,
    InvalidPlace,
    InvalidTarget,
    TypeMismatch,
    Arity,
    Uninitialized,
    AlreadyInitialized,
    Unreachable,
    InvalidSpan,
    BuilderClosed,
    IncompleteBody,
    Accounting,
    CallFrame,
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

/// Closed scalar representation, independent of the legacy dynamic Value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scalar {
    Bool(bool),
    I32(i32),
    Unit,
}
impl Scalar {
    fn ty(self) -> hir::Ty {
        match self {
            Self::Bool(_) => hir::Ty::Bool,
            Self::I32(_) => hir::Ty::I32,
            Self::Unit => hir::Ty::Unit,
        }
    }
    pub(super) fn json(self) -> String {
        match self {
            Self::Bool(value) => format!("{{\"type\":\"bool\",\"value\":{value}}}"),
            Self::Unit => "{\"type\":\"unit\"}".into(),
            Self::I32(value) => format!("{{\"type\":\"i32\",\"value\":{value}}}"),
        }
    }
}
impl std::fmt::Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::I32(value) => write!(f, "{value}"),
            Self::Unit => f.write_str("()"),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RunFailure {
    Entry(Option<Span>),
    Fuel(Span),
    Frames(Span),
    Slots(Span),
    Overflow(Span),
    Internal(OirFailure),
}
impl RunFailure {
    pub(super) fn diagnostic(&self, sources: &SourceMap) -> Box<Diagnostic> {
        let (code, message, span) = match *self {
            Self::Entry(None) => (
                "E0600",
                "typed-preview run requires a declared zero-argument main returning bool, i32 or ()",
                None,
            ),
            Self::Entry(span) => ("E0600", "typed-preview main must have no parameters", span),
            Self::Fuel(span) => ("E0601", "execution fuel exhausted", Some(span)),
            Self::Frames(span) => ("E0602", "live call-frame limit exceeded", Some(span)),
            Self::Slots(span) => ("E0603", "live local-slot limit exceeded", Some(span)),
            Self::Overflow(span) => ("E0604", "checked i32 arithmetic overflow", Some(span)),
            Self::Internal(ref error) => return error.diagnostic(sources),
        };
        Diagnostic::new(
            code,
            "oir-run",
            message,
            span.filter(|span| sources.is_valid_span(*span)),
        )
    }
}

/// One mandatory boundary for all successful typed-preview checking and running.
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

#[cfg(test)]
mod cfg_tests;

#[cfg(test)]
mod i32_tests;

#[cfg(test)]
mod arithmetic_tests;

#[cfg(test)]
mod comparison_tests;

#[cfg(test)]
mod logical_tests;

#[cfg(test)]
mod mutable_tests;

#[cfg(test)]
mod loop_control_tests;
