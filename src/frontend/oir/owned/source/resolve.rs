//! Complete declaration/name resolution. No ownership or loan analysis.
use super::hir::*;
use super::resolver_storage::{self as storage, Kind, PaidStorage};
use crate::frontend::{
    ast,
    builtin_catalog::{BuiltinEnum, BuiltinFunction, BuiltinSet},
    declaration_index::{
        self as index, Access, DeclarationIndex, Exposure, IndexLimits, NominalExposure, NominalId,
        PreparedTypeName, QualifiedValueEndpoint, QuerySession, SourceOwner, TypeContext,
        WorkMeter,
    },
    diagnostic::Diagnostic,
    owned_diagnostic::{self, diagnostic, secondary},
    parser::MAX_DIAGNOSTICS,
    project::{
        budget::{Allocator, ReserveFailure},
        ItemPathRef, ModuleId, QualifiedPathRef,
    },
    source::{SourceFile, SourceMap, SourceView, Span},
};
use std::collections::HashMap;

/// Construction policy, retained even for array-free inputs. No executable
/// entrypoint accepts a caller-selected admission policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SourceAdmission {
    Executable,
    #[cfg(test)]
    ObserveArrayTypes,
    #[cfg(test)]
    ObserveEnumTypes,
    #[cfg(test)]
    EnumPipeline,
    #[cfg(test)]
    BuiltinPipeline,
    #[cfg(test)]
    ObserveArrayPipeline,
    #[cfg(test)]
    ArrayConsumer,
}
impl SourceAdmission {
    pub(super) fn executable(self) -> bool {
        self == Self::Executable
    }
    /// Exact source/admission relation for fresh paid typing. Inventory and
    /// paid storage alone never establish source provenance. This predicate
    /// constructs no owner and is repeated at completion and raw emission.
    pub(super) fn allows_paid_source(self, index: &DeclarationIndex<'_>) -> bool {
        if index.is_current_source_pipeline() {
            return self == Self::Executable;
        }
        #[cfg(test)]
        {
            self == Self::BuiltinPipeline
                && (index.is_output_candidate_pipeline()
                    || (index.is_input_candidate_pipeline() && !index.builtin_set().has_output()))
        }
        #[cfg(not(test))]
        {
            false
        }
    }
    pub(super) fn allows_lowering(self) -> bool {
        match self {
            Self::Executable => true,
            #[cfg(test)]
            Self::ObserveArrayPipeline
            | Self::ArrayConsumer
            | Self::EnumPipeline
            | Self::BuiltinPipeline => true,
            #[cfg(test)]
            Self::ObserveArrayTypes | Self::ObserveEnumTypes => false,
        }
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // Keep the checked legacy carrier allocation-free.
enum IndexOwner<'src> {
    Owned(DeclarationIndex<'src>),
    Borrowed(&'src DeclarationIndex<'src>),
}

#[derive(Debug)]
enum MeterOwner<'src> {
    Owned(WorkMeter),
    Borrowed(&'src WorkMeter),
}
#[derive(Debug)]
pub(in crate::frontend::oir) struct ResolvedOwnedProgram<'src> {
    projection_bytes: std::cell::Cell<usize>,
    admission: SourceAdmission,
    index: IndexOwner<'src>,
    work: MeterOwner<'src>,
    sources: SourceView<'src>,
    records: Vec<Record>,
    signatures: Vec<Signature>,
    functions: Vec<Function>,
    entry: Option<DefId>,
}
impl<'src> ResolvedOwnedProgram<'src> {
    pub(super) fn conversion_capacity_excess(
        &self,
        inventory: &mut super::hir_budget::CapacityExcess<'_>,
    ) -> Result<(), Box<Diagnostic>> {
        inventory.vector(&self.records)?;
        inventory.vector(&self.signatures)?;
        inventory.vector(&self.functions)?;
        for record in &self.records {
            inventory.row()?;
            inventory.vector(&record.fields)?;
        }
        for signature in &self.signatures {
            inventory.row()?;
            inventory.vector(&signature.params)?;
        }
        for function in &self.functions {
            inventory.row()?;
            inventory.vector(&function.bindings)?;
            inventory.vector(&function.expressions)?;
            inventory.vector(&function.blocks)?;
            for expression in &function.expressions {
                inventory.row()?;
                match &expression.kind {
                    ExprKind::Call { args, .. } => inventory.vector(args)?,
                    ExprKind::StructLiteral { fields, .. } => inventory.vector(fields)?,
                    ExprKind::ArrayLiteral { elements } => inventory.vector(elements)?,
                    _ => (),
                }
            }
            for block in &function.blocks {
                inventory.row()?;
                inventory.vector(&block.body)?;
                for statement in &block.body {
                    inventory.row()?;
                    if let StmtKind::Match { arms, .. } = &statement.kind {
                        inventory.vector(arms)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Retained typed path payload is cumulative across functions. Admit the
    /// complete exact reservation before allocating; raw paths are independently
    /// inventoried again by lowering and by the verifier.
    pub(super) fn admit_projection(
        &self,
        length: usize,
        span: Span,
    ) -> Result<(), Box<Diagnostic>> {
        if length == 0 || length > 64 {
            return Err(error(
                "E0400",
                format_args!("record access path depth limit exceeded"),
                span,
            ));
        }
        let bytes = length
            .checked_mul(std::mem::size_of::<FieldId>())
            .ok_or_else(|| {
                error(
                    "E0400",
                    format_args!("typed record projection payload limit exceeded"),
                    span,
                )
            })?;
        self.admit_projection_metadata(bytes, span)?;
        // Existing one-hop fields retain their permission-query work charge.
        if length > 1 {
            self.work()
                .debit(length as u64, span, "record projection path")?;
        }
        Ok(())
    }
    /// Sparse borrow-path lookup entries and path fields share a cumulative byte
    /// cap. Admit each complete requested reservation before any growth.
    pub(super) fn admit_projection_metadata(
        &self,
        bytes: usize,
        span: Span,
    ) -> Result<(), Box<Diagnostic>> {
        let total = self
            .projection_bytes
            .get()
            .checked_add(bytes)
            .filter(|bytes| *bytes <= super::budget::MAX_RAW_BYTES)
            .ok_or_else(|| {
                error(
                    "E0400",
                    format_args!("typed record projection payload limit exceeded"),
                    span,
                )
            })?;
        self.projection_bytes.set(total);
        Ok(())
    }
    pub(super) fn admission(&self) -> SourceAdmission {
        self.admission
    }
    /// Shared budget access for closed fresh typing only. This is not a seed
    /// setter or an admission/source-association proof.
    pub(super) fn type_storage_cell(&self) -> &std::cell::Cell<usize> {
        &self.projection_bytes
    }
    pub(super) fn index(&self) -> &DeclarationIndex<'src> {
        match &self.index {
            IndexOwner::Owned(index) => index,
            IndexOwner::Borrowed(index) => index,
        }
    }
    pub(super) fn work(&self) -> &WorkMeter {
        match &self.work {
            MeterOwner::Owned(work) => work,
            MeterOwner::Borrowed(work) => work,
        }
    }
    pub(super) fn query(&self) -> QuerySession<'_, 'src> {
        self.index().query(self.work())
    }
    pub(super) fn requester(&self, function: DefId) -> Result<ModuleId, Box<Diagnostic>> {
        self.index().function(function).map(|(_, module)| module)
    }
    pub(super) fn prepare_name(
        &self,
        record: RecordId,
        at: Span,
    ) -> Result<PreparedTypeName<'src>, Box<Diagnostic>> {
        self.query().prepare_type_name(record, at)
    }
    pub(super) fn text(&self, span: Span) -> &str {
        self.sources.text(span)
    }
    pub(super) fn records(&self) -> &[Record] {
        &self.records
    }
    pub(super) fn signatures(&self) -> &[Signature] {
        &self.signatures
    }
    pub(super) fn functions(&self) -> &[Function] {
        &self.functions
    }
    pub(super) fn entry(&self) -> Option<DefId> {
        self.entry
    }
    /// Check the source-body prefix and finite catalog signature suffix against
    /// the frozen index. Called before builtin paid typing and source lowering;
    /// a signature vector's length or diagnostic spelling is never authority.
    pub(super) fn validate_function_signatures(&self) -> Result<(), Box<Diagnostic>> {
        let index = self.index();
        let at = index.sources().eof();
        self.work().debit(1, at, "source signature identity")?;
        if self.functions.len() != index.source_function_count()
            || self.signatures.len() != index.function_count()
        {
            return Err(invalid_signature_identity(at));
        }
        for (ordinal, function) in self.functions.iter().enumerate() {
            self.work()
                .debit(1, function.end, "source signature identity")?;
            if function.id != DefId(ordinal) {
                return Err(invalid_signature_identity(function.end));
            }
        }
        for builtin in BuiltinFunction::ALL {
            if !index.builtin_set().contains_function(builtin) {
                continue;
            }
            let id = index.builtin_function_id(builtin)?;
            let enumeration = index.builtin_enum_id(match builtin {
                BuiltinFunction::ReadStdin => BuiltinEnum::ReadStatus,
                BuiltinFunction::WriteStdout => BuiltinEnum::WriteStatus,
            })?;
            let anchor = index.builtin_function_anchor(builtin)?;
            let signature = self
                .signatures
                .get(id.0)
                .ok_or_else(|| invalid_signature_identity(at))?;
            let (parameter, result) = builtin.signature(enumeration);
            let expected = self.functions.len()
                + usize::from(
                    builtin == BuiltinFunction::WriteStdout
                        && index
                            .builtin_set()
                            .contains_function(BuiltinFunction::ReadStdin),
                );
            self.work().debit(5, anchor, "builtin signature identity")?;
            if id.0 != expected
                || signature.params.as_slice() != [parameter]
                || signature.result != result
                || signature.span != anchor
            {
                return Err(invalid_signature_identity(anchor));
            }
        }
        Ok(())
    }
}
fn invalid_signature_identity(at: Span) -> Box<Diagnostic> {
    error(
        "E0500",
        format_args!("invalid source signature identity"),
        at,
    )
}
fn text(source: &SourceFile, span: Span) -> &str {
    source.text_at(span)
}
fn error(code: &'static str, message: std::fmt::Arguments<'_>, span: Span) -> Box<Diagnostic> {
    diagnostic(code, "resolve", message, Some(span))
}
fn duplicate(span: Span, original: Span) -> Box<Diagnostic> {
    secondary(
        error(
            "E0201",
            format_args!("duplicate binding; shadowing is unavailable in typed-preview"),
            span,
        ),
        original,
        format_args!("first declared here"),
    )
}
fn value_type(
    query: &mut QuerySession<'_, '_>,
    requester: ModuleId,
    ty: ast::TypeSyntax,
) -> Result<ValueTy, Box<Diagnostic>> {
    query.value_type(requester, ty, TypeContext::Value)
}
// Generic value-type resolution must never silently authorize enum containment.
fn record_field_type(ty: ValueTy, span: Span) -> Result<ValueTy, Box<Diagnostic>> {
    if ty == ValueTy::Scalar(Ty::U8) {
        Err(error(
            "E0202",
            format_args!("u8 record fields are not supported"),
            span,
        ))
    } else if matches!(ty, ValueTy::Owned(AggregateTy::Enum(_))) {
        Err(error(
            "E0300",
            format_args!("enum values cannot be record fields"),
            span,
        ))
    } else {
        Ok(ty)
    }
}
#[test]
fn bounded_enum_source_record_field_fence_is_independent_of_resolution() {
    let mut sources = SourceMap::new();
    let id = sources.add("enum-field-gate.ox".into(), "field".into());
    let span = sources.get(id).span(0, 5);
    for id in [0, usize::MAX] {
        let error = record_field_type(
            ValueTy::Owned(AggregateTy::Enum(
                crate::frontend::oir::owned_types::EnumId(id),
            )),
            span,
        )
        .unwrap_err();
        assert_eq!(error.code, "E0300");
        assert_eq!(error.primary, Some(span));
    }
}

fn parameter_type(
    query: &mut QuerySession<'_, '_>,
    requester: ModuleId,
    ty: ast::TypeSyntax,
) -> Result<ParameterTy, Box<Diagnostic>> {
    query.parameter_type(requester, ty)
}
fn decimal_i32(digits: &str, negative: bool, span: Span) -> Result<i32, Box<Diagnostic>> {
    let mut value = 0i32;
    for byte in digits.bytes() {
        let digit = i32::from(byte - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| {
                if negative {
                    value.checked_sub(digit)
                } else {
                    value.checked_add(digit)
                }
            })
            .ok_or_else(|| {
                error(
                    "E0203",
                    format_args!(
                        "decimal literal is outside the i32 range [-2147483648, 2147483647]"
                    ),
                    span,
                )
            })?;
    }
    Ok(value)
}

#[cfg(test)]
pub(super) fn resolve<'src>(
    source: &'src SourceFile,
    ast: &'src ast::Program,
) -> Result<ResolvedOwnedProgram<'src>, Vec<Diagnostic>> {
    resolve_with_view(source, ast, SourceView::Single(source))
}
pub(super) fn resolve_in_map<'src>(
    source: &'src SourceFile,
    ast: &'src ast::Program,
    sources: &'src SourceMap,
) -> Result<ResolvedOwnedProgram<'src>, Vec<Diagnostic>> {
    resolve_with_view(source, ast, SourceView::Map(sources))
}
fn resolve_with_view<'src>(
    source: &'src SourceFile,
    ast: &'src ast::Program,
    view: SourceView<'src>,
) -> Result<ResolvedOwnedProgram<'src>, Vec<Diagnostic>> {
    let sources = SourceOwner::original(source, ast, view).map_err(|e| vec![*e])?;
    resolve_sources(sources)
}
pub(in crate::frontend) fn resolve_sources(
    sources: SourceOwner<'_>,
) -> Result<ResolvedOwnedProgram<'_>, Vec<Diagnostic>> {
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let (index, (records, signatures, functions)) =
        resolve_source_parts(sources, &work, &mut allocator, IndexLimits::default())?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::Executable,
        sources: sources.view(),
        index: IndexOwner::Owned(index),
        work: MeterOwner::Owned(work),
        records,
        signatures,
        functions,
        entry,
    })
}
#[cfg(test)]
pub(in crate::frontend::oir) fn resolve_observed<'s>(
    source: &'s SourceFile,
    ast: &'s ast::Program,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let sources =
        SourceOwner::original(source, ast, SourceView::Single(source)).map_err(|e| vec![*e])?;
    let (index, (records, signatures, functions)) =
        resolve_source_parts(sources, work, allocator, IndexLimits::default())?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::Executable,
        sources: sources.view(),
        index: IndexOwner::Owned(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry,
    })
}
fn resolve_source_parts<'s>(
    sources: SourceOwner<'s>,
    work: &WorkMeter,
    allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<(DeclarationIndex<'s>, ResolvedParts), Vec<Diagnostic>> {
    let facts = index::collect_originals(sources, limits, work, allocator).map_err(|e| vec![*e])?;
    let index = facts.finish(work, allocator)?;
    let parts = resolve_index(&index, work, allocator)?;
    Ok((index, parts))
}
pub(in crate::frontend) fn resolve_project<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let mut allocator = Allocator::default();
    let (records, signatures, functions) = resolve_index(index, work, &mut allocator)?;
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::Executable,
        sources: index.sources().view(),
        index: IndexOwner::Borrowed(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry: index.root_original_main(),
    })
}
type ResolvedParts = (Vec<Record>, Vec<Signature>, Vec<Function>);
fn resolve_index(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<ResolvedParts, Vec<Diagnostic>> {
    index
        .require_current_source_pipeline()
        .map_err(|e| vec![*e])?;
    if index.enum_count() != 0 {
        return Err(vec![*error(
            "E0101",
            format_args!("enum source requires fresh paid typing"),
            index.sources().eof(),
        )]);
    }
    resolve_index_impl(index, work, allocator, None)
}
/// Sole production enum-bearing typing construction, including builtin enums.
/// Source provenance is checked before storage; callers supply no owner, seed,
/// plan or admission.
/// Public facades select this only for an enum-bearing frozen index.
#[allow(dead_code)]
pub(in crate::frontend::oir) fn type_enum_source<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<super::typeck::TypedOwnedProgram<'s>, Vec<Diagnostic>> {
    index
        .require_current_source_pipeline()
        .map_err(|error| vec![*error])?;
    let at = index.sources().eof();
    if index.enum_count() == 0 {
        return Err(vec![*error(
            "E0500",
            format_args!("fresh enum typing requires enum declarations"),
            at,
        )]);
    }
    type_paid_source(index, work, allocator, SourceAdmission::Executable)
}

