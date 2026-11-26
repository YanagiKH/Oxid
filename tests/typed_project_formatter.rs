//! Typed formatting follows project source declarations without loading dependencies.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-package-format-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), "[project]\nname=\"format-example\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[dependencies]\nmath=\"../missing-package\"\n").unwrap();
        Self(root)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .args(args)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .output()
            .unwrap()
    }
    fn unchanged(&self, source: &str) {
        assert_eq!(
            fs::read_to_string(self.0.join("src/main.ox")).unwrap(),
            source
        );
        for path in [".oxid", "cache", "oxid.lock"] {
            assert!(!self.0.join(path).exists(), "{path}");
        }
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn package_edition_formats_named_files_without_resolving_missing_dependencies() {
    let source = "use crate::math::missing;fn main()->i32{return missing();}";
    let project = Project::new(source);
    let inherited = project.run(&["fmt", "src/main.ox"]);
    let explicit = project.run(&["fmt", "--edition=typed-preview", "src/main.ox"]);
    assert!(inherited.status.success(), "{inherited:?}");
    assert!(explicit.status.success(), "{explicit:?}");
    assert_eq!(inherited.stdout, explicit.stdout);
    assert!(inherited.stderr.is_empty());
    assert!(String::from_utf8(inherited.stdout)
        .unwrap()
        .contains("return missing();"));
    project.unchanged(source);
}

#[test]
fn package_format_check_reports_only_formatting_drift_and_preserves_input() {
    let source = "fn main()->char{return '🦀';}";
    let project = Project::new(source);
    let drift = project.run(&["fmt", "--check", "src/main.ox"]);
    assert_eq!(drift.status.code(), Some(1));
    assert!(drift.stdout.is_empty());
    assert_eq!(drift.stderr, b"typed-preview fmt: formatting required\n");
    project.unchanged(source);
    let formatted = project.run(&["fmt", "src/main.ox"]);
    assert!(formatted.status.success(), "{formatted:?}");
    fs::write(project.0.join("src/main.ox"), &formatted.stdout).unwrap();
    let clean = project.run(&["fmt", "src/main.ox", "--check"]);
    assert!(clean.status.success(), "{clean:?}");
    assert!(clean.stdout.is_empty());
    assert!(clean.stderr.is_empty());
    project.unchanged(std::str::from_utf8(&formatted.stdout).unwrap());
}

#[test]
fn project_format_check_visits_modules_and_declared_tests_without_dependencies() {
    let main = "mod helper;fn main()->i32{return crate::helper::value();}";
    let helper = "pub fn value()->i32{return 42;}";
    let test = "fn main()->i32{return 0;}";
    let project = Project::new(main);
    let root = &project.0;
    fs::write(root.join("src/helper.ox"), helper).unwrap();
    fs::write(root.join("src/unrelated.ox"), "!").unwrap();
    fs::create_dir(root.join("tests")).unwrap();
    fs::write(root.join("tests/smoke.ox"), test).unwrap();
    let manifest = root.join("oxid.toml");
    let mut contents = fs::read_to_string(&manifest).unwrap();
    contents.push_str("[tests]\nsmoke = \"tests/smoke.ox\"\n");
    fs::write(&manifest, &contents).unwrap();

    let files = [
        ("src/helper.ox", helper),
        ("src/main.ox", main),
        ("tests/smoke.ox", test),
    ];
    let result = project.run(&["fmt", "--check"]);
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert!(result.stdout.is_empty());
    let reported = String::from_utf8(result.stderr).unwrap().replace('\\', "/");
    assert_eq!(
        reported,
        files
            .map(|(path, _)| format!("formatting required for {path}\n"))
            .concat()
    );
    let mut expected = Vec::new();
    for (path, before) in files {
        assert_eq!(fs::read_to_string(root.join(path)).unwrap(), before);
        let formatted = project.run(&["fmt", path]);
        assert!(formatted.status.success(), "{formatted:?}");
        expected.push((path, formatted.stdout));
    }
    let source = root.join("src/main.ox");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&source, fs::Permissions::from_mode(0o740)).unwrap();
    }
    let permissions = fs::metadata(&source).unwrap().permissions();
    let wrote = project.run(&["fmt", "--write"]);
    assert!(wrote.status.success(), "{wrote:?}");
    for (path, formatted) in expected {
        assert_eq!(fs::read(root.join(path)).unwrap(), formatted);
    }
    assert_eq!(fs::metadata(source).unwrap().permissions(), permissions);
    let clean = project.run(&["fmt", "--check", "--edition=typed-preview"]);
    assert!(
        clean.status.success() && clean.stdout.is_empty() && clean.stderr.is_empty(),
        "{clean:?}"
    );
    assert_eq!(
        fs::read_to_string(root.join("src/unrelated.ox")).unwrap(),
        "!"
    );
    assert_eq!(fs::read_to_string(manifest).unwrap(), contents);
    for path in [".oxid", "cache", "oxid.lock"] {
        assert!(!root.join(path).exists(), "{path}");
    }
}

