//! One immutable owner for an explicitly selected application and its packages.
use super::{
    declaration_index::{IndexLimits, WorkMeter},
    diagnostic::Diagnostic,
    manifest::{self, Command, Request},
    oir,
    options::EntryPolicy,
    project::{budget::Allocator, forest::PackageId, ProjectLimits, ProjectSources, SyntaxFlavor},
    project_build,
    source::SourceMap,
};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Engine {
    Interpreter,
    Native,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edition {
    TypedPreview,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Host,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Profile {
    Default,
}

#[derive(Clone, Copy)]
pub(super) struct Configuration {
    pub command: Command,
    pub engine: Engine,
    pub entry_policy: EntryPolicy,
    pub json: bool,
    edition: Edition,
    target: Target,
    profile: Profile,
}
impl Configuration {
    pub fn new(command: Command, engine: Engine, entry_policy: EntryPolicy, json: bool) -> Self {
        Self {
            command,
            engine,
            entry_policy,
            json,
            edition: Edition::TypedPreview,
            target: Target::Host,
            profile: Profile::Default,
        }
    }
}

pub(super) struct CompilerSession {
    selected: PackageId,
    configuration: Configuration,
    parent: String,
    project: ProjectSources,
}
impl CompilerSession {
    pub fn load(
        request: Box<[Request; 1]>,
        configuration: Configuration,
    ) -> Result<Self, (SourceMap, Vec<Diagnostic>)> {
        debug_assert!(matches!(
            (
                configuration.edition,
                configuration.target,
                configuration.profile
            ),
            (Edition::TypedPreview, Target::Host, Profile::Default)
        ));
        let needs_name = configuration.engine == Engine::Native
            && matches!(
                configuration.command,
                Command::Run | Command::Build | Command::Package
            );
        let selection = manifest::graph::resolve(request, configuration.command, needs_name)
            .map_err(|failure| {
                let (code, stage) = failure.classification();
                (
                    SourceMap::new(),
                    vec![*Diagnostic::new(code, stage, failure.message(), None)],
                )
            })?;
        // Preserve native Process setup ordering without installing signal state.
        if configuration.entry_policy == EntryPolicy::Process && !oir::process::supported_host() {
            return Err((
                SourceMap::new(),
                vec![*Diagnostic::new(
                    "E0608",
                    "oir-run",
                    "process execution requires Linux x86_64",
                    None,
                )],
            ));
        }
        #[cfg(test)]
        manifest::observe_source_load();
        let project = ProjectSources::load_forest(selection.graph, ProjectLimits::default())
            .map_err(|failure| (failure.sources, failure.diagnostics))?;
        Ok(Self {
            selected: PackageId(0),
            configuration,
            parent: selection.parent,
            project,
        })
    }
    pub fn project(&self) -> &ProjectSources {
        &self.project
    }
    pub fn parent(&self) -> &str {
        &self.parent
    }
    pub fn configuration(&self) -> Configuration {
        self.configuration
    }
    pub fn selected_name(&self) -> &str {
        &self
            .project
            .graph()
            .expect("manifest session graph")
            .packages[self.selected.0]
            .name
    }
    pub fn check(&self) -> Result<oir::CheckedSourceProgram<'_>, Vec<Diagnostic>> {
        check_project(&self.project)
    }
    pub fn tests(&self) -> Result<oir::CheckedProjectTests<'_>, Vec<Diagnostic>> {
        let limits = IndexLimits::default();
        oir::check_project_tests(
            &self.project,
            limits,
            &WorkMeter::new(limits.work),
            &mut Allocator::default(),
        )
    }
    pub fn build(&self) -> Result<project_build::Artifact, Vec<Diagnostic>> {
        let verified = self.check()?;
        let module = match self.configuration.entry_policy {
            EntryPolicy::Result => verified.native_module(),
            EntryPolicy::Process => verified.native_process_module(),
        }
        .map_err(|e| vec![*e])?;
        let package = &self
            .project
            .graph()
            .expect("manifest session graph")
            .packages[self.selected.0];
        let copy = |text: &str| -> Result<String, Vec<Diagnostic>> {
            let mut result = String::new();
            result.try_reserve_exact(text.len()).map_err(|_| {
                vec![*Diagnostic::new(
                    "E0400",
                    "compiler-session",
                    "publication selection allocation failed",
                    None,
                )]
            })?;
            result.push_str(text);
            Ok(result)
        };
        let selection = manifest::BuildSelection {
            parent: copy(&self.parent)?,
            name: copy(&package.name)?,
        };
        project_build::publish(&module, &selection, self.configuration.entry_policy)
            .map_err(|e| vec![*e])
    }
}

pub(super) fn check_project(
    project: &ProjectSources,
) -> Result<oir::CheckedSourceProgram<'_>, Vec<Diagnostic>> {
    match project.syntax_flavor() {
        SyntaxFlavor::OriginalSingleFile => {
            let (source, ast) = project
                .original_file()
                .expect("original syntax retains one file");
            oir::check_source(source, ast, project.sources())
        }
        SyntaxFlavor::ProjectSyntax => {
            let limits = IndexLimits::default();
            oir::project::check_project_executable(
                project,
                limits,
                &WorkMeter::new(limits.work),
                &mut Allocator::default(),
            )
        }
    }
}
