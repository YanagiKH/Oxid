//! The owned source association walk; raw ownership/shape verification is unchanged.
use super::super::*;
use crate::frontend::{
    builtin_catalog::{BuiltinEnum, BuiltinFunction},
    declaration_index::DeclarationIndex,
    oir::source::association::{
        authenticate_conversion, bad, conversion_carrier_bytes, conversion_seen_bytes, BindUsage,
        ConversionOwners, ConversionSeen, Visitor,
    },
};

/// Owns the exact authenticated program; there is no detachable or reusable permit.
pub(in crate::frontend::oir::owned) struct AssociatedOwned<'s> {
    raw: RawOwnedProgram,
    sources: &'s SourceMap,
}
impl<'s> AssociatedOwned<'s> {
    pub(super) fn program(&self) -> &RawOwnedProgram {
        &self.raw
    }

    pub(in crate::frontend::oir::owned) fn into_parts(self) -> (RawOwnedProgram, &'s SourceMap) {
        (self.raw, self.sources)
    }
}
/// Source authority is minted only from this exact trusted lowering invocation.
/// No production caller may supply raw operations or a detachable source permit.
pub(super) fn lower_and_associate<'s>(
    typed: &'s super::typeck::TypedOwnedProgram<'_>,
) -> Result<AssociatedOwned<'s>, Box<Diagnostic>> {
    let crate::frontend::source::SourceView::Map(sources) = typed.index().sources().view() else {
        return Err(bad());
    };
    typed.index().validate_generic_source(typed.work())?;
    let raw =
        super::lower::lower(typed).map_err(|error| super::diagnostic::lower(&error, sources))?;
    let index = typed.index();
    index.require_current_source_pipeline()?;
    check_impl(
        &raw,
        index,
        sources,
        index.enum_count() != 0,
        index.builtin_set() != BuiltinOrigins::None,
        Some(typed),
    )?;
    Ok(AssociatedOwned { raw, sources })
}

// Private historical test helper. It is absent from production, where only
// lower_and_associate above can mint this source-owned value.
#[cfg(test)]
fn associate_lowered<'s>(
    raw: RawOwnedProgram,
    typed: &'s super::typeck::TypedOwnedProgram<'_>,
) -> Result<AssociatedOwned<'s>, Box<Diagnostic>> {
    let index = typed.index();
    index.require_current_source_pipeline()?;
    let crate::frontend::source::SourceView::Map(sources) = index.sources().view() else {
        return Err(bad());
    };
    check_impl(
        &raw,
        index,
        sources,
        index.enum_count() != 0,
        index.builtin_set() != BuiltinOrigins::None,
        Some(typed),
    )?;
    Ok(AssociatedOwned { raw, sources })
}
/// Historical mutation observations never construct SourceProgram. Keep this
/// test-only raw seam out of production instead of granting it source authority.
#[cfg(test)]
pub(in crate::frontend::oir::owned) fn associate<'s>(
    raw: RawOwnedProgram,
    typed: &'s super::typeck::TypedOwnedProgram<'_>,
) -> Result<AssociatedOwned<'s>, Box<Diagnostic>> {
    associate_lowered(raw, typed)
}

#[cfg(test)]
pub(super) fn associate_candidate<'s>(
    raw: RawOwnedProgram,
    typed: &'s super::typeck::TypedOwnedProgram<'_>,
) -> Result<AssociatedOwned<'s>, Box<Diagnostic>> {
    let index = typed.index();
    let builtins = typed.admission() == super::resolve::SourceAdmission::BuiltinPipeline;
    if builtins {
        index.require_builtin_candidate_pipeline()?;
    } else {
        index.require_no_builtin_candidate()?;
    }
    let crate::frontend::source::SourceView::Map(sources) = index.sources().view() else {
        return Err(bad());
    };
    check_impl(&raw, index, sources, true, builtins, Some(typed))?;
    Ok(AssociatedOwned { raw, sources })
}

fn enumeration(
    enumeration: &RawEnumDecl,
    visitor: &mut Visitor<'_>,
) -> Result<(), Box<Diagnostic>> {
    visitor.declaration()?;
    visitor.span(enumeration.span)?;
    for variant in &enumeration.variants {
        visitor.declaration()?;
        visitor.span(variant.span)?;
    }
    Ok(())
}
fn record(record: &RawRecordDecl, visitor: &mut Visitor<'_>) -> Result<(), Box<Diagnostic>> {
    visitor.declaration()?;
    visitor.span(record.span)?;
    for field in &record.fields {
        visitor.declaration()?;
        visitor.span(field.span)?;
    }
    Ok(())
}
fn origins(
    origins: Option<DiagnosticOrigins>,
    visitor: &mut Visitor<'_>,
) -> Result<(), Box<Diagnostic>> {
    if let Some(origins) = origins {
        visitor.span(origins.primary)?;
        visitor.span(origins.cause)?;
    }
    Ok(())
}
#[derive(Clone, Copy)]
pub(super) enum FunctionSource<'a> {
    Count,
    Synthetic,
    Original(
        &'a crate::frontend::ast::Program,
        &'a crate::frontend::ast::Function,
        &'a SourceMap,
        usize,
    ),
}
fn function(
    function: &RawOwnedFunction,
    visitor: &mut Visitor<'_>,
    allow_enums: bool,
    builtin: Option<BuiltinFunction>,
    source: FunctionSource<'_>,
    seen: &mut ConversionSeen,
) -> Result<(), Box<Diagnostic>> {
    if !allow_enums && !function.matches.is_empty() {
        return Err(bad());
    }
    visitor.declaration()?;
    visitor.span(function.span)?;
    for local in &function.locals {
        visitor.span(local.span)?;
    }
    for place in &function.places {
        visitor.span(place.span)?;
    }
    for owner in &function.owners {
        visitor.span(owner.span)?;
    }
    for reference in &function.references {
        visitor.span(reference.span)?;
    }
    for call in &function.calls {
        visitor.span(call.span)?;
    }
    for loan in &function.loans {
        visitor.span(loan.span)?;
    }
    for descriptor in &function.matches {
        visitor.declaration()?;
        visitor.span(descriptor.span)?;
        for _ in &descriptor.arms {
            // Raw arm rows have no independent source span.
            visitor.declaration()?;
        }
    }
    for block in &function.blocks {
        visitor.span(block.span)?;
        if let Some(merge) = &block.merge {
            visitor.merge(merge)?;
        }
        for (position, statement) in block.statements.iter().enumerate() {
            visitor.span(statement.span)?;
            origins(statement.diagnostic_origins, visitor)?;
            match &statement.kind {
                OwnedInstruction::PrepareBorrowRange { start, end, .. } => {
                    visitor.span(start.span)?;
                    visitor.span(end.span)?;
                }
                OwnedInstruction::Experimental { .. } | OwnedInstruction::Vector { .. } => {
                    if matches!(source, FunctionSource::Original(..)) {
                        return Err(bad());
                    }
                }
                OwnedInstruction::WriteStdout { .. } => {
                    if builtin != Some(BuiltinFunction::WriteStdout) {
                        return Err(bad());
                    }
                }
                OwnedInstruction::ReadStdin { .. } => {
                    if builtin != Some(BuiltinFunction::ReadStdin) {
                        return Err(bad());
                    }
                }
                OwnedInstruction::DestructureRecord { .. } => {
                    if matches!(source, FunctionSource::Synthetic) {
                        return Err(bad());
                    }
                }
                OwnedInstruction::ConstructEnum { payload, .. } => {
                    if !allow_enums {
                        return Err(bad());
                    }
                    if let Some(payload) = payload {
                        visitor.span(payload.span)?;
                    }
                }
                OwnedInstruction::ConsumeVariant { .. }
                | OwnedInstruction::ConstructOwnedEnum { .. }
                | OwnedInstruction::ConsumeOwnedVariant { .. } => {
                    if !allow_enums {
                        return Err(bad());
                    }
                }
                OwnedInstruction::Scalar(statement) => {
                    visitor.statement(statement)?;
                    if matches!(source, FunctionSource::Synthetic)
                        && matches!(
                            statement,
                            Statement::Assign(Assign {
                                value: Rvalue::CheckedI32ToU8 { .. }
                                    | Rvalue::U8ToI32 { .. }
                                    | Rvalue::WidenI64 { .. }
                                    | Rvalue::CheckedI64ToI32 { .. }
                                    | Rvalue::CheckedI64ToU8 { .. }
                                    | Rvalue::CheckedIntegerCast { .. },
                                ..
                            })
                        )
                    {
                        return Err(bad());
                    }
                    if let (
                        FunctionSource::Original(ast, owner, sources, owner_slot),
                        Statement::Assign(assign),
                    ) = (source, statement)
                    {
                        let previous = position
                            .checked_sub(1)
                            .and_then(|position| block.statements.get(position))
                            .and_then(|instruction| match &instruction.kind {
                                OwnedInstruction::Scalar(statement) => Some(statement),
                                _ => None,
                            });
                        authenticate_conversion(
                            ast,
                            owner,
                            assign,
                            previous,
                            &function.locals,
                            &function.places,
                            sources,
                            visitor,
                            seen,
                            owner_slot,
                        )?;
                    }
                }
                OwnedInstruction::Construct { fields, .. } => {
                    for (_, operand) in fields {
                        visitor.span(operand.span)?;
                    }
                }
                OwnedInstruction::ConstructComposite { fields, .. } => {
                    for (_, value) in fields {
                        if let FieldInitializer::Scalar(operand) = value {
                            visitor.span(operand.span)?;
                        }
                    }
                }
                OwnedInstruction::ReadProjection { index, .. } => {
                    if let Some(index) = index {
                        visitor.span(index.span)?;
                    }
                }
                OwnedInstruction::WriteProjection { index, value, .. } => {
                    if let Some(index) = index {
                        visitor.span(index.span)?;
                    }
                    visitor.span(value.span)?;
                }
                OwnedInstruction::ProjectionLength { .. } => (),
                OwnedInstruction::AcquireLocalBorrow(_)
                | OwnedInstruction::ReleaseLocalBorrow(_) => {
                    if matches!(source, FunctionSource::Synthetic) {
                        return Err(bad());
                    }
                }
                OwnedInstruction::ConstructArray { elements, .. } => {
                    for operand in elements {
                        visitor.span(operand.span)?;
                    }
                }
                OwnedInstruction::ReadIndex { index, .. } => visitor.span(index.span)?,
                OwnedInstruction::WriteIndex { value, index, .. } => {
                    visitor.span(value.span)?;
                    visitor.span(index.span)?;
                }
                OwnedInstruction::WriteField { value, .. }
                | OwnedInstruction::PrepareScalar { value, .. } => visitor.span(value.span)?,
                OwnedInstruction::StorageLive(_)
                | OwnedInstruction::StorageEnd(_)
                | OwnedInstruction::CopyInitialize { .. }
                | OwnedInstruction::MoveInitialize { .. }
                | OwnedInstruction::Replace { .. }
                | OwnedInstruction::Discard(_)
                | OwnedInstruction::ReadField { .. }
                | OwnedInstruction::ArrayLength { .. }
                | OwnedInstruction::OpenCall(_)
                | OwnedInstruction::PrepareOwned { .. }
                | OwnedInstruction::PrepareBorrow { .. } => (),
            }
        }
        if let Some(terminator) = &block.terminator {
            visitor.span(terminator.span)?;
            origins(terminator.diagnostic_origins, visitor)?;
            match &terminator.kind {
                OwnedTerminatorKind::MatchDispatch { .. } => {
                    if !allow_enums {
                        return Err(bad());
                    }
                }
                OwnedTerminatorKind::Branch { condition, .. } => visitor.span(condition.span)?,
                OwnedTerminatorKind::ReturnScalar(operand) => visitor.span(operand.span)?,
                OwnedTerminatorKind::Goto(_)
                | OwnedTerminatorKind::Invoke { .. }
                | OwnedTerminatorKind::ReturnOwned(_) => (),
            }
        }
    }
    Ok(())
}
/// Non-witness structural observation. Production must use `associate`, which
/// derives and admits the complete containing typed-source storage envelope.
pub(super) fn check(
    raw: &RawOwnedProgram,
    index: &DeclarationIndex<'_>,
    sources: &SourceMap,
) -> Result<BindUsage, Box<Diagnostic>> {
    // Reuse the same provenance helper/result as fresh typing, after that
    // invocation has ended. Candidate-marked indices stay on the private seam.
    index.require_current_source_pipeline()?;
    check_impl(
        raw,
        index,
        sources,
        index.enum_count() != 0,
        index.builtin_set() != BuiltinOrigins::None,
        None,
    )
}

