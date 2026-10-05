//! Unary checked negation witnesses built without any source front-end path.
use super::*;

fn raw(value: i32) -> (SourceMap, Program, Span) {
    let mut sources = SourceMap::new();
    let file = sources.add(
        "raw-negation-雪.ox".into(),
        "// 雪\r\nentry input - result return".into(),
    );
    let source = sources.get(file);
    let span = |word: &str| {
        let start = source.text().find(word).unwrap();
        source.span(start, start + word.len())
    };
    let entry = span("entry");
    let input = span("input");
    let minus = span("-");
    let result = span("result");
    let ret = span("return");
    let p = Program {
        functions: vec![Function {
            id: hir::DefId(0),
            span: entry,
            result: hir::Ty::I32,
            param_count: 0,
            locals: vec![input, result]
                .into_iter()
                .map(|span| LocalDecl {
                    ty: hir::Ty::I32,
                    kind: LocalKind::Temporary,
                    span,
                })
                .collect(),
            places: vec![],
            entry: BlockId(0),
            blocks: vec![BasicBlock {
                merge: None,
                span: entry,
                statements: vec![
                    Statement::Assign(Assign {
                        destination: LocalId(0),
                        value: Rvalue::I32(value),
                        span: input,
                    }),
                    Statement::Assign(Assign {
                        destination: LocalId(1),
                        value: Rvalue::CheckedNegateI32 {
                            operand: Operand {
                                local: LocalId(0),
                                span: input,
                            },
                            operator_span: minus,
                        },
                        span: result,
                    }),
                ],
                terminator: Some(Terminator {
                    kind: TerminatorKind::Return(Operand {
                        local: LocalId(1),
                        span: ret,
                    }),
                    span: ret,
                }),
            }],
        }],
    };
    (sources, p, minus)
}

fn append_cycle(p: &mut Program) {
    let span = p.functions[0].span;
    p.functions.push(Function {
        id: hir::DefId(1),
        span,
        result: hir::Ty::Unit,
        param_count: 0,
        locals: vec![],
        places: vec![],
        entry: BlockId(0),
        blocks: vec![BasicBlock {
            merge: None,
            span,
            statements: vec![],
            terminator: Some(Terminator {
                kind: TerminatorKind::Goto { target: BlockId(0) },
                span,
            }),
        }],
    });
}

#[test]
fn raw_negation_reference_boundaries_and_exact_one_assignment_fuel() {
    for (value, expected) in [
        (0, 0),
        (1, -1),
        (-1, 1),
        (i32::MAX, -i32::MAX),
        (i32::MIN + 1, i32::MAX),
    ] {
        let (sources, p, _) = raw(value);
        let f = &p.functions[0];
        // Entry+two scalar slots=3, literal=1, negation=1, return=1.
        let boundaries = [
            f.span,
            f.span,
            f.span,
            f.blocks[0].statements[0].span(),
            f.blocks[0].statements[1].span(),
            f.blocks[0].terminator.as_ref().unwrap().span,
        ];
        let verified = verify::verify(p, &sources).unwrap();
        for (fuel, span) in boundaries.into_iter().enumerate() {
            assert_eq!(
                execute::run_with_fuel(&verified, hir::DefId(0), fuel),
                Err(RunFailure::Fuel(span))
            );
        }
        assert_eq!(
            execute::run_with_fuel(&verified, hir::DefId(0), 6),
            Ok(Scalar::I32(expected))
        );
    }
    let (sources, p, minus) = raw(i32::MIN);
    let assignment = p.functions[0].blocks[0].statements[1].span();
    let verified = verify::verify(p, &sources).unwrap();
    assert_eq!(
        execute::run_with_fuel(&verified, hir::DefId(0), 4),
        Err(RunFailure::Fuel(assignment))
    );
    assert_eq!(
        execute::run_with_fuel(&verified, hir::DefId(0), 5),
        Err(RunFailure::Overflow(minus))
    );
    assert_eq!(
        RunFailure::Overflow(minus).diagnostic(&sources).code,
        "E0604"
    );
}

