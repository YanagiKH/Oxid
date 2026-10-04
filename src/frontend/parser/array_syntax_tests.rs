//! Local structural/resource checks; independent source-authority receipts are separate.
use super::*;
use crate::frontend::{
    declaration_index::{collect_originals, IndexLimits, SourceOwner, WorkMeter},
    lexer,
    project::ModuleId,
    source::{SourceFileId, SourceMap, SourceView},
};

fn source(text: &str) -> SourceMap {
    let mut map = SourceMap::new();
    map.add("arrays.ox".into(), text.into());
    map
}
fn parsed(map: &SourceMap, allocator: &mut Allocator) -> Result<(Program, usize), Vec<Diagnostic>> {
    let file = map.get(SourceFileId(0));
    parse_counted_with_arrays(
        file,
        lexer::lex(file).unwrap(),
        SourceMode::OwnedCandidate,
        MAX_NODES,
        allocator,
        ArraySyntaxPolicy::Candidate,
    )
}
fn candidate(map: &SourceMap) -> Program {
    parsed(map, &mut Allocator::default()).unwrap().0
}

#[test]
fn unit3a_real_ast_retains_target_once_and_original_origins() {
    let map = source("// 雪\nfn f(p:&mut [i32; 2])->[();0]{let mut a:[i32;2]=[5,7,];a[(0)]=a[1];a.len();return [];}");
    let file = map.get(SourceFileId(0));
    let p = candidate(&map);
    assert!(p.belongs_to(file));
    assert!(p.validate_spans_and_ids(|span| file.try_text(span).is_some()));
    assert!(p.uses_owned_syntax(file));
    assert!(!p.uses_project_syntax());
    let function = &p.functions[0];
    assert!(matches!(
        function.params[0].ty.kind,
        TypeSyntaxKind::ArrayReference {
            mutable: true,
            array: FixedArraySyntax {
                element: ScalarTypeSyntax::I32,
                length: 2
            }
        }
    ));
    assert!(matches!(
        function.result.kind,
        TypeSyntaxKind::Array(FixedArraySyntax {
            element: ScalarTypeSyntax::Unit,
            length: 0
        })
    ));
    let StmtKind::IndexAssign {
        target,
        operator_span,
        value,
    } = function.blocks[0].body[1].kind
    else {
        panic!("direct indexed store")
    };
    assert_eq!(file.text_at(operator_span), "=");
    assert_eq!(file.text_at(p.expressions[target.0].span), "a[(0)]");
    assert_eq!(file.text_at(p.expressions[value.0].span), "a[1]");
    assert_eq!(
        p.expressions
            .iter()
            .filter(|e| matches!(e.kind, ExprKind::IndexRead { .. }))
            .count(),
        2
    );
    let ExprKind::IndexRead { base, index } = p.expressions[target.0].kind else {
        unreachable!()
    };
    assert_eq!(file.text_at(base), "a");
    assert_eq!(file.text_at(p.expressions[index.0].span), "(0)");
    assert_eq!(
        p.expressions
            .iter()
            .filter(|e| matches!(e.kind, ExprKind::Name(_)))
            .count(),
        0
    );
    let other = source(file.text());
    assert!(!p.belongs_to(other.get(SourceFileId(0))));
}

#[test]
fn unit3a_structural_queries_are_checked_without_execution() {
    use crate::frontend::{
        declaration_index::TypeContext,
        hir::Ty,
        oir::owned_types::{
            AggregateTy, BorrowKind, BorrowedTy, FixedArrayTy, ParameterTy, ValueTy,
        },
    };
    let map = source("fn f(a:[bool;0],b:[i32;0],c:[();0],d:&[i32;0])->[i32;0000]{}");
    let file = map.get(SourceFileId(0));
    let ast = candidate(&map);
    let work = WorkMeter::default();
    let mut alloc = Allocator::default();
    let owner = SourceOwner::original(file, &ast, SourceView::Map(&map)).unwrap();
    let facts = collect_originals(owner, IndexLimits::default(), &work, &mut alloc).unwrap();
    let index = facts.finish(&work, &mut alloc).unwrap();
    let mut query = index.query(&work);
    for (param, element) in ast.functions[0].params[..3]
        .iter()
        .zip([Ty::Bool, Ty::I32, Ty::Unit])
    {
        assert_eq!(
            query
                .value_type(ModuleId(0), param.ty, TypeContext::Value)
                .unwrap(),
            ValueTy::Owned(AggregateTy::FixedArray(
                FixedArrayTy::check(element, 0).unwrap()
            ))
        );
    }
    let param = ast.functions[0].params[3].ty;
    let TypeSyntaxKind::ArrayReference { mutable, array } = param.kind else {
        unreachable!()
    };
    assert!(!mutable);
    assert_eq!(
        query.parameter_type(ModuleId(0), param).unwrap(),
        ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::FixedArray(
                FixedArrayTy::check(Ty::I32, 0).unwrap()
            )),
            kind: BorrowKind::Shared,
        }
    );
    assert_eq!(
        query.array_type(ModuleId(0), array, param.span).unwrap(),
        FixedArrayTy::check(Ty::I32, 0).unwrap()
    );
    assert_eq!(
        query
            .value_type(ModuleId(0), ast.functions[0].result, TypeContext::Value)
            .unwrap(),
        query
            .value_type(
                ModuleId(0),
                ast.functions[0].params[1].ty,
                TypeContext::Value
            )
            .unwrap()
    );
    let forged = FixedArraySyntax {
        element: ScalarTypeSyntax::I32,
        length: 1025,
    };
    assert!(query.array_type(ModuleId(0), forged, param.span).is_err());
    let errors = crate::frontend::hir::resolve(file, &ast).unwrap_err();
    assert_eq!((errors[0].code, errors[0].stage), ("E0500", "resolve"));
    assert_eq!(
        errors[0].message,
        "array source execution is unavailable in this dormant syntax checkpoint"
    );
}

