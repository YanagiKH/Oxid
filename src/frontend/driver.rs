use super::project::packages::{self, LoadedSourceInput, PackageSources, PackageSpec};
use super::{
    build_config::{ExecutionPolicy, NativeBuildSettings, NativeSourceNames},
    declaration_index::{IndexLimits, WorkMeter},
    diagnostic::{json_string, Diagnostic},
    oir,
    options::{self, EntryPolicy, Operation, Route},
    project::{budget::Allocator, ProjectLimits, ProjectSources, SyntaxFlavor},
    source::SourceMap,
};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(crate) enum PackageCommand<'a> {
    Check,
    Run,
    Compile {
        output: &'a str,
        managed: bool,
        incremental: bool,
    },
}

/// A source checksum has no package identity or manifest semantics. The caller
/// binds its graph index to the independently admitted manifest and source ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PackageInputFingerprint {
    pub package: usize,
    pub checksum: String,
    pub files: usize,
    pub bytes: usize,
}

#[derive(Debug)]
pub(crate) struct PreparedPackageProject<'a> {
    sources: PreparedSources<'a>,
    dependencies: Vec<PackageInputFingerprint>,
}

// Keep the bounded package sidecar inline. No existing source/index carrier is
// enlarged, and ordinary file and package routes do not construct this enum.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum PreparedSources<'a> {
    Single(ProjectSources),
    Packages(PackageSources<'a>),
}

#[derive(Debug)]
pub(crate) struct PackagePreparationError {
    sources: SourceMap,
    diagnostics: Vec<Diagnostic>,
}

impl From<super::project::LoadFailure> for PackagePreparationError {
    fn from(failure: super::project::LoadFailure) -> Self {
        Self {
            sources: failure.sources,
            diagnostics: failure.diagnostics,
        }
    }
}

impl PackagePreparationError {
    /// Lock creation needs bounded plain text, while executable commands retain
    /// the complete source-aware frontend diagnostic protocol through report.
    pub(crate) fn message(&self) -> String {
        preparation_message(&self.sources, &self.diagnostics)
    }

    pub(crate) fn report(self, command: PackageCommand<'_>, json: bool, process: bool) -> i32 {
        let operation = package_operation(&command);
        if process && operation == Operation::Run {
            if json {
                return process_option_error(
                    "process-mode run does not support --message-format=json",
                );
            }
            if !oir::process::setup() {
                return 74;
            }
            report_process(&self.sources, &self.diagnostics)
        } else {
            report(
                &self.sources,
                self.diagnostics,
                json,
                Summary::empty(operation),
            )
        }
    }
}

fn preparation_message(sources: &SourceMap, diagnostics: &[Diagnostic]) -> String {
    struct Bounded(String);
    impl std::fmt::Write for Bounded {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            let available = 2048 - self.0.len();
            let mut end = available.min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            self.0.push_str(&text[..end]);
            if end == text.len() {
                Ok(())
            } else {
                Err(std::fmt::Error)
            }
        }
    }
    let mut out = Bounded(String::with_capacity(2048));
    if let Some(error) = diagnostics.first() {
        let _ = error.write_human(sources, &mut out);
    } else {
        out.0.push_str("package source preparation failed");
    }
    out.0
}

fn package_operation(command: &PackageCommand<'_>) -> Operation {
    match command {
        PackageCommand::Check => Operation::Check,
        PackageCommand::Run => Operation::Run,
        PackageCommand::Compile { .. } => Operation::Compile,
    }
}

/// Load once, without type checking, execution, LLVM or output publication.
/// Test overrides select retained application source without changing the graph
/// metadata supplied by the caller. Only dependencies receive content pins.
pub(crate) fn prepare_package_project<'a>(
    packages: &'a [PackageSpec],
    entry_override: Option<&Path>,
) -> Result<PreparedPackageProject<'a>, PackagePreparationError> {
    prepare_package_with_limits(packages, entry_override, ProjectLimits::default())
}

/// Source packaging shares its remaining retention budget with every session.
/// Existing project entry points retain their original default limits.
pub(crate) fn prepare_package_project_bounded<'a>(
    packages: &'a [PackageSpec],
    entry_override: Option<&Path>,
    files: usize,
    bytes: usize,
) -> Result<PreparedPackageProject<'a>, PackagePreparationError> {
    if files == 0 {
        return Err(PackagePreparationError {
            sources: SourceMap::new(),
            diagnostics: vec![*Diagnostic::new(
                "E0400",
                "package",
                "source package retained file budget exceeded",
                None,
            )],
        });
    }
    prepare_package_with_limits(
        packages,
        entry_override,
        ProjectLimits {
            modules: files.min(256),
            source_bytes: bytes.min(ProjectLimits::default().source_bytes),
            ..ProjectLimits::default()
        },
    )
}

fn prepare_package_with_limits<'a>(
    packages: &'a [PackageSpec],
    entry_override: Option<&Path>,
    limits: ProjectLimits,
) -> Result<PreparedPackageProject<'a>, PackagePreparationError> {
    prepare_package_with_editor(packages, entry_override, limits, None)
}

fn prepare_package_with_editor<'a>(
    packages: &'a [PackageSpec],
    entry_override: Option<&Path>,
    limits: ProjectLimits,
    overlay: Option<&super::project::editor_sources::SourceOverlay<'_>>,
) -> Result<PreparedPackageProject<'a>, PackagePreparationError> {
    packages::validate_graph(packages, entry_override).map_err(|error| {
        PackagePreparationError {
            sources: SourceMap::new(),
            diagnostics: vec![*error],
        }
    })?;
    if packages.len() == 1 {
        let entry = entry_override
            .unwrap_or(&packages[0].entry)
            .to_str()
            .expect("validated UTF-8 entry");
        let project = if let Some(overlay) = overlay {
            ProjectSources::load_typed_editor(entry, limits, overlay)?
        } else {
            ProjectSources::load_typed(entry, limits)?
        };
        return Ok(PreparedPackageProject {
            sources: PreparedSources::Single(project),
            dependencies: Vec::new(),
        });
    }
    let sources = if overlay.is_some() {
        PackageSources::load_with_editor(packages, entry_override, limits, overlay)?
    } else {
        PackageSources::load(packages, entry_override, limits)?
    };
    let dependencies = match dependency_fingerprints(&sources, packages.len()) {
        Ok(dependencies) => dependencies,
        Err(error) => {
            return Err(PackagePreparationError {
                sources: sources.into_source_map(),
                diagnostics: vec![*error],
            })
        }
    };
    Ok(PreparedPackageProject {
        sources: PreparedSources::Packages(sources),
        dependencies,
    })
}

/// Editor checking has the same grammar, package namespaces, type checking,
/// ownership and verified OIR as project checking. It performs no execution,
/// native compilation, artifact publication or lock update.
pub(super) fn editor_check(
    packages: &[PackageSpec],
    entry_override: Option<&Path>,
    overlay: &super::project::editor_sources::SourceOverlay<'_>,
) -> super::editor::EditorCheck {
    match prepare_package_with_editor(
        packages,
        entry_override,
        ProjectLimits::default(),
        Some(overlay),
    ) {
        Err(error) => super::editor::checked_sources(&error.sources, &error.diagnostics, false),
        Ok(prepared) => match prepared.checked() {
            Ok(_) => super::editor::checked_sources(prepared.project().sources(), &[], true),
            Err(errors) => {
                super::editor::checked_sources(prepared.project().sources(), &errors, true)
            }
        },
    }
}

pub(super) fn editor_complete(
    packages: &[PackageSpec],
    entry: Option<&Path>,
    overlay: &super::project::editor_sources::SourceOverlay<'_>,
    path: &Path,
    offset: usize,
) -> Result<super::editor::EditorCompletions, Vec<super::editor::EditorDiagnostic>> {
    if overlay
        .text(path)
        .is_some_and(|text| super::editor::completion::qualified_hint(text, offset))
    {
        let session = super::project::editor_sources::QuerySources::load(packages, entry, overlay)
            .map_err(|error| super::editor::diagnostics(&error.sources, &error.diagnostics))?;
        return super::editor::completion::complete_qualified(&session, path, offset)
            .map_err(|errors| super::editor::diagnostics(session.project().sources(), &errors));
    }
    let prepared =
        match prepare_package_with_editor(packages, entry, ProjectLimits::default(), Some(overlay))
        {
            Ok(prepared) => prepared,
            Err(error)
                if !error.diagnostics.is_empty()
                    && error
                        .diagnostics
                        .iter()
                        .all(|diagnostic| matches!(diagnostic.code, "E0100" | "E0101")) =>
            {
                return super::editor::completion::fallback(&error.sources, path, offset)
                    .map_err(|errors| super::editor::diagnostics(&error.sources, &errors))
            }
            Err(error) => {
                return Err(super::editor::diagnostics(
                    &error.sources,
                    &error.diagnostics,
                ))
            }
        };
    let owner = match &prepared.sources {
        PreparedSources::Single(project) => super::declaration_index::SourceOwner::project(project),
        PreparedSources::Packages(package) => {
            super::declaration_index::SourceOwner::packaged(package)
        }
    };
    let sources = prepared.project().sources();
    super::editor::completion::complete(owner, sources, path, offset)
        .map_err(|errors| super::editor::diagnostics(sources, &errors))
}

pub(super) fn editor_definition(
    packages: &[PackageSpec],
    entry: Option<&Path>,
    overlay: &super::project::editor_sources::SourceOverlay<'_>,
    path: &Path,
    offset: usize,
) -> Result<super::editor::EditorDefinition, Vec<super::editor::EditorDiagnostic>> {
    let prepared =
        prepare_package_with_editor(packages, entry, ProjectLimits::default(), Some(overlay))
            .map_err(|error| super::editor::diagnostics(&error.sources, &error.diagnostics))?;
    let owner = match &prepared.sources {
        PreparedSources::Single(project) => super::declaration_index::SourceOwner::project(project),
        PreparedSources::Packages(package) => {
            super::declaration_index::SourceOwner::packaged(package)
        }
    };
    let sources = prepared.project().sources();
    let location = super::editor::navigation::definition(owner, sources, path, offset)
        .map_err(|errors| super::editor::diagnostics(sources, &errors))?;
    Ok(super::editor::EditorDefinition {
        location,
        files: sources.files().len(),
        bytes: sources
            .files()
            .iter()
            .map(|source| source.text().len())
            .sum(),
    })
}

