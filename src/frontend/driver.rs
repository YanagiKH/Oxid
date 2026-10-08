use super::{
    declaration_index::{IndexLimits, WorkMeter},
    diagnostic::{json_string, Diagnostic},
    oir,
    options::{self, EntryPolicy, Operation, Route},
    project::{budget::Allocator, ProjectLimits, ProjectSources, SyntaxFlavor},
    source::SourceMap,
};

/// None returns an untouched/default or explicitly legacy command to the old CLI.
pub fn dispatch(args: &mut Vec<String>) -> Option<i32> {
    dispatch_route(options::route(args), args)
}

fn dispatch_route(route: Route, args: &mut Vec<String>) -> Option<i32> {
    match route {
        Route::TypedImport { request } => Some(process_import(&request[0])),
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
    format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":{},\"errors\":{errors},\"result\":{}}}",
        errors == 0, result.filter(|_| errors == 0).map_or("null".into(), oir::Scalar::json))
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
    let project = match ProjectSources::load_typed(path, ProjectLimits::default()) {
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
    let executable = match project.syntax_flavor() {
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
            oir::project::check_project_executable(&project, limits, &work, &mut allocator)
        }
    };
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
                let module = verified.native_module().map_err(|e| vec![*e])?;
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
