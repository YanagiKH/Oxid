//! Test-only raw programs and frozen, independently hand-counted fuel schedules.
//! This module does not import production plan or operation-cost helpers.
use super::*;

pub(super) struct Schedule {
    pub result: Scalar,
    pub entry: hir::DefId,
    pub events: Vec<(Span, usize)>,
}
impl Schedule {
    pub fn fuel(&self) -> usize {
        self.events.iter().map(|e| e.1).sum()
    }
    pub fn failure(&self, mut fuel: usize) -> Option<Span> {
        for &(span, cost) in &self.events {
            if fuel < cost {
                return Some(span);
            }
            fuel -= cost;
        }
        None
    }
}
pub(super) fn context() -> (SourceMap, impl Fn(usize) -> Span) {
    let mut sources = SourceMap::new();
    let file = sources.add("raw-owned-consumers.ox".into(), "x\n".repeat(4096));
    (sources, move |i| Span {
        file,
        start: i * 2,
        end: i * 2 + 1,
    })
}
pub(super) fn scalar(ty: hir::Ty, span: Span) -> LocalDecl {
    LocalDecl {
        ty,
        kind: LocalKind::Temporary,
        span,
    }
}
pub(super) fn operand(local: usize, span: Span) -> Operand {
    Operand {
        local: LocalId(local),
        span,
    }
}
pub(super) fn instruction(kind: OwnedInstruction, span: Span) -> OwnedStatement {
    OwnedStatement {
        diagnostic_origins: None,
        kind,
        span,
    }
}
pub(super) fn assign(destination: usize, value: Rvalue, span: Span) -> OwnedStatement {
    instruction(
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            destination: LocalId(destination),
            value,
            span,
        })),
        span,
    )
}
pub(super) fn end(kind: OwnedTerminatorKind, span: Span) -> Option<OwnedTerminator> {
    Some(OwnedTerminator {
        diagnostic_origins: None,
        kind,
        span,
    })
}
pub(super) fn function(id: usize, result: ValueTy, span: Span) -> RawOwnedFunction {
    RawOwnedFunction {
        id: hir::DefId(id),
        span,
        result,
        parameters: vec![],
        locals: vec![],
        places: vec![],
        owners: vec![],
        references: vec![],
        calls: vec![],
        loans: vec![],
        matches: Vec::new(),
        entry: BlockId(0),
        blocks: vec![],
    }
}
pub(super) fn record(types: &[hir::Ty], span: Span) -> RawRecordDecl {
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
pub(super) fn owner(kind: OwnerKind, span: Span) -> OwnerDecl {
    OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
        kind,
        span,
    }
}
fn block(statements: Vec<OwnedStatement>, kind: OwnedTerminatorKind, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: end(kind, span),
    }
}

