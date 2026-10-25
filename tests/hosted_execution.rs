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
const HEAP_GROWTH: &str = r#"
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use alloc::bytes::try_reserve;
use alloc::bytes::len;
use alloc::bytes::capacity;
fn main()->i32{
    let mut bytes=new();
    let initial=try_resize(&mut bytes,8,7u8);
    match initial{
        GrowStatus::Ok=>{},GrowStatus::MemoryLimit=>{return 1;},
        GrowStatus::CapacityOverflow=>{return -1;},GrowStatus::AllocationFailed=>{return -2;},
    }
    let grown=try_reserve(&mut bytes,1);
    match grown{
        GrowStatus::Ok=>{if len(&bytes)==8 && capacity(&bytes)==16{return 2;}return -3;},
        GrowStatus::MemoryLimit=>{},
        GrowStatus::CapacityOverflow=>{return -4;},GrowStatus::AllocationFailed=>{return -5;},
    }
    if len(&bytes)!=8 || capacity(&bytes)!=8{return -6;}
    let mut other=new();
    let allocated=try_resize(&mut other,8,9u8);
    match allocated{
        GrowStatus::Ok=>{return 3;},GrowStatus::MemoryLimit=>{return 4;},
        GrowStatus::CapacityOverflow=>{return -7;},GrowStatus::AllocationFailed=>{return -8;},
    }
}"#;
const LARGE_HEAP: &str = r#"
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::try_resize;
use alloc::bytes::len;
use alloc::bytes::capacity;
fn main()->i32{
    let mut bytes=new();let status=try_resize(&mut bytes,20971520,7u8);
    match status{
        GrowStatus::MemoryLimit=>{if len(&bytes)==0 && capacity(&bytes)==0{return 42;}return -1;},
        GrowStatus::Ok=>{if len(&bytes)==20971520 && capacity(&bytes)==20971520{return 43;}return -2;},
        GrowStatus::CapacityOverflow=>{return -3;},GrowStatus::AllocationFailed=>{return -4;},
    }
}"#;
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
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
    fn heap_limit(&self, limit: &str) {
        self.manifest(Some("hosted"), false);
        let path = self.0.join("oxid.toml");
        let text = fs::read_to_string(&path).unwrap();
        fs::write(path, format!("{text}heap-limit = {limit}\n")).unwrap();
    }
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
    project.manifest(Some("bounded"), false);
    success(project.command(&["run"]), b"42\n");
}

#[test]
fn hosted_heap_settings_account_for_headers_growth_peaks_and_failed_allocation_rollback() {
    let project = Project::new(HEAP_GROWTH, Some("hosted"), false);
    for (limit, expected) in [
        ("0", "1\n"),
        ("39", "1\n"),
        ("40", "4\n"),
        ("79", "4\n"),
        ("80", "3\n"),
        ("87", "3\n"),
        ("88", "2\n"),
        ("18446744073709551615", "2\n"),
    ] {
        project.heap_limit(limit);
        success(project.command(&["run"]), expected.as_bytes());
    }
    let project = Project::new(MEMORY, Some("hosted"), false);
    for (limit, expected) in [
        ("16777247", "42\n"),
        ("16777248", "-2\n"),
        ("67108864", "-2\n"),
    ] {
        project.heap_limit(limit);
        success(project.command(&["run"]), expected.as_bytes());
    }
    let project = Project::new(LARGE_HEAP, Some("hosted"), false);
    for (limit, expected) in [
        ("20971551", "42\n"),
        ("20971552", "43\n"),
        ("67108864", "43\n"),
    ] {
        project.heap_limit(limit);
        success(project.command(&["run"]), expected.as_bytes());
    }
}

#[test]
fn invalid_heap_settings_fail_before_tools_or_modifying_previous_outputs() {
    for value in [
        "-1",
        "1.5",
        "true",
        "\"40\"",
        "18446744073709551616",
        "1 0",
        "",
        "+40",
    ] {
        let project = Project::new("fn main()->i32{return 0;}", Some("hosted"), false);
        project.heap_limit(value);
        fs::create_dir_all(project.0.join(".oxid/bin")).unwrap();
        let artifact = project.0.join(".oxid/bin/app");
        fs::write(&artifact, b"previous executable").unwrap();
        for command in ["check", "run", "build", "test"] {
            let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
                .arg(command)
                .current_dir(&project.0)
                .env("OXID_LLVM_BIN", project.0.join("missing-llvm"))
                .env("PATH", project.0.join("missing-path"))
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1), "{value:?}: {output:?}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("heap-limit"),
                "{output:?}"
            );
            assert_eq!(fs::read(&artifact).unwrap(), b"previous executable");
            assert!(!project.0.join("oxid.lock").exists());
        }
    }
    for settings in [
        "[execution]\nheap-limit=40\n",
        "[execution]\npolicy=\"bounded\"\nheap-limit=40\n",
        "[execution]\npolicy=\"hosted\"\nheap-limit=40\nheap-limit=41\n",
    ] {
        let project = Project::new("fn main()->i32{return 0;}", None, false);
        let path = project.0.join("oxid.toml");
        let text = fs::read_to_string(&path).unwrap();
        fs::write(path, text + settings).unwrap();
        let result = project.command(&["check"]);
        assert!(!result.status.success(), "{result:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("heap-limit"),
            "{result:?}"
        );
        assert!(!project.0.join("oxid.lock").exists());
    }
}

