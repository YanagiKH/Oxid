//! Workspace commands use ordinary package graphs and retained compiler inputs.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, MutexGuard,
    },
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
static FIXTURE_EXECUTION: Mutex<()> = Mutex::new(());
struct Workspace {
    root: PathBuf,
    _execution: MutexGuard<'static, ()>,
}
impl Workspace {
    fn new() -> Self {
        // A native fixture copies its compiler; other workspace children must
        // not inherit that copy's temporarily writable descriptor.
        let execution = FIXTURE_EXECUTION
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = std::env::temp_dir().join(format!(
            "oxid-workspace-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let workspace = Self {
            root,
            _execution: execution,
        };
        workspace.write("oxid.toml", "[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/one\", \"apps/two\", \"libs/math\"]\n");
        for (member, status) in [("one", 7), ("two", 9)] {
            workspace.write(&format!("apps/{member}/oxid.toml"), &format!("[project]\nname = \"{member}\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[dependencies]\nmath = \"../../libs/math\"\n[tests]\nsmoke = \"tests/smoke.ox\"\n"));
            workspace.write(
                &format!("apps/{member}/src/main.ox"),
                &format!(
                    "use crate::math::value;\nfn main() -> i32 {{ return value() + {status}; }}\n"
                ),
            );
            workspace.write(
                &format!("apps/{member}/tests/smoke.ox"),
                "use crate::math::value;\nfn main() -> i32 { return value(); }\n",
            );
        }
        workspace.write("libs/math/oxid.toml", "[project]\nname = \"math\"\nedition = \"typed-preview\"\nkind = \"lib\"\n[tests]\nsmoke = \"tests/smoke.ox\"\n");
        workspace.write(
            "libs/math/src/lib.ox",
            "pub fn value() -> i32 { return 0; }\n",
        );
        workspace.write(
            "libs/math/tests/smoke.ox",
            "fn main() -> i32 { return 0; }\n",
        );
        workspace
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn command(&self, directory: &str, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(self.root.join(directory))
            .args(args)
            .stdin(Stdio::null())
            .env("OXID_CACHE_DIR", self.root.join("cache"))
            .env_remove("OXID_PATH");
        command
    }
    fn run(&self, directory: &str, args: &[&str]) -> Output {
        self.command(directory, args).output().unwrap()
    }
    fn no_outputs(&self) {
        for path in [
            "oxid.lock",
            ".oxid",
            "cache",
            "apps/one/.oxid",
            "apps/two/.oxid",
            "libs/math/.oxid",
            "apps/one/oxid.lock",
            "apps/two/oxid.lock",
            "libs/math/oxid.lock",
        ] {
            assert!(!self.root.join(path).exists(), "unexpected {path}");
        }
    }
    fn qualified(&self) -> bool {
        if cfg!(target_os = "linux") {
            return true;
        }
        failure(
            self.run("", &["check"]),
            "package source policy is qualified only on Linux",
        );
        self.no_outputs();
        false
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn success(output: Output) -> Output {
    assert!(output.status.success(), "{output:?}");
    output
}
fn failure(output: Output, message: &str) -> Output {
    assert!(!output.status.success(), "{output:?}");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains(message), "expected {message:?}: {text}");
    output
}

#[test]
fn virtual_workspace_checks_members_and_selects_a_single_process_without_extra_stdout() {
    let workspace = Workspace::new();
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["check"]));
    failure(workspace.run("", &["run"]), "one binary member");
    let result = success(workspace.run(
        "",
        &[
            "run",
            "-p",
            "one",
            "--entry-mode=result",
            "--message-format=json",
        ],
    ));
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("\"value\":7"),
        "{result:?}"
    );
    success(workspace.run("apps/one/src", &["check"]));
    if cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        let output = workspace.run("apps/one/src", &["run"]);
        assert_eq!(output.status.code(), Some(7), "{output:?}");
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
        success(workspace.run("", &["test", "--workspace"]));
    }
    workspace.no_outputs();
}

#[test]
fn defaults_member_directory_and_path_selection_have_deterministic_scope() {
    let workspace = Workspace::new();
    workspace.write("oxid.toml", "[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/one\", \"apps/two\", \"libs/math\"]\ndefault-members = [\"apps/two\"]\n");
    if !workspace.qualified() {
        return;
    }
    let output =
        success(workspace.run("", &["run", "--entry-mode=result", "--message-format=json"]));
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"value\":9"));
    workspace.write(
        "apps/two/src/main.ox",
        "fn main() -> i32 { return false; }\n",
    );
    success(workspace.run("apps/one/src", &["check"]));
    success(workspace.run("", &["check", "--package=apps/one"]));
    failure(workspace.run("apps/one", &["check", "--workspace"]), "type");
    failure(workspace.run("", &["check"]), "type");
    failure(
        workspace.run("", &["check", "-p", "missing"]),
        "unknown workspace member",
    );
    workspace.no_outputs();
}

#[test]
fn same_names_are_selectable_by_member_path_without_collapsing_identity() {
    let workspace = Workspace::new();
    let manifest = fs::read_to_string(workspace.root.join("apps/two/oxid.toml")).unwrap();
    workspace.write(
        "apps/two/oxid.toml",
        &manifest.replace("name = \"two\"", "name = \"one\""),
    );
    failure(workspace.run("", &["check", "-p", "one"]), "ambiguous");
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["check", "-p", "apps/one"]));
    let output = success(workspace.run(
        "",
        &[
            "run",
            "-p",
            "apps/two",
            "--entry-mode=result",
            "--message-format=json",
        ],
    ));
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"value\":9"));
    success(workspace.run("", &["lock"]));
    let lock = fs::read_to_string(workspace.root.join("oxid.lock")).unwrap();
    assert_eq!(lock.matches("[[package]]").count(), 3);
    assert_eq!(lock.matches("name = \"one\"").count(), 2);
}

