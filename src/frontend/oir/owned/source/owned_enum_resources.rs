//! Owned enum payload successor to the unchanged scalar source role banks.
//!
//! The complete newly reachable roles below are charged in addition to the
//! historical scalar branches, never substituted as equal-size spare slots.
//! No retained carrier, source authority or global limit changes. This is the
//! same conservative named-value envelope as the predecessor, not an RSS claim.
use super::{hir::*, lower::EvaluatedValue};
use crate::frontend::{
    ast,
    declaration_index::{
        EnumView, NominalExposure, NominalId, QuerySession, VariantView, WorkMeter,
    },
    diagnostic::Diagnostic,
    oir::owned::{OwnedInstruction, OwnerPlaceId},
    oir::owned_types::EnumId,
    parser::{MAX_BLOCK_NESTING, MAX_NESTING},
    project::ModuleId,
    source::Span,
};
use std::mem::size_of;

#[allow(dead_code)]
struct PayloadQueryRoles {
    receiver: &'static VariantView<'static>,
    work: &'static WorkMeter,
    syntax: Option<&'static ast::EnumVariantSyntax>,
    optional_payload: Option<ast::EnumPayloadSyntax>,
    payload: ast::EnumPayloadSyntax,
    type_receiver: ast::EnumPayloadSyntax,
    constructed_type: ast::TypeSyntax,
    returned_type: ast::TypeSyntax,
    type_argument: ast::TypeSyntax,
    module_return: Result<ModuleId, Box<Diagnostic>>,
    module: ModuleId,
    query: QuerySession<'static, 'static>,
    query_return: Result<ValueTy, Box<Diagnostic>>,
    returned: Result<Option<ValueTy>, Box<Diagnostic>>,
    caller_return: Result<Option<ValueTy>, Box<Diagnostic>>,
}

#[allow(dead_code)]
struct OwnedExpressionRoles {
    query: PayloadQueryRoles,
    constructor_shape: (Option<ExprId>, Option<ValueTy>),
    constructor_payload: ExprId,
    expected: ValueTy,
    actual: ValueTy,
    child_return: Result<ValueTy, Box<Diagnostic>>,
    arguments: Option<&'static [ast::Argument]>,
    value_argument: &'static ast::ExprId,
    resolved_child: Result<ExprId, Box<Diagnostic>>,
    resolved_construction: super::hir::ExprKind,
}

#[allow(dead_code)]
struct OwnedMatchRoles {
    query: PayloadQueryRoles,
    shape: (Option<ValueTy>, Option<BindingId>),
    payload: ValueTy,
    binding: BindingId,
    declaration: &'static Binding,
    annotation: Option<ValueTy>,
    initialized_payload: AggregateTy,
    initialized_slot: &'static mut Option<ParameterTy>,
    bound_annotation: Option<ValueTy>,
    bound_result: Result<BindingId, Box<Diagnostic>>,
    local_type: ParameterTy,
    local_aggregate: AggregateTy,
    local_owner: OwnerPlaceId,
}

#[allow(dead_code)]
struct OwnedDeclarationRoles {
    inventory_visits: u64,
    inventory_result: Result<bool, Box<Diagnostic>>,
    inventory_cursor: std::slice::Iter<'static, ast::EnumVariantSyntax>,
    inventory_payload: Option<ast::EnumPayloadSyntax>,
    query: PayloadQueryRoles,
    enumeration: EnumView<'static>,
    enumeration_result: Result<EnumView<'static>, Box<Diagnostic>>,
    variant: VariantView<'static>,
    variant_result: Result<VariantView<'static>, Box<Diagnostic>>,
    payload: Option<ValueTy>,
    nominal: NominalId,
    exposure: Result<NominalExposure, Box<Diagnostic>>,
    origin: Span,
    declaration_origin: Span,
    restrictor: Option<Span>,
    enumeration_ordinals: std::ops::Range<usize>,
    variant_ordinals: std::ops::Range<usize>,
    stack: [Option<(EnumId, usize)>; MAX_NESTING],
    cursor: (EnumId, usize),
    nested: EnumId,
    ancestor: Option<(EnumId, usize)>,
    depth: usize,
    root: usize,
    checked_depth: Option<usize>,
}

#[allow(dead_code)]
struct OwnedLowerRoles {
    expression: Option<ExprId>,
    child: ExprId,
    evaluated: Result<EvaluatedValue, super::super::OwnedFailure>,
    payload: OwnerPlaceId,
    destination: OwnerPlaceId,
    local_type: ParameterTy,
    local_aggregate: AggregateTy,
    local_owner: OwnerPlaceId,
    instruction: OwnedInstruction,
    statement_argument: OwnedInstruction,
    query: PayloadQueryRoles,
    declaration_payload: Option<ValueTy>,
    raw_payload: Option<ParameterTy>,
}

pub(super) const fn fixed_bytes() -> usize {
    // Resolver/typing follow the existing source-child depth bound. Match
    // lifetimes follow the existing block bound. Count and emit lower passes
    // are sequential, but pay both complete role sets conservatively.
    MAX_NESTING * size_of::<OwnedExpressionRoles>()
        + MAX_BLOCK_NESTING * size_of::<OwnedMatchRoles>()
        + size_of::<OwnedDeclarationRoles>()
        + 2 * size_of::<OwnedLowerRoles>()
}
