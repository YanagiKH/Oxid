use super::super::context::queries::DirectResourceKind;
use super::*;
use crate::frontend::oir::owned::owned_record_codec::bounds::CheckedElements;

fn type_only() -> (SourceMap, RawOwnedProgram) {
    let (sources, mut raw, _, _) = fixture(None);
    raw.builtins = BuiltinOrigins::None.admit(BuiltinItem::Vector(vector::Item::Vec));
    raw.functions.clear();
    raw.enums.clear();
    raw.vector_functions.clear();
    (sources, raw)
}

fn member(record: usize, index: usize, ty: ValueTy, span: Span) -> RawFieldDecl {
    RawFieldDecl {
        id: FieldId {
            record: RecordId(record),
            index,
        },
        ty: ParameterTy::Value(ty),
        span,
    }
}

fn record(raw: &mut RawOwnedProgram, fields: &[ValueTy], span: Span) -> RecordId {
    let id = RecordId(raw.records.len());
    raw.records.push(RawRecordDecl {
        id,
        is_copy: false,
        span,
        fields: fields
            .iter()
            .copied()
            .enumerate()
            .map(|(index, ty)| member(id.0, index, ty, span))
            .collect(),
    });
    id
}

fn forward(aggregate: AggregateTy, span: Span) -> RawOwnedFunction {
    let mut function = consumer_fixtures::function(0, ValueTy::Owned(aggregate), span);
    function
        .parameters
        .push(ParameterBinding::Owned(OwnerPlaceId(0)));
    function.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
        kind: OwnerKind::Parameter { position: 0 },
        span,
    });
    function.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![],
        terminator: consumer_fixtures::end(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)), span),
    });
    function
}

#[test]
fn owned_vector_declaration_zero_operation_type_only_parameters_and_returns_are_real() {
    for borrowed in [false, true] {
        let (sources, mut raw) = type_only();
        let span = raw.records[0].span;
        let vector = AggregateTy::OwnedRecordVector(RecordId(0));
        let function = if borrowed {
            let mut function = consumer_fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), span);
            function
                .locals
                .push(consumer_fixtures::scalar(hir::Ty::I32, span));
            function
                .parameters
                .push(ParameterBinding::Reference(ReferenceParamId(0)));
            function.references.push(ReferenceDecl {
                referent: BorrowedSlot::check(BorrowedTy::Exact(vector)).unwrap(),
                kind: BorrowKind::Shared,
                position: 0,
                span,
            });
            function.blocks.push(OwnedBlock {
                span,
                merge: None,
                statements: vec![consumer_fixtures::assign(0, Rvalue::I32(7), span)],
                terminator: consumer_fixtures::end(
                    OwnedTerminatorKind::ReturnScalar(consumer_fixtures::operand(0, span)),
                    span,
                ),
            });
            function
        } else {
            forward(vector, span)
        };
        raw.functions.push(function);
        let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
        assert!(wrapper.claims.functions.is_empty());
        assert_eq!(wrapper.claims.functions.capacity(), 0);
        assert!(wrapper.declarations.is_some());
        let mut remaining = wrapper
            .remaining_context_budget(budget::Limits::DEFAULT)
            .unwrap();
        let mut allocator = Allocator::default();
        let context = wrapper
            .checked_context(&sources, &mut remaining, &mut allocator)
            .unwrap();
        assert_eq!(allocator.attempts, 2);
        assert!(!context.is_legacy());
        assert_eq!(
            context
                .aggregate_facts(vector, &mut remaining)
                .unwrap()
                .direct_resource(),
            Some(DirectResourceKind::OwnedRecordVector(RecordId(0)))
        );
        assert!(context
            .probe_ordinary::<false>(&sources, &mut remaining)
            .is_ok());
        drop(context);
        assert!(verify_owned(wrapper.program, &sources).is_err());
    }
}

#[test]
fn owned_vector_declaration_direct_unrelated_container_keeps_eligible_element_and_exact_projection()
{
    let (sources, mut raw) = type_only();
    let span = raw.records[0].span;
    let vector = AggregateTy::OwnedRecordVector(RecordId(0));
    let container = record(&mut raw, &[ValueTy::Owned(vector)], span);
    raw.functions
        .push(forward(AggregateTy::Record(container), span));
    assert!(Declarations::check_combined(&raw.records, &raw.enums, &sources).is_err());
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    let facade = wrapper.declarations.as_ref().unwrap();
    assert_eq!(
        facade
            .projection(
                AggregateTy::Record(container),
                &[FieldId {
                    record: container,
                    index: 0
                }]
            )
            .unwrap(),
        (ValueTy::Owned(vector), 0)
    );
    assert!(facade
        .contains_resources(AggregateTy::Record(container))
        .unwrap());
    assert_eq!(
        facade
            .aggregate_layout(AggregateTy::Record(container))
            .unwrap()
            .size(),
        16
    );
    assert_eq!(
        facade
            .aggregate_width(AggregateTy::Record(container))
            .unwrap(),
        // Record width is max(sum(field widths), 1); the sole descriptor
        // contributes three cells. Owner-state storage is accounted separately.
        3
    );
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    assert!(context.aggregate_facts(vector, &mut remaining).is_ok());
    assert!(context
        .aggregate_facts(AggregateTy::OwnedRecordVector(container), &mut remaining)
        .is_err());
    assert!(context
        .probe_ordinary::<false>(&sources, &mut remaining)
        .is_ok());
}

