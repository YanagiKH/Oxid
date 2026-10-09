//! Unit2B raw checks. The probe returns usage/failure and cannot authorize execution.
use super::*;

fn probe(raw: &RawOwnedProgram, sources: &SourceMap) -> Result<OwnershipUsage, OwnedFailure> {
    verified::probe_array_validation(raw, sources, budget::Limits::DEFAULT)
}
fn array_slot(ty: hir::Ty, length: usize) -> AggregateSlot {
    AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(
        FixedArrayTy::check(ty, length).unwrap(),
    ))
    .unwrap()
}
fn at(span: Span, start: usize) -> Span {
    Span {
        start,
        end: start + 1,
        ..span
    }
}
fn scalar(id: usize, value: Rvalue, span: Span) -> OwnedStatement {
    instruction(
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            destination: LocalId(id),
            value,
            span,
        })),
        span,
    )
}
fn op(id: usize, span: Span) -> Operand {
    Operand {
        local: LocalId(id),
        span,
    }
}
fn fixture(span: Span, ty: hir::Ty, length: usize) -> RawOwnedProgram {
    let mut raw = subject(span);
    let f = &mut raw.functions[0];
    f.owners[0].aggregate = array_slot(ty, length);
    f.locals.extend(
        [ty, hir::Ty::I32, ty, hir::Ty::I32]
            .into_iter()
            .map(|ty| LocalDecl {
                ty,
                kind: LocalKind::Temporary,
                span,
            }),
    );
    let value = match ty {
        hir::Ty::U8 => unreachable!("u8 is outside this predecessor fixture or observation domain"),
        hir::Ty::Bool => Rvalue::Bool(true),
        hir::Ty::I32 => Rvalue::I32(7),
        hir::Ty::Unit => Rvalue::Unit,
    };
    f.blocks[0].statements = vec![
        scalar(0, Rvalue::Unit, span),
        scalar(1, value, span),
        scalar(2, Rvalue::I32(0), span),
        instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), span),
        instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(0),
                elements: vec![op(1, at(span, 11)); length],
            },
            at(span, 10),
        ),
        instruction(
            OwnedInstruction::ReadIndex {
                destination: LocalId(3),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: op(2, at(span, 21)),
            },
            at(span, 20),
        ),
        instruction(
            OwnedInstruction::WriteIndex {
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: op(2, at(span, 31)),
                value: op(1, at(span, 32)),
            },
            at(span, 30),
        ),
        instruction(
            OwnedInstruction::ArrayLength {
                destination: LocalId(4),
                base: AccessBase::Owner(OwnerPlaceId(0)),
            },
            at(span, 40),
        ),
        instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), at(span, 50)),
    ];
    raw
}
fn malformed(raw: &RawOwnedProgram, sources: &SourceMap, expected: Malformed, span: Option<Span>) {
    let e = probe(raw, sources).unwrap_err();
    assert_eq!(e.kind, OwnedFailureKind::Malformed(expected));
    assert_eq!(e.primary.get(), span);
}

#[test]
fn array_types_and_lengths_require_full_production_validation() {
    let (sources, s) = context();
    let mut cases = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for length in [0, 1, 2, 4, 1024] {
            let raw = fixture(s, ty, length);
            let usage = probe(&raw, &sources).unwrap();
            assert_eq!(usage.owner_cells, length.max(1));
            assert_eq!(
                usage.owner_layout_bytes,
                if ty == hir::Ty::I32 {
                    4 * length.max(1)
                } else {
                    length.max(1)
                }
            );
            let witness = verify_owned(raw, &sources).unwrap();
            assert_eq!(witness.usage(), usage);
            cases += 1;
        }
    }
    // Every new opcode is rejected even if its only carrier is nominal, its
    // base/destination is invalid, and it lies in an unreachable block.
    for kind in [
        OwnedInstruction::ConstructArray {
            destination: OwnerPlaceId(999),
            elements: vec![],
        },
        OwnedInstruction::ReadIndex {
            destination: LocalId(999),
            base: AccessBase::Owner(OwnerPlaceId(999)),
            index: op(999, s),
        },
        OwnedInstruction::WriteIndex {
            base: AccessBase::Owner(OwnerPlaceId(999)),
            index: op(999, s),
            value: op(999, s),
        },
        OwnedInstruction::ArrayLength {
            destination: LocalId(999),
            base: AccessBase::Owner(OwnerPlaceId(999)),
        },
    ] {
        let mut raw = subject(s);
        raw.functions[0].blocks.push(OwnedBlock {
            span: s,
            merge: None,
            statements: vec![instruction(kind, at(s, 70))],
            terminator: end(OwnedTerminatorKind::Goto(BlockId(1)), s),
        });
        let e = error(raw, &sources);
        assert_eq!(e.kind, OwnedFailureKind::Malformed(Malformed::Id));
        assert_eq!(e.primary.get(), Some(at(s, 70)));
        cases += 1;
    }
    assert_eq!(cases, 19);
}

