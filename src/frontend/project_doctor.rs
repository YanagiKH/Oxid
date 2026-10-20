//! Explicit project configuration reporting, not source or application health.
//!
//! Reuse the manifest graph used by the compiler session without opening source
//! files, typechecking, building the application, or changing its artifacts.
//! Only an explicit native request invokes the existing toolchain-only doctor.
use super::{
    diagnostic::Diagnostic,
    manifest::{self, Command, Failure, Request},
    project::forest::PackageGraph,
    source::SourceMap,
};
use std::{
    fmt::{self, Write as _},
    io::{self, Write},
};

/// Quote terminal-facing manifest text without retaining an escaped copy.
/// JSON escaping alone would leave DEL and C1 terminal controls unescaped.
struct Quoted<'a>(&'a str);

impl fmt::Display for Quoted<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_char('"')?;
        for character in self.0.chars() {
            if character.is_control() || matches!(character, '"' | '\\') {
                for escaped in character.escape_default() {
                    out.write_char(escaped)?;
                }
            } else {
                out.write_char(character)?;
            }
        }
        out.write_char('"')
    }
}

fn report_failure(failure: Failure, stderr: &mut impl Write) {
    let (code, stage) = failure.classification();
    report_diagnostic(
        &Diagnostic::new(code, stage, failure.message(), None),
        stderr,
    );
}

fn report_diagnostic(diagnostic: &Diagnostic, stderr: &mut impl Write) {
    let _ = stderr.write_all(diagnostic.render_human(&SourceMap::new()).as_bytes());
}

pub(super) fn cli_error(failure: Failure) -> i32 {
    report_failure(failure, &mut io::stderr().lock());
    1
}

pub(super) fn run(request: Box<[Request; 1]>, native: bool) -> i32 {
    // Do not hold a stdout/stderr lock when the existing native doctor takes
    // its own locks. Configuration output is flushed before it is invoked.
    run_with_probe(
        request,
        native,
        &mut io::stdout(),
        &mut io::stderr(),
        super::toolchain_doctor::run,
    )
}

/// The injected callback tests ordering and selection only. A test callback is
/// never evidence that LLVM, linking, or execution succeeded.
fn run_with_probe(
    request: Box<[Request; 1]>,
    native: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    probe: impl FnOnce() -> i32,
) -> i32 {
    // The same requirements as manifest CHECK, including optional direct
    // project names. Probing the toolchain does not build the application and
    // must not impose native BUILD's extra name requirement.
    let selection = match manifest::graph::resolve(request, Command::Check, false) {
        Ok(selection) => selection,
        Err(failure) => {
            report_failure(failure, stderr);
            return 1;
        }
    };
    if write_configuration(&selection.graph, native, stdout)
        .and_then(|()| stdout.flush())
        .is_err()
    {
        report_diagnostic(
            &Diagnostic::new(
                "E0002",
                "project-doctor",
                "project configuration resolved, but its report could not be written",
                None,
            ),
            stderr,
        );
        return 1;
    }
    if native {
        probe()
    } else {
        0
    }
}

