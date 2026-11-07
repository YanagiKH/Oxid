//! The only production owner of the source/body/entry association. No child modules.
use super::super::{
    owned,
    project::{CheckedProjectTypes, ProjectRoute},
    *,
};
use super::association::{self, Declarations};
use crate::frontend::{
    ast,
    build_config::NativeSourceNames,
    declaration_index::{self as index, IndexLimits, SourceOwner, WorkMeter},
    project::{
        budget::{Allocator, ReserveFailure},
        ProjectSources, SyntaxFlavor,
    },
    source::{SourceFile, SourceView, Span},
};

// Keep the witness inline: its enum growth is the already-admitted raw vector
// header and checked declaration facade. Boxing would add an unbudgeted allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum CheckedBody {
    Scalar(VerifiedProgram),
    Owned(owned::SourceProgram),
    OwnedVector(Box<[owned::source::OwnedVectorSourceProgram; 1]>),
}

/// Borrows only the immutable source map, never an index, AST or type-pass state.
/// Complete source pipelines below are the only ways to install this tuple.
#[derive(Debug)]
pub(in crate::frontend) struct CheckedSourceProgram<'s> {
    sources: &'s SourceMap,
    body: CheckedBody,
    entry: Option<hir::DefId>,
}

/// Separate opt-in authority: neither an entry option nor a caller-supplied
/// DefId can construct a source library. Existing checked owners stay unchanged.
#[derive(Debug)]
pub(in crate::frontend) struct CheckedScalarLibrary<'s> {
    sources: &'s SourceMap,
    program: VerifiedProgram,
    export: hir::DefId,
    parameters: usize,
    limits: IndexLimits,
}

#[allow(dead_code)]
struct LibrarySourceBank {
    owners: [CheckedScalarLibrary<'static>; 3],
    inputs: [(
        &'static ProjectSources,
        &'static str,
        IndexLimits,
        &'static WorkMeter,
        &'static mut Allocator,
    ); 2],
    selection: [Option<hir::DefId>; 2],
    declaration: [(
        &'static ast::Function,
        crate::frontend::project::FunctionAstKey,
        hir::DefId,
    ); 2],
    lookup: [Result<hir::DefId, Box<Diagnostic>>; 2],
    index: &'static index::DeclarationIndex<'static>,
    counts: [usize; 4],
    diagnostic_values: [Diagnostic; 2],
    diagnostic_lists: [Vec<Diagnostic>; 2],
    diagnostic_messages: [[u8; LIBRARY_SOURCE_ERROR.len()]; 2],
}

const LIBRARY_SOURCE_ERROR: &str = "scalar libraries support only bool, i32 and unit helper bodies without owned values, references, arrays, resources, generics or I/O";

pub(in crate::frontend) fn check_scalar_library<'s>(
    project: &'s ProjectSources,
    export: &str,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<CheckedScalarLibrary<'s>, Vec<Diagnostic>> {
    let failure = |message: &'static str| {
        let message = if message.len() <= LIBRARY_SOURCE_ERROR.len() {
            message
        } else {
            "internal compiler error: unpriced library source diagnostic"
        };
        Diagnostic::new("E0700", "native-library", message, None)
    };
    let name = crate::frontend::native_library::export_name(export).ok_or_else(|| {
        vec![*failure(
            "library export requires exactly crate::<ASCII identifier>",
        )]
    })?;
    let banks = std::mem::size_of::<LibrarySourceBank>() as u64;
    // New export/constructor transports are additive. The old index limit and
    // all original source/index carriers retain their previous values/layouts.
    let limits = IndexLimits {
        retained: limits
            .retained
            .min(IndexLimits::default().retained)
            .checked_sub(banks)
            .ok_or_else(|| vec![*failure("library export carrier budget exceeded")])?,
        scratch: limits
            .scratch
            .min(IndexLimits::default().scratch)
            .checked_sub(banks)
            .ok_or_else(|| vec![*failure("library export carrier budget exceeded")])?,
        work: limits.work.min(IndexLimits::default().work),
    };
    let sources = SourceOwner::project(project);
    // Feature facts are denial only. Every loaded module participates, including
    // unused declarations; only the complete scalar pipeline can mint authority.
    if sources.owned(work).map_err(|e| vec![*e])? {
        return Err(vec![*failure(LIBRARY_SOURCE_ERROR)]);
    }
    for module in project.modules() {
        work.debit(1, sources.eof(), "library module admission")
            .map_err(|e| vec![*e])?;
        let ast = project
            .try_file_ast(module.file)
            .ok_or_else(|| vec![*association::bad()])?;
        if !ast.generic_headers().is_empty() {
            return Err(vec![*failure("scalar libraries do not support generic declarations, including unused declarations")]);
        }
    }
    let facts = index::collect_originals(sources, limits, work, allocator).map_err(|e| vec![*e])?;
    let frozen = facts.finish(work, allocator)?;
    let root = project
        .modules()
        .first()
        .ok_or_else(|| vec![*association::bad()])?;
    let ast = project
        .try_file_ast(root.file)
        .ok_or_else(|| vec![*association::bad()])?;
    let mut selected = None;
    for (ordinal, function) in ast.functions.iter().enumerate() {
        work.debit(
            (128 + name.len()) as u64,
            function.name,
            "library export selection",
        )
        .map_err(|e| vec![*e])?;
        if project.sources().text(function.name) != name {
            continue;
        }
        if function.public.is_none() {
            return Err(vec![*failure(
                "library export must name an explicitly public root function",
            )]);
        }
        let key = crate::frontend::project::FunctionAstKey {
            file: root.file,
            index: ordinal,
        };
        let id = frozen.def_for(key).map_err(|e| vec![*e])?;
        if frozen.function(id).map_err(|e| vec![*e])?.0 != key || selected.replace(id).is_some() {
            return Err(vec![*association::bad()]);
        }
    }
    let export = selected.ok_or_else(|| {
        vec![*failure(
            "library export does not name a declared public root function",
        )]
    })?;
    let resolved = hir::resolve_project(&frozen, work)?;
    let typed = typeck::check(resolved)?;
    let raw = lower::lower(&typed).map_err(|e| vec![*e.diagnostic(project.sources())])?;
    let associated =
        association::authenticate_scalar(raw, project.sources(), Declarations::Project(&frozen))
            .map_err(|e| vec![*e])?;
    let program = verify::verify_associated(associated)
        .map_err(|e| vec![*e.diagnostic(project.sources())])?;
    let function = program
        .program
        .functions
        .get(export.0)
        .filter(|f| f.id == export)
        .ok_or_else(|| vec![*association::bad()])?;
    if function.result != hir::Ty::I32
        || function
            .locals
            .iter()
            .take(function.param_count)
            .any(|p| p.ty != hir::Ty::I32)
    {
        return Err(vec![*failure(
            "library export requires i32 parameters and an i32 result",
        )]);
    }
    let parameters = function.param_count;
    Ok(CheckedScalarLibrary {
        sources: project.sources(),
        program,
        export,
        parameters,
        limits,
    })
}

