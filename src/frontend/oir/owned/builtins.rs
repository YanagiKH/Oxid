//! Closed builtin descriptor checks. These facts are inert: they grant neither
//! source provenance nor an executable witness, and do not replace the ordinary
//! declaration, shape, CFG, or ownership proof.
use super::*;
use crate::frontend::builtin_catalog::{BuiltinEnum, BuiltinFunction};

pub(super) const INPUT_SCRATCH_BYTES: usize = 1024;
pub(super) const OUTPUT_SCRATCH_BYTES: usize = 1024;

/// Header + enum + three variants; the function adds its declaration,
/// reference, owner, block, two statements and terminator. Work units count
/// these fixed descriptor visits, not individual scalar comparisons. Each
/// family's descriptor has the same finite shape; this grants no admission.
pub(super) fn descriptor_visits(origin: BuiltinOrigins) -> usize {
    if origin == BuiltinOrigins::None {
        0
    } else {
        1 + 4 * origin.extra_enums() + 7 * origin.extra_functions()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BuiltinIds {
    enumeration_base: EnumId,
    function_base: hir::DefId,
    inventory: BuiltinOrigins,
}
impl BuiltinIds {
    pub(super) fn enumeration(&self, kind: BuiltinEnum) -> Option<EnumId> {
        self.enumeration_base
            .0
            .checked_add(self.inventory.enum_rank(kind)?)
            .map(EnumId)
    }
    pub(super) fn function(&self, kind: BuiltinFunction) -> Option<hir::DefId> {
        self.function_base
            .0
            .checked_add(self.inventory.function_rank(kind)?)
            .map(hir::DefId)
    }
}

/// Inspect only the fixed canonical suffix selected by the explicit origin
/// claim. In particular, ordinary declarations with matching shapes never
/// acquire builtin identity, and the None case does not visit any raw row.
pub(super) fn check(raw: &RawOwnedProgram) -> Result<BuiltinIds, OwnedFailure> {
    // Output consumers are not activated yet. Reject
    // every output family claim before reading even the first descriptor row.
    if raw.builtins.has_output() {
        return Err(missing());
    }
    check_candidate(raw)
}

/// Complete inert descriptor validation, including the denied output candidate.
/// This cannot produce an executable witness or bypass ordinary proofs. The
/// production entry above remains the sole builtin admission gate.
pub(super) fn check_candidate(raw: &RawOwnedProgram) -> Result<BuiltinIds, OwnedFailure> {
    let ids = BuiltinIds {
        enumeration_base: EnumId(
            raw.enums
                .len()
                .checked_sub(raw.builtins.extra_enums())
                .ok_or_else(missing)?,
        ),
        function_base: hir::DefId(
            raw.functions
                .len()
                .checked_sub(raw.builtins.extra_functions())
                .ok_or_else(missing)?,
        ),
        inventory: raw.builtins,
    };
    if raw.builtins == BuiltinOrigins::None {
        return Ok(ids);
    }

    for kind in BuiltinEnum::ALL {
        if let Some(id) = ids.enumeration(kind) {
            check_enumeration(raw.enums.get(id.0).ok_or_else(missing)?, id, kind)?;
        }
    }
    for kind in BuiltinFunction::ALL {
        if let Some(id) = ids.function(kind) {
            let enumeration = ids
                .enumeration(match kind {
                    BuiltinFunction::ReadStdin => BuiltinEnum::ReadStatus,
                    BuiltinFunction::WriteStdout => BuiltinEnum::WriteStatus,
                })
                .ok_or_else(missing)?;
            check_function(
                raw.functions.get(id.0).ok_or_else(missing)?,
                id,
                enumeration,
                kind,
            )?;
        }
    }
    Ok(ids)
}

fn check_enumeration(
    enumeration: &RawEnumDecl,
    enumeration_id: EnumId,
    kind: BuiltinEnum,
) -> Result<(), OwnedFailure> {
    if enumeration.id != enumeration_id {
        return Err(OwnedFailure::malformed(
            Malformed::Binding,
            enumeration.span,
        ));
    }
    let [first, second, third] = enumeration.variants.as_slice() else {
        return Err(OwnedFailure::malformed(
            Malformed::Binding,
            enumeration.span,
        ));
    };
    // The catalog's names are not raw data. Identity is the enum plus these
    // three fixed member ordinals, never the diagnostic anchor's spelling.
    let payload_member = match kind {
        BuiltinEnum::ReadStatus => 0,
        BuiltinEnum::WriteStatus => 2,
    };
    for (index, variant) in [first, second, third].into_iter().enumerate() {
        let payload =
            (index == payload_member).then_some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)));
        if variant.id
            != (VariantId {
                enumeration: enumeration_id,
                index,
            })
            || variant.payload != payload
        {
            return Err(OwnedFailure::malformed(Malformed::Binding, variant.span));
        }
        if variant.span != enumeration.span {
            return Err(OwnedFailure::malformed(
                Malformed::CanonicalSite,
                variant.span,
            ));
        }
    }

    Ok(())
}

fn missing() -> OwnedFailure {
    OwnedFailure {
        kind: OwnedFailureKind::Malformed(Malformed::Binding),
        primary: Origin::NONE,
        related: Origin::NONE,
        declaration: Origin::NONE,
        context: None,
    }
}

