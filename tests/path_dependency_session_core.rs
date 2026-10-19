//! Ordinary path-session controls. Native controls require separately provisioned LLVM.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-path-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let f = Self(root);
        for (path, text) in [
            (
                "app/oxid.toml",
                include_str!("../examples/projects/path_dependency_session/app/oxid.toml"),
            ),
            (
                "app/main.ox",
                include_str!("../examples/projects/path_dependency_session/app/main.ox"),
            ),
            (
                "library/oxid.toml",
                include_str!("../examples/projects/path_dependency_session/library/oxid.toml"),
            ),
            (
                "library/lib.ox",
                include_str!("../examples/projects/path_dependency_session/library/lib.ox"),
            ),
            (
                "library/model.ox",
                include_str!("../examples/projects/path_dependency_session/library/model.ox"),
            ),
            (
                "shared/oxid.toml",
                include_str!("../examples/projects/path_dependency_session/shared/oxid.toml"),
            ),
            (
                "shared/lib.ox",
                include_str!("../examples/projects/path_dependency_session/shared/lib.ox"),
            ),
        ] {
            f.write(path, text);
        }
        f
    }
    fn write(&self, path: &str, text: &str) {
        let p = self.0.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .stdin(Stdio::null())
            .env_remove("OXID_PATH")
            .output()
            .unwrap()
    }
    fn invoke(&self, command: &str, extra: &[&str]) -> Output {
        let mut args = vec![command, "--manifest-path=app/oxid.toml"];
        args.extend_from_slice(extra);
        self.command(&args)
    }
    fn refuse(&self, source: &str, expected: &str) {
        self.write("app/main.ox", source);
        let out = self.invoke("check", &["--message-format=json"]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains(expected), "{text}");
        assert!(text.contains("\"success\":false"), "{text}");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn path_session_real_transitive_opaque_api_check_run_test_and_reload() {
    let f = Fixture::new();
    assert!(f.invoke("check", &[]).status.success());
    let run = f.invoke("run", &[]);
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, b"14\n");
    let suite = f.invoke("test", &[]);
    assert!(suite.status.success(), "{suite:?}");
    assert_eq!(
        suite.stdout,
        b"PASS test_record\nPASS test_scalar\ntest result: 2 passed; 0 failed; 2 total\n"
    );
    f.write("shared/lib.ox", "pub fn number()->i32{return 8;}");
    assert_eq!(f.invoke("run", &[]).stdout, b"16\n");
    let suite = f.invoke("test", &[]);
    assert_eq!(suite.status.code(), Some(1));
    assert!(String::from_utf8(suite.stdout)
        .unwrap()
        .contains("0 passed; 2 failed; 2 total"));
    assert!(!f.0.join("oxid.lock").exists());
    assert!(!f.0.join("app/.oxid").exists());
}