#[test]
fn package_root_joins_the_roster_and_root_commands_use_workspace_defaults() {
    let workspace = Workspace::new();
    workspace.write("oxid.toml", "[project]\nname = \"root\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/one\", \"libs/math\"]\ndefault-members = [\"apps/one\"]\n");
    workspace.write("src/main.ox", "fn main() -> i32 { return 23; }\n");
    if !workspace.qualified() {
        return;
    }
    let output =
        success(workspace.run("", &["run", "--entry-mode=result", "--message-format=json"]));
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"value\":7"));
    for (directory, selector) in [("", Some(".")), ("src", None)] {
        let mut args = vec!["run", "--entry-mode=result", "--message-format=json"];
        if let Some(selector) = selector {
            args.extend(["-p", selector]);
        }
        let output = success(workspace.run(directory, &args));
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("\"value\":23"),
            "{output:?}"
        );
    }
    success(workspace.run("", &["lock"]));
    let lock = fs::read_to_string(workspace.root.join("oxid.lock")).unwrap();
    assert_eq!(lock.matches("[[member]]").count(), 3);
    assert!(lock.contains("path = \".\""));
    success(workspace.run("", &["check", "--frozen", "--workspace"]));
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn package_root_declared_tests_keep_their_member_when_defaults_select_another_package() {
    let workspace = Workspace::new();
    workspace.write("oxid.toml", "[project]\nname = \"root\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[tests]\nsmoke = \"tests/root.ox\"\n[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/one\", \"libs/math\"]\ndefault-members = [\"apps/one\"]\n");
    workspace.write("src/main.ox", "fn main() -> i32 { return 0; }\n");
    workspace.write("tests/root.ox", "fn main() -> i32 { return 0; }\n");
    workspace.write(
        "apps/one/tests/smoke.ox",
        "fn main() -> i32 { return 13; }\n",
    );
    success(workspace.run("", &["lock"]));
    success(workspace.run("", &["test", "-p", ".", "--frozen"]));
    failure(
        workspace.run(
            "apps/one/src",
            &[
                "run",
                "--edition=typed-preview",
                "--entry-mode=process",
                "--project-test=smoke",
            ],
        ),
        "requires the member root",
    );
}

#[test]
fn library_members_are_really_checked_and_cannot_be_executed_as_binaries() {
    let workspace = Workspace::new();
    failure(
        workspace.run("", &["run", "-p", "math"]),
        "one binary member",
    );
    failure(
        workspace.run("", &["build", "-p", "math"]),
        "do not produce standalone executables",
    );
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["check", "-p", "math"]));
    workspace.write(
        "libs/math/src/lib.ox",
        "pub fn value() -> i32 { return false; }\n",
    );
    failure(workspace.run("", &["check", "-p", "math"]), "type");
    failure(workspace.run("", &["build", "--workspace"]), "type");
    workspace.no_outputs();
}

#[test]
fn library_only_workspace_build_checks_without_tools_or_artifacts() {
    let workspace = Workspace::new();
    workspace.write(
        "oxid.toml",
        "[workspace]\nedition = \"typed-preview\"\nmembers = [\"libs/math\"]\n",
    );
    let output = success(
        workspace
            .command(
                "",
                &[
                    "build",
                    "--workspace",
                    "--incremental",
                    "--message-format=json",
                ],
            )
            .env("OXID_LLVM_BIN", workspace.root.join("no-tools"))
            .output()
            .unwrap(),
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"status\":\"checked-library\""), "{text}");
    assert!(text.contains("\"success\":true"), "{text}");
    assert!(!text.contains("\"kind\":\"build-cache\""), "{text}");
    workspace.no_outputs();
}

#[test]
fn binary_members_are_not_silently_imported_as_libraries() {
    let workspace = Workspace::new();
    let manifest = fs::read_to_string(workspace.root.join("libs/math/oxid.toml")).unwrap();
    workspace.write(
        "libs/math/oxid.toml",
        &manifest.replace("kind = \"lib\"\n", "entry = \"src/lib.ox\"\n"),
    );
    failure(
        workspace.run("", &["check", "-p", "one"]),
        "binary workspace members cannot be imported",
    );
    workspace.no_outputs();
}

#[test]
fn selection_and_unsupported_options_fail_before_effects() {
    let workspace = Workspace::new();
    for args in [
        vec!["lock", "--package"],
        vec!["lock", "--package="],
        vec!["lock", "-p", "one", "--package=two"],
        vec!["lock", "-p", "one", "--workspace"],
        vec!["lock", "--workspace", "--workspace"],
        vec!["lock", "--workspace=false"],
        vec!["lock", "--unexpected"],
        vec!["build", "--experimental-hir-import=on"],
        vec!["build", "--workspace", "--incremental", "--incremental"],
        vec!["build", "--workspace", "--incremental=true"],
        vec!["lock", "--entry-mode=result"],
        vec!["test", "--entry-mode=process"],
        vec!["lock", "--update", "--offline"],
        vec!["build", "--update"],
        vec!["check", "--", "--workspace"],
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=smoke",
            "-p",
            "one",
        ],
    ] {
        let output = workspace.run("", &args);
        assert!(!output.status.success(), "{args:?}: {output:?}");
        workspace.no_outputs();
    }
}

