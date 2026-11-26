#![cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Project(PathBuf);

impl Project {
    fn new(manifest: &str, source: &str, filename: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-build-timings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        let project = Self(root);
        project.write("member/oxid.toml", manifest);
        project.write(filename, source);
        project
    }

    fn write(&self, name: &str, content: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn run(&self, directory: &str, args: &[&str], llvm: Option<&str>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command.current_dir(self.0.join(directory)).args(args);
        if let Some(llvm) = llvm {
            command.env("OXID_LLVM_BIN", llvm);
        } else {
            command.env_remove("OXID_LLVM_BIN");
        }
        command.output().unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rows(output: Output) -> Vec<Value> {
    assert!(
        output.status.success() && output.stderr.is_empty(),
        "{output:?}"
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn require_timings(records: &[Value], last_stage: &str) {
    let entry: Vec<_> = records
        .iter()
        .filter(|row| row["kind"] == "build-timings")
        .collect();
    assert_eq!(entry.len(), 1, "{records:?}");
    let stages = entry[0]["stages"].as_array().unwrap();
    assert_eq!(stages.len(), 3);
    for (stage, name) in stages.iter().zip([
        "Resolve dependencies",
        "Prepare sources and verify lock",
        last_stage,
    ]) {
        assert_eq!(stage["name"], name);
        assert!(stage["elapsed_ns"].as_u64().is_some(), "{stage:?}");
    }
}

#[test]
fn workspace_timings_are_opt_in_and_keep_the_summary_last() {
    let project = Project::new(
        "[project]\nname=\"library\"\nedition=\"typed-preview\"\nkind=\"lib\"\n",
        "pub fn answer()->i32{return 42;}\n",
        "member/src/lib.ox",
    );
    project.write(
        "oxid.toml",
        "[workspace]\nedition=\"typed-preview\"\nmembers=[\"member\"]\n",
    );
    let default = rows(project.run("", &["build", "--workspace", "--message-format=json"], None));
    assert!(default.iter().all(|row| row["kind"] != "build-timings"));
    let timed = rows(project.run(
        "",
        &["build", "--workspace", "--timings", "--message-format=json"],
        None,
    ));
    require_timings(&timed, "Build selected members");
    assert_eq!(timed.last(), default.last());
    for args in [
        &["build", "--workspace", "--timings", "--timings"][..],
        &["build", "--workspace", "--timings=false"],
        &["check", "--workspace", "--timings"],
    ] {
        let rejected = project.run("", args, None);
        assert_eq!(rejected.status.code(), Some(1), "{rejected:?}");
    }
}

#[test]
#[ignore = "requires LLVM 19.1.7 in OXID_TEST_LLVM_BIN"]
fn native_single_project_timings_preserve_the_executable() {
    let llvm = std::env::var("OXID_TEST_LLVM_BIN").unwrap();
    let project = Project::new(
        "[project]\nname=\"hello\"\nedition=\"typed-preview\"\n[build]\nentry-mode=\"result\"\nreproducible=true\n",
        "fn main()->i32{return 42;}\n",
        "member/src/main.ox",
    );
    let default = rows(project.run(
        "member",
        &["build", "--profile=release", "--message-format=json"],
        Some(&llvm),
    ));
    assert!(default.iter().all(|row| row["kind"] != "build-timings"));
    let executable = project.0.join("member/.oxid/bin/hello");
    let original = fs::read(&executable).unwrap();
    let timed = rows(project.run(
        "member",
        &[
            "build",
            "--profile=release",
            "--timings",
            "--message-format=json",
        ],
        Some(&llvm),
    ));
    require_timings(&timed, "Compile and link");
    assert_eq!(fs::read(&executable).unwrap(), original);
    let run = Command::new(executable).output().unwrap();
    assert!(run.status.success() && run.stdout == b"42\n", "{run:?}");
}
