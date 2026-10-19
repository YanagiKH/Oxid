use std::process::{Command, Output};

fn doctor(args: &[&str], llvm: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oxid"))
        .args(args)
        .env("OXID_LLVM_BIN", llvm)
        .output()
        .unwrap()
}

#[test]
fn typed_doctor_rejects_json_before_toolchain_lookup() {
    let out = doctor(
        &["doctor", "--edition=typed-preview", "--message-format=json"],
        std::path::Path::new("/nonexistent-oxid-doctor-tools"),
    );
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("E0001"), "{text}");
    assert!(!text.contains("E0701"), "{text}");
    assert!(!text.contains("toolchain ready"), "{text}");
}

#[test]
fn typed_doctor_missing_toolchain_is_failure() {
    let out = doctor(
        &["doctor", "--edition=typed-preview"],
        std::path::Path::new("/nonexistent-oxid-doctor-tools"),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8(out.stderr).unwrap().contains("E0701"));
}

#[test]
#[ignore = "requires an explicitly provided LLVM 19.1.7 toolchain on Linux x86_64"]
fn typed_doctor_compiles_and_links_real_runtime() {
    let llvm = std::env::var_os("OXID_TEST_LLVM_BIN")
        .expect("set OXID_TEST_LLVM_BIN for this opt-in test");
    let out = doctor(
        &["doctor", "--edition=typed-preview"],
        std::path::Path::new(&llvm),
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    assert_eq!(out.stdout, b"typed native toolchain ready\ntarget: x86_64-unknown-linux-gnu\nLLVM: 19.1.7\nprobe: bundled runtime compiled and linked; program not executed\n");
}
