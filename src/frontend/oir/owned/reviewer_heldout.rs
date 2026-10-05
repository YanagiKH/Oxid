//! Reviewer-authored held-out raw fixtures. No implementer fixture/oracle helpers.
use super::super::*;
fn ctx() -> (SourceMap, Span) {
    let mut m = SourceMap::new();
    let file = m.add("reviewer-heldout.ox".into(), "x".repeat(4096));
    (
        m,
        Span {
            file,
            start: 0,
            end: 1,
        },
    )
}
fn at(s: Span, n: usize) -> Span {
    Span {
        start: n,
        end: n + 1,
        ..s
    }
}
fn op(id: usize, s: Span) -> Operand {
    Operand {
        local: LocalId(id),
        span: s,
    }
}
fn ins(kind: OwnedInstruction, s: Span) -> OwnedStatement {
    OwnedStatement {
        diagnostic_origins: None,
        kind,
        span: s,
    }
}
fn term(kind: OwnedTerminatorKind, s: Span) -> Option<OwnedTerminator> {
    Some(OwnedTerminator {
        diagnostic_origins: None,
        kind,
        span: s,
    })
}
fn scalar(id: usize, value: Rvalue, s: Span) -> OwnedStatement {
    ins(
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            destination: LocalId(id),
            value,
            span: s,
        })),
        s,
    )
}
fn decl(ty: hir::Ty, s: Span) -> LocalDecl {
    LocalDecl {
        ty,
        kind: LocalKind::Temporary,
        span: s,
    }
}
fn field(i: usize) -> FieldId {
    FieldId {
        record: RecordId(0),
        index: i,
    }
}
fn construct(id: usize, s: Span) -> OwnedStatement {
    ins(
        OwnedInstruction::Construct {
            destination: OwnerPlaceId(id),
            fields: vec![
                (field(2), op(0, s)),
                (field(0), op(1, s)),
                (field(1), op(2, s)),
            ],
        },
        s,
    )
}
fn body(id: usize, s: Span) -> RawOwnedFunction {
    RawOwnedFunction {
        id: hir::DefId(id),
        span: s,
        result: ValueTy::Scalar(hir::Ty::Unit),
        parameters: vec![],
        locals: vec![
            decl(hir::Ty::Unit, s),
            decl(hir::Ty::I32, s),
            decl(hir::Ty::Bool, s),
        ],
        places: vec![],
        owners: vec![],
        references: vec![],
        calls: vec![],
        loans: vec![],
        matches: Vec::new(),
        entry: BlockId(0),
        blocks: vec![OwnedBlock {
            merge: None,
            span: s,
            statements: vec![
                scalar(0, Rvalue::Unit, s),
                scalar(1, Rvalue::I32(17), s),
                scalar(2, Rvalue::Bool(true), s),
            ],
            terminator: term(OwnedTerminatorKind::ReturnScalar(op(0, s)), s),
        }],
    }
}
fn base(s: Span, owners: usize) -> RawOwnedProgram {
    let records = vec![RawRecordDecl {
        id: RecordId(0),
        span: s,
        fields: [hir::Ty::I32, hir::Ty::Bool, hir::Ty::Unit]
            .into_iter()
            .enumerate()
            .map(|(i, t)| RawFieldDecl {
                id: field(i),
                ty: ParameterTy::Value(ValueTy::Scalar(t)),
                span: s,
            })
            .collect(),
    }];
    let mut f = body(0, s);
    for id in 0..owners {
        f.owners.push(OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
            kind: OwnerKind::Local { mutable: true },
            span: s,
        });
        f.blocks[0]
            .statements
            .push(ins(OwnedInstruction::StorageLive(OwnerPlaceId(id)), s));
        f.blocks[0].statements.push(construct(id, s));
    }
    RawOwnedProgram {
        enums: vec![],
        records,
        functions: vec![f],
    }
}
fn borrow_program(s: Span, specs: &[(usize, BorrowKind)]) -> RawOwnedProgram {
    let count = specs.iter().map(|p| p.0 + 1).max().unwrap_or(1);
    let mut p = base(s, count);
    let mut callee = body(1, s);
    for (i, (_, kind)) in specs.iter().enumerate() {
        callee
            .parameters
            .push(ParameterBinding::Reference(ReferenceParamId(i)));
        callee.references.push(ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            kind: *kind,
            position: i,
            span: s,
        });
    }
    p.functions.push(callee);
    let f = &mut p.functions[0];
    f.locals.push(decl(hir::Ty::Unit, s));
    f.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: (0..specs.len())
            .map(|i| ArgumentSlot::Borrow(LoanId(i)))
            .collect(),
        result: CallResult::Scalar(LocalId(3)),
        parent: None,
        span: s,
    });
    f.blocks[0]
        .statements
        .push(ins(OwnedInstruction::OpenCall(CallSiteId(0)), at(s, 30)));
    for (i, (owner, kind)) in specs.iter().enumerate() {
        let t = at(s, 40 + i);
        f.loans.push(LoanDecl {
            projection: Vec::new(),
            call: CallSiteId(0),
            argument: i,
            authority: AccessBase::Owner(OwnerPlaceId(*owner)),
            kind: *kind,
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            span: t,
        });
        f.blocks[0].statements.push(ins(
            OwnedInstruction::PrepareBorrow {
                call: CallSiteId(0),
                argument: i,
                loan: LoanId(i),
            },
            t,
        ));
    }
    f.blocks[0].terminator = term(
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        at(s, 100),
    );
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: vec![],
        terminator: term(OwnedTerminatorKind::ReturnScalar(op(3, s)), s),
    });
    p
}
fn owned_call(s: Span) -> RawOwnedProgram {
    let mut p = base(s, 1);
    let mut callee = body(1, s);
    callee.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
    callee.owners = vec![OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
        kind: OwnerKind::Parameter { position: 0 },
        span: s,
    }];
    callee.result = ValueTy::Owned(AggregateTy::Record(RecordId(0)));
    callee.blocks[0].terminator = term(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)), s);
    p.functions.push(callee);
    let f = &mut p.functions[0];
    f.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
        kind: OwnerKind::StagedArgument {
            call: CallSiteId(0),
            argument: 0,
        },
        span: s,
    });
    f.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
        kind: OwnerKind::CallResult {
            call: CallSiteId(0),
        },
        span: s,
    });
    f.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(1))],
        result: CallResult::Owned(OwnerPlaceId(2)),
        parent: None,
        span: s,
    });
    f.blocks[0].statements.extend([
        ins(OwnedInstruction::OpenCall(CallSiteId(0)), s),
        ins(
            OwnedInstruction::PrepareOwned {
                call: CallSiteId(0),
                argument: 0,
                source: OwnerPlaceId(0),
            },
            s,
        ),
    ]);
    f.blocks[0].terminator = term(
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s,
    );
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: vec![ins(OwnedInstruction::Discard(OwnerPlaceId(2)), s)],
        terminator: term(OwnedTerminatorKind::ReturnScalar(op(0, s)), s),
    });
    p
}
fn denied(p: RawOwnedProgram, m: &SourceMap) {
    assert!(verify_owned(p, m).is_err(), "mutant obtained a witness");
}

