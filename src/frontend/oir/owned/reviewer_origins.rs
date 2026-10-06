//! Independent Unit4B raw metadata, authority, and resource qualification.
use super::*;
use std::cell::Cell;
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}
pub(super) fn note_alloc() {
    let _ = COUNTING.try_with(|on| {
        if on.get() {
            let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        }
    });
}
fn counted<T>(f: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COUNTING.with(|x| x.set(false));
        }
    }
    ALLOCS.with(|n| n.set(0));
    COUNTING.with(|x| x.set(true));
    let guard = Reset;
    let value = f();
    drop(guard);
    (value, ALLOCS.with(Cell::get))
}
fn span(file: super::super::super::source::SourceFileId, at: usize) -> Span {
    Span {
        file,
        start: at,
        end: at + 1,
    }
}
fn stmt(kind: OwnedInstruction, s: Span) -> OwnedStatement {
    OwnedStatement {
        kind,
        span: s,
        diagnostic_origins: None,
    }
}
fn end(kind: OwnedTerminatorKind, s: Span) -> Option<OwnedTerminator> {
    Some(OwnedTerminator {
        kind,
        span: s,
        diagnostic_origins: None,
    })
}
fn setup(active: bool) -> (SourceMap, RawOwnedProgram, Span) {
    let mut sm = SourceMap::new();
    let file = sm.add("review.ox".into(), format!("{}éx", "x".repeat(1024)));
    let s = span(file, 0);
    let f = RawOwnedFunction {
        id: hir::DefId(0),
        span: s,
        result: ValueTy::Scalar(hir::Ty::Unit),
        parameters: if active {
            vec![ParameterBinding::Owned(OwnerPlaceId(0))]
        } else {
            vec![]
        },
        locals: vec![LocalDecl {
            ty: hir::Ty::Unit,
            kind: LocalKind::Temporary,
            span: s,
        }],
        places: vec![],
        owners: if active {
            vec![OwnerDecl {
                aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0)))
                    .unwrap(),
                kind: OwnerKind::Parameter { position: 0 },
                span: s,
            }]
        } else {
            vec![]
        },
        references: vec![],
        calls: vec![],
        loans: vec![],
        matches: Vec::new(),
        entry: BlockId(0),
        blocks: vec![OwnedBlock {
            span: s,
            merge: None,
            statements: vec![stmt(
                OwnedInstruction::Scalar(Statement::Assign(Assign {
                    destination: LocalId(0),
                    value: Rvalue::Unit,
                    span: s,
                })),
                s,
            )],
            terminator: end(
                OwnedTerminatorKind::ReturnScalar(Operand {
                    local: LocalId(0),
                    span: s,
                }),
                s,
            ),
        }],
    };
    (
        sm,
        RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![],
            records: vec![RawRecordDecl {
                id: RecordId(0),
                span: s,
                fields: vec![],
            }],
            functions: vec![f],
        },
        s,
    )
}
#[test]
fn heldout_invalid_origins_precede_reachability_for_all_span_failure_classes() {
    let mut cases = 0;
    for active in [false, true] {
        for unreachable in [false, true] {
            for term in [false, true] {
                for cause in [false, true] {
                    for invalid_kind in 0..4 {
                        let (sm, mut p, s) = setup(active);
                        let invalid = match invalid_kind {
                            0 => Span {
                                file: super::super::super::source::SourceFileId(17),
                                ..s
                            },
                            1 => Span {
                                start: 100,
                                end: 99,
                                ..s
                            },
                            2 => Span { end: 1028, ..s },
                            _ => Span {
                                start: 1025,
                                end: 1026,
                                ..s
                            },
                        };
                        if unreachable {
                            let b = p.functions[0].blocks[0].clone();
                            p.functions[0].blocks.push(b);
                        }
                        let b = p.functions[0].blocks.last_mut().unwrap();
                        let o = Some(DiagnosticOrigins {
                            primary: if cause { s } else { invalid },
                            cause: if cause { invalid } else { s },
                        });
                        if term {
                            b.terminator.as_mut().unwrap().diagnostic_origins = o;
                        } else {
                            b.statements[0].diagnostic_origins = o;
                        }
                        let e = verify_owned(p, &sm).unwrap_err();
                        assert_eq!(e.kind, OwnedFailureKind::Malformed(Malformed::Span));
                        assert_eq!(e.primary.get(), Some(invalid));
                        assert!(e.context.is_none());
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 64);
}
#[test]
fn heldout_valid_origins_cannot_whitewash_invalid_operation_spans() {
    for active in [false, true] {
        for term in [false, true] {
            let (sm, mut p, s) = setup(active);
            let bad = Span {
                start: 100,
                end: 99,
                ..s
            };
            let origins = Some(DiagnosticOrigins {
                primary: s,
                cause: s,
            });
            let b = &mut p.functions[0].blocks[0];
            if term {
                let e = b.terminator.as_mut().unwrap();
                e.span = bad;
                e.diagnostic_origins = origins;
            } else {
                b.statements[0].span = bad;
                b.statements[0].diagnostic_origins = origins;
            }
            let e = verify_owned(p, &sm).unwrap_err();
            assert_eq!(e.kind, OwnedFailureKind::Malformed(Malformed::Span));
            assert_eq!(e.primary.get(), Some(bad));
        }
    }
}
#[test]
fn heldout_same_site_state_selection_ignores_primary_and_cause_order() {
    for primary in [1, 300, 700] {
        for cause in [2, 400, 900] {
            let (sm, mut p, s) = setup(true);
            let f = &mut p.functions[0];
            f.locals.push(LocalDecl {
                ty: hir::Ty::Bool,
                kind: LocalKind::Temporary,
                span: s,
            });
            f.blocks[0].statements.push(stmt(
                OwnedInstruction::Scalar(Statement::Assign(Assign {
                    destination: LocalId(1),
                    value: Rvalue::Bool(true),
                    span: s,
                })),
                s,
            ));
            f.blocks[0].terminator = end(
                OwnedTerminatorKind::Branch {
                    condition: Operand {
                        local: LocalId(1),
                        span: s,
                    },
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
                s,
            );
            for kind in [
                OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
                OwnedInstruction::Discard(OwnerPlaceId(0)),
            ] {
                f.blocks.push(OwnedBlock {
                    span: s,
                    merge: None,
                    statements: vec![stmt(kind, span(s.file, 50))],
                    terminator: end(OwnedTerminatorKind::Goto(BlockId(3)), s),
                });
            }
            let mut denied = stmt(
                OwnedInstruction::Discard(OwnerPlaceId(0)),
                span(s.file, 100),
            );
            denied.diagnostic_origins = Some(DiagnosticOrigins {
                primary: span(s.file, primary),
                cause: span(s.file, cause),
            });
            f.blocks.push(OwnedBlock {
                span: s,
                merge: None,
                statements: vec![denied],
                terminator: end(
                    OwnedTerminatorKind::ReturnScalar(Operand {
                        local: LocalId(0),
                        span: s,
                    }),
                    s,
                ),
            });
            let e = verify_owned(p, &sm).unwrap_err();
            assert_eq!(e.kind, OwnedFailureKind::Ownership(Violation::Unavailable));
            assert_eq!(e.primary.get(), Some(span(s.file, primary)));
            assert_eq!(e.related.get(), None);
            let facts = e.context.unwrap().facts();
            assert_eq!(facts.state, flow::ObservedState::Dead);
            assert_eq!(facts.role, flow::DeniedRole::SourceConsume);
        }
    }
}
#[test]
fn heldout_loan_cleanup_keeps_original_role_under_origin_swaps() {
    for primary in [1, 800, 1600] {
        for cause in [2, 1000, 2000] {
            let (sm, mut p, _) = consumer_fixtures::shared_read();
            let f = &mut p.functions[0];
            let s = f.span;
            let denied_span = span(s.file, 30);
            f.blocks[1].statements.clear();
            let mut denied = stmt(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), denied_span);
            denied.diagnostic_origins = Some(DiagnosticOrigins {
                primary: span(s.file, primary),
                cause: s,
            });
            f.blocks[0].statements.push(denied);
            for i in &mut f.blocks[0].statements {
                if matches!(i.kind, OwnedInstruction::PrepareBorrow { .. }) {
                    i.diagnostic_origins = Some(DiagnosticOrigins {
                        primary: span(s.file, 2500),
                        cause: span(s.file, cause),
                    });
                }
            }
            let e = verify_owned(p, &sm).unwrap_err();
            assert_eq!(e.kind, OwnedFailureKind::Ownership(Violation::LoanConflict));
            assert_eq!(e.primary.get(), Some(span(s.file, primary)));
            assert_eq!(e.related.get(), Some(span(s.file, cause)));
            let f = e.context.unwrap().facts();
            assert_eq!(
                (f.operation, f.role, f.state),
                (
                    flow::DeniedOperation::StorageEnd,
                    flow::DeniedRole::Storage,
                    flow::ObservedState::NotObserved
                )
            );
        }
    }
}
#[test]
fn heldout_independent_meter_totals_and_inclusive_program_caps() {
    let (sm, mut p, s) = setup(true);
    // Active: O=1,P=1,V=1,I=1,B=1; N=7; W=36*7=252, G=2 =>260.
    // Metadata: 48 owner-site +16 parameter +56*(1+1)=176.
    for i in &mut p.functions[0].blocks[0].statements {
        i.diagnostic_origins = Some(DiagnosticOrigins {
            primary: s,
            cause: s,
        });
    }
    p.functions[0].blocks[0]
        .terminator
        .as_mut()
        .unwrap()
        .diagnostic_origins = Some(DiagnosticOrigins {
        primary: s,
        cause: s,
    });
    let (_, mut scalar, _) = setup(false);
    scalar.functions[0].id = hir::DefId(1);
    scalar.functions[0].blocks[0].statements[0].diagnostic_origins = Some(DiagnosticOrigins {
        primary: s,
        cause: s,
    });
    p.functions.push(scalar.functions.remove(0));
    let (u, allocs) = counted(|| budget::preflight(&p, budget::Limits::DEFAULT));
    let u = u.unwrap();
    assert_eq!(allocs, 0);
    assert_eq!(
        (
            u.owners,
            u.expanded_events,
            u.work,
            u.metadata_bytes,
            u.scratch_bytes
        ),
        (
            1,
            1,
            262,
            320 + 2 * std::mem::size_of::<Vec<MatchDecl>>(),
            33
        )
    );
    let d = Declarations::check(&p.records, &sm).unwrap();
    let mut meter = budget::Meter {
        visits: 0,
        ceiling: u.work,
    };
    let checked = shape::check(&p.functions[0], &p, &d, &sm, &mut meter).unwrap();
    assert_eq!(meter.visits, 4);
    shape::check(&p.functions[1], &p, &d, &sm, &mut meter).unwrap();
    assert_eq!(meter.visits, 6);
    let checked_again = shape::check(&p.functions[0], &p, &d, &sm, &mut meter).unwrap();
    assert_eq!(meter.visits, 10);
    flow::check(&p.functions[0], &checked_again, &mut meter).unwrap();
    assert_eq!(meter.visits, 13);
    drop(checked);
    let exact = budget::Limits {
        owners: 1,
        events: 1,
        work: 262,
        metadata: 296
            + std::mem::size_of::<Vec<RawEnumDecl>>()
            + 2 * std::mem::size_of::<Vec<MatchDecl>>(),
        scratch: 33,
    };
    assert!(verify_with_limits(clone_raw(&p), &sm, exact).is_ok());
    for (limits, label) in [
        (budget::Limits { owners: 0, ..exact }, "owners"),
        (
            budget::Limits { events: 0, ..exact },
            "expanded ownership events",
        ),
        (budget::Limits { work: 261, ..exact }, "ownership work"),
        (
            budget::Limits {
                metadata: 295
                    + std::mem::size_of::<Vec<RawEnumDecl>>()
                    + 2 * std::mem::size_of::<Vec<MatchDecl>>(),
                ..exact
            },
            "ownership metadata",
        ),
        (
            budget::Limits {
                scratch: 32,
                ..exact
            },
            "ownership scratch",
        ),
    ] {
        let raw = clone_raw(&p);
        let (result, allocs) = counted(|| verify_with_limits(raw, &sm, limits));
        assert_eq!(allocs, 0, "{label}");
        assert_eq!(result.unwrap_err().kind, OwnedFailureKind::Resource(label));
    }
}
#[test]
fn heldout_oversized_raw_preflight_allocates_nothing_and_ignores_false_source_counts() {
    let (sm, mut p, s) = setup(false);
    let f = &mut p.functions[0];
    f.owners = vec![
        OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
            kind: OwnerKind::Local { mutable: false },
            span: s
        };
        1000
    ];
    f.blocks = vec![
        OwnedBlock {
            span: s,
            merge: None,
            statements: vec![],
            terminator: None
        };
        30000
    ];
    let mut false_counts = budget::ProgramCounts::default();
    budget::account_function(
        budget::FunctionCounts::default(),
        budget::Limits::DEFAULT,
        &mut false_counts,
    )
    .unwrap();
    assert_eq!(
        false_counts.usage(),
        OwnershipUsage {
            metadata_bytes: std::mem::size_of::<Vec<MatchDecl>>(),
            ..OwnershipUsage::default()
        }
    );
    let (result, allocations) = counted(|| verify_owned(p, &sm));
    assert_eq!(allocations, 0);
    assert_eq!(
        result.unwrap_err().kind,
        OwnedFailureKind::Resource("ownership work")
    );
}
#[test]
fn heldout_origins_preserve_all_frozen_reference_fuel_and_native_modules() {
    type Fixture = (SourceMap, RawOwnedProgram, consumer_fixtures::Schedule);
    let fixtures: Vec<Fixture> = vec![
        consumer_fixtures::empty_record(),
        consumer_fixtures::owned_relay(),
        consumer_fixtures::shared_read(),
        consumer_fixtures::shared_children(false),
        consumer_fixtures::shared_children(true),
        consumer_fixtures::mixed_relay(0),
        consumer_fixtures::mixed_relay(1),
        consumer_fixtures::mixed_relay(2),
        consumer_fixtures::owner_loop(),
        consumer_fixtures::replacement(false),
        consumer_fixtures::replacement(true),
        consumer_fixtures::distinct_owned_results(),
        consumer_fixtures::later_argument_loop(),
    ];
    let mut runs = 0;
    for (sm, raw, schedule) in fixtures {
        let mut varied = clone_raw(&raw);
        for f in &mut varied.functions {
            for b in &mut f.blocks {
                for i in &mut b.statements {
                    i.diagnostic_origins = Some(DiagnosticOrigins {
                        primary: f.span,
                        cause: b.span,
                    });
                }
                let e = b.terminator.as_mut().unwrap();
                e.diagnostic_origins = Some(DiagnosticOrigins {
                    primary: f.span,
                    cause: b.span,
                });
            }
        }
        let plain = verify_owned(raw, &sm).unwrap();
        let varied = verify_owned(varied, &sm).unwrap();
        assert_eq!(
            native::native_module(&plain, Some(schedule.entry), &sm).unwrap(),
            native::native_module(&varied, Some(schedule.entry), &sm).unwrap()
        );
        for fuel in 0..=schedule.fuel() {
            let limits = execute::Limits {
                fuel,
                ..execute::Limits::default()
            };
            let p = execute::run_limits(&plain, Some(schedule.entry), limits);
            let v = execute::run_limits(&varied, Some(schedule.entry), limits);
            assert_eq!(p, v);
            if let Some(at) = schedule.failure(fuel) {
                assert_eq!(
                    v,
                    Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(at)))
                );
            } else {
                assert_eq!(v.unwrap(), schedule.result);
            }
            runs += 1;
        }
    }
    println!("independent origin invariance:13 fixtures, {runs} budget comparisons,26 emitted native modules");
}

fn clone_raw(p: &RawOwnedProgram) -> RawOwnedProgram {
    RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: p.enums.clone(),
        records: p
            .records
            .iter()
            .map(|r| RawRecordDecl {
                id: r.id,
                span: r.span,
                fields: r
                    .fields
                    .iter()
                    .map(|f| RawFieldDecl {
                        id: f.id,
                        ty: f.ty,
                        span: f.span,
                    })
                    .collect(),
            })
            .collect(),
        functions: p.functions.clone(),
    }
}

