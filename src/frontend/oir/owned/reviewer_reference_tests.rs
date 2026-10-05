// Reviewer-owned independent executable invariants; no producer cost/model helpers.
fn rv_span(s: Span, n: usize) -> Span {
    Span {
        file: s.file,
        start: n * 3,
        end: n * 3 + 1,
    }
}
fn rv_env() -> (SourceMap, Span) {
    let mut sources = SourceMap::new();
    let file = sources.add("reviewer-owned.ox".into(), "abc\n".repeat(8192));
    (
        sources,
        Span {
            file,
            start: 0,
            end: 1,
        },
    )
}
fn rv_op(i: usize, span: Span) -> Operand {
    Operand {
        local: LocalId(i),
        span,
    }
}
fn rv_local(ty: hir::Ty, span: Span) -> LocalDecl {
    LocalDecl {
        ty,
        kind: LocalKind::Temporary,
        span,
    }
}
fn rv_ins(kind: OwnedInstruction, span: Span) -> OwnedStatement {
    OwnedStatement {
        diagnostic_origins: None,
        kind,
        span,
    }
}
fn rv_assign(i: usize, value: Rvalue, span: Span) -> OwnedStatement {
    rv_ins(
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            destination: LocalId(i),
            value,
            span,
        })),
        span,
    )
}
fn rv_block(statements: Vec<OwnedStatement>, kind: OwnedTerminatorKind, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: Some(OwnedTerminator {
            diagnostic_origins: None,
            kind,
            span,
        }),
    }
}
fn rv_fn(id: usize, ty: ValueTy, span: Span) -> RawOwnedFunction {
    RawOwnedFunction {
        id: hir::DefId(id),
        span,
        result: ty,
        parameters: vec![],
        locals: vec![],
        places: vec![],
        owners: vec![],
        references: vec![],
        calls: vec![],
        loans: vec![],
        entry: BlockId(0),
        blocks: vec![],
    }
}
fn rv_record(types: &[hir::Ty], span: Span) -> RawRecordDecl {
    RawRecordDecl {
        id: RecordId(0),
        span,
        fields: types
            .iter()
            .enumerate()
            .map(|(index, &ty)| RawFieldDecl {
                id: FieldId {
                    record: RecordId(0),
                    index,
                },
                ty: ParameterTy::Value(ValueTy::Scalar(ty)),
                span,
            })
            .collect(),
    }
}
fn rv_field(index: usize) -> FieldId {
    FieldId {
        record: RecordId(0),
        index,
    }
}
fn rv_owner(kind: OwnerKind, span: Span) -> OwnerDecl {
    OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
        kind,
        span,
    }
}
fn rv_machine<'p, 'w>(plan: &'p ExecutionPlan<'w>) -> Machine<'p, 'w> {
    let mut m = Machine {
        plan,
        frames: vec![],
        limits: Limits {
            frames: 8,
            ..Limits::default()
        },
        fuel: plan::MAX_FUEL,
        next_activation: 1,
        live_slots: 0,
        live_cells: 0,
        live_bytes: 0,
        header_bytes: 8 * size_of::<Frame>(),
        events: vec![],
        observer: array_observe::Observer::default(),
    };
    let root = Frame::allocate(plan, hir::DefId(0), 1, None).unwrap();
    m.install(root);
    m
}
fn rv_run(raw: RawOwnedProgram, sources: &SourceMap) -> Scalar {
    let w =
        verified::verify_owned(raw, sources).expect("reviewer raw must obtain a genuine witness");
    run(&w, Some(hir::DefId(0))).unwrap()
}

#[test]
fn reviewer_allocation_failures_have_activation_origin() {
    let (sources, s) = rv_env();
    let mut f = rv_fn(0, ValueTy::Scalar(hir::Ty::Unit), s);
    f.locals.push(rv_local(hir::Ty::Unit, s));
    f.blocks.push(rv_block(
        vec![rv_assign(0, Rvalue::Unit, rv_span(s, 1))],
        OwnedTerminatorKind::ReturnScalar(rv_op(0, s)),
        rv_span(s, 2),
    ));
    let w = verified::verify_owned(
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        },
        &sources,
    )
    .unwrap();
    let plan = ExecutionPlan::build(&w).unwrap();
    let result = plan::fail_allocation_after(0, || {
        execute_plan(&plan, hir::DefId(0), Limits::default(), None)
    });
    assert_eq!(
        result,
        Err(OwnedRunFailure::Resource(plan::AdmissionFailure {
            name: "injected owned allocation failure",
            span: Some(s)
        }))
    );
}

