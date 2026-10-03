// Independent expected results use raw fixture sequences and hand-counted schedules.
fn independent_native(raw:RawOwnedProgram,sources:&SourceMap,fuel:usize)->String {
    verified::probe_array_native(raw,sources,budget::Limits::DEFAULT,Some(hir::DefId(0)),sources,NativeControl{fuel,..NativeControl::default()}).unwrap().result.unwrap()
}
fn independent_diagnostic(kind:&str,span:Span,sources:&SourceMap)->String {
    let (code,stage,message)=match kind {"fuel"=>("E0601","oir-run","execution fuel exhausted"),"bounds"=>("E0606","oir-owned-run","array index out of bounds"),"overflow"=>("E0604","oir-run","checked i32 arithmetic overflow"),_=>panic!("independent failure kind")};
    let mut out=format!("error[{code}] ({stage}): {message}\n");
    if let Some(source)=sources.files().get(span.file.0) {
        let t=source.text();
        if span.start<=span.end && span.end<=t.len() && t.is_char_boundary(span.start) && t.is_char_boundary(span.end) {
            let before=&t[..span.start]; let line=1+before.bytes().filter(|b|*b==b'\n').count(); let column=1+before.rsplit('\n').next().unwrap().chars().count();
            let path:String=source.path().chars().flat_map(|c|if c.is_control(){c.escape_default().collect::<Vec<_>>()}else{vec![c]}).collect();
            out.push_str(&format!("  --> {path}:{line}:{column}\n"));
        }
    }
    out
}
fn independent_run(scratch:&Scratch,binary:&std::path::Path,name:&str,args:&[String])->std::process::Output {
    let root=std::path::PathBuf::from(std::env::var_os("OXID_UNIT2D_EXEC_EVIDENCE").expect("explicit execution receipt directory"));
    std::fs::create_dir_all(&root).unwrap();
    assert!(!std::fs::read_dir(&scratch.0).unwrap().any(|p|matches!(p.unwrap().path().extension().and_then(|x|x.to_str()),Some("ll"|"bc"|"c"))));
    let result=scratch.run(binary,args);
    std::fs::write(root.join(format!("{name}.stdout")),&result.stdout).unwrap();
    std::fs::write(root.join(format!("{name}.stderr")),&result.stderr).unwrap();
    let quote=crate::frontend::diagnostic::json_string;
    let argv=args.iter().map(|s|quote(s)).collect::<Vec<_>>().join(",");
    let status=result.status.code().map_or_else(||"null".to_string(),|n|n.to_string());
    std::fs::write(root.join(format!("{name}.json")),format!("{{\"binary\":{},\"args\":[{argv}],\"cwd\":{},\"env_clear\":true,\"PATH\":{},\"status\":{status}}}\n",quote(binary.to_str().unwrap()),quote(scratch.0.to_str().unwrap()),quote(scratch.0.join("no-tools").to_str().unwrap()))).unwrap();
    result
}
fn independent_assert_output(actual:std::process::Output,expected:Option<Scalar>,error:Option<String>) {
    match expected {Some(value)=>assert_result(actual,&scalar_output(value),b"",0),None=>assert_result(actual,b"",error.unwrap().as_bytes(),1)}
}
fn kind_name(ty:hir::Ty)->&'static str {match ty{hir::Ty::I32=>"i32",hir::Ty::Bool=>"bool",hir::Ty::Unit=>"unit"}}
fn independent_structure_receipt(raw:&RawOwnedProgram,n:usize,name:&str,guarded:bool) {
    use std::fmt::Write;
    let root=std::path::PathBuf::from(std::env::var_os("OXID_UNIT2D_EXEC_EVIDENCE").unwrap());std::fs::create_dir_all(&root).unwrap();
    let mut text=String::from("function\tprefix\tlength\tguarded\n");
    for f in &raw.functions {for (b,block) in f.blocks.iter().enumerate(){for (i,statement) in block.statements.iter().enumerate(){if matches!(statement.kind,OwnedInstruction::ReadIndex{..}|OwnedInstruction::WriteIndex{..}){writeln!(text,"__oxid_owned_fn_{}\tf{}_b{b}_i{i}\t{n}\t{guarded}",f.id.0,f.id.0).unwrap();}}}}
    std::fs::write(root.join(format!("{name}.structure.tsv")),text).unwrap();
}
#[test]
#[ignore="independent Unit2D pinned LLVM source-free ELF gate"]
fn independent_unit2d_core_and_width_llvm() {
    let scratch=Scratch::new();let mut core=0;let mut wide=0;
    for ty in [hir::Ty::Bool,hir::Ty::I32,hir::Ty::Unit] {for n in [0usize,1,2,3,4,1024] {
        let mut indexes=if n==1024 {vec![i32::MIN,-1,0,1023,1024,i32::MAX]}else{let mut xs=vec![i32::MIN,-1];xs.extend(0..=n as i32);xs.push(i32::MAX);xs};indexes.sort();indexes.dedup();
        for mode in 0..=2 {for &index in if mode==0 {&indexes[0..1]}else{&indexes[..]} {
            let (sources,raw,expected,access,_)=native_core_fixture(ty,n,index,mode);
            let name=format!("core-{}-n{n}-m{mode}-i{index}",kind_name(ty));
            independent_structure_receipt(&raw,n,&name,false);
            let module=independent_native(raw,&sources,1_000_000);let binary=scratch.compile(&module,&name);
            independent_assert_output(independent_run(&scratch,&binary,&name,&[]),expected,Some(independent_diagnostic("bounds",access,&sources)));
            if n==1024{wide+=1}else{core+=1}
        }}
    }}
    assert_eq!((core,wide),(195,39));
    eprintln!("independent native core inputs={core}, maximum-width inputs={wide}; all real LLVM/source-free ELF");
}
#[test]
#[ignore="independent Unit2D pinned LLVM source-free ELF gate"]
fn independent_unit2d_phi_and_untaken_llvm() {
    let scratch=Scratch::new();let mut diamonds=0;let mut untaken=0;
    for pattern in ["arithmetic_bounds","bounds_arithmetic","bounds_bounds","bounds_suffix","no_split"] {for take in [false,true] {for guarded in [false,true] {
        let (sources,raw)=native_phi_fixture(pattern,take,guarded);let name=format!("phi-{pattern}-t{take}-g{guarded}");independent_structure_receipt(&raw,1,&name,guarded);
        let module=independent_native(raw,&sources,1_000_000);let binary=scratch.compile(&module,&name);independent_assert_output(independent_run(&scratch,&binary,&name,&[]),Some(Scalar::Bool(take)),None);diamonds+=1;
    }}}
    for take in [false,true] {for empty in [false,true] {for guarded in [false,true] {
        let(sources,raw)=native_untaken_bounds_fixture(take,empty,guarded);let name=format!("untaken-t{take}-empty{empty}-g{guarded}");independent_structure_receipt(&raw,usize::from(!empty),&name,guarded);
        let module=independent_native(raw,&sources,1_000_000);let binary=scratch.compile(&module,&name);independent_assert_output(independent_run(&scratch,&binary,&name,&[]),Some(Scalar::Bool(take)),None);untaken+=1;
    }}}
    assert_eq!((diamonds,untaken),(20,8));eprintln!("independent native phi diamonds={diamonds}, untaken invalid bounds={untaken}");
}
#[test]
#[ignore="independent Unit2D pinned LLVM source-free ELF gate"]
fn independent_unit2d_transfer_chains_llvm() {
    let scratch=Scratch::new();let mut count=0;
    for ty in [hir::Ty::Bool,hir::Ty::I32,hir::Ty::Unit] {for n in [0usize,1,4,1024] {
        let(sources,raw,_)=chain_fixture(ty,n);let name=format!("chain-{}-n{n}",kind_name(ty));let module=independent_native(raw,&sources,1_000_000);let binary=scratch.compile(&module,&name);independent_assert_output(independent_run(&scratch,&binary,&name,&[]),Some(Scalar::I32(n as i32)),None);count+=1;
    }for n in [0usize,4] {
        let(sources,raw,_)=self_replacement_chain(ty,n);let name=format!("selfchain-{}-n{n}",kind_name(ty));let module=independent_native(raw,&sources,1_000_000);let binary=scratch.compile(&module,&name);independent_assert_output(independent_run(&scratch,&binary,&name,&[]),Some(Scalar::I32(n as i32)),None);count+=1;
    }}
    assert_eq!(count,18);eprintln!("independent native whole-transfer chain programs={count}; physical storage checks separate");
}
#[test]
#[ignore="independent Unit2D pinned LLVM source-free ELF gate"]
fn independent_unit2d_core_fuel_llvm() {
    let scratch=Scratch::new();let mut profiles=0;
    let traces=[(hir::Ty::I32,0,0,1),(hir::Ty::I32,1,0,1),(hir::Ty::I32,1,0,2),(hir::Ty::I32,1,-1,2),(hir::Ty::Bool,4,3,2),(hir::Ty::Unit,4,3,1),(hir::Ty::Bool,0,0,0),(hir::Ty::I32,0,0,0),(hir::Ty::Unit,0,0,0)];
    for (case,(ty,n,index,mode)) in traces.into_iter().enumerate() {
        let(sources,mut raw,expected,access,mut schedule)=native_core_fixture(ty,n,index,mode);append_cycle(&mut raw);
        if expected.is_none(){let last=schedule.iter().position(|(at,_)|*at==access).unwrap()+1;schedule.truncate(last);}
        let total:usize=schedule.iter().map(|x|x.1).sum();let name=format!("fuel-core-{case}");independent_structure_receipt(&raw,n,&name,true);
        let module=independent_native(raw,&sources,total);std::fs::write(std::path::PathBuf::from(std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE").unwrap()).join(format!("{name}-production.ll")),&module).unwrap();
        let harness=argv_fuel_harness(&module,total);let binary=scratch.compile(&harness,&name);
        for fuel in 0..=total {
            let mut remaining=fuel;let mut unpaid=None;for &(at,cost) in &schedule {if remaining<cost{unpaid=Some(at);break;}remaining-=cost;}
            let output=independent_run(&scratch,&binary,&format!("{name}-budget{fuel}"),&[fuel.to_string()]);
            if let Some(at)=unpaid {independent_assert_output(output,None,Some(independent_diagnostic("fuel",at,&sources)));}else{independent_assert_output(output,expected,Some(independent_diagnostic("bounds",access,&sources)));}
            profiles+=1;
        }
    }
    eprintln!("independent native core all-budget profiles={profiles}, traces=9");
}
