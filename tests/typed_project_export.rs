//! Companion acceptance for the normalized typed compiler-source export
//! `oxid export --manifest-path PATH [--package NAME] --output NEW_DIRECTORY`.
//!
//! The ordinary workspace `examples/projects/export_relocation` is copied byte
//! for byte into a fresh temp tree (one dependency module is rewritten with
//! CRLF line endings, and a lockfile plus a stale build artifact are added as
//! deliberately omitted project metadata); the in-tree example is never
//! exported in place. Every expected manifest, path and source byte below is a
//! frozen literal written by hand from the frozen layout contract, never
//! produced by the production planner or writer. Each invocation gets a
//! regular-file stdin whose shared offset must stay at zero and recording LLVM
//! stubs on `OXID_LLVM_BIN` and `PATH` that must never run. Relocated trees are
//! exercised only through the existing `check`, `run` and `test` commands of
//! this driver. No native, held, fuel, capacity or provider work is exercised.
#![cfg(target_os = "linux")]
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{ErrorKind, Seek},
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const EXAMPLE: &str = "examples/projects/export_relocation";
const STDIN: &[u8] = b"stdin belongs to the caller and is never consumed\n";
const APP: &str = "export-app";

const WORKSPACE_MANIFEST: &str = "[workspace]\nmembers = [\"app\", \"geo\", \"tools\"]\n";
const APP_MANIFEST: &str = r#"# Original manifest comment: normalized away by export.
[project]
name = "export-app"
version = "0.3.0"
edition = "typed-preview"
entry = "src/main.ox"

[dependencies]
geo = "../geo"
stats = "../stats"
"#;
const APP_MAIN: &str = r#"// Selected workspace member. Unicode survives byte for byte: café ☕ 日本語 🦀
mod report;
use geo::origin_offset;
use geo::shapes::area::square_area;
use geo::shapes::area::doubled_area;
use stats::summary;
use crate::report::frame::framed;

fn main() -> i32 {
    return framed(square_area(3) + doubled_area(1) + origin_offset() + summary());
}

fn test_area() -> bool {
    return square_area(3) + doubled_area(1) == 11;
}

fn test_distinct_units() -> bool {
    return origin_offset() * 1000 + summary() == 6117;
}

fn test_report() -> i32 {
    return framed(0) - 1000;
}
"#;
const APP_REPORT: &str = "/// Nested module chain: report -> frame.\npub mod frame;\n";
const APP_FRAME: &str = r#"/// Frames a value for the report.
pub fn framed(value: i32) -> i32 {
    return value + 1000;
}
"#;
const APP_UNUSED: &str = r#"/// Not declared by any module: never loaded, checked or exported.
fn never_declared() -> i32 {
    return 0;
}
"#;
const APP_ASSET: &str = "Asset bytes are project metadata: never exported.\n";
const APP_SCRIPT: &str = "Release notes script placeholder: scripts are never exported or run.\n";
const GEO_MANIFEST: &str = r#"[project]
name = "export-geo"
version = "1.2.0"
edition = "typed-preview"
entry = "src/lib.ox"

[dependencies]
kernel = "../core"
units = "vendor/units"
"#;
const GEO_LIB: &str = r#"/// Geometry member: depends on the shared kernel and its own `units`.
pub mod shapes;
use kernel::seed;
use units::unit;

pub fn origin_offset() -> i32 {
    let s = seed();
    return s.value + unit();
}
"#;
const GEO_SHAPES: &str = "pub mod area;\n";
const GEO_AREA: &str = r#"use kernel::scale::double;

pub fn square_area(side: i32) -> i32 {
    return side * side;
}

pub fn doubled_area(side: i32) -> i32 {
    return double(side * side);
}
"#;
const UNITS_MANIFEST: &str =
    "[project]\nname = \"units\"\nedition = \"typed-preview\"\nentry = \"lib.ox\"\n";
const GEO_UNITS: &str = r#"/// Same package name as the stats `units`, but a distinct package.
pub fn unit() -> i32 {
    return 1;
}
"#;
/// Quote, backslash and 雪 are spelled as escapes here; BEL and tab too.
const CORE_MANIFEST: &str = r#"[project]
name = "export \u0022core\u0022 \u005C \u96EA\u0007\tend"
edition = "typed-preview"
entry = "src/lib.ox"
"#;
const CORE_LIB: &str = r#"/// Shared kernel: one package in the export even though two packages use it.
pub mod scale;

pub struct Seed {
    pub value: i32,
}

pub fn seed() -> Seed {
    return Seed { value: 5 };
}
"#;
const CORE_SCALE: &str = "pub fn double(x: i32) -> i32 {\n    return x * 2;\n}\n";
const STATS_MANIFEST: &str = r#"[project]
name = "export-stats"
edition = "typed-preview"
entry = "src/lib.ox"

