use super::*;
use crate::frontend::{
    lexer, parser,
    project::budget::Allocator,
    source::{SourceMap, SourceView},
};

#[allow(clippy::result_large_err)]
fn verify_original(
    text: &str,
    wire: &[u8],
    allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<leaf::VerifyFacts, leaf::VerifyRejected> {
    let mut sources = SourceMap::new();
    let id = sources.add("verify-import.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    leaf::verify(owner, text.as_bytes(), wire, allocator, limits)
}

#[test]
fn checked_hir_import_verify_genuine_rich_fixed_facts() {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    let wire = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-success.bin"
    ));
    let mut allocator = Allocator::default();
    let facts = verify_original(text, wire, &mut allocator, IndexLimits::default()).unwrap();
    assert_eq!(facts.verified.functions, 2);
    assert_eq!(facts.verified.typed_cells, usize::from(wire[8]));
    assert_eq!(allocator.attempts, 16);
    assert!(facts.verified.candidate.equal);
    assert_eq!(
        facts.total_work,
        facts.source_work
            + facts.canonical_work
            + facts.verified.candidate.charged_work
            + facts.verified.pass_work
            + facts.verified.typed_work
    );
    println!("HIR_IMPORT_VERIFY_SUCCESS {facts:?}");
}
