//! Focused acceptance for the frozen `packages/csv_records` reader (alias
//! `csv`) through two independent consumers: the pure contract project
//! `examples/projects/csv_records_contracts` (interpreter TEST, run once) and
//! the `oxcsvsum` Process application in `examples/projects/oxcsvsum`
//! (interpreter Process route, each case run once). Both consumers share the
//! ordinary-source helper `examples/projects/oxcsvsum/core`, which also uses
//! `packages/utf8_text` and `packages/i32_decimal`. Sources are copied into a
//! fresh repo-shaped temp tree; the in-tree projects are never built. No
//! CHECK/Result warm-up runs.
//!
//! Expected reports and statuses are fixed literals written from the handoff
//! and the package README, never computed by the code under test. Process
//! cases use regular-file stdin and one closed-stdout pipe; the child's actual
//! read_some boundaries are neither controlled nor recorded, so no read
//! partition is claimed. oxcsvsum is a complete-buffer program, not a
//! streaming one. Not covered: row-count overflow, capacity overflow (70),
//! allocation/memory refusal (71), read errors, EINTR, short or zero-progress
//! writes; only the EPIPE write failure (74) is actually executed.
//! Interpreter only: no native/LLVM run.
//!
//! Set `OXID_CSV_RECORDS_EVIDENCE=<dir>` to retain command/stdin/stdout/
//! stderr/exit evidence of successful runs. A failing test keeps its fixture
//! directory and writes `evidence.txt` there.
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
const CONTRACTS: &str = "examples/projects/csv_records_contracts";
const WORKSPACE: &str = "examples/projects/oxcsvsum";
const EVIDENCE: &str = "OXID_CSV_RECORDS_EVIDENCE";

/// Source inputs copied into each fixture, relative to the repository root.
const SOURCES: [&str; 13] = [
    "packages/csv_records/oxid.toml",
    "packages/csv_records/src/lib.ox",
    "packages/utf8_text/oxid.toml",
    "packages/utf8_text/src/lib.ox",
    "packages/i32_decimal/oxid.toml",
    "packages/i32_decimal/src/lib.ox",
    "examples/projects/csv_records_contracts/oxid.toml",
    "examples/projects/csv_records_contracts/src/main.ox",
    "examples/projects/oxcsvsum/oxid.toml",
    "examples/projects/oxcsvsum/app/oxid.toml",
    "examples/projects/oxcsvsum/app/src/main.ox",
    "examples/projects/oxcsvsum/core/oxid.toml",
    "examples/projects/oxcsvsum/core/src/lib.ox",
];

/// Root `test_` declarations of the contract project, in source order.
const CASES: &[&str] = &[
    "test_dialect_table",
    "test_crlf_and_trailing_comma_boundaries",
    "test_quoted_payload_bytes",
    "test_quote_runs_one_to_five",
    "test_malformed_offsets_are_sticky",
    "test_copy_destinations",
    "test_current_field_lifecycle",
    "test_metadata_cannot_select_or_size",
    "test_summary_fixed_reports",
    "test_summary_rejections_emit_no_report",
    "test_summary_lifecycle",
];

/// Process oracles from the handoff table: input, exit, exact stdout.
const HANDOFF: &[(&str, &[u8], i32, &[u8])] = &[
    ("empty", b"", 0, b"0 0\n"),
    ("single", b"a,1", 0, b"1 1\n"),
    ("two-records", b"a,1\nb,-2\n", 0, b"2 -1\n"),
    ("quoted-crlf", b"\"a,b\",\"+00012\"\r\nx,-2", 0, b"2 10\n"),
    ("quoted-lf-label", b"\"a\nb\",3\n", 0, b"1 3\n"),
    (
        "extremes",
        b"\"a\"\"b\",-2147483648\nx,2147483647",
        0,
        b"2 -1\n",
    ),
    ("empty-label", b",0", 0, b"1 0\n"),
    ("leading-zeroes", b"a,0000000000000000000001", 0, b"1 1\n"),
    ("blank-line", b"\n", 65, b""),
    ("label-only", b"a", 65, b""),
    ("trailing-comma", b"a,1,", 65, b""),
    ("empty-value", b"a,", 65, b""),
    ("spaced-value", b"a, 1", 65, b""),
    ("out-of-range", b"a,2147483648", 65, b""),
    ("late-unterminated", b"a,1\nb,\"2", 65, b""),
    ("sum-above-max", b"a,2147483647\nb,1", 70, b""),
    ("sum-below-min", b"a,-2147483648\nb,-1", 70, b""),
    ("high-then-back", b"a,2147483647\nb,1\nc,-1", 70, b""),
];