#[test]
fn owned_vector_declaration_shared_dag_exclusion_propagates_without_poisoning_other_nominals() {
    let (sources, mut raw) = type_only();
    let span = raw.records[0].span;
    let container = record(
        &mut raw,
        &[ValueTy::Owned(AggregateTy::OwnedRecordVector(RecordId(0)))],
        span,
    );
    let left = record(
        &mut raw,
        &[
            ValueTy::Owned(AggregateTy::Record(container)),
            ValueTy::Owned(AggregateTy::Record(RecordId(0))),
        ],
        span,
    );
    let right = record(
        &mut raw,
        &[
            ValueTy::Owned(AggregateTy::Record(RecordId(0))),
            ValueTy::Owned(AggregateTy::Record(container)),
        ],
        span,
    );
    let shared = record(
        &mut raw,
        &[
            ValueTy::Owned(AggregateTy::Record(left)),
            ValueTy::Owned(AggregateTy::Record(right)),
            ValueTy::Owned(AggregateTy::Record(left)),
        ],
        span,
    );
    let facade =
        Declarations::check_combined_owned_vectors(&raw.records, &raw.enums, &sources).unwrap();
    let bounds = CheckedElements::check(&facade, plan::MAX_PLAN_BYTES).unwrap();
    assert!(bounds.element(RecordId(0)).is_ok());
    for id in [container, left, right, shared] {
        assert!(bounds.element(id).is_err());
    }
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    assert!(!context.is_legacy());
    assert!(context
        .probe_ordinary::<false>(&sources, &mut remaining)
        .is_ok());
}

#[test]
fn owned_vector_declaration_enum_valid_alternative_cannot_hide_ineligible_owned_vector_path() {
    for excluded_first in [false, true] {
        let (sources, mut raw) = type_only();
        let span = raw.records[0].span;
        let vector = ValueTy::Owned(AggregateTy::OwnedRecordVector(RecordId(0)));
        let wide = ValueTy::Owned(AggregateTy::FixedArray(
            FixedArrayTy::check(hir::Ty::I128, 12).unwrap(),
        ));
        let values = if excluded_first {
            [vector, wide]
        } else {
            [wide, vector]
        };
        raw.enums.push(RawEnumDecl {
            id: EnumId(0),
            span,
            variants: values
                .into_iter()
                .enumerate()
                .map(|(index, ty)| RawVariantDecl {
                    id: VariantId {
                        enumeration: EnumId(0),
                        index,
                    },
                    payload: Some(ParameterTy::Value(ty)),
                    span,
                })
                .collect(),
        });
        let container = record(
            &mut raw,
            &[ValueTy::Owned(AggregateTy::Enum(EnumId(0)))],
            span,
        );
        let parent = record(
            &mut raw,
            &[
                ValueTy::Owned(AggregateTy::Record(container)),
                ValueTy::Owned(AggregateTy::Record(container)),
            ],
            span,
        );
        raw.functions
            .push(forward(AggregateTy::Record(parent), span));
        assert!(Declarations::check_combined(&raw.records, &raw.enums, &sources).is_err());
        let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
        let facade = wrapper.declarations.as_ref().unwrap();
        let index = usize::from(!excluded_first);
        let variant = facade
            .enums()
            .variant(
                EnumId(0),
                VariantId {
                    enumeration: EnumId(0),
                    index,
                },
            )
            .unwrap();
        assert_eq!(variant.value_payload(), Some(vector));
        assert_eq!(variant.payload_offset(), Some(16));
        assert!(facade
            .contains_resources(AggregateTy::Record(parent))
            .unwrap());
        let bounds = CheckedElements::check(facade, plan::MAX_PLAN_BYTES).unwrap();
        assert!(bounds.element(RecordId(0)).is_ok());
        assert!(bounds.element(container).is_err());
        assert!(bounds.element(parent).is_err());
        let mut remaining = wrapper
            .remaining_context_budget(budget::Limits::DEFAULT)
            .unwrap();
        let context = wrapper
            .checked_context(&sources, &mut remaining, &mut Allocator::default())
            .unwrap();
        assert!(context
            .probe_ordinary::<false>(&sources, &mut remaining)
            .is_ok());
    }
}

