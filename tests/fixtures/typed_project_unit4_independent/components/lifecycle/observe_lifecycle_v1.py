#!/usr/bin/env python3
"""Actual isolated public-CLI logical lifecycle and ordinary-binary passivity."""
import collections,copy,hashlib,importlib.util,json,os,subprocess,sys,time
from pathlib import Path
OUT=Path(__file__).resolve().parent;ROOT=OUT.parent
HP=ROOT/'typed-project-unit4-public-qualification/public_cli.py'
spec=importlib.util.spec_from_file_location('public_cli',HP);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
EXPECTED='5cf0205cc29d28a8982ae29e3ccd818d48f67814d17889e1b3df00e9002908bb'
EP=OUT/'lifecycle-expectations-v1.json';assert h.fsha(EP)==EXPECTED
E=h.load(EP);COUNTERS=E['counter_order'];FILE_COUNTERS=E['file_counter_order']

def verify_trace(trace,expected):
    phases={k:sum(e['event']==k for e in trace) for k in COUNTERS}
    h.need(phases==expected['expected_lifecycle'],f'phase counts {phases} != {expected["expected_lifecycle"]}')
    files={}
    for event in trace:
        kind=event['event'];subject=event['subject']
        h.need(kind in COUNTERS+FILE_COUNTERS+['consumer_entry'],'unknown event')
        if kind in FILE_COUNTERS:
            files.setdefault(subject,{k:0 for k in FILE_COUNTERS})[kind]+=1
    h.need(files==expected['file_counts'],f'file lifecycle {files} != {expected["file_counts"]}')
    for subject,counts in files.items():
        actual=[e['event'] for e in trace if e['subject']==subject and e['event'] in FILE_COUNTERS]
        wanted=[k for k in FILE_COUNTERS for _ in range(counts[k])]
        h.need(actual==wanted,'per-file phase ordering')
    phase_order=['checker_attempts','route_attempts','route_completions','index_attempts','index_completions','checked_program_completions','consumer_entry']
    actual=[e['event'] for e in trace if e['event'] in phase_order]
    counts={**phases,'consumer_entry':phases['checked_program_completions']}
    wanted=[k for k in phase_order for _ in range(counts[k])]
    h.need(actual==wanted,'checker/route/index/consumer ordering')
    if phases['checker_attempts']:
        first=next(i for i,e in enumerate(trace) if e['event']=='checker_attempts')
        h.need(not any(e['event'] in FILE_COUNTERS for e in trace[first:]),'source operation after load/check boundary')
    if expected['expected_route'] is not None:
        h.need([x['subject'] for x in trace if x['event']=='route_completions']==[expected['expected_route']],'wrong route')
    if expected['expected_flavor'] is not None:
        h.need([x['subject'] for x in trace if x['event']=='checker_attempts']==[expected['expected_flavor']],'wrong flavor/checker')
    return {'phase_counts':phases,'file_counts':files,'consumer_source_reopens':0}

def controls(trace,expected):
    verify_trace(trace,expected);result=[]
    variants={
      'zero-events':[],
      'duplicate-source-read':trace+[next(copy.deepcopy(e) for e in trace if e['event']=='source_read_attempt')],
      'duplicate-checker':trace+[{'event':'checker_attempts','subject':'project'}],
      'missing-index-completion':[e for e in trace if e['event']!='index_completions'],
      'missing-checked-completion':[e for e in trace if e['event']!='checked_program_completions'],
      'consumer-source-reopen':trace+[{'event':'source_open_attempt','subject':'main.ox'}],
      'wrong-route':[{**e,'subject':'owned' if e['subject']=='scalar' else 'scalar'} if e['event']=='route_completions' else e for e in trace],
      'reordered-phases':list(reversed(trace))}
    for name,v in variants.items():
      try:verify_trace(v,expected)
      except h.Reject as err:result.append({'id':name,'status':'REJECTED','reason':str(err)})
      else:raise h.Reject('accepted mutation '+name)
    return result

