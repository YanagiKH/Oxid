//! Raw composition qualification, shared with the source-free native consumer.
use super::consumer_fixtures as raw;
use super::*;

fn field(record: usize, index: usize) -> FieldId {
    FieldId {
        record: RecordId(record),
        index,
    }
}
fn declaration(id: usize, fields: &[ValueTy], span: Span) -> RawRecordDecl {
    RawRecordDecl {
        id: RecordId(id),
        span,
        fields: fields
            .iter()
            .enumerate()
            .map(|(index, &ty)| RawFieldDecl {
                id: field(id, index),
                ty: ParameterTy::Value(ty),
                span,
            })
            .collect(),
    }
}
fn owner(aggregate: AggregateTy, kind: OwnerKind, span: Span) -> OwnerDecl {
    OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
        kind,
        span,
    }
}
fn block(statements: Vec<OwnedStatement>, kind: OwnedTerminatorKind, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: raw::end(kind, span),
    }
}
fn arithmetic(
    destination: usize,
    op: hir::ArithmeticOp,
    left: usize,
    right: usize,
    span: Span,
) -> OwnedStatement {
    raw::assign(
        destination,
        Rvalue::CheckedI32 {
            op,
            left: raw::operand(left, span),
            right: raw::operand(right, span),
            operator_span: span,
        },
        span,
    )
}