fn check_function(
    function: &RawOwnedFunction,
    function_id: hir::DefId,
    enumeration_id: EnumId,
    kind: BuiltinFunction,
) -> Result<(), OwnedFailure> {
    let binding = || OwnedFailure::malformed(Malformed::Binding, function.span);
    let site = |span| OwnedFailure::malformed(Malformed::CanonicalSite, span);
    if function.id != function_id
        || function.result != ValueTy::Owned(AggregateTy::Enum(enumeration_id))
        || !matches!(
            function.parameters.as_slice(),
            [ParameterBinding::Reference(ReferenceParamId(0))]
        )
        || !function.locals.is_empty()
        || !function.places.is_empty()
        || !function.calls.is_empty()
        || !function.loans.is_empty()
        || !function.matches.is_empty()
    {
        return Err(binding());
    }
    let [reference] = function.references.as_slice() else {
        return Err(binding());
    };
    let [owner] = function.owners.as_slice() else {
        return Err(binding());
    };
    if reference.position != 0
        || reference.kind
            != match kind {
                BuiltinFunction::ReadStdin => BorrowKind::Exclusive,
                BuiltinFunction::WriteStdout => BorrowKind::Shared,
            }
        || reference.referent() != BorrowedTy::ScalarSlice(hir::Ty::I32)
        || owner.kind != OwnerKind::Temporary
        || owner.aggregate() != AggregateTy::Enum(enumeration_id)
    {
        return Err(binding());
    }
    for span in [reference.span, owner.span] {
        if span != function.span {
            return Err(site(span));
        }
    }

    let [block] = function.blocks.as_slice() else {
        return Err(site(function.span));
    };
    if function.entry != BlockId(0) || block.merge.is_some() {
        return Err(site(block.span));
    }
    if block.span != function.span {
        return Err(site(block.span));
    }
    let [live, operation] = block.statements.as_slice() else {
        return Err(site(block.span));
    };
    if !matches!(live.kind, OwnedInstruction::StorageLive(OwnerPlaceId(0))) {
        return Err(site(live.span));
    }
    if !matches!(
        (kind, &operation.kind),
        (
            BuiltinFunction::ReadStdin,
            OwnedInstruction::ReadStdin {
                buffer: ReferenceParamId(0),
                destination: OwnerPlaceId(0),
            }
        ) | (
            BuiltinFunction::WriteStdout,
            OwnedInstruction::WriteStdout {
                buffer: ReferenceParamId(0),
                destination: OwnerPlaceId(0),
            }
        )
    ) {
        return Err(site(operation.span));
    }
    for statement in [live, operation] {
        if statement.span != function.span || statement.diagnostic_origins.is_some() {
            return Err(site(statement.span));
        }
    }
    let Some(terminator) = &block.terminator else {
        return Err(site(block.span));
    };
    if !matches!(
        terminator.kind,
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0))
    ) || terminator.span != function.span
        || terminator.diagnostic_origins.is_some()
    {
        return Err(site(terminator.span));
    }
    Ok(())
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    #[test]
    fn builtin_ids_nine_closed_inventories_keep_independent_suffix_ranks() {
        use BuiltinOrigins::*;
        // Expected identities are independent of the catalog rank helpers.
        // These inert values never enter check(), verification, or a consumer.
        for (inventory, enums, functions, visits) in [
            (
                None,
                [Option::None, Option::None],
                [Option::None, Option::None],
                0,
            ),
            (
                ReadStatus,
                [Some(11), Option::None],
                [Option::None, Option::None],
                5,
            ),
            (
                ReadStdin,
                [Some(11), Option::None],
                [Some(23), Option::None],
                12,
            ),
            (
                WriteStatus,
                [Option::None, Some(11)],
                [Option::None, Option::None],
                5,
            ),
            (
                WriteStdout,
                [Option::None, Some(11)],
                [Option::None, Some(23)],
                12,
            ),
            (
                ReadStatusWriteStatus,
                [Some(11), Some(12)],
                [Option::None, Option::None],
                9,
            ),
            (
                ReadStatusWriteStdout,
                [Some(11), Some(12)],
                [Option::None, Some(23)],
                16,
            ),
            (
                ReadStdinWriteStatus,
                [Some(11), Some(12)],
                [Some(23), Option::None],
                16,
            ),
            (
                ReadStdinWriteStdout,
                [Some(11), Some(12)],
                [Some(23), Some(24)],
                23,
            ),
        ] {
            let ids = BuiltinIds {
                enumeration_base: EnumId(11),
                function_base: hir::DefId(23),
                inventory,
            };
            assert_eq!(
                ids.enumeration(BuiltinEnum::ReadStatus),
                enums[0].map(EnumId)
            );
            assert_eq!(
                ids.enumeration(BuiltinEnum::WriteStatus),
                enums[1].map(EnumId)
            );
            assert_eq!(
                ids.function(BuiltinFunction::ReadStdin),
                functions[0].map(hir::DefId)
            );
            assert_eq!(
                ids.function(BuiltinFunction::WriteStdout),
                functions[1].map(hir::DefId)
            );
            assert_eq!(descriptor_visits(inventory), visits);
        }
    }

    #[test]
    fn builtin_ids_rank_projection_does_not_wrap() {
        // Even an invalid test-only base cannot wrap a family onto a source ID.
        let ids = BuiltinIds {
            enumeration_base: EnumId(usize::MAX),
            function_base: hir::DefId(usize::MAX),
            inventory: BuiltinOrigins::ReadStdinWriteStdout,
        };
        assert_eq!(ids.enumeration(BuiltinEnum::WriteStatus), None);
        assert_eq!(ids.function(BuiltinFunction::WriteStdout), None);
    }
}
