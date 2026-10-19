//! Ordinary explicit-workspace controls; real native execution is opt-in.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const ROOT: &str = include_str!("../examples/projects/typed_workspace_selection/oxid.toml");
const APP: &str = include_str!("../examples/projects/typed_workspace_selection/app/oxid.toml");
const LIB: &str = include_str!("../examples/projects/typed_workspace_selection/library/oxid.toml");
const PASS: &[u8] = b"PASS test_answer\nPASS test_unit\ntest result: 2 passed; 0 failed; 2 total\n";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-workspace-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let f = Self(root);
        for (path, text) in [
            ("oxid.toml", ROOT),
            ("app/oxid.toml", APP),
            ("library/oxid.toml", LIB),
            (
                "app/main.ox",
                include_str!("../examples/projects/typed_workspace_selection/app/main.ox"),
            ),
            (
                "library/lib.ox",
                include_str!("../examples/projects/typed_workspace_selection/library/lib.ox"),
            ),
        ] {
            f.write(path, text);
        }
        f
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn command(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .env_remove("OXID_PATH")
            .output()
            .unwrap()
    }
    fn invoke(&self, command: &str, extra: &[&str]) -> Output {
        let mut args = vec![command, "--manifest-path=oxid.toml"];
        args.extend_from_slice(extra);
        self.command(&self.0, &args)
    }
    fn refuse(&self, extra: &[&str], needle: &str) {
        let out = self.invoke("check", extra);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stdout.is_empty(), "{out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(needle),
            "{out:?}"
        );
    }
    fn no_root_outputs(&self) {
        assert!(!self.0.join(".oxid").exists());
        for prefix in ["", "app", "library"] {
            assert!(!self.0.join(prefix).join("oxid.lock").exists());
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn success(out: Output, stdout: &[u8]) {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, stdout);
    assert!(out.stderr.is_empty(), "{out:?}");
}

#[test]
fn workspace_commands_select_one_root_and_preserve_cwd_independent_results() {
    let f = Fixture::new();
    let check = f.invoke(
        "check",
        &["--package=workspace-app", "--message-format=json"],
    );
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    assert!(check.stderr.is_empty());
    assert!(String::from_utf8_lossy(&check.stdout).contains("\"success\":true"));
    for (command, expected) in [("run", b"7\n".as_slice()), ("test", PASS)] {
        let ordinary = f.invoke(command, &["--package", "workspace-app"]);
        assert_eq!(ordinary.status.code(), Some(0), "{ordinary:?}");
        assert_eq!(ordinary.stdout, expected);
        assert!(ordinary.stderr.is_empty());
        let absolute = f.0.join("oxid.toml");
        let relocated = f.command(
            &f.0.join("library"),
            &[
                command,
                "--manifest-path",
                absolute.to_str().unwrap(),
                "--package=workspace-app",
            ],
        );
        assert_eq!(relocated.status.code(), Some(0));
        assert_eq!(relocated.stdout, ordinary.stdout);
        assert_eq!(relocated.stderr, ordinary.stderr);
    }
    success(f.invoke("run", &["--package=workspace-library"]), b"91\n");
    success(
        f.invoke("test", &["--package=workspace-library"]),
        b"PASS test_library\ntest result: 1 passed; 0 failed; 1 total\n",
    );
    f.write("oxid.toml", "[workspace]\nmembers=[\"app\"]\n");
    success(f.invoke("run", &[]), b"7\n");
    success(f.invoke("test", &[]), PASS);
    f.no_root_outputs();
    assert!(!f.0.join("app/.oxid").exists());
}

#[test]
fn workspace_selection_errors_validate_all_member_manifests_before_sources() {
    let f = Fixture::new();
    fs::remove_file(f.0.join("app/main.ox")).unwrap();
    f.refuse(&[], "available packages: workspace-app, workspace-library");
    f.refuse(&["--package=absent"], "unknown workspace package");
    f.refuse(&["--package=app"], "unknown workspace package");
    f.refuse(
        &["--package=workspace-app", "--package=workspace-library"],
        "--package may only be specified once",
    );
    for (manifest, needle) in [
        (
            LIB.replace("workspace-library", "workspace-app"),
            "duplicate workspace member name",
        ),
        (LIB.replace("workspace-library", ""), "name must match"),
        (
            LIB.replace("name = \"workspace-library\"\n", ""),
            "requires [project].name",
        ),
        (
            LIB.replace("typed-preview", "legacy-0.9"),
            "requires [project].edition",
        ),
        (
            "[project]\nname=broken".into(),
            "expected double-quoted value",
        ),
        (
            "[workspace]\nmembers=[\"../app\"]".into(),
            "nested workspaces",
        ),
    ] {
        f.write("library/oxid.toml", &manifest);
        f.refuse(&["--package=workspace-app"], needle);
        f.refuse(&["--package=absent"], needle);
    }
    f.write("library/oxid.toml", LIB);
    f.write("oxid.toml", "[workspace]\nmembers=[\".\"]\n");
    f.refuse(&[], "workspace membership cycle");
    f.write("oxid.toml", "[workspace]\nmembers=[\"app\",\"app/.\"]\n");
    f.refuse(&[], "duplicate canonical workspace member");
    f.no_root_outputs();
}

#[test]
fn workspace_membership_is_not_an_import_or_source_execution_edge() {
    let f = Fixture::new();
    f.write("app/oxid.toml", APP.split("[dependencies]").next().unwrap());
    f.write("library/lib.ox", "not valid source");
    f.write(
        "app/main.ox",
        "fn main()->i32{return 7;} fn test_app()->bool{return true;}",
    );
    success(f.invoke("run", &["--package=workspace-app"]), b"7\n");
    assert_eq!(
        f.invoke("test", &["--package=workspace-app"]).status.code(),
        Some(0)
    );
    f.write(
        "app/main.ox",
        "use math::number; fn main()->i32{return number();}",
    );
    f.refuse(
        &["--package=workspace-app"],
        "unknown direct dependency alias",
    );
    f.write("app/oxid.toml", APP);
    let out = f.invoke(
        "check",
        &["--package=workspace-app", "--message-format=json"],
    );
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("library/lib.ox"), "{text}");
    assert!(!text.contains("E0500"), "{text}");
    f.no_root_outputs();
}

