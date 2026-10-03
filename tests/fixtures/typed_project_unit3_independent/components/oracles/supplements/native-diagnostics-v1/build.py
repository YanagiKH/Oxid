#!/usr/bin/env python3
"""Freeze native/driver streams from source and existing diagnostic contracts.

No candidate output, Debug snapshot, compiler process or LLVM process is read.
Only explicit literal path placeholders may be substituted by a comparator.
"""
from pathlib import Path
import gzip,hashlib,json

HERE=Path(__file__).resolve().parent;PACKAGE=HERE.parents[1]
ROOT=PACKAGE.parent;REPO=ROOT/'oxid-typed-index-ci-fix'
ORIGINAL='b89c8c5b00b13de9573513ba0f96d28fc1046f68c55d32d4ac1aad03d15c24ba'
FUEL={
 'pilot-scalar':[164,165],
 'pilot-mechanical-batch':[201,281,204,1255,1296,1297],
 'pilot-opaque-batch':[280,377,244,1528,1576,1577],
 'owned-root_child-import-root-loop_cleanup':[149,337,353,354],
}
DRIVER=['pilot-scalar','pilot-counter','negative-relay_reuse','negative-arity','negative-private_read',
 'negative-imported_main-scalar','negative-imported_main-owned','negative-native_recursion-scalar-false',
 'negative-native_recursion-owned-false','origin-unicode-crlf-scalar_overflow','origin-unicode-crlf-owned_overflow','origin-unicode-crlf-owned_loan']
def sha(data):return hashlib.sha256(data).hexdigest()
def enc(x):return json.dumps(x,separators=(',',':'),ensure_ascii=False)
def write(name,x):(HERE/name).write_text(json.dumps(x,indent=2,ensure_ascii=False)+'\n')
def verify():
    path=PACKAGE/'pre-execution-manifest.json';assert sha(path.read_bytes())==ORIGINAL
    for row in json.loads(path.read_text())['files']:
        data=(PACKAGE/row['path']).read_bytes();assert len(data)==row['bytes'] and sha(data)==row['sha256']

def location(c,span):
    if span is None:return None
    fid,a,b=span;rel=c['source_manifest'][fid]['path'];data=(PACKAGE/'sources'/c['id']/rel).read_bytes()
    before=data[:a].decode();end=data[:b].decode()
    return dict(file_id=fid,path='${FIXTURE_ROOT}/'+c['id']+'/'+rel,start=a,end=b,
                line=before.count('\n')+1,column=len(before.rsplit('\n',1)[-1])+1,
                end_line=end.count('\n')+1,end_column=len(end.rsplit('\n',1)[-1])+1)
def diagnostic(c,d):
    code=d['code'];stage=d['stage'];labels=[]
    if code=='E0310':message='owned value is not available on every path';labels=['value moved here','value declared here']
    elif code=='E0311':message='access conflicts with an active borrow';labels=['borrow acquired here','value declared here']
    elif code=='E0301':message='wrong argument count: expected 1, found 0';labels=['function declared here']
    elif code=='E0206':message='record field is private to its declaring module';labels=['private declaration here']
    elif code=='E0600':message='typed-preview run requires a declared zero-argument main returning bool, i32 or ()'
    elif code=='E0601':message='execution fuel exhausted'
    elif code=='E0604':message='checked i32 arithmetic overflow'
    elif code=='E0700':
        message=('native preview does not support recursive call graphs, including unused functions and unchosen branches'
                 if 'native_recursion' in c['id'] else 'native compile requires a declared zero-argument main')
    elif code=='E0701':message='native output already exists; choose a new path (existing files and symlinks are never overwritten)'
    else:raise AssertionError(('unfrozen diagnostic text',code))
    related=d.get('related',[]);assert len(labels)==len(related),(code,labels,related)
    primary=location(c,d.get('span'))
    secondaries=[dict(span=location(c,s),message=label) for s,label in zip(related,labels)]
    row=dict(schema_version=1,edition='typed-preview',kind='diagnostic',severity='error',code=code,stage=stage,message=message,
             primary=primary,secondary=secondaries,notes=[])
    human=f'error[{code}] ({stage}): {message}\n'
    if primary:human+='  --> '+primary['path']+f":{primary['line']}:{primary['column']}\n"
    for sec in secondaries:
        s=sec['span'];human+='  ::: '+s['path']+f":{s['line']}:{s['column']}: "+sec['message']+'\n'
    return row,human

def result(c):
    e=c['expected'];return dict(type='unit') if e['result_type']=='()' else dict(type=e['result_type'],value=e['result'])
def scalar_text(c):
    r=result(c);return ('()' if r['type']=='unit' else 'true' if r.get('value') is True else 'false' if r.get('value') is False else str(r['value']))+'\n'
def source_error(c):
    e=c['expected'];return e if e.get('status')=='reject' or e.get('check')=='reject' else None
