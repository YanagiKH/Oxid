//! Public CLI regressions for the opt-in typed-preview boundary.
//! All effects are confined to unique temporary projects; no network is used.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);
const LEGACY_SOURCE: &str = "fn fs_write(path, text) { write_text(path, text); }\nfs_write(\"sentinel.txt\", \"executed\");\nprint 5 / 2;\n";
const TYPED_SOURCE: &str = "fn identity(value: bool) -> bool { return value; }\nfn main() -> () { identity(true); return; }\n";

struct Project(PathBuf);

impl Project {
    fn new(source: &str) -> Self {
        let sequence = NEXT_PROJECT.fetch_add(1, Ordering::Relaxed);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "oxid-edition-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create isolated edition project");
        fs::write(root.join("main.ox"), source).unwrap();
        fs::create_dir(root.join("tests")).unwrap();
        fs::write(root.join("tests/sentinel.ox"), LEGACY_SOURCE).unwrap();
        fs::create_dir(root.join(".oxid")).unwrap();
        fs::write(root.join(".oxid/keep.txt"), "do not clean").unwrap();
        fs::write(
            root.join("oxid.toml"),
            "[project]\nname = \"edition-fixture\"\nentry = \"main.ox\"\n[dependencies]\n",
        )
        .unwrap();
        Self(root)
    }

    fn command(&self, arguments: &[&str]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(arguments)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run Oxid CLI");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait().expect("poll Oxid CLI").is_some() {
                return child.wait_with_output().expect("collect Oxid CLI output");
            }
            if Instant::now() >= deadline {
                child
                    .kill()
                    .expect("stop unexpectedly persistent CLI command");
                let output = child.wait_with_output().expect("reap Oxid CLI");
                panic!("CLI did not reject promptly: {arguments:?}; output: {output:?}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn snapshot(&self) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                let relative = path.strip_prefix(root).unwrap().to_path_buf();
                if path.is_dir() {
                    out.insert(relative, None);
                    visit(root, &path, out);
                } else {
                    out.insert(relative, Some(fs::read(path).unwrap()));
                }
            }
        }
        let mut result = BTreeMap::new();
        visit(&self.0, &self.0, &mut result);
        result
    }

    fn rejected_without_effects(&self, arguments: &[&str], expected: &str) {
        let before = self.snapshot();
        let output = self.command(arguments);
        assert_eq!(output.status.code(), Some(1), "{arguments:?}: {output:?}");
        assert!(
            output.stdout.is_empty(),
            "unexpected output for {arguments:?}: {output:?}"
        );
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains(expected), "{arguments:?}: {diagnostic}");
        assert_eq!(
            self.snapshot(),
            before,
            "typed selection caused filesystem effects: {arguments:?}"
        );
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn typed_run_rejects_legacy_effects_and_direct_file_is_unavailable_at_any_option_placement() {
    for command in [&["run", "main.ox"][..], &["main.ox"][..]] {
        for position in std::iter::once(command.len()).chain(0..command.len()) {
            let project = Project::new(LEGACY_SOURCE);
            let mut args = command.to_vec();
            args.splice(position..position, ["--edition", "typed-preview"]);
            project.rejected_without_effects(
                &args,
                if command[0] == "run" {
                    "parameter requires an explicit type"
                } else {
                    "command `main.ox` is unavailable"
                },
            );
        }
    }
}

#[test]
fn typed_compile_without_backend_and_ast_never_create_artifacts_at_any_option_placement() {
    for command in ["compile", "ast"] {
        for position in 0..=2 {
            let project = Project::new(LEGACY_SOURCE);
            let mut args = vec![command, "main.ox", "-o", "output.oxb"];
            args.splice(position..position, ["--edition=typed-preview"]);
            project.rejected_without_effects(&args, "typed-preview");
        }
        let project = Project::new(LEGACY_SOURCE);
        project.rejected_without_effects(
            &[
                command,
                "main.ox",
                "-o",
                "output.oxb",
                "--edition",
                "typed-preview",
            ],
            "typed-preview",
        );
    }
}

#[test]
fn typed_project_and_other_operations_reject_before_any_effects() {
    let commands: &[&[&str]] = &[
        &["build"],
        &["install"],
        &["test"],
        &["watch", "main.ox"],
        &["new", "created-project"],
        &["init", "created-project"],
        &["clean"],
        &["doc"],
        &["doctor"],
        &["diagnose"],
        &["lock"],
        &["fetch"],
        &["update"],
        &["list"],
        &["remove", "absent"],
        &["add", "local", "./local"],
        &["bridge", "python"],
        &["web", "new", "created-project"],
        &["discord", "new", "created-project"],
        &["bench", "main.ox"],
        &["bootstrap"],
        &["self-compile"],
        &["self-host"],
        &["emit"],
        &["frontend"],
        &["lint"],
        &["module"],
        &["syntax"],
        &["interop"],
        &["inspect", "main.ox"],
        &["help"],
        &["--version"],
        &["repl"],
    ];
    for command in commands {
        for position in 0..=command.len() {
            let project = Project::new(LEGACY_SOURCE);
            let mut args = command.to_vec();
            args.splice(position..position, ["--edition", "typed-preview"]);
            project.rejected_without_effects(&args, "typed-preview");
        }
    }
}