fn write_configuration(
    graph: &PackageGraph,
    native: bool,
    stdout: &mut impl Write,
) -> io::Result<()> {
    let selected = graph.packages.first().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "project doctor requires a selected manifest package",
        )
    })?;
    writeln!(stdout, "typed project configuration resolved")?;
    writeln!(stdout, "edition: typed-preview")?;
    writeln!(stdout, "selected manifest: {}", Quoted(&selected.manifest))?;
    if selected.name.is_empty() {
        writeln!(stdout, "selected package: <unnamed>")?;
    } else {
        writeln!(stdout, "selected package: {}", Quoted(&selected.name))?;
    }
    writeln!(stdout, "entry: {}", Quoted(&selected.entry))?;
    writeln!(stdout, "packages: {}", graph.packages.len())?;
    for (index, package) in graph.packages.iter().enumerate() {
        write!(
            stdout,
            "package {index}: manifest={} name=",
            Quoted(&package.manifest),
        )?;
        if package.name.is_empty() {
            write!(stdout, "<unnamed>")?;
        } else {
            write!(stdout, "{}", Quoted(&package.name))?;
        }
        writeln!(stdout, " entry={}", Quoted(&package.entry))?;
        for dependency in &package.dependencies {
            writeln!(
                stdout,
                "dependency {index}: alias={} target={}",
                Quoted(&dependency.alias),
                dependency.target.0,
            )?;
        }
    }
    writeln!(stdout, "sources: not checked")?;
    if native {
        writeln!(stdout, "native probe: requested")
    } else {
        writeln!(stdout, "native probe: not requested (use --native)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::options::{self, Route};
    use std::{
        cell::Cell,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    const PROJECT: &str = "[project]\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n";

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "oxid-project-doctor-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn write(&self, relative: &str, text: &str) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        fn request(&self, relative: &str, package: Option<&str>) -> Box<[Request; 1]> {
            // Use the unchanged CHECK gate to construct a real admitted request.
            // Doctor CLI ownership is a separate module's responsibility.
            let mut args = vec![
                "check".to_string(),
                "--manifest-path".to_string(),
                self.0.join(relative).to_str().unwrap().to_string(),
            ];
            if let Some(package) = package {
                args.push("--package".to_string());
                args.push(package.to_string());
            }
            match options::route(&args) {
                Route::ManifestCheck { request } => request,
                other => panic!("expected admitted manifest CHECK request: {other:?}"),
            }
        }

        fn no_artifacts(&self) {
            for relative in [".oxid", "oxid.lock", "app/.oxid", "app/oxid.lock"] {
                assert!(!self.0.join(relative).exists(), "{relative}");
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn configuration(fixture: &Fixture, package: Option<&str>) -> (i32, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_with_probe(
            fixture.request("oxid.toml", package),
            false,
            &mut stdout,
            &mut stderr,
            || panic!("default doctor must not probe the native toolchain"),
        );
        (
            status,
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
        )
    }

    #[test]
    fn configuration_does_not_load_sources_require_a_name_or_probe() {
        let fixture = Fixture::new();
        fixture.write("oxid.toml", PROJECT);
        for invalid_source in [None, Some("this is not valid Oxid source")] {
            if let Some(text) = invalid_source {
                fixture.write("src/main.ox", text);
            }
            let (status, stdout, stderr) = configuration(&fixture, None);
            assert_eq!(status, 0, "{stderr}");
            assert!(stderr.is_empty());
            assert!(stdout.starts_with("typed project configuration resolved\n"));
            assert!(stdout.contains("selected package: <unnamed>\n"));
            assert!(stdout.contains("packages: 1\n"));
            assert!(stdout
                .ends_with("sources: not checked\nnative probe: not requested (use --native)\n"));
            fixture.no_artifacts();
        }
    }

    #[test]
    fn workspace_selection_reuses_the_real_dependency_graph() {
        let fixture = Fixture::new();
        fixture.write("oxid.toml", "[workspace]\nmembers=[\"app\",\"library\"]\n");
        fixture.write(
            "app/oxid.toml",
            &format!("{PROJECT}name=\"application\"\n[dependencies]\nlib=\"../library\"\n"),
        );
        fixture.write("library/oxid.toml", &format!("{PROJECT}name=\"library\"\n"));
        let (status, stdout, stderr) = configuration(&fixture, Some("application"));
        assert_eq!(status, 0, "{stderr}");
        assert!(stderr.is_empty());
        assert!(stdout.contains("selected package: \"application\"\n"));
        assert!(stdout.contains("packages: 2\n"));
        assert!(stdout.contains("dependency 0: alias=\"lib\" target=1\n"));
        let selected_manifest = fixture.0.join("app/oxid.toml");
        assert!(stdout.contains(&format!(
            "selected manifest: {}\n",
            Quoted(selected_manifest.to_str().unwrap()),
        )));
        assert!(!fixture.0.join("app/src/main.ox").exists());
        assert!(!fixture.0.join("library/src/main.ox").exists());
        fixture.no_artifacts();
    }

    #[test]
    fn invalid_unselected_workspace_manifest_fails_before_probe() {
        let fixture = Fixture::new();
        fixture.write("oxid.toml", "[workspace]\nmembers=[\"app\",\"library\"]\n");
        fixture.write("app/oxid.toml", &format!("{PROJECT}name=\"application\"\n"));
        fixture.write(
            "library/oxid.toml",
            "[project]\nname=\"library\"\nedition=\"legacy-0.9\"\nentry=\"main.ox\"\n",
        );
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_with_probe(
            fixture.request("oxid.toml", Some("application")),
            true,
            &mut stdout,
            &mut stderr,
            || panic!("an invalid manifest must prevent the requested probe"),
        );
        assert_eq!(status, 1);
        assert!(stdout.is_empty());
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("E0001"), "{stderr}");
        assert!(stderr.contains("requires [project].edition"), "{stderr}");
        fixture.no_artifacts();
    }

    #[test]
    fn missing_manifest_or_invalid_entry_does_not_probe() {
        let fixture = Fixture::new();
        for manifest in [
            None,
            Some("[project]\nedition=\"typed-preview\"\n"),
            Some("[project]\nedition=\"typed-preview\"\nentry=\"../escape.ox\"\n"),
        ] {
            if let Some(text) = manifest {
                fixture.write("oxid.toml", text);
            }
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let status = run_with_probe(
                fixture.request("oxid.toml", None),
                true,
                &mut stdout,
                &mut stderr,
                || panic!("manifest resolution must finish before any probe"),
            );
            assert_eq!(status, 1);
            assert!(stdout.is_empty());
            assert!(!stderr.is_empty());
            fixture.no_artifacts();
        }
    }

    #[test]
    fn failed_dependency_resolution_does_not_probe() {
        let fixture = Fixture::new();
        fixture.write(
            "oxid.toml",
            &format!("{PROJECT}[dependencies]\nmissing=\"missing\"\n"),
        );
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_with_probe(
            fixture.request("oxid.toml", None),
            true,
            &mut stdout,
            &mut stderr,
            || panic!("an unresolved dependency must prevent the probe"),
        );
        assert_eq!(status, 1);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr).unwrap().contains("E0002"));
        fixture.no_artifacts();
    }

    struct ObservedWriter<'a> {
        bytes: Vec<u8>,
        flushed: &'a Cell<bool>,
        fail_write: bool,
        fail_flush: bool,
    }

    impl Write for ObservedWriter<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_write {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "test writer"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "test flush"));
            }
            self.flushed.set(true);
            Ok(())
        }
    }

    #[test]
    fn explicit_probe_runs_once_after_report_flush_and_preserves_its_status() {
        let fixture = Fixture::new();
        fixture.write("oxid.toml", PROJECT);
        for probe_status in [0, 1] {
            let calls = Cell::new(0);
            let flushed = Cell::new(false);
            let mut stdout = ObservedWriter {
                bytes: Vec::new(),
                flushed: &flushed,
                fail_write: false,
                fail_flush: false,
            };
            let mut stderr = Vec::new();
            let status = run_with_probe(
                fixture.request("oxid.toml", None),
                true,
                &mut stdout,
                &mut stderr,
                || {
                    assert!(flushed.get(), "report must be flushed before probing");
                    calls.set(calls.get() + 1);
                    probe_status
                },
            );
            assert_eq!(status, probe_status);
            assert_eq!(calls.get(), 1);
            assert!(stderr.is_empty());
            let text = String::from_utf8(stdout.bytes).unwrap();
            assert!(text.contains("selected package: <unnamed>\n"));
            assert!(text.ends_with("sources: not checked\nnative probe: requested\n"));
            // The callback exercises orchestration, not LLVM readiness.
            assert!(!text.contains("toolchain ready"));
            fixture.no_artifacts();
        }
    }

    #[test]
    fn report_write_or_flush_failure_prevents_probe_and_returns_failure() {
        let fixture = Fixture::new();
        fixture.write("oxid.toml", PROJECT);
        for fail_write in [false, true] {
            let flushed = Cell::new(false);
            let mut stdout = ObservedWriter {
                bytes: Vec::new(),
                flushed: &flushed,
                fail_write,
                fail_flush: !fail_write,
            };
            let mut stderr = Vec::new();
            let status = run_with_probe(
                fixture.request("oxid.toml", None),
                true,
                &mut stdout,
                &mut stderr,
                || panic!("failed report output must prevent the probe"),
            );
            assert_eq!(status, 1);
            assert!(!flushed.get());
            let stderr = String::from_utf8(stderr).unwrap();
            assert!(stderr.contains("E0002"), "{stderr}");
            assert!(stderr.contains("project-doctor"), "{stderr}");
            assert!(stderr.contains("report could not be written"), "{stderr}");
            fixture.no_artifacts();
        }
    }

    #[test]
    fn quoted_text_escapes_controls_quotes_and_backslashes() {
        assert_eq!(
            format!("{}", Quoted("é\"\u{0}\t\n\r\u{1b}\u{7f}\u{85}\\")),
            "\"é\\\"\\u{0}\\t\\n\\r\\u{1b}\\u{7f}\\u{85}\\\\\"",
        );
    }

    #[test]
    fn decoded_project_name_is_terminal_safe_without_extra_name_restrictions() {
        let fixture = Fixture::new();
        fixture.write(
            "oxid.toml",
            &format!("{PROJECT}name=\"hello\\n\\u001b\\u007f\\u0085\\\"\\\\\"\n"),
        );
        let (status, stdout, stderr) = configuration(&fixture, None);
        assert_eq!(status, 0, "{stderr}");
        assert!(stderr.is_empty());
        assert!(stdout.contains("selected package: \"hello\\n\\u{1b}\\u{7f}\\u{85}\\\"\\\\\"\n"));
        assert!(stdout.chars().all(|c| c == '\n' || !c.is_control()));
    }

    #[test]
    fn failure_reporting_preserves_classification_and_escapes_controls() {
        let mut stderr = Vec::new();
        report_failure(Failure::Cli("bad\n\u{1b}argument".into()), &mut stderr);
        let text = String::from_utf8(stderr).unwrap();
        assert_eq!(text, "error[E0001] (cli): bad\\n\\u{1b}argument\n");
    }
}
