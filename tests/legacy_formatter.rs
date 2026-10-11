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
        command
            .args(["fmt", target])
            .output()
            .expect("launch legacy formatter directly")
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
