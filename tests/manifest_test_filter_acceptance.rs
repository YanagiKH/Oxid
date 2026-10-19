//! Independent explicit-manifest `oxid test --filter` acceptance (task
//! r1-test-filter-24781574; companion to root's CLI/runner patch). Tests
//! marked `#[ignore]` need LLVM 19.1.7: run them with
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test
//! manifest_test_filter_acceptance -- --include-ignored`. Opting in without
//! that env var fails the LLVM gate; a missing tool is never a pass.
//!
//! Frozen here: the fixed suite, the selected cases, their PASS/FAIL results,
//! source order, exit status, "no case executed" / "no summary" structure,
//! unchanged no-filter output and unchanged existing (JSON, global and local
//! CLI) error priority. The new diagnostics are pinned to miku's supplement
//! (ts 1792436551.614339): no match -> E0800 project-test "no tests match
//! --filter" without a primary span; empty/missing -> "--filter requires a
//! value"; duplicate -> "--filter may only be specified once"; any other
//! operation -> "--filter is only supported by explicit manifest test", all
//! on the existing CLI (E0001) rejection path. The filtered summary counts
//! only executed cases (root bundle spec/typed-project-test.md).
//! No qualification, capacity or fuel probes.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

type FileSnapshot = Option<(u64, u64, Vec<u8>)>;
/// One CLI argument form (e.g. `["--filter", "keep"]`).
type Args = Vec<&'static str>;
/// A suite case: root test name and whether it passes.
type Case = (&'static str, bool);

static NEXT: AtomicUsize = AtomicUsize::new(0);
const APP: &str = "filteraccept-app";
const HELPER: &str = "filteraccept-helper";
const ROOT_MANIFEST: &str = "[workspace]\nmembers = [\"app\", \"helper\"]\n";
const APP_MANIFEST: &str = "[project]\nname = \"filteraccept-app\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\n[dependencies]\nhelper = \"../helper\"\n";
const APP_MAIN: &str = "mod child;\nuse helper::seven;\nuse helper::nothing;\nfn main() -> i32 {\n    return seven();\n}\nfn test_keep_z() -> bool {\n    return seven() == 7;\n}\nfn test_drop_false() -> bool {\n    return seven() == 8;\n}\nfn test_keep_a() -> i32 {\n    return nothing();\n}\nfn test_drop_error() -> i32 {\n    return seven() / nothing();\n}\nfn test_keep_unit() -> () {\n    return;\n}\n";
const APP_CHILD: &str = "pub fn test_keep_child() -> bool {\n    return false;\n}\n";
const HELPER_MANIFEST: &str = "[project]\nname = \"filteraccept-helper\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/lib.ox\"\n";
const HELPER_LIB: &str = "pub fn seven() -> i32 {\n    return 7;\n}\npub fn nothing() -> i32 {\n    return 0;\n}\nfn main() -> i32 {\n    return 91;\n}\nfn test_keep_helper() -> bool {\n    return seven() == 7;\n}\n";
const FILES: [(&str, &str); 6] = [
    ("oxid.toml", ROOT_MANIFEST),
    ("app/oxid.toml", APP_MANIFEST),
    ("app/src/main.ox", APP_MAIN),
    ("app/src/child.ox", APP_CHILD),
    ("helper/oxid.toml", HELPER_MANIFEST),
    ("helper/src/lib.ox", HELPER_LIB),
];

/// The frozen suite: root `test_` declarations in source order with their
/// result (`true` = PASS). `main`, the child's `test_keep_child` and the
/// dependency's `test_keep_helper` are never cases of the app suite.
const SUITE: [(&str, bool); 5] = [
    ("test_keep_z", true),
    ("test_drop_false", false),
    ("test_keep_a", true),
    ("test_drop_error", false),
    ("test_keep_unit", true),
];
/// Existing no-filter output (unchanged by the filter feature).
const NO_FILTER: &[u8] = b"PASS test_keep_z\nFAIL test_drop_false\nPASS test_keep_a\nFAIL test_drop_error\nPASS test_keep_unit\ntest result: 3 passed; 2 failed; 5 total\n";
const HELPER_SUITE: &[u8] = b"PASS test_keep_helper\ntest result: 1 passed; 0 failed; 1 total\n";
/// Runtime diagnostic of test_drop_error; its absence proves the case did not run.
const DIV_ZERO: &str = "E0607";
const AVAILABLE: &str =
    "workspace requires --package; available packages: filteraccept-app, filteraccept-helper";
/// A manifest path that never exists: refusals must precede reading it.
const MISSING: &str = "absent/oxid.toml";
const NO_MATCH: &str = "error[E0800] (project-test): no tests match --filter\n";
const REQUIRES_VALUE: &str = "--filter requires a value";
const DUPLICATE: &str = "--filter may only be specified once";
const ONLY_TEST: &str = "--filter is only supported by explicit manifest test";
/// Existing stdout line of every refused manifest package invocation.
const PACKAGE_FAILED: &[u8] =
    b"typed-preview native package failed: managed executable not published; archive not published\n";

fn llvm_bin() -> PathBuf {
    let dir = PathBuf::from(
        std::env::var_os("OXID_TEST_LLVM_BIN")
            .expect("native filter acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin)"),
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
            "oxid-test-filter-accept-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
        };
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/manifest_test_filter_acceptance");
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
    fn oxid(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.root)
            .env("OXID_LLVM_BIN", &self.llvm)
            .env("OXID_CACHE_DIR", self.path("cache"))
            .env("PATH", self.path("absent-path"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .output()
            .unwrap()
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
        for dir in ["ex", "ex/app", "ex/helper", ""] {
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
/// Suite cases whose root name contains `filter` (case-sensitive), in
/// original source order. Used only to cross-check the frozen tables below.
fn expected_selection(filter: &str) -> Vec<(&'static str, bool)> {
    SUITE
        .iter()
        .copied()
        .filter(|(name, _)| name.contains(filter))
        .collect()
}
fn case_lines(cases: &[(&str, bool)]) -> String {
    cases
        .iter()
        .map(|(name, pass)| format!("{} {name}\n", if *pass { "PASS" } else { "FAIL" }))
        .collect()
}

/// A filtered run that selected `cases`: exactly these case lines in source
/// order, then exactly one summary line with the frozen counts. Exit 0 iff
/// every selected case passed, otherwise the existing failure status 1.
fn selected(out: &Output, cases: &[(&str, bool)]) {
    let passed = cases.iter().filter(|(_, pass)| *pass).count();
    let failed = cases.len() - passed;
    let expected_code = if failed == 0 { 0 } else { 1 };
    assert_eq!(out.status.code(), Some(expected_code), "{out:?}");
    let body = text(&out.stdout);
    let lines: Vec<&str> = body.lines().collect();
    assert!(body.ends_with('\n'), "{out:?}");
    assert_eq!(lines.len(), cases.len() + 1, "{out:?}");
    assert_eq!(
        body[..body.len() - lines[cases.len()].len() - 1],
        case_lines(cases),
        "{out:?}"
    );
    // The summary counts only executed (selected) cases.
    assert_eq!(
        lines[cases.len()],
        format!(
            "test result: {passed} passed; {failed} failed; {} total",
            cases.len()
        ),
        "{out:?}"
    );
    let ran_division = cases.iter().any(|(name, _)| *name == "test_drop_error");
    assert_eq!(
        text(&out.stderr).contains(DIV_ZERO),
        ran_division,
        "{out:?}"
    );
    if !ran_division {
        assert!(out.stderr.is_empty(), "{out:?}");
    }
}
/// No match after the full check: E0800 project-test "no tests match
/// --filter" without a primary span, no case executed or reported, no summary.
fn no_case(out: &Output) {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert_eq!(text(&out.stderr), NO_MATCH, "{out:?}");
}
/// A `--filter` refusal on the existing CLI (E0001) path before project I/O:
/// exactly one text diagnostic whose message is one of `allowed`, the
/// operation's existing refusal stdout, never the missing-manifest read
/// error, a swallowed-option symptom or a case.
fn filter_refusal(out: &Output, operation: &str, allowed: &[&str]) {
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
/// An existing preflight refusal (signature, discovery or type error).
fn refusal(out: &Output, message: &str) {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(text(&out.stderr).contains(message), "{out:?}");
    assert!(!text(&out.stderr).contains(DIV_ZERO), "{out:?}");
}

/// Frozen filter table: (argument forms, selected cases).
fn frozen_filters() -> Vec<(Vec<Args>, Vec<Case>)> {
    vec![
        (
            vec![
                vec!["--filter", "keep"],
                vec!["--filter=keep"],
                vec!["--engine", "interpreter", "--filter", "keep"],
                vec!["--filter", "keep", "--engine=interpreter"],
            ],
            vec![
                ("test_keep_z", true),
                ("test_keep_a", true),
                ("test_keep_unit", true),
            ],
        ),
        (
            vec![
                vec!["--filter=test_drop_false"],
                vec!["--filter", "test_drop_false"],
            ],
            vec![("test_drop_false", false)],
        ),
        (
            vec![vec!["--filter", "a"]],
            vec![("test_drop_false", false), ("test_keep_a", true)],
        ),
        (
            vec![vec!["--filter=test_drop"]],
            vec![("test_drop_false", false), ("test_drop_error", false)],
        ),
        (vec![vec!["--filter", "_z"]], vec![("test_keep_z", true)]),
        (
            vec![vec!["--filter=test_keep_unit"]],
            vec![("test_keep_unit", true)],
        ),
        (vec![vec!["--filter", "test_"]], SUITE.to_vec()),
    ]
}
const UNMATCHED: [&str; 7] = [
    "KEEP",
    "Keep",
    "main",
    "test_keep_child",
    "test_keep_helper",
    "test_keep_z_",
    "zzz",
];
/// Malformed --filter forms with the accepted refusal message(s); each must
/// be refused before project I/O and without consuming the following option.
/// The first local error wins; on a repeated token, duplicate precedes missing.
const MALFORMED: [(&[&str], &[&str]); 11] = [
    (&["--filter="], &[REQUIRES_VALUE]),
    (&["--filter", ""], &[REQUIRES_VALUE]),
    (&["--filter"], &[REQUIRES_VALUE]),
    (&["--filter", "--engine", "interpreter"], &[REQUIRES_VALUE]),
    (&["--filter", "--engine=native"], &[REQUIRES_VALUE]),
    (&["--engine", "native", "--filter"], &[REQUIRES_VALUE]),
    (&["--filter=keep", "--filter=keep"], &[DUPLICATE]),
    (&["--filter", "keep", "--filter=a"], &[DUPLICATE]),
    (
        &[
            "--filter",
            "keep",
            "--engine",
            "interpreter",
            "--filter",
            "z",
        ],
        &[DUPLICATE],
    ),
    (&["--filter=", "--filter", "keep"], &[REQUIRES_VALUE]),
    (&["--filter=keep", "--filter="], &[DUPLICATE]),
];

// ---- Interpreter / CLI contracts (no toolchain required) ----

#[test]
fn frozen_tables_match_case_sensitive_substring_in_source_order() {
    for (forms, cases) in frozen_filters() {
        for form in forms {
            let filter = form
                .iter()
                .zip(form.iter().skip(1))
                .find(|(flag, _)| **flag == "--filter")
                .map(|(_, value)| *value)
                .or_else(|| form.iter().find_map(|arg| arg.strip_prefix("--filter=")))
                .unwrap();
            assert_eq!(expected_selection(filter), cases, "{form:?}");
        }
    }
    for filter in UNMATCHED {
        assert!(expected_selection(filter).is_empty(), "{filter}");
    }
    assert_eq!(
        Some(case_lines(&SUITE).as_bytes()),
        NO_FILTER.strip_suffix(b"test result: 3 passed; 2 failed; 5 total\n")
    );
}

#[test]
fn interpreter_no_filter_output_unchanged_and_main_not_a_case() {
    let f = Fixture::new(None);
    for extra in [
        vec![],
        vec!["--engine", "interpreter"],
        vec!["--engine=interpreter"],
    ] {
        let out = f.app(&extra);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert_eq!(out.stdout, NO_FILTER, "{out:?}");
        assert_eq!(text(&out.stderr).matches(DIV_ZERO).count(), 1, "{out:?}");
    }
    let ws = f.ws("test", &["--package", APP]);
    assert_eq!(ws.status.code(), Some(1), "{ws:?}");
    assert_eq!(ws.stdout, NO_FILTER, "{ws:?}");
    let helper = f.ws("test", &["--package", HELPER]);
    assert_eq!(helper.status.code(), Some(0), "{helper:?}");
    assert_eq!(helper.stdout, HELPER_SUITE, "{helper:?}");
    assert!(helper.stderr.is_empty(), "{helper:?}");
    let run = f.run_with("run", "ex/app/oxid.toml", &[]);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert_eq!(run.stdout, b"7\n");
    assert!(run.stderr.is_empty(), "{run:?}");
    f.no_managed_output();
}

#[test]
fn interpreter_filter_selects_frozen_cases_in_source_order() {
    let f = Fixture::new(None);
    for (forms, cases) in frozen_filters() {
        for form in forms {
            selected(&f.app(&form), &cases);
            let mut ws = vec!["--package", APP];
            ws.extend_from_slice(&form);
            selected(&f.ws("test", &ws), &cases);
        }
    }
    f.no_managed_output();
}

#[test]
fn interpreter_unmatched_filter_runs_no_case_and_prints_no_summary() {
    let f = Fixture::new(None);
    for filter in UNMATCHED {
        let split = ["--filter", filter];
        let joined = format!("--filter={filter}");
        no_case(&f.app(&split));
        no_case(&f.app(&[joined.as_str()]));
        no_case(&f.ws("test", &["--package", APP, "--filter", filter]));
    }
    // The dependency suite has no app case names.
    no_case(&f.ws("test", &["--package", HELPER, "--filter", "test_keep_z"]));
    f.no_managed_output();
}

#[test]
fn filter_never_skips_whole_project_typecheck_or_signature_validation() {
    // Every variant breaks only code that no frozen filter selects; the whole
    // suite is still refused before the first case, on both engines (native
    // refuses in preflight before reaching the absent toolchain).
    let variants: [(&str, &str, &str); 6] = [
        (
            "ex/app/src/main.ox",
            "fn test_drop_bad(x: i32) -> i32 {\n    return x;\n}\n",
            "zero parameters",
        ),
        (
            "ex/app/src/main.ox",
            "fn test_drop_typed() -> bool {\n    return 1;\n}\n",
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
            "ex/app/src/main.ox",
            "fn test_drop_param(flag: bool) -> bool {\n    return flag;\n}\n",
            "zero parameters",
        ),
    ];
    let f = Fixture::new(None);
    for (file, extra, message) in variants {
        f.reset();
        f.append(file, extra);
        for filter in [
            vec!["--filter", "keep"],
            vec!["--filter=test_keep_z"],
            vec!["--filter", "zzz"],
        ] {
            for engine in ["interpreter", "native"] {
                let mut args = filter.clone();
                args.extend_from_slice(&["--engine", engine]);
                let out = f.app(&args);
                refusal(&out, message);
                assert!(!text(&out.stderr).contains("E0701"), "{out:?}");
                let mut ws = vec!["--package", APP];
                ws.extend_from_slice(&args);
                refusal(&f.ws("test", &ws), message);
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
            &f.app(&["--filter", "keep", "--engine", engine]),
            "no root test_",
        );
    }
    f.no_managed_output();
}

#[test]
fn malformed_filter_is_refused_before_reading_missing_project() {
    let f = Fixture::new(None);
    for (form, allowed) in MALFORMED {
        filter_refusal(&f.run_with("test", MISSING, form), "test", allowed);
        // An existing workspace without --package: the filter refusal precedes
        // reading it (no package listing).
        filter_refusal(&f.ws("test", form), "test", allowed);
        // A following --package is not swallowed as the filter value.
        let mut ws = form.to_vec();
        ws.extend_from_slice(&["--package", APP]);
        filter_refusal(&f.ws("test", &ws), "test", allowed);
    }
    for form in [
        &["--filter", "--package", APP][..],
        &["--filter", "--package=filteraccept-app"],
    ] {
        filter_refusal(&f.ws("test", form), "test", &[REQUIRES_VALUE]);
    }
    assert!(!f.path("absent").exists());
    f.no_managed_output();
}

#[test]
fn filter_on_other_manifest_operations_is_refused_before_project_io() {
    let f = Fixture::new(None);
    let forms: [(&[&str], &[&str]); 5] = [
        (&["--filter", "keep"], &[ONLY_TEST]),
        (&["--filter=keep"], &[ONLY_TEST]),
        (&["--filter="], &[REQUIRES_VALUE]),
        (&["--filter"], &[REQUIRES_VALUE]),
        (&["--filter=keep", "--filter=keep"], &[DUPLICATE]),
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
        for (form, allowed) in forms {
            let mut args = prefix.to_vec();
            args.extend_from_slice(form);
            filter_refusal(&f.run_with(operation, MISSING, &args), operation, allowed);
            let mut ws = vec!["--package", APP];
            ws.extend_from_slice(&args);
            filter_refusal(&f.ws(operation, &ws), operation, allowed);
        }
    }
    // Non-manifest (legacy) TEST does not gain --filter either.
    for form in [
        &["test", "--filter", "keep"][..],
        &["test", "--filter=keep"],
    ] {
        filter_refusal(&f.oxid(form), "test", &[ONLY_TEST]);
    }
    assert!(!f.path("out.tar").exists());
    f.no_managed_output();
}

#[test]
fn json_global_and_local_error_priority_unchanged_by_valid_filter() {
    let f = Fixture::new(None);
    // A valid filter changes none of the existing refusals: byte-identical
    // to the same invocation without it (all precede reading the project).
    let cases: [&[&str]; 10] = [
        &["--message-format=json"],
        &["--message-format", "json", "--engine", "native"],
        &["--message-format=yaml"],
        &["--manifest-path", "other/oxid.toml"],
        &["--edition", "typed-preview", "--edition=legacy-0.9"],
        &["--engine", "bogus"],
        &["--entry-mode", "process"],
        &["--edition", "legacy-0.9"],
        &["--output", "x.tar"],
        &["extra-operand"],
    ];
    for existing in cases {
        let baseline = f.run_with("test", MISSING, existing);
        assert_eq!(baseline.status.code(), Some(1), "{baseline:?}");
        assert!(
            !text(&baseline.stderr).contains("cannot read"),
            "{baseline:?}"
        );
        for filter in [&["--filter", "keep"][..], &["--filter=test_drop_false"]] {
            for before in [true, false] {
                let mut args = Vec::new();
                if before {
                    args.extend_from_slice(filter);
                }
                args.extend_from_slice(existing);
                if !before {
                    args.extend_from_slice(filter);
                }
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
    // The existing TEST JSON refusal itself, unchanged with a valid filter.
    let json = f.run_with("test", MISSING, &["--message-format=json", "--filter=keep"]);
    assert_eq!(json.status.code(), Some(1), "{json:?}");
    assert!(json.stderr.is_empty(), "{json:?}");
    let body = text(&json.stdout);
    assert_eq!(body.lines().count(), 1, "{body}");
    assert!(body.contains("\"code\":\"E0001\""), "{body}");
    assert!(
        body.contains("manifest test does not support --message-format=json"),
        "{body}"
    );
    // Global-tier errors keep priority over a malformed filter.
    for (global, message) in [
        (
            &["--message-format=yaml"][..],
            "unknown --message-format `yaml`",
        ),
        (
            &["--manifest-path", "other/oxid.toml"][..],
            "--manifest-path may only be specified once",
        ),
    ] {
        let baseline = f.run_with("test", MISSING, global);
        assert!(text(&baseline.stderr).contains(message), "{baseline:?}");
        for malformed in [
            &["--filter="][..],
            &["--filter"],
            &["--filter=a", "--filter=b"],
        ] {
            let mut args = malformed.to_vec();
            args.extend_from_slice(global);
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
    // JSON with a malformed filter: the filter refusal is an ordinary local
    // CLI error, which (as before) precedes the TEST JSON refusal and is
    // rendered as one JSON diagnostic; nothing is read.
    for (malformed, message) in [
        (&["--filter="][..], REQUIRES_VALUE),
        (&["--filter"], REQUIRES_VALUE),
        (&["--filter=a", "--filter=b"], DUPLICATE),
    ] {
        let mut args = vec!["--message-format=json"];
        args.extend_from_slice(malformed);
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
fn workspace_package_and_path_dependency_filtering() {
    let f = Fixture::new(None);
    let keep = [
        ("test_keep_z", true),
        ("test_keep_a", true),
        ("test_keep_unit", true),
    ];
    selected(
        &f.ws("test", &["--package", APP, "--filter", "keep"]),
        &keep,
    );
    selected(
        &f.ws("test", &["--package=filteraccept-app", "--filter=keep"]),
        &keep,
    );
    selected(
        &f.ws("test", &["--filter", "keep", "--package", APP]),
        &keep,
    );
    selected(
        &f.ws("test", &["--package", APP, "--filter=test_drop_false"]),
        &[("test_drop_false", false)],
    );
    // The helper package's own suite is filtered on its own root names.
    selected(
        &f.ws("test", &["--package", HELPER, "--filter", "keep"]),
        &[("test_keep_helper", true)],
    );
    selected(
        &f.ws(
            "test",
            &["--package=filteraccept-helper", "--filter=helper"],
        ),
        &[("test_keep_helper", true)],
    );
    // A valid filter keeps the existing package-selection refusal.
    for extra in [&["--filter", "keep"][..], &["--filter=zzz"]] {
        let out = f.ws("test", extra);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stdout.is_empty(), "{out:?}");
        assert!(text(&out.stderr).contains(AVAILABLE), "{out:?}");
    }
    // Path dependency: each command loads a fresh session, so changing the
    // helper changes exactly the selected results (seven() == 8 now).
    f.write(
        "ex/helper/src/lib.ox",
        &HELPER_LIB.replacen("return 7;", "return 8;", 1),
    );
    let changed = [
        ("test_keep_z", false),
        ("test_keep_a", true),
        ("test_keep_unit", true),
    ];
    selected(&f.app(&["--filter", "keep"]), &changed);
    selected(
        &f.ws("test", &["--package", APP, "--filter=keep"]),
        &changed,
    );
    selected(
        &f.app(&["--filter=test_drop_false"]),
        &[("test_drop_false", true)],
    );
    f.no_managed_output();
}

#[test]
fn native_without_toolchain_filtered_test_does_not_fallback_or_touch_app_outputs() {
    let f = Fixture::new(None);
    for mode in ["result", "process"] {
        let dir = f.path(&format!("ex/app/.oxid/typed-preview/{mode}"));
        fs::create_dir_all(dir.join(format!(".{APP}.build"))).unwrap();
        fs::write(dir.join(APP), "existing application").unwrap();
        fs::write(dir.join(format!(".{APP}.build/owner")), "existing receipt").unwrap();
    }
    let before = f.app_state();
    for filter in [&["--filter", "keep"][..], &["--filter=test_drop_false"]] {
        let mut args = filter.to_vec();
        args.extend_from_slice(&["--engine", "native"]);
        let out = f.app(&args);
        refusal(&out, "E0701");
        assert!(!text(&out.stdout).contains("PASS"), "{out:?}");
        assert_eq!(before, f.app_state());
    }
    // No fallback: the interpreter still has to be asked for explicitly.
    selected(
        &f.app(&["--filter", "keep"]),
        &[
            ("test_keep_z", true),
            ("test_keep_a", true),
            ("test_keep_unit", true),
        ],
    );
}

#[test]
fn example_readme_documents_frozen_contract() {
    let readme = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/manifest_test_filter_acceptance/README.md"),
    )
    .unwrap();
    for needle in [
        "--filter keep",
        "--filter=test_drop_false",
        "test result: 3 passed; 2 failed; 5 total",
        "--package filteraccept-app",
        "LLVM 19.1.7",
        "OXID_LLVM_BIN",
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

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_matches_interpreter_for_every_frozen_filter() {
    let f = Fixture::native();
    let interpreter = f.app(&[]);
    let native = f.app(&["--engine", "native"]);
    assert_eq!(native.status.code(), Some(1), "{native:?}");
    assert_eq!(native.stdout, NO_FILTER, "{native:?}");
    assert_eq!(native.stdout, interpreter.stdout);
    for (forms, cases) in frozen_filters() {
        // Forms that already name the interpreter engine are covered above.
        for form in forms
            .into_iter()
            .filter(|form| !form.iter().any(|arg| arg.starts_with("--engine")))
        {
            let interpreter = f.app(&form);
            let native = f.app(&native_args(&form));
            selected(&interpreter, &cases);
            selected(&native, &cases);
            assert_eq!(native.status.code(), interpreter.status.code(), "{form:?}");
            assert_eq!(native.stdout, interpreter.stdout, "{form:?}");
        }
    }
    for filter in UNMATCHED {
        no_case(&f.app(&native_args(&["--filter", filter])));
    }
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_artifacts_keep_original_suite_ordinals() {
    // Fresh output: only the selected ordinals are published, each bound to
    // its own original suite case.
    for (filter, ordinals) in [
        ("keep", vec![0usize, 2, 4]),
        ("test_drop", vec![1, 3]),
        ("a", vec![1, 2]),
        ("test_keep_unit", vec![4]),
    ] {
        let f = Fixture::native();
        selected(
            &f.app(&native_args(&["--filter", filter])),
            &expected_selection(filter),
        );
        let dir = f.test_dir("app");
        for (ordinal, (name, pass)) in SUITE.iter().enumerate() {
            let artifact = dir.join(format!("case-{ordinal}"));
            let receipt = dir.join(format!(".case-{ordinal}.build/owner"));
            if !ordinals.contains(&ordinal) {
                assert!(!artifact.exists(), "{filter}: unselected {name} published");
                assert!(!receipt.exists(), "{filter}: unselected {name} receipt");
                continue;
            }
            assert!(artifact.is_file(), "{filter}: {name}");
            assert!(receipt.is_file(), "{filter}: {name}");
            let exe = standalone(&artifact);
            assert_eq!(
                exe.status.code(),
                Some(if *pass { 0 } else { 1 }),
                "{name}: {exe:?}"
            );
            assert_eq!(
                text(&exe.stderr).contains(DIV_ZERO),
                *name == "test_drop_error",
                "{name}: {exe:?}"
            );
        }
        assert!(!f.path("ex/helper/.oxid").exists());
        assert!(!f.path("ex/.oxid").exists());
    }
    // After a full suite published all five ordinals, filtered runs must still
    // execute the matching original case (no zip mismatch on reuse).
    let f = Fixture::native();
    assert_eq!(f.app(&["--engine", "native"]).stdout, NO_FILTER);
    for filter in ["keep", "a", "test_keep_unit", "test_drop_error", "_z"] {
        let out = f.app(&native_args(&["--filter", filter]));
        selected(&out, &expected_selection(filter));
        assert_eq!(out.stdout, f.app(&["--filter", filter]).stdout, "{filter}");
    }
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_prepare_failure_runs_no_selected_case() {
    // An unowned publication target at a later selected ordinal aborts the
    // whole filtered run before the first selected case executes or reports.
    for (filter, ordinal) in [("keep", 4usize), ("test_drop", 3), ("a", 2)] {
        let f = Fixture::native();
        let dir = f.test_dir("app");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("case-{ordinal}")), "unowned").unwrap();
        let out = f.app(&native_args(&["--filter", filter]));
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stdout.is_empty(), "{out:?}");
        assert!(text(&out.stderr).contains("unowned"), "{out:?}");
        assert!(!text(&out.stderr).contains(DIV_ZERO), "{out:?}");
        assert_eq!(
            fs::read(dir.join(format!("case-{ordinal}"))).unwrap(),
            b"unowned"
        );
    }
    // A selected case the native backend cannot admit likewise prevents every
    // earlier selected case from reporting; the interpreter runs them all.
    let f = Fixture::native();
    f.append(
        "ex/app/src/main.ox",
        "fn test_keep_recursive() -> bool {\n    return test_keep_recursive();\n}\n",
    );
    let out = f.app(&native_args(&["--filter", "keep"]));
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(text(&out.stderr).contains("E0700"), "{out:?}");
    assert!(!text(&out.stderr).contains(DIV_ZERO), "{out:?}");
    let interpreter = f.app(&["--filter", "keep"]);
    assert_eq!(interpreter.status.code(), Some(1), "{interpreter:?}");
    let body = text(&interpreter.stdout);
    assert!(
        body.starts_with(
            "PASS test_keep_z\nPASS test_keep_a\nPASS test_keep_unit\nFAIL test_keep_recursive\n"
        ),
        "{interpreter:?}"
    );
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_filtered_test_leaves_app_artifacts_and_receipts_unchanged() {
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
    let forms: [&[&str]; 4] = [
        &["--filter", "keep"],
        &["--filter=test_drop_false"],
        &["--filter", "test_"],
        &["--filter", "zzz"],
    ];
    for form in forms {
        let _ = f.app(&native_args(form));
        assert_eq!(before, f.app_state(), "{form:?} changed application state");
    }
    // A prepare failure (fresh test namespace, unowned target at a selected
    // ordinal) leaves them unchanged too.
    fs::remove_dir_all(f.test_dir("app")).unwrap();
    fs::create_dir_all(f.test_dir("app")).unwrap();
    fs::write(f.test_dir("app").join("case-4"), "unowned").unwrap();
    let out = f.app(&native_args(&["--filter", "keep"]));
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

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_workspace_package_and_path_dependency_filter_matches_interpreter() {
    let f = Fixture::native();
    let keep = [
        ("test_keep_z", true),
        ("test_keep_a", true),
        ("test_keep_unit", true),
    ];
    for selector in [vec!["--package", APP], vec!["--package=filteraccept-app"]] {
        let mut args = selector.clone();
        args.extend_from_slice(&["--filter", "keep"]);
        let interpreter = f.ws("test", &args);
        let native = f.ws("test", &native_args(&args));
        selected(&native, &keep);
        assert_eq!(native.stdout, interpreter.stdout);
    }
    let dir = f.test_dir("app");
    for ordinal in 0..5 {
        assert_eq!(
            dir.join(format!("case-{ordinal}")).is_file(),
            [0, 2, 4].contains(&ordinal),
            "case-{ordinal}"
        );
    }
    assert!(
        !f.path("ex/.oxid").exists(),
        "virtual root receives no output"
    );
    assert!(
        !f.path("ex/helper/.oxid").exists(),
        "dependency receives no output"
    );
    let helper = f.ws(
        "test",
        &native_args(&["--package", HELPER, "--filter=keep"]),
    );
    selected(&helper, &[("test_keep_helper", true)]);
    assert!(f.test_dir("helper").join("case-0").is_file());
    no_case(&f.ws(
        "test",
        &native_args(&["--package", HELPER, "--filter", "test_keep_z"]),
    ));
    // Fresh session through the path dependency on the native engine too.
    f.write(
        "ex/helper/src/lib.ox",
        &HELPER_LIB.replacen("return 7;", "return 8;", 1),
    );
    let changed = [
        ("test_keep_z", false),
        ("test_keep_a", true),
        ("test_keep_unit", true),
    ];
    let native = f.app(&native_args(&["--filter", "keep"]));
    selected(&native, &changed);
    assert_eq!(native.stdout, f.app(&["--filter", "keep"]).stdout);
}
