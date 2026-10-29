//! Read-only source facts for the private owned-vector typing continuation.
//! These rows provide neither raw instructions nor executable source authority.
use super::{hir::*, hir_budget::HirPlan, resolve, typeck};
use crate::frontend::{
    builtin_catalog::owned_vector,
    declaration_index::{DeclarationIndex, WorkMeter},
    diagnostic::Diagnostic,
    project::budget::Allocator,
    source::Span,
};

pub(super) struct SelectedSignature {
    pub(super) id: DefId,
    pub(super) anchor: Span,
    pub(super) signature: owned_vector::Signature,
}

fn invalid(at: Span) -> Box<Diagnostic> {
    crate::frontend::owned_diagnostic::diagnostic(
        "E0500",
        "resolve",
        format_args!("invalid owned vector source signature identity"),
        Some(at),
    )
}

pub(super) fn signature(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    rank: usize,
    key: owned_vector::Function,
) -> Result<SelectedSignature, Box<Diagnostic>> {
    let at = index.sources().eof();
    work.debit(1, at, "owned vector source signature position")?;
    if !index.is_owned_vector_candidate_pipeline()
        || index.owned_vector_functions().get(rank) != Some(&key)
    {
        return Err(invalid(at));
    }
    // These new queries meter their own complete key scans. Unlike the old
    // unmetered key projection, they require no caller-estimated prefix debit.
    let id = index.owned_vector_function_id(key, work)?;
    let anchor = index.owned_vector_anchor(key, work)?;
    let signature = index.owned_vector_signature(key, work)?;
    let expected = index
        .source_function_count()
        .checked_add(index.builtin_set().extra_functions())
        .and_then(|n| n.checked_add(index.vector_functions().len()))
        .and_then(|n| n.checked_add(rank))
        .map(DefId)
        .ok_or_else(|| invalid(anchor))?;
    if id != expected {
        return Err(invalid(anchor));
    }
    Ok(SelectedSignature {
        id,
        anchor,
        signature,
    })
}

// Four actual source families use these facts: count, signature construction,
// resolved identity completion and typed identity recheck before lowering.
// Payload rows/capacities are additional and are admitted by HirPlan.
#[allow(dead_code)]
struct SignatureRoles {
    index: &'static DeclarationIndex<'static>,
    work: &'static WorkMeter,
    rank: usize,
    key: owned_vector::Function,
    selected: SelectedSignature,
    returned: Result<SelectedSignature, Box<Diagnostic>>,
    signature: owned_vector::Signature,
    signature_return: Result<owned_vector::Signature, Box<Diagnostic>>,
    id: DefId,
    id_return: Result<DefId, Box<Diagnostic>>,
    anchor: Span,
    anchor_return: Result<Span, Box<Diagnostic>>,
    keys: &'static [owned_vector::Function],
    iterator:
        std::iter::Enumerate<std::iter::Copied<std::slice::Iter<'static, owned_vector::Function>>>,
    next: Option<(usize, owned_vector::Function)>,
    parameters: std::slice::Iter<'static, ParameterTy>,
    parameter: Option<&'static ParameterTy>,
    actual: Option<&'static Signature>,
    constructed: Signature,
    checked: [Option<usize>; 4],
    counts: [usize; 4],
    origin: bool,
    comparisons: [bool; 4],
    unit: Result<(), Box<Diagnostic>>,
    normalized: Result<(), Vec<Diagnostic>>,
}

#[allow(dead_code)]
struct TypingRoles {
    index: &'static DeclarationIndex<'static>,
    work: &'static WorkMeter,
    allocator: &'static mut Allocator,
    replay: Result<(), Vec<Diagnostic>>,
    returned: Result<typeck::TypedOwnedProgram<'static>, Vec<Diagnostic>>,
    dispatch_inputs: (
        &'static DeclarationIndex<'static>,
        &'static WorkMeter,
        &'static mut Allocator,
        resolve::SourceAdmission,
    ),
    dispatch_return: Result<typeck::TypedOwnedProgram<'static>, Vec<Diagnostic>>,
    owned_completion: resolve::ResolvedOwnedProgram<'static>,
    completed: Result<typeck::TypedOwnedProgram<'static>, Vec<Diagnostic>>,
    completion_inputs: (&'static HirPlan, &'static mut Allocator, usize),
    guard_receivers: [&'static DeclarationIndex<'static>; 5],
    guard_returns: [bool; 5],
    admission: [resolve::SourceAdmission; 3],
    refusals: [bool; 3],
    plan_argument: &'static HirPlan,
    plan: HirPlan,
    preflight_return: Result<Option<HirPlan>, Box<Diagnostic>>,
    wrapper_return: Result<Option<HirPlan>, Box<Diagnostic>>,
    counts: super::hir_budget::HirCounts,
    owned_keys: usize,
    owned_parameters: usize,
    limit: usize,
    row_bytes: [usize; 3],
    checked: [Option<usize>; 4],
    additions: [Result<usize, Box<Diagnostic>>; 4],
}

#[allow(dead_code)]
struct Carriers {
    signatures: [SignatureRoles; 4],
    typing: TypingRoles,
    association_storage: [AssociationStorageRoles; 2],
}

#[allow(dead_code)]
struct AssociationStorageRoles {
    typed: &'static typeck::TypedOwnedProgram<'static>,
    index: &'static DeclarationIndex<'static>,
    work: &'static WorkMeter,
    source: crate::frontend::declaration_index::SourceOwner<'static>,
    at: Span,
    preflight: Result<Option<HirPlan>, Box<Diagnostic>>,
    selected: HirPlan,
    cell: &'static std::cell::Cell<usize>,
    current: usize,
    dynamic: Option<usize>,
    returned: Result<usize, Box<Diagnostic>>,
    admission: resolve::SourceAdmission,
    selected_domain: bool,
}

pub(super) const fn carrier_bytes() -> usize {
    std::mem::size_of::<Carriers>() + typeck::owned_vector_formatter_carrier_bytes()
}

#[cfg(test)]
#[path = "owned_vector_source_tests.rs"]
mod tests;
