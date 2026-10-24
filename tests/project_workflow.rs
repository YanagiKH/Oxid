//! Manifest-driven typed projects use the public compiler routes end to end.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const PROCESS_HOST: bool = cfg!(all(target_os = "linux", target_arch = "x86_64"));

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-workflow-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, source: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    fn typed(&self, extra: &str) {
        self.write(
            "oxid.toml",
            &format!(
                "[project]\nname = \"workflow\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n{extra}"
            ),
        );
        self.write("src/main.ox", "fn main() -> i32 { return 0; }\n");
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .args(args)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null());
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    fn no_outputs(&self) {
        assert!(!self.0.join(".oxid").exists());
        assert!(!self.0.join("cache").exists());
        assert!(!self.0.join("oxid.lock").exists());
        assert!(!self.0.join("sentinel").exists());
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(output: Output) -> Output {
    assert!(output.status.success(), "{output:?}");
    output
}
fn failure(output: Output, message: &str) -> Output {
    assert!(!output.status.success(), "{output:?}");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains(message), "{combined:?}");
    output
}

#[test]
fn incremental_is_a_boolean_build_option_and_rejects_ambiguous_arguments_before_effects() {
    let project = Project::new();
    project.typed("");
    for args in [
        vec!["build", "--incremental", "--incremental"],
        vec!["build", "--incremental=true"],
        vec!["build", "--incremental", "false"],
        vec!["check", "--incremental"],
    ] {
        let result = project.run(&args);
        assert!(!result.status.success(), "{args:?}: {result:?}");
        project.no_outputs();
    }
}

#[test]
fn typed_new_records_edition_and_runs_without_repeated_flags() {
    let project = Project::new();
    success(project.run(&["new", "app", "--edition=typed-preview"]));
    let manifest = fs::read_to_string(project.0.join("app/oxid.toml")).unwrap();
    assert!(manifest.contains("edition = \"typed-preview\""));
    assert!(manifest.contains("[tests]"));
    let mut check = project.command(&["check"]);
    success(
        check
            .current_dir(project.0.join("app/src"))
            .output()
            .unwrap(),
    );
    if PROCESS_HOST {
        for args in [&["run"][..], &["test"][..]] {
            let output = project
                .command(args)
                .current_dir(project.0.join("app"))
                .output()
                .unwrap();
            success(output);
        }
    }
}

#[test]
fn implicit_file_selection_uses_source_manifest_outside_current_directory() {
    let project = Project::new();
    project.typed("");
    let elsewhere = Project::new();
    let source = project.0.join("src/main.ox");
    let output =
        success(elsewhere.run(&["check", source.to_str().unwrap(), "--message-format=json"]));
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"kind\":\"check-summary\""));
    project.no_outputs();
    elsewhere.no_outputs();
}

#[test]
fn explicit_file_edition_preserves_old_result_mode_and_overrides_manifest() {
    let project = Project::new();
    project.typed("");
    project.write("src/main.ox", "fn main() -> i32 { return 7; }\n");
    let output = success(project.run(&[
        "run",
        "src/main.ox",
        "--edition=typed-preview",
        "--message-format=json",
    ]));
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"value\":7"));
    project.write("legacy.ox", "print 5 / 2;\n");
    let output = success(project.run(&["run", "legacy.ox", "--edition=legacy-0.9"]));
    assert_eq!(output.stdout, b"2.5\n");
}

#[test]
fn inherited_process_mode_preserves_exit_status_and_stdout() {
    if !PROCESS_HOST {
        return;
    }
    let project = Project::new();
    project.typed("");
    project.write("src/main.ox", "fn main() -> i32 { return 7; }\n");
    let output = project.run(&["run"]);
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let output = failure(project.run(&["run", "--message-format=json"]), "process");
    assert!(output.stdout.is_empty());
    project.no_outputs();
}

#[test]
fn tests_run_only_declared_roots_and_nonzero_status_fails() {
    if !PROCESS_HOST {
        return;
    }
    let project = Project::new();
    project.typed("[tests]\nsmoke = \"tests/smoke.ox\"\n");
    project.write("tests/smoke.ox", "fn main() -> i32 { return 0; }\n");
    project.write("tests/helper.ox", "this must not be parsed as a test root");
    success(project.run(&["test"]));
    let json = success(project.run(&["test", "--message-format=json"]));
    assert!(json.stderr.is_empty(), "{json:?}");
    let records = String::from_utf8(json.stdout).unwrap();
    assert!(records.contains("\"kind\":\"test-summary\""));
    assert!(records.contains("\"passed\":1"));
    project.write("tests/smoke.ox", "fn main() -> i32 { return 9; }\n");
    let failed = failure(project.run(&["test"]), "9");
    assert_eq!(failed.status.code(), Some(1));
    project.no_outputs();
}

