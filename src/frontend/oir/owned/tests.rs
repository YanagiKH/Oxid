use super::*;
fn context() -> (SourceMap, Span) {
    let mut sources = SourceMap::new();
    let file = sources.add("owned-raw.ox".into(), "x".repeat(200));
    (
        sources,
        Span {
            file,
            start: 0,
            end: 1,
        },
    )
}
fn subject(span: Span) -> RawOwnedProgram {
    RawOwnedProgram {
        records: vec![RawRecordDecl {
            id: RecordId(0),
            span,
            fields: vec![],
        }],
        functions: vec![RawOwnedFunction {
            id: hir::DefId(0),
            span,
            result: ValueTy::Scalar(hir::Ty::Unit),
            parameters: vec![],
            locals: vec![LocalDecl {
                ty: hir::Ty::Unit,
                kind: LocalKind::Temporary,
                span,
            }],
            places: vec![],
            owners: vec![OwnerDecl {
                aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0)))
                    .unwrap(),
                kind: OwnerKind::Local { mutable: true },
                span,
            }],
            references: vec![],
            calls: vec![],
            loans: vec![],
            entry: BlockId(0),
            blocks: vec![OwnedBlock {
                merge: None,
                span,
                statements: vec![
                    OwnedStatement {
                        diagnostic_origins: None,
                        kind: OwnedInstruction::Scalar(Statement::Assign(Assign {
                            destination: LocalId(0),
                            value: Rvalue::Unit,
                            span,
                        })),
                        span,
                    },
                    OwnedStatement {
                        diagnostic_origins: None,
                        kind: OwnedInstruction::StorageLive(OwnerPlaceId(0)),
                        span,
                    },
                    OwnedStatement {
                        diagnostic_origins: None,
                        kind: OwnedInstruction::Construct {
                            destination: OwnerPlaceId(0),
                            fields: vec![],
                        },
                        span,
                    },
                    OwnedStatement {
                        diagnostic_origins: None,
                        kind: OwnedInstruction::Discard(OwnerPlaceId(0)),
                        span,
                    },
                ],
                terminator: Some(OwnedTerminator {
                    diagnostic_origins: None,
                    kind: OwnedTerminatorKind::ReturnScalar(Operand {
                        local: LocalId(0),
                        span,
                    }),
                    span,
                }),
            }],
        }],
    }
}
#[test]
fn verifies_whole_owner_consumption_without_execution_api() {
    let (sources, span) = context();
    let checked =
        verify_owned(subject(span), &sources).expect("well-shaped whole owner must verify");
    assert_eq!(checked.functions().len(), 1);
    assert_eq!(checked.usage().owners, 1);
}
fn instruction(kind: OwnedInstruction, span: Span) -> OwnedStatement {
    OwnedStatement {
        diagnostic_origins: None,
        kind,
        span,
    }
}
fn end(kind: OwnedTerminatorKind, span: Span) -> Option<OwnedTerminator> {
    Some(OwnedTerminator {
        diagnostic_origins: None,
        kind,
        span,
    })
}
fn unit_function(id: usize, span: Span) -> RawOwnedFunction {
    RawOwnedFunction {
        id: hir::DefId(id),
        span,
        result: ValueTy::Scalar(hir::Ty::Unit),
        parameters: vec![],
        locals: vec![LocalDecl {
            ty: hir::Ty::Unit,
            kind: LocalKind::Temporary,
            span,
        }],
        places: vec![],
        owners: vec![],
        references: vec![],
        calls: vec![],
        loans: vec![],
        entry: BlockId(0),
        blocks: vec![OwnedBlock {
            span,
            merge: None,
            statements: vec![instruction(
                OwnedInstruction::Scalar(Statement::Assign(Assign {
                    destination: LocalId(0),
                    value: Rvalue::Unit,
                    span,
                })),
                span,
            )],
            terminator: end(
                OwnedTerminatorKind::ReturnScalar(Operand {
                    local: LocalId(0),
                    span,
                }),
                span,
            ),
        }],
    }
}
fn error(raw: RawOwnedProgram, sources: &SourceMap) -> OwnedFailure {
    verify_owned(raw, sources).expect_err("invalid raw ownership input cannot obtain seal")
}
fn borrowed(span: Span, kind: BorrowKind) -> RawOwnedProgram {
    let mut raw = subject(span);
    raw.functions[0].blocks[0].statements.pop();
    let mut callee = unit_function(1, span);
    callee
        .parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    callee.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        kind,
        position: 0,
        span,
    });
    raw.functions.push(callee);
    let f = &mut raw.functions[0];
    f.locals.push(LocalDecl {
        ty: hir::Ty::Unit,
        kind: LocalKind::Temporary,
        span,
    });
    f.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
        result: CallResult::Scalar(LocalId(1)),
        parent: None,
        span,
    });
    f.loans.push(LoanDecl {
        call: CallSiteId(0),
        argument: 0,
        authority: AccessBase::Owner(OwnerPlaceId(0)),
        kind,
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        span,
    });
    f.blocks[0].statements.extend([
        instruction(OwnedInstruction::OpenCall(CallSiteId(0)), span),
        instruction(
            OwnedInstruction::PrepareBorrow {
                call: CallSiteId(0),
                argument: 0,
                loan: LoanId(0),
            },
            span,
        ),
    ]);
    f.blocks[0].terminator = end(
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        span,
    );
    f.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![instruction(
            OwnedInstruction::Discard(OwnerPlaceId(0)),
            span,
        )],
        terminator: end(
            OwnedTerminatorKind::ReturnScalar(Operand {
                local: LocalId(1),
                span,
            }),
            span,
        ),
    });
    raw
}
#[test]
fn loans_end_only_on_owning_normal_return() {
    let (sources, s) = context();
    for kind in [BorrowKind::Shared, BorrowKind::Exclusive] {
        assert!(verify_owned(borrowed(s, kind), &sources).is_ok());
        let mut raw = borrowed(s, kind);
        raw.functions[0].blocks[0]
            .statements
            .push(instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s));
        raw.functions[0].blocks[1].statements.clear();
        assert_eq!(
            error(raw, &sources).kind,
            OwnedFailureKind::Ownership(Violation::LoanConflict)
        );
    }
}
#[test]
fn malformed_unreachable_instruction_precedes_reachability_and_ownership() {
    let (sources, s) = context();
    let mut raw = subject(s);
    raw.functions[0].blocks[0]
        .statements
        .push(instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s));
    raw.functions[0].blocks.push(OwnedBlock {
        span: s,
        merge: None,
        statements: vec![instruction(
            OwnedInstruction::Discard(OwnerPlaceId(usize::MAX)),
            s,
        )],
        terminator: end(
            OwnedTerminatorKind::ReturnScalar(Operand {
                local: LocalId(0),
                span: s,
            }),
            s,
        ),
    });
    assert_eq!(
        error(raw, &sources).kind,
        OwnedFailureKind::Malformed(Malformed::Id)
    );
}
#[test]
fn ownership_class_matrix_denies_counterfeit_lifecycle_and_staged_access() {
    let (sources, s) = context();
    for op in [
        OwnedInstruction::StorageLive(OwnerPlaceId(0)),
        OwnedInstruction::Construct {
            destination: OwnerPlaceId(0),
            fields: vec![],
        },
        OwnedInstruction::MoveInitialize {
            destination: OwnerPlaceId(0),
            source: OwnerPlaceId(1),
        },
        OwnedInstruction::Replace {
            destination: OwnerPlaceId(0),
            source: OwnerPlaceId(1),
        },
    ] {
        let mut raw = subject(s);
        let f = &mut raw.functions[0];
        f.owners[0].kind = OwnerKind::Parameter { position: 0 };
        f.parameters.push(ParameterBinding::Owned(OwnerPlaceId(0)));
        f.owners.push(OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
            kind: OwnerKind::Temporary,
            span: s,
        });
        f.blocks[0].statements = vec![instruction(op, s)];
        assert!(matches!(
            error(raw, &sources).kind,
            OwnedFailureKind::Malformed(_)
        ));
    }
    for op in [
        OwnedInstruction::StorageLive(OwnerPlaceId(1)),
        OwnedInstruction::StorageEnd(OwnerPlaceId(1)),
        OwnedInstruction::Discard(OwnerPlaceId(1)),
        OwnedInstruction::Construct {
            destination: OwnerPlaceId(1),
            fields: vec![],
        },
        OwnedInstruction::MoveInitialize {
            destination: OwnerPlaceId(1),
            source: OwnerPlaceId(0),
        },
        OwnedInstruction::Replace {
            destination: OwnerPlaceId(1),
            source: OwnerPlaceId(0),
        },
    ] {
        let mut raw = subject(s);
        let mut callee = unit_function(1, s);
        callee
            .parameters
            .push(ParameterBinding::Owned(OwnerPlaceId(0)));
        callee.owners.push(OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
            kind: OwnerKind::Parameter { position: 0 },
            span: s,
        });
        raw.functions.push(callee);
        let f = &mut raw.functions[0];
        f.calls.push(CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(1))],
            result: CallResult::Scalar(LocalId(0)),
            parent: None,
            span: s,
        });
        f.owners.push(OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
            kind: OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            span: s,
        });
        f.blocks[0].statements.push(instruction(op, s));
        assert!(matches!(
            error(raw, &sources).kind,
            OwnedFailureKind::Malformed(Malformed::OwnerClass)
        ));
    }
}
fn nested(span: Span) -> RawOwnedProgram {
    let mut f = unit_function(0, span);
    f.locals.extend([
        LocalDecl {
            ty: hir::Ty::Unit,
            kind: LocalKind::Temporary,
            span,
        },
        LocalDecl {
            ty: hir::Ty::Unit,
            kind: LocalKind::Temporary,
            span,
        },
    ]);
    f.calls = vec![
        CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Scalar],
            result: CallResult::Scalar(LocalId(1)),
            parent: None,
            span,
        },
        CallDecl {
            target: hir::DefId(2),
            arguments: vec![],
            result: CallResult::Scalar(LocalId(2)),
            parent: Some((CallSiteId(0), 0)),
            span,
        },
    ];
    f.blocks[0].statements.extend([
        instruction(OwnedInstruction::OpenCall(CallSiteId(0)), span),
        instruction(OwnedInstruction::OpenCall(CallSiteId(1)), span),
    ]);
    f.blocks[0].terminator = end(
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(1),
            continuation: BlockId(1),
        },
        span,
    );
    f.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![instruction(
            OwnedInstruction::PrepareScalar {
                call: CallSiteId(0),
                argument: 0,
                value: Operand {
                    local: LocalId(2),
                    span,
                },
            },
            span,
        )],
        terminator: end(
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(2),
            },
            span,
        ),
    });
    f.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![],
        terminator: end(
            OwnedTerminatorKind::ReturnScalar(Operand {
                local: LocalId(1),
                span,
            }),
            span,
        ),
    });
    let mut target = unit_function(1, span);
    target.parameters.push(ParameterBinding::Scalar(LocalId(0)));
    target.locals[0].kind = LocalKind::Parameter;
    target.blocks[0].statements.clear();
    RawOwnedProgram {
        records: vec![],
        functions: vec![f, target, unit_function(2, span)],
    }
}
#[test]
fn child_requires_exact_active_parent_and_ancestors_cannot_advance() {
    let (sources, s) = context();
    assert!(verify_owned(nested(s), &sources).is_ok());
    let mut raw = nested(s);
    let open = raw.functions[0].blocks[0].statements.remove(1);
    raw.functions[0].blocks[1].statements.insert(0, open);
    assert_eq!(
        error(raw, &sources).kind,
        OwnedFailureKind::Ownership(Violation::CallRegion)
    );
    let mut raw = nested(s);
    let mut prep = raw.functions[0].blocks[1].statements.remove(0);
    if let OwnedInstruction::PrepareScalar { value, .. } = &mut prep.kind {
        value.local = LocalId(0);
    }
    raw.functions[0].blocks[0].statements.push(prep);
    assert_eq!(
        error(raw, &sources).kind,
        OwnedFailureKind::Ownership(Violation::CallRegion)
    );
    let mut raw = nested(s);
    raw.functions[0].calls[1].parent = None;
    assert_eq!(
        error(raw, &sources).kind,
        OwnedFailureKind::Ownership(Violation::CallRegion)
    );
    let mut raw = nested(s);
    raw.functions[0].calls[0].parent = Some((CallSiteId(1), 0));
    assert!(matches!(
        error(raw, &sources).kind,
        OwnedFailureKind::Malformed(Malformed::CallParent)
    ));
}
#[test]
fn exact_regions_reject_one_arm_acquisition_before_apparent_release() {
    let (sources, s) = context();
    let mut raw = borrowed(s, BorrowKind::Shared);
    let f = &mut raw.functions[0];
    f.locals.push(LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span: s,
    });
    let acquire = f.blocks[0].statements.split_off(3);
    f.blocks[0].statements.push(instruction(
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            destination: LocalId(2),
            value: Rvalue::Bool(true),
            span: s,
        })),
        s,
    ));
    f.blocks[0].terminator = end(
        OwnedTerminatorKind::Branch {
            condition: Operand {
                local: LocalId(2),
                span: s,
            },
            then_block: BlockId(2),
            else_block: BlockId(3),
        },
        s,
    );
    f.blocks.push(OwnedBlock {
        span: s,
        merge: None,
        statements: acquire,
        terminator: end(OwnedTerminatorKind::Goto(BlockId(3)), s),
    });
    f.blocks.push(OwnedBlock {
        span: s,
        merge: None,
        statements: vec![],
        terminator: end(
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            s,
        ),
    });
    assert_eq!(
        error(raw, &sources).kind,
        OwnedFailureKind::Ownership(Violation::LoanRegion)
    );
}
#[test]
fn raw_type_binding_span_and_canonical_mutants_cannot_get_a_witness() {
    let (sources, s) = context();
    let mut cases = Vec::new();
    let mut raw = borrowed(s, BorrowKind::Shared);
    raw.functions[0].loans[0].span.start = 1;
    cases.push(raw);
    let mut raw = borrowed(s, BorrowKind::Shared);
    raw.functions[0].loans[0].argument = usize::MAX;
    cases.push(raw);
    let mut raw = borrowed(s, BorrowKind::Shared);
    raw.functions[0].loans[0].authority = AccessBase::Parameter(ReferenceParamId(0));
    cases.push(raw);
    let mut raw = borrowed(s, BorrowKind::Shared);
    raw.functions[0].calls[0].target = hir::DefId(usize::MAX);
    cases.push(raw);
    let mut raw = borrowed(s, BorrowKind::Shared);
    raw.functions[0].blocks[0]
        .statements
        .push(instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s));
    cases.push(raw);
    let mut raw = borrowed(s, BorrowKind::Shared);
    raw.functions[1]
        .parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    cases.push(raw);
    let mut raw = subject(s);
    raw.functions[0].owners[0].aggregate =
        AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(u32::MAX as usize)))
            .unwrap();
    cases.push(raw);
    let mut raw = subject(s);
    raw.functions[0].blocks[0].span.end = 201;
    cases.push(raw);
    for raw in cases {
        assert!(matches!(
            error(raw, &sources).kind,
            OwnedFailureKind::Malformed(_)
        ));
    }
}
#[test]
fn resource_ceilings_are_inclusive_lowerable_and_precede_shape_allocations() {
    let (sources, s) = context();
    let usage = budget::preflight(&subject(s), budget::Limits::DEFAULT).unwrap();
    let exact = budget::Limits {
        owners: usage.owners,
        events: usage.expanded_events,
        work: usage.work,
        scratch: usage.scratch_bytes,
        metadata: usage.metadata_bytes,
    };
    assert!(verify_with_limits(subject(s), &sources, exact).is_ok());
    for limits in [
        budget::Limits {
            owners: exact.owners - 1,
            ..exact
        },
        budget::Limits {
            events: exact.events - 1,
            ..exact
        },
        budget::Limits {
            work: exact.work - 1,
            ..exact
        },
        budget::Limits {
            scratch: exact.scratch - 1,
            ..exact
        },
        budget::Limits {
            metadata: exact.metadata - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            verify_with_limits(subject(s), &sources, limits)
                .unwrap_err()
                .kind,
            OwnedFailureKind::Resource(_)
        ));
    }
    assert!(budget::add(usize::MAX, 1).is_err());
    assert!(budget::mul(usize::MAX, 2).is_err());
    assert!(budget::reserve::<u64>(usize::MAX).is_err());
    let mut raw = subject(s);
    // Compact carriers cannot construct usize::MAX; preserve this test's
    // resource-before-validation purpose with a representable invalid ordinal.
    raw.functions[0].owners[0].aggregate =
        AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(u32::MAX as usize)))
            .unwrap();
    assert_eq!(
        verify_with_limits(raw, &sources, budget::Limits { work: 0, ..exact })
            .unwrap_err()
            .kind,
        OwnedFailureKind::Resource("ownership work")
    );
}
#[test]
fn scalar_only_owned_adapter_bypasses_every_new_cap_and_keeps_raw_256_parameters() {
    let (sources, s) = context();
    for count in [0, 65, 256] {
        let mut f = unit_function(0, s);
        if count > 0 {
            f.locals = vec![
                LocalDecl {
                    ty: hir::Ty::Unit,
                    kind: LocalKind::Parameter,
                    span: s
                };
                count
            ];
            f.parameters = (0..count)
                .map(|i| ParameterBinding::Scalar(LocalId(i)))
                .collect();
            f.blocks[0].statements.clear();
        }
        assert!(verify_with_limits(
            RawOwnedProgram {
                records: vec![],
                functions: vec![f]
            },
            &sources,
            budget::Limits {
                owners: 0,
                events: 0,
                work: 0,
                scratch: 0,
                metadata: 0
            }
        )
        .is_ok());
    }
    for count in [65, 256] {
        let mut f = unit_function(0, s);
        f.references = (0..count)
            .map(|position| ReferenceDecl {
                referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                    .unwrap(),
                kind: BorrowKind::Shared,
                position,
                span: s,
            })
            .collect();
        f.parameters = (0..count)
            .map(|i| ParameterBinding::Reference(ReferenceParamId(i)))
            .collect();
        assert!(verify_owned(
            RawOwnedProgram {
                records: vec![RawRecordDecl {
                    id: RecordId(0),
                    span: s,
                    fields: vec![]
                }],
                functions: vec![f]
            },
            &sources
        )
        .is_ok());
    }
}
fn moved_join(s: Span) -> RawOwnedProgram {
    let mut raw = subject(s);
    let f = &mut raw.functions[0];
    f.blocks[0].statements.pop();
    f.locals.push(LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span: s,
    });
    f.blocks[0].statements.push(instruction(
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
    for (action, target, origin) in [
        (true, 4, 90),
        (false, 3, 30),
        (true, 4, 20),
        (true, usize::MAX, 120),
    ] {
        let span = Span {
            start: origin,
            end: origin + 1,
            ..s
        };
        f.blocks.push(OwnedBlock {
            span: s,
            merge: None,
            statements: if action {
                vec![instruction(
                    OwnedInstruction::Discard(OwnerPlaceId(0)),
                    span,
                )]
            } else {
                vec![]
            },
            terminator: if target == usize::MAX {
                end(
                    OwnedTerminatorKind::ReturnScalar(Operand {
                        local: LocalId(0),
                        span: s,
                    }),
                    s,
                )
            } else {
                end(OwnedTerminatorKind::Goto(BlockId(target)), s)
            },
        });
    }
    raw
}
#[test]
fn last_move_diagnostics_use_earliest_real_reaching_origin_after_saturation() {
    let (sources, s) = context();
    let failure = error(moved_join(s), &sources);
    assert_eq!(failure.primary.get().unwrap().start, 120);
    assert_eq!(failure.related.get().unwrap().start, 20);
    // The lower source origin is on a terminated arm, so it cannot explain join.
    let mut raw = moved_join(s);
    raw.functions[0].blocks[3].terminator = end(
        OwnedTerminatorKind::ReturnScalar(Operand {
            local: LocalId(0),
            span: s,
        }),
        s,
    );
    let failure = error(raw, &sources);
    assert_eq!(failure.related.get().unwrap().start, 90);
}
#[test]
fn every_added_reservation_failure_denies_the_seal_and_preflight_allocates_nothing() {
    let (sources, s) = context();
    let mut valid_count = 0;
    for nth in 0..200 {
        let result = budget::fail_allocation_after(nth, || {
            verify_owned(borrowed(s, BorrowKind::Shared), &sources)
        });
        match result {
            Ok(_) => break,
            Err(f) => {
                assert_eq!(
                    f.kind,
                    OwnedFailureKind::Resource("injected allocation failure")
                );
                valid_count += 1;
            }
        }
    }
    assert!(
        valid_count > 20,
        "all shape, owner, loan, and call region reservations exercised"
    );
    let mut failed_count = 0;
    for nth in 0..200 {
        let result = budget::fail_allocation_after(nth, || verify_owned(moved_join(s), &sources));
        match result {
            Err(f) if f.kind == OwnedFailureKind::Resource("injected allocation failure") => {
                failed_count += 1
            }
            Err(f) => {
                assert_eq!(f.kind, OwnedFailureKind::Ownership(Violation::Unavailable));
                break;
            }
            Ok(_) => panic!("invalid owner obtained seal"),
        }
    }
    assert!(
        failed_count > 20,
        "diagnostic predecessor and queue reservations exercised"
    );
    let failure = budget::fail_allocation_after(0, || {
        verify_with_limits(
            subject(s),
            &sources,
            budget::Limits {
                work: 0,
                ..budget::Limits::DEFAULT
            },
        )
    })
    .unwrap_err();
    assert_eq!(failure.kind, OwnedFailureKind::Resource("ownership work"));
    println!("allocation failures: valid-route {valid_count}, diagnostic-route {failed_count}");
}
#[test]
fn no_return_cycle_can_hold_a_loan_but_normal_exit_cannot() {
    let (sources, s) = context();
    let mut raw = borrowed(s, BorrowKind::Shared);
    let f = &mut raw.functions[0];
    f.locals.push(LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span: s,
    });
    f.blocks[0].statements.insert(
        1,
        instruction(
            OwnedInstruction::Scalar(Statement::Assign(Assign {
                destination: LocalId(2),
                value: Rvalue::Bool(true),
                span: s,
            })),
            s,
        ),
    );
    f.blocks[0].terminator = end(
        OwnedTerminatorKind::Branch {
            condition: Operand {
                local: LocalId(2),
                span: s,
            },
            then_block: BlockId(2),
            else_block: BlockId(3),
        },
        s,
    );
    f.blocks.push(OwnedBlock {
        span: s,
        merge: None,
        statements: vec![],
        terminator: end(OwnedTerminatorKind::Goto(BlockId(2)), s),
    });
    f.blocks.push(OwnedBlock {
        span: s,
        merge: None,
        statements: vec![],
        terminator: end(
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            s,
        ),
    });
    assert!(verify_owned(raw, &sources).is_ok());
}
#[test]
fn owned_result_initializes_only_on_return_and_is_consumed_once() {
    let (sources, s) = context();
    let mut raw = subject(s);
    let mut helper = unit_function(1, s);
    helper.result = ValueTy::Owned(AggregateTy::Record(RecordId(0)));
    helper
        .parameters
        .push(ParameterBinding::Owned(OwnerPlaceId(0)));
    helper.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
        kind: OwnerKind::Parameter { position: 0 },
        span: s,
    });
    helper.blocks[0].terminator = end(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)), s);
    raw.functions.push(helper);
    let f = &mut raw.functions[0];
    f.blocks[0].statements.pop();
    f.owners.extend([
        OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
            kind: OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            span: s,
        },
        OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
            kind: OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            span: s,
        },
    ]);
    f.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(1))],
        result: CallResult::Owned(OwnerPlaceId(2)),
        parent: None,
        span: s,
    });
    f.blocks[0].statements.extend([
        instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s),
        instruction(
            OwnedInstruction::PrepareOwned {
                call: CallSiteId(0),
                argument: 0,
                source: OwnerPlaceId(0),
            },
            s,
        ),
    ]);
    f.blocks[0].terminator = end(
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s,
    );
    f.blocks.push(OwnedBlock {
        span: s,
        merge: None,
        statements: vec![
            instruction(OwnedInstruction::Discard(OwnerPlaceId(2)), s),
            instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(2)), s),
        ],
        terminator: end(
            OwnedTerminatorKind::ReturnScalar(Operand {
                local: LocalId(0),
                span: s,
            }),
            s,
        ),
    });
    let duplicate = f.clone();
    assert!(verify_owned(raw, &sources).is_ok());
    // Rebuild with a checked helper and a second use of the returned owner.
    let mut raw = subject(s);
    let mut helper = unit_function(1, s);
    helper.result = ValueTy::Owned(AggregateTy::Record(RecordId(0)));
    helper
        .parameters
        .push(ParameterBinding::Owned(OwnerPlaceId(0)));
    helper.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
        kind: OwnerKind::Parameter { position: 0 },
        span: s,
    });
    helper.blocks[0].terminator = end(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)), s);
    raw.functions = vec![duplicate, helper];
    raw.functions[0].blocks[1].statements.insert(
        1,
        instruction(OwnedInstruction::Discard(OwnerPlaceId(2)), s),
    );
    assert_eq!(
        error(raw, &sources).kind,
        OwnedFailureKind::Ownership(Violation::Unavailable)
    );
}
#[test]
#[ignore = "maximum-block ownership scratch/RSS qualification"]
fn maximum_block_shape_uses_linear_scratch_and_oversize_product_is_preflighted() {
    let (sources, s) = context();
    let mut raw = subject(s);
    let f = &mut raw.functions[0];
    f.blocks[0].statements.pop();
    let empty = OwnedBlock {
        span: s,
        merge: None,
        statements: vec![],
        terminator: end(
            OwnedTerminatorKind::ReturnScalar(Operand {
                local: LocalId(0),
                span: s,
            }),
            s,
        ),
    };
    f.blocks.resize(MAX_BLOCKS, empty);
    for b in 0..MAX_BLOCKS - 1 {
        f.blocks[b].terminator = end(OwnedTerminatorKind::Goto(BlockId(b + 1)), s);
    }
    let expected = budget::preflight(&raw, budget::Limits::DEFAULT).unwrap();
    assert_eq!(expected.scratch_bytes, 33 * MAX_BLOCKS);
    let failure = budget::fail_allocation_after(0, || {
        verify_with_limits(
            RawOwnedProgram {
                records: vec![],
                functions: vec![raw.functions[0].clone()],
            },
            &sources,
            budget::Limits {
                scratch: expected.scratch_bytes - 1,
                ..budget::Limits::DEFAULT
            },
        )
    })
    .unwrap_err();
    assert_eq!(
        failure.kind,
        OwnedFailureKind::Resource("ownership scratch")
    );
    let checked = verify_owned(raw, &sources).unwrap();
    assert_eq!(checked.usage().scratch_bytes, 9_900_000);
    println!("maximum owned graph usage: {:?}", checked.usage());
    drop(checked);
    let mut raw = subject(s);
    let f = &mut raw.functions[0];
    f.owners.resize(
        MAX_LOCALS - 1,
        OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
            kind: OwnerKind::Local { mutable: true },
            span: s,
        },
    );
    f.blocks.resize(
        MAX_BLOCKS,
        OwnedBlock {
            span: s,
            merge: None,
            statements: vec![],
            terminator: end(OwnedTerminatorKind::Goto(BlockId(0)), s),
        },
    );
    let failure = budget::fail_allocation_after(0, || verify_owned(raw, &sources)).unwrap_err();
    assert_eq!(failure.kind, OwnedFailureKind::Resource("ownership work"));
}
#[test]
fn nominal_record_ids_and_ordered_constructor_fields_survive_in_the_witness() {
    let (sources, s) = context();
    let mut raw = subject(s);
    raw.records[0].fields = vec![
        RawFieldDecl {
            id: FieldId {
                record: RecordId(0),
                index: 0,
            },
            ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::Unit)),
            span: s,
        },
        RawFieldDecl {
            id: FieldId {
                record: RecordId(0),
                index: 1,
            },
            ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::Unit)),
            span: s,
        },
    ];
    if let OwnedInstruction::Construct { fields, .. } =
        &mut raw.functions[0].blocks[0].statements[2].kind
    {
        *fields = vec![
            (
                FieldId {
                    record: RecordId(0),
                    index: 1,
                },
                Operand {
                    local: LocalId(0),
                    span: s,
                },
            ),
            (
                FieldId {
                    record: RecordId(0),
                    index: 0,
                },
                Operand {
                    local: LocalId(0),
                    span: s,
                },
            ),
        ];
    }
    let checked = verify_owned(raw, &sources).unwrap();
    let OwnedInstruction::Construct { fields, .. } =
        &checked.functions()[0].blocks[0].statements[2].kind
    else {
        panic!("exact instruction was changed")
    };
    assert_eq!(fields[0].0.index, 1);
    assert_eq!(fields[1].0.index, 0);
    assert_eq!(checked.usage().owner_cells, 2);
    assert_eq!(checked.usage().owner_layout_bytes, 2);
    let mut raw = subject(s);
    raw.records.push(RawRecordDecl {
        id: RecordId(1),
        span: s,
        fields: vec![],
    });
    raw.functions[0].owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(1))).unwrap(),
        kind: OwnerKind::Temporary,
        span: s,
    });
    raw.functions[0].blocks[0].statements.push(instruction(
        OwnedInstruction::MoveInitialize {
            destination: OwnerPlaceId(1),
            source: OwnerPlaceId(0),
        },
        s,
    ));
    assert_eq!(
        error(raw, &sources).kind,
        OwnedFailureKind::Malformed(Malformed::Type)
    );
}
fn reborrows(s: Span, parent: BorrowKind, children: [BorrowKind; 2]) -> RawOwnedProgram {
    let mut raw = borrowed(s, children[0]);
    let f = &mut raw.functions[0];
    f.owners.clear();
    f.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
    f.references = vec![ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        kind: parent,
        position: 0,
        span: s,
    }];
    f.loans[0].authority = AccessBase::Parameter(ReferenceParamId(0));
    f.blocks[0].statements.retain(|i| {
        !matches!(
            i.kind,
            OwnedInstruction::StorageLive(_) | OwnedInstruction::Construct { .. }
        )
    });
    f.blocks[1].statements.clear();
    f.calls[0].arguments.push(ArgumentSlot::Borrow(LoanId(1)));
    f.loans.push(LoanDecl {
        call: CallSiteId(0),
        argument: 1,
        authority: AccessBase::Parameter(ReferenceParamId(0)),
        kind: children[1],
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        span: s,
    });
    f.blocks[0].statements.push(instruction(
        OwnedInstruction::PrepareBorrow {
            call: CallSiteId(0),
            argument: 1,
            loan: LoanId(1),
        },
        s,
    ));
    let target = &mut raw.functions[1];
    target
        .parameters
        .push(ParameterBinding::Reference(ReferenceParamId(1)));
    target.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        kind: children[1],
        position: 1,
        span: s,
    });
    raw
}
#[test]
fn shared_children_preserve_reads_without_upgrading_parent_authority() {
    let (sources, s) = context();
    use BorrowKind::*;
    for parent in [Shared, Exclusive] {
        for children in [
            [Shared, Shared],
            [Shared, Exclusive],
            [Exclusive, Shared],
            [Exclusive, Exclusive],
        ] {
            let result = verify_owned(reborrows(s, parent, children), &sources);
            if children == [Shared, Shared] {
                assert!(result.is_ok());
            } else {
                assert_eq!(
                    result.unwrap_err().kind,
                    OwnedFailureKind::Ownership(if parent == Shared {
                        Violation::Permission
                    } else {
                        Violation::LoanConflict
                    })
                );
            }
        }
    }
}
#[test]
fn scalar_dominance_uses_real_owned_instruction_positions_and_return_edges() {
    let (sources, s) = context();
    let mut raw = subject(s);
    raw.records[0].fields.push(RawFieldDecl {
        id: FieldId {
            record: RecordId(0),
            index: 0,
        },
        ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::Unit)),
        span: s,
    });
    if let OwnedInstruction::Construct { fields, .. } =
        &mut raw.functions[0].blocks[0].statements[2].kind
    {
        fields.push((
            FieldId {
                record: RecordId(0),
                index: 0,
            },
            Operand {
                local: LocalId(0),
                span: s,
            },
        ));
    }
    let assign = raw.functions[0].blocks[0].statements.remove(0);
    raw.functions[0].blocks[0].statements.insert(2, assign);
    assert_eq!(
        error(raw, &sources).kind,
        OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Uninitialized))
    );
    let mut raw = borrowed(s, BorrowKind::Shared);
    let f = &mut raw.functions[0];
    f.locals.push(LocalDecl {
        ty: hir::Ty::Unit,
        kind: LocalKind::Temporary,
        span: s,
    });
    f.blocks[0].statements.push(instruction(
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            destination: LocalId(2),
            value: Rvalue::Copy(Operand {
                local: LocalId(1),
                span: s,
            }),
            span: s,
        })),
        s,
    ));
    assert_eq!(
        error(raw, &sources).kind,
        OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Uninitialized))
    );
}