#[test]
fn workspace_cli_rejections_keep_json_and_direct_project_behavior() {
    let f = Fixture::new();
    let direct = f.command(&f.0, &["run", "--manifest-path=app/oxid.toml"]);
    assert_eq!(direct.status.code(), Some(0));
    assert_eq!(direct.stdout, b"7\n");
    let out = f.command(
        &f.0,
        &[
            "check",
            "--manifest-path=app/oxid.toml",
            "--package=workspace-app",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("direct project manifests do not accept it")
    );
    for command in ["run", "test"] {
        let out = f.command(
            &f.0,
            &[
                command,
                "--manifest-path=missing",
                "--package=workspace-app",
                "--engine=native",
                "--message-format=json",
            ],
        );
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stderr.is_empty());
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            text.contains("does not support --message-format=json"),
            "{text}"
        );
        assert!(!text.contains("E0002"));
        assert!(!text.contains("test-summary"));
    }
    let out = f.command(
        &f.0,
        &[
            "run",
            "--manifest-path=missing",
            "--package=a",
            "--package=b",
            "--message-format=json",
            "--message-format=text",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.is_empty());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--message-format may only be specified once"));
    assert!(text.contains("check-summary"));
}

#[test]
fn workspace_preflight_preserves_selected_artifacts_for_all_native_routes() {
    let f = Fixture::new();
    let artifact = "app/.oxid/typed-preview/result/workspace-app";
    let receipt = "app/.oxid/typed-preview/result/.workspace-app.build/current";
    f.write(artifact, "existing application");
    f.write(receipt, "existing receipt");
    for (path, text, needle) in [
        ("library/oxid.toml", "[project]\nname=broken", "E0001"),
        (
            "library/lib.ox",
            "pub fn number()->i32{return false;}",
            "E0300",
        ),
    ] {
        f.write("library/oxid.toml", LIB);
        f.write(path, text);
        for command in ["build", "run", "test"] {
            let mut args = vec!["--package=workspace-app"];
            if command != "build" {
                args.push("--engine=native");
            }
            let out = f.invoke(command, &args);
            assert_eq!(out.status.code(), Some(1), "{out:?}");
            assert!(out.stdout.is_empty(), "{out:?}");
            let error = String::from_utf8_lossy(&out.stderr);
            assert!(error.contains(needle), "{error}");
            assert!(!error.contains("clang"));
            assert_eq!(
                fs::read(f.0.join(artifact)).unwrap(),
                b"existing application"
            );
            assert_eq!(fs::read(f.0.join(receipt)).unwrap(), b"existing receipt");
        }
    }
    f.no_root_outputs();
}

