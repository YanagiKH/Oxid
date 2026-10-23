//! Independent raw descriptor validation for the closed experimental catalog.
use super::*;
use crate::frontend::builtin_catalog::experimental::{Enumeration, Function};

pub(super) fn check(raw: &RawOwnedProgram, ids: &builtins::BuiltinIds) -> Result<(), OwnedFailure> {
    for kind in Enumeration::ALL {
        let Some(id) = ids.experimental_enum(kind) else {
            continue;
        };
        let declaration = raw.enums.get(id.0).ok_or_else(bad)?;
        if declaration.id != id || declaration.variants.len() != kind.members().len() {
            return Err(bad());
        }
        for (index, variant) in declaration.variants.iter().enumerate() {
            if variant.id
                != (VariantId {
                    enumeration: id,
                    index,
                })
                || variant.span != declaration.span
                || variant.payload != kind.value_payload(index).map(ParameterTy::Value)
            {
                return Err(bad());
            }
        }
    }
    for kind in Function::ALL {
        let Some(id) = ids.experimental_function(kind) else {
            continue;
        };
        let enumeration = kind
            .result_enum()
            .map(|kind| ids.experimental_enum(kind).ok_or_else(bad))
            .transpose()?;
        let signature = kind.signature(|_| enumeration.expect("checked dependency"));
        function(
            raw.functions.get(id.0).ok_or_else(bad)?,
            id,
            kind,
            &signature,
        )?;
    }
    Ok(())
}
fn bad() -> OwnedFailure {
    OwnedFailure {
        kind: OwnedFailureKind::Malformed(Malformed::Binding),
        primary: Origin::NONE,
        related: Origin::NONE,
        declaration: Origin::NONE,
        context: None,
    }
}

fn function(
    f: &RawOwnedFunction,
    id: hir::DefId,
    operation: Function,
    signature: &crate::frontend::builtin_catalog::experimental::Signature,
) -> Result<(), OwnedFailure> {
    let fail = || OwnedFailure::malformed(Malformed::CanonicalSite, f.span);
    if f.id != id
        || f.result != signature.result
        || f.parameters.len() != signature.parameters().len()
        || !f.places.is_empty()
        || !f.calls.is_empty()
        || !f.loans.is_empty()
        || !f.matches.is_empty()
        || f.entry != BlockId(0)
        || f.blocks.len() != 1
    {
        return Err(fail());
    }
    let (mut scalars, mut references) = (0, 0);
    for (position, (actual, expected)) in
        f.parameters.iter().zip(signature.parameters()).enumerate()
    {
        match (actual, expected) {
            (ParameterBinding::Scalar(local), ParameterTy::Value(ValueTy::Scalar(ty))) => {
                if *local != LocalId(scalars)
                    || f.locals.get(scalars)
                        != Some(&LocalDecl {
                            ty: *ty,
                            kind: LocalKind::Parameter,
                            span: f.span,
                        })
                {
                    return Err(fail());
                }
                scalars += 1;
            }
            (ParameterBinding::Reference(reference), ParameterTy::Reference { referent, kind }) => {
                let row = f.references.get(references).ok_or_else(fail)?;
                if *reference != ReferenceParamId(references)
                    || row.referent() != *referent
                    || row.kind != *kind
                    || row.position != position
                    || row.span != f.span
                {
                    return Err(fail());
                }
                references += 1;
            }
            _ => return Err(fail()),
        }
    }
    if f.references.len() != references {
        return Err(fail());
    }
    let (destination, owned) = match signature.result {
        ValueTy::Owned(aggregate) => {
            if f.locals.len() != scalars || f.owners.len() != 1 {
                return Err(fail());
            }
            let row = &f.owners[0];
            if row.aggregate() != aggregate
                || row.kind != OwnerKind::Temporary
                || row.span != f.span
            {
                return Err(fail());
            }
            (CallResult::Owned(OwnerPlaceId(0)), true)
        }
        ValueTy::Scalar(ty) => {
            if f.locals.len() != scalars + 1
                || !f.owners.is_empty()
                || f.locals[scalars]
                    != (LocalDecl {
                        ty,
                        kind: LocalKind::Temporary,
                        span: f.span,
                    })
            {
                return Err(fail());
            }
            (CallResult::Scalar(LocalId(scalars)), false)
        }
    };
    let block = &f.blocks[0];
    if block.span != f.span
        || block.merge.is_some()
        || block.statements.len() != 1 + usize::from(owned)
    {
        return Err(fail());
    }
    if owned
        && !matches!(
            block.statements[0].kind,
            OwnedInstruction::StorageLive(OwnerPlaceId(0))
        )
    {
        return Err(fail());
    }
    if !matches!(block.statements[usize::from(owned)].kind,
        OwnedInstruction::Experimental { operation: op, destination: dest } if op == operation && same_result(dest, destination))
    {
        return Err(fail());
    }
    for row in &block.statements {
        if row.span != f.span || row.diagnostic_origins.is_some() {
            return Err(fail());
        }
    }
    let end = block.terminator.as_ref().ok_or_else(fail)?;
    if end.span != f.span || end.diagnostic_origins.is_some() {
        return Err(fail());
    }
    match (&end.kind, destination) {
        (OwnedTerminatorKind::ReturnOwned(actual), CallResult::Owned(expected))
            if *actual == expected => {}
        (OwnedTerminatorKind::ReturnScalar(actual), CallResult::Scalar(expected))
            if actual.local == expected && actual.span == f.span => {}
        _ => return Err(fail()),
    }
    Ok(())
}
fn same_result(a: CallResult, b: CallResult) -> bool {
    matches!((a, b), (CallResult::Owned(a), CallResult::Owned(b)) if a == b)
        || matches!((a, b), (CallResult::Scalar(a), CallResult::Scalar(b)) if a == b)
}
