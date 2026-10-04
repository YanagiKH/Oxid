//! Facts must describe the verifier's exact denied operand, without source tags.
use super::flow::{
    DeniedOperation as Op, DeniedRole as Role, DeniedSubject, ObservedState as State,
};
use super::origin_tests::*;
use super::*;

fn facts(p: RawOwnedProgram, sources: &SourceMap, violation: Violation) -> flow::DenialFacts {
    let e = fail(p, sources);
    assert_eq!(e.kind, OwnedFailureKind::Ownership(violation));
    e.context
        .expect("specific denied access retains context")
        .facts()
}
fn assert_owner(facts: flow::DenialFacts, id: OwnerPlaceId, class: OwnerKind, s: Span) {
    let DeniedSubject::Owner(owner) = facts.subject else {
        panic!("denial must identify the owner");
    };
    assert_eq!(owner.id, id);
    assert_eq!(owner.aggregate(), AggregateTy::Record(RecordId(0)));
    assert_eq!(owner.class, class);
    assert_eq!(owner.declaration, s);
}

#[test]
fn moved_name_source_retains_exact_owner_and_temporary_counterpart() {
    let (sources, s) = context();
    for kind in [
        OwnerKind::Local { mutable: false },
        OwnerKind::Parameter { position: 0 },
    ] {
        let mut p = raw(s);
        let f = &mut p.functions[0];
        let source = owner(f, kind, at(s, 2));
        if !matches!(kind, OwnerKind::Parameter { .. }) {
            construct(f, source, at(s, 3));
        }
        let destination = owner(f, OwnerKind::Temporary, at(s, 4));
        f.blocks[0].statements.extend([
            ins(OwnedInstruction::Discard(source), at(s, 10)),
            ins(
                OwnedInstruction::MoveInitialize {
                    destination,
                    source,
                },
                at(s, 20),
            ),
        ]);
        let facts = facts(p, &sources, Violation::Unavailable);
        assert_eq!(
            (facts.operation, facts.role, facts.state),
            (Op::MoveInitialize, Role::SourceConsume, State::Moved)
        );
        assert_owner(facts, source, kind, at(s, 2));
        assert_eq!(facts.counterpart.unwrap().class, OwnerKind::Temporary);
        assert_eq!(facts.counterpart.unwrap().id, destination);
        assert_eq!(facts.requested_borrow, None);
    }
}

#[test]
fn consume_observed_states_and_internal_classes_survive_denial() {
    let (sources, s) = context();
    for class in [OwnerKind::Local { mutable: true }, OwnerKind::Temporary] {
        for state in [State::Dead, State::Uninitialized, State::Moved] {
            let mut p = raw(s);
            let f = &mut p.functions[0];
            let source = owner(f, class, at(s, 2));
            if state == State::Moved {
                construct(f, source, at(s, 3));
                f.blocks[0]
                    .statements
                    .push(ins(OwnedInstruction::Discard(source), at(s, 4)));
            }
            if state == State::Dead {
                f.blocks[0]
                    .statements
                    .push(ins(OwnedInstruction::StorageEnd(source), at(s, 4)));
            }
            f.blocks[0]
                .statements
                .push(ins(OwnedInstruction::Discard(source), at(s, 5)));
            let facts = facts(p, &sources, Violation::Unavailable);
            assert_eq!(
                (facts.operation, facts.role, facts.state),
                (Op::Discard, Role::SourceConsume, state)
            );
            assert_owner(facts, source, class, at(s, 2));
        }
    }
}

#[test]
fn move_destination_failure_is_not_reclassified_as_source_access() {
    let (sources, s) = context();
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let source = owner(f, OwnerKind::Parameter { position: 0 }, at(s, 1));
    let destination = owner(f, OwnerKind::Temporary, at(s, 2));
    let live = f.blocks[0].statements.pop().unwrap();
    f.blocks[0].statements.extend([
        ins(
            OwnedInstruction::MoveInitialize {
                destination,
                source,
            },
            at(s, 3),
        ),
        live,
    ]);
    let facts = facts(p, &sources, Violation::Initialization);
    assert_eq!(
        (facts.operation, facts.role, facts.state),
        (
            Op::MoveInitialize,
            Role::InitializationDestination,
            State::Dead
        )
    );
    assert_owner(facts, destination, OwnerKind::Temporary, at(s, 2));
    assert_eq!(facts.counterpart.unwrap().id, source);
}

