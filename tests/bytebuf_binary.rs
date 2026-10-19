//! Focused public acceptance for the ordinary three-package binary application.
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn acceptance(engine: &str) {
    let evidence = std::env::temp_dir().join(format!(
        "oxid-oxrev-{engine}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let output = Command::new("python3")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/accept_oxrev.py"))
        .arg(env!("CARGO_BIN_EXE_oxid"))
        .args(["--engine", engine, "--evidence"])
        .arg(&evidence)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "evidence={} {output:?}",
        evidence.display()
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("bytes=8199 exit=0"));
    std::fs::remove_dir_all(evidence).unwrap();
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn bytebuf_public_oxrev_reference_binary() {
    acceptance("interpreter");
}

#[test]
#[ignore = "requires already installed Linux x86_64 LLVM 19.1.7; independent native acceptance"]
fn bytebuf_public_oxrev_native_deployment() {
    acceptance("native");
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn bytebuf_public_process_manifest_errors_do_not_print_results() {
    let manifest =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/projects/oxrev/oxid.toml");
    for extra in [vec!["--message-format=json"], vec!["--output=unused"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(["run", "--manifest-path"])
            .arg(&manifest)
            .args(["--package", "oxrev", "--entry-mode=process"])
            .args(extra)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("error[E0001]"),
            "{output:?}"
        );
    }
}
