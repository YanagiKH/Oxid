use super::{
    diagnostic::{json_string, Diagnostic},
    oir,
    options::{self, EntryPolicy, Operation, Route},
    project::{ProjectLimits, ProjectSources},
    session::{CompilerSession, Configuration, Engine},
    source::SourceMap,
};

/// None returns an untouched/default or explicitly legacy command to the old CLI.
pub fn dispatch(args: &mut Vec<String>) -> Option<i32> {
    dispatch_route(options::route(args), args)
}

fn dispatch_route(route: Route, args: &mut Vec<String>) -> Option<i32> {
    match route {
        Route::TypedDoctor => Some(super::toolchain_doctor::run()),
        Route::TypedNew { path } => Some(scaffold_project(&path)),
        Route::ManifestPackage {
            request,
            output,
            entry_policy,
        } => Some(super::project_package::run(request, &output, entry_policy)),
        Route::ManifestPackageError {
            failure,
            json,
            entry_policy,
        } => Some(super::project_package::cli_error(
            failure,
            json,
            entry_policy,
        )),
        Route::ManifestBuild {
            request,
            entry_policy,
        } => Some(manifest_build(request, entry_policy)),
        Route::ManifestBuildError {
            failure,
            json,
            entry_policy,
        } => {
            let diagnostic = manifest_diagnostic(failure);
            Some(report_build(
                &SourceMap::new(),
                vec![diagnostic],
                json,
                entry_policy,
                None,
            ))
        }
        Route::ManifestError {
            failure,
            json,
            operation,
        } => Some(manifest_error(failure, json, operation)),
        Route::ManifestCheck { request } => Some(manifest_file(request, Operation::Check)),
        Route::ManifestRun { request } => Some(manifest_file(request, Operation::Run)),
        Route::ManifestProcessRun { request } => Some(manifest_process_run(request)),
        Route::ManifestProcessError { failure } => Some(if !oir::process::setup() {
            74
        } else {
            report_process(&SourceMap::new(), &[manifest_diagnostic(failure)])
        }),
        Route::ManifestTest {
            request,
            native,
            filter,
        } => {
            let filter = filter.map(|index| {
                args[index]
                    .strip_prefix("--filter=")
                    .unwrap_or(args[index].as_str())
            });
            Some(super::project_test::run(request, native, filter))
        }
        Route::ManifestTestError { failure, json } => {
            Some(super::project_test::cli_error(failure, json))
        }
        Route::ManifestNativeRun {
            request,
            entry_policy,
        } => Some(manifest_native_run(request, entry_policy)),
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
/// Project creation is a command, not a source-checking operation.
fn scaffold_project(path: &str) -> i32 {
    use std::io::Write;
    let result = super::project_scaffold::create(std::path::Path::new(path));
    let error = match result {
        Ok(()) => {
            if writeln!(
                std::io::stdout().lock(),
                "created typed project: {}",
                json_string(path)
            )
            .is_ok()
            {
                return 0;
            }
            format!("project {} was created, but its success message could not be written; files were retained", json_string(path))
        }
        Err(error) => error,
    };
    let diagnostic = Diagnostic::new("E0002", "project-new", error, None);
    let _ = writeln!(
        std::io::stderr().lock(),
        "{}",
        diagnostic.render_human(&SourceMap::new())
    );
    1
}

/// Explicit manifest CHECK or Result RUN: no output, provider, or Process policy.
fn manifest_file(request: Box<[super::manifest::Request; 1]>, operation: Operation) -> i32 {
    let json = request[0].json;
    let configuration = Configuration::new(
        operation.into(),
        Engine::Interpreter,
        EntryPolicy::Result,
        json,
    );
    match CompilerSession::load(request, configuration) {
        Ok(session) => process_loaded(
            session.project(),
            session.configuration().json,
            operation,
            None,
            session.check(),
        ),
        Err((sources, diagnostics)) => {
            report(&sources, diagnostics, json, Summary::empty(operation))
        }
    }
}

fn manifest_diagnostic(failure: super::manifest::Failure) -> Diagnostic {
    let (code, stage) = failure.classification();
    *Diagnostic::new(code, stage, failure.message(), None)
}

/// The same selected package/source session as ordinary manifest run, with
/// process signal setup before loading and status-only execution/reporting.
fn manifest_process_run(request: Box<[super::manifest::Request; 1]>) -> i32 {
    if !oir::process::setup() {
        return 74;
    }
    let session = match CompilerSession::load(
        request,
        Configuration::new(
            super::manifest::Command::Run,
            Engine::Interpreter,
            EntryPolicy::Process,
            false,
        ),
    ) {
        Ok(session) => session,
        Err((sources, diagnostics)) => return report_process(&sources, &diagnostics),
    };
    match session.check() {
        Ok(verified) => match verified.run_process() {
            Ok(status) => status,
            Err(oir::ProcessFailure::Setup) => 74,
            Err(oir::ProcessFailure::Diagnostic(diagnostic)) => {
                report_process(session.project().sources(), &[*diagnostic])
            }
        },
        Err(diagnostics) => report_process(session.project().sources(), &diagnostics),
    }
}

fn manifest_build(request: Box<[super::manifest::Request; 1]>, entry_policy: EntryPolicy) -> i32 {
    let json = request[0].json;
    let session = match CompilerSession::load(
        request,
        Configuration::new(
            super::manifest::Command::Build,
            Engine::Native,
            entry_policy,
            json,
        ),
    ) {
        Ok(session) => session,
        Err((sources, diagnostics)) => {
            return report_build(&sources, diagnostics, json, entry_policy, None);
        }
    };
    match session.build() {
        Ok(artifact) => report_build(
            session.project().sources(),
            Vec::new(),
            json,
            entry_policy,
            Some(artifact.path),
        ),
        Err(diagnostics) => report_build(
            session.project().sources(),
            diagnostics,
            json,
            entry_policy,
            None,
        ),
    }
}

fn manifest_native_run(
    request: Box<[super::manifest::Request; 1]>,
    entry_policy: EntryPolicy,
) -> i32 {
    let session = match CompilerSession::load(
        request,
        Configuration::new(
            crate::frontend::manifest::Command::Run,
            Engine::Native,
            entry_policy,
            false,
        ),
    ) {
        Ok(session) => session,
        Err((sources, diagnostics)) => return report_native_run(&sources, &diagnostics),
    };
    let artifact = match session.build() {
        Ok(artifact) => artifact,
        Err(diagnostics) => return report_native_run(session.project().sources(), &diagnostics),
    };
    // Release compiler/source owners before spawning. The artifact owns its inode,
    // independently of the managed path, receipts, and the released build lock.
    drop(session);
    match artifact.run() {
        Ok(status) => status,
        Err(diagnostic) => report_native_run(&SourceMap::new(), &[*diagnostic]),
    }
}

fn report_native_run(sources: &SourceMap, diagnostics: &[Diagnostic]) -> i32 {
    use std::io::Write;
    let stderr = &mut std::io::stderr().lock();
    for diagnostic in diagnostics {
        if write!(stderr, "{}", diagnostic.render_human(sources)).is_err() {
            return 1;
        }
    }
    if stderr.flush().is_err() {
        return 1;
    }
    exit_status(diagnostics)
}

fn report_build(
    sources: &SourceMap,
    diagnostics: Vec<Diagnostic>,
    json: bool,
    entry_policy: EntryPolicy,
    output: Option<String>,
) -> i32 {
    write_build_report(
        sources,
        &diagnostics,
        json,
        entry_policy,
        output.as_deref(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    )
}

fn write_build_report(
    sources: &SourceMap,
    diagnostics: &[Diagnostic],
    json: bool,
    entry_policy: EntryPolicy,
    published: Option<&str>,
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
) -> i32 {
    let success = diagnostics.is_empty();
    let mode = super::project_build::mode(entry_policy);
    let written = (|| -> std::io::Result<()> {
        for diagnostic in diagnostics {
            if json {
                writeln!(stdout, "{}", diagnostic.render_json(sources))?;
            } else {
                write!(stderr, "{}", diagnostic.render_human(sources))?;
            }
        }
        if json {
            writeln!(stdout, "{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"build-summary\",\"success\":{},\"errors\":{},\"output\":{},\"entry_mode\":{}}}", success, diagnostics.len(), published.filter(|_| success).map_or("null".into(), json_string), json_string(mode))?;
        } else if let Some(path) = published.filter(|_| success) {
            writeln!(
                stdout,
                "typed-preview native build ok: {}",
                json_string(path)
            )?;
        }
        stdout.flush()?;
        stderr.flush()
    })();
    if let Err(error) = written {
        let state = published.map_or("no executable was published".into(), |path| {
            format!(
                "executable already published at {}; previous artifact was not restored",
                json_string(path)
            )
        });
        let _ = writeln!(stderr, "error: build reporting failed: {error}; {state}");
        return 1;
    }
    exit_status(diagnostics)
}

fn manifest_error(failure: super::manifest::Failure, json: bool, operation: Operation) -> i32 {
    let (code, stage) = failure.classification();
    report(
        &SourceMap::new(),
        vec![*Diagnostic::new(code, stage, failure.message(), None)],
        json,
        Summary::empty(operation),
    )
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
            oir::Imported::Ran(value) => Ok(Summary::Run(Some(value))),
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
            oir::Imported::Ran(value) => Ok(Summary::Run(Some(value))),
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
    Run(Option<oir::Scalar>),
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
fn report(
    sources: &SourceMap,
    mut diagnostics: Vec<Diagnostic>,
    json: bool,
    summary: Summary,
) -> i32 {
    let summary = if matches!(summary, Summary::Run(Some(oir::Scalar::U8(_)))) {
        diagnostics.push(*Diagnostic::new(
            "E0500",
            "oir-run",
            "invalid byte entry result",
            None,
        ));
        Summary::Run(None)
    } else {
        summary
    };
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
            if json {
                println!("{}", run_summary(diagnostics.len(), result));
            } else if let Some(result) = result.filter(|_| success) {
                println!("{result}");
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

fn run_summary(errors: usize, result: Option<oir::Scalar>) -> String {
    let errors = if matches!(result, Some(oir::Scalar::U8(_))) {
        errors.max(1)
    } else {
        errors
    };
    format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":{},\"errors\":{errors},\"result\":{}}}",
        errors == 0, result.filter(|_| errors == 0).and_then(oir::Scalar::json).unwrap_or_else(|| "null".into()))
}

pub(super) fn exit_status(diagnostics: &[Diagnostic]) -> i32 {
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
            "process execution requires Linux x86_64",
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
    super::session::check_project(project)
}

/// Emission only: no publication, reporting, terminal setup, or execution.
fn native_module(
    verified: &oir::CheckedSourceProgram<'_>,
    entry_policy: EntryPolicy,
) -> Result<String, Box<Diagnostic>> {
    match entry_policy {
        EntryPolicy::Result => verified.native_module(),
        EntryPolicy::Process => verified.native_process_module(),
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
                let module = native_module(&verified, EntryPolicy::Result).map_err(|e| vec![*e])?;
                let output = output.expect("compile route validates output");
                super::native::compile(&module, output).map_err(|e| vec![*e])?;
                Ok(Summary::Compile(Some(output.to_string())))
            }
            Operation::Run => verified
                .run()
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
        let module = native_module(&verified, EntryPolicy::Process).map_err(|e| vec![*e])?;
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

#[cfg(test)]
mod build_reporting_tests {
    use super::*;
    #[test]
    fn build_reporting_success_uses_final_output_and_one_summary() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            write_build_report(
                &SourceMap::new(),
                &[],
                true,
                EntryPolicy::Process,
                Some(".oxid/typed-preview/process/app"),
                &mut out,
                &mut err
            ),
            0
        );
        assert!(err.is_empty());
        assert_eq!(String::from_utf8(out).unwrap(), "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"build-summary\",\"success\":true,\"errors\":0,\"output\":\".oxid/typed-preview/process/app\",\"entry_mode\":\"process\"}\n");
        let mut out = Vec::new();
        assert_eq!(
            write_build_report(
                &SourceMap::new(),
                &[],
                false,
                EntryPolicy::Result,
                Some("output"),
                &mut out,
                &mut err
            ),
            0
        );
        assert_eq!(out, b"typed-preview native build ok: \"output\"\n");
    }
    #[test]
    fn build_reporting_failure_after_publication_does_not_claim_rollback() {
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut err = Vec::new();
        assert_eq!(
            write_build_report(
                &SourceMap::new(),
                &[],
                true,
                EntryPolicy::Result,
                Some("output"),
                &mut Broken,
                &mut err
            ),
            1
        );
        let text = String::from_utf8(err).unwrap();
        assert!(text.contains("already published"));
        assert!(text.contains("not restored"));
    }
    #[test]
    fn build_reporting_internal_failure_keeps_exit_two_and_null_output() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let diagnostic = *Diagnostic::new("E0500", "oir-verify", "internal compiler error", None);
        assert_eq!(
            write_build_report(
                &SourceMap::new(),
                &[diagnostic],
                true,
                EntryPolicy::Result,
                None,
                &mut out,
                &mut err
            ),
            2
        );
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("\"output\":null"));
    }
}

#[cfg(test)]
mod native_project_tests {
    use super::*;
    #[test]
    fn native_project_session_retains_one_manifest_and_source_snapshot() {
        let root = std::env::temp_dir().join(format!("oxid-native-session-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("oxid.toml");
        std::fs::write(
            &path,
            "[project]\nedition=\"typed-preview\"\nentry=\"main.ox\"\nname=\"app\"\n",
        )
        .unwrap();
        std::fs::write(root.join("main.ox"), "fn main()->i32{return true;}").unwrap();
        let args = vec![
            "run".into(),
            format!("--manifest-path={}", path.display()),
            "--engine=native".into(),
        ];
        let Route::ManifestNativeRun {
            request,
            entry_policy,
        } = options::route(&args)
        else {
            panic!("native route");
        };
        let session = CompilerSession::load(
            request,
            Configuration::new(
                crate::frontend::manifest::Command::Run,
                Engine::Native,
                entry_policy,
                false,
            ),
        )
        .unwrap_or_else(|_| panic!("load snapshot"));
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(root.join("main.ox")).unwrap();
        let diagnostics = session.build().unwrap_err();
        assert!(diagnostics.iter().any(|d| d.code == "E0300"));
        assert!(diagnostics.iter().any(|d| d
            .render_human(session.project().sources())
            .contains("main.ox")));
        assert_eq!(session.project().graph().unwrap().packages[0].name, "app");
        assert_eq!(session.parent(), format!("{}/", root.display()));
        assert!(!root.join(".oxid").exists());
        std::fs::remove_dir(root).unwrap();
    }
}
