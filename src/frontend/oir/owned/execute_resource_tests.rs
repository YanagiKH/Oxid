use super::super::super::consumer_fixtures as fixture;
use super::*;

#[test]
fn record_vector_resource_checker_has_distinct_raw_mapped_and_selected_roles() {
    use crate::frontend::oir::owned::vector_type::{self, VectorTy};
    use std::mem::{align_of, offset_of, size_of_val};

    type Checked = byte_buffer::CheckedVectorElement<'static>;
    type Raw = std::result::Result<Checked, byte_buffer::Failure>;
    type Mapped = Result<Checked>;
    // Independent caller composition. The selected Checked witness is already
    // a separate field of the inherited bank and is deliberately absent here.
    #[allow(dead_code)]
    #[repr(C)]
    struct CheckedCaller {
        selected_type: VectorTy,
        raw: Raw,
        mapped: Mapped,
    }
    let _: for<'a> fn(&'a ReferenceWalkCarriers) -> &'a VectorTy = |model| &model.vector_type;
    let _: for<'a> fn(&'a ReferenceWalkCarriers) -> &'a Raw = |model| &model.vector_element_return;
    let _: for<'a> fn(&'a ReferenceWalkCarriers) -> &'a Mapped =
        |model| &model.mapped_vector_element_return;
    let _: for<'a> fn(&'a ReferenceWalkCarriers) -> &'a Checked = |model| &model.vector_element;
    let _: for<'a> fn(
        VectorTy,
        &'a Declarations,
    ) -> std::result::Result<
        byte_buffer::CheckedVectorElement<'a>,
        byte_buffer::Failure,
    > = vector_type::checked;

    let (_, at) = fixture::context();
    let span = at(0);
    let mapper = vector_type_error_mapper(&span);
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!((size_of::<VectorTy>(), align_of::<VectorTy>()), (16, 8));
        assert_eq!((size_of::<Checked>(), align_of::<Checked>()), (24, 8));
        assert_eq!((size_of::<Raw>(), align_of::<Raw>()), (32, 8));
        assert_eq!((size_of::<Mapped>(), align_of::<Mapped>()), (72, 8));
        assert_eq!(
            (
                offset_of!(CheckedCaller, selected_type),
                offset_of!(CheckedCaller, raw),
                offset_of!(CheckedCaller, mapped),
                size_of::<CheckedCaller>(),
                align_of::<CheckedCaller>(),
            ),
            (0, 16, 48, 120, 8)
        );
        assert_eq!(size_of_val(&mapper), size_of::<&Span>());
        assert_eq!(size_of_val(&mapper), 8);
        assert_eq!(size_of::<CheckedCaller>() + size_of_val(&mapper), 128);
        let roles = [
            (offset_of!(ReferenceWalkCarriers, vector_type), 16),
            (offset_of!(ReferenceWalkCarriers, vector_element_return), 32),
            (
                offset_of!(ReferenceWalkCarriers, mapped_vector_element_return),
                72,
            ),
            (offset_of!(ReferenceWalkCarriers, vector_element), 24),
        ];
        for (index, &(offset, width)) in roles.iter().enumerate() {
            assert_eq!(offset % 8, 0);
            assert!(offset + width <= size_of::<ReferenceWalkCarriers>());
            for &(other, other_width) in &roles[index + 1..] {
                assert!(offset + width <= other || other + other_width <= offset);
            }
        }
    }
    assert_eq!(
        mapper(byte_buffer::Failure::Descriptor),
        bad("record vector type", span)
    );
}

