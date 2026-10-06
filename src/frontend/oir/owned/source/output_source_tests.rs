//! Private real-source process proof. Public source/CLI admission stays closed.
//! The exact child is inert unless its parent provides OXID_SOURCE_STDOUT_PATH.
//! Run owns a dedicated inherited stdout fd; Emit only builds saved artifacts.
use super::super::{
    builtin_output_process_tests::{redirect_actual_stdout, save_new},
    execute::OwnedRunFailure,
    plan, process, Diagnostic, Scalar, SourceMap,
};
use super::program::{self, OutputPipelineMode, OutputPipelineOutput, OutputPipelineRequest};
use crate::frontend::{
    declaration_index::SourceOwner,
    project::{budget::Allocator, ProjectLimits, ProjectSources},
};
use std::path::Path;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const CHILD: &str = concat!(
    "frontend::oir::owned::source::output_source_tests::",
    "output_source_subprocess_child"
);
const DRIVER_OPTIONS: &[u8] = b"invalid private output source process options\n";

fn bounded_fuel() -> Option<usize> {
    match std::env::var("OXID_SOURCE_STDOUT_FUEL") {
        Ok(value) => value.parse().ok().filter(|fuel| *fuel <= plan::MAX_FUEL),
        Err(std::env::VarError::NotPresent) => Some(plan::MAX_FUEL),
        Err(_) => None,
    }
}

fn report_before_execution(bytes: &[u8]) -> i32 {
    if process::setup() {
        process::diagnostic(bytes)
    } else {
        74
    }
}

/// These errors precede Run, hence setup must precede even their presentation.
/// The map is always the actual loader/pipeline owner's map, including failure.
fn source_errors_status(errors: &[Diagnostic], sources: &SourceMap) -> i32 {
    if !process::setup() {
        return 74;
    }
    for error in errors {
        if process::diagnostic(error.render_human(sources).as_bytes()) == 74 {
            return 74;
        }
    }
    1
}

fn reference_status(path: &str, fuel: usize) -> i32 {
    let project = match ProjectSources::load_output_candidate(
        path,
        ProjectLimits::default(),
        &mut Allocator::default(),
    ) {
        Ok(project) => project,
        Err(failure) => {
            return source_errors_status(&failure.diagnostics, &failure.sources);
        }
    };
    let output = program::run_output_source(
        SourceOwner::project(&project),
        OutputPipelineRequest {
            mode: OutputPipelineMode::Run,
            fuel,
        },
    );
    let status = match output {
        Err(errors) => source_errors_status(&errors, project.sources()),
        Ok(OutputPipelineOutput::Run(Ok(Scalar::I32(value @ 0..=255)))) => value,
        // Neither failure has established safe inherited-descriptor reporting.
        Ok(OutputPipelineOutput::Run(Err(
            OwnedRunFailure::ProcessSetup | OwnedRunFailure::ProcessHost,
        ))) => 74,
        Ok(OutputPipelineOutput::Run(Err(error))) => process::diagnostic(
            error
                .diagnostic(project.sources())
                .render_human(project.sources())
                .as_bytes(),
        ),
        Ok(OutputPipelineOutput::Run(Ok(_))) => {
            process::diagnostic(b"invalid private process result\n")
        }
        Ok(OutputPipelineOutput::Emit(_)) => {
            report_before_execution(b"invalid private source pipeline mode\n")
        }
    };
    drop(project);
    // Fresh paid index/typed/raw/witness owners died inside run_output_source;
    // all diagnostic and project owners die here, before the child's exit.
    status
}

fn save_build_errors(directory: &Path, errors: &[Diagnostic], sources: &SourceMap) -> i32 {
    let mut text = String::new();
    for error in errors {
        text.push_str(&error.render_human(sources));
    }
    if save_new(directory.join("build-diagnostic.txt"), text.as_bytes()).is_ok() {
        1
    } else {
        74
    }
}

