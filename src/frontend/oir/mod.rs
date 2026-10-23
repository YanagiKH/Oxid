//! Private verified scalar/owned OIR with bounded consumers. No serialized contract.
mod execute;
mod lower;
mod native;
mod owned;
pub(super) mod owned_types;
pub(super) mod project;
mod source;
mod verify;

pub(in crate::frontend) use owned::process;
pub(super) use source::{
    check_source, import_checked, import_produced, CheckedSourceProgram, Imported, ProcessFailure,
    IMPORT_BYTES,
};

use super::{
    diagnostic::Diagnostic,
    hir::{self, Ty},
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
    CheckedI32ToU8 {
        operand: Operand,
        name_span: Span,
        source_expr: crate::frontend::ast::ExprId,
    },
    U8ToI32 {
        operand: Operand,
        name_span: Span,
        source_expr: crate::frontend::ast::ExprId,
    },
    Load(Place),
    NotBool {
        operand: Operand,
        operator_span: Span,
    },
    CheckedNegateI32 {
        operand: Operand,
        operator_span: Span,
    },
    Bool(bool),
    I32(i32),
    U8(u8),
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
    I64(i64),
    WidenI64 {
        operand: Operand,
        name_span: Span,
        source_expr: crate::frontend::ast::ExprId,
    },
    CheckedI64ToI32 {
        operand: Operand,
        name_span: Span,
        source_expr: crate::frontend::ast::ExprId,
    },
    CheckedI64ToU8 {
        operand: Operand,
        name_span: Span,
        source_expr: crate::frontend::ast::ExprId,
    },
    CheckedNegateI64 {
        operand: Operand,
        operator_span: Span,
    },
    CheckedI64 {
        op: hir::ArithmeticOp,
        left: Operand,
        right: Operand,
        operator_span: Span,
    },
    Integer {
        ty: Ty,
        bits: u64,
    },
    CheckedInteger {
        op: hir::ArithmeticOp,
        left: Operand,
        right: Operand,
        operator_span: Span,
    },
    CheckedNegateInteger {
        operand: Operand,
        operator_span: Span,
    },
    CheckedIntegerCast {
        operand: Operand,
        target: Ty,
        name_span: Span,
        source_expr: crate::frontend::ast::ExprId,
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
        self.run_with_execution(
            entry,
            crate::frontend::build_config::ExecutionPolicy::Bounded,
        )
    }

    pub(super) fn run_with_execution(
        &self,
        entry: Option<hir::DefId>,
        execution: crate::frontend::build_config::ExecutionPolicy,
    ) -> Result<Scalar, RunFailure> {
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
        if function.result == hir::Ty::U8 {
            return Err(RunFailure::ByteEntry(function.span));
        }
        if execution.hosted() {
            execute::run_with_execution(self, id, execution)
        } else {
            execute::run(self, id)
        }
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
    UnauthenticatedConversion,
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
    U8(u8),
    Unit,
}
impl Scalar {
    fn ty(self) -> hir::Ty {
        match self {
            Self::Bool(_) => hir::Ty::Bool,
            Self::I32(_) => hir::Ty::I32,
            Self::U8(_) => hir::Ty::U8,
            Self::Unit => hir::Ty::Unit,
        }
    }
    /// Public entry results retain the closed bool/i32/unit schema.
    pub(super) fn json(self) -> Option<String> {
        Some(match self {
            Self::Bool(value) => format!("{{\"type\":\"bool\",\"value\":{value}}}"),
            Self::Unit => "{\"type\":\"unit\"}".into(),
            Self::I32(value) => format!("{{\"type\":\"i32\",\"value\":{value}}}"),
            Self::U8(_) => return None,
        })
    }
}
impl std::fmt::Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::I32(value) => write!(f, "{value}"),
            Self::Unit => f.write_str("()"),
            Self::U8(_) => Err(std::fmt::Error),
        }
    }
}

