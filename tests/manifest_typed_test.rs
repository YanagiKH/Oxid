//! Ordinary explicit-manifest typed TEST acceptance (independent of the
//! author's `project_test_core`). Tests marked `#[ignore]` need LLVM 19.1.7:
//! run them with `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test
//! manifest_typed_test -- --include-ignored`. Opting in without that env var
//! fails the LLVM gate (tests do not silently pass as ignored). The remaining
//! tests prove CLI/interpreter contracts and preflight refusals that precede
//! the toolchain. No qualification, capacity or fuel probes.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const NAME: &str = "typed-manifest-test-example";
const MANIFEST: &str = "[project]\nname = \"typed-manifest-test-example\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n";
const MAIN: &str = "mod answer;\nfn main() -> i32 {\n    return crate::answer::value();\n}\nfn test_zulu() -> bool {\n    return crate::answer::value() == 7;\n}\nfn test_alpha() -> () {\n    return;\n}\nfn test_zero() -> i32 {\n    return crate::answer::value() - 7;\n}\n";
const ANSWER: &str = "pub fn value() -> i32 {\n    return 7;\n}\npub fn test_child() -> bool {\n    return false;\n}\n";
const EXAMPLE_PASS: &[u8] =
    b"PASS test_zulu\nPASS test_alpha\nPASS test_zero\ntest result: 3 passed; 0 failed; 3 total\n";