#[test]
fn unit3a_literal_limits_reservations_and_tree_height_are_bounded() {
    for count in [0, 1, 256, 257, 1024] {
        let values = "0,".repeat(count);
        let map = source(&format!("fn f()->(){{[{values}];}}"));
        let mut alloc = Allocator::default();
        let (p, nodes) = parsed(&map, &mut alloc).unwrap();
        assert_eq!(nodes, count + 3); // function, statement, literal, and scalar children
        let ExprKind::ArrayLiteral { elements } = &p.expressions.last().unwrap().kind else {
            unreachable!()
        };
        assert_eq!(elements.len(), count);
        assert_eq!(alloc.trace.len(), count);
        for (i, event) in alloc.trace.iter().enumerate() {
            assert_eq!(
                (event.kind, event.length, event.element_bytes, event.success),
                (
                    "array literal elements",
                    i + 1,
                    std::mem::size_of::<ExprId>(),
                    true
                )
            );
        }
    }
    let map = source(&format!("fn f()->(){{[{}unknown()];}}", "0,".repeat(1024)));
    let mut alloc = Allocator::default();
    let d = parsed(&map, &mut alloc).unwrap_err().remove(0);
    assert_eq!(
        (d.code, d.stage, d.message.as_str()),
        ("E0400", "parse", "array element limit exceeded (1024)")
    );
    assert_eq!(map.text(d.primary.unwrap()), "unknown");
    assert_eq!(alloc.trace.len(), 1024);
    for depth in [63, 64] {
        let map = source(&format!(
            "fn f()->(){{{}0{};}}",
            "[".repeat(depth),
            "]".repeat(depth)
        ));
        let result = parsed(&map, &mut Allocator::default());
        assert_eq!(result.is_ok(), depth == 63);
    }
    // A deep index child has the same height accounting as a literal child.
    for depth in [62, 63] {
        let map = source(&format!(
            "fn f()->(){{a[{}0{}];}}",
            "(".repeat(depth),
            ")".repeat(depth)
        ));
        assert_eq!(parsed(&map, &mut Allocator::default()).is_ok(), depth == 62);
    }
}

#[test]
fn unit3a_array_lengths_are_bounded_without_placeholder_nodes() {
    for (spelling, expected) in [("0".repeat(65536), 0), ("0001024".into(), 1024)] {
        let map = source(&format!("fn f(a:[bool;{spelling}])->(){{}}"));
        let mut alloc = Allocator::default();
        let (p, nodes) = parsed(&map, &mut alloc).unwrap();
        assert_eq!(nodes, 2); // function and parameter only
        assert!(p.expressions.is_empty());
        assert!(alloc.trace.is_empty());
        let TypeSyntaxKind::Array(array) = p.functions[0].params[0].ty.kind else {
            unreachable!()
        };
        assert_eq!(usize::from(array.length), expected);
    }
}

#[test]
fn unit3a_reserve_failure_drops_candidate_without_partial_ast() {
    let map = source("fn f()->(){[1,2,3];}");
    for fail_at in 1..=3 {
        let mut alloc = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        let d = parsed(&map, &mut alloc).unwrap_err().remove(0);
        assert_eq!(
            (d.code, d.stage, d.message.as_str()),
            ("E0400", "parse", "array syntax allocation failed")
        );
        assert_eq!(alloc.trace.len(), fail_at);
        assert!(!alloc.trace.last().unwrap().success);
    }
}

