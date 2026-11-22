//! File append through the interpreter and standalone native builds.
#![cfg(any(
    all(
        any(target_os = "linux", target_os = "macos"),
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "windows",
        target_env = "msvc",
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    )
))]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

const SOURCE: &str = r#"
use std::fs::FileStatus;
use std::fs::OpenResult;
use std::fs::open_append;
use std::fs::seek_absolute;
use std::fs::sync_all;
use std::fs::write_some;
use std::io::WriteSomeStatus;

fn main() -> i32 {
    let invalid = b"entries.bin\x00suffix";
    let refused = open_append(&invalid);
    match refused {
        OpenResult::Error(code) => { if code != 22 { return 1; } },
        OpenResult::Opened(value) => { return 2; },
        OpenResult::MemoryLimit => { return 3; },
        OpenResult::AllocationFailed => { return 4; },
    }
    let path = b"entries.bin";
    let opened = open_append(&path);
    match opened {
        OpenResult::Opened(value) => {
            let mut file = value;
            let moved = seek_absolute(&mut file, 0i64);
            match moved {
                FileStatus::Ok => {},
                FileStatus::Error(code) => { return 5; },
            }
            let data = b"\xFF\x00B\n";
            let written = write_some(&mut file, &data);
            match written {
                WriteSomeStatus::Written(count) => { if count != data.len() { return 6; } },
                WriteSomeStatus::Error(code) => { return 7; },
            }
            let synced = sync_all(&mut file);
            match synced {
                FileStatus::Ok => { return 42; },
                FileStatus::Error(code) => { return 8; },
            }
        },
        OpenResult::Error(code) => { return 9; },
        OpenResult::MemoryLimit => { return 10; },
        OpenResult::AllocationFailed => { return 11; },
    }
}
"#;

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-append-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn project() -> Self {
        let project = Self::new();
        fs::write(project.0.join("main.ox"), SOURCE).unwrap();
        fs::write(
            project.0.join("oxid.toml"),
            "[project]\nname=\"append\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n",
        ).unwrap();
        project
    }

    fn compiler(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .args(args)
            .env_remove("OXID_PATH");
        command
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn verify_appends(root: &Path, mut run: impl FnMut() -> Output) {
    let file = root.join("entries.bin");
    assert!(!file.exists());
    let first = run();
    assert!(
        first.status.success() && first.stderr.is_empty(),
        "{first:?}"
    );
    assert_eq!(first.stdout, b"42\n");
    assert_eq!(fs::read(&file).unwrap(), b"\xFF\x00B\n");

    fs::write(&file, b"existing\n").unwrap();
    for expected in [
        b"existing\n\xFF\x00B\n".as_slice(),
        b"existing\n\xFF\x00B\n\xFF\x00B\n".as_slice(),
    ] {
        let next = run();
        assert!(next.status.success() && next.stderr.is_empty(), "{next:?}");
        assert_eq!(next.stdout, b"42\n");
        assert_eq!(fs::read(&file).unwrap(), expected);
    }
}

#[test]
fn interpreted_append_creates_missing_files_and_preserves_existing_bytes() {
    let project = Directory::project();
    let checked = project.compiler(&["check"]).output().unwrap();
    assert!(checked.status.success(), "{checked:?}");
    verify_appends(&project.0, || project.compiler(&["run"]).output().unwrap());
}

#[test]
#[ignore = "requires LLVM 19.1.7 and a supported native target"]
fn native_append_always_writes_at_the_end_after_source_removal() {
    let llvm = std::env::var_os("OXID_TEST_LLVM_BIN").expect("OXID_TEST_LLVM_BIN is required");
    for profile in ["dev", "release"] {
        let project = Directory::project();
        let built = project
            .compiler(&["build", "--profile", profile])
            .env("OXID_LLVM_BIN", &llvm)
            .output()
            .unwrap();
        assert!(built.status.success(), "{profile}: {built:?}");
        let deployment = Directory::new();
        let name = format!("append{}", std::env::consts::EXE_SUFFIX);
        let executable = deployment.0.join(&name);
        fs::copy(project.0.join(".oxid/bin").join(&name), &executable).unwrap();
        fs::remove_dir_all(&project.0).unwrap();
        verify_appends(&deployment.0, || {
            Command::new(&executable)
                .current_dir(&deployment.0)
                .env("PATH", deployment.0.join("missing-tools"))
                .output()
                .unwrap()
        });
    }
}
