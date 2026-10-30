//! Ordinary source packages recover pushed owners and distinguish empty pops.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
#[path = "../src/frontend/oir/owned/source/owned_parameter_cases.rs"]
mod cases;
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
                "owned vector package command timed out: {command:?}; stdout={:?}; stderr={:?}",
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
            "oxid-owned-vector-results-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir_all(root.join("owned_vec/src")).unwrap();
        fs::write(root.join("owned_vec/src/lib.ox"), cases::LIBRARY).unwrap();
        fs::write(
            root.join("owned_vec/oxid.toml"),
            include_str!("../packages/owned_vec/oxid.toml"),
        )
        .unwrap();
        fs::write(root.join("owned_vec.ox"), cases::LIBRARY).unwrap();
        fs::write(root.join("main.ox"), format!("mod owned_vec;{source}")).unwrap();
        fs::write(root.join("oxid.toml"), Self::manifest(policy, false)).unwrap();
        Self(root)
    }
    fn manifest(policy: &str, package: bool) -> String {
        let mut manifest = format!("[project]\nname=\"owned-vector-results\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n");
        if package {
            manifest.push_str("[dependencies]\nowned_vec=\"owned_vec\"\n");
        }
        manifest
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn package(source: &str, policy: &str) -> Self {
        let project = Self::new(source, policy);
        fs::write(project.0.join("main.ox"), source).unwrap();
        fs::remove_file(project.0.join("owned_vec.ox")).unwrap();
        fs::write(project.0.join("oxid.toml"), Self::manifest(policy, true)).unwrap();
        project
    }
    fn invoke(&self, arguments: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .env_remove("OXID_PATH")
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .args(arguments);
        if arguments.first() == Some(&"build") {
            if let Some(llvm) = std::env::var_os("OXID_TEST_LLVM_BIN") {
                command.env("OXID_LLVM_BIN", llvm);
            }
        }
        output(&mut command)
    }
    fn heap_limit(&self, limit: u64) {
        let path = self.0.join("oxid.toml");
        let manifest = fs::read_to_string(&path).unwrap();
        let section = "[execution]\n";
        assert_eq!(manifest.matches(section).count(), 1);
        fs::write(
            path,
            manifest.replacen(section, &format!("{section}heap-limit={limit}\n"), 1),
        )
        .unwrap();
    }
    fn no_artifacts(&self) {
        for name in [".oxid", "oxid.lock", "cache", "main.oxb"] {
            assert!(!self.0.join(name).exists(), "unexpected {name}");
        }
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        if std::thread::panicking() || std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_some() {
            eprintln!(
                "owned vector package fixture retained: {}",
                self.0.display()
            );
        } else {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

#[test]
fn owned_vector_results_use_one_source_module_with_distinct_record_enum_builtin_instances() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(cases::EXAMPLE, policy);
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        let ran = project.invoke(&["run"]);
        assert!(ran.status.success(), "{ran:?}");
        assert_eq!(ran.stdout, b"42\n");
        assert!(ran.stderr.is_empty(), "{ran:?}");
        project.no_artifacts();
    }
}

#[test]
fn generic_vector_owners_transfer_and_detach_across_ordinary_package_boundaries() {
    for policy in ["bounded", "hosted"] {
        let flat = Project::new(cases::OWNERSHIP, policy);
        let projects = [flat];
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let projects = {
            let package = Project::package(cases::OWNERSHIP, policy);
            let [flat] = projects;
            [flat, package]
        };
        for project in projects {
            let checked = project.invoke(&["check"]);
            assert!(checked.status.success(), "{checked:?}");
            assert!(checked.stderr.is_empty(), "{checked:?}");
            let ran = project.invoke(&["run"]);
            assert!(ran.status.success(), "{ran:?}");
            assert_eq!(ran.stdout, b"42\n");
            assert!(ran.stderr.is_empty(), "{ran:?}");
            project.no_artifacts();
        }
    }
}

#[test]
fn generic_vector_owner_signatures_do_not_allow_duplicate_or_borrowed_moves() {
    const IMPORTS: &str = "use alloc::vec::Vec;use alloc::vec::take;";
    for (declaration, valid) in [
        (
            "fn unused<T>(v:Vec<T>)->Vec<T>{let moved=v;return v;}",
            "fn unused<T>(v:Vec<T>)->Vec<T>{let moved=v;return moved;}",
        ),
        (
            "fn unused<T>(v:Vec<T>,flag:bool)->Vec<T>{if flag{let moved=v;}return v;}",
            "fn unused<T>(v:Vec<T>,flag:bool)->Vec<T>{if flag{return v;}return v;}",
        ),
        (
            "fn unused<T>(v:&Vec<T>)->Vec<T>{return take(&*v);}",
            "fn unused<T>(v:&mut Vec<T>)->Vec<T>{return take(&mut *v);}",
        ),
        (
            "fn unused<T>(v:Vec<T>)->T{return v[0];}",
            "fn unused<T>(v:Vec<T>)->Vec<T>{return v;}",
        ),
    ] {
        let project = Project::new(
            &format!("{IMPORTS}{declaration}fn main()->i32{{return 0;}}"),
            "bounded",
        );
        for command in ["check", "run"] {
            let refused = project.invoke(&[command, "--message-format=json"]);
            assert_eq!(refused.status.code(), Some(1), "{declaration}: {refused:?}");
            assert!(refused.stderr.is_empty(), "{refused:?}");
            let json = String::from_utf8_lossy(&refused.stdout);
            // Failed candidate construction preserves the original public
            // import diagnostic. Private definition tests separately require
            // the actual affine E0300; the valid twin below must be accepted
            // with this same library/import context.
            assert!(json.contains("\"code\":\"E0205\""), "{declaration}: {json}");
            assert!(
                json.contains("unsupported standard library import"),
                "{json}"
            );
            assert!(!json.contains("E0500"), "{json}");
        }
        project.no_artifacts();
        let accepted = Project::new(
            &format!("{IMPORTS}{valid}fn main()->i32{{return 0;}}"),
            "bounded",
        );
        for command in ["check", "run"] {
            let result = accepted.invoke(&[command]);
            assert!(result.status.success(), "{valid}: {result:?}");
            assert!(result.stderr.is_empty(), "{valid}: {result:?}");
            if command == "run" {
                assert_eq!(result.stdout, b"0\n");
            }
        }
        accepted.no_artifacts();
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn generic_vector_package_discard_releases_a_file_remaining_in_the_batch() {
    for policy in ["bounded", "hosted"] {
        let project = Project::package(cases::FILE_OWNERSHIP, policy);
        let file = fs::canonicalize(&project.0).unwrap().join("owned-file");
        let source = cases::FILE_OWNERSHIP.replace(
            "__OWNED_VECTOR_FILE_PATH__",
            &format!("{:?}", file.to_str().unwrap()),
        );
        fs::write(project.0.join("main.ox"), source).unwrap();
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        assert!(checked.stderr.is_empty(), "{checked:?}");
        assert!(!file.exists());
        let ran = project.invoke(&["run"]);
        assert!(ran.status.success(), "{ran:?}");
        assert_eq!(ran.stdout, b"42\n");
        assert!(ran.stderr.is_empty(), "{ran:?}");
        assert!(file.is_file());
        assert!(fs::read(&file).unwrap().is_empty());
        project.no_artifacts();
    }
}

#[test]
fn full_vector_push_source_takes_success_without_injected_failure() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(cases::FAILURE, policy);
        let result = project.invoke(&["run"]);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(result.stdout, b"0\n");
        assert!(result.stderr.is_empty(), "{result:?}");
        project.no_artifacts();
    }
}

#[test]
fn hosted_failed_push_recovers_the_incoming_owner_and_unchanged_vector_at_the_public_heap_limit() {
    let flat = Project::new(cases::FAILURE, "hosted");
    flat.heap_limit(512);
    let projects = [flat];
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let projects = {
        let package = Project::package(cases::FAILURE, "hosted");
        package.heap_limit(512);
        let [flat] = projects;
        [flat, package]
    };
    for project in projects {
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        assert!(checked.stderr.is_empty(), "{checked:?}");
        let ran = project.invoke(&["run"]);
        assert!(ran.status.success(), "{ran:?}");
        // MemoryLimit contributes41; independently recovered incoming and
        // original ByteBuf owners each contribute their checked length1.
        assert_eq!(ran.stdout, b"43\n");
        assert!(ran.stderr.is_empty(), "{ran:?}");
        project.no_artifacts();
    }
}

#[test]
fn bounded_vector_results_still_refuse_a_custom_heap_limit_before_execution_or_building() {
    let project = Project::new(cases::FAILURE, "bounded");
    project.heap_limit(512);
    for command in ["check", "run", "build"] {
        let refused = project.invoke(&[command]);
        assert_eq!(refused.status.code(), Some(1), "{refused:?}");
        assert!(
            String::from_utf8_lossy(&refused.stderr)
                .contains("execution heap-limit requires policy = \"hosted\""),
            "{refused:?}"
        );
        assert!(refused.stdout.is_empty(), "{refused:?}");
        project.no_artifacts();
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn owned_vector_results_are_an_ordinary_source_package_dependency() {
    for policy in ["bounded", "hosted"] {
        let project = Project::package(cases::EXAMPLE, policy);
        for command in ["check", "run"] {
            let result = project.invoke(&[command]);
            assert!(result.status.success(), "{result:?}");
            assert!(result.stderr.is_empty(), "{result:?}");
            if command == "run" {
                assert_eq!(result.stdout, b"42\n");
            }
        }
        project.no_artifacts();
    }
}

#[test]
fn unused_generic_vector_definitions_reject_duplicate_moves_and_overlapping_borrows() {
    const IMPORTS: &str =
        "use alloc::vec::Vec;use alloc::vec::push_reserved;use alloc::vec::pop_last;";
    for (declaration, valid) in [
        ("fn unused<T>(v:&mut Vec<T>,x:T)->(){push_reserved(&mut *v,x);push_reserved(&mut *v,x);return;}",
         "fn unused<T>(v:&mut Vec<T>,x:T)->(){push_reserved(&mut *v,x);return;}"),
        ("fn unused<T>(v:&mut Vec<T>)->(){push_reserved(&mut *v,pop_last(&mut *v));return;}",
         "fn unused<T>(v:&mut Vec<T>)->(){let value=pop_last(&mut *v);push_reserved(&mut *v,value);return;}"),
        ("fn unused<T,U>(v:&mut Vec<T>,x:U)->(){push_reserved(&mut *v,x);return;}",
         "fn unused<T,U>(v:&mut Vec<U>,x:U)->(){push_reserved(&mut *v,x);return;}"),
    ] {
        let project = Project::new(&format!("{IMPORTS}{declaration}fn main()->i32{{return 0;}}"), "bounded");
        for command in ["check", "run"] {
            let result = project.invoke(&[command, "--message-format=json"]);
            assert_eq!(result.status.code(), Some(1), "{declaration}: {result:?}");
            assert!(result.stderr.is_empty(), "{result:?}");
            let json = String::from_utf8_lossy(&result.stdout);
            assert!(json.contains("\"code\":\"E0205\""), "{declaration}: {json}");
            assert!(json.contains("unsupported standard library import"), "{json}");
            assert!(!json.contains("E0500"), "{json}");
        }
        project.no_artifacts();
        let accepted = Project::new(&format!("{IMPORTS}{valid}fn main()->i32{{return 0;}}"), "bounded");
        for command in ["check", "run"] {
            let result = accepted.invoke(&[command]);
            assert!(result.status.success(), "{valid}: {result:?}");
            assert!(result.stderr.is_empty(), "{valid}: {result:?}");
            if command == "run" {
                assert_eq!(result.stdout, b"0\n");
            }
        }
        accepted.no_artifacts();
    }
}

#[test]
fn checked_copy_and_scalar_wrappers_keep_the_existing_copy_operations() {
    let source = "use alloc::vec::Vec;use alloc::vec::GrowStatus;use alloc::vec::new;use alloc::vec::try_push;use alloc::vec::len;#[derive(Copy)]struct C{value:i32}fn append<T:Copy>(v:&mut Vec<T>,x:T)->GrowStatus{return try_push(&mut *v,x);}fn main()->i32{let mut a=new::<i32>();append(&mut a,40);let mut b=new::<C>();append(&mut b,C{value:2});let last=b[0];return a[0]+last.value;}";
    for policy in ["bounded", "hosted"] {
        let project = Project::new(source, policy);
        let result = project.invoke(&["run"]);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(result.stdout, b"42\n");
        assert!(result.stderr.is_empty(), "{result:?}");
        project.no_artifacts();
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires LLVM 19.1.7 on a supported Linux or macOS host"]
fn native_owned_vector_result_package_executes_after_sources_and_cache_are_removed() {
    for (source, policy, heap_limit, expected) in [
        (cases::EXAMPLE, "bounded", None, b"42\n".as_slice()),
        (cases::EXAMPLE, "hosted", None, b"42\n".as_slice()),
        (cases::FAILURE, "hosted", Some(512), b"43\n".as_slice()),
        (cases::OWNERSHIP, "bounded", None, b"42\n".as_slice()),
        (cases::OWNERSHIP, "hosted", None, b"42\n".as_slice()),
        (cases::FILE_OWNERSHIP, "bounded", None, b"42\n".as_slice()),
        (cases::FILE_OWNERSHIP, "hosted", None, b"42\n".as_slice()),
    ] {
        for profile in ["dev", "release"] {
            let project = Project::package(source, policy);
            let file = fs::canonicalize(&project.0).unwrap().join("owned-file");
            if source.contains("__OWNED_VECTOR_FILE_PATH__") {
                let source = source.replace(
                    "__OWNED_VECTOR_FILE_PATH__",
                    &format!("{:?}", file.to_str().unwrap()),
                );
                fs::write(project.0.join("main.ox"), source).unwrap();
            }
            if let Some(limit) = heap_limit {
                project.heap_limit(limit);
            }
            let built = project.invoke(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            assert!(!file.exists());
            let binary = project.0.join(".oxid/bin/owned-vector-results");
            let destination = project.0.join("standalone");
            fs::create_dir(&destination).unwrap();
            let relocated = destination.join("app");
            fs::copy(binary, &relocated).unwrap();
            fs::remove_file(project.0.join("main.ox")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            fs::remove_dir_all(project.0.join("owned_vec")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            for name in ["cache", "oxid.lock"] {
                let path = project.0.join(name);
                if path.is_dir() {
                    fs::remove_dir_all(path).unwrap();
                } else if path.exists() {
                    fs::remove_file(path).unwrap();
                }
            }
            assert!(!project.0.join("owned_vec").exists());
            assert!(!project.0.join("cache").exists());
            let ran = output(
                Command::new(&relocated)
                    .current_dir(&destination)
                    .env_clear(),
            );
            assert!(ran.status.success(), "{ran:?}");
            assert_eq!(ran.stdout, expected);
            assert!(ran.stderr.is_empty(), "{ran:?}");
            if source.contains("__OWNED_VECTOR_FILE_PATH__") {
                assert!(file.is_file());
                assert!(fs::read(&file).unwrap().is_empty());
            }
        }
    }
}
