//! Local indexing syntax remains separate from call-only slice arguments.
use super::*;
use crate::frontend::{
    lexer,
    source::{SourceFileId, SourceMap},
};
fn parsed(text: &str) -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let file = sources.add("element-reference.ox".into(), text.into());
    let source = sources.get(file);
    let (program, _) = parse_typed_counted(
        source,
        lexer::lex(source).unwrap(),
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default(),
    )
    .unwrap();
    (sources, program)
}
#[test]
fn local_record_element_syntax_preserves_mutability_forwarding_and_one_index_child() {
    let text="fn f(values:&mut Vec<Entry>)->(){let shared=&*values[0];return;}fn main()->i32{let item=&mut values[next()];return 0;}";
    let (sources, program) = parsed(text);
    let file = sources.get(SourceFileId(0));
    let rows = program
        .expressions
        .iter()
        .filter_map(|expression| match expression.kind {
            ExprKind::BorrowElement {
                mutable,
                name,
                forwarded,
                index,
            } => Some((mutable, name, forwarded, index, expression.span)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].0, rows[0].2), (false, true));
    assert_eq!((rows[1].0, rows[1].2), (true, false));
    for (_, name, _, index, span) in rows {
        assert_eq!(file.text_at(name), "values");
        let child = program.expressions[index.0].span;
        assert!(name.end < child.start && child.end < span.end);
    }
    assert!(program.validate_spans_and_ids_counted_with_features(
        Some(file),
        |at| at.is_none_or(|at| file.try_text(at).is_some()),
        &mut None,
        StdImportPolicy::Enabled,
        &mut false,
        &mut Default::default()
    ));
}
#[test]
fn local_record_element_syntax_rejects_forward_or_external_index_ids_and_false_roots() {
    for mutation in 0..3 {
        let (sources, mut program) =
            parsed("fn main()->i32{let outside=7;let item=&values[0];return 0;}");
        let file = sources.get(SourceFileId(0));
        let position = program
            .expressions
            .iter()
            .position(|expression| matches!(expression.kind, ExprKind::BorrowElement { .. }))
            .unwrap();
        let whole = program.expressions[position].span;
        let ExprKind::BorrowElement { name, index, .. } = &mut program.expressions[position].kind
        else {
            unreachable!()
        };
        match mutation {
            0 => index.0 = position,
            1 => index.0 = 0,
            2 => *name = whole,
            _ => unreachable!(),
        }
        assert!(
            !program.validate_spans_and_ids_counted_with_features(
                Some(file),
                |at| at.is_none_or(|at| file.try_text(at).is_some()),
                &mut None,
                StdImportPolicy::Enabled,
                &mut false,
                &mut Default::default()
            ),
            "AST forgery {mutation}"
        );
    }
}
