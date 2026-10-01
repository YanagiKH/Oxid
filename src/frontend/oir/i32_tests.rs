//! Numeric seam regressions complement the generic verifier's CFG/dataflow tests.
use super::*;
use crate::frontend::{ast, lexer, parser};
fn raw(text: &str) -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("i32.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    (sources, lower::lower(&typed).unwrap())
}
fn constant() -> (SourceMap, Program) {
    raw("// 雪\r\nfn main() -> i32 { return - /* é */ 2147483648; }")
}
#[test]
fn exact_signed_origin_and_value_survive_every_representation() {
    let text = "// 雪\r\nfn main() -> i32 { return - /* é */ 2147483648; }";
    let (sources, raw) = constant();
    let source = sources.get(raw.functions[0].span.file);
    let parsed = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let span = source.span(
        text.find("- /*").unwrap(),
        text.find("2147483648").unwrap() + 10,
    );
    assert_eq!(parsed.expressions[0].span, span);
    let ast::ExprKind::Number { digits, negative } = parsed.expressions[0].kind else {
        panic!("literal")
    };
    assert!(negative);
    assert_eq!(&text[digits.start..digits.end], "2147483648");
    let resolved = hir::resolve(source, &parsed).unwrap();
    assert_eq!(resolved.functions[0].expressions[0].span, span);
    assert!(matches!(
        resolved.functions[0].expressions[0].kind,
        hir::ExprKind::I32(i32::MIN)
    ));
    let typed = typeck::check(resolved).unwrap();
    assert_eq!(
        typed
            .functions()
            .next()
            .unwrap()
            .expression_ty(hir::ExprId(0)),
        hir::Ty::I32
    );
    let f = &raw.functions[0];
    assert_eq!(f.locals[0].span, span);
    assert_eq!(f.locals[0].ty, hir::Ty::I32);
    assert_eq!(f.blocks[0].statements[0].assignment().span, span);
    assert_eq!(
        f.blocks[0].statements[0].assignment().value,
        Rvalue::I32(i32::MIN)
    );
    let verified = verify::verify(raw, &sources).unwrap();
    for _ in 0..3 {
        assert_eq!(verified.run(Some(hir::DefId(0))), Ok(Scalar::I32(i32::MIN)));
    }
}
#[test]
fn raw_i32_constants_only_initialize_i32_destinations() {
    for value in [i32::MIN, -1, 0, 1, i32::MAX] {
        let (sources, mut p) = constant();
        p.functions[0].blocks[0].statements[0]
            .assignment_mut()
            .value = Rvalue::I32(value);
        let verified = verify::verify(p.clone(), &sources).unwrap();
        assert_eq!(verified.run(Some(hir::DefId(0))), Ok(Scalar::I32(value)));
        for ty in [hir::Ty::Bool, hir::Ty::Unit] {
            let mut wrong = p.clone();
            wrong.functions[0].locals[0].ty = ty;
            assert_eq!(
                verify::verify(wrong, &sources).unwrap_err().kind,
                FailureKind::TypeMismatch
            );
        }
        for constant in [Rvalue::Unit, Rvalue::Bool(false)] {
            let mut wrong = p.clone();
            wrong.functions[0].blocks[0].statements[0]
                .assignment_mut()
                .value = constant;
            assert_eq!(
                verify::verify(wrong, &sources).unwrap_err().kind,
                FailureKind::TypeMismatch
            );
        }
    }
}
#[test]
fn raw_i32_copy_argument_return_and_branch_types_cannot_bypass_verification() {
    let (sources, p) = raw(
        "fn id(x: i32) -> i32 { let copy = x; return copy; } fn main() -> i32 { return id(-1); }",
    );
    for mutation in 0..4 {
        let mut wrong = p.clone();
        match mutation {
            0 => {
                // Copy destination differs from its i32 operand.
                let destination = wrong.functions[0].blocks[0].statements[0]
                    .assignment()
                    .destination;
                wrong.functions[0].locals[destination.0].ty = hir::Ty::Bool;
            }
            1 => {
                // Valid bool caller constant passed to the i32 parameter.
                let main = &mut wrong.functions[1];
                let assign = main.blocks[0].statements[0].assignment_mut();
                main.locals[assign.destination.0].ty = hir::Ty::Bool;
                assign.value = Rvalue::Bool(true);
            }
            2 => {
                // Call destination differs from the callee result.
                let main = &mut wrong.functions[1];
                let TerminatorKind::Call { destination, .. } =
                    main.blocks[0].terminator.as_ref().unwrap().kind
                else {
                    panic!("call")
                };
                main.locals[destination.0].ty = hir::Ty::Unit;
            }
            3 => wrong.functions[0].result = hir::Ty::Unit,
            _ => unreachable!(),
        }
        assert_eq!(
            verify::verify(wrong, &sources).unwrap_err().kind,
            FailureKind::TypeMismatch,
            "mutation {mutation}"
        );
    }
    let (sources, mut p) = raw("fn main() -> i32 { if true { return 0; } else { return 1; } }");
    let main = &mut p.functions[0];
    let assign = main.blocks[0].statements[0].assignment_mut();
    main.locals[assign.destination.0].ty = hir::Ty::I32;
    assign.value = Rvalue::I32(1);
    assert_eq!(
        verify::verify(p, &sources).unwrap_err().kind,
        FailureKind::TypeMismatch
    );
}
#[test]
fn raw_i32_origins_and_aggregate_local_limit_are_still_checked() {
    let (sources, p) = constant();
    for bad_span in [
        Span {
            start: 4,
            end: 5,
            ..p.functions[0].span
        },
        Span {
            start: usize::MAX,
            end: usize::MAX,
            ..p.functions[0].span
        },
    ] {
        let mut wrong = p.clone();
        wrong.functions[0].blocks[0].statements[0]
            .assignment_mut()
            .span = bad_span;
        let error = verify::verify(wrong, &sources).unwrap_err();
        assert_eq!(error.kind, FailureKind::InvalidSpan);
        assert!(error
            .diagnostic(&sources)
            .render_json(&sources)
            .contains("\"primary\":null"));
    }
    let mut bounded = p;
    let decl = bounded.functions[0].locals[0].clone();
    bounded.functions[0].locals.resize(MAX_LOCALS, decl.clone());
    assert!(verify::verify(bounded.clone(), &sources).is_ok());
    bounded.functions[0].locals.push(decl);
    assert_eq!(
        verify::verify(bounded, &sources).unwrap_err().kind,
        FailureKind::ResourceLimit("locals")
    );
}