#[path = "reviewer_heldout.rs"]
mod reviewer_heldout;

#[test]
fn production_array_carriers_require_consistent_signatures_and_loans() {
    let (sources, s) = context();
    let mut rejected = 0;
    let mut admitted = 0;
    for element in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for length in [0, 1, 1024] {
            let aggregate = AggregateTy::FixedArray(FixedArrayTy::check(element, length).unwrap());
            let slot = AggregateSlot::try_from_aggregate(aggregate).unwrap();
            for carrier in 0..6 {
                // A result-only, ownership-inactive function has no array use.
                // An infinite loop is valid for either result type.
                let mut function = unit_function(0, s);
                function.blocks[0].statements.clear();
                function.blocks[0].terminator = end(OwnedTerminatorKind::Goto(BlockId(0)), s);
                let raw = |function| RawOwnedProgram {
                    records: vec![],
                    functions: vec![function],
                };
                verify_owned(raw(function.clone()), &sources).unwrap();
                match carrier {
                    0 => function.result = ValueTy::Owned(aggregate),
                    1 | 2 => {
                        function.owners.push(OwnerDecl {
                            aggregate: slot,
                            kind: if carrier == 1 {
                                OwnerKind::Parameter { position: 0 }
                            } else {
                                OwnerKind::Local { mutable: false }
                            },
                            span: s,
                        });
                        if carrier == 1 {
                            function
                                .parameters
                                .push(ParameterBinding::Owned(OwnerPlaceId(0)));
                        } else {
                            // The unused local has no constructor or projection.
                            function.blocks[0].statements.extend([
                                instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s),
                                instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s),
                            ]);
                        }
                    }
                    3 | 4 => {
                        function.references.push(ReferenceDecl {
                            referent: BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate()))
                                .unwrap(),
                            kind: BorrowKind::Shared,
                            position: 0,
                            span: s,
                        });
                        if carrier == 3 {
                            function
                                .parameters
                                .push(ParameterBinding::Reference(ReferenceParamId(0)));
                        }
                    }
                    5 => function.loans.push(LoanDecl {
                        referent: BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap(),
                        kind: BorrowKind::Shared,
                        call: CallSiteId(0),
                        argument: 0,
                        authority: AccessBase::Owner(OwnerPlaceId(0)),
                        span: s,
                    }),
                    _ => unreachable!(),
                }
                let result = verify_owned(raw(function), &sources);
                if carrier <= 3 {
                    let witness = result.unwrap();
                    plan::ExecutionPlan::build(&witness).unwrap();
                    admitted += 1;
                } else {
                    // Unbound references and loans without their call/owner
                    // remain malformed even though their array identity is valid.
                    let error = result.unwrap_err();
                    assert_eq!(
                        error.kind,
                        OwnedFailureKind::Malformed(if carrier == 4 {
                            Malformed::Binding
                        } else {
                            Malformed::Id
                        })
                    );
                    assert_eq!(error.primary.get(), Some(s));
                    rejected += 1;
                }
            }
        }
    }
    assert_eq!(admitted, 36);
    assert_eq!(rejected, 18);
}