#[test]
fn reviewer_signature_and_body_are_authoritative() {
    let (m, s) = ctx();
    assert!(verify_owned(borrow_program(s, &[(0, BorrowKind::Shared)]), &m).is_ok());
    for variant in 0..12 {
        let mut p = borrow_program(s, &[(0, BorrowKind::Shared)]);
        match variant {
            0 => p.functions[1].references[0].kind = BorrowKind::Exclusive,
            1 => p.functions[0].calls[0].target = hir::DefId(0),
            2 => p.functions[0].loans[0].call = CallSiteId(usize::MAX),
            3 => p.functions[0].loans[0].authority = AccessBase::Owner(OwnerPlaceId(usize::MAX)),
            4 => p.functions[0].calls[0].arguments[0] = ArgumentSlot::Scalar,
            5 => p.functions[0].calls[0].result = CallResult::Scalar(LocalId(1)),
            6 => {
                p.functions[0].blocks[0].terminator = term(
                    OwnedTerminatorKind::Invoke {
                        call: CallSiteId(0),
                        continuation: BlockId(0),
                    },
                    s,
                )
            }
            7 => {
                p.functions[0].blocks[0].terminator = term(OwnedTerminatorKind::Goto(BlockId(1)), s)
            }
            8 => p.functions[0].blocks[0]
                .statements
                .push(ins(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s)),
            9 => p.functions[0].blocks[0]
                .statements
                .push(ins(OwnedInstruction::Discard(OwnerPlaceId(0)), s)),
            10 => p.functions[0].blocks[0].statements.swap(5, 6),
            11 => p.functions[0].blocks[0].statements.push(ins(
                OwnedInstruction::WriteField {
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    field: field(0),
                    value: op(1, s),
                },
                s,
            )),
            _ => unreachable!(),
        }
        denied(p, &m);
    }
}
#[test]
fn reviewer_constructor_order_and_scalar_inventory() {
    let (m, s) = ctx();
    assert!(verify_owned(base(s, 1), &m).is_ok());
    for variant in 0..8 {
        let mut p = base(s, 1);
        match variant {
            0 => {
                if let OwnedInstruction::Construct { fields, .. } =
                    &mut p.functions[0].blocks[0].statements[4].kind
                {
                    fields[1].0 = field(2);
                }
            }
            1 => {
                if let OwnedInstruction::Construct { fields, .. } =
                    &mut p.functions[0].blocks[0].statements[4].kind
                {
                    fields[1].0.index = usize::MAX;
                }
            }
            2 => {
                if let OwnedInstruction::Construct { fields, .. } =
                    &mut p.functions[0].blocks[0].statements[4].kind
                {
                    fields[1].1 = op(2, s);
                }
            }
            3 => {
                if let OwnedInstruction::Construct { fields, .. } =
                    &mut p.functions[0].blocks[0].statements[4].kind
                {
                    fields[1].1 = op(99, s);
                }
            }
            4 => {
                p.functions[0].blocks[0].statements.swap(1, 4);
            }
            5 => {
                p.functions[0].blocks[0].statements.push(ins(
                    OwnedInstruction::ReadField {
                        destination: LocalId(1),
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        field: field(0),
                    },
                    s,
                ));
            }
            6 => {
                p.functions[0].blocks[0].statements.push(ins(
                    OwnedInstruction::WriteField {
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        field: field(0),
                        value: op(77, s),
                    },
                    s,
                ));
            }
            7 => {
                p.functions[0].blocks[0].statements.push(ins(
                    OwnedInstruction::ReadField {
                        destination: LocalId(0),
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        field: field(1),
                    },
                    s,
                ));
            }
            _ => unreachable!(),
        }
        denied(p, &m);
    }
}
#[test]
fn reviewer_staging_results_and_parameter_class_operations() {
    let (m, s) = ctx();
    assert!(verify_owned(owned_call(s), &m).is_ok());
    for variant in 0..11 {
        let mut p = owned_call(s);
        let bad = match variant {
            0 => OwnedInstruction::Discard(OwnerPlaceId(1)),
            1 => OwnedInstruction::StorageEnd(OwnerPlaceId(1)),
            2 => OwnedInstruction::StorageLive(OwnerPlaceId(1)),
            3 => OwnedInstruction::MoveInitialize {
                source: OwnerPlaceId(1),
                destination: OwnerPlaceId(0),
            },
            4 => OwnedInstruction::ReadField {
                destination: LocalId(1),
                base: AccessBase::Owner(OwnerPlaceId(1)),
                field: field(0),
            },
            5 => OwnedInstruction::StorageLive(OwnerPlaceId(2)),
            6 => OwnedInstruction::Construct {
                destination: OwnerPlaceId(2),
                fields: vec![],
            },
            7 => OwnedInstruction::Discard(OwnerPlaceId(2)),
            8 => OwnedInstruction::Discard(OwnerPlaceId(0)),
            9 => OwnedInstruction::PrepareOwned {
                call: CallSiteId(0),
                argument: 0,
                source: OwnerPlaceId(1),
            },
            10 => OwnedInstruction::Replace {
                source: OwnerPlaceId(0),
                destination: OwnerPlaceId(2),
            },
            _ => unreachable!(),
        };
        p.functions[0].blocks[0].statements.push(ins(bad, s));
        denied(p, &m);
    }
    let mut p = owned_call(s);
    p.functions[1].blocks[0]
        .statements
        .push(ins(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s));
    denied(p, &m);
    let mut p = owned_call(s);
    p.functions[0].blocks[1]
        .statements
        .push(ins(OwnedInstruction::Discard(OwnerPlaceId(2)), s));
    denied(p, &m);
}
#[test]
fn reviewer_permuted_alias_requests_and_access_boundaries() {
    let (m, s) = ctx();
    let mut count = 0;
    for roots in 0..16usize {
        for modes in 0..16usize {
            let specs: Vec<_> = (0..4)
                .map(|i| {
                    (
                        (roots >> i) & 1,
                        if (modes >> i) & 1 == 0 {
                            BorrowKind::Shared
                        } else {
                            BorrowKind::Exclusive
                        },
                    )
                })
                .collect();
            let expected = (0..4).all(|i| {
                (0..i).all(|j| {
                    specs[i].0 != specs[j].0
                        || (specs[i].1 == BorrowKind::Shared && specs[j].1 == BorrowKind::Shared)
                })
            });
            assert_eq!(
                verify_owned(borrow_program(s, &specs), &m).is_ok(),
                expected,
                "roots={roots}, modes={modes}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 256);
    for kind in [BorrowKind::Shared, BorrowKind::Exclusive] {
        for boundary in [false, true] {
            for write in [false, true] {
                let mut p = borrow_program(s, &[(0, kind)]);
                let f = &mut p.functions[0];
                f.locals.push(decl(hir::Ty::I32, s));
                let access = if write {
                    OwnedInstruction::WriteField {
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        field: field(0),
                        value: op(1, s),
                    }
                } else {
                    OwnedInstruction::ReadField {
                        destination: LocalId(4),
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        field: field(0),
                    }
                };
                f.blocks[usize::from(boundary)]
                    .statements
                    .push(ins(access, s));
                assert_eq!(
                    verify_owned(p, &m).is_ok(),
                    boundary || (!write && kind == BorrowKind::Shared)
                );
            }
        }
    }
}
#[test]
fn reviewer_nonzero_entry_backedge_and_missing_end() {
    let (m, s) = ctx();
    for end_lifetime in [false, true] {
        let mut p = base(s, 1);
        let f = &mut p.functions[0];
        let lifecycle = f.blocks[0].statements.split_off(3);
        f.blocks[0].terminator = term(OwnedTerminatorKind::Goto(BlockId(1)), s);
        f.blocks.push(OwnedBlock {
            merge: None,
            span: s,
            statements: lifecycle,
            terminator: term(
                OwnedTerminatorKind::Branch {
                    condition: op(2, s),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
                s,
            ),
        });
        if end_lifetime {
            f.blocks[1]
                .statements
                .push(ins(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s));
        }
        f.blocks.push(OwnedBlock {
            merge: None,
            span: s,
            statements: vec![],
            terminator: term(OwnedTerminatorKind::ReturnScalar(op(0, s)), s),
        });
        // Permute physical entry to 2, retaining the cyclic logical header at 0.
        f.blocks.rotate_left(1);
        f.entry = BlockId(2);
        for b in &mut f.blocks {
            match &mut b.terminator.as_mut().unwrap().kind {
                OwnedTerminatorKind::Goto(t) => t.0 = (t.0 + 2) % 3,
                OwnedTerminatorKind::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    then_block.0 = (then_block.0 + 2) % 3;
                    else_block.0 = (else_block.0 + 2) % 3;
                }
                _ => {}
            }
        }
        assert_eq!(verify_owned(p, &m).is_ok(), end_lifetime);
    }
}

fn permutations(values: &mut [usize], at: usize, out: &mut Vec<Vec<usize>>) {
    if at == values.len() {
        out.push(values.to_vec());
        return;
    }
    for i in at..values.len() {
        values.swap(at, i);
        permutations(values, at + 1, out);
        values.swap(at, i);
    }
}
#[test]
fn reviewer_call_forest_all_unique_event_orders() {
    let (m, s) = ctx();
    let mut orders = vec![];
    permutations(&mut [0, 1, 2, 3, 4], 0, &mut orders);
    let mut accepts = 0;
    for nested in [false, true] {
        for order in &orders {
            let mut p = base(s, 0);
            let mut outer = body(1, s);
            outer.locals[0].kind = LocalKind::Parameter;
            outer.parameters.push(ParameterBinding::Scalar(LocalId(0)));
            outer.blocks[0].statements.remove(0);
            p.functions.push(outer);
            p.functions.push(body(2, s));
            let f = &mut p.functions[0];
            f.locals
                .extend([decl(hir::Ty::Unit, s), decl(hir::Ty::Unit, s)]);
            f.calls = vec![
                CallDecl {
                    target: hir::DefId(1),
                    arguments: vec![ArgumentSlot::Scalar],
                    result: CallResult::Scalar(LocalId(3)),
                    parent: None,
                    span: s,
                },
                CallDecl {
                    target: hir::DefId(2),
                    arguments: vec![],
                    result: CallResult::Scalar(LocalId(4)),
                    parent: if nested {
                        Some((CallSiteId(0), 0))
                    } else {
                        None
                    },
                    span: s,
                },
            ];
            f.blocks[0].terminator = term(OwnedTerminatorKind::Goto(BlockId(1)), s);
            let mut stack: Vec<(usize, usize)> = vec![];
            let mut expected = true;
            for (idx, event) in order.iter().enumerate() {
                let next = BlockId(idx + 2);
                let (statement, end) = match event {
                    0 => (
                        Some(OwnedInstruction::OpenCall(CallSiteId(0))),
                        OwnedTerminatorKind::Goto(next),
                    ),
                    1 => (
                        Some(OwnedInstruction::PrepareScalar {
                            call: CallSiteId(0),
                            argument: 0,
                            value: op(0, s),
                        }),
                        OwnedTerminatorKind::Goto(next),
                    ),
                    2 => (
                        None,
                        OwnedTerminatorKind::Invoke {
                            call: CallSiteId(0),
                            continuation: next,
                        },
                    ),
                    3 => (
                        Some(OwnedInstruction::OpenCall(CallSiteId(1))),
                        OwnedTerminatorKind::Goto(next),
                    ),
                    4 => (
                        None,
                        OwnedTerminatorKind::Invoke {
                            call: CallSiteId(1),
                            continuation: next,
                        },
                    ),
                    _ => unreachable!(),
                };
                if expected {
                    match event {
                        0 => {
                            expected = stack.is_empty();
                            stack.push((0, 0));
                        }
                        1 => {
                            expected = stack.last() == Some(&(0, 0));
                            if expected {
                                stack.last_mut().unwrap().1 = 1;
                            }
                        }
                        2 => {
                            expected = stack.last() == Some(&(0, 1));
                            if expected {
                                stack.pop();
                            }
                        }
                        3 => {
                            expected = if nested {
                                stack.last() == Some(&(0, 0))
                            } else {
                                stack.is_empty()
                            };
                            stack.push((1, 0));
                        }
                        4 => {
                            expected = stack.last() == Some(&(1, 0));
                            if expected {
                                stack.pop();
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                f.blocks.push(OwnedBlock {
                    merge: None,
                    span: s,
                    statements: statement.into_iter().map(|x| ins(x, s)).collect(),
                    terminator: term(end, s),
                });
            }
            f.blocks.push(OwnedBlock {
                merge: None,
                span: s,
                statements: vec![],
                terminator: term(OwnedTerminatorKind::ReturnScalar(op(0, s)), s),
            });
            expected &= stack.is_empty();
            assert_eq!(
                verify_owned(p, &m).is_ok(),
                expected,
                "nested={nested}, order={order:?}"
            );
            accepts += usize::from(expected);
        }
    }
    assert_eq!(accepts, 3);
}
#[test]
fn reviewer_reference_authority_is_separate_from_parent_owner() {
    let (m, s) = ctx();
    for incoming in [BorrowKind::Shared, BorrowKind::Exclusive] {
        for child in [BorrowKind::Shared, BorrowKind::Exclusive] {
            for write in [false, true] {
                let mut p = borrow_program(s, &[(0, incoming)]);
                let mut leaf = body(2, s);
                leaf.parameters
                    .push(ParameterBinding::Reference(ReferenceParamId(0)));
                leaf.references.push(ReferenceDecl {
                    referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(
                        RecordId(0),
                    )))
                    .unwrap(),
                    kind: child,
                    position: 0,
                    span: s,
                });
                p.functions.push(leaf);
                let f = &mut p.functions[1];
                f.locals
                    .extend([decl(hir::Ty::Unit, s), decl(hir::Ty::I32, s)]);
                f.calls.push(CallDecl {
                    target: hir::DefId(2),
                    arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
                    result: CallResult::Scalar(LocalId(3)),
                    parent: None,
                    span: s,
                });
                f.loans.push(LoanDecl {
                    projection: Vec::new(),
                    call: CallSiteId(0),
                    argument: 0,
                    authority: AccessBase::Parameter(ReferenceParamId(0)),
                    kind: child,
                    referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(
                        RecordId(0),
                    )))
                    .unwrap(),
                    span: s,
                });
                f.blocks[0].statements.extend([
                    ins(OwnedInstruction::OpenCall(CallSiteId(0)), s),
                    ins(
                        OwnedInstruction::PrepareBorrow {
                            call: CallSiteId(0),
                            argument: 0,
                            loan: LoanId(0),
                        },
                        s,
                    ),
                ]);
                let access = if write {
                    OwnedInstruction::WriteField {
                        base: AccessBase::Parameter(ReferenceParamId(0)),
                        field: field(0),
                        value: op(1, s),
                    }
                } else {
                    OwnedInstruction::ReadField {
                        destination: LocalId(4),
                        base: AccessBase::Parameter(ReferenceParamId(0)),
                        field: field(0),
                    }
                };
                f.blocks[0].statements.push(ins(access, s));
                f.blocks[0].terminator = term(
                    OwnedTerminatorKind::Invoke {
                        call: CallSiteId(0),
                        continuation: BlockId(1),
                    },
                    s,
                );
                f.blocks.push(OwnedBlock {
                    merge: None,
                    span: s,
                    statements: vec![],
                    terminator: term(OwnedTerminatorKind::ReturnScalar(op(3, s)), s),
                });
                let expected = !write && child == BorrowKind::Shared;
                assert_eq!(
                    verify_owned(p, &m).is_ok(),
                    expected,
                    "parent={incoming:?}, child={child:?}, write={write}"
                );
            }
        }
    }
    // Two incoming shared parameters may alias one runtime owner. Both can
    // independently supply shared reborrows to a further helper.
    let mut p = borrow_program(s, &[(0, BorrowKind::Shared), (0, BorrowKind::Shared)]);
    let mut leaf = body(2, s);
    for i in 0..2 {
        leaf.parameters
            .push(ParameterBinding::Reference(ReferenceParamId(i)));
        leaf.references.push(ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            kind: BorrowKind::Shared,
            position: i,
            span: s,
        });
    }
    p.functions.push(leaf);
    let f = &mut p.functions[1];
    f.locals.push(decl(hir::Ty::Unit, s));
    f.calls.push(CallDecl {
        target: hir::DefId(2),
        arguments: vec![
            ArgumentSlot::Borrow(LoanId(0)),
            ArgumentSlot::Borrow(LoanId(1)),
        ],
        result: CallResult::Scalar(LocalId(3)),
        parent: None,
        span: s,
    });
    f.blocks[0]
        .statements
        .push(ins(OwnedInstruction::OpenCall(CallSiteId(0)), s));
    for i in 0..2 {
        f.loans.push(LoanDecl {
            projection: Vec::new(),
            call: CallSiteId(0),
            argument: i,
            authority: AccessBase::Parameter(ReferenceParamId(i)),
            kind: BorrowKind::Shared,
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            span: s,
        });
        f.blocks[0].statements.push(ins(
            OwnedInstruction::PrepareBorrow {
                call: CallSiteId(0),
                argument: i,
                loan: LoanId(i),
            },
            s,
        ));
    }
    f.blocks[0].terminator = term(
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s,
    );
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: vec![],
        terminator: term(OwnedTerminatorKind::ReturnScalar(op(3, s)), s),
    });
    assert!(verify_owned(p, &m).is_ok());
}