/// Private qualification also accepts intentionally candidate-marked indices.
#[cfg(test)]
pub(super) fn check_enum_candidate(
    raw: &RawOwnedProgram,
    index: &DeclarationIndex<'_>,
    sources: &SourceMap,
) -> Result<BindUsage, Box<Diagnostic>> {
    index.require_no_builtin_candidate()?;
    check_impl(raw, index, sources, true, false, None)
}

/// Import-derived identity is checked separately from the raw descriptor proof.
/// This entry retains the private marker even after current-source activation.
#[cfg(test)]
pub(super) fn check_builtin_candidate(
    raw: &RawOwnedProgram,
    index: &DeclarationIndex<'_>,
    sources: &SourceMap,
) -> Result<BindUsage, Box<Diagnostic>> {
    index.require_builtin_candidate_pipeline()?;
    check_impl(raw, index, sources, true, true, None)
}

// Only builtin identity/anchor transports, not inherited Visitor internals.
// Selectors, iterator/next, checked-rank results, projected-ID results, receiver
// borrows and retained first-function identity are separate roles: no lifetime reuse.
// The private caller pays this complete envelope before source lowering.
#[allow(dead_code)]
struct BuiltinAssociationCarriers {
    output_origin_guard: OutputOriginGuardCarriers,
    ids_return: Result<builtins::BuiltinIds, OwnedFailure>,
    normalized_ids: Result<builtins::BuiltinIds, Box<Diagnostic>>,
    ids: builtins::BuiltinIds,
    enum_id: EnumId,
    function_id: hir::DefId,
    enum_lookup: Result<EnumId, Box<Diagnostic>>,
    function_lookup: Result<hir::DefId, Box<Diagnostic>>,
    anchor_lookup: Result<Span, Box<Diagnostic>>,
    enum_row: Option<&'static RawEnumDecl>,
    function_row: Option<&'static RawOwnedFunction>,
    enum_row_return: Result<&'static RawEnumDecl, Box<Diagnostic>>,
    function_row_return: Result<&'static RawOwnedFunction, Box<Diagnostic>>,
    set: BuiltinOrigins,
    iteration: std::iter::Enumerate<std::slice::Iter<'static, RawOwnedFunction>>,
    enumeration_kind: BuiltinEnum,
    function_kind: BuiltinFunction,
    enumeration_rank: Option<usize>,
    function_rank: Option<usize>,
    family_iteration: std::array::IntoIter<BuiltinEnum, 2>,
    next_family: Option<BuiltinEnum>,
    enumeration_id_return: Option<EnumId>,
    function_id_return: Option<hir::DefId>,
    first_function: Option<hir::DefId>,
    enumeration_ids_borrow: &'static builtins::BuiltinIds,
    function_ids_borrow: &'static builtins::BuiltinIds,
    // The old single retained function slot now holds the first admitted
    // builtin function. Only builtin inventories enter the finite classifier;
    // the source walk's old bool parameter becomes a one-byte exact-kind option.
    first_function_kind: BuiltinFunction,
    first_function_set_receiver: BuiltinOrigins,
    first_function_selector: BuiltinFunction,
    first_function_lookup_kind: BuiltinFunction,
    first_function_has_input: bool,
    classifier_set: BuiltinOrigins,
    classifier_first: hir::DefId,
    classifier_current: hir::DefId,
    classifier_rank: usize,
    classifier_rank_return: Option<usize>,
    classifier_input: bool,
    classifier_output: bool,
    classifier_set_receivers: [BuiltinOrigins; 2],
    classifier_kind_arguments: [BuiltinFunction; 2],
    classifier_choice: (bool, bool, usize),
    classifier_return: Option<BuiltinFunction>,
    count_permission: Option<BuiltinFunction>,
    validation_permission: Option<BuiltinFunction>,
    function_permission: Option<BuiltinFunction>,
}
#[allow(dead_code)]
struct OutputOriginGuardCarriers {
    index_receiver: &'static DeclarationIndex<'static>,
    source_inventory: BuiltinOrigins,
    raw_inventory: BuiltinOrigins,
    source_has_output: bool,
    raw_has_output: bool,
    current_receiver: &'static DeclarationIndex<'static>,
    current_result: bool,
    output_receiver: &'static DeclarationIndex<'static>,
    output_result: bool,
    origin_allowed: bool,
    rejected: bool,
}
pub(super) const fn builtin_carrier_bytes() -> usize {
    std::mem::size_of::<BuiltinAssociationCarriers>()
}

#[cfg(test)]
#[path = "output_lower_tests.rs"]
mod output_lower_tests;

/// The descriptor and index identity checks already establish independent
/// canonical function ranks. Classify only that at-most-two-function suffix.
/// This does not grant any ordinary source function an opcode permission.
fn builtin_kind(
    set: BuiltinOrigins,
    first: hir::DefId,
    current: hir::DefId,
) -> Option<BuiltinFunction> {
    let rank = current.0.checked_sub(first.0)?;
    let input = set.contains_function(BuiltinFunction::ReadStdin);
    let output = set.contains_function(BuiltinFunction::WriteStdout);
    match (input, output, rank) {
        (true, _, 0) => Some(BuiltinFunction::ReadStdin),
        (false, true, 0) | (true, true, 1) => Some(BuiltinFunction::WriteStdout),
        _ => None,
    }
}