[dependencies]
measure = "vendor/units"
kernel = "../core"
"#;
const STATS_LIB: &str = r#"mod tally;
use measure::unit;
use kernel::seed;
use kernel::scale::double;

pub fn summary() -> i32 {
    let s = seed();
    return double(s.value) + unit() + crate::tally::count();
}
"#;
/// Checked in with LF; every fixture rewrites it with CRLF line endings.
const STATS_TALLY: &str =
    "/// Counts entries \u{2014} \u{f1}.\npub fn count() -> i32 {\n    return 7;\n}\n";
const STATS_UNITS: &str = r#"/// Same package name as the geo `units`, but a distinct package.
pub fn unit() -> i32 {
    return 100;
}
"#;
const TOOLS_MANIFEST: &str =
    "[project]\nname = \"export-tools\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n";
const TOOLS_MAIN: &str = r#"/// Unrelated workspace member: never part of the export-app closure.
fn main() -> i32 {
    return 0;
}
"#;

/// Every file of the example except README.md, with frozen bytes.
const EXAMPLE_FILES: [(&str, &str); 24] = [
    ("oxid.toml", WORKSPACE_MANIFEST),
    ("app/oxid.toml", APP_MANIFEST),
    ("app/src/main.ox", APP_MAIN),
    ("app/src/report.ox", APP_REPORT),
    ("app/src/report/frame.ox", APP_FRAME),
    ("app/src/unused.ox", APP_UNUSED),
    ("app/assets/banner.txt", APP_ASSET),
    ("app/scripts/publish.txt", APP_SCRIPT),
    ("geo/oxid.toml", GEO_MANIFEST),
    ("geo/src/lib.ox", GEO_LIB),
    ("geo/src/shapes.ox", GEO_SHAPES),
    ("geo/src/shapes/area.ox", GEO_AREA),
    ("geo/vendor/units/oxid.toml", UNITS_MANIFEST),
    ("geo/vendor/units/lib.ox", GEO_UNITS),
    ("core/oxid.toml", CORE_MANIFEST),
    ("core/src/lib.ox", CORE_LIB),
    ("core/src/scale.ox", CORE_SCALE),
    ("stats/oxid.toml", STATS_MANIFEST),
    ("stats/src/lib.ox", STATS_LIB),
    ("stats/src/tally.ox", STATS_TALLY),
    ("stats/vendor/units/oxid.toml", UNITS_MANIFEST),
    ("stats/vendor/units/lib.ox", STATS_UNITS),
    ("tools/oxid.toml", TOOLS_MANIFEST),
    ("tools/src/main.ox", TOOLS_MAIN),
];

// Normalized manifests, written by hand from the layout contract: package
// declaration-DFS ordinals app=0, geo=1, kernel=2, geo units=3, stats=4,
// stats units=5; root edges use packages/pN, non-root edges ../pN.
const EXPORT_ROOT: &str = "[project]\nedition = \"typed-preview\"\nentry = \"src/.root.ox\"\n\
    name = \"export-app\"\n\n[dependencies]\ngeo = \"packages/p1\"\nstats = \"packages/p4\"\n";
const EXPORT_GEO: &str = "[project]\nedition = \"typed-preview\"\nentry = \"src/.root.ox\"\n\
    name = \"export-geo\"\n\n[dependencies]\nkernel = \"../p2\"\nunits = \"../p3\"\n";
const EXPORT_CORE: &str = r#"[project]
edition = "typed-preview"
entry = "src/.root.ox"
name = "export \"core\" \\ 雪\u0007\tend"
"#;
const EXPORT_UNITS: &str =
    "[project]\nedition = \"typed-preview\"\nentry = \"src/.root.ox\"\nname = \"units\"\n";
const EXPORT_STATS: &str = "[project]\nedition = \"typed-preview\"\nentry = \"src/.root.ox\"\n\
    name = \"export-stats\"\n\n[dependencies]\nmeasure = \"../p5\"\nkernel = \"../p2\"\n";
const PASS_ALL: &str = "PASS test_area\nPASS test_distinct_units\nPASS test_report\n\
    test result: 3 passed; 0 failed; 3 total\n";

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Directory,
    File(Vec<u8>),
    Link(PathBuf),
}
type Tree = BTreeMap<String, Node>;

fn tally_crlf() -> String {
    STATS_TALLY.replace('\n', "\r\n")
}

