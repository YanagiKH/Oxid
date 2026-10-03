// Independently frozen Unit2D qualification artifacts, mechanically concatenated.
use super::*;

// BEGIN inherited-raw-fixtures-v1.rs
// Exact unchanged raw fixture functions from the independently qualified Unit2C corpus.
// Native tests inherit these raw schedules; no consumer plan derives expectations.

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

// END inherited-raw-fixtures-v1.rs

// BEGIN native-core-fixtures-v1.rs
// Independent native scalar/index model, frozen before candidate observation.
// This file contains raw adapters and expected scalar sequences only.

fn native_core_value(ty: hir::Ty, ordinal: usize) -> Scalar {
    match ty {
        hir::Ty::Bool => Scalar::Bool(!ordinal.is_multiple_of(2)),
        hir::Ty::I32 => Scalar::I32(
            (if ordinal.is_multiple_of(2) { 1 } else { -1 }) * (31 + 17 * ordinal as i32),
        ),
        hir::Ty::Unit => Scalar::Unit,
    }
}
fn native_core_replacement(ty: hir::Ty) -> Scalar {
    match ty {
        hir::Ty::Bool => Scalar::Bool(true),
        hir::Ty::I32 => Scalar::I32(i32::MIN),
        hir::Ty::Unit => Scalar::Unit,
    }
}
// Mode0 length, mode1 read, mode2 write followed by read. N1024 uses two
// repeated immutable operands; it never asks native admission for1024 locals.
fn native_core_fixture(
    ty: hir::Ty,
    n: usize,
    index: i32,
    mode: usize,
) -> (
    SourceMap,
    RawOwnedProgram,
    Option<Scalar>,
    Span,
    Vec<(Span, usize)>,
) {
    let mut sources = SourceMap::new();
    let file = sources.add("unit2d-independent-core.ox".into(), "abcdef\n".repeat(128));
    let at = |i: usize| Span {
        file,
        start: i * 7,
        end: i * 7 + 6,
    };
    let literal_count = if n > 4 { 2 } else { n };
    let aggregate = AggregateTy::FixedArray(FixedArrayTy::check(ty, n).unwrap());
    let mut f = fixtures::function(
        0,
        ValueTy::Scalar(if mode == 0 { hir::Ty::I32 } else { ty }),
        at(0),
    );
    f.locals = (0..literal_count)
        .map(|_| fixtures::scalar(ty, at(0)))
        .collect();
    f.locals.extend(
        [hir::Ty::I32, ty, hir::Ty::I32, ty]
            .into_iter()
            .map(|t| fixtures::scalar(t, at(0))),
    );
    f.owners = vec![OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
        kind: OwnerKind::Local { mutable: true },
        span: at(0),
    }];
    let index_local = literal_count;
    let replacement_local = literal_count + 1;
    let length_local = literal_count + 2;
    let result_local = literal_count + 3;
    let mut body = vec![];
    let width = n.max(1);
    let mut schedule = vec![(at(0), 1 + (literal_count + 4) + width + 4)];
    let scalar_rvalue = |s| match s {
        Scalar::Bool(v) => Rvalue::Bool(v),
        Scalar::I32(v) => Rvalue::I32(v),
        Scalar::Unit => Rvalue::Unit,
    };
    for j in 0..literal_count {
        body.push(fixtures::assign(
            j,
            scalar_rvalue(native_core_value(ty, j)),
            at(1 + j),
        ));
        schedule.push((at(1 + j), 1));
    }
    body.extend([
        fixtures::assign(index_local, Rvalue::I32(index), at(10)),
        fixtures::assign(
            replacement_local,
            scalar_rvalue(native_core_replacement(ty)),
            at(11),
        ),
        fixtures::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), at(12)),
        fixtures::instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(0),
                elements: (0..n)
                    .map(|j| fixtures::operand(if n > 4 { j % 2 } else { j }, at(13)))
                    .collect(),
            },
            at(14),
        ),
    ]);
    schedule.extend([(at(10), 1), (at(11), 1), (at(12), 1), (at(14), 1 + width)]);
    let access = at(if mode == 0 {
        15
    } else if mode == 1 {
        16
    } else {
        17
    });
    if mode == 0 {
        body.push(fixtures::instruction(
            OwnedInstruction::ArrayLength {
                destination: LocalId(length_local),
                base: AccessBase::Owner(OwnerPlaceId(0)),
            },
            access,
        ));
        schedule.push((access, 1));
    } else {
        if mode == 2 {
            body.push(fixtures::instruction(
                OwnedInstruction::WriteIndex {
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    index: fixtures::operand(index_local, at(18)),
                    value: fixtures::operand(replacement_local, at(19)),
                },
                access,
            ));
            schedule.push((access, 1));
        }
        let read_span = if mode == 2 { at(20) } else { access };
        body.push(fixtures::instruction(
            OwnedInstruction::ReadIndex {
                destination: LocalId(result_local),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: fixtures::operand(index_local, at(21)),
            },
            read_span,
        ));
        schedule.push((read_span, 1));
    }
    f.blocks = vec![block(
        body,
        OwnedTerminatorKind::ReturnScalar(fixtures::operand(
            if mode == 0 {
                length_local
            } else {
                result_local
            },
            at(22),
        )),
        at(22),
    )];
    schedule.push((at(22), 1 + width));
    let expected = if mode == 0 {
        Some(Scalar::I32(n as i32))
    } else if index >= 0 && (index as usize) < n {
        Some(if mode == 2 {
            native_core_replacement(ty)
        } else {
            native_core_value(
                ty,
                if n > 4 {
                    index as usize % 2
                } else {
                    index as usize
                },
            )
        })
    } else {
        None
    };
    (
        sources,
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        },
        expected,
        access,
        schedule,
    )
}

// END native-core-fixtures-v1.rs

