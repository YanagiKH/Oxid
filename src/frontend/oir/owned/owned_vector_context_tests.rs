//! Real owning-wrapper and bounds-factory controls. No executable witness is
//! constructed from this inert context.
use super::super::context::{ContextRequest, ProgramDomainQueries};
use super::*;
use crate::frontend::oir::owned::owned_record_codec::bounds::CheckedElements;

fn exact_limits(wrapper: &RawOwnedVectorProgram, request: ContextRequest) -> budget::Limits {
    let mut limits = budget::Limits::DEFAULT;
    limits.metadata = wrapper
        .usage
        .metadata_bytes
        .checked_add(request.metadata_bytes)
        .unwrap();
    limits.work = wrapper.usage.work.checked_add(request.work).unwrap();
    limits
}

fn prepared_fixture() -> (SourceMap, RawOwnedVectorProgram, [Function; 8]) {
    let (sources, raw, keys, imports) = fixture(None);
    let wrapper = prepare(&sources, raw, &keys, imports);
    (sources, wrapper, keys)
}

fn query_domains<Q: ProgramDomainQueries>(
    query: &Q,
    id: hir::DefId,
) -> (Option<usize>, Option<vector::Function>, Option<Function>) {
    (
        query.fixed_builtin_base(),
        query.old_vector_operation(id),
        query.owned_vector_operation(id),
    )
}

fn observed_context_controls() -> usize {
    let (sources, raw, _) = consumer_fixtures::shared_read();
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    let request = wrapper.context_request().unwrap();
    let mut remaining = wrapper
        .remaining_context_budget(exact_limits(&wrapper, request))
        .unwrap();
    let before = remaining.remaining().0;
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    assert!(context.is_legacy());
    before - remaining.remaining().0
}

#[test]
fn owned_vector_context_binds_exact_wrapper_facade_suffix_and_element() {
    let (sources, wrapper, keys) = prepared_fixture();
    let (_, other, _) = prepared_fixture();
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let mut allocator = Allocator::default();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut allocator)
        .unwrap();
    assert_eq!(allocator.attempts, 2);
    assert!(context.belongs_to(&wrapper));
    assert!(!context.belongs_to(&other));
    assert!(!context.is_legacy());
    let old = vector::Function::new(vector::Operation::New, hir::Ty::I32).unwrap();
    assert_eq!(
        query_domains(&context, hir::DefId(0)),
        (Some(0), Some(old), None)
    );
    for (rank, key) in keys.into_iter().enumerate() {
        let function = hir::DefId(rank + 1);
        assert_eq!(
            query_domains(&context, function),
            (Some(0), None, Some(key))
        );
        let element = context.element_for(function, &mut remaining).unwrap();
        assert_eq!(element.record(), RecordId(0));
        assert_eq!(element.size(), 4);
        assert!(std::ptr::eq(
            element.declarations(),
            wrapper.declarations.as_ref().unwrap()
        ));
        assert!(!std::ptr::eq(
            element.declarations(),
            other.declarations.as_ref().unwrap()
        ));
    }
    assert_eq!(context.owned_vector_operation(hir::DefId(9)), None);
    assert_eq!(context.owned_vector_operation(hir::DefId(usize::MAX)), None);
    assert_eq!(context.old_vector_operation(hir::DefId(usize::MAX)), None);
    assert!(context.element_for(hir::DefId(0), &mut remaining).is_err());
}

#[test]
fn owned_vector_context_rejects_a_different_wrapper_budget_before_any_allocation() {
    let (sources, wrapper, _) = prepared_fixture();
    let (_, other, _) = prepared_fixture();
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let before = remaining.remaining();
    let mut allocator = Allocator::default();
    assert!(other
        .checked_context(&sources, &mut remaining, &mut allocator)
        .is_err());
    assert_eq!(allocator.attempts, 0);
    assert_eq!(remaining.remaining(), before);
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut allocator)
        .unwrap();
    let mut other_budget = other
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let before = other_budget.remaining();
    assert!(context
        .element_for(hir::DefId(1), &mut other_budget)
        .is_err());
    assert_eq!(other_budget.remaining(), before);
}

