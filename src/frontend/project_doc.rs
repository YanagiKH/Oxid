//! Checked, selected-package API documentation, emitted only to stdout.
//! This consumer reuses the ordinary session/checker and retained syntax. It
//! never executes source, invokes native tools, writes files or expands limits.
use super::{
    diagnostic::Diagnostic,
    manifest::{Command, Failure, Request},
    options::EntryPolicy,
    session::{CompilerSession, Configuration, Engine},
    source::SourceMap,
};
use std::io::{self, Write};

mod comments;
mod render;

fn report(sources: &SourceMap, diagnostics: &[Diagnostic], stderr: &mut impl Write) {
    for diagnostic in diagnostics {
        let _ = write!(stderr, "{}", diagnostic.render_human(sources));
    }
    let _ = stderr.flush();
}

pub(super) fn cli_error(failure: Failure) -> i32 {
    let (code, stage) = failure.classification();
    report(
        &SourceMap::new(),
        &[*Diagnostic::new(code, stage, failure.message(), None)],
        &mut io::stderr().lock(),
    );
    1
}

pub(super) fn run(request: Box<[Request; 1]>) -> i32 {
    run_with_writers(request, &mut io::stdout().lock(), &mut io::stderr().lock())
}

fn run_with_writers(
    request: Box<[Request; 1]>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let session = match CompilerSession::load(
        request,
        Configuration::new(
            Command::Check,
            Engine::Interpreter,
            EntryPolicy::Result,
            false,
        ),
    ) {
        Ok(session) => session,
        Err((sources, diagnostics)) => {
            report(&sources, &diagnostics, stderr);
            return 1;
        }
    };
    // Retain only the existing immutable project after the ordinary checker
    // succeeds. Do not create an entry, execute it or bypass dependency bodies.
    if let Err(diagnostics) = session.check() {
        report(session.project().sources(), &diagnostics, stderr);
        return 1;
    }
    // Walk the same renderer into a sink first. Source/AST invariants must fail
    // before the first stdout byte; stdout itself is explicitly not atomic.
    if render::document(session.project(), &mut io::sink()).is_err() {
        report(
            session.project().sources(),
            &[*Diagnostic::new(
                "E0002",
                "project-doc",
                "checked API documentation could not be prepared",
                None,
            )],
            stderr,
        );
        return 1;
    }
    if render::document(session.project(), stdout)
        .and_then(|()| stdout.flush())
        .is_err()
    {
        report(
            session.project().sources(),
            &[*Diagnostic::new(
                "E0002",
                "project-doc",
                "checked API documentation could not be written; stdout may contain a partial document",
                None,
            )],
            stderr,
        );
        return 1;
    }
    0
}
