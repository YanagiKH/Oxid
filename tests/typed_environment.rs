//! Environment bytes through the typed interpreter and native process entry.
#![cfg(any(
    all(
        any(target_os = "linux", target_os = "macos"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "windows", target_env = "msvc", target_arch = "x86_64")
))]

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

const SOURCE: &str = r#"
use std::env::read_variable;
use std::io::ReadSomeStatus;

fn range_rejected(key: &[u8], output: &mut [u8]) -> bool {
    let result = read_variable(&*key, &mut *output);
    match result {
        ReadSomeStatus::Data(count) => { return false; },
        ReadSomeStatus::Eof => { return false; },
        ReadSomeStatus::Error(code) => { return code == 34; },
    }
}

fn optional_raw() -> bool {
    let key = b"OXID_TEST_TYPED_ENV_RAW";
    let mut output = b"xx";
    let result = read_variable(&key, &mut output);
    match result {
        ReadSomeStatus::Data(count) => { return count == 1 && output[0] == 255u8 && output[1] == 120u8; },
        ReadSomeStatus::Eof => { return output[0] == 120u8; },
        ReadSomeStatus::Error(code) => { return false; },
    }
}

fn main() -> i32 {
    let key = b"OXID_TEST_TYPED_ENV_PRESENT";
    let mut longer = b"_________";
    let expected = b"桜🦀";
    let found = read_variable(&key, &mut longer);
    match found {
        ReadSomeStatus::Data(count) => {
            if count != expected.len() || longer[count] != 95u8 { return 1; }
            for i in 0..count { if longer[i] != expected[i] { return 2; } }
        },
        ReadSomeStatus::Eof => { return 3; },
        ReadSomeStatus::Error(code) => { return 4; },
    }

    let mut output = b"x";
    if !range_rejected(&key, &mut output) || output[0] != 120u8 { return 5; }
    let large = b"OXID_TEST_TYPED_ENV_LARGE";
    if !range_rejected(&large, &mut output) || output[0] != 120u8 { return 6; }

    let missing = b"OXID_TEST_TYPED_ENV_MISSING";
    let missing_status = read_variable(&missing, &mut output);
    match missing_status {
        ReadSomeStatus::Eof => {},
        ReadSomeStatus::Data(count) => { return 7; },
        ReadSomeStatus::Error(code) => { return 8; },
    }
    let empty = b"OXID_TEST_TYPED_ENV_EMPTY";
    let empty_status = read_variable(&empty, &mut output);
    match empty_status {
        ReadSomeStatus::Data(count) => { if count != 0 { return 9; } },
        ReadSomeStatus::Eof => { return 10; },
        ReadSomeStatus::Error(code) => { return 11; },
    }
    let invalid = b"OXID_TEST_TYPED_ENV_PRESENT=other";
    let invalid_status = read_variable(&invalid, &mut output);
    match invalid_status {
        ReadSomeStatus::Data(count) => { return 12; },
        ReadSomeStatus::Eof => { return 13; },
        ReadSomeStatus::Error(code) => { if code != 22 { return 14; } },
    }
    if output[0] != 120u8 || !optional_raw() { return 15; }
    return 42;
}
"#;

struct Project(PathBuf);

impl Project {
    fn new(entry_mode: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "oxid-typed-env-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(directory.join("src")).unwrap();
        fs::write(
            directory.join("oxid.toml"),
            format!(
                "[project]\nname = \"environment\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\
                 [build]\nentry-mode = \"{entry_mode}\"\n[execution]\npolicy = \"bounded\"\n"
            ),
        )
        .unwrap();
        fs::write(directory.join("src/main.ox"), SOURCE).unwrap();
        Self(directory)
    }

    fn command(&self, executable: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(executable);
        command
            .current_dir(&self.0)
            .env_remove("OXID_PATH")
            .env_remove("OXID_TEST_TYPED_ENV_MISSING")
            .env_remove("OXID_TEST_TYPED_ENV_RAW")
            .env("OXID_TEST_TYPED_ENV_PRESENT", "桜🦀")
            .env("OXID_TEST_TYPED_ENV_EMPTY", "")
            .env("OXID_TEST_TYPED_ENV_LARGE", "猫".repeat(11_000));
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            command.env(
                "OXID_TEST_TYPED_ENV_RAW",
                std::ffi::OsString::from_vec(vec![255]),
            );
        }
        if let Some(llvm) = std::env::var_os("OXID_TEST_LLVM_BIN") {
            command.env("OXID_LLVM_BIN", llvm);
        }
        command
    }

    fn oxid(&self) -> Command {
        self.command(env!("CARGO_BIN_EXE_oxid"))
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(result: Output) {
    assert_eq!(result.status.code(), Some(42), "{result:?}");
    assert!(result.stdout.is_empty(), "{result:?}");
    assert!(result.stderr.is_empty(), "{result:?}");
}

#[test]
fn interpreter_reads_environment_without_losing_raw_bytes_or_overwriting_short_buffers() {
    success(Project::new("process").oxid().arg("run").output().unwrap());
}

#[test]
fn environment_reads_require_a_process_entry_and_disjoint_slices() {
    let result = Project::new("result").oxid().arg("run").output().unwrap();
    assert_ne!(result.status.code(), Some(42), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("requires process entry"));

    let project = Project::new("process");
    fs::write(
        project.0.join("src/main.ox"),
        r#"use std::env::read_variable;
fn main() -> i32 {
    let mut bytes = b"PATH";
    let status = read_variable(&bytes, &mut bytes);
    return 42;
}
"#,
    )
    .unwrap();
    let checked = project.oxid().arg("check").output().unwrap();
    assert_eq!(checked.status.code(), Some(1), "{checked:?}");
    assert!(
        String::from_utf8_lossy(&checked.stderr).contains("error[E0311]"),
        "{checked:?}"
    );
}

#[test]
#[ignore = "requires LLVM 19.1.7 and the host SDK"]
fn native_dev_and_release_read_the_same_environment_values() {
    let project = Project::new("process");
    let name = if cfg!(windows) {
        "environment.exe"
    } else {
        "environment"
    };
    for profile in ["dev", "release"] {
        let built = project
            .oxid()
            .args(["build", "--profile", profile])
            .output()
            .unwrap();
        assert!(built.status.success(), "{built:?}");
        success(
            project
                .command(project.0.join(".oxid/bin").join(name))
                .output()
                .unwrap(),
        );
    }
}
