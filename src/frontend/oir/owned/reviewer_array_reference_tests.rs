//! Reviewer-owned fixtures. Expected sequence/cost model frozen before implementation.
use super::*;
use execute::{Event, Limits, ObservationControl, OwnedRunFailure, ReferenceObservation};

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
fn observed(
    raw: RawOwnedProgram,
    sources: &SourceMap,
    limits: Limits,
    poison: bool,
) -> ReferenceObservation {
    let control = ObservationControl {
        poison_destinations: poison,
        ..ObservationControl::default()
    };
    let result = verified::probe_array_reference(
        raw,
        sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        limits,
        control,
    )
    .unwrap();
    assert!(
        !result.truncated,
        "independent observation must be complete"
    );
    result
}

// mode0=length only, mode1=read, mode2=write, mode3=read/write/read.
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

#[test]
fn independent_unit2c_ordered_scalars_and_signed_bounds() {
    let mut cases = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize, 1, 4, 17, 1024] {
            let mut indexes = vec![i32::MIN, -1, 0, n as i32 - 1, n as i32, i32::MAX];
            indexes.sort();
            indexes.dedup();
            for index in indexes {
                for mode in [1, 2] {
                    let (sources, raw, _, seq, access) = sequence_fixture(ty, n, index, mode);
                    let observation = observed(raw, &sources, Limits::default(), false);
                    let expected = if index >= 0 && (index as usize) < n {
                        Ok(if mode == 1 {
                            seq[index as usize]
                        } else {
                            replacement(ty)
                        })
                    } else {
                        Err(OwnedRunFailure::Bounds(access))
                    };
                    assert_eq!(
                        observation.result, expected,
                        "T={ty:?},N={n},i={index},mode={mode}"
                    );
                    let mut expected_seq = seq;
                    if mode == 2 && index >= 0 && (index as usize) < n {
                        expected_seq[index as usize] = replacement(ty);
                    }
                    let last = observation.storage.last().expect("terminal owner bytes");
                    assert_eq!(
                        last.bytes,
                        bytes(ty, &expected_seq),
                        "physical T={ty:?},N={n},i={index},mode={mode}"
                    );
                    assert_eq!(last.state, 2);
                    assert_eq!(last.key.generation, 2);
                    assert!(!last.poisoned);
                    cases += 1;
                }
            }
            let (sources, raw, _, _, _) = sequence_fixture(ty, n, 0, 0);
            assert_eq!(
                observed(raw, &sources, Limits::default(), false).result,
                Ok(Scalar::I32(n as i32))
            );
            cases += 1;
        }
    }
    eprintln!("unit2c independent ordered/bounds inputs={cases}");
}

