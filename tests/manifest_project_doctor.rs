//! Companion controls for explicit manifest-selected `doctor`.
//!
//! Contract: `doctor --manifest-path PATH [--package NAME] [--native]`.
//! The default report resolves only the manifest graph; it never opens sources,
//! compiles or executes the project. `--native` adds the existing toolchain
//! compile/link probe and never executes its product. Only failing probes are
//! controlled here: the toolchain directory is either absent or holds recording
//! stubs that fail. No real LLVM, native artifact or held boundary is used.
//!
//! Report structure (confirmed contract): `typed project configuration
//! resolved`, `edition: typed-preview`, `selected manifest: "PATH"`,
//! `selected package: "NAME"` (or `<unnamed>`), `entry: "PATH"`,
//! `packages: N`, then per package in graph order
//! `package INDEX: manifest="PATH" name="NAME" entry="PATH"` followed by its
//! `dependency SOURCE: alias="ALIAS" target=TARGET` lines (SOURCE is the
//! owning package's index), then `sources: not checked` and a final
//! `native probe: not requested (use --native)` line. A shared dependency is
//! listed once. Rejections are text diagnostics on stderr with an empty stdout
//! (no sticky JSON). With `--native`, the same report ends in
//! `native probe: requested` and is written and flushed before the probe.
#![cfg(target_os = "linux")]
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const NOT_CHECKED: &str = "sources: not checked";
const NOT_REQUESTED: &str = "native probe: not requested (use --native)";
const REQUESTED: &str = "native probe: requested";

/// `project/` holds manifests and sources; `tools/` holds failing recording
/// toolchain stubs whose invocations are appended to `probe.log` outside the
/// project tree, so artifact snapshots of `project/` stay unaffected.
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-project-doctor-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("project")).unwrap();
        fs::create_dir(root.join("tools")).unwrap();
        let fixture = Self(root);
        fixture.recording_tools();
        fixture
    }
    fn project(&self) -> PathBuf {
        self.0.join("project")
    }
    fn recording_tools(&self) {
        use std::os::unix::fs::PermissionsExt;
        let log = self.0.join("probe.log");
        for name in ["clang", "opt", "ld.lld"] {
            let path = self.0.join("tools").join(name);
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$0 $*\" >> '{}'\nexit 3\n",
                    log.display()
                ),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    fn probed(&self) -> bool {
        self.0.join("probe.log").exists()
    }
    fn write(&self, path: &str, text: &str) {
        self.write_bytes(path, text.as_bytes());
    }
    fn write_bytes(&self, path: &str, bytes: &[u8]) {
        let path = self.project().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    /// A named typed-preview project with optional `alias="dir"` dependencies.
    fn package(&self, dir: &str, name: &str, entry: &str, dependencies: &[(&str, &str)]) {
        let mut text =
            format!("[project]\nname=\"{name}\"\nedition=\"typed-preview\"\nentry=\"{entry}\"\n");
        if !dependencies.is_empty() {
            text.push_str("[dependencies]\n");
            for (alias, target) in dependencies {
                text.push_str(&format!("{alias}=\"{target}\"\n"));
            }
        }
        self.write(&format!("{dir}/oxid.toml"), &text);
    }
    fn workspace(&self, members: &[&str]) {
        let members = members
            .iter()
            .map(|m| format!("\"{m}\""))
            .collect::<Vec<_>>()
            .join(",");
        self.write("oxid.toml", &format!("[workspace]\nmembers=[{members}]\n"));
    }
    fn run_with(&self, args: &[&str], llvm: &Path, stdout: Stdio) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(self.project())
            .env("OXID_LLVM_BIN", llvm)
            .env_remove("OXID_PATH")
            .args(args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(Stdio::piped())
            .output()
            .unwrap()
    }
    /// Runs with the failing recording toolchain.
    fn run(&self, args: &[&str]) -> Output {
        self.run_with(args, &self.0.join("tools"), Stdio::piped())
    }
    /// Runs with no toolchain directory at all.
    fn run_without_llvm(&self, args: &[&str]) -> Output {
        self.run_with(args, &self.0.join("no-native-toolchain"), Stdio::piped())
    }
    fn doctor(&self, extra: &[&str]) -> Output {
        let mut args = vec!["doctor", "--manifest-path=oxid.toml"];
        args.extend_from_slice(extra);
        self.run(&args)
    }
    /// Every path and its bytes under `project/`; the report must not change it.
    fn snapshot(&self) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let kind = entry.file_type().unwrap();
                let relative = entry.path().strip_prefix(root).unwrap().to_path_buf();
                if kind.is_dir() {
                    out.insert(relative, None);
                    visit(root, &entry.path(), out);
                } else {
                    out.insert(relative, Some(fs::read(entry.path()).unwrap()));
                }
            }
        }
        let mut out = BTreeMap::new();
        visit(&self.project(), &self.project(), &mut out);
        out
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
fn combined(output: &Output) -> String {
    format!("{}{}", stdout(output), stderr(output))
}

