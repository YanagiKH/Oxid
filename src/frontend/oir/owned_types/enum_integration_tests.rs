//! Combined admission and exclusion tests; these create no executable witness.
use super::enums::{EnumDecl, VariantDecl};
use super::tests::{record, source};
use super::*;

fn enumeration(id: usize, span: Span) -> RawEnumDecl {
    RawEnumDecl {
        id: EnumId(id),
        span,
        variants: vec![RawVariantDecl {
            id: VariantId {
                enumeration: EnumId(id),
                index: 0,
            },
            payload: Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32))),
            span,
        }],
    }
}

#[test]
fn bounded_enum_nominal_identity_and_compact_slots_are_distinct() {
    let (sources, span) = source();
    let d = Declarations::check_combined(
        &[record(0, &[], span)],
        &[enumeration(0, span), enumeration(1, span)],
        &sources,
    )
    .unwrap();
    for id in [EnumId(0), EnumId(1)] {
        let ty = AggregateTy::Enum(id);
        let slot = AggregateSlot::try_from_aggregate(ty).unwrap();
        assert_eq!(slot.aggregate(), ty);
        assert_eq!(
            d.aggregate_layout(ty).unwrap(),
            Layout { size: 8, align: 4 }
        );
        assert_eq!(d.aggregate_width(ty).unwrap(), 2);
        d.same_aggregate_type(ty, ty).unwrap();
        assert_eq!(
            d.same_aggregate_type(ty, AggregateTy::Record(RecordId(0))),
            Err(DeclarationError::TypeMismatch)
        );
    }
    assert_eq!(
        d.same_aggregate_type(AggregateTy::Enum(EnumId(0)), AggregateTy::Enum(EnumId(1))),
        Err(DeclarationError::TypeMismatch)
    );
    for id in [2, 4096, u32::MAX as usize, usize::MAX] {
        let ty = AggregateTy::Enum(EnumId(id));
        assert_eq!(
            d.same_aggregate_type(ty, ty),
            Err(DeclarationError::InvalidEnumId(EnumId(id)))
        );
        assert_eq!(
            d.same_value_type(ValueTy::Owned(ty), ValueTy::Owned(ty)),
            Err(DeclarationError::InvalidEnumId(EnumId(id)))
        );
        assert!(d.aggregate_layout(ty).is_err());
        assert!(d.aggregate_width(ty).is_err());
    }
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(usize::MAX))),
        Err(DeclarationError::InvalidEnumId(EnumId(usize::MAX)))
    );
}

#[test]
fn bounded_enum_borrow_field_projection_and_static_leaves_are_closed() {
    let (sources, span) = source();
    let raw = [enumeration(0, span)];
    let d = Declarations::check_combined(&[], &raw, &sources).unwrap();
    for id in [0, usize::MAX] {
        let ty = AggregateTy::Enum(EnumId(id));
        let borrow = BorrowedTy::Exact(ty);
        assert!(!borrow.accepts(borrow));
        assert_eq!(borrow.element(), None);
        assert_eq!(
            BorrowedSlot::check(borrow),
            Err(DeclarationError::TypeMismatch)
        );
        assert_eq!(
            d.check_borrowed_type(borrow),
            Err(DeclarationError::TypeMismatch)
        );
        assert_eq!(
            d.same_borrowed_type(borrow, borrow),
            Err(DeclarationError::TypeMismatch)
        );
        assert_eq!(
            d.check_borrowed_view(borrow, borrow),
            Err(DeclarationError::TypeMismatch)
        );
        assert!(d.leaves(ty).is_err());
        assert!(d
            .projection(
                ty,
                &[FieldId {
                    record: RecordId(0),
                    index: 0
                }]
            )
            .is_err());
        let mut records = [record(0, &[hir::Ty::I32], span)];
        records[0].fields[0].ty = ParameterTy::Value(ValueTy::Owned(ty));
        assert_eq!(
            Declarations::check_combined(&records, &raw, &sources).unwrap_err(),
            DeclarationError::NonScalarField(records[0].fields[0].id)
        );
        assert_eq!(
            admit_value_layouts([vec![ValueTy::Owned(ty)].into_iter()].into_iter()),
            Err(DeclarationError::TypeMismatch)
        );
    }
}

#[test]
fn bounded_enum_combined_preflight_precedes_all_retained_reservations() {
    let (sources, span) = source();
    let records = [record(0, &[hir::Ty::Bool], span)];
    let enums = [enumeration(0, span)];
    let table_bytes = size_of::<EnumDeclarations>()
        + size_of::<RecordDecl>()
        + size_of::<FieldDecl>()
        + size_of::<EnumDecl>()
        + size_of::<VariantDecl>();
    let exact = Limits {
        records: 2,
        fields: 2,
        table_bytes,
        layout_bytes: 9,
        ..Limits::DEFAULT
    };
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(4).unwrap();
    let d =
        Declarations::check_combined_with_limits(&records, &enums, &sources, exact, &mut allocator)
            .unwrap();
    assert_eq!(d.usage().table_bytes, table_bytes);
    assert_eq!(d.usage().layout_bytes, 9);
    assert_eq!(d.enums().usage().enums, 1);
    assert_eq!(allocator.attempts, 4);
    assert_eq!(
        allocator
            .trace
            .iter()
            .map(|r| r.length * r.element_bytes)
            .sum::<usize>()
            + size_of::<EnumDeclarations>(),
        table_bytes
    );
    for (limits, resource) in [
        (
            Limits {
                records: 1,
                ..exact
            },
            "aggregate declarations",
        ),
        (Limits { fields: 1, ..exact }, "aggregate members"),
        (
            Limits {
                table_bytes: table_bytes - 1,
                ..exact
            },
            "declaration table bytes",
        ),
        (
            Limits {
                layout_bytes: 8,
                ..exact
            },
            "declaration layout bytes",
        ),
    ] {
        let mut allocator = Allocator {
            fail_at: Some(1),
            ..Allocator::default()
        };
        let error = Declarations::check_combined_with_limits(
            &records,
            &enums,
            &sources,
            limits,
            &mut allocator,
        )
        .unwrap_err();
        assert_eq!(error, DeclarationError::ResourceLimit(resource));
        assert_eq!(allocator.attempts, 0);
    }
    for ordinal in 1..=5 {
        let mut allocator = Allocator {
            fail_at: Some(ordinal),
            ..Allocator::default()
        };
        let result = Declarations::check_combined_with_limits(
            &records,
            &enums,
            &sources,
            exact,
            &mut allocator,
        );
        if ordinal <= 4 {
            assert_eq!(result.unwrap_err(), DeclarationError::Allocation);
            assert_eq!(allocator.attempts, ordinal);
        } else {
            assert!(result.is_ok());
            assert_eq!(allocator.attempts, 4);
        }
    }
}

