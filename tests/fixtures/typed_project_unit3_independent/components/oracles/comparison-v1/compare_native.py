#!/usr/bin/env python3
"""Exact native/driver stream comparison against the reviewed frozen supplement."""
import sys
if sys.flags.optimize:
    raise RuntimeError("Unit3 verification requires Python assertions (PYTHONOPTIMIZE=0)")

from pathlib import Path
import argparse,gzip,hashlib,importlib.util,json
from expected_projection import load,verify_manifest,PACKAGE

SUP='9cae46e804a2776eb44732ced7201f99e37312346ef81958d025ee255f644c0a'
FIXTURE_ROOT=PACKAGE.parent/'typed-project-unit3-native-driver-review/fixtures'
def sha(data):return hashlib.sha256(data).hexdigest()
def main():
    p=argparse.ArgumentParser();p.add_argument('receipts',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    directory=PACKAGE/'supplements/native-diagnostics-v1';verify_manifest(directory/'supplement-manifest.json',SUP)
    expectations=json.loads((directory/'expectations.json').read_text());cases=load()
    spec=importlib.util.spec_from_file_location('frozen_native_expectations',directory/'build.py');builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(builder)
    results=[]
    for file in sorted(a.receipts.rglob('receipt.json')):
        receipt=json.loads(file.read_text());id=receipt['id'];case=cases[id].case
        assert receipt['source_files']==case['source_manifest'];assert receipt['fixture_root']==str(FIXTURE_ROOT/id)
        invocation=json.loads((file.parent/'invocation.json').read_text())
        # The invocation file is retained as a separate source-only record.
        assert receipt['entry']==str(FIXTURE_ROOT/id/'main.ox')
        operation=receipt['operation'];fmt=receipt['format'];budget=receipt.get('fuel');no_clobber=receipt.get('noclobber_before') is not None
        driver_case=case
        if case['expected'].get('native',{}).get('stage')=='oir-run':
            # Collision controls carry an executable runtime diagnostic under
            # native; that is not a native compilation/admission denial.
            default=next(x for x in expectations['defaults'] if x['id']==id)
            assert default['compile']=='success' and default['executable']
            driver_case={**case,'expected':{k:v for k,v in case['expected'].items() if k!='native'}}
        outcome=builder.driver(driver_case,operation,fmt,no_clobber and not driver_case['expected'].get('native'))
        selected_fuel=None
        if budget is not None:
            selected_fuel=next(x for x in expectations['fuel'] if x['id']==id and x['budget']==budget)
            if operation=='run':
                assert fmt=='text','additional JSON reduced-run stream requires separately frozen framing'
                outcome={k:selected_fuel[k] for k in ('exit_status','stdout','stderr')}
        def substitute(text):
            text=text.replace('${FIXTURE_ROOT}',str(FIXTURE_ROOT))
            if '${OUTPUT}' in text:
                assert isinstance(receipt['output'],str);text=text.replace('${OUTPUT}',receipt['output'])
            return text
        def streams(actual,want,label):
            assert actual['status']==want['exit_status'],(id,label,'status',want['exit_status'],actual['status'])
            for key in ('stdout','stderr'):
                expected=substitute(want[key]);assert actual[key]==expected,(id,label,key,expected,actual[key])
                assert actual[key+'_sha256']==sha(actual[key].encode()),(id,'stream hash',key)
        streams(receipt['compiler'],outcome,'driver')
        for key in ('stdout','stderr'):assert (file.parent/key).read_bytes()==receipt['compiler'][key].encode()
        if outcome['exit_status'] or operation!='compile':assert receipt['tools']==[],(id,'unexpected external tool invocation')
        executed=False
        if 'execution' in receipt:
            wanted=selected_fuel or next(x for x in expectations['defaults'] if x['id']==id)
            assert wanted['compile']=='success' and wanted['executable'];streams(receipt['execution'],wanted,'source-free ELF');executed=True
            for key in ('stdout','stderr'):assert (file.parent/'execution'/key).read_bytes()==receipt['execution'][key].encode()
            assert receipt['execution']['source_unavailable'] and receipt['source_restored_after_execution']
            assert receipt['execution']['cwd_before_files']==['program']
            assert receipt['execution']['elf_sha256']==receipt['artifact_sha256']
        elif operation=='compile' and not outcome['exit_status']:
            # Driver-only compiles may deliberately omit execution; preserve
            # that scope instead of counting an absent ELF run as a success.
            assert receipt['group'] in ('driver',),('missing requested ELF execution',id,receipt['group'])
        results.append(dict(id=id,profile=receipt['profile'],operation=operation,format=fmt,fuel=budget,no_clobber=no_clobber,
            driver_streams='EXACT_MATCH',elf_streams='EXACT_MATCH' if executed else 'NOT_EXECUTED',receipt=str(file),receipt_sha256=sha(file.read_bytes())))
    assert results,'no actual receipts'
    report=dict(schema='unit3-independent-native-stream-comparison-v1',expectation_manifest=SUP,comparator_sha256=sha(Path(__file__).read_bytes()),
        receipts=len(results),actual_elf_runs=sum(r['elf_streams']=='EXACT_MATCH' for r in results),results=results,
        scope='Exact frozen semantic streams/status only; core reviewer separately checks build/protocol/tool/source-unavailability provenance')
    a.output.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='results'}))

if __name__=='__main__':main()