/// The complete expected export: every directory and every file's bytes.
fn expected_export() -> Tree {
    let tally = tally_crlf();
    let mut tree = Tree::new();
    for directory in [
        "src",
        "src/report",
        "packages",
        "packages/p1",
        "packages/p1/src",
        "packages/p1/src/shapes",
        "packages/p2",
        "packages/p2/src",
        "packages/p3",
        "packages/p3/src",
        "packages/p4",
        "packages/p4/src",
        "packages/p5",
        "packages/p5/src",
    ] {
        tree.insert(directory.to_owned(), Node::Directory);
    }
    for (path, bytes) in [
        ("oxid.toml", EXPORT_ROOT),
        ("src/.root.ox", APP_MAIN),
        ("src/report.ox", APP_REPORT),
        ("src/report/frame.ox", APP_FRAME),
        ("packages/p1/oxid.toml", EXPORT_GEO),
        ("packages/p1/src/.root.ox", GEO_LIB),
        ("packages/p1/src/shapes.ox", GEO_SHAPES),
        ("packages/p1/src/shapes/area.ox", GEO_AREA),
        ("packages/p2/oxid.toml", EXPORT_CORE),
        ("packages/p2/src/.root.ox", CORE_LIB),
        ("packages/p2/src/scale.ox", CORE_SCALE),
        ("packages/p3/oxid.toml", EXPORT_UNITS),
        ("packages/p3/src/.root.ox", GEO_UNITS),
        ("packages/p4/oxid.toml", EXPORT_STATS),
        ("packages/p4/src/.root.ox", STATS_LIB),
        ("packages/p4/src/tally.ox", tally.as_str()),
        ("packages/p5/oxid.toml", EXPORT_UNITS),
        ("packages/p5/src/.root.ox", STATS_UNITS),
    ] {
        tree.insert(path.to_owned(), Node::File(bytes.as_bytes().to_vec()));
    }
    tree
}