/// Emission never calls Run, process setup, or process diagnostic reporting.
/// The existing native compiler adapter compiles the bounded module afterward.
fn native_artifact_status(path: &str, fuel: usize, directory: &Path) -> i32 {
    if !directory.is_dir() {
        return 74;
    }
    let project = match ProjectSources::load_output_candidate(
        path,
        ProjectLimits::default(),
        &mut Allocator::default(),
    ) {
        Ok(project) => project,
        Err(failure) => {
            return save_build_errors(directory, &failure.diagnostics, &failure.sources);
        }
    };
    let output = program::run_output_source(
        SourceOwner::project(&project),
        OutputPipelineRequest {
            mode: OutputPipelineMode::Emit,
            fuel,
        },
    );
    let module = match output {
        Ok(OutputPipelineOutput::Emit(Ok(module))) => module,
        Ok(OutputPipelineOutput::Emit(Err(error))) => {
            return save_build_errors(directory, &[*error], project.sources());
        }
        Err(errors) => return save_build_errors(directory, &errors, project.sources()),
        Ok(OutputPipelineOutput::Run(_)) => {
            return if save_new(directory.join("build-diagnostic.txt"), DRIVER_OPTIONS).is_ok() {
                1
            } else {
                74
            };
        }
    };
    drop(project);
    if save_new(directory.join("program.ll"), module.as_bytes()).is_err() {
        return 74;
    }
    let output = directory.join("program");
    let Some(output) = output.to_str() else {
        return 74;
    };
    match crate::frontend::native::compile(&module, output) {
        Ok(()) => 0,
        Err(error) => save_build_errors(directory, &[*error], &SourceMap::new()),
    }
}

/// Invoke only this exact test with --exact --nocapture --test-threads=1.
/// A normal suite invocation has no selector and performs no process effects.
#[test]
fn output_source_subprocess_child() {
    let Some(selected) = std::env::var_os("OXID_SOURCE_STDOUT_PATH") else {
        return;
    };
    let native_directory = std::env::var_os("OXID_SOURCE_STDOUT_NATIVE_DIR");
    let status = if let Some(directory) = native_directory {
        let directory = std::path::PathBuf::from(directory);
        match (selected.to_str(), bounded_fuel()) {
            (Some(path), Some(fuel)) => native_artifact_status(path, fuel, &directory),
            _ => {
                if save_new(directory.join("build-diagnostic.txt"), DRIVER_OPTIONS).is_ok() {
                    1
                } else {
                    74
                }
            }
        }
    } else if !redirect_actual_stdout() {
        74
    } else {
        match (selected.to_str(), bounded_fuel()) {
            (Some(path), Some(fuel)) => reference_status(path, fuel),
            _ => report_before_execution(DRIVER_OPTIONS),
        }
    };
    drop(selected);
    // Libtest's closing marker must never become part of language stdout.
    std::process::exit(status);
}

