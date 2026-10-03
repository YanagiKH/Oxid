#!/usr/bin/env python3
"""Independent frozen source/model versus actual Unit3 observer comparator."""
import sys
if sys.flags.optimize:
    raise RuntimeError("Unit3 verification requires Python assertions (PYTHONOPTIMIZE=0)")

from pathlib import Path
import argparse,gzip,hashlib,json,traceback
from expected_projection import load,PACKAGE
from compare_static import association,correspondence,eq
from actual_trace import normalize
from loan_groups import project as loan_groups

def sha(data):return hashlib.sha256(data).hexdigest()
def strict_json(data):
    def pairs(rows):
        out={}
        for key,value in rows:
            if key in out:raise ValueError('duplicate JSON key '+key)
            out[key]=value
        return out
    return json.loads(data,object_pairs_hook=pairs,parse_constant=lambda v:(_ for _ in ()).throw(ValueError(v)))

def diagnostic(d,loaded):
    def span(s):
        if s is None:return None
        fid=s['file_id'];source=loaded[fid];a,b=s['start'],s['end'];data=source['text'].encode()
        assert 0<=a<=b<=len(data);before=data[:a].decode();end=data[:b].decode()
        eq(source['path'],s['path'],'diagnostic source path')
        eq((before.count('\n')+1,len(before.rsplit('\n',1)[-1])+1),(s['line'],s['column']),'primary byte/scalar coordinates')
        eq((end.count('\n')+1,len(end.rsplit('\n',1)[-1])+1),(s['end_line'],s['end_column']),'end byte/scalar coordinates')
        return [fid,a,b]
    return dict(code=d['code'],stage=d['stage'],span=span(d['primary']),related=[span(s['span']) for s in d['secondary']])

def expected_diagnostic(d):return {k:d.get(k,[]) if k=='related' else d.get(k) for k in ('code','stage','span','related')}
def compare_case(case,actual,source_root):
    expected=case.case['expected'];loaded=actual['loaded']['sources']['files'];report={}
    eq(list(range(len(case.case['source_manifest']))),[s['id'] for s in loaded],'complete dense loaded source identity/order')
    for source,wanted in zip(loaded,case.case['source_manifest']):
        data=source['text'].encode();eq(wanted['bytes'],len(data),'loaded source byte count before any raw conditional')
        eq(wanted['sha256'],sha(data),'loaded source bytes before any raw conditional')
        eq(str(Path(source_root)/wanted['path']),source['path'],'actual display path bound to source request path')
    if 'route' in actual:eq(case.case['route'],actual['route'],'observed selected route')
    rejected=expected.get('status')=='reject' or expected.get('check')=='reject'
    if actual.get('raw') is not None or actual.get('raw_before_audit') is not None:
        report['association']=association(case,actual)
    if rejected:
        assert 'raw' not in actual,'source rejection returned checked witness'
        rows=actual.get('diagnostics',[]);eq(1,len(rows),'single specified source diagnostic')
        eq(expected_diagnostic(expected),diagnostic(rows[0],loaded),'source rejection')
        eq({},actual['reference'],'rejected source has no execution');report['source_check']='EXPECTED_REJECTION';return report
    assert 'raw' in actual,'accepted source has no actual checked raw witness'
    eq([],actual.get('diagnostics',[]),'accepted source diagnostics')
    report['source_check']='ACCEPT';report['correspondence']=correspondence(case,actual['raw'])
    wanted_names={'reference-default'}|{'reference-fuel-'+str(b['budget']) for b in case.case.get('budget_cases',[])}
    eq(wanted_names,set(actual['reference']),'complete exact reference request inventory')
    runs=[]
    for name,row in actual['reference'].items():
        budget=None if name=='reference-default' else int(name[len('reference-fuel-'):]);want=case.expected_reference(budget)
        outcome=row['outcome']
        if want.get('failure'):
            eq(expected_diagnostic(want['failure']),diagnostic(outcome,loaded),'reference failure')
        else:
            value={'type':'unit'} if want['result_type']=='()' else {'type':want['result_type'],'value':want['result']}
            eq({'result':value},outcome,'reference scalar result')
        normalized=normalize(actual['raw'],actual['route'],row['trace'],budget)
        if case.facts:
            eq(case.expected_charges(budget),normalized['charges'],'complete operation/target/owner/fuel/origin prefix')
            eq(case.expected_writes(budget),normalized['writes'],'committed scalar/field/replacement write prefix')
            eq(loan_groups(case.expected_loans(budget)),loan_groups(normalized['loans']),'ordered acquisition and exact atomic-return loan membership/lifetime correspondence')
            eq(case.expected_transfers(budget),normalized['transfers'],'source-to-temporary/staging/parameter/result transfer identity correspondence')
        elif (want.get('failure') or {}).get('code')=='E0600':eq([],normalized['charges'],'entry rejection before consumer work')
        if not want.get('failure'):
            eq([],normalized['live_loan_handles'],'normal run releases all loans')
            eq([],normalized['active_call_stack'],'successful run returns every actual activation including root')
            eq(False,normalized['ended_with_uncommitted_operation'],'successful run commits final operation')
        runs.append(dict(name=name,charges=len(normalized['charges']),committed_writes=len(normalized['writes']),loan_events=len(normalized['loans']),
                         actual_activation_count=normalized['activations'],runtime_epoch_slots=normalized['runtime_epoch_slots'],
                         runtime_epoch_transitions=len(normalized['raw_epoch_journal']),status='MATCH'))
    report['runs']=runs;return report

