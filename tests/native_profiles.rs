//! Public profile admission and real LLVM optimization/semantic acceptance.
//! Rust's cargo profile does not select Oxid's native optimization profile.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
    compiler: PathBuf,
    tools: PathBuf,
    environment: Vec<(OsString, OsString)>,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-native-profiles-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let compiler = root.join("oxid");
        fs::copy(env!("CARGO_BIN_EXE_oxid"), &compiler).unwrap();
        let tools = root.join("tools");
        fs::create_dir(&tools).unwrap();
        fs::create_dir(root.join("traces")).unwrap();
        let mut environment: Vec<_> = std::env::vars_os()
            .filter(|(name, _)| {
                !matches!(
                    name.to_str(),
                    Some(
                        "OXID_LLVM_BIN"
                            | "OXID_PATH"
                            | "LD_PRELOAD"
                            | "LD_AUDIT"
                            | "LD_TRACE_LOADED_OBJECTS"
                    )
                )
            })
            .collect();
        environment.push(("OXID_LLVM_BIN".into(), tools.clone().into_os_string()));
        environment.push((
            "PROFILE_TOOL_MARKER".into(),
            root.join("tools-invoked").into_os_string(),
        ));
        let fixture = Self {
            root,
            compiler,
            tools,
            environment,
        };
        fixture.manifest(None);
        fixture.write("app/src/main.ox", "fn main()->i32{return 7;}\n");
        fixture.write("app/tests/smoke.ox", "fn main()->i32{return 0;}\n");
        for tool in ["clang", "opt", "ld.lld"] {
            fixture.script_tool(
                tool,
                "#!/bin/sh\nprintf '%s\\n' called >> \"$PROFILE_TOOL_MARKER\"\nexit 91\n",
            );
        }
        fixture
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn manifest(&self, profile: Option<&str>) {
        let profile = profile.map_or(String::new(), |value| format!("profile = \"{value}\"\n"));
        self.write("app/oxid.toml", &format!("[project]\nname = \"app\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[tests]\nsmoke = \"tests/smoke.ox\"\n[build]\nentry-mode = \"result\"\n{profile}"));
    }

    fn script_tool(&self, name: &str, text: &str) {
        let path = self.tools.join(name);
        if fs::symlink_metadata(&path).is_ok() {
            fs::remove_file(&path).unwrap();
        }
        fs::write(&path, text).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn real_tools(&self) {
        let real = PathBuf::from(
            std::env::var_os("OXID_LLVM_BIN").expect("set OXID_LLVM_BIN to real LLVM 19.1.7 tools"),
        );
        for tool in ["clang", "opt", "ld.lld"] {
            fs::remove_file(self.tools.join(tool)).unwrap();
            std::os::unix::fs::symlink(real.join(tool), self.tools.join(tool)).unwrap();
        }
    }

    fn command(&self, directory: &str, args: &[&str]) -> Command {
        let mut command = Command::new(&self.compiler);
        command
            .current_dir(self.root.join(directory))
            .args(args)
            .env_clear()
            .envs(self.environment.iter().cloned())
            .stdin(Stdio::null());
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command("app", args).output().unwrap()
    }

    fn build(&self, flags: &[&str]) -> Vec<u8> {
        let mut args = vec!["build", "--message-format=json"];
        args.extend_from_slice(flags);
        success(self.run(&args));
        let body = fs::read(self.executable()).unwrap();
        assert_eq!(&body[..7], b"\x7fELF\x02\x01\x01");
        body
    }

    fn traced_build(&self, name: &str, directory: &str, flags: &[&str]) -> (Output, String) {
        assert!(
            Path::new("/usr/bin/strace").is_file(),
            "install strace for native profile acceptance"
        );
        let path = self.root.join("traces").join(name);
        let output = Command::new("/usr/bin/strace")
            .args(["-qq", "-f", "-s", "65536", "-e", "trace=execve", "-o"])
            .arg(&path)
            .arg("--")
            .arg(&self.compiler)
            .args(["build", "--message-format=json"])
            .args(flags)
            .current_dir(self.root.join(directory))
            .env_clear()
            .envs(self.environment.iter().cloned())
            .stdin(Stdio::null())
            .output()
            .unwrap();
        (output, fs::read_to_string(path).unwrap())
    }

    fn phases(&self, trace: &str, codegen: &[&str], runtime: &[&str]) {
        let selected = format!("execve({:?}, ", self.tools.join("clang"));
        let clang: Vec<_> = trace
            .lines()
            .filter(|line| line.contains(&selected) && line.contains("\"--no-default-config\""))
            .collect();
        let ir: Vec<_> = clang
            .iter()
            .filter(|line| line.contains("\"-x\", \"ir\""))
            .collect();
        let c: Vec<_> = clang
            .iter()
            .filter(|line| line.contains("\"-c\"") && line.contains("\"runtime.c\""))
            .collect();
        assert_eq!(ir.len(), codegen.len(), "{trace}");
        assert_eq!(c.len(), runtime.len(), "{trace}");
        for (line, expected) in ir
            .into_iter()
            .zip(codegen)
            .chain(c.into_iter().zip(runtime))
        {
            let options: Vec<_> = line
                .split('"')
                .filter(|word| {
                    word.starts_with("-O")
                        || word.starts_with("-flto")
                        || word.starts_with("-ffast-math")
                        || word.starts_with("-march")
                        || word.starts_with("-mtune")
                })
                .collect();
            assert_eq!(options, [*expected], "{line}");
            assert!(
                line.contains("\"--target=x86_64-unknown-linux-gnu\""),
                "{line}"
            );
        }
        let links: Vec<_> = clang
            .iter()
            .filter(|line| line.contains("\"-pie\""))
            .collect();
        assert_eq!(links.len(), runtime.len(), "{trace}");
        assert!(
            links
                .iter()
                .all(|line| !line.split('"').any(|word| word.starts_with("-O"))),
            "{trace}"
        );
        let opt = format!("execve({:?}, ", self.tools.join("opt"));
        let verifications: Vec<_> = trace
            .lines()
            .filter(|line| line.contains(&opt) && !line.contains("\"--version\""))
            .collect();
        assert_eq!(verifications.len(), runtime.len(), "{trace}");
        assert!(
            verifications.iter().all(
                |line| line.contains("\"-passes=verify\", \"-disable-output\", \"program.ll\"")
            ),
            "{trace}"
        );
    }

    fn executable(&self) -> PathBuf {
        self.root.join("app/.oxid/bin/app")
    }

    fn execute(&self, payload: &[u8]) -> Output {
        let mut child = Command::new(self.executable())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(payload).unwrap();
        child.wait_with_output().unwrap()
    }

    fn no_tools_or_cache(&self) {
        assert!(!self.root.join("tools-invoked").exists());
        for path in ["app/.oxid/cache", "other/.oxid/cache", ".oxid/cache"] {
            assert!(!self.root.join(path).exists(), "unexpected {path}");
        }
    }

    fn objects(&self) -> Vec<Vec<u8>> {
        let directory = self.root.join("app/.oxid/cache/native-object-v1");
        let mut objects: Vec<_> = fs::read_dir(directory)
            .unwrap()
            .map(|entry| {
                let bytes = fs::read(entry.unwrap().path()).unwrap();
                assert_eq!(&bytes[..16], b"OXID-OBJECT-v1\0\0");
                assert_eq!(
                    u64::from_le_bytes(bytes[48..56].try_into().unwrap()) as usize,
                    bytes.len() - 88
                );
                assert_eq!(&bytes[56..88], &Sha256::digest(&bytes[88..])[..]);
                assert_eq!(&bytes[88..95], b"\x7fELF\x02\x01\x01");
                assert_eq!(&bytes[104..108], &[1, 0, 62, 0]);
                bytes[88..].to_vec()
            })
            .collect();
        objects.sort();
        objects
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "retained failed native profile fixture: {}",
                self.root.display()
            );
        } else {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn success(output: Output) -> Output {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
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

fn outcome(output: Output, status: i32, stdout: &[u8], stderr: &[u8]) {
    assert_eq!(output.status.code(), Some(status), "{output:?}");
    assert_eq!(output.stdout, stdout, "{output:?}");
    assert_eq!(output.stderr, stderr, "{output:?}");
}

fn diagnostic(source: &str, marker: &str, code: &str, stage: &str, message: &str) -> Vec<u8> {
    let offset = source.find(marker).unwrap();
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap().chars().count() + 1;
    format!("error[{code}] ({stage}): {message}\n  --> src/main.ox:{line}:{column}\n").into_bytes()
}

#[test]
fn profile_cli_values_are_unambiguous_and_limited_to_project_build() {
    let fixture = Fixture::new();
    for command in ["check", "run", "test", "lock", "fmt", "doctor", "package"] {
        for profile in ["bounded", "dev", "release"] {
            failure(fixture.run(&[command, "--profile", profile, "--message-format=json"]));
            fixture.no_tools_or_cache();
            assert!(!fixture.root.join("app/oxid.lock").exists());
        }
    }
    for flags in [
        vec!["--profile"],
        vec!["--profile="],
        vec!["--profile", "fast"],
        vec!["--profile=Release"],
        vec!["--profile=O2"],
        vec!["--profile", "dev", "--profile=dev"],
        vec!["--profile=dev", "--profile", "release"],
        vec!["--profile=release", "stray"],
    ] {
        let mut args = vec!["build", "--message-format=json"];
        args.extend(flags);
        failure(fixture.run(&args));
        fixture.no_tools_or_cache();
    }
    for profile in ["bounded", "dev", "release"] {
        failure(fixture.run(&[
            "compile",
            "src/main.ox",
            "--edition=typed-preview",
            "--backend=llvm",
            "--output=kept",
            "--profile",
            profile,
        ]));
        assert!(!fixture.root.join("app/kept").exists());
    }
    assert!(!fixture.root.join("app/.oxid").exists());
}

#[test]
fn profile_value_must_be_adjacent_and_cannot_be_taken_past_global_flags() {
    let fixture = Fixture::new();
    for interrupting_flag in [
        "",
        "--",
        "--message-format=json",
        "--locked",
        "--offline",
        "--frozen",
        "--entry-mode=result",
        "--edition=typed-preview",
    ] {
        let text = failure(fixture.run(&["build", "--profile", interrupting_flag, "release"]));
        assert!(
            text.contains("--profile requires"),
            "{interrupting_flag}: {text}"
        );
        fixture.no_tools_or_cache();
        assert!(!fixture.root.join("app/oxid.lock").exists());
        assert!(!fixture.root.join("app/.oxid").exists());
    }
}

#[test]
fn invalid_manifest_profile_is_not_hidden_by_cli_override_or_command_selection() {
    let fixture = Fixture::new();
    success(fixture.run(&["lock"]));
    let lock = fs::read(fixture.root.join("app/oxid.lock")).unwrap();
    for profile in ["", "fast", "debug", "Release", "O2"] {
        fixture.manifest(Some(profile));
        for command in [
            "check", "run", "test", "lock", "build", "fmt", "doctor", "package",
        ] {
            let text = failure(fixture.run(&[command, "--message-format=json"]));
            assert!(text.contains("profile"), "{command}: {text}");
        }
        let text = failure(fixture.run(&["build", "--profile=dev", "--message-format=json"]));
        assert!(text.contains("profile"), "{text}");
        fixture.no_tools_or_cache();
        assert_eq!(fs::read(fixture.root.join("app/oxid.lock")).unwrap(), lock);
        assert!(!fixture.root.join("app/.oxid").exists());
    }
}

#[test]
fn supported_manifest_profiles_do_not_change_interpreter_or_declared_test_semantics() {
    let fixture = Fixture::new();
    for profile in ["bounded", "dev", "release"] {
        fixture.manifest(Some(profile));
        success(fixture.run(&["check"]));
        let result = success(fixture.run(&["run", "--message-format=json"]));
        assert!(String::from_utf8_lossy(&result.stdout).contains("\"value\":7"));
        let tested = success(fixture.run(&["test", "--message-format=json"]));
        let summary = String::from_utf8(tested.stdout).unwrap();
        assert!(
            summary.contains("\"passed\":1") && summary.contains("\"failed\":0"),
            "{summary}"
        );
        fixture.no_tools_or_cache();
        assert!(!fixture.root.join("app/.oxid").exists());
    }
}

#[test]
fn profile_changes_require_lock_update_and_fail_before_native_side_effects() {
    let fixture = Fixture::new();
    fixture.manifest(Some("dev"));
    success(fixture.run(&["lock"]));
    let lock = fs::read(fixture.root.join("app/oxid.lock")).unwrap();
    fixture.write("app/.oxid/bin/app", "previous executable");
    fixture.manifest(Some("release"));
    for flag in ["--profile=dev", "--profile=release"] {
        let text = failure(fixture.run(&[
            "build",
            flag,
            "--frozen",
            "--incremental",
            "--message-format=json",
        ]));
        assert!(text.contains("out of date"), "{text}");
        assert_eq!(
            fs::read(fixture.executable()).unwrap(),
            b"previous executable"
        );
        assert_eq!(fs::read(fixture.root.join("app/oxid.lock")).unwrap(), lock);
        fixture.no_tools_or_cache();
    }
}

#[test]
fn workspace_admits_all_settings_and_selected_sources_before_publishing_any_member() {
    let fixture = Fixture::new();
    fixture.write("oxid.toml", "[workspace]\nedition = \"typed-preview\"\nmembers = [\"app\", \"other\"]\ndefault-members = [\"app\"]\n");
    let other = "[project]\nname = \"other\"\nedition = \"typed-preview\"\n[build]\nprofile = \"release\"\n";
    fixture.write("other/oxid.toml", &other.replace("release", "fast"));
    fixture.write("other/src/main.ox", "fn main()->i32{return 9;}\n");
    for path in ["app/.oxid/bin/app", "other/.oxid/bin/other"] {
        fixture.write(path, "previous executable");
    }
    let invalid = failure(
        fixture
            .command(
                "",
                &[
                    "build",
                    "--package=app",
                    "--profile=dev",
                    "--incremental",
                    "--message-format=json",
                ],
            )
            .output()
            .unwrap(),
    );
    assert!(
        invalid.contains("profile") && !invalid.contains("workspace-member"),
        "{invalid}"
    );
    fixture.write("other/oxid.toml", other);
    fixture.write("other/src/main.ox", "fn main()->i32{return false;}\n");
    let invalid = failure(
        fixture
            .command(
                "",
                &[
                    "build",
                    "--workspace",
                    "--profile=release",
                    "--incremental",
                    "--message-format=json",
                ],
            )
            .output()
            .unwrap(),
    );
    assert!(
        invalid.contains("E0300") && !invalid.contains("workspace-member"),
        "{invalid}"
    );
    for path in ["app/.oxid/bin/app", "other/.oxid/bin/other"] {
        assert_eq!(
            fs::read(fixture.root.join(path)).unwrap(),
            b"previous executable"
        );
    }
    fixture.no_tools_or_cache();
    fixture.write("oxid.toml", "[workspace]\nedition = \"typed-preview\"\nmembers = [\"app\"]\n[build]\nprofile = \"dev\"\n");
    failure(
        fixture
            .command("", &["build", "--workspace", "--profile=release"])
            .output()
            .unwrap(),
    );
    fixture.no_tools_or_cache();
}

#[test]
fn profile_selection_does_not_change_compile_time_type_or_ownership_rejections() {
    let fixture = Fixture::new();
    for (source, code) in [
        ("fn main()->i32{return false;}\n", "E0300"),
        ("struct R { value:i32 }fn take(x:R)->i32{return x.value;}fn main()->i32{let x=R{value:7};let a=take(x);return a+take(x);}\n", "E0310"),
    ] {
        fixture.write("app/src/main.ox", source);
        let default = fixture.run(&["build", "--message-format=json"]);
        assert_eq!(default.status.code(), Some(1), "{default:?}");
        assert!(String::from_utf8_lossy(&default.stdout).contains(code), "{default:?}");
        for profile in ["bounded", "dev", "release"] {
            let selected = fixture.run(&["build", "--profile", profile, "--message-format=json"]);
            assert_eq!(selected.status.code(), Some(1), "{selected:?}");
            assert_eq!(selected.stdout, default.stdout);
            assert_eq!(selected.stderr, default.stderr);
        }
        fixture.no_tools_or_cache();
    }
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN and /usr/bin/strace"]
fn real_profiles_compile_both_stages_and_reuse_only_matching_optimization_objects() {
    let fixture = Fixture::new();
    fixture.real_tools();
    fixture.write("app/src/main.ox", "fn sum(n:i32)->i32{let mut total=0;let mut i=0;while i<n{total=total+i;i=i+1;}return total;}fn main()->i32{return sum(8);}\n");
    let (plain, trace) = fixture.traced_build("default", "app", &[]);
    success(plain);
    fixture.phases(&trace, &["-O0"], &["-O0"]);
    let default = fs::read(fixture.executable()).unwrap();
    outcome(fixture.execute(b""), 0, b"28\n", b"");
    assert!(!fixture.root.join("app/.oxid/cache").exists());
    let mut release = None;
    for (name, profile, status, codegen, flag, entries) in [
        ("bounded-cold", "bounded", "miss", true, "-O0", 1),
        ("dev-warm", "dev", "hit", false, "-O0", 1),
        ("release-cold", "release", "miss", true, "-O2", 2),
        ("release-warm", "release", "hit", false, "-O2", 2),
        ("dev-return", "dev", "hit", false, "-O0", 2),
    ] {
        let (output, trace) =
            fixture.traced_build(name, "app", &["--profile", profile, "--incremental"]);
        let output = success(output);
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(&format!("\"status\":\"{status}\"")), "{text}");
        fixture.phases(
            &trace,
            if codegen {
                std::slice::from_ref(&flag)
            } else {
                &[]
            },
            &[flag],
        );
        let bytes = fs::read(fixture.executable()).unwrap();
        if flag == "-O0" {
            assert_eq!(bytes, default);
        } else if let Some(previous) = &release {
            assert_eq!(&bytes, previous);
        } else {
            assert_ne!(bytes, default);
            release = Some(bytes);
        }
        let objects = fixture.objects();
        assert_eq!(objects.len(), entries);
        if entries == 2 {
            assert_ne!(
                objects[0], objects[1],
                "O2 must change actual code generation in this fixture"
            );
        }
        outcome(fixture.execute(b""), 0, b"28\n", b"");
    }
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN and /usr/bin/strace"]
fn manifest_and_workspace_profiles_resolve_per_member_with_explicit_cli_precedence() {
    let fixture = Fixture::new();
    fixture.real_tools();
    fixture.write("math/src/lib.ox", "pub fn value()->i32{return 7;}\n");
    fixture.write(
        "app/src/main.ox",
        "fn main()->i32{return crate::math::value();}\n",
    );
    for (name, root, dependency, flags, expected) in [
        ("manifest-release", "release", "dev", vec![], "-O2"),
        ("cli-dev", "release", "dev", vec!["--profile=dev"], "-O0"),
        ("dependency-release", "dev", "release", vec![], "-O0"),
        (
            "cli-release",
            "dev",
            "release",
            vec!["--profile", "release"],
            "-O2",
        ),
    ] {
        fixture.write("app/oxid.toml", &format!("[project]\nname=\"app\"\nedition=\"typed-preview\"\n[build]\nentry-mode=\"result\"\nprofile=\"{root}\"\n[dependencies]\nmath=\"../math\"\n"));
        fixture.write("math/oxid.toml", &format!("[project]\nname=\"math\"\nkind=\"lib\"\nedition=\"typed-preview\"\n[build]\nprofile=\"{dependency}\"\n"));
        let (output, trace) = fixture.traced_build(name, "app", &flags);
        success(output);
        fixture.phases(&trace, &[expected], &[expected]);
        outcome(fixture.execute(b""), 0, b"7\n", b"");
    }
    fixture.write(
        "oxid.toml",
        "[workspace]\nedition=\"typed-preview\"\nmembers=[\"app\",\"math\",\"other\"]\n",
    );
    fixture.write("other/oxid.toml", "[project]\nname=\"other\"\nedition=\"typed-preview\"\n[build]\nprofile=\"release\"\nentry-mode=\"result\"\n");
    fixture.write("other/src/main.ox", "fn main()->i32{return 9;}\n");
    for (name, flags, expected) in [
        ("workspace-default", vec!["--workspace"], ["-O0", "-O2"]),
        (
            "workspace-dev",
            vec!["--workspace", "--profile=dev"],
            ["-O0", "-O0"],
        ),
        (
            "workspace-release",
            vec!["--workspace", "--profile=release"],
            ["-O2", "-O2"],
        ),
    ] {
        let (output, trace) = fixture.traced_build(name, "", &flags);
        let output = success(output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("checked-library"));
        fixture.phases(&trace, &expected, &expected);
        outcome(fixture.execute(b""), 0, b"7\n", b"");
        outcome(
            Command::new(fixture.root.join("other/.oxid/bin/other"))
                .output()
                .unwrap(),
            0,
            b"9\n",
            b"",
        );
        assert!(!fixture.root.join("math/.oxid/bin").exists());
    }
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN"]
fn optimized_scalar_paths_preserve_discarded_failures_and_untaken_operations() {
    let fixture = Fixture::new();
    fixture.real_tools();
    for (expression, marker, code, message) in [
        (
            "2147483647+1",
            "+1",
            "E0604",
            "checked i32 arithmetic overflow",
        ),
        ("1/0", "/0", "E0607", "checked i32 division by zero"),
        (
            "-2147483648%-1",
            "%-1",
            "E0604",
            "checked i32 arithmetic overflow",
        ),
    ] {
        let source = format!("fn main()->i32{{let unused={expression};return 7;}}\n");
        fixture.write("app/src/main.ox", &source);
        let stderr = diagnostic(&source, marker, code, "oir-run", message);
        if code == "E0607" {
            // Optimization is independent of the source-path presentation axis.
            fixture.build(&["--profile=release", "--no-reproducible"]);
            let absolute = String::from_utf8(stderr.clone()).unwrap().replace(
                "src/main.ox",
                fixture.root.join("app/src/main.ox").to_str().unwrap(),
            );
            outcome(fixture.execute(b""), 1, b"", absolute.as_bytes());
        }
        for profile in ["dev", "release"] {
            for mode in ["--entry-mode=result", "--entry-mode=process"] {
                fixture.build(&["--profile", profile, mode, "--reproducible"]);
                outcome(fixture.execute(b""), 1, b"", &stderr);
            }
        }
    }
    for (source, expected) in [
        ("fn main()->i32{let a=false&&(2147483647+1==0);let b=true||(1/0==0);if false{let bad=-2147483648%-1;}if !a&&b{return 7;}return 99;}\n", b"7\n".as_slice()),
        ("fn main()->i32{return (-17/5)*10+(-17%5);}\n", b"-32\n".as_slice()),
        ("fn main()->i32{return -2147483648;}\n", b"-2147483648\n".as_slice()),
    ] {
        fixture.write("app/src/main.ox", source);
        for profile in ["dev", "release"] {
            fixture.build(&["--profile", profile, "--reproducible"]);
            outcome(fixture.execute(b""), 0, expected, b"");
        }
    }
    fixture.write(
        "app/src/main.ox",
        "fn wait()->i32{while true {}return 0;}fn main()->i32{return wait();}\n",
    );
    let mut previous = None;
    for profile in ["dev", "release"] {
        fixture.build(&["--profile", profile, "--reproducible"]);
        let result = fixture.execute(b"");
        assert_eq!(result.status.code(), Some(1), "{result:?}");
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr).starts_with(
                "error[E0601] (oir-run): execution fuel exhausted\n  --> src/main.ox:1:"
            ),
            "{result:?}"
        );
        if let Some(before) = &previous {
            assert_eq!(&result.stderr, before);
        } else {
            previous = Some(result.stderr);
        }
    }
}

const OWNED: &str = r#"use std::io::read_stdin;
use std::io::ReadStatus;
use std::io::write_stdout;
use std::io::WriteStatus;
struct Empty {}
struct Pair { left:i32, right:i32 }
enum Value { Number(i32), Flag(bool), End }
fn relay(value:Value)->Value{return value;}
fn score(value:Value)->i32{match value{Value::Number(n)=>{return n;},Value::Flag(b)=>{if b{return 2;}return 3;},Value::End=>{return 5;},}}
fn bump(pair:&mut Pair)->(){pair.left=pair.left+5;return;}
fn empty(value:Empty, values:[i32;0])->i32{let copied=value;let moved=values;return moved.len();}
fn prefix()->i32{let bytes=[65];let written=write_stdout(&bytes);match written{WriteStatus::Complete=>{return 0;},WriteStatus::InvalidInput=>{return 72;},WriteStatus::IoError(n)=>{return 73;},}}
fn spin()->i32{while true {}return 0;}
fn main()->i32{
    let mut input=[0];let read=read_stdin(&mut input);
    match read{ReadStatus::Full=>{},ReadStatus::Eof(n)=>{return 70;},ReadStatus::IoError=>{return 71;},}
    let selected=input[0];let mut values=[7];
    if selected==0{let unused=2147483647+1;return 99;}
    if selected==1{let unused=1/0;return 99;}
    if selected==2{let unused=-2147483648%-1;return 99;}
    if selected==3{return values[-1];}
    if selected==4{return values[1];}
    if selected==5{values[1]=9/0;return 99;}
    if selected==6{let sent=prefix();if sent!=0{return sent;}return values[2];}
    if selected==7{let sent=prefix();if sent!=0{return sent;}return spin();}
    if selected==8{let a=false&&(values[3]==0);let b=true||(values[4]==0);if !a&&b{return 8;}return 99;}
    if selected==9{let mut value=relay(Value::Number(17));let number=score(value);value=relay(Value::Flag(true));let flag=score(value);value=relay(Value::End);let end=score(value);let zeros:[i32;0]=[];return number+flag+end+empty(Empty{},zeros);}
    if selected==10{let mut pair=Pair{left:7,right:3};bump(&mut pair);let moved=pair;return moved.left+moved.right;}
    if selected==11||selected==12{let left=(selected==11)||(values[0]==7);let right=(selected==12)&&(values[0]==7);if left{if right{return 12;}return 11;}return 99;}
    if selected==13{return 255;}
    if selected==14{return -1;}
    if selected==15{return 256;}
    return 0;
}
"#;

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN"]
fn optimized_owned_values_bounds_effect_order_and_process_statuses_match_exact_oracles() {
    let fixture = Fixture::new();
    fixture.real_tools();
    fixture.write("app/src/main.ox", OWNED);
    for profile in ["dev", "release"] {
        fixture.build(&[
            "--profile",
            profile,
            "--entry-mode=process",
            "--reproducible",
        ]);
        for (input, marker, code, message, stdout) in [
            (
                0,
                "+1;return 99",
                "E0604",
                "checked i32 arithmetic overflow",
                b"".as_slice(),
            ),
            (
                1,
                "/0;return 99",
                "E0607",
                "checked i32 division by zero",
                b"".as_slice(),
            ),
            (
                2,
                "%-1",
                "E0604",
                "checked i32 arithmetic overflow",
                b"".as_slice(),
            ),
            (
                3,
                "values[-1]",
                "E0606",
                "array index out of bounds",
                b"".as_slice(),
            ),
            (
                4,
                "values[1];",
                "E0606",
                "array index out of bounds",
                b"".as_slice(),
            ),
            (
                5,
                "/0;return 99;}\n    if selected==6",
                "E0607",
                "checked i32 division by zero",
                b"".as_slice(),
            ),
            (
                6,
                "values[2]",
                "E0606",
                "array index out of bounds",
                b"A".as_slice(),
            ),
        ] {
            // Arithmetic uses the shared scalar diagnostic contract even
            // when the surrounding program contains owned values and I/O.
            let stage = if code == "E0606" {
                "oir-owned-run"
            } else {
                "oir-run"
            };
            let stderr = diagnostic(OWNED, marker, code, stage, message);
            outcome(fixture.execute(&[input]), 1, stdout, &stderr);
        }
        let exhausted = diagnostic(
            OWNED,
            "}return 0;}\nfn main",
            "E0601",
            "oir-run",
            "execution fuel exhausted",
        );
        outcome(fixture.execute(&[7]), 1, b"A", &exhausted);
        for (input, status) in [
            (8, 8),
            (9, 24),
            (10, 15),
            (11, 11),
            (12, 12),
            (13, 255),
            (16, 0),
        ] {
            outcome(fixture.execute(&[input]), status, b"", b"");
        }
        let invalid = diagnostic(
            OWNED,
            "main()->i32",
            "E0600",
            "oir-run",
            "process main must return a status in 0..255",
        );
        for input in [14, 15] {
            outcome(fixture.execute(&[input]), 1, b"", &invalid);
        }
    }
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN"]
fn optimized_io_preserves_raw_bytes_eof_invalid_input_and_real_descriptor_failures() {
    let fixture = Fixture::new();
    fixture.real_tools();
    let input = "use std::io::read_stdin;use std::io::ReadStatus;fn main()->i32{let mut bytes=[0,0];let result=read_stdin(&mut bytes);match result{ReadStatus::Full=>{if bytes[0]==0&&bytes[1]==255{return 39;}return 40;},ReadStatus::Eof(n)=>{return n;},ReadStatus::IoError=>{return 71;},}}\n";
    let output = "use std::io::write_stdout;use std::io::WriteStatus;fn main()->i32{let bytes=[0,65,255];let result=write_stdout(&bytes);match result{WriteStatus::Complete=>{return 39;},WriteStatus::InvalidInput=>{return 72;},WriteStatus::IoError(n)=>{return 73;},}}\n";
    for profile in ["dev", "release"] {
        fixture.write("app/src/main.ox", input);
        fixture.build(&[
            "--profile",
            profile,
            "--entry-mode=process",
            "--reproducible",
        ]);
        for (payload, status) in [
            (b"\0\xff".as_slice(), 39),
            (b"x".as_slice(), 1),
            (b"".as_slice(), 0),
        ] {
            outcome(fixture.execute(payload), status, b"", b"");
        }
        let mut command = Command::new(fixture.executable());
        command.stdin(Stdio::null());
        // close is async-signal-safe and scoped to this test child.
        unsafe {
            command.pre_exec(|| {
                libc::close(libc::STDIN_FILENO);
                Ok(())
            });
        }
        outcome(command.output().unwrap(), 71, b"", b"");
        fixture.write("app/src/main.ox", output);
        fixture.build(&[
            "--profile",
            profile,
            "--entry-mode=process",
            "--reproducible",
        ]);
        outcome(fixture.execute(b""), 39, &[0, 65, 255], b"");
        let full = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        outcome(
            Command::new(fixture.executable())
                .stdout(Stdio::from(full))
                .output()
                .unwrap(),
            73,
            b"",
            b"",
        );
        fixture.write("app/src/main.ox", &output.replace("[0,65,255]", "[65,256]"));
        fixture.build(&[
            "--profile",
            profile,
            "--entry-mode=process",
            "--reproducible",
        ]);
        outcome(fixture.execute(b""), 72, b"", b"");
    }
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN"]
fn switching_profile_with_a_failed_link_preserves_the_previous_executable_and_lock() {
    let fixture = Fixture::new();
    fixture.real_tools();
    success(fixture.run(&["lock"]));
    let lock = fs::read(fixture.root.join("app/oxid.lock")).unwrap();
    let original = fixture.build(&["--profile=dev", "--incremental", "--frozen"]);
    fixture.script_tool("ld.lld", "#!/bin/sh\nif [ \"$1\" = --version ]; then printf '%s\\n' 'LLD 19.1.7'; exit 0; fi\nprintf '%s\\n' 'forced profile link failure' >&2\nexit 9\n");
    let error = failure(fixture.run(&[
        "build",
        "--profile=release",
        "--incremental",
        "--frozen",
        "--message-format=json",
    ]));
    assert!(error.contains("forced profile link failure"), "{error}");
    assert_eq!(fs::read(fixture.executable()).unwrap(), original);
    assert_eq!(fs::read(fixture.root.join("app/oxid.lock")).unwrap(), lock);
    outcome(fixture.execute(b""), 0, b"7\n", b"");
}
