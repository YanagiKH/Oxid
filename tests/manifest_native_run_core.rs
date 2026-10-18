//! Ordinary native-RUN routing and pre-launch failures. No LLVM substitute.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-native-run-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("project/src")).unwrap();
        let p = Self(root);
        p.manifest("name=\"app\"\n");
        p.source("mod answer; fn main()->i32{return crate::answer::value();}");
        fs::write(
            p.0.join("project/src/answer.ox"),
            "pub fn value()->i32{return 7;}",
        )
        .unwrap();
        p
    }
    fn manifest(&self, name: &str) {
        fs::write(
            self.0.join("project/oxid.toml"),
            format!("[project]\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n{name}"),
        )
        .unwrap();
    }
    fn source(&self, text: &str) {
        fs::write(self.0.join("project/src/main.ox"), text).unwrap();
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .env("OXID_LLVM_BIN", self.0.join("missing-llvm"))
            .args(args)
            .output()
            .unwrap()
    }
    fn run(&self, extra: &[&str]) -> Output {
        let mut args = vec!["run", "--manifest-path=project/oxid.toml"];
        args.extend_from_slice(extra);
        self.command(&args)
    }
    fn native(&self, extra: &[&str]) -> Output {
        let mut options = vec!["--engine=native"];
        options.extend_from_slice(extra);
        self.run(&options)
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn text_failure(out: Output, message: &str) -> String {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    let text = String::from_utf8(out.stderr).unwrap();
    assert!(text.contains(message), "{text}");
    assert!(!text.contains("build-summary"));
    assert!(!text.contains("native build ok"));
    text
}
#[test]
fn native_run_interpreter_default_and_explicit_engine_preserve_text_and_json() {
    let p = Project::new();
    p.manifest("");
    for format in ["text", "json"] {
        let format = format!("--message-format={format}");
        let implicit = p.run(&[&format]);
        let explicit = p.run(&["--engine", "interpreter", &format]);
        assert_eq!(implicit.status.code(), Some(0), "{implicit:?}");
        assert_eq!(implicit.status.code(), explicit.status.code());
        assert_eq!(implicit.stdout, explicit.stdout);
        assert_eq!(implicit.stderr, explicit.stderr);
        if format.ends_with("text") {
            assert_eq!(explicit.stdout, b"7\n");
        } else {
            assert!(String::from_utf8(explicit.stdout)
                .unwrap()
                .contains("\"result\":{\"type\":\"i32\",\"value\":7}"));
        }
    }
    for mode in ["result", "process"] {
        let mode = format!("--entry-mode={mode}");
        let implicit = p.run(&[&mode]);
        let explicit = p.run(&["--engine=interpreter", &mode]);
        assert_eq!(implicit.status.code(), Some(1));
        assert_eq!(implicit.stdout, explicit.stdout);
        assert_eq!(implicit.stderr, explicit.stderr);
    }
    assert!(!p.0.join("project/.oxid").exists());
}
#[test]
fn native_run_cli_errors_precede_manifest_and_source_io() {
    let p = Project::new();
    fs::remove_file(p.0.join("project/oxid.toml")).unwrap();
    for extra in [
        vec!["--message-format=json"],
        vec!["--entry-mode=process", "--message-format=json"],
    ] {
        let out = p.native(&extra);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stderr.is_empty());
        let text = String::from_utf8(out.stdout).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("\"code\":\"E0001\""));
        assert!(text.contains("native manifest run does not support --message-format=json"));
        assert!(text
            .lines()
            .last()
            .unwrap()
            .contains("\"kind\":\"run-summary\""));
        assert!(!text.contains("cannot read manifest"));
    }
    for (extra, expected) in [
        (
            vec!["--engine=native", "--engine=interpreter"],
            "--engine may only be specified once",
        ),
        (vec!["--engine=bad"], "unknown --engine"),
        (vec!["--engine"], "--engine requires a value"),
        (
            vec!["--engine=native", "--entry-mode=bad"],
            "unknown --entry-mode",
        ),
        (
            vec!["--engine=native", "--output=out"],
            "does not support backend",
        ),
        (
            vec!["--engine=native", "--", "arg"],
            "does not accept a source operand",
        ),
        (
            vec!["--engine=native", "--profile=release"],
            "unsupported option",
        ),
    ] {
        text_failure(p.run(&extra), expected);
    }
    for command in ["check", "build"] {
        text_failure(
            p.command(&[
                command,
                "--manifest-path=project/oxid.toml",
                "--engine=native",
            ]),
            "--engine is only supported by manifest run",
        );
    }
    assert!(!p.0.join("project/.oxid").exists());
}
#[test]
fn native_run_requires_build_name_and_preserves_module_diagnostics() {
    let p = Project::new();
    p.manifest("");
    text_failure(p.native(&[]), "manifest run requires [project].name");
    for name in ["../bad", "bad name", "-bad"] {
        p.manifest(&format!("name=\"{name}\"\n"));
        text_failure(p.native(&[]), "manifest run name must match");
    }
    p.manifest("name=\"app\"\n");
    fs::write(
        p.0.join("project/src/answer.ox"),
        "pub fn value()->i32{return true;}",
    )
    .unwrap();
    let text = text_failure(p.native(&[]), "E0300");
    assert!(text.contains("project/src/answer.ox"));
    assert!(!p.0.join("project/.oxid").exists());
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn native_run_failed_build_never_launches_previous_artifact() {
    use std::os::unix::fs::PermissionsExt;
    let p = Project::new();
    for mode in ["result", "process"] {
        let root = p.0.join(format!("project/.oxid/typed-preview/{mode}"));
        let state = root.join(".app.build");
        fs::create_dir_all(&state).unwrap();
        let old = b"#!/bin/sh\nprintf 'previous artifact ran\\n'\ntouch executed\n";
        fs::write(root.join("app"), old).unwrap();
        fs::set_permissions(root.join("app"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(state.join("owner"), b"oxid managed native output v1\n").unwrap();
        fs::hard_link(root.join("app"), state.join("current")).unwrap();
        let mode = format!("--entry-mode={mode}");
        p.source("fn main()->i32{return true;}");
        text_failure(p.native(&[&mode]), "E0300");
        p.source("fn main()->i32{return 7;}");
        text_failure(p.native(&[&mode]), "E0701");
        assert_eq!(fs::read(root.join("app")).unwrap(), old);
        assert!(!state.join("stage").exists());
        assert!(!p.0.join("executed").exists());
        assert!(!p.0.join("project/executed").exists());
    }
}
#[test]
fn native_run_option_does_not_select_source_or_legacy_routes() {
    let p = Project::new();
    let out = p.command(&[
        "run",
        "--edition=typed-preview",
        "project/src/main.ox",
        "--engine=native",
    ]);
    text_failure(out, "E0001");
    fs::write(p.0.join("legacy.ox"), "print(7);").unwrap();
    let plain = p.command(&["run", "legacy.ox"]);
    let extra = p.command(&["run", "legacy.ox", "--engine=native"]);
    assert_eq!(plain.status.code(), Some(0));
    assert_eq!(plain.status.code(), extra.status.code());
    assert_eq!(plain.stdout, extra.stdout);
    assert_eq!(plain.stderr, extra.stderr);
}
