#[cfg(test)]
mod record_adapter_controls {
    use super::*;
    #[test]
    fn record_adapter_scalar_fields_keep_historical_names() {
        for (ty, name) in [(Ty::Bool, "bool"), (Ty::I32, "i32"), (Ty::Unit, "unit")] {
            assert_eq!(historical_field_scalar(ValueTy::Scalar(ty)).unwrap(), name);
        }
    }
    #[test]
    fn record_adapter_owned_fields_and_slice_referents_fail_closed() {
        let array = AggregateTy::FixedArray(FixedArrayTy::check(Ty::I32, 1).unwrap());
        for aggregate in [AggregateTy::Record(RecordId(0)), array] {
            assert!(historical_field_scalar(ValueTy::Owned(aggregate)).is_err());
            assert_eq!(historical_referent(BorrowedTy::Exact(aggregate)).unwrap(), aggregate);
        }
        for ty in [Ty::Bool, Ty::I32, Ty::Unit] { assert!(historical_referent(BorrowedTy::ScalarSlice(ty)).is_err()); }
    }
    #[test]
    fn record_adapter_projection_json_is_unchanged_and_borrowed() {
        assert_eq!(ProjectionRow(None).to_string(), "null");
        let field = FieldId { record: RecordId(2), index: 3 };
        for (base, mode) in [(AccessBase::Owner(BindingId(7)), "owner"),
            (AccessBase::Reference { binding: BindingId(7), kind: BorrowKind::Shared }, "shared"),
            (AccessBase::Reference { binding: BindingId(7), kind: BorrowKind::Exclusive }, "exclusive")] {
            let projection = Projection { base, field, path: vec![field] };
            let actual = ProjectionRow(Some(&projection)).to_string();
            assert_eq!(actual, format!("{{\"binding\":7,\"mode\":\"{mode}\",\"record\":2,\"field\":3}}"));
        }
    }
    #[test]
    fn record_adapter_empty_multihop_and_mismatched_paths_fail_closed() {
        let field = FieldId { record: RecordId(2), index: 3 };
        let other = FieldId { record: RecordId(4), index: 5 };
        for path in [vec![], vec![field, other], vec![other]] {
            let projection = Projection { base: AccessBase::Owner(BindingId(7)), field, path };
            assert!(write!(&mut String::new(), "{}", ProjectionRow(Some(&projection))).is_err());
        }
    }
}