#[test]
fn array_preflight_and_declarations_still_precede_shape_checks() {
    let (sources, s) = context();
    let aggregate = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 0).unwrap());
    let mut raw = subject(s);
    raw.functions[0].owners[0].aggregate = AggregateSlot::try_from_aggregate(aggregate).unwrap();
    assert_eq!(
        verify_with_limits(
            raw,
            &sources,
            budget::Limits {
                work: 0,
                ..budget::Limits::DEFAULT
            }
        )
        .unwrap_err()
        .kind,
        OwnedFailureKind::Resource("ownership work")
    );
    let mut raw = subject(s);
    raw.functions[0].result = ValueTy::Owned(aggregate);
    raw.records[0].id = RecordId(4096);
    let failure = error(raw, &sources);
    assert_eq!(
        failure.kind,
        OwnedFailureKind::Malformed(Malformed::Declaration(DeclarationError::InvalidRecordId(
            RecordId(4096)
        )))
    );
    assert_eq!(failure.primary.get(), None);
    // References remain forbidden stored fields after composition activation.
    for ty in [
        ParameterTy::Reference {
            referent: BorrowedTy::Exact(aggregate),
            kind: BorrowKind::Shared,
        },
        ParameterTy::Reference {
            referent: BorrowedTy::Exact(aggregate),
            kind: BorrowKind::Exclusive,
        },
    ] {
        let mut raw = subject(s);
        let id = FieldId {
            record: RecordId(0),
            index: 0,
        };
        raw.records[0].fields.push(RawFieldDecl { id, ty, span: s });
        assert_eq!(
            error(raw, &sources).kind,
            OwnedFailureKind::Malformed(Malformed::Declaration(DeclarationError::NonScalarField(
                id
            )))
        );
    }
}

