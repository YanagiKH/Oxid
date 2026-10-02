//! Reviewer-owned controls. Inject as a child of declaration_index only in the
//! independently copied frozen tree; expectations are source/algebra derived.
use super::*;
use crate::frontend::{lexer, parser, project::ProjectLimits, source::SourceMap};
use std::{fs, path::PathBuf, sync::atomic::{AtomicUsize, Ordering as AO}};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let p = PathBuf::from(std::env::var("OXID_RESOURCE_FIXTURES").expect("review fixture root"))
            .join(format!("case-{}", NEXT.fetch_add(1, AO::Relaxed)));
        fs::create_dir_all(&p).unwrap();
        for (name, text) in files { let file=p.join(name); fs::create_dir_all(file.parent().unwrap()).unwrap(); fs::write(file,text).unwrap(); }
        Self(p)
    }
    fn load(&self) -> ProjectSources { ProjectSources::load_project_candidate(self.0.join("root.ox").to_str().unwrap(), ProjectLimits::default()).unwrap() }
}
impl Drop for Fixture { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }
fn parsed(text: &str) -> (SourceMap, ast::Program) {
    let mut map=SourceMap::new(); let id=map.add("review.ox".into(),text.into()); let f=map.get(id);
    let p=parser::parse_with_mode(f,lexer::lex(f).unwrap(),parser::SourceMode::OwnedCandidate).unwrap(); (map,p)
}
fn check_error(e: &Diagnostic, message: &str, at: Span) { assert_eq!((e.code,e.stage,e.message.as_str(),e.primary),("E0400","resolve-project",message,Some(at))); }

