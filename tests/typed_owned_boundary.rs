//! Private ownership consumers do not enable struct/borrow source syntax.
use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn ownership_source_stays_closed_before_execution_tools_or_artifact_effects() {
    let scratch = Scratch(std::env::temp_dir().join(format!(
        "oxid-owned-source-gate-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir(&scratch.0).unwrap();
    for text in [
        "struct Marker {} fn main() -> () { return; }",
        "fn inspect(value: &bool) -> () { return; } fn main() -> () { return; }",
    ] {
        fs::write(scratch.0.join("main.ox"), text).unwrap();
        for command in ["check", "run", "compile"] {
            let mut child = Command::new(env!("CARGO_BIN_EXE_oxid"));
            child
                .current_dir(&scratch.0)
                .env("OXID_LLVM_BIN", scratch.0.join("missing-tools"))
                .args([
                    command,
                    "main.ox",
                    "--edition=typed-preview",
                    "--message-format=json",
                ]);
            if command == "compile" {
                child.args(["--backend=llvm", "--output=must-not-exist"]);
            }
            let result = child.output().unwrap();
            let output = String::from_utf8(result.stdout).unwrap();
            assert_eq!(result.status.code(), Some(1), "{command}: {output}");
            assert!(
                output.contains("E0101") && output.contains("\"stage\":\"parse\""),
                "{command}: {output}"
            );
            assert!(
                !output.contains("E0500") && !output.contains("E0701"),
                "{command}: {output}"
            );
            assert!(result.stderr.is_empty());
            assert!(!scratch.0.join("must-not-exist").exists());
        }
    }
}
