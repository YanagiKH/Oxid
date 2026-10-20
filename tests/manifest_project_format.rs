//! Focused product controls for explicit manifest-selected formatting.
//! No LLVM, native execution, qualification fixture, or CI activation is used.
#![cfg(target_os = "linux")]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-project-format-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn read(&self, path: &str) -> Vec<u8> {
        fs::read(self.0.join(path)).unwrap()
    }
    fn manifest(&self, path: &str, name: &str, entry: &str) {
        self.write(
            path,
            &format!("[project]\nname=\"{name}\"\nedition=\"typed-preview\"\nentry=\"{entry}\"\n"),
        );
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .env("OXID_LLVM_BIN", self.0.join("no-native-toolchain"))
            .env_remove("OXID_PATH")
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn fmt(&self, mode: &str) -> Output {
        self.run(&["fmt", "--manifest-path=oxid.toml", mode])
    }
    fn no_generated_artifacts(&self) {
        fn visit(path: &Path) {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let name = entry.file_name();
                let name = name.to_string_lossy();
                assert!(
                    !name.starts_with(".oxid-fmt-"),
                    "leftover staging: {entry:?}"
                );
                assert_ne!(name, ".oxid");
                assert_ne!(name, "oxid.lock");
                if entry.file_type().unwrap().is_dir() {
                    visit(&entry.path());
                }
            }
        }
        visit(&self.0);
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn exit(output: &Output, status: i32) {
    assert_eq!(output.status.code(), Some(status), "{output:?}");
}

#[test]
fn project_check_is_readonly_and_explicit_write_is_idempotent() {
    let project = Project::new();
    project.manifest("oxid.toml", "app", "main.ox");
    project.write(
        "main.ox",
        "mod helper;fn main()->i32{return crate::helper::answer();}",
    );
    project.write("helper.ox", "pub fn answer()->i32{return 7;}");
    project.write("unused.ox", "this is not a valid declared source module");
    let main = project.read("main.ox");
    let helper = project.read("helper.ox");
    let check = project.fmt("--check");
    exit(&check, 1);
    assert!(
        String::from_utf8_lossy(&check.stdout).contains("2 files checked; 2 require formatting")
    );
    assert_eq!(project.read("main.ox"), main);
    assert_eq!(project.read("helper.ox"), helper);
    project.no_generated_artifacts();
    let write = project.fmt("--write");
    exit(&write, 0);
    assert!(write.stderr.is_empty(), "{write:?}");
    assert_ne!(project.read("main.ox"), main);
    assert_ne!(project.read("helper.ox"), helper);
    assert_eq!(
        project.read("unused.ox"),
        b"this is not a valid declared source module"
    );
    exit(&project.fmt("--check"), 0);
    let main = project.read("main.ox");
    let helper = project.read("helper.ox");
    let again = project.fmt("--write");
    exit(&again, 0);
    assert!(String::from_utf8_lossy(&again.stdout).contains("0 formatted"));
    assert_eq!(project.read("main.ox"), main);
    assert_eq!(project.read("helper.ox"), helper);
    project.no_generated_artifacts();
}

#[test]
fn semantic_errors_do_not_prevent_syntax_formatting() {
    let project = Project::new();
    project.manifest("oxid.toml", "app", "main.ox");
    project.write("main.ox", "fn main()->i32{return unknown_value;}");
    exit(&project.fmt("--write"), 0);
    exit(&project.fmt("--check"), 0);
    let check = project.run(&["check", "--manifest-path=oxid.toml"]);
    assert!(!check.status.success(), "{check:?}");
    assert!(String::from_utf8_lossy(&check.stderr).contains("unknown_value"));
    project.no_generated_artifacts();
}

#[test]
fn syntax_failure_in_later_module_leaves_every_original_untouched() {
    let project = Project::new();
    project.manifest("oxid.toml", "app", "main.ox");
    project.write("main.ox", "mod later;fn main()->i32{return 0;}");
    project.write("later.ox", "pub fn broken( {");
    let main = project.read("main.ox");
    let later = project.read("later.ox");
    let write = project.fmt("--write");
    exit(&write, 2);
    assert!(write.stdout.is_empty(), "{write:?}");
    assert!(!write.stderr.is_empty());
    assert_eq!(project.read("main.ox"), main);
    assert_eq!(project.read("later.ox"), later);
    project.no_generated_artifacts();
}

#[test]
fn workspace_formats_only_selected_package_not_dependencies_or_other_members() {
    let project = Project::new();
    project.write(
        "oxid.toml",
        "[workspace]\nmembers=[\"app\",\"library\",\"unselected\"]\n",
    );
    project.write(
        "app/oxid.toml",
        "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[dependencies]\nmath=\"../library\"\n",
    );
    project.manifest("library/oxid.toml", "library", "lib.ox");
    project.manifest("unselected/oxid.toml", "unselected", "main.ox");
    project.write("app/main.ox", "fn main()->i32{return 0;}");
    project.write("library/lib.ox", "pub fn value()->i32{return 3;}");
    project.write("unselected/main.ox", "invalid syntax is not loaded");
    let library = project.read("library/lib.ox");
    let unselected = project.read("unselected/main.ox");
    let output = project.run(&[
        "fmt",
        "--manifest-path=oxid.toml",
        "--package=app",
        "--write",
    ]);
    exit(&output, 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 files checked; 1 formatted"));
    assert_eq!(project.read("library/lib.ox"), library);
    assert_eq!(project.read("unselected/main.ox"), unselected);
    exit(
        &project.run(&[
            "fmt",
            "--manifest-path=oxid.toml",
            "--package=app",
            "--check",
        ]),
        0,
    );
    project.no_generated_artifacts();
}

#[test]
fn write_refuses_symlink_entry_while_check_keeps_existing_loader_behavior() {
    use std::os::unix::fs::symlink;
    let project = Project::new();
    project.manifest("oxid.toml", "app", "entry.ox");
    project.write("actual.ox", "fn main()->i32{return 0;}");
    symlink("actual.ox", project.0.join("entry.ox")).unwrap();
    let original = project.read("actual.ox");
    exit(&project.fmt("--check"), 1);
    let write = project.fmt("--write");
    exit(&write, 2);
    assert!(String::from_utf8_lossy(&write.stderr).contains("symbolic links"));
    assert_eq!(project.read("actual.ox"), original);
    assert!(fs::symlink_metadata(project.0.join("entry.ox"))
        .unwrap()
        .file_type()
        .is_symlink());
    project.no_generated_artifacts();
}

#[test]
fn selected_source_also_used_by_dependency_is_not_replaced() {
    use std::os::unix::fs::symlink;
    let project = Project::new();
    project.write(
        "oxid.toml",
        "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[dependencies]\nshared=\"library\"\n",
    );
    project.write("main.ox", "fn main()->i32{return 0;}");
    project.manifest("library/oxid.toml", "library", "shared.ox");
    symlink("../main.ox", project.0.join("library/shared.ox")).unwrap();
    let original = project.read("main.ox");
    let write = project.fmt("--write");
    exit(&write, 2);
    assert!(String::from_utf8_lossy(&write.stderr).contains("also belongs to a dependency"));
    assert_eq!(project.read("main.ox"), original);
    project.no_generated_artifacts();
}

#[test]
fn readonly_source_fails_without_changing_mode_or_bytes() {
    use std::os::unix::fs::PermissionsExt;
    let project = Project::new();
    project.manifest("oxid.toml", "app", "main.ox");
    project.write("main.ox", "fn main()->i32{return 0;}");
    let path = project.0.join("main.ox");
    let original = project.read("main.ox");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o440)).unwrap();
    let write = project.fmt("--write");
    exit(&write, 2);
    assert!(String::from_utf8_lossy(&write.stderr).contains("read-only"));
    assert_eq!(project.read("main.ox"), original);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o440
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    project.no_generated_artifacts();
}