#[test]
fn record_vector_resource_checker_is_paid_without_vector_operation_frames_before_release() {
    use crate::frontend::builtin_catalog::experimental::Function;
    let checked = crate::frontend::oir::owned::source::resource_fixtures::checked_enum_project(
        "use alloc::bytes::new;use alloc::bytes::try_push;fn main()->i32{let n=7;let byte=n.to_u8_checked();let mut bytes=new();try_push(&mut bytes,byte);return 42;}",
    );
    let plan = ExecutionPlan::build(&checked.witness).unwrap();
    assert!(plan.uses_dynamic_resources());
    assert!(!plan.uses_vectors());
    assert!(!plan.uses_record_vectors());
    assert!(!plan.uses_typed_ranges());
    assert!(!plan.uses_enum_record_nesting());
    assert!(!plan.uses_wide_scalars());
    assert!(!plan.uses_integer_extension());
    assert!(!plan.uses_bitwise());
    assert!(!plan.uses_characters());
    assert!(!plan.uses_floats());
    assert!(!plan.has_binary_delivery());
    assert!(!plan.has_file_delivery());
    assert!(!plan.has_program_arguments());
    let new = checked
        .witness
        .experimental_function(Function::New)
        .unwrap();
    let push = checked
        .witness
        .experimental_function(Function::Push)
        .unwrap();
    let entry = &checked.witness.functions()[checked.entry.0];
    let origin = entry
        .blocks
        .iter()
        .find_map(|block| {
            let end = block.terminator.as_ref()?;
            match &end.kind {
                OwnedTerminatorKind::Invoke { call, .. } if entry.calls[call.0].target == push => {
                    Some(end.span)
                }
                _ => None,
            }
        })
        .unwrap();
    let main_bytes = plan.function(checked.entry).usage().reference_bytes;
    let new_bytes = plan.function(new).usage().reference_bytes;
    let push_bytes = plan.function(push).usage().reference_bytes;
    assert!(push_bytes > new_bytes);
    // The unchanged frame plans are inputs to this routing boundary. Only main
    // and one primitive can coexist; the new checker bank is fixed once, not
    // per frame/leaf, and no vector-operation control bank may subsidize it.
    let exact = 2 * size_of::<Frame>()
        + size_of::<byte_buffer::Heap>()
        + control_bytes()
        + size_of::<Scalar>()
        + main_bytes
        + push_bytes;
    assert_eq!(values::control_bytes::<Scalar>(), 0);
    assert!(exact < plan::MAX_REFERENCE_BYTES);
    assert_eq!(plan::MAX_REFERENCE_BYTES, 16 * 1024 * 1024);
    for fault in [None, Some((byte_buffer::Boundary::Allocate, 0))] {
        for admitted in [false, true] {
            let (result, observed) = byte_buffer::testing::capture(
                byte_buffer::testing::Control {
                    fault,
                    ..Default::default()
                },
                || {
                    run_limits(
                        &checked.witness,
                        Some(checked.entry),
                        Limits {
                            frames: 2,
                            bytes: exact - usize::from(!admitted),
                            ..Default::default()
                        },
                    )
                },
            );
            if admitted {
                assert_eq!(result, Ok(Scalar::I32(42)));
                assert_eq!(observed.fault_applied, fault.is_some());
                let successful_allocations = u64::from(fault.is_none());
                assert_eq!(observed.before_cleanup.allocations, successful_allocations);
                assert_eq!(observed.before_cleanup.releases, successful_allocations);
            } else {
                assert_eq!(
                    result,
                    Err(OwnedRunFailure::Resource(plan::AdmissionFailure::new(
                        "live requested bytes",
                        Some(origin),
                    )))
                );
                assert!(!observed.fault_applied);
                assert_eq!(observed.before_cleanup.allocations, 0);
                assert_eq!(observed.before_cleanup.releases, 0);
            }
            assert_eq!(observed.before_cleanup.live_allocations, 0);
            assert_eq!(observed.after_cleanup.live_bytes, 0);
        }
    }
}

fn declarations() -> (Declarations, Span) {
    let (sources, at) = fixture::context();
    let span = at(0);
    let mut inner = fixture::record(&[hir::Ty::Unit, hir::Ty::I32], span);
    inner.fields[0].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::ByteBuf));
    let outer = RawRecordDecl {
        is_copy: false,
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