#[test]
fn path_session_dependency_nominal_mismatch_is_a_located_type_error() {
    let f = Fixture::new();
    let source = "use math::model::make;\nfn main()->i32{return make();}\n";
    f.write("app/main.ox", source);
    let out = f.invoke("check", &["--message-format=json"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("\"code\":\"E0300\""), "{text}");
    assert!(!text.contains("E0500"), "{text}");
    assert!(
        text.contains("type mismatch: expected i32, found package#1::model::Record"),
        "{text}"
    );
    assert!(
        text.contains("\"file_id\":0,\"path\":\"app/main.ox\""),
        "{text}"
    );
    let start = source.rfind("make()").unwrap();
    assert!(
        text.contains(&format!("\"start\":{start},\"end\":{}", start + 6)),
        "{text}"
    );
    assert!(text.contains("\"line\":2,"), "{text}");
    assert!(text.contains("\"success\":false"), "{text}");
}

#[test]
fn path_session_visibility_nominal_identity_and_direct_alias_rules() {
    let f = Fixture::new();
    f.refuse("use math::local; fn main()->i32{return local();}", "E0206");
    f.refuse(
        "use math::model::Record; fn main()->i32{let x=Record{n:1};return 0;}",
        "private",
    );
    f.refuse(
        "use math::model::make; fn main()->i32{let x=make();return x.n;}",
        "private",
    );
    f.refuse(
        "use shared::number; fn main()->i32{return number();}",
        "unknown direct dependency alias",
    );
    f.refuse(
        "fn main()->i32{return math::answer();}",
        "unknown enum type",
    );
    f.refuse("use math::model::value; struct Record{n:i32} fn main()->i32{let x=Record{n:7};return value(&x);}","E0300");
    f.write(
        "library/lib.ox",
        "struct Secret{} pub fn leak()->Secret{return Secret{};}",
    );
    f.refuse("fn main()->i32{return 0;}", "E0207");
}

#[test]
fn path_session_cycles_alias_collisions_and_manifest_validation() {
    for (deps, needle) in [
        (
            "math=\"../library\"\nmath=\"../shared\"",
            "duplicate dependency alias",
        ),
        (
            "math=\"../library\"\nother=\"../library/.\"",
            "duplicate canonical dependency target",
        ),
        ("std=\"../library\"", "non-reserved source identifier"),
        ("crate=\"../library\"", "non-reserved source identifier"),
        ("match=\"../library\"", "non-reserved source identifier"),
        ("as=\"../library\"", "non-reserved source identifier"),
        ("bad-name=\"../library\"", "non-reserved source identifier"),
        ("1bad=\"../library\"", "non-reserved source identifier"),
        ("self_dep=\".\"", "dependency cycle"),
    ] {
        let f = Fixture::new();
        f.write(
            "app/oxid.toml",
            &format!(
                "[project]\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[dependencies]\n{deps}\n"
            ),
        );
        let out = f.invoke("check", &[]);
        assert_eq!(out.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(needle),
            "{out:?}"
        );
    }
    let f = Fixture::new();
    f.write(
        "shared/oxid.toml",
        "[project]\nedition=\"typed-preview\"\nentry=\"lib.ox\"\n[dependencies]\napp=\"../app\"\n",
    );
    let a = f.invoke("check", &[]);
    let b = f.invoke("check", &[]);
    assert_eq!(a.stderr, b.stderr);
    assert!(String::from_utf8_lossy(&a.stderr).contains("dependency cycle"));
    for manifest in [
        "[project]\nentry=\"lib.ox\"",
        "[project]\nedition=\"legacy-0.9\"\nentry=\"lib.ox\"",
        "[project]\nedition=\"typed-preview\"\n",
        "[project]\nedition=\"typed-preview\"\nentry=\"../lib.ox\"",
        "[workspace]\n",
    ] {
        f.write("shared/oxid.toml", manifest);
        assert_eq!(f.invoke("check", &[]).status.code(), Some(1));
    }
}

#[test]
fn path_session_whole_graph_preflight_preserves_outputs_and_test_io_rules() {
    let f = Fixture::new();
    f.write(
        "app/.oxid/typed-preview/result/path-session",
        "existing artifact",
    );
    f.write(
        "app/.oxid/typed-preview/result/.path-session.build/current",
        "existing receipt",
    );
    f.write(
        "shared/lib.ox",
        "pub fn number()->i32{return 7;} fn unused()->i32{return false;}",
    );
    for (command, extra) in [
        ("check", vec![]),
        ("run", vec![]),
        ("run", vec!["--engine=native"]),
        ("build", vec![]),
        ("test", vec![]),
        ("test", vec!["--engine=native"]),
    ] {
        let out = f.invoke(command, &extra);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(text.contains("E0300"), "{text}");
        assert!(text.contains("shared/lib.ox"), "{text}");
        assert!(!text.contains("PASS"));
        assert_eq!(
            fs::read(f.0.join("app/.oxid/typed-preview/result/path-session")).unwrap(),
            b"existing artifact"
        );
        assert_eq!(
            fs::read(f.0.join("app/.oxid/typed-preview/result/.path-session.build/current"))
                .unwrap(),
            b"existing receipt"
        );
    }
    f.write(
        "shared/lib.ox",
        "use std::io::read_stdin as read; pub fn number()->i32{return 7;}",
    );
    for extra in [vec![], vec!["--engine=native"]] {
        let out = f.invoke("test", &extra);
        assert_eq!(out.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&out.stderr).contains("read_stdin"));
    }
    f.write(
        "shared/lib.ox",
        "use std::io::ReadStatus; pub fn number()->i32{return 7;}",
    );
    assert!(f.invoke("test", &[]).status.success());
    let out = f.command(&[
        "test",
        "--manifest-path=missing/oxid.toml",
        "--message-format=json",
    ]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.contains("E0001"));
    assert!(!text.contains("E0002"));
}