/// Every path below `root` (relative, `/`-separated) and what it is.
fn tree(root: &Path) -> Tree {
    fn visit(root: &Path, path: &Path, out: &mut Tree) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            let metadata = fs::symlink_metadata(&path).unwrap();
            if metadata.file_type().is_symlink() {
                out.insert(relative, Node::Link(fs::read_link(&path).unwrap()));
            } else if metadata.is_dir() {
                out.insert(relative, Node::Directory);
                visit(root, &path, out);
            } else {
                out.insert(relative, Node::File(fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = Tree::new();
    visit(root, root, &mut out);
    out
}

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    /// `root/{ws,out,tools}` plus the shared stdin file and LLVM stubs.
    fn empty() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-project-export-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for directory in ["ws", "out", "tools"] {
            fs::create_dir(root.join(directory)).unwrap();
        }
        fs::write(root.join("stdin.bin"), STDIN).unwrap();
        let fixture = Self { root };
        fixture.recording_tools();
        fixture
    }
    /// A byte-for-byte copy of the example workspace in `root/ws`, with the
    /// CRLF dependency module and omitted lockfile/artifact metadata added.
    fn example() -> Self {
        let fixture = Self::empty();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE);
        for (path, expected) in EXAMPLE_FILES {
            let bytes = fs::read(repository.join(path)).unwrap();
            assert_eq!(text(&bytes), expected, "checked-in example {path} changed");
            fixture.write(&format!("ws/{path}"), &bytes);
        }
        fixture.write("ws/stats/src/tally.ox", tally_crlf().as_bytes());
        fixture.write("ws/app/oxid.lock", b"# lockfile: never exported\n");
        fixture.write(
            "ws/app/.oxid/typed-preview/result/export-app",
            b"stale build artifact: never exported\n",
        );
        fixture
    }
    fn recording_tools(&self) {
        let log = self.root.join("probe.log");
        for name in ["clang", "opt", "ld.lld", "llc", "cc"] {
            let path = self.root.join("tools").join(name);
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
    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn append(&self, relative: &str, addition: &str) {
        let mut bytes = fs::read(self.path(relative)).unwrap();
        bytes.extend_from_slice(addition.as_bytes());
        self.write(relative, &bytes);
    }
    fn probed(&self) -> bool {
        self.path("probe.log").exists()
    }
    /// Runs `command` in `cwd` with the given stdout. Stdin is a regular file
    /// whose open file description is shared with the child, so any read by
    /// the child would move the offset observed here.
    fn spawn(&self, mut command: Command, cwd: &Path, stdout: Stdio) -> Output {
        let mut stdin = File::open(self.path("stdin.bin")).unwrap();
        let output = command
            .current_dir(cwd)
            .env("OXID_LLVM_BIN", self.path("tools"))
            .env("PATH", self.path("tools"))
            .env("OXID_CACHE_DIR", self.path("cache"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::from(stdin.try_clone().unwrap()))
            .stdout(stdout)
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert_eq!(
            stdin.stream_position().unwrap(),
            0,
            "stdin read: {command:?}"
        );
        output
    }
    fn run_in(&self, cwd: &str, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command.args(args);
        self.spawn(command, &self.path(cwd), Stdio::piped())
    }
    fn oxid(&self, args: &[&str]) -> Output {
        self.run_in("", args)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.path("ws"), fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn status_line(output: &str) -> String {
    format!("Exported checked compiler sources to \"{output}\"\n")
}

/// A successful export: status 0, empty stderr, exactly one status line.
fn assert_exported(label: &str, output: &Output, destination: &str) {
    assert_eq!(output.status.code(), Some(0), "{label}: {output:?}");
    assert!(
        output.stderr.is_empty(),
        "{label}: {}",
        text(&output.stderr)
    );
    assert_eq!(text(&output.stdout), status_line(destination), "{label}");
}

/// A refused export: status 1, no stdout, a text diagnostic. Returns stderr.
fn assert_refused(label: &str, output: &Output) -> String {
    assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
    assert!(
        output.stdout.is_empty(),
        "{label}: {}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(stderr.starts_with("error["), "{label}: {stderr}");
    assert!(!stderr.contains("\"code\""), "{label}: {stderr}");
    stderr
}

/// `error[CODE] (stage)` of the first diagnostic line.
fn classification(stderr: &str) -> &str {
    let line = stderr.lines().next().unwrap_or_default();
    &line[..line.find("): ").map_or(line.len(), |end| end + 1)]
}

fn workspace_export(output: &str) -> Vec<String> {
    [
        "export",
        "--manifest-path=ws/oxid.toml",
        "--package=export-app",
    ]
    .iter()
    .map(|argument| (*argument).to_owned())
    .chain([format!("--output={output}")])
    .collect()
}

fn strings(arguments: &[String]) -> Vec<&str> {
    arguments.iter().map(String::as_str).collect()
}

#[test]
fn relocated_export_checks_runs_and_tests_without_the_original() {
    let f = Fixture::example();
    let original = tree(&f.path("ws"));
    let exported = f.oxid(&strings(&workspace_export("out/first")));
    assert_exported("first", &exported, "out/first");
    assert_eq!(tree(&f.path("out/first")), expected_export());
    assert_eq!(
        tree(&f.path("ws")),
        original,
        "the original project changed"
    );
    // No original host path is emitted anywhere in the portable tree.
    let canonical = fs::canonicalize(&f.root).unwrap();
    for (path, node) in tree(&f.path("out/first")) {
        if let Node::File(bytes) = node {
            let body = text(&bytes);
            for host in [
                f.root.to_str().unwrap(),
                canonical.to_str().unwrap(),
                "/ws/",
                "../geo",
            ] {
                assert!(!body.contains(host), "{path} leaks {host}");
            }
        }
    }

    // Relocate the whole export, then move the original tree out of the way
    // (every byte retained) so its path provably no longer exists.
    fs::create_dir(f.path("elsewhere")).unwrap();
    fs::rename(f.path("out/first"), f.path("elsewhere/moved")).unwrap();
    assert!(tree(&f.path("out")).is_empty());
    let retained = f.path("retained-original");
    assert_eq!(
        fs::symlink_metadata(&retained).unwrap_err().kind(),
        ErrorKind::NotFound
    );
    fs::rename(f.path("ws"), &retained).unwrap();
    let assert_original_absent = |when: &str| {
        let metadata = fs::symlink_metadata(f.path("ws"));
        assert_eq!(
            metadata.as_ref().map_err(|error| error.kind()).err(),
            Some(ErrorKind::NotFound),
            "{when}: {metadata:?}"
        );
        let manifest = File::open(f.path("ws/app/oxid.toml"));
        assert_eq!(
            manifest.as_ref().map_err(|error| error.kind()).err(),
            Some(ErrorKind::NotFound),
            "{when}: {manifest:?}"
        );
    };
    assert_original_absent("before relocated commands");

    let check = f.run_in(
        "elsewhere",
        &["check", "--manifest-path", "moved/oxid.toml"],
    );
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    let run = f.run_in("elsewhere", &["run", "--manifest-path", "moved/oxid.toml"]);
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert_eq!(text(&run.stdout), "1134\n", "{run:?}");
    assert!(run.stderr.is_empty(), "{run:?}");
    let suite = f.run_in("elsewhere", &["test", "--manifest-path=moved/oxid.toml"]);
    assert_eq!(suite.status.code(), Some(0), "{suite:?}");
    assert_eq!(text(&suite.stdout), PASS_ALL, "{suite:?}");
    assert!(suite.stderr.is_empty(), "{suite:?}");
    let absolute = f.path("elsewhere/moved/oxid.toml");
    let run = f.oxid(&["run", "--manifest-path", absolute.to_str().unwrap()]);
    assert_eq!(text(&run.stdout), "1134\n", "{run:?}");
    assert_eq!(
        tree(&f.path("elsewhere/moved")),
        expected_export(),
        "commands wrote"
    );

    // Re-exporting the relocated tree reproduces it byte for byte: escaped
    // names, aliases, shared ordinals and relative paths survive the
    // existing manifest parser.
    let again = f.run_in(
        "elsewhere",
        &[
            "export",
            "--manifest-path=moved/oxid.toml",
            "--output=again",
        ],
    );
    assert_exported("again", &again, "again");
    assert_eq!(tree(&f.path("elsewhere/again")), expected_export());
    let kernel = f.run_in(
        "elsewhere",
        &[
            "export",
            "--manifest-path",
            "moved/packages/p2/oxid.toml",
            "--output",
            "kernel",
        ],
    );
    assert_exported("kernel", &kernel, "kernel");
    let mut expected = Tree::new();
    expected.insert(
        "oxid.toml".into(),
        Node::File(EXPORT_CORE.as_bytes().to_vec()),
    );
    expected.insert("src".into(), Node::Directory);
    expected.insert(
        "src/.root.ox".into(),
        Node::File(CORE_LIB.as_bytes().to_vec()),
    );
    expected.insert(
        "src/scale.ox".into(),
        Node::File(CORE_SCALE.as_bytes().to_vec()),
    );
    assert_eq!(tree(&f.path("elsewhere/kernel")), expected);

    assert_original_absent("after relocated commands");
    fs::rename(&retained, f.path("ws")).unwrap();
    assert_eq!(
        tree(&f.path("ws")),
        original,
        "the original project changed"
    );
    assert!(!f.probed(), "toolchain invoked");
}

#[test]
fn selection_spellings_and_destinations_give_identical_trees() {
    let f = Fixture::example();
    let original = tree(&f.path("ws"));
    let workspace = f.path("ws/oxid.toml");
    let absolute_output = f.path("out/absolute");
    let absolute_output = absolute_output.to_str().unwrap();
    let cases: [(&[&str], &str); 5] = [
        (
            &[
                "export",
                "--manifest-path",
                "ws/oxid.toml",
                "--package",
                APP,
                "--output",
                "out/split",
            ],
            "out/split",
        ),
        (
            &[
                "--edition=typed-preview",
                "export",
                "--package",
                APP,
                "--manifest-path=ws/oxid.toml",
                "--message-format=text",
                "--output=out/global",
            ],
            "out/global",
        ),
        (
            &[
                "export",
                "--manifest-path=ws/app/oxid.toml",
                "--output",
                "out/direct",
            ],
            "out/direct",
        ),
        (
            &[
                "export",
                "--manifest-path",
                workspace.to_str().unwrap(),
                "--package",
                APP,
                "--output",
                absolute_output,
            ],
            absolute_output,
        ),
        (
            &[
                "export",
                "--manifest-path=ws/oxid.toml",
                "--package=export-app",
                "--output=out/a",
            ],
            "out/a",
        ),
    ];
    for (args, destination) in cases {
        let output = f.oxid(args);
        assert_exported(destination, &output, destination);
        assert_eq!(tree(&f.path(destination)), expected_export(), "{args:?}");
    }
    assert_eq!(tree(&f.path("ws")), original);
    assert!(!f.probed(), "toolchain invoked");
}

#[test]
fn normalized_metadata_is_distinct_from_deliberately_omitted_project_metadata() {
    let f = Fixture::example();
    assert_exported(
        "export",
        &f.oxid(&strings(&workspace_export("out/x"))),
        "out/x",
    );
    let exported = tree(&f.path("out/x"));
    // Omitted: original manifest comments/version, assets, scripts,
    // undeclared sources, README, the workspace manifest and the unrelated
    // member, lockfiles and build artifacts. Source files keep only their
    // module-relative names below `src`.
    for path in exported.keys() {
        for omitted in [
            "assets",
            "scripts",
            "unused",
            "README",
            "tools",
            "oxid.lock",
            ".oxid",
            "main.ox",
            "lib.ox",
            "vendor",
            "core",
            "stats",
            "geo",
        ] {
            assert!(!path.contains(omitted), "{path} contains omitted {omitted}");
        }
    }
    for original in [APP_MANIFEST, GEO_MANIFEST] {
        assert!(original.contains("version = "));
    }
    assert!(APP_MANIFEST.starts_with('#'));
    let manifests = [
        "oxid.toml",
        "packages/p1/oxid.toml",
        "packages/p2/oxid.toml",
    ];
    for path in manifests
        .into_iter()
        .chain(["packages/p3/oxid.toml", "packages/p4/oxid.toml"])
    {
        let Some(Node::File(bytes)) = exported.get(path) else {
            panic!("{path} missing");
        };
        let manifest = text(bytes);
        // Normalized compiler metadata only: explicit edition, the fixed
        // hidden root entry, a retained name and relocated dependency edges.
        assert!(manifest.starts_with(
            "[project]\nedition = \"typed-preview\"\nentry = \"src/.root.ox\"\nname = "
        ));
        for line in manifest.lines() {
            let normalized = line.is_empty()
                || line == "[project]"
                || line == "[dependencies]"
                || line.starts_with("edition = ")
                || line.starts_with("entry = ")
                || line.starts_with("name = ")
                || line.ends_with("= \"packages/p1\"")
                || line.ends_with("= \"packages/p4\"")
                || (line.contains(" = \"../p") && line.ends_with('"'));
            assert!(normalized, "{path}: unexpected manifest line {line:?}");
        }
    }
    // Compiler sources are byte-identical, including CRLF and Unicode.
    let tally = tally_crlf();
    for (exported_path, source) in [
        ("src/.root.ox", APP_MAIN),
        ("packages/p4/src/tally.ox", tally.as_str()),
        ("packages/p3/src/.root.ox", GEO_UNITS),
        ("packages/p5/src/.root.ox", STATS_UNITS),
    ] {
        assert_eq!(
            exported.get(exported_path),
            Some(&Node::File(source.as_bytes().to_vec())),
            "{exported_path}"
        );
    }
    // Same-named packages stay distinct; the shared kernel is emitted once.
    assert_eq!(
        exported.get("packages/p3/oxid.toml"),
        exported.get("packages/p5/oxid.toml")
    );
    assert_ne!(
        exported.get("packages/p3/src/.root.ox"),
        exported.get("packages/p5/src/.root.ox")
    );
    let kernel_sources = exported
        .iter()
        .filter(|(_, node)| **node == Node::File(CORE_LIB.as_bytes().to_vec()))
        .count();
    assert_eq!(kernel_sources, 1);
}

#[test]
fn cli_rejections_precede_manifest_io_and_create_nothing() {
    let f = Fixture::example();
    let original = tree(&f.path("ws"));
    let absent = "--manifest-path=absent/oxid.toml";
    let cases: [(&[&str], &str); 18] = [
        (&[], "manifest export requires --output"),
        (&["--output"], "--output requires a value"),
        (&["--output="], "--output requires a value"),
        (
            &["--output=out/a", "--output=out/b"],
            "--output may only be specified once",
        ),
        (
            &["--output=out/a", "--package=x", "--package=y"],
            "--package may only be specified once",
        ),
        (
            &["--output=out/a", "--package"],
            "--package requires a value",
        ),
        (
            &["--output=out/a", "--message-format=json"],
            "does not support --message-format=json",
        ),
        (
            &["--output=out/a", "--message-format", "json"],
            "does not support --message-format=json",
        ),
        (
            &["--output=out/a", "--engine=native"],
            "unsupported option for manifest export: --engine=native",
        ),
        (
            &["--output=out/a", "--engine", "native"],
            "unsupported option for manifest export: --engine",
        ),
        (
            &["--output=out/a", "--entry-mode=process"],
            "does not support entry-mode or experimental selectors",
        ),
        (
            &["--output=out/a", "--experimental-hir-import=x.json"],
            "does not support entry-mode or experimental selectors",
        ),
        (
            &["--output=out/a", "--profile=release"],
            "--profile is only supported by",
        ),
        (
            &["--output=out/a", "--edition=legacy-0.9"],
            "conflicts with --edition legacy-0.9",
        ),
        (
            &["--output=out/a", "--edition=bogus"],
            "unknown edition `bogus`",
        ),
        (
            &["--output=out/a", "src/main.ox"],
            "does not accept a source operand",
        ),
        (
            &["--output=out/a", "--", "--package=export-app"],
            "does not accept a source operand",
        ),
        (
            &["--", "--output=out/a"],
            "does not accept a source operand",
        ),
    ];
    let mut all: Vec<(Vec<&str>, &str)> = cases
        .iter()
        .map(|(tail, needle)| {
            let mut args = vec!["export", absent];
            args.extend_from_slice(tail);
            (args, *needle)
        })
        .collect();
    all.push((
        vec![absent, "export", "--output=out/a"],
        "--manifest-path must follow export",
    ));
    for (args, needle) in &all {
        let stderr = assert_refused(needle, &f.oxid(args));
        assert!(
            stderr.starts_with("error[E0001] (cli): "),
            "{args:?}: {stderr}"
        );
        assert!(stderr.contains(needle), "{args:?}: {stderr}");
        assert!(tree(&f.path("out")).is_empty(), "{args:?} created output");
    }
    // Commands owned by other routes never export: script arguments, `--`
    // operands, a missing manifest selector and another command's options.
    for args in [
        &[
            "script",
            "export",
            "--manifest-path=ws/oxid.toml",
            "--output=out/s",
        ][..],
        &[
            "export",
            "--",
            "--manifest-path=ws/oxid.toml",
            "--output=out/d",
        ],
        &["export", "--output=out/l"],
        &[
            "check",
            "--manifest-path=ws/app/oxid.toml",
            "--output=out/c",
        ],
    ] {
        let output = f.oxid(args);
        assert!(
            !text(&output.stdout).contains("Exported checked compiler sources"),
            "{args:?}: {output:?}"
        );
        assert!(tree(&f.path("out")).is_empty(), "{args:?} created output");
    }
    // Manifest selection failures keep the classification of `check`.
    for (selector, needle) in [
        (
            &["--manifest-path=ws/oxid.toml"][..],
            "workspace requires --package",
        ),
        (
            &["--manifest-path=ws/oxid.toml", "--package=absent"],
            "unknown workspace package `absent`",
        ),
        (
            &["--manifest-path=ws/app/oxid.toml", "--package", APP],
            "--package requires a virtual workspace manifest",
        ),
        (&["--manifest-path=ws/absent.toml"], "cannot read manifest"),
    ] {
        let mut export = vec!["export"];
        export.extend_from_slice(selector);
        export.push("--output=out/a");
        let stderr = assert_refused(needle, &f.oxid(&export));
        assert!(stderr.contains(needle), "{selector:?}: {stderr}");
        let mut check = vec!["check"];
        check.extend_from_slice(selector);
        let checked = f.oxid(&check);
        assert_eq!(checked.status.code(), Some(1), "{checked:?}");
        assert_eq!(
            classification(&stderr),
            classification(&text(&checked.stderr)),
            "{selector:?}"
        );
        assert!(
            tree(&f.path("out")).is_empty(),
            "{selector:?} created output"
        );
    }
    assert_eq!(tree(&f.path("ws")), original);
    assert!(!f.probed(), "toolchain invoked");
}

#[test]
fn load_and_check_failures_precede_destination_creation() {
    let selector = ["--manifest-path=ws/oxid.toml", "--package=export-app"];
    let cases: [(&str, &str, &str); 5] = [
        // Dependency bodies are checked, including unused ones.
        (
            "ws/stats/vendor/units/lib.ox",
            "fn unused() -> i32 { return true; }\n",
            "error[E0300] (type)",
        ),
        (
            "ws/core/src/scale.ox",
            "fn lost() -> i32 { return missing(); }\n",
            "error[E0200] (resolve)",
        ),
        (
            "ws/app/src/report/frame.ox",
            "pub fn broken( {\n",
            "error[E0100] (parse)",
        ),
        (
            "ws/geo/src/lib.ox",
            "pub mod absent;\n",
            "error[E0002] (source)",
        ),
        ("ws/stats/vendor", "", ""),
    ];
    for (path, addition, expected) in cases {
        let f = Fixture::example();
        if addition.is_empty() {
            fs::remove_dir_all(f.path(path)).unwrap();
        } else {
            f.append(path, addition);
        }
        let mut export = vec!["export"];
        export.extend_from_slice(&selector);
        export.push("--output=out/x");
        let stderr = assert_refused(path, &f.oxid(&export));
        let mut check = vec!["check"];
        check.extend_from_slice(&selector);
        let checked = f.oxid(&check);
        assert_eq!(checked.status.code(), Some(1), "{checked:?}");
        assert_eq!(
            classification(&stderr),
            classification(&text(&checked.stderr)),
            "{path}"
        );
        if !expected.is_empty() {
            assert_eq!(classification(&stderr), expected, "{path}: {stderr}");
        }
        assert!(
            tree(&f.path("out")).is_empty(),
            "{path}: destination created"
        );
        assert!(!f.probed(), "{path}: toolchain invoked");
    }
}

#[test]
fn existing_and_unsafe_destinations_are_refused_without_change() {
    let f = Fixture::example();
    fs::create_dir(f.path("out/existing-dir")).unwrap();
    f.write("out/existing-dir/keep", b"keep");
    f.write("out/existing-file", b"file");
    symlink(f.path("out/existing-dir"), f.path("out/link-dir")).unwrap();
    symlink(f.path("out/nowhere"), f.path("out/dangling")).unwrap();
    fs::create_dir(f.path("out/real")).unwrap();
    symlink(f.path("out/real"), f.path("out/alias")).unwrap();
    f.write("out/plain-file", b"plain");
    let before = tree(&f.path("out"));
    let original = tree(&f.path("ws"));
    let absolute_alias = f.path("out/alias/absolute");
    let dots = "must not contain `.` or `..` components";
    let cases = [
        ("out/existing-dir", "already exists as a directory"),
        ("out/existing-file", "already exists as a file"),
        ("out/link-dir", "already exists as a symbolic link"),
        ("out/dangling", "already exists as a symbolic link"),
        ("out/alias/new", "is a symbolic link"),
        (absolute_alias.to_str().unwrap(), "is a symbolic link"),
        ("out/plain-file/new", "is not a directory"),
        ("out/missing/new", "does not exist"),
        ("out/existing-dir/../new", dots),
        ("out/new/.", dots),
        ("out/new/..", dots),
        ("./out/new", dots),
        ("out/new/", "without a trailing separator"),
    ];
    for (destination, needle) in cases {
        let stderr = assert_refused(
            destination,
            &f.oxid(&strings(&workspace_export(destination))),
        );
        assert!(
            stderr.starts_with("error[E0002] (project-export): "),
            "{destination}: {stderr}"
        );
        assert!(stderr.contains(needle), "{destination}: {stderr}");
        assert!(
            stderr.contains("; the destination was not created by this invocation"),
            "{destination}: {stderr}"
        );
        assert_eq!(
            tree(&f.path("out")),
            before,
            "{destination}: output changed"
        );
    }
    // Failure reporting is control-safe even for a hostile output spelling.
    let hostile = "out/missing\u{1b}[31m\u{9b}\u{7}/new";
    let stderr = assert_refused("hostile", &f.oxid(&strings(&workspace_export(hostile))));
    assert!(
        !stderr.contains(['\u{1b}', '\u{9b}', '\u{7}']),
        "{stderr:?}"
    );
    assert!(stderr.contains("does not exist"), "{stderr:?}");
    assert_eq!(tree(&f.path("out")), before);
    assert_eq!(tree(&f.path("ws")), original);
    assert!(!f.probed(), "toolchain invoked");
}

#[test]
fn partial_writer_failure_is_retained_and_reported_truthfully() {
    let f = Fixture::empty();
    let manifest = "[project]\nname = \"big\"\nedition = \"typed-preview\"\nentry = \"main.ox\"\n";
    let mut source = String::new();
    for line in 0..1000 {
        source.push_str(&format!(
            "// filler line {line:04} for the partial writer control\n"
        ));
    }
    source.push_str("fn main() -> i32 {\n    return 0;\n}\n");
    f.write("ws/big/oxid.toml", manifest.as_bytes());
    f.write("ws/big/main.ox", source.as_bytes());
    // A real, deterministic partial write: the file-size limit (a few KiB)
    // admits the small manifest but cuts the large root source short; with
    // SIGXFSZ ignored the write fails with EFBIG instead of killing Oxid.
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg("trap '' XFSZ; ulimit -f 4 && exec \"$0\" \"$@\"")
        .arg(env!("CARGO_BIN_EXE_oxid"))
        .args([
            "export",
            "--manifest-path=ws/big/oxid.toml",
            "--output=out/big",
        ]);
    let output = f.spawn(command, &f.root, Stdio::piped());
    let stderr = assert_refused("partial", &output);
    assert!(
        stderr.starts_with(
            "error[E0002] (project-export): failed writing export file \"out/big/src/.root.ox\""
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "a partial destination may remain; it was created by this invocation and was retained"
        ),
        "{stderr}"
    );
    let retained = tree(&f.path("out/big"));
    assert_eq!(
        retained.keys().map(String::as_str).collect::<Vec<_>>(),
        ["oxid.toml", "src", "src/.root.ox"]
    );
    assert_eq!(
        retained["oxid.toml"],
        Node::File(EXPORT_UNITS.replace("\"units\"", "\"big\"").into_bytes())
    );
    let Node::File(partial) = &retained["src/.root.ox"] else {
        panic!("root source is not a file");
    };
    assert!(
        !partial.is_empty() && partial.len() < source.len(),
        "{}",
        partial.len()
    );
    assert!(source.as_bytes().starts_with(partial));
    assert!(!f.probed(), "toolchain invoked");
}

#[test]
fn status_output_failure_after_success_keeps_the_complete_destination() {
    let f = Fixture::example();
    let full = File::options().write(true).open("/dev/full").unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
    command.args(strings(&workspace_export("out/full")));
    let output = f.spawn(command, &f.root, Stdio::from(full));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with("error[E0002] (project-export): "),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "export completed at \"out/full\" but status output failed; the complete destination was retained"
        ),
        "{stderr}"
    );
    assert_eq!(tree(&f.path("out/full")), expected_export());
    assert!(!f.probed(), "toolchain invoked");
}

#[test]
fn successful_export_status_escapes_control_path() {
    let f = Fixture::example();
    let original = tree(&f.path("ws"));
    let destination = "out/status-\u{7f}\u{80}\u{9b}親";
    let output = f.oxid(&strings(&workspace_export(destination)));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    assert_eq!(
        stdout,
        "Exported checked compiler sources to \"out/status-\\u007f\\u0080\\u009b親\"\n"
    );
    let line = stdout.strip_suffix('\n').unwrap();
    assert!(!line.chars().any(char::is_control), "{stdout:?}");
    assert_eq!(tree(&f.path(destination)), expected_export());
    assert_eq!(
        tree(&f.path("ws")),
        original,
        "the original project changed"
    );
    assert!(!f.probed(), "toolchain invoked");
}
