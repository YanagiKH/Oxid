//! Run scoped record element loans through the ordinary project CLI.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

fn output(command: &mut Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let drain = |mut stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            bytes
        })
    };
    let stdout = drain(Box::new(child.stdout.take().unwrap()));
    let stderr = drain(Box::new(child.stderr.take().unwrap()));
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Output {
                status,
                stdout: stdout.join().unwrap(),
                stderr: stderr.join().unwrap(),
            };
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!(
                "record element command timed out: {command:?}; stdout={:?}; stderr={:?}",
                stdout.join().unwrap(),
                stderr.join().unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
struct Project(PathBuf);
impl Project {
    fn new(source: &str, policy: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-record-element-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), format!(
            "[project]\nname=\"element-loans\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n"
        )).unwrap();
        Self(root)
    }

    fn invoke(&self, arguments: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .env_remove("OXID_PATH")
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .args(arguments);
        if let Some(llvm) = std::env::var_os("OXID_TEST_LLVM_BIN") {
            command.env("OXID_LLVM_BIN", llvm);
        }
        output(&mut command)
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        if std::thread::panicking() || std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_some() {
            eprintln!("record element test project retained: {}", self.0.display());
        } else {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

#[path = "../src/frontend/oir/owned/source/owned_record_element_cases.rs"]
mod local_element_cases;

#[test]
fn local_owning_record_elements_support_lexical_reborrows_and_real_child_mutation() {
    for body in [
        local_element_cases::SIMPLE,
        local_element_cases::CHILDREN,
        local_element_cases::LOOP,
        local_element_cases::REALLOCATING_INDEX,
        local_element_cases::GENERIC_RECORD,
        local_element_cases::EMPTY_RECORD,
        local_element_cases::EMPTY_END_SLICES,
    ] {
        for policy in ["bounded", "hosted"] {
            let project = Project::new(&format!("{}{body}", local_element_cases::IMPORTS), policy);
            let checked = project.invoke(&["check"]);
            assert!(checked.status.success(), "{checked:?}");
            let ran = project.invoke(&["run"]);
            assert!(ran.status.success(), "{ran:?}");
            assert_eq!(ran.stdout, b"42\n");
            assert!(ran.stderr.is_empty(), "{ran:?}");
        }
    }
}
#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn local_owning_record_element_file_and_disjoint_bytes_use_real_file_ownership() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new("", policy);
        let file = fs::canonicalize(&project.0).unwrap().join("element-file");
        let source = local_element_cases::FILE.replace(
            "__ELEMENT_FILE_PATH__",
            &format!("{:?}", file.to_str().unwrap()),
        );
        fs::write(
            project.0.join("main.ox"),
            format!("{}{source}", local_element_cases::IMPORTS),
        )
        .unwrap();
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        assert!(!file.exists());
        let ran = project.invoke(&["run"]);
        assert!(ran.status.success(), "{ran:?}");
        assert_eq!(ran.stdout, b"42\n");
        assert!(ran.stderr.is_empty());
        assert_eq!(fs::read(file).unwrap(), [42, 0, 13, 10, 26, 255]);
    }
}
#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
#[test]
fn local_owning_record_element_source_still_refuses_windows_native_before_publication() {
    let project = Project::new(
        &format!(
            "{}{}",
            local_element_cases::IMPORTS,
            local_element_cases::SIMPLE
        ),
        "bounded",
    );
    let checked = project.invoke(&["check"]);
    assert!(checked.status.success(), "{checked:?}");
    let built = project.invoke(&["build", "--profile", "dev"]);
    assert!(!built.status.success());
    let stderr = String::from_utf8_lossy(&built.stderr);
    assert!(
        stderr.contains("E0700") && stderr.contains("Windows native builds"),
        "{built:?}"
    );
    assert!(!project.0.join(".oxid/bin/element-loans.exe").exists());
}
#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires LLVM 19.1.7 on a supported Linux or macOS host"]
fn native_local_owning_record_element_binaries_preserve_children_without_sources_or_cache() {
    for body in [
        local_element_cases::SIMPLE,
        local_element_cases::CHILDREN,
        local_element_cases::LOOP,
        local_element_cases::REALLOCATING_INDEX,
        local_element_cases::GENERIC_RECORD,
        local_element_cases::EMPTY_RECORD,
        local_element_cases::EMPTY_END_SLICES,
        local_element_cases::FILE,
    ] {
        for policy in ["bounded", "hosted"] {
            for profile in ["dev", "release"] {
                let project =
                    Project::new(&format!("{}{body}", local_element_cases::IMPORTS), policy);
                let file = fs::canonicalize(&project.0).unwrap().join("element-file");
                if body.contains("__ELEMENT_FILE_PATH__") {
                    let source = body.replace(
                        "__ELEMENT_FILE_PATH__",
                        &format!("{:?}", file.to_str().unwrap()),
                    );
                    fs::write(
                        project.0.join("main.ox"),
                        format!("{}{source}", local_element_cases::IMPORTS),
                    )
                    .unwrap();
                }
                let built = project.invoke(&["build", "--profile", profile]);
                assert!(built.status.success(), "{built:?}");
                assert!(!file.exists());
                let destination = project.0.join("standalone");
                fs::create_dir(&destination).unwrap();
                let binary = destination.join("app");
                fs::copy(project.0.join(".oxid/bin/element-loans"), &binary).unwrap();
                for name in ["main.ox", "oxid.toml", "oxid.lock"] {
                    let path = project.0.join(name);
                    if path.exists() {
                        fs::remove_file(path).unwrap();
                    }
                }
                for name in [".oxid", "cache"] {
                    let path = project.0.join(name);
                    if path.exists() {
                        fs::remove_dir_all(path).unwrap();
                    }
                }
                assert!(!project.0.join("main.ox").exists());
                assert!(!project.0.join("cache").exists());
                let ran = output(Command::new(binary).current_dir(&destination).env_clear());
                assert!(ran.status.success(), "{ran:?}");
                assert_eq!(ran.stdout, b"42\n");
                assert!(ran.stderr.is_empty(), "{ran:?}");
                if body.contains("__ELEMENT_FILE_PATH__") {
                    assert_eq!(fs::read(&file).unwrap(), [42, 0, 13, 10, 26, 255]);
                }
            }
        }
    }
}
