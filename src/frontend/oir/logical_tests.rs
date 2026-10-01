use super::*;

#[test]
fn explicit_bool_merge_admits_branch_local_values_at_the_join() {
    let mut sources = SourceMap::new();
    let id = sources.add(
        "logic.ox".into(),
        "fn main() -> bool { return false && true; }".into(),
    );
    let source = sources.get(id);
    let ast = crate::frontend::parser::parse(source, crate::frontend::lexer::lex(source).unwrap())
        .unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    let program = lower_and_verify(&typed, &sources).unwrap();
    let entry = hir::DefId(0);
    assert_eq!(program.run(Some(entry)).unwrap(), Scalar::Bool(false));
}

// Hand-authored raw diamond: each edge supplies a different branch-local slot.
fn diamond() -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("raw-merge.ox".into(), "雪 xy".into());
    let span = sources.get(id).span(4, 5);
    let op = |i| Operand {
        local: LocalId(i),
        span,
    };
    let block = |statements, kind| BasicBlock {
        merge: None,
        span,
        statements,
        terminator: Some(Terminator { kind, span }),
    };
    let assign = |destination, value| {
        Statement::Assign(Assign {
            destination: LocalId(destination),
            value,
            span,
        })
    };
    let mut blocks = vec![
        block(
            vec![],
            TerminatorKind::Branch {
                condition: op(0),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(
            vec![assign(1, Rvalue::Bool(true))],
            TerminatorKind::Goto { target: BlockId(3) },
        ),
        block(
            vec![assign(2, Rvalue::Bool(false))],
            TerminatorKind::Goto { target: BlockId(3) },
        ),
        block(vec![], TerminatorKind::Return(op(3))),
    ];
    blocks[3].merge = Some(BoolMerge {
        destination: LocalId(3),
        incoming: [
            MergeInput {
                predecessor: BlockId(1),
                value: op(1),
            },
            MergeInput {
                predecessor: BlockId(2),
                value: op(2),
            },
        ],
        span,
        operator_span: span,
    });
    let locals = (0..4)
        .map(|i| LocalDecl {
            ty: hir::Ty::Bool,
            kind: if i == 0 {
                LocalKind::Parameter
            } else {
                LocalKind::Temporary
            },
            span,
        })
        .collect();
    (
        sources,
        Program {
            functions: vec![Function {
                places: vec![],
                id: hir::DefId(0),
                span,
                result: hir::Ty::Bool,
                param_count: 1,
                locals,
                entry: BlockId(0),
                blocks,
            }],
        },
    )
}
fn merge(p: &mut Program) -> &mut BoolMerge {
    p.functions[0].blocks[3].merge.as_mut().unwrap()
}
fn rejects(p: Program, s: &SourceMap, kind: FailureKind) {
    assert_eq!(verify::verify(p, s).unwrap_err().kind, kind);
}
#[test]
fn merge_requires_exactly_two_distinct_real_predecessor_edges_and_no_entry_merge() {
    let (s, p) = diamond();
    verify::verify(p, &s).unwrap();
    for case in 0..6 {
        let (s, mut p) = diamond();
        match case {
            0 => merge(&mut p).incoming[1].predecessor = BlockId(1),
            1 => merge(&mut p).incoming[0].predecessor = BlockId(0),
            2 => merge(&mut p).incoming[0].predecessor = BlockId(usize::MAX),
            3 => {
                p.functions[0].blocks[1].terminator.as_mut().unwrap().kind =
                    TerminatorKind::Branch {
                        condition: merge(&mut p).incoming[0].value,
                        then_block: BlockId(3),
                        else_block: BlockId(3),
                    }
            }
            4 => {
                let m = p.functions[0].blocks[3].merge.take();
                p.functions[0].blocks[0].merge = m;
            }
            _ => {
                p.functions[0].blocks[2].terminator.as_mut().unwrap().kind =
                    TerminatorKind::Return(merge(&mut p).incoming[1].value)
            }
        }
        rejects(
            p,
            &s,
            if case == 2 {
                FailureKind::InvalidBlock
            } else {
                FailureKind::InvalidMerge
            },
        );
    }
}
#[test]
fn merge_global_definitions_and_each_incoming_read_remain_strict() {
    for side in 0..2 {
        let (s, mut p) = diamond();
        merge(&mut p).incoming[side].value.local = LocalId(2 - side);
        rejects(p, &s, FailureKind::Uninitialized);
        let (s, mut p) = diamond();
        merge(&mut p).incoming[side].value.local = LocalId(3);
        rejects(p, &s, FailureKind::Uninitialized);
        let (s, mut p) = diamond();
        p.functions[0].blocks[side + 1].statements.clear();
        rejects(p, &s, FailureKind::Uninitialized);
    }
    for dest in 0..3 {
        let (s, mut p) = diamond();
        merge(&mut p).destination = LocalId(dest);
        rejects(p, &s, FailureKind::AlreadyInitialized);
    }
    let (s, mut p) = diamond();
    let m = merge(&mut p).clone();
    p.functions[0].blocks[3]
        .statements
        .push(Statement::Assign(Assign {
            destination: m.destination,
            value: Rvalue::Bool(true),
            span: m.span,
        }));
    rejects(p, &s, FailureKind::AlreadyInitialized);
    let (s, mut p) = diamond();
    let m = merge(&mut p).clone();
    p.functions[0].locals.push(LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span: m.span,
    });
    p.functions[0].blocks[3]
        .statements
        .push(Statement::Assign(Assign {
            destination: LocalId(4),
            value: Rvalue::NotBool {
                operand: Operand {
                    local: m.destination,
                    span: m.span,
                },
                operator_span: m.span,
            },
            span: m.span,
        }));
    verify::verify(p, &s).unwrap(); // entry definition precedes the first ordinary assignment
}
#[test]
fn merge_checks_bool_types_all_ids_and_all_origins_before_cfg_analysis() {
    for ty in [hir::Ty::I32, hir::Ty::Unit] {
        let (s, mut p) = diamond();
        p.functions[0].locals[3].ty = ty;
        rejects(p, &s, FailureKind::TypeMismatch);
        for side in 0..2 {
            let (s, mut p) = diamond();
            p.functions[0].locals[side + 1].ty = ty;
            p.functions[0].blocks[side + 1].statements[0]
                .assignment_mut()
                .value = if ty == hir::Ty::I32 {
                Rvalue::I32(1)
            } else {
                Rvalue::Unit
            };
            rejects(p, &s, FailureKind::TypeMismatch);
        }
    }
    for field in 0..3 {
        let (s, mut p) = diamond();
        let m = merge(&mut p);
        if field == 0 {
            m.destination = LocalId(usize::MAX);
        } else {
            m.incoming[field - 1].value.local = LocalId(usize::MAX);
        }
        rejects(p, &s, FailureKind::InvalidLocal);
    }
    for field in 0..4 {
        for invalid in [(1, 2), (usize::MAX, usize::MAX)] {
            let (s, mut p) = diamond();
            let m = merge(&mut p);
            let origin = match field {
                0 => &mut m.span,
                1 => &mut m.operator_span,
                _ => &mut m.incoming[field - 2].value.span,
            };
            origin.start = invalid.0;
            origin.end = invalid.1;
            rejects(p, &s, FailureKind::InvalidSpan);
        }
    }
}
#[test]
fn merge_call_results_are_available_only_on_their_normal_incoming_edge() {
    let (s, mut base) = diamond();
    let f = &mut base.functions[0];
    let span = f.span;
    f.blocks[1].statements.clear();
    f.blocks[1].terminator.as_mut().unwrap().kind = TerminatorKind::Call {
        target: hir::DefId(0),
        args: vec![Operand {
            local: LocalId(0),
            span,
        }],
        destination: LocalId(1),
        continuation: BlockId(3),
    };
    verify::verify(base.clone(), &s).unwrap();
    let mut p = base.clone();
    merge(&mut p).incoming[1].value.local = LocalId(1);
    rejects(p, &s, FailureKind::Uninitialized);
    let mut p = base.clone();
    if let TerminatorKind::Call { args, .. } =
        &mut p.functions[0].blocks[1].terminator.as_mut().unwrap().kind
    {
        args[0].local = LocalId(1);
    }
    rejects(p, &s, FailureKind::Uninitialized);
    let mut p = base;
    p.functions[0].blocks[1]
        .statements
        .push(Statement::Assign(Assign {
            destination: LocalId(2),
            value: Rvalue::Copy(Operand {
                local: LocalId(1),
                span,
            }),
            span,
        }));
    p.functions[0].blocks[2].statements.clear();
    rejects(p, &s, FailureKind::Uninitialized);
}
#[test]
fn merges_count_toward_assignment_budget_before_definition_traversal() {
    for (ordinary, expected) in [
        (MAX_ASSIGNMENTS - 1, FailureKind::AlreadyInitialized),
        (MAX_ASSIGNMENTS, FailureKind::ResourceLimit("assignments")),
    ] {
        let (s, mut p) = diamond();
        let template = p.functions[0].blocks[1].statements[0].clone();
        p.functions[0].blocks[1].statements = vec![template; ordinary];
        p.functions[0].blocks[2].statements.clear();
        rejects(p, &s, expected);
    }
}
#[test]
fn logical_operator_and_merge_origins_survive_unicode_crlf_lowering() {
    let text = "// 雪\r\nfn main() -> bool { return !false && true || false; }";
    let mut sources = SourceMap::new();
    let id = sources.add("origins.ox".into(), text.into());
    let source = sources.get(id);
    let ast = crate::frontend::parser::parse(source, crate::frontend::lexer::lex(source).unwrap())
        .unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    let raw = lower::lower(&typed).unwrap();
    let mut ops = vec![];
    for b in &raw.functions[0].blocks {
        if let Some(m) = &b.merge {
            ops.push(&text[m.operator_span.start..m.operator_span.end]);
            assert!(text[m.span.start..m.span.end].starts_with("!false"));
        }
        for a in &b.statements {
            if let Rvalue::NotBool { operator_span, .. } = a.assignment().value {
                ops.push(&text[operator_span.start..operator_span.end]);
            }
        }
    }
    ops.sort();
    assert_eq!(ops, vec!["!", "&&", "||"]);
    verify::verify(raw, &sources).unwrap();
}