#[test]
fn heldout_causes_exclude_invalid_and_terminated_predecessors() {
    let mut cases = 0;
    for mode in 0..3 {
        for a in [3, 350, 800] {
            for b in [2, 400, 900] {
                for reversed in [false, true] {
                    let (sm, mut p, s) = setup(true);
                    let f = &mut p.functions[0];
                    f.locals.push(LocalDecl {
                        ty: hir::Ty::Bool,
                        kind: LocalKind::Temporary,
                        span: s,
                    });
                    f.blocks[0].statements.push(stmt(
                        OwnedInstruction::Scalar(Statement::Assign(Assign {
                            destination: LocalId(1),
                            value: Rvalue::Bool(true),
                            span: s,
                        })),
                        s,
                    ));
                    f.blocks[0].terminator = end(
                        OwnedTerminatorKind::Branch {
                            condition: Operand {
                                local: LocalId(1),
                                span: s,
                            },
                            then_block: BlockId(if reversed { 2 } else { 1 }),
                            else_block: BlockId(if reversed { 1 } else { 2 }),
                        },
                        s,
                    );
                    for (arm, cause) in [(1, a), (2, b)] {
                        let mut moved = stmt(
                            OwnedInstruction::Discard(OwnerPlaceId(0)),
                            span(s.file, 200 + arm),
                        );
                        moved.diagnostic_origins = Some(DiagnosticOrigins {
                            primary: span(s.file, 600 + arm),
                            cause: span(s.file, cause),
                        });
                        let mut statements = vec![moved];
                        if arm == 1 && mode == 1 {
                            statements.push(stmt(
                                OwnedInstruction::Discard(OwnerPlaceId(0)),
                                span(s.file, 500),
                            ));
                        }
                        f.blocks.push(OwnedBlock {
                            span: s,
                            merge: None,
                            statements,
                            terminator: end(
                                if arm == 1 && mode == 2 {
                                    OwnedTerminatorKind::ReturnScalar(Operand {
                                        local: LocalId(0),
                                        span: s,
                                    })
                                } else {
                                    OwnedTerminatorKind::Goto(BlockId(3))
                                },
                                s,
                            ),
                        });
                    }
                    let mut denied =
                        stmt(OwnedInstruction::Discard(OwnerPlaceId(0)), span(s.file, 10));
                    denied.diagnostic_origins = Some(DiagnosticOrigins {
                        primary: span(s.file, 950),
                        cause: span(s.file, 951),
                    });
                    f.blocks.push(OwnedBlock {
                        span: s,
                        merge: None,
                        statements: vec![denied],
                        terminator: end(
                            OwnedTerminatorKind::ReturnScalar(Operand {
                                local: LocalId(0),
                                span: s,
                            }),
                            s,
                        ),
                    });
                    let e = verify_owned(p, &sm).unwrap_err();
                    assert_eq!(e.primary.get(), Some(span(s.file, 950)));
                    assert_eq!(
                        e.related.get(),
                        Some(span(s.file, if mode == 0 { a.min(b) } else { b }))
                    );
                    assert_eq!(e.context.unwrap().facts().state, flow::ObservedState::Moved);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 54);
}

// Independent integration controls only.
pub(super) fn integration_counted<T>(action: impl FnOnce() -> T) -> (T, usize) {
    counted(action)
}
pub(super) fn integration_enabled() -> bool {
    COUNTING.with(Cell::get)
}
