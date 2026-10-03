//! Public dispatch controls for the bounded typed-project activation.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SENTINEL: &[u8] = b"keep existing output\n";

struct Scratch(PathBuf);
impl Scratch {
    fn new(files: &[(&str, &str)]) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "oxid-project-dispatch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        for (name, text) in files {
            fs::write(dir.join(name), text).unwrap();
        }
        fs::write(dir.join("output"), SENTINEL).unwrap();
        Self(dir)
    }
    fn typed(&self, operation: &str) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .env("OXID_LLVM_BIN", self.0.join("missing-tools"))
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .args([
                operation,
                "main.ox",
                "--edition=typed-preview",
                "--message-format=json",
            ]);
        if operation == "compile" {
            command.args(["--backend=llvm", "--output=output"]);
        }
        let output = command.output().unwrap();
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(fs::read(self.0.join("output")).unwrap(), SENTINEL);
        assert!(!self.0.join("cache").exists());
        assert!(!self.0.join("main.oxb").exists());
        output
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output, summary: &str) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(String::from_utf8(output.stdout).unwrap(), summary);
}
fn failure(output: Output, code: &str, stage: &str, path: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for field in [
        format!("\"code\":\"{code}\""),
        format!("\"stage\":\"{stage}\""),
        format!("\"path\":\"{path}\""),
        "\"success\":false".into(),
    ] {
        assert!(text.contains(&field), "{text}");
    }
    assert!(!text.contains("E0500"), "{text}");
}

#[test]
fn module_free_project_syntax_keeps_public_check_and_run_portable() {
    for source in [
        "pub fn main()->i32{return 7;}",
        "struct State{n:i32} pub fn main()->i32{let s=State{n:7};return s.n;}",
        "fn value()->i32{return 7;} fn main()->i32{return crate::value();}",
    ] {
        let scratch = Scratch::new(&[("main.ox", source)]);
        let functions = if source.contains("fn value") { 2 } else { 1 };
        success(scratch.typed("check"), &format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":true,\"errors\":0,\"functions\":{functions}}}\n"));
        success(scratch.typed("run"), "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{\"type\":\"i32\",\"value\":7}}\n");
    }
}

#[test]
fn module_free_native_denial_keeps_the_existing_output() {
    let scratch = Scratch::new(&[(
        "main.ox",
        "pub fn main()->i32{return again();} fn again()->i32{return again();}",
    )]);
    failure(
        scratch.typed("compile"),
        "E0700",
        "native-admission",
        "main.ox",
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linked_scalar_count_and_root_main_identity_reach_public_consumers() {
    let scratch = Scratch::new(&[
        ("main.ox", "mod child; use crate::child::value as imported; fn helper()->i32{return 1;} fn main()->i32{return helper()+imported();}"),
        ("child.ox", "pub fn value()->i32{return 6;} fn main()->i32{return 99;}"),
    ]);
    success(scratch.typed("check"), "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":true,\"errors\":0,\"functions\":4}\n");
    success(scratch.typed("run"), "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{\"type\":\"i32\",\"value\":7}}\n");
}

#[cfg(target_os = "linux")]
#[test]
fn imported_main_is_not_an_entry_and_check_needs_no_entry() {
    let scratch = Scratch::new(&[
        ("main.ox", "mod child; use crate::child::value as main;"),
        ("child.ox", "pub fn value()->i32{return 99;}"),
    ]);
    success(scratch.typed("check"), "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":true,\"errors\":0,\"functions\":1}\n");
    for operation in ["run", "compile"] {
        let output = scratch.typed(operation);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("main"), "{text}");
        assert!(text.contains("\"success\":false"), "{text}");
        assert!(!text.contains("native-toolchain"), "{text}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn unused_child_is_checked_before_entry_or_native_admission() {
    let scratch = Scratch::new(&[
        ("main.ox", "mod child;"),
        ("child.ox", "fn bad()->i32{return true;}"),
    ]);
    for operation in ["check", "run", "compile"] {
        failure(scratch.typed(operation), "E0300", "type", "child.ox");
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn child_discovery_is_rejected_at_the_root_declaration() {
    let scratch = Scratch::new(&[("main.ox", "mod child; fn main()->i32{return 7;}")]);
    for operation in ["check", "run", "compile"] {
        failure(scratch.typed(operation), "E0005", "source", "main.ox");
    }
}