#[test]
fn member_declarations_reject_escape_duplicates_mixed_editions_and_nested_workspaces() {
    let workspace = Workspace::new();
    for (declaration, message) in [
        ("[workspace]\nedition = \"typed-preview\"\nmembers = [\"../outside\"]\n", "exact relative directories"),
        ("[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/*\"]\n", "exact relative directories"),
        ("[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/one\", \"apps/one\"]\n", "duplicate canonical"),
        ("[workspace]\nedition = \"typed-preview\"\nmembers = [\".\"]\n", "virtual workspace root"),
        ("[workspace]\nedition = \"typed-preview\"\nmembers = []\n", "between 1 and 32"),
        ("[workspace]\nedition = \"legacy-0.9\"\nmembers = [\"apps/one\"]\n", "explicit edition"),
        ("[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/one\"]\ndefault-members = [\"apps/two\"]\n", "not declared"),
    ] {
        workspace.write("oxid.toml", declaration);
        failure(workspace.run("", &["lock"]), message);
        workspace.no_outputs();
    }
    workspace.write(
        "oxid.toml",
        "[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/one\", \"libs/math\"]\n",
    );
    let member = fs::read_to_string(workspace.root.join("apps/one/oxid.toml")).unwrap();
    workspace.write(
        "apps/one/oxid.toml",
        &member.replace("typed-preview", "legacy-0.9"),
    );
    failure(workspace.run("", &["lock"]), "mixed legacy");
    workspace.write(
        "apps/one/oxid.toml",
        &(member.clone() + "[workspace]\nedition = \"typed-preview\"\nmembers = []\n"),
    );
    failure(workspace.run("", &["lock"]), "nested workspace");
    failure(workspace.run("apps/one", &["check"]), "nested workspaces");
    workspace.write("apps/one/oxid.toml", &member);
    workspace.write(
        "loose/oxid.toml",
        "[project]\nname = \"loose\"\nedition = \"typed-preview\"\n",
    );
    failure(
        workspace.run("loose", &["check"]),
        "not a declared workspace member",
    );
    workspace.no_outputs();
}

#[test]
fn quoted_multiline_workspace_arrays_work_and_metadata_cannot_inject_a_workspace() {
    let workspace = Workspace::new();
    workspace.write("oxid.toml", "[\"workspace\"] # table\nedition = 'typed-preview'\nmembers = [\n  'apps/one', # app\n  \"libs/math\",\n]\n\"default-members\" = [ \"apps/one\", ]\n");
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["check"]));
    success(workspace.run("", &["lock"]));
    for declaration in [
        "[workspace\nedition = \"typed-preview\"\nmembers = [\"apps/one\"]\n",
        "[[workspace]]\nedition = \"typed-preview\"\nmembers = [\"apps/one\"]\n",
        "workspace.members = [\"apps/one\"]\n",
        "[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/one\"]\nmembers = [\"libs/math\"]\n",
        "[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/one\"]\nregistry = \"hidden\"\n",
    ] {
        workspace.write("oxid.toml", declaration);
        assert!(!workspace.run("", &["check"]).status.success());
    }
    fs::remove_file(workspace.root.join("oxid.lock")).unwrap();
    workspace.write("oxid.toml", "[project]\nname = \"standalone\"\nedition = \"typed-preview\"\nentry = \"main.ox\"\n[metadata]\nworkspace = \"ordinary metadata\"\n");
    workspace.write("main.ox", "fn main() -> i32 { return 0; }\n");
    success(workspace.run("", &["check"]));
    success(workspace.run("", &["lock"]));
    let lock = fs::read_to_string(workspace.root.join("oxid.lock")).unwrap();
    assert!(lock.starts_with("version = 2\nkind = \"typed-local-v1\"\n"));
    fs::remove_file(workspace.root.join("oxid.lock")).unwrap();
    workspace.write(
        "oxid.toml",
        "[metadata]\ntext = '''\n[workspace]\nmembers = [\"missing\"]\n'''\n",
    );
    workspace.write("main.ox", "print 5 / 2;\n");
    let output = success(workspace.run("", &["run", "main.ox"]));
    assert!(String::from_utf8_lossy(&output.stdout).contains("2.5"));
}

#[test]
fn workspace_lock_covers_union_and_selection_cannot_hide_unselected_drift() {
    let workspace = Workspace::new();
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["check", "--offline", "-p", "one"]));
    workspace.no_outputs();
    failure(
        workspace.run("", &["check", "--locked", "-p", "one"]),
        "requires oxid.lock",
    );
    success(workspace.run("", &["lock", "-p", "one"]));
    let original = fs::read(workspace.root.join("oxid.lock")).unwrap();
    let text = String::from_utf8(original.clone()).unwrap();
    assert!(
        text.starts_with("version = 3\nkind = \"typed-workspace-v1\"\n"),
        "{text}"
    );
    assert_eq!(text.matches("[[package]]").count(), 3);
    assert_eq!(text.matches("[[member]]").count(), 3);
    assert_eq!(text.matches("[[dependency]]").count(), 2);
    assert_eq!(text.matches("sources = \"root\"").count(), 2);
    assert_eq!(text.matches("sources = \"sha256:").count(), 1);
    success(workspace.run("apps/two/src", &["check", "--frozen"]));
    let unselected_manifest =
        fs::read_to_string(workspace.root.join("apps/two/oxid.toml")).unwrap();
    workspace.write(
        "apps/two/oxid.toml",
        &(unselected_manifest.clone() + "# changed unselected package\n"),
    );
    failure(
        workspace.run("", &["check", "-p", "one"]),
        "lock is out of date",
    );
    assert_eq!(
        fs::read(workspace.root.join("oxid.lock")).unwrap(),
        original
    );
    workspace.write("apps/two/oxid.toml", &unselected_manifest);
    workspace.write(
        "libs/math/src/lib.ox",
        "pub fn value() -> i32 { return 4; }\n",
    );
    failure(workspace.run("", &["lock"]), "lock is out of date");
    failure(
        workspace.run("", &["run", "-p", "one", "--entry-mode=result"]),
        "lock is out of date",
    );
    success(workspace.run("", &["lock", "--update"]));
    assert_ne!(
        fs::read(workspace.root.join("oxid.lock")).unwrap(),
        original
    );
    success(workspace.run("", &["check", "--frozen", "--workspace"]));
}

