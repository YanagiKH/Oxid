//! Manifest-selected, syntax-only project formatting.
//!
//! Selection and source loading use the unchanged compiler session. Formatting
//! never typechecks, executes, builds, or modifies dependencies. Project writes
//! are explicit and atomic per file, not across the selected package.
use super::{
    diagnostic::Diagnostic,
    manifest::{Command, Failure, Request},
    options::EntryPolicy,
    project::{ModuleHeader, ProjectSources},
    session::{CompilerSession, Configuration, Engine},
    source::SourceMap,
};
use std::{io::Write, path::Path};

#[path = "project_format_files.rs"]
mod files;

fn error(message: impl Into<String>) -> Box<Diagnostic> {
    Diagnostic::new("E0002", "project-format", message, None)
}

/// Terminal display, not JSON: JSON may legally retain DEL and C1 controls.
fn display_path(path: &str) -> String {
    let mut result = String::from("\"");
    for character in path.chars() {
        if character.is_control() || matches!(character, '"' | '\\') {
            result.extend(character.escape_default());
        } else {
            result.push(character);
        }
    }
    result.push('"');
    result
}

pub(super) fn cli_error(failure: Failure) -> i32 {
    let (code, stage) = failure.classification();
    report(
        &SourceMap::new(),
        &[*Diagnostic::new(code, stage, failure.message(), None)],
        &mut std::io::stderr().lock(),
    );
    2
}

pub(super) fn run(request: Box<[Request; 1]>, write: bool) -> i32 {
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
            report(&sources, &diagnostics, &mut std::io::stderr().lock());
            return 2;
        }
    };
    process(
        session.project(),
        write,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    )
}

fn selected_modules(project: &ProjectSources) -> Result<&[ModuleHeader], Box<Diagnostic>> {
    let package = project
        .graph()
        .and_then(|graph| graph.packages.first())
        .ok_or_else(|| error("formatter requires an explicitly selected project"))?;
    project
        .modules()
        .get(package.root.0..package.end)
        .filter(|modules| !modules.is_empty())
        .ok_or_else(|| error("selected formatter module interval is invalid"))
}

/// This additional write-only check must not relax the existing loader. Shared
/// source paths between packages cannot be edited while promising dependencies
/// remain untouched, even though each package has its own nominal identity.
fn preflight_writes(
    project: &ProjectSources,
    selected: &[ModuleHeader],
) -> Result<(), Box<Diagnostic>> {
    for module in selected {
        let source = project.sources().get(module.file);
        let path = Path::new(source.path());
        files::verify_original(path, source.text().as_bytes()).map_err(|cause| {
            error(format!(
                "cannot format {}: {cause}",
                display_path(source.path())
            ))
        })?;
        let canonical = path.canonicalize().map_err(|cause| {
            error(format!(
                "cannot inspect {}: {cause}",
                display_path(source.path())
            ))
        })?;
        for other in project.modules() {
            if selected.iter().any(|selected| selected.file == other.file) {
                continue;
            }
            let other_source = project.sources().get(other.file);
            let other_canonical =
                Path::new(other_source.path())
                    .canonicalize()
                    .map_err(|cause| {
                        error(format!(
                            "cannot inspect dependency source {}: {cause}",
                            display_path(other_source.path())
                        ))
                    })?;
            if canonical == other_canonical {
                return Err(error(format!(
                    "refusing to write {}: the same source path also belongs to a dependency",
                    display_path(source.path())
                )));
            }
        }
    }
    Ok(())
}

