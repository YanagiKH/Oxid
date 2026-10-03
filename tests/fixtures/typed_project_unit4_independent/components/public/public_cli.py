#!/usr/bin/env python3
"""Independent observations against frozen Unit4 public CLI expectations.
Never builds production code or modifies expected files.
"""
import argparse, copy, hashlib, json, os, platform, shutil, stat, struct, subprocess, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = Path(__file__).resolve().parent
ORACLE = ROOT / 'typed-project-unit4-oracles'
CONTRACT_HASH = '1cd3b42d2f93158e88b6a37ac99b89b9a10e85c926e2e7eb4d63675ab170073f'
FREEZE_HASH = '0407021036765aa7c4afbce7b101186ecab4e55c75678628f5bdf9e68a92984d'
LLVM = ROOT / 'toolchains/llvm19/usr/lib/llvm-19/bin'
LIBS = ROOT / 'toolchains/llvm19/usr/lib/x86_64-linux-gnu'

class Reject(Exception): pass

def need(condition, message):
    if not condition: raise Reject(message)

def sha(data): return hashlib.sha256(data).hexdigest()
def fsha(path): return sha(Path(path).read_bytes())
def load(path): return json.loads(Path(path).read_text())
def save(path, value): Path(path).write_text(json.dumps(value, indent=2, sort_keys=True)+'\n')
def binding(path):
    path = Path(path)
    return {'path':str(path), 'bytes':path.stat().st_size, 'sha256':fsha(path)}
def verify_file(row, base=ORACLE):
    p=Path(row['path']); p=p if p.is_absolute() else base/p
    need(p.is_file(),f'missing authority {p}')
    need(p.stat().st_size==row['bytes'] and fsha(p)==row['sha256'],f'changed authority {p}')
    return binding(p)

def authorities():
    need(fsha(ORACLE/'public-cli-contract-v3.json')==CONTRACT_HASH,'public contract hash')
    need(fsha(ORACLE/'contract-freeze-v3.json')==FREEZE_HASH,'freeze hash')
    verified=[]
    for version in (1,2,3):
        freeze=load(ORACLE/f'contract-freeze-v{version}.json')
        for row in freeze['files']: verified.append(verify_file(row))
        for k in ('parent_freeze','basis_review','active_parser_contract','active_public_contract','unchanged_predecessor_reuse'):
            if k in freeze: verified.append(verify_file(freeze[k]))
    reuse=load(ORACLE/'predecessor-reuse-v1.json')
    for row in reuse['bindings']: verified.append(verify_file(row))
    c=load(ORACLE/'public-cli-contract-v3.json')
    source_count=0
    for case in c['literal_cases']+c['inherited_negative_and_route_cases']:
        src=source_root(case)
        need(len({x['path'] for x in case['source_files']})==len(case['source_files']),'duplicate source path')
        for row in case['source_files']: verified.append(verify_file(row,src)); source_count+=1
    return c,{'verified_bindings':verified,'case_source_bindings':source_count,'predecessor_bindings':len(reuse['bindings'])}

def source_root(case):
    p=Path(case['source_root']); return p if p.is_absolute() else ORACLE/p

def host():
    system=platform.system(); system='macOS' if system=='Darwin' else system
    return system+' '+platform.machine()

def roster(contract):
    result=[]
    for group in ('literal_cases','inherited_negative_and_route_cases'):
        for case in contract[group]:
            ops=case.get('observations',case.get('operations'))
            for i,op in enumerate(ops):
                obs=op if isinstance(op,dict) else {'argv':op}
                hosts=obs.get('required_hosts',case.get('required_hosts',case.get('hosts')))
                need(bool(hosts),f'missing host scope {case["id"]}')
                for profile in contract['profiles']:
                    result.append({'key':f'{group}/{case["id"]}/{i}/{profile}/{host()}',
                        'group':group,'case_id':case['id'],'operation_index':i,'profile':profile,
                        'host':host(),'required_hosts':hosts,'applicable':any(host()==x or host().startswith(x+' ') for x in hosts),
                        'argv_template':obs['argv'],'entry':case['entry'],'source_files':case['source_files'],
                        'case':case,'observation':obs})
    need(len({r['key'] for r in result})==len(result),'duplicate roster key')
    return result