pub(super) fn editor_symbols(
    packages: &[PackageSpec],
    entry: Option<&Path>,
    overlay: &super::project::editor_sources::SourceOverlay<'_>,
    path: Option<&Path>,
    query: &str,
) -> Result<super::editor::EditorSymbols, Vec<super::editor::EditorDiagnostic>> {
    let prepared =
        prepare_package_with_editor(packages, entry, ProjectLimits::default(), Some(overlay))
            .map_err(|error| super::editor::diagnostics(&error.sources, &error.diagnostics))?;
    let owner = match &prepared.sources {
        PreparedSources::Single(project) => super::declaration_index::SourceOwner::project(project),
        PreparedSources::Packages(package) => {
            super::declaration_index::SourceOwner::packaged(package)
        }
    };
    super::editor::navigation::symbols(owner, prepared.project().sources(), path, query)
        .map_err(|errors| super::editor::diagnostics(prepared.project().sources(), &errors))
}

fn fingerprint_error() -> Box<Diagnostic> {
    Diagnostic::new(
        "E0400",
        "package",
        "dependency input fingerprint budget exceeded",
        None,
    )
}

/// lp(b) = u64 little-endian length followed by b. The domain and actual entry
/// path precede a u64 file count; each path-sorted source is F + lp(path) + lp(bytes).
/// SHA-256 stays in the driver so standalone parser/static observers need no
/// extra crate. The loader exposes borrowed inputs, not a second filesystem walk.
fn dependency_fingerprints(
    sources: &PackageSources<'_>,
    count: usize,
) -> Result<Vec<PackageInputFingerprint>, Box<Diagnostic>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count.saturating_sub(1))
        .map_err(|_| fingerprint_error())?;
    for package in 1..count {
        result.push(input_fingerprint(package, sources.source_inputs(package)?)?);
    }
    Ok(result)
}

fn input_fingerprint<'a>(
    package: usize,
    inputs: impl ExactSizeIterator<Item = Result<LoadedSourceInput<'a>, Box<Diagnostic>>>,
) -> Result<PackageInputFingerprint, Box<Diagnostic>> {
    let files = inputs.len();
    if files == 0 || files > 256 {
        return Err(fingerprint_error());
    }
    let mut sorted = [LoadedSourceInput::default(); 256];
    let mut bytes = 0usize;
    for (index, input) in inputs.enumerate() {
        let input = input?;
        bytes = bytes
            .checked_add(input.bytes.len())
            .ok_or_else(fingerprint_error)?;
        sorted[index] = input;
    }
    let entry = sorted[0].path;
    sorted[..files].sort_unstable_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    if sorted[..files]
        .windows(2)
        .any(|pair| pair[0].path == pair[1].path)
    {
        return Err(fingerprint_error());
    }
    fn lp(hash: &mut Sha256, bytes: &[u8]) {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    let mut hash = Sha256::new();
    lp(&mut hash, b"oxid-typed-local-source-v1");
    lp(&mut hash, entry.as_bytes());
    hash.update((files as u64).to_le_bytes());
    for input in &sorted[..files] {
        hash.update(b"F");
        lp(&mut hash, input.path.as_bytes());
        lp(&mut hash, input.bytes);
    }
    let digest = hash.finalize();
    let mut checksum = String::new();
    checksum
        .try_reserve_exact(71)
        .map_err(|_| fingerprint_error())?;
    checksum.push_str("sha256:");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        checksum.push(HEX[usize::from(byte >> 4)] as char);
        checksum.push(HEX[usize::from(byte & 15)] as char);
    }
    Ok(PackageInputFingerprint {
        package,
        checksum,
        files,
        bytes,
    })
}

