//! Independent explicit-manifest `oxid test --fail-fast` acceptance (task
//! r1-test-fail-fast-2dcbc8a1; companion to root's host CLI/runner patch).
//! Tests marked `#[ignore]` need LLVM 19.1.7: run them with
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test
//! manifest_test_fail_fast_acceptance -- --include-ignored`. Opting in
//! without that env var fails the LLVM gate; a missing tool is never a pass.
//!
//! Every expected output below is hand-written from miku's frozen contract
//! (ts 1792440998.868009) and the base 2dcbc8a1 sources; none is generated
//! from candidate output. Frozen here: one bare `--fail-fast` on explicit
//! manifest TEST (with `--filter`, `--package`, both engines); a value
//! (including empty) -> "--fail-fast does not accept a value"; a repeat ->
//! "--fail-fast may only be specified once"; first scanner error per tier,
//! global priority and the TEST JSON refusal unchanged; any other explicit
//! manifest operation -> "--fail-fast is only supported by explicit manifest
//! test", except that a valid `--filter` reports the existing filter
//! applicability first in both orders; PACKAGE keeps its failure summary.
//! Execution: whole project and every test signature checked first; native
//! prepares every selected artifact before any case runs; cases run in source
//! order and stop after the first false / non-zero / runtime diagnostic; the
//! unchanged summary counts only executed cases and is followed by
//! "test stopped: N selected tests not run (--fail-fast)\n" iff selected cases
//! remain; exit 1. All-pass and last-case failure print no stopped line. No
//! flag stays byte-identical. Prepare / infrastructure failures print no
//! summary. Legacy, script and `--` forms are pinned to the base 2dcbc8a1
//! binary bytes. No qualification, held, capacity or fuel probes.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

