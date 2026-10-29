use super::super::context::{signature_queries::ScopedSignatureQueries, OwnedVectorContext};
use super::*;

fn prepared() -> (SourceMap, RawOwnedVectorProgram) {
    let (sources, raw, keys, imports) = fixture(None);
    let wrapper = prepare(&sources, raw, &keys, imports);
    (sources, wrapper)
}

#[test]
fn owned_vector_signature_shared_body_reuses_one_prepaid_bank_without_escaping_facts() {
    let (sources, wrapper) = prepared();
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    let before = remaining.remaining();
    let (metadata, construction_work) = ScopedSignatureQueries::requested().unwrap();
    {
        let mut queries = ScopedSignatureQueries::new(&context, &mut remaining).unwrap();
        let initial = queries.remaining();
        assert_eq!(initial, (before.0 - metadata, before.1 - construction_work));
        shape::signatures_with_queries(&wrapper.program, &mut queries, &sources).unwrap();
        let first = queries.remaining();
        shape::signatures_with_queries(&wrapper.program, &mut queries, &sources).unwrap();
        let second = queries.remaining();
        assert_eq!(first.0, initial.0);
        assert_eq!(second.0, initial.0);
        assert!(first.1 < initial.1);
        assert_eq!(initial.1 - first.1, first.1 - second.1);
    }
    assert_eq!(remaining.remaining().0, before.0 - metadata);
    assert!(shape::signatures(
        &wrapper.program,
        wrapper.declarations.as_ref().unwrap(),
        &sources
    )
    .is_err());
}

#[test]
fn owned_vector_signature_scope_rejects_foreign_programs_and_budgets() {
    let (sources, wrapper) = prepared();
    let (_, other) = prepared();
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    let mut foreign = other
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let before = foreign.remaining();
    assert!(ScopedSignatureQueries::new(&context, &mut foreign).is_err());
    assert_eq!(foreign.remaining(), before);
    let mut queries = ScopedSignatureQueries::new(&context, &mut remaining).unwrap();
    let before = queries.remaining();
    let result = shape::signatures_with_queries(&other.program, &mut queries, &sources);
    assert!(matches!(
        result,
        Err(OwnedFailure {
            kind: OwnedFailureKind::Malformed(Malformed::Binding),
            ..
        })
    ));
    assert_eq!(queries.remaining(), before);
}

#[test]
fn owned_vector_signature_failed_nested_query_reclaims_only_its_paid_temporary_bank() {
    let (sources, wrapper) = prepared();
    let request = wrapper.context_request().unwrap();
    let (_, construction_work) = ScopedSignatureQueries::requested().unwrap();
    let mut limits = budget::Limits::DEFAULT;
    limits.work = wrapper.usage.work + request.work + construction_work + 11;
    let mut remaining = wrapper.remaining_context_budget(limits).unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    let mut queries = ScopedSignatureQueries::new(&context, &mut remaining).unwrap();
    let before = queries.remaining();
    assert_eq!(before.1, 11);
    assert!(matches!(
        shape::signatures_with_queries(&wrapper.program, &mut queries, &sources),
        Err(OwnedFailure {
            kind: OwnedFailureKind::Resource("owned vector aggregate query work"),
            ..
        })
    ));
    assert_eq!(queries.remaining(), before);
}

