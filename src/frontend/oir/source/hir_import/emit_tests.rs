//! Private LLVM-text qualification through genuine retained source ownership.
//! No LLVM tool, executable, alternate witness or repaired observation is used.
use super::{candidate, leaf, SourceOwner};
use crate::frontend::{
    declaration_index::IndexLimits,
    diagnostic::Diagnostic,
    lexer,
    oir::{native::private_emit, owned, source::check_source},
    parser,
    project::budget::Allocator,
    source::{SourceMap, SourceView},
};

const RICH_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-source.txt"
));
const RICH_WIRE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-success.bin"
));

macro_rules! capture {
    ($name:literal) => {
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/",
                $name,
                "-source.txt"
            )),
            &include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/",
                $name,
                "-success.bin"
            ))[..],
        )
    };
}

fn assert_output_facts(output: &leaf::EmitOutput, wire: &[u8]) {
    let artifact = &output.artifact;
    let verified = &artifact.verified;
    assert!(verified.candidate.equal);
    assert!(verified.runtime.is_none());
    assert_eq!(verified.typed_cells, usize::from(wire[8]));
    assert_eq!(artifact.bytes, artifact.text.len());
    assert_eq!(artifact.capacity, artifact.text.capacity());
    assert_eq!(artifact.bytes, artifact.capacity);
    assert_eq!(
        (artifact.connection_work, artifact.formula_work),
        (32_768, 4_096)
    );
    assert_eq!(
        output.total_work,
        output.source_work
            + output.canonical_work
            + verified.candidate.charged_work
            + verified.pass_work
            + verified.typed_work
            + verified.entry_work
            + artifact.connection_work
            + artifact.scan_work
            + artifact.formula_work
            + artifact.body_work
    );
}

