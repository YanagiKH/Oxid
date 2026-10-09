//! The only production owner of the source/body/entry association. No child modules.
use super::super::{
    owned,
    project::{CheckedProjectTypes, ProjectRoute},
    *,
};
use super::association::{self, Declarations};
use crate::frontend::{
    ast,
    declaration_index::{self as index, IndexLimits, SourceOwner, WorkMeter},
    project::{budget::Allocator, ProjectSources, SyntaxFlavor},
    source::{SourceFile, SourceView},
};

// Keep the witness inline: its enum growth is the already-admitted raw vector
// header and checked declaration facade. Boxing would add an unbudgeted allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum CheckedBody {
    Scalar(VerifiedProgram),
    Owned(owned::SourceProgram),
}

/// Borrows only the immutable source map, never an index, AST or type-pass state.
/// Complete source pipelines below are the only ways to install this tuple.
#[derive(Debug)]
pub(in crate::frontend) struct CheckedSourceProgram<'s> {
    sources: &'s SourceMap,
    body: CheckedBody,
    entry: Option<hir::DefId>,
}

pub(in crate::frontend) fn check_source<'s>(
    source: &SourceFile,
    ast: &ast::Program,
    sources: &'s SourceMap,
) -> Result<CheckedSourceProgram<'s>, Vec<Diagnostic>> {
    let owner =
        SourceOwner::original(source, ast, SourceView::Map(sources)).map_err(|e| vec![*e])?;
    let (body, entry) = if !ast.enums.is_empty() {
        let (program, entry) = owned::source::check_enum_source(owner)?;
        (CheckedBody::Owned(program), entry)
    } else if ast.uses_owned_syntax(source) {
        let resolved = owned::source::resolve::resolve_sources(owner)?;
        let typed = owned::source::typeck::check(resolved)?;
        let entry = typed.entry();
        (
            CheckedBody::Owned(owned::source::check_typed(&typed)?),
            entry,
        )
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
    let route = if sources.owned(work).map_err(|e| vec![*e])? {
        ProjectRoute::Owned
    } else {
        ProjectRoute::Scalar
    };
    #[cfg(test)]
    work.observe(crate::frontend::declaration_index::Observation::Route {
        owned: route == ProjectRoute::Owned,
    });
    let facts = index::collect_originals(sources, limits, work, allocator).map_err(|e| {
        work.record_error(&e);
        vec![*e]
    })?;
    let original_signatures =
        if route == ProjectRoute::Scalar && sources.flavor() == SyntaxFlavor::OriginalSingleFile {
            Some(hir::original_signatures(&facts, work)?)
        } else {
            None
        };
    let frozen = facts.finish(work, allocator)?;
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
            let typed = if frozen.enum_count() != 0 {
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
    pub(in crate::frontend) fn function_count(&self) -> usize {
        match &self.body {
            CheckedBody::Scalar(program) => program.function_count(),
            CheckedBody::Owned(program) => program.function_count(),
        }
    }
    pub(in crate::frontend) fn run(&self) -> Result<Scalar, Box<Diagnostic>> {
        match &self.body {
            CheckedBody::Scalar(program) => program
                .run(self.entry)
                .map_err(|e| e.diagnostic(self.sources)),
            CheckedBody::Owned(program) => program.run(self.entry, self.sources),
        }
    }
    pub(in crate::frontend) fn native_module(&self) -> Result<String, Box<Diagnostic>> {
        match &self.body {
            CheckedBody::Scalar(program) => program.native_module(self.entry, self.sources),
            CheckedBody::Owned(program) => program.native_module(self.entry, self.sources),
        }
    }
    /// A single source-owned signature rule serves both public process consumers.
    fn checked_process_entry(&self) -> Result<(hir::DefId, Span), Box<Diagnostic>> {
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
                "process entry requires original-root fn main() -> i32 with no parameters",
                other.map(|(_, (_, _, span))| span),
            )),
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
        }
    }

    #[cfg(test)]
    pub(in crate::frontend) fn route(&self) -> ProjectRoute {
        match self.body {
            CheckedBody::Scalar(_) => ProjectRoute::Scalar,
            CheckedBody::Owned(_) => ProjectRoute::Owned,
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
    let growth = size_of::<Vec<RawEnumDecl>>() + size_of::<EnumDeclarations>() + size_of::<usize>();
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
