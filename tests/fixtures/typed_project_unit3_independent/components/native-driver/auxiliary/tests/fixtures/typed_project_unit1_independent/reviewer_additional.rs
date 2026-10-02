#[test]
fn heldout_origin_bytes_and_three_file_labels() {
    let mut s=SourceMap::new();
    s.add("root\t.ox".into(),"aé🦀\r\nZ\t".into());
    s.add("dir\nb.ox".into(),"abcdefg\r\nQ".into());
    s.add("c\u{1b}.ox".into(),"é\r\nzz".into());
    let sp=|f,a,b|Span{file:SourceFileId(f),start:a,end:b};
    assert_eq!(s.try_text(sp(0,1,3)),Some("é"));
    assert_eq!(s.try_text(sp(1,1,3)),Some("bc"));
    for (f,a,b) in [(0,1,2),(0,2,3),(0,4,7),(0,10,9),(0,0,12),(3,0,0),(1,11,11)] {
        assert_eq!(s.try_text(sp(f,a,b)),None);
    }
    assert_eq!(s.get(SourceFileId(0)).try_text(sp(1,1,3)),None);
    assert_eq!(SourceView::Map(&s).text(sp(1,1,3)),"bc");
    assert_eq!(SourceView::Single(s.get(SourceFileId(0))).text(sp(0,1,3)),"é");
    for (offset,expected) in [(7,(1,4)),(8,(1,5)),(9,(2,1))] { assert_eq!(s.get(SourceFileId(0)).location(offset),expected); }
    assert_eq!(s.get(SourceFileId(2)).location(6),(2,3));
    let d=Diagnostic::new("E0201","resolve","independent origins",Some(sp(0,9,10)))
        .secondary(sp(1,1,3),"second").secondary(sp(2,4,6),"third");
    let human=d.render_human(&s);
    assert_eq!(human,"error[E0201] (resolve): independent origins\n  --> root\\t.ox:2:1\n  ::: dir\\nb.ox:1:2: second\n  ::: c\\u{1b}.ox:2:1: third\n");
    let j=d.render_json(&s);
    for expected in ["\"file_id\":0,\"path\":\"root\\t.ox\",\"start\":9,\"end\":10,\"line\":2,\"column\":1,\"end_line\":2,\"end_column\":2", "\"file_id\":1,\"path\":\"dir\\nb.ox\",\"start\":1,\"end\":3,\"line\":1,\"column\":2,\"end_line\":1,\"end_column\":4", "\"file_id\":2,\"path\":\"c\\u001b.ox\",\"start\":4,\"end\":6,\"line\":2,\"column\":1,\"end_line\":2,\"end_column\":3"] { assert!(j.contains(expected),"{j}"); }
}

#[test]
fn heldout_local_keys_keep_file_and_function_owners() {
    let p=load("fixtures","file_local_expr_blocks","app.ox").unwrap();
    for (file,name,value) in [(0,"root","7"),(1,"child","9")] {
        let f=FunctionAstKey{file:SourceFileId(file),index:0};
        assert_eq!(p.text(p.try_function(f).unwrap().name),name);
        let e=ExprKey{file:SourceFileId(file),expression:ast::ExprId(0)};
        assert_eq!(p.text(p.try_expression(e).unwrap().span),value);
        assert_eq!(p.try_block(BlockKey{function:f,block:ast::BodyBlockId(0)}).unwrap().span.file,SourceFileId(file));
        assert!(p.try_function(FunctionAstKey{file:SourceFileId(file),index:1}).is_none());
        assert!(p.try_block(BlockKey{function:f,block:ast::BodyBlockId(1)}).is_none());
    }
    assert!(p.try_file_ast(SourceFileId(2)).is_none());
    assert!(p.try_function(FunctionAstKey{file:SourceFileId(usize::MAX),index:0}).is_none());
    assert!(p.try_expression(ExprKey{file:SourceFileId(0),expression:ast::ExprId(usize::MAX)}).is_none());
    assert!(p.try_record(RecordAstKey{file:SourceFileId(0),index:0}).is_none());
    let q=load("fixtures","dfs_module_major","app.ox").unwrap();
    let b0=q.try_block(BlockKey{function:FunctionAstKey{file:SourceFileId(0),index:0},block:ast::BodyBlockId(0)}).unwrap();
    let b1=q.try_block(BlockKey{function:FunctionAstKey{file:SourceFileId(0),index:1},block:ast::BodyBlockId(0)}).unwrap();
    assert_eq!((b0.span.file,b1.span.file),(SourceFileId(0),SourceFileId(0)));assert!(b0.span.end<b1.span.start);
}