#[test]
fn unit2b_shape_checks_each_new_operand_and_kind_before_cfg() {
    let (sources, s) = context();
    for defect in 0..16 {
        let mut raw = fixture(s, hir::Ty::I32, 2);
        let f = &mut raw.functions[0];
        let expected = match defect {
            0 | 1 => {
                let OwnedInstruction::ConstructArray { elements, .. } =
                    &mut f.blocks[0].statements[4].kind
                else {
                    unreachable!()
                };
                if defect == 0 {
                    elements.pop();
                } else {
                    elements.push(op(1, s));
                }
                (Malformed::Type, at(s, 10))
            }
            2 => {
                let OwnedInstruction::ConstructArray { elements, .. } =
                    &mut f.blocks[0].statements[4].kind
                else {
                    unreachable!()
                };
                elements[1] = op(0, at(s, 12));
                (Malformed::Type, at(s, 12))
            }
            3 => {
                let OwnedInstruction::ConstructArray { elements, .. } =
                    &mut f.blocks[0].statements[4].kind
                else {
                    unreachable!()
                };
                elements[1] = op(999, at(s, 12));
                (Malformed::Id, at(s, 12))
            }
            4 => {
                f.owners[0].aggregate =
                    AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap();
                (Malformed::Type, at(s, 10))
            }
            5 => {
                f.blocks[0].statements[5].kind = OwnedInstruction::ReadIndex {
                    destination: LocalId(3),
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    index: op(0, at(s, 21)),
                };
                (Malformed::Type, at(s, 21))
            }
            6 => {
                f.locals[3].ty = hir::Ty::Bool;
                (Malformed::Type, at(s, 20))
            }
            7 => {
                f.blocks[0].statements[6].kind = OwnedInstruction::WriteIndex {
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    index: op(2, at(s, 31)),
                    value: op(0, at(s, 32)),
                };
                (Malformed::Type, at(s, 32))
            }
            8 => {
                f.blocks[0].statements[6].kind = OwnedInstruction::WriteIndex {
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    index: op(0, at(s, 31)),
                    value: op(1, at(s, 32)),
                };
                (Malformed::Type, at(s, 31))
            }
            9 => {
                f.locals[4].ty = hir::Ty::Bool;
                (Malformed::Type, at(s, 40))
            }
            10 => {
                f.blocks[0].statements[7].kind = OwnedInstruction::ArrayLength {
                    destination: LocalId(999),
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                };
                (Malformed::Id, at(s, 40))
            }
            11 => {
                f.blocks[0].statements[7].kind = OwnedInstruction::ArrayLength {
                    destination: LocalId(4),
                    base: AccessBase::Owner(OwnerPlaceId(999)),
                };
                (Malformed::Id, at(s, 40))
            }
            12 => {
                let bad = Span { end: 201, ..s };
                if let OwnedInstruction::ConstructArray { elements, .. } =
                    &mut f.blocks[0].statements[4].kind
                {
                    elements[0].span = bad;
                }
                (Malformed::Span, bad)
            }
            13 => {
                let bad = Span { end: 201, ..s };
                if let OwnedInstruction::ReadIndex { index, .. } =
                    &mut f.blocks[0].statements[5].kind
                {
                    index.span = bad;
                }
                (Malformed::Span, bad)
            }
            14 => {
                let bad = Span { end: 201, ..s };
                if let OwnedInstruction::WriteIndex { value, .. } =
                    &mut f.blocks[0].statements[6].kind
                {
                    value.span = bad;
                }
                (Malformed::Span, bad)
            }
            15 => {
                let bad = Span { end: 201, ..s };
                if let OwnedInstruction::WriteIndex { index, .. } =
                    &mut f.blocks[0].statements[6].kind
                {
                    index.span = bad;
                }
                (Malformed::Span, bad)
            }
            _ => unreachable!(),
        };
        malformed(&raw, &sources, expected.0, Some(expected.1));
    }
    for operation in 0..3 {
        let mut raw = subject(s);
        let kind = match operation {
            0 => OwnedInstruction::ReadIndex {
                destination: LocalId(0),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: op(0, s),
            },
            1 => OwnedInstruction::WriteIndex {
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: op(0, s),
                value: op(0, s),
            },
            _ => OwnedInstruction::ArrayLength {
                destination: LocalId(0),
                base: AccessBase::Owner(OwnerPlaceId(0)),
            },
        };
        raw.functions[0].blocks[0]
            .statements
            .insert(3, instruction(kind, at(s, 60)));
        malformed(&raw, &sources, Malformed::Type, Some(at(s, 60)));
    }
    // Record operations never turn array ordinals into nominal FieldIds.
    let mut raw = fixture(s, hir::Ty::I32, 2);
    raw.functions[0].blocks[0].statements[5].kind = OwnedInstruction::ReadField {
        destination: LocalId(3),
        base: AccessBase::Owner(OwnerPlaceId(0)),
        field: FieldId {
            record: RecordId(0),
            index: 0,
        },
    };
    malformed(&raw, &sources, Malformed::Type, Some(at(s, 20)));
    // Malformed unreachable operations are shape checked before reachability.
    let mut raw = fixture(s, hir::Ty::I32, 2);
    raw.functions[0].blocks.push(OwnedBlock {
        span: s,
        merge: None,
        statements: vec![instruction(
            OwnedInstruction::WriteIndex {
                base: AccessBase::Owner(OwnerPlaceId(0)),
                index: op(0, at(s, 61)),
                value: op(1, s),
            },
            s,
        )],
        terminator: end(OwnedTerminatorKind::Goto(BlockId(1)), s),
    });
    malformed(&raw, &sources, Malformed::Type, Some(at(s, 61)));
}

