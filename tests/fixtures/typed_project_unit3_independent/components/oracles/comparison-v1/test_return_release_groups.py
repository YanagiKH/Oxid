#!/usr/bin/env python3
"""Narrow return-group equivalence controls over preserved actual journals.

Source expected membership is read unchanged from the original pre-execution
freeze. No new expected outcome is obtained from a candidate observation.
"""
import sys
if sys.flags.optimize:raise RuntimeError('Python assertions required')
from pathlib import Path
import copy,gzip,hashlib,json
from expected_projection import load
from actual_trace import normalize
from compare_static import eq
from loan_groups import project

HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[1]
PAIR=ROOT/'typed-project-unit3-observer/full-qualification-v1/debug/positive-exclusive_alias/observations.json.gz'
REPEATED=ROOT/'typed-project-unit3-observer/smoke-v1/owned-debug-final/observations.json.gz'
def read(path):return json.loads(gzip.decompress(path.read_bytes()))
def release_indices(rows):return [i for i,r in enumerate(rows) if r['kind']=='existing_owned_event' and r['payload']['tag']=='Release']
def main():
    c=load()['positive-exclusive_alias'];a=read(PAIR);raw=a['raw'];rows=a['reference']['reference-default']['trace']
    wanted=project(c.expected_loans());normal=normalize(raw,'owned',rows)
    eq(wanted,project(normal['loans']),'frozen exact per-return membership')
    ids=release_indices(rows);assert len(ids)==2 and rows[ids[0]]['context']==rows[ids[1]]['context']
    altered=copy.deepcopy(rows);altered[ids[0]],altered[ids[1]]=altered[ids[1]],altered[ids[0]]
    reversed_actual=normalize(raw,'owned',altered)
    assert normal['loans']!=reversed_actual['loans'],'physical release order must remain visible'
    eq(wanted,project(reversed_actual['loans']),'only sibling release permutation is equivalent')
    results=[]
    def attempt(name,raw,rows):
        try:normalize(raw,'owned',rows)
        except Exception as e:results.append(dict(id=name,status='REJECTED',reason=repr(e)))
        else:raise AssertionError(('invalid return grouping escaped',name))
    missing=copy.deepcopy(rows);missing.pop(ids[0]);attempt('missing-release',raw,missing)
    dup=copy.deepcopy(rows);dup.insert(ids[0],copy.deepcopy(dup[ids[0]]));attempt('duplicate-release',raw,dup)
    moved=copy.deepcopy(rows);event=moved.pop(ids[0]);root_return=max(i for i,r in enumerate(moved) if r['kind']=='operation_start' and r['context']['position']=='terminator')
    event['context']=copy.deepcopy(moved[root_return]['context']);moved.insert(root_return+2,event);attempt('moved-to-root-return',raw,moved)
    wrong_owner=copy.deepcopy(rows)
    acq=[i for i,r in enumerate(wrong_owner) if r['kind']=='existing_owned_event' and r['payload']['tag']=='Acquire']
    wrong_owner[acq[0]]['payload']['items'][1]=copy.deepcopy(wrong_owner[acq[1]]['payload']['items'][1])
    attempt('wrong-borrow-owner',raw,wrong_owner)
    repeated=read(REPEATED);rs=repeated['reference']['reference-default']['trace'];ri=release_indices(rs);assert len(ri)==2
    # Same caller activation and static call site, different actual invocation
    # and LoanKey.instance: a target-function/span-only grouping would miss this.
    first,second=rs[ri[0]]['payload']['items'][0],rs[ri[1]]['payload']['items'][0]
    assert first['activation']==second['activation'] and first['loan']==second['loan'] and first['instance']!=second['instance']
    shifted=copy.deepcopy(rs);shifted[ri[0]]['payload'],shifted[ri[1]]['payload']=shifted[ri[1]]['payload'],shifted[ri[0]]['payload']
    attempt('shifted-between-repeated-call-returns',repeated['raw'],shifted)
    wrong_group=copy.deepcopy(normal['loans']);rel=next(e for e in wrong_group if e['kind']=='release');rel['return_group']['return_activation']+=1
    try:eq(wanted,project(wrong_group),'wrong returning activation')
    except Exception as e:results.append(dict(id='wrong-returning-activation',status='REJECTED',reason=repr(e)))
    else:raise AssertionError('wrong returning activation escaped')
    report=dict(schema='unit3-normal-return-release-equivalence-v1',source_contract_note='proposals/same-return-release-contract.md',
        input_hashes={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in (PAIR,REPEATED)},
        original_physical_order_retained=True,frozen_model_unchanged=True,positive_same_return_permutation='MATCH',negative_controls=results,
        compiler_invocations=0,new_source_cases=0,scope='unordered membership only inside an exact charged returning activation and entered resume call')
    (HERE/'return-release-group-sensitivity.json').write_text(json.dumps(report,indent=2)+'\n');print('same-return permutation matches; invalid return grouping controls rejected',len(results))
if __name__=='__main__':main()
