use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const CORPUS: &str = "tests/fixtures/typed_project_examples";
const CHILD_TIMEOUT: Duration = Duration::from_secs(30);
const OUTPUT_LIMIT: usize = 1024 * 1024;
const CHECK_ARGV: &[&str] = &[
    "check",
    "--manifest-path=oxid.toml",
    "--message-format=json",
];
const RUN_ARGV: &[&str] = &["run", "--manifest-path=oxid.toml", "--entry-mode=result"];
const SINGLE_MANIFEST: &[u8] =
    b"[project]\nname = \"example\"\nedition = \"typed-preview\"\nentry = \"main.ox\"\n";
const APP_MANIFEST: &[u8] = b"[project]\nname = \"example\"\nedition = \"typed-preview\"\nentry = \"main.ox\"\n[dependencies]\nhelper = \"helper\"\n";
const HELPER_MANIFEST: &[u8] =
    b"[project]\nname = \"helper\"\nedition = \"typed-preview\"\nentry = \"lib.ox\"\n";
const MAIN_ZERO: &[u8] = b"fn main() -> i32 { return 0; }\n";
const MAIN_MISSING_SEMI: &[u8] = b"fn main() -> i32 { return 0 }\n";
const MAIN_TRUE: &[u8] = b"fn main() -> i32 { return true; }\n";
const MAIN_SEVEN: &[u8] = b"fn main() -> i32 { return 7; }\n";
const MAIN_C06: &[u8] = b"use helper::value;\nfn main() -> i32 { return value(); }\n";
const MAIN_C07: &[u8] = b"use helper::value;\nfn main() -> i32 { return 0; }\n";
const HELPER_SEVEN: &[u8] = b"pub fn value() -> i32 { return 7; }\n";
const HELPER_TRUE: &[u8] = b"pub fn value() -> i32 { return true; }\n";
const INVALID_UTF8: &[u8] = &[0xff];
const DOC_D01: &[u8] = b"# D01\n\n```oxid\nfn main() -> i32 { return 7; }\n```\n";
const DOC_D02: &[u8] = b"# D02\n\n```oxid\nfn main() -> i32 { return 7; }\n```\n";
const DOC_D03: &[u8] = b"# D03\n\n```oxid\nfn main() -> i32 { return 7; }\n```\n";
const DOC_D04: &[u8] = b"# D04\n\n```oxid\nfn main() -> i32 { return 8; }\n```\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    CheckPass,
    CompileFail,
    RejectSelftest,
    DocCheck,
    DocRun,
    InertPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    CheckPass,
    CompileFail,
    RejectSelftest,
    DocCheck,
    DocRun,
    InertPolicy,
}