/// Sole caller is program.rs's fresh private candidate continuation. The
/// borrowed typed owner remains inside that continuation and never grants
/// SourceProgram/executable authority, including a candidate without imports.
#[cfg(test)]
pub(super) fn type_builtin_source<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<super::typeck::TypedOwnedProgram<'s>, Vec<Diagnostic>> {
    index
        .require_builtin_candidate_pipeline()
        .map_err(|error| vec![*error])?;
    type_paid_source(index, work, allocator, SourceAdmission::BuiltinPipeline)
}

// The condition keeps production and private-test admission branches explicit.
#[allow(clippy::blocks_in_conditions)]
fn type_paid_source<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
    admission: SourceAdmission,
) -> Result<super::typeck::TypedOwnedProgram<'s>, Vec<Diagnostic>> {
    if !admission.allows_paid_source(index) {
        return Err(vec![*invalid_signature_identity(index.sources().eof())]);
    }
    let at = index.sources().eof();
    let attempts_before = allocator.attempts;
    let plan = match admission {
        #[cfg(test)]
        SourceAdmission::BuiltinPipeline => super::hir_budget::preflight_builtin_hir(index, work),
        _ => super::hir_budget::preflight_current_hir(index, work),
    }
    .map_err(|error| vec![*error])?
    .ok_or_else(|| {
        vec![*error(
            "E0500",
            format_args!("missing enum HIR preflight"),
            at,
        )]
    })?;
    let parts;
    let resolver_end;
    {
        let mut paid = PaidStorage::new(plan.counts);
        parts = resolve_index_impl(index, work, allocator, Some(&mut paid))?;
        let inventory = storage::inventory_parts(&parts, work, at).map_err(|error| vec![*error])?;
        resolver_end = allocator.attempts;
        let delta = resolver_end.checked_sub(attempts_before).ok_or_else(|| {
            vec![*error(
                "E0400",
                format_args!("resolver allocation attempt counter regressed"),
                at,
            )]
        })?;
        paid.reconcile(&plan, inventory, delta, work, at)
            .map_err(|error| vec![*error])?;
    }
    let (records, signatures, functions) = parts;
    let program = ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(plan.total),
        admission,
        sources: index.sources().view(),
        index: IndexOwner::Borrowed(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry: index.root_original_main(),
    };
    // The scalar checkpoint detects any request inserted between reconciled
    // resolution and paid typing; it is not a caller-chosen seed or capability.
    match admission {
        #[cfg(test)]
        SourceAdmission::BuiltinPipeline => {
            super::typeck::finish_builtin_source(program, &plan, allocator, resolver_end)
        }
        _ => super::typeck::finish_enum_source(program, &plan, allocator, resolver_end),
    }
}

/// Test-only fixed statistics, never a source/typing/ownership witness. All
/// private parts and construction-local storage end before the result returns.
#[cfg(test)]
pub(super) fn probe_enum_resolver_storage(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<Option<storage::ResolverStorageObservation>, Vec<Diagnostic>> {
    index
        .require_no_builtin_candidate()
        .map_err(|error| vec![*error])?;
    // The selector must precede every new query, phase, work debit or reserve.
    if index.enum_count() == 0 {
        return Ok(None);
    }
    let attempts_before = allocator.attempts;
    // Existing allocation-free index query state remains a separate ledger.
    // Retain only the Span, never a new SourceOwner/owner wrapper.
    let at = index.sources().eof();
    // Tail matches keep each success-pattern payload as the sole caller value;
    // there is no second outer plan, inventory or delta transfer slot.
    match super::hir_budget::preflight_enum_hir(index, work) {
        Ok(Some(plan)) => {
            let observation = {
                let mut paid = PaidStorage::new(plan.counts);
                let parts = resolve_index_impl(index, work, allocator, Some(&mut paid))?;
                match storage::inventory_parts(&parts, work, at) {
                    Ok(inventory) => {
                        let attempts_after = allocator.attempts;
                        let delta_option = attempts_after.checked_sub(attempts_before);
                        match delta_option {
                            Some(delta) => {
                                match paid.reconcile(&plan, inventory, delta, work, at) {
                                    Ok(observation) => {
                                        drop(parts);
                                        // The scalar-only account leaves this enclosing
                                        // scope too; only this pattern and outer fixed
                                        // observation slots are independently held.
                                        observation
                                    }
                                    Err(error) => return Err(vec![*error]),
                                }
                            }
                            None => {
                                return Err(vec![*error(
                                    "E0400",
                                    format_args!("resolver allocation attempt counter regressed"),
                                    at,
                                )])
                            }
                        }
                    }
                    Err(error) => return Err(vec![*error]),
                }
            };
            Ok(Some(observation))
        }
        Ok(None) => Err(vec![*error(
            "E0500",
            format_args!("missing enum HIR preflight for resolver observation"),
            at,
        )]),
        Err(error) => Err(vec![*error]),
    }
}

/// Private fixed statistics through the one reviewed fresh construction path.
/// Enum-free selection stays inert. No source/type/ownership witness escapes,
/// and executable enum admission remains closed.
#[cfg(test)]
pub(super) fn probe_enum_type_storage<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>> {
    index
        .require_no_builtin_candidate()
        .map_err(|error| vec![*error])?;
    if index.enum_count() == 0 {
        return Ok(None);
    }
    fresh_enum_type_storage(index, work, allocator)
}

/// Fixed facts only. The one source plan stays inside the resolver member.
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) struct EnumTypeStorageObservation {
    pub(super) resolver: storage::ResolverStorageObservation,
    pub(super) typed: super::type_storage::TypeStorageObservation,
}

/// Only the private fixed-statistics selector above calls this construction.
/// Its three inputs cannot substitute an owner, seed, context or callback.
#[cfg(test)]
#[allow(dead_code)]
fn fresh_enum_type_storage<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>> {
    if index.enum_count() == 0 {
        return Ok(None);
    }
    let attempts_before = allocator.attempts;
    let at = index.sources().eof();
    match super::hir_budget::preflight_enum_hir(index, work) {
        Ok(Some(plan)) => {
            let parts;
            let resolver_observation;
            {
                let mut paid = PaidStorage::new(plan.counts);
                parts = resolve_index_impl(index, work, allocator, Some(&mut paid))?;
                match storage::inventory_parts(&parts, work, at) {
                    Ok(inventory) => {
                        let attempts_after = allocator.attempts;
                        let delta_option = attempts_after.checked_sub(attempts_before);
                        match delta_option {
                            Some(delta) => {
                                match paid.reconcile(&plan, inventory, delta, work, at) {
                                    Ok(observation) => resolver_observation = observation,
                                    Err(error) => return Err(vec![*error]),
                                }
                            }
                            None => {
                                return Err(vec![*error(
                                    "E0400",
                                    format_args!("resolver allocation attempt counter regressed"),
                                    at,
                                )])
                            }
                        }
                    }
                    Err(error) => return Err(vec![*error]),
                }
            }
            // The resolver account and inventory borrows have ended. Move the
            // exact fresh parts directly into one owner; never upgrade a prior
            // executable owner or accept a caller-chosen seed/context.
            let typed_observation;
            {
                let (records, signatures, functions) = parts;
                let program = ResolvedOwnedProgram {
                    projection_bytes: std::cell::Cell::new(plan.total),
                    admission: SourceAdmission::ObserveEnumTypes,
                    index: IndexOwner::Borrowed(index),
                    work: MeterOwner::Borrowed(work),
                    sources: index.sources().view(),
                    records,
                    signatures,
                    functions,
                    entry: index.root_original_main(),
                };
                typed_observation =
                    super::typeck::observe_enum_type_storage(&program, &plan, &mut *allocator)?;
            }
            // The owner is gone before any combined facts are constructed.
            let attempts_final = allocator.attempts;
            let total_delta_option = attempts_final.checked_sub(attempts_before);
            match total_delta_option {
                Some(total_delta) => {
                    let phase_delta_option = resolver_observation
                        .reservation_attempts
                        .checked_add(typed_observation.typed_attempts);
                    match phase_delta_option {
                        Some(phase_delta) => {
                            if total_delta != phase_delta {
                                return Err(vec![*error(
                                    "E0500",
                                    format_args!("typed observation allocation interval mismatch"),
                                    at,
                                )]);
                            }
                            Ok(Some(EnumTypeStorageObservation {
                                resolver: resolver_observation,
                                typed: typed_observation,
                            }))
                        }
                        None => Err(vec![*error(
                            "E0400",
                            format_args!("typed observation allocation interval overflow"),
                            at,
                        )]),
                    }
                }
                None => Err(vec![*error(
                    "E0400",
                    format_args!("typed observation allocation counter regressed"),
                    at,
                )]),
            }
        }
        Ok(None) => Err(vec![*error(
            "E0500",
            format_args!("missing enum HIR preflight for typed observation"),
            at,
        )]),
        Err(error) => Err(vec![*error]),
    }
}

