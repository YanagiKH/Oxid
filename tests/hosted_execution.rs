//! Project execution policy: ordinary long-running programs and unchanged traps.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const SCALAR: &str = "fn main()->i32{let mut i=0;while i<200000{i=i+1;}return i;}";
const BUFFER: &str = r#"
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_push;
use alloc::bytes::len;
fn main()->i32{
    let mut bytes=new();let value=255;let byte=value.to_u8_checked();let mut i=0;
    while i<65536{
        let status=try_push(&mut bytes,byte);
        match status{
            GrowStatus::Ok=>{},
            GrowStatus::CapacityOverflow=>{return -1;},
            GrowStatus::AllocationFailed=>{return -2;},
            GrowStatus::MemoryLimit=>{return -3;},
        }
        i=i+1;
    }
    return len(&bytes);
}"#;
const MEMORY: &str = r#"
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_reserve;
use alloc::bytes::len;
use alloc::bytes::capacity;
fn main()->i32{
    let mut bytes=new();let status=try_reserve(&mut bytes,16777216);
    match status{
        GrowStatus::MemoryLimit=>{if len(&bytes)==0 && capacity(&bytes)==0{return 42;}return -1;},
        GrowStatus::Ok=>{return -2;},
        GrowStatus::CapacityOverflow=>{return -3;},
        GrowStatus::AllocationFailed=>{return -4;},
    }
}"#;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const BINARY: &str = r#"
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use std::io::WriteSomeStatus;
use std::io::write_some;
fn main()->i32{
    let mut bytes=new();let n=88;
    let grown=try_resize(&mut bytes,256,n.to_u8_checked());
    match grown{
        GrowStatus::Ok=>{},GrowStatus::CapacityOverflow=>{return 70;},
        GrowStatus::AllocationFailed=>{return 71;},GrowStatus::MemoryLimit=>{return 71;},
    }
    let mut i=0;
    while i<5001{
        let status=write_some(&bytes);
        match status{
            WriteSomeStatus::Written(count)=>{if count!=256{return 74;}},
            WriteSomeStatus::Error(code)=>{return 74;},
        }
        i=i+1;
    }
    return 0;
}"#;

struct Project(PathBuf);
impl Project {
    fn new(source: &str, policy: Option<&str>, process: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-hosted-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        let project = Self(root);
        project.manifest(policy, process);
        project
    }
    fn manifest(&self, policy: Option<&str>, process: bool) {
        let mut text=format!("[project]\nname = \"app\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[build]\nentry-mode = \"{}\"\n",if process {"process"} else {"result"});
        if let Some(policy) = policy {
            text.push_str(&format!("[execution]\npolicy = \"{policy}\"\n"));
        }
        fs::write(self.0.join("oxid.toml"), text).unwrap();
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output, expected: &[u8]) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, expected, "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}
fn fuel_failure(output: &Output) {
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("error[E0601]"),
        "{output:?}"
    );
}

#[test]
fn scalar_and_owned_projects_can_continue_after_the_bounded_instruction_limit() {
    for (source, expected) in [
        (SCALAR, b"200000\n".as_slice()),
        (BUFFER, b"65536\n".as_slice()),
    ] {
        let project = Project::new(source, None, false);
        fuel_failure(&project.command(&["run"]));
        project.manifest(Some("bounded"), false);
        fuel_failure(&project.command(&["run"]));
        project.manifest(Some("hosted"), false);
        success(project.command(&["run"]), expected);
    }
}

#[test]
fn hosted_growth_preserves_the_live_heap_limit_and_the_original_buffer_on_refusal() {
    let project = Project::new(MEMORY, Some("hosted"), false);
    success(project.command(&["run"]), b"42\n");
}

#[test]
fn library_execution_settings_cannot_change_the_calling_binary_policy() {
    let source =
        "use crate::math::target; fn main()->i32{let mut i=0;while i<target(){i=i+1;}return i;}";
    let project = Project::new(source, None, false);
    fs::create_dir_all(project.0.join("library/src")).unwrap();
    fs::write(
        project.0.join("library/src/lib.ox"),
        "pub fn target()->i32{return 200000;}",
    )
    .unwrap();
    let library = project.0.join("library/oxid.toml");
    let manifest="[project]\nname = \"math\"\nkind = \"lib\"\nedition = \"typed-preview\"\nentry = \"src/lib.ox\"\n[execution]\npolicy = \"hosted\"\n";
    fs::write(&library, manifest).unwrap();
    let root_manifest = project.0.join("oxid.toml");
    let mut root = fs::read_to_string(&root_manifest).unwrap();
    root.push_str("[dependencies]\nmath = \"library\"\n");
    fs::write(&root_manifest, &root).unwrap();
    fuel_failure(&project.command(&["run"]));
    fs::write(&library, manifest.replace("\"hosted\"", "\"bounded\"")).unwrap();
    root.push_str("[execution]\npolicy = \"hosted\"\n");
    fs::write(root_manifest, root).unwrap();
    success(project.command(&["run"]), b"200000\n");
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn hosted_binary_delivery_continues_past_the_previous_program_budget() {
    let project = Project::new(BINARY, Some("bounded"), true);
    let bounded = project.command(&["run"]);
    assert!(!bounded.status.success(), "{bounded:?}");
    assert!(
        String::from_utf8_lossy(&bounded.stderr).contains("E0601"),
        "{bounded:?}"
    );
    assert!(bounded.stdout.len() < 256 * 5001 && bounded.stdout.len().is_multiple_of(256));
    assert!(bounded.stdout.iter().all(|byte| *byte == b'X'));
    project.manifest(Some("hosted"), true);
    success(project.command(&["run"]), &vec![b'X'; 256 * 5001]);
}

#[test]
fn hosted_execution_retains_arithmetic_bounds_and_entry_checks() {
    let mut cases = vec![
        (
            "fn main()->i32{let n=2147483647;return n+1;}",
            false,
            "E0604",
        ),
        (
            "fn main()->i32{let a=[1];let i=1;return a[i];}",
            false,
            "E0606",
        ),
    ];
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        cases.push(("fn main()->i32{return 256;}", true, "E0600"));
    }
    for (source, process, code) in cases {
        let project = Project::new(source, Some("bounded"), process);
        let bounded = project.command(&["run"]);
        assert!(!bounded.status.success(), "{bounded:?}");
        assert!(
            String::from_utf8_lossy(&bounded.stderr).contains(code),
            "{bounded:?}"
        );
        project.manifest(Some("hosted"), process);
        let hosted = project.command(&["run"]);
        assert_eq!(hosted.status.code(), bounded.status.code(), "{hosted:?}");
        assert_eq!(hosted.stdout, bounded.stdout, "{hosted:?}");
        assert_eq!(hosted.stderr, bounded.stderr, "{hosted:?}");
    }
}

