use super::*;
use std::mem::{align_of, size_of};

#[test]
fn owned_enum_vector_identity_slots_keep_every_legacy_discriminant_and_width() {
    assert_eq!(
        (size_of::<OwnedVectorRoot>(), align_of::<OwnedVectorRoot>()),
        (16, 8)
    );
    assert_eq!(
        (
            size_of::<OwnedVectorRootSlot>(),
            align_of::<OwnedVectorRootSlot>()
        ),
        (4, 4)
    );
    assert_eq!(
        (size_of::<AggregateTy>(), align_of::<AggregateTy>()),
        (16, 8)
    );
    assert_eq!((size_of::<ValueTy>(), align_of::<ValueTy>()), (16, 8));
    assert_eq!(
        (size_of::<ParameterTy>(), align_of::<ParameterTy>()),
        (24, 8)
    );
    assert_eq!(
        (size_of::<AggregateSlot>(), align_of::<AggregateSlot>()),
        (8, 8)
    );
    assert_eq!(
        (size_of::<BorrowedSlot>(), align_of::<BorrowedSlot>()),
        (8, 8)
    );
    assert_eq!(
        (size_of::<ResourceKind>(), align_of::<ResourceKind>()),
        (8, 4)
    );
    for ordinal in 0..4096 {
        let record = OwnedVectorRoot::Record(RecordId(ordinal));
        let enumeration = OwnedVectorRoot::Enum(EnumId(ordinal));
        for root in [record, enumeration] {
            let slot = OwnedVectorRootSlot::check(root).unwrap();
            assert_eq!(
                slot.raw(),
                ordinal as u32 | if root == enumeration { 0x8000_0000 } else { 0 }
            );
            assert_eq!(OwnedVectorRootSlot::from_raw(slot.raw()), Some(slot));
            assert_eq!(slot.root(), root);
        }
    }
    for invalid in [4096, 0x7fff_ffff, 0x8000_1000, u32::MAX] {
        assert!(OwnedVectorRootSlot::from_raw(invalid).is_none());
    }
    for invalid in [4096, usize::MAX] {
        assert!(OwnedVectorRootSlot::check(OwnedVectorRoot::Record(RecordId(invalid))).is_none());
        assert!(OwnedVectorRootSlot::check(OwnedVectorRoot::Enum(EnumId(invalid))).is_none());
    }
    for ordinal in [0, 1, 4095, u32::MAX as usize] {
        let aggregate = AggregateTy::OwnedEnumVector(EnumId(ordinal));
        let slot = AggregateSlot::try_from_aggregate(aggregate).unwrap();
        let borrowed = BorrowedSlot::check(BorrowedTy::Exact(aggregate)).unwrap();
        assert_eq!(slot.aggregate(), aggregate);
        assert_eq!(borrowed.referent(), BorrowedTy::Exact(aggregate));
        // SAFETY: Read only initialized repr(C) discriminants, never padding.
        let aggregate_tag = unsafe {
            std::ptr::read_unaligned((&slot.0 as *const AggregateSlotRepr).cast::<u32>())
        };
        let borrowed_tag = unsafe {
            std::ptr::read_unaligned((&borrowed.0 as *const BorrowedSlotRepr).cast::<u32>())
        };
        assert_eq!((aggregate_tag, borrowed_tag), (8, 9));
        assert_ne!(
            slot,
            AggregateSlot::try_from_aggregate(AggregateTy::OwnedRecordVector(RecordId(ordinal)))
                .unwrap()
        );
        assert_eq!(BorrowedTy::Exact(aggregate).element(), None);
        assert!(!BorrowedTy::RecordSlice(RecordId(ordinal)).accepts(BorrowedTy::Exact(aggregate)));
    }
    #[cfg(target_pointer_width = "64")]
    {
        let invalid = AggregateTy::OwnedEnumVector(EnumId(u32::MAX as usize + 1));
        assert!(AggregateSlot::try_from_aggregate(invalid).is_err());
        assert!(BorrowedSlot::check(BorrowedTy::Exact(invalid)).is_err());
    }
}