/// Fixed private consumer request, never a source owner or witness capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EnumPipelineRequest {
    pub(super) emit_llvm: bool,
    pub(super) fuel: usize,
}
impl EnumPipelineRequest {
    pub(super) const REFERENCE: Self = Self {
        emit_llvm: false,
        fuel: super::super::plan::MAX_FUEL,
    };
}

/// Only fixed observations and an optional bounded artifact/diagnostic escape.
#[derive(Debug)]
pub(super) struct EnumPipelineOutput {
    pub(super) observation: EnumPipelineObservation,
    pub(super) llvm: Option<Result<String, Box<Diagnostic>>>,
}

/// Private source-only selection through the measured closed construction.
/// Only fixed observations/artifacts escape; public admission stays unchanged.
#[cfg(test)]
#[allow(dead_code)]
pub(super) fn probe_enum_pipeline<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
    request: EnumPipelineRequest,
) -> Result<Option<EnumPipelineOutput>, Vec<Diagnostic>> {
    index
        .require_no_builtin_candidate()
        .map_err(|error| vec![*error])?;
    if index.enum_count() == 0 {
        return Ok(None);
    }
    fresh_enum_pipeline(index, work, allocator, request)
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) struct EnumPipelineObservation {
    pub(super) resolver: storage::ResolverStorageObservation,
    pub(super) typed: super::type_storage::TypeStorageObservation,
    pub(super) pipeline: super::program::EnumPipelineFacts,
}

#[cfg(test)]
#[allow(dead_code)]
fn fresh_enum_pipeline<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
    request: EnumPipelineRequest,
) -> Result<Option<EnumPipelineOutput>, Vec<Diagnostic>> {
    if index.enum_count() == 0 {
        return Ok(None);
    }
    let attempts_before = allocator.attempts;
    let at = index.sources().eof();
    match super::hir_budget::preflight_enum_hir(index, work) {
        Ok(Some(plan)) => {
            let parts;
            let resolver_observation;
            {
                let mut paid = PaidStorage::new(plan.counts);
                parts = resolve_index_impl(index, work, allocator, Some(&mut paid))?;
                match storage::inventory_parts(&parts, work, at) {
                    Ok(inventory) => {
                        let attempts_after = allocator.attempts;
                        let delta_option = attempts_after.checked_sub(attempts_before);
                        match delta_option {
                            Some(delta) => {
                                match paid.reconcile(&plan, inventory, delta, work, at) {
                                    Ok(observation) => resolver_observation = observation,
                                    Err(error) => return Err(vec![*error]),
                                }
                            }
                            None => {
                                return Err(vec![*error(
                                    "E0400",
                                    format_args!("resolver allocation attempt counter regressed"),
                                    at,
                                )])
                            }
                        }
                    }
                    Err(error) => return Err(vec![*error]),
                }
            }
            // The resolver account and inventory borrows have ended. Move the
            // exact fresh parts directly into one owner; never upgrade a prior
            // executable owner or accept a caller-chosen seed/context.
            let typed_observation;
            {
                let (records, signatures, functions) = parts;
                let program = ResolvedOwnedProgram {
                    projection_bytes: std::cell::Cell::new(plan.total),
                    admission: SourceAdmission::EnumPipeline,
                    index: IndexOwner::Borrowed(index),
                    work: MeterOwner::Borrowed(work),
                    sources: index.sources().view(),
                    records,
                    signatures,
                    functions,
                    entry: index.root_original_main(),
                };
                typed_observation =
                    super::typeck::finish_enum_pipeline(program, &plan, &mut *allocator, request)?;
            }
            // The owner is gone before any combined facts are constructed.
            let attempts_final = allocator.attempts;
            let total_delta_option = attempts_final.checked_sub(attempts_before);
            match total_delta_option {
                Some(total_delta) => {
                    let phase_delta_option = resolver_observation
                        .reservation_attempts
                        .checked_add(typed_observation.typed.typed_attempts);
                    match phase_delta_option {
                        Some(phase_delta) => {
                            if total_delta != phase_delta {
                                return Err(vec![*error(
                                    "E0500",
                                    format_args!("typed observation allocation interval mismatch"),
                                    at,
                                )]);
                            }
                            Ok(Some(EnumPipelineOutput {
                                observation: EnumPipelineObservation {
                                    resolver: resolver_observation,
                                    typed: typed_observation.typed,
                                    pipeline: typed_observation.pipeline,
                                },
                                llvm: typed_observation.llvm,
                            }))
                        }
                        None => Err(vec![*error(
                            "E0400",
                            format_args!("typed observation allocation interval overflow"),
                            at,
                        )]),
                    }
                }
                None => Err(vec![*error(
                    "E0400",
                    format_args!("typed observation allocation counter regressed"),
                    at,
                )]),
            }
        }
        Ok(None) => Err(vec![*error(
            "E0500",
            format_args!("missing enum HIR preflight for typed observation"),
            at,
        )]),
        Err(error) => Err(vec![*error]),
    }
}

// Stage D's separately prepaid actual roles. Existing resolver fixed,
// inventory and ProbeCarriers retain the plan/account/parts and resolver-fact
// pattern/caller roles plus the initial resolver interval controls. The primary
// local owner is the role already embedded in TypedOwnedProgram; only its new
// complete inline construction is added here. No owner Result/Option exists.
#[allow(dead_code)]
struct FreshTypeObservationCarriers {
    enum_count: usize,
    eof_source: SourceOwner<'static>,
    records: Vec<Record>,
    signatures: Vec<Signature>,
    functions: Vec<Function>,
    constructed_owner: ResolvedOwnedProgram<'static>,
    view_source: SourceOwner<'static>,
    view_return: SourceView<'static>,
    entry_return: Option<DefId>,
    seed: usize,
    cell_return: std::cell::Cell<usize>,
    owner_borrow: &'static ResolvedOwnedProgram<'static>,
    plan_borrow: &'static super::hir_budget::HirPlan,
    allocator_reborrow: &'static mut Allocator,
    typed_observation: super::type_storage::TypeStorageObservation,
    attempts_final: usize,
    total_delta_option: Option<usize>,
    total_delta: usize,
    phase_delta_option: Option<usize>,
    phase_delta: usize,
    interval_mismatch: bool,
    constructed: EnumTypeStorageObservation,
    optional: Option<EnumTypeStorageObservation>,
    returned: Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>>,
}
// Same closed fresh-construction role family, with an owned continuation input
// and larger fixed facts. Only one entry is selected; charge the measured excess
// over its existing bank rather than duplicating common plan/account/parts state.
#[allow(dead_code)]
struct FreshEnumPipelineCarriers {
    requests: [EnumPipelineRequest; 2],
    enum_count: usize,
    eof_source: SourceOwner<'static>,
    records: Vec<Record>,
    signatures: Vec<Signature>,
    functions: Vec<Function>,
    constructed_owner: ResolvedOwnedProgram<'static>,
    view_source: SourceOwner<'static>,
    view_return: SourceView<'static>,
    entry_return: Option<DefId>,
    seed: usize,
    cell_return: std::cell::Cell<usize>,
    owner_argument: ResolvedOwnedProgram<'static>,
    plan_borrow: &'static super::hir_budget::HirPlan,
    allocator_reborrow: &'static mut Allocator,
    typed_observation: super::typeck::EnumPipelineTypedOutput,
    typed_return: Result<super::typeck::EnumPipelineTypedOutput, Vec<Diagnostic>>,
    attempts_final: usize,
    total_delta_option: Option<usize>,
    total_delta: usize,
    phase_delta_option: Option<usize>,
    phase_delta: usize,
    interval_mismatch: bool,
    constructed_observation: EnumPipelineObservation,
    constructed: EnumPipelineOutput,
    optional: Option<EnumPipelineOutput>,
    returned: Result<Option<EnumPipelineOutput>, Vec<Diagnostic>>,
}
#[allow(dead_code)]
struct DeniedEnumPipelineProbeCarriers {
    requests: [EnumPipelineRequest; 2],
    request_denied: bool,
    index: &'static DeclarationIndex<'static>,
    work: &'static WorkMeter,
    allocator: &'static mut Allocator,
    enum_count: usize,
    sources: SourceOwner<'static>,
    origin: Span,
    returned: Result<Option<EnumPipelineOutput>, Vec<Diagnostic>>,
}
pub(super) const fn enum_pipeline_source_extra_bytes() -> usize {
    std::mem::size_of::<FreshEnumPipelineCarriers>()
        .saturating_sub(std::mem::size_of::<FreshTypeObservationCarriers>())
        + std::mem::size_of::<DeniedEnumPipelineProbeCarriers>()
            .saturating_sub(std::mem::size_of::<DeniedTypeProbeCarriers>())
}

#[allow(dead_code)]
struct TypeStorageCellCarriers {
    program: &'static ResolvedOwnedProgram<'static>,
    returned: &'static std::cell::Cell<usize>,
    caller: &'static std::cell::Cell<usize>,
}
// Pure sizeof getters. The existing fixed sizing-helper return role is
// reused; no future selector/caller role or production pricing array is implied.
#[allow(dead_code)]
pub(super) const fn fresh_type_observation_carrier_bytes() -> usize {
    std::mem::size_of::<FreshTypeObservationCarriers>()
}
#[allow(dead_code)]
pub(super) const fn type_storage_cell_carrier_bytes() -> usize {
    std::mem::size_of::<TypeStorageCellCarriers>()
}
#[allow(dead_code)]
pub(super) const fn enum_type_observation_bytes() -> usize {
    std::mem::size_of::<EnumTypeStorageObservation>()
}
#[allow(dead_code)]
pub(super) const fn enum_type_observation_option_bytes() -> usize {
    std::mem::size_of::<Option<EnumTypeStorageObservation>>()
}
#[allow(dead_code)]
pub(super) const fn enum_type_observation_return_bytes() -> usize {
    std::mem::size_of::<Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>>>()
}

// Retain the reviewed selector envelope, including its complete inhabited
// return. The separate helper input/return roles remain in their existing banks.
// source/origin are now unused conservative denial-era charges; they provide no
// credit for a different forwarding role and cause no pointless runtime query.
#[allow(dead_code)]
struct DeniedTypeProbeCarriers {
    index: &'static DeclarationIndex<'static>,
    work: &'static WorkMeter,
    allocator: &'static mut Allocator,
    enum_count: usize,
    source: SourceOwner<'static>,
    origin: Span,
    returned: Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>>,
}
pub(super) const fn denied_type_probe_carrier_bytes() -> usize {
    std::mem::size_of::<DeniedTypeProbeCarriers>()
}

