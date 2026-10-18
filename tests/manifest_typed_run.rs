//! Ordinary explicit-manifest Result RUN controls (R1a); no capacity/fuel/native probes.
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const MANIFEST: &str = "[project]\nname = \"ordinary-r1a-example\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n";
const MAIN: &str = "mod answer;\nfn main() -> i32 {\n    return crate::answer::value();\n}\n";
const ANSWER: &str = "pub fn value() -> i32 {\n    return 7;\n}\n";
const RUN_FAILURE_SUMMARY: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":false,\"errors\":1,\"result\":null}";
const CHECK_FAILURE_SUMMARY: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}";

struct Fixture(PathBuf);
impl Fixture {
    /// Project lives under `<root>/example`, mirroring the contract acceptance.
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-manifest-run-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let result = Self(path);
        result.write("example/oxid.toml", MANIFEST);
        result.write("example/src/main.ox", MAIN);
        result.write("example/src/answer.ox", ANSWER);
        for name in [
            "example/oxid.lock",
            "example/.oxid/keep",
            "example/target/keep",
            "example/cache/keep",
            "example/existing.oxbc",
        ] {
            result.write(name, "keep existing output\n");
        }
        result
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn invoke(&self, cwd: &str, args: &[&str], stdin: Option<&[u8]>) -> Output {
        let cwd = self.0.join(cwd);
        let before = snapshot(&self.0);
        let mut child = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&cwd)
            .env("OXID_CACHE_DIR", self.0.join("example/cache"))
            .env_remove("OXID_PATH")
            .env("PATH", self.0.join("absent-tools"))
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(bytes) = stdin {
            let mut pipe = child.stdin.take().unwrap();
            pipe.write_all(bytes).unwrap();
        }
        let drain = |mut pipe: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                pipe.read_to_end(&mut bytes).unwrap();
                bytes
            })
        };
        let stdout = drain(Box::new(child.stdout.take().unwrap()));
        let stderr = drain(Box::new(child.stderr.take().unwrap()));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                let out = Output {
                    status,
                    stdout: stdout.join().unwrap(),
                    stderr: stderr.join().unwrap(),
                };
                assert_eq!(before, snapshot(&self.0), "RUN changed fixture files");
                return out;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("manifest RUN timed out");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn command(&self, args: &[&str]) -> Output {
        self.invoke("", args, None)
    }
    fn run(&self, json: bool) -> Output {
        let mut args = vec!["run", "--manifest-path=example/oxid.toml"];
        if json {
            args.push("--message-format=json");
        }
        self.command(&args)
    }
    fn source_run(&self, source: &str, json: bool) -> Output {
        let mut args = vec!["--edition=typed-preview", "run", source];
        if json {
            args.push("--message-format=json");
        }
        self.command(&args)
    }
    /// Complete stdout/stderr/status parity with the explicit source route.
    fn assert_parity(&self) -> (Output, Output) {
        let mut outputs = Vec::new();
        for json in [false, true] {
            let manifest = self.run(json);
            let source = self.source_run("example/src/main.ox", json);
            assert_eq!(manifest.status.code(), source.status.code(), "{json}");
            assert_eq!(manifest.stdout, source.stdout, "{json}");
            assert_eq!(manifest.stderr, source.stderr, "{json}");
            outputs.push(manifest);
        }
        let json = outputs.pop().unwrap();
        (outputs.pop().unwrap(), json)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(root: &Path, result: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                result.push((
                    path.clone(),
                    fs::read_link(path)
                        .unwrap()
                        .to_string_lossy()
                        .as_bytes()
                        .to_vec(),
                ));
            } else if meta.is_dir() {
                result.push((path.clone(), Vec::new()));
                walk(&path, result);
            } else {
                result.push((path.clone(), fs::read(path).unwrap()));
            }
        }
    }
    let mut result = Vec::new();
    walk(root, &mut result);
    result.sort();
    result
}
fn success_json(value: &str) -> String {
    format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{value}}}\n")
}
/// One ordinary JSON refusal: one diagnostic plus the named terminal summary.
fn refusal(out: &Output, code: &str, stage: &str, message: &str, summary: &str) {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    let stdout = String::from_utf8(out.stdout.clone()).unwrap();
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert!(lines[0].contains("\"kind\":\"diagnostic\""), "{stdout}");
    assert!(
        lines[0].contains(&format!("\"code\":\"{code}\",\"stage\":\"{stage}\"")),
        "{stdout}"
    );
    assert!(lines[0].contains(message), "{stdout}");
    assert_eq!(lines[1], summary, "{stdout}");
}

