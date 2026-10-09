"""Exact, fail-closed current Unit2 accounting successor; no production changes."""
import hashlib
import json

AUTHORITY_SHA = '9cad87cddf974c53b94bb5883884fd895ca50eef6ea939b300dcbcd09930d612'
PREDECESSOR_SHA = '289d310395a7abb701c03d3ca065f87accba6ed02884fbcf21e2d13235c08dde'
DERIVED_SHA = '1b717bef7b1d6ffebcab1fe245f60ef95ba2b4819bccddad90de078fcee05831'
SOURCE_SHA = '35ee91911bb62c38c831aecb97c918bd14d9516013f5da3e62f445a1153e1cc4'
SEAMS = ((b"fn reviewer_independent_exact_admission_and_failure_order() {\n    // 'fn f()->(){}' has ten non-EOF toke"
  b'ns, one function node and no statements.\n    // Validator=2*(10+1)+2+8=32; inventory=module+item=2, tota'
  b'l preflight=34.\n    // Mandatory reservation=16*(A+M)+128+(h(1)+1)*(A+Lo)+2A+4Lo=168.\n    let (map,p'
  b')=parsed("fn f()->(){}"); let file=map.get(SourceFileId(0));\n    let owner=SourceOwner::original(file,&p'
  b",SourceView::Map(&map)).unwrap(); let at=owner.eof();\n    let retained=120+size_of::<DeclarationIndex<'_"
  b'>>() as u64; let scratch=8+FIXED_SCRATCH as u64;\n    let cases=[\n        (retained,scratch,202,None)'
  b',\n        (retained-1,scratch,202,Some("declaration index retained byte limit exceeded")),\n        ('
  b'retained,scratch-1,202,Some("declaration index scratch byte limit exceeded")),\n        (retained,scratch'
  b',201,Some("declaration index mandatory build work limit exceeded")),\n        (0,0,0,Some("declaration in'
  b'dex retained byte limit exceeded")),\n        (retained,0,0,Some("declaration index scratch byte limit ex'
  b'ceeded")),\n    ];\n    for (retained,scratch,work,expected) in cases {\n        let meter=WorkMeter::d'
  b'efault(); meter.enable_observation(); let mut alloc=Allocator::default();\n        let out=collect_origin'
  b'als(owner,IndexLimits{retained,scratch,work},&meter,&mut alloc);\n        if let Some(message)=expected {'
  b' check_error(&out.unwrap_err(),message,at); assert_eq!(alloc.attempts,0); }\n        else { let facts=out'
  b'.unwrap(); assert_eq!(facts.plan().build_work,168); assert_eq!(alloc.attempts,16); }\n        assert_eq!('
  b'meter.events.borrow().iter().filter(|e|e.operation=="preflight visit").map(|e|e.units).sum::<u64>(),34);'
  b'\n    }\n}\n',
  b"fn reviewer_independent_exact_admission_and_failure_order() {\n    // 'fn f()->(){}' has ten non-EOF toke"
  b'ns, one function node and no statements.\n    // Validator=2*(10+1)+2+8=32; inventory=module+item=2, tota'
  b'l preflight=34.\n    // Historical mandatory reservation=168 remains an immutable predecessor.\n    //'
  b' Current reservation adds 2M+3(O+I)+4(R+E+I)=2+3=5; total=173.\n    // Therefore exact admission is 34+17'
  b'3=207, and 206 must fail before allocation.\n    let (map,p)=parsed("fn f()->(){}"); let file=map.get(Sou'
  b'rceFileId(0));\n    let owner=SourceOwner::original(file,&p,SourceView::Map(&map)).unwrap(); let at=owner'
  b".eof();\n    let retained=120+size_of::<DeclarationIndex<'_>>() as u64; let scratch=8+FIXED_SCRATCH as u6"
  b'4;\n    let cases=[\n        (retained,scratch,202,Some("declaration index mandatory build work limit '
  b'exceeded")),\n        (retained,scratch,207,None),\n        (retained-1,scratch,207,Some("declaration '
  b'index retained byte limit exceeded")),\n        (retained,scratch-1,207,Some("declaration index scratch b'
  b'yte limit exceeded")),\n        (retained,scratch,206,Some("declaration index mandatory build work limit '
  b'exceeded")),\n        (0,0,0,Some("declaration index retained byte limit exceeded")),\n        (retain'
  b'ed,0,0,Some("declaration index scratch byte limit exceeded")),\n    ];\n    for (retained,scratch,work'
  b',expected) in cases {\n        let meter=WorkMeter::default(); meter.enable_observation(); let mut alloc='
  b'Allocator::default();\n        let out=collect_originals(owner,IndexLimits{retained,scratch,work},&meter,'
  b'&mut alloc);\n        if let Some(message)=expected { check_error(&out.unwrap_err(),message,at); assert_e'
  b'q!(alloc.attempts,0); assert!(alloc.trace.is_empty()); }\n        else { let facts=out.unwrap(); assert_e'
  b'q!(facts.plan().build_work,173); assert_eq!(alloc.attempts,16); }\n        assert_eq!(meter.events.borrow'
  b'().iter().filter(|e|e.operation=="preflight visit").map(|e|e.units).sum::<u64>(),34);\n    }\n}\n'),
 (b'fn reviewer_import_resource_failures_never_partially_commit() {\n    let root="mod c; use crate::c::X as '
  b'Y;";\n    let fix=Fixture::new(&[("root.ox",root),("c.ox","pub struct X{} pub fn X()->(){}")]);let p=fix.'
  b'load();let source=p.sources().get(SourceFileId(0));\n    // Finish: original replay3 + syntax grouping2 +'
  b' transaction38 + freeze3 =46.\n    // Transaction: begin1, intermediate8, endpoint9, endpoint permissions'
  b'4,\n    // seen lanes2, builtin alias4, original type collision search5, value4, stage1.\n    for budg'
  b'et in 0..=46u64 {\n        let w=WorkMeter::default();w.enable_observation();let mut a=Allocator::default'
  b'();\n        let facts=collect_originals(SourceOwner::project(&p),IndexLimits::default(),&w,&mut a).unwra'
  b'p();let before=w.used();w.restrict(before+budget);\n        let result=facts.finish(&w,&mut a);let obs=w.'
  b'observations.borrow();\n        let imports:Vec<_>=obs.iter().filter_map(|o|if let Observation::Import{co'
  b'mmitted,aliases,seen,..}=o{Some((*committed,aliases,seen))}else{None}).collect();\n        if budget<43 {'
  b'assert!(!imports.iter().any(|e|e.0));for (_,aliases,seen) in &imports {assert!(aliases.iter().all(|c|c.t'
  b'y.is_none()&&c.value.is_none()&&c.type_first.is_none()&&c.value_first.is_none()));assert!(seen.iter().al'
  b'l(|c|c.type_first.is_none()&&c.value_first.is_none()));}}\n        else {assert_eq!(imports.len(),1);asse'
  b'rt!(imports[0].0);assert_eq!(imports[0].1[0].ty,Some(RecordId(0)));assert_eq!(imports[0].1[0].value,Some'
  b'(DefId(0)));}\n        if budget==46 {assert!(result.is_ok());assert_eq!(w.used()-before,46);} else {let '
  b'e=&result.unwrap_err()[0];assert_eq!((e.code,e.stage),("E0400","resolve-project"));assert!(!obs.iter().a'
  b'ny(|o|matches!(o,Observation::Frozen{..})));}\n        if let Some((start,end))=match budget {5=>Some((26'
  b',27)),6..=13=>Some((18,19)),14..=26=>Some((21,22)),27..=42=>Some((26,27)),_=>None} {\n            let d=o'
  b'bs.iter().find_map(|o|if let Observation::Diagnostic{diagnostic,..}=o{Some(diagnostic)}else{None});if le'
  b't Some(d)=d {assert_eq!(d.primary,Some(source.span(start,end)),"budget={budget}");}\n        }\n    }\n'
  b'}\n',
  b'fn reviewer_import_resource_failures_never_partially_commit() {\n    let root="mod c; use crate::c::X as '
  b'Y;";\n    let fix=Fixture::new(&[("root.ox",root),("c.ox","pub struct X{} pub fn X()->(){}")]);let p=fix.'
  b'load();let source=p.sources().get(SourceFileId(0));\n    // Predecessor finish: replay3 + grouping2 + tra'
  b'nsaction38 + freeze3 =46.\n    // Current post-graph pass adds order M+2N=10 and reservation M+N+3B=1'
  b'2.\n    // Here M=2,N=4,B=2; X and Y mismatch u8 on their first byte.\n    // Exact finish=68, minus-o'
  b'ne=67. Import commit remains at43, before this pass.\n    // Transaction: begin1, intermediate8, endpoint'
  b'9, endpoint permissions4,\n    // seen lanes2, builtin alias4, original type collision search5, value4, s'
  b'tage1.\n    for budget in 0..=68u64 {\n        let w=WorkMeter::default();w.enable_observation();let m'
  b'ut a=Allocator::default();\n        let facts=collect_originals(SourceOwner::project(&p),IndexLimits::def'
  b'ault(),&w,&mut a).unwrap();let before=w.used(); let event_start=w.events.borrow().len();w.restrict(befor'
  b'e+budget);\n        let result=facts.finish(&w,&mut a);let obs=w.observations.borrow();\n        let i'
  b'mports:Vec<_>=obs.iter().filter_map(|o|if let Observation::Import{committed,aliases,seen,..}=o{Some((*co'
  b'mmitted,aliases,seen))}else{None}).collect();\n        if budget<43 {assert!(!imports.iter().any(|e|e.0))'
  b';for (_,aliases,seen) in &imports {assert!(aliases.iter().all(|c|c.ty.is_none()&&c.value.is_none()&&c.ty'
  b'pe_first.is_none()&&c.value_first.is_none()));assert!(seen.iter().all(|c|c.type_first.is_none()&&c.value'
  b'_first.is_none()));}}\n        else {assert_eq!(imports.len(),1);assert!(imports[0].0);assert_eq!(imports'
  b'[0].1[0].ty,Some(RecordId(0)));assert_eq!(imports[0].1[0].value,Some(DefId(0)));assert_eq!(imports[0].1['
  b'0].type_first,Some(0));assert_eq!(imports[0].1[0].value_first,Some(0));assert_eq!(imports[0].2[0].type_f'
  b'irst,Some(0));assert_eq!(imports[0].2[0].value_first,Some(0));}\n        // The complete successful post-'
  b'graph debit trace is hand-derived, including\n        // every failure prefix and its exact next diagnost'
  b'ic location.\n        let eof=SourceOwner::project(&p).eof();\n        let child=p.sources().get(Sourc'
  b'eFileId(1));\n        let module=source.span(4,5); let alias=source.span(26,27);\n        let record=c'
  b'hild.span(11,12); let function=child.span(22,23);\n        let suffix=[\n            ("u8 order module'
  b'",eof),("u8 order item",eof),("u8 order comparison",module),\n            ("u8 order item",eof),("u8 orde'
  b'r comparison",alias),\n            ("u8 order module",eof),("u8 order item",eof),("u8 order comparison",r'
  b'ecord),\n            ("u8 order item",eof),("u8 order comparison",function),\n            ("u8 reserva'
  b'tion module",eof),("u8 reservation item",eof),\n            ("u8 reservation item",eof),("u8 reservation '
  b'binding",alias),\n            ("comparison",alias),("compared byte",alias),\n            ("u8 reservat'
  b'ion module",eof),("u8 reservation item",eof),\n            ("u8 reservation binding",record),("comparison'
  b'",record),("compared byte",record),\n            ("u8 reservation item",eof),\n        ];\n        if b'
  b'udget>=46 {\n            let events=w.events.borrow();\n            let finish=&events[event_start..];'
  b'\n            let at=finish.iter().position(|e|e.operation=="u8 order module").unwrap_or(finish.len()'
  b');\n            assert_eq!(finish[..at].iter().map(|e|e.units).sum::<u64>(),46);\n            let paid'
  b'=(budget-46) as usize;\n            assert_eq!(finish[at..].iter().map(|e|(e.operation,e.origin,e.units))'
  b'.collect::<Vec<_>>(),suffix[..paid].iter().map(|(op,span)|(*op,*span,1)).collect::<Vec<_>>(),"budget={bu'
  b'dget}");\n            if budget<68 {check_error(&result.as_ref().unwrap_err()[0],"declaration index work '
  b'limit exceeded",suffix[paid].1);}\n        }\n        if budget==68 {assert!(result.is_ok());assert_eq'
  b'!(w.used()-before,68);} else {let e=&result.unwrap_err()[0];assert_eq!((e.code,e.stage),("E0400","resolv'
  b'e-project"));assert!(!obs.iter().any(|o|matches!(o,Observation::Frozen{..})));}\n        if let Some((sta'
  b'rt,end))=match budget {5=>Some((26,27)),6..=13=>Some((18,19)),14..=26=>Some((21,22)),27..=42=>Some((26,2'
  b'7)),_=>None} {\n            let d=obs.iter().find_map(|o|if let Observation::Diagnostic{diagnostic,..}=o{'
  b'Some(diagnostic)}else{None});if let Some(d)=d {assert_eq!(d.primary,Some(source.span(start,end)),"budget'
  b'={budget}");}\n        }\n    }\n}\n'))