pub(super) fn batch() -> (SourceMap, RawOwnedProgram) {
    let (sources, s) = raw::context();
    let meta = AggregateTy::Record(RecordId(0));
    let batch = AggregateTy::Record(RecordId(1));
    let array = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 3).unwrap());
    let mut main = raw::function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    main.locals = (0..14)
        .map(|id| raw::scalar(if id == 4 { hir::Ty::Unit } else { hir::Ty::I32 }, s(id)))
        .collect();
    main.owners = vec![
        owner(meta, OwnerKind::Local { mutable: false }, s(0)),
        owner(meta, OwnerKind::Temporary, s(0)),
        owner(array, OwnerKind::Local { mutable: false }, s(0)),
        owner(array, OwnerKind::Temporary, s(0)),
        owner(batch, OwnerKind::Local { mutable: false }, s(0)),
        owner(
            batch,
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            s(0),
        ),
        owner(
            batch,
            OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            s(0),
        ),
        owner(batch, OwnerKind::Local { mutable: true }, s(0)),
    ];
    main.calls = vec![
        CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(5))],
            result: CallResult::Owned(OwnerPlaceId(6)),
            parent: None,
            span: s(30),
        },
        CallDecl {
            target: hir::DefId(2),
            arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
            result: CallResult::Scalar(LocalId(4)),
            parent: None,
            span: s(40),
        },
    ];
    main.loans = vec![LoanDecl {
        projection: Vec::new(),
        call: CallSiteId(1),
        argument: 0,
        authority: AccessBase::Owner(OwnerPlaceId(7)),
        kind: BorrowKind::Exclusive,
        referent: BorrowedSlot::check(BorrowedTy::Exact(batch)).unwrap(),
        span: s(41),
    }];
    let mut statements: Vec<_> = (0..4)
        .map(|n| raw::assign(n, Rvalue::I32(n as i32), s(n + 1)))
        .collect();
    statements.extend([
        raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(10)),
        raw::instruction(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(0),
                fields: vec![(field(0, 0), raw::operand(0, s(11)))],
            },
            s(11),
        ),
        raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), s(12)),
        raw::instruction(
            OwnedInstruction::MoveInitialize {
                destination: OwnerPlaceId(1),
                source: OwnerPlaceId(0),
            },
            s(13),
        ),
        raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(2)), s(14)),
        raw::instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(2),
                elements: (1..4).map(|n| raw::operand(n, s(15))).collect(),
            },
            s(15),
        ),
        raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(3)), s(16)),
        raw::instruction(
            OwnedInstruction::MoveInitialize {
                destination: OwnerPlaceId(3),
                source: OwnerPlaceId(2),
            },
            s(17),
        ),
        raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(4)), s(18)),
        raw::instruction(
            OwnedInstruction::ConstructComposite {
                destination: OwnerPlaceId(4),
                fields: vec![
                    (field(1, 0), FieldInitializer::Owned(OwnerPlaceId(1))),
                    (field(1, 1), FieldInitializer::Owned(OwnerPlaceId(3))),
                ],
            },
            s(19),
        ),
    ]);
    for owner in 0..4 {
        statements.push(raw::instruction(
            OwnedInstruction::StorageEnd(OwnerPlaceId(owner)),
            s(20 + owner),
        ));
    }
    statements.extend([
        raw::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(30)),
        raw::instruction(
            OwnedInstruction::PrepareOwned {
                call: CallSiteId(0),
                argument: 0,
                source: OwnerPlaceId(4),
            },
            s(31),
        ),
    ]);
    main.blocks.push(block(
        statements,
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s(32),
    ));
    main.blocks.push(block(
        vec![
            raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(7)), s(35)),
            raw::instruction(
                OwnedInstruction::MoveInitialize {
                    destination: OwnerPlaceId(7),
                    source: OwnerPlaceId(6),
                },
                s(36),
            ),
            raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(6)), s(37)),
            raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(4)), s(38)),
            raw::instruction(OwnedInstruction::OpenCall(CallSiteId(1)), s(40)),
            raw::instruction(
                OwnedInstruction::PrepareBorrow {
                    call: CallSiteId(1),
                    argument: 0,
                    loan: LoanId(0),
                },
                s(41),
            ),
        ],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(1),
            continuation: BlockId(2),
        },
        s(42),
    ));
    main.blocks.push(block(
        vec![
            raw::instruction(
                OwnedInstruction::ReadProjection {
                    destination: LocalId(5),
                    base: AccessBase::Owner(OwnerPlaceId(7)),
                    path: vec![field(1, 0), field(0, 0)],
                    index: None,
                },
                s(50),
            ),
            raw::instruction(
                OwnedInstruction::ReadProjection {
                    destination: LocalId(6),
                    base: AccessBase::Owner(OwnerPlaceId(7)),
                    path: vec![field(1, 1)],
                    index: Some(raw::operand(0, s(51))),
                },
                s(51),
            ),
            raw::instruction(
                OwnedInstruction::ReadProjection {
                    destination: LocalId(7),
                    base: AccessBase::Owner(OwnerPlaceId(7)),
                    path: vec![field(1, 1)],
                    index: Some(raw::operand(2, s(52))),
                },
                s(52),
            ),
            raw::assign(8, Rvalue::I32(100), s(53)),
            raw::assign(9, Rvalue::I32(10), s(54)),
            arithmetic(10, hir::ArithmeticOp::Multiply, 5, 8, s(55)),
            arithmetic(11, hir::ArithmeticOp::Multiply, 6, 9, s(56)),
            arithmetic(12, hir::ArithmeticOp::Add, 10, 11, s(57)),
            arithmetic(13, hir::ArithmeticOp::Add, 12, 7, s(58)),
            raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(7)), s(59)),
        ],
        OwnedTerminatorKind::ReturnScalar(raw::operand(13, s(60))),
        s(60),
    ));

    let mut relay = raw::function(1, ValueTy::Owned(batch), s(100));
    relay.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
    relay.owners = vec![owner(batch, OwnerKind::Parameter { position: 0 }, s(100))];
    relay.blocks = vec![block(
        vec![],
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
        s(101),
    )];

    let mut bump = raw::function(2, ValueTy::Scalar(hir::Ty::Unit), s(200));
    bump.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
    bump.references = vec![ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(batch)).unwrap(),
        kind: BorrowKind::Exclusive,
        position: 0,
        span: s(200),
    }];
    bump.locals = (0..17)
        .map(|id| {
            raw::scalar(
                if id == 16 {
                    hir::Ty::Unit
                } else {
                    hir::Ty::I32
                },
                s(200 + id),
            )
        })
        .collect();
    let base = AccessBase::Parameter(ReferenceParamId(0));
    let mut body: Vec<_> = (0..3)
        .map(|n| raw::assign(n, Rvalue::I32(n as i32), s(201 + n)))
        .collect();
    body.push(raw::instruction(
        OwnedInstruction::ProjectionLength {
            destination: LocalId(3),
            base,
            path: vec![field(1, 1)],
        },
        s(204),
    ));
    for ordinal in 0..3 {
        let local = 4 + 4 * ordinal;
        let at = 210 + 10 * ordinal;
        body.extend([
            raw::instruction(
                OwnedInstruction::ReadProjection {
                    destination: LocalId(local),
                    base,
                    path: vec![field(1, 1)],
                    index: Some(raw::operand(ordinal, s(at))),
                },
                s(at),
            ),
            arithmetic(local + 1, hir::ArithmeticOp::Add, local, 1, s(at + 1)),
            raw::instruction(
                OwnedInstruction::WriteProjection {
                    base,
                    path: vec![field(1, 1)],
                    index: Some(raw::operand(ordinal, s(at + 2))),
                    value: raw::operand(local + 1, s(at + 2)),
                },
                s(at + 2),
            ),
            raw::instruction(
                OwnedInstruction::ReadProjection {
                    destination: LocalId(local + 2),
                    base,
                    path: vec![field(1, 0), field(0, 0)],
                    index: None,
                },
                s(at + 3),
            ),
            arithmetic(local + 3, hir::ArithmeticOp::Add, local + 2, 1, s(at + 4)),
            raw::instruction(
                OwnedInstruction::WriteProjection {
                    base,
                    path: vec![field(1, 0), field(0, 0)],
                    index: None,
                    value: raw::operand(local + 3, s(at + 5)),
                },
                s(at + 5),
            ),
        ]);
    }
    body.push(raw::assign(16, Rvalue::Unit, s(240)));
    bump.blocks = vec![block(
        body,
        OwnedTerminatorKind::ReturnScalar(raw::operand(16, s(241))),
        s(241),
    )];
    (
        sources,
        RawOwnedProgram {
            records: vec![
                declaration(0, &[ValueTy::Scalar(hir::Ty::I32)], s(300)),
                declaration(1, &[ValueTy::Owned(meta), ValueTy::Owned(array)], s(301)),
            ],
            functions: vec![main, relay, bump],
        },
    )
}