// The ordinary wrapper retains its source gate and absent paid policy. Only the
// cfg(test) observation entry above supplies a preflighted local paid account.
fn resolve_index_impl(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    allocator: &mut Allocator,
    mut paid: Option<&mut PaidStorage>,
) -> Result<ResolvedParts, Vec<Diagnostic>> {
    if index.builtin_set() != BuiltinSet::None && paid.is_none() {
        return Err(vec![*invalid_signature_identity(index.sources().eof())]);
    }
    let sources = index.sources();
    let mut diagnostics = Vec::new();
    let mut records = match paid.as_deref_mut() {
        Some(paid) => paid
            .reserve(
                allocator,
                Kind::Records,
                index.record_count(),
                sources.eof(),
            )
            .map_err(|e| vec![*e])?,
        None => Vec::new(),
    };
    work.phase("record-fields");
    for id in 0..index.record_count() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let (key, module) = index.record(RecordId(id)).map_err(|e| vec![*e])?;
        let record = &sources.ast(module).map_err(|e| vec![*e])?.records[key.index];
        work.record_start(RecordId(id), record.name);
        let result = (|| {
            let mut fields = match paid.as_deref_mut() {
                Some(paid) => {
                    paid.reserve(allocator, Kind::Fields, record.fields.len(), record.span)?
                }
                None => Vec::new(),
            };
            let mut names = HashMap::new();
            for (position, field) in record.fields.iter().enumerate() {
                if paid.is_some() {
                    for earlier in &record.fields[..position] {
                        if storage::compare(index, work, earlier.name, field.name)?
                            == std::cmp::Ordering::Equal
                        {
                            return Err(duplicate(field.name, earlier.name));
                        }
                    }
                } else if let Some(first) = names.insert(sources.text(field.name)?, field.name) {
                    return Err(duplicate(field.name, first));
                }
                let ty = value_type(&mut index.query(work), module, field.ty)?;
                let ty = record_field_type(
                    ty,
                    if ty == ValueTy::Scalar(Ty::U8) {
                        field.ty.span
                    } else {
                        field.span
                    },
                )?;
                storage::room(&fields, fields.capacity(), paid.is_some(), field.span)?;
                fields.push(Field {
                    id: FieldId {
                        record: RecordId(id),
                        index: fields.len(),
                    },
                    ty,
                    name_span: field.name,
                    span: field.span,
                });
            }
            Ok(Record {
                id: RecordId(id),
                name_span: record.name,
                span: record.span,
                end: record.end,
                fields,
            })
        })();
        match result {
            Ok(record) => {
                storage::room(&records, records.capacity(), paid.is_some(), record.span)
                    .map_err(|e| vec![*e])?;
                records.push(record);
            }
            Err(error) => {
                work.record_error(&error);
                diagnostics.push(*error)
            }
        }
    }
    work.phase("signatures");
    let mut signatures = match paid.as_deref_mut() {
        Some(paid) => paid
            .reserve(
                allocator,
                Kind::Signatures,
                index.function_count(),
                sources.eof(),
            )
            .map_err(|e| vec![*e])?,
        None => Vec::new(),
    };
    for id in 0..index.source_function_count() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let (key, module) = index.function(DefId(id)).map_err(|e| vec![*e])?;
        let function = &sources.ast(module).map_err(|e| vec![*e])?.functions[key.index];
        work.signature_start(DefId(id), function.name);
        let result: Result<Signature, Box<Diagnostic>> = (|| {
            let params = if let Some(paid) = paid.as_deref_mut() {
                let mut params = paid.reserve(
                    allocator,
                    Kind::Parameters,
                    function.params.len(),
                    function.name,
                )?;
                for parameter in &function.params {
                    let ty = parameter_type(&mut index.query(work), module, parameter.ty)?;
                    storage::room(&params, params.capacity(), true, parameter.ty.span)?;
                    params.push(ty);
                }
                params
            } else {
                function
                    .params
                    .iter()
                    .map(|p| parameter_type(&mut index.query(work), module, p.ty))
                    .collect::<Result<Vec<_>, _>>()?
            };
            let result = value_type(&mut index.query(work), module, function.result)?;
            for block in &function.blocks {
                for statement in &block.body {
                    if let ast::StmtKind::Let {
                        annotation: Some(ty),
                        ..
                    } = statement.kind
                    {
                        value_type(&mut index.query(work), module, ty)?;
                    } else if index.enum_count() == 0
                        && matches!(statement.kind, ast::StmtKind::Match { .. })
                    {
                        // This existing eager walk precedes every function body
                        // reserve. Candidate syntax alone never selects paid HIR.
                        return Err(error(
                            "E0101",
                            format_args!("enum source syntax is unavailable"),
                            statement.span,
                        ));
                    }
                }
            }
            Ok(Signature {
                params,
                result,
                span: function.name,
            })
        })();
        match result {
            Ok(signature) => {
                storage::room(
                    &signatures,
                    signatures.capacity(),
                    paid.is_some(),
                    signature.span,
                )
                .map_err(|e| vec![*e])?;
                signatures.push(signature);
            }
            Err(error) => {
                work.record_error(&error);
                diagnostics.push(*error)
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    // Explicit finite calls keep the canonical input-before-output order without
    // retaining a generic inventory iterator in the source resolver.
    if index
        .builtin_set()
        .contains_function(BuiltinFunction::ReadStdin)
    {
        append_builtin_signature(
            index,
            work,
            allocator,
            paid.as_deref_mut(),
            &mut signatures,
            BuiltinFunction::ReadStdin,
        )
        .map_err(|error| vec![*error])?;
    }
    if index
        .builtin_set()
        .contains_function(BuiltinFunction::WriteStdout)
    {
        append_builtin_signature(
            index,
            work,
            allocator,
            paid.as_deref_mut(),
            &mut signatures,
            BuiltinFunction::WriteStdout,
        )
        .map_err(|error| vec![*error])?;
    }
    if records.iter().any(|record| {
        record
            .fields
            .iter()
            .any(|field| matches!(field.ty, ValueTy::Owned(_)))
    }) {
        use crate::frontend::oir::owned_types::{admit_value_layouts, DeclarationError};
        if let Err(failure) = admit_value_layouts(
            records
                .iter()
                .map(|record| record.fields.iter().map(|field| field.ty)),
        ) {
            let (code, message, span) = match failure {
                DeclarationError::ContainmentCycle(record) => (
                    "E0202",
                    "record containment must be acyclic",
                    records[record.0].name_span,
                ),
                DeclarationError::ResourceLimit(resource) => ("E0400", resource, sources.eof()),
                DeclarationError::LayoutOverflow => {
                    ("E0400", "record layout overflow", sources.eof())
                }
                DeclarationError::Allocation => (
                    "E0400",
                    "record declaration allocation failed",
                    sources.eof(),
                ),
                _ => (
                    "E0500",
                    "invalid resolved record declaration",
                    sources.eof(),
                ),
            };
            return Err(vec![*error(code, format_args!("{message}"), span)]);
        }
    }
    work.phase("exposure");
    // Exposure consumes already selected nominal identities; it does not resolve
    // signatures a second time or consult caller enumeration.
    for (id, signature) in signatures[..index.source_function_count()]
        .iter()
        .enumerate()
    {
        let (key, module) = index.function(DefId(id)).map_err(|e| vec![*e])?;
        let function = &sources.ast(module).map_err(|e| vec![*e])?.functions[key.index];
        let params = signature
            .params
            .iter()
            .zip(&function.params)
            .map(|(ty, p)| {
                (
                    match *ty {
                        ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(r)))
                        | ParameterTy::Reference {
                            referent: BorrowedTy::Exact(AggregateTy::Record(r)),
                            ..
                        } => Some(NominalId::Record(r)),
                        ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(e))) => {
                            Some(NominalId::Enum(e))
                        }
                        _ => None,
                    },
                    &p.ty,
                )
            });
        let result = (
            match signature.result {
                ValueTy::Owned(AggregateTy::Record(r)) => Some(NominalId::Record(r)),
                ValueTy::Owned(AggregateTy::Enum(e)) => Some(NominalId::Enum(e)),
                _ => None,
            },
            &function.result,
        );
        for (nominal, ty) in params.chain(std::iter::once(result)) {
            let Some(nominal) = nominal else { continue };
            let at = match ty.kind {
                ast::TypeSyntaxKind::Reference { referent, .. } => sources
                    .path_span(ItemPathRef {
                        file: ty.span.file,
                        path: referent,
                    })
                    .map_err(|e| vec![*e])?,
                _ => ty.span,
            };
            // Retain the old record query, work and diagnostic route exactly.
            let exposure = match nominal {
                NominalId::Record(record) => {
                    index.query(work).signature_exposure(DefId(id), record, at)
                }
                NominalId::Enum(_) => index
                    .query(work)
                    .nominal_signature_exposure(DefId(id), nominal, at)
                    .map(|exposure| match exposure {
                        NominalExposure::Allowed => Exposure::Allowed,
                        NominalExposure::Denied {
                            declaration,
                            restrictor,
                        } => Exposure::Denied {
                            record: declaration,
                            restrictor,
                        },
                    }),
            }
            .map_err(|e| vec![*e])?;
            match exposure {
                Exposure::Allowed => (),
                Exposure::Denied { record, restrictor } => {
                    let mut error = secondary(
                        error(
                            "E0207",
                            format_args!(
                                "function signature exposes a less visible {} type",
                                if matches!(nominal, NominalId::Enum(_)) {
                                    "enum"
                                } else {
                                    "record"
                                }
                            ),
                            at,
                        ),
                        record,
                        format_args!(
                            "{} type declared here",
                            if matches!(nominal, NominalId::Enum(_)) {
                                "enum"
                            } else {
                                "record"
                            }
                        ),
                    );
                    if let Some(restrictor) = restrictor {
                        error = secondary(
                            error,
                            restrictor,
                            format_args!("restrictive module declared here"),
                        );
                    }
                    work.record_error(&error);
                    diagnostics.push(*error);
                    break;
                }
            }
        }
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    work.phase("body-resolution");
    let mut functions = match paid.as_deref_mut() {
        Some(paid) => paid
            .reserve(
                allocator,
                Kind::Functions,
                index.source_function_count(),
                sources.eof(),
            )
            .map_err(|e| vec![*e])?,
        None => Vec::new(),
    };
    let mut array_entries = 0usize;
    for id in 0..index.source_function_count() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let (key, requester) = index.function(DefId(id)).map_err(|e| vec![*e])?;
        let ast = sources.ast(requester).map_err(|e| vec![*e])?;
        let mut resolver = Resolver {
            allocator,
            array_entries: &mut array_entries,
            ast,
            index,
            work,
            requester,
            records: &records,
            scope: HashMap::new(),
            storage: storage::ResolverStorage {
                paid: paid.as_deref_mut(),
                scope: None,
            },
            bindings: Vec::new(),
            expressions: Vec::new(),
        };
        match resolver.function(DefId(id), &ast.functions[key.index]) {
            Ok(function) => {
                storage::room(
                    &functions,
                    functions.capacity(),
                    resolver.storage.paid.is_some(),
                    function.end,
                )
                .map_err(|e| vec![*e])?;
                functions.push(function);
            }
            Err(error) => {
                work.record_error(&error);
                diagnostics.push(*error)
            }
        }
    }
    if diagnostics.is_empty() {
        Ok((records, signatures, functions))
    } else {
        Err(diagnostics)
    }
}

fn append_builtin_signature(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    allocator: &mut Allocator,
    paid: Option<&mut PaidStorage>,
    signatures: &mut Vec<Signature>,
    builtin: BuiltinFunction,
) -> Result<(), Box<Diagnostic>> {
    let at = index.builtin_function_anchor(builtin)?;
    let id = index.builtin_function_id(builtin)?;
    if id.0 != signatures.len() || id.0 < index.source_function_count() {
        return Err(invalid_signature_identity(at));
    }
    let enumeration = index.builtin_enum_id(match builtin {
        BuiltinFunction::ReadStdin => BuiltinEnum::ReadStatus,
        BuiltinFunction::WriteStdout => BuiltinEnum::WriteStatus,
    })?;
    let (parameter, result) = builtin.signature(enumeration);
    let paid = paid.ok_or_else(|| invalid_signature_identity(at))?;
    work.debit(2, at, "builtin signature construction")?;
    let mut params = paid.reserve(allocator, Kind::Parameters, 1, at)?;
    storage::room(&params, params.capacity(), true, at)?;
    params.push(parameter);
    storage::room(signatures, signatures.capacity(), true, at)?;
    signatures.push(Signature {
        params,
        result,
        span: at,
    });
    Ok(())
}
// Real construction-local state, never retained in the returned program.
// The vector backing is separately admitted by Kind::MatchArms.
struct MatchBuilder {
    scrutinee: BindingId,
    enumeration: Option<crate::frontend::oir::owned_types::EnumId>,
    variant_count: usize,
    coverage: [u64; 4],
    arms: Vec<MatchArm>,
}

