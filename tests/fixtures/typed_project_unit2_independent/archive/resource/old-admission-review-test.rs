#[test]
fn reviewer_old_declaration_admission_precedes_all_new_index_gates() {
    let cases=[(0..4097).map(|i|format!("struct R{i}{{}}\n")).collect::<String>(),format!("struct R{{{}}}",(0..1025).map(|i|format!("f{i}:i32")).collect::<Vec<_>>().join(","))];
    for text in cases {let fixture=Fixture::new(&[("root.ox",&text)]);let p=fixture.load();let w=WorkMeter::new(0);let mut a=Allocator::default();let at=p.sources().get(SourceFileId(0)).span(0,0);
        let error=collect_originals(SourceOwner::project(&p),IndexLimits{retained:0,scratch:0,work:0},&w,&mut a).unwrap_err();
        assert_eq!((error.code,error.stage,error.message.as_str(),error.primary),("E0400","resolve","owned declaration resource limit exceeded",Some(at)));assert_eq!(a.attempts,0);assert_eq!(w.used(),0);
    }
}