pub(super) fn borrowed(s: Span, kind: BorrowKind) -> RawOwnedProgram {
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let source = owner(f, OwnerKind::Local { mutable: true }, at(s, 1));
    construct(f, source, at(s, 2));
    f.locals.push(LocalDecl {
        ty: hir::Ty::Unit,
        kind: LocalKind::Temporary,
        span: s,
    });
    f.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
        result: CallResult::Scalar(LocalId(1)),
        parent: None,
        span: at(s, 5),
    });
    f.loans.push(LoanDecl {
        call: CallSiteId(0),
        argument: 0,
        authority: AccessBase::Owner(source),
        kind,
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        span: at(s, 7),
    });
    f.blocks[0].statements.extend([
        ins(OwnedInstruction::OpenCall(CallSiteId(0)), at(s, 5)),
        ins(
            OwnedInstruction::PrepareBorrow {
                call: CallSiteId(0),
                argument: 0,
                loan: LoanId(0),
            },
            at(s, 7),
        ),
    ]);
    f.blocks[0].terminator = term(
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        at(s, 9),
    );
    f.blocks.push(OwnedBlock {
        span: s,
        merge: None,
        statements: vec![],
        terminator: term(OwnedTerminatorKind::ReturnScalar(operand(s)), s),
    });
    let mut callee = function(1, s);
    callee
        .parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    callee.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        kind,
        position: 0,
        span: at(s, 30),
    });
    p.functions.push(callee);
    p
}

#[test]
fn loan_conflicts_keep_source_destination_cleanup_roles_and_acquisition_cause() {
    let (sources, s) = context();
    for role in [
        Role::SourceConsume,
        Role::ReplacementDestination,
        Role::Storage,
    ] {
        let mut p = borrowed(s, BorrowKind::Shared);
        let f = &mut p.functions[0];
        let t = owner(f, OwnerKind::Temporary, at(s, 12));
        let kind = match role {
            Role::SourceConsume => OwnedInstruction::MoveInitialize {
                source: OwnerPlaceId(0),
                destination: t,
            },
            Role::ReplacementDestination => {
                construct(f, t, at(s, 13));
                OwnedInstruction::Replace {
                    source: t,
                    destination: OwnerPlaceId(0),
                }
            }
            Role::Storage => OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
            _ => unreachable!(),
        };
        f.blocks[0].statements[4].diagnostic_origins = Some(DiagnosticOrigins {
            primary: at(s, 70),
            cause: at(s, 71),
        });
        let mut denied = ins(kind, at(s, 20));
        denied.diagnostic_origins = Some(DiagnosticOrigins {
            primary: at(s, 90),
            cause: at(s, 91),
        });
        f.blocks[0].statements.push(denied);
        let e = fail(p, &sources);
        assert_eq!(e.kind, OwnedFailureKind::Ownership(Violation::LoanConflict));
        assert_eq!(e.primary.get(), Some(at(s, 90)));
        assert_eq!(e.related.get(), Some(at(s, 71)));
        let facts = e.context.unwrap().facts();
        assert_eq!((facts.role, facts.state), (role, State::NotObserved));
        assert_owner(
            facts,
            OwnerPlaceId(0),
            OwnerKind::Local { mutable: true },
            at(s, 1),
        );
        assert_eq!(
            facts.operation,
            match role {
                Role::SourceConsume => Op::MoveInitialize,
                Role::ReplacementDestination => Op::Replace,
                _ => Op::StorageEnd,
            }
        );
    }
}

