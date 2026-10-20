//! Independent acceptance for explicit sequential workspace CHECK and
//! interpreter TEST (`--workspace`). Expectations are hand-written from the
//! contract or taken from the ordinary selected-package command; native tools
//! are never used.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

macro_rules! example {
    ($path:literal) => {
        (
            $path,
            include_str!(concat!("../examples/projects/workspace_batch/", $path)),
        )
    };
}

const EXAMPLE: [(&str, &str); 9] = [
    example!("oxid.toml"),
    example!("README.md"),
    example!("core/oxid.toml"),
    example!("core/src/lib.ox"),
    example!("core/src/scale.ox"),
    example!("app/oxid.toml"),
    example!("app/src/main.ox"),
    example!("tools/oxid.toml"),
    example!("tools/src/main.ox"),
];
const MEMBERS: [(&str, &str); 3] = [
    ("core", "batch-core"),
    ("app", "batch-app"),
    ("tools", "batch-tools"),
];
const CORE_PASS: &str = "PASS test_base\ntest result: 1 passed; 0 failed; 1 total\n";
const APP_PASS: &str =
    "PASS test_double\nPASS test_unit\ntest result: 2 passed; 0 failed; 2 total\n";
const TOOLS_PASS: &str = "PASS test_offset\ntest result: 1 passed; 0 failed; 1 total\n";
const TYPE_ERROR: &str =
    "fn main() -> i32 {\n    return true;\n}\nfn test_offset() -> bool {\n    return true;\n}\n";

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "oxid-workspace-batch-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("{error}"),
            }
        }
        panic!("scratch collisions");
    }

    /// A fresh copy of the independent example workspace.
    fn example() -> Self {
        let scratch = Self::new();
        for (path, text) in EXAMPLE {
            scratch.write(path, text);
        }
        scratch
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn command_in(&self, cwd: &Path, args: &[&str], stdout: Stdio) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(cwd)
            .args(args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(Stdio::piped())
            .env_remove("OXID_PATH")
            .env("OXID_LLVM_BIN", self.0.join("missing-llvm"))
            .output()
            .unwrap()
    }

    fn command(&self, args: &[&str]) -> Output {
        self.command_in(&self.0, args, Stdio::piped())
    }

    /// `oxid COMMAND --manifest-path oxid.toml --workspace EXTRA...` from the root.
    fn batch(&self, command: &str, extra: &[&str]) -> Output {
        let mut args = vec![command, "--manifest-path", "oxid.toml", "--workspace"];
        args.extend_from_slice(extra);
        self.command(&args)
    }

    /// The ordinary selected-package CHECK, used as the per-member oracle.
    fn ordinary_check(&self, package: &str) -> String {
        let out = self.command(&["check", "--manifest-path=oxid.toml", "--package", package]);
        let stdout = success(&out);
        assert!(stdout.starts_with("typed-preview check ok ("), "{stdout}");
        assert!(stdout.ends_with(" functions; check only)\n"), "{stdout}");
        stdout
    }

    fn no_outputs(&self) {
        for prefix in ["", "core", "app", "tools"] {
            let directory = self.0.join(prefix);
            assert!(!directory.join(".oxid").exists(), "{prefix}");
            assert!(!directory.join("oxid.lock").exists(), "{prefix}");
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn success(out: &Output) -> String {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    text(&out.stdout)
}

fn heading(name: &str) -> String {
    format!("workspace package: {name}\n")
}

fn summary(passed: usize, failed: usize, unrun: usize, total: usize) -> String {
    assert_eq!(passed + failed + unrun, total);
    format!("workspace result: {passed} passed; {failed} failed; {unrun} unrun; {total} total packages\n")
}

/// Exit 1 with the given stdout; stderr checked by the caller.
fn failure(out: &Output, stdout: &str) -> String {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(text(&out.stdout), stdout);
    text(&out.stderr)
}

/// Catalog or CLI refusal: exit 1, no heading or summary, one diagnostic.
fn refused(out: &Output, needle: &str) -> String {
    let stderr = failure(out, "");
    assert!(stderr.contains(needle), "{needle}: {stderr}");
    stderr
}

#[test]
fn example_check_and_test_run_every_member_in_declared_order() {
    let scratch = Scratch::example();
    let mut check = String::new();
    for (_, name) in MEMBERS {
        check.push_str(&heading(name));
        check.push_str(&scratch.ordinary_check(name));
    }
    check.push_str(&summary(3, 0, 0, 3));
    assert_eq!(success(&scratch.batch("check", &[])), check);
    assert_eq!(success(&scratch.batch("check", &["--fail-fast"])), check);

    let test = [
        heading("batch-core"),
        CORE_PASS.into(),
        heading("batch-app"),
        APP_PASS.into(),
        heading("batch-tools"),
        TOOLS_PASS.into(),
        summary(3, 0, 0, 3),
    ]
    .concat();
    // Dependency and child-module tests (test_base in app/tools, the child's
    // always-false test in core) are never discovered for another member.
    assert_eq!(success(&scratch.batch("test", &[])), test);
    for extra in [
        &["--engine=interpreter"][..],
        &["--engine", "interpreter", "--fail-fast"][..],
        &["--message-format=text"][..],
    ] {
        assert_eq!(success(&scratch.batch("test", extra)), test, "{extra:?}");
    }
    // Each member's TEST output equals the ordinary selected-package run.
    for ((_, name), expected) in MEMBERS.into_iter().zip([CORE_PASS, APP_PASS, TOOLS_PASS]) {
        let out = scratch.command(&["test", "--manifest-path=oxid.toml", "--package", name]);
        assert_eq!(success(&out), expected);
    }
    // An absolute manifest path gives identical results from another cwd.
    let absolute = scratch.0.join("oxid.toml");
    for (command, expected) in [("check", &check), ("test", &test)] {
        let out = scratch.command_in(
            &scratch.0.join("tools/src"),
            &[
                command,
                "--workspace",
                "--manifest-path",
                absolute.to_str().unwrap(),
            ],
            Stdio::piped(),
        );
        assert_eq!(&success(&out), expected);
    }
    let readme = EXAMPLE[1].1;
    for command in [
        "oxid check --manifest-path oxid.toml --workspace",
        "oxid test --manifest-path oxid.toml --workspace",
        "oxid test --manifest-path oxid.toml --workspace --fail-fast",
    ] {
        assert!(readme.contains(command), "{command}");
    }
    assert!(readme.contains(&test));
    scratch.no_outputs();
}

#[test]
fn fresh_new_workspace_checks_and_tests_both_members() {
    let scratch = Scratch::new();
    let out = scratch.command(&["new", "fresh", "--edition", "typed-preview", "--workspace"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let root = scratch.0.join("fresh");
    let readme = fs::read_to_string(root.join("README.md")).unwrap();
    assert!(readme
        .contains("- Check every member: `oxid check --manifest-path oxid.toml --workspace`\n"));
    assert!(
        readme.contains("- Test every member: `oxid test --manifest-path oxid.toml --workspace`\n")
    );
    assert!(!readme.contains("workspace-wide execution is not provided"));
    // Run exactly the README commands from the generated directory.
    let run = |args: &[&str]| scratch.command_in(&root, args, Stdio::piped());
    let mut check = String::new();
    for name in ["workspace-app", "workspace-library"] {
        let ordinary = success(&run(&[
            "check",
            "--manifest-path",
            "oxid.toml",
            "--package",
            name,
        ]));
        assert!(
            ordinary.starts_with("typed-preview check ok ("),
            "{ordinary}"
        );
        check.push_str(&heading(name));
        check.push_str(&ordinary);
    }
    check.push_str(&summary(2, 0, 0, 2));
    assert_eq!(
        success(&run(&[
            "check",
            "--manifest-path",
            "oxid.toml",
            "--workspace"
        ])),
        check
    );
    assert_eq!(
        success(&run(&[
            "test",
            "--manifest-path",
            "oxid.toml",
            "--workspace"
        ])),
        [
            heading("workspace-app"),
            "PASS test_answer\nPASS test_unit\ntest result: 2 passed; 0 failed; 2 total\n".into(),
            heading("workspace-library"),
            "PASS test_library\ntest result: 1 passed; 0 failed; 1 total\n".into(),
            summary(2, 0, 0, 2),
        ]
        .concat()
    );
    for member in ["", "app", "library"] {
        assert!(!root.join(member).join(".oxid").exists(), "{member}");
    }
}

#[test]
fn default_continue_and_fail_fast_with_a_later_type_error() {
    let scratch = Scratch::example();
    // App fails one test case; tools (declared last) has a type error.
    scratch.write(
        "app/src/main.ox",
        &EXAMPLE[6]
            .1
            .replace("double(base()) == 10", "double(base()) == 11"),
    );
    scratch.write("tools/src/main.ox", TYPE_ERROR);
    let core_check = scratch.ordinary_check("batch-core");
    let app_check = scratch.ordinary_check("batch-app");

    let stderr = failure(
        &scratch.batch("test", &[]),
        &[
            heading("batch-core"),
            CORE_PASS.into(),
            heading("batch-app"),
            "FAIL test_double\nPASS test_unit\ntest result: 1 passed; 1 failed; 2 total\n".into(),
            heading("batch-tools"),
            summary(1, 2, 0, 3),
        ]
        .concat(),
    );
    assert!(stderr.contains("E0300"), "{stderr}");

    // Fail-fast stops inside app's suite and never starts tools.
    let stderr = failure(
        &scratch.batch("test", &["--fail-fast"]),
        &[
            heading("batch-core"),
            CORE_PASS.into(),
            heading("batch-app"),
            "FAIL test_double\ntest result: 0 passed; 1 failed; 1 total\n".into(),
            "test stopped: 1 selected tests not run (--fail-fast)\n".into(),
            summary(1, 1, 1, 3),
        ]
        .concat(),
    );
    assert!(stderr.is_empty(), "{stderr}");

    let expected = [
        heading("batch-core"),
        core_check,
        heading("batch-app"),
        app_check,
        heading("batch-tools"),
        summary(2, 1, 0, 3),
    ]
    .concat();
    let stderr = failure(&scratch.batch("check", &[]), &expected);
    assert!(stderr.contains("E0300"), "{stderr}");
    assert_eq!(
        failure(&scratch.batch("check", &["--fail-fast"]), &expected),
        stderr
    );

    // A failing shared dependency is checked again for each dependent.
    scratch.write("tools/src/main.ox", EXAMPLE[8].1);
    scratch.write(
        "core/src/lib.ox",
        &EXAMPLE[3].1.replace("    return 5;", "    return true;"),
    );
    let all = [
        heading("batch-core"),
        heading("batch-app"),
        heading("batch-tools"),
        summary(0, 3, 0, 3),
    ]
    .concat();
    let stderr = failure(&scratch.batch("check", &[]), &all);
    assert!(stderr.matches("E0300").count() >= 3, "{stderr}");
    let stderr = failure(
        &scratch.batch("check", &["--fail-fast"]),
        &[heading("batch-core"), summary(0, 1, 2, 3)].concat(),
    );
    assert!(stderr.contains("E0300"), "{stderr}");
    let stderr = failure(&scratch.batch("test", &[]), &all);
    assert!(stderr.matches("E0300").count() >= 3, "{stderr}");
    scratch.no_outputs();
}

#[test]
fn later_preparation_failure_follows_earlier_members_without_prevalidation() {
    let scratch = Scratch::example();
    // Valid manifests but tools' source is absent: only its own session fails.
    fs::remove_file(scratch.0.join("tools/src/main.ox")).unwrap();
    let stderr = failure(
        &scratch.batch("test", &[]),
        &[
            heading("batch-core"),
            CORE_PASS.into(),
            heading("batch-app"),
            APP_PASS.into(),
            heading("batch-tools"),
            summary(2, 1, 0, 3),
        ]
        .concat(),
    );
    assert!(stderr.contains("E0002"), "{stderr}");
    assert!(stderr.contains("main.ox"), "{stderr}");
}

#[test]
fn no_suite_and_invalid_signature_members_are_failures_not_skips() {
    let scratch = Scratch::example();
    let no_suite = "use shared::base;\nuse shared::scale::double;\nfn main() -> i32 {\n    return double(base());\n}\n";
    scratch.write("app/src/main.ox", no_suite);
    // CHECK has no test-suite requirement.
    assert!(success(&scratch.batch("check", &[])).ends_with(&summary(3, 0, 0, 3)));
    let expected = [
        heading("batch-core"),
        CORE_PASS.into(),
        heading("batch-app"),
        heading("batch-tools"),
        TOOLS_PASS.into(),
        summary(2, 1, 0, 3),
    ]
    .concat();
    let stderr = failure(&scratch.batch("test", &[]), &expected);
    assert!(stderr.contains("no root test_"), "{stderr}");

    // Every discovered signature is checked before any of app's cases run.
    let invalid = format!(
        "{}fn test_bad(x: i32) -> i32 {{\n    return x;\n}}\n",
        EXAMPLE[6].1
    );
    scratch.write("app/src/main.ox", &invalid);
    let stderr = failure(&scratch.batch("test", &[]), &expected);
    assert!(stderr.contains("zero parameters"), "{stderr}");
    assert!(!stderr.contains("no root test_"), "{stderr}");
    let stderr = failure(
        &scratch.batch("test", &["--fail-fast"]),
        &[
            heading("batch-core"),
            CORE_PASS.into(),
            heading("batch-app"),
            summary(1, 1, 1, 3),
        ]
        .concat(),
    );
    assert!(stderr.contains("zero parameters"), "{stderr}");
}

#[test]
fn catalog_failures_start_no_member() {
    let scratch = Scratch::example();
    // Every member would pass: any heading would prove a premature start.
    refused(
        &scratch.command(&["check", "--manifest-path", "missing.toml", "--workspace"]),
        "cannot read manifest `missing.toml`",
    );
    refused(
        &scratch.command(&["test", "--manifest-path", "core/oxid.toml", "--workspace"]),
        "--workspace requires a virtual workspace manifest",
    );
    for (manifest, needle) in [
        (
            "[workspace]\nmembers = [\"core\", \"app\", \"tools\", \"core\"]\n",
            "duplicate canonical workspace member `core`",
        ),
        (
            "[workspace]\nmembers = [\"core\", \"app\", \"absent\"]\n",
            "absent",
        ),
        (
            "[workspace]\nmembers = []\n",
            "at least one explicit member",
        ),
        ("[workspace]\nmembers = \"core\"\n", "string array"),
    ] {
        scratch.write("oxid.toml", manifest);
        for command in ["check", "test"] {
            refused(&scratch.batch(command, &[]), needle);
        }
    }
    scratch.write("oxid.toml", EXAMPLE[0].1);
    assert!(success(&scratch.batch("check", &[])).ends_with(&summary(3, 0, 0, 3)));

    scratch.write(
        "tools/oxid.toml",
        &EXAMPLE[7].1.replace("batch-tools", "batch-app"),
    );
    for command in ["check", "test"] {
        refused(
            &scratch.batch(command, &["--fail-fast"]),
            "duplicate workspace member name `batch-app`",
        );
    }
    scratch.write("tools/oxid.toml", "[project\n");
    for command in ["check", "test"] {
        refused(&scratch.batch(command, &[]), "tools");
    }
    scratch.no_outputs();
}

#[test]
fn incompatible_options_are_rejected_before_manifest_io() {
    let scratch = Scratch::new();
    for (command, extra, needle) in [
        (
            "test",
            &["--engine=native"][..],
            "permits --engine=interpreter",
        ),
        ("check", &["--engine=interpreter"][..], "for test only"),
        (
            "test",
            &["--package=batch-app"][..],
            "conflicts with --package",
        ),
        (
            "check",
            &["--package", "batch-app"][..],
            "conflicts with --package",
        ),
        (
            "test",
            &["--filter=test_base"][..],
            "does not support --filter",
        ),
        (
            "check",
            &["--message-format=json"][..],
            "does not support --message-format=json",
        ),
        (
            "test",
            &["--profile=release"][..],
            "--profile is only supported",
        ),
        (
            "check",
            &["--entry-mode=result"][..],
            "does not support entry-mode",
        ),
        ("test", &["--fail-fast=true"][..], "does not accept a value"),
        (
            "check",
            &["src/main.ox"][..],
            "does not accept a source operand",
        ),
    ] {
        let mut args = vec![command, "--manifest-path=absent/oxid.toml", "--workspace"];
        args.extend_from_slice(extra);
        let stderr = refused(&scratch.command(&args), needle);
        assert!(
            !stderr.contains("cannot read manifest"),
            "{args:?}: {stderr}"
        );
    }
    let stderr = refused(
        &scratch.command(&["check", "--workspace"]),
        "requires --manifest-path",
    );
    assert!(!stderr.contains("cannot read"), "{stderr}");
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
}

#[cfg(target_os = "linux")]
#[test]
fn a_failing_heading_write_starts_no_member_and_never_succeeds() {
    let scratch = Scratch::example();
    let full = || {
        Stdio::from(
            fs::OpenOptions::new()
                .write(true)
                .open("/dev/full")
                .unwrap(),
        )
    };
    // Every member passes, but the first heading cannot be written.
    let out = scratch.command_in(
        &scratch.0,
        &["test", "--manifest-path=oxid.toml", "--workspace"],
        full(),
    );
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    // With a type error in the first member, its diagnostic would appear on
    // stderr if the member had started after the failed heading.
    scratch.write(
        "core/src/lib.ox",
        &EXAMPLE[3].1.replace("    return 5;", "    return true;"),
    );
    for command in ["check", "test"] {
        let out = scratch.command_in(
            &scratch.0,
            &[command, "--manifest-path=oxid.toml", "--workspace"],
            full(),
        );
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stderr.is_empty(), "{out:?}");
    }
    scratch.no_outputs();
}
