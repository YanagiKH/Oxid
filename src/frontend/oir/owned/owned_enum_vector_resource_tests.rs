use super::*;
use crate::frontend::oir::owned::byte_buffer::Descriptor;
use crate::frontend::oir::owned::owned_record_codec::bounds::{CheckedElements, ShapeFailure};
use crate::frontend::oir::owned_types::*;
use crate::frontend::project::budget::Allocator;
use crate::frontend::{hir, source::SourceMap};
use std::mem::{align_of, size_of};

fn abundant_work() -> usize {
    usize::MAX
}

fn declarations() -> Declarations {
    let mut sources = SourceMap::new();
    let file = sources.add(
        "enum-vector-layout.ox".into(),
        "enum vector controls".into(),
    );
    let span = sources.get(file).span(0, 4);
    let records = [
        vec![ValueTy::Scalar(hir::Ty::I128); 3],
        vec![ValueTy::Owned(AggregateTy::ByteBuf); 2],
    ]
    .into_iter()
    .enumerate()
    .map(|(record, types)| RawRecordDecl {
        id: RecordId(record),
        is_copy: false,
        span,
        fields: types
            .into_iter()
            .enumerate()
            .map(|(index, value)| RawFieldDecl {
                id: FieldId {
                    record: RecordId(record),
                    index,
                },
                ty: ParameterTy::Value(value),
                span,
            })
            .collect(),
    })
    .collect::<Vec<_>>();
    let enumerations = (0..2)
        .map(|enumeration| RawEnumDecl {
            id: EnumId(enumeration),
            span,
            variants: [
                None,
                Some(ValueTy::Owned(AggregateTy::ByteBuf)),
                Some(ValueTy::Scalar(hir::Ty::I128)),
                Some(ValueTy::Owned(AggregateTy::Record(RecordId(1)))),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, value)| RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(enumeration),
                    index,
                },
                payload: value.map(ParameterTy::Value),
                span,
            })
            .collect(),
        })
        .collect::<Vec<_>>();
    Declarations::check_combined(&records, &enumerations, &sources).unwrap()
}

fn make_child(heap: &mut Heap) -> Descriptor {
    let mut child = Descriptor::empty();
    assert_eq!(
        heap.try_extend(&mut child, b"enum", &mut abundant_work()),
        Ok(GrowStatus::Ok)
    );
    child
}
fn incoming(child: &Descriptor) -> Vec<u8> {
    let mut bytes = vec![0xa5; 48];
    bytes[..4].copy_from_slice(&1u32.to_le_bytes());
    bytes[16..32].copy_from_slice(&child.encoding());
    bytes
}
fn reserve(heap: &mut Heap, vector: &mut OwnedVectorDescriptor, element: Element<'_>) {
    assert_eq!(
        heap.owned_vector_try_reserve(vector, element, 1, &mut abundant_work()),
        Ok(GrowStatus::Ok)
    );
}

#[test]
fn owned_enum_vector_real_layout_and_header_keep_old_widths_and_separate_roots() {
    assert_eq!(
        (size_of::<Element<'_>>(), align_of::<Element<'_>>()),
        (40, 8)
    );
    assert_eq!(
        (
            size_of::<OwnedVectorDescriptor>(),
            align_of::<OwnedVectorDescriptor>()
        ),
        (16, 8)
    );
    assert_eq!(
        (
            size_of::<OwnedVectorHeader>(),
            align_of::<OwnedVectorHeader>()
        ),
        (64, 16)
    );
    assert_eq!(std::mem::offset_of!(OwnedVectorHeader, record), 36);
    let declarations = declarations();
    let checked = CheckedElements::check(
        &declarations,
        crate::frontend::oir::owned::plan::MAX_PLAN_BYTES,
    )
    .unwrap();
    let record = checked.element(RecordId(0)).unwrap();
    let enumeration = checked
        .element_root(OwnedVectorRoot::Enum(EnumId(0)))
        .unwrap();
    assert_eq!(
        (record.storage_key(), enumeration.storage_key()),
        (0, 0x8000_0000)
    );
    assert_eq!((record.size(), enumeration.size()), (48, 48));
    assert_eq!(enumeration.identity_capacity(), 2);
    assert_eq!(enumeration.traversal_work(1), Some(7));
    let layout = declarations
        .aggregate_layout(AggregateTy::Enum(EnumId(0)))
        .unwrap();
    assert_eq!((layout.size(), layout.align()), (48, 16));
    for variant in declarations.enums().variants(EnumId(0)).unwrap() {
        assert_eq!(
            variant.payload_offset(),
            variant.value_payload().map(|_| 16)
        );
    }
    for root in [
        OwnedVectorRoot::Enum(EnumId(2)),
        OwnedVectorRoot::Enum(EnumId(4096)),
        OwnedVectorRoot::Record(RecordId(4096)),
    ] {
        assert!(checked.element_root(root).is_err());
    }
    assert_eq!(allocation_layout(enumeration, 8).unwrap().size(), 576);
    assert_eq!(allocation_layout(record, 8).unwrap().size(), 448);
}

