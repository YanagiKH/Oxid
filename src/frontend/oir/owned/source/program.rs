//! Source-only construction keeps the owned witness sealed in this child module.
use super::super::{execute, native, verified, *};
use super::{diagnostic, lower, resolve, typeck};
use crate::frontend::{ast, source::SourceFile};
#[cfg(test)]
use crate::frontend::{
    declaration_index::{IndexLimits, SourceOwner, WorkMeter},
    project::budget::Allocator,
};

#[derive(Debug)]
pub(in crate::frontend::oir) struct SourceProgram {
    witness: verified::VerifiedOwnedProgram,
}

pub(in crate::frontend::oir) fn check_source(
    source: &SourceFile,
    ast: &ast::Program,
    sources: &SourceMap,
) -> Result<(SourceProgram, Option<hir::DefId>), Vec<Diagnostic>> {
    let resolved = resolve::resolve_in_map(source, ast, sources)?;
    let typed = typeck::check(resolved)?;
    let entry = typed.entry();
    Ok((check_typed(&typed)?, entry))
}

/// The map comes from the checked typed/index owner, never an independent caller.
pub(in crate::frontend::oir) fn check_typed(
    typed: &typeck::TypedOwnedProgram<'_>,
) -> Result<SourceProgram, Vec<Diagnostic>> {
    if !typed.admission().executable() {
        return Err(vec![*crate::frontend::owned_diagnostic::diagnostic(
            "E0500",
            "oir-owned-lower",
            format_args!("internal compiler error: owned invariant violation"),
            Some(typed.index().sources().eof()),
        )]);
    }
    let index = typed.index();
    let crate::frontend::source::SourceView::Map(sources) = index.sources().view() else {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    };
    if typed.entry() != index.root_original_main() {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    let raw = lower::lower(typed).map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
    super::association::check(&raw, index, sources).map_err(|error| vec![*error])?;
    let witness = verified::verify_owned(raw, sources)
        .map_err(|error| vec![*diagnostic::verify(&error, sources)])?;
    Ok(SourceProgram { witness })
}

/// Private consumer qualification remains separate from public SourceProgram
/// admission. No typed owner, raw program or witness is returned by this seam.
#[cfg(test)]
pub(super) fn run_array_source<'s>(
    owner: SourceOwner<'s>,
    limits: IndexLimits,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<Scalar, Vec<Diagnostic>> {
    let typed = typeck::check(resolve::resolve_array_consumer(
        owner, limits, work, allocator,
    )?)?;
    let (raw, sources, entry) = lower_array_consumer(&typed)?;
    let observation = verified::probe_array_reference(
        raw,
        sources,
        super::super::budget::Limits::DEFAULT,
        entry,
        execute::Limits::default(),
        execute::ObservationControl::default(),
    )
    .map_err(|error| vec![*diagnostic::verify(&error, sources)])?;
    observation
        .result
        .map_err(|error| vec![*error.diagnostic(sources)])
}

/// LLVM emission shares the same source-owned admission and association checks.
/// The existing sealed probe returns only bounded module text or a diagnostic.
#[cfg(test)]
pub(super) fn emit_array_source<'s>(
    owner: SourceOwner<'s>,
    limits: IndexLimits,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<String, Vec<Diagnostic>> {
    let typed = typeck::check(resolve::resolve_array_consumer(
        owner, limits, work, allocator,
    )?)?;
    let (raw, sources, entry) = lower_array_consumer(&typed)?;
    let observation = verified::probe_array_native(
        raw,
        sources,
        super::super::budget::Limits::DEFAULT,
        entry,
        sources,
        native::NativeControl::default(),
    )
    .map_err(|error| vec![*diagnostic::verify(&error, sources)])?;
    observation.result.map_err(|error| vec![*error])
}

/// Entry and rendering sources are derived from the checked index, never
/// supplied independently by the caller. The owner stays in its entrypoint.
#[cfg(test)]
fn lower_array_consumer<'s>(
    typed: &'s typeck::TypedOwnedProgram<'_>,
) -> Result<(RawOwnedProgram, &'s SourceMap, Option<hir::DefId>), Vec<Diagnostic>> {
    let index = typed.index();
    let crate::frontend::source::SourceView::Map(sources) = index.sources().view() else {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    };
    let entry = index.root_original_main();
    if typed.admission() != resolve::SourceAdmission::ArrayConsumer || typed.entry() != entry {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    let raw = lower::lower(typed).map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
    super::association::check(&raw, index, sources).map_err(|error| vec![*error])?;
    Ok((raw, sources, entry))
}

impl SourceProgram {
    pub(in crate::frontend::oir) fn function_count(&self) -> usize {
        self.witness.functions().len()
    }

    pub(in crate::frontend::oir) fn run(
        &self,
        entry: Option<hir::DefId>,
        sources: &SourceMap,
    ) -> Result<Scalar, Box<Diagnostic>> {
        execute::run(&self.witness, entry).map_err(|error| error.diagnostic(sources))
    }

    pub(in crate::frontend::oir) fn native_module(
        &self,
        entry: Option<hir::DefId>,
        sources: &SourceMap,
    ) -> Result<String, Box<Diagnostic>> {
        native::native_module(&self.witness, entry, sources)
    }
}

#[cfg(test)]
#[path = "candidate_adapter.rs"]
mod candidate_adapter;

/// Fixed observations from a closed source-to-reference path. This is neither
/// a source owner nor a raw/verified/execution witness.
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) struct EnumPipelineFacts {
    pub(super) source_seed_before: usize,
    pub(super) source_seed_after: usize,
    pub(super) source_usage: super::budget::Usage,
    pub(super) raw_usage: OwnershipUsage,
    pub(super) verified_usage: OwnershipUsage,
    pub(super) enum_count: usize,
    pub(super) variant_count: usize,
    pub(super) function_count: usize,
    pub(super) match_count: usize,
    pub(super) arm_count: usize,
    pub(super) result: Scalar,
}

