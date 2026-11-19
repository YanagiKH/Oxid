//! Mixed-resource transaction oracles.
#![cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use super::super::super::consumer_fixtures as fixture;
use super::*;
use std::{
    fs,
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct TemporaryPath(PathBuf);
impl TemporaryPath {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "oxid-vector-resource-consumer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for TemporaryPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn declarations() -> (Declarations, Span) {
    let (sources, at) = fixture::context();
    let span = at(0);
    let mut record = fixture::record(&[hir::Ty::Unit; 4], span);
    for (field, ty) in record.fields.iter_mut().zip([
        ValueTy::Owned(AggregateTy::ByteBuf),
        ValueTy::Owned(AggregateTy::File),
        ValueTy::Owned(AggregateTy::Vector(hir::Ty::I128)),
        ValueTy::Scalar(hir::Ty::Bool),
    ]) {
        field.ty = ParameterTy::Value(ty);
    }
    let enumeration = RawEnumDecl {
        id: EnumId(0),
        span,
        variants: [Some(ValueTy::Owned(AggregateTy::Record(RecordId(0)))), None]
            .into_iter()
            .enumerate()
            .map(|(index, payload)| RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(0),
                    index,
                },
                span,
                payload: payload.map(ParameterTy::Value),
            })
            .collect(),
    };
    let declarations = Declarations::check_combined(&[record], &[enumeration], &sources).unwrap();
    assert_eq!(
        declarations
            .fields(RecordId(0))
            .unwrap()
            .iter()
            .map(FieldDecl::offset)
            .collect::<Vec<_>>(),
        [0, 16, 32, 48]
    );
    assert_eq!(
        declarations
            .aggregate_layout(AggregateTy::Record(RecordId(0)))
            .unwrap()
            .size(),
        56
    );
    assert_eq!(
        declarations
            .aggregate_layout(AggregateTy::Enum(EnumId(0)))
            .unwrap()
            .size(),
        64
    );
    (declarations, span)
}

fn mixed_value(
    heap: &mut byte_buffer::Heap,
    vector_element: byte_buffer::VectorElement,
) -> (TemporaryPath, [u8; 56]) {
    let path = TemporaryPath::new();
    let mut fuel = 100_000;
    let mut bytes = [0xcc; 56];
    let mut buffer = byte_buffer::Descriptor::empty();
    assert_eq!(
        heap.try_extend(&mut buffer, &[0, 128, 255], &mut fuel),
        Ok(byte_buffer::GrowStatus::Ok)
    );
    buffer.write(&mut bytes[..16]).unwrap();
    let file = match heap
        .file_open(path.0.as_os_str().as_bytes(), true, &mut fuel)
        .unwrap()
    {
        byte_buffer::OpenValue::Opened(file) => file,
        _ => panic!("expected actual temporary file"),
    };
    bytes[16..32].copy_from_slice(&file.encoding());
    let mut vector = byte_buffer::VectorDescriptor::empty();
    assert_eq!(
        heap.vector_try_push(
            &mut vector,
            vector_element,
            &i128::MIN.to_le_bytes(),
            &mut fuel
        ),
        Ok(byte_buffer::GrowStatus::Ok)
    );
    vector.write(&mut bytes[32..48]).unwrap();
    bytes[48] = 1;
    // ByteBuf 32+8; File 32; vector 48+8*16. These are independent literals.
    assert_eq!(
        (heap.ledger().live_allocations, heap.ledger().live_bytes),
        (3, 248)
    );
    (path, bytes)
}