#[test]
#[cfg(target_os = "linux")]
fn workspace_members_and_declared_tests_use_their_own_heap_budgets() {
    let project = Project::new("fn main()->i32{return 0;}", None, false);
    let workspace = "[workspace]\nedition=\"typed-preview\"\nmembers=[\"small\",\"large\"]\n";
    fs::write(project.0.join("oxid.toml"), workspace).unwrap();
    for (name, limit, expected) in [("small", 0, 1), ("large", 88, 2)] {
        let root = project.0.join(name);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), HEAP_GROWTH).unwrap();
        let probe = HEAP_GROWTH.replace("fn main()", "fn probe()");
        fs::write(root.join("src/check.ox"), format!("{probe}\nfn main()->i32{{let actual=probe();if actual=={expected}{{return 0;}}return 1;}}\n")).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname=\"{name}\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n[tests]\nheap=\"src/check.ox\"\n[execution]\npolicy=\"hosted\"\nheap-limit={limit}\n")).unwrap();
    }
    for (name, expected) in [("small", 1), ("large", 2)] {
        success(
            project.command(&["run", "--package", name]),
            format!("{expected}\n").as_bytes(),
        );
    }
    let result = project.command(&["test", "--workspace", "--message-format=json"]);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(
        String::from_utf8_lossy(&result.stdout)
            .matches("\"kind\":\"test-result\"")
            .count(),
        2
    );
    let previous_lock = fs::read(project.0.join("oxid.lock")).ok();
    fs::write(
        project.0.join("oxid.toml"),
        format!("{workspace}[execution]\npolicy=\"hosted\"\nheap-limit=999\n"),
    )
    .unwrap();
    let refused = project.command(&["check", "--workspace"]);
    assert!(!refused.status.success(), "{refused:?}");
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("virtual workspace"),
        "{refused:?}"
    );
    assert_eq!(fs::read(project.0.join("oxid.lock")).ok(), previous_lock);
}

#[test]
#[cfg(target_os = "linux")]
fn a_library_cannot_increase_its_calling_binary_heap_budget() {
    let project = Project::new(
        "use crate::library::probe;fn main()->i32{return probe();}",
        Some("hosted"),
        false,
    );
    fs::create_dir_all(project.0.join("library/src")).unwrap();
    fs::write(
        project.0.join("library/src/lib.ox"),
        HEAP_GROWTH.replace("fn main()", "pub fn probe()"),
    )
    .unwrap();
    fs::write(project.0.join("library/oxid.toml"), "[project]\nname=\"library\"\nkind=\"lib\"\nedition=\"typed-preview\"\nentry=\"src/lib.ox\"\n[execution]\npolicy=\"hosted\"\nheap-limit=67108864\n").unwrap();
    for (limit, expected) in [("0", "1\n"), ("87", "3\n"), ("88", "2\n")] {
        project.heap_limit(limit);
        let path = project.0.join("oxid.toml");
        let text = fs::read_to_string(&path).unwrap();
        fs::write(path, text + "[dependencies]\nlibrary=\"library\"\n").unwrap();
        success(project.command(&["run"]), expected.as_bytes());
    }
}

