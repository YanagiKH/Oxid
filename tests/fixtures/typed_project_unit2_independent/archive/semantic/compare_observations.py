#!/usr/bin/env python3
"""Comparison process only. This owns expected artifacts; the observer does not."""
from pathlib import Path
import copy, importlib.util, json, sys

ROOT=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('frozen_comparator',ROOT.parent/'typed-project-unit2-oracles/adapter_protocol.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
profile=sys.argv[1];actual_path=Path(sys.argv[2])
prose={(r['case'],r['diagnostic_index']):r['expected'] for r in json.loads((ROOT/'original93-prose-expectations.json').read_text())['diagnostics']}
scalar=json.loads((ROOT/'legacy-scalar-schedule-clarification.json').read_text())['cases']
results=[]

def equal(want,got,label,failures):
    if want!=got:failures.append({'at':label,'expected':want,'actual':got})


for line in actual_path.read_text().splitlines():
    item=json.loads(line);cohort,case=item['case'].split('/',1)
    failures=[];unavailable=[]
    if 'normalization_error' in item:
        results.append({'case':item['case'],'status':'normalization-error','error':item['normalization_error']});continue
    actual=item['result']
    if cohort in ('original93','visibility','imports','import-replacement'):
        root=ROOT.parent/'typed-project-unit2-oracles/cases' if cohort=='original93' else ROOT/'module-alias-replacements/cases' if cohort=='import-replacement' else ROOT/(cohort+'-cases' if cohort=='visibility' else 'import-cases')
        expected=json.loads((root/case/'adapter-expected.json').read_text())
        if cohort=='original93':
            for index,d in enumerate(expected['diagnostics']):
                if (case,index) in prose:d.update(prose[(case,index)])
        failures=module.compare(expected,actual)
    elif cohort in ('parser','grammar-amendment'):
        root=ROOT/('parser-cases' if cohort=='parser' else 'grammar-amendment-cases')
        expected=json.loads((root/case/'expected.json').read_text())
        if 'parser_accepts' in expected:
            ok=actual['parser_ok']
            if ok is None:ok=actual['load_ok']
            equal(expected['parser_accepts'],ok,'parser_accepts',failures)
        if 'load_accepts' in expected:equal(expected['load_accepts'],actual['load_ok'],'load_accepts',failures)
        if 'syntax_flavor' in expected:equal(expected['syntax_flavor'],actual['syntax_flavor'],'syntax_flavor',failures)
        if 'first_diagnostic' in expected:
            got={k:actual['diagnostics'][0][k] for k in expected['first_diagnostic']} if actual['diagnostics'] else None
            equal(expected['first_diagnostic'],got,'first_diagnostic',failures)
        if expected.get('index_collection_started') is False:
            equal(False,'collection' in actual['actual_phases'],'index_collection_started',failures)
        for prop in ('children_read','logical_read_attempts'):
            if prop not in expected:continue
            if 'program_read_source_entries' not in item:
                unavailable.append(prop+': program-level read_source-entry observation unavailable')
                continue
            got=[r['display_file'] for r in item['program_read_source_entries']]
            if prop=='children_read':
                equal(1,got.count('root.ox'),'root_read_source_entries',failures)
                equal(expected[prop],[p for p in got if p!='root.ox'],'child_read_source_entries',failures)
            else:
                want=['root.ox'] if case=='missing-child-before-original-conflict' else expected[prop]
                equal(want,got,'logical_read_source_entries',failures)
    elif cohort=='legacy':
        expected=json.loads((ROOT/'legacy-cases'/case/'expected.json').read_text())
        projected=[]
        for index,d in enumerate(actual['diagnostics']):
            fields=expected['diagnostics'][index].keys() if index<len(expected['diagnostics']) else d.keys()
            projected.append({k:d[k] for k in fields})
        equal(expected['diagnostics'],projected,'diagnostics',failures)
        for prop,phase in [('bodies_started','body-resolution'),('signatures_started','signatures')]:
            if prop in expected:equal(expected[prop],phase in actual['actual_phases'],prop,failures)
        if case in scalar:
            want=scalar[case]['signatures_started'];starts=actual['signature_starts']
            equal(want,len(starts),'signature_start_count',failures)
            equal(list(range(want)),[s['function'] for s in starts],'signature_start_ids',failures)
        if case.startswith('owned-fields-before-functions-'):
            count=int(case.rsplit('-',1)[1])
            equal(min(count,100),len(actual['record_starts']),'record_start_count',failures)
            equal(1 if count==99 else 0,len(actual['signature_starts']),'signature_start_count',failures)
    else:raise ValueError(cohort)
    results.append({'case':item['case'],'status':'mismatch' if failures else 'partial' if unavailable else 'match',
                    'failures':failures,'unavailable':unavailable})
counts={s:sum(r['status']==s for r in results) for s in sorted({r['status'] for r in results})}
report={'profile':profile,'normalized_observations':str(actual_path),'cases':len(results),'counts':counts,'results':results}
out=Path(sys.argv[3]) if len(sys.argv)>3 else ROOT/'execution'/f'{profile}-comparison-initial.json'
assert not out.exists(), 'Preserve earlier comparison results'
out.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'profile':profile,'cases':len(results),'counts':counts,'first_mismatches':[r for r in results if r['status']=='mismatch'][:12]},indent=2))
