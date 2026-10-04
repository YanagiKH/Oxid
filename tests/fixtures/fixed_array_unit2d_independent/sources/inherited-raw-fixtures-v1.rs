// Exact unchanged raw fixture functions from the independently qualified Unit2C corpus.
// Native tests inherit these raw schedules; no consumer plan derives expectations.
use super::*;

fn env() -> (SourceMap, Span) {
    let mut sources = SourceMap::new();
    let file = sources.add("unit2c-independent.ox".into(), "abc\n".repeat(16384));
    (
        sources,
        Span {
            file,
            start: 0,
            end: 1,
        },
    )
}

fn s(base: Span, n: usize) -> Span {
    Span {
        start: n * 4,
        end: n * 4 + 1,
        ..base
    }
}

fn op(id: usize, span: Span) -> Operand {
    Operand {
        local: LocalId(id),
        span,
    }
}

fn local(ty: hir::Ty, span: Span) -> LocalDecl {
    LocalDecl {
        ty,
        kind: LocalKind::Temporary,
        span,
    }
}

fn ins(kind: OwnedInstruction, span: Span) -> OwnedStatement {
    OwnedStatement {
        kind,
        span,
        diagnostic_origins: None,
    }
}

fn literal(id: usize, value: Scalar, span: Span) -> OwnedStatement {
    let value = match value {
        Scalar::I32(v) => Rvalue::I32(v),
        Scalar::Bool(v) => Rvalue::Bool(v),
        Scalar::Unit => Rvalue::Unit,
    };
    ins(
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            destination: LocalId(id),
            value,
            span,
        })),
        span,
    )
}

fn array(ty: hir::Ty, n: usize) -> AggregateTy {
    AggregateTy::FixedArray(FixedArrayTy::check(ty, n).unwrap())
}

fn own(aggregate: AggregateTy, kind: OwnerKind, span: Span) -> OwnerDecl {
    OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
        kind,
        span,
    }
}

