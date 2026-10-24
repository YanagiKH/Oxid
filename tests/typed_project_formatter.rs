//! Named-file formatting uses the package edition without loading dependencies.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-package-format-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), "[project]\nname=\"format-example\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[dependencies]\nmath=\"../missing-package\"\n").unwrap();
        Self(root)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .args(args)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .output()
            .unwrap()
    }
    fn unchanged(&self, source: &str) {
        assert_eq!(
            fs::read_to_string(self.0.join("src/main.ox")).unwrap(),
            source
        );
        for path in [".oxid", "cache", "oxid.lock"] {
            assert!(!self.0.join(path).exists(), "{path}");
        }
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn package_edition_formats_named_files_without_resolving_missing_dependencies() {
    let source = "use crate::math::missing;fn main()->i32{return missing();}";
    let project = Project::new(source);
    let inherited = project.run(&["fmt", "src/main.ox"]);
    let explicit = project.run(&["fmt", "--edition=typed-preview", "src/main.ox"]);
    assert!(inherited.status.success(), "{inherited:?}");
    assert!(explicit.status.success(), "{explicit:?}");
    assert_eq!(inherited.stdout, explicit.stdout);
    assert!(inherited.stderr.is_empty());
    assert!(String::from_utf8(inherited.stdout)
        .unwrap()
        .contains("return missing();"));
    project.unchanged(source);
}

#[test]
fn package_format_check_reports_only_formatting_drift_and_preserves_input() {
    let source = "fn main()->char{return '🦀';}";
    let project = Project::new(source);
    let drift = project.run(&["fmt", "--check", "src/main.ox"]);
    assert_eq!(drift.status.code(), Some(1));
    assert!(drift.stdout.is_empty());
    assert_eq!(drift.stderr, b"typed-preview fmt: formatting required\n");
    project.unchanged(source);
    let formatted = project.run(&["fmt", "src/main.ox"]);
    assert!(formatted.status.success(), "{formatted:?}");
    fs::write(project.0.join("src/main.ox"), &formatted.stdout).unwrap();
    let clean = project.run(&["fmt", "src/main.ox", "--check"]);
    assert!(clean.status.success(), "{clean:?}");
    assert!(clean.stdout.is_empty());
    assert!(clean.stderr.is_empty());
    project.unchanged(std::str::from_utf8(&formatted.stdout).unwrap());
}

#[test]
fn project_graph_operations_keep_their_named_source_restriction() {
    let source = "fn main()->i32{return 0;}";
    let project = Project::new(source);
    for command in ["check", "run", "compile"] {
        let output = project.run(&[command, "src/main.ox"]);
        assert_eq!(output.status.code(), Some(1), "{command}: {output:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("package dependencies require a source-less project command"));
    }
    project.unchanged(source);
}

#[test]
fn format_rejects_directory_and_lock_options_before_touching_project_outputs() {
    let source = "fn main()->i32{return 0;}";
    let project = Project::new(source);
    for (args, message) in [
        (vec!["fmt", "."], "requires one source file"),
        (
            vec!["fmt", "src/main.ox", "--locked"],
            "locking options require a typed project lock, check, run, build or test command",
        ),
    ] {
        let output = project.run(&args);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains(message));
    }
    project.unchanged(source);
}
