//! Independent native `--profile dev|release` acceptance (task
//! r1-native-profile-21fa59b0; companion to root's compiler bundle). Tests
//! marked `#[ignore]` need LLVM 19.1.7: run them with
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test
//! manifest_native_profile_acceptance -- --include-ignored`. Opting in without
//! that env var fails the LLVM gate; a missing tool is never a pass.
//!
//! Every expected output below is hand-written from miku's frozen contract
//! (ts 1792450065.357839, oracle supplement ts 1792450080.674959, bundle
//! supplement ts 1792450387.883899) and the client arithmetic documented in
//! the example README; none is generated from candidate output. Frozen here:
//! `--profile dev|release` and `--profile=dev|release`; omitted means dev/O0.
//! Release passes O2 to clang for both the LLVM IR and the C runtime and
//! publishes into `release-result`, `release-process` and
//! `release-test/case-ORDINAL`; dev paths and default bytes are unchanged.
//! Package metadata `native-preview-O0` / `native-preview-O2` follows the
//! retained profile. Scanner messages: "--profile requires a value",
//! "--profile may only be specified once", "unknown --profile `V`; expected
//! dev or release" in the global first-error tier; applicability ("--profile
//! is only supported by typed-preview source compile, manifest build,
//! package, native run or native test") after filter / fail-fast / engine
//! applicability; E0001 presentation and the PACKAGE failure summary kept.
//! Words after `--` or a script name are unchanged. No speed claim and no
//! qualification, held, capacity, fuel or oxcodepoints probes.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

type FileSnapshot = Option<(u64, u64, Vec<u8>)>;
/// A suite case: root test name and whether it passes.
type Case = (&'static str, bool);

static NEXT: AtomicUsize = AtomicUsize::new(0);
const EX: &str = "examples/projects/manifest_native_profile_acceptance";
const TEXT_PACKAGE: [&str; 2] = [
    "packages/utf8_text/oxid.toml",
    "packages/utf8_text/src/lib.ox",
];
const APP: &str = "npp-app";
const GATE: &str = "npp-gate";
/// Checked-in client sources pinned by length and SHA-256 (README is checked
/// by content below). A drift fails before any command runs.
const SOURCES: [(&str, usize, &str); 9] = [
    (
        "oxid.toml",
        47,
        "98c1a0524ac2c640a222c570f7f5f0e1da92c66b5477033d549a7cbfcc5fbd9a",
    ),
    (
        "app/oxid.toml",
        168,
        "247b7252713863415354c01599276d3a66dc20cf4b109f653cc2a17e21538581",
    ),
    (
        "app/src/main.ox",
        2207,
        "0d5680f3fc4145c12f3a9ec0990c5f64b5f760115ece06f6c5251c4c6423730d",
    ),
    (
        "model/oxid.toml",
        94,
        "37f70da1e1f6a1d48645d0e55532905ebcc4478eb97059cab098ebd730f6652b",
    ),
    (
        "model/src/lib.ox",
        147,
        "48c5f68f6a18eb53a4d630e06143e73854b1d82d72f200450a2ac0bc92b91ce7",
    ),
    (
        "model/src/shapes.ox",
        1442,
        "2d066bbe91ba84cf1d104fd4384e8127bf8dd789fa246ebb77acd8b2f4e9ae23",
    ),
    (
        "gate/oxid.toml",
        129,
        "28bf641159511473a7f2f142e1234580d1be63d5d5e1b9274d578758da4defaa",
    ),
    (
        "gate/src/main.ox",
        294,
        "88911b2b00c7e42d6b091a7b7ce4e732e2c33f2aacb205f619d2895a2d91f5c4",
    ),
    (
        "standalone/main.ox",
        1851,
        "35225572f483345256a47a3fbd6b2a2e93e4adc66c3d401c96a5ce0d943dfff1",
    ),
];
const MODEL_LIB: &str = "model/src/lib.ox";
const BASE_7: &str = "pub fn base() -> i32 {\n    return 7;\n}\n";
const BASE_8: &str = "pub fn base() -> i32 {\n    return 8;\n}\n";
/// Appended to the dependency to make every build of the app fail in checking.
const BROKEN: &str = "pub fn broken() -> i32 {\n    return true;\n}\n";

// ---- Hand-written oracle (literal bytes, written from the contract) ----

/// main() with base() == 7 (21 + 9 + 2 + 6 + 4), identical for dev and release.
const RESULT: &[u8] = b"42\n";
const RESULT_CODE: i32 = 42;
/// main() after the dependency changes base() to 8 (24 + 10 + 3 + 6 + 4).
const CHANGED: &[u8] = b"47\n";
const CHANGED_CODE: i32 = 47;
const APP_SUITE_CASES: [Case; 6] = [
    ("test_scalar_scale", true),
    ("test_record_pair", true),
    ("test_enum_outcomes", true),
    ("test_buffer_bytes", true),
    ("test_text_scalars", true),
    ("test_unit_tail", true),
];
const APP_SUITE: &[u8] = b"PASS test_scalar_scale\nPASS test_record_pair\nPASS test_enum_outcomes\nPASS test_buffer_bytes\nPASS test_text_scalars\nPASS test_unit_tail\ntest result: 6 passed; 0 failed; 6 total\n";
/// `--filter text` (with or without `--fail-fast`).
const APP_TEXT: &[u8] = b"PASS test_text_scalars\ntest result: 1 passed; 0 failed; 1 total\n";
/// The app suite after base() becomes 8.
const CHANGED_CASES: [Case; 6] = [
    ("test_scalar_scale", false),
    ("test_record_pair", false),
    ("test_enum_outcomes", false),
    ("test_buffer_bytes", true),
    ("test_text_scalars", true),
    ("test_unit_tail", true),
];
const CHANGED_SUITE: &[u8] = b"FAIL test_scalar_scale\nFAIL test_record_pair\nFAIL test_enum_outcomes\nPASS test_buffer_bytes\nPASS test_text_scalars\nPASS test_unit_tail\ntest result: 3 passed; 3 failed; 6 total\n";
const CHANGED_FAIL_FAST: &[u8] = b"FAIL test_scalar_scale\ntest result: 0 passed; 1 failed; 1 total\ntest stopped: 5 selected tests not run (--fail-fast)\n";
const GATE_CASES: [Case; 4] = [
    ("test_gate_first", true),
    ("test_gate_false", false),
    ("test_gate_divide", false),
    ("test_gate_after", true),
];
const GATE_SUITE: &[u8] = b"PASS test_gate_first\nFAIL test_gate_false\nFAIL test_gate_divide\nPASS test_gate_after\ntest result: 2 passed; 2 failed; 4 total\n";
const GATE_FAIL_FAST: &[u8] = b"PASS test_gate_first\nFAIL test_gate_false\ntest result: 1 passed; 1 failed; 2 total\ntest stopped: 2 selected tests not run (--fail-fast)\n";
/// `--filter divide [--fail-fast]`: the only (last) selected case has a
/// runtime diagnostic; no stopped line.
const GATE_DIVIDE: &[u8] = b"FAIL test_gate_divide\ntest result: 0 passed; 1 failed; 1 total\n";
/// `--filter gate_f --fail-fast`: the last selected case fails; no stopped line.
const GATE_F: &[u8] =
    b"PASS test_gate_first\nFAIL test_gate_false\ntest result: 1 passed; 1 failed; 2 total\n";
const GATE_AFTER: &[u8] = b"PASS test_gate_after\ntest result: 1 passed; 0 failed; 1 total\n";
/// Runtime diagnostic of test_gate_divide; checks stay on under O2.
const DIV_ZERO: &str = "E0607";

const MISSING_VALUE: &str = "--profile requires a value";
const DUPLICATE: &str = "--profile may only be specified once";
const ONLY: &str = "--profile is only supported by typed-preview source compile, manifest build, package, native run or native test";
const FILTER_ONLY_TEST: &str = "--filter is only supported by explicit manifest test";
const FAIL_FAST_ONLY_TEST: &str = "--fail-fast is only supported by explicit manifest test";
const ENGINE_ONLY: &str = "--engine is only supported by manifest run or test";
const NO_OVERWRITE: &str = "native output already exists; choose a new path (existing files and symlinks are never overwritten)";
/// A manifest path that never exists: refusals must precede reading it.
const MISSING: &str = "absent/oxid.toml";
/// Existing stdout line of every refused manifest package invocation.
const PACKAGE_FAILED: &[u8] =
    b"typed-preview native package failed: managed executable not published; archive not published\n";
/// Unchanged base form (same bytes as pinned with the base 2dcbc8a1 binary by
/// the fail-fast companion), from a directory without any manifest.
const LEGACY_SCRIPT: &[u8] =
    b"error: cannot read manifest: ./oxid.toml (No such file or directory (os error 2))\n";

fn unknown(value: &str) -> String {
    format!("unknown --profile `{value}`; expected dev or release")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Profile {
    Dev,
    Release,
}
impl Profile {
    fn result(self) -> &'static str {
        match self {
            Self::Dev => "result",
            Self::Release => "release-result",
        }
    }
    fn process(self) -> &'static str {
        match self {
            Self::Dev => "process",
            Self::Release => "release-process",
        }
    }
    fn test(self) -> &'static str {
        match self {
            Self::Dev => "test",
            Self::Release => "release-test",
        }
    }
    fn metadata(self) -> &'static str {
        match self {
            Self::Dev => "native-preview-O0",
            Self::Release => "native-preview-O2",
        }
    }
    fn optimization(self) -> &'static str {
        match self {
            Self::Dev => "-O0",
            Self::Release => "-O2",
        }
    }
}
/// Every accepted spelling; the empty form is the unchanged default (dev).
const FORMS: [(&[&str], Profile); 5] = [
    (&[], Profile::Dev),
    (&["--profile", "dev"], Profile::Dev),
    (&["--profile=dev"], Profile::Dev),
    (&["--profile", "release"], Profile::Release),
    (&["--profile=release"], Profile::Release),
];
/// Explicit spellings only (each must be refused where not applicable).
const EXPLICIT: [&[&str]; 4] = [
    &["--profile", "dev"],
    &["--profile=dev"],
    &["--profile", "release"],
    &["--profile=release"],
];