#[test]
fn owned_vector_context_prepays_exact_shared_metadata_and_work_before_allocations() {
    for denied in [None, Some(true), Some(false)] {
        let (sources, wrapper, _) = prepared_fixture();
        let request = wrapper.context_request().unwrap();
        let mut limits = exact_limits(&wrapper, request);
        if let Some(metadata) = denied {
            if metadata {
                limits.metadata -= 1;
            } else {
                limits.work -= 1;
            }
        }
        let mut remaining = wrapper.remaining_context_budget(limits).unwrap();
        let before = remaining.remaining();
        let mut allocator = Allocator::default();
        let result = wrapper.checked_context(&sources, &mut remaining, &mut allocator);
        if denied.is_some() {
            assert!(matches!(
                result,
                Err(OwnedFailure {
                    kind: OwnedFailureKind::Resource(
                        "owned vector context metadata" | "owned vector context work"
                    ),
                    ..
                })
            ));
            assert_eq!(allocator.attempts, 0);
            assert_eq!(remaining.remaining(), before);
        } else {
            let context = result.unwrap();
            assert_eq!(allocator.attempts, 2);
            assert_eq!(remaining.remaining().1, 0);
            assert!(remaining.remaining().0 < before.0);
            assert!(context.retained_bounds_bytes() > 0);
        }
        assert!(wrapper.check(&sources, budget::Limits::DEFAULT).is_ok());
    }
}

#[test]
fn owned_vector_context_both_real_bounds_reservations_fail_without_retaining_a_table() {
    for fail_at in 1..=2 {
        let (sources, wrapper, _) = prepared_fixture();
        let request = wrapper.context_request().unwrap();
        let mut remaining = wrapper
            .remaining_context_budget(exact_limits(&wrapper, request))
            .unwrap();
        let before = remaining.remaining();
        let mut allocator = Allocator {
            fail_at: Some(fail_at),
            ..Default::default()
        };
        let result = wrapper.checked_context(&sources, &mut remaining, &mut allocator);
        assert!(matches!(
            result,
            Err(OwnedFailure {
                kind: OwnedFailureKind::Resource("owned vector context bounds allocation"),
                ..
            })
        ));
        assert_eq!(allocator.attempts, fail_at);
        assert_eq!(remaining.remaining(), (before.0, 0));
        assert!(wrapper.check(&sources, budget::Limits::DEFAULT).is_ok());
    }
}

#[test]
fn owned_vector_context_accounts_the_actual_retained_bound_table_and_query_work() {
    let controls = observed_context_controls();
    for query_work in [3, 4] {
        for record_count in [2, 17] {
            let (sources, mut raw, keys, imports) = fixture(None);
            let span = raw.records[0].span;
            for id in raw.records.len()..record_count {
                raw.records.push(RawRecordDecl {
                    id: RecordId(id),
                    is_copy: false,
                    span,
                    fields: vec![RawFieldDecl {
                        id: FieldId {
                            record: RecordId(id),
                            index: 0,
                        },
                        ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)),
                        span,
                    }],
                });
            }
            let wrapper = prepare(&sources, raw, &keys, imports);
            let request = wrapper.context_request().unwrap();
            let mut limits = exact_limits(&wrapper, request);
            limits.work += query_work;
            let mut remaining = wrapper.remaining_context_budget(limits).unwrap();
            let metadata_before = remaining.remaining().0;
            let mut allocator = Allocator::default();
            let context = wrapper
                .checked_context(&sources, &mut remaining, &mut allocator)
                .unwrap();
            let declarations = wrapper.declarations.as_ref().unwrap();
            let mut independent_allocator = Allocator::default();
            let independent = CheckedElements::check_with_allocator(
                declarations,
                plan::MAX_PLAN_BYTES,
                &mut independent_allocator,
            )
            .unwrap();
            assert_eq!(independent_allocator.attempts, 2);
            assert_eq!(
                context.retained_bounds_bytes(),
                independent.retained_bytes().unwrap()
            );
            // Derive the constant caller bank through a real legacy context and
            // the variable table through a separate real bounds allocation. The
            // shared ledger must retain exactly those complete charged scopes.
            assert_eq!(
                metadata_before - remaining.remaining().0,
                controls + independent.retained_bytes().unwrap()
            );
            assert!(context.retained_bounds_bytes() < request.metadata_bytes);
            let before = remaining.remaining();
            let result = context.element_for(hir::DefId(1), &mut remaining);
            if query_work == 3 {
                assert!(matches!(
                    result,
                    Err(OwnedFailure {
                        kind: OwnedFailureKind::Resource("owned vector context element work"),
                        ..
                    })
                ));
                assert_eq!(remaining.remaining(), before);
            } else {
                let element = result.unwrap();
                assert_eq!(element.record(), RecordId(0));
                assert_eq!(remaining.remaining().1, 0);
            }
        }
    }
}