#[test]
fn owned_vector_signature_scope_exact_and_one_less_construction_budgets_are_real() {
    for denied in [None, Some(true), Some(false)] {
        let (sources, wrapper) = prepared();
        let mut measured = wrapper
            .remaining_context_budget(budget::Limits::DEFAULT)
            .unwrap();
        let before = measured.remaining().0;
        let measured_context = wrapper
            .checked_context(&sources, &mut measured, &mut Allocator::default())
            .unwrap();
        let retained = before - measured.remaining().0;
        drop(measured_context);
        let context_request = wrapper.context_request().unwrap();
        let (query_metadata, query_work) = OwnedVectorContext::aggregate_query_request().unwrap();
        let (scope_metadata, scope_work) = ScopedSignatureQueries::requested().unwrap();
        assert_eq!(scope_work, 4);
        let additional = context_request
            .metadata_bytes
            .saturating_sub(retained + scope_metadata);
        let calls = additional.div_ceil(query_metadata) + 1;
        let mut limits = budget::Limits::DEFAULT;
        limits.metadata =
            wrapper.usage.metadata_bytes + retained + scope_metadata + calls * query_metadata
                - usize::from(denied == Some(true));
        limits.work = wrapper.usage.work + context_request.work + scope_work + calls * query_work
            - usize::from(denied == Some(false));
        let mut remaining = wrapper.remaining_context_budget(limits).unwrap();
        let context = wrapper
            .checked_context(&sources, &mut remaining, &mut Allocator::default())
            .unwrap();
        for _ in 0..calls {
            assert!(context
                .aggregate_facts(AggregateTy::OwnedRecordVector(RecordId(0)), &mut remaining)
                .is_ok());
        }
        let before = remaining.remaining();
        assert_eq!(
            before,
            (
                scope_metadata - usize::from(denied == Some(true)),
                scope_work - usize::from(denied == Some(false))
            )
        );
        {
            let result = ScopedSignatureQueries::new(&context, &mut remaining);
            if denied.is_some() {
                assert!(matches!(
                    result,
                    Err(OwnedFailure {
                        kind: OwnedFailureKind::Resource(
                            "owned vector signature query metadata"
                                | "owned vector signature query work"
                        ),
                        ..
                    })
                ));
            } else {
                assert!(result.is_ok());
            }
        }
        assert_eq!(
            remaining.remaining(),
            if denied.is_some() { before } else { (0, 0) }
        );
    }
}

fn with_ordinary_reference(record: RecordId) -> (SourceMap, RawOwnedVectorProgram) {
    let (sources, mut raw, keys, imports) = fixture(None);
    let span = raw.records[0].span;
    let mut user = consumer_fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), span);
    user.locals
        .push(consumer_fixtures::scalar(hir::Ty::I32, span));
    user.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::OwnedRecordVector(record)))
            .unwrap(),
        kind: BorrowKind::Shared,
        position: 0,
        span,
    });
    user.parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    user.blocks.push(OwnedBlock {
        merge: None,
        span,
        statements: vec![consumer_fixtures::assign(0, Rvalue::I32(7), span)],
        terminator: consumer_fixtures::end(
            OwnedTerminatorKind::ReturnScalar(consumer_fixtures::operand(0, span)),
            span,
        ),
    });
    for function in &mut raw.functions {
        function.id.0 += 1;
    }
    raw.functions.insert(0, user);
    let wrapper = prepare(&sources, raw, &keys, imports);
    (sources, wrapper)
}

#[test]
fn owned_vector_signature_ordinary_type_queries_reject_copy_unknown_and_bad_bindings() {
    for record in [RecordId(0), RecordId(1), RecordId(2)] {
        let (sources, wrapper) = with_ordinary_reference(record);
        let mut remaining = wrapper
            .remaining_context_budget(budget::Limits::DEFAULT)
            .unwrap();
        let context = wrapper.checked_context(&sources, &mut remaining, &mut Allocator::default());
        if record != RecordId(0) {
            // The complete type-use census now refuses invalid element
            // identities before a signature-query scope can be constructed.
            assert!(matches!(
                context,
                Err(OwnedFailure {
                    kind: OwnedFailureKind::Malformed(Malformed::Binding),
                    ..
                })
            ));
            continue;
        }
        let context = context.unwrap();
        let mut queries = ScopedSignatureQueries::new(&context, &mut remaining).unwrap();
        let result = shape::signatures_with_queries(&wrapper.program, &mut queries, &sources);
        assert_eq!(result.is_ok(), record == RecordId(0));
    }
    let (sources, mut wrapper) = with_ordinary_reference(RecordId(0));
    wrapper.program.functions[0].references[0].position = 1;
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    let mut queries = ScopedSignatureQueries::new(&context, &mut remaining).unwrap();
    assert!(matches!(
        shape::signatures_with_queries(&wrapper.program, &mut queries, &sources),
        Err(OwnedFailure {
            kind: OwnedFailureKind::Malformed(Malformed::Binding),
            ..
        })
    ));
}

#[test]
fn owned_vector_signature_success_still_cannot_construct_an_ordinary_seal() {
    let (sources, wrapper) = prepared();
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    {
        let mut queries = ScopedSignatureQueries::new(&context, &mut remaining).unwrap();
        shape::signatures_with_queries(&wrapper.program, &mut queries, &sources).unwrap();
    }
    drop(context);
    assert!(verify_owned(wrapper.program, &sources).is_err());
}
