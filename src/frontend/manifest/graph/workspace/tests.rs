use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-workspace-unit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn manifest(&self) -> String {
        self.0.join("oxid.toml").to_str().unwrap().into()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn workspace_array_grammar_uses_manifest_strings_and_separators() {
    let text = "# root\r\n[workspace]\r\nmembers = [\n \"app\", # comment\n \"comma,dir\", \"lib\\u0072ary\",\n]\n";
    let fields = members(
        "w",
        header("w", text).unwrap().unwrap(),
        &mut Budget::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let names: Vec<_> = fields
        .into_iter()
        .map(|field| decoded(text, field, &mut Allocator::default()).unwrap())
        .collect();
    assert_eq!(names, ["app", "comma,dir", "library"]);
    for suffix in [
        "members=[]",
        "members=\"app\"",
        "members=[app]",
        "members=[\"app\" \"library\"]",
        "members=[\"app\",,]",
        "members=[\"app\"",
        "members=[\"app\"] trailing",
        "members=[\"app\"]\nmembers=[\"library\"]",
        "default-members=[\"app\"]",
        "members=[\"app\"]\n[project]",
        "members=[\"app\"]\n[dependencies]",
        "members=[\"app\"]\n[workspace]",
        "members=[\"bad\\q\"]",
    ] {
        let text = format!("[workspace]\n{suffix}");
        assert!(
            members(
                "w",
                header("w", &text).unwrap().unwrap(),
                &mut Budget::default(),
                &mut Allocator::default()
            )
            .is_err(),
            "{text}"
        );
    }
}

#[test]
fn workspace_selection_reuses_member_snapshots_and_one_manifest_budget() {
    let f = Fixture::new();
    let root = "[workspace]\nmembers=[\"app\",\"library\",\"unused\"]\n";
    let app = "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[dependencies]\nmath=\"../library\"\n";
    let library = "[project]\nname=\"library\"\nedition=\"typed-preview\"\nentry=\"lib.ox\"\n";
    let unused = "[project]\nname=\"unused\"\nedition=\"typed-preview\"\nentry=\"missing.ox\"\n";
    for (path, text) in [
        ("oxid.toml", root),
        ("app/oxid.toml", app),
        ("library/oxid.toml", library),
        ("unused/oxid.toml", unused),
    ] {
        f.write(path, text);
    }
    let mut budget = Budget::default();
    let mut allocator = Allocator::default();
    let selected = select(
        &f.manifest(),
        root.as_bytes(),
        Some("app"),
        Command::Check,
        false,
        &mut budget,
        &mut allocator,
    )
    .unwrap();
    assert_eq!(
        budget.manifest_bytes,
        root.len() + app.len() + library.len() + unused.len()
    );
    assert_eq!(selected.cached[1]._raw, library.as_bytes());
    // Graph resolution must consume the already validated bytes, never reopen it.
    f.write("library/oxid.toml", "[project]\nname=broken");
    let bytes = budget.manifest_bytes;
    let selection = finish(selected, Command::Check, &mut budget, &mut allocator).unwrap();
    assert_eq!(budget.manifest_bytes, bytes);
    assert_eq!(selection.graph.packages.len(), 2);
    assert_eq!(selection.graph.packages[1].name, "library");
    assert!(selection.parent.ends_with("app/"));
    assert!(!f.0.join("app/main.ox").exists());
}

#[test]
fn workspace_unselected_manifests_and_dependency_reads_share_input_bound() {
    let f = Fixture::new();
    let root = "[workspace]\nmembers=[\"app\",\"unused\"]\n";
    let app = "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[dependencies]\nlib=\"../library\"\n";
    let tail = format!("#{}\n", "x".repeat(8500));
    f.write("oxid.toml", root);
    f.write("app/oxid.toml", app);
    f.write(
        "unused/oxid.toml",
        &format!(
            "[project]\nname=\"unused\"\nedition=\"typed-preview\"\nentry=\"missing.ox\"\n{tail}"
        ),
    );
    f.write(
        "library/oxid.toml",
        &format!("[project]\nedition=\"typed-preview\"\nentry=\"lib.ox\"\n{tail}"),
    );
    let mut budget = Budget::default();
    let mut allocator = Allocator::default();
    let selected = select(
        &f.manifest(),
        root.as_bytes(),
        Some("app"),
        Command::Check,
        false,
        &mut budget,
        &mut allocator,
    )
    .unwrap();
    let error = finish(selected, Command::Check, &mut budget, &mut allocator)
        .err()
        .unwrap();
    assert_eq!(error.classification().0, "E0400");
    assert!(error.message().contains("aggregate manifest input"));
}

#[test]
fn workspace_cli_keeps_global_priority_json_and_argument_ownership() {
    use crate::frontend::options;
    let route =
        |args: &[&str]| options::route(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>());
    match route(&[
        "run",
        "--manifest-path=absent",
        "--package=app",
        "--package=lib",
        "--edition=bad",
        "--message-format=json",
    ]) {
        Route::ManifestError {
            failure,
            json,
            operation,
        } => {
            assert!(json);
            assert_eq!(operation, Operation::Check);
            assert!(failure.message().starts_with("unknown edition"));
        }
        other => panic!("{other:?}"),
    }
    for args in [
        vec![
            "run",
            "--manifest-path=absent",
            "--engine=native",
            "--package=app",
            "--message-format=json",
        ],
        vec![
            "test",
            "--manifest-path=absent",
            "--engine=native",
            "--package=app",
            "--message-format=json",
        ],
    ] {
        let failure = match route(&args) {
            Route::ManifestError {
                failure,
                json: true,
                ..
            }
            | Route::ManifestTestError {
                failure,
                json: true,
            } => failure,
            other => panic!("{other:?}"),
        };
        assert!(failure
            .message()
            .contains("does not support --message-format=json"));
    }
    let args: Vec<String> = ["script", "probe", "--package=app", "--manifest-path=absent"]
        .iter()
        .map(|s| (*s).into())
        .collect();
    assert_eq!(options::route(&args), Route::Legacy(args));
    let args: Vec<String> = ["check", "--", "--manifest-path=absent", "--package=app"]
        .iter()
        .map(|s| (*s).into())
        .collect();
    assert!(super::super::super::cli::select(&args).is_none());
}
