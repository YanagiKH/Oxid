//! Explicit, text-only project suites. Complete preparation precedes execution.
use super::{
    declaration_index::{IndexLimits, WorkMeter},
    diagnostic::Diagnostic,
    manifest,
    oir::{self, CheckedProjectTests},
    project::{budget::Allocator, ProjectLimits, ProjectSources},
    project_build::{self, Artifact},
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
    let (entry, parent) = match manifest::test(request) {
        Ok(selection) => selection,
        Err(failure) => return cli_error(failure, false),
    };
    let project = match ProjectSources::load_typed(&entry, ProjectLimits::default()) {
        Ok(project) => project,
        Err(failure) => return report_errors(&failure.sources, &failure.diagnostics),
    };
    let limits = IndexLimits::default();
    let suite = match oir::check_project_tests(
        &project,
        limits,
        &WorkMeter::new(limits.work),
        &mut Allocator::default(),
    ) {
        Ok(suite) => suite,
        Err(errors) => return report_errors(project.sources(), &errors),
    };
    let artifacts = if native {
        match prepare_native(&suite, &parent) {
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