pub(super) enum ResolveFrame {
    Enter(ast::BodyBlockId),
    Next(ast::BodyBlockId, usize),
    // One cursor per active match, regardless of its arm count. The parent
    // statement is retained before any arm body is entered.
    MatchArm {
        block: ast::BodyBlockId,
        statement: usize,
        arm: usize,
    },
    Leave,
    LeaveLoop,
}
struct Resolver<'i, 'a> {
    allocator: &'i mut Allocator,
    array_entries: &'i mut usize,
    ast: &'a ast::Program,
    index: &'i DeclarationIndex<'a>,
    work: &'i WorkMeter,
    requester: ModuleId,
    records: &'i [Record],
    scope: HashMap<&'a str, (BindingId, Span)>,
    // Construction-local policy; no returned program owner contains it.
    storage: super::resolver_storage::ResolverStorage<'i>,
    bindings: Vec<Binding>,
    expressions: Vec<Expr>,
}
impl<'a> Resolver<'_, 'a> {
    fn text(&self, span: Span) -> &'a str {
        self.index
            .sources()
            .text(span)
            .expect("validated source span")
    }
    fn active(&self, span: Span) -> Result<Option<BindingId>, Box<Diagnostic>> {
        match &self.storage.scope {
            Some(scope) => scope.active(self.index, self.work, span),
            None => Ok(self.scope.get(self.text(span)).map(|entry| entry.0)),
        }
    }
    fn lookup(&self, span: Span) -> Result<BindingId, Box<Diagnostic>> {
        // Projected array access and borrowing retain the named-root path in their
        // base span; only its first identifier participates in local lookup.
        let start = self
            .ast
            .tokens
            .partition_point(|token| token.span.end <= span.start);
        let root = self
            .ast
            .tokens
            .get(start)
            .map(|token| token.span)
            .unwrap_or(span);
        self.active(root)?.ok_or_else(|| {
            error(
                "E0200",
                format_args!(
                    "unknown local `{}`",
                    owned_diagnostic::name(self.text(span))
                ),
                span,
            )
        })
    }
    fn bind(
        &mut self,
        span: Span,
        annotation: Option<ValueTy>,
        mutable: bool,
        scope: BodyBlockId,
        parameter_position: Option<usize>,
    ) -> Result<BindingId, Box<Diagnostic>> {
        let name = self.text(span);
        if let Some(previous) = self.active(span)? {
            return Err(duplicate(span, self.bindings[previous.0].span));
        }
        if let Some(previous) = self
            .index
            .query(self.work)
            .value_binding_for_local_conflict(self.requester, span)?
        {
            return Err(duplicate(span, previous));
        }
        let row = match &self.storage.scope {
            Some(scope) => Some(scope.row(self.index, self.work, span)?.ok_or_else(|| {
                error(
                    "E0500",
                    format_args!("missing inventoried paid scope name"),
                    span,
                )
            })?),
            None => None,
        };
        let id = BindingId(self.bindings.len());
        storage::room(
            &self.bindings,
            self.bindings.capacity(),
            self.storage.paid.is_some(),
            span,
        )?;
        self.bindings.push(Binding {
            span,
            annotation,
            mutable,
            scope,
            parameter_position,
        });
        if let (Some(scope), Some(row)) = (&mut self.storage.scope, row) {
            scope.activate(row, id, span)?;
        } else {
            self.scope.insert(name, (id, span));
        }
        Ok(id)
    }
    fn function(
        &mut self,
        id: DefId,
        function: &ast::Function,
    ) -> Result<Function, Box<Diagnostic>> {
        let counts = if let Some(paid) = self.storage.paid.as_deref_mut() {
            let mut counts = super::hir_budget::HirCounts::default();
            super::hir_budget::count_function(self.ast, function, self.work, &mut counts)?;
            self.bindings = paid.reserve(
                self.allocator,
                Kind::Bindings,
                counts.bindings,
                function.name,
            )?;
            self.expressions = paid.reserve(
                self.allocator,
                Kind::Expressions,
                counts.expressions,
                function.name,
            )?;
            self.storage.scope = Some(storage::PaidScope::new(
                self.index,
                self.work,
                function,
                &counts,
                paid,
                self.allocator,
            )?);
            Some(counts)
        } else {
            None
        };
        for (position, param) in function.params.iter().enumerate() {
            self.bind(
                param.name,
                None,
                false,
                BodyBlockId(function.body.0),
                Some(position),
            )?;
        }
        // Keep block IDs stable while resolving statements depth first. Only
        // currently active names stay in the lookup table; each scope removes
        // its own names on exit, so neither cloning nor ancestor scans are needed.
        let mut blocks: Vec<BodyBlock> = if let Some(paid) = self.storage.paid.as_deref_mut() {
            let mut blocks = paid.reserve(
                self.allocator,
                Kind::Blocks,
                function.blocks.len(),
                function.name,
            )?;
            for block in &function.blocks {
                let body = paid.reserve(
                    self.allocator,
                    Kind::Statements,
                    block.body.len(),
                    block.span,
                )?;
                storage::room(&blocks, blocks.capacity(), true, block.span)?;
                blocks.push(BodyBlock {
                    body,
                    span: block.span,
                    end: block.end,
                });
            }
            blocks
        } else {
            function
                .blocks
                .iter()
                .map(|block| BodyBlock {
                    body: Vec::with_capacity(block.body.len()),
                    span: block.span,
                    end: block.end,
                })
                .collect()
        };
        // The paid branch never pushes into this legacy lexical-name carrier.
        let mut scopes: Vec<Vec<&'a str>> = Vec::new();
        let mut loops = if let (Some(paid), Some(counts)) =
            (self.storage.paid.as_deref_mut(), counts.as_ref())
        {
            paid.reserve(
                self.allocator,
                Kind::Loops,
                counts.loop_slots,
                function.name,
            )?
        } else {
            Vec::new()
        };
        let mut frames = if let (Some(paid), Some(counts)) =
            (self.storage.paid.as_deref_mut(), counts.as_ref())
        {
            let mut frames = paid.reserve(
                self.allocator,
                Kind::Frames,
                counts.resolve_frames,
                function.name,
            )?;
            storage::room(&frames, frames.capacity(), true, function.name)?;
            frames.push(ResolveFrame::Enter(function.body));
            frames
        } else {
            vec![ResolveFrame::Enter(function.body)]
        };
        while let Some(frame) = frames.pop() {
            let (block, index) = match frame {
                ResolveFrame::Enter(block) => {
                    if let Some(scope) = &mut self.storage.scope {
                        scope.enter(function.blocks[block.0].span)?;
                    } else {
                        scopes.push(Vec::new());
                    }
                    storage::room(
                        &frames,
                        frames.capacity(),
                        self.storage.paid.is_some(),
                        function.name,
                    )?;
                    frames.push(ResolveFrame::Leave);
                    storage::room(
                        &frames,
                        frames.capacity(),
                        self.storage.paid.is_some(),
                        function.name,
                    )?;
                    frames.push(ResolveFrame::Next(block, 0));
                    continue;
                }
                ResolveFrame::MatchArm {
                    block,
                    statement,
                    arm,
                } => {
                    let ast::StmtKind::Match { arms, .. } =
                        &function.blocks[block.0].body[statement].kind
                    else {
                        return Err(error(
                            "E0500",
                            format_args!("invalid match cursor"),
                            function.name,
                        ));
                    };
                    let Some(syntax) = arms.get(arm) else {
                        continue;
                    };
                    self.work.debit(1, syntax.span, "enum resolve arm entry")?;
                    if let Some(scope) = &mut self.storage.scope {
                        scope.enter(function.blocks[syntax.body.0].span)?;
                    } else {
                        scopes.push(Vec::new());
                    }
                    let StmtKind::Match { arms, .. } = &mut blocks[block.0].body[statement].kind
                    else {
                        return Err(error(
                            "E0500",
                            format_args!("invalid match cursor"),
                            syntax.span,
                        ));
                    };
                    if let Some(name) = syntax.binding {
                        let enumeration = self.index.enum_view(arms[arm].variant.enumeration)?;
                        let variant = enumeration.variant(arms[arm].variant)?;
                        let payload = variant.payload().ok_or_else(|| {
                            error(
                                "E0500",
                                format_args!("invalid match payload binding"),
                                syntax.span,
                            )
                        })?;
                        arms[arm].binding = Some(self.bind(
                            name,
                            Some(ValueTy::Scalar(payload)),
                            false,
                            BodyBlockId(syntax.body.0),
                            None,
                        )?);
                        if self.storage.paid.is_none() {
                            scopes
                                .last_mut()
                                .expect("active arm scope")
                                .push(self.text(name));
                        }
                    }
                    storage::room(
                        &frames,
                        frames.capacity(),
                        self.storage.paid.is_some(),
                        syntax.span,
                    )?;
                    frames.push(ResolveFrame::MatchArm {
                        block,
                        statement,
                        arm: arm + 1,
                    });
                    storage::room(
                        &frames,
                        frames.capacity(),
                        self.storage.paid.is_some(),
                        syntax.span,
                    )?;
                    frames.push(ResolveFrame::Leave);
                    storage::room(
                        &frames,
                        frames.capacity(),
                        self.storage.paid.is_some(),
                        syntax.span,
                    )?;
                    frames.push(ResolveFrame::Next(syntax.body, 0));
                    continue;
                }
                ResolveFrame::Leave => {
                    if let Some(scope) = &mut self.storage.scope {
                        scope.leave(self.work, function.name)?;
                    } else {
                        for name in scopes.pop().expect("entered scope") {
                            self.scope.remove(name);
                        }
                    }
                    continue;
                }
                ResolveFrame::LeaveLoop => {
                    loops.pop().expect("entered loop context");
                    continue;
                }
                ResolveFrame::Next(block, index) => (block, index),
            };
            let Some(statement) = function.blocks[block.0].body.get(index) else {
                continue;
            };
            storage::room(
                &frames,
                frames.capacity(),
                self.storage.paid.is_some(),
                statement.span,
            )?;
            frames.push(ResolveFrame::Next(block, index + 1));
            let kind = match &statement.kind {
                ast::StmtKind::Let {
                    mutable,
                    name,
                    annotation,
                    init,
                } => {
                    let init = self.expression(*init)?;
                    let annotation = annotation
                        .map(|ty| -> Result<ValueTy, Box<Diagnostic>> {
                            value_type(&mut self.index.query(self.work), self.requester, ty)
                        })
                        .transpose()?;
                    let local =
                        self.bind(*name, annotation, *mutable, BodyBlockId(block.0), None)?;
                    if self.storage.paid.is_none() {
                        scopes
                            .last_mut()
                            .expect("active body scope")
                            .push(self.text(*name));
                    }
                    StmtKind::Let {
                        binding: local,
                        init,
                    }
                }
                ast::StmtKind::Assign {
                    name,
                    operator_span,
                    value,
                } => {
                    let text = self.text(*name);
                    let local = self.active(*name)?.ok_or_else(|| {
                        diagnostic(
                            "E0200",
                            "resolve",
                            format_args!("unknown local `{}`", owned_diagnostic::name(text)),
                            Some(*name),
                        )
                    })?;
                    StmtKind::Assign {
                        binding: local,
                        target_span: *name,
                        operator_span: *operator_span,
                        value: self.expression(*value)?,
                    }
                }
                ast::StmtKind::FieldAssign {
                    base,
                    field,
                    target_span,
                    operator_span,
                    value,
                } => {
                    let binding = self.lookup(*base)?;
                    StmtKind::FieldAssign {
                        base: binding,
                        base_span: *base,
                        field_span: *field,
                        target_span: *target_span,
                        operator_span: *operator_span,
                        value: self.expression(*value)?,
                    }
                }
                ast::StmtKind::IndexAssign {
                    target,
                    operator_span,
                    value,
                } => {
                    let target = &self.ast.expressions[target.0];
                    self.work.debit(1, target.span, "array resolve store")?;
                    let ast::ExprKind::IndexRead { base, index } = target.kind else {
                        return Err(error(
                            "E0500",
                            format_args!("invalid indexed assignment target"),
                            target.span,
                        ));
                    };
                    let binding = self.lookup(base)?;
                    // The retained AST target wrapper is syntax only, never a HIR read.
                    let value = self.expression(*value)?;
                    let index = self.expression(index)?;
                    StmtKind::IndexAssign {
                        base: binding,
                        base_span: base,
                        target_span: target.span,
                        operator_span: *operator_span,
                        value,
                        index,
                    }
                }
                ast::StmtKind::Expr(expr) => StmtKind::Expr(self.expression(*expr)?),
                ast::StmtKind::Match { scrutinee, arms } => {
                    let scrutinee = self.lookup(*scrutinee)?;
                    if arms.is_empty() || arms.len() > 256 {
                        return Err(error(
                            "E0300",
                            format_args!("match requires 1..256 arms"),
                            statement.span,
                        ));
                    }
                    let mut builder = MatchBuilder {
                        scrutinee,
                        enumeration: None,
                        variant_count: 0,
                        coverage: [0u64; 4],
                        arms: match self.storage.paid.as_deref_mut() {
                            Some(paid) => paid.reserve(
                                self.allocator,
                                Kind::MatchArms,
                                arms.len(),
                                statement.span,
                            )?,
                            None => {
                                return Err(error(
                                    "E0101",
                                    format_args!("enum source syntax is unavailable"),
                                    statement.span,
                                ))
                            }
                        },
                    };
                    for arm in arms {
                        self.work.debit(1, arm.span, "enum resolve arm")?;
                        let variant = self.index.query(self.work).variant(
                            self.requester,
                            QualifiedPathRef {
                                file: arm.span.file,
                                path: arm.variant,
                            },
                        )?;
                        let view = self.index.enum_view(variant.enumeration)?;
                        if let Some(expected) = builder.enumeration {
                            if variant.enumeration != expected {
                                return Err(error(
                                    "E0300",
                                    format_args!("match arms must name variants of one enum"),
                                    arm.span,
                                ));
                            }
                        } else {
                            builder.enumeration = Some(variant.enumeration);
                            builder.variant_count = view.variant_count();
                        }
                        if variant.index >= builder.variant_count || variant.index >= 256 {
                            return Err(error(
                                "E0500",
                                format_args!("invalid match variant"),
                                arm.span,
                            ));
                        }
                        let declared = view.variant(variant)?;
                        if declared.payload().is_some() != arm.binding.is_some() {
                            return Err(error(
                                "E0300",
                                format_args!(
                                    "match pattern payload binding does not match variant"
                                ),
                                arm.span,
                            ));
                        }
                        let word = variant.index / 64;
                        let bit = 1u64 << (variant.index % 64);
                        if builder.coverage[word] & bit != 0 {
                            return Err(error(
                                "E0300",
                                format_args!("duplicate match variant"),
                                arm.span,
                            ));
                        }
                        builder.coverage[word] |= bit;
                        storage::room(&builder.arms, builder.arms.capacity(), true, arm.span)?;
                        builder.arms.push(MatchArm {
                            variant,
                            binding: None,
                            body: BodyBlockId(arm.body.0),
                            span: arm.span,
                        });
                    }
                    if arms.len() != builder.variant_count {
                        return Err(error(
                            "E0300",
                            format_args!("match must cover every enum variant exactly once"),
                            statement.span,
                        ));
                    }
                    storage::room(
                        &frames,
                        frames.capacity(),
                        self.storage.paid.is_some(),
                        statement.span,
                    )?;
                    frames.push(ResolveFrame::MatchArm {
                        block,
                        statement: index,
                        arm: 0,
                    });
                    StmtKind::Match {
                        scrutinee: builder.scrutinee,
                        arms: builder.arms,
                    }
                }
                ast::StmtKind::Return(expr) => {
                    StmtKind::Return(expr.map(|expr| self.expression(expr)).transpose()?)
                }
                ast::StmtKind::Break | ast::StmtKind::Continue => {
                    let is_break = matches!(statement.kind, ast::StmtKind::Break);
                    let keyword = if is_break { "break" } else { "continue" };
                    let target = loops.last().copied().ok_or_else(|| {
                        diagnostic(
                            "E0204",
                            "resolve",
                            format_args!(
                                "`{keyword}` requires an enclosing while in the same function"
                            ),
                            Some(statement.span),
                        )
                    })?;
                    if is_break {
                        StmtKind::Break { target }
                    } else {
                        StmtKind::Continue { target }
                    }
                }
                ast::StmtKind::While { condition, body } => {
                    let condition = self.expression(*condition)?;
                    let loop_id = LoopId(body.0);
                    storage::room(
                        &loops,
                        loops.capacity(),
                        self.storage.paid.is_some(),
                        statement.span,
                    )?;
                    loops.push(loop_id);
                    storage::room(
                        &frames,
                        frames.capacity(),
                        self.storage.paid.is_some(),
                        statement.span,
                    )?;
                    frames.push(ResolveFrame::LeaveLoop);
                    storage::room(
                        &frames,
                        frames.capacity(),
                        self.storage.paid.is_some(),
                        statement.span,
                    )?;
                    frames.push(ResolveFrame::Enter(*body));
                    StmtKind::While {
                        loop_id,
                        condition,
                        body: BodyBlockId(body.0),
                    }
                }
                ast::StmtKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let condition = self.expression(*condition)?;
                    if let Some(otherwise) = else_block {
                        storage::room(
                            &frames,
                            frames.capacity(),
                            self.storage.paid.is_some(),
                            statement.span,
                        )?;
                        frames.push(ResolveFrame::Enter(*otherwise));
                    }
                    storage::room(
                        &frames,
                        frames.capacity(),
                        self.storage.paid.is_some(),
                        statement.span,
                    )?;
                    frames.push(ResolveFrame::Enter(*then_block));
                    StmtKind::If {
                        condition,
                        then_block: BodyBlockId(then_block.0),
                        else_block: else_block.map(|id| BodyBlockId(id.0)),
                    }
                }
            };
            storage::room(
                &blocks[block.0].body,
                blocks[block.0].body.capacity(),
                self.storage.paid.is_some(),
                statement.span,
            )?;
            blocks[block.0].body.push(Stmt {
                kind,
                span: statement.span,
            });
        }
        if let (Some(paid), Some(scope)) = (
            self.storage.paid.as_deref_mut(),
            self.storage.scope.as_ref(),
        ) {
            // Exactly once, at this function's successful scratch lifetime end.
            // An observation error aborts; partial counters are never retried.
            paid.observe_scratch(scope, &loops, &frames, function.end)?;
        }
        Ok(Function {
            id,
            bindings: std::mem::take(&mut self.bindings),
            expressions: std::mem::take(&mut self.expressions),
            body: BodyBlockId(function.body.0),
            blocks,
            end: function.end,
        })
    }
    fn argument(&mut self, arg: &ast::Argument) -> Result<Argument, Box<Diagnostic>> {
        match arg {
            ast::Argument::Value(id) => self.expression(*id).map(Argument::Value),
            ast::Argument::Borrow {
                mutable,
                place,
                span,
            } => {
                let (name_span, star_span) = match place {
                    ast::BorrowPlace::OwnerName(name) => (*name, None),
                    ast::BorrowPlace::ForwardedParameter { name, star_span } => {
                        (*name, Some(*star_span))
                    }
                };
                let binding = self.lookup(name_span)?;
                Ok(Argument::Borrow {
                    kind: if *mutable {
                        BorrowKind::Exclusive
                    } else {
                        BorrowKind::Shared
                    },
                    place: if star_span.is_some() {
                        BorrowPlace::Forwarded(binding)
                    } else {
                        BorrowPlace::Owner(binding)
                    },
                    span: *span,
                    name_span,
                    star_span,
                })
            }
        }
    }
    fn expression(&mut self, id: ast::ExprId) -> Result<ExprId, Box<Diagnostic>> {
        let expr = &self.ast.expressions[id.0];
        let kind = match &expr.kind {
            ast::ExprKind::QualifiedValue { path, args } => {
                // The zero-enum absolute call is the same old query over the
                // same borrowed syntax, before any argument work or retention.
                let endpoint = if self.index.enum_count() == 0
                    && args.is_some()
                    && self.ast.paths[path.0].root == ast::PathRoot::Crate
                {
                    QualifiedValueEndpoint::Function(self.index.query(self.work).callee(
                        self.requester,
                        ItemPathRef {
                            file: expr.span.file,
                            path: ast::ItemPath::Absolute(*path),
                        },
                        false,
                    )?)
                } else {
                    self.index.query(self.work).qualified_value_endpoint(
                        self.requester,
                        QualifiedPathRef {
                            file: expr.span.file,
                            path: *path,
                        },
                    )?
                };
                match endpoint {
                    QualifiedValueEndpoint::Function(target) => {
                        let args = args.as_ref().ok_or_else(|| {
                            error(
                                "E0300",
                                format_args!("function call requires parentheses"),
                                expr.span,
                            )
                        })?;
                        let args = if let Some(paid) = self.storage.paid.as_deref_mut() {
                            let mut resolved = paid.reserve(
                                self.allocator,
                                Kind::Arguments,
                                args.len(),
                                expr.span,
                            )?;
                            for arg in args {
                                let arg = self.argument(arg)?;
                                storage::room(&resolved, resolved.capacity(), true, expr.span)?;
                                resolved.push(arg);
                            }
                            resolved
                        } else {
                            args.iter()
                                .map(|arg| self.argument(arg))
                                .collect::<Result<Vec<_>, _>>()?
                        };
                        ExprKind::Call { target, args }
                    }
                    QualifiedValueEndpoint::Variant(variant) => {
                        let enumeration = self.index.enum_view(variant.enumeration)?;
                        let declared = enumeration.variant(variant)?;
                        let payload = match (declared.payload(), args.as_deref()) {
                            (None, None) => None,
                            (Some(_), Some([ast::Argument::Value(value)])) => {
                                Some(self.expression(*value)?)
                            }
                            (None, Some(_)) => {
                                return Err(error(
                                    "E0300",
                                    format_args!(
                                        "nullary enum variant does not accept parentheses"
                                    ),
                                    expr.span,
                                ))
                            }
                            (Some(_), _) => {
                                return Err(error(
                                    "E0300",
                                    format_args!(
                                        "enum payload variant requires exactly one value argument"
                                    ),
                                    expr.span,
                                ))
                            }
                        };
                        ExprKind::ConstructEnum { variant, payload }
                    }
                }
            }
            ast::ExprKind::ArrayLiteral { elements } => {
                self.work.debit(1, expr.span, "array resolve literal")?;
                let requested = checked_array_entries(*self.array_entries, elements.len())
                    .map_err(|failure| array_reserve_error(failure, expr.span))?;
                let mut resolved = if let Some(paid) = self.storage.paid.as_deref_mut() {
                    paid.reserve(
                        self.allocator,
                        Kind::ArrayEntries,
                        elements.len(),
                        expr.span,
                    )?
                } else {
                    let mut resolved = Vec::new();
                    self.allocator
                        .vector_exact(&mut resolved, elements.len(), "array HIR elements")
                        .map_err(|failure| array_reserve_error(failure, expr.span))?;
                    resolved
                };
                *self.array_entries = requested;
                for element in elements {
                    self.work.debit(1, expr.span, "array resolve edge")?;
                    let element = self.expression(*element)?;
                    storage::room(
                        &resolved,
                        resolved.capacity(),
                        self.storage.paid.is_some(),
                        expr.span,
                    )?;
                    resolved.push(element);
                }
                ExprKind::ArrayLiteral { elements: resolved }
            }
            ast::ExprKind::IndexRead { base, index } => {
                self.work.debit(1, expr.span, "array resolve read")?;
                let binding = self.lookup(*base)?;
                let index = self.expression(*index)?;
                ExprKind::IndexRead {
                    base: binding,
                    base_span: *base,
                    index,
                }
            }
            ast::ExprKind::ArrayLength { base } => {
                self.work.debit(1, expr.span, "array resolve length")?;
                ExprKind::ArrayLength {
                    base: self.lookup(*base)?,
                    base_span: *base,
                }
            }
            ast::ExprKind::FieldRead { base, field } => ExprKind::FieldRead {
                base: self.lookup(*base)?,
                base_span: *base,
                field_span: *field,
            },
            ast::ExprKind::StructLiteral { record, fields } => {
                let path = *record;
                let at = self.index.sources().path_span(ItemPathRef {
                    file: expr.span.file,
                    path,
                })?;
                let record = self.index.query(self.work).record_type(
                    self.requester,
                    ItemPathRef {
                        file: expr.span.file,
                        path,
                    },
                    TypeContext::Constructor,
                )?;
                if let Access::Denied(field) =
                    self.index
                        .query(self.work)
                        .construction_access(self.requester, record, at)?
                {
                    let mut primary = at;
                    for init in fields {
                        if self
                            .index
                            .query(self.work)
                            .initializer_matches_field(field, init.name)?
                        {
                            primary = init.name;
                            break;
                        }
                    }
                    return Err(self
                        .index
                        .query(self.work)
                        .private_field_diagnostic(field, primary, "resolve")?);
                }
                let declared = &self.records[record.0];
                let mut seen = HashMap::new();
                let mut resolved = match self.storage.paid.as_deref_mut() {
                    Some(paid) => paid.reserve(
                        self.allocator,
                        Kind::FieldInitializers,
                        fields.len(),
                        expr.span,
                    )?,
                    None => Vec::new(),
                };
                for (position, field) in fields.iter().enumerate() {
                    let spelling = self.text(field.name);
                    let target = if self.storage.paid.is_some() {
                        let mut target = None;
                        for declared in &declared.fields {
                            if storage::compare(
                                self.index,
                                self.work,
                                declared.name_span,
                                field.name,
                            )? == std::cmp::Ordering::Equal
                            {
                                target = Some(declared);
                                break;
                            }
                        }
                        target
                    } else {
                        declared
                            .fields
                            .iter()
                            .find(|f| self.text(f.name_span) == spelling)
                    }
                    .ok_or_else(|| {
                        error(
                            "E0200",
                            format_args!(
                                "unknown literal field `{}`",
                                owned_diagnostic::name(spelling)
                            ),
                            field.name,
                        )
                    })?
                    .id;
                    if self.storage.paid.is_some() {
                        for earlier in &fields[..position] {
                            if storage::compare(self.index, self.work, earlier.name, field.name)?
                                == std::cmp::Ordering::Equal
                            {
                                return Err(duplicate(field.name, earlier.name));
                            }
                        }
                    } else if let Some(first) = seen.insert(spelling, field.name) {
                        return Err(duplicate(field.name, first));
                    }
                    let value = self.expression(field.value)?;
                    storage::room(
                        &resolved,
                        resolved.capacity(),
                        self.storage.paid.is_some(),
                        field.span,
                    )?;
                    resolved.push(FieldInit {
                        field: target,
                        value,
                        span: field.span,
                    });
                }
                #[cfg(test)]
                self.work
                    .observe(crate::frontend::declaration_index::Observation::Target {
                        operation: "constructor-resolved",
                        origin: at,
                        kind: "record",
                        id: record.0,
                    });
                ExprKind::StructLiteral {
                    record,
                    fields: resolved,
                }
            }
            ast::ExprKind::Bool(value) => ExprKind::Bool(*value),
            ast::ExprKind::Number { digits, negative } => {
                ExprKind::I32(decimal_i32(self.text(*digits), *negative, expr.span)?)
            }
            ast::ExprKind::Unit => ExprKind::Unit,
            ast::ExprKind::Name(span) => {
                let name = self.text(*span);
                let local = self.active(*span)?.ok_or_else(|| {
                    diagnostic(
                        "E0200",
                        "resolve",
                        format_args!("unknown local `{}`", owned_diagnostic::name(name)),
                        Some(*span),
                    )
                })?;
                ExprKind::Binding(local)
            }
            ast::ExprKind::Call { callee, args } => {
                let target = self.index.query(self.work).callee(
                    self.requester,
                    ItemPathRef {
                        file: expr.span.file,
                        path: *callee,
                    },
                    false,
                )?;
                let args = if let Some(paid) = self.storage.paid.as_deref_mut() {
                    let mut resolved =
                        paid.reserve(self.allocator, Kind::Arguments, args.len(), expr.span)?;
                    for arg in args {
                        let arg = self.argument(arg)?;
                        storage::room(&resolved, resolved.capacity(), true, expr.span)?;
                        resolved.push(arg);
                    }
                    resolved
                } else {
                    args.iter()
                        .map(|arg| self.argument(arg))
                        .collect::<Result<Vec<_>, _>>()?
                };
                ExprKind::Call { target, args }
            }
            ast::ExprKind::Conversion {
                op,
                operand,
                name_span,
            } => ExprKind::Conversion {
                source_expr: id,
                op: *op,
                operand: self.expression(*operand)?,
                name_span: *name_span,
            },
            ast::ExprKind::Group(inner) => ExprKind::Group(self.expression(*inner)?),
            ast::ExprKind::Negate {
                operand,
                operator_span,
            } => ExprKind::Negate {
                operand: self.expression(*operand)?,
                operator_span: *operator_span,
            },
            ast::ExprKind::Not {
                operand,
                operator_span,
            } => ExprKind::Not {
                operand: self.expression(*operand)?,
                operator_span: *operator_span,
            },
            ast::ExprKind::Logical {
                op,
                left,
                right,
                operator_span,
            } => {
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                ExprKind::Logical {
                    op: *op,
                    left,
                    right,
                    operator_span: *operator_span,
                }
            }
            ast::ExprKind::Comparison {
                op,
                left,
                right,
                operator_span,
            } => {
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                ExprKind::Comparison {
                    op: *op,
                    left,
                    right,
                    operator_span: *operator_span,
                }
            }
            ast::ExprKind::Arithmetic {
                op,
                left,
                right,
                operator_span,
            } => {
                // Complete the left subtree before starting the right, including calls.
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                ExprKind::Arithmetic {
                    op: *op,
                    left,
                    right,
                    operator_span: *operator_span,
                }
            }
        };
        let id = ExprId(self.expressions.len());
        storage::room(
            &self.expressions,
            self.expressions.capacity(),
            self.storage.paid.is_some(),
            expr.span,
        )?;
        self.expressions.push(Expr {
            kind,
            span: expr.span,
        });
        Ok(id)
    }
}