pub(super) fn empty_record() -> (SourceMap, RawOwnedProgram, Schedule) {
    let (sources, s) = context();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::Unit), s(0));
    f.locals = vec![scalar(hir::Ty::Unit, s(0))];
    f.owners = vec![owner(OwnerKind::Local { mutable: false }, s(0))];
    f.blocks.push(block(
        vec![
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(1)),
            instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(0),
                    fields: vec![],
                },
                s(2),
            ),
            instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s(3)),
            instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(4)),
            assign(0, Rvalue::Unit, s(5)),
        ],
        OwnedTerminatorKind::ReturnScalar(operand(0, s(6))),
        s(6),
    ));
    let events = [7, 1, 2, 2, 2, 1, 2]
        .into_iter()
        .enumerate()
        .map(|(i, cost)| (s(i), cost))
        .collect();
    (
        sources,
        RawOwnedProgram {
            enums: vec![],
            records: vec![record(&[], s(0))],
            functions: vec![f],
        },
        Schedule {
            result: Scalar::Unit,
            entry: hir::DefId(0),
            events,
        },
    )
}
pub(super) fn owned_relay() -> (SourceMap, RawOwnedProgram, Schedule) {
    let (sources, s) = context();
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    let mut f = function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    f.locals = vec![scalar(hir::Ty::I32, s(1)), scalar(hir::Ty::I32, s(8))];
    f.owners = vec![
        owner(OwnerKind::Local { mutable: false }, s(2)),
        owner(
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            s(4),
        ),
        owner(
            OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            s(6),
        ),
    ];
    f.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(1))],
        result: CallResult::Owned(OwnerPlaceId(2)),
        parent: None,
        span: s(4),
    }];
    f.blocks.push(block(
        vec![
            assign(0, Rvalue::I32(73), s(1)),
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(2)),
            instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(0),
                    fields: vec![(field, operand(0, s(3)))],
                },
                s(3),
            ),
            instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(4)),
            instruction(
                OwnedInstruction::PrepareOwned {
                    call: CallSiteId(0),
                    argument: 0,
                    source: OwnerPlaceId(0),
                },
                s(5),
            ),
        ],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s(6),
    ));
    f.blocks.push(block(
        vec![
            instruction(
                OwnedInstruction::ReadField {
                    destination: LocalId(1),
                    base: AccessBase::Owner(OwnerPlaceId(2)),
                    field,
                },
                s(8),
            ),
            instruction(OwnedInstruction::Discard(OwnerPlaceId(2)), s(9)),
            instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(10)),
            instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(2)), s(11)),
        ],
        OwnedTerminatorKind::ReturnScalar(operand(1, s(12))),
        s(12),
    ));
    let mut callee = function(1, ValueTy::Owned(AggregateTy::Record(RecordId(0))), s(20));
    callee.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
    callee.owners = vec![owner(OwnerKind::Parameter { position: 0 }, s(20))];
    callee.blocks = vec![block(
        vec![],
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
        s(7),
    )];
    let events = [21, 1, 1, 2, 2, 2, 8, 3, 1, 2, 2, 2, 5]
        .into_iter()
        .enumerate()
        .map(|(i, cost)| (s(i), cost))
        .collect();
    (
        sources,
        RawOwnedProgram {
            enums: vec![],
            records: vec![record(&[hir::Ty::I32], s(0))],
            functions: vec![f, callee],
        },
        Schedule {
            result: Scalar::I32(73),
            entry: hir::DefId(0),
            events,
        },
    )
}
pub(super) fn shared_read() -> (SourceMap, RawOwnedProgram, Schedule) {
    let (sources, s) = context();
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    let mut f = function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    f.locals = vec![scalar(hir::Ty::I32, s(1)), scalar(hir::Ty::I32, s(6))];
    f.owners = vec![owner(OwnerKind::Local { mutable: false }, s(2))];
    f.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
        result: CallResult::Scalar(LocalId(1)),
        parent: None,
        span: s(4),
    }];
    f.loans = vec![LoanDecl {
        projection: Vec::new(),
        call: CallSiteId(0),
        argument: 0,
        authority: AccessBase::Owner(OwnerPlaceId(0)),
        kind: BorrowKind::Shared,
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        span: s(5),
    }];
    f.blocks.push(block(
        vec![
            assign(0, Rvalue::I32(-47), s(1)),
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(2)),
            instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(0),
                    fields: vec![(field, operand(0, s(3)))],
                },
                s(3),
            ),
            instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(4)),
            instruction(
                OwnedInstruction::PrepareBorrow {
                    call: CallSiteId(0),
                    argument: 0,
                    loan: LoanId(0),
                },
                s(5),
            ),
        ],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s(6),
    ));
    f.blocks.push(block(
        vec![instruction(
            OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
            s(9),
        )],
        OwnedTerminatorKind::ReturnScalar(operand(1, s(10))),
        s(10),
    ));
    let mut callee = function(1, ValueTy::Scalar(hir::Ty::I32), s(20));
    callee.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
    callee.references = vec![ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        kind: BorrowKind::Shared,
        position: 0,
        span: s(20),
    }];
    callee.locals = vec![scalar(hir::Ty::I32, s(7))];
    callee.blocks.push(block(
        vec![instruction(
            OwnedInstruction::ReadField {
                destination: LocalId(0),
                base: AccessBase::Parameter(ReferenceParamId(0)),
                field,
            },
            s(7),
        )],
        OwnedTerminatorKind::ReturnScalar(operand(0, s(8))),
        s(8),
    ));
    let events = [23, 1, 1, 2, 1, 1, 11, 1, 2, 2, 4]
        .into_iter()
        .enumerate()
        .map(|(i, cost)| (s(i), cost))
        .collect();
    (
        sources,
        RawOwnedProgram {
            enums: vec![],
            records: vec![record(&[hir::Ty::I32], s(0))],
            functions: vec![f, callee],
        },
        Schedule {
            result: Scalar::I32(-47),
            entry: hir::DefId(0),
            events,
        },
    )
}