impl CheckedScalarLibrary<'_> {
    pub(in crate::frontend) fn parameters(&self) -> usize {
        self.parameters
    }

    pub(in crate::frontend) fn native_module(
        &self,
        fuel: usize,
        work: &WorkMeter,
        allocator: &mut Allocator,
    ) -> Result<String, Box<Diagnostic>> {
        native::library::module_with_limits(
            &self.program,
            self.export,
            self.sources,
            fuel,
            self.limits,
            work,
            allocator,
        )
    }
}

pub(in crate::frontend) fn check_source<'s>(
    source: &SourceFile,
    ast: &ast::Program,
    sources: &'s SourceMap,
) -> Result<CheckedSourceProgram<'s>, Vec<Diagnostic>> {
    let owner =
        SourceOwner::original(source, ast, SourceView::Map(sources)).map_err(|e| vec![*e])?;
    let (body, entry) = if !ast.enums.is_empty()
        || ast.has_range_for()
        || ast.uses_owned_syntax(source)
        || ast.uses_byte_scalars(source)
        || ast.uses_bitwise_syntax()
        || ast.uses_float_syntax(source)
        || ast.uses_character_syntax(source)
    {
        // The freshly frozen index selects paid typing for every owned source
        // feature. AST routing chooses only the scalar/owned producer family.
        let (program, entry) = owned::source::check_enum_source(owner)?;
        (CheckedBody::Owned(program), entry)
    } else {
        // Preserve the original scalar schedule and diagnostic adapters exactly.
        let resolved = hir::resolve(source, ast)?;
        let entry = ast
            .functions
            .iter()
            .zip(&resolved.functions)
            .find(|(declaration, _)| source.text_at(declaration.name) == "main")
            .map(|(_, function)| function.id);
        let typed = typeck::check(resolved)?;
        let raw = lower::lower(&typed).map_err(|e| vec![*e.diagnostic(sources)])?;
        let associated =
            association::authenticate_scalar(raw, sources, Declarations::Original(ast))
                .map_err(|e| vec![*e])?;
        let verified =
            verify::verify_associated(associated).map_err(|e| vec![*e.diagnostic(sources)])?;
        (CheckedBody::Scalar(verified), entry)
    };
    Ok(CheckedSourceProgram {
        sources,
        body,
        entry,
    })
}

#[derive(Clone, Copy)]
enum CheckDepth {
    Types,
    Executable,
}
// This allocation-free result contains the same already-accounted inline witness.
#[allow(clippy::large_enum_variant)]
enum Checked<'s> {
    Types(CheckedProjectTypes),
    Executable(CheckedSourceProgram<'s>),
}

