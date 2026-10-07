//! Private LLVM-text qualification through genuine retained source ownership.
//! No LLVM tool, executable, alternate witness or repaired observation is used.
use super::{leaf, SourceOwner};
use crate::frontend::{
    declaration_index::IndexLimits,
    lexer,
    oir::{owned, source::check_source},
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