#[test]
fn vector_reference_mixed_record_transfer_keeps_payload_and_one_release_obligation() {
    let (declarations, span) = declarations();
    let aggregate = AggregateTy::Record(RecordId(0));
    let mut heap = byte_buffer::Heap::default();
    let (_path, source) = mixed_value(&mut heap, byte_buffer::VectorElement::I128);
    let before = heap.ledger();
    let mut target = [0xa5; 56];
    preflight(&declarations, &heap, aggregate, &source, target.len(), span).unwrap();
    copy_active_bytes(&declarations, aggregate, &source, &mut target, span).unwrap();
    assert_eq!(heap.ledger(), before);
    assert_eq!(&target[..49], &source[..49]);
    assert_eq!(&target[49..], &[0xa5; 7]);
    let buffer = heap.descriptor(&target[..16]).unwrap();
    assert_eq!(heap.bytes(&buffer).unwrap(), [0, 128, 255]);
    heap.file_descriptor(&target[16..32]).unwrap();
    let vector = heap
        .vector_descriptor(&target[32..48], byte_buffer::VectorElement::I128)
        .unwrap();
    let payload = heap
        .vector_range(&vector, byte_buffer::VectorElement::I128, 0, 1)
        .unwrap();
    assert_eq!(payload, i128::MIN.to_le_bytes());
    assert_eq!(payload.as_ptr() as usize % 16, 0);
    release(&declarations, &mut heap, aggregate, &target, span).unwrap();
    assert_eq!(
        (
            heap.ledger().live_allocations,
            heap.ledger().live_bytes,
            heap.ledger().releases
        ),
        (0, 0, 3)
    );
    assert!(release(&declarations, &mut heap, aggregate, &source, span).is_err());
    assert_eq!(heap.ledger().releases, 3);
}

#[test]
fn vector_reference_late_wrong_element_refuses_before_any_mixed_release_or_copy() {
    let (declarations, span) = declarations();
    let aggregate = AggregateTy::Record(RecordId(0));
    let mut heap = byte_buffer::Heap::default();
    // Identical widths and valid scalar bits cannot substitute U128 for I128.
    let (_path, source) = mixed_value(&mut heap, byte_buffer::VectorElement::U128);
    let before = heap.ledger();
    let mut target = [0xa5; 56];
    let transfer = preflight(&declarations, &heap, aggregate, &source, target.len(), span)
        .and_then(|()| copy_active_bytes(&declarations, aggregate, &source, &mut target, span));
    assert!(transfer.is_err());
    assert_eq!(target, [0xa5; 56]);
    assert!(release(&declarations, &mut heap, aggregate, &source, span).is_err());
    assert_eq!(heap.ledger(), before);
    assert_eq!(heap.ledger().releases, 0);
    heap.descriptor(&source[..16]).unwrap();
    heap.file_descriptor(&source[16..32]).unwrap();
    let vector = heap
        .vector_descriptor(&source[32..48], byte_buffer::VectorElement::U128)
        .unwrap();
    assert_eq!(
        heap.vector_validate(&vector, byte_buffer::VectorElement::I128),
        Err(byte_buffer::Failure::Descriptor)
    );
    assert_eq!(
        heap.vector_range(&vector, byte_buffer::VectorElement::U128, 0, 1)
            .unwrap(),
        i128::MIN.to_le_bytes()
    );
    heap.cleanup();
    assert_eq!(
        (
            heap.ledger().live_allocations,
            heap.ledger().live_bytes,
            heap.ledger().releases
        ),
        (0, 0, 3)
    );
}

#[test]
fn vector_reference_enum_walk_ignores_inactive_poison_and_releases_only_active_mixed_leaves() {
    let (declarations, span) = declarations();
    let aggregate = AggregateTy::Enum(EnumId(0));
    let mut heap = byte_buffer::Heap::default();
    let (_path, record) = mixed_value(&mut heap, byte_buffer::VectorElement::I128);
    let mut inactive = [0xff; 64];
    inactive[..4].copy_from_slice(&1u32.to_le_bytes());
    release(&declarations, &mut heap, aggregate, &inactive, span).unwrap();
    assert_eq!(
        (heap.ledger().live_allocations, heap.ledger().releases),
        (3, 0)
    );
    let mut source = [0xcc; 64];
    source[..4].copy_from_slice(&0u32.to_le_bytes());
    source[8..].copy_from_slice(&record);
    let mut target = [0xa5; 64];
    preflight(&declarations, &heap, aggregate, &source, target.len(), span).unwrap();
    copy_active_bytes(&declarations, aggregate, &source, &mut target, span).unwrap();
    assert_eq!(&target[..4], &0u32.to_le_bytes());
    assert_eq!(&target[4..8], &[0xa5; 4]);
    assert_eq!(&target[8..57], &source[8..57]);
    assert_eq!(&target[57..], &[0xa5; 7]);
    release(&declarations, &mut heap, aggregate, &target, span).unwrap();
    assert_eq!(
        (
            heap.ledger().live_allocations,
            heap.ledger().live_bytes,
            heap.ledger().releases
        ),
        (0, 0, 3)
    );
}
