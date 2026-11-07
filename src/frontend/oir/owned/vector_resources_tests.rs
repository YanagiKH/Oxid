//! Independent storage and transaction oracles for typed vectors.
use super::*;

fn fuel() -> usize {
    1_000_000
}
fn snapshot(heap: &Heap, value: &VectorDescriptor, element: Element) -> ([u8; 16], Vec<u8>) {
    (
        value.encoding(),
        heap.vector_range(value, element, 0, value.length)
            .unwrap()
            .to_vec(),
    )
}

#[test]
fn vector_scalar_widths_use_literal_bit_and_charge_oracles() {
    // Expected widths/charges are literal ABI requirements, not runtime queries.
    let cases: &[(Element, usize, u64, &[u8])] = &[
        (Element::Bool, 1, 64, &[1]),
        (Element::Unit, 1, 64, &[0]),
        (Element::I8, 1, 64, &[0x80]),
        (Element::U8, 1, 64, &[0xff]),
        (Element::I16, 2, 64, &[0, 0x80]),
        (Element::U16, 2, 64, &[0xab, 0xfe]),
        (Element::I32, 4, 80, &[0, 0, 0, 0x80]),
        (Element::U32, 4, 80, &[255; 4]),
        (Element::I64, 8, 112, &[0, 0, 0, 0, 0, 0, 0, 0x80]),
        (Element::U64, 8, 112, &[255; 8]),
        (Element::Isize, 8, 112, &[0, 0, 0, 0, 0, 0, 0, 0x80]),
        (Element::Usize, 8, 112, &[255; 8]),
        (Element::F32, 4, 80, &[0, 0, 0, 0x80]), // negative zero
        (Element::F64, 8, 112, &[1, 0, 0, 0, 0, 0, 0xf8, 0x7f]), // NaN payload
        (Element::Char, 4, 80, &[0xff, 0xff, 0x10, 0]), // U+10FFFF
        (
            Element::I128,
            16,
            176,
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80],
        ),
        (Element::U128, 16, 176, &[255; 16]),
    ];
    for &(element, width, charge, scalar) in cases {
        let mut heap = Heap::default();
        let mut value = VectorDescriptor::empty();
        assert_eq!(
            heap.vector_try_push(&mut value, element, scalar, &mut fuel()),
            Ok(GrowStatus::Ok)
        );
        assert_eq!((value.length, value.capacity), (1, 8));
        assert_eq!(heap.ledger.live_bytes, charge);
        let bytes = heap.vector_range(&value, element, 0, 1).unwrap();
        assert_eq!(bytes, scalar);
        assert_eq!(bytes.len(), width);
        assert_eq!(bytes.as_ptr() as usize % 16, 0);
        let before = heap.ledger;
        heap.limit = charge;
        let mut moved = heap.vector_take(&mut value, element).unwrap();
        assert_eq!(value, VectorDescriptor::empty());
        assert_eq!(heap.ledger.live_bytes, before.live_bytes);
        assert_eq!(heap.ledger.allocations, before.allocations);
        assert_eq!(heap.vector_range(&moved, element, 0, 1).unwrap(), scalar);
        heap.vector_release(&mut moved, element).unwrap();
        assert_eq!(heap.ledger.live_bytes, 0);
        assert_eq!(heap.ledger.allocations, heap.ledger.releases);
    }
}