def projection_location(actual):
    if actual is None: return None
    return {'file':actual['file_id'],'path':actual['path'],'start':actual['start'],'end':actual['end'],
            'line':actual['line'],'scalar_column':actual['column']}

def diagnostics(actual, expected):
    need(len(actual)==len(expected),'diagnostic count')
    for a,e in zip(actual,expected):
        need(a['kind']=='diagnostic' and a['severity']=='error','diagnostic kind/severity')
        for k in ('code','stage'): need(a[k]==e[k],f'diagnostic {k}')
        need(projection_location(a['primary'])==e['location'],'diagnostic primary location')
        secondary=[projection_location(x['span'] if 'span' in x else x) for x in a['secondary']]
        need(secondary==e['related_locations'],'diagnostic secondary locations')
        need(isinstance(a['message'],str) and len(a['message'])>0,'diagnostic message missing')
        need(isinstance(a['notes'],list),'diagnostic notes missing')

def check_inherited(row, got):
    case=row['case']; op=row['argv_template'][1]; e=case['literal_accepted_expected_projection']
    lines=[json.loads(x) for x in got['stdout'].splitlines()]
    need(bool(lines),'missing JSON output')
    for obj in lines: need(obj['schema_version']==1 and obj['edition']=='typed-preview','JSON schema/edition')
    need(got['stderr']=='','inherited stderr')
    summ=lines[-1]; diags=lines[:-1]
    need(summ['kind']==op+'-summary','summary kind')
    rejection=e if e.get('status')=='reject' else e.get('native' if op=='compile' else op)
    rejected=isinstance(rejection,dict)
    need(got['status']==(1 if rejected else 0),'inherited process status')
    need(summ['success']==(not rejected),'summary success')
    need(summ['errors']==len(diags),'summary error count')
    if rejected:
        expected=rejection.get('reachable_diagnostics',[rejection])
        diagnostics(diags,expected)
        field={'check':'functions','run':'result','compile':'output'}[op]
        need(summ[field] is None,'rejection summary payload')
    else:
        need(diags==[],'unexpected inherited diagnostic')
        if op=='check': need(summ['functions']==case['expected_function_count'],'function count')
        elif op=='run': need(summ['result']=={'type':e['result_type'],'value':e['result']},'run result')
        else: need(summ['output']=='out.bin','compile output')
    return not rejected

def inventory(root):
    result=[]
    for p in sorted(root.rglob('*')):
        rel=p.relative_to(root).as_posix()
        if p.is_symlink(): result.append({'path':rel,'kind':'symlink','target':os.readlink(p)})
        elif p.is_dir(): result.append({'path':rel,'kind':'directory'})
        else: result.append({'path':rel,'kind':'file','bytes':p.stat().st_size,'sha256':fsha(p)})
    return result

def sources_from_inventory(inv):
    return [{k:r[k] for k in ('path','bytes','sha256')} for r in inv if r['kind']=='file' and r['path']!='out.bin']

def materialize(row, target):
    target.mkdir(parents=True,exist_ok=False)
    for src in row['source_files']:
        path=Path(src['path'])
        need(not path.is_absolute() and '..' not in path.parts,'unsafe source relative path')
        dest=target/path; dest.parent.mkdir(parents=True,exist_ok=True)
        data=(source_root(row['case'])/path).read_bytes()
        need(len(data)==src['bytes'] and sha(data)==src['sha256'],'source materialization authority')
        dest.write_bytes(data)
    need(sorted(sources_from_inventory(inventory(target)),key=lambda x:x['path'])==sorted(row['source_files'],key=lambda x:x['path']),'materialized source map')
    need(not os.path.lexists(target/'out.bin'),'initial output exists')

def proc(argv,cwd,env,timeout=120):
    start=time.time_ns()
    try:
        p=subprocess.run(argv,cwd=cwd,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=timeout)
        return {'argv':list(argv),'cwd':str(cwd),'status':p.returncode,'stdout':p.stdout.decode('utf-8'),
                'stderr':p.stderr.decode('utf-8'),'stdout_sha256':sha(p.stdout),'stderr_sha256':sha(p.stderr),
                'started_ns':start,'completed_ns':time.time_ns(),'timed_out':False}
    except subprocess.TimeoutExpired as e:
        return {'argv':list(argv),'cwd':str(cwd),'status':None,'stdout':(e.stdout or b'').decode('utf-8'),
                'stderr':(e.stderr or b'').decode('utf-8'),'stdout_sha256':sha(e.stdout or b''),'stderr_sha256':sha(e.stderr or b''),
                'started_ns':start,'completed_ns':time.time_ns(),'timed_out':True}

