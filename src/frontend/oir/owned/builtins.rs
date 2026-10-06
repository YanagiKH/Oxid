//! Closed builtin descriptor checks. These facts are inert: they grant neither
//! source provenance nor an executable witness, and do not replace the ordinary
//! declaration, shape, CFG, or ownership proof.
use super::*;

pub(super) const INPUT_SCRATCH_BYTES: usize = 1024;

/// Header + enum + three variants; the function adds its declaration,
/// reference, owner, block, two statements and terminator. Work units count
/// these fixed descriptor visits, not individual scalar comparisons.
pub(super) const fn descriptor_visits(origin: BuiltinOrigins) -> usize {
    match origin {
        BuiltinOrigins::None => 0,
        BuiltinOrigins::ReadStatus => 5,
        BuiltinOrigins::ReadStdin => 12,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BuiltinIds {
    pub enumeration: Option<EnumId>,
    pub function: Option<hir::DefId>,
}

/// Inspect only the fixed canonical suffix selected by the explicit origin
/// claim. In particular, ordinary declarations with matching shapes never
/// acquire builtin identity, and the None case does not visit any raw row.
pub(super) fn check(raw: &RawOwnedProgram) -> Result<BuiltinIds, OwnedFailure> {
    if raw.builtins == BuiltinOrigins::None {
        return Ok(BuiltinIds {
            enumeration: None,
            function: None,
        });
    }

    let enumeration = raw.enums.last().ok_or_else(missing)?;
    // last() established nonemptiness before the canonical ordinal subtraction.
    let enumeration_id = EnumId(raw.enums.len() - 1);
    if enumeration.id != enumeration_id {
        return Err(OwnedFailure::malformed(
            Malformed::Binding,
            enumeration.span,
        ));
    }
    let [eof, full, io_error] = enumeration.variants.as_slice() else {
        return Err(OwnedFailure::malformed(
            Malformed::Binding,
            enumeration.span,
        ));
    };
    // The catalog's names are not raw data. Identity is the enum plus these
    // three fixed member ordinals, never the diagnostic anchor's spelling.
    for (index, variant, payload) in [
        (
            0,
            eof,
            Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32))),
        ),
        (1, full, None),
        (2, io_error, None),
    ] {
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

    let function = match raw.builtins {
        BuiltinOrigins::ReadStatus => None,
        BuiltinOrigins::ReadStdin => {
            let function = raw.functions.last().ok_or_else(missing)?;
            // As above, derive the suffix ordinal only after the bounds check.
            let function_id = hir::DefId(raw.functions.len() - 1);
            check_function(function, function_id, enumeration_id)?;
            Some(function_id)
        }
        BuiltinOrigins::None => unreachable!("handled before inspecting raw declarations"),
    };
    Ok(BuiltinIds {
        enumeration: Some(enumeration_id),
        function,
    })
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
        || reference.kind != BorrowKind::Exclusive
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
    let [live, read] = block.statements.as_slice() else {
        return Err(site(block.span));
    };
    if !matches!(live.kind, OwnedInstruction::StorageLive(OwnerPlaceId(0))) {
        return Err(site(live.span));
    }
    if !matches!(
        read.kind,
        OwnedInstruction::ReadStdin {
            buffer: ReferenceParamId(0),
            destination: OwnerPlaceId(0),
        }
    ) {
        return Err(site(read.span));
    }
    for statement in [live, read] {
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
