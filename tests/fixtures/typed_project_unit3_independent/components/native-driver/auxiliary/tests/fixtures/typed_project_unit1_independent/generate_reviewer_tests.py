#!/usr/bin/env python3
if not __debug__:
    raise SystemExit('Qualification refuses optimized Python because it can remove verification guards')
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
from rust_strings import rust_string_literal
ROOT=Path(__file__).resolve().parent
out=['''//! Independent review tests generated from a pre-output contract fixture manifest.
use super::*;
use crate::frontend::{source::{SourceView, SourceMap, SourceFileId, Span}, diagnostic::Diagnostic};
const REVIEW_ROOT: &str = '''+rust_string_literal(str(ROOT))+''';
fn entry(group: &str, name: &str, supplied: &str) -> String {
    format!("{REVIEW_ROOT}/{group}/{name}/{supplied}")
}
fn load(group: &str, name: &str, supplied: &str) -> Result<ProjectSources, LoadFailure> {
    ProjectSources::load_modules(&entry(group,name,supplied), ProjectLimits::default())
}
fn tuple(s: Span) -> (usize,usize,usize) { (s.file.0,s.start,s.end) }
fn check_error(failure: &LoadFailure, code: &str, stage: &str, primary: Option<(usize,usize,usize)>) {
    assert!(!failure.diagnostics.is_empty());
    let d=&failure.diagnostics[0];
    assert_eq!((d.code,d.stage),(code,stage),"{d:?}");
    if let Some(p)=primary { assert_eq!(d.primary.map(tuple),Some(p),"{d:?}"); }
    for d in &failure.diagnostics {
        if let Some(p)=d.primary { assert!(failure.sources.is_valid_span(p)); }
        for (p,_) in &d.secondary { assert!(failure.sources.is_valid_span(*p)); }
        let _=d.render_json(&failure.sources);
        let _=d.render_human(&failure.sources);
    }
}
''']
q=rust_string_literal
usage={'bytes':'source_bytes','tokens':'non_eof_tokens','nodes':'syntax_nodes','eofs':None,'probes':'probes','entries':'directory_entries','name_units':'directory_name_units',
'aggregate_source_bytes':'source_bytes','modules':'modules','scan_entries':'directory_entries','scan_name_units':'directory_name_units'}
for manifest,group in [('fixture-manifest.json','fixtures'),('resource-fixture-manifest.json','resource-fixtures')]:
    for row in json.loads((ROOT/manifest).read_text())['fixtures']:
        if row.get('unavailable'):continue
        name=row['id'];e=row['expected'];supplied=row.get('entry','app.ox')
        t=[f'#[test]\nfn heldout_{name}() {{',f'    let result=load({q(group)},{q(name)},{q(supplied)});']
        if e['result']=='ok':
            t+=['    let p=result.unwrap();','    let u=p.usage();']
            if 'module_paths'in e:
                paths=[''if not x else x+'.ox' for x in e['module_paths']]
                t+=[f'    assert_eq!(p.modules().iter().map(|m|m.relative_path.as_str()).collect::<Vec<_>>(),vec![{",".join(q(x) for x in paths)}]);',
                    '    for (i,m) in p.modules().iter().enumerate() { assert_eq!(m.file,SourceFileId(i)); assert!(p.try_file_ast(m.file).is_some()); }',
                    f'    assert_eq!(p.sources().get(SourceFileId(0)).path(),entry({q(group)},{q(name)},{q(supplied)}));']
                parent=entrypath=None
                for i,x in enumerate(paths[1:],1):
                    # Expected display join derived only from literal supplied path.
                    source_entry=f'{ROOT}/{group}/{name}/{supplied}'
                    expected=source_entry.rsplit('/',1)[0]+'/'+x
                    t+=[f'    assert_eq!(p.sources().get(SourceFileId({i})).path(),{q(expected)});']
            for method,field in [('function_handles','function_handles'),('record_handles','record_handles')]:
                if field in e:
                    getter='try_function' if field.startswith('function') else 'try_record'
                    expects=','.join(f'({a},{b},{q(c)})'for a,b,c in e[field])
                    t+=[f'    assert_eq!(p.{method}().map(|k|(k.file.0,k.index,p.text(p.{getter}(k).unwrap().name))).collect::<Vec<_>>(),vec![{expects}]);']
            allusage={**e.get('usage',{}),**{k:v for k,v in e.items()if k in usage}}
            for key,value in allusage.items():
                if key=='eofs':t+=[f'    assert_eq!(p.programs.iter().map(|a|a.tokens.iter().filter(|t|t.kind==lexer::Kind::Eof).count()).sum::<usize>(),{value});']
                elif key in usage:t+=[f'    assert_eq!(u.{usage[key]},{value});']
            if 'visibility_span'in e:t+=['    assert_eq!(p.modules()[1].public.map(tuple),Some((0,0,3)));']
            if name=='native_path_identity':
                t+=['    let a=p.modules()[0].canonical_path.as_ref().unwrap(); let b=p.modules()[1].canonical_path.as_ref().unwrap(); assert_ne!(a,b); assert_eq!(a.to_string_lossy(),b.to_string_lossy());']
        else:
            prim=e.get('primary');prim=(tuple(prim[k]for k in ['file','start','end']) if isinstance(prim,dict) else tuple(prim))if prim else None
            pexpr='Some('+repr(prim)+')'if prim else'None'
            t+=['    let f=result.unwrap_err();',f'    check_error(&f,{q(e["code"])},{q(e["stage"])},{pexpr});']
            if 'primary_file'in e:t+=[f'    assert_eq!(f.diagnostics[0].primary.unwrap().file.0,{e["primary_file"]});']
            if 'retained_files'in e:t+=[f'    assert_eq!(f.sources.files().len(),{e["retained_files"]});']
            if e.get('message'):t+=[f'    assert_eq!(f.diagnostics[0].message,{q(e["message"])});']
            if 'secondary'in e:
                s=e['secondary'];t+=[f'    assert_eq!(f.diagnostics[0].secondary.first().map(|s|tuple(s.0)),Some(({s["file"]},{s["start"]},{s["end"]})));']
        t+=['}'];out.append('\n'.join(t))
(ROOT/'reviewer_cases.rs').write_text('\n\n'.join(out)+'\n')
print('Generated independently expected cases')