// BEGIN native-chain-extension-v1.rs
// Reviewer-owned extension of the unchanged independently frozen Unit2C chain.
// Adds source-like a=a through a distinct Temporary before argument staging.
fn self_replacement_chain(
    ty: hir::Ty,
    n: usize,
) -> (SourceMap, RawOwnedProgram, Vec<(Span, usize)>) {
    let (sources, mut raw, mut events) = chain_fixture(ty, n);
    let base = raw.functions[0].span;
    let f = &mut raw.functions[0];
    assert_eq!(f.owners.len(), 12);
    f.owners.push(own(array(ty, n), OwnerKind::Temporary, base));
    f.owners.push(own(
        AggregateTy::Record(RecordId(0)),
        OwnerKind::Local { mutable: false },
        base,
    ));
    let at = f.blocks[0]
        .statements
        .iter()
        .position(|x| matches!(x.kind, OwnedInstruction::OpenCall(_)))
        .unwrap();
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    let added = [
        ins(OwnedInstruction::StorageLive(OwnerPlaceId(13)), s(base, 40)),
        ins(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(13),
                fields: vec![(field, op(0, base))],
            },
            s(base, 41),
        ),
        ins(OwnedInstruction::StorageLive(OwnerPlaceId(12)), s(base, 42)),
        ins(
            OwnedInstruction::MoveInitialize {
                destination: OwnerPlaceId(12),
                source: OwnerPlaceId(0),
            },
            s(base, 43),
        ),
        ins(
            OwnedInstruction::Replace {
                destination: OwnerPlaceId(0),
                source: OwnerPlaceId(12),
            },
            s(base, 44),
        ),
    ];
    f.blocks[0].statements.splice(at..at, added);
    let width = n.max(1);
    events[0].1 += width + 9; // extra array+record owner cells and 8 owner-state cells
    let event_at = events.iter().position(|(p, _)| *p == s(base, 60)).unwrap();
    events.splice(
        event_at..event_at,
        [
            (s(base, 40), 1),
            (s(base, 41), 2),
            (s(base, 42), 1),
            (s(base, 43), 1 + width),
            (s(base, 44), 1 + 2 * width),
        ],
    );
    events.last_mut().unwrap().1 += width + 1; // return drops both new owner widths
    (sources, raw, events)
}

// END native-chain-extension-v1.rs

// BEGIN native-phi-fixtures-v1.rs
// Independent raw diamond families: expected result is exactly the selected branch.
// The boolean merge's other input slot is never initialized on that execution.
fn native_phi_fixture(
    pattern: &str,
    take_left: bool,
    guarded: bool,
) -> (SourceMap, RawOwnedProgram) {
    use fixtures::*;
    let (sources, s) = context();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::Bool), s(0));
    let types = [
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::Bool,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
    ];
    f.locals = types.into_iter().map(|t| scalar(t, s(0))).collect();
    f.owners = vec![OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(
            FixedArrayTy::check(hir::Ty::I32, 1).unwrap(),
        ))
        .unwrap(),
        kind: OwnerKind::Local { mutable: false },
        span: s(0),
    }];
    let checked = || {
        assign(
            3,
            Rvalue::CheckedI32 {
                op: hir::ArithmeticOp::Add,
                left: operand(1, s(30)),
                right: operand(2, s(31)),
                operator_span: s(32),
            },
            s(33),
        )
    };
    let read = |destination, at| {
        instruction(
            OwnedInstruction::ReadIndex {
                destination: LocalId(destination),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: operand(1, s(at + 1)),
            },
            s(at),
        )
    };
    let mut left = vec![assign(5, Rvalue::Bool(true), s(20))];
    match pattern {
        "arithmetic_bounds" => left.extend([checked(), read(4, 40)]),
        "bounds_arithmetic" => left.extend([read(4, 40), checked()]),
        "bounds_bounds" => left.extend([read(4, 40), read(8, 50)]),
        "bounds_suffix" => left.extend([read(4, 40), assign(9, Rvalue::I32(-9), s(60))]),
        "no_split" => {}
        _ => panic!("unknown independent phi family"),
    }
    f.blocks = vec![
        block(
            vec![
                assign(0, Rvalue::Bool(take_left), s(1)),
                assign(1, Rvalue::I32(0), s(2)),
                assign(2, Rvalue::I32(7), s(3)),
                assign(10, Rvalue::I32(-1), s(4)),
                instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(5)),
                instruction(
                    OwnedInstruction::ConstructArray {
                        destination: OwnerPlaceId(0),
                        elements: vec![operand(2, s(6))],
                    },
                    s(7),
                ),
            ],
            OwnedTerminatorKind::Branch {
                condition: operand(0, s(8)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
            s(9),
        ),
        block(left, OwnedTerminatorKind::Goto(BlockId(3)), s(70)),
        block(
            vec![assign(6, Rvalue::Bool(false), s(71))],
            OwnedTerminatorKind::Goto(BlockId(3)),
            s(72),
        ),
        block(
            vec![],
            OwnedTerminatorKind::ReturnScalar(operand(7, s(74))),
            s(75),
        ),
    ];
    f.blocks[3].merge = Some(BoolMerge {
        destination: LocalId(7),
        incoming: [
            MergeInput {
                predecessor: BlockId(1),
                value: operand(5, s(73)),
            },
            MergeInput {
                predecessor: BlockId(2),
                value: operand(6, s(73)),
            },
        ],
        operator_span: s(73),
        span: s(73),
    });
    let mut raw = RawOwnedProgram {
        records: vec![],
        functions: vec![f],
    };
    if guarded {
        append_cycle(&mut raw);
    }
    (sources, raw)
}
fn native_untaken_bounds_fixture(
    take_left: bool,
    empty: bool,
    guarded: bool,
) -> (SourceMap, RawOwnedProgram) {
    let (sources, mut raw) = native_phi_fixture("no_split", take_left, guarded);
    let f = &mut raw.functions[0];
    if empty {
        f.owners[0].aggregate = AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(
            FixedArrayTy::check(hir::Ty::I32, 0).unwrap(),
        ))
        .unwrap();
        let OwnedInstruction::ConstructArray { elements, .. } = &mut f.blocks[0].statements[5].kind
        else {
            unreachable!()
        };
        elements.clear();
    }
    let at = Span {
        start: 160,
        end: 161,
        ..f.span
    };
    f.blocks[if take_left { 2 } else { 1 }]
        .statements
        .push(fixtures::instruction(
            OwnedInstruction::ReadIndex {
                destination: LocalId(8),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: fixtures::operand(10, at),
            },
            at,
        ));
    (sources, raw)
}