type FileSnapshot = Option<(u64, u64, Vec<u8>)>;
/// A suite case: root test name and whether it passes.
type Case = (&'static str, bool);

static NEXT: AtomicUsize = AtomicUsize::new(0);
const APP: &str = "failfast-app";
const HELPER: &str = "failfast-helper";
const GREEN: &str = "failfast-green";
const TAIL: &str = "failfast-tail";
const EXAMPLE: &str = "examples/projects/manifest_test_fail_fast_acceptance";
const ROOT_MANIFEST: &str = "[workspace]\nmembers = [\"app\", \"helper\", \"green\", \"tail\"]\n";
const APP_MANIFEST: &str = "[project]\nname = \"failfast-app\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\n[dependencies]\nhelper = \"../helper\"\n";
/// The fixed suite: pass -> false -> pass -> divide-by-zero -> unit.
const APP_MAIN: &str = "mod child;\nuse helper::seven;\nuse helper::nothing;\nfn main() -> i32 {\n    return seven();\n}\nfn test_early_pass() -> bool {\n    return seven() == 7;\n}\nfn test_early_false() -> bool {\n    return seven() == 8;\n}\nfn test_mid_pass() -> i32 {\n    return nothing();\n}\nfn test_late_divide() -> i32 {\n    return seven() / nothing();\n}\nfn test_late_unit() -> () {\n    return;\n}\n";
const APP_CHILD: &str = "pub fn test_child_false() -> bool {\n    return false;\n}\n";
const HELPER_MANIFEST: &str = "[project]\nname = \"failfast-helper\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/lib.ox\"\n";
const HELPER_LIB: &str = "pub fn seven() -> i32 {\n    return 7;\n}\npub fn nothing() -> i32 {\n    return 0;\n}\nfn main() -> i32 {\n    return 91;\n}\nfn test_helper_false() -> bool {\n    return seven() == 8;\n}\n";
const GREEN_MANIFEST: &str = "[project]\nname = \"failfast-green\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\n[dependencies]\nhelper = \"../helper\"\n";
const GREEN_MAIN: &str = "use helper::seven;\nuse helper::nothing;\nfn main() -> i32 {\n    return nothing();\n}\nfn test_green_bool() -> bool {\n    return seven() == 7;\n}\nfn test_green_zero() -> i32 {\n    return nothing();\n}\nfn test_green_unit() -> () {\n    return;\n}\n";
const TAIL_MANIFEST: &str = "[project]\nname = \"failfast-tail\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\n[dependencies]\nhelper = \"../helper\"\n";
const TAIL_MAIN: &str = "use helper::seven;\nfn main() -> i32 {\n    return 0;\n}\nfn test_tail_pass() -> bool {\n    return seven() == 7;\n}\nfn test_tail_unit() -> () {\n    return;\n}\nfn test_tail_nonzero() -> i32 {\n    return seven() - 4;\n}\n";
/// Tiny side-effect-free legacy program (never the repository's old suite).
const LEGACY_PROBE: &str = "fn main() {\n    print \"legacy fail-fast probe\";\n}\n";
const FILES: [(&str, &str); 11] = [
    ("oxid.toml", ROOT_MANIFEST),
    ("app/oxid.toml", APP_MANIFEST),
    ("app/src/main.ox", APP_MAIN),
    ("app/src/child.ox", APP_CHILD),
    ("helper/oxid.toml", HELPER_MANIFEST),
    ("helper/src/lib.ox", HELPER_LIB),
    ("green/oxid.toml", GREEN_MANIFEST),
    ("green/src/main.ox", GREEN_MAIN),
    ("tail/oxid.toml", TAIL_MANIFEST),
    ("tail/src/main.ox", TAIL_MAIN),
    ("legacy/tests/fail_fast_probe.ox", LEGACY_PROBE),
];

/// The app suite in source order with each case's result (`true` = PASS).
/// `main`, the child's `test_child_false` and the dependency's
/// `test_helper_false` are never cases of the app suite.
const SUITE: [Case; 5] = [
    ("test_early_pass", true),
    ("test_early_false", false),
    ("test_mid_pass", true),
    ("test_late_divide", false),
    ("test_late_unit", true),
];
const GREEN_SUITE_CASES: [Case; 3] = [
    ("test_green_bool", true),
    ("test_green_zero", true),
    ("test_green_unit", true),
];
const TAIL_SUITE_CASES: [Case; 3] = [
    ("test_tail_pass", true),
    ("test_tail_unit", true),
    ("test_tail_nonzero", false),
];

// ---- Hand-written oracle (literal bytes, written from the contract) ----

/// No flag: the existing output, byte-identical.
const DEFAULT_APP: &[u8] = b"PASS test_early_pass\nFAIL test_early_false\nPASS test_mid_pass\nFAIL test_late_divide\nPASS test_late_unit\ntest result: 3 passed; 2 failed; 5 total\n";
/// `--fail-fast`: stops after the first failure (case 2 of 5).
const FAIL_FAST_APP: &[u8] = b"PASS test_early_pass\nFAIL test_early_false\ntest result: 1 passed; 1 failed; 2 total\ntest stopped: 3 selected tests not run (--fail-fast)\n";
/// `--filter late` without the flag (existing filter behavior).
const FILTER_LATE: &[u8] =
    b"FAIL test_late_divide\nPASS test_late_unit\ntest result: 1 passed; 1 failed; 2 total\n";
/// `--filter late --fail-fast`: the runtime failure stops before the unit case.
const FILTER_LATE_FAIL_FAST: &[u8] = b"FAIL test_late_divide\ntest result: 0 passed; 1 failed; 1 total\ntest stopped: 1 selected tests not run (--fail-fast)\n";
/// `--filter l --fail-fast` (selects ordinals 0, 1, 3, 4).
const FILTER_L_FAIL_FAST: &[u8] = b"PASS test_early_pass\nFAIL test_early_false\ntest result: 1 passed; 1 failed; 2 total\ntest stopped: 2 selected tests not run (--fail-fast)\n";
/// `--filter early --fail-fast`: the last selected case fails, no stopped line.
const FILTER_EARLY: &[u8] =
    b"PASS test_early_pass\nFAIL test_early_false\ntest result: 1 passed; 1 failed; 2 total\n";
/// `--filter divide --fail-fast`: the only (last) selected case has a runtime
/// diagnostic, no stopped line.
const FILTER_DIVIDE: &[u8] = b"FAIL test_late_divide\ntest result: 0 passed; 1 failed; 1 total\n";
/// `--filter pass --fail-fast`: all selected pass, exit 0, no stopped line.
const FILTER_PASS: &[u8] =
    b"PASS test_early_pass\nPASS test_mid_pass\ntest result: 2 passed; 0 failed; 2 total\n";
/// `--filter unit --fail-fast`.
const FILTER_UNIT: &[u8] = b"PASS test_late_unit\ntest result: 1 passed; 0 failed; 1 total\n";
/// All-pass suite (with or without the flag): exit 0, no stopped line.
const GREEN_SUITE: &[u8] = b"PASS test_green_bool\nPASS test_green_zero\nPASS test_green_unit\ntest result: 3 passed; 0 failed; 3 total\n";
/// Last case fails with a non-zero i32 (with or without the flag): exit 1,
/// no stopped line.
const TAIL_SUITE: &[u8] = b"PASS test_tail_pass\nPASS test_tail_unit\nFAIL test_tail_nonzero\ntest result: 2 passed; 1 failed; 3 total\n";
/// The dependency's own one-case suite (the only case is the last case).
const HELPER_SUITE: &[u8] = b"FAIL test_helper_false\ntest result: 0 passed; 1 failed; 1 total\n";
/// After the path dependency changes seven() to 8, the first case fails.
const CHANGED_FAIL_FAST: &[u8] = b"FAIL test_early_pass\ntest result: 0 passed; 1 failed; 1 total\ntest stopped: 4 selected tests not run (--fail-fast)\n";
const CHANGED_FALSE: &[u8] = b"PASS test_early_false\ntest result: 1 passed; 0 failed; 1 total\n";

/// Runtime diagnostic of test_late_divide; its absence proves the case did not run.
const DIV_ZERO: &str = "E0607";
const STOPPED: &str = "test stopped:";
const WORKSPACE_REQUIRES_PACKAGE: &str = "workspace requires --package; available packages:";
/// A manifest path that never exists: refusals must precede reading it.
const MISSING: &str = "absent/oxid.toml";
const NO_MATCH: &[u8] = b"error[E0800] (project-test): no tests match --filter\n";
const NO_VALUE: &str = "--fail-fast does not accept a value";
const DUPLICATE: &str = "--fail-fast may only be specified once";
const ONLY_TEST: &str = "--fail-fast is only supported by explicit manifest test";
const FILTER_ONLY_TEST: &str = "--filter is only supported by explicit manifest test";
const FILTER_REQUIRES_VALUE: &str = "--filter requires a value";
const FILTER_DUPLICATE: &str = "--filter may only be specified once";
const OPERAND: &str = "manifest test does not accept a source operand";
const SIGNATURE: &str = "test_ functions require zero parameters and return bool, i32 or ()";
/// Existing stdout line of every refused manifest package invocation.
const PACKAGE_FAILED: &[u8] =
    b"typed-preview native package failed: managed executable not published; archive not published\n";

// Byte-exact baselines captured once with the base binary 2dcbc8a1 (sha256
// 4c4e8da1321d9742704d1bcbc63044cfd72968f3e78ca669fdbd0d972e7a256e) in
// `legacy/` with OXID_LLVM_BIN/PATH pointing at absent directories.
/// Legacy `oxid test` (each form in `LEGACY_SUITE_FORMS`): exit 0, empty stderr.
const LEGACY_SUITE: &[u8] = b"test: ./tests/fail_fast_probe.ox\nlegacy fail-fast probe\n";
const LEGACY_SUITE_FORMS: [&[&str]; 7] = [
    &["test"],
    &["test", "--fail-fast"],
    &["test", "--fail-fast="],
    &["test", "--fail-fast=true"],
    &["test", "--fail-fast", "--fail-fast"],
    &["test", "--", "--fail-fast"],
    &["test", "--fail-fast", "--"],
];
/// Legacy `oxid test --fail-fast --filter keep` / `--filter=keep --fail-fast`:
/// exit 1, empty stdout.
const LEGACY_FILTER: &[u8] =
    b"error[E0001] (cli): --filter is only supported by explicit manifest test\n";
/// `oxid script task --fail-fast --manifest-path=p`: exit 1, empty stdout.
const LEGACY_SCRIPT: &[u8] =
    b"error: cannot read manifest: ./oxid.toml (No such file or directory (os error 2))\n";
/// `oxid check --edition=typed-preview -- --fail-fast`: exit 1, empty stdout.
const LEGACY_CHECK_SEPARATED: &[u8] =
    b"error[E0002] (source): cannot read file --fail-fast: No such file or directory (os error 2)\n";
/// `oxid test --manifest-path=p -- --fail-fast`: exit 1, empty stdout.
const MANIFEST_SEPARATED: &[u8] =
    b"error[E0001] (cli): manifest test does not accept a source operand\n";

fn llvm_bin() -> PathBuf {
    let dir = PathBuf::from(
        std::env::var_os("OXID_TEST_LLVM_BIN")
            .expect("native fail-fast acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin)"),
    );
    for (tool, marker) in [
        ("clang", "clang version 19.1.7"),
        ("opt", "LLVM version 19.1.7"),
        ("ld.lld", "LLD 19.1.7"),
    ] {
        let version = Command::new(dir.join(tool))
            .arg("--version")
            .output()
            .unwrap_or_else(|e| panic!("missing native tool {tool}: {e}"));
        assert!(
            String::from_utf8_lossy(&version.stdout).contains(marker),
            "{tool}: {version:?}"
        );
    }
    dir
}

struct Fixture {
    root: PathBuf,
    llvm: PathBuf,
}
impl Fixture {
    /// Copies the checked-in example to `<root>/ex` (never run in-tree).
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-test-fail-fast-accept-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
        };
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE);
        for (file, expected) in FILES {
            let text = fs::read_to_string(example.join(file)).unwrap();
            assert_eq!(text, expected, "checked-in example {file} changed");
            fixture.write(&format!("ex/{file}"), &text);
        }
        fixture
    }
    fn native() -> Self {
        Self::new(Some(llvm_bin()))
    }
    fn path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.path(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn append(&self, path: &str, text: &str) {
        let mut body = fs::read_to_string(self.path(path)).unwrap();
        body.push_str(text);
        self.write(path, &body);
    }
    fn reset(&self) {
        for (file, text) in FILES {
            self.write(&format!("ex/{file}"), text);
        }
    }
    fn oxid_in(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(cwd)
            .env("OXID_LLVM_BIN", &self.llvm)
            .env("OXID_CACHE_DIR", self.path("cache"))
            .env("PATH", self.path("absent-path"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn oxid(&self, args: &[&str]) -> Output {
        self.oxid_in(&self.root, args)
    }
    fn legacy(&self, args: &[&str]) -> Output {
        self.oxid_in(&self.path("ex/legacy"), args)
    }
    fn run_with(&self, command: &str, manifest: &str, extra: &[&str]) -> Output {
        let mut args = vec![command, "--manifest-path", manifest];
        args.extend_from_slice(extra);
        self.oxid(&args)
    }
    /// `oxid test --manifest-path ex/app/oxid.toml <extra>` (single package
    /// with a path dependency on the helper).
    fn app(&self, extra: &[&str]) -> Output {
        self.run_with("test", "ex/app/oxid.toml", extra)
    }
    /// `oxid <command> --manifest-path ex/oxid.toml <extra>` (virtual workspace).
    fn ws(&self, command: &str, extra: &[&str]) -> Output {
        self.run_with(command, "ex/oxid.toml", extra)
    }
    /// `oxid test --manifest-path ex/oxid.toml --package <package> <extra>`.
    fn member(&self, package: &str, extra: &[&str]) -> Output {
        let mut args = vec!["--package", package];
        args.extend_from_slice(extra);
        self.ws("test", &args)
    }
    fn test_dir(&self, member: &str) -> PathBuf {
        self.path(&format!("ex/{member}/.oxid/typed-preview/test"))
    }
    fn app_out(&self, mode: &str) -> PathBuf {
        self.path(&format!("ex/app/.oxid/typed-preview/{mode}/{APP}"))
    }
    fn app_state(&self) -> Vec<[FileSnapshot; 4]> {
        ["result", "process"]
            .iter()
            .map(|mode| {
                let artifact = self.app_out(mode);
                let state = artifact.parent().unwrap().join(format!(".{APP}.build"));
                [
                    artifact,
                    state.join("owner"),
                    state.join("current"),
                    state.join("next"),
                ]
                .map(|path| optional_identity(&path))
            })
            .collect()
    }
    /// Interpreter routes and refusals write no managed output anywhere.
    fn no_managed_output(&self) {
        for dir in [
            "",
            "ex",
            "ex/app",
            "ex/helper",
            "ex/green",
            "ex/tail",
            "ex/legacy",
        ] {
            assert!(!self.path(dir).join(".oxid").exists(), "{dir}/.oxid");
            assert!(!self.path(dir).join("oxid.lock").exists(), "{dir}");
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}
fn optional_identity(path: &Path) -> FileSnapshot {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            assert!(meta.file_type().is_file(), "{path:?}");
            Some((meta.dev(), meta.ino(), fs::read(path).unwrap()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("cannot snapshot {path:?}: {error}"),
    }
}
/// Contract model, used ONLY to cross-check the literal oracle above (never
/// to produce an expectation): run in order, stop after the first failure,
/// count executed cases, then the stopped line iff selected cases remain.
fn model(cases: &[Case], fail_fast: bool) -> Vec<u8> {
    let mut out = String::new();
    let (mut passed, mut executed) = (0, 0);
    for (name, pass) in cases {
        executed += 1;
        passed += usize::from(*pass);
        out.push_str(&format!("{} {name}\n", if *pass { "PASS" } else { "FAIL" }));
        if fail_fast && !pass {
            break;
        }
    }
    out.push_str(&format!(
        "test result: {passed} passed; {} failed; {executed} total\n",
        executed - passed
    ));
    if executed < cases.len() {
        out.push_str(&format!(
            "test stopped: {} selected tests not run (--fail-fast)\n",
            cases.len() - executed
        ));
    }
    out.into_bytes()
}
fn select(filter: &str) -> Vec<Case> {
    SUITE
        .iter()
        .copied()
        .filter(|(name, _)| name.contains(filter))
        .collect()
}

/// A run whose stdout must equal `expected` exactly, with the frozen status
/// (0 iff the summary reports no failure) and the division diagnostic on
/// stderr iff test_late_divide appears in stdout; otherwise stderr is empty.
fn exact(out: &Output, expected: &[u8]) {
    assert_eq!(text(&out.stdout), text(expected), "{out:?}");
    let failed = !text(expected).contains(" 0 failed;");
    assert_eq!(out.status.code(), Some(i32::from(failed)), "{out:?}");
    let ran_division = text(expected).contains(" test_late_divide\n");
    let err = text(&out.stderr);
    assert_eq!(
        err.matches(DIV_ZERO).count(),
        usize::from(ran_division),
        "{out:?}"
    );
    if !ran_division {
        assert!(out.stderr.is_empty(), "{out:?}");
    }
}
/// A refusal on the existing CLI (E0001) path before project I/O: exactly one
/// text diagnostic whose message is one of `allowed`, the operation's
/// existing refusal stdout, never the missing-manifest read error or a case.
fn cli_refusal(out: &Output, operation: &str, allowed: &[&str]) {
    assert_eq!(out.status.code(), Some(1), "{operation}: {out:?}");
    let stdout: &[u8] = if operation == "package" {
        PACKAGE_FAILED
    } else {
        b""
    };
    assert_eq!(out.stdout, stdout, "{operation}: {out:?}");
    let err = text(&out.stderr);
    assert!(
        allowed
            .iter()
            .any(|message| err == format!("error[E0001] (cli): {message}\n")),
        "{operation} expected one of {allowed:?}: {out:?}"
    );
}
/// An existing preflight refusal (signature, discovery, type or toolchain
/// error): no case line, no summary, no stopped line, no case executed.
fn refusal(out: &Output, message: &str) {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    let err = text(&out.stderr);
    assert!(err.contains(message), "{out:?}");
    assert!(!err.contains(DIV_ZERO), "{out:?}");
    assert!(!err.contains(STOPPED), "{out:?}");
}
fn with(prefix: &[&'static str], rest: &[&'static str]) -> Vec<&'static str> {
    let mut args = prefix.to_vec();
    args.extend_from_slice(rest);
    args
}

/// Bare flag spellings (all interpreter); each must behave identically.
const BARE: [&[&str]; 4] = [
    &["--fail-fast"],
    &["--fail-fast", "--engine", "interpreter"],
    &["--engine=interpreter", "--fail-fast"],
    &["--engine", "interpreter", "--fail-fast"],
];
/// Malformed forms with the frozen refusal message. The first local scanner
/// error wins; on the repeated token itself, the duplicate precedes the value
/// (as for --filter: `--filter=keep --filter=` is a duplicate).
const MALFORMED: [(&[&str], &str); 18] = [
    (&["--fail-fast="], NO_VALUE),
    (&["--fail-fast=true"], NO_VALUE),
    (&["--fail-fast=false"], NO_VALUE),
    (&["--fail-fast=1"], NO_VALUE),
    (&["--fail-fast=late"], NO_VALUE),
    (&["--fail-fast=", "--filter", "late"], NO_VALUE),
    (&["--filter=late", "--fail-fast="], NO_VALUE),
    (&["--fail-fast=", "--fail-fast"], NO_VALUE),
    (&["--fail-fast", "--fail-fast"], DUPLICATE),
    (&["--fail-fast", "--fail-fast="], DUPLICATE),
    (&["--fail-fast", "--filter=late", "--fail-fast"], DUPLICATE),
    (
        &["--fail-fast", "--engine", "interpreter", "--fail-fast"],
        DUPLICATE,
    ),
    // Other local scanner errors keep first-error-wins within the tier.
    (&["--fail-fast=1", "--engine", "bogus"], NO_VALUE),
    (
        &["--engine", "bogus", "--fail-fast=1"],
        "unknown --engine `bogus`; expected interpreter or native",
    ),
    (&["--filter=", "--fail-fast=1"], FILTER_REQUIRES_VALUE),
    (&["--fail-fast=1", "--filter="], NO_VALUE),
    (
        &["--filter=a", "--filter=b", "--fail-fast=1"],
        FILTER_DUPLICATE,
    ),
    // A bare --fail-fast never swallows the next option.
    (&["--filter", "--fail-fast"], FILTER_REQUIRES_VALUE),
];

// ---- Oracle self-consistency (no binary executed) ----

#[test]
fn literal_oracle_agrees_with_contract_model() {
    assert_eq!(model(&SUITE, false), DEFAULT_APP);
    assert_eq!(model(&SUITE, true), FAIL_FAST_APP);
    assert_eq!(model(&select("late"), false), FILTER_LATE);
    assert_eq!(model(&select("late"), true), FILTER_LATE_FAIL_FAST);
    assert_eq!(model(&select("l"), true), FILTER_L_FAIL_FAST);
    assert_eq!(model(&select("early"), true), FILTER_EARLY);
    assert_eq!(model(&select("divide"), true), FILTER_DIVIDE);
    assert_eq!(model(&select("pass"), true), FILTER_PASS);
    assert_eq!(model(&select("unit"), true), FILTER_UNIT);
    assert_eq!(model(&select("test_"), true), FAIL_FAST_APP);
    assert!(select("zzz").is_empty());
    for fail_fast in [false, true] {
        assert_eq!(model(&GREEN_SUITE_CASES, fail_fast), GREEN_SUITE);
        assert_eq!(model(&TAIL_SUITE_CASES, fail_fast), TAIL_SUITE);
        assert_eq!(
            model(&[("test_helper_false", false)], fail_fast),
            HELPER_SUITE
        );
    }
    let changed = [
        ("test_early_pass", false),
        ("test_early_false", true),
        ("test_mid_pass", true),
        ("test_late_divide", false),
        ("test_late_unit", true),
    ];
    assert_eq!(model(&changed, true), CHANGED_FAIL_FAST);
    assert_eq!(model(&changed[1..2], true), CHANGED_FALSE);
    // Legacy forms are distinct argv spellings.
    for (index, form) in LEGACY_SUITE_FORMS.iter().enumerate() {
        assert!(!LEGACY_SUITE_FORMS[..index].contains(form), "{form:?}");
    }
}

// ---- Interpreter / CLI contracts (no toolchain required) ----

#[test]
fn interpreter_no_flag_output_unchanged() {
    let f = Fixture::new(None);
    for extra in [
        &[][..],
        &["--engine", "interpreter"],
        &["--engine=interpreter"],
    ] {
        exact(&f.app(extra), DEFAULT_APP);
        exact(&f.member(APP, extra), DEFAULT_APP);
    }
    exact(&f.app(&["--filter", "late"]), FILTER_LATE);
    exact(&f.member(GREEN, &[]), GREEN_SUITE);
    exact(&f.member(TAIL, &[]), TAIL_SUITE);
    exact(&f.member(HELPER, &[]), HELPER_SUITE);
    let run = f.run_with("run", "ex/app/oxid.toml", &[]);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert_eq!(run.stdout, b"7\n");
    assert!(run.stderr.is_empty(), "{run:?}");
    f.no_managed_output();
}

#[test]
fn interpreter_fail_fast_stops_after_first_failure_in_source_order() {
    let f = Fixture::new(None);
    for form in BARE {
        exact(&f.app(form), FAIL_FAST_APP);
        exact(&f.member(APP, form), FAIL_FAST_APP);
    }
    // --package and the flag in either order; flag before --manifest-path.
    exact(
        &f.ws("test", &["--fail-fast", "--package", APP]),
        FAIL_FAST_APP,
    );
    exact(
        &f.ws("test", &["--package=failfast-app", "--fail-fast"]),
        FAIL_FAST_APP,
    );
    exact(
        &f.oxid(&["test", "--fail-fast", "--manifest-path", "ex/app/oxid.toml"]),
        FAIL_FAST_APP,
    );
    exact(
        &f.oxid(&["test", "--fail-fast", "--manifest-path=ex/app/oxid.toml"]),
        FAIL_FAST_APP,
    );
    f.no_managed_output();
}

#[test]
fn interpreter_fail_fast_with_filter_counts_only_executed_cases() {
    let f = Fixture::new(None);
    let table: [(&[&str], &[u8]); 12] = [
        (&["--filter", "late", "--fail-fast"], FILTER_LATE_FAIL_FAST),
        (&["--fail-fast", "--filter=late"], FILTER_LATE_FAIL_FAST),
        (
            &["--filter", "late", "--fail-fast", "--engine", "interpreter"],
            FILTER_LATE_FAIL_FAST,
        ),
        (&["--fail-fast", "--filter", "l"], FILTER_L_FAIL_FAST),
        (&["--fail-fast", "--filter", "early"], FILTER_EARLY),
        (&["--filter=divide", "--fail-fast"], FILTER_DIVIDE),
        (&["--fail-fast", "--filter", "pass"], FILTER_PASS),
        (&["--fail-fast", "--filter", "unit"], FILTER_UNIT),
        (&["--fail-fast", "--filter", "test_"], FAIL_FAST_APP),
        (&["--filter=test_", "--fail-fast"], FAIL_FAST_APP),
        (&["--fail-fast", "--filter=_late_"], FILTER_LATE_FAIL_FAST),
        (
            &["--fail-fast", "--filter", "mid"],
            b"PASS test_mid_pass\ntest result: 1 passed; 0 failed; 1 total\n",
        ),
    ];
    for (form, expected) in table {
        exact(&f.app(form), expected);
        exact(&f.member(APP, form), expected);
    }
    // The failing case's own runtime diagnostic is unchanged by the flag.
    let plain = f.app(&["--filter", "divide"]);
    exact(&plain, FILTER_DIVIDE);
    for form in [
        &["--filter", "late", "--fail-fast"][..],
        &["--filter", "divide", "--fail-fast"],
    ] {
        assert_eq!(f.app(form).stderr, plain.stderr, "{form:?}");
    }
    assert_eq!(f.app(&["--filter", "late"]).stderr, plain.stderr);
    // No match keeps the existing E0800 refusal: no summary, no stopped line.
    for form in [
        &["--fail-fast", "--filter", "zzz"][..],
        &["--filter=test_child_false", "--fail-fast"],
        &["--filter=test_helper_false", "--fail-fast"],
    ] {
        let out = f.app(form);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stdout.is_empty(), "{out:?}");
        assert_eq!(out.stderr, NO_MATCH, "{out:?}");
    }
    f.no_managed_output();
}

#[test]
fn interpreter_all_pass_and_last_case_failure_print_no_stopped_line() {
    let f = Fixture::new(None);
    for form in BARE {
        exact(&f.member(GREEN, form), GREEN_SUITE);
        exact(&f.member(TAIL, form), TAIL_SUITE);
        exact(&f.member(HELPER, form), HELPER_SUITE);
        exact(&f.run_with("test", "ex/green/oxid.toml", form), GREEN_SUITE);
        exact(&f.run_with("test", "ex/tail/oxid.toml", form), TAIL_SUITE);
    }
    f.no_managed_output();
}

#[test]
fn fail_fast_uses_a_fresh_path_dependency_session() {
    let f = Fixture::new(None);
    f.write(
        "ex/helper/src/lib.ox",
        &HELPER_LIB.replacen("return 7;", "return 8;", 1),
    );
    exact(&f.app(&["--fail-fast"]), CHANGED_FAIL_FAST);
    exact(&f.member(APP, &["--fail-fast"]), CHANGED_FAIL_FAST);
    exact(&f.app(&["--fail-fast", "--filter", "false"]), CHANGED_FALSE);
    f.no_managed_output();
}

#[test]
fn fail_fast_never_skips_whole_project_or_signature_validation() {
    // Every variant breaks only code that runs after the first failure (or
    // that the filter does not select); the whole suite is still refused
    // before the first case, on both engines (native refuses in preflight
    // before reaching the absent toolchain).
    let variants: [(&str, &str, &str); 7] = [
        (
            "ex/app/src/main.ox",
            "fn test_zz_bad(x: i32) -> i32 {\n    return x;\n}\n",
            SIGNATURE,
        ),
        (
            "ex/app/src/main.ox",
            "fn test_zz_flag(flag: bool) -> bool {\n    return flag;\n}\n",
            SIGNATURE,
        ),
        (
            "ex/app/src/main.ox",
            "fn test_zz_typed() -> bool {\n    return 1;\n}\n",
            "E0300",
        ),
        (
            "ex/app/src/main.ox",
            "fn unused() -> i32 {\n    return true;\n}\n",
            "E0300",
        ),
        (
            "ex/app/src/child.ox",
            "pub fn unused() -> i32 {\n    return true;\n}\n",
            "E0300",
        ),
        (
            "ex/helper/src/lib.ox",
            "pub fn unused() -> i32 {\n    return true;\n}\n",
            "E0300",
        ),
        (
            "ex/helper/src/lib.ox",
            "fn test_zz_helper() -> bool {\n    return 1;\n}\n",
            "E0300",
        ),
    ];
    let f = Fixture::new(None);
    for (file, extra, message) in variants {
        f.reset();
        f.append(file, extra);
        for form in [
            &["--fail-fast"][..],
            &["--fail-fast", "--filter", "early"],
            &["--filter=test_early_pass", "--fail-fast"],
            &["--fail-fast", "--filter", "zzz"],
        ] {
            for engine in ["interpreter", "native"] {
                let args = with(form, &["--engine", engine]);
                let out = f.app(&args);
                refusal(&out, message);
                assert!(!text(&out.stderr).contains("E0701"), "{out:?}");
                refusal(&f.member(APP, &args), message);
            }
        }
        f.no_managed_output();
    }
    // A suite without any root test_ keeps the existing discovery refusal.
    f.reset();
    f.write(
        "ex/app/src/main.ox",
        "fn main() -> i32 {\n    return 1;\n}\nfn helper() -> i32 {\n    return 0;\n}\n",
    );
    for engine in ["interpreter", "native"] {
        refusal(
            &f.app(&["--fail-fast", "--engine", engine]),
            "no root test_ function declarations found",
        );
    }
    f.no_managed_output();
}

#[test]
fn malformed_fail_fast_is_refused_before_reading_missing_project() {
    let f = Fixture::new(None);
    for (form, message) in MALFORMED {
        cli_refusal(&f.run_with("test", MISSING, form), "test", &[message]);
        // An existing workspace without --package: the refusal precedes
        // reading it (no package listing).
        cli_refusal(&f.ws("test", form), "test", &[message]);
        // A following --package is not swallowed.
        cli_refusal(
            &f.ws("test", &with(form, &["--package", APP])),
            "test",
            &[message],
        );
        cli_refusal(&f.app(form), "test", &[message]);
    }
    // A bare flag followed by a word: never a valid run (the word is either a
    // refused value or the existing source-operand refusal).
    for form in [
        &["--fail-fast", "true"][..],
        &["--fail-fast", "late"],
        &["--fail-fast", ""],
    ] {
        cli_refusal(
            &f.run_with("test", MISSING, form),
            "test",
            &[NO_VALUE, OPERAND],
        );
    }
    // A bare flag before `--`: the separated word stays a source operand.
    for form in [&["--fail-fast", "--", "x"][..], &["--", "--fail-fast"]] {
        cli_refusal(&f.run_with("test", MISSING, form), "test", &[OPERAND]);
        cli_refusal(&f.app(form), "test", &[OPERAND]);
    }
    assert!(!f.path("absent").exists());
    f.no_managed_output();
}

#[test]
fn fail_fast_on_other_manifest_operations_is_refused_before_project_io() {
    let f = Fixture::new(None);
    let forms: [(&[&str], &str); 10] = [
        (&["--fail-fast"], ONLY_TEST),
        // A valid --filter reports the existing filter applicability first,
        // in both orders.
        (&["--fail-fast", "--filter", "late"], FILTER_ONLY_TEST),
        (&["--filter", "late", "--fail-fast"], FILTER_ONLY_TEST),
        (&["--fail-fast", "--filter=late"], FILTER_ONLY_TEST),
        (&["--filter=late", "--fail-fast"], FILTER_ONLY_TEST),
        // Scanner errors still precede applicability.
        (&["--fail-fast="], NO_VALUE),
        (&["--fail-fast=true"], NO_VALUE),
        (&["--fail-fast", "--fail-fast"], DUPLICATE),
        (&["--fail-fast=", "--filter=late"], NO_VALUE),
        (&["--filter=", "--fail-fast"], FILTER_REQUIRES_VALUE),
    ];
    let operations: [(&str, &[&str]); 6] = [
        ("check", &[]),
        ("run", &[]),
        ("run", &["--engine", "native"]),
        ("build", &[]),
        ("build", &["--entry-mode=process"]),
        ("package", &["--output", "out.tar"]),
    ];
    for (operation, prefix) in operations {
        for (form, message) in forms {
            let args = with(prefix, form);
            cli_refusal(
                &f.run_with(operation, MISSING, &args),
                operation,
                &[message],
            );
            cli_refusal(
                &f.run_with(operation, "ex/app/oxid.toml", &args),
                operation,
                &[message],
            );
            let ws = with(&["--package", APP], &args);
            cli_refusal(&f.ws(operation, &ws), operation, &[message]);
        }
    }
    assert!(!f.path("out.tar").exists());
    f.no_managed_output();
}

#[test]
fn json_global_and_local_error_priority_unchanged_by_valid_fail_fast() {
    let f = Fixture::new(None);
    // A valid bare flag changes none of the existing refusals: byte-identical
    // to the same invocation without it (all precede reading the project).
    let cases: [&[&str]; 11] = [
        &["--message-format=json"],
        &["--message-format", "json", "--engine", "native"],
        &["--message-format=yaml"],
        &["--manifest-path", "other/oxid.toml"],
        &["--edition", "typed-preview", "--edition=legacy-0.9"],
        &["--engine", "bogus"],
        &["--entry-mode", "process"],
        &["--edition", "legacy-0.9"],
        &["--output", "x.tar"],
        &["--filter="],
        &["--filter=a", "--filter=b"],
    ];
    for existing in cases {
        let baseline = f.run_with("test", MISSING, existing);
        assert_eq!(baseline.status.code(), Some(1), "{baseline:?}");
        assert!(
            !text(&baseline.stderr).contains("cannot read"),
            "{baseline:?}"
        );
        for before in [true, false] {
            let args = if before {
                with(&["--fail-fast"], existing)
            } else {
                with(existing, &["--fail-fast"])
            };
            let out = f.run_with("test", MISSING, &args);
            assert_eq!(
                out.status.code(),
                baseline.status.code(),
                "{args:?}: {out:?}"
            );
            assert_eq!(out.stdout, baseline.stdout, "{args:?}: {out:?}");
            assert_eq!(out.stderr, baseline.stderr, "{args:?}: {out:?}");
        }
    }
    // The existing workspace package-selection refusal, unchanged.
    let baseline = f.ws("test", &[]);
    assert!(
        text(&baseline.stderr).contains(WORKSPACE_REQUIRES_PACKAGE),
        "{baseline:?}"
    );
    let out = f.ws("test", &["--fail-fast"]);
    assert_eq!(out.status.code(), baseline.status.code(), "{out:?}");
    assert_eq!(out.stdout, baseline.stdout, "{out:?}");
    assert_eq!(out.stderr, baseline.stderr, "{out:?}");
    // The existing TEST JSON refusal itself, unchanged with a valid flag.
    for args in [
        &["--message-format=json", "--fail-fast"][..],
        &["--fail-fast", "--message-format", "json", "--filter=late"],
    ] {
        let json = f.run_with("test", MISSING, args);
        assert_eq!(json.status.code(), Some(1), "{json:?}");
        assert!(json.stderr.is_empty(), "{json:?}");
        assert_eq!(
            text(&json.stdout),
            "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0001\",\"stage\":\"cli\",\"message\":\"manifest test does not support --message-format=json\",\"primary\":null,\"secondary\":[],\"notes\":[]}\n",
            "{json:?}"
        );
    }
    // Global-tier errors keep priority over a malformed flag.
    for (global, message) in [
        (
            &["--message-format=yaml"][..],
            "unknown --message-format `yaml`; expected text or json",
        ),
        (
            &["--manifest-path", "other/oxid.toml"][..],
            "--manifest-path may only be specified once",
        ),
        (
            &["--edition=bogus"][..],
            "unknown edition `bogus`; expected legacy-0.9 or typed-preview",
        ),
    ] {
        let baseline = f.run_with("test", MISSING, global);
        assert_eq!(
            text(&baseline.stderr),
            format!("error[E0001] (cli): {message}\n"),
            "{baseline:?}"
        );
        for malformed in [
            &["--fail-fast="][..],
            &["--fail-fast=true"],
            &["--fail-fast", "--fail-fast"],
        ] {
            for before in [true, false] {
                let args = if before {
                    with(malformed, global)
                } else {
                    with(global, malformed)
                };
                let out = f.run_with("test", MISSING, &args);
                assert_eq!(
                    out.status.code(),
                    baseline.status.code(),
                    "{args:?}: {out:?}"
                );
                assert_eq!(out.stdout, baseline.stdout, "{args:?}: {out:?}");
                assert_eq!(out.stderr, baseline.stderr, "{args:?}: {out:?}");
            }
        }
    }
    // JSON with a malformed flag: an ordinary local CLI error, which (as for
    // --filter) precedes the TEST JSON refusal and is rendered as one JSON
    // diagnostic; nothing is read.
    for (malformed, message) in [
        (&["--fail-fast="][..], NO_VALUE),
        (&["--fail-fast=1"], NO_VALUE),
        (&["--fail-fast", "--fail-fast"], DUPLICATE),
    ] {
        let args = with(&["--message-format=json"], malformed);
        let out = f.run_with("test", MISSING, &args);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stderr.is_empty(), "{out:?}");
        assert_eq!(
            text(&out.stdout),
            format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0001\",\"stage\":\"cli\",\"message\":\"{message}\",\"primary\":null,\"secondary\":[],\"notes\":[]}}\n"),
            "{out:?}"
        );
    }
    assert!(!f.path("x.tar").exists());
    f.no_managed_output();
}

#[test]
fn native_without_toolchain_fail_fast_prints_no_summary_or_fallback() {
    let f = Fixture::new(None);
    for mode in ["result", "process"] {
        let dir = f.path(&format!("ex/app/.oxid/typed-preview/{mode}"));
        fs::create_dir_all(dir.join(format!(".{APP}.build"))).unwrap();
        fs::write(dir.join(APP), "existing application").unwrap();
        fs::write(dir.join(format!(".{APP}.build/owner")), "existing receipt").unwrap();
    }
    let before = f.app_state();
    for form in [
        &["--fail-fast", "--engine", "native"][..],
        &["--engine=native", "--fail-fast", "--filter", "late"],
    ] {
        let out = f.app(form);
        refusal(&out, "E0701");
        assert_eq!(before, f.app_state());
        refusal(&f.member(APP, form), "E0701");
        assert_eq!(before, f.app_state());
    }
    assert!(!f.test_dir("app").join("case-0").exists());
    // No fallback: the interpreter still has to be asked for explicitly.
    exact(&f.app(&["--fail-fast"]), FAIL_FAST_APP);
}

// ---- Legacy (no --manifest-path), script and separator forms ----

#[test]
fn legacy_script_and_separator_forms_byte_identical_to_base() {
    let f = Fixture::new(None);
    for form in LEGACY_SUITE_FORMS {
        let out = f.legacy(form);
        assert_eq!(out.status.code(), Some(0), "{form:?}: {out:?}");
        assert_eq!(out.stdout, LEGACY_SUITE, "{form:?}: {out:?}");
        assert!(out.stderr.is_empty(), "{form:?}: {out:?}");
    }
    for form in [
        &["test", "--fail-fast", "--filter", "keep"][..],
        &["test", "--filter=keep", "--fail-fast"],
    ] {
        let out = f.legacy(form);
        assert_eq!(out.status.code(), Some(1), "{form:?}: {out:?}");
        assert!(out.stdout.is_empty(), "{form:?}: {out:?}");
        assert_eq!(out.stderr, LEGACY_FILTER, "{form:?}: {out:?}");
    }
    for (form, stderr) in [
        (
            &["script", "task", "--fail-fast", "--manifest-path=p"][..],
            LEGACY_SCRIPT,
        ),
        (
            &["check", "--edition=typed-preview", "--", "--fail-fast"],
            LEGACY_CHECK_SEPARATED,
        ),
        (
            &["test", "--manifest-path=p", "--", "--fail-fast"],
            MANIFEST_SEPARATED,
        ),
    ] {
        let out = f.legacy(form);
        assert_eq!(out.status.code(), Some(1), "{form:?}: {out:?}");
        assert!(out.stdout.is_empty(), "{form:?}: {out:?}");
        assert_eq!(out.stderr, stderr, "{form:?}: {out:?}");
    }
    assert_eq!(
        fs::read_to_string(f.path("ex/legacy/tests/fail_fast_probe.ox")).unwrap(),
        LEGACY_PROBE
    );
    f.no_managed_output();
}

#[test]
fn example_readme_documents_frozen_contract() {
    let readme = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(EXAMPLE)
            .join("README.md"),
    )
    .unwrap();
    for needle in [
        "--fail-fast",
        "--filter late",
        "--package failfast-app",
        "test result: 1 passed; 1 failed; 2 total",
        "test stopped: 3 selected tests not run (--fail-fast)",
        NO_VALUE,
        DUPLICATE,
        ONLY_TEST,
        "4c4e8da1321d9742704d1bcbc63044cfd72968f3e78ca669fdbd0d972e7a256e",
        "LLVM 19.1.7",
        "OXID_TEST_LLVM_BIN",
        "case-<ordinal>",
    ] {
        assert!(readme.contains(needle), "{needle}");
    }
}

#[test]
fn opt_in_llvm_gate_fails_without_toolchain_env() {
    if std::env::var_os("OXID_TEST_LLVM_BIN").is_some() {
        let _ = llvm_bin();
        return;
    }
    let result = std::panic::catch_unwind(|| {
        let _ = llvm_bin();
    });
    assert!(result.is_err(), "llvm_bin must require OXID_TEST_LLVM_BIN");
}

// ---- Native execution (LLVM 19.1.7) ----

fn native_args<'a>(form: &[&'a str]) -> Vec<&'a str> {
    let mut args = form.to_vec();
    args.extend_from_slice(&["--engine", "native"]);
    args
}
/// Runs a native test artifact directly (exit 0 = pass, 1 = fail).
fn standalone(artifact: &Path) -> Output {
    Command::new(artifact)
        .current_dir("/")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap()
}
/// Published ordinals of `member`'s test namespace (artifact and receipt).
fn published(f: &Fixture, member: &str, count: usize) -> Vec<usize> {
    let dir = f.test_dir(member);
    (0..count)
        .filter(|ordinal| {
            let artifact = dir.join(format!("case-{ordinal}")).is_file();
            let receipt = dir.join(format!(".case-{ordinal}.build/owner")).is_file();
            assert_eq!(artifact, receipt, "case-{ordinal}");
            artifact
        })
        .collect()
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_fail_fast_matches_hand_written_oracle() {
    let f = Fixture::native();
    exact(&f.app(&["--engine", "native"]), DEFAULT_APP);
    let table: [(&[&str], &[u8]); 7] = [
        (&["--fail-fast"], FAIL_FAST_APP),
        (&["--filter", "late", "--fail-fast"], FILTER_LATE_FAIL_FAST),
        (&["--fail-fast", "--filter", "l"], FILTER_L_FAIL_FAST),
        (&["--fail-fast", "--filter", "early"], FILTER_EARLY),
        (&["--filter=divide", "--fail-fast"], FILTER_DIVIDE),
        (&["--fail-fast", "--filter", "pass"], FILTER_PASS),
        (&["--fail-fast", "--filter=test_"], FAIL_FAST_APP),
    ];
    for (form, expected) in table {
        let native = f.app(&native_args(form));
        exact(&native, expected);
        assert_eq!(native.stdout, f.app(form).stdout, "{form:?}");
    }
    let out = f.app(&native_args(&["--fail-fast", "--filter", "zzz"]));
    assert_eq!(out.stderr, NO_MATCH, "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    for form in [&["--fail-fast"][..], &[]] {
        exact(&f.member(GREEN, &native_args(form)), GREEN_SUITE);
        exact(&f.member(TAIL, &native_args(form)), TAIL_SUITE);
        exact(&f.member(HELPER, &native_args(form)), HELPER_SUITE);
    }
    exact(
        &f.ws("test", &native_args(&["--fail-fast", "--package", APP])),
        FAIL_FAST_APP,
    );
    assert!(
        !f.path("ex/.oxid").exists(),
        "virtual root receives no output"
    );
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_prepares_every_selected_artifact_with_original_ordinals() {
    // A fresh fail-fast run publishes every selected ordinal (prepared before
    // the first case runs) even though it executes only the first two, and
    // each artifact stays bound to its own original suite case.
    for (filter, expected, ordinals) in [
        (None, FAIL_FAST_APP, vec![0usize, 1, 2, 3, 4]),
        (Some("late"), FILTER_LATE_FAIL_FAST, vec![3, 4]),
        (Some("l"), FILTER_L_FAIL_FAST, vec![0, 1, 3, 4]),
        (Some("early"), FILTER_EARLY, vec![0, 1]),
    ] {
        let f = Fixture::native();
        let mut form = vec!["--fail-fast"];
        if let Some(filter) = filter {
            form.extend_from_slice(&["--filter", filter]);
        }
        exact(&f.app(&native_args(&form)), expected);
        assert_eq!(published(&f, "app", 5), ordinals, "{filter:?}");
        let dir = f.test_dir("app");
        for &ordinal in &ordinals {
            let (name, pass) = SUITE[ordinal];
            let exe = standalone(&dir.join(format!("case-{ordinal}")));
            assert_eq!(exe.status.code(), Some(i32::from(!pass)), "{name}: {exe:?}");
            assert_eq!(
                text(&exe.stderr).contains(DIV_ZERO),
                name == "test_late_divide",
                "{name}: {exe:?}"
            );
        }
        assert!(!f.path("ex/helper/.oxid").exists());
        assert!(!f.path("ex/.oxid").exists());
    }
    // After a full no-flag suite published all five ordinals, fail-fast runs
    // reuse them and still execute the matching original cases.
    let f = Fixture::native();
    exact(&f.app(&["--engine", "native"]), DEFAULT_APP);
    exact(&f.app(&native_args(&["--fail-fast"])), FAIL_FAST_APP);
    exact(
        &f.app(&native_args(&["--fail-fast", "--filter", "late"])),
        FILTER_LATE_FAIL_FAST,
    );
    assert_eq!(published(&f, "app", 5), vec![0, 1, 2, 3, 4]);
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_later_prepare_failure_runs_no_case_and_prints_no_summary() {
    // An unowned publication target at a later selected ordinal (after the
    // first failure) aborts the whole run before any case executes or reports.
    for (form, ordinal) in [
        (&["--fail-fast"][..], 4usize),
        (&["--fail-fast"], 2),
        (&["--fail-fast", "--filter", "late"], 4),
        (&["--filter=l", "--fail-fast"], 3),
    ] {
        let f = Fixture::native();
        let dir = f.test_dir("app");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("case-{ordinal}")), "unowned").unwrap();
        let out = f.app(&native_args(form));
        refusal(&out, "unowned");
        assert_eq!(
            fs::read(dir.join(format!("case-{ordinal}"))).unwrap(),
            b"unowned"
        );
    }
    // A later case the native backend cannot admit likewise prevents the
    // earlier cases (including the first failure) from running or reporting.
    let f = Fixture::native();
    f.append(
        "ex/app/src/main.ox",
        "fn test_zz_recursive() -> bool {\n    return test_zz_recursive();\n}\n",
    );
    refusal(&f.app(&native_args(&["--fail-fast"])), "E0700");
    refusal(
        &f.app(&native_args(&["--fail-fast", "--filter", "test_"])),
        "E0700",
    );
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_fail_fast_leaves_app_artifacts_and_receipts_unchanged() {
    let f = Fixture::native();
    for mode in ["result", "process"] {
        let flag = format!("--entry-mode={mode}");
        let build = f.run_with("build", "ex/app/oxid.toml", &[&flag]);
        assert_eq!(build.status.code(), Some(0), "{build:?}");
    }
    let before = f.app_state();
    for state in &before {
        assert!(state[0].is_some() && state[1].is_some(), "{state:?}");
    }
    for form in [
        &["--fail-fast"][..],
        &["--fail-fast", "--filter", "late"],
        &["--fail-fast", "--filter", "pass"],
        &["--fail-fast", "--filter", "zzz"],
    ] {
        let _ = f.app(&native_args(form));
        assert_eq!(before, f.app_state(), "{form:?} changed application state");
    }
    // A prepare failure (fresh test namespace, unowned later ordinal) leaves
    // them unchanged too.
    fs::remove_dir_all(f.test_dir("app")).unwrap();
    fs::create_dir_all(f.test_dir("app")).unwrap();
    fs::write(f.test_dir("app").join("case-4"), "unowned").unwrap();
    let out = f.app(&native_args(&["--fail-fast"]));
    assert!(out.stdout.is_empty(), "{out:?}");
    assert_eq!(
        before,
        f.app_state(),
        "prepare failure changed application state"
    );
    // Ordinary native RUN of main is unaffected and reports only main's result.
    let run = f.run_with("run", "ex/app/oxid.toml", &["--engine", "native"]);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert_eq!(run.stdout, b"7\n");
}