def adapt(original, authority_bytes, source_bytes, api):
    api.require(api.digest(authority_bytes) == AUTHORITY_SHA, "stale Unit2 u8 resource authority")
    api.require(api.digest(source_bytes) == SOURCE_SHA, "wrong Unit2 u8 current source")
    authority = json.loads(authority_bytes)
    source = json.loads(source_bytes)
    api.require(authority["reviewed_source_head"] == source["reviewed_source_head"]
                and authority["source_only_tree"] == source["source_only_tree"]
                and all(row in source["files"] for row in authority["source_dependencies"]),
                "stale Unit2 u8 resource source dependency")
    name = authority["predecessor"]["path"]
    api.require(api.digest(original) == PREDECESSOR_SHA
                and api.entry(name, original) == authority["predecessor"], "wrong Unit2 u8 predecessor")
    result = original
    api.require(len(SEAMS) == 2, "wrong Unit2 u8 substitution count")
    for old, new in SEAMS:
        api.require(result.count(old) == 1 and result.count(new) == 0, "wrong Unit2 u8 resource seam")
        result = result.replace(old, new, 1)
    api.require(api.digest(result) == DERIVED_SHA
                and api.entry(name, result) == authority["derived"], "wrong Unit2 u8 derived resource")
    restored = result
    for old, new in reversed(SEAMS):
        api.require(restored.count(new) == 1, "ambiguous Unit2 u8 inverse seam")
        restored = restored.replace(new, old, 1)
    api.require(restored == original, "Unit2 u8 predecessor not restored")
    return result, authority