// END native-phi-fixtures-v1.rs

// BEGIN native-semantic-tests-v1.rs
// Independent expected results use raw fixture sequences and hand-counted schedules.
fn independent_native(raw: RawOwnedProgram, sources: &SourceMap, fuel: usize) -> String {
    verified::probe_array_native(
        raw,
        sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        sources,
        NativeControl {
            fuel,
            ..NativeControl::default()
        },
    )
    .unwrap()
    .result
    .unwrap()
}
fn independent_diagnostic(kind: &str, span: Span, sources: &SourceMap) -> String {
    let (code, stage, message) = match kind {
        "fuel" => ("E0601", "oir-run", "execution fuel exhausted"),
        "bounds" => ("E0606", "oir-owned-run", "array index out of bounds"),
        "overflow" => ("E0604", "oir-run", "checked i32 arithmetic overflow"),
        _ => panic!("independent failure kind"),
    };
    let mut out = format!("error[{code}] ({stage}): {message}\n");
    if let Some(source) = sources.files().get(span.file.0) {
        let t = source.text();
        if span.start <= span.end
            && span.end <= t.len()
            && t.is_char_boundary(span.start)
            && t.is_char_boundary(span.end)
        {
            let before = &t[..span.start];
            let line = 1 + before.bytes().filter(|b| *b == b'\n').count();
            let column = 1 + before.rsplit('\n').next().unwrap().chars().count();
            let path: String = source
                .path()
                .chars()
                .flat_map(|c| {
                    if c.is_control() {
                        c.escape_default().collect::<Vec<_>>()
                    } else {
                        vec![c]
                    }
                })
                .collect();
            out.push_str(&format!("  --> {path}:{line}:{column}\n"));
        }
    }
    out
}
fn independent_run(
    scratch: &Scratch,
    binary: &std::path::Path,
    name: &str,
    args: &[String],
) -> std::process::Output {
    let root = std::path::PathBuf::from(
        std::env::var_os("OXID_UNIT2D_EXEC_EVIDENCE")
            .expect("explicit execution receipt directory"),
    );
    std::fs::create_dir_all(&root).unwrap();
    assert!(!std::fs::read_dir(&scratch.0).unwrap().any(|p| matches!(
        p.unwrap().path().extension().and_then(|x| x.to_str()),
        Some("ll" | "bc" | "c")
    )));
    let result = scratch.run(binary, args);
    std::fs::write(root.join(format!("{name}.stdout")), &result.stdout).unwrap();
    std::fs::write(root.join(format!("{name}.stderr")), &result.stderr).unwrap();
    let quote = crate::frontend::diagnostic::json_string;
    let argv = args.iter().map(|s| quote(s)).collect::<Vec<_>>().join(",");
    let status = result
        .status
        .code()
        .map_or_else(|| "null".to_string(), |n| n.to_string());
    std::fs::write(root.join(format!("{name}.json")),format!("{{\"binary\":{},\"args\":[{argv}],\"cwd\":{},\"env_clear\":true,\"PATH\":{},\"status\":{status}}}\n",quote(binary.to_str().unwrap()),quote(scratch.0.to_str().unwrap()),quote(scratch.0.join("no-tools").to_str().unwrap()))).unwrap();
    result
}
fn independent_assert_output(
    actual: std::process::Output,
    expected: Option<Scalar>,
    error: Option<String>,
) {
    match expected {
        Some(value) => assert_result(actual, &scalar_output(value), b"", 0),
        None => assert_result(actual, b"", error.unwrap().as_bytes(), 1),
    }
}
fn kind_name(ty: hir::Ty) -> &'static str {
    match ty {
        hir::Ty::I32 => "i32",
        hir::Ty::Bool => "bool",
        hir::Ty::Unit => "unit",
    }
}
fn independent_structure_receipt(raw: &RawOwnedProgram, n: usize, name: &str, guarded: bool) {
    use std::fmt::Write;
    let root = std::path::PathBuf::from(std::env::var_os("OXID_UNIT2D_EXEC_EVIDENCE").unwrap());
    std::fs::create_dir_all(&root).unwrap();
    let mut text = String::from("function\tprefix\tlength\tguarded\n");
    for f in &raw.functions {
        for (b, block) in f.blocks.iter().enumerate() {
            for (i, statement) in block.statements.iter().enumerate() {
                if matches!(
                    statement.kind,
                    OwnedInstruction::ReadIndex { .. } | OwnedInstruction::WriteIndex { .. }
                ) {
                    writeln!(
                        text,
                        "__oxid_owned_fn_{}\tf{}_b{b}_i{i}\t{n}\t{guarded}",
                        f.id.0, f.id.0
                    )
                    .unwrap();
                }
            }
        }
    }
    std::fs::write(root.join(format!("{name}.structure.tsv")), text).unwrap();
}
#[test]
#[ignore = "independent Unit2D pinned LLVM source-free ELF gate"]
fn independent_unit2d_core_and_width_llvm() {
    let scratch = Scratch::new();
    let mut core = 0;
    let mut wide = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize, 1, 2, 3, 4, 1024] {
            let mut indexes = if n == 1024 {
                vec![i32::MIN, -1, 0, 1023, 1024, i32::MAX]
            } else {
                let mut xs = vec![i32::MIN, -1];
                xs.extend(0..=n as i32);
                xs.push(i32::MAX);
                xs
            };
            indexes.sort();
            indexes.dedup();
            for mode in 0..=2 {
                for &index in if mode == 0 {
                    &indexes[0..1]
                } else {
                    &indexes[..]
                } {
                    let (sources, raw, expected, access, _) =
                        native_core_fixture(ty, n, index, mode);
                    let name = format!("core-{}-n{n}-m{mode}-i{index}", kind_name(ty));
                    independent_structure_receipt(&raw, n, &name, false);
                    let module = independent_native(raw, &sources, 1_000_000);
                    let binary = scratch.compile(&module, &name);
                    independent_assert_output(
                        independent_run(&scratch, &binary, &name, &[]),
                        expected,
                        Some(independent_diagnostic("bounds", access, &sources)),
                    );
                    if n == 1024 {
                        wide += 1
                    } else {
                        core += 1
                    }
                }
            }
        }
    }
    assert_eq!((core, wide), (195, 39));
    eprintln!("independent native core inputs={core}, maximum-width inputs={wide}; all real LLVM/source-free ELF");
}
#[test]
#[ignore = "independent Unit2D pinned LLVM source-free ELF gate"]
fn independent_unit2d_phi_and_untaken_llvm() {
    let scratch = Scratch::new();
    let mut diamonds = 0;
    let mut untaken = 0;
    for pattern in [
        "arithmetic_bounds",
        "bounds_arithmetic",
        "bounds_bounds",
        "bounds_suffix",
        "no_split",
    ] {
        for take in [false, true] {
            for guarded in [false, true] {
                let (sources, raw) = native_phi_fixture(pattern, take, guarded);
                let name = format!("phi-{pattern}-t{take}-g{guarded}");
                independent_structure_receipt(&raw, 1, &name, guarded);
                let module = independent_native(raw, &sources, 1_000_000);
                let binary = scratch.compile(&module, &name);
                independent_assert_output(
                    independent_run(&scratch, &binary, &name, &[]),
                    Some(Scalar::Bool(take)),
                    None,
                );
                diamonds += 1;
            }
        }
    }
    for take in [false, true] {
        for empty in [false, true] {
            for guarded in [false, true] {
                let (sources, raw) = native_untaken_bounds_fixture(take, empty, guarded);
                let name = format!("untaken-t{take}-empty{empty}-g{guarded}");
                independent_structure_receipt(&raw, usize::from(!empty), &name, guarded);
                let module = independent_native(raw, &sources, 1_000_000);
                let binary = scratch.compile(&module, &name);
                independent_assert_output(
                    independent_run(&scratch, &binary, &name, &[]),
                    Some(Scalar::Bool(take)),
                    None,
                );
                untaken += 1;
            }
        }
    }
    assert_eq!((diamonds, untaken), (20, 8));
    eprintln!("independent native phi diamonds={diamonds}, untaken invalid bounds={untaken}");
}
#[test]
#[ignore = "independent Unit2D pinned LLVM source-free ELF gate"]
fn independent_unit2d_transfer_chains_llvm() {
    let scratch = Scratch::new();
    let mut count = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize, 1, 4, 1024] {
            let (sources, raw, _) = chain_fixture(ty, n);
            let name = format!("chain-{}-n{n}", kind_name(ty));
            let module = independent_native(raw, &sources, 1_000_000);
            let binary = scratch.compile(&module, &name);
            independent_assert_output(
                independent_run(&scratch, &binary, &name, &[]),
                Some(Scalar::I32(n as i32)),
                None,
            );
            count += 1;
        }
        for n in [0usize, 4] {
            let (sources, raw, _) = self_replacement_chain(ty, n);
            let name = format!("selfchain-{}-n{n}", kind_name(ty));
            let module = independent_native(raw, &sources, 1_000_000);
            let binary = scratch.compile(&module, &name);
            independent_assert_output(
                independent_run(&scratch, &binary, &name, &[]),
                Some(Scalar::I32(n as i32)),
                None,
            );
            count += 1;
        }
    }
    assert_eq!(count, 18);
    eprintln!("independent native whole-transfer chain programs={count}; physical storage checks separate");
}
#[test]
#[ignore = "independent Unit2D pinned LLVM source-free ELF gate"]
fn independent_unit2d_core_fuel_llvm() {
    let scratch = Scratch::new();
    let mut profiles = 0;
    let traces = [
        (hir::Ty::I32, 0, 0, 1),
        (hir::Ty::I32, 1, 0, 1),
        (hir::Ty::I32, 1, 0, 2),
        (hir::Ty::I32, 1, -1, 2),
        (hir::Ty::Bool, 4, 3, 2),
        (hir::Ty::Unit, 4, 3, 1),
        (hir::Ty::Bool, 0, 0, 0),
        (hir::Ty::I32, 0, 0, 0),
        (hir::Ty::Unit, 0, 0, 0),
    ];
    for (case, (ty, n, index, mode)) in traces.into_iter().enumerate() {
        let (sources, mut raw, expected, access, mut schedule) =
            native_core_fixture(ty, n, index, mode);
        append_cycle(&mut raw);
        if expected.is_none() {
            let last = schedule.iter().position(|(at, _)| *at == access).unwrap() + 1;
            schedule.truncate(last);
        }
        let total: usize = schedule.iter().map(|x| x.1).sum();
        let name = format!("fuel-core-{case}");
        independent_structure_receipt(&raw, n, &name, true);
        let module = independent_native(raw, &sources, total);
        std::fs::write(
            std::path::PathBuf::from(std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE").unwrap())
                .join(format!("{name}-production.ll")),
            &module,
        )
        .unwrap();
        let harness = argv_fuel_harness(&module, total);
        let binary = scratch.compile(&harness, &name);
        for fuel in 0..=total {
            let mut remaining = fuel;
            let mut unpaid = None;
            for &(at, cost) in &schedule {
                if remaining < cost {
                    unpaid = Some(at);
                    break;
                }
                remaining -= cost;
            }
            let output = independent_run(
                &scratch,
                &binary,
                &format!("{name}-budget{fuel}"),
                &[fuel.to_string()],
            );
            if let Some(at) = unpaid {
                independent_assert_output(
                    output,
                    None,
                    Some(independent_diagnostic("fuel", at, &sources)),
                );
            } else {
                independent_assert_output(
                    output,
                    expected,
                    Some(independent_diagnostic("bounds", access, &sources)),
                );
            }
            profiles += 1;
        }
    }
    eprintln!("independent native core all-budget profiles={profiles}, traces=9");
}

// END native-semantic-tests-v1.rs

// BEGIN native-effect-tests-v1.rs
// Observation-only store readback. Every inserted load follows the untouched
// production i32 store, so its four bytes are initialized. No behavior is replaced.
fn independent_effect_observer(module: &str) -> String {
    let sites = ["f0_b0_i2", "f1_b0_i2", "f2_b0_i2", "f0_b2_i0"];
    let mut result = module.to_string();
    let mut insertions = vec![];
    for (ordinal, prefix) in sites.into_iter().enumerate() {
        let found: Vec<_> = module
            .lines()
            .filter(|line| {
                line.trim_start().starts_with("store i32 ") && line.contains(&format!("%{prefix}_"))
            })
            .collect();
        assert_eq!(found.len(), 1, "one production store for {prefix}");
        let line = found[0];
        let pointer = line
            .split_once(", ptr ")
            .unwrap()
            .1
            .split(',')
            .next()
            .unwrap();
        let inserted = format!(
            "  %__independent_effect_{ordinal} = load i32, ptr {pointer}, align 1\n  call i32 @__oxid_print_i32(i32 %__independent_effect_{ordinal})\n"
        );
        let marker = format!("{line}\n");
        assert_eq!(result.matches(&marker).count(), 1);
        result = result.replacen(&marker, &format!("{marker}{inserted}"), 1);
        insertions.push(inserted);
    }
    let mut reconstructed = result.clone();
    for inserted in insertions {
        assert_eq!(reconstructed.matches(&inserted).count(), 1);
        reconstructed = reconstructed.replacen(&inserted, "", 1);
    }
    assert_eq!(
        reconstructed, module,
        "every original operation body remains byte-exact"
    );
    result
}
#[test]
#[ignore = "independent Unit2D pinned LLVM helper-effect and all-budget gate"]
fn independent_unit2d_helper_effects_all_budgets_llvm() {
    let scratch = Scratch::new();
    let evidence =
        std::path::PathBuf::from(std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE").unwrap());
    let mut profiles = 0;
    for failure in 0..4 {
        let (sources, mut raw, mut schedule) = effects_fixture(failure);
        let base = raw.functions[0].span;
        let bounds = match failure {
            1 => Some(s(base, 105)),
            2 => Some(s(base, 205)),
            3 => Some(s(base, 30)),
            _ => None,
        };
        if let Some(at) = bounds {
            schedule.truncate(schedule.iter().position(|event| event.0 == at).unwrap() + 1);
        }
        let total: usize = schedule.iter().map(|event| event.1).sum();
        if failure == 0 {
            assert_eq!(total, 96);
        }
        if failure == 3 {
            assert_eq!(total, 89);
        }
        append_cycle(&mut raw);
        let name = format!("effects-{failure}");
        independent_structure_receipt(&raw, 1, &name, true);
        let module = independent_native(raw, &sources, total);
        std::fs::write(evidence.join(format!("{name}-production.ll")), &module).unwrap();
        let production = scratch.compile(&module, &format!("{name}-production"));
        independent_assert_output(
            independent_run(&scratch, &production, &format!("{name}-production"), &[]),
            bounds.is_none().then_some(Scalar::I32(77)),
            bounds.map(|at| independent_diagnostic("bounds", at, &sources)),
        );
        let plain = scratch.compile(&argv_fuel_harness(&module, total), &format!("{name}-argv"));
        let observation = independent_effect_observer(&module);
        std::fs::write(
            evidence.join(format!("{name}-observation.ll")),
            &observation,
        )
        .unwrap();
        let observed = scratch.compile(
            &argv_fuel_harness(&observation, total),
            &format!("{name}-observed-argv"),
        );
        for fuel in 0..=total {
            let mut remaining = fuel;
            let mut unpaid = None;
            let mut paid = vec![];
            for &(at, cost) in &schedule {
                if remaining < cost {
                    unpaid = Some(at);
                    break;
                }
                remaining -= cost;
                paid.push(at);
            }
            let error = unpaid
                .map(|at| independent_diagnostic("fuel", at, &sources))
                .or_else(|| bounds.map(|at| independent_diagnostic("bounds", at, &sources)));
            let plain_output = independent_run(
                &scratch,
                &plain,
                &format!("{name}-plain-budget{fuel}"),
                &[fuel.to_string()],
            );
            independent_assert_output(
                plain_output,
                error.is_none().then_some(Scalar::I32(77)),
                error.clone(),
            );
            let mut stdout = String::new();
            for (at, value, valid) in [
                (3, 17, true),
                (103, 31, true),
                (203, 47, true),
                (30, 77, failure != 3),
            ] {
                if valid && paid.contains(&s(base, at)) {
                    stdout.push_str(&format!("{value}\n"));
                }
            }
            if error.is_none() {
                stdout.push_str("77\n");
            }
            let observed_output = independent_run(
                &scratch,
                &observed,
                &format!("{name}-observed-budget{fuel}"),
                &[fuel.to_string()],
            );
            assert_result(
                observed_output,
                stdout.as_bytes(),
                error.as_deref().unwrap_or("").as_bytes(),
                i32::from(error.is_some()),
            );
            profiles += 1;
        }
    }
    assert_eq!(profiles, 341);
    eprintln!("independent helper effects: 341 budgets in each of ordinary and readback-observation ELF, plus 4 untouched production wrappers");
}

// END native-effect-tests-v1.rs

// BEGIN native-identity-tests-v1.rs
#[test]
fn independent_unit2d_full_identity_and_closed_production_gate() {
    let mut identities: Vec<_> = [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit]
        .into_iter()
        .flat_map(|ty| [0usize, 1, 4].into_iter().map(move |n| array(ty, n)))
        .collect();
    identities.push(AggregateTy::Record(RecordId(0)));
    let mut valid = 0;
    let mut denied = 0;
    let mut production_denials = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize, 1, 4] {
            for &expected in &identities {
                let (sources, mut raw, _) = chain_fixture(ty, n);
                let call_span = s(raw.functions[0].span, 60);
                raw.functions[1].owners[0].aggregate =
                    AggregateSlot::try_from_aggregate(expected).unwrap();
                raw.functions[1].result = ValueTy::Owned(expected);
                let result = verified::probe_array_native(
                    raw,
                    &sources,
                    budget::Limits::DEFAULT,
                    Some(hir::DefId(0)),
                    &sources,
                    NativeControl {
                        fail_after: (expected != array(ty, n)).then_some(0),
                        ..NativeControl::default()
                    },
                );
                if expected == array(ty, n) {
                    assert!(result.unwrap().result.is_ok());
                    valid += 1;
                } else {
                    let error = match result {
                        Err(e) => e,
                        Ok(_) => panic!("wrong full aggregate identity entered native consumer"),
                    };
                    assert_eq!(error.kind, OwnedFailureKind::Malformed(Malformed::Type));
                    assert_eq!(error.primary.get(), Some(call_span));
                    denied += 1;
                }
            }
            let (sources, raw, _) = chain_fixture(ty, n);
            let error = verified::verify_owned(raw, &sources).unwrap_err();
            assert_eq!(
                error.kind,
                OwnedFailureKind::Malformed(Malformed::UnsupportedArray)
            );
            production_denials += 1;
        }
    }
    assert_eq!((valid, denied, production_denials), (9, 81, 9));
    eprintln!("independent native identity: 9 valid pairs,81 wrong T/N/nominal pairs denied before native allocation;9 distinct production gate controls");
}
#[test]
#[ignore = "independent Unit2D cross-file structural identity real LLVM gate"]
fn independent_unit2d_cross_file_identity_llvm() {
    let (mut sources, mut raw, _) = chain_fixture(hir::Ty::I32, 4);
    let file = sources.add("same-array-another-file.ox".into(), "abc\n".repeat(16384));
    let callee = &mut raw.functions[1];
    callee.span.file = file;
    callee.owners[0].span.file = file;
    callee.blocks[0].span.file = file;
    callee.blocks[0].terminator.as_mut().unwrap().span.file = file;
    let module = independent_native(raw, &sources, 1_000_000);
    let scratch = Scratch::new();
    let binary = scratch.compile(&module, "cross-file-identity");
    independent_assert_output(
        independent_run(&scratch, &binary, "cross-file-identity", &[]),
        Some(Scalar::I32(4)),
        None,
    );
}

