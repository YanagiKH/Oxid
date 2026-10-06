//! Owned consumers exercise raw scalar negation without AST/HIR/source lowering.
use super::consumer_fixtures as fixtures;
use super::*;

fn raw(value: i32, guarded: bool) -> (SourceMap, RawOwnedProgram, Span) {
    let mut sources = SourceMap::new();
    let file = sources.add(
        "raw-owned-negation-雪.ox".into(),
        "// 雪\r\nentry input result - return cycle".into(),
    );
    let source = sources.get(file);
    let origins = ["entry", "input", "result", "-", "return", "cycle"].map(|word| {
        let start = source.text().find(word).unwrap();
        source.span(start, start + word.len())
    });
    let span = |i: usize| origins[i];
    let mut f = fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), span(0));
    f.locals = vec![
        fixtures::scalar(hir::Ty::I32, span(1)),
        fixtures::scalar(hir::Ty::I32, span(2)),
    ];
    f.blocks.push(OwnedBlock {
        merge: None,
        span: span(0),
        statements: vec![
            fixtures::assign(0, Rvalue::I32(value), span(1)),
            fixtures::assign(
                1,
                Rvalue::CheckedNegateI32 {
                    operand: fixtures::operand(0, span(1)),
                    operator_span: span(3),
                },
                span(2),
            ),
        ],
        terminator: fixtures::end(
            OwnedTerminatorKind::ReturnScalar(fixtures::operand(1, span(4))),
            span(4),
        ),
    });
    let mut functions = vec![f];
    if guarded {
        let mut cycle = fixtures::function(1, ValueTy::Scalar(hir::Ty::Unit), span(5));
        cycle.blocks.push(OwnedBlock {
            merge: None,
            span: span(5),
            statements: vec![],
            terminator: fixtures::end(OwnedTerminatorKind::Goto(BlockId(0)), span(5)),
        });
        functions.push(cycle);
    }
    (
        sources,
        RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![],
            records: vec![],
            functions,
        },
        span(3),
    )
}
fn assignment(p: &mut RawOwnedProgram) -> &mut Assign {
    let OwnedInstruction::Scalar(Statement::Assign(a)) =
        &mut p.functions[0].blocks[0].statements[1].kind
    else {
        unreachable!()
    };
    a
}

#[test]
fn raw_negation_owned_reference_has_exact_fuel_and_minus_overflow() {
    for value in [0, 1, -1, i32::MAX, i32::MIN + 1, i32::MIN] {
        let (sources, p, minus) = raw(value, false);
        let f = &p.functions[0];
        let entry = f.span;
        let input = f.blocks[0].statements[0].span;
        let negation = f.blocks[0].statements[1].span;
        let ret = f.blocks[0].terminator.as_ref().unwrap().span;
        let witness = verify_owned(p, &sources).unwrap();
        for fuel in 0..=6 {
            let expected = match fuel {
                0..=2 => Err(RunFailure::Fuel(entry)),
                3 => Err(RunFailure::Fuel(input)),
                4 => Err(RunFailure::Fuel(negation)),
                _ if value == i32::MIN => Err(RunFailure::Overflow(minus)),
                5 => Err(RunFailure::Fuel(ret)),
                _ => Ok(Scalar::I32(-value)),
            }
            .map_err(execute::OwnedRunFailure::Scalar);
            assert_eq!(
                execute::run_limits(
                    &witness,
                    Some(hir::DefId(0)),
                    execute::Limits {
                        fuel,
                        ..execute::Limits::default()
                    }
                ),
                expected,
                "value={value}, fuel={fuel}"
            );
        }
    }
}

