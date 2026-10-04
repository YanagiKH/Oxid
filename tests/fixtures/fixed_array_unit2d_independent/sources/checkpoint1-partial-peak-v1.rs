#[test]
fn independent_unit2d_checkpoint1_partial_allocation_peaks() {
    let (sources,s)=fixtures::context();
    let mut f=fixtures::function(0,ValueTy::Scalar(hir::Ty::I32),s(0));
    f.locals=(0..11).map(|_|fixtures::scalar(hir::Ty::I32,s(0))).collect();
    let mut body=vec![fixtures::assign(0,Rvalue::I32(0),s(0)),fixtures::assign(1,Rvalue::I32(1),s(1))];
    for j in 0..9 {body.push(fixtures::assign(j+2,Rvalue::CheckedI32{op:hir::ArithmeticOp::Add,left:fixtures::operand(0,s(j+2)),right:fixtures::operand(1,s(j+2)),operator_span:s(j+2)},s(j+2)));}
    f.blocks.push(block(body,OwnedTerminatorKind::ReturnScalar(fixtures::operand(10,s(12))),s(12)));
    let witness=verified::verify_owned(RawOwnedProgram{records:vec![],functions:vec![f]},&sources).unwrap();
    for (fail_after,phase,expected) in [(1,"diagnostic lookup",808),(2,"diagnostic headers",1168)] {
        let mut accounting=Accounting{fail_after:Some(fail_after),..Accounting::default()};
        let error=native_module_accounted(&witness,Some(hir::DefId(0)),&sources,1_000_000,Limits::DEFAULT,&mut accounting).unwrap_err();
        assert_eq!(error.code,"E0700");assert_eq!(accounting.metrics.failed_allocation,Some(phase));
        assert_eq!(accounting.metrics.occurrences,9);assert_eq!(accounting.metrics.unique,9);
        assert_eq!(accounting.metrics.metadata_peak,expected,"partial peak at {phase}: {:?}",accounting.metrics);
    }
}