#[test]
fn unknown_editions_and_unsupported_configuration_never_fall_back() {
    let cases = [
        ("[project]\nedition = \"typo\"\n", "edition"),
        (
            "[project]\nedition = \"typed-preview\"\n[build]\nbackend = \"rust\"\n",
            "backend",
        ),
        (
            "[project]\nedition = \"typed-preview\"\n[build]\nprofile = \"unknown\"\n",
            "profile",
        ),
        (
            "[project]\nedition = \"typed-preview\"\n[build]\ntarget = \"aarch64-unknown-linux-gnu\"\n",
            "target",
        ),
        (
            "[project]\nedition = \"typed-preview\"\n[dependencies]\nremote = \"https://invalid.example/repo.git\"\n",
            "dependencies",
        ),
    ];
    for (manifest, expected) in cases {
        let project = Project::new();
        project.write("oxid.toml", manifest);
        project.write("src/main.ox", "write_text(\"sentinel\", \"executed\");\n");
        failure(project.run(&["build"]), expected);
        project.no_outputs();
    }
}

#[test]
fn project_json_diagnostics_do_not_mix_text_and_json() {
    let project = Project::new();
    project.typed("[dependencies]\nbad = \"./absent\"\n");
    let output = failure(
        project.run(&["build", "--message-format=json"]),
        "dependencies",
    );
    assert!(output.stderr.is_empty());
    let output = String::from_utf8(output.stdout).unwrap();
    assert!(output
        .lines()
        .all(|line| line.starts_with('{') && line.ends_with('}')));
    assert!(output.contains("\"success\":false"));
    project.no_outputs();
}

#[test]
fn manifest_selection_preserves_legacy_metadata_and_rejects_broken_typed_configuration() {
    let project = Project::new();
    project.write("legacy.ox", "print 7;\n");
    project.write("oxid.toml", "[unrelated]\nmetadata = \"unterminated\n");
    assert_eq!(success(project.run(&["run", "legacy.ox"])).stdout, b"7\n");
    let project = Project::new();
    project.write("legacy.ox", "print 7;\n");
    project.write(
        "oxid.toml",
        "[project]\nedition = \"typed-preview\"\n[unrelated]\nmetadata = \"unterminated\n",
    );
    failure(project.run(&["run", "legacy.ox"]), "manifest");
    project.write(
        "oxid.toml",
        "[project]\nedition = \"typed-preview\"\nedition = \"legacy-0.9\"\n",
    );
    failure(project.run(&["run", "legacy.ox"]), "duplicate edition");
    project.no_outputs();
}

#[test]
fn edition_selection_ignores_multiline_data_and_does_not_hide_invalid_typed_utf8() {
    let legacy = Project::new();
    legacy.write("legacy.ox", "print 7;\n");
    legacy.write(
        "oxid.toml",
        "[metadata]\nexample = '''\n[project]\nedition = \"typed-preview\"\n'''\n",
    );
    assert_eq!(success(legacy.run(&["run", "legacy.ox"])).stdout, b"7\n");

    let typed = Project::new();
    typed.write("src/main.ox", "write_text(\"sentinel\", \"executed\");\n");
    fs::write(
        typed.0.join("oxid.toml"),
        b"[project]\nedition = \"typed-preview\"\n[metadata]\ncomment = \"broken\xff\"\n",
    )
    .unwrap();
    failure(typed.run(&["run", "src/main.ox"]), "manifest");
    typed.no_outputs();
}

#[test]
fn standard_quoted_and_dotted_editions_select_the_typed_compiler() {
    for selection in [
        "[\"project\"]\nedition = \"typed-preview\"\n",
        "['project']\nedition = \"typed-preview\"\n",
        "project.edition = \"typed-preview\"\n",
        "\"project\".\"edition\" = \"typed-preview\"\n",
        "project.\"edi\\u0074ion\" = \"typed-preview\"\n",
        "[project]\n\"edi\\u0074ion\" = \"typed-preview\"\n",
        "[metadata]\n\"not=edition\" = \"example\"\n[project]\nedition = \"typed-preview\"\n",
    ] {
        let project = Project::new();
        project.write("oxid.toml", selection);
        project.write("main.ox", "fn main() -> i32 { return 0; }\n");
        success(project.run(&["check", "main.ox"]));
        project.write("main.ox", "write_text(\"sentinel\", \"executed\");\n");
        assert!(
            !project.run(&["run", "main.ox"]).status.success(),
            "{selection:?}"
        );
        project.no_outputs();
    }
}

