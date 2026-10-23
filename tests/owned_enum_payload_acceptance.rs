//! Fixed owned enum payload acceptance, driven only through the
//! public `oxid` CLI. Fixtures are `examples/projects/owned_enum_payload_acceptance`
//! (copied to a fresh temporary directory, never built in-tree); negative
//! sources and expected values are fixed below. Adapted from the candidate's
//! independent fixtures to current project commands. The ignored test needs LLVM 19.1.7:
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test
//! owned_enum_payload_acceptance -- --include-ignored`. No qualification,
//! capacity or fuel probes.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// (project, package name, exact result-mode stdout). Exit 0, empty stderr.
const POSITIVE: [(&str, &str, &[u8]); 3] = [
    ("ready", "oea-ready", b"42\n"),
    ("failed", "oea-failed", b"-7\n"),
    ("values", "oea-values", b"42\n"),
];
const SUMMARY_FAILED: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}";

const MOVED_USE: &str = "mod flow;
use crate::lib::model::measure;
fn main() -> i32 {
    let produced = measure(19, 23);
    let first = crate::flow::settle(crate::flow::forward(produced));
    return first + crate::flow::settle(produced);
}
";
const NOMINAL: &str = "struct Packet {
    left: i32,
    right: i32
}
struct Parcel {
    left: i32,
    right: i32
}
enum Slot {
    Hit(Packet)
}
fn parcel() -> Parcel {
    return Parcel { left: 19, right: 23 };
}
fn main() -> i32 {
    let slot = Slot::Hit((parcel()));
    match slot {
        Slot::Hit(packet) => {
            return packet.left + packet.right;
        },
    }
}
";
const PRIVATE_FIELD: &str = "use crate::lib::model::measure;
use crate::lib::model::Outcome;
fn main() -> i32 {
    let produced = measure(19, 23);
    match produced {
        Outcome::Ready(reading) => {
            return reading.left;
        },
        Outcome::Failed(code) => {
            return code;
        },
    }
}
";
const DIRECT_CYCLE: &str = "enum Chain {
    End,
    Next(Chain)
}
fn main() -> i32 {
    return 42;
}
";
const MUTUAL_CYCLE: &str = "enum Left {
    Stop,
    Go(Right)
}
enum Right {
    Back(Left)
}
fn main() -> i32 {
    return 42;
}
";

struct Negative {
    name: &'static str,
    /// true: replaces `ready/src/main.ox` (path dependency); false: single package.
    dependency: bool,
    source: &'static str,
    /// Exact code, stage and message of the one diagnostic.
    code: &'static str,
    stage: &'static str,
    message: &'static str,
    /// Primary span line and column.
    at: (u32, u32),
}
const NEGATIVE: [Negative; 5] = [
    Negative {
        name: "moved-use",
        dependency: true,
        source: MOVED_USE,
        code: "E0310",
        stage: "ownership",
        message: "owned value is not available on every path",
        at: (6, 40),
    },
    Negative {
        name: "nominal-mismatch",
        dependency: false,
        source: NOMINAL,
        code: "E0300",
        stage: "type",
        message: "type mismatch: expected Packet, found Parcel",
        at: (16, 26),
    },
    Negative {
        name: "private-field",
        dependency: true,
        source: PRIVATE_FIELD,
        code: "E0206",
        stage: "type",
        message: "record field is private to its declaring module",
        at: (7, 28),
    },
    Negative {
        name: "direct-cycle",
        dependency: false,
        source: DIRECT_CYCLE,
        code: "E0202",
        stage: "resolve",
        message: "enum containment must be acyclic",
        at: (3, 10),
    },
    Negative {
        name: "mutual-cycle",
        dependency: false,
        source: MUTUAL_CYCLE,
        code: "E0202",
        stage: "resolve",
        message: "enum containment must be acyclic",
        at: (6, 10),
    },
];

fn llvm_bin() -> PathBuf {
    let dir = PathBuf::from(
        std::env::var_os("OXID_TEST_LLVM_BIN")
            .expect("native acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin directory)"),
    );
    for (tool, marker) in [
        ("clang", "clang version 19.1.7"),
        ("opt", "LLVM version 19.1.7"),
        ("ld.lld", "LLD 19.1.7"),
    ] {
        let version = Command::new(dir.join(tool))
            .arg("--version")
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&version.stdout).contains(marker),
            "{tool}: {version:?}"
        );
    }
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