#[test]
fn unit2b_canonical_cfg_sees_every_array_use_and_definition() {
    let (sources, s) = context();
    for use_site in 0..4 {
        let mut raw = fixture(s, hir::Ty::I32, 2);
        let f = &mut raw.functions[0];
        f.locals.push(LocalDecl {
            ty: hir::Ty::I32,
            kind: LocalKind::Temporary,
            span: s,
        });
        let missing = op(5, at(s, 65));
        match use_site {
            0 => {
                if let OwnedInstruction::ConstructArray { elements, .. } =
                    &mut f.blocks[0].statements[4].kind
                {
                    elements[1] = missing;
                }
            }
            1 => {
                if let OwnedInstruction::ReadIndex { index, .. } =
                    &mut f.blocks[0].statements[5].kind
                {
                    *index = missing;
                }
            }
            2 => {
                if let OwnedInstruction::WriteIndex { index, .. } =
                    &mut f.blocks[0].statements[6].kind
                {
                    *index = missing;
                }
            }
            _ => {
                if let OwnedInstruction::WriteIndex { value, .. } =
                    &mut f.blocks[0].statements[6].kind
                {
                    *value = missing;
                }
            }
        }
        // Defining the operand later is still not dominance.
        f.blocks[0].statements.push(scalar(5, Rvalue::I32(0), s));
        malformed(
            &raw,
            &sources,
            Malformed::Scalar(FailureKind::Uninitialized),
            Some(at(s, 65)),
        );
    }
    for definition in [5, 7] {
        let mut raw = fixture(s, hir::Ty::I32, 2);
        let f = &mut raw.functions[0];
        let dupe = f.blocks[0].statements[definition].clone();
        f.blocks[0].statements.insert(definition + 1, dupe);
        let e = probe(&raw, &sources).unwrap_err();
        assert_eq!(
            e.kind,
            OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::AlreadyInitialized))
        );
    }
    let mut raw = fixture(s, hir::Ty::I32, 2);
    if let OwnedInstruction::ReadIndex {
        destination, index, ..
    } = &mut raw.functions[0].blocks[0].statements[5].kind
    {
        index.local = *destination;
    }
    malformed(
        &raw,
        &sources,
        Malformed::Scalar(FailureKind::Uninitialized),
        Some(at(s, 21)),
    );
}