impl PreparedPackageProject<'_> {
    pub(crate) fn dependency_fingerprints(&self) -> &[PackageInputFingerprint] {
        &self.dependencies
    }

    /// Workspace roots can also be imported library members. Bind their exact
    /// retained bytes with the same source protocol before combining sessions.
    pub(crate) fn workspace_fingerprints(
        &self,
        root: &PackageSpec,
    ) -> Result<Vec<(usize, String)>, String> {
        let fingerprint = match &self.sources {
            PreparedSources::Packages(package) => input_fingerprint(
                0,
                package
                    .source_inputs(0)
                    .map_err(|error| error.message.clone())?,
            ),
            PreparedSources::Single(project) => {
                let inputs = project.modules().iter().map(|header| {
                    let source = project
                        .sources()
                        .files()
                        .get(header.file.0)
                        .ok_or_else(fingerprint_error)?;
                    let path = header
                        .canonical_path
                        .as_deref()
                        .unwrap_or_else(|| Path::new(source.path()))
                        .strip_prefix(&root.root)
                        .ok()
                        .and_then(Path::to_str)
                        .ok_or_else(fingerprint_error)?;
                    Ok(LoadedSourceInput {
                        path,
                        bytes: source.text().as_bytes(),
                    })
                });
                input_fingerprint(0, inputs)
            }
        }
        .map_err(|error| error.message.clone())?;
        let mut result = Vec::with_capacity(self.dependencies.len() + 1);
        result.push((0, fingerprint.checksum));
        result.extend(
            self.dependencies
                .iter()
                .map(|input| (input.package, input.checksum.clone())),
        );
        Ok(result)
    }

    pub(crate) fn input_usage(&self) -> (usize, usize) {
        let sources = self.project().sources().files();
        (
            sources.len(),
            sources.iter().map(|source| source.text().len()).sum(),
        )
    }

    /// Presentation is derived from the canonical paths retained by this exact
    /// source owner. A singleton keeps its admitted absolute entry spelling and
    /// does not need the module loader's deferred canonical-path allocation.
    pub(crate) fn native_source_names(
        &self,
        logical_root: &Path,
    ) -> Result<NativeSourceNames<'_>, Box<Diagnostic>> {
        let project = self.project();
        let sources = project.sources();
        if project.modules().len() != sources.files().len()
            || project
                .modules()
                .iter()
                .any(|header| header.file.0 >= sources.files().len())
        {
            return Err(Diagnostic::new(
                "E0700",
                "native-admission",
                "native source presentation has an invalid retained source identity",
                None,
            ));
        }
        NativeSourceNames::project_relative(
            sources,
            logical_root,
            project.modules().iter().map(|header| {
                (
                    header.file,
                    header
                        .canonical_path
                        .as_deref()
                        .unwrap_or_else(|| Path::new(sources.get(header.file).path())),
                )
            }),
        )
    }

    fn native_module_with_settings(
        &self,
        checked: &oir::CheckedSourceProgram<'_>,
        process: bool,
        settings: &NativeBuildSettings,
        logical_root: &Path,
    ) -> Result<String, Box<Diagnostic>> {
        super::windows_native::preflight(self.project(), process, Some(settings))?;
        if process && (settings.reproducible() || settings.debug_info()) {
            checked.process_preflight()?;
        }
        let names = if settings.reproducible() {
            Some(self.native_source_names(logical_root)?)
        } else {
            None
        };
        if settings.debug_info() {
            let marked = if settings.optimized() {
                checked.native_optimized_module_with_execution(
                    process,
                    settings.execution(),
                    names.as_ref(),
                    true,
                )?
            } else {
                checked.native_debug_module_with_execution(
                    process,
                    settings.execution(),
                    names.as_ref(),
                )?
            };
            return super::native_debug::add(
                &marked,
                self.project(),
                names.as_ref(),
                settings.optimized(),
                checked.debug_records(),
            );
        }
        if settings.optimized() {
            checked.native_optimized_module_with_execution(
                process,
                settings.execution(),
                names.as_ref(),
                false,
            )
        } else {
            checked.native_module_with_execution(process, settings.execution(), names.as_ref())
        }
    }

    /// Native workspace preflight uses the same retained inputs, presentation,
    /// and IR admission as publication, before any member invokes native tools.
    pub(crate) fn workspace_preflight_with_native_settings(
        &self,
        json: bool,
        process: bool,
        settings: &NativeBuildSettings,
        logical_root: &Path,
    ) -> i32 {
        if !settings.reproducible() && !settings.execution().hosted() && !settings.debug_info() {
            return self.workspace_preflight(json, process, true, false);
        }
        let admitted = self.checked().and_then(|checked| {
            self.native_module_with_settings(&checked, process, settings, logical_root)
                .map(|_| ())
                .map_err(|error| vec![*error])
        });
        match admitted {
            Ok(()) => 0,
            Err(errors) => report(
                self.project().sources(),
                errors,
                json,
                Summary::empty(Operation::Check),
            ),
        }
    }

    /// Semantic preflight emits nothing on success and cannot run source or
    /// invoke native tools, so a later member refusal leaves all outputs intact.
    pub(crate) fn workspace_preflight(
        &self,
        json: bool,
        process: bool,
        native: bool,
        process_run: bool,
    ) -> i32 {
        let admitted = self.checked().and_then(|checked| {
            if native {
                super::windows_native::preflight(self.project(), process, None)
                    .map_err(|error| vec![*error])?;
                if process {
                    checked.native_process_module()
                } else {
                    checked.native_module()
                }
                .map(|_| ())
                .map_err(|error| vec![*error])
            } else if process {
                checked.process_preflight().map_err(|error| vec![*error])
            } else {
                Ok(())
            }
        });
        match admitted {
            Ok(_) => 0,
            Err(errors) if process_run => {
                if !oir::process::setup() {
                    return 74;
                }
                report_process(self.project().sources(), &errors)
            }
            Err(errors) => report(
                self.project().sources(),
                errors,
                json,
                Summary::empty(Operation::Check),
            ),
        }
    }

    /// Visit only retained source buffers. Canonical package identity is supplied
    /// separately; no path is reopened to assemble a source package.
    pub(crate) fn visit_package_sources(
        &self,
        root: &PackageSpec,
        mut visit: impl FnMut(usize, &str, &[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        match &self.sources {
            PreparedSources::Packages(sources) => {
                for package in 0..=self.dependencies.len() {
                    for input in sources
                        .source_inputs(package)
                        .map_err(|error| error.message.clone())?
                    {
                        let input = input.map_err(|error| error.message.clone())?;
                        visit(package, input.path, input.bytes)?;
                    }
                }
            }
            PreparedSources::Single(project) => {
                for header in project.modules() {
                    let source = project
                        .sources()
                        .files()
                        .get(header.file.0)
                        .ok_or("invalid retained source identity")?;
                    let path = header
                        .canonical_path
                        .as_deref()
                        .unwrap_or_else(|| Path::new(source.path()))
                        .strip_prefix(&root.root)
                        .ok()
                        .and_then(Path::to_str)
                        .ok_or("retained source must remain inside its package root")?;
                    visit(0, path, source.text().as_bytes())?;
                }
            }
        }
        Ok(())
    }

    /// Pure checking plus the existing executable entry contract. Libraries
    /// have no entry; neither branch executes source or invokes native tools.
    pub(crate) fn source_package_preflight(&self, entry: Option<bool>) -> Result<(), String> {
        self.checked()
            .and_then(|checked| {
                match entry {
                    None => Ok(()),
                    Some(true) => checked.process_preflight(),
                    Some(false) => checked.native_module().map(|_| ()),
                }
                .map_err(|error| vec![*error])
            })
            .map_err(|errors| preparation_message(self.project().sources(), &errors))
    }

    fn project(&self) -> &ProjectSources {
        match &self.sources {
            PreparedSources::Single(project) => project,
            PreparedSources::Packages(package) => &package.project,
        }
    }

    /// API output consumes only the checked session's retained source bytes.
    /// Dependencies are checked but documented only when selected themselves.
    pub(crate) fn documentation(
        &self,
        name: &str,
        version: Option<&str>,
        maximum: usize,
    ) -> Result<String, String> {
        let _checked = self
            .checked()
            .map_err(|errors| preparation_message(self.project().sources(), &errors))?;
        let modules = match &self.sources {
            PreparedSources::Single(project) => 0..project.modules().len(),
            PreparedSources::Packages(packages) => {
                let start = packages
                    .context
                    .target(0)
                    .ok_or("missing documentation package root")?
                    .0;
                let end = packages
                    .context
                    .target(1)
                    .map_or(packages.project.modules().len(), |module| module.0);
                start..end
            }
        };
        super::documentation::fragment(self.project(), modules, name, version, maximum)
    }

    fn checked(&self) -> Result<oir::CheckedSourceProgram<'_>, Vec<Diagnostic>> {
        match &self.sources {
            PreparedSources::Single(project) => check_loaded_project(project),
            PreparedSources::Packages(package) => {
                let limits = IndexLimits::default();
                oir::project::check_package_executable(
                    package,
                    limits,
                    &WorkMeter::new(limits.work),
                    &mut Allocator::default(),
                )
            }
        }
    }

    /// Execute only an explicitly selected declared pure test. The caller owns
    /// declared-entry selection; the sealed program owns its typed entry facts.
    pub(crate) fn execute_test(&self, execution: ExecutionPolicy) -> i32 {
        let project = self.project();
        let result = (|| {
            let checked = self.checked()?;
            super::windows_native::test_preflight(project, execution)
                .map_err(|error| vec![*error])?;
            checked.run_test_i32().map_err(|error| vec![*error])
        })();
        match result {
            Ok(status) => status,
            Err(errors) => report(project.sources(), errors, false, Summary::Run(None)),
        }
    }

    /// Consume the retained input logically, never by reopening a source path.
    /// All compilation and publication remain behind the ordinary OIR witness.
    pub(crate) fn execute(&self, command: PackageCommand<'_>, json: bool, process: bool) -> i32 {
        let operation = package_operation(&command);
        let process_run = process && operation == Operation::Run;
        if process_run && json {
            return process_option_error("process-mode run does not support --message-format=json");
        }
        if process_run && !oir::process::setup() {
            return 74;
        }
        let project = self.project();
        let executable = self.checked();
        if process_run {
            return match executable {
                Ok(checked) => match checked.run_process() {
                    Ok(status) => status,
                    Err(oir::ProcessFailure::Setup) => 74,
                    Err(oir::ProcessFailure::Diagnostic(error)) => {
                        report_process(project.sources(), &[*error])
                    }
                },
                Err(errors) => report_process(project.sources(), &errors),
            };
        }
        if let PackageCommand::Compile {
            output,
            managed,
            incremental,
        } = command
        {
            let result = (|| {
                let checked = executable?;
                super::windows_native::preflight(project, process, None)
                    .map_err(|error| vec![*error])?;
                let module = if process {
                    checked.native_process_module()
                } else {
                    checked.native_module()
                }
                .map_err(|error| vec![*error])?;
                if managed && incremental {
                    super::native::compile_managed_incremental(&module, output)
                        .map(|outcome| report_cache(outcome, json))
                } else if managed {
                    super::native::compile_managed(&module, output)
                } else {
                    super::native::compile(&module, output)
                }
                .map_err(|error| vec![*error])?;
                Ok(Summary::Compile(Some(output.to_owned())))
            })();
            return match result {
                Ok(summary) => report(project.sources(), Vec::new(), json, summary),
                Err(errors) => report(project.sources(), errors, json, Summary::Compile(None)),
            };
        }
        process_loaded(project, json, operation, None, executable)
    }

    pub(crate) fn execute_with_execution(
        &self,
        command: PackageCommand<'_>,
        json: bool,
        process: bool,
        execution: ExecutionPolicy,
    ) -> i32 {
        if !execution.hosted() || !matches!(command, PackageCommand::Run) {
            return self.execute(command, json, process);
        }
        if process && json {
            return process_option_error("process-mode run does not support --message-format=json");
        }
        if process && !oir::process::setup() {
            return 74;
        }
        let project = self.project();
        let executable = self.checked();
        if process {
            return match executable {
                Ok(checked) => match checked.run_process_with_execution(execution) {
                    Ok(status) => status,
                    Err(oir::ProcessFailure::Setup) => 74,
                    Err(oir::ProcessFailure::Diagnostic(error)) => {
                        report_process(project.sources(), &[*error])
                    }
                },
                Err(errors) => report_process(project.sources(), &errors),
            };
        }
        let result = executable.and_then(|checked| {
            checked
                .run_result(execution)
                .map(|value| Summary::Run(Some(value)))
                .map_err(|error| vec![*error])
        });
        match result {
            Ok(summary) => report(project.sources(), Vec::new(), json, summary),
            Err(errors) => report(project.sources(), errors, json, Summary::Run(None)),
        }
    }

    /// Typed project builds consume one resolved plan. Non-native operations
    /// keep their original execution and reporting paths; explicit-file callers
    /// continue using execute and the historical native wrapper entry points.
    pub(crate) fn execute_with_native_settings(
        &self,
        command: PackageCommand<'_>,
        json: bool,
        process: bool,
        settings: &NativeBuildSettings,
        logical_root: &Path,
    ) -> i32 {
        let PackageCommand::Compile {
            output,
            managed,
            incremental,
        } = command
        else {
            return self.execute_with_execution(command, json, process, settings.execution());
        };
        let result = (|| {
            let checked = self.checked()?;
            let module = self
                .native_module_with_settings(&checked, process, settings, logical_root)
                .map_err(|error| vec![*error])?;
            if managed && incremental {
                super::native::compile_managed_incremental_with_settings(&module, output, settings)
                    .map(|outcome| report_cache(outcome, json))
            } else if managed {
                super::native::compile_managed_with_settings(&module, output, settings)
            } else {
                super::native::compile_with_settings(&module, output, settings)
            }
            .map_err(|error| vec![*error])?;
            Ok(Summary::Compile(Some(output.to_owned())))
        })();
        match result {
            Ok(summary) => report(self.project().sources(), Vec::new(), json, summary),
            Err(errors) => report(
                self.project().sources(),
                errors,
                json,
                Summary::Compile(None),
            ),
        }
    }
}

/// Project commands carry the actual admitted dependency graph into the checked
/// source pipeline. The explicit single-file router remains independent.
pub(crate) fn process_package_project(
    packages: &[PackageSpec],
    entry_override: Option<&Path>,
    command: PackageCommand<'_>,
    json: bool,
    process: bool,
) -> i32 {
    if matches!(
        command,
        PackageCommand::Compile {
            incremental: true,
            ..
        }
    ) {
        return match prepare_package_project(packages, entry_override) {
            Ok(prepared) => prepared.execute(command, json, process),
            Err(error) => error.report(command, json, process),
        };
    }
    let operation = match command {
        PackageCommand::Check => Operation::Check,
        PackageCommand::Run => Operation::Run,
        PackageCommand::Compile { .. } => Operation::Compile,
    };
    let process_run = process && operation == Operation::Run;
    if process_run && json {
        return process_option_error("process-mode run does not support --message-format=json");
    }
    if let Err(error) = packages::validate_graph(packages, entry_override) {
        return if process_run {
            report_process(&SourceMap::new(), &[*error])
        } else {
            report(
                &SourceMap::new(),
                vec![*error],
                json,
                Summary::empty(operation),
            )
        };
    }
    if packages.len() == 1 && packages[0].dependencies.is_empty() {
        let entry = entry_override
            .unwrap_or(&packages[0].entry)
            .to_str()
            .expect("validated path");
        if let PackageCommand::Compile {
            output,
            managed: true,
            ..
        } = command
        {
            return build_project(entry, output, json, process);
        }
        let output = match command {
            PackageCommand::Compile { output, .. } => Some(output),
            _ => None,
        };
        return process_file(
            entry,
            json,
            operation,
            output,
            if process && operation != Operation::Check {
                EntryPolicy::Process
            } else {
                EntryPolicy::Result
            },
        );
    }
    let package = match PackageSources::load(packages, entry_override, ProjectLimits::default()) {
        Ok(package) => package,
        Err(failure) => {
            return if process_run {
                report_process(&failure.sources, &failure.diagnostics)
            } else {
                report(
                    &failure.sources,
                    failure.diagnostics,
                    json,
                    Summary::empty(operation),
                )
            }
        }
    };
    let limits = IndexLimits::default();
    let work = WorkMeter::new(limits.work);
    let executable =
        oir::project::check_package_executable(&package, limits, &work, &mut Allocator::default());
    let project = &package.project;
    if process_run {
        return match executable {
            Ok(verified) => match verified.run_process() {
                Ok(status) => status,
                Err(oir::ProcessFailure::Setup) => 74,
                Err(oir::ProcessFailure::Diagnostic(error)) => {
                    report_process(project.sources(), &[*error])
                }
            },
            Err(errors) => report_process(project.sources(), &errors),
        };
    }
    if let PackageCommand::Compile {
        output, managed, ..
    } = command
    {
        let result = (|| {
            let checked = executable?;
            let module = if process {
                checked.native_process_module()
            } else {
                checked.native_module()
            }
            .map_err(|e| vec![*e])?;
            if managed {
                super::native::compile_managed(&module, output)
            } else {
                super::native::compile(&module, output)
            }
            .map_err(|e| vec![*e])?;
            Ok(Summary::Compile(Some(output.to_owned())))
        })();
        return match result {
            Ok(summary) => report(project.sources(), Vec::new(), json, summary),
            Err(errors) => report(project.sources(), errors, json, Summary::Compile(None)),
        };
    }
    process_loaded(project, json, operation, None, executable)
}

