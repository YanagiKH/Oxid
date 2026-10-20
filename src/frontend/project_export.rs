//! Checked, normalized compiler-source export. No execution or native tools.
//! Source bytes come only from the immutable session, never reopened paths.
use super::{
    diagnostic::{json_string, Diagnostic},
    manifest::{Command, Failure, Request},
    options::EntryPolicy,
    session::{CompilerSession, Configuration, Engine},
    source::SourceMap,
};
use std::{
    borrow::Cow,
    io::{self, Write},
    path::Path,
};

mod files;
mod plan;

pub(super) struct Entry<'a> {
    pub relative_path: String,
    pub bytes: Cow<'a, [u8]>,
}

/// Preserve JSON-style path quoting while also escaping DEL/C1 for terminals.
/// Stream the extra escaping without retaining a second escaped path buffer.
struct QuotedPath<'a>(&'a str);

impl std::fmt::Display for QuotedPath<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write as _;

        for character in json_string(self.0).chars() {
            if character.is_control() {
                write!(out, "\\u{:04x}", u32::from(character))?;
            } else {
                out.write_char(character)?;
            }
        }
        Ok(())
    }
}

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

pub(super) fn run(request: Box<[Request; 1]>, output: &str) -> i32 {
    run_with_writers(
        request,
        output,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}

fn run_with_writers(
    request: Box<[Request; 1]>,
    output: &str,
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
    if let Err(diagnostics) = session.check() {
        report(session.project().sources(), &diagnostics, stderr);
        return 1;
    }
    let entries = match plan::build(session.project()) {
        Ok(entries) => entries,
        Err(message) => {
            report(
                session.project().sources(),
                &[*Diagnostic::new("E0002", "project-export", message, None)],
                stderr,
            );
            return 1;
        }
    };
    if let Err(failure) = files::write_new(Path::new(output), &entries) {
        let state = if failure.destination_created {
            "a partial destination may remain; it was created by this invocation and was retained"
        } else {
            "the destination was not created by this invocation"
        };
        report(
            session.project().sources(),
            &[*Diagnostic::new(
                "E0002",
                "project-export",
                format!("{}; {state}", failure.message),
                None,
            )],
            stderr,
        );
        return 1;
    }
    if writeln!(
        stdout,
        "Exported checked compiler sources to {}",
        QuotedPath(output)
    )
    .and_then(|()| stdout.flush())
    .is_err()
    {
        report(session.project().sources(), &[*Diagnostic::new(
            "E0002", "project-export", format!(
                "export completed at {} but status output failed; the complete destination was retained",
                QuotedPath(output),
            ), None,
        )], stderr);
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::QuotedPath;

    #[test]
    fn status_path_is_quoted_without_terminal_control_characters() {
        assert_eq!(format!("{}", QuotedPath("demo")), "\"demo\"");
        assert_eq!(
            format!(
                "{}",
                QuotedPath("parent \"quoted\"\\\n\t\u{7f}\u{80}\u{9b}/demo")
            ),
            "\"parent \\\"quoted\\\"\\\\\\n\\t\\u007f\\u0080\\u009b/demo\"",
        );
        let controls = (0..=0x9f)
            .filter_map(char::from_u32)
            .filter(|character| character.is_control())
            .collect::<String>();
        let displayed = format!("{}", QuotedPath(&format!("{controls}/demo")));
        assert!(!displayed.chars().any(char::is_control));
        assert!(displayed.starts_with('"'));
        assert!(displayed.ends_with("/demo\""));
        assert_eq!(format!("{}", QuotedPath("親/demo")), "\"親/demo\"");
    }
}