// END native-identity-tests-v1.rs

// BEGIN native-work-tests-v1.rs
fn independent_wide_array_loop(replacements: usize) -> (SourceMap, RawOwnedProgram) {
    use fixtures::*;
    let (sources, s) = context();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::Unit), s(0));
    f.locals = vec![scalar(hir::Ty::I32, s(0))];
    let aggregate = AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(
        FixedArrayTy::check(hir::Ty::I32, 64).unwrap(),
    ))
    .unwrap();
    f.owners = vec![
        OwnerDecl {
            aggregate,
            kind: OwnerKind::Local { mutable: true },
            span: s(0)
        };
        2
    ];
    let mut body = vec![assign(0, Rvalue::I32(17), s(0))];
    for owner in 0..2 {
        body.push(instruction(
            OwnedInstruction::StorageLive(OwnerPlaceId(owner)),
            s(0),
        ));
        body.push(instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(owner),
                elements: vec![operand(0, s(0)); 64],
            },
            s(0),
        ));
    }
    f.blocks
        .push(block(body, OwnedTerminatorKind::Goto(BlockId(1)), s(0)));
    f.blocks.push(block(
        (0..replacements)
            .map(|i| {
                instruction(
                    OwnedInstruction::Replace {
                        source: OwnerPlaceId(i % 2),
                        destination: OwnerPlaceId((i + 1) % 2),
                    },
                    s(1),
                )
            })
            .collect(),
        OwnedTerminatorKind::Goto(BlockId(1)),
        s(1),
    ));
    (
        sources,
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        },
    )
}
#[test]
fn independent_unit2d_expansion_identity_and_bounded_prefix() {
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize, 1, 4, 1024] {
            let (sources, raw, _) = chain_fixture(ty, n);
            let observation = verified::probe_array_native(
                raw,
                &sources,
                budget::Limits::DEFAULT,
                Some(hir::DefId(0)),
                &sources,
                NativeControl::default(),
            )
            .unwrap();
            let module = observation.result.unwrap();
            let metrics = observation.metrics;
            let expected_cells = 9 * n.max(1) + 6; // nine array writes/transfers plus six one-field guard constructors
            assert_eq!(metrics.transfer_cells, expected_cells);
            assert_eq!(metrics.message_bytes, 0); // acyclic, no arithmetic/index sites; length has no bounds diagnostic
            assert_eq!(metrics.count_expansions, expected_cells);
            assert_eq!(metrics.render_expansions, expected_cells);
            assert_eq!(metrics.count_bytes, module.len());
            assert_eq!(metrics.render_bytes, module.len());
        }
    }
    let mut prefixes = vec![];
    for replacements in [32usize, 32768] {
        let mut observations = vec![];
        for cap in [1024usize, 8192, 65536] {
            let (sources, raw) = independent_wide_array_loop(replacements);
            let observation = verified::probe_array_native(
                raw,
                &sources,
                budget::Limits::DEFAULT,
                Some(hir::DefId(0)),
                &sources,
                NativeControl {
                    limits: Limits {
                        ir_bytes: cap,
                        ..Limits::DEFAULT
                    },
                    fail_after: Some(5),
                    ..NativeControl::default()
                },
            )
            .unwrap();
            let error = observation.result.unwrap_err();
            assert_eq!(error.code, "E0700");
            assert!(error.message.contains("LLVM bytes"));
            let metrics = observation.metrics;
            assert_eq!(metrics.occurrences, replacements + 8);
            assert_eq!(metrics.unique, 2);
            assert_eq!(metrics.allocation_attempts, 5); // three inventories plus two unique messages; no LLVM allocation
            assert_eq!(metrics.failed_allocation, None);
            assert!(metrics.count_bytes <= cap);
            assert!(metrics.count_expansions <= cap + 1);
            assert_eq!(metrics.render_bytes, 0);
            assert_eq!(metrics.render_expansions, 0);
            assert_eq!(metrics.message_bytes, 164);
            if cap == 65536 {
                assert!(
                    metrics.count_expansions > 164 + 128,
                    "must pass both constructor expansions after diagnostic escaping"
                );
            }
            observations.push((
                metrics.count_bytes,
                metrics.count_expansions,
                metrics.count_ordinary_visits,
                metrics.count_predecessor_visits,
            ));
        }
        prefixes.push(observations);
    }
    assert_eq!(
        prefixes[0], prefixes[1],
        "an exhausted emitter must not visit the oversized suffix"
    );
    eprintln!("independent native expansion:12 exact site inventories;6 lowered-cap profiles with identical short/long prefixes: {:?}",prefixes[0]);
}