/// Two shared children of a shared or exclusive incoming capability. The parent
/// reads while the first shared child is held, and exclusive authority resumes.
pub(super) fn shared_children(exclusive: bool) -> (SourceMap, RawOwnedProgram, Schedule) {
    let (sources, mut raw, _) = shared_read();
    let file = raw.functions[0].span.file;
    let s = |i: usize| Span {
        file,
        start: 2 * i,
        end: 2 * i + 1,
    };
    let mode = if exclusive {
        BorrowKind::Exclusive
    } else {
        BorrowKind::Shared
    };
    raw.functions[0].owners[0].kind = OwnerKind::Local { mutable: true };
    raw.functions[0].loans[0].kind = mode;
    let parent = &mut raw.functions[1];
    parent.references[0].kind = mode;
    parent.locals = (0..if exclusive { 3 } else { 2 })
        .map(|_| scalar(hir::Ty::I32, s(20)))
        .collect();
    parent.calls = vec![CallDecl {
        target: hir::DefId(2),
        arguments: vec![
            ArgumentSlot::Borrow(LoanId(0)),
            ArgumentSlot::Borrow(LoanId(1)),
        ],
        result: CallResult::Scalar(LocalId(1)),
        parent: None,
        span: s(21),
    }];
    parent.loans = (0..2)
        .map(|i| LoanDecl {
            projection: Vec::new(),
            call: CallSiteId(0),
            argument: i,
            authority: AccessBase::Parameter(ReferenceParamId(0)),
            kind: BorrowKind::Shared,
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            span: s(if i == 0 { 22 } else { 24 }),
        })
        .collect();
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    parent.blocks = vec![block(
        vec![
            instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(21)),
            instruction(
                OwnedInstruction::PrepareBorrow {
                    call: CallSiteId(0),
                    argument: 0,
                    loan: LoanId(0),
                },
                s(22),
            ),
            instruction(
                OwnedInstruction::ReadField {
                    destination: LocalId(0),
                    base: AccessBase::Parameter(ReferenceParamId(0)),
                    field,
                },
                s(23),
            ),
            instruction(
                OwnedInstruction::PrepareBorrow {
                    call: CallSiteId(0),
                    argument: 1,
                    loan: LoanId(1),
                },
                s(24),
            ),
        ],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s(25),
    )];
    let after = if exclusive {
        vec![
            assign(2, Rvalue::I32(9), s(30)),
            instruction(
                OwnedInstruction::WriteField {
                    base: AccessBase::Parameter(ReferenceParamId(0)),
                    field,
                    value: operand(2, s(31)),
                },
                s(31),
            ),
        ]
    } else {
        vec![]
    };
    parent.blocks.push(block(
        after,
        OwnedTerminatorKind::ReturnScalar(operand(1, s(32))),
        s(32),
    ));
    let mut pair = function(2, ValueTy::Scalar(hir::Ty::I32), s(40));
    pair.locals = (0..3).map(|_| scalar(hir::Ty::I32, s(40))).collect();
    pair.parameters = (0..2)
        .map(|i| ParameterBinding::Reference(ReferenceParamId(i)))
        .collect();
    pair.references = (0..2)
        .map(|position| ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            kind: BorrowKind::Shared,
            position,
            span: s(40),
        })
        .collect();
    pair.blocks = vec![block(
        vec![
            instruction(
                OwnedInstruction::ReadField {
                    destination: LocalId(0),
                    base: AccessBase::Parameter(ReferenceParamId(0)),
                    field,
                },
                s(26),
            ),
            instruction(
                OwnedInstruction::ReadField {
                    destination: LocalId(1),
                    base: AccessBase::Parameter(ReferenceParamId(1)),
                    field,
                },
                s(27),
            ),
            assign(
                2,
                Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Add,
                    left: operand(0, s(28)),
                    right: operand(1, s(28)),
                    operator_span: s(28),
                },
                s(28),
            ),
        ],
        OwnedTerminatorKind::ReturnScalar(operand(2, s(29))),
        s(29),
    )];
    raw.functions.push(pair);
    let mut events = vec![
        (s(0), 23),
        (s(1), 1),
        (s(2), 1),
        (s(3), 2),
        (s(4), 1),
        (s(5), 1),
        (s(6), if exclusive { 41 } else { 40 }),
        (s(21), 1),
        (s(22), 1),
        (s(23), 1),
        (s(24), 1),
        (s(25), 23),
        (s(26), 1),
        (s(27), 1),
        (s(28), 1),
        (s(29), 3),
    ];
    if exclusive {
        events.extend([(s(30), 1), (s(31), 1)]);
    }
    events.extend([(s(32), 5), (s(9), 2), (s(10), 4)]);
    (
        sources,
        raw,
        Schedule {
            result: Scalar::I32(-94),
            entry: hir::DefId(0),
            events,
        },
    )
}