/// Failure-only routing owns no retained permit. All full return/carrier roles
/// are paid by the new lane; original diagnostic backing is counted separately
/// from these headers and is released before paid HIR or raw construction.
#[allow(dead_code)]
struct OwnedVectorRouteBank {
    project: &'static ProjectSources,
    sources: SourceOwner<'static>,
    work: &'static WorkMeter,
    allocator: &'static mut Allocator,
    limits: [IndexLimits; 3],
    route: ProjectRoute,
    depth: CheckDepth,
    original: [Vec<Diagnostic>; 3],
    diagnostics: std::slice::Iter<'static, Diagnostic>,
    diagnostic: Option<&'static Diagnostic>,
    eligibility: std::slice::Iter<'static, Diagnostic>,
    eligibility_row: Option<&'static Diagnostic>,
    secondaries: std::slice::Iter<'static, (Span, String)>,
    secondary: Option<&'static (Span, String)>,
    notes: std::slice::Iter<'static, String>,
    note: Option<&'static String>,
    candidate: index::DeclarationIndex<'static>,
    candidate_return: Result<index::DeclarationIndex<'static>, Vec<Diagnostic>>,
    typed: owned::source::typeck::TypedOwnedProgram<'static>,
    typed_return: Result<owned::source::typeck::TypedOwnedProgram<'static>, Vec<Diagnostic>>,
    completed: owned::source::CandidateSourceProgram,
    completion_return: Result<owned::source::CandidateSourceProgram, Vec<Diagnostic>>,
    owned: owned::source::OwnedVectorSourceProgram,
    boxed: Box<[owned::source::OwnedVectorSourceProgram; 1]>,
    boxed_return: Result<Box<[owned::source::OwnedVectorSourceProgram; 1]>, ReserveFailure>,
    body: CheckedBody,
    constructed: CheckedSourceProgram<'static>,
    returned: Result<Checked<'static>, Vec<Diagnostic>>,
    signature_entry: Option<hir::DefId>,
    summary: CheckedProjectTypes,
    inventory: crate::frontend::builtin_catalog::experimental::Inventory,
    source_needed: bool,
    new_imports: bool,
    user_error: bool,
    import_presence: [bool; 2],
    diagnostic_capacity: usize,
    held_bytes: usize,
    metadata: usize,
    sizes: [Option<usize>; 4],
    retained_scope: Option<u64>,
    scratch_scope: Option<u64>,
    metadata_scope: Option<usize>,
    admitted: Result<usize, Box<Diagnostic>>,
    at: Span,
}

pub(in crate::frontend::oir) const fn owned_vector_route_carrier_bytes() -> usize {
    std::mem::size_of::<OwnedVectorRouteBank>()
}

pub(super) fn owned_route_storage(
    errors: &[Diagnostic],
    capacity: usize,
    work: &WorkMeter,
    at: Span,
) -> Result<usize, Box<Diagnostic>> {
    let failure = || {
        crate::frontend::owned_diagnostic::diagnostic(
            "E0400",
            "source",
            format_args!("owned vector source routing storage limit exceeded"),
            Some(at),
        )
    };
    if capacity < errors.len() {
        return Err(failure());
    }
    let mut bytes = capacity
        .checked_mul(std::mem::size_of::<Diagnostic>())
        .ok_or_else(failure)?;
    for error in errors {
        work.debit(1, at, "owned vector retained diagnostic")?;
        bytes = bytes
            .checked_add(error.message.capacity())
            .and_then(|n| {
                n.checked_add(
                    error
                        .secondary
                        .capacity()
                        .checked_mul(std::mem::size_of::<(Span, String)>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    error
                        .notes
                        .capacity()
                        .checked_mul(std::mem::size_of::<String>())?,
                )
            })
            .ok_or_else(failure)?;
        for (_, message) in &error.secondary {
            work.debit(1, at, "owned vector retained secondary diagnostic")?;
            bytes = bytes.checked_add(message.capacity()).ok_or_else(failure)?;
        }
        for note in &error.notes {
            work.debit(1, at, "owned vector retained diagnostic note")?;
            bytes = bytes.checked_add(note.capacity()).ok_or_else(failure)?;
        }
    }
    bytes
        .checked_add(owned_vector_route_carrier_bytes())
        .ok_or_else(failure)
}

#[allow(clippy::too_many_arguments)]
fn owned_vector_fallback<'s>(
    project: &'s ProjectSources,
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
    depth: CheckDepth,
    route: ProjectRoute,
    original: Vec<Diagnostic>,
    #[cfg(test)] mut observations: index::SemanticObservationTransaction<'_>,
) -> Result<Checked<'s>, Vec<Diagnostic>> {
    if route != ProjectRoute::Owned || original.is_empty() {
        return Err(original);
    }
    let at = sources.eof();
    for error in &original {
        if !(error.code.starts_with("E02") || error.code.starts_with("E03"))
            || work
                .debit(1, at, "owned vector fallback error family")
                .is_err()
        {
            return Err(original);
        }
    }
    let Ok(held) = owned_route_storage(&original, original.capacity(), work, at) else {
        return Err(original);
    };
    let defaults = IndexLimits::default();
    let (Some(retained), Some(scratch), Some(metadata)) = (
        limits
            .retained
            .min(defaults.retained)
            .checked_sub(held as u64),
        limits
            .scratch
            .min(defaults.scratch)
            .checked_sub(held as u64),
        (8usize * 1024 * 1024).checked_sub(held),
    ) else {
        return Err(original);
    };
    let candidate_limits = IndexLimits {
        retained,
        scratch,
        work: limits.work.min(defaults.work),
    };
    #[cfg(test)]
    observations.candidate_started();
    let candidate = match index::collect_owned_vector_candidate(
        sources,
        candidate_limits,
        metadata,
        work,
        allocator,
    ) {
        Ok(candidate) => candidate,
        Err(_) => return Err(original),
    };
    let inventory = candidate.builtin_set().experimental();
    let new_imports = inventory
        .contains_owned_vector(crate::frontend::builtin_catalog::owned_vector::Item::PushReserved)
        || inventory
            .contains_owned_vector(crate::frontend::builtin_catalog::owned_vector::Item::PopLast);
    if !candidate.owned_vector_source_needed() && !new_imports {
        return Err(original);
    }
    #[cfg(test)]
    observations.candidate_selected();
    // Only this exact source fact, not an error message or a guessed type name,
    // replaces the old diagnostic. Its backing is gone before fresh paid typing.
    drop(original);
    let typed = owned::source::resolve::type_owned_vector_source(&candidate, work, allocator)?;
    let entry = candidate.root_original_main();
    if typed.entry() != entry {
        return Err(vec![*association::bad()]);
    }
    if matches!(depth, CheckDepth::Types) {
        return Ok(Checked::Types(CheckedProjectTypes::new(
            ProjectRoute::Owned,
            candidate.function_count(),
            candidate.record_count(),
            entry,
        )));
    }
    let body = match owned::source::check_owned_vector_candidate_typed(&typed)? {
        owned::source::CandidateSourceProgram::Legacy(program) => CheckedBody::Owned(program),
        owned::source::CandidateSourceProgram::OwnedVector(program) => {
            let boxed = allocator
                .boxed_carrier(program, "owned vector source body")
                .map_err(|_| {
                    vec![*crate::frontend::owned_diagnostic::diagnostic(
                        "E0400",
                        "source",
                        format_args!("owned vector source body allocation failed"),
                        Some(at),
                    )]
                })?;
            CheckedBody::OwnedVector(boxed)
        }
    };
    Ok(Checked::Executable(CheckedSourceProgram {
        sources: project.sources(),
        body,
        entry,
    }))
}

