// NOT EXECUTED. Integration specification for an in-crate test at candidate review.
// This deliberately calls scalar HIR directly. The public selector routes the
// unknown annotations below into owned resolution, which has another schedule.
#[test]
fn compatibility_scalar_duplicate_signature_interleaving() {
    use crate::frontend::{hir, lexer, parser, source::SourceMap};
    let text = concat!(
        "fn same(a: Missing) -> i32 { return 1; }\n",
        "fn same(a: AlsoMissing) -> i32 { return 2; }\n",
    );
    let mut sources = SourceMap::new();
    let id = sources.add("scalar-schedule.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    assert!(ast.uses_owned_syntax(source)); // Not public scalar evidence.
    let errors = hir::resolve(source, &ast).unwrap_err();
    assert_eq!(
        errors.iter().map(|e| (e.code, e.stage)).collect::<Vec<_>>(),
        vec![("E0202", "resolve"), ("E0201", "resolve"), ("E0202", "resolve")],
    );
    let missing = text.find("Missing").unwrap();
    let first_same = text.find("same").unwrap();
    let second_same = text.rfind("same").unwrap();
    let also_missing = text.find("AlsoMissing").unwrap();
    assert_eq!(errors[0].primary, Some(source.span(missing, missing + 7)));
    assert_eq!(errors[1].primary, Some(source.span(second_same, second_same + 4)));
    assert_eq!(errors[1].secondary.len(), 1);
    assert_eq!(errors[1].secondary[0].0, source.span(first_same, first_same + 4));
    assert_eq!(errors[2].primary, Some(source.span(also_missing, also_missing + 11)));
}
