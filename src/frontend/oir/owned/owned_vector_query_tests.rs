use super::super::context::{queries::DirectResourceKind, OwnedVectorContext};
use super::*;

fn prepared() -> (SourceMap, RawOwnedVectorProgram) {
    let (sources, raw, keys, imports) = fixture(None);
    let wrapper = prepare(&sources, raw, &keys, imports);
    (sources, wrapper)
}

#[test]
fn owned_vector_query_facts_keep_exact_context_and_distinct_resource_identity() {
    let (sources, wrapper) = prepared();
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let mut allocator = Allocator::default();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut allocator)
        .unwrap();
    let other = wrapper
        .checked_context(&sources, &mut remaining, &mut allocator)
        .unwrap();
    let attempts = allocator.attempts;
    let ty = AggregateTy::OwnedRecordVector(RecordId(0));
    let facts = context.aggregate_facts(ty, &mut remaining).unwrap();
    assert!(facts.belongs_to(&context));
    assert!(!facts.belongs_to(&other));
    assert_eq!(facts.aggregate(), ty);
    assert_eq!(facts.layout().size(), 16);
    assert_eq!(facts.layout().align(), 8);
    assert_eq!(facts.width(), 3);
    assert!(!facts.is_copy());
    assert!(facts.contains_resources());
    assert_eq!(
        facts.direct_resource(),
        Some(DirectResourceKind::OwnedRecordVector(RecordId(0)))
    );
    assert_ne!(
        facts.direct_resource(),
        Some(DirectResourceKind::Existing(ResourceKind::ByteBuf))
    );
    assert_ne!(
        facts.direct_resource(),
        Some(DirectResourceKind::Existing(ResourceKind::RecordVector(0)))
    );
    assert_eq!(allocator.attempts, attempts);
    assert!(wrapper
        .declarations
        .as_ref()
        .unwrap()
        .check_aggregate_type(ty)
        .is_err());
}

#[test]
fn owned_vector_query_old_facts_use_the_actual_unmodified_facade() {
    let (sources, wrapper) = prepared();
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    let declarations = wrapper.declarations.as_ref().unwrap();
    for aggregate in [
        AggregateTy::Record(RecordId(0)),
        AggregateTy::Record(RecordId(1)),
        AggregateTy::Enum(EnumId(0)),
        AggregateTy::ByteBuf,
        AggregateTy::File,
        AggregateTy::Vector(hir::Ty::I32),
        AggregateTy::RecordVector(RecordId(1)),
        AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 0).unwrap()),
    ] {
        let facts = context.aggregate_facts(aggregate, &mut remaining).unwrap();
        assert_eq!(
            facts.layout(),
            declarations.aggregate_layout(aggregate).unwrap()
        );
        assert_eq!(
            facts.width(),
            declarations.aggregate_width(aggregate).unwrap()
        );
        assert_eq!(facts.is_copy(), declarations.is_copy(aggregate));
        assert_eq!(
            facts.contains_resources(),
            declarations.contains_resources(aggregate).unwrap()
        );
        assert_eq!(facts.aggregate(), aggregate);
    }
    let scalar = context
        .aggregate_facts(AggregateTy::Vector(hir::Ty::I32), &mut remaining)
        .unwrap();
    assert_eq!(
        scalar.direct_resource(),
        Some(DirectResourceKind::Existing(ResourceKind::Vector(
            hir::Ty::I32
        )))
    );
    let copy = context
        .aggregate_facts(AggregateTy::RecordVector(RecordId(1)), &mut remaining)
        .unwrap();
    assert_eq!(
        copy.direct_resource(),
        Some(DirectResourceKind::Existing(ResourceKind::RecordVector(1)))
    );
}

#[test]
fn owned_vector_query_exact_and_one_less_metadata_work_share_the_remaining_ledger() {
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
        let (metadata, work) = OwnedVectorContext::aggregate_query_request().unwrap();
        assert_eq!(work, 12);
        // Cover the real context's temporary peak, then fund complete query
        // scopes. Earlier real queries leave an exact final query allowance.
        let calls = (context_request.metadata_bytes - retained).div_ceil(metadata) + 1;
        let mut limits = budget::Limits::DEFAULT;
        limits.metadata = wrapper.usage.metadata_bytes + retained + calls * metadata
            - usize::from(denied == Some(true));
        limits.work = wrapper.usage.work + context_request.work + calls * work
            - usize::from(denied == Some(false));
        let mut remaining = wrapper.remaining_context_budget(limits).unwrap();
        let context = wrapper
            .checked_context(&sources, &mut remaining, &mut Allocator::default())
            .unwrap();
        for _ in 1..calls {
            assert!(context
                .aggregate_facts(AggregateTy::OwnedRecordVector(RecordId(0)), &mut remaining)
                .is_ok());
        }
        let before = remaining.remaining();
        assert_eq!(
            before,
            (
                metadata - usize::from(denied == Some(true)),
                work - usize::from(denied == Some(false))
            )
        );
        let result =
            context.aggregate_facts(AggregateTy::OwnedRecordVector(RecordId(0)), &mut remaining);
        if denied.is_some() {
            assert!(matches!(
                result,
                Err(OwnedFailure {
                    kind: OwnedFailureKind::Resource(
                        "owned vector aggregate query metadata"
                            | "owned vector aggregate query work"
                    ),
                    ..
                })
            ));
            assert_eq!(remaining.remaining(), before);
        } else {
            assert!(result.is_ok());
            assert_eq!(remaining.remaining(), (0, 0));
        }
    }
}

#[test]
fn owned_vector_query_copy_unknown_and_foreign_facade_claims_cannot_gain_facts() {
    let (sources, wrapper) = prepared();
    let (_, other) = prepared();
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    for ty in [
        AggregateTy::OwnedRecordVector(RecordId(1)),
        AggregateTy::OwnedRecordVector(RecordId(2)),
        AggregateTy::OwnedRecordVector(RecordId(usize::MAX)),
        AggregateTy::Record(RecordId(2)),
    ] {
        assert!(context.aggregate_facts(ty, &mut remaining).is_err());
    }
    let mut foreign = other
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let before = foreign.remaining();
    assert!(context
        .aggregate_facts(AggregateTy::OwnedRecordVector(RecordId(0)), &mut foreign)
        .is_err());
    assert_eq!(foreign.remaining(), before);
}