fn llvm_bin() -> PathBuf {
    let dir = PathBuf::from(
        std::env::var_os("OXID_TEST_LLVM_BIN")
            .expect("native profile acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin)"),
    );
    for (tool, marker) in [
        ("clang", "clang version 19.1.7"),
        ("opt", "LLVM version 19.1.7"),
        ("ld.lld", "LLD 19.1.7"),
    ] {
        let version = Command::new(dir.join(tool))
            .arg("--version")
            .output()
            .unwrap_or_else(|e| panic!("missing native tool {tool}: {e}"));
        assert!(
            String::from_utf8_lossy(&version.stdout).contains(marker),
            "{tool}: {version:?}"
        );
    }
    dir
}

struct Fixture {
    root: PathBuf,
    llvm: PathBuf,
}
impl Fixture {
    /// Copies the checked-in client to `<root>/EX` and the Text package to
    /// `<root>/packages/utf8_text` (relative dependency path preserved).
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-native-profile-accept-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
        };
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        for (file, length, digest) in SOURCES {
            let bytes = fs::read(repository.join(EX).join(file)).unwrap();
            assert_eq!(bytes.len(), length, "checked-in example {file} changed");
            assert_eq!(sha256(&bytes), digest, "checked-in example {file} changed");
            fixture.write(
                &format!("{EX}/{file}"),
                std::str::from_utf8(&bytes).unwrap(),
            );
        }
        for file in TEXT_PACKAGE {
            let text = fs::read_to_string(repository.join(file)).unwrap();
            fixture.write(file, &text);
        }
        fs::create_dir_all(fixture.path("caller")).unwrap();
        fs::create_dir_all(fixture.path("dist")).unwrap();
        fixture
    }
    fn native() -> Self {
        Self::new(Some(llvm_bin()))
    }
    fn path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
    fn ex(&self, path: &str) -> PathBuf {
        self.root.join(EX).join(path)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.path(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn edit_model(&self, from: &str, to: &str) {
        let path = self.ex(MODEL_LIB);
        let body = fs::read_to_string(&path).unwrap();
        assert_eq!(body.matches(from).count(), 1, "{body}");
        fs::write(path, body.replacen(from, to, 1)).unwrap();
    }
    fn command_in(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(args)
            .current_dir(cwd)
            .env("OXID_LLVM_BIN", &self.llvm)
            .env("OXID_CACHE_DIR", self.path("cache"))
            .env("PATH", self.path("absent-path"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null());
        command
    }
    fn oxid_in(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command_in(cwd, args).output().unwrap()
    }
    /// `oxid <args>` from the client root.
    fn oxid(&self, args: &[&str]) -> Output {
        self.oxid_in(&self.path(EX), args)
    }
    /// `oxid <command> --manifest-path <manifest> <extra...> <profile...>`.
    fn manifest(&self, command: &str, manifest: &str, extra: &[&str], profile: &[&str]) -> Output {
        let mut args = vec![command, "--manifest-path", manifest];
        args.extend_from_slice(extra);
        args.extend_from_slice(profile);
        self.oxid(&args)
    }
    fn build(&self, extra: &[&str], profile: &[&str]) -> Output {
        self.manifest("build", "app/oxid.toml", extra, profile)
    }
    fn native_run(&self, extra: &[&str], profile: &[&str]) -> Output {
        let mut args = vec!["--engine", "native"];
        args.extend_from_slice(extra);
        self.manifest("run", "app/oxid.toml", &args, profile)
    }
    fn native_test(&self, member: &str, extra: &[&str], profile: &[&str]) -> Output {
        let mut args = vec!["--engine", "native"];
        args.extend_from_slice(extra);
        self.manifest("test", &format!("{member}/oxid.toml"), &args, profile)
    }
    fn package(&self, output: &str, extra: &[&str], profile: &[&str]) -> Output {
        let mut args = vec!["--output", output];
        args.extend_from_slice(extra);
        self.manifest("package", "app/oxid.toml", &args, profile)
    }
    fn compile(&self, output: &str, profile: &[&str]) -> Output {
        let mut args = vec![
            "compile",
            "standalone/main.ox",
            "--edition",
            "typed-preview",
            "--backend",
            "llvm",
            "--output",
            output,
        ];
        args.extend_from_slice(profile);
        self.oxid(&args)
    }
    /// Managed artifact path, relative to the client root.
    fn managed(member: &str, namespace: &str, name: &str) -> String {
        format!("{member}/.oxid/typed-preview/{namespace}/{name}")
    }
    /// Artifact plus owner / current / next receipts of one namespace.
    fn state(&self, member: &str, namespace: &str, name: &str) -> [FileSnapshot; 4] {
        let artifact = self.ex(&Self::managed(member, namespace, name));
        let receipts = artifact.parent().unwrap().join(format!(".{name}.build"));
        [
            artifact,
            receipts.join("owner"),
            receipts.join("current"),
            receipts.join("next"),
        ]
        .map(|path| snapshot(&path))
    }
    fn app_states(&self) -> Vec<[FileSnapshot; 4]> {
        [
            Profile::Dev.result(),
            Profile::Release.result(),
            Profile::Dev.process(),
            Profile::Release.process(),
        ]
        .iter()
        .map(|namespace| self.state("app", namespace, APP))
        .collect()
    }
    fn test_dir(&self, member: &str, profile: Profile) -> PathBuf {
        self.ex(&format!("{member}/.oxid/typed-preview/{}", profile.test()))
    }
    /// Refusals and source commands write no managed output anywhere.
    fn no_managed_output(&self) {
        for dir in [
            "caller".to_string(),
            EX.to_string(),
            format!("{EX}/app"),
            format!("{EX}/model"),
            format!("{EX}/gate"),
            format!("{EX}/standalone"),
            "packages/utf8_text".to_string(),
        ] {
            assert!(!self.path(&dir).join(".oxid").exists(), "{dir}/.oxid");
            assert!(!self.path(&dir).join("oxid.lock").exists(), "{dir}");
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn snapshot(path: &Path) -> FileSnapshot {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            assert!(meta.file_type().is_file(), "{path:?}");
            Some((meta.dev(), meta.ino(), fs::read(path).unwrap()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("cannot snapshot {path:?}: {error}"),
    }
}
fn with(prefix: &[&'static str], rest: &[&'static str]) -> Vec<&'static str> {
    let mut args = prefix.to_vec();
    args.extend_from_slice(rest);
    args
}
/// Contract model, used ONLY to cross-check the literal oracle above.
fn model(cases: &[Case], fail_fast: bool) -> Vec<u8> {
    let mut out = String::new();
    let (mut passed, mut executed) = (0, 0);
    for (name, pass) in cases {
        executed += 1;
        passed += usize::from(*pass);
        out.push_str(&format!("{} {name}\n", if *pass { "PASS" } else { "FAIL" }));
        if fail_fast && !pass {
            break;
        }
    }
    out.push_str(&format!(
        "test result: {passed} passed; {} failed; {executed} total\n",
        executed - passed
    ));
    if executed < cases.len() {
        out.push_str(&format!(
            "test stopped: {} selected tests not run (--fail-fast)\n",
            cases.len() - executed
        ));
    }
    out.into_bytes()
}
fn select(cases: &[Case], filter: &str) -> Vec<Case> {
    cases
        .iter()
        .copied()
        .filter(|(name, _)| name.contains(filter))
        .collect()
}
/// The documented client arithmetic (README), for the oracle cross-check.
fn main_value(base: i32) -> i32 {
    let scalar = base * 3;
    let record = base + 2;
    let outcome = (base + 1) + -6;
    let buffer = 1 + 2 + 3;
    let text = (4 * 10 + 3) / 10;
    scalar + record + outcome + buffer + text
}

/// Exit 0, exact stdout, empty stderr.
fn ok(out: &Output, stdout: &[u8]) {
    assert_eq!(text(&out.stdout), text(stdout), "{out:?}");
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
}
fn build_ok(member: &str, namespace: &str, name: &str) -> Vec<u8> {
    format!(
        "typed-preview native build ok: \"{}\"\n",
        Fixture::managed(member, namespace, name)
    )
    .into_bytes()
}
/// A suite run: exact stdout, exit 0 iff no failure, and the division
/// diagnostic on stderr iff test_gate_divide ran; otherwise empty stderr.
fn suite(out: &Output, expected: &[u8]) {
    assert_eq!(text(&out.stdout), text(expected), "{out:?}");
    let failed = !text(expected).contains(" 0 failed;");
    assert_eq!(out.status.code(), Some(i32::from(failed)), "{out:?}");
    let divided = text(expected).contains(" test_gate_divide\n");
    let err = text(&out.stderr);
    assert_eq!(
        err.matches(DIV_ZERO).count(),
        usize::from(divided),
        "{out:?}"
    );
    if !divided {
        assert!(out.stderr.is_empty(), "{out:?}");
    }
}
/// A refusal on the existing CLI (E0001) text path before project I/O.
fn cli_refusal(out: &Output, operation: &str, message: &str) {
    assert_eq!(out.status.code(), Some(1), "{operation}: {out:?}");
    let stdout: &[u8] = if operation == "package" {
        PACKAGE_FAILED
    } else {
        b""
    };
    assert_eq!(text(&out.stdout), text(stdout), "{operation}: {out:?}");
    assert_eq!(
        text(&out.stderr),
        format!("error[E0001] (cli): {message}\n"),
        "{operation}: {out:?}"
    );
}
/// A failure after argv acceptance (toolchain, checking, publication): exit
/// 1, the expected diagnostic, no result line and no build summary.
fn failure(out: &Output, operation: &str, message: &str) {
    assert_eq!(out.status.code(), Some(1), "{operation}: {out:?}");
    let stdout: &[u8] = if operation == "package" {
        PACKAGE_FAILED
    } else {
        b""
    };
    assert_eq!(text(&out.stdout), text(stdout), "{operation}: {out:?}");
    assert!(text(&out.stderr).contains(message), "{operation}: {out:?}");
}
/// Runs an executable outside the source tree (cwd=/, empty env).
fn exec(artifact: &Path) -> Output {
    Command::new(artifact)
        .current_dir("/")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap()
}
fn exec_result(artifact: &Path, stdout: &[u8]) {
    ok(&exec(artifact), stdout);
}
fn exec_process(artifact: &Path, code: i32) {
    let out = exec(artifact);
    assert_eq!(out.status.code(), Some(code), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
}
fn assert_elf(bytes: &[u8]) {
    assert_eq!(&bytes[..4], b"\x7fELF");
    assert_eq!(&bytes[4..7], &[2, 1, 1]);
    assert_eq!(&bytes[16..20], &[3, 0, 62, 0]);
    assert!(bytes.len() > 4096, "{}", bytes.len());
}
fn case_states(dir: &Path, count: usize) -> Vec<[FileSnapshot; 2]> {
    (0..count)
        .map(|ordinal| {
            [
                snapshot(&dir.join(format!("case-{ordinal}"))),
                snapshot(&dir.join(format!(".case-{ordinal}.build/owner"))),
            ]
        })
        .collect()
}

// ---- Oracle self-consistency (no binary executed) ----

#[test]
fn literal_oracle_agrees_with_contract_model() {
    assert_eq!(format!("{}\n", main_value(7)).as_bytes(), RESULT);
    assert_eq!(main_value(7), RESULT_CODE);
    assert_eq!(format!("{}\n", main_value(8)).as_bytes(), CHANGED);
    assert_eq!(main_value(8), CHANGED_CODE);
    for fail_fast in [false, true] {
        assert_eq!(model(&APP_SUITE_CASES, fail_fast), APP_SUITE);
        assert_eq!(
            model(&select(&APP_SUITE_CASES, "text"), fail_fast),
            APP_TEXT
        );
        assert_eq!(model(&select(&GATE_CASES, "after"), fail_fast), GATE_AFTER);
        assert_eq!(
            model(&select(&GATE_CASES, "divide"), fail_fast),
            GATE_DIVIDE
        );
    }
    assert_eq!(model(&CHANGED_CASES, false), CHANGED_SUITE);
    assert_eq!(model(&CHANGED_CASES, true), CHANGED_FAIL_FAST);
    assert_eq!(model(&GATE_CASES, false), GATE_SUITE);
    assert_eq!(model(&GATE_CASES, true), GATE_FAIL_FAST);
    assert_eq!(model(&select(&GATE_CASES, "gate"), true), GATE_FAIL_FAST);
    assert_eq!(model(&select(&GATE_CASES, "gate_f"), true), GATE_F);
    // The unknown-value diagnostic carries exactly one ASCII backtick
    // (U+0060) on each side of the value.
    let message = unknown("bogus");
    assert_eq!(message.matches('\u{60}').count(), 2);
    assert!(message.contains("\u{60}bogus\u{60};"));
    // Release namespaces are distinct from every dev namespace.
    let dev = [
        Profile::Dev.result(),
        Profile::Dev.process(),
        Profile::Dev.test(),
    ];
    for namespace in [
        Profile::Release.result(),
        Profile::Release.process(),
        Profile::Release.test(),
    ] {
        assert!(!dev.contains(&namespace), "{namespace}");
    }
    for (index, (form, _)) in FORMS.iter().enumerate() {
        assert!(!FORMS[..index].iter().any(|(f, _)| f == form), "{form:?}");
    }
}

#[test]
fn example_readme_documents_frozen_contract() {
    let readme = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(EX)
            .join("README.md"),
    )
    .unwrap();
    for needle in [
        "--profile dev",
        "--profile=release",
        "release-result",
        "release-process",
        "release-test/case-ORDINAL",
        "native-preview-O0",
        "native-preview-O2",
        "scalar 21 + record 9 + enum 2 + owned buffer 6 + Text 4 = 42",
        "24 + 10 + 3 + 6 + 4 = 47",
        "test stopped: 2 selected tests not run (--fail-fast)",
        MISSING_VALUE,
        DUPLICATE,
        "unknown --profile `VALUE`; expected dev or release",
        ONLY,
        "LLVM 19.1.7",
        "OXID_TEST_LLVM_BIN",
    ] {
        assert!(readme.contains(needle), "{needle}");
    }
}

#[test]
fn opt_in_llvm_gate_fails_without_toolchain_env() {
    if std::env::var_os("OXID_TEST_LLVM_BIN").is_some() {
        let _ = llvm_bin();
        return;
    }
    let result = std::panic::catch_unwind(|| {
        let _ = llvm_bin();
    });
    assert!(result.is_err(), "llvm_bin must require OXID_TEST_LLVM_BIN");
}

// ---- CLI contracts (no toolchain required) ----

/// Manifest operations that accept a profile, with their required prefix.
const APPLICABLE: [(&str, &[&str]); 6] = [
    ("build", &[]),
    ("build", &["--entry-mode=process"]),
    ("run", &["--engine", "native"]),
    ("run", &["--engine=native", "--entry-mode", "process"]),
    ("test", &["--engine", "native"]),
    ("package", &["--output", "out.tar"]),
];
/// Manifest operations that never accept a profile (interpreter and check).
const NOT_APPLICABLE: [(&str, &[&str]); 5] = [
    ("check", &[]),
    ("run", &[]),
    ("run", &["--engine", "interpreter"]),
    ("test", &[]),
    ("test", &["--engine=interpreter"]),
];
/// Malformed profile forms with the frozen first-error message. The first 17
/// rows contain only profile options.
fn malformed() -> Vec<(Vec<&'static str>, String)> {
    vec![
        (vec!["--profile"], MISSING_VALUE.into()),
        (vec!["--profile="], MISSING_VALUE.into()),
        // A following dash-prefixed option is not consumed as the value.
        (vec!["--profile", "--package", APP], MISSING_VALUE.into()),
        (vec!["--profile", "--profile=dev"], MISSING_VALUE.into()),
        // Duplicates are never last-wins; on the repeated token itself the
        // duplicate precedes its value.
        (
            vec!["--profile", "dev", "--profile", "dev"],
            DUPLICATE.into(),
        ),
        (vec!["--profile=release", "--profile=dev"], DUPLICATE.into()),
        (vec!["--profile=dev", "--profile=release"], DUPLICATE.into()),
        (vec!["--profile=dev", "--profile"], DUPLICATE.into()),
        (vec!["--profile=dev", "--profile=bogus"], DUPLICATE.into()),
        (vec!["--profile=", "--profile=dev"], MISSING_VALUE.into()),
        (vec!["--profile=bogus", "--profile=dev"], unknown("bogus")),
        (vec!["--profile=bogus"], unknown("bogus")),
        (vec!["--profile", "Release"], unknown("Release")),
        (vec!["--profile=DEV"], unknown("DEV")),
        (vec!["--profile=O2"], unknown("O2")),
        (vec!["--profile=debug"], unknown("debug")),
        (vec!["--profile=dev,release"], unknown("dev,release")),
        // Global tier: the first global error wins in argv order.
        (
            vec!["--profile=bogus", "--message-format=yaml"],
            unknown("bogus"),
        ),
        (
            vec!["--message-format=yaml", "--profile=bogus"],
            "unknown --message-format `yaml`; expected text or json".into(),
        ),
        (
            vec!["--edition=bogus", "--profile="],
            "unknown edition `bogus`; expected legacy-0.9 or typed-preview".into(),
        ),
        (vec!["--profile=", "--edition=bogus"], MISSING_VALUE.into()),
        // ...ahead of manifest-local scanner errors, in either order.
        (vec!["--fail-fast=1", "--profile=bogus"], unknown("bogus")),
        (vec!["--profile=bogus", "--fail-fast=1"], unknown("bogus")),
        (vec!["--filter=", "--profile"], MISSING_VALUE.into()),
        (
            vec!["--output=a.tar", "--output=b.tar", "--profile=x"],
            unknown("x"),
        ),
    ]
}

#[test]
fn malformed_profile_is_refused_before_reading_any_project() {
    let f = Fixture::new(None);
    for (operation, prefix) in APPLICABLE.iter().chain(NOT_APPLICABLE.iter()) {
        for (form, message) in malformed() {
            let args = with(prefix, &form);
            cli_refusal(
                &f.manifest(operation, MISSING, &args, &[]),
                operation,
                &message,
            );
            // The same refusal for an existing project and for a workspace
            // without --package: nothing is read or listed.
            cli_refusal(
                &f.manifest(operation, "app/oxid.toml", &args, &[]),
                operation,
                &message,
            );
            cli_refusal(
                &f.manifest(operation, "oxid.toml", &args, &[]),
                operation,
                &message,
            );
        }
    }
    assert!(!f.path("absent").exists());
    for archive in ["out.tar", "a.tar", "b.tar"] {
        assert!(!f.ex(archive).exists(), "{archive}");
    }
    f.no_managed_output();
}

#[test]
fn explicit_profile_even_dev_is_refused_by_interpreter_and_check() {
    let f = Fixture::new(None);
    for (operation, prefix) in NOT_APPLICABLE {
        for profile in EXPLICIT {
            for manifest in [MISSING, "app/oxid.toml", "gate/oxid.toml"] {
                cli_refusal(
                    &f.manifest(operation, manifest, prefix, profile),
                    operation,
                    ONLY,
                );
                // Profile before the manifest path and before the command.
                let mut args = vec![operation];
                args.extend_from_slice(profile);
                args.extend_from_slice(&["--manifest-path", manifest]);
                args.extend_from_slice(prefix);
                cli_refusal(&f.oxid(&args), operation, ONLY);
                let mut args = profile.to_vec();
                args.extend_from_slice(&[operation, "--manifest-path", manifest]);
                args.extend_from_slice(prefix);
                cli_refusal(&f.oxid(&args), operation, ONLY);
            }
            // Workspace selection does not make it applicable.
            let ws = with(&["--package", APP], prefix);
            cli_refusal(
                &f.manifest(operation, "oxid.toml", &ws, profile),
                operation,
                ONLY,
            );
        }
    }
    // Interpreter TEST with its own valid filter / fail-fast.
    for profile in EXPLICIT {
        for extra in [
            &["--filter", "text"][..],
            &["--fail-fast"],
            &["--filter=gate", "--fail-fast", "--engine", "interpreter"],
        ] {
            cli_refusal(
                &f.manifest("test", "app/oxid.toml", extra, profile),
                "test",
                ONLY,
            );
        }
    }
    f.no_managed_output();
}

#[test]
fn profile_applicability_follows_filter_fail_fast_engine_and_existing_checks() {
    let f = Fixture::new(None);
    let operations: [(&str, &[&str]); 5] = [
        ("check", &[]),
        ("run", &[]),
        ("run", &["--engine", "native"]),
        ("build", &[]),
        ("package", &["--output", "out.tar"]),
    ];
    for (operation, prefix) in operations {
        for profile in EXPLICIT {
            for (extra, message) in [
                (&["--filter", "text"][..], FILTER_ONLY_TEST),
                (&["--filter=text"], FILTER_ONLY_TEST),
                (&["--fail-fast"], FAIL_FAST_ONLY_TEST),
                (&["--filter", "a", "--fail-fast"], FILTER_ONLY_TEST),
            ] {
                let args = with(prefix, extra);
                cli_refusal(
                    &f.manifest(operation, MISSING, &args, profile),
                    operation,
                    message,
                );
                // Profile first in argv: the applicability order is unchanged.
                let mut first = profile.to_vec();
                first.extend_from_slice(&args);
                cli_refusal(
                    &f.manifest(operation, MISSING, &first, &[]),
                    operation,
                    message,
                );
            }
        }
    }
    // Engine applicability precedes profile applicability.
    for (operation, prefix) in [
        ("check", &[][..]),
        ("build", &[]),
        ("package", &["--output", "out.tar"]),
    ] {
        for profile in EXPLICIT {
            for engine in [&["--engine", "native"][..], &["--engine=interpreter"]] {
                let args = with(prefix, engine);
                cli_refusal(
                    &f.manifest(operation, MISSING, &args, profile),
                    operation,
                    ENGINE_ONLY,
                );
            }
        }
    }
    // Existing command, manifest-position, edition and operand validation.
    for profile in EXPLICIT {
        for (operation, prefix) in APPLICABLE.iter().chain(NOT_APPLICABLE.iter()) {
            let args = with(prefix, &["x.ox"]);
            cli_refusal(
                &f.manifest(operation, MISSING, &args, profile),
                operation,
                &format!("manifest {operation} does not accept a source operand"),
            );
            let args = with(prefix, &["--edition", "legacy-0.9"]);
            cli_refusal(
                &f.manifest(operation, MISSING, &args, profile),
                operation,
                &format!("manifest {operation} conflicts with --edition legacy-0.9"),
            );
            let mut before = vec!["--manifest-path", MISSING, *operation];
            before.extend_from_slice(prefix);
            before.extend_from_slice(profile);
            cli_refusal(
                &f.oxid(&before),
                operation,
                &format!("--manifest-path must follow {operation}"),
            );
        }
    }
    assert!(!f.ex("out.tar").exists());
    f.no_managed_output();
}

#[test]
fn valid_profile_keeps_existing_json_and_package_refusals() {
    let f = Fixture::new(None);
    // A valid profile on an applicable operation changes none of the existing
    // refusals: byte-identical to the same invocation without it.
    let cases: [(&str, &[&str]); 6] = [
        ("test", &["--engine", "native", "--message-format=json"]),
        ("run", &["--engine", "native", "--message-format", "json"]),
        ("package", &[]),
        ("package", &["--output", "a.tar", "--output=b.tar"]),
        ("build", &["--entry-mode", "both"]),
        ("test", &["--engine", "native", "--fail-fast=1"]),
    ];
    for (operation, existing) in cases {
        let baseline = f.manifest(operation, MISSING, existing, &[]);
        assert_eq!(baseline.status.code(), Some(1), "{baseline:?}");
        for profile in EXPLICIT {
            let out = f.manifest(operation, MISSING, existing, profile);
            assert_eq!(out.status.code(), baseline.status.code(), "{out:?}");
            assert_eq!(
                out.stdout, baseline.stdout,
                "{operation} {profile:?}: {out:?}"
            );
            assert_eq!(
                out.stderr, baseline.stderr,
                "{operation} {profile:?}: {out:?}"
            );
        }
    }
    // A malformed profile with JSON on TEST: an ordinary global CLI error
    // rendered as one JSON diagnostic, ahead of the TEST JSON refusal.
    for (form, message) in [
        (&["--profile="][..], MISSING_VALUE.to_string()),
        (&["--profile=bogus"], unknown("bogus")),
        (&["--profile=dev", "--profile=dev"], DUPLICATE.to_string()),
    ] {
        let args = with(&["--engine", "native", "--message-format=json"], form);
        let out = f.manifest("test", MISSING, &args, &[]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stderr.is_empty(), "{out:?}");
        assert_eq!(
            text(&out.stdout),
            format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0001\",\"stage\":\"cli\",\"message\":\"{message}\",\"primary\":null,\"secondary\":[],\"notes\":[]}}\n"),
            "{out:?}"
        );
    }
    assert!(!f.ex("a.tar").exists() && !f.ex("b.tar").exists());
    f.no_managed_output();
}

#[test]
fn source_and_unrelated_commands_follow_profile_applicability() {
    let f = Fixture::new(None);
    let source = "standalone/main.ox";
    // Malformed profile on typed source compile (global tier, before the
    // compile-local backend/output validation).
    for (form, message) in malformed().into_iter().take(17) {
        let mut args = vec!["compile", source, "--edition", "typed-preview"];
        args.extend_from_slice(&["--backend", "llvm", "--output", "out.elf"]);
        args.extend_from_slice(&form);
        cli_refusal(&f.oxid(&args), "compile", &message);
        let mut args = vec!["compile", source, "--edition=typed-preview"];
        args.extend_from_slice(&form);
        cli_refusal(&f.oxid(&args), "compile", &message);
    }
    // Explicit dev or release on every unrelated command.
    for profile in EXPLICIT {
        let forms: [Vec<&str>; 9] = [
            vec!["check", "--edition", "typed-preview", source],
            vec!["run", "--edition=typed-preview", source],
            vec!["fmt", "--edition", "typed-preview", source],
            vec!["fmt", "--edition", "typed-preview", "--check", source],
            vec!["doctor", "--edition", "typed-preview"],
            vec!["new", "--edition", "typed-preview", "fresh-project"],
            vec![
                "compile",
                source,
                "--edition=typed-preview",
                "--backend=llvm",
                "--output=out.elf",
                "--experimental-hir-import",
                "absent-observation",
            ],
            // Legacy argv: a profile is not a legacy optimization option.
            vec!["run", source],
            vec!["test"],
        ];
        for form in forms {
            let mut args = form.clone();
            args.extend_from_slice(profile);
            cli_refusal(&f.oxid(&args), form[0], ONLY);
            let mut args = profile.to_vec();
            args.extend_from_slice(&form);
            cli_refusal(&f.oxid(&args), form[0], ONLY);
        }
    }
    assert!(!f.ex("fresh-project").exists());
    assert!(!f.ex("out.elf").exists());
    f.no_managed_output();
}

#[test]
fn script_and_separator_words_are_never_profiles() {
    let f = Fixture::new(None);
    let caller = f.path("caller");
    let cases: [(&[&str], &[u8]); 7] = [
        (
            &["script", "task", "--profile", "dev", "--manifest-path=p"],
            LEGACY_SCRIPT,
        ),
        (&["script", "task", "--profile=bogus"], LEGACY_SCRIPT),
        (&["script", "task", "--profile"], LEGACY_SCRIPT),
        (
            &["check", "--edition=typed-preview", "--", "--profile"],
            b"error[E0002] (source): cannot read file --profile: No such file or directory (os error 2)\n",
        ),
        (
            &["test", "--manifest-path=p", "--", "--profile"],
            b"error[E0001] (cli): manifest test does not accept a source operand\n",
        ),
        (
            &["build", "--manifest-path=p", "--", "--profile=release"],
            b"error[E0001] (cli): manifest build does not accept a source operand\n",
        ),
        (
            &[
                "run",
                "--manifest-path=p",
                "--engine",
                "native",
                "--",
                "--profile",
                "release",
            ],
            b"error[E0001] (cli): manifest run does not accept a source operand\n",
        ),
    ];
    for (form, stderr) in cases {
        let out = f.oxid_in(&caller, form);
        assert_eq!(out.status.code(), Some(1), "{form:?}: {out:?}");
        assert!(out.stdout.is_empty(), "{form:?}: {out:?}");
        assert_eq!(text(&out.stderr), text(stderr), "{form:?}: {out:?}");
    }
    assert_eq!(fs::read_dir(&caller).unwrap().count(), 0);
    f.no_managed_output();
}

#[test]
fn source_compile_with_profile_never_overwrites_existing_output() {
    let f = Fixture::new(None);
    f.write(&format!("{EX}/out/kept.elf"), "existing output");
    let before = snapshot(&f.ex("out/kept.elf"));
    for (profile, _) in FORMS {
        let out = f.compile("out/kept.elf", profile);
        failure(&out, "compile", NO_OVERWRITE);
        assert!(text(&out.stderr).contains("E0701"), "{out:?}");
        assert_eq!(before, snapshot(&f.ex("out/kept.elf")), "{profile:?}");
    }
    f.no_managed_output();
}

#[test]
fn native_without_toolchain_refuses_every_profile_and_preserves_artifacts() {
    // Without a toolchain no executable and no current/next receipt appears
    // in any namespace. (Planted fake outputs are refused earlier by the
    // existing ownership checks, so preservation of real artifacts is covered
    // by the native tests.) The existing lock step may create the owner
    // marker, but only in the selected profile's own namespaces.
    for (profile, kind) in FORMS {
        let f = Fixture::new(None);
        for (operation, out) in [
            ("build", f.build(&[], profile)),
            ("build", f.build(&["--entry-mode=process"], profile)),
            ("run", f.native_run(&[], profile)),
            ("test", f.native_test("app", &[], profile)),
            ("package", f.package("../../../dist/none.tar", &[], profile)),
        ] {
            failure(&out, operation, "E0701");
        }
        failure(&f.compile("out.elf", profile), "compile", "E0701");
        assert!(!f.ex("out.elf").exists());
        assert!(!f.path("dist/none.tar").exists());
        for (index, state) in f.app_states().iter().enumerate() {
            // app_states order: dev result, release result, dev process,
            // release process.
            let selected = (index % 2 == 1) == (kind == Profile::Release);
            assert!(state[0].is_none(), "{profile:?} {index}: {state:?}");
            assert!(
                state[2].is_none() && state[3].is_none(),
                "{profile:?} {index}"
            );
            if !selected {
                assert!(
                    state[1].is_none(),
                    "{profile:?} {index}: other profile touched"
                );
            }
        }
        for other in [Profile::Dev, Profile::Release] {
            assert!(!f.test_dir("app", other).join("case-0").exists());
        }
        if kind == Profile::Release {
            assert!(!f.ex("app/.oxid/typed-preview/result").exists());
            assert!(!f.ex("app/.oxid/typed-preview/process").exists());
        }
        // No fallback: the interpreter still has to be asked for explicitly.
        ok(&f.manifest("run", "app/oxid.toml", &[], &[]), RESULT);
    }
}

// ---- Native execution (LLVM 19.1.7) ----

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_build_and_run_give_identical_results_in_separate_namespaces() {
    let f = Fixture::native();
    // Default (no flag) is dev: the existing result path and output.
    ok(&f.build(&[], &[]), &build_ok("app", "result", APP));
    let dev_path = f.ex(&Fixture::managed("app", "result", APP));
    exec_result(&dev_path, RESULT);
    let default_bytes = fs::read(&dev_path).unwrap();
    assert_elf(&default_bytes);
    // Explicit dev reuses the same namespace with the same bytes.
    for profile in [&["--profile", "dev"][..], &["--profile=dev"]] {
        ok(&f.build(&[], profile), &build_ok("app", "result", APP));
        assert_eq!(fs::read(&dev_path).unwrap(), default_bytes, "{profile:?}");
    }
    let dev_state = f.state("app", "result", APP);
    assert!(f.state("app", "release-result", APP)[0].is_none());
    // Release publishes separately and leaves the dev artifact and receipts.
    for profile in [&["--profile", "release"][..], &["--profile=release"]] {
        ok(
            &f.build(&[], profile),
            &build_ok("app", "release-result", APP),
        );
        assert_eq!(dev_state, f.state("app", "result", APP), "{profile:?}");
    }
    // Profile before the command (existing global-option scanner).
    ok(
        &f.oxid(&[
            "--profile",
            "release",
            "build",
            "--manifest-path",
            "app/oxid.toml",
        ]),
        &build_ok("app", "release-result", APP),
    );
    let release_path = f.ex(&Fixture::managed("app", "release-result", APP));
    exec_result(&release_path, RESULT);
    let release_bytes = fs::read(&release_path).unwrap();
    assert_elf(&release_bytes);
    assert_ne!(
        release_bytes, default_bytes,
        "release must not reuse the O0 artifact"
    );
    assert_eq!(dev_state, f.state("app", "result", APP));
    // Native RUN prints only main's result under every spelling.
    for (profile, _) in FORMS {
        ok(&f.native_run(&[], profile), RESULT);
    }
    let release_state = f.state("app", "release-result", APP);
    for profile in [&[][..], &["--profile", "dev"], &["--profile=dev"]] {
        ok(&f.native_run(&[], profile), RESULT);
        assert_eq!(
            release_state,
            f.state("app", "release-result", APP),
            "{profile:?}"
        );
    }
    // Process entry policy in both profiles: exit status, no stdout.
    for (profile, kind) in FORMS {
        ok(
            &f.build(&["--entry-mode=process"], profile),
            &build_ok("app", kind.process(), APP),
        );
        exec_process(
            &f.ex(&Fixture::managed("app", kind.process(), APP)),
            RESULT_CODE,
        );
        let run = f.native_run(&["--entry-mode", "process"], profile);
        assert_eq!(run.status.code(), Some(RESULT_CODE), "{run:?}");
        assert!(run.stdout.is_empty() && run.stderr.is_empty(), "{run:?}");
    }
    // Workspace selection keeps the member's namespaces.
    let ws = f.manifest(
        "build",
        "oxid.toml",
        &["--package", APP],
        &["--profile=release"],
    );
    assert_eq!(ws.status.code(), Some(0), "{ws:?}");
    assert!(ws.stderr.is_empty(), "{ws:?}");
    assert!(
        text(&ws.stdout).starts_with("typed-preview native build ok: \""),
        "{ws:?}"
    );
    assert!(
        text(&ws.stdout).ends_with(&format!("app/.oxid/typed-preview/release-result/{APP}\"\n")),
        "{ws:?}"
    );
    exec_result(&release_path, RESULT);
    let ws_run = f.manifest(
        "run",
        "oxid.toml",
        &["--package", APP, "--engine", "native"],
        &["--profile", "release"],
    );
    ok(&ws_run, RESULT);
    for dir in ["", "model", "gate", "standalone"] {
        assert!(!f.ex(dir).join(".oxid").exists(), "{dir}/.oxid");
    }
    assert!(!f.path("packages/utf8_text/.oxid").exists());
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_test_profiles_with_filter_and_fail_fast_use_separate_case_namespaces() {
    let f = Fixture::native();
    // Release first: only release-test is published, case-ORDINAL names.
    suite(
        &f.native_test("app", &[], &["--profile", "release"]),
        APP_SUITE,
    );
    let release_dir = f.test_dir("app", Profile::Release);
    for ordinal in 0..APP_SUITE_CASES.len() {
        let case = release_dir.join(format!("case-{ordinal}"));
        assert!(case.is_file(), "{case:?}");
        assert!(release_dir
            .join(format!(".case-{ordinal}.build/owner"))
            .is_file());
        assert_eq!(exec(&case).status.code(), Some(0), "{case:?}");
    }
    assert!(!release_dir
        .join(format!("case-{}", APP_SUITE_CASES.len()))
        .exists());
    assert!(!f.test_dir("app", Profile::Dev).exists());
    // Dev runs (default and explicit) touch only test/.
    let release_cases = case_states(&release_dir, APP_SUITE_CASES.len());
    for profile in [&[][..], &["--profile", "dev"], &["--profile=dev"]] {
        suite(&f.native_test("app", profile, &[]), APP_SUITE);
        suite(
            &f.native_test("app", &["--filter", "text"], profile),
            APP_TEXT,
        );
        suite(
            &f.native_test("app", &["--filter=text", "--fail-fast"], profile),
            APP_TEXT,
        );
        suite(&f.native_test("app", &["--fail-fast"], profile), APP_SUITE);
        assert_eq!(
            release_cases,
            case_states(&release_dir, APP_SUITE_CASES.len()),
            "{profile:?}"
        );
    }
    let dev_dir = f.test_dir("app", Profile::Dev);
    assert!(dev_dir
        .join(format!("case-{}", APP_SUITE_CASES.len() - 1))
        .is_file());
    // Release runs touch only release-test/.
    let dev_cases = case_states(&dev_dir, APP_SUITE_CASES.len());
    for profile in [&["--profile", "release"][..], &["--profile=release"]] {
        suite(&f.native_test("app", &[], profile), APP_SUITE);
        suite(
            &f.native_test("app", &["--filter", "text"], profile),
            APP_TEXT,
        );
        suite(
            &f.native_test("app", &["--fail-fast", "--filter=text"], profile),
            APP_TEXT,
        );
        suite(&f.native_test("app", &["--fail-fast"], profile), APP_SUITE);
        assert_eq!(
            dev_cases,
            case_states(&dev_dir, APP_SUITE_CASES.len()),
            "{profile:?}"
        );
    }
    // The gate suite: filter + fail-fast, identical under both profiles.
    let table: [(&[&str], &[u8]); 7] = [
        (&[], GATE_SUITE),
        (&["--fail-fast"], GATE_FAIL_FAST),
        (&["--filter", "gate", "--fail-fast"], GATE_FAIL_FAST),
        (&["--filter=divide", "--fail-fast"], GATE_DIVIDE),
        (&["--filter", "divide"], GATE_DIVIDE),
        (&["--fail-fast", "--filter", "gate_f"], GATE_F),
        (&["--filter", "after", "--fail-fast"], GATE_AFTER),
    ];
    for (extra, expected) in table {
        let mut outputs = Vec::new();
        for (profile, _) in FORMS {
            let out = f.native_test("gate", extra, profile);
            suite(&out, expected);
            outputs.push(out);
        }
        for out in &outputs[1..] {
            assert_eq!(out.stdout, outputs[0].stdout, "{extra:?}");
            assert_eq!(
                text(&out.stderr).replace("release-test", "test"),
                text(&outputs[0].stderr).replace("release-test", "test"),
                "{extra:?}"
            );
        }
    }
    // Fail-fast still prepares every selected case in the release namespace,
    // each bound to its own original ordinal.
    let g = Fixture::native();
    suite(
        &g.native_test("gate", &["--fail-fast"], &["--profile=release"]),
        GATE_FAIL_FAST,
    );
    let gate_release = g.test_dir("gate", Profile::Release);
    for (ordinal, (name, pass)) in GATE_CASES.iter().enumerate() {
        let out = exec(&gate_release.join(format!("case-{ordinal}")));
        assert_eq!(out.status.code(), Some(i32::from(!pass)), "{name}: {out:?}");
        assert_eq!(
            text(&out.stderr).contains(DIV_ZERO),
            *name == "test_gate_divide",
            "{name}: {out:?}"
        );
    }
    assert!(!g.test_dir("gate", Profile::Dev).exists());
    // Workspace selection.
    suite(
        &g.manifest(
            "test",
            "oxid.toml",
            &["--package", GATE, "--engine", "native", "--fail-fast"],
            &["--profile", "release"],
        ),
        GATE_FAIL_FAST,
    );
    assert!(!g.ex(".oxid").exists(), "virtual root receives no output");
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn dependency_change_rebuilds_each_profile_independently() {
    let f = Fixture::native();
    let dev = f.ex(&Fixture::managed("app", "result", APP));
    let release = f.ex(&Fixture::managed("app", "release-result", APP));
    ok(
        &f.build(&[], &["--profile", "dev"]),
        &build_ok("app", "result", APP),
    );
    ok(
        &f.build(&[], &["--profile", "release"]),
        &build_ok("app", "release-result", APP),
    );
    exec_result(&dev, RESULT);
    exec_result(&release, RESULT);
    f.edit_model(BASE_7, BASE_8);
    // Rebuilding dev changes only the dev artifact.
    let release_before = f.state("app", "release-result", APP);
    ok(&f.build(&[], &[]), &build_ok("app", "result", APP));
    exec_result(&dev, CHANGED);
    assert_eq!(release_before, f.state("app", "release-result", APP));
    exec_result(&release, RESULT);
    // Rebuilding release picks up the changed dependency too.
    let dev_after = f.state("app", "result", APP);
    ok(
        &f.build(&[], &["--profile=release"]),
        &build_ok("app", "release-result", APP),
    );
    exec_result(&release, CHANGED);
    assert_eq!(dev_after, f.state("app", "result", APP));
    for (profile, _) in FORMS {
        ok(&f.native_run(&[], profile), CHANGED);
        let run = f.native_run(&["--entry-mode=process"], profile);
        assert_eq!(run.status.code(), Some(CHANGED_CODE), "{run:?}");
        suite(&f.native_test("app", &[], profile), CHANGED_SUITE);
        suite(
            &f.native_test("app", &["--fail-fast"], profile),
            CHANGED_FAIL_FAST,
        );
    }
    // Source compile is single-file and unaffected by the dependency.
    ok(
        &f.compile("release.elf", &["--profile", "release"]),
        b"typed-preview native compile ok: \"release.elf\"\n",
    );
    exec_result(&f.ex("release.elf"), RESULT);
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn failed_build_preserves_both_existing_artifacts_and_receipts() {
    let f = Fixture::native();
    for (profile, kind) in [
        (&["--profile", "dev"][..], Profile::Dev),
        (&["--profile", "release"], Profile::Release),
    ] {
        ok(&f.build(&[], profile), &build_ok("app", kind.result(), APP));
        ok(
            &f.build(&["--entry-mode", "process"], profile),
            &build_ok("app", kind.process(), APP),
        );
    }
    let before = f.app_states();
    for state in &before {
        assert!(state[0].is_some() && state[1].is_some(), "{state:?}");
    }
    let broken = format!("{BASE_7}{BROKEN}");
    f.edit_model(BASE_7, &broken);
    for (profile, _) in FORMS {
        for (operation, out) in [
            ("build", f.build(&[], profile)),
            ("build", f.build(&["--entry-mode=process"], profile)),
            ("run", f.native_run(&[], profile)),
            ("run", f.native_run(&["--entry-mode=process"], profile)),
            ("test", f.native_test("app", &[], profile)),
            (
                "package",
                f.package("../../../dist/broken.tar", &[], profile),
            ),
        ] {
            failure(&out, operation, "E0300");
            assert_eq!(before, f.app_states(), "{operation} {profile:?}");
        }
    }
    assert!(!f.path("dist/broken.tar").exists());
    // Both preserved executables still run with their original results.
    exec_result(&f.ex(&Fixture::managed("app", "result", APP)), RESULT);
    exec_result(
        &f.ex(&Fixture::managed("app", "release-result", APP)),
        RESULT,
    );
    exec_process(&f.ex(&Fixture::managed("app", "process", APP)), RESULT_CODE);
    exec_process(
        &f.ex(&Fixture::managed("app", "release-process", APP)),
        RESULT_CODE,
    );
    // Repairing the dependency publishes again, in the selected namespace only.
    f.edit_model(&broken, BASE_7);
    ok(
        &f.build(&[], &["--profile=release"]),
        &build_ok("app", "release-result", APP),
    );
    assert_eq!(before[0], f.app_states()[0], "a release build touched dev");
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn source_compile_profiles_publish_explicit_paths_without_overwrite() {
    let f = Fixture::native();
    fs::create_dir_all(f.ex("out")).unwrap();
    let outputs = [
        "out/default.elf",
        "out/dev.elf",
        "out/dev-eq.elf",
        "out/release.elf",
        "out/release-eq.elf",
    ];
    for ((profile, _), output) in FORMS.iter().zip(outputs) {
        ok(
            &f.compile(output, profile),
            format!("typed-preview native compile ok: \"{output}\"\n").as_bytes(),
        );
        exec_result(&f.ex(output), RESULT);
        assert_elf(&fs::read(f.ex(output)).unwrap());
    }
    let default = fs::read(f.ex("out/default.elf")).unwrap();
    assert_eq!(fs::read(f.ex("out/dev.elf")).unwrap(), default);
    assert_eq!(fs::read(f.ex("out/dev-eq.elf")).unwrap(), default);
    assert_ne!(fs::read(f.ex("out/release.elf")).unwrap(), default);
    // Profile before the command.
    ok(
        &f.oxid(&[
            "--profile=release",
            "compile",
            "standalone/main.ox",
            "--edition=typed-preview",
            "--backend=llvm",
            "--output=out/first.elf",
        ]),
        b"typed-preview native compile ok: \"out/first.elf\"\n",
    );
    exec_result(&f.ex("out/first.elf"), RESULT);
    // No overwrite in either direction; the previous bytes and inode stay.
    let snapshots: Vec<_> = outputs.iter().map(|o| snapshot(&f.ex(o))).collect();
    for output in outputs {
        for (profile, _) in FORMS {
            failure(&f.compile(output, profile), "compile", NO_OVERWRITE);
        }
    }
    let after: Vec<_> = outputs.iter().map(|o| snapshot(&f.ex(o))).collect();
    assert_eq!(snapshots, after);
    // Source compile assigns no managed namespace.
    f.no_managed_output();
}

/// Strict ustar member reader (regular members, zero padding, two end blocks).
fn read_tar(bytes: &[u8]) -> Vec<(String, u64, Vec<u8>)> {
    fn octal(field: &[u8]) -> u64 {
        let end = field
            .iter()
            .position(|b| *b == 0 || *b == b' ')
            .unwrap_or(field.len());
        u64::from_str_radix(std::str::from_utf8(&field[..end]).unwrap(), 8).unwrap()
    }
    assert_eq!(bytes.len() % 512, 0);
    let mut members = Vec::new();
    let mut at = 0;
    loop {
        let header = &bytes[at..at + 512];
        if header.iter().all(|b| *b == 0) {
            assert_eq!(bytes.len(), at + 1024);
            assert!(bytes[at..].iter().all(|b| *b == 0));
            return members;
        }
        assert_eq!(&header[257..263], b"ustar\0");
        assert_eq!(header[156], b'0');
        let end = header[..100].iter().position(|b| *b == 0).unwrap_or(100);
        let name = String::from_utf8(header[..end].to_vec()).unwrap();
        let size = octal(&header[124..136]) as usize;
        let data = bytes[at + 512..at + 512 + size].to_vec();
        members.push((name, octal(&header[100..108]), data));
        at += 512 + size.div_ceil(512) * 512;
    }
}
/// The exact `package.json` bytes for one archive (key order unchanged).
fn metadata(name: &str, profile: Profile, entry: &str, digest: &str) -> String {
    format!("{{\"schema\":\"oxid-native-bundle-v1\",\"name\":\"{name}\",\"target\":\"x86_64-unknown-linux-gnu\",\"profile\":\"{}\",\"entry_mode\":\"{entry}\",\"executable\":\"bin/{name}\",\"sha256\":\"{digest}\",\"dynamic_runtime\":\"host-provided\"}}\n", profile.metadata())
}
/// One package case: archive name, profile argv, profile, entry argv, mode.
struct PackageCase {
    archive: &'static str,
    profile_args: &'static [&'static str],
    profile: Profile,
    entry_args: &'static [&'static str],
    entry: &'static str,
}
const PACKAGES: [PackageCase; 6] = [
    PackageCase {
        archive: "default.tar",
        profile_args: &[],
        profile: Profile::Dev,
        entry_args: &[],
        entry: "result",
    },
    PackageCase {
        archive: "dev.tar",
        profile_args: &["--profile", "dev"],
        profile: Profile::Dev,
        entry_args: &[],
        entry: "result",
    },
    PackageCase {
        archive: "release.tar",
        profile_args: &["--profile", "release"],
        profile: Profile::Release,
        entry_args: &[],
        entry: "result",
    },
    PackageCase {
        archive: "release-eq.tar",
        profile_args: &["--profile=release"],
        profile: Profile::Release,
        entry_args: &[],
        entry: "result",
    },
    PackageCase {
        archive: "dev-proc.tar",
        profile_args: &["--profile=dev"],
        profile: Profile::Dev,
        entry_args: &["--entry-mode", "process"],
        entry: "process",
    },
    PackageCase {
        archive: "release-proc.tar",
        profile_args: &["--profile", "release"],
        profile: Profile::Release,
        entry_args: &["--entry-mode=process"],
        entry: "process",
    },
];

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn package_metadata_digest_and_clean_directory_deployment() {
    let f = Fixture::native();
    let mut deployed = Vec::new();
    for case in &PACKAGES {
        let output = format!("../../../dist/{}", case.archive);
        let namespace = if case.entry == "process" {
            case.profile.process()
        } else {
            case.profile.result()
        };
        let managed = Fixture::managed("app", namespace, APP);
        ok(
            &f.package(&output, case.entry_args, case.profile_args),
            format!(
                "typed-preview native package ok: managed executable \"{managed}\"; archive \"{output}\"\n"
            )
            .as_bytes(),
        );
        let executable = fs::read(f.ex(&managed)).unwrap();
        assert_elf(&executable);
        let bytes = fs::read(f.path(&format!("dist/{}", case.archive))).unwrap();
        let members = read_tar(&bytes);
        let names: Vec<&str> = members.iter().map(|m| m.0.as_str()).collect();
        assert_eq!(
            names,
            [format!("bin/{APP}").as_str(), "package.json", "README.txt"]
        );
        assert_eq!(members[0].1, 0o755);
        assert_eq!(
            members[0].2, executable,
            "{}: archive must hold the managed executable",
            case.archive
        );
        let digest = sha256(&executable);
        assert_eq!(
            text(&members[1].2),
            metadata(APP, case.profile, case.entry, &digest),
            "{}",
            case.archive
        );
        assert!(text(&members[2].2).contains(&format!("Entry mode: {}.", case.entry)));
        deployed.push((case.archive, case.entry, digest));
    }
    // Clean-directory deployment: no sources, compiler cache or toolchain.
    fs::remove_dir_all(f.path("examples")).unwrap();
    fs::remove_dir_all(f.path("packages")).unwrap();
    let _ = fs::remove_dir_all(f.path("cache"));
    for (archive, entry, digest) in deployed {
        let target = f.path(&format!("deploy-{archive}"));
        fs::create_dir(&target).unwrap();
        let status = Command::new("tar")
            .arg("-xf")
            .arg(f.path(&format!("dist/{archive}")))
            .arg("-C")
            .arg(&target)
            .status()
            .unwrap();
        assert!(status.success());
        let binary = target.join("bin").join(APP);
        assert_eq!(sha256(&fs::read(&binary).unwrap()), digest);
        assert_eq!(
            fs::metadata(&binary).unwrap().permissions().mode() & 0o777,
            0o755
        );
        let out = Command::new(format!("./bin/{APP}"))
            .current_dir(&target)
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .unwrap();
        if entry == "process" {
            assert_eq!(out.status.code(), Some(RESULT_CODE), "{archive}: {out:?}");
            assert!(
                out.stdout.is_empty() && out.stderr.is_empty(),
                "{archive}: {out:?}"
            );
        } else {
            ok(&out, RESULT);
        }
    }
}

/// A toolchain directory whose clang / opt / ld.lld log their argv (one line
/// per call) and then exec the real LLVM 19.1.7 tool unchanged.
fn logging_toolchain(f: &Fixture, real: &Path) -> (PathBuf, PathBuf) {
    let dir = f.path("logging-llvm");
    let log = f.path("llvm-argv.log");
    fs::create_dir_all(&dir).unwrap();
    for tool in ["clang", "opt", "ld.lld"] {
        let script = format!(
            "#!/bin/sh\n{{ printf '%s' '{tool}'; for a in \"$@\"; do printf ' %s' \"$a\"; done; printf '\\n'; }} >> '{}'\nexec '{}' \"$@\"\n",
            log.display(),
            real.join(tool).display()
        );
        let path = dir.join(tool);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    (dir, log)
}
/// The clang compile (`-c`) invocations logged after line `from`, and the
/// new line count.
fn compiles(log: &Path, from: usize) -> (usize, Vec<String>) {
    let lines: Vec<String> = fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    let selected = lines[from..]
        .iter()
        .filter(|line| line.starts_with("clang ") && line.split(' ').any(|w| w == "-c"))
        .cloned()
        .collect();
    (lines.len(), selected)
}
/// Every compile is either the generated IR or the bundled C runtime, both
/// kinds occur, and each carries exactly the profile's optimization flag
/// (no LTO, fast-math or CPU tuning).
fn assert_profile_argv(lines: &[String], profile: Profile) {
    let ir = lines
        .iter()
        .filter(|l| l.contains(" -x ir program.ll "))
        .count();
    let runtime = lines.iter().filter(|l| l.contains(" runtime.c ")).count();
    assert!(ir >= 1 && runtime >= 1, "{lines:#?}");
    assert_eq!(ir + runtime, lines.len(), "{lines:#?}");
    for line in lines {
        let flags: Vec<&str> = line
            .split(' ')
            .filter(|w| {
                w.starts_with("-O")
                    || w.starts_with("-flto")
                    || w.starts_with("-ffast-math")
                    || w.starts_with("-march")
                    || w.starts_with("-mtune")
            })
            .collect();
        assert_eq!(flags, [profile.optimization()], "{line}");
        assert!(
            line.contains(" --target=x86_64-unknown-linux-gnu "),
            "{line}"
        );
    }
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn real_compiler_invocations_pass_o0_or_o2_to_ir_and_runtime() {
    let real = llvm_bin();
    let mut f = Fixture::new(Some(real.clone()));
    let (dir, log) = logging_toolchain(&f, &real);
    f.llvm = dir;
    let mut seen = 0;
    for (profile, kind) in FORMS {
        let namespace = kind.result();
        // Every build compiles again (receipts are not a cache).
        for _ in 0..2 {
            ok(&f.build(&[], profile), &build_ok("app", namespace, APP));
            let (total, lines) = compiles(&log, seen);
            assert_profile_argv(&lines, kind);
            seen = total;
            exec_result(&f.ex(&Fixture::managed("app", namespace, APP)), RESULT);
        }
        // Typed source compile.
        let output = format!("argv-{}.elf", NEXT.fetch_add(1, Ordering::Relaxed));
        ok(
            &f.compile(&output, profile),
            format!("typed-preview native compile ok: \"{output}\"\n").as_bytes(),
        );
        let (total, lines) = compiles(&log, seen);
        assert_profile_argv(&lines, kind);
        seen = total;
        exec_result(&f.ex(&output), RESULT);
        // Native TEST prepares its cases with the same profile.
        suite(
            &f.native_test("gate", &["--fail-fast"], profile),
            GATE_FAIL_FAST,
        );
        let (total, lines) = compiles(&log, seen);
        assert_profile_argv(&lines, kind);
        seen = total;
        // Package.
        let archive = format!(
            "../../../dist/argv-{}.tar",
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let out = f.package(&archive, &[], profile);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        let (total, lines) = compiles(&log, seen);
        assert_profile_argv(&lines, kind);
        seen = total;
    }
    // The link step carries no optimization flag; opt only verifies.
    let all = fs::read_to_string(&log).unwrap();
    assert!(all
        .lines()
        .any(|line| line.starts_with("clang ") && line.contains(" -pie ")));
    for line in all.lines() {
        if line.starts_with("clang ") && line.contains(" -pie ") {
            assert!(!line.split(' ').any(|w| w.starts_with("-O")), "{line}");
        }
        if line.starts_with("opt ") && !line.contains("--version") {
            assert_eq!(line, "opt -passes=verify -disable-output program.ll");
        }
    }
}
