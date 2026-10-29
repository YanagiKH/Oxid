//! Heap/codec controls for owned records. Public source remains separately
//! gated until the checked operation sidecars and both consumers are complete.
use super::*;
use crate::frontend::hir;
use crate::frontend::oir::owned::owned_record_codec::bounds::{CheckedElements, ShapeFailure};
use crate::frontend::oir::owned::{
    byte_buffer::{Descriptor, VectorDescriptor, VectorElement},
    AggregateTy, Declarations, EnumId, FieldId, ParameterTy, RawEnumDecl, RawFieldDecl,
    RawRecordDecl, RawVariantDecl, RecordId, SourceMap, ValueTy, VariantId,
};
use crate::frontend::project::budget::Allocator;

fn declarations() -> Declarations {
    let mut sources = SourceMap::new();
    let file = sources.add("owned-vector-controls.ox".into(), "record control".into());
    let span = sources.get(file).span(0, 6);
    let fields = [
        ValueTy::Scalar(hir::Ty::Bool),
        ValueTy::Owned(AggregateTy::ByteBuf),
        ValueTy::Scalar(hir::Ty::I32),
    ];
    let records = (0..2)
        .map(|record| RawRecordDecl {
            is_copy: false,
            id: RecordId(record),
            span,
            fields: fields
                .iter()
                .enumerate()
                .map(|(index, ty)| RawFieldDecl {
                    id: FieldId {
                        record: RecordId(record),
                        index,
                    },
                    ty: ParameterTy::Value(*ty),
                    span,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    Declarations::check(&records, &sources).unwrap()
}

#[test]
fn owned_record_vector_paid_resource_validation_matches_native_r2_and_failed_claims() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let other = Element::check(&declarations, RecordId(1)).unwrap();
    let mut heap = Heap::default();
    let mut empty_fuel = 3;
    assert_eq!(
        heap.owned_vector_validate(&OwnedVectorDescriptor::empty(), element, &mut empty_fuel),
        Err(Failure::Fuel)
    );
    assert_eq!(empty_fuel, 3);
    empty_fuel = 4;
    assert_eq!(
        heap.owned_vector_validate(&OwnedVectorDescriptor::empty(), element, &mut empty_fuel),
        Ok(())
    );
    assert_eq!(empty_fuel, 0);
    let (_child, _source) = incoming(&mut heap, element, 41);
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut vector, element, 1);
    assert_eq!(heap.ledger().live_allocations, 2);
    let encoded = vector.encoding();
    let claim = OwnedVectorDescriptor::decode(&encoded).unwrap();
    let ledger = heap.ledger();
    let mut fuel = 9;
    assert_eq!(
        heap.owned_vector_validate(&claim, element, &mut fuel),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, 9);
    assert_eq!(heap.ledger(), ledger);
    assert_eq!(vector.encoding(), encoded);
    fuel = 10;
    assert_eq!(
        heap.owned_vector_validate(&claim, element, &mut fuel),
        Ok(())
    );
    assert_eq!(fuel, 0);
    fuel = 10;
    assert_eq!(
        heap.owned_vector_validate(&claim, other, &mut fuel),
        Err(Failure::Descriptor)
    );
    assert_eq!(fuel, 10);
    let mut poisoned = encoded;
    poisoned[8..12].copy_from_slice(&(-1i32).to_le_bytes());
    let poisoned = OwnedVectorDescriptor::decode(&poisoned).unwrap();
    assert_eq!(
        heap.owned_vector_validate(&poisoned, element, &mut fuel),
        Err(Failure::Descriptor)
    );
    assert_eq!(fuel, 10);
    assert_eq!(heap.ledger(), ledger);
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert_eq!(
        heap.owned_vector_validate(&claim, element, &mut fuel),
        Err(Failure::Descriptor)
    );
    assert_eq!(fuel, 10);
}

#[test]
fn owned_record_vector_output_queries_match_native_exact_and_one_less_work() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let other = Element::check(&declarations, RecordId(1)).unwrap();
    let mut heap = Heap::default();
    let (_child, _source) = incoming(&mut heap, element, 41);
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut vector, element, 1);
    let mut output = -17;
    let mut fuel = 18;
    assert_eq!(
        heap.owned_vector_len_into(&vector, element, &mut fuel, &mut output),
        Err(Failure::Fuel)
    );
    assert_eq!((fuel, output), (18, -17));
    fuel = 19;
    assert_eq!(
        heap.owned_vector_len_into(&vector, element, &mut fuel, &mut output),
        Ok(())
    );
    assert_eq!((fuel, output), (0, 0));
    fuel = 18;
    assert_eq!(
        heap.owned_vector_capacity_into(&vector, element, &mut fuel, &mut output),
        Err(Failure::Fuel)
    );
    assert_eq!((fuel, output), (18, 0));
    fuel = 19;
    assert_eq!(
        heap.owned_vector_capacity_into(&vector, element, &mut fuel, &mut output),
        Ok(())
    );
    assert_eq!((fuel, output), (0, 8));
    fuel = 19;
    assert_eq!(
        heap.owned_vector_len_into(&vector, other, &mut fuel, &mut output),
        Err(Failure::Descriptor)
    );
    assert_eq!((fuel, output), (19, 8));
    assert_eq!(heap.ledger().live_allocations, 2);
}