#[test]
fn invalid_execution_settings_fail_before_source_tools_and_artifact_replacement() {
    for settings in [
        "[execution]\npolicy = \"Hosted\"\n",
        "[execution]\npolicy = \"release\"\n",
        "[execution]\npolicy = hosted\n",
        "[execution]\npolicy = \"hosted\"\npolicy = \"bounded\"\n",
        "[execution]\npolcy = \"hosted\"\n",
        "[execution]\npolicy : \"hosted\"\n",
        "[[execution]]\npolicy = \"hosted\"\n",
        "[execution.extra]\npolicy = \"hosted\"\n",
        "execution.policy = \"hosted\"\n",
        "execution = { policy = \"hosted\" }\n",
    ] {
        let project = Project::new("invalid source", None, false);
        let path = project.0.join("oxid.toml");
        let original = fs::read_to_string(&path).unwrap();
        // Root dotted/inline declarations must actually be at the root.
        fs::write(&path, format!("{settings}{original}")).unwrap();
        fs::create_dir_all(project.0.join(".oxid/bin")).unwrap();
        let artifact = project.0.join(".oxid/bin/app");
        fs::write(&artifact, b"previous executable").unwrap();
        for command in ["check", "run", "build", "test"] {
            let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
                .arg(command)
                .current_dir(&project.0)
                .env("OXID_LLVM_BIN", project.0.join("absent-llvm"))
                .env("PATH", project.0.join("absent-path"))
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(1),
                "settings={settings:?} {output:?}"
            );
            assert!(output.stdout.is_empty(), "{output:?}");
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(
                error.contains("E0001") && error.contains("execution"),
                "{output:?}"
            );
            assert_eq!(fs::read(&artifact).unwrap(), b"previous executable");
        }
    }
}

#[test]
#[ignore = "requires Linux x86_64 LLVM 19.1.7 in OXID_LLVM_BIN"]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn native_hosted_and_bounded_policies_keep_distinct_cache_entries_at_both_optimizations() {
    for source in [SCALAR, BUFFER, MEMORY] {
        let project = Project::new(source, Some("hosted"), false);
        for profile in ["dev", "release"] {
            success_build(project.command(&["build", "--profile", profile, "--incremental"]));
            let artifact = project.0.join(".oxid/bin/app");
            let bytes = fs::read(&artifact).unwrap();
            let cwd = project.0.join("empty");
            fs::create_dir_all(&cwd).unwrap();
            let native = Command::new(&artifact)
                .current_dir(&cwd)
                .env_clear()
                .output()
                .unwrap();
            success(
                native,
                if source == SCALAR {
                    b"200000\n"
                } else if source == BUFFER {
                    b"65536\n"
                } else {
                    b"42\n"
                },
            );
            assert!(fs::read_dir(&cwd).unwrap().next().is_none());
            let warm = project.command(&[
                "build",
                "--profile",
                profile,
                "--incremental",
                "--message-format=json",
            ]);
            assert!(warm.status.success(), "{warm:?}");
            assert!(
                String::from_utf8_lossy(&warm.stdout).contains("\"status\":\"hit\""),
                "{warm:?}"
            );
            assert_eq!(fs::read(&artifact).unwrap(), bytes);
            project.manifest(Some("bounded"), false);
            let bounded = project.command(&[
                "build",
                "--profile",
                profile,
                "--incremental",
                "--message-format=json",
            ]);
            assert!(bounded.status.success(), "{bounded:?}");
            assert!(
                String::from_utf8_lossy(&bounded.stdout).contains("\"status\":\"miss\""),
                "{bounded:?}"
            );
            assert_ne!(fs::read(&artifact).unwrap(), bytes);
            let failed = Command::new(&artifact)
                .current_dir(&cwd)
                .env_clear()
                .output()
                .unwrap();
            fuel_failure(&failed);
            project.manifest(Some("hosted"), false);
        }
    }
}

#[test]
#[ignore = "requires Linux x86_64 LLVM 19.1.7 in OXID_LLVM_BIN"]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn native_hosted_binary_status_delivery_survives_the_long_output_workload() {
    let project = Project::new(BINARY, Some("hosted"), true);
    for profile in ["dev", "release"] {
        success_build(project.command(&["build", "--profile", profile]));
        let binary = project.0.join(".oxid/bin/app");
        let cwd = project.0.join("empty");
        fs::create_dir_all(&cwd).unwrap();
        let native = Command::new(binary)
            .current_dir(&cwd)
            .env_clear()
            .output()
            .unwrap();
        success(native, &vec![b'X'; 256 * 5001]);
        assert!(fs::read_dir(&cwd).unwrap().next().is_none());
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn success_build(output: Output) {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}
