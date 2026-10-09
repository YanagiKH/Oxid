//! Primitive value admission must not expand predecessor aggregate allowlists.
use super::tests::{record, source};
use super::*;

#[test]
fn u8_standalone_value_and_parameter_admit_but_all_aggregate_paths_reject() {
    let (sources, span) = source();
    let empty = Declarations::check(&[], &sources).unwrap();
    assert!(empty.check_value_type(ValueTy::Scalar(hir::Ty::U8)).is_ok());
    assert!(empty
        .check_parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::U8)))
        .is_ok());
    for n in [0, 1, 1024] {
        assert_eq!(
            FixedArrayTy::check(hir::Ty::U8, n),
            Err(DeclarationError::TypeMismatch)
        );
        // Independent malformed internal structural descriptor, including length zero.
        let array = FixedArrayTy {
            element: hir::Ty::U8,
            length: n as u16,
        };
        assert_eq!(
            empty.check_aggregate_type(AggregateTy::FixedArray(array)),
            Err(DeclarationError::TypeMismatch)
        );
        assert_eq!(
            empty.check_borrowed_type(BorrowedTy::Exact(AggregateTy::FixedArray(array))),
            Err(DeclarationError::TypeMismatch)
        );
    }
    let borrow = BorrowedTy::ScalarSlice(hir::Ty::U8);
    assert!(!borrow.accepts(borrow));
    assert_eq!(
        BorrowedSlot::check(borrow),
        Err(DeclarationError::TypeMismatch)
    );
    assert_eq!(
        empty.check_borrowed_type(borrow),
        Err(DeclarationError::TypeMismatch)
    );
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
