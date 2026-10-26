//! Source/index integration only. Descriptor shape does not create provenance.
use super::super::{Origin, OwnedFailure, OwnedFailureKind, RawOwnedFunction, RawOwnedProgram};
use crate::frontend::{
    builtin_catalog::vector,
    declaration_index::{DeclarationIndex, WorkMeter},
    diagnostic::Diagnostic,
    hir::DefId,
    source::Span,
};

pub(super) struct SelectedSignature {
    pub(super) id: DefId,
    pub(super) anchor: Span,
    pub(super) signature: vector::Signature,
}

fn invalid(at: Span) -> Box<Diagnostic> {
    crate::frontend::owned_diagnostic::diagnostic(
        "E0500",
        "resolve",
        format_args!("invalid vector source signature identity"),
        Some(at),
    )
}

/// Index key lookup is read-only and deliberately not a second mutable table.
/// Pay the actual prefix compared by its linear rank query before calling it.
pub(super) fn lookup_work(
    work: &WorkMeter,
    rank: usize,
    lookups: usize,
    at: Span,
) -> Result<(), Box<Diagnostic>> {
    let visits = rank
        .checked_add(1)
        .and_then(|n| n.checked_mul(lookups))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| {
            crate::frontend::owned_diagnostic::diagnostic(
                "E0400",
                "resolve",
                format_args!("vector source work overflow"),
                Some(at),
            )
        })?;
    work.debit(visits, at, "vector concrete key lookup")
}

pub(super) fn function(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    rank: usize,
    key: vector::Function,
) -> Result<(DefId, Span), Box<Diagnostic>> {
    let at = index.sources().eof();
    work.debit(1, at, "vector source key position")?;
    if index.vector_functions().get(rank) != Some(&key) {
        return Err(invalid(at));
    }
    lookup_work(work, rank, 1, at)?;
    let id = index.vector_function_id(key)?;
    let anchor = index.vector_anchor(key.item())?;
    let expected = index
        .source_function_count()
        .checked_add(index.builtin_set().extra_functions())
        .and_then(|base| base.checked_add(rank))
        .map(DefId)
        .ok_or_else(|| invalid(anchor))?;
    if id != expected {
        return Err(invalid(anchor));
    }
    Ok((id, anchor))
}

pub(super) fn signature(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    rank: usize,
    key: vector::Function,
) -> Result<SelectedSignature, Box<Diagnostic>> {
    let (id, anchor) = function(index, work, rank, key)?;
    // vector_signature independently rechecks the concrete key's ID once.
    lookup_work(work, rank, 1, anchor)?;
    let signature = index.vector_signature(key)?;
    Ok(SelectedSignature {
        id,
        anchor,
        signature,
    })
}

// Preserve the boxed diagnostic transport used by checked source queries.
#[allow(clippy::boxed_local)]
pub(super) fn lower_error(error: Box<Diagnostic>) -> OwnedFailure {
    let at = error.primary;
    if error.code == "E0400" {
        let mut result = OwnedFailure::resource("vector source work");
        result.primary = at.map_or(Origin::NONE, Origin);
        result
    } else {
        OwnedFailure {
            kind: OwnedFailureKind::Malformed(super::super::Malformed::Binding),
            primary: at.map_or(Origin::NONE, Origin),
            related: Origin::NONE,
            declaration: Origin::NONE,
            context: None,
        }
    }
}

/// Extra actual vector-specific transports, summed across the sequential
/// preflight, resolved/typed signature checks, raw copy and association roles.
/// Payload capacities and canonical function rows are charged separately.
#[allow(dead_code)]
struct Carriers {
    selections: [SelectedSignature; 4],
    selection_returns: [Result<SelectedSignature, Box<Diagnostic>>; 4],
    signatures: [vector::Signature; 4],
    signature_returns: [Result<vector::Signature, Box<Diagnostic>>; 4],
    functions: [vector::Function; 8],
    key_slices: [&'static [vector::Function]; 4],
    key_iterations:
        [std::iter::Enumerate<std::iter::Copied<std::slice::Iter<'static, vector::Function>>>; 4],
    next_keys: [Option<(usize, vector::Function)>; 4],
    key_claim: Vec<vector::Function>,
    claim_reservation: Result<Vec<vector::Function>, OwnedFailure>,
    key_options: [Option<&'static vector::Function>; 2],
    parameters: std::slice::Iter<'static, super::hir::ParameterTy>,
    signature_row: Option<&'static super::hir::Signature>,
    raw: &'static RawOwnedProgram,
    raw_function: Option<&'static RawOwnedFunction>,
    function_returns: [Result<(DefId, Span), Box<Diagnostic>>; 3],
    lookup_returns: [Result<DefId, Box<Diagnostic>>; 3],
    anchors: [Result<Span, Box<Diagnostic>>; 3],
    selected_anchor: Span,
    checked_indices: [Option<usize>; 4],
    checked_counts: [usize; 4],
    lookup_unit: Result<(), Box<Diagnostic>>,
    source_rebuild: Result<(), Box<Diagnostic>>,
    normalized: Result<(), Vec<Diagnostic>>,
    raw_result: Result<(), OwnedFailure>,
}

pub(super) const fn carrier_bytes() -> usize {
    std::mem::size_of::<Carriers>()
}

/// Additional typed-range coercion roles, separately priced per actual borrow
/// argument in element-range source even when no concrete suffix is emitted.
#[allow(dead_code)]
struct RangeCarriers {
    element: crate::frontend::hir::Ty,
    admitted: bool,
    array: super::hir::FixedArrayTy,
    actual: super::hir::ParameterTy,
    referent: super::hir::BorrowedTy,
    kind: super::hir::BorrowKind,
    converted: super::hir::ParameterTy,
}
pub(super) const fn range_carrier_bytes() -> usize {
    std::mem::size_of::<RangeCarriers>()
}
