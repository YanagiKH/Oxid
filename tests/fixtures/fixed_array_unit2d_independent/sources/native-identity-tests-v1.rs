#[test]
fn independent_unit2d_full_identity_and_closed_production_gate() {
    let mut identities: Vec<_> = [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit].into_iter()
        .flat_map(|ty| [0usize,1,4].into_iter().map(move |n| array(ty,n))).collect();
    identities.push(AggregateTy::Record(RecordId(0)));
    let mut valid = 0;
    let mut denied = 0;
    let mut production_denials = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0usize,1,4] {
            for &expected in &identities {
                let (sources,mut raw,_) = chain_fixture(ty,n);
                let call_span = s(raw.functions[0].span,60);
                raw.functions[1].owners[0].aggregate = AggregateSlot::try_from_aggregate(expected).unwrap();
                raw.functions[1].result = ValueTy::Owned(expected);
                let result = verified::probe_array_native(
                    raw,&sources,budget::Limits::DEFAULT,Some(hir::DefId(0)),&sources,
                    NativeControl { fail_after: (expected != array(ty,n)).then_some(0), ..NativeControl::default() },
                );
                if expected == array(ty,n) {
                    assert!(result.unwrap().result.is_ok());
                    valid += 1;
                } else {
                    let error = match result { Err(e) => e, Ok(_) => panic!("wrong full aggregate identity entered native consumer") };
                    assert_eq!(error.kind,OwnedFailureKind::Malformed(Malformed::Type));
                    assert_eq!(error.primary.get(),Some(call_span));
                    denied += 1;
                }
            }
            let (sources,raw,_) = chain_fixture(ty,n);
            let error = verified::verify_owned(raw,&sources).unwrap_err();
            assert_eq!(error.kind,OwnedFailureKind::Malformed(Malformed::UnsupportedArray));
            production_denials += 1;
        }
    }
    assert_eq!((valid,denied,production_denials),(9,81,9));
    eprintln!("independent native identity: 9 valid pairs,81 wrong T/N/nominal pairs denied before native allocation;9 distinct production gate controls");
}
#[test]
#[ignore = "independent Unit2D cross-file structural identity real LLVM gate"]
fn independent_unit2d_cross_file_identity_llvm() {
    let (mut sources,mut raw,_) = chain_fixture(hir::Ty::I32,4);
    let file = sources.add("same-array-another-file.ox".into(),"abc\n".repeat(16384));
    let callee = &mut raw.functions[1];
    callee.span.file = file;
    callee.owners[0].span.file = file;
    callee.blocks[0].span.file = file;
    callee.blocks[0].terminator.as_mut().unwrap().span.file = file;
    let module = independent_native(raw,&sources,1_000_000);
    let scratch = Scratch::new();
    let binary = scratch.compile(&module,"cross-file-identity");
    independent_assert_output(independent_run(&scratch,&binary,"cross-file-identity",&[]),Some(Scalar::I32(4)),None);
}
