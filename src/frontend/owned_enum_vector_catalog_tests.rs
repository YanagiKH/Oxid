use super::*;
use std::mem::{align_of, size_of};

#[test]
fn owned_enum_vector_keys_exhaustively_preserve_record_encoding_and_are_disjoint() {
    assert_eq!((size_of::<Function>(), align_of::<Function>()), (2, 1));
    let mut seen = [false; 65536];
    for ordinal in 0..4096 {
        for operation in Operation::ALL {
            let record = Function::record(operation, RecordId(ordinal)).unwrap();
            let enumeration = Function::enumeration(operation, EnumId(ordinal)).unwrap();
            let old = ordinal as u16 * 8 + operation as u16;
            assert_eq!(record.raw(), old);
            assert_eq!(enumeration.raw(), old | 0x8000);
            assert_ne!(record, enumeration);
            assert_eq!(record.element(), OwnedVectorRoot::Record(RecordId(ordinal)));
            assert_eq!(
                enumeration.element(),
                OwnedVectorRoot::Enum(EnumId(ordinal))
            );
            for key in [record, enumeration] {
                assert!(!seen[key.raw() as usize]);
                seen[key.raw() as usize] = true;
                assert_eq!(Function::from_raw(key.raw() as u32), Some(key));
                assert_eq!(key.operation(), operation);
            }
        }
    }
    assert!(seen.into_iter().all(|present| present));
    for invalid in [4096, 65535, usize::MAX] {
        for operation in Operation::ALL {
            assert!(Function::record(operation, RecordId(invalid)).is_none());
            assert!(Function::enumeration(operation, EnumId(invalid)).is_none());
        }
    }
    for invalid in [65536, 0x8000_0000, u32::MAX] {
        assert!(Function::from_raw(invalid).is_none());
    }
}

#[test]
fn owned_enum_vector_concrete_signatures_never_reinterpret_an_enum_as_a_record() {
    let vector = AggregateTy::OwnedEnumVector(EnumId(23));
    for operation in Operation::ALL {
        let key = Function::enumeration(operation, EnumId(23)).unwrap();
        let signature = key.concrete_signature(EnumId(91));
        if operation != Operation::New {
            assert!(matches!(signature.parameters()[0], ParameterTy::Reference {
                referent: BorrowedTy::Exact(actual), ..
            } if actual == vector));
        }
        match operation {
            Operation::New | Operation::Take => {
                assert_eq!(signature.result, ValueTy::Owned(vector))
            }
            Operation::PopLast => assert_eq!(
                signature.result,
                ValueTy::Owned(AggregateTy::Enum(EnumId(23)))
            ),
            Operation::PushReserved => assert_eq!(
                signature.parameters()[1],
                ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(EnumId(23))))
            ),
            Operation::TryReserve => assert_eq!(
                signature.result,
                ValueTy::Owned(AggregateTy::Enum(EnumId(91)))
            ),
            Operation::Len | Operation::Capacity => {
                assert_eq!(signature.result, ValueTy::Scalar(hir::Ty::I32))
            }
            Operation::Truncate => assert_eq!(signature.result, ValueTy::Scalar(hir::Ty::Unit)),
        }
    }
}