#[test]
fn reviewer_adapter_catches_forward_uses_in_every_extra_scalar_operand() {
    let (m, s) = ctx();
    for constructor in [false, true] {
        let mut p = base(s, 1);
        let f = &mut p.functions[0];
        f.locals.push(decl(hir::Ty::I32, s));
        if constructor {
            if let OwnedInstruction::Construct { fields, .. } = &mut f.blocks[0].statements[4].kind
            {
                fields[1].1 = op(3, s);
            }
        } else {
            f.blocks[0].statements.push(ins(
                OwnedInstruction::WriteField {
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    field: field(0),
                    value: op(3, s),
                },
                s,
            ));
        }
        f.blocks[0].statements.push(scalar(3, Rvalue::I32(4), s));
        assert!(matches!(
            verify_owned(p, &m).unwrap_err().kind,
            OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Uninitialized))
        ));
    }
    let mut p = base(s, 0);
    let mut callee = body(1, s);
    callee.parameters.push(ParameterBinding::Scalar(LocalId(0)));
    callee.locals[0].kind = LocalKind::Parameter;
    callee.blocks[0].statements.remove(0);
    p.functions.push(callee);
    let f = &mut p.functions[0];
    f.locals
        .extend([decl(hir::Ty::Unit, s), decl(hir::Ty::Unit, s)]);
    f.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Scalar],
        result: CallResult::Scalar(LocalId(4)),
        parent: None,
        span: s,
    });
    f.blocks[0].statements.extend([
        ins(OwnedInstruction::OpenCall(CallSiteId(0)), s),
        ins(
            OwnedInstruction::PrepareScalar {
                call: CallSiteId(0),
                argument: 0,
                value: op(3, s),
            },
            s,
        ),
        scalar(3, Rvalue::Unit, s),
    ]);
    f.blocks[0].terminator = term(
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s,
    );
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: vec![],
        terminator: term(OwnedTerminatorKind::ReturnScalar(op(4, s)), s),
    });
    assert!(matches!(
        verify_owned(p, &m).unwrap_err().kind,
        OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Uninitialized))
    ));
}
fn edge_merge(s: Span) -> RawOwnedProgram {
    let mut p = base(s, 0);
    let mut callee = body(1, s);
    callee.result = ValueTy::Scalar(hir::Ty::Bool);
    callee.blocks[0].terminator = term(OwnedTerminatorKind::ReturnScalar(op(2, s)), s);
    p.functions.push(callee);
    let f = &mut p.functions[0];
    f.result = ValueTy::Scalar(hir::Ty::Bool);
    f.locals
        .extend([decl(hir::Ty::Bool, s), decl(hir::Ty::Bool, s)]);
    f.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![],
        result: CallResult::Scalar(LocalId(3)),
        parent: None,
        span: s,
    });
    f.blocks[0].terminator = term(
        OwnedTerminatorKind::Branch {
            condition: op(2, s),
            then_block: BlockId(1),
            else_block: BlockId(2),
        },
        s,
    );
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: vec![ins(OwnedInstruction::OpenCall(CallSiteId(0)), s)],
        terminator: term(
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(3),
            },
            s,
        ),
    });
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: vec![],
        terminator: term(OwnedTerminatorKind::Goto(BlockId(3)), s),
    });
    f.blocks.push(OwnedBlock {
        merge: Some(BoolMerge {
            operator_span: s,
            destination: LocalId(4),
            incoming: [
                MergeInput {
                    predecessor: BlockId(1),
                    value: op(3, s),
                },
                MergeInput {
                    predecessor: BlockId(2),
                    value: op(2, s),
                },
            ],
            span: s,
        }),
        span: s,
        statements: vec![],
        terminator: term(OwnedTerminatorKind::ReturnScalar(op(4, s)), s),
    });
    p
}
#[test]
fn reviewer_adapter_keeps_call_edge_merge_semantics() {
    let (m, s) = ctx();
    assert!(verify_owned(edge_merge(s), &m).is_ok());
    for variant in 0..4 {
        let mut p = edge_merge(s);
        let f = &mut p.functions[0];
        match variant {
            0 => f.blocks[3].merge.as_mut().unwrap().incoming[1].value = op(3, s),
            1 => f.blocks[3].merge.as_mut().unwrap().incoming[1].predecessor = BlockId(1),
            2 => {
                f.blocks[1]
                    .statements
                    .push(scalar(3, Rvalue::Bool(false), s));
            }
            3 => {
                f.blocks[1].terminator = term(
                    OwnedTerminatorKind::Invoke {
                        call: CallSiteId(0),
                        continuation: BlockId(2),
                    },
                    s,
                )
            }
            _ => unreachable!(),
        }
        denied(p, &m);
    }
}
#[test]
fn reviewer_last_move_origin_cannot_cross_generation_reset() {
    let (m, s) = ctx();
    let mut p = base(s, 1);
    let f = &mut p.functions[0];
    let lifecycle = f.blocks[0].statements.split_off(3);
    f.blocks[0].terminator = term(OwnedTerminatorKind::Goto(BlockId(1)), s);
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: lifecycle,
        terminator: term(
            OwnedTerminatorKind::Branch {
                condition: op(2, s),
                then_block: BlockId(2),
                else_block: BlockId(3),
            },
            s,
        ),
    });
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: vec![
            ins(OwnedInstruction::Discard(OwnerPlaceId(0)), at(s, 10)),
            ins(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), at(s, 11)),
        ],
        terminator: term(OwnedTerminatorKind::Goto(BlockId(1)), s),
    });
    f.locals.push(decl(hir::Ty::I32, s));
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: vec![
            ins(OwnedInstruction::Discard(OwnerPlaceId(0)), at(s, 90)),
            ins(
                OwnedInstruction::ReadField {
                    destination: LocalId(3),
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    field: field(0),
                },
                at(s, 120),
            ),
        ],
        terminator: term(OwnedTerminatorKind::ReturnScalar(op(0, s)), s),
    });
    let e = verify_owned(p, &m).unwrap_err();
    assert_eq!(e.kind, OwnedFailureKind::Ownership(Violation::Unavailable));
    assert_eq!(e.related.get(), Some(at(s, 90)));
}