def driver(c,operation,format,no_clobber=False):
    error=source_error(c)
    if error is None and operation=='run':error=c['expected'].get('run')
    if error is None and operation=='compile':error=c['expected'].get('native')
    if no_clobber:
        assert error is None and operation=='compile'
        error=dict(code='E0701',stage='native-toolchain',span=None,related=[])
    output='';stderr='';diag=None
    if error:diag,stderr=diagnostic(c,error)
    payload={'check':'functions','run':'result','compile':'output'}[operation]
    value=None if error else c['function_count'] if operation=='check' else result(c) if operation=='run' else '${OUTPUT}'
    summary=dict(schema_version=1,edition='typed-preview',kind=operation+'-summary',success=error is None,errors=int(error is not None),**{payload:value})
    if format=='json':output=(enc(diag)+'\n' if error else '')+enc(summary)+'\n';stderr=''
    elif not error:
        output=(f"typed-preview check ok ({c['function_count']} functions; check only)\n" if operation=='check'
                else scalar_text(c) if operation=='run' else 'typed-preview native compile ok: "${OUTPUT}"\n')
    return dict(operation=operation,format=format,exit_status=1 if error else 0,stdout=output,stderr=stderr,
                json_summary=summary,diagnostic=diag,external_tools='none' if error or operation!='compile' else 'real qualified LLVM only')

def main():
    verify();cases={c['id']:c for c in map(json.loads,gzip.open(PACKAGE/'expected.jsonl.gz','rt'))}
    requests=[json.loads(x) for x in (PACKAGE/'requests.jsonl').read_text().splitlines()];byrequest={r['id']:r for r in requests}
    defaults=[]
    for req in requests:
        if 'native' not in req:continue
        c=cases[req['id']];failure=c['expected'].get('native') or c['expected'].get('run')
        if failure:
            diag,human=diagnostic(c,failure);row=dict(id=c['id'],compile='reject' if req['native'].get('admission_only') else 'success',
                executable=not req['native'].get('admission_only',False),exit_status=1,stdout='',stderr=human,diagnostic=diag)
        else:row=dict(id=c['id'],compile='success',executable=True,exit_status=0,stdout=scalar_text(c),stderr='')
        defaults.append(row)
    assert len(defaults)==32 and sum(r['executable'] for r in defaults)==22
    fuels=[];fuel_requests=[]
    for id,budgets in FUEL.items():
        c=cases[id];req={**byrequest[id],'fuel_budgets':budgets};fuel_requests.append(req)
        # Native guarding requires actual source loops, not merely large fuel.
        assert any(b'while ' in (PACKAGE/'sources'/id/r['path']).read_bytes() for r in c['source_manifest'])
        for budget in budgets:
            denied=next((s for s in c['schedule']['items'] if s['end_fuel']>budget),None)
            row=dict(id=id,budget=budget,total_fuel=c['schedule']['total_fuel'],compile='success',executable=True)
            if denied:
                d=dict(code='E0601',stage='oir-run',span=c['origins'][denied['origin']],related=[])
                diag,human=diagnostic(c,d)
                row.update(exit_status=1,stdout='',stderr=human,diagnostic=diag,next_denied_operation=denied['operation'],
                           paid=denied['start_fuel'],denied_cost=denied['cost'])
            else:row.update(exit_status=0,stdout=scalar_text(c),stderr='',paid=c['schedule']['total_fuel'])
            fuels.append(row)
    assert len(fuels)==18
    drivers=[dict(id=id,expectations=[driver(cases[id],op,fmt) for op in ('check','run','compile') for fmt in ('text','json')]) for id in DRIVER]
    no_clobber=[dict(id=id,output_kind=kind,expectations=[driver(cases[id],'compile',fmt,True) for fmt in ('text','json')])
                for id in ('pilot-scalar','pilot-counter') for kind in ('regular','symlink')]
    write('expectations.json',dict(schema='unit3-native-driver-streams-v1',parent_manifest_sha256=ORIGINAL,
        path_substitution=dict(FIXTURE_ROOT='literal absolute root selected before invocation; same path source compiler receives',OUTPUT='literal output argument selected before invocation'),
        defaults=defaults,fuel=fuels,driver=drivers,no_clobber=no_clobber,compiler_invocations=0,candidate_output_inputs=0))
    (HERE/'fuel-requests.jsonl').write_text(''.join(enc(r)+'\n' for r in fuel_requests))
    # These request rows remain transport-only and do not carry expected output.
    write('fuel-request-roster.json',dict(cases=18,sources=4,budgets=FUEL,profiles=['debug','release']))
    inputs=[REPO/'src/frontend/diagnostic.rs',REPO/'src/frontend/driver.rs',REPO/'src/frontend/source.rs',REPO/'src/frontend/oir/mod.rs',
            REPO/'src/frontend/oir/owned/source/diagnostic.rs',REPO/'src/frontend/oir/owned/source/typeck.rs',
            REPO/'src/frontend/declaration_index.rs',REPO/'src/frontend/oir/native.rs',REPO/'src/frontend/oir/owned/native.rs',REPO/'src/frontend/native.rs']
    write('source-provenance.json',dict(inputs=[dict(path=str(p),sha256=sha(p.read_bytes())) for p in inputs],
        default_descriptors=32,default_elf=22,admission_only=10,fuel_cases=18,driver_fixtures=12,driver_outcomes_per_profile=72,
        source_only=True,parent_semantics_unchanged=True))
    verify();print(json.dumps(dict(defaults=32,elf=22,admission_only=10,fuel=18,driver=72,no_clobber=8)))

if __name__=='__main__':main()
