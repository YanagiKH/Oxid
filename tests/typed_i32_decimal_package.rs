//! Client acceptance for the pure typed `packages/i32_decimal` path package
//! through the independent user project `examples/projects/i32_decimal_contracts`
//! (dependency alias `decimal`). The client imports the public `ParseResult` and
//! `FormatResult` enums and `parse_i32` / `format_i32`. Every expectation is
//! hand-written in the project source and never produced by the package.
//! The published project and package are copied into a fresh repo-shaped temp
//! tree (relative dependency path preserved); the in-tree project is never
//! built. Coverage is ordinary check, interpreter Result run (exact `42\n`,
//! exit 0, empty stderr) and interpreter `test` reporting every named case
//! below as PASS. No native/LLVM run, capacity or fuel pressure, or allocation
//! fault injection; no compiler or builtin change.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const RESULT: &[u8] = b"42\n";

/// Source inputs copied into each fixture, relative to the repository root.
const SOURCES: [&str; 4] = [
    "examples/projects/i32_decimal_contracts/oxid.toml",
    "examples/projects/i32_decimal_contracts/src/main.ox",
    "packages/i32_decimal/oxid.toml",
    "packages/i32_decimal/src/lib.ox",
];

/// Root `test_` declarations of the contracts project, in source order.
const CASES: &[&str] = &[
    "test_parse_signed_zero_and_units",
    "test_parse_digit_count_boundaries",
    "test_parse_extremes_and_explicit_plus",
    "test_parse_leading_zero_boundaries",
    "test_parse_empty_and_sign_only",
    "test_parse_extra_signs_rejected",
    "test_parse_whitespace_rejected",
    "test_parse_underscore_point_and_comma_rejected",
    "test_parse_nul_and_high_bytes_rejected",
    "test_parse_digit_neighbors_and_letters_rejected",
    "test_parse_positive_overflow_index",
    "test_parse_negative_overflow_index",
    "test_parse_long_overflow_strings",
    "test_parse_first_failure_left_to_right",
    "test_parse_leaves_input_unchanged",
    "test_format_small_values_exact",
    "test_format_extremes_exact",
    "test_format_zero_length_buffer",
    "test_format_one_short_keeps_whole_buffer",
    "test_format_exact_buffer",
    "test_format_oversized_keeps_suffix",
    "test_round_trip_supplement",
];

struct Fixture {
    root: PathBuf,
    project: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-i32-decimal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        for path in SOURCES {
            let destination = root.join(path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(repository.join(path), destination).unwrap();
        }
        let project = root.join("examples/projects/i32_decimal_contracts");
        Self { root, project }
    }

    fn oxid(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.project)
            .env("OXID_LLVM_BIN", self.root.join("absent-llvm"))
            .env("OXID_CACHE_DIR", self.root.join("cache"))
            .env("PATH", self.root.join("absent-path"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .output()
            .unwrap()
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
fn decimal_contracts_check_and_interpret_fixed_result() {
    let f = Fixture::new();
    let checked = f.oxid(&["check", "--manifest-path=oxid.toml"]);
    assert_eq!(checked.status.code(), Some(0), "{checked:?}");
    assert!(checked.stderr.is_empty(), "{checked:?}");
    let text = std::str::from_utf8(&checked.stdout).unwrap();
    // The inventory includes package functions this client does not fix, so
    // validate the exact summary shape without guessing the count.
    let count = text
        .strip_prefix("typed-preview check ok (")
        .and_then(|s| s.strip_suffix(" functions; check only)\n"))
        .expect("one successful check summary");
    assert!(count.parse::<usize>().unwrap() > 0, "{text}");
    let ran = f.oxid(&[
        "run",
        "--manifest-path=oxid.toml",
        "--engine=interpreter",
        "--entry-mode=result",
    ]);
    assert_success(&ran, RESULT);
}

#[test]
fn decimal_contracts_pass_every_named_case() {
    let f = Fixture::new();
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