#[test]
fn independent_unit2c_hand_counted_every_small_fuel() {
    let mut profiles = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize, 1, 4] {
            let mode = if n == 0 { 0 } else { 3 };
            let (_, _, schedule, _, _) = sequence_fixture(ty, n, 0, mode);
            let total: usize = schedule.iter().map(|x| x.1).sum();
            assert_eq!(total, if n == 0 { 19 } else { 2 * n + 3 * n + 19 });
            for fuel in 0..=total {
                let (sources, raw, _, _, _) = sequence_fixture(ty, n, 0, mode);
                let actual = observed(
                    raw,
                    &sources,
                    Limits {
                        fuel,
                        ..Limits::default()
                    },
                    false,
                );
                let mut left = fuel;
                let mut paid = vec![];
                let mut failure = None;
                for &(span, cost) in &schedule {
                    if left < cost {
                        failure = Some(span);
                        break;
                    }
                    left -= cost;
                    paid.push((span, cost));
                }
                let expected = match failure {
                    Some(span) => Err(OwnedRunFailure::Scalar(RunFailure::Fuel(span))),
                    None => Ok(if n == 0 {
                        Scalar::I32(0)
                    } else {
                        replacement(ty)
                    }),
                };
                assert_eq!(actual.result, expected, "T={ty:?},N={n},fuel={fuel}");
                assert_eq!(actual.remaining_fuel, left, "T={ty:?},N={n},fuel={fuel}");
                let charges: Vec<_> = actual
                    .events
                    .iter()
                    .filter_map(|e| {
                        if let Event::Charge(span, cost) = e {
                            Some((*span, *cost))
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(charges, paid, "T={ty:?},N={n},fuel={fuel}");
                profiles += 1;
            }
        }
    }
    eprintln!("unit2c independent small-fuel profiles={profiles}");
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

#[test]
fn independent_unit2c_all_whole_transfer_sites_and_costs() {
    let mut cases = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize, 1, 4, 17, 1024] {
            let (sources, raw, schedule) = chain_fixture(ty, n);
            let actual = observed(raw, &sources, Limits::default(), true);
            assert_eq!(actual.result, Ok(Scalar::I32(n as i32)), "T={ty:?},N={n}");
            let charges: Vec<_> = actual
                .events
                .iter()
                .filter_map(|e| {
                    if let Event::Charge(span, cost) = e {
                        Some((*span, *cost))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(charges, schedule, "T={ty:?},N={n}");
            assert_eq!(
                actual
                    .events
                    .iter()
                    .filter(|e| matches!(e, Event::Transfer(..)))
                    .count(),
                6
            );
            let writes: Vec<_> = actual.storage.iter().filter(|x| x.poisoned).collect();
            assert_eq!(
                writes.len(),
                9,
                "one constructor/copy observation per destination"
            );
            let expected_values = [
                value(ty, 0),
                value(ty, 0),
                value(ty, 1),
                value(ty, 1),
                value(ty, 2),
                value(ty, 2),
                value(ty, 2),
                value(ty, 2),
                value(ty, 2),
            ];
            for (snapshot, v) in writes.iter().zip(expected_values) {
                assert_eq!(snapshot.bytes, bytes(ty, &vec![v; n]));
                assert_eq!(snapshot.guards_before, snapshot.guards_after);
            }
            let terminal: Vec<_> = actual
                .storage
                .iter()
                .filter(|x| x.kind == execute::StorageObservationKind::Return && x.key.frame == 0)
                .collect();
            assert_eq!(terminal.len(), 12);
            for snapshot in terminal {
                let id = snapshot.key.owner as usize;
                if id % 2 == 1 {
                    assert_eq!(snapshot.bytes, 0x5a5b5c5di32.to_le_bytes());
                    assert_eq!(snapshot.state, 2);
                } else {
                    let v = if id == 4 {
                        value(ty, 0)
                    } else if id == 2 {
                        value(ty, 1)
                    } else {
                        value(ty, 2)
                    };
                    assert_eq!(snapshot.bytes, bytes(ty, &vec![v; n]));
                }
            }
            cases += 1;
        }
    }
    eprintln!("unit2c independent whole-transfer inputs={cases}");
}

#[test]
fn independent_unit2c_exact_resources_and_allocation_topology() {
    let mut limits_cases = 0;
    let mut allocations = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize, 1, 4, 1024] {
            let a = n.max(1) * if ty == hir::Ty::I32 { 4 } else { 1 };
            let b = 6 * (a.div_ceil(4) * 4 + 4);
            let w = n.max(1);
            let limits = Limits {
                frames: 2,
                slots: 5,
                cells: 66 + 7 * w,
                bytes: 1032 + b + a,
                ..Limits::default()
            };
            for defect in 0..5 {
                let (sources, raw, _) = chain_fixture(ty, n);
                let base = raw.functions[0].span;
                let mut selected = limits;
                let expected = match defect {
                    0 => Ok(Scalar::I32(n as i32)),
                    1 => {
                        selected.frames = 1;
                        Err(OwnedRunFailure::Scalar(RunFailure::Frames(s(base, 62))))
                    }
                    2 => {
                        selected.slots = 4;
                        Err(OwnedRunFailure::Scalar(RunFailure::Slots(s(base, 0))))
                    }
                    3 => {
                        selected.cells -= 1;
                        Err(OwnedRunFailure::Resource(plan::AdmissionFailure {
                            name: "live expanded cells",
                            span: Some(s(base, 62)),
                        }))
                    }
                    _ => {
                        selected.bytes -= 1;
                        Err(OwnedRunFailure::Resource(plan::AdmissionFailure {
                            name: "live requested bytes",
                            span: Some(s(base, 62)),
                        }))
                    }
                };
                assert_eq!(
                    observed(raw, &sources, selected, false).result,
                    expected,
                    "T={ty:?},N={n},defect={defect}"
                );
                limits_cases += 1;
            }
        }
    }
    for n in [0usize, 1, 1024] {
        for fail in 0..=24 {
            let (sources, raw, _) = chain_fixture(hir::Ty::I32, n);
            let base = raw.functions[0].span;
            let actual = plan::fail_allocation_after(fail, || {
                observed(raw, &sources, Limits::default(), false)
            });
            let expected = if fail == 24 {
                Ok(Scalar::I32(n as i32))
            } else {
                Err(OwnedRunFailure::Resource(plan::AdmissionFailure {
                    name: "injected owned allocation failure",
                    span: if fail < 9 {
                        None
                    } else if fail < 17 {
                        Some(s(base, 0))
                    } else {
                        Some(s(base, 62))
                    },
                }))
            };
            assert_eq!(actual.result, expected, "N={n},allocation={fail}");
            allocations += 1;
        }
    }
    eprintln!(
        "unit2c independent resource inputs={limits_cases},allocation profiles={allocations}"
    );
}

#[test]
fn independent_unit2c_bounds_primary_origin_and_entry_gates() {
    let mut inputs = 0;
    for mode in [1usize, 2] {
        for index in [-1i32, 1] {
            let (sources, mut raw, schedule, _, access) =
                sequence_fixture(hir::Ty::I32, 1, index, mode);
            let base = raw.functions[0].span;
            let primary = s(base, 2100);
            let cause = s(base, 2101);
            let statement = raw.functions[0].blocks[0]
                .statements
                .iter_mut()
                .find(|x| x.span == access)
                .unwrap();
            statement.diagnostic_origins = Some(DiagnosticOrigins { primary, cause });
            let position = schedule.iter().position(|x| x.0 == access).unwrap();
            let fuel: usize = schedule[..position].iter().map(|x| x.1).sum();
            let actual = observed(
                raw,
                &sources,
                Limits {
                    fuel: fuel + 1,
                    ..Limits::default()
                },
                false,
            );
            assert_eq!(actual.result, Err(OwnedRunFailure::Bounds(primary)));
            let error = actual.result.unwrap_err();
            let d = error.diagnostic(&sources);
            assert_eq!(
                (d.code, d.stage, d.message.as_str(), d.primary),
                (
                    "E0606",
                    "oir-owned-run",
                    "array index out of bounds",
                    Some(primary)
                )
            );
            let d = error.diagnostic(&SourceMap::new());
            assert_eq!(d.primary, None);
            assert_eq!(
                d.render_human(&SourceMap::new()),
                "error[E0606] (oir-owned-run): array index out of bounds\n"
            );
            let (sources, mut raw, _, _, access) = sequence_fixture(hir::Ty::I32, 1, index, mode);
            raw.functions[0].blocks[0]
                .statements
                .iter_mut()
                .find(|x| x.span == access)
                .unwrap()
                .diagnostic_origins = Some(DiagnosticOrigins { primary, cause });
            assert_eq!(
                observed(
                    raw,
                    &sources,
                    Limits {
                        fuel,
                        ..Limits::default()
                    },
                    false
                )
                .result,
                Err(OwnedRunFailure::Scalar(RunFailure::Fuel(primary)))
            );
            inputs += 2;
        }
    }
    for entry in [None, Some(hir::DefId(usize::MAX)), Some(hir::DefId(0))] {
        let (sources, mut raw, _, _, _) = sequence_fixture(hir::Ty::I32, 0, 0, 0);
        let span = raw.functions[0].span;
        raw.functions[0].result = ValueTy::Owned(array(hir::Ty::I32, 0));
        raw.functions[0].blocks[0].terminator.as_mut().unwrap().kind =
            OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0));
        let outcome = plan::fail_allocation_after(0, || {
            verified::probe_array_reference(
                raw,
                &sources,
                budget::Limits::DEFAULT,
                entry,
                Limits::default(),
                ObservationControl::default(),
            )
            .unwrap()
        });
        let expected = match entry {
            None => OwnedRunFailure::Scalar(RunFailure::Entry(None)),
            Some(hir::DefId(0)) => OwnedRunFailure::EntryResult(span),
            _ => OwnedRunFailure::Invariant("entry identity", None),
        };
        assert_eq!(outcome.result, Err(expected));
        assert!(!outcome.truncated);
        inputs += 1;
    }
    eprintln!("unit2c independent origins/entry inputs={inputs}");
}