/// Additional fixed examples: Unicode and NUL labels, malformed UTF-8 and
/// late malformed CSV, plus small dialect/decimal edges.
const EXTRA: &[(&str, &[u8], i32, &[u8])] = &[
    (
        "unicode-labels",
        b"\"caf\xc3\xa9, \xe6\x9d\xb1\xe4\xba\xac\",7\n\xe2\x82\xac,-3",
        0,
        b"2 4\n",
    ),
    ("nul-labels", b"\x00,5\n\"\x00\",-1", 0, b"2 4\n"),
    ("bom-is-label-text", b"\xef\xbb\xbfa,1", 0, b"1 1\n"),
    ("quoted-negative", b"a,\"-5\"", 0, b"1 -5\n"),
    ("minimum", b"a,-2147483648", 0, b"1 -2147483648\n"),
    ("invalid-utf8-label", b"a\xff,1", 65, b""),
    ("truncated-utf8", b"\xe2\x82,1", 65, b""),
    ("late-malformed-csv", b"a,1\nb,2\nc\"d,3", 65, b""),
    ("decoded-quote-value", b"a,\"1\"\"2\"", 65, b""),
    ("header-row", b"name,value\nb,2", 65, b""),
    ("bare-cr", b"a,1\rb,2", 65, b""),
    ("blank-between", b"a,1\n\nb,2", 65, b""),
    ("three-fields", b"a,1,2", 65, b""),
    ("sign-only", b"a,-", 65, b""),
    ("trailing-space", b"a,1 ", 65, b""),
    ("low-then-back", b"a,-2147483648\nb,-1\nc,1", 70, b""),
];

/// 5 bytes, one record with value 25.
const UNIT: &[u8; 5] = b"k,25\n";

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
            "oxid-csv-records-{}-{}",
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
        let mut log = self.log.borrow_mut();
        let _ = writeln!(log, "case: {case}");
        let _ = writeln!(log, "command: oxid {command}");
        let _ = writeln!(log, "stdin: {stdin}");
        let _ = writeln!(log, "exit: {exit:?}");
        let _ = writeln!(log, "stdout hex: {}", hex(&output.stdout));
        let _ = writeln!(log, "stderr hex: {}\n", hex(&output.stderr));
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
            "--package=oxcsvsum",
            "--engine=interpreter",
            "--entry-mode=process",
        ];
        (self.command(WORKSPACE, &args), args)
    }

    /// Regular-file stdin; the child's read_some results are not observed.
    fn summarize(&self, case: &str, input: &[u8]) -> Output {
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
fn csv_records_contracts_pass_every_named_case() {
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
fn oxcsvsum_handoff_vectors() {
    let f = Fixture::new("process-handoff");
    for (label, input, status, stdout) in HANDOFF {
        let output = f.summarize(label, input);
        assert_exact(label, &output, *status, stdout);
    }
}

#[test]
fn oxcsvsum_additional_fixed_vectors() {
    let f = Fixture::new("process-extra");
    for (label, input, status, stdout) in EXTRA {
        let output = f.summarize(label, input);
        assert_exact(label, &output, *status, stdout);
    }
}

#[test]
fn oxcsvsum_multi_read_size_inputs() {
    // Fixed independent results: UNIT is one 5-byte record with value 25, so
    // 40 units are 200 bytes, 40 rows and total 40 * 25 = 1000. The inputs
    // exceed the app's 64-byte read buffer; actual read results are not
    // observed.
    let f = Fixture::new("process-multi-read");
    let valid = UNIT.repeat(40);
    assert_eq!(valid.len(), 200);
    let output = f.summarize("valid-200", &valid);
    assert_exact("valid-200", &output, 0, b"40 1000\n");
    let rejected: [(&str, Vec<u8>); 2] = [
        ("late-unterminated-204", repeated(40, b"x,\"1")),
        ("late-invalid-utf8-203", repeated(40, b"x,\xc3")),
    ];
    for (label, input) in rejected {
        let output = f.summarize(label, &input);
        assert_exact(label, &output, 65, b"");
    }
}

#[test]
fn oxcsvsum_closed_stdout_is_io_failure() {
    let f = Fixture::new("process-closed-stdout");
    let output = f.closed_stdout("closed-stdout", b"a,1");
    assert_exact("closed-stdout", &output, 74, b"");
}
