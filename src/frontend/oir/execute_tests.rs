use super::*;
use crate::frontend::{lexer, parser};

pub(super) fn compiled(text: &str) -> (SourceMap, VerifiedProgram, hir::DefId) {
    let mut sources = SourceMap::new();
    let id = sources.add("run.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let resolved = hir::resolve(source, &ast).unwrap();
    let entry = ast
        .functions
        .iter()
        .zip(&resolved.functions)
        .find(|(f, _)| &source.text()[f.name.start..f.name.end] == "main")
        .unwrap()
        .1
        .id;
    let typed = typeck::check(resolved).unwrap();
    let program = lower_and_verify(&typed, &sources).unwrap();
    (sources, program, entry)
}
fn limits(fuel: usize, frames: usize, slots: usize) -> Limits {
    Limits {
        fuel,
        frames,
        slots,
    }
}
fn constant() -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let id = sources.add(
        "raw.ox".into(),
        "entry assign return branch goto call".into(),
    );
    let source = sources.get(id);
    let entry = source.span(0, 5);
    let assign = source.span(6, 12);
    let ret = source.span(13, 19);
    (
        sources,
        Program {
            functions: vec![Function {
                id: hir::DefId(0),
                span: entry,
                result: hir::Ty::Bool,
                param_count: 0,
                locals: vec![LocalDecl {
                    ty: hir::Ty::Bool,
                    kind: LocalKind::Temporary,
                    span: assign,
                }],
                entry: BlockId(0),
                blocks: vec![BasicBlock {
                    span: entry,
                    statements: vec![Assign {
                        destination: LocalId(0),
                        value: Rvalue::Bool(true),
                        span: assign,
                    }],
                    terminator: Some(Terminator {
                        kind: TerminatorKind::Return(Operand {
                            local: LocalId(0),
                            span: ret,
                        }),
                        span: ret,
                    }),
                }],
            }],
        },
    )
}
fn code(error: RunFailure, sources: &SourceMap) -> String {
    error.diagnostic(sources).code.to_string()
}
fn recursive() -> (SourceMap, VerifiedProgram) {
    let (sources, mut raw) = constant();
    let f = &mut raw.functions[0];
    let ret = f.blocks[0].terminator.take();
    f.blocks[0].statements.clear();
    let span = sources.get(f.span.file).span(32, 36);
    f.blocks[0].terminator = Some(Terminator {
        kind: TerminatorKind::Call {
            target: hir::DefId(0),
            args: vec![],
            destination: LocalId(0),
            continuation: BlockId(1),
        },
        span,
    });
    f.blocks.push(BasicBlock {
        span,
        statements: vec![],
        terminator: ret,
    });
    {
        let verified = verify::verify(raw, &sources).unwrap();
        (sources, verified)
    }
}
#[test]
fn constant_cost_is_four_and_every_next_unperformed_origin_is_exact() {
    let (sources, raw) = constant();
    let origin = raw.functions[0].span;
    let assign = raw.functions[0].blocks[0].statements[0].span;
    let ret = raw.functions[0].blocks[0].terminator.as_ref().unwrap().span;
    let verified = verify::verify(raw, &sources).unwrap();
    for (fuel, span) in [(0, origin), (1, origin), (2, assign), (3, ret)] {
        assert_eq!(
            invoke(&verified, hir::DefId(0), &[], limits(fuel, 1, 1)),
            Err(RunFailure::Fuel(span))
        );
    }
    for _ in 0..3 {
        assert_eq!(
            invoke(&verified, hir::DefId(0), &[], limits(4, 1, 1)),
            Ok(Scalar::Bool(true))
        );
    }
    assert_eq!(
        invoke(&verified, hir::DefId(0), &[], limits(4, 0, 1)),
        Err(RunFailure::Frames(origin))
    );
    assert_eq!(
        invoke(&verified, hir::DefId(0), &[], limits(4, 1, 0)),
        Err(RunFailure::Slots(origin))
    );
}
#[test]
fn actual_source_condition_call_is_exactly_fifteen_including_all_slot_initialization() {
    let text = "fn id(x: bool) -> bool { return x; }\nfn main() -> bool { if id(true) { return false; } else { return true; } }";
    let (sources, verified, main) = compiled(text);
    // Independently counted: main frame5, true1, call4, id copy+return2,
    // branch1, chosen false+return2. The unused arm only reserves one slot.
    assert_eq!(
        invoke(&verified, main, &[], limits(15, 2, 6)),
        Ok(Scalar::Bool(false))
    );
    let error = invoke(&verified, main, &[], limits(14, 2, 6)).unwrap_err();
    let span = error.diagnostic(&sources).primary.unwrap();
    assert_eq!(&text[span.start..span.end], "return false;");
    assert!(matches!(error, RunFailure::Fuel(_)));
}
#[test]
fn branch_goto_and_backward_table_edges_each_pay_one() {
    let (sources, mut raw) = constant();
    let f = &mut raw.functions[0];
    let source = sources.get(f.span.file);
    let branch = source.span(20, 26);
    let goto = source.span(27, 31);
    let ret = f.blocks[0].terminator.take();
    f.blocks[0].terminator = Some(Terminator {
        kind: TerminatorKind::Branch {
            condition: Operand {
                local: LocalId(0),
                span: branch,
            },
            then_block: BlockId(2),
            else_block: BlockId(1),
        },
        span: branch,
    });
    f.blocks.push(BasicBlock {
        span: branch,
        statements: vec![],
        terminator: ret,
    });
    f.blocks.push(BasicBlock {
        span: goto,
        statements: vec![],
        terminator: Some(Terminator {
            kind: TerminatorKind::Goto { target: BlockId(1) },
            span: goto,
        }),
    });
    let verified = verify::verify(raw, &sources).unwrap();
    assert_eq!(
        invoke(&verified, hir::DefId(0), &[], limits(3, 1, 1)),
        Err(RunFailure::Fuel(branch))
    );
    assert_eq!(
        invoke(&verified, hir::DefId(0), &[], limits(4, 1, 1)),
        Err(RunFailure::Fuel(goto))
    );
    assert_eq!(
        invoke(&verified, hir::DefId(0), &[], limits(6, 1, 1)),
        Ok(Scalar::Bool(true))
    );
}
#[test]
fn resource_limits_are_live_exact_and_fail_in_fuel_frame_slot_order() {
    let (sources, verified) = recursive();
    for (limits, expected) in [
        (limits(6, 3, 3), "E0601"),
        (limits(100, 3, 3), "E0602"),
        (limits(100, 4, 3), "E0603"),
    ] {
        let error = invoke(&verified, hir::DefId(0), &[], limits).unwrap_err();
        let span = error.diagnostic(&sources).primary.unwrap();
        assert_eq!(&sources.get(span.file).text()[span.start..span.end], "call");
        assert_eq!(code(error, &sources), expected);
    }
    let (_, verified, entry) =
        compiled("fn id() -> bool { return true; } fn main() -> bool { return id(); }");
    // main: one call destination; id: one literal. Both full local tables are live.
    assert_eq!(
        invoke(&verified, entry, &[], limits(100, 2, 2)),
        Ok(Scalar::Bool(true))
    );
    assert!(matches!(
        invoke(&verified, entry, &[], limits(100, 1, 2)),
        Err(RunFailure::Frames(_))
    ));
    assert!(matches!(
        invoke(&verified, entry, &[], limits(100, 2, 1)),
        Err(RunFailure::Slots(_))
    ));
}
#[test]
fn returned_frames_release_slots_before_many_shallow_calls() {
    let mut text = "fn id() -> () { return; } fn main() -> () { ".to_string();
    for _ in 0..50 {
        text.push_str("id(); ");
    }
    text.push_str("return; }");
    let (_, verified, entry) = compiled(&text);
    // 50 result slots plus root unit; each helper has one unit slot.
    assert_eq!(
        invoke(&verified, entry, &[], limits(1000, 2, 52)),
        Ok(Scalar::Unit)
    );
    assert!(matches!(
        invoke(&verified, entry, &[], limits(1000, 2, 51)),
        Err(RunFailure::Slots(_))
    ));
}
#[test]
fn deep_mutual_recursion_is_iterative_and_internal_limits_cannot_raise_absolute_caps() {
    let (sources, verified, entry) = compiled("fn a() -> () { b(); return; } fn b() -> () { a(); return; } fn main() -> () { a(); return; }");
    assert_eq!(
        code(
            invoke(
                &verified,
                entry,
                &[],
                limits(usize::MAX, usize::MAX, usize::MAX)
            )
            .unwrap_err(),
            &sources
        ),
        "E0602"
    );
    assert_eq!(
        code(
            invoke(&verified, entry, &[], limits(10, 100, 100)).unwrap_err(),
            &sources
        ),
        "E0601"
    );
}
#[test]
fn exponential_shallow_call_tree_exhausts_fuel_without_exhausting_frames() {
    let mut text = "fn f0() -> () { return; }".to_string();
    for i in 1..20 {
        text.push_str(&format!(
            "fn f{i}() -> () {{ f{}(); f{}(); return; }}",
            i - 1,
            i - 1
        ));
    }
    text.push_str("fn main() -> () { f19(); return; }");
    let (sources, verified, entry) = compiled(&text);
    assert_eq!(
        code(verified.run(Some(entry)).unwrap_err(), &sources),
        "E0601"
    );
}
#[test]
fn private_invocation_checks_entry_ids_argument_arity_and_types() {
    let (sources, verified, entry) =
        compiled("fn id(x: bool, y: ()) -> bool { return x; } fn main() -> () { return; }");
    assert_eq!(
        invoke(
            &verified,
            hir::DefId(0),
            &[Scalar::Bool(false), Scalar::Unit],
            Limits::default()
        ),
        Ok(Scalar::Bool(false))
    );
    for (id, args, kind) in [
        (hir::DefId(0), vec![], FailureKind::Arity),
        (
            hir::DefId(0),
            vec![Scalar::Unit, Scalar::Unit],
            FailureKind::TypeMismatch,
        ),
        (entry, vec![Scalar::Unit], FailureKind::Arity),
        (hir::DefId(99), vec![], FailureKind::InvalidFunctionId),
    ] {
        let error = invoke(&verified, id, &args, Limits::default()).unwrap_err();
        assert!(matches!(&error, RunFailure::Internal(e) if e.kind == kind));
        assert_eq!(code(error, &sources), "E0500");
    }
    assert!(matches!(
        verified.run(Some(hir::DefId(0))),
        Err(RunFailure::Entry(Some(_)))
    ));
    assert_eq!(verified.run(None), Err(RunFailure::Entry(None)));
    assert_eq!(
        code(verified.run(Some(hir::DefId(99))).unwrap_err(), &sources),
        "E0500"
    );
}
#[test]
fn malformed_raw_oir_has_no_witness_and_fault_diagnostics_filter_invalid_origins() {
    let (sources, mut raw) = constant();
    raw.functions[0].blocks[0].statements.clear();
    assert!(matches!(
        verify::verify(raw.clone(), &sources),
        Err(OirFailure {
            kind: FailureKind::Uninitialized,
            ..
        })
    ));
    // Test-only fault injection: production cannot construct this witness.
    let forged = VerifiedProgram { program: raw };
    assert_eq!(
        code(
            invoke(&forged, hir::DefId(0), &[], Limits::default()).unwrap_err(),
            &sources
        ),
        "E0500"
    );
    let (sources, mut raw) = constant();
    raw.functions[0].id = hir::DefId(2);
    assert!(verify::verify(raw, &sources).is_err());
    let (sources, raw) = constant();
    let mut span = raw.functions[0].span;
    span.start = usize::MAX;
    let error = internal(FailureKind::Uninitialized, Some(span));
    assert!(error.diagnostic(&sources).primary.is_none());
    assert!(error
        .diagnostic(&sources)
        .render_json(&sources)
        .contains("\"primary\":null"));
    assert!(error
        .diagnostic(&sources)
        .render_human(&sources)
        .contains("E0500"));
}
#[test]
fn checked_accounting_overflow_is_internal_and_preflight_does_not_allocate() {
    let (_, raw) = constant();
    let origin = raw.functions[0].span;
    assert!(matches!(
        add(usize::MAX, 1, origin),
        Err(RunFailure::Internal(OirFailure {
            kind: FailureKind::Accounting,
            ..
        }))
    ));
    let mut fuel = 10;
    assert!(matches!(
        preflight(&mut fuel, 1, 0, usize::MAX, 1, Limits::default(), origin),
        Err(RunFailure::Internal(OirFailure {
            kind: FailureKind::Accounting,
            ..
        }))
    ));
}
#[test]
fn scalar_frame_and_argument_storage_layout_is_measured() {
    eprintln!("reference runner layout: scalar={} optional-slot={} frame={} resume={} argument-scratch-max={}", std::mem::size_of::<Scalar>(), std::mem::size_of::<Option<Scalar>>(), std::mem::size_of::<Frame>(), std::mem::size_of::<Resume>(), parser::MAX_PARAMS * std::mem::size_of::<Scalar>());
    // Closed i32/bool/unit slots are finite; this is a layout regression guard,
    // not a portable ABI or a total process-memory/OOM guarantee.
    assert!(std::mem::size_of::<Option<Scalar>>() <= 16);
}