#[test]
fn heldout_frozen_bytes_survive_disk_replacement_and_removal() {
    let dir=PathBuf::from(REVIEW_ROOT).join("runtime-fixtures/frozen-after-load");
    std::fs::create_dir_all(&dir).unwrap();
    let root="mod a;\nfn r()->i32{return 7;}\n";
    let child="//é\r\nfn c()->i32{return 9;}\r\n";
    std::fs::write(dir.join("app.ox"),root).unwrap();std::fs::write(dir.join("a.ox"),child).unwrap();
    let p=ProjectSources::load_modules(dir.join("app.ox").to_str().unwrap(),ProjectLimits::default()).unwrap();
    std::fs::write(dir.join("app.ox"),"/* replaced").unwrap();std::fs::remove_file(dir.join("a.ox")).unwrap();
    assert_eq!(p.sources().get(SourceFileId(0)).text(),root);
    assert_eq!(p.sources().get(SourceFileId(1)).text(),child);
    let c=p.try_function(FunctionAstKey{file:SourceFileId(1),index:0}).unwrap();
    assert_eq!(p.text(c.name),"c");
    let d=Diagnostic::new("E0201","resolve","frozen",Some(c.name));
    assert!(d.render_human(p.sources()).contains("a.ox:2:4"));
    assert!(d.render_json(p.sources()).contains("\"line\":2,\"column\":4"));
}