#[test]
fn owned_enum_vector_exact_work_and_failed_growth_preserve_all_incoming_owners() {
    let declarations = declarations();
    let checked = CheckedElements::check(
        &declarations,
        crate::frontend::oir::owned::plan::MAX_PLAN_BYTES,
    )
    .unwrap();
    let element = checked
        .element_root(OwnedVectorRoot::Enum(EnumId(0)))
        .unwrap();
    let mut heap = Heap::default();
    let child = make_child(&mut heap);
    let mut source = incoming(&child);
    let original = source.clone();
    let mut vector = OwnedVectorDescriptor::empty();
    // One existing 40-byte child: controls7 + full576-byte staging block.
    let mut fuel = 582;
    let before = heap.ledger();
    assert_eq!(
        heap.owned_vector_try_reserve(&mut vector, element, 1, &mut fuel),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, 582);
    assert_eq!(heap.ledger(), before);
    assert_eq!(vector.encoding(), [0; 16]);
    assert_eq!(source, original);
    fuel = 583;
    assert_eq!(
        heap.owned_vector_try_reserve(&mut vector, element, 1, &mut fuel),
        Ok(GrowStatus::Ok)
    );
    assert_eq!(fuel, 0);
    assert_eq!(heap.ledger().live_bytes, 40 + 576);
    // Two nodes, three external ranges: controls15; three7-visit walks21;
    // two potential children*two nodes4; bounded uniqueness16; copy/zero144.
    fuel = 199;
    assert_eq!(
        heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel),
        Err(Failure::Fuel)
    );
    assert_eq!(fuel, 199);
    assert_eq!(source, original);
    assert_eq!(heap.bytes(&child).unwrap(), b"enum");
    fuel = 200;
    heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut fuel)
        .unwrap();
    assert_eq!(fuel, 0);
    assert_eq!(source, vec![0; 48]);
    let untouched = make_child(&mut heap);
    let untouched_bytes = incoming(&untouched);
    for boundary in [Boundary::Allocate, Boundary::Initialize, Boundary::Commit] {
        let vector_before = vector.encoding();
        let before = heap.ledger();
        heap.fault = Some((boundary, 0));
        assert_eq!(
            heap.owned_vector_try_reserve(&mut vector, element, 8, &mut abundant_work()),
            Ok(GrowStatus::AllocationFailed)
        );
        assert_eq!(vector.encoding(), vector_before);
        assert_eq!(
            (heap.ledger().live_bytes, heap.ledger().live_allocations),
            (before.live_bytes, before.live_allocations)
        );
        assert_eq!(heap.bytes(&untouched).unwrap(), b"enum");
        assert_eq!(untouched_bytes, incoming(&untouched));
    }
    let mut output = vec![0xcc; 48];
    heap.owned_vector_pop_last(&mut vector, element, &mut output, &mut abundant_work())
        .unwrap();
    assert_eq!(&output[..4], &1u32.to_le_bytes());
    assert!(output[4..16].iter().all(|byte| *byte == 0));
    assert_eq!(&output[16..32], &child.encoding());
    assert!(output[32..].iter().all(|byte| *byte == 0));
    heap.owned_vector_push_reserved(&mut vector, element, &mut output, &mut abundant_work())
        .unwrap();
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert!(heap.bytes(&child).is_err());
    assert_eq!(heap.bytes(&untouched).unwrap(), b"enum");
    heap.cleanup();
    assert_eq!(heap.ledger().live_bytes, 0);
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}