#[test]
fn production_caps_count_exact_frame_and_full_unused_slot_storage() {
    let (sources, recursive) = recursive();
    let mut entered = 0;
    let error = execute(
        &recursive,
        hir::DefId(0),
        &[],
        Limits::default(),
        &mut |event| {
            if matches!(event, Event::Enter(_)) {
                entered += 1;
            }
        },
    )
    .unwrap_err();
    assert_eq!(code(error, &sources), "E0602");
    assert_eq!(entered, 1024);
    // A raw verified recursive function with 50,000 full-table slots fits four
    // live frames exactly, even though only slot 0 is dynamically initialized.
    let mut raw = recursive.program.clone();
    let declaration = raw.functions[0].locals[0].clone();
    raw.functions[0].locals.resize(50_000, declaration);
    let verified = verify::verify(raw.clone(), &sources).unwrap();
    let mut entered = 0;
    let error = execute(
        &verified,
        hir::DefId(0),
        &[],
        Limits::default(),
        &mut |event| {
            if matches!(event, Event::Enter(_)) {
                entered += 1;
            }
        },
    )
    .unwrap_err();
    assert_eq!(code(error, &sources), "E0603");
    assert_eq!(entered, 4);
    let extra = raw.functions[0].locals[0].clone();
    raw.functions[0].locals.push(extra);
    let verified = verify::verify(raw, &sources).unwrap();
    let mut entered = 0;
    let error = execute(
        &verified,
        hir::DefId(0),
        &[],
        Limits::default(),
        &mut |event| {
            if matches!(event, Event::Enter(_)) {
                entered += 1;
            }
        },
    )
    .unwrap_err();
    assert_eq!(code(error, &sources), "E0603");
    assert_eq!(entered, 3);
}
#[test]
fn maximum_source_arguments_are_copied_in_order_and_allocation_is_fully_charged() {
    let params = (0..256)
        .map(|i| format!("p{i}: bool"))
        .collect::<Vec<_>>()
        .join(", ");
    let args = (0..256)
        .map(|i| if i == 255 { "true" } else { "false" })
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!(
        "fn many({params}) -> bool {{ return p255; }} fn main() -> bool {{ return many({args}); }}"
    );
    let (_, verified, entry) = compiled(&text);
    // root258 + literals256 + call514 + parameter-copy1 + callee-return1 + root-return1
    assert_eq!(
        invoke(&verified, entry, &[], limits(1031, 2, 514)),
        Ok(Scalar::Bool(true))
    );
    assert!(matches!(
        invoke(&verified, entry, &[], limits(1030, 2, 514)),
        Err(RunFailure::Fuel(_))
    ));
    assert!(matches!(
        invoke(&verified, entry, &[], limits(1031, 2, 513)),
        Err(RunFailure::Slots(_))
    ));
}
#[test]
fn verified_origins_are_not_reinterpreted_as_entry_names() {
    let (sources, mut raw) = constant();
    // All raw origins are valid but the source does not declare ANY function.
    raw.functions[0].span = sources.get(raw.functions[0].span.file).span(6, 12);
    let verified = verify::verify(raw, &sources).unwrap();
    assert_eq!(verified.run(None), Err(RunFailure::Entry(None)));
    assert_eq!(verified.run(Some(hir::DefId(0))), Ok(Scalar::Bool(true)));
}

