//! Focused acceptance for the frozen `packages/utf8_stream` decoder (alias
//! `stream`) through two independent consumers: the pure contract project
//! `examples/projects/utf8_stream_contracts` (interpreter TEST, run once) and
//! the `oxtextstat` Process application in `examples/projects/oxtextstat`
//! (interpreter Process route, each case run once). Both consumers share the
//! ordinary-source helper `examples/projects/oxtextstat/core`, which also uses
//! `packages/i32_decimal`. Sources are copied into a fresh repo-shaped temp
//! tree; the in-tree projects are never built. No CHECK/Result warm-up runs.
//!
//! Expected reports are fixed literals, never computed by the decoder. Logical
//! chunk partitions are exercised by the pure contracts through the shared
//! helper. Process cases use regular-file stdin and one piped case; the child's
//! actual read_some boundaries are neither controlled nor recorded, so no
//! read-boundary placement is claimed. The earlier valid 2,161-byte input that
//! exhausted the unchanged interpreter fuel budget (E0601) is kept only as a
//! recorded limitation outside this target and is not executed here.
//! Not covered: i32 position/counter
//! overflow (70), buffer refusal (71), read errors, EINTR, short or zero-progress
//! writes; only the EPIPE write failure (74) is actually executed.
//! Interpreter only: no native/LLVM run.
//!
//! Set `OXID_UTF8_STREAM_EVIDENCE=<dir>` to retain command/stdout/stderr/exit
//! evidence of successful runs. A failing test keeps its fixture directory and
//! writes `evidence.txt` there.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    cell::RefCell,
    fmt::Write as _,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const CONTRACTS: &str = "examples/projects/utf8_stream_contracts";
const WORKSPACE: &str = "examples/projects/oxtextstat";
const EVIDENCE: &str = "OXID_UTF8_STREAM_EVIDENCE";

/// Source inputs copied into each fixture, relative to the repository root.
const SOURCES: [&str; 11] = [
    "packages/utf8_stream/oxid.toml",
    "packages/utf8_stream/src/lib.ox",
    "packages/i32_decimal/oxid.toml",
    "packages/i32_decimal/src/lib.ox",
    "examples/projects/utf8_stream_contracts/oxid.toml",
    "examples/projects/utf8_stream_contracts/src/main.ox",
    "examples/projects/oxtextstat/oxid.toml",
    "examples/projects/oxtextstat/app/oxid.toml",
    "examples/projects/oxtextstat/app/src/main.ox",
    "examples/projects/oxtextstat/core/oxid.toml",
    "examples/projects/oxtextstat/core/src/lib.ox",
];

/// Root `test_` declarations of the contract project, in source order.
const CASES: &[&str] = &[
    "test_ascii_and_nul_scalars",
    "test_two_byte_boundaries",
    "test_three_byte_boundaries",
    "test_four_byte_boundaries",
    "test_bom_noncharacters_and_combining",
    "test_mixed_sequence_events",
    "test_invalid_leads_and_stray_continuations",
    "test_second_byte_restrictions",
    "test_wrong_later_continuations",
    "test_incomplete_tails_at_eof",
    "test_premature_eof_cannot_be_repaired",
    "test_successful_finish_closes_stream",
    "test_consumer_fixed_reports",
    "test_consumer_rejections_emit_no_report",
    "test_consumer_every_two_chunk_split",
    "test_consumer_every_three_chunk_split",
    "test_consumer_bytewise_with_empty_chunks",
    "test_consumer_split_errors",
    "test_consumer_lifecycle_and_render_bounds",
];

