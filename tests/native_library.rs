//! Public admission controls and explicitly serialized native-library clients.
//! The real-toolchain test compiles clients; it is never an environment-based skip.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const PRICING: &str = include_str!("fixtures/native_library/pricing.ox.in");

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-native-library-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("input.ox"), source).unwrap();
        Self(root)
    }

    fn command(&self, export: &str, directory: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command.current_dir(&self.0).stdin(Stdio::null()).args([
            "--edition=typed-preview",
            "library",
            "--export",
            export,
            "--output",
            directory,
            "input.ox",
            "--message-format=json",
        ]);
        command
    }

    fn without_tools(&self, export: &str, extra: &[&str]) -> Output {
        self.command(export, "published-v1")
            .env("OXID_LLVM_BIN", self.0.join("absent-tools"))
            .args(extra)
            .output()
            .unwrap()
    }

    fn entries(&self) -> Vec<std::ffi::OsString> {
        let mut result: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        result.sort();
        result
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() || std::env::var_os("OXID_TEST_KEEP_ARTIFACTS").is_some() {
            eprintln!("retained native-library fixture: {}", self.0.display());
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn failure(output: Output, code: &str) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(&format!("\"code\":\"{code}\"")), "{text}");
    assert!(!text.contains("\"success\":true"), "{text}");
    text
}