#[test]
fn shared_lock_allows_binary_and_test_edits_and_locked_tests_without_main_sources() {
    let workspace = Workspace::new();
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["lock"]));
    let original = fs::read(workspace.root.join("oxid.lock")).unwrap();
    workspace.write("apps/one/src/main.ox", "fn main() -> i32 { return 41; }\n");
    workspace.write(
        "apps/one/tests/smoke.ox",
        "fn main() -> i32 { return 0; }\n",
    );
    let output = success(workspace.run(
        "",
        &[
            "run",
            "-p",
            "one",
            "--frozen",
            "--entry-mode=result",
            "--message-format=json",
        ],
    ));
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"value\":41"));
    if cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        fs::remove_file(workspace.root.join("apps/one/src/main.ox")).unwrap();
        fs::remove_file(workspace.root.join("apps/two/src/main.ox")).unwrap();
        success(workspace.run("", &["test", "--workspace", "--frozen"]));
        success(workspace.run("apps/one", &["test", "--locked"]));
    }
    assert_eq!(
        fs::read(workspace.root.join("oxid.lock")).unwrap(),
        original
    );
}

#[test]
fn complete_workspace_relocation_preserves_relative_identity_and_lock_bytes() {
    let mut workspace = Workspace::new();
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["lock"]));
    let original = fs::read(workspace.root.join("oxid.lock")).unwrap();
    let relocated = workspace.root.with_extension("relocated");
    fs::rename(&workspace.root, &relocated).unwrap();
    workspace.root = relocated;
    success(workspace.run("", &["check", "--frozen"]));
    success(workspace.run("apps/one", &["lock"]));
    assert_eq!(
        fs::read(workspace.root.join("oxid.lock")).unwrap(),
        original
    );
}

#[test]
fn shared_lock_ownership_survives_a_removed_workspace_declaration() {
    let workspace = Workspace::new();
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["lock"]));
    let original = fs::read(workspace.root.join("oxid.lock")).unwrap();
    for manifest in [
        "# declaration was removed\n",
        "[workspace # missing bracket\nedition = \"typed-preview\"\nmembers = [\"apps/one\"]\n",
        "[\"workspace\" # missing bracket\nedition = \"typed-preview\"\nmembers = [\"apps/one\"]\n",
    ] {
        workspace.write("oxid.toml", manifest);
        let output = workspace.run("apps/one", &["check"]);
        assert!(!output.status.success(), "{output:?}");
        assert_eq!(
            fs::read(workspace.root.join("oxid.lock")).unwrap(),
            original
        );
    }
    fs::remove_file(workspace.root.join("oxid.toml")).unwrap();
    failure(
        workspace.run("apps/one", &["check"]),
        "requires its [workspace] manifest declaration",
    );
    assert!(!workspace.root.join("apps/one/oxid.lock").exists());
    assert!(!workspace.root.join("apps/one/.oxid").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn ancestor_manifest_size_is_admitted_before_allocation_and_explicit_files_keep_their_routes() {
    use std::{io::Write, os::unix::process::CommandExt};
    let workspace = Workspace::new();
    let mut oversized = fs::File::create(workspace.root.join("oxid.toml")).unwrap();
    oversized
        .write_all(b"[workspace]\nedition = \"typed-preview\"\nmembers = [\"apps/one\"]\n")
        .unwrap();
    oversized.set_len(128 * 1024 * 1024).unwrap();
    drop(oversized);
    let mut command = workspace.command(
        "apps/one",
        &["check", "--edition=typed-preview", "--message-format=json"],
    );
    // Sparse input plus a low address-space budget distinguishes metadata
    // admission from allocating the entire ancestor before checking its cap.
    unsafe {
        command.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: 64 * 1024 * 1024,
                rlim_max: 64 * 1024 * 1024,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &limit) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
    let output = failure(command.output().unwrap(), "1048576");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("out of memory"));
    workspace.write("plain.ox", "fn main() -> i32 { return 0; }\n");
    success(workspace.run("", &["check", "plain.ox", "--edition=typed-preview"]));
    workspace.no_outputs();
    workspace.write("legacy.ox", "print 5 / 2;\n");
    let output = success(workspace.run("", &["run", "legacy.ox", "--edition=legacy-0.9"]));
    assert!(String::from_utf8_lossy(&output.stdout).contains("2.5"));
}

