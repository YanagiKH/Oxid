//! Current RFC0031 successor of RFC0030 primitive-only aggregate exclusions.
use super::tests::{record, source};
use super::*;

#[test]
fn u8_standalone_value_and_parameter_byte_storage_successor() {
    let (sources, span) = source();
    let empty = Declarations::check(&[], &sources).unwrap();
    assert!(empty.check_value_type(ValueTy::Scalar(hir::Ty::U8)).is_ok());
    assert!(empty
        .check_parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::U8)))
        .is_ok());
    // RFC0030 historical array/slice expectation: TypeMismatch at every
    // independent check below, and false from BorrowedTy::accepts. RFC0031
    // replaces only those standalone exclusions. The declaration fences stay.
    const PREDECESSOR: DeclarationError = DeclarationError::TypeMismatch;
    for n in [0, 1, 1024] {
        let array = FixedArrayTy::check(hir::Ty::U8, n)
            .unwrap_or_else(|error| panic!("successor of {PREDECESSOR:?}: {error:?}"));
        assert_eq!(
            empty.check_aggregate_type(AggregateTy::FixedArray(array)),
            Ok(())
        );
        assert_eq!(
            empty.check_borrowed_type(BorrowedTy::Exact(AggregateTy::FixedArray(array))),
            Ok(())
        );
    }
    let borrow = BorrowedTy::ScalarSlice(hir::Ty::U8);
    assert!(borrow.accepts(borrow));
    assert_eq!(BorrowedSlot::check(borrow).unwrap().referent(), borrow);
    assert_eq!(empty.check_borrowed_type(borrow), Ok(()));
    assert!(Declarations::check(&[record(0, &[hir::Ty::U8], span)], &sources).is_err());
    let enumeration = RawEnumDecl {
        id: EnumId(0),
        span,
        variants: vec![RawVariantDecl {
            id: VariantId {
                enumeration: EnumId(0),
                index: 0,
            },
            span,
            payload: Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::U8))),
        }],
    };
    assert!(Declarations::check_combined(&[], &[enumeration], &sources).is_err());
}

#[test]
fn raw_nominal_declarations_cannot_claim_reserved_u8_source_names() {
    let mut sources = SourceMap::new();
    let id = sources.add("reserved-raw.ox".into(), "u8".into());
    let span = sources.get(id).span(0, 2);
    assert!(Declarations::check(&[record(0, &[], span)], &sources).is_err());
    let enumeration = RawEnumDecl {
        id: EnumId(0),
        span,
        variants: vec![RawVariantDecl {
            id: VariantId {
                enumeration: EnumId(0),
                index: 0,
            },
            span,
            payload: None,
        }],
    };
    assert!(Declarations::check_combined(&[], &[enumeration], &sources).is_err());
}