#[test]
fn bounded_enum_combined_malformed_inputs_reject_before_output() {
    let (sources, span) = source();
    for case in 0..5 {
        let mut records = [record(0, &[hir::Ty::I32], span)];
        let mut enums = [enumeration(0, span)];
        match case {
            0 => records[0].id = RecordId(1),
            1 => enums[0].id = EnumId(1),
            2 => enums[0].variants[0].id.index = 1,
            3 => {
                enums[0].variants[0].payload = Some(ParameterTy::Value(ValueTy::Owned(
                    AggregateTy::Enum(EnumId(0)),
                )))
            }
            _ => {
                records[0].fields[0].ty =
                    ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(0))))
            }
        }
        let mut allocator = Allocator {
            fail_at: Some(1),
            ..Allocator::default()
        };
        let error = Declarations::check_combined_with_limits(
            &records,
            &enums,
            &sources,
            Limits::DEFAULT,
            &mut allocator,
        )
        .unwrap_err();
        assert_ne!(error, DeclarationError::Allocation);
        assert_eq!(allocator.attempts, 0);
    }
}

#[test]
fn bounded_enum_shared_production_count_caps_remain_inclusive() {
    let (sources, span) = source();
    let mut records: Vec<_> = (0..4095).map(|id| record(id, &[], span)).collect();
    let enums = [enumeration(0, span)];
    assert!(Declarations::check_combined(&records, &enums, &sources).is_ok());
    records.push(record(4095, &[], span));
    assert_eq!(
        Declarations::check_combined(&records, &enums, &sources).unwrap_err(),
        DeclarationError::ResourceLimit("aggregate declarations")
    );
    let mut records: Vec<_> = (0..64)
        .map(|id| {
            record(
                id,
                &vec![hir::Ty::I32; if id == 63 { 1023 } else { 1024 }],
                span,
            )
        })
        .collect();
    assert!(Declarations::check_combined(&records, &enums, &sources).is_ok());
    records[63] = record(63, &vec![hir::Ty::I32; 1024], span);
    assert_eq!(
        Declarations::check_combined(&records, &enums, &sources).unwrap_err(),
        DeclarationError::ResourceLimit("aggregate members")
    );
}

#[test]
fn bounded_enum_empty_facade_header_is_charged_without_heap_allocation() {
    let (sources, _) = source();
    let mut allocator = Allocator::default();
    let exact = Limits {
        table_bytes: size_of::<EnumDeclarations>(),
        ..Limits::DEFAULT
    };
    let d = Declarations::check_combined_with_limits(&[], &[], &sources, exact, &mut allocator)
        .unwrap();
    assert_eq!(d.usage().table_bytes, size_of::<EnumDeclarations>());
    assert_eq!(allocator.attempts, 0);
    assert_eq!(
        Declarations::check_combined_with_limits(
            &[],
            &[],
            &sources,
            Limits {
                table_bytes: exact.table_bytes - 1,
                ..exact
            },
            &mut allocator
        )
        .unwrap_err(),
        DeclarationError::ResourceLimit("declaration table bytes")
    );
    assert_eq!(allocator.attempts, 0);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        (
            size_of::<AggregateTy>(),
            size_of::<AggregateSlot>(),
            size_of::<EnumDeclarations>(),
            size_of::<Declarations>()
        ),
        (16, 8, 80, 160)
    );
}

#[test]
fn bounded_enum_combined_production_layout_cap_includes_enum_bytes() {
    let (sources, span) = source();
    let enums = [enumeration(0, span)];
    for final_length in [1021, 1022, 1023] {
        let mut records = [record(0, &vec![hir::Ty::I32; 256], span)];
        for (index, field) in records[0].fields.iter_mut().enumerate() {
            let length = if index == 255 { final_length } else { 1024 };
            field.ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(
                FixedArrayTy::check(hir::Ty::I32, length).unwrap(),
            )));
        }
        let mut allocator = Allocator::default();
        let result = Declarations::check_combined_with_limits(
            &records,
            &enums,
            &sources,
            Limits::DEFAULT,
            &mut allocator,
        );
        if final_length <= 1022 {
            assert_eq!(
                result.unwrap().usage().layout_bytes,
                255 * 4096 + final_length * 4 + 8
            );
        } else {
            assert_eq!(
                result.unwrap_err(),
                DeclarationError::ResourceLimit("declaration layout bytes")
            );
            assert_eq!(allocator.attempts, 0);
        }
    }
}
