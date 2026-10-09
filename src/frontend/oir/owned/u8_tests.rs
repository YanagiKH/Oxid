//! RFC 0030 owned consumers: independent raw type, fuel and native controls.
use super::consumer_fixtures as fixtures;
use super::*;

pub(super) fn raw(value: i32, guarded: bool) -> (SourceMap, RawOwnedProgram, [Span; 8]) {
    let text=format!("struct R {{}} fn main()->i32{{let x={value};let b=x.to_u8_checked();return b.to_i32();}}{}",if guarded {" fn cycle()->(){while true{}return;}"} else {""});
    let (sources, raw) = source::u8_tests::raw_source(&text);
    let source = sources.get(crate::frontend::source::SourceFileId(0));
    let f = &raw.functions[0];
    let statement = |i: usize| f.blocks[0].statements[i].span;
    let start = text.find("to_u8_checked").unwrap();
    let spans = [
        f.span,
        statement(0),
        statement(2),
        statement(3),
        source.span(start, start + 13),
        statement(5),
        statement(6),
        f.blocks[0].terminator.as_ref().unwrap().span,
    ];
    (sources, raw, spans)
}
fn verify_source(raw: RawOwnedProgram, sources: &SourceMap) -> verified::VerifiedOwnedProgram {
    source::u8_tests::verify_source_raw(raw, sources).unwrap()
}

#[test]
fn owned_u8_roundtrips_and_paid_failure_have_independent_exact_fuel() {
    for value in 0..=256 {
        let (sources, raw, spans) = raw(value, false);
        let witness = verify_source(raw, &sources);
        for (fuel, expected) in [
            (10, Err(RunFailure::Fuel(spans[2]))),
            (11, Err(RunFailure::Fuel(spans[3]))),
            (
                12,
                if value == 256 {
                    Err(RunFailure::ByteRange(spans[4]))
                } else {
                    Err(RunFailure::Fuel(
                        witness.functions()[0].blocks[0].statements[4].span,
                    ))
                },
            ),
            (
                16,
                if value == 256 {
                    Err(RunFailure::ByteRange(spans[4]))
                } else {
                    Ok(Scalar::I32(value))
                },
            ),
        ] {
            assert_eq!(
                execute::run_limits(
                    &witness,
                    Some(hir::DefId(0)),
                    execute::Limits {
                        fuel,
                        ..Default::default()
                    }
                ),
                expected.map_err(execute::OwnedRunFailure::Scalar),
                "value={value} fuel={fuel}"
            );
        }
    }
    for value in [-1, i32::MIN, i32::MAX] {
        let (sources, raw, spans) = raw(value, false);
        let witness = verify_source(raw, &sources);
        assert_eq!(
            execute::run(&witness, Some(hir::DefId(0))),
            Err(execute::OwnedRunFailure::Scalar(RunFailure::ByteRange(
                spans[4]
            )))
        );
    }
}

#[test]
fn owned_u8_verifier_checks_all_conversion_type_pairs() {
    for narrow in [false, true] {
        for input in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
            for output in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
                let (sources, mut raw, s) = raw(0, false);
                let f = &mut raw.functions[0];
                f.result = ValueTy::Scalar(output);
                f.locals = vec![
                    LocalDecl {
                        ty: input,
                        kind: LocalKind::Parameter,
                        span: s[0],
                    },
                    fixtures::scalar(output, s[0]),
                ];
                f.parameters = vec![ParameterBinding::Scalar(LocalId(0))];
                f.blocks[0].statements = vec![fixtures::assign(
                    1,
                    if narrow {
                        Rvalue::CheckedI32ToU8 {
                            source_expr: crate::frontend::ast::ExprId(0),
                            operand: fixtures::operand(0, s[2]),
                            name_span: s[4],
                        }
                    } else {
                        Rvalue::U8ToI32 {
                            source_expr: crate::frontend::ast::ExprId(0),
                            operand: fixtures::operand(0, s[2]),
                            name_span: s[4],
                        }
                    },
                    s[3],
                )];
                f.blocks[0].terminator = fixtures::end(
                    OwnedTerminatorKind::ReturnScalar(fixtures::operand(1, s[7])),
                    s[7],
                );
                let valid = if narrow {
                    (input, output) == (hir::Ty::I32, hir::Ty::U8)
                } else {
                    (input, output) == (hir::Ty::U8, hir::Ty::I32)
                };
                assert_eq!(
                    super::super::verify::scalar_statement_shape(
                        &f.locals,
                        &f.places,
                        match &f.blocks[0].statements[0].kind {
                            OwnedInstruction::Scalar(s) => s,
                            _ => unreachable!(),
                        },
                        &sources
                    )
                    .is_ok(),
                    valid,
                    "narrow={narrow} input={input:?} output={output:?}"
                );
            }
        }
    }
}