fn check_impl(
    raw: &RawOwnedProgram,
    index: &DeclarationIndex<'_>,
    sources: &SourceMap,
    allow_enums: bool,
    allow_builtins: bool,
    typed: Option<&super::typeck::TypedOwnedProgram<'_>>,
) -> Result<BindUsage, Box<Diagnostic>> {
    let expected_vectors = index.vector_functions();
    if raw.vector_functions.len() != expected_vectors.len()
        || raw.vector_functions.capacity() != expected_vectors.len()
    {
        return Err(bad());
    }
    if !expected_vectors.is_empty() {
        let typed = typed.ok_or_else(bad)?;
        if !allow_builtins || !index.has_generic_instances() {
            return Err(bad());
        }
        for (actual, expected) in raw.vector_functions.iter().zip(expected_vectors) {
            typed
                .work()
                .debit(1, index.sources().eof(), "vector raw source key identity")?;
            if actual != expected {
                return Err(bad());
            }
        }
        // A retained table is not its own source authority. Rebuild from the
        // immutable authenticated templates before trusting its concrete keys.
        index.validate_generic_source(typed.work())?;
    }
    // Public association already requires Current. The shared helper retains
    // an independent exact-origin check for output claims; the older private
    // input candidate may never acquire output merely by changing raw shapes.
    if (raw.builtins.has_output() || index.builtin_set().has_output())
        && (!allow_builtins
            || !(index.is_current_source_pipeline() || {
                #[cfg(test)]
                {
                    index.is_output_candidate_pipeline()
                }
                #[cfg(not(test))]
                {
                    false
                }
            }))
    {
        return Err(bad());
    }
    let first_function = if allow_builtins {
        if raw.builtins != index.builtin_set() {
            return Err(bad());
        }
        let ids = builtins::check(raw).map_err(|_| bad())?;
        // Enum and function suffixes have independent family ranks. A status
        // without a function must never shift another family's function ID.
        for enumeration_kind in BuiltinEnum::ALL {
            let function_kind = match enumeration_kind {
                BuiltinEnum::ReadStatus => BuiltinFunction::ReadStdin,
                BuiltinEnum::WriteStatus => BuiltinFunction::WriteStdout,
            };
            if let Some(id) = ids.enumeration(enumeration_kind) {
                if id != index.builtin_enum_id(enumeration_kind).map_err(|_| bad())?
                    || raw.enums.get(id.0).ok_or_else(bad)?.span
                        != index
                            .builtin_enum_anchor(enumeration_kind)
                            .map_err(|_| bad())?
                {
                    return Err(bad());
                }
            }
            if let Some(id) = ids.function(function_kind) {
                if id
                    != index
                        .builtin_function_id(function_kind)
                        .map_err(|_| bad())?
                    || raw.functions.get(id.0).ok_or_else(bad)?.span
                        != index
                            .builtin_function_anchor(function_kind)
                            .map_err(|_| bad())?
                {
                    return Err(bad());
                }
            }
        }
        for kind in crate::frontend::builtin_catalog::experimental::Enumeration::ALL {
            if let Some(id) = ids.experimental_enum(kind) {
                if id != index.experimental_enum_id(kind).map_err(|_| bad())?
                    || raw.enums.get(id.0).ok_or_else(bad)?.span
                        != index.experimental_anchor(kind.item()).map_err(|_| bad())?
                {
                    return Err(bad());
                }
            }
        }
        for kind in crate::frontend::builtin_catalog::experimental::Function::ALL {
            if let Some(id) = ids.experimental_function(kind) {
                if id != index.experimental_function_id(kind).map_err(|_| bad())?
                    || raw.functions.get(id.0).ok_or_else(bad)?.span
                        != index.experimental_anchor(kind.item()).map_err(|_| bad())?
                {
                    return Err(bad());
                }
            }
        }
        if !expected_vectors.is_empty() {
            let typed = typed.ok_or_else(bad)?;
            for (rank, key) in expected_vectors.iter().copied().enumerate() {
                let (expected, anchor) =
                    super::vector_source::function(index, typed.work(), rank, key)?;
                let actual = ids.vector_function_at(rank).ok_or_else(bad)?;
                if actual != expected || raw.functions.get(actual.0).ok_or_else(bad)?.span != anchor
                {
                    return Err(bad());
                }
            }
        }
        let first_kind = if raw.builtins.contains_function(BuiltinFunction::ReadStdin) {
            BuiltinFunction::ReadStdin
        } else {
            BuiltinFunction::WriteStdout
        };
        ids.function(first_kind)
    } else {
        raw.builtins.require_none().map_err(|_| bad())?;
        if index.builtin_set() != BuiltinOrigins::None {
            return Err(bad());
        }
        None
    };
    if !allow_enums && (!raw.enums.is_empty() || index.enum_count() != 0) {
        return Err(bad());
    }
    let mut count = Visitor::count();
    for _ in &raw.vector_functions {
        count.declaration()?;
    }
    for declaration in &raw.enums {
        enumeration(declaration, &mut count)?;
    }
    for declaration in &raw.records {
        record(declaration, &mut count)?;
    }
    for (ordinal, declaration) in raw.functions.iter().enumerate() {
        function(
            declaration,
            &mut count,
            allow_enums,
            if let Some(first) = first_function {
                builtin_kind(raw.builtins, first, hir::DefId(ordinal))
            } else {
                None
            },
            FunctionSource::Count,
            &mut ConversionSeen::empty(),
        )?;
    }
    let mut visitor = Visitor::validate(sources);
    if !raw.vector_functions.is_empty() {
        visitor.dimension(raw.vector_functions.len(), expected_vectors.len())?;
        for _ in &raw.vector_functions {
            visitor.declaration()?;
        }
    }
    let mut seen = if count.has_conversions() {
        let owners = index.sources().count();
        let mut expressions = 0usize;
        for owner in 0..owners {
            visitor.dimension(1, 1)?;
            let extent = index
                .sources()
                .try_ast_borrowed(crate::frontend::project::ModuleId(owner))
                .ok_or_else(bad)?
                .expressions
                .len();
            expressions = expressions.checked_add(extent).ok_or_else(bad)?;
        }
        let requested = conversion_seen_bytes(owners, expressions)?;
        if let Some(typed) = typed {
            let seed = typed.conversion_association_storage_bytes()?;
            // The fixed RFC0030 bank already paid the shared authentication
            // helper. Only tracker/header/reservation overlap is additional.
            let additional = requested
                .checked_sub(conversion_carrier_bytes())
                .ok_or_else(bad)?;
            super::budget::admit_conversion_scratch(seed, additional)
                .map_err(|e| super::diagnostic::lower(&e, sources))?;
        }
        #[cfg(test)]
        CONVERSION_RESERVE_ENTRIES.with(|entries| entries.set(entries.get() + 1));
        ConversionSeen::new(ConversionOwners::Indexed(index.sources()), &mut visitor)?
    } else {
        ConversionSeen::empty()
    };
    // The ordinary zero-enum path keeps its historical dimension/work count.
    if allow_enums {
        visitor.dimension(raw.enums.len(), index.enum_count())?;
    }
    for (ordinal, declaration) in raw.enums.iter().enumerate() {
        let id = EnumId(ordinal);
        let original = index.enum_view(id).map_err(|_| bad())?;
        if declaration.id != id || declaration.span != original.diagnostic_span() {
            return Err(bad());
        }
        visitor.dimension(declaration.variants.len(), original.variant_count())?;
        for (index, variant) in declaration.variants.iter().enumerate() {
            let id = VariantId {
                enumeration: id,
                index,
            };
            let expected = original.variant(id).map_err(|_| bad())?;
            if variant.id != id
                || variant.span != expected.diagnostic_span()
                || variant.payload
                    != if expected.has_owned_payload() {
                        expected
                            .value_payload(typed.ok_or_else(bad)?.work())?
                            .map(ParameterTy::Value)
                    } else {
                        expected
                            .payload()
                            .map(|ty| ParameterTy::Value(ValueTy::Scalar(ty)))
                    }
            {
                return Err(bad());
            }
        }
        visitor.file(original.diagnostic_span().file);
        enumeration(declaration, &mut visitor)?;
    }
    visitor.dimension(raw.records.len(), index.record_count())?;
    visitor.dimension(raw.functions.len(), index.function_count())?;
    // New enum fields require exact source types even in nongeneric, unused
    // records. Compare the entire field table: checking only the raw enum case
    // would let a changed raw scalar or record type evade authentication.
    let check_field_types = index.has_generic_instances()
        || typed.is_some_and(|program| super::record_enum_layouts::needed(program.records()));
    for (ordinal, declaration) in raw.records.iter().enumerate() {
        let id = RecordId(ordinal);
        let (key, module) = index.record(id).map_err(|_| bad())?;
        let ast = index.sources().ast(module).map_err(|_| bad())?;
        let original = ast.records.get(key.index).ok_or_else(bad)?;
        if declaration.id != id
            || declaration.span != original.name
            || declaration.is_copy != index.record_is_copy(id).map_err(|_| bad())?
        {
            return Err(bad());
        }
        visitor.dimension(declaration.fields.len(), original.fields.len())?;
        for (field_index, (field, original)) in
            declaration.fields.iter().zip(&original.fields).enumerate()
        {
            if field.id
                != (FieldId {
                    record: id,
                    index: field_index,
                })
                || field.span != original.name
            {
                return Err(bad());
            }
            if check_field_types {
                let typed = typed.ok_or_else(bad)?;
                if field.ty
                    != index.instantiated_record_field(
                        FieldId {
                            record: id,
                            index: field_index,
                        },
                        original.ty,
                        typed.work(),
                    )?
                {
                    return Err(bad());
                }
            }
        }
        visitor.file(original.name.file);
        record(declaration, &mut visitor)?;
    }
    for (ordinal, declaration) in raw.functions.iter().enumerate() {
        let id = hir::DefId(ordinal);
        if ordinal >= index.source_function_count() {
            if !allow_builtins || declaration.id != id {
                return Err(bad());
            }
            visitor.file(declaration.span.file);
            function(
                declaration,
                &mut visitor,
                allow_enums,
                if let Some(first) = first_function {
                    builtin_kind(raw.builtins, first, id)
                } else {
                    None
                },
                FunctionSource::Synthetic,
                &mut seen,
            )?;
            continue;
        }
        let (key, module) = index.function(id).map_err(|_| bad())?;
        let ast = index.sources().ast(module).map_err(|_| bad())?;
        let original = ast.functions.get(key.index).ok_or_else(bad)?;
        if declaration.id != id || declaration.span != original.name {
            return Err(bad());
        }
        if index.has_generic_instances() {
            let typed = typed.ok_or_else(bad)?;
            if declaration.result != index.instantiated_value(id, original.result, typed.work())? {
                return Err(bad());
            }
            if declaration.parameters.len() != original.params.len() {
                return Err(bad());
            }
            for (binding, parameter) in declaration.parameters.iter().zip(&original.params) {
                let actual = match *binding {
                    ParameterBinding::Scalar(local) => ParameterTy::Value(ValueTy::Scalar(
                        declaration.locals.get(local.0).ok_or_else(bad)?.ty,
                    )),
                    ParameterBinding::Owned(place) => {
                        let owner = declaration.owners.get(place.0).ok_or_else(bad)?;
                        ParameterTy::Value(ValueTy::Owned(owner.aggregate()))
                    }
                    ParameterBinding::Reference(reference) => {
                        let reference = declaration.references.get(reference.0).ok_or_else(bad)?;
                        ParameterTy::Reference {
                            referent: reference.referent(),
                            kind: reference.kind,
                        }
                    }
                };
                if actual != index.instantiated_parameter(id, parameter.ty, typed.work())? {
                    return Err(bad());
                }
            }
            for call in &declaration.calls {
                if call.target != index.expected_generic_call(id, call.span, typed.work())? {
                    return Err(bad());
                }
            }
            for block in &declaration.blocks {
                for statement in &block.statements {
                    let (destination, variant) = match statement.kind {
                        OwnedInstruction::Construct { destination, .. }
                        | OwnedInstruction::ConstructComposite { destination, .. } => {
                            (destination, None)
                        }
                        OwnedInstruction::ConstructEnum {
                            destination,
                            variant,
                            ..
                        }
                        | OwnedInstruction::ConstructOwnedEnum {
                            destination,
                            variant,
                            ..
                        } => (destination, Some(variant)),
                        _ => continue,
                    };
                    let (expected, expected_variant) =
                        index.expected_generic_construction(id, statement.span, typed.work())?;
                    let actual = ValueTy::Owned(
                        declaration
                            .owners
                            .get(destination.0)
                            .ok_or_else(bad)?
                            .aggregate(),
                    );
                    if actual != expected || variant != expected_variant {
                        return Err(bad());
                    }
                }
            }
            for descriptor in &declaration.matches {
                let enumeration =
                    index.expected_generic_match(id, descriptor.span, typed.work())?;
                if declaration
                    .owners
                    .get(descriptor.source.0)
                    .ok_or_else(bad)?
                    .aggregate()
                    != AggregateTy::Enum(enumeration)
                {
                    return Err(bad());
                }
                let original = original
                    .blocks
                    .iter()
                    .flat_map(|block| &block.body)
                    .find(|statement| statement.span == descriptor.span)
                    .ok_or_else(bad)?;
                let crate::frontend::ast::StmtKind::Match { arms, .. } = &original.kind else {
                    return Err(bad());
                };
                if arms.len() != descriptor.arms.len() {
                    return Err(bad());
                }
                for (source, actual) in arms.iter().zip(&descriptor.arms) {
                    let template = index.query(typed.work()).variant(
                        module,
                        crate::frontend::project::QualifiedPathRef {
                            file: source.span.file,
                            path: source.variant,
                        },
                    )?;
                    let expected = index.instantiated_match_variant(id, source.span, template)?;
                    if actual.variant != expected {
                        return Err(bad());
                    }
                }
            }
        }
        for block in &declaration.blocks {
            for statement in &block.statements {
                let OwnedInstruction::DestructureRecord { source, ref fields } = statement.kind
                else {
                    continue;
                };
                let typed = typed.ok_or_else(bad)?;
                let (record, pattern, initializer) =
                    index.source_record_pattern(id, statement.span, typed.work())?;
                let source_decl = declaration.owners.get(source.0).ok_or_else(bad)?;
                if source_decl.aggregate() != AggregateTy::Record(record)
                    || source_decl.span != initializer
                    || !matches!(
                        source_decl.kind,
                        OwnerKind::Temporary | OwnerKind::CallResult { .. }
                    )
                {
                    return Err(bad());
                }
                let (record_key, record_module) = index.record(record)?;
                let record_ast = &index.sources().ast(record_module)?.records[record_key.index];
                let mut owned = 0usize;
                for field in pattern {
                    typed.work().debit(
                        record_ast.fields.len() as u64 + 1,
                        field.span,
                        "record pattern field correspondence",
                    )?;
                    let ordinal = record_ast
                        .fields
                        .iter()
                        .position(|member| {
                            index.sources().text(member.name).ok()
                                == index.sources().text(field.field).ok()
                        })
                        .ok_or_else(bad)?;
                    let expected_id = FieldId {
                        record,
                        index: ordinal,
                    };
                    let expected = index.instantiated_record_field(
                        expected_id,
                        record_ast.fields[ordinal].ty,
                        typed.work(),
                    )?;
                    match expected {
                        ParameterTy::Value(ValueTy::Owned(aggregate)) => {
                            let (actual_id, destination) = fields.get(owned).ok_or_else(bad)?;
                            let target = declaration.owners.get(destination.0).ok_or_else(bad)?;
                            if *actual_id != expected_id
                                || target.aggregate() != aggregate
                                || target.span != field.binding
                                || target.kind != (OwnerKind::Local { mutable: false })
                            {
                                return Err(bad());
                            }
                            owned += 1;
                        }
                        ParameterTy::Value(ValueTy::Scalar(ty)) => {
                            let mut matches = 0usize;
                            for raw in declaration
                                .blocks
                                .iter()
                                .flat_map(|block| &block.statements)
                            {
                                typed.work().debit(
                                    1,
                                    field.span,
                                    "record pattern scalar correspondence",
                                )?;
                                if raw.span != field.span {
                                    continue;
                                }
                                if let OwnedInstruction::ReadField {
                                    destination,
                                    base: AccessBase::Owner(actual_source),
                                    field: actual_field,
                                } = raw.kind
                                {
                                    let destination =
                                        declaration.locals.get(destination.0).ok_or_else(bad)?;
                                    if actual_source != source
                                        || actual_field != expected_id
                                        || destination.ty != ty
                                        || destination.span != field.binding
                                        || destination.kind != LocalKind::Binding
                                    {
                                        return Err(bad());
                                    }
                                    matches += 1;
                                }
                            }
                            if matches != 1 {
                                return Err(bad());
                            }
                        }
                        _ => return Err(bad()),
                    }
                }
                if owned != fields.len() {
                    return Err(bad());
                }
            }
        }
        let mut local_declarations = 0usize;
        for statement in original.blocks.iter().flat_map(|block| &block.body) {
            if let crate::frontend::ast::StmtKind::Let { init, .. } = statement.kind {
                if let crate::frontend::ast::ExprKind::Borrow { .. } = ast.expressions[init.0].kind
                {
                    local_declarations = local_declarations.checked_add(1).ok_or_else(bad)?;
                    let typed = typed.ok_or_else(bad)?;
                    let view = typed.functions().nth(ordinal).ok_or_else(bad)?;
                    let borrow_span = ast.expressions[init.0].span;
                    let mut found = 0usize;
                    for (loan_id, loan) in declaration.loans.iter().enumerate() {
                        typed.work().debit(
                            1,
                            borrow_span,
                            "local reference source correspondence",
                        )?;
                        if loan.region != LoanRegion::Local || loan.span != borrow_span {
                            continue;
                        }
                        found += 1;
                        typed.work().debit(
                            u64::try_from(view.hir().blocks.len()).map_err(|_| bad())?,
                            borrow_span,
                            "local reference block inventory",
                        )?;
                        let rows = view
                            .hir()
                            .blocks
                            .iter()
                            .try_fold(0usize, |sum, block| sum.checked_add(block.body.len()))
                            .ok_or_else(bad)?;
                        let scans = rows
                            .checked_mul(2)
                            .and_then(|sum| sum.checked_add(1))
                            .ok_or_else(bad)?;
                        typed.work().debit(
                            u64::try_from(scans).map_err(|_| bad())?,
                            borrow_span,
                            "local reference declaration lookup",
                        )?;
                        let source_binding = view
                            .hir()
                            .blocks
                            .iter()
                            .flat_map(|block| &block.body)
                            .find_map(|row| {
                                if let super::hir::StmtKind::BorrowLet {
                                    binding,
                                    kind,
                                    place,
                                    borrow_span: origin,
                                    ..
                                } = row.kind
                                {
                                    (origin == borrow_span).then_some((binding, kind, place))
                                } else {
                                    None
                                }
                            })
                            .ok_or_else(bad)?;
                        let (binding, kind, place) = source_binding;
                        if view.binding_ty(binding)
                            != (ParameterTy::Reference {
                                referent: loan.referent(),
                                kind: loan.kind,
                            })
                            || kind != loan.kind
                            || !loan.projection.is_empty()
                        {
                            return Err(bad());
                        }
                        let root = match place {
                            super::hir::BorrowPlace::Owner(id)
                            | super::hir::BorrowPlace::Forwarded(id) => id,
                        };
                        let origin = view.hir().bindings[root.0].span;
                        match loan.authority {
                            AccessBase::Owner(owner)
                                if declaration
                                    .owners
                                    .get(owner.0)
                                    .is_some_and(|row| row.span == origin) => {}
                            AccessBase::Parameter(parameter)
                                if declaration
                                    .references
                                    .get(parameter.0)
                                    .is_some_and(|row| row.span == origin) => {}
                            AccessBase::Loan(parent) => {
                                if parent.0 == loan_id {
                                    return Err(bad());
                                }
                                let parent = declaration.loans.get(parent.0).ok_or_else(bad)?;
                                let binding = view
                                    .hir()
                                    .blocks
                                    .iter()
                                    .flat_map(|block| &block.body)
                                    .find_map(|row| {
                                        if let super::hir::StmtKind::BorrowLet {
                                            binding,
                                            borrow_span,
                                            ..
                                        } = row.kind
                                        {
                                            (binding == root).then_some(borrow_span)
                                        } else {
                                            None
                                        }
                                    })
                                    .ok_or_else(bad)?;
                                if parent.region != LoanRegion::Local || parent.span != binding {
                                    return Err(bad());
                                }
                            }
                            _ => return Err(bad()),
                        }
                    }
                    if found != 1 {
                        return Err(bad());
                    }
                }
            }
            if !matches!(
                statement.kind,
                crate::frontend::ast::StmtKind::DestructureRecord { .. }
            ) {
                continue;
            }
            let typed = typed.ok_or_else(bad)?;
            let mut count = 0usize;
            for lowered in declaration
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
            {
                typed
                    .work()
                    .debit(1, statement.span, "record pattern statement correspondence")?;
                count += usize::from(
                    lowered.span == statement.span
                        && matches!(lowered.kind, OwnedInstruction::DestructureRecord { .. }),
                );
            }
            if count != 1 {
                return Err(bad());
            }
        }
        if declaration
            .loans
            .iter()
            .filter(|loan| loan.region == LoanRegion::Local)
            .count()
            != local_declarations
        {
            return Err(bad());
        }
        visitor.file(original.name.file);
        function(
            declaration,
            &mut visitor,
            allow_enums,
            None,
            FunctionSource::Original(ast, original, sources, module.0),
            &mut seen,
        )?;
    }
    visitor.finish(count)
}

