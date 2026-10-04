//! Public typed formatter contract, isolated from legacy fixture discovery.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-typed-formatter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("input.ox"), source).unwrap();
        Self(path)
    }
    fn command(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(arguments)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env("OXID_LLVM_BIN", self.0.join("native-tools"))
            .env_remove("OXID_PATH");
        command
    }
    fn run(&self, arguments: &[&str]) -> Output {
        self.command(arguments).output().unwrap()
    }
    fn format(&self, check: bool) -> Output {
        let mut args = vec!["fmt", "--edition=typed-preview", "input.ox"];
        if check {
            args.push("--check");
        }
        self.run(&args)
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn assert_error(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected), "{stderr}");
    assert!(!stderr.contains("formatting required"), "{stderr}");
}

#[test]
fn explicit_formatter_outputs_only_complete_source_without_writing_input() {
    let input = b"fn  main ( )->bool{ return true;}";
    let project = Project::new(input);
    let output = project.format(false);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"fn main() -> bool { return true; }\n");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(fs::read(project.0.join("input.ox")).unwrap(), input);
    assert_eq!(fs::read_dir(&project.0).unwrap().count(), 1);
}