pub(super) fn mixed_relay(result_field: usize) -> (SourceMap, RawOwnedProgram, Schedule) {
    let (sources, mut raw, _) = owned_relay();
    let file = raw.functions[0].span.file;
    let s = |i: usize| Span {
        file,
        start: i * 2,
        end: i * 2 + 1,
    };
    let types = [hir::Ty::Bool, hir::Ty::Unit, hir::Ty::I32];
    raw.records = vec![record(&types, s(0))];
    let f = &mut raw.functions[0];
    f.result = ValueTy::Scalar(types[result_field]);
    f.locals = types
        .into_iter()
        .chain(types)
        .map(|ty| scalar(ty, s(0)))
        .collect();
    f.blocks[0].statements[0] = assign(0, Rvalue::Bool(false), s(1));
    f.blocks[0]
        .statements
        .insert(1, assign(1, Rvalue::Unit, s(13)));
    f.blocks[0]
        .statements
        .insert(2, assign(2, Rvalue::I32(-99), s(14)));
    let OwnedInstruction::Construct { fields, .. } = &mut f.blocks[0].statements[4].kind else {
        unreachable!()
    };
    *fields = (0..3)
        .rev()
        .map(|index| {
            (
                FieldId {
                    record: RecordId(0),
                    index,
                },
                operand(index, s(3)),
            )
        })
        .collect();
    f.blocks[1].statements[0] = instruction(
        OwnedInstruction::ReadField {
            destination: LocalId(3),
            base: AccessBase::Owner(OwnerPlaceId(2)),
            field: FieldId {
                record: RecordId(0),
                index: 0,
            },
        },
        s(8),
    );
    for (at, field, span) in [(1, 1, s(15)), (2, 2, s(16))] {
        f.blocks[1].statements.insert(
            at,
            instruction(
                OwnedInstruction::ReadField {
                    destination: LocalId(field + 3),
                    base: AccessBase::Owner(OwnerPlaceId(2)),
                    field: FieldId {
                        record: RecordId(0),
                        index: field,
                    },
                },
                span,
            ),
        );
    }
    f.blocks[1].terminator = end(
        OwnedTerminatorKind::ReturnScalar(operand(result_field + 3, s(12))),
        s(12),
    );
    let events = vec![
        (s(0), 31),
        (s(1), 1),
        (s(13), 1),
        (s(14), 1),
        (s(2), 1),
        (s(3), 4),
        (s(4), 2),
        (s(5), 4),
        (s(6), 12),
        (s(7), 7),
        (s(8), 1),
        (s(15), 1),
        (s(16), 1),
        (s(9), 4),
        (s(10), 4),
        (s(11), 4),
        (s(12), 11),
    ];
    let result = [Scalar::Bool(false), Scalar::Unit, Scalar::I32(-99)][result_field];
    (
        sources,
        raw,
        Schedule {
            entry: hir::DefId(0),
            result,
            events,
        },
    )
}
pub(super) fn owner_loop() -> (SourceMap, RawOwnedProgram, Schedule) {
    let (sources, s) = context();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    f.locals = [
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::I32,
    ]
    .into_iter()
    .map(|ty| scalar(ty, s(0)))
    .collect();
    f.places = vec![PlaceDecl {
        ty: hir::Ty::I32,
        span: s(4),
    }];
    f.owners = vec![owner(OwnerKind::Local { mutable: false }, s(9))];
    f.blocks.push(block(
        vec![
            assign(0, Rvalue::I32(0), s(1)),
            assign(1, Rvalue::I32(1), s(2)),
            assign(2, Rvalue::I32(3), s(3)),
            instruction(
                OwnedInstruction::Scalar(Statement::Initialize {
                    place: Place {
                        id: PlaceId(0),
                        span: s(4),
                    },
                    value: operand(0, s(4)),
                    span: s(4),
                }),
                s(4),
            ),
        ],
        OwnedTerminatorKind::Goto(BlockId(1)),
        s(5),
    ));
    f.blocks.push(block(
        vec![
            assign(
                3,
                Rvalue::Load(Place {
                    id: PlaceId(0),
                    span: s(6),
                }),
                s(6),
            ),
            assign(
                4,
                Rvalue::CompareScalar {
                    op: hir::ComparisonOp::Less,
                    left: operand(3, s(7)),
                    right: operand(2, s(7)),
                    operator_span: s(7),
                },
                s(7),
            ),
        ],
        OwnedTerminatorKind::Branch {
            condition: operand(4, s(8)),
            then_block: BlockId(2),
            else_block: BlockId(3),
        },
        s(8),
    ));
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    f.blocks.push(block(
        vec![
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(9)),
            instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(0),
                    fields: vec![(field, operand(3, s(10)))],
                },
                s(10),
            ),
            instruction(
                OwnedInstruction::ReadField {
                    destination: LocalId(6),
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    field,
                },
                s(11),
            ),
            instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s(12)),
            instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(13)),
            assign(
                5,
                Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Add,
                    left: operand(6, s(14)),
                    right: operand(1, s(14)),
                    operator_span: s(14),
                },
                s(14),
            ),
            instruction(
                OwnedInstruction::Scalar(Statement::Store {
                    place: Place {
                        id: PlaceId(0),
                        span: s(15),
                    },
                    value: operand(5, s(15)),
                    operator_span: s(15),
                    span: s(15),
                }),
                s(15),
            ),
        ],
        OwnedTerminatorKind::Goto(BlockId(1)),
        s(16),
    ));
    f.blocks.push(block(
        vec![],
        OwnedTerminatorKind::ReturnScalar(operand(3, s(17))),
        s(17),
    ));
    let mut events = vec![(s(0), 14)];
    events.extend((1..=5).map(|i| (s(i), 1)));
    for _ in 0..3 {
        events.extend((6..=8).map(|i| (s(i), 1)));
        events.extend([
            (s(9), 1),
            (s(10), 2),
            (s(11), 1),
            (s(12), 2),
            (s(13), 2),
            (s(14), 1),
            (s(15), 1),
            (s(16), 1),
        ]);
    }
    events.extend((6..=8).map(|i| (s(i), 1)));
    events.push((s(17), 2));
    (
        sources,
        RawOwnedProgram {
            enums: vec![],
            records: vec![record(&[hir::Ty::I32], s(0))],
            functions: vec![f],
        },
        Schedule {
            entry: hir::DefId(0),
            result: Scalar::I32(3),
            events,
        },
    )
}
pub(super) fn replacement(moved: bool) -> (SourceMap, RawOwnedProgram, Schedule) {
    let (sources, s) = context();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    f.locals = (0..3).map(|_| scalar(hir::Ty::I32, s(0))).collect();
    f.owners = vec![
        owner(OwnerKind::Local { mutable: true }, s(3)),
        owner(OwnerKind::Temporary, s(6)),
    ];
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    let mut statements = vec![
        assign(0, Rvalue::I32(5), s(1)),
        assign(1, Rvalue::I32(9), s(2)),
        instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(3)),
        instruction(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(0),
                fields: vec![(field, operand(0, s(4)))],
            },
            s(4),
        ),
    ];
    if moved {
        statements.push(instruction(
            OwnedInstruction::Discard(OwnerPlaceId(0)),
            s(5),
        ));
    }
    statements.extend([
        instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), s(6)),
        instruction(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(1),
                fields: vec![(field, operand(1, s(7)))],
            },
            s(7),
        ),
        instruction(
            OwnedInstruction::Replace {
                destination: OwnerPlaceId(0),
                source: OwnerPlaceId(1),
            },
            s(8),
        ),
        instruction(
            OwnedInstruction::ReadField {
                destination: LocalId(2),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                field,
            },
            s(9),
        ),
        instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(10)),
        instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), s(11)),
    ]);
    f.blocks.push(block(
        statements,
        OwnedTerminatorKind::ReturnScalar(operand(2, s(12))),
        s(12),
    ));
    let mut events = vec![(s(0), 14), (s(1), 1), (s(2), 1), (s(3), 1), (s(4), 2)];
    if moved {
        events.push((s(5), 2));
    }
    events.extend([
        (s(6), 1),
        (s(7), 2),
        (s(8), 3),
        (s(9), 1),
        (s(10), 2),
        (s(11), 2),
        (s(12), 3),
    ]);
    (
        sources,
        RawOwnedProgram {
            enums: vec![],
            records: vec![record(&[hir::Ty::I32], s(0))],
            functions: vec![f],
        },
        Schedule {
            entry: hir::DefId(0),
            result: Scalar::I32(9),
            events,
        },
    )
}