#[test]
fn corrupt_incompatible_and_member_local_locks_are_preserved_even_with_update() {
    let workspace = Workspace::new();
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["lock"]));
    let valid = fs::read_to_string(workspace.root.join("oxid.lock")).unwrap();
    for bytes in [
        "version = 1\n".to_owned(),
        valid.replace("version = 3", "version = 900"),
        valid.clone() + "\nunknown = \"field\"\n",
        valid.replace("sources = \"sha256:", "sources = \"invalid:"),
        valid.replace("[[member]]", "[[unknown]]"),
    ] {
        workspace.write("oxid.lock", &bytes);
        let output = workspace.run("", &["lock", "--update"]);
        assert!(!output.status.success(), "{output:?}");
        assert_eq!(
            fs::read_to_string(workspace.root.join("oxid.lock")).unwrap(),
            bytes
        );
        assert!(!fs::read_dir(&workspace.root).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".oxid-lock-")));
    }
    workspace.write("oxid.lock", &valid);
    workspace.write("apps/one/oxid.lock", "version = 1\n");
    failure(
        workspace.run("", &["lock", "--update"]),
        "conflicting local oxid.lock",
    );
    assert_eq!(
        fs::read_to_string(workspace.root.join("oxid.lock")).unwrap(),
        valid
    );
    assert_eq!(
        fs::read_to_string(workspace.root.join("apps/one/oxid.lock")).unwrap(),
        "version = 1\n"
    );
}

#[test]
fn changed_workspace_roster_and_raw_root_manifest_require_explicit_update() {
    let workspace = Workspace::new();
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["lock"]));
    let original = fs::read(workspace.root.join("oxid.lock")).unwrap();
    let manifest = fs::read_to_string(workspace.root.join("oxid.toml")).unwrap();
    workspace.write(
        "oxid.toml",
        &(manifest.clone() + "# raw workspace policy bytes\n"),
    );
    failure(
        workspace.run("", &["check", "-p", "one"]),
        "lock is out of date",
    );
    workspace.write("oxid.toml", &manifest.replace("\"apps/two\", ", ""));
    failure(workspace.run("", &["lock"]), "lock is out of date");
    assert_eq!(
        fs::read(workspace.root.join("oxid.lock")).unwrap(),
        original
    );
    success(workspace.run("", &["lock", "--update"]));
    assert_eq!(
        fs::read_to_string(workspace.root.join("oxid.lock"))
            .unwrap()
            .matches("[[member]]")
            .count(),
        2
    );
}

#[test]
fn semantic_and_entry_refusals_preflight_every_selected_binary_before_native_tools() {
    let workspace = Workspace::new();
    if !workspace.qualified() {
        return;
    }
    for invalid in [
        "fn main() -> i32 { return false; }\n",
        "pub fn helper() -> i32 { return 0; }\n",
        "fn main(value: i32) -> i32 { return value; }\n",
    ] {
        workspace.write("apps/two/src/main.ox", invalid);
        let output = workspace
            .command("", &["build", "--workspace", "--message-format=json"])
            .env("OXID_LLVM_BIN", workspace.root.join("missing-tools"))
            .output()
            .unwrap();
        assert!(!output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("\"kind\":\"diagnostic\""), "{text}");
        assert!(
            !text.contains("toolchain") && !text.contains("missing-tools"),
            "{text}"
        );
        workspace.no_outputs();
    }
}

#[test]
fn stale_workspace_lock_stops_build_before_replacing_any_member_output() {
    let workspace = Workspace::new();
    if !workspace.qualified() {
        return;
    }
    success(workspace.run("", &["lock"]));
    let original = fs::read(workspace.root.join("oxid.lock")).unwrap();
    workspace.write("apps/one/.oxid/bin/one", "original one");
    workspace.write("apps/two/.oxid/bin/two", "original two");
    workspace.write(
        "libs/math/src/lib.ox",
        "pub fn value() -> i32 { return 99; }\n",
    );
    for args in [
        &["build", "--workspace", "--locked"][..],
        &["build", "-p", "one"],
        &["test", "--workspace", "--frozen"],
        &["run", "-p", "one", "--entry-mode=result"],
    ] {
        failure(
            workspace
                .command("", args)
                .env("OXID_LLVM_BIN", workspace.root.join("missing-tools"))
                .output()
                .unwrap(),
            "lock is out of date",
        );
        assert_eq!(
            fs::read_to_string(workspace.root.join("apps/one/.oxid/bin/one")).unwrap(),
            "original one"
        );
        assert_eq!(
            fs::read_to_string(workspace.root.join("apps/two/.oxid/bin/two")).unwrap(),
            "original two"
        );
        assert_eq!(
            fs::read(workspace.root.join("oxid.lock")).unwrap(),
            original
        );
    }
}