#[cfg(test)]
mod source_identity_tests {
    use super::*;
    #[test]
    fn projection_payload_admission_is_cumulative_and_checked_before_growth() {
        let mut map = SourceMap::new();
        let file = map.add("paths.ox".into(), "struct C{}".into());
        let source = map.get(file);
        let ast = crate::frontend::parser::parse_with_mode(
            source,
            crate::frontend::lexer::lex(source).unwrap(),
            crate::frontend::parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let program = resolve(source, &ast).unwrap();
        let span = source.span(0, 0);
        let maximum = super::super::budget::MAX_RAW_BYTES;
        program
            .projection_bytes
            .set(maximum - std::mem::size_of::<FieldId>());
        program.admit_projection(1, span).unwrap();
        assert_eq!(program.admit_projection(1, span).unwrap_err().code, "E0400");
        assert_eq!(program.projection_bytes.get(), maximum);
        program.projection_bytes.set(0);
        assert_eq!(
            program.admit_projection(65, span).unwrap_err().code,
            "E0400"
        );
        assert_eq!(program.projection_bytes.get(), 0);
        let metadata = std::mem::size_of::<Vec<FieldId>>();
        program.admit_projection_metadata(metadata, span).unwrap();
        program.admit_projection(2, span).unwrap();
        assert_eq!(
            program.projection_bytes.get(),
            metadata + 2 * std::mem::size_of::<FieldId>()
        );
        assert_eq!(
            program
                .admit_projection_metadata(usize::MAX, span)
                .unwrap_err()
                .code,
            "E0400"
        );
        assert_eq!(
            program.projection_bytes.get(),
            metadata + 2 * std::mem::size_of::<FieldId>()
        );
    }
    #[test]
    fn resolved_owned_names_select_each_original_file() {
        let mut map = SourceMap::new();
        let first = map.add("first.ox".into(), "fn old() -> () { return; }".into());
        let second = map.add(
            "second.ox".into(),
            "struct C {} fn main() -> () { return; }".into(),
        );
        let file = map.get(second);
        let ast = crate::frontend::parser::parse(file, crate::frontend::lexer::lex(file).unwrap())
            .unwrap();
        let program = resolve_in_map(file, &ast, &map).unwrap();
        assert_eq!(program.text(map.get(first).span(3, 6)), "old");
        assert_eq!(program.text(map.get(second).span(7, 8)), "C");
        assert_eq!(program.index().function(DefId(0)).unwrap().0.file, second);
    }
}

// Cumulative requested slots include pending outer buffers and never reset at
// function boundaries. These are checks, not a trusted allocation inventory.
fn checked_array_entries(current: usize, length: usize) -> Result<usize, ReserveFailure> {
    if length > 1024 {
        return Err(ReserveFailure::Overflow);
    }
    let total = current
        .checked_add(length)
        .ok_or(ReserveFailure::Overflow)?;
    total
        .checked_mul(std::mem::size_of::<ExprId>())
        .ok_or(ReserveFailure::Overflow)?;
    Ok(total)
}
fn array_reserve_error(failure: ReserveFailure, span: Span) -> Box<Diagnostic> {
    error(
        "E0400",
        format_args!(
            "{}",
            match failure {
                ReserveFailure::Overflow => "array HIR count overflow",
                ReserveFailure::Allocation => "array HIR allocation failed",
            }
        ),
        span,
    )
}

/// Private observation construction only. Callers must copy inert facts before
/// typing consumes this owner; no executable entrypoint chooses this policy.
#[cfg(test)]
pub(super) fn resolve_array_types<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let (index, (records, signatures, functions)) =
        resolve_source_parts(sources, work, allocator, limits)?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::ObserveArrayTypes,
        sources: sources.view(),
        index: IndexOwner::Owned(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry,
    })
}

