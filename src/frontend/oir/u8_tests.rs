//! RFC0030 scalar conversion contracts and independent malformed OIR controls.
use super::*;
use crate::frontend::{ast, lexer, parser};

fn parsed(text: &str) -> (SourceMap, ast::Program) {
    let mut sources = SourceMap::new();
    let file = sources.add("u8-雪.ox".into(), text.into());
    let source = sources.get(file);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    (sources, ast)
}
fn raw(text: &str) -> (SourceMap, ast::Program, Program) {
    let (sources, ast) = parsed(text);
    let source = sources.get(crate::frontend::source::SourceFileId(0));
    assert!(
        !ast.uses_owned_syntax(source),
        "byte scalars must retain scalar routing"
    );
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    let raw = lower::lower(&typed).unwrap();
    (sources, ast, raw)
}
fn associated(
    raw: Program,
    sources: &SourceMap,
    ast: &ast::Program,
) -> Result<VerifiedProgram, OirFailure> {
    let associated = source::association::authenticate_scalar(
        raw,
        sources,
        source::association::Declarations::Original(ast),
    )
    .unwrap();
    verify::verify_associated(associated)
}
fn check(text: &str) -> (SourceMap, Vec<Diagnostic>) {
    let (sources, ast) = parsed(text);
    let source = sources.get(crate::frontend::source::SourceFileId(0));
    let errors = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap_err();
    (sources, errors)
}
fn run(text: &str, entry: usize) -> Scalar {
    let (sources, ast, raw) = raw(text);
    source::association::scalar(
        &raw,
        &sources,
        source::association::Declarations::Original(&ast),
    )
    .unwrap();
    associated(raw, &sources, &ast)
        .unwrap()
        .run(Some(hir::DefId(entry)))
        .unwrap()
}

#[test]
fn conversion_has_real_receiver_snapshot_distinct_temp_and_all_origins() {
    let text = "// 雪\r\nfn main()->i32{let x=255;let b=x /* 雪 */ . /* a */ to_u8_checked /* b */ ( /* c */ );return b.to_i32();}";
    let (sources, ast, raw) = raw(text);
    let f = &raw.functions[0];
    let assignments = &f.blocks[0].statements;
    assert_eq!(f.locals.len(), 7);
    assert_eq!(assignments.len(), 7);
    let snapshot = assignments[2].assignment();
    let narrow = assignments[3].assignment();
    let Rvalue::CheckedI32ToU8 {
        operand, name_span, ..
    } = narrow.value
    else {
        panic!("narrow")
    };
    assert_eq!(operand.local, snapshot.destination);
    assert_ne!(narrow.destination, snapshot.destination);
    assert_eq!(f.locals[narrow.destination.0].ty, hir::Ty::U8);
    assert_eq!(f.locals[snapshot.destination.0].ty, hir::Ty::I32);
    assert_eq!(&text[operand.span.start..operand.span.end], "x");
    assert_eq!(operand.span, snapshot.span);
    assert_eq!(&text[name_span.start..name_span.end], "to_u8_checked");
    assert_eq!(
        &text[narrow.span.start..narrow.span.end],
        "x /* 雪 */ . /* a */ to_u8_checked /* b */ ( /* c */ )"
    );
    source::association::scalar(
        &raw,
        &sources,
        source::association::Declarations::Original(&ast),
    )
    .unwrap();
    assert_eq!(
        associated(raw, &sources, &ast)
            .unwrap()
            .run(Some(hir::DefId(0))),
        Ok(Scalar::I32(255))
    );
}