#[test]
fn unit2b_denials_preserve_distinct_operation_role_and_origins() {
    let (sources, s) = context();
    for (site, operation) in [
        (5, flow::DeniedOperation::ReadIndex),
        (6, flow::DeniedOperation::WriteIndex),
        (7, flow::DeniedOperation::ArrayLength),
    ] {
        let mut raw = fixture(s, hir::Ty::I32, 2);
        let f = &mut raw.functions[0];
        let mut access = f.blocks[0].statements[site].clone();
        access.diagnostic_origins = Some(DiagnosticOrigins {
            primary: at(s, 71),
            cause: at(s, 72),
        });
        let move_span = at(s, 70);
        f.blocks[0].statements.truncate(5);
        f.blocks[0].statements.extend([
            instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), move_span),
            access,
        ]);
        let expected_aggregate = f.owners[0].aggregate();
        let charge_span = plan::instruction_span(&f.blocks[0].statements[6]);
        let e = probe(&raw, &sources).unwrap_err();
        assert_eq!(e.kind, OwnedFailureKind::Ownership(Violation::Unavailable));
        assert_eq!(e.primary.get(), Some(at(s, 71)));
        assert_eq!(e.related.get(), Some(move_span));
        let facts = e.context.unwrap().facts();
        assert_eq!(
            (facts.operation, facts.role, facts.state),
            (
                operation,
                flow::DeniedRole::ArrayBase,
                flow::ObservedState::Moved
            )
        );
        assert_eq!(
            facts.subject.referent(),
            BorrowedTy::Exact(expected_aggregate)
        );
        assert_eq!(charge_span, at(s, 71));
    }
    let mut raw = fixture(s, hir::Ty::I32, 2);
    raw.functions[0].blocks[0].statements.remove(3); // Missing live is shape malformed first.
    malformed(&raw, &sources, Malformed::CanonicalSite, Some(s));
    let mut raw = fixture(s, hir::Ty::I32, 2);
    raw.functions[0].blocks[0].statements.swap(3, 4); // Canonical sites exist; dynamic initialization is illegal.
    let e = probe(&raw, &sources).unwrap_err();
    assert_eq!(
        e.kind,
        OwnedFailureKind::Ownership(Violation::Initialization)
    );
    let facts = e.context.unwrap().facts();
    assert_eq!(
        (facts.operation, facts.role, facts.state),
        (
            flow::DeniedOperation::ConstructArray,
            flow::DeniedRole::InitializationDestination,
            flow::ObservedState::Dead
        )
    );
}

