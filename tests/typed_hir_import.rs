//! Public opt-in bridge contracts. Producer captures remain untrusted inputs.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const SOURCE: &[u8] = include_bytes!("fixtures/checked_hir_import/rich-source.txt");
const WIRE: &[u8] = include_bytes!("fixtures/checked_hir_import/rich-success.bin");
struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &[u8], wire: &[u8]) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "oxid-hir-public-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("source.ox"), source).unwrap();
        fs::write(dir.join("observation.bin"), wire).unwrap();
        Self(dir)
    }
    fn command(&self, operation: &str, imported: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command.current_dir(&self.0).args([
            operation,
            "source.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ]);
        if imported {
            command.arg("--experimental-hir-import=observation.bin");
        }
        if operation == "compile" {
            command.args(["--backend=llvm", "--output=program.elf"]);
        }
        command
    }
    fn run(&self, operation: &str, imported: bool) -> Output {
        self.command(operation, imported)
            .env("OXID_LLVM_BIN", self.0.join("missing-tools"))
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn equal(left: Output, right: Output) {
    assert_eq!(left.status.code(), right.status.code());
    assert_eq!(left.stdout, right.stdout);
    assert_eq!(left.stderr, right.stderr);
}
fn rejected(output: Output, code: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(&format!("\"code\":\"{code}\"")), "{text}");
    assert!(text.contains("\"success\":false"), "{text}");
}

#[test]
fn imported_original_and_public_check_run_preserve_normal_results() {
    for (source, wire) in [
        (SOURCE, WIRE),
        (
            include_bytes!("fixtures/checked_hir_import/public-source.txt").as_slice(),
            include_bytes!("fixtures/checked_hir_import/public-success.bin").as_slice(),
        ),
    ] {
        let fixture = Fixture::new(source, wire);
        for op in ["check", "run"] {
            let actual = fixture.run(op, true);
            assert!(actual.status.success(), "{actual:?}");
            equal(actual, fixture.run(op, false));
        }
    }
}

#[test]
fn imported_library_preserves_check_and_canonical_entry_errors() {
    let fixture = Fixture::new(
        include_bytes!("fixtures/checked_hir_import/scalar-arithmetic-source.txt"),
        include_bytes!("fixtures/checked_hir_import/scalar-arithmetic-success.bin"),
    );
    assert!(fixture.run("check", true).status.success());
    for op in ["check", "run", "compile"] {
        equal(fixture.run(op, true), fixture.run(op, false));
    }
}

fn arithmetic_cases() -> [(&'static [u8], &'static [u8]); 2] {
    [
        (
            include_bytes!("fixtures/checked_hir_import/synthetic-overflow-source.txt"),
            include_bytes!("fixtures/checked_hir_import/synthetic-overflow-success.bin"),
        ),
        (
            include_bytes!("fixtures/checked_hir_import/synthetic-division-source.txt"),
            include_bytes!("fixtures/checked_hir_import/synthetic-division-success.bin"),
        ),
    ]
}

#[test]
fn imported_arithmetic_preserves_full_canonical_runtime_diagnostics() {
    for ((source, wire), code) in arithmetic_cases().into_iter().zip(["E0604", "E0607"]) {
        let fixture = Fixture::new(source, wire);
        assert!(fixture.run("check", true).status.success());
        rejected(fixture.run("run", true), code);
        equal(fixture.run("run", true), fixture.run("run", false));
    }
}

#[test]
fn malformed_partial_trailing_and_corrupt_observations_never_fallback() {
    let mut variants = vec![
        vec![],
        WIRE[..2606].to_vec(),
        WIRE[..1575].to_vec(),
        [WIRE, &[0]].concat(),
        [WIRE, &vec![0; 1 << 20]].concat(),
    ];
    for offset in [0, 4, 8, 9, 10, 11, 1559, 1563, 1575, 2091] {
        let mut wire = WIRE.to_vec();
        wire[offset] ^= 1;
        variants.push(wire);
    }
    for wire in variants {
        let fixture = Fixture::new(SOURCE, &wire);
        assert!(fixture.run("run", false).status.success());
        for operation in ["check", "run", "compile"] {
            rejected(fixture.run(operation, true), "E0702");
        }
        assert!(!fixture.0.join("program.elf").exists());
    }
}

#[test]
fn stale_source_and_expanded_domains_are_rejected_without_native_publication() {
    let mut stale = SOURCE.to_vec();
    let index = stale.windows(4).position(|w| w == b"f(1)").unwrap();
    stale[index + 2] = b'2';
    for source in [
        stale,
        [SOURCE, b"                "].concat(),
        "//é\nfn main()->i32{return 1;}".as_bytes().to_vec(),
    ] {
        let fixture = Fixture::new(&source, WIRE);
        rejected(fixture.run("run", true), "E0702");
    }
}

#[test]
fn source_diagnostics_precede_missing_artifact_and_ordinary_route_ignores_artifact() {
    let fixture = Fixture::new(b"fn main( {", WIRE);
    fs::remove_file(fixture.0.join("observation.bin")).unwrap();
    equal(fixture.run("check", true), fixture.run("check", false));
    let valid = Fixture::new(SOURCE, WIRE);
    fs::remove_file(valid.0.join("observation.bin")).unwrap();
    assert!(valid.run("run", false).status.success());
    rejected(valid.run("run", true), "E0702");
    fs::create_dir(valid.0.join("observation.bin")).unwrap();
    rejected(valid.run("run", true), "E0702");
}

#[test]
fn imported_native_compile_preserves_no_replace_before_tool_access() {
    let fixture = Fixture::new(SOURCE, WIRE);
    fs::write(fixture.0.join("program.elf"), b"keep original").unwrap();
    rejected(fixture.run("compile", true), "E0701");
    assert_eq!(
        fs::read(fixture.0.join("program.elf")).unwrap(),
        b"keep original"
    );
    fs::remove_file(fixture.0.join("program.elf")).unwrap();
    rejected(fixture.run("compile", true), "E0701");
    assert!(!fixture.0.join("program.elf").exists());
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "explicit Linux x86_64 LLVM19.1.7 integration; requires OXID_LLVM_BIN"]
fn imported_public_compile_produces_source_independent_executable() {
    std::env::var_os("OXID_LLVM_BIN").expect("explicit LLVM tool directory required");
    for (source, wire) in [(SOURCE, WIRE)].into_iter().chain(arithmetic_cases()) {
        let fixture = Fixture::new(source, wire);
        let expected = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&fixture.0)
            .args(["run", "source.ox", "--edition=typed-preview"])
            .output()
            .unwrap();
        let compiled = fixture.command("compile", true).output().unwrap();
        assert!(compiled.status.success(), "{compiled:?}");
        fs::remove_file(fixture.0.join("source.ox")).unwrap();
        fs::remove_file(fixture.0.join("observation.bin")).unwrap();
        let result = Command::new(fixture.0.join("program.elf"))
            .current_dir(&fixture.0)
            .env_clear()
            .env("PATH", "/no-tools")
            .output()
            .unwrap();
        equal(result, expected);
    }
}