#[test]
fn owned_record_vector_output_take_matches_native_move_and_failed_output_preservation() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let (mut child, mut source) = incoming(&mut heap, element, 73);
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut vector, element, 1);
    let mut fuel = usize::MAX;
    heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel)
        .unwrap();
    let before = vector.encoding();
    let mut output = OwnedVectorDescriptor::empty();
    fuel = 46;
    assert_eq!(
        heap.owned_vector_take_into(&mut vector, element, &mut fuel, &mut output),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, 46);
    assert_eq!(vector.encoding(), before);
    assert_eq!(output.encoding(), [0; 16]);
    assert!(heap.descriptor(&child.encoding()).is_ok());
    fuel = 47;
    heap.owned_vector_take_into(&mut vector, element, &mut fuel, &mut output)
        .unwrap();
    assert_eq!(fuel, 0);
    assert_eq!(vector.encoding(), [0; 16]);
    assert_eq!(output.encoding(), before);
    let mut record = vec![0; element.size()];
    fuel = usize::MAX;
    heap.owned_vector_pop_last(&mut output, element, &mut record, &mut fuel)
        .unwrap();
    let offset = element.declarations().fields(element.record()).unwrap()[1].offset();
    assert_eq!(&record[offset..offset + 16], &child.encoding());
    heap.owned_vector_release(&mut output, element).unwrap();
    assert!(heap.descriptor(&child.encoding()).is_ok());
    heap.release(&mut child).unwrap();
    assert_eq!(heap.ledger().live_bytes, 0);
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}

fn incoming(heap: &mut Heap, element: Element<'_>, value: i32) -> (Descriptor, Vec<u8>) {
    let mut child = Descriptor::empty();
    let mut fuel = usize::MAX;
    assert_eq!(
        heap.try_extend(&mut child, b"owned message", &mut fuel),
        Ok(GrowStatus::Ok)
    );
    let fields = element.declarations().fields(element.record()).unwrap();
    let mut record = vec![0xa5; element.size()];
    record[fields[0].offset()] = 1;
    let offset = fields[1].offset();
    record[offset..offset + 16].copy_from_slice(&child.encoding());
    let offset = fields[2].offset();
    record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    (child, record)
}

fn reserve(heap: &mut Heap, vector: &mut OwnedVectorDescriptor, element: Element<'_>, count: i32) {
    let mut fuel = usize::MAX;
    assert_eq!(
        heap.owned_vector_try_reserve(vector, element, count, &mut fuel),
        Ok(GrowStatus::Ok)
    );
}

#[test]
fn owned_record_vector_codec_uses_checked_layout_and_distinct_heap_class() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    assert_eq!(element.size(), 32);
    assert_eq!(element.identity_capacity(), 1);
    assert_eq!(element.traversal_work(1), Some(7));
    assert!(Element::check(&declarations, RecordId(2)).is_err());
    let mut heap = Heap::default();
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut vector, element, 1);
    assert_eq!(heap.owned_vector_capacity(&vector, element), Ok(8));
    assert_eq!(heap.ledger().live_bytes, 64 + 8 * (32 + 8));
    assert!(heap.descriptor(&vector.encoding()).is_err());
    assert!(heap
        .vector_descriptor(&vector.encoding(), VectorElement::I32)
        .is_err());
    assert!(heap
        .owned_vector_descriptor(
            &vector.encoding(),
            Element::check(&declarations, RecordId(1)).unwrap(),
        )
        .is_err());
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert_eq!(heap.ledger().live_bytes, 0);
}