/// Value spelling of the engine's `Quoted` renderer: names, paths and aliases
/// in double quotes, with control characters, quotes and backslashes written
/// as Rust `char::escape_default` escapes. `target` is the zero-based graph
/// index of the target package; the index after `dependency` is the owning
/// (source) package's index.
fn quoted(text: &str) -> String {
    let mut out = String::from('"');
    for character in text.chars() {
        if character.is_control() || matches!(character, '"' | '\\') {
            out.extend(character.escape_default());
        } else {
            out.push(character);
        }
    }
    out.push('"');
    out
}
fn quoted_path(path: &Path) -> String {
    quoted(path.to_str().unwrap())
}
fn quoted_name(name: Option<&str>) -> String {
    name.map_or_else(|| "<unnamed>".to_owned(), quoted)
}

/// One reported graph node: manifest, name (`None` when unnamed), entry and
/// direct dependency edges as `(alias, target package index)`.
struct Node<'a> {
    manifest: PathBuf,
    name: Option<&'a str>,
    entry: PathBuf,
    dependencies: &'a [(&'a str, usize)],
}

/// The complete default report, line by line, ending in the not-requested line.
fn expected_structure(nodes: &[Node]) -> Vec<String> {
    let root = &nodes[0];
    let mut lines = vec![
        "typed project configuration resolved".to_owned(),
        "edition: typed-preview".to_owned(),
        format!("selected manifest: {}", quoted_path(&root.manifest)),
        format!("selected package: {}", quoted_name(root.name)),
        format!("entry: {}", quoted_path(&root.entry)),
        format!("packages: {}", nodes.len()),
    ];
    for (index, node) in nodes.iter().enumerate() {
        lines.push(format!(
            "package {index}: manifest={} name={} entry={}",
            quoted_path(&node.manifest),
            quoted_name(node.name),
            quoted_path(&node.entry)
        ));
        for (alias, target) in node.dependencies {
            lines.push(format!(
                "dependency {index}: alias={} target={target}",
                quoted(alias)
            ));
        }
    }
    lines.push(NOT_CHECKED.to_owned());
    lines.push(NOT_REQUESTED.to_owned());
    lines
}

/// Compares the complete report byte for byte: every line, in order, each
/// terminated by a newline.
fn assert_structure(text: &str, expected: &[String]) {
    let mut whole = expected.join("\n");
    whole.push('\n');
    assert_eq!(text, whole);
}

/// The `--native` configuration report for a default report: identical
/// package, dependency and sources lines, with only the final line replaced.
fn requested_report(default: &str) -> String {
    let body = default
        .strip_suffix(&format!("{NOT_REQUESTED}\n"))
        .unwrap_or_else(|| panic!("default report has no final line: {default}"));
    format!("{body}{REQUESTED}\n")
}

/// `--native` stdout: non-empty, not the default report, and beginning with
/// the complete requested configuration report; native doctor output may follow.
fn assert_requested(output: &Output, default: &str) {
    let text = stdout(output);
    assert!(!text.is_empty(), "{output:?}");
    assert_ne!(text, default, "{output:?}");
    assert!(!text.contains(NOT_REQUESTED), "{output:?}");
    assert!(text.starts_with(&requested_report(default)), "{output:?}");
}

