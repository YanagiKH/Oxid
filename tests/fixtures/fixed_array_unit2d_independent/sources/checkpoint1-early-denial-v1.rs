#[test]
fn independent_unit2d_checkpoint1_early_denial_peak() {
    let (sources, s) = fixtures::context();
    let mut f = fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    f.locals = vec![fixtures::scalar(hir::Ty::I32, s(0))];
    f.blocks = vec![block(
        vec![fixtures::assign(0, Rvalue::I32(1), s(1))],
        OwnedTerminatorKind::ReturnScalar(fixtures::operand(0, s(2))),
        s(2),
    )];
    let witness = verified::verify_owned(
        RawOwnedProgram { records: vec![], functions: vec![f] },
        &sources,
    ).unwrap();
    let mut accounting = Accounting { fail_after: Some(0), ..Accounting::default() };
    let error = native_module_accounted(
        &witness, Some(hir::DefId(0)), &sources, 1_000_000,
        Limits { function_slots: 0, ..Limits::DEFAULT }, &mut accounting,
    ).unwrap_err();
    assert_eq!(error.code, "E0700");
    assert!(error.message.contains("scalar slots per function"));
    assert_eq!(accounting.metrics.allocation_attempts, 0);
    assert_eq!(accounting.metrics.plan_bytes, 184);
    assert_eq!(accounting.metrics.admission_scratch_peak, 32); // caller header 24 + remaining 8
    assert_eq!(accounting.metrics.metadata_peak, 216);
    assert_eq!(accounting.metrics.occurrences, 0);
    assert_eq!(accounting.metrics.count_bytes, 0);
}