/// Minimum Process vectors from the handoff: input, exit, exact stdout.
const VECTORS: &[(&str, &[u8], i32, &[u8])] = &[
    ("empty", b"", 0, b"0 0 0\n"),
    ("letter", b"\x41", 0, b"1 1 1\n"),
    ("lf", b"\x0a", 0, b"1 1 1\n"),
    ("cr", b"\x0d", 0, b"1 1 1\n"),
    ("crlf", b"\x0d\x0a", 0, b"2 2 1\n"),
    ("lf-lf", b"\x0a\x0a", 0, b"2 2 2\n"),
    ("unterminated", b"\x41\x0a\x42", 0, b"3 3 2\n"),
    ("terminated", b"\x41\x0a\x42\x0a", 0, b"4 4 2\n"),
    ("nul", b"\x00", 0, b"1 1 1\n"),
    ("bom", b"\xef\xbb\xbf", 0, b"3 1 1\n"),
    ("combining", b"\x65\xcc\x81", 0, b"3 2 1\n"),
    (
        "mixed",
        b"\x41\xc2\xa2\xe2\x82\xac\xf0\x9f\x98\x80\x0a",
        0,
        b"11 5 1\n",
    ),
    ("stray", b"\x80", 65, b""),
    ("wrong-third", b"\x41\xe2\x28", 65, b""),
    ("truncated", b"\x41\xf0\x90", 65, b""),
    ("surrogate", b"\xed\xa0\x80", 65, b""),
    ("above-max", b"\xf4\x90\x80\x80", 65, b""),
];

/// 12 bytes, 6 scalars (A, U+00A2, U+20AC, U+1F600, CR, LF), one LF.
const UNIT: &[u8; 12] = b"A\xc2\xa2\xe2\x82\xac\xf0\x9f\x98\x80\r\n";