#[test]
fn owned_u8_verifier_rejects_forged_constant_ids_order_and_spans() {
    for mutation in 0..7 {
        let (sources, mut raw, _) = raw(0, false);
        let f = &mut raw.functions[0];
        if mutation == 0 {
            f.locals[0].ty = hir::Ty::U8;
        } else if mutation == 1 {
            f.blocks[0].statements.swap(2, 3);
        } else {
            let OwnedInstruction::Scalar(Statement::Assign(a)) =
                &mut f.blocks[0].statements[3].kind
            else {
                unreachable!()
            };
            if mutation == 2 {
                a.destination = LocalId(usize::MAX);
            } else {
                let Rvalue::CheckedI32ToU8 {
                    operand, name_span, ..
                } = &mut a.value
                else {
                    unreachable!()
                };
                match mutation {
                    3 => operand.local = LocalId(usize::MAX),
                    4 => operand.local = LocalId(2),
                    5 => name_span.end = usize::MAX,
                    6 => operand.span.start = usize::MAX,
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            source::u8_tests::verify_source_raw(raw, &sources).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn owned_u8_native_ir_keeps_i64_arenas_and_checked_unsigned_semantics() {
    for guarded in [false, true] {
        let (sources, raw, _) = raw(256, guarded);
        let witness = verify_source(raw, &sources);
        let ir = native::native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
        assert!(ir.contains("icmp slt i32 %f0_b0_i3_operand, 0"));
        assert!(ir.contains("icmp sgt i32 %f0_b0_i3_operand, 255"));
        assert!(ir.contains("unreachable\nf0_b0_i3_conversion_ok:\n  %f0_b0_i3_value = trunc i32"));
        assert!(ir.contains("zext i8 %f0_b0_i3_value to i64"));
        assert!(ir.contains("zext i8 %f0_b0_i6_operand to i32"));
        assert!(!ir.contains("sext i8"));
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn owned_u8_real_native_range_and_fuel_match_reference() {
    for value in [0, 127, 128, 255, -1, 256, i32::MIN, i32::MAX] {
        let (sources, raw, _) = raw(value, true);
        let witness = verify_source(raw, &sources);
        for fuel in [10, 11, 12, 14, 15, 16] {
            let ir =
                native::native_module_with_fuel(&witness, hir::DefId(0), &sources, fuel).unwrap();
            let expected = execute::run_limits(
                &witness,
                Some(hir::DefId(0)),
                execute::Limits {
                    fuel,
                    ..Default::default()
                },
            )
            .map_err(|e| match e {
                execute::OwnedRunFailure::Scalar(e) => e,
                other => panic!("{other:?}"),
            });
            super::super::negation_raw_tests::run_native(&ir, &expected, &sources);
        }
    }
}

fn comparison(a: u8, b: u8, op: hir::ComparisonOp) -> (SourceMap, RawOwnedProgram) {
    let spelling = match op {
        hir::ComparisonOp::Equal => "==",
        hir::ComparisonOp::NotEqual => "!=",
        hir::ComparisonOp::Less => "<",
        hir::ComparisonOp::LessEqual => "<=",
        hir::ComparisonOp::Greater => ">",
        hir::ComparisonOp::GreaterEqual => ">=",
    };
    source::u8_tests::raw_source(&format!("struct R {{}} fn main()->bool{{let x={a};let a=x.to_u8_checked();let y={b};let b=y.to_u8_checked();return a{spelling}b;}}"))
}

#[test]
fn owned_u8_all_six_comparisons_use_unsigned_values() {
    for a in [0, 1, 127, 128, 254, 255] {
        for b in [0, 1, 127, 128, 254, 255] {
            for (op, expected, predicate) in [
                (hir::ComparisonOp::Equal, a == b, "eq"),
                (hir::ComparisonOp::NotEqual, a != b, "ne"),
                (hir::ComparisonOp::Less, a < b, "ult"),
                (hir::ComparisonOp::LessEqual, a <= b, "ule"),
                (hir::ComparisonOp::Greater, a > b, "ugt"),
                (hir::ComparisonOp::GreaterEqual, a >= b, "uge"),
            ] {
                let (sources, raw) = comparison(a, b, op);
                let witness = verify_source(raw, &sources);
                assert_eq!(
                    execute::run(&witness, Some(hir::DefId(0))).unwrap(),
                    Scalar::Bool(expected),
                    "{a} {op:?} {b}"
                );
                let ir = native::native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
                assert!(ir.contains(&format!("icmp {predicate} i8")));
            }
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn owned_u8_real_native_unsigned_comparisons() {
    for (a, b) in [(0, 255), (127, 128), (255, 0), (128, 128)] {
        for (op, expected) in [
            (hir::ComparisonOp::Equal, a == b),
            (hir::ComparisonOp::NotEqual, a != b),
            (hir::ComparisonOp::Less, a < b),
            (hir::ComparisonOp::LessEqual, a <= b),
            (hir::ComparisonOp::Greater, a > b),
            (hir::ComparisonOp::GreaterEqual, a >= b),
        ] {
            let (sources, raw) = comparison(a, b, op);
            let witness = verify_source(raw, &sources);
            let ir = native::native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
            super::super::negation_raw_tests::run_native(
                &ir,
                &Ok(Scalar::Bool(expected)),
                &sources,
            );
        }
    }
}