#[test]
fn raw_negation_verifier_rejects_types_ids_origins_and_nondominating_values() {
    for mutation in 0..10 {
        let (sources, mut p, _) = raw(7);
        let f = &mut p.functions[0];
        let expected = match mutation {
            0 | 1 => {
                let ty = if mutation == 0 {
                    hir::Ty::Bool
                } else {
                    hir::Ty::Unit
                };
                f.locals[0].ty = ty;
                f.blocks[0].statements[0].assignment_mut().value = if ty == hir::Ty::Bool {
                    Rvalue::Bool(true)
                } else {
                    Rvalue::Unit
                };
                FailureKind::TypeMismatch
            }
            2 => {
                f.locals[1].ty = hir::Ty::Bool;
                FailureKind::TypeMismatch
            }
            3 => {
                f.blocks[0].statements[1].assignment_mut().destination = LocalId(usize::MAX);
                FailureKind::InvalidLocal
            }
            4 => {
                let Rvalue::CheckedNegateI32 { operand, .. } =
                    &mut f.blocks[0].statements[1].assignment_mut().value
                else {
                    unreachable!()
                };
                operand.local = LocalId(usize::MAX);
                FailureKind::InvalidLocal
            }
            5..=7 => {
                let Rvalue::CheckedNegateI32 {
                    operand,
                    operator_span,
                } = &mut f.blocks[0].statements[1].assignment_mut().value
                else {
                    unreachable!()
                };
                let span = if mutation == 5 {
                    operator_span
                } else {
                    &mut operand.span
                };
                if mutation == 7 {
                    span.file = super::super::source::SourceFileId(usize::MAX);
                } else {
                    span.start = 4;
                    span.end = 5;
                } // Inside the UTF-8 snow character.
                FailureKind::InvalidSpan
            }
            8 => {
                f.blocks[0].statements.swap(0, 1);
                FailureKind::Uninitialized
            }
            9 => {
                let Rvalue::CheckedNegateI32 { operand, .. } =
                    &mut f.blocks[0].statements[1].assignment_mut().value
                else {
                    unreachable!()
                };
                operand.local = LocalId(1);
                FailureKind::Uninitialized
            }
            _ => unreachable!(),
        };
        assert_eq!(
            verify::verify(p, &sources).unwrap_err().kind,
            expected,
            "mutation {mutation}"
        );
    }
}

