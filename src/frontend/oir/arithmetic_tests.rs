//! Raw-IR arithmetic adversaries are independent of source typing/lowering.
use super::*;
use crate::frontend::{ast, lexer, parser};
fn raw(text: &str) -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("arithmetic.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    (sources, lower::lower(&typed).unwrap())
}
fn simple() -> (SourceMap, Program) {
    raw("// 雪\r\nfn main() -> i32 { return 1 + 2; }")
}
#[test]
fn operator_and_operand_provenance_survive_ast_hir_oir_and_execution() {
    let text = "// 雪\r\nfn main() -> i32 { return - /* é */ 2147483648 * -1; }";
    let (sources, p) = raw(text);
    let source = sources.get(p.functions[0].span.file);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let expression = ast.expressions.last().unwrap();
    let ast::ExprKind::Arithmetic {
        op,
        left,
        right,
        operator_span,
    } = expression.kind
    else {
        panic!("arithmetic")
    };
    assert_eq!(op, hir::ArithmeticOp::Multiply);
    assert_eq!(&text[operator_span.start..operator_span.end], "*");
    assert_eq!(
        &text[ast.expressions[left.0].span.start..ast.expressions[left.0].span.end],
        "- /* é */ 2147483648"
    );
    assert_eq!(
        &text[ast.expressions[right.0].span.start..ast.expressions[right.0].span.end],
        "-1"
    );
    let resolved = hir::resolve(source, &ast).unwrap();
    let hir::ExprKind::Arithmetic {
        operator_span: hir_span,
        ..
    } = resolved.functions[0].expressions.last().unwrap().kind
    else {
        panic!("HIR")
    };
    assert_eq!(operator_span, hir_span);
    let assign = p.functions[0].blocks[0]
        .statements
        .last()
        .unwrap()
        .assignment();
    let Rvalue::CheckedI32 {
        operator_span: ir_span,
        left: ir_left,
        right: ir_right,
        ..
    } = assign.value
    else {
        panic!("OIR")
    };
    assert_eq!(assign.span, expression.span);
    assert_eq!(ir_span, operator_span);
    assert_eq!(ir_left.span, ast.expressions[left.0].span);
    assert_eq!(ir_right.span, ast.expressions[right.0].span);
    let verified = verify::verify(p, &sources).unwrap();
    assert_eq!(
        verified.run(Some(hir::DefId(0))),
        Err(RunFailure::Overflow(operator_span))
    );
}
#[test]
fn verifier_checks_each_arithmetic_operand_and_destination_type_and_id() {
    for side in 0..3 {
        for ty in [hir::Ty::Bool, hir::Ty::Unit] {
            let (sources, mut p) = simple();
            let f = &mut p.functions[0];
            f.locals[side].ty = ty;
            if side < 2 {
                f.blocks[0].statements[side].assignment_mut().value = if ty == hir::Ty::Bool {
                    Rvalue::Bool(true)
                } else {
                    Rvalue::Unit
                };
            }
            assert_eq!(
                verify::verify(p, &sources).unwrap_err().kind,
                FailureKind::TypeMismatch
            );
        }
        let (sources, mut p) = simple();
        let assign = p.functions[0].blocks[0].statements[2].assignment_mut();
        let Rvalue::CheckedI32 { left, right, .. } = &mut assign.value else {
            panic!()
        };
        match side {
            0 => left.local = LocalId(usize::MAX),
            1 => right.local = LocalId(usize::MAX),
            _ => assign.destination = LocalId(usize::MAX),
        }
        assert_eq!(
            verify::verify(p, &sources).unwrap_err().kind,
            FailureKind::InvalidLocal
        );
    }
}
#[test]
fn arithmetic_operands_must_be_initialized_before_assignment_in_both_positions() {
    for side in 0..2 {
        for destination_self_read in [false, true] {
            let (sources, mut p) = simple();
            let f = &mut p.functions[0];
            let local = if destination_self_read {
                LocalId(2)
            } else {
                f.locals.push(f.locals[0].clone());
                LocalId(3)
            };
            let Rvalue::CheckedI32 { left, right, .. } =
                &mut f.blocks[0].statements[2].assignment_mut().value
            else {
                panic!()
            };
            if side == 0 {
                left.local = local;
            } else {
                right.local = local;
            }
            assert_eq!(
                verify::verify(p, &sources).unwrap_err().kind,
                FailureKind::Uninitialized
            );
        }
        let (sources, mut p) = simple();
        // The other literal exists later in the same block: it is not dominating.
        p.functions[0].blocks[0].statements.swap(side, 2);
        assert_eq!(
            verify::verify(p, &sources).unwrap_err().kind,
            FailureKind::Uninitialized
        );
    }
}
#[test]
fn branch_only_and_call_results_cannot_bypass_arithmetic_operand_dominance() {
    for side in 0..2 {
        let (sources, mut p) = raw("fn id(x: i32) -> i32 { return x; } fn main() -> i32 { if true { id(10); } return 1 + 2; }");
        let main = &mut p.functions[1];
        let destination = main
            .blocks
            .iter()
            .find_map(|b| match b.terminator.as_ref().unwrap().kind {
                TerminatorKind::Call { destination, .. } => Some(destination),
                _ => None,
            })
            .unwrap();
        let assign = main
            .blocks
            .iter_mut()
            .flat_map(|b| &mut b.statements)
            .map(Statement::assignment_mut)
            .find(|a| matches!(a.value, Rvalue::CheckedI32 { .. }))
            .unwrap();
        let Rvalue::CheckedI32 { left, right, .. } = &mut assign.value else {
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
fn arithmetic_operator_and_both_operand_spans_are_validated() {
    for field in 0..3 {
        for range in [(4, 5), (usize::MAX, usize::MAX)] {
            let (sources, mut p) = simple();
            let Rvalue::CheckedI32 {
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
            let origin = match field {
                0 => operator_span,
                1 => &mut left.span,
                _ => &mut right.span,
            };
            origin.start = range.0;
            origin.end = range.1;
            let error = verify::verify(p, &sources).unwrap_err();
            assert_eq!(error.kind, FailureKind::InvalidSpan);
            assert!(error
                .diagnostic(&sources)
                .render_json(&sources)
                .contains("\"primary\":null"));
        }
    }
}