#[test]
fn explicit_file_editions_and_legacy_commands_keep_their_existing_boundaries() {
    let workspace = Workspace::new();
    workspace.write("plain.ox", "fn main() -> i32 { return 0; }\n");
    success(workspace.run("", &["check", "plain.ox", "--edition=typed-preview"]));
    failure(
        workspace.run("", &["check", "plain.ox"]),
        "explicit --edition",
    );
    success(workspace.run("", &["--version"]));
    success(workspace.run("apps/one", &["--help"]));
    workspace.no_outputs();
    workspace.write("legacy.ox", "print 5 / 2;\n");
    let output = success(workspace.run("", &["run", "legacy.ox", "--edition=legacy-0.9"]));
    assert!(String::from_utf8_lossy(&output.stdout).contains("2.5"));
    // The explicitly selected legacy runtime retains its existing cache effects.
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn tests_preflight_all_declared_entries_and_report_nonzero_status_with_clean_stdout() {
    let workspace = Workspace::new();
    workspace.write(
        "apps/one/tests/smoke.ox",
        "fn main() -> i32 { return 13; }\n",
    );
    workspace.write(
        "apps/two/tests/smoke.ox",
        "pub fn no_main() -> i32 { return 0; }\n",
    );
    let output = failure(
        workspace.run("", &["test", "--workspace", "--message-format=json"]),
        "process entry",
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("\"kind\":\"test-result\""),
        "{output:?}"
    );
    workspace.write(
        "apps/two/tests/smoke.ox",
        "fn main() -> i32 { return 0; }\n",
    );
    let output = failure(
        workspace.run("", &["test", "--workspace", "--message-format=json"]),
        "\"exit_status\":13",
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        text.matches("\"kind\":\"test-result\"").count(),
        3,
        "{text}"
    );
    assert!(text.contains("\"failed\":1"), "{text}");
    let output = failure(workspace.run("", &["test", "--workspace"]), "FAILED");
    assert!(output.stdout.is_empty(), "{output:?}");
    workspace.no_outputs();
}

#[cfg(unix)]
#[test]
fn symbolic_members_manifests_and_shared_locks_cannot_redirect_workspace_effects() {
    use std::os::unix::fs::symlink;
    let workspace = Workspace::new();
    symlink(workspace.root.join("apps/one"), workspace.root.join("link")).unwrap();
    workspace.write(
        "oxid.toml",
        "[workspace]\nedition = \"typed-preview\"\nmembers = [\"link\"]\n",
    );
    failure(workspace.run("", &["lock"]), "without symbolic links");
    workspace.write(
        "oxid.toml",
        "[workspace]\nedition = \"typed-preview\"\nmembers = [\"libs/math\"]\n",
    );
    fs::rename(
        workspace.root.join("libs/math/oxid.toml"),
        workspace.root.join("real-manifest"),
    )
    .unwrap();
    symlink(
        workspace.root.join("real-manifest"),
        workspace.root.join("libs/math/oxid.toml"),
    )
    .unwrap();
    failure(workspace.run("", &["lock"]), "regular file");
    fs::remove_file(workspace.root.join("libs/math/oxid.toml")).unwrap();
    fs::rename(
        workspace.root.join("real-manifest"),
        workspace.root.join("libs/math/oxid.toml"),
    )
    .unwrap();
    workspace.write("sentinel", "preserved");
    symlink(
        workspace.root.join("sentinel"),
        workspace.root.join("oxid.lock"),
    )
    .unwrap();
    failure(workspace.run("", &["lock", "--update"]), "regular file");
    assert_eq!(
        fs::read_to_string(workspace.root.join("sentinel")).unwrap(),
        "preserved"
    );
    assert!(fs::symlink_metadata(workspace.root.join("oxid.lock"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn member_count_and_aggregate_retained_source_limits_are_enforced() {
    let workspace = Workspace::new();
    let paths = (0..33)
        .map(|index| format!("\"member{index}\""))
        .collect::<Vec<_>>()
        .join(",");
    workspace.write(
        "oxid.toml",
        &format!("[workspace]\nedition = \"typed-preview\"\nmembers = [{paths}]\n"),
    );
    failure(workspace.run("", &["lock"]), "32-member limit");
    workspace.write(
        "oxid.toml",
        "[workspace]\nedition = \"typed-preview\"\nmembers = [\"libs/math\"]\n",
    );
    // Each file remains below the existing per-project bound. Repeated declared
    // tests must still fit one aggregate retained workspace input budget.
    let test_source = format!(
        "{}fn main() -> i32 {{ return 0; }}\n",
        format!("// {}\n", "x".repeat(50_000)).repeat(3)
    );
    let mut manifest =
        "[project]\nname = \"math\"\nedition = \"typed-preview\"\nkind = \"lib\"\n[tests]\n"
            .to_owned();
    for index in 0..60 {
        manifest.push_str(&format!("test{index:02} = \"tests/large.ox\"\n"));
    }
    workspace.write("libs/math/oxid.toml", &manifest);
    workspace.write("libs/math/tests/large.ox", &test_source);
    if cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        failure(workspace.run("", &["test"]), "8 MiB retained source limit");
    }
    workspace.no_outputs();
}

#[test]
fn standalone_local_lock_is_not_converted_when_a_workspace_is_added() {
    let workspace = Workspace::new();
    let manifest = fs::read_to_string(workspace.root.join("oxid.toml")).unwrap();
    workspace.write(
        "oxid.toml",
        "[project]\nname = \"former_root\"\nedition = \"typed-preview\"\nkind = \"lib\"\n",
    );
    workspace.write("src/lib.ox", "pub fn value() -> i32 { return 0; }\n");
    success(workspace.run("", &["lock"]));
    let standalone = fs::read(workspace.root.join("oxid.lock")).unwrap();
    assert!(standalone.starts_with(b"version = 2\n"));
    workspace.write("oxid.toml", &manifest);
    failure(
        workspace.run("", &["lock", "--update"]),
        "lock format does not match workspace membership",
    );
    assert_eq!(
        fs::read(workspace.root.join("oxid.lock")).unwrap(),
        standalone
    );
}

#[cfg(target_os = "linux")]
#[test]
fn shared_lock_and_member_manifest_special_files_fail_without_opening_a_fifo() {
    use std::{
        ffi::CString,
        os::unix::ffi::OsStrExt,
        thread,
        time::{Duration, Instant},
    };
    let workspace = Workspace::new();
    let fifo = workspace.root.join("oxid.lock");
    let encoded = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(encoded.as_ptr(), 0o600) }, 0);
    let run_bounded = |args: &[&str]| {
        let mut child = workspace
            .command("", args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if child.try_wait().unwrap().is_some() {
                return child.wait_with_output().unwrap();
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!("workspace command blocked on a special file: {output:?}");
            }
            thread::sleep(Duration::from_millis(10));
        }
    };
    failure(run_bounded(&["lock", "--update"]), "regular file");
    fs::remove_file(fifo).unwrap();
    let path = workspace.root.join("apps/one/oxid.toml");
    fs::remove_file(&path).unwrap();
    let encoded = CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(encoded.as_ptr(), 0o600) }, 0);
    failure(run_bounded(&["lock"]), "regular file");
    workspace.no_outputs();
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn inferred_process_diagnostics_remain_fallible_and_result_manifest_errors_are_json() {
    let workspace = Workspace::new();
    workspace.write("apps/one/src/main.ox", "this is invalid typed source;\n");
    let output = workspace
        .command("apps/one", &["run"])
        .stderr(
            fs::OpenOptions::new()
                .write(true)
                .open("/dev/full")
                .unwrap(),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(74), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let manifest = fs::read_to_string(workspace.root.join("apps/one/oxid.toml")).unwrap();
    workspace.write(
        "apps/one/oxid.toml",
        &(manifest + "[build]\nentry-mode = \"result\"\n"),
    );
    let output = failure(
        workspace.run("apps/one", &["run", "--locked", "--message-format=json"]),
        "requires oxid.lock",
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"kind\":\"run-summary\""));
    workspace.no_outputs();
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 in OXID_LLVM_BIN"]
fn workspace_native_build_keeps_every_retained_dependency_snapshot_and_separates_same_names() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = Workspace::new();
    let manifest = fs::read_to_string(workspace.root.join("apps/two/oxid.toml")).unwrap();
    workspace.write(
        "apps/two/oxid.toml",
        &manifest.replace("name = \"two\"", "name = \"one\""),
    );
    success(workspace.run("", &["lock"]));
    let original = fs::read(workspace.root.join("oxid.lock")).unwrap();
    let real = std::env::var_os("OXID_LLVM_BIN").expect("set OXID_LLVM_BIN to LLVM 19.1.7");
    let tools = workspace.root.join("tools");
    fs::create_dir(&tools).unwrap();
    for name in ["clang", "opt", "ld.lld"] {
        let script = tools.join(name);
        // A tool changes the on-disk shared library only after all retained
        // sessions and the shared lock have been admitted.
        fs::write(&script, "#!/bin/sh\nif [ \"$1\" = --version ]; then\n  printf '%s\\n' 'pub fn value() -> i32 { return 99; }' > \"$OXID_WORKSPACE_TEST_DEP\"\nfi\nexec \"$OXID_WORKSPACE_TEST_REAL/$(basename \"$0\")\" \"$@\"\n").unwrap();
        fs::set_permissions(script, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let output = success(
        workspace
            .command(
                "",
                &["build", "--workspace", "--frozen", "--message-format=json"],
            )
            .env("OXID_LLVM_BIN", tools)
            .env("OXID_WORKSPACE_TEST_REAL", real)
            .env(
                "OXID_WORKSPACE_TEST_DEP",
                workspace.root.join("libs/math/src/lib.ox"),
            )
            .output()
            .unwrap(),
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"status\":\"checked-library\""));
    let binaries = ["apps/one/.oxid/bin/one", "apps/two/.oxid/bin/one"];
    let saved = binaries.map(|path| fs::read(workspace.root.join(path)).unwrap());
    for (path, status) in binaries.iter().zip([7, 9]) {
        let result = Command::new(workspace.root.join(path)).output().unwrap();
        assert_eq!(result.status.code(), Some(status), "{result:?}");
        assert!(result.stdout.is_empty() && result.stderr.is_empty());
    }
    assert!(!workspace.root.join("libs/math/.oxid").exists());
    failure(
        workspace.run("", &["build", "--workspace", "--locked"]),
        "lock is out of date",
    );
    for (path, bytes) in binaries.iter().zip(saved) {
        assert_eq!(fs::read(workspace.root.join(path)).unwrap(), bytes);
    }
    assert_eq!(
        fs::read(workspace.root.join("oxid.lock")).unwrap(),
        original
    );
    fs::remove_dir_all(workspace.root.join("libs")).unwrap();
    fs::remove_dir_all(workspace.root.join("apps/one/src")).unwrap();
    fs::remove_dir_all(workspace.root.join("apps/two/src")).unwrap();
    for (path, status) in binaries.iter().zip([7, 9]) {
        let result = Command::new(workspace.root.join(path)).output().unwrap();
        assert_eq!(result.status.code(), Some(status), "{result:?}");
        assert!(result.stdout.is_empty() && result.stderr.is_empty());
    }
}

#[test]
fn native_doctor_admits_every_workspace_member_without_claiming_typechecks_or_running_code() {
    let workspace = Workspace::new();
    let doctor = |directory: &str| {
        workspace
            .command(directory, &["doctor", "--native", "--message-format=json"])
            .env("OXID_LLVM_BIN", workspace.root.join("missing-tools"))
            .output()
            .unwrap()
    };
    if !cfg!(target_os = "linux") {
        let output = failure(
            doctor(""),
            "package source policy is qualified only on Linux",
        );
        assert!(String::from_utf8_lossy(&output.stdout)
            .contains("\"id\":\"project\",\"status\":\"failed\""));
        workspace.no_outputs();
        return;
    }
    success(workspace.run("", &["lock"]));
    let lock = fs::read(workspace.root.join("oxid.lock")).unwrap();
    // Parsing succeeds, type checking fails, and the body must never execute.
    workspace.write(
        "apps/two/src/main.ox",
        "fn main() -> i32 { return false; }\n",
    );
    let output = doctor("apps/one/src");
    assert!(
        !output.status.success(),
        "the missing native tools must remain a separate failure"
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("\"id\":\"project\",\"status\":\"passed\""),
        "{text}"
    );
    assert!(
        text.contains("3 workspace member(s) and 3 local package(s) admitted"),
        "{text}"
    );
    assert!(
        text.contains("parsed, not executed or type checked"),
        "{text}"
    );
    assert!(text.contains("workspace oxid.lock verified"), "{text}");
    failure(workspace.run("", &["check", "-p", "two"]), "type");
    workspace.write(
        "libs/math/src/lib.ox",
        "pub fn value() -> i32 { return 4; }\n",
    );
    let output = failure(doctor("apps/one"), "lock is out of date");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("\"id\":\"project\",\"status\":\"failed\""));
    assert_eq!(fs::read(workspace.root.join("oxid.lock")).unwrap(), lock);
    for path in [
        ".oxid",
        "apps/one/.oxid",
        "apps/two/.oxid",
        "libs/math/.oxid",
        "cache",
    ] {
        assert!(!workspace.root.join(path).exists(), "doctor created {path}");
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 in OXID_LLVM_BIN"]
fn incremental_workspace_builds_keep_per_member_caches_behind_shared_lock_and_doctor_admission() {
    let workspace = Workspace::new();
    success(workspace.run("", &["lock"]));
    let lock = fs::read(workspace.root.join("oxid.lock")).unwrap();
    // Retain one compiler executable across the complete cache history.
    let compiler = workspace.root.join("retained-oxid");
    fs::copy(env!("CARGO_BIN_EXE_oxid"), &compiler).unwrap();
    let command = |args: &[&str]| {
        Command::new(&compiler)
            .current_dir(&workspace.root)
            .args(args)
            .env("OXID_CACHE_DIR", workspace.root.join("cache"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let args = [
        "build",
        "--workspace",
        "--incremental",
        "--frozen",
        "--message-format=json",
    ];
    for expected in ["miss", "hit"] {
        let output = success(command(&args));
        assert!(output.stderr.is_empty(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        let events = text
            .lines()
            .filter(|line| line.contains("\"kind\":\"build-cache\""))
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 2, "{text}");
        assert!(
            events
                .iter()
                .all(|line| line.contains(&format!("\"status\":\"{expected}\""))),
            "{text}"
        );
        assert!(text.contains("\"status\":\"checked-library\""), "{text}");
    }
    assert!(!workspace.root.join("libs/math/.oxid").exists());
    for (path, status) in [("apps/one/.oxid/bin/one", 7), ("apps/two/.oxid/bin/two", 9)] {
        let output = Command::new(workspace.root.join(path)).output().unwrap();
        assert_eq!(output.status.code(), Some(status), "{output:?}");
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    let output = success(command(&["doctor", "--native", "--message-format=json"]));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("\"toolchain_ready\":true,\"success\":true"),
        "{text}"
    );
    assert!(
        text.contains("\"id\":\"project\",\"status\":\"passed\""),
        "{text}"
    );
    assert!(text.contains("3 workspace member(s)"), "{text}");
    assert_eq!(fs::read(workspace.root.join("oxid.lock")).unwrap(), lock);

    workspace.write(
        "apps/one/src/main.ox",
        "use crate::math::value;\nfn main() -> i32 { return value() + 17; }\n",
    );
    let output = success(command(&args));
    let text = String::from_utf8(output.stdout).unwrap();
    let events = text
        .lines()
        .filter(|line| line.contains("\"kind\":\"build-cache\""))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2, "{text}");
    assert!(
        events[0].contains("\"status\":\"miss\"") && events[1].contains("\"status\":\"hit\""),
        "{text}"
    );
    let binaries = ["apps/one/.oxid/bin/one", "apps/two/.oxid/bin/two"];
    let saved = binaries.map(|path| fs::read(workspace.root.join(path)).unwrap());
    let cache_before = ["apps/one", "apps/two"].map(|member| {
        let mut entries = fs::read_dir(
            workspace
                .root
                .join(member)
                .join(".oxid/cache/native-object-v1"),
        )
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), fs::read(path).unwrap())
        })
        .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    });
    workspace.write(
        "libs/math/src/lib.ox",
        "pub fn value() -> i32 { return 99; }\n",
    );
    let output = failure(command(&args), "lock is out of date");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("\"kind\":\"build-cache\""));
    failure(
        command(&["doctor", "--native", "--message-format=json"]),
        "lock is out of date",
    );
    for (path, bytes) in binaries.iter().zip(saved) {
        assert_eq!(fs::read(workspace.root.join(path)).unwrap(), bytes);
    }
    for entries in cache_before {
        assert_eq!(
            fs::read_dir(entries[0].0.parent().unwrap())
                .unwrap()
                .count(),
            entries.len()
        );
        for (path, bytes) in entries {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
    }
    assert_eq!(fs::read(workspace.root.join("oxid.lock")).unwrap(), lock);
}