#[cfg(test)]
mod generic_source_tests {
    use super::*;
    use crate::frontend::{
        declaration_index::{collect_originals, IndexLimits, SourceOwner, WorkMeter},
        project::{budget::Allocator, ProjectLimits, ProjectSources},
    };

    #[test]
    fn nominal_same_layout_identity_swaps_fail_source_association() {
        use crate::frontend::oir::owned_types::AggregateSlot;
        for (kind, source) in [
            ("record", "struct Marker<T>{value:i32}fn main()->i32{let first=Marker::<i32>{value:1};let second=Marker::<bool>{value:2};return first.value+second.value;}"),
            ("enum", "enum Marker<T>{Empty,Value(i32)}fn main()->i32{let first=Marker::<i32>::Value(1);let second=Marker::<bool>::Value(2);match first{Marker::Empty=>{return 0;},Marker::Value(value)=>{},}match second{Marker::Empty=>{return 0;},Marker::Value(value)=>{return value;},}}"),
        ] {
            let path=std::env::temp_dir().join(format!("oxid-nominal-auth-{kind}-{}.ox",std::process::id()));
            std::fs::write(&path,source).unwrap();
            let project=ProjectSources::load_typed(path.to_str().unwrap(),ProjectLimits::default()).unwrap();
            std::fs::remove_file(path).unwrap();
            let work=WorkMeter::default();
            let mut allocator=Allocator::default();
            let index=collect_originals(SourceOwner::project(&project),IndexLimits::default(),&work,&mut allocator).unwrap().finish(&work,&mut allocator).unwrap();
            let typed=if kind=="enum" {
                super::super::resolve::type_enum_source(&index,&work,&mut allocator).unwrap()
            } else {
                super::super::typeck::check(super::super::resolve::resolve_project(&index,&work).unwrap()).unwrap()
            };
            assert!(associate_lowered(super::super::lower::lower(&typed).unwrap(),&typed).is_ok());
            for independent in [true,false] {
                let mut raw=super::super::lower::lower(&typed).unwrap();
                assert_eq!(if kind=="record" {raw.records.len()} else {raw.enums.len()},2);
                let swap_record=|field:&mut FieldId|{assert!(field.record.0<2);field.record.0=1-field.record.0;};
                for function in &mut raw.functions {
                    for owner in &mut function.owners {
                        let aggregate=match owner.aggregate() {
                            AggregateTy::Record(RecordId(id)) if kind=="record"=>AggregateTy::Record(RecordId(1-id)),
                            AggregateTy::Enum(EnumId(id)) if kind=="enum"=>AggregateTy::Enum(EnumId(1-id)),
                            other=>other,
                        };
                        owner.aggregate=AggregateSlot::try_from_aggregate(aggregate).unwrap();
                    }
                    for descriptor in &mut function.matches {
                        for arm in &mut descriptor.arms {arm.variant.enumeration.0=1-arm.variant.enumeration.0;}
                    }
                    for block in &mut function.blocks {
                        for statement in &mut block.statements {
                            match &mut statement.kind {
                                OwnedInstruction::Construct {fields,..}=>{for (field,_) in fields {swap_record(field);}},
                                OwnedInstruction::ConstructComposite {fields,..}=>{for (field,_) in fields {swap_record(field);}},
                                OwnedInstruction::ReadField {field,..}=>swap_record(field),
                                OwnedInstruction::ReadProjection {path,..}|OwnedInstruction::WriteProjection {path,..}|OwnedInstruction::ProjectionLength {path,..}=>{for field in path {swap_record(field);}},
                                OwnedInstruction::ConstructEnum {variant,..}|OwnedInstruction::ConstructOwnedEnum {variant,..}=>{variant.enumeration.0=1-variant.enumeration.0;},
                                _=>(),
                            }
                        }
                    }
                }
                if independent {
                    assert!(super::super::super::verified::verify_owned(raw,project.sources()).is_ok());
                } else {assert!(associate_lowered(raw,&typed).is_err());}
            }
        }
    }

