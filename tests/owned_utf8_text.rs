//! Regression coverage for the ordinary owned UTF-8 Text source prototype.
//! The published project is copied to a fresh directory, never built in-tree.
//! Expected results are fixed: check succeeds; result-mode execution prints
//! exactly `42\n`, exits 0, and leaves stderr empty. Native coverage compiles
//! and executes a standalone artifact; it does not claim manifest-native RUN.
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
const CHECK: &[u8] = b"typed-preview check ok (32 functions; check only)\n";
const CHECK_FAILED: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}";

struct Fixture {
    root: PathBuf,
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
        fs::create_dir(root.join("src")).unwrap();
        let project =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/projects/owned_utf8_text");
        // Only retained source inputs: no caches, prior artifacts or README.
        for path in ["oxid.toml", "src/main.ox", "src/text.ox"] {
            fs::copy(project.join(path), root.join(path)).unwrap();
        }
        Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
        }
    }

    fn oxid(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.root)
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
    assert_success(&f.oxid(&["check", "--manifest-path=oxid.toml"]), CHECK);
    assert_success(&f.reference(), RESULT);
}

#[test]
fn text_private_storage_and_moved_owner_are_rejected() {
    // These diagnostic identities are established by the ordinary private
    // record and owned-value checks. Do not guess source span coordinates.
    for (source, code, stage, message) in [
        (
            "mod text; use alloc::bytes::len; use crate::text::Text; use crate::text::new_text; fn inspect(document:&Text)->i32 { return len(&*document.bytes); } fn main()->i32 { let document=new_text(); return inspect(&document); }",
            "E0206",
            "type",
            "record field is private to its declaring module",
        ),
        (
            "mod text; use crate::text::new_text; use crate::text::byte_len; fn main()->i32 { let document=new_text(); let moved=document; return byte_len(&document); }",
            "E0310",
            "ownership",
            "owned value is not available on every path",
        ),
    ] {
        let f = Fixture::new(None);
        fs::write(f.root.join("src/main.ox"), source).unwrap();
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
    let artifact = f.root.join("owned-utf8-text");
    assert!(!artifact.exists());
    let compiled = f.oxid(&[
        "compile",
        "src/main.ox",
        "--edition=typed-preview",
        "--entry-mode=result",
        "--backend=llvm",
        "--target=x86_64-unknown-linux-gnu",
        "--output=owned-utf8-text",
    ]);
    assert_success(
        &compiled,
        b"typed-preview native compile ok: \"owned-utf8-text\"\n",
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