#[test]
fn raw_negation_owned_verifier_rejects_types_ids_origins_and_nondominating_values() {
    for mutation in 0..9 {
        let (sources, mut p, _) = raw(7, false);
        let expected = match mutation {
            0 | 1 => {
                let ty = if mutation == 0 {
                    hir::Ty::Bool
                } else {
                    hir::Ty::Unit
                };
                p.functions[0].locals[0].ty = ty;
                let OwnedInstruction::Scalar(Statement::Assign(a)) =
                    &mut p.functions[0].blocks[0].statements[0].kind
                else {
                    unreachable!()
                };
                a.value = if ty == hir::Ty::Bool {
                    Rvalue::Bool(true)
                } else {
                    Rvalue::Unit
                };
                FailureKind::TypeMismatch
            }
            2 => {
                p.functions[0].locals[1].ty = hir::Ty::Unit;
                FailureKind::TypeMismatch
            }
            3 => {
                assignment(&mut p).destination = LocalId(usize::MAX);
                FailureKind::InvalidLocal
            }
            4 => {
                let Rvalue::CheckedNegateI32 { operand, .. } = &mut assignment(&mut p).value else {
                    unreachable!()
                };
                operand.local = LocalId(usize::MAX);
                FailureKind::InvalidLocal
            }
            5 | 6 => {
                let Rvalue::CheckedNegateI32 {
                    operand,
                    operator_span,
                } = &mut assignment(&mut p).value
                else {
                    unreachable!()
                };
                let span = if mutation == 5 {
                    operator_span
                } else {
                    &mut operand.span
                };
                span.start = usize::MAX;
                span.end = usize::MAX;
                FailureKind::InvalidSpan
            }
            7 => {
                p.functions[0].blocks[0].statements.swap(0, 1);
                FailureKind::Uninitialized
            }
            8 => {
                let Rvalue::CheckedNegateI32 { operand, .. } = &mut assignment(&mut p).value else {
                    unreachable!()
                };
                operand.local = LocalId(1);
                FailureKind::Uninitialized
            }
            _ => unreachable!(),
        };
        assert_eq!(
            verify_owned(p, &sources).unwrap_err().kind,
            OwnedFailureKind::Malformed(Malformed::Scalar(expected)),
            "mutation {mutation}"
        );
    }
}