fn add_unit_field(p: &mut RawOwnedProgram, s: Span) -> FieldId {
    let field = FieldId {
        record: RecordId(0),
        index: 0,
    };
    p.records[0].fields.push(RawFieldDecl {
        id: field,
        ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::Unit)),
        span: s,
    });
    for f in &mut p.functions {
        for b in &mut f.blocks {
            for i in &mut b.statements {
                if let OwnedInstruction::Construct { fields, .. } = &mut i.kind {
                    fields.push((field, operand(s)));
                }
            }
        }
    }
    field
}

#[test]
fn shared_reference_write_denial_preserves_granted_mode_without_invented_state() {
    let (sources, s) = context();
    let mut p = raw(s);
    let field = add_unit_field(&mut p, s);
    let f = &mut p.functions[0];
    f.parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    f.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        kind: BorrowKind::Shared,
        position: 0,
        span: at(s, 1),
    });
    f.blocks[0].statements.push(ins(
        OwnedInstruction::WriteField {
            base: AccessBase::Parameter(ReferenceParamId(0)),
            field,
            value: operand(s),
        },
        at(s, 4),
    ));
    let e = fail(p, &sources);
    assert_eq!(e.kind, OwnedFailureKind::Ownership(Violation::Permission));
    assert_eq!(
        e.declaration.get(),
        None,
        "preserve the old raw Permission declaration field"
    );
    let facts = e.context.unwrap().facts();
    assert_eq!(
        (facts.operation, facts.role, facts.state),
        (Op::WriteField, Role::FieldBase, State::NotObserved)
    );
    assert_eq!(
        facts.subject,
        DeniedSubject::Reference {
            id: ReferenceParamId(0),
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            granted: BorrowKind::Shared,
            declaration: at(s, 1)
        }
    );
}

#[test]
fn exclusive_reborrow_permission_retains_requested_and_granted_modes() {
    let (sources, s) = context();
    let mut p = borrowed(s, BorrowKind::Exclusive);
    let f = &mut p.functions[0];
    f.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        kind: BorrowKind::Shared,
        position: 0,
        span: at(s, 1),
    });
    f.parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    f.loans[0].authority = AccessBase::Parameter(ReferenceParamId(0));
    let facts = facts(p, &sources, Violation::Permission);
    assert_eq!(
        (facts.operation, facts.role, facts.state),
        (Op::PrepareBorrow, Role::BorrowAuthority, State::NotObserved)
    );
    assert_eq!(facts.requested_borrow, Some(BorrowKind::Exclusive));
    assert!(matches!(
        facts.subject,
        DeniedSubject::Reference {
            granted: BorrowKind::Shared,
            ..
        }
    ));
}

#[test]
fn field_access_denials_keep_owner_base_and_requested_access() {
    let (sources, s) = context();
    for write in [false, true] {
        let mut p = borrowed(s, BorrowKind::Exclusive);
        let field = add_unit_field(&mut p, s);
        let f = &mut p.functions[0];
        let kind = if write {
            OwnedInstruction::WriteField {
                base: AccessBase::Owner(OwnerPlaceId(0)),
                field,
                value: operand(s),
            }
        } else {
            f.locals.push(LocalDecl {
                ty: hir::Ty::Unit,
                kind: LocalKind::Temporary,
                span: s,
            });
            OwnedInstruction::ReadField {
                destination: LocalId(2),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                field,
            }
        };
        f.blocks[0].statements.push(ins(kind, at(s, 20)));
        let facts = facts(p, &sources, Violation::LoanConflict);
        assert_eq!(
            (facts.operation, facts.role, facts.state),
            (
                if write { Op::WriteField } else { Op::ReadField },
                Role::FieldBase,
                State::NotObserved
            )
        );
    }
}

#[test]
fn unavailable_return_value_is_internal_even_for_a_named_owner() {
    let (sources, s) = context();
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let o = owner(f, OwnerKind::Parameter { position: 0 }, at(s, 1));
    f.result = ValueTy::Owned(AggregateTy::Record(RecordId(0)));
    f.blocks[0]
        .statements
        .push(ins(OwnedInstruction::StorageEnd(o), at(s, 2)));
    f.blocks[0].terminator = term(OwnedTerminatorKind::ReturnOwned(o), at(s, 3));
    let facts = facts(p, &sources, Violation::Unavailable);
    assert_eq!(
        (facts.operation, facts.role, facts.state),
        (Op::ReturnOwned, Role::ReturnValue, State::Dead)
    );
}