#[test]
fn i32_constants_have_identical_fuel_charges_and_exact_failure_origins() {
    let text = "fn main() -> i32 { return - /* é */ 2147483648; }";
    let (sources, verified, entry) = compiled(text);
    // One root slot: allocation2 + constant1 + return1, independent of magnitude/sign.
    assert_eq!(
        invoke(&verified, entry, &[], limits(4, 1, 1)),
        Ok(Scalar::I32(i32::MIN))
    );
    for (fuel, origin) in [
        (0, "main"),
        (1, "main"),
        (2, "- /* é */ 2147483648"),
        (3, "return - /* é */ 2147483648;"),
    ] {
        let error = invoke(&verified, entry, &[], limits(fuel, 1, 1)).unwrap_err();
        let RunFailure::Fuel(span) = error else {
            panic!("fuel")
        };
        let source = sources.get(span.file);
        assert_eq!(&source.text()[span.start..span.end], origin);
    }
    assert!(matches!(
        invoke(&verified, entry, &[], limits(4, 0, 0)),
        Err(RunFailure::Frames(_))
    ));
    assert!(matches!(
        invoke(&verified, entry, &[], limits(4, 1, 0)),
        Err(RunFailure::Slots(_))
    ));
}
#[test]
fn i32_parameters_are_checked_and_recursive_frames_remain_isolated() {
    let (_, verified, _) = compiled("fn choose(x: i32, c: bool) -> i32 { if c { choose(2147483647, false); return x; } return x; } fn main() -> i32 { return choose(-2147483648, true); }");
    for _ in 0..3 {
        assert_eq!(
            invoke(
                &verified,
                hir::DefId(0),
                &[Scalar::I32(i32::MIN), Scalar::Bool(true)],
                Limits::default()
            ),
            Ok(Scalar::I32(i32::MIN))
        );
    }
    assert!(matches!(
        invoke(
            &verified,
            hir::DefId(0),
            &[Scalar::Bool(false), Scalar::Bool(true)],
            Limits::default()
        ),
        Err(RunFailure::Internal(OirFailure {
            kind: FailureKind::TypeMismatch,
            ..
        }))
    ));
}
#[test]
fn i32_live_slot_and_frame_caps_count_complete_tables_unchanged() {
    let (sources, recursive, entry) = compiled("fn main() -> i32 { return main(); }");
    for (locals, expected_entries, expected_code) in [
        (1, 1024, "E0602"),
        (50_000, 4, "E0603"),
        (50_001, 3, "E0603"),
    ] {
        let mut raw = recursive.program.clone();
        let decl = raw.functions[0].locals[0].clone();
        raw.functions[0].locals.resize(locals, decl);
        let verified = verify::verify(raw, &sources).unwrap();
        let mut entries = 0;
        let error = execute(&verified, entry, &[], Limits::default(), &mut |event| {
            if matches!(event, Event::Enter(_)) {
                entries += 1;
            }
        })
        .unwrap_err();
        assert_eq!(entries, expected_entries);
        assert_eq!(code(error, &sources), expected_code);
    }
}
#[test]
fn i32_maximum_argument_scratch_preserves_values_order_and_charges() {
    let params = (0..256)
        .map(|i| format!("p{i}: i32"))
        .collect::<Vec<_>>()
        .join(",");
    let args = (0..256)
        .map(|i| {
            if i == 255 {
                "-2147483648"
            } else {
                "2147483647"
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let (_, verified, entry) = compiled(&format!(
        "fn many({params}) -> i32 {{ return p255; }} fn main() -> i32 {{ return many({args}); }}"
    ));
    // Same 1031 abstract work and 514 live slots as the bool fixture.
    assert_eq!(
        invoke(&verified, entry, &[], limits(1031, 2, 514)),
        Ok(Scalar::I32(i32::MIN))
    );
    assert!(matches!(
        invoke(&verified, entry, &[], limits(1030, 2, 514)),
        Err(RunFailure::Fuel(_))
    ));
    assert!(matches!(
        invoke(&verified, entry, &[], limits(1031, 2, 513)),
        Err(RunFailure::Slots(_))
    ));
}

#[test]
fn checked_arithmetic_costs_one_and_fuel_precedes_overflow_without_writing() {
    for (expr, expected) in [("1 + 2", Ok(Scalar::I32(3))), ("2147483647 + 1", Err(()))] {
        let text = format!("fn main() -> i32 {{ return {expr}; }}");
        let (sources, program, entry) = compiled(&text);
        let function = &program.program.functions[entry.0];
        assert_eq!(function.locals.len(), 3);
        let binary = &function.blocks[0].statements[2];
        let Rvalue::CheckedI32 { operator_span, .. } = binary.value else {
            panic!("binary")
        };
        let end = function.blocks[0].terminator.as_ref().unwrap().span;
        assert_eq!(
            invoke(&program, entry, &[], limits(6, 1, 3)),
            Err(RunFailure::Fuel(binary.span))
        );
        for _ in 0..2 {
            if let Ok(value) = expected {
                assert_eq!(
                    invoke(&program, entry, &[], limits(7, 1, 3)),
                    Err(RunFailure::Fuel(end))
                );
                assert_eq!(invoke(&program, entry, &[], limits(8, 1, 3)), Ok(value));
            } else {
                assert_eq!(
                    invoke(&program, entry, &[], limits(7, 1, 3)),
                    Err(RunFailure::Overflow(operator_span))
                );
                assert_eq!(
                    code(
                        invoke(&program, entry, &[], limits(8, 1, 3)).unwrap_err(),
                        &sources
                    ),
                    "E0604"
                );
            }
        }
        assert!(matches!(
            invoke(&program, entry, &[], limits(8, 1, 2)),
            Err(RunFailure::Slots(_))
        ));
    }
}

#[test]
fn arithmetic_calls_are_completed_in_order_exactly_once() {
    let (_, program, entry) = compiled("fn left() -> i32 { return 2; } fn right() -> i32 { return 3; } fn main() -> i32 { return left() + right() * left(); }");
    let mut events = Vec::new();
    let result = execute(&program, entry, &[], Limits::default(), &mut |event| {
        events.push(event)
    });
    assert_eq!(result, Ok(Scalar::I32(8)));
    assert_eq!(
        events,
        vec![
            Event::Enter(hir::DefId(2)),
            Event::Enter(hir::DefId(0)),
            Event::Return(hir::DefId(0), Scalar::I32(2)),
            Event::Enter(hir::DefId(1)),
            Event::Return(hir::DefId(1), Scalar::I32(3)),
            Event::Enter(hir::DefId(0)),
            Event::Return(hir::DefId(0), Scalar::I32(2)),
            Event::Return(hir::DefId(2), Scalar::I32(8)),
        ]
    );
}