pub(in crate::frontend::oir) fn check_project_candidate(
    project: &ProjectSources,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<CheckedProjectTypes, Vec<Diagnostic>> {
    match check_project(project, limits, work, allocator, CheckDepth::Types)? {
        Checked::Types(checked) => Ok(checked),
        Checked::Executable(_) => Err(vec![*association::bad()]),
    }
}
pub(in crate::frontend::oir) fn check_project_executable_candidate<'s>(
    project: &'s ProjectSources,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<CheckedSourceProgram<'s>, Vec<Diagnostic>> {
    match check_project(project, limits, work, allocator, CheckDepth::Executable)? {
        Checked::Executable(checked) => Ok(checked),
        Checked::Types(_) => Err(vec![*association::bad()]),
    }
}
fn check_project<'s>(
    project: &'s ProjectSources,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
    depth: CheckDepth,
) -> Result<Checked<'s>, Vec<Diagnostic>> {
    let sources = SourceOwner::project(project);
    check_project_owner(project, sources, limits, work, allocator, depth)
}

pub(in crate::frontend::oir) fn check_package_executable<'s>(
    package: &'s crate::frontend::project::packages::PackageSources<'s>,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<CheckedSourceProgram<'s>, Vec<Diagnostic>> {
    match check_project_owner(
        &package.project,
        SourceOwner::packaged(package),
        limits,
        work,
        allocator,
        CheckDepth::Executable,
    )? {
        Checked::Executable(checked) => Ok(checked),
        Checked::Types(_) => Err(vec![*association::bad()]),
    }
}

fn check_project_owner<'s>(
    project: &'s ProjectSources,
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
    depth: CheckDepth,
) -> Result<Checked<'s>, Vec<Diagnostic>> {
    let route = if sources.owned(work).map_err(|e| vec![*e])? {
        ProjectRoute::Owned
    } else {
        ProjectRoute::Scalar
    };
    #[cfg(test)]
    work.observe(crate::frontend::declaration_index::Observation::Route {
        owned: route == ProjectRoute::Owned,
    });
    #[cfg(test)]
    let observations = work.semantic_observation_transaction();
    let facts = match index::collect_originals(sources, limits, work, allocator) {
        Ok(facts) => facts,
        Err(error) => {
            work.record_error(&error);
            return owned_vector_fallback(
                project,
                sources,
                limits,
                work,
                allocator,
                depth,
                route,
                vec![*error],
                #[cfg(test)]
                observations,
            );
        }
    };
    let original_signatures =
        if route == ProjectRoute::Scalar && sources.flavor() == SyntaxFlavor::OriginalSingleFile {
            Some(hir::original_signatures(&facts, work)?)
        } else {
            None
        };
    let frozen = match facts.finish(work, allocator) {
        Ok(frozen) => frozen,
        Err(errors) => {
            drop(original_signatures);
            return owned_vector_fallback(
                project,
                sources,
                limits,
                work,
                allocator,
                depth,
                route,
                errors,
                #[cfg(test)]
                observations,
            );
        }
    };
    let entry = frozen.root_original_main();
    let summary =
        || CheckedProjectTypes::new(route, frozen.function_count(), frozen.record_count(), entry);
    let body = match route {
        ProjectRoute::Scalar => {
            let resolved = match original_signatures {
                Some(signatures) => hir::resolve_bodies(&frozen, work, signatures)?,
                None => hir::resolve_project(&frozen, work)?,
            };
            work.phase("type");
            let typed = typeck::check(resolved).inspect_err(|errors| {
                for error in errors {
                    work.record_error(error)
                }
            })?;
            if matches!(depth, CheckDepth::Types) {
                return Ok(Checked::Types(summary()));
            }
            let raw = lower::lower(&typed).map_err(|e| vec![*e.diagnostic(project.sources())])?;
            let associated = association::authenticate_scalar(
                raw,
                project.sources(),
                Declarations::Project(&frozen),
            )
            .map_err(|e| vec![*e])?;
            CheckedBody::Scalar(
                verify::verify_associated(associated)
                    .map_err(|e| vec![*e.diagnostic(project.sources())])?,
            )
        }
        ProjectRoute::Owned => {
            let typed = if owned::source::resolve::requires_paid_source(&frozen) {
                owned::source::resolve::type_enum_source(&frozen, work, allocator)?
            } else {
                let resolved = owned::source::resolve::resolve_project(&frozen, work)?;
                owned::source::typeck::check(resolved)?
            };
            if matches!(depth, CheckDepth::Types) {
                return Ok(Checked::Types(summary()));
            }
            if typed.entry() != entry {
                return Err(vec![*association::bad()]);
            }
            CheckedBody::Owned(owned::source::check_typed(&typed)?)
        }
    };
    Ok(Checked::Executable(CheckedSourceProgram {
        sources: project.sources(),
        body,
        entry,
    }))
}