def tools_setup(run):
    wrappers=run/'tool-wrappers'; wrappers.mkdir()
    wrapper=(OUT/'tool_wrapper.py').read_bytes()
    for tool in ('clang','opt','ld.lld'):
        path=wrappers/tool; path.write_bytes(wrapper); path.chmod(0o755)
    env={**os.environ,'OXID_LLVM_BIN':str(wrappers),'LD_LIBRARY_PATH':str(LIBS), 'UNIT4_REAL_LLVM':str(LLVM)}
    results=[]
    for tool,marker in (('clang','clang version 19.1.7'),('opt','LLVM version 19.1.7'),('ld.lld','LLD 19.1.7')):
        need((LLVM/tool).is_file(),f'missing real LLVM {tool}')
        p=proc([str(LLVM/tool),'--version'],run,env)
        need(p['status']==0 and marker in p['stdout'],f'wrong real LLVM {tool}')
        results.append({'name':tool,'executable':binding(LLVM/tool),'version_process':p,'wrapper':binding(wrappers/tool)})
    return env, results

def native_execution(row, fixture, evidence, env):
    elf=fixture/'out.bin'; data=elf.read_bytes()
    need(data[:4]==b'\x7fELF' and data[4:6]==b'\x02\x01' and struct.unpack('<H',data[18:20])[0]==62,'output is not x86_64 ELF')
    dest=evidence/'native-source-free';dest.mkdir()
    copied=dest/'program';shutil.copy2(elf,copied)
    need(inventory(dest)==[{'path':'program','kind':'file','bytes':len(data),'sha256':sha(data)}],'source-free directory not ELF-only')
    source_bytes={s['path']:(fixture/s['path']).read_bytes() for s in row['source_files']}
    result={'original_elf':binding(elf),'copied_elf':binding(copied),'initial_directory':inventory(dest),'sources_removed':[]}
    try:
        for name in source_bytes: (fixture/name).unlink()
        result['sources_removed']=[name for name in source_bytes if not os.path.lexists(fixture/name)]
        need(len(result['sources_removed'])==len(source_bytes),'source removal incomplete')
        result['process']=proc([str(copied)],dest,env)
        result['final_directory']=inventory(dest)
    finally:
        for name,b in source_bytes.items(): (fixture/name).write_bytes(b)
        result['sources_restored']=[{'path':name,'bytes':len(b),'sha256':fsha(fixture/name)} for name,b in source_bytes.items()]
        result['restored_exactly']=all((fixture/name).read_bytes()==b for name,b in source_bytes.items())
    return result