#[test]
fn scalar_snapshot_narrow_widen_and_return_exact_fuel_controls() {
    for value in [255, 256] {
        let text = format!(
            "fn main() -> i32 {{ let x = {value}; let b = x.to_u8_checked(); return b.to_i32(); }}"
        );
        let (sources, ast, raw) = raw(&text);
        let f = &raw.functions[0];
        assert_eq!(f.locals.len(), 7);
        let statements = &f.blocks[0].statements;
        let read = statements[2].span();
        let narrow = statements[3].assignment();
        let Rvalue::CheckedI32ToU8 { name_span, .. } = narrow.value else {
            panic!()
        };
        let narrow_span = narrow.span;
        let widen = statements[6].span();
        let ret = f.blocks[0].terminator.as_ref().unwrap().span;
        let program = associated(raw, &sources, &ast).unwrap();
        assert_eq!(
            execute::run_with_fuel(&program, hir::DefId(0), 10),
            Err(RunFailure::Fuel(read))
        );
        assert_eq!(
            execute::run_with_fuel(&program, hir::DefId(0), 11),
            Err(RunFailure::Fuel(narrow_span))
        );
        if value == 256 {
            assert_eq!(
                execute::run_with_fuel(&program, hir::DefId(0), 12),
                Err(RunFailure::ByteRange(name_span))
            );
        } else {
            assert_eq!(
                execute::run_with_fuel(&program, hir::DefId(0), 14),
                Err(RunFailure::Fuel(widen))
            );
            assert_eq!(
                execute::run_with_fuel(&program, hir::DefId(0), 15),
                Err(RunFailure::Fuel(ret))
            );
            assert_eq!(
                execute::run_with_fuel(&program, hir::DefId(0), 16),
                Ok(Scalar::I32(255))
            );
        }
    }
}

#[test]
fn bytes_flow_through_mutable_places_helpers_recursion_and_unsigned_edges() {
    assert_eq!(run("fn pass(x:u8)->u8{return x;} fn byte(x:i32)->u8{return x.to_u8_checked();} fn main()->i32{let lo=byte(0);let a=byte(127);let b=byte(128);let hi=pass(byte(255));if lo<a && a<b && b<hi{return hi.to_i32();}return -1;}", 2), Scalar::I32(255));
    assert_eq!(run("fn byte(x:i32)->u8{return x.to_u8_checked();} fn main()->i32{let mut b=byte(127);let a=b;b=byte(128);return a.to_i32();}", 1), Scalar::I32(127));
    assert_eq!(run("fn recurse(b:u8,n:i32)->u8{if n==0{return b;}return recurse(b,n-1);} fn main()->i32{let x=255;let b=recurse(x.to_u8_checked(),4);return b.to_i32();}", 1), Scalar::I32(255));
    assert_eq!(run("fn bad()->bool{let n=256;n.to_u8_checked();return true;} fn main()->i32{if false && bad(){return -1;}if true || bad(){let x=255;let b=x.to_u8_checked();return b.to_i32();}return 0;}", 1), Scalar::I32(255));
}

#[test]
fn scalar_typecheck_keeps_closed_conversion_and_operator_tables() {
    for (ty, expression, expected) in [
        ("i32", "x.to_i32()", "u8"),
        ("u8", "x.to_u8_checked()", "i32"),
        ("bool", "x.to_u8_checked()", "i32"),
        ("()", "x.to_i32()", "u8"),
    ] {
        let text = format!("fn f(x:{ty})->i32{{return {expression};}}");
        let (sources, errors) = check(&text);
        assert_eq!(errors.len(), 1);
        let e = &errors[0];
        assert_eq!((e.code, e.stage), ("E0300", "type"));
        assert_eq!(
            e.message,
            format!("type mismatch: expected {expected}, found {ty}")
        );
        assert_eq!(
            sources
                .get(e.primary.unwrap().file)
                .text_at(e.primary.unwrap()),
            "x"
        );
    }
    for expression in [
        "a+b", "a-b", "a*b", "a/b", "a%b", "-a", "!a", "a&&b", "a||b", "a==1", "a!=1", "a<1",
        "a<=1", "a>1", "a>=1", "1==a", "1<a",
    ] {
        let (_, errors) = check(&format!("fn f(a:u8,b:u8)->bool{{return {expression};}}"));
        assert_eq!(
            (errors[0].code, errors[0].stage),
            ("E0300", "type"),
            "{expression}"
        );
    }
    for text in [
        "fn f()->u8{return 255;}",
        "fn f()->(){let x:u8=255;return;}",
        "fn f(x:u8)->(){return;}fn main()->(){f(255);return;}",
    ] {
        let (_, errors) = check(text);
        assert_eq!((errors[0].code, errors[0].stage), ("E0300", "type"));
    }
    let (sources, errors) = check("fn main() -> bool { return () == (true + 1); }");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type mismatch: expected i32, found bool");
    assert_eq!(
        sources
            .get(errors[0].primary.unwrap().file)
            .text_at(errors[0].primary.unwrap()),
        "true"
    );
}