#[test]
fn composite_reference_batch_relay_and_whole_root_borrow_returns_324() {
    let (sources, raw) = batch();
    let witness = verify_owned(raw, &sources).unwrap();
    let mut events = vec![];
    assert_eq!(
        execute::run_observed(
            &witness,
            hir::DefId(0),
            execute::Limits::default(),
            &mut events
        ),
        Ok(Scalar::I32(324))
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, execute::Event::WriteIndex(..)))
            .count(),
        3
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, execute::Event::WriteField(..)))
            .count(),
        3
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, execute::Event::Transfer(..)))
            .count(),
        6
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, execute::Event::Acquire(..)))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, execute::Event::Release(..)))
            .count(),
        1
    );
}

pub(super) fn empty_composition() -> (SourceMap, RawOwnedProgram) {
    let (sources, s) = raw::context();
    let empty = AggregateTy::Record(RecordId(0));
    let outer = AggregateTy::Record(RecordId(1));
    let array = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 0).unwrap());
    let mut main = raw::function(0, ValueTy::Scalar(hir::Ty::Unit), s(0));
    main.locals = vec![
        raw::scalar(hir::Ty::Bool, s(1)),
        raw::scalar(hir::Ty::Unit, s(2)),
        raw::scalar(hir::Ty::I32, s(20)),
    ];
    main.owners = vec![
        owner(empty, OwnerKind::Temporary, s(0)),
        owner(array, OwnerKind::Temporary, s(0)),
        owner(outer, OwnerKind::Local { mutable: false }, s(0)),
        owner(
            outer,
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            s(0),
        ),
        owner(
            outer,
            OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            s(0),
        ),
    ];
    main.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(3))],
        result: CallResult::Owned(OwnerPlaceId(4)),
        parent: None,
        span: s(12),
    }];
    main.blocks = vec![
        block(
            vec![
                raw::assign(0, Rvalue::Bool(true), s(1)),
                raw::assign(1, Rvalue::Unit, s(2)),
                raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(3)),
                raw::instruction(
                    OwnedInstruction::Construct {
                        destination: OwnerPlaceId(0),
                        fields: vec![],
                    },
                    s(4),
                ),
                raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), s(5)),
                raw::instruction(
                    OwnedInstruction::ConstructArray {
                        destination: OwnerPlaceId(1),
                        elements: vec![],
                    },
                    s(6),
                ),
                raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(2)), s(7)),
                raw::instruction(
                    OwnedInstruction::ConstructComposite {
                        destination: OwnerPlaceId(2),
                        fields: vec![
                            (field(1, 3), FieldInitializer::Scalar(raw::operand(1, s(8)))),
                            (field(1, 1), FieldInitializer::Owned(OwnerPlaceId(0))),
                            (field(1, 0), FieldInitializer::Scalar(raw::operand(0, s(8)))),
                            (field(1, 2), FieldInitializer::Owned(OwnerPlaceId(1))),
                        ],
                    },
                    s(8),
                ),
                raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(9)),
                raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), s(10)),
                raw::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(12)),
                raw::instruction(
                    OwnedInstruction::PrepareOwned {
                        call: CallSiteId(0),
                        argument: 0,
                        source: OwnerPlaceId(2),
                    },
                    s(13),
                ),
            ],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            s(14),
        ),
        block(
            vec![
                raw::instruction(
                    OwnedInstruction::ProjectionLength {
                        destination: LocalId(2),
                        base: AccessBase::Owner(OwnerPlaceId(4)),
                        path: vec![field(1, 2)],
                    },
                    s(20),
                ),
                raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(2)), s(21)),
                raw::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(4)), s(22)),
            ],
            OwnedTerminatorKind::ReturnScalar(raw::operand(1, s(23))),
            s(23),
        ),
    ];
    let mut relay = raw::function(1, ValueTy::Owned(outer), s(30));
    relay.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
    relay.owners = vec![owner(outer, OwnerKind::Parameter { position: 0 }, s(30))];
    relay.blocks = vec![block(
        vec![],
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
        s(31),
    )];
    (
        sources,
        RawOwnedProgram {
            records: vec![
                declaration(0, &[], s(40)),
                declaration(
                    1,
                    &[
                        ValueTy::Scalar(hir::Ty::Bool),
                        ValueTy::Owned(empty),
                        ValueTy::Owned(array),
                        ValueTy::Scalar(hir::Ty::Unit),
                    ],
                    s(41),
                ),
            ],
            functions: vec![main, relay],
        },
    )
}