    #[test]
    fn inferred_constructor_nominals_cannot_be_replaced_by_equal_layouts() {
        use super::super::record_pattern_tests::with_typed;
        use crate::frontend::oir::owned_types::AggregateSlot;
        for (kind, text) in [
            ("record", "struct Left{value:i32}struct Right{value:i32}struct Cell<T>{value:T}fn main()->i32{let first=Cell{value:Left{value:20}};let second=Cell{value:Right{value:22}};return first.value.value+second.value.value;}"),
            ("enum", "struct Left{value:i32}struct Right{value:i32}enum Choice<T>{Empty,Some(T)}fn main()->i32{let first=Choice::Some(Left{value:20});let second=Choice::Some(Right{value:22});match first{Choice::Empty=>{return 0;},Choice::Some(value)=>{},}match second{Choice::Empty=>{return 0;},Choice::Some(value)=>{return value.value;},}}"),
        ] {
            with_typed(text, |typed,sources| {
                let valid=super::super::lower::lower(typed).unwrap();
                assert_eq!(valid.records.len(),if kind=="record" {4} else {2});
                assert_eq!(valid.enums.len(),if kind=="record" {0} else {2});
                for id in 0..valid.records.len() {
                    let template=typed.index().template_record_for_instance(RecordId(id)).unwrap();
                    assert_eq!(template.0,if id<2 {id} else {2});
                }
                assert!(associate_lowered(valid,typed).is_ok());
                for independent in [true,false] {
                    let mut raw=super::super::lower::lower(typed).unwrap();
                    let swap_record=|field:&mut FieldId| {field.record.0^=1;};
                    for function in &mut raw.functions {
                        for owner in &mut function.owners {
                            owner.aggregate=AggregateSlot::try_from_aggregate(match owner.aggregate() {
                                AggregateTy::Record(RecordId(id))=>AggregateTy::Record(RecordId(id^1)),
                                AggregateTy::Enum(EnumId(id))=>AggregateTy::Enum(EnumId(id^1)),
                                other=>other,
                            }).unwrap();
                        }
                        for descriptor in &mut function.matches {
                            for arm in &mut descriptor.arms {arm.variant.enumeration.0^=1;}
                        }
                        for block in &mut function.blocks {
                            for statement in &mut block.statements {
                                match &mut statement.kind {
                                    OwnedInstruction::Construct{fields,..}=>{
                                        for (field,_) in fields {swap_record(field);}
                                    }
                                    OwnedInstruction::ConstructComposite{fields,..}=>{
                                        for (field,_) in fields {swap_record(field);}
                                    }
                                    OwnedInstruction::ReadField{field,..}=>swap_record(field),
                                    OwnedInstruction::ReadProjection{path,..}|OwnedInstruction::WriteProjection{path,..}|OwnedInstruction::ProjectionLength{path,..}=>{
                                        for field in path {swap_record(field);}
                                    }
                                    OwnedInstruction::ConstructEnum{variant,..}|OwnedInstruction::ConstructOwnedEnum{variant,..}=>{variant.enumeration.0^=1;}
                                    _=>(),
                                }
                            }
                        }
                    }
                    if independent {
                        assert!(super::super::super::verified::verify_owned(raw,sources).is_ok(),"{kind}");
                    } else {assert!(associate_lowered(raw,typed).is_err(),"{kind}");}
                }
            });
        }
    }