#[test]
fn owned_vector_context_true_legacy_allocates_no_bounds_and_keeps_ordinary_behavior() {
    let (sources, raw, _) = consumer_fixtures::shared_read();
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    let legacy_domains = query_domains(&wrapper.program, hir::DefId(0));
    let request = wrapper.context_request().unwrap();
    let mut remaining = wrapper
        .remaining_context_budget(exact_limits(&wrapper, request))
        .unwrap();
    let mut allocator = Allocator::default();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut allocator)
        .unwrap();
    assert_eq!(allocator.attempts, 0);
    assert!(context.is_legacy());
    assert_eq!(context.retained_bounds_bytes(), 0);
    assert_eq!(query_domains(&context, hir::DefId(0)), legacy_domains);
    drop(context);
    assert!(verify_owned(wrapper.program, &sources).is_ok());
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(size_of::<RawOwnedProgram>(), 112);
        assert_eq!(size_of::<OwnedInstruction>(), 128);
        assert_eq!(size_of::<vector::Function>(), 2);
        assert_eq!(size_of::<Function>(), 2);
    }
}

#[test]
fn owned_vector_context_zero_keys_never_classify_type_only_extensions_as_legacy() {
    enum Role {
        Result,
        Owner,
        Parameter,
        Reference,
        Loan,
        Field,
        Variant,
        Opcode,
    }
    for role in [
        Role::Result,
        Role::Owner,
        Role::Parameter,
        Role::Reference,
        Role::Loan,
        Role::Field,
        Role::Variant,
        Role::Opcode,
    ] {
        let (sources, mut raw, _) = consumer_fixtures::shared_read();
        let aggregate = AggregateTy::OwnedRecordVector(RecordId(0));
        let span = raw.functions[0].span;
        match role {
            Role::Result => raw.functions[0].result = ValueTy::Owned(aggregate),
            Role::Owner => {
                raw.functions[0].owners[0].aggregate =
                    AggregateSlot::try_from_aggregate(aggregate).unwrap()
            }
            Role::Parameter => {
                let callee = &mut raw.functions[1];
                let owner = OwnerPlaceId(callee.owners.len());
                callee.owners.push(OwnerDecl {
                    aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
                    kind: OwnerKind::Parameter {
                        position: callee.parameters.len(),
                    },
                    span,
                });
                callee.parameters.push(ParameterBinding::Owned(owner));
            }
            Role::Reference => {
                raw.functions[1].references[0].referent =
                    BorrowedSlot::check(BorrowedTy::Exact(aggregate)).unwrap()
            }
            Role::Loan => {
                raw.functions[0].loans[0].referent =
                    BorrowedSlot::check(BorrowedTy::Exact(aggregate)).unwrap()
            }
            Role::Field => {
                raw.records[0].fields[0].ty = ParameterTy::Value(ValueTy::Owned(aggregate))
            }
            Role::Variant => raw.enums.push(RawEnumDecl {
                id: EnumId(0),
                span,
                variants: vec![RawVariantDecl {
                    id: VariantId {
                        enumeration: EnumId(0),
                        index: 0,
                    },
                    payload: Some(ParameterTy::Value(ValueTy::Owned(aggregate))),
                    span,
                }],
            }),
            Role::Opcode => {
                raw.functions[0].blocks[0]
                    .statements
                    .push(consumer_fixtures::instruction(
                        OwnedInstruction::OwnedVector {
                            operation: Function::record(Operation::New, RecordId(0)).unwrap(),
                            destination: CallResult::Owned(OwnerPlaceId(0)),
                        },
                        span,
                    ))
            }
        }
        let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
        assert!(wrapper.claims.functions.is_empty());
        assert!(matches!(
            wrapper.context_request(),
            Err(OwnedFailure {
                kind: OwnedFailureKind::Malformed(Malformed::Type),
                ..
            })
        ));
        let mut remaining = wrapper
            .remaining_context_budget(budget::Limits::DEFAULT)
            .unwrap();
        let before = remaining.remaining();
        let mut allocator = Allocator::default();
        assert!(wrapper
            .checked_context(&sources, &mut remaining, &mut allocator)
            .is_err());
        assert_eq!(allocator.attempts, 0);
        assert_eq!(remaining.remaining(), before);
    }
}

#[test]
fn owned_vector_context_remains_inert_at_the_ordinary_seal_boundary() {
    let (sources, wrapper, _) = prepared_fixture();
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    assert!(!context.is_legacy());
    let declarations = wrapper.declarations.as_ref().unwrap();
    assert!(declarations
        .check_aggregate_type(AggregateTy::OwnedRecordVector(RecordId(0)))
        .is_err());
    assert!(shape::signatures(&wrapper.program, declarations, &sources).is_err());
    drop(context);
    assert!(verify_owned(wrapper.program, &sources).is_err());
}