def check_row(row, got, config, observer_sha):
    need(got['key']==row['key'],'row key')
    for k in ('case_id','group','operation_index','profile','host','applicable'):
        need(got[k]==row[k],f'row {k}')
    need(got['contract_sha256']==CONTRACT_HASH,'row contract')
    need(got['observer_sha256']==observer_sha,'row observer')
    need(got['candidate_binding_sha256']==config['_binding_sha256'],'candidate binding')
    if not row['applicable']:
        need(got['executed'] is False and got['skip_reason']=='host outside frozen required_hosts','bad host skip')
        return
    need(got['executed'] is True and 'skip_reason' not in got,'unexecuted applicable row')
    expected_bin=config['binaries'][row['profile']]
    need(got['executable_before']==expected_bin and got['executable_after']==expected_bin,'stale executable')
    need(got['source_files']==row['source_files'],'source binding')
    need(got['output_initially_absent'] is True,'initial output not absent')
    need(got['process']['argv']==[expected_bin['path']]+row['argv_template'][1:],'wrong argv')
    need(got['process']['cwd']==got['fixture_directory'],'wrong cwd')
    p=got['process'];need(not p['timed_out'] and p['completed_ns']>=p['started_ns'],'incomplete process')
    for stream in ('stdout','stderr'): need(sha(p[stream].encode())==p[stream+'_sha256'],f'{stream} bytes/hash')
    before=got['directory_before'];after=got['directory_after']
    need(sorted(sources_from_inventory(before),key=lambda x:x['path'])==sorted(row['source_files'],key=lambda x:x['path']),'before source map')
    need(sorted(sources_from_inventory(after),key=lambda x:x['path'])==sorted(row['source_files'],key=lambda x:x['path']),'after source map')
    if row['group']=='literal_cases':
        for k in ('status','stdout','stderr'): need(p[k]==row['observation'][k],f'literal {k}')
        success=p['status']==0
        delta=row['observation']['directory_delta']
    else:
        success=check_inherited(row,p)
        delta=['out.bin'] if success and row['argv_template'][1]=='compile' else []
    need([r['path'] for r in after if r not in before]==delta,'directory delta')
    need(all(r in after for r in before),'directory removed/changed input')
    calls=got['tool_invocations'];op=row['argv_template'][1]
    for call in calls:
        tool=config['_tool_bindings'][call['tool']]
        need(call['real_path']==tool['path'] and call['real_sha256']==tool['sha256'],'stale/wrong real tool')
    if op!='compile' or not success: need(calls==[],'unexpected external tool invocation')
    if op=='compile' and success:
        need(len(calls)>=8,'missing real LLVM invocations')
        need(all(x['status']==0 for x in calls),'LLVM invocation failure')
        need(any(x['tool']=='opt' and x['argv']==['-passes=verify','-disable-output','program.ll'] for x in calls),'missing LLVM verify')
        compiles=[x for x in calls if x['tool']=='clang' and '-c' in x['argv']]
        need(len(compiles)==2 and all('-O0' in x['argv'] for x in compiles),'missing O0 compilation')
        need(any(x['tool']=='ld.lld' and '--version' not in x['argv'] for x in calls),'missing real linker')
        need(got.get('emitted_llvm') is not None,'missing LLVM emission evidence')
        n=got['native_execution'];need(n['original_elf']['sha256']==n['copied_elf']['sha256'],'copied ELF identity')
        need(n['sources_removed']==[s['path'] for s in row['source_files']],'source-free removal roster')
        need(n['restored_exactly'] is True,'sources not restored')
        need(n['sources_restored']==row['source_files'],'restored source identities')
        need(n['initial_directory']==n['final_directory'] and len(n['initial_directory'])==1,'source-free directory mutation')
        native=row['case'].get('native_execution') or config.get('native_supplement',{}).get(row['case_id'])
        need(native is not None,'native expected envelope missing')
        for k in ('status','stdout','stderr'): need(n['process'][k]==native[k],f'native {k}')
        need(not n['process']['timed_out'],'native timed out')
        need(n['process']['cwd']==str(Path(n['copied_elf']['path']).parent),'native cwd')
        need(n['process']['argv']==[n['copied_elf']['path']],'native argv')

def validate(rows, observations, config, observer_sha):
    need(len(observations)>0,'zero execution roster')
    keys=[g['key'] for g in observations]
    need(len(keys)==len(set(keys)),'duplicate observation')
    need(set(keys)=={r['key'] for r in rows},'missing/extra roster')
    bykey={g['key']:g for g in observations}
    failures=[]
    for row in rows:
        try:check_row(row,bykey[row['key']],config,observer_sha)
        except (Reject,KeyError,TypeError,ValueError) as e: failures.append({'key':row['key'],'reason':str(e)})
    need(not failures,json.dumps(failures))
    return {'status':'PASS','observed':sum(g['executed'] for g in observations),'host_inapplicable':sum(not g['executed'] for g in observations),'rows':len(rows)}