#[test]
fn bool_negation_independently_checks_types_ids_origins_and_read_order() {
    for case in 0..8 {
        let (s, mut p) = diamond();
        let f = &mut p.functions[0];
        let span = f.span;
        // Keep a single block and one ordinary bool destination, independent of merges.
        f.blocks.truncate(1);
        f.locals.truncate(2);
        f.blocks[0].statements = vec![Statement::Assign(Assign {
            destination: LocalId(1),
            value: Rvalue::NotBool {
                operand: Operand {
                    local: LocalId(0),
                    span,
                },
                operator_span: span,
            },
            span,
        })];
        f.blocks[0].terminator.as_mut().unwrap().kind = TerminatorKind::Return(Operand {
            local: LocalId(1),
            span,
        });
        let a = f.blocks[0].statements[0].assignment_mut();
        let Rvalue::NotBool {
            operand,
            operator_span,
        } = &mut a.value
        else {
            panic!()
        };
        let expected = match case {
            0 => {
                f.locals[0].ty = hir::Ty::I32;
                FailureKind::TypeMismatch
            }
            1 => {
                f.locals[0].ty = hir::Ty::Unit;
                FailureKind::TypeMismatch
            }
            2 => {
                f.locals[1].ty = hir::Ty::I32;
                FailureKind::TypeMismatch
            }
            3 => {
                operand.local = LocalId(usize::MAX);
                FailureKind::InvalidLocal
            }
            4 => {
                a.destination = LocalId(usize::MAX);
                FailureKind::InvalidLocal
            }
            5 => {
                operand.local = LocalId(1);
                FailureKind::Uninitialized
            }
            6 => {
                operator_span.start = 1;
                operator_span.end = 2;
                FailureKind::InvalidSpan
            }
            _ => {
                operand.span.start = 1;
                operand.span.end = 2;
                FailureKind::InvalidSpan
            }
        };
        rejects(p, &s, expected);
    }
}