#[test]
fn owned_record_vector_move_round_trip_canonicalizes_padding_and_retains_child() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let (mut child, mut source) = incoming(&mut heap, element, 731);
    let child_encoding = child.encoding();
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut vector, element, 1);
    let mut fuel = usize::MAX;
    heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel)
        .unwrap();
    assert!(source.iter().all(|byte| *byte == 0));
    assert_eq!(heap.bytes(&child).unwrap(), b"owned message");
    let mut result = vec![0xee; element.size()];
    heap.owned_vector_pop_last(&mut vector, element, &mut result, &mut fuel)
        .unwrap();
    assert_eq!(heap.owned_vector_len(&vector, element), Ok(0));
    assert_eq!(result[0], 1);
    assert_eq!(&result[1..8], &[0; 7]);
    assert_eq!(&result[8..24], &child_encoding);
    assert_eq!(&result[24..28], &731i32.to_le_bytes());
    assert_eq!(&result[28..], &[0; 4]);
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert_eq!(heap.ledger().live_allocations, 1);
    heap.release(&mut child).unwrap();
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}

#[test]
fn owned_record_vector_growth_failures_never_drop_or_consume_children() {
    for boundary in [Boundary::Allocate, Boundary::Initialize, Boundary::Commit] {
        let declarations = declarations();
        let element = Element::check(&declarations, RecordId(0)).unwrap();
        let mut heap = Heap::default();
        let (child, mut source) = incoming(&mut heap, element, 11);
        let mut vector = OwnedVectorDescriptor::empty();
        reserve(&mut heap, &mut vector, element, 1);
        let mut fuel = usize::MAX;
        heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel)
            .unwrap();
        let before = vector.encoding();
        let ledger = heap.ledger();
        heap.fault = Some((boundary, 0));
        assert_eq!(
            heap.owned_vector_try_reserve(&mut vector, element, 8, &mut fuel),
            Ok(GrowStatus::AllocationFailed)
        );
        assert_eq!(vector.encoding(), before);
        assert_eq!(heap.bytes(&child).unwrap(), b"owned message");
        assert_eq!(heap.ledger().live_allocations, ledger.live_allocations);
        assert_eq!(heap.ledger().live_bytes, ledger.live_bytes);
        heap.owned_vector_release(&mut vector, element).unwrap();
        assert_eq!(heap.ledger().live_allocations, 0);
        assert_eq!(heap.ledger().allocations, heap.ledger().releases);
    }
}

#[test]
fn owned_record_vector_growth_and_truncate_drop_active_children_once() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let mut vector = OwnedVectorDescriptor::empty();
    let mut fuel = usize::MAX;
    let mut children = Vec::new();
    for value in 0..11 {
        let (child, mut source) = incoming(&mut heap, element, value);
        reserve(&mut heap, &mut vector, element, 1);
        heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel)
            .unwrap();
        children.push(child);
    }
    assert_eq!(heap.owned_vector_capacity(&vector, element), Ok(16));
    for child in &children {
        assert_eq!(heap.bytes(child).unwrap(), b"owned message");
    }
    heap.owned_vector_truncate(&mut vector, element, 3, &mut fuel)
        .unwrap();
    assert_eq!(heap.ledger().live_allocations, 4);
    for (index, child) in children.iter().enumerate() {
        assert_eq!(heap.bytes(child).is_ok(), index < 3);
    }
    let mut taken = heap
        .owned_vector_take(&mut vector, element, &mut fuel)
        .unwrap();
    assert_eq!(vector.encoding(), [0; 16]);
    heap.owned_vector_release(&mut taken, element).unwrap();
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
    assert_eq!(heap.ledger().live_bytes, 0);
}

#[test]
fn owned_record_vector_duplicate_children_fail_before_ownership_commit() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let (child, mut source) = incoming(&mut heap, element, 9);
    let mut duplicate = source.clone();
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut vector, element, 2);
    let mut fuel = usize::MAX;
    heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel)
        .unwrap();
    let before = vector.encoding();
    let incoming = duplicate.clone();
    assert_eq!(
        heap.owned_vector_push_reserved(&mut vector, element, &mut duplicate, &mut fuel,),
        Err(Failure::Descriptor)
    );
    assert_eq!(duplicate, incoming);
    assert_eq!(vector.encoding(), before);
    assert!(heap.bytes(&child).is_ok());
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert_eq!(heap.ledger().live_allocations, 0);
}