fn function(id: usize, result: ValueTy, span: Span) -> RawOwnedFunction {
    RawOwnedFunction {
        id: hir::DefId(id),
        result,
        span,
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

fn block(statements: Vec<OwnedStatement>, kind: OwnedTerminatorKind, span: Span) -> OwnedBlock {
    OwnedBlock {
        statements,
        span,
        merge: None,
        terminator: Some(OwnedTerminator {
            kind,
            span,
            diagnostic_origins: None,
        }),
    }
}

fn value(ty: hir::Ty, j: usize) -> Scalar {
    match ty {
        hir::Ty::I32 => Scalar::I32(37 * j as i32 - 91),
        hir::Ty::Bool => Scalar::Bool(j.is_multiple_of(2)),
        hir::Ty::Unit => Scalar::Unit,
    }
}

fn replacement(ty: hir::Ty) -> Scalar {
    match ty {
        hir::Ty::I32 => Scalar::I32(-123456789),
        hir::Ty::Bool => Scalar::Bool(false),
        hir::Ty::Unit => Scalar::Unit,
    }
}

fn bytes(ty: hir::Ty, sequence: &[Scalar]) -> Vec<u8> {
    if sequence.is_empty() {
        return vec![0; if ty == hir::Ty::I32 { 4 } else { 1 }];
    }
    let mut out = vec![];
    for v in sequence {
        match v {
            Scalar::I32(v) => out.extend(v.to_le_bytes()),
            Scalar::Bool(v) => out.push(u8::from(*v)),
            Scalar::Unit => out.push(0),
        }
    }
    out
}

fn sequence_fixture(
    ty: hir::Ty,
    n: usize,
    index: i32,
    mode: usize,
) -> (
    SourceMap,
    RawOwnedProgram,
    Vec<(Span, usize)>,
    Vec<Scalar>,
    Span,
) {
    let (sources, base) = env();
    let w = n.max(1);
    let result_ty = if mode == 0 { hir::Ty::I32 } else { ty };
    let mut f = function(0, ValueTy::Scalar(result_ty), s(base, 0));
    f.locals = (0..n).map(|_| local(ty, base)).collect();
    f.locals.extend(
        [hir::Ty::I32, ty, hir::Ty::I32, ty, ty]
            .into_iter()
            .map(|t| local(t, base)),
    );
    f.owners = vec![own(array(ty, n), OwnerKind::Local { mutable: true }, base)];
    let seq: Vec<_> = (0..n).map(|j| value(ty, j)).collect();
    let mut body = vec![];
    let mut schedule = vec![(s(base, 0), n + w + 10)];
    for (j, &v) in seq.iter().enumerate() {
        body.push(literal(j, v, s(base, 100 + j)));
        schedule.push((s(base, 100 + j), 1));
    }
    for (id, v, at) in [
        (n, Scalar::I32(index), 2000),
        (n + 1, replacement(ty), 2001),
    ] {
        body.push(literal(id, v, s(base, at)));
        schedule.push((s(base, at), 1));
    }
    body.push(ins(
        OwnedInstruction::StorageLive(OwnerPlaceId(0)),
        s(base, 2010),
    ));
    schedule.push((s(base, 2010), 1));
    body.push(ins(
        OwnedInstruction::ConstructArray {
            destination: OwnerPlaceId(0),
            elements: (0..n).map(|j| op(j, s(base, 3000 + j))).collect(),
        },
        s(base, 2020),
    ));
    schedule.push((s(base, 2020), 1 + w));
    body.push(ins(
        OwnedInstruction::ArrayLength {
            destination: LocalId(n + 2),
            base: AccessBase::Owner(OwnerPlaceId(0)),
        },
        s(base, 2030),
    ));
    schedule.push((s(base, 2030), 1));
    let access = s(base, 2040);
    if mode == 1 || mode == 3 {
        body.push(ins(
            OwnedInstruction::ReadIndex {
                destination: LocalId(n + 3),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: op(n, s(base, 2041)),
            },
            access,
        ));
        schedule.push((access, 1));
    }
    if mode == 2 || mode == 3 {
        body.push(ins(
            OwnedInstruction::WriteIndex {
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: op(n, s(base, 2051)),
                value: op(n + 1, s(base, 2052)),
            },
            s(base, 2050),
        ));
        schedule.push((s(base, 2050), 1));
        body.push(ins(
            OwnedInstruction::ReadIndex {
                destination: LocalId(n + 4),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: op(n, s(base, 2061)),
            },
            s(base, 2060),
        ));
        schedule.push((s(base, 2060), 1));
    }
    let result = match mode {
        0 => n + 2,
        1 => n + 3,
        _ => n + 4,
    };
    f.blocks.push(block(
        body,
        OwnedTerminatorKind::ReturnScalar(op(result, s(base, 2070))),
        s(base, 2070),
    ));
    schedule.push((s(base, 2070), 1 + w));
    (
        sources,
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        },
        schedule,
        seq,
        if mode == 2 { s(base, 2050) } else { access },
    )
}

fn chain_fixture(ty: hir::Ty, n: usize) -> (SourceMap, RawOwnedProgram, Vec<(Span, usize)>) {
    let (sources, base) = env();
    let a = array(ty, n);
    let w = n.max(1);
    let r = AggregateTy::Record(RecordId(0));
    let mut f = function(0, ValueTy::Scalar(hir::Ty::I32), s(base, 0));
    f.locals = [hir::Ty::I32, ty, ty, ty, hir::Ty::I32]
        .into_iter()
        .map(|t| local(t, base))
        .collect();
    let kinds = [
        OwnerKind::Local { mutable: true },
        OwnerKind::Temporary,
        OwnerKind::Local { mutable: false },
        OwnerKind::Temporary,
        OwnerKind::StagedArgument {
            call: CallSiteId(0),
            argument: 0,
        },
        OwnerKind::CallResult {
            call: CallSiteId(0),
        },
    ];
    for kind in kinds {
        f.owners.push(own(a, kind, base));
        f.owners
            .push(own(r, OwnerKind::Local { mutable: false }, base));
    }
    f.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(8))],
        result: CallResult::Owned(OwnerPlaceId(10)),
        parent: None,
        span: s(base, 60),
    }];
    let mut body = vec![];
    let mut schedule = vec![(s(base, 0), 63 + 6 * w)];
    for (j, v) in [
        Scalar::I32(0x5a5b5c5d),
        value(ty, 0),
        value(ty, 1),
        value(ty, 2),
    ]
    .into_iter()
    .enumerate()
    {
        body.push(literal(j, v, s(base, 1 + j)));
        schedule.push((s(base, 1 + j), 1));
    }
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    for j in 0..6 {
        let id = 2 * j + 1;
        let at = 10 + 2 * j;
        body.push(ins(
            OwnedInstruction::StorageLive(OwnerPlaceId(id)),
            s(base, at),
        ));
        schedule.push((s(base, at), 1));
        body.push(ins(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(id),
                fields: vec![(field, op(0, base))],
            },
            s(base, at + 1),
        ));
        schedule.push((s(base, at + 1), 2));
    }
    for (at, kind, cost) in [
        (30, OwnedInstruction::StorageLive(OwnerPlaceId(0)), 1),
        (
            31,
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(0),
                elements: vec![op(1, base); n],
            },
            1 + w,
        ),
        (32, OwnedInstruction::StorageLive(OwnerPlaceId(4)), 1),
        (
            33,
            OwnedInstruction::MoveInitialize {
                destination: OwnerPlaceId(4),
                source: OwnerPlaceId(0),
            },
            1 + w,
        ),
        (34, OwnedInstruction::StorageLive(OwnerPlaceId(2)), 1),
        (
            35,
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(2),
                elements: vec![op(2, base); n],
            },
            1 + w,
        ),
        (
            36,
            OwnedInstruction::Replace {
                destination: OwnerPlaceId(0),
                source: OwnerPlaceId(2),
            },
            1 + 2 * w,
        ),
        (37, OwnedInstruction::StorageLive(OwnerPlaceId(6)), 1),
        (
            38,
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(6),
                elements: vec![op(3, base); n],
            },
            1 + w,
        ),
        (
            39,
            OwnedInstruction::Replace {
                destination: OwnerPlaceId(0),
                source: OwnerPlaceId(6),
            },
            1 + 2 * w,
        ),
        (60, OwnedInstruction::OpenCall(CallSiteId(0)), 2),
        (
            61,
            OwnedInstruction::PrepareOwned {
                call: CallSiteId(0),
                argument: 0,
                source: OwnerPlaceId(0),
            },
            1 + w,
        ),
    ] {
        body.push(ins(kind, s(base, at)));
        schedule.push((s(base, at), cost));
    }
    f.blocks.push(block(
        body,
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s(base, 62),
    ));
    schedule.push((s(base, 62), 6 + 2 * w));
    let mut g = function(1, ValueTy::Owned(a), s(base, 70));
    g.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
    g.owners = vec![own(a, OwnerKind::Parameter { position: 0 }, base)];
    g.blocks = vec![block(
        vec![],
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
        s(base, 71),
    )];
    schedule.push((s(base, 71), 1 + 2 * w));
    f.blocks.push(block(
        vec![ins(
            OwnedInstruction::ArrayLength {
                destination: LocalId(4),
                base: AccessBase::Owner(OwnerPlaceId(10)),
            },
            s(base, 80),
        )],
        OwnedTerminatorKind::ReturnScalar(op(4, base)),
        s(base, 81),
    ));
    schedule.extend([(s(base, 80), 1), (s(base, 81), 8 + 6 * w)]);
    let record = RawRecordDecl {
        id: RecordId(0),
        span: base,
        fields: vec![RawFieldDecl {
            id: field,
            ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)),
            span: base,
        }],
    };
    (
        sources,
        RawOwnedProgram {
            records: vec![record],
            functions: vec![f, g],
        },
        schedule,
    )
}