#[test]
fn path_session_bad_dependency_metadata_aborts_before_publication() {
    let f = Fixture::new();
    f.write(
        "app/.oxid/typed-preview/result/path-session",
        "previous output",
    );
    for manifest in [
        "[project]\nedition=\"legacy-0.9\"\nentry=\"lib.ox\"",
        "[project]\nedition=\"typed-preview\"\nentry=\"absent.ox\"",
        "[project]\nedition=\"typed-preview\"\nentry=\"../lib.ox\"",
        "[project]\nedition=\"typed-preview\"\nentry=broken",
    ] {
        f.write("shared/oxid.toml", manifest);
        for (command, extra) in [
            ("build", vec![]),
            ("run", vec!["--engine=native"]),
            ("test", vec!["--engine=native"]),
        ] {
            let out = f.invoke(command, &extra);
            assert_eq!(out.status.code(), Some(1), "{out:?}");
            assert!(!String::from_utf8_lossy(&out.stderr).contains("clang"));
            assert!(!String::from_utf8_lossy(&out.stdout).contains("PASS"));
            assert_eq!(
                fs::read(f.0.join("app/.oxid/typed-preview/result/path-session")).unwrap(),
                b"previous output"
            );
        }
    }
}

#[test]
#[cfg(unix)]
fn path_session_canonical_target_dedup_does_not_weaken_child_confinement() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    symlink("library", f.0.join("library-alias")).unwrap();
    f.write("app/oxid.toml", "[project]\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[dependencies]\nmath=\"../library\"\nother=\"../library-alias\"\n");
    let out = f.invoke("check", &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("duplicate canonical dependency target"));
    f.write(
        "app/oxid.toml",
        include_str!("../examples/projects/path_dependency_session/app/oxid.toml"),
    );
    fs::remove_file(f.0.join("library/model.ox")).unwrap();
    symlink("../shared/lib.ox", f.0.join("library/model.ox")).unwrap();
    let out = f.invoke("check", &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("symbolic link"));
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[ignore = "requires the separately authorized LLVM 19.1.7/Linux x86_64 toolchain"]
fn path_session_native_build_run_test_and_dependency_rebuild() {
    use std::os::unix::fs::MetadataExt;
    let f = Fixture::new();
    let build = f.invoke("build", &[]);
    assert!(build.status.success(), "{build:?}");
    let artifact = f.0.join("app/.oxid/typed-preview/result/path-session");
    let state_dir =
        f.0.join("app/.oxid/typed-preview/result/.path-session.build");
    let identity = |path: &std::path::Path| match fs::symlink_metadata(path) {
        Ok(meta) => {
            assert!(meta.file_type().is_file(), "{path:?}");
            Some((meta.ino(), fs::read(path).unwrap()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("{path:?}: {error}"),
    };
    let state = || {
        [
            artifact.clone(),
            state_dir.join("owner"),
            state_dir.join("current"),
            state_dir.join("next"),
        ]
        .map(|path| identity(&path))
    };
    let run = Command::new(&artifact).output().unwrap();
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, b"14\n");
    let before = state();
    assert!(before[0].is_some());
    assert!(before[1].is_some());
    assert!(before[2].is_none());
    assert!(before[3].is_some());
    let suite = f.invoke("test", &["--engine=native"]);
    let interpreted = f.invoke("test", &[]);
    assert!(suite.status.success(), "{suite:?}");
    assert!(interpreted.status.success(), "{interpreted:?}");
    assert_eq!(suite.stdout, interpreted.stdout);
    assert_eq!(state(), before);
    let run = f.invoke("run", &["--engine=native"]);
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, b"14\n");
    f.write("shared/lib.ox", "pub fn number()->i32{return 8;}");
    let build = f.invoke("build", &[]);
    assert!(build.status.success(), "{build:?}");
    let run = Command::new(&artifact).output().unwrap();
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, b"16\n");
    let rebuilt = state();
    let suite = f.invoke("test", &["--engine=native"]);
    let interpreted = f.invoke("test", &[]);
    assert_eq!(suite.status.code(), Some(1), "{suite:?}");
    assert_eq!(interpreted.status.code(), Some(1), "{interpreted:?}");
    assert_eq!(
        suite.stdout,
        b"FAIL test_record\nFAIL test_scalar\ntest result: 0 passed; 2 failed; 2 total\n"
    );
    assert_eq!(suite.stdout, interpreted.stdout);
    assert_eq!(state(), rebuilt);
    f.write("shared/lib.ox", "pub fn number()->i32{return false;}");
    assert_eq!(f.invoke("build", &[]).status.code(), Some(1));
    assert_eq!(state(), rebuilt);
}
