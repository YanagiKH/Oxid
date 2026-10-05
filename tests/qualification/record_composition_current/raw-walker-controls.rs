#[cfg(test)]
mod record_adapter_controls {
    use super::*;
    fn span() -> Span { Span { file: crate::frontend::source::SourceFileId(0), start: 0, end: 1 } }
    fn function() -> RawOwnedFunction {
        RawOwnedFunction { id: hir::DefId(0), span: span(), result: ValueTy::Scalar(hir::Ty::Unit),
            parameters: vec![], locals: vec![], places: vec![], owners: vec![], references: vec![],
            calls: vec![], loans: vec![], entry: BlockId(0), blocks: vec![] }
    }
    fn denies(raw: &RawOwnedProgram) { assert!(std::panic::catch_unwind(|| historical_domain(raw)).is_err()); }
    #[test]
    fn record_adapter_rejects_each_new_instruction_variant() {
        let base = AccessBase::Owner(OwnerPlaceId(0));
        let value = Operand { local: LocalId(0), span: span() };
        let invalid = [
            OwnedInstruction::ConstructComposite { destination: OwnerPlaceId(0), fields: vec![] },
            OwnedInstruction::ReadProjection { destination: LocalId(0), base, path: vec![], index: None },
            OwnedInstruction::WriteProjection { base, path: vec![], index: None, value },
            OwnedInstruction::ProjectionLength { destination: LocalId(0), base, path: vec![] },
            OwnedInstruction::ConstructArray { destination: OwnerPlaceId(0), elements: vec![] },
            OwnedInstruction::ReadIndex { destination: LocalId(0), base, index: value },
            OwnedInstruction::WriteIndex { base, index: value, value },
            OwnedInstruction::ArrayLength { destination: LocalId(0), base },
        ];
        for instruction in invalid {
            assert!(!historical_instruction(&instruction));
            let mut function = function();
            function.blocks.push(OwnedBlock { merge: None, span: span(), statements: vec![
                OwnedStatement { kind: instruction, span: span(), diagnostic_origins: None }], terminator: None });
            denies(&RawOwnedProgram { records: vec![], functions: vec![function] });
        }
        assert!(historical_instruction(&OwnedInstruction::StorageLive(OwnerPlaceId(0))));
    }
    #[test]
    fn record_adapter_rejects_owned_and_reference_stored_fields() {
        let array = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 1).unwrap());
        let record = AggregateTy::Record(RecordId(0));
        for ty in [ParameterTy::Value(ValueTy::Owned(record)), ParameterTy::Value(ValueTy::Owned(array)),
            ParameterTy::Reference { referent: BorrowedTy::Exact(record), kind: BorrowKind::Shared },
            ParameterTy::Reference { referent: BorrowedTy::ScalarSlice(hir::Ty::I32), kind: BorrowKind::Exclusive }] {
            denies(&RawOwnedProgram { records: vec![RawRecordDecl { id: RecordId(0), span: span(), fields: vec![
                RawFieldDecl { id: FieldId { record: RecordId(0), index: 0 }, ty, span: span() }] }], functions: vec![] });
        }
        for scalar in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
            historical_domain(&RawOwnedProgram { records: vec![RawRecordDecl { id: RecordId(0), span: span(), fields: vec![
                RawFieldDecl { id: FieldId { record: RecordId(0), index: 0 }, ty: ParameterTy::Value(ValueTy::Scalar(scalar)), span: span() }] }], functions: vec![] });
        }
    }
    #[test]
    fn record_adapter_rejects_array_owners_results_and_borrowed_views() {
        let array = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 1).unwrap());
        let mut f = function(); f.result = ValueTy::Owned(array);
        denies(&RawOwnedProgram { records: vec![], functions: vec![f] });
        let mut f = function(); f.owners.push(OwnerDecl { aggregate: AggregateSlot::try_from_aggregate(array).unwrap(), kind: OwnerKind::Temporary, span: span() });
        denies(&RawOwnedProgram { records: vec![], functions: vec![f] });
        for referent in [BorrowedTy::Exact(array), BorrowedTy::ScalarSlice(hir::Ty::I32)] {
            let mut f = function(); f.references.push(ReferenceDecl { referent: BorrowedSlot::check(referent).unwrap(), kind: BorrowKind::Shared, position: 0, span: span() });
            denies(&RawOwnedProgram { records: vec![], functions: vec![f] });
            let mut f = function(); f.loans.push(LoanDecl { call: CallSiteId(0), argument: 0, authority: AccessBase::Owner(OwnerPlaceId(0)), kind: BorrowKind::Shared, referent: BorrowedSlot::check(referent).unwrap(), span: span() });
            denies(&RawOwnedProgram { records: vec![], functions: vec![f] });
        }
        historical_domain(&RawOwnedProgram { records: vec![], functions: vec![function()] });
    }
    #[test]
    fn record_adapter_disabled_journal_does_not_reject_current_domain() {
        assert!(!journal::enabled());
        let mut f = function(); f.result = ValueTy::Owned(AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 1).unwrap()));
        register(&RawOwnedProgram { records: vec![], functions: vec![f] });
    }
}
