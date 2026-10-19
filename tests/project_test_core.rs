//! Implementation controls; fufu's independent manifest_typed_test stays separate.
#![cfg(target_os = "linux")]
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-test-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("oxid.toml"),
            "[project]\nedition=\"typed-preview\"\nentry=\"app.ox\"\n",
        )
        .unwrap();
        fs::write(root.join("app.ox"), source).unwrap();
        Self(root)
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .env("OXID_LLVM_BIN", self.0.join("missing-llvm"))
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn test(&self, options: &[&str]) -> Output {
        let mut args = vec!["test", "--manifest-path=oxid.toml"];
        args.extend_from_slice(options);
        self.command(&args)
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn refusal(out: Output, expected: &str) {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains(expected),
        "{out:?}"
    );
}

#[test]
fn test_cli_reports_source_order_and_preserves_ordinary_main() {
    let p = Project::new("mod helper; fn main()->i32{return crate::helper::answer();} fn test_z()->bool{return crate::helper::answer()==7;} fn test_a()->(){return;} fn test_zero()->i32{return crate::helper::answer()-7;}");
    fs::write(
        p.0.join("helper.ox"),
        "pub fn answer()->i32{return 7;} fn test_child(x:i32)->i32{return x;}",
    )
    .unwrap();
    for options in [vec![], vec!["--engine", "interpreter"]] {
        let out = p.test(&options);
        assert_eq!(out.status.code(), Some(0));
        assert!(out.stderr.is_empty(), "{out:?}");
        assert_eq!(
            out.stdout,
            b"PASS test_z\nPASS test_a\nPASS test_zero\ntest result: 3 passed; 0 failed; 3 total\n"
        );
    }
    let run = p.command(&["run", "--manifest-path=oxid.toml"]);
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(run.stdout, b"7\n");
    assert_eq!(
        p.command(&["check", "--manifest-path=oxid.toml"])
            .status
            .code(),
        Some(0)
    );
    assert!(!p.0.join(".oxid").exists());
}