fn report_cache(outcome: super::native_cache::Outcome, json: bool) {
    if json {
        println!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"build-cache\",\"stage\":\"native-object\",\"status\":\"{}\"}}", outcome.name());
    } else {
        println!("typed-preview native object cache: {}", outcome.name());
    }
}

/// None returns an untouched/default or explicitly legacy command to the old CLI.
pub fn dispatch(args: &mut Vec<String>) -> Option<i32> {
    dispatch_route(options::route(args), args)
}

fn dispatch_route(route: Route, args: &mut Vec<String>) -> Option<i32> {
    match route {
        Route::TypedLexical { request }
            if request[0].operation == Operation::Run
                && request[0].entry_policy == EntryPolicy::Process
                && request[0].json =>
        {
            Some(process_option_error(
                "process-mode run does not support --message-format=json",
            ))
        }
        Route::TypedLexical { request } => Some(process_file_with_lexical(
            &request[0].path,
            request[0].json,
            request[0].operation,
            request[0].output.as_deref(),
            request[0].entry_policy,
            Some(&request[0].bundle),
        )),
        Route::TypedImport { request } => Some(process_import(&request[0])),
        Route::TypedProducer { request } => Some(process_producer(&request[0])),
        Route::ProcessError { message } => Some(process_option_error(&message)),
        Route::TypedRun {
            json: true,
            entry_policy: EntryPolicy::Process,
            ..
        } => Some(process_option_error(
            "process-mode run does not support --message-format=json",
        )),
        Route::Legacy(legacy) => {
            *args = legacy;
            None
        }
        Route::Error {
            message,
            json,
            operation,
        } => Some(report(
            &SourceMap::new(),
            vec![*Diagnostic::new("E0001", "cli", message, None)],
            json,
            Summary::empty(operation),
        )),
        Route::FormatError { message } => Some(super::format_cli::cli_error(&message)),
        Route::TypedFormat { path, check } => Some(super::format_cli::process_file(&path, check)),
        Route::TypedCheck { path, json } => Some(process_file(
            &path,
            json,
            Operation::Check,
            None,
            EntryPolicy::Result,
        )),
        Route::TypedRun {
            path,
            json,
            entry_policy,
        } => Some(process_file(
            &path,
            json,
            Operation::Run,
            None,
            entry_policy,
        )),
        Route::TypedCompile {
            path,
            json,
            output,
            entry_policy,
        } => Some(process_file(
            &path,
            json,
            Operation::Compile,
            Some(&output),
            entry_policy,
        )),
    }
}
/// Actual bridge-only I/O/result carriers, debited by the import facade. The
/// already loaded ProjectSources and ordinary reporting infrastructure retain
/// their existing baseline. This sum is conservative, not a stack/RSS claim.
pub(super) fn import_transport_bytes() -> usize {
    use std::{
        fs::{File, Metadata},
        io,
        mem::size_of,
    };
    size_of::<[u8; oir::IMPORT_BYTES + 1]>()
        + 3 * size_of::<Result<Summary, Vec<Diagnostic>>>()
        + 3 * size_of::<oir::Imported>()
        + 2 * size_of::<File>()
        + 2 * size_of::<io::Result<Metadata>>()
        + 2 * size_of::<io::Result<usize>>()
        + size_of::<[usize; 4]>()
        + 2 * size_of::<&options::ImportOptions>()
        + 2 * size_of::<&[u8]>()
}

/// Explicit imported Result route; ordinary loading/reporting/publication stay
/// shared. Read at most the exact wire plus one trailing-byte witness.
fn process_import(request: &options::ImportOptions) -> i32 {
    use std::io::Read;
    let project = match ProjectSources::load_typed(&request.path, ProjectLimits::default()) {
        Ok(project) => project,
        Err(failure) => {
            return report(
                &failure.sources,
                failure.diagnostics,
                request.json,
                Summary::empty(request.operation),
            )
        }
    };
    let mut observation = [0u8; oir::IMPORT_BYTES + 1];
    let result = (|| {
        // Same stable-filesystem model as source loading, not a TOCTOU
        // sandbox. Reject ordinary FIFOs/devices/directories before opening.
        if !std::fs::metadata(&request.observation).is_ok_and(|meta| meta.is_file()) {
            return Err(vec![*Diagnostic::new(
                "E0702",
                "hir-import",
                "experimental HIR observation must be a readable regular file",
                None,
            )]);
        }
        let mut file = std::fs::File::open(&request.observation).map_err(|_| {
            vec![*Diagnostic::new(
                "E0702",
                "hir-import",
                "cannot open experimental HIR observation",
                None,
            )]
        })?;
        if !file.metadata().is_ok_and(|meta| meta.is_file()) {
            return Err(vec![*Diagnostic::new(
                "E0702",
                "hir-import",
                "experimental HIR observation must be a readable regular file",
                None,
            )]);
        }
        let mut length = 0;
        while length < observation.len() {
            match file.read(&mut observation[length..]) {
                Ok(0) => break,
                Ok(count) => length += count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    return Err(vec![*Diagnostic::new(
                        "E0702",
                        "hir-import",
                        "cannot read experimental HIR observation",
                        None,
                    )])
                }
            }
        }
        if length != oir::IMPORT_BYTES {
            return Err(vec![*Diagnostic::new("E0702", "hir-import", "experimental HIR observation must contain exactly 2607 bytes with no trailing data", None)]);
        }
        match oir::import_checked(&project, &observation[..length], request.operation)? {
            oir::Imported::Checked(functions) => Ok(Summary::Check(Some(functions))),
            oir::Imported::Ran(value) => Ok(Summary::Run(Some(value.into()))),
            oir::Imported::Emitted(module) => {
                let output = request
                    .output
                    .as_deref()
                    .expect("import compile validates output");
                super::native::compile(&module, output).map_err(|error| vec![*error])?;
                Ok(Summary::Compile(Some(output.to_string())))
            }
        }
    })();
    match result {
        Ok(summary) => report(project.sources(), Vec::new(), request.json, summary),
        Err(errors) => report(
            project.sources(),
            errors,
            request.json,
            Summary::empty(request.operation),
        ),
    }
}

/// New producer-only CLI and driver carriers, debited by its import facade.
/// Complete types name conservative local, moved and result transport roles;
/// this is requested storage accounting, not a physical stack or RSS estimate.
/// The loaded ProjectSources and ordinary reporting keep their prior baseline.
pub(super) fn producer_transport_bytes() -> usize {
    use super::hir_producer;
    use std::{mem::size_of, path::Path};

    // New classifier selection/local extraction, options input/local/moved box
    // payload, and exact reserve/box-conversion transports. The already owned
    // argv String payloads retain their baseline; these headers are additional.
    2 * size_of::<Option<String>>()
        + size_of::<String>()
        + 3 * size_of::<options::ProducerOptions>()
        + size_of::<Vec<options::ProducerOptions>>()
        + 2 * size_of::<Box<[options::ProducerOptions; 1]>>()
        + size_of::<Box<[options::ProducerOptions]>>()
        + size_of::<Result<Box<[options::ProducerOptions; 1]>, Box<[options::ProducerOptions]>>>()
        + 2 * size_of::<&options::ProducerOptions>()
        // Producer return/local and the complete fixed observation extraction.
        + 2 * size_of::<hir_producer::Outcome>()
        + size_of::<Option<hir_producer::Observation>>()
        + size_of::<hir_producer::Observation>()
        + 2 * size_of::<&hir_producer::Outcome>()
        + 2 * size_of::<&ProjectSources>()
        + 2 * size_of::<&Path>()
        + 2 * size_of::<&hir_producer::Observation>()
        + 2 * size_of::<&[u8]>()
        // Imported call, held result and match/closure return carriers.
        + 3 * size_of::<Result<oir::Imported, Vec<Diagnostic>>>()
        + 3 * size_of::<oir::Imported>()
        + 3 * size_of::<Result<Summary, Vec<Diagnostic>>>()
        + 2 * size_of::<Option<&'static str>>()
        + 2 * size_of::<Operation>()
        + 2 * size_of::<bool>()
}

/// Explicit producer Result route. Load genuine source once, run the producer
/// pipeline, and submit its unchanged fixed observation to the checked importer.
fn process_producer(request: &options::ProducerOptions) -> i32 {
    use super::hir_producer;
    use std::path::Path;

    let project = match ProjectSources::load_typed(&request.path, ProjectLimits::default()) {
        Ok(project) => project,
        Err(failure) => {
            return report(
                &failure.sources,
                failure.diagnostics,
                request.json,
                Summary::empty(request.operation),
            )
        }
    };
    let outcome = hir_producer::produce(&project, Path::new(&request.bundle));
    hir_producer::print_records(&outcome, request.json);
    let result = (|| {
        if let Some(error) = outcome.error {
            return Err(vec![*Diagnostic::new("E0703", "hir-producer", error, None)]);
        }
        let Some(observation) = outcome.observation else {
            return Err(vec![*Diagnostic::new(
                "E0703",
                "hir-producer",
                "experimental HIR producers returned no observation",
                None,
            )]);
        };
        match oir::import_produced(&project, observation.as_bytes(), request.operation)? {
            oir::Imported::Checked(functions) => Ok(Summary::Check(Some(functions))),
            oir::Imported::Ran(value) => Ok(Summary::Run(Some(value.into()))),
            oir::Imported::Emitted(module) => {
                let output = request
                    .output
                    .as_deref()
                    .expect("producer compile validates output");
                super::native::compile(&module, output).map_err(|error| vec![*error])?;
                Ok(Summary::Compile(Some(output.to_string())))
            }
        }
    })();
    match result {
        Ok(summary) => report(project.sources(), Vec::new(), request.json, summary),
        Err(errors) => report(
            project.sources(),
            errors,
            request.json,
            Summary::empty(request.operation),
        ),
    }
}

