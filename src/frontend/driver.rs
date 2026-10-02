use super::{
    diagnostic::{json_string, Diagnostic},
    oir,
    options::{self, Operation, Route},
    project::{ProjectLimits, ProjectSources},
    source::SourceMap,
};

/// None returns an untouched/default or explicitly legacy command to the old CLI.
pub fn dispatch(args: &mut Vec<String>) -> Option<i32> {
    match options::route(args) {
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
        Route::TypedCheck { path, json } => Some(process_file(&path, json, Operation::Check, None)),
        Route::TypedRun { path, json } => Some(process_file(&path, json, Operation::Run, None)),
        Route::TypedCompile { path, json, output } => {
            Some(process_file(&path, json, Operation::Compile, Some(&output)))
        }
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

fn process_file(path: &str, json: bool, operation: Operation, output: Option<&str>) -> i32 {
    let project = match ProjectSources::load_original(path, ProjectLimits::default()) {
        Ok(project) => project,
        Err(failure) => {
            return report(
                &failure.sources,
                failure.diagnostics,
                json,
                Summary::empty(operation),
            )
        }
    };
    let sources = project.sources();
    let (source, ast) = project
        .original_file()
        .expect("public source facade remains one-file");
    debug_assert!(!ast.tokens.is_empty());
    process_loaded(
        &project,
        json,
        operation,
        output,
        oir::check_source(source, ast, sources),
    )
}

/// Shared post-load operation path; private candidate adapters supply a checked
/// project result, while public dispatch continues to load only original syntax.
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

#[cfg(test)]
mod tests {
    use super::*;
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