#[test]
fn unit2a_malformed_record_sites_preserve_categories_and_origins() {
    let (sources, s) = context();
    let invalid = AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(4096))).unwrap();
    let at = |n| Span {
        start: n,
        end: n + 1,
        ..s
    };
    // Call descriptor errors precede the later loan span/authority walk.
    for defect in 0..6 {
        let mut raw = borrowed(s, BorrowKind::Shared);
        let f = &mut raw.functions[0];
        f.calls[0].span = at(10);
        f.loans[0].referent = BorrowedSlot::check(BorrowedTy::Exact(invalid.aggregate())).unwrap();
        match defect {
            0 => (),
            1 => f.loans[0].call = CallSiteId(4096),
            2 => f.loans[0].argument = 4096,
            3 => f.loans[0].kind = BorrowKind::Exclusive,
            4 => f.loans[0].span = Span { end: 201, ..s },
            5 => f.loans[0].authority = AccessBase::Owner(OwnerPlaceId(4096)),
            _ => unreachable!(),
        }
        let failure = error(raw, &sources);
        assert_eq!(
            failure.kind,
            OwnedFailureKind::Malformed(Malformed::Binding)
        );
        assert_eq!(failure.primary.get(), Some(at(10)));
    }
    for carrier in 0..3 {
        let mut raw = borrowed(s, BorrowKind::Shared);
        match carrier {
            0 => {
                raw.functions[0].result = ValueTy::Owned(AggregateTy::Record(RecordId(usize::MAX)))
            }
            1 => raw.functions[0].owners[0].aggregate = invalid,
            2 => {
                raw.functions[1].references[0].referent =
                    BorrowedSlot::check(BorrowedTy::Exact(invalid.aggregate())).unwrap()
            }
            _ => unreachable!(),
        }
        let failure = error(raw, &sources);
        let id = if carrier == 0 { usize::MAX } else { 4096 };
        assert_eq!(
            failure.kind,
            OwnedFailureKind::Malformed(Malformed::Declaration(DeclarationError::InvalidRecordId(
                RecordId(id)
            )))
        );
        assert_eq!(failure.primary.get(), None);
    }
}