fn llvm_bin() -> PathBuf {
    let dir = PathBuf::from(
        std::env::var_os("OXID_TEST_LLVM_BIN")
            .expect("native TEST acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin)"),
    );
    let version = Command::new(dir.join("clang"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("clang version 19.1.7"),
        "{version:?}"
    );
    dir
}

struct Fixture {
    root: PathBuf,
    llvm: PathBuf,
}
impl Fixture {
    /// Copies the checked-in example to `<root>/example` (never run in-tree).
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-manifest-test-accept-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
        };
        let example =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/projects/typed_manifest_test");
        for (file, expected) in [
            ("oxid.toml", MANIFEST),
            ("src/main.ox", MAIN),
            ("src/answer.ox", ANSWER),
        ] {
            let text = fs::read_to_string(example.join(file)).unwrap();
            assert_eq!(text, expected, "checked-in example {file} changed");
            fixture.write(&format!("example/{file}"), &text);
        }
        fixture
    }
    fn native() -> Self {
        Self::new(Some(llvm_bin()))
    }
    /// A single-file project at `<root>/example` with entry `src/main.ox`.
    fn custom(main: &str, answer: Option<&str>) -> Self {
        let fixture = Self::new(None);
        fixture.write("example/src/main.ox", main);
        if let Some(answer) = answer {
            fixture.write("example/src/answer.ox", answer);
        }
        fixture
    }
    fn custom_native(main: &str, answer: Option<&str>) -> Self {
        let fixture = Self::native();
        fixture.write("example/src/main.ox", main);
        if let Some(answer) = answer {
            fixture.write("example/src/answer.ox", answer);
        }
        fixture
    }
    fn path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.path(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
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
    fn test(&self, extra: &[&str]) -> Output {
        let mut args = vec!["test", "--manifest-path", "example/oxid.toml"];
        args.extend_from_slice(extra);
        self.oxid(&args)
    }
    fn app_out(&self, mode: &str) -> PathBuf {
        self.path(&format!("example/.oxid/typed-preview/{mode}/{NAME}"))
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
fn identity(path: &Path) -> (u64, Vec<u8>) {
    let meta = fs::symlink_metadata(path).unwrap();
    assert!(meta.file_type().is_file(), "{path:?}");
    (meta.ino(), fs::read(path).unwrap())
}
fn optional_identity(path: &Path) -> Option<(u64, Vec<u8>)> {
    match fs::symlink_metadata(path) {
        Ok(_) => Some(identity(path)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("{path:?}: {error}"),
    }
}
fn refusal(out: &Output, message: &str) {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(text(&out.stderr).contains(message), "{out:?}");
}

// ---- Interpreter / CLI contracts (no toolchain required) ----

#[test]
fn interpreter_default_source_order_shared_helper_and_main_unchanged() {
    let f = Fixture::new(None);
    for extra in [
        vec![],
        vec!["--engine", "interpreter"],
        vec!["--engine=interpreter"],
    ] {
        let out = f.test(&extra);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert_eq!(out.stdout, EXAMPLE_PASS, "{out:?}");
        assert!(out.stderr.is_empty(), "{out:?}");
    }
    // Interpreter TEST writes no managed output; ordinary main is untouched.
    assert!(!f.path("example/.oxid").exists());
    let run = f.oxid(&["run", "--manifest-path", "example/oxid.toml"]);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert_eq!(run.stdout, b"7\n");
    assert!(run.stderr.is_empty(), "{run:?}");
    let check = f.oxid(&["check", "--manifest-path", "example/oxid.toml"]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    assert!(!f.path("example/.oxid").exists());
    // An import alias of the child's test_ function is still not a case.
    f.write(
        "example/src/main.ox",
        &MAIN.replace(
            "mod answer;",
            "mod answer;\nuse crate::answer::test_child as test_alias;",
        ),
    );
    let out = f.test(&[]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, EXAMPLE_PASS, "{out:?}");
}

#[test]
fn interpreter_outcomes_continue_after_failures_with_fresh_state() {
    let f = Fixture::custom(
        "fn test_false() -> bool { return false; }\nfn test_error() -> i32 { return 1 / 0; }\nfn test_later() -> bool { let mut x = 0; while x < 2 { x = x + 1; } return x == 2; }\nfn test_negative() -> i32 { return -1; }\nfn test_wrap() -> i32 { return 256; }\nfn test_negative_wrap() -> i32 { return -256; }\nfn test_unit() -> () { return; }\nfn test_zero() -> i32 { return 0; }\n",
        None,
    );
    let out = f.test(&[]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(
        out.stdout,
        b"FAIL test_false\nFAIL test_error\nPASS test_later\nFAIL test_negative\nFAIL test_wrap\nFAIL test_negative_wrap\nPASS test_unit\nPASS test_zero\ntest result: 3 passed; 5 failed; 8 total\n",
        "{out:?}"
    );
    assert!(text(&out.stderr).contains("zero"), "{out:?}");
    assert!(!f.path("example/.oxid").exists());
}

#[test]
fn preflight_rejections_precede_first_case_on_both_engines() {
    let cases: [(&str, Option<&str>, &str); 6] = [
        ("fn helper() -> i32 { return 0; }\n", None, "no root test_"),
        (
            "fn test_first() -> bool { return true; }\nfn test_bad(x: i32) -> i32 { return x; }\n",
            None,
            "zero parameters",
        ),
        (
            "fn test_first() -> bool { return true; }\nfn unused() -> i32 { return true; }\n",
            None,
            "E0300",
        ),
        (
            "use std::io::read_stdin as input;\nfn test_first() -> bool { return true; }\n",
            None,
            "read_stdin function imports",
        ),
        (
            "mod answer;\nfn test_first() -> bool { return true; }\n",
            Some("use std::io::read_stdin as unused;\npub fn value() -> i32 { return 7; }\n"),
            "read_stdin function imports",
        ),
        (
            "use std::io::write_stdout;\nfn test_first() -> bool { return true; }\n",
            None,
            "process",
        ),
    ];
    for (main, answer, message) in cases {
        let f = Fixture::custom(main, answer);
        for engine in ["interpreter", "native"] {
            let out = f.test(&["--engine", engine]);
            refusal(&out, message);
            assert!(!f.path("example/.oxid").exists(), "{out:?}");
        }
    }
    // ReadStatus alone remains legal: interpreter passes, and native only
    // reaches the (absent) toolchain after preflight has accepted the suite.
    let f = Fixture::custom(
        "use std::io::ReadStatus;\nfn test_ok() -> bool { return true; }\n",
        None,
    );
    let out = f.test(&[]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(
        out.stdout,
        b"PASS test_ok\ntest result: 1 passed; 0 failed; 1 total\n"
    );
    refusal(&f.test(&["--engine", "native"]), "E0701");
}

#[test]
fn json_is_rejected_before_manifest_io_on_both_engines() {
    let f = Fixture::new(None);
    fs::remove_file(f.path("example/oxid.toml")).unwrap();
    for engine in ["interpreter", "native"] {
        let out = f.test(&["--engine", engine, "--message-format=json"]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stderr.is_empty(), "{out:?}");
        let body = text(&out.stdout);
        assert_eq!(body.lines().count(), 1, "{body}");
        assert!(body.contains("\"code\":\"E0001\""), "{body}");
        assert!(
            body.contains("does not support --message-format=json"),
            "{body}"
        );
        assert!(!body.contains("cannot read"), "{body}");
        assert!(!body.contains("test result"), "{body}");
    }
    assert!(!f.path("example/.oxid").exists());
}

#[test]
fn native_without_toolchain_does_not_fallback_or_touch_app_outputs() {
    let f = Fixture::new(None);
    for mode in ["result", "process"] {
        let dir = f.path(&format!("example/.oxid/typed-preview/{mode}"));
        fs::create_dir_all(dir.join(format!(".{NAME}.build"))).unwrap();
        fs::write(dir.join(NAME), "existing application").unwrap();
        fs::write(dir.join(format!(".{NAME}.build/owner")), "existing receipt").unwrap();
    }
    let before: Vec<_> = ["result", "process"]
        .iter()
        .map(|mode| identity(&f.app_out(mode)))
        .collect();
    refusal(&f.test(&["--engine", "native"]), "E0701");
    let after: Vec<_> = ["result", "process"]
        .iter()
        .map(|mode| identity(&f.app_out(mode)))
        .collect();
    assert_eq!(before, after);
    // No fallback: the interpreter still has to be asked for explicitly.
    let out = f.test(&[]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, EXAMPLE_PASS);
}

#[test]
fn ordinary_new_check_run_regression_without_native() {
    let f = Fixture::new(None);
    let created = f.oxid(&["new", "--edition", "typed-preview", "app"]);
    assert_eq!(created.status.code(), Some(0), "{created:?}");
    assert_eq!(created.stdout, b"created typed project: \"app\"\n");
    let check = f.oxid(&["check", "--manifest-path", "app/oxid.toml"]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    let run = f.oxid(&["run", "--manifest-path", "app/oxid.toml"]);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert_eq!(run.stdout, b"7\n");
    assert!(!f.path("app/.oxid").exists());
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

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_matches_interpreter_on_example_and_fixed_outcomes() {
    let f = Fixture::native();
    let interpreter = f.test(&[]);
    let native = f.test(&["--engine", "native"]);
    assert_eq!(native.status.code(), Some(0), "{native:?}");
    assert_eq!(native.stdout, EXAMPLE_PASS, "{native:?}");
    assert_eq!(native.stdout, interpreter.stdout);
    assert!(native.stderr.is_empty(), "{native:?}");
    for ordinal in 0..3 {
        assert!(f
            .path(&format!("example/.oxid/typed-preview/test/case-{ordinal}"))
            .is_file());
        assert!(f
            .path(&format!(
                "example/.oxid/typed-preview/test/.case-{ordinal}.build/owner"
            ))
            .is_file());
    }
    // Fixed mixed outcomes: native and interpreter agree exactly on the
    // reported cases, order and totals, including a runtime failure.
    f.write(
        "example/src/main.ox",
        "fn test_false() -> bool { return false; }\nfn test_error() -> i32 { return 1 / 0; }\nfn test_later() -> bool { return true; }\nfn test_negative() -> i32 { return -256; }\nfn test_zero() -> i32 { return 0; }\n",
    );
    let interpreter = f.test(&[]);
    let native = f.test(&["--engine", "native"]);
    assert_eq!(interpreter.status.code(), Some(1), "{interpreter:?}");
    assert_eq!(
        native.status.code(),
        interpreter.status.code(),
        "{native:?}"
    );
    assert_eq!(native.stdout, interpreter.stdout, "{native:?}");
    assert_eq!(
        native.stdout,
        b"FAIL test_false\nFAIL test_error\nPASS test_later\nFAIL test_negative\nPASS test_zero\ntest result: 2 passed; 3 failed; 5 total\n"
    );
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_prepares_every_artifact_before_first_case_executes() {
    // A later unowned publication target must abort the whole native run
    // before the first (runtime-failing) case can execute or report.
    let f = Fixture::custom_native(
        "fn test_first() -> i32 { return 1 / 0; }\nfn test_second() -> bool { return true; }\n",
        None,
    );
    let dir = f.path("example/.oxid/typed-preview/test");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("case-1"), "unowned").unwrap();
    let out = f.test(&["--engine", "native"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(text(&out.stderr).contains("unowned"), "{out:?}");
    assert!(!text(&out.stderr).contains("division by zero"), "{out:?}");
    assert!(dir.join("case-0").is_file());
    assert_eq!(fs::read(dir.join("case-1")).unwrap(), b"unowned");
    // A later case the native backend cannot admit likewise prevents the
    // first case from reporting PASS; the interpreter reports both cases.
    f.write(
        "example/src/main.ox",
        "fn test_first() -> bool { return true; }\nfn test_recursive() -> bool { return test_recursive(); }\n",
    );
    let _ = fs::remove_dir_all(f.path("example/.oxid"));
    let out = f.test(&["--engine", "native"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(text(&out.stderr).contains("E0700"), "{out:?}");
    let interpreter = f.test(&[]);
    assert_eq!(interpreter.status.code(), Some(1), "{interpreter:?}");
    assert!(
        text(&interpreter.stdout).contains("PASS test_first\n"),
        "{interpreter:?}"
    );
    assert!(
        text(&interpreter.stdout).contains("FAIL test_recursive\n"),
        "{interpreter:?}"
    );
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_test_leaves_app_artifacts_and_receipts_unchanged() {
    let f = Fixture::native();
    for mode in ["result", "process"] {
        let flag = format!("--entry-mode={mode}");
        let build = f.oxid(&["build", "--manifest-path", "example/oxid.toml", &flag]);
        assert_eq!(build.status.code(), Some(0), "{build:?}");
    }
    let app_state = || {
        ["result", "process"].map(|mode| {
            let state = f.path(&format!("example/.oxid/typed-preview/{mode}/.{NAME}.build"));
            (
                identity(&f.app_out(mode)),
                identity(&state.join("owner")),
                optional_identity(&state.join("current")),
                optional_identity(&state.join("next")),
            )
        })
    };
    let before = app_state();
    let out = f.test(&["--engine", "native"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, EXAMPLE_PASS);
    assert_eq!(before, app_state(), "first TEST changed application state");
    // Ordinary native RUN of main still works and reports only the result.
    let run = f.oxid(&[
        "run",
        "--manifest-path",
        "example/oxid.toml",
        "--engine",
        "native",
    ]);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert_eq!(run.stdout, b"7\n");
    assert_eq!(before[1], app_state()[1], "process app state changed");
    // Ordinary RUN may republish Result, so snapshot both modes again.
    let before_test = app_state();
    let out = f.test(&["--engine", "native"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, EXAMPLE_PASS);
    assert_eq!(
        before_test,
        app_state(),
        "second TEST changed application state"
    );
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn example_readme_commands_from_examples_projects() {
    let f = Fixture::native();
    fs::rename(f.path("example"), f.path("typed_manifest_test")).unwrap();
    for engine_args in [vec![], vec!["--engine", "native"]] {
        let mut args = vec!["test", "--manifest-path", "typed_manifest_test/oxid.toml"];
        args.extend(engine_args);
        let out = f.oxid(&args);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert_eq!(out.stdout, EXAMPLE_PASS, "{out:?}");
    }
    let run = f.oxid(&["run", "--manifest-path", "typed_manifest_test/oxid.toml"]);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert_eq!(run.stdout, b"7\n");
    let build = f.oxid(&["build", "--manifest-path", "typed_manifest_test/oxid.toml"]);
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    assert!(
        text(&build.stdout).contains("typed-preview native build ok"),
        "{build:?}"
    );
    let readme = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/typed_manifest_test/README.md"),
    )
    .unwrap();
    assert!(readme.contains("LLVM 19.1.7"));
    assert!(readme.contains("OXID_LLVM_BIN"));
    assert!(readme.contains("--engine native"));
    assert!(readme.contains("test result: 3 passed; 0 failed; 3 total"));
}