/// A default report: success, empty stderr, every expected package listed,
/// no forbidden package listed, and the explicit unchecked-sources statement.
fn report(output: &Output, listed: &[&str], absent: &[&str]) -> String {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = stdout(output);
    assert_eq!(
        text.lines().filter(|l| *l == NOT_CHECKED).count(),
        1,
        "{text}"
    );
    assert!(
        text.starts_with("typed project configuration resolved\n"),
        "{text}"
    );
    assert!(
        text.ends_with(&format!("{NOT_CHECKED}\n{NOT_REQUESTED}\n")),
        "{text}"
    );
    let count = format!("packages: {}", listed.len());
    assert!(text.lines().any(|l| l == count), "{text}");
    for name in listed {
        let field = format!(" name={} ", quoted(name));
        assert!(text.contains(&field), "missing `{field}`: {text}");
    }
    for name in absent {
        assert!(!text.contains(name), "unexpected `{name}`: {text}");
    }
    for forbidden in ["E0002", "E0003", "E0300", "E0701", "toolchain ready"] {
        assert!(!text.contains(forbidden), "{text}");
    }
    text
}

/// A CLI or manifest failure: exit 1, empty stdout, a text diagnostic on
/// stderr containing `needle`, no report, and no toolchain lookup or probe.
fn refused(fixture: &Fixture, output: &Output, needle: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let text = stderr(output);
    assert!(text.contains(needle), "missing `{needle}`: {text}");
    assert!(
        !text.trim_start().starts_with('{'),
        "JSON diagnostic: {text}"
    );
    assert!(!text.contains(NOT_CHECKED), "{text}");
    assert!(!text.contains("E0701"), "{text}");
    assert!(!fixture.probed(), "toolchain stub was invoked: {text}");
}

/// A command-line rejection: a text `E0001` diagnostic plus `needle`.
fn cli_refused(fixture: &Fixture, output: &Output, needle: &str) {
    refused(fixture, output, "error[E0001] (cli)");
    refused(fixture, output, needle);
}

fn workspace_graph(f: &Fixture) {
    // app -> left -> shared, app -> right -> shared; `other` is unselected.
    f.workspace(&["app", "left", "right", "shared", "other"]);
    f.package(
        "app",
        "dapp",
        "main.ox",
        &[("dleft", "../left"), ("dright", "../right")],
    );
    f.package("left", "dleft", "lib.ox", &[("dshared", "../shared")]);
    f.package("right", "dright", "lib.ox", &[("dshared", "../shared")]);
    f.package("shared", "dshared", "lib.ox", &[]);
    f.package("other", "dunselected", "main.ox", &[]);
}