#[test]
fn raw_negation_owned_native_checks_before_store_and_accounts_diagnostic() {
    for guarded in [false, true] {
        let (sources, p, minus) = raw(i32::MIN, guarded);
        let witness = verify_owned(p, &sources).unwrap();
        let module = native::native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
        assert!(module.contains("@llvm.ssub.with.overflow.i32(i32 0, i32 %f0_b0_i1_operand)"));
        assert!(module.contains(
            "  br i1 %f0_b0_i1_overflow, label %f0_b0_i1_checked_error, label %f0_b0_i1_checked_ok"
        ));
        assert!(module.contains("  unreachable\nf0_b0_i1_checked_ok:\n  %f0_b0_i1_value = extractvalue { i32, i1 } %f0_b0_i1_checked, 0"));
        let expected = RunFailure::Overflow(minus)
            .diagnostic(&sources)
            .render_human(&sources);
        let encoded: String = expected
            .bytes()
            .map(|byte| format!("\\{byte:02X}"))
            .collect();
        assert!(module.contains(&encoded));
        for forbidden in ["nsw", "nuw", "poison", "undef", " sub i32 "] {
            assert!(!module.contains(forbidden), "{forbidden}");
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn raw_negation_owned_witnesses_use_real_llvm() {
    for guarded in [false, true] {
        for value in [0, 1, -1, i32::MAX, i32::MIN + 1, i32::MIN] {
            let (sources, p, minus) = raw(value, guarded);
            let witness = verify_owned(p, &sources).unwrap();
            let module =
                native::native_module_with_fuel(&witness, hir::DefId(0), &sources, 6).unwrap();
            let expected = if value == i32::MIN {
                Err(RunFailure::Overflow(minus))
            } else {
                Ok(Scalar::I32(-value))
            };
            assert_eq!(
                execute::run(&witness, Some(hir::DefId(0))),
                if value == i32::MIN {
                    Err(execute::OwnedRunFailure::Scalar(RunFailure::Overflow(
                        minus,
                    )))
                } else {
                    Ok(Scalar::I32(-value))
                }
            );
            super::super::negation_raw_tests::run_native(&module, &expected, &sources);
            if guarded {
                for fuel in [3, 4, 5] {
                    let module =
                        native::native_module_with_fuel(&witness, hir::DefId(0), &sources, fuel)
                            .unwrap();
                    let expected = execute::run_limits(
                        &witness,
                        Some(hir::DefId(0)),
                        execute::Limits {
                            fuel,
                            ..execute::Limits::default()
                        },
                    )
                    .map_err(|error| match error {
                        execute::OwnedRunFailure::Scalar(error) => error,
                        other => panic!("unexpected failure: {other:?}"),
                    });
                    super::super::negation_raw_tests::run_native(&module, &expected, &sources);
                }
            }
        }
    }
}

fn phi_raw(guarded: bool) -> (SourceMap, RawOwnedProgram) {
    let (sources, mut p, minus) = raw(7, guarded);
    let f = &mut p.functions[0];
    let span = f.span;
    let negate = f.blocks[0].statements.pop().unwrap();
    f.result = ValueTy::Scalar(hir::Ty::Bool);
    f.locals
        .extend((2..6).map(|_| fixtures::scalar(hir::Ty::Bool, span)));
    let operand = |local| fixtures::operand(local, span);
    let end = |kind| fixtures::end(kind, span);
    f.blocks[0]
        .statements
        .push(fixtures::assign(2, Rvalue::Bool(true), span));
    f.blocks[0].terminator = end(OwnedTerminatorKind::Branch {
        condition: operand(2),
        then_block: BlockId(1),
        else_block: BlockId(2),
    });
    f.blocks.extend([
        OwnedBlock {
            merge: None,
            span,
            statements: vec![
                negate,
                fixtures::assign(
                    3,
                    Rvalue::CompareScalar {
                        op: hir::ComparisonOp::Less,
                        left: operand(1),
                        right: operand(0),
                        operator_span: minus,
                    },
                    span,
                ),
            ],
            terminator: end(OwnedTerminatorKind::Goto(BlockId(3))),
        },
        OwnedBlock {
            merge: None,
            span,
            statements: vec![fixtures::assign(4, Rvalue::Bool(false), span)],
            terminator: end(OwnedTerminatorKind::Goto(BlockId(3))),
        },
        OwnedBlock {
            merge: Some(BoolMerge {
                operator_span: minus,
                destination: LocalId(5),
                incoming: [
                    MergeInput {
                        predecessor: BlockId(1),
                        value: operand(3),
                    },
                    MergeInput {
                        predecessor: BlockId(2),
                        value: operand(4),
                    },
                ],
                span,
            }),
            span,
            statements: vec![],
            terminator: end(OwnedTerminatorKind::ReturnScalar(operand(5))),
        },
    ]);
    (sources, p)
}

#[test]
fn raw_negation_owned_native_phi_exits_and_branch_dominance() {
    for guarded in [false, true] {
        let (sources, p) = phi_raw(guarded);
        let witness = verify_owned(p, &sources).unwrap();
        assert_eq!(
            execute::run(&witness, Some(hir::DefId(0))),
            Ok(Scalar::Bool(true))
        );
        let module = native::native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
        let label = if guarded {
            "f0_b1_g3_ok"
        } else {
            "f0_b1_i0_checked_ok"
        };
        assert!(module.contains(&format!("[ %s3, %{label} ]")), "{module}");
    }
    let (sources, mut p) = phi_raw(false);
    let span = p.functions[0].span;
    let OwnedInstruction::Scalar(Statement::Assign(a)) =
        &mut p.functions[0].blocks[2].statements[0].kind
    else {
        unreachable!()
    };
    a.value = Rvalue::CompareScalar {
        op: hir::ComparisonOp::Less,
        left: fixtures::operand(1, span),
        right: fixtures::operand(0, span),
        operator_span: span,
    };
    assert_eq!(
        verify_owned(p, &sources).unwrap_err().kind,
        OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Uninitialized))
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn raw_negation_owned_phi_uses_real_llvm() {
    for guarded in [false, true] {
        let (sources, p) = phi_raw(guarded);
        let witness = verify_owned(p, &sources).unwrap();
        let module = native::native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
        super::super::negation_raw_tests::run_native(&module, &Ok(Scalar::Bool(true)), &sources);
    }
}