fn process(
    project: &ProjectSources,
    write: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let sources = project.sources();
    let selected = match selected_modules(project) {
        Ok(selected) => selected,
        Err(diagnostic) => {
            report(sources, &[*diagnostic], stderr);
            return 2;
        }
    };
    if write {
        if let Err(diagnostic) = preflight_writes(project, selected) {
            report(sources, &[*diagnostic], stderr);
            return 2;
        }
    }
    let mut changed = Vec::new();
    if changed.try_reserve_exact(selected.len()).is_err() {
        report(
            sources,
            &[*Diagnostic::new(
                "E0400",
                "project-format",
                "cannot allocate formatter result storage",
                None,
            )],
            stderr,
        );
        return 2;
    }
    let mut staged = Vec::new();
    if write && staged.try_reserve_exact(selected.len()).is_err() {
        report(
            sources,
            &[*Diagnostic::new(
                "E0400",
                "project-format",
                "cannot allocate formatter staging storage",
                None,
            )],
            stderr,
        );
        return 2;
    }
    // Every candidate is syntax-validated before any original is replaced.
    // Candidates are staged and dropped one at a time, not retained as a second
    // source forest in memory. Both vectors hold at most the selected module count.
    for module in selected {
        let source = sources.get(module.file);
        let candidate = match super::format::format_source(source) {
            Ok(candidate) => candidate,
            Err(diagnostics) => {
                report(sources, &diagnostics, stderr);
                return 2;
            }
        };
        if candidate.as_bytes() == source.text().as_bytes() {
            continue;
        }
        if write {
            let stage = match files::StagedFile::prepare(
                Path::new(source.path()),
                source.text().as_bytes(),
                candidate.as_bytes(),
            ) {
                Ok(stage) => stage,
                Err(cause) => {
                    report(
                        sources,
                        &[*error(format!(
                            "cannot stage {}: {cause}; no source files were replaced",
                            display_path(source.path())
                        ))],
                        stderr,
                    );
                    return 2;
                }
            };
            staged.push(stage);
        }
        changed.push(source.path());
    }
    if !write {
        let written = (|| -> std::io::Result<()> {
            for path in &changed {
                writeln!(stderr, "formatting required: {}", display_path(path))?;
            }
            writeln!(
                stdout,
                "typed-preview project fmt: {} files checked; {} require formatting",
                selected.len(),
                changed.len()
            )?;
            stdout.flush()?;
            stderr.flush()
        })();
        if let Err(cause) = written {
            report(
                sources,
                &[*error(format!("formatter report failed: {cause}"))],
                stderr,
            );
            return 2;
        }
        return i32::from(!changed.is_empty());
    }
    // Recheck clean selected files too: a later editor change must not be
    // silently ignored after the initial snapshot and formatter pass.
    if let Err(diagnostic) = preflight_writes(project, selected) {
        report(sources, &[*diagnostic], stderr);
        return 2;
    }
    for stage in &staged {
        if let Err(cause) = stage.verify() {
            report(
                sources,
                &[*error(format!(
                    "formatter precommit check failed: {cause}; no source files were replaced"
                ))],
                stderr,
            );
            return 2;
        }
    }
    let mut written_count = 0;
    for (stage, path) in staged.iter().zip(&changed) {
        if let Err(cause) = stage.commit() {
            report(
                sources,
                &[*error(format!(
                    "cannot replace {}: {cause}; {written_count} files already formatted; remaining files were not replaced",
                    display_path(path)
                ))],
                stderr,
            );
            return 2;
        }
        written_count += 1;
    }
    let result = (|| -> std::io::Result<()> {
        for path in &changed {
            writeln!(stdout, "formatted: {}", display_path(path))?;
        }
        writeln!(
            stdout,
            "typed-preview project fmt: {} files checked; {written_count} formatted",
            selected.len()
        )?;
        stdout.flush()
    })();
    if let Err(cause) = result {
        report(
            sources,
            &[*error(format!(
                "formatter report failed: {cause}; {written_count} files were formatted and retained"
            ))],
            stderr,
        );
        return 2;
    }
    0
}

fn report(sources: &SourceMap, diagnostics: &[Diagnostic], stderr: &mut impl Write) {
    for diagnostic in diagnostics {
        let _ = write!(stderr, "{}", diagnostic.render_human(sources));
    }
    let _ = stderr.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displayed_paths_escape_quotes_backslashes_and_every_control_character() {
        assert_eq!(display_path("a\u{7f}\u{9b}b"), "\"a\\u{7f}\\u{9b}b\"");
        assert_eq!(display_path("a\"\\\nb"), "\"a\\\"\\\\\\nb\"");
        let rendered = display_path("é\u{1b}[31m\u{85}\u{9b}Z");
        assert!(!rendered.chars().any(char::is_control));
        assert!(rendered.contains('é'));
    }

    #[test]
    fn project_without_manifest_selection_is_rejected_without_output_or_writes() {
        let project = ProjectSources::from_u8_index_test_files(&[(
            "main.ox",
            "fn main() -> i32 { return 0; }",
        )]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(process(&project, false, &mut stdout, &mut stderr), 2);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("explicitly selected project"));
    }
}