struct Fixture {
    root: PathBuf,
    llvm: PathBuf,
}
impl Fixture {
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-owned-enum-accept-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        copy_tree(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/projects/owned_enum_payload_acceptance"),
            &root,
        );
        Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
        }
    }
    fn oxid(&self, project: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(self.root.join(project))
            .env("OXID_LLVM_BIN", &self.llvm)
            .env("OXID_CACHE_DIR", self.root.join("cache"))
            .env("PATH", self.root.join("absent-path"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn run(&self, project: &str) -> Output {
        self.oxid(project, &["run", "--entry-mode=result"])
    }
    fn exec(&self, path: &Path) -> Output {
        Command::new(path)
            .current_dir("/")
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn negative(&self, case: &Negative) -> String {
        let project = if case.dependency {
            "ready".to_string()
        } else {
            let dir = format!("neg-{}", case.name);
            fs::create_dir_all(self.root.join(&dir).join("src")).unwrap();
            fs::write(
                self.root.join(&dir).join("oxid.toml"),
                "[project]\nname = \"oea-negative\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n",
            )
            .unwrap();
            dir
        };
        fs::write(self.root.join(&project).join("src/main.ox"), case.source).unwrap();
        project
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_result(output: &Output, stdout: &[u8], what: &str) {
    assert_eq!(output.status.code(), Some(0), "{what}: {output:?}");
    assert_eq!(output.stdout, stdout, "{what}: {output:?}");
    assert!(output.stderr.is_empty(), "{what}: {output:?}");
}

fn assert_diagnostic(output: &Output, case: &Negative) {
    let name = case.name;
    assert_eq!(output.status.code(), Some(1), "{name}: {output:?}");
    assert!(output.stderr.is_empty(), "{name}: {output:?}");
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(!text.contains("E0500"), "{name}: {text}");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{name}: exactly one diagnostic: {text}");
    let prefix = format!(
        "{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"{}\",\"stage\":\"{}\",\"message\":\"{}\",\"primary\":{{",
        case.code, case.stage, case.message
    );
    assert!(lines[0].starts_with(&prefix), "{name}: {}", lines[0]);
    let primary = &lines[0][prefix.len()..];
    let primary = &primary[..primary.find('}').unwrap()];
    let at = format!("\"line\":{},\"column\":{},", case.at.0, case.at.1);
    assert!(
        primary.contains(&at),
        "{name}: primary {primary} lacks {at}"
    );
    assert_eq!(lines[1], SUMMARY_FAILED, "{name}");
}

#[test]
fn reference_results_and_fixed_negative_diagnostics() {
    let f = Fixture::new(None);
    for (project, _, stdout) in POSITIVE {
        let checked = f.oxid(project, &["check"]);
        assert_eq!(checked.status.code(), Some(0), "{project}: {checked:?}");
        assert_result(&f.run(project), stdout, project);
        // Native is a real route, never an interpreter fallback: without the
        // toolchain it must fail before producing a result.
        let native = f.oxid(project, &["build"]);
        assert_eq!(native.status.code(), Some(1), "{project}: {native:?}");
        assert!(native.stdout.is_empty(), "{project}: {native:?}");
        assert!(
            String::from_utf8_lossy(&native.stderr).starts_with("error[E0701] (native-toolchain):"),
            "{project}: {native:?}"
        );
    }
    for case in &NEGATIVE {
        let project = f.negative(case);
        assert_diagnostic(&f.oxid(&project, &["check", "--message-format=json"]), case);
        // The same rejection precedes the (absent) native toolchain.
        let native = f.oxid(&project, &["build"]);
        let stderr = String::from_utf8_lossy(&native.stderr);
        assert_eq!(native.status.code(), Some(1), "{}: {native:?}", case.name);
        assert!(native.stdout.is_empty(), "{}: {native:?}", case.name);
        assert!(
            stderr.starts_with(&format!("error[{}]", case.code)),
            "{}: {stderr}",
            case.name
        );
        assert!(
            !stderr.contains("E0500") && !stderr.contains("E0701"),
            "{}: {stderr}",
            case.name
        );
    }
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN; no installation or fallback"]
fn native_run_and_built_artifact_match_reference_bytes() {
    let f = Fixture::new(Some(llvm_bin()));
    for (project, name, stdout) in POSITIVE {
        let reference = f.run(project);
        assert_result(&reference, stdout, project);
        let artifact = f.root.join(project).join(".oxid/bin").join(name);
        for profile in ["dev", "release"] {
            let built = f.oxid(project, &["build", "--profile", profile, "--reproducible"]);
            assert_eq!(built.status.code(), Some(0), "{project}: {built:?}");
            assert!(built.stderr.is_empty(), "{project}: {built:?}");
            let deployed = f.root.join(format!("deployed-{project}-{profile}"));
            fs::copy(&artifact, &deployed).unwrap();
            let executed = f.exec(&deployed);
            assert_result(&executed, stdout, project);
            assert_eq!(
                (&executed.stdout, executed.status.code(), &executed.stderr),
                (
                    &reference.stdout,
                    reference.status.code(),
                    &reference.stderr
                ),
                "{project}"
            );
        }
    }
    // Executables must remain usable after every source and manifest is gone.
    for name in ["ready", "failed", "values", "library"] {
        fs::remove_dir_all(f.root.join(name)).unwrap();
    }
    for (project, _, stdout) in POSITIVE {
        for profile in ["dev", "release"] {
            assert_result(
                &f.exec(&f.root.join(format!("deployed-{project}-{profile}"))),
                stdout,
                project,
            );
        }
    }
}