/// Only the separate source-only test entry chooses this immutable policy.
/// A types-only owner cannot be converted or forwarded into this construction.
#[cfg(test)]
pub(super) fn resolve_array_pipeline<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let (index, (records, signatures, functions)) =
        resolve_source_parts(sources, work, allocator, limits)?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::ObserveArrayPipeline,
        sources: sources.view(),
        index: IndexOwner::Owned(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry,
    })
}

/// A fresh source owner is required for consumer qualification. Existing
/// observation admissions remain immutable and cannot be promoted.
#[cfg(test)]
pub(super) fn resolve_array_consumer<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let (index, (records, signatures, functions)) =
        resolve_source_parts(sources, work, allocator, limits)?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::ArrayConsumer,
        sources: sources.view(),
        index: IndexOwner::Owned(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry,
    })
}

#[cfg(test)]
mod array_reservation_tests {
    use super::*;
    #[test]
    fn unit3b1_checked_array_entry_arithmetic_is_cumulative() {
        assert_eq!(checked_array_entries(0, 0), Ok(0));
        assert_eq!(checked_array_entries(2, 3), Ok(5));
        assert_eq!(checked_array_entries(0, 1024), Ok(1024));
        for (control, current, length) in [
            ("length", 0, 1025),
            ("cumulative", usize::MAX, 1),
            ("multiply", usize::MAX / std::mem::size_of::<ExprId>(), 1),
        ] {
            let result = checked_array_entries(current, length);
            println!("CONTROL {{\"schema\":\"oxid-array-types-controls-v1\",\"kind\":\"overflow\",\"control\":\"{control}\",\"synthetic\":true,\"current\":{current},\"length\":{length},\"element_bytes\":{},\"result\":\"{:?}\",\"reservation_attempts\":0}}", std::mem::size_of::<ExprId>(), result);
            assert_eq!(result, Err(ReserveFailure::Overflow));
        }
        let mut allocator = Allocator {
            attempts: usize::MAX,
            ..Allocator::default()
        };
        let mut values = Vec::<ExprId>::new();
        let result = allocator.vector_exact(&mut values, 0, "array HIR elements");
        println!("CONTROL {{\"schema\":\"oxid-array-types-controls-v1\",\"kind\":\"overflow\",\"control\":\"attempt\",\"synthetic\":true,\"attempt_counter\":{},\"result\":\"{:?}\",\"trace_rows\":{}}}", allocator.attempts, result, allocator.trace.len());
        assert_eq!(result, Err(ReserveFailure::Overflow));
        assert!(allocator.trace.is_empty() && values.is_empty() && values.capacity() == 0);
    }
}

#[cfg(test)]
mod enum_index_layout_tests {
    use super::*;
    #[test]
    fn enum_index_owned_direct_source_producer_rejects_before_hir_reservations() {
        for text in [
            "enum E{V} fn main()->(){return;}",
            "fn main()->(){E::V;return;}",
            "fn main()->(){match x{E::V=>{}} return;}",
        ] {
            let mut sources = SourceMap::new();
            let file = sources.add("enum-index-owned-gate.ox".into(), text.into());
            let source = sources.get(file);
            let (ast, _) = crate::frontend::parser::parse_enum_candidate_counted(
                source,
                crate::frontend::lexer::lex(source).unwrap(),
                crate::frontend::parser::SourceMode::ProjectCandidate,
                crate::frontend::parser::MAX_NODES,
                &mut Allocator::default(),
                &mut Default::default(),
            )
            .unwrap();
            let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            let index =
                index::collect_enum_candidate(owner, IndexLimits::default(), &work, &mut allocator)
                    .unwrap()
                    .finish(&work, &mut allocator)
                    .unwrap();
            let mut denied_allocator = Allocator {
                fail_at: Some(1),
                ..Allocator::default()
            };
            assert_eq!(
                resolve_index(&index, &work, &mut denied_allocator).unwrap_err()[0].code,
                "E0101"
            );
            assert_eq!(denied_allocator.attempts, 0);
            assert_eq!(resolve_project(&index, &work).unwrap_err()[0].code, "E0101");
        }
    }
    #[test]
    fn enum_index_enclosing_source_owners_are_measured() {
        println!(
            "enum-index-source-layout IndexOwner={} ResolvedOwnedProgram={} TypedOwnedProgram={}",
            std::mem::size_of::<IndexOwner<'_>>(),
            std::mem::size_of::<ResolvedOwnedProgram<'_>>(),
            std::mem::size_of::<super::super::typeck::TypedOwnedProgram<'_>>()
        );
    }
}

