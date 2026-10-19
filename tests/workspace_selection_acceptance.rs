//! Independent explicit-workspace selection acceptance (separate from the
//! author's `workspace_selection_core`). Tests marked `#[ignore]` need LLVM
//! 19.1.7: run them with `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test
//! --test workspace_selection_acceptance -- --include-ignored`. Opting in
//! without that env var fails the LLVM gate; a missing tool is never a pass.
//! No qualification, capacity or fuel probes.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

type FileSnapshot = Option<(u64, u64, Vec<u8>)>;

static NEXT: AtomicUsize = AtomicUsize::new(0);
const APP: &str = "wsaccept-app";
const LIB: &str = "wsaccept-library";
const ROOT_MANIFEST: &str = "[workspace]\nmembers = [\"app\", \"library\"]\n";
const APP_MANIFEST: &str = "[project]\nname = \"wsaccept-app\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\n[dependencies]\nlib = \"../library\"\n";
const APP_MAIN: &str = "use lib::scale;\nuse lib::model::make_pair;\nuse lib::model::pair_sum;\nfn main() -> i32 {\n    let p = make_pair();\n    return scale(pair_sum(&p));\n}\nfn test_scale() -> bool {\n    return scale(2) == 6;\n}\nfn test_pair() -> bool {\n    let p = make_pair();\n    return pair_sum(&p) == 11;\n}\n";
const LIB_MANIFEST: &str = "[project]\nname = \"wsaccept-library\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/lib.ox\"\n";
const LIB_ROOT: &str = "pub mod model;\nfn factor() -> i32 {\n    return 3;\n}\npub fn scale(x: i32) -> i32 {\n    return x * crate::factor();\n}\nfn main() -> i32 {\n    return 55;\n}\nfn test_library() -> bool {\n    return scale(1) == 3;\n}\n";
const LIB_MODEL: &str = "pub struct Pair {\n    a: i32,\n    b: i32\n}\npub fn make_pair() -> Pair {\n    return Pair { a: 4, b: 7 };\n}\npub fn pair_sum(p: &Pair) -> i32 {\n    return p.a + p.b;\n}\n";
const PASS_APP: &[u8] =
    b"PASS test_scale\nPASS test_pair\ntest result: 2 passed; 0 failed; 2 total\n";
const PASS_LIB: &[u8] = b"PASS test_library\ntest result: 1 passed; 0 failed; 1 total\n";
const AVAILABLE: &str = "available packages: wsaccept-app, wsaccept-library";