#[test]
#[cfg(not(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
fn library_command_rejects_unsupported_hosts_before_tools_and_publication() {
    let fixture = Fixture::new(PRICING);
    let text = failure(fixture.without_tools("crate::line_total", &[]), "E0700");
    assert!(
        text.contains("scalar native libraries currently require Linux x86_64 or ARM64"),
        "{text}"
    );
    assert!(!text.contains("E0701"), "{text}");
    assert_eq!(fixture.entries(), [std::ffi::OsString::from("input.ox")]);
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux {
    use super::*;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::path::Path;

    fn hash(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn public_export_identity_and_exact_i32_signature_are_required_before_tools() {
        for (source, export) in [
            (
                "fn line_total(value:i32)->i32{return value;}",
                "crate::line_total",
            ),
            (
                "pub fn line_total(value:i32)->i32{return value;}",
                "crate::missing",
            ),
            (
                "pub fn line_total(value:bool)->i32{if value{return 1;}return 0;}",
                "crate::line_total",
            ),
            (
                "pub fn line_total(value:i32)->bool{return value==0;}",
                "crate::line_total",
            ),
        ] {
            let fixture = Fixture::new(source);
            let before = fixture.entries();
            let text = failure(fixture.without_tools(export, &[]), "E0700");
            assert!(!text.contains("E0701"), "{text}");
            assert_eq!(fixture.entries(), before);
        }
    }

    #[test]
    fn unused_loaded_declarations_cannot_expand_the_library_abi_or_escape_typechecking() {
        for unused in [
            "struct P{x:i32}",
            "enum E{Value}",
            "fn unused(value:u8)->u8{return value;}",
            "fn unused(value:i64)->i64{return value;}",
            "fn unused(value:f64)->f64{return value;}",
            "fn unused(value:char)->char{return value;}",
            "fn unused(value:&[i32])->i32{return value[0];}",
            "fn unused<T:Copy>(value:T)->T{return value;}",
        ] {
            let fixture = Fixture::new(&format!(
                "{unused}\npub fn line_total(value:i32)->i32{{return value;}}"
            ));
            let before = fixture.entries();
            let text = failure(fixture.without_tools("crate::line_total", &[]), "E0700");
            assert!(!text.contains("E0701"), "{text}");
            assert_eq!(fixture.entries(), before);
        }
        let fixture = Fixture::new("mod hidden;pub fn line_total(value:i32)->i32{return value;}");
        fs::write(
            fixture.0.join("hidden.ox"),
            "fn unused(value:f64)->f64{return value;}",
        )
        .unwrap();
        let before = fixture.entries();
        failure(fixture.without_tools("crate::line_total", &[]), "E0700");
        assert_eq!(fixture.entries(), before);

        let fixture = Fixture::new(
            "fn unused()->i32{return true;} pub fn line_total(value:i32)->i32{return value;}",
        );
        failure(fixture.without_tools("crate::line_total", &[]), "E0300");
        assert_eq!(fixture.entries(), [std::ffi::OsString::from("input.ox")]);
    }

    #[test]
    fn valid_scalar_library_and_dev_alias_reach_tools_without_publishing_partial_artifacts() {
        for extra in [
            &[][..],
            &["--backend=llvm"][..],
            &["--profile=bounded"][..],
            &["--profile=release"][..],
        ] {
            let fixture = Fixture::new(PRICING);
            let text = failure(fixture.without_tools("crate::line_total", extra), "E0701");
            assert!(!text.contains("E0700"), "{text}");
            assert_eq!(fixture.entries(), [std::ffi::OsString::from("input.ox")]);
        }
    }

    #[test]
    fn library_only_option_errors_do_not_create_outputs() {
        for extra in [
            &["--export=crate::line_total"][..],
            &["--profile=fast"][..],
            &["--backend=other"][..],
            &["--fuel=-1"][..],
            &["--fuel=1000001"][..],
            &["--fuel=one"][..],
        ] {
            let fixture = Fixture::new(PRICING);
            failure(fixture.without_tools("crate::line_total", extra), "E0001");
            assert_eq!(fixture.entries(), [std::ffi::OsString::from("input.ox")]);
        }
    }

    #[test]
    fn global_options_cannot_supply_missing_adjacent_library_values() {
        let pairs = [
            ("--output", "published-v1"),
            ("--export", "crate::line_total"),
            ("--backend", "llvm"),
            ("--profile", "dev"),
            ("--fuel", "5"),
        ];
        for (selected, _) in pairs {
            for interrupted_by in [
                vec!["--message-format=json"],
                vec!["--message-format", "json"],
                vec!["--edition=typed-preview"],
                vec!["--edition", "typed-preview"],
            ] {
                let fixture = Fixture::new(PRICING);
                fs::remove_file(fixture.0.join("input.ox")).unwrap();
                let format_inside = interrupted_by[0].starts_with("--message-format");
                let mut arguments = vec![
                    if format_inside {
                        "--edition=typed-preview"
                    } else {
                        "--message-format=json"
                    },
                    "library",
                ];
                for (key, value) in pairs {
                    arguments.push(key);
                    if key == selected {
                        arguments.extend(interrupted_by.iter().copied());
                    }
                    arguments.push(value);
                }
                arguments.push("input.ox");
                let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
                    .current_dir(&fixture.0)
                    .stdin(Stdio::null())
                    .env("OXID_LLVM_BIN", fixture.0.join("absent-tools"))
                    .args(arguments)
                    .output()
                    .unwrap();
                let text = failure(output, "E0001");
                assert!(
                    text.contains("requires an adjacent nonempty value"),
                    "{text}"
                );
                assert!(
                    fixture.entries().is_empty(),
                    "invalid argv touched source or output"
                );
            }
            for invalid in [Some(""), Some("-1"), Some("--"), None] {
                let fixture = Fixture::new(PRICING);
                fs::remove_file(fixture.0.join("input.ox")).unwrap();
                let mut arguments = vec![
                    "--edition=typed-preview",
                    "--message-format=json",
                    "library",
                ];
                for (key, value) in pairs {
                    if key != selected {
                        arguments.extend([key, value]);
                    }
                }
                arguments.extend(["input.ox", selected]);
                if let Some(value) = invalid {
                    arguments.push(value);
                }
                let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
                    .current_dir(&fixture.0)
                    .stdin(Stdio::null())
                    .env("OXID_LLVM_BIN", fixture.0.join("absent-tools"))
                    .args(arguments)
                    .output()
                    .unwrap();
                let text = failure(output, "E0001");
                assert!(
                    text.contains("requires an adjacent nonempty value"),
                    "{text}"
                );
                assert!(fixture.entries().is_empty());
            }
        }
    }

    #[test]
    fn complete_library_values_keep_global_options_equals_forms_and_literal_paths() {
        for arguments in [
            vec![
                "--edition=typed-preview",
                "library",
                "--output",
                "published-v1",
                "--message-format=json",
                "--export",
                "crate::line_total",
                "input.ox",
            ],
            vec![
                "library",
                "--output",
                "published-v1",
                "--edition=typed-preview",
                "--export",
                "crate::line_total",
                "input.ox",
                "--message-format=json",
            ],
            vec![
                "library",
                "--output=published-v1",
                "--export=crate::line_total",
                "--backend=llvm",
                "--profile=dev",
                "--fuel=5",
                "input.ox",
                "--edition=typed-preview",
                "--message-format=json",
            ],
            vec![
                "--edition=typed-preview",
                "--message-format=json",
                "library",
                "--output",
                "published-v1",
                "--export",
                "crate::line_total",
                "--",
                "--output",
            ],
        ] {
            let fixture = Fixture::new(PRICING);
            if arguments.last() == Some(&"--output") {
                fs::rename(fixture.0.join("input.ox"), fixture.0.join("--output")).unwrap();
            }
            let before = fixture.entries();
            let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
                .current_dir(&fixture.0)
                .stdin(Stdio::null())
                .env("OXID_LLVM_BIN", fixture.0.join("absent-tools"))
                .args(arguments)
                .output()
                .unwrap();
            failure(output, "E0701");
            assert_eq!(fixture.entries(), before);
        }
    }

    #[test]
    fn rejected_process_option_preserves_the_existing_stderr_diagnostic() {
        let fixture = Fixture::new(PRICING);
        let output = fixture.without_tools("crate::line_total", &["--entry-mode=process"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert_eq!(
            output.stderr,
            b"error[E0001] (cli): --entry-mode requires typed-preview run or compile\n"
        );
        assert_eq!(fixture.entries(), [std::ffi::OsString::from("input.ox")]);
    }

    #[test]
    fn an_existing_version_directory_file_or_symlink_is_never_overwritten() {
        use std::os::unix::fs::symlink;
        for kind in ["directory", "file", "symlink"] {
            let fixture = Fixture::new(PRICING);
            let target = fixture.0.join("published-v1");
            match kind {
                "directory" => {
                    fs::create_dir(&target).unwrap();
                    fs::write(target.join("keep"), b"previous library set").unwrap();
                }
                "file" => fs::write(&target, b"previous file").unwrap(),
                "symlink" => {
                    fs::write(fixture.0.join("keep"), b"symlink destination").unwrap();
                    symlink("keep", &target).unwrap();
                }
                _ => unreachable!(),
            }
            let before = fixture.entries();
            let text = failure(fixture.without_tools("crate::line_total", &[]), "E0701");
            assert!(
                text.contains(
                    "native library output already exists; choose a new version directory"
                ),
                "{text}"
            );
            assert_eq!(fixture.entries(), before);
            match kind {
                "directory" => assert_eq!(
                    fs::read(target.join("keep")).unwrap(),
                    b"previous library set"
                ),
                "file" => assert_eq!(fs::read(target).unwrap(), b"previous file"),
                "symlink" => {
                    assert_eq!(fs::read_link(target).unwrap(), Path::new("keep"));
                    assert_eq!(
                        fs::read(fixture.0.join("keep")).unwrap(),
                        b"symlink destination"
                    );
                }
                _ => unreachable!(),
            }
        }
    }

    fn captured(fixture: &Fixture, label: &str, command: &mut Command) -> Output {
        command.current_dir(&fixture.0).stdin(Stdio::null());
        fs::write(
            fixture.0.join(format!("{label}.argv")),
            format!("{command:?}\n"),
        )
        .unwrap();
        let output = command.output().unwrap();
        fs::write(fixture.0.join(format!("{label}.stdout")), &output.stdout).unwrap();
        fs::write(fixture.0.join(format!("{label}.stderr")), &output.stderr).unwrap();
        fs::write(
            fixture.0.join(format!("{label}.exit")),
            format!("{}\n", output.status),
        )
        .unwrap();
        output
    }
    fn success(output: Output) -> Output {
        assert!(output.status.success(), "{output:?}");
        output
    }
    fn client(fixture: &Fixture, label: &str, command: &mut Command, expected: &[u8]) {
        for name in [
            "LD_LIBRARY_PATH",
            "LD_PRELOAD",
            "LD_AUDIT",
            "LD_TRACE_LOADED_OBJECTS",
        ] {
            command.env_remove(name);
        }
        command.current_dir(&fixture.0).stdin(Stdio::null());
        let output = success(captured(fixture, label, command));
        assert_eq!(output.stdout, expected, "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
    }

    fn verify_set(
        fixture: &Fixture,
        bundle: &Path,
        profile: &str,
        fuel: u64,
        llvm: &Path,
    ) -> Vec<(String, Vec<u8>)> {
        let mut names: Vec<_> = fs::read_dir(bundle)
            .unwrap()
            .map(|p| p.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["liboxid_library.so", "metadata.json", "oxid_library.h"].map(std::ffi::OsString::from)
        );
        let library = fs::read(bundle.join("liboxid_library.so")).unwrap();
        assert_eq!(&library[..6], b"\x7fELF\x02\x01");
        assert_eq!(u16::from_le_bytes(library[16..18].try_into().unwrap()), 3);
        assert_eq!(
            u16::from_le_bytes(library[18..20].try_into().unwrap()),
            if cfg!(target_arch = "aarch64") {
                183
            } else {
                62
            }
        );
        let header = fs::read(bundle.join("oxid_library.h")).unwrap();
        let metadata: Value =
            serde_json::from_slice(&fs::read(bundle.join("metadata.json")).unwrap()).unwrap();
        assert_eq!(metadata["schema"], "oxid-scalar-native-library-v1");
        assert_eq!(metadata["abi_version"], 1);
        assert_eq!(metadata["export"], "crate::line_total");
        assert_eq!(metadata["symbol"], "oxid_library_call_v1");
        assert_eq!(metadata["parameters"], json!(["i32", "i32", "i32"]));
        assert_eq!(metadata["result"], "i32");
        assert_eq!(
            metadata["statuses"],
            json!({"success":0,"overflow":1,"division_by_zero":2,"fuel":3,"invalid_output":4})
        );
        assert_eq!(metadata["profile"], profile);
        assert_eq!(metadata["execution_policy"], "bounded");
        assert_eq!(metadata["fuel"], fuel);
        assert_eq!(
            metadata["target"],
            if cfg!(target_arch = "aarch64") {
                "aarch64-unknown-linux-gnu"
            } else {
                "x86_64-unknown-linux-gnu"
            }
        );
        assert_eq!(metadata["library_sha256"], hash(&library));
        assert_eq!(metadata["header_sha256"], hash(&header));
        let sources = metadata["sources"].as_array().unwrap();
        assert_eq!(sources.len(), 1);
        assert!(sources[0]["path"].as_str().unwrap().ends_with("input.ox"));
        assert_eq!(sources[0]["sha256"], hash(PRICING.as_bytes()));

        let defined = success(captured(
            fixture,
            "defined-symbols",
            Command::new(llvm.join("llvm-nm"))
                .args([
                    "--defined-only",
                    "--extern-only",
                    "--dynamic",
                    "--format=posix",
                ])
                .arg(bundle.join("liboxid_library.so")),
        ));
        let text = String::from_utf8(defined.stdout).unwrap();
        let mut functions: Vec<_> = text
            .lines()
            .filter_map(|line| {
                let mut words = line.split_whitespace();
                let name = words.next()?;
                let kind = words.next()?;
                (kind != "A").then(|| name.split('@').next().unwrap().to_owned())
            })
            .collect();
        functions.sort();
        assert_eq!(
            functions,
            ["oxid_library_abi_version", "oxid_library_call_v1"]
        );
        let undefined = success(captured(
            fixture,
            "undefined-symbols",
            Command::new(llvm.join("llvm-nm"))
                .args(["--undefined-only", "--dynamic", "--format=posix"])
                .arg(bundle.join("liboxid_library.so")),
        ));
        let undefined = String::from_utf8(undefined.stdout).unwrap();
        for line in undefined.lines() {
            let name = line
                .split_whitespace()
                .next()
                .unwrap()
                .split('@')
                .next()
                .unwrap();
            assert!(
                ![
                    "exit",
                    "_exit",
                    "ExitProcess",
                    "abort",
                    "longjmp",
                    "_longjmp",
                    "malloc",
                    "free",
                    "signal",
                    "sigaction"
                ]
                .contains(&name),
                "forbidden library dependency: {line}"
            );
        }
        assert!(
            undefined.trim().is_empty(),
            "scalar-only -nostdlib library imported symbols: {undefined}"
        );
        names
            .into_iter()
            .map(|name| {
                let bytes = fs::read(bundle.join(&name)).unwrap();
                (name.to_string_lossy().into_owned(), bytes)
            })
            .collect()
    }

    #[test]
    #[ignore = "serialized real Linux LLVM 19.1.7, rustc and Python native-call qualification; set OXID_LLVM_BIN"]
    fn actual_same_library_is_called_by_c_rust_python_with_returning_errors_and_concurrent_calls() {
        let llvm = PathBuf::from(
            std::env::var_os("OXID_TEST_LLVM_BIN")
                .or_else(|| std::env::var_os("OXID_LLVM_BIN"))
                .expect("set OXID_LLVM_BIN to the real LLVM 19.1.7 bin directory"),
        );
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let python = std::env::var_os("OXID_TEST_PYTHON").unwrap_or_else(|| "python3".into());
        for profile in ["dev", "release"] {
            for fuel_only in [false, true] {
                let fixture = Fixture::new(PRICING);
                let fuel = if fuel_only { 0 } else { 1_000_000 };
                let bundle = fixture.0.join("published-v1");
                let built = success(captured(
                    &fixture,
                    "library-build",
                    fixture
                        .command("crate::line_total", "published-v1")
                        .env("OXID_LLVM_BIN", &llvm)
                        .args(["--profile", profile, "--fuel", &fuel.to_string()]),
                ));
                assert!(built.stderr.is_empty(), "{built:?}");
                let summaries: Vec<Value> = String::from_utf8(built.stdout)
                    .unwrap()
                    .lines()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect();
                let summary = summaries.last().unwrap();
                assert_eq!(summary["kind"], "compile-summary");
                assert_eq!(summary["success"], true);
                assert_eq!(summary["output"], "published-v1");
                let frozen = verify_set(&fixture, &bundle, profile, fuel, &llvm);
                let repeated = fixture
                    .command("crate::line_total", "published-v1")
                    .env("OXID_LLVM_BIN", fixture.0.join("absent-tools"))
                    .output()
                    .unwrap();
                assert!(failure(repeated, "E0701").contains("already exists"));
                let refused_version = fixture
                    .command("crate::line_total", "refused-v2")
                    .env("OXID_LLVM_BIN", fixture.0.join("absent-tools"))
                    .output()
                    .unwrap();
                failure(refused_version, "E0701");
                assert!(!fixture.0.join("refused-v2").exists());
                for (name, bytes) in &frozen {
                    assert_eq!(fs::read(bundle.join(name)).unwrap(), *bytes);
                }

                fs::write(
                    fixture.0.join("client.c"),
                    include_bytes!("native_library_c.c"),
                )
                .unwrap();
                fs::write(
                    fixture.0.join("client.rs"),
                    include_bytes!("fixtures/native_library/client.rs"),
                )
                .unwrap();
                fs::write(
                    fixture.0.join("client.py"),
                    include_bytes!("native_library_python.py"),
                )
                .unwrap();
                let rpath = format!("-Wl,-rpath,{}", bundle.display());
                success(captured(
                    &fixture,
                    "c-build",
                    Command::new(llvm.join("clang"))
                        .args([
                            "--no-default-config",
                            "-std=c11",
                            "-Wall",
                            "-Wextra",
                            "-Werror",
                            "-O0",
                        ])
                        .arg("-I")
                        .arg(&bundle)
                        .arg(fixture.0.join("client.c"))
                        .arg("-L")
                        .arg(&bundle)
                        .args(["-loxid_library", "-ldl", "-pthread", &rpath])
                        .arg("-o")
                        .arg(fixture.0.join("c-client")),
                ));
                success(captured(
                    &fixture,
                    "rust-build",
                    Command::new(&rustc)
                        .args(["--edition=2021"])
                        .arg(fixture.0.join("client.rs"))
                        .arg(format!("-Lnative={}", bundle.display()))
                        .args(["-C", &format!("link-arg={rpath}")])
                        .arg("-o")
                        .arg(fixture.0.join("rust-client")),
                ));
                let independent = fixture.0.join("independent.so");
                fs::copy(bundle.join("liboxid_library.so"), &independent).unwrap();
                assert_eq!(
                    hash(&fs::read(&independent).unwrap()),
                    hash(&fs::read(bundle.join("liboxid_library.so")).unwrap())
                );
                fs::rename(
                    fixture.0.join("input.ox"),
                    fixture.0.join("removed-source.ox"),
                )
                .unwrap();
                fs::remove_file(fixture.0.join("removed-source.ox")).unwrap();
                let mode = if fuel_only { "fuel" } else { "pricing" };
                client(
                    &fixture,
                    "c-calls",
                    Command::new(fixture.0.join("c-client"))
                        .arg(&independent)
                        .arg(mode),
                    b"native C calls passed\n",
                );
                client(
                    &fixture,
                    "rust-calls",
                    Command::new(fixture.0.join("rust-client")).arg(mode),
                    b"native Rust calls passed\n",
                );
                client(
                    &fixture,
                    "python-calls",
                    Command::new(&python)
                        .arg(fixture.0.join("client.py"))
                        .arg(bundle.join("liboxid_library.so"))
                        .arg(mode)
                        .env_remove("PYTHONOPTIMIZE"),
                    b"native Python calls passed\n",
                );
                for (name, bytes) in &frozen {
                    assert_eq!(fs::read(bundle.join(name)).unwrap(), *bytes);
                }
                fs::write(fixture.0.join("result.json"), serde_json::to_vec_pretty(&json!({
                    "schema": "oxid-native-library-client-result-v1",
                    "status": "passed",
                    "profile": profile,
                    "fuel": fuel,
                    "library_sha256": hash(&fs::read(bundle.join("liboxid_library.so")).unwrap()),
                    "clients": ["C", "Rust", "Python"],
                    "source_removed": true
                })).unwrap()).unwrap();
            }
        }
        // This independent scalar OIR costs activation (1 + one local),
        // one literal assignment and one return: exactly 2 + 1 + 1 = 4.
        // The private unit control separately checks that lowering structure.
        const ANSWER: &str = "pub fn answer()->i32{return 42;}";
        const ANSWER_CLIENT: &str = r#"
import ctypes
import sys
if sys.flags.optimize:
    raise RuntimeError("exact fuel assertions require Python without -O/-OO")
library = ctypes.CDLL(sys.argv[1])
library.oxid_library_abi_version.argtypes = []
library.oxid_library_abi_version.restype = ctypes.c_uint32
assert library.oxid_library_abi_version() == 1
call = library.oxid_library_call_v1
call.argtypes = [ctypes.POINTER(ctypes.c_int32)]
call.restype = ctypes.c_int32
expected_status, expected_value = int(sys.argv[2]), int(sys.argv[3])
for _ in range(32):
    output = ctypes.c_int32(123456789)
    assert call(ctypes.byref(output)) == expected_status
    assert output.value == expected_value
print("native exact fuel calls passed")
"#;
        for (fuel, expected_status, expected_value) in [(4_u64, 0_i32, 42_i32), (3, 3, 123456789)] {
            let fixture = Fixture::new(ANSWER);
            let bundle = fixture.0.join("published-v1");
            let built = success(captured(
                &fixture,
                "exact-fuel-build",
                fixture
                    .command("crate::answer", "published-v1")
                    .env("OXID_LLVM_BIN", &llvm)
                    .args(["--fuel", &fuel.to_string()]),
            ));
            assert!(built.stderr.is_empty(), "{built:?}");
            let metadata: Value =
                serde_json::from_slice(&fs::read(bundle.join("metadata.json")).unwrap()).unwrap();
            assert_eq!(metadata["export"], "crate::answer");
            assert_eq!(metadata["parameters"], json!([]));
            assert_eq!(metadata["profile"], "dev");
            assert_eq!(metadata["fuel"], fuel);
            let library = fs::read(bundle.join("liboxid_library.so")).unwrap();
            assert_eq!(metadata["library_sha256"], hash(&library));
            fs::write(fixture.0.join("exact_fuel.py"), ANSWER_CLIENT).unwrap();
            fs::remove_file(fixture.0.join("input.ox")).unwrap();
            client(
                &fixture,
                "exact-fuel-calls",
                Command::new(&python)
                    .arg(fixture.0.join("exact_fuel.py"))
                    .arg(bundle.join("liboxid_library.so"))
                    .arg(expected_status.to_string())
                    .arg(expected_value.to_string())
                    .env_remove("PYTHONOPTIMIZE"),
                b"native exact fuel calls passed\n",
            );
            assert_eq!(
                fs::read(bundle.join("liboxid_library.so")).unwrap(),
                library
            );
            fs::write(
                fixture.0.join("result.json"),
                serde_json::to_vec_pretty(&json!({
                    "schema": "oxid-native-library-exact-fuel-result-v1",
                    "status": "passed",
                    "fuel": fuel,
                    "expected_status": expected_status,
                    "expected_value": expected_value,
                    "library_sha256": hash(&library),
                    "source_removed": true
                }))
                .unwrap(),
            )
            .unwrap();
        }
        // Separate bodies deliberately reuse the same source/helper names and
        // numeric function order. RTLD_GLOBAL must not merge their behavior.
        let fixture = Fixture::new("");
        let mut distinct = Vec::new();
        for expected in [42, 43] {
            let source = format!("fn leaf()->i32{{return {expected};}}fn middle()->i32{{return leaf();}}pub fn line_total(a:i32,b:i32,c:i32)->i32{{return middle();}}");
            fs::write(fixture.0.join("input.ox"), source).unwrap();
            let directory = format!("version-{expected}");
            let built = success(captured(
                &fixture,
                &format!("interposition-build-{expected}"),
                fixture
                    .command("crate::line_total", &directory)
                    .env("OXID_LLVM_BIN", &llvm),
            ));
            assert!(built.stderr.is_empty(), "{built:?}");
            let path = fixture.0.join(&directory).join("liboxid_library.so");
            let bytes = fs::read(&path).unwrap();
            distinct.push((path, bytes));
        }
        assert_ne!(hash(&distinct[0].1), hash(&distinct[1].1));
        const DISTINCT_CLIENT: &str = r#"
import concurrent.futures
import ctypes
import os
import sys
import threading
if sys.flags.optimize:
    raise RuntimeError("interposition assertions require Python without -O/-OO")
libraries = [ctypes.CDLL(path, mode=os.RTLD_NOW | os.RTLD_GLOBAL) for path in sys.argv[1:]]
functions = []
for library, expected in zip(libraries, (42, 43)):
    library.oxid_library_abi_version.argtypes = []
    library.oxid_library_abi_version.restype = ctypes.c_uint32
    assert library.oxid_library_abi_version() == 1
    call = library.oxid_library_call_v1
    call.argtypes = [ctypes.c_int32, ctypes.c_int32, ctypes.c_int32, ctypes.POINTER(ctypes.c_int32)]
    call.restype = ctypes.c_int32
    functions.append((call, expected))
assert len(functions) == 2
barrier = threading.Barrier(4)
def check():
    for call, expected in functions:
        output = ctypes.c_int32(123456789)
        assert call(1, 2, 3, ctypes.byref(output)) == 0
        assert output.value == expected
def worker(_):
    barrier.wait()
    for _ in range(128):
        check()
check()
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
    list(executor.map(worker, range(4)))
check()
print("native distinct libraries passed")
"#;
        fs::write(fixture.0.join("distinct.py"), DISTINCT_CLIENT).unwrap();
        fs::remove_file(fixture.0.join("input.ox")).unwrap();
        client(
            &fixture,
            "interposition-calls",
            Command::new(&python)
                .arg(fixture.0.join("distinct.py"))
                .arg(&distinct[0].0)
                .arg(&distinct[1].0)
                .env_remove("PYTHONOPTIMIZE"),
            b"native distinct libraries passed\n",
        );
        for (path, bytes) in &distinct {
            assert_eq!(fs::read(path).unwrap(), *bytes);
        }
        fs::write(
            fixture.0.join("result.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": "oxid-native-library-interposition-result-v1",
                "status": "passed",
                "expected_values": [42, 43],
                "library_sha256": [hash(&distinct[0].1), hash(&distinct[1].1)],
                "loader_mode": "RTLD_NOW|RTLD_GLOBAL",
                "source_removed": true
            }))
            .unwrap(),
        )
        .unwrap();

        // Distinct 1..=64 inputs cross both register/stack boundaries. The
        // nested helper also adds the private fuel argument before all 64
        // source arguments. Expected result: 1 + 8*2 + 9*3 + 32*5 + 64*7 = 652.
        let parameters = (0..64)
            .map(|index| format!("p{index}:i32"))
            .collect::<Vec<_>>()
            .join(",");
        let arguments = (0..64)
            .map(|index| format!("p{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let maximum_arity = format!("fn weighted64({parameters})->i32{{return p0+p7*2+p8*3+p31*5+p63*7;}}pub fn line_total({parameters})->i32{{return weighted64({arguments});}}");
        const ARITY_C: &str = r#"
#include "oxid_library.h"
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#define FIRST_63 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16, \
    17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32, \
    33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48, \
    49,50,51,52,53,54,55,56,57,58,59,60,61,62,63
#define ALL_64 FIRST_63,64
int main(void) {
    if (oxid_library_abi_version() != 1) return 1;
    for (unsigned repeat = 0; repeat < 32; ++repeat) {
        int32_t output = INT32_C(123456789);
        if (oxid_library_call_v1(ALL_64, &output) != 0 || output != 652) return 2;
        output = INT32_C(123456789);
        if (oxid_library_call_v1(FIRST_63, INT32_MAX, &output) != 1
            || output != INT32_C(123456789)) return 3;
        if (oxid_library_call_v1(ALL_64, NULL) != 4) return 4;
    }
    puts("native C 64-argument calls passed");
    return 0;
}
"#;
        const ARITY_PYTHON: &str = r#"
import ctypes
import sys
if sys.flags.optimize:
    raise RuntimeError("maximum arity assertions require Python without -O/-OO")
library = ctypes.CDLL(sys.argv[1])
library.oxid_library_abi_version.argtypes = []
library.oxid_library_abi_version.restype = ctypes.c_uint32
assert library.oxid_library_abi_version() == 1
call = library.oxid_library_call_v1
call.argtypes = [ctypes.c_int32] * 64 + [ctypes.POINTER(ctypes.c_int32)]
call.restype = ctypes.c_int32
values = list(range(1, 65))
overflow = values[:-1] + [2147483647]
for _ in range(32):
    output = ctypes.c_int32(123456789)
    assert call(*(values + [ctypes.byref(output)])) == 0
    assert output.value == 652
    output.value = 123456789
    assert call(*(overflow + [ctypes.byref(output)])) == 1
    assert output.value == 123456789
    assert call(*(values + [None])) == 4
print("native Python 64-argument calls passed")
"#;
        for profile in ["dev", "release"] {
            let fixture = Fixture::new(&maximum_arity);
            let bundle = fixture.0.join("published-v1");
            let built = success(captured(
                &fixture,
                "arity64-build",
                fixture
                    .command("crate::line_total", "published-v1")
                    .env("OXID_LLVM_BIN", &llvm)
                    .args(["--profile", profile]),
            ));
            assert!(built.stderr.is_empty(), "{built:?}");
            let metadata: Value =
                serde_json::from_slice(&fs::read(bundle.join("metadata.json")).unwrap()).unwrap();
            assert_eq!(metadata["export"], "crate::line_total");
            assert_eq!(metadata["profile"], profile);
            let parameters = metadata["parameters"].as_array().unwrap();
            assert_eq!(parameters.len(), 64);
            assert!(parameters.iter().all(|parameter| parameter == "i32"));
            let library = fs::read(bundle.join("liboxid_library.so")).unwrap();
            let header = fs::read(bundle.join("oxid_library.h")).unwrap();
            assert_eq!(metadata["library_sha256"], hash(&library));
            assert_eq!(metadata["header_sha256"], hash(&header));
            let sources = metadata["sources"].as_array().unwrap();
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0]["sha256"], hash(maximum_arity.as_bytes()));
            fs::write(fixture.0.join("arity64.c"), ARITY_C).unwrap();
            fs::write(fixture.0.join("arity64.py"), ARITY_PYTHON).unwrap();
            let rpath = format!("-Wl,-rpath,{}", bundle.display());
            success(captured(
                &fixture,
                "arity64-c-build",
                Command::new(llvm.join("clang"))
                    .args([
                        "--no-default-config",
                        "-std=c11",
                        "-Wall",
                        "-Wextra",
                        "-Werror",
                        "-O0",
                    ])
                    .arg("-I")
                    .arg(&bundle)
                    .arg(fixture.0.join("arity64.c"))
                    .arg("-L")
                    .arg(&bundle)
                    .args(["-loxid_library", &rpath])
                    .arg("-o")
                    .arg(fixture.0.join("arity64-c-client")),
            ));
            fs::remove_file(fixture.0.join("input.ox")).unwrap();
            client(
                &fixture,
                "arity64-c-calls",
                &mut Command::new(fixture.0.join("arity64-c-client")),
                b"native C 64-argument calls passed\n",
            );
            client(
                &fixture,
                "arity64-python-calls",
                Command::new(&python)
                    .arg(fixture.0.join("arity64.py"))
                    .arg(bundle.join("liboxid_library.so"))
                    .env_remove("PYTHONOPTIMIZE"),
                b"native Python 64-argument calls passed\n",
            );
            assert_eq!(
                fs::read(bundle.join("liboxid_library.so")).unwrap(),
                library
            );
            assert_eq!(fs::read(bundle.join("oxid_library.h")).unwrap(), header);
            fs::write(
                fixture.0.join("result.json"),
                serde_json::to_vec_pretty(&json!({
                    "schema": "oxid-native-library-maximum-arity-result-v1",
                    "status": "passed",
                    "profile": profile,
                    "parameters": 64,
                    "expected_value": 652,
                    "library_sha256": hash(&library),
                    "clients": ["C", "Python"],
                    "source_removed": true
                }))
                .unwrap(),
            )
            .unwrap();
        }
    }
}
