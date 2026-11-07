use super::tests::{record, source};
use super::*;

#[test]
fn checked_copy_declarations_accept_all_scalars_and_only_marked_record_closures() {
    let (sources, span) = source();
    let scalars = [
        hir::Ty::Unit,
        hir::Ty::Bool,
        hir::Ty::I8,
        hir::Ty::I16,
        hir::Ty::I32,
        hir::Ty::I64,
        hir::Ty::I128,
        hir::Ty::U8,
        hir::Ty::U16,
        hir::Ty::U32,
        hir::Ty::U64,
        hir::Ty::U128,
        hir::Ty::Isize,
        hir::Ty::Usize,
        hir::Ty::F32,
        hir::Ty::F64,
        hir::Ty::Char,
    ];
    let mut inner = record(0, &scalars, span);
    inner.is_copy = true;
    let mut outer = record(1, &[hir::Ty::Unit], span);
    outer.is_copy = true;
    outer.fields[0].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(0))));
    let declarations = Declarations::check(&[inner, outer], &sources).unwrap();
    assert!(declarations.is_copy(AggregateTy::Record(RecordId(0))));
    assert!(declarations.is_copy(AggregateTy::Record(RecordId(1))));
    assert!(!declarations.is_copy(AggregateTy::Record(RecordId(2))));
    assert_eq!(
        declarations
            .leaves(AggregateTy::Record(RecordId(1)))
            .unwrap()
            .count(),
        17
    );
}

#[test]
fn checked_copy_declarations_reject_forged_resource_array_reference_and_cycle_closures() {
    let (sources, span) = source();
    for field in [
        ParameterTy::Value(ValueTy::Owned(AggregateTy::ByteBuf)),
        ParameterTy::Value(ValueTy::Owned(AggregateTy::File)),
        ParameterTy::Value(ValueTy::Owned(AggregateTy::Vector(hir::Ty::I32))),
        ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(
            FixedArrayTy::check(hir::Ty::I32, 0).unwrap(),
        ))),
        ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
            kind: BorrowKind::Shared,
        },
    ] {
        let mut declaration = record(0, &[hir::Ty::Unit], span);
        declaration.is_copy = true;
        declaration.fields[0].ty = field;
        assert!(
            Declarations::check(&[declaration], &sources).is_err(),
            "{field:?}"
        );
    }
    for marked_inner in [false, true] {
        let mut inner = record(0, &[hir::Ty::Unit], span);
        inner.is_copy = marked_inner;
        let mut outer = record(1, &[hir::Ty::Unit], span);
        outer.is_copy = true;
        outer.fields[0].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(0))));
        if marked_inner {
            inner.fields[0].ty =
                ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(1))));
        }
        assert!(Declarations::check(&[inner, outer], &sources).is_err());
    }
}