#[test]
fn initialization_storage_and_replacement_denials_retain_their_exact_roles() {
    let (sources, s) = context();
    // A canonical initializer appearing before the canonical live site observes Dead.
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let o = owner(f, OwnerKind::Temporary, at(s, 1));
    let live = f.blocks[0].statements.pop().unwrap();
    construct(f, o, at(s, 2));
    f.blocks[0].statements.push(live);
    let result = facts(p, &sources, Violation::Initialization);
    assert_eq!(
        (result.operation, result.role, result.state),
        (Op::Construct, Role::InitializationDestination, State::Dead)
    );
    // StorageEnd in a second generation never acquires a user-facing consume role.
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let o = owner(f, OwnerKind::Local { mutable: true }, at(s, 1));
    f.blocks[0].statements.extend([
        ins(OwnedInstruction::StorageEnd(o), at(s, 2)),
        ins(OwnedInstruction::StorageEnd(o), at(s, 3)),
    ]);
    let result = facts(p, &sources, Violation::Lifetime);
    assert_eq!(
        (result.operation, result.role, result.state),
        (Op::StorageEnd, Role::Storage, State::Dead)
    );
    // Replace source and destination must remain distinguishable on one operation.
    for source_fails in [false, true] {
        let mut p = raw(s);
        let f = &mut p.functions[0];
        let destination = owner(f, OwnerKind::Local { mutable: true }, at(s, 1));
        if source_fails {
            construct(f, destination, at(s, 2));
        }
        let source = owner(f, OwnerKind::Temporary, at(s, 3));
        construct(f, source, at(s, 4));
        if source_fails {
            f.blocks[0]
                .statements
                .push(ins(OwnedInstruction::Discard(source), at(s, 5)));
        }
        f.blocks[0].statements.push(ins(
            OwnedInstruction::Replace {
                destination,
                source,
            },
            at(s, 6),
        ));
        let result = facts(
            p,
            &sources,
            if source_fails {
                Violation::Unavailable
            } else {
                Violation::Initialization
            },
        );
        assert_eq!(
            (result.operation, result.role, result.state),
            (
                Op::Replace,
                if source_fails {
                    Role::SourceConsume
                } else {
                    Role::ReplacementDestination
                },
                if source_fails {
                    State::Moved
                } else {
                    State::Uninitialized
                }
            )
        );
        assert_eq!(
            result.counterpart.unwrap().id,
            if source_fails { destination } else { source }
        );
    }
}

#[test]
fn owned_argument_and_invoke_denials_keep_internal_staging_roles() {
    let (sources, s) = context();
    for denied_source in [false, true] {
        let mut p = raw(s);
        let f = &mut p.functions[0];
        let source = owner(f, OwnerKind::Temporary, at(s, 1));
        construct(f, source, at(s, 2));
        let staged = owner(
            f,
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            at(s, 3),
        );
        f.locals.push(LocalDecl {
            ty: hir::Ty::Unit,
            kind: LocalKind::Temporary,
            span: s,
        });
        f.calls.push(CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Owned(staged)],
            result: CallResult::Scalar(LocalId(1)),
            parent: None,
            span: at(s, 4),
        });
        f.blocks[0]
            .statements
            .push(ins(OwnedInstruction::OpenCall(CallSiteId(0)), at(s, 4)));
        if denied_source {
            f.blocks[0]
                .statements
                .push(ins(OwnedInstruction::Discard(source), at(s, 5)));
        }
        let prepare = ins(
            OwnedInstruction::PrepareOwned {
                call: CallSiteId(0),
                argument: 0,
                source,
            },
            at(s, 6),
        );
        if denied_source {
            f.blocks[0].statements.push(prepare.clone());
        }
        f.blocks[0].terminator = term(
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            at(s, 7),
        );
        // The missing preparation is canonically present after Invoke; availability
        // must reject the observed staged Uninitialized value before call regions.
        f.blocks.push(OwnedBlock {
            span: s,
            merge: None,
            statements: if denied_source { vec![] } else { vec![prepare] },
            terminator: term(OwnedTerminatorKind::ReturnScalar(operand(s)), s),
        });
        let mut callee = function(1, s);
        owner(&mut callee, OwnerKind::Parameter { position: 0 }, at(s, 8));
        p.functions.push(callee);
        let result = facts(p, &sources, Violation::Unavailable);
        assert_eq!(
            (result.operation, result.role, result.state),
            if denied_source {
                (Op::PrepareOwned, Role::SourceConsume, State::Moved)
            } else {
                (Op::Invoke, Role::StagedInput, State::Uninitialized)
            }
        );
    }
}