#[test]
fn unit2b_raw_q_limits_and_payload_are_independent_and_allocation_free() {
    let (sources, s) = context();
    let raw = fixture(s, hir::Ty::I32, 2);
    let u = probe(&raw, &sources).unwrap();
    // Hand inventory: B1, E0, I9, M0, A0, F0, Q2, O1, L/C/Pb/R0, V5, Z0.
    assert_eq!(u.expanded_events, 11);
    assert_eq!(u.work, 36 * (1 + 2 + 9 + 2 + 1 + 5));
    assert_eq!(u.scratch_bytes, 33);
    let expected_metadata = (std::mem::size_of::<Vec<RawEnumDecl>>()
        + std::mem::size_of::<usize>()
        + std::mem::size_of::<Vec<MatchDecl>>())
        + 10 * std::mem::size_of::<Option<DiagnosticOrigins>>()
        + std::mem::size_of::<shape::OwnerSites>()
        + 2 * std::mem::size_of::<Operand>();
    assert_eq!(u.metadata_bytes, expected_metadata);
    for which in 0..5 {
        let mut limits = budget::Limits::DEFAULT;
        match which {
            0 => limits.owners = u.owners,
            1 => limits.events = u.expanded_events,
            2 => limits.work = u.work,
            3 => limits.metadata = u.metadata_bytes,
            _ => limits.scratch = u.scratch_bytes,
        };
        assert!(verified::probe_array_validation(&raw, &sources, limits).is_ok());
        let expected = match which {
            0 => {
                limits.owners -= 1;
                "owners"
            }
            1 => {
                limits.events -= 1;
                "expanded ownership events"
            }
            2 => {
                limits.work -= 1;
                "ownership work"
            }
            3 => {
                limits.metadata -= 1;
                "ownership metadata"
            }
            _ => {
                limits.scratch -= 1;
                "ownership scratch"
            }
        };
        let e = budget::fail_allocation_after(0, || {
            verified::probe_array_validation(&raw, &sources, limits)
        })
        .unwrap_err();
        assert_eq!(e.kind, OwnedFailureKind::Resource(expected));
    }
    let mut oversized = fixture(s, hir::Ty::Unit, 0);
    if let OwnedInstruction::ConstructArray { elements, .. } =
        &mut oversized.functions[0].blocks[0].statements[4].kind
    {
        *elements = vec![op(usize::MAX, Span { end: 201, ..s }); 1025];
    }
    let (result, allocations) = reviewer_origins::integration_counted(|| {
        budget::fail_allocation_after(0, || probe(&oversized, &sources))
    });
    assert_eq!(allocations, 0);
    let e = result.unwrap_err();
    assert_eq!(
        e.kind,
        OwnedFailureKind::Resource("array constructor elements")
    );
    // Preflight traverses only vector lengths, never malformed operand contents.
    assert_eq!(
        error(oversized, &sources).kind,
        OwnedFailureKind::Resource("array constructor elements")
    );
    let mut failures = 0;
    loop {
        match budget::fail_allocation_after(failures, || probe(&raw, &sources)) {
            Ok(_) => break,
            Err(e) => assert_eq!(
                e.kind,
                OwnedFailureKind::Resource("injected allocation failure")
            ),
        }
        failures += 1;
        assert!(failures < 100);
    }
    assert!(failures > 0);
    println!("unit2b existing verifier allocation points: {failures}");
    // No Q-shaped scratch/allocation: zero and maximum width use the same points.
    for length in [0, 1024] {
        let raw = fixture(s, hir::Ty::Unit, length);
        assert_eq!(
            budget::fail_allocation_after(failures - 1, || probe(&raw, &sources))
                .unwrap_err()
                .kind,
            OwnedFailureKind::Resource("injected allocation failure")
        );
        assert!(budget::fail_allocation_after(failures, || probe(&raw, &sources)).is_ok());
        assert_eq!(
            probe(&raw, &sources).unwrap().scratch_bytes,
            u.scratch_bytes
        );
    }
}

