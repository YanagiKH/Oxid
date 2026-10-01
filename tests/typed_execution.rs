//! The public typed-run boundary uses isolated, timeout-bounded subprocesses.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-run-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        Self(root)
    }
    fn command(&self, args: &[&str]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if child.try_wait().unwrap().is_some() {
                return child.wait_with_output().unwrap();
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                panic!(
                    "typed run timed out: {:?}",
                    child.wait_with_output().unwrap()
                );
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn run(&self, json: bool) -> Output {
        let mut args = vec!["run", "main.ox", "--edition=typed-preview"];
        if json {
            args.push("--message-format=json");
        }
        self.command(&args)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn summary(success: bool, errors: usize, result: &str) -> String {
    format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":{success},\"errors\":{errors},\"result\":{result}}}\n")
}
#[test]
fn scalar_results_have_exact_text_json_and_success_exit_including_false() {
    for (value, declaration, json) in [
        (
            "true",
            "fn main() -> bool { return true; }",
            "{\"type\":\"bool\",\"value\":true}",
        ),
        (
            "false",
            "fn main() -> bool { return false; }",
            "{\"type\":\"bool\",\"value\":false}",
        ),
        ("()", "fn main() -> () { return; }", "{\"type\":\"unit\"}"),
    ] {
        let fixture = Fixture::new(declaration);
        for _ in 0..2 {
            let out = fixture.run(false);
            assert_eq!(out.status.code(), Some(0), "{out:?}");
            assert_eq!(out.stdout, format!("{value}\n").as_bytes());
            assert!(out.stderr.is_empty());
            let out = fixture.run(true);
            assert_eq!(out.status.code(), Some(0), "{out:?}");
            assert_eq!(out.stdout, summary(true, 0, json).as_bytes());
            assert!(out.stderr.is_empty());
        }
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
}
#[test]
fn run_entry_is_required_only_after_whole_file_verification() {
    for (source, code, location) in [
        ("", "E0600", "\"primary\":null"),
        (
            "fn helper() -> bool { return true; }",
            "E0600",
            "\"primary\":null",
        ),
        (
            "fn main(x: bool) -> bool { return x; }",
            "E0600",
            "\"start\":3,\"end\":7",
        ),
        (
            "fn main(x: bool) -> bool { return (); }",
            "E0300",
            "\"stage\":\"type\"",
        ),
        (
            "fn main() -> bool { if true { return true; } else { return missing; } }",
            "E0200",
            "\"stage\":\"resolve\"",
        ),
    ] {
        let fixture = Fixture::new(source);
        let out = fixture.run(true);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert!(
            stdout.contains(code) && stdout.contains(location),
            "{stdout}"
        );
        assert!(stdout.ends_with(&summary(false, 1, "null")), "{stdout}");
        assert!(!stdout.contains("check-summary"));
        assert!(out.stderr.is_empty());
        let text = fixture.run(false);
        assert!(text.stdout.is_empty());
        assert_eq!(text.status.code(), Some(1));
        if code == "E0600" {
            let check = fixture.command(&["check", "main.ox", "--edition=typed-preview"]);
            assert_eq!(check.status.code(), Some(0), "{check:?}");
        }
    }
}
#[test]
fn run_options_keep_placements_separator_and_explicit_summary_boundary() {
    let fixture = Fixture::new("fn main() -> bool { return true; }");
    for args in [
        vec!["--edition", "typed-preview", "run", "main.ox"],
        vec!["run", "--edition=typed-preview", "main.ox"],
        vec!["run", "main.ox", "--edition", "typed-preview"],
        vec![
            "--message-format=text",
            "run",
            "--edition=typed-preview",
            "--",
            "main.ox",
        ],
    ] {
        let out = fixture.command(&args);
        assert_eq!(out.stdout, b"true\n", "{out:?}");
        assert!(out.status.success());
    }
    for extra in [
        vec![],
        vec!["main.ox", "other.ox"],
        vec!["main.ox", "--target=x"],
        vec!["--", "main.ox", "argument"],
    ] {
        let mut args = vec!["run", "--edition=typed-preview", "--message-format=json"];
        args.extend(extra);
        let out = fixture.command(&args);
        let text = String::from_utf8(out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(
            text.contains("E0001") && text.ends_with(&summary(false, 1, "null")),
            "{text}"
        );
    }
    for global in [
        vec!["--edition=nope"],
        vec!["--edition"],
        vec!["--edition=typed-preview", "--edition=typed-preview"],
        vec!["--edition=typed-preview", "--message-format=bad"],
    ] {
        let mut args = vec!["run", "main.ox", "--message-format=json"];
        args.extend(global);
        let out = fixture.command(&args);
        let text = String::from_utf8(out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(
            text.contains("\"kind\":\"check-summary\"") && !text.contains("run-summary"),
            "{text}"
        );
    }
}
#[test]
fn branches_calls_joins_and_isolated_recursive_frames_execute() {
    for (source, result) in [
        ("fn id(x: bool) -> bool { return x; } fn main() -> bool { if id(true) { return false; } else { return true; } }", "false"),
        ("fn main() -> bool { if false { return false; } let y = id((true)); if y { id(false); } else {} return y; } fn id(x: bool) -> bool { return x; }", "true"),
        ("fn bounce(x: bool) -> bool { if x { bounce(false); return x; } else { return x; } } fn main() -> bool { return bounce(true); }", "true"),
        ("fn a(x: bool) -> bool { if x { return b(false); } return true; } fn b(x: bool) -> bool { return a(x); } fn main() -> bool { return a(true); }", "true"),
        ("fn id(x: ()) -> () { let y: () = (x); return y; } fn main() -> () { let z = id(()); return z; }", "()"),
        ("fn forever() -> bool { return forever(); } fn main() -> bool { if false { return forever(); } else { return true; } }", "true"),
    ] { let out = Fixture::new(source).run(false); assert_eq!(out.status.code(), Some(0), "{out:?}"); assert_eq!(out.stdout, format!("{result}\n").as_bytes()); }
}
#[test]
fn discarded_calls_execute_and_first_argument_failure_keeps_its_exact_origin() {
    let source = "// é\r\nfn left() -> bool { return left(); }\r\nfn right() -> bool { return right(); }\r\nfn choose(a: bool, b: bool) -> bool { return b; }\r\nfn main() -> bool { choose(left(), right()); return false; }";
    let fixture = Fixture::new(source);
    let first = fixture.run(true);
    let text = String::from_utf8(first.stdout.clone()).unwrap();
    let start = source.find("left();").unwrap();
    assert_eq!(first.status.code(), Some(1));
    assert!(
        text.contains("E0602") && text.contains("\"stage\":\"oir-run\""),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "\"start\":{start},\"end\":{}",
            start + "left()".len()
        )),
        "{text}"
    );
    assert!(text.ends_with(&summary(false, 1, "null")));
    assert_eq!(first.stdout, fixture.run(true).stdout);
    let check = fixture.command(&["check", "main.ox", "--edition=typed-preview"]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
}
#[test]
fn unsupported_sources_and_artifacts_never_fallback_or_write_outputs() {
    for source in [
        "OXBCgarbage",
        "fn main() -> bool { return 1; }",
        "fn main() -> bool { return true == false; }",
        "import evil;",
        "fn main() -> () { print true; return; }",
        "fn main() -> () { write_text(); return; }",
    ] {
        let fixture = Fixture::new(source);
        let out = fixture.run(true);
        assert_eq!(out.status.code(), Some(1), "{source}: {out:?}");
        assert!(String::from_utf8(out.stdout)
            .unwrap()
            .ends_with(&summary(false, 1, "null")));
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
}

#[test]
fn run_source_read_errors_and_literal_dash_path_have_operation_correct_records() {
    let fixture = Fixture::new("fn main() -> bool { return false; }");
    let out = fixture.command(&[
        "run",
        "missing.ox",
        "--edition=typed-preview",
        "--message-format=json",
    ]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(text.contains("E0002") && text.ends_with(&summary(false, 1, "null")));
    fs::write(fixture.0.join("main.ox"), [0xff]).unwrap();
    let out = fixture.run(true);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("E0003") && text.ends_with(&summary(false, 1, "null")));
    fs::write(
        fixture.0.join("--main.ox"),
        "fn main() -> bool { return false; }",
    )
    .unwrap();
    let out = fixture.command(&["run", "--edition=typed-preview", "--", "--main.ox"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, b"false\n");
}
#[test]
fn production_fuel_exhaustion_is_an_ordinary_run_failure_after_check_succeeds() {
    let mut source = "fn f0() -> () { return; }".to_string();
    for i in 1..20 {
        source.push_str(&format!(
            "fn f{i}() -> () {{ f{}(); f{}(); return; }}",
            i - 1,
            i - 1
        ));
    }
    source.push_str("fn main() -> () { f19(); return; }");
    let fixture = Fixture::new(&source);
    let check = fixture.command(&[
        "check",
        "main.ox",
        "--edition=typed-preview",
        "--message-format=json",
    ]);
    assert_eq!(check.status.code(), Some(0));
    assert!(String::from_utf8(check.stdout)
        .unwrap()
        .contains("\"kind\":\"check-summary\""));
    let run = fixture.run(true);
    assert_eq!(run.status.code(), Some(1));
    let text = String::from_utf8(run.stdout).unwrap();
    assert!(
        text.contains("E0601") && text.ends_with(&summary(false, 1, "null")),
        "{text}"
    );
}