enum Summary {
    Check(Option<usize>),
    Run(Option<oir::RuntimeValue>),
    Compile(Option<String>),
}
impl Summary {
    fn empty(operation: Operation) -> Self {
        match operation {
            Operation::Check => Self::Check(None),
            Operation::Run => Self::Run(None),
            Operation::Compile => Self::Compile(None),
        }
    }
}
fn report(sources: &SourceMap, diagnostics: Vec<Diagnostic>, json: bool, summary: Summary) -> i32 {
    let success = diagnostics.is_empty();
    for diagnostic in &diagnostics {
        if json {
            println!("{}", diagnostic.render_json(sources));
        } else {
            eprint!("{}", diagnostic.render_human(sources));
        }
    }
    match summary {
        Summary::Check(functions) => {
            if json {
                println!("{}", check_summary(diagnostics.len(), functions));
            } else if success {
                println!(
                    "typed-preview check ok ({} functions; check only)",
                    functions.unwrap_or(0)
                );
            }
        }
        Summary::Compile(output) => {
            if json {
                println!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"compile-summary\",\"success\":{},\"errors\":{},\"output\":{}}}", success, diagnostics.len(), output.filter(|_| success).map_or("null".into(), |p| json_string(&p)));
            } else if let Some(path) = output.filter(|_| success) {
                println!("typed-preview native compile ok: {}", json_string(&path));
            }
        }
        Summary::Run(result) => {
            use std::io::Write;
            let mut output = std::io::stdout().lock();
            let written = if json {
                writeln!(output, "{}", run_runtime_summary(diagnostics.len(), result))
            } else if let Some(result) = result.filter(|_| success) {
                writeln!(output, "{result}")
            } else {
                Ok(())
            };
            if written.and_then(|()| output.flush()).is_err() {
                return 74;
            }
        }
    }
    exit_status(&diagnostics)
}
fn check_summary(errors: usize, functions: Option<usize>) -> String {
    format!("{{\"schema_version\":1,\"edition\":{},\"kind\":\"check-summary\",\"success\":{},\"errors\":{},\"functions\":{}}}",
        json_string("typed-preview"), errors == 0, errors,
        functions.filter(|_| errors == 0).map_or("null".to_string(), |n| n.to_string()))
}

#[cfg(test)]
fn run_summary(errors: usize, result: Option<oir::Scalar>) -> String {
    if matches!(result, Some(oir::Scalar::U8(_))) {
        return run_runtime_summary(errors.max(1), None);
    }
    run_runtime_summary(errors, result.map(oir::RuntimeValue::from))
}
fn run_runtime_summary(errors: usize, result: Option<oir::RuntimeValue>) -> String {
    format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":{},\"errors\":{errors},\"result\":{}}}",
        errors == 0, result.filter(|_| errors == 0).and_then(oir::RuntimeValue::json).unwrap_or_else(|| "null".into()))
}

fn exit_status(diagnostics: &[Diagnostic]) -> i32 {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "E0500")
    {
        2
    } else {
        i32::from(!diagnostics.is_empty())
    }
}

/// Process errors never use the ordinary JSON/scalar stdout reporter. Callers
/// establish terminal safety before constructing or rendering any diagnostics.
fn report_process(sources: &SourceMap, diagnostics: &[Diagnostic]) -> i32 {
    for diagnostic in diagnostics {
        if oir::process::diagnostic(diagnostic.render_human(sources).as_bytes()) != 1 {
            return 74;
        }
    }
    1
}

fn process_option_error(message: &str) -> i32 {
    if !oir::process::setup() {
        return 74;
    }
    report_process(
        &SourceMap::new(),
        &[*Diagnostic::new("E0001", "cli", message, None)],
    )
}

fn process_file(
    path: &str,
    json: bool,
    operation: Operation,
    output: Option<&str>,
    entry_policy: EntryPolicy,
) -> i32 {
    process_file_with_lexical(path, json, operation, output, entry_policy, None)
}

fn process_file_with_lexical(
    path: &str,
    json: bool,
    operation: Operation,
    output: Option<&str>,
    entry_policy: EntryPolicy,
    lexical_bundle: Option<&str>,
) -> i32 {
    let process_run = operation == Operation::Run && entry_policy == EntryPolicy::Process;
    // Pure argv classification already finished. Setup must precede even source
    // loading, because all subsequent failures use the process stderr channel.
    if process_run && !oir::process::setup() {
        return 74;
    }
    if entry_policy == EntryPolicy::Process && !oir::process::supported_host() {
        let diagnostic = *Diagnostic::new(
            "E0608",
            "oir-run",
            "process execution requires Linux or macOS on x86_64 or ARM64",
            None,
        );
        return if process_run {
            report_process(&SourceMap::new(), &[diagnostic])
        } else {
            report(
                &SourceMap::new(),
                vec![diagnostic],
                json,
                Summary::empty(operation),
            )
        };
    }
    let loaded = if let Some(directory) = lexical_bundle {
        let mut provider =
            match super::lexical_provider::Provider::load(std::path::Path::new(directory)) {
                Ok(provider) => provider,
                Err(message) => {
                    let diagnostic = *Diagnostic::new("E0703", "lexical-provider", message, None);
                    return if process_run {
                        report_process(&SourceMap::new(), &[diagnostic])
                    } else {
                        report(
                            &SourceMap::new(),
                            vec![diagnostic],
                            json,
                            Summary::empty(operation),
                        )
                    };
                }
            };
        let result =
            ProjectSources::load_typed_with_provider(path, ProjectLimits::default(), &mut provider);
        let sources = match &result {
            Ok(project) => project.sources(),
            Err(failure) => &failure.sources,
        };
        if !provider.print_receipts(sources, json, process_run) {
            return 74;
        }
        // Drop executable ownership, transport accounting and receipt storage
        // before canonical HIR checking or native output publication begins.
        drop(provider);
        result
    } else {
        ProjectSources::load_typed(path, ProjectLimits::default())
    };
    let project = match loaded {
        Ok(project) => project,
        Err(failure) => {
            return if process_run {
                report_process(&failure.sources, &failure.diagnostics)
            } else {
                report(
                    &failure.sources,
                    failure.diagnostics,
                    json,
                    Summary::empty(operation),
                )
            }
        }
    };
    let executable = check_loaded_project(&project);
    if process_run {
        return match executable {
            Ok(verified) => match verified.run_process() {
                Ok(status) => status,
                Err(oir::ProcessFailure::Setup) => 74,
                Err(oir::ProcessFailure::Diagnostic(diagnostic)) => {
                    report_process(project.sources(), &[*diagnostic])
                }
            },
            Err(diagnostics) => report_process(project.sources(), &diagnostics),
        };
    }
    if entry_policy == EntryPolicy::Process {
        return process_compile_loaded(&project, json, output, executable);
    }
    process_loaded(&project, json, operation, output, executable)
}

fn check_loaded_project(
    project: &ProjectSources,
) -> Result<oir::CheckedSourceProgram<'_>, Vec<Diagnostic>> {
    match project.syntax_flavor() {
        SyntaxFlavor::OriginalSingleFile => {
            let (source, ast) = project
                .original_file()
                .expect("original syntax retains one source file");
            debug_assert!(!ast.tokens.is_empty());
            oir::check_source(source, ast, project.sources())
        }
        SyntaxFlavor::ProjectSyntax => {
            let limits = IndexLimits::default();
            let work = WorkMeter::new(limits.work);
            let mut allocator = Allocator::default();
            oir::project::check_project_executable(project, limits, &work, &mut allocator)
        }
    }
}

/// Manifest builds use the same checked program as explicit file operations.
/// Only the publication policy differs: a managed artifact can be replaced after
/// successful compilation. No output directory is touched before verification.
pub(crate) fn build_project(path: &str, output: &str, json: bool, process: bool) -> i32 {
    let project = match ProjectSources::load_typed(path, ProjectLimits::default()) {
        Ok(project) => project,
        Err(failure) => {
            return report(
                &failure.sources,
                failure.diagnostics,
                json,
                Summary::Compile(None),
            );
        }
    };
    let result = (|| {
        let verified = check_loaded_project(&project)?;
        super::windows_native::preflight(&project, process, None).map_err(|error| vec![*error])?;
        let module = if process {
            verified.native_process_module()
        } else {
            verified.native_module()
        }
        .map_err(|error| vec![*error])?;
        super::native::compile_managed(&module, output).map_err(|error| vec![*error])?;
        Ok(Summary::Compile(Some(output.to_string())))
    })();
    match result {
        Ok(summary) => report(project.sources(), Vec::new(), json, summary),
        Err(diagnostics) => report(project.sources(), diagnostics, json, Summary::Compile(None)),
    }
}

/// Shared post-load operation path for both original and project source syntax.
fn process_loaded(
    project: &ProjectSources,
    json: bool,
    operation: Operation,
    output: Option<&str>,
    executable: Result<oir::CheckedSourceProgram<'_>, Vec<Diagnostic>>,
) -> i32 {
    let sources = project.sources();
    let result = (|| {
        let verified = executable?;
        match operation {
            Operation::Check => Ok(Summary::Check(Some(verified.function_count()))),
            Operation::Compile => {
                super::windows_native::preflight(project, false, None)
                    .map_err(|error| vec![*error])?;
                let module = verified.native_module().map_err(|e| vec![*e])?;
                let output = output.expect("compile route validates output");
                super::native::compile(&module, output).map_err(|e| vec![*e])?;
                Ok(Summary::Compile(Some(output.to_string())))
            }
            Operation::Run => verified
                .run_result(crate::frontend::build_config::ExecutionPolicy::Bounded)
                .map(|value| Summary::Run(Some(value)))
                .map_err(|error| vec![*error]),
        }
    })();
    match result {
        Ok(summary) => report(sources, Vec::new(), json, summary),
        Err(diagnostics) => report(sources, diagnostics, json, Summary::empty(operation)),
    }
}

