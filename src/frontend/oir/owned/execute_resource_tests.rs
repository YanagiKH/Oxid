use super::super::super::consumer_fixtures as fixture;
use super::*;

fn declarations() -> (Declarations, Span) {
    let (sources, at) = fixture::context();
    let span = at(0);
    let mut inner = fixture::record(&[hir::Ty::Unit, hir::Ty::I32], span);
    inner.fields[0].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::ByteBuf));
    let outer = RawRecordDecl {
        id: RecordId(1),
        span,
        fields: [AggregateTy::Record(RecordId(0)), AggregateTy::ByteBuf]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| RawFieldDecl {
                id: FieldId {
                    record: RecordId(1),
                    index,
                },
                span,
                ty: ParameterTy::Value(ValueTy::Owned(ty)),
            })
            .collect(),
    };
    let enums = [
        AggregateTy::Record(RecordId(1)),
        AggregateTy::Enum(EnumId(0)),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, payload)| RawEnumDecl {
        id: EnumId(id),
        span,
        variants: [
            Some(ValueTy::Owned(payload)),
            Some(ValueTy::Scalar(hir::Ty::Bool)),
            None,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, payload)| RawVariantDecl {
            id: VariantId {
                enumeration: EnumId(id),
                index,
            },
            span,
            payload: payload.map(ParameterTy::Value),
        })
        .collect(),
    })
    .collect::<Vec<_>>();
    (
        Declarations::check_combined(&[inner, outer], &enums, &sources).unwrap(),
        span,
    )
}

fn value(heap: &mut byte_buffer::Heap) -> [u8; 56] {
    let mut bytes = [0xcc; 56];
    bytes[..4].copy_from_slice(&0u32.to_le_bytes());
    bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&17i32.to_le_bytes());
    let mut fuel = 100;
    for (offset, contents) in [(16, &[0, 128, 255][..]), (40, &[3, 2, 1][..])] {
        let mut descriptor = byte_buffer::Descriptor::empty();
        assert_eq!(
            heap.try_extend(&mut descriptor, contents, &mut fuel),
            Ok(byte_buffer::GrowStatus::Ok)
        );
        descriptor.write(&mut bytes[offset..offset + 16]).unwrap();
    }
    bytes
}

#[test]
fn bytebuf_reference_active_transfer_preserves_padding_and_one_obligation() {
    let (declarations, span) = declarations();
    let aggregate = AggregateTy::Enum(EnumId(1));
    for _ in 0..100 {
        let mut heap = byte_buffer::Heap::default();
        let bytes = value(&mut heap);
        let mut target = [0xa5; 56];
        preflight(&declarations, &heap, aggregate, &bytes, target.len(), span).unwrap();
        copy_active_bytes(&declarations, aggregate, &bytes, &mut target, span).unwrap();
        assert_eq!(heap.ledger().live_allocations, 2);
        for (offset, expected) in [(16, &[0, 128, 255][..]), (40, &[3, 2, 1][..])] {
            let descriptor = heap.descriptor(&target[offset..]).unwrap();
            assert_eq!(heap.bytes(&descriptor).unwrap(), expected);
        }
        for range in [4..8, 12..16, 36..40] {
            assert_eq!(&target[range], &[0xa5; 4]);
        }
        assert_eq!(&target[32..36], &17i32.to_le_bytes());
        release(&declarations, &mut heap, aggregate, &target, span).unwrap();
        assert_eq!(heap.ledger().live_allocations, 0);
        assert_eq!(heap.ledger().live_bytes, 0);
        assert_eq!(heap.ledger().releases, 2);
        // The old storage is stale after its logical move. It cannot release
        // either transferred obligation a second time, even under corruption.
        assert!(release(&declarations, &mut heap, aggregate, &bytes, span).is_err());
        assert_eq!(heap.ledger().releases, 2);
    }
}

#[test]
fn bytebuf_reference_release_ignores_inactive_payload_and_preflights_all_leaves() {
    let (declarations, span) = declarations();
    let aggregate = AggregateTy::Enum(EnumId(1));
    let mut heap = byte_buffer::Heap::default();
    let original = value(&mut heap);
    let mut malformed = original;
    malformed[48..52].copy_from_slice(&9i32.to_le_bytes());
    assert!(release(&declarations, &mut heap, aggregate, &malformed, span).is_err());
    assert_eq!(heap.ledger().live_allocations, 2);
    assert_eq!(heap.ledger().releases, 0);
    let mut inactive = [0xff; 56];
    inactive[..4].copy_from_slice(&0u32.to_le_bytes());
    inactive[8..12].copy_from_slice(&1u32.to_le_bytes());
    inactive[16] = 1;
    release(&declarations, &mut heap, aggregate, &inactive, span).unwrap();
    assert_eq!(heap.ledger().releases, 0);
    heap.cleanup();
    assert_eq!(heap.ledger().live_allocations, 0);
    assert_eq!(heap.ledger().releases, 2);
}

#[test]
fn bytebuf_reference_scalar_alternative_uses_checked_align8_payload_offset() {
    let (declarations, span) = declarations();
    let variant = declarations.enums().variant_for_tag(EnumId(0), 1).unwrap();
    let value = EnumValue {
        tag: 1,
        payload: Some(Scalar::Bool(true)),
        payload_offset: variant.payload_offset().unwrap() as u8,
    };
    let mut bytes = [0xcc; 48];
    encode_enum(&mut bytes, 0..48, value, span).unwrap();
    assert_eq!(&bytes[..4], &1u32.to_le_bytes());
    assert_eq!(bytes[8], 1);
    assert_eq!(&bytes[4..8], &[0xcc; 4]);
    assert!(bytes[9..].iter().all(|byte| *byte == 0xcc));
    assert!(std::mem::size_of::<EnumValue>() <= 16);
}
