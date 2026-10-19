//! Regression coverage for the ordinary owned UTF-8 Text path package.
//! The published project is copied to a fresh directory, never built in-tree.
//! Expected results are fixed: check succeeds; result-mode execution prints
//! exactly `42\n`, exits 0, and leaves stderr empty. Native coverage compiles
//! through the manifest and executes a standalone artifact.
//! Run the ignored native test explicitly with:
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test owned_utf8_text -- --include-ignored`.
//! No compiler changes, fault injection, resource probes, or UTF-8 host oracle.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const RESULT: &[u8] = b"42\n";
const CHECK_FAILED: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}";

struct Fixture {
    root: PathBuf,
    project: PathBuf,
    llvm: PathBuf,
}
impl Fixture {
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-owned-utf8-text-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        let project = root.join("examples/projects/owned_utf8_text");
        // Preserve manifest-relative dependency paths; copy only source inputs.
        for path in [
            "examples/projects/owned_utf8_text/oxid.toml",
            "examples/projects/owned_utf8_text/src/main.ox",
            "packages/utf8_text/oxid.toml",
            "packages/utf8_text/src/lib.ox",
        ] {
            let destination = root.join(path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(repository.join(path), destination).unwrap();
        }
        Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
            project,
        }
    }

    fn oxid(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.project)
            .env("OXID_LLVM_BIN", &self.llvm)
            .env("OXID_CACHE_DIR", self.root.join("cache"))
            .env("PATH", self.root.join("absent-path"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn reference(&self) -> Output {
        self.oxid(&[
            "run",
            "--manifest-path=oxid.toml",
            "--engine=interpreter",
            "--entry-mode=result",
        ])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_success(output: &Output, stdout: &[u8]) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, stdout, "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn published_text_checks_and_interprets_fixed_result() {
    let f = Fixture::new(None);
    let checked = f.oxid(&["check", "--manifest-path=oxid.toml"]);
    assert_eq!(checked.status.code(), Some(0), "{checked:?}");
    assert!(checked.stderr.is_empty(), "{checked:?}");
    let text = std::str::from_utf8(&checked.stdout).unwrap();
    // Function inventory changes when a package endpoint is added. Validate the
    // exact summary shape without guessing a compiler-reported function count.
    let count = text
        .strip_prefix("typed-preview check ok (")
        .and_then(|s| s.strip_suffix(" functions; check only)\n"))
        .expect("one successful check summary");
    assert!(count.parse::<usize>().unwrap() > 0, "{text}");
    assert_success(&f.reference(), RESULT);
}

#[test]
fn text_private_storage_and_moved_owner_are_rejected() {
    // These diagnostic identities are established by the ordinary private
    // record and owned-value checks. Do not guess source span coordinates.
    for (source, code, stage, message) in [
        (
            "use alloc::bytes::len; use text::Text; use text::new_text; fn inspect(document:&Text)->i32 { return len(&*document.bytes); } fn main()->i32 { let document=new_text(); return inspect(&document); }",
            "E0206",
            "type",
            "record field is private to its declaring module",
        ),
        (
            "use text::new_text; use text::byte_len; fn main()->i32 { let document=new_text(); let moved=document; return byte_len(&document); }",
            "E0310",
            "ownership",
            "owned value is not available on every path",
        ),
    ] {
        let f = Fixture::new(None);
        fs::write(f.project.join("src/main.ox"), source).unwrap();
        let output = f.oxid(&[
            "check",
            "--manifest-path=oxid.toml",
            "--message-format=json",
        ]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "exactly one diagnostic: {text}");
        let prefix = format!(
            "{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"{code}\",\"stage\":\"{stage}\",\"message\":\"{message}\",\"primary\":{{"
        );
        assert!(lines[0].starts_with(&prefix), "{text}");
        assert_eq!(lines[1], CHECK_FAILED, "{text}");
    }
}

fn llvm_bin() -> PathBuf {
    let dir =
        PathBuf::from(std::env::var_os("OXID_TEST_LLVM_BIN").expect(
            "native Text regression requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin directory)",
        ));
    // Resolve relative opt-in paths before changing to the fixture directory.
    let dir = fs::canonicalize(dir).expect("LLVM bin directory must exist");
    for (tool, marker) in [
        ("clang", "clang version 19.1.7"),
        ("opt", "LLVM version 19.1.7"),
        ("ld.lld", "LLD 19.1.7"),
    ] {
        let version = Command::new(dir.join(tool))
            .arg("--version")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(version.status.success(), "{tool}: {version:?}");
        assert!(
            String::from_utf8_lossy(&version.stdout).contains(marker),
            "{tool}: {version:?}"
        );
    }
    dir
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN; no installation or fallback"]
fn standalone_native_text_matches_fixed_reference_result() {
    let f = Fixture::new(Some(llvm_bin()));
    assert_success(&f.reference(), RESULT);
    let artifact = f
        .project
        .join(".oxid/typed-preview/result/owned-utf8-text-prototype");
    assert!(!artifact.exists());
    let compiled = f.oxid(&["build", "--manifest-path=oxid.toml", "--entry-mode=result"]);
    assert_success(
        &compiled,
        b"typed-preview native build ok: \".oxid/typed-preview/result/owned-utf8-text-prototype\"\n",
    );
    assert!(artifact.is_file(), "compiler must produce the artifact");
    let executed = Command::new(&artifact)
        .current_dir("/")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_success(&executed, RESULT);
}