fn rv_mixed(permutation: &[usize], a: i32, b: i32, flag: bool) -> (SourceMap, RawOwnedProgram) {
    let (sources, s) = rv_env();
    let mut f = rv_fn(0, ValueTy::Scalar(hir::Ty::I32), s);
    f.locals = [
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::Unit,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
    ]
    .into_iter()
    .map(|ty| rv_local(ty, s))
    .collect();
    f.owners = vec![
        rv_owner(OwnerKind::Local { mutable: true }, s),
        rv_owner(OwnerKind::Temporary, s),
        rv_owner(OwnerKind::Temporary, s),
    ];
    let mut ins = vec![
        rv_assign(0, Rvalue::Bool(flag), rv_span(s, 1)),
        rv_assign(1, Rvalue::I32(a), rv_span(s, 2)),
        rv_assign(2, Rvalue::Unit, rv_span(s, 3)),
        rv_assign(3, Rvalue::I32(b), rv_span(s, 4)),
    ];
    for i in 0..3 {
        ins.push(rv_ins(
            OwnedInstruction::StorageLive(OwnerPlaceId(i)),
            rv_span(s, 5 + i),
        ));
    }
    ins.push(rv_ins(
        OwnedInstruction::Construct {
            destination: OwnerPlaceId(0),
            fields: permutation
                .iter()
                .map(|&i| (rv_field(i), rv_op(i, s)))
                .collect(),
        },
        rv_span(s, 8),
    ));
    ins.push(rv_ins(
        OwnedInstruction::ReadField {
            destination: LocalId(4),
            base: AccessBase::Owner(OwnerPlaceId(0)),
            field: rv_field(1),
        },
        rv_span(s, 9),
    ));
    ins.push(rv_ins(
        OwnedInstruction::MoveInitialize {
            destination: OwnerPlaceId(1),
            source: OwnerPlaceId(0),
        },
        rv_span(s, 10),
    ));
    ins.push(rv_ins(
        OwnedInstruction::Construct {
            destination: OwnerPlaceId(2),
            fields: vec![
                (rv_field(3), rv_op(1, s)),
                (rv_field(2), rv_op(2, s)),
                (rv_field(1), rv_op(3, s)),
                (rv_field(0), rv_op(0, s)),
            ],
        },
        rv_span(s, 11),
    ));
    ins.push(rv_ins(
        OwnedInstruction::Replace {
            destination: OwnerPlaceId(0),
            source: OwnerPlaceId(2),
        },
        rv_span(s, 12),
    ));
    ins.push(rv_ins(
        OwnedInstruction::ReadField {
            destination: LocalId(5),
            base: AccessBase::Owner(OwnerPlaceId(0)),
            field: rv_field(1),
        },
        rv_span(s, 13),
    ));
    ins.push(rv_ins(
        OwnedInstruction::ReadField {
            destination: LocalId(6),
            base: AccessBase::Owner(OwnerPlaceId(1)),
            field: rv_field(3),
        },
        rv_span(s, 14),
    ));
    ins.push(rv_assign(
        7,
        Rvalue::CheckedI32 {
            op: hir::ArithmeticOp::Add,
            left: rv_op(4, s),
            right: rv_op(5, s),
            operator_span: rv_span(s, 15),
        },
        rv_span(s, 15),
    ));
    ins.push(rv_assign(
        8,
        Rvalue::CheckedI32 {
            op: hir::ArithmeticOp::Add,
            left: rv_op(7, s),
            right: rv_op(6, s),
            operator_span: rv_span(s, 16),
        },
        rv_span(s, 16),
    ));
    for i in 0..3 {
        ins.push(rv_ins(
            OwnedInstruction::StorageEnd(OwnerPlaceId(i)),
            rv_span(s, 17 + i),
        ));
    }
    f.blocks.push(rv_block(
        ins,
        OwnedTerminatorKind::ReturnScalar(rv_op(8, s)),
        rv_span(s, 20),
    ));
    (
        sources,
        RawOwnedProgram {
            records: vec![rv_record(
                &[hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit, hir::Ty::I32],
                s,
            )],
            functions: vec![f],
        },
    )
}
#[test]
fn reviewer_192_heldout_mixed_field_orders_and_whole_reinitialization() {
    let mut count = 0;
    for a in [-13, 0, 31, 777] {
        for flag in [false, true] {
            for p0 in 0..4 {
                for p1 in 0..4 {
                    for p2 in 0..4 {
                        for p3 in 0..4 {
                            let mut sorted = vec![p0, p1, p2, p3];
                            sorted.sort();
                            if sorted != [0, 1, 2, 3] {
                                continue;
                            }
                            let (sources, raw) = rv_mixed(&[p0, p1, p2, p3], a, -23, flag);
                            assert_eq!(rv_run(raw, &sources), Scalar::I32(a - 46));
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 192);
}

fn rv_alias(
    roots: [usize; 2],
    modes: [BorrowKind; 2],
    a: i32,
    b: i32,
) -> (SourceMap, RawOwnedProgram, Vec<(Span, usize)>) {
    let (sources, s) = rv_env();
    let mut f = rv_fn(0, ValueTy::Scalar(hir::Ty::I32), s);
    f.locals = (0..3).map(|_| rv_local(hir::Ty::I32, s)).collect();
    f.owners = (0..2)
        .map(|_| rv_owner(OwnerKind::Local { mutable: true }, s))
        .collect();
    f.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![
            ArgumentSlot::Borrow(LoanId(0)),
            ArgumentSlot::Borrow(LoanId(1)),
        ],
        result: CallResult::Scalar(LocalId(2)),
        parent: None,
        span: rv_span(s, 7),
    }];
    f.loans = (0..2)
        .map(|i| LoanDecl {
            projection: Vec::new(),
            call: CallSiteId(0),
            argument: i,
            authority: AccessBase::Owner(OwnerPlaceId(roots[i])),
            kind: modes[i],
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            span: rv_span(s, 8 + i),
        })
        .collect();
    let ins = vec![
        rv_assign(0, Rvalue::I32(a), rv_span(s, 1)),
        rv_assign(1, Rvalue::I32(b), rv_span(s, 2)),
        rv_ins(
            OwnedInstruction::StorageLive(OwnerPlaceId(0)),
            rv_span(s, 3),
        ),
        rv_ins(
            OwnedInstruction::StorageLive(OwnerPlaceId(1)),
            rv_span(s, 4),
        ),
        rv_ins(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(0),
                fields: vec![(rv_field(0), rv_op(0, s))],
            },
            rv_span(s, 5),
        ),
        rv_ins(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(1),
                fields: vec![(rv_field(0), rv_op(1, s))],
            },
            rv_span(s, 6),
        ),
        rv_ins(OwnedInstruction::OpenCall(CallSiteId(0)), rv_span(s, 7)),
        rv_ins(
            OwnedInstruction::PrepareBorrow {
                call: CallSiteId(0),
                argument: 0,
                loan: LoanId(0),
            },
            rv_span(s, 8),
        ),
        rv_ins(
            OwnedInstruction::PrepareBorrow {
                call: CallSiteId(0),
                argument: 1,
                loan: LoanId(1),
            },
            rv_span(s, 9),
        ),
    ];
    f.blocks = vec![
        rv_block(
            ins,
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            rv_span(s, 10),
        ),
        rv_block(
            vec![
                rv_ins(
                    OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
                    rv_span(s, 11),
                ),
                rv_ins(
                    OwnedInstruction::StorageEnd(OwnerPlaceId(1)),
                    rv_span(s, 12),
                ),
            ],
            OwnedTerminatorKind::ReturnScalar(rv_op(2, s)),
            rv_span(s, 13),
        ),
    ];
    let mut g = rv_fn(1, ValueTy::Scalar(hir::Ty::I32), rv_span(s, 100));
    g.locals = (0..3).map(|_| rv_local(hir::Ty::I32, s)).collect();
    g.parameters = (0..2)
        .map(|i| ParameterBinding::Reference(ReferenceParamId(i)))
        .collect();
    g.references = (0..2)
        .map(|i| ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            kind: modes[i],
            position: i,
            span: s,
        })
        .collect();
    g.blocks = vec![rv_block(
        vec![
            rv_ins(
                OwnedInstruction::ReadField {
                    destination: LocalId(0),
                    base: AccessBase::Parameter(ReferenceParamId(0)),
                    field: rv_field(0),
                },
                rv_span(s, 101),
            ),
            rv_ins(
                OwnedInstruction::ReadField {
                    destination: LocalId(1),
                    base: AccessBase::Parameter(ReferenceParamId(1)),
                    field: rv_field(0),
                },
                rv_span(s, 102),
            ),
            rv_assign(
                2,
                Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Add,
                    left: rv_op(0, s),
                    right: rv_op(1, s),
                    operator_span: rv_span(s, 103),
                },
                rv_span(s, 103),
            ),
        ],
        OwnedTerminatorKind::ReturnScalar(rv_op(2, s)),
        rv_span(s, 104),
    )];
    let schedule = [
        (0, 42),
        (1, 1),
        (2, 1),
        (3, 1),
        (4, 1),
        (5, 2),
        (6, 2),
        (7, 1),
        (8, 1),
        (9, 1),
        (10, 23),
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 3),
        (11, 2),
        (12, 2),
        (13, 6),
    ]
    .into_iter()
    .map(|(i, c)| (rv_span(s, i), c))
    .collect();
    (
        sources,
        RawOwnedProgram {
            records: vec![rv_record(&[hir::Ty::I32], s)],
            functions: vec![f, g],
        },
        schedule,
    )
}
#[test]
fn reviewer_50_heldout_reference_alias_capability_calls() {
    let mut count = 0;
    for a in [-999, -1, 0, 7, 222] {
        for r0 in 0..2 {
            for r1 in 0..2 {
                for m0 in [BorrowKind::Shared, BorrowKind::Exclusive] {
                    for m1 in [BorrowKind::Shared, BorrowKind::Exclusive] {
                        if r0 == r1 && (m0 == BorrowKind::Exclusive || m1 == BorrowKind::Exclusive)
                        {
                            continue;
                        }
                        let (sources, raw, _) = rv_alias([r0, r1], [m0, m1], a, 17);
                        let values = [a, 17];
                        assert_eq!(rv_run(raw, &sources), Scalar::I32(values[r0] + values[r1]));
                        count += 1;
                    }
                }
            }
        }
    }
    assert_eq!(count, 50);
}
#[test]
fn reviewer_shared_alias_all_92_fuel_boundaries_and_no_early_release() {
    let (sources, raw, schedule) = rv_alias([0, 0], [BorrowKind::Shared; 2], 17, 23);
    assert_eq!(schedule.iter().map(|v| v.1).sum::<usize>(), 92);
    let w = verified::verify_owned(raw, &sources).unwrap();
    for fuel in 0..=92 {
        let mut remaining = fuel;
        let mut paid = 0;
        for &(_, cost) in &schedule {
            if remaining < cost {
                break;
            }
            remaining -= cost;
            paid += 1;
        }
        let mut events = vec![];
        let result = run_observed(
            &w,
            hir::DefId(0),
            Limits {
                fuel,
                ..Limits::default()
            },
            &mut events,
        );
        if paid < schedule.len() {
            assert_eq!(
                result,
                Err(OwnedRunFailure::Scalar(RunFailure::Fuel(schedule[paid].0)))
            );
        } else {
            assert_eq!(result, Ok(Scalar::I32(34)));
        }
        let charges: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let Event::Charge(s, c) = e {
                    Some((*s, *c))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(charges, schedule[..paid]);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Release(_)))
                .count(),
            if fuel >= 82 { 2 } else { 0 }
        );
    }
}
#[test]
fn reviewer_direct_provenance_epoch_and_failed_charge_faults() {
    let (sources, raw, _) = rv_alias([0, 0], [BorrowKind::Shared; 2], 17, 23);
    let s = raw.functions[0].span;
    let w = verified::verify_owned(raw, &sources).unwrap();
    let p = ExecutionPlan::build(&w).unwrap();
    let mut m = rv_machine(&p);
    for ins in &w.functions()[0].blocks[0].statements {
        m.statement(0, &ins.kind, plan::instruction_span(ins))
            .unwrap();
    }
    let h = m.handle(0, LoanId(0), s).unwrap();
    assert!(m.validate_handle(h, Access::Read, s).is_ok());
    for which in 0..9 {
        let mut stale = h;
        match which {
            0 => stale.root.frame = 99,
            1 => stale.root.activation += 1,
            2 => stale.root.owner = 99,
            3 => stale.root.generation += 1,
            4 => stale.permission.frame = 99,
            5 => stale.permission.activation += 1,
            6 => stale.permission.loan = 99,
            7 => stale.permission.instance += 1,
            _ => stale.root.owner = 1,
        };
        assert!(
            m.validate_handle(stale, Access::Read, s).is_err(),
            "fault {which}"
        );
    }
    let initial = m.frames[0].owners.clone();
    let loans = m.frames[0].loans.clone();
    m.fuel = 0;
    m.frames[0].next = 6;
    let before = m.frames[0].payload.clone();
    assert!(matches!(
        m.execute(),
        Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_)))
    ));
    assert_eq!(m.frames[0].owners, initial);
    assert_eq!(m.frames[0].loans, loans);
    assert_eq!(m.frames[0].payload, before);
    m.release(h.permission, s).unwrap();
    assert!(m.validate_handle(h, Access::Read, s).is_err());
    m.acquire(0, LoanId(0), s).unwrap();
    let fresh = m.handle(0, LoanId(0), s).unwrap();
    assert_ne!(fresh.permission.instance, h.permission.instance);
    assert!(m.validate_handle(h, Access::Read, s).is_err());
    m.release(fresh.permission, s).unwrap();
    m.frames[0].loans[0].instance = u64::MAX;
    let owners = m.frames[0].owners.clone();
    assert!(m.acquire(0, LoanId(0), s).is_err());
    assert_eq!(m.frames[0].owners, owners);
    m.frames[0].loans[0].instance = 0;
    let h1 = m.handle(0, LoanId(1), s).unwrap();
    m.release(h1.permission, s).unwrap();
    m.frames[0].owners[0].generation = u64::MAX;
    let owners = m.frames[0].owners.clone();
    assert!(m
        .statement(0, &OwnedInstruction::Discard(OwnerPlaceId(0)), s)
        .is_err());
    assert_eq!(m.frames[0].owners, owners);
}
#[test]
fn reviewer_activation_resource_order_and_requested_bytes_boundaries() {
    let (sources, raw, _) = rv_alias([0, 1], [BorrowKind::Exclusive; 2], 17, 23);
    let s = raw.functions[0].span;
    let w = verified::verify_owned(raw, &sources).unwrap();
    let p = ExecutionPlan::build(&w).unwrap();
    // Independent census: root S=3,A=2,B=8,O=2,R=0,L=2,C=1; child S=3,R=2.
    assert_eq!(
        p.function(hir::DefId(0)).usage().reference_bytes,
        5 * size_of::<Option<Scalar>>() + 8 + 64 + 192 + 16
    );
    assert_eq!(
        p.function(hir::DefId(1)).usage().reference_bytes,
        3 * size_of::<Option<Scalar>>() + 128
    );
    let exact = 2 * size_of::<Frame>()
        + size_of::<Scalar>()
        + (5 * size_of::<Option<Scalar>>() + 280)
        + (3 * size_of::<Option<Scalar>>() + 128);
    for (slots, cells, bytes, expected) in [
        (5, 60, exact, "slots"),
        (6, 59, exact, "cells"),
        (6, 60, exact - 1, "bytes"),
        (6, 60, exact, "ok"),
    ] {
        let result = execute_plan(
            &p,
            hir::DefId(0),
            Limits {
                frames: 2,
                slots,
                cells,
                bytes,
                ..Limits::default()
            },
            None,
        );
        match expected {
            "slots" => assert!(matches!(
                result,
                Err(OwnedRunFailure::Scalar(RunFailure::Slots(_)))
            )),
            "cells" => assert!(matches!(
                result,
                Err(OwnedRunFailure::Resource(plan::AdmissionFailure {
                    name: "live expanded cells",
                    ..
                }))
            )),
            "bytes" => assert!(matches!(
                result,
                Err(OwnedRunFailure::Resource(plan::AdmissionFailure {
                    name: "live requested bytes",
                    ..
                }))
            )),
            _ => assert_eq!(result, Ok(Scalar::I32(40))),
        }
    }
    let mut m = rv_machine(&p);
    m.limits = Limits {
        fuel: 0,
        frames: 0,
        slots: 0,
        cells: 0,
        bytes: 0,
    };
    m.fuel = 0;
    assert_eq!(
        m.activation_preflight(hir::DefId(1), 1, s),
        Err(OwnedRunFailure::Scalar(RunFailure::Fuel(s)))
    );
    m.fuel = 2;
    assert_eq!(
        m.activation_preflight(hir::DefId(1), 1, s),
        Err(OwnedRunFailure::Scalar(RunFailure::Frames(s)))
    );
    assert_eq!(m.fuel, 1);
}