    #[test]
    fn same_signature_generic_call_swap_is_rejected_by_source_association() {
        let path =
            std::env::temp_dir().join(format!("oxid-generic-call-auth-{}.ox", std::process::id()));
        std::fs::write(&path, "fn one<T>(value:T)->i32{return 1;}fn two<T>(value:T)->i32{return 2;}fn main()->i32{return one(42)+two(42);}").unwrap();
        let project =
            ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default()).unwrap();
        std::fs::remove_file(path).unwrap();
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
        let typed = super::super::typeck::check(
            super::super::resolve::resolve_project(&index, &work).unwrap(),
        )
        .unwrap();
        let raw = super::super::lower::lower(&typed).unwrap();
        assert!(associate_lowered(raw, &typed).is_ok());
        let mut raw = super::super::lower::lower(&typed).unwrap();
        let entry = index.root_original_main().unwrap();
        assert_eq!(raw.functions[entry.0].calls.len(), 2);
        let other = raw.functions[entry.0].calls[1].target;
        raw.functions[entry.0].calls[0].target = other;
        // The independent raw verifier accepts the identical ABI and ownership.
        assert!(super::super::super::verified::verify_owned(raw, project.sources()).is_ok());
        let mut raw = super::super::lower::lower(&typed).unwrap();
        raw.functions[entry.0].calls[0].target = other;
        assert!(associate_lowered(raw, &typed).is_err());
    }
}

/// Preserve the RFC 0025 predecessor/candidate layout evidence alongside the
/// actual closed transport. None of these model types supplies a witness.
#[cfg(test)]
#[allow(dead_code)]
mod output_layout_feasibility {
    use super::*;
    use std::mem::{align_of, size_of};

    // Preserve the predecessor's singleton fields and their declaration order.
    struct BaselineIds {
        enumeration: Option<EnumId>,
        function: Option<hir::DefId>,
    }

    // Each family is absent, status-only, or function plus required status.
    // These nine states encode dependency closure, but prove no provenance.
    enum Inventory {
        None,
        ReadStatus,
        ReadStdin,
        WriteStatus,
        ReadStatusWriteStatus,
        ReadStdinWriteStatus,
        WriteStdout,
        ReadStatusWriteStdout,
        ReadStdinWriteStdout,
    }
    struct ExplicitIds {
        read_enumeration: Option<EnumId>,
        read_function: Option<hir::DefId>,
        write_enumeration: Option<EnumId>,
        write_function: Option<hir::DefId>,
    }
    // Independent enum/function suffix bases; Inventory determines which
    // family exists and each family's rank within its own suffix. A future
    // validator must prove bounds, canonical order and source association.
    struct SuffixIds {
        enumeration_base: EnumId,
        function_base: hir::DefId,
        inventory: Inventory,
    }
    // Exact pre-vector compact inventory shape, including its enum niche.
    // Preserve the old transport receipt separately from the current extension.
    enum PreVectorInventory {
        None,
        ReadStatus,
        ReadStdin,
        WriteStatus,
        WriteStdout,
        ReadStatusWriteStatus,
        ReadStatusWriteStdout,
        ReadStdinWriteStatus,
        ReadStdinWriteStdout,
        Extended([u8; 4]),
    }
    struct PreVectorIds {
        enumeration_base: EnumId,
        function_base: hir::DefId,
        inventory: PreVectorInventory,
    }
    struct VectorSuffixIds {
        enumeration_base: EnumId,
        function_base: hir::DefId,
        inventory: BuiltinOrigins,
        vector_count: usize,
    }
    enum Family {
        Input,
        Output,
    }
    enum Enumeration {
        ReadStatus,
        WriteStatus,
    }
    enum Function {
        ReadStdin,
        WriteStdout,
    }

    // Complete predecessor envelope, in its original declaration order.
    // Candidate and current models append the named extra roles below.
    macro_rules! carriers {
        ($name:ident, $ids:ty, $inventory:ty; $($extra:tt)*) => {
            struct $name {
                ids_return: Result<$ids, OwnedFailure>,
                normalized_ids: Result<$ids, Box<Diagnostic>>,
                ids: $ids,
                enum_id: EnumId,
                function_id: hir::DefId,
                enum_lookup: Result<EnumId, Box<Diagnostic>>,
                function_lookup: Result<hir::DefId, Box<Diagnostic>>,
                anchor_lookup: Result<Span, Box<Diagnostic>>,
                enum_row: Option<&'static RawEnumDecl>,
                function_row: Option<&'static RawOwnedFunction>,
                enum_row_return: Result<&'static RawEnumDecl, Box<Diagnostic>>,
                function_row_return: Result<&'static RawOwnedFunction, Box<Diagnostic>>,
                set: $inventory,
                iteration: std::iter::Enumerate<std::slice::Iter<'static, RawOwnedFunction>>,
                $($extra)*
            }
        };
    }
    carriers!(BaselineCarriers, BaselineIds, BuiltinOrigins;);
    carriers!(ExplicitSubstitutionCarriers, ExplicitIds, Inventory;);
    carriers!(SuffixSubstitutionCarriers, SuffixIds, Inventory;);

    // Price a concrete finite-family walk separately from ID substitution.
    // All named roles coexist in this model; no padding or lifetime reuse is
    // assumed. This does not assert that future control flow needs only these.
    macro_rules! family_carriers {
        ($name:ident, $ids:ty) => {
            carriers!($name, $ids, Inventory;
                family: Family,
                enumeration: Enumeration,
                function: Function,
                enumeration_rank: Option<usize>,
                function_rank: Option<usize>,
                family_iteration: std::array::IntoIter<Family, 2>,
                next_family: Option<Family>,
            );
        };
    }
    family_carriers!(ExplicitFamilyCarriers, ExplicitIds);
    family_carriers!(SuffixFamilyCarriers, SuffixIds);
    macro_rules! closed_transport {
        ($name:ident, $ids:ty, $inventory:ty) => {
            carriers!($name, $ids, $inventory;
        enumeration_kind: BuiltinEnum,
        function_kind: BuiltinFunction,
        enumeration_rank: Option<usize>,
        function_rank: Option<usize>,
        family_iteration: std::array::IntoIter<BuiltinEnum, 2>,
        next_family: Option<BuiltinEnum>,
        enumeration_id_return: Option<EnumId>,
        function_id_return: Option<hir::DefId>,
        input_function: Option<hir::DefId>,
        enumeration_ids_borrow: &'static $ids,
        function_ids_borrow: &'static $ids,
            );
        };
    }
    closed_transport!(
        PreVectorClosedTransportCarriers,
        PreVectorIds,
        PreVectorInventory
    );
    closed_transport!(
        ClosedTransportCarriers,
        builtins::BuiltinIds,
        BuiltinOrigins
    );
    carriers!(ClosedSourceCarriers, builtins::BuiltinIds, BuiltinOrigins;
        enumeration_kind: BuiltinEnum,
        function_kind: BuiltinFunction,
        enumeration_rank: Option<usize>,
        function_rank: Option<usize>,
        family_iteration: std::array::IntoIter<BuiltinEnum, 2>,
        next_family: Option<BuiltinEnum>,
        enumeration_id_return: Option<EnumId>,
        function_id_return: Option<hir::DefId>,
        first_function: Option<hir::DefId>,
        enumeration_ids_borrow: &'static builtins::BuiltinIds,
        function_ids_borrow: &'static builtins::BuiltinIds,
        first_function_kind: BuiltinFunction,
        first_function_set_receiver: BuiltinOrigins,
        first_function_selector: BuiltinFunction,
        first_function_lookup_kind: BuiltinFunction,
        first_function_has_input: bool,
        classifier_set: BuiltinOrigins,
        classifier_first: hir::DefId,
        classifier_current: hir::DefId,
        classifier_rank: usize,
        classifier_rank_return: Option<usize>,
        classifier_input: bool,
        classifier_output: bool,
        classifier_set_receivers: [BuiltinOrigins; 2],
        classifier_kind_arguments: [BuiltinFunction; 2],
        classifier_choice: (bool, bool, usize),
        classifier_return: Option<BuiltinFunction>,
        count_permission: Option<BuiltinFunction>,
        validation_permission: Option<BuiltinFunction>,
        function_permission: Option<BuiltinFunction>,
    );

    fn same_layout<T, U>() {
        assert_eq!(size_of::<T>(), size_of::<U>());
        assert_eq!(align_of::<T>(), align_of::<U>());
    }
    fn report<T>(name: &str) {
        println!(
            "OUTPUT_ASSOCIATION_LAYOUT {name} bytes={} align={}",
            size_of::<T>(),
            align_of::<T>()
        );
    }
    fn candidate<T>(name: &str) {
        println!(
            "OUTPUT_ASSOCIATION_CANDIDATE {name} historical_carriers={} \
             candidate_carriers={} delta={} admission=NOT_ESTABLISHED",
            size_of::<BaselineCarriers>(),
            size_of::<T>(),
            size_of::<T>() as i128 - size_of::<BaselineCarriers>() as i128,
        );
    }

