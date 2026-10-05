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
    let (body, entry) = if ast.uses_owned_syntax(source) {
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
        association::scalar(&raw, sources, Declarations::Original(ast)).map_err(|e| vec![*e])?;
        let verified = verify::verify(raw, sources).map_err(|e| vec![*e.diagnostic(sources)])?;
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
            association::scalar(&raw, project.sources(), Declarations::Project(&frozen))
                .map_err(|e| vec![*e])?;
            CheckedBody::Scalar(
                verify::verify(raw, project.sources())
                    .map_err(|e| vec![*e.diagnostic(project.sources())])?,
            )
        }
        ProjectRoute::Owned => {
            let resolved = owned::source::resolve::resolve_project(&frozen, work)?;
            let typed = owned::source::typeck::check(resolved)?;
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
    let growth = size_of::<Vec<RawEnumDecl>>() + size_of::<EnumDeclarations>();
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
