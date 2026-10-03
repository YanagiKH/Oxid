#!/usr/bin/env python3
"""Journal corruption sensitivity, with no model expectations inferred from it.

The real hand-smoke is a transport control. Its immutable original bytes stay
untouched; each altered journal is an in-memory test of runtime identity and
event-order invariants. Compiler execution is not performed here.
"""
import sys
if sys.flags.optimize:
    raise RuntimeError("Unit3 verification requires Python assertions (PYTHONOPTIMIZE=0)")

from pathlib import Path
import copy,gzip,hashlib,json
from actual_trace import normalize

HERE=Path(__file__).resolve().parent
INPUT=HERE.parents[1]/'typed-project-unit3-observer/smoke-v1/owned-debug-final/observations.json.gz'
def main():
    x=json.loads(gzip.decompress(INPUT.read_bytes()));raw=x['raw'];route=x['route'];original=x['reference']['reference-default']['trace']
    normal=normalize(raw,route,original);assert not normal['active_call_stack'] and not normal['live_loan_handles']
    results=[]
    def select(rows,kind,tag=None,op=None):
        return next(i for i,r in enumerate(rows) if r['kind']==kind and (tag is None or r['payload']['tag']==tag)
                    and (op is None or r['context']['operation'].get('kind',{}).get('tag')==op))
    def attempt(name,mutator):
        rows=copy.deepcopy(original);mutator(rows)
        try:normalize(raw,route,rows)
        except Exception as e:results.append(dict(id=name,status='REJECTED',reason=repr(e)))
        else:raise AssertionError(('corrupt actual journal escaped',name))
    def transfer(rows,side,field,value):
        i=select(rows,'existing_owned_event','Transfer');rows[i]['payload']['items'][side][field]=value
    for side,label in [(0,'source'),(1,'destination')]:
        for field,value in [('owner',999999),('activation',999999),('frame',999999),('generation',999999)]:
            attempt('transfer-'+label+'-'+field,lambda rows,s=side,f=field,v=value:transfer(rows,s,f,v))
    def physical(rows,field,value):
        i=select(rows,'payload_write',op='WriteField');rows[i]['payload'][0][field]=value
    for field,value in [('owner',999999),('activation',999999),('frame',999999),('generation',999999)]:
        attempt('field-target-'+field,lambda rows,f=field,v=value:physical(rows,f,v))
    def transition(rows,part):
        i=select(rows,'owner_transition');v=rows[i]['payload']
        if part=='function':v[1]=999999
        elif part=='state':v[3]=(v[3]+1)%4
        else:v[2][part]=999999
    for part in ('function','state','owner','activation','frame','generation'):
        attempt('transition-'+part,lambda rows,p=part:transition(rows,p))
    for tag in ('Charge','Return','Transfer','Acquire','Release','WriteField'):
        def drop(rows,t=tag):
            indices=[i for i,r in enumerate(rows) if r['kind']=='existing_owned_event' and r['payload']['tag']==t]
            rows.pop(indices[-1] if t=='Return' else indices[0])
        attempt('drop-callback-'+tag,drop)
    def late(rows):
        i=select(rows,'payload_write',op='WriteField');j=next(j for j in range(i+1,len(rows)) if rows[j]['kind']=='operation_commit')
        effect=rows.pop(i);rows.insert(j,effect)
    attempt('physical-effect-after-commit',late)
    def duplicate_payload(rows):
        i=select(rows,'payload_write',op='WriteField');rows.insert(i,copy.deepcopy(rows[i]))
    attempt('duplicate-physical-field-effect',duplicate_payload)
    for tag,side,label in [('Acquire',0,'acquire-loan-frame'),('Acquire',1,'acquire-owner-frame'),('Release',0,'release-loan-frame')]:
        def badframe(rows,t=tag,s=side):
            i=select(rows,'existing_owned_event',t);rows[i]['payload']['items'][s]['frame']=999999
        attempt(label,badframe)
    def scalar_frame(rows):
        i=select(rows,'owned_scalar_place_write');rows[i]['payload'][0]=999999
    attempt('owned-scalar-write-frame',scalar_frame)
    report=dict(schema='unit3-actual-journal-corruption-sensitivity-v2',scope='Transport/identity invariants only; no source expectation derived',
                input=str(INPUT),input_sha256=hashlib.sha256(INPUT.read_bytes()).hexdigest(),normalizer_sha256=hashlib.sha256((HERE/'actual_trace.py').read_bytes()).hexdigest(),
                positive='PASS',compiler_invocations=0,mutants=results,rejected=len(results))
    (HERE/'observed-transport-sensitivity-v2.json').write_text(json.dumps(report,indent=2)+'\n');print('actual journal mutants rejected',len(results))

if __name__=='__main__':main()