#[test]
fn raw_mutability_is_authority_and_is_not_reconstructed_from_source_origins() {
    let (sources, s) = context();
    let build = |mutable| {
        let mut p = raw(s);
        let f = &mut p.functions[0];
        let destination = owner(f, OwnerKind::Local { mutable }, at(s, 1));
        construct(f, destination, at(s, 2));
        let source = owner(f, OwnerKind::Temporary, at(s, 3));
        construct(f, source, at(s, 4));
        let mut replace = ins(
            OwnedInstruction::Replace {
                destination,
                source,
            },
            at(s, 5),
        );
        replace.diagnostic_origins = Some(DiagnosticOrigins {
            primary: at(s, 40),
            cause: at(s, 50),
        });
        f.blocks[0].statements.push(replace);
        p
    };
    let result = facts(build(false), &sources, Violation::Permission);
    assert_eq!(
        (result.operation, result.role, result.state),
        (
            Op::Replace,
            Role::ReplacementDestination,
            State::NotObserved
        )
    );
    assert!(
        verify_owned(build(true), &sources).is_ok(),
        "a coherent producer mutability lie requires independent source correspondence tests"
    );
}

#[test]
fn moved_field_bases_and_borrow_authorities_keep_observed_state() {
    let (sources, s) = context();
    for operation in [Op::ReadField, Op::WriteField, Op::PrepareBorrow] {
        let mut p = borrowed(s, BorrowKind::Shared);
        let field = add_unit_field(&mut p, s);
        let f = &mut p.functions[0];
        // Insert the move immediately before the denied access; all call sites
        // remain canonical, so this exercises flow rather than shape rejection.
        f.blocks[0]
            .statements
            .insert(4, ins(OwnedInstruction::Discard(OwnerPlaceId(0)), at(s, 6)));
        if operation != Op::PrepareBorrow {
            let kind = if operation == Op::WriteField {
                OwnedInstruction::WriteField {
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    field,
                    value: operand(s),
                }
            } else {
                f.locals.push(LocalDecl {
                    ty: hir::Ty::Unit,
                    kind: LocalKind::Temporary,
                    span: s,
                });
                OwnedInstruction::ReadField {
                    destination: LocalId(2),
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    field,
                }
            };
            f.blocks[0].statements.insert(5, ins(kind, at(s, 8)));
        }
        let result = facts(p, &sources, Violation::Unavailable);
        assert_eq!(
            (result.operation, result.role, result.state),
            (
                operation,
                if operation == Op::PrepareBorrow {
                    Role::BorrowAuthority
                } else {
                    Role::FieldBase
                },
                State::Moved
            )
        );
        assert_eq!(
            result.requested_borrow,
            (operation == Op::PrepareBorrow).then_some(BorrowKind::Shared)
        );
    }
}