#[derive(Clone, Copy, Debug)]
struct Case {
    id: &'static str,
    kind: Kind,
    mode: Mode,
    files: &'static [FileSpec],
    doc: Option<DocSpec>,
    diagnostic: Option<DiagnosticExpectation>,
    functions: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
struct FileSpec {
    path: &'static str,
    bytes: &'static [u8],
}

#[derive(Clone, Copy, Debug)]
struct DocSpec {
    path: &'static str,
    bytes: &'static [u8],
    source_path: &'static str,
    source_bytes: &'static [u8],
    body_range: (usize, usize),
}

#[derive(Clone, Copy, Debug)]
struct DiagnosticExpectation {
    code: &'static str,
    stage: &'static str,
    path: Option<&'static str>,
    start: u64,
    end: u64,
    line: u64,
    column: u64,
    end_line: u64,
    end_column: u64,
}

const C01_FILES: &[FileSpec] = &[
    FileSpec {
        path: "oxid.toml",
        bytes: SINGLE_MANIFEST,
    },
    FileSpec {
        path: "main.ox",
        bytes: MAIN_ZERO,
    },
];
const C02_FILES: &[FileSpec] = &[
    FileSpec {
        path: "oxid.toml",
        bytes: SINGLE_MANIFEST,
    },
    FileSpec {
        path: "main.ox",
        bytes: MAIN_MISSING_SEMI,
    },
];
const C03_FILES: &[FileSpec] = &[
    FileSpec {
        path: "oxid.toml",
        bytes: SINGLE_MANIFEST,
    },
    FileSpec {
        path: "main.ox",
        bytes: MAIN_TRUE,
    },
];
const C04_FILES: &[FileSpec] = C03_FILES;
const C05_FILES: &[FileSpec] = C01_FILES;
const C06_FILES: &[FileSpec] = &[
    FileSpec {
        path: "oxid.toml",
        bytes: APP_MANIFEST,
    },
    FileSpec {
        path: "main.ox",
        bytes: MAIN_C06,
    },
    FileSpec {
        path: "helper/oxid.toml",
        bytes: HELPER_MANIFEST,
    },
    FileSpec {
        path: "helper/lib.ox",
        bytes: HELPER_SEVEN,
    },
];
const C07_FILES: &[FileSpec] = &[
    FileSpec {
        path: "oxid.toml",
        bytes: APP_MANIFEST,
    },
    FileSpec {
        path: "main.ox",
        bytes: MAIN_C07,
    },
    FileSpec {
        path: "helper/oxid.toml",
        bytes: HELPER_MANIFEST,
    },
    FileSpec {
        path: "helper/lib.ox",
        bytes: HELPER_TRUE,
    },
];
const C08_FILES: &[FileSpec] = &[
    FileSpec {
        path: "oxid.toml",
        bytes: SINGLE_MANIFEST,
    },
    FileSpec {
        path: "main.ox",
        bytes: INVALID_UTF8,
    },
];
const D01_FILES: &[FileSpec] = &[
    FileSpec {
        path: "oxid.toml",
        bytes: SINGLE_MANIFEST,
    },
    FileSpec {
        path: "main.ox",
        bytes: MAIN_SEVEN,
    },
];
const D02_FILES: &[FileSpec] = D01_FILES;
const D03_FILES: &[FileSpec] = D01_FILES;
const D04_FILES: &[FileSpec] = D01_FILES;

const C02_DIAG: DiagnosticExpectation = DiagnosticExpectation {
    code: "E0100",
    stage: "parse",
    path: Some("main.ox"),
    start: 28,
    end: 29,
    line: 1,
    column: 29,
    end_line: 1,
    end_column: 30,
};
const C03_DIAG: DiagnosticExpectation = DiagnosticExpectation {
    code: "E0300",
    stage: "type",
    path: Some("main.ox"),
    start: 26,
    end: 30,
    line: 1,
    column: 27,
    end_line: 1,
    end_column: 31,
};
const C04_WRONG_DIAG: DiagnosticExpectation = DiagnosticExpectation {
    code: "E0100",
    stage: "type",
    path: Some("main.ox"),
    start: 26,
    end: 30,
    line: 1,
    column: 27,
    end_line: 1,
    end_column: 31,
};
const C05_WRONG_DIAG: DiagnosticExpectation = DiagnosticExpectation {
    code: "E0300",
    stage: "type",
    path: Some("main.ox"),
    start: 26,
    end: 27,
    line: 1,
    column: 27,
    end_line: 1,
    end_column: 28,
};
const C07_DIAG: DiagnosticExpectation = DiagnosticExpectation {
    code: "E0300",
    stage: "type",
    path: Some("helper/lib.ox"),
    start: 31,
    end: 35,
    line: 1,
    column: 32,
    end_line: 1,
    end_column: 36,
};
const C08_DIAG: DiagnosticExpectation = DiagnosticExpectation {
    code: "E0003",
    stage: "source",
    path: None,
    start: 0,
    end: 0,
    line: 0,
    column: 0,
    end_line: 0,
    end_column: 0,
};
const D01_DOC: DocSpec = DocSpec {
    path: "docs/D01.md",
    bytes: DOC_D01,
    source_path: "main.ox",
    source_bytes: MAIN_SEVEN,
    body_range: (15, 46),
};
const D02_DOC: DocSpec = DocSpec {
    path: "docs/D02.md",
    bytes: DOC_D02,
    source_path: "main.ox",
    source_bytes: MAIN_SEVEN,
    body_range: (15, 46),
};
const D03_DOC: DocSpec = DocSpec {
    path: "docs/D03.md",
    bytes: DOC_D03,
    source_path: "main.ox",
    source_bytes: MAIN_SEVEN,
    body_range: (15, 46),
};
const D04_DOC: DocSpec = DocSpec {
    path: "docs/D04.md",
    bytes: DOC_D04,
    source_path: "main.ox",
    source_bytes: MAIN_SEVEN,
    body_range: (15, 46),
};

/// This registry is deliberately independent of any compiler-emitted output.
const REGISTRY: [Case; 12] = [
    Case {
        id: "C01",
        kind: Kind::CheckPass,
        mode: Mode::CheckPass,
        files: C01_FILES,
        doc: None,
        diagnostic: None,
        functions: Some(1),
    },
    Case {
        id: "C02",
        kind: Kind::CompileFail,
        mode: Mode::CompileFail,
        files: C02_FILES,
        doc: None,
        diagnostic: Some(C02_DIAG),
        functions: None,
    },
    Case {
        id: "C03",
        kind: Kind::CompileFail,
        mode: Mode::CompileFail,
        files: C03_FILES,
        doc: None,
        diagnostic: Some(C03_DIAG),
        functions: None,
    },
    Case {
        id: "C04",
        kind: Kind::RejectSelftest,
        mode: Mode::RejectSelftest,
        files: C04_FILES,
        doc: None,
        diagnostic: Some(C04_WRONG_DIAG),
        functions: None,
    },
    Case {
        id: "C05",
        kind: Kind::RejectSelftest,
        mode: Mode::RejectSelftest,
        files: C05_FILES,
        doc: None,
        diagnostic: Some(C05_WRONG_DIAG),
        functions: None,
    },
    Case {
        id: "C06",
        kind: Kind::CheckPass,
        mode: Mode::CheckPass,
        files: C06_FILES,
        doc: None,
        diagnostic: None,
        functions: Some(2),
    },
    Case {
        id: "C07",
        kind: Kind::CompileFail,
        mode: Mode::CompileFail,
        files: C07_FILES,
        doc: None,
        diagnostic: Some(C07_DIAG),
        functions: None,
    },
    Case {
        id: "C08",
        kind: Kind::CompileFail,
        mode: Mode::CompileFail,
        files: C08_FILES,
        doc: None,
        diagnostic: Some(C08_DIAG),
        functions: None,
    },
    Case {
        id: "D01",
        kind: Kind::DocCheck,
        mode: Mode::DocCheck,
        files: D01_FILES,
        doc: Some(D01_DOC),
        diagnostic: None,
        functions: Some(1),
    },
    Case {
        id: "D02",
        kind: Kind::DocRun,
        mode: Mode::DocRun,
        files: D02_FILES,
        doc: Some(D02_DOC),
        diagnostic: None,
        functions: Some(1),
    },
    Case {
        id: "D03",
        kind: Kind::InertPolicy,
        mode: Mode::InertPolicy,
        files: D03_FILES,
        doc: Some(D03_DOC),
        diagnostic: None,
        functions: None,
    },
    Case {
        id: "D04",
        kind: Kind::RejectSelftest,
        mode: Mode::RejectSelftest,
        files: D04_FILES,
        doc: Some(D04_DOC),
        diagnostic: None,
        functions: None,
    },
];

/// An independently frozen ID/category inventory: never derived from REGISTRY.
const REQUIRED_INVENTORY: [(&str, Kind); 12] = [
    ("C01", Kind::CheckPass),
    ("C02", Kind::CompileFail),
    ("C03", Kind::CompileFail),
    ("C04", Kind::RejectSelftest),
    ("C05", Kind::RejectSelftest),
    ("C06", Kind::CheckPass),
    ("C07", Kind::CompileFail),
    ("C08", Kind::CompileFail),
    ("D01", Kind::DocCheck),
    ("D02", Kind::DocRun),
    ("D03", Kind::InertPolicy),
    ("D04", Kind::RejectSelftest),
];
const NORMAL_IDS: [&str; 8] = ["C01", "C02", "C03", "C06", "C07", "C08", "D01", "D02"];
const INERT_ID: &str = "D03";
const EXECUTABLE_DOC_IDS: [&str; 2] = ["D01", "D02"];
static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Eq, PartialEq)]
enum HarnessError {
    Configuration(String),
    Fixture(String),
    DocFixtureByteMismatch {
        doc_offset: usize,
        fixture_offset: usize,
        doc_byte: u8,
        fixture_byte: u8,
    },
    Protocol(String),
    DiagnosticCodeMismatch {
        expected: String,
        actual: String,
    },
    ExpectedCompileFailureButCheckSucceeded,
    Launch(LaunchError),
    CheckFailed(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum LaunchError {
    Spawn(String),
    ReadStdout(String),
    ReadStderr(String),
    Wait(String),
    Timeout,
    Signal,
    OutputLimit(&'static str),
    Kill(String),
    Reap(String),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ProcessStatus {
    Code(i32),
    Signal,
}

#[derive(Debug, Clone)]
struct ProcessResult {
    status: ProcessStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[derive(Debug, Clone)]
struct Request {
    case_id: String,
    cwd: PathBuf,
    argv: Vec<String>,
}

trait Launcher {
    fn launch(&mut self, request: &Request) -> Result<ProcessResult, LaunchError>;
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[derive(Default)]
struct RealLauncher {
    trace: Trace,
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
impl Launcher for RealLauncher {
    fn launch(&mut self, request: &Request) -> Result<ProcessResult, LaunchError> {
        self.trace.requests.push(request.clone());
        let result = run_child(
            env!("CARGO_BIN_EXE_oxid"),
            request,
            CHILD_TIMEOUT,
            OUTPUT_LIMIT,
            None,
        );
        println!(
            "RAW-LAUNCH {}",
            serde_json::json!({
                "case": request.case_id, "cwd": request.cwd, "argv": request.argv,
                "result": format!("{result:?}")
            })
        );
        result
    }
}

#[derive(Default)]
struct Trace {
    requests: Vec<Request>,
}

struct SpyLauncher {
    trace: Trace,
    replies: VecDeque<Result<ProcessResult, LaunchError>>,
}
impl SpyLauncher {
    fn new(replies: impl IntoIterator<Item = Result<ProcessResult, LaunchError>>) -> Self {
        Self {
            trace: Trace::default(),
            replies: replies.into_iter().collect(),
        }
    }
}
impl Launcher for SpyLauncher {
    fn launch(&mut self, request: &Request) -> Result<ProcessResult, LaunchError> {
        self.trace.requests.push(request.clone());
        self.replies.pop_front().unwrap_or_else(|| {
            Err(LaunchError::Spawn(
                "spy has no scripted response".to_owned(),
            ))
        })
    }
}

fn invoke<L: Launcher>(
    launcher: &mut L,
    case: &Case,
    cwd: &Path,
    argv: &[&str],
) -> Result<ProcessResult, HarnessError> {
    let request = Request {
        case_id: case.id.to_owned(),
        cwd: cwd.to_path_buf(),
        argv: argv.iter().map(|item| (*item).to_owned()).collect(),
    };
    launcher.launch(&request).map_err(HarnessError::Launch)
}

fn run_child(
    executable: &str,
    request: &Request,
    timeout: Duration,
    cap: usize,
    child_mode: Option<&str>,
) -> Result<ProcessResult, LaunchError> {
    let mut command = Command::new(executable);
    command
        .args(&request.argv)
        .current_dir(&request.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(mode) = child_mode {
        command.env("OXID_EXAMPLES_TEST_CHILD_MODE", mode);
    }
    let mut child = command
        .spawn()
        .map_err(|error| LaunchError::Spawn(error.to_string()))?;
    let stdout = match child.stdout.take() {
        Some(pipe) => pipe,
        None => {
            let _ = terminate_and_reap(&mut child);
            return Err(LaunchError::ReadStdout(
                "stdout pipe unavailable".to_owned(),
            ));
        }
    };
    let stderr = match child.stderr.take() {
        Some(pipe) => pipe,
        None => {
            let _ = terminate_and_reap(&mut child);
            return Err(LaunchError::ReadStderr(
                "stderr pipe unavailable".to_owned(),
            ));
        }
    };
    let (sender, receiver) = mpsc::channel();
    let out_sender = sender.clone();
    let out_thread = thread::spawn(move || {
        let result = read_capped(stdout, cap, "stdout");
        let _ = out_sender.send(("stdout", result));
    });
    let err_thread = thread::spawn(move || {
        let result = read_capped(stderr, cap, "stderr");
        let _ = sender.send(("stderr", result));
    });
    let deadline = Instant::now() + timeout;
    let mut status: Option<ExitStatus> = None;
    let mut out: Option<Vec<u8>> = None;
    let mut err: Option<Vec<u8>> = None;
    let mut readers = 0;
    loop {
        if status.is_none() {
            match child.try_wait() {
                Ok(result) => status = result,
                Err(error) => {
                    let _ = terminate_and_reap(&mut child);
                    join_readers(out_thread, err_thread);
                    return Err(LaunchError::Wait(error.to_string()));
                }
            }
        }
        loop {
            match receiver.try_recv() {
                Ok((stream, result)) => {
                    readers += 1;
                    match (stream, result) {
                        ("stdout", Ok(bytes)) => out = Some(bytes),
                        ("stderr", Ok(bytes)) => err = Some(bytes),
                        ("stdout", Err(ReaderFailure::Io(error))) => {
                            let _ = terminate_and_reap(&mut child);
                            join_readers(out_thread, err_thread);
                            return Err(LaunchError::ReadStdout(error));
                        }
                        ("stderr", Err(ReaderFailure::Io(error))) => {
                            let _ = terminate_and_reap(&mut child);
                            join_readers(out_thread, err_thread);
                            return Err(LaunchError::ReadStderr(error));
                        }
                        ("stdout", Err(ReaderFailure::Limit)) => {
                            let _ = terminate_and_reap(&mut child);
                            join_readers(out_thread, err_thread);
                            return Err(LaunchError::OutputLimit("stdout"));
                        }
                        ("stderr", Err(ReaderFailure::Limit)) => {
                            let _ = terminate_and_reap(&mut child);
                            join_readers(out_thread, err_thread);
                            return Err(LaunchError::OutputLimit("stderr"));
                        }
                        _ => unreachable!(),
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
        if status.is_some() && readers == 2 {
            break;
        }
        if Instant::now() >= deadline {
            let termination = terminate_and_reap(&mut child);
            join_readers(out_thread, err_thread);
            termination?;
            return Err(LaunchError::Timeout);
        }
        match receiver.recv_timeout(Duration::from_millis(10)) {
            Ok((stream, result)) => {
                readers += 1;
                match (stream, result) {
                    ("stdout", Ok(bytes)) => out = Some(bytes),
                    ("stderr", Ok(bytes)) => err = Some(bytes),
                    ("stdout", Err(ReaderFailure::Io(error))) => {
                        let _ = terminate_and_reap(&mut child);
                        join_readers(out_thread, err_thread);
                        return Err(LaunchError::ReadStdout(error));
                    }
                    ("stderr", Err(ReaderFailure::Io(error))) => {
                        let _ = terminate_and_reap(&mut child);
                        join_readers(out_thread, err_thread);
                        return Err(LaunchError::ReadStderr(error));
                    }
                    ("stdout", Err(ReaderFailure::Limit)) => {
                        let _ = terminate_and_reap(&mut child);
                        join_readers(out_thread, err_thread);
                        return Err(LaunchError::OutputLimit("stdout"));
                    }
                    ("stderr", Err(ReaderFailure::Limit)) => {
                        let _ = terminate_and_reap(&mut child);
                        join_readers(out_thread, err_thread);
                        return Err(LaunchError::OutputLimit("stderr"));
                    }
                    _ => unreachable!(),
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }
    }
    join_readers(out_thread, err_thread);
    let exit = status.expect("status checked before successful exit");
    let process_status = if let Some(code) = exit.code() {
        ProcessStatus::Code(code)
    } else {
        ProcessStatus::Signal
    };
    Ok(ProcessResult {
        status: process_status,
        stdout: out.unwrap_or_default(),
        stderr: err.unwrap_or_default(),
    })
}

#[derive(Debug)]
enum ReaderFailure {
    Io(String),
    Limit,
}
fn read_capped(mut reader: impl Read, cap: usize, _stream: &str) -> Result<Vec<u8>, ReaderFailure> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| ReaderFailure::Io(error.to_string()))?;
        if count == 0 {
            return Ok(bytes);
        }
        if bytes.len().saturating_add(count) > cap {
            return Err(ReaderFailure::Limit);
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
}
fn terminate_and_reap(child: &mut Child) -> Result<(), LaunchError> {
    let kill_result = child.kill();
    let wait_result = child.wait();
    if let Err(error) = wait_result {
        return Err(LaunchError::Reap(error.to_string()));
    }
    if let Err(error) = kill_result {
        if error.kind() != std::io::ErrorKind::InvalidInput {
            return Err(LaunchError::Kill(error.to_string()));
        }
    }
    Ok(())
}
fn join_readers(out: thread::JoinHandle<()>, err: thread::JoinHandle<()>) {
    let _ = out.join();
    let _ = err.join();
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Observation {
    code: Option<String>,
    stage: Option<String>,
    path: Option<String>,
    start: Option<u64>,
    end: Option<u64>,
    line: Option<u64>,
    column: Option<u64>,
    end_line: Option<u64>,
    end_column: Option<u64>,
    file_id: Option<u64>,
    success: bool,
    errors: u64,
    functions: Option<u64>,
}

fn run_check<L: Launcher>(launcher: &mut L, case: &Case, cwd: &Path) -> Result<(), HarnessError> {
    if case.mode == Mode::InertPolicy {
        return Err(HarnessError::Configuration(
            "inert policy material cannot launch CHECK".to_owned(),
        ));
    }
    let result = invoke(launcher, case, cwd, CHECK_ARGV)?;
    let observation = parse_check_output(&result, cwd)?;
    if !result.stderr.is_empty() {
        return Err(HarnessError::Protocol(
            "CHECK stderr must be empty".to_owned(),
        ));
    }
    if let Some(expected) = case.diagnostic {
        if observation.success {
            if case.id == "C05" {
                if observation.functions != Some(1) {
                    return Err(HarnessError::Protocol(
                        "C05 successful CHECK must report one function".to_owned(),
                    ));
                }
                return Err(HarnessError::ExpectedCompileFailureButCheckSucceeded);
            }
            return Err(HarnessError::CheckFailed(
                "expected a diagnostic, observed success".to_owned(),
            ));
        }
        if observation.stage.as_deref() != Some(expected.stage)
            || observation.path.as_deref() != expected.path
            || observation.start != expected.path.map(|_| expected.start)
            || observation.end != expected.path.map(|_| expected.end)
            || observation.line != expected.path.map(|_| expected.line)
            || observation.column != expected.path.map(|_| expected.column)
            || observation.end_line != expected.path.map(|_| expected.end_line)
            || observation.end_column != expected.path.map(|_| expected.end_column)
        {
            return Err(HarnessError::Protocol(format!(
                "{} diagnostic projection differs from frozen oracle",
                case.id
            )));
        }
        if observation.code.as_deref() != Some(expected.code) {
            return Err(HarnessError::DiagnosticCodeMismatch {
                expected: expected.code.to_owned(),
                actual: observation.code.unwrap_or_else(|| "<missing>".to_owned()),
            });
        }
    } else if !observation.success || observation.errors != 0 || observation.code.is_some() {
        return Err(HarnessError::CheckFailed(format!(
            "{} expected successful CHECK",
            case.id
        )));
    }
    if let Some(functions) = case.functions {
        if observation.functions != Some(functions) {
            return Err(HarnessError::Protocol(format!(
                "{} functions count differs",
                case.id
            )));
        }
    }
    Ok(())
}

fn parse_check_output(result: &ProcessResult, cwd: &Path) -> Result<Observation, HarnessError> {
    match result.status {
        ProcessStatus::Signal => return Err(HarnessError::Launch(LaunchError::Signal)),
        ProcessStatus::Code(2) => {
            return Err(HarnessError::CheckFailed(
                "internal compiler exit 2".to_owned(),
            ))
        }
        ProcessStatus::Code(code) if code != 0 && code != 1 => {
            return Err(HarnessError::CheckFailed(format!(
                "unexpected CHECK exit {code}"
            )))
        }
        _ => {}
    }
    let text = std::str::from_utf8(&result.stdout)
        .map_err(|error| HarnessError::Protocol(format!("stdout is not UTF-8: {error}")))?;
    if !text.ends_with('\n') {
        return Err(HarnessError::Protocol(
            "JSONL stdout lacks final LF".to_owned(),
        ));
    }
    let mut records = Vec::new();
    for (index, line) in text.split_terminator('\n').enumerate() {
        if line.trim().is_empty() {
            return Err(HarnessError::Protocol(format!(
                "blank JSONL record at line {}",
                index + 1
            )));
        }
        let value: Value = serde_json::from_str(line).map_err(|error| {
            HarnessError::Protocol(format!("invalid JSONL record {}: {error}", index + 1))
        })?;
        if !value.is_object() {
            return Err(HarnessError::Protocol(format!(
                "JSONL record {} is not an object",
                index + 1
            )));
        }
        records.push(value);
    }
    if records.is_empty() {
        return Err(HarnessError::Protocol("empty JSONL stream".to_owned()));
    }
    let summaries: Vec<usize> = records
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            (value.get("kind").and_then(Value::as_str) == Some("check-summary")).then_some(index)
        })
        .collect();
    if summaries != [records.len() - 1] {
        return Err(HarnessError::Protocol(
            "exactly one final check-summary is required".to_owned(),
        ));
    }
    let summary = records.last().expect("nonempty records");
    validate_summary(summary)?;
    let diagnostics = &records[..records.len() - 1];
    let mut projections = Vec::new();
    let mut file_ids = HashMap::<String, u64>::new();
    for diagnostic in diagnostics {
        let projection = validate_diagnostic(diagnostic, cwd)?;
        if let (Some(path), Some(file_id)) = (&projection.path, projection.file_id) {
            if file_ids
                .insert(path.clone(), file_id)
                .is_some_and(|old| old != file_id)
            {
                return Err(HarnessError::Protocol(
                    "file_id changed for one source path".to_owned(),
                ));
            }
        }
        projections.push(projection);
    }
    let success = summary["success"].as_bool().expect("summary validated");
    let errors = summary["errors"].as_u64().expect("summary validated");
    let functions = summary["functions"].as_u64();
    let status = match result.status {
        ProcessStatus::Code(code) => code,
        ProcessStatus::Signal => unreachable!(),
    };
    if success != (errors == 0) || errors != projections.len() as u64 || success != (status == 0) {
        return Err(HarnessError::Protocol(
            "summary, diagnostics, and exit status disagree".to_owned(),
        ));
    }
    if (success && functions.is_none()) || (!success && !summary["functions"].is_null()) {
        return Err(HarnessError::Protocol(
            "functions must be a nonnegative integer on success and null on failure".to_owned(),
        ));
    }
    if (success && !projections.is_empty()) || (!success && projections.len() != 1) {
        return Err(HarnessError::Protocol(
            "this corpus requires zero or exactly one diagnostic".to_owned(),
        ));
    }
    let first = projections.into_iter().next();
    Ok(Observation {
        code: first.as_ref().and_then(|item| item.code.clone()),
        stage: first.as_ref().and_then(|item| item.stage.clone()),
        path: first.as_ref().and_then(|item| item.path.clone()),
        start: first.as_ref().and_then(|item| item.start),
        end: first.as_ref().and_then(|item| item.end),
        line: first.as_ref().and_then(|item| item.line),
        column: first.as_ref().and_then(|item| item.column),
        end_line: first.as_ref().and_then(|item| item.end_line),
        end_column: first.as_ref().and_then(|item| item.end_column),
        file_id: first.as_ref().and_then(|item| item.file_id),
        success,
        errors,
        functions,
    })
}

fn exact_keys(value: &Value, keys: &[&str], what: &str) -> Result<(), HarnessError> {
    let object = value
        .as_object()
        .ok_or_else(|| HarnessError::Protocol(format!("{what} is not an object")))?;
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = keys.iter().copied().collect();
    if actual != expected {
        return Err(HarnessError::Protocol(format!("{what} key set differs")));
    }
    Ok(())
}
fn validate_summary(value: &Value) -> Result<(), HarnessError> {
    exact_keys(
        value,
        &[
            "schema_version",
            "edition",
            "kind",
            "success",
            "errors",
            "functions",
        ],
        "summary",
    )?;
    if value["schema_version"].as_u64() != Some(1)
        || value["edition"].as_str() != Some("typed-preview")
        || value["kind"].as_str() != Some("check-summary")
        || value["success"].as_bool().is_none()
        || value["errors"].as_u64().is_none()
        || !(value["functions"].is_null() || value["functions"].as_u64().is_some())
    {
        return Err(HarnessError::Protocol(
            "summary field value/type is invalid".to_owned(),
        ));
    }
    Ok(())
}
fn validate_diagnostic(value: &Value, cwd: &Path) -> Result<Observation, HarnessError> {
    exact_keys(
        value,
        &[
            "schema_version",
            "edition",
            "kind",
            "severity",
            "code",
            "stage",
            "message",
            "primary",
            "secondary",
            "notes",
        ],
        "diagnostic",
    )?;
    if value["schema_version"].as_u64() != Some(1)
        || value["edition"].as_str() != Some("typed-preview")
        || value["kind"].as_str() != Some("diagnostic")
        || value["severity"].as_str() != Some("error")
        || value["code"].as_str().is_none()
        || value["stage"].as_str().is_none()
        || value["message"].as_str().is_none()
        || !value["secondary"].is_array()
        || !value["notes"].is_array()
        || value["secondary"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
        || value["notes"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    {
        return Err(HarnessError::Protocol(
            "diagnostic field value/type is invalid".to_owned(),
        ));
    }
    let (path, start, end, line, column, end_line, end_column, file_id) = if value["primary"]
        .is_null()
    {
        (None, None, None, None, None, None, None, None)
    } else {
        let span = &value["primary"];
        exact_keys(
            span,
            &[
                "file_id",
                "path",
                "start",
                "end",
                "line",
                "column",
                "end_line",
                "end_column",
            ],
            "primary span",
        )?;
        let file_id = span["file_id"].as_u64().ok_or_else(|| {
            HarnessError::Protocol("file_id must be a nonnegative integer".to_owned())
        })?;
        let raw_path = span["path"]
            .as_str()
            .ok_or_else(|| HarnessError::Protocol("span path must be a string".to_owned()))?;
        let normalized = normalize_path(raw_path, cwd)?;
        let numbers = ["start", "end", "line", "column", "end_line", "end_column"].map(|name| {
            span[name].as_u64().ok_or_else(|| {
                HarnessError::Protocol(format!("span {name} must be a nonnegative integer"))
            })
        });
        let [start, end, line, column, end_line, end_column] = numbers;
        (
            Some(normalized),
            Some(start?),
            Some(end?),
            Some(line?),
            Some(column?),
            Some(end_line?),
            Some(end_column?),
            Some(file_id),
        )
    };
    Ok(Observation {
        code: value["code"].as_str().map(str::to_owned),
        stage: value["stage"].as_str().map(str::to_owned),
        path,
        start,
        end,
        line,
        column,
        end_line,
        end_column,
        file_id,
        success: false,
        errors: 1,
        functions: None,
    })
}
fn normalize_path(raw: &str, cwd: &Path) -> Result<String, HarnessError> {
    let reject = || HarnessError::Protocol("unrecognized diagnostic path normalization".to_owned());
    let cwd_text = cwd.to_str().ok_or_else(reject)?;
    let prefix = format!("{cwd_text}/");
    let relative = if Path::new(raw).is_absolute() {
        raw.strip_prefix(&prefix).ok_or_else(reject)?
    } else {
        raw
    };
    let text = relative.strip_prefix("./").unwrap_or(relative);
    if text.is_empty()
        || text.contains('\\')
        || text.contains(':')
        || text
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || Path::new(text)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(reject());
    }
    Ok(text.to_owned())
}

fn run_case<L: Launcher>(launcher: &mut L, case: &Case, cwd: &Path) -> Result<(), HarnessError> {
    run_check(launcher, case, cwd)?;
    if case.mode == Mode::DocRun {
        let result = invoke(launcher, case, cwd, RUN_ARGV)?;
        if result.status == ProcessStatus::Signal {
            return Err(HarnessError::Launch(LaunchError::Signal));
        }
        if result.status != ProcessStatus::Code(0) {
            return Err(HarnessError::CheckFailed(
                "D02 RUN did not exit zero".to_owned(),
            ));
        }
        if result.stdout != b"7\n" || !result.stderr.is_empty() {
            return Err(HarnessError::Protocol(
                "D02 RUN must emit exactly 7 LF and empty stderr".to_owned(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
struct TempTree(PathBuf);
impl TempTree {
    const CREATE_ATTEMPTS: usize = 128;

    fn new() -> Self {
        Self::try_new_in(&std::env::temp_dir(), || {
            let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            format!("oxid-typed-project-examples-{}-{nonce}", std::process::id())
        })
        .expect("create exclusive harness temp tree")
    }

    // A test-owned parent and deterministic names exercise the same creation path.
    // Only a successful exclusive create confers ownership and permits Drop cleanup.
    fn try_new_in(parent: &Path, mut next_name: impl FnMut() -> String) -> std::io::Result<Self> {
        for _ in 0..Self::CREATE_ATTEMPTS {
            let path = parent.join(next_name());
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "exhausted 128 exclusive harness temp-tree creation attempts",
        ))
    }
    fn case_dir(&self, id: &str) -> PathBuf {
        self.0.join(id)
    }
}
impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn frozen_case(id: &str) -> &'static Case {
    REGISTRY
        .iter()
        .find(|case| case.id == id)
        .expect("frozen case ID")
}
fn validate_registry(rows: &[Case]) -> Result<(), HarnessError> {
    if rows.len() != REQUIRED_INVENTORY.len() {
        return Err(HarnessError::Configuration(
            "registry must contain exactly twelve rows".to_owned(),
        ));
    }
    let mut seen = BTreeSet::new();
    for row in rows {
        if !seen.insert(row.id) {
            return Err(HarnessError::Configuration(format!(
                "duplicate registry ID {}",
                row.id
            )));
        }
        let expected = REQUIRED_INVENTORY
            .iter()
            .find(|(id, _)| *id == row.id)
            .ok_or_else(|| {
                HarnessError::Configuration(format!("unknown registry ID {}", row.id))
            })?;
        if row.kind != expected.1 || mode_kind(row.mode) != row.kind {
            return Err(HarnessError::Configuration(format!(
                "wrong category or mode for {}",
                row.id
            )));
        }
        let frozen = REGISTRY
            .iter()
            .find(|candidate| candidate.id == row.id)
            .ok_or_else(|| HarnessError::Configuration(format!("missing frozen row {}", row.id)))?;
        if row.mode != frozen.mode
            || row.files.len() != frozen.files.len()
            || row
                .files
                .iter()
                .zip(frozen.files)
                .any(|(actual, fixed)| actual.path != fixed.path || actual.bytes != fixed.bytes)
            || !same_doc(row.doc, frozen.doc)
            || !same_diagnostic(row.diagnostic, frozen.diagnostic)
            || row.functions != frozen.functions
        {
            return Err(HarnessError::Configuration(format!(
                "changed frozen file inventory or mode for {}",
                row.id
            )));
        }
    }
    for (id, _) in REQUIRED_INVENTORY {
        if !seen.contains(id) {
            return Err(HarnessError::Configuration(format!(
                "missing registry ID {id}"
            )));
        }
    }
    Ok(())
}
fn same_doc(left: Option<DocSpec>, right: Option<DocSpec>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.path == right.path
                && left.bytes == right.bytes
                && left.source_path == right.source_path
                && left.source_bytes == right.source_bytes
                && left.body_range == right.body_range
        }
        _ => false,
    }
}
fn same_diagnostic(
    left: Option<DiagnosticExpectation>,
    right: Option<DiagnosticExpectation>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.code == right.code
                && left.stage == right.stage
                && left.path == right.path
                && left.start == right.start
                && left.end == right.end
                && left.line == right.line
                && left.column == right.column
                && left.end_line == right.end_line
                && left.end_column == right.end_column
        }
        _ => false,
    }
}
fn mode_kind(mode: Mode) -> Kind {
    match mode {
        Mode::CheckPass => Kind::CheckPass,
        Mode::CompileFail => Kind::CompileFail,
        Mode::RejectSelftest => Kind::RejectSelftest,
        Mode::DocCheck => Kind::DocCheck,
        Mode::DocRun => Kind::DocRun,
        Mode::InertPolicy => Kind::InertPolicy,
    }
}
fn validate_selection(
    rows: &[Case],
    selected: &[&str],
    expected: &[&str],
    allow_inert: bool,
) -> Result<Vec<&'static Case>, HarnessError> {
    validate_registry(rows)?;
    if selected.is_empty() {
        return Err(HarnessError::Configuration(
            "empty executable selection".to_owned(),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut cases = Vec::new();
    for id in selected {
        if !seen.insert(*id) {
            return Err(HarnessError::Configuration(format!(
                "duplicate selected ID {id}"
            )));
        }
        let case = rows
            .iter()
            .find(|row| row.id == *id)
            .ok_or_else(|| HarnessError::Configuration(format!("unknown selected ID {id}")))?;
        if case.mode == Mode::InertPolicy && !allow_inert {
            return Err(HarnessError::Configuration(
                "inert row selected for execution".to_owned(),
            ));
        }
        if case.mode == Mode::RejectSelftest && !expected.contains(id) {
            return Err(HarnessError::Configuration(
                "reject-selftest leaked into ordinary selection".to_owned(),
            ));
        }
        if !expected.contains(id) {
            return Err(HarnessError::Configuration(format!(
                "unexpected selected ID {id}"
            )));
        }
        cases.push(frozen_case(id));
    }
    if seen.len() != expected.len() || expected.iter().any(|id| !seen.contains(id)) {
        return Err(HarnessError::Configuration(
            "selected list omits a required ID".to_owned(),
        ));
    }
    if cases.iter().all(|case| case.mode == Mode::InertPolicy) {
        return Err(HarnessError::Configuration(
            "zero executable selections".to_owned(),
        ));
    }
    Ok(cases)
}

fn preflight(cases: &[&Case]) -> Result<(), HarnessError> {
    if cases.is_empty() {
        return Err(HarnessError::Configuration(
            "zero executable selections".to_owned(),
        ));
    }
    let mut actual_paths = BTreeSet::new();
    collect_files(Path::new(CORPUS), Path::new(CORPUS), &mut actual_paths)?;
    validate_inventory_paths(&actual_paths)?;
    let mut selected_ids = BTreeSet::new();
    for case in cases {
        if !selected_ids.insert(case.id) {
            return Err(HarnessError::Configuration(
                "duplicate selected case ID".to_owned(),
            ));
        }
        for file in case.files {
            let rel = Path::new(file.path);
            if rel.is_absolute()
                || rel
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
            {
                return Err(HarnessError::Configuration(format!(
                    "invalid declared file path {}",
                    file.path
                )));
            }
            let path = Path::new(CORPUS).join(case.id).join(rel);
            let actual = fs::read(&path)
                .map_err(|error| HarnessError::Fixture(format!("{}: {error}", path.display())))?;
            if actual != file.bytes {
                return Err(HarnessError::Fixture(format!(
                    "fixture bytes differ: {}",
                    path.display()
                )));
            }
        }
        if let Some(doc) = case.doc {
            let path = Path::new(CORPUS).join(doc.path);
            let actual = fs::read(&path)
                .map_err(|error| HarnessError::Fixture(format!("{}: {error}", path.display())))?;
            if actual != doc.bytes {
                return Err(HarnessError::Fixture(format!(
                    "document bytes differ: {}",
                    path.display()
                )));
            }
            let source = case
                .files
                .iter()
                .find(|file| file.path == doc.source_path)
                .ok_or_else(|| {
                    HarnessError::Configuration(format!(
                        "{} maps to undeclared source {}",
                        doc.path, doc.source_path
                    ))
                })?;
            if source.bytes != doc.source_bytes {
                return Err(HarnessError::Configuration(format!(
                    "{} source mapping differs from frozen bytes",
                    doc.path
                )));
            }
            validate_doc_mapping(doc, &actual)?;
        }
    }
    Ok(())
}
fn validate_doc_mapping(doc: DocSpec, actual: &[u8]) -> Result<(), HarnessError> {
    let (start, end) = doc.body_range;
    if actual.len() != 50 || (start, end) != (15, 46) || end > actual.len() || start > end {
        return Err(HarnessError::Fixture(format!(
            "{} frozen document position/length changed",
            doc.path
        )));
    }
    let body = &actual[start..end];
    if body != doc.source_bytes {
        let difference = body
            .iter()
            .zip(doc.source_bytes)
            .position(|(left, right)| left != right)
            .unwrap_or_else(|| body.len().min(doc.source_bytes.len()));
        return Err(HarnessError::DocFixtureByteMismatch {
            doc_offset: start + difference,
            fixture_offset: difference,
            doc_byte: *body.get(difference).unwrap_or(&0),
            fixture_byte: *doc.source_bytes.get(difference).unwrap_or(&0),
        });
    }
    Ok(())
}
fn copy_preflighted(
    cases: &[&Case],
    tree: &TempTree,
) -> Result<BTreeMap<&'static str, PathBuf>, HarnessError> {
    let mut roots = BTreeMap::new();
    for case in cases {
        let root = tree.case_dir(case.id);
        for file in case.files {
            let source = Path::new(CORPUS).join(case.id).join(file.path);
            let destination = root.join(file.path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| HarnessError::Fixture(error.to_string()))?;
            }
            fs::copy(source, destination)
                .map_err(|error| HarnessError::Fixture(error.to_string()))?;
        }
        roots.insert(case.id, root);
    }
    Ok(roots)
}

fn run_normal_corpus<L: Launcher>(launcher: &mut L) -> Result<(), HarnessError> {
    let cases = validate_selection(&REGISTRY, &NORMAL_IDS, &NORMAL_IDS, false)?;
    run_cases_after_preflight(launcher, &cases)
}
fn run_cases_after_preflight<L: Launcher>(
    launcher: &mut L,
    cases: &[&Case],
) -> Result<(), HarnessError> {
    validate_registry(&REGISTRY)?;
    if cases.iter().any(|case| case.mode == Mode::InertPolicy) {
        return Err(HarnessError::Configuration(
            "inert case selected for executable path".to_owned(),
        ));
    }
    preflight(cases)?;
    let tree = TempTree::new();
    let roots = copy_preflighted(cases, &tree)?;
    for case in cases {
        run_case(launcher, case, &roots[case.id])?;
    }
    Ok(())
}
fn run_single<L: Launcher>(launcher: &mut L, id: &str) -> Result<(), HarnessError> {
    validate_registry(&REGISTRY)?;
    let case = frozen_case(id);
    run_cases_after_preflight(launcher, &[case])
}
fn process_document_input<L: Launcher>(launcher: &mut L, id: &str) -> Result<(), HarnessError> {
    validate_registry(&REGISTRY)?;
    let case = frozen_case(id);
    preflight(&[case])?;
    if EXECUTABLE_DOC_IDS.contains(&id) && matches!(case.mode, Mode::DocCheck | Mode::DocRun) {
        return run_cases_after_preflight(launcher, &[case]);
    }
    if case.mode == Mode::InertPolicy && !EXECUTABLE_DOC_IDS.contains(&id) {
        return Ok(());
    }
    Err(HarnessError::Configuration(format!(
        "{} is not a registered documentation example",
        id
    )))
}

fn success_result(functions: u64) -> ProcessResult {
    ProcessResult {
        status: ProcessStatus::Code(0),
        stdout: format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":true,\"errors\":0,\"functions\":{functions}}}\n").into_bytes(),
        stderr: Vec::new(),
    }
}
#[allow(clippy::too_many_arguments)]
fn failure_result(
    code: &str,
    stage: &str,
    path: &str,
    start: u64,
    end: u64,
    line: u64,
    column: u64,
    end_line: u64,
    end_column: u64,
) -> ProcessResult {
    let diagnostic = format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"{code}\",\"stage\":\"{stage}\",\"message\":\"frozen test diagnostic\",\"primary\":{{\"file_id\":1,\"path\":\"{path}\",\"start\":{start},\"end\":{end},\"line\":{line},\"column\":{column},\"end_line\":{end_line},\"end_column\":{end_column}}},\"secondary\":[],\"notes\":[]}}\n");
    let summary = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}\n";
    ProcessResult {
        status: ProcessStatus::Code(1),
        stdout: format!("{diagnostic}{summary}").into_bytes(),
        stderr: Vec::new(),
    }
}
fn doc_run_result() -> ProcessResult {
    ProcessResult {
        status: ProcessStatus::Code(0),
        stdout: b"7\n".to_vec(),
        stderr: Vec::new(),
    }
}

#[test]
fn typed_project_examples_exact_corpus_inventory_and_byte_mappings() {
    validate_registry(&REGISTRY).unwrap();
    assert_eq!(REQUIRED_INVENTORY.len(), 12);
    let mut expected_paths = BTreeSet::new();
    for case in REGISTRY {
        assert_eq!(mode_kind(case.mode), case.kind, "{} category/mode", case.id);
        for file in case.files {
            let path = Path::new(CORPUS).join(case.id).join(file.path);
            let bytes =
                fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert_eq!(bytes, file.bytes, "{}", path.display());
            expected_paths.insert(
                path.strip_prefix(CORPUS)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
        if let Some(doc) = case.doc {
            let path = Path::new(CORPUS).join(doc.path);
            let bytes = fs::read(&path).unwrap();
            assert_eq!(bytes, doc.bytes);
            expected_paths.insert(doc.path.to_owned());
            assert_eq!(bytes.len(), 50);
            if case.id == "D04" {
                assert_eq!(
                    validate_doc_mapping(doc, &bytes),
                    Err(HarnessError::DocFixtureByteMismatch {
                        doc_offset: 41,
                        fixture_offset: 26,
                        doc_byte: 0x38,
                        fixture_byte: 0x37,
                    })
                );
            } else {
                validate_doc_mapping(doc, &bytes).unwrap();
                assert_eq!(&bytes[15..46], doc.source_bytes);
            }
        }
    }
    assert_eq!(
        fs::read(Path::new(CORPUS).join("C08/main.ox")).unwrap(),
        [0xff]
    );
    assert_eq!(
        fs::metadata(Path::new(CORPUS).join("C08/main.ox"))
            .unwrap()
            .len(),
        1
    );
    let mut actual_paths = BTreeSet::new();
    collect_files(Path::new(CORPUS), Path::new(CORPUS), &mut actual_paths).unwrap();
    assert_eq!(
        actual_paths, expected_paths,
        "no undeclared or missing corpus files"
    );
    assert_eq!(&DOC_D04[41..42], b"8");
    assert_eq!(&MAIN_SEVEN[26..27], b"7");
}
fn expected_corpus_paths() -> BTreeSet<String> {
    let mut expected = BTreeSet::new();
    for case in REGISTRY {
        for file in case.files {
            expected.insert(format!("{}/{}", case.id, file.path));
        }
        if let Some(doc) = case.doc {
            expected.insert(doc.path.to_owned());
        }
    }
    expected
}
fn validate_inventory_paths(actual: &BTreeSet<String>) -> Result<(), HarnessError> {
    if actual != &expected_corpus_paths() {
        return Err(HarnessError::Configuration(
            "missing or undeclared corpus file".to_owned(),
        ));
    }
    Ok(())
}
fn collect_files(
    root: &Path,
    current: &Path,
    output: &mut BTreeSet<String>,
) -> Result<(), HarnessError> {
    let fixture_error = |error: std::io::Error| HarnessError::Fixture(error.to_string());
    for item in fs::read_dir(current).map_err(fixture_error)? {
        let item = item.map_err(fixture_error)?;
        let path = item.path();
        let kind = item.file_type().map_err(fixture_error)?;
        if kind.is_symlink() || !(kind.is_dir() || kind.is_file()) {
            return Err(HarnessError::Configuration(
                "non-regular corpus entry".to_owned(),
            ));
        }
        if kind.is_dir() {
            collect_files(root, &path, output)?;
        } else {
            output.insert(
                path.strip_prefix(root)
                    .expect("recursive corpus root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn typed_project_examples_linux_normal_real_cli_corpus() {
    let mut launcher = RealLauncher::default();
    run_normal_corpus(&mut launcher).unwrap();
    assert_eq!(launcher.trace.requests.len(), 9);
    assert_eq!(
        launcher
            .trace
            .requests
            .iter()
            .filter(|request| request.argv.first().map(String::as_str) == Some("run"))
            .count(),
        1
    );
    assert_eq!(launcher.trace.requests.last().unwrap().case_id, "D02");
    for request in &launcher.trace.requests {
        assert!(
            request.argv
                == CHECK_ARGV
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect::<Vec<_>>()
                || (request.case_id == "D02"
                    && request.argv
                        == RUN_ARGV
                            .iter()
                            .map(|value| (*value).to_owned())
                            .collect::<Vec<_>>())
        );
        assert!(request.cwd.is_absolute());
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn typed_project_examples_linux_rejects_c04_for_exact_diagnostic_code_mismatch() {
    let mut launcher = RealLauncher::default();
    let error = run_single(&mut launcher, "C04").unwrap_err();
    assert_eq!(
        error,
        HarnessError::DiagnosticCodeMismatch {
            expected: "E0100".to_owned(),
            actual: "E0300".to_owned()
        }
    );
    assert_eq!(launcher.trace.requests.len(), 1);
    assert_eq!(
        launcher.trace.requests[0].argv,
        CHECK_ARGV
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn typed_project_examples_linux_rejects_c05_for_exact_unexpected_check_success() {
    let mut launcher = RealLauncher::default();
    assert_eq!(
        run_single(&mut launcher, "C05").unwrap_err(),
        HarnessError::ExpectedCompileFailureButCheckSucceeded
    );
    assert_eq!(launcher.trace.requests.len(), 1);
    assert_eq!(
        launcher.trace.requests[0].argv,
        CHECK_ARGV
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn typed_project_examples_spy_proves_d01_check_only_and_d02_check_then_run() {
    let d01 = success_result(1);
    let d02 = success_result(1);
    let mut launcher = SpyLauncher::new([Ok(d01), Ok(d02), Ok(doc_run_result())]);
    run_single(&mut launcher, "D01").unwrap();
    run_single(&mut launcher, "D02").unwrap();
    let requests = &launcher.trace.requests;
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].case_id, "D01");
    assert_eq!(
        requests[0].argv,
        CHECK_ARGV
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(requests[1].case_id, "D02");
    assert_eq!(
        requests[1].argv,
        CHECK_ARGV
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(requests[2].case_id, "D02");
    assert_eq!(
        requests[2].argv,
        RUN_ARGV
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn typed_project_examples_failed_d02_check_never_emits_run() {
    let mut launcher = SpyLauncher::new([Ok(failure_result(
        "E0100", "parse", "main.ox", 28, 29, 1, 29, 1, 30,
    ))]);
    assert!(run_single(&mut launcher, "D02").is_err());
    assert_eq!(launcher.trace.requests.len(), 1);
    assert_eq!(
        launcher.trace.requests[0].argv,
        CHECK_ARGV
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn typed_project_examples_d03_inert_fence_has_no_launch_result() {
    let mut launcher = SpyLauncher::new([]);
    process_document_input(&mut launcher, INERT_ID).unwrap();
    assert!(launcher.trace.requests.is_empty());

    let mut positive_control = SpyLauncher::new([Ok(success_result(1)), Ok(doc_run_result())]);
    process_document_input(&mut positive_control, "D02").unwrap();
    assert_eq!(positive_control.trace.requests.len(), 2);
    assert_eq!(
        positive_control.trace.requests[0].argv,
        CHECK_ARGV
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        positive_control.trace.requests[1].argv,
        RUN_ARGV
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn typed_project_examples_d04_exact_mismatch_precedes_any_launch() {
    let mut launcher = SpyLauncher::new([]);
    let error = run_single(&mut launcher, "D04").unwrap_err();
    assert_eq!(
        error,
        HarnessError::DocFixtureByteMismatch {
            doc_offset: 41,
            fixture_offset: 26,
            doc_byte: 0x38,
            fixture_byte: 0x37,
        }
    );
    assert!(launcher.trace.requests.is_empty());
}

#[test]
fn typed_project_examples_global_preflight_rejects_valid_d01_before_bad_d04() {
    let mut launcher = SpyLauncher::new([]);
    let cases = [frozen_case("D01"), frozen_case("D04")];
    let error = run_cases_after_preflight(&mut launcher, &cases).unwrap_err();
    assert_eq!(
        error,
        HarnessError::DocFixtureByteMismatch {
            doc_offset: 41,
            fixture_offset: 26,
            doc_byte: 0x38,
            fixture_byte: 0x37,
        }
    );
    assert!(launcher.trace.requests.is_empty());
}

#[test]
fn typed_project_examples_registry_and_selection_mutations_are_zero_call_errors() {
    let launcher = SpyLauncher::new([]);
    let mut missing = REGISTRY.to_vec();
    missing.pop();
    assert!(matches!(
        validate_registry(&missing),
        Err(HarnessError::Configuration(_))
    ));

    let mut duplicate = REGISTRY.to_vec();
    duplicate.push(REGISTRY[0]);
    assert!(matches!(
        validate_registry(&duplicate),
        Err(HarnessError::Configuration(_))
    ));

    let mut wrong_mode = REGISTRY.to_vec();
    wrong_mode[0].mode = Mode::CompileFail;
    assert!(matches!(
        validate_registry(&wrong_mode),
        Err(HarnessError::Configuration(_))
    ));

    assert!(validate_selection(&REGISTRY, &[], &NORMAL_IDS, false).is_err());
    assert!(validate_selection(&REGISTRY, &["C01", "C01"], &["C01", "C01"], false).is_err());
    assert!(validate_selection(&REGISTRY, &["C01"], &["C01", "C02"], false).is_err());
    assert!(validate_selection(&REGISTRY, &[INERT_ID], &[INERT_ID], true).is_err());

    let mut missing_file_case = *frozen_case("C01");
    static MISSING_DECLARATION: &[FileSpec] = &[FileSpec {
        path: "missing.ox",
        bytes: b"missing\n",
    }];
    missing_file_case.files = MISSING_DECLARATION;
    assert!(matches!(
        preflight(&[&missing_file_case]),
        Err(HarnessError::Fixture(_))
    ));
    assert!(launcher.trace.requests.is_empty());
}

fn rejected_check(result: ProcessResult, case_id: &str) -> HarnessError {
    let mut launcher = SpyLauncher::new([Ok(result)]);
    run_check(
        &mut launcher,
        frozen_case(case_id),
        Path::new("/tmp/typed-example-case"),
    )
    .unwrap_err()
}

fn success_json(functions: &str) -> String {
    format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":true,\"errors\":0,\"functions\":{functions}}}\n")
}
fn failure_json(diagnostic: &str, summary: &str) -> Vec<u8> {
    format!("{diagnostic}\n{summary}\n").into_bytes()
}
fn diag_json(code: &str, stage: &str, path: &str, span: &str) -> String {
    format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"{code}\",\"stage\":\"{stage}\",\"message\":\"handwritten mutation\",\"primary\":{{\"file_id\":1,\"path\":\"{path}\",{span}}},\"secondary\":[],\"notes\":[]}}")
}
const FAIL_SUMMARY: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}";
const GOOD_SPAN: &str =
    "\"start\":26,\"end\":30,\"line\":1,\"column\":27,\"end_line\":1,\"end_column\":31";

#[test]
fn typed_project_examples_protocol_rejects_wrong_code_stage_path_span_and_positions() {
    let cases = [
        ("E0100", "type", "main.ox", GOOD_SPAN, "code mismatch"),
        ("E0300", "parse", "main.ox", GOOD_SPAN, "stage mismatch"),
        ("E0300", "type", "helper/lib.ox", GOOD_SPAN, "path mismatch"),
        (
            "E0300",
            "type",
            "main.ox",
            "\"start\":25,\"end\":30,\"line\":1,\"column\":27,\"end_line\":1,\"end_column\":31",
            "span mismatch",
        ),
        (
            "E0300",
            "type",
            "main.ox",
            "\"start\":26,\"end\":30,\"line\":1,\"column\":28,\"end_line\":1,\"end_column\":31",
            "position mismatch",
        ),
    ];
    for (code, stage, path, span, label) in cases {
        let diagnostic = diag_json(code, stage, path, span);
        let stdout = failure_json(&diagnostic, FAIL_SUMMARY);
        let error = rejected_check(
            ProcessResult {
                status: ProcessStatus::Code(1),
                stdout,
                stderr: Vec::new(),
            },
            "C03",
        );
        assert!(
            matches!(
                error,
                HarnessError::DiagnosticCodeMismatch { .. } | HarnessError::Protocol(_)
            ),
            "{label}: {error:?}"
        );
    }
    let escaping = diag_json("E0300", "type", "../main.ox", GOOD_SPAN);
    let error = rejected_check(
        ProcessResult {
            status: ProcessStatus::Code(1),
            stdout: failure_json(&escaping, FAIL_SUMMARY),
            stderr: Vec::new(),
        },
        "C03",
    );
    assert!(matches!(error, HarnessError::Protocol(_)));
}

#[test]
fn typed_project_examples_protocol_rejects_record_count_and_summary_mutations() {
    let good_diag = diag_json("E0300", "type", "main.ox", GOOD_SPAN);
    let good = failure_json(&good_diag, FAIL_SUMMARY);
    let summary = FAIL_SUMMARY;
    let cases = [
        (ProcessStatus::Code(1), format!("{summary}\n"), "missing diagnostic"),
        (ProcessStatus::Code(1), format!("{good_diag}\n{good_diag}\n{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":2,\"functions\":null}}\n"), "duplicate diagnostic"),
        (ProcessStatus::Code(1), format!("{good_diag}\n"), "missing summary"),
        (ProcessStatus::Code(1), format!("{summary}\n{good_diag}\n"), "non-final summary"),
        (ProcessStatus::Code(1), format!("{good_diag}\n{summary}\n{summary}\n"), "duplicate summary"),
        (ProcessStatus::Code(1), format!("{good_diag}\n{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":0,\"functions\":null}}\n"), "error count mismatch"),
        (ProcessStatus::Code(1), format!("{good_diag}\n{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":true,\"errors\":1,\"functions\":1}}\n"), "success mismatch"),
        (ProcessStatus::Code(0), "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":true,\"errors\":0,\"functions\":null}\n".to_owned(), "functions null on success"),
        (ProcessStatus::Code(0), success_json("-1"), "negative functions"),
        (ProcessStatus::Code(0), success_json("1") + "{}\n", "extra record"),
    ];
    for (status, text, label) in cases {
        let error = rejected_check(
            ProcessResult {
                status,
                stdout: text.into_bytes(),
                stderr: Vec::new(),
            },
            "C03",
        );
        assert!(
            matches!(
                error,
                HarnessError::Protocol(_) | HarnessError::CheckFailed(_)
            ),
            "{label}: {error:?}"
        );
    }
    let _ = good;
}

#[test]
fn typed_project_examples_protocol_rejects_json_types_framing_and_stderr() {
    let malformed = [
        b"not-json\n".to_vec(),
        b"{} trailing\n".to_vec(),
        b"{}\n\n{}\n".to_vec(),
        b"{}".to_vec(),
        b"\xff\n".to_vec(),
        b"{} {}\n".to_vec(),
        b"[]\n".to_vec(),
        b"{\"schema_version\":\"1\",\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":true,\"errors\":0,\"functions\":1}\n".to_vec(),
    ];
    for stdout in malformed {
        let error = rejected_check(
            ProcessResult {
                status: ProcessStatus::Code(0),
                stdout,
                stderr: Vec::new(),
            },
            "C01",
        );
        assert!(matches!(error, HarnessError::Protocol(_)));
    }
    let extra_diag_key = diag_json("E0300", "type", "main.ox", GOOD_SPAN)
        .replace("\"notes\":[]}", "\"notes\":[],\"extra\":true}");
    let error = rejected_check(
        ProcessResult {
            status: ProcessStatus::Code(1),
            stdout: failure_json(&extra_diag_key, FAIL_SUMMARY),
            stderr: Vec::new(),
        },
        "C03",
    );
    assert!(matches!(error, HarnessError::Protocol(_)));
    let stderr_error = rejected_check(
        ProcessResult {
            status: ProcessStatus::Code(0),
            stdout: success_result(1).stdout,
            stderr: b"noise\n".to_vec(),
        },
        "C01",
    );
    assert_eq!(
        stderr_error,
        HarnessError::Protocol("CHECK stderr must be empty".to_owned())
    );
}

#[test]
fn typed_project_examples_protocol_rejects_internal_exit_and_signal() {
    let exit_two = rejected_check(
        ProcessResult {
            status: ProcessStatus::Code(2),
            stdout: Vec::new(),
            stderr: Vec::new(),
        },
        "C03",
    );
    assert_eq!(
        exit_two,
        HarnessError::CheckFailed("internal compiler exit 2".to_owned())
    );
    let signal = rejected_check(
        ProcessResult {
            status: ProcessStatus::Signal,
            stdout: Vec::new(),
            stderr: Vec::new(),
        },
        "C03",
    );
    assert_eq!(signal, HarnessError::Launch(LaunchError::Signal));
}

#[test]
fn typed_project_examples_c08_null_primary_is_structurally_accepted() {
    let stdout = b"{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0003\",\"stage\":\"source\",\"message\":\"invalid UTF-8\",\"primary\":null,\"secondary\":[],\"notes\":[]}\n{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}\n".to_vec();
    let mut launcher = SpyLauncher::new([Ok(ProcessResult {
        status: ProcessStatus::Code(1),
        stdout,
        stderr: Vec::new(),
    })]);
    run_check(
        &mut launcher,
        frozen_case("C08"),
        Path::new("/tmp/typed-example-case"),
    )
    .unwrap();
}

#[test]
fn typed_project_examples_unordered_projection_comparison_preserves_multiplicity() {
    assert!(same_multiset(&["a", "b", "a"], &["b", "a", "a"]));
    assert!(!same_multiset(&["a", "b", "a"], &["a", "b"]));
}
fn same_multiset<T: Ord + Clone>(expected: &[T], actual: &[T]) -> bool {
    fn counts<T: Ord + Clone>(values: &[T]) -> BTreeMap<T, usize> {
        let mut result = BTreeMap::new();
        for value in values {
            *result.entry(value.clone()).or_insert(0) += 1;
        }
        result
    }
    counts(expected) == counts(actual)
}

#[test]
fn typed_project_examples_launcher_errors_remain_distinct_and_never_become_expected_failures() {
    let errors = [
        LaunchError::Spawn("injected".to_owned()),
        LaunchError::ReadStdout("injected".to_owned()),
        LaunchError::ReadStderr("injected".to_owned()),
        LaunchError::Wait("injected".to_owned()),
        LaunchError::Timeout,
        LaunchError::Signal,
        LaunchError::OutputLimit("stdout"),
        LaunchError::Kill("injected".to_owned()),
        LaunchError::Reap("injected".to_owned()),
    ];
    for expected in errors {
        let mut launcher = SpyLauncher::new([Err(expected.clone())]);
        let error = run_single(&mut launcher, "C02").unwrap_err();
        assert_eq!(error, HarnessError::Launch(expected));
        assert_eq!(launcher.trace.requests.len(), 1);
        assert_eq!(
            launcher.trace.requests[0].argv,
            CHECK_ARGV
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn typed_project_examples_output_reader_caps_and_distinguishes_reader_errors() {
    assert_eq!(read_capped(&b"abc"[..], 3, "stdout").unwrap(), b"abc");
    assert!(matches!(
        read_capped(&b"abcd"[..], 3, "stdout"),
        Err(ReaderFailure::Limit)
    ));
    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected read failure"))
        }
    }
    assert!(matches!(
        read_capped(BrokenReader, OUTPUT_LIMIT, "stderr"),
        Err(ReaderFailure::Io(_))
    ));
}

#[test]
fn typed_project_examples_absolute_and_single_dot_paths_are_accepted_but_traversal_is_not() {
    let cwd = Path::new("project-root");
    assert_eq!(normalize_path("main.ox", cwd).unwrap(), "main.ox");
    assert_eq!(
        normalize_path("./helper/lib.ox", cwd).unwrap(),
        "helper/lib.ox"
    );
    assert!(normalize_path("././main.ox", cwd).is_err());
    assert!(normalize_path("x/../main.ox", cwd).is_err());

    #[cfg(target_os = "linux")]
    {
        let cwd = Path::new("/tmp/project-root");
        assert!(cwd.is_absolute());
        assert_eq!(
            normalize_path("/tmp/project-root/helper/lib.ox", cwd).unwrap(),
            "helper/lib.ox"
        );
        assert!(normalize_path("/tmp/other/main.ox", cwd).is_err());
    }
}

#[test]
fn typed_project_examples_launcher_child_mode() {
    let Ok(mode) = std::env::var("OXID_EXAMPLES_TEST_CHILD_MODE") else {
        return;
    };
    match mode.as_str() {
        "timeout" => thread::sleep(Duration::from_secs(5)),
        "drain" => {
            use std::io::Write;
            let writer =
                thread::spawn(|| std::io::stderr().write_all(&vec![b'e'; 96 * 1024]).unwrap());
            std::io::stdout().write_all(&vec![b'o'; 96 * 1024]).unwrap();
            writer.join().unwrap();
        }
        "overflow" => {
            use std::io::Write;
            let _ = std::io::stdout().write_all(&vec![b'x'; 96 * 1024]);
        }
        _ => panic!("unknown launcher selftest child mode"),
    }
}

#[test]
fn typed_project_examples_launcher_real_timeout_drain_cap_and_spawn_errors() {
    let tree = TempTree::new();
    let executable = std::env::current_exe().unwrap();
    let executable = executable.to_str().unwrap();
    let request = Request {
        case_id: "launcher-selftest".to_owned(),
        cwd: tree.0.clone(),
        argv: vec![
            "--exact".to_owned(),
            "typed_project_examples_launcher_child_mode".to_owned(),
            "--nocapture".to_owned(),
        ],
    };
    let started = Instant::now();
    assert!(matches!(
        run_child(
            executable,
            &request,
            Duration::from_millis(100),
            OUTPUT_LIMIT,
            Some("timeout")
        ),
        Err(LaunchError::Timeout)
    ));
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "timeout failed to bound a five-second child"
    );
    let drained = run_child(
        executable,
        &request,
        CHILD_TIMEOUT,
        OUTPUT_LIMIT,
        Some("drain"),
    )
    .unwrap();
    assert_eq!(drained.status, ProcessStatus::Code(0));
    assert!(drained
        .stdout
        .windows(96 * 1024)
        .any(|bytes| bytes.iter().all(|byte| *byte == b'o')));
    assert_eq!(drained.stderr, vec![b'e'; 96 * 1024]);
    assert_eq!(
        run_child(executable, &request, CHILD_TIMEOUT, 4096, Some("overflow")).unwrap_err(),
        LaunchError::OutputLimit("stdout")
    );
    assert!(matches!(
        run_child(
            "/nonexistent-oxid-examples-launcher-binary",
            &request,
            CHILD_TIMEOUT,
            OUTPUT_LIMIT,
            None
        ),
        Err(LaunchError::Spawn(_))
    ));
}

#[test]
fn typed_project_examples_reject_selftests_cannot_hide_other_protocol_failures() {
    let result = failure_result("E0300", "parse", "main.ox", 26, 30, 1, 27, 1, 31);
    assert!(matches!(
        rejected_check(result, "C04"),
        HarnessError::Protocol(_)
    ));
    let result = failure_result("E0300", "type", "main.ox", 25, 30, 1, 27, 1, 31);
    assert!(matches!(
        rejected_check(result, "C04"),
        HarnessError::Protocol(_)
    ));
    assert!(matches!(
        rejected_check(success_result(99), "C05"),
        HarnessError::Protocol(_)
    ));
}

#[test]
fn typed_project_examples_inventory_mutations_duplicate_selection_and_inert_bypass_are_zero_call_errors(
) {
    let mut missing = expected_corpus_paths();
    missing.remove("C01/oxid.toml");
    assert!(matches!(
        validate_inventory_paths(&missing),
        Err(HarnessError::Configuration(_))
    ));
    let mut extra = expected_corpus_paths();
    extra.insert("C01/undeclared.ox".to_owned());
    assert!(matches!(
        validate_inventory_paths(&extra),
        Err(HarnessError::Configuration(_))
    ));
    let mut spy = SpyLauncher::new([]);
    assert!(
        run_cases_after_preflight(&mut spy, &[frozen_case("D01"), frozen_case("D01")]).is_err()
    );
    assert!(run_single(&mut spy, "D03").is_err());
    assert!(run_check(
        &mut spy,
        frozen_case("D03"),
        Path::new("/tmp/inert-material")
    )
    .is_err());
    assert!(spy.trace.requests.is_empty());
}

#[test]
fn typed_project_examples_path_normalization_rejects_undocumented_rewrites() {
    let cwd = Path::new("/tmp/project-root");
    for raw in [
        "helper//lib.ox",
        "helper/./lib.ox",
        "main.ox/",
        "././main.ox",
        "C:/foreign/main.ox",
        "helper\\lib.ox",
        "/tmp/project-root//main.ox",
        "/tmp/project-root/../main.ox",
    ] {
        assert!(normalize_path(raw, cwd).is_err(), "{raw}");
    }
}

#[test]
fn typed_project_examples_temp_tree_directory_collision_preserves_sentinel_after_drop() {
    let parent = TempTree::new();
    let collision = parent.0.join("collision");
    fs::create_dir(&collision).unwrap();
    let sentinel = collision.join("sentinel");
    fs::write(&sentinel, b"keep this directory unchanged\n").unwrap();
    let mut names = ["collision", "fresh"].into_iter();
    let tree = TempTree::try_new_in(&parent.0, || names.next().unwrap().to_owned()).unwrap();
    assert_eq!(tree.0, parent.0.join("fresh"));
    let created = tree.0.clone();
    assert_eq!(
        fs::read(&sentinel).unwrap(),
        b"keep this directory unchanged\n"
    );
    drop(tree);
    assert!(!created.exists());
    assert!(collision.is_dir());
    assert_eq!(
        fs::read(&sentinel).unwrap(),
        b"keep this directory unchanged\n"
    );
}

#[cfg(unix)]
#[test]
fn typed_project_examples_temp_tree_symlink_collisions_preserve_links_and_target_after_drop() {
    use std::os::unix::fs::symlink;

    let parent = TempTree::new();
    let target = parent.0.join("target");
    fs::create_dir(&target).unwrap();
    let sentinel = target.join("sentinel");
    fs::write(&sentinel, b"keep this symlink target unchanged\n").unwrap();
    let linked = parent.0.join("linked");
    let dangling = parent.0.join("dangling");
    let absent_target = parent.0.join("absent-target");
    symlink(&target, &linked).unwrap();
    symlink(&absent_target, &dangling).unwrap();
    let mut names = ["linked", "dangling", "fresh"].into_iter();
    let tree = TempTree::try_new_in(&parent.0, || names.next().unwrap().to_owned()).unwrap();
    assert_eq!(tree.0, parent.0.join("fresh"));
    let created = tree.0.clone();
    drop(tree);
    assert!(!created.exists());
    assert!(fs::symlink_metadata(&linked)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(fs::symlink_metadata(&dangling)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_link(&linked).unwrap(), target);
    assert_eq!(fs::read_link(&dangling).unwrap(), absent_target);
    assert_eq!(
        fs::read(&sentinel).unwrap(),
        b"keep this symlink target unchanged\n"
    );
    assert!(!absent_target.exists());
}

#[test]
fn typed_project_examples_temp_tree_collision_retries_are_bounded_without_adoption() {
    let parent = TempTree::new();
    let collision = parent.0.join("collision");
    fs::create_dir(&collision).unwrap();
    let sentinel = collision.join("sentinel");
    fs::write(&sentinel, b"keep this exhausted candidate unchanged\n").unwrap();
    let mut attempts = 0;
    let error = TempTree::try_new_in(&parent.0, || {
        attempts += 1;
        "collision".to_owned()
    })
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(attempts, TempTree::CREATE_ATTEMPTS);
    assert!(collision.is_dir());
    assert_eq!(
        fs::read(&sentinel).unwrap(),
        b"keep this exhausted candidate unchanged\n"
    );
}

#[test]
fn typed_project_examples_temp_tree_non_collision_error_is_returned_without_retry() {
    let parent = TempTree::new();
    let missing_parent = parent.0.join("missing-parent");
    let mut attempts = 0;
    let error = TempTree::try_new_in(&missing_parent, || {
        attempts += 1;
        "candidate".to_owned()
    })
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(attempts, 1);
    assert!(!missing_parent.exists());
}