// END native-work-tests-v1.rs

// BEGIN native-external-observer-tests-v1.rs
#[test]
#[ignore = "independent Unit2D physical storage and mutation ELF gate"]
fn independent_unit2d_external_storage_observer_llvm() {
    let directory = std::path::PathBuf::from(
        std::env::var_os("OXID_UNIT2D_EXTERNAL_OBSERVERS")
            .expect("frozen reviewer observer directory"),
    );
    let manifest = std::fs::read_to_string(directory.join("manifest.tsv")).unwrap();
    let mut lines = manifest.lines();
    assert_eq!(
        lines.next(),
        Some("case\tmodule\tstatus\tstdout_hex\tstderr_hex")
    );
    let decode = |text: &str| -> Vec<u8> {
        assert!(text.len().is_multiple_of(2));
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    };
    let scratch = Scratch::new();
    let mut cases = 0;
    for line in lines {
        let cells: Vec<_> = line.split('\t').collect();
        assert_eq!(cells.len(), 5);
        assert!(!cells[1].contains("..") && !std::path::Path::new(cells[1]).is_absolute());
        let module = std::fs::read_to_string(directory.join(cells[1])).unwrap();
        let name = format!("physical-{}", cells[0]);
        let binary = scratch.compile(&module, &name);
        let result = independent_run(&scratch, &binary, &name, &[]);
        assert_result(
            result,
            &decode(cells[3]),
            &decode(cells[4]),
            cells[2].parse().unwrap(),
        );
        cases += 1;
    }
    assert!(cases > 0);
    eprintln!(
        "independent physical/sensitivity ELF cases={cases}; exact manifest expectation checked"
    );
}