#[test]
fn conflicting_borrow_captures_requested_mode_for_owners_and_references() {
    let (sources, s) = context();
    for through_reference in [false, true] {
        let mut p = borrowed(s, BorrowKind::Shared);
        let f = &mut p.functions[0];
        let authority = if through_reference {
            f.references.push(ReferenceDecl {
                referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                    .unwrap(),
                kind: BorrowKind::Exclusive,
                position: 0,
                span: at(s, 1),
            });
            f.parameters
                .push(ParameterBinding::Reference(ReferenceParamId(0)));
            AccessBase::Parameter(ReferenceParamId(0))
        } else {
            AccessBase::Owner(OwnerPlaceId(0))
        };
        f.loans[0].authority = authority;
        f.calls[0].arguments.push(ArgumentSlot::Borrow(LoanId(1)));
        f.loans.push(LoanDecl {
            call: CallSiteId(0),
            argument: 1,
            authority,
            kind: BorrowKind::Exclusive,
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            span: at(s, 8),
        });
        f.blocks[0].statements.push(ins(
            OwnedInstruction::PrepareBorrow {
                call: CallSiteId(0),
                argument: 1,
                loan: LoanId(1),
            },
            at(s, 8),
        ));
        let callee = &mut p.functions[1];
        callee
            .parameters
            .push(ParameterBinding::Reference(ReferenceParamId(1)));
        callee.references.push(ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            kind: BorrowKind::Exclusive,
            position: 1,
            span: at(s, 31),
        });
        let result = facts(p, &sources, Violation::LoanConflict);
        assert_eq!(
            (
                result.operation,
                result.role,
                result.state,
                result.requested_borrow
            ),
            (
                Op::PrepareBorrow,
                Role::BorrowAuthority,
                State::NotObserved,
                Some(BorrowKind::Exclusive)
            )
        );
        assert_eq!(
            matches!(
                result.subject,
                DeniedSubject::Reference {
                    granted: BorrowKind::Exclusive,
                    ..
                }
            ),
            through_reference
        );
    }
}

#[test]
fn call_result_lifetime_denial_captures_result_class_and_available_state() {
    let (sources, s) = context();
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let result = owner(
        f,
        OwnerKind::CallResult {
            call: CallSiteId(0),
        },
        at(s, 1),
    );
    f.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![],
        result: CallResult::Owned(result),
        parent: None,
        span: at(s, 2),
    });
    f.blocks[0]
        .statements
        .push(ins(OwnedInstruction::OpenCall(CallSiteId(0)), at(s, 2)));
    f.blocks[0].terminator = term(
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(0),
        },
        at(s, 3),
    );
    let mut callee = function(1, s);
    let value = owner(&mut callee, OwnerKind::Temporary, at(s, 4));
    construct(&mut callee, value, at(s, 5));
    callee.result = ValueTy::Owned(AggregateTy::Record(RecordId(0)));
    callee.blocks[0].terminator = term(OwnedTerminatorKind::ReturnOwned(value), at(s, 6));
    p.functions.push(callee);
    let result = facts(p, &sources, Violation::Lifetime);
    assert_eq!(
        (result.operation, result.role, result.state),
        (Op::Invoke, Role::CallResult, State::Available)
    );
    assert!(matches!(
        result.subject,
        DeniedSubject::Owner(flow::OwnerSubject {
            class: OwnerKind::CallResult {
                call: CallSiteId(0)
            },
            ..
        })
    ));
}

#[test]
fn immutable_fact_copies_cannot_rebind_the_original_context() {
    let (sources, s) = context();
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let o = owner(f, OwnerKind::Temporary, at(s, 1));
    f.blocks[0]
        .statements
        .push(ins(OwnedInstruction::Discard(o), at(s, 2)));
    let error = fail(p, &sources);
    let context = error.context.unwrap();
    let mut facts = context.facts();
    facts.role = Role::FieldBase;
    assert_eq!(facts.role, Role::FieldBase);
    assert_eq!(context.facts().role, Role::SourceConsume);
    let mut malformed = raw(s);
    malformed.functions[0].blocks[0]
        .statements
        .push(ins(OwnedInstruction::Discard(OwnerPlaceId(99)), s));
    assert!(fail(malformed, &sources).context.is_none());
}
