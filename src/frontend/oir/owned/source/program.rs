//! Source-only construction keeps the owned witness sealed in this child module.
use super::super::{execute, native, verified, *};
use super::{diagnostic, lower, resolve, typeck};
use crate::frontend::{ast, source::SourceFile};
use crate::frontend::{
    declaration_index::{self as index, IndexLimits, SourceOwner, WorkMeter},
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

/// Keep the fresh index and paid typed owner local until ordinary source
/// lowering, association and the complete raw verifier have finished.
pub(in crate::frontend::oir) fn check_enum_source(
    owner: SourceOwner<'_>,
) -> Result<(SourceProgram, Option<hir::DefId>), Vec<Diagnostic>> {
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts = index::collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
        .map_err(|error| vec![*error])?;
    let index = facts.finish(&work, &mut allocator)?;
    let typed = resolve::type_enum_source(&index, &work, &mut allocator)?;
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
    pub(super) result: Result<Scalar, execute::OwnedRunFailure>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(super) struct EnumPipelineProgramOutput {
    pub(super) facts: EnumPipelineFacts,
    pub(super) llvm: Option<Result<String, Box<Diagnostic>>>,
}

/// Sole caller is the paid typeck continuation. Fixed runtime observations and
/// bounded artifacts leave only after the same verified witness is dropped.
#[cfg(test)]
#[allow(dead_code)]
pub(super) fn observe_enum_pipeline(
    typed: &typeck::TypedOwnedProgram<'_>,
    request: resolve::EnumPipelineRequest,
) -> Result<EnumPipelineProgramOutput, Vec<Diagnostic>> {
    if typed.admission() != resolve::SourceAdmission::EnumPipeline {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    observe_private_pipeline(typed, request)
}

/// One fresh source-owned entry. Candidate identity, paid typing, association
/// and raw verification all remain inside this call; only fixed observations
/// and bounded LLVM text can escape after those owners are dropped.
#[cfg(test)]
pub(super) fn run_builtin_source(
    owner: SourceOwner<'_>,
    request: resolve::EnumPipelineRequest,
) -> Result<EnumPipelineProgramOutput, Vec<Diagnostic>> {
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts =
        index::collect_builtin_candidate(owner, IndexLimits::default(), &work, &mut allocator)
            .map_err(|error| vec![*error])?;
    let index = facts.finish(&work, &mut allocator)?;
    let typed = resolve::type_builtin_source(&index, &work, &mut allocator)?;
    if typed.admission() != resolve::SourceAdmission::BuiltinPipeline {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    observe_private_pipeline(&typed, request)
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OutputPipelineMode {
    Run,
    Emit,
}
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) struct OutputPipelineRequest {
    pub(super) mode: OutputPipelineMode,
    pub(super) fuel: usize,
}
#[cfg(test)]
#[derive(Debug)]
pub(super) enum OutputPipelineOutput {
    Run(Result<Scalar, execute::OwnedRunFailure>),
    Emit(Result<String, Box<Diagnostic>>),
}

/// One fresh paid source continuation. Emission never runs the reference
/// consumer. Neither an index, typed owner nor executable witness can escape.
#[cfg(test)]
pub(super) fn run_output_source(
    owner: SourceOwner<'_>,
    request: OutputPipelineRequest,
) -> Result<OutputPipelineOutput, Vec<Diagnostic>> {
    const OUTPUT_SOURCE_ENABLED: bool = true;
    if !OUTPUT_SOURCE_ENABLED {
        return Err(vec![*Diagnostic::new(
            "E0101",
            "resolve",
            "output source pipeline is not enabled",
            Some(owner.eof()),
        )]);
    }
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts =
        index::collect_output_candidate(owner, IndexLimits::default(), &work, &mut allocator)
            .map_err(|error| vec![*error])?;
    let index = facts.finish(&work, &mut allocator)?;
    index
        .require_output_candidate_pipeline()
        .map_err(|error| vec![*error])?;
    let typed = resolve::type_builtin_source(&index, &work, &mut allocator)?;
    if typed.admission() != resolve::SourceAdmission::BuiltinPipeline {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    let crate::frontend::source::SourceView::Map(sources) = index.sources().view() else {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    };
    let entry = index.root_original_main();
    if typed.entry() != entry {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    let seed_before = typed
        .source_storage_bytes()
        .ok_or_else(|| vec![*crate::frontend::oir::source::association::bad()])?;
    let source_usage = super::budget::preflight(&typed, super::budget::Limits::DEFAULT)
        .map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
    let raw = lower::lower(&typed).map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
    let raw_usage = super::super::budget::preflight(&raw, super::super::budget::Limits::DEFAULT)
        .map_err(|error| vec![*diagnostic::verify(&error, sources)])?;
    if source_usage.analysis != raw_usage {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    super::association::check_builtin_candidate(&raw, &index, sources)
        .map_err(|error| vec![*error])?;
    let witness = verified::verify_owned(raw, sources)
        .map_err(|error| vec![*diagnostic::verify(&error, sources)])?;
    let seed_after = typed
        .source_storage_bytes()
        .ok_or_else(|| vec![*crate::frontend::oir::source::association::bad()])?;
    if seed_before != seed_after {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    let entry = checked_output_entry(&witness, entry).map_err(|error| vec![*error])?;
    let output = match request.mode {
        OutputPipelineMode::Run => OutputPipelineOutput::Run(execute::run_process_limits(
            &witness,
            Some(entry),
            execute::Limits {
                fuel: request.fuel,
                ..execute::Limits::default()
            },
        )),
        OutputPipelineMode::Emit => OutputPipelineOutput::Emit(
            native::native_process_module_with_fuel(&witness, entry, sources, request.fuel),
        ),
    };
    drop(witness);
    Ok(output)
}

/// Shared source-level process signature rule, before either consumer. The
/// entry identity came from the original-root index and full source association.
#[cfg(test)]
fn checked_output_entry(
    witness: &verified::VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
) -> Result<hir::DefId, Box<Diagnostic>> {
    let function = match entry {
        Some(id) => match witness.functions().get(id.0) {
            Some(function) if function.id == id => Some(function),
            _ => None,
        },
        None => None,
    };
    match function {
        Some(function)
            if function.parameters.is_empty()
                && function.result == ValueTy::Scalar(hir::Ty::I32) =>
        {
            Ok(function.id)
        }
        _ => Err(Diagnostic::new(
            "E0600",
            "oir-run",
            "process entry requires original-root fn main() -> i32 with no parameters",
            match function {
                Some(function) => Some(function.span),
                None => None,
            },
        )),
    }
}

// Complete new named caller surface, in addition to existing HIR/raw/analysis
// banks. Index collection/finish storage is independently admitted by IndexPlan.
// This is not a claim to model inherited helpers, diagnostics, stack or RSS.
#[cfg(test)]
#[allow(dead_code)]
struct OutputProgramCarriers {
    owner: SourceOwner<'static>,
    request: OutputPipelineRequest,
    mode: OutputPipelineMode,
    work: WorkMeter,
    allocator: Allocator,
    index: index::DeclarationIndex<'static>,
    index_borrows: [&'static index::DeclarationIndex<'static>; 4],
    work_borrow: &'static WorkMeter,
    allocator_borrow: &'static mut Allocator,
    marker: bool,
    gate: Result<(), Box<Diagnostic>>,
    gate_normalized: Result<(), Vec<Diagnostic>>,
    typed_return: Result<typeck::TypedOwnedProgram<'static>, Vec<Diagnostic>>,
    typed: typeck::TypedOwnedProgram<'static>,
    typed_borrows: [&'static typeck::TypedOwnedProgram<'static>; 3],
    source_view: crate::frontend::source::SourceView<'static>,
    sources: &'static SourceMap,
    source_captures: [&'static SourceMap; 4],
    entries: [Option<hir::DefId>; 2],
    selected_entry: hir::DefId,
    entry_return: Result<hir::DefId, Box<Diagnostic>>,
    entry_normalized: Result<hir::DefId, Vec<Diagnostic>>,
    entry_function: Option<&'static RawOwnedFunction>,
    entry_predicate: hir::DefId,
    entry_span: Option<crate::frontend::source::Span>,
    seed_options: [Option<usize>; 2],
    seed_returns: [Result<usize, Vec<Diagnostic>>; 2],
    seeds: [usize; 2],
    source_return: Result<super::budget::Usage, OwnedFailure>,
    source_normalized: Result<super::budget::Usage, Vec<Diagnostic>>,
    source_usage: super::budget::Usage,
    raw_return: Result<RawOwnedProgram, OwnedFailure>,
    raw_normalized: Result<RawOwnedProgram, Vec<Diagnostic>>,
    raw: RawOwnedProgram,
    raw_borrows: [&'static RawOwnedProgram; 2],
    raw_usage_return: Result<OwnershipUsage, OwnedFailure>,
    raw_usage_normalized: Result<OwnershipUsage, Vec<Diagnostic>>,
    raw_usage: OwnershipUsage,
    association_return:
        Result<crate::frontend::oir::source::association::BindUsage, Box<Diagnostic>>,
    association_normalized:
        Result<crate::frontend::oir::source::association::BindUsage, Vec<Diagnostic>>,
    witness_return: Result<verified::VerifiedOwnedProgram, OwnedFailure>,
    witness_normalized: Result<verified::VerifiedOwnedProgram, Vec<Diagnostic>>,
    witness: verified::VerifiedOwnedProgram,
    witness_borrows: [&'static verified::VerifiedOwnedProgram; 3],
    limits: execute::Limits,
    execution_return: Result<Scalar, execute::OwnedRunFailure>,
    native_return: Result<String, Box<Diagnostic>>,
    lower_identity_normalized: Result<(), OwnedFailure>,
    output: OutputPipelineOutput,
    returned: Result<OutputPipelineOutput, Vec<Diagnostic>>,
}
#[cfg(test)]
pub(super) const fn output_program_carrier_bytes() -> usize {
    std::mem::size_of::<OutputProgramCarriers>() + super::association::builtin_carrier_bytes()
}
#[test]
fn bounded_output_source_caller_layout_and_emit_has_no_reference_effects() {
    println!(
        "OUTPUT_SOURCE_CALLER caller={} request={} output={} returned={}",
        output_program_carrier_bytes(),
        std::mem::size_of::<OutputPipelineRequest>(),
        std::mem::size_of::<OutputPipelineOutput>(),
        std::mem::size_of::<Result<OutputPipelineOutput, Vec<Diagnostic>>>()
    );
    let mut sources = SourceMap::new();
    let file = sources.add(
        "closed-output.ox".into(),
        "fn main()->i32{return 0;}".into(),
    );
    let source = sources.get(file);
    let ast = crate::frontend::parser::parse(source, crate::frontend::lexer::lex(source).unwrap())
        .unwrap();
    let owner = SourceOwner::original(
        source,
        &ast,
        crate::frontend::source::SourceView::Map(&sources),
    )
    .unwrap();
    let output = run_output_source(
        owner,
        OutputPipelineRequest {
            mode: OutputPipelineMode::Emit,
            fuel: 0,
        },
    )
    .unwrap();
    let OutputPipelineOutput::Emit(module) = output else {
        panic!("emission must not select the reference consumer");
    };
    let module = module.unwrap();
    assert!(module.contains("@__oxid_process_setup"));
    assert!(module.contains("store i64 0"));
    assert!(!module.contains("call i32 @__oxid_print_i32"));
}

#[cfg(test)]
fn observe_private_pipeline(
    typed: &typeck::TypedOwnedProgram<'_>,
    request: resolve::EnumPipelineRequest,
) -> Result<EnumPipelineProgramOutput, Vec<Diagnostic>> {
    let index = typed.index();
    let crate::frontend::source::SourceView::Map(sources) = index.sources().view() else {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    };
    let entry = index.root_original_main();
    if typed.entry() != entry {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    match typed.admission() {
        resolve::SourceAdmission::EnumPipeline if index.enum_count() != 0 => {
            index
                .require_no_builtin_candidate()
                .map_err(|error| vec![*error])?;
        }
        resolve::SourceAdmission::BuiltinPipeline => {
            index
                .require_builtin_candidate_pipeline()
                .map_err(|error| vec![*error])?;
        }
        _ => return Err(vec![*crate::frontend::oir::source::association::bad()]),
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
    match typed.admission() {
        resolve::SourceAdmission::BuiltinPipeline => {
            super::association::check_builtin_candidate(&raw, index, sources)
        }
        _ => super::association::check_enum_candidate(&raw, index, sources),
    }
    .map_err(|error| vec![*error])?;
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
    let limits = execute::Limits {
        fuel: request.fuel,
        ..execute::Limits::default()
    };
    let result = execute::run_limits(&witness, entry, limits);
    // Runtime failure is inert data: native emission still uses this exact
    // verified witness and the ordinary bounded native_module implementation.
    let llvm = if request.emit_llvm {
        Some(native::native_module(&witness, entry, sources))
    } else {
        None
    };
    let source_seed_after = typed
        .source_storage_bytes()
        .ok_or_else(|| vec![*crate::frontend::oir::source::association::bad()])?;
    if source_seed_after != source_seed_before {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    drop(witness);
    Ok(EnumPipelineProgramOutput {
        facts: EnumPipelineFacts {
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
        },
        llvm,
    })
}

// Only the new closed driver's named caller/control surfaces. Raw backing,
// declaration/verification payloads and execution plans retain their separate
// authoritative budgets; no inherited verifier/runtime stack is modeled here.
#[allow(dead_code)]
struct EnumPipelineProgramCarriers {
    // Publicly inaccessible selector forwards these by value/reference to the
    // common body; price both roles without relying on tail-call elimination.
    request: [resolve::EnumPipelineRequest; 2],
    execution_limits: [execute::Limits; 3],
    typed: [&'static typeck::TypedOwnedProgram<'static>; 2],
    index: &'static crate::frontend::declaration_index::DeclarationIndex<'static>,
    source_owner: crate::frontend::declaration_index::SourceOwner<'static>,
    source_view: crate::frontend::source::SourceView<'static>,
    sources: &'static SourceMap,
    entry: Option<hir::DefId>,
    seed_options: [Option<usize>; 4],
    seed_results: [Result<usize, Vec<Diagnostic>>; 2],
    seeds: [usize; 2],
    source_return: Result<super::budget::Usage, OwnedFailure>,
    source_normalized: Result<super::budget::Usage, Vec<Diagnostic>>,
    source_usage: super::budget::Usage,
    raw_return: Result<RawOwnedProgram, OwnedFailure>,
    raw_normalized: Result<RawOwnedProgram, Vec<Diagnostic>>,
    raw: RawOwnedProgram,
    raw_borrows: [&'static RawOwnedProgram; 2],
    raw_usage_return: Result<OwnershipUsage, OwnedFailure>,
    raw_usage_normalized: Result<OwnershipUsage, Vec<Diagnostic>>,
    raw_usage: OwnershipUsage,
    association_return:
        Result<crate::frontend::oir::source::association::BindUsage, Box<Diagnostic>>,
    association_normalized:
        Result<crate::frontend::oir::source::association::BindUsage, Vec<Diagnostic>>,
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
    additions_normalized: [Result<usize, Vec<Diagnostic>>; 3],
    work_borrows: [&'static crate::frontend::declaration_index::WorkMeter; 3],
    visits: [Result<(), Box<Diagnostic>>; 3],
    visits_normalized: [Result<(), Vec<Diagnostic>>; 3],
    witness_return: Result<verified::VerifiedOwnedProgram, OwnedFailure>,
    witness_normalized: Result<verified::VerifiedOwnedProgram, Vec<Diagnostic>>,
    witness: verified::VerifiedOwnedProgram,
    witness_borrows: [&'static verified::VerifiedOwnedProgram; 2],
    verified_usage: OwnershipUsage,
    execution_return: Result<Scalar, execute::OwnedRunFailure>,
    result: Result<Scalar, execute::OwnedRunFailure>,
    native_witness: &'static verified::VerifiedOwnedProgram,
    native_entry: Option<hir::DefId>,
    native_sources: &'static SourceMap,
    native_return: Result<String, Box<Diagnostic>>,
    native_options: [Option<Result<String, Box<Diagnostic>>>; 2],
    constructed_facts: EnumPipelineFacts,
    constructed: EnumPipelineProgramOutput,
    returned: Result<EnumPipelineProgramOutput, Vec<Diagnostic>>,
}
pub(super) const fn enum_pipeline_program_carrier_bytes() -> usize {
    std::mem::size_of::<EnumPipelineProgramCarriers>()
}

// New original-file caller surfaces. The index's collection/finish prefix is
// independently admitted by IndexPlan. These owners remain live during fresh
// paid typing; downstream raw/lower/verifier roles keep their existing banks.
#[allow(dead_code)]
struct ProductionProgramCarriers {
    owner: SourceOwner<'static>,
    work: WorkMeter,
    allocator: Allocator,
    index: index::DeclarationIndex<'static>,
    index_borrow: &'static index::DeclarationIndex<'static>,
    work_borrow: &'static WorkMeter,
    allocator_borrow: &'static mut Allocator,
    typed_return: Result<typeck::TypedOwnedProgram<'static>, Vec<Diagnostic>>,
    typed: typeck::TypedOwnedProgram<'static>,
    typed_borrow: &'static typeck::TypedOwnedProgram<'static>,
    entry: Option<hir::DefId>,
    program_return: Result<SourceProgram, Vec<Diagnostic>>,
    program: SourceProgram,
    tuple: (SourceProgram, Option<hir::DefId>),
    returned: Result<(SourceProgram, Option<hir::DefId>), Vec<Diagnostic>>,
}
pub(super) const fn production_program_carrier_bytes() -> usize {
    std::mem::size_of::<ProductionProgramCarriers>()
}

/// The private input caller's affected owner/return roles. Index collection is
/// admitted separately by IndexPlan; source typing and the shared observation
/// body keep their existing complete banks. No index or typed owner escapes.
#[allow(dead_code)]
struct BuiltinProgramCarriers {
    owner: SourceOwner<'static>,
    work: WorkMeter,
    allocator: Allocator,
    index: index::DeclarationIndex<'static>,
    index_borrow: &'static index::DeclarationIndex<'static>,
    work_borrow: &'static WorkMeter,
    allocator_borrow: &'static mut Allocator,
    typed_return: Result<typeck::TypedOwnedProgram<'static>, Vec<Diagnostic>>,
    typed: typeck::TypedOwnedProgram<'static>,
    typed_borrow: &'static typeck::TypedOwnedProgram<'static>,
    request: resolve::EnumPipelineRequest,
    gate: Result<(), Box<Diagnostic>>,
    gate_normalized: Result<(), Vec<Diagnostic>>,
    // Source preflight normalizes its typed identity check to OwnedFailure;
    // the resolver's boxed/Vec diagnostic envelopes do not contain this role.
    lower_identity_normalized: Result<(), OwnedFailure>,
    returned: Result<EnumPipelineProgramOutput, Vec<Diagnostic>>,
}
pub(super) const fn builtin_program_carrier_bytes() -> usize {
    std::mem::size_of::<BuiltinProgramCarriers>() + super::association::builtin_carrier_bytes()
}

/// Production uses the already-priced ordinary source caller, not the private
/// observation driver. Only the builtin association and typed-to-raw identity
/// normalization add new roles to that existing route.
pub(super) const fn builtin_production_extra_bytes() -> usize {
    super::association::builtin_carrier_bytes() + std::mem::size_of::<Result<(), OwnedFailure>>()
}

#[test]
fn bounded_enum_production_program_caller_layout() {
    println!(
        "ENUM_PRODUCTION_PROGRAM_LAYOUT controls={} source={}",
        production_program_carrier_bytes(),
        std::mem::size_of::<SourceProgram>()
    );
}