fn original_text_parity(text: &str, wire: &[u8]) -> leaf::EmitOutput {
    let (output, ordinary) = {
        let mut sources = SourceMap::new();
        let id = sources.add("emit-text.ox".into(), text.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        let output = leaf::emit(
            owner,
            text.as_bytes(),
            wire,
            &mut Allocator::default(),
            IndexLimits::default(),
        )
        .unwrap();
        let ordinary = check_source(source, &ast, &sources)
            .unwrap()
            .native_module()
            .unwrap();
        (output, ordinary)
    };
    // Both source backing and ordinary checked owner have ended their scopes.
    assert_eq!(output.artifact.text, ordinary);
    assert_output_facts(&output, wire);
    output
}

fn original_native_rejection(text: &str, wire: &[u8]) -> Box<Diagnostic> {
    let mut sources = SourceMap::new();
    let id = sources.add("emit-native-errors.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(32).unwrap();
    let result = leaf::emit(
        owner,
        text.as_bytes(),
        wire,
        &mut allocator,
        IndexLimits::default(),
    );
    let Err(leaf::VerifyRejected::Terminal(candidate::VerifyRejected::Native(
        private_emit::Failure::Diagnostic(actual),
    ))) = result
    else {
        panic!("adequate private resources must preserve native rejection: {result:?}");
    };
    let expected = check_source(source, &ast, &sources)
        .unwrap()
        .native_module()
        .unwrap_err();
    assert_eq!(actual.code, expected.code);
    assert_eq!(actual.stage, expected.stage);
    assert_eq!(actual.message, expected.message);
    assert_eq!(actual.primary, expected.primary);
    assert_eq!(actual.secondary, expected.secondary);
    assert_eq!(actual.notes, expected.notes);
    assert_eq!(
        actual.render_human(&sources),
        expected.render_human(&sources)
    );
    assert_eq!(actual.render_json(&sources), expected.render_json(&sources));
    assert!(allocator.trace.iter().all(|event| event.success));
    assert!(!allocator
        .trace
        .iter()
        .any(|event| event.kind == "private LLVM text"));
    assert!(!allocator.observer_trace_overflow);
    // Source/AST, both prior diagnostics and the prepared trace are baseline.
    // A complete failed import drops its returned diagnostic in this interval.
    let mut cleanup_allocator = Allocator::default();
    cleanup_allocator.observer_trace_bound(32).unwrap();
    let trace_capacity = cleanup_allocator.trace.capacity();
    let mut same = false;
    let cleanup = owned::hir_import_measure_allocations(|| {
        let result = leaf::emit(
            owner,
            text.as_bytes(),
            wire,
            &mut cleanup_allocator,
            IndexLimits::default(),
        );
        same = matches!(&result,
            Err(leaf::VerifyRejected::Terminal(candidate::VerifyRejected::Native(private_emit::Failure::Diagnostic(error))))
                if error.code == actual.code && error.stage == actual.stage
                    && error.message == actual.message && error.primary == actual.primary
                    && error.secondary == actual.secondary && error.notes == actual.notes);
        drop(result);
    });
    assert!(same);
    assert_eq!(cleanup.2, 0);
    assert_eq!(cleanup_allocator.attempts, allocator.attempts);
    assert_eq!(cleanup_allocator.trace.capacity(), trace_capacity);
    assert!(!cleanup_allocator.observer_trace_overflow);
    assert!(cleanup_allocator
        .trace
        .iter()
        .all(|row| row.kind != "private LLVM text"));
    assert!(owned::hir_import_allocation_observers_idle());
    println!(
        "HIR_IMPORT_EMIT_NATIVE_ERROR_DROP code={} source_bytes={} allocation={cleanup:?}",
        actual.code,
        text.len()
    );
    // Rendering was compared while the genuine source map still existed.
    // Only the authentic owned diagnostic leaves this helper.
    actual
}

#[test]
fn checked_hir_import_emit_rich_smoke_matches_source_after_backing_drop() {
    let (output, ordinary, allocation) = {
        let mut sources = SourceMap::new();
        let id = sources.add("main.ox".into(), RICH_SOURCE.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(32).unwrap();
        let trace_capacity = allocator.trace.capacity();
        // Genuine source/AST, committed capture and prepared reserve trace are
        // baseline. Only the one LLVM String should remain from this interval.
        let mut result = None;
        let allocation = owned::hir_import_measure_allocations(|| {
            result = Some(leaf::emit(
                owner,
                RICH_SOURCE.as_bytes(),
                RICH_WIRE,
                &mut allocator,
                IndexLimits::default(),
            ));
        });
        let output = result.unwrap().unwrap();
        let artifact = &output.artifact;
        let candidate = artifact.verified.candidate.allocation;
        assert_eq!(candidate.reserves, 16);
        assert_eq!(allocator.attempts, candidate.reserves + 1);
        assert_eq!(allocator.trace.len(), allocator.attempts);
        assert!(allocator.trace.iter().all(|event| event.success));
        assert_eq!(allocator.trace.capacity(), trace_capacity);
        assert!(!allocator.observer_trace_overflow);
        assert_eq!(
            allocator
                .trace
                .iter()
                .filter(|event| event.kind == "private LLVM text")
                .count(),
            1
        );
        let final_request = allocator.trace.last().unwrap();
        assert_eq!(final_request.kind, "private LLVM text");
        assert_eq!(final_request.element_bytes, 1);
        assert_eq!(final_request.length, artifact.bytes);
        assert_eq!(artifact.capacity, final_request.length);
        assert_eq!(allocation.2, isize::try_from(artifact.capacity).unwrap());
        assert!(owned::hir_import_allocation_observers_idle());
        // The unchanged ordinary source facade constructs its own checked
        // program. It is dropped here, before the backing scope ends below.
        let ordinary = {
            let checked = check_source(source, &ast, &sources).unwrap();
            checked.native_module().unwrap()
        };
        (output, ordinary, allocation)
    };
    // Actual SourceMap/path/text/AST and both compiler pipelines have dropped.
    // Dropping the Copy SourceOwner adapter alone would not establish this.
    let artifact = &output.artifact;
    let verified = &artifact.verified;
    assert_eq!(artifact.text, ordinary);
    assert!(artifact.text.contains("define i32 @main"));
    assert_eq!(artifact.bytes, artifact.text.len());
    assert_eq!(artifact.capacity, artifact.text.capacity());
    assert_eq!(artifact.bytes, artifact.capacity);
    assert_eq!(artifact.bytes, 14_325);
    assert!(verified.candidate.equal);
    assert_eq!(verified.functions, 2);
    assert_eq!(verified.typed_cells, 29);
    assert!(verified.runtime.is_none());
    assert_eq!(verified.entry_work, 33_792);
    assert_eq!(
        (
            artifact.connection_work,
            artifact.scan_work,
            artifact.formula_work,
            artifact.body_work
        ),
        (32_768, 8_064, 4_096, 8_713_600)
    );
    assert_eq!(
        output.total_work,
        output.source_work
            + output.canonical_work
            + verified.candidate.charged_work
            + verified.pass_work
            + verified.typed_work
            + verified.entry_work
            + artifact.connection_work
            + artifact.scan_work
            + artifact.formula_work
            + artifact.body_work
    );
    assert_eq!(output.total_work, 10_069_187);
    println!("HIR_IMPORT_EMIT_SMOKE original_after_backing_drop text={} capacity={} candidate_reserves={} final_reserves=1 source_work={} canonical_work={} connection_work={} scan_work={} formula_work={} body_work={} total_work={} allocations={allocation:?}",
        artifact.bytes, artifact.capacity, verified.candidate.allocation.reserves,
        output.source_work, output.canonical_work, artifact.connection_work,
        artifact.scan_work, artifact.formula_work, artifact.body_work, output.total_work);
}

#[test]
fn checked_hir_import_emit_genuine_scalar_libraries_preserve_native_missing_main() {
    for (text, wire) in [
        capture!("scalar-arithmetic"),
        capture!("scalar-boolean"),
        capture!("scalar-comparison"),
        capture!("scalar-unit"),
        capture!("scalar-loop"),
        capture!("scalar-assignment"),
    ] {
        let error = original_native_rejection(text, wire);
        assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
        assert_eq!(
            error.message,
            "native compile requires a declared zero-argument main"
        );
        assert_eq!(error.primary, None);
    }
}

#[test]
fn checked_hir_import_emit_synthetic_entry_and_recursion_match_native_errors() {
    use super::run_execution_tests::hand_authored_frame;

    // Reuse the existing explicit Run control rows and encoder. These are
    // synthetic observations, not producer captures or compiler-generated facts.
    let empty = original_native_rejection("", &hand_authored_frame("", &[]));
    assert_eq!(empty.primary, None);
    assert_eq!(
        empty.message,
        "native compile requires a declared zero-argument main"
    );

    let arity = "fn main(x:i32)->i32{return x;}";
    let arity_wire = hand_authored_frame(
        arity,
        &[
            ([1, 3, 7, 0, 0, 2, 4, 5], 1, 2),
            ([2, 8, 9, 0, 3, 0, 0, 0], 1, 2),
            ([3, 10, 13, 0, 7, 0, 0, 0], 2, 0),
            ([3, 16, 19, 0, 10, 0, 0, 0], 2, 0),
            ([5, 19, 30, 0, 16, 6, 0, 0], 0, 2),
            ([10, 20, 29, 0, 7, 0, 0, 0], 0, 0),
            ([19, 27, 28, 0, 14, 0, 0, 1], 2, 2),
        ],
    );
    let arity = original_native_rejection(arity, &arity_wire);
    assert_eq!((arity.code, arity.stage), ("E0700", "native-admission"));
    assert_eq!(arity.message, "native main must have no parameters");
    assert_eq!(
        arity.primary.map(|span| (span.start, span.end)),
        Some((3, 7))
    );

    let recursive = "fn main()->i32{return main();}";
    let recursive_wire = hand_authored_frame(
        recursive,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 2),
            ([3, 11, 14, 0, 7, 0, 0, 0], 2, 0),
            ([5, 14, 30, 0, 15, 4, 0, 0], 0, 2),
            ([10, 15, 29, 0, 5, 0, 0, 0], 0, 0),
            ([20, 22, 28, 0, 11, 0, 0, 1], 1, 2),
        ],
    );
    let recursive = original_native_rejection(recursive, &recursive_wire);
    assert_eq!(
        (recursive.code, recursive.stage),
        ("E0700", "native-admission")
    );
    assert_eq!(recursive.message, "native preview does not support recursive call graphs, including unused functions and unchosen branches");
    assert_eq!(
        recursive.primary.map(|span| (span.start, span.end)),
        Some((3, 7))
    );
}

fn arithmetic_frames() -> [(&'static str, [u8; super::SUCCESS_BYTES]); 2] {
    use super::run_execution_tests::hand_authored_frame;

    // Reuse these explicit synthetic controls for inert text checks and the
    // separately ignored native gate; no compiler-derived wire fact generator.
    let overflow = "fn main()->i32{return 2147483647+1;}";
    let overflow_wire = hand_authored_frame(
        overflow,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 2),
            ([3, 11, 14, 0, 7, 0, 0, 0], 2, 0),
            ([5, 14, 36, 0, 15, 4, 0, 0], 0, 2),
            ([10, 15, 35, 0, 6, 0, 0, 0], 0, 0),
            ([15, 22, 32, 0, 11, 0, 0, 1], i32::MAX, 2),
            ([24, 22, 34, 0, 5, 7, 12, 2], 0, 2),
            ([15, 33, 34, 0, 13, 0, 0, 1], 1, 2),
        ],
    );
    let division = "fn main()->i32{return 1/0;}";
    let division_wire = hand_authored_frame(
        division,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 2),
            ([3, 11, 14, 0, 7, 0, 0, 0], 2, 0),
            ([5, 14, 27, 0, 15, 4, 0, 0], 0, 2),
            ([10, 15, 26, 0, 6, 0, 0, 0], 0, 0),
            ([15, 22, 23, 0, 11, 0, 0, 1], 1, 2),
            ([27, 22, 25, 0, 5, 7, 12, 2], 0, 2),
            ([15, 24, 25, 0, 13, 0, 0, 1], 0, 2),
        ],
    );
    [(overflow, overflow_wire), (division, division_wire)]
}

