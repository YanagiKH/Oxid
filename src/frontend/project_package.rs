//! Explicit project packaging: one session, silent managed build, then archive.
use super::{
    diagnostic::{json_string, Diagnostic},
    driver::exit_status,
    manifest::{Command, Failure, Request},
    native_bundle::{self, BundleEntry},
    options::{BuildProfile, EntryPolicy},
    project_build::{self, Artifact},
    session::{CompilerSession, Configuration, Engine},
    source::SourceMap,
};
use std::{fs, io::Write, path::Path};

fn failure(message: impl Into<String>) -> Box<Diagnostic> {
    Diagnostic::new("E0720", "project-package", message, None)
}

/// Obvious collisions fail before compilation; create's hard link is the final
/// no-replacement authority, including when a name appears after this check.
fn preflight(output: &Path) -> Result<(), Box<Diagnostic>> {
    match fs::symlink_metadata(output) {
        Ok(_) => {
            return Err(failure(
                "package output already exists; replacement is forbidden",
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(failure(format!("inspect package output: {e}"))),
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let metadata =
        fs::metadata(parent).map_err(|e| failure(format!("inspect package output parent: {e}")))?;
    if !metadata.is_dir() || output.file_name().is_none() {
        return Err(failure(
            "package output requires a filename and an existing parent directory",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Report<'a> {
    json: bool,
    entry_policy: EntryPolicy,
    // These are facts about this invocation, independent of overall success.
    managed: Option<&'a str>,
    archive: Option<&'a str>,
}
impl Report<'_> {
    fn empty(json: bool, entry_policy: EntryPolicy) -> Self {
        Self {
            json,
            entry_policy,
            managed: None,
            archive: None,
        }
    }
}

pub(super) fn cli_error(failure: Failure, json: bool, entry_policy: EntryPolicy) -> i32 {
    let (code, stage) = failure.classification();
    report(
        &SourceMap::new(),
        &[*Diagnostic::new(code, stage, failure.message(), None)],
        Report::empty(json, entry_policy),
    )
}

pub(super) fn run(
    request: Box<[Request; 1]>,
    output: &str,
    entry_policy: EntryPolicy,
    profile: BuildProfile,
) -> i32 {
    let empty = Report::empty(request[0].json, entry_policy);
    if let Err(diagnostic) = preflight(Path::new(output)) {
        return report(&SourceMap::new(), &[*diagnostic], empty);
    }
    let session = match CompilerSession::load(
        request,
        Configuration::new(Command::Package, Engine::Native, entry_policy, empty.json)
            .with_profile(profile),
    ) {
        Ok(session) => session,
        Err((sources, diagnostics)) => return report(&sources, &diagnostics, empty),
    };
    match session.build() {
        Ok(artifact) => finish(
            &session,
            &artifact,
            output,
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        ),
        Err(diagnostics) => report(session.project().sources(), &diagnostics, empty),
    }
}

// A narrow post-build boundary also lets ordinary tests use the real managed
// publication protocol with synthetic ELF bytes, without pretending to run LLVM.
pub(super) fn finish(
    session: &CompilerSession,
    artifact: &Artifact,
    output: &str,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let config = session.configuration();
    let entry = match config.entry_policy {
        EntryPolicy::Result => BundleEntry::Result,
        EntryPolicy::Process => BundleEntry::Process,
    };
    let result = artifact.package_bytes().and_then(|bytes| {
        native_bundle::create(
            Path::new(output),
            session.selected_name(),
            entry,
            &bytes,
            artifact.profile(),
        )
    });
    let state = Report {
        json: config.json,
        entry_policy: config.entry_policy,
        managed: Some(&artifact.path),
        archive: result.is_ok().then_some(output),
    };
    let diagnostics = result
        .err()
        .map(|error| *failure(error))
        .into_iter()
        .collect::<Vec<_>>();
    write_report(
        session.project().sources(),
        &diagnostics,
        state,
        stdout,
        stderr,
    )
}

fn report(sources: &SourceMap, diagnostics: &[Diagnostic], state: Report<'_>) -> i32 {
    write_report(
        sources,
        diagnostics,
        state,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    )
}
fn write_report(
    sources: &SourceMap,
    diagnostics: &[Diagnostic],
    state: Report<'_>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let written = (|| -> std::io::Result<()> {
        for diagnostic in diagnostics {
            if state.json {
                writeln!(stdout, "{}", diagnostic.render_json(sources))?;
            } else {
                write!(stderr, "{}", diagnostic.render_human(sources))?;
            }
        }
        if state.json {
            writeln!(stdout, "{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"package-summary\",\"success\":{},\"errors\":{},\"entry_mode\":{},\"managed_published\":{},\"managed_output\":{},\"archive_published\":{},\"archive_output\":{}}}", diagnostics.is_empty(), diagnostics.len(), json_string(project_build::mode(state.entry_policy)), state.managed.is_some(), state.managed.map_or("null".into(), json_string), state.archive.is_some(), state.archive.map_or("null".into(), json_string))?;
        } else {
            writeln!(
                stdout,
                "typed-preview native package {}: managed executable {}; archive {}",
                if diagnostics.is_empty() {
                    "ok"
                } else {
                    "failed"
                },
                state.managed.map_or("not published".into(), json_string),
                state.archive.map_or("not published".into(), json_string)
            )?;
        }
        stdout.flush()?;
        stderr.flush()
    })();
    if let Err(error) = written {
        let diagnostic = Diagnostic::new("E0721", "package-report", format!("package report failed: {error}; managed executable {}; archive {}; published files were retained", state.managed.map_or("not published".into(), json_string), state.archive.map_or("not published".into(), json_string)), None);
        let _ = write!(stderr, "{}", diagnostic.render_human(sources));
        return 1;
    }
    exit_status(diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_summary_preserves_invariant_exit_and_failed_phase_facts() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let diagnostics = [*Diagnostic::new(
            "E0500",
            "manifest-input",
            "invariant",
            None,
        )];
        assert_eq!(
            write_report(
                &SourceMap::new(),
                &diagnostics,
                Report::empty(true, EntryPolicy::Process),
                &mut out,
                &mut err
            ),
            2
        );
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.matches("package-summary").count(), 1);
        assert!(text.contains("\"entry_mode\":\"process\""));
        assert!(text.contains("\"managed_published\":false"));
        assert!(text.contains("\"archive_published\":false"));
        assert!(err.is_empty());
    }
}
