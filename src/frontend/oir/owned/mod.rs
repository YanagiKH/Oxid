//! Authoritative private ownership OIR and bounded verified consumers.
//! Production record source lowering and both consumers require the sealed witness.
//! Fixed-array carriers are rejected before an executable witness can be built.
#![allow(dead_code)]
// Denials retain exact verifier-derived facts on the stack. Boxing this fixed
// transport would add an allocation on ownership/resource failure paths.
#![allow(clippy::result_large_err)]
use super::{owned_types::*, *};
mod budget;
mod cfg;
mod execute;
mod flow;
mod native;
mod plan;
mod shape;
pub(super) mod source;
pub(super) use source::SourceProgram;
mod storage;
mod verified;
#[cfg(test)]
use verified::{verify_owned, verify_with_limits};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReferenceParamId(usize);
#[derive(Debug)]
struct RawOwnedProgram {
    records: Vec<RawRecordDecl>,
    functions: Vec<RawOwnedFunction>,
}
#[derive(Clone, Debug)]
struct RawOwnedFunction {
    id: hir::DefId,
    span: Span,
    result: ValueTy,
    parameters: Vec<ParameterBinding>,
    locals: Vec<LocalDecl>,
    places: Vec<PlaceDecl>,
    owners: Vec<OwnerDecl>,
    references: Vec<ReferenceDecl>,
    calls: Vec<CallDecl>,
    loans: Vec<LoanDecl>,
    entry: BlockId,
    blocks: Vec<OwnedBlock>,
}
#[derive(Clone, Copy, Debug)]
enum ParameterBinding {
    Scalar(LocalId),
    Owned(OwnerPlaceId),
    Reference(ReferenceParamId),
}
#[derive(Clone, Debug)]
struct OwnerDecl {
    aggregate: AggregateSlot,
    kind: OwnerKind,
    span: Span,
}
impl OwnerDecl {
    fn aggregate(&self) -> AggregateTy {
        self.aggregate.aggregate()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OwnerKind {
    Parameter { position: usize },
    Local { mutable: bool },
    Temporary,
    StagedArgument { call: CallSiteId, argument: usize },
    CallResult { call: CallSiteId },
}
#[derive(Clone, Debug)]
struct ReferenceDecl {
    referent: BorrowedSlot,
    kind: BorrowKind,
    position: usize,
    span: Span,
}
impl ReferenceDecl {
    fn referent(&self) -> BorrowedTy {
        self.referent.referent()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AccessBase {
    Owner(OwnerPlaceId),
    Parameter(ReferenceParamId),
}
#[derive(Clone, Debug)]
struct LoanDecl {
    call: CallSiteId,
    argument: usize,
    authority: AccessBase,
    projection: Vec<FieldId>,
    kind: BorrowKind,
    referent: BorrowedSlot,
    span: Span,
}
impl LoanDecl {
    fn referent(&self) -> BorrowedTy {
        self.referent.referent()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArgumentSlot {
    Scalar,
    Owned(OwnerPlaceId),
    Borrow(LoanId),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CallResult {
    Scalar(LocalId),
    Owned(OwnerPlaceId),
}
#[derive(Clone, Debug)]
struct CallDecl {
    target: hir::DefId,
    arguments: Vec<ArgumentSlot>,
    result: CallResult,
    parent: Option<(CallSiteId, usize)>,
    span: Span,
}
#[derive(Clone, Debug)]
struct OwnedBlock {
    merge: Option<BoolMerge>,
    span: Span,
    statements: Vec<OwnedStatement>,
    terminator: Option<OwnedTerminator>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DiagnosticOrigins {
    primary: Span,
    cause: Span,
}
#[derive(Clone, Debug)]
struct OwnedStatement {
    kind: OwnedInstruction,
    span: Span,
    diagnostic_origins: Option<DiagnosticOrigins>,
}
impl OwnedStatement {
    fn primary_span(&self) -> Span {
        self.diagnostic_origins.map_or(self.span, |o| o.primary)
    }
    fn cause_span(&self) -> Span {
        self.diagnostic_origins.map_or(self.span, |o| o.cause)
    }
}
#[derive(Clone, Copy, Debug)]
enum FieldInitializer {
    Scalar(Operand),
    // Must name a complete ordinary Temporary; producer stages at evaluation
    // time using MoveInitialize. The verifier independently enforces consumption.
    Owned(OwnerPlaceId),
}

#[derive(Clone, Debug)]
enum OwnedInstruction {
    ConstructComposite {
        destination: OwnerPlaceId,
        fields: Vec<(FieldId, FieldInitializer)>,
    },
    ReadProjection {
        destination: LocalId,
        base: AccessBase,
        path: Vec<FieldId>,
        index: Option<Operand>,
    },
    WriteProjection {
        base: AccessBase,
        path: Vec<FieldId>,
        index: Option<Operand>,
        value: Operand,
    },
    ProjectionLength {
        destination: LocalId,
        base: AccessBase,
        path: Vec<FieldId>,
    },
    Scalar(Statement),
    StorageLive(OwnerPlaceId),
    StorageEnd(OwnerPlaceId),
    Construct {
        destination: OwnerPlaceId,
        fields: Vec<(FieldId, Operand)>,
    },
    ConstructArray {
        destination: OwnerPlaceId,
        elements: Vec<Operand>,
    },
    MoveInitialize {
        destination: OwnerPlaceId,
        source: OwnerPlaceId,
    },
    Replace {
        destination: OwnerPlaceId,
        source: OwnerPlaceId,
    },
    Discard(OwnerPlaceId),
    ReadField {
        destination: LocalId,
        base: AccessBase,
        field: FieldId,
    },
    WriteField {
        base: AccessBase,
        field: FieldId,
        value: Operand,
    },
    ReadIndex {
        destination: LocalId,
        base: AccessBase,
        index: Operand,
    },
    WriteIndex {
        base: AccessBase,
        index: Operand,
        value: Operand,
    },
    ArrayLength {
        destination: LocalId,
        base: AccessBase,
    },
    OpenCall(CallSiteId),
    PrepareScalar {
        call: CallSiteId,
        argument: usize,
        value: Operand,
    },
    PrepareOwned {
        call: CallSiteId,
        argument: usize,
        source: OwnerPlaceId,
    },
    PrepareBorrow {
        call: CallSiteId,
        argument: usize,
        loan: LoanId,
    },
}
#[derive(Clone, Debug)]
struct OwnedTerminator {
    kind: OwnedTerminatorKind,
    span: Span,
    diagnostic_origins: Option<DiagnosticOrigins>,
}
impl OwnedTerminator {
    fn primary_span(&self) -> Span {
        self.diagnostic_origins.map_or(self.span, |o| o.primary)
    }
    fn cause_span(&self) -> Span {
        self.diagnostic_origins.map_or(self.span, |o| o.cause)
    }
}
#[derive(Clone, Debug)]
enum OwnedTerminatorKind {
    Branch {
        condition: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
    Goto(BlockId),
    Invoke {
        call: CallSiteId,
        continuation: BlockId,
    },
    ReturnScalar(Operand),
    ReturnOwned(OwnerPlaceId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Malformed {
    Id,
    Span,
    Type,
    Binding,
    OwnerClass,
    CanonicalSite,
    CallParent,
    MissingTerminator,
    Scalar(FailureKind),
    Declaration(DeclarationError),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Violation {
    Unavailable,
    Lifetime,
    Initialization,
    Permission,
    LoanConflict,
    LoanRegion,
    CallRegion,
    ActiveAtExit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OwnedFailureKind {
    Malformed(Malformed),
    Ownership(Violation),
    Resource(&'static str),
}
/// Compact optional origin. The sentinel cannot be a valid SourceMap identity;
/// no failure path needs a heap allocation merely to retain diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Origin(Span);
impl Origin {
    const NONE: Self = Self(Span {
        file: super::super::source::SourceFileId(usize::MAX),
        start: 0,
        end: 0,
    });
    fn from(value: Option<Span>) -> Self {
        value.map_or(Self::NONE, Self)
    }
    fn get(self) -> Option<Span> {
        (self.0.file.0 != usize::MAX).then_some(self.0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OwnedFailure {
    kind: OwnedFailureKind,
    primary: Origin,
    related: Origin,
    declaration: Origin,
    context: Option<flow::DenialContext>,
}
impl OwnedFailure {
    fn malformed(kind: Malformed, span: Span) -> Self {
        Self {
            kind: OwnedFailureKind::Malformed(kind),
            primary: Origin(span),
            related: Origin::NONE,
            declaration: Origin::NONE,
            context: None,
        }
    }
    fn violation(kind: Violation, span: Span, declaration: Option<Span>) -> Self {
        Self {
            kind: OwnedFailureKind::Ownership(kind),
            primary: Origin(span),
            related: Origin::NONE,
            declaration: Origin::from(declaration),
            context: None,
        }
    }
    fn resource(name: &'static str) -> Self {
        Self {
            kind: OwnedFailureKind::Resource(name),
            primary: Origin::NONE,
            related: Origin::NONE,
            declaration: Origin::NONE,
            context: None,
        }
    }
}
impl From<OirFailure> for OwnedFailure {
    fn from(value: OirFailure) -> Self {
        Self {
            kind: match value.kind {
                FailureKind::ResourceLimit(name) => OwnedFailureKind::Resource(name),
                other => OwnedFailureKind::Malformed(Malformed::Scalar(other)),
            },
            primary: Origin::from(value.span),
            related: Origin::NONE,
            declaration: Origin::NONE,
            context: None,
        }
    }
}
impl From<DeclarationError> for OwnedFailure {
    fn from(value: DeclarationError) -> Self {
        Self {
            kind: match value {
                DeclarationError::ResourceLimit(name) => OwnedFailureKind::Resource(name),
                DeclarationError::Allocation => OwnedFailureKind::Resource("allocation"),
                DeclarationError::LayoutOverflow => OwnedFailureKind::Resource("layout overflow"),
                other => OwnedFailureKind::Malformed(Malformed::Declaration(other)),
            },
            primary: Origin::NONE,
            related: Origin::NONE,
            declaration: Origin::NONE,
            context: None,
        }
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct OwnershipUsage {
    owners: usize,
    expanded_events: usize,
    work: usize,
    scratch_bytes: usize,
    metadata_bytes: usize,
    owner_cells: usize,
    owner_layout_bytes: usize,
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod oracle_tests;

#[cfg(test)]
mod consumer_fixtures;
#[cfg(test)]
mod consumer_pilot;
#[cfg(test)]
mod consumer_tests;

#[cfg(test)]
mod denial_tests;
#[cfg(test)]
mod origin_tests;

#[cfg(test)]
mod reviewer_allocator;
#[cfg(test)]
mod reviewer_origins;

#[cfg(test)]
mod array_reference_tests;

#[cfg(test)]
mod reviewer_array_reference_tests;

#[cfg(test)]
mod composition_reference_tests;
#[cfg(test)]
mod composition_verifier_tests;

#[cfg(test)]
mod negation_raw_tests;

#[cfg(test)]
mod enum_layout_tests;