#[test]
fn unit3a_flat_children_preserve_tree_height_limit() {
    for terms in [63, 64] {
        let sum = std::iter::repeat_n("0", terms)
            .collect::<Vec<_>>()
            .join("+");
        for wrapper in [format!("[{sum}]"), format!("a[{sum}]")] {
            let map = source(&format!("fn f()->(){{{wrapper};}}"));
            let result = parsed(&map, &mut Allocator::default());
            if terms == 63 {
                assert!(result.is_ok(), "{wrapper}");
            } else {
                let error = result.unwrap_err().remove(0);
                assert_eq!((error.code, error.stage), ("E0400", "parse"));
                assert_eq!(error.message, "expression nesting limit exceeded");
                assert_eq!(map.text(error.primary.unwrap()), wrapper);
            }
        }
    }
}

#[test]
fn unit3a_allocator_request_counter_overflow_precedes_reservation() {
    let map = source("fn f()->(){[1];}");
    let mut allocator = Allocator {
        attempts: usize::MAX,
        ..Allocator::default()
    };
    let error = parsed(&map, &mut allocator).unwrap_err().remove(0);
    assert_eq!(
        (error.code, error.stage, error.message.as_str()),
        ("E0400", "parse", "array syntax count overflow")
    );
    assert_eq!(map.text(error.primary.unwrap()), "1");
    assert!(allocator.trace.is_empty());
    assert_eq!(allocator.attempts, usize::MAX);
}

#[test]
fn unit3a_new_ast_edges_spans_and_descriptors_fail_closed_when_corrupted() {
    let map = source("fn f()->[i32;1]{let a=[1];a[0]=1;return a;}");
    let valid = |p: &Program| p.validate_spans_and_ids(|span| map.try_text(span).is_some());
    let mut p = candidate(&map);
    p.functions[0].result.kind = TypeSyntaxKind::Array(FixedArraySyntax {
        element: ScalarTypeSyntax::I32,
        length: 1025,
    });
    assert!(!valid(&p));
    let mut p = candidate(&map);
    let n = p.expressions.len();
    let ExprKind::ArrayLiteral { elements } = &mut p.expressions[1].kind else {
        unreachable!()
    };
    elements[0] = ExprId(n);
    assert!(!valid(&p));
    let mut p = candidate(&map);
    let StmtKind::IndexAssign { target, .. } = &mut p.functions[0].blocks[0].body[1].kind else {
        unreachable!()
    };
    *target = ExprId(0); // scalar expression exists but is not a direct IndexRead
    assert!(!valid(&p));
    let mut p = candidate(&map);
    let ExprKind::IndexRead { base, .. } = &mut p.expressions[3].kind else {
        unreachable!()
    };
    base.file = SourceFileId(1);
    assert!(!valid(&p));
}

#[test]
fn unit3a_public_token_tape_and_rejections_stay_closed() {
    for text in [
        "fn f(a:[i32;1])->(){}",
        "fn f()->(){[1];}",
        "fn f()->(){a[0];}",
        "fn f()->(){a.len();}",
    ] {
        let map = source(text);
        let file = map.get(SourceFileId(0));
        let tape = lexer::lex(file).unwrap();
        assert_eq!(
            tape.iter()
                .map(|t| file.text_at(t.span))
                .collect::<String>(),
            text
        );
        for token in &tape {
            if matches!(file.text_at(token.span), "[" | "]") {
                assert_eq!(token.kind, Kind::Unsupported);
            }
        }
        for mode in [
            SourceMode::ScalarOnly,
            SourceMode::OwnedCandidate,
            SourceMode::ModuleCandidate,
            SourceMode::ProjectCandidate,
        ] {
            assert!(parse_with_mode(file, lexer::lex(file).unwrap(), mode).is_err());
        }
    }
}

#[test]
fn unit3a_nonstarter_diagnostic_is_bounded_without_changing_public_formatting() {
    use crate::frontend::owned_diagnostic::{measure_formatting, MAX_MESSAGE_BYTES};
    let token = format!("\"{}\"", "雪".repeat(21_000));
    let map = source(&format!("fn f()->(){{[1,{token}];}}"));
    let mut allocator = Allocator::default();
    let (result, metrics) = measure_formatting(|| parsed(&map, &mut allocator));
    let error = result.unwrap_err().remove(0);
    assert_eq!((error.code, error.stage), ("E0101", "parse"));
    assert_eq!(map.text(error.primary.unwrap()), token);
    assert!(error
        .message
        .starts_with("unsupported typed-preview construct `\"雪"));
    assert!(error.message.ends_with("..."));
    assert!(error.message.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(metrics.allocations, 1);
    assert!(metrics.copied_bytes <= MAX_MESSAGE_BYTES);
    assert_eq!(allocator.trace.len(), 1); // no entry reserved for the nonstarter

    let map = source(&format!("fn f()->(){{{token};}}"));
    let file = map.get(SourceFileId(0));
    let error = parse_with_mode(file, lexer::lex(file).unwrap(), SourceMode::OwnedCandidate)
        .unwrap_err()
        .remove(0);
    assert_eq!(
        error.message,
        format!("unsupported typed-preview construct `{token}`")
    );
}