#[test]
fn unit2b_structural_identity_controls_loans_moves_and_returns() {
    let (mut sources, s) = context();
    let foreign = sources.add("other-array-use.ox".into(), "x".repeat(200));
    for (actual, expected) in [
        ((hir::Ty::Bool, 0), (hir::Ty::I32, 0)),
        ((hir::Ty::Bool, 4), (hir::Ty::Unit, 4)),
        ((hir::Ty::Bool, 4), (hir::Ty::I32, 1)),
        ((hir::Ty::I32, 2), (hir::Ty::I32, 3)),
    ] {
        let mut raw = fixture(s, actual.0, actual.1);
        raw.functions[0].result = ValueTy::Owned(array_slot(expected.0, expected.1).aggregate());
        raw.functions[0].blocks[0].statements.pop();
        raw.functions[0].blocks[0].terminator =
            end(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)), at(s, 80));
        malformed(&raw, &sources, Malformed::Type, Some(at(s, 80)));
        raw.functions[0].result = ValueTy::Owned(array_slot(actual.0, actual.1).aggregate());
        assert!(probe(&raw, &sources).is_ok());
        let f = &mut raw.functions[0];
        f.owners.push(OwnerDecl {
            aggregate: array_slot(expected.0, expected.1),
            kind: OwnerKind::Temporary,
            span: s,
        });
        f.blocks[0].statements.extend([
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), s),
            instruction(
                OwnedInstruction::MoveInitialize {
                    destination: OwnerPlaceId(1),
                    source: OwnerPlaceId(0),
                },
                at(s, 81),
            ),
        ]);
        malformed(&raw, &sources, Malformed::Type, Some(at(s, 81)));
    }
    for kind in [BorrowKind::Shared, BorrowKind::Exclusive] {
        let mut raw = borrowed(s, kind);
        let slot = array_slot(hir::Ty::I32, 0);
        for f in &mut raw.functions {
            for o in &mut f.owners {
                o.aggregate = slot;
            }
            for r in &mut f.references {
                r.referent = BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap();
                r.span = Span {
                    file: foreign,
                    ..r.span
                };
            }
            for l in &mut f.loans {
                l.referent = BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap();
            }
            for b in &mut f.blocks {
                for i in &mut b.statements {
                    if let OwnedInstruction::Construct { destination, .. } = i.kind {
                        i.kind = OwnedInstruction::ConstructArray {
                            destination,
                            elements: vec![],
                        };
                    }
                }
            }
        }
        // Structural identity carries no first-use file association.
        assert!(probe(&raw, &sources).is_ok());
        raw.functions[1].references[0].referent =
            BorrowedSlot::check(BorrowedTy::Exact(array_slot(hir::Ty::Bool, 0).aggregate()))
                .unwrap();
        let call_span = raw.functions[0].calls[0].span;
        malformed(&raw, &sources, Malformed::Binding, Some(call_span));
    }
    let mut raw = fixture(s, hir::Ty::I32, 2);
    if let OwnedInstruction::ConstructArray { elements, .. } =
        &mut raw.functions[0].blocks[0].statements[4].kind
    {
        elements[0].span.file = foreign;
    }
    assert!(
        probe(&raw, &sources).is_ok(),
        "raw validation checks SourceMap validity, not source function association"
    );
}

