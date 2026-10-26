//! Public settings/admission contracts and opt-in real LLVM diagnostic checks.
//! The independent seven-build ELF oracle is verify_reproducible_native.py.
#![cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, MutexGuard,
    },
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
static FIXTURE_EXECUTION: Mutex<()> = Mutex::new(());

struct Fixture {
    _execution: MutexGuard<'static, ()>,
    root: PathBuf,
    compiler: PathBuf,
    tools: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        // Keep copied compiler descriptors out of concurrent fixture children.
        let execution = FIXTURE_EXECUTION
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = std::env::temp_dir().join(format!(
            "oxid-reproducible-{}-{}-目錄",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let compiler = root.join("oxid");
        fs::copy(env!("CARGO_BIN_EXE_oxid"), &compiler).unwrap();
        let fixture = Self {
            _execution: execution,
            tools: root.join("tools"),
            root,
            compiler,
        };
        fixture.manifest("");
        fixture.write("app/src/main.ox", "fn main() -> i32 { return 7; }\n");
        fixture.write("app/tests/smoke.ox", "fn main() -> i32 { return 0; }\n");
        fs::create_dir(&fixture.tools).unwrap();
        for name in ["clang", "opt", "ld.lld"] {
            fixture.tool(
                name,
                "#!/bin/sh\nprintf '%s\\n' called >> \"$REPRO_TOOL_MARKER\"\nexit 91\n",
            );
        }
        fixture
    }
    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    fn manifest(&self, setting: &str) {
        self.write("app/oxid.toml", &format!("[project]\nname = \"app\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[tests]\nsmoke = \"tests/smoke.ox\"\n[build]\nentry-mode = \"result\"\n{setting}"));
    }
    fn tool(&self, name: &str, script: &str) {
        let path = self.tools.join(name);
        if fs::symlink_metadata(&path).is_ok() {
            fs::remove_file(&path).unwrap();
        }
        fs::write(&path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn real_tools(&self) {
        let real = std::env::var_os("OXID_LLVM_BIN")
            .expect("set OXID_LLVM_BIN to real LLVM 19.1.7 for ignored native tests");
        for name in ["clang", "opt", "ld.lld"] {
            fs::remove_file(self.tools.join(name)).unwrap();
            symlink(Path::new(&real).join(name), self.tools.join(name)).unwrap();
        }
    }
    fn command(&self, directory: &str, args: &[&str]) -> Command {
        let mut command = Command::new(&self.compiler);
        command
            .current_dir(self.root.join(directory))
            .args(args)
            .env("OXID_LLVM_BIN", &self.tools)
            .env("REPRO_TOOL_MARKER", self.root.join("tools-invoked"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null());
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command("app", args).output().unwrap()
    }
    fn build(&self, flags: &[&str]) -> Vec<u8> {
        let mut args = vec!["build", "--message-format=json"];
        args.extend_from_slice(flags);
        let output = success(self.run(&args));
        assert!(output.stderr.is_empty(), "{output:?}");
        let bytes = fs::read(self.root.join("app/.oxid/bin/app")).unwrap();
        assert_eq!(&bytes[..7], b"\x7fELF\x02\x01\x01");
        bytes
    }
    fn execute(&self) -> Output {
        Command::new(self.root.join("app/.oxid/bin/app"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn no_tools_or_cache(&self) {
        assert!(!self.root.join("tools-invoked").exists());
        for relative in ["app/.oxid/cache", "other/.oxid/cache", ".oxid/cache"] {
            assert!(!self.root.join(relative).exists(), "unexpected {relative}");
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn success(output: Output) -> Output {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    output
}
fn failure(output: Output) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!text.contains("\"kind\":\"build-cache\""), "{text}");
    text
}

#[test]
fn reproducibility_flags_require_a_project_native_build_and_are_unambiguous() {
    let fixture = Fixture::new();
    for flag in ["--reproducible", "--no-reproducible"] {
        for command in ["check", "run", "test", "lock", "fmt", "doctor", "package"] {
            failure(fixture.run(&[command, flag, "--message-format=json"]));
            fixture.no_tools_or_cache();
            assert!(!fixture.root.join("app/oxid.lock").exists());
            assert!(!fixture.root.join("app/.oxid/bin").exists());
        }
        failure(fixture.run(&[
            "compile",
            "src/main.ox",
            "--edition=typed-preview",
            "--backend=llvm",
            "--output=program",
            flag,
        ]));
        assert!(!fixture.root.join("app/program").exists());
    }
    for flags in [
        vec!["--reproducible", "--reproducible"],
        vec!["--no-reproducible", "--no-reproducible"],
        vec!["--reproducible", "--no-reproducible"],
        vec!["--no-reproducible", "--reproducible"],
        vec!["--reproducible=true"],
        vec!["--no-reproducible=false"],
        vec!["--reproducible", "true"],
    ] {
        let mut args = vec!["build", "--message-format=json"];
        args.extend(flags);
        failure(fixture.run(&args));
        fixture.no_tools_or_cache();
    }
}

#[test]
fn strict_boolean_manifest_validation_precedes_all_project_command_effects() {
    let fixture = Fixture::new();
    // Packaging always admits an existing lock before preparing its graph.
    // Supply that prerequisite so every command reaches settings validation.
    success(fixture.run(&["lock"]));
    let lock = fs::read(fixture.root.join("app/oxid.lock")).unwrap();
    for value in [
        "\"true\"",
        "\"false\"",
        "1",
        "0",
        "[]",
        "{}",
        "TRUE",
        "false extra",
    ] {
        fixture.manifest(&format!("reproducible = {value}\n"));
        for command in [
            "check", "run", "test", "lock", "build", "fmt", "doctor", "package",
        ] {
            let text = failure(fixture.run(&[command, "--message-format=json"]));
            assert!(text.contains("reproducible"), "{command}: {text}");
            fixture.no_tools_or_cache();
            assert_eq!(fs::read(fixture.root.join("app/oxid.lock")).unwrap(), lock);
            assert!(!fixture.root.join("app/.oxid").exists());
        }
    }
}

#[test]
fn unsupported_manifest_table_spellings_and_virtual_workspace_settings_are_rejected() {
    let fixture = Fixture::new();
    for spelling in [
        "[build]\nreproducible = true,\n",
        "[build]\nreproducible = true\nreproducible = false\n",
        "[[build]]\nreproducible = true\n",
        "[build.paths]\nreproducible = true\n",
        "build = { reproducible = true }\n",
        "build.reproducible = true\n",
    ] {
        let project = "[project]\nname = \"app\"\nedition = \"typed-preview\"\n";
        let source = if spelling.starts_with('[') {
            format!("{project}{spelling}")
        } else {
            // Root inline/dotted keys must precede [project]; placing them
            // afterwards would describe unrelated project.build metadata.
            format!("{spelling}{project}")
        };
        fixture.write("app/oxid.toml", &source);
        failure(fixture.run(&["check", "--message-format=json"]));
        fixture.no_tools_or_cache();
    }
    fixture.manifest("");
    fixture.write("oxid.toml", "[workspace]\nedition = \"typed-preview\"\nmembers = [\"app\"]\n[build]\nreproducible = true\n");
    failure(
        fixture
            .command("", &["build", "--workspace", "--reproducible"])
            .output()
            .unwrap(),
    );
    fixture.no_tools_or_cache();
    assert!(!fixture.root.join("app/.oxid").exists());
}

#[test]
fn valid_manifest_setting_does_not_change_interpreter_or_declared_test_semantics() {
    let fixture = Fixture::new();
    for value in ["false", "true"] {
        fixture.manifest(&format!("reproducible = {value}\n"));
        success(fixture.run(&["check"]));
        let result = success(fixture.run(&["run", "--message-format=json"]));
        assert!(String::from_utf8_lossy(&result.stdout).contains("\"value\":7"));
        success(fixture.run(&["test"]));
        fixture.no_tools_or_cache();
        assert!(!fixture.root.join("app/.oxid").exists());
    }
}

#[test]
fn build_path_presentation_does_not_change_compile_time_diagnostics() {
    let fixture = Fixture::new();
    fixture.write("app/src/main.ox", "fn main() -> i32 { return false; }\n");
    let ordinary = fixture.run(&["build", "--message-format=json"]);
    let mapped = fixture.run(&["build", "--reproducible", "--message-format=json"]);
    assert_eq!(ordinary.status.code(), Some(1), "{ordinary:?}");
    assert_eq!(mapped.status.code(), Some(1), "{mapped:?}");
    assert_eq!(ordinary.stdout, mapped.stdout);
    assert_eq!(ordinary.stderr, mapped.stderr);
    assert!(String::from_utf8_lossy(&mapped.stdout).contains("E0300"));
    fixture.no_tools_or_cache();
}

#[test]
fn changing_manifest_setting_requires_explicit_lock_update_before_build_effects() {
    let fixture = Fixture::new();
    fixture.manifest("reproducible = false\n");
    success(fixture.run(&["lock"]));
    let lock = fs::read(fixture.root.join("app/oxid.lock")).unwrap();
    fixture.write("app/.oxid/bin/app", "previous successful executable");
    fixture.manifest("reproducible = true\n");
    let text = failure(fixture.run(&[
        "build",
        "--no-reproducible",
        "--incremental",
        "--frozen",
        "--message-format=json",
    ]));
    assert!(text.contains("out of date"), "{text}");
    assert_eq!(fs::read(fixture.root.join("app/oxid.lock")).unwrap(), lock);
    assert_eq!(
        fs::read(fixture.root.join("app/.oxid/bin/app")).unwrap(),
        b"previous successful executable"
    );
    fixture.no_tools_or_cache();
}

#[test]
fn workspace_settings_and_source_preflight_precede_any_selected_member_output() {
    let fixture = Fixture::new();
    fixture.write("oxid.toml", "[workspace]\nedition = \"typed-preview\"\nmembers = [\"app\", \"other\"]\ndefault-members = [\"app\"]\n");
    let good =
        "[project]\nname = \"other\"\nedition = \"typed-preview\"\n[build]\nreproducible = false\n";
    fixture.write("other/oxid.toml", &good.replace("false", "\"false\""));
    fixture.write("other/src/main.ox", "fn main() -> i32 { return 9; }\n");
    for relative in ["app/.oxid/bin/app", "other/.oxid/bin/other"] {
        fixture.write(relative, "previous executable");
    }
    let options = [
        "build",
        "--package=app",
        "--reproducible",
        "--incremental",
        "--message-format=json",
    ];
    let invalid = failure(fixture.command("", &options).output().unwrap());
    assert!(invalid.contains("reproducible"), "{invalid}");
    assert!(!invalid.contains("workspace-member"), "{invalid}");
    fixture.write("other/oxid.toml", good);
    fixture.write("other/src/main.ox", "fn main() -> i32 { return false; }\n");
    let invalid = failure(
        fixture
            .command(
                "",
                &[
                    "build",
                    "--workspace",
                    "--reproducible",
                    "--incremental",
                    "--message-format=json",
                ],
            )
            .output()
            .unwrap(),
    );
    assert!(invalid.contains("E0300"), "{invalid}");
    assert!(!invalid.contains("workspace-member"), "{invalid}");
    for relative in ["app/.oxid/bin/app", "other/.oxid/bin/other"] {
        assert_eq!(
            fs::read(fixture.root.join(relative)).unwrap(),
            b"previous executable"
        );
    }
    fixture.no_tools_or_cache();
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN"]
fn native_default_paths_and_cli_overrides_preserve_every_other_runtime_diagnostic_byte() {
    let fixture = Fixture::new();
    fixture.real_tools();
    let source = "fn main() -> i32 { let value: i32 = 2147483647; return value + 1; }\n";
    fixture.write("app/src/main.ox", source);
    let original = fixture.build(&[]);
    let default_failure = fixture.execute();
    assert_eq!(
        default_failure.status.code(),
        Some(1),
        "{default_failure:?}"
    );
    assert!(default_failure.stdout.is_empty());
    let text = String::from_utf8(default_failure.stderr).unwrap();
    let prefix = format!("{}/", fixture.root.join("app").display());
    assert!(text.contains(&prefix), "{text}");
    let column = source.find('+').unwrap() + 1;
    assert_eq!(text, format!("error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> {prefix}src/main.ox:1:{column}\n"));
    let expected = format!(
        "error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> src/main.ox:1:{column}\n"
    )
    .into_bytes();
    assert_eq!(fixture.build(&["--no-reproducible"]), original);
    let reproducible = fixture.build(&["--reproducible"]);
    let mapped_failure = fixture.execute();
    assert_eq!(mapped_failure.status.code(), Some(1));
    assert!(mapped_failure.stdout.is_empty());
    assert_eq!(mapped_failure.stderr, expected);
    assert!(!reproducible
        .windows(prefix.len())
        .any(|bytes| bytes == prefix.as_bytes()));
    fixture.manifest("reproducible = true\n");
    assert_eq!(fixture.build(&[]), reproducible);
    assert_eq!(fixture.build(&["--no-reproducible"]), original);
    fixture.manifest("reproducible = false\n");
    assert_eq!(fixture.build(&[]), original);
    assert_eq!(fixture.build(&["--reproducible"]), reproducible);

    // A dependency's preference must not override the selected app, and a
    // sibling source retains its real relative topology despite equal names.
    fixture.write("app/oxid.toml", "[project]\nname = \"app\"\nedition = \"typed-preview\"\n[build]\nentry-mode = \"result\"\nreproducible = false\n[dependencies]\nmath = \"../共享 μ\"\n");
    fixture.write("共享 μ/oxid.toml", "[project]\nname = \"app\"\nedition = \"typed-preview\"\nkind = \"lib\"\n[build]\nreproducible = true\n");
    let library = "pub fn fail()->i32{return 2147483647+1;}\n";
    fixture.write("共享 μ/src/lib.ox", library);
    fixture.write(
        "app/src/main.ox",
        "fn main()->i32{return crate::math::fail();}\n",
    );
    fixture.build(&[]);
    let result = fixture.execute();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr)
        .contains(fixture.root.join("共享 μ/src/lib.ox").to_str().unwrap()));
    fixture.build(&["--reproducible"]);
    let result = fixture.execute();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert_eq!(result.stderr, format!("error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> ../共享 μ/src/lib.ox:1:{}\n", library.find('+').unwrap() + 1).as_bytes());
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN"]
fn workspace_members_use_their_own_setting_and_share_the_lock_owner_display_root() {
    let fixture = Fixture::new();
    fixture.real_tools();
    fixture.manifest("reproducible = true\n");
    let source = "fn main()->i32{return 2147483647+1;}\n";
    fixture.write("app/src/main.ox", source);
    fixture.write("other/oxid.toml", "[project]\nname = \"other\"\nedition = \"typed-preview\"\n[build]\nentry-mode = \"result\"\nreproducible = false\n");
    fixture.write("other/src/main.ox", source);
    fixture.write(
        "oxid.toml",
        "[workspace]\nedition = \"typed-preview\"\nmembers = [\"app\", \"other\"]\n",
    );
    for (override_flag, relative) in [
        (None, [true, false]),
        (Some("--reproducible"), [true, true]),
        (Some("--no-reproducible"), [false, false]),
    ] {
        let mut args = vec!["build", "--workspace", "--message-format=json"];
        if let Some(flag) = override_flag {
            args.push(flag);
        }
        let compiled = success(fixture.command("", &args).output().unwrap());
        assert!(compiled.stderr.is_empty(), "{compiled:?}");
        for (name, relative) in ["app", "other"].into_iter().zip(relative) {
            let result = Command::new(fixture.root.join(format!("{name}/.oxid/bin/{name}")))
                .output()
                .unwrap();
            assert_eq!(result.status.code(), Some(1), "{result:?}");
            assert!(result.stdout.is_empty());
            let path = if relative {
                format!("{name}/src/main.ox")
            } else {
                fixture
                    .root
                    .join(format!("{name}/src/main.ox"))
                    .display()
                    .to_string()
            };
            assert_eq!(
                result.stderr,
                format!(
                    "error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> {path}:1:{}\n",
                    source.find('+').unwrap() + 1
                )
                .as_bytes()
            );
        }
    }
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN"]
fn scalar_and_owned_result_and_process_failures_keep_codes_origins_and_effect_order() {
    let fixture = Fixture::new();
    fixture.real_tools();
    for (source, code, stage, message, marker) in [
        (
            "fn main()->i32{return 2147483647+1;}\n",
            "E0604",
            "oir-run",
            "checked i32 arithmetic overflow",
            Some("+1"),
        ),
        (
            "fn main()->i32{let values=[7];return values[1];}\n",
            "E0606",
            "oir-owned-run",
            "array index out of bounds",
            Some("values[1]"),
        ),
        (
            "fn main()->i32{while true {}return 0;}\n",
            "E0601",
            "oir-run",
            "execution fuel exhausted",
            None,
        ),
    ] {
        fixture.write("app/src/main.ox", source);
        for mode in ["--entry-mode=result", "--entry-mode=process"] {
            fixture.build(&[mode]);
            let ordinary = fixture.execute();
            fixture.build(&[mode, "--reproducible"]);
            let mapped = fixture.execute();
            assert_eq!(ordinary.status.code(), Some(1), "{ordinary:?}");
            assert_eq!(mapped.status.code(), Some(1), "{mapped:?}");
            assert!(ordinary.stdout.is_empty() && mapped.stdout.is_empty());
            let prefix = format!("{}/", fixture.root.join("app").display());
            let original = String::from_utf8(ordinary.stderr).unwrap();
            let relative = String::from_utf8(mapped.stderr).unwrap();
            assert!(
                original.starts_with(&format!(
                    "error[{code}] ({stage}): {message}\n  --> {prefix}src/main.ox:1:"
                )),
                "{original}"
            );
            assert_eq!(relative, original.replace(&prefix, ""));
            if let Some(marker) = marker {
                let column = source.find(marker).unwrap() + 1;
                assert_eq!(
                    relative,
                    format!("error[{code}] ({stage}): {message}\n  --> src/main.ox:1:{column}\n")
                );
            }
        }
    }
    // An already completed write must remain visible before the first bounds
    // failure. Later arithmetic must not replace that failure's origin.
    let source = "use std::io::write_stdout;use std::io::WriteStatus;fn main()->i32{let values=[65];let written=write_stdout(&values);match written{WriteStatus::Complete=>{},WriteStatus::InvalidInput=>{return 2;},WriteStatus::IoError(n)=>{return 3;},}let bad=values[1];return bad+(2147483647+1);}\n";
    fixture.write("app/src/main.ox", source);
    fixture.build(&["--entry-mode=process", "--reproducible"]);
    let result = fixture.execute();
    let column = source.find("values[1]").unwrap() + 1;
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert_eq!(result.stdout, b"A");
    assert_eq!(result.stderr, format!("error[E0606] (oir-owned-run): array index out of bounds\n  --> src/main.ox:1:{column}\n").as_bytes());
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN"]
fn reproducible_process_mode_preserves_actual_input_and_output_failure_results() {
    use std::os::unix::process::CommandExt;
    let fixture = Fixture::new();
    fixture.real_tools();
    let input = "use std::io::read_stdin;use std::io::ReadStatus;fn main()->i32{let mut values=[0];let result=read_stdin(&mut values);match result{ReadStatus::Full=>{return 1;},ReadStatus::Eof(n)=>{return n;},ReadStatus::IoError=>{return 71;},}}\n";
    let output = "use std::io::write_stdout;use std::io::WriteStatus;fn main()->i32{let values=[65];let result=write_stdout(&values);match result{WriteStatus::Complete=>{return 0;},WriteStatus::InvalidInput=>{return 2;},WriteStatus::IoError(n)=>{return 73;},}}\n";
    for reproducible in [false, true] {
        let mut flags = vec!["--entry-mode=process"];
        if reproducible {
            flags.push("--reproducible");
        }
        fixture.write("app/src/main.ox", input);
        fixture.build(&flags);
        let mut command = Command::new(fixture.root.join("app/.oxid/bin/app"));
        command.stdin(Stdio::null());
        // close is async-signal-safe and affects only this test child.
        unsafe {
            command.pre_exec(|| {
                libc::close(libc::STDIN_FILENO);
                Ok(())
            });
        }
        let result = command.output().unwrap();
        assert_eq!(result.status.code(), Some(71), "{result:?}");
        assert!(result.stdout.is_empty() && result.stderr.is_empty());
        fixture.write("app/src/main.ox", output);
        fixture.build(&flags);
        let full = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let result = Command::new(fixture.root.join("app/.oxid/bin/app"))
            .stdin(Stdio::null())
            .stdout(full)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(73), "{result:?}");
        assert!(result.stdout.is_empty() && result.stderr.is_empty());
    }
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN"]
fn reproducible_failed_link_preserves_previous_executable_and_emits_no_cache_success() {
    let fixture = Fixture::new();
    fixture.real_tools();
    let before = fixture.build(&["--reproducible", "--incremental"]);
    fixture.tool("ld.lld", "#!/bin/sh\nif [ \"$1\" = --version ]; then printf '%s\\n' 'LLD 19.1.7'; exit 0; fi\nprintf '%s\\n' 'intentional reproducible link failure' >&2\nexit 9\n");
    let error = failure(fixture.run(&[
        "build",
        "--reproducible",
        "--incremental",
        "--message-format=json",
    ]));
    assert!(
        error.contains("intentional reproducible link failure"),
        "{error}"
    );
    assert_eq!(
        fs::read(fixture.root.join("app/.oxid/bin/app")).unwrap(),
        before
    );
    let result = fixture.execute();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert_eq!(result.stdout, b"7\n");
    assert!(result.stderr.is_empty());
}
