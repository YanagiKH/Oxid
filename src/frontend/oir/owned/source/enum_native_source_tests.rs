//! Explicit closed-source enum/native gates. Expectations are frozen source
//! oracles, never a reconstructed execution schedule. Artifacts are retained.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use super::{resolve, reviewer_source};
use crate::frontend::{
    declaration_index::{
        collect_enum_candidate, DeclarationIndex, IndexLimits, SourceOwner, WorkMeter,
    },
    diagnostic::Diagnostic,
    oir::{owned::execute::OwnedRunFailure, owned::plan::MAX_FUEL, RunFailure, Scalar},
    project::{budget::Allocator, ProjectLimits, ProjectSources},
    source::{SourceMap, Span},
};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
};

// source-enum-independent-fixtures-d6249e7.json, SHA256
// 7e56b2ab048462b322adcac4d9773066a53a7ffb51522db0d49fa0c7bc083bbd.
const TINY: &str = "enum E{N,V(i32)} fn relay(x:E)->E{return x;} fn take(x:E)->i32{match x{E::N=>{return 0;},E::V(v)=>{return v;},}} fn main()->i32{return take(relay(E::V(7)));}";
const REORDERED: &str = "enum E{N,V(i32)} fn relay(x:E)->E{return x;} fn take(x:E)->i32{match x{E::V(v)=>{return v;},E::N=>{return 0;},}} fn main()->i32{return take(relay(E::V(7)));}";
const GUARD: &str = " fn guard()->(){while true{} return;}";
const SCANNER_MAIN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/bounded_enum_scanner/main.ox"
));
const SCANNER_BODY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/bounded_enum_scanner/scanner.ox"
));
const CODES: &str = "let codes = [49, 50, 32, 43, 32, 51];";
const SCANNER_SHA256: &str = "d13847eaecd69667031e54bb6f3f329365427a483e0bff537d0b28882516ac14";

fn new_file(path: &Path) -> File {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap_or_else(|error| panic!("fresh evidence {}: {error}", path.display()))
}

fn save(path: &Path, bytes: impl AsRef<[u8]>) {
    new_file(path).write_all(bytes.as_ref()).unwrap();
}

fn directory(parent: &Path, name: &str) -> PathBuf {
    let path = parent.join(name);
    fs::create_dir(&path).expect("evidence directory must be fresh; never remove prior evidence");
    path
}