#[test]
fn test_cli_continues_after_runtime_and_logical_failures() {
    let p = Project::new("fn test_false()->bool{return false;} fn test_error()->i32{return 1/0;} fn test_later()->bool{return true;} fn test_negative()->i32{return -256;} fn test_large()->i32{return 2147483647;}");
    let out = p.test(&[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("zero"));
    assert_eq!(out.stdout, b"FAIL test_false\nFAIL test_error\nPASS test_later\nFAIL test_negative\nFAIL test_large\ntest result: 1 passed; 4 failed; 5 total\n");
}

#[test]
fn test_cli_preflight_is_shared_by_both_engines() {
    for (source, message) in [
        ("fn helper()->i32{return 0;}", "no root test_"),
        (
            "fn test_first()->bool{return true;} fn test_bad(x:i32)->i32{return x;}",
            "zero parameters",
        ),
        (
            "fn test_first()->bool{return true;} fn unused()->i32{return true;}",
            "E0300",
        ),
        (
            "use std::io::read_stdin as input; fn test_first()->bool{return true;}",
            "read_stdin function imports",
        ),
        (
            "use std::io::write_stdout; fn test_first()->bool{return true;}",
            "process",
        ),
    ] {
        let p = Project::new(source);
        for engine in ["interpreter", "native"] {
            refusal(p.test(&["--engine", engine]), message);
            assert!(!p.0.join(".oxid").exists());
        }
    }
    let p = Project::new("use std::io::ReadStatus; fn test_ok()->bool{return true;}");
    assert_eq!(p.test(&[]).status.code(), Some(0));
    refusal(p.test(&["--engine=native"]), "E0701");
}

#[test]
fn test_cli_json_and_invalid_options_are_rejected_before_io() {
    let p = Project::new("");
    fs::remove_file(p.0.join("oxid.toml")).unwrap();
    for engine in ["interpreter", "native"] {
        let out = p.test(&["--engine", engine, "--message-format=json"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stderr.is_empty());
        let text = String::from_utf8(out.stdout).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("\"code\":\"E0001\""));
        assert!(text.contains("does not support --message-format=json"));
        assert!(!text.contains("summary"));
        assert!(!text.contains("cannot read"));
    }
    for (options, message) in [
        (vec!["--engine=bad"], "unknown --engine"),
        (
            vec!["--engine=native", "--engine=interpreter"],
            "only be specified once",
        ),
        (vec!["--engine"], "requires a value"),
        (vec!["--entry-mode=process"], "does not support entry-mode"),
        (vec!["--not-a-test-option=test_a"], "unsupported option"),
        (vec!["--filter=a", "--filter=b"], "only be specified once"),
        (vec!["--filter"], "requires a value"),
        (vec!["--filter="], "requires a value"),
        (vec!["--filter", ""], "requires a value"),
        (vec!["--filter", "--engine=native"], "requires a value"),
        (vec!["--", "app.ox"], "does not accept a source operand"),
    ] {
        refusal(p.test(&options), message);
    }
    assert!(!p.0.join(".oxid").exists());
}

#[test]
fn test_cli_native_refusals_do_not_report_partial_success_or_touch_app_outputs() {
    let p = Project::new("fn test_ok()->bool{return true;} fn test_later()->bool{return true;}");
    for mode in ["result", "process"] {
        let dir = p.0.join(format!(".oxid/typed-preview/{mode}"));
        fs::create_dir_all(dir.join(".app.build")).unwrap();
        fs::write(dir.join("app"), "existing application").unwrap();
        fs::write(dir.join(".app.build/owner"), "existing receipt").unwrap();
    }
    refusal(p.test(&["--engine=native"]), "E0701");
    for mode in ["result", "process"] {
        let dir = p.0.join(format!(".oxid/typed-preview/{mode}"));
        assert_eq!(fs::read(dir.join("app")).unwrap(), b"existing application");
        assert_eq!(
            fs::read(dir.join(".app.build/owner")).unwrap(),
            b"existing receipt"
        );
    }
    fs::write(
        p.0.join("app.ox"),
        "fn test_ok()->bool{return true;} fn test_recursive()->bool{return test_recursive();}",
    )
    .unwrap();
    refusal(p.test(&["--engine=native"]), "E0700");
}

#[test]
fn test_cli_default_legacy_route_is_unchanged() {
    let p = Project::new("fn test_typed()->bool{return false;}");
    fs::create_dir(p.0.join("tests")).unwrap();
    fs::write(p.0.join("tests/plain.ox"), "print(23);").unwrap();
    let out = p.command(&["test"]);
    // Default legacy test discovery never selects project.entry as a typed suite.
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("23"));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("test_typed"));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("project-test"));
    assert!(!p.0.join(".oxid").exists());
}

#[test]
#[ignore = "requires separately installed LLVM 19.1.7 on Linux x86_64"]
fn test_cli_native_prepares_every_artifact_before_first_execution() {
    let p = Project::new("fn test_first()->i32{return 1/0;} fn test_second()->bool{return true;}");
    let dir = p.0.join(".oxid/typed-preview/test");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("case-1"), "unowned").unwrap();
    // Use the real configured toolchain. The first artifact can compile, but
    // publication of the second must fail before the first division executes.
    let out = Command::new(env!("CARGO_BIN_EXE_oxid"))
        .current_dir(&p.0)
        .args(["test", "--manifest-path=oxid.toml", "--engine=native"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("unowned"),
        "{out:?}"
    );
    assert!(!String::from_utf8_lossy(&out.stderr).contains("division by zero"));
    assert!(dir.join("case-0").is_file());
    assert_eq!(fs::read(dir.join("case-1")).unwrap(), b"unowned");
}