#[test]
fn malformed_editions_fail_instead_of_running_legacy() {
    let selections: &[&[&str]] = &[
        &["--edition"],
        &["--edition="],
        &["--edition", "unknown"],
        &["--edition=unknown"],
        &["--edition", "--other"],
        &["--edition=typed-preview", "--edition=typed-preview"],
        &["--edition=legacy-0.9", "--edition=legacy-0.9"],
        &["--edition=legacy-0.9", "--edition=typed-preview"],
    ];
    for selection in selections {
        let project = Project::new(LEGACY_SOURCE);
        let mut args = vec!["run", "main.ox"];
        args.extend_from_slice(selection);
        project.rejected_without_effects(&args, "edition");
    }
}

#[test]
fn message_format_requires_typed_preview_and_a_valid_single_value() {
    let selections: &[&[&str]] = &[
        &["--message-format", "text"],
        &["--edition=legacy-0.9", "--message-format=text"],
        &["--edition=typed-preview", "--message-format"],
        &["--edition=typed-preview", "--message-format="],
        &["--edition=typed-preview", "--message-format=xml"],
        &[
            "--edition=typed-preview",
            "--message-format=text",
            "--message-format=text",
        ],
    ];
    for selection in selections {
        let project = Project::new(LEGACY_SOURCE);
        let mut args = vec!["check", "main.ox"];
        args.extend_from_slice(selection);
        project.rejected_without_effects(&args, "message-format");
    }
}

#[test]
fn typed_check_accepts_global_flag_placements_and_source_separator() {
    let arguments: &[&[&str]] = &[
        &["--edition", "typed-preview", "check", "main.ox"],
        &["check", "--edition=typed-preview", "main.ox"],
        &["check", "main.ox", "--edition", "typed-preview"],
        &[
            "--message-format=text",
            "check",
            "--edition=typed-preview",
            "main.ox",
        ],
        &["check", "--edition=typed-preview", "--", "main.ox"],
        &[
            "--edition=typed-preview",
            "check",
            "--message-format",
            "text",
            "--",
            "main.ox",
        ],
    ];
    for args in arguments {
        let project = Project::new(TYPED_SOURCE);
        let before = project.snapshot();
        let output = project.command(args);
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert_eq!(
            project.snapshot(),
            before,
            "typed check wrote files: {args:?}"
        );
    }
}

#[test]
fn typed_check_requires_exactly_one_source_and_rejects_unknown_options() {
    let arguments: &[&[&str]] = &[
        &["check", "--edition=typed-preview"],
        &["check", "main.ox", "extra.ox", "--edition=typed-preview"],
        &["check", "main.ox", "--unknown", "--edition=typed-preview"],
        &[
            "check",
            "--edition=typed-preview",
            "--",
            "main.ox",
            "--message-format=json",
        ],
    ];
    for args in arguments {
        Project::new(TYPED_SOURCE).rejected_without_effects(args, "check");
    }
}

#[test]
fn explicit_legacy_matches_default_run_check_compile_and_ast() {
    for command in [
        &["run", "main.ox"][..],
        &["main.ox"][..],
        &["check", "main.ox"][..],
        &["compile", "main.ox", "-o", "program.oxb"][..],
        &["ast", "main.ox", "-o", "program.oxa"][..],
    ] {
        let project = Project::new(LEGACY_SOURCE);
        let baseline = project.command(command);
        assert!(
            baseline.status.success(),
            "baseline {command:?}: {baseline:?}"
        );
        let expected_artifact = command
            .last()
            .filter(|last| last.ends_with(".oxb") || last.ends_with(".oxa"))
            .map(|last| fs::read(project.0.join(last)).unwrap());
        for position in 0..=command.len() {
            let mut args = command.to_vec();
            args.splice(position..position, ["--edition", "legacy-0.9"]);
            let explicit = project.command(&args);
            assert_eq!(
                explicit.status.code(),
                baseline.status.code(),
                "{args:?}: {explicit:?}"
            );
            assert_eq!(explicit.stdout, baseline.stdout, "{args:?}");
            assert_eq!(explicit.stderr, baseline.stderr, "{args:?}");
            if let Some(bytes) = &expected_artifact {
                assert_eq!(
                    &fs::read(project.0.join(command.last().unwrap())).unwrap(),
                    bytes
                );
            }
        }
        if command[0] == "run" || command[0] == "main.ox" {
            assert_eq!(baseline.stdout, b"2.5\n");
            assert_eq!(
                fs::read(project.0.join("sentinel.txt")).unwrap(),
                b"executed"
            );
        }
    }
}