#[test]
fn owned_vector_declaration_whole_vector_field_loan_uses_real_shared_projection_proof() {
    let (sources, mut raw) = type_only();
    let span = raw.records[0].span;
    let vector = AggregateTy::OwnedRecordVector(RecordId(0));
    let container = record(&mut raw, &[ValueTy::Owned(vector)], span);
    let mut function = consumer_fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), span);
    function
        .locals
        .push(consumer_fixtures::scalar(hir::Ty::I32, span));
    function
        .parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    function.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(container))).unwrap(),
        kind: BorrowKind::Exclusive,
        position: 0,
        span,
    });
    function.loans.push(LoanDecl {
        region: LoanRegion::Call {
            call: CallSiteId(0),
            argument: 0,
        },
        authority: AccessBase::Parameter(ReferenceParamId(0)),
        projection: vec![FieldId {
            record: container,
            index: 0,
        }],
        kind: BorrowKind::Exclusive,
        referent: BorrowedSlot::check(BorrowedTy::Exact(vector)).unwrap(),
        span,
    });
    function.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
        result: CallResult::Scalar(LocalId(0)),
        parent: None,
        span,
    });
    function.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![
            consumer_fixtures::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), span),
            consumer_fixtures::instruction(
                OwnedInstruction::PrepareBorrow {
                    call: CallSiteId(0),
                    argument: 0,
                    loan: LoanId(0),
                },
                span,
            ),
        ],
        terminator: consumer_fixtures::end(
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            span,
        ),
    });
    function.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![],
        terminator: consumer_fixtures::end(
            OwnedTerminatorKind::ReturnScalar(consumer_fixtures::operand(0, span)),
            span,
        ),
    });
    raw.functions.push(function);
    let mut helper = consumer_fixtures::function(1, ValueTy::Scalar(hir::Ty::I32), span);
    helper
        .locals
        .push(consumer_fixtures::scalar(hir::Ty::I32, span));
    helper
        .parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    helper.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(vector)).unwrap(),
        kind: BorrowKind::Exclusive,
        position: 0,
        span,
    });
    helper.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![consumer_fixtures::assign(0, Rvalue::I32(7), span)],
        terminator: consumer_fixtures::end(
            OwnedTerminatorKind::ReturnScalar(consumer_fixtures::operand(0, span)),
            span,
        ),
    });
    raw.functions.push(helper);
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    let context = wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .unwrap();
    context
        .probe_ordinary::<false>(&sources, &mut remaining)
        .expect("valid projected whole-vector call loan was refused");
}

#[test]
fn owned_vector_declaration_recursive_nested_copy_and_unknown_element_types_are_rejected() {
    for target in [RecordId(0), RecordId(1), RecordId(99)] {
        let (sources, mut raw) = type_only();
        raw.records[0].fields[0].ty =
            ParameterTy::Value(ValueTy::Owned(AggregateTy::OwnedRecordVector(target)));
        let result = RawOwnedVectorProgram::prepare(
            raw,
            OwnedVectorImportClaims::empty(),
            &[],
            &sources,
            budget::Limits::DEFAULT,
            &mut Allocator::default(),
        );
        if target == RecordId(0) {
            let wrapper = result.unwrap();
            let mut remaining = wrapper
                .remaining_context_budget(budget::Limits::DEFAULT)
                .unwrap();
            assert!(wrapper
                .checked_context(&sources, &mut remaining, &mut Allocator::default())
                .is_err());
        } else {
            assert!(result.is_err());
        }
    }
    let (sources, mut raw) = type_only();
    let span = raw.records[0].span;
    let inner = record(
        &mut raw,
        &[ValueTy::Owned(AggregateTy::OwnedRecordVector(RecordId(0)))],
        span,
    );
    record(
        &mut raw,
        &[ValueTy::Owned(AggregateTy::OwnedRecordVector(inner))],
        span,
    );
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    let mut remaining = wrapper
        .remaining_context_budget(budget::Limits::DEFAULT)
        .unwrap();
    assert!(wrapper
        .checked_context(&sources, &mut remaining, &mut Allocator::default())
        .is_err());
}

#[test]
fn owned_vector_declaration_zero_key_headers_validate_every_mentioned_element_nominal() {
    for target in [RecordId(1), RecordId(99)] {
        let (sources, mut raw) = type_only();
        let span = raw.records[0].span;
        raw.functions
            .push(forward(AggregateTy::OwnedRecordVector(target), span));
        let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
        let mut remaining = wrapper
            .remaining_context_budget(budget::Limits::DEFAULT)
            .unwrap();
        assert!(wrapper
            .checked_context(&sources, &mut remaining, &mut Allocator::default())
            .is_err());
    }
}

#[test]
fn owned_vector_declaration_type_only_bounds_failures_use_both_real_allocation_sites() {
    for fail_at in 1..=2 {
        let (sources, mut raw) = type_only();
        let span = raw.records[0].span;
        raw.functions
            .push(forward(AggregateTy::OwnedRecordVector(RecordId(0)), span));
        let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
        let mut remaining = wrapper
            .remaining_context_budget(budget::Limits::DEFAULT)
            .unwrap();
        let mut allocator = Allocator {
            fail_at: Some(fail_at),
            ..Default::default()
        };
        assert!(matches!(
            wrapper.checked_context(&sources, &mut remaining, &mut allocator),
            Err(OwnedFailure {
                kind: OwnedFailureKind::Resource("owned vector context bounds allocation"),
                ..
            })
        ));
        assert_eq!(allocator.attempts, fail_at);
    }
}
