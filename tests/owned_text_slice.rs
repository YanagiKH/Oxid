//! Client acceptance for `text::slice(&Text, i32, i32) -> SliceResult` in the
//! ordinary utf8_text path package. The published project is copied to a fresh
//! directory, never built in-tree. Expected results are fixed in advance and
//! never taken from the package: check succeeds; result-mode execution prints
//! exactly `42\n`, exits 0, and leaves stderr empty; the contracts suite reports
//! every named case below as PASS. Every expected slice is a hand-written raw
//! byte array in the project source, so no host UTF-8 decoder is involved.
//! Run the ignored native test explicitly with:
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test owned_text_slice -- --include-ignored`.
//! No compiler changes, capacity or fuel pressure, or allocation fault injection.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const RESULT: &[u8] = b"42\n";
const ARTIFACT: &str = ".oxid/typed-preview/result/owned-text-slice";

/// Root `test_` declarations of the contracts project, in source order.
const CASES: &[&str] = &[
    "test_full_range_copies_every_byte",
    "test_prefix_middle_suffix_ranges",
    "test_each_scalar_width_alone",
    "test_ascii_and_nul_subranges",
    "test_empty_ranges_on_boundaries",
    "test_interior_start_rejected",
    "test_interior_end_rejected",
    "test_interior_empty_and_both_interior_rejected",
    "test_negative_bounds_rejected",
    "test_reversed_bounds_rejected",
    "test_out_of_range_bounds_rejected",
    "test_changing_result_text_keeps_original",
    "test_changing_result_bytes_keeps_original",
    "test_changing_original_keeps_result",
];

struct Fixture {
    root: PathBuf,
    project: PathBuf,
    llvm: PathBuf,
}
impl Fixture {
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-owned-text-slice-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        let project = root.join("examples/projects/owned_text_slice");
        // Preserve manifest-relative dependency paths; copy only source inputs.
        for path in [
            "examples/projects/owned_text_slice/oxid.toml",
            "examples/projects/owned_text_slice/src/main.ox",
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
fn slice_contracts_check_and_interpret_fixed_result() {
    let f = Fixture::new(None);
    let checked = f.oxid(&["check", "--manifest-path=oxid.toml"]);
    assert_eq!(checked.status.code(), Some(0), "{checked:?}");
    assert!(checked.stderr.is_empty(), "{checked:?}");
    let text = std::str::from_utf8(&checked.stdout).unwrap();
    // The function inventory includes package functions the client does not
    // fix. Validate the exact summary shape without guessing that count.
    let count = text
        .strip_prefix("typed-preview check ok (")
        .and_then(|s| s.strip_suffix(" functions; check only)\n"))
        .expect("one successful check summary");
    assert!(count.parse::<usize>().unwrap() > 0, "{text}");
    assert_success(&f.reference(), RESULT);
}

#[test]
fn slice_contracts_pass_every_named_case() {
    let f = Fixture::new(None);
    let tested = f.oxid(&["test", "--manifest-path=oxid.toml", "--engine=interpreter"]);
    let mut expected = String::new();
    for case in CASES {
        expected.push_str(&format!("PASS {case}\n"));
    }
    let total = CASES.len();
    expected.push_str(&format!(
        "test result: {total} passed; 0 failed; {total} total\n"
    ));
    assert_success(&tested, expected.as_bytes());
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN; no installation or fallback"]
fn slice_contracts_pass_every_named_case_natively() {
    let f = Fixture::new(Some(llvm_bin()));
    let tested = f.oxid(&["test", "--manifest-path=oxid.toml", "--engine=native"]);
    let mut expected = String::new();
    for case in CASES {
        expected.push_str(&format!("PASS {case}\n"));
    }
    let total = CASES.len();
    expected.push_str(&format!(
        "test result: {total} passed; 0 failed; {total} total\n"
    ));
    assert_success(&tested, expected.as_bytes());
}

fn llvm_bin() -> PathBuf {
    let dir = PathBuf::from(std::env::var_os("OXID_TEST_LLVM_BIN").expect(
        "native Text slice acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin directory)",
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
fn standalone_native_slice_matches_fixed_reference_result() {
    let f = Fixture::new(Some(llvm_bin()));
    assert_success(&f.reference(), RESULT);
    let artifact = f.project.join(ARTIFACT);
    assert!(!artifact.exists());
    let compiled = f.oxid(&["build", "--manifest-path=oxid.toml", "--entry-mode=result"]);
    assert_success(
        &compiled,
        format!("typed-preview native build ok: \"{ARTIFACT}\"\n").as_bytes(),
    );
    assert!(artifact.is_file(), "compiler must produce the artifact");
    // Execute a copy outside the source tree after the sources are removed.
    let standalone = f.root.join("standalone/owned-text-slice");
    fs::create_dir_all(standalone.parent().unwrap()).unwrap();
    fs::copy(&artifact, &standalone).unwrap();
    fs::remove_dir_all(f.root.join("examples")).unwrap();
    fs::remove_dir_all(f.root.join("packages")).unwrap();
    let executed = Command::new(&standalone)
        .current_dir("/")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_success(&executed, RESULT);
}