#[test]
fn composite_reference_empty_children_keep_full_sentinels_and_do_not_copy_padding() {
    let (sources, raw) = empty_composition();
    let witness = verify_owned(raw, &sources).unwrap();
    let observed = execute::run_array_observed(
        &witness,
        Some(hir::DefId(0)),
        execute::Limits::default(),
        execute::ObservationControl {
            poison_destinations: true,
            ..Default::default()
        },
    );
    assert_eq!(observed.result, Ok(Scalar::Unit));
    assert!(!observed.truncated);
    let constructed = observed
        .storage
        .iter()
        .find(|row| row.kind == execute::StorageObservationKind::Construction && row.key.owner == 2)
        .unwrap();
    assert!(constructed.poisoned);
    assert_eq!(
        constructed.bytes,
        vec![1, 0, 0xa5, 0xa5, 0, 0, 0, 0, 0, 0xa5, 0xa5, 0xa5]
    );
    let returned = observed
        .storage
        .iter()
        .find(|row| {
            row.kind == execute::StorageObservationKind::Return
                && row.key.frame == 0
                && row.key.owner == 4
        })
        .unwrap();
    assert_eq!(returned.bytes, vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert!(observed
        .events
        .iter()
        .any(|event| matches!(event, execute::Event::ArrayLength(_, 0))));
    for snapshot in &observed.storage {
        assert_eq!(snapshot.guards_before, snapshot.guards_after);
    }
}

#[test]
fn composite_reference_preflights_all_scalar_fields_before_writes_or_moves() {
    let (sources, raw) = empty_composition();
    let at = raw.functions[0].blocks[0].statements[7].span;
    let witness = verify_owned(raw, &sources).unwrap();
    let observed = execute::run_array_observed(
        &witness,
        Some(hir::DefId(0)),
        execute::Limits::default(),
        execute::ObservationControl {
            poison_destinations: true,
            fault: Some(execute::FaultInjection {
                at,
                kind: execute::FaultKind::ConstructorLastScalarType,
            }),
            ..Default::default()
        },
    );
    assert!(observed.fault_applied);
    assert!(
        matches!(observed.result,Err(execute::OwnedRunFailure::Invariant("construction type",Some(span))) if span==at)
    );
    assert!(!observed.storage.iter().any(|row| row.kind
        == execute::StorageObservationKind::Construction
        && row.key.owner == 2));
    for (owner, state) in [(0, 2), (1, 2), (2, 1)] {
        let row = observed
            .storage
            .iter()
            .find(|row| {
                row.kind == execute::StorageObservationKind::Failure && row.key.owner == owner
            })
            .unwrap();
        assert_eq!(row.state, state);
        if owner == 2 {
            assert!(row.bytes.iter().all(|byte| *byte == 0));
        }
    }
}

#[test]
fn composite_reference_projection_guards_reject_stale_or_suspended_whole_roots() {
    for operation in [3, 4, 6, 7, 9] {
        for kind in [
            execute::FaultKind::StaleOwnerActivation,
            execute::FaultKind::StaleOwnerGeneration,
            execute::FaultKind::StaleLoanActivation,
            execute::FaultKind::StaleLoanInstance,
            execute::FaultKind::SuspendedReferencePermission,
        ] {
            let (sources, raw) = batch();
            let at = raw.functions[2].blocks[0].statements[operation].span;
            let witness = verify_owned(raw, &sources).unwrap();
            let observed = execute::run_array_observed(
                &witness,
                Some(hir::DefId(0)),
                execute::Limits::default(),
                execute::ObservationControl {
                    fault: Some(execute::FaultInjection { at, kind }),
                    ..Default::default()
                },
            );
            assert!(observed.fault_applied, "{kind:?}, operation {operation}");
            assert!(
                matches!(observed.result,Err(execute::OwnedRunFailure::Invariant(_,Some(span))) if span==at),
                "{kind:?}, operation {operation}: {:?}",
                observed.result
            );
            assert_eq!(observed.events.last(), Some(&execute::Event::Charge(at, 1)));
        }
    }
}

#[test]
fn composite_reference_projected_bounds_follow_charge_and_never_store() {
    let (sources, mut raw) = batch();
    let bump = &mut raw.functions[2];
    // Previously computed RHS 2 is valid, but index 3 equals array len().
    let OwnedInstruction::WriteProjection { index, .. } = &mut bump.blocks[0].statements[6].kind
    else {
        panic!()
    };
    *index = Some(raw::operand(3, bump.span));
    let at = bump.blocks[0].statements[6].span;
    let witness = verify_owned(raw, &sources).unwrap();
    let observed = execute::run_array_observed(
        &witness,
        Some(hir::DefId(0)),
        execute::Limits::default(),
        Default::default(),
    );
    assert_eq!(observed.result, Err(execute::OwnedRunFailure::Bounds(at)));
    assert_eq!(observed.events.last(), Some(&execute::Event::Charge(at, 1)));
    assert!(!observed
        .events
        .iter()
        .any(|e| matches!(e, execute::Event::WriteIndex(..))));
    let prefix: usize = observed
        .events
        .iter()
        .filter_map(|e| {
            if let execute::Event::Charge(_, n) = e {
                Some(n)
            } else {
                None
            }
        })
        .sum();
    let unpaid = execute::run_array_observed(
        &witness,
        Some(hir::DefId(0)),
        execute::Limits {
            fuel: prefix - 1,
            ..Default::default()
        },
        Default::default(),
    );
    assert_eq!(
        unpaid.result,
        Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(at)))
    );
    assert!(!unpaid
        .events
        .iter()
        .any(|e| matches!(e,execute::Event::Charge(span,_) if *span==at)));
}