fn effects_fixture(failure: usize) -> (SourceMap, RawOwnedProgram, Vec<(Span, usize)>) {
    let (sources, base) = env();
    let a = array(hir::Ty::I32, 1);
    let slot = AggregateSlot::try_from_aggregate(a).unwrap();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::I32), s(base, 0));
    f.locals = (0..4).map(|_| local(hir::Ty::I32, base)).collect();
    f.owners = vec![own(a, OwnerKind::Local { mutable: true }, base)];
    for j in 0..2 {
        f.calls.push(CallDecl {
            target: hir::DefId(j + 1),
            arguments: vec![ArgumentSlot::Borrow(LoanId(j))],
            result: CallResult::Scalar(LocalId(j + 1)),
            parent: None,
            span: s(base, 10 + 10 * j),
        });
        f.loans.push(LoanDecl {
            call: CallSiteId(j),
            argument: 0,
            authority: AccessBase::Owner(OwnerPlaceId(0)),
            kind: BorrowKind::Exclusive,
            aggregate: slot,
            span: s(base, 11 + 10 * j),
        });
    }
    let mut schedule = vec![
        (s(base, 0), 40),
        (s(base, 1), 1),
        (s(base, 2), 1),
        (s(base, 3), 2),
    ];
    let mut body = vec![
        literal(0, Scalar::I32(17), s(base, 1)),
        ins(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(base, 2)),
        ins(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(0),
                elements: vec![op(0, base)],
            },
            s(base, 3),
        ),
    ];
    for j in 0..2 {
        let at = 10 + 10 * j;
        body.extend([
            ins(OwnedInstruction::OpenCall(CallSiteId(j)), s(base, at)),
            ins(
                OwnedInstruction::PrepareBorrow {
                    call: CallSiteId(j),
                    argument: 0,
                    loan: LoanId(j),
                },
                s(base, at + 1),
            ),
        ]);
        f.blocks.push(block(
            body,
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(j),
                continuation: BlockId(j + 1),
            },
            s(base, at + 2),
        ));
        schedule.extend([
            (s(base, at), 1),
            (s(base, at + 1), 1),
            (s(base, at + 2), 14),
        ]);
        let h = 100 + 100 * j;
        schedule.extend([
            (s(base, h + 1), 1),
            (s(base, h + 2), 1),
            (s(base, h + 3), 1),
            (s(base, h + 4), 1),
        ]);
        if failure == j + 1 {
            schedule.push((s(base, h + 5), 1));
        } else {
            schedule.push((s(base, h + 6), 2));
        }
        body = vec![];
    }
    f.blocks.push(block(
        vec![
            ins(
                OwnedInstruction::WriteIndex {
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    index: op(2, base),
                    value: op(1, base),
                },
                s(base, 30),
            ),
            ins(
                OwnedInstruction::ReadIndex {
                    destination: LocalId(3),
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    index: op(2, base),
                },
                s(base, 31),
            ),
        ],
        OwnedTerminatorKind::ReturnScalar(op(3, base)),
        s(base, 32),
    ));
    schedule.extend([(s(base, 30), 1), (s(base, 31), 1), (s(base, 32), 6)]);
    let mut functions = vec![f];
    for j in 0..2 {
        let at = 100 + 100 * j;
        let mut g = function(j + 1, ValueTy::Scalar(hir::Ty::I32), s(base, at));
        g.locals = (0..4).map(|_| local(hir::Ty::I32, base)).collect();
        g.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
        g.references = vec![ReferenceDecl {
            aggregate: slot,
            kind: BorrowKind::Exclusive,
            position: 0,
            span: base,
        }];
        let result = if failure == j + 1 || (j == 1 && failure == 3) {
            -1
        } else if j == 0 {
            77
        } else {
            0
        };
        let mut body = vec![
            literal(0, Scalar::I32(0), s(base, at + 1)),
            literal(
                1,
                Scalar::I32(if j == 0 { 31 } else { 47 }),
                s(base, at + 2),
            ),
            ins(
                OwnedInstruction::WriteIndex {
                    base: AccessBase::Parameter(ReferenceParamId(0)),
                    index: op(0, base),
                    value: op(1, base),
                },
                s(base, at + 3),
            ),
            literal(2, Scalar::I32(result), s(base, at + 4)),
        ];
        if failure == j + 1 {
            body.push(ins(
                OwnedInstruction::ReadIndex {
                    destination: LocalId(3),
                    base: AccessBase::Parameter(ReferenceParamId(0)),
                    index: op(2, base),
                },
                s(base, at + 5),
            ));
        }
        g.blocks.push(block(
            body,
            OwnedTerminatorKind::ReturnScalar(op(2, base)),
            s(base, at + 6),
        ));
        functions.push(g);
    }
    (
        sources,
        RawOwnedProgram {
            records: vec![],
            functions,
        },
        schedule,
    )
}

