//! Explicit, text-only project suites. Complete preparation precedes execution.
use super::{
    diagnostic::Diagnostic,
    manifest,
    oir::CheckedProjectTests,
    options::EntryPolicy,
    project_build::{self, Artifact},
    session::{CompilerSession, Configuration, Engine},
    source::SourceMap,
};
use std::io::Write;

fn manifest_error(failure: manifest::Failure) -> Diagnostic {
    let (code, stage) = failure.classification();
    *Diagnostic::new(code, stage, failure.message(), None)
}

pub(super) fn cli_error(failure: manifest::Failure, json: bool) -> i32 {
    let diagnostic = manifest_error(failure);
    if json {
        // Diagnostic protocol only: there is no JSON test summary in this slice.
        let _ = writeln!(
            std::io::stdout().lock(),
            "{}",
            diagnostic.render_json(&SourceMap::new())
        );
    } else {
        report_errors(&SourceMap::new(), &[diagnostic]);
    }
    1
}

fn report_errors(sources: &SourceMap, errors: &[Diagnostic]) -> i32 {
    let stderr = &mut std::io::stderr().lock();
    for error in errors {
        let _ = write!(stderr, "{}", error.render_human(sources));
    }
    1
}

/// The vector retains exact executable inodes, even if another invocation later
/// republishes the same ordinal. Compilation is silent and never runs a case.
fn prepare_native(
    suite: &CheckedProjectTests<'_>,
    parent: &str,
    filter: Option<&str>,
    selected_count: usize,
) -> Result<Vec<Artifact>, Box<Diagnostic>> {
    let mut artifacts = Vec::new();
    artifacts.try_reserve_exact(selected_count).map_err(|_| {
        Diagnostic::new(
            "E0400",
            "project-test",
            "test artifact handles allocation failed",
            None,
        )
    })?;
    for ordinal in selected_ordinals(suite, filter) {
        let module = suite.native_module(ordinal)?;
        artifacts.push(project_build::publish_test(&module, parent, ordinal)?);
    }
    Ok(artifacts)
}

// Borrow the validated root suite and argv. No names or ordinal vector are copied.
fn selected_ordinals<'a>(
    suite: &'a CheckedProjectTests<'a>,
    filter: Option<&'a str>,
) -> impl Iterator<Item = usize> + 'a {
    (0..suite.len()).filter(move |&ordinal| {
        filter.is_none_or(|text| suite.name(ordinal).expect("checked ordinal").contains(text))
    })
}

pub(super) fn run(request: Box<[manifest::Request; 1]>, native: bool, filter: Option<&str>) -> i32 {
    let configuration = Configuration::new(
        manifest::Command::Test,
        if native {
            Engine::Native
        } else {
            Engine::Interpreter
        },
        EntryPolicy::Result,
        false,
    );
    let session = match CompilerSession::load(request, configuration) {
        Ok(session) => session,
        Err((sources, diagnostics)) => return report_errors(&sources, &diagnostics),
    };
    let project = session.project();
    let suite = match session.tests() {
        Ok(suite) => suite,
        Err(errors) => return report_errors(project.sources(), &errors),
    };
    // Loading checks the whole graph; tests() validates every discovered signature.
    // Selection is deliberately later, including the zero-match error.
    let selected_count = selected_ordinals(&suite, filter).count();
    if filter.is_some() && selected_count == 0 {
        return report_errors(
            project.sources(),
            &[*Diagnostic::new(
                "E0800",
                "project-test",
                "no tests match --filter",
                None,
            )],
        );
    }
    let artifacts = if native {
        match prepare_native(&suite, session.parent(), filter, selected_count) {
            Ok(artifacts) => Some(artifacts),
            Err(error) => return report_errors(project.sources(), &[*error]),
        }
    } else {
        None
    };
    let mut passed = 0;
    let stdout = &mut std::io::stdout().lock();
    // Artifact slots follow selection order; names retain original suite ordinals.
    for (selected_index, ordinal) in selected_ordinals(&suite, filter).enumerate() {
        let success = if let Some(artifacts) = &artifacts {
            match artifacts[selected_index].run_test() {
                Ok(success) => success,
                // Launch/wait errors are infrastructure failures, not outcomes.
                Err(error) => return report_errors(project.sources(), &[*error]),
            }
        } else {
            match suite.run(ordinal) {
                Ok(success) => success,
                Err(error) => {
                    report_errors(project.sources(), &[*error]);
                    false
                }
            }
        };
        passed += usize::from(success);
        if writeln!(
            stdout,
            "{} {}",
            if success { "PASS" } else { "FAIL" },
            suite.name(ordinal).expect("checked ordinal")
        )
        .and_then(|_| stdout.flush())
        .is_err()
        {
            return 1;
        }
    }
    let failed = selected_count - passed;
    if writeln!(
        stdout,
        "test result: {passed} passed; {failed} failed; {} total",
        selected_count
    )
    .and_then(|_| stdout.flush())
    .is_err()
    {
        return 1;
    }
    i32::from(selected_count == 0 || failed != 0)
}