pub(super) fn forbidden_snapshot_redefinition() -> (SourceMap, RawOwnedProgram) {
    let (sources, s) = context();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    f.locals = [
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::I32,
    ]
    .into_iter()
    .map(|ty| scalar(ty, s(0)))
    .collect();
    f.places = vec![
        PlaceDecl {
            ty: hir::Ty::I32,
            span: s(0),
        },
        PlaceDecl {
            ty: hir::Ty::Bool,
            span: s(0),
        },
    ];
    let mut entry = vec![
        assign(0, Rvalue::I32(0), s(0)),
        assign(1, Rvalue::I32(1), s(0)),
        assign(2, Rvalue::Bool(false), s(0)),
        assign(3, Rvalue::Bool(true), s(0)),
    ];
    for (place, value) in [(0, 0), (1, 2)] {
        entry.push(instruction(
            OwnedInstruction::Scalar(Statement::Initialize {
                place: Place {
                    id: PlaceId(place),
                    span: s(0),
                },
                value: operand(value, s(0)),
                span: s(0),
            }),
            s(0),
        ));
    }
    f.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Scalar, ArgumentSlot::Scalar],
        result: CallResult::Scalar(LocalId(6)),
        parent: None,
        span: s(2),
    }];
    f.blocks
        .push(block(entry, OwnedTerminatorKind::Goto(BlockId(1)), s(0)));
    f.blocks.push(block(
        vec![
            assign(
                4,
                Rvalue::Load(Place {
                    id: PlaceId(0),
                    span: s(1),
                }),
                s(1),
            ),
            assign(
                5,
                Rvalue::Load(Place {
                    id: PlaceId(1),
                    span: s(1),
                }),
                s(1),
            ),
        ],
        OwnedTerminatorKind::Branch {
            condition: operand(5, s(1)),
            then_block: BlockId(3),
            else_block: BlockId(2),
        },
        s(1),
    ));
    let mut prepare = vec![
        instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(2)),
        instruction(
            OwnedInstruction::PrepareScalar {
                call: CallSiteId(0),
                argument: 0,
                value: operand(4, s(2)),
            },
            s(2),
        ),
    ];
    for (place, value) in [(0, 1), (1, 3)] {
        prepare.push(instruction(
            OwnedInstruction::Scalar(Statement::Store {
                place: Place {
                    id: PlaceId(place),
                    span: s(2),
                },
                value: operand(value, s(2)),
                operator_span: s(2),
                span: s(2),
            }),
            s(2),
        ));
    }
    f.blocks
        .push(block(prepare, OwnedTerminatorKind::Goto(BlockId(1)), s(2)));
    f.blocks.push(block(
        vec![instruction(
            OwnedInstruction::PrepareScalar {
                call: CallSiteId(0),
                argument: 1,
                value: operand(4, s(3)),
            },
            s(3),
        )],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(4),
        },
        s(3),
    ));
    f.blocks.push(block(
        vec![],
        OwnedTerminatorKind::ReturnScalar(operand(6, s(4))),
        s(4),
    ));
    let mut callee = function(1, ValueTy::Scalar(hir::Ty::I32), s(5));
    callee.locals = (0..2)
        .map(|_| LocalDecl {
            ty: hir::Ty::I32,
            kind: LocalKind::Parameter,
            span: s(5),
        })
        .collect();
    callee.parameters = (0..2)
        .map(|i| ParameterBinding::Scalar(LocalId(i)))
        .collect();
    callee.blocks = vec![block(
        vec![],
        OwnedTerminatorKind::ReturnScalar(operand(0, s(5))),
        s(5),
    )];
    (
        sources,
        RawOwnedProgram {
            enums: vec![],
            records: vec![],
            functions: vec![f, callee],
        },
    )
}

