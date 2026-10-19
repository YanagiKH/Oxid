//! Fixed R2 layout oracles, independent of source production.
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
fn r2_owned_enum_layout_uses_maximum_alternative_and_preserves_empty_sentinels() {
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
fn r2_owned_enum_cycles_and_invalid_shapes_reject_without_executable_witness() {
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