#[test]
fn reviewer_nine_independent_space_cases() {
    // Tuple is F,R,K,M,U,A, exact retained payload, conservative variable scratch.
    let cases: Vec<(Vec<(&str,&str)>,[u64;8])> = vec![
        (vec![("root.ox","")],[0,0,0,1,0,0,60,4]),
        (vec![("root.ox","fn f()->(){}")],[1,0,0,1,0,1,112,8]),
        (vec![("root.ox","struct R{}")],[0,1,0,1,0,1,128,8]),
        (vec![("root.ox","struct R{a:i32,b:bool,c:i32}")],[0,1,3,1,0,1,176,8]),
        (vec![("root.ox","mod c;"),("c.ox","")],[0,0,0,2,0,1,164,12]),
        (vec![("root.ox","mod c; use crate::c::R;"),("c.ox","pub struct R{}")],[0,1,0,2,1,2,272,28]),
        (vec![("root.ox","mod c; use crate::c::f;"),("c.ox","pub fn f()->(){}")],[1,0,0,2,1,2,256,28]),
        (vec![("root.ox","mod c; use crate::c::R;"),("c.ox","pub struct R{} pub fn R()->(){}")],[1,1,0,2,1,3,324,32]),
        (vec![("root.ox","mod c; use crate::c::R; use crate::c::R as Q;"),("c.ox","pub struct R{} pub fn R()->(){}")],[1,1,0,2,2,3,364,44]),
    ];
    for (files,[f,r,k,m,u,a,payload,scratch]) in cases {
        let fix=Fixture::new(&files); let p=fix.load(); let w=WorkMeter::default(); let mut al=Allocator::default();
        let facts=collect_originals(SourceOwner::project(&p),IndexLimits::default(),&w,&mut al).unwrap();
        let plan=facts.plan(); let c=plan.counts;
        assert_eq!([c.functions,c.records,c.fields,c.modules,c.imports,c.originals],[f,r,k,m,u,a]);
        assert_eq!(plan.retained,payload+size_of::<DeclarationIndex<'_>>() as u64);
        assert_eq!(plan.scratch,scratch+FIXED_SCRATCH as u64);
        assert_eq!(al.attempts,14);
        assert_eq!(al.trace.iter().take(10).map(|e|e.length*e.element_bytes).sum::<usize>(),payload as usize);
        assert_eq!(al.trace.iter().skip(10).map(|e|e.length*e.element_bytes).sum::<usize>(),scratch as usize);
        assert!(al.trace.iter().all(|e|e.success));
        println!("space F={f} R={r} K={k} M={m} U={u} payload={payload} scratch={scratch}");
    }
}

#[test]
fn reviewer_independent_exact_admission_and_failure_order() {
    // 'fn f()->(){}' has ten non-EOF tokens, one function node and no statements.
    // Validator=2*(10+1)+2+8=32; inventory=module+item=2, total preflight=34.
    // Mandatory reservation=16*(A+M)+128+(h(1)+1)*(A+Lo)+2A+4Lo=168.
    let (map,p)=parsed("fn f()->(){}"); let file=map.get(SourceFileId(0));
    let owner=SourceOwner::original(file,&p,SourceView::Map(&map)).unwrap(); let at=owner.eof();
    let retained=112+size_of::<DeclarationIndex<'_>>() as u64; let scratch=8+FIXED_SCRATCH as u64;
    let cases=[
        (retained,scratch,202,None),
        (retained-1,scratch,202,Some("declaration index retained byte limit exceeded")),
        (retained,scratch-1,202,Some("declaration index scratch byte limit exceeded")),
        (retained,scratch,201,Some("declaration index mandatory build work limit exceeded")),
        (0,0,0,Some("declaration index retained byte limit exceeded")),
        (retained,0,0,Some("declaration index scratch byte limit exceeded")),
    ];
    for (retained,scratch,work,expected) in cases {
        let meter=WorkMeter::default(); meter.enable_observation(); let mut alloc=Allocator::default();
        let out=collect_originals(owner,IndexLimits{retained,scratch,work},&meter,&mut alloc);
        if let Some(message)=expected { check_error(&out.unwrap_err(),message,at); assert_eq!(alloc.attempts,0); }
        else { let facts=out.unwrap(); assert_eq!(facts.plan().build_work,168); assert_eq!(alloc.attempts,14); }
        assert_eq!(meter.events.borrow().iter().filter(|e|e.operation=="preflight visit").map(|e|e.units).sum::<u64>(),34);
    }
}

#[test]
fn reviewer_fourteen_actual_index_reserve_failures() {
    let fix=Fixture::new(&[("root.ox","mod c; use crate::c::R; fn f()->(){}"),("c.ox","pub struct R{pub n:i32} pub fn R()->(){}")]);
    let p=fix.load(); let owner=SourceOwner::project(&p); let at=owner.eof();
    let expected=[("index originals",4,36),("index original order",4,4),("index functions",2,12),("index records",1,28),("index fields",1,16),("index modules",2,60),("index children",1,4),("index imports",1,20),("index aliases",1,16),("index alias order",1,4),("index merge workspace",4,4),("index target order",1,4),("index target seen",1,8),("index child cursors",2,4)];
    for fail in 1..=14 {
        let meter=WorkMeter::default(); meter.enable_observation(); let mut alloc=Allocator{fail_at:Some(fail),..Allocator::default()};
        let e=collect_originals(owner,IndexLimits::default(),&meter,&mut alloc).unwrap_err();
        check_error(&e,"declaration index allocation failed",at); assert_eq!(alloc.attempts,fail); assert_eq!(alloc.trace.len(),fail);
        for (i,event) in alloc.trace.iter().enumerate() { assert_eq!((event.kind,event.length,event.element_bytes),expected[i]); assert_eq!(event.success,i+1<fail); }
        assert!(!meter.observations.borrow().iter().any(|e|matches!(e,Observation::Frozen{..})));
        // Immediate fresh construction after every failure checks no retained transaction/allocator state escapes.
        let meter=WorkMeter::default(); let mut alloc=Allocator::default();
        let index=collect_originals(owner,IndexLimits::default(),&meter,&mut alloc).unwrap().finish(&meter,&mut alloc).unwrap();
        assert_eq!(index.row_lengths(),[4,4,2,1,1,2,1,1,1,1]);
    }
}

#[test]
fn reviewer_five_actual_parser_reserve_positions() {
    let text="use crate::f;";
    let expected=[("absolute path segments",1,24,(4,9)),("absolute path segments",2,24,(11,12)),("absolute paths",1,40,(4,12)),("import declarations",1,56,(0,13)),("import items",1,16,(0,13))];
    for fail in 1..=5 {
        let mut map=SourceMap::new(); let id=map.add("reserve.ox".into(),text.into()); let file=map.get(id); let tokens=lexer::lex(file).unwrap();
        let mut alloc=Allocator{fail_at:Some(fail),..Allocator::default()};
        let errors=parser::parse_counted(file,tokens,parser::SourceMode::ProjectCandidate,parser::MAX_NODES,&mut alloc).unwrap_err();
        assert_eq!(errors.len(),1); let e=&errors[0];
        assert_eq!((e.code,e.stage,e.message.as_str(),e.primary),("E0400","parse","project syntax allocation failed",Some(file.span(expected[fail-1].3.0,expected[fail-1].3.1))));
        assert_eq!(alloc.attempts,fail);
        for (i,event) in alloc.trace.iter().enumerate() { assert_eq!((event.kind,event.length,event.element_bytes),(expected[i].0,expected[i].1,expected[i].2)); assert_eq!(event.success,i+1<fail); }
    }
}

#[test]
fn reviewer_checked_arithmetic_and_malformed_compact_identity() {
    let span=Span{file:SourceFileId(0),start:0,end:0};
    assert_eq!(resource::add(u64::MAX,0,span).unwrap(),u64::MAX);
    check_error(&resource::add(u64::MAX,1,span).unwrap_err(),"declaration index count overflow",span);
    for lane in 0..9 {
        let mut c=Counts{modules:1,..Counts::default()};
        match lane {0=>c.originals=u64::MAX,1=>c.functions=u64::MAX,2=>c.records=u64::MAX,3=>c.fields=u64::MAX,4=>c.modules=u64::MAX,5=>c.imports=u64::MAX,6=>{c.originals=1;c.original_bytes=u64::MAX;},7=>{c.imports=1;c.alias_bytes=u64::MAX;},_=>{c.imports=1;c.path_weight=u64::MAX;}}
        let e=IndexPlan::calculate(c,0,0,IndexLimits{retained:0,scratch:0,work:0},span).unwrap_err();
        check_error(&e,"declaration index count overflow",span);
    }
    assert_eq!(compact((u32::MAX-2) as usize,span).unwrap(),u32::MAX-2);
    for n in [(u32::MAX-1) as usize,u32::MAX as usize,usize::MAX] { assert_eq!(compact(n,span).unwrap_err().code,"E0500"); }
    assert_eq!(IndexPlan::calculate(Counts::default(),0,0,IndexLimits::default(),span).unwrap_err().code,"E0500");
}

#[test]
fn reviewer_exact_queries_use_hand_costs_and_current_origins() {
    let text="fn aa()->(){} fn zz()->(){}"; let (map,p)=parsed(text); let f=map.get(SourceFileId(0));
    let owner=SourceOwner::original(f,&p,SourceView::Map(&map)).unwrap(); let w=WorkMeter::default(); let mut a=Allocator::default();
    let index=collect_originals(owner,IndexLimits::default(),&w,&mut a).unwrap().finish(&w,&mut a).unwrap();
    for (which,units) in [(0,10),(1,7)] { let at=p.functions[which].name;
        for (limit,ok) in [(units,true),(units-1,false)] { let meter=WorkMeter::new(limit); let r=index.query(&meter).callee(ModuleId(0),ItemPathRef{file:at.file,path:ast::ItemPath::Unqualified(at)},true);
            assert_eq!(r.is_ok(),ok); if ok {assert_eq!(r.unwrap(),DefId(which));assert_eq!(meter.used(),units);} else {check_error(&r.unwrap_err(),"declaration index work limit exceeded",at);}
        }
    }
}

#[test]
fn reviewer_byte_comparators_and_stable_merge_bounds() {
    let at=Span{file:SourceFileId(0),start:0,end:0};
    for (a,b,cost) in [("","",1),("a","b",2),("same","same",5),("a","ab",2),("abcd","abce",5)] {
        let work=WorkMeter::new(cost); assert_eq!(compare_bytes(a,b,&work,at).unwrap(),a.cmp(b)); assert_eq!(work.used(),cost);
        assert!(compare_bytes(a,b,&WorkMeter::new(cost-1),at).is_err());
    }
    let alphabet=["","a","aa","ab","b"];
    let mut cases=0;
    for n in 0..=6usize {
        for mut code in 0..5usize.pow(n as u32) {
            let mut keys=Vec::new(); for _ in 0..n {keys.push(alphabet[code%5]);code/=5;}
            let mut order:Vec<u32>=(0..n as u32).collect(); let mut scratch=vec![0;n]; let w=WorkMeter::default();
            merge_sort(&mut order,&mut scratch,|a,b|compare_bytes(keys[a as usize],keys[b as usize],&w,at),&w,at).unwrap();
            let mut expected:Vec<u32>=(0..n as u32).collect(); expected.sort_by_key(|i|keys[*i as usize]); assert_eq!(order,expected);
            let h=if n<=1 {0} else {usize::BITS-(n-1).leading_zeros()} as usize;
            assert!(w.used()<= (h*(n+keys.iter().map(|s|s.len()).sum::<usize>())) as u64); cases+=1;
        }
    }
    assert_eq!(cases,19531); println!("reviewed merge sequences={cases}");
}

#[test]
fn reviewer_import_resource_failures_never_partially_commit() {
    let root="mod c; use crate::c::X as Y;";
    let fix=Fixture::new(&[("root.ox",root),("c.ox","pub struct X{} pub fn X()->(){}")]);let p=fix.load();let source=p.sources().get(SourceFileId(0));
    // Finish: original replay3 + syntax grouping2 + transaction38 + freeze3 =46.
    // Transaction: begin1, intermediate8, endpoint9, endpoint permissions4,
    // seen lanes2, builtin alias4, original type collision search5, value4, stage1.
    for budget in 0..=46u64 {
        let w=WorkMeter::default();w.enable_observation();let mut a=Allocator::default();
        let facts=collect_originals(SourceOwner::project(&p),IndexLimits::default(),&w,&mut a).unwrap();let before=w.used();w.restrict(before+budget);
        let result=facts.finish(&w,&mut a);let obs=w.observations.borrow();
        let imports:Vec<_>=obs.iter().filter_map(|o|if let Observation::Import{committed,aliases,seen,..}=o{Some((*committed,aliases,seen))}else{None}).collect();
        if budget<43 {assert!(!imports.iter().any(|e|e.0));for (_,aliases,seen) in &imports {assert!(aliases.iter().all(|c|c.ty.is_none()&&c.value.is_none()&&c.type_first.is_none()&&c.value_first.is_none()));assert!(seen.iter().all(|c|c.type_first.is_none()&&c.value_first.is_none()));}}
        else {assert_eq!(imports.len(),1);assert!(imports[0].0);assert_eq!(imports[0].1[0].ty,Some(RecordId(0)));assert_eq!(imports[0].1[0].value,Some(DefId(0)));}
        if budget==46 {assert!(result.is_ok());assert_eq!(w.used()-before,46);} else {let e=&result.unwrap_err()[0];assert_eq!((e.code,e.stage),("E0400","resolve-project"));assert!(!obs.iter().any(|o|matches!(o,Observation::Frozen{..})));}
        if let Some((start,end))=match budget {5=>Some((26,27)),6..=13=>Some((18,19)),14..=26=>Some((21,22)),27..=42=>Some((26,27)),_=>None} {
            let d=obs.iter().find_map(|o|if let Observation::Diagnostic{diagnostic,..}=o{Some(diagnostic)}else{None});if let Some(d)=d {assert_eq!(d.primary,Some(source.span(start,end)),"budget={budget}");}
        }
    }
}

#[test]
fn reviewer_type_owner_and_use_site_resource_guards() {
    let root="mod c; fn f(x:crate::c::R)->(){return;}";
    let fix=Fixture::new(&[("root.ox",root),("c.ox","pub struct R{n:i32} pub fn g()->bool{return true;}")]);let p=fix.load();let w=WorkMeter::default();let mut a=Allocator::default();
    let i=collect_originals(SourceOwner::project(&p),IndexLimits::default(),&w,&mut a).unwrap().finish(&w,&mut a).unwrap();
    let root_ast=p.try_file_ast(SourceFileId(0)).unwrap();let child_ast=p.try_file_ast(SourceFileId(1)).unwrap();let at=root_ast.functions[0].name;
    let ast::TypeSyntaxKind::Name(child_path)=child_ast.functions[0].result.kind else {panic!("bool type");};
    let foreign=ast::TypeSyntax{span:at,kind:ast::TypeSyntaxKind::Name(child_path)};
    assert_eq!(i.query(&WorkMeter::default()).value_type(ModuleId(0),foreign,TypeContext::Value).unwrap_err().code,"E0500");
    for limit in [0,1,2] {
        let meter=WorkMeter::new(limit);let out=i.query(&meter).construction_access(ModuleId(0),RecordId(0),at);
        if limit<2 {check_error(&out.unwrap_err(),"declaration index work limit exceeded",at);}else{assert!(matches!(out.unwrap(),Access::Denied(FieldId{record:RecordId(0),index:0})));}
        let meter=WorkMeter::new(limit);let out=i.query(&meter).field_access(ModuleId(0),FieldId{record:RecordId(0),index:0},at);
        if limit<2 {check_error(&out.unwrap_err(),"declaration index work limit exceeded",at);}else{assert!(matches!(out.unwrap(),Access::Denied(_)));}
    }
    assert_eq!(i.query(&WorkMeter::default()).construction_access(ModuleId(1),RecordId(0),at).unwrap_err().code,"E0500");
}

#[test]
fn reviewer_original_conflict_debit_precedes_semantics() {
    let(map,p)=parsed("fn same()->(){} fn same()->(){}");let source=map.get(SourceFileId(0));let owner=SourceOwner::original(source,&p,SourceView::Map(&map)).unwrap();
    for budget in [0,1,2] {let w=WorkMeter::default();let mut a=Allocator::default();let facts=collect_originals(owner,IndexLimits::default(),&w,&mut a).unwrap();w.restrict(w.used()+budget);let errors=facts.finish(&w,&mut a).unwrap_err();
        assert_eq!(errors.len(),1);let e=&errors[0];if budget<2 {check_error(e,"declaration index work limit exceeded",p.functions[budget as usize].name);}else{assert_eq!((e.code,e.primary),("E0201",Some(p.functions[1].name)));}
    }
}

#[test]
fn reviewer_actual_layout_and_source_node_envelope() {
    macro_rules! layouts { ($($t:ty),+) => { $(println!("layout {} bytes={} alignment={}",stringify!($t),size_of::<$t>(),std::mem::align_of::<$t>());)+ }; }
    layouts!(OriginalRow,FunctionRow,RecordRow,FieldRow,ModuleRow,ImportRow,AliasCell,SeenCell,DeclarationIndex<'_>,DeclarationFacts<'_>,IndexPlan,Counts,Scratch,SourceOwner<'_>,PreparedTypeName<'_>,QuerySession<'_,'_>,SourceFile,SourceMap,Span,lexer::Token,ast::Program,ast::Function,ast::Param,ast::BodyBlock,ast::Stmt,ast::StructDecl,ast::StructField,ast::Expr,ast::Argument,ast::FieldInit,ast::ItemId,ast::ModuleDecl,ast::AbsolutePath,ast::ImportDecl,ProjectSources);
    println!("fixed scratch={FIXED_SCRATCH}");assert!(FIXED_SCRATCH<=4096);
    assert_eq!([size_of::<OriginalRow>(),size_of::<FunctionRow>(),size_of::<RecordRow>(),size_of::<FieldRow>(),size_of::<ModuleRow>(),size_of::<ImportRow>(),size_of::<AliasCell>(),size_of::<SeenCell>()],[36,12,28,16,60,20,16,8]);
    let source=(0..100).map(|i|format!("fn f{i}()->(){{}}\n")).collect::<String>();let f=Fixture::new(&[("root.ox",&source)]);let p=f.load();
    assert_eq!(p.usage().syntax_nodes,100);assert_eq!(p.inventory().unwrap().ast_payload,28800);
    for text in ["fn f()->(){}","fn f(a:i32,b:bool)->i32{if b {return a;} else {return 1;}}","struct R{n:i32} fn f(x:&mut R)->(){x.n=1;return;}","fn f()->i32{return g(1,2+3);}","struct R{n:i32} fn f()->(){let mut r=R{n:1};g(&mut r);return;}"] {
        let mut map=SourceMap::new();let id=map.add("validate.ox".into(),text.into());let source=map.get(id);let tokens=lexer::lex(source).unwrap();let t=tokens.len()-1;
        let(p,n)=parser::parse_counted(source,tokens,parser::SourceMode::OwnedCandidate,parser::MAX_NODES,&mut Allocator::default()).unwrap();let mut callbacks=0u64;
        assert!(p.validate_spans_and_ids_counted(|span|{callbacks+=1;span.is_none_or(|span|source.try_text(span).is_some())}));
        assert!(callbacks<=11*n as u64+2*t as u64+4);if text=="fn f()->(){}"{assert_eq!((t,n,callbacks),(10,1,32));}
    }
}

#[test]
fn reviewer_exposure_and_nominal_format_work_are_independent_hand_costs() {
    let fix=Fixture::new(&[("root.ox","struct Hidden{} pub fn reveal()->Hidden{return Hidden{};}")]);let p=fix.load();let w=WorkMeter::default();let mut a=Allocator::default();let i=collect_originals(SourceOwner::project(&p),IndexLimits::default(),&w,&mut a).unwrap().finish(&w,&mut a).unwrap();
    let at=p.try_file_ast(SourceFileId(0)).unwrap().functions[0].result.span;
    for limit in [0,1] {let meter=WorkMeter::new(limit);let out=i.query(&meter).signature_exposure(DefId(0),RecordId(0),at);if limit==0{check_error(&out.unwrap_err(),"declaration index work limit exceeded",at);}else{assert!(matches!(out.unwrap(),Exposure::Denied{..}));assert_eq!(meter.used(),1);}}
    for (text,cost,expected) in [("struct R{}",2,"R"),("pub struct R{}",9,"crate::R")] {
        let f=Fixture::new(&[("root.ox",text)]);let p=f.load();let w=WorkMeter::default();let mut a=Allocator::default();let i=collect_originals(SourceOwner::project(&p),IndexLimits::default(),&w,&mut a).unwrap().finish(&w,&mut a).unwrap();let at=p.try_file_ast(SourceFileId(0)).unwrap().records[0].name;
        for limit in [cost-1,cost] {let meter=WorkMeter::new(limit);let out=i.query(&meter).prepare_type_name(RecordId(0),at);if limit<cost{check_error(&out.unwrap_err(),"declaration index work limit exceeded",at);}else{assert_eq!(out.unwrap().to_string(),expected);assert_eq!(meter.used(),cost);}}
    }
}

#[test]
fn reviewer_original_association_and_corrupt_arena_reject_cleanly() {
    let text="/*é*/ fn f(a:i32)->i32{let x:i32=a;return x;}";
    for damage in 0..7 {
        let(mut map,mut p)=parsed(text);let id=map.add("later.ox".into(),text.into());
        match damage {
            0=>p.functions[0].name.file=id,
            1=>p.functions[0].name=Span{file:SourceFileId(0),start:3,end:4},
            2=>p.functions[0].name=Span{file:SourceFileId(0),start:7,end:6},
            3=>p.functions[0].end=Span{file:SourceFileId(0),start:text.len()+1,end:text.len()+1},
            4=>p.functions[0].body=ast::BodyBlockId(usize::MAX),
            5=>p.functions[0].params[0].ty.span.file=id,
            _=>p.functions[0].blocks[0].body[0].kind=ast::StmtKind::Return(Some(ast::ExprId(usize::MAX))),
        }
        let file=map.get(SourceFileId(0));let owner=SourceOwner::original(file,&p,SourceView::Map(&map)).unwrap();let mut a=Allocator::default();
        assert_eq!(collect_originals(owner,IndexLimits::default(),&WorkMeter::default(),&mut a).unwrap_err().code,"E0500");assert_eq!(a.attempts,0);
    }
    let mut map=SourceMap::new();map.add("padding.ox".into(),"".into());let id=map.add("actual.ox".into(),"fn f()->(){}".into());let file=map.get(id);let p=parser::parse(file,lexer::lex(file).unwrap()).unwrap();
    let owner=SourceOwner::original(file,&p,SourceView::Map(&map)).unwrap();let w=WorkMeter::default();let mut a=Allocator::default();let i=collect_originals(owner,IndexLimits::default(),&w,&mut a).unwrap().finish(&w,&mut a).unwrap();assert_eq!(i.function(DefId(0)).unwrap(),(FunctionAstKey{file:id,index:0},ModuleId(0)));
    let stale={let(map,p)=parsed("");drop(map);p};let(map,_)=parsed("");assert_eq!(SourceOwner::original(map.get(SourceFileId(0)),&stale,SourceView::Map(&map)).unwrap_err().code,"E0500");
}

#[test]
fn reviewer_path_resource_exhaustion_immediately_precedes_semantic_failures() {
    let root="mod c; fn f()->(){crate::c::secret();crate::missing::f();crate::c::ghost();return;}";
    let fix=Fixture::new(&[("root.ox",root),("c.ox","fn secret()->(){}")]);let p=fix.load();let w=WorkMeter::default();let mut a=Allocator::default();let index=collect_originals(SourceOwner::project(&p),IndexLimits::default(),&w,&mut a).unwrap().finish(&w,&mut a).unwrap();let ast=p.try_file_ast(SourceFileId(0)).unwrap();
    // Sorted keys: root value f, root type c, child value secret.
    // Private endpoint: facade1 + c traversal6 + endpoint searches15 + access2=24.
    // Missing intermediate: facade1 + segment1 + root-type search5=7.
    // Missing endpoint: facade1 + c traversal6 + endpoint searches10=17.
    for (path_index,cost,code,word) in [(0,24,"E0206","secret"),(1,7,"E0205","missing"),(2,17,"E0200","ghost")] {
        let segments=ast.path_segments(ast::PathId(path_index)).unwrap();let at=*segments.iter().find(|s|p.try_text(**s)==Some(word)).unwrap();
        for limit in [cost-1,cost] {let meter=WorkMeter::new(limit);let error=index.query(&meter).callee(ModuleId(0),ItemPathRef{file:SourceFileId(0),path:ast::ItemPath::Absolute(ast::PathId(path_index))},true).unwrap_err();
            if limit<cost {check_error(&error,"declaration index work limit exceeded",at);}else{assert_eq!((error.code,error.stage,error.primary),(code,"resolve",Some(at)));assert_eq!(meter.used(),cost);}
        }
    }
}