// failure0 succeeds, failure1 RHS fails, failure2 index helper fails, failure3 final bounds.
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
            referent: BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap(),
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
            referent: BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap(),
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

#[test]
fn independent_unit2c_helper_effects_precede_final_store() {
    let mut profiles = 0;
    for failure in 0..4 {
        let (_, _, mut schedule) = effects_fixture(failure);
        let (_, raw, _, _, _) = sequence_fixture(hir::Ty::I32, 0, 0, 0);
        let base = raw.functions[0].span;
        let failed = match failure {
            1 => Some(s(base, 105)),
            2 => Some(s(base, 205)),
            3 => Some(s(base, 30)),
            _ => None,
        };
        if let Some(at) = failed {
            let end = schedule.iter().position(|x| x.0 == at).unwrap() + 1;
            schedule.truncate(end);
        }
        let total: usize = schedule.iter().map(|x| x.1).sum();
        if failure == 0 {
            assert_eq!(total, 96);
        }
        if failure == 3 {
            assert_eq!(total, 89);
        }
        for fuel in 0..=total {
            let (sources, raw, _) = effects_fixture(failure);
            let observation = observed(
                raw,
                &sources,
                Limits {
                    fuel,
                    ..Limits::default()
                },
                false,
            );
            let mut left = fuel;
            let mut paid = vec![];
            let mut unpaid = None;
            for &(span, cost) in &schedule {
                if left < cost {
                    unpaid = Some(span);
                    break;
                }
                left -= cost;
                paid.push((span, cost));
            }
            let expected = if let Some(span) = unpaid {
                Err(OwnedRunFailure::Scalar(RunFailure::Fuel(span)))
            } else if let Some(span) = failed {
                Err(OwnedRunFailure::Bounds(span))
            } else {
                Ok(Scalar::I32(77))
            };
            assert_eq!(
                observation.result, expected,
                "failure={failure},fuel={fuel}"
            );
            let charged: Vec<_> = observation
                .events
                .iter()
                .filter_map(|e| {
                    if let Event::Charge(span, cost) = e {
                        Some((*span, *cost))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(charged, paid);
            if let Some(snapshot) = observation.storage.iter().rev().find(|x| {
                x.key.frame == 0
                    && x.key.owner == 0
                    && matches!(
                        x.kind,
                        execute::StorageObservationKind::Return
                            | execute::StorageObservationKind::Failure
                    )
            }) {
                let value: i32 = if paid.iter().any(|x| x.0 == s(base, 30)) && failure != 3 {
                    77
                } else if paid.iter().any(|x| x.0 == s(base, 203)) {
                    47
                } else if paid.iter().any(|x| x.0 == s(base, 103)) {
                    31
                } else if paid.iter().any(|x| x.0 == s(base, 3)) {
                    17
                } else {
                    0
                };
                assert_eq!(
                    snapshot.bytes,
                    value.to_le_bytes(),
                    "failure={failure},fuel={fuel}"
                );
                assert!(!snapshot.poisoned);
            }
            profiles += 1;
        }
    }
    eprintln!("unit2c independent helper-effect fuel profiles={profiles}");
}

fn adapt_one_i32_record(
    mut raw: RawOwnedProgram,
    schedule: &mut consumer_fixtures::Schedule,
) -> RawOwnedProgram {
    let a = array(hir::Ty::I32, 1);
    let slot = AggregateSlot::try_from_aggregate(a).unwrap();
    let mut invocations = vec![];
    let mut added = vec![];
    for f in &mut raw.functions {
        let zero = f.locals.len();
        let span = Span {
            start: 7000 + 2 * f.id.0,
            end: 7001 + 2 * f.id.0,
            ..f.span
        };
        added.push(span);
        f.locals.push(local(hir::Ty::I32, span));
        if matches!(f.result, ValueTy::Owned(_)) {
            f.result = ValueTy::Owned(a);
        }
        for o in &mut f.owners {
            o.aggregate = slot;
        }
        for r in &mut f.references {
            r.referent = BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap();
        }
        for l in &mut f.loans {
            l.referent = BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap();
        }
        for b in &mut f.blocks {
            if let Some(OwnedTerminator {
                kind: OwnedTerminatorKind::Invoke { call, .. },
                span,
                ..
            }) = &b.terminator
            {
                invocations.push((*span, f.calls[call.0].target.0));
            }
            for i in &mut b.statements {
                i.kind = match &i.kind {
                    OwnedInstruction::Construct {
                        destination,
                        fields,
                    } => {
                        assert_eq!(fields.len(), 1);
                        OwnedInstruction::ConstructArray {
                            destination: *destination,
                            elements: vec![fields[0].1],
                        }
                    }
                    OwnedInstruction::ReadField {
                        destination, base, ..
                    } => OwnedInstruction::ReadIndex {
                        destination: *destination,
                        base: *base,
                        index: op(zero, i.span),
                    },
                    OwnedInstruction::WriteField { base, value, .. } => {
                        OwnedInstruction::WriteIndex {
                            base: *base,
                            index: op(zero, i.span),
                            value: *value,
                        }
                    }
                    original => original.clone(),
                };
            }
        }
        f.blocks[f.entry.0]
            .statements
            .insert(0, literal(zero, Scalar::I32(0), span));
    }
    let mut events = vec![];
    for (i, &(span, cost)) in schedule.events.iter().enumerate() {
        if i == 0 {
            events.push((span, cost + 1));
            events.push((added[schedule.entry.0], 1));
        } else if let Some((_, callee)) = invocations.iter().find(|x| x.0 == span) {
            events.push((span, cost + 1));
            events.push((added[*callee], 1));
        } else {
            events.push((span, cost));
        }
    }
    schedule.events = events;
    raw
}

#[test]
fn independent_unit2c_inherited_capability_and_generation_models() {
    let mut profiles = 0;
    for exclusive in [false, true] {
        let (_, raw, mut schedule) = consumer_fixtures::shared_children(exclusive);
        let _ = adapt_one_i32_record(raw, &mut schedule);
        let total = schedule.fuel();
        for fuel in 0..=total {
            let (sources, raw, mut frozen) = consumer_fixtures::shared_children(exclusive);
            let raw = adapt_one_i32_record(raw, &mut frozen);
            let actual = observed(
                raw,
                &sources,
                Limits {
                    fuel,
                    ..Limits::default()
                },
                false,
            );
            let expected = match frozen.failure(fuel) {
                Some(span) => Err(OwnedRunFailure::Scalar(RunFailure::Fuel(span))),
                None => Ok(frozen.result),
            };
            assert_eq!(actual.result, expected, "exclusive={exclusive},fuel={fuel}");
            let charges: Vec<_> = actual
                .events
                .iter()
                .filter_map(|e| {
                    if let Event::Charge(span, cost) = e {
                        Some((*span, *cost))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(charges, frozen.events[..charges.len()]);
            if fuel == total {
                assert_eq!(
                    actual
                        .events
                        .iter()
                        .filter(|e| matches!(e, Event::Acquire(..)))
                        .count(),
                    3
                );
                assert_eq!(
                    actual
                        .events
                        .iter()
                        .filter(|e| matches!(e, Event::Release(..)))
                        .count(),
                    3
                );
            }
            profiles += 1;
        }
    }
    let (sources, raw, mut schedule) = consumer_fixtures::owner_loop();
    let raw = adapt_one_i32_record(raw, &mut schedule);
    let observation = observed(raw, &sources, Limits::default(), false);
    assert_eq!(observation.result, Ok(Scalar::I32(3)));
    let generations: Vec<_> = observation
        .events
        .iter()
        .filter_map(|e| {
            if let Event::ReadIndex(key, _, value) = e {
                Some((key.generation, *value))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        generations,
        vec![
            (2, Scalar::I32(0)),
            (6, Scalar::I32(1)),
            (10, Scalar::I32(2))
        ]
    );
    let charges: Vec<_> = observation
        .events
        .iter()
        .filter_map(|e| {
            if let Event::Charge(span, cost) = e {
                Some((*span, *cost))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(charges, schedule.events);
    eprintln!("unit2c independent inherited capability profiles={profiles},generation programs=1");
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
    f.loans[0].referent = BorrowedSlot::check(BorrowedTy::Exact(a)).unwrap();
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
    g.references[0].referent = BorrowedSlot::check(BorrowedTy::Exact(a)).unwrap();
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
fn fault_observed(
    raw: RawOwnedProgram,
    sources: &SourceMap,
    kind: execute::FaultKind,
    at: Span,
) -> ReferenceObservation {
    let observation = verified::probe_array_reference(
        raw,
        sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        Limits::default(),
        ObservationControl {
            poison_destinations: false,
            fault: Some(execute::FaultInjection { at, kind }),
            ..ObservationControl::default()
        },
    )
    .unwrap();
    assert!(!observation.truncated);
    assert!(observation.fault_applied);
    observation
}

#[test]
fn independent_unit2c_stale_identity_permission_and_full_type_checks() {
    use execute::FaultKind::*;
    let faults = [
        (StaleOwnerActivation, "stale owner activation"),
        (StaleOwnerGeneration, "stale owner generation"),
        (StaleLoanActivation, "stale loan activation"),
        (StaleLoanInstance, "stale loan instance"),
        (SuspendedReferencePermission, "suspended parent permission"),
        (LoanRootMismatch, "loan root mismatch"),
    ];
    let mut inputs = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize, 4] {
            for mode in 0..3 {
                let (sources, raw, at) = reference_fixture(ty, n, mode, array(ty, n), false);
                let expected = if n == 0 && mode != 2 {
                    Err(OwnedRunFailure::Bounds(at))
                } else {
                    Ok(Scalar::I32(77))
                };
                assert_eq!(
                    observed(raw, &sources, Limits::default(), false).result,
                    expected
                );
                inputs += 1;
                for (kind, message) in faults {
                    let (sources, raw, at) = reference_fixture(ty, n, mode, array(ty, n), true);
                    let actual = fault_observed(raw, &sources, kind, at);
                    assert_eq!(
                        actual.result,
                        Err(OwnedRunFailure::Invariant(message, Some(at))),
                        "T={ty:?},N={n},mode={mode},fault={kind:?}"
                    );
                    let terminal = actual
                        .storage
                        .iter()
                        .rev()
                        .find(|x| {
                            x.kind == execute::StorageObservationKind::Failure
                                && x.key.frame == 0
                                && x.key.owner == 0
                        })
                        .unwrap();
                    assert_eq!(terminal.bytes, bytes(ty, &vec![value(ty, 0); n]));
                    inputs += 1;
                }
            }
        }
    }
    for (ty, n, other) in [
        (hir::Ty::I32, 0, array(hir::Ty::I32, 1)),
        (hir::Ty::I32, 1, array(hir::Ty::Bool, 4)),
        (hir::Ty::Bool, 0, array(hir::Ty::Unit, 0)),
        (hir::Ty::Bool, 4, array(hir::Ty::Unit, 4)),
        (hir::Ty::I32, 1, AggregateTy::Record(RecordId(0))),
        (hir::Ty::Unit, 4, array(hir::Ty::I32, 1)),
    ] {
        for mode in 0..3 {
            let (sources, raw, at) = reference_fixture(ty, n, mode, other, true);
            let actual = fault_observed(raw, &sources, ArrayReferenceDifferentType, at);
            assert_eq!(
                actual.result,
                Err(OwnedRunFailure::Invariant("array base type", Some(at))),
                "T={ty:?},N={n},other={other:?},mode={mode}"
            );
            inputs += 1;
        }
        let (sources, raw, _) = reference_fixture(ty, n, 2, other, false);
        let at = s(raw.functions[0].span, 12);
        let actual = fault_observed(raw, &sources, IncomingReferenceDifferentType, at);
        assert_eq!(
            actual.result,
            Err(OwnedRunFailure::Invariant(
                "incoming reference type",
                Some(at)
            ))
        );
        inputs += 1;
    }
    eprintln!("unit2c independent provenance/type/control inputs={inputs}");
}

#[test]
fn independent_unit2c_late_constructor_snapshot_is_atomic() {
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        let (sources, raw, _, _, _) = sequence_fixture(ty, 4, 0, 0);
        let at = s(raw.functions[0].span, 2020);
        let actual = fault_observed(
            raw,
            &sources,
            execute::FaultKind::ConstructorLastScalarType,
            at,
        );
        assert_eq!(
            actual.result,
            Err(OwnedRunFailure::Invariant(
                "array construction type",
                Some(at)
            ))
        );
        let terminal = actual.storage.last().unwrap();
        assert_eq!(terminal.kind, execute::StorageObservationKind::Failure);
        assert_eq!(terminal.state, 1);
        assert_eq!(terminal.key.generation, 1);
        assert_eq!(
            terminal.bytes,
            vec![0; 4 * if ty == hir::Ty::I32 { 4 } else { 1 }]
        );
        assert!(!terminal.poisoned);
    }
    eprintln!("unit2c independent distinct-last-snapshot inputs=3");
}

#[test]
fn independent_unit2c_incoming_exclusive_alias_is_rejected() {
    let (sources, mut raw, _) =
        reference_fixture(hir::Ty::I32, 1, 2, array(hir::Ty::I32, 1), false);
    let base = raw.functions[0].span;
    raw.functions[0].calls[0]
        .arguments
        .push(ArgumentSlot::Borrow(LoanId(1)));
    raw.functions[0].loans.push(LoanDecl {
        call: CallSiteId(0),
        argument: 1,
        authority: AccessBase::Owner(OwnerPlaceId(1)),
        kind: BorrowKind::Exclusive,
        referent: BorrowedSlot::check(BorrowedTy::Exact(array(hir::Ty::I32, 1))).unwrap(),
        span: s(base, 7),
    });
    raw.functions[0].blocks[0].statements.push(ins(
        OwnedInstruction::PrepareBorrow {
            call: CallSiteId(0),
            argument: 1,
            loan: LoanId(1),
        },
        s(base, 7),
    ));
    raw.functions[1]
        .parameters
        .push(ParameterBinding::Reference(ReferenceParamId(1)));
    raw.functions[1].references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(array(hir::Ty::I32, 1))).unwrap(),
        kind: BorrowKind::Exclusive,
        position: 1,
        span: base,
    });
    let actual = fault_observed(
        raw,
        &sources,
        execute::FaultKind::IncomingExclusiveAlias,
        s(base, 12),
    );
    assert_eq!(
        actual.result,
        Err(OwnedRunFailure::Invariant(
            "incoming reference alias contract",
            Some(s(base, 12))
        ))
    );
    assert_eq!(
        actual
            .events
            .iter()
            .filter(|e| matches!(e, Event::Enter(..)))
            .count(),
        1
    );
}

#[test]
fn independent_array_identity_validation_and_production_execution() {
    let mut inputs = 0;
    let mut identities: Vec<_> = [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit]
        .into_iter()
        .flat_map(|ty| [0usize, 1, 4].into_iter().map(move |n| array(ty, n)))
        .collect();
    identities.push(AggregateTy::Record(RecordId(0)));
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize, 1, 4] {
            for expected in &identities {
                let (sources, mut raw, _) = chain_fixture(ty, n);
                let at = s(raw.functions[0].span, 60);
                raw.functions[1].owners[0].aggregate =
                    AggregateSlot::try_from_aggregate(*expected).unwrap();
                raw.functions[1].result = ValueTy::Owned(*expected);
                let result = verified::probe_array_reference(
                    raw,
                    &sources,
                    budget::Limits::DEFAULT,
                    Some(hir::DefId(0)),
                    Limits::default(),
                    ObservationControl::default(),
                );
                if *expected == array(ty, n) {
                    assert_eq!(result.unwrap().result, Ok(Scalar::I32(n as i32)));
                } else {
                    let error = result.unwrap_err();
                    assert_eq!(error.kind, OwnedFailureKind::Malformed(Malformed::Type));
                    assert_eq!(error.primary.get(), Some(at));
                }
                let (sources, raw, _) = chain_fixture(ty, n);
                let witness = verified::verify_owned(raw, &sources).unwrap();
                assert_eq!(
                    execute::run(&witness, Some(hir::DefId(0))).unwrap(),
                    Scalar::I32(n as i32)
                );
                inputs += 1;
            }
        }
    }
    let (mut sources, mut raw, _) = chain_fixture(hir::Ty::I32, 4);
    let file = sources.add(
        "array-same-type-another-file.ox".into(),
        "abc\n".repeat(16384),
    );
    let f = &mut raw.functions[1];
    f.span.file = file;
    f.owners[0].span.file = file;
    f.blocks[0].span.file = file;
    f.blocks[0].terminator.as_mut().unwrap().span.file = file;
    assert_eq!(
        observed(raw, &sources, Limits::default(), false).result,
        Ok(Scalar::I32(4))
    );
    eprintln!("unit2c independent full identity pairs={inputs},source-identity controls=1");
}

// Append inside the frozen independent fixture module. No new executor or witness seam.
#[test]
fn independent_unit2c_observer_reservation_errors_preserve_language_outcome() {
    use execute::ObservationAllocationSite::{Event, Payload, StorageHeader};
    let mut cases = 0;
    for site in [Event, StorageHeader, Payload] {
        for poison in [false, true] {
            let (sources, raw, _, _, _) = sequence_fixture(hir::Ty::I32, 4, 0, 3);
            let actual = verified::probe_array_reference(
                raw,
                &sources,
                budget::Limits::DEFAULT,
                Some(hir::DefId(0)),
                Limits::default(),
                ObservationControl {
                    allocation_failure: Some(site),
                    poison_destinations: poison,
                    ..ObservationControl::default()
                },
            )
            .unwrap();
            assert_eq!(actual.result, Ok(Scalar::I32(-123456789)));
            assert_eq!(actual.remaining_fuel, 1_000_000 - 39);
            assert!(actual.truncated);
            assert!(actual.allocation_fault_applied);
            assert!(!actual.fault_applied);
            assert!(actual.storage.is_empty(), "no partially published snapshot");
            if matches!(site, Event) {
                assert!(actual.events.is_empty());
            }
            cases += 1;
        }
    }
    eprintln!("independent observer reservation-error inputs={cases}");
}

fn observer_scalar_cap_fixture(statements: usize) -> (SourceMap, RawOwnedProgram) {
    let (sources, base) = env();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::Unit), base);
    f.locals = (0..statements)
        .map(|_| local(hir::Ty::Unit, base))
        .collect();
    let body = (0..statements)
        .map(|i| literal(i, Scalar::Unit, s(base, 1)))
        .collect();
    f.blocks = vec![block(
        body,
        OwnedTerminatorKind::ReturnScalar(op(statements - 1, base)),
        s(base, 2),
    )];
    (
        sources,
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        },
    )
}
fn observer_storage_cap_fixture(owners: usize, n: usize) -> (SourceMap, RawOwnedProgram) {
    let (sources, base) = env();
    let mut f = function(0, ValueTy::Scalar(hir::Ty::Unit), base);
    f.locals = vec![local(hir::Ty::I32, base), local(hir::Ty::Unit, base)];
    f.owners = (0..owners)
        .map(|_| {
            own(
                array(hir::Ty::I32, n),
                OwnerKind::Local { mutable: false },
                base,
            )
        })
        .collect();
    let mut body = vec![
        literal(0, Scalar::I32(17), s(base, 1)),
        literal(1, Scalar::Unit, s(base, 2)),
    ];
    for i in 0..owners {
        body.push(ins(
            OwnedInstruction::StorageLive(OwnerPlaceId(i)),
            s(base, 10 + 2 * i),
        ));
        body.push(ins(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(i),
                elements: vec![op(0, base); n],
            },
            s(base, 11 + 2 * i),
        ));
    }
    f.blocks = vec![block(
        body,
        OwnedTerminatorKind::ReturnScalar(op(1, base)),
        s(base, 200),
    )];
    (
        sources,
        RawOwnedProgram {
            records: vec![],
            functions: vec![f],
        },
    )
}