fn trace_load(limits: ProjectLimits, fail_at: Option<usize>) -> (Result<ProjectSources,LoadFailure>,Allocator) {
    let mut a=Allocator{fail_at,..Allocator::default()};
    let r=ProjectSources::load(&entry("fixtures","minimal","app.ox"),limits,parser::SourceMode::ModuleCandidate,&mut a);
    (r,a)
}
#[test]
fn heldout_reduced_global_limits_and_preflight_traces() {
    let mut l=ProjectLimits::default();l.source_bytes=27;l.tokens=18;l.nodes=3;
    let (r,_)=trace_load(l,None);let p=r.unwrap();assert_eq!((p.usage().source_bytes,p.usage().non_eof_tokens,p.usage().syntax_nodes),(27,18,3));
    let cases=[("bytes",26,"E0400","source",(0,4,5)),("tokens",17,"E0400","lex",(1,19,20)),("nodes",2,"E0400","parse",(1,11,17))];
    for (kind,n,code,stage,span) in cases {
        let mut l=ProjectLimits::default();match kind {"bytes"=>l.source_bytes=n,"tokens"=>l.tokens=n,_=>l.nodes=n};
        let (r,_)=trace_load(l,None);check_error(&r.unwrap_err(),code,stage,Some(span));
    }
    for dimension in ["modules","depth","component","relative","probes","paths"] {
        let mut l=ProjectLimits::default();match dimension {"modules"=>l.modules=1,"depth"=>l.depth=0,"component"=>l.component_bytes=0,"relative"=>l.relative_bytes=3,"probes"=>l.probes=0,_=>l.path_bytes=0};
        let (r,_)=trace_load(l,None);let f=r.unwrap_err();check_error(&f,"E0400","source-project",Some((0,4,5)));
        assert!(f.allocator.trace.iter().all(|e| !matches!(e.kind,"logical module path"|"module display"|"module probe path")),"{dimension}: {:?}",f.allocator.trace);
        assert_eq!(f.usage.probes,0);
    }
    for dimension in ["entries","name_units"] {
        let mut l=ProjectLimits::default();if dimension=="entries" {l.directory_entries=1;}else{l.directory_name_units=9;}
        let (r,_)=trace_load(l,None);let f=r.unwrap_err();check_error(&f,"E0400","source-project",Some((0,4,5)));
        assert_eq!(f.diagnostics[0].message,"module directory scan budget exceeded");
        assert_eq!(f.allocator.trace.iter().filter(|e|e.kind=="source bytes").count(),1);
    }
}
#[test]
fn heldout_retained_paths_exact_and_one_below() {
    let e=entry("fixtures","minimal","app.ox").len();
    // Supplied absolute root display and canonical file are identical. Directory
    // is seven bytes shorter; child final filename is two bytes shorter.
    let root_payload=3*e-7;let full_payload=5*e-7;
    let mut l=ProjectLimits::default();l.path_bytes=full_payload;
    let (r,_)=trace_load(l,None);let p=r.unwrap();assert_eq!(p.usage().retained_path_bytes,full_payload);
    l.path_bytes-=1;let (r,_)=trace_load(l,None);let f=r.unwrap_err();
    check_error(&f,"E0400","source-project",Some((0,4,5)));assert_eq!(f.usage.retained_path_bytes,root_payload);
    assert!(f.allocator.trace.iter().all(|e|!matches!(e.kind,"logical module path"|"module display"|"module probe path")));
}
#[test]
fn heldout_every_controlled_reserve_failure_has_valid_origin() {
    let (r,a)=trace_load(ProjectLimits::default(),None);r.unwrap();assert!(a.attempts>0);
    let kinds=a.trace.iter().map(|x|x.kind).collect::<std::collections::BTreeSet<_>>();
    for k in ["source bytes","entry display","line starts","source files","module declarations","module items","file ASTs","module headers","logical module path","module display","module probe path"]{assert!(kinds.contains(k),"missing {k}");}
    for ordinal in 1..=a.attempts {
        let (r,_)=trace_load(ProjectLimits::default(),Some(ordinal));let f=r.expect_err("failure injection must be observed");
        assert!(f.diagnostics.iter().any(|d|d.code=="E0400"));
        let failures=f.allocator.trace.iter().filter(|e|!e.success).collect::<Vec<_>>();assert_eq!(failures.len(),1);
        let d=&f.diagnostics[0];assert!(d.message.contains("allocation failed"));
        if f.sources.files().is_empty(){assert_eq!(d.primary,None);assert_eq!(d.stage,"source-project");}
        else{assert!(d.primary.is_some());}
        for d in &f.diagnostics {let _=d.render_json(&f.sources);let _=d.render_human(&f.sources);}
        eprintln!("RESERVE_FAILURE ordinal={ordinal} kind={} stage={} primary={:?}",failures[0].kind,d.stage,d.primary.map(tuple));
    }
}
#[test]
fn heldout_child_count_overflow_beats_all_dimension_caps() {
    let at=Some(Span{file:SourceFileId(0),start:4,end:5});
    let mut l=ProjectLimits::default();l.modules=0;l.depth=0;l.component_bytes=0;l.relative_bytes=0;l.path_bytes=0;l.probes=0;
    let cases=[(SourceUsage{modules:usize::MAX,..SourceUsage::default()},0,0,1,0,0),
        (SourceUsage::default(),usize::MAX,0,1,0,0),(SourceUsage::default(),0,usize::MAX,1,0,0),
        (SourceUsage::default(),0,0,usize::MAX,0,0),(SourceUsage::default(),0,0,1,usize::MAX,0),
        (SourceUsage::default(),0,0,1,0,usize::MAX),(SourceUsage{retained_path_bytes:usize::MAX,..SourceUsage::default()},0,0,1,0,0),
        (SourceUsage{probes:usize::MAX,..SourceUsage::default()},0,0,1,0,0)];
    for (u,d,p,c,s,k) in cases {let e=ChildPlan::new(u,l,d,p,c,s,k,at).unwrap_err();assert_eq!((e.code,e.stage,e.message.as_str()),("E0400","source-project","project source count overflow"));}
}
#[test]
fn heldout_initial_dimension_precedence_over_root_path_cap() {
    for (case,winner) in [(0,"module count limit exceeded"),(1,"module depth limit exceeded"),(2,"module component limit exceeded"),(3,"module relative path limit exceeded")] {
        let mut l=ProjectLimits::default();l.path_bytes=0;l.probes=0;
        match case {0=>{l.modules=1;l.depth=0;l.component_bytes=0;l.relative_bytes=0},1=>{l.depth=0;l.component_bytes=0;l.relative_bytes=0},2=>{l.component_bytes=0;l.relative_bytes=0},_=>l.relative_bytes=3};
        let (r,_)=trace_load(l,None);let f=r.unwrap_err();
        check_error(&f,"E0400","source-project",Some((0,4,5)));
        assert_eq!(f.diagnostics[0].message,winner);
        assert_eq!(f.usage.probes,0);
    }
}
#[test]
fn heldout_line_index_dense_source_is_separately_preflighted() {
    let dir=PathBuf::from(REVIEW_ROOT).join("runtime-fixtures/newline-dense");std::fs::create_dir_all(&dir).unwrap();
    let mut bytes=Vec::new();
    for _ in 0..16 {bytes.extend(std::iter::repeat_n(b'\n',65532));bytes.extend_from_slice(b"/**/");}
    assert_eq!(bytes.len(),1048576);
    std::fs::write(dir.join("app.ox"),&bytes).unwrap();
    let mut a=Allocator::default();
    let p=ProjectSources::load(dir.join("app.ox").to_str().unwrap(),ProjectLimits::default(),parser::SourceMode::ModuleCandidate,&mut a).unwrap();
    assert_eq!(p.usage().line_starts,1048513);assert_eq!(p.usage().non_eof_tokens,32);assert_eq!(p.usage().syntax_nodes,0);
    let line=a.trace.iter().filter(|e|e.kind=="line starts").collect::<Vec<_>>();assert_eq!(line.len(),1);assert_eq!(line[0].length,1048513);assert_eq!(line[0].element_bytes,std::mem::size_of::<usize>());
}
#[test]
fn heldout_source_bytes_precede_artifact_and_encoding() {
    let mut l=ProjectLimits::default();l.source_bytes=11;
    let f=ProjectSources::load_modules(&entry("fixtures","oxbc_before_utf8","app.ox"),l).unwrap_err();
    check_error(&f,"E0400","source",Some((0,4,5)));assert_eq!(f.sources.files().len(),1);
    l.source_bytes=12;
    let f=ProjectSources::load_modules(&entry("fixtures","oxbc_before_utf8","app.ox"),l).unwrap_err();
    check_error(&f,"E0004","source",Some((0,4,5)));
}
#[test]
fn heldout_no_mod_sources_have_no_new_path_or_depth_policy() {
    let l=ProjectLimits{modules:1,depth:0,component_bytes:0,relative_bytes:0,path_bytes:0,probes:0,directory_entries:0,directory_name_units:0,..ProjectLimits::default()};
    for mode in [parser::SourceMode::OwnedCandidate,parser::SourceMode::ModuleCandidate] {
        let mut a=Allocator::default();
        let p=ProjectSources::load(&entry("fixtures","no_textual_mod_discovery","app.ox"),l,mode,&mut a).unwrap();
        assert_eq!(p.syntax_flavor(),SyntaxFlavor::OriginalSingleFile);assert_eq!(p.usage().probes,0);assert!(p.modules()[0].canonical_path.is_none());
    }
}
#[test]
fn heldout_measure_source_and_ast_layouts() {
    macro_rules! sz {($t:ty)=>{eprintln!("LAYOUT {} {}",stringify!($t),std::mem::size_of::<$t>());};}
    sz!(crate::frontend::source::SourceFile);sz!(lexer::Token);sz!(ast::Program);sz!(ModuleHeader);sz!(ProjectSources);sz!(SourceUsage);sz!(Span);sz!(Frame);
    sz!(FunctionAstKey);sz!(RecordAstKey);sz!(ExprKey);sz!(BlockKey);
    sz!(ast::Expr);sz!(ast::Stmt);sz!(ast::BodyBlock);sz!(ast::Function);sz!(ast::StructDecl);sz!(ast::StructField);sz!(ast::Param);sz!(ast::TypeSyntax);sz!(ast::FieldInit);sz!(ast::Argument);sz!(ast::ItemId);sz!(ast::ModuleDecl);
}
#[test]
fn heldout_checked_counter_overflow_stays_distinct_from_allocation_failure() {
    let mut a=Allocator::default();let mut bytes=vec![1u8];
    assert_eq!(a.vector(&mut bytes,usize::MAX,"length overflow"),Err(ReserveFailure::Overflow));
    let mut words=Vec::<u64>::new();assert_eq!(a.vector(&mut words,usize::MAX/8+1,"product overflow"),Err(ReserveFailure::Overflow));
    let mut text=String::from("x");assert_eq!(a.string(&mut text,usize::MAX,"string overflow"),Err(ReserveFailure::Overflow));
    let mut path=PathBuf::from("x");assert_eq!(a.path(&mut path,usize::MAX,"path overflow"),Err(ReserveFailure::Overflow));
    assert_eq!(a.attempts,0);assert!(a.trace.is_empty());
    let (ok,a)=trace_load(ProjectLimits::default(),None);ok.unwrap();
    for ordinal in 1..=a.attempts {
        let mut allocator=Allocator{attempts:usize::MAX-(ordinal-1),..Allocator::default()};
        let f=ProjectSources::load(&entry("fixtures","minimal","app.ox"),ProjectLimits::default(),parser::SourceMode::ModuleCandidate,&mut allocator).unwrap_err();
        let d=&f.diagnostics[0];assert_eq!(d.code,"E0400");
        let parser_reserve=matches!(a.trace[ordinal-1].kind,"module declarations"|"module items");
        assert_eq!(d.stage,if parser_reserve{"parse"}else{"source-project"});
        assert!(d.message.contains("overflow"),"ordinal={ordinal}: {d:?}");
        assert!(!d.message.contains("allocation failed"));
        assert_eq!(f.allocator.trace.len(),ordinal-1,"overflow must reject before a reserve attempt");
        if f.sources.files().is_empty(){assert_eq!(d.primary,None);}else{assert!(d.primary.is_some());}
        for d in &f.diagnostics {let _=d.render_json(&f.sources);}
        eprintln!("COUNTER_OVERFLOW ordinal={ordinal} kind={} stage={} primary={:?}",a.trace[ordinal-1].kind,d.stage,d.primary.map(tuple));
    }
}
