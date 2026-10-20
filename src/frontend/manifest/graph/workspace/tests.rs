use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);

#[test]
fn batch_catalog_preserves_selection_budget_and_allocation_sequence() {
    let f = Fixture::new();
    let root = "[workspace]\nmembers=[\"app\",\"library\"]\n";
    let app = "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"missing.ox\"\n[dependencies]\nmath=\"../library\"\n";
    let library = "[project]\nname=\"library\"\nedition=\"typed-preview\"\nentry=\"missing.ox\"\n";
    for (path, text) in [
        ("oxid.toml", root),
        ("app/oxid.toml", app),
        ("library/oxid.toml", library),
    ] {
        f.write(path, text);
    }
    for operation in [Command::Check, Command::Test] {
        let mut selected_budget = Budget::default();
        let mut selected_allocator = Allocator::default();
        let selected = select(
            &f.manifest(),
            root.as_bytes(),
            Some("app"),
            operation,
            false,
            &mut selected_budget,
            &mut selected_allocator,
        )
        .unwrap();
        let mut catalog_budget = Budget::default();
        let mut catalog_allocator = Allocator::default();
        let catalog = enumerate(
            &f.manifest(),
            root.as_bytes(),
            None,
            operation,
            &mut catalog_budget,
            &mut catalog_allocator,
        )
        .unwrap();
        assert_eq!(
            catalog[0].package.as_ref().unwrap().name,
            selected.root.name
        );
        assert_eq!(catalog[1].package.as_ref().unwrap().name, "library");
        assert_eq!(catalog[0]._raw, app.as_bytes());
        assert_eq!(catalog[1]._raw, library.as_bytes());
        assert_eq!(
            catalog_budget.manifest_bytes,
            selected_budget.manifest_bytes
        );
        assert_eq!(catalog_budget.edge_bytes, selected_budget.edge_bytes);
        assert_eq!(catalog_budget.path_bytes, selected_budget.path_bytes);
        assert_eq!(catalog_allocator.attempts, selected_allocator.attempts);
        assert_eq!(
            catalog_budget.manifest_bytes,
            root.len() + app.len() + library.len()
        );
    }
}

#[test]
fn batch_callback_requires_complete_valid_membership_but_no_sources() {
    let f = Fixture::new();
    f.write("oxid.toml", "[workspace]\nmembers=[\"app\",\"library\"]\n");
    f.write(
        "app/oxid.toml",
        "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"missing.ox\"\n",
    );
    f.write(
        "library/oxid.toml",
        "[project]\nname=\"library\"\nedition=\"typed-preview\"\nentry=\"missing.ox\"\n",
    );
    let names = with_workspace_members(
        crate::frontend::manifest::format_request(&f.manifest(), None).unwrap(),
        Command::Check,
        |path, members| {
            assert_eq!(path, f.manifest());
            assert_eq!(members.len(), 2);
            assert_eq!(members.name(2), None);
            [
                members.name(0).unwrap().to_owned(),
                members.name(1).unwrap().to_owned(),
            ]
        },
    )
    .unwrap();
    assert_eq!(names, ["app", "library"]);
    assert!(!f.0.join("app/missing.ox").exists());
    for (path, selector) in [
        (f.manifest(), Some("app")),
        (f.0.join("app/oxid.toml").to_str().unwrap().into(), None),
    ] {
        let mut called = false;
        assert!(with_workspace_members(
            crate::frontend::manifest::format_request(&path, selector).unwrap(),
            Command::Check,
            |_, _| called = true,
        )
        .is_err());
        assert!(!called);
    }
    for last_member in [
        "[project]\nname=broken",
        "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"missing.ox\"\n",
    ] {
        f.write("library/oxid.toml", last_member);
        let mut called = false;
        assert!(with_workspace_members(
            crate::frontend::manifest::format_request(&f.manifest(), None).unwrap(),
            Command::Test,
            |_, _| called = true,
        )
        .is_err());
        assert!(!called);
    }
}
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