#[test]
fn vector_float_bit_patterns_are_preserved_without_numeric_conversion() {
    for bits in [
        0u64,
        0x8000_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff8_0000_0000_0042,
        0x7ff0_0000_0000_0001,
    ] {
        let mut heap = Heap::default();
        let mut value = VectorDescriptor::empty();
        let bytes = bits.to_le_bytes();
        assert_eq!(
            heap.vector_try_resize(&mut value, Element::F64, 9, &bytes, &mut fuel()),
            Ok(GrowStatus::Ok)
        );
        for word in heap
            .vector_range(&value, Element::F64, 0, 9)
            .unwrap()
            .as_chunks::<8>()
            .0
        {
            assert_eq!(*word, bytes);
        }
        assert_eq!(
            heap.vector_try_reserve(&mut value, Element::F64, 20, &mut fuel()),
            Ok(GrowStatus::Ok)
        );
        for word in heap
            .vector_range(&value, Element::F64, 0, 9)
            .unwrap()
            .as_chunks::<8>()
            .0
        {
            assert_eq!(*word, bytes);
        }
    }
    for bits in [
        0u32,
        0x8000_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0042,
        0x7f80_0001,
    ] {
        let mut heap = Heap::default();
        let mut value = VectorDescriptor::empty();
        let bytes = bits.to_le_bytes();
        assert_eq!(
            heap.vector_try_push(&mut value, Element::F32, &bytes, &mut fuel()),
            Ok(GrowStatus::Ok)
        );
        assert_eq!(
            heap.vector_range(&value, Element::F32, 0, 1).unwrap(),
            bytes
        );
    }
}

#[test]
fn vector_i128_all_transaction_refusals_preserve_descriptor_and_payload() {
    let scalar = i128::MIN.to_le_bytes();
    for boundary in [Boundary::Allocate, Boundary::Initialize, Boundary::Commit] {
        for initial_length in [0, 8] {
            let mut heap = Heap::default();
            let mut value = VectorDescriptor::empty();
            if initial_length != 0 {
                assert_eq!(
                    heap.vector_try_resize(&mut value, Element::I128, 8, &scalar, &mut fuel()),
                    Ok(GrowStatus::Ok)
                );
            }
            let before = snapshot(&heap, &value, Element::I128);
            let ledger = heap.ledger;
            heap.fault = Some((boundary, 0));
            assert_eq!(
                heap.vector_try_resize(&mut value, Element::I128, 9, &scalar, &mut fuel()),
                Ok(GrowStatus::AllocationFailed)
            );
            assert_eq!(snapshot(&heap, &value, Element::I128), before);
            assert_eq!(heap.ledger.live_bytes, ledger.live_bytes);
            assert_eq!(heap.ledger.live_allocations, ledger.live_allocations);
            assert_eq!(
                heap.ledger.allocations - heap.ledger.releases,
                heap.ledger.live_allocations
            );
            assert!(heap.fault.is_none());
            assert_eq!(
                heap.vector_try_resize(&mut value, Element::I128, 9, &scalar, &mut fuel()),
                Ok(GrowStatus::Ok)
            );
            let bytes = heap.vector_range(&value, Element::I128, 0, 9).unwrap();
            assert_eq!(bytes.as_ptr() as usize % 16, 0);
            for word in bytes.as_chunks::<16>().0 {
                assert_eq!(*word, scalar);
            }
            heap.cleanup();
            assert_eq!(heap.ledger.live_bytes, 0);
            assert_eq!(heap.ledger.allocations, heap.ledger.releases);
        }
    }
}