    #[test]
    fn bounded_stdout_association_disconnected_layout_feasibility() {
        same_layout::<SuffixIds, PreVectorIds>();
        same_layout::<VectorSuffixIds, builtins::BuiltinIds>();
        // Retain the closed predecessor as a distinct measured receipt; the
        // activated association adds its complete independent origin guard.
        assert_eq!(
            size_of::<BuiltinAssociationCarriers>(),
            size_of::<ClosedSourceCarriers>() + size_of::<OutputOriginGuardCarriers>()
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(size_of::<PreVectorClosedTransportCarriers>(), 568);
        same_layout::<Result<SuffixIds, OwnedFailure>, Result<PreVectorIds, OwnedFailure>>();
        same_layout::<Result<SuffixIds, Box<Diagnostic>>, Result<PreVectorIds, Box<Diagnostic>>>();
        same_layout::<
            Result<VectorSuffixIds, OwnedFailure>,
            Result<builtins::BuiltinIds, OwnedFailure>,
        >();
        same_layout::<
            Result<VectorSuffixIds, Box<Diagnostic>>,
            Result<builtins::BuiltinIds, Box<Diagnostic>>,
        >();
        assert_eq!(
            builtin_carrier_bytes(),
            size_of::<BuiltinAssociationCarriers>()
        );

        macro_rules! layouts {
            ($($ty:ty),+ $(,)?) => {$(report::<$ty>(stringify!($ty));)+};
        }
        macro_rules! id_transports {
            ($ids:ty) => {
                layouts!(
                    $ids,
                    Result<$ids, OwnedFailure>,
                    Result<$ids, Box<Diagnostic>>,
                );
            };
        }
        layouts!(
            EnumId,
            hir::DefId,
            Option<EnumId>,
            Option<hir::DefId>,
            OwnedFailure,
            Box<Diagnostic>,
            BuiltinOrigins,
            Inventory,
            Family,
            Enumeration,
            Function,
            Option<usize>,
            std::array::IntoIter<Family, 2>,
            Option<Family>,
            BuiltinAssociationCarriers,
            BaselineCarriers,
            ExplicitSubstitutionCarriers,
            SuffixSubstitutionCarriers,
            ExplicitFamilyCarriers,
            SuffixFamilyCarriers,
            ClosedTransportCarriers,
            PreVectorClosedTransportCarriers,
            ClosedSourceCarriers,
            OutputOriginGuardCarriers,
        );
        id_transports!(builtins::BuiltinIds);
        id_transports!(BaselineIds);
        id_transports!(ExplicitIds);
        id_transports!(SuffixIds);
        id_transports!(PreVectorIds);
        id_transports!(VectorSuffixIds);
        candidate::<ExplicitSubstitutionCarriers>("four_optional_ids_substitution_only");
        candidate::<SuffixSubstitutionCarriers>("suffix_bases_substitution_only");
        candidate::<ExplicitFamilyCarriers>("four_optional_ids_with_family_roles");
        candidate::<SuffixFamilyCarriers>("suffix_bases_with_family_roles");
        println!(
            "OUTPUT_ASSOCIATION_INTEGRATED historical_carriers={} actual_carriers={} \
             delta={} selectors_ranks_iteration_values_results=PAID public_output_admission=CURRENT_EXECUTABLE",
            size_of::<BaselineCarriers>(),
            builtin_carrier_bytes(),
            builtin_carrier_bytes() as i128 - size_of::<BaselineCarriers>() as i128,
        );
    }
}

#[cfg(test)]
mod array_tests {
    use super::*;
    use crate::frontend::{lexer, parser};

    #[test]
    fn builtin_association_output_claims_are_denied_before_the_source_walk() {
        let mut sources = SourceMap::new();
        let text = "struct C {} fn main()->(){let c=C{};return;}";
        let file = sources.add("closed-output-association.ox".into(), text.into());
        let source = sources.get(file);
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let typed =
            super::super::typeck::check(super::super::resolve::resolve(source, &ast).unwrap())
                .unwrap();
        let mut raw = super::super::lower::lower(&typed).unwrap();
        assert!(check(&raw, typed.index(), &sources).is_ok());
        for origin in [
            BuiltinOrigins::WriteStatus,
            BuiltinOrigins::WriteStdout,
            BuiltinOrigins::ReadStatusWriteStatus,
            BuiltinOrigins::ReadStatusWriteStdout,
            BuiltinOrigins::ReadStdinWriteStatus,
            BuiltinOrigins::ReadStdinWriteStdout,
        ] {
            raw.builtins = origin;
            // Even the private builtin-allowed continuation cannot admit these
            // raw claims without the exact output source marker. No traversal or proof allocation
            // occurs; only the existing boxed diagnostic is allocated.
            let (denied, allocations) =
                super::super::super::reviewer_origins::integration_counted(|| {
                    check_impl(&raw, typed.index(), &sources, true, true, None)
                });
            let error = denied.unwrap_err();
            assert_eq!(error.code, "E0500");
            assert_eq!(error.stage, "oir-project-bind");
            assert_eq!(allocations, 2);
            assert!(check(&raw, typed.index(), &sources).is_err());
        }
    }

