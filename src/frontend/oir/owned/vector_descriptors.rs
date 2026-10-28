//! Independent concrete-vector canonical descriptor checks. No source
//! provenance or executable authority is created by matching these raw rows.
use super::*;
use crate::frontend::builtin_catalog::{experimental::Enumeration, vector::Function};

pub(super) fn check(raw: &RawOwnedProgram, ids: &builtins::BuiltinIds) -> Result<(), OwnedFailure> {
    for (rank, operation) in raw.vector_functions.iter().enumerate() {
        if !raw.builtins.contains_vector(operation.item())
            || (operation.record_id().is_none()
                && !crate::frontend::builtin_catalog::vector::scalar_admitted(operation.element()))
            || operation
                .record_id()
                .is_some_and(|id| !raw.records.get(id.0).is_some_and(|row| row.is_copy))
            || raw.vector_functions[..rank].contains(operation)
        {
            return Err(bad());
        }
        let status = if operation.operation().requires_grow_status() {
            ids.experimental_enum(Enumeration::Grow).ok_or_else(bad)?
        } else {
            EnumId(0)
        };
        let id = ids.vector_function_at(rank).ok_or_else(bad)?;
        function(
            raw.functions.get(id.0).ok_or_else(bad)?,
            id,
            *operation,
            &operation.signature(status),
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
    signature: &crate::frontend::builtin_catalog::vector::Signature,
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
    let (mut scalars, mut references, mut input_owners) = (0, 0, 0);
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
            (ParameterBinding::Owned(owner), ParameterTy::Value(ValueTy::Owned(aggregate))) => {
                let row = f.owners.get(input_owners).ok_or_else(fail)?;
                if *owner != OwnerPlaceId(input_owners)
                    || row.aggregate() != *aggregate
                    || row.kind != (OwnerKind::Parameter { position })
                    || row.span != f.span
                {
                    return Err(fail());
                }
                input_owners += 1;
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
            if f.locals.len() != scalars || f.owners.len() != input_owners + 1 {
                return Err(fail());
            }
            let row = &f.owners[input_owners];
            if row.aggregate() != aggregate
                || row.kind != OwnerKind::Temporary
                || row.span != f.span
            {
                return Err(fail());
            }
            (CallResult::Owned(OwnerPlaceId(input_owners)), true)
        }
        ValueTy::Scalar(ty) => {
            if f.locals.len() != scalars + 1
                || f.owners.len() != input_owners
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
            OwnedInstruction::StorageLive(owner) if owner == OwnerPlaceId(input_owners)
        )
    {
        return Err(fail());
    }
    if !matches!(block.statements[usize::from(owned)].kind,
        OwnedInstruction::Vector { operation: op, destination: dest } if op == operation && same_result(dest, destination))
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

/// Supplemental descriptor transports against the existing metadata ceiling.
/// This is a named request envelope, not a bound on Rust machine-stack usage.
/// The raw proof's inherited scratch quota still prices vector/table payloads.
#[allow(dead_code)]
struct DescriptorCarriers {
    identities: builtins::BuiltinIds,
    identity_return: Result<builtins::BuiltinIds, OwnedFailure>,
    keys: std::iter::Enumerate<std::slice::Iter<'static, Function>>,
    duplicate_keys: std::slice::Iter<'static, Function>,
    key: Function,
    key_return: Option<Function>,
    signatures: [crate::frontend::builtin_catalog::vector::Signature; 2],
    parameters: std::iter::Enumerate<
        std::iter::Zip<
            std::slice::Iter<'static, ParameterBinding>,
            std::slice::Iter<'static, ParameterTy>,
        >,
    >,
    statements: std::slice::Iter<'static, OwnedStatement>,
    function_id: Option<hir::DefId>,
    status_id: Option<EnumId>,
    descriptor_returns: [Result<(), OwnedFailure>; 2],
    destinations: [(CallResult, bool); 2],
}
pub(super) fn carrier_bytes() -> usize {
    std::mem::size_of::<DescriptorCarriers>()
}
