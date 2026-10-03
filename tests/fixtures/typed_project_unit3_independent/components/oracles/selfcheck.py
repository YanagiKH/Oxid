#!/usr/bin/env python3
"""Package/model self-check only. Never invokes a compiler or consumes results."""
from pathlib import Path
from collections import Counter
import copy,gzip,hashlib,json
import raw_span_oracle

HERE=Path(__file__).resolve().parent
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    cases=[json.loads(line) for line in gzip.open(HERE/'expected.jsonl.gz','rt')]
    byid={x['id']:x for x in cases};assert len(byid)==len(cases)
    requests=[json.loads(line) for line in (HERE/'requests.jsonl').read_text().splitlines()]
    assert [x['id'] for x in requests]==[x['id'] for x in cases]
    counts=Counter(x['family'] for x in cases)
    assert counts['scalar24']==24 and counts['owned48']==48 and counts['origin16']==16 and counts['collision2']==2
    allowed={'id','mode','entry','source_root','source_files','operations','profiles','fuel_budgets','native'}
    source_files=0;span_occurrences=0
    for req,c in zip(requests,cases):
        assert set(req)<=allowed
        assert req['source_files']==c['source_manifest']
        files={i:(HERE/req['source_root']/f['path']).read_bytes() for i,f in enumerate(req['source_files'])}
        for i,f in enumerate(req['source_files']):
            assert len(files[i])==f['bytes'] and hashlib.sha256(files[i]).hexdigest()==f['sha256'];source_files+=1
        for key,(fid,start,end) in c.get('origins',{}).items():
            data=files[fid];assert 0<=start<=end<=len(data),(c['id'],key)
            data[:start].decode();data[:end].decode();span_occurrences+=1
        for call in c.get('correspondence',{}).get('calls',[]):
            assert 0<=call['global_caller']<c['function_count'] and 0<=call['global_target']<c['function_count']
        if 'schedule' in c:
            schedule=c['schedule'];spent=0
            for item in schedule['items']:
                assert item['start_fuel']==spent and item['cost']>0
                spent+=item['cost'];assert item['end_fuel']==spent
            assert spent==schedule['total_fuel']
            if c['route']=='owned':
                for frame in schedule['frames'].values():
                    assert frame['X']==frame['S']+frame['A']+frame['P']+4*frame['O']+8*frame['R']+12*frame['L']+2*frame['C']
            for budget in c['budget_cases']:
                if budget['budget']<spent:assert budget['expected']['failure']['code']=='E0601'
                else:assert budget['expected']['result']==c['expected']['result']
    assert {tuple(sorted(c['axes'].items())) for c in cases if c['family']=='scalar24'}.__len__()==24
    assert {tuple(sorted(c['axes'].items())) for c in cases if c['family']=='owned48'}.__len__()==48
    for identifier,result,fuel in [('pilot-original-batch',816,1297),('pilot-mechanical-batch',816,1297),('pilot-opaque-batch',816,1577),
                                    ('pilot-scalar',10,165),('pilot-counter',5,126),('control-owned-entry-id1',5,126)]:
        c=byid[identifier];assert (c['expected']['result'],c['schedule']['total_fuel'])==(result,fuel)
    def charges(c):return [(x['operation'],x['cost'],x['origin']) for x in c['schedule']['items']]
    assert charges(byid['pilot-original-batch'])==charges(byid['pilot-mechanical-batch'])
    assert charges(byid['pilot-scalar'])==charges(byid['control-original-scalar-pilot'])
    entry=byid['control-owned-entry-id1'];assert entry['entry']==1 and entry['function_count']==6
    funcs=sorted((r for r in entry['declarations'] if r['kind']=='function'),key=lambda r:r['id'])
    assert [r['symbol']['name'] for r in funcs]==['helper','main','make','bump','finish','main']
    assert funcs[5]['parameters']==[['value','Counter']] and funcs[5]['result']=='Counter'
    mutations=json.loads((HERE/'mutation-expectations.json').read_text())
    assert mutations['count']==len(mutations['mutations'])==114
    for row in mutations['mutations']:assert row['positive_control'] in byid,row['id']
    # Synthetic inventory self-control: repeated equal spans are distinct slots;
    # deleting one or duplicating one visit must fail despite equal byte ranges.
    span=[0,0,1]
    raw={'functions':[dict(id=0,span=span,locals=[dict(span=span)],places=[],blocks=[dict(span=span,merge=None,
        statements=[dict(tag='Assign',span=span,value=dict(tag='I32',value=1))],terminator=dict(span=span,kind=dict(tag='Return',value=dict(span=span))))]) ]}
    inv=raw_span_oracle.inventory(raw,'scalar');assert (inv['D'],inv['Sspan'],inv['Vbind'])==(1,6,7)
    visits=[dict(kind='declaration',path=x['path']) for x in inv['declarations']]+[dict(kind='span',path=x['path']) for x in inv['spans']]
    cnt={key:{k:inv[k] for k in ('D','Sspan','Vbind')} for key in ('count','validate')}
    expected_decls=[dict(kind='function',id=0,span=span)]
    raw_span_oracle.check(raw,'scalar',{0:b'x'},expected_decls,visits,cnt)
    for mutant in (visits[:-1],visits+[visits[-1]],visits[:-1]+[visits[-2]]):
        try:raw_span_oracle.check(raw,'scalar',{0:b'x'},expected_decls,mutant,cnt)
        except AssertionError:pass
        else:raise AssertionError('missing/duplicate visit escaped independent comparator')
    receipt=dict(evidence_kind='PACKAGE_MODEL_SELF_CHECK_ONLY',cases=len(cases),families=dict(counts),source_files=source_files,
        valid_origin_entries=span_occurrences,mutations=mutations['count'],synthetic_visit_mutants_rejected=3,
        compiler_executions=0,native_executions=0,no_candidate_outputs_read=True,
        scalar_pilot_hand_ledger=dict(root_entry=8,root_sum_arg=1,call_sum=22,sum_runtime=115,choose_args=3,call_choose=10,choose_runtime=4,root_add=1,root_return=1),
        scalar_sum_hand_ledger=dict(initializers=4,initial_goto=1,four_conditions=16,four_inc_and_stores=48,four_skip_branches=16,
                                   continue_goto=1,three_accumulations=12,three_break_conditions=12,break_goto=1,two_backedges=2,return_value_and_return=2),
        scalar_route_fuel=dict(without_unused_nominal=byid['control-unused_nominal-false']['schedule']['total_fuel'],
                               with_unused_nominal=byid['control-unused_nominal-true']['schedule']['total_fuel']))
    assert sum(receipt['scalar_pilot_hand_ledger'].values())==165
    assert sum(receipt['scalar_sum_hand_ledger'].values())==115
    (HERE/'selfcheck.model-only.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps(receipt))

if __name__=='__main__':main()