/// Sole caller is the paid typeck continuation; the fresh driver remains denied
/// until its enclosing carrier layouts have been measured and reviewed.
#[cfg(test)]
#[allow(dead_code)]
pub(super) fn observe_enum_pipeline(
    typed: &typeck::TypedOwnedProgram<'_>,
) -> Result<EnumPipelineFacts, Vec<Diagnostic>> {
    if typed.admission() != resolve::SourceAdmission::EnumPipeline {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    let index = typed.index();
    let crate::frontend::source::SourceView::Map(sources) = index.sources().view() else {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    };
    let entry = index.root_original_main();
    if typed.entry() != entry || index.enum_count() == 0 {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    let source_seed_before = typed
        .source_storage_bytes()
        .ok_or_else(|| vec![*crate::frontend::oir::source::association::bad()])?;
    let source_usage = super::budget::preflight(typed, super::budget::Limits::DEFAULT)
        .map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
    let raw = lower::lower(typed).map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
    let raw_usage = super::super::budget::preflight(&raw, super::super::budget::Limits::DEFAULT)
        .map_err(|error| vec![*diagnostic::verify(&error, sources)])?;
    if source_usage.analysis != raw_usage {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    super::association::check_enum_candidate(&raw, index, sources).map_err(|error| vec![*error])?;
    // Only fixed header facts are sampled here. This does not replace either
    // source association or the independent complete raw proof below.
    let enum_count = raw.enums.len();
    let function_count = raw.functions.len();
    let mut variant_count = 0;
    let mut match_count = 0;
    let mut arm_count = 0;
    for enumeration in &raw.enums {
        typed
            .work()
            .debit(1, enumeration.span, "enum pipeline facts")
            .map_err(|error| {
                typed.work().record_error(&error);
                vec![*error]
            })?;
        variant_count = super::budget::add(variant_count, enumeration.variants.len())
            .map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
    }
    for function in &raw.functions {
        typed
            .work()
            .debit(1, function.span, "enum pipeline facts")
            .map_err(|error| {
                typed.work().record_error(&error);
                vec![*error]
            })?;
        match_count = super::budget::add(match_count, function.matches.len())
            .map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
        for descriptor in &function.matches {
            typed
                .work()
                .debit(1, descriptor.span, "enum pipeline facts")
                .map_err(|error| {
                    typed.work().record_error(&error);
                    vec![*error]
                })?;
            arm_count = super::budget::add(arm_count, descriptor.arms.len())
                .map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
        }
    }
    // Use the authoritative path including prepare, inventory_carriers and
    // shape/CFG/flow validation. No probe-only shortcut or second runtime.
    let witness = verified::verify_owned(raw, sources)
        .map_err(|error| vec![*diagnostic::verify(&error, sources)])?;
    let verified_usage = witness.usage();
    // checked_entry retains ordinary main parameter/result rejection before
    // execution-plan allocation. Ordinary relay functions may return enums.
    let result = execute::run(&witness, entry).map_err(|error| vec![*error.diagnostic(sources)])?;
    let source_seed_after = typed
        .source_storage_bytes()
        .ok_or_else(|| vec![*crate::frontend::oir::source::association::bad()])?;
    if source_seed_after != source_seed_before {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    drop(witness);
    Ok(EnumPipelineFacts {
        source_seed_before,
        source_seed_after,
        source_usage,
        raw_usage,
        verified_usage,
        enum_count,
        variant_count,
        function_count,
        match_count,
        arm_count,
        result,
    })
}

// Only the new closed driver's named caller/control surfaces. Raw backing,
// declaration/verification payloads and execution plans retain their separate
// authoritative budgets; no inherited verifier/runtime stack is modeled here.
#[allow(dead_code)]
struct EnumPipelineProgramCarriers {
    typed: &'static typeck::TypedOwnedProgram<'static>,
    index: &'static crate::frontend::declaration_index::DeclarationIndex<'static>,
    source_owner: crate::frontend::declaration_index::SourceOwner<'static>,
    source_view: crate::frontend::source::SourceView<'static>,
    sources: &'static SourceMap,
    entry: Option<hir::DefId>,
    seed_options: [Option<usize>; 4],
    seed_results: [Result<usize, Vec<Diagnostic>>; 2],
    seeds: [usize; 2],
    source_return: Result<super::budget::Usage, OwnedFailure>,
    source_usage: super::budget::Usage,
    raw_return: Result<RawOwnedProgram, OwnedFailure>,
    raw: RawOwnedProgram,
    raw_borrows: [&'static RawOwnedProgram; 2],
    raw_usage_return: Result<OwnershipUsage, OwnedFailure>,
    raw_usage: OwnershipUsage,
    association_return:
        Result<crate::frontend::oir::source::association::BindUsage, Box<Diagnostic>>,
    association_payload: crate::frontend::oir::source::association::BindUsage,
    enum_count: usize,
    function_count: usize,
    variant_count: usize,
    match_count: usize,
    arm_count: usize,
    enums: std::slice::Iter<'static, RawEnumDecl>,
    enumeration: &'static RawEnumDecl,
    enum_next: Option<&'static RawEnumDecl>,
    functions: std::slice::Iter<'static, RawOwnedFunction>,
    function: &'static RawOwnedFunction,
    function_next: Option<&'static RawOwnedFunction>,
    descriptors: std::slice::Iter<'static, MatchDecl>,
    descriptor: &'static MatchDecl,
    descriptor_next: Option<&'static MatchDecl>,
    additions: [Result<usize, OwnedFailure>; 3],
    work_borrows: [&'static crate::frontend::declaration_index::WorkMeter; 3],
    visits: [Result<(), Box<Diagnostic>>; 3],
    witness_return: Result<verified::VerifiedOwnedProgram, OwnedFailure>,
    witness: verified::VerifiedOwnedProgram,
    witness_borrows: [&'static verified::VerifiedOwnedProgram; 2],
    verified_usage: OwnershipUsage,
    execution_return: Result<Scalar, execute::OwnedRunFailure>,
    result: Scalar,
    constructed: EnumPipelineFacts,
    returned: Result<EnumPipelineFacts, Vec<Diagnostic>>,
}
pub(super) const fn enum_pipeline_program_carrier_bytes() -> usize {
    std::mem::size_of::<EnumPipelineProgramCarriers>()
}