fn llvm_bin() -> PathBuf {
    let dir = PathBuf::from(
        std::env::var_os("OXID_TEST_LLVM_BIN")
            .expect("native workspace acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin)"),
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
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-wsselect-accept-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
        };
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/workspace_selection_acceptance");
        for (file, expected) in [
            ("oxid.toml", ROOT_MANIFEST),
            ("app/oxid.toml", APP_MANIFEST),
            ("app/src/main.ox", APP_MAIN),
            ("library/oxid.toml", LIB_MANIFEST),
            ("library/src/lib.ox", LIB_ROOT),
            ("library/src/model.ox", LIB_MODEL),
        ] {
            let text = fs::read_to_string(example.join(file)).unwrap();
            assert_eq!(text, expected, "checked-in example {file} changed");
            fixture.write(&format!("ws/{file}"), &text);
        }
        fixture
    }
    fn native() -> Self {
        Self::new(Some(llvm_bin()))
    }
    fn path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.path(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn command_in(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command_with(&self.llvm, cwd, args)
    }
    fn command_with(&self, llvm: &Path, cwd: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(cwd)
            .env("OXID_LLVM_BIN", llvm)
            .env("OXID_CACHE_DIR", self.path("cache"))
            .env("PATH", self.path("absent-path"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn oxid(&self, args: &[&str]) -> Output {
        self.command_in(&self.root, args)
    }
    fn invoke(&self, command: &str, extra: &[&str]) -> Output {
        let mut args = vec![command, "--manifest-path", "ws/oxid.toml"];
        args.extend_from_slice(extra);
        self.oxid(&args)
    }
    fn refuse(&self, command: &str, extra: &[&str], needle: &str) -> Output {
        refused(self.invoke(command, extra), command, extra, needle)
    }
    fn refuse_without_llvm(&self, command: &str, extra: &[&str]) -> Output {
        let mut args = vec![command, "--manifest-path", "ws/oxid.toml"];
        args.extend_from_slice(extra);
        let out = self.command_with(&self.path("absent-llvm"), &self.root, &args);
        let out = refused(out, command, extra, "E0701");
        assert!(text(&out.stderr).contains("absent-llvm"), "{out:?}");
        assert!(!text(&out.stdout).contains("33"), "{out:?}");
        out
    }
    fn artifact(&self, member: &str, name: &str) -> PathBuf {
        self.path(&format!("ws/{member}/.oxid/typed-preview/result/{name}"))
    }
    fn state(&self, member: &str, name: &str) -> [FileSnapshot; 4] {
        let artifact = self.artifact(member, name);
        let state = artifact.parent().unwrap().join(format!(".{name}.build"));
        [
            artifact,
            state.join("owner"),
            state.join("current"),
            state.join("next"),
        ]
        .map(|path| optional_identity(&path))
    }
    fn app_state(&self) -> [FileSnapshot; 4] {
        self.state("app", APP)
    }
    fn lib_state(&self) -> [FileSnapshot; 4] {
        self.state("library", LIB)
    }
    fn reset(&self) {
        for (file, text) in [
            ("ws/oxid.toml", ROOT_MANIFEST),
            ("ws/app/oxid.toml", APP_MANIFEST),
            ("ws/app/src/main.ox", APP_MAIN),
            ("ws/library/oxid.toml", LIB_MANIFEST),
            ("ws/library/src/lib.ox", LIB_ROOT),
            ("ws/library/src/model.ox", LIB_MODEL),
        ] {
            self.write(file, text);
        }
    }
    /// The virtual root and the invoking directory never receive application
    /// output, and no lockfile is written anywhere.
    fn no_root_outputs(&self) {
        assert!(!self.path("ws/.oxid").exists());
        assert!(!self.path(".oxid").exists());
        for dir in ["ws", "ws/app", "ws/library", ""] {
            assert!(!self.path(dir).join("oxid.lock").exists(), "{dir}");
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.path("ws/app/src"), fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn refused(out: Output, command: &str, extra: &[&str], needle: &str) -> Output {
    assert_eq!(out.status.code(), Some(1), "{command} {extra:?}: {out:?}");
    assert!(
        text(&out.stderr).contains(needle),
        "{command} {extra:?} expected {needle:?}: {out:?}"
    );
    assert!(!text(&out.stdout).contains("PASS"), "{out:?}");
    out
}
fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}
fn optional_identity(path: &Path) -> FileSnapshot {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            assert!(meta.file_type().is_file(), "{path:?}");
            Some((meta.dev(), meta.ino(), fs::read(path).unwrap()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("cannot snapshot {path:?}: {error}"),
    }
}
fn ok(out: &Output, stdout: &[u8]) {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, stdout, "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
}
fn standalone(artifact: &Path) -> Output {
    Command::new(artifact)
        .current_dir("/")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap()
}
/// The existing first-publication contract: owner and next exist, next is the
/// artifact inode, and there is no current receipt yet.
fn assert_first_publication(state: &[FileSnapshot; 4]) {
    assert!(state[0].is_some(), "artifact missing: {state:?}");
    assert!(state[1].is_some(), "owner missing");
    assert!(
        state[2].is_none(),
        "first publication must not have current"
    );
    assert_eq!(state[3], state[0], "next must be the artifact inode");
}

// ---- Native execution (LLVM 19.1.7) ----

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_multi_package_flow_first_publication_and_member_artifacts() {
    let f = Fixture::native();
    // Ordinary check/run/test with split and `=` selector forms.
    for selector in [vec!["--package", APP], vec!["--package=wsaccept-app"]] {
        let check = f.invoke("check", &selector);
        assert_eq!(check.status.code(), Some(0), "{check:?}");
        assert!(text(&check.stdout).contains("typed-preview check ok"));
        ok(&f.invoke("run", &selector), b"33\n");
        ok(&f.invoke("test", &selector), PASS_APP);
    }
    assert!(
        !f.path("ws/app/.oxid").exists(),
        "interpreter routes publish nothing"
    );
    // Native RUN is the very first publication: owner + next only, no current.
    let native_run = ["--package", APP, "--engine", "native"];
    ok(&f.invoke("run", &native_run), b"33\n");
    let first = f.app_state();
    assert_first_publication(&first);
    // Native TEST agrees with the interpreter, uses its own namespace and
    // leaves every result slot (including the absent current) untouched.
    let suite = f.invoke("test", &["--package", APP, "--engine", "native"]);
    ok(&suite, PASS_APP);
    assert_eq!(suite.stdout, f.invoke("test", &["--package", APP]).stdout);
    assert_eq!(f.app_state(), first);
    assert!(f.path("ws/app/.oxid/typed-preview/test").is_dir());
    // BUILD rotates: the previous next becomes current, next is the new artifact.
    let build = f.invoke("build", &["--package", APP]);
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    assert!(text(&build.stdout).contains("typed-preview native build ok"));
    assert!(
        text(&build.stdout).contains("ws/app/.oxid/typed-preview/result/wsaccept-app"),
        "{build:?}"
    );
    let second = f.app_state();
    assert_eq!(second[1], first[1], "owner is stable");
    assert_eq!(second[2], first[3], "current is the previous next inode");
    assert_eq!(second[3], second[0]);
    assert_ne!(second[0].as_ref().unwrap().1, first[0].as_ref().unwrap().1);
    let exe = standalone(&f.artifact("app", APP));
    assert_eq!(exe.status.code(), Some(0), "{exe:?}");
    assert_eq!(exe.stdout, b"33\n");
    // Absolute manifest from another cwd (inside the library) with JSON build:
    // output still belongs to the selected member, nothing lands in the cwd.
    let absolute = f.path("ws/oxid.toml");
    let json = f.command_in(
        &f.path("ws/library/src"),
        &[
            "build",
            "--manifest-path",
            absolute.to_str().unwrap(),
            "--package=wsaccept-app",
            "--message-format=json",
        ],
    );
    assert_eq!(json.status.code(), Some(0), "{json:?}");
    assert!(text(&json.stdout).contains("\"kind\":\"build-summary\""));
    assert!(text(&json.stdout).contains("\"success\":true"));
    assert!(json.stderr.is_empty(), "{json:?}");
    assert!(!f.path("ws/library/src/.oxid").exists());
    assert!(!f.path("ws/library/.oxid").exists());
    let third = f.app_state();
    assert_eq!(third[2], second[3]);
    assert_eq!(third[3], third[0]);
    // Relative manifest from the app directory, native RUN and TEST.
    let relative = f.command_in(
        &f.path("ws/app"),
        &[
            "run",
            "--manifest-path=../oxid.toml",
            "--package",
            APP,
            "--engine=native",
        ],
    );
    ok(&relative, b"33\n");
    let suite = f.command_in(
        Path::new("/"),
        &[
            "test",
            "--manifest-path",
            absolute.to_str().unwrap(),
            "--package",
            APP,
            "--engine",
            "native",
        ],
    );
    ok(&suite, PASS_APP);
    // Selecting the library natively publishes into the library member only;
    // its first publication also has only next, and the app is untouched.
    let app_before = f.app_state();
    let build = f.invoke("build", &["--package", LIB]);
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    assert_first_publication(&f.lib_state());
    let exe = standalone(&f.artifact("library", LIB));
    assert_eq!(exe.stdout, b"55\n");
    ok(
        &f.invoke("test", &["--package", LIB, "--engine", "native"]),
        PASS_LIB,
    );
    assert_eq!(f.app_state(), app_before);
    f.no_root_outputs();
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_failures_preserve_member_artifact_and_receipt_slots() {
    let f = Fixture::native();
    let build = f.invoke("build", &["--package", APP]);
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    let first = f.app_state();
    assert_first_publication(&first);
    let routes: [(&str, &[&str]); 3] = [
        ("build", &["--package", APP]),
        ("run", &["--package", APP, "--engine", "native"]),
        ("test", &["--package", APP, "--engine", "native"]),
    ];
    // Frontend failures on top of a FIRST publication keep current absent and
    // owner/next/artifact identities exact.
    let failures = [
        (
            "ws/library/src/model.ox",
            format!("{LIB_MODEL}fn unused() -> i32 {{ return false; }}\n"),
            "E0300",
        ),
        (
            "ws/library/oxid.toml",
            "[project]\nname=broken".to_string(),
            "E0001",
        ),
        (
            "ws/oxid.toml",
            "[workspace]\nmembers = [\"app\", \"library\", \"absent\"]\n".to_string(),
            "E0002",
        ),
        (
            "ws/app/src/main.ox",
            APP_MAIN.replace("return scale(pair_sum(&p));", "return false;"),
            "E0300",
        ),
    ];
    for (file, source, needle) in &failures {
        f.reset();
        f.write(file, source);
        for (command, extra) in routes {
            let out = f.refuse(command, extra, needle);
            assert!(!text(&out.stderr).contains("clang"), "{out:?}");
            assert!(!text(&out.stdout).contains("33"), "{out:?}");
            assert_eq!(f.app_state(), first, "{file} {command}");
        }
    }
    // Selector failures are also preflight.
    f.reset();
    for (command, extra) in routes {
        let extra: Vec<&str> = extra
            .iter()
            .copied()
            .filter(|a| *a != "--package" && *a != APP)
            .collect();
        f.refuse(command, &extra, AVAILABLE);
        assert_eq!(f.app_state(), first);
    }
    // An unselected member's broken source does not block the library; the
    // app's publication is untouched. Its broken manifest rejects selection.
    f.write("ws/app/src/main.ox", "not valid source");
    let build = f.invoke("build", &["--package", LIB]);
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    let lib_first = f.lib_state();
    assert_first_publication(&lib_first);
    assert_eq!(f.app_state(), first);
    f.write("ws/app/oxid.toml", "[project]\nname=broken");
    f.refuse("build", &["--package", LIB], "ws/app/oxid.toml");
    f.refuse(
        "run",
        &["--package", LIB, "--engine", "native"],
        "ws/app/oxid.toml",
    );
    assert_eq!(f.lib_state(), lib_first);
    assert_eq!(f.app_state(), first);
    // A failing native suite returns 1 like the interpreter and touches no slot.
    f.reset();
    f.write(
        "ws/app/src/main.ox",
        &format!("{APP_MAIN}fn test_wrong() -> bool {{ return scale(1) == 4; }}\n"),
    );
    let native = f.invoke("test", &["--package", APP, "--engine", "native"]);
    let interpreter = f.invoke("test", &["--package", APP]);
    assert_eq!(native.status.code(), Some(1), "{native:?}");
    assert_eq!(native.stdout, interpreter.stdout);
    assert_eq!(
        native.stdout,
        b"PASS test_scale\nPASS test_pair\nFAIL test_wrong\ntest result: 2 passed; 1 failed; 3 total\n"
    );
    assert_eq!(f.app_state(), first);
    // After a second publication (current + next), failures keep all four.
    f.reset();
    let build = f.invoke("build", &["--package", APP]);
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    let second = f.app_state();
    assert_eq!(second[2], first[3]);
    assert_eq!(second[3], second[0]);
    for (file, source, needle) in &failures {
        f.reset();
        f.write(file, source);
        for (command, extra) in routes {
            f.refuse(command, extra, needle);
            assert_eq!(f.app_state(), second, "{file} {command}");
        }
    }
    // A missing native tool is a failure (E0701), never a pass. The artifact
    // bytes/inode and owner survive and stay owned by a receipt; the existing
    // pre-compile normalization may move next to current (reported).
    f.reset();
    for (command, extra) in [routes[0], routes[1]] {
        let before = f.app_state();
        f.refuse_without_llvm(command, extra);
        let after = f.app_state();
        assert_eq!(after[0], before[0], "artifact identity");
        assert_eq!(after[1], before[1], "owner identity");
        assert!(after[2] == after[0] || after[3] == after[0], "{after:?}");
    }
    let before = f.app_state();
    f.refuse_without_llvm("test", routes[2].1);
    assert_eq!(f.app_state(), before);
    let exe = standalone(&f.artifact("app", APP));
    assert_eq!(exe.stdout, b"33\n");
    f.no_root_outputs();
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn example_readme_commands_from_example_directory() {
    let f = Fixture::native();
    let dir = f.path("ws");
    let check = f.command_in(
        &dir,
        &["check", "--manifest-path", "oxid.toml", "--package", APP],
    );
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    ok(
        &f.command_in(
            &dir,
            &["run", "--manifest-path", "oxid.toml", "--package", APP],
        ),
        b"33\n",
    );
    ok(
        &f.command_in(
            &dir,
            &["test", "--manifest-path", "oxid.toml", "--package", APP],
        ),
        PASS_APP,
    );
    ok(
        &f.command_in(
            &dir,
            &[
                "run",
                "--manifest-path",
                "oxid.toml",
                "--package=wsaccept-library",
            ],
        ),
        b"55\n",
    );
    let build = f.command_in(
        &dir,
        &["build", "--manifest-path", "oxid.toml", "--package", APP],
    );
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    assert_first_publication(&f.app_state());
    ok(
        &f.command_in(
            &dir,
            &[
                "run",
                "--manifest-path",
                "oxid.toml",
                "--package",
                APP,
                "--engine",
                "native",
            ],
        ),
        b"33\n",
    );
    ok(
        &f.command_in(
            &dir,
            &[
                "test",
                "--manifest-path",
                "oxid.toml",
                "--package",
                APP,
                "--engine",
                "native",
            ],
        ),
        PASS_APP,
    );
    let readme = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/workspace_selection_acceptance/README.md"),
    )
    .unwrap();
    for needle in [
        "LLVM 19.1.7",
        "33",
        "55",
        AVAILABLE.trim_start_matches("available packages: "),
        "app/.oxid/typed-preview/result/wsaccept-app",
        "no `current`",
    ] {
        assert!(readme.contains(needle), "{needle}");
    }
    f.no_root_outputs();
}

// ---- Ordinary contracts (no toolchain required) ----

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

#[test]
fn missing_native_toolchain_is_a_failure_not_a_pass() {
    let f = Fixture::new(None);
    for (command, extra) in [
        ("build", vec!["--package", APP]),
        ("run", vec!["--package", APP, "--engine", "native"]),
        ("test", vec!["--package", APP, "--engine", "native"]),
    ] {
        f.refuse_without_llvm(command, &extra);
        assert!(!f.artifact("app", APP).exists());
    }
    f.no_root_outputs();
}

#[test]
fn interpreter_cwd_and_absolute_manifest_paths() {
    let f = Fixture::new(None);
    let absolute = f.path("ws/oxid.toml");
    let absolute = absolute.to_str().unwrap();
    fs::create_dir_all(f.path("ws/library/src/deep")).unwrap();
    for (cwd, manifest) in [
        (f.path(""), "ws/oxid.toml"),
        (f.path("ws"), "oxid.toml"),
        (f.path("ws/app"), "../oxid.toml"),
        (f.path("ws/library/src/deep"), "../../../oxid.toml"),
        (PathBuf::from("/"), absolute),
        (f.path("ws/library"), absolute),
    ] {
        let check = f.command_in(
            &cwd,
            &["check", "--manifest-path", manifest, "--package", APP],
        );
        assert_eq!(check.status.code(), Some(0), "{cwd:?}: {check:?}");
        ok(
            &f.command_in(
                &cwd,
                &[
                    "run",
                    &format!("--manifest-path={manifest}"),
                    "--package",
                    APP,
                ],
            ),
            b"33\n",
        );
        ok(
            &f.command_in(
                &cwd,
                &[
                    "test",
                    "--manifest-path",
                    manifest,
                    "--package=wsaccept-app",
                ],
            ),
            PASS_APP,
        );
        ok(
            &f.command_in(
                &cwd,
                &["run", "--manifest-path", manifest, "--package", LIB],
            ),
            b"55\n",
        );
        ok(
            &f.command_in(
                &cwd,
                &["test", "--manifest-path", manifest, "--package", LIB],
            ),
            PASS_LIB,
        );
        let omitted = f.command_in(&cwd, &["run", "--manifest-path", manifest]);
        assert_eq!(omitted.status.code(), Some(1));
        assert!(text(&omitted.stderr).contains(AVAILABLE), "{omitted:?}");
    }
    // A virtual manifest beside the members names explicit sibling paths.
    f.write(
        "outer/oxid.toml",
        "[workspace]\nmembers = [\n  \"../ws/app\", # app\n  \"../ws/library\",\n]\n",
    );
    ok(
        &f.oxid(&[
            "run",
            "--manifest-path",
            "outer/oxid.toml",
            "--package",
            APP,
        ]),
        b"33\n",
    );
    // A symlinked workspace directory resolves members lexically from it.
    symlink(f.path("ws"), f.path("ws-link")).unwrap();
    ok(
        &f.oxid(&[
            "run",
            "--manifest-path",
            "ws-link/oxid.toml",
            "--package",
            APP,
        ]),
        b"33\n",
    );
    assert!(!f.path("outer/.oxid").exists());
    assert!(!f.path("ws/library/src/deep/.oxid").exists());
    assert!(!f.path("ws/app/.oxid").exists());
    f.no_root_outputs();
}

#[test]
fn selector_rules_single_multi_direct_and_unknown_names() {
    let f = Fixture::new(None);
    // Multiple members: omission fails before any source is loaded, for
    // every command, listing names in declared order.
    fs::remove_file(f.path("ws/app/src/main.ox")).unwrap();
    for command in ["check", "run", "build", "test"] {
        let out = f.refuse(command, &[], "workspace requires --package");
        assert!(text(&out.stderr).contains(AVAILABLE), "{out:?}");
        assert!(!text(&out.stderr).contains("E0002"), "{out:?}");
    }
    let run = f.refuse("run", &["--engine", "native"], AVAILABLE);
    assert!(!text(&run.stderr).contains("E0701"), "{run:?}");
    // Wrong names and path-like selectors are unknown package names.
    for selector in [
        "absent",
        "app",
        "library",
        "./app",
        "app/oxid.toml",
        "lib",
        "WSACCEPT-APP",
        "wsaccept_app",
        " wsaccept-app",
    ] {
        let arg = format!("--package={selector}");
        let out = f.refuse(
            "check",
            &[&arg],
            &format!("unknown workspace package `{selector}`"),
        );
        assert!(text(&out.stderr).contains(AVAILABLE), "{out:?}");
    }
    // Duplicate or valueless selectors.
    for extra in [
        vec!["--package", APP, "--package", APP],
        vec!["--package=wsaccept-app", "--package", LIB],
    ] {
        f.refuse("check", &extra, "--package may only be specified once");
    }
    f.refuse("check", &["--package="], "--package requires a value");
    f.refuse(
        "check",
        &["--package", "--message-format=text"],
        "--package requires a value",
    );
    let long = format!("--package={}", "a".repeat(65));
    f.refuse("check", &[&long], "--package name exceeds 64 bytes");
    // A selector before the command word is not a manifest-local option.
    let early = f.oxid(&["--package", APP, "check", "--manifest-path", "ws/oxid.toml"]);
    assert_eq!(early.status.code(), Some(1), "{early:?}");
    assert!(
        text(&early.stderr).contains("unsupported option"),
        "{early:?}"
    );
    // The selector may precede --manifest-path after the command word.
    f.write("ws/app/src/main.ox", APP_MAIN);
    ok(
        &f.oxid(&["run", "--package", APP, "--manifest-path", "ws/oxid.toml"]),
        b"33\n",
    );
    // Single member: omission and the exact name both select it; others fail.
    f.write("ws/oxid.toml", "[workspace]\nmembers = [\"app\"]\n");
    ok(&f.invoke("run", &[]), b"33\n");
    ok(&f.invoke("test", &[]), PASS_APP);
    ok(&f.invoke("run", &["--package", APP]), b"33\n");
    let out = f.refuse(
        "run",
        &["--package", LIB],
        "unknown workspace package `wsaccept-library`",
    );
    assert!(text(&out.stderr).contains("available packages: wsaccept-app"));
    // A direct project manifest rejects --package and is unchanged without it.
    for command in ["check", "run", "build", "test"] {
        let out = f.oxid(&[
            command,
            "--manifest-path",
            "ws/app/oxid.toml",
            "--package",
            APP,
        ]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(
            text(&out.stderr).contains("direct project manifests do not accept it"),
            "{out:?}"
        );
    }
    ok(
        &f.oxid(&["run", "--manifest-path", "ws/app/oxid.toml"]),
        b"33\n",
    );
    ok(
        &f.oxid(&["test", "--manifest-path", "ws/app/oxid.toml"]),
        PASS_APP,
    );
    f.no_root_outputs();
    assert!(!f.path("ws/app/.oxid").exists());
}

#[test]
fn virtual_manifest_and_member_shape_rejections() {
    let f = Fixture::new(None);
    for (root, needle) in [
        (
            "[workspace]\nmembers = [\"app\", \"library\"]\n[project]\nname = \"x\"\n",
            "exactly one [workspace] table",
        ),
        (
            "[workspace]\nmembers = [\"app\", \"library\"]\n[dependencies]\nlib = \"library\"\n",
            "exactly one [workspace] table",
        ),
        (
            "[workspace]\nmembers = [\"app\"]\n[workspace]\nmembers = [\"library\"]\n",
            "exactly one [workspace] table",
        ),
        (
            "[workspace]\nmembers = [\"app\"]\nmembers = [\"library\"]\n",
            "duplicate workspace.members",
        ),
        (
            "[workspace]\nmembers = []\n",
            "at least one explicit member",
        ),
        ("[workspace]\n", "at least one explicit member"),
        (
            "[workspace]\nmembers = [\"app\", \"library\"]\ndefault-members = [\"app\"]\n",
            "only the members key",
        ),
        ("[workspace]\nmembers = [\"app\", \"lib*\"]\n", "globs"),
        (
            "[workspace]\nmembers = [\"app\", \"lib\\\\rary\"]\n",
            "backslashes",
        ),
        (
            "[workspace]\nmembers = [\"/abs/app\"]\n",
            "must be relative",
        ),
        ("[workspace]\nmembers = [\"\"]\n", "1..2048 decoded bytes"),
        (
            "[workspace]\nmembers = \"app\"\n",
            "requires a string array",
        ),
        (
            "[workspace]\nmembers = [\"app\" \"library\"]\n",
            "expected comma or ]",
        ),
        (
            "[workspace]\nmembers = [\"app\", \"library\", \"./app/\"]\n",
            "duplicate canonical workspace member `./app/`",
        ),
        (
            "[workspace]\nmembers = [\"app\", \"library\", \"../ws/library\"]\n",
            "duplicate canonical workspace member `../ws/library`",
        ),
        (
            "[workspace]\nmembers = [\"app\", \"library\", \"alias\"]\n",
            "duplicate canonical workspace member `alias`",
        ),
        (
            "[workspace]\nmembers = [\"app\", \".\"]\n",
            "workspace membership cycle",
        ),
        (
            "[workspace]\nmembers = [\"app\", \"inner\"]\n",
            "nested workspaces are not supported",
        ),
        ("[workspace]\nmembers = [\"app\", \"empty\"]\n", "E0002"),
    ] {
        f.reset();
        symlink("library", f.path("ws/alias")).ok();
        fs::create_dir_all(f.path("ws/empty")).unwrap();
        f.write(
            "ws/inner/oxid.toml",
            "[workspace]\nmembers = [\"../library\"]\n",
        );
        f.write("ws/oxid.toml", root);
        for selector in [vec!["--package", APP], vec!["--package=absent"]] {
            let out = f.refuse("check", &selector, needle);
            assert!(
                out.stdout.is_empty() || !text(&out.stdout).contains("ok"),
                "{out:?}"
            );
        }
    }
    // A [project] manifest that later declares [workspace] is a direct
    // manifest: it rejects --package and rejects the workspace section.
    f.reset();
    f.write("ws/oxid.toml", "[project]\nname = \"x\"\nedition = \"typed-preview\"\nentry = \"app/src/main.ox\"\n[workspace]\nmembers = [\"app\"]\n");
    f.refuse(
        "check",
        &["--package", APP],
        "direct project manifests do not accept it",
    );
    f.refuse(
        "check",
        &[],
        "unsupported manifest section for typed check: workspace",
    );
    // Member manifests: edition, entry, name and duplicates are validated for
    // every member, selected or not, before any source is read.
    for (manifest, needle) in [
        (
            LIB_MANIFEST.replace("typed-preview", "legacy-0.9"),
            "requires [project].edition",
        ),
        (
            LIB_MANIFEST.replace("edition = \"typed-preview\"\n", ""),
            "requires [project].edition",
        ),
        (
            LIB_MANIFEST.replace("entry = \"src/lib.ox\"\n", ""),
            "requires [project].entry",
        ),
        (
            LIB_MANIFEST.replace("src/lib.ox", "../escape.ox"),
            "manifest entry must be",
        ),
        (
            LIB_MANIFEST.replace("src/lib.ox", "src/lib.txt"),
            "manifest entry must be",
        ),
        (
            LIB_MANIFEST.replace("src/lib.ox", "./src/lib.ox"),
            "manifest entry must be",
        ),
        (
            LIB_MANIFEST.replace("wsaccept-library", "wsaccept-app"),
            "duplicate workspace member name `wsaccept-app`",
        ),
        (
            LIB_MANIFEST.replace("name = \"wsaccept-library\"\n", ""),
            "requires [project].name",
        ),
        (
            LIB_MANIFEST.replace("wsaccept-library", "bad name"),
            "name must match",
        ),
        (
            LIB_MANIFEST.replace(
                "[project]",
                "[workspace]\nmembers = [\"../app\"]\n[project]",
            ),
            "nested workspaces",
        ),
        (
            "[project]\nname=broken".to_string(),
            "expected double-quoted value",
        ),
    ] {
        f.reset();
        f.write("ws/library/oxid.toml", &manifest);
        for command in ["check", "run", "test"] {
            let out = f.refuse(command, &["--package", APP], needle);
            // The duplicate-name refusal names the virtual manifest instead.
            let owner = if needle.starts_with("duplicate workspace member name") {
                "manifest `ws/oxid.toml`"
            } else {
                "ws/library/oxid.toml"
            };
            assert!(text(&out.stderr).contains(owner), "{out:?}");
        }
        f.refuse("check", &["--package", LIB], needle);
        f.refuse("check", &["--package=absent"], needle);
    }
    // Absent entry: rejected for the selected member (source read) only.
    f.reset();
    f.write(
        "ws/app/oxid.toml",
        &APP_MANIFEST.replace("src/main.ox", "src/absent.ox"),
    );
    f.refuse("check", &["--package", APP], "ws/app/src/absent.ox");
    ok(&f.invoke("run", &["--package", LIB]), b"55\n");
    // Wrong edition on the selected member itself.
    f.write(
        "ws/app/oxid.toml",
        &APP_MANIFEST.replace("typed-preview", "legacy-0.9"),
    );
    f.refuse("run", &["--package", APP], "requires [project].edition");
    f.no_root_outputs();
}

#[test]
fn unselected_sources_are_not_read_but_dependency_sources_are() {
    let f = Fixture::new(None);
    // Selected library: the unselected app's source is invalid, then unreadable.
    f.write("ws/app/src/main.ox", "not valid source");
    ok(&f.invoke("run", &["--package", LIB]), b"55\n");
    ok(&f.invoke("test", &["--package", LIB]), PASS_LIB);
    let check = f.invoke("check", &["--package", LIB, "--message-format=json"]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    assert!(!text(&check.stdout).contains("app/src/main.ox"));
    fs::set_permissions(f.path("ws/app/src"), fs::Permissions::from_mode(0o000)).unwrap();
    ok(&f.invoke("run", &["--package", LIB]), b"55\n");
    fs::set_permissions(f.path("ws/app/src"), fs::Permissions::from_mode(0o755)).unwrap();
    // ...but an invalid unselected manifest rejects the selection.
    f.write("ws/app/oxid.toml", "[project]\nname=broken");
    for command in ["check", "run", "test"] {
        f.refuse(command, &["--package", LIB], "ws/app/oxid.toml");
    }
    f.reset();
    // Selected app: its dependency's (library) source errors are reported with
    // the dependency file, because the dependency is in the selected closure.
    f.write(
        "ws/library/src/model.ox",
        &format!("{LIB_MODEL}fn unused() -> i32 {{ return false; }}\n"),
    );
    let out = f.invoke("check", &["--package", APP, "--message-format=json"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(text(&out.stdout).contains("\"code\":\"E0300\""), "{out:?}");
    assert!(
        text(&out.stdout).contains("ws/library/src/model.ox"),
        "{out:?}"
    );
    for command in ["run", "test"] {
        let out = f.refuse(command, &["--package", APP], "E0300");
        assert!(
            text(&out.stderr).contains("library/src/model.ox"),
            "{out:?}"
        );
    }
    // Library-only main/tests never become the app's entry or suite.
    f.reset();
    ok(&f.invoke("test", &["--package", APP]), PASS_APP);
    f.no_root_outputs();
}

#[test]
fn membership_grants_no_import_visibility() {
    let f = Fixture::new(None);
    // The library cannot import the app by member directory, name or alias.
    for source in [
        "use app::main;\nfn main() -> i32 { return 1; }\n",
        "use wsaccept_app::main;\nfn main() -> i32 { return 1; }\n",
        "use lib::scale;\nfn main() -> i32 { return 1; }\n",
    ] {
        f.write("ws/library/src/lib.ox", source);
        let out = f.invoke("check", &["--package", LIB, "--message-format=json"]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let body = text(&out.stdout);
        assert!(body.contains("\"code\":\"E0205\""), "{body}");
        assert!(body.contains("unknown direct dependency alias"), "{body}");
        assert!(body.contains("ws/library/src/lib.ox"), "{body}");
        assert!(!body.contains("E0500"), "{body}");
    }
    f.reset();
    // Without its explicit [dependencies] entry the app cannot import the
    // library even though both are members.
    f.write(
        "ws/app/oxid.toml",
        APP_MANIFEST.split("\n[dependencies]").next().unwrap(),
    );
    for source in [
        APP_MAIN.to_string(),
        "use library::scale;\nfn main() -> i32 { return scale(1); }\n".to_string(),
        "use wsaccept_library::scale;\nfn main() -> i32 { return scale(1); }\n".to_string(),
    ] {
        f.write("ws/app/src/main.ox", &source);
        let out = f.invoke("check", &["--package", APP, "--message-format=json"]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let body = text(&out.stdout);
        assert!(body.contains("\"code\":\"E0205\""), "{body}");
        assert!(body.contains("ws/app/src/main.ox"), "{body}");
        assert!(!body.contains("E0500"), "{body}");
    }
    // The explicit dependency alias restores the import.
    f.reset();
    ok(&f.invoke("run", &["--package", APP]), b"33\n");
}

#[test]
fn json_early_rejection_legacy_script_and_separator_ownership() {
    let f = Fixture::new(None);
    // JSON check renders selection errors as JSON diagnostics plus summary.
    let out = f.invoke("check", &["--message-format=json"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    let body = text(&out.stdout);
    assert!(body.contains("\"code\":\"E0001\""), "{body}");
    assert!(body.contains(AVAILABLE), "{body}");
    assert!(body.contains("\"kind\":\"check-summary\""), "{body}");
    let good = f.invoke("check", &["--package", APP, "--message-format=json"]);
    assert_eq!(good.status.code(), Some(0), "{good:?}");
    assert!(text(&good.stdout).contains("\"success\":true"));
    // Native RUN JSON and all TEST JSON are refused before manifest I/O, even
    // with a selector and an invalid/missing manifest.
    for manifest in ["ws/missing/oxid.toml", "ws/oxid.toml"] {
        f.write(
            "ws/oxid.toml",
            "[workspace]\nmembers = [\"app\", \"inner\"]\n",
        );
        for (command, extra, needle) in [
            (
                "run",
                vec!["--engine=native"],
                "native manifest run does not support --message-format=json",
            ),
            (
                "test",
                vec![],
                "manifest test does not support --message-format=json",
            ),
            (
                "test",
                vec!["--engine", "native"],
                "manifest test does not support --message-format=json",
            ),
        ] {
            let mut args = vec![
                command,
                "--manifest-path",
                manifest,
                "--package",
                APP,
                "--message-format=json",
            ];
            args.extend(extra);
            let out = f.oxid(&args);
            assert_eq!(out.status.code(), Some(1), "{out:?}");
            assert!(out.stderr.is_empty(), "{out:?}");
            let body = text(&out.stdout);
            assert!(body.contains(needle), "{body}");
            assert!(!body.contains("E0002"), "{body}");
            assert!(!body.contains("nested"), "{body}");
            assert!(!body.contains("test-summary"), "{body}");
        }
    }
    f.reset();
    // Global errors (sticky JSON, duplicate --message-format) outrank the
    // local duplicate selector.
    let out = f.invoke(
        "run",
        &[
            "--package=a",
            "--package=b",
            "--message-format=json",
            "--message-format=text",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(text(&out.stdout).contains("--message-format may only be specified once"));
    assert!(!text(&out.stdout).contains("--package may only"));
    // `--` operands are owned by the command, never parsed as a selector.
    for extra in [
        vec!["--package", APP, "--", "--package", LIB],
        vec!["--", "--package=wsaccept-app"],
    ] {
        f.refuse(
            "run",
            &extra,
            "manifest run does not accept a source operand",
        );
    }
    f.refuse(
        "check",
        &["--package", APP, "extra.ox"],
        "does not accept a source operand",
    );
    f.refuse(
        "run",
        &["--package", APP, "--edition", "legacy-0.9"],
        "conflicts with --edition legacy-0.9",
    );
    f.refuse(
        "check",
        &["--package", APP, "--engine", "native"],
        "--engine is only supported",
    );
    // Legacy test discovery and single-file routes are unchanged; without
    // --manifest-path a --package word is not intercepted.
    f.write("legacy/tests/plain.ox", "print(23);\n");
    let legacy = f.command_in(&f.path("legacy"), &["test"]);
    assert_eq!(legacy.status.code(), Some(0), "{legacy:?}");
    assert!(text(&legacy.stdout).contains("23"), "{legacy:?}");
    f.write("single/hello.ox", "print(23);\n");
    for args in [vec!["hello.ox"], vec!["run", "hello.ox"]] {
        ok(&f.command_in(&f.path("single"), &args), b"23\n");
    }
    // Script arguments, including --manifest-path/--package/--, belong to the script.
    f.write(
        "script/oxid.toml",
        "[project]\nname = \"legacy-script\"\nversion = \"0.1.0\"\n\n[scripts]\nshow = \"/bin/echo\"\n",
    );
    let script = f.command_in(
        &f.path("script"),
        &[
            "script",
            "show",
            "--manifest-path",
            "../ws/oxid.toml",
            "--package=wsaccept-app",
            "--",
            "tail",
        ],
    );
    ok(
        &script,
        b"--manifest-path ../ws/oxid.toml --package=wsaccept-app -- tail\n",
    );
    f.no_root_outputs();
    assert!(!f.path("ws/app/.oxid").exists());
}
