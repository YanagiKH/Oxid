//! Focused acceptance for the `oxi32sort` Process CLI workspace in
//! `examples/projects/oxi32sort` (member `oxi32sort`), a client of the direct
//! path dependency `packages/i32_vec` (alias `vec`). The app reads stdin to EOF,
//! converts the complete input with `from_le_bytes`, insertion-sorts the
//! little-endian signed i32 records with `get`/`set`/`len`, and writes the
//! result consumed by `into_le_bytes`. A tail that is not a whole record exits
//! 65 with empty stdout. Stderr stays empty for every fixed case.
//!
//! The fixed binary oracle lives in `examples/projects/oxi32sort/cases`; every
//! file is pinned here by length and SHA-256, and each expected output is also
//! re-derived independently in Rust. Published sources are copied into a fresh
//! repo-shaped temp tree (relative package path preserved); the in-tree
//! workspace is never built. Interpreter coverage runs `oxid run` directly.
//! Native coverage builds and packages, removes the source copy, then runs only
//! the deployed `bin/oxi32sort` through the existing `scripts/cli_driver.py`
//! (segmented stdin, close EOF, cwd=/, empty env). Run the ignored native test
//! explicitly with:
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test owned_i32_vec_package -- --include-ignored`.
//! Raw records go to a temp evidence directory removed on success; set
//! `OXID_OXI32SORT_EVIDENCE=<absent dir>` to keep them. Exit codes 70, 71 and
//! 74 are documented, not forced. No capacity, fuel, fault-injection or
//! qualification probes. The driver framework is not rewritten.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const PROJECT: &str = "examples/projects/oxi32sort";
const PACKAGE_DIR: &str = "packages/i32_vec";
const APP_SOURCES: [&str; 3] = [
    "examples/projects/oxi32sort/oxid.toml",
    "examples/projects/oxi32sort/app/oxid.toml",
    "examples/projects/oxi32sort/app/src/main.ox",
];
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

struct Case {
    name: &'static str,
    exit: i32,
    input_bytes: usize,
    input_sha256: &'static str,
    expected_bytes: usize,
    expected_sha256: &'static str,
}

const CASES: [Case; 9] = [
    Case {
        name: "empty",
        exit: 0,
        input_bytes: 0,
        input_sha256: EMPTY_SHA256,
        expected_bytes: 0,
        expected_sha256: EMPTY_SHA256,
    },
    Case {
        name: "min-singleton",
        exit: 0,
        input_bytes: 4,
        input_sha256: "6d58692645c9d1cfaf13541cbd258f86193ef63c2f1d38f6bbca9617372d7bd6",
        expected_bytes: 4,
        expected_sha256: "6d58692645c9d1cfaf13541cbd258f86193ef63c2f1d38f6bbca9617372d7bd6",
    },
    Case {
        name: "max-singleton",
        exit: 0,
        input_bytes: 4,
        input_sha256: "a2c70538651a7e9296b097e8c3dfc1b195a945802ffe45aa471868fba6f1042e",
        expected_bytes: 4,
        expected_sha256: "a2c70538651a7e9296b097e8c3dfc1b195a945802ffe45aa471868fba6f1042e",
    },
    Case {
        name: "mixed-duplicates",
        exit: 0,
        input_bytes: 56,
        input_sha256: "405f3890255934bf23ca077319cb897ebb7065594252edbb2f588a926b32a64f",
        expected_bytes: 56,
        expected_sha256: "823e0cb6738c1686eda364ebd79b25c6c6c3e8a135d8c518f04124a54c78daea",
    },
    Case {
        name: "short-reverse",
        exit: 0,
        input_bytes: 24,
        input_sha256: "d3b582ed00875f47aa9988330cd92153429c956032ffffb2a636d0e0c2eed617",
        expected_bytes: 24,
        expected_sha256: "ae866ee4229268ff9505d0933e35829e0fba3fdff973a7ec0980e81318ff0023",
    },
    Case {
        name: "tail-1",
        exit: 65,
        input_bytes: 13,
        input_sha256: "bffd26ef7af6ddd1070840822ebad578b6411fee34fb3beb52e99487165448a0",
        expected_bytes: 0,
        expected_sha256: EMPTY_SHA256,
    },
    Case {
        name: "tail-2",
        exit: 65,
        input_bytes: 14,
        input_sha256: "8b710d9d7704ad68e8848ed243cc2a01e124deda3ee83877282ddd5241c0b64c",
        expected_bytes: 0,
        expected_sha256: EMPTY_SHA256,
    },
    Case {
        name: "tail-3",
        exit: 65,
        input_bytes: 15,
        input_sha256: "2b62b65922238df6d735545c5a466f9508368087aa026fdaefda04b97c21152b",
        expected_bytes: 0,
        expected_sha256: EMPTY_SHA256,
    },
    Case {
        name: "sorted-259",
        exit: 0,
        input_bytes: 1036,
        input_sha256: "65393136cc3cfc5dbf706f4d0bba429a8bbfc080bc41ed56b26d5aa6dc1996fe",
        expected_bytes: 1036,
        expected_sha256: "65393136cc3cfc5dbf706f4d0bba429a8bbfc080bc41ed56b26d5aa6dc1996fe",
    },
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sha(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Loads the fixed oracle, checks its pins, and re-derives every expected
/// output: whole records sort ascending as signed i32; a partial tail is empty.
fn oracle() -> Vec<(&'static Case, Vec<u8>, Vec<u8>)> {
    let dir = root().join(PROJECT).join("cases");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    let mut wanted: Vec<String> = CASES
        .iter()
        .flat_map(|c| {
            [
                format!("{}.expected.bin", c.name),
                format!("{}.input.bin", c.name),
            ]
        })
        .collect();
    wanted.sort();
    assert_eq!(names, wanted, "fixed oracle file set");
    CASES
        .iter()
        .map(|case| {
            let input = fs::read(dir.join(format!("{}.input.bin", case.name))).unwrap();
            let expected = fs::read(dir.join(format!("{}.expected.bin", case.name))).unwrap();
            assert_eq!(input.len(), case.input_bytes, "{}", case.name);
            assert_eq!(sha(&input), case.input_sha256, "{}", case.name);
            assert_eq!(expected.len(), case.expected_bytes, "{}", case.name);
            assert_eq!(sha(&expected), case.expected_sha256, "{}", case.name);
            if case.exit == 0 {
                assert_eq!(input.len() % 4, 0, "{}", case.name);
                let mut values: Vec<i32> = input
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|r| i32::from_le_bytes(*r))
                    .collect();
                values.sort();
                let derived: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
                assert_eq!(derived, expected, "{}", case.name);
            } else {
                assert_eq!(case.exit, 65, "{}", case.name);
                assert_ne!(input.len() % 4, 0, "{}", case.name);
                assert!(expected.is_empty(), "{}", case.name);
            }
            (case, input, expected)
        })
        .collect()
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    let mut entries: Vec<_> = fs::read_dir(from).unwrap().map(|e| e.unwrap()).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let kind = entry.file_type().unwrap();
        let target = to.join(entry.file_name());
        assert!(!kind.is_symlink(), "unexpected symlink {:?}", entry.path());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Repo-shaped copy: the three app files plus the published package tree.
fn materialize(destination: &Path) -> PathBuf {
    for path in APP_SOURCES {
        let target = destination.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(root().join(path), &target).unwrap();
    }
    let package = root().join(PACKAGE_DIR);
    assert!(
        package.join("oxid.toml").is_file(),
        "{} is required (the fixed i32_vec package bundle must be applied first)",
        package.display()
    );
    copy_tree(&package, &destination.join(PACKAGE_DIR));
    destination.join(PROJECT)
}

struct Evidence {
    dir: PathBuf,
    keep: bool,
    receipt: String,
}

impl Evidence {
    fn new(engine: &str) -> Self {
        let suffix = format!(
            "oxid-oxi32sort-{engine}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let (dir, keep) = match std::env::var_os("OXID_OXI32SORT_EVIDENCE") {
            Some(base) => (PathBuf::from(base).join(suffix), true),
            None => (std::env::temp_dir().join(suffix), false),
        };
        fs::create_dir_all(dir.parent().unwrap()).unwrap();
        fs::create_dir(&dir).unwrap();
        Evidence {
            dir,
            keep,
            receipt: String::new(),
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Keeps raw streams and appends one receipt line per command.
    fn record(&mut self, label: &str, command: &Command, input: &[u8], output: &Output) {
        fs::write(self.path(&format!("{label}.stdout")), &output.stdout).unwrap();
        fs::write(self.path(&format!("{label}.stderr")), &output.stderr).unwrap();
        let argv: Vec<String> = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|part| part.to_string_lossy().into_owned())
            .collect();
        self.receipt.push_str(&format!(
            "{label} exit={:?} stdin_bytes={} stdin_sha256={} stdout_bytes={} stdout_sha256={} stderr_bytes={} stderr_sha256={} cwd={:?} argv={argv:?}\n",
            output.status.code(),
            input.len(),
            sha(input),
            output.stdout.len(),
            sha(&output.stdout),
            output.stderr.len(),
            sha(&output.stderr),
            command.get_current_dir(),
        ));
        fs::write(self.path("receipt.txt"), &self.receipt).unwrap();
    }

    fn run(&mut self, label: &str, mut command: Command, input: &[u8]) -> Output {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let data = input.to_vec();
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&data);
        });
        let output = child.wait_with_output().unwrap();
        writer.join().unwrap();
        self.record(label, &command, input, &output);
        output
    }

    /// A tool step that must exit 0 with empty stderr.
    fn ok(&mut self, label: &str, command: Command) -> Vec<u8> {
        let output = self.run(label, command, b"");
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{label} evidence={} {output:?}",
            self.dir.display()
        );
        output.stdout
    }
}

fn acceptance(engine: &str) {
    let cases = oracle();
    let mut evidence = Evidence::new(engine);
    let fixture_root = evidence.path("fixture");
    let project = materialize(&fixture_root);
    let manifest = project.join("oxid.toml");
    let cache = evidence.path("cache");
    fs::create_dir(&cache).unwrap();
    let llvm = if engine == "native" {
        PathBuf::from(std::env::var_os("OXID_TEST_LLVM_BIN").expect(
            "native oxi32sort acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin directory)",
        ))
    } else {
        fixture_root.join("absent-llvm")
    };
    let oxid = |args: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(args)
            .current_dir(&fixture_root)
            .env("OXID_CACHE_DIR", &cache)
            .env("OXID_LLVM_BIN", &llvm)
            .env("OXID_TEST_LLVM_BIN", &llvm)
            .env("PATH", fixture_root.join("absent-path"))
            .env_remove("OXID_PATH");
        command
    };
    let manifest_arg = manifest.to_str().unwrap();
    let main_source = project.join("app/src/main.ox");

    let checked = evidence.ok(
        "check-oxi32sort",
        oxid(&[
            "check",
            "--manifest-path",
            manifest_arg,
            "--package=oxi32sort",
        ]),
    );
    let checked = String::from_utf8(checked).unwrap();
    assert!(checked.starts_with("typed-preview check ok ("), "{checked}");
    assert!(checked.ends_with(" functions; check only)\n"), "{checked}");
    let formatted = evidence.ok(
        "fmt-check-oxi32sort",
        oxid(&[
            "--edition=typed-preview",
            "fmt",
            "--check",
            main_source.to_str().unwrap(),
        ]),
    );
    assert!(formatted.is_empty(), "{formatted:?}");

    if engine == "native" {
        for tool in ["clang", "opt", "llvm-as", "ld.lld"] {
            let mut command = Command::new(llvm.join(tool));
            command.arg("--version");
            let version = evidence.ok(&format!("tool-{tool}"), command);
            let version = String::from_utf8_lossy(&version);
            assert!(version.contains("19.1.7"), "{tool}: {version}");
        }
        evidence.ok(
            "build-oxi32sort",
            oxid(&[
                "build",
                "--manifest-path",
                manifest_arg,
                "--package=oxi32sort",
                "--entry-mode=process",
            ]),
        );
        let archive = evidence.path("oxi32sort.tar");
        evidence.ok(
            "package-oxi32sort",
            oxid(&[
                "package",
                "--manifest-path",
                manifest_arg,
                "--package=oxi32sort",
                "--entry-mode=process",
                "--output",
                archive.to_str().unwrap(),
            ]),
        );
        let deploy = evidence.path("deploy");
        fs::create_dir(&deploy).unwrap();
        let mut extract = Command::new("tar");
        extract.arg("-xf").arg(&archive).arg("-C").arg(&deploy);
        evidence.ok("extract-oxi32sort", extract);
        let executable = deploy.join("bin/oxi32sort");
        assert!(executable.is_file(), "{}", executable.display());
        evidence.receipt.push_str(&format!(
            "archive_sha256={} native_sha256={}\n",
            sha(&fs::read(&archive).unwrap()),
            sha(&fs::read(&executable).unwrap())
        ));
        // Only the deployed artifact runs from here on: drop the source copy.
        fs::remove_dir_all(&fixture_root).unwrap();
        let empty_stderr = evidence.path("empty.stderr");
        fs::write(&empty_stderr, b"").unwrap();
        for (case, input, expected) in &cases {
            let input_file = evidence.path(&format!("input-{}.bin", case.name));
            let expected_file = evidence.path(&format!("expected-{}.bin", case.name));
            fs::write(&input_file, input).unwrap();
            fs::write(&expected_file, expected).unwrap();
            let out_dir = evidence.path(&format!("driver-{}", case.name));
            let mut command = Command::new("python3");
            command
                .arg(root().join("scripts/cli_driver.py"))
                .arg("--stdin-file")
                .arg(&input_file)
                .args(["--segments", "1,7,4096", "--timeout", "60", "--cwd", "/"])
                .arg("--env-clear")
                .arg("--out")
                .arg(&out_dir)
                .arg("--expect-exit")
                .arg(case.exit.to_string())
                .arg("--expect-stdout-file")
                .arg(&expected_file)
                .arg("--expect-stderr-file")
                .arg(&empty_stderr)
                .arg("--")
                .arg(&executable)
                .current_dir("/");
            let driven = evidence.run(&format!("driver-{}", case.name), command, b"");
            let report = fs::read_to_string(out_dir.join("result.json")).unwrap();
            assert!(
                driven.status.success() && driven.stderr.is_empty(),
                "{} {driven:?} {report}",
                case.name
            );
            for needle in [
                format!("\"exit_code\": {},", case.exit),
                "\"term_signal\": null,".to_string(),
                "\"timed_out\": false,".to_string(),
                "\"env_clear\": true,".to_string(),
                "\"cwd\": \"/\",".to_string(),
                "\"complete\": true,".to_string(),
                "\"closed\": true,".to_string(),
                "\"process_group_alive_final\": false,".to_string(),
                "\"exit\": true,".to_string(),
                "\"stdout\": true,".to_string(),
                "\"stderr\": true".to_string(),
                "\"record_ok\": true,".to_string(),
                "\"reject_reasons\": [],".to_string(),
            ] {
                assert!(
                    report.contains(&needle),
                    "{} missing {needle}: {report}",
                    case.name
                );
            }
            assert_eq!(
                &fs::read(out_dir.join("stdin.bin")).unwrap(),
                input,
                "{}",
                case.name
            );
            assert_eq!(
                &fs::read(out_dir.join("stdout.bin")).unwrap(),
                expected,
                "{}",
                case.name
            );
            assert!(
                fs::read(out_dir.join("stderr.bin")).unwrap().is_empty(),
                "{}",
                case.name
            );
            let line = format!(
                "{engine} {} bytes={} exit={} sha256={}\n",
                case.name,
                input.len(),
                case.exit,
                sha(input)
            );
            print!("{line}");
            evidence.receipt.push_str(&line);
        }
    } else {
        for (case, input, expected) in &cases {
            let output = evidence.run(
                &format!("run-{}", case.name),
                oxid(&[
                    "run",
                    "--manifest-path",
                    manifest_arg,
                    "--package=oxi32sort",
                    "--engine=interpreter",
                    "--entry-mode=process",
                ]),
                input,
            );
            assert_eq!(
                output.status.code(),
                Some(case.exit),
                "{} {output:?}",
                case.name
            );
            assert_eq!(&output.stdout, expected, "{}", case.name);
            assert!(output.stderr.is_empty(), "{} {output:?}", case.name);
            let line = format!(
                "{engine} {} bytes={} exit={} sha256={}\n",
                case.name,
                input.len(),
                case.exit,
                sha(input)
            );
            print!("{line}");
            evidence.receipt.push_str(&line);
        }
    }
    fs::write(evidence.path("receipt.txt"), &evidence.receipt).unwrap();
    if !evidence.keep {
        let _ = fs::remove_dir_all(&evidence.dir);
    }
}

#[test]
fn owned_i32_vec_package_interpreter_acceptance() {
    acceptance("interpreter");
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN; no installation or fallback"]
fn owned_i32_vec_package_native_packaged_acceptance() {
    acceptance("native");
}