// END native-external-observer-tests-v1.rs

// BEGIN native-authority-tests-v1.rs
fn independent_alias_fixture(shared: bool) -> (SourceMap, RawOwnedProgram, Span) {
    let (sources, mut raw, _) =
        reference_fixture(hir::Ty::I32, 1, 0, array(hir::Ty::I32, 1), false);
    let base = raw.functions[0].span;
    let kind = if shared {
        BorrowKind::Shared
    } else {
        BorrowKind::Exclusive
    };
    let second = s(base, 90);
    let f = &mut raw.functions[0];
    f.loans[0].kind = kind;
    f.calls[0].arguments.push(ArgumentSlot::Borrow(LoanId(1)));
    f.loans.push(LoanDecl {
        call: CallSiteId(0),
        argument: 1,
        authority: AccessBase::Owner(OwnerPlaceId(0)),
        kind,
        aggregate: AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 1)).unwrap(),
        span: second,
    });
    f.blocks[0].statements.push(ins(
        OwnedInstruction::PrepareBorrow {
            call: CallSiteId(0),
            argument: 1,
            loan: LoanId(1),
        },
        second,
    ));
    let g = &mut raw.functions[1];
    g.references[0].kind = kind;
    g.references.push(ReferenceDecl {
        aggregate: AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 1)).unwrap(),
        kind,
        position: 1,
        span: second,
    });
    g.parameters
        .push(ParameterBinding::Reference(ReferenceParamId(1)));
    (sources, raw, second)
}
fn independent_expect_raw_denial(
    raw: RawOwnedProgram,
    sources: &SourceMap,
    kind: OwnedFailureKind,
    span: Span,
) {
    let result = verified::probe_array_native(
        raw,
        sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        sources,
        NativeControl {
            fail_after: Some(0),
            ..NativeControl::default()
        },
    );
    let failure = match result {
        Err(failure) => failure,
        Ok(_) => panic!("invalid raw input reached native work"),
    };
    assert_eq!(failure.kind, kind);
    assert_eq!(failure.primary.get(), Some(span));
}
#[test]
fn independent_unit2d_raw_authority_denials_precede_native_work() {
    // Zero length is still a full read in U and M states.
    for moved in [false, true] {
        let (sources, mut raw, _, _, _) = native_core_fixture(hir::Ty::I32, 0, 0, 0);
        let body = &mut raw.functions[0].blocks[0].statements;
        let length_at = body.last().unwrap().span;
        if moved {
            let at = Span {
                start: 210,
                end: 216,
                ..length_at
            };
            body.insert(
                4,
                fixtures::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), at),
            );
        } else {
            body.swap(3, 4); // Retain the canonical constructor site, but read while U.
        }
        independent_expect_raw_denial(
            raw,
            &sources,
            OwnedFailureKind::Ownership(Violation::Unavailable),
            length_at,
        );
    }
    // A matching shared signature cannot authorize a write through that parameter.
    let (sources, mut raw, _) = effects_fixture(0);
    let write_at = raw.functions[1].blocks[0].statements[2].span;
    raw.functions[0].loans[0].kind = BorrowKind::Shared;
    raw.functions[1].references[0].kind = BorrowKind::Shared;
    independent_expect_raw_denial(
        raw,
        &sources,
        OwnedFailureKind::Ownership(Violation::Permission),
        write_at,
    );
    // The second exclusive argument is acquired immediately, before invocation.
    let (sources, raw, second) = independent_alias_fixture(false);
    independent_expect_raw_denial(
        raw,
        &sources,
        OwnedFailureKind::Ownership(Violation::LoanConflict),
        second,
    );
    // A pending exclusive argument also blocks N0 length at the caller.
    let (sources, mut raw, _) =
        reference_fixture(hir::Ty::I32, 0, 2, array(hir::Ty::I32, 1), false);
    let at = s(raw.functions[0].span, 91);
    raw.functions[0].blocks[0].statements.push(ins(
        OwnedInstruction::ArrayLength {
            destination: LocalId(3),
            base: AccessBase::Owner(OwnerPlaceId(0)),
        },
        at,
    ));
    independent_expect_raw_denial(
        raw,
        &sources,
        OwnedFailureKind::Ownership(Violation::LoanConflict),
        at,
    );
    // Missing/excess construction elements and bad scalar IDs are raw-shape failures.
    let (sources, mut raw, _, _, _) = native_core_fixture(hir::Ty::I32, 1, 0, 1);
    let statement = raw.functions[0].blocks[0]
        .statements
        .iter_mut()
        .find(|s| matches!(s.kind, OwnedInstruction::ConstructArray { .. }))
        .unwrap();
    let at = statement.span;
    let OwnedInstruction::ConstructArray { elements, .. } = &mut statement.kind else {
        unreachable!()
    };
    elements.push(elements[0]);
    independent_expect_raw_denial(
        raw,
        &sources,
        OwnedFailureKind::Malformed(Malformed::Type),
        at,
    );
    let (sources, mut raw, _, _, _) = native_core_fixture(hir::Ty::I32, 1, 0, 1);
    let statement = raw.functions[0].blocks[0]
        .statements
        .iter_mut()
        .find(|s| matches!(s.kind, OwnedInstruction::ReadIndex { .. }))
        .unwrap();
    let OwnedInstruction::ReadIndex { index, .. } = &mut statement.kind else {
        unreachable!()
    };
    index.local = LocalId(999);
    let at = index.span;
    independent_expect_raw_denial(
        raw,
        &sources,
        OwnedFailureKind::Malformed(Malformed::Id),
        at,
    );
    // Equal shared roots remain legal; this is a separate positive alias control.
    let (sources, raw, _) = independent_alias_fixture(true);
    assert!(verified::probe_array_native(
        raw,
        &sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        &sources,
        NativeControl::default()
    )
    .unwrap()
    .result
    .is_ok());
    eprintln!("independent raw/native boundary: 7 unavailable/permission/alias/malformed denials and 1 legal shared-alias emission");
}
#[test]
#[ignore = "independent Unit2D shared-alias source-free ELF gate"]
fn independent_unit2d_shared_aliases_llvm() {
    let (sources, raw, _) = independent_alias_fixture(true);
    let module = independent_native(raw, &sources, 1_000_000);
    let scratch = Scratch::new();
    let binary = scratch.compile(&module, "shared-array-aliases");
    independent_assert_output(
        independent_run(&scratch, &binary, "shared-array-aliases", &[]),
        Some(Scalar::I32(77)),
        None,
    );
}