#[test]
fn verifier_checks_every_conversion_operand_and_destination_type_pair() {
    for (text, narrow) in [
        ("fn f(x:i32)->u8{return x.to_u8_checked();}", true),
        ("fn f(x:u8)->i32{return x.to_i32();}", false),
    ] {
        let (sources, _, baseline) = raw(text);
        for operand_ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
            for destination_ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
                let mut raw = baseline.clone();
                let f = &mut raw.functions[0];
                f.locals[0].ty = operand_ty;
                f.locals[1].ty = operand_ty;
                f.locals[2].ty = destination_ty;
                f.result = destination_ty;
                let permitted = if narrow {
                    (hir::Ty::I32, hir::Ty::U8)
                } else {
                    (hir::Ty::U8, hir::Ty::I32)
                };
                let result = verify::scalar_statement_shape(
                    &f.locals,
                    &f.places,
                    &f.blocks[0].statements[1],
                    &sources,
                );
                if (operand_ty, destination_ty) == permitted {
                    result.unwrap();
                } else {
                    assert_eq!(result.unwrap_err().kind, FailureKind::TypeMismatch);
                }
            }
        }
    }
}

#[test]
fn verifier_rejects_raw_byte_constants_invalid_ids_and_missing_snapshots() {
    let (sources, _, baseline) = raw("fn f(x:i32)->u8{return x.to_u8_checked();}");
    let mut raw = baseline.clone();
    raw.functions[0].blocks[0].statements[1]
        .assignment_mut()
        .value = Rvalue::I32(0);
    assert_eq!(
        verify::verify(raw, &sources).unwrap_err().kind,
        FailureKind::TypeMismatch
    );
    let mut raw = baseline.clone();
    let Rvalue::CheckedI32ToU8 { operand, .. } = &mut raw.functions[0].blocks[0].statements[1]
        .assignment_mut()
        .value
    else {
        panic!()
    };
    operand.local = LocalId(usize::MAX);
    assert_eq!(
        verify::scalar_statement_shape(
            &raw.functions[0].locals,
            &raw.functions[0].places,
            &raw.functions[0].blocks[0].statements[1],
            &sources
        )
        .unwrap_err()
        .kind,
        FailureKind::InvalidLocal
    );
    let mut raw = baseline.clone();
    raw.functions[0].blocks[0].statements.swap(0, 1);
    assert!(verify::verify(raw, &sources).is_err());
    let mut raw = baseline;
    raw.functions[0].blocks[0].statements.remove(0);
    assert!(verify::verify(raw, &sources).is_err());
}