#[test]
fn direct_project_default_report_needs_no_sources_toolchain_or_artifacts() {
    let f = Fixture::new();
    f.package(".", "dapp", "main.ox", &[]);
    // The declared entry does not exist: configuration is still reportable.
    let before = f.snapshot();
    let first = f.doctor(&[]);
    let text = report(&first, &["dapp"], &[]);
    for args in [
        &["doctor", "--manifest-path", "oxid.toml"][..],
        &[
            "doctor",
            "--manifest-path=oxid.toml",
            "--message-format=text",
        ],
        &[
            "doctor",
            "--manifest-path=oxid.toml",
            "--edition=typed-preview",
        ],
        &[
            "doctor",
            "--edition",
            "typed-preview",
            "--manifest-path",
            "oxid.toml",
        ],
    ] {
        let again = f.run(args);
        assert_eq!(report(&again, &["dapp"], &[]), text, "{args:?}");
    }
    let absolute = f.project().join("oxid.toml");
    let elsewhere = Command::new(env!("CARGO_BIN_EXE_oxid"))
        .current_dir(&f.0)
        .env("OXID_LLVM_BIN", f.0.join("tools"))
        .env_remove("OXID_PATH")
        .args(["doctor", "--manifest-path", absolute.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    report(&elsewhere, &["dapp"], &[]);
    let without_llvm = f.run_without_llvm(&["doctor", "--manifest-path=oxid.toml"]);
    assert_eq!(report(&without_llvm, &["dapp"], &[]), text);
    assert!(!f.probed());
    assert_eq!(f.snapshot(), before, "default doctor changed the project");
}

#[test]
fn missing_or_invalid_sources_still_report_configuration() {
    let f = Fixture::new();
    f.package(".", "dapp", "main.ox", &[("dlib", "lib")]);
    f.package("lib", "dlib", "lib.ox", &[]);
    let baseline = report(&f.doctor(&[]), &["dapp", "dlib"], &[]);
    let sources: [(&str, &[u8]); 5] = [
        ("main.ox", b"fn main( {"),
        ("main.ox", b"fn main()->i32{return unknown_value;}"),
        ("main.ox", b"\xff\xfe not utf-8"),
        ("lib/lib.ox", b"pub fn broken( {"),
        ("lib/lib.ox", b"\xff\xfe not utf-8"),
    ];
    for (path, bytes) in sources {
        f.write_bytes(path, bytes);
        let before = f.snapshot();
        let output = f.doctor(&[]);
        assert_eq!(report(&output, &["dapp", "dlib"], &[]), baseline, "{path}");
        assert_eq!(f.snapshot(), before, "{path}");
    }
    // An entry path that names a directory is still a source concern only.
    fs::remove_file(f.project().join("main.ox")).unwrap();
    fs::create_dir(f.project().join("main.ox")).unwrap();
    assert_eq!(report(&f.doctor(&[]), &["dapp", "dlib"], &[]), baseline);
    assert!(!f.probed());
}

/// `--manifest-path=<canonical fixture root>/oxid.toml`, so every reported
/// spelling is an actual canonical fixture path.
fn absolute_doctor(f: &Fixture, root: &Path, extra: &[&str]) -> Output {
    let selector = format!("--manifest-path={}", root.join("oxid.toml").display());
    let mut args = vec!["doctor", selector.as_str()];
    args.extend_from_slice(extra);
    f.run(&args)
}

#[test]
fn direct_project_report_has_exact_structure_in_graph_order() {
    let f = Fixture::new();
    let root = fs::canonicalize(f.project()).unwrap();
    // `dleft` is shared by the root and by `dright`, and is listed once.
    f.package(
        ".",
        "dapp",
        "main.ox",
        &[("dleft", "left"), ("dright", "right")],
    );
    f.package("left", "dleft", "lib.ox", &[]);
    f.package("right", "dright", "lib.ox", &[("dleft", "../left")]);
    let expected = expected_structure(&[
        Node {
            manifest: root.join("oxid.toml"),
            name: Some("dapp"),
            entry: root.join("main.ox"),
            dependencies: &[("dleft", 1), ("dright", 2)],
        },
        Node {
            manifest: root.join("left/oxid.toml"),
            name: Some("dleft"),
            entry: root.join("left/lib.ox"),
            dependencies: &[],
        },
        Node {
            manifest: root.join("right/oxid.toml"),
            name: Some("dright"),
            entry: root.join("right/lib.ox"),
            dependencies: &[("dleft", 1)],
        },
    ]);
    let output = absolute_doctor(&f, &root, &[]);
    let text = report(&output, &["dapp", "dleft", "dright"], &[]);
    assert_structure(&text, &expected);
    // `--native` writes the same report with the requested final line first,
    // then the probe fails.
    let native = absolute_doctor(&f, &root, &["--native"]);
    assert_eq!(native.status.code(), Some(1), "{native:?}");
    assert_requested(&native, &text);
    assert!(stderr(&native).contains("E0701"), "{native:?}");
    assert!(f.probed());
}

#[test]
fn unnamed_packages_report_unnamed_placeholder() {
    let f = Fixture::new();
    let root = fs::canonicalize(f.project()).unwrap();
    f.write(
        "oxid.toml",
        "[project]\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[dependencies]\ndlib=\"lib\"\n",
    );
    f.write(
        "lib/oxid.toml",
        "[project]\nedition=\"typed-preview\"\nentry=\"lib.ox\"\n",
    );
    let expected = expected_structure(&[
        Node {
            manifest: root.join("oxid.toml"),
            name: None,
            entry: root.join("main.ox"),
            dependencies: &[("dlib", 1)],
        },
        Node {
            manifest: root.join("lib/oxid.toml"),
            name: None,
            entry: root.join("lib/lib.ox"),
            dependencies: &[],
        },
    ]);
    let output = absolute_doctor(&f, &root, &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_structure(&stdout(&output), &expected);
    assert!(!f.probed());
}

#[test]
fn workspace_report_has_exact_structure_for_selected_member() {
    let f = Fixture::new();
    let root = fs::canonicalize(f.project()).unwrap();
    f.workspace(&["app", "other"]);
    f.package(
        "app",
        "dapp",
        "main.ox",
        &[("dleft", "left"), ("dright", "right")],
    );
    f.package("app/left", "dleft", "lib.ox", &[]);
    f.package("app/right", "dright", "lib.ox", &[("dleft", "../left")]);
    f.package("other", "dunselected", "main.ox", &[]);
    let app = root.join("app");
    let expected = expected_structure(&[
        Node {
            manifest: app.join("oxid.toml"),
            name: Some("dapp"),
            entry: app.join("main.ox"),
            dependencies: &[("dleft", 1), ("dright", 2)],
        },
        Node {
            manifest: app.join("left/oxid.toml"),
            name: Some("dleft"),
            entry: app.join("left/lib.ox"),
            dependencies: &[],
        },
        Node {
            manifest: app.join("right/oxid.toml"),
            name: Some("dright"),
            entry: app.join("right/lib.ox"),
            dependencies: &[("dleft", 1)],
        },
    ]);
    let equals = absolute_doctor(&f, &root, &["--package=dapp"]);
    let text = report(&equals, &["dapp", "dleft", "dright"], &["dunselected"]);
    assert_structure(&text, &expected);
    let split = absolute_doctor(&f, &root, &["--package", "dapp"]);
    assert_eq!(stdout(&split), text, "{split:?}");
    assert!(!f.probed());
}

#[test]
fn workspace_selection_lists_selected_package_and_dependency_closure() {
    let f = Fixture::new();
    workspace_graph(&f);
    // The unselected member's source is invalid and is never opened.
    f.write("other/main.ox", "not a source file");
    let before = f.snapshot();
    let equals = f.doctor(&["--package=dapp"]);
    let text = report(
        &equals,
        &["dapp", "dleft", "dright", "dshared"],
        &["dunselected"],
    );
    let split = f.doctor(&["--package", "dapp"]);
    assert_eq!(
        report(
            &split,
            &["dapp", "dleft", "dright", "dshared"],
            &["dunselected"]
        ),
        text
    );
    // Selecting a dependency member lists only that member's closure.
    report(
        &f.doctor(&["--package=dleft"]),
        &["dleft", "dshared"],
        &["dapp", "dright", "dunselected"],
    );
    report(
        &f.doctor(&["--package", "dshared"]),
        &["dshared"],
        &["dapp", "dleft", "dright", "dunselected"],
    );
    assert_eq!(f.snapshot(), before);
    // A single-member workspace needs no selector.
    f.workspace(&["shared"]);
    report(&f.doctor(&[]), &["dshared"], &["dapp", "dunselected"]);
    assert!(!f.probed());
}

#[test]
fn shared_transitive_dependency_in_direct_project_is_not_a_cycle() {
    let f = Fixture::new();
    f.package(
        ".",
        "dapp",
        "main.ox",
        &[("dleft", "left"), ("dright", "right")],
    );
    f.package("left", "dleft", "lib.ox", &[("dshared", "../shared")]);
    f.package("right", "dright", "lib.ox", &[("dshared", "../shared")]);
    f.package("shared", "dshared", "lib.ox", &[]);
    report(&f.doctor(&[]), &["dapp", "dleft", "dright", "dshared"], &[]);
    assert!(!f.probed());
}

#[test]
fn selection_and_graph_failures_exit_one_without_probing() {
    let f = Fixture::new();
    workspace_graph(&f);
    for native in [&[][..], &["--native"]] {
        let with = |extra: &[&str]| {
            let mut args = extra.to_vec();
            args.extend_from_slice(native);
            f.doctor(&args)
        };
        cli_refused(&f, &with(&[]), "workspace requires --package");
        cli_refused(
            &f,
            &with(&["--package=absent"]),
            "unknown workspace package",
        );
        cli_refused(
            &f,
            &with(&["--package", "app"]),
            "unknown workspace package",
        );
        cli_refused(
            &f,
            &with(&["--package=dapp", "--package=dleft"]),
            "--package may only be specified once",
        );
    }
    // An invalid unselected member manifest fails selection.
    f.write("other/oxid.toml", "[project]\nname=broken");
    cli_refused(&f, &f.doctor(&["--package=dapp"]), "E0001");
    cli_refused(&f, &f.doctor(&["--package=dapp", "--native"]), "E0001");
    f.package("other", "dunselected", "main.ox", &[]);
    // A dependency cycle in the selected closure.
    f.package("shared", "dshared", "lib.ox", &[("dapp", "../app")]);
    refused(&f, &f.doctor(&["--package=dapp"]), "dependency cycle");
    refused(
        &f,
        &f.doctor(&["--native", "--package=dapp"]),
        "dependency cycle",
    );
    f.package("shared", "dshared", "lib.ox", &[]);
    // A dependency directory without a manifest.
    f.package("left", "dleft", "lib.ox", &[("dshared", "../absent")]);
    for native in [&[][..], &["--native"]] {
        let mut args = vec!["--package=dapp"];
        args.extend_from_slice(native);
        let output = f.doctor(&args);
        refused(&f, &output, "error[E0002]");
        refused(&f, &output, "cannot read manifest");
    }
}

#[test]
fn direct_project_selection_failures_exit_one_without_probing() {
    let f = Fixture::new();
    f.package(".", "dapp", "main.ox", &[]);
    for native in [&[][..], &["--native"]] {
        let mut args = vec!["--package=dapp"];
        args.extend_from_slice(native);
        cli_refused(
            &f,
            &f.doctor(&args),
            "--package requires a virtual workspace manifest",
        );
    }
    let missing = f.run(&["doctor", "--manifest-path=absent.toml", "--native"]);
    refused(&f, &missing, "error[E0002]");
    refused(&f, &missing, "cannot read manifest `absent.toml`");
    f.write(
        "oxid.toml",
        "[project]\nname=\"dapp\"\nedition=\"legacy-0.9\"\nentry=\"main.ox\"\n",
    );
    cli_refused(&f, &f.doctor(&["--native"]), "typed-preview");
    f.write_bytes("oxid.toml", b"[project]\nname=\"\xff\"\n");
    refused(&f, &f.doctor(&["--native"]), "error[E0003]");
    refused(&f, &f.doctor(&["--native"]), "not valid UTF-8");
}

#[test]
fn missing_or_invalid_entry_declarations_fail() {
    let f = Fixture::new();
    let invalid = "manifest entry must be a nonempty relative .ox path";
    for (manifest, needle) in [
        (
            "[project]\nname=\"dapp\"\nedition=\"typed-preview\"\n",
            "requires [project].entry",
        ),
        (
            "[project]\nname=\"dapp\"\nedition=\"typed-preview\"\nentry=\"\"\n",
            invalid,
        ),
        (
            "[project]\nname=\"dapp\"\nedition=\"typed-preview\"\nentry=\"../main.ox\"\n",
            invalid,
        ),
        (
            "[project]\nname=\"dapp\"\nedition=\"typed-preview\"\nentry=\"main.txt\"\n",
            invalid,
        ),
        (
            "[project]\nname=\"dapp\"\nedition=\"typed-preview\"\nentry=\"/abs/main.ox\"\n",
            invalid,
        ),
    ] {
        f.write("oxid.toml", manifest);
        cli_refused(&f, &f.doctor(&[]), needle);
        cli_refused(&f, &f.doctor(&["--native"]), needle);
        // The same declaration in a dependency manifest also fails.
        f.package(".", "dapp", "main.ox", &[("dlib", "lib")]);
        f.write("lib/oxid.toml", &manifest.replace("dapp", "dlib"));
        cli_refused(&f, &f.doctor(&[]), needle);
        cli_refused(&f, &f.doctor(&["--native"]), needle);
        fs::remove_dir_all(f.project().join("lib")).unwrap();
    }
}

#[test]
fn unsupported_options_operands_and_order_are_cli_errors_without_probing() {
    let f = Fixture::new();
    f.package(".", "dapp", "main.ox", &[]);
    let before = f.snapshot();
    let rejected: &[&[&str]] = &[
        &["--message-format=json"],
        &["--message-format", "json"],
        &["--profile=dev"],
        &["--profile", "release"],
        &["--engine=native"],
        &["--engine", "interpreter"],
        &["--entry-mode=result"],
        &["--entry-mode", "process"],
        &["--backend=llvm"],
        &["--target=x86_64-unknown-linux-gnu"],
        &["--output=out"],
        &["--output", "out"],
        &["--experimental-hir-import=x.json"],
        &["--experimental-hir-producers=x"],
        &["--experimental-lexical-provider=x"],
        &["--edition=legacy-0.9"],
        &["extra"],
        &["main.ox"],
    ];
    for extra in rejected {
        for native in [&[][..], &["--native"]] {
            let mut args = extra.to_vec();
            args.extend_from_slice(native);
            let output = f.doctor(&args);
            cli_refused(&f, &output, "E0001");
            let mut args = native.to_vec();
            args.extend_from_slice(extra);
            cli_refused(&f, &f.doctor(&args), "E0001");
        }
    }
    // JSON is refused as a text diagnostic, never as sticky JSON on stdout.
    for args in [
        &["--message-format=json"][..],
        &["--message-format", "json", "--native"],
    ] {
        let output = f.doctor(args);
        cli_refused(&f, &output, "E0001");
        assert!(!stderr(&output).contains("\"code\""), "{output:?}");
    }
    // `--native` is a single flag without a value.
    for args in [
        &["doctor", "--manifest-path=oxid.toml", "--native=yes"][..],
        &["doctor", "--native=1", "--manifest-path", "oxid.toml"],
    ] {
        let output = f.run(args);
        cli_refused(&f, &output, "--native");
        cli_refused(&f, &output, "does not accept a value");
    }
    for args in [
        &[
            "doctor",
            "--manifest-path=oxid.toml",
            "--native",
            "--native",
        ][..],
        &[
            "doctor",
            "--native",
            "--manifest-path",
            "oxid.toml",
            "--native",
        ],
    ] {
        let output = f.run(args);
        cli_refused(&f, &output, "--native");
        cli_refused(&f, &output, "may only be specified once");
    }
    // The manifest selector must follow the command word.
    for args in [
        &["--manifest-path=oxid.toml", "doctor"][..],
        &["--manifest-path", "oxid.toml", "doctor", "--native"],
    ] {
        cli_refused(&f, &f.run(args), "--manifest-path must follow doctor");
    }
    assert_eq!(f.snapshot(), before);
}

#[test]
fn native_probe_failure_is_e0701_and_sources_stay_unchecked() {
    let f = Fixture::new();
    f.package(".", "dapp", "main.ox", &[]);
    // Invalid source does not turn the probe request into a source diagnostic.
    f.write("main.ox", "fn main( {");
    let before = f.snapshot();
    let baseline = report(&f.doctor(&[]), &["dapp"], &[]);
    assert!(!f.probed());
    for args in [
        &["doctor", "--manifest-path=oxid.toml", "--native"][..],
        &["doctor", "--native", "--manifest-path", "oxid.toml"],
    ] {
        // The complete configuration report precedes the failed probe.
        let missing = f.run_without_llvm(args);
        assert_eq!(missing.status.code(), Some(1), "{missing:?}");
        assert_requested(&missing, &baseline);
        let text = stderr(&missing);
        assert!(text.contains("E0701"), "{text}");
        assert!(!combined(&missing).contains("E0002"), "{missing:?}");
        assert!(!combined(&missing).contains("toolchain ready"));
        assert!(!f.probed());
        let failing = f.run(args);
        assert_eq!(failing.status.code(), Some(1), "{failing:?}");
        assert_requested(&failing, &baseline);
        assert!(stderr(&failing).contains("E0701"), "{failing:?}");
        assert!(!stderr(&failing).contains(NOT_CHECKED), "{failing:?}");
        assert!(!combined(&failing).contains("E0002"), "{failing:?}");
        assert!(f.probed(), "--native did not reach the toolchain probe");
        fs::remove_file(f.0.join("probe.log")).unwrap();
    }
    assert_eq!(f.snapshot(), before, "--native left project artifacts");
}

#[test]
fn workspace_native_probe_failure_is_e0701_for_both_selector_forms() {
    let f = Fixture::new();
    workspace_graph(&f);
    let before = f.snapshot();
    let baseline = report(
        &f.doctor(&["--package=dapp"]),
        &["dapp", "dleft", "dright", "dshared"],
        &["dunselected"],
    );
    for extra in [
        &["--package=dapp", "--native"][..],
        &["--native", "--package", "dapp"],
    ] {
        let output = f.doctor(extra);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert_requested(&output, &baseline);
        assert!(stderr(&output).contains("E0701"), "{output:?}");
        assert!(f.probed());
        fs::remove_file(f.0.join("probe.log")).unwrap();
    }
    assert_eq!(f.snapshot(), before);
}

#[test]
fn report_write_failure_exits_one_and_never_probes() {
    let f = Fixture::new();
    f.package(".", "dapp", "main.ox", &[]);
    for args in [
        &["doctor", "--manifest-path=oxid.toml"][..],
        &["doctor", "--manifest-path=oxid.toml", "--native"],
        &["doctor", "--native", "--manifest-path", "oxid.toml"],
    ] {
        let full = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let output = f.run_with(args, &f.0.join("tools"), Stdio::from(full));
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        assert!(!stderr(&output).contains("E0701"), "{args:?}: {output:?}");
        assert!(!f.probed(), "{args:?}: probe after a failed report write");
    }
}

#[test]
fn bare_and_explicit_typed_doctor_routes_are_unchanged() {
    let f = Fixture::new();
    f.package(".", "dapp", "main.ox", &[]);
    // Bare doctor stays on the legacy route; no implicit manifest discovery.
    let bare = f.run(&["doctor"]);
    assert!(!combined(&bare).contains(NOT_CHECKED), "{bare:?}");
    assert!(stdout(&bare).starts_with("manifest: "), "{bare:?}");
    assert!(!f.probed());
    // Explicit typed doctor without a manifest still runs only the probe.
    let typed = f.run(&["doctor", "--edition=typed-preview"]);
    assert_eq!(typed.status.code(), Some(1), "{typed:?}");
    assert!(typed.stdout.is_empty(), "{typed:?}");
    assert!(stderr(&typed).contains("E0701"));
    assert!(!combined(&typed).contains(NOT_CHECKED));
    assert!(f.probed());
    fs::remove_file(f.0.join("probe.log")).unwrap();
    // Without a manifest selector, `--native` and `--package` remain local
    // options that explicit typed doctor rejects before any probe.
    for extra in [&["--native"][..], &["--package=dapp"]] {
        let mut args = vec!["doctor", "--edition=typed-preview"];
        args.extend_from_slice(extra);
        let output = f.run(&args);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(combined(&output).contains("typed-preview doctor accepts no operands"));
        assert!(!f.probed());
    }
}
