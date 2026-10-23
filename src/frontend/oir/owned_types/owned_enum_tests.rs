//! Fixed owned enum layout oracles, independent of source production.
use super::tests::{record, source};
use super::*;

fn enumeration(id: usize, payloads: &[Option<ValueTy>], span: Span) -> RawEnumDecl {
    RawEnumDecl {
        id: EnumId(id),
        span,
        variants: payloads
            .iter()
            .enumerate()
            .map(|(index, payload)| RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(id),
                    index,
                },
                payload: payload.map(ParameterTy::Value),
                span,
            })
            .collect(),
    }
}
fn owned(ty: AggregateTy) -> Option<ValueTy> {
    Some(ValueTy::Owned(ty))
}

#[test]
fn owned_enum_layout_uses_maximum_alternative_and_preserves_empty_sentinels() {
    let (sources, span) = source();
    let records = [
        record(0, &[hir::Ty::I32, hir::Ty::I32], span),
        record(1, &[], span),
    ];
    let arrays =
        [2, 0].map(|n| AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, n).unwrap()));
    let enums = [
        enumeration(
            0,
            &[
                owned(AggregateTy::Record(RecordId(0))),
                Some(ValueTy::Scalar(hir::Ty::I32)),
            ],
            span,
        ),
        enumeration(1, &[owned(AggregateTy::Enum(EnumId(0))), None], span),
        enumeration(
            2,
            &[
                owned(AggregateTy::Record(RecordId(1))),
                owned(arrays[1]),
                Some(ValueTy::Scalar(hir::Ty::Unit)),
            ],
            span,
        ),
        enumeration(3, &[owned(arrays[0]), None], span),
    ];
    let checked = Declarations::check_combined(&records, &enums, &sources).unwrap();
    for (id, size, width) in [(0, 12, 3), (1, 16, 4), (2, 8, 2), (3, 12, 3)] {
        let ty = AggregateTy::Enum(EnumId(id));
        assert_eq!(
            checked.aggregate_layout(ty).unwrap(),
            Layout { size, align: 4 }
        );
        assert_eq!(checked.aggregate_width(ty).unwrap(), width);
    }
    assert_eq!(checked.usage().layout_bytes, 8 + 1 + 12 + 16 + 8 + 12);
    let variant = VariantId {
        enumeration: EnumId(0),
        index: 0,
    };
    let declared = checked.enums().variant(EnumId(0), variant).unwrap();
    assert_eq!(
        declared.value_payload(),
        owned(AggregateTy::Record(RecordId(0)))
    );
    assert_eq!(declared.payload_offset(), Some(4));
    assert!(checked
        .enums()
        .check_payload(EnumId(0), variant, Some(hir::Ty::I32))
        .is_err());
    assert!(checked
        .enums()
        .check_value_payload(EnumId(0), variant, owned(AggregateTy::Record(RecordId(1))))
        .is_err());
}

#[test]
fn owned_enum_cycles_and_invalid_shapes_reject_without_executable_witness() {
    let (sources, span) = source();
    for enums in [
        vec![enumeration(
            0,
            &[None, owned(AggregateTy::Enum(EnumId(0)))],
            span,
        )],
        vec![
            enumeration(0, &[None, owned(AggregateTy::Enum(EnumId(1)))], span),
            enumeration(1, &[owned(AggregateTy::Enum(EnumId(0)))], span),
        ],
    ] {
        assert_eq!(
            Declarations::check_combined(&[], &enums, &sources).unwrap_err(),
            DeclarationError::ResourceLimit("recursive enum layout")
        );
    }
    for ty in [
        AggregateTy::Record(RecordId(0)),
        AggregateTy::Enum(EnumId(99)),
    ] {
        let enums = [enumeration(0, &[owned(ty)], span)];
        assert!(Declarations::check_combined(&[], &enums, &sources).is_err());
    }
    let mut enums = [enumeration(0, &[None], span)];
    enums[0].variants[0].payload = Some(ParameterTy::Reference {
        referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
        kind: BorrowKind::Shared,
    });
    assert_eq!(
        Declarations::check_combined(&[record(0, &[], span)], &enums, &sources).unwrap_err(),
        DeclarationError::NonScalarPayload(enums[0].variants[0].id)
    );
}