#[test]
fn owned_record_vector_bounds_fuel_and_scalar_failures_preserve_owners() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let (mut child, mut source) = incoming(&mut heap, element, 47);
    let mut vector = OwnedVectorDescriptor::empty();
    let incoming = source.clone();
    let mut fuel = usize::MAX;
    assert_eq!(
        heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel),
        Err(Failure::Bounds)
    );
    reserve(&mut heap, &mut vector, element, 1);
    let empty = vector.encoding();
    let mut local_fuel = 0;
    assert_eq!(
        heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut local_fuel,),
        Err(Failure::Fuel)
    );
    assert_eq!(source, incoming);
    assert_eq!(vector.encoding(), empty);
    source[0] = 2;
    assert_eq!(
        heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel),
        Err(Failure::Descriptor)
    );
    assert_eq!(source[0], 2);
    assert_eq!(vector.encoding(), empty);
    let mut result = vec![0xc9; element.size()];
    assert_eq!(
        heap.owned_vector_pop_last(&mut vector, element, &mut result, &mut fuel),
        Err(Failure::Bounds)
    );
    assert!(result.iter().all(|byte| *byte == 0xc9));
    heap.owned_vector_release(&mut vector, element).unwrap();
    heap.release(&mut child).unwrap();
    assert_eq!(heap.ledger().live_allocations, 0);
}

#[test]
fn owned_record_vector_memory_limit_and_fatal_cleanup_need_no_drop_scratch() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let (child, mut source) = incoming(&mut heap, element, 47);
    let incoming = source.clone();
    let mut vector = OwnedVectorDescriptor::empty();
    let mut fuel = usize::MAX;
    heap.limit = heap.ledger().live_bytes + 64 + 8 * (32 + 8) - 1;
    assert_eq!(
        heap.owned_vector_try_reserve(&mut vector, element, 1, &mut fuel),
        Ok(GrowStatus::MemoryLimit)
    );
    assert_eq!(source, incoming);
    assert!(heap.bytes(&child).is_ok());
    heap.limit += 1;
    reserve(&mut heap, &mut vector, element, 1);
    heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel)
        .unwrap();
    heap.cleanup();
    assert_eq!(heap.ledger().live_bytes, 0);
    assert_eq!(heap.ledger().live_allocations, 0);
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
    assert!(heap.bytes(&child).is_err());
}

#[test]
fn owned_record_vector_old_vector_descriptor_is_never_an_owned_record_vector() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let mut old = VectorDescriptor::empty();
    let mut fuel = usize::MAX;
    assert_eq!(
        heap.vector_try_reserve(&mut old, VectorElement::I32, 1, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert!(heap
        .owned_vector_descriptor(&old.encoding(), element)
        .is_err());
    heap.vector_release(&mut old, VectorElement::I32).unwrap();
    assert_eq!(heap.ledger().live_bytes, 0);
}

#[test]
fn owned_record_vector_tail_alignment_and_offset_are_independently_checked() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let mut vector = OwnedVectorDescriptor::empty();
    assert_eq!(heap.ledger().live_bytes, 0);
    reserve(&mut heap, &mut vector, element, 1);
    let node = heap.locate_owned_vector(&vector, element).unwrap().unwrap();
    let layout = allocation_layout(element, 8).unwrap();
    assert_eq!(layout.identity_offset, 320);
    assert_eq!(layout.identity_slots, 8);
    assert_eq!(layout.size(), 384);
    assert_eq!((node.as_ptr() as usize + layout.identity_offset) % 8, 0);
    unsafe {
        (*node.cast::<OwnedVectorHeader>().as_ptr()).identity_offset += 8;
    }
    assert_eq!(
        heap.owned_vector_len(&vector, element),
        Err(Failure::Descriptor)
    );
    // Raw fatal cleanup does not consult a corrupted payload/tail offset.
    heap.cleanup();
    assert_eq!(heap.ledger().live_bytes, 0);
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}