/// Process setup failure is kept out of ordinary diagnostic rendering.
#[derive(Debug)]
pub(in crate::frontend) enum ProcessFailure {
    Setup,
    Diagnostic(Box<Diagnostic>),
}
impl From<Box<Diagnostic>> for ProcessFailure {
    fn from(diagnostic: Box<Diagnostic>) -> Self {
        Self::Diagnostic(diagnostic)
    }
}

impl CheckedSourceProgram<'_> {
    pub(in crate::frontend) fn debug_records(&self) -> Option<super::super::DebugRecords<'_>> {
        match &self.body {
            CheckedBody::Scalar(_) => None,
            CheckedBody::Owned(program) => Some(program.debug_records()),
            CheckedBody::OwnedVector(program) => Some(program[0].debug_records()),
        }
    }

    pub(in crate::frontend) fn function_count(&self) -> usize {
        match &self.body {
            CheckedBody::Scalar(program) => program.function_count(),
            CheckedBody::Owned(program) => program.function_count(),
            CheckedBody::OwnedVector(program) => program[0].function_count(),
        }
    }
    #[cfg(test)]
    pub(in crate::frontend) fn run(&self) -> Result<Scalar, Box<Diagnostic>> {
        match &self.body {
            CheckedBody::Scalar(program) => program
                .run(self.entry)
                .map_err(|e| e.diagnostic(self.sources)),
            CheckedBody::Owned(program) => program.run(self.entry, self.sources),
            CheckedBody::OwnedVector(program) => program[0].run(self.entry, self.sources),
        }
    }
    pub(in crate::frontend) fn native_module(&self) -> Result<String, Box<Diagnostic>> {
        match &self.body {
            CheckedBody::Scalar(program) => program.native_module(self.entry, self.sources),
            CheckedBody::Owned(program) => program.native_module_with_execution(
                self.entry,
                self.sources,
                false,
                crate::frontend::build_config::ExecutionPolicy::Bounded,
                None,
            ),
            CheckedBody::OwnedVector(program) => program[0].native_module_with_execution(
                self.entry,
                self.sources,
                false,
                crate::frontend::build_config::ExecutionPolicy::Bounded,
                None,
            ),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)] // Retained narrow facade for private predecessor consumers.
    pub(in crate::frontend) fn run_with_execution(
        &self,
        execution: crate::frontend::build_config::ExecutionPolicy,
    ) -> Result<Scalar, Box<Diagnostic>> {
        if !execution.hosted() {
            return self.run();
        }
        match &self.body {
            CheckedBody::Scalar(program) => program
                .run_with_execution(self.entry, execution)
                .map_err(|error| error.diagnostic(self.sources)),
            CheckedBody::Owned(program) => {
                program.run_with_execution(self.entry, self.sources, execution)
            }
            CheckedBody::OwnedVector(program) => {
                program[0].run_with_execution(self.entry, self.sources, execution)
            }
        }
    }

    pub(in crate::frontend) fn run_result(
        &self,
        execution: crate::frontend::build_config::ExecutionPolicy,
    ) -> Result<RuntimeValue, Box<Diagnostic>> {
        match &self.body {
            CheckedBody::Scalar(program) => program
                .run_with_execution(self.entry, execution)
                .map(RuntimeValue::from)
                .map_err(|error| error.diagnostic(self.sources)),
            CheckedBody::Owned(program) => program.run_result(self.entry, self.sources, execution),
            CheckedBody::OwnedVector(program) => {
                program[0].run_result(self.entry, self.sources, execution)
            }
        }
    }

    #[allow(dead_code)] // Retained checked scalar facade for internal consumers.
    pub(in crate::frontend) fn run_value(
        &self,
        execution: crate::frontend::build_config::ExecutionPolicy,
    ) -> Result<RuntimeScalar, Box<Diagnostic>> {
        match &self.body {
            CheckedBody::Scalar(program) => program
                .run_with_execution(self.entry, execution)
                .map(RuntimeScalar::from)
                .map_err(|error| error.diagnostic(self.sources)),
            CheckedBody::Owned(program) => program.run_value(self.entry, self.sources, execution),
            CheckedBody::OwnedVector(program) => {
                program[0].run_value(self.entry, self.sources, execution)
            }
        }
    }

    pub(in crate::frontend) fn native_module_with_execution(
        &self,
        process: bool,
        execution: crate::frontend::build_config::ExecutionPolicy,
        names: Option<&NativeSourceNames<'_>>,
    ) -> Result<String, Box<Diagnostic>> {
        if !execution.hosted() {
            return match (process, names) {
                (false, None) => self.native_module(),
                (true, None) => self.native_process_module(),
                (false, Some(names)) => self.native_module_with_source_names(names),
                (true, Some(names)) => self.native_process_module_with_source_names(names),
            };
        }
        if process {
            self.process_preflight()?;
        }
        match &self.body {
            CheckedBody::Scalar(program) => program.native_module_with_execution(
                self.entry,
                self.sources,
                process,
                execution,
                names,
            ),
            CheckedBody::Owned(program) => program.native_module_with_execution(
                self.entry,
                self.sources,
                process,
                execution,
                names,
            ),
            CheckedBody::OwnedVector(program) => program[0].native_module_with_execution(
                self.entry,
                self.sources,
                process,
                execution,
                names,
            ),
        }
    }

    pub(in crate::frontend) fn native_optimized_module_with_execution(
        &self,
        process: bool,
        execution: crate::frontend::build_config::ExecutionPolicy,
        names: Option<&NativeSourceNames<'_>>,
        debug: bool,
    ) -> Result<String, Box<Diagnostic>> {
        if process {
            self.process_preflight()?;
        }
        match &self.body {
            CheckedBody::Scalar(_) => {
                if debug {
                    self.native_debug_module_with_execution(process, execution, names)
                } else {
                    self.native_module_with_execution(process, execution, names)
                }
            }
            CheckedBody::Owned(program) => program.native_optimized_module_with_execution(
                self.entry,
                self.sources,
                process,
                execution,
                names,
                debug,
            ),
            CheckedBody::OwnedVector(program) => program[0].native_optimized_module_with_execution(
                self.entry,
                self.sources,
                process,
                execution,
                names,
                debug,
            ),
        }
    }

    pub(in crate::frontend) fn native_debug_module_with_execution(
        &self,
        process: bool,
        execution: crate::frontend::build_config::ExecutionPolicy,
        names: Option<&NativeSourceNames<'_>>,
    ) -> Result<String, Box<Diagnostic>> {
        if process {
            self.process_preflight()?;
        }
        match &self.body {
            CheckedBody::Scalar(program) => program.native_debug_module_with_execution(
                self.entry,
                self.sources,
                process,
                execution,
                names,
            ),
            CheckedBody::Owned(program) => program.native_debug_module_with_execution(
                self.entry,
                self.sources,
                process,
                execution,
                names,
            ),
            CheckedBody::OwnedVector(program) => program[0].native_debug_module_with_execution(
                self.entry,
                self.sources,
                process,
                execution,
                names,
            ),
        }
    }

    pub(in crate::frontend) fn native_module_with_source_names(
        &self,
        names: &NativeSourceNames<'_>,
    ) -> Result<String, Box<Diagnostic>> {
        match &self.body {
            CheckedBody::Scalar(program) => {
                program.native_module_with_source_names(self.entry, self.sources, names)
            }
            CheckedBody::Owned(program) => program.native_module_with_execution(
                self.entry,
                self.sources,
                false,
                crate::frontend::build_config::ExecutionPolicy::Bounded,
                Some(names),
            ),
            CheckedBody::OwnedVector(program) => program[0].native_module_with_execution(
                self.entry,
                self.sources,
                false,
                crate::frontend::build_config::ExecutionPolicy::Bounded,
                Some(names),
            ),
        }
    }
    /// A single source-owned signature rule serves process and pure test consumers.
    fn checked_process_entry(&self) -> Result<(hir::DefId, Span), Box<Diagnostic>> {
        self.checked_i32_entry(
            "process entry requires original-root fn main() -> i32 with no parameters",
        )
    }

    fn checked_i32_entry(
        &self,
        message: &'static str,
    ) -> Result<(hir::DefId, Span), Box<Diagnostic>> {
        let signature = match self.entry {
            Some(id) => {
                let facts = match &self.body {
                    CheckedBody::Scalar(program) => program
                        .program
                        .functions
                        .get(id.0)
                        .filter(|function| function.id == id)
                        .map(|function| {
                            (
                                function.param_count,
                                function.result == hir::Ty::I32,
                                function.span,
                            )
                        }),
                    CheckedBody::Owned(program) => program.entry_signature(id),
                    CheckedBody::OwnedVector(program) => program[0].entry_signature(id),
                }
                .ok_or_else(association::bad)?;
                Some((id, facts))
            }
            None => None,
        };
        match signature {
            Some((id, (0, true, span))) => Ok((id, span)),
            other => Err(Diagnostic::new(
                "E0600",
                "oir-run",
                message,
                other.map(|(_, (_, _, span))| span),
            )),
        }
    }

    /// A pure declared test uses the same checked original entry identity, but
    /// returns its bounded typed value without installing process capabilities.
    pub(in crate::frontend) fn run_test_i32(&self) -> Result<i32, Box<Diagnostic>> {
        self.checked_i32_entry(
            "test entry requires fn main() -> i32 with no parameters in the declared test file",
        )?;
        match self.run_result(crate::frontend::build_config::ExecutionPolicy::Bounded)? {
            RuntimeValue::Scalar(RuntimeScalar::Narrow(Scalar::I32(status))) => Ok(status),
            _ => Err(association::bad()),
        }
    }

    fn require_process_host(&self) -> Result<(), Box<Diagnostic>> {
        if super::super::process::supported_host() {
            Ok(())
        } else {
            Err(Diagnostic::new(
                "E0608",
                "oir-run",
                "process execution requires Linux x86_64",
                None,
            ))
        }
    }

    /// Admit workspace process entries before any selected test can execute.
    /// This reuses the runtime signature rule without running source or tools.
    pub(in crate::frontend) fn process_preflight(&self) -> Result<(), Box<Diagnostic>> {
        self.require_process_host()?;
        self.checked_process_entry().map(|_| ())
    }

    pub(in crate::frontend) fn run_process(&self) -> Result<i32, ProcessFailure> {
        self.require_process_host()?;
        if !super::super::process::setup() {
            return Err(ProcessFailure::Setup);
        }
        let (entry, span) = self.checked_process_entry()?;
        let value = match &self.body {
            CheckedBody::Scalar(program) => program
                .run(Some(entry))
                .map_err(|error| error.diagnostic(self.sources))?,
            CheckedBody::Owned(program) => program.run_process(entry, self.sources)?,
            CheckedBody::OwnedVector(program) => program[0].run_process(entry, self.sources)?,
        };
        match value {
            Scalar::I32(status @ 0..=255) => Ok(status),
            _ => Err(Diagnostic::new(
                "E0600",
                "oir-run",
                "process main must return a status in 0..255",
                Some(span),
            )
            .into()),
        }
    }

    pub(in crate::frontend) fn run_process_with_execution(
        &self,
        execution: crate::frontend::build_config::ExecutionPolicy,
    ) -> Result<i32, ProcessFailure> {
        if !execution.hosted() {
            return self.run_process();
        }
        self.require_process_host()?;
        if !super::super::process::setup() {
            return Err(ProcessFailure::Setup);
        }
        let (entry, span) = self.checked_process_entry()?;
        let value = match &self.body {
            CheckedBody::Scalar(program) => program
                .run_with_execution(Some(entry), execution)
                .map_err(|error| error.diagnostic(self.sources))?,
            CheckedBody::Owned(program) => {
                program.run_process_with_execution(entry, self.sources, execution)?
            }
            CheckedBody::OwnedVector(program) => {
                program[0].run_process_with_execution(entry, self.sources, execution)?
            }
        };
        match value {
            Scalar::I32(status @ 0..=255) => Ok(status),
            _ => Err(Diagnostic::new(
                "E0600",
                "oir-run",
                "process main must return a status in 0..255",
                Some(span),
            )
            .into()),
        }
    }

    pub(in crate::frontend) fn native_process_module(&self) -> Result<String, Box<Diagnostic>> {
        self.require_process_host()?;
        let (entry, _) = self.checked_process_entry()?;
        match &self.body {
            CheckedBody::Scalar(program) => {
                program.native_process_module(Some(entry), self.sources)
            }
            CheckedBody::Owned(program) => program.native_process_module(entry, self.sources),
            CheckedBody::OwnedVector(program) => {
                program[0].native_process_module(entry, self.sources)
            }
        }
    }

    pub(in crate::frontend) fn native_process_module_with_source_names(
        &self,
        names: &NativeSourceNames<'_>,
    ) -> Result<String, Box<Diagnostic>> {
        self.require_process_host()?;
        let (entry, _) = self.checked_process_entry()?;
        match &self.body {
            CheckedBody::Scalar(program) => {
                program.native_process_module_with_source_names(Some(entry), self.sources, names)
            }
            CheckedBody::Owned(program) => {
                program.native_process_module_with_source_names(entry, self.sources, names)
            }
            CheckedBody::OwnedVector(program) => {
                program[0].native_process_module_with_source_names(entry, self.sources, names)
            }
        }
    }

    #[cfg(test)]
    pub(in crate::frontend) fn route(&self) -> ProjectRoute {
        match self.body {
            CheckedBody::Scalar(_) => ProjectRoute::Scalar,
            CheckedBody::Owned(_) => ProjectRoute::Owned,
            CheckedBody::OwnedVector(_) => ProjectRoute::Owned,
        }
    }
    #[cfg(test)]
    pub(in crate::frontend) fn entry(&self) -> Option<hir::DefId> {
        self.entry
    }
}