#[path = "array_tests.rs"]
mod arrays;

#[test]
fn unit2b_promoted_record_loan_defects_preserve_all_short_circuits() {
    let (sources, s) = context();
    let at = |n| Span {
        start: n,
        end: n + 1,
        ..s
    };
    for kind in [BorrowKind::Shared, BorrowKind::Exclusive] {
        for defects in 0usize..64 {
            let mut raw = borrowed(s, kind);
            let f = &mut raw.functions[0];
            f.calls[0].span = at(10);
            f.loans[0].span = at(20);
            for b in &mut f.blocks {
                for i in &mut b.statements {
                    if matches!(i.kind, OwnedInstruction::PrepareBorrow { .. }) {
                        i.span = at(20);
                    }
                }
            }
            if defects & 1 != 0 {
                f.loans[0].referent =
                    BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(4096))))
                        .unwrap();
            }
            if defects & 2 != 0 {
                f.loans[0].call = CallSiteId(4096);
            }
            if defects & 4 != 0 {
                f.loans[0].argument = 4096;
            }
            if defects & 8 != 0 {
                f.loans[0].kind = if kind == BorrowKind::Shared {
                    BorrowKind::Exclusive
                } else {
                    BorrowKind::Shared
                };
            }
            if defects & 16 != 0 {
                f.loans[0].span = Span { end: 201, ..s };
            }
            if defects & 32 != 0 {
                f.loans[0].authority = AccessBase::Owner(OwnerPlaceId(4096));
            }
            if defects == 0 {
                assert!(verify_owned(raw, &sources).is_ok());
                continue;
            }
            let e = error(raw, &sources);
            let (expected, origin) = if defects & 15 != 0 {
                (Malformed::Binding, at(10))
            } else if defects & 16 != 0 {
                (Malformed::Span, Span { end: 201, ..s })
            } else {
                (Malformed::Id, at(20))
            };
            assert_eq!(
                e.kind,
                OwnedFailureKind::Malformed(expected),
                "mode {kind:?}, defects {defects}"
            );
            assert_eq!(e.primary.get(), Some(origin));
        }
    }
}
