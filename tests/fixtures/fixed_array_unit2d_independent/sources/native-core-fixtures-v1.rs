// Independent native scalar/index model, frozen before candidate observation.
// This file contains raw adapters and expected scalar sequences only.
use super::*;

fn native_core_value(ty: hir::Ty, ordinal: usize) -> Scalar {
    match ty {
        hir::Ty::Bool => Scalar::Bool(ordinal % 2 != 0),
        hir::Ty::I32 => Scalar::I32((if ordinal % 2 == 0 { 1 } else { -1 }) * (31 + 17 * ordinal as i32)),
        hir::Ty::Unit => Scalar::Unit,
    }
}
fn native_core_replacement(ty: hir::Ty) -> Scalar {
    match ty { hir::Ty::Bool => Scalar::Bool(true), hir::Ty::I32 => Scalar::I32(i32::MIN), hir::Ty::Unit => Scalar::Unit }
}
// Mode0 length, mode1 read, mode2 write followed by read. N1024 uses two
// repeated immutable operands; it never asks native admission for1024 locals.
fn native_core_fixture(ty: hir::Ty,n: usize,index: i32,mode: usize) -> (SourceMap,RawOwnedProgram,Option<Scalar>,Span,Vec<(Span,usize)>) {
    let mut sources=SourceMap::new();
    let file=sources.add("unit2d-independent-core.ox".into(),"abcdef\n".repeat(128));
    let at=|i:usize| Span{file,start:i*7,end:i*7+6};
    let literal_count=if n>4 { 2 } else { n };
    let aggregate=AggregateTy::FixedArray(FixedArrayTy::check(ty,n).unwrap());
    let mut f=fixtures::function(0,ValueTy::Scalar(if mode==0 {hir::Ty::I32} else {ty}),at(0));
    f.locals=(0..literal_count).map(|_| fixtures::scalar(ty,at(0))).collect();
    f.locals.extend([hir::Ty::I32,ty,hir::Ty::I32,ty].into_iter().map(|t|fixtures::scalar(t,at(0))));
    f.owners=vec![OwnerDecl{aggregate:AggregateSlot::try_from_aggregate(aggregate).unwrap(),kind:OwnerKind::Local{mutable:true},span:at(0)}];
    let index_local=literal_count; let replacement_local=literal_count+1; let length_local=literal_count+2; let result_local=literal_count+3;
    let mut body=vec![]; let width=n.max(1);
    let mut schedule=vec![(at(0),1+(literal_count+4)+width+4)];
    let scalar_rvalue=|s|match s{Scalar::Bool(v)=>Rvalue::Bool(v),Scalar::I32(v)=>Rvalue::I32(v),Scalar::Unit=>Rvalue::Unit};
    for j in 0..literal_count { body.push(fixtures::assign(j,scalar_rvalue(native_core_value(ty,j)),at(1+j)));schedule.push((at(1+j),1)); }
    body.extend([
        fixtures::assign(index_local,Rvalue::I32(index),at(10)),
        fixtures::assign(replacement_local,scalar_rvalue(native_core_replacement(ty)),at(11)),
        fixtures::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)),at(12)),
        fixtures::instruction(OwnedInstruction::ConstructArray{destination:OwnerPlaceId(0),elements:(0..n).map(|j|fixtures::operand(if n>4 {j%2} else {j},at(13))).collect()},at(14)),
    ]);
    schedule.extend([(at(10),1),(at(11),1),(at(12),1),(at(14),1+width)]);
    let access=at(if mode==0 {15} else if mode==1 {16} else {17});
    if mode==0 {
        body.push(fixtures::instruction(OwnedInstruction::ArrayLength{destination:LocalId(length_local),base:AccessBase::Owner(OwnerPlaceId(0))},access));
        schedule.push((access,1));
    } else {
        if mode==2 {body.push(fixtures::instruction(OwnedInstruction::WriteIndex{base:AccessBase::Owner(OwnerPlaceId(0)),index:fixtures::operand(index_local,at(18)),value:fixtures::operand(replacement_local,at(19))},access));schedule.push((access,1));}
        let read_span=if mode==2 {at(20)} else {access};
        body.push(fixtures::instruction(OwnedInstruction::ReadIndex{destination:LocalId(result_local),base:AccessBase::Owner(OwnerPlaceId(0)),index:fixtures::operand(index_local,at(21))},read_span));schedule.push((read_span,1));
    }
    f.blocks=vec![block(body,OwnedTerminatorKind::ReturnScalar(fixtures::operand(if mode==0 {length_local} else {result_local},at(22))),at(22))];
    schedule.push((at(22),1+width));
    let expected=if mode==0 {Some(Scalar::I32(n as i32))} else if index>=0 && (index as usize)<n {Some(if mode==2 {native_core_replacement(ty)} else {native_core_value(ty,if n>4 {index as usize%2} else {index as usize})})} else {None};
    (sources,RawOwnedProgram{records:vec![],functions:vec![f]},expected,access,schedule)
}
