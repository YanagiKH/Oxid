//! Ordinary explicit-manifest CHECK controls; no capacity/fuel/native probes.
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const MANIFEST: &str = "[project]\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n";
const MAIN: &str = "mod child; fn helper()->i32{return crate::child::value();}";
const CHILD: &str = "pub fn value()->i32{return 7;}";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-manifest-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let result = Self(path);
        result.write("oxid.toml", MANIFEST);
        result.write("src/main.ox", MAIN);
        result.write("src/child.ox", CHILD);
        for name in [
            "oxid.lock",
            ".oxid/keep",
            "target/keep",
            "cache/keep",
            "existing.oxbc",
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
    fn command(&self, args: &[&str]) -> Output {
        let before = snapshot(&self.0);
        let mut child = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .env("PATH", self.0.join("absent-tools"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
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
                assert_eq!(before, snapshot(&self.0), "CHECK changed fixture files");
                return out;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("manifest CHECK timed out");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn check(&self) -> Output {
        self.command(&[
            "check",
            "--manifest-path=oxid.toml",
            "--message-format=json",
        ])
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
fn refusal(out: Output, code: &str, stage: &str, message: &str, primary_null: bool) -> String {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    let diagnostic = lines[0];
    assert!(diagnostic.contains("\"kind\":\"diagnostic\""));
    assert!(
        diagnostic.contains(&format!("\"code\":\"{code}\"")),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains(&format!("\"stage\":\"{stage}\"")),
        "{diagnostic}"
    );
    assert!(diagnostic.contains(message), "{diagnostic}");
    assert!(!diagnostic.contains("\"success\":"));
    assert!(!diagnostic.contains("\"functions\":"));
    assert_eq!(
        diagnostic.contains("\"primary\":null"),
        primary_null,
        "{diagnostic}"
    );
    assert_eq!(lines[1], "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}");
    stdout
}
#[test]
#[cfg(target_os = "linux")]
fn manifest_two_file_check_matches_source_route_and_no_writes() {
    let fixture = Fixture::new();
    // An invalid nested manifest is never consulted.
    fixture.write("src/oxid.toml", "[scripts]\nforbidden=\"anything\"");
    let manifest = fixture.check();
    assert_eq!(manifest.status.code(), Some(0), "{manifest:?}");
    assert!(manifest.stderr.is_empty());
    assert_eq!(manifest.stdout, b"{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":true,\"errors\":0,\"functions\":2}\n");
    let source = fixture.command(&[
        "--edition=typed-preview",
        "check",
        "src/main.ox",
        "--message-format=json",
    ]);
    assert_eq!(manifest.stdout, source.stdout);
    assert_eq!(manifest.status.code(), source.status.code());
    let matching = fixture.command(&[
        "--edition=typed-preview",
        "check",
        "--manifest-path",
        "oxid.toml",
        "--message-format=json",
    ]);
    assert_eq!(manifest.stdout, matching.stdout);
}
#[test]
fn manifest_module_free_without_main_and_text_rendering() {
    let fixture = Fixture::new();
    fixture.write("src/main.ox", "fn helper()->i32{return 7;}");
    let out = fixture.command(&["check", "--manifest-path=oxid.toml"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    assert_eq!(
        out.stdout,
        b"typed-preview check ok (1 functions; check only)\n"
    );
    let fail = fixture.command(&[
        "check",
        "--manifest-path=missing.toml",
        "--message-format=bad",
    ]);
    assert_eq!(fail.status.code(), Some(1));
    assert!(fail.stdout.is_empty());
    assert!(String::from_utf8(fail.stderr)
        .unwrap()
        .contains("unknown --message-format"));
}
#[test]
#[cfg(target_os = "linux")]
fn manifest_child_errors_and_unused_child_are_preserved() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.0.join("src/child.ox")).unwrap();
    refusal(fixture.check(), "E0002", "source", "src/main.ox", false);
    fixture.write("src/child.ox", "pub fn value()->i32{return true;}");
    let manifest = refusal(fixture.check(), "E0300", "type", "child.ox", false);
    let source = fixture.command(&[
        "--edition=typed-preview",
        "check",
        "src/main.ox",
        "--message-format=json",
    ]);
    assert_eq!(manifest.as_bytes(), source.stdout);
    fixture.write("src/main.ox", "mod child; fn helper()->i32{return 7;}");
    refusal(fixture.check(), "E0300", "type", "child.ox", false);
}
#[test]
fn manifest_cli_precedes_io_and_manifest_precedes_source() {
    let fixture = Fixture::new();
    refusal(
        fixture.command(&[
            "check",
            "--manifest-path=missing.toml",
            "--edition=typed-preview",
            "--edition=typed-preview",
            "--message-format=json",
        ]),
        "E0001",
        "cli",
        "--edition may only be specified once",
        true,
    );
    refusal(
        fixture.command(&["check", "--manifest-path=", "--message-format=json"]),
        "E0001",
        "cli",
        "--manifest-path requires a value",
        true,
    );
    refusal(
        fixture.command(&[
            "check",
            "--manifest-path=missing.toml",
            "--message-format=bad",
            "--message-format=json",
        ]),
        "E0001",
        "cli",
        "unknown --message-format",
        true,
    );
    fs::remove_file(fixture.0.join("src/main.ox")).unwrap();
    fixture.write(
        "oxid.toml",
        "[project]\nedition=\"legacy-0.9\"\nentry=\"absent.ox\"",
    );
    refusal(
        fixture.check(),
        "E0001",
        "cli",
        "requires [project].edition",
        true,
    );
    fixture.write("oxid.toml", MANIFEST);
    refusal(fixture.check(), "E0002", "source", "main.ox", true);
    fixture.write(
        "oxid.toml",
        &format!("{MANIFEST}[scripts]\nprobe=\"touch forbidden\""),
    );
    refusal(
        fixture.check(),
        "E0001",
        "cli",
        "unsupported manifest section",
        true,
    );
}
#[test]
fn manifest_file_size_utf8_and_nonregular_refusals() {
    let fixture = Fixture::new();
    fixture.write("src/main.ox", "fn helper()->i32{return 7;}");
    let mut bytes = MANIFEST.as_bytes().to_vec();
    bytes.push(b'#');
    bytes.resize(16_384, b' ');
    fs::write(fixture.0.join("oxid.toml"), &bytes).unwrap();
    assert_eq!(fixture.check().status.code(), Some(0));
    bytes[0] = 0xff;
    bytes.push(b'x');
    fs::write(fixture.0.join("oxid.toml"), &bytes).unwrap();
    refusal(
        fixture.check(),
        "E0400",
        "manifest-input",
        "exceeds 16384",
        true,
    );
    bytes.pop();
    fs::write(fixture.0.join("oxid.toml"), &bytes).unwrap();
    refusal(fixture.check(), "E0003", "source", "not valid UTF-8", true);
    refusal(
        fixture.command(&["check", "--manifest-path=src", "--message-format=json"]),
        "E0002",
        "source",
        "regular file",
        true,
    );
}
#[test]
#[cfg(unix)]
fn manifest_symlink_uses_lexical_parent_and_dangling_refuses() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    fixture.write("cfg/src/main.ox", "fn selected()->i32{return 3;}");
    fixture.write("other/settings.toml", MANIFEST);
    fixture.write("other/src/main.ox", "not valid source");
    symlink("../other/settings.toml", fixture.0.join("cfg/oxid.toml")).unwrap();
    let out = fixture.command(&[
        "check",
        "--manifest-path=./cfg/oxid.toml",
        "--message-format=json",
    ]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("\"functions\":1"));
    fs::remove_file(fixture.0.join("other/settings.toml")).unwrap();
    refusal(
        fixture.command(&[
            "check",
            "--manifest-path=cfg/oxid.toml",
            "--message-format=json",
        ]),
        "E0002",
        "source",
        "cannot read manifest",
        true,
    );
}
#[test]
fn manifest_after_separator_stays_literal_source_operand() {
    let fixture = Fixture::new();
    let out = fixture.command(&[
        "--edition=typed-preview",
        "check",
        "--",
        "--manifest-path=absent.toml",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("E0002"));
    assert!(err.contains("--manifest-path=absent.toml"));
}

#[test]
#[cfg(target_os = "linux")]
fn manifest_path_spelling_and_case_diagnostics_match_source() {
    let fixture = Fixture::new();
    fs::rename(
        fixture.0.join("src/child.ox"),
        fixture.0.join("src/Child.ox"),
    )
    .unwrap();
    for (manifest, source) in [
        ("oxid.toml", "src/main.ox"),
        ("./oxid.toml", "./src/main.ox"),
    ] {
        let observed = refusal(
            fixture.command(&[
                "check",
                "--manifest-path",
                manifest,
                "--message-format=json",
            ]),
            "E0005",
            "source",
            "case",
            false,
        );
        let explicit = fixture.command(&[
            "--edition=typed-preview",
            "check",
            source,
            "--message-format=json",
        ]);
        assert_eq!(observed.as_bytes(), explicit.stdout);
    }
}
#[test]
fn manifest_selector_absent_preserves_bare_check() {
    let fixture = Fixture::new();
    let out = fixture.command(&["check"]);
    assert!(!out.status.success());
    assert!(!String::from_utf8_lossy(&out.stdout).contains("check ok"));
    assert!(String::from_utf8_lossy(&out.stderr).contains("path"));
}

#[test]
fn manifest_script_selector_refuses_before_legacy_dispatch() {
    let fixture = Fixture::new();
    fixture.write(
        "oxid.toml",
        "[scripts]\n--manifest-path=\"touch forbidden\"\n",
    );
    for words in [
        vec!["script", "--manifest-path", "./pkg/oxid.toml", "echo"],
        vec!["script", "--manifest-path=./pkg/oxid.toml", "echo"],
        vec![
            "--manifest-path=p",
            "script",
            "--output",
            "x",
            "--message-format=json",
        ],
        vec![
            "--manifest-path=p",
            "script",
            "--output",
            "--message-format=json",
        ],
    ] {
        let out = fixture.command(&words);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(
            err.contains("E0001") && err.contains("--manifest-path requires check"),
            "{words:?}: {err}"
        );
        assert!(!err.contains("requires a value"));
    }
    for words in [
        vec![
            "script",
            "--manifest-path",
            "./pkg/oxid.toml",
            "--message-format=json",
            "echo",
        ],
        vec![
            "script",
            "--manifest-path=./pkg/oxid.toml",
            "--message-format=json",
            "echo",
        ],
    ] {
        refusal(
            fixture.command(&words),
            "E0001",
            "cli",
            "--manifest-path requires check",
            true,
        );
    }
}

#[test]
fn manifest_local_options_do_not_hide_first_forwarded_word() {
    let fixture = Fixture::new();
    for local in [
        vec!["--output=unused"],
        vec!["--output", "unused"],
        vec!["--backend=unused"],
        vec!["--backend", "unused"],
        vec!["--target=unused"],
        vec!["--target", "unused"],
    ] {
        let mut words = vec!["--manifest-path=p"];
        words.extend_from_slice(&local);
        words.extend(["script", "echo", "--edition=bad", "--message-format=json"]);
        refusal(
            fixture.command(&words),
            "E0001",
            "cli",
            "unknown edition `bad`; expected legacy-0.9 or typed-preview",
            true,
        );

        // The same local tokens after script are its name/payload, including
        // when a conventional name has already been supplied.
        for prefix in [
            vec!["--manifest-path=p", "script"],
            vec!["--manifest-path=p", "script", "echo"],
        ] {
            let mut words = prefix;
            words.extend_from_slice(&local);
            words.extend(["echo", "--edition=bad", "--message-format=json"]);
            let out = fixture.command(&words);
            assert_eq!(out.status.code(), Some(1), "{words:?}");
            assert!(out.stdout.is_empty(), "{words:?}");
            let stderr = String::from_utf8(out.stderr).unwrap();
            assert!(stderr.contains("E0001"), "{words:?}: {stderr}");
            assert!(stderr.contains("--manifest-path requires check"));
            assert!(!stderr.contains("unknown edition"));
        }
    }
}

#[test]
fn manifest_global_errors_precede_local_and_operand_errors() {
    let fixture = Fixture::new();
    for words in [
        vec![
            "check",
            "--wat",
            "--edition=bad",
            "--manifest-path=p",
            "--message-format=json",
        ],
        vec![
            "check",
            "--output",
            "--edition=bad",
            "--manifest-path=p",
            "--message-format=json",
        ],
        vec![
            "check",
            "main.ox",
            "other.ox",
            "--edition=bad",
            "--manifest-path=p",
            "--message-format=json",
        ],
    ] {
        refusal(
            fixture.command(&words),
            "E0001",
            "cli",
            "unknown edition",
            true,
        );
    }
}