#[test]
fn bounded_enum_sealed_source_carrier_measurements() {
    use crate::frontend::oir::owned_types::{EnumDeclarations, RawEnumDecl};
    use std::mem::{align_of, size_of};

    // Baseline carriers contain the old inline record-only witness. The only
    // new stored data is its raw enum Vec header plus checked enum facade;
    // enclosing wrappers neither duplicate it nor introduce a heap allocation.
    // Closed builtin origins add one measured padded header word. None
    // still denies every builtin entry; this is physical carrier accounting.
    // The vector successor embeds one more Vec and grows the closed catalog
    // from five to nine bytes. Round each catalog independently to the inline
    // Vec alignment, without deriving the expected delta from SourceProgram.
    let header_alignment = align_of::<Vec<()>>();
    let padded_catalog = |bytes: usize| bytes.div_ceil(header_alignment) * header_alignment;
    let vector_header_growth = size_of::<Vec<()>>() + padded_catalog(9) - padded_catalog(5);
    let growth = size_of::<Vec<RawEnumDecl>>()
        + size_of::<EnumDeclarations>()
        + size_of::<usize>()
        + vector_header_growth;
    macro_rules! measured {
        ($($ty:ty => $baseline:expr),+ $(,)?) => { $(
            println!(
                "enum-sealed-layout {} bytes={} align={} admitted-growth={}",
                stringify!($ty), size_of::<$ty>(), align_of::<$ty>(), growth,
            );
            #[cfg(target_pointer_width = "64")]
            assert_eq!(size_of::<$ty>(), $baseline + growth);
        )+ };
    }
    measured!(
        owned::SourceProgram => 184,
        CheckedBody => 184,
        CheckedSourceProgram<'_> => 208,
        Checked<'_> => 208,
    );
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(size_of::<crate::frontend::builtin_catalog::BuiltinSet>(), 9);
        assert_eq!(padded_catalog(5), 8);
        assert_eq!(padded_catalog(9), 16);
        assert_eq!(vector_header_growth, 32);
        assert_eq!(size_of::<Vec<RawEnumDecl>>(), 24);
        assert_eq!(size_of::<EnumDeclarations>(), 80);
        assert_eq!(size_of::<VerifiedProgram>(), 24);
        assert_eq!(size_of::<CheckedProjectTypes>(), 40);
    }
}