#[test]
fn owned_enum_vector_wrong_domains_forged_tags_and_duplicate_children_refuse_before_commit() {
    let declarations = declarations();
    let checked = CheckedElements::check(
        &declarations,
        crate::frontend::oir::owned::plan::MAX_PLAN_BYTES,
    )
    .unwrap();
    let element = checked
        .element_root(OwnedVectorRoot::Enum(EnumId(0)))
        .unwrap();
    let other = checked
        .element_root(OwnedVectorRoot::Enum(EnumId(1)))
        .unwrap();
    let record = checked.element(RecordId(0)).unwrap();
    let mut heap = Heap::default();
    let child = make_child(&mut heap);
    let mut source = incoming(&child);
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut vector, element);
    for wrong in [other, record] {
        let before = source.clone();
        let ledger = heap.ledger();
        assert_eq!(
            heap.owned_vector_push_reserved(&mut vector, wrong, &mut source, &mut abundant_work()),
            Err(Failure::Descriptor)
        );
        assert_eq!(source, before);
        assert_eq!(heap.ledger(), ledger);
    }
    for tag in [4u32, u32::MAX] {
        source[..4].copy_from_slice(&tag.to_le_bytes());
        let before = source.clone();
        assert_eq!(
            heap.owned_vector_push_reserved(
                &mut vector,
                element,
                &mut source,
                &mut abundant_work()
            ),
            Err(Failure::Descriptor)
        );
        assert_eq!(source, before);
        assert_eq!(vector.length, 0);
        assert_eq!(heap.bytes(&child).unwrap(), b"enum");
    }
    source[..4].copy_from_slice(&3u32.to_le_bytes());
    source[32..48].copy_from_slice(&child.encoding());
    let duplicate = source.clone();
    assert_eq!(
        heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut abundant_work()),
        Err(Failure::Descriptor)
    );
    assert_eq!(source, duplicate);
    source = incoming(&child);
    heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut abundant_work())
        .unwrap();
    let mut alias = incoming(&child);
    let alias_before = alias.clone();
    assert_eq!(
        heap.owned_vector_push_reserved(&mut vector, element, &mut alias, &mut abundant_work()),
        Err(Failure::Descriptor)
    );
    assert_eq!(alias, alias_before);
    assert_eq!(vector.length, 1);
    let claim = OwnedVectorDescriptor::decode(&vector.encoding()).unwrap();
    assert_eq!(
        heap.owned_vector_validate(&claim, record, &mut abundant_work()),
        Err(Failure::Descriptor)
    );
    assert_eq!(
        heap.owned_vector_validate(&claim, other, &mut abundant_work()),
        Err(Failure::Descriptor)
    );
    heap.owned_vector_release(&mut vector, element).unwrap();
    assert!(heap.bytes(&child).is_err());
    assert_eq!(heap.ledger().live_allocations, 0);
}

#[test]
fn owned_enum_vector_inactive_poison_is_unread_and_never_drains_unrelated_children() {
    let declarations = declarations();
    let checked = CheckedElements::check(
        &declarations,
        crate::frontend::oir::owned::plan::MAX_PLAN_BYTES,
    )
    .unwrap();
    let element = checked
        .element_root(OwnedVectorRoot::Enum(EnumId(0)))
        .unwrap();
    let mut heap = Heap::default();
    let mut child = make_child(&mut heap);
    let mut source = incoming(&child);
    source[..4].copy_from_slice(&0u32.to_le_bytes());
    let mut vector = OwnedVectorDescriptor::empty();
    reserve(&mut heap, &mut vector, element);
    heap.owned_vector_push_reserved(&mut vector, element, &mut source, &mut abundant_work())
        .unwrap();
    let mut output = vec![0xff; 48];
    heap.owned_vector_pop_last(&mut vector, element, &mut output, &mut abundant_work())
        .unwrap();
    assert_eq!(output, vec![0; 48]);
    heap.owned_vector_push_reserved(&mut vector, element, &mut output, &mut abundant_work())
        .unwrap();
    heap.owned_vector_truncate(&mut vector, element, 0, &mut abundant_work())
        .unwrap();
    assert_eq!(heap.bytes(&child).unwrap(), b"enum");
    heap.owned_vector_release(&mut vector, element).unwrap();
    heap.release(&mut child).unwrap();
    assert_eq!(heap.ledger().live_allocations, 0);
}

#[test]
fn owned_enum_vector_bounds_preflight_pays_exact_metadata_before_any_reservation() {
    let declarations = declarations();
    let request = CheckedElements::requested_bytes(&declarations).unwrap();
    let mut allocator = Allocator::default();
    assert!(matches!(
        CheckedElements::check_with_allocator(&declarations, request - 1, &mut allocator),
        Err(ShapeFailure::MetadataLimit)
    ));
    assert_eq!(allocator.attempts, 0);
    for ordinal in 1..=2 {
        let mut allocator = Allocator {
            fail_at: Some(ordinal),
            ..Default::default()
        };
        assert!(matches!(
            CheckedElements::check_with_allocator(&declarations, request, &mut allocator),
            Err(ShapeFailure::Allocation)
        ));
        assert_eq!(allocator.attempts, ordinal);
    }
    let checked =
        CheckedElements::check_with_allocator(&declarations, request, &mut Allocator::default())
            .unwrap();
    assert_eq!(
        checked
            .element_root(OwnedVectorRoot::Enum(EnumId(0)))
            .unwrap()
            .size(),
        48
    );
}