fn direct_call_merge() -> (SourceMap, Program) {
    let (sources, mut raw) = diamond();
    let span = raw.functions[0].span;
    let mut callee = raw.functions[0].clone();
    callee.id = hir::DefId(1);
    callee.param_count = 0;
    callee.locals = vec![LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span,
    }];
    callee.blocks.truncate(1);
    callee.blocks[0].statements = vec![Statement::Assign(Assign {
        destination: LocalId(0),
        value: Rvalue::Bool(true),
        span,
    })];
    callee.blocks[0].terminator.as_mut().unwrap().kind = TerminatorKind::Return(Operand {
        local: LocalId(0),
        span,
    });
    raw.functions.push(callee);
    let f = &mut raw.functions[0];
    f.param_count = 0;
    f.locals[0].kind = LocalKind::Temporary;
    f.blocks[0].statements = vec![Statement::Assign(Assign {
        destination: LocalId(0),
        value: Rvalue::Bool(true),
        span,
    })];
    f.blocks[1].statements.clear();
    f.blocks[1].terminator.as_mut().unwrap().kind = TerminatorKind::Call {
        target: hir::DefId(1),
        args: vec![],
        destination: LocalId(1),
        continuation: BlockId(3),
    };
    (sources, raw)
}
#[test]
fn direct_call_return_into_merge_executes_the_selected_incoming_value() {
    let (sources, raw) = direct_call_merge();
    for chosen in [true, false] {
        let mut p = raw.clone();
        p.functions[0].blocks[0].statements[0]
            .assignment_mut()
            .value = Rvalue::Bool(chosen);
        let verified = verify::verify(p, &sources).unwrap();
        assert_eq!(verified.run(Some(hir::DefId(0))), Ok(Scalar::Bool(chosen)));
        let module = verified
            .native_module(Some(hir::DefId(0)), &sources)
            .unwrap();
        assert!(module.contains("%v3 = phi i1 [ %v1, %b1 ], [ %v2, %b2 ]"));
    }
}