def control_suite(rows, observations, config, observer_sha):
    positive=validate(rows,observations,config,observer_sha)
    idx=next(i for i,g in enumerate(observations) if g['executed'])
    nat=next(i for i,g in enumerate(observations) if g.get('native_execution'))
    cases=[]
    def mutate(name,fn):
        altered=copy.deepcopy(observations);fn(altered)
        try:validate(rows,altered,config,observer_sha)
        except (Reject,KeyError,TypeError,ValueError) as e: cases.append({'control':name,'status':'REJECTED','reason':str(e)[:1000]})
        else:raise Reject('negative control accepted: '+name)
    mutate('zero-execution-roster',lambda a:a.clear())
    mutate('missing-row',lambda a:a.pop(idx))
    mutate('duplicate-row',lambda a:a.append(copy.deepcopy(a[idx])))
    mutate('extra-row',lambda a:a.append({**copy.deepcopy(a[idx]),'key':'extra'}))
    for name,key,val in [('wrong-profile','profile','other'),('wrong-host','host','other'),('false-skip','executed',False),('stale-contract','contract_sha256','0'*64),('stale-observer','observer_sha256','0'*64)]:
        mutate(name,lambda a,k=key,v=val:a[idx].__setitem__(k,v))
    mutate('wrong-argv',lambda a:a[idx]['process']['argv'].append('--invented'))
    mutate('stale-binary',lambda a:a[idx]['executable_before'].__setitem__('sha256','0'*64))
    mutate('wrong-source',lambda a:a[idx]['source_files'][0].__setitem__('sha256','0'*64))
    mutate('wrong-status',lambda a:a[idx]['process'].__setitem__('status',42))
    for stream in ('stdout','stderr'):
        mutate('missing-'+stream,lambda a,s=stream:a[idx]['process'].pop(s))
        def corrupt(a,s=stream):
            a[idx]['process'][s]+='forged';a[idx]['process'][s+'_sha256']=sha(a[idx]['process'][s].encode())
        mutate('altered-'+stream,corrupt)
    mutate('unexpected-file',lambda a:a[idx]['directory_after'].append({'path':'cache','kind':'file','bytes':0,'sha256':sha(b'')}))
    mutate('missing-ELF',lambda a:a[nat].pop('native_execution'))
    mutate('changed-copied-ELF',lambda a:a[nat]['native_execution']['copied_elf'].__setitem__('sha256','0'*64))
    mutate('sources-available',lambda a:a[nat]['native_execution'].__setitem__('sources_removed',[]))
    mutate('unrestored-sources',lambda a:a[nat]['native_execution'].__setitem__('restored_exactly',False))
    mutate('missing-LLVM-emission',lambda a:a[nat].pop('emitted_llvm'))
    mutate('missing-tool-invocations',lambda a:a[nat].__setitem__('tool_invocations',[]))
    return {'positive':positive,'controls':cases,'all_rejected':True}

def read_config(path):
    cfg=load(path);cfg['_binding_sha256']=fsha(path)
    need(set(cfg['binaries'])=={'debug','release'},'binary profile roster')
    for row in cfg['binaries'].values(): verify_file(row)
    for row in cfg['authority_bindings']: verify_file(row)
    need(cfg['compiler_head'] and cfg['compiler_tree'],'missing compiler head/tree')
    if 'source_manifest_binding' in cfg:
        verify_file(cfg['source_manifest_binding'])
        for source in load(cfg['source_manifest_binding']['path'])['files']:
            verify_file(source,Path(cfg['source_root']))
    if 'native_supplement' in cfg:
        supplement=cfg['native_supplement_binding'];verify_file(supplement)
        need(supplement in cfg['authority_bindings'],'unbound native supplement')
        need(cfg['native_supplement']=={case['id']:case['native_execution'] for case in load(supplement['path'])['cases']},'native envelope differs from frozen supplement')
    return cfg