// END native-authority-tests-v1.rs

// BEGIN native-extreme-tests-v1.rs
#[test]
#[ignore = "independent Unit2D extreme payload representation LLVM gate"]
fn independent_unit2d_payload_extremes_llvm() {
    let scratch = Scratch::new();
    for (label, value) in [("min", i32::MIN), ("max", i32::MAX)] {
        let (sources, mut raw, _, _, _) = native_core_fixture(hir::Ty::I32, 1, 0, 1);
        let OwnedInstruction::Scalar(Statement::Assign(assign)) =
            &mut raw.functions[0].blocks[0].statements[0].kind
        else {
            unreachable!()
        };
        assign.value = Rvalue::I32(value);
        let module = independent_native(raw, &sources, 1_000_000);
        let name = format!("payload-extreme-construct-{label}");
        let binary = scratch.compile(&module, &name);
        independent_assert_output(
            independent_run(&scratch, &binary, &name, &[]),
            Some(Scalar::I32(value)),
            None,
        );

        let (sources, mut raw, _) = chain_fixture(hir::Ty::I32, 4);
        let base = raw.functions[0].span;
        let f = &mut raw.functions[0];
        let OwnedInstruction::Scalar(Statement::Assign(assign)) =
            &mut f.blocks[0].statements[3].kind
        else {
            unreachable!()
        };
        assert_eq!(assign.destination, LocalId(3));
        assign.value = Rvalue::I32(value);
        f.locals
            .extend([local(hir::Ty::I32, base), local(hir::Ty::I32, base)]);
        f.blocks[1].statements.extend([
            literal(5, Scalar::I32(3), s(base, 82)),
            ins(
                OwnedInstruction::ReadIndex {
                    destination: LocalId(6),
                    base: AccessBase::Owner(OwnerPlaceId(10)),
                    index: op(5, s(base, 83)),
                },
                s(base, 84),
            ),
        ]);
        f.blocks[1].terminator.as_mut().unwrap().kind =
            OwnedTerminatorKind::ReturnScalar(op(6, s(base, 85)));
        let module = independent_native(raw, &sources, 1_000_000);
        let name = format!("payload-extreme-transfer-{label}");
        let binary = scratch.compile(&module, &name);
        independent_assert_output(
            independent_run(&scratch, &binary, &name, &[]),
            Some(Scalar::I32(value)),
            None,
        );
    }
    eprintln!("independent extreme payloads: MIN/MAX through constructor-read and complete transfer/call chain, four source-free ELF cases");
}

// END native-extreme-tests-v1.rs