#[test]
fn independent_unit2c_observer_exact_caps_preserve_eventual_result() {
    let mut cases = 0;
    for statements in [16380usize, 16381] {
        let (sources, raw) = observer_scalar_cap_fixture(statements);
        let actual = verified::probe_array_reference(
            raw,
            &sources,
            budget::Limits::DEFAULT,
            Some(hir::DefId(0)),
            Limits::default(),
            ObservationControl::default(),
        )
        .unwrap();
        assert_eq!(actual.result, Ok(Scalar::Unit));
        assert_eq!(actual.remaining_fuel, 1_000_000 - (2 * statements + 2));
        assert_eq!(actual.events.len(), 16384);
        assert_eq!(actual.truncated, statements == 16381);
        assert!(actual.storage.is_empty());
        assert!(!actual.allocation_fault_applied);
        cases += 1;
    }
    for owners in [32usize, 33] {
        for n in [0usize, 1024] {
            let (sources, raw) = observer_storage_cap_fixture(owners, n);
            let actual = verified::probe_array_reference(
                raw,
                &sources,
                budget::Limits::DEFAULT,
                Some(hir::DefId(0)),
                Limits::default(),
                ObservationControl {
                    poison_destinations: true,
                    ..ObservationControl::default()
                },
            )
            .unwrap();
            assert_eq!(actual.result, Ok(Scalar::Unit));
            assert_eq!(
                actual.remaining_fuel,
                1_000_000 - (6 + owners * (3 * n.max(1) + 6))
            );
            assert_eq!(actual.storage.len(), 64);
            assert_eq!(actual.truncated, owners == 33);
            assert!(!actual.allocation_fault_applied);
            assert_eq!(
                actual
                    .storage
                    .iter()
                    .map(|row| row.bytes.len())
                    .sum::<usize>(),
                64 * 4 * n.max(1)
            );
            for row in &actual.storage {
                assert_eq!(row.bytes, bytes(hir::Ty::I32, &vec![Scalar::I32(17); n]));
                assert_eq!(row.guards_before, row.guards_after);
            }
            cases += 1;
        }
    }
    eprintln!("independent observer exact-cap inputs={cases}");
}

