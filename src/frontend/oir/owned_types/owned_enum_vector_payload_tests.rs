use super::super::AggregateTy;
use super::*;

#[test]
fn owned_enum_vector_payload_slots_append_tags_without_changing_storage() {
    assert_eq!(
        (
            std::mem::size_of::<PayloadSlot>(),
            std::mem::align_of::<PayloadSlot>()
        ),
        (8, 4)
    );
    for ordinal in [0, 1, 4095, u32::MAX as usize] {
        for alignment in [8, 16] {
            let aggregate = AggregateTy::OwnedEnumVector(EnumId(ordinal));
            let payload =
                PayloadSlot::check_domain::<true>(ValueTy::Owned(aggregate), alignment).unwrap();
            assert_eq!(payload.value(), ValueTy::Owned(aggregate));
            assert_eq!(payload.offset(), alignment);
            assert!(
                PayloadSlot::check_domain::<false>(ValueTy::Owned(aggregate), alignment).is_err()
            );
        }
    }
}