def run():
    run=OUT/'lifecycle-run-v1';run.mkdir(exist_ok=False)
    manifest=OUT/'observer-source-manifest-v1.json';mb=h.fsha(manifest)
    for f in h.load(manifest)['files']:h.verify_file(f,OUT/'observer-source-v1')
    observed={r['profile']:r for r in h.load(OUT/'binaries-v1.json')['binaries']}
    ordinary={r['profile']:r for r in h.load(ROOT/'typed-project-unit4-dispatch-evidence/binaries-v1.json')['executables']}
    for f in list(observed.values())+list(ordinary.values()):h.verify_file(f)
    prior_path=ROOT/'typed-project-unit4-public-qualification/run-v1/observations.jsonl'
    prior={r['key']:r for r in [json.loads(l) for l in prior_path.read_text().splitlines()]}
    contract,_=h.authorities();roster=h.roster(contract);public={x['id']:x for x in E['public_cases']}
    env={**os.environ,'OXID_LLVM_BIN':str(h.LLVM),'LD_LIBRARY_PATH':str(h.LIBS)}
    env.pop('UNIT4_LIFECYCLE_LOG',None)
    records=[];failures=[];negative=None
    def execute(key,profile,fixture,args,expectation,base):
      nonlocal negative
      log=fixture.parent/'lifecycle.jsonl';before=h.inventory(fixture)
      process=h.proc([observed[profile]['path']]+args,fixture,{**env,'UNIT4_LIFECYCLE_LOG':str(log)})
      trace=[json.loads(l) for l in log.read_text().splitlines()] if log.exists() else []
      result={'key':key,'profile':profile,'process':process,'trace':trace,'directory_before':before,
       'directory_after':h.inventory(fixture),'ordinary_process':base['process'],
       'ordinary_record_sha256':h.sha(json.dumps(base,sort_keys=True).encode()),
       'observer_binary_sha256':observed[profile]['sha256'],'ordinary_binary_sha256':ordinary[profile]['sha256']}
      for stream in ('stdout','stderr'):(fixture.parent/stream).write_text(process[stream])
      try:
        for field in ('status','stdout','stderr'):h.need(process[field]==base['process'][field],'passivity '+field)
        h.need(result['directory_after']==base['directory_after'],'passivity final directory/ELF identity')
        result['lifecycle']=verify_trace(trace,expectation)
        h.need(not process['timed_out'],'timed out')
        if 'expected_status' in expectation:h.need(process['status']==expectation['expected_status'],'heldout literal status')
        result['status']='PASS'
        if negative is None and expectation['expected_lifecycle']['checked_program_completions']:
            negative=controls(trace,expectation)
      except h.Reject as e:result['status']='FAIL';result['failure']=str(e);failures.append({'key':key,'reason':str(e)})
      h.save(fixture.parent/'receipt.json',result);records.append(result)
      with (run/'observations.jsonl').open('a') as f:f.write(json.dumps(result,sort_keys=True)+'\n')
    for i,row in enumerate(roster):
      if not row['applicable']:continue
      evidence=run/f'public-{i:03d}';evidence.mkdir();fixture=evidence/'fixture';h.materialize(row,fixture)
      execute(row['key'],row['profile'],fixture,row['argv_template'][1:],public[row['case_id']],prior[row['key']])
    for i,case in enumerate(E['heldout_cases']):
      for profile in ('debug','release'):
        evidence=run/f'heldout-{i:02d}-{profile}';evidence.mkdir()
        for role in ('ordinary','fixture'):
          root=evidence/role;root.mkdir()
          for name,data in case['files_hex'].items():
            p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(bytes.fromhex(data))
        p=h.proc([ordinary[profile]['path']]+case['argv'],evidence/'ordinary',env)
        base={'process':p,'directory_after':h.inventory(evidence/'ordinary')}
        execute('heldout/'+case['id']+'/'+profile,profile,evidence/'fixture',case['argv'],case,base)
    for f in list(observed.values())+list(ordinary.values()):h.verify_file(f)
    for f in h.load(manifest)['files']:h.verify_file(f,OUT/'observer-source-v1')
    h.need(mb==h.fsha(manifest),'observer manifest changed')
    h.save(run/'negative-controls.json',negative)
    report={'status':'PASS' if not failures else 'FAIL','public_observed_rows':140,'heldout_observed_rows':30,
      'heldout_ordinary_rows':30,'passivity_pairs':len(records),'logical_phase_rows':len(records),'failures':failures,
      'host_inapplicable':6,'observer_manifest_sha256':mb,'expectations_sha256':EXPECTED,
      'public_ordinary_observations_sha256':h.fsha(prior_path),'runner_sha256':h.fsha(__file__),
      'imported_public_harness_sha256':h.fsha(HP),'counter_semantics':'Logical operations; not OS syscall tracing',
      'passivity_scope':'Status/stdout/stderr and complete fixture directory identities, including generated ELF bytes. Excludes timing, memory, OOM and OS syscall counts.'}
    h.save(run/'comparison.json',report);print(json.dumps(report,indent=2));return bool(failures)
if __name__=='__main__':sys.exit(run())