pub(super) fn distinct_owned_results() -> (SourceMap, RawOwnedProgram, Schedule) {
    let (sources, mut raw, _) = owned_relay();
    let file = raw.functions[0].span.file;
    let s = |i: usize| Span {
        file,
        start: 2 * i,
        end: 2 * i + 1,
    };
    let f = &mut raw.functions[0];
    f.locals = (0..5).map(|_| scalar(hir::Ty::I32, s(0))).collect();
    f.owners.extend([
        owner(OwnerKind::Local { mutable: false }, s(13)),
        owner(
            OwnerKind::StagedArgument {
                call: CallSiteId(1),
                argument: 0,
            },
            s(15),
        ),
        owner(
            OwnerKind::CallResult {
                call: CallSiteId(1),
            },
            s(17),
        ),
    ]);
    f.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(4))],
        result: CallResult::Owned(OwnerPlaceId(5)),
        parent: None,
        span: s(15),
    });
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    f.blocks[0]
        .statements
        .insert(1, assign(2, Rvalue::I32(-29), s(18)));
    f.blocks[0].statements.insert(
        4,
        instruction(OwnedInstruction::StorageLive(OwnerPlaceId(3)), s(13)),
    );
    f.blocks[0].statements.insert(
        5,
        instruction(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(3),
                fields: vec![(field, operand(2, s(14)))],
            },
            s(14),
        ),
    );
    f.blocks[1] = block(
        vec![
            instruction(OwnedInstruction::OpenCall(CallSiteId(1)), s(15)),
            instruction(
                OwnedInstruction::PrepareOwned {
                    call: CallSiteId(1),
                    argument: 0,
                    source: OwnerPlaceId(3),
                },
                s(16),
            ),
        ],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(1),
            continuation: BlockId(2),
        },
        s(17),
    );
    let mut tail = vec![
        instruction(
            OwnedInstruction::ReadField {
                destination: LocalId(1),
                base: AccessBase::Owner(OwnerPlaceId(2)),
                field,
            },
            s(8),
        ),
        instruction(
            OwnedInstruction::ReadField {
                destination: LocalId(3),
                base: AccessBase::Owner(OwnerPlaceId(5)),
                field,
            },
            s(19),
        ),
        assign(
            4,
            Rvalue::CheckedI32 {
                op: hir::ArithmeticOp::Add,
                left: operand(1, s(20)),
                right: operand(3, s(20)),
                operator_span: s(20),
            },
            s(20),
        ),
        instruction(OwnedInstruction::Discard(OwnerPlaceId(2)), s(9)),
        instruction(OwnedInstruction::Discard(OwnerPlaceId(5)), s(21)),
    ];
    for (owner, origin) in [(0, 10), (3, 22), (2, 11), (5, 23)] {
        tail.push(instruction(
            OwnedInstruction::StorageEnd(OwnerPlaceId(owner)),
            s(origin),
        ));
    }
    f.blocks.push(block(
        tail,
        OwnedTerminatorKind::ReturnScalar(operand(4, s(12))),
        s(12),
    ));
    let events = vec![
        (s(0), 42),
        (s(1), 1),
        (s(18), 1),
        (s(2), 1),
        (s(3), 2),
        (s(13), 1),
        (s(14), 2),
        (s(4), 2),
        (s(5), 2),
        (s(6), 8),
        (s(7), 3),
        (s(15), 2),
        (s(16), 2),
        (s(17), 8),
        (s(7), 3),
        (s(8), 1),
        (s(19), 1),
        (s(20), 1),
        (s(9), 2),
        (s(21), 2),
        (s(10), 2),
        (s(22), 2),
        (s(11), 2),
        (s(23), 2),
        (s(12), 9),
    ];
    (
        sources,
        raw,
        Schedule {
            entry: hir::DefId(0),
            result: Scalar::I32(44),
            events,
        },
    )
}