    #[test]
    fn unit2b_association_walks_every_array_operand_in_both_passes() {
        let mut sources = SourceMap::new();
        let text = "struct C {} fn main()->(){let a=C{};return;}";
        let file = sources.add("array-association.ox".into(), text.into());
        let foreign = sources.add("unrelated.ox".into(), text.into());
        let source = sources.get(file);
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let typed =
            super::super::typeck::check(super::super::resolve::resolve(source, &ast).unwrap())
                .unwrap();
        let mut raw = super::super::lower::lower(&typed).unwrap();
        let baseline = check(&raw, typed.index(), &sources).unwrap();
        let span = raw.functions[0].span;
        let operand = Operand {
            local: LocalId(0),
            span,
        };
        let base = AccessBase::Owner(OwnerPlaceId(0));
        let kinds = [
            (
                OwnedInstruction::ConstructArray {
                    destination: OwnerPlaceId(0),
                    elements: vec![operand; 1024],
                },
                1025,
            ),
            (
                OwnedInstruction::ReadIndex {
                    destination: LocalId(0),
                    base,
                    index: operand,
                },
                2,
            ),
            (
                OwnedInstruction::WriteIndex {
                    base,
                    index: operand,
                    value: operand,
                },
                3,
            ),
            (
                OwnedInstruction::ArrayLength {
                    destination: LocalId(0),
                    base,
                },
                1,
            ),
        ];
        for (kind, span_count) in kinds {
            raw.functions[0].blocks[0].statements.push(OwnedStatement {
                kind,
                span,
                diagnostic_origins: None,
            });
            let (result, allocations) =
                super::super::super::reviewer_origins::integration_counted(|| {
                    check(&raw, typed.index(), &sources)
                });
            assert_eq!(allocations, 0);
            let usage = result.unwrap();
            assert_eq!(usage.count.spans, baseline.count.spans + span_count);
            assert_eq!(
                usage.validation.spans,
                baseline.validation.spans + span_count
            );
            assert_eq!(usage.count.declarations, baseline.count.declarations);
            assert_eq!(usage.dimensions, baseline.dimensions);
            let original = raw.functions[0].blocks[0].statements.pop().unwrap();
            // One mutation per actual operand, plus the instruction and both
            // diagnostic origins. These spans are valid in SourceMap, but do
            // not belong to the indexed source function.
            let operand_count = span_count - 1;
            for location in 0..operand_count + 3 {
                let mut changed = original.clone();
                let wrong = Span {
                    file: foreign,
                    ..span
                };
                assert!(sources.is_valid_span(wrong));
                if location == operand_count {
                    changed.span = wrong;
                } else if location == operand_count + 1 {
                    changed.diagnostic_origins = Some(DiagnosticOrigins {
                        primary: wrong,
                        cause: span,
                    });
                } else if location == operand_count + 2 {
                    changed.diagnostic_origins = Some(DiagnosticOrigins {
                        primary: span,
                        cause: wrong,
                    });
                } else {
                    match &mut changed.kind {
                        OwnedInstruction::ConstructArray { elements, .. } => {
                            elements[location].span = wrong
                        }
                        OwnedInstruction::ReadIndex { index, .. } => index.span = wrong,
                        OwnedInstruction::WriteIndex { value, index, .. } => {
                            if location == 0 {
                                value.span = wrong;
                            } else {
                                index.span = wrong;
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                raw.functions[0].blocks[0].statements.push(changed);
                let e = check(&raw, typed.index(), &sources).unwrap_err();
                assert_eq!(e.code, "E0500");
                assert_eq!(e.stage, "oir-project-bind");
                raw.functions[0].blocks[0].statements.pop();
            }
        }
    }
}

#[cfg(test)]
mod enum_tests {
    use super::*;
    use crate::frontend::{
        declaration_index::{self, IndexLimits, SourceOwner, WorkMeter},
        lexer, parser,
        project::budget::Allocator,
        source::SourceView,
    };

    #[test]
    fn owned_enum_association_binds_payload_identity_and_new_instruction_origins() {
        let text = "struct R{x:i32} struct S{x:i32} enum First{V(R)} enum Second{V(S)} enum Wrap{A([i32;2]),N(First)} fn main()->i32{let e=First::V(R{x:42});match e{First::V(r)=>{return r.x;},}}";
        let mut sources = SourceMap::new();
        let file = sources.add("owned-enum-origin.ox".into(), text.into());
        let foreign = sources.add("other-owned-enum-origin.ox".into(), text.into());
        let source = sources.get(file);
        let (ast, _) = parser::parse_typed_counted(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::ProjectCandidate,
            parser::MAX_NODES,
            &mut Allocator::default(),
            &mut Default::default(),
        )
        .unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = declaration_index::collect_originals(
            owner,
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        let typed = super::super::resolve::type_enum_source(&index, &work, &mut allocator).unwrap();
        let mut raw = super::super::lower::lower(&typed).unwrap();
        let check =
            |raw: &RawOwnedProgram| check_impl(raw, &index, &sources, true, false, Some(&typed));
        assert!(check(&raw).is_ok());
        // These are valid declaration shapes with different nominal or
        // structural payload identities; none can inherit the source index.
        for (enumeration, variant, payload) in [
            (0, 0, AggregateTy::Record(RecordId(1))),
            (
                2,
                0,
                AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 3).unwrap()),
            ),
            (2, 1, AggregateTy::Enum(EnumId(1))),
        ] {
            let previous = raw.enums[enumeration].variants[variant].payload;
            raw.enums[enumeration].variants[variant].payload =
                Some(ParameterTy::Value(ValueTy::Owned(payload)));
            Declarations::check_combined(&raw.records, &raw.enums, &sources).unwrap();
            let error = check(&raw).unwrap_err();
            assert_eq!((error.code, error.stage), ("E0500", "oir-project-bind"));
            raw.enums[enumeration].variants[variant].payload = previous;
        }
        let mut visited = 0;
        for block in 0..raw.functions[0].blocks.len() {
            for statement in 0..raw.functions[0].blocks[block].statements.len() {
                if !matches!(
                    raw.functions[0].blocks[block].statements[statement].kind,
                    OwnedInstruction::ConstructOwnedEnum { .. }
                        | OwnedInstruction::ConsumeOwnedVariant { .. }
                ) {
                    continue;
                }
                let previous = raw.functions[0].blocks[block].statements[statement].span;
                raw.functions[0].blocks[block].statements[statement].span = Span {
                    file: foreign,
                    ..previous
                };
                let error = check(&raw).unwrap_err();
                assert_eq!((error.code, error.stage), ("E0500", "oir-project-bind"));
                raw.functions[0].blocks[block].statements[statement].span = previous;
                visited += 1;
            }
        }
        assert_eq!(visited, 2);
        assert!(check(&raw).is_ok());
    }

    #[test]
    fn enum_association_checks_unused_payload_identity_and_every_new_span() {
        let text = "enum E{N,V(i32),U(())} fn main()->(){return;}";
        let mut sources = SourceMap::new();
        let file = sources.add("enum-association.ox".into(), text.into());
        let foreign = sources.add("other-enum-association.ox".into(), text.into());
        let source = sources.get(file);
        let (ast, _) = parser::parse_enum_candidate_counted(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::ProjectCandidate,
            parser::MAX_NODES,
            &mut Allocator::default(),
            &mut Default::default(),
        )
        .unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = declaration_index::collect_enum_candidate(
            owner,
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        let declaration = &ast.enums[0];
        let span = ast.functions[0].name;
        let mut raw = RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            vector_functions: Vec::new(),
            enums: vec![RawEnumDecl {
                id: EnumId(0),
                span: declaration.name,
                variants: [None, Some(hir::Ty::I32), Some(hir::Ty::Unit)]
                    .into_iter()
                    .enumerate()
                    .map(|(index, payload)| RawVariantDecl {
                        id: VariantId {
                            enumeration: EnumId(0),
                            index,
                        },
                        payload: payload.map(|ty| ParameterTy::Value(ValueTy::Scalar(ty))),
                        span: declaration.variants[index].name,
                    })
                    .collect(),
            }],
            records: vec![],
            // Deliberately an association-only carrier fixture. Association
            // certifies provenance, not arbitrary expression/arm equivalence.
            functions: vec![RawOwnedFunction {
                id: hir::DefId(0),
                span,
                result: ValueTy::Scalar(hir::Ty::Unit),
                parameters: vec![],
                locals: vec![],
                places: vec![],
                owners: vec![],
                references: vec![],
                calls: vec![],
                loans: vec![],
                matches: vec![MatchDecl {
                    source: OwnerPlaceId(0),
                    span,
                    arms: vec![MatchArm {
                        variant: VariantId {
                            enumeration: EnumId(0),
                            index: 0,
                        },
                        dispatch: BlockId(0),
                        entry: BlockId(1),
                    }],
                }],
                entry: BlockId(0),
                blocks: vec![OwnedBlock {
                    merge: None,
                    span,
                    statements: vec![
                        OwnedStatement {
                            span,
                            diagnostic_origins: Some(DiagnosticOrigins {
                                primary: span,
                                cause: span,
                            }),
                            kind: OwnedInstruction::ConstructEnum {
                                destination: OwnerPlaceId(0),
                                variant: VariantId {
                                    enumeration: EnumId(0),
                                    index: 1,
                                },
                                payload: Some(Operand {
                                    local: LocalId(0),
                                    span,
                                }),
                            },
                        },
                        OwnedStatement {
                            span,
                            diagnostic_origins: Some(DiagnosticOrigins {
                                primary: span,
                                cause: span,
                            }),
                            kind: OwnedInstruction::ConsumeVariant {
                                match_id: MatchId(0),
                                arm: 0,
                                destination: None,
                            },
                        },
                    ],
                    terminator: Some(OwnedTerminator {
                        span,
                        diagnostic_origins: Some(DiagnosticOrigins {
                            primary: span,
                            cause: span,
                        }),
                        kind: OwnedTerminatorKind::MatchDispatch {
                            match_id: MatchId(0),
                            arm: 0,
                        },
                    }),
                }],
            }],
        };
        let (result, allocations) =
            super::super::super::reviewer_origins::integration_counted(|| {
                check_enum_candidate(&raw, &index, &sources)
            });
        assert_eq!(allocations, 0);
        let usage = result.unwrap();
        assert_eq!(usage.count, usage.validation);
        assert_eq!((usage.count.declarations, usage.count.spans), (7, 17));
        assert_eq!(usage.dimensions, 4);
        for origin in [
            BuiltinOrigins::ReadStatus,
            BuiltinOrigins::ReadStdin,
            BuiltinOrigins::WriteStatus,
            BuiltinOrigins::WriteStdout,
            BuiltinOrigins::ReadStatusWriteStatus,
            BuiltinOrigins::ReadStatusWriteStdout,
            BuiltinOrigins::ReadStdinWriteStatus,
            BuiltinOrigins::ReadStdinWriteStdout,
        ] {
            raw.builtins = origin;
            let (denied, allocations) =
                super::super::super::reviewer_origins::integration_counted(|| {
                    check_enum_candidate(&raw, &index, &sources)
                });
            assert!(denied.is_err());
            // The existing boxed diagnostic allocates its message and Box;
            // no declaration/proof storage is prepared on this denial.
            assert_eq!(allocations, 2);
        }
        raw.builtins = BuiltinOrigins::None;
        assert!(check(&raw, &index, &sources).is_err());

        // A scalar type mutation is still a valid independent raw declaration,
        // but no longer the enum declaration in the original source index.
        for payload in [
            Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::Bool))),
            None,
        ] {
            let original = raw.enums[0].variants[1].payload;
            raw.enums[0].variants[1].payload = payload;
            EnumDeclarations::prepare(&raw.enums, &sources, DeclarationUsage::default()).unwrap();
            assert!(check_enum_candidate(&raw, &index, &sources).is_err());
            raw.enums[0].variants[1].payload = original;
        }
        let original = raw.enums[0].variants[2].payload;
        raw.enums[0].variants[2].payload = None;
        assert!(check_enum_candidate(&raw, &index, &sources).is_err());
        raw.enums[0].variants[2].payload = original;

        // Every new source carrier, optional scalar operand, and both origin
        // roles must remain in the indexed declaration/function source file.
        fn target_span(raw: &mut RawOwnedProgram, target: usize) -> &mut Span {
            match target {
                0 => &mut raw.enums[0].span,
                1 => &mut raw.enums[0].variants[1].span,
                2 => &mut raw.functions[0].matches[0].span,
                3 => &mut raw.functions[0].blocks[0].statements[0].span,
                4 => {
                    &mut raw.functions[0].blocks[0].statements[0]
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .primary
                }
                5 => {
                    &mut raw.functions[0].blocks[0].statements[0]
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .cause
                }
                6 => match &mut raw.functions[0].blocks[0].statements[0].kind {
                    OwnedInstruction::ConstructEnum {
                        payload: Some(payload),
                        ..
                    } => &mut payload.span,
                    _ => unreachable!(),
                },
                7 => &mut raw.functions[0].blocks[0].statements[1].span,
                8 => {
                    &mut raw.functions[0].blocks[0].statements[1]
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .primary
                }
                9 => {
                    &mut raw.functions[0].blocks[0].statements[1]
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .cause
                }
                10 => &mut raw.functions[0].blocks[0].terminator.as_mut().unwrap().span,
                11 => {
                    &mut raw.functions[0].blocks[0]
                        .terminator
                        .as_mut()
                        .unwrap()
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .primary
                }
                _ => {
                    &mut raw.functions[0].blocks[0]
                        .terminator
                        .as_mut()
                        .unwrap()
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .cause
                }
            }
        }
        for target in 0..13 {
            let wrong = Span {
                file: foreign,
                ..span
            };
            assert!(sources.is_valid_span(wrong));
            let saved = std::mem::replace(target_span(&mut raw, target), wrong);
            assert!(check_enum_candidate(&raw, &index, &sources).is_err());
            *target_span(&mut raw, target) = saved;
        }
        assert_eq!(check_enum_candidate(&raw, &index, &sources).unwrap(), usage);
    }
}

#[test]
fn owned_u8_synthetic_function_cannot_inherit_program_source_authority() {
    let (sources, raw, _) = super::super::u8_tests::raw(1, false);
    let mut visitor = Visitor::validate(&sources);
    visitor.file(raw.functions[0].span.file);
    let error = function(
        &raw.functions[0],
        &mut visitor,
        true,
        None,
        FunctionSource::Synthetic,
        &mut ConversionSeen::empty(),
    )
    .unwrap_err();
    assert_eq!(
        (error.code, error.stage, error.primary),
        ("E0500", "oir-project-bind", None)
    );
}

#[cfg(test)]
thread_local! {
    pub(super) static CONVERSION_RESERVE_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
#[path = "byte_storage_association_tests.rs"]
mod byte_storage_association_tests;