#[test]
fn raw_negation_native_checks_before_extracting_and_embeds_minus_diagnostic() {
    for guarded in [false, true] {
        let (sources, mut p, minus) = raw(i32::MIN);
        if guarded {
            append_cycle(&mut p);
        }
        let verified = verify::verify(p, &sources).unwrap();
        let module = verified
            .native_module(Some(hir::DefId(0)), &sources)
            .unwrap();
        assert!(module.contains("@llvm.ssub.with.overflow.i32(i32 0, i32 %v0)"));
        assert!(module.contains("  br i1 %overflow1, label %overflow1_error, label %checked1_ok"));
        assert!(module.contains(
            "  unreachable\nchecked1_ok:\n  %v1 = extractvalue { i32, i1 } %checked1, 0"
        ));
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
#[ignore = "requires pinned LLVM 19.1.7; explicitly run in the native CI job"]
fn raw_negation_scalar_witnesses_use_real_llvm() {
    for guarded in [false, true] {
        for value in [0, 1, -1, i32::MAX, i32::MIN + 1, i32::MIN] {
            let (sources, mut p, minus) = raw(value);
            if guarded {
                append_cycle(&mut p);
            }
            let verified = verify::verify(p, &sources).unwrap();
            let module = verified
                .native_module_with_fuel(hir::DefId(0), &sources, 6)
                .unwrap();
            let expected = if value == i32::MIN {
                Err(RunFailure::Overflow(minus))
            } else {
                Ok(Scalar::I32(-value))
            };
            assert_eq!(verified.run(Some(hir::DefId(0))), expected);
            run_native(&module, &expected, &sources);
            if guarded {
                for fuel in [3, 4, 5] {
                    let module = verified
                        .native_module_with_fuel(hir::DefId(0), &sources, fuel)
                        .unwrap();
                    run_native(
                        &module,
                        &execute::run_with_fuel(&verified, hir::DefId(0), fuel),
                        &sources,
                    );
                }
            }
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(super) fn run_native(module: &str, expected: &Result<Scalar, RunFailure>, sources: &SourceMap) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root =
        std::env::temp_dir().join(format!("oxid-unary-raw-{}-{sequence}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let scratch = Scratch(root);
    let output = scratch.0.join("program");
    crate::frontend::native::compile(module, output.to_str().unwrap()).unwrap();
    assert_eq!(&std::fs::read(&output).unwrap()[..4], b"\x7fELF");
    let result = std::process::Command::new(&output)
        .current_dir(&scratch.0)
        .env_clear()
        .env("PATH", scratch.0.join("no-tools"))
        .output()
        .unwrap();
    if let Some(evidence) = std::env::var_os("OXID_UNARY_NATIVE_EVIDENCE") {
        let evidence = std::path::PathBuf::from(evidence);
        std::fs::create_dir_all(&evidence).unwrap();
        let stem = format!("raw-unary-{}-{sequence}", std::process::id());
        std::fs::write(evidence.join(format!("{stem}.ll")), module).unwrap();
        std::fs::copy(&output, evidence.join(format!("{stem}.elf"))).unwrap();
        std::fs::write(evidence.join(format!("{stem}.stdout")), &result.stdout).unwrap();
        std::fs::write(evidence.join(format!("{stem}.stderr")), &result.stderr).unwrap();
        std::fs::write(
            evidence.join(format!("{stem}.status")),
            result.status.to_string(),
        )
        .unwrap();
    }
    match expected {
        Ok(value) => {
            assert_eq!(result.status.code(), Some(0));
            assert!(result.stderr.is_empty());
            assert_eq!(result.stdout, format!("{value}\n").as_bytes());
        }
        Err(error) => {
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            assert_eq!(
                result.stderr,
                error.diagnostic(sources).render_human(sources).as_bytes()
            );
        }
    }
}

fn phi_raw(guarded: bool) -> (SourceMap, Program) {
    let (sources, mut p, minus) = raw(7);
    let f = &mut p.functions[0];
    let span = f.span;
    let negate = f.blocks[0].statements.pop().unwrap();
    f.result = hir::Ty::Bool;
    f.locals.extend((2..6).map(|_| LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span,
    }));
    let operand = |local| Operand {
        local: LocalId(local),
        span,
    };
    let statement = |destination, value| {
        Statement::Assign(Assign {
            destination: LocalId(destination),
            value,
            span,
        })
    };
    let end = |kind| Some(Terminator { kind, span });
    f.blocks[0]
        .statements
        .push(statement(2, Rvalue::Bool(true)));
    f.blocks[0].terminator = end(TerminatorKind::Branch {
        condition: operand(2),
        then_block: BlockId(1),
        else_block: BlockId(2),
    });
    f.blocks.extend([
        BasicBlock {
            merge: None,
            span,
            statements: vec![
                negate,
                statement(
                    3,
                    Rvalue::CompareScalar {
                        op: hir::ComparisonOp::Less,
                        left: operand(1),
                        right: operand(0),
                        operator_span: minus,
                    },
                ),
            ],
            terminator: end(TerminatorKind::Goto { target: BlockId(3) }),
        },
        BasicBlock {
            merge: None,
            span,
            statements: vec![statement(4, Rvalue::Bool(false))],
            terminator: end(TerminatorKind::Goto { target: BlockId(3) }),
        },
        BasicBlock {
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
            terminator: end(TerminatorKind::Return(operand(5))),
        },
    ]);
    if guarded {
        append_cycle(&mut p);
    }
    (sources, p)
}

#[test]
fn raw_negation_scalar_native_phi_exits_and_branch_dominance() {
    for guarded in [false, true] {
        let (sources, p) = phi_raw(guarded);
        let witness = verify::verify(p, &sources).unwrap();
        assert_eq!(witness.run(Some(hir::DefId(0))), Ok(Scalar::Bool(true)));
        let module = witness
            .native_module(Some(hir::DefId(0)), &sources)
            .unwrap();
        let label = if guarded { "g1_3_ok" } else { "checked1_ok" };
        assert!(module.contains(&format!("[ %v3, %{label} ]")), "{module}");
    }
    let (sources, mut p) = phi_raw(false);
    // The other branch cannot consume the first branch's negated result.
    let span = p.functions[0].span;
    p.functions[0].blocks[2].statements[0]
        .assignment_mut()
        .value = Rvalue::CompareScalar {
        op: hir::ComparisonOp::Less,
        left: Operand {
            local: LocalId(1),
            span,
        },
        right: Operand {
            local: LocalId(0),
            span,
        },
        operator_span: span,
    };
    assert_eq!(
        verify::verify(p, &sources).unwrap_err().kind,
        FailureKind::Uninitialized
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run in the native CI job"]
fn raw_negation_scalar_phi_uses_real_llvm() {
    for guarded in [false, true] {
        let (sources, p) = phi_raw(guarded);
        let witness = verify::verify(p, &sources).unwrap();
        let module = witness
            .native_module(Some(hir::DefId(0)), &sources)
            .unwrap();
        run_native(&module, &Ok(Scalar::Bool(true)), &sources);
    }
}
