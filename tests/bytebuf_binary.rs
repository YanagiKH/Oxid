//! Public binary I/O acceptance through a separate Python byte oracle.
#![cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn acceptance(reference_only: bool) {
    let evidence = std::env::temp_dir().join(format!(
        "oxid-byte-reverse-{}-{}-{}",
        if reference_only {
            "reference"
        } else {
            "native"
        },
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut command = Command::new("python3");
    command
        .arg("-B")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/verify_byte_reverse_app.py"))
        .arg(env!("CARGO_BIN_EXE_oxid"))
        .arg("--output")
        .arg(&evidence);
    if reference_only {
        command.arg("--reference-only");
    } else {
        command.arg("--llvm-bin").arg(
            std::env::var_os("OXID_LLVM_BIN")
                .expect("native acceptance requires LLVM 19.1.7 in OXID_LLVM_BIN"),
        );
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "retained evidence={} {output:?}",
        evidence.display()
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let mut validation = Command::new("python3")
        .args([
            "-B",
            "-c",
            r#"import json, pathlib, sys
reference_only = sys.argv[2] == "true"
summary = json.load(sys.stdin)
result = json.loads((pathlib.Path(sys.argv[1]) / "result.json").read_text())
assert summary["status"] == "passed", summary
assert summary["reference_only"] is reference_only, summary
assert summary["maximum_input_bytes"] == 8199, summary
assert result["status"] == "passed", result["status"]
assert result["reference_only"] is reference_only, result["reference_only"]
assert isinstance(result["builds"], list) and len(result["builds"]) == (0 if reference_only else 8)
assert isinstance(result["deployments"], list) and len(result["deployments"]) == (0 if reference_only else 2)
"#,
        ])
        .arg(&evidence)
        .arg(if reference_only { "true" } else { "false" })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    validation
        .stdin
        .take()
        .unwrap()
        .write_all(&output.stdout)
        .unwrap();
    let validated = validation.wait_with_output().unwrap();
    assert!(
        validated.status.success(),
        "retained evidence={} {validated:?}",
        evidence.display()
    );
    std::fs::remove_dir_all(evidence).unwrap();
}
#[test]
fn bytebuf_public_byte_reverse_reference_bytes_and_refusals() {
    acceptance(true);
}

#[test]
#[ignore = "requires real Linux x86_64 or ARM64 LLVM 19.1.7 in OXID_LLVM_BIN"]
fn bytebuf_public_byte_reverse_native_cache_relocation_and_deployment() {
    acceptance(false);
}

#[test]
fn bytebuf_public_process_manifest_errors_do_not_print_results() {
    let application = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("apps/byte-reverse/app");
    for extra in [
        "--message-format=json",
        "--output=unused",
        "--profile=release",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&application)
            .args(["run", extra])
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
