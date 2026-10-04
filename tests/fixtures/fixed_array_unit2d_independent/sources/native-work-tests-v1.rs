fn independent_wide_array_loop(replacements: usize) -> (SourceMap,RawOwnedProgram) {
    use fixtures::*;
    let (sources,s) = context();
    let mut f = function(0,ValueTy::Scalar(hir::Ty::Unit),s(0));
    f.locals = vec![scalar(hir::Ty::I32,s(0))];
    let aggregate = AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32,64).unwrap())).unwrap();
    f.owners = vec![OwnerDecl { aggregate,kind:OwnerKind::Local{mutable:true},span:s(0) };2];
    let mut body = vec![assign(0,Rvalue::I32(17),s(0))];
    for owner in 0..2 {
        body.push(instruction(OwnedInstruction::StorageLive(OwnerPlaceId(owner)),s(0)));
        body.push(instruction(OwnedInstruction::ConstructArray{destination:OwnerPlaceId(owner),elements:vec![operand(0,s(0));64]},s(0)));
    }
    f.blocks.push(block(body,OwnedTerminatorKind::Goto(BlockId(1)),s(0)));
    f.blocks.push(block((0..replacements).map(|i|instruction(OwnedInstruction::Replace{source:OwnerPlaceId(i%2),destination:OwnerPlaceId((i+1)%2)},s(1))).collect(),OwnedTerminatorKind::Goto(BlockId(1)),s(1)));
    (sources,RawOwnedProgram{records:vec![],functions:vec![f]})
}
#[test]
fn independent_unit2d_expansion_identity_and_bounded_prefix() {
    for ty in [hir::Ty::Bool,hir::Ty::I32,hir::Ty::Unit] {
        for n in [0usize,1,4,1024] {
            let (sources,raw,_) = chain_fixture(ty,n);
            let observation = verified::probe_array_native(raw,&sources,budget::Limits::DEFAULT,Some(hir::DefId(0)),&sources,NativeControl::default()).unwrap();
            let module = observation.result.unwrap();
            let metrics = observation.metrics;
            let expected_cells = 9*n.max(1)+6; // nine array writes/transfers plus six one-field guard constructors
            assert_eq!(metrics.transfer_cells,expected_cells);
            assert_eq!(metrics.message_bytes,0); // acyclic, no arithmetic/index sites; length has no bounds diagnostic
            assert_eq!(metrics.count_expansions,expected_cells);
            assert_eq!(metrics.render_expansions,expected_cells);
            assert_eq!(metrics.count_bytes,module.len());
            assert_eq!(metrics.render_bytes,module.len());
        }
    }
    let mut prefixes = vec![];
    for replacements in [32usize,32768] {
        let mut observations = vec![];
        for cap in [1024usize,8192,65536] {
            let (sources,raw) = independent_wide_array_loop(replacements);
            let observation = verified::probe_array_native(raw,&sources,budget::Limits::DEFAULT,Some(hir::DefId(0)),&sources,NativeControl{limits:Limits{ir_bytes:cap,..Limits::DEFAULT},fail_after:Some(5),..NativeControl::default()}).unwrap();
            let error = observation.result.unwrap_err();
            assert_eq!(error.code,"E0700");assert!(error.message.contains("LLVM bytes"));
            let metrics = observation.metrics;
            assert_eq!(metrics.occurrences,replacements+8);
            assert_eq!(metrics.unique,2);
            assert_eq!(metrics.allocation_attempts,5); // three inventories plus two unique messages; no LLVM allocation
            assert_eq!(metrics.failed_allocation,None);
            assert!(metrics.count_bytes<=cap);
            assert!(metrics.count_expansions<=cap+1);
            assert_eq!(metrics.render_bytes,0);
            assert_eq!(metrics.render_expansions,0);
            if cap==65536 {assert!(metrics.count_expansions>128,"must pass both constructor expansions");}
            observations.push((metrics.count_bytes,metrics.count_expansions,metrics.count_ordinary_visits,metrics.count_predecessor_visits));
        }
        prefixes.push(observations);
    }
    assert_eq!(prefixes[0],prefixes[1],"an exhausted emitter must not visit the oversized suffix");
    eprintln!("independent native expansion:12 exact site inventories;6 lowered-cap profiles with identical short/long prefixes: {:?}",prefixes[0]);
}