const ABC: &str = r#"
use std::io::write_stdout as output;
use std::io::WriteStatus as Status;
fn main() -> i32 {
    let bytes = [65, 66, 67];
    let status = output(&bytes);
    match status {
        Status::Complete => { return 0; },
        Status::InvalidInput => { return 1; },
        Status::IoError(n) => { return n + 2; },
    }
}
"#;
const PROJECTED: &str = r#"
use std::io::write_stdout as output;
use std::io::WriteStatus as Status;
struct Packet { bytes: [i32; 3] }
fn main() -> i32 {
    let packet = Packet { bytes: [65, 66, 67] };
    let status = output(&packet.bytes);
    match status {
        Status::Complete => { return 0; },
        Status::InvalidInput => { return 1; },
        Status::IoError(n) => { return n + 2; },
    }
}
"#;
const FORWARDED: &str = r#"
use std::io::write_stdout as output;
use std::io::WriteStatus as Status;
fn relay(bytes: &[i32]) -> Status { return output(&*bytes); }
fn main() -> i32 {
    let bytes = [65, 66, 67];
    let status = relay(&bytes);
    match status {
        Status::Complete => { return 0; },
        Status::InvalidInput => { return 1; },
        Status::IoError(n) => { return n + 2; },
    }
}
"#;
const MIXED_ZERO_INPUT: &str = r#"
use std::io::write_stdout as output;
use std::io::WriteStatus as Written;
use std::io::read_stdin as input;
use std::io::ReadStatus as Read;
fn main() -> i32 {
    let mut empty: [i32; 0] = [];
    let read = input(&mut empty);
    match read {
        Read::Eof(n) => { return n + 10; },
        Read::Full => {
            let bytes = [65, 66, 67];
            let written = output(&bytes);
            match written {
                Written::Complete => { return 0; },
                Written::InvalidInput => { return 1; },
                Written::IoError(n) => { return n + 2; },
            }
        },
        Read::IoError => { return 11; },
    }
}
"#;
const STATUS_ONLY: &str = r#"
use std::io::ReadStatus as R;
use std::io::WriteStatus as W;
fn main() -> i32 {
    let read = R::Full;
    match read {
        R::Eof(n) => { return n; },
        R::Full => {
            let written = W::IoError(5);
            match written {
                W::Complete => { return 0; },
                W::InvalidInput => { return 1; },
                W::IoError(n) => { return n + 2; },
            }
        },
        R::IoError => { return 11; },
    }
}
"#;
const MODULE_ROOT: &str = r#"
use std::io::WriteStatus as FirstStatus;
pub mod child;
use crate::child::relay;
fn output() -> i32 { return 23; }
fn main() -> i32 {
    let bytes = [65, 66, 67];
    return relay(&bytes);
}
"#;
const MODULE_CHILD: &str = r#"
use std::io::write_stdout as output;
use std::io::WriteStatus as Status;
pub fn relay(bytes: &[i32]) -> i32 {
    let status = output(&*bytes);
    match status {
        Status::Complete => { return 0; },
        Status::InvalidInput => { return 1; },
        Status::IoError(n) => { return n + 2; },
    }
}
"#;

struct Case {
    name: &'static str,
    text: String,
    bytes: &'static [u8],
    status: i32,
}
fn cases() -> Vec<Case> {
    [
        ("abc", ABC.to_owned(), b"ABC".as_slice(), 0),
        (
            "invalid-last-256",
            ABC.replace("65, 66, 67", "65, 66, 256"),
            b"",
            1,
        ),
        (
            "invalid-last-negative",
            ABC.replace("65, 66, 67", "65, 66, -1"),
            b"",
            1,
        ),
        (
            "empty",
            ABC.replace("let bytes = [65, 66, 67];", "let bytes: [i32; 0] = [];"),
            b"",
            0,
        ),
        ("projected", PROJECTED.to_owned(), b"ABC", 0),
        ("forwarded", FORWARDED.to_owned(), b"ABC", 0),
        ("mixed-zero-input", MIXED_ZERO_INPUT.to_owned(), b"ABC", 0),
        ("status-only", STATUS_ONLY.to_owned(), b"", 7),
        ("status-0", "fn main()->i32{return 0;}".into(), b"", 0),
        ("status-37", "fn main()->i32{return 37;}".into(), b"", 37),
    ]
    .into_iter()
    .map(|(name, text, bytes, status)| Case {
        name,
        text,
        bytes,
        status,
    })
    .collect()
}

struct ProjectFixture(std::path::PathBuf);
impl ProjectFixture {
    fn new(files: &[(&str, &str)]) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "oxid-output-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        for (name, text) in files {
            save_new(directory.join(name), text.as_bytes()).unwrap();
        }
        Self(directory)
    }
    fn path(&self) -> std::path::PathBuf {
        self.0.join("main.ox")
    }
    fn load(&self) -> ProjectSources {
        ProjectSources::load_output_candidate(
            self.path().to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap()
    }
}
impl Drop for ProjectFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn emit(project: &ProjectSources) -> Result<OutputPipelineOutput, Vec<Diagnostic>> {
    program::run_output_source(
        SourceOwner::project(project),
        OutputPipelineRequest {
            mode: OutputPipelineMode::Emit,
            // Even a zero-fuel Run could change signal policy. Emit only
            // constructs text and is safe in an ordinary parallel test suite.
            fuel: 0,
        },
    )
}
fn emitted_module(project: &ProjectSources) -> String {
    let OutputPipelineOutput::Emit(module) = emit(project).unwrap() else {
        panic!("Emit must never select the reference consumer");
    };
    module.unwrap()
}