#[test]
fn unsupported_inline_and_array_project_tables_never_select_legacy() {
    for manifest in [
        "project = { edition = \"typed-preview\", entry = \"main.ox\" }\n",
        "[[project]]\nedition = \"typed-preview\"\nentry = \"main.ox\"\n",
    ] {
        let project = Project::new();
        project.write("oxid.toml", manifest);
        project.write("main.ox", "write_text(\"sentinel\", \"executed\");\n");
        assert!(!project.run(&["run", "main.ox"]).status.success());
        project.no_outputs();
    }
}

#[test]
fn legacy_entry_override_remains_last_wins_and_typed_duplicates_are_rejected() {
    let legacy = Project::new();
    legacy.write(
        "oxid.toml",
        "[project]\nname = \"legacy\"\nentry = \"missing.ox\"\n[build]\nentry = \"main.ox\"\n",
    );
    legacy.write("main.ox", "print 5 / 2;\n");
    success(legacy.run(&["build"]));
    assert_eq!(
        success(legacy.run(&["run", ".oxid/bin/legacy.oxb"])).stdout,
        b"2.5\n"
    );
    for duplicate in [
        "[build]\nentry = \"other.ox\"\n",
        "[build]\nbackend = \"llvm\"\nbackend = \"llvm\"\n",
        "[tests]\nsmoke = \"tests/a.ox\"\nsmoke = \"tests/b.ox\"\n",
    ] {
        let typed = Project::new();
        typed.typed(duplicate);
        failure(typed.run(&["check"]), "duplicate");
        typed.no_outputs();
    }
}

#[test]
#[cfg(target_os = "linux")]
fn project_process_errors_report_io_failure_without_panicking_or_stdout_fallback() {
    for configuration in [
        "[build]\nbackend = \"unavailable\"\n",
        "[metadata]\nvalue = \"unterminated\n",
    ] {
        let project = Project::new();
        project.typed(configuration);
        for args in [&["run"][..], &["run", "--message-format=json"][..]] {
            let output = project
                .command(args)
                .stderr(
                    fs::OpenOptions::new()
                        .write(true)
                        .open("/dev/full")
                        .unwrap(),
                )
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(74), "{output:?}");
            assert!(output.stdout.is_empty());
        }
        project.no_outputs();
    }
}

#[test]
fn tool_metadata_remains_available_in_a_typed_project() {
    let project = Project::new();
    project.typed("");
    for command in ["help", "--help", "-h", "--version", "-V"] {
        success(project.run(&[command]));
    }
    project.no_outputs();
}

#[test]
fn invalid_options_do_not_create_projects_or_build_artifacts() {
    for args in [
        vec!["new", "app", "--edition=typed-preview", "--unknown"],
        vec![
            "new",
            "app",
            "--edition=typed-preview",
            "--edition=typed-preview",
        ],
        vec!["new", "app", "--edition=typed-preview", "--profile=release"],
    ] {
        let project = Project::new();
        assert!(!project.run(&args).status.success());
        assert!(!project.0.join("app").exists());
    }
    let project = Project::new();
    project.typed("");
    failure(
        project.run(&["build", "--experimental-hir-import=absent"]),
        "unsupported",
    );
    let duplicate = project.run(&["build", "--entry-mode=process", "--entry-mode=result"]);
    if PROCESS_HOST || cfg!(all(target_os = "macos", target_pointer_width = "64")) {
        assert_eq!(duplicate.status.code(), Some(1));
        failure(duplicate, "once");
    } else {
        assert_eq!(duplicate.status.code(), Some(74));
        assert!(duplicate.stdout.is_empty() && duplicate.stderr.is_empty());
    }
    project.no_outputs();
}

#[test]
fn unsupported_commands_and_directory_formatting_do_not_reach_legacy() {
    let project = Project::new();
    project.typed("");
    project.write("legacy.ox", "write_text(\"sentinel\", \"executed\");\n");
    let original = fs::read(project.0.join("legacy.ox")).unwrap();
    for args in [
        &["legacy.ox"][..],
        &["ast", "legacy.ox"][..],
        &["fmt", "."][..],
    ] {
        assert!(!project.run(args).status.success(), "{args:?}");
    }
    assert_eq!(fs::read(project.0.join("legacy.ox")).unwrap(), original);
    project.no_outputs();
}

