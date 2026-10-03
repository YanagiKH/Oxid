// Reviewer-owned extension of the unchanged independently frozen Unit2C chain.
// Adds source-like a=a through a distinct Temporary before argument staging.
fn self_replacement_chain(ty: hir::Ty,n: usize) -> (SourceMap,RawOwnedProgram,Vec<(Span,usize)>) {
    let (sources,mut raw,mut events)=chain_fixture(ty,n);
    let base=raw.functions[0].span;
    let f=&mut raw.functions[0];
    assert_eq!(f.owners.len(),12);
    f.owners.push(own(array(ty,n),OwnerKind::Temporary,base));
    f.owners.push(own(AggregateTy::Record(RecordId(0)),OwnerKind::Local{mutable:false},base));
    let at=f.blocks[0].statements.iter().position(|x|matches!(x.kind,OwnedInstruction::OpenCall(_))).unwrap();
    let field=FieldId{record:RecordId(0),index:0};
    let added=[
        ins(OwnedInstruction::StorageLive(OwnerPlaceId(13)),s(base,40)),
        ins(OwnedInstruction::Construct{destination:OwnerPlaceId(13),fields:vec![(field,op(0,base))]},s(base,41)),
        ins(OwnedInstruction::StorageLive(OwnerPlaceId(12)),s(base,42)),
        ins(OwnedInstruction::MoveInitialize{destination:OwnerPlaceId(12),source:OwnerPlaceId(0)},s(base,43)),
        ins(OwnedInstruction::Replace{destination:OwnerPlaceId(0),source:OwnerPlaceId(12)},s(base,44)),
    ];
    f.blocks[0].statements.splice(at..at,added);
    let width=n.max(1);
    events[0].1+=width+9; // extra array+record owner cells and 8 owner-state cells
    let event_at=events.iter().position(|(p,_)|*p==s(base,60)).unwrap();
    events.splice(event_at..event_at,[(s(base,40),1),(s(base,41),2),(s(base,42),1),(s(base,43),1+width),(s(base,44),1+2*width)]);
    events.last_mut().unwrap().1+=width+1; // return drops both new owner widths
    (sources,raw,events)
}
