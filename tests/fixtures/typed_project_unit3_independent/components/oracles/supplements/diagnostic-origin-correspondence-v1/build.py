#!/usr/bin/env python3
"""Recover a missing exact metadata obligation from frozen source inputs only."""
import sys
if sys.flags.optimize:raise RuntimeError('Python assertions required')
from pathlib import Path
import gzip,hashlib,json
HERE=Path(__file__).resolve().parent;PACKAGE=HERE.parents[1];ROOT=PACKAGE.parent
sys.path.insert(0,str(PACKAGE))
import generate as g
m=g.m
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    original=PACKAGE/'pre-execution-manifest.json'
    assert sha(original)=='b89c8c5b00b13de9573513ba0f96d28fc1046f68c55d32d4ac1aad03d15c24ba'
    for f in json.loads(original.read_text())['files']:assert sha(PACKAGE/f['path'])==f['sha256']
    predecessor=ROOT/'typed-project-unit2-ci-fix/final-candidate/manifest.json'
    assert sha(predecessor)=='bcbbebf6f22d5055069cdd61dee663e15470f2d4215d739725ca935fb2f8b24f'
    lower=ROOT/'oxid-typed-index-ci-fix/src/frontend/oir/owned/source/lower.rs'
    expected_lower=next(f for f in json.loads(predecessor.read_text())['source_files'] if f['path']=='src/frontend/oir/owned/source/lower.rs')
    assert sha(lower)==expected_lower['sha256']
    # Reproduce the exact independently authored AST construction in frozen
    # negative_cases.py. No parser, compiler resolver, or actual raw is read.
    b=m.Builder();record=m.Record('Cell',(('value','i32'),));fs=[];homes={}
    def fn(name,params,result,body):fs.append(m.Function(name,params,result,body));homes[name]='state'
    fn('read',(('p','&Cell'),),'i32',b.block(b.ret(b.f('p','value'))))
    fn('take',(('p','Cell'),),'i32',b.block(b.ret(b.f('p','value'))))
    fn('relay',(('p','Cell'),),'Cell',b.block(b.ret(b.v('p'))))
    fn('bump',(('p','&mut Cell'),),'i32',b.block(b.write('p','value',b.op(b.f('p','value'),'+',b.i(1))),b.ret(b.f('p','value'))))
    pre=[b.let('x',b.lit('Cell',('value',b.i(4))),True),b.let('y',b.lit('Cell',('value',b.i(7))),True)]
    fn('pair',(('p','&mut Cell'),('n','i32')),'i32',b.block(b.ret(b.v('n'))))
    read=b.f('x','value');call=b.call('pair',b.borrow('x',True),read);statement=b.ret(call)
    main=m.Function('main',(),'i32',b.block(*pre,statement));fs.insert(0,main);homes['main']='root'
    program=m.Program((record,),tuple(fs));project=g.projectify(program,homes,{'Cell':'state'},'absolute',explicit_files=['root','state','other'])
    case=next(c for c in map(json.loads,gzip.open(PACKAGE/'expected.jsonl.gz','rt')) if c['id']=='negative-exclusive_then_read')
    assert project['origins']==case['origins'],'all original AST origin labels must reproduce'
    assert project['bijection']==case['source_to_model_bijection'],'complete existing module-to-model correspondence'
    for name,text in project['files'].items():assert text.encode()==(PACKAGE/'sources'/case['id']/name).read_bytes()
    assert statement.tag=='return' and statement.data['value'] is call and call.data['args'][1] is read and read.tag=='field'
    fid=next(d['id'] for d in case['declarations'] if d['kind']=='function' and d['model_label']=='main')
    expected=dict(schema='unit3-exact-diagnostic-origin-obligation-v1',case=case['id'],route='owned',
        obligations=[dict(function=fid,operation='ReadField',operation_span=project['origins'][read.key],
            primary=project['origins'][read.key],cause=project['origins'][statement.key],
            source_expression_key=read.key,enclosing_statement_key=statement.key)],
        rule='ReadField primary is its source expression; cause is the enclosing source statement passed unchanged through nested expression lowering',
        scope='Exact stored metadata correspondence for this finite mutation control; independent of whether a selected diagnostic renders cause')
    (HERE/'expectations.json').write_text(json.dumps(expected,indent=2)+'\n')
    proof=dict(schema='unit3-source-only-diagnostic-origin-proof-v1',all_source_bytes_reproduced=True,all_frozen_origins_reproduced=len(project['origins']),full_module_to_model_bijection_reproduced=True,
        statement_tag=statement.tag,expression_tag=read.tag,relationship='return statement -> call expression -> second argument field read',
        producer_contract_locations=['owned/source/lower.rs:1177-1186 passes statement span to expression','owned/source/lower.rs:581-844 carries cause through nested expression/call frames','owned/source/lower.rs:926-941 ReadField uses expression span for primary and inherited cause'],
        design_requirement='Unit3 design section 9 Metadata authority: independent exact-origin comparator rejects incorrect source placement',
        inputs=[dict(path=str(p),sha256=sha(p)) for p in [original,PACKAGE/'negative_cases.py',PACKAGE/'generate.py',PACKAGE/'vendor/owned_source_model.py',PACKAGE/'expected.jsonl.gz',predecessor,lower,ROOT/'typed-project-unit3-design/proposed-design.md']],
        candidate_inputs=[],compiler_invocations=0,original_expected_edits=0)
    (HERE/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n')
if __name__=='__main__':main()