def read_actual(receipt_path,receipt):
    eq('observed',receipt['status'],'observer completed');eq(0,receipt['exit_code'],'observer process exit')
    row=next(x for x in receipt['artifacts'] if Path(x['path']).name=='observations.json')
    if 'compressed' in row:
        compressed=Path(row['compressed']['path']).read_bytes();eq(row['compressed']['sha256'],sha(compressed),'compressed actual identity');data=gzip.decompress(compressed)
    else:data=Path(row['path']).read_bytes()
    eq(row['bytes'],len(data),'actual receipt byte count');eq(row['sha256'],sha(data),'actual receipt hash')
    return strict_json(data)

def main():
    p=argparse.ArgumentParser();p.add_argument('receipts',type=Path);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--allow-subset',action='store_true');p.add_argument('--profiles',default='debug,release');args=p.parse_args()
    cases=load();results=[];seen=set();profiles=set(args.profiles.split(','))
    for path in sorted(args.receipts.rglob('receipt.json')):
        receipt=strict_json(path.read_bytes());id=receipt['case'];profile=receipt['profile']
        if profile not in profiles:continue
        assert id in cases;identity=(id,profile);assert identity not in seen,('duplicate observation',identity);seen.add(identity)
        row=dict(id=id,profile=profile,receipt=str(path),receipt_sha256=sha(path.read_bytes()))
        try:
            eq(cases[id].case['source_manifest'],receipt['source_request']['source_files'],'request source identity')
            actual=read_actual(path,receipt)
            source_root=(Path(receipt['request_file']['path']).parent/receipt['source_request']['source_root']).resolve()
            row['result']=compare_case(cases[id],actual,source_root);row['status']='MATCH'
        except Exception as error:
            row['status']='MISMATCH';row['error']=repr(error);row['traceback']=traceback.format_exc()
        results.append(row)
    assert results,'no actual source receipts'
    if not args.allow_subset:eq({(id,p) for id in cases for p in profiles},seen,'complete profile corpus')
    report=dict(schema='unit3-independent-actual-comparison-v1',cases=len(results),matched=sum(r['status']=='MATCH' for r in results),
                mismatches=sum(r['status']=='MISMATCH' for r in results),results=results,
                source_expectations='unchanged original freeze plus accepted source-only declaration supplement',
                comparator_files=[dict(path=p.name,sha256=sha(p.read_bytes())) for p in sorted(Path(__file__).parent.glob('*.py'))])
    args.output.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k not in ('results','comparator_files')}))
    return int(report['mismatches']!=0)

if __name__=='__main__':raise SystemExit(main())
