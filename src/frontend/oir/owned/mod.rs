//! Authoritative private ownership OIR and bounded verified consumers.
//! Production record source lowering and both consumers require the sealed witness.
//! Finite enum payloads and built-in resources use the same sealed witness.
#![allow(dead_code)]
// Denials retain exact verifier-derived facts on the stack. Boxing this fixed
// transport would add an allocation on ownership/resource failure paths.
#![allow(clippy::result_large_err)]
use super::{owned_types::*, *};
mod binary_io;
mod budget;
mod builtins;
// Built-in buffer operations require the checked canonical resource catalog.
mod byte_buffer;
mod cfg;
mod execute;
mod experimental_descriptors;
mod flow;
mod input;
mod native;
mod output;
mod plan;
pub(in crate::frontend) mod process;
#[cfg(test)]
mod resource_gate_tests;
mod shape;
pub(super) mod source;
mod vector_descriptors;
mod vector_type;
pub(super) use source::SourceProgram;
mod storage;
mod verified;
#[cfg(test)]
use verified::{verify_owned, verify_with_limits};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReferenceParamId(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MatchId(usize);
#[derive(Clone, Debug)]
struct MatchDecl {
    source: OwnerPlaceId,
    arms: Vec<MatchArm>,
    span: Span,
}
#[derive(Clone, Copy, Debug)]
struct MatchArm {
    variant: VariantId,
    dispatch: BlockId,
    entry: BlockId,
}
use crate::frontend::builtin_catalog::BuiltinSet as BuiltinOrigins;
impl BuiltinOrigins {
    fn require_none(self) -> Result<(), OwnedFailure> {
        if self == Self::None {
            return Ok(());
        }
        Err(OwnedFailure {
            kind: OwnedFailureKind::Malformed(Malformed::Binding),
            primary: Origin::NONE,
            related: Origin::NONE,
            declaration: Origin::NONE,
            context: None,
        })
    }
}
#[derive(Debug)]
struct RawOwnedProgram {
    builtins: BuiltinOrigins,
    vector_functions: Vec<crate::frontend::builtin_catalog::vector::Function>,
    enums: Vec<RawEnumDecl>,
    records: Vec<RawRecordDecl>,
    functions: Vec<RawOwnedFunction>,
}
impl RawOwnedProgram {
    /// Untrusted ordinals only. The descriptor and ordinary proofs must still
    /// succeed before these offsets can be exposed by a sealed witness.
    fn fixed_builtin_base(&self) -> Option<usize> {
        self.functions
            .len()
            .checked_sub(self.vector_functions.len())?
            .checked_sub(self.builtins.extra_functions())
    }
    fn vector_operation_claim(
        &self,
        id: hir::DefId,
    ) -> Option<crate::frontend::builtin_catalog::vector::Function> {
        let base = self
            .functions
            .len()
            .checked_sub(self.vector_functions.len())?;
        self.vector_functions.get(id.0.checked_sub(base)?).copied()
    }
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
    matches: Vec<MatchDecl>,
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
    Loan(LoanId),
}
const MAX_LOCAL_REFERENCE_DEPTH: usize = 64;
const _: () = assert!(MAX_LOCAL_REFERENCE_DEPTH <= u8::MAX as usize);
#[derive(Clone, Debug)]
struct LoanDecl {
    region: LoanRegion,
    authority: AccessBase,
    projection: Vec<FieldId>,
    kind: BorrowKind,
    referent: BorrowedSlot,
    span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoanRegion {
    Call { call: CallSiteId, argument: usize },
    Local,
}
impl LoanDecl {
    fn call_binding(&self) -> Option<(CallSiteId, usize)> {
        match self.region {
            LoanRegion::Call { call, argument } => Some((call, argument)),
            LoanRegion::Local => None,
        }
    }
    #[cfg(test)]
    fn call_binding_mut(&mut self) -> Option<(&mut CallSiteId, &mut usize)> {
        match &mut self.region {
            LoanRegion::Call { call, argument } => Some((call, argument)),
            LoanRegion::Local => None,
        }
    }
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
    /// Consume a complete record and initialize every owned field binding.
    /// Scalar fields use ordinary checked reads before this atomic transfer.
    DestructureRecord {
        source: OwnerPlaceId,
        fields: Vec<(FieldId, OwnerPlaceId)>,
    },
    /// Operands are the independently checked canonical parameter rows. No
    /// spelling, producer layout, or arbitrary owner ID grants this authority.
    Experimental {
        operation: crate::frontend::builtin_catalog::experimental::Function,
        destination: CallResult,
    },
    /// Confined to the independently checked concrete builtin suffix.
    Vector {
        operation: crate::frontend::builtin_catalog::vector::Function,
        destination: CallResult,
    },
    // Atomic input is confined to the verified canonical builtin body.
    ReadStdin {
        buffer: ReferenceParamId,
        destination: OwnerPlaceId,
    },
    // Output remains confined to the verified canonical builtin body.
    WriteStdout {
        buffer: ReferenceParamId,
        destination: OwnerPlaceId,
    },
    ConstructEnum {
        destination: OwnerPlaceId,
        variant: VariantId,
        payload: Option<Operand>,
    },
    ConstructOwnedEnum {
        destination: OwnerPlaceId,
        variant: VariantId,
        payload: OwnerPlaceId,
    },
    ConsumeVariant {
        match_id: MatchId,
        arm: usize,
        destination: Option<LocalId>,
    },
    // First instruction of a guarded arm. Atomically starts and initializes a
    // distinct immutable binding; no separate StorageLive is permitted.
    ConsumeOwnedVariant {
        match_id: MatchId,
        arm: usize,
        destination: OwnerPlaceId,
    },
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
    /// A finite checked record copied from a vector element into a new owner.
    CopyIndexInitialize {
        destination: OwnerPlaceId,
        base: AccessBase,
        path: Vec<FieldId>,
        index: Operand,
    },
    /// Whole-record replacement through exclusive vector or record-slice authority.
    CopyIndexWrite {
        base: AccessBase,
        path: Vec<FieldId>,
        index: Operand,
        source: OwnerPlaceId,
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
    PrepareBorrowRange {
        call: CallSiteId,
        argument: usize,
        loan: LoanId,
        start: Operand,
        end: Operand,
    },
    PrepareBorrow {
        call: CallSiteId,
        argument: usize,
        loan: LoanId,
    },
    AcquireLocalBorrow(LoanId),
    ReleaseLocalBorrow(LoanId),
    /// Checked finite record copy. Source stays live; validate all leaves
    /// before writing any initialized destination leaf. Padding is never read.
    CopyInitialize {
        destination: OwnerPlaceId,
        source: OwnerPlaceId,
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
    /// Even the final arm tests its expected checked tag; it is never a Goto.
    MatchDispatch {
        match_id: MatchId,
        arm: usize,
    },
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
mod builtin_descriptor_tests;
#[cfg(test)]
mod builtin_input_fixtures;
#[cfg(test)]
mod builtin_input_tests;
#[cfg(test)]
mod builtin_origin_tests;
#[cfg(test)]
mod builtin_output_descriptor_tests;
#[cfg(test)]
mod builtin_output_fixtures;
#[cfg(test)]
mod builtin_output_process_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod vector_raw_tests;

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
#[cfg(test)]
mod owned_enum_tests;

#[cfg(test)]
mod enum_admission_tests;

#[cfg(test)]
mod enum_match_tests;

#[cfg(test)]
mod enum_consumer_fixtures;
#[cfg(test)]
mod enum_reference_tests;

#[cfg(test)]
mod enum_parser_allocation_tests;

#[cfg(test)]
mod enum_formatter_allocation_tests;
#[cfg(test)]
mod enum_index_allocation_tests;
#[cfg(test)]
mod enum_query_allocation_tests;

// Narrow test-only forwarding for contained scalar HIR cleanup controls. The
// action cannot return an owner; only primitive observer facts cross this seam.
#[cfg(test)]
pub(in crate::frontend::oir) fn hir_import_measure_allocations(
    action: impl FnOnce(),
) -> (usize, usize, isize, isize) {
    assert!(
        hir_import_allocation_observers_idle(),
        "nested HIR allocation observer"
    );
    let (((), (calls, live, peak)), attempts) = reviewer_origins::integration_counted(|| {
        source::reviewer_source::integration_measured(action)
    });
    (attempts, calls, live, peak)
}
#[cfg(test)]
pub(in crate::frontend::oir) fn hir_import_allocation_observers_idle() -> bool {
    !reviewer_origins::integration_enabled() && !source::reviewer_source::integration_enabled()
}

#[cfg(test)]
mod u8_tests;

#[cfg(test)]
mod byte_storage_reference_tests;

#[cfg(test)]
#[path = "record_vector_shape_tests.rs"]
mod record_vector_shape_tests;