#[test]
#[cfg(target_os = "linux")]
fn manifest_run_two_module_normal_flow() {
    let fixture = Fixture::new();
    // A nested invalid manifest is never consulted.
    fixture.write("example/src/oxid.toml", "[scripts]\nforbidden=\"anything\"");
    // 1. CHECK keeps the existing two-function check-only result.
    let check = fixture.command(&["check", "--manifest-path=example/oxid.toml"]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    assert!(check.stderr.is_empty());
    assert_eq!(
        check.stdout,
        b"typed-preview check ok (2 functions; check only)\n"
    );
    // 2. RUN prints exactly the scalar, stderr empty, status 0.
    let run = fixture.run(false);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert!(run.stderr.is_empty());
    assert_eq!(run.stdout, b"7\n");
    // 3. JSON: exactly one successful run-summary, no check-summary before it.
    let json = fixture.run(true);
    assert_eq!(json.status.code(), Some(0));
    assert!(json.stderr.is_empty());
    assert_eq!(
        String::from_utf8(json.stdout).unwrap(),
        success_json("{\"type\":\"i32\",\"value\":7}")
    );
    fixture.assert_parity();
    // Selector spellings, redundant matching edition, globals before/after the
    // command word and a terminal empty separator all select the same RUN.
    for args in [
        vec!["run", "--manifest-path", "example/oxid.toml"],
        vec![
            "--edition=typed-preview",
            "run",
            "--manifest-path=example/oxid.toml",
        ],
        vec![
            "run",
            "--manifest-path=example/oxid.toml",
            "--edition",
            "typed-preview",
        ],
        vec!["run", "--manifest-path=example/oxid.toml", "--"],
        vec![
            "--message-format=text",
            "run",
            "--manifest-path=example/oxid.toml",
        ],
    ] {
        let out = fixture.command(&args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {out:?}");
        assert_eq!(out.stdout, b"7\n", "{args:?}");
        assert!(out.stderr.is_empty(), "{args:?}");
    }
    let out = fixture.command(&[
        "--message-format=json",
        "run",
        "--manifest-path=example/oxid.toml",
    ]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        success_json("{\"type\":\"i32\",\"value\":7}")
    );
    // 4. Edit only the child: a fresh checked reference run returns 9.
    fixture.write("example/src/answer.ox", &ANSWER.replace('7', "9"));
    let changed = fixture.run(false);
    assert_eq!(changed.status.code(), Some(0));
    assert!(changed.stderr.is_empty());
    assert_eq!(changed.stdout, b"9\n");
    // 5. Child type error: same diagnostic as the explicit source route; RUN
    // executes nothing and reports with the RUN summary.
    fixture.write("example/src/answer.ox", &ANSWER.replace("7", "true"));
    let (text, json) = fixture.assert_parity();
    assert_eq!(text.status.code(), Some(1));
    assert!(text.stdout.is_empty());
    refusal(&json, "E0300", "type", "answer.ox", RUN_FAILURE_SUMMARY);
    let check = fixture.command(&[
        "check",
        "--manifest-path=example/oxid.toml",
        "--message-format=json",
    ]);
    let source_check = fixture.command(&[
        "--edition=typed-preview",
        "check",
        "example/src/main.ox",
        "--message-format=json",
    ]);
    assert_eq!(check.stdout, source_check.stdout);
    refusal(&check, "E0300", "type", "answer.ox", CHECK_FAILURE_SUMMARY);
    // 6. Restore; a different cwd selects the same entry tree with the path
    // spelling appropriate to that invocation.
    fixture.write("example/src/answer.ox", ANSWER);
    for (cwd, manifest, source) in [
        ("example", "oxid.toml", "src/main.ox"),
        ("example", "./oxid.toml", "./src/main.ox"),
        ("example/src", "../oxid.toml", "../src/main.ox"),
    ] {
        let out = fixture.invoke(cwd, &["run", "--manifest-path", manifest], None);
        let explicit = fixture.invoke(cwd, &["--edition=typed-preview", "run", source], None);
        assert_eq!(
            out.status.code(),
            explicit.status.code(),
            "{cwd} {manifest}"
        );
        assert_eq!(out.stdout, explicit.stdout, "{cwd} {manifest}");
        assert_eq!(out.stderr, explicit.stderr, "{cwd} {manifest}");
        assert_eq!(out.status.code(), Some(0), "{cwd} {manifest}: {out:?}");
        assert!(out.stderr.is_empty(), "{cwd} {manifest}: {out:?}");
        assert_eq!(out.stdout, b"7\n", "{cwd} {manifest}");
        let check = fixture.invoke(cwd, &["check", "--manifest-path", manifest], None);
        let explicit_check =
            fixture.invoke(cwd, &["--edition=typed-preview", "check", source], None);
        assert_eq!(check.status.code(), explicit_check.status.code());
        assert_eq!(check.stdout, explicit_check.stdout);
        assert_eq!(check.stderr, explicit_check.stderr);
        assert_eq!(check.status.code(), Some(0), "{cwd} {manifest}: {check:?}");
        assert!(check.stderr.is_empty(), "{cwd} {manifest}: {check:?}");
        assert_eq!(
            check.stdout,
            b"typed-preview check ok (2 functions; check only)\n"
        );
    }
    let absolute = fixture.0.join("example/oxid.toml");
    let out = fixture.invoke(
        "example/src",
        &["run", "--manifest-path", absolute.to_str().unwrap()],
        None,
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    assert_eq!(out.stdout, b"7\n", "{out:?}");
}

#[test]
#[cfg(target_os = "linux")]
fn manifest_run_missing_children_and_unused_child_match_source() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.0.join("example/src/answer.ox")).unwrap();
    let (_, json) = fixture.assert_parity();
    refusal(
        &json,
        "E0002",
        "source",
        "declared module",
        RUN_FAILURE_SUMMARY,
    );
    // Unused declared child is still checked; RUN executes nothing.
    fixture.write(
        "example/src/answer.ox",
        "pub fn value() -> i32 { return true; }",
    );
    fixture.write(
        "example/src/main.ox",
        "mod answer;\nfn main() -> i32 { return 1; }\n",
    );
    let (text, json) = fixture.assert_parity();
    assert!(text.stdout.is_empty());
    refusal(&json, "E0300", "type", "answer.ox", RUN_FAILURE_SUMMARY);
    // Missing entry source after a valid manifest.
    fs::remove_file(fixture.0.join("example/src/main.ox")).unwrap();
    let (_, json) = fixture.assert_parity();
    refusal(&json, "E0002", "source", "main.ox", RUN_FAILURE_SUMMARY);
}

#[test]
fn manifest_run_result_values_match_scalar_serializer() {
    let fixture = Fixture::new();
    for (source, text, json) in [
        (
            "fn main() -> i32 { return 0; }",
            "0\n",
            "{\"type\":\"i32\",\"value\":0}",
        ),
        (
            "fn main() -> i32 { return -3; }",
            "-3\n",
            "{\"type\":\"i32\",\"value\":-3}",
        ),
        (
            "fn main() -> bool { return false; }",
            "false\n",
            "{\"type\":\"bool\",\"value\":false}",
        ),
        (
            "fn main() -> bool { return true; }",
            "true\n",
            "{\"type\":\"bool\",\"value\":true}",
        ),
        (
            "fn main() -> () { return (); }",
            "()\n",
            "{\"type\":\"unit\"}",
        ),
    ] {
        fixture.write("example/src/main.ox", source);
        let out = fixture.run(false);
        assert_eq!(out.status.code(), Some(0), "{source}: {out:?}");
        assert_eq!(String::from_utf8(out.stdout).unwrap(), text, "{source}");
        assert!(out.stderr.is_empty());
        let out = fixture.run(true);
        assert_eq!(out.status.code(), Some(0), "{source}");
        assert_eq!(String::from_utf8(out.stdout).unwrap(), success_json(json));
        fixture.assert_parity();
    }
}

#[test]
fn manifest_run_entry_distinction_and_runtime_error_parity() {
    let fixture = Fixture::new();
    // Helper-only project: CHECK succeeds, RUN is E0600 with the RUN summary.
    fixture.write("example/src/main.ox", "fn helper() -> i32 { return 7; }");
    let check = fixture.command(&["check", "--manifest-path=example/oxid.toml"]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    let (text, json) = fixture.assert_parity();
    assert!(text.stdout.is_empty());
    refusal(&json, "E0600", "oir-run", "main", RUN_FAILURE_SUMMARY);
    fixture.write(
        "example/src/main.ox",
        "fn main(x: i32) -> i32 { return x; }",
    );
    let (text, json) = fixture.assert_parity();
    assert_eq!(text.status.code(), Some(1), "{text:?}");
    assert!(text.stdout.is_empty());
    refusal(
        &json,
        "E0600",
        "oir-run",
        "main must have no parameters",
        RUN_FAILURE_SUMMARY,
    );
    fixture.write(
        "example/src/main.ox",
        "fn main() -> u8 { let n = 0; return n.to_u8_checked(); }",
    );
    let (text, json) = fixture.assert_parity();
    assert_eq!(text.status.code(), Some(1), "{text:?}");
    assert!(text.stdout.is_empty());
    refusal(
        &json,
        "E0600",
        "oir-run",
        "main must return bool, i32 or ()",
        RUN_FAILURE_SUMMARY,
    );
    // Small checked-arithmetic runtime error keeps its runtime code.
    fixture.write(
        "example/src/main.ox",
        "fn main() -> i32 { let x = 2147483647; return x + 1; }",
    );
    let (text, json) = fixture.assert_parity();
    assert_eq!(text.status.code(), Some(1), "{text:?}");
    assert!(text.stdout.is_empty());
    refusal(
        &json,
        "E0604",
        "oir-run",
        "checked i32 arithmetic overflow",
        RUN_FAILURE_SUMMARY,
    );
}

#[test]
#[cfg(target_os = "linux")]
fn manifest_run_child_main_is_not_root_entry() {
    let fixture = Fixture::new();
    fixture.write(
        "example/src/main.ox",
        "mod answer;\nfn helper() -> i32 { return crate::answer::main(); }\n",
    );
    fixture.write(
        "example/src/answer.ox",
        "pub fn main() -> i32 { return 7; }\n",
    );
    let (text, json) = fixture.assert_parity();
    assert!(text.stdout.is_empty());
    refusal(&json, "E0600", "oir-run", "main", RUN_FAILURE_SUMMARY);
}

#[test]
#[cfg(target_os = "linux")]
fn manifest_run_imported_main_is_not_root_entry() {
    let fixture = Fixture::new();
    fixture.write(
        "example/src/main.ox",
        "mod answer;\nuse crate::answer::main;\n",
    );
    fixture.write(
        "example/src/answer.ox",
        "pub fn main() -> i32 { return 7; }\n",
    );
    let (text, json) = fixture.assert_parity();
    assert!(text.stdout.is_empty());
    refusal(&json, "E0600", "oir-run", "main", RUN_FAILURE_SUMMARY);
}

#[test]
fn manifest_run_cli_precedence_and_summary_selection() {
    let fixture = Fixture::new();
    // Contract section 3 examples; JSON is selected before the boundary.
    for (args, code, stage, message, summary) in [
        (
            vec!["run", "--wat", "--manifest-path=missing.toml", "--edition=bad", "--message-format=json"],
            "E0001",
            "cli",
            "unknown edition `bad`",
            CHECK_FAILURE_SUMMARY,
        ),
        (
            vec!["run", "--manifest-path=", "--message-format=json"],
            "E0001",
            "cli",
            "--manifest-path requires a value",
            CHECK_FAILURE_SUMMARY,
        ),
        (
            vec!["run", "--manifest-path=missing.toml", "--output=out", "--message-format=json"],
            "E0001",
            "cli",
            "manifest run does not support entry-mode, backend, target, output or experimental selectors",
            RUN_FAILURE_SUMMARY,
        ),
        (
            vec!["run", "source.ox", "--manifest-path=missing.toml", "--output=out", "--message-format=json"],
            "E0001",
            "cli",
            "manifest run does not accept a source operand",
            RUN_FAILURE_SUMMARY,
        ),
        (
            vec!["run", "--manifest-path=missing.toml", "--message-format=json"],
            "E0002",
            "source",
            "cannot read manifest `missing.toml`",
            RUN_FAILURE_SUMMARY,
        ),
        (
            vec!["compile", "--manifest-path=missing.toml", "--message-format=json"],
            "E0001",
            "cli",
            "--manifest-path requires check, run or build",
            CHECK_FAILURE_SUMMARY,
        ),
        (
            vec!["--manifest-path=example/oxid.toml", "run", "--message-format=json"],
            "E0001",
            "cli",
            "--manifest-path must follow run",
            RUN_FAILURE_SUMMARY,
        ),
        (
            vec!["run", "--manifest-path=example/oxid.toml", "--edition=legacy-0.9", "--message-format=json"],
            "E0001",
            "cli",
            "manifest run conflicts with --edition legacy-0.9",
            RUN_FAILURE_SUMMARY,
        ),
        (
            vec!["run", "--manifest-path=example/oxid.toml", "--message-format=json", "--", "argument"],
            "E0001",
            "cli",
            "manifest run does not accept a source operand",
            RUN_FAILURE_SUMMARY,
        ),
        (
            vec!["run", "--manifest-path=example/oxid.toml", "--profile=release", "--message-format=json"],
            "E0001",
            "cli",
            "unsupported option for manifest run: --profile=release",
            RUN_FAILURE_SUMMARY,
        ),
        // Process entry is an ordinary manifest CLI refusal on stdout NDJSON,
        // not the Process safe-stderr protocol.
        (
            vec!["run", "--manifest-path=example/oxid.toml", "--entry-mode=process", "--message-format=json"],
            "E0001",
            "cli",
            "manifest run does not support entry-mode",
            RUN_FAILURE_SUMMARY,
        ),
        (
            vec!["run", "--manifest-path=example/oxid.toml", "--entry-mode=result", "--message-format=json"],
            "E0001",
            "cli",
            "manifest run does not support entry-mode",
            RUN_FAILURE_SUMMARY,
        ),
        (
            vec!["install", "--manifest-path=example/oxid.toml", "--message-format=json"],
            "E0001",
            "cli",
            "--manifest-path requires check, run or build",
            CHECK_FAILURE_SUMMARY,
        ),
        (
            vec!["check", "--manifest-path=missing.toml", "--message-format=json"],
            "E0002",
            "source",
            "cannot read manifest `missing.toml`",
            CHECK_FAILURE_SUMMARY,
        ),
    ] {
        let out = fixture.command(&args);
        refusal(&out, code, stage, message, summary);
        assert!(String::from_utf8_lossy(&out.stdout).contains("\"primary\":null"), "{args:?}");
    }
    // Text mode: human diagnostic on stderr, nothing on stdout.
    let out = fixture.command(&[
        "run",
        "--manifest-path=missing.toml",
        "--entry-mode=process",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .contains("error[E0001] (cli): manifest run does not support"));
}

#[test]
fn manifest_run_manifest_precedes_source_and_uses_run_wording() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.0.join("example/src/main.ox")).unwrap();
    for (manifest, message) in [
        (
            "[project]\nedition=\"legacy-0.9\"\nentry=\"src/main.ox\"",
            "manifest run requires [project].edition = \\\"typed-preview\\\"",
        ),
        (
            "[project]\nedition=\"typed-preview\"",
            "manifest run requires [project].entry",
        ),
        // Grammar failure wins before missing-field validation.
        (
            "[project]\nedition=typed-preview",
            "expected double-quoted value",
        ),
        (
            "[project]\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[scripts]\nprobe=\"touch forbidden\"",
            "unsupported manifest section for typed run: scripts",
        ),
        (
            "[project]\nedition=\"typed-preview\"\nentry=\"../main.ox\"",
            "manifest entry must be a nonempty relative .ox path",
        ),
    ] {
        fixture.write("example/oxid.toml", manifest);
        refusal(&fixture.run(true), "E0001", "cli", message, RUN_FAILURE_SUMMARY);
    }
    fixture.write("example/oxid.toml", MANIFEST);
    refusal(
        &fixture.run(true),
        "E0002",
        "source",
        "main.ox",
        RUN_FAILURE_SUMMARY,
    );
    // Nonregular manifest.
    refusal(
        &fixture.command(&[
            "run",
            "--manifest-path=example/src",
            "--message-format=json",
        ]),
        "E0002",
        "source",
        "regular file",
        RUN_FAILURE_SUMMARY,
    );
}

#[test]
#[cfg(unix)]
fn manifest_run_symlink_uses_lexical_parent() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    fixture.write("cfg/src/main.ox", "fn main() -> i32 { return 3; }");
    fixture.write("other/settings.toml", MANIFEST);
    fixture.write("other/src/main.ox", "not valid source");
    symlink("../other/settings.toml", fixture.0.join("cfg/oxid.toml")).unwrap();
    let out = fixture.command(&["run", "--manifest-path=./cfg/oxid.toml"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, b"3\n");
    fs::remove_file(fixture.0.join("other/settings.toml")).unwrap();
    refusal(
        &fixture.command(&[
            "run",
            "--manifest-path=cfg/oxid.toml",
            "--message-format=json",
        ]),
        "E0002",
        "source",
        "cannot read manifest",
        RUN_FAILURE_SUMMARY,
    );
}

#[test]
fn manifest_run_boundaries_keep_existing_routes() {
    let fixture = Fixture::new();
    fixture.write("example/src/main.ox", "fn main() -> i32 { return 5; }");
    // Selector after `--` stays a literal file-based typed RUN operand.
    let out = fixture.command(&[
        "--edition=typed-preview",
        "run",
        "--",
        "--manifest-path=missing.toml",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.contains("E0002") && err.contains("--manifest-path=missing.toml"),
        "{err}"
    );
    assert!(!err.contains("cannot read manifest"), "{err}");
    // File-based Result RUN is unchanged and needs no manifest.
    let out = fixture.source_run("example/src/main.ox", false);
    assert_eq!(out.stdout, b"5\n");
    // Bare run has no discovery even beside an oxid.toml.
    let out = fixture.invoke("example", &["--edition=typed-preview", "run"], None);
    assert!(!out.status.success());
    assert!(!String::from_utf8_lossy(&out.stdout).contains('5'));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("manifest"));
    // Selector inside script payload does not opt in; before the name it does.
    let out = fixture.command(&["--manifest-path=example/oxid.toml", "script", "echo"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .contains("--manifest-path requires check, run or build"));
}

#[test]
fn manifest_run_refuses_stdout_import_under_result() {
    let fixture = Fixture::new();
    fixture.write(
        "example/src/main.ox",
        "use std::io::write_stdout as unused; fn main() -> i32 { return 0; }",
    );
    let (text, json) = fixture.assert_parity();
    assert!(text.stdout.is_empty());
    refusal(&json, "E0609", "oir-owned-run", "", RUN_FAILURE_SUMMARY);
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn manifest_run_bounded_stdin_matches_source_route() {
    let fixture = Fixture::new();
    fixture.write(
        "example/src/main.ox",
        "use std::io::read_stdin as read;\nuse std::io::ReadStatus as Status;\nfn main() -> i32 {\n    let mut bytes = [0, 0, 0, 0];\n    let status = read(&mut bytes);\n    match status {\n        Status::Eof(count) => { return count; },\n        Status::Full => { return 4; },\n        Status::IoError => { return -5; },\n    }\n}\n",
    );
    for (input, expected) in [
        (&b"ab"[..], b"2\n".as_slice()),
        (&b""[..], b"0\n".as_slice()),
        (&b"abcdef"[..], b"4\n".as_slice()),
    ] {
        let manifest = fixture.invoke(
            "",
            &["run", "--manifest-path=example/oxid.toml"],
            Some(input),
        );
        let source = fixture.invoke(
            "",
            &["--edition=typed-preview", "run", "example/src/main.ox"],
            Some(input),
        );
        assert_eq!(manifest.status.code(), source.status.code(), "{input:?}");
        assert_eq!(manifest.stdout, source.stdout, "{input:?}");
        assert_eq!(manifest.stderr, source.stderr, "{input:?}");
        assert_eq!(manifest.status.code(), Some(0), "{manifest:?}");
        assert!(manifest.stderr.is_empty(), "{manifest:?}");
        assert_eq!(manifest.stdout, expected, "{input:?}");
    }
}