/// A pending outer scalar call keeps its first snapshot while its later argument
/// performs three nested mutating calls in a loop. The outer site repeats twice.
pub(super) fn later_argument_loop() -> (SourceMap, RawOwnedProgram, Schedule) {
    let (sources, s) = context();
    use hir::Ty::{Bool, I32};
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    let root = AccessBase::Owner(OwnerPlaceId(0));
    let mut main = function(0, ValueTy::Scalar(I32), s(0));
    main.locals = [
        I32, I32, I32, I32, I32, I32, Bool, I32, I32, Bool, I32, I32, I32, I32, I32, I32, I32, I32,
    ]
    .into_iter()
    .map(|ty| scalar(ty, s(0)))
    .collect();
    main.places = (0..3)
        .map(|_| PlaceDecl {
            ty: I32,
            span: s(0),
        })
        .collect();
    main.owners = vec![owner(OwnerKind::Local { mutable: true }, s(8))];
    main.calls = vec![
        CallDecl {
            target: hir::DefId(2),
            arguments: vec![ArgumentSlot::Scalar, ArgumentSlot::Scalar],
            result: CallResult::Scalar(LocalId(13)),
            parent: None,
            span: s(16),
        },
        CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
            result: CallResult::Scalar(LocalId(10)),
            parent: Some((CallSiteId(0), 1)),
            span: s(23),
        },
    ];
    main.loans = vec![LoanDecl {
        projection: Vec::new(),
        call: CallSiteId(1),
        argument: 0,
        authority: root,
        kind: BorrowKind::Exclusive,
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        span: s(24),
    }];
    let init = |place, value, origin| {
        instruction(
            OwnedInstruction::Scalar(Statement::Initialize {
                place: Place {
                    id: PlaceId(place),
                    span: s(origin),
                },
                value: operand(value, s(origin)),
                span: s(origin),
            }),
            s(origin),
        )
    };
    let store = |place, value, origin| {
        instruction(
            OwnedInstruction::Scalar(Statement::Store {
                place: Place {
                    id: PlaceId(place),
                    span: s(origin),
                },
                value: operand(value, s(origin)),
                span: s(origin),
                operator_span: s(origin),
            }),
            s(origin),
        )
    };
    let add = |destination, left, right, origin| {
        assign(
            destination,
            Rvalue::CheckedI32 {
                op: hir::ArithmeticOp::Add,
                left: operand(left, s(origin)),
                right: operand(right, s(origin)),
                operator_span: s(origin),
            },
            s(origin),
        )
    };
    let less = |destination, left, right, origin| {
        assign(
            destination,
            Rvalue::CompareScalar {
                op: hir::ComparisonOp::Less,
                left: operand(left, s(origin)),
                right: operand(right, s(origin)),
                operator_span: s(origin),
            },
            s(origin),
        )
    };
    let load = |destination, place, origin| {
        assign(
            destination,
            Rvalue::Load(Place {
                id: PlaceId(place),
                span: s(origin),
            }),
            s(origin),
        )
    };
    main.blocks.push(block(
        vec![
            assign(0, Rvalue::I32(0), s(1)),
            assign(1, Rvalue::I32(1), s(2)),
            assign(2, Rvalue::I32(2), s(3)),
            assign(3, Rvalue::I32(3), s(4)),
            assign(4, Rvalue::I32(10), s(5)),
            init(0, 0, 6),
            init(1, 0, 7),
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(8)),
            instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(0),
                    fields: vec![(field, operand(4, s(9)))],
                },
                s(9),
            ),
        ],
        OwnedTerminatorKind::Goto(BlockId(1)),
        s(10),
    ));
    main.blocks.push(block(
        vec![load(5, 1, 11), less(6, 5, 2, 12)],
        OwnedTerminatorKind::Branch {
            condition: operand(6, s(13)),
            then_block: BlockId(2),
            else_block: BlockId(8),
        },
        s(13),
    ));
    main.blocks.push(block(
        vec![
            instruction(
                OwnedInstruction::ReadField {
                    destination: LocalId(7),
                    base: root,
                    field,
                },
                s(15),
            ),
            instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(16)),
            instruction(
                OwnedInstruction::PrepareScalar {
                    call: CallSiteId(0),
                    argument: 0,
                    value: operand(7, s(17)),
                },
                s(17),
            ),
            init(2, 0, 18),
        ],
        OwnedTerminatorKind::Goto(BlockId(3)),
        s(19),
    ));
    main.blocks.push(block(
        vec![load(8, 2, 20), less(9, 8, 3, 21)],
        OwnedTerminatorKind::Branch {
            condition: operand(9, s(22)),
            then_block: BlockId(4),
            else_block: BlockId(6),
        },
        s(22),
    ));
    main.blocks.push(block(
        vec![
            instruction(OwnedInstruction::OpenCall(CallSiteId(1)), s(23)),
            instruction(
                OwnedInstruction::PrepareBorrow {
                    call: CallSiteId(1),
                    argument: 0,
                    loan: LoanId(0),
                },
                s(24),
            ),
        ],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(1),
            continuation: BlockId(5),
        },
        s(25),
    ));
    main.blocks.push(block(
        vec![add(11, 8, 1, 26), store(2, 11, 27)],
        OwnedTerminatorKind::Goto(BlockId(3)),
        s(28),
    ));
    main.blocks.push(block(
        vec![
            instruction(
                OwnedInstruction::ReadField {
                    destination: LocalId(12),
                    base: root,
                    field,
                },
                s(29),
            ),
            instruction(
                OwnedInstruction::PrepareScalar {
                    call: CallSiteId(0),
                    argument: 1,
                    value: operand(12, s(30)),
                },
                s(30),
            ),
        ],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(7),
        },
        s(31),
    ));
    main.blocks.push(block(
        vec![
            load(14, 0, 32),
            add(15, 14, 13, 33),
            store(0, 15, 34),
            add(16, 5, 1, 35),
            store(1, 16, 36),
        ],
        OwnedTerminatorKind::Goto(BlockId(1)),
        s(37),
    ));
    main.blocks.push(block(
        vec![
            load(17, 0, 38),
            instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(39)),
        ],
        OwnedTerminatorKind::ReturnScalar(operand(17, s(40))),
        s(40),
    ));
    let mut increment = function(1, ValueTy::Scalar(I32), s(50));
    increment.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
    increment.references = vec![ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        kind: BorrowKind::Exclusive,
        position: 0,
        span: s(50),
    }];
    increment.locals = (0..3).map(|_| scalar(I32, s(50))).collect();
    increment.blocks.push(block(
        vec![
            instruction(
                OwnedInstruction::ReadField {
                    destination: LocalId(0),
                    base: AccessBase::Parameter(ReferenceParamId(0)),
                    field,
                },
                s(51),
            ),
            assign(1, Rvalue::I32(1), s(52)),
            add(2, 0, 1, 53),
            instruction(
                OwnedInstruction::WriteField {
                    base: AccessBase::Parameter(ReferenceParamId(0)),
                    field,
                    value: operand(2, s(54)),
                },
                s(54),
            ),
        ],
        OwnedTerminatorKind::ReturnScalar(operand(2, s(55))),
        s(55),
    ));
    let mut combine = function(2, ValueTy::Scalar(I32), s(60));
    combine.locals = (0..5)
        .map(|i| LocalDecl {
            ty: I32,
            kind: if i < 2 {
                LocalKind::Parameter
            } else {
                LocalKind::Temporary
            },
            span: s(60),
        })
        .collect();
    combine.parameters = (0..2)
        .map(|i| ParameterBinding::Scalar(LocalId(i)))
        .collect();
    combine.blocks.push(block(
        vec![
            assign(2, Rvalue::I32(100), s(61)),
            assign(
                3,
                Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Multiply,
                    left: operand(0, s(62)),
                    right: operand(2, s(62)),
                    operator_span: s(62),
                },
                s(62),
            ),
            add(4, 3, 1, 63),
        ],
        OwnedTerminatorKind::ReturnScalar(operand(4, s(64))),
        s(64),
    ));
    let mut events = vec![(s(0), 46)];
    events.extend((1..=8).map(|i| (s(i), 1)));
    events.extend([(s(9), 2), (s(10), 1)]);
    for _ in 0..2 {
        events.extend((11..=13).map(|i| (s(i), 1)));
        events.extend((15..=19).map(|i| (s(i), 1)));
        for _ in 0..3 {
            events.extend((20..=24).map(|i| (s(i), 1)));
            events.push((s(25), 13));
            events.extend((51..=54).map(|i| (s(i), 1)));
            events.push((s(55), 2));
            events.extend((26..=28).map(|i| (s(i), 1)));
        }
        events.extend((20..=22).map(|i| (s(i), 1)));
        events.extend([(s(29), 1), (s(30), 1), (s(31), 8)]);
        events.extend((61..=64).map(|i| (s(i), 1)));
        events.extend((32..=37).map(|i| (s(i), 1)));
    }
    events.extend((11..=13).map(|i| (s(i), 1)));
    events.extend([(s(38), 1), (s(39), 2), (s(40), 5)]);
    (
        sources,
        RawOwnedProgram {
            enums: vec![],
            records: vec![record(&[I32], s(0))],
            functions: vec![main, increment, combine],
        },
        Schedule {
            entry: hir::DefId(0),
            result: Scalar::I32(2329),
            events,
        },
    )
}