#[test]
fn checked_hir_import_emit_synthetic_arithmetic_is_inert_llvm_text() {
    let [(overflow, overflow_wire), (division, division_wire)] = arithmetic_frames();
    let overflow = original_text_parity(overflow, &overflow_wire);
    assert!(overflow
        .artifact
        .text
        .contains("@llvm.sadd.with.overflow.i32"));
    let division = original_text_parity(division, &division_wire);
    assert!(division.artifact.text.contains("sdiv i32"));
}

#[cfg(target_os = "linux")]
#[test]
fn checked_hir_import_emit_genuine_rich_and_public_project_text_survives_backing_drop() {
    use crate::frontend::{declaration_index::WorkMeter, oir::project};

    for (text, wire) in [(RICH_SOURCE, RICH_WIRE), capture!("public")] {
        let (output, ordinary) = {
            let sources = super::tests::project(text);
            let owner = SourceOwner::project(&sources);
            assert_eq!(owner.count(), 1);
            let output = leaf::emit(
                owner,
                text.as_bytes(),
                wire,
                &mut Allocator::default(),
                IndexLimits::default(),
            )
            .unwrap();
            let ordinary = project::check_project_executable(
                &sources,
                IndexLimits::default(),
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap()
            .native_module()
            .unwrap();
            (output, ordinary)
        };
        // The actual ProjectSources owner, including its source map and parsed
        // AST backing, is gone; both independent text buffers remain valid.
        assert_eq!(output.artifact.text, ordinary);
        assert_output_facts(&output, wire);
        assert_eq!(output.artifact.verified.functions, 2);
        println!("HIR_IMPORT_EMIT_PROJECT after_backing_drop source_bytes={} text_bytes={} total_work={}",
            text.len(), output.artifact.bytes, output.total_work);
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn checked_hir_import_emit_project_child_policy_stays_e0005_on_unqualified_hosts() {
    use crate::frontend::project::{ProjectLimits, ProjectSources};

    // Root-only loading intentionally has no extra host check. Exercise the
    // actual child-module boundary; do not claim the root-only fixture rejects.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("oxid-emit-host-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let file = directory.join("main.ox");
    std::fs::write(&file, "pub mod child;").unwrap();
    std::fs::write(directory.join("child.ox"), "@ child must not be parsed").unwrap();
    let failure =
        ProjectSources::load_typed(file.to_str().unwrap(), ProjectLimits::default()).unwrap_err();
    assert_eq!(failure.diagnostics.len(), 1);
    let error = &failure.diagnostics[0];
    assert_eq!(
        (error.code, error.stage, error.message.as_str()),
        (
            "E0005",
            "source",
            "module source policy is not qualified on this host"
        )
    );
    assert_eq!(failure.sources.text(error.primary.unwrap()), "child");
    assert!(error.secondary.is_empty() && error.notes.is_empty());
    assert_eq!(failure.usage.probes, 0);
}

#[test]
fn checked_hir_import_emit_synthetic_bool_and_unit_text_match_result_abi() {
    use super::run_execution_tests::hand_authored_frame;
    // Reuse the existing explicit Run controls; no compiler-derived wire facts.
    let boolean = "fn main()->bool{return true;}";
    let boolean_wire = hand_authored_frame(
        boolean,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 1),
            ([3, 11, 15, 0, 7, 0, 0, 0], 1, 0),
            ([5, 15, 29, 0, 13, 4, 0, 0], 0, 2),
            ([10, 16, 28, 0, 5, 0, 0, 0], 0, 0),
            ([17, 23, 27, 0, 0, 0, 0, 1], 0, 1),
        ],
    );
    let boolean_output = original_text_parity(boolean, &boolean_wire);
    assert!(boolean_output.artifact.text.contains("define i32 @main"));
    let unit = "fn main()->(){return;}";
    let unit_wire = hand_authored_frame(
        unit,
        &[
            ([1, 3, 7, 0, 0, 0, 2, 3], 1, 3),
            ([4, 11, 13, 0, 0, 0, 0, 0], 3, 0),
            ([5, 13, 22, 0, 12, 4, 0, 0], 0, 2),
            ([10, 14, 21, 0, 0, 0, 0, 0], 0, 0),
        ],
    );
    let unit_output = original_text_parity(unit, &unit_wire);
    assert!(unit_output.artifact.text.contains("define i32 @main"));
}

#[path = "emit_native_tests.rs"]
mod native_tests;