#[test]
fn independent_unit2c_fault_attempt_consumes_only_first_statement_or_invoke_hook() {
    let mut cases = 0;
    for variant in 0..4 {
        let (sources, mut raw, _, _, _) = sequence_fixture(hir::Ty::I32, 4, 0, 0);
        let at = s(raw.functions[0].span, 2020);
        let mut limits = Limits::default();
        match variant {
            1 => raw.functions[0].blocks[0].statements[0] = literal(0, value(hir::Ty::I32, 0), at),
            2 => raw.functions[0].span = at,
            3 => limits.fuel = 25,
            _ => {}
        }
        let actual = verified::probe_array_reference(
            raw,
            &sources,
            budget::Limits::DEFAULT,
            Some(hir::DefId(0)),
            limits,
            ObservationControl {
                fault: Some(execute::FaultInjection {
                    at,
                    kind: execute::FaultKind::ConstructorLastScalarType,
                }),
                ..ObservationControl::default()
            },
        )
        .unwrap();
        assert!(!actual.truncated);
        assert!(!actual.allocation_fault_applied);
        match variant {
            1 => {
                assert!(!actual.fault_applied);
                assert_eq!(actual.result, Ok(Scalar::I32(4)));
            }
            3 => {
                assert!(!actual.fault_applied);
                assert_eq!(
                    actual.result,
                    Err(OwnedRunFailure::Scalar(RunFailure::Fuel(at)))
                );
            }
            _ => {
                assert!(actual.fault_applied);
                assert_eq!(
                    actual.result,
                    Err(OwnedRunFailure::Invariant(
                        "array construction type",
                        Some(at)
                    ))
                );
            }
        }
        cases += 1;
    }
    let (sources, mut raw, _) = chain_fixture(hir::Ty::I32, 1);
    let base = raw.functions[0].span;
    let at = s(base, 62);
    let g = &mut raw.functions[1];
    g.locals.push(local(hir::Ty::I32, base));
    g.owners
        .push(own(array(hir::Ty::I32, 1), OwnerKind::Temporary, base));
    g.blocks[0].statements = vec![
        literal(0, Scalar::I32(91), s(base, 121)),
        ins(OwnedInstruction::StorageLive(OwnerPlaceId(1)), s(base, 122)),
        ins(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(1),
                elements: vec![op(0, base)],
            },
            at,
        ),
        ins(OwnedInstruction::Discard(OwnerPlaceId(1)), s(base, 123)),
    ];
    let actual = verified::probe_array_reference(
        raw,
        &sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        Limits::default(),
        ObservationControl {
            fault: Some(execute::FaultInjection {
                at,
                kind: execute::FaultKind::ConstructorLastScalarType,
            }),
            ..ObservationControl::default()
        },
    )
    .unwrap();
    assert!(!actual.truncated);
    assert!(!actual.fault_applied);
    assert_eq!(actual.result, Ok(Scalar::I32(1)));
    cases += 1;
    eprintln!("independent observer first-hook inputs={cases}");
}