#[test]
fn test_cli_filter_selects_root_names_in_source_order_and_counts_only_matches() {
    let p = Project::new("mod helper; fn test_keep_z()->bool{return true;} fn test_skip()->bool{return false;} fn test_keep_a()->(){return;} fn test_KEEP()->bool{return false;}");
    fs::write(
        p.0.join("helper.ox"),
        "fn test_keep_child(x:i32)->i32{return x;}",
    )
    .unwrap();
    for options in [vec!["--filter", "keep"], vec!["--filter=keep"]] {
        let out = p.test(&options);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert!(out.stderr.is_empty(), "{out:?}");
        assert_eq!(
            out.stdout,
            b"PASS test_keep_z\nPASS test_keep_a\ntest result: 2 passed; 0 failed; 2 total\n"
        );
    }
    for engine in ["interpreter", "native"] {
        refusal(
            p.test(&["--engine", engine, "--filter=absent"]),
            "no tests match --filter",
        );
        refusal(
            p.test(&["--engine", engine, "--filter=child"]),
            "no tests match --filter",
        );
    }
    assert!(!p.0.join(".oxid").exists());
}

#[test]
fn test_cli_filter_does_not_hide_invalid_signatures_or_project_errors() {
    for (source, expected) in [
        (
            "fn test_keep()->bool{return true;} fn test_bad(x:i32)->i32{return x;}",
            "zero parameters",
        ),
        (
            "fn test_keep()->bool{return true;} fn unused()->i32{return true;}",
            "E0300",
        ),
    ] {
        let p = Project::new(source);
        for engine in ["interpreter", "native"] {
            for filter in ["keep", "absent"] {
                refusal(p.test(&["--engine", engine, "--filter", filter]), expected);
                assert!(!p.0.join(".oxid").exists());
            }
        }
    }
}

#[test]
fn test_cli_filtered_failures_still_continue() {
    let p = Project::new("fn test_skip()->bool{return false;} fn test_keep_false()->bool{return false;} fn test_keep_error()->i32{return 1/0;} fn test_keep_later()->bool{return true;}");
    let out = p.test(&["--filter=keep"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("zero"));
    assert_eq!(out.stdout, b"FAIL test_keep_false\nFAIL test_keep_error\nPASS test_keep_later\ntest result: 1 passed; 2 failed; 3 total\n");
}

#[test]
fn test_cli_filter_is_rejected_outside_manifest_test_before_effects() {
    let p = Project::new("");
    fs::remove_file(p.0.join("oxid.toml")).unwrap();
    for command in ["check", "run", "build", "package"] {
        let out = p.command(&[command, "--manifest-path=oxid.toml", "--filter=a"]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let expected_stdout: &[u8] = if command == "package" {
            b"typed-preview native package failed: managed executable not published; archive not published\n"
        } else {
            b""
        };
        assert_eq!(out.stdout.as_slice(), expected_stdout, "{out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr)
                .contains("only supported by explicit manifest test"),
            "{out:?}"
        );
    }
    for command in ["test", "check", "run", "build", "package"] {
        refusal(
            p.command(&[command, "--filter=a"]),
            "only supported by explicit manifest test",
        );
    }
    assert!(!p.0.join(".oxid").exists());
}

#[test]
#[ignore = "requires separately installed LLVM 19.1.7 on Linux x86_64"]
fn test_cli_native_filter_keeps_original_ordinals_and_preparation_barrier() {
    for blocked in [false, true] {
        let p = Project::new("fn test_skip()->bool{return true;} fn test_keep_fail()->bool{return false;} fn test_skip_later()->bool{return true;} fn test_keep_pass()->bool{return true;}");
        let dir = p.0.join(".oxid/typed-preview/test");
        if blocked {
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("case-3"), "unowned").unwrap();
        }
        let out = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&p.0)
            .args([
                "test",
                "--manifest-path=oxid.toml",
                "--engine=native",
                "--filter=keep",
            ])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        if blocked {
            assert!(out.stdout.is_empty(), "{out:?}");
            assert!(String::from_utf8_lossy(&out.stderr).contains("unowned"));
            assert_eq!(fs::read(dir.join("case-3")).unwrap(), b"unowned");
        } else {
            assert!(out.stderr.is_empty(), "{out:?}");
            assert_eq!(out.stdout, b"FAIL test_keep_fail\nPASS test_keep_pass\ntest result: 1 passed; 1 failed; 2 total\n");
            assert!(dir.join("case-3").is_file());
        }
        assert!(dir.join("case-1").is_file());
        assert!(!dir.join("case-0").exists());
        assert!(!dir.join("case-2").exists());
    }
}
