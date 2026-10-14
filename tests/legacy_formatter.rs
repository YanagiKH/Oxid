//! Selected legacy formatter behavior; not a contract for the typed formatter.
//! Directory traversal order and cross-file atomicity are deliberately unspecified.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);
const ORIGINAL: &[u8] = b"fn main() {\r\n return true;  \r\n}\r\n";
const FORMATTED: &[u8] = b"fn main() {\n    return true;\n}\n";

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-legacy-formatter-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        // Never reuse or remove a preexisting directory, even after a stale run.
        fs::create_dir(&root).expect("create isolated formatter project");
        Self(root)
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("create fixture parent");
        fs::write(path, bytes).expect("write formatter fixture");
    }

    fn read(&self, relative: &str) -> Vec<u8> {
        fs::read(self.0.join(relative)).expect("read formatter fixture")
    }

    fn format(&self, target: &str, explicit_legacy: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .stdin(Stdio::null())
            .env("OXID_CACHE_DIR", self.0.join(".oxid"))
            .env_remove("OXID_PATH");
        if explicit_legacy {
            command.arg("--edition=legacy-0.9");
        }
        let mut child = command
            .args(["fmt", target])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("launch legacy formatter directly");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if child.try_wait().expect("poll formatter").is_some() {
                return child.wait_with_output().expect("collect formatter output");
            }
            if std::time::Instant::now() >= deadline {
                child.kill().expect("stop timed-out formatter");
                let output = child.wait_with_output().expect("reap formatter");
                panic!("legacy formatter exceeded ten seconds: {output:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_success(output: &Output) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

fn assert_failure(output: &Output, reason: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(stderr.contains(reason), "{stderr}");
}

#[test]
fn explicit_file_is_formatted_in_place_and_repeated_formatting_is_stable() {
    for explicit_legacy in [false, true] {
        let project = Project::new();
        project.write("input.ox", ORIGINAL);
        assert_success(&project.format("input.ox", explicit_legacy));
        assert_eq!(project.read("input.ox"), FORMATTED);
        assert_success(&project.format("input.ox", explicit_legacy));
        assert_eq!(project.read("input.ox"), FORMATTED);
    }
}

#[test]
fn directory_formatting_recurses_but_preserves_ignored_trees_and_other_extensions() {
    for explicit_legacy in [false, true] {
        let project = Project::new();
        let selected = ["main.ox", "nested/child.ox"];
        for path in selected {
            project.write(path, ORIGINAL);
        }
        let preserved = [
            ".git/invalid.ox",
            "target/invalid.ox",
            ".oxid/invalid.ox",
            "nested/.git/invalid.ox",
            "nested/target/invalid.ox",
            "nested/.oxid/invalid.ox",
            "oxid.toml",
            "nested/notes.txt",
            "nested/helper.c",
        ];
        // Invalid UTF-8 makes accidental selection fail instead of silently passing.
        for path in preserved {
            project.write(path, b"untouched\xff\n");
        }
        assert_success(&project.format(".", explicit_legacy));
        for path in selected {
            assert_eq!(project.read(path), FORMATTED, "{path}");
        }
        for path in preserved {
            assert_eq!(project.read(path), b"untouched\xff\n", "{path}");
        }
    }
}

#[test]
fn directory_without_eligible_sources_fails_then_recovers_after_adding_one() {
    for explicit_legacy in [false, true] {
        let project = Project::new();
        let ignored = [".git/hidden.ox", "target/hidden.ox", ".oxid/hidden.ox"];
        for path in ignored {
            project.write(path, ORIGINAL);
        }
        project.write("notes.txt", ORIGINAL);
        assert_failure(
            &project.format(".", explicit_legacy),
            "no Oxid source files found under .",
        );
        for path in ignored {
            assert_eq!(project.read(path), ORIGINAL, "{path}");
        }
        assert_eq!(project.read("notes.txt"), ORIGINAL);

        project.write("added.ox", ORIGINAL);
        assert_success(&project.format(".", explicit_legacy));
        assert_eq!(project.read("added.ox"), FORMATTED);
        for path in ignored {
            assert_eq!(project.read(path), ORIGINAL, "{path}");
        }
        assert_eq!(project.read("notes.txt"), ORIGINAL);
    }
}

#[test]
fn invalid_utf8_file_is_preserved_and_formatting_recovers_after_repair() {
    for explicit_legacy in [false, true] {
        let project = Project::new();
        let invalid = b"fn main() {\n\xff\n}\n";
        project.write("input.ox", invalid);
        assert_failure(
            &project.format("input.ox", explicit_legacy),
            "cannot read file: input.ox (",
        );
        assert_eq!(project.read("input.ox"), invalid);

        project.write("input.ox", ORIGINAL);
        assert_success(&project.format("input.ox", explicit_legacy));
        assert_eq!(project.read("input.ox"), FORMATTED);
    }
}

#[cfg(unix)]
#[test]
fn directory_formatting_skips_external_file_symlinks() {
    for explicit_legacy in [false, true] {
        let project = Project::new();
        let outside = Project::new();
        outside.write("external.ox", ORIGINAL);
        project.write("nested/real.ox", ORIGINAL);
        let link = project.0.join("nested/linked.ox");
        let target = outside.0.join("external.ox");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_success(&project.format(".", explicit_legacy));
        assert_eq!(outside.read("external.ox"), ORIGINAL);
        assert_eq!(project.read("nested/real.ox"), FORMATTED);
        assert_eq!(fs::read_link(link).unwrap(), target);
    }
}

#[cfg(unix)]
#[test]
fn directory_formatting_skips_external_directory_symlinks() {
    for explicit_legacy in [false, true] {
        let project = Project::new();
        let outside = Project::new();
        outside.write("deep/external.ox", ORIGINAL);
        project.write("nested/real.ox", ORIGINAL);
        let link = project.0.join("external");
        std::os::unix::fs::symlink(&outside.0, &link).unwrap();
        assert_success(&project.format(".", explicit_legacy));
        assert_eq!(outside.read("deep/external.ox"), ORIGINAL);
        assert_eq!(project.read("nested/real.ox"), FORMATTED);
        assert_eq!(fs::read_link(link).unwrap(), outside.0);
    }
}

#[cfg(unix)]
#[test]
fn directory_formatting_skips_ancestor_cycles_and_dangling_symlinks() {
    for explicit_legacy in [false, true] {
        let project = Project::new();
        project.write("nested/real.ox", ORIGINAL);
        let cycle = project.0.join("nested/back");
        let dangling = project.0.join("missing.ox");
        std::os::unix::fs::symlink("..", &cycle).unwrap();
        std::os::unix::fs::symlink("absent.ox", &dangling).unwrap();
        assert_success(&project.format(".", explicit_legacy));
        assert_eq!(project.read("nested/real.ox"), FORMATTED);
        assert_eq!(fs::read_link(cycle).unwrap(), PathBuf::from(".."));
        assert_eq!(fs::read_link(dangling).unwrap(), PathBuf::from("absent.ox"));
    }
}

#[cfg(unix)]
#[test]
fn explicit_file_symlink_formats_its_target_and_preserves_the_link() {
    for explicit_legacy in [false, true] {
        let project = Project::new();
        let outside = Project::new();
        outside.write("external.ox", ORIGINAL);
        let target = outside.0.join("external.ox");
        let link = project.0.join("linked.ox");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_success(&project.format("linked.ox", explicit_legacy));
        assert_eq!(outside.read("external.ox"), FORMATTED);
        assert_eq!(fs::read_link(link).unwrap(), target);
    }
}

#[cfg(unix)]
#[test]
fn explicit_directory_symlink_formats_real_children_but_skips_child_symlinks() {
    for explicit_legacy in [false, true] {
        let project = Project::new();
        let outside = Project::new();
        let other = Project::new();
        outside.write("nested/real.ox", ORIGINAL);
        other.write("external.ox", ORIGINAL);
        let child_target = other.0.join("external.ox");
        let child_link = outside.0.join("nested/linked.ox");
        std::os::unix::fs::symlink(&child_target, &child_link).unwrap();
        let link = project.0.join("linked");
        std::os::unix::fs::symlink(&outside.0, &link).unwrap();
        assert_success(&project.format("linked", explicit_legacy));
        assert_eq!(outside.read("nested/real.ox"), FORMATTED);
        assert_eq!(other.read("external.ox"), ORIGINAL);
        assert_eq!(fs::read_link(link).unwrap(), outside.0);
        assert_eq!(fs::read_link(child_link).unwrap(), child_target);
    }
}

#[cfg(unix)]
#[test]
fn directory_with_only_symlinks_reports_no_sources_and_preserves_targets() {
    for explicit_legacy in [false, true] {
        let project = Project::new();
        let outside = Project::new();
        outside.write("external.ox", ORIGINAL);
        let target = outside.0.join("external.ox");
        let file_link = project.0.join("linked.ox");
        let dir_link = project.0.join("linked-dir");
        std::os::unix::fs::symlink(&target, &file_link).unwrap();
        std::os::unix::fs::symlink(&outside.0, &dir_link).unwrap();
        assert_failure(
            &project.format(".", explicit_legacy),
            "no Oxid source files found under .",
        );
        assert_eq!(outside.read("external.ox"), ORIGINAL);
        assert_eq!(fs::read_link(file_link).unwrap(), target);
        assert_eq!(fs::read_link(dir_link).unwrap(), outside.0);
    }
}
