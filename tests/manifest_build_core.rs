//! Ordinary manifest build integration. No qualification packages or LLVM mocks.
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
            "oxid-manifest-build-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let p = Self(root);
        p.manifest("name=\"app\"\n");
        fs::write(p.0.join("main.ox"), "fn main()->i32{return 7;}").unwrap();
        p
    }
    fn manifest(&self, extra: &str) {
        fs::write(
            self.0.join("oxid.toml"),
            format!("[project]\nedition=\"typed-preview\"\nentry=\"main.ox\"\n{extra}"),
        )
        .unwrap();
    }
    fn cmd(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .env("OXID_LLVM_BIN", self.0.join("missing-llvm"))
            .args(args)
            .output()
            .unwrap()
    }
    fn build(&self, extra: &[&str]) -> Output {
        let mut args = vec![
            "build",
            "--manifest-path=oxid.toml",
            "--message-format=json",
        ];
        args.extend_from_slice(extra);
        self.cmd(&args)
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn failure(out: Output, mode: &str, message: &str) -> String {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(message), "{text}");
    let summary = text.lines().last().unwrap();
    assert_eq!(summary, format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"build-summary\",\"success\":false,\"errors\":1,\"output\":null,\"entry_mode\":\"{mode}\"}}"));
    assert_eq!(text.matches("build-summary").count(), 1);
    assert!(!text.contains("compile-summary"));
    text
}
#[test]
fn build_cli_rejects_unsupported_options_before_manifest_io() {
    let p = Project::new();
    fs::remove_file(p.0.join("oxid.toml")).unwrap();
    for option in [
        "--output=out",
        "--backend=llvm",
        "--target=x86_64-unknown-linux-gnu",
        "--experimental-hir-import=x",
        "--experimental-hir-producers=x",
        "--experimental-lexical-provider=x",
        "--locked",
        "--offline",
        "--frozen",
        "--update",
    ] {
        let text = failure(p.build(&[option]), "result", "E0001");
        assert!(!text.contains("cannot read manifest"));
    }
    failure(p.build(&["main.ox"]), "result", "source operand");
    failure(
        p.cmd(&[
            "--manifest-path=oxid.toml",
            "build",
            "--message-format=json",
        ]),
        "result",
        "must follow build",
    );
    failure(
        p.build(&["--entry-mode=bad"]),
        "result",
        "unknown --entry-mode",
    );
    failure(
        p.build(&["--entry-mode=process", "--edition=bad"]),
        "process",
        "unknown edition",
    );
    assert!(!p.0.join(".oxid").exists());
}
#[test]
fn build_name_required_only_for_build_and_validated_before_source_loading() {
    let p = Project::new();
    p.manifest("");
    assert!(p
        .cmd(&["check", "--manifest-path=oxid.toml"])
        .status
        .success());
    assert!(p
        .cmd(&["run", "--manifest-path=oxid.toml"])
        .status
        .success());
    fs::remove_file(p.0.join("main.ox")).unwrap();
    failure(p.build(&[]), "result", "requires [project].name");
    for name in ["", "-bad", "../bad", "has space", "é", &"a".repeat(65)] {
        p.manifest(&format!("name=\"{name}\"\n"));
        failure(p.build(&[]), "result", "name must match");
    }
    for name in ["A0_-", &"a".repeat(64)] {
        p.manifest(&format!("name=\"{name}\"\n"));
        failure(p.build(&[]), "result", "E0002");
    }
    assert!(!p.0.join(".oxid").exists());
}
#[test]
fn build_type_and_native_entry_failures_precede_managed_output() {
    let p = Project::new();
    fs::write(p.0.join("main.ox"), "fn main()->i32{return true;}").unwrap();
    failure(p.build(&[]), "result", "E0300");
    assert!(!p.0.join(".oxid").exists());
    fs::write(p.0.join("main.ox"), "fn helper()->i32{return 7;}").unwrap();
    failure(p.build(&[]), "result", "main");
    assert!(!p.0.join(".oxid").exists());
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn build_missing_toolchain_reports_build_only_and_preserves_unrelated_output() {
    let p = Project::new();
    failure(p.build(&[]), "result", "E0701");
    assert!(!p.0.join(".oxid/typed-preview/result/app").exists());
    assert!(!p
        .0
        .join(".oxid/typed-preview/result/.app.build/stage")
        .exists());
    fs::write(
        p.0.join(".oxid/typed-preview/result/app"),
        b"keep unrelated",
    )
    .unwrap();
    failure(p.build(&[]), "result", "unowned");
    assert_eq!(
        fs::read(p.0.join(".oxid/typed-preview/result/app")).unwrap(),
        b"keep unrelated"
    );
    failure(p.build(&["--entry-mode=process"]), "process", "E0701");
}
#[test]
fn bare_build_and_separator_keep_existing_routes() {
    let p = Project::new();
    let bare = p.cmd(&["build", "--locked", "--unknown"]);
    assert!(String::from_utf8_lossy(&bare.stderr).contains("unknown dependency option"));
    let literal = p.cmd(&[
        "check",
        "--edition=typed-preview",
        "--",
        "--manifest-path=oxid.toml",
    ]);
    assert!(String::from_utf8_lossy(&literal.stderr).contains("E0002"));
    assert!(!p.0.join(".oxid").exists());
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn low_level_compile_output_still_never_replaces_existing_file() {
    let p = Project::new();
    fs::write(p.0.join("sentinel"), b"keep").unwrap();
    let output = p.cmd(&[
        "compile",
        "main.ox",
        "--edition=typed-preview",
        "--backend=llvm",
        "--output=sentinel",
        "--message-format=json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("native output already exists"));
    assert!(text.contains("compile-summary"));
    assert!(!text.contains("build-summary"));
    assert_eq!(fs::read(p.0.join("sentinel")).unwrap(), b"keep");
}
