use super::super::context::{signature_queries::ScopedSignatureQueries, ProgramDomainQueries};
use super::*;

#[path = "owned_vector_witness_tests.rs"]
mod witness_tests;

fn insert_user(raw: &mut RawOwnedProgram, user: RawOwnedFunction) {
    for function in &mut raw.functions {
        function.id.0 += 1;
    }
    raw.functions.insert(0, user);
}

fn identity(span: Span) -> RawOwnedFunction {
    let aggregate = AggregateTy::OwnedRecordVector(RecordId(0));
    let mut user = consumer_fixtures::function(0, ValueTy::Owned(aggregate), span);
    user.parameters
        .push(ParameterBinding::Owned(OwnerPlaceId(0)));
    user.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
        kind: OwnerKind::Parameter { position: 0 },
        span,
    });
    user.blocks.push(OwnedBlock {
        merge: None,
        span,
        statements: vec![],
        terminator: consumer_fixtures::end(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)), span),
    });
    user
}

#[test]
fn owned_vector_proof_one_ordinary_authority_checks_all_eight_functions_and_type_only_body() {
    let (sources, mut raw, keys, imports) = fixture(None);
    let span = raw.records[0].span;
    insert_user(&mut raw, identity(span));
    let wrapper = prepare(&sources, raw, &keys, imports);
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    let before = remaining.remaining();
    let usage = context
        .probe_ordinary::<false>(&sources, &mut remaining)
        .unwrap();
    assert!(usage.owner_cells > 3);
    assert!(usage.owner_layout_bytes >= 16);
    assert!(remaining.remaining().0 < before.0);
    assert!(remaining.remaining().1 < before.1);
    assert_eq!(
        usage.metadata_bytes,
        budget::Limits::DEFAULT.metadata - remaining.remaining().0
    );
    assert_eq!(
        usage.work,
        budget::Limits::DEFAULT.work - remaining.remaining().1
    );
    for (rank, key) in keys.into_iter().enumerate() {
        assert_eq!(
            context.owned_vector_operation(hir::DefId(rank + 2)),
            Some(key)
        );
    }
    assert!(wrapper.program.functions[0].blocks[0].statements.is_empty());
    drop(context);
    assert!(verify_owned(wrapper.program, &sources).is_err());
}

#[test]
fn owned_vector_proof_unreachable_bad_instruction_precedes_cfg_or_flow_acceptance() {
    let (sources, mut raw, keys, imports) = fixture(None);
    let span = raw.records[0].span;
    let bad_span = Span {
        start: 4,
        end: 5,
        ..span
    };
    let mut user = identity(span);
    user.blocks.push(OwnedBlock {
        merge: None,
        span,
        statements: vec![consumer_fixtures::assign(
            usize::MAX,
            Rvalue::I32(1),
            bad_span,
        )],
        terminator: consumer_fixtures::end(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)), span),
    });
    insert_user(&mut raw, user);
    let wrapper = prepare(&sources, raw, &keys, imports);
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    let result = context.probe_ordinary::<false>(&sources, &mut remaining);
    assert!(
        matches!(result, Err(OwnedFailure { kind: OwnedFailureKind::Malformed(Malformed::Scalar(_)), primary, .. }) if primary.get() == Some(bad_span))
    );
}

#[test]
fn owned_vector_proof_uses_the_actual_cfg_before_ownership_flow() {
    let (sources, mut raw, keys, imports) = fixture(None);
    let span = raw.records[0].span;
    let mut user = identity(span);
    user.locals
        .push(consumer_fixtures::scalar(hir::Ty::I32, span));
    user.blocks[0].statements.push(consumer_fixtures::assign(
        0,
        Rvalue::Copy(consumer_fixtures::operand(0, span)),
        span,
    ));
    insert_user(&mut raw, user);
    let wrapper = prepare(&sources, raw, &keys, imports);
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    let result = context.probe_ordinary::<false>(&sources, &mut remaining);
    assert!(matches!(
        result,
        Err(OwnedFailure {
            kind: OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Uninitialized)),
            ..
        })
    ));
}

#[test]
fn owned_vector_proof_real_move_then_use_rejects_the_new_descriptor_owner() {
    let (sources, mut raw, keys, imports) = fixture(None);
    let span = raw.records[0].span;
    let mut user = identity(span);
    user.owners.push(OwnerDecl {
        aggregate: user.owners[0].aggregate,
        kind: OwnerKind::Temporary,
        span,
    });
    user.blocks[0].statements = vec![
        consumer_fixtures::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), span),
        consumer_fixtures::instruction(
            OwnedInstruction::MoveInitialize {
                destination: OwnerPlaceId(1),
                source: OwnerPlaceId(0),
            },
            span,
        ),
        consumer_fixtures::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), span),
    ];
    user.blocks[0].terminator =
        consumer_fixtures::end(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(1)), span);
    insert_user(&mut raw, user);
    let wrapper = prepare(&sources, raw, &keys, imports);
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    assert!(matches!(
        context.probe_ordinary::<false>(&sources, &mut remaining),
        Err(OwnedFailure {
            kind: OwnedFailureKind::Ownership(Violation::Unavailable),
            ..
        })
    ));
}