#[test]
fn owned_enum_scalar_only_declarations_preserve_reserve_order_and_counts() {
    let (sources, span) = source();
    let scalar = [enumeration(
        0,
        &[None, Some(ValueTy::Scalar(hir::Ty::I32))],
        span,
    )];
    for raw in [&[][..], &scalar[..]] {
        let mut allocator = Allocator::default();
        let checked = Declarations::check_combined_with_limits(
            &[],
            raw,
            &sources,
            Limits::DEFAULT,
            &mut allocator,
        )
        .unwrap();
        assert_eq!(allocator.attempts, if raw.is_empty() { 0 } else { 2 });
        assert_eq!(checked.usage().layout_bytes, 8 * raw.len());
        if !raw.is_empty() {
            assert_eq!(
                allocator
                    .trace
                    .iter()
                    .map(|row| (row.kind, row.length))
                    .collect::<Vec<_>>(),
                [
                    ("checked enum declarations", 1),
                    ("checked enum variants", 2)
                ],
            );
        }
    }
    let mut allocator = Allocator::default();
    let error = Declarations::check_combined_with_limits(
        &[],
        &scalar,
        &sources,
        Limits {
            layout_bytes: 7,
            ..Limits::DEFAULT
        },
        &mut allocator,
    )
    .unwrap_err();
    assert_eq!(
        error,
        DeclarationError::ResourceLimit("declaration layout bytes")
    );
    assert_eq!(allocator.attempts, 0);
}

#[test]
fn owned_enum_scratch_is_admitted_before_reserve_and_every_reserve_can_fail() {
    let (sources, span) = source();
    let raw = [enumeration(
        0,
        &[
            owned(AggregateTy::FixedArray(
                FixedArrayTy::check(hir::Ty::I32, 2).unwrap(),
            )),
            None,
        ],
        span,
    )];
    let checked = Declarations::check_combined(&[], &raw, &sources).unwrap();
    // Four vector headers: the existing record summaries plus enum summaries,
    // state and DFS. One checked enum needs one row/state/stack slot.
    let scratch = 4 * size_of::<Vec<usize>>()
        + size_of::<ContainmentSummary>()
        + size_of::<u8>()
        + size_of::<(usize, usize)>();
    let exact = checked.usage().table_bytes + scratch;
    for less in [0, 1] {
        let mut allocator = Allocator::default();
        let result = Declarations::check_combined_with_limits(
            &[],
            &raw,
            &sources,
            Limits {
                table_bytes: exact - less,
                ..Limits::DEFAULT
            },
            &mut allocator,
        );
        if less == 0 {
            result.unwrap();
            assert_eq!(allocator.attempts, 5);
            assert_eq!(
                allocator
                    .trace
                    .iter()
                    .take(3)
                    .map(|row| (row.kind, row.length, row.element_bytes))
                    .collect::<Vec<_>>(),
                [
                    (
                        "owned enum layout summaries",
                        1,
                        size_of::<ContainmentSummary>()
                    ),
                    ("owned enum layout visit states", 1, size_of::<u8>()),
                    (
                        "owned enum layout work stack",
                        1,
                        size_of::<(usize, usize)>()
                    ),
                ],
            );
        } else {
            assert_eq!(
                result.unwrap_err(),
                DeclarationError::ResourceLimit("owned enum declaration scratch bytes")
            );
            assert_eq!(allocator.attempts, 0);
        }
    }
    for ordinal in 1..=5 {
        let mut allocator = Allocator {
            fail_at: Some(ordinal),
            ..Allocator::default()
        };
        let error = Declarations::check_combined_with_limits(
            &[],
            &raw,
            &sources,
            Limits::DEFAULT,
            &mut allocator,
        )
        .unwrap_err();
        assert_eq!(error, DeclarationError::Allocation);
        assert_eq!(allocator.attempts, ordinal);
        assert!(allocator.trace[..ordinal - 1].iter().all(|row| row.success));
        assert!(!allocator.trace[ordinal - 1].success);
    }
}

