#!/usr/bin/env python3
"""Source-only correction of one inapplicable frozen mutation control.

Reads only the original pre-execution package. No candidate/observer inputs.
"""
import sys
if sys.flags.optimize:raise RuntimeError('Python assertions required')
from pathlib import Path
import hashlib,gzip,json
HERE=Path(__file__).resolve().parent;PACKAGE=HERE.parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    manifest=json.loads((PACKAGE/'pre-execution-manifest.json').read_text())
    assert sha(PACKAGE/'pre-execution-manifest.json')=='b89c8c5b00b13de9573513ba0f96d28fc1046f68c55d32d4ac1aad03d15c24ba'
    for f in manifest['files']:
        assert sha(PACKAGE/f['path'])==f['sha256']
    original=json.loads((PACKAGE/'mutation-expectations.json').read_text())
    cases={c['id']:c for c in map(json.loads,gzip.open(PACKAGE/'expected.jsonl.gz','rt'))}
    old='control-span-variants-scalar';new='scalar-root_child-absolute-entry0-i32'
    old_calls=cases[old]['correspondence']['calls'];new_calls=cases[new]['correspondence']['calls']
    assert len(old_calls)==1 and old_calls[0]['result_type']=='()'
    assert len(new_calls)==1 and new_calls[0]['result_type']=='i32'
    assert cases[new]['route']=='scalar' and new_calls[0]['global_target']==1
    row=next(r for r in original['mutations'] if r['id']=='raw-wrong_scalar_result')
    assert row['positive_control']==old and row['mutation']=={'kind':'call-result-slot-type','value':'opposite scalar type'}
    row['positive_control']=new
    requests=[{k:v for k,v in r.items() if k!='expected'} for r in original['mutations']]
    for name,value in [('expectations.json',original),('requests.json',dict(schema='unit3-finite-mutation-requests-v1',count=114,mutations=requests))]:
        (HERE/name).write_text(json.dumps(value,indent=2)+'\n')
    proof=dict(schema='unit3-mutation-applicability-source-proof-v1',id=row['id'],original_control=old,corrected_control=new,
        original_call=old_calls[0],corrected_call=new_calls[0],
        reason='The original sole call returns unit, which has no opposite i32/bool type. The corrected existing frozen source has one i32-returning helper call; replacing only its result-slot type with bool creates the originally intended raw signature mismatch.',
        unchanged=['114 mutation IDs','mutation action','raw-verifier-rejection authority','E0500 internal classification','all original source fixtures and model facts','all original freeze bytes'],
        candidate_inputs=[],candidate_invocations=0,
        source_inputs=[dict(path=str(PACKAGE/f),sha256=sha(PACKAGE/f)) for f in ['pre-execution-manifest.json','mutation-expectations.json','mutation-requests.json','expected.jsonl.gz',
            'sources/'+old+'/main.ox','sources/'+old+'/state.ox','sources/'+new+'/main.ox','sources/'+new+'/left.ox']])
    (HERE/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n')
if __name__=='__main__':main()