fn repeated(count: usize, tail: &[u8]) -> Vec<u8> {
    let mut bytes = UNIT.repeat(count);
    bytes.extend_from_slice(tail);
    bytes
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::new();
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

struct Fixture {
    root: PathBuf,
    label: &'static str,
    log: RefCell<String>,
}
impl Fixture {
    fn new(label: &'static str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-utf8-stream-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        for path in SOURCES {
            let destination = root.join(path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(repository.join(path), destination).unwrap();
        }
        Self {
            root,
            label,
            log: RefCell::new(String::new()),
        }
    }

    fn command(&self, project: &str, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(args)
            .current_dir(self.root.join(project))
            .env("OXID_LLVM_BIN", self.root.join("absent-llvm"))
            .env("OXID_CACHE_DIR", self.root.join("cache"))
            .env("PATH", self.root.join("absent-path"))
            .env_remove("OXID_PATH");
        command
    }

    fn record(&self, case: &str, args: &[&str], stdin: &str, output: &Output) {
        let command = args.join(" ");
        let exit = output.status.code();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut log = self.log.borrow_mut();
        let _ = writeln!(log, "case: {case}");
        let _ = writeln!(log, "command: oxid {command}");
        let _ = writeln!(log, "stdin: {stdin}");
        let _ = writeln!(log, "exit: {exit:?}");
        let _ = writeln!(log, "stdout: {stdout:?}");
        let _ = writeln!(log, "stderr: {stderr:?}\n");
    }

    fn contracts_test(&self) -> Output {
        let args = ["test", "--manifest-path=oxid.toml", "--engine=interpreter"];
        let output = self
            .command(CONTRACTS, &args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        self.record("contracts-test", &args, "null", &output);
        output
    }

    fn app(&self) -> (Command, [&'static str; 5]) {
        let args = [
            "run",
            "--manifest-path=oxid.toml",
            "--package=oxtextstat",
            "--engine=interpreter",
            "--entry-mode=process",
        ];
        (self.command(WORKSPACE, &args), args)
    }

    /// Regular-file stdin; the child's read_some results are not observed.
    fn stream(&self, case: &str, input: &[u8]) -> Output {
        let path = self.root.join(format!(
            "stdin-{}.bin",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, input).unwrap();
        let (mut command, args) = self.app();
        let file = File::open(path).unwrap();
        let output = command.stdin(file).output().unwrap();
        let described = format!("file {} bytes {}", input.len(), hex(input));
        self.record(case, &args, &described, &output);
        output
    }

    /// Separate pipe writes, then EOF. Write segmentation does not guarantee
    /// corresponding child reads; the report is deferred until EOF.
    fn piped(&self, case: &str, segments: &[&[u8]]) -> Output {
        let (mut command, args) = self.app();
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        for segment in segments {
            stdin.write_all(segment).unwrap();
            stdin.flush().unwrap();
        }
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        let mut described = String::from("pipe segments");
        for segment in segments {
            described.push(' ');
            described.push_str(&hex(segment));
        }
        self.record(case, &args, &described, &output);
        output
    }

    /// Piped stdin: stdout's only read end is closed before any input is
    /// supplied, so the final report write fails with EPIPE.
    fn closed_stdout(&self, case: &str, input: &[u8]) -> Output {
        let (mut command, args) = self.app();
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stdout.take());
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(input).unwrap();
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        let described = format!("pipe {} (stdout reader closed first)", hex(input));
        self.record(case, &args, &described, &output);
        output
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let log = self.log.take();
        if std::thread::panicking() {
            let path = self.root.join("evidence.txt");
            let _ = fs::write(&path, log.as_bytes());
            let shown = path.display();
            eprintln!("{}: evidence retained at {shown}", self.label);
            return;
        }
        if let Some(directory) = std::env::var_os(EVIDENCE) {
            let directory = PathBuf::from(directory);
            let _ = fs::create_dir_all(&directory);
            let target = directory.join(format!("{}.txt", self.label));
            let _ = fs::write(target, log.as_bytes());
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_exact(label: &str, output: &Output, status: i32, stdout: &[u8]) {
    assert_eq!(output.status.code(), Some(status), "{label}: {output:?}");
    assert_eq!(output.stdout, stdout, "{label}: {output:?}");
    assert!(output.stderr.is_empty(), "{label}: {output:?}");
}

#[test]
fn utf8_stream_contracts_pass_every_named_case() {
    let f = Fixture::new("contracts");
    let tested = f.contracts_test();
    let mut expected = String::new();
    for case in CASES {
        expected.push_str(&format!("PASS {case}\n"));
    }
    let total = CASES.len();
    expected.push_str(&format!(
        "test result: {total} passed; 0 failed; {total} total\n"
    ));
    assert_exact("contracts test", &tested, 0, expected.as_bytes());
}

#[test]
fn oxtextstat_minimum_vectors() {
    let f = Fixture::new("process-vectors");
    for (label, input, status, stdout) in VECTORS {
        let output = f.stream(label, input);
        assert_exact(label, &output, *status, stdout);
    }
}

#[test]
fn oxtextstat_multi_read_size_inputs() {
    // Fixed independent counts: UNIT is 12 bytes / 6 scalars / 1 LF. With the
    // app's 64-byte buffer, offsets 64 and 128 fall inside a 3-byte and a
    // 4-byte scalar; the child's actual read results are not observed.
    let f = Fixture::new("process-multi-read");
    let valid = repeated(16, b"Z");
    assert_eq!(valid.len(), 193);
    let output = f.stream("valid-193", &valid);
    assert_exact("valid-193", &output, 0, b"193 97 17\n");
    let exact = UNIT.repeat(16);
    assert_eq!(exact.len(), 192);
    let output = f.stream("valid-192", &exact);
    assert_exact("valid-192", &output, 0, b"192 96 16\n");
    let mut middle = repeated(6, b"\xed\xa0\x80");
    middle.extend_from_slice(&UNIT.repeat(6));
    let rejected: [(&str, Vec<u8>); 4] = [
        ("late-wrong-third-194", repeated(16, b"\xe2\x28")),
        ("late-truncated-195", repeated(16, b"\xf0\x9f\x98")),
        ("late-stray-193", repeated(16, b"\x80")),
        ("surrogate-at-72", middle),
    ];
    for (label, input) in rejected {
        let output = f.stream(label, &input);
        assert_exact(label, &output, 65, b"");
    }
}

#[test]
fn oxtextstat_piped_segments_without_read_boundary_claim() {
    let f = Fixture::new("process-piped");
    let output = f.piped(
        "mixed-segments",
        &[b"A\xc2", b"\xa2\xe2\x82", b"\xac\xf0\x9f\x98\x80\n"],
    );
    assert_exact("mixed-segments", &output, 0, b"11 5 1\n");
    let output = f.piped("truncated-segments", &[b"A\n", b"\xf0\x9f"]);
    assert_exact("truncated-segments", &output, 65, b"");
}

#[test]
fn oxtextstat_closed_stdout_is_io_failure() {
    let f = Fixture::new("process-closed-stdout");
    let output = f.closed_stdout("closed-stdout", b"A\n");
    assert_exact("closed-stdout", &output, 74, b"");
}