fn reference_fixture(
    ty: hir::Ty,
    n: usize,
    mode: usize,
    other: AggregateTy,
    negative: bool,
) -> (SourceMap, RawOwnedProgram, Span) {
    let (sources, mut raw, _) = effects_fixture(0);
    let base = raw.functions[0].span;
    let a = array(ty, n);
    let (other_ty, other_n) = match other {
        AggregateTy::FixedArray(t) => (t.element(), t.length()),
        AggregateTy::Record(_) => (hir::Ty::I32, 1),
    };
    let f = &mut raw.functions[0];
    f.calls.truncate(1);
    f.loans.truncate(1);
    f.owners[0].aggregate = AggregateSlot::try_from_aggregate(a).unwrap();
    f.loans[0].aggregate = AggregateSlot::try_from_aggregate(a).unwrap();
    f.locals[0].ty = ty;
    f.locals.push(local(other_ty, base));
    f.owners
        .push(own(other, OwnerKind::Local { mutable: true }, base));
    f.blocks[0].statements[0] = literal(0, value(ty, 0), s(base, 1));
    f.blocks[0].statements[2] = ins(
        OwnedInstruction::ConstructArray {
            destination: OwnerPlaceId(0),
            elements: vec![op(0, base); n],
        },
        s(base, 3),
    );
    let other_constructor = match other {
        AggregateTy::FixedArray(_) => OwnedInstruction::ConstructArray {
            destination: OwnerPlaceId(1),
            elements: vec![op(4, base); other_n],
        },
        AggregateTy::Record(record) => {
            let field = FieldId { record, index: 0 };
            raw.records = vec![RawRecordDecl {
                id: record,
                span: base,
                fields: vec![RawFieldDecl {
                    id: field,
                    ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)),
                    span: base,
                }],
            }];
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(1),
                fields: vec![(field, op(4, base))],
            }
        }
    };
    f.blocks[0].statements.splice(
        3..3,
        [
            literal(4, value(other_ty, 1), s(base, 4)),
            ins(OwnedInstruction::StorageLive(OwnerPlaceId(1)), s(base, 5)),
            ins(other_constructor, s(base, 6)),
        ],
    );
    f.blocks.truncate(1);
    f.blocks.push(block(
        vec![],
        OwnedTerminatorKind::ReturnScalar(op(1, base)),
        s(base, 40),
    ));
    raw.functions.truncate(2);
    let g = &mut raw.functions[1];
    g.references[0].aggregate = AggregateSlot::try_from_aggregate(a).unwrap();
    g.locals[1].ty = ty;
    g.locals[3].ty = if mode == 2 { hir::Ty::I32 } else { ty };
    g.blocks[0].statements[0] =
        literal(0, Scalar::I32(if negative { -1 } else { 0 }), s(base, 101));
    g.blocks[0].statements[1] = literal(1, replacement(ty), s(base, 102));
    g.blocks[0].statements[2] = ins(
        match mode {
            0 => OwnedInstruction::ReadIndex {
                destination: LocalId(3),
                base: AccessBase::Parameter(ReferenceParamId(0)),
                index: op(0, base),
            },
            1 => OwnedInstruction::WriteIndex {
                base: AccessBase::Parameter(ReferenceParamId(0)),
                index: op(0, base),
                value: op(1, base),
            },
            _ => OwnedInstruction::ArrayLength {
                destination: LocalId(3),
                base: AccessBase::Parameter(ReferenceParamId(0)),
            },
        },
        s(base, 103),
    );
    (sources, raw, s(base, 103))
}