#[test]
fn owned_enum_shared_suffix_depth_is_independent_of_dfs_visit_order() {
    let (sources, span) = source();
    for total_depth in [MAX_CONTAINMENT_DEPTH, MAX_CONTAINMENT_DEPTH + 1] {
        for reverse in [false, true] {
            let tail = 24;
            let prefix = total_depth - tail - 1;
            let mut raw = Vec::new();
            // The common tail is already complete before the longer root
            // path is visited. A recursion-stack-only limit would miss it.
            for id in 0..tail {
                let payload =
                    (id + 1 < tail).then(|| ValueTy::Owned(AggregateTy::Enum(EnumId(id + 1))));
                raw.push(enumeration(id, &[payload], span));
            }
            for offset in 0..prefix {
                let id = tail + offset;
                let child = if offset + 1 == prefix { 0 } else { id + 1 };
                raw.push(enumeration(
                    id,
                    &[owned(AggregateTy::Enum(EnumId(child)))],
                    span,
                ));
            }
            let mut edges = [
                owned(AggregateTy::Enum(EnumId(0))),
                owned(AggregateTy::Enum(EnumId(tail))),
            ];
            if reverse {
                edges.reverse();
            }
            raw.push(enumeration(raw.len(), &edges, span));
            let result = Declarations::check_combined(&[], &raw, &sources);
            if total_depth == MAX_CONTAINMENT_DEPTH {
                let checked = result.unwrap();
                assert_eq!(
                    checked
                        .aggregate_width(AggregateTy::Enum(EnumId(raw.len() - 1)))
                        .unwrap(),
                    total_depth + 1
                );
            } else {
                assert_eq!(
                    result.unwrap_err(),
                    DeclarationError::ResourceLimit("enum containment depth")
                );
            }
        }
    }
}

#[test]
fn owned_enum_depth_includes_the_complete_record_payload() {
    let (sources, span) = source();
    for depth in [MAX_CONTAINMENT_DEPTH - 1, MAX_CONTAINMENT_DEPTH] {
        let mut records = Vec::new();
        for id in 0..depth {
            let mut declaration = record(id, &[hir::Ty::I32], span);
            if id + 1 < depth {
                declaration.fields[0].ty =
                    ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(id + 1))));
            }
            records.push(declaration);
        }
        let raw = [enumeration(
            0,
            &[owned(AggregateTy::Record(RecordId(0)))],
            span,
        )];
        let result = Declarations::check_combined(&records, &raw, &sources);
        if depth < MAX_CONTAINMENT_DEPTH {
            let checked = result.unwrap();
            assert_eq!(
                checked
                    .aggregate_layout(AggregateTy::Enum(EnumId(0)))
                    .unwrap(),
                Layout { size: 8, align: 4 }
            );
        } else {
            assert_eq!(
                result.unwrap_err(),
                DeclarationError::ResourceLimit("enum containment depth")
            );
        }
    }
}

#[test]
fn owned_enum_variant_count_refuses_before_new_scratch_or_payload_walk() {
    let (sources, span) = source();
    let raw = [enumeration(
        0,
        &vec![owned(AggregateTy::Record(RecordId(999))); 257],
        span,
    )];
    let mut allocator = Allocator {
        fail_at: Some(1),
        ..Allocator::default()
    };
    let error = Declarations::check_combined_with_limits(
        &[],
        &raw,
        &sources,
        Limits::DEFAULT,
        &mut allocator,
    )
    .unwrap_err();
    assert_eq!(error, DeclarationError::ResourceLimit("variants per enum"));
    assert_eq!(allocator.attempts, 0);
}
