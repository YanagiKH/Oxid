//! Ordinary local path-dependency session acceptance (independent of the
//! author's `path_dependency_session_core`). Tests marked `#[ignore]` need
//! LLVM 19.1.7: run them with `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin
//! cargo test --test path_dependency_session_acceptance -- --include-ignored`.
//! Opting in without that env var fails the LLVM gate. No qualification,
//! capacity or fuel probes.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

type FileSnapshot = Option<(u64, u64, Vec<u8>)>;

static NEXT: AtomicUsize = AtomicUsize::new(0);
const NAME: &str = "pathdep-accept-app";
const APP_MANIFEST: &str = "[project]\nname = \"pathdep-accept-app\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\n[dependencies]\nlib = \"../library\"\n";
const APP_MAIN: &str = "use lib::total;\nuse lib::model::make_packet;\nuse lib::model::packet_value;\nuse lib::model::make_signal;\nuse lib::model::signal_score;\nfn main() -> i32 {\n    let p = make_packet();\n    return packet_value(&p) + signal_score(make_signal()) + total();\n}\nfn test_packet() -> bool {\n    let p = make_packet();\n    return packet_value(&p) == 20;\n}\nfn test_signal() -> bool {\n    return signal_score(make_signal()) == 3;\n}\nfn test_total() -> bool {\n    return total() == 23;\n}\n";
const LIB_MANIFEST: &str = "[project]\nname = \"pathdep-accept-library\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/lib.ox\"\n\n[dependencies]\nshared = \"../shared\"\n";
const LIB_ROOT: &str = "pub mod model;\nuse shared::base;\nfn bonus() -> i32 {\n    return 3;\n}\npub fn total() -> i32 {\n    return base() + crate::bonus();\n}\nfn main() -> i32 {\n    return 99;\n}\nfn test_library(x: i32) -> i32 {\n    return x;\n}\n";
const LIB_MODEL: &str = "use shared::base;\npub struct Packet {\n    n: i32\n}\npub enum Signal {\n    Ready(i32),\n    Idle\n}\nfn hidden() -> i32 {\n    return 1;\n}\npub fn make_packet() -> Packet {\n    return Packet { n: base() };\n}\npub fn packet_value(p: &Packet) -> i32 {\n    return p.n;\n}\npub fn make_signal() -> Signal {\n    return Signal::Ready(3);\n}\npub fn signal_score(s: Signal) -> i32 {\n    match s {\n        Signal::Ready(n) => {\n            return n;\n        },\n        Signal::Idle => {\n            return 0;\n        },\n    }\n}\n";
const SHARED_MANIFEST: &str = "[project]\nedition = \"typed-preview\"\nentry = \"src/lib.ox\"\n";
const SHARED_ROOT: &str = "fn seed() -> i32 {\n    return 20;\n}\npub fn base() -> i32 {\n    return crate::seed();\n}\npub struct Packet {\n    n: i32\n}\nfn main() -> i32 {\n    return 77;\n}\nfn test_shared() -> bool {\n    return false;\n}\npub fn make_packet() -> Packet {\n    return Packet { n: base() };\n}\n";
const PASS_ALL: &[u8] = b"PASS test_packet\nPASS test_signal\nPASS test_total\ntest result: 3 passed; 0 failed; 3 total\n";