#[test]
fn unit2b_index_dominance_uses_real_branch_edges() {
    let (sources, s) = context();
    for site in 0..4 {
        let mut raw = fixture(s, hir::Ty::I32, 2);
        let f = &mut raw.functions[0];
        f.locals.push(LocalDecl {
            ty: hir::Ty::Bool,
            kind: LocalKind::Temporary,
            span: s,
        });
        f.locals.push(LocalDecl {
            ty: hir::Ty::I32,
            kind: LocalKind::Temporary,
            span: s,
        });
        let use_span = at(s, 90);
        match site {
            0 => {
                if let OwnedInstruction::ConstructArray { elements, .. } =
                    &mut f.blocks[0].statements[4].kind
                {
                    elements[0] = op(6, use_span);
                }
            }
            1 => {
                if let OwnedInstruction::ReadIndex { index, .. } =
                    &mut f.blocks[0].statements[5].kind
                {
                    *index = op(6, use_span);
                }
            }
            2 => {
                if let OwnedInstruction::WriteIndex { index, .. } =
                    &mut f.blocks[0].statements[6].kind
                {
                    *index = op(6, use_span);
                }
            }
            _ => {
                if let OwnedInstruction::WriteIndex { value, .. } =
                    &mut f.blocks[0].statements[6].kind
                {
                    *value = op(6, use_span);
                }
            }
        }
        let mut entry = f.blocks[0].clone();
        entry.statements = vec![scalar(5, Rvalue::Bool(true), s)];
        entry.terminator = end(
            OwnedTerminatorKind::Branch {
                condition: op(5, s),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
            s,
        );
        let left = OwnedBlock {
            span: s,
            merge: None,
            statements: vec![scalar(6, Rvalue::I32(0), s)],
            terminator: end(OwnedTerminatorKind::Goto(BlockId(3)), s),
        };
        let right = OwnedBlock {
            span: s,
            merge: None,
            statements: vec![],
            terminator: end(OwnedTerminatorKind::Goto(BlockId(3)), s),
        };
        let body = f.blocks.remove(0);
        f.blocks = vec![entry, left, right, body];
        malformed(
            &raw,
            &sources,
            Malformed::Scalar(FailureKind::Uninitialized),
            Some(use_span),
        );
    }
}

#[test]
fn unit2b_scalar_bypass_and_inactive_array_shapes_are_distinct() {
    let (sources, s) = context();
    let raw = RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: vec![],
        records: vec![],
        functions: vec![unit_function(0, s)],
    };
    let zero = budget::Limits {
        owners: 0,
        events: 0,
        work: 0,
        scratch: 0,
        metadata: (std::mem::size_of::<Vec<RawEnumDecl>>()
            + std::mem::size_of::<usize>()
            + std::mem::size_of::<Vec<MatchDecl>>()),
    };
    assert_eq!(
        verified::probe_array_validation(&raw, &sources, zero).unwrap(),
        OwnershipUsage {
            metadata_bytes: (std::mem::size_of::<Vec<RawEnumDecl>>()
                + std::mem::size_of::<usize>()
                + std::mem::size_of::<Vec<MatchDecl>>()),
            ..OwnershipUsage::default()
        }
    );
    assert_eq!(
        verify_with_limits(raw, &sources, zero).unwrap().usage(),
        OwnershipUsage {
            metadata_bytes: (std::mem::size_of::<Vec<RawEnumDecl>>()
                + std::mem::size_of::<usize>()
                + std::mem::size_of::<Vec<MatchDecl>>()),
            ..OwnershipUsage::default()
        }
    );
    let mut raw = RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: vec![],
        records: vec![],
        functions: vec![unit_function(0, s)],
    };
    raw.functions[0].blocks[0].statements.push(instruction(
        OwnedInstruction::ArrayLength {
            destination: LocalId(0),
            base: AccessBase::Owner(OwnerPlaceId(0)),
        },
        at(s, 91),
    ));
    assert!(
        budget::preflight(&raw, budget::Limits::DEFAULT)
            .unwrap()
            .work
            > 0
    );
    malformed(&raw, &sources, Malformed::Id, Some(at(s, 91)));
    assert_eq!(
        verified::probe_array_validation(&raw, &sources, zero)
            .unwrap_err()
            .kind,
        OwnedFailureKind::Resource("expanded ownership events")
    );
    // Result-only array carriers do not invent owner/flow state, but still use
    // all-function signatures/shape/CFG in the production verifier.
    let f = &mut raw.functions[0];
    f.blocks[0].statements.pop();
    f.result = ValueTy::Owned(array_slot(hir::Ty::I32, 0).aggregate());
    let term_span = f.blocks[0].terminator.as_ref().unwrap().span;
    malformed(&raw, &sources, Malformed::Type, Some(term_span));
    assert_eq!(
        error(raw, &sources).kind,
        OwnedFailureKind::Malformed(Malformed::Type)
    );
}