// Only the new selector/caller transports. CheckedBody and checked source
// construction reuse the ordinary facade's existing complete witness roles.
#[allow(dead_code)]
struct ProductionEnumFacadeCarriers {
    original_selected: bool,
    original_return: Result<(owned::SourceProgram, Option<hir::DefId>), Vec<Diagnostic>>,
    original_program: owned::SourceProgram,
    original_entry: Option<hir::DefId>,
    project_count: usize,
    project_selected: bool,
    project_index: &'static index::DeclarationIndex<'static>,
    project_work: &'static WorkMeter,
    project_allocator: &'static mut Allocator,
    project_return: Result<owned::source::typeck::TypedOwnedProgram<'static>, Vec<Diagnostic>>,
    project_typed: owned::source::typeck::TypedOwnedProgram<'static>,
}
pub(in crate::frontend::oir) const fn enum_facade_carrier_bytes() -> usize {
    std::mem::size_of::<ProductionEnumFacadeCarriers>()
}

#[test]
fn bounded_enum_production_facade_caller_layout() {
    println!(
        "ENUM_PRODUCTION_FACADE_LAYOUT controls={} body={} source={} result={}",
        enum_facade_carrier_bytes(),
        std::mem::size_of::<CheckedBody>(),
        std::mem::size_of::<CheckedSourceProgram<'_>>(),
        std::mem::size_of::<Checked<'_>>()
    );
}