#[test]
#[cfg(unix)]
fn workspace_canonical_duplicates_and_explicit_sibling_escaped_paths() {
    let f = Fixture::new();
    std::os::unix::fs::symlink("library", f.0.join("alias")).unwrap();
    f.write(
        "oxid.toml",
        "[workspace]\nmembers=[\"library\",\"alias\"]\n",
    );
    f.refuse(&[], "duplicate canonical workspace member");
    fs::rename(f.0.join("library"), f.0.join("comma,library")).unwrap();
    f.write(
        "app/oxid.toml",
        &APP.replace("../library", "../comma,library"),
    );
    f.write(
        "virtual/oxid.toml",
        "[workspace]\nmembers=[\"../ap\\u0070\", # explicit sibling\n\"../comma,library\",]\n",
    );
    let out = f.command(
        &f.0,
        &[
            "run",
            "--manifest-path=virtual/oxid.toml",
            "--package=workspace-app",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, b"7\n");
    assert!(out.stderr.is_empty());
    f.no_root_outputs();
    assert!(!f.0.join("virtual/.oxid").exists());
}

#[test]
#[ignore = "requires separately authorized LLVM 19.1.7/Linux x86_64"]
fn workspace_native_outputs_belong_to_selected_member_and_tests_are_isolated() {
    let f = Fixture::new();
    let build = f.invoke("build", &["--package=workspace-app"]);
    assert_eq!(build.status.code(), Some(0), "{build:?}");
    let artifact = f.0.join("app/.oxid/typed-preview/result/workspace-app");
    // First publication owns the artifact through next; current does not exist yet.
    let receipt =
        f.0.join("app/.oxid/typed-preview/result/.workspace-app.build/next");
    let bytes = fs::read(&artifact).unwrap();
    assert_eq!(fs::read(&receipt).unwrap(), bytes);
    let native = f.invoke("run", &["--package=workspace-app", "--engine=native"]);
    assert_eq!(native.status.code(), Some(0), "{native:?}");
    assert_eq!(native.stdout, b"7\n");
    assert!(native.stderr.is_empty());
    let suite = f.invoke("test", &["--package=workspace-app", "--engine=native"]);
    assert_eq!(suite.status.code(), Some(0), "{suite:?}");
    assert_eq!(suite.stdout, PASS);
    assert!(suite.stderr.is_empty());
    f.write("app/main.ox", "fn main()->i32{return 7;} fn test_bad()->i32{return -256;} fn test_good()->bool{return true;}");
    let interpreter = f.invoke("test", &["--package=workspace-app"]);
    let native = f.invoke("test", &["--package=workspace-app", "--engine=native"]);
    assert_eq!(native.status.code(), Some(1));
    assert_eq!(interpreter.status.code(), Some(1));
    assert_eq!(
        native.stdout,
        b"FAIL test_bad\nPASS test_good\ntest result: 1 passed; 1 failed; 2 total\n"
    );
    assert_eq!(native.stdout, interpreter.stdout);
    assert!(native.stderr.is_empty());
    assert_eq!(fs::read(&artifact).unwrap(), bytes);
    assert_eq!(fs::read(&receipt).unwrap(), bytes);
    f.write("library/lib.ox", "pub fn number()->i32{return false;}");
    assert_eq!(
        f.invoke("build", &["--package=workspace-app"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(fs::read(&artifact).unwrap(), bytes);
    f.no_root_outputs();
}