#[test]
fn output_source_single_file_emit_is_effect_free_and_portable() {
    for case in cases() {
        let fixture = ProjectFixture::new(&[("main.ox", &case.text)]);
        let project = fixture.load();
        let module = emitted_module(&project);
        assert!(
            module.contains("call i32 @__oxid_process_setup()"),
            "{}",
            case.name
        );
        assert!(module.contains("store i64 0, ptr %fuel"), "{}", case.name);
        assert!(
            !module.contains("call i32 @__oxid_print_i32"),
            "{}",
            case.name
        );
        if case.name == "status-only" {
            assert!(!module.contains("@__oxid_write_stdout_byte"));
            assert!(!module.contains("@__oxid_read_stdin_byte"));
        }
        if case.name == "mixed-zero-input" {
            assert!(module.contains("@__oxid_write_stdout_byte"));
            assert!(module.contains("@__oxid_read_stdin_byte"));
        }
        // Expectations are independently literal source bytes/statuses, used
        // only by the explicit subprocess parent below, never by the pipeline.
        assert!(case.bytes.len() <= 3 && (0..=255).contains(&case.status));
    }
}

#[test]
fn output_source_aliases_and_status_dependencies_remain_scoped() {
    let text = format!("fn write_stdout()->i32{{return 23;}} {ABC}");
    let fixture = ProjectFixture::new(&[("main.ox", &text)]);
    assert!(emitted_module(&fixture.load()).contains("@__oxid_write_stdout_byte"));

    let text = "use std::io::write_stdout as output; fn helper()->WriteStatus{return;} fn main()->i32{return 0;}";
    let fixture = ProjectFixture::new(&[("main.ox", text)]);
    let project = fixture.load();
    let errors = emit(&project).unwrap_err();
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|error| error
        .primary
        .is_some_and(|span| { project.sources().text(span) == "WriteStatus" })));
}

#[test]
fn output_source_emit_requires_original_zero_argument_i32_main() {
    for text in [
        "fn main()->bool{return true;}",
        "fn main(value:i32)->i32{return value;}",
        "fn helper()->i32{return 0;}",
    ] {
        let fixture = ProjectFixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        let errors = emit(&project).unwrap_err();
        assert_eq!(errors.len(), 1, "{text}");
        assert_eq!((errors[0].code, errors[0].stage), ("E0600", "oir-run"));
    }
}

#[test]
fn output_source_public_loader_stays_closed() {
    let fixture = ProjectFixture::new(&[("main.ox", ABC)]);
    let failure =
        ProjectSources::load_typed(fixture.path().to_str().unwrap(), ProjectLimits::default())
            .unwrap_err();
    assert!(!failure.diagnostics.is_empty());
    assert!(failure
        .diagnostics
        .iter()
        .any(|error| error.code == "E0101"));
}