def run_all(config_path,run_name):
    contract,integrity=authorities();rows=roster(contract);config=read_config(config_path)
    run=OUT/run_name;run.mkdir(exist_ok=False)
    save(run/'input-integrity.json',integrity);save(run/'candidate-binding.json',config)
    observer=fsha(__file__);env,tool_bindings=tools_setup(run);save(run/'tool-bindings.json',tool_bindings)
    config['_tool_bindings']={x['name']:x['executable'] for x in tool_bindings}
    observations=[];op_failures=[]
    for i,row in enumerate(rows):
        got={k:copy.deepcopy(row[k]) for k in ('key','group','case_id','operation_index','profile','host','applicable')}
        got.update(contract_sha256=CONTRACT_HASH,observer_sha256=observer,candidate_binding_sha256=config['_binding_sha256'])
        if not row['applicable']:
            got.update(executed=False,skip_reason='host outside frozen required_hosts')
        else:
            evidence=run/f'{i:03d}-{row["profile"]}-{row["case_id"]}-{row["operation_index"]}';evidence.mkdir()
            fixture=evidence/'fixture';materialize(row,fixture)
            binary=config['binaries'][row['profile']]
            got.update(executed=True,fixture_directory=str(fixture),source_files=row['source_files'],executable_before=binding(binary['path']),
                output_initially_absent=not os.path.lexists(fixture/'out.bin'),directory_before=inventory(fixture))
            log=evidence/'tools.jsonl'
            per_env={**env,'UNIT4_TOOL_LOG':str(log),'UNIT4_TOOL_EVIDENCE':str(evidence)}
            got['process']=proc([binary['path']]+row['argv_template'][1:],fixture,per_env)
            (evidence/'stdout').write_text(got['process']['stdout']);(evidence/'stderr').write_text(got['process']['stderr'])
            got['tool_invocations']=[json.loads(x) for x in log.read_text().splitlines()] if log.exists() else []
            ir=evidence/'program.ll';got['emitted_llvm']=binding(ir) if ir.exists() else None
            if got['process']['status']==0 and row['argv_template'][1]=='compile':
                got['native_execution']=native_execution(row,fixture,evidence,per_env)
            got['directory_after']=inventory(fixture);got['executable_after']=binding(binary['path'])
            save(evidence/'receipt.json',got)
        observations.append(got)
        with (run/'observations.jsonl').open('a') as f:f.write(json.dumps(got,sort_keys=True)+'\n')
        try:check_row(row,got,config,observer)
        except (Reject,KeyError,TypeError,ValueError) as e:
            op_failures.append({'key':row['key'],'reason':str(e)});print('FAIL',row['key'],str(e),flush=True)
    _,after=authorities();need(after==integrity,'authority changed during run')
    read_config(config_path)
    for tool in tool_bindings:verify_file(tool['executable']);verify_file(tool['wrapper'])
    report={'status':'FAIL' if op_failures else 'PASS','failures':op_failures,'executed':sum(x['executed'] for x in observations),
            'host_inapplicable':sum(not x['executed'] for x in observations),'rows':len(rows),'observer':binding(__file__),'tool_wrapper':binding(OUT/'tool_wrapper.py'),
            'observation_boundary':'actual public executable only; internal lifecycle/route/fuel facts are not observed',
            'contract_sha256':CONTRACT_HASH,'candidate_binding_sha256':config['_binding_sha256']}
    save(run/'comparison.json',report)
    if not op_failures:save(run/'negative-controls.json',control_suite(rows,observations,config,observer))
    print(json.dumps(report,indent=2));return 1 if op_failures else 0

def main():
    p=argparse.ArgumentParser();p.add_argument('command',choices=('prepare','run','verify'));p.add_argument('--binding');p.add_argument('--run',default='run-v1');a=p.parse_args()
    if a.command=='prepare':
        c,integrity=authorities();r=roster(c)
        save(OUT/'authority-integrity-v1.json',integrity)
        save(OUT/'roster-v1.json',[{k:v for k,v in x.items() if k not in ('case','observation')} for x in r])
        print(json.dumps({'rows':len(r),'execute':sum(x['applicable'] for x in r),'host_inapplicable':sum(not x['applicable'] for x in r),'sources':integrity['case_source_bindings'],'predecessors':integrity['predecessor_bindings']}));return 0
    need(a.binding is not None,'--binding required')
    if a.command=='run':return run_all(a.binding,a.run)
    c,_=authorities();cfg=read_config(a.binding);observations=[json.loads(l) for l in (OUT/a.run/'observations.jsonl').read_text().splitlines()]
    cfg['_tool_bindings']={x['name']:x['executable'] for x in load(OUT/a.run/'tool-bindings.json')}
    for tool in cfg['_tool_bindings'].values():verify_file(tool)
    print(json.dumps(validate(roster(c),observations,cfg,fsha(__file__)),indent=2));return 0
if __name__=='__main__':
    try:sys.exit(main())
    except Reject as e:print('REJECT:',e,file=sys.stderr);sys.exit(1)
