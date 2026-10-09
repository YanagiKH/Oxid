//! Independently mutated OIR checks the explicit comparison type table.
use super::*;
use crate::frontend::{ast, lexer, parser};

fn raw(text: &str) -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("comparison.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    (sources, lower::lower(&typed).unwrap())
}
fn simple() -> (SourceMap, Program) {
    raw("// 雪\r\nfn main() -> bool { return true == false; }")
}
#[test]
fn comparison_origin_and_operand_order_survive_every_representation() {
    let text = "// 雪\r\nfn main() -> bool { return - /* é */ 2147483648 <= 2147483647; }";
    let (sources, program) = raw(text);
    let source = sources.get(program.functions[0].span.file);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let expr = ast.expressions.last().unwrap();
    let ast::ExprKind::Comparison {
        op,
        left,
        right,
        operator_span,
    } = expr.kind
    else {
        panic!()
    };
    assert_eq!(op, hir::ComparisonOp::LessEqual);
    assert_eq!(&text[operator_span.start..operator_span.end], "<=");
    assert_eq!(
        &text[ast.expressions[left.0].span.start..ast.expressions[left.0].span.end],
        "- /* é */ 2147483648"
    );
    assert_eq!(
        &text[ast.expressions[right.0].span.start..ast.expressions[right.0].span.end],
        "2147483647"
    );
    let resolved = hir::resolve(source, &ast).unwrap();
    let hir::ExprKind::Comparison {
        operator_span: hir_span,
        ..
    } = resolved.functions[0].expressions.last().unwrap().kind
    else {
        panic!()
    };
    let assign = program.functions[0].blocks[0]
        .statements
        .last()
        .unwrap()
        .assignment();
    let Rvalue::CompareScalar {
        operator_span: oir_span,
        left: oir_left,
        right: oir_right,
        ..
    } = assign.value
    else {
        panic!()
    };
    assert_eq!(operator_span, hir_span);
    assert_eq!(operator_span, oir_span);
    assert_eq!(assign.span, expr.span);
    assert_eq!(oir_left.span, ast.expressions[left.0].span);
    assert_eq!(oir_right.span, ast.expressions[right.0].span);
    assert_eq!(
        verify::verify(program, &sources)
            .unwrap()
            .run(Some(hir::DefId(0))),
        Ok(Scalar::Bool(true))
    );
}
#[test]
fn verifier_enforces_the_complete_closed_comparison_type_table() {
    use hir::{ComparisonOp::*, Ty};
    for op in [Equal, NotEqual, Less, LessEqual, Greater, GreaterEqual] {
        for left_ty in [Ty::I32, Ty::Bool, Ty::Unit] {
            for right_ty in [Ty::I32, Ty::Bool, Ty::Unit] {
                let (sources, mut program) = simple();
                let f = &mut program.functions[0];
                for (i, ty) in [left_ty, right_ty].into_iter().enumerate() {
                    f.locals[i].ty = ty;
                    f.blocks[0].statements[i].assignment_mut().value = match ty {
                        Ty::I32 => Rvalue::I32(if i == 0 { -1 } else { 0 }),
                        Ty::Bool => Rvalue::Bool(i != 0),
                        Ty::Unit => Rvalue::Unit,
                        Ty::U8 => unreachable!("legacy closed type inventory"),
                    };
                }
                let Rvalue::CompareScalar { op: actual, .. } =
                    &mut f.blocks[0].statements[2].assignment_mut().value
                else {
                    panic!()
                };
                *actual = op;
                let allowed = left_ty == right_ty
                    && (left_ty == Ty::I32
                        || (left_ty == Ty::Bool && matches!(op, Equal | NotEqual)));
                let result = verify::verify(program, &sources);
                if allowed {
                    let expected = matches!(op, NotEqual | Less | LessEqual);
                    assert_eq!(
                        result.unwrap().run(Some(hir::DefId(0))),
                        Ok(Scalar::Bool(expected)),
                        "{op:?} {left_ty:?} {right_ty:?}"
                    );
                } else {
                    assert_eq!(
                        result.unwrap_err().kind,
                        FailureKind::TypeMismatch,
                        "{op:?} {left_ty:?} {right_ty:?}"
                    );
                }
            }
        }
    }
}
#[test]
fn comparison_destination_and_every_operand_id_are_checked() {
    for wrong in [hir::Ty::I32, hir::Ty::Unit] {
        let (sources, mut p) = simple();
        p.functions[0].locals[2].ty = wrong;
        assert_eq!(
            verify::verify(p, &sources).unwrap_err().kind,
            FailureKind::TypeMismatch
        );
    }
    for side in 0..3 {
        let (sources, mut p) = simple();
        let a = p.functions[0].blocks[0].statements[2].assignment_mut();
        let Rvalue::CompareScalar { left, right, .. } = &mut a.value else {
            panic!()
        };
        match side {
            0 => left.local = LocalId(usize::MAX),
            1 => right.local = LocalId(usize::MAX),
            _ => a.destination = LocalId(usize::MAX),
        }
        assert_eq!(
            verify::verify(p, &sources).unwrap_err().kind,
            FailureKind::InvalidLocal
        );
    }
}
#[test]
fn comparison_reads_need_definitions_before_both_operands_and_a_unique_destination() {
    for side in 0..2 {
        for self_read in [false, true] {
            let (sources, mut p) = simple();
            let f = &mut p.functions[0];
            let missing = if self_read {
                LocalId(2)
            } else {
                f.locals.push(f.locals[0].clone());
                LocalId(3)
            };
            let Rvalue::CompareScalar { left, right, .. } =
                &mut f.blocks[0].statements[2].assignment_mut().value
            else {
                panic!()
            };
            if side == 0 {
                left.local = missing;
            } else {
                right.local = missing;
            }
            assert_eq!(
                verify::verify(p, &sources).unwrap_err().kind,
                FailureKind::Uninitialized
            );
        }
        let (sources, mut p) = simple();
        p.functions[0].blocks[0].statements.swap(side, 2);
        assert_eq!(
            verify::verify(p, &sources).unwrap_err().kind,
            FailureKind::Uninitialized
        );
        let (sources, mut p) = simple();
        p.functions[0].blocks[0].statements[2]
            .assignment_mut()
            .destination = LocalId(side);
        assert_eq!(
            verify::verify(p, &sources).unwrap_err().kind,
            FailureKind::AlreadyInitialized
        );
    }
}
#[test]
fn comparison_cannot_read_a_call_result_from_only_one_branch() {
    for side in 0..2 {
        let (sources, mut p) = raw("fn id(x: i32) -> i32 { return x; } fn main() -> bool { if true { id(10); } return 1 < 2; }");
        let main = &mut p.functions[1];
        let destination = main
            .blocks
            .iter()
            .find_map(|b| match b.terminator.as_ref().unwrap().kind {
                TerminatorKind::Call { destination, .. } => Some(destination),
                _ => None,
            })
            .unwrap();
        let a = main
            .blocks
            .iter_mut()
            .flat_map(|b| &mut b.statements)
            .map(Statement::assignment_mut)
            .find(|a| matches!(a.value, Rvalue::CompareScalar { .. }))
            .unwrap();
        let Rvalue::CompareScalar { left, right, .. } = &mut a.value else {
            panic!()
        };
        if side == 0 {
            left.local = destination;
        } else {
            right.local = destination;
        }
        assert_eq!(
            verify::verify(p, &sources).unwrap_err().kind,
            FailureKind::Uninitialized
        );
    }
}
#[test]
fn comparison_operator_and_both_operand_origins_are_independently_validated() {
    for field in 0..3 {
        for range in [(4, 5), (usize::MAX, usize::MAX)] {
            let (sources, mut p) = simple();
            let Rvalue::CompareScalar {
                operator_span,
                left,
                right,
                ..
            } = &mut p.functions[0].blocks[0].statements[2]
                .assignment_mut()
                .value
            else {
                panic!()
            };
            let span = match field {
                0 => operator_span,
                1 => &mut left.span,
                _ => &mut right.span,
            };
            span.start = range.0;
            span.end = range.1;
            let error = verify::verify(p, &sources).unwrap_err();
            assert_eq!(error.kind, FailureKind::InvalidSpan);
            assert!(error
                .diagnostic(&sources)
                .render_json(&sources)
                .contains("\"primary\":null"));
        }
    }
}
