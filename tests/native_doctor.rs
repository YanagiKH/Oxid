use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-native-doctor-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("scratch")).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .env("TMPDIR", self.0.join("scratch"))
            .env("OXID_LLVM_BIN", self.0.join("missing-tools"));
        #[cfg(windows)]
        command
            .env("TMP", self.0.join("scratch"))
            .env("TEMP", self.0.join("scratch"));
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn clean(&self) {
        assert_eq!(
            fs::read_dir(self.0.join("scratch")).unwrap().count(),
            0,
            "probe storage leaked"
        );
        assert!(
            !self.0.join(".oxid").exists(),
            "doctor published project output"
        );
        assert!(
            !self.0.join("oxid.lock").exists(),
            "doctor created a lockfile"
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn text(output: &Output) -> String {
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout.clone()).unwrap()
}
fn status(output: &str, id: &str, expected: &str) {
    assert!(
        output.contains(&format!("\"id\":\"{id}\",\"status\":\"{expected}\"")),
        "{output}"
    );
}
const LINKER: &str = if cfg!(target_os = "macos") {
    "ld64.lld"
} else {
    "ld.lld"
};
const SUPPORTED: bool = cfg!(all(
    target_pointer_width = "64",
    target_endian = "little",
    any(
        all(
            any(target_os = "linux", target_os = "macos"),
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(
            target_os = "windows",
            target_env = "msvc",
            target_arch = "x86_64"
        )
    )
));

#[test]
fn native_doctor_reports_real_tool_selection_and_unsupported_hosts_honestly() {
    let fixture = Fixture::new();
    let output = fixture.run(&["doctor", "--native", "--message-format=json"]);
    assert_eq!(output.status.code(), Some(1));
    let output = text(&output);
    assert!(
        output.starts_with("{\"schema_version\":1,\"kind\":\"native-doctor\""),
        "{output}"
    );
    assert_eq!(output.lines().count(), 1);
    let triple = if cfg!(all(
        target_os = "windows",
        target_env = "msvc",
        target_arch = "x86_64"
    )) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    assert!(output.contains(&format!("\"target\":\"{triple}\"")));
    assert!(output.contains("\"llvm_version\":\"19.1.7\""));
    assert!(output.contains("\"toolchain_ready\":false,\"success\":false"));
    status(&output, "host", if SUPPORTED { "passed" } else { "failed" });
    status(
        &output,
        "clang",
        if SUPPORTED { "failed" } else { "skipped" },
    );
    status(&output, "execute", "skipped");
    status(&output, "project", "not-applicable");
    fixture.clean();
}

#[test]
fn native_doctor_validates_all_options_before_probes_or_project_reads() {
    let fixture = Fixture::new();
    fixture.write("oxid.toml", "malformed manifest");
    for args in [
        vec!["doctor", "--native", "--native"],
        vec!["doctor", "--native=yes"],
        vec!["doctor", "--native", "--edition=legacy-0.9"],
        vec!["doctor", "--native", "--offline"],
        vec!["doctor", "--native", "--target=aarch64-unknown-linux-gnu"],
        vec!["doctor", "--native", "source.ox"],
        vec!["doctor", "--native", "--edition"],
    ] {
        let mut command = fixture.command();
        let output = command
            .args(&args)
            .arg("--message-format=json")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let output = text(&output);
        status(&output, "options", "failed");
        assert!(!output.contains("\"id\":\"host\""));
        assert!(!output.contains("manifest"));
        fixture.clean();
    }
}

#[test]
fn native_doctor_alias_and_human_output_do_not_replace_legacy_doctor() {
    let fixture = Fixture::new();
    fixture.write("oxid.toml", "[project]\nname=\"old\"\nentry=\"main.ox\"\n");
    fixture.write("main.ox", "print(42);\n");
    let old = fixture.run(&["doctor"]);
    assert!(old.status.success(), "{old:?}");
    assert!(text(&old).contains("manifest: ok"));
    let separated = fixture.run(&["doctor", "--", "--native"]);
    assert!(separated.status.success(), "{separated:?}");
    assert!(text(&separated).contains("manifest: ok"));
    let output = fixture.run(&[
        "diagnose",
        "--native",
        "--edition",
        "typed-preview",
        "--message-format",
        "text",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let output = text(&output);
    assert!(output.starts_with("Oxid native doctor (LLVM 19.1.7"));
    assert!(output.contains("project: not-applicable"));
    assert!(output.contains("toolchain-ready: false"));
    fixture.clean();
}

#[test]
fn native_doctor_accepts_global_option_placements() {
    let fixture = Fixture::new();
    for args in [
        &[
            "--edition=typed-preview",
            "doctor",
            "--native",
            "--message-format=json",
        ][..],
        &[
            "--message-format",
            "json",
            "--edition",
            "typed-preview",
            "diagnose",
            "--native",
        ],
        &["--native", "doctor", "--message-format=json"],
    ] {
        let output = text(&fixture.run(args));
        assert!(
            output.contains("\"kind\":\"native-doctor\""),
            "{args:?}: {output}"
        );
        assert!(!output.contains("\"id\":\"options\""), "{args:?}: {output}");
        status(&output, "host", if SUPPORTED { "passed" } else { "failed" });
        fixture.clean();
    }
}

#[test]
fn native_doctor_reads_package_sources_without_executing_or_typechecking_them() {
    let fixture = Fixture::new();
    fixture.write(
        "oxid.toml",
        "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n",
    );
    fixture.write("src/main.ox", "fn main()->i32{return missing();}");
    let output = fixture.run(&["doctor", "--native", "--message-format=json"]);
    let output = text(&output);
    status(&output, "project", "passed");
    assert!(output.contains("1 local package(s) admitted"));
    assert!(output.contains("oxid.lock absent (optional)"));
    fixture.write("src/main.ox", "fn broken{");
    let output = text(&fixture.run(&["doctor", "--native", "--message-format=json"]));
    status(&output, "project", "failed");
    fixture.clean();
}

#[test]
fn invalid_nearest_manifest_is_not_hidden_as_an_absent_project() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join("oxid.toml")).unwrap();
    let output = text(&fixture.run(&["doctor", "--native", "--message-format=json"]));
    status(&output, "project", "failed");
    assert!(!output.contains("\"id\":\"project\",\"status\":\"not-applicable\""));
    fixture.clean();
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn native_doctor_escapes_tool_paths_in_both_output_formats() {
    let fixture = Fixture::new();
    let tools = fixture.0.join("absent\n\u{1b}[2J\"\\tools");
    let output = fixture
        .command()
        .env("OXID_LLVM_BIN", &tools)
        .args(["doctor", "--native", "--message-format=json"])
        .output()
        .unwrap();
    let output = text(&output);
    assert_eq!(output.lines().count(), 1);
    assert!(!output.contains('\u{1b}'));
    assert!(output.contains("\\n\\u001b[2J\\\"\\\\tools"), "{output}");
    let output = text(
        &fixture
            .command()
            .env("OXID_LLVM_BIN", &tools)
            .args(["doctor", "--native"])
            .output()
            .unwrap(),
    );
    assert!(!output.contains('\u{1b}'));
    assert!(output.contains("absent\\n\\u{1b}[2J"), "{output}");
    fixture.clean();
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn fake_tools(fixture: &Fixture, clang_mode: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let tools = fixture.0.join("tools");
    fs::create_dir(&tools).unwrap();
    for (name, marker) in [
        ("clang", "clang version"),
        ("opt", "LLVM version"),
        (LINKER, "LLD"),
    ] {
        let action = if name == "clang" { clang_mode } else { "" };
        let script = format!("#!/bin/sh\nif [ \"$1\" = --version ]; then\n{action}\nprintf '%s\\n' '{marker} 19.1.7'\nexit 0\nfi\nprintf 'SDK unavailable\\n' >&2\nexit 9\n");
        for installed in [name.to_owned(), format!("{name}-19")] {
            let path = tools.join(installed);
            fs::write(&path, &script).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    tools
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn version_only_tools_cannot_claim_sdk_or_native_readiness() {
    let fixture = Fixture::new();
    fake_tools(&fixture, "");
    for path_fallback in [false, true] {
        let mut command = fixture.command();
        command.args(["doctor", "--native", "--message-format=json"]);
        if path_fallback {
            command
                .env_remove("OXID_LLVM_BIN")
                .env("PATH", fixture.0.join("tools"));
        } else {
            command.env("OXID_LLVM_BIN", "tools");
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        let output = text(&output);
        for id in ["clang", "opt", "linker"] {
            status(&output, id, "passed");
        }
        status(&output, "sdk", "failed");
        assert!(output.contains("SDK unavailable"));
        status(&output, "compile-link", "skipped");
        assert!(output.contains(if path_fallback {
            "/tools/clang-19"
        } else {
            "/tools/clang"
        }));
        fixture.clean();
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn wrong_versions_output_floods_and_hung_tools_are_bounded() {
    for (script, expected) in [
        ("printf 'clang version 19.1.70\\n'; exit 0", "must report clang version 19.1.7"),
        ("while :; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done", "exceeds 65536 bytes"),
        ("/bin/sleep 30 & wait", "timed out after 5 seconds"),
    ] {
        let fixture = Fixture::new();
        let tools = fake_tools(&fixture, script);
        let start = std::time::Instant::now();
        let output = fixture.command().env("OXID_LLVM_BIN", tools).args(["doctor", "--native", "--message-format=json"]).output().unwrap();
        assert!(start.elapsed().as_secs() < 15, "doctor did not enforce its timeout");
        assert_eq!(output.status.code(), Some(1));
        let output = text(&output);
        assert!(output.len() < 12_000);
        status(&output, "clang", "failed");
        assert!(output.contains(expected), "{output}");
        status(&output, "sdk", "skipped");
        fixture.clean();
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn exited_probe_leader_does_not_leave_a_child_holding_capture_pipes() {
    let fixture = Fixture::new();
    let tools = fake_tools(&fixture, "/bin/sleep 30 &\nprintf '%s' \"$!\" > \"$DOCTOR_DESCENDANT_PID\"\nprintf 'clang version 19.1.7\\n'\nexit 0");
    let child_pid_file = fixture.0.join("child-pid");
    let start = std::time::Instant::now();
    let output = fixture
        .command()
        .env("OXID_LLVM_BIN", tools)
        .env("DOCTOR_DESCENDANT_PID", &child_pid_file)
        .args(["doctor", "--native", "--message-format=json"])
        .output()
        .unwrap();
    assert!(start.elapsed().as_secs() < 15);
    let output = text(&output);
    status(&output, "clang", "failed");
    assert!(output.contains("timed out after 5 seconds"));
    let pid: u32 = fs::read_to_string(child_pid_file).unwrap().parse().unwrap();
    // A container's PID 1 may retain an orphan zombie. It must not be running
    // after the doctor's group cleanup, regardless of that external reaper.
    let stopped = || {
        fs::read_to_string(format!("/proc/{pid}/stat")).map_or(true, |value| {
            value
                .rsplit_once(") ")
                .is_some_and(|(_, state)| state.starts_with('Z'))
        })
    };
    for _ in 0..100 {
        if stopped() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(stopped(), "probe descendant survived group cleanup");
    fixture.clean();
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn inherited_auto_reaping_is_rejected_before_probe_processes() {
    use std::os::unix::process::CommandExt;
    let fixture = Fixture::new();
    let tools = fake_tools(&fixture, "");
    let mut command = fixture.command();
    unsafe {
        command.pre_exec(|| {
            if libc::signal(libc::SIGCHLD, libc::SIG_IGN) == libc::SIG_ERR {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = command
        .env("OXID_LLVM_BIN", tools)
        .args(["doctor", "--native", "--message-format=json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let output = text(&output);
    status(&output, "clang", "failed");
    assert!(output.contains("default SIGCHLD handling and waitable children"));
    status(&output, "sdk", "skipped");
    fixture.clean();
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn malformed_locks_and_changed_dependency_inputs_fail_package_diagnosis_without_rewrites() {
    let fixture = Fixture::new();
    fixture.write(
        "oxid.toml",
        "[project]\nname=\"app\"\nedition=\"typed-preview\"\n[dependencies]\nlib=\"./lib\"\n",
    );
    fixture.write("src/main.ox", "fn main()->i32{return crate::lib::value();}");
    fixture.write(
        "lib/oxid.toml",
        "[project]\nname=\"lib\"\nedition=\"typed-preview\"\n",
    );
    fixture.write("lib/src/lib.ox", "pub fn value()->i32{return 42;}");
    let locked = fixture.run(&["lock"]);
    assert!(locked.status.success(), "{locked:?}");
    let original = fs::read(fixture.0.join("oxid.lock")).unwrap();
    let output = text(&fixture.run(&["doctor", "--native", "--message-format=json"]));
    status(&output, "project", "passed");
    assert!(output.contains("oxid.lock verified against retained inputs"));
    fixture.write("lib/src/lib.ox", "pub fn value()->i32{return 99;}");
    let output = text(&fixture.run(&["doctor", "--native", "--message-format=json"]));
    status(&output, "project", "failed");
    assert!(output.contains("lock"));
    assert_eq!(fs::read(fixture.0.join("oxid.lock")).unwrap(), original);
    fixture.write("oxid.lock", "broken lock\n");
    let output = text(&fixture.run(&["doctor", "--native", "--message-format=json"]));
    status(&output, "project", "failed");
    assert_eq!(
        fs::read_to_string(fixture.0.join("oxid.lock")).unwrap(),
        "broken lock\n"
    );
    assert_eq!(fs::read_dir(fixture.0.join("scratch")).unwrap().count(), 0);
    assert!(!fixture.0.join(".oxid").exists());
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 tools"]
fn real_native_doctor_compiles_links_executes_and_preserves_the_project() {
    use std::path::Path;
    let fixture = Fixture::new();
    let tools = std::env::var_os("OXID_LLVM_BIN").expect("set OXID_LLVM_BIN to LLVM 19.1.7");
    let output = fixture
        .command()
        .env("OXID_LLVM_BIN", &tools)
        .args(["doctor", "--native", "--message-format=json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let output = text(&output);
    for id in [
        "host",
        "temporary-directory",
        "clang",
        "opt",
        "linker",
        "sdk",
        "compile-link",
        "execute",
    ] {
        status(&output, id, "passed");
    }
    assert!(output.contains("\"toolchain_ready\":true,\"success\":true"));
    let format = if cfg!(target_os = "macos") {
        "Mach-O64"
    } else {
        "ELF64"
    };
    assert!(output.contains(&format!("{format} {} PIE", std::env::consts::ARCH)));
    fixture.clean();
    // A real compiler with its SDK include search deliberately disabled must
    // fail the SDK check even though all three version checks still succeed.
    use std::os::unix::fs::PermissionsExt;
    let wrapper = fixture.0.join("without-sdk");
    fs::create_dir(&wrapper).unwrap();
    let clang = Path::new(&tools).join("clang");
    let quoted = clang.to_string_lossy().replace('\'', "'\\''");
    fs::write(wrapper.join("clang"), format!("#!/bin/sh\nif [ \"$1\" = --version ]; then exec '{quoted}' \"$@\"; fi\nexec '{quoted}' -nostdinc \"$@\"\n")).unwrap();
    fs::set_permissions(wrapper.join("clang"), fs::Permissions::from_mode(0o700)).unwrap();
    for tool in ["opt", LINKER] {
        std::os::unix::fs::symlink(Path::new(&tools).join(tool), wrapper.join(tool)).unwrap();
    }
    let output = fixture
        .command()
        .env("OXID_LLVM_BIN", wrapper)
        .args(["doctor", "--native", "--message-format=json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let output = text(&output);
    status(&output, "clang", "passed");
    status(&output, "sdk", "failed");
    status(&output, "compile-link", "skipped");
    fixture.clean();
    // Real headers and code generation still do not prove the target libraries
    // can link. Remove only the standard libraries at the actual link step.
    let missing_libraries = fixture.0.join("without-libraries");
    fs::create_dir(&missing_libraries).unwrap();
    fs::write(missing_libraries.join("clang"), format!("#!/bin/sh\nfor arg do\ncase \"$arg\" in -pie|-Wl,-pie) exec '{quoted}' -nostdlib \"$@\";; esac\ndone\nexec '{quoted}' \"$@\"\n")).unwrap();
    fs::set_permissions(
        missing_libraries.join("clang"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    for tool in ["opt", LINKER] {
        std::os::unix::fs::symlink(Path::new(&tools).join(tool), missing_libraries.join(tool))
            .unwrap();
    }
    let output = fixture
        .command()
        .env("OXID_LLVM_BIN", missing_libraries)
        .args(["doctor", "--native", "--message-format=json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let output = text(&output);
    status(&output, "sdk", "passed");
    status(&output, "compile-link", "failed");
    assert!(output.contains("undefined symbol"), "{output}");
    status(&output, "execute", "skipped");
    fixture.clean();
    #[cfg(target_os = "linux")]
    {
        // A tool flooding inside the real compiler child must be contained even
        // though the existing native build boundary buffers that tool's output.
        // This is a constant-output test tool, never a user's program.
        let nested_flood = fixture.0.join("nested-flood");
        fs::create_dir(&nested_flood).unwrap();
        for tool in ["clang", LINKER] {
            std::os::unix::fs::symlink(Path::new(&tools).join(tool), nested_flood.join(tool))
                .unwrap();
        }
        let opt = Path::new(&tools)
            .join("opt")
            .to_string_lossy()
            .replace('\'', "'\\''");
        fs::write(nested_flood.join("opt"), format!("#!/bin/sh\nif [ \"$1\" = --version ]; then exec '{opt}' \"$@\"; fi\nexec /usr/bin/yes nested-flood\n")).unwrap();
        fs::set_permissions(nested_flood.join("opt"), fs::Permissions::from_mode(0o700)).unwrap();
        let start = std::time::Instant::now();
        let output = fixture
            .command()
            .env("OXID_LLVM_BIN", nested_flood)
            .args(["doctor", "--native", "--message-format=json"])
            .output()
            .unwrap();
        assert!(
            start.elapsed().as_secs() < 20,
            "nested tool did not hit the memory bound"
        );
        assert_eq!(output.status.code(), Some(1));
        let output = text(&output);
        status(&output, "sdk", "passed");
        status(&output, "compile-link", "failed");
        assert!(
            output.contains("memory allocation") && output.contains("failed"),
            "{output}"
        );
        assert!(!fixture.0.join("core").exists());
        fixture.clean();
    }
    #[cfg(target_os = "macos")]
    {
        // Darwin retains inherited address-space limits. Use a finite failed
        // tool to verify diagnostic clipping through the nested compiler probe.
        let nested_failure = fixture.0.join("nested-finite-failure");
        fs::create_dir(&nested_failure).unwrap();
        for tool in ["clang", LINKER] {
            std::os::unix::fs::symlink(Path::new(&tools).join(tool), nested_failure.join(tool))
                .unwrap();
        }
        let opt = Path::new(&tools)
            .join("opt")
            .to_string_lossy()
            .replace('\'', "'\\''");
        fs::write(
            nested_failure.join("opt"),
            format!(
                "#!/bin/sh\nif [ \"$1\" = --version ]; then exec '{opt}' \"$@\"; fi\n\
                 printf 'finite-opt-failure\\n' >&2\n\
                 count=0\nwhile [ \"$count\" -lt 1024 ]; do\n\
                 printf '%s' 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx' >&2\n\
                 count=$((count + 1))\ndone\n\
                 printf '\\nfinite-opt-tail-must-be-clipped\\n' >&2\nexit 9\n"
            ),
        )
        .unwrap();
        fs::set_permissions(
            nested_failure.join("opt"),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let output = fixture
            .command()
            .env("OXID_LLVM_BIN", nested_failure)
            .args(["doctor", "--native", "--message-format=json"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let output = text(&output);
        status(&output, "sdk", "passed");
        status(&output, "compile-link", "failed");
        status(&output, "execute", "skipped");
        assert_eq!(output.lines().count(), 1);
        assert!(output.len() < 12_000, "{output}");
        assert!(output.contains("finite-opt-failure"), "{output}");
        assert!(!output.contains("finite-opt-tail-must-be-clipped"));
        assert!(!fixture.0.join("core").exists());
        fixture.clean();
    }
}
