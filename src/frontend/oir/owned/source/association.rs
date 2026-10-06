//! The owned source association walk; raw ownership/shape verification is unchanged.
use super::super::*;
use crate::frontend::{
    builtin_catalog::{BuiltinEnum, BuiltinFunction},
    declaration_index::DeclarationIndex,
    oir::source::association::{bad, BindUsage, Visitor},
};

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
fn function(
    function: &RawOwnedFunction,
    visitor: &mut Visitor<'_>,
    allow_enums: bool,
    builtin: Option<BuiltinFunction>,
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
        for statement in &block.statements {
            visitor.span(statement.span)?;
            origins(statement.diagnostic_origins, visitor)?;
            match &statement.kind {
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
                OwnedInstruction::ConstructEnum { payload, .. } => {
                    if !allow_enums {
                        return Err(bad());
                    }
                    if let Some(payload) = payload {
                        visitor.span(payload.span)?;
                    }
                }
                OwnedInstruction::ConsumeVariant { .. } => {
                    if !allow_enums {
                        return Err(bad());
                    }
                }
                OwnedInstruction::Scalar(statement) => visitor.statement(statement)?,
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
    check_impl(raw, index, sources, true, false)
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
    check_impl(raw, index, sources, true, true)
}

// Only builtin identity/anchor transports, not inherited Visitor internals.
// Selectors, iterator/next, checked-rank results, projected-ID results, receiver
// borrows and retained first-function identity are separate roles: no lifetime reuse.
// The private caller pays this complete envelope before source lowering.
#[allow(dead_code)]
struct BuiltinAssociationCarriers {
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
) -> Result<BindUsage, Box<Diagnostic>> {
    // Closed identity transport does not activate output source descriptors.
    // Keep this gate in addition to the parser/index and raw verifier gates.
    if raw.builtins.has_output() || index.builtin_set().has_output() {
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
        )?;
    }
    let mut visitor = Visitor::validate(sources);
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
                    != expected
                        .payload()
                        .map(|ty| ParameterTy::Value(ValueTy::Scalar(ty)))
            {
                return Err(bad());
            }
        }
        visitor.file(original.diagnostic_span().file);
        enumeration(declaration, &mut visitor)?;
    }
    visitor.dimension(raw.records.len(), index.record_count())?;
    visitor.dimension(raw.functions.len(), index.function_count())?;
    for (ordinal, declaration) in raw.records.iter().enumerate() {
        let id = RecordId(ordinal);
        let (key, module) = index.record(id).map_err(|_| bad())?;
        let ast = index.sources().ast(module).map_err(|_| bad())?;
        let original = ast.records.get(key.index).ok_or_else(bad)?;
        if declaration.id != id || declaration.span != original.name {
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
            )?;
            continue;
        }
        let (key, module) = index.function(id).map_err(|_| bad())?;
        let ast = index.sources().ast(module).map_err(|_| bad())?;
        let original = ast.functions.get(key.index).ok_or_else(bad)?;
        if declaration.id != id || declaration.span != original.name {
            return Err(bad());
        }
        visitor.file(original.name.file);
        function(declaration, &mut visitor, allow_enums, None)?;
    }
    visitor.finish(count)
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
    carriers!(ClosedTransportCarriers, builtins::BuiltinIds, BuiltinOrigins;
        enumeration_kind: BuiltinEnum,
        function_kind: BuiltinFunction,
        enumeration_rank: Option<usize>,
        function_rank: Option<usize>,
        family_iteration: std::array::IntoIter<BuiltinEnum, 2>,
        next_family: Option<BuiltinEnum>,
        enumeration_id_return: Option<EnumId>,
        function_id_return: Option<hir::DefId>,
        input_function: Option<hir::DefId>,
        enumeration_ids_borrow: &'static builtins::BuiltinIds,
        function_ids_borrow: &'static builtins::BuiltinIds,
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
        same_layout::<SuffixIds, builtins::BuiltinIds>();
        same_layout::<ClosedSourceCarriers, BuiltinAssociationCarriers>();
        #[cfg(target_pointer_width = "64")]
        assert_eq!(size_of::<ClosedTransportCarriers>(), 568);
        same_layout::<Result<SuffixIds, OwnedFailure>, Result<builtins::BuiltinIds, OwnedFailure>>(
        );
        same_layout::<
            Result<SuffixIds, Box<Diagnostic>>,
            Result<builtins::BuiltinIds, Box<Diagnostic>>,
        >();
        assert_eq!(builtin_carrier_bytes(), size_of::<ClosedSourceCarriers>());

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
            ClosedSourceCarriers,
        );
        id_transports!(builtins::BuiltinIds);
        id_transports!(BaselineIds);
        id_transports!(ExplicitIds);
        id_transports!(SuffixIds);
        candidate::<ExplicitSubstitutionCarriers>("four_optional_ids_substitution_only");
        candidate::<SuffixSubstitutionCarriers>("suffix_bases_substitution_only");
        candidate::<ExplicitFamilyCarriers>("four_optional_ids_with_family_roles");
        candidate::<SuffixFamilyCarriers>("suffix_bases_with_family_roles");
        println!(
            "OUTPUT_ASSOCIATION_INTEGRATED historical_carriers={} actual_carriers={} \
             delta={} selectors_ranks_iteration_values_results=PAID output_admission=DENIED",
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
            // raw claims. No source/descriptor traversal or proof allocation
            // occurs; only the existing boxed diagnostic is allocated.
            let (denied, allocations) =
                super::super::super::reviewer_origins::integration_counted(|| {
                    check_impl(&raw, typed.index(), &sources, true, true)
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