#[test]
fn public_process_signature_is_shared_and_keeps_checked_owner_inline() {
    use crate::frontend::{lexer, parser};
    use std::mem::{align_of, size_of};
    for (text, accepted) in [
        ("fn main()->i32{return 37;}", true),
        ("fn main()->bool{return true;}", false),
        ("fn main()->(){return;}", false),
        ("fn main(x:i32)->i32{return x;}", false),
        ("fn helper()->i32{return 37;}", false),
        (
            "struct C{n:i32} fn main()->i32{let c=C{n:37};return c.n;}",
            true,
        ),
        ("struct C{} fn main()->C{return C{};}", false),
    ] {
        let mut sources = SourceMap::new();
        let file = sources.add("process-signature.ox".into(), text.into());
        let source = sources.get(file);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let checked = check_source(source, &ast, &sources).unwrap();
        let entry = checked.checked_process_entry();
        assert_eq!(entry.is_ok(), accepted, "{text}");
        if let Err(error) = entry {
            assert_eq!((error.code, error.stage), ("E0600", "oir-run"));
            assert_eq!(
                error.message,
                "process entry requires original-root fn main() -> i32 with no parameters"
            );
            if super::super::process::supported_host() {
                let emitted = checked.native_process_module().unwrap_err();
                assert_eq!(emitted.render_json(&sources), error.render_json(&sources));
            }
        }
    }
    println!(
        "PROCESS_SOURCE_CARRIERS failure={}/{} result={}/{} signature={}/{} checked={}/{} owner_policy_fields=0",
        size_of::<ProcessFailure>(),
        align_of::<ProcessFailure>(),
        size_of::<Result<i32, ProcessFailure>>(),
        align_of::<Result<i32, ProcessFailure>>(),
        size_of::<Result<(hir::DefId, Span), Box<Diagnostic>>>(),
        align_of::<Result<(hir::DefId, Span), Box<Diagnostic>>>(),
        size_of::<CheckedSourceProgram<'static>>(),
        align_of::<CheckedSourceProgram<'static>>()
    );
}