#[test]
fn unit2b_actual_raw_q_reaches_expanded_event_ceiling_inclusively() {
    let (sources, s) = context();
    let mut raw = RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: vec![],
        records: vec![],
        functions: vec![unit_function(0, s)],
    };
    // A deliberately shape-invalid, caller-owned raw payload: 97*1024+573
    // operands and 99 statements give exactly 100,000 expanded events. Raw
    // preflight must admit its lengths independently of invalid owner IDs.
    for length in std::iter::repeat_n(1024, 97).chain([573]) {
        raw.functions[0].blocks[0].statements.push(instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(usize::MAX),
                elements: vec![op(usize::MAX, Span { end: 201, ..s }); length],
            },
            s,
        ));
    }
    let admitted = budget::preflight(&raw, budget::Limits::DEFAULT).unwrap();
    assert_eq!(admitted.expanded_events, 100_000);
    assert_eq!(admitted.work, 32 * (1 + 2 + 99 + 99_901 + 1));
    assert_eq!(
        admitted.metadata_bytes,
        (std::mem::size_of::<Vec<RawEnumDecl>>()
            + std::mem::size_of::<usize>()
            + std::mem::size_of::<Vec<MatchDecl>>())
            + 100 * std::mem::size_of::<Option<DiagnosticOrigins>>()
            + 99_901 * std::mem::size_of::<Operand>()
    );
    if let OwnedInstruction::ConstructArray { elements, .. } = &mut raw.functions[0].blocks[0]
        .statements
        .last_mut()
        .unwrap()
        .kind
    {
        elements.push(op(usize::MAX, s));
    }
    let e = budget::fail_allocation_after(0, || probe(&raw, &sources)).unwrap_err();
    assert_eq!(
        e.kind,
        OwnedFailureKind::Resource("expanded ownership events")
    );
}

#[test]
fn unit2b_valid_arrays_reach_expanded_event_ceiling_inclusively() {
    let (sources, s) = context();
    let mut function = unit_function(0, s);
    function.result = ValueTy::Scalar(hir::Ty::I32);
    function.parameters = vec![ParameterBinding::Scalar(LocalId(0))];
    function.locals[0].ty = hir::Ty::I32;
    function.locals[0].kind = LocalKind::Parameter;
    function.blocks[0].statements.clear();
    // A parameter supplies the scalar snapshot, so the only statements are98
    // distinct lifetimes and constructors. Every canonical site is legitimate.
    for (ordinal, length) in std::iter::repeat_n(1024, 97).chain([476]).enumerate() {
        let owner = OwnerPlaceId(ordinal);
        function.owners.push(OwnerDecl {
            aggregate: array_slot(hir::Ty::I32, length),
            kind: OwnerKind::Local { mutable: false },
            span: s,
        });
        function.blocks[0].statements.extend([
            instruction(OwnedInstruction::StorageLive(owner), s),
            instruction(
                OwnedInstruction::ConstructArray {
                    destination: owner,
                    elements: vec![op(0, s); length],
                },
                s,
            ),
        ]);
    }
    let mut raw = RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: vec![],
        records: vec![],
        functions: vec![function],
    };
    let usage = probe(&raw, &sources).unwrap();
    assert_eq!(usage.expanded_events, 100_000);
    assert_eq!(usage.owner_cells, 99_804);
    assert_eq!(usage.owner_layout_bytes, 4 * 99_804);
    assert_eq!(
        usage.work,
        (4 * 98 + 32) * (1 + 2 + 196 + 99_804 + 98 + 1 + 1)
    );
    // The paired one-over program remains structurally consistent: only the
    // actual length and matching type change, before any verification reserve.
    raw.functions[0].owners[97].aggregate = array_slot(hir::Ty::I32, 477);
    if let OwnedInstruction::ConstructArray { elements, .. } = &mut raw.functions[0].blocks[0]
        .statements
        .last_mut()
        .unwrap()
        .kind
    {
        elements.push(op(0, s));
    }
    let (result, allocations) = reviewer_origins::integration_counted(|| {
        budget::fail_allocation_after(0, || probe(&raw, &sources))
    });
    assert_eq!(allocations, 0);
    assert_eq!(
        result.unwrap_err().kind,
        OwnedFailureKind::Resource("expanded ownership events")
    );
}