// This is mandatory in the pinned Linux native job, explicitly ignored only
// by ordinary toolchain-free host suites. Missing LLVM in an explicit run fails.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run in the native CI job"]
fn raw_bool_merge_uses_real_llvm() {
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("oxid-raw-merge-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let scratch = Scratch(root);
    let (sources, base) = direct_call_merge();
    let mut artifacts = 0;
    for chosen in [false, true] {
        for split in [false, true] {
            for reversed in [false, true] {
                let mut raw = base.clone();
                let f = &mut raw.functions[0];
                let span = f.span;
                f.blocks[0].statements[0].assignment_mut().value = Rvalue::Bool(chosen);
                if split {
                    for _ in 0..3 {
                        f.locals.push(LocalDecl {
                            ty: hir::Ty::I32,
                            kind: LocalKind::Temporary,
                            span,
                        });
                    }
                    f.blocks[1].statements = vec![
                        Statement::Assign(Assign {
                            destination: LocalId(4),
                            value: Rvalue::I32(1),
                            span,
                        }),
                        Statement::Assign(Assign {
                            destination: LocalId(5),
                            value: Rvalue::I32(2),
                            span,
                        }),
                        Statement::Assign(Assign {
                            destination: LocalId(6),
                            value: Rvalue::CheckedI32 {
                                op: hir::ArithmeticOp::Add,
                                left: Operand {
                                    local: LocalId(4),
                                    span,
                                },
                                right: Operand {
                                    local: LocalId(5),
                                    span,
                                },
                                operator_span: span,
                            },
                            span,
                        }),
                    ];
                }
                if reversed {
                    super::cfg_tests::permute(&mut raw, &[3, 2, 1, 0]);
                }
                let program = verify::verify(raw, &sources).unwrap();
                assert_eq!(program.run(Some(hir::DefId(0))), Ok(Scalar::Bool(chosen)));
                let module = program
                    .native_module(Some(hir::DefId(0)), &sources)
                    .unwrap();
                let output = scratch.0.join(format!("artifact-{artifacts}"));
                crate::frontend::native::compile(&module, output.to_str().unwrap()).unwrap();
                let result = std::process::Command::new(&output)
                    .env_clear()
                    .env("PATH", scratch.0.join("no-tools"))
                    .output()
                    .unwrap();
                assert_eq!(result.status.code(), Some(0));
                assert!(result.stderr.is_empty());
                assert_eq!(result.stdout, format!("{chosen}\n").as_bytes());
                artifacts += 1;
                if split && !reversed {
                    let wrong = module.replace("[ %v1, %checked6_ok ]", "[ %v1, %b1 ]");
                    assert_ne!(wrong, module);
                    let invalid = scratch.0.join(format!("wrong-predecessor-{artifacts}"));
                    let error = crate::frontend::native::compile(&wrong, invalid.to_str().unwrap())
                        .unwrap_err();
                    assert_eq!(error.code, "E0701");
                    assert!(error.message.contains("PHI"), "{}", error.message);
                    assert!(!invalid.exists());
                }
            }
        }
    }
    assert_eq!(artifacts, 8);
}
