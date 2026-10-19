//! Focused acceptance for the `oxutf8` Process CLI workspace in
//! `examples/projects/oxutf8` (members `oxutf8` and `oxutf8-contracts`).
//! Published sources are copied to a fresh repo-shaped directory by
//! `scripts/accept_oxutf8.py`; checks never build the in-tree workspace for
//! isolation. Expected results are fixed: every valid corpus input is copied
//! byte-for-byte to stdout with exit 0; invalid UTF-8 exits 65 with empty
//! stdout; stderr stays empty. Native coverage builds, packages, and runs the
//! deployed binary through `scripts/cli_driver.py` (segmented stdin, close EOF,
//! cwd=/, empty env). Run the ignored native test explicitly with:
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test owned_utf8_package -- --include-ignored`.
//! Exit codes 70, 71 and 74 are documented, not forced. No capacity, fuel,
//! fault-injection or qualification probes. The driver framework is not rewritten.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn acceptance(engine: &str) {
    let evidence = std::env::temp_dir().join(format!(
        "oxid-oxutf8-{engine}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut command = Command::new("python3");
    command
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/accept_oxutf8.py"))
        .arg(env!("CARGO_BIN_EXE_oxid"))
        .args(["--engine", engine, "--evidence"])
        .arg(&evidence);
    if engine == "native" {
        let llvm = std::env::var_os("OXID_TEST_LLVM_BIN").expect(
            "native oxutf8 acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin directory)",
        );
        command.env("OXID_LLVM_BIN", &llvm);
        command.env("OXID_TEST_LLVM_BIN", &llvm);
    } else {
        command.env_remove("OXID_LLVM_BIN");
        command.env_remove("OXID_TEST_LLVM_BIN");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "evidence={} {output:?}",
        evidence.display()
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("valid-8199 bytes=8199 exit=0"), "{text}");
    assert!(text.contains("invalid-overlong bytes=4 exit=65"), "{text}");
    let _ = std::fs::remove_dir_all(evidence);
}

#[test]
fn owned_utf8_package_interpreter_acceptance() {
    acceptance("interpreter");
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN; no installation or fallback"]
fn owned_utf8_package_native_packaged_acceptance() {
    acceptance("native");
}
