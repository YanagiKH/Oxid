//! Ordinary PACKAGE integration and opt-in native acceptance; no tool installation.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-package-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn project(&self) {
        self.write(
            "app/oxid.toml",
            "[project]\nname=\"demo\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n",
        );
        self.write(
            "app/main.ox",
            "mod helper; fn main()->i32{return crate::helper::answer();}",
        );
        self.write("app/helper.ox", "pub fn answer()->i32{return 7;}");
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args)
            .env("OXID_LLVM_BIN", self.0.join("absent-llvm"))
            .output()
            .unwrap()
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(args)
            .current_dir(&self.0)
            .env_remove("OXID_PATH")
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .stdin(Stdio::null());
        command
    }
    fn package(&self, extra: &[&str]) -> Output {
        let mut args = vec![
            "package",
            "--manifest-path=app/oxid.toml",
            "--output=bundle.tar",
            "--message-format=json",
        ];
        args.extend_from_slice(extra);
        self.run(&args)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn failed(output: Output, code: &str, text: &str, mode: &str) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let out = String::from_utf8(output.stdout).unwrap();
    assert!(out.contains(&format!("\"code\":\"{code}\"")), "{out}");
    assert!(out.contains(text), "{out}");
    assert_eq!(out.matches("package-summary").count(), 1, "{out}");
    assert!(
        !out.contains("build-summary") && !out.contains("check-summary"),
        "{out}"
    );
    let summary = out.lines().last().unwrap();
    assert!(summary.contains("\"success\":false"));
    assert!(summary.contains("\"managed_published\":false,\"managed_output\":null"));
    assert!(summary.contains("\"archive_published\":false,\"archive_output\":null"));
    assert!(summary.contains(&format!("\"entry_mode\":\"{mode}\"")));
    out
}
#[test]
fn package_cli_errors_precede_io_and_keep_global_precedence() {
    let f = Fixture::new();
    for (extra, expected) in [
        (vec![], "requires --output ARCHIVE"),
        (vec!["--output"], "--output requires a value"),
        (vec!["--output="], "--output requires a value"),
        (
            vec!["--output=a", "--output=b"],
            "--output may only be specified once",
        ),
        (
            vec!["--output=a", "--engine=native"],
            "--engine is only supported",
        ),
        (
            vec!["--output=a", "--target=host"],
            "does not support backend",
        ),
        (
            vec!["--output=a", "source.ox"],
            "does not accept a source operand",
        ),
        (
            vec!["--output=a", "--unknown", "--entry-mode=bad"],
            "unknown --entry-mode",
        ),
    ] {
        let mut args = vec![
            "package",
            "--manifest-path=missing",
            "--message-format=json",
        ];
        args.extend(extra);
        failed(f.run(&args), "E0001", expected, "result");
    }
    failed(
        f.run(&[
            "--manifest-path=missing",
            "package",
            "--output=a",
            "--message-format=json",
            "--entry-mode=process",
        ]),
        "E0001",
        "--manifest-path must follow package",
        "process",
    );
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
}
#[test]
fn package_output_preflight_refuses_files_directories_and_dangling_symlinks() {
    for kind in ["file", "directory", "dangling"] {
        let f = Fixture::new();
        let output = f.0.join("bundle.tar");
        match kind {
            "file" => fs::write(&output, b"keep").unwrap(),
            "directory" => fs::create_dir(&output).unwrap(),
            _ => symlink("missing-target", &output).unwrap(),
        }
        failed(f.package(&[]), "E0720", "already exists", "result");
        match kind {
            "file" => assert_eq!(fs::read(output).unwrap(), b"keep"),
            "directory" => assert!(output.is_dir()),
            _ => assert_eq!(fs::read_link(output).unwrap(), Path::new("missing-target")),
        }
        assert!(!f.0.join("app").exists());
    }
    let f = Fixture::new();
    failed(
        f.run(&[
            "package",
            "--manifest-path=missing",
            "--output=absent/out.tar",
            "--message-format=json",
        ]),
        "E0720",
        "output parent",
        "result",
    );
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
}
#[test]
fn package_build_failures_publish_neither_output_and_preserve_old_bytes() {
    let f = Fixture::new();
    f.project();
    let managed = "app/.oxid/typed-preview/result/demo";
    f.write(managed, "existing unrelated artifact");
    f.write("app/helper.ox", "pub fn answer()->i32{return true;}");
    failed(f.package(&[]), "E0300", "type mismatch", "result");
    assert_eq!(
        fs::read(f.0.join(managed)).unwrap(),
        b"existing unrelated artifact"
    );
    assert!(!f.0.join("bundle.tar").exists());
    f.write("app/helper.ox", "pub fn answer()->i32{return 7;}");
    failed(f.package(&[]), "E0710", "unowned", "result");
    assert_eq!(
        fs::read(f.0.join(managed)).unwrap(),
        b"existing unrelated artifact"
    );
    assert!(!f.0.join("bundle.tar").exists());
}
#[test]
fn package_missing_toolchain_and_invalid_managed_name_report_package_failure() {
    let f = Fixture::new();
    f.project();
    failed(f.package(&[]), "E0701", "cannot launch", "result");
    assert!(!f.0.join("bundle.tar").exists());
    assert!(!f.0.join("app/.oxid/typed-preview/result/demo").exists());
    f.write(
        "app/oxid.toml",
        "[project]\nname=\"_invalid\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n",
    );
    failed(
        f.package(&[]),
        "E0001",
        "manifest package name must match",
        "result",
    );
}
#[test]
fn package_workspace_selection_checks_one_selected_session() {
    let f = Fixture::new();
    f.project();
    f.write("oxid.toml", "[workspace]\nmembers=[\"app\",\"other\"]\n");
    f.write(
        "other/oxid.toml",
        "[project]\nname=\"other\"\nedition=\"typed-preview\"\nentry=\"absent.ox\"\n",
    );
    let args = [
        "package",
        "--manifest-path=oxid.toml",
        "--output=bundle.tar",
        "--message-format=json",
    ];
    failed(f.run(&args), "E0001", "requires --package", "result");
    let mut selected = args.to_vec();
    selected.push("--package=demo");
    failed(f.run(&selected), "E0701", "cannot launch", "result");
    assert!(!f.0.join("other/.oxid").exists());
    assert!(!f.0.join("bundle.tar").exists());
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN and a standard tar reader"]
fn package_real_native_result_process_and_caller_relative_archive() {
    use sha2::{Digest, Sha256};
    let llvm = std::env::var_os("OXID_TEST_LLVM_BIN").expect("supply LLVM 19.1.7 bin directory");
    for mode in ["result", "process"] {
        let f = Fixture::new();
        f.project();
        f.write("oxid.toml", "[workspace]\nmembers=[\"app\"]\n");
        if mode == "process" {
            f.write("app/main.ox", "mod helper; use std::io::write_stdout; fn main()->i32{let bytes=[80,75,71]; let written=write_stdout(&bytes); return crate::helper::answer();}");
        }
        let args = [
            "package",
            "--manifest-path=oxid.toml",
            "--package=demo",
            "--output=bundle.tar",
            "--message-format=json",
            "--entry-mode",
            mode,
        ];
        let output = f
            .command(&args)
            .env("OXID_LLVM_BIN", &llvm)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            text.lines().count(),
            1,
            "packaging must not execute application: {text}"
        );
        assert!(text.contains("\"kind\":\"package-summary\",\"success\":true"));
        assert!(text.contains("\"archive_output\":\"bundle.tar\""));
        assert!(
            text.contains("\"managed_published\":true")
                && text.contains("\"archive_published\":true")
        );
        assert!(!f.0.join("app/bundle.tar").exists());
        fs::create_dir(f.0.join("extract")).unwrap();
        assert!(Command::new("tar")
            .args(["-xf", "bundle.tar", "-C", "extract"])
            .current_dir(&f.0)
            .status()
            .unwrap()
            .success());
        let bytes = fs::read(f.0.join("extract/bin/demo")).unwrap();
        assert_eq!(
            bytes,
            fs::read(f.0.join(format!("app/.oxid/typed-preview/{mode}/demo"))).unwrap()
        );
        let metadata = fs::read_to_string(f.0.join("extract/package.json")).unwrap();
        assert!(metadata.contains(&format!("\"sha256\":\"{:x}\"", Sha256::digest(&bytes))));
        assert!(metadata.contains(&format!("\"entry_mode\":\"{mode}\"")));
        let executed = Command::new(f.0.join("extract/bin/demo"))
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(
            executed.status.code(),
            Some(if mode == "result" { 0 } else { 7 })
        );
        assert_eq!(
            executed.stdout,
            if mode == "result" {
                b"7\n".as_slice()
            } else {
                b"PKG".as_slice()
            }
        );
    }
}