fn borrowing_user(span: Span, len: hir::DefId, conflict: bool) -> RawOwnedFunction {
    let vector = BorrowedTy::Exact(AggregateTy::OwnedRecordVector(RecordId(0)));
    let mut user = consumer_fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), span);
    user.locals
        .push(consumer_fixtures::scalar(hir::Ty::I32, span));
    user.parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    user.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(vector).unwrap(),
        kind: BorrowKind::Exclusive,
        position: 0,
        span,
    });
    user.calls.push(CallDecl {
        target: len,
        arguments: vec![ArgumentSlot::Borrow(LoanId(1))],
        result: CallResult::Scalar(LocalId(0)),
        parent: None,
        span,
    });
    user.loans = vec![
        LoanDecl {
            region: LoanRegion::Local,
            authority: AccessBase::Parameter(ReferenceParamId(0)),
            projection: vec![],
            kind: BorrowKind::Exclusive,
            referent: BorrowedSlot::check(vector).unwrap(),
            span,
        },
        LoanDecl {
            region: LoanRegion::Call {
                call: CallSiteId(0),
                argument: 0,
            },
            authority: AccessBase::Parameter(ReferenceParamId(0)),
            projection: vec![],
            kind: BorrowKind::Shared,
            referent: BorrowedSlot::check(vector).unwrap(),
            span,
        },
    ];
    let mut statements = vec![consumer_fixtures::instruction(
        OwnedInstruction::AcquireLocalBorrow(LoanId(0)),
        span,
    )];
    if !conflict {
        statements.push(consumer_fixtures::instruction(
            OwnedInstruction::ReleaseLocalBorrow(LoanId(0)),
            span,
        ));
    }
    statements.extend([
        consumer_fixtures::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), span),
        consumer_fixtures::instruction(
            OwnedInstruction::PrepareBorrow {
                call: CallSiteId(0),
                argument: 0,
                loan: LoanId(1),
            },
            span,
        ),
    ]);
    user.blocks = vec![
        OwnedBlock {
            merge: None,
            span,
            statements,
            terminator: consumer_fixtures::end(
                OwnedTerminatorKind::Invoke {
                    call: CallSiteId(0),
                    continuation: BlockId(1),
                },
                span,
            ),
        },
        OwnedBlock {
            merge: None,
            span,
            statements: if conflict {
                vec![consumer_fixtures::instruction(
                    OwnedInstruction::ReleaseLocalBorrow(LoanId(0)),
                    span,
                )]
            } else {
                vec![]
            },
            terminator: consumer_fixtures::end(
                OwnedTerminatorKind::ReturnScalar(consumer_fixtures::operand(0, span)),
                span,
            ),
        },
    ];
    user
}

#[test]
fn owned_vector_proof_actual_call_and_local_loan_regions_preserve_exclusive_authority() {
    for conflict in [false, true] {
        let (sources, mut raw, keys, imports) = fixture(None);
        let span = raw.records[0].span;
        let len = hir::DefId(
            2 + keys
                .iter()
                .position(|key| key.operation() == Operation::Len)
                .unwrap(),
        );
        insert_user(&mut raw, borrowing_user(span, len, conflict));
        let wrapper = prepare(&sources, raw, &keys, imports);
        let mut remaining = wrapper
            .remaining_context_budget(budget::Limits::DEFAULT)
            .unwrap();
        let context = wrapper
            .checked_context(&sources, &mut remaining, &mut Allocator::default())
            .unwrap();
        let result = context.probe_ordinary::<false>(&sources, &mut remaining);
        if conflict {
            assert!(matches!(
                result,
                Err(OwnedFailure {
                    kind: OwnedFailureKind::Ownership(Violation::LoanConflict),
                    ..
                })
            ));
        } else {
            assert!(result.is_ok());
        }
    }
}

#[test]
fn owned_vector_proof_rejects_a_cloned_function_with_the_same_ordinal() {
    let (sources, raw, keys, imports) = fixture(None);
    let wrapper = prepare(&sources, raw, &keys, imports);
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    let mut queries = ScopedSignatureQueries::new(&context, &mut remaining).unwrap();
    let function = wrapper.program.functions[0].clone();
    let mut meter = budget::Meter {
        visits: 0,
        ceiling: 1000,
    };
    assert!(matches!(
        shape::check_with_queries(
            &function,
            &wrapper.program,
            &mut queries,
            &sources,
            &mut meter,
            false
        ),
        Err(OwnedFailure {
            kind: OwnedFailureKind::Malformed(Malformed::Binding),
            ..
        })
    ));
}

fn identity_with_work_limit(work: usize) -> Result<(OwnershipUsage, usize), OwnedFailure> {
    let (sources, mut raw, keys, imports) = fixture(None);
    let span = raw.records[0].span;
    insert_user(&mut raw, identity(span));
    let wrapper = prepare(&sources, raw, &keys, imports);
    let mut limits = budget::Limits::DEFAULT;
    limits.work = work;
    let mut remaining = wrapper.remaining_context_budget(limits)?;
    let context = wrapper.checked_context(&sources, &mut remaining, &mut Allocator::default())?;
    let usage = context.probe_ordinary::<false>(&sources, &mut remaining)?;
    Ok((usage, remaining.remaining().1))
}

#[test]
fn owned_vector_proof_complete_charged_work_is_exact_and_resource_errors_stay_resources() {
    let (observed, _) = identity_with_work_limit(budget::Limits::DEFAULT.work).unwrap();
    let (exact, remaining) = identity_with_work_limit(observed.work).unwrap();
    assert_eq!(remaining, 0);
    assert_eq!(exact.work, observed.work);
    assert!(matches!(
        identity_with_work_limit(observed.work - 1),
        Err(OwnedFailure {
            kind: OwnedFailureKind::Resource(_),
            ..
        })
    ));
}