#[test]
fn output_source_module_policy_and_local_aliases_are_preserved() {
    let fixture = ProjectFixture::new(&[("main.ox", MODULE_ROOT), ("child.ox", MODULE_CHILD)]);
    let result = ProjectSources::load_output_candidate(
        fixture.path().to_str().unwrap(),
        ProjectLimits::default(),
        &mut Allocator::default(),
    );
    if cfg!(target_os = "linux") {
        let project = result.unwrap();
        assert_eq!(project.sources().files().len(), 2);
        assert!(emitted_module(&project).contains("@__oxid_write_stdout_byte"));
    } else {
        let failure = result.unwrap_err();
        assert_eq!(failure.diagnostics.len(), 1);
        assert_eq!(
            (failure.diagnostics[0].code, failure.diagnostics[0].stage),
            ("E0005", "source")
        );
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod subprocess {
    use super::*;
    use std::os::{fd::AsRawFd, unix::process::CommandExt};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    fn capture(
        directory: &Path,
        files: &[(&str, &str)],
        readonly: bool,
    ) -> (i32, Vec<u8>, Vec<u8>) {
        std::fs::create_dir(directory).unwrap();
        for (name, text) in files {
            save_new(directory.join(name), text.as_bytes()).unwrap();
        }
        let output_path = directory.join("actual.stdout");
        let output = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&output_path)
            .unwrap();
        let output = if readonly {
            drop(output);
            std::fs::File::open(&output_path).unwrap()
        } else {
            output
        };
        let fd = output.as_raw_fd();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", CHILD, "--nocapture", "--test-threads=1"])
            .env("OXID_SOURCE_STDOUT_PATH", directory.join("main.ox"))
            .env("OXID_RAW_STDOUT_FD", fd.to_string())
            .env_remove("OXID_SOURCE_STDOUT_NATIVE_DIR")
            .env_remove("OXID_SOURCE_STDOUT_FUEL")
            .env_remove("OXID_RAW_STDOUT_CASE")
            .stdin(Stdio::null())
            .stdout(std::fs::File::create(directory.join("harness.stdout")).unwrap())
            .stderr(std::fs::File::create(directory.join("actual.stderr")).unwrap());
        // SAFETY: after fork, only fcntl is called to retain the dedicated fd
        // across exec. The parent's own descriptor flags are unchanged.
        unsafe {
            command.pre_exec(move || {
                unsafe extern "C" {
                    fn fcntl(fd: std::ffi::c_int, command: std::ffi::c_int, ...)
                        -> std::ffi::c_int;
                }
                if fcntl(fd, 2, 0) < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let mut child = command.spawn().unwrap();
        drop(output);
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("source process proof timed out: {}", directory.display());
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        save_new(
            directory.join("status.txt"),
            format!("{status}\n").as_bytes(),
        )
        .unwrap();
        (
            status
                .code()
                .expect("process must exit normally, never by SIGPIPE"),
            std::fs::read(output_path).unwrap(),
            std::fs::read(directory.join("actual.stderr")).unwrap(),
        )
    }

    #[test]
    #[ignore = "explicit private source process proof; retains OXID_SOURCE_STDOUT_EVIDENCE_DIR"]
    fn output_source_reference_bytes_and_statuses() {
        let directory = std::path::PathBuf::from(
            std::env::var_os("OXID_SOURCE_STDOUT_EVIDENCE_DIR")
                .expect("fresh source proof directory"),
        );
        std::fs::create_dir(&directory).unwrap();
        for case in cases() {
            assert_eq!(
                capture(
                    &directory.join(case.name),
                    &[("main.ox", &case.text)],
                    false
                ),
                (case.status, case.bytes.to_vec(), vec![]),
                "{}",
                case.name,
            );
        }
        assert_eq!(
            capture(
                &directory.join("module-alias"),
                &[("main.ox", MODULE_ROOT), ("child.ox", MODULE_CHILD)],
                false
            ),
            (0, b"ABC".to_vec(), vec![]),
        );
        assert_eq!(
            capture(&directory.join("abc-readonly"), &[("main.ox", ABC)], true),
            (2, vec![], vec![])
        );
        let empty = ABC.replace("let bytes = [65, 66, 67];", "let bytes: [i32; 0] = [];");
        assert_eq!(
            capture(
                &directory.join("empty-readonly"),
                &[("main.ox", &empty)],
                true
            ),
            (0, vec![], vec![])
        );
        for (name, text) in [
            ("status-negative", "fn main()->i32{return -256;}"),
            ("status-256", "fn main()->i32{return 256;}"),
        ] {
            let (status, stdout, stderr) =
                capture(&directory.join(name), &[("main.ox", text)], false);
            assert_eq!((status, stdout), (1, vec![]));
            let diagnostic = String::from_utf8(stderr).unwrap();
            assert!(diagnostic.starts_with(
                "error[E0600] (oir-run): process main must return a status in 0..255\n"
            ));
            assert!(diagnostic.ends_with("main.ox:1:4\n"));
        }
    }
}
