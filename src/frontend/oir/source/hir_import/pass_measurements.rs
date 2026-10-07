//! Ordinary canonical-pass observations only. The importer Verify gate stays
//! denied; these measurements do not create or execute imported typed authority.
use super::*;
use crate::frontend::{
    lexer,
    oir::{lower, source::association, verify},
    parser,
    source::{SourceMap, SourceView},
    typeck,
};

fn observe_case(label: &str, owner: SourceOwner<'_>, sources: &SourceMap) {
    assert!(!candidate::VERIFY_ADMITTED);
    let root = owner.ast(ModuleId(0)).unwrap();
    let canonical = hir::resolve_sources(owner).unwrap();
    let hir_payload = StoragePlan::describe(&canonical)
        .unwrap()
        .canonical_payload_bytes;
    let type_guard = typeck::measurement::begin();
    let typed = typeck::check(canonical).unwrap();
    let type_scratch = type_guard.finish();
    let typed_payload = typeck::measurement::retained_payload(&typed).unwrap();
    let lower_guard = lower::measurement::begin();
    let raw = lower::lower(&typed).unwrap();
    let lower_scratch = lower_guard.finish();
    let raw_payload = lower::measurement::retained_payload(&raw).unwrap();
    // Source/AST is the already-owned baseline. These conservative successful
    // phase envelopes add retained owners and actual per-family maxima; they
    // are not exact heap peaks, inherited admission caps or failure envelopes.
    let type_phase = hir_payload
        .checked_add(typed_payload.bytes)
        .unwrap()
        .checked_add(type_scratch.body_payload_envelope_bytes)
        .unwrap();
    let lower_phase = hir_payload
        .checked_add(typed_payload.bytes)
        .unwrap()
        .checked_add(raw_payload.bytes)
        .unwrap()
        .checked_add(lower_scratch.scratch_payload_envelope_bytes)
        .unwrap();
    drop(typed);
    let associated =
        association::scalar(&raw, sources, association::Declarations::Original(root)).unwrap();
    let verify_guard = verify::measurement::begin();
    let verified = verify::verify(raw, sources).unwrap();
    let verify_scratch = verify_guard.finish();
    let verify_phase = raw_payload
        .bytes
        .checked_add(verify_scratch.per_function_capacity_max_bytes)
        .unwrap();
    assert_eq!(verified.function_count(), root.functions.len());
    assert_eq!(verify_scratch.successful_functions, root.functions.len());
    assert_eq!(verify_scratch.excluded_failed_functions, 0);
    assert_eq!(associated.count, associated.validation);
    if !root.functions.is_empty() {
        assert!(type_scratch.frame_bytes > 0);
    }
    println!("HIR_INHERITED {label} hir={hir_payload} typed={typed_payload:?} type_scratch={type_scratch:?} raw={raw_payload:?} lower_scratch={lower_scratch:?} verifier={verify_scratch:?} phases={type_phase}/{lower_phase}/{verify_phase}");
    drop(verified);
}

#[test]
fn checked_hir_import_ordinary_pass_layouts_and_capacity_envelopes() {
    println!(
        "HIR_INHERITED_LAYOUT type={:?} lower={:?} verifier={:?}",
        typeck::measurement::layout(),
        lower::measurement::layout(),
        verify::measurement::layout()
    );
    for (label, text) in [
        ("empty", ""),
        (
            "rich",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/rich-source.txt"
            )),
        ),
        (
            "booleans",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/scalar-boolean-source.txt"
            )),
        ),
        (
            "loops",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/scalar-loop-source.txt"
            )),
        ),
        (
            "recursive-library",
            "fn f(n:i32)->i32{if n==0{return 0;}else{return f(n-1);}}",
        ),
    ] {
        assert!(text.len() <= MAX_ROWS && text.is_ascii());
        let mut sources = SourceMap::new();
        let id = sources.add(format!("{label}.ox"), text.to_owned());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        observe_case(label, owner, &sources);
    }
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/public-source.txt"
    ));
    let project = super::tests::project(text);
    observe_case(
        "public-project",
        SourceOwner::project(&project),
        project.sources(),
    );
    assert!(!candidate::VERIFY_ADMITTED);
}
