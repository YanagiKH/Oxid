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
            5 | 6 | 7 => {
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