#[test]
fn raw_byte_comparison_table_and_arithmetic_fences_are_independent() {
    let (sources, _, baseline) = raw("fn compare(a:u8,b:u8)->bool{return a==b;}");
    let f = &baseline.functions[0];
    let template = f.blocks[0].statements[2].assignment().clone();
    for op in [
        hir::ComparisonOp::Equal,
        hir::ComparisonOp::NotEqual,
        hir::ComparisonOp::Less,
        hir::ComparisonOp::LessEqual,
        hir::ComparisonOp::Greater,
        hir::ComparisonOp::GreaterEqual,
    ] {
        for left in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
            for right in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
                let mut function = f.clone();
                function.locals[2].ty = left;
                function.locals[3].ty = right;
                let mut assign = template.clone();
                let Rvalue::CompareScalar { op: actual, .. } = &mut assign.value else {
                    panic!()
                };
                *actual = op;
                let allowed = left == right
                    && (matches!(left, hir::Ty::I32 | hir::Ty::U8)
                        || (left == hir::Ty::Bool
                            && matches!(
                                op,
                                hir::ComparisonOp::Equal | hir::ComparisonOp::NotEqual
                            )));
                assert_eq!(
                    verify::scalar_statement_shape(
                        &function.locals,
                        &function.places,
                        &Statement::Assign(assign),
                        &sources
                    )
                    .is_ok(),
                    allowed,
                    "{op:?} {left:?} {right:?}"
                );
            }
        }
    }
    let Rvalue::CompareScalar {
        left,
        right,
        operator_span,
        ..
    } = template.value
    else {
        panic!()
    };
    for op in [
        hir::ArithmeticOp::Add,
        hir::ArithmeticOp::Subtract,
        hir::ArithmeticOp::Multiply,
        hir::ArithmeticOp::Divide,
        hir::ArithmeticOp::Remainder,
    ] {
        let mut function = f.clone();
        function.locals[template.destination.0].ty = hir::Ty::U8;
        let statement = Statement::Assign(Assign {
            value: Rvalue::CheckedI32 {
                op,
                left,
                right,
                operator_span,
            },
            ..template.clone()
        });
        assert_eq!(
            verify::scalar_statement_shape(
                &function.locals,
                &function.places,
                &statement,
                &sources
            )
            .unwrap_err()
            .kind,
            FailureKind::TypeMismatch
        );
    }
}

#[test]
fn u8_scalar_measured_carriers_and_successful_payloads() {
    use std::mem::{align_of, size_of};
    let text = "fn main()->i32{let x=255;let b=x.to_u8_checked();return b.to_i32();}";
    let (sources, ast) = parsed(text);
    let hir = hir::resolve(sources.get(crate::frontend::source::SourceFileId(0)), &ast).unwrap();
    let type_guard = typeck::measurement::begin();
    let typed = typeck::check(hir).unwrap();
    let type_measurement = type_guard.finish();
    let lower_guard = lower::measurement::begin();
    let raw = lower::lower(&typed).unwrap();
    let lower_measurement = lower_guard.finish();
    let retained = lower::measurement::retained_payload(&raw).unwrap();
    let associated = source::association::authenticate_scalar(
        raw,
        &sources,
        source::association::Declarations::Original(&ast),
    )
    .unwrap();
    let verify_guard = verify::measurement::begin();
    let verified = verify::verify_associated(associated).unwrap();
    let verify_measurement = verify_guard.finish();
    let execution_guard = execute::measurement::begin();
    assert_eq!(verified.run(Some(hir::DefId(0))), Ok(Scalar::I32(255)));
    let execution_measurement = execution_guard.finish();
    println!("u8_scalar_layout ty=({}, {}) scalar=({}, {}) option_scalar=({}, {}) rvalue=({}, {}) assign=({}, {})",size_of::<hir::Ty>(),align_of::<hir::Ty>(),size_of::<Scalar>(),align_of::<Scalar>(),size_of::<Option<Scalar>>(),align_of::<Option<Scalar>>(),size_of::<Rvalue>(),align_of::<Rvalue>(),size_of::<Assign>(),align_of::<Assign>());
    println!(
        "u8_type_layout={:?} type_success={type_measurement:?}",
        typeck::measurement::layout()
    );
    println!(
        "u8_lower_layout={:?} lower_success={lower_measurement:?} retained={retained:?}",
        lower::measurement::layout()
    );
    println!(
        "u8_verify_layout={:?} verify_success={verify_measurement:?}",
        verify::measurement::layout()
    );
    println!(
        "u8_execute_layout={:?} execute_success={execution_measurement:?}",
        execute::measurement::layout()
    );
}