/// Compile reporting remains the ordinary build protocol and never performs
/// terminal setup or source execution, including for a Process executable.
fn process_compile_loaded(
    project: &ProjectSources,
    json: bool,
    output: Option<&str>,
    executable: Result<oir::CheckedSourceProgram<'_>, Vec<Diagnostic>>,
) -> i32 {
    let result = (|| {
        let verified = executable?;
        super::windows_native::preflight(project, true, None).map_err(|error| vec![*error])?;
        let module = verified.native_process_module().map_err(|e| vec![*e])?;
        let output = output.expect("compile route validates output");
        super::native::compile(&module, output).map_err(|e| vec![*e])?;
        Ok(Summary::Compile(Some(output.to_string())))
    })();
    match result {
        Ok(summary) => report(project.sources(), Vec::new(), json, summary),
        Err(diagnostics) => report(project.sources(), diagnostics, json, Summary::Compile(None)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn byte_scalar_cannot_be_serialized_as_an_entry_result() {
        assert_eq!(super::oir::Scalar::U8(255).json(), None);
        let summary = super::run_summary(0, Some(super::oir::Scalar::U8(255)));
        assert!(summary.contains("\"success\":false"));
        assert!(summary.contains("\"result\":null"));
        assert!(!summary.contains("255"));
    }

    use super::*;

    use crate::frontend::project::packages::PackageDependency;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT_PREPARED: AtomicUsize = AtomicUsize::new(0);

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn import_free_scalar_ranges_use_fresh_typing_in_both_driver_flavors() {
        // Independent results exercise direct owners, projected owners and
        // forwarded exclusive slices without enums, generics or Vec imports.
        for (text, expected, functions) in [
            (
                "fn score(p:&[i32])->i32{let a=p[0];let b=p[1];return a+b+p.len();}fn main()->i32{let a=[10,20,30,40];return score(&a[1..3]);}",
                52,
                2,
            ),
            (
                "struct R{values:[i32;4]}fn score(p:&[i32])->i32{let a=p[0];let b=p[1];return a+b+p.len();}fn projected(p:&R)->i32{return score(&*p.values[1..3]);}fn main()->i32{let r=R{values:[1,2,3,4]};return projected(&r);}",
                7,
                3,
            ),
            (
                "fn put(p:&mut[i32])->(){p[0]=77;return;}fn forward(p:&mut[i32])->(){put(&mut *p[1..2]);return;}fn main()->i32{let mut a=[10,20,30,40];forward(&mut a[1..4]);let x=a[2];let y=a[1];return x+y;}",
                97,
                3,
            ),
            (
                "struct R{values:[i32;4]}fn put(p:&mut[i32])->(){p[0]=77;return;}fn projected(p:&mut R)->(){put(&mut *p.values[1..3]);return;}fn main()->i32{let mut r=R{values:[1,2,3,4]};projected(&mut r);let x=r.values[1];let y=r.values[2];return x+y;}",
                80,
                3,
            ),
        ] {
            for project_syntax in [false, true] {
                let fixture = PreparedFixture::new();
                let source = if project_syntax {
                    fixture.write("app/src/child.ox", "fn unused()->(){return;}");
                    format!("mod child;{text}")
                } else {
                    text.to_owned()
                };
                fixture.write("app/src/main.ox", &source);
                let entry = fixture.0.join("app/src/main.ox");
                let project = ProjectSources::load_typed(
                    entry.to_str().unwrap(),
                    ProjectLimits::default(),
                )
                .unwrap();
                assert_eq!(
                    project.syntax_flavor(),
                    if project_syntax { SyntaxFlavor::ProjectSyntax }
                    else { SyntaxFlavor::OriginalSingleFile },
                );
                // Call the actual production selector used by file commands
                // and manifest builds; do not duplicate its dispatch here.
                let checked = check_loaded_project(&project).unwrap();
                assert_eq!(checked.route(), oir::project::ProjectRoute::Owned);
                assert_eq!(checked.function_count(), functions + usize::from(project_syntax));
                assert_eq!(checked.run().unwrap(), oir::Scalar::I32(expected), "{source}");
            }
        }
    }

    struct PreparedFixture(PathBuf);
    impl PreparedFixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "oxid-prepared-input-{}-{}",
                std::process::id(),
                NEXT_PREPARED.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
        fn write(&self, path: &str, source: &str) {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, source).unwrap();
        }
        fn spec(&self, name: &str, entry: &str, dependencies: &[(&str, usize)]) -> PackageSpec {
            PackageSpec {
                root: self.0.join(name),
                entry: self.0.join(name).join(entry),
                dependencies: dependencies
                    .iter()
                    .map(|&(alias, package)| PackageDependency {
                        alias: alias.into(),
                        package,
                    })
                    .collect(),
            }
        }
        #[cfg(target_os = "linux")]
        fn pair(&self, library_entry: &str) -> [PackageSpec; 2] {
            [
                self.spec("app", "src/main.ox", &[("lib", 1)]),
                self.spec("lib", library_entry, &[]),
            ]
        }
    }
    impl Drop for PreparedFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn prepared_single_package_preserves_syntax_flavor_usage_and_retained_input() {
        let fixture = PreparedFixture::new();
        fixture.write("app/src/main.ox", "fn main()->i32{return 42;}");
        let specs = [fixture.spec("app", "src/main.ox", &[])];
        let old =
            ProjectSources::load_typed(specs[0].entry.to_str().unwrap(), ProjectLimits::default())
                .unwrap();
        let prepared = prepare_package_project(&specs, None).unwrap();
        assert!(matches!(prepared.sources, PreparedSources::Single(_)));
        assert_eq!(
            prepared.project().syntax_flavor(),
            SyntaxFlavor::OriginalSingleFile
        );
        assert_eq!(prepared.project().usage(), old.usage());
        assert!(prepared.dependency_fingerprints().is_empty());
        fixture.write("app/src/main.ox", "fn main()->i32{return 99;}");
        let changed = prepare_package_project(&specs, None).unwrap();
        assert_eq!(
            changed.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(99)
        );
        std::fs::remove_dir_all(fixture.0.join("app")).unwrap();
        assert_eq!(
            prepared.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(42)
        );
        assert_eq!(prepared.execute(PackageCommand::Check, false, false), 0);
    }

    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn optional_debug_metadata_preserves_every_instruction_and_uses_retained_sources() {
        fn instructions(text: &str) -> Vec<String> {
            text.lines()
                .filter(|line| !line.trim().is_empty() && !line.starts_with('!'))
                .filter(|line| {
                    // Debug intrinsics describe values without executing a
                    // program operation. Keep every other declaration/call.
                    let line = line.trim_start();
                    ![
                        "declare void @llvm.dbg.value(",
                        "declare void @llvm.dbg.declare(",
                        "call void @llvm.dbg.value(",
                        "call void @llvm.dbg.declare(",
                    ]
                    .iter()
                    .any(|prefix| line.starts_with(prefix))
                })
                .map(|line| {
                    if let Some((before, _)) = line
                        .strip_prefix("define ")
                        .and_then(|_| line.split_once(" !dbg !"))
                    {
                        format!("{before} {{")
                    } else {
                        line.split_once(", !dbg !")
                            .map_or(line, |(before, _)| before)
                            .to_owned()
                    }
                })
                .collect()
        }
        for source in [
            "fn main()->i32{return 40+2;}",
            "use alloc::bytes::new;use alloc::bytes::try_push;use alloc::bytes::len;enum Decision<T>{Keep(T),Empty}fn main()->i32{let mut bytes=new();for index in 0..3{try_push(&mut bytes,index.to_u8_checked());}let value=Decision::Keep(bytes);match value{Decision::Keep(b)=>{return len(&b);},Decision::Empty=>{return 0;}}}",
        ] {
            let fixture = PreparedFixture::new();
            fixture.write("app/src/main.ox", source);
            let specs = [fixture.spec("app", "src/main.ox", &[])];
            let prepared = prepare_package_project(&specs, None).unwrap();
            let checked = prepared.checked().unwrap();
            let usage = prepared.project().usage();
            for relative in [false, true] {
                for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
                    for process in [false, true] {
                        let settings = NativeBuildSettings::new(relative).with_execution(policy);
                        let plain = prepared.native_module_with_settings(&checked, process, &settings, &specs[0].root).unwrap();
                        let settings = settings.with_debug_info(true);
                        let debug = prepared.native_module_with_settings(&checked, process, &settings, &specs[0].root).unwrap();
                        assert_eq!(instructions(&debug), instructions(&plain));
                        assert!(debug.contains("!DICompileUnit") && debug.contains("!DISubprogram(name: \"main\""));
                        if specs[0].root.exists() { std::fs::remove_dir_all(&specs[0].root).unwrap(); }
                        assert_eq!(prepared.native_module_with_settings(&checked, process, &settings, &specs[0].root).unwrap(), debug);
                    }
                }
            }
            assert_eq!(prepared.project().usage(), usage);
        }
    }

    #[test]
    fn prepared_native_names_keep_singleton_ownership_and_default_emission_after_removal() {
        let fixture = PreparedFixture::new();
        fixture.write("app/src/main.ox", "fn main()->i32{return 40+2;}");
        let mut specs = [fixture.spec("app", "src/main.ox", &[])];
        // Keep the admitted synthetic path in the host's canonical separator
        // spelling, including Windows compiler-host unit tests.
        specs[0].entry = specs[0].root.join("src").join("main.ox");
        let prepared = prepare_package_project(&specs, None).unwrap();
        let usage = prepared.project().usage();
        let heap = prepared.project().sources().heap_capacity_bytes();
        let checked = prepared.checked().unwrap();
        let original = checked.native_module().unwrap();
        assert_eq!(
            prepared
                .native_module_with_settings(
                    &checked,
                    false,
                    &NativeBuildSettings::new(false),
                    Path::new("irrelevant-original-presentation-root"),
                )
                .unwrap(),
            original
        );
        let settings = NativeBuildSettings::new(true);
        let relative = prepared
            .native_module_with_settings(&checked, false, &settings, &specs[0].root)
            .unwrap();
        let names = prepared.native_source_names(&specs[0].root).unwrap();
        assert_eq!(
            names.path(super::super::source::SourceFileId(0)),
            Some("src/main.ox")
        );
        names.validate(prepared.project().sources()).unwrap();
        assert_ne!(relative, original);
        fixture.write("app/src/main.ox", "not valid source anymore");
        std::fs::remove_dir_all(&specs[0].root).unwrap();
        assert_eq!(
            prepared
                .native_module_with_settings(&checked, false, &settings, &specs[0].root)
                .unwrap(),
            relative
        );
        assert_eq!(
            prepared.checked().unwrap().native_module().unwrap(),
            original
        );
        assert_eq!(prepared.project().usage(), usage);
        assert_eq!(prepared.project().sources().heap_capacity_bytes(), heap);
        assert_eq!(
            prepared.project().syntax_flavor(),
            SyntaxFlavor::OriginalSingleFile
        );
        assert_eq!(
            prepared.workspace_preflight_with_native_settings(
                false,
                false,
                &settings,
                &specs[0].root
            ),
            0
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_relative_native_ir_preserves_diamond_and_unicode_sibling_layout_after_removal() {
        fn graph(fixture: &PreparedFixture) -> Vec<PackageSpec> {
            fixture.write(
                "workspace/app/main.ox",
                "fn main()->i32{return crate::left::value()+crate::right::value();}",
            );
            fixture.write(
                "workspace/left/lib.ox",
                "pub fn value()->i32{return crate::shared::value();}",
            );
            fixture.write(
                "workspace/right/lib.ox",
                "pub fn value()->i32{return crate::shared::value();}",
            );
            fixture.write(
                "共享/target/lib.ox",
                "mod nested; pub fn value()->i32{return crate::nested::value();}",
            );
            fixture.write("共享/target/nested.ox", "pub fn value()->i32{return 21;}");
            vec![
                fixture.spec("workspace/app", "main.ox", &[("left", 1), ("right", 2)]),
                fixture.spec("workspace/left", "lib.ox", &[("shared", 3)]),
                fixture.spec("workspace/right", "lib.ox", &[("shared", 3)]),
                fixture.spec("共享", "target/lib.ox", &[]),
            ]
        }
        let first = PreparedFixture::new();
        let second = PreparedFixture::new();
        let first_graph = graph(&first);
        let second_graph = graph(&second);
        let first_prepared = prepare_package_project(&first_graph, None).unwrap();
        let second_prepared = prepare_package_project(&second_graph, None).unwrap();
        let first_root = first.0.join("workspace");
        let second_root = second.0.join("workspace");
        let settings = NativeBuildSettings::new(true);
        let fingerprint = first_prepared
            .workspace_fingerprints(&first_graph[0])
            .unwrap();
        let names = first_prepared.native_source_names(&first_root).unwrap();
        let mut paths: Vec<_> = (0..first_prepared.input_usage().0)
            .map(|id| names.path(super::super::source::SourceFileId(id)).unwrap())
            .collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            [
                "../共享/target/lib.ox",
                "../共享/target/nested.ox",
                "app/main.ox",
                "left/lib.ox",
                "right/lib.ox"
            ]
        );
        let before = first_prepared.checked().unwrap();
        let after = second_prepared.checked().unwrap();
        for process in [false, true] {
            if process && !cfg!(target_arch = "x86_64") {
                continue;
            }
            let module = first_prepared
                .native_module_with_settings(&before, process, &settings, &first_root)
                .unwrap();
            assert_eq!(
                module,
                second_prepared
                    .native_module_with_settings(&after, process, &settings, &second_root)
                    .unwrap()
            );
        }
        std::fs::remove_dir_all(&first.0).unwrap();
        std::fs::remove_dir_all(&second.0).unwrap();
        assert_eq!(
            first_prepared
                .workspace_fingerprints(&first_graph[0])
                .unwrap(),
            fingerprint
        );
        assert_eq!(
            first_prepared.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(42)
        );
        assert_eq!(
            first_prepared
                .native_module_with_settings(
                    &first_prepared.checked().unwrap(),
                    false,
                    &settings,
                    &first_root
                )
                .unwrap(),
            second_prepared
                .native_module_with_settings(
                    &second_prepared.checked().unwrap(),
                    false,
                    &settings,
                    &second_root
                )
                .unwrap(),
        );
    }

    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    #[ignore = "requires LLVM 19.1.7 and the qualified native SDK"]
    fn prepared_reproducible_native_uses_retained_paths_and_bytes_after_source_removal() {
        let fixture = PreparedFixture::new();
        fixture.write(
            "app/src/main.ox",
            "fn main()->i32{return crate::lib::value();}",
        );
        fixture.write(
            "lib/src/lib.ox",
            "pub fn value()->i32{return 2147483647+1;}",
        );
        let specs = fixture.pair("src/lib.ox");
        let prepared = prepare_package_project(&specs, None).unwrap();
        std::fs::remove_dir_all(&specs[0].root).unwrap();
        std::fs::remove_dir_all(&specs[1].root).unwrap();
        let output = fixture.0.join(".oxid/bin/retained-relative-program");
        assert_eq!(
            prepared.execute_with_native_settings(
                PackageCommand::Compile {
                    output: output.to_str().unwrap(),
                    managed: true,
                    incremental: false
                },
                false,
                true,
                &NativeBuildSettings::new(true),
                &specs[0].root,
            ),
            0
        );
        let result = std::process::Command::new(output).output().unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(stderr.contains("../lib/src/lib.ox:1:"), "{stderr}");
        assert!(stderr.contains("overflow"), "{stderr}");
        assert!(!stderr.contains(fixture.0.to_str().unwrap()), "{stderr}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_fingerprint_known_answer_covers_raw_bytes_and_loaded_target_modules() {
        let fixture = PreparedFixture::new();
        fixture.write(
            "app/src/main.ox",
            "fn main()->i32{return crate::lib::value();}",
        );
        fixture.write(
            "lib/target/zlib.ox",
            "mod nested;\r\npub fn value()->i32{return crate::nested::value();}\r\n",
        );
        fixture.write(
            "lib/target/nested.ox",
            "// λ\npub fn value()->i32{return 42;}\n",
        );
        let specs = fixture.pair("target/zlib.ox");
        let prepared = prepare_package_project(&specs, None).unwrap();
        // Independently calculated with Python hashlib and struct.pack('<Q').
        assert_eq!(
            prepared.dependency_fingerprints(),
            &[PackageInputFingerprint {
                package: 1,
                checksum: "sha256:b11e12f6f2dce024527bca9500ce04e7695e0e04f489bdc2f03f5c2a60875cc9"
                    .into(),
                files: 2,
                bytes: 104,
            }]
        );
        assert_eq!(
            prepared.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(42)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn package_source_visitor_keeps_retained_bytes_and_admits_remaining_limits() {
        let fixture = PreparedFixture::new();
        let text = "// λ\r\nfn main()->i32{return crate::lib::value();}\r\n";
        fixture.write("app/src/main.ox", text);
        fixture.write("lib/src/lib.ox", "pub fn value()->i32{return 17;}");
        let graph = fixture.pair("src/lib.ox");
        let error = prepare_package_project_bounded(&graph, None, 0, 1024)
            .unwrap_err()
            .message();
        assert!(error.contains("retained file budget"), "{error}");
        assert!(prepare_package_project_bounded(&graph, None, 2, text.len() - 1).is_err());
        assert!(prepare_package_project_bounded(&graph, None, 1, 1024).is_err());
        let prepared = prepare_package_project_bounded(&graph, None, 2, 1024).unwrap();
        std::fs::remove_dir_all(fixture.0.join("app")).unwrap();
        std::fs::remove_dir_all(fixture.0.join("lib")).unwrap();
        let mut seen = Vec::new();
        prepared
            .visit_package_sources(&graph[0], |package, path, bytes| {
                seen.push((package, path.to_owned(), bytes.to_vec()));
                Ok(())
            })
            .unwrap();
        assert_eq!(
            seen,
            vec![
                (0, "src/main.ox".to_owned(), text.as_bytes().to_vec()),
                (
                    1,
                    "src/lib.ox".to_owned(),
                    b"pub fn value()->i32{return 17;}".to_vec()
                ),
            ]
        );
        if cfg!(all(
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )) {
            prepared.source_package_preflight(Some(true)).unwrap();
        } else {
            assert!(prepared
                .source_package_preflight(Some(true))
                .unwrap_err()
                .contains("Linux x86_64"));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn workspace_member_fingerprint_matches_imported_bytes_and_binds_test_entry() {
        let fixture = PreparedFixture::new();
        fixture.write(
            "app/src/main.ox",
            "fn main()->i32{return crate::lib::value();}",
        );
        let library = "mod nested;\r\npub fn value()->i32{return crate::nested::value();}\r\n";
        fixture.write("lib/target/zlib.ox", library);
        fixture.write(
            "lib/target/nested.ox",
            "// λ\npub fn value()->i32{return 42;}\n",
        );
        let graph = fixture.pair("target/zlib.ox");
        let imported = prepare_package_project(&graph, None).unwrap();
        let member = [fixture.spec("lib", "target/zlib.ox", &[])];
        let standalone = prepare_package_project(&member, None).unwrap();
        let root_fingerprint = standalone.workspace_fingerprints(&member[0]).unwrap();
        assert_eq!(
            root_fingerprint[0].1,
            imported.dependency_fingerprints()[0].checksum
        );
        assert_eq!(
            root_fingerprint[0].1,
            "sha256:b11e12f6f2dce024527bca9500ce04e7695e0e04f489bdc2f03f5c2a60875cc9"
        );

        fixture.write("lib/target/ztest.ox", library);
        let test_path = fixture.0.join("lib/target/ztest.ox");
        let declared = prepare_package_project(&member, Some(&test_path)).unwrap();
        assert_ne!(
            declared.workspace_fingerprints(&member[0]).unwrap(),
            root_fingerprint
        );
        // Retained fingerprints do not reopen a member entry or its modules.
        std::fs::remove_dir_all(fixture.0.join("lib")).unwrap();
        assert_eq!(
            standalone.workspace_fingerprints(&member[0]).unwrap(),
            root_fingerprint
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_fingerprint_covers_transitive_dot_oxid_inputs_and_ignores_unloaded_files() {
        let fixture = PreparedFixture::new();
        fixture.write(
            "app/src/main.ox",
            "fn main()->i32{return crate::lib::value();}",
        );
        fixture.write(
            "lib/.oxid/lib.ox",
            "mod inner;pub fn value()->i32{return crate::inner::value();}",
        );
        fixture.write(
            "lib/.oxid/inner.ox",
            "mod deep;pub fn value()->i32{return crate::inner::deep::value();}",
        );
        fixture.write("lib/.oxid/inner/deep.ox", "pub fn value()->i32{return 42;}");
        let specs = fixture.pair(".oxid/lib.ox");
        let prepared = prepare_package_project(&specs, None).unwrap();
        assert_eq!(prepared.dependency_fingerprints()[0].files, 3);
        assert_eq!(
            prepared.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(42)
        );
        fixture.write("lib/README.md", "not a compiler input");
        fixture.write("lib/oxid.lock", "not a compiler source");
        fixture.write("lib/.oxid/unloaded.ox", "this file must not be parsed");
        let unmodified = prepare_package_project(&specs, None).unwrap();
        assert_eq!(
            prepared.dependency_fingerprints(),
            unmodified.dependency_fingerprints()
        );
        fixture.write("lib/.oxid/inner/deep.ox", "pub fn value()->i32{return 43;}");
        let modified = prepare_package_project(&specs, None).unwrap();
        assert_ne!(
            prepared.dependency_fingerprints(),
            modified.dependency_fingerprints()
        );
        assert_eq!(
            modified.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(43)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_fingerprints_ignore_absolute_paths_source_ids_and_root_test_overrides() {
        let fixtures = [PreparedFixture::new(), PreparedFixture::new()];
        for (index, fixture) in fixtures.iter().enumerate() {
            let sources = [
                (
                    "lib/src/lib.ox",
                    "mod value;pub fn value()->i32{return crate::value::get();}",
                ),
                ("lib/src/value.ox", "pub fn get()->i32{return 42;}"),
            ];
            for &(name, source) in sources.iter().cycle().skip(index).take(2) {
                fixture.write(name, source);
            }
            fixture.write(
                "app/src/main.ox",
                "fn main()->i32{return crate::lib::value();}",
            );
            fixture.write(
                "app/tests/declared.ox",
                "fn main()->i32{return crate::lib::value()-42;}",
            );
        }
        let first = fixtures[0].pair("src/lib.ox");
        let second = fixtures[1].pair("src/lib.ox");
        let prepared = prepare_package_project(&first, None).unwrap();
        // An additional root module changes dependency SourceFileId values.
        fixtures[1].write(
            "app/src/main.ox",
            "mod extra;fn main()->i32{return crate::extra::number()+crate::lib::value();}",
        );
        fixtures[1].write("app/src/extra.ox", "pub fn number()->i32{return 0;}");
        let relocated = prepare_package_project(&second, None).unwrap();
        assert_eq!(
            prepared.dependency_fingerprints(),
            relocated.dependency_fingerprints()
        );
        let test_entry = fixtures[1].0.join("app/tests/declared.ox");
        let declared = prepare_package_project(&second, Some(&test_entry)).unwrap();
        assert_eq!(
            prepared.dependency_fingerprints(),
            declared.dependency_fingerprints()
        );
        assert_eq!(
            declared.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(0)
        );
        assert_eq!(second[0].entry, fixtures[1].0.join("app/src/main.ox"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_fingerprint_binds_exact_entry_path_without_backslash_normalization() {
        let fixture = PreparedFixture::new();
        fixture.write(
            "app/src/main.ox",
            "fn main()->i32{return crate::lib::value();}",
        );
        for path in [
            "lib/left/lib.ox",
            "lib/right/lib.ox",
            "lib/odd\\name.ox",
            "lib/odd/name.ox",
        ] {
            fixture.write(path, "pub fn value()->i32{return 42;}");
        }
        let checksums: Vec<_> = ["left/lib.ox", "right/lib.ox", "odd\\name.ox", "odd/name.ox"]
            .iter()
            .map(|path| {
                let specs = fixture.pair(path);
                prepare_package_project(&specs, None)
                    .unwrap()
                    .dependency_fingerprints()[0]
                    .checksum
                    .clone()
            })
            .collect();
        for (index, checksum) in checksums.iter().enumerate() {
            assert!(!checksums[..index].contains(checksum));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_fingerprint_is_independent_of_package_graph_ordinal() {
        let fixture = PreparedFixture::new();
        fixture.write(
            "app/src/main.ox",
            "fn main()->i32{return crate::left::value()+crate::right::value();}",
        );
        fixture.write("left/src/lib.ox", "pub fn value()->i32{return 20;}");
        fixture.write("right/src/lib.ox", "pub fn value()->i32{return 22;}");
        let first = [
            fixture.spec("app", "src/main.ox", &[("left", 1), ("right", 2)]),
            fixture.spec("left", "src/lib.ox", &[]),
            fixture.spec("right", "src/lib.ox", &[]),
        ];
        let second = [
            fixture.spec("app", "src/main.ox", &[("left", 2), ("right", 1)]),
            fixture.spec("right", "src/lib.ox", &[]),
            fixture.spec("left", "src/lib.ox", &[]),
        ];
        let before = prepare_package_project(&first, None).unwrap();
        let after = prepare_package_project(&second, None).unwrap();
        for (original, reordered) in [(0, 1), (1, 0)] {
            let original = &before.dependency_fingerprints()[original];
            let reordered = &after.dependency_fingerprints()[reordered];
            assert_ne!(original.package, reordered.package);
            assert_eq!(original.checksum, reordered.checksum);
            assert_eq!(original.files, reordered.files);
            assert_eq!(original.bytes, reordered.bytes);
        }
        assert_eq!(
            before.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(42)
        );
        assert_eq!(
            after.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(42)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_package_execution_uses_the_same_fingerprinted_bytes_after_disk_mutation() {
        let fixture = PreparedFixture::new();
        fixture.write(
            "app/src/main.ox",
            "fn main()->i32{return crate::lib::value();}",
        );
        fixture.write("lib/src/lib.ox", "pub fn value()->i32{return 42;}");
        let specs = fixture.pair("src/lib.ox");
        let prepared = prepare_package_project(&specs, None).unwrap();
        let fingerprints = prepared.dependency_fingerprints().to_vec();
        fixture.write("lib/src/lib.ox", "pub fn value()->i32{return 99;}");
        let changed = prepare_package_project(&specs, None).unwrap();
        assert_ne!(fingerprints, changed.dependency_fingerprints());
        assert_eq!(
            changed.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(99)
        );
        std::fs::remove_dir_all(fixture.0.join("app")).unwrap();
        std::fs::remove_dir_all(fixture.0.join("lib")).unwrap();
        assert_eq!(prepared.dependency_fingerprints(), fingerprints);
        assert_eq!(
            prepared.checked().unwrap().run().unwrap(),
            oir::Scalar::I32(42)
        );
        assert_eq!(prepared.execute(PackageCommand::Check, false, false), 0);
        assert_eq!(prepared.execute(PackageCommand::Run, false, false), 0);
        assert!(prepare_package_project(&specs, None).is_err());
    }

    #[test]
    fn prepared_loading_does_not_typecheck_or_execute() {
        let fixture = PreparedFixture::new();
        fixture.write("app/src/main.ox", "fn main()->i32{return true+1;}");
        let specs = [fixture.spec("app", "src/main.ox", &[])];
        let prepared = prepare_package_project(&specs, None).unwrap();
        let errors = prepared.checked().unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "E0300"),
            "{errors:?}"
        );
    }

    #[test]
    fn prepared_error_message_is_bounded_sanitized_and_utf8_complete() {
        let error = PackagePreparationError {
            sources: SourceMap::new(),
            diagnostics: vec![*Diagnostic::new(
                "E0006",
                "package",
                format!("\u{1b}[31m{}", "λ".repeat(3000)),
                None,
            )],
        };
        let message = error.message();
        assert!(message.len() <= 2048);
        assert!(!message.contains('\u{1b}'));
        assert!(message.contains('λ'));
        assert!(message.ends_with('λ'));
    }

    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    #[ignore = "requires pinned LLVM 19.1.7 tools"]
    fn prepared_native_uses_loaded_sources_after_removal() {
        let fixture = PreparedFixture::new();
        fixture.write(
            "app/src/main.ox",
            "fn main()->i32{return crate::lib::value();}",
        );
        fixture.write("lib/src/lib.ox", "pub fn value()->i32{return 42;}");
        let specs = fixture.pair("src/lib.ox");
        let prepared = prepare_package_project(&specs, None).unwrap();
        std::fs::remove_dir_all(fixture.0.join("app")).unwrap();
        std::fs::remove_dir_all(fixture.0.join("lib")).unwrap();
        let output = fixture.0.join(".oxid/bin/retained-program");
        assert_eq!(
            prepared.execute(
                PackageCommand::Compile {
                    output: output.to_str().unwrap(),
                    managed: true,
                    incremental: false,
                },
                false,
                true
            ),
            0
        );
        let result = std::process::Command::new(&output).output().unwrap();
        assert_eq!(result.status.code(), Some(42));
        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn process_dispatch_is_typed_and_cannot_reach_legacy_arguments() {
        // Classification is safe to test on every host without changing the
        // test harness signal policy. Public subprocess tests cover execution.
        for values in [
            &[
                "run",
                "missing.ox",
                "--edition=typed-preview",
                "--entry-mode=process",
            ][..],
            &["unknown", "--entry-mode=process"],
            &["run", "--edition=typed-preview", "--entry-mode="],
        ] {
            let arguments: Vec<String> = values.iter().map(|s| (*s).into()).collect();
            assert!(!matches!(options::route(&arguments), Route::Legacy(_)));
        }
    }

    #[test]
    fn diagnostic_exit_status_preserves_source_errors_and_distinguishes_oir_internal_errors() {
        assert_eq!(exit_status(&[]), 0);
        for (code, stage) in [("E0300", "type"), ("E0400", "oir-lower")] {
            assert_eq!(
                exit_status(&[*Diagnostic::new(code, stage, "failure", None)]),
                1
            );
        }
        let diagnostic = *Diagnostic::new("E0500", "oir-verify", "internal compiler error", None);
        assert_eq!(exit_status(std::slice::from_ref(&diagnostic)), 2);
        assert!(diagnostic
            .render_json(&SourceMap::new())
            .contains("\"primary\":null"));
        assert_eq!(
            report(
                &SourceMap::new(),
                vec![diagnostic],
                true,
                Summary::Check(None)
            ),
            2
        );
        assert_eq!(check_summary(1, Some(7)), "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}");
    }
}
