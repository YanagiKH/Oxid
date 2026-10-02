#[test]
fn reviewer_reference_exposure_uses_referent_origin_and_current_work_site() {
    for reference in ["&R","&mut R","&crate::c::R","&mut crate::c::R"] {
        let qualified=reference.contains("crate");let root=if qualified{format!("mod c; pub fn leak(x:{reference})->(){{return;}}") }else{format!("struct R{{}} pub fn leak(x:{reference})->(){{return;}}")};
        let files=if qualified{vec![("root.ox",root.as_str()),("c.ox","pub struct R{}")]}else{vec![("root.ox",root.as_str())]};let fixture=Fixture::new(&files);let p=fixture.load();let source=p.sources().get(SourceFileId(0));let name=if qualified{"crate::c::R"}else{"R"};let start=root.find(reference).unwrap()+reference.len()-name.len();let at=source.span(start,start+name.len());
        let errors=crate::frontend::oir::project::check_project_candidate(&p,IndexLimits::default(),&WorkMeter::default(),&mut Allocator::default()).unwrap_err();assert_eq!((errors[0].code,errors[0].stage,errors[0].primary),("E0207","resolve",Some(at)));
        let work=WorkMeter::default();let mut allocator=Allocator::default();let index=collect_originals(SourceOwner::project(&p),IndexLimits::default(),&work,&mut allocator).unwrap().finish(&work,&mut allocator).unwrap();
        check_error(&index.query(&WorkMeter::new(0)).signature_exposure(DefId(0),RecordId(0),at).unwrap_err(),"declaration index work limit exceeded",at);
        assert!(matches!(index.query(&WorkMeter::new(1)).signature_exposure(DefId(0),RecordId(0),at).unwrap(),Exposure::Denied{..}));
    }
}