#[test]
#[ignore = "requires native Linux and LLVM 19.1.7"]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn native_heap_settings_apply_after_cache_hits_and_source_free_deployment() {
    for profile in ["dev", "release"] {
        let project = Project::new(HEAP_GROWTH, Some("hosted"), false);
        for (index, (limit, expected)) in
            [("87", "3\n"), ("88", "2\n"), ("0", "1\n"), ("87", "3\n")]
                .into_iter()
                .enumerate()
        {
            project.heap_limit(limit);
            let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
            command
                .args([
                    "build",
                    "--incremental",
                    "--profile",
                    profile,
                    "--message-format=json",
                ])
                .current_dir(&project.0);
            if let Some(llvm) = std::env::var_os("OXID_TEST_LLVM_BIN") {
                command.env("OXID_LLVM_BIN", llvm);
            }
            let built = command.output().unwrap();
            assert!(built.status.success(), "{built:?}");
            let expected_status = if index == 0 { "miss" } else { "hit" };
            assert!(
                String::from_utf8_lossy(&built.stdout)
                    .contains(&format!("\"status\":\"{expected_status}\"")),
                "{built:?}"
            );
            success(
                Command::new(project.0.join(".oxid/bin/app"))
                    .env_clear()
                    .output()
                    .unwrap(),
                expected.as_bytes(),
            );
        }
        let deployed = project.0.join("deployed");
        fs::copy(project.0.join(".oxid/bin/app"), &deployed).unwrap();
        fs::remove_dir_all(project.0.join("src")).unwrap();
        fs::remove_dir_all(project.0.join(".oxid")).unwrap();
        fs::remove_file(project.0.join("oxid.toml")).unwrap();
        if project.0.join("oxid.lock").exists() {
            fs::remove_file(project.0.join("oxid.lock")).unwrap();
        }
        success(Command::new(deployed).env_clear().output().unwrap(), b"3\n");
        let project = Project::new(LARGE_HEAP, Some("hosted"), false);
        for (limit, expected) in [
            ("20971551", "42\n"),
            ("20971552", "43\n"),
            ("67108864", "43\n"),
            ("18446744073709551615", "43\n"),
        ] {
            project.heap_limit(limit);
            let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
            command
                .args(["build", "--profile", profile])
                .current_dir(&project.0);
            if let Some(llvm) = std::env::var_os("OXID_TEST_LLVM_BIN") {
                command.env("OXID_LLVM_BIN", llvm);
            }
            let built = command.output().unwrap();
            assert!(built.status.success(), "{built:?}");
            success(
                Command::new(project.0.join(".oxid/bin/app"))
                    .env_clear()
                    .output()
                    .unwrap(),
                expected.as_bytes(),
            );
        }
    }
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
    if !cfg!(target_os = "linux") {
        let refused = project.command(&["run"]);
        assert_eq!(refused.status.code(), Some(1), "{refused:?}");
        assert!(refused.stdout.is_empty(), "{refused:?}");
        assert!(
            String::from_utf8_lossy(&refused.stderr).contains("E0006"),
            "{refused:?}"
        );
        return;
    }
    fuel_failure(&project.command(&["run"]));
    fs::write(&library, manifest.replace("\"hosted\"", "\"bounded\"")).unwrap();
    root.push_str("[execution]\npolicy = \"hosted\"\n");
    fs::write(root_manifest, root).unwrap();
    success(project.command(&["run"]), b"200000\n");
}

#[test]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
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
    if cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
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
        let project = Project::new("fn main()->i32{return 0;}", None, false);
        let path = project.0.join("oxid.toml");
        let mut original = fs::read_to_string(&path).unwrap();
        original.push_str("[tests]\nsmoke = \"src/main.ox\"\n");
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
fn malformed_execution_tables_and_values_cannot_inherit_build_settings() {
    for settings in [
        "[execution\npolicy = \"hosted\"\n",
        "[execution # missing bracket\npolicy = \"hosted\"\n",
        "[execution] trailing\npolicy = \"hosted\"\n",
        "[execution]]\npolicy = \"hosted\"\n",
        "[execution extra]\npolicy = \"hosted\"\n",
        "[\"execution\"] trailing\npolicy = \"hosted\"\n",
        "[\"execution]\npolicy = \"hosted\"\n",
        "['execution]\npolicy = \"hosted\"\n",
        "[\"execution] # comment\npolicy = \"hosted\"\n",
        "['execution] # comment\npolicy = \"hosted\"\n",
        "[\"execu\\u0074ion] # comment\npolicy = \"hosted\"\n",
        "[\"execution']\npolicy = \"hosted\"\n",
        "[[[execution]]]\npolicy = \"hosted\"\n",
        "[execution]\npolicy = \"hosted\",\n",
        "[execution]\npolicy = 'hosted',\n",
        "[execution]\npolicy = \"hosted\", # invalid comma\n",
        "[execution]\n[execution]\n",
        "[metadata]\nmatrix = [\n[execution]\npolicy = \"hosted\"\n",
    ] {
        let project = Project::new("fn main()->i32{return 0;}", None, false);
        let manifest = project.0.join("oxid.toml");
        let original = fs::read_to_string(&manifest).unwrap();
        fs::write(
            &manifest,
            format!("{original}{settings}[tests]\nsmoke = \"src/main.ox\"\n"),
        )
        .unwrap();
        fs::create_dir_all(project.0.join(".oxid/bin")).unwrap();
        let artifact = project.0.join(".oxid/bin/app");
        fs::write(&artifact, b"previous executable").unwrap();
        for command in ["check", "run", "test", "build"] {
            let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
                .arg(command)
                .current_dir(&project.0)
                .env("OXID_LLVM_BIN", project.0.join("absent-llvm"))
                .env("PATH", project.0.join("absent-path"))
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1), "{settings:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(
                error.contains("E0001")
                    && (error.contains("execution") || error.contains("manifest")),
                "{settings:?}: {output:?}"
            );
            assert_eq!(fs::read(&artifact).unwrap(), b"previous executable");
        }
    }
}