#[test]
fn composite_reference_batch_has_independent_269_fuel_schedule() {
    let (sources, raw) = batch();
    let (_, s) = raw::context();
    let mut schedule = vec![(s(0), 89)]; // 14 scalar slots + 2 snapshots + 24 payload cells + 48 metadata cells.
    schedule.extend((1..=4).map(|n| (s(n), 1)));
    schedule.extend(
        [
            (10, 1),
            (11, 2),
            (12, 1),
            (13, 2),
            (14, 1),
            (15, 4),
            (16, 1),
            (17, 4),
            (18, 1),
            (19, 5),
            (20, 2),
            (21, 2),
            (22, 4),
            (23, 4),
            (30, 2),
            (31, 5),
            (32, 14),
            (101, 9),
            (35, 1),
            (36, 5),
            (37, 5),
            (38, 5),
            (40, 1),
            (41, 1),
            (42, 27),
        ]
        .map(|(n, c)| (s(n), c)),
    );
    schedule.extend((201..=204).map(|n| (s(n), 1)));
    for base in [210, 220, 230] {
        schedule.extend((base..base + 6).map(|n| (s(n), 1)));
    }
    schedule.extend([(s(240), 1), (s(241), 2)]);
    schedule.extend((50..=58).map(|n| (s(n), 1)));
    schedule.extend([(s(59), 5), (s(60), 28)]);
    assert_eq!(schedule.iter().map(|(_, cost)| cost).sum::<usize>(), 269);
    let witness = verify_owned(raw, &sources).unwrap();
    for fuel in 0..=269 {
        let mut events = vec![];
        let result = execute::run_observed(
            &witness,
            hir::DefId(0),
            execute::Limits {
                fuel,
                ..Default::default()
            },
            &mut events,
        );
        let mut remaining = fuel;
        let failure = schedule.iter().find_map(|(span, cost)| {
            if remaining < *cost {
                Some(*span)
            } else {
                remaining -= cost;
                None
            }
        });
        assert_eq!(
            result,
            match failure {
                Some(span) => Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(span))),
                None => Ok(Scalar::I32(324)),
            },
            "fuel {fuel}"
        );
        let charges: Vec<_> = events
            .iter()
            .filter_map(|event| {
                if let execute::Event::Charge(span, cost) = event {
                    Some((*span, *cost))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(charges, schedule[..charges.len()], "fuel {fuel}");
    }
}

#[test]
fn composite_reference_projection_rejects_valid_but_wrong_nominal_root() {
    for operation in [3, 4, 6, 7, 9] {
        let (sources, mut raw) = batch();
        let span = raw.functions[0].span;
        raw.functions[0].owners.push(owner(
            AggregateTy::Record(RecordId(0)),
            OwnerKind::Local { mutable: true },
            span,
        ));
        raw.functions[0].blocks[1].statements.splice(
            0..0,
            [
                raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(8)), span),
                raw::instruction(
                    OwnedInstruction::Construct {
                        destination: OwnerPlaceId(8),
                        fields: vec![(field(0, 0), raw::operand(0, span))],
                    },
                    span,
                ),
            ],
        );
        raw.functions[0].blocks[2].statements.push(raw::instruction(
            OwnedInstruction::StorageEnd(OwnerPlaceId(8)),
            span,
        ));
        let at = raw.functions[2].blocks[0].statements[operation].span;
        let witness = verify_owned(raw, &sources).unwrap();
        let observed = execute::run_array_observed(
            &witness,
            Some(hir::DefId(0)),
            execute::Limits::default(),
            execute::ObservationControl {
                fault: Some(execute::FaultInjection {
                    at,
                    kind: execute::FaultKind::ArrayReferenceDifferentType,
                }),
                ..Default::default()
            },
        );
        assert!(observed.fault_applied);
        assert_eq!(
            observed.result,
            Err(execute::OwnedRunFailure::Invariant(
                "projection base type",
                Some(at)
            ))
        );
        assert_eq!(observed.events.last(), Some(&execute::Event::Charge(at, 1)));
    }
}