fn rv_loop(n: i32) -> (SourceMap, RawOwnedProgram) {
    let (sources, s) = rv_env();
    let mut f = rv_fn(0, ValueTy::Scalar(hir::Ty::I32), s);
    f.locals = (0..10)
        .map(|i| rv_local(if i == 4 { hir::Ty::Bool } else { hir::Ty::I32 }, s))
        .collect();
    f.places = (0..2)
        .map(|_| PlaceDecl {
            ty: hir::Ty::I32,
            span: s,
        })
        .collect();
    let place = |i| Place {
        id: PlaceId(i),
        span: s,
    };
    f.owners = vec![rv_owner(OwnerKind::Local { mutable: true }, s)];
    f.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
        result: CallResult::Scalar(LocalId(5)),
        parent: None,
        span: s,
    }];
    f.loans = vec![LoanDecl {
        projection: Vec::new(),
        call: CallSiteId(0),
        argument: 0,
        authority: AccessBase::Owner(OwnerPlaceId(0)),
        kind: BorrowKind::Shared,
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        span: rv_span(s, 13),
    }];
    f.blocks = vec![
        rv_block(
            vec![
                rv_assign(0, Rvalue::I32(0), rv_span(s, 1)),
                rv_assign(1, Rvalue::I32(1), rv_span(s, 2)),
                rv_assign(2, Rvalue::I32(n), rv_span(s, 3)),
                rv_ins(
                    OwnedInstruction::Scalar(Statement::Initialize {
                        place: place(0),
                        value: rv_op(0, s),
                        span: rv_span(s, 4),
                    }),
                    rv_span(s, 40),
                ),
                rv_ins(
                    OwnedInstruction::Scalar(Statement::Initialize {
                        place: place(1),
                        value: rv_op(0, s),
                        span: rv_span(s, 5),
                    }),
                    rv_span(s, 50),
                ),
            ],
            OwnedTerminatorKind::Goto(BlockId(1)),
            rv_span(s, 6),
        ),
        rv_block(
            vec![
                rv_assign(3, Rvalue::Load(place(0)), rv_span(s, 7)),
                rv_assign(
                    4,
                    Rvalue::CompareScalar {
                        op: hir::ComparisonOp::Less,
                        left: rv_op(3, s),
                        right: rv_op(2, s),
                        operator_span: rv_span(s, 8),
                    },
                    rv_span(s, 8),
                ),
            ],
            OwnedTerminatorKind::Branch {
                condition: rv_op(4, s),
                then_block: BlockId(2),
                else_block: BlockId(4),
            },
            rv_span(s, 9),
        ),
        rv_block(
            vec![
                rv_ins(
                    OwnedInstruction::StorageLive(OwnerPlaceId(0)),
                    rv_span(s, 10),
                ),
                rv_ins(
                    OwnedInstruction::Construct {
                        destination: OwnerPlaceId(0),
                        fields: vec![(rv_field(0), rv_op(3, s))],
                    },
                    rv_span(s, 11),
                ),
                rv_ins(OwnedInstruction::OpenCall(CallSiteId(0)), rv_span(s, 12)),
                rv_ins(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(0),
                        argument: 0,
                        loan: LoanId(0),
                    },
                    rv_span(s, 13),
                ),
            ],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(3),
            },
            rv_span(s, 14),
        ),
        rv_block(
            vec![
                rv_assign(6, Rvalue::Load(place(1)), rv_span(s, 15)),
                rv_assign(
                    7,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Add,
                        left: rv_op(6, s),
                        right: rv_op(5, s),
                        operator_span: rv_span(s, 16),
                    },
                    rv_span(s, 16),
                ),
                rv_ins(
                    OwnedInstruction::Scalar(Statement::Store {
                        place: place(1),
                        value: rv_op(7, s),
                        operator_span: rv_span(s, 17),
                        span: rv_span(s, 17),
                    }),
                    rv_span(s, 170),
                ),
                rv_assign(
                    8,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Add,
                        left: rv_op(3, s),
                        right: rv_op(1, s),
                        operator_span: rv_span(s, 18),
                    },
                    rv_span(s, 18),
                ),
                rv_ins(
                    OwnedInstruction::Scalar(Statement::Store {
                        place: place(0),
                        value: rv_op(8, s),
                        operator_span: rv_span(s, 19),
                        span: rv_span(s, 19),
                    }),
                    rv_span(s, 190),
                ),
                rv_ins(
                    OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
                    rv_span(s, 20),
                ),
            ],
            OwnedTerminatorKind::Goto(BlockId(1)),
            rv_span(s, 21),
        ),
        rv_block(
            vec![rv_assign(9, Rvalue::Load(place(1)), rv_span(s, 22))],
            OwnedTerminatorKind::ReturnScalar(rv_op(9, s)),
            rv_span(s, 23),
        ),
    ];
    let mut g = rv_fn(1, ValueTy::Scalar(hir::Ty::I32), rv_span(s, 100));
    g.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
    g.references = vec![ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        kind: BorrowKind::Shared,
        position: 0,
        span: s,
    }];
    g.locals = vec![rv_local(hir::Ty::I32, s)];
    g.blocks = vec![rv_block(
        vec![rv_ins(
            OwnedInstruction::ReadField {
                destination: LocalId(0),
                base: AccessBase::Parameter(ReferenceParamId(0)),
                field: rv_field(0),
            },
            rv_span(s, 101),
        )],
        OwnedTerminatorKind::ReturnScalar(rv_op(0, s)),
        rv_span(s, 102),
    )];
    (
        sources,
        RawOwnedProgram {
            records: vec![rv_record(&[hir::Ty::I32], s)],
            functions: vec![f, g],
        },
    )
}
#[test]
fn reviewer_loop_reuses_slots_with_fresh_generations_loans_activations() {
    for n in [0, 1, 2, 17, 10000, 33331] {
        let (sources, raw) = rv_loop(n);
        let w = verified::verify_owned(raw, &sources).unwrap();
        let p = ExecutionPlan::build(&w).unwrap();
        let mut events = vec![];
        // Root X=32 (S12,A1,P1,O4,L12,C2), child X9. Only two frames and 13 old scalar slots ever coexist.
        let bytes = 2 * size_of::<Frame>()
            + size_of::<Scalar>()
            + (13 * size_of::<Option<Scalar>>() + 4 + 32 + 96 + 16)
            + (size_of::<Option<Scalar>>() + 64);
        assert_eq!(p.function(hir::DefId(0)).usage().expanded_cells, 32);
        assert_eq!(
            run_observed(
                &w,
                hir::DefId(0),
                Limits {
                    frames: 2,
                    slots: 13,
                    cells: 41,
                    bytes,
                    ..Limits::default()
                },
                &mut events
            ),
            Ok(Scalar::I32(n * (n - 1) / 2))
        );
        let acquired: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let Event::Acquire(loan, root, _) = e {
                    Some((*loan, *root))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(acquired.len(), n as usize);
        for (i, (loan, root)) in acquired.iter().enumerate() {
            assert_eq!(loan.instance, i as u64 + 1);
            assert_eq!(loan.frame, 0);
            assert_eq!(loan.activation, 1);
            assert_eq!(root.generation, 3 * i as u64 + 2);
        }
        let children: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let Event::Enter(hir::DefId(1), a) = e {
                    Some(*a)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(children, (2..n as u64 + 2).collect::<Vec<_>>());
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Release(_)))
                .count(),
            n as usize
        );
    }
}
#[test]
fn reviewer_embedded_scalar_charge_spans_and_source_independence() {
    let (mut sources, raw) = rv_loop(1);
    let s = raw.functions[0].span;
    let copied = raw.functions.clone();
    let w = verified::verify_owned(raw, &sources).unwrap();
    let mut external = copied;
    external[0].blocks.clear();
    sources = SourceMap::new();
    sources.add(
        "replacement.ox".into(),
        "fn main()->i32{return 999;}".into(),
    );
    assert_eq!(run(&w, Some(hir::DefId(0))), Ok(Scalar::I32(0)));
    // Root 33, three literals 3, next inner Initialize span must win over its outer owned span.
    assert_eq!(
        run_limits(
            &w,
            Some(hir::DefId(0)),
            Limits {
                fuel: 36,
                ..Limits::default()
            }
        ),
        Err(OwnedRunFailure::Scalar(RunFailure::Fuel(rv_span(s, 4))))
    );
}
#[test]
fn reviewer_measured_requested_memory_inequality() {
    assert_eq!(size_of::<Option<Scalar>>(), 8);
    assert_eq!(size_of::<Scalar>(), 8);
    assert_eq!(size_of::<Frame>(), 272);
    let global = 1024 * size_of::<Frame>() + 200000 * 8 + size_of::<Scalar>();
    assert_eq!(global, 1878536);
    let (sources, raw) = rv_mixed(&[3, 0, 2, 1], 7, 31, true);
    let w = verified::verify_owned(raw, &sources).unwrap();
    let p = ExecutionPlan::build(&w).unwrap();
    for f in p.functions() {
        let u = f.usage();
        assert!(u.payload_bytes.div_ceil(4) * 4 <= 4 * u.owner_cells);
        assert!(u.reference_bytes <= 8 * u.expanded_cells);
    }
    eprintln!("reviewer requested-memory measurements: OptionScalar={} Scalar={} Frame={} global_upper_bound={}",size_of::<Option<Scalar>>(),size_of::<Scalar>(),size_of::<Frame>(),global);
}
#[test]
fn reviewer_every_root_and_child_reservation_origin_after_fix() {
    let (sources, raw, _) = rv_alias([0, 1], [BorrowKind::Exclusive; 2], 17, 23);
    let s = raw.functions[0].span;
    let w = verified::verify_owned(raw, &sources).unwrap();
    let p = ExecutionPlan::build(&w).unwrap();
    for fail in 0..=15 {
        let mut events = vec![];
        let result = plan::fail_allocation_after(fail, || {
            execute_plan(&p, hir::DefId(0), Limits::default(), Some(&mut events))
        });
        if fail < 15 {
            assert_eq!(
                result,
                Err(OwnedRunFailure::Resource(plan::AdmissionFailure {
                    name: "injected owned allocation failure",
                    span: Some(if fail < 8 { s } else { rv_span(s, 10) })
                }))
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Enter(hir::DefId(1), _)))
                    .count(),
                0
            );
        } else {
            assert_eq!(result, Ok(Scalar::I32(40)));
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RvEdge {
    shared: bool,
    aliases: bool,
}
fn rv_chain(
    n: i32,
    root_shared: bool,
    edges: &[RvEdge],
    selection: usize,
) -> (SourceMap, RawOwnedProgram, usize) {
    let (sources, mut raw) = rv_loop(n);
    let s = raw.functions[0].span;
    let mode = |shared| {
        if shared {
            BorrowKind::Shared
        } else {
            BorrowKind::Exclusive
        }
    };
    raw.functions[0].loans[0].kind = mode(root_shared);
    raw.functions.truncate(1);
    let mut writes = 0;
    for layer in 0..=edges.len() {
        let id = layer + 1;
        let shared = if layer == 0 {
            root_shared
        } else {
            edges[layer - 1].shared
        };
        let refs = if layer > 0 && edges[layer - 1].aliases {
            2
        } else {
            1
        };
        let selected = selection % refs;
        let span = rv_span(s, 300 + layer * 50);
        let at = |k| rv_span(s, 301 + layer * 50 + k);
        let base = AccessBase::Parameter(ReferenceParamId(selected));
        let mut f = rv_fn(id, ValueTy::Scalar(hir::Ty::I32), span);
        f.parameters = (0..refs)
            .map(|i| ParameterBinding::Reference(ReferenceParamId(i)))
            .collect();
        f.references = (0..refs)
            .map(|i| ReferenceDecl {
                referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                    .unwrap(),
                kind: mode(shared),
                position: i,
                span,
            })
            .collect();
        let mut next = 0;
        let mut tail = vec![];
        if let Some(edge) = edges.get(layer) {
            let count = if edge.aliases { 2 } else { 1 };
            let mut before = vec![
                rv_ins(OwnedInstruction::OpenCall(CallSiteId(0)), at(0)),
                rv_ins(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(0),
                        argument: 0,
                        loan: LoanId(0),
                    },
                    at(1),
                ),
            ];
            if edge.shared {
                before.push(rv_ins(
                    OwnedInstruction::ReadField {
                        destination: LocalId(next),
                        base,
                        field: rv_field(0),
                    },
                    at(2),
                ));
                next += 1;
            }
            if edge.aliases {
                before.push(rv_ins(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(0),
                        argument: 1,
                        loan: LoanId(1),
                    },
                    at(3),
                ));
            }
            f.calls = vec![CallDecl {
                target: hir::DefId(id + 1),
                arguments: (0..count)
                    .map(|i| ArgumentSlot::Borrow(LoanId(i)))
                    .collect(),
                result: CallResult::Scalar(LocalId(next)),
                parent: None,
                span: at(0),
            }];
            next += 1;
            f.loans = (0..count)
                .map(|i| LoanDecl {
                    projection: Vec::new(),
                    call: CallSiteId(0),
                    argument: i,
                    authority: base,
                    kind: mode(edge.shared),
                    referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(
                        RecordId(0),
                    )))
                    .unwrap(),
                    span: at(if i == 0 { 1 } else { 3 }),
                })
                .collect();
            f.blocks.push(rv_block(
                before,
                OwnedTerminatorKind::Invoke {
                    call: CallSiteId(0),
                    continuation: BlockId(1),
                },
                at(4),
            ));
        }
        let read = next;
        next += 1;
        tail.push(rv_ins(
            OwnedInstruction::ReadField {
                destination: LocalId(read),
                base,
                field: rv_field(0),
            },
            at(10),
        ));
        let mut result = read;
        if !shared {
            let one = next;
            let sum = next + 1;
            next += 2;
            tail.push(rv_assign(one, Rvalue::I32(1), at(11)));
            tail.push(rv_assign(
                sum,
                Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Add,
                    left: rv_op(read, s),
                    right: rv_op(one, s),
                    operator_span: at(12),
                },
                at(12),
            ));
            tail.push(rv_ins(
                OwnedInstruction::WriteField {
                    base,
                    field: rv_field(0),
                    value: rv_op(sum, s),
                },
                at(13),
            ));
            result = sum;
            writes += 1;
        }
        f.blocks.push(rv_block(
            tail,
            OwnedTerminatorKind::ReturnScalar(rv_op(result, s)),
            at(14),
        ));
        f.locals = (0..next).map(|_| rv_local(hir::Ty::I32, span)).collect();
        raw.functions.push(f);
    }
    (sources, raw, writes)
}
fn rv_shapes(shared: bool, depth: usize, prefix: &mut Vec<RvEdge>, out: &mut Vec<Vec<RvEdge>>) {
    if depth == 0 {
        out.push(prefix.clone());
        return;
    }
    for edge in [
        RvEdge {
            shared: true,
            aliases: false,
        },
        RvEdge {
            shared: true,
            aliases: true,
        },
        RvEdge {
            shared: false,
            aliases: false,
        },
    ] {
        if shared && !edge.shared {
            continue;
        }
        prefix.push(edge);
        rv_shapes(edge.shared, depth - 1, prefix, out);
        prefix.pop();
    }
}
#[derive(Clone, Debug)]
struct RvPermission {
    key: LoanKey,
    root: OwnerKey,
    shared: bool,
    parent: Option<usize>,
    children: Vec<usize>,
    active: bool,
}
fn rv_probe_trace(
    m: &mut Machine<'_, '_>,
    model: &mut Vec<RvPermission>,
    seen: &mut usize,
    selection: usize,
    shape: (bool, &[RvEdge]),
    denials: &mut usize,
    span: Span,
) {
    let changed = m.events[*seen..]
        .iter()
        .any(|e| matches!(e, Event::Acquire(..) | Event::Release(..)));
    for event in &m.events[*seen..] {
        match event {
            Event::Acquire(key, root, kind) => {
                let shared = if key.frame == 0 {
                    shape.0
                } else {
                    shape.1[key.frame as usize - 1].shared
                };
                assert_eq!(
                    *kind,
                    if shared {
                        BorrowKind::Shared
                    } else {
                        BorrowKind::Exclusive
                    }
                );
                let parent = if key.frame == 0 {
                    None
                } else {
                    let candidates: Vec<_> = model
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| p.active && p.key.frame + 1 == key.frame)
                        .map(|(i, _)| i)
                        .collect();
                    assert!(!candidates.is_empty());
                    Some(candidates[selection % candidates.len()])
                };
                let idx = model.len();
                model.push(RvPermission {
                    key: *key,
                    root: *root,
                    shared,
                    parent,
                    children: vec![],
                    active: true,
                });
                if let Some(p) = parent {
                    model[p].children.push(idx);
                }
            }
            Event::Release(key) => {
                let index = model
                    .iter()
                    .position(|p| p.key == *key && p.active)
                    .unwrap();
                assert!(model[index].children.iter().all(|&c| !model[c].active));
                model[index].active = false;
            }
            _ => {}
        }
    }
    *seen = m.events.len();
    if !changed {
        return;
    }
    for p in model.iter().filter(|p| p.active) {
        let children: Vec<_> = p
            .children
            .iter()
            .map(|&c| &model[c])
            .filter(|p| p.active)
            .collect();
        let can_read = children.iter().all(|c| c.shared);
        let can_write = !p.shared && children.is_empty();
        let h = ReferenceHandle {
            root: p.root,
            permission: p.key,
            view: m.loan(p.key, span).unwrap().view,
        };
        assert_eq!(
            m.validate_handle(h, Access::Read, span).is_ok(),
            can_read,
            "model={model:?}"
        );
        assert_eq!(
            m.validate_handle(h, Access::Write, span).is_ok(),
            can_write,
            "model={model:?}"
        );
        assert_eq!(
            m.release_preflight(p.key, span).is_ok(),
            children.is_empty()
        );
        if !children.is_empty() {
            let before: Vec<_> = m
                .frames
                .iter()
                .map(|f| (f.owners.clone(), f.loans.clone()))
                .collect();
            assert!(m.release(p.key, span).is_err());
            let after: Vec<_> = m
                .frames
                .iter()
                .map(|f| (f.owners.clone(), f.loans.clone()))
                .collect();
            assert_eq!(before, after);
            *denials += 1;
        }
        if let Some(parent) = p.parent {
            assert!(model[parent].active);
        }
    }
}
#[test]
fn reviewer_generated_capability_trees_match_independent_permission_model() {
    let (mut cases, mut denials, mut acquisitions) = (0, 0, 0);
    for shared in [false, true] {
        for depth in 0..=3 {
            let mut shapes = vec![];
            rv_shapes(shared, depth, &mut vec![], &mut shapes);
            for edges in shapes {
                for selection in 0..2 {
                    for iterations in [1, 3] {
                        let (sources, raw, writes) =
                            rv_chain(iterations, shared, &edges, selection);
                        let s = raw.functions[0].span;
                        let w = verified::verify_owned(raw, &sources).unwrap();
                        let p = ExecutionPlan::build(&w).unwrap();
                        let mut m = rv_machine(&p);
                        m.fuel = 0;
                        let mut model = vec![];
                        let mut seen = 0;
                        loop {
                            m.fuel += 1;
                            let result = m.execute();
                            rv_probe_trace(
                                &mut m,
                                &mut model,
                                &mut seen,
                                selection,
                                (shared, &edges),
                                &mut denials,
                                s,
                            );
                            match result {
                                Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_))) => {}
                                Ok(value) => {
                                    assert_eq!(
                                        value,
                                        Scalar::I32(
                                            iterations * (iterations - 1) / 2
                                                + iterations * writes as i32
                                        )
                                    );
                                    break;
                                }
                                other => panic!("unexpected result {other:?}"),
                            }
                        }
                        assert!(model.iter().all(|p| !p.active));
                        assert_eq!(
                            m.events
                                .iter()
                                .filter(|e| matches!(e, Event::WriteField(..)))
                                .count(),
                            iterations as usize * writes
                        );
                        assert_eq!(
                            model.len(),
                            iterations as usize
                                * (1 + edges
                                    .iter()
                                    .map(|e| if e.aliases { 2 } else { 1 })
                                    .sum::<usize>())
                        );
                        acquisitions += model.len();
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 164);
    assert!(denials > 1000);
    eprintln!("reviewer capability corpus: cases={cases}, acquisitions={acquisitions}, checked live-descendant release denials={denials}; rejected raw executions=0");
}

#[test]
fn reviewer_reference_256_parameter_boundary_and_257_verifier_denial() {
    for count in [255, 256, 257] {
        let (sources, mut raw, _) = rv_alias([0, 0], [BorrowKind::Shared; 2], 19, 23);
        let s = raw.functions[0].span;
        raw.functions[0].calls[0].arguments = (0..count)
            .map(|i| ArgumentSlot::Borrow(LoanId(i)))
            .collect();
        raw.functions[0].loans = (0..count)
            .map(|i| LoanDecl {
                projection: Vec::new(),
                call: CallSiteId(0),
                argument: i,
                authority: AccessBase::Owner(OwnerPlaceId(0)),
                kind: BorrowKind::Shared,
                referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                    .unwrap(),
                span: rv_span(s, 1000 + i),
            })
            .collect();
        raw.functions[0].blocks[0].statements.truncate(7);
        for i in 0..count {
            raw.functions[0].blocks[0].statements.push(rv_ins(
                OwnedInstruction::PrepareBorrow {
                    call: CallSiteId(0),
                    argument: i,
                    loan: LoanId(i),
                },
                rv_span(s, 1000 + i),
            ));
        }
        raw.functions[1].parameters = (0..count)
            .map(|i| ParameterBinding::Reference(ReferenceParamId(i)))
            .collect();
        raw.functions[1].references = (0..count)
            .map(|i| ReferenceDecl {
                referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                    .unwrap(),
                kind: BorrowKind::Shared,
                position: i,
                span: s,
            })
            .collect();
        if let OwnedInstruction::ReadField { base, .. } =
            &mut raw.functions[1].blocks[0].statements[1].kind
        {
            *base = AccessBase::Parameter(ReferenceParamId(count - 1));
        }
        match verified::verify_owned(raw, &sources) {
            Ok(w) => {
                assert!(count <= 256);
                assert_eq!(run(&w, Some(hir::DefId(0))), Ok(Scalar::I32(38)));
            }
            Err(_) => assert_eq!(count, 257),
        }
    }
}
fn rv_owned_chain(depth: usize, a: i32, b: i32) -> (SourceMap, RawOwnedProgram) {
    let (sources, s) = rv_env();
    let mut root = rv_fn(0, ValueTy::Scalar(hir::Ty::I32), s);
    root.locals = [
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::Unit,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
    ]
    .into_iter()
    .map(|ty| rv_local(ty, s))
    .collect();
    root.owners = vec![
        rv_owner(OwnerKind::Local { mutable: false }, s),
        rv_owner(
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            s,
        ),
        rv_owner(
            OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            s,
        ),
    ];
    root.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(1))],
        result: CallResult::Owned(OwnerPlaceId(2)),
        parent: None,
        span: s,
    }];
    root.blocks = vec![
        rv_block(
            vec![
                rv_assign(0, Rvalue::Bool(true), rv_span(s, 1)),
                rv_assign(1, Rvalue::I32(a), rv_span(s, 2)),
                rv_assign(2, Rvalue::Unit, rv_span(s, 3)),
                rv_assign(3, Rvalue::I32(b), rv_span(s, 4)),
                rv_ins(
                    OwnedInstruction::StorageLive(OwnerPlaceId(0)),
                    rv_span(s, 5),
                ),
                rv_ins(
                    OwnedInstruction::Construct {
                        destination: OwnerPlaceId(0),
                        fields: vec![
                            (rv_field(3), rv_op(3, s)),
                            (rv_field(1), rv_op(1, s)),
                            (rv_field(0), rv_op(0, s)),
                            (rv_field(2), rv_op(2, s)),
                        ],
                    },
                    rv_span(s, 6),
                ),
                rv_ins(OwnedInstruction::OpenCall(CallSiteId(0)), rv_span(s, 7)),
                rv_ins(
                    OwnedInstruction::PrepareOwned {
                        call: CallSiteId(0),
                        argument: 0,
                        source: OwnerPlaceId(0),
                    },
                    rv_span(s, 8),
                ),
            ],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            rv_span(s, 9),
        ),
        rv_block(
            vec![
                rv_ins(
                    OwnedInstruction::ReadField {
                        destination: LocalId(4),
                        base: AccessBase::Owner(OwnerPlaceId(2)),
                        field: rv_field(1),
                    },
                    rv_span(s, 10),
                ),
                rv_ins(
                    OwnedInstruction::ReadField {
                        destination: LocalId(5),
                        base: AccessBase::Owner(OwnerPlaceId(2)),
                        field: rv_field(3),
                    },
                    rv_span(s, 11),
                ),
                rv_assign(
                    6,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Subtract,
                        left: rv_op(4, s),
                        right: rv_op(5, s),
                        operator_span: rv_span(s, 12),
                    },
                    rv_span(s, 12),
                ),
                rv_ins(
                    OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
                    rv_span(s, 13),
                ),
                rv_ins(
                    OwnedInstruction::StorageEnd(OwnerPlaceId(2)),
                    rv_span(s, 14),
                ),
            ],
            OwnedTerminatorKind::ReturnScalar(rv_op(6, s)),
            rv_span(s, 15),
        ),
    ];
    let mut functions = vec![root];
    for id in 1..=depth {
        let mut f = rv_fn(
            id,
            ValueTy::Owned(AggregateTy::Record(RecordId(0))),
            rv_span(s, 100 + id),
        );
        f.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
        f.owners = vec![rv_owner(OwnerKind::Parameter { position: 0 }, s)];
        if id == depth {
            f.blocks = vec![rv_block(
                vec![],
                OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
                rv_span(s, 100 + id),
            )];
        } else {
            f.owners.extend([
                rv_owner(
                    OwnerKind::StagedArgument {
                        call: CallSiteId(0),
                        argument: 0,
                    },
                    s,
                ),
                rv_owner(
                    OwnerKind::CallResult {
                        call: CallSiteId(0),
                    },
                    s,
                ),
            ]);
            f.calls = vec![CallDecl {
                target: hir::DefId(id + 1),
                arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(1))],
                result: CallResult::Owned(OwnerPlaceId(2)),
                parent: None,
                span: s,
            }];
            f.blocks = vec![
                rv_block(
                    vec![
                        rv_ins(OwnedInstruction::OpenCall(CallSiteId(0)), s),
                        rv_ins(
                            OwnedInstruction::PrepareOwned {
                                call: CallSiteId(0),
                                argument: 0,
                                source: OwnerPlaceId(0),
                            },
                            s,
                        ),
                    ],
                    OwnedTerminatorKind::Invoke {
                        call: CallSiteId(0),
                        continuation: BlockId(1),
                    },
                    s,
                ),
                rv_block(vec![], OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(2)), s),
            ];
        }
        functions.push(f);
    }
    (
        sources,
        RawOwnedProgram {
            records: vec![rv_record(
                &[hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit, hir::Ty::I32],
                s,
            )],
            functions,
        },
    )
}
#[test]
fn reviewer_owned_returns_keep_distinct_values_across_popped_frames() {
    for depth in [1, 2, 7, 31, 64] {
        for (a, b) in [(711, -27), (-101, 33)] {
            let (sources, raw) = rv_owned_chain(depth, a, b);
            let w = verified::verify_owned(raw, &sources).unwrap();
            let mut events = vec![];
            assert_eq!(
                run_observed(&w, hir::DefId(0), Limits::default(), &mut events),
                Ok(Scalar::I32(a - b))
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Transfer(..)))
                    .count(),
                3 * depth
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Return(_)))
                    .count(),
                depth + 1
            );
        }
    }
}
#[test]
fn reviewer_overflow_after_successful_write_and_unpaid_write_are_distinct() {
    let (sources, mut raw) = rv_mixed(&[3, 2, 1, 0], i32::MAX, 1, true);
    let s = raw.functions[0].span;
    // Insert an earlier complete store before the already-existing later addition overflows.
    let at = rv_span(s, 90);
    raw.functions[0].blocks[0].statements.insert(
        9,
        rv_ins(
            OwnedInstruction::WriteField {
                base: AccessBase::Owner(OwnerPlaceId(0)),
                field: rv_field(3),
                value: rv_op(3, s),
            },
            at,
        ),
    );
    let w = verified::verify_owned(raw, &sources).unwrap();
    let mut events = vec![];
    assert_eq!(
        run_observed(&w, hir::DefId(0), Limits::default(), &mut events),
        Err(OwnedRunFailure::Scalar(RunFailure::Overflow(rv_span(
            s, 15
        ))))
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::WriteField(..)))
            .count(),
        1
    );
    // Root X=33 =>34, four literals, three lives, Construct5, ReadField1 =47 before the inserted write.
    let mut failed = vec![];
    assert_eq!(
        run_observed(
            &w,
            hir::DefId(0),
            Limits {
                fuel: 47,
                ..Limits::default()
            },
            &mut failed
        ),
        Err(OwnedRunFailure::Scalar(RunFailure::Fuel(at)))
    );
    assert!(!failed.iter().any(|e| matches!(e, Event::WriteField(..))));
}
#[test]
fn reviewer_scalar_snapshot_and_owned_staging_are_independent_storage() {
    let (sources, s) = rv_env();
    let mut root = rv_fn(0, ValueTy::Scalar(hir::Ty::I32), s);
    root.locals = (0..3).map(|_| rv_local(hir::Ty::I32, s)).collect();
    root.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Scalar; 2],
        result: CallResult::Scalar(LocalId(2)),
        parent: None,
        span: s,
    }];
    root.blocks = vec![
        rv_block(
            vec![
                rv_assign(0, Rvalue::I32(41), s),
                rv_ins(OwnedInstruction::OpenCall(CallSiteId(0)), s),
                rv_ins(
                    OwnedInstruction::PrepareScalar {
                        call: CallSiteId(0),
                        argument: 0,
                        value: rv_op(0, s),
                    },
                    s,
                ),
                rv_assign(1, Rvalue::I32(99), s),
                rv_ins(
                    OwnedInstruction::PrepareScalar {
                        call: CallSiteId(0),
                        argument: 1,
                        value: rv_op(1, s),
                    },
                    s,
                ),
            ],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            s,
        ),
        rv_block(vec![], OwnedTerminatorKind::ReturnScalar(rv_op(2, s)), s),
    ];
    let mut child = rv_fn(1, ValueTy::Scalar(hir::Ty::I32), s);
    child.locals = (0..3)
        .map(|i| LocalDecl {
            ty: hir::Ty::I32,
            kind: if i < 2 {
                LocalKind::Parameter
            } else {
                LocalKind::Temporary
            },
            span: s,
        })
        .collect();
    child.parameters = vec![
        ParameterBinding::Scalar(LocalId(0)),
        ParameterBinding::Scalar(LocalId(1)),
    ];
    child.blocks = vec![rv_block(
        vec![rv_assign(
            2,
            Rvalue::CheckedI32 {
                op: hir::ArithmeticOp::Add,
                left: rv_op(0, s),
                right: rv_op(1, s),
                operator_span: s,
            },
            s,
        )],
        OwnedTerminatorKind::ReturnScalar(rv_op(2, s)),
        s,
    )];
    let w = verified::verify_owned(
        RawOwnedProgram {
            records: vec![],
            functions: vec![root, child],
        },
        &sources,
    )
    .unwrap();
    assert_eq!(run(&w, Some(hir::DefId(0))), Ok(Scalar::I32(140)));
    let p = ExecutionPlan::build(&w).unwrap();
    let mut m = rv_machine(&p);
    for ins in &w.functions()[0].blocks[0].statements[..3] {
        m.statement(0, &ins.kind, s).unwrap();
    }
    m.frames[0].next = 3;
    m.frames[0].slots[0] = Some(Scalar::I32(999));
    assert_eq!(m.frames[0].snapshots[0], Some(Scalar::I32(41)));
    assert_eq!(m.execute(), Ok(Scalar::I32(140)));
    let (sources, raw) = rv_owned_chain(2, 771, -31);
    let w = verified::verify_owned(raw, &sources).unwrap();
    let p = ExecutionPlan::build(&w).unwrap();
    let mut m = rv_machine(&p);
    for ins in &w.functions()[0].blocks[0].statements {
        m.statement(0, &ins.kind, ins.span).unwrap();
    }
    assert_eq!(m.frames[0].owners[0].state, MOVED);
    assert_eq!(m.frames[0].owners[1].state, AVAILABLE);
    let original = p.function(hir::DefId(0)).owner_offset(OwnerPlaceId(0));
    m.frames[0].payload[original..original + 16].fill(0);
    m.frames[0].next = w.functions()[0].blocks[0].statements.len();
    assert_eq!(m.execute(), Ok(Scalar::I32(802)));
}
#[test]
fn reviewer_frozen_hand_counts_and_batch_observed_totals() {
    // Expected literals are recomputed from the reviewed cost specification, not Schedule or plan helpers.
    for (build, costs) in [
        (
            empty_record as fn() -> (SourceMap, RawOwnedProgram, Schedule),
            vec![7, 1, 2, 2, 2, 1, 2],
        ),
        (owned_relay, vec![21, 1, 1, 2, 2, 2, 8, 3, 1, 2, 2, 2, 5]),
        (shared_read, vec![23, 1, 1, 2, 1, 1, 11, 1, 2, 2, 4]),
    ] {
        let (sources, raw, _) = build();
        let w = verified::verify_owned(raw, &sources).unwrap();
        let mut full = vec![];
        assert!(run_observed(&w, hir::DefId(0), Limits::default(), &mut full).is_ok());
        let actual: Vec<_> = full
            .iter()
            .filter_map(|e| {
                if let Event::Charge(s, c) = e {
                    Some((*s, *c))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(actual.iter().map(|e| e.1).collect::<Vec<_>>(), costs);
        let total: usize = costs.iter().sum();
        for fuel in 0..total {
            let mut remaining = fuel;
            let mut index = 0;
            while remaining >= costs[index] {
                remaining -= costs[index];
                index += 1;
            }
            assert_eq!(
                run_limits(
                    &w,
                    Some(hir::DefId(0)),
                    Limits {
                        fuel,
                        ..Limits::default()
                    }
                ),
                Err(OwnedRunFailure::Scalar(RunFailure::Fuel(actual[index].0)))
            );
        }
    }
    let (sources, raw, _) = super::super::consumer_pilot::batch();
    let w = verified::verify_owned(raw, &sources).unwrap();
    let mut events = vec![];
    assert_eq!(
        run_observed(&w, hir::DefId(0), Limits::default(), &mut events),
        Ok(Scalar::I32(
            (1..=6).map(|job| job * 10).sum::<i32>() + 6 * 100 + 6
        ))
    );
    let charges: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let Event::Charge(_, c) = e {
                Some(*c)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(charges.len(), 404);
    assert_eq!(
        charges.iter().sum::<usize>(),
        112 + 12 + 5 * 140 + 142 + 120
    );
    let writes: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let Event::WriteField(_, f, v) = e {
                Some((f.index, *v))
            } else {
                None
            }
        })
        .collect();
    let mut checksum = 0;
    let mut expected = vec![];
    for job in 1..=6 {
        checksum += job * 10;
        expected.extend([
            (1, Scalar::I32(job)),
            (0, Scalar::I32(job)),
            (2, Scalar::I32(checksum)),
        ]);
    }
    expected.push((3, Scalar::Bool(false)));
    assert_eq!(writes, expected);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Acquire(..)))
            .count(),
        6 * (1 + 2 + 1)
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Release(..)))
            .count(),
        24
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Transfer(..)))
            .count(),
        6
    );
}
#[test]
fn reviewer_popped_frame_reuse_never_revives_old_loan_permission() {
    let (sources, raw, _) = rv_chain(
        3,
        false,
        &[RvEdge {
            shared: true,
            aliases: false,
        }],
        0,
    );
    let s = raw.functions[0].span;
    let w = verified::verify_owned(raw, &sources).unwrap();
    let p = ExecutionPlan::build(&w).unwrap();
    let mut m = rv_machine(&p);
    m.fuel = 0;
    let (mut remembered, mut compared): (Option<ReferenceHandle>, bool) = (None, false);
    loop {
        m.fuel += 1;
        let result = m.execute();
        if m.frames.len() > 1 && m.frames[1].loans[0].state == 1 {
            let current = m.handle(1, LoanId(0), s).unwrap();
            if let Some(old) = remembered {
                if old != current {
                    assert_eq!(old.permission.frame, current.permission.frame);
                    assert_ne!(old.permission.activation, current.permission.activation);
                    assert!(m.validate_handle(old, Access::Read, s).is_err());
                    assert!(m.validate_handle(current, Access::Read, s).is_ok());
                    compared = true;
                }
            } else {
                remembered = Some(current);
            }
        }
        match result {
            Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_))) => {}
            Ok(_) => break,
            other => panic!("{other:?}"),
        }
    }
    assert!(compared);
}
#[test]
fn reviewer_transfer_event_destination_is_the_installed_generation() {
    let (sources, raw) = rv_owned_chain(2, 31, -7);
    let s = raw.functions[0].span;
    let w = verified::verify_owned(raw, &sources).unwrap();
    let p = ExecutionPlan::build(&w).unwrap();
    let mut m = rv_machine(&p);
    for ins in &w.functions()[0].blocks[0].statements {
        let before = m.events.len();
        m.statement(0, &ins.kind, ins.span).unwrap();
        for event in &m.events[before..] {
            if let Event::Transfer(_, to) = event {
                assert!(
                    m.owner(*to, s).is_ok(),
                    "transfer destination {to:?} should identify initialized value"
                );
            }
        }
    }
}
#[test]
fn reviewer_nonzero_entry_bool_merge_selects_only_taken_initialized_input() {
    for take_left in [false, true] {
        let (sources, s) = rv_env();
        let mut f = rv_fn(0, ValueTy::Scalar(hir::Ty::Bool), s);
        f.entry = BlockId(3);
        f.locals = (0..4).map(|_| rv_local(hir::Ty::Bool, s)).collect();
        f.owners = vec![rv_owner(OwnerKind::Local { mutable: false }, s)];
        f.blocks = vec![
            rv_block(
                vec![rv_assign(1, Rvalue::Bool(true), rv_span(s, 1))],
                OwnedTerminatorKind::Goto(BlockId(2)),
                rv_span(s, 2),
            ),
            rv_block(
                vec![rv_assign(2, Rvalue::Bool(false), rv_span(s, 11))],
                OwnedTerminatorKind::Goto(BlockId(2)),
                rv_span(s, 12),
            ),
            rv_block(
                vec![rv_ins(
                    OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
                    rv_span(s, 22),
                )],
                OwnedTerminatorKind::ReturnScalar(rv_op(3, s)),
                rv_span(s, 23),
            ),
            rv_block(
                vec![
                    rv_assign(0, Rvalue::Bool(take_left), rv_span(s, 31)),
                    rv_ins(
                        OwnedInstruction::StorageLive(OwnerPlaceId(0)),
                        rv_span(s, 32),
                    ),
                    rv_ins(
                        OwnedInstruction::Construct {
                            destination: OwnerPlaceId(0),
                            fields: vec![],
                        },
                        rv_span(s, 33),
                    ),
                ],
                OwnedTerminatorKind::Branch {
                    condition: rv_op(0, s),
                    then_block: BlockId(0),
                    else_block: BlockId(1),
                },
                rv_span(s, 34),
            ),
        ];
        f.blocks[2].merge = Some(BoolMerge {
            operator_span: rv_span(s, 20),
            destination: LocalId(3),
            incoming: [
                MergeInput {
                    predecessor: BlockId(0),
                    value: rv_op(1, s),
                },
                MergeInput {
                    predecessor: BlockId(1),
                    value: rv_op(2, s),
                },
            ],
            span: rv_span(s, 21),
        });
        let w = verified::verify_owned(
            RawOwnedProgram {
                records: vec![rv_record(&[], s)],
                functions: vec![f],
            },
            &sources,
        )
        .unwrap();
        let schedule = [
            (0, 10),
            (31, 1),
            (32, 1),
            (33, 2),
            (34, 1),
            (if take_left { 1 } else { 11 }, 1),
            (if take_left { 2 } else { 12 }, 1),
            (21, 1),
            (22, 2),
            (23, 2),
        ];
        for fuel in 0..=22 {
            let mut remaining = fuel;
            let mut failure = None;
            for (i, cost) in schedule {
                if remaining < cost {
                    failure = Some(rv_span(s, i));
                    break;
                }
                remaining -= cost;
            }
            let actual = run_limits(
                &w,
                Some(hir::DefId(0)),
                Limits {
                    fuel,
                    ..Limits::default()
                },
            );
            assert_eq!(
                actual,
                match failure {
                    Some(s) => Err(OwnedRunFailure::Scalar(RunFailure::Fuel(s))),
                    None => Ok(Scalar::Bool(take_left)),
                }
            );
        }
    }
}
