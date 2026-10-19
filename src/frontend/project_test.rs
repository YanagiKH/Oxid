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
) -> Result<Vec<Artifact>, Box<Diagnostic>> {
    let mut artifacts = Vec::new();
    artifacts.try_reserve_exact(suite.len()).map_err(|_| {
        Diagnostic::new(
            "E0400",
            "project-test",
            "test artifact handles allocation failed",
            None,
        )
    })?;
    for ordinal in 0..suite.len() {
        let module = suite.native_module(ordinal)?;
        artifacts.push(project_build::publish_test(&module, parent, ordinal)?);
    }
    Ok(artifacts)
}

pub(super) fn run(request: Box<[manifest::Request; 1]>, native: bool) -> i32 {
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
    let artifacts = if native {
        match prepare_native(&suite, session.parent()) {
            Ok(artifacts) => Some(artifacts),
            Err(error) => return report_errors(project.sources(), &[*error]),
        }
    } else {
        None
    };
    let mut passed = 0;
    let stdout = &mut std::io::stdout().lock();
    for ordinal in 0..suite.len() {
        let success = if let Some(artifacts) = &artifacts {
            match artifacts[ordinal].run_test() {
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
    let failed = suite.len() - passed;
    if writeln!(
        stdout,
        "test result: {passed} passed; {failed} failed; {} total",
        suite.len()
    )
    .and_then(|_| stdout.flush())
    .is_err()
    {
        return 1;
    }
    i32::from(suite.len() == 0 || failed != 0)
}