#[test]
fn invalid_source_or_tool_failure_preserves_previous_managed_executable() {
    let project = Project::new();
    project.typed("");
    project.write(".oxid/bin/workflow", "previous successful executable");
    project.write("src/main.ox", "fn main() -> i32 { return true; }\n");
    failure(project.run(&["build"]), "E0300");
    assert_eq!(
        fs::read(project.0.join(".oxid/bin/workflow")).unwrap(),
        b"previous successful executable"
    );
    project.write("src/main.ox", "fn main() -> i32 { return 0; }\n");
    let output = project
        .command(&["build"])
        .env("OXID_LLVM_BIN", project.0.join("missing"))
        .output()
        .unwrap();
    failure(output, if PROCESS_HOST { "E0701" } else { "E0608" });
    assert_eq!(
        fs::read(project.0.join(".oxid/bin/workflow")).unwrap(),
        b"previous successful executable"
    );
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn managed_build_rejects_symlinks_and_directory_outputs_before_tool_invocation() {
    use std::os::unix::fs::symlink;

    for boundary in [".oxid", ".oxid/bin", ".oxid/bin/workflow", "directory"] {
        let project = Project::new();
        project.typed("");
        let external = Project::new();
        external.write("bin/workflow", "protected external executable");
        match boundary {
            ".oxid" => symlink(&external.0, project.0.join(".oxid")).unwrap(),
            ".oxid/bin" => {
                fs::create_dir(project.0.join(".oxid")).unwrap();
                symlink(external.0.join("bin"), project.0.join(".oxid/bin")).unwrap();
            }
            ".oxid/bin/workflow" => {
                fs::create_dir_all(project.0.join(".oxid/bin")).unwrap();
                symlink(external.0.join("bin/workflow"), project.0.join(boundary)).unwrap();
            }
            _ => project.write(".oxid/bin/workflow/keep", "protected directory"),
        }
        let output = project
            .command(&["build"])
            .env("OXID_LLVM_BIN", project.0.join("missing-tools"))
            .output()
            .unwrap();
        failure(output, "managed");
        assert_eq!(
            fs::read(external.0.join("bin/workflow")).unwrap(),
            b"protected external executable"
        );
        if boundary == "directory" {
            assert_eq!(
                fs::read(project.0.join(".oxid/bin/workflow/keep")).unwrap(),
                b"protected directory"
            );
        } else {
            assert!(fs::symlink_metadata(project.0.join(boundary))
                .unwrap()
                .file_type()
                .is_symlink());
        }
    }
}

#[test]
fn typed_formatter_inherits_manifest_and_preserves_stdout_contract() {
    let project = Project::new();
    project.typed("");
    project.write("src/main.ox", "fn main()->i32{return 0;}\n");
    let before = fs::read(project.0.join("src/main.ox")).unwrap();
    let output = success(project.run(&["fmt", "src/main.ox"]));
    assert!(!output.stdout.is_empty());
    assert_ne!(output.stdout, before);
    assert_eq!(fs::read(project.0.join("src/main.ox")).unwrap(), before);
    project.no_outputs();
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[ignore = "requires Linux x86_64 and pinned LLVM 19.1.7"]
fn native_build_replaces_managed_output_after_module_edit_and_preserves_compile_contract() {
    let project = Project::new();
    project.typed("");
    project.write(
        "src/main.ox",
        "mod value;\nuse crate::value::status;\nfn main() -> i32 { return status(); }\n",
    );
    project.write("src/value.ox", "pub fn status() -> i32 { return 0; }\n");
    success(project.run(&["build"]));
    let binary = project.0.join(".oxid/bin/workflow");
    assert_eq!(Command::new(&binary).status().unwrap().code(), Some(0));
    project.write("src/value.ox", "pub fn status() -> i32 { return 7; }\n");
    success(project.run(&["build"]));
    assert_eq!(Command::new(&binary).status().unwrap().code(), Some(7));
    let previous = fs::read(&binary).unwrap();
    failure(
        project.run(&[
            "compile",
            "src/main.ox",
            "--edition=typed-preview",
            "--backend=llvm",
            "--output",
            binary.to_str().unwrap(),
        ]),
        "already exists",
    );
    assert_eq!(fs::read(&binary).unwrap(), previous);
    project.write("src/value.ox", "pub fn status() -> i32 { return false; }\n");
    failure(project.run(&["build"]), "E0300");
    assert_eq!(fs::read(&binary).unwrap(), previous);
    assert_eq!(Command::new(&binary).status().unwrap().code(), Some(7));
    assert!(!project.0.join(".oxid/bin/workflow.oxb").exists());
}