#[test]
fn quoted_execution_names_and_comments_remain_valid() {
    for settings in [
        "[execution] # table comment,\npolicy = \"hosted\" # value comment,\n",
        "['execution']\n'policy' = 'hosted' # comment,\n",
        "[\"execu\\u0074ion\"]\n\"policy\" = \"hos\\u0074ed\"\n",
        "[metadata]\nmatrix = [\n[1, 2],\n[3, 4],\n]\n[execution]\npolicy = \"hosted\"\n",
        "[metadata]\nmatrix = [\n[],\n[\"[execution]\", \"# a comment in data\"],\n]\n[execution]\npolicy = \"hosted\"\n",
    ] {
        let project = Project::new(SCALAR, None, false);
        let path = project.0.join("oxid.toml");
        let original = fs::read_to_string(&path).unwrap();
        fs::write(path, original + settings).unwrap();
        success(project.command(&["run"]), b"200000\n");
    }
}

#[test]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn workspace_declared_tests_use_the_owning_binary_or_library_execution_policy() {
    let project = Project::new("fn main()->i32{return 0;}", None, false);
    fs::write(
        project.0.join("oxid.toml"),
        "[workspace]\nedition = \"typed-preview\"\nmembers = [\"app\", \"library\"]\n",
    )
    .unwrap();
    for (member, library) in [("app", false), ("library", true)] {
        let root = project.0.join(member);
        fs::create_dir_all(root.join("src")).unwrap();
        let entry = if library { "src/lib.ox" } else { "src/main.ox" };
        fs::write(
            root.join(entry),
            if library {
                "pub fn value()->i32{return 1;}"
            } else {
                "fn main()->i32{return 0;}"
            },
        )
        .unwrap();
        fs::write(
            root.join("src/long.ox"),
            "fn main()->i32{let mut i=0;while i<200000{i=i+1;}return 0;}",
        )
        .unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname = \"{member}\"\nedition = \"typed-preview\"\nkind = \"{}\"\nentry = \"{entry}\"\n[tests]\nlong = \"src/long.ox\"\n[execution]\npolicy = \"hosted\"\n", if library { "lib" } else { "bin" })).unwrap();
    }
    let hosted = project.command(&["test", "--workspace", "--message-format=json"]);
    assert_eq!(hosted.status.code(), Some(0), "{hosted:?}");
    assert!(hosted.stderr.is_empty(), "{hosted:?}");
    let body = String::from_utf8(hosted.stdout).unwrap();
    assert_eq!(body.matches("\"kind\":\"test-result\"").count(), 2);
    assert!(body.contains("\"failed\":0"), "{body}");
    for member in ["app", "library"] {
        let path = project.0.join(member).join("oxid.toml");
        fs::write(
            &path,
            fs::read_to_string(&path)
                .unwrap()
                .replace("\"hosted\"", "\"bounded\""),
        )
        .unwrap();
    }
    let bounded = project.command(&["test", "--workspace", "--message-format=json"]);
    assert_eq!(bounded.status.code(), Some(1), "{bounded:?}");
    assert!(bounded.stderr.is_empty(), "{bounded:?}");
    let body = String::from_utf8(bounded.stdout).unwrap();
    assert!(
        body.contains("E0601") && body.contains("\"failed\":2"),
        "{body}"
    );
}

#[test]
#[ignore = "requires Linux x86_64 or ARM64 LLVM 19.1.7 in OXID_LLVM_BIN"]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
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
            if source == MEMORY {
                success(failed, b"42\n");
            } else {
                fuel_failure(&failed);
            }
            project.manifest(Some("hosted"), false);
        }
    }
}

#[test]
#[ignore = "requires Linux x86_64 or ARM64 LLVM 19.1.7 in OXID_LLVM_BIN"]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
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

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn success_build(output: Output) {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}