#[test]
fn separator_keeps_later_edition_words_as_legacy_arguments() {
    let project = Project::new(LEGACY_SOURCE);
    let baseline = project.command(&["run", "main.ox"]);
    let separated = project.command(&["run", "main.ox", "--", "--edition", "typed-preview"]);
    assert_eq!(separated.status.code(), baseline.status.code());
    assert_eq!(separated.stdout, baseline.stdout);
    assert_eq!(separated.stderr, baseline.stderr);
    assert_eq!(
        fs::read(project.0.join("sentinel.txt")).unwrap(),
        b"executed"
    );
}

#[cfg(unix)]
#[test]
fn script_payload_is_not_reinterpreted_as_oxid_options() {
    let project = Project::new(LEGACY_SOURCE);
    fs::write(project.0.join("payload.sh"), "printf '%s\\n' \"$@\"\n").unwrap();
    fs::write(
        project.0.join("oxid.toml"),
        "[scripts]\npayload = \"sh payload.sh\"\n",
    )
    .unwrap();
    let output = project.command(&[
        "script",
        "payload",
        "--edition",
        "typed-preview",
        "--message-format=json",
        "--edition=invalid",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        output.stdout,
        b"--edition\ntyped-preview\n--message-format=json\n--edition=invalid\n"
    );
    assert!(output.stderr.is_empty());
    for args in [
        &["--edition=typed-preview", "script", "payload"][..],
        &["script", "--edition=typed-preview", "payload"][..],
    ] {
        project.rejected_without_effects(args, "typed-preview");
    }
    let output = project.command(&[
        "script",
        "--edition=legacy-0.9",
        "payload",
        "--edition=typed-preview",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"--edition=typed-preview\n");
}

#[test]
fn cli_errors_honor_json_even_when_the_invalid_option_comes_first() {
    let arguments: &[&[&str]] = &[
        &[
            "compile",
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ],
        &[
            "check",
            "main.ox",
            "--edition=unknown",
            "--message-format",
            "json",
        ],
        &["check", "main.ox", "--edition", "--message-format=json"],
        &["check", "main.ox", "--message-format=json"],
        &[
            "check",
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
            "--message-format=text",
        ],
    ];
    for args in arguments {
        let project = Project::new(LEGACY_SOURCE);
        let before = project.snapshot();
        let output = project.command(args);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        assert!(
            output.stderr.is_empty(),
            "JSON mode wrote text diagnostics: {args:?}: {output:?}"
        );
        let text = String::from_utf8(output.stdout).unwrap();
        let records = text.lines().collect::<Vec<_>>();
        assert_eq!(
            records.len(),
            2,
            "expected one diagnostic and one summary: {args:?}: {text}"
        );
        assert!(
            records
                .iter()
                .all(|line| line.starts_with('{') && line.ends_with('}')),
            "non-JSON prose: {text}"
        );
        assert!(records[0].contains("\"kind\":\"diagnostic\""), "{text}");
        assert!(records[0].contains("\"stage\":\"cli\""), "{text}");
        let summary = if args.first() == Some(&"compile") {
            "compile-summary"
        } else {
            "check-summary"
        };
        assert!(
            records[1].contains(&format!("\"kind\":\"{summary}\"")),
            "{text}"
        );
        assert!(records[1].contains("\"success\":false"), "{text}");
        assert_eq!(
            project.snapshot(),
            before,
            "JSON CLI error caused filesystem effects"
        );
    }
}

#[test]
fn typed_check_separator_allows_a_dash_prefixed_source_filename() {
    let project = Project::new(TYPED_SOURCE);
    fs::write(project.0.join("--edition=legacy-0.9"), TYPED_SOURCE).unwrap();
    let before = project.snapshot();
    let output = project.command(&[
        "--edition=typed-preview",
        "check",
        "--message-format=json",
        "--",
        "--edition=legacy-0.9",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"success\":true"), "{text}");
    assert_eq!(project.snapshot(), before);
}

#[test]
fn typed_format_rejects_legacy_syntax_without_effects_at_any_option_placement() {
    for position in 0..=2 {
        let project = Project::new(LEGACY_SOURCE);
        let before = project.snapshot();
        let mut args = vec!["fmt", "main.ox"];
        args.splice(position..position, ["--edition", "typed-preview"]);
        let output = project.command(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("parameter requires an explicit type")
        );
        assert_eq!(project.snapshot(), before);
    }
}