fn hash(path: &Path) -> String {
    let output = Command::new("sha256sum")
        .arg("--")
        .arg(path)
        .output()
        .expect("explicit native gate requires sha256sum");
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    let digest = text.split_whitespace().next().unwrap();
    assert!(digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    digest.into()
}

fn hash_receipt(path: &Path, receipt: &Path, expected: Option<&str>) {
    let digest = hash(path);
    save(receipt, format!("{digest}  {}\n", path.display()));
    if let Some(expected) = expected {
        assert_eq!(
            digest,
            expected,
            "frozen source identity: {}",
            path.display()
        );
    }
}

fn process_receipt(root: &Path, name: &str, output: &Output, invocation: &str) {
    save(&root.join(format!("{name}.stdout")), &output.stdout);
    save(&root.join(format!("{name}.stderr")), &output.stderr);
    save(
        &root.join(format!("{name}.result.txt")),
        format!(
            "invocation={invocation}\nstatus={:?}\n",
            output.status.code()
        ),
    );
}

fn evidence_group(name: &str) -> PathBuf {
    let configured = std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE")
        .expect("explicit native gate requires OXID_OWNED_NATIVE_EVIDENCE");
    let parent =
        fs::canonicalize(configured).expect("parent must provide a fresh evidence directory");
    assert!(parent.is_dir());
    let root = directory(&parent, name);
    let llvm = fs::canonicalize(
        std::env::var_os("OXID_LLVM_BIN").expect("explicit native gate requires OXID_LLVM_BIN"),
    )
    .expect("configured pinned LLVM bin directory");
    assert!(llvm.is_dir());
    for (tool, marker) in [
        ("clang", "clang version"),
        ("opt", "LLVM version"),
        ("ld.lld", "LLD"),
        ("llvm-as", "LLVM version"),
    ] {
        let path = llvm.join(tool);
        let output = Command::new(&path).arg("--version").output().unwrap();
        process_receipt(
            &root,
            tool,
            &output,
            &format!("{} --version", path.display()),
        );
        assert!(output.status.success());
        let expected = format!("{marker} 19.1.7");
        assert!(String::from_utf8_lossy(&output.stdout).lines().any(|line| {
            line.trim()
                .split_once(&expected)
                .is_some_and(|(_, after)| after.is_empty() || after.starts_with([' ', '(']))
        }));
    }
    for (name, args) in [
        ("git-head", vec!["rev-parse", "HEAD"]),
        ("git-status", vec!["status", "--porcelain=v1"]),
        (
            "git-diff",
            vec!["diff", "--no-ext-diff", "--binary", "HEAD"],
        ),
    ] {
        let output = Command::new("git")
            .arg("--no-optional-locks")
            .args(&args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("explicit native gate requires read-only Git identity receipts");
        process_receipt(&root, name, &output, &format!("git {}", args.join(" ")));
        assert!(output.status.success());
    }
    // Bind the compiled test bytes even when this new file is still untracked.
    let harness = root.join("test-source.rs");
    save(&harness, include_str!("enum_native_source_tests.rs"));
    hash_receipt(&harness, &root.join("test-source.sha256"), None);
    hash_receipt(
        &std::env::current_exe().unwrap(),
        &root.join("test-binary.sha256"),
        None,
    );
    save(&root.join("scope.txt"), "Linux x86_64; LLVM 19.1.7; frontend::native::compile (opt verify, O0, clang/lld, PIE, no-replace publication).\nRun cwd contains only program.elf; env_clear; PATH=/no-tools. This is a source-free runtime dependency check, not an OS filesystem sandbox.\nGit receipts describe the working tree at test execution; the test-binary hash binds the executable actually run.\n");
    root
}

fn with_project(
    root: &Path,
    main: &str,
    main_hash: Option<&str>,
    scanner: bool,
    action: impl FnOnce(&DeclarationIndex<'_>, &SourceMap),
) {
    // Fixture I/O, parsing, the source owner and index all precede measurement.
    let source_dir = directory(root, "source");
    let main_path = source_dir.join("main.ox");
    save(&main_path, main);
    hash_receipt(&main_path, &root.join("main.source.sha256"), main_hash);
    if scanner {
        let path = source_dir.join("scanner.ox");
        save(&path, SCANNER_BODY);
        hash_receipt(
            &path,
            &root.join("scanner.source.sha256"),
            Some(SCANNER_SHA256),
        );
    }
    let project = ProjectSources::load_enum_index_candidate(
        main_path.to_str().unwrap(),
        ProjectLimits::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_enum_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    assert_eq!(project.sources().files().len(), if scanner { 2 } else { 1 });
    action(&index, project.sources());
}

#[derive(Clone, Copy)]
enum Expected {
    Value(i32),
    Overflow(Span),
    Fuel(Span),
}

impl Expected {
    fn result(self) -> Result<Scalar, OwnedRunFailure> {
        match self {
            Self::Value(value) => Ok(Scalar::I32(value)),
            Self::Overflow(span) => Err(OwnedRunFailure::Scalar(RunFailure::Overflow(span))),
            Self::Fuel(span) => Err(OwnedRunFailure::Scalar(RunFailure::Fuel(span))),
        }
    }

    fn output(self, sources: &SourceMap) -> (i32, Vec<u8>, Vec<u8>) {
        let (code, message, span) = match self {
            Self::Value(value) => return (0, format!("{value}\n").into_bytes(), Vec::new()),
            Self::Overflow(span) => ("E0604", "checked i32 arithmetic overflow", span),
            Self::Fuel(span) => ("E0601", "execution fuel exhausted", span),
        };
        // Independently selected code, message and frozen source origin. The
        // shared renderer supplies only the ordinary human diagnostic format.
        let diagnostic = Diagnostic::new(code, "oir-run", message, Some(span));
        (1, Vec::new(), diagnostic.render_human(sources).into_bytes())
    }
}

fn probe(
    root: &Path,
    name: &str,
    index: &DeclarationIndex<'_>,
    sources: &SourceMap,
    request: resolve::EnumPipelineRequest,
    expected: Expected,
    counts: [usize; 5],
) -> Option<String> {
    let emit_llvm = request.emit_llvm;
    let fuel = request.fuel;
    let mut receipt = new_file(&root.join(format!("{name}.reference.txt")));
    let mut module = emit_llvm.then(|| new_file(&root.join(format!("{name}.ll"))));
    let mut diagnostic = new_file(&root.join(format!("{name}.reference.stderr")));
    let expected_output = expected.output(sources);
    save(
        &root.join(format!("{name}.expected.stderr")),
        &expected_output.2,
    );
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(1024).unwrap();
    let trace_capacity = allocator.trace.capacity();
    let trace_pointer = allocator.trace.as_ptr();
    let (_, (calls, live, peak)) = reviewer_source::integration_measured(|| {
        let output = match resolve::probe_enum_pipeline(index, &work, &mut allocator, request) {
            Ok(Some(output)) => output,
            other => {
                writeln!(receipt, "pipeline={other:?}").unwrap();
                panic!("closed enum source pipeline must produce its output");
            }
        };
        writeln!(
            receipt,
            "fuel={} emit_llvm={}\nobservation={:#?}",
            fuel, emit_llvm, output.observation
        )
        .unwrap();
        match (&mut module, &output.llvm) {
            (Some(file), Some(Ok(llvm))) => file.write_all(llvm.as_bytes()).unwrap(),
            (None, None) => {}
            (_, other) => {
                writeln!(receipt, "llvm={other:?}").unwrap();
                panic!("unexpected LLVM product");
            }
        }
        if let Err(error) = &output.observation.pipeline.result {
            let error = error.diagnostic(sources);
            diagnostic
                .write_all(error.render_human(sources).as_bytes())
                .unwrap();
            assert_eq!(error.render_human(sources).as_bytes(), expected_output.2);
        }
        assert_eq!(output.observation.pipeline.result, expected.result());
        let facts = &output.observation;
        assert_eq!(
            [
                facts.pipeline.enum_count,
                facts.pipeline.variant_count,
                facts.pipeline.function_count,
                facts.pipeline.match_count,
                facts.pipeline.arm_count,
            ],
            counts
        );
        assert_eq!(
            facts.pipeline.source_usage.analysis,
            facts.pipeline.raw_usage
        );
        assert_eq!(
            facts.pipeline.verified_usage.owners,
            facts.pipeline.raw_usage.owners
        );
        assert_eq!(
            facts.pipeline.verified_usage.expanded_events,
            facts.pipeline.raw_usage.expanded_events
        );
        assert_eq!(facts.pipeline.source_seed_before, facts.typed.final_cell);
        assert_eq!(facts.pipeline.source_seed_after, facts.typed.final_cell);
        assert_eq!(
            allocator.attempts,
            facts.resolver.reservation_attempts + facts.typed.typed_attempts
        );
        // The returned artifact was retained until consumed above. Its backing
        // and the complete remaining product drop before the live-zero check.
        drop(output);
    });
    writeln!(
        receipt,
        "heap_calls={calls} heap_live_after_consuming_product={live} heap_peak={peak}"
    )
    .unwrap();
    assert_eq!(live, 0);
    assert!(calls > 0 && peak > 0);
    assert_eq!(allocator.trace.capacity(), trace_capacity);
    assert_eq!(allocator.trace.as_ptr(), trace_pointer);
    assert!(!allocator.observer_trace_overflow);
    assert!(allocator.trace.iter().all(|event| event.success));
    emit_llvm.then(|| {
        let path = root.join(format!("{name}.ll"));
        hash_receipt(&path, &root.join(format!("{name}.ll.sha256")), None);
        fs::read_to_string(path).unwrap()
    })
}

fn compile(root: &Path, name: &str, module: &str) -> PathBuf {
    let run = directory(root, &format!("{name}-run"));
    let executable = run.join("program.elf");
    let result = crate::frontend::native::compile(module, executable.to_str().unwrap());
    save(
        &root.join(format!("{name}.compile.txt")),
        format!("{result:?}\n"),
    );
    result.unwrap();
    let elf = fs::read(&executable).unwrap();
    assert_eq!(elf.get(..4), Some(b"\x7fELF".as_slice()));
    assert_eq!(elf.get(4..6), Some([2, 1].as_slice())); // ELF64, little endian
    assert_eq!(elf.get(16..20), Some([3, 0, 62, 0].as_slice())); // PIE, x86_64
    assert_eq!(fs::read_dir(&run).unwrap().count(), 1);
    hash_receipt(&executable, &root.join(format!("{name}.elf.sha256")), None);
    executable
}

fn run(
    root: &Path,
    name: &str,
    executable: &Path,
    fuel: Option<usize>,
    expected: Expected,
    sources: &SourceMap,
) {
    let directory = executable.parent().unwrap();
    assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
    let mut command = Command::new(fs::canonicalize(executable).unwrap());
    if let Some(fuel) = fuel {
        command.arg(fuel.to_string());
    }
    let output = command
        .current_dir(directory)
        .env_clear()
        .env("PATH", "/no-tools")
        .output()
        .unwrap();
    process_receipt(
        root,
        name,
        &output,
        &format!(
            "{} fuel={fuel:?}; cwd={}; env_clear; PATH=/no-tools",
            executable.display(),
            directory.display()
        ),
    );
    assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
    let (status, stdout, stderr) = expected.output(sources);
    assert_eq!(output.status.code(), Some(status));
    assert_eq!(output.stdout, stdout);
    assert_eq!(output.stderr, stderr);
}

// Only the existing native_tests argv wrapper and initial store are rewritten.
// Complete function bodies and diagnostics are recoverable byte-for-byte.
fn argv_fuel_harness(module: &str) -> String {
    assert_eq!(MAX_FUEL, 1_000_000);
    let old_header = "define i32 @main() {\nentry:\n";
    let new_header = "declare i64 @strtoull(ptr, ptr, i32)\ndefine i32 @main(i32 %argc, ptr %argv) {\nentry:\n  %test_arg_slot = getelementptr ptr, ptr %argv, i64 1\n  %test_arg = load ptr, ptr %test_arg_slot\n  %test_parsed_fuel = call i64 @strtoull(ptr %test_arg, ptr null, i32 10)\n  %test_over_cap = icmp ugt i64 %test_parsed_fuel, 1000000\n  %test_fuel = select i1 %test_over_cap, i64 1000000, i64 %test_parsed_fuel\n";
    let old_store = format!("store i64 {MAX_FUEL}, ptr %fuel, align 8");
    let new_store = "store i64 %test_fuel, ptr %fuel, align 8";
    assert_eq!(module.matches(old_header).count(), 1);
    assert_eq!(
        module.matches(&old_store).count(),
        1,
        "source must select guarded ABI"
    );
    assert!(!module.contains(new_header));
    assert!(!module.contains(new_store));
    let harness = module
        .replacen(old_header, new_header, 1)
        .replacen(&old_store, new_store, 1);
    assert_eq!(harness.matches(new_header).count(), 1);
    assert_eq!(harness.matches(new_store).count(), 1);
    assert_eq!(
        harness
            .replacen(new_header, old_header, 1)
            .replacen(new_store, &old_store, 1),
        module
    );
    harness
}

fn source_span(sources: &SourceMap, file_name: &str, text: &str) -> Span {
    let source = sources
        .files()
        .iter()
        .find(|source| Path::new(source.path()).file_name().unwrap() == file_name)
        .unwrap();
    assert_eq!(source.text().matches(text).count(), 1);
    let start = source.text().find(text).unwrap();
    source.span(start, start + text.len())
}

#[test]
#[ignore = "requires reviewed enum artifact carrier gate, LLVM 19.1.7 and fresh OXID_OWNED_NATIVE_EVIDENCE"]
fn bounded_enum_pipeline_native_tiny_and_scanner_source_free() {
    let root = evidence_group("source-native-success");
    let mut artifacts = 0;
    let mut runs = 0;
    for (name, source, digest, scanner, value, counts) in [
        (
            "tiny",
            TINY,
            "95bbfb95b733bc272e2b11e6772b66a74bf14507f4a86a7f3e9258d4c88d2220",
            false,
            7,
            [1, 2, 3, 1, 2],
        ),
        (
            "scanner",
            SCANNER_MAIN,
            "768232ca66cd91a6439cf21393a8666f4ec10bcfac4d097caece354440278c3a",
            true,
            115,
            [1, 4, 3, 1, 4],
        ),
    ] {
        let case = directory(&root, name);
        with_project(&case, source, Some(digest), scanner, |index, sources| {
            let expected = Expected::Value(value);
            let module = probe(
                &case,
                "production",
                index,
                sources,
                resolve::EnumPipelineRequest {
                    emit_llvm: true,
                    fuel: MAX_FUEL,
                },
                expected,
                counts,
            )
            .unwrap();
            let store = format!("store i64 {MAX_FUEL}, ptr %fuel, align 8");
            assert_eq!(module.matches(&store).count(), usize::from(scanner));
            let executable = compile(&case, "production", &module);
            artifacts += 1;
            run(&case, "production", &executable, None, expected, sources);
            runs += 1;
        });
    }
    assert_eq!((artifacts, runs), (2, 2));
    println!("ENUM_SOURCE_NATIVE_SUCCESS artifacts={artifacts} runs={runs}");
    save(&root.join("completed.txt"), "artifacts=2 runs=2\nUntouched tiny=7 and canonical two-file scanner=115; reference and source-free production ELF agree.\n");
}

// Exact existing scanner-oracle.json order (SHA256
// b0f2905ac99b3be36685a0ca49ca133d941bc4cafd9341674f8c8bc6a5a8cf8f).
// Token/cursor fields remain expectations; only consumer result/diagnostic is
// observed. No runtime token or cursor instrumentation is introduced.
struct ScannerCase {
    codes: &'static [i32],
    tokens: &'static str,
    cursor: usize,
    result: Result<i32, (usize, usize)>, // frozen overflow byte offset, column
}
const SCANNER_CASES: [ScannerCase; 12] = [
    ScannerCase {
        codes: &[],
        tokens: "End",
        cursor: 0,
        result: Ok(0),
    },
    ScannerCase {
        codes: &[32, 9, 10, 13],
        tokens: "End",
        cursor: 4,
        result: Ok(0),
    },
    ScannerCase {
        codes: &[48],
        tokens: "Integer(0), End",
        cursor: 1,
        result: Ok(0),
    },
    ScannerCase {
        codes: &[49, 50, 32, 43, 32, 51],
        tokens: "Integer(12), Plus, Integer(3), End",
        cursor: 6,
        result: Ok(115),
    },
    ScannerCase {
        codes: &[46, 32, 43, 57, 57],
        tokens: "Invalid(46), Plus, Integer(99), End",
        cursor: 5,
        result: Ok(153),
    },
    ScannerCase {
        codes: &[43, 43],
        tokens: "Plus, Plus, End",
        cursor: 2,
        result: Ok(200),
    },
    ScannerCase {
        codes: &[49, 120, 50],
        tokens: "Integer(1), Invalid(120), Integer(2), End",
        cursor: 3,
        result: Ok(-117),
    },
    ScannerCase {
        codes: &[0, 128, -1],
        tokens: "Invalid(0), Invalid(128), Invalid(-1), End",
        cursor: 3,
        result: Ok(-127),
    },
    ScannerCase {
        codes: &[48, 48, 49, 50],
        tokens: "Integer(12), End",
        cursor: 4,
        result: Ok(12),
    },
    ScannerCase {
        codes: &[50, 49, 52, 55, 52, 56, 51, 54, 52, 55],
        tokens: "Integer(2147483647), End",
        cursor: 10,
        result: Ok(i32::MAX),
    },
    ScannerCase {
        codes: &[50, 49, 52, 55, 52, 56, 51, 54, 52, 56],
        tokens: "no token returned: checked addition overflow incorporating final 8",
        cursor: 9,
        result: Err((889, 32)),
    },
    ScannerCase {
        codes: &[57, 57, 57, 57, 57, 57, 57, 57, 57, 57],
        tokens: "no token returned: checked multiplication overflow incorporating tenth 9",
        cursor: 9,
        result: Err((884, 27)),
    },
];

#[test]
#[ignore = "run after untouched source-native successes; requires reviewed carrier gate, LLVM 19.1.7 and fresh evidence"]
fn bounded_enum_pipeline_native_scanner_overflow_and_tiny_fuel() {
    let root = evidence_group("source-native-boundaries");
    let mut artifacts = 0;
    let mut runs = 0;
    assert_eq!(SCANNER_MAIN.matches(CODES).count(), 1);
    for (number, oracle) in SCANNER_CASES.iter().enumerate() {
        let case = directory(&root, &format!("scanner-{number:02}"));
        let initializer = if oracle.codes.is_empty() {
            "let codes: [i32; 0] = [];".into()
        } else {
            format!(
                "let codes = [{}];",
                oracle
                    .codes
                    .iter()
                    .map(i32::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let main = SCANNER_MAIN.replacen(CODES, &initializer, 1);
        assert_eq!(main.replacen(&initializer, CODES, 1), SCANNER_MAIN);
        save(&case.join("oracle.txt"), format!("expected_tokens={}\nexpected_cursor{}={}\ntokens_and_cursor_are_independent_expectations_not_observed_fields=true\nexpected_consumer={:?}\n", oracle.tokens, if oracle.result.is_err() { "_before_failure" } else { "" }, oracle.cursor, oracle.result));
        with_project(&case, &main, None, true, |index, sources| {
            let expected = match oracle.result {
                Ok(value) => Expected::Value(value),
                Err((start, column)) => {
                    let source = sources
                        .files()
                        .iter()
                        .find(|source| source.path().ends_with("/scanner.ox"))
                        .unwrap();
                    assert_eq!(source.text(), SCANNER_BODY);
                    assert_eq!(source.location(start), (28, column));
                    assert_eq!(
                        &source.text()[start..start + 1],
                        if start == 889 { "+" } else { "*" }
                    );
                    Expected::Overflow(source.span(start, start + 1))
                }
            };
            let module = probe(
                &case,
                "production",
                index,
                sources,
                resolve::EnumPipelineRequest {
                    emit_llvm: true,
                    fuel: MAX_FUEL,
                },
                expected,
                [1, 4, 3, 1, 4],
            )
            .unwrap();
            assert_eq!(
                module
                    .matches(&format!("store i64 {MAX_FUEL}, ptr %fuel, align 8"))
                    .count(),
                1
            );
            let executable = compile(&case, "production", &module);
            artifacts += 1;
            run(&case, "production", &executable, None, expected, sources);
            runs += 1;
        });
    }
    for (name, original, digest, fuels) in [
        (
            "guarded-tiny",
            TINY,
            "0b77b5baf5511ac440ddd59e52e9beb5304bee4b17ca5a10c5665e8a46c6ce5d",
            &[98, 99, 100, 101, 102, 120, 121][..],
        ),
        (
            "guarded-reordered",
            REORDERED,
            "f8287e3d7b2bdd5b6f1c054b011bef53474bdec39ba38ae0dd14ef232076641a",
            &[119, 120][..],
        ),
    ] {
        let case = directory(&root, name);
        let source = format!("{original}{GUARD}");
        with_project(&case, &source, Some(digest), false, |index, sources| {
            let production = probe(
                &case,
                "production",
                index,
                sources,
                resolve::EnumPipelineRequest {
                    emit_llvm: true,
                    fuel: MAX_FUEL,
                },
                Expected::Value(7),
                [1, 2, 4, 1, 2],
            )
            .unwrap();
            let harness = argv_fuel_harness(&production);
            let harness_path = case.join("argv-harness.ll");
            save(&harness_path, &harness);
            hash_receipt(&harness_path, &case.join("argv-harness.ll.sha256"), None);
            save(&case.join("harness-scope.txt"), "Instrumented argv wrapper plus initial fuel store only; exact reverse rewrite equals the separately retained production module. Uncalled guard is source-only and does not change executed schedule.\n");
            let executable = compile(&case, "argv-harness", &harness);
            artifacts += 1;
            for &fuel in fuels {
                let expected = if fuel == *fuels.last().unwrap() {
                    Expected::Value(7)
                } else if fuel <= 102 {
                    Expected::Fuel(source_span(
                        sources,
                        "main.ox",
                        "match x{E::N=>{return 0;},E::V(v)=>{return v;},}",
                    ))
                } else {
                    Expected::Fuel(source_span(
                        sources,
                        "main.ox",
                        "return take(relay(E::V(7)));",
                    ))
                };
                let label = format!("fuel-{fuel}");
                assert!(probe(
                    &case,
                    &label,
                    index,
                    sources,
                    resolve::EnumPipelineRequest {
                        emit_llvm: false,
                        fuel
                    },
                    expected,
                    [1, 2, 4, 1, 2]
                )
                .is_none());
                run(&case, &label, &executable, Some(fuel), expected, sources);
                runs += 1;
            }
        });
    }
    assert_eq!((artifacts, runs), (14, 21));
    println!("ENUM_SOURCE_NATIVE_BOUNDARIES artifacts={artifacts} runs={runs}");
    save(&root.join("completed.txt"), "artifacts=14 runs=21\nTwelve frozen scanner inputs, exact addition/multiplication overflow origins, and guarded tiny finite fuel boundaries agree between fresh source reference and source-free ELF.\n");
}
