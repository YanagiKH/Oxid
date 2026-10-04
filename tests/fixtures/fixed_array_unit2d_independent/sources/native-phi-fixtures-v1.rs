// Independent raw diamond families: expected result is exactly the selected branch.
// The boolean merge's other input slot is never initialized on that execution.
fn native_phi_fixture(pattern: &str,take_left: bool,guarded: bool) -> (SourceMap,RawOwnedProgram) {
    use fixtures::*;
    let (sources,s)=context();
    let mut f=function(0,ValueTy::Scalar(hir::Ty::Bool),s(0));
    let types=[hir::Ty::Bool,hir::Ty::I32,hir::Ty::I32,hir::Ty::I32,hir::Ty::I32,hir::Ty::Bool,hir::Ty::Bool,hir::Ty::Bool,hir::Ty::I32,hir::Ty::I32,hir::Ty::I32];
    f.locals=types.into_iter().map(|t|scalar(t,s(0))).collect();
    f.owners=vec![OwnerDecl{aggregate:AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32,1).unwrap())).unwrap(),kind:OwnerKind::Local{mutable:false},span:s(0)}];
    let checked=||assign(3,Rvalue::CheckedI32{op:hir::ArithmeticOp::Add,left:operand(1,s(30)),right:operand(2,s(31)),operator_span:s(32)},s(33));
    let read=|destination,at|instruction(OwnedInstruction::ReadIndex{destination:LocalId(destination),base:AccessBase::Owner(OwnerPlaceId(0)),index:operand(1,s(at+1))},s(at));
    let mut left=vec![assign(5,Rvalue::Bool(true),s(20))];
    match pattern {
        "arithmetic_bounds"=>left.extend([checked(),read(4,40)]),
        "bounds_arithmetic"=>left.extend([read(4,40),checked()]),
        "bounds_bounds"=>left.extend([read(4,40),read(8,50)]),
        "bounds_suffix"=>left.extend([read(4,40),assign(9,Rvalue::I32(-9),s(60))]),
        "no_split"=>{},
        _=>panic!("unknown independent phi family"),
    }
    f.blocks=vec![
        block(vec![assign(0,Rvalue::Bool(take_left),s(1)),assign(1,Rvalue::I32(0),s(2)),assign(2,Rvalue::I32(7),s(3)),assign(10,Rvalue::I32(-1),s(4)),instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)),s(5)),instruction(OwnedInstruction::ConstructArray{destination:OwnerPlaceId(0),elements:vec![operand(2,s(6))]},s(7))],OwnedTerminatorKind::Branch{condition:operand(0,s(8)),then_block:BlockId(1),else_block:BlockId(2)},s(9)),
        block(left,OwnedTerminatorKind::Goto(BlockId(3)),s(70)),
        block(vec![assign(6,Rvalue::Bool(false),s(71))],OwnedTerminatorKind::Goto(BlockId(3)),s(72)),
        block(vec![],OwnedTerminatorKind::ReturnScalar(operand(7,s(74))),s(75)),
    ];
    f.blocks[3].merge=Some(BoolMerge{destination:LocalId(7),incoming:[MergeInput{predecessor:BlockId(1),value:operand(5,s(73))},MergeInput{predecessor:BlockId(2),value:operand(6,s(73))}],operator_span:s(73),span:s(73)});
    let mut raw=RawOwnedProgram{records:vec![],functions:vec![f]};
    if guarded {append_cycle(&mut raw);}
    (sources,raw)
}
fn native_untaken_bounds_fixture(take_left: bool,empty: bool,guarded: bool) -> (SourceMap,RawOwnedProgram) {
    let (sources,mut raw)=native_phi_fixture("no_split",take_left,guarded);
    let f=&mut raw.functions[0];
    if empty {
        f.owners[0].aggregate=AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32,0).unwrap())).unwrap();
        let OwnedInstruction::ConstructArray{elements,..}=&mut f.blocks[0].statements[5].kind else {unreachable!()};
        elements.clear();
    }
    let at=Span{start:160,end:161,..f.span};
    f.blocks[if take_left {2} else {1}].statements.push(fixtures::instruction(OwnedInstruction::ReadIndex{destination:LocalId(8),base:AccessBase::Owner(OwnerPlaceId(0)),index:fixtures::operand(10,at)},at));
    (sources,raw)
}