#[test]
fn reviewer_heldout_two_owner_irreducible_graph_corpus() {
    let (m, s) = ctx();
    let mut rng = 0x57f8_ba1d_3344_9017u64;
    let mut checked = 0;
    let mut accepted = 0;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng as usize
    };
    for _ in 0..12000 {
        let n = 4 + next() % 2;
        let entry = next() % n;
        let actions: Vec<usize> = (0..n).map(|_| next() % 7).collect();
        let successors: Vec<Vec<usize>> = (0..n)
            .map(|_| (0..next() % 3).map(|_| next() % n).collect())
            .collect();
        let mut reached = vec![false; n];
        let mut pending = vec![entry];
        while let Some(b) = pending.pop() {
            if !reached[b] {
                reached[b] = true;
                pending.extend_from_slice(&successors[b]);
            }
        }
        if reached.iter().any(|r| !*r) {
            continue;
        }
        // Independent operational oracle: 0=available, 1=moved, 2=dead.
        let mut seen = std::collections::BTreeSet::new();
        let mut pending = vec![(entry, [0u8; 2])];
        let mut expected = true;
        while let Some((b, mut state)) = pending.pop() {
            if !seen.insert((b, state)) {
                continue;
            }
            let a = actions[b];
            if a != 0 {
                let owner = (a - 1) % 2;
                let kind = (a - 1) / 2;
                match kind {
                    0 => {
                        if state[owner] != 0 {
                            expected = false;
                            break;
                        }
                    }
                    1 => {
                        if state[owner] != 0 {
                            expected = false;
                            break;
                        }
                        state[owner] = 1;
                    }
                    2 => {
                        if state[owner] == 2 {
                            expected = false;
                            break;
                        }
                        state[owner] = 2;
                    }
                    _ => unreachable!(),
                }
            }
            for &to in &successors[b] {
                pending.push((to, state));
            }
        }
        let mut p = base(s, 0);
        let f = &mut p.functions[0];
        f.locals = vec![
            LocalDecl {
                ty: hir::Ty::Bool,
                kind: LocalKind::Parameter,
                span: s,
            },
            decl(hir::Ty::Unit, s),
        ];
        f.parameters = vec![
            ParameterBinding::Scalar(LocalId(0)),
            ParameterBinding::Owned(OwnerPlaceId(0)),
            ParameterBinding::Owned(OwnerPlaceId(1)),
        ];
        f.owners = (0..2)
            .map(|i| OwnerDecl {
                aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0)))
                    .unwrap(),
                kind: OwnerKind::Parameter { position: i + 1 },
                span: s,
            })
            .collect();
        f.entry = BlockId(entry);
        f.blocks.clear();
        for b in 0..n {
            let mut statements = vec![];
            if b == entry {
                statements.push(scalar(1, Rvalue::Unit, s));
            }
            let a = actions[b];
            if a != 0 {
                let owner = OwnerPlaceId((a - 1) % 2);
                let kind = match (a - 1) / 2 {
                    0 => {
                        let id = f.locals.len();
                        f.locals.push(decl(hir::Ty::I32, s));
                        OwnedInstruction::ReadField {
                            destination: LocalId(id),
                            base: AccessBase::Owner(owner),
                            field: field(0),
                        }
                    }
                    1 => OwnedInstruction::Discard(owner),
                    2 => OwnedInstruction::StorageEnd(owner),
                    _ => unreachable!(),
                };
                statements.push(ins(kind, at(s, 100 + b)));
            }
            let end = match successors[b].as_slice() {
                [] => OwnedTerminatorKind::ReturnScalar(op(1, s)),
                [a] => OwnedTerminatorKind::Goto(BlockId(*a)),
                [a, b] => OwnedTerminatorKind::Branch {
                    condition: op(0, s),
                    then_block: BlockId(*a),
                    else_block: BlockId(*b),
                },
                _ => unreachable!(),
            };
            f.blocks.push(OwnedBlock {
                merge: None,
                span: s,
                statements,
                terminator: term(end, s),
            });
        }
        let actual = verify_owned(p, &m);
        assert_eq!(
            actual.is_ok(),
            expected,
            "entry={entry}, actions={actions:?}, successors={successors:?}, actual={actual:?}"
        );
        checked += 1;
        accepted += usize::from(expected);
    }
    println!("reviewer heldout graph seed=57f8ba1d33449017 candidates=12000 reachable={checked} accepted={accepted} mismatches=0");
    assert_eq!(checked, 987);
}