/// Source-owned query receivers are additional to the index's internal ledger.
/// Charge complete return and caller carriers separately; no new enum table or
/// inherited helper-stack model is included. One branch survives per recursive
/// expression level, including constructors whose only child is their payload.
pub(super) const fn enum_expression_carrier_bytes() -> usize {
    use std::mem::size_of;
    size_of::<QualifiedValueEndpoint>()
        + size_of::<Result<QualifiedValueEndpoint, Box<Diagnostic>>>()
        + size_of::<QualifiedPathRef>()
        + size_of::<index::EnumView<'_>>()
        + size_of::<Result<index::EnumView<'_>, Box<Diagnostic>>>()
        + size_of::<index::VariantView<'_>>()
        + size_of::<Result<index::VariantView<'_>, Box<Diagnostic>>>()
        + size_of::<Option<Ty>>()
        + size_of::<Option<&[ast::Argument]>>()
        + size_of::<Option<ExprId>>()
        + size_of::<VariantId>()
        + size_of::<DefId>()
        + size_of::<&Vec<ast::Argument>>()
        + size_of::<Option<&Vec<ast::Argument>>>()
        // The match discriminant tuple is its own complete temporary, in
        // addition to the separately received payload/slice values above.
        + size_of::<(Option<Ty>, Option<&[ast::Argument]>)>()
        + size_of::<Result<ExprId, Box<Diagnostic>>>()
        + size_of::<&ast::ExprId>()
        + size_of::<ast::ExprId>()
}
/// A complete actual builder, the shape loop and the later incremental arm
/// entry's borrowed row/view receivers. The phases do not recursively call one
/// another; summing them once per match is conservative. Vector transport is
/// already in PaidStorage's generic complete-return envelope.
pub(super) const fn enum_match_carrier_bytes() -> usize {
    use std::mem::size_of;
    size_of::<MatchBuilder>()
        // Row construction and the local scrutinee copy precede their moves
        // into admitted retained carriers; do not assume compiler slot reuse.
        + size_of::<MatchArm>()
        + size_of::<BindingId>()
        + size_of::<crate::frontend::oir::owned_types::EnumId>()
        + size_of::<Span>()
        + size_of::<&Vec<ast::MatchArmSyntax>>()
        + size_of::<&mut Vec<MatchArm>>()
        + size_of::<ResolveFrame>()
        + size_of::<std::slice::Iter<'_, ast::MatchArmSyntax>>()
        + size_of::<Option<&ast::MatchArmSyntax>>()
        + 2 * size_of::<&ast::MatchArmSyntax>()
        + size_of::<VariantId>()
        + size_of::<Result<VariantId, Box<Diagnostic>>>()
        + size_of::<QualifiedPathRef>()
        + 2 * size_of::<index::EnumView<'_>>()
        + 2 * size_of::<Result<index::EnumView<'_>, Box<Diagnostic>>>()
        + 2 * size_of::<index::VariantView<'_>>()
        + 2 * size_of::<Result<index::VariantView<'_>, Box<Diagnostic>>>()
        + size_of::<Option<Ty>>()
        + size_of::<Result<Ty, Box<Diagnostic>>>()
        + size_of::<Option<ValueTy>>()
        + size_of::<Option<BindingId>>()
        + size_of::<Result<BindingId, Box<Diagnostic>>>()
        + 2 * size_of::<usize>()
        + size_of::<u64>()
}

/// Complete resolver header; its inherited HashMap payload is not admitted by C3a.
#[allow(dead_code)]
pub(super) const fn resolver_carrier_bytes() -> usize {
    std::mem::size_of::<Resolver<'_, '_>>()
}

#[cfg(test)]
#[path = "resolver_paid_tests.rs"]
mod paid_tests;

#[cfg(test)]
mod fresh_type_observation_layout_tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn c3_t1_uncalled_fresh_observation_models_have_explicit_roles() {
        // Type-only schemas. Never invoke the fresh helper or assemble an owner.
        macro_rules! roles {
            ($model:ty, $count:expr; $( $field:ident : $ty:ty ),+ $(,)?) => {{
                $(let _: for<'a> fn(&'a $model) -> &'a $ty = |model| &model.$field;)+
                let roles = [$( (std::mem::offset_of!($model, $field), size_of::<$ty>(), align_of::<$ty>()) ),+];
                assert_eq!(roles.len(), $count);
                let mut occupied = 0;
                for (i, (offset, bytes, alignment)) in roles.iter().copied().enumerate() {
                    assert_eq!(offset % alignment, 0);
                    assert!(offset + bytes <= size_of::<$model>());
                    occupied += bytes;
                    for (j, (other, width, _)) in roles.iter().copied().enumerate() {
                        if i != j && bytes != 0 && width != 0 {
                            assert!(offset + bytes <= other || other + width <= offset);
                        }
                    }
                }
                assert!(occupied <= size_of::<$model>());
                println!("C3_T1_FRESH_ROLES {} fields={} typed_bytes={} padding={}",
                    stringify!($model), roles.len(), occupied, size_of::<$model>() - occupied);
            }};
        }
        roles!(EnumTypeStorageObservation, 2;
            resolver: storage::ResolverStorageObservation,
            typed: super::super::type_storage::TypeStorageObservation);
        roles!(FreshTypeObservationCarriers, 24;
            enum_count: usize, eof_source: SourceOwner<'static>, records: Vec<Record>,
            signatures: Vec<Signature>, functions: Vec<Function>,
            constructed_owner: ResolvedOwnedProgram<'static>, view_source: SourceOwner<'static>,
            view_return: SourceView<'static>, entry_return: Option<DefId>, seed: usize,
            cell_return: std::cell::Cell<usize>, owner_borrow: &'static ResolvedOwnedProgram<'static>,
            plan_borrow: &'static super::super::hir_budget::HirPlan, allocator_reborrow: &'static mut Allocator,
            typed_observation: super::super::type_storage::TypeStorageObservation,
            attempts_final: usize, total_delta_option: Option<usize>, total_delta: usize,
            phase_delta_option: Option<usize>, phase_delta: usize, interval_mismatch: bool,
            constructed: EnumTypeStorageObservation, optional: Option<EnumTypeStorageObservation>,
            returned: Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>>);
        roles!(TypeStorageCellCarriers, 3;
            program: &'static ResolvedOwnedProgram<'static>,
            returned: &'static std::cell::Cell<usize>, caller: &'static std::cell::Cell<usize>);
        macro_rules! layout {
            ($($ty:ty),* $(,)?) => { $(
                println!("C3_T1_FRESH_LAYOUT {} {} {}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
            )* };
        }
        layout!(
            EnumTypeStorageObservation,
            Option<EnumTypeStorageObservation>,
            Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>>,
            FreshTypeObservationCarriers,
            TypeStorageCellCarriers
        );
        assert_eq!(
            fresh_type_observation_carrier_bytes(),
            size_of::<FreshTypeObservationCarriers>()
        );
        assert_eq!(
            type_storage_cell_carrier_bytes(),
            size_of::<TypeStorageCellCarriers>()
        );
        assert_eq!(
            enum_type_observation_bytes(),
            size_of::<EnumTypeStorageObservation>()
        );
        assert_eq!(
            enum_type_observation_option_bytes(),
            size_of::<Option<EnumTypeStorageObservation>>()
        );
        assert_eq!(
            enum_type_observation_return_bytes(),
            size_of::<Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>>>()
        );
    }
}

#[cfg(test)]
#[path = "resolver_enum_tests.rs"]
mod enum_semantic_tests;

#[cfg(test)]
#[path = "enum_storage_failure_tests.rs"]
mod enum_storage_failure_tests;

// New production-only named transports. Existing resolver/paid preparation
// banks retain their exact plan/account/parts/owner-construction roles; no
// artifact/observation envelope is substituted for a typed-owner return.
#[allow(dead_code)]
struct ProductionSourceCarriers {
    index: &'static DeclarationIndex<'static>,
    work: &'static WorkMeter,
    allocator: &'static mut Allocator,
    provenance_return: Result<(), Box<Diagnostic>>,
    provenance_normalized: Result<(), Vec<Diagnostic>>,
    plan_normalized: Result<Option<super::hir_budget::HirPlan>, Vec<Diagnostic>>,
    selected_plan: Result<super::hir_budget::HirPlan, Vec<Diagnostic>>,
    inventory_normalized: Result<storage::ResolverInventory, Vec<Diagnostic>>,
    resolver_end: usize,
    checkpoint_argument: usize,
    delta_normalized: Result<usize, Vec<Diagnostic>>,
    reconciliation_normalized: Result<storage::ResolverStorageObservation, Vec<Diagnostic>>,
    discarded_observation: storage::ResolverStorageObservation,
    returned: Result<super::typeck::TypedOwnedProgram<'static>, Vec<Diagnostic>>,
    // The source wrappers now transfer to one paid body. Both the wrapper and
    // shared callee have named input/return roles; no tail-call elision assumed.
    dispatch_inputs: (
        &'static DeclarationIndex<'static>,
        &'static WorkMeter,
        &'static mut Allocator,
        SourceAdmission,
    ),
    dispatch_return: Result<super::typeck::TypedOwnedProgram<'static>, Vec<Diagnostic>>,
    admission_guard: PaidSourceAdmissionCarriers,
}
/// Complete new guard roles. Each call site pays its own copy, without
/// assuming that predicate receivers, comparison results or returned booleans
/// reuse the caller's gate storage. Private branch roles are included in this
/// conservative shared envelope, as are the current branch's actual transports.
#[allow(dead_code)]
pub(super) struct PaidSourceAdmissionCarriers {
    admission: SourceAdmission,
    index: &'static DeclarationIndex<'static>,
    current_receiver: &'static DeclarationIndex<'static>,
    current_result: bool,
    current_admission_matches: bool,
    private_admission_matches: bool,
    output_receiver: &'static DeclarationIndex<'static>,
    output_result: bool,
    input_receiver: &'static DeclarationIndex<'static>,
    input_result: bool,
    input_inventory: BuiltinSet,
    input_has_output: bool,
    private_marker_matches: bool,
    returned: bool,
    rejected: bool,
}
#[cfg(test)]
pub(super) const fn paid_source_admission_carrier_bytes() -> usize {
    std::mem::size_of::<PaidSourceAdmissionCarriers>()
}
pub(super) const fn production_source_carrier_bytes() -> usize {
    std::mem::size_of::<ProductionSourceCarriers>()
}

/// New signature construction roles. The pending Vec<ParameterTy> is already
/// priced by HirCounts.signatures; the retained header lives inside Signature.
/// The complete constructed Signature is separate from both named headers.
#[allow(dead_code)]
struct BuiltinSignatureCarriers {
    index: &'static DeclarationIndex<'static>,
    work: &'static WorkMeter,
    allocator: &'static mut Allocator,
    paid_argument: Option<&'static mut PaidStorage>,
    paid: &'static mut PaidStorage,
    signatures: &'static mut Vec<Signature>,
    builtin: BuiltinFunction,
    enum_selector: BuiltinEnum,
    anchor: Span,
    function: DefId,
    enumeration: crate::frontend::oir::owned_types::EnumId,
    recipe: (ParameterTy, ValueTy),
    recipe_return: (ParameterTy, ValueTy),
    constructed: Signature,
    anchor_return: Result<Span, Box<Diagnostic>>,
    function_return: Result<DefId, Box<Diagnostic>>,
    enum_return: Result<crate::frontend::oir::owned_types::EnumId, Box<Diagnostic>>,
    paid_return: Result<&'static mut PaidStorage, Box<Diagnostic>>,
    returns: [Result<(), Box<Diagnostic>>; 4],
    normalized: Result<(), Vec<Diagnostic>>,
}

/// Two distinct caller families are summed in the conservative source envelope:
/// fresh typing checks the resolved owner; pre-lowering checks the typed wrapper.
/// Repeated nonrecursive preflight calls reuse the latter named control family.
#[allow(dead_code)]
struct SignatureIdentityCarriers {
    program: &'static ResolvedOwnedProgram<'static>,
    typed: &'static super::typeck::TypedOwnedProgram<'static>,
    index: &'static DeclarationIndex<'static>,
    at: Span,
    functions: std::iter::Enumerate<std::slice::Iter<'static, Function>>,
    next: Option<(usize, &'static Function)>,
    current: (usize, &'static Function),
    builtin_array: [BuiltinFunction; 2],
    builtin_functions: std::array::IntoIter<BuiltinFunction, 2>,
    builtin_next: Option<BuiltinFunction>,
    builtin: BuiltinFunction,
    enum_selector: BuiltinEnum,
    expected: usize,
    function: DefId,
    enumeration: crate::frontend::oir::owned_types::EnumId,
    anchor: Span,
    signature: &'static Signature,
    recipe: (ParameterTy, ValueTy),
    recipe_return: (ParameterTy, ValueTy),
    parameter_comparison: [ParameterTy; 1],
    anchor_return: Result<Span, Box<Diagnostic>>,
    function_return: Result<DefId, Box<Diagnostic>>,
    enum_return: Result<crate::frontend::oir::owned_types::EnumId, Box<Diagnostic>>,
    signature_option: Option<&'static Signature>,
    signature_return: Result<&'static Signature, Box<Diagnostic>>,
    returns: [Result<(), Box<Diagnostic>>; 4],
    normalized: Result<(), Vec<Diagnostic>>,
}
pub(super) const fn builtin_signature_carrier_bytes() -> usize {
    std::mem::size_of::<BuiltinSignatureCarriers>()
}
pub(super) const fn signature_identity_carrier_bytes() -> usize {
    std::mem::size_of::<SignatureIdentityCarriers>()
}

#[cfg(test)]
#[path = "builtin_signature_tests.rs"]
mod builtin_signature_tests;

#[cfg(test)]
#[path = "output_typing_tests.rs"]
mod output_typing_tests;
