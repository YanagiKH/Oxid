//! Independent heap controls; a checked cell address grants no language loan.
use super::*;

fn filled(heap: &mut Heap, element: Element<'_>) -> OwnedVectorDescriptor {
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(heap, &mut vector, element, 2);
    let mut fuel = usize::MAX;
    for number in [41, 42] {
        let (_, mut value) = incoming(heap, element, number);
        heap.owned_vector_push_reserved(&mut vector, element, &mut value, &mut fuel)
            .unwrap();
    }
    vector
}
#[test]
fn owned_record_element_heap_lookup_is_exact_prepaid_noalloc_and_preserves_all_owners() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let mut vector = filled(&mut heap, element);
    let before = heap.ledger();
    let original = vector.encoding();
    let expected =
        heap.owned_vector_control_work(3).unwrap() + heap.owned_vector_work(element, 2, 0).unwrap();
    for ordinal in [0, 1] {
        let mut fuel = expected;
        let mut offset = usize::MAX;
        heap.fault = Some((Boundary::Allocate, 0));
        heap.owned_vector_borrow_record(&vector, element, ordinal, &mut fuel, &mut offset)
            .unwrap();
        assert_eq!(fuel, 0);
        assert_eq!(offset, ordinal as usize * element.size());
        assert_eq!(heap.fault, Some((Boundary::Allocate, 0)));
        assert_eq!(heap.ledger(), before);
        assert_eq!(vector.encoding(), original);
        let bytes = heap
            .owned_vector_cell_bytes(&vector, element, offset, element.size())
            .unwrap();
        assert_eq!(&bytes[24..28], &(41 + ordinal).to_le_bytes());
    }
    heap.fault = None;
    let mut fuel = expected - 1;
    let mut offset = 731;
    assert_eq!(
        heap.owned_vector_borrow_record(&vector, element, 0, &mut fuel, &mut offset),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, expected - 1);
    assert_eq!(offset, 731);
    assert_eq!(heap.ledger(), before);
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert_eq!(heap.ledger().live_allocations, 0);
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}
#[test]
fn owned_record_element_heap_lookup_rejects_nominal_bounds_stale_and_poisoned_prefix_before_publication(
) {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let other = Element::check(&declarations, RecordId(1)).unwrap();
    let mut heap = Heap::default();
    let mut vector = filled(&mut heap, element);
    let before = heap.ledger();
    let mut fuel = usize::MAX;
    for (element, ordinal, expected) in [
        (element, -1, Failure::Bounds),
        (element, 2, Failure::Bounds),
        (other, 0, Failure::Descriptor),
    ] {
        let mut offset = 17;
        assert_eq!(
            heap.owned_vector_borrow_record(&vector, element, ordinal, &mut fuel, &mut offset),
            Err(expected)
        );
        assert_eq!(fuel, usize::MAX);
        assert_eq!(offset, 17);
        assert_eq!(heap.ledger(), before);
    }
    let node = heap.locate_owned_vector(&vector, element).unwrap().unwrap();
    // A bad cell outside the selected ordinal must still be refused. Restore
    // exact canonical bytes before the normal owning cleanup runs.
    let second = unsafe { data(node).add(element.size()) };
    let old_bool = unsafe { *second };
    unsafe {
        *second = 2;
    }
    let mut offset = 17;
    assert_eq!(
        heap.owned_vector_borrow_record(&vector, element, 0, &mut fuel, &mut offset),
        Err(Failure::Descriptor)
    );
    assert_eq!((fuel, offset), (usize::MAX, 17));
    unsafe {
        *second = old_bool;
    }
    let first_child = unsafe { slice::from_raw_parts(data(node).add(8), 16) }.to_vec();
    let second_child = unsafe { slice::from_raw_parts(second.add(8), 16) }.to_vec();
    unsafe {
        ptr::copy_nonoverlapping(first_child.as_ptr(), second.add(8), 16);
    }
    assert_eq!(
        heap.owned_vector_borrow_record(&vector, element, 0, &mut fuel, &mut offset),
        Err(Failure::Descriptor)
    );
    assert_eq!((fuel, offset), (usize::MAX, 17));
    unsafe {
        ptr::copy_nonoverlapping(second_child.as_ptr(), second.add(8), 16);
    }
    let claim = OwnedVectorDescriptor::decode(&vector.encoding()).unwrap();
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert_eq!(
        heap.owned_vector_borrow_record(&claim, element, 0, &mut fuel, &mut offset),
        Err(Failure::Descriptor)
    );
    assert_eq!((fuel, offset), (usize::MAX, 17));
    assert_eq!(heap.ledger().live_allocations, 0);
}
#[test]
fn owned_record_element_heap_lookup_checks_external_controls_and_captured_cell_bounds() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let mut vector = filled(&mut heap, element);
    let before = heap.ledger();
    let node = heap.locate_owned_vector(&vector, element).unwrap().unwrap();
    let live = data(node) as usize;
    let mut fuel = usize::MAX;
    let mut offset = 0;
    assert_eq!(
        heap.owned_vector_disjoint(&[(live, 8)]),
        Err(Failure::Descriptor)
    );
    assert_eq!(
        heap.owned_vector_cell_bytes(&vector, element, 2 * element.size(), 1),
        Err(Failure::Bounds)
    );
    heap.owned_vector_borrow_record(&vector, element, 1, &mut fuel, &mut offset)
        .unwrap();
    assert_eq!(offset, element.size());
    assert_eq!(heap.ledger(), before);
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}