#[test]
fn workspace_format_check_uses_default_explicit_and_all_member_selection() {
    let project = Project::new("");
    let root = &project.0;
    fs::write(root.join("oxid.toml"), "[workspace]\nedition=\"typed-preview\"\nmembers=[\"app\",\"lib\"]\ndefault-members=[\"app\"]\n").unwrap();
    fs::create_dir_all(root.join("app/src")).unwrap();
    fs::create_dir_all(root.join("lib/src")).unwrap();
    fs::write(root.join("app/oxid.toml"), "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[dependencies]\nexternal=\"../missing\"\n").unwrap();
    fs::write(root.join("lib/oxid.toml"), "[project]\nname=\"library\"\nedition=\"typed-preview\"\nkind=\"lib\"\nentry=\"src/lib.ox\"\n").unwrap();
    let app = root.join("app/src/main.ox");
    let library = root.join("lib/src/lib.ox");
    fs::write(&app, "fn main()->i32{return 42;}").unwrap();
    fs::write(&library, "pub fn value()->i32{return 42;}").unwrap();
    let formatted = project.run(&["fmt", "--edition=typed-preview", "app/src/main.ox"]);
    assert!(formatted.status.success(), "{formatted:?}");
    fs::write(&app, &formatted.stdout).unwrap();

    let default = project.run(&["fmt", "--check"]);
    assert!(
        default.status.success() && default.stderr.is_empty(),
        "{default:?}"
    );
    for args in [
        &["fmt", "--check", "--workspace"][..],
        &["fmt", "--check", "-p", "library"],
    ] {
        let result = project.run(args);
        assert_eq!(result.status.code(), Some(1), "{result:?}");
        assert!(result.stdout.is_empty());
        assert_eq!(
            String::from_utf8(result.stderr).unwrap().replace('\\', "/"),
            "formatting required for lib/src/lib.ox\n"
        );
    }
    assert_eq!(fs::read(app).unwrap(), formatted.stdout);
    assert_eq!(
        fs::read_to_string(&library).unwrap(),
        "pub fn value()->i32{return 42;}"
    );
    let changed = project.run(&["fmt", "--write", "-p", "library"]);
    assert!(changed.status.success(), "{changed:?}");
    let clean = project.run(&["fmt", "--check", "--workspace"]);
    assert!(clean.status.success(), "{clean:?}");
    let source = "fn main()->i32{return 42;}";
    let app = root.join("app/src/main.ox");
    fs::write(&app, source).unwrap();
    fs::write(&library, "pub fn incomplete(").unwrap();
    let failed = project.run(&["fmt", "--write", "--workspace"]);
    assert_eq!(failed.status.code(), Some(1), "{failed:?}");
    assert_eq!(fs::read_to_string(&app).unwrap(), source);
    fs::write(&library, "pub fn value()->i32{return 42;}").unwrap();
    let changed = project.run(&["fmt", "--write", "--workspace"]);
    assert!(changed.status.success(), "{changed:?}");
    let clean = project.run(&["fmt", "--check", "--workspace"]);
    assert!(clean.status.success(), "{clean:?}");
    assert!(!root.join("oxid.lock").exists() && !root.join(".oxid").exists());
}

#[test]
fn project_format_write_checks_every_source_before_replacing_files() {
    let source = "fn main()->i32{return 0;}";
    let project = Project::new(source);
    let mut manifest = fs::OpenOptions::new()
        .append(true)
        .open(project.0.join("oxid.toml"))
        .unwrap();
    use std::io::Write;
    writeln!(manifest, "[tests]\nsmoke=\"tests/smoke.ox\"").unwrap();
    drop(manifest);
    let test = project.0.join("tests/smoke.ox");
    fs::create_dir(test.parent().unwrap()).unwrap();
    fs::write(&test, "fn incomplete(").unwrap();
    let result = project.run(&["fmt", "--write"]);
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert_eq!(fs::read_to_string(test).unwrap(), "fn incomplete(");
    project.unchanged(source);
    assert_eq!(fs::read_dir(project.0.join("src")).unwrap().count(), 1);
}

#[test]
fn source_less_write_needs_a_typed_manifest() {
    let source = "fn main()->i32{return 0;}";
    let project = Project::new(source);
    let manifest = project.0.join("oxid.toml");
    fs::write(&manifest, "[project]\nname=\"legacy\"\n").unwrap();
    for args in [
        &["fmt", "--write"][..],
        &["fmt", "--write", "--edition=legacy-0.9"],
    ] {
        let rejected = project.run(args);
        assert_eq!(rejected.status.code(), Some(1), "{rejected:?}");
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("requires a typed project"));
    }
    fs::remove_file(manifest).unwrap();
    let rejected = project.run(&["fmt", "--write"]);
    assert_eq!(rejected.status.code(), Some(1), "{rejected:?}");
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("require an oxid.toml manifest"));
    project.unchanged(source);
}

#[test]
fn project_graph_operations_keep_their_named_source_restriction() {
    let source = "fn main()->i32{return 0;}";
    let project = Project::new(source);
    for command in ["check", "run", "compile"] {
        let output = project.run(&[command, "src/main.ox"]);
        assert_eq!(output.status.code(), Some(1), "{command}: {output:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("package dependencies require a source-less project command"));
    }
    project.unchanged(source);
}

#[test]
fn format_rejects_directory_and_lock_options_before_touching_project_outputs() {
    let source = "fn main()->i32{return 0;}";
    let project = Project::new(source);
    for (args, message) in [
        (vec!["fmt", "."], "requires one source file"),
        (
            vec!["fmt", "--check", "--write"],
            "use oxid fmt --check or oxid fmt --write",
        ),
        (
            vec!["fmt", "src/main.ox", "--locked"],
            "locking options require a typed project lock, check, run, build, test or doc command",
        ),
    ] {
        let output = project.run(&args);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains(message));
    }
    project.unchanged(source);
}
