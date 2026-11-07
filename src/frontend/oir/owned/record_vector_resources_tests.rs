//! Independent nominal, transaction, canonical-byte and physical storage oracles.
use super::*;
use crate::frontend::{
    hir,
    oir::{
        owned::consumer_fixtures as fixture,
        owned_types::{
            AggregateTy, Declarations, FieldId, ParameterTy, RawRecordDecl, RecordId, ValueTy,
        },
    },
};

const P: [u8; 10] = [1, 21, 0, 0, 0, 255, 255, 16, 0, 0];
fn record(id: usize, types: &[hir::Ty], span: crate::frontend::source::Span) -> RawRecordDecl {
    let mut record = fixture::record(types, span);
    record.id = RecordId(id);
    record.is_copy = true;
    for field in &mut record.fields {
        field.id.record = record.id;
    }
    record
}
fn declarations() -> Declarations {
    let (sources, span) = fixture::context();
    Declarations::check(
        &[
            record(
                0,
                &[hir::Ty::Bool, hir::Ty::I32, hir::Ty::Char, hir::Ty::Unit],
                span(0),
            ),
            record(
                1,
                &[hir::Ty::Bool, hir::Ty::I32, hir::Ty::Char, hir::Ty::Unit],
                span(1),
            ),
            record(2, &[], span(2)),
        ],
        &sources,
    )
    .unwrap()
}
fn snapshot(
    heap: &Heap,
    value: &VectorDescriptor,
    element: RecordElement<'_>,
) -> ([u8; 16], Vec<u8>) {
    (
        value.encoding(),
        heap.vector_range(value, element, 0, value.length)
            .unwrap()
            .to_vec(),
    )
}
#[test]
fn record_vectors_use_dense_initialized_leaves_and_exact_nominal_identity() {
    let declarations = declarations();
    assert_eq!(
        declarations
            .fields(RecordId(0))
            .unwrap()
            .iter()
            .map(|field| field.offset())
            .collect::<Vec<_>>(),
        [0, 4, 8, 12]
    );
    assert_eq!(
        declarations
            .aggregate_layout(AggregateTy::Record(RecordId(0)))
            .unwrap()
            .size(),
        16
    );
    let element = RecordElement::check(&declarations, RecordId(0)).unwrap();
    assert_eq!((element.tag(), element.size()), (0x8000_0000, 10));
    let other = RecordElement::check(&declarations, RecordId(1)).unwrap();
    assert_eq!((other.tag(), other.size()), (0x8000_0001, 10));
    let mut heap = Heap::default();
    let mut value = VectorDescriptor::empty();
    let mut fuel = 1320; // 131*10 validation plus10 initialized bytes.
    assert_eq!(
        heap.vector_try_push(&mut value, element, &P, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(fuel, 0);
    assert_eq!(value.encoding().len(), 16);
    assert_eq!(heap.ledger.live_bytes, 128); // align16(48+8*10)
    assert_eq!(heap.vector_range(&value, element, 0, 1).unwrap(), P);
    assert_eq!(
        heap.vector_validate(&value, other),
        Err(Failure::Descriptor)
    );
    assert_eq!(
        heap.vector_validate(&value, Element::U8),
        Err(Failure::Descriptor)
    );
    heap.vector_release(&mut value, element).unwrap();
    assert_eq!(
        (heap.ledger.live_bytes, heap.ledger.live_allocations),
        (0, 0)
    );
}
#[test]
fn record_vectors_support_all_nine_operations_without_reading_owner_padding() {
    let declarations = declarations();
    let element = RecordElement::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let mut value = VectorDescriptor::empty();
    let mut source = VectorDescriptor::empty();
    let mut fuel = 1_000_000;
    assert_eq!(heap.vector_len(&value, element), Ok(0));
    assert_eq!(heap.vector_capacity(&value, element), Ok(0));
    assert_eq!(
        heap.vector_try_reserve(&mut value, element, 2, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(heap.vector_len(&value, element), Ok(0));
    assert_eq!(heap.vector_capacity(&value, element), Ok(8));
    assert_eq!(
        heap.vector_try_push(&mut value, element, &P, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(
        heap.vector_try_resize(&mut source, element, 3, &P, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(
        heap.vector_try_extend_vector(&mut value, element, &source, 1, 3, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(heap.vector_len(&value, element), Ok(3));
    assert_eq!(
        heap.vector_range(&value, element, 0, 3).unwrap(),
        P.repeat(3)
    );
    assert_eq!(
        heap.vector_try_resize(&mut value, element, 9, &P, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    heap.vector_truncate(&mut value, element, 2).unwrap();
    let mut moved = heap.vector_take(&mut value, element).unwrap();
    assert_eq!(value, VectorDescriptor::empty());
    assert_eq!(
        heap.vector_range(&moved, element, 0, 2).unwrap(),
        P.repeat(2)
    );
    heap.vector_release(&mut moved, element).unwrap();
    heap.vector_release(&mut source, element).unwrap();
    assert_eq!(heap.ledger.live_bytes, 0);
    assert_eq!(heap.ledger.allocations, heap.ledger.releases);
}
#[test]
fn record_vector_refusals_preserve_values_descriptor_and_live_obligations() {
    let declarations = declarations();
    let element = RecordElement::check(&declarations, RecordId(0)).unwrap();
    for boundary in [Boundary::Allocate, Boundary::Initialize, Boundary::Commit] {
        for initial in [0, 8] {
            let mut heap = Heap::default();
            let mut value = VectorDescriptor::empty();
            let mut fuel = 1_000_000;
            assert_eq!(
                heap.vector_try_resize(&mut value, element, initial, &P, &mut fuel),
                Ok(GrowStatus::Ok)
            );
            let before = snapshot(&heap, &value, element);
            let live = (heap.ledger.live_bytes, heap.ledger.live_allocations);
            heap.fault = Some((boundary, 0));
            assert_eq!(
                heap.vector_try_resize(&mut value, element, 9, &P, &mut fuel),
                Ok(GrowStatus::AllocationFailed)
            );
            assert_eq!(snapshot(&heap, &value, element), before);
            assert_eq!((heap.ledger.live_bytes, heap.ledger.live_allocations), live);
            assert_eq!(
                heap.ledger.allocations - heap.ledger.releases,
                heap.ledger.live_allocations
            );
            assert_eq!(
                heap.vector_try_resize(&mut value, element, 9, &P, &mut fuel),
                Ok(GrowStatus::Ok)
            );
            heap.cleanup();
            assert_eq!(heap.ledger.allocations, heap.ledger.releases);
        }
    }
}
#[test]
fn record_vector_canonical_and_same_width_nominal_refusals_precede_mutation() {
    let declarations = declarations();
    let element = RecordElement::check(&declarations, RecordId(0)).unwrap();
    let mut cases = Vec::new();
    for (offset, word) in [
        (0, &[2][..]),
        (9, &[1][..]),
        (5, &[0, 216, 0, 0][..]),
        (5, &[0, 0, 17, 0][..]),
    ] {
        let mut invalid = P;
        invalid[offset..offset + word.len()].copy_from_slice(word);
        cases.push(invalid);
    }
    for invalid in cases {
        let mut heap = Heap::default();
        let mut value = VectorDescriptor::empty();
        let mut fuel = 1_000_000;
        assert_eq!(
            heap.vector_try_push(&mut value, element, &P, &mut fuel),
            Ok(GrowStatus::Ok)
        );
        let before = snapshot(&heap, &value, element);
        let live = heap.ledger;
        let remaining = fuel;
        assert_eq!(
            heap.vector_try_push(&mut value, element, &invalid, &mut fuel),
            Err(Failure::Descriptor)
        );
        assert_eq!(snapshot(&heap, &value, element), before);
        assert_eq!(heap.ledger, live);
        assert_eq!(fuel, remaining);
    }
    assert!(RecordElement::check(&declarations, RecordId(4096)).is_err());
    let (sources, at) = fixture::context();
    let affine = fixture::record(&[hir::Ty::I32], at(0));
    let affine = Declarations::check(&[affine], &sources).unwrap();
    assert!(RecordElement::check(&affine, RecordId(0)).is_err());
}
#[test]
fn record_vector_dense_heap_peak_and_fuel_limits_have_literal_oracles() {
    let declarations = declarations();
    let element = RecordElement::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap {
        limit: 335,
        ..Heap::default()
    };
    let mut value = VectorDescriptor::empty();
    let mut fuel = 1390; // 131*10+8*10.
    assert_eq!(
        heap.vector_try_resize(&mut value, element, 8, &P, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!((fuel, heap.ledger.live_bytes), (0, 128));
    let before = snapshot(&heap, &value, element);
    let mut fuel = 80;
    assert_eq!(
        heap.vector_try_reserve(&mut value, element, 1, &mut fuel),
        Ok(GrowStatus::MemoryLimit)
    );
    assert_eq!(fuel, 80);
    assert_eq!(snapshot(&heap, &value, element), before);
    heap.limit = 336; // old128 + align16(48+16*10)=208.
    let mut short = 79;
    assert_eq!(
        heap.vector_try_reserve(&mut value, element, 1, &mut short),
        Err(Failure::Fuel)
    );
    assert_eq!(short, 79);
    assert_eq!(snapshot(&heap, &value, element), before);
    assert_eq!(
        heap.vector_try_reserve(&mut value, element, 1, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(
        (fuel, heap.ledger.live_bytes, heap.ledger.peak_bytes),
        (0, 208, 336)
    );
}
#[test]
fn empty_record_elements_have_a_positive_initialized_unit_sentinel() {
    let declarations = declarations();
    let element = RecordElement::check(&declarations, RecordId(2)).unwrap();
    assert_eq!(element.size(), 1);
    let mut heap = Heap::default();
    let mut value = VectorDescriptor::empty();
    let mut fuel = 132;
    assert_eq!(
        heap.vector_try_push(&mut value, element, &[0], &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!((fuel, heap.ledger.live_bytes), (0, 64));
    assert_eq!(heap.vector_range(&value, element, 0, 1).unwrap(), [0]);
    assert_eq!(
        heap.vector_try_push(&mut value, element, &[1], &mut 1000),
        Err(Failure::Descriptor)
    );
}
#[test]
fn record_validation_work_covers_deep_unary_records_and_more_than_sixty_four_leaves() {
    let (sources, at) = fixture::context();
    let mut deep = vec![record(0, &[hir::Ty::Unit], at(0))];
    for id in 1..64 {
        let mut next = record(id, &[hir::Ty::Unit], at(id));
        next.fields[0].ty =
            ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(id - 1))));
        deep.push(next);
    }
    let deep = Declarations::check(&deep, &sources).unwrap();
    let element = RecordElement::check(&deep, RecordId(63)).unwrap();
    assert_eq!(element.size(), 1);
    let mut heap = Heap::default();
    let mut value = VectorDescriptor::empty();
    let mut short = 131;
    assert_eq!(
        heap.vector_try_push(&mut value, element, &[0], &mut short),
        Err(Failure::Fuel)
    );
    assert_eq!(value, VectorDescriptor::empty());
    assert_eq!(
        heap.vector_try_push(&mut value, element, &[0], &mut 132),
        Ok(GrowStatus::Ok)
    );
    let wide = Declarations::check(&[record(0, &[hir::Ty::Bool; 80], at(0))], &sources).unwrap();
    let element = RecordElement::check(&wide, RecordId(0)).unwrap();
    assert_eq!(element.size(), 80);
    let mut value = VectorDescriptor::empty();
    let mut short = 10_559; // 80*131+80-1, independent of a production subtotal.
    assert_eq!(
        heap.vector_try_push(&mut value, element, &[1; 80], &mut short),
        Err(Failure::Fuel)
    );
    let mut exact = 10_560;
    assert_eq!(
        heap.vector_try_push(&mut value, element, &[1; 80], &mut exact),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(exact, 0);
}
#[test]
fn record_vector_compact_carriers_retain_descriptor_and_checked_table_sizes() {
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(std::mem::size_of::<AggregateTy>(), 16);
        assert_eq!(
            std::mem::size_of::<crate::frontend::oir::owned_types::BorrowedTy>(),
            16
        );
        assert_eq!(
            std::mem::size_of::<crate::frontend::oir::owned_types::RecordDecl>(),
            80
        );
        assert_eq!(
            std::mem::size_of::<crate::frontend::builtin_catalog::vector::Function>(),
            2
        );
        assert_eq!(std::mem::size_of::<VectorDescriptor>(), 16);
        assert_eq!(std::mem::size_of::<VectorHeader>(), 48);
        assert_eq!(std::mem::align_of::<VectorHeader>(), 16);
        assert_eq!(
            std::mem::size_of::<crate::frontend::oir::owned_types::ResourceLeaf>(),
            16
        );
    }
    let _ = FieldId {
        record: RecordId(0),
        index: 0,
    };
}