#[test]
fn owned_record_vector_control_and_move_work_has_exact_and_one_less_admission() {
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let (child, mut source) = incoming(&mut heap, element, 47);
    let source_before = source.clone();
    let mut vector = OwnedVectorDescriptor::empty();

    // N=1, R=2: 1 fixed + 2 Heap + 1 pair + 3 node visits = 7.
    // The empty reserve writes the complete independently known 384-byte block.
    let mut fuel = 390;
    let before = heap.ledger();
    assert_eq!(
        heap.owned_vector_try_reserve(&mut vector, element, 1, &mut fuel),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, 390);
    assert_eq!(vector.encoding(), [0; 16]);
    assert_eq!(heap.ledger(), before);
    assert_eq!(source, source_before);
    fuel = 391;
    assert_eq!(
        heap.owned_vector_try_reserve(&mut vector, element, 1, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(fuel, 0);
    assert_eq!(heap.ledger().live_bytes, 45 + 384);

    // N=2, R=3: controls 1+3+3+8=15. One record has seven
    // transitions (three passes), one child lookup (two visits), four sort
    // units and 96 canonical-copy/zeroing bytes: 15+21+2+4+96=138.
    fuel = 137;
    let before = vector.encoding();
    let ledger = heap.ledger();
    assert_eq!(
        heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, 137);
    assert_eq!(vector.encoding(), before);
    assert_eq!(source, source_before);
    assert_eq!(heap.ledger(), ledger);
    assert_eq!(heap.bytes(&child).unwrap(), b"owned message");
    fuel = 138;
    heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel)
        .unwrap();
    assert_eq!(fuel, 0);
    assert_eq!(source, vec![0; 32]);

    // Pop retains the same 15 control units and 21+2+4 validation units,
    // but writes only 64 move/zeroing bytes: total 106.
    let mut destination = vec![0xcc; 32];
    fuel = 105;
    let before = vector.encoding();
    assert_eq!(
        heap.owned_vector_pop_last(&mut vector, element, &mut destination, &mut fuel),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, 105);
    assert_eq!(vector.encoding(), before);
    assert_eq!(destination, vec![0xcc; 32]);
    assert_eq!(heap.ledger(), ledger);
    assert_eq!(heap.bytes(&child).unwrap(), b"owned message");
    fuel = 106;
    heap.owned_vector_pop_last(&mut vector, element, &mut destination, &mut fuel)
        .unwrap();
    assert_eq!(fuel, 0);
    assert_eq!(&destination[8..24], &child.encoding());
    fuel = 138;
    heap.owned_vector_push_reserved(&mut vector, element, &mut destination, &mut fuel)
        .unwrap();
    assert_eq!(fuel, 0);

    // Taking a populated vector moves its child obligations without walking
    // or releasing them: N=2 controls cost ten, plus 32 descriptor bytes.
    fuel = 41;
    let before = vector.encoding();
    assert_eq!(
        heap.owned_vector_take(&mut vector, element, &mut fuel),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, 41);
    assert_eq!(vector.encoding(), before);
    assert_eq!(heap.bytes(&child).unwrap(), b"owned message");
    assert_eq!(heap.ledger(), ledger);
    fuel = 42;
    let taken = heap
        .owned_vector_take(&mut vector, element, &mut fuel)
        .unwrap();
    assert_eq!(fuel, 0);
    assert_eq!(vector.encoding(), [0; 16]);
    assert_eq!(taken.encoding(), before);
    assert_eq!(heap.bytes(&child).unwrap(), b"owned message");
    vector = taken;

    // Truncate R=2 controls are 1+2+1+6=10. Full preflight costs
    // 21+2+4, clearing costs 32, and two release lookups cost 4: total 73.
    fuel = 72;
    let before = vector.encoding();
    assert_eq!(
        heap.owned_vector_truncate(&mut vector, element, 0, &mut fuel),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, 72);
    assert_eq!(vector.encoding(), before);
    assert_eq!(heap.bytes(&child).unwrap(), b"owned message");
    assert_eq!(heap.ledger(), ledger);
    fuel = 73;
    heap.owned_vector_truncate(&mut vector, element, 0, &mut fuel)
        .unwrap();
    assert_eq!(fuel, 0);
    assert!(heap.bytes(&child).is_err());
    assert_eq!(heap.ledger().live_bytes, 384);

    // Take with one remaining node admits seven control units and copies /
    // clears the two 16-byte descriptor carriers, without walking children.
    fuel = 38;
    let before = vector.encoding();
    let ledger = heap.ledger();
    assert_eq!(
        heap.owned_vector_take(&mut vector, element, &mut fuel),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, 38);
    assert_eq!(vector.encoding(), before);
    assert_eq!(heap.ledger(), ledger);
    fuel = 39;
    let mut taken = heap
        .owned_vector_take(&mut vector, element, &mut fuel)
        .unwrap();
    assert_eq!(fuel, 0);
    assert_eq!(vector.encoding(), [0; 16]);
    assert_eq!(taken.encoding(), before);
    // Owner-exit maintenance is deliberately independent of exhausted fuel.
    heap.owned_vector_release(&mut taken, element).unwrap();
    assert_eq!(fuel, 0);
    assert_eq!(heap.ledger().live_bytes, 0);
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}

#[test]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64"),
))]
fn owned_record_vector_numeric_external_ranges_reject_every_registered_kind() {
    use crate::frontend::oir::owned::byte_buffer::{OpenValue, RecordVectorElement};
    use std::os::unix::ffi::OsStrExt;
    struct Path(std::path::PathBuf);
    impl Drop for Path {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let path = Path(std::env::temp_dir().join(format!(
        "oxid-owned-vector-extents-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    )));
    let declarations = declarations();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut copy_sources = SourceMap::new();
    let file = copy_sources.add("copy-vector-extent.ox".into(), "copy record".into());
    let span = copy_sources.get(file).span(0, 4);
    let copy_declarations = Declarations::check(
        &[RawRecordDecl {
            id: RecordId(0),
            is_copy: true,
            span,
            fields: [hir::Ty::I32, hir::Ty::Bool]
                .into_iter()
                .enumerate()
                .map(|(index, ty)| RawFieldDecl {
                    id: FieldId {
                        record: RecordId(0),
                        index,
                    },
                    ty: ParameterTy::Value(ValueTy::Scalar(ty)),
                    span,
                })
                .collect(),
        }],
        &copy_sources,
    )
    .unwrap();
    let copy_element = RecordVectorElement::check(&copy_declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let mut fuel = usize::MAX;
    let file = match heap
        .file_open(path.0.as_os_str().as_bytes(), true, &mut fuel)
        .unwrap()
    {
        OpenValue::Opened(file) => file,
        _ => panic!("extent control file open failed"),
    };
    let mut bytes = Descriptor::empty();
    assert_eq!(
        heap.try_extend(&mut bytes, b"message", &mut fuel),
        Ok(GrowStatus::Ok)
    );
    let mut scalars = VectorDescriptor::empty();
    assert_eq!(
        heap.vector_try_reserve(&mut scalars, VectorElement::I32, 1, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    let mut copies = VectorDescriptor::empty();
    assert_eq!(
        heap.vector_try_reserve(&mut copies, copy_element, 1, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    let mut selected = OwnedVectorDescriptor::empty();
    let mut other = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut selected, element, 1);
    reserve(&mut heap, &mut other, element, 1);
    let entries = [
        (file.encoding(), 32usize),
        (bytes.encoding(), 40),
        (scalars.encoding(), 80),
        (copies.encoding(), 96),
        (selected.encoding(), 384),
        (other.encoding(), 384),
    ];
    let before = heap.ledger();
    assert_eq!(before.live_allocations, 6);
    assert_eq!(before.live_bytes, 32 + 40 + 80 + 96 + 384 + 384);
    for (encoding, expected_bytes) in entries {
        let identity = u64::from_le_bytes(encoding[..8].try_into().unwrap());
        let mut cursor = heap.head;
        let node = loop {
            let node = NonNull::new(cursor).expect("live extent fixture node");
            // SAFETY: The numeric probe uses only actual live registry nodes;
            // it never creates a reference alias into an external argument.
            let header = unsafe { node.as_ref() };
            if header.identity == identity {
                break node;
            }
            cursor = header.next;
        };
        let (start, count) = unsafe { registered_extent(node) }.unwrap();
        assert_eq!(count, expected_bytes);
        for (address, length) in [(start, count), (start, 1), (start + count - 1, 1)] {
            assert_eq!(
                heap.owned_vector_disjoint(&[(address, length)]),
                Err(Failure::Descriptor)
            );
        }
        assert!(!overlaps(start + count, 1, start, count));
    }
    assert_eq!(
        heap.owned_vector_disjoint(&[
            (&heap as *const Heap as usize, std::mem::size_of::<Heap>(),)
        ]),
        Err(Failure::Descriptor)
    );
    assert_eq!(
        heap.owned_vector_disjoint(&[(usize::MAX - 2, 8)]),
        Err(Failure::Descriptor)
    );
    let scratch = [0u8; 16];
    let start = scratch.as_ptr() as usize;
    assert_eq!(
        heap.owned_vector_disjoint(&[(start, 9), (start + 8, 8)]),
        Err(Failure::Descriptor)
    );
    assert_eq!(
        heap.owned_vector_disjoint(&[(start, 8), (start + 8, 8)]),
        Ok(())
    );
    assert_eq!(heap.ledger(), before);
    assert!(heap.file_descriptor(&file.encoding()).is_ok());
    assert_eq!(heap.bytes(&bytes).unwrap(), b"message");
    assert_eq!(heap.vector_validate(&scalars, VectorElement::I32), Ok(()));
    assert_eq!(heap.vector_validate(&copies, copy_element), Ok(()));
    assert_eq!(heap.owned_vector_capacity(&selected, element), Ok(8));
    assert_eq!(heap.owned_vector_capacity(&other, element), Ok(8));
    heap.cleanup();
    assert_eq!(heap.ledger().live_bytes, 0);
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}

#[test]
fn owned_record_vector_checked_shape_metadata_limits_and_failures_precede_allocations() {
    let declarations = declarations();
    let request = CheckedElements::requested_bytes(&declarations).unwrap();
    let mut allocator = Allocator::default();
    assert!(matches!(
        CheckedElements::check_with_allocator(&declarations, request - 1, &mut allocator,),
        Err(ShapeFailure::MetadataLimit)
    ));
    assert_eq!(allocator.attempts, 0);
    for fault in 1..=2 {
        let mut allocator = Allocator {
            fail_at: Some(fault),
            ..Default::default()
        };
        assert!(matches!(
            CheckedElements::check_with_allocator(&declarations, request, &mut allocator,),
            Err(ShapeFailure::Allocation)
        ));
        assert_eq!(allocator.attempts, fault);
    }
    let checked = CheckedElements::check(&declarations, request).unwrap();
    assert!(checked.retained_bytes().unwrap() <= request);
    assert_eq!(checked.element(RecordId(0)).unwrap().identity_capacity(), 1);
}

fn enum_dag_declarations(depth: usize) -> Declarations {
    let mut sources = SourceMap::new();
    let file = sources.add(
        "owned-vector-enum-controls.ox".into(),
        "enum record control".into(),
    );
    let span = sources.get(file).span(0, 4);
    let records = [RawRecordDecl {
        is_copy: false,
        id: RecordId(0),
        span,
        fields: vec![RawFieldDecl {
            id: FieldId {
                record: RecordId(0),
                index: 0,
            },
            ty: ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(EnumId(0)))),
            span,
        }],
    }];
    let enums = (0..depth)
        .map(|enumeration| {
            let payload = if enumeration + 1 == depth {
                AggregateTy::ByteBuf
            } else {
                AggregateTy::Enum(EnumId(enumeration + 1))
            };
            RawEnumDecl {
                id: EnumId(enumeration),
                span,
                variants: (0..3)
                    .map(|index| RawVariantDecl {
                        id: VariantId {
                            enumeration: EnumId(enumeration),
                            index,
                        },
                        payload: (index != 2)
                            .then_some(ParameterTy::Value(ValueTy::Owned(payload))),
                        span,
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    Declarations::check_combined(&records, &enums, &sources).unwrap()
}

#[test]
fn owned_record_vector_shared_enum_dag_bounds_and_active_payloads_are_finite() {
    const DEPTH: usize = 24;
    let declarations = enum_dag_declarations(DEPTH);
    let checked =
        CheckedElements::check(&declarations, super::super::super::plan::MAX_PLAN_BYTES).unwrap();
    let element = checked.element(RecordId(0)).unwrap();
    assert_eq!(element.identity_capacity(), 1);
    assert_eq!(element.traversal_work(1), Some(2 * DEPTH + 3));
    let mut heap = Heap::default();
    let mut child = Descriptor::empty();
    let mut fuel = usize::MAX;
    assert_eq!(
        heap.try_extend(&mut child, b"enum child", &mut fuel),
        Ok(GrowStatus::Ok)
    );
    let mut source = vec![0xa5; element.size()];
    let mut offset = declarations.fields(RecordId(0)).unwrap()[0].offset();
    for enumeration in 0..DEPTH {
        source[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes());
        let variant = declarations
            .enums()
            .variant_for_tag(EnumId(enumeration), 0)
            .unwrap();
        offset += variant.payload_offset().unwrap();
    }
    source[offset..offset + 16].copy_from_slice(&child.encoding());
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut vector, element, 1);
    heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel)
        .unwrap();
    assert!(heap.bytes(&child).is_ok());
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert!(heap.bytes(&child).is_err());
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}

#[test]
fn owned_record_vector_inactive_enum_bytes_never_acquire_child_ownership() {
    let declarations = enum_dag_declarations(3);
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let mut child = Descriptor::empty();
    let mut fuel = usize::MAX;
    assert_eq!(
        heap.try_extend(&mut child, b"caller still owns", &mut fuel),
        Ok(GrowStatus::Ok)
    );
    let mut source = vec![0xa5; element.size()];
    source[..4].copy_from_slice(&2u32.to_le_bytes());
    source[8..24].copy_from_slice(&child.encoding());
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut vector, element, 1);
    heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel)
        .unwrap();
    let mut result = vec![0xff; element.size()];
    heap.owned_vector_pop_last(&mut vector, element, &mut result, &mut fuel)
        .unwrap();
    assert_eq!(&result[..4], &2u32.to_le_bytes());
    assert!(result[4..].iter().all(|byte| *byte == 0));
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert_eq!(heap.bytes(&child).unwrap(), b"caller still owns");
    heap.release(&mut child).unwrap();
    assert_eq!(heap.ledger().live_allocations, 0);
}

#[test]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64"),
))]
fn owned_record_vector_file_scalar_and_copy_record_vector_leaves_release_once() {
    use crate::frontend::oir::owned::byte_buffer::{OpenValue, RecordVectorElement};
    use std::os::unix::ffi::OsStrExt;
    struct Path(std::path::PathBuf);
    impl Drop for Path {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let path = Path(std::env::temp_dir().join(format!(
        "oxid-owned-vector-leaves-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
    )));
    let mut sources = SourceMap::new();
    let source_id = sources.add(
        "owned-vector-resource-leaves.ox".into(),
        "record leaves".into(),
    );
    let span = sources.get(source_id).span(0, 6);
    let field_types = [
        vec![
            ValueTy::Owned(AggregateTy::File),
            ValueTy::Owned(AggregateTy::Vector(hir::Ty::I32)),
            ValueTy::Owned(AggregateTy::RecordVector(RecordId(1))),
        ],
        vec![
            ValueTy::Scalar(hir::Ty::I32),
            ValueTy::Scalar(hir::Ty::Bool),
        ],
    ];
    let records = field_types
        .iter()
        .enumerate()
        .map(|(record, fields)| RawRecordDecl {
            id: RecordId(record),
            is_copy: record == 1,
            span,
            fields: fields
                .iter()
                .enumerate()
                .map(|(index, ty)| RawFieldDecl {
                    id: FieldId {
                        record: RecordId(record),
                        index,
                    },
                    ty: ParameterTy::Value(*ty),
                    span,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let declarations = Declarations::check(&records, &sources).unwrap();
    let element = Element::check(&declarations, RecordId(0)).unwrap();
    assert_eq!(element.size(), 48);
    assert_eq!(element.identity_capacity(), 3);
    let copy_element = RecordVectorElement::check(&declarations, RecordId(1)).unwrap();
    let mut heap = Heap::default();
    let mut fuel = usize::MAX;
    let file = match heap
        .file_open(path.0.as_os_str().as_bytes(), true, &mut fuel)
        .unwrap()
    {
        OpenValue::Opened(file) => file,
        _ => panic!("resource leaf file open failed"),
    };
    assert_eq!(
        heap.file_access(&file)
            .unwrap()
            .write(b"alive", &mut fuel)
            .unwrap(),
        Ok(5)
    );
    let file_access = heap.file_access(&file).unwrap();
    let mut scalars = VectorDescriptor::empty();
    assert_eq!(
        heap.vector_try_push(
            &mut scalars,
            VectorElement::I32,
            &37i32.to_le_bytes(),
            &mut fuel
        ),
        Ok(GrowStatus::Ok)
    );
    let mut copies = VectorDescriptor::empty();
    assert_eq!(
        heap.vector_try_push(&mut copies, copy_element, &[37, 0, 0, 0, 1], &mut fuel),
        Ok(GrowStatus::Ok)
    );
    let mut incoming = vec![0; element.size()];
    for (field, descriptor) in declarations.fields(RecordId(0)).unwrap().iter().zip([
        file.encoding(),
        scalars.encoding(),
        copies.encoding(),
    ]) {
        incoming[field.offset()..field.offset() + 16].copy_from_slice(&descriptor);
    }
    let mut owned = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut owned, element, 1);
    assert_eq!(heap.ledger().live_bytes, 32 + 80 + 96 + 640);
    heap.owned_vector_push_reserved(&mut owned, element, &mut incoming, &mut fuel)
        .unwrap();
    heap.owned_vector_release(&mut owned, element).unwrap();
    assert!(heap.file_descriptor(&file.encoding()).is_err());
    assert!(heap.vector_validate(&scalars, VectorElement::I32).is_err());
    assert!(heap.vector_validate(&copies, copy_element).is_err());
    // The capability retained the exact fd proven writable before transfer.
    // No descriptor is opened by this test between release and this syscall.
    assert_eq!(
        file_access.write(b"x", &mut fuel).unwrap(),
        Err(libc::EBADF)
    );
    assert_eq!(heap.ledger().live_bytes, 0);
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}