fn llvm_bin() -> PathBuf {
    let dir =
        PathBuf::from(std::env::var_os("OXID_TEST_LLVM_BIN").expect(
            "native path-dependency acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin)",
        ));
    let version = Command::new(dir.join("clang"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("clang version 19.1.7"),
        "{version:?}"
    );
    dir
}

struct Fixture {
    root: PathBuf,
    llvm: PathBuf,
}
impl Fixture {
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-pathdep-accept-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
        };
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/path_dependency_acceptance");
        for (file, expected) in [
            ("app/oxid.toml", APP_MANIFEST),
            ("app/src/main.ox", APP_MAIN),
            ("library/oxid.toml", LIB_MANIFEST),
            ("library/src/lib.ox", LIB_ROOT),
            ("library/src/model.ox", LIB_MODEL),
            ("shared/oxid.toml", SHARED_MANIFEST),
            ("shared/src/lib.ox", SHARED_ROOT),
        ] {
            let text = fs::read_to_string(example.join(file)).unwrap();
            assert_eq!(text, expected, "checked-in example {file} changed");
            fixture.write(&format!("proj/{file}"), &text);
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
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(cwd)
            .env("OXID_LLVM_BIN", &self.llvm)
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
        let mut args = vec![command, "--manifest-path", "proj/app/oxid.toml"];
        args.extend_from_slice(extra);
        self.oxid(&args)
    }
    fn check_json(&self) -> String {
        let out = self.invoke("check", &["--message-format=json"]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stderr.is_empty(), "{out:?}");
        String::from_utf8(out.stdout).unwrap()
    }
    fn app_out(&self) -> PathBuf {
        self.path(&format!("proj/app/.oxid/typed-preview/result/{NAME}"))
    }
    fn app_state(&self) -> [FileSnapshot; 4] {
        let state = self
            .app_out()
            .parent()
            .unwrap()
            .join(format!(".{NAME}.build"));
        [
            self.app_out(),
            state.join("owner"),
            state.join("current"),
            state.join("next"),
        ]
        .map(|path| optional_identity(&path))
    }
    fn receipt(&self) -> PathBuf {
        self.path(&format!(
            "proj/app/.oxid/typed-preview/result/.{NAME}.build/current"
        ))
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
fn identity(path: &Path) -> (u64, Vec<u8>) {
    let meta = fs::symlink_metadata(path).unwrap();
    assert!(meta.file_type().is_file(), "{path:?}");
    (meta.ino(), fs::read(path).unwrap())
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
fn assert_shared_nominal_diamond(f: &Fixture, engine: &[&str]) {
    f.write(
        "proj/app/oxid.toml",
        &format!("{APP_MANIFEST}common = \"../shared\"\n"),
    );
    f.write(
        "proj/shared/src/lib.ox",
        &format!("{SHARED_ROOT}pub fn packet_value(p: &Packet) -> i32 {{ return p.n; }}\n"),
    );
    f.write(
        "proj/library/src/lib.ox",
        &format!("{LIB_ROOT}use shared::Packet;\nuse shared::packet_value as shared_value;\npub fn shared_packet_value(p: &Packet) -> i32 {{ return shared_value(&*p); }}\n"),
    );
    f.write(
        "proj/app/src/main.ox",
        "use common::make_packet;\nuse lib::shared_packet_value;\nfn main() -> i32 { let p = make_packet(); return shared_packet_value(&p); }\nfn test_shared_packet() -> bool { return main() == 20; }\n",
    );
    let check = f.invoke("check", &[]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    let run = f.invoke("run", engine);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert_eq!(run.stdout, b"20\n");
    assert!(run.stderr.is_empty(), "{run:?}");
    let before = f.app_state();
    let suite = f.invoke("test", engine);
    assert_eq!(suite.status.code(), Some(0), "{suite:?}");
    assert_eq!(
        suite.stdout,
        b"PASS test_shared_packet\ntest result: 1 passed; 0 failed; 1 total\n"
    );
    assert!(suite.stderr.is_empty(), "{suite:?}");
    assert_eq!(f.app_state(), before);
}
fn assert_run46(out: &Output) {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, b"46\n", "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
}

// ---- Interpreter / graph contracts (no toolchain required) ----

#[test]
fn interpreter_transitive_opaque_flow_and_dependency_reload() {
    let f = Fixture::new(None);
    let check = f.invoke("check", &[]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    // The library/shared `main` (99/77) and dependency test_ functions are
    // never the application entry or suite: run is 46 and only 3 cases run.
    assert_run46(&f.invoke("run", &[]));
    let suite = f.invoke("test", &[]);
    assert_eq!(suite.status.code(), Some(0), "{suite:?}");
    assert_eq!(suite.stdout, PASS_ALL, "{suite:?}");
    assert!(suite.stderr.is_empty(), "{suite:?}");
    assert!(!f.path("proj/app/.oxid").exists());
    for package in ["app", "library", "shared"] {
        assert!(!f.path(&format!("proj/{package}/oxid.lock")).exists());
    }
    // Editing the transitive shared dependency changes the next session.
    f.write(
        "proj/shared/src/lib.ox",
        &SHARED_ROOT.replace("return 20;", "return 21;"),
    );
    assert_eq!(f.invoke("run", &[]).stdout, b"48\n");
    let suite = f.invoke("test", &[]);
    assert_eq!(suite.status.code(), Some(1), "{suite:?}");
    assert_eq!(
        suite.stdout,
        b"FAIL test_packet\nPASS test_signal\nFAIL test_total\ntest result: 1 passed; 2 failed; 3 total\n"
    );
    // Editing the library's own crate:: helper only moves its own total.
    f.write("proj/shared/src/lib.ox", SHARED_ROOT);
    f.write(
        "proj/library/src/lib.ox",
        &LIB_ROOT.replace("return 3;", "return 4;"),
    );
    assert_eq!(f.invoke("run", &[]).stdout, b"47\n");
}

#[test]
fn private_indirect_and_nonreexport_rules_are_located_refusals() {
    let f = Fixture::new(None);
    for (source, needle) in [
        ("use lib::bonus;\nfn main() -> i32 { return bonus(); }\n", "\"code\":\"E0206\""),
        (
            "use lib::model::hidden;\nfn main() -> i32 { return hidden(); }\n",
            "\"code\":\"E0206\"",
        ),
        (
            "use lib::model::make_packet;\nfn main() -> i32 { let p = make_packet(); return p.n; }\n",
            "record field is private",
        ),
        (
            "use lib::model::Packet;\nfn main() -> i32 { let p = Packet { n: 1 }; return 0; }\n",
            "private",
        ),
        (
            "use shared::base;\nfn main() -> i32 { return base(); }\n",
            "unknown direct dependency alias",
        ),
        // `base` is a private imported binding in the library, not an
        // original public declaration that the app can import through it.
        (
            "use lib::base;\nfn main() -> i32 { return base(); }\n",
            "\"code\":\"E0205\"",
        ),
        (
            "fn main() -> i32 { return lib::total(); }\n",
            "unknown enum type `lib`",
        ),
    ] {
        f.write("proj/app/src/main.ox", source);
        let body = f.check_json();
        assert!(body.contains(needle), "{body}");
        assert!(body.contains("app/src/main.ox"), "{body}");
        assert!(body.contains("\"success\":false"), "{body}");
        assert!(!body.contains("E0500"), "{body}");
    }
}

#[test]
fn nominal_identity_and_located_type_mismatch_e0300() {
    let f = Fixture::new(None);
    // Same-spelled local record is a different nominal type.
    f.write(
        "proj/app/src/main.ox",
        "use lib::model::packet_value;\nstruct Packet { n: i32 }\nfn main() -> i32 { let p = Packet { n: 7 }; return packet_value(&p); }\n",
    );
    let body = f.check_json();
    assert!(body.contains("\"code\":\"E0300\""), "{body}");
    assert!(!body.contains("E0500"), "{body}");
    // Cross-package same-named records stay distinct even when the shared
    // package is also a direct dependency of the application.
    f.write(
        "proj/app/oxid.toml",
        "[project]\nname = \"pathdep-accept-app\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[dependencies]\nlib = \"../library\"\ncommon = \"../shared\"\n",
    );
    f.write(
        "proj/app/src/main.ox",
        "use lib::model::packet_value;\nuse common::make_packet as shared_packet;\nfn main() -> i32 { let p = shared_packet(); return packet_value(&p); }\n",
    );
    let body = f.check_json();
    assert!(body.contains("\"code\":\"E0300\""), "{body}");
    assert!(!body.contains("E0500"), "{body}");
    // ... while the shared package itself is loaded once: direct and
    // transitive views of its scalar API agree in one compiling session.
    f.write(
        "proj/app/src/main.ox",
        "use lib::model::packet_value;\nuse lib::model::make_packet;\nuse common::base;\nfn main() -> i32 { let p = make_packet(); return packet_value(&p) + base(); }\nfn test_dummy() -> bool { return true; }\n",
    );
    assert_eq!(f.invoke("run", &[]).stdout, b"40\n");
    // A value created via the direct alias must have the same nominal owner
    // as the type the library imports through its transitive edge.
    assert_shared_nominal_diamond(&Fixture::new(None), &[]);
    // Located mismatch: exact file/line/span of the offending call.
    let source = "use lib::model::make_packet;\nfn main() -> i32 { return make_packet(); }\n";
    f.write("proj/app/src/main.ox", source);
    let body = f.check_json();
    assert!(body.contains("\"code\":\"E0300\""), "{body}");
    assert!(!body.contains("E0500"), "{body}");
    assert!(body.contains("app/src/main.ox"), "{body}");
    assert!(body.contains("\"line\":2,"), "{body}");
    let start = source.rfind("make_packet()").unwrap();
    assert!(
        body.contains(&format!("\"start\":{start},\"end\":{}", start + 13)),
        "{body}"
    );
}

#[test]
fn graph_validation_cycles_aliases_edition_and_entry() {
    for (deps, needle) in [
        (
            "lib = \"../library\"\nlib = \"../shared\"\n",
            "duplicate dependency alias",
        ),
        (
            "lib = \"../library\"\nother = \"../library/.\"\n",
            "duplicate canonical dependency target",
        ),
        ("std = \"../library\"\n", "non-reserved source identifier"),
        ("crate = \"../library\"\n", "non-reserved source identifier"),
        ("match = \"../library\"\n", "non-reserved source identifier"),
    ] {
        let f = Fixture::new(None);
        f.write(
            "proj/app/oxid.toml",
            &format!("[project]\nname = \"x\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[dependencies]\n{deps}"),
        );
        let out = f.invoke("check", &[]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(text(&out.stderr).contains(needle), "{out:?}");
    }
    // Symlinked spelling of the same canonical target is also a duplicate.
    let f = Fixture::new(None);
    symlink("library", f.path("proj/library-alias")).unwrap();
    f.write(
        "proj/app/oxid.toml",
        "[project]\nname = \"x\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[dependencies]\nlib = \"../library\"\nother = \"../library-alias\"\n",
    );
    let out = f.invoke("check", &[]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        text(&out.stderr).contains("duplicate canonical dependency target"),
        "{out:?}"
    );
    // A true cycle through the shared package is rejected deterministically.
    let f = Fixture::new(None);
    f.write(
        "proj/shared/oxid.toml",
        "[project]\nedition = \"typed-preview\"\nentry = \"src/lib.ox\"\n[dependencies]\napp = \"../app\"\n",
    );
    let a = f.invoke("check", &[]);
    let b = f.invoke("check", &[]);
    assert_eq!(a.status.code(), Some(1), "{a:?}");
    assert_eq!(a.stderr, b.stderr);
    assert!(text(&a.stderr).contains("dependency cycle"), "{a:?}");
    // Wrong edition / entry problems in a dependency abort the session.
    for manifest in [
        "[project]\nedition = \"legacy-0.9\"\nentry = \"src/lib.ox\"\n",
        "[project]\nentry = \"src/lib.ox\"\n",
        "[project]\nedition = \"typed-preview\"\n",
        "[project]\nedition = \"typed-preview\"\nentry = \"../lib.ox\"\n",
        "[project]\nedition = \"typed-preview\"\nentry = \"src/absent.ox\"\n",
    ] {
        let f = Fixture::new(None);
        f.write("proj/shared/oxid.toml", manifest);
        let out = f.invoke("check", &[]);
        assert_eq!(out.status.code(), Some(1), "{manifest:?} -> {out:?}");
        assert!(!out.stderr.is_empty(), "{out:?}");
        assert!(!f.path("proj/app/.oxid").exists());
    }
}

#[test]
fn whole_graph_type_error_preflight_and_json_legacy_singlefile_routing() {
    let f = Fixture::new(None);
    // An unused body in the transitive dependency is checked before any
    // command runs or publishes, with its own file named.
    f.write(
        "proj/shared/src/lib.ox",
        &format!("{SHARED_ROOT}fn unused() -> i32 {{ return false; }}\n"),
    );
    for (command, extra) in [("check", vec![]), ("run", vec![]), ("test", vec![])] {
        let out = f.invoke(command, &extra);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let combined = format!("{}{}", text(&out.stdout), text(&out.stderr));
        assert!(combined.contains("E0300"), "{combined}");
        assert!(combined.contains("shared/src/lib.ox"), "{combined}");
        assert!(!combined.contains("PASS"), "{combined}");
    }
    assert!(!f.path("proj/app/.oxid").exists());
    f.write("proj/shared/src/lib.ox", SHARED_ROOT);
    // JSON check of a good graph still succeeds through the session.
    let out = f.invoke("check", &["--message-format=json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(
        text(&out.stdout).contains("\"kind\":\"check-summary\""),
        "{out:?}"
    );
    assert!(text(&out.stdout).contains("\"success\":true"), "{out:?}");
    // JSON test is refused before manifest I/O (one E0001 line, no E0002).
    let out = f.oxid(&[
        "test",
        "--manifest-path=proj/missing/oxid.toml",
        "--message-format=json",
    ]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let body = text(&out.stdout);
    assert_eq!(body.lines().count(), 1, "{body}");
    assert!(body.contains("E0001"), "{body}");
    assert!(!body.contains("E0002"), "{body}");
    // Legacy test discovery and single-file routes are unchanged.
    f.write("legacy/tests/plain.ox", "print(23);\n");
    let legacy = f.command_in(&f.path("legacy"), &["test"]);
    assert_eq!(legacy.status.code(), Some(0), "{legacy:?}");
    assert!(text(&legacy.stdout).contains("23"), "{legacy:?}");
    assert!(!text(&legacy.stdout).contains("test_packet"), "{legacy:?}");
    f.write("single/hello.ox", "print(23);\n");
    for args in [vec!["hello.ox"], vec!["run", "hello.ox"]] {
        let out = f.command_in(&f.path("single"), &args);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert_eq!(out.stdout, b"23\n", "{out:?}");
    }
}

#[test]
fn different_cwd_and_explicit_sibling_manifest_paths() {
    let f = Fixture::new(None);
    // From inside the library package, explicitly selecting the sibling app.
    let sibling = f.command_in(
        &f.path("proj/library"),
        &["check", "--manifest-path", "../app/oxid.toml"],
    );
    assert_eq!(sibling.status.code(), Some(0), "{sibling:?}");
    // From a deep directory with a relative path back to the app.
    fs::create_dir_all(f.path("proj/library/src/deep")).unwrap();
    let deep = f.command_in(
        &f.path("proj/library/src/deep"),
        &["run", "--manifest-path=../../../app/oxid.toml"],
    );
    assert_run46(&deep);
    // Absolute manifest from `/`, and inside the app with a bare filename.
    let absolute = f.path("proj/app/oxid.toml");
    let from_root = f.command_in(
        Path::new("/"),
        &["run", "--manifest-path", absolute.to_str().unwrap()],
    );
    assert_run46(&from_root);
    let inside = f.command_in(&f.path("proj/app"), &["run", "--manifest-path=oxid.toml"]);
    assert_run46(&inside);
    assert!(!f.path("proj/library/src/deep/.oxid").exists());
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

// ---- Native execution (LLVM 19.1.7) ----

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn native_build_run_test_standalone_and_rebuild_preserves_on_failure() {
    let f = Fixture::native();
    let build = f.invoke("build", &[]);
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    assert!(
        text(&build.stdout).contains("typed-preview native build ok"),
        "{build:?}"
    );
    let artifact = f.app_out();
    assert!(artifact.is_file());
    // The artifact runs standalone, away from the project and toolchain.
    let standalone = Command::new(&artifact)
        .current_dir("/")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(standalone.status.code(), Some(0), "{standalone:?}");
    assert_eq!(standalone.stdout, b"46\n");
    // The first publication has an owner and next, but no current yet.
    // Native TEST must preserve both present and absent receipt slots.
    let first_publication = f.app_state();
    assert!(first_publication[0].is_some());
    assert!(first_publication[1].is_some());
    assert!(first_publication[2].is_none());
    assert_eq!(first_publication[3], first_publication[0]);
    let first_suite = f.invoke("test", &["--engine", "native"]);
    assert_eq!(first_suite.status.code(), Some(0), "{first_suite:?}");
    assert_eq!(first_suite.stdout, PASS_ALL);
    assert_eq!(f.app_state(), first_publication);
    assert_run46(&f.invoke("run", &["--engine", "native"]));
    // Native TEST agrees with the interpreter and touches neither the app
    // artifact nor its ownership receipt.
    let before = identity(&artifact);
    let receipt_before = identity(&f.receipt());
    let state_before = f.app_state();
    let native_suite = f.invoke("test", &["--engine", "native"]);
    assert_eq!(native_suite.status.code(), Some(0), "{native_suite:?}");
    assert_eq!(native_suite.stdout, PASS_ALL);
    assert_eq!(native_suite.stdout, f.invoke("test", &[]).stdout);
    assert_eq!(identity(&artifact), before);
    assert_eq!(identity(&f.receipt()), receipt_before);
    assert_eq!(f.app_state(), state_before);
    // A transitive dependency edit rebuilds to the new fixed result.
    f.write(
        "proj/shared/src/lib.ox",
        &SHARED_ROOT.replace("return 20;", "return 21;"),
    );
    let build = f.invoke("build", &[]);
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    let rebuilt = identity(&artifact);
    assert_ne!(rebuilt.1, before.1);
    let standalone = Command::new(&artifact)
        .current_dir("/")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(standalone.stdout, b"48\n");
    let state_before_failure = f.app_state();
    let native_suite = f.invoke("test", &["--engine", "native"]);
    assert_eq!(native_suite.status.code(), Some(1), "{native_suite:?}");
    assert_eq!(native_suite.stdout, f.invoke("test", &[]).stdout);
    assert_eq!(f.app_state(), state_before_failure);
    // A type error in the dependency then fails every native command before
    // publication/execution and leaves the rebuilt artifact in place.
    f.write(
        "proj/shared/src/lib.ox",
        &format!(
            "{}fn unused() -> i32 {{ return false; }}\n",
            SHARED_ROOT.replace("return 20;", "return 21;")
        ),
    );
    // `.build/current` retains the previously published artifact (it lags
    // one publication), so the invariant is that its identity is untouched,
    // not that its bytes equal the live artifact.
    let receipt_at_failure = identity(&f.receipt());
    assert_eq!(receipt_at_failure.1, before.1);
    let state_at_failure = f.app_state();
    for (command, extra) in [
        ("build", vec![]),
        ("run", vec!["--engine", "native"]),
        ("test", vec!["--engine", "native"]),
    ] {
        let out = f.invoke(command, &extra);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(!text(&out.stdout).contains("PASS"), "{out:?}");
        assert_eq!(identity(&artifact), rebuilt);
        assert_eq!(identity(&f.receipt()), receipt_at_failure);
        assert_eq!(f.app_state(), state_at_failure);
    }
    // Exercise the same direct/transitive nominal crossing with real LLVM.
    assert_shared_nominal_diamond(&Fixture::native(), &["--engine", "native"]);
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn example_readme_commands_from_acceptance_root() {
    let f = Fixture::native();
    fs::rename(f.path("proj"), f.path("path_dependency_acceptance")).unwrap();
    let manifest = "path_dependency_acceptance/app/oxid.toml";
    let check = f.oxid(&["check", "--manifest-path", manifest]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    assert_run46(&f.oxid(&["run", "--manifest-path", manifest]));
    for engine in [vec![], vec!["--engine", "native"]] {
        let mut args = vec!["test", "--manifest-path", manifest];
        args.extend(engine);
        let out = f.oxid(&args);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert_eq!(out.stdout, PASS_ALL, "{out:?}");
    }
    let build = f.oxid(&["build", "--manifest-path", manifest]);
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    let artifact =
        f.path("path_dependency_acceptance/app/.oxid/typed-preview/result/pathdep-accept-app");
    let standalone = Command::new(&artifact)
        .current_dir("/")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(standalone.stdout, b"46\n");
    assert_run46(&f.oxid(&["run", "--manifest-path", manifest, "--engine", "native"]));
    let readme = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/path_dependency_acceptance/README.md"),
    )
    .unwrap();
    assert!(readme.contains("LLVM 19.1.7"));
    assert!(readme.contains("46"));
    assert!(readme.contains("pathdep-accept-app"));
}