#[test]
fn vector_in_place_commit_refusal_and_truncate_cannot_resurrect_old_elements() {
    let mut heap = Heap::default();
    let mut value = VectorDescriptor::empty();
    let first = [0x34, 0x12];
    assert_eq!(
        heap.vector_try_resize(&mut value, Element::U16, 3, &first, &mut fuel()),
        Ok(GrowStatus::Ok)
    );
    let before = snapshot(&heap, &value, Element::U16);
    heap.fault = Some((Boundary::Commit, 0));
    assert_eq!(
        heap.vector_try_push(&mut value, Element::U16, &[0xcd, 0xab], &mut fuel()),
        Ok(GrowStatus::AllocationFailed)
    );
    assert_eq!(snapshot(&heap, &value, Element::U16), before);
    heap.fault = Some((Boundary::Commit, 0));
    assert_eq!(
        heap.vector_try_resize(&mut value, Element::U16, 1, &first, &mut fuel()),
        Ok(GrowStatus::AllocationFailed)
    );
    assert_eq!(snapshot(&heap, &value, Element::U16), before);
    heap.vector_truncate(&mut value, Element::U16, 1).unwrap();
    assert_eq!(
        heap.vector_try_resize(&mut value, Element::U16, 3, &[0xef, 0xbe], &mut fuel()),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(
        heap.vector_range(&value, Element::U16, 0, 3).unwrap(),
        [0x34, 0x12, 0xef, 0xbe, 0xef, 0xbe]
    );
}

#[test]
fn vector_peak_charge_and_fuel_boundaries_have_literal_oracles() {
    let mut heap = Heap {
        limit: 479,
        ..Heap::default()
    };
    let mut value = VectorDescriptor::empty();
    let zero = [0u8; 16];
    let mut budget = 144; // 16 bytes validation + eight initialized 16-byte cells.
    assert_eq!(
        heap.vector_try_resize(&mut value, Element::I128, 8, &zero, &mut budget),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(budget, 0);
    assert_eq!(heap.ledger.live_bytes, 176); // 48 + 8*16.
    let before = snapshot(&heap, &value, Element::I128);
    let mut budget = 128; // Growth copies eight cells; no input scratch.
    assert_eq!(
        heap.vector_try_reserve(&mut value, Element::I128, 1, &mut budget),
        Ok(GrowStatus::MemoryLimit)
    );
    assert_eq!(budget, 128);
    assert_eq!(snapshot(&heap, &value, Element::I128), before);
    heap.limit = 480; // Old 176 + new (48 + 16*16) 304.
    let mut short = 127;
    assert_eq!(
        heap.vector_try_reserve(&mut value, Element::I128, 1, &mut short),
        Err(Failure::Fuel)
    );
    assert_eq!(short, 127);
    assert_eq!(snapshot(&heap, &value, Element::I128), before);
    assert_eq!(
        heap.vector_try_reserve(&mut value, Element::I128, 1, &mut budget),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(budget, 0);
    assert_eq!((heap.ledger.live_bytes, heap.ledger.peak_bytes), (304, 480));
    assert_eq!(
        heap.vector_range(&value, Element::I128, 0, 8)
            .unwrap()
            .as_ptr() as usize
            % 16,
        0
    );
}

#[test]
fn vector_rejects_invalid_values_types_classes_and_extents_before_mutation() {
    for (element, invalid) in [
        (Element::Bool, &[2][..]),
        (Element::Unit, &[1][..]),
        (Element::Char, &[0, 0xd8, 0, 0][..]),
        (Element::Char, &[0, 0, 0x11, 0][..]),
    ] {
        let mut heap = Heap::default();
        let mut value = VectorDescriptor::empty();
        let mut budget = 100;
        assert_eq!(
            heap.vector_try_push(&mut value, element, invalid, &mut budget),
            Err(Failure::Descriptor)
        );
        assert_eq!(value, VectorDescriptor::empty());
        assert_eq!(budget, 100);
        assert_eq!(heap.ledger.live_bytes, 0);
    }
    assert_eq!(Element::from_tag(0), None);
    assert_eq!(Element::from_tag(18), None);
    let mut heap = Heap::default();
    let mut value = VectorDescriptor::empty();
    assert_eq!(
        heap.vector_try_push(&mut value, Element::I128, &[0; 16], &mut fuel()),
        Ok(GrowStatus::Ok)
    );
    let before = snapshot(&heap, &value, Element::I128);
    assert_eq!(
        heap.vector_validate(&value, Element::U128),
        Err(Failure::Descriptor)
    );
    assert!(matches!(
        heap.descriptor(&value.encoding()),
        Err(Failure::Descriptor)
    ));
    assert!(matches!(
        heap.file_descriptor(&value.encoding()),
        Err(Failure::Descriptor)
    ));
    let mut bytes = super::super::Descriptor::empty();
    assert_eq!(
        heap.try_push(&mut bytes, 7, &mut fuel()),
        Ok(GrowStatus::Ok)
    );
    assert!(matches!(
        heap.vector_descriptor(&bytes.encoding(), Element::U8),
        Err(Failure::Descriptor)
    ));
    let mut budget = 100;
    assert_eq!(
        heap.vector_try_reserve(&mut value, Element::I128, -1, &mut budget),
        Err(Failure::Bounds)
    );
    assert_eq!(
        heap.vector_try_reserve(&mut value, Element::I128, i32::MAX, &mut budget),
        Ok(GrowStatus::CapacityOverflow)
    );
    assert_eq!(budget, 100);
    assert_eq!(snapshot(&heap, &value, Element::I128), before);
    assert!(matches!(
        heap.vector_range(&value, Element::I128, -1, 1),
        Err(Failure::Bounds)
    ));
    assert!(matches!(
        heap.vector_range(&value, Element::I128, 0, 2),
        Err(Failure::Bounds)
    ));
}

#[test]
fn vector_distinct_source_extend_move_and_registry_cleanup_keep_one_obligation() {
    let mut heap = Heap::default();
    let mut source = VectorDescriptor::empty();
    let mut target = VectorDescriptor::empty();
    assert_eq!(
        heap.vector_try_extend(&mut source, Element::U16, &[1, 0, 2, 0, 3, 0], &mut fuel()),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(
        heap.vector_try_extend_vector(&mut target, Element::U16, &source, 1, 3, &mut fuel()),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(
        heap.vector_range(&target, Element::U16, 0, 2).unwrap(),
        [2, 0, 3, 0]
    );
    let stale = target.encoding();
    let mut destination = VectorDescriptor::empty();
    heap.vector_move(&mut target, Element::U16, &mut destination)
        .unwrap();
    assert_eq!(target, VectorDescriptor::empty());
    assert_eq!(
        heap.vector_move(&mut source, Element::U16, &mut destination),
        Err(Failure::Descriptor)
    );
    heap.vector_release(&mut destination, Element::U16).unwrap();
    assert!(matches!(
        heap.vector_descriptor(&stale, Element::U16),
        Err(Failure::Descriptor)
    ));
    heap.cleanup();
    assert_eq!(
        (heap.ledger.live_allocations, heap.ledger.live_bytes),
        (0, 0)
    );
    assert_eq!(heap.ledger.allocations, heap.ledger.releases);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn vector_fuel_alias_refuses_before_mutation() {
    let mut heap = Heap::default();
    let mut value = VectorDescriptor::empty();
    let scalar = 1000u64.to_le_bytes();
    assert_eq!(
        heap.vector_try_resize(&mut value, Element::U64, 8, &scalar, &mut fuel()),
        Ok(GrowStatus::Ok)
    );
    let before = snapshot(&heap, &value, Element::U64);
    let pointer = unsafe {
        heap.vector_range_mut(&value, Element::U64, 0, 1)
            .unwrap()
            .as_mut_ptr()
    };
    heap.fault = Some((Boundary::Allocate, 0));
    // SAFETY: The allocation is aligned 16 and contains eight initialized bytes.
    // The temporary mutable byte view has ended. No scalar validity restriction
    // applies to usize. The operation must reject this private parameter alias
    // before reading or writing either the fuel word or target payload.
    let status = unsafe {
        heap.vector_try_reserve(&mut value, Element::U64, 1, &mut *pointer.cast::<usize>())
    };
    assert_eq!(status, Err(Failure::Descriptor));
    assert_eq!(snapshot(&heap, &value, Element::U64), before);
    assert_eq!(heap.fault, Some((Boundary::Allocate, 0)));
    assert_eq!(
        (heap.ledger.live_bytes, heap.ledger.live_allocations),
        (112, 1)
    );
}

#[test]
fn vector_identity_exhaustion_refuses_before_allocation_or_fuel_change() {
    let mut heap = Heap {
        next_identity: u64::MAX,
        fault: Some((Boundary::Allocate, 0)),
        ..Heap::default()
    };
    let mut value = VectorDescriptor::empty();
    let mut budget = 100;
    assert_eq!(
        heap.vector_try_reserve(&mut value, Element::U8, 1, &mut budget),
        Err(Failure::IdentityExhausted)
    );
    assert_eq!(value, VectorDescriptor::empty());
    assert_eq!(budget, 100);
    assert_eq!(heap.fault, Some((Boundary::Allocate, 0)));
    assert_eq!(heap.next_identity, u64::MAX);
    assert!(heap.head.is_null());
    assert_eq!(heap.ledger.live_bytes, 0);
}