#[test]
fn reviewer_exhaustive_one_two_block_lifecycle_cfgs() {
    let (m, s) = ctx();
    let mut total = 0usize;
    let mut accepted = 0usize;
    let mut canonical_denials = 0usize;
    let mut unreachable_denials = 0usize;
    let mut ownership_denials = 0usize;
    for n in 1usize..=2 {
        let mut forms = vec![vec![]];
        for a in 0..n {
            forms.push(vec![a]);
        }
        for a in 0..n {
            for b in 0..n {
                forms.push(vec![a, b]);
            }
        }
        for graph_code in 0..forms.len().pow(n as u32) {
            let mut c = graph_code;
            let successors: Vec<Vec<usize>> = (0..n)
                .map(|_| {
                    let r = forms[c % forms.len()].clone();
                    c /= forms.len();
                    r
                })
                .collect();
            for entry in 0..n {
                let mut reached = vec![false; n];
                let mut pending = vec![entry];
                while let Some(b) = pending.pop() {
                    if !reached[b] {
                        reached[b] = true;
                        pending.extend_from_slice(&successors[b]);
                    }
                }
                let reachable = reached.iter().all(|r| *r);
                for action_code in 0..7usize.pow(n as u32) {
                    let mut c = action_code;
                    let actions: Vec<usize> = (0..n)
                        .map(|_| {
                            let r = c % 7;
                            c /= 7;
                            r
                        })
                        .collect();
                    for initial in 0u8..4 {
                        let lives =
                            usize::from(initial != 0) + actions.iter().filter(|a| **a == 4).count();
                        let inits =
                            usize::from(initial >= 2) + actions.iter().filter(|a| **a == 5).count();
                        let canonical = lives == 1 && inits <= 1;
                        let mut seen = std::collections::BTreeSet::new();
                        let mut pending = vec![(entry, initial)];
                        let mut semantically_valid = true;
                        while let Some((b, mut state)) = pending.pop() {
                            if !seen.insert((b, state)) {
                                continue;
                            }
                            let next = match actions[b] {
                                0 => Some(state),
                                1 if state == 2 => Some(state),
                                2 if state == 2 => Some(3),
                                3 if state == 2 || state == 3 => Some(2),
                                4 if state == 0 => Some(1),
                                5 if state == 1 => Some(2),
                                6 if state != 0 => Some(0),
                                _ => None,
                            };
                            if let Some(v) = next {
                                state = v;
                            } else {
                                semantically_valid = false;
                                break;
                            }
                            for &to in &successors[b] {
                                pending.push((to, state));
                            }
                        }
                        for prologue in 0..=n {
                            let physical = |b: usize| BlockId(b + usize::from(b >= prologue));
                            let mut p = base(s, 0);
                            let f = &mut p.functions[0];
                            f.owners.push(OwnerDecl {
                                aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(
                                    RecordId(0),
                                ))
                                .unwrap(),
                                kind: OwnerKind::Local { mutable: true },
                                span: s,
                            });
                            let mut setup = std::mem::take(&mut f.blocks[0].statements);
                            f.blocks.clear();
                            if initial != 0 {
                                setup.push(ins(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s));
                            }
                            if initial >= 2 {
                                setup.push(construct(0, s));
                            }
                            if initial == 3 {
                                setup.push(ins(OwnedInstruction::Discard(OwnerPlaceId(0)), s));
                            }
                            for b in 0..n {
                                let mut statements = vec![];
                                match actions[b] {
                                    0 => {}
                                    1 => {
                                        let id = f.locals.len();
                                        f.locals.push(decl(hir::Ty::I32, s));
                                        statements.push(ins(
                                            OwnedInstruction::ReadField {
                                                destination: LocalId(id),
                                                base: AccessBase::Owner(OwnerPlaceId(0)),
                                                field: field(0),
                                            },
                                            s,
                                        ));
                                    }
                                    2 => statements
                                        .push(ins(OwnedInstruction::Discard(OwnerPlaceId(0)), s)),
                                    3 => {
                                        let id = f.owners.len();
                                        f.owners.push(OwnerDecl {
                                            aggregate: AggregateSlot::try_from_aggregate(
                                                AggregateTy::Record(RecordId(0)),
                                            )
                                            .unwrap(),
                                            kind: OwnerKind::Temporary,
                                            span: s,
                                        });
                                        statements.extend([
                                            ins(OwnedInstruction::StorageLive(OwnerPlaceId(id)), s),
                                            construct(id, s),
                                            ins(
                                                OwnedInstruction::Replace {
                                                    destination: OwnerPlaceId(0),
                                                    source: OwnerPlaceId(id),
                                                },
                                                s,
                                            ),
                                            ins(OwnedInstruction::StorageEnd(OwnerPlaceId(id)), s),
                                        ]);
                                    }
                                    4 => statements.push(ins(
                                        OwnedInstruction::StorageLive(OwnerPlaceId(0)),
                                        s,
                                    )),
                                    5 => statements.push(construct(0, s)),
                                    6 => statements.push(ins(
                                        OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
                                        s,
                                    )),
                                    _ => unreachable!(),
                                }
                                let end = match successors[b].as_slice() {
                                    [] => OwnedTerminatorKind::ReturnScalar(op(0, s)),
                                    [a] => OwnedTerminatorKind::Goto(physical(*a)),
                                    [a, b] => OwnedTerminatorKind::Branch {
                                        condition: op(2, s),
                                        then_block: physical(*a),
                                        else_block: physical(*b),
                                    },
                                    _ => unreachable!(),
                                };
                                f.blocks.push(OwnedBlock {
                                    merge: None,
                                    span: s,
                                    statements,
                                    terminator: term(end, s),
                                });
                            }
                            f.entry = BlockId(prologue);
                            f.blocks.insert(
                                prologue,
                                OwnedBlock {
                                    merge: None,
                                    span: s,
                                    statements: setup,
                                    terminator: term(OwnedTerminatorKind::Goto(physical(entry)), s),
                                },
                            );
                            let actual = verify_owned(p, &m);
                            let expected = canonical && reachable && semantically_valid;
                            assert_eq!(actual.is_ok(),expected,"n={n},graph={graph_code},entry={entry},actions={actions:?},initial={initial},prologue={prologue},actual={actual:?}");
                            total += 1;
                            if expected {
                                accepted += 1;
                            } else if !canonical {
                                assert!(matches!(
                                    actual.unwrap_err().kind,
                                    OwnedFailureKind::Malformed(Malformed::CanonicalSite)
                                ));
                                canonical_denials += 1;
                            } else if !reachable {
                                assert!(matches!(
                                    actual.unwrap_err().kind,
                                    OwnedFailureKind::Malformed(Malformed::Scalar(
                                        FailureKind::Unreachable
                                    ))
                                ));
                                unreachable_denials += 1;
                            } else {
                                assert!(matches!(
                                    actual.unwrap_err().kind,
                                    OwnedFailureKind::Ownership(_)
                                ));
                                ownership_denials += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("reviewer lifecycle CFG: {total} raw comparisons, {accepted} accepted, {canonical_denials} canonical denials, {unreachable_denials} unreachable denials, {ownership_denials} ownership denials, zero mismatches");
    assert_eq!(total, 57792);
}

fn sibling_diamond(s: Span) -> RawOwnedProgram {
    let mut p = base(s, 0);
    let mut outer = body(1, s);
    outer.locals[0].kind = LocalKind::Parameter;
    outer.parameters.push(ParameterBinding::Scalar(LocalId(0)));
    outer.blocks[0].statements.remove(0);
    p.functions.push(outer);
    p.functions.push(body(2, s));
    let f = &mut p.functions[0];
    f.locals.extend([
        decl(hir::Ty::Unit, s),
        decl(hir::Ty::Unit, s),
        decl(hir::Ty::Unit, s),
    ]);
    f.calls = vec![
        CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Scalar],
            result: CallResult::Scalar(LocalId(3)),
            parent: None,
            span: s,
        },
        CallDecl {
            target: hir::DefId(2),
            arguments: vec![],
            result: CallResult::Scalar(LocalId(4)),
            parent: Some((CallSiteId(0), 0)),
            span: s,
        },
        CallDecl {
            target: hir::DefId(2),
            arguments: vec![],
            result: CallResult::Scalar(LocalId(5)),
            parent: Some((CallSiteId(0), 0)),
            span: s,
        },
    ];
    f.blocks[0]
        .statements
        .push(ins(OwnedInstruction::OpenCall(CallSiteId(0)), s));
    f.blocks[0].terminator = term(
        OwnedTerminatorKind::Branch {
            condition: op(2, s),
            then_block: BlockId(1),
            else_block: BlockId(3),
        },
        s,
    );
    for (call, continuation) in [(1, 2), (2, 4)] {
        f.blocks.push(OwnedBlock {
            merge: None,
            span: s,
            statements: vec![ins(OwnedInstruction::OpenCall(CallSiteId(call)), s)],
            terminator: term(
                OwnedTerminatorKind::Invoke {
                    call: CallSiteId(call),
                    continuation: BlockId(continuation),
                },
                s,
            ),
        });
        f.blocks.push(OwnedBlock {
            merge: None,
            span: s,
            statements: vec![],
            terminator: term(OwnedTerminatorKind::Goto(BlockId(5)), s),
        });
    }
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: vec![ins(
            OwnedInstruction::PrepareScalar {
                call: CallSiteId(0),
                argument: 0,
                value: op(0, s),
            },
            s,
        )],
        terminator: term(
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(6),
            },
            s,
        ),
    });
    f.blocks.push(OwnedBlock {
        merge: None,
        span: s,
        statements: vec![],
        terminator: term(OwnedTerminatorKind::ReturnScalar(op(0, s)), s),
    });
    p
}
#[test]
fn reviewer_branching_sibling_call_regions_and_scrambled_forest_ids() {
    let (m, s) = ctx();
    assert!(verify_owned(sibling_diamond(s), &m).is_ok());
    let mut p = sibling_diamond(s);
    let f = &mut p.functions[0];
    f.calls.swap(0, 1);
    let remap = |x: usize| match x {
        0 => 1,
        1 => 0,
        other => other,
    };
    for c in &mut f.calls {
        if let Some((parent, _)) = &mut c.parent {
            parent.0 = remap(parent.0);
        }
    }
    for b in &mut f.blocks {
        for i in &mut b.statements {
            match &mut i.kind {
                OwnedInstruction::OpenCall(c) | OwnedInstruction::PrepareScalar { call: c, .. } => {
                    c.0 = remap(c.0)
                }
                _ => {}
            }
        }
        if let OwnedTerminatorKind::Invoke { call, .. } = &mut b.terminator.as_mut().unwrap().kind {
            call.0 = remap(call.0);
        }
    }
    assert!(verify_owned(p, &m).is_ok());
    let mut p = sibling_diamond(s);
    let f = &mut p.functions[0];
    f.blocks[2].terminator = f.blocks[1].terminator.clone();
    if let OwnedTerminatorKind::Invoke { continuation, .. } =
        &mut f.blocks[2].terminator.as_mut().unwrap().kind
    {
        *continuation = BlockId(5);
    }
    f.blocks[1].terminator = term(
        OwnedTerminatorKind::Branch {
            condition: op(2, s),
            then_block: BlockId(2),
            else_block: BlockId(5),
        },
        s,
    );
    assert!(matches!(
        verify_owned(p, &m).unwrap_err().kind,
        OwnedFailureKind::Ownership(Violation::CallRegion)
    ));
    let mut p = sibling_diamond(s);
    p.functions[0].calls[1].parent = None;
    denied(p, &m);
}