/// Typed results may carry a full-width integer; the closed scalar ABI retains
/// its original representation for existing programs and private wire formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeScalar {
    Narrow(Scalar),
    I64(i64),
    I8(i8),
    I16(i16),
    U16(u16),
    U32(u32),
    U64(u64),
    Isize(i64),
    Usize(u64),
}
impl From<Scalar> for RuntimeScalar {
    fn from(value: Scalar) -> Self {
        Self::Narrow(value)
    }
}
impl RuntimeScalar {
    pub(super) fn integer(self) -> Option<(Ty, u64)> {
        Some(match self {
            Self::Narrow(Scalar::I32(value)) => (Ty::I32, value as i64 as u64),
            Self::Narrow(Scalar::U8(value)) => (Ty::U8, u64::from(value)),
            Self::I64(value) => (Ty::I64, value as u64),
            Self::I8(value) => (Ty::I8, value as i64 as u64),
            Self::I16(value) => (Ty::I16, value as i64 as u64),
            Self::U16(value) => (Ty::U16, u64::from(value)),
            Self::U32(value) => (Ty::U32, u64::from(value)),
            Self::U64(value) => (Ty::U64, value),
            Self::Isize(value) => (Ty::Isize, value as u64),
            Self::Usize(value) => (Ty::Usize, value),
            Self::Narrow(Scalar::Bool(_) | Scalar::Unit) => return None,
        })
    }
    pub(super) fn from_integer(ty: Ty, bits: u64) -> Option<Self> {
        match ty {
            Ty::I8 => i8::try_from(bits as i64).ok().map(Self::I8),
            Ty::I16 => i16::try_from(bits as i64).ok().map(Self::I16),
            Ty::I32 => i32::try_from(bits as i64)
                .ok()
                .map(|value| Self::Narrow(Scalar::I32(value))),
            Ty::I64 => Some(Self::I64(bits as i64)),
            Ty::U8 => u8::try_from(bits)
                .ok()
                .map(|value| Self::Narrow(Scalar::U8(value))),
            Ty::U16 => u16::try_from(bits).ok().map(Self::U16),
            Ty::U32 => u32::try_from(bits).ok().map(Self::U32),
            Ty::U64 => Some(Self::U64(bits)),
            Ty::Isize => Some(Self::Isize(bits as i64)),
            Ty::Usize => Some(Self::Usize(bits)),
            Ty::Bool | Ty::Unit => None,
        }
    }
    pub(super) fn narrow(self) -> Option<Scalar> {
        match self {
            Self::Narrow(value) => Some(value),
            Self::I64(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::Isize(_)
            | Self::Usize(_) => None,
        }
    }
    pub(super) fn json(self) -> Option<String> {
        match self {
            Self::Narrow(Scalar::U8(value)) => {
                Some(format!("{{\"type\":\"u8\",\"value\":{value}}}"))
            }
            Self::Narrow(value) => value.json(),
            // Decimal text preserves the complete value in JSON consumers that
            // represent numeric tokens using a 53-bit floating-point mantissa.
            Self::I64(value) => Some(format!("{{\"type\":\"i64\",\"value\":\"{value}\"}}")),
            Self::I8(value) => Some(format!("{{\"type\":\"i8\",\"value\":{value}}}")),
            Self::I16(value) => Some(format!("{{\"type\":\"i16\",\"value\":{value}}}")),
            Self::U16(value) => Some(format!("{{\"type\":\"u16\",\"value\":{value}}}")),
            Self::U32(value) => Some(format!("{{\"type\":\"u32\",\"value\":{value}}}")),
            Self::U64(value) => Some(format!("{{\"type\":\"u64\",\"value\":\"{value}\"}}")),
            Self::Isize(value) => Some(format!("{{\"type\":\"isize\",\"value\":\"{value}\"}}")),
            Self::Usize(value) => Some(format!("{{\"type\":\"usize\",\"value\":\"{value}\"}}")),
        }
    }
}
impl std::fmt::Display for RuntimeScalar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Narrow(Scalar::U8(value)) => value.fmt(formatter),
            Self::Narrow(value) => value.fmt(formatter),
            Self::I64(value) => value.fmt(formatter),
            Self::I8(value) => value.fmt(formatter),
            Self::I16(value) => value.fmt(formatter),
            Self::U16(value) => value.fmt(formatter),
            Self::U32(value) => value.fmt(formatter),
            Self::U64(value) => value.fmt(formatter),
            Self::Isize(value) => value.fmt(formatter),
            Self::Usize(value) => value.fmt(formatter),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RunFailure {
    Entry(Option<Span>),
    ByteEntry(Span),
    Fuel(Span),
    Frames(Span),
    Slots(Span),
    Overflow(Span),
    DivisionByZero(Span),
    ByteRange(Span),
    WideOverflow(Span),
    WideDivisionByZero(Span),
    I64ToI32Range(Span),
    I64ToU8Range(Span),
    IntegerOverflow { ty: Ty, span: Span },
    IntegerDivisionByZero { ty: Ty, span: Span },
    IntegerRange { input: Ty, target: Ty, span: Span },
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
            Self::ByteEntry(span) => ("E0600", "typed-preview main must return bool, i32 or ()", Some(span)),
            Self::Fuel(span) => ("E0601", "execution fuel exhausted", Some(span)),
            Self::Frames(span) => ("E0602", "live call-frame limit exceeded", Some(span)),
            Self::Slots(span) => ("E0603", "live local-slot limit exceeded", Some(span)),
            Self::Overflow(span) => ("E0604", "checked i32 arithmetic overflow", Some(span)),
            Self::DivisionByZero(span) => ("E0607", "checked i32 division by zero", Some(span)),
            Self::ByteRange(span) => ("E0610", "checked i32 to u8 conversion out of range", Some(span)),
            Self::WideOverflow(span) => ("E0604", "checked i64 arithmetic overflow", Some(span)),
            Self::WideDivisionByZero(span) => ("E0607", "checked i64 division by zero", Some(span)),
            Self::I64ToI32Range(span) => ("E0610", "checked i64 to i32 conversion out of range", Some(span)),
            Self::I64ToU8Range(span) => ("E0610", "checked i64 to u8 conversion out of range", Some(span)),
            Self::IntegerOverflow { ty, span } => return Diagnostic::new(
                "E0604", "oir-run", format!("checked {ty} arithmetic overflow"),
                Some(span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::IntegerDivisionByZero { ty, span } => return Diagnostic::new(
                "E0607", "oir-run", format!("checked {ty} division by zero"),
                Some(span).filter(|span| sources.is_valid_span(*span)),
            ),
            Self::IntegerRange { input, target, span } => {
                // Primitive names are at most five bytes. A fixed reservation
                // keeps formatting inside the existing diagnostic envelope.
                use std::fmt::Write;
                let mut message = String::with_capacity(64);
                write!(&mut message, "checked {input} to {target} conversion out of range").unwrap();
                return Diagnostic::new("E0610", "oir-run", message,
                    Some(span).filter(|span| sources.is_valid_span(*span)));
            }
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

/// Retained direct raw scalar test seam. Source production adds association checks.
#[cfg(test)]
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

#[cfg(test)]
mod negation_raw_tests;

#[cfg(test)]
mod unary_source_tests;

#[cfg(test)]
mod u8_tests;

#[cfg(test)]
mod u8_carrier_tests;
